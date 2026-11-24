//! Desktop playback session adapter, client-local lyrics/artwork/motion loading, and status export.
//!
//! 1. **Session Bridge**: maps `SessionEvent`s and coalesced `PlaybackProgress` into desktop view state
//!    and forwards seek updates to MPRIS2.
//! 2. **Client Media Tasks**: owns desktop-local lyrics, artwork, cover enrichment, and GPUI motion
//!    covers under a per-generation `PlaybackTaskSet`.
//! 3. **Now Playing Export**: writes deduplicated `$XDG_RUNTIME_DIR/sonata/now_playing.json` state.

use super::types::{track_source_tag, ExportFingerprint, InternalEvent};
use super::BackendEngine;
use crate::integrations::mpris::MprisUpdate;
use sonata_core::lyrics::{
    active_line_index, lrclib, romanize_lyrics_state, translate_lyrics_state, LyricsState,
};
use sonata_core::player::RepeatMode;
use sonata_core::providers::hifi::{ResolvedMusicVideo, VideoQualityStream};
use sonata_core::providers::ytmusic::MusicVideoSyncSegment;
use sonata_core::providers::Track;
use sonata_core::redaction::sensitive_text;
use sonata_core::scrobble::{should_scrobble, ScrobbleTrack, ScrobbleTracker};
use sonata_runtime::config::AppConfig;
use sonata_runtime::discord::{DiscordConfig, DiscordPlaybackState, DiscordTrackInfo};
use sonata_runtime::media;
use sonata_runtime::motion;
use sonata_runtime::player::{SessionCommand, SessionEvent};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PLAYBACK_SPEED_PRESETS: &[f64] = &[0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
pub const SLEEP_TIMER_PRESET_SECS: &[Option<u64>] = &[
    None,
    Some(15 * 60),
    Some(30 * 60),
    Some(45 * 60),
    Some(60 * 60),
];
pub const VIDEO_QUALITY_OPTIONS: &[&str] = &["Auto", "1080p", "720p", "480p", "360p"];

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

/// Parses a video quality preference (`"Auto"`, `"1080p"`, `"720p"`, `"480p"`, `"360p"`) into a height cap.
pub fn video_quality_max_height(preference: &str) -> Option<u32> {
    let trimmed = preference.trim().to_ascii_lowercase();
    if trimmed.is_empty() || trimmed == "auto" {
        return None;
    }
    trimmed
        .trim_end_matches('p')
        .parse::<u32>()
        .ok()
        .filter(|&h| h > 0)
}

/// Selects the best matching [`VideoQualityStream`] for `preference`, honoring height cap with
/// fps and bitrate tie-breakers and falling back to the lowest available stream if all exceed the cap.
pub fn select_video_quality_stream<'a>(
    qualities: &'a [VideoQualityStream],
    preference: &str,
) -> Option<&'a VideoQualityStream> {
    if qualities.is_empty() {
        return None;
    }
    let mut sorted: Vec<&VideoQualityStream> = qualities.iter().collect();
    sorted.sort_by(|a, b| {
        b.height
            .cmp(&a.height)
            .then_with(|| b.fps.cmp(&a.fps))
            .then_with(|| b.bitrate_bps.cmp(&a.bitrate_bps))
    });
    let Some(cap) = video_quality_max_height(preference) else {
        return sorted.into_iter().next();
    };
    sorted
        .iter()
        .copied()
        .find(|q| q.height == cap)
        .or_else(|| sorted.iter().copied().find(|q| q.height <= cap))
        .or_else(|| sorted.last().copied())
}

