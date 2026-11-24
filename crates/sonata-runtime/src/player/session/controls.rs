//! Playback transport controls, crossfade ramps, sleep timer fades, and video-mode switching.
//!
//! 1. **Transport & Options**: handles pause, seek, volume/mute, speed, and silence skipping.
//! 2. **Crossfade & Sleep Timer**: implements linear volume ramps for track transitions (`0..=12` s,
//!    auto-bypassed in Video mode) and sleep-timer fade-out with volume restoration.
//! 3. **Video Mode**: switches audio-only and video playback while preserving position and headers.

use super::{SessionActor, SessionEvent};
use crate::player::mpv::clamp_playback_speed;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator, PlaybackTransition};
use std::time::{Duration, Instant};

/// Smooth volume ramp-down duration before a sleep timer pauses playback.
pub const SLEEP_TIMER_FADE_DURATION: Duration = Duration::from_secs(5);
/// Maximum supported track transition crossfade duration in seconds.
pub const MAX_CROSSFADE_SECS: u8 = 12;

/// Clamps crossfade duration into the supported `[0, 12]` seconds range.
pub fn clamp_crossfade_secs(secs: u8) -> u8 {
    secs.min(MAX_CROSSFADE_SECS)
}

/// Computes the exit fade-out volume (`0..=base_volume`) as `remaining_secs` approaches `0`.
pub fn compute_crossfade_exit_volume(base_volume: u8, remaining_secs: f64, fade_secs: f64) -> u8 {
    if !remaining_secs.is_finite() || !fade_secs.is_finite() || fade_secs <= 0.0 {
        return base_volume;
    }
    let ratio = (remaining_secs / fade_secs).clamp(0.0, 1.0);
    ((base_volume as f64) * ratio)
        .round()
        .clamp(0.0, base_volume as f64) as u8
}

/// Computes the entry fade-in volume (`0..=base_volume`) as `elapsed_secs` progresses toward `fade_secs`.
pub fn compute_crossfade_entry_volume(base_volume: u8, elapsed_secs: f64, fade_secs: f64) -> u8 {
    if !elapsed_secs.is_finite() || !fade_secs.is_finite() || fade_secs <= 0.0 {
        return base_volume;
    }
    let ratio = (elapsed_secs / fade_secs).clamp(0.0, 1.0);
    ((base_volume as f64) * ratio)
        .round()
        .clamp(0.0, base_volume as f64) as u8
}

#[derive(Debug, Clone)]
pub(super) struct SleepTimerState {
    pub deadline: Instant,
    pub fade_duration: Duration,
    pub pre_fade_volume: Option<u8>,
    pub last_applied_volume: Option<u8>,
}

#[derive(Debug, Clone)]
pub(super) struct CrossfadeEntryState {
    pub started_at: Instant,
    pub duration_secs: f64,
    pub last_applied_volume: Option<u8>,
}

impl SessionActor {
    pub(super) fn effective_skip_silence(&self) -> bool {
        self.skip_silence_pref && !self.video_mode
    }

    pub(super) fn effective_crossfade_secs(&self) -> u8 {
        if self.video_mode {
            0
        } else {
            self.crossfade_secs
        }
    }

    pub(super) fn sleep_timer_fading(&self) -> bool {
        self.sleep_timer
            .as_ref()
            .is_some_and(|t| t.pre_fade_volume.is_some())
    }

    pub(super) fn cancel_crossfade(&mut self, restore_volume: bool) {
        let was_fading = self.crossfade_exit_active || self.crossfade_entry.is_some();
        self.crossfade_exit_active = false;
        self.crossfade_pending_entry = false;
        self.crossfade_last_exit_vol = None;
        self.crossfade_entry = None;
        if restore_volume && was_fading && !self.muted && !self.sleep_timer_fading() {
            self.driver.set_volume(self.playback.volume);
        }
    }

    pub(super) fn begin_crossfade_entry_if_pending(&mut self) {
        if !self.crossfade_pending_entry {
            return;
        }
        self.crossfade_pending_entry = false;
        let fade_secs = self.effective_crossfade_secs();
        if fade_secs > 0 && !self.muted && !self.sleep_timer_fading() {
            let entry_duration = (fade_secs as f64).clamp(0.5, 4.0);
            self.crossfade_entry = Some(CrossfadeEntryState {
                started_at: Instant::now(),
                duration_secs: entry_duration,
                last_applied_volume: Some(0),
            });
            self.driver.set_volume(0);
        }
    }

