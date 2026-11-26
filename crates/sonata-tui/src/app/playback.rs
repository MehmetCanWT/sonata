//! TUI playback session adapter, client-local lyrics/artwork loading, Kitty graphics, and status export.
//!
//! 1. **Session Bridge**: translates TUI transport controls into `SessionCommand`s and mirrors
//!    `SessionEvent`s and coalesced `PlaybackProgress` into local UI state.
//! 2. **Client Media Tasks**: owns TUI-local lyrics, cover art, cover enrichment, and motion cover
//!    frame decoding under a per-generation `PlaybackTaskSet`.
//! 3. **Terminal Graphics & Export**: renders Kitty inline images and writes deduplicated
//!    `$XDG_RUNTIME_DIR/sonata/now_playing.json` state.

use super::{tui_track_source_tag, ActiveModal, App, AppEvent, ExportFingerprint};
use base64::Engine as _;
use ratatui_image::picker::ProtocolType;
use sonata_core::lyrics::{lrclib, romanize_lyrics_state, translate_lyrics_state, LyricsState};
use sonata_core::player::RepeatMode;
use sonata_core::providers::Track;
use sonata_core::redaction::sensitive_text;
use sonata_core::scrobble::{should_scrobble, ScrobbleTrack, ScrobbleTracker};
use sonata_runtime::config::{sanitize_crossfade_secs, AppConfig, MAX_CROSSFADE_SECS};
use sonata_runtime::media;
use sonata_runtime::player::{SessionCommand, SessionEvent};
use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PLAYBACK_SPEED_PRESETS: &[f64] = &[0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
pub const SLEEP_TIMER_PRESET_SECS: &[Option<u64>] = &[
    None,
    Some(15 * 60),
    Some(30 * 60),
    Some(45 * 60),
    Some(60 * 60),
];

/// Preserves source context tags (e.g., `Cached`, `Offline`, `JioSaavn`, `Jellyfin`) when
/// replacing an initial stream badge with live measured `mpv` codec/bitrate/bit-depth telemetry.
pub fn merge_measured_audio_badge(existing_badge: &str, measured_badge: &str) -> String {
    const SOURCE_TAGS: &[&str] = &[
        "Cached", "Offline", "JioSaavn", "Jellyfin", "Subsonic", "WebDAV", "Video",
    ];
    let measured_trimmed = measured_badge.trim();
    if measured_trimmed.is_empty() {
        return existing_badge.to_string();
    }
    let mut parts: Vec<String> = vec![measured_trimmed.to_string()];
    for token in existing_badge.split(" · ").map(str::trim) {
        if SOURCE_TAGS
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(token))
            && !parts
                .iter()
                .any(|p| p.to_ascii_lowercase().contains(&token.to_ascii_lowercase()))
        {
            parts.push(token.to_string());
        }
    }
    parts.join(" · ")
}

/// Steps through the discrete playback speed presets (`0.75x..=2.0x`).
pub fn step_speed_preset(current: f64, forward: bool) -> f64 {
    let idx = PLAYBACK_SPEED_PRESETS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((*a - current).abs())
                .partial_cmp(&((*b - current).abs()))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(1);
    let next = if forward {
        (idx + 1) % PLAYBACK_SPEED_PRESETS.len()
    } else {
        (idx + PLAYBACK_SPEED_PRESETS.len() - 1) % PLAYBACK_SPEED_PRESETS.len()
    };
    PLAYBACK_SPEED_PRESETS[next]
}

/// Cycles through `Off`, `15m`, `30m`, `45m`, and `60m` sleep timer presets.
pub fn next_sleep_preset(current_remaining_secs: Option<u64>, forward: bool) -> Option<u64> {
    let idx = match current_remaining_secs {
        None | Some(0) => 0,
        Some(rem) => SLEEP_TIMER_PRESET_SECS
            .iter()
            .enumerate()
            .skip(1)
            .min_by_key(|(_, preset)| preset.unwrap_or(0).abs_diff(rem))
            .map(|(i, _)| i)
            .unwrap_or(1),
    };
    let next = if forward {
        (idx + 1) % SLEEP_TIMER_PRESET_SECS.len()
    } else {
        (idx + SLEEP_TIMER_PRESET_SECS.len() - 1) % SLEEP_TIMER_PRESET_SECS.len()
    };
    SLEEP_TIMER_PRESET_SECS[next]
}

