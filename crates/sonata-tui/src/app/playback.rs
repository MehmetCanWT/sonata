//! Playback orchestration, mpv event handling, lyrics/artwork loading, and status export for the TUI.
//!
//! 1. **Transport & Queue**: play selection, next/previous transitions, pause, seek, volume, shuffle, and repeat controls.
//! 2. **Stream & Precache**: fast stream resolution, background Hi-Fi upgrades, recovery retries, and lookahead next-track precaching.
//! 3. **Mpv Events**: handles position, duration, pause, start, track-end, and playback-error notifications from `MpvController`.
//! 4. **Media & Export**: fetches lyrics and cover art, renders Kitty terminal graphics, and writes deduplicated now-playing JSON state.

use super::{ActiveModal, App, AppEvent, ExportFingerprint};
use base64::Engine as _;
use ratatui_image::picker::ProtocolType;
use sonata_core::lyrics::{active_line_index, lrclib, LyricsState};
use sonata_core::media;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator, PlaybackTransition};
use sonata_core::player::{MpvEvent, PlaybackTask};
use sonata_core::providers::hifi::{ResolvedStream, StreamResolver};
use sonata_core::providers::ProviderSource;
use std::io::Write;
use std::time::Instant;

impl App {
    pub(super) fn handle_mpv_event(&mut self, mev: MpvEvent) {
        match mev {
            MpvEvent::Position(pos) => {
                if !self.playback.loading_track && !self.awaiting_playback_start {
                    self.playback.position_secs = pos;
                    if !self.playback.paused {
                        self.pos_anchor = Some((pos, Instant::now()));
                    }
                }
            }
            MpvEvent::Duration(dur) => {
                if !self.playback.loading_track && dur > 1.0 {
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
                            let lyrics_tx = self.tx.clone();
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
                                let _ = lyrics_tx.send(AppEvent::LyricsReady {
                                    play_seq,
                                    track_id: lyrics_track.id,
                                    lyrics,
                                });
                            });
                        }
                    }
                }
            }
            MpvEvent::Paused(p) => {
                if self.awaiting_playback_start {
                    if p {
                        self.playback.paused = true;
                    }
                    return;
                }
                if !self.playback.loading_track {
                    if p {
                        self.playback.position_secs = self.interpolated_position();
                        self.pos_anchor = None;
                    } else {
                        self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                    }
                    self.playback.paused = p;
                }
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
                            client = "tui",
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
                        self.status_line.clear();
                        Some((vid, duration))
                    } else {
                        None
                    };
                    if let Some((vid, duration)) = telemetry {
                        if !vid.starts_with(sonata_core::providers::jiosaavn::ID_PREFIX) {
                            let ytm = self.ytm.clone();
                            tokio::spawn(async move {
                                if let Err(err) = ytm.report_playback(&vid, None, duration).await {
                                    tracing::debug!(
                                        client = "tui",
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
                if !self.playback.loading_track {
                    let pos = self.interpolated_position();
                    let dur = self.playback.duration_secs;
                    let is_natural_end = dur <= 0.0 || pos >= (dur - 2.5).max(0.0);
                    let is_remote = self
                        .playback
                        .active_stream
                        .as_ref()
                        .is_some_and(|s| s.url.starts_with("http"));
                    if !is_natural_end {
                        tracing::warn!(
                            event = "premature_track_end",
                            client = "tui",
                            track_id = self
                                .playback
                                .current_track()
                                .map(|t| t.id.as_str())
                                .unwrap_or("-"),
                            play_seq = self.play_seq,
                            position_secs = pos,
                            duration_secs = dur,
                            remote = is_remote,
                            "mpv reported premature EOF before track end; recovering at current position"
                        );
                        self.playback.position_secs = pos;
                        self.pos_anchor = None;
                        if is_remote {
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
                            client = "tui",
                            track_id = %track.id,
                            play_seq = self.play_seq,
                            position_secs = self.playback.position_secs,
                            duration_secs = self.playback.duration_secs,
                            "track ended"
                        );
                    }
                    if let Some(next_idx) = self.playback.peek_next_index() {
                        self.prepare_next_before_advance(next_idx);
                    } else {
                        self.playback.paused = true;
                        self.pos_anchor = None;
                    }
                }
            }
            MpvEvent::PlaybackError { error: err, file } => {
                if let (Some(file), Some(active)) =
                    (file.as_deref(), self.playback.active_stream.as_ref())
                {
                    if active.url != file {
                        tracing::debug!(
                            event = "stale_playback_error",
                            client = "tui",
                            play_seq = self.play_seq,
                            "ignored playback error for stale stream"
                        );
                        return;
                    }
                }
                if !self.playback.loading_track {
                    tracing::warn!(
                        event = "playback_error",
                        client = "tui",
                        track_id = self.playback.current_track().map(|track| track.id.as_str()).unwrap_or("-"),
                        play_seq = self.play_seq,
                        error = %err,
                        "mpv could not continue playback"
                    );
                    if let Some(stream) = self.playback.restore_base_stream() {
                        let resume_at = self.interpolated_position();
                        tracing::warn!(
                            event = "upgrade_fallback",
                            client = "tui",
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
                        self.status_line =
                            "Quality upgrade failed; continuing original stream".to_string();
                        if let Some(mpv) = &self.mpv {
                            mpv.load_stream_at(&stream, resume_at);
                        }
                        return;
                    }
                    if self.retried_play_seq != self.play_seq {
                        if let Some(track) = self.playback.current_track().cloned() {
                            self.playback_tasks.restart();
                            if let Some((_, task)) = self.next_prefetch_task.take() {
                                task.abort();
                            }
                            if let Some((_, task)) = self.adopted_prefetch_task.take() {
                                task.abort();
                            }
                            self.next_precache.take();
                            self.retried_play_seq = self.play_seq;
                            let play_seq = self.play_seq;
                            self.playback.loading_track = true;
                            self.awaiting_playback_start = false;
                            self.playback.paused = true;
                            self.pos_anchor = None;
                            self.upgrade_started_play_seq = None;
                            self.playback.stream_badge = "Retrying...".to_string();
                            self.status_line = "Stream expired, retrying...".to_string();
                            tracing::warn!(
                                event = "retrying",
                                client = "tui",
                                track_id = %track.id,
                                title = %track.title,
                                artist = %track.artist,
                                provider = ?track.source,
                                play_seq,
                                error = %err,
                                "playback stream failed; resolving a fresh stream"
                            );
                            let resolver = self.resolver.clone();
                            let tx = self.tx.clone();
                            self.playback_tasks.spawn(async move {
                                match resolver.resolve_recovery(&track).await {
                                    Ok(stream) => {
                                        let _ = tx.send(AppEvent::StreamReady {
                                            play_seq,
                                            track_id: track.id,
                                            stream,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(AppEvent::StreamError {
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
                    self.playback.paused = true;
                    self.pos_anchor = None;
                    self.status_line = format!("Playback error: {err}");
                }
            }
        }
    }

    pub fn toggle_hifi_substitution(&mut self) {
        self.config.hifi_substitution = !self.config.hifi_substitution;
        let _ = self.config.save();
        if self.config.hifi_substitution {
            self.start_hifi_upgrade();
        }
        self.status_line = format!(
            "YouTube Music Quality Upgrade: {}",
            if self.config.hifi_substitution {
                "Enabled"
            } else {
                "Disabled"
            }
        );
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
        tracing::info!(
            event = "upgrade_resolving",
            client = "tui",
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq,
            current_quality = %stream.badge,
            "resolving higher quality stream"
        );
        let resolver = self.resolver.clone();
        let tx = self.tx.clone();
        let allow_hifi = self.config.hifi_substitution;
        self.playback_tasks.spawn(async move {
            if let Some(upgraded) = resolver
                .resolve_hifi_upgrade(&track, &stream, allow_hifi)
                .await
            {
                let _ = tx.send(AppEvent::StreamUpgrade {
                    play_seq,
                    track_id: track.id,
                    stream: upgraded,
                });
            } else {
                tracing::info!(
                    event = "upgrade_unavailable",
                    client = "tui",
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
        if track.source != ProviderSource::YouTubeMusic && track.source != ProviderSource::Spotify {
            return;
        }

        let play_seq = self.play_seq;
        self.cover_started_play_seq = Some(play_seq);
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let tx = self.tx.clone();
        self.playback_tasks.spawn(async move {
            let Some((url, _source)) =
                media::resolve_track_cover(&http, cache_db.as_ref(), &track).await
            else {
                return;
            };
            let Some((image, png_bytes)) = media::fetch_and_decode_art(&http, &url).await else {
                return;
            };
            let _ = tx.send(AppEvent::ArtworkEnhanced {
                play_seq,
                track_id: track.id,
                url,
                image,
                png_bytes,
            });
        });
    }

    pub fn play_selected_track(&mut self) {
        let Some(selected) = self.track_table_state.selected() else {
            return;
        };
        if self.visible_tracks.is_empty() || selected >= self.visible_tracks.len() {
            return;
        }
        PlaybackCoordinator::apply(
            &mut self.playback,
            PlaybackCommand::QueueChanged(self.visible_tracks.clone()),
        );
        self.play_queue_index_inner(selected, false);
    }

    pub fn clear_kitty_images(&mut self) {
        if self.picker.protocol_type() == ProtocolType::Kitty {
            let mut out = std::io::stdout();
            let _ = out.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\");
            let _ = out.flush();
            self.kitty_drawn_track = None;
            self.kitty_motion_uploaded_track = None;
            self.kitty_motion_count = 0;
            self.kitty_motion_last_drawn = None;
            self.kitty_motion_buf_slot = 0;
        }
    }

    pub fn sync_kitty_artwork(&mut self) {
        if self.picker.protocol_type() != ProtocolType::Kitty {
            return;
        }
        if self.modal != ActiveModal::None {
            if self.kitty_drawn_track.is_some() || self.kitty_motion_last_drawn.is_some() {
                self.clear_kitty_images();
            }
            return;
        }
        let (Some(track_id), Some(rect)) = (self.album_art_track_id.clone(), self.last_art_rect)
        else {
            return;
        };
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        if !self.anim_art_frames.is_empty()
            && self.config.motion_covers
            && !self.config.reduce_motion
        {
            let count = self.anim_art_frames.len();
            let idx = self.anim_art_idx % count;
            if self.kitty_motion_uploaded_track.as_deref() == Some(track_id.as_str())
                && self.kitty_motion_last_drawn == Some((idx, rect))
            {
                return;
            }
            let png = &self.anim_art_frames[idx].1;
            if png.is_empty() {
                return;
            }

            let next_id: u32 = if self.kitty_motion_buf_slot == 0 {
                2
            } else {
                3
            };
            let prev_id: u32 = if next_id == 2 { 3 } else { 2 };
            let mut out = std::io::stdout();
            if self.kitty_drawn_track.is_some() {
                let _ = out.write_all(b"\x1b_Ga=d,d=i,i=1,q=2\x1b\\");
                self.kitty_drawn_track = None;
            }
            let b64 = base64::engine::general_purpose::STANDARD.encode(png);
            let chunks: Vec<&[u8]> = b64.as_bytes().chunks(4096).collect();
            let _ = write!(out, "\x1b[s\x1b[{};{}H", rect.y + 1, rect.x + 1);
            for (c_idx, chunk) in chunks.iter().enumerate() {
                let more = usize::from(c_idx + 1 < chunks.len());
                if c_idx == 0 {
                    let _ = write!(
                        out,
                        "\x1b_Ga=T,f=100,i={next_id},p=1,c={},r={},C=1,z=-1,q=2,m={more};",
                        rect.width, rect.height
                    );
                } else {
                    let _ = write!(out, "\x1b_Gm={more};");
                }
                let _ = out.write_all(chunk);
                let _ = out.write_all(b"\x1b\\");
            }
            let _ = write!(out, "\x1b[u\x1b_Ga=d,d=i,i={prev_id},q=2\x1b\\");
            let _ = out.flush();
            self.kitty_motion_buf_slot = 1 - self.kitty_motion_buf_slot;
            self.kitty_motion_uploaded_track = Some(track_id);
            self.kitty_motion_count = count;
            self.kitty_motion_last_drawn = Some((idx, rect));
            return;
        }

        let Some(png) = self.kitty_art_png.as_ref() else {
            return;
        };
        let target = (track_id, rect);
        if self.kitty_drawn_track.as_ref() == Some(&target) {
            return;
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let bytes = b64.as_bytes();
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x1b_Ga=d,d=i,i=1,q=2\x1b\\");
        let _ = write!(out, "\x1b[s\x1b[{};{}H", rect.y + 1, rect.x + 1);

        let chunks: Vec<&[u8]> = bytes.chunks(4096).collect();
        for (idx, chunk) in chunks.iter().enumerate() {
            let more = usize::from(idx + 1 < chunks.len());
            if idx == 0 {
                let _ = write!(
                    out,
                    "\x1b_Ga=T,f=100,i=1,q=2,C=1,c={},r={},m={};",
                    rect.width, rect.height, more
                );
            } else {
                let _ = write!(out, "\x1b_Gm={more};");
            }
            let _ = out.write_all(chunk);
            let _ = out.write_all(b"\x1b\\");
        }
        let _ = out.write_all(b"\x1b[u");
        let _ = out.flush();
        self.kitty_drawn_track = Some(target);
    }

    pub fn play_next_track(&mut self) {
        if let Some(transition) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Next)
        {
            self.start_play_transition(transition, None);
        } else {
            self.playback.paused = true;
            self.pos_anchor = None;
        }
    }

    pub fn play_prev_track(&mut self) {
        if let Some(transition) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Previous)
        {
            self.start_play_transition(transition, None);
        }
    }

    fn play_queue_index_inner(&mut self, index: usize, push_history: bool) {
        self.play_queue_index_with_stream(index, push_history, None);
    }

    pub(super) fn play_queue_index_with_stream(
        &mut self,
        index: usize,
        push_history: bool,
        prepared_stream: Option<ResolvedStream>,
    ) {
        let Some(transition) = PlaybackCoordinator::apply(
            &mut self.playback,
            PlaybackCommand::Play {
                index,
                push_history,
            },
        ) else {
            return;
        };
        self.start_play_transition(transition, prepared_stream);
    }

    fn start_play_transition(
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
        self.clear_kitty_images();
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
            client = "tui",
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
        self.upgrade_started_play_seq = None;
        self.cover_started_play_seq = None;
        self.pos_anchor = None;
        self.lyrics = LyricsState::Loading;
        self.album_art = None;
        self.kitty_art_png = None;
        self.anim_art_frames.clear();
        self.anim_art_idx = 0;
        self.last_anim_step = Instant::now();
        self.album_art_track_id = Some(track.id.clone());
        self.status_line = format!("Loading: {} — {}", track.title, track.artist);

        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        let track_clone = track.clone();

        if let Some(stream) = prepared_stream {
            let _ = tx.send(AppEvent::StreamReady {
                play_seq,
                track_id: track_clone.id.clone(),
                stream,
            });
        } else {
            self.playback_tasks.spawn(async move {
                match resolver.resolve_fast(&track_clone).await {
                    Ok(stream) => {
                        let _ = tx.send(AppEvent::StreamReady {
                            play_seq,
                            track_id: track_clone.id.clone(),
                            stream,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::StreamError {
                            play_seq,
                            track_id: track_clone.id.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            });
        }

        let lyrics_tx = self.tx.clone();
        let lyrics_http = http.clone();
        let lyrics_ytm = self.ytm.clone();
        let lyrics_cache = self.cache_db.clone();
        let lyrics_track = track.clone();
        self.playback_tasks.spawn(async move {
            let lyrics = lrclib::fetch_lyrics(
                &lyrics_http,
                &lyrics_ytm,
                &lyrics_track,
                lyrics_cache.as_ref(),
            )
            .await;
            let _ = lyrics_tx.send(AppEvent::LyricsReady {
                play_seq,
                track_id: lyrics_track.id,
                lyrics,
            });
        });

        if let Some(url) = track.thumbnail_url.clone() {
            let url = media::upscale_artwork_url(&url);
            let art_tx = self.tx.clone();
            let art_http = http.clone();
            let track_id = track.id.clone();
            self.playback_tasks.spawn(async move {
                if let Some((img, png_bytes)) = media::fetch_and_decode_art(&art_http, &url).await {
                    let _ = art_tx.send(AppEvent::ArtworkReady {
                        play_seq,
                        track_id,
                        url,
                        image: img,
                        png_bytes,
                    });
                }
            });
        }
        self.start_cover_enrichment();

        if self.config.motion_covers && !self.config.reduce_motion {
            let anim_tx = self.tx.clone();
            let anim_http = http.clone();
            let anim_track = track.clone();
            let picker = self.picker.clone();
            let need_kitty_png = picker.protocol_type() == ProtocolType::Kitty;
            let (max_dim, target_fps, max_frames) = if need_kitty_png {
                (320, 15.0, 40)
            } else {
                (360, 15.0, 40)
            };
            self.playback_tasks.spawn(async move {
                let Some(motion) = sonata_core::motion::resolve_motion_cover(
                    &anim_http,
                    &anim_track,
                    max_dim,
                    target_fps,
                    max_frames,
                )
                .await
                else {
                    return;
                };
                let fps = motion.fps;
                let frames = tokio::task::spawn_blocking(move || {
                    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
                    use image::ImageEncoder as _;

                    let mut decoded = Vec::with_capacity(motion.frames_rgba.len());
                    for rgba in motion.frames_rgba {
                        if need_kitty_png {
                            let mut buf = Vec::new();
                            let encoder = PngEncoder::new_with_quality(
                                &mut buf,
                                CompressionType::Fast,
                                FilterType::Sub,
                            );
                            if encoder
                                .write_image(
                                    &rgba,
                                    motion.width,
                                    motion.height,
                                    image::ExtendedColorType::Rgba8,
                                )
                                .is_err()
                            {
                                continue;
                            }
                            decoded.push((None, buf));
                        } else {
                            let Some(img_buf) =
                                image::RgbaImage::from_raw(motion.width, motion.height, rgba)
                            else {
                                continue;
                            };
                            let dyn_img = image::DynamicImage::ImageRgba8(img_buf);
                            decoded.push((Some(picker.new_resize_protocol(dyn_img)), Vec::new()));
                        }
                    }
                    decoded
                })
                .await
                .unwrap_or_default();
                if !frames.is_empty() {
                    let _ = anim_tx.send(AppEvent::AnimatedArtworkReady {
                        play_seq,
                        track_id: anim_track.id,
                        fps,
                        frames,
                    });
                }
            });
        }
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
                        client = "tui",
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
            client = "tui",
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
                client = "tui",
                track_id = %track_id,
                reason = "quality-unavailable",
                "adopted next-track precache did not produce an upgrade"
            );
            return;
        };
        tracing::info!(
            event = "next_precache_adopted",
            client = "tui",
            track_id = %track_id,
            quality = %stream.badge,
            "applying completed precache to current track"
        );
        let _ = self.tx.send(AppEvent::StreamUpgrade {
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
        let tx = self.tx.clone();
        self.playback_tasks.spawn(async move {
            let result = match prepared {
                Some(stream) => Ok(stream),
                None => resolver
                    .resolve_fast(&track)
                    .await
                    .map_err(|error| error.to_string()),
            };
            let _ = tx.send(AppEvent::NextTrackPrepared {
                play_seq,
                index,
                track_id: track.id,
                result,
            });
        });
    }

    pub fn toggle_pause(&mut self) {
        let new_paused = !self.playback.paused;
        if new_paused {
            self.playback.position_secs = self.interpolated_position();
            self.pos_anchor = None;
        } else if !self.playback.loading_track {
            self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
        }
        PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Pause(new_paused));
        if let Some(mpv) = &self.mpv {
            mpv.set_paused(new_paused);
        }
    }

    pub fn set_volume(&mut self, vol: u8, save_now: bool) {
        let next = vol.min(100);
        if self.playback.volume == next {
            return;
        }
        self.playback.volume = next;
        self.config.volume = next;
        if save_now {
            self.config_dirty = false;
            let _ = self.config.save();
        } else {
            self.config_dirty = true;
        }
        if let Some(mpv) = &self.mpv {
            mpv.set_volume(next);
        }
    }

    pub fn adjust_volume(&mut self, delta: i8) {
        let next = (self.playback.volume as i16 + delta as i16).clamp(0, 100) as u8;
        self.set_volume(next, true);
    }

    pub fn interpolated_position(&self) -> f64 {
        let raw = match self.pos_anchor {
            Some((base, at)) if !self.playback.paused && !self.playback.loading_track => {
                base + at.elapsed().as_secs_f64()
            }
            _ => self.playback.position_secs,
        };
        if self.playback.duration_secs > 0.0 {
            raw.clamp(0.0, self.playback.duration_secs)
        } else {
            raw.max(0.0)
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.playback.shuffle = !self.playback.shuffle;
        self.config.shuffle = self.playback.shuffle;
        let _ = self.config.save();
        self.playback.clear_upcoming();
        self.status_line = format!(
            "Shuffle: {}",
            if self.playback.shuffle { "On" } else { "Off" }
        );
    }

    pub fn cycle_repeat(&mut self) {
        self.playback.repeat = self.playback.repeat.next();
        self.config.repeat_mode = self.playback.repeat;
        let _ = self.config.save();
        self.playback.clear_upcoming();
        self.status_line = format!("Repeat: {}", self.playback.repeat.label());
    }

    pub fn seek(&mut self, secs: f64) {
        let target = (self.interpolated_position() + secs).max(0.0);
        let Some(PlaybackTransition::Seeked(target)) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Seek(target))
        else {
            return;
        };
        if !self.playback.paused {
            self.pos_anchor = Some((target, Instant::now()));
        }
        if let Some(mpv) = &self.mpv {
            mpv.seek_relative(secs);
        }
    }

    pub fn seek_to(&mut self, secs: f64) {
        let Some(PlaybackTransition::Seeked(target)) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Seek(secs))
        else {
            return;
        };
        if !self.playback.paused {
            self.pos_anchor = Some((target, Instant::now()));
        }
        if let Some(mpv) = &self.mpv {
            mpv.seek_absolute(target);
        }
    }

    pub fn export_now_playing(&mut self) {
        let track = self.playback.current_track();
        let status = match track {
            None => "Stopped",
            Some(_) if self.playback.loading_track => "Loading",
            Some(_) if self.playback.paused => "Paused",
            Some(_) => "Playing",
        };
        let track_id = track.map(|t| t.id.as_str()).unwrap_or("");
        let cur_pos = self.interpolated_position();
        let pos_secs = cur_pos.max(0.0).floor() as u64;
        let dur_secs = self.playback.duration_secs.max(0.0).floor() as u64;

        let active_lyric = match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, cur_pos)
                .and_then(|idx| lines.get(idx))
                .map(|l| l.text.as_str())
                .filter(|s| !s.is_empty()),
            _ => None,
        };

        if let Some(prev) = self.last_exported_state.as_ref() {
            if prev.track_id == track_id
                && prev.status == status
                && prev.pos_secs == pos_secs
                && prev.dur_secs == dur_secs
                && prev.lyric.as_deref() == active_lyric
                && (prev.art_path.is_some() || self.album_art_track_id.as_deref() != Some(track_id))
            {
                return;
            }
        }

        let cached_art_path = self.last_exported_state.as_ref().and_then(|prev| {
            if prev.track_id == track_id {
                prev.art_path.clone()
            } else {
                None
            }
        });

        let art_url = track.and_then(|t| t.thumbnail_url.as_deref());
        let art_path = cached_art_path.or_else(|| {
            art_url
                .map(media::art_cache_path)
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().into_owned())
        });

        let active_lyric_owned = active_lyric.map(str::to_string);
        let fingerprint = ExportFingerprint {
            track_id: track_id.to_string(),
            status: status.to_string(),
            lyric: active_lyric_owned.clone(),
            art_path: art_path.clone(),
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
            active_lyric,
            art_path.as_deref(),
            art_url,
        );
        media::write_now_playing_json(&payload);
    }

    pub fn clear_now_playing(&self) {
        media::clear_now_playing();
    }
}
