//! TUI playback session adapter, client-local lyrics/artwork loading, Kitty graphics, and status export.
//!
//! 1. **Session Bridge**: translates TUI transport controls into `SessionCommand`s and mirrors
//!    `SessionEvent`s and coalesced `PlaybackProgress` into local UI state.
//! 2. **Client Media Tasks**: owns TUI-local lyrics, cover art, cover enrichment, and motion cover
//!    frame decoding under a per-generation `PlaybackTaskSet`.
//! 3. **Terminal Graphics & Export**: renders Kitty inline images and writes deduplicated
//!    `$XDG_RUNTIME_DIR/sonata/now_playing.json` state.

use super::{ActiveModal, App, AppEvent, ExportFingerprint};
use base64::Engine as _;
use ratatui_image::picker::ProtocolType;
use sonata_core::lyrics::{active_line_index, lrclib, LyricsState};
use sonata_core::player::RepeatMode;
use sonata_core::providers::Track;
use sonata_runtime::media;
use sonata_runtime::player::{SessionCommand, SessionEvent};
use std::io::Write;
use std::time::Instant;

impl App {
    pub fn handle_session_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::QueueChanged {
                queue,
                current_index,
                status,
            } => {
                self.playback.queue = queue;
                self.playback.current_index = current_index;
                if let Some(msg) = status {
                    self.status_line = msg;
                }
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
                self.status_line = format!("Loading: {} — {}", track.title, track.artist);
                self.start_client_media_for_track(generation.value(), track);
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
                self.playback.loading_track = false;
                self.playback.paused = true;
                self.playback.position_secs = resume_at;
                self.playback.set_active_stream(stream.clone());
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
                    self.status_line.clear();
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
                                    client = "tui",
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
                        let lyrics_tx = self.tx.clone();
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
                            let _ = lyrics_tx.send(AppEvent::LyricsReady {
                                play_seq,
                                track_id: lyrics_track.id,
                                lyrics,
                            });
                        });
                    }
                }
            }
            SessionEvent::UpgradeStarted { .. } | SessionEvent::UpgradeUnavailable { .. } => {}
            SessionEvent::StreamUpgraded {
                generation,
                stream,
                resume_at,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.set_upgraded_stream(stream.clone());
                self.playback.position_secs = resume_at;
                self.playback.stream_badge = stream.badge;
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
                self.playback.position_secs = resume_at;
                self.playback.paused = true;
                self.playback.set_active_stream(stream.clone());
                self.playback.stream_badge = stream.badge;
                self.status_line = "Quality upgrade failed; continuing original stream".to_string();
            }
            SessionEvent::RecoveryStarted { generation, .. } => {
                if generation.value() != self.play_seq {
                    return;
                }
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = "Retrying...".to_string();
                self.status_line = "Stream expired, retrying...".to_string();
            }
            SessionEvent::TrackEnded { .. } => {}
            SessionEvent::QueueFinished { .. } => {
                if self.config.autoplay
                    && self.playback.repeat == RepeatMode::Off
                    && !self.playback.queue.is_empty()
                {
                    let play_seq = self.play_seq;
                    let seeds: Vec<String> = self
                        .playback
                        .queue
                        .iter()
                        .rev()
                        .map(|t| t.id.clone())
                        .filter(|id| {
                            !id.is_empty()
                                && !id.starts_with("local:")
                                && !id.starts_with("jiosaavn:")
                        })
                        .take(2)
                        .collect();
                    let excludes: Vec<String> =
                        self.playback.queue.iter().map(|t| t.id.clone()).collect();
                    let preferred_artists: Vec<String> = self
                        .playback
                        .queue
                        .iter()
                        .rev()
                        .take(8)
                        .map(|t| t.artist.clone())
                        .filter(|a| !a.is_empty())
                        .collect();
                    let playlist_id = self.current_playlist_id.clone();
                    let ytm = self.ytm.clone();
                    let tx = self.tx.clone();
                    self.playback.loading_track = true;
                    self.playback.paused = true;
                    self.playback.stream_badge = "Radio...".to_string();
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
                        let _ = tx.send(AppEvent::AutoplayTracksReady { play_seq, tracks });
                    });
                } else {
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                }
            }
            SessionEvent::Stopped { generation } => {
                self.play_seq = generation.value();
                self.media_tasks.restart();
                self.playback.paused = true;
                self.playback.loading_track = false;
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
                self.playback.paused = true;
                self.playback.stream_badge = badge;
                self.status_line = format!("Playback error: {error}");
            }
            SessionEvent::ModeChanged {
                shuffle,
                repeat,
                volume,
                ..
            } => {
                self.playback.shuffle = shuffle;
                self.playback.repeat = repeat;
                self.playback.volume = volume;
            }
            SessionEvent::PrecacheUpdated { .. } => {}
        }
    }

    pub fn toggle_hifi_substitution(&mut self) {
        self.config.hifi_substitution = !self.config.hifi_substitution;
        let _ = self.config.save();
        self.enqueue_session_cmd(SessionCommand::UpdatePolicy {
            policy: self.session_policy(),
            trigger_upgrade: self.config.hifi_substitution,
        });
        self.status_line = format!(
            "YouTube Music Quality Upgrade: {}",
            if self.config.hifi_substitution {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    fn start_cover_enrichment(&mut self) {
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
        let tx = self.tx.clone();
        self.media_tasks.spawn(async move {
            let Some((url, _source)) =
                media::resolve_track_cover(&http, cache_db.as_ref(), &track).await
            else {
                return;
            };
            let Some((image, png_bytes)) =
                media::fetch_and_decode_art(&http, &url, &paths, cache_limit).await
            else {
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

    fn start_client_media_for_track(&mut self, play_seq: u64, track: Track) {
        self.media_tasks.restart();
        self.clear_kitty_images();
        self.cover_started_play_seq = None;
        self.lyrics = LyricsState::Loading;
        self.lyrics_scroll_init = false;
        self.lyrics_scroll_target = 0.0;
        self.lyrics_scroll_vel = 0.0;
        self.lyrics_prev_active = None;
        self.lyrics_cur_active = None;
        self.album_art = None;
        self.kitty_art_png = None;
        self.anim_art_frames.clear();
        self.anim_art_idx = 0;
        self.last_anim_step = Instant::now();
        self.album_art_track_id = Some(track.id.clone());

        let http = self.http.clone();
        let lyrics_tx = self.tx.clone();
        let lyrics_http = http.clone();
        let lyrics_ytm = self.ytm.clone();
        let lyrics_cache = self.cache_db.clone();
        let lyrics_paths = self.paths.clone();
        let lyrics_limit = self.config.cache_limit_bytes();
        let lyrics_track = track.clone();
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
            let art_paths = self.paths.clone();
            let art_limit = self.config.cache_limit_bytes();
            let track_id = track.id.clone();
            self.media_tasks.spawn(async move {
                if let Some((img, png_bytes)) =
                    media::fetch_and_decode_art(&art_http, &url, &art_paths, art_limit).await
                {
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
            let anim_http = http;
            let anim_track = track;
            let anim_paths = self.paths.clone();
            let anim_enabled = self.config.motion_covers;
            let picker = self.picker.clone();
            let need_kitty_png = picker.protocol_type() == ProtocolType::Kitty;
            let (max_dim, target_fps, max_frames) = if need_kitty_png {
                (320, 15.0, 40)
            } else {
                (360, 15.0, 40)
            };
            self.media_tasks.spawn(async move {
                let Some(motion) = sonata_runtime::motion::resolve_motion_cover(
                    &anim_http,
                    &anim_track,
                    &anim_paths,
                    anim_enabled,
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

    pub fn play_selected_track(&mut self) {
        let Some(selected) = self.track_table_state.selected() else {
            return;
        };
        if self.visible_tracks.is_empty() || selected >= self.visible_tracks.len() {
            return;
        }
        self.enqueue_session_cmd(SessionCommand::PlayQueue {
            tracks: self.visible_tracks.clone(),
            start_index: selected,
            push_history: false,
        });
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
        self.enqueue_session_cmd(SessionCommand::Next);
    }

    pub fn play_prev_track(&mut self) {
        self.enqueue_session_cmd(SessionCommand::Previous {
            restart_if_after_secs: None,
        });
    }

    pub fn toggle_pause(&mut self) {
        self.enqueue_session_cmd(SessionCommand::TogglePause);
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
        self.enqueue_session_cmd(SessionCommand::SetVolume(next));
    }

    pub fn adjust_volume(&mut self, delta: i8) {
        let next = (self.playback.volume as i16 + delta as i16).clamp(0, 100) as u8;
        self.set_volume(next, true);
    }

    pub fn interpolated_position(&self) -> f64 {
        let progress = self.session.progress();
        if progress.generation.value() == self.play_seq && progress.generation.value() > 0 {
            progress.interpolated_position()
        } else {
            self.playback.position_secs
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.playback.shuffle = !self.playback.shuffle;
        self.config.shuffle = self.playback.shuffle;
        let _ = self.config.save();
        self.enqueue_session_cmd(SessionCommand::SetShuffle(self.playback.shuffle));
        self.status_line = format!(
            "Shuffle: {}",
            if self.playback.shuffle { "On" } else { "Off" }
        );
    }

    pub fn cycle_repeat(&mut self) {
        self.playback.repeat = self.playback.repeat.next();
        self.config.repeat_mode = self.playback.repeat;
        let _ = self.config.save();
        self.enqueue_session_cmd(SessionCommand::SetRepeat(self.playback.repeat));
        self.status_line = format!("Repeat: {}", self.playback.repeat.label());
    }

    pub fn toggle_autoplay(&mut self) {
        self.config.autoplay = !self.config.autoplay;
        let _ = self.config.save();
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = if self.config.autoplay {
            s.autoplay_on
        } else {
            s.autoplay_off
        }
        .to_string();
    }

    pub fn seek(&mut self, secs: f64) {
        self.enqueue_session_cmd(SessionCommand::SeekRelative(secs));
    }

    pub fn seek_to(&mut self, secs: f64) {
        self.enqueue_session_cmd(SessionCommand::SeekAbsolute(secs));
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