/// Formats remaining sleep-timer seconds as `MM:SS` (or `H:MM:SS` if >= 1 hour).
pub fn format_sleep_countdown(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem_secs = secs % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{rem_secs:02}")
    } else {
        format!("{mins:02}:{rem_secs:02}")
    }
}

impl App {
    fn now_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn start_listen_tracking(&mut self, play_seq: u64, track: &Track) {
        let now_unix = Self::now_unix_secs();
        let scrobble_track = ScrobbleTrack::from_track(track);
        self.scrobble_tracker = Some(ScrobbleTracker::start(
            track.id.clone(),
            scrobble_track.clone(),
            now_unix,
        ));
        self.active_listen_session = Some(super::TuiListenSession {
            play_seq,
            track: track.clone(),
            started_at_unix: now_unix,
            accumulated_secs: 0.0,
            last_tick: Some(Instant::now()),
            stats_recorded: false,
        });
        if self.scrobble_config.any_configured()
            && !scrobble_track.title.trim().is_empty()
            && !scrobble_track.artist.trim().is_empty()
        {
            let client = self.scrobble_client.clone();
            let cfg = self.scrobble_config.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                match client.update_now_playing(&cfg, &scrobble_track).await {
                    Ok(()) => {
                        let _ = tx.send(AppEvent::ScrobbleError(None));
                    }
                    Err(err) => {
                        let safe = sensitive_text(&err.to_string());
                        tracing::debug!(client = "tui", error = %safe, "scrobble now-playing failed");
                        let _ = tx.send(AppEvent::ScrobbleError(Some(safe)));
                    }
                }
            });
        }
    }

    pub fn pause_listen_tracking(&mut self) {
        if let Some(ref mut sess) = self.active_listen_session {
            if let Some(prev) = sess.last_tick.take() {
                let delta = prev.elapsed().as_secs_f64().clamp(0.0, 2.0);
                sess.accumulated_secs += delta;
            }
        }
    }

    pub fn resume_listen_tracking(&mut self) {
        if let Some(ref mut sess) = self.active_listen_session {
            if sess.last_tick.is_none() {
                sess.last_tick = Some(Instant::now());
            }
        }
    }

    pub fn advance_listen_tracking(&mut self) {
        if self.playback.paused || self.playback.loading_track {
            return;
        }
        let delta = match self.active_listen_session.as_mut() {
            Some(sess) if sess.play_seq == self.play_seq => {
                let now = Instant::now();
                let delta = sess
                    .last_tick
                    .map(|prev| {
                        now.saturating_duration_since(prev)
                            .as_secs_f64()
                            .clamp(0.0, 2.0)
                    })
                    .unwrap_or(0.0);
                sess.last_tick = Some(now);
                sess.accumulated_secs += delta;
                delta
            }
            _ => return,
        };
        if let Some(tracker) = self.scrobble_tracker.as_mut() {
            if let Some((strack, started_at)) = tracker.advance(delta) {
                if self.scrobble_config.any_configured() {
                    let client = self.scrobble_client.clone();
                    let cfg = self.scrobble_config.clone();
                    let track_owned = strack.clone();
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        match client.scrobble(&cfg, &track_owned, Some(started_at)).await {
                            Ok(()) => {
                                let _ = tx.send(AppEvent::ScrobbleError(None));
                            }
                            Err(err) => {
                                let safe = sensitive_text(&err.to_string());
                                tracing::debug!(client = "tui", error = %safe, "scrobble submit failed");
                                let _ = tx.send(AppEvent::ScrobbleError(Some(safe)));
                            }
                        }
                    });
                }
            }
        }
    }

    pub fn finalize_listen_stats(&mut self) {
        self.pause_listen_tracking();
        let Some(ref mut sess) = self.active_listen_session else {
            return;
        };
        if sess.stats_recorded {
            return;
        }
        let played_secs = sess.accumulated_secs.round() as u64;
        let duration_secs = sess.track.duration_secs.unwrap_or(0);
        if !should_scrobble(sess.track.duration_secs, played_secs) {
            return;
        }
        sess.stats_recorded = true;
        let ytm_watchtime_track = (!self.config.personal_only_mode
            && self.ytm.is_authenticated()
            && !sess.track.id.is_empty()
            && !sess.track.id.starts_with("local:")
            && !sess.track.id.starts_with("jiosaavn:")
            && sonata_core::providers::servers::parse_server_item_id(&sess.track.id).is_none())
        .then(|| {
            (
                self.ytm.clone(),
                sess.track.id.clone(),
                sess.track.duration_secs,
            )
        });
        if let Some((ytm, vid, dur)) = ytm_watchtime_track {
            tokio::spawn(async move {
                let _ = ytm.report_watchtime(&vid, played_secs, dur).await;
            });
        }
        let Some(db) = self.stats_db.clone() else {
            return;
        };
        let track = sess.track.clone();
        let started_at = sess.started_at_unix;
        let source_tag = tui_track_source_tag(&track).to_string();
        let refresh_win =
            matches!(self.modal, ActiveModal::ListeningStats(_)).then_some(self.stats_window);
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(err) = db.record_play(
                &track.id,
                &track.title,
                &track.artist,
                &track.album,
                &source_tag,
                duration_secs,
                played_secs,
                started_at,
            ) {
                tracing::debug!(client = "tui", error = %err, "failed to record listening stat");
                return;
            }
            if let Some(window) = refresh_win {
                if let Ok(summary) = db.query_summary(window) {
                    let _ = tx.send(AppEvent::StatsLoaded { window, summary });
                }
            }
        });
    }

    pub fn toggle_gapless(&mut self) {
        self.config.gapless_enabled = !self.config.gapless_enabled;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetGapless(self.config.gapless_enabled));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.gapless_playback,
            if self.config.gapless_enabled {
                s.enabled
            } else {
                s.disabled
            }
        );
    }

    pub fn step_crossfade(&mut self, forward: bool) {
        let cur = sanitize_crossfade_secs(self.config.crossfade_secs);
        let next = if forward {
            if cur >= MAX_CROSSFADE_SECS {
                0
            } else {
                cur + 1
            }
        } else if cur == 0 {
            MAX_CROSSFADE_SECS
        } else {
            cur - 1
        };
        self.config.crossfade_secs = next;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetCrossfade(next));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = if next == 0 {
            format!("{}: {}", s.crossfade, s.off)
        } else {
            format!("{}: {next}s", s.crossfade)
        };
    }

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
                    self.status_line = crate::ui::sanitize_tui_text(&msg);
                }
            }
            SessionEvent::TrackTransition {
                generation,
                index,
                track,
                ..
            } => {
                self.finalize_listen_stats();
                self.scrobble_tracker = None;
                self.active_listen_session = None;
                self.play_seq = generation.value();
                self.playback.current_index = Some(index);
                self.playback.position_secs = 0.0;
                self.playback.duration_secs = track.duration_secs.unwrap_or(0) as f64;
                self.playback.loading_track = true;
                self.playback.paused = true;
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.playback.stream_badge = s.badge_resolving.to_string();
                self.status_line = format!(
                    "{}: {} — {}",
                    s.loading_prefix,
                    crate::ui::sanitize_tui_text(&track.title),
                    crate::ui::sanitize_tui_text(&track.artist)
                );
                self.start_client_media_for_track(generation.value(), track);
                self.sync_discord_rpc();
            }
            SessionEvent::PreparingNextTrack { track, .. } => {
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = s.badge_preparing_next.to_string();
                self.status_line = format!(
                    "{}: {}",
                    s.caching_next_track,
                    crate::ui::sanitize_tui_text(&track.title)
                );
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
                    let s = sonata_runtime::i18n::strings(self.config.language);
                    self.status_line = format!(
                        "{}: {} — {}",
                        s.starting_prefix,
                        crate::ui::sanitize_tui_text(&track.title),
                        crate::ui::sanitize_tui_text(&track.artist)
                    );
                }
                self.sync_discord_rpc();
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
                self.sync_discord_rpc();
                if was_initial_start {
                    self.start_listen_tracking(generation.value(), &track);
                    if let Some((seek_id, seek_secs)) = self.pending_connect_seek.take() {
                        if seek_id == track.id && seek_secs.is_finite() && seek_secs > 0.5 {
                            self.enqueue_session_cmd(SessionCommand::SeekAbsolute(seek_secs));
                        }
                    }
                    self.status_line.clear();
                    if !self.config.personal_only_mode && self.ytm.is_authenticated() {
                        if sonata_core::playlists::is_personal_track_id(&track.id) {
                            if self.config.effective_attribute_personal_to_ytm() {
                                let ytm = self.ytm.clone();
                                let attr_track = track.clone();
                                tokio::spawn(async move {
                                    match ytm.attribute_personal_track_to_ytm(&attr_track).await {
                                        Ok(Some(vid)) => {
                                            tracing::debug!(
                                                client = "tui",
                                                track_id = %attr_track.id,
                                                ytm_video_id = %vid,
                                                "personal track attributed to ytm"
                                            );
                                        }
                                        Ok(None) => {}
                                        Err(err) => {
                                            tracing::debug!(
                                                client = "tui",
                                                track_id = %attr_track.id,
                                                error = %err,
                                                "ytm personal attribution failed"
                                            );
                                        }
                                    }
                                });
                            }
                        } else if !track
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
                } else {
                    self.resume_listen_tracking();
                }
                self.start_cover_enrichment();
            }
            SessionEvent::AudioQualityUpdated {
                generation,
                track_id,
                badge,
                ..
            } => {
                if generation.value() != self.play_seq {
                    return;
                }
                if self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id != track_id)
                {
                    return;
                }
                self.playback.stream_badge =
                    merge_measured_audio_badge(&self.playback.stream_badge, &badge);
            }
            SessionEvent::PausedChanged {
                paused,
                position_secs,
                ..
            } => {
                self.playback.paused = paused;
                self.playback.position_secs = position_secs;
                if paused {
                    self.pause_listen_tracking();
                } else {
                    self.resume_listen_tracking();
                }
                self.sync_discord_rpc();
            }
            SessionEvent::Seeked { position_secs, .. } => {
                self.playback.position_secs = position_secs;
                let _ = self
                    .mpris_tx
                    .send(crate::mpris::MprisUpdate::Seeked(position_secs));
                self.sync_discord_rpc();
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
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.status_line = s.upgrade_failed_continuing.to_string();
            }
            SessionEvent::RecoveryStarted { generation, .. } => {
                if generation.value() != self.play_seq {
                    return;
                }
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = s.badge_retrying.to_string();
                self.status_line = s.stream_expired_retrying.to_string();
            }
            SessionEvent::TrackEnded { .. } => {
                self.finalize_listen_stats();
            }
            SessionEvent::QueueFinished { .. } => {
                self.finalize_listen_stats();
                if !self.config.personal_only_mode
                    && self.config.autoplay
                    && self.playback.repeat == RepeatMode::Off
                    && !self.playback.queue.is_empty()
                {
                    let play_seq = self.play_seq;
                    let is_ytm_seed = |t: &Track| {
                        !t.id.is_empty()
                            && !t.id.starts_with("local:")
                            && !t.id.starts_with("jiosaavn:")
                            && sonata_core::providers::servers::parse_server_item_id(&t.id)
                                .is_none()
                    };
                    let mut seeds: Vec<String> = Vec::new();
                    if let Some(first) = self.playback.queue.iter().find(|t| is_ytm_seed(t)) {
                        seeds.push(first.id.clone());
                    }
                    if let Some(cur) = self.playback.current_track().filter(|t| is_ytm_seed(t)) {
                        if !seeds.iter().any(|id| id == &cur.id) {
                            seeds.push(cur.id.clone());
                        }
                    }
                    let excludes: Vec<String> =
                        self.playback.queue.iter().map(|t| t.id.clone()).collect();
                    let preferred_artists: Vec<String> = Vec::new();
                    // Do not attach `self.current_playlist_id` (sidebar view state) to endless radio.
                    let playlist_id: Option<String> = None;
                    let ytm = self.ytm.clone();
                    let tx = self.tx.clone();
                    self.playback.loading_track = true;
                    self.playback.paused = true;
                    let s = sonata_runtime::i18n::strings(self.config.language);
                    self.playback.stream_badge = s.badge_radio.to_string();
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
                    let s = sonata_runtime::i18n::strings(self.config.language);
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = s.badge_finished.to_string();
                    self.discord_rpc.try_clear_activity();
                }
            }
            SessionEvent::Stopped { generation } => {
                self.finalize_listen_stats();
                self.scrobble_tracker = None;
                self.active_listen_session = None;
                self.play_seq = generation.value();
                self.media_tasks.restart();
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.playback.paused = true;
                self.playback.loading_track = false;
                self.playback.position_secs = 0.0;
                self.playback.stream_badge = s.badge_stopped.to_string();
                self.discord_rpc.try_clear_activity();
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
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.status_line = format!(
                    "{}: {}",
                    s.playback_error,
                    crate::ui::sanitize_tui_text(&error)
                );
                self.sync_discord_rpc();
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
            SessionEvent::PlaybackOptionsChanged {
                playback_speed,
                skip_silence,
                video_mode,
                sleep_timer_remaining_secs,
            } => {
                self.playback_speed = playback_speed;
                self.skip_silence = skip_silence;
                self.video_mode = video_mode;
                self.sleep_timer_deadline = sleep_timer_remaining_secs
                    .filter(|&s| s > 0)
                    .map(|secs| Instant::now() + Duration::from_secs(secs));
                self.sync_discord_rpc();
            }
            SessionEvent::PrecacheUpdated { .. } => {}
        }
    }

    pub fn sleep_timer_remaining_secs(&self) -> Option<u64> {
        let deadline = self.sleep_timer_deadline?;
        let rem = deadline.saturating_duration_since(Instant::now());
        let secs = rem.as_secs() + u64::from(rem.subsec_nanos() > 0);
        (secs > 0).then_some(secs)
    }

    pub fn set_playback_speed(&mut self, speed: f64) {
        let sanitized = sonata_runtime::config::sanitize_playback_speed(speed);
        self.playback_speed = sanitized;
        self.config.playback_speed = sanitized;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetPlaybackSpeed(sanitized));
        self.sync_discord_rpc();
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!("{}: {:.2}x", s.playback_speed, sanitized);
    }

    pub fn step_playback_speed(&mut self, forward: bool) {
        let next = step_speed_preset(self.playback_speed, forward);
        self.set_playback_speed(next);
    }

    pub fn toggle_skip_silence(&mut self) {
        self.config.skip_silence = !self.config.skip_silence;
        self.skip_silence = self.config.skip_silence && !self.video_mode;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetSkipSilence(self.config.skip_silence));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.skip_silence,
            if self.config.skip_silence {
                s.enabled
            } else {
                s.disabled
            }
        );
    }

    pub fn cycle_sleep_timer(&mut self, forward: bool) {
        let next = next_sleep_preset(self.sleep_timer_remaining_secs(), forward);
        self.sleep_timer_deadline = next.map(|secs| Instant::now() + Duration::from_secs(secs));
        self.enqueue_session_cmd(SessionCommand::SetSleepTimer(next));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = match next {
            Some(secs) => format!("{}: {}m", s.sleep_timer, secs / 60),
            None => format!("{}: {}", s.sleep_timer, s.off),
        };
    }

    pub fn toggle_lyrics_translate(&mut self) {
        self.config.lyrics_translate = !self.config.lyrics_translate;
        self.save_config_async();
        if !self.config.lyrics_translate {
            self.translated_lyrics = None;
        } else {
            self.refresh_lyrics_transforms();
        }
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.translate_lyrics,
            if self.config.lyrics_translate {
                s.enabled
            } else {
                s.disabled
            }
        );
    }

    pub fn toggle_lyrics_romanize(&mut self) {
        self.config.lyrics_romanize = !self.config.lyrics_romanize;
        self.save_config_async();
        if !self.config.lyrics_romanize {
            self.romanized_lyrics = None;
        } else {
            self.refresh_lyrics_transforms();
        }
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.romanize_lyrics,
            if self.config.lyrics_romanize {
                s.enabled
            } else {
                s.disabled
            }
        );
    }

    pub fn refresh_lyrics_transforms(&mut self) {
        if !self.config.lyrics_translate && !self.config.lyrics_romanize {
            return;
        }
        let Some(track) = self.playback.current_track().cloned() else {
            return;
        };
        if !matches!(
            self.lyrics,
            LyricsState::Synced(ref lines) if !lines.is_empty()
        ) && !matches!(
            self.lyrics,
            LyricsState::Plain(ref lines) if !lines.is_empty()
        ) {
            return;
        }
        let play_seq = self.play_seq;
        let lyrics_state = self.lyrics.clone();
        let http = self.http.clone();
        let paths = self.paths.clone();
        let tx = self.tx.clone();

        if self.config.lyrics_translate && self.translated_lyrics.is_none() {
            let target_lang = self.config.language.code().to_string();
            let track_id = track.id.clone();
            let http_t = http.clone();
            let paths_t = paths.clone();
            let state_t = lyrics_state.clone();
            let tx_t = tx.clone();
            self.media_tasks.spawn(async move {
                let res =
                    translate_lyrics_state(&http_t, &paths_t, &track_id, &state_t, &target_lang)
                        .await;
                let lines = res.unwrap_or(None);
                let _ = tx_t.send(AppEvent::LyricsTranslated {
                    play_seq,
                    track_id,
                    target_lang,
                    lines,
                });
            });
        }
        if self.config.lyrics_romanize && self.romanized_lyrics.is_none() {
            let track_id = track.id;
            self.media_tasks.spawn(async move {
                let res = romanize_lyrics_state(&http, &paths, &track_id, &lyrics_state).await;
                let lines = res.unwrap_or(None);
                let _ = tx.send(AppEvent::LyricsRomanized {
                    play_seq,
                    track_id,
                    lines,
                });
            });
        }
    }

    pub fn toggle_hifi_substitution(&mut self) {
        self.config.hifi_substitution = !self.config.hifi_substitution;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::UpdatePolicy {
            policy: self.session_policy(),
            trigger_upgrade: self.config.hifi_substitution,
        });
        let i18n = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            i18n.hifi_upgrade,
            if self.config.hifi_substitution {
                i18n.enabled
            } else {
                i18n.disabled
            }
        );
    }

    fn start_cover_enrichment(&mut self) {
        if self.config.personal_only_mode {
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
        self.translated_lyrics = None;
        self.romanized_lyrics = None;
        self.lyrics_scroll_init = false;
        self.lyrics_scroll_target = 0.0;
        self.lyrics_scroll_vel = 0.0;
        self.lyrics_prev_active = None;
        self.lyrics_cur_active = None;
        self.album_art = None;
        self.kitty_art_png = None;
        self.anim_art_frames.clear();
        self.motion_cover_path = None;
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

        if !self.config.personal_only_mode && self.config.motion_covers {
            let anim_tx = self.tx.clone();
            let anim_http = http;
            let anim_track = track;
            let anim_paths = self.paths.clone();
            let anim_enabled = self.config.motion_covers;
            if self.config.reduce_motion {
                self.media_tasks.spawn(async move {
                    if let Some(motion_path) = sonata_runtime::motion::resolve_motion_cover_path(
                        &anim_http,
                        &anim_track,
                        &anim_paths,
                        anim_enabled,
                    )
                    .await
                    {
                        let _ = anim_tx.send(AppEvent::MotionPathReady {
                            play_seq,
                            track_id: anim_track.id,
                            motion_path,
                        });
                    }
                });
            } else {
                let picker = self.picker.clone();
                let need_kitty_png = picker.protocol_type() == ProtocolType::Kitty;
                let (max_dim, target_fps, max_frames) = if need_kitty_png {
                    (320, 15.0, 40)
                } else {
                    (360, 15.0, 40)
                };
                self.media_tasks.spawn(async move {
                    let Some((motion_path, motion)) =
                        sonata_runtime::motion::resolve_motion_cover_with_path(
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
                                decoded
                                    .push((Some(picker.new_resize_protocol(dyn_img)), Vec::new()));
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
                            motion_path,
                            frames,
                        });
                    }
                });
            }
        }
    }

    pub fn play_selected_track(&mut self) {
        let Some(selected) = self.track_table_state.selected() else {
            return;
        };
        if self.visible_tracks.is_empty() || selected >= self.visible_tracks.len() {
            return;
        }
        if self.config.personal_only_mode {
            let track = &self.visible_tracks[selected];
            let is_downloaded = self.downloaded_track_ids.contains(&track.id);
            if !super::can_play_tui_track(track, true, is_downloaded) {
                let s = sonata_runtime::i18n::strings(self.config.language);
                self.status_line = s.personal_only_streaming_disabled.to_string();
                return;
            }
            let filtered: Vec<Track> = self
                .visible_tracks
                .iter()
                .filter(|t| {
                    super::can_play_tui_track(t, true, self.downloaded_track_ids.contains(&t.id))
                })
                .cloned()
                .collect();
            let start_index = filtered.iter().position(|t| t.id == track.id).unwrap_or(0);
            self.enqueue_session_cmd(SessionCommand::PlayQueue {
                tracks: filtered,
                start_index,
                push_history: false,
            });
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
            self.save_config_async();
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
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetShuffle(self.playback.shuffle));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.shuffle,
            if self.playback.shuffle {
                s.on_label
            } else {
                s.off
            }
        );
    }

    pub fn cycle_repeat(&mut self) {
        self.playback.repeat = self.playback.repeat.next();
        self.config.repeat_mode = self.playback.repeat;
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::SetRepeat(self.playback.repeat));
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.repeat,
            sonata_runtime::i18n::repeat_mode_label(self.config.language, self.playback.repeat)
        );
    }

    pub fn toggle_autoplay(&mut self) {
        self.config.autoplay = !self.config.autoplay;
        self.save_config_async();
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
        let cur_pos = self.interpolated_position().max(0.0);
        let dur_secs_f = self.playback.duration_secs.max(0.0);
        let pos_secs = cur_pos.floor() as u64;
        let dur_secs = dur_secs_f.floor() as u64;

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
                .and_then(|u| {
                    u.strip_prefix("file://")
                        .map(std::path::PathBuf::from)
                        .or_else(|| u.starts_with('/').then(|| std::path::PathBuf::from(u)))
                        .filter(|p| p.exists())
                })
                .or_else(|| art_url.map(media::art_cache_path).filter(|p| p.exists()))
                .map(|p| p.to_string_lossy().into_owned())
        });
        let motion_art_path = self.motion_cover_path.as_ref().filter(|p| p.exists());
        let motion_art_path_str = motion_art_path.map(|p| p.to_string_lossy().into_owned());
        let motion_art_url = motion_art_path.and_then(|p| media::path_to_file_url(p));
        let ambient_rgb = match self.base_theme.accent {
            crate::theme::RgbColor::Rgb(r, g, b) => Some((r, g, b)),
            _ => None,
        };

        let export = media::NowPlayingExport::build(media::NowPlayingInput {
            client: "tui",
            status,
            track,
            quality: &self.playback.stream_badge,
            position_secs: cur_pos,
            duration_secs: dur_secs_f,
            lyrics: &self.lyrics,
            lyric_position_secs: cur_pos,
            translated_lyrics: self.translated_lyrics.as_deref(),
            romanized_lyrics: self.romanized_lyrics.as_deref(),
            art_path: art_path.as_deref(),
            art_url,
            motion_art_path: motion_art_path_str.as_deref(),
            motion_art_url: motion_art_url.as_deref(),
            ambient_rgb,
        });

        let lyrics_key = format!(
            "{track_id}|{}|{}|{:?}|{}|{}",
            export.lyrics.state,
            export.lyrics.line_count,
            export.lyrics.active_index,
            self.translated_lyrics.is_some(),
            self.romanized_lyrics.is_some(),
        );
        if self.last_lyrics_export_key.as_ref() != Some(&lyrics_key) {
            self.last_lyrics_export_key = Some(lyrics_key);
            let full_lyrics = media::LyricsExport::build(
                "tui",
                track,
                &self.lyrics,
                cur_pos,
                self.translated_lyrics.as_deref(),
                self.romanized_lyrics.as_deref(),
            );
            media::write_lyrics_export(&full_lyrics);
        }

        let fingerprint = ExportFingerprint {
            track_id: track_id.to_string(),
            status: status.to_string(),
            lyric: export.lyric.clone(),
            lyric_index: export.lyrics.active_index,
            lyric_word_index: export
                .lyrics
                .current
                .as_ref()
                .and_then(|c| c.active_word.as_ref().map(|w| w.index)),
            lyrics_state: export.lyrics.state.clone(),
            has_translation: self.translated_lyrics.is_some(),
            has_romanization: self.romanized_lyrics.is_some(),
            art_path: art_path.clone(),
            motion_art_path: motion_art_path_str,
            pos_secs,
            dur_secs,
        };
        if self.last_exported_state.as_ref() == Some(&fingerprint) {
            return;
        }
        self.last_exported_state = Some(fingerprint);
        media::write_now_playing(&export);
        self.sync_mpris(art_path.as_deref().map(std::path::PathBuf::from));
    }

    pub fn sync_mpris(&self, art_path_override: Option<std::path::PathBuf>) {
        let track = self.playback.current_track().cloned();
        let art_path = art_path_override.or_else(|| {
            track
                .as_ref()
                .and_then(|t| t.thumbnail_url.as_deref())
                .map(media::art_cache_path)
                .filter(|p| p.exists())
        });
        let snap = crate::mpris::TuiMprisSnapshot {
            track,
            art_path,
            motion_cover_path: self.motion_cover_path.clone(),
            position_secs: self.interpolated_position(),
            duration_secs: self.playback.duration_secs,
            paused: self.playback.paused,
            muted: self.playback.volume == 0,
            volume: self.playback.volume,
            shuffle: self.playback.shuffle,
            repeat: self.playback.repeat,
            queue_len: self.playback.queue.len(),
            current_index: self.playback.current_index,
        };
        let _ = self
            .mpris_tx
            .send(crate::mpris::MprisUpdate::Sync(Box::new(snap)));
    }

    pub fn clear_now_playing(&self) {
        media::clear_now_playing();
    }

    pub(super) fn discord_config_from(
        config: &AppConfig,
    ) -> sonata_runtime::discord::DiscordConfig {
        sonata_runtime::discord::DiscordConfig {
            enabled: config.discord_rpc_enabled,
            ..sonata_runtime::discord::DiscordConfig::default()
        }
    }

    fn discord_track_info(track: &Track) -> sonata_runtime::discord::DiscordTrackInfo {
        let mut info = sonata_runtime::discord::DiscordTrackInfo::from_track(track);
        if sonata_core::playlists::is_personal_track_id(&track.id) {
            info.listen_url = None;
            info.buttons.clear();
            if track
                .thumbnail_url
                .as_deref()
                .is_none_or(|u| u.starts_with('/') || u.starts_with("file://"))
            {
                info.artwork_url = None;
            }
        }
        info
    }

    pub fn sync_discord_rpc(&self) {
        let cfg = Self::discord_config_from(&self.config);
        if !cfg.enabled {
            self.discord_rpc.try_update_config(cfg);
            return;
        }
        let Some(track) = self.playback.current_track() else {
            self.discord_rpc.try_clear_activity();
            return;
        };
        let info = Self::discord_track_info(track);
        let pos = self.playback.position_secs;
        let state = if self.playback.paused || self.playback.loading_track {
            sonata_runtime::discord::DiscordPlaybackState::paused(
                Some(info),
                pos,
                self.playback_speed,
            )
        } else {
            sonata_runtime::discord::DiscordPlaybackState::playing(info, pos, self.playback_speed)
        };
        self.discord_rpc.try_update_playback(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::providers::ProviderSource;

    #[test]
    fn tui_discord_track_info_strips_ytm_links_for_self_hosted_tracks() {
        let track = Track {
            id: "subsonic:srv1:tr9".to_string(),
            title: "Local FLAC".to_string(),
            artist: "Home Artist".to_string(),
            album: "Navidrome".to_string(),
            album_id: None,
            duration_secs: Some(180),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            liked: false,
            is_explicit: false,
            set_video_id: None,
            audio_badge: Some("FLAC".to_string()),
        };
        let info = App::discord_track_info(&track);
        assert!(info.listen_url.is_none());
        assert!(info.buttons.is_empty());
        assert!(info.artwork_url.is_none());
    }

    #[test]
    fn tui_discord_config_locks_app_id_and_respects_enabled() {
        let cfg = AppConfig {
            discord_rpc_enabled: true,
            discord_app_id: Some("123456789012345678".to_string()),
            ..Default::default()
        };
        let dc = App::discord_config_from(&cfg);
        assert!(dc.enabled);
        assert_eq!(
            dc.effective_app_id(),
            sonata_runtime::discord::DEFAULT_SONATA_DISCORD_APP_ID
        );
    }

    #[test]
    fn measured_audio_badge_preserves_source_context_tags() {
        assert_eq!(
            merge_measured_audio_badge("FLAC · Cached", "FLAC · 880 kbps · 16-bit/44.1kHz"),
            "FLAC · 880 kbps · 16-bit/44.1kHz · Cached"
        );
        assert_eq!(
            merge_measured_audio_badge("AAC 320kbps · JioSaavn", "AAC · 320 kbps"),
            "AAC · 320 kbps · JioSaavn"
        );
        assert_eq!(
            merge_measured_audio_badge("OPUS 160kbps · HQ", "OPUS · 158 kbps"),
            "OPUS · 158 kbps"
        );
        assert_eq!(
            merge_measured_audio_badge(
                "FLAC · Jellyfin · Cached",
                "FLAC · 2450 kbps · 24-bit/96kHz"
            ),
            "FLAC · 2450 kbps · 24-bit/96kHz · Jellyfin · Cached"
        );
    }

    #[test]
    fn playback_speed_and_sleep_presets_cycle_cleanly() {
        assert_eq!(step_speed_preset(1.0, true), 1.25);
        assert_eq!(step_speed_preset(2.0, true), 0.75);
        assert_eq!(step_speed_preset(0.75, false), 2.0);
        assert_eq!(next_sleep_preset(None, true), Some(900));
        assert_eq!(next_sleep_preset(Some(900), true), Some(1800));
        assert_eq!(next_sleep_preset(Some(3600), true), None);
        assert_eq!(next_sleep_preset(None, false), Some(3600));
        assert_eq!(format_sleep_countdown(905), "15:05");
        assert_eq!(format_sleep_countdown(3661), "1:01:01");
    }
}
