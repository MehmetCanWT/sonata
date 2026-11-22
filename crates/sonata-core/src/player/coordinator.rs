//! Pure command-to-transition logic for Sonata playback.
//!
//! 1. **Commands**: `PlaybackCommand` describes user intent (play, pause, seek, next, previous, queue change).
//! 2. **Transitions**: `PlaybackTransition` describes the side effect the caller must perform, such as
//!    loading a track into mpv.
//! 3. **Application**: `PlaybackCoordinator::apply` mutates `PlaybackState` and returns the transition, or
//!    `None` when the command has no effect. It performs no I/O, so front-ends share identical queue behavior.

use super::PlaybackState;
use crate::providers::Track;

#[derive(Debug, Clone)]
pub enum PlaybackCommand {
    Play { index: usize, push_history: bool },
    Pause(bool),
    Seek(f64),
    Next,
    Previous,
    QueueChanged(Vec<Track>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PlaybackTransition {
    Play {
        index: usize,
        push_history: bool,
        track: Track,
    },
    Paused(bool),
    Seeked(f64),
    QueueChanged,
}

pub struct PlaybackCoordinator;

impl PlaybackCoordinator {
    pub fn apply(
        state: &mut PlaybackState,
        command: PlaybackCommand,
    ) -> Option<PlaybackTransition> {
        match command {
            PlaybackCommand::Play {
                index,
                push_history,
            } => Self::start_play(state, index, push_history),
            PlaybackCommand::Next => {
                let index = state.peek_next_index()?;
                Self::start_play(state, index, true)
            }
            PlaybackCommand::Previous => {
                let index = state.pop_prev_index()?;
                Self::start_play(state, index, false)
            }
            PlaybackCommand::Pause(paused) => {
                state.paused = paused;
                Some(PlaybackTransition::Paused(paused))
            }
            PlaybackCommand::Seek(position) => {
                let position = if state.duration_secs > 0.0 {
                    position.clamp(0.0, state.duration_secs)
                } else {
                    position.max(0.0)
                };
                state.position_secs = position;
                Some(PlaybackTransition::Seeked(position))
            }
            PlaybackCommand::QueueChanged(queue) => {
                state.queue = queue;
                state.history.clear();
                state.clear_upcoming();
                if state
                    .current_index
                    .is_some_and(|index| index >= state.queue.len())
                {
                    state.current_index = None;
                }
                Some(PlaybackTransition::QueueChanged)
            }
        }
    }

    fn start_play(
        state: &mut PlaybackState,
        index: usize,
        push_history: bool,
    ) -> Option<PlaybackTransition> {
        let track = state.queue.get(index)?.clone();
        state.commit_play(index, push_history);
        state.position_secs = 0.0;
        state.duration_secs = track.duration_secs.unwrap_or(0) as f64;
        state.loading_track = true;
        state.paused = true;
        state.stream_badge = "Resolving...".to_string();
        Some(PlaybackTransition::Play {
            index,
            push_history,
            track,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    fn track(id: &str, duration_secs: u64) -> Track {
        Track {
            id: id.to_string(),
            title: id.to_string(),
            artist: "Artist".to_string(),
            album: String::new(),
            duration_secs: Some(duration_secs),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        }
    }

    #[test]
    fn shared_commands_keep_queue_and_transport_consistent() {
        let mut state = PlaybackState::new(80);
        PlaybackCoordinator::apply(
            &mut state,
            PlaybackCommand::QueueChanged(vec![track("a", 100), track("b", 200)]),
        );
        let started = PlaybackCoordinator::apply(
            &mut state,
            PlaybackCommand::Play {
                index: 0,
                push_history: false,
            },
        );
        assert!(matches!(
            started,
            Some(PlaybackTransition::Play { index: 0, .. })
        ));
        assert_eq!(state.duration_secs, 100.0);
        assert!(state.loading_track);

        let next = PlaybackCoordinator::apply(&mut state, PlaybackCommand::Next);
        assert!(matches!(
            next,
            Some(PlaybackTransition::Play { index: 1, .. })
        ));
        assert_eq!(state.history, vec![0]);

        PlaybackCoordinator::apply(&mut state, PlaybackCommand::Seek(999.0));
        assert_eq!(state.position_secs, 200.0);
        PlaybackCoordinator::apply(&mut state, PlaybackCommand::Pause(false));
        assert!(!state.paused);

        let previous = PlaybackCoordinator::apply(&mut state, PlaybackCommand::Previous);
        assert!(matches!(
            previous,
            Some(PlaybackTransition::Play { index: 0, .. })
        ));
    }
}