    pub(super) fn tick_crossfade(&mut self) {
        if self.muted || self.sleep_timer_fading() {
            return;
        }
        let fade_secs = self.effective_crossfade_secs();
        if fade_secs == 0 {
            self.cancel_crossfade(true);
            return;
        }
        if self.playback.paused || self.playback.loading_track {
            return;
        }

        if let Some(entry) = self.crossfade_entry.as_mut() {
            let elapsed = entry.started_at.elapsed().as_secs_f64();
            if elapsed >= entry.duration_secs {
                self.crossfade_entry = None;
                self.driver.set_volume(self.playback.volume);
            } else {
                let vol = compute_crossfade_entry_volume(
                    self.playback.volume,
                    elapsed,
                    entry.duration_secs,
                );
                if entry.last_applied_volume != Some(vol) {
                    entry.last_applied_volume = Some(vol);
                    self.driver.set_volume(vol);
                }
            }
            return;
        }

        let duration = self.playback.duration_secs;
        if duration <= 1.0 || self.playback.peek_next_index().is_none() {
            return;
        }
        let active_fade = (fade_secs as f64).min(duration * 0.4);
        if active_fade <= 0.0 {
            return;
        }
        let remaining = (duration - self.interpolated_position()).max(0.0);
        if remaining <= active_fade {
            self.crossfade_exit_active = true;
            self.crossfade_pending_entry = true;
            let vol = compute_crossfade_exit_volume(self.playback.volume, remaining, active_fade);
            if self.crossfade_last_exit_vol != Some(vol) {
                self.crossfade_last_exit_vol = Some(vol);
                self.driver.set_volume(vol);
            }
        }
    }

    pub(super) fn sleep_timer_remaining_secs(&self) -> Option<u64> {
        self.sleep_timer.as_ref().map(|timer| {
            let rem = timer.deadline.saturating_duration_since(Instant::now());
            rem.as_secs() + u64::from(rem.subsec_nanos() > 0)
        })
    }

    pub(super) async fn emit_mode_changed(&self) {
        self.emit(SessionEvent::ModeChanged {
            shuffle: self.playback.shuffle,
            repeat: self.playback.repeat,
            volume: self.playback.volume,
            muted: self.muted,
        })
        .await;
    }

    pub(super) async fn emit_playback_options_changed(&self) {
        self.emit(SessionEvent::PlaybackOptionsChanged {
            playback_speed: self.playback_speed,
            skip_silence: self.effective_skip_silence(),
            video_mode: self.video_mode,
            sleep_timer_remaining_secs: self.sleep_timer_remaining_secs(),
        })
        .await;
    }

    pub(super) async fn set_volume(&mut self, vol: u8) {
        let clamped = vol.min(100);
        self.muted = false;
        self.playback.volume = clamped;
        self.cancel_crossfade(false);
        if let Some(timer) = self.sleep_timer.as_mut() {
            if timer.pre_fade_volume.is_some() {
                timer.pre_fade_volume = Some(clamped);
                timer.last_applied_volume = Some(clamped);
            }
        }
        self.driver.set_volume(clamped);
        self.emit_mode_changed().await;
    }

