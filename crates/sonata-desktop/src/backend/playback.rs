//! Desktop playback session adapter, client-local lyrics/artwork/motion loading, and status export.
//!
//! 1. **Session Bridge**: maps `SessionEvent`s and coalesced `PlaybackProgress` into desktop view state
//!    and forwards seek updates to MPRIS2.
//! 2. **Client Media Tasks**: owns desktop-local lyrics, artwork, cover enrichment, and GPUI motion
//!    covers under a per-generation `PlaybackTaskSet`.
//! 3. **Now Playing Export**: writes deduplicated `$XDG_RUNTIME_DIR/sonata/now_playing.json` state.

use super::types::{ExportFingerprint, InternalEvent};
use super::BackendEngine;
use crate::integrations::mpris::MprisUpdate;
use sonata_core::lyrics::{active_line_index, lrclib, LyricsState};
use sonata_core::player::RepeatMode;
use sonata_core::providers::Track;
use sonata_runtime::media;
use sonata_runtime::motion;
use sonata_runtime::player::SessionEvent;
use std::sync::Arc;

impl BackendEngine {
    pub(super) fn handle_session_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::QueueChanged {
                mut queue,
                current_index,
                status,
            } => {
                for track in &mut queue {
                    if self.liked_track_ids.contains(&track.id) {
                        track.liked = true;
                    }
                }
                self.playback.queue = queue;
                self.playback.current_index = current_index;
                if let Some(msg) = status {
                    self.status_line = msg;
                }
                self.maybe_prefetch_endless_autoplay();
            }
            SessionEvent::TrackTransition {
                generation,
                index,
                track,
                ..
            } => {
                self.play_seq = generation.value();
                self.playback.current_index = Some(index);
                self.playback.position_secs = 0.0;
                self.playback.duration_secs = track.duration_secs.unwrap_or(0) as f64;
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = "Resolving...".to_string();
                self.upgrading_quality = false;
                self.status_line = format!("Loading: {} — {}", track.title, track.artist);
                self.record_played_track(track.clone());
                self.start_client_media_for_track(generation.value(), track);
                self.maybe_prefetch_endless_autoplay();
            }
            SessionEvent::PreparingNextTrack { track, .. } => {
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = "Preparing next track...".to_string();
                self.status_line = format!("Caching next track: {}", track.title);
            }
            SessionEvent::StreamLoaded {
                generation,
                track_id,
                stream,
                resume_at,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.set_active_stream(stream.clone());
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.paused = true;
                self.playback.position_secs = resume_at;
                self.playback.stream_badge = stream.badge;
                if let Some(track) = self.playback.current_track().filter(|t| t.id == track_id) {
                    self.status_line = format!("Starting: {} — {}", track.title, track.artist);
                }
            }
            SessionEvent::PlaybackStarted {
                generation,
                track,
                badge,
                was_initial_start,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.loading_track = false;
                self.playback.paused = false;
                self.playback.stream_badge = badge;
                if was_initial_start {
                    self.status_line = format!("Playing: {} — {}", track.title, track.artist);
                    if !track
                        .id
                        .starts_with(sonata_core::providers::jiosaavn::ID_PREFIX)
                    {
                        let ytm = self.ytm.clone();
                        let vid = track.id.clone();
                        let duration = track.duration_secs;
                        tokio::spawn(async move {
                            if let Err(err) = ytm.report_playback(&vid, None, duration).await {
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
                self.start_cover_enrichment();
            }
            SessionEvent::PausedChanged {
                paused,
                position_secs,
                ..
            } => {
                self.playback.paused = paused;
                self.playback.position_secs = position_secs;
            }
            SessionEvent::Seeked { position_secs, .. } => {
                self.playback.position_secs = position_secs;
                let _ = self.mpris_tx.send(MprisUpdate::Seeked(position_secs));
            }
            SessionEvent::DurationChanged {
                generation,
                duration_secs,
                updated_track,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.duration_secs = duration_secs;
                let dur_u64 = duration_secs.round() as u64;
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
                    }
                }
                if let Some(lyrics_track) = updated_track {
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
                        let lyrics_paths = self.paths.clone();
                        let lyrics_limit = self.config.cache_limit_bytes();
                        self.media_tasks.spawn(async move {
                            let lyrics = lrclib::fetch_lyrics(
                                &lyrics_http,
                                &lyrics_ytm,
                                &lyrics_track,
                                lyrics_cache.as_ref(),
                                &lyrics_paths,
                                lyrics_limit,
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
            SessionEvent::UpgradeStarted { generation, .. } => {
                if generation.value() == self.play_seq {
                    self.upgrading_quality = true;
                }
            }
            SessionEvent::StreamUpgraded {
                generation,
                stream,
                resume_at,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.upgrading_quality = false;
                self.playback.set_upgraded_stream(stream.clone());
                self.playback.position_secs = resume_at;
                self.playback.stream_badge = stream.badge.clone();
                self.status_line = format!("Upgraded audio quality: {}", stream.badge);
            }
            SessionEvent::UpgradeUnavailable { generation, .. } => {
                if generation.value() == self.play_seq {
                    self.upgrading_quality = false;
                }
            }
            SessionEvent::UpgradeFallback {
                generation,
                stream,
                resume_at,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.upgrading_quality = false;
                self.playback.position_secs = resume_at;
                self.playback.paused = true;
                self.playback.set_active_stream(stream.clone());
                self.playback.stream_badge = stream.badge;
                self.status_line = "Quality upgrade failed; continuing original stream".to_string();
            }
            SessionEvent::RecoveryStarted {
                generation, track, ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.upgrading_quality = false;
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = "Retrying...".to_string();
                self.status_line = format!("Stream expired; refreshing \"{}\"...", track.title);
            }
            SessionEvent::TrackEnded { .. } => {}
            SessionEvent::QueueFinished { .. } => {
                if self.config.autoplay
                    && self.playback.repeat == RepeatMode::Off
                    && !self.playback.queue.is_empty()
                {
                    let play_seq = self.play_seq;
                    if !self.prefetched_autoplay_tracks.is_empty() {
                        let ready = std::mem::take(&mut self.prefetched_autoplay_tracks);
                        self.prefetched_autoplay_seed = None;
                        let _ = self.int_tx.send(InternalEvent::AutoplayTracksReady {
                            play_seq,
                            tracks: ready,
                        });
                        return;
                    }
                    let seeds = self.endless_autoplay_seeds();
                    let excludes = self.endless_autoplay_excludes();
                    let preferred_artists = self.listener_preferred_artists();
                    let playlist_id = self.active_queue_playlist_id.clone();
                    let ytm = self.ytm.clone();
                    let int_tx = self.int_tx.clone();
                    self.playback.loading_track = true;
                    self.playback.paused = true;
                    self.playback.stream_badge = "Radio...".to_string();
                    self.status_line = "Finding similar tracks...".to_string();
                    self.media_tasks.spawn(async move {
                        let tracks = ytm
                            .fetch_endless_radio_queue(
                                &seeds,
                                playlist_id.as_deref(),
                                &excludes,
                                &preferred_artists,
                            )
                            .await
                            .unwrap_or_default();
                        let _ =
                            int_tx.send(InternalEvent::AutoplayTracksReady { play_seq, tracks });
                    });
                } else {
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    self.status_line = "Queue finished".to_string();
                }
            }
            SessionEvent::Stopped { generation } => {
                self.play_seq = generation.value();
                self.media_tasks.restart();
                self.playback.paused = true;
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.position_secs = 0.0;
                self.playback.stream_badge = "Stopped".to_string();
            }
            SessionEvent::PlaybackError {
                generation,
                error,
                badge,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.paused = true;
                self.playback.stream_badge = badge;
                self.status_line = format!("Playback failed: {error}");
            }
            SessionEvent::ModeChanged {
                shuffle,
                repeat,
                volume,
                muted,
            } => {
                self.playback.shuffle = shuffle;
                self.playback.repeat = repeat;
                self.playback.volume = volume;
                self.muted = muted;
            }
            SessionEvent::PrecacheUpdated { .. } => {}
        }
    }

    pub(super) fn listener_preferred_artists(&self) -> Vec<String> {
        let mut weights: std::collections::HashMap<String, (String, u32)> =
            std::collections::HashMap::new();
        let mut bump = |raw: &str, pts: u32| {
            let trimmed = raw.split(['•', ',', ';']).next().unwrap_or(raw).trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown artist") {
                return;
            }
            let key = trimmed.to_ascii_lowercase();
            let entry = weights
                .entry(key)
                .or_insert_with(|| (trimmed.to_string(), 0));
            entry.1 = entry.1.saturating_add(pts);
        };
        for (i, t) in self.playback.queue.iter().rev().take(8).enumerate() {
            bump(&t.artist, 12u32.saturating_sub(i as u32));
        }
        for (i, t) in self.last_played_tracks.iter().take(20).enumerate() {
            bump(&t.artist, 10u32.saturating_sub((i / 2) as u32).max(2));
        }
        for (i, t) in self.history_tracks.iter().take(30).enumerate() {
            bump(&t.artist, 6u32.saturating_sub((i / 6) as u32).max(1));
        }
        let mut ranked: Vec<(String, u32)> = weights.into_values().collect();
        ranked.sort_by_key(|a| std::cmp::Reverse(a.1));
        ranked.into_iter().take(12).map(|(name, _)| name).collect()
    }

    pub(super) fn endless_autoplay_seeds(&self) -> Vec<String> {
        self.playback
            .queue
            .iter()
            .rev()
            .map(|t| t.id.clone())
            .filter(|id| {
                !id.is_empty() && !id.starts_with("local:") && !id.starts_with("jiosaavn:")
            })
            .take(3)
            .collect()
    }

    pub(super) fn endless_autoplay_excludes(&self) -> Vec<String> {
        self.playback
            .queue
            .iter()
            .map(|t| t.id.clone())
            .chain(
                self.last_played_tracks
                    .iter()
                    .take(40)
                    .map(|t| t.id.clone()),
            )
            .collect()
    }

    pub(super) fn maybe_prefetch_endless_autoplay(&mut self) {
        if !self.config.autoplay
            || self.playback.repeat != RepeatMode::Off
            || self.playback.queue.is_empty()
        {
            return;
        }
        let Some(cur_idx) = self.playback.current_index else {
            return;
        };
        if cur_idx + 2 < self.playback.queue.len() {
            return;
        }
        let seeds = self.endless_autoplay_seeds();
        if seeds.is_empty() {
            return;
        }
        let seed_key = format!(
            "{}|{}",
            seeds.join(","),
            self.active_queue_playlist_id.as_deref().unwrap_or("")
        );
        if self.prefetched_autoplay_seed.as_deref() == Some(seed_key.as_str()) {
            return;
        }
        self.prefetched_autoplay_seed = Some(seed_key.clone());
        self.prefetched_autoplay_tracks.clear();
        let excludes = self.endless_autoplay_excludes();
        let preferred_artists = self.listener_preferred_artists();
        let playlist_id = self.active_queue_playlist_id.clone();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let tracks = ytm
                .fetch_endless_radio_queue(
                    &seeds,
                    playlist_id.as_deref(),
                    &excludes,
                    &preferred_artists,
                )
                .await
                .unwrap_or_default();
            let _ = int_tx.send(InternalEvent::AutoplayPrefetched { seed_key, tracks });
        });
    }

    pub(super) fn start_cover_enrichment(&mut self) {
        if !self.window_visible {
            self.deferred_media_play_seq = Some(self.play_seq);
            return;
        }
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
        let paths = self.paths.clone();
        let cache_limit = self.config.cache_limit_bytes();
        let int_tx = self.int_tx.clone();
        let anim_http = self.http.clone();
        let anim_paths = self.paths.clone();
        let anim_enabled = self.config.motion_covers && !self.ui_compact;
        let anim_tx = self.int_tx.clone();
        let anim_track = track.clone();
        self.media_tasks.spawn(async move {
            let Some((url, _source)) =
                media::resolve_track_cover(&http, cache_db.as_ref(), &track).await
            else {
                return;
            };
            let Some((image, _)) =
                media::fetch_and_decode_art(&http, &url, &paths, cache_limit).await
            else {
                return;
            };
            let ambient_rgb = media::extract_ambient_rgb(&image);
            let path = media::art_cache_path_in(&paths, &url);
            let _ = int_tx.send(InternalEvent::ArtworkEnhanced {
                play_seq,
                track_id: track.id,
                url,
                art_path: path.exists().then_some(path),
                ambient_rgb,
            });
        });
        if anim_enabled {
            self.media_tasks.spawn(async move {
                if let Some(cover) = motion::resolve_motion_cover(
                    &anim_http,
                    &anim_track,
                    &anim_paths,
                    anim_enabled,
                    360,
                    15.0,
                    45,
                )
                .await
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

    /// Catches up artwork, lyrics, and motion cover when the window wakes from tray/taskbar or mini mode.
    pub(super) fn resume_deferred_media(&mut self) {
        if !self.window_visible {
            return;
        }
        let Some(track) = self.playback.current_track().cloned() else {
            return;
        };
        let play_seq = self.play_seq;
        let needs_media = self.deferred_media_play_seq == Some(play_seq)
            || (!self.ui_compact && matches!(self.lyrics, LyricsState::Idle));
        if needs_media {
            self.deferred_media_play_seq = None;
            self.start_client_media_for_track(play_seq, track);
            if !self.playback.loading_track {
                self.start_cover_enrichment();
            }
        } else if !self.ui_compact && self.config.motion_covers && self.motion_cover.is_none() {
            self.cover_started_play_seq = None;
            self.start_cover_enrichment();
        }
    }

    fn start_client_media_for_track(&mut self, play_seq: u64, track: Track) {
        self.media_tasks.restart();
        self.cover_started_play_seq = None;
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

        // While hidden/minimized, skip lyrics & artwork network/decode churn; keep audio only.
        if !self.window_visible {
            self.lyrics = LyricsState::Idle;
            self.deferred_media_play_seq = Some(play_seq);
            return;
        }

        let http = self.http.clone();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();

        if !self.ui_compact {
            self.lyrics = LyricsState::Loading;
            let lyrics_track = track.clone();
            let lyrics_http = http.clone();
            let lyrics_cache = self.cache_db.clone();
            let lyrics_paths = self.paths.clone();
            let lyrics_limit = self.config.cache_limit_bytes();
            let lyrics_tx = int_tx.clone();
            self.media_tasks.spawn(async move {
                let lyrics = lrclib::fetch_lyrics(
                    &lyrics_http,
                    &ytm,
                    &lyrics_track,
                    lyrics_cache.as_ref(),
                    &lyrics_paths,
                    lyrics_limit,
                )
                .await;
                let _ = lyrics_tx.send(InternalEvent::LyricsReady {
                    play_seq,
                    track_id: lyrics_track.id,
                    lyrics,
                });
            });
        } else {
            self.lyrics = LyricsState::Idle;
            self.deferred_media_play_seq = Some(play_seq);
        }

        if let Some(url) = track
            .thumbnail_url
            .as_deref()
            .map(media::upscale_artwork_url)
        {
            let art_tx = int_tx;
            let art_paths = self.paths.clone();
            let art_limit = self.config.cache_limit_bytes();
            let track_id = track.id.clone();
            self.media_tasks.spawn(async move {
                if let Some((img, _)) =
                    media::fetch_and_decode_art(&http, &url, &art_paths, art_limit).await
                {
                    let ambient = media::extract_ambient_rgb(&img);
                    let path = media::art_cache_path_in(&art_paths, &url);
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

    pub(super) fn interpolated_position(&self) -> f64 {
        let progress = self.session.progress();
        if progress.generation.value() == self.play_seq && progress.generation.value() > 0 {
            progress.interpolated_position()
        } else {
            self.playback.position_secs
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
        if (next - self.playback.position_secs).abs() >= 0.015 {
            self.playback.position_secs = next;
            true
        } else {
            false
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