/// Maps an album-audio playback position (seconds) onto the companion Music Video timeline.
pub fn map_music_to_video_secs(music_secs: f64, segments: &[MusicVideoSyncSegment]) -> f64 {
    let clamped_secs = if music_secs.is_finite() {
        music_secs.max(0.0)
    } else {
        0.0
    };
    let music_ms = (clamped_secs * 1000.0).round() as u64;
    if segments.is_empty() {
        return clamped_secs;
    }
    let mut ordered: Vec<&MusicVideoSyncSegment> =
        segments.iter().filter(|s| s.duration_ms > 0).collect();
    if ordered.is_empty() {
        return clamped_secs;
    }
    ordered.sort_by_key(|s| s.music_start_ms);

    let first = ordered[0];
    if music_ms < first.music_start_ms {
        return first.video_start_ms as f64 / 1000.0;
    }
    for seg in &ordered {
        let m_end = seg.music_start_ms.saturating_add(seg.duration_ms);
        if music_ms < seg.music_start_ms {
            return seg.video_start_ms as f64 / 1000.0;
        }
        if music_ms <= m_end {
            let mapped_ms = seg
                .video_start_ms
                .saturating_add(music_ms - seg.music_start_ms);
            return mapped_ms as f64 / 1000.0;
        }
    }
    let last = ordered[ordered.len() - 1];
    let last_m_end = last.music_start_ms.saturating_add(last.duration_ms);
    let last_v_end = last.video_start_ms.saturating_add(last.duration_ms);
    last_v_end.saturating_add(music_ms.saturating_sub(last_m_end)) as f64 / 1000.0
}

/// Maps a Music Video playback position (seconds) onto the canonical album-audio timeline
/// used by synced lyrics and Music mode resume.
pub fn map_video_to_music_secs(video_secs: f64, segments: &[MusicVideoSyncSegment]) -> f64 {
    if !video_secs.is_finite() || video_secs <= 0.0 {
        return 0.0;
    }
    let video_ms = (video_secs * 1000.0).round() as u64;
    if segments.is_empty() {
        return video_secs;
    }
    let mut ordered: Vec<&MusicVideoSyncSegment> =
        segments.iter().filter(|s| s.duration_ms > 0).collect();
    if ordered.is_empty() {
        return video_secs;
    }
    ordered.sort_by_key(|s| s.video_start_ms);

    let first = ordered[0];
    if video_ms < first.video_start_ms {
        return first.music_start_ms as f64 / 1000.0;
    }
    for seg in &ordered {
        let v_end = seg.video_start_ms.saturating_add(seg.duration_ms);
        if video_ms < seg.video_start_ms {
            return seg.music_start_ms as f64 / 1000.0;
        }
        if video_ms <= v_end {
            let mapped_ms = seg
                .music_start_ms
                .saturating_add(video_ms - seg.video_start_ms);
            return mapped_ms as f64 / 1000.0;
        }
    }
    let last = ordered[ordered.len() - 1];
    last.music_start_ms.saturating_add(last.duration_ms) as f64 / 1000.0
}

pub(super) fn discord_config_from(config: &AppConfig) -> DiscordConfig {
    DiscordConfig {
        enabled: config.discord_rpc_enabled,
        ..DiscordConfig::default()
    }
    .with_app_id(config.discord_app_id.clone())
}

fn discord_track_info(track: &Track, duration_secs: f64) -> DiscordTrackInfo {
    let mut info = DiscordTrackInfo::from_track(track);
    if duration_secs > 0.0 {
        info.duration_secs = Some(duration_secs);
    }
    if sonata_core::playlists::is_personal_track_id(&track.id) {
        info.listen_url = None;
        info.buttons.clear();
        if track.thumbnail_url.is_none() {
            info.artwork_url = None;
        }
    }
    info
}

impl BackendEngine {
    pub(super) fn remaining_sleep_secs(&self) -> Option<u64> {
        let deadline = self.sleep_timer_deadline?;
        let rem = deadline.saturating_duration_since(Instant::now());
        let secs = rem.as_secs() + u64::from(rem.subsec_nanos() > 0);
        (secs > 0).then_some(secs)
    }