    pub(super) async fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if muted {
            self.cancel_crossfade(false);
        }
        let effective = if self.muted { 0 } else { self.playback.volume };
        self.driver.set_volume(effective);
        self.emit_mode_changed().await;
    }

    pub(super) async fn set_playback_speed(&mut self, speed: f64) {
        let clamped = clamp_playback_speed(speed);
        if !self.playback.paused && !self.playback.loading_track {
            let cur = self.interpolated_position();
            self.playback.position_secs = cur;
            self.pos_anchor = Some((cur, Instant::now()));
        }
        self.playback_speed = clamped;
        self.driver.set_speed(clamped);
        self.emit_playback_options_changed().await;
    }

    pub(super) async fn set_skip_silence(&mut self, enabled: bool) {
        self.skip_silence_pref = enabled;
        let effective = self.effective_skip_silence();
        self.driver.set_skip_silence(effective);
        self.emit_playback_options_changed().await;
    }

    pub(super) fn set_crossfade(&mut self, secs: u8) {
        self.crossfade_secs = clamp_crossfade_secs(secs);
        if self.effective_crossfade_secs() == 0 {
            self.cancel_crossfade(true);
        }
    }

    pub(super) async fn set_sleep_timer(&mut self, seconds: Option<u64>) {
        if let Some(prev) = self.sleep_timer.take() {
            if let Some(orig) = prev.pre_fade_volume {
                self.playback.volume = orig;
                if !self.muted {
                    self.driver.set_volume(orig);
                }
            }
        }
        if let Some(secs) = seconds {
            if secs == 0 {
                self.set_paused(true).await;
            } else {
                let total = Duration::from_secs(secs);
                let fade_duration = total.min(SLEEP_TIMER_FADE_DURATION);
                self.sleep_timer = Some(SleepTimerState {
                    deadline: Instant::now() + total,
                    fade_duration,
                    pre_fade_volume: None,
                    last_applied_volume: None,
                });
            }
        }
        self.emit_playback_options_changed().await;
    }

    pub(super) async fn tick_sleep_timer(&mut self) {
        let now = Instant::now();
        let Some(timer) = self.sleep_timer.as_mut() else {
            return;
        };
        if now >= timer.deadline {
            let timer = self.sleep_timer.take().expect("sleep timer checked above");
            let restore_vol = timer.pre_fade_volume.unwrap_or(self.playback.volume);
            self.set_paused(true).await;
            self.playback.volume = restore_vol;
            if !self.muted {
                self.driver.set_volume(restore_vol);
            }
            self.emit_playback_options_changed().await;
            return;
        }
        let remaining = timer.deadline.saturating_duration_since(now);
        if remaining <= timer.fade_duration && !timer.fade_duration.is_zero() {
            if timer.pre_fade_volume.is_none() {
                timer.pre_fade_volume = Some(self.playback.volume);
            }
            let base_vol = timer.pre_fade_volume.unwrap_or(self.playback.volume);
            let ratio =
                (remaining.as_secs_f64() / timer.fade_duration.as_secs_f64()).clamp(0.0, 1.0);
            let faded = ((base_vol as f64) * ratio)
                .round()
                .clamp(0.0, base_vol as f64) as u8;
            if !self.muted && timer.last_applied_volume != Some(faded) {
                timer.last_applied_volume = Some(faded);
                self.driver.set_volume(faded);
            }
        }
    }

    pub(super) async fn set_video_mode(
        &mut self,
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    ) {
        self.video_mode = enabled;
        if enabled {
            self.cancel_crossfade(true);
        }
        self.driver.set_skip_silence(self.effective_skip_silence());

        let (target_url, target_headers) = match url {
            Some(u) if !u.is_empty() => (Some(u), headers),
            _ => match self.playback.active_stream.as_ref() {
                Some(active) => {
                    let hdrs = if headers.is_empty() {
                        active.headers.clone()
                    } else {
                        headers
                    };
                    (Some(active.url.clone()), hdrs)
                }
                None => (None, headers),
            },
        };
        let effective_seek = seek_secs.or_else(|| {
            self.playback
                .current_track()
                .map(|_| self.interpolated_position())
        });

        if target_url.is_some() {
            if let Some(pos) = effective_seek {
                self.playback.position_secs = pos.max(0.0);
            }
            self.pos_anchor = None;
            self.awaiting_playback_start = true;
            self.stream_loaded_at = Some(Instant::now());
        }

        if let Err(err) = self
            .driver
            .set_video_mode(enabled, target_url.clone(), target_headers, effective_seek)
            .await
        {
            self.awaiting_playback_start = false;
            tracing::warn!(
                event = "video_mode_switch_failed",
                client = self.config.client_name,
                enabled,
                error = %err,
                "failed to switch video mode"
            );
            self.emit(SessionEvent::PlaybackError {
                generation: self.generation,
                track_id: self.playback.current_track().map(|t| t.id.clone()),
                error: err,
                badge: self.playback.stream_badge.clone(),
            })
            .await;
        } else if target_url.is_some() && self.playback.paused {
            self.driver.set_paused(true);
        }
        self.publish_progress();
        self.emit_playback_options_changed().await;
    }

    pub(super) async fn set_paused(&mut self, paused: bool) {
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
        self.driver.set_paused(paused);
        self.publish_progress();
        self.emit(SessionEvent::PausedChanged {
            generation: self.generation,
            paused,
            position_secs: self.playback.position_secs,
        })
        .await;
    }

    pub(super) async fn seek_absolute(&mut self, secs: f64) {
        let Some(PlaybackTransition::Seeked(clamped)) =
            PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Seek(secs))
        else {
            return;
        };
        if !self.playback.paused && !self.playback.loading_track {
            self.pos_anchor = Some((clamped, Instant::now()));
        } else {
            self.pos_anchor = None;
        }
        self.driver.seek_absolute(clamped);
        self.publish_progress();
        self.emit(SessionEvent::Seeked {
            generation: self.generation,
            position_secs: clamped,
        })
        .await;
        self.maybe_precache_next().await;
    }
}
