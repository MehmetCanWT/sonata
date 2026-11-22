//! Playback transitions, stream resolution, Hi-Fi upgrades, precaching, and cover enrichment.
//!
//! 1. **Transition & Recovery**: `start_play_transition` resets task state, upscales initial cover thumbnails, resolves fast-start streams/lyrics, and drives retry/recovery.
//! 2. **Hi-Fi & Cover Enrichment**: `start_hifi_upgrade` and `start_cover_enrichment` immediately upgrade streams and resolve high-res artwork for both YouTube Music and Spotify.
//! 3. **Precache & MPV Events**: `maybe_precache_next`, `handle_mpv_event`, and `export_now_playing` handle lookahead caching, natural track end, and `$XDG_RUNTIME_DIR/sonata/now_playing.json`.

use super::types::{ExportFingerprint, InternalEvent};
use super::BackendEngine;
use crate::integrations::mpris::MprisUpdate;
use sonata_core::lyrics::{active_line_index, lrclib, LyricsState};
use sonata_core::media;
use sonata_core::motion;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator, PlaybackTransition};
use sonata_core::player::{MpvEvent, PlaybackTask};
use sonata_core::providers::hifi::{ResolvedStream, StreamResolver};
use sonata_core::providers::ProviderSource;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl BackendEngine {
    pub(super) fn is_natural_track_end(position_secs: f64, duration_secs: f64) -> bool {
        if duration_secs <= 0.0 {
            return false;
        }
        position_secs >= (duration_secs - 2.5).max(0.0)
    }

    pub(super) fn should_ignore_stale_playback_error(
        loading_track: bool,
        stream_loaded_at: Option<Instant>,
        grace_elapsed: Duration,
    ) -> bool {
        loading_track || stream_loaded_at.is_none_or(|loaded| loaded.elapsed() < grace_elapsed)
    }

    pub(super) fn should_attempt_recovery(retried_play_seq: u64, play_seq: u64) -> bool {
        retried_play_seq != play_seq
    }

    pub(super) fn start_hifi_upgrade(&mut self) {
        if self.playback.paused
            || self.playback.loading_track
            || self.upgrade_started_play_seq == Some(self.play_seq)
        {
            return;
        }
        let (Some(track), Some(stream)) = (
            self.playback.current_track().cloned(),
            self.playback.active_stream.clone(),
        ) else {
            return;
        };
        if track.source == ProviderSource::Spotify
            && (!self.config.hifi_substitution
                || !StreamResolver::needs_hifi_upgrade(&stream.badge, &stream.url))
        {
            return;
        }

        let play_seq = self.play_seq;
        self.upgrade_started_play_seq = Some(play_seq);
        self.upgrading_quality = true;
        tracing::info!(
            event = "upgrade_resolving",
            client = "desktop",
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq,
            current_quality = %stream.badge,
            "resolving higher quality stream"
        );
        let resolver = self.resolver.clone();
        let int_tx = self.int_tx.clone();
        let allow_hifi = self.config.hifi_substitution;
        self.playback_tasks.spawn(async move {
            if let Some(upgraded) = resolver
                .resolve_hifi_upgrade(&track, &stream, allow_hifi)
                .await
            {
                let _ = int_tx.send(InternalEvent::StreamUpgrade {
                    play_seq,
                    track_id: track.id,
                    stream: upgraded,
                });
            } else {
                let _ = int_tx.send(InternalEvent::UpgradeFinished { play_seq });
                tracing::info!(
                    event = "upgrade_unavailable",
                    client = "desktop",
                    track_id = %track.id,
                    play_seq,
                    "no higher quality stream available"
                );
            }
        });
    }

    pub(super) fn start_cover_enrichment(&mut self) {
        if self.cover_started_play_seq == Some(self.play_seq) {
            return;
        }
        let Some(track) = self.playback.current_track().cloned() else {
            return;
        };

        let play_seq = self.play_seq;
        self.cover_started_play_seq = Some(play_seq);
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        let anim_http = self.http.clone();
        let anim_tx = self.int_tx.clone();
        let anim_track = track.clone();
        self.playback_tasks.spawn(async move {
            let Some((url, _source)) =
                media::resolve_track_cover(&http, cache_db.as_ref(), &track).await
            else {
                return;
            };
            let Some((image, _)) = media::fetch_and_decode_art(&http, &url).await else {
                return;
            };
            let ambient_rgb = media::extract_ambient_rgb(&image);
            let path = media::art_cache_path(&url);
            let _ = int_tx.send(InternalEvent::ArtworkEnhanced {
                play_seq,
                track_id: track.id,
                url,
                art_path: path.exists().then_some(path),
                ambient_rgb,
            });
        });
        if self.config.motion_covers && !self.config.reduce_motion {
            self.playback_tasks.spawn(async move {
                if let Some(cover) =
                    motion::resolve_motion_cover(&anim_http, &anim_track, 768, 24.0, 96).await
                {
                    let _ = anim_tx.send(InternalEvent::AnimatedArtworkReady {
                        play_seq,
                        track_id: anim_track.id,
                        cover: Arc::new(cover),
                    });
                }
            });
        }
    }

    pub(super) fn interpolated_position(&self) -> f64 {
        let pos = if let Some((base, at)) = self.pos_anchor {
            if !self.playback.paused && !self.playback.loading_track {
                base + at.elapsed().as_secs_f64()
            } else {
                base
            }
        } else {
            self.playback.position_secs
        };
        if self.playback.duration_secs > 0.0 {
            pos.clamp(0.0, self.playback.duration_secs)
        } else {
            pos.max(0.0)
        }
    }

    pub(super) fn tick_playback(&mut self) -> bool {
        if self.playback.paused
            || self.playback.loading_track
            || self.playback.current_track().is_none()
        {
            return false;
        }
        let next = self.interpolated_position();
        let changed = if (next - self.playback.position_secs).abs() >= 0.03 {
            self.playback.position_secs = next;
            true
        } else {
            false
        };
        self.poll_adopted_precache();
        self.maybe_precache_next();
        changed
    }

    pub(super) fn play_track_at(&mut self, idx: usize, push_history: bool) {
        self.play_track_with_stream(idx, push_history, None);
    }

    pub(super) fn play_track_with_stream(
        &mut self,
        idx: usize,
        push_history: bool,
        prepared_stream: Option<ResolvedStream>,
    ) {
        let Some(transition) = PlaybackCoordinator::apply(
            &mut self.playback,
            PlaybackCommand::Play {
                index: idx,
                push_history,
            },
        ) else {
            return;
        };
        self.start_play_transition(transition, prepared_stream);
    }

    pub(super) fn start_play_transition(
        &mut self,
        transition: PlaybackTransition,
        prepared_stream: Option<ResolvedStream>,
    ) {
        let PlaybackTransition::Play { track, .. } = transition else {
            return;
        };
        self.playback_tasks.restart();
        if let Some((_, task)) = self.next_prefetch_task.take() {
            task.abort();
        }
        let adopted_for_track = self
            .adopted_prefetch_task
            .as_ref()
            .is_some_and(|(track_id, _)| track_id == &track.id);
        if !adopted_for_track {
            if let Some((_, task)) = self.adopted_prefetch_task.take() {
                task.abort();
            }
        }
        self.next_precache.take();
        self.pos_anchor = None;
        self.stream_loaded_at = None;
        self.retried_play_seq = 0;
        self.upgrade_started_play_seq = None;
        self.upgrading_quality = false;
        self.lyrics = LyricsState::Loading;
        self.motion_cover = None;

        if let Some(url) = track
            .thumbnail_url
            .as_deref()
            .map(media::upscale_artwork_url)
        {
            let cached = media::art_cache_path(&url);
            self.art_path = cached.exists().then_some(cached);
        } else {
            self.art_path = None;
        }

        if let Some(mpv) = &self.mpv {
            mpv.stop();
        }

        self.play_seq = self.play_seq.wrapping_add(1);
        let play_seq = self.play_seq;
        if adopted_for_track {
            self.upgrade_started_play_seq = Some(play_seq);
        }
        self.play_requested_at = Some(Instant::now());
        tracing::info!(
            event = "resolving",
            client = "desktop",
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq,
            prepared = prepared_stream.is_some(),
            "resolving playback stream"
        );
        self.awaiting_playback_start = false;
        self.pending_auto_advance = None;
        self.cover_started_play_seq = None;
        self.status_line = format!("Loading: {} — {}", track.title, track.artist);

        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();

        let track_for_stream = track.clone();
        if let Some(stream) = prepared_stream {
            let _ = int_tx.send(InternalEvent::StreamReady {
                play_seq,
                track_id: track_for_stream.id.clone(),
                stream,
            });
        } else {
            let stream_tx = int_tx.clone();
            self.playback_tasks.spawn(async move {
                match resolver.resolve_fast(&track_for_stream).await {
                    Ok(stream) => {
                        let _ = stream_tx.send(InternalEvent::StreamReady {
                            play_seq,
                            track_id: track_for_stream.id,
                            stream,
                        });
                    }
                    Err(e) => {
                        let _ = stream_tx.send(InternalEvent::StreamError {
                            play_seq,
                            track_id: track_for_stream.id,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }

        let lyrics_track = track.clone();
        let lyrics_http = http.clone();
        let lyrics_cache = self.cache_db.clone();
        let lyrics_tx = int_tx.clone();
        self.playback_tasks.spawn(async move {
            let lyrics =
                lrclib::fetch_lyrics(&lyrics_http, &ytm, &lyrics_track, lyrics_cache.as_ref())
                    .await;
            let _ = lyrics_tx.send(InternalEvent::LyricsReady {
                play_seq,
                track_id: lyrics_track.id,
                lyrics,
            });
        });

        if let Some(url) = track
            .thumbnail_url
            .as_deref()
            .map(media::upscale_artwork_url)
        {
            let art_tx = int_tx;
            let track_id = track.id.clone();
            self.playback_tasks.spawn(async move {
                if let Some((img, _)) = media::fetch_and_decode_art(&http, &url).await {
                    let ambient = media::extract_ambient_rgb(&img);
                    let path = media::art_cache_path(&url);
                    let _ = art_tx.send(InternalEvent::ArtworkReady {
                        play_seq,
                        track_id,
                        url: url.clone(),
                        art_path: path.exists().then_some(path),
                        ambient_rgb: ambient,
                    });
                }
            });
        }

        self.start_cover_enrichment();
    }

    pub(super) fn maybe_precache_next(&mut self) {
        let current = self.playback.current_track().cloned();
        let next_track = self
            .playback
            .peek_next_index()
            .and_then(|index| self.playback.queue.get(index).cloned());
        let Some(current) = current else { return };
        let next_id = next_track.as_ref().map(|track| track.id.as_str());

        if let Some(cancelled) =
            self.next_precache
                .cancel_if_changed(self.play_seq, Some(&current.id), next_id)
        {
            if let Some((key, task)) = self.next_prefetch_task.take() {
                if key == cancelled {
                    task.abort();
                    tracing::info!(
                        event = "next_precache_cancelled",
                        client = "desktop",
                        track_id = %cancelled.next_track_id,
                        reason = "next-track-changed",
                        "next-track precache cancelled"
                    );
                } else {
                    self.next_prefetch_task = Some((key, task));
                }
            }
        }

        let Some(key) = self.next_precache.schedule(
            self.play_seq,
            &current.id,
            next_id,
            self.playback.position_secs,
            self.playback.duration_secs,
            self.config.cache_limit_gb > 0
                && (self.config.hifi_substitution || self.config.jiosaavn_enabled),
        ) else {
            return;
        };
        let Some(next_track) = next_track else { return };
        tracing::info!(
            event = "next_precache_scheduled",
            client = "desktop",
            track_id = %next_track.id,
            title = %next_track.title,
            remaining_secs = (self.playback.duration_secs - self.playback.position_secs).max(0.0),
            "next-track quality precache scheduled"
        );
        let resolver = self.resolver.clone();
        let hifi = self.config.hifi_substitution;
        let task = PlaybackTask::spawn_detached(async move {
            resolver.precache_hifi_audio(&next_track, hifi).await
        });
        self.next_prefetch_task = Some((key, task));
    }

    pub(super) fn poll_adopted_precache(&mut self) {
        let finished = self
            .adopted_prefetch_task
            .as_mut()
            .and_then(|(_, task)| task.try_take());
        let Some(result) = finished else { return };
        let Some((track_id, _)) = self.adopted_prefetch_task.take() else {
            return;
        };
        let Some(stream) = result else {
            tracing::info!(
                event = "next_precache_failed",
                client = "desktop",
                track_id = %track_id,
                reason = "quality-unavailable",
                "adopted next-track precache did not produce an upgrade"
            );
            return;
        };
        tracing::info!(
            event = "next_precache_adopted",
            client = "desktop",
            track_id = %track_id,
            quality = %stream.badge,
            "applying completed precache to current track"
        );
        let _ = self.int_tx.send(InternalEvent::StreamUpgrade {
            play_seq: self.play_seq,
            track_id,
            stream,
        });
    }

    fn prepare_next_before_advance(&mut self, index: usize) {
        // A quality prefetch must not delay the next audible track.
        let Some(track) = self.playback.queue.get(index).cloned() else {
            return;
        };
        if self
            .pending_auto_advance
            .as_ref()
            .is_some_and(|(seq, id)| *seq == self.play_seq && id == &track.id)
        {
            return;
        }
        self.pending_auto_advance = Some((self.play_seq, track.id.clone()));
        self.playback.loading_track = true;
        self.playback.paused = true;
        self.playback.stream_badge = "Preparing next track...".to_string();
        self.pos_anchor = None;
        self.status_line = format!("Caching next track: {}", track.title);

        let prepared = match self.next_prefetch_task.take() {
            Some((key, mut task)) if key.next_track_id == track.id => match task.try_take() {
                Some(result) => {
                    if result.is_some() {
                        self.next_precache.mark_ready(&key);
                    } else {
                        self.next_precache.mark_failed(&key);
                    }
                    result
                }
                None => {
                    self.adopted_prefetch_task = Some((track.id.clone(), task));
                    None
                }
            },
            Some((_, task)) => {
                task.abort();
                None
            }
            None => None,
        };
        let play_seq = self.play_seq;
        let resolver = self.resolver.clone();
        let int_tx = self.int_tx.clone();
        self.playback_tasks.spawn(async move {
            let result = match prepared {
                Some(stream) => Ok(stream),
                None => resolver
                    .resolve_fast(&track)
                    .await
                    .map_err(|error| error.to_string()),
            };
            let _ = int_tx.send(InternalEvent::NextTrackPrepared {
                play_seq,
                index,
                track_id: track.id,
                result,
            });
        });
    }

    pub(super) fn set_paused(&mut self, paused: bool) {
        if paused == self.playback.paused {
            return;
        }
        if paused {
            self.playback.position_secs = self.interpolated_position();
            self.pos_anchor = None;
        } else if !self.playback.loading_track && self.playback.current_track().is_some() {
            self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
        }
        PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Pause(paused));
        if let Some(mpv) = &self.mpv {
            mpv.set_paused(paused);
        }
    }

    pub(super) fn seek_absolute(&mut self, secs: f64) {
        let Some(PlaybackTransition::Seeked(clamped)) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Seek(secs))
        else {
            return;
        };
        if !self.playback.paused && !self.playback.loading_track {
            self.pos_anchor = Some((clamped, Instant::now()));
        }
        if let Some(mpv) = &self.mpv {
            mpv.seek_absolute(clamped);
        }
        let _ = self.mpris_tx.send(MprisUpdate::Seeked(clamped));
    }

    pub(super) fn handle_mpv_event(&mut self, ev: MpvEvent) {
        match ev {
            MpvEvent::Position(pos) => {
                if self.awaiting_playback_start {
                    return;
                }
                self.playback.position_secs = pos;
                if !self.playback.paused && !self.playback.loading_track {
                    self.pos_anchor = Some((pos, Instant::now()));
                }
            }
            MpvEvent::Duration(dur) => {
                if dur > 1.0 {
                    self.playback.duration_secs = dur;
                    let dur_u64 = dur.round() as u64;
                    let mut track_to_refetch = None;
                    if let Some(track) = self
                        .playback
                        .current_index
                        .and_then(|idx| self.playback.queue.get_mut(idx))
                    {
                        if track
                            .duration_secs
                            .is_none_or(|prev| prev.abs_diff(dur_u64) > 3)
                        {
                            track.duration_secs = Some(dur_u64);
                            track_to_refetch = Some(track.clone());
                        }
                    }
                    if let Some(lyrics_track) = track_to_refetch {
                        let lyrics_valid = matches!(
                            &self.lyrics,
                            LyricsState::Synced(lines)
                                if sonata_core::lyrics::synced_lyrics_match_duration(
                                    lines,
                                    Some(dur_u64),
                                )
                        );
                        if !lyrics_valid {
                            let play_seq = self.play_seq;
                            let lyrics_tx = self.int_tx.clone();
                            let lyrics_http = self.http.clone();
                            let lyrics_ytm = self.ytm.clone();
                            let lyrics_cache = self.cache_db.clone();
                            self.playback_tasks.spawn(async move {
                                let lyrics = lrclib::fetch_lyrics(
                                    &lyrics_http,
                                    &lyrics_ytm,
                                    &lyrics_track,
                                    lyrics_cache.as_ref(),
                                )
                                .await;
                                let _ = lyrics_tx.send(InternalEvent::LyricsReady {
                                    play_seq,
                                    track_id: lyrics_track.id,
                                    lyrics,
                                });
                            });
                        }
                    }
                }
            }
            MpvEvent::Paused(paused) => {
                if self.awaiting_playback_start {
                    if paused {
                        self.playback.paused = true;
                    }
                    return;
                }
                if paused {
                    self.playback.position_secs = self.interpolated_position();
                    self.pos_anchor = None;
                } else if !self.playback.loading_track {
                    self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                }
                self.playback.paused = paused;
            }
            MpvEvent::PlaybackStarted => {
                let was_starting = self.awaiting_playback_start;
                self.awaiting_playback_start = false;
                self.playback.paused = false;
                self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                if was_starting {
                    let telemetry = if let Some(track) = self.playback.current_track() {
                        tracing::info!(
                            event = "playing",
                            client = "desktop",
                            track_id = %track.id,
                            title = %track.title,
                            artist = %track.artist,
                            provider = ?track.source,
                            play_seq = self.play_seq,
                            quality = %self.playback.stream_badge,
                            first_audio_ms = self.play_requested_at.take().map_or(0, |at| at.elapsed().as_millis()),
                            pending_tasks = self.playback_tasks.pending_count(),
                            "playback started"
                        );
                        let duration = track.duration_secs;
                        let vid = track.id.clone();
                        self.status_line = format!("Playing: {} — {}", track.title, track.artist);
                        Some((vid, duration))
                    } else {
                        None
                    };
                    if let Some((vid, duration)) = telemetry {
                        if !vid.starts_with(sonata_core::providers::jiosaavn::ID_PREFIX) {
                            let ytm = self.ytm.clone();
                            let playlist_id = self
                                .playlist_id
                                .as_ref()
                                .filter(|_| self.playlist_tracks.iter().any(|t| t.id == vid))
                                .cloned();
                            tokio::spawn(async move {
                                if let Err(err) = ytm
                                    .report_playback(&vid, playlist_id.as_deref(), duration)
                                    .await
                                {
                                    tracing::debug!(
                                        client = "desktop",
                                        track_id = %vid,
                                        error = %err,
                                        "ytm playback reporting failed"
                                    );
                                }
                            });
                        }
                    }
                }
                self.start_hifi_upgrade();
                self.start_cover_enrichment();
            }
            MpvEvent::TrackEnded => {
                let pos = self.interpolated_position();
                self.pos_anchor = None;
                if Self::should_ignore_stale_playback_error(
                    self.playback.loading_track,
                    self.stream_loaded_at,
                    Duration::from_millis(750),
                ) {
                    tracing::debug!(
                        event = "stale_track_end",
                        client = "desktop",
                        play_seq = self.play_seq,
                        loading = self.playback.loading_track,
                        stream_loaded = self.stream_loaded_at.is_some(),
                        "ignored stale track-end event"
                    );
                    return;
                }
                if !Self::is_natural_track_end(pos, self.playback.duration_secs) {
                    let is_remote = self
                        .playback
                        .active_stream
                        .as_ref()
                        .is_some_and(|s| s.url.starts_with("http"));
                    tracing::warn!(
                        event = "premature_track_end",
                        client = "desktop",
                        track_id = self
                            .playback
                            .current_track()
                            .map(|t| t.id.as_str())
                            .unwrap_or("-"),
                        play_seq = self.play_seq,
                        position_secs = pos,
                        duration_secs = self.playback.duration_secs,
                        remote = is_remote,
                        "mpv reported premature EOF before track end; recovering at current position"
                    );
                    if is_remote && self.playback.duration_secs > 0.0 {
                        self.playback.position_secs = pos;
                        let active_url =
                            self.playback.active_stream.as_ref().map(|s| s.url.clone());
                        self.handle_mpv_event(MpvEvent::PlaybackError {
                            error: "stream interrupted while seeking".to_string(),
                            file: active_url,
                        });
                    }
                    return;
                }
                if let Some(track) = self.playback.current_track() {
                    tracing::info!(
                        event = "track_ended",
                        client = "desktop",
                        track_id = %track.id,
                        play_seq = self.play_seq,
                        position_secs = self.playback.position_secs,
                        duration_secs = self.playback.duration_secs,
                        "track ended"
                    );
                }
                if let Some(next) = self.playback.peek_next_index() {
                    self.prepare_next_before_advance(next);
                } else {
                    self.playback.paused = true;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    self.status_line = "Queue finished".to_string();
                }
            }
            MpvEvent::PlaybackError { error: err, file } => {
                tracing::warn!(
                    event = "playback_error",
                    client = "desktop",
                    track_id = self.playback.current_track().map(|track| track.id.as_str()).unwrap_or("-"),
                    play_seq = self.play_seq,
                    error = %err,
                    "mpv could not continue playback"
                );
                if let (Some(file), Some(active)) =
                    (file.as_deref(), self.playback.active_stream.as_ref())
                {
                    if active.url != file {
                        tracing::debug!(
                            event = "stale_playback_error",
                            client = "desktop",
                            play_seq = self.play_seq,
                            "ignored playback error for stale stream"
                        );
                        return;
                    }
                }
                if self.playback.loading_track {
                    tracing::debug!(
                        event = "stale_playback_error",
                        client = "desktop",
                        play_seq = self.play_seq,
                        loading = self.playback.loading_track,
                        stream_loaded = self.stream_loaded_at.is_some(),
                        "ignored playback error while loading another stream"
                    );
                    return;
                }
                if let Some(stream) = self.playback.restore_base_stream() {
                    let resume_at = self.interpolated_position();
                    tracing::warn!(
                        event = "upgrade_fallback",
                        client = "desktop",
                        track_id = self.playback.current_track().map(|track| track.id.as_str()).unwrap_or("-"),
                        play_seq = self.play_seq,
                        error = %err,
                        fallback_quality = %stream.badge,
                        resume_secs = resume_at,
                        "quality upgrade failed; restoring original stream"
                    );
                    self.playback.position_secs = resume_at;
                    self.playback.paused = true;
                    self.awaiting_playback_start = true;
                    self.pos_anchor = None;
                    self.playback.stream_badge = stream.badge.clone();
                    self.stream_loaded_at = Some(Instant::now());
                    self.status_line =
                        "Quality upgrade failed; continuing original stream".to_string();
                    if let Some(mpv) = &self.mpv {
                        mpv.load_stream_at(&stream, resume_at);
                    }
                    return;
                }
                if let Some(track) = self.playback.current_track().cloned() {
                    if Self::should_attempt_recovery(self.retried_play_seq, self.play_seq) {
                        self.retried_play_seq = self.play_seq;
                        self.upgrade_started_play_seq = None;
                        self.upgrading_quality = false;
                        let play_seq = self.play_seq;
                        self.playback_tasks.restart();
                        if let Some((_, task)) = self.next_prefetch_task.take() {
                            task.abort();
                        }
                        if let Some((_, task)) = self.adopted_prefetch_task.take() {
                            task.abort();
                        }
                        self.next_precache.take();
                        self.playback.loading_track = true;
                        self.awaiting_playback_start = false;
                        self.pos_anchor = None;
                        self.playback.stream_badge = "Retrying...".to_string();
                        self.status_line =
                            format!("Stream expired; refreshing \"{}\"...", track.title);
                        let resolver = self.resolver.clone();
                        let int_tx = self.int_tx.clone();
                        tracing::warn!(
                            event = "retrying",
                            client = "desktop",
                            track_id = %track.id,
                            title = %track.title,
                            artist = %track.artist,
                            provider = ?track.source,
                            play_seq,
                            error = %err,
                            "playback stream failed; resolving a fresh stream"
                        );
                        self.playback_tasks.spawn(async move {
                            match resolver.resolve_recovery(&track).await {
                                Ok(stream) => {
                                    let _ = int_tx.send(InternalEvent::StreamReady {
                                        play_seq,
                                        track_id: track.id,
                                        stream,
                                    });
                                }
                                Err(e) => {
                                    let _ = int_tx.send(InternalEvent::StreamError {
                                        play_seq,
                                        track_id: track.id,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        });
                        return;
                    }
                }
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.paused = true;
                self.awaiting_playback_start = false;
                self.pos_anchor = None;
                self.playback.stream_badge = "Error".to_string();
                self.status_line = format!("Playback failed: {err}");
                if let Some(mpv) = &self.mpv {
                    mpv.set_paused(true);
                }
            }
        }
    }

    pub(super) fn export_now_playing(&mut self) {
        let track = self.playback.current_track();
        let status = match track {
            None => "Stopped",
            Some(_) if self.playback.loading_track => "Loading",
            Some(_) if self.playback.paused => "Paused",
            Some(_) => "Playing",
        };
        let track_id = track.map(|t| t.id.clone()).unwrap_or_default();
        let cur_pos = self.interpolated_position();
        let pos_secs = cur_pos.max(0.0).floor() as u64;
        let dur_secs = self.playback.duration_secs.max(0.0).floor() as u64;

        let active_lyric = match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, cur_pos)
                .and_then(|idx| lines.get(idx))
                .map(|l| l.text.clone())
                .filter(|s| !s.is_empty()),
            _ => None,
        };

        let art_url = track.and_then(|t| t.thumbnail_url.clone());
        let art_path_str = self
            .art_path
            .as_ref()
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned());

        let fingerprint = ExportFingerprint {
            track_id,
            status: status.to_string(),
            lyric: active_lyric.clone(),
            art_path: art_path_str.clone(),
            pos_secs,
            dur_secs,
        };
        if self.last_exported_state.as_ref() == Some(&fingerprint) {
            return;
        }
        self.last_exported_state = Some(fingerprint);

        let payload = media::build_now_playing_json(
            status,
            track,
            &self.playback.stream_badge,
            pos_secs,
            dur_secs,
            active_lyric.as_deref(),
            art_path_str.as_deref(),
            art_url.as_deref(),
        );
        media::write_now_playing_json(&payload);
    }
}
