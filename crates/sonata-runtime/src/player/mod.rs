//! Linux audio playback drivers and shared playback session state machine.
//!
//! 1. **Controller (`mpv`)**: Spawns and controls headless `mpv` instances with gapless playback
//!    and redacted IPC logging.
//! 2. **Session (`session`)**: Shared Linux playback session owning generation tracking, transport
//!    transitions, recovery budgets, Hi-Fi upgrade swaps, lookahead precache, and bounded shutdown.

pub mod mpv;
pub mod session;

pub use mpv::{MpvController, MpvEvent};
pub use session::{
    NoopAudioDriver, PlaybackAudioDriver, PlaybackProgress, PlaybackSession, PlaybackSessionConfig,
    PlaybackSessionHandle, PlaybackStreamSource, PrecacheNotice, SessionClosed, SessionCommand,
    SessionEvent, SessionPolicy, ShutdownReport, SESSION_COMMAND_CAPACITY, SESSION_EVENT_CAPACITY,
};