    pub(super) fn now_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub(super) fn start_listen_tracking(&mut self, play_seq: u64, track: &Track) {
        let now_unix = Self::now_unix_secs();
        let scrobble_track = ScrobbleTrack::from_track(track);
        self.scrobble_tracker = Some(ScrobbleTracker::start(
            track.id.clone(),
            scrobble_track.clone(),
            now_unix,
        ));
        self.active_listen_session = Some(super::ActiveListenSession {
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
            let int_tx = self.int_tx.clone();
            tokio::spawn(async move {
                match client.update_now_playing(&cfg, &scrobble_track).await {
                    Ok(()) => {
                        let _ = int_tx.send(InternalEvent::ScrobbleError(None));
                    }
                    Err(err) => {
                        let safe = sensitive_text(&err.to_string());
                        tracing::debug!(client = "desktop", error = %safe, "scrobble now-playing failed");
                        let _ = int_tx.send(InternalEvent::ScrobbleError(Some(safe)));
                    }
                }
            });
        }
    }

    pub(super) fn pause_listen_tracking(&mut self) {
        if let Some(ref mut sess) = self.active_listen_session {
            if let Some(prev) = sess.last_tick.take() {
                let delta = prev.elapsed().as_secs_f64().clamp(0.0, 2.0);
                sess.accumulated_secs += delta;
            }
        }
    }

    pub(super) fn resume_listen_tracking(&mut self) {
        if let Some(ref mut sess) = self.active_listen_session {
            if sess.last_tick.is_none() {
                sess.last_tick = Some(Instant::now());
            }
        }
    }

    pub(super) fn advance_listen_tracking(&mut self) {
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
                    let int_tx = self.int_tx.clone();
                    tokio::spawn(async move {
                        match client.scrobble(&cfg, &track_owned, Some(started_at)).await {
                            Ok(()) => {
                                let _ = int_tx.send(InternalEvent::ScrobbleError(None));
                            }
                            Err(err) => {
                                let safe = sensitive_text(&err.to_string());
                                tracing::debug!(client = "desktop", error = %safe, "scrobble submit failed");
                                let _ = int_tx.send(InternalEvent::ScrobbleError(Some(safe)));
                            }
                        }
                    });
                }
            }
        }
    }

    pub(super) fn finalize_listen_stats(&mut self) {
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
        let Some(db) = self.stats_db.clone() else {
            return;
        };
        let track = sess.track.clone();
        let started_at = sess.started_at_unix;
        let source_tag = track_source_tag(&track).to_string();
        let refresh_window =
            (self.active_nav == super::types::NavTarget::Stats).then_some(self.stats_window);
        let int_tx = self.int_tx.clone();
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
                tracing::debug!(client = "desktop", error = %err, "failed to record listening stat");
                return;
            }
            if let Some(window) = refresh_window {
                if let Ok(summary) = db.query_summary(window) {
                    let _ = int_tx.send(InternalEvent::StatsLoaded { window, summary });
                }
            }
        });
    }

    pub(super) async fn handle_session_event(&mut self, event: SessionEvent) {
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
                self.finalize_listen_stats();
                self.scrobble_tracker = None;
                self.active_listen_session = None;
                self.play_seq = generation.value();
                self.playback.current_index = Some(index);
                self.playback.position_secs = 0.0;
                self.playback.duration_secs = track.duration_secs.unwrap_or(0) as f64;
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.playback.stream_badge = "Resolving...".to_string();
                self.upgrading_quality = false;
                self.music_video_candidate = None;
                self.resolved_music_video = None;
                self.video_loading = false;
                self.audio_stream_badge_backup = None;
                self.translated_lyrics = None;
                self.romanized_lyrics = None;
                if self.video_mode {
                    self.video_mode = false;
                    self.skip_silence = self.config.skip_silence;
                    let _ = self
                        .session
                        .send(SessionCommand::SetVideoMode {
                            enabled: false,
                            url: None,
                            headers: Vec::new(),
                            seek_secs: None,
                        })
                        .await;
                }
                self.status_line = format!("Loading: {} — {}", track.title, track.artist);
                self.record_played_track(track.clone());
                self.start_client_media_for_track(generation.value(), track);
                self.maybe_prefetch_endless_autoplay();
                self.sync_discord_rpc();
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
                if !self.video_mode {
                    self.audio_stream_badge_backup = Some(stream.badge.clone());
                    self.playback.stream_badge = stream.badge;
                }
                if let Some(track) = self.playback.current_track().filter(|t| t.id == track_id) {
                    self.status_line = format!("Starting: {} — {}", track.title, track.artist);
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
                if !self.video_mode {
                    self.audio_stream_badge_backup = Some(badge.clone());
                    self.playback.stream_badge = badge;
                }
                if was_initial_start {
                    self.start_listen_tracking(generation.value(), &track);
                    if let Some((seek_id, seek_secs)) = self.pending_connect_seek.take() {
                        if seek_id == track.id && seek_secs.is_finite() && seek_secs > 0.5 {
                            let _ = self
                                .session
                                .send(SessionCommand::SeekAbsolute(seek_secs))
                                .await;
                        }
                    }
                    self.status_line = format!("Playing: {} — {}", track.title, track.artist);
                    if sonata_core::playlists::is_personal_track_id(&track.id) {
                        if self.config.effective_attribute_personal_to_ytm()
                            && !self.config.personal_only_mode
                            && self.ytm.is_authenticated()
                        {
                            let ytm = self.ytm.clone();
                            let attr_track = track.clone();
                            tokio::spawn(async move {
                                match ytm.attribute_personal_track_to_ytm(&attr_track).await {
                                    Ok(Some(matched_id)) => {
                                        tracing::debug!(
                                            client = "desktop",
                                            track_id = %attr_track.id,
                                            matched_id = %matched_id,
                                            "attributed personal track play to ytm"
                                        );
                                    }
                                    Ok(None) => {}
                                    Err(err) => {
                                        tracing::debug!(
                                            client = "desktop",
                                            track_id = %attr_track.id,
                                            error = %err,
                                            "personal track ytm attribution failed"
                                        );
                                    }
                                }
                            });
                        }
                    } else if !track.is_jiosaavn()
                        && !self.config.personal_only_mode
                        && self.ytm.is_authenticated()
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
                } else {
                    self.resume_listen_tracking();
                }
                self.start_cover_enrichment();
                self.sync_discord_rpc();
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
                let merged = merge_measured_audio_badge(&self.playback.stream_badge, &badge);
                if !self.video_mode {
                    self.audio_stream_badge_backup = Some(merged.clone());
                }
                self.playback.stream_badge = merged;
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
                let _ = self.mpris_tx.send(MprisUpdate::Seeked(position_secs));
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
                self.sync_discord_rpc();
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
                self.audio_stream_badge_backup = Some(stream.badge.clone());
                if !self.video_mode {
                    self.playback.stream_badge = stream.badge.clone();
                }
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
            SessionEvent::TrackEnded { .. } => {
                self.finalize_listen_stats();
            }
            SessionEvent::QueueFinished { .. } => {
                self.finalize_listen_stats();
                if self.config.autoplay
                    && !self.config.personal_only_mode
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
                    self.discord_rpc.try_clear_activity();
                }
            }
            SessionEvent::Stopped { generation } => {
                self.finalize_listen_stats();
                self.scrobble_tracker = None;
                self.active_listen_session = None;
                self.play_seq = generation.value();
                self.media_tasks.restart();
                self.playback.paused = true;
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.video_mode = false;
                self.video_loading = false;
                self.music_video_candidate = None;
                self.resolved_music_video = None;
                self.audio_stream_badge_backup = None;
                self.translated_lyrics = None;
                self.romanized_lyrics = None;
                self.playback.position_secs = 0.0;
                self.playback.stream_badge = "Stopped".to_string();
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
                self.upgrading_quality = false;
                self.playback.paused = true;
                self.playback.stream_badge = badge;
                self.status_line = format!("Playback failed: {error}");
                self.sync_discord_rpc();
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

    pub(super) fn refresh_lyrics_transforms(&mut self) {
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
        let int_tx = self.int_tx.clone();

        if self.config.lyrics_translate && self.translated_lyrics.is_none() {
            let target_lang = self.ytm_language.code().to_string();
            let track_id = track.id.clone();
            let http_t = http.clone();
            let paths_t = paths.clone();
            let state_t = lyrics_state.clone();
            let tx_t = int_tx.clone();
            self.media_tasks.spawn(async move {
                let res =
                    translate_lyrics_state(&http_t, &paths_t, &track_id, &state_t, &target_lang)
                        .await;
                let lines = res.unwrap_or(None);
                let _ = tx_t.send(InternalEvent::LyricsTranslated {
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
                let _ = int_tx.send(InternalEvent::LyricsRomanized {
                    play_seq,
                    track_id,
                    lines,
                });
            });
        }
    }

    pub(super) async fn set_video_mode_enabled(&mut self, enabled: bool) {
        if enabled == self.video_mode && !self.video_loading {
            return;
        }
        if !enabled {
            let video_pos = self.interpolated_position();
            let music_pos = self
                .music_video_candidate
                .as_ref()
                .map_or(video_pos, |cand| {
                    map_video_to_music_secs(video_pos, &cand.sync_segments)
                });
            self.video_mode = false;
            self.video_loading = false;
            self.skip_silence = self.config.skip_silence;
            self.playback.position_secs = music_pos;
            if let Some(ref backup) = self.audio_stream_badge_backup {
                self.playback.stream_badge = backup.clone();
            }
            let _ = self
                .session
                .send(SessionCommand::SetVideoMode {
                    enabled: false,
                    url: None,
                    headers: Vec::new(),
                    seek_secs: Some(music_pos),
                })
                .await;
            self.status_line = "Switched to Music mode".to_string();
            return;
        }

        let Some(track) = self.playback.current_track().cloned() else {
            return;
        };
        let Some(cand) = self.music_video_candidate.clone() else {
            self.status_line = sonata_runtime::i18n::strings(self.ytm_language)
                .video_unavailable
                .to_string();
            return;
        };

        if let Some(resolved) = self
            .resolved_music_video
            .as_ref()
            .filter(|mv| mv.video_id == cand.video_id && !mv.qualities.is_empty())
            .cloned()
        {
            self.activate_resolved_music_video(&cand.sync_segments, &resolved)
                .await;
            return;
        }

        self.video_loading = true;
        self.status_line = sonata_runtime::i18n::strings(self.ytm_language)
            .loading_video
            .to_string();
        let play_seq = self.play_seq;
        let track_id = track.id;
        let video_id = cand.video_id;
        let core_resolver = self.resolver.inner().clone();
        let int_tx = self.int_tx.clone();
        self.media_tasks.spawn(async move {
            let result = core_resolver
                .resolve_music_video_streams(&video_id)
                .await
                .map_err(|err| err.to_string());
            let _ = int_tx.send(InternalEvent::MusicVideoStreamsReady {
                play_seq,
                track_id,
                video_id,
                result,
            });
        });
    }

    pub(super) async fn handle_music_video_streams_ready(
        &mut self,
        video_id: String,
        result: Result<ResolvedMusicVideo, String>,
    ) {
        match result {
            Ok(resolved) if !resolved.qualities.is_empty() => {
                let segments = self
                    .music_video_candidate
                    .as_ref()
                    .filter(|c| c.video_id == video_id)
                    .map(|c| c.sync_segments.clone())
                    .unwrap_or_default();
                self.resolved_music_video = Some(resolved.clone());
                self.activate_resolved_music_video(&segments, &resolved)
                    .await;
            }
            Ok(_) => {
                self.video_mode = false;
                self.status_line = sonata_runtime::i18n::strings(self.ytm_language)
                    .video_unavailable
                    .to_string();
            }
            Err(err) => {
                self.video_mode = false;
                self.status_line = format!(
                    "{}: {err}",
                    sonata_runtime::i18n::strings(self.ytm_language).video_unavailable
                );
            }
        }
    }

    async fn activate_resolved_music_video(
        &mut self,
        segments: &[MusicVideoSyncSegment],
        resolved: &ResolvedMusicVideo,
    ) {
        let Some(stream) =
            select_video_quality_stream(&resolved.qualities, &self.config.video_quality)
        else {
            self.video_mode = false;
            self.status_line = sonata_runtime::i18n::strings(self.ytm_language)
                .video_unavailable
                .to_string();
            return;
        };

        let raw_pos = self.interpolated_position();
        let seek_secs = if self.video_mode {
            raw_pos
        } else {
            if self.audio_stream_badge_backup.is_none() {
                self.audio_stream_badge_backup = Some(self.playback.stream_badge.clone());
            }
            map_music_to_video_secs(raw_pos, segments)
        };
        self.video_mode = true;
        self.video_loading = false;
        self.skip_silence = false;
        self.playback.position_secs = seek_secs;
        let q_label = if stream.quality_label.trim().is_empty() {
            format!("{}p", stream.height)
        } else {
            stream.quality_label.clone()
        };
        self.playback.stream_badge = format!("{q_label} · Video");
        let _ = self
            .session
            .send(SessionCommand::SetVideoMode {
                enabled: true,
                url: Some(stream.video_url.clone()),
                headers: resolved.headers.clone(),
                seek_secs: Some(seek_secs),
            })
            .await;
        self.status_line = format!("Video mode ({q_label})");
    }

    pub(super) async fn apply_video_quality(&mut self, quality: String) {
        let normalized = VIDEO_QUALITY_OPTIONS
            .iter()
            .find(|opt| opt.eq_ignore_ascii_case(quality.trim()))
            .copied()
            .unwrap_or("Auto")
            .to_string();
        self.config.video_quality = normalized.clone();
        let _ = self.config.save();
        if self.video_mode {
            if let Some(resolved) = self.resolved_music_video.clone() {
                let segments = self
                    .music_video_candidate
                    .as_ref()
                    .map(|c| c.sync_segments.clone())
                    .unwrap_or_default();
                self.activate_resolved_music_video(&segments, &resolved)
                    .await;
                return;
            }
        }
        self.status_line = format!(
            "{}: {}",
            sonata_runtime::i18n::strings(self.ytm_language).video_quality,
            normalized
        );
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
            .filter(|t| {
                !t.id.is_empty()
                    && !t.id.starts_with("local:")
                    && !t.is_jiosaavn()
                    && !t.is_self_hosted()
            })
            .map(|t| t.id.clone())
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
            || self.config.personal_only_mode
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
        if self.config.personal_only_mode {
            self.motion_cover = None;
            return;
        }
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
        } else if !self.config.personal_only_mode
            && !self.ui_compact
            && self.config.motion_covers
            && self.motion_cover.is_none()
        {
            self.cover_started_play_seq = None;
            self.start_cover_enrichment();
        }
    }

    fn start_client_media_for_track(&mut self, play_seq: u64, track: Track) {
        self.media_tasks.restart();
        self.cover_started_play_seq = None;
        self.motion_cover = None;

        if let Some(url) = track.thumbnail_url.as_deref() {
            let local_cover = url
                .strip_prefix("file://")
                .map(PathBuf::from)
                .or_else(|| url.starts_with('/').then(|| PathBuf::from(url)));
            if let Some(path) = local_cover.filter(|p| p.exists()) {
                self.art_path = Some(path);
            } else {
                let upscaled = media::upscale_artwork_url(url);
                let cached = media::art_cache_path(&upscaled);
                self.art_path = cached.exists().then_some(cached);
            }
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

        if !self.config.personal_only_mode
            && !track.id.trim().is_empty()
            && !sonata_core::playlists::is_personal_track_id(&track.id)
        {
            let mv_ytm = ytm.clone();
            let mv_track = track.clone();
            let mv_tx = int_tx.clone();
            self.media_tasks.spawn(async move {
                let candidate = mv_ytm.resolve_music_video_candidate(&mv_track).await;
                let _ = mv_tx.send(InternalEvent::MusicVideoCandidateReady {
                    play_seq,
                    track_id: mv_track.id,
                    candidate,
                });
            });
        }

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

    pub(super) fn sync_discord_rpc(&self) {
        let Some(track) = self.playback.current_track() else {
            self.discord_rpc.try_clear_activity();
            return;
        };
        let pos = self.interpolated_position();
        let dur = if self.playback.duration_secs > 0.0 {
            self.playback.duration_secs
        } else {
            track.duration_secs.unwrap_or(0) as f64
        };
        let info = discord_track_info(track, dur);
        let state = if self.playback.paused || self.playback.loading_track {
            DiscordPlaybackState::paused(Some(info), pos, self.playback_speed)
        } else {
            DiscordPlaybackState::playing(info, pos, self.playback_speed)
        };
        self.discord_rpc.try_update_playback(state);
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
        let lyric_pos = if self.video_mode {
            self.music_video_candidate.as_ref().map_or(cur_pos, |cand| {
                map_video_to_music_secs(cur_pos, &cand.sync_segments)
            })
        } else {
            cur_pos
        };

        let active_lyric = match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, lyric_pos)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::providers::ProviderSource;

    #[test]
    fn discord_track_info_clears_ytm_links_for_self_hosted_tracks() {
        let track = Track {
            id: "jellyfin:srv1:item42".to_string(),
            title: "Midnight Signal".to_string(),
            artist: "Echoes".to_string(),
            album: "Home NAS".to_string(),
            album_id: None,
            duration_secs: Some(215),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            liked: false,
            is_explicit: false,
            set_video_id: None,
            audio_badge: Some("FLAC".to_string()),
        };
        let info = discord_track_info(&track, 215.0);
        assert!(info.listen_url.is_none());
        assert!(info.buttons.is_empty());
        assert!(info.artwork_url.is_none());
    }

    #[test]
    fn discord_config_respects_enabled_and_custom_app_id() {
        let cfg = AppConfig {
            discord_rpc_enabled: false,
            discord_app_id: Some("999999999999999999".to_string()),
            ..Default::default()
        };
        let dc = discord_config_from(&cfg);
        assert!(!dc.enabled);
        assert_eq!(dc.effective_app_id(), "999999999999999999");
    }

    #[test]
    fn desktop_measured_badge_preserves_source_context() {
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
    }

    #[test]
    fn video_quality_selector_picks_exact_or_best_under_cap() {
        let streams = vec![
            VideoQualityStream {
                quality_label: "360p".to_string(),
                height: 360,
                fps: 30,
                video_url: "https://v/360".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate_bps: 500_000,
                is_muxed: true,
            },
            VideoQualityStream {
                quality_label: "720p60".to_string(),
                height: 720,
                fps: 60,
                video_url: "https://v/720".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate_bps: 1_800_000,
                is_muxed: false,
            },
            VideoQualityStream {
                quality_label: "1080p".to_string(),
                height: 1080,
                fps: 30,
                video_url: "https://v/1080".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate_bps: 2_500_000,
                is_muxed: false,
            },
        ];
        assert_eq!(
            select_video_quality_stream(&streams, "Auto").map(|s| s.height),
            Some(1080)
        );
        assert_eq!(
            select_video_quality_stream(&streams, "720p").map(|s| s.height),
            Some(720)
        );
        assert_eq!(
            select_video_quality_stream(&streams, "480p").map(|s| s.height),
            Some(360)
        );
        assert!(select_video_quality_stream(&[], "Auto").is_none());
    }

    #[test]
    fn segment_sync_maps_music_and_video_timelines_bidirectionally() {
        let segments = vec![
            MusicVideoSyncSegment {
                music_start_ms: 0,
                video_start_ms: 12_000,
                duration_ms: 30_000,
            },
            MusicVideoSyncSegment {
                music_start_ms: 30_000,
                video_start_ms: 50_000,
                duration_ms: 60_000,
            },
        ];
        // Intro skip: music 0s -> video 12s; video 5s -> holds music 0s
        assert!((map_music_to_video_secs(0.0, &segments) - 12.0).abs() < 1e-3);
        assert!((map_video_to_music_secs(5.0, &segments) - 0.0).abs() < 1e-3);
        // Inside first segment: music 10s -> video 22s; video 22s -> music 10s
        assert!((map_music_to_video_secs(10.0, &segments) - 22.0).abs() < 1e-3);
        assert!((map_video_to_music_secs(22.0, &segments) - 10.0).abs() < 1e-3);
        // Mid-video skit (42s..50s): holds music at 30s
        assert!((map_video_to_music_secs(45.0, &segments) - 30.0).abs() < 1e-3);
        // Second segment: music 40s -> video 60s
        assert!((map_music_to_video_secs(40.0, &segments) - 60.0).abs() < 1e-3);
        assert!((map_video_to_music_secs(60.0, &segments) - 40.0).abs() < 1e-3);
    }
}
