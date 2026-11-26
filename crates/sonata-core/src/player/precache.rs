//! Shared next-track precache scheduling rules and lifecycle state machine.
//!
//! 1. **Scheduling Window (`NEXT_TRACK_PRECACHE_LEAD_SECS`)**: Triggers lookahead precaching
//!    once playback enters the final 10 seconds of the active track without duplicating work.
//! 2. **Invalidation & Lifecycle (`NextTrackPrecache`, `PrecacheKey`)**: Tracks `Idle`,
//!    `Resolving`, `Ready`, and `Failed` states and cancels only when playback identity or the
//!    resolved next track changes.

/// Delay before a track ends when the next-track cache may start.
pub const NEXT_TRACK_PRECACHE_LEAD_SECS: f64 = 10.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecacheKey {
    pub play_seq: u64,
    pub current_track_id: String,
    pub next_track_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NextTrackPrecacheState {
    #[default]
    Idle,
    Resolving,
    Ready,
    Failed,
}

#[derive(Debug, Default)]
pub struct NextTrackPrecache {
    key: Option<PrecacheKey>,
    state: NextTrackPrecacheState,
}

impl NextTrackPrecache {
    /// Return a key only once the player enters the final precache window.
    ///
    /// A started task remains valid after a backwards seek. It is invalidated
    /// only when playback identity or the resolved next track changes.
    pub fn schedule(
        &mut self,
        play_seq: u64,
        current_track_id: &str,
        next_track_id: Option<&str>,
        position_secs: f64,
        duration_secs: f64,
        enabled: bool,
    ) -> Option<PrecacheKey> {
        let requested = next_track_id.filter(|next| {
            enabled
                && duration_secs > 0.0
                && *next != current_track_id
                && duration_secs - position_secs <= NEXT_TRACK_PRECACHE_LEAD_SECS
        });

        let next_track_id = requested?;
        let key = PrecacheKey {
            play_seq,
            current_track_id: current_track_id.to_string(),
            next_track_id: next_track_id.to_string(),
        };
        if self.key.as_ref() == Some(&key) {
            return None;
        }
        self.key = Some(key.clone());
        self.state = NextTrackPrecacheState::Resolving;
        Some(key)
    }

    pub fn cancel_if_changed(
        &mut self,
        play_seq: u64,
        current_track_id: Option<&str>,
        next_track_id: Option<&str>,
    ) -> Option<PrecacheKey> {
        let key = self.key.as_ref()?;
        let still_valid = current_track_id == Some(key.current_track_id.as_str())
            && next_track_id == Some(key.next_track_id.as_str())
            && key.play_seq == play_seq;
        if still_valid {
            return None;
        }
        self.take()
    }

    pub fn take(&mut self) -> Option<PrecacheKey> {
        self.state = NextTrackPrecacheState::Idle;
        self.key.take()
    }

    pub fn mark_ready(&mut self, key: &PrecacheKey) {
        if self.key.as_ref() == Some(key) {
            self.state = NextTrackPrecacheState::Ready;
        }
    }

    pub fn mark_failed(&mut self, key: &PrecacheKey) {
        if self.key.as_ref() == Some(key) {
            self.state = NextTrackPrecacheState::Failed;
        }
    }

    pub fn state(&self) -> NextTrackPrecacheState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_once_inside_final_ten_seconds() {
        let mut precache = NextTrackPrecache::default();
        assert!(precache
            .schedule(4, "current", Some("next"), 89.9, 100.0, true)
            .is_none());
        let key = precache
            .schedule(4, "current", Some("next"), 90.0, 100.0, true)
            .expect("final window schedules");
        assert_eq!(key.next_track_id, "next");
        assert!(precache
            .schedule(4, "current", Some("next"), 95.0, 100.0, true)
            .is_none());
    }

    #[test]
    fn keeps_started_work_after_backwards_seek() {
        let mut precache = NextTrackPrecache::default();
        let _ = precache.schedule(2, "current", Some("next"), 90.0, 100.0, true);
        assert!(precache
            .cancel_if_changed(2, Some("current"), Some("next"))
            .is_none());
    }

    #[test]
    fn invalidates_when_next_track_changes() {
        let mut precache = NextTrackPrecache::default();
        let _ = precache.schedule(2, "current", Some("next"), 90.0, 100.0, true);
        let cancelled = precache
            .cancel_if_changed(2, Some("current"), Some("replacement"))
            .expect("changed target cancels");
        assert_eq!(cancelled.next_track_id, "next");
        assert_eq!(precache.state(), NextTrackPrecacheState::Idle);
    }

    #[test]
    fn repeat_one_never_schedules_the_current_track() {
        let mut precache = NextTrackPrecache::default();
        assert!(precache
            .schedule(2, "current", Some("current"), 90.0, 100.0, true)
            .is_none());
    }
}
