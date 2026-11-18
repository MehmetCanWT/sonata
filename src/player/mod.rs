pub mod mpv;

use crate::config::EnhancerMode;
use crate::providers::Track;
pub use mpv::{MpvController, MpvEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "All",
            Self::One => "One",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub paused: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub enhancer: EnhancerMode,
    pub stream_badge: String,
    pub loading_track: bool,
}

impl PlaybackState {
    pub fn new(volume: u8, enhancer: EnhancerMode) -> Self {
        Self {
            queue: Vec::new(),
            current_index: None,
            paused: true,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume,
            shuffle: false,
            repeat: RepeatMode::Off,
            enhancer,
            stream_badge: "Idle".to_string(),
            loading_track: false,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    pub fn next_index(&self) -> Option<usize> {
        let cur = self.current_index?;
        if self.queue.is_empty() {
            return None;
        }
        if self.repeat == RepeatMode::One {
            return Some(cur);
        }
        if self.shuffle && self.queue.len() > 1 {
            // Deterministic-free fast pseudo-random index distinct from current
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(1);
            let mut next = nanos % self.queue.len();
            if next == cur {
                next = (cur + 1) % self.queue.len();
            }
            return Some(next);
        }
        if cur + 1 < self.queue.len() {
            Some(cur + 1)
        } else if self.repeat == RepeatMode::All {
            Some(0)
        } else {
            None
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        let cur = self.current_index?;
        if self.queue.is_empty() {
            return None;
        }
        if cur > 0 {
            Some(cur - 1)
        } else if self.repeat == RepeatMode::All {
            Some(self.queue.len() - 1)
        } else {
            Some(0)
        }
    }
}
