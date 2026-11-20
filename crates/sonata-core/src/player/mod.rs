pub mod mpv;

use crate::config::EnhancerMode;
use crate::providers::hifi::ResolvedStream;
use crate::providers::Track;
pub use mpv::{MpvController, MpvEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
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
    pub upcoming_index: Option<usize>,
    pub history: Vec<usize>,
    pub paused: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub enhancer: EnhancerMode,
    pub stream_badge: String,
    pub loading_track: bool,
    pub active_stream: Option<ResolvedStream>,
    pub base_stream: Option<ResolvedStream>,
}

impl PlaybackState {
    pub fn new(volume: u8, enhancer: EnhancerMode) -> Self {
        Self {
            queue: Vec::new(),
            current_index: None,
            upcoming_index: None,
            history: Vec::new(),
            paused: true,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume,
            shuffle: false,
            repeat: RepeatMode::Off,
            enhancer,
            stream_badge: "Idle".to_string(),
            loading_track: false,
            active_stream: None,
            base_stream: None,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    pub fn clear_upcoming(&mut self) {
        self.upcoming_index = None;
    }

    pub fn commit_play(&mut self, index: usize, push_history: bool) {
        self.active_stream = None;
        self.base_stream = None;
        if push_history {
            if let Some(prev) = self.current_index {
                if prev != index && prev < self.queue.len() {
                    self.history.push(prev);
                    if self.history.len() > 100 {
                        let excess = self.history.len() - 100;
                        self.history.drain(..excess);
                    }
                }
            }
        }
        self.current_index = Some(index);
        self.upcoming_index = None;
    }

    pub fn set_active_stream(&mut self, stream: ResolvedStream) {
        self.active_stream = Some(stream);
        self.base_stream = None;
    }

    pub fn set_upgraded_stream(&mut self, stream: ResolvedStream) -> bool {
        let Some(active) = self.active_stream.take() else {
            return false;
        };
        if active.url == stream.url {
            self.active_stream = Some(active);
            return false;
        }
        self.base_stream = Some(active);
        self.active_stream = Some(stream);
        true
    }

    pub fn restore_base_stream(&mut self) -> Option<ResolvedStream> {
        let base = self.base_stream.take()?;
        self.active_stream = Some(base.clone());
        Some(base)
    }

    pub fn peek_next_index(&mut self) -> Option<usize> {
        let cur = self.current_index?;
        if self.queue.is_empty() {
            self.upcoming_index = None;
            return None;
        }
        if self.repeat == RepeatMode::One {
            self.upcoming_index = Some(cur);
            return Some(cur);
        }
        if self.shuffle && self.queue.len() > 1 {
            if let Some(cached) = self.upcoming_index {
                if cached < self.queue.len() && cached != cur {
                    return Some(cached);
                }
            }
            let next = self.next_index()?;
            self.upcoming_index = Some(next);
            return Some(next);
        }
        self.upcoming_index = None;
        self.next_index()
    }

    pub fn pop_prev_index(&mut self) -> Option<usize> {
        while let Some(idx) = self.history.pop() {
            if idx < self.queue.len() && Some(idx) != self.current_index {
                return Some(idx);
            }
        }
        self.prev_index()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    fn dummy_track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artist: "Artist".into(),
            album: String::new(),
            duration_secs: Some(180),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
        }
    }

    #[test]
    fn peek_next_and_history_stay_consistent_in_shuffle() {
        let mut st = PlaybackState::new(80, EnhancerMode::HiResCrystal);
        st.queue = vec![
            dummy_track("t0"),
            dummy_track("t1"),
            dummy_track("t2"),
            dummy_track("t3"),
        ];
        st.shuffle = true;
        st.commit_play(0, false);

        let peek1 = st.peek_next_index().expect("shuffled next");
        let peek2 = st.peek_next_index().expect("cached shuffled next");
        assert_eq!(peek1, peek2);
        assert_ne!(peek1, 0);

        st.commit_play(peek1, true);
        assert_eq!(st.current_index, Some(peek1));
        assert_eq!(st.pop_prev_index(), Some(0));
    }
}
