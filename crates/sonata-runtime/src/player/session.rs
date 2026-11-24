//! Shared Linux playback session owning transport transitions, recovery, Hi-Fi upgrades, and precache.
//!
//! 1. **Abstractions**: `PlaybackAudioDriver` and `PlaybackStreamSource` decouple the session state
//!    machine from live `mpv` processes and network resolvers so every transition is deterministically testable.
//! 2. **Bounded Channels & Backpressure**:
//!    - Transport commands (`SessionCommand`) travel over a bounded `mpsc` channel and never drop.
//!    - Progress updates (`PlaybackProgress`) travel over a `watch` channel and coalesce to the latest position.
//!    - Terminal, state, and error events (`SessionEvent`) travel over a bounded `mpsc` channel and never drop.
//! 3. **Generation Isolation**: Every play transition advances `PlaybackGeneration` and restarts a single
//!    `PlaybackTaskSet` cancellation tree, rejecting any stale async stream, upgrade, or auto-advance result.
//! 4. **Resilience & Precache**: Owns natural vs. premature track-end detection, per-generation recovery
//!    budgets, Hi-Fi upgrade swaps with base-stream fallback, lookahead next-track precaching, and bounded shutdown.

use crate::player::mpv::{
    clamp_playback_speed, compute_bitrate_kbps_from_bytes, format_measured_audio_badge,
    normalize_audio_codec_label, MpvController, MpvEvent,
};
use crate::process::LinuxStreamResolver;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator, PlaybackTransition};
use sonata_core::player::{
    NextTrackPrecache, PlaybackGeneration, PlaybackState, PlaybackTask, PlaybackTaskSet,
    PrecacheKey, RepeatMode,
};
use sonata_core::providers::hifi::{extract_badge_kbps, ResolvedStream};
use sonata_core::providers::{ProviderSource, Track};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, watch};

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

/// Bounded command queue depth; callers await capacity so transport commands never drop.
pub const SESSION_COMMAND_CAPACITY: usize = 64;
/// Bounded event queue depth; the session awaits capacity so terminal/error events never drop.
pub const SESSION_EVENT_CAPACITY: usize = 128;
/// Bounded internal task completion queue depth inside the session actor.
const INTERNAL_RESULT_CAPACITY: usize = 64;

/// Error returned when sending a command to a stopped [`PlaybackSession`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionClosed;

impl std::fmt::Display for SessionClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("playback session has shut down")
    }
}

impl std::error::Error for SessionClosed {}

/// Result reported when a [`PlaybackSession`] completes shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReport {
    /// `true` when all generation tasks exited cooperatively within the configured shutdown bound.
    pub tasks_joined_cleanly: bool,
}

/// Runtime policy toggles that govern Hi-Fi upgrades and lookahead precaching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    pub hifi_substitution: bool,
    pub jiosaavn_enabled: bool,
    pub cache_enabled: bool,
}

impl SessionPolicy {
    pub const fn allows_precache(self) -> bool {
        self.cache_enabled && (self.hifi_substitution || self.jiosaavn_enabled)
    }
}

/// Configuration for spawning a [`PlaybackSession`].
#[derive(Debug, Clone)]
pub struct PlaybackSessionConfig {
    pub client_name: &'static str,
    pub initial_volume: u8,
    pub initial_muted: bool,
    pub initial_shuffle: bool,
    pub initial_repeat: RepeatMode,
    pub initial_gapless: bool,
    pub initial_crossfade_secs: u8,
    pub policy: SessionPolicy,
    pub stale_error_grace: Duration,
    pub shutdown_timeout: Duration,
    pub tick_interval: Duration,
    pub position_steering: bool,
}

impl PlaybackSessionConfig {
    pub fn for_client(client_name: &'static str) -> Self {
        Self {
            client_name,
            initial_volume: 80,
            initial_muted: false,
            initial_shuffle: false,
            initial_repeat: RepeatMode::Off,
            initial_gapless: true,
            initial_crossfade_secs: 0,
            policy: SessionPolicy {
                hifi_substitution: true,
                jiosaavn_enabled: false,
                cache_enabled: true,
            },
            stale_error_grace: Duration::from_millis(750),
            shutdown_timeout: Duration::from_secs(2),
            tick_interval: Duration::from_millis(250),
            position_steering: true,
        }
    }
}

/// Coalesced playback clock and transport progress published over a `watch` channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackProgress {
    pub generation: PlaybackGeneration,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub paused: bool,
    pub loading: bool,
    pub anchor_at: Option<Instant>,
}

impl Default for PlaybackProgress {
    fn default() -> Self {
        Self {
            generation: PlaybackGeneration::default(),
            position_secs: 0.0,
            duration_secs: 0.0,
            paused: true,
            loading: false,
            anchor_at: None,
        }
    }
}

impl PlaybackProgress {
    /// Computes smoothly interpolated playback position clamped to `[0.0, duration_secs]`.
    pub fn interpolated_position(&self) -> f64 {
        let raw = match self.anchor_at {
            Some(at) if !self.paused && !self.loading => {
                self.position_secs + at.elapsed().as_secs_f64()
            }
            _ => self.position_secs,
        };
        if self.duration_secs > 0.0 {
            raw.clamp(0.0, self.duration_secs)
        } else {
            raw.max(0.0)
        }
    }
}

/// Audio backend driver interface implemented by [`MpvController`] and test doubles.
pub trait PlaybackAudioDriver: Send + Sync + 'static {
    fn load_stream(&self, stream: &ResolvedStream);
    fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64);
    fn stop(&self);
    fn set_paused(&self, paused: bool);
    fn seek_absolute(&self, secs: f64);
    fn set_volume(&self, volume: u8);
    fn set_speed(&self, _speed: f64) {}
    fn set_skip_silence(&self, _enabled: bool) {}
    fn set_gapless(&self, _enabled: bool) {}
    fn set_video_mode(
        &self,
        _enabled: bool,
        _url: Option<String>,
        _headers: Vec<(String, String)>,
        _seek_secs: Option<f64>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

impl PlaybackAudioDriver for MpvController {
    fn load_stream(&self, stream: &ResolvedStream) {
        MpvController::load_stream(self, stream);
    }

    fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64) {
        MpvController::load_stream_at(self, stream, start_secs);
    }

    fn stop(&self) {
        MpvController::stop(self);
    }

    fn set_paused(&self, paused: bool) {
        MpvController::set_paused(self, paused);
    }

    fn seek_absolute(&self, secs: f64) {
        MpvController::seek_absolute(self, secs);
    }

    fn set_volume(&self, volume: u8) {
        MpvController::set_volume(self, volume);
    }

    fn set_speed(&self, speed: f64) {
        MpvController::set_speed(self, speed);
    }

    fn set_skip_silence(&self, enabled: bool) {
        MpvController::set_skip_silence(self, enabled);
    }

    fn set_gapless(&self, enabled: bool) {
        MpvController::set_gapless(self, enabled);
    }

    fn set_video_mode(
        &self,
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            MpvController::set_video_mode(self, enabled, url, headers, seek_secs)
                .await
                .map_err(|err| err.to_string())
        })
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(MpvController::shutdown(self))
    }
}

/// No-op audio driver used when `mpv` is unavailable at startup.
#[derive(Debug, Default)]
pub struct NoopAudioDriver;

impl PlaybackAudioDriver for NoopAudioDriver {
    fn load_stream(&self, _stream: &ResolvedStream) {}
    fn load_stream_at(&self, _stream: &ResolvedStream, _start_secs: f64) {}
    fn stop(&self) {}
    fn set_paused(&self, _paused: bool) {}
    fn seek_absolute(&self, _secs: f64) {}
    fn set_volume(&self, _volume: u8) {}
    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

/// Stream resolution interface implemented by [`LinuxStreamResolver`] and test doubles.
pub trait PlaybackStreamSource: Send + Sync + 'static {
    fn resolve_fast(
        &self,
        track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>>;

    fn resolve_recovery(
        &self,
        track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>>;

    fn resolve_hifi_upgrade(
        &self,
        track: Track,
        current: ResolvedStream,
        allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>>;

    fn precache_hifi_audio(
        &self,
        track: Track,
        allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>>;

    fn needs_hifi_upgrade(&self, badge: &str, url: &str) -> bool {
        LinuxStreamResolver::needs_hifi_upgrade(badge, url)
    }
}

impl PlaybackStreamSource for LinuxStreamResolver {
    fn resolve_fast(
        &self,
        track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
        let resolver = self.clone();
        Box::pin(async move {
            resolver
                .resolve_fast(&track)
                .await
                .map_err(|err| err.to_string())
        })
    }

    fn resolve_recovery(
        &self,
        track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
        let resolver = self.clone();
        Box::pin(async move {
            resolver
                .resolve_recovery(&track)
                .await
                .map_err(|err| err.to_string())
        })
    }

    fn resolve_hifi_upgrade(
        &self,
        track: Track,
        current: ResolvedStream,
        allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        let resolver = self.clone();
        Box::pin(async move {
            resolver
                .resolve_hifi_upgrade(&track, &current, allow_hifi)
                .await
        })
    }

    fn precache_hifi_audio(
        &self,
        track: Track,
        allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        let resolver = self.clone();
        Box::pin(async move { resolver.precache_hifi_audio(&track, allow_hifi).await })
    }
}

/// Typed commands accepted by [`PlaybackSessionHandle`] over a bounded channel.
pub enum SessionCommand {
    /// Replaces the active queue and starts playback at `start_index`.
    PlayQueue {
        tracks: Vec<Track>,
        start_index: usize,
        push_history: bool,
    },
    /// Plays an existing queue entry by index.
    PlayIndex { index: usize, push_history: bool },
    /// Inserts a track directly after the current entry (or starts playing it if the queue is empty).
    PlayTrackNext(Track),
    /// Appends a track to the end of the queue.
    AddTrackToQueue(Track),
    /// Removes a non-playing entry from the queue.
    RemoveFromQueue(usize),
    /// Reorders an entry inside the queue while keeping `current_index` aligned.
    MoveQueueItem { from: usize, to: usize },
    /// Clears all upcoming/non-current entries from the queue.
    ClearQueue,
    /// Shuffles all non-current entries in the queue, moving the current track to index 0.
    ShuffleQueue,
    /// Updates a track's artwork URL inside the session queue after cover enrichment.
    UpdateTrackThumbnail { track_id: String, url: String },
    /// Sets explicit pause state.
    SetPaused(bool),
    /// Toggles pause state.
    TogglePause,
    /// Stops playback and cancels in-flight generation and precache tasks.
    Stop,
    /// Advances to the next track according to shuffle/repeat policy.
    Next,
    /// Returns to the previous track from history or index; optionally restarts current track first.
    Previous { restart_if_after_secs: Option<f64> },
    /// Seeks to an absolute timestamp clamped to `[0.0, duration]`.
    SeekAbsolute(f64),
    /// Seeks relative to the current interpolated position, clamped to `[0.0, duration]`.
    SeekRelative(f64),
    /// Sets volume (`0..=100`) and unmutes.
    SetVolume(u8),
    /// Sets explicit mute state without losing configured volume.
    SetMuted(bool),
    /// Toggles mute state.
    ToggleMute,
    /// Sets explicit shuffle mode.
    SetShuffle(bool),
    /// Toggles shuffle mode.
    ToggleShuffle,
    /// Sets explicit repeat mode.
    SetRepeat(RepeatMode),
    /// Cycles repeat mode (`Off -> All -> One -> Off`).
    CycleRepeat,
    /// Sets playback speed multiplier clamped to `0.5..=2.0`.
    SetPlaybackSpeed(f64),
    /// Enables or disables silence skipping (`af` filter; auto-disabled while video mode is active).
    SetSkipSilence(bool),
    /// Enables or disables gapless audio transitions (`gapless-audio=yes/no`).
    SetGapless(bool),
    /// Sets smooth audio crossfade ramp duration (`0..=12` seconds; automatically bypassed in Video mode).
    SetCrossfade(u8),
    /// Arms or cancels a sleep timer (`Some(secs)` fades volume smoothly over 5 seconds before pausing and restoring volume).
    SetSleepTimer(Option<u64>),
    /// Switches between audio-only (`--no-video`) and video (`--vid=auto`) playback, preserving position and headers.
    SetVideoMode {
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    },
    /// Updates Hi-Fi/JioSaavn/cache policy and optionally triggers an immediate upgrade check.
    UpdatePolicy {
        policy: SessionPolicy,
        trigger_upgrade: bool,
    },
    /// Swaps the underlying stream resolver (for example after browser sign-in refreshes cookies).
    UpdateStreamSource(Arc<dyn PlaybackStreamSource>),
    /// Cancels all tasks, waits up to `shutdown_timeout`, shuts down the audio driver, and replies.
    Shutdown(oneshot::Sender<ShutdownReport>),
}

/// Precache lifecycle notifications emitted as part of [`SessionEvent::PrecacheUpdated`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecacheNotice {
    Scheduled,
    Cancelled,
    Ready,
    Adopted,
    Failed,
}

/// Terminal, state-transition, and error events emitted by [`PlaybackSession`].
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum SessionEvent {
    QueueChanged {
        queue: Vec<Track>,
        current_index: Option<usize>,
        status: Option<String>,
    },
    TrackTransition {
        generation: PlaybackGeneration,
        index: usize,
        track: Track,
        prepared: bool,
    },
    PreparingNextTrack {
        generation: PlaybackGeneration,
        track: Track,
    },
    StreamLoaded {
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
        is_retry: bool,
        resume_at: f64,
    },
    PlaybackStarted {
        generation: PlaybackGeneration,
        track: Track,
        badge: String,
        first_audio_ms: Option<u128>,
        was_initial_start: bool,
    },
    AudioQualityUpdated {
        generation: PlaybackGeneration,
        track_id: String,
        badge: String,
        codec: Option<String>,
        bitrate_kbps: Option<u32>,
        sample_rate_hz: Option<u32>,
        bit_depth: Option<u8>,
    },
    PausedChanged {
        generation: PlaybackGeneration,
        paused: bool,
        position_secs: f64,
    },
    Seeked {
        generation: PlaybackGeneration,
        position_secs: f64,
    },
    DurationChanged {
        generation: PlaybackGeneration,
        track_id: String,
        duration_secs: f64,
        updated_track: Option<Track>,
    },
    UpgradeStarted {
        generation: PlaybackGeneration,
        track_id: String,
    },
    StreamUpgraded {
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
        resume_at: f64,
    },
    UpgradeUnavailable {
        generation: PlaybackGeneration,
        track_id: String,
    },
    UpgradeFallback {
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
        resume_at: f64,
        error: String,
    },
    RecoveryStarted {
        generation: PlaybackGeneration,
        track: Track,
        error: String,
    },
    TrackEnded {
        generation: PlaybackGeneration,
        track_id: String,
        advancing: bool,
    },
    QueueFinished {
        generation: PlaybackGeneration,
    },
    Stopped {
        generation: PlaybackGeneration,
    },
    PlaybackError {
        generation: PlaybackGeneration,
        track_id: Option<String>,
        error: String,
        badge: String,
    },
    ModeChanged {
        shuffle: bool,
        repeat: RepeatMode,
        volume: u8,
        muted: bool,
    },
    PlaybackOptionsChanged {
        playback_speed: f64,
        skip_silence: bool,
        video_mode: bool,
        sleep_timer_remaining_secs: Option<u64>,
    },
    PrecacheUpdated {
        generation: PlaybackGeneration,
        next_track_id: String,
        notice: PrecacheNotice,
    },
}

/// Cloneable client handle for commanding a running [`PlaybackSession`] and reading coalesced progress.
#[derive(Clone)]
pub struct PlaybackSessionHandle {
    cmd_tx: mpsc::Sender<SessionCommand>,
    progress_rx: watch::Receiver<PlaybackProgress>,
}

impl PlaybackSessionHandle {
    /// Sends a command without dropping, awaiting bounded channel capacity when congested.
    pub async fn send(&self, cmd: SessionCommand) -> Result<(), SessionClosed> {
        self.cmd_tx.send(cmd).await.map_err(|_| SessionClosed)
    }

    /// Returns a watch receiver for coalesced playback position/progress updates.
    pub fn progress_rx(&self) -> watch::Receiver<PlaybackProgress> {
        self.progress_rx.clone()
    }

    /// Returns the latest coalesced playback progress snapshot.
    pub fn progress(&self) -> PlaybackProgress {
        *self.progress_rx.borrow()
    }

    /// Requests bounded graceful shutdown and awaits completion of the session actor.
    pub async fn shutdown(&self) -> ShutdownReport {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(SessionCommand::Shutdown(ack_tx))
            .await
            .is_err()
        {
            return ShutdownReport {
                tasks_joined_cleanly: true,
            };
        }
        ack_rx.await.unwrap_or(ShutdownReport {
            tasks_joined_cleanly: false,
        })
    }
}

/// Spawned session bundle containing the command handle and bounded event stream.
pub struct PlaybackSession {
    pub handle: PlaybackSessionHandle,
    pub events: mpsc::Receiver<SessionEvent>,
}

impl PlaybackSession {
    /// Spawns a Linux playback session actor on the current Tokio runtime.
    pub fn spawn(
        config: PlaybackSessionConfig,
        driver: Arc<dyn PlaybackAudioDriver>,
        source: Arc<dyn PlaybackStreamSource>,
        mpv_rx: mpsc::Receiver<MpvEvent>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(SESSION_COMMAND_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(SESSION_EVENT_CAPACITY);
        let (progress_tx, progress_rx) = watch::channel(PlaybackProgress::default());

        let actor = SessionActor::new(config, driver, source, event_tx, progress_tx);
        tokio::spawn(actor.run(cmd_rx, mpv_rx));

        Self {
            handle: PlaybackSessionHandle {
                cmd_tx,
                progress_rx,
            },
            events: event_rx,
        }
    }

    /// Returns whether a reported track end occurred near the end of the track duration.
    pub fn is_natural_track_end(position_secs: f64, duration_secs: f64) -> bool {
        if duration_secs <= 0.0 {
            return false;
        }
        position_secs >= (duration_secs - 2.5).max(0.0)
    }

    /// Returns whether a track-end signal is stale relative to a recent stream load.
    pub fn should_ignore_stale_track_end(
        loading_track: bool,
        stream_loaded_at: Option<Instant>,
        grace_elapsed: Duration,
    ) -> bool {
        loading_track || stream_loaded_at.is_none_or(|loaded| loaded.elapsed() < grace_elapsed)
    }

    /// Returns whether recovery budget remains for the active playback generation.
    pub fn should_attempt_recovery(
        retried_generation: Option<PlaybackGeneration>,
        generation: PlaybackGeneration,
    ) -> bool {
        retried_generation != Some(generation)
    }
}

enum InternalResult {
    StreamReady {
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamError {
        generation: PlaybackGeneration,
        track_id: String,
        error: String,
    },
    StreamUpgrade {
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
    },
    UpgradeFinished {
        generation: PlaybackGeneration,
        track_id: String,
    },
    NextTrackPrepared {
        generation: PlaybackGeneration,
        index: usize,
        track_id: String,
        result: Result<ResolvedStream, String>,
    },
    LocalFileSize {
        generation: PlaybackGeneration,
        track_id: String,
        bytes: u64,
    },
}

#[derive(Debug, Clone, Default)]
struct AudioQualityTracker {
    track_id: String,
    base_badge: String,
    local_file_bytes: Option<u64>,
    codec: Option<String>,
    live_bitrate_bps: Option<u64>,
    sample_rate_hz: Option<u32>,
    bit_depth: Option<u8>,
    last_emitted_badge: Option<String>,
}

#[derive(Debug, Clone)]
struct SleepTimerState {
    deadline: Instant,
    fade_duration: Duration,
    pre_fade_volume: Option<u8>,
    last_applied_volume: Option<u8>,
}

#[derive(Debug, Clone)]
struct CrossfadeEntryState {
    started_at: Instant,
    duration_secs: f64,
    last_applied_volume: Option<u8>,
}

fn local_path_from_stream_url(url: &str) -> Option<PathBuf> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return None;
    }
    let path_str = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    (!path_str.is_empty()).then(|| PathBuf::from(path_str))
}

struct SessionActor {
    config: PlaybackSessionConfig,
    driver: Arc<dyn PlaybackAudioDriver>,
    source: Arc<dyn PlaybackStreamSource>,
    event_tx: mpsc::Sender<SessionEvent>,
    progress_tx: watch::Sender<PlaybackProgress>,
    internal_tx: mpsc::Sender<InternalResult>,
    internal_rx: mpsc::Receiver<InternalResult>,

    playback: PlaybackState,
    muted: bool,
    playback_speed: f64,
    skip_silence_pref: bool,
    gapless_enabled: bool,
    crossfade_secs: u8,
    crossfade_exit_active: bool,
    crossfade_pending_entry: bool,
    crossfade_last_exit_vol: Option<u8>,
    crossfade_entry: Option<CrossfadeEntryState>,
    video_mode: bool,
    sleep_timer: Option<SleepTimerState>,
    audio_quality: AudioQualityTracker,
    generation: PlaybackGeneration,
    retried_generation: Option<PlaybackGeneration>,
    upgrade_started_generation: Option<PlaybackGeneration>,
    upgrading_quality: bool,
    awaiting_playback_start: bool,
    stream_loaded_at: Option<Instant>,
    play_requested_at: Option<Instant>,
    pos_anchor: Option<(f64, Instant)>,

    playback_tasks: PlaybackTaskSet,
    next_precache: NextTrackPrecache,
    next_prefetch_task: Option<(PrecacheKey, PlaybackTask<Option<ResolvedStream>>)>,
    adopted_prefetch_task: Option<(String, PlaybackTask<Option<ResolvedStream>>)>,
    pending_auto_advance: Option<(PlaybackGeneration, String)>,
}

impl SessionActor {
    fn new(
        config: PlaybackSessionConfig,
        driver: Arc<dyn PlaybackAudioDriver>,
        source: Arc<dyn PlaybackStreamSource>,
        event_tx: mpsc::Sender<SessionEvent>,
        progress_tx: watch::Sender<PlaybackProgress>,
    ) -> Self {
        let mut playback = PlaybackState::new(config.initial_volume.min(100));
        playback.shuffle = config.initial_shuffle;
        playback.repeat = config.initial_repeat;
        let muted = config.initial_muted;
        let gapless_enabled = config.initial_gapless;
        let crossfade_secs = clamp_crossfade_secs(config.initial_crossfade_secs);
        let (internal_tx, internal_rx) = mpsc::channel(INTERNAL_RESULT_CAPACITY);

        Self {
            config,
            driver,
            source,
            event_tx,
            progress_tx,
            internal_tx,
            internal_rx,
            playback,
            muted,
            playback_speed: 1.0,
            skip_silence_pref: false,
            gapless_enabled,
            crossfade_secs,
            crossfade_exit_active: false,
            crossfade_pending_entry: false,
            crossfade_last_exit_vol: None,
            crossfade_entry: None,
            video_mode: false,
            sleep_timer: None,
            audio_quality: AudioQualityTracker::default(),
            generation: PlaybackGeneration::default(),
            retried_generation: None,
            upgrade_started_generation: None,
            upgrading_quality: false,
            awaiting_playback_start: false,
            stream_loaded_at: None,
            play_requested_at: None,
            pos_anchor: None,
            playback_tasks: PlaybackTaskSet::default(),
            next_precache: NextTrackPrecache::default(),
            next_prefetch_task: None,
            adopted_prefetch_task: None,
            pending_auto_advance: None,
        }
    }

    async fn run(
        mut self,
        mut cmd_rx: mpsc::Receiver<SessionCommand>,
        mut mpv_rx: mpsc::Receiver<MpvEvent>,
    ) {
        let mut tick = tokio::time::interval(self.config.tick_interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let active_play = !self.playback.paused
                && !self.playback.loading_track
                && self.playback.current_track().is_some();
            let has_sleep_timer = self.sleep_timer.is_some();

            tokio::select! {
                biased;

                cmd = cmd_rx.recv() => {
                    let Some(cmd) = cmd else {
                        let _ = self.shutdown().await;
                        break;
                    };
                    if let SessionCommand::Shutdown(ack) = cmd {
                        let report = self.shutdown().await;
                        let _ = ack.send(report);
                        break;
                    }
                    self.handle_command(cmd).await;
                }
                Some(res) = self.internal_rx.recv() => {
                    self.handle_internal_result(res).await;
                }
                Some(mev) = mpv_rx.recv() => {
                    self.handle_mpv_event(mev).await;
                }
                _ = tick.tick(), if active_play || has_sleep_timer => {
                    if active_play {
                        self.tick_playback().await;
                    }
                    if self.sleep_timer.is_some() {
                        self.tick_sleep_timer().await;
                    }
                }
            }
        }
    }

    async fn emit(&self, event: SessionEvent) {
        let _ = self.event_tx.send(event).await;
    }

    fn publish_progress(&self) {
        let (position_secs, anchor_at) = match self.pos_anchor {
            Some((base, at)) if !self.playback.paused && !self.playback.loading_track => {
                (base, Some(at))
            }
            _ => (self.playback.position_secs, None),
        };
        self.progress_tx.send_replace(PlaybackProgress {
            generation: self.generation,
            position_secs,
            duration_secs: self.playback.duration_secs,
            paused: self.playback.paused,
            loading: self.playback.loading_track,
            anchor_at,
        });
    }

    fn interpolated_position(&self) -> f64 {
        let raw = match self.pos_anchor {
            Some((base, at)) if !self.playback.paused && !self.playback.loading_track => {
                base + at.elapsed().as_secs_f64() * self.playback_speed
            }
            _ => self.playback.position_secs,
        };
        if self.playback.duration_secs > 0.0 {
            raw.clamp(0.0, self.playback.duration_secs)
        } else {
            raw.max(0.0)
        }
    }

    async fn handle_command(&mut self, cmd: SessionCommand) {
        match cmd {
            SessionCommand::PlayQueue {
                tracks,
                start_index,
                push_history,
            } => {
                if tracks.is_empty() {
                    return;
                }
                PlaybackCoordinator::apply(
                    &mut self.playback,
                    PlaybackCommand::QueueChanged(tracks),
                );
                let idx = start_index.min(self.playback.queue.len() - 1);
                self.emit(SessionEvent::QueueChanged {
                    queue: self.playback.queue.clone(),
                    current_index: self.playback.current_index,
                    status: None,
                })
                .await;
                self.play_index_with_stream(idx, push_history, None).await;
            }
            SessionCommand::PlayIndex {
                index,
                push_history,
            } => {
                if index < self.playback.queue.len() {
                    self.play_index_with_stream(index, push_history, None).await;
                }
            }
            SessionCommand::PlayTrackNext(track) => {
                let title = track.title.clone();
                if self.playback.queue.is_empty() {
                    self.playback.queue.push(track);
                    self.emit(SessionEvent::QueueChanged {
                        queue: self.playback.queue.clone(),
                        current_index: self.playback.current_index,
                        status: Some(format!("Playing next: {title}")),
                    })
                    .await;
                    self.play_index_with_stream(0, false, None).await;
                } else {
                    let insert_at = self
                        .playback
                        .current_index
                        .map(|i| (i + 1).min(self.playback.queue.len()))
                        .unwrap_or(0);
                    self.playback.queue.insert(insert_at, track);
                    self.playback.clear_upcoming();
                    self.emit(SessionEvent::QueueChanged {
                        queue: self.playback.queue.clone(),
                        current_index: self.playback.current_index,
                        status: Some(format!("Playing next: {title}")),
                    })
                    .await;
                    self.maybe_precache_next().await;
                }
            }
            SessionCommand::AddTrackToQueue(track) => {
                let title = track.title.clone();
                self.playback.queue.push(track);
                self.playback.clear_upcoming();
                self.emit(SessionEvent::QueueChanged {
                    queue: self.playback.queue.clone(),
                    current_index: self.playback.current_index,
                    status: Some(format!("Added to queue: {title}")),
                })
                .await;
                self.maybe_precache_next().await;
            }
            SessionCommand::RemoveFromQueue(idx) => {
                if idx < self.playback.queue.len() && Some(idx) != self.playback.current_index {
                    self.playback.queue.remove(idx);
                    if let Some(cur) = self.playback.current_index {
                        if idx < cur {
                            self.playback.current_index = Some(cur - 1);
                        }
                    }
                    self.playback.clear_upcoming();
                    self.emit(SessionEvent::QueueChanged {
                        queue: self.playback.queue.clone(),
                        current_index: self.playback.current_index,
                        status: None,
                    })
                    .await;
                    self.maybe_precache_next().await;
                }
            }
            SessionCommand::MoveQueueItem { from, to } => {
                let len = self.playback.queue.len();
                if from < len && to < len && from != to {
                    let cur_id = self.playback.current_track().map(|t| t.id.clone());
                    let item = self.playback.queue.remove(from);
                    self.playback.queue.insert(to, item);
                    if let Some(cid) = cur_id {
                        self.playback.current_index =
                            self.playback.queue.iter().position(|t| t.id == cid);
                    }
                    self.playback.clear_upcoming();
                    self.emit(SessionEvent::QueueChanged {
                        queue: self.playback.queue.clone(),
                        current_index: self.playback.current_index,
                        status: None,
                    })
                    .await;
                    self.maybe_precache_next().await;
                }
            }
            SessionCommand::ClearQueue => {
                if let Some(cur_idx) = self.playback.current_index {
                    if let Some(cur) = self.playback.queue.get(cur_idx).cloned() {
                        self.playback.queue = vec![cur];
                        self.playback.current_index = Some(0);
                    } else {
                        self.playback.queue.clear();
                        self.playback.current_index = None;
                    }
                } else {
                    self.playback.queue.clear();
                }
                self.playback.clear_upcoming();
                self.emit(SessionEvent::QueueChanged {
                    queue: self.playback.queue.clone(),
                    current_index: self.playback.current_index,
                    status: Some("Cleared upcoming queue".to_string()),
                })
                .await;
                self.maybe_precache_next().await;
            }
            SessionCommand::ShuffleQueue => {
                let len = self.playback.queue.len();
                if len > 2 {
                    let cur = self.playback.current_index;
                    let cur_track = cur.map(|i| self.playback.queue.remove(i));
                    let mut seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(17);
                    for i in (1..self.playback.queue.len()).rev() {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let j = seed % (i + 1);
                        self.playback.queue.swap(i, j);
                    }
                    if let Some(t) = cur_track {
                        self.playback.queue.insert(0, t);
                        self.playback.current_index = Some(0);
                    }
                    self.playback.clear_upcoming();
                    self.emit(SessionEvent::QueueChanged {
                        queue: self.playback.queue.clone(),
                        current_index: self.playback.current_index,
                        status: Some("Shuffled queue".to_string()),
                    })
                    .await;
                    self.maybe_precache_next().await;
                }
            }
            SessionCommand::UpdateTrackThumbnail { track_id, url } => {
                if let Some(track) = self
                    .playback
                    .queue
                    .iter_mut()
                    .find(|track| track.id == track_id)
                {
                    track.thumbnail_url = Some(url);
                }
            }
            SessionCommand::SetPaused(paused) => {
                self.set_paused(paused).await;
            }
            SessionCommand::TogglePause => {
                self.set_paused(!self.playback.paused).await;
            }
            SessionCommand::Stop => {
                self.stop_playback().await;
            }
            SessionCommand::Next => {
                if let Some(transition) =
                    PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Next)
                {
                    self.start_play_transition(transition, None).await;
                } else {
                    self.cancel_crossfade(true);
                    self.playback.paused = true;
                    self.pos_anchor = None;
                    self.publish_progress();
                    self.emit(SessionEvent::PausedChanged {
                        generation: self.generation,
                        paused: true,
                        position_secs: self.playback.position_secs,
                    })
                    .await;
                }
            }
            SessionCommand::Previous {
                restart_if_after_secs,
            } => {
                if let Some(threshold) = restart_if_after_secs {
                    if self.interpolated_position() > threshold {
                        self.seek_absolute(0.0).await;
                        return;
                    }
                }
                if let Some(transition) =
                    PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Previous)
                {
                    self.start_play_transition(transition, None).await;
                }
            }
            SessionCommand::SeekAbsolute(secs) => {
                self.seek_absolute(secs).await;
            }
            SessionCommand::SeekRelative(delta) => {
                let target = self.interpolated_position() + delta;
                self.seek_absolute(target).await;
            }
            SessionCommand::SetVolume(vol) => {
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
            SessionCommand::SetMuted(muted) => {
                self.muted = muted;
                if muted {
                    self.cancel_crossfade(false);
                }
                let effective = if self.muted { 0 } else { self.playback.volume };
                self.driver.set_volume(effective);
                self.emit_mode_changed().await;
            }
            SessionCommand::ToggleMute => {
                self.muted = !self.muted;
                if self.muted {
                    self.cancel_crossfade(false);
                }
                let effective = if self.muted { 0 } else { self.playback.volume };
                self.driver.set_volume(effective);
                self.emit_mode_changed().await;
            }
            SessionCommand::SetShuffle(on) => {
                self.playback.shuffle = on;
                self.playback.clear_upcoming();
                self.emit_mode_changed().await;
                self.maybe_precache_next().await;
            }
            SessionCommand::ToggleShuffle => {
                self.playback.shuffle = !self.playback.shuffle;
                self.playback.clear_upcoming();
                self.emit_mode_changed().await;
                self.maybe_precache_next().await;
            }
            SessionCommand::SetRepeat(mode) => {
                self.playback.repeat = mode;
                self.playback.clear_upcoming();
                self.emit_mode_changed().await;
                self.maybe_precache_next().await;
            }
            SessionCommand::CycleRepeat => {
                self.playback.repeat = self.playback.repeat.next();
                self.playback.clear_upcoming();
                self.emit_mode_changed().await;
                self.maybe_precache_next().await;
            }
            SessionCommand::SetPlaybackSpeed(speed) => {
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
            SessionCommand::SetSkipSilence(enabled) => {
                self.skip_silence_pref = enabled;
                let effective = self.effective_skip_silence();
                self.driver.set_skip_silence(effective);
                self.emit_playback_options_changed().await;
            }
            SessionCommand::SetGapless(enabled) => {
                self.gapless_enabled = enabled;
                self.driver.set_gapless(enabled);
            }
            SessionCommand::SetCrossfade(secs) => {
                self.crossfade_secs = clamp_crossfade_secs(secs);
                if self.effective_crossfade_secs() == 0 {
                    self.cancel_crossfade(true);
                }
            }
            SessionCommand::SetSleepTimer(seconds) => {
                self.set_sleep_timer(seconds).await;
            }
            SessionCommand::SetVideoMode {
                enabled,
                url,
                headers,
                seek_secs,
            } => {
                self.set_video_mode(enabled, url, headers, seek_secs).await;
            }
            SessionCommand::UpdatePolicy {
                policy,
                trigger_upgrade,
            } => {
                self.config.policy = policy;
                if trigger_upgrade {
                    self.start_hifi_upgrade().await;
                }
                self.maybe_precache_next().await;
            }
            SessionCommand::UpdateStreamSource(source) => {
                self.source = source;
            }
            SessionCommand::Shutdown(_) => {}
        }
    }

    fn effective_skip_silence(&self) -> bool {
        self.skip_silence_pref && !self.video_mode
    }

    fn effective_crossfade_secs(&self) -> u8 {
        if self.video_mode {
            0
        } else {
            self.crossfade_secs
        }
    }

    fn sleep_timer_fading(&self) -> bool {
        self.sleep_timer
            .as_ref()
            .is_some_and(|t| t.pre_fade_volume.is_some())
    }

    fn cancel_crossfade(&mut self, restore_volume: bool) {
        let was_fading = self.crossfade_exit_active || self.crossfade_entry.is_some();
        self.crossfade_exit_active = false;
        self.crossfade_pending_entry = false;
        self.crossfade_last_exit_vol = None;
        self.crossfade_entry = None;
        if restore_volume && was_fading && !self.muted && !self.sleep_timer_fading() {
            self.driver.set_volume(self.playback.volume);
        }
    }

    fn tick_crossfade(&mut self) {
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

    fn sleep_timer_remaining_secs(&self) -> Option<u64> {
        self.sleep_timer.as_ref().map(|timer| {
            let rem = timer.deadline.saturating_duration_since(Instant::now());
            rem.as_secs() + u64::from(rem.subsec_nanos() > 0)
        })
    }

    async fn emit_mode_changed(&self) {
        self.emit(SessionEvent::ModeChanged {
            shuffle: self.playback.shuffle,
            repeat: self.playback.repeat,
            volume: self.playback.volume,
            muted: self.muted,
        })
        .await;
    }

    async fn emit_playback_options_changed(&self) {
        self.emit(SessionEvent::PlaybackOptionsChanged {
            playback_speed: self.playback_speed,
            skip_silence: self.effective_skip_silence(),
            video_mode: self.video_mode,
            sleep_timer_remaining_secs: self.sleep_timer_remaining_secs(),
        })
        .await;
    }

    async fn set_sleep_timer(&mut self, seconds: Option<u64>) {
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

    async fn tick_sleep_timer(&mut self) {
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

    async fn set_video_mode(
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

    fn begin_audio_quality_tracking(
        &mut self,
        generation: PlaybackGeneration,
        track_id: &str,
        stream: &ResolvedStream,
    ) {
        self.audio_quality = AudioQualityTracker {
            track_id: track_id.to_string(),
            base_badge: stream.badge.clone(),
            ..AudioQualityTracker::default()
        };
        if let Some(local_path) = local_path_from_stream_url(&stream.url) {
            let tx = self.internal_tx.clone();
            let owned_track_id = track_id.to_string();
            self.playback_tasks.spawn(async move {
                let bytes = tokio::task::spawn_blocking(move || {
                    std::fs::metadata(&local_path)
                        .ok()
                        .map(|m| m.len())
                        .filter(|&n| n > 0)
                })
                .await
                .ok()
                .flatten();
                if let Some(bytes) = bytes {
                    let _ = tx
                        .send(InternalResult::LocalFileSize {
                            generation,
                            track_id: owned_track_id,
                            bytes,
                        })
                        .await;
                }
            });
        }
    }

    async fn refresh_measured_audio_quality(&mut self) {
        if self.audio_quality.track_id.is_empty() || self.playback.loading_track {
            return;
        }
        let live_kbps = self.audio_quality.live_bitrate_bps.and_then(|bps| {
            let kbps = ((bps as f64) / 1000.0).round() as u32;
            (kbps > 0).then_some(kbps)
        });
        let local_kbps = self
            .audio_quality
            .local_file_bytes
            .and_then(|bytes| compute_bitrate_kbps_from_bytes(bytes, self.playback.duration_secs));
        let fallback_kbps = extract_badge_kbps(&self.audio_quality.base_badge);
        let measured_kbps = live_kbps.or(local_kbps).or(fallback_kbps);

        let Some(badge) = format_measured_audio_badge(
            self.audio_quality.codec.as_deref(),
            measured_kbps,
            self.audio_quality.bit_depth,
            self.audio_quality.sample_rate_hz,
            Some(&self.audio_quality.base_badge),
        ) else {
            return;
        };

        if self.audio_quality.last_emitted_badge.as_deref() == Some(badge.as_str()) {
            return;
        }
        self.audio_quality.last_emitted_badge = Some(badge.clone());
        self.playback.stream_badge = badge.clone();

        let codec = self
            .audio_quality
            .codec
            .as_deref()
            .map(normalize_audio_codec_label);
        let track_id = self.audio_quality.track_id.clone();
        let sample_rate_hz = self.audio_quality.sample_rate_hz;
        let bit_depth = self.audio_quality.bit_depth;

        self.emit(SessionEvent::AudioQualityUpdated {
            generation: self.generation,
            track_id,
            badge,
            codec,
            bitrate_kbps: measured_kbps,
            sample_rate_hz,
            bit_depth,
        })
        .await;
    }

    async fn set_paused(&mut self, paused: bool) {
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

    async fn seek_absolute(&mut self, secs: f64) {
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

    async fn stop_playback(&mut self) {
        self.generation.advance();
        self.playback_tasks.restart();
        self.cancel_crossfade(true);
        if let Some((key, task)) = self.next_prefetch_task.take() {
            task.abort();
            self.emit(SessionEvent::PrecacheUpdated {
                generation: self.generation,
                next_track_id: key.next_track_id,
                notice: PrecacheNotice::Cancelled,
            })
            .await;
        }
        if let Some((_, task)) = self.adopted_prefetch_task.take() {
            task.abort();
        }
        self.next_precache.take();
        self.pending_auto_advance = None;
        self.awaiting_playback_start = false;
        self.stream_loaded_at = None;
        self.upgrade_started_generation = None;
        self.upgrading_quality = false;
        self.audio_quality = AudioQualityTracker::default();
        self.playback.paused = true;
        self.playback.loading_track = false;
        self.playback.position_secs = 0.0;
        self.playback.stream_badge = "Stopped".to_string();
        self.pos_anchor = None;
        self.driver.stop();
        self.publish_progress();
        self.emit(SessionEvent::Stopped {
            generation: self.generation,
        })
        .await;
    }

    async fn play_index_with_stream(
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
        self.start_play_transition(transition, prepared_stream)
            .await;
    }

    async fn start_play_transition(
        &mut self,
        transition: PlaybackTransition,
        prepared_stream: Option<ResolvedStream>,
    ) {
        let PlaybackTransition::Play { index, track, .. } = transition else {
            return;
        };
        self.playback_tasks.restart();
        if let Some((key, task)) = self.next_prefetch_task.take() {
            task.abort();
            self.emit(SessionEvent::PrecacheUpdated {
                generation: self.generation,
                next_track_id: key.next_track_id,
                notice: PrecacheNotice::Cancelled,
            })
            .await;
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
        self.pos_anchor = None;
        self.stream_loaded_at = None;
        self.retried_generation = None;
        self.upgrade_started_generation = None;
        self.upgrading_quality = false;
        self.awaiting_playback_start = false;
        self.pending_auto_advance = None;
        self.audio_quality = AudioQualityTracker::default();
        if !self.crossfade_pending_entry {
            self.cancel_crossfade(true);
        }
        self.crossfade_exit_active = false;
        self.crossfade_last_exit_vol = None;
        self.driver.stop();

        let generation = self.generation.advance();
        if adopted_for_track {
            self.upgrade_started_generation = Some(generation);
        }
        self.play_requested_at = Some(Instant::now());

        tracing::info!(
            event = "resolving",
            client = self.config.client_name,
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq = generation.value(),
            prepared = prepared_stream.is_some(),
            "resolving playback stream"
        );

        self.publish_progress();
        self.emit(SessionEvent::TrackTransition {
            generation,
            index,
            track: track.clone(),
            prepared: prepared_stream.is_some(),
        })
        .await;

        let tx = self.internal_tx.clone();
        if let Some(stream) = prepared_stream {
            self.apply_stream_ready(generation, track.id, stream).await;
        } else {
            let source = Arc::clone(&self.source);
            let track_id = track.id.clone();
            self.playback_tasks.spawn(async move {
                match source.resolve_fast(track).await {
                    Ok(stream) => {
                        let _ = tx
                            .send(InternalResult::StreamReady {
                                generation,
                                track_id,
                                stream,
                            })
                            .await;
                    }
                    Err(error) => {
                        let _ = tx
                            .send(InternalResult::StreamError {
                                generation,
                                track_id,
                                error,
                            })
                            .await;
                    }
                }
            });
        }
    }

    async fn apply_stream_ready(
        &mut self,
        generation: PlaybackGeneration,
        track_id: String,
        stream: ResolvedStream,
    ) {
        if generation != self.generation {
            return;
        }
        let Some(track) = self
            .playback
            .current_track()
            .filter(|t| t.id == track_id)
            .cloned()
        else {
            return;
        };
        let is_retry = self.retried_generation == Some(generation);
        tracing::info!(
            event = "stream_ready",
            client = self.config.client_name,
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq = generation.value(),
            attempt = if is_retry { "retry" } else { "initial" },
            transport = if stream.url.starts_with("http") { "remote" } else { "local" },
            quality = %stream.badge,
            resolve_ms = self.play_requested_at.map_or(0, |at| at.elapsed().as_millis()),
            "stream resolved"
        );
        self.playback.set_active_stream(stream.clone());
        self.playback.loading_track = false;
        self.upgrading_quality = false;
        self.playback.paused = true;
        self.awaiting_playback_start = true;
        let resume_at = if is_retry {
            self.playback.position_secs
        } else {
            0.0
        };
        self.playback.position_secs = resume_at;
        self.playback.stream_badge = stream.badge.clone();
        self.pos_anchor = None;
        self.stream_loaded_at = Some(Instant::now());
        self.begin_audio_quality_tracking(generation, &track_id, &stream);
        if resume_at >= 0.05 {
            self.driver.load_stream_at(&stream, resume_at);
        } else {
            self.driver.load_stream(&stream);
        }
        self.driver.set_paused(false);
        self.publish_progress();
        self.emit(SessionEvent::StreamLoaded {
            generation,
            track_id,
            stream,
            is_retry,
            resume_at,
        })
        .await;
    }

    async fn handle_internal_result(&mut self, res: InternalResult) {
        match res {
            InternalResult::StreamReady {
                generation,
                track_id,
                stream,
            } => {
                self.apply_stream_ready(generation, track_id, stream).await;
            }
            InternalResult::StreamError {
                generation,
                track_id,
                error,
            } => {
                if generation != self.generation {
                    return;
                }
                let Some(track) = self
                    .playback
                    .current_track()
                    .filter(|t| t.id == track_id)
                    .cloned()
                else {
                    return;
                };
                tracing::error!(
                    event = "resolve_failed",
                    client = self.config.client_name,
                    track_id = %track.id,
                    title = %track.title,
                    artist = %track.artist,
                    provider = ?track.source,
                    play_seq = generation.value(),
                    error = %error,
                    "stream resolution failed"
                );
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.paused = true;
                self.awaiting_playback_start = false;
                self.pos_anchor = None;
                self.playback.stream_badge = "Stream Error".to_string();
                self.driver.set_paused(true);
                self.publish_progress();
                self.emit(SessionEvent::PlaybackError {
                    generation,
                    track_id: Some(track_id),
                    error,
                    badge: self.playback.stream_badge.clone(),
                })
                .await;
            }
            InternalResult::StreamUpgrade {
                generation,
                track_id,
                stream,
            } => {
                if generation != self.generation {
                    return;
                }
                self.upgrading_quality = false;
                if !self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    return;
                }
                if !self.playback.set_upgraded_stream(stream.clone()) {
                    return;
                }
                let resume_at = self.interpolated_position();
                let was_paused = self.playback.paused;
                self.playback.position_secs = resume_at;
                self.stream_loaded_at = Some(Instant::now());
                if !was_paused {
                    self.pos_anchor = Some((resume_at, Instant::now()));
                } else {
                    self.pos_anchor = None;
                }
                self.playback.stream_badge = stream.badge.clone();
                self.begin_audio_quality_tracking(generation, &track_id, &stream);
                tracing::info!(
                    event = "quality_upgraded",
                    client = self.config.client_name,
                    track_id = %track_id,
                    play_seq = generation.value(),
                    quality = %stream.badge,
                    resume_secs = resume_at,
                    "audio quality upgraded"
                );
                self.driver.load_stream_at(&stream, resume_at);
                self.driver.set_paused(was_paused);
                self.publish_progress();
                self.emit(SessionEvent::StreamUpgraded {
                    generation,
                    track_id,
                    stream,
                    resume_at,
                })
                .await;
            }
            InternalResult::UpgradeFinished {
                generation,
                track_id,
            } => {
                if generation == self.generation {
                    self.upgrading_quality = false;
                    self.emit(SessionEvent::UpgradeUnavailable {
                        generation,
                        track_id,
                    })
                    .await;
                }
            }
            InternalResult::NextTrackPrepared {
                generation,
                index,
                track_id,
                result,
            } => {
                if generation != self.generation
                    || self
                        .pending_auto_advance
                        .as_ref()
                        .is_none_or(|(gen, id)| *gen != generation || id != &track_id)
                {
                    return;
                }
                self.pending_auto_advance = None;
                if !self
                    .playback
                    .queue
                    .get(index)
                    .is_some_and(|track| track.id == track_id)
                {
                    self.playback.loading_track = false;
                    self.publish_progress();
                    return;
                }
                match result {
                    Ok(stream) => {
                        self.play_index_with_stream(index, true, Some(stream)).await;
                    }
                    Err(error) => {
                        self.playback.loading_track = false;
                        self.playback.paused = true;
                        self.playback.stream_badge = "Error".to_string();
                        self.publish_progress();
                        self.emit(SessionEvent::PlaybackError {
                            generation,
                            track_id: Some(track_id),
                            error: format!("Could not prepare next track: {error}"),
                            badge: self.playback.stream_badge.clone(),
                        })
                        .await;
                    }
                }
            }
            InternalResult::LocalFileSize {
                generation,
                track_id,
                bytes,
            } => {
                if generation == self.generation && self.audio_quality.track_id == track_id {
                    self.audio_quality.local_file_bytes = Some(bytes);
                    self.refresh_measured_audio_quality().await;
                }
            }
        }
    }

    async fn handle_mpv_event(&mut self, ev: MpvEvent) {
        match ev {
            MpvEvent::Position(pos) => {
                if self.playback.loading_track || self.awaiting_playback_start {
                    return;
                }
                if !self.playback.paused {
                    let next_pos = if self.config.position_steering {
                        let current = self.interpolated_position();
                        let err = pos - current;
                        if err.abs() > 0.35 || self.pos_anchor.is_none() {
                            pos
                        } else {
                            (current + err * 0.25).max(0.0)
                        }
                    } else {
                        pos
                    };
                    self.playback.position_secs = next_pos;
                    self.pos_anchor = Some((next_pos, Instant::now()));
                } else {
                    self.playback.position_secs = pos;
                }
                self.publish_progress();
                self.poll_adopted_precache().await;
                self.maybe_precache_next().await;
            }
            MpvEvent::Duration(dur) => {
                if self.playback.loading_track || dur <= 1.0 {
                    return;
                }
                self.playback.duration_secs = dur;
                let dur_u64 = dur.round() as u64;
                let mut updated_track = None;
                let mut track_id = String::new();
                if let Some(track) = self
                    .playback
                    .current_index
                    .and_then(|idx| self.playback.queue.get_mut(idx))
                {
                    track_id = track.id.clone();
                    if track
                        .duration_secs
                        .is_none_or(|prev| prev.abs_diff(dur_u64) > 3)
                    {
                        track.duration_secs = Some(dur_u64);
                        updated_track = Some(track.clone());
                    }
                }
                self.publish_progress();
                if !track_id.is_empty() {
                    self.emit(SessionEvent::DurationChanged {
                        generation: self.generation,
                        track_id,
                        duration_secs: dur,
                        updated_track,
                    })
                    .await;
                }
                self.refresh_measured_audio_quality().await;
            }
            MpvEvent::Paused(paused) => {
                if self.playback.current_track().is_none() {
                    if !self.playback.paused || self.pos_anchor.is_some() {
                        self.playback.paused = true;
                        self.pos_anchor = None;
                        self.publish_progress();
                    }
                    return;
                }
                if self.awaiting_playback_start {
                    if paused {
                        self.playback.paused = true;
                        self.publish_progress();
                    }
                    return;
                }
                if self.playback.loading_track {
                    return;
                }
                if paused {
                    self.playback.position_secs = self.interpolated_position();
                    self.pos_anchor = None;
                } else {
                    self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                }
                self.playback.paused = paused;
                self.publish_progress();
                self.emit(SessionEvent::PausedChanged {
                    generation: self.generation,
                    paused,
                    position_secs: self.playback.position_secs,
                })
                .await;
            }
            MpvEvent::PlaybackStarted => {
                let was_initial_start = self.awaiting_playback_start;
                self.awaiting_playback_start = false;
                self.playback.paused = false;
                self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                if was_initial_start && self.crossfade_pending_entry {
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
                self.publish_progress();

                let first_audio_ms = if was_initial_start {
                    self.play_requested_at
                        .take()
                        .map(|at| at.elapsed().as_millis())
                } else {
                    None
                };

                if let Some(track) = self.playback.current_track().cloned() {
                    if was_initial_start {
                        tracing::info!(
                            event = "playing",
                            client = self.config.client_name,
                            track_id = %track.id,
                            title = %track.title,
                            artist = %track.artist,
                            provider = ?track.source,
                            play_seq = self.generation.value(),
                            quality = %self.playback.stream_badge,
                            first_audio_ms = first_audio_ms.unwrap_or(0),
                            pending_tasks = self.playback_tasks.pending_count(),
                            "playback started"
                        );
                    }
                    self.emit(SessionEvent::PlaybackStarted {
                        generation: self.generation,
                        track,
                        badge: self.playback.stream_badge.clone(),
                        first_audio_ms,
                        was_initial_start,
                    })
                    .await;
                }
                self.start_hifi_upgrade().await;
            }
            MpvEvent::TrackEnded => {
                let pos = self.interpolated_position();
                self.pos_anchor = None;
                if PlaybackSession::should_ignore_stale_track_end(
                    self.playback.loading_track,
                    self.stream_loaded_at,
                    self.config.stale_error_grace,
                ) {
                    tracing::debug!(
                        event = "stale_track_end",
                        client = self.config.client_name,
                        play_seq = self.generation.value(),
                        loading = self.playback.loading_track,
                        stream_loaded = self.stream_loaded_at.is_some(),
                        "ignored stale track-end event"
                    );
                    return;
                }
                if !PlaybackSession::is_natural_track_end(pos, self.playback.duration_secs) {
                    let is_remote = self
                        .playback
                        .active_stream
                        .as_ref()
                        .is_some_and(|s| s.url.starts_with("http"));
                    tracing::warn!(
                        event = "premature_track_end",
                        client = self.config.client_name,
                        track_id = self
                            .playback
                            .current_track()
                            .map(|t| t.id.as_str())
                            .unwrap_or("-"),
                        play_seq = self.generation.value(),
                        position_secs = pos,
                        duration_secs = self.playback.duration_secs,
                        remote = is_remote,
                        "mpv reported premature EOF before track end; recovering at current position"
                    );
                    if is_remote && self.playback.duration_secs > 0.0 {
                        self.playback.position_secs = pos;
                        let active_url =
                            self.playback.active_stream.as_ref().map(|s| s.url.clone());
                        self.handle_playback_error(
                            "stream interrupted while seeking".to_string(),
                            active_url,
                        )
                        .await;
                    }
                    return;
                }

                let track_id = self
                    .playback
                    .current_track()
                    .map(|t| t.id.clone())
                    .unwrap_or_default();
                if !track_id.is_empty() {
                    tracing::info!(
                        event = "track_ended",
                        client = self.config.client_name,
                        track_id = %track_id,
                        play_seq = self.generation.value(),
                        position_secs = self.playback.position_secs,
                        duration_secs = self.playback.duration_secs,
                        "track ended"
                    );
                }

                if let Some(next_idx) = self.playback.peek_next_index() {
                    self.emit(SessionEvent::TrackEnded {
                        generation: self.generation,
                        track_id,
                        advancing: true,
                    })
                    .await;
                    self.prepare_next_before_advance(next_idx).await;
                } else {
                    self.cancel_crossfade(true);
                    self.playback.paused = true;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    self.publish_progress();
                    self.emit(SessionEvent::TrackEnded {
                        generation: self.generation,
                        track_id,
                        advancing: false,
                    })
                    .await;
                    self.emit(SessionEvent::QueueFinished {
                        generation: self.generation,
                    })
                    .await;
                }
            }
            MpvEvent::PlaybackError { error, file } => {
                self.handle_playback_error(error, file).await;
            }
            MpvEvent::AudioBitrate(bps) => {
                if self.playback.loading_track || self.playback.current_track().is_none() {
                    return;
                }
                self.audio_quality.live_bitrate_bps = bps;
                self.refresh_measured_audio_quality().await;
            }
            MpvEvent::AudioCodec(codec) => {
                if self.playback.loading_track || self.playback.current_track().is_none() {
                    return;
                }
                self.audio_quality.codec = codec;
                self.refresh_measured_audio_quality().await;
            }
            MpvEvent::AudioParams(params) => {
                if self.playback.loading_track || self.playback.current_track().is_none() {
                    return;
                }
                match params {
                    Some(p) => {
                        self.audio_quality.sample_rate_hz = p.sample_rate_hz;
                        self.audio_quality.bit_depth = p.bit_depth;
                    }
                    None => {
                        self.audio_quality.sample_rate_hz = None;
                        self.audio_quality.bit_depth = None;
                    }
                }
                self.refresh_measured_audio_quality().await;
            }
        }
    }

    async fn handle_playback_error(&mut self, err: String, file: Option<String>) {
        tracing::warn!(
            event = "playback_error",
            client = self.config.client_name,
            track_id = self
                .playback
                .current_track()
                .map(|track| track.id.as_str())
                .unwrap_or("-"),
            play_seq = self.generation.value(),
            error = %err,
            "mpv could not continue playback"
        );
        if let (Some(file), Some(active)) = (file.as_deref(), self.playback.active_stream.as_ref())
        {
            if active.url != file {
                tracing::debug!(
                    event = "stale_playback_error",
                    client = self.config.client_name,
                    play_seq = self.generation.value(),
                    "ignored playback error for stale stream"
                );
                return;
            }
        }
        if self.playback.loading_track {
            tracing::debug!(
                event = "stale_playback_error",
                client = self.config.client_name,
                play_seq = self.generation.value(),
                loading = self.playback.loading_track,
                stream_loaded = self.stream_loaded_at.is_some(),
                "ignored playback error while loading another stream"
            );
            return;
        }
        if let Some(stream) = self.playback.restore_base_stream() {
            let resume_at = self.interpolated_position();
            let track_id = self
                .playback
                .current_track()
                .map(|track| track.id.clone())
                .unwrap_or_default();
            tracing::warn!(
                event = "upgrade_fallback",
                client = self.config.client_name,
                track_id = %track_id,
                play_seq = self.generation.value(),
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
            self.stream_loaded_at = Some(Instant::now());
            self.begin_audio_quality_tracking(self.generation, &track_id, &stream);
            self.driver.load_stream_at(&stream, resume_at);
            self.publish_progress();
            self.emit(SessionEvent::UpgradeFallback {
                generation: self.generation,
                track_id,
                stream,
                resume_at,
                error: err,
            })
            .await;
            return;
        }
        if let Some(track) = self.playback.current_track().cloned() {
            if PlaybackSession::should_attempt_recovery(self.retried_generation, self.generation) {
                let generation = self.generation;
                self.retried_generation = Some(generation);
                self.upgrade_started_generation = None;
                self.upgrading_quality = false;
                self.playback_tasks.restart();
                if let Some((key, task)) = self.next_prefetch_task.take() {
                    task.abort();
                    self.emit(SessionEvent::PrecacheUpdated {
                        generation,
                        next_track_id: key.next_track_id,
                        notice: PrecacheNotice::Cancelled,
                    })
                    .await;
                }
                if let Some((_, task)) = self.adopted_prefetch_task.take() {
                    task.abort();
                }
                self.next_precache.take();
                self.playback.loading_track = true;
                self.playback.paused = true;
                self.awaiting_playback_start = false;
                self.pos_anchor = None;
                self.playback.stream_badge = "Retrying...".to_string();
                self.publish_progress();

                tracing::warn!(
                    event = "retrying",
                    client = self.config.client_name,
                    track_id = %track.id,
                    title = %track.title,
                    artist = %track.artist,
                    provider = ?track.source,
                    play_seq = generation.value(),
                    error = %err,
                    "playback stream failed; resolving a fresh stream"
                );
                self.emit(SessionEvent::RecoveryStarted {
                    generation,
                    track: track.clone(),
                    error: err,
                })
                .await;

                let source = Arc::clone(&self.source);
                let tx = self.internal_tx.clone();
                let track_id = track.id.clone();
                self.playback_tasks.spawn(async move {
                    match source.resolve_recovery(track).await {
                        Ok(stream) => {
                            let _ = tx
                                .send(InternalResult::StreamReady {
                                    generation,
                                    track_id,
                                    stream,
                                })
                                .await;
                        }
                        Err(error) => {
                            let _ = tx
                                .send(InternalResult::StreamError {
                                    generation,
                                    track_id,
                                    error,
                                })
                                .await;
                        }
                    }
                });
                return;
            }
        }
        self.playback.loading_track = false;
        self.upgrading_quality = false;
        self.playback.paused = true;
        self.awaiting_playback_start = false;
        self.pos_anchor = None;
        self.playback.stream_badge = "Error".to_string();
        self.driver.set_paused(true);
        self.publish_progress();
        self.emit(SessionEvent::PlaybackError {
            generation: self.generation,
            track_id: self.playback.current_track().map(|t| t.id.clone()),
            error: err,
            badge: self.playback.stream_badge.clone(),
        })
        .await;
    }

    async fn start_hifi_upgrade(&mut self) {
        if self.playback.paused
            || self.playback.loading_track
            || self.upgrade_started_generation == Some(self.generation)
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
            && (!self.config.policy.hifi_substitution
                || !self.source.needs_hifi_upgrade(&stream.badge, &stream.url))
        {
            return;
        }

        let generation = self.generation;
        self.upgrade_started_generation = Some(generation);
        self.upgrading_quality = true;
        tracing::info!(
            event = "upgrade_resolving",
            client = self.config.client_name,
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq = generation.value(),
            current_quality = %stream.badge,
            "resolving higher quality stream"
        );
        self.emit(SessionEvent::UpgradeStarted {
            generation,
            track_id: track.id.clone(),
        })
        .await;

        let source = Arc::clone(&self.source);
        let tx = self.internal_tx.clone();
        let allow_hifi = self.config.policy.hifi_substitution;
        let client_name = self.config.client_name;
        let track_id = track.id.clone();
        self.playback_tasks.spawn(async move {
            if let Some(upgraded) = source.resolve_hifi_upgrade(track, stream, allow_hifi).await {
                let _ = tx
                    .send(InternalResult::StreamUpgrade {
                        generation,
                        track_id,
                        stream: upgraded,
                    })
                    .await;
            } else {
                tracing::info!(
                    event = "upgrade_unavailable",
                    client = client_name,
                    track_id = %track_id,
                    play_seq = generation.value(),
                    "no higher quality stream available"
                );
                let _ = tx
                    .send(InternalResult::UpgradeFinished {
                        generation,
                        track_id,
                    })
                    .await;
            }
        });
    }

    async fn tick_playback(&mut self) {
        if self.playback.paused
            || self.playback.loading_track
            || self.playback.current_track().is_none()
        {
            return;
        }
        let next = self.interpolated_position();
        if (next - self.playback.position_secs).abs() >= 0.015 {
            self.playback.position_secs = next;
            self.pos_anchor = Some((next, Instant::now()));
            self.publish_progress();
        }
        self.tick_crossfade();
        self.poll_adopted_precache().await;
        self.maybe_precache_next().await;
    }

    async fn maybe_precache_next(&mut self) {
        let current = self.playback.current_track().cloned();
        let next_track = self
            .playback
            .peek_next_index()
            .and_then(|index| self.playback.queue.get(index).cloned());
        let Some(current) = current else { return };
        let next_id = next_track.as_ref().map(|track| track.id.as_str());

        if let Some(cancelled) = self.next_precache.cancel_if_changed(
            self.generation.value(),
            Some(&current.id),
            next_id,
        ) {
            if let Some((key, task)) = self.next_prefetch_task.take() {
                if key == cancelled {
                    task.abort();
                    tracing::info!(
                        event = "next_precache_cancelled",
                        client = self.config.client_name,
                        track_id = %cancelled.next_track_id,
                        reason = "next-track-changed",
                        "next-track precache cancelled"
                    );
                    self.emit(SessionEvent::PrecacheUpdated {
                        generation: self.generation,
                        next_track_id: cancelled.next_track_id,
                        notice: PrecacheNotice::Cancelled,
                    })
                    .await;
                } else {
                    self.next_prefetch_task = Some((key, task));
                }
            }
        }

        let Some(key) = self.next_precache.schedule(
            self.generation.value(),
            &current.id,
            next_id,
            self.interpolated_position(),
            self.playback.duration_secs,
            self.config.policy.allows_precache(),
        ) else {
            return;
        };
        let Some(next_track) = next_track else { return };
        tracing::info!(
            event = "next_precache_scheduled",
            client = self.config.client_name,
            track_id = %next_track.id,
            title = %next_track.title,
            remaining_secs = (self.playback.duration_secs - self.playback.position_secs).max(0.0),
            "next-track quality precache scheduled"
        );
        self.emit(SessionEvent::PrecacheUpdated {
            generation: self.generation,
            next_track_id: next_track.id.clone(),
            notice: PrecacheNotice::Scheduled,
        })
        .await;

        let source = Arc::clone(&self.source);
        let allow_hifi = self.config.policy.hifi_substitution;
        let task = PlaybackTask::spawn_detached(async move {
            source.precache_hifi_audio(next_track, allow_hifi).await
        });
        self.next_prefetch_task = Some((key, task));
    }

    async fn poll_adopted_precache(&mut self) {
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
                client = self.config.client_name,
                track_id = %track_id,
                reason = "quality-unavailable",
                "adopted next-track precache did not produce an upgrade"
            );
            self.emit(SessionEvent::PrecacheUpdated {
                generation: self.generation,
                next_track_id: track_id,
                notice: PrecacheNotice::Failed,
            })
            .await;
            return;
        };
        tracing::info!(
            event = "next_precache_adopted",
            client = self.config.client_name,
            track_id = %track_id,
            quality = %stream.badge,
            "applying completed precache to current track"
        );
        self.emit(SessionEvent::PrecacheUpdated {
            generation: self.generation,
            next_track_id: track_id.clone(),
            notice: PrecacheNotice::Adopted,
        })
        .await;
        self.handle_internal_result(InternalResult::StreamUpgrade {
            generation: self.generation,
            track_id,
            stream,
        })
        .await;
    }

    async fn prepare_next_before_advance(&mut self, index: usize) {
        let Some(track) = self.playback.queue.get(index).cloned() else {
            return;
        };
        if self
            .pending_auto_advance
            .as_ref()
            .is_some_and(|(gen, id)| *gen == self.generation && id == &track.id)
        {
            return;
        }
        let generation = self.generation;
        self.pending_auto_advance = Some((generation, track.id.clone()));
        self.playback.loading_track = true;
        self.playback.paused = true;
        self.playback.stream_badge = "Preparing next track...".to_string();
        self.pos_anchor = None;
        self.publish_progress();
        self.emit(SessionEvent::PreparingNextTrack {
            generation,
            track: track.clone(),
        })
        .await;

        let prepared = match self.next_prefetch_task.take() {
            Some((key, mut task)) if key.next_track_id == track.id => match task.try_take() {
                Some(result) => {
                    if result.is_some() {
                        self.next_precache.mark_ready(&key);
                        self.emit(SessionEvent::PrecacheUpdated {
                            generation,
                            next_track_id: track.id.clone(),
                            notice: PrecacheNotice::Ready,
                        })
                        .await;
                    } else {
                        self.next_precache.mark_failed(&key);
                        self.emit(SessionEvent::PrecacheUpdated {
                            generation,
                            next_track_id: track.id.clone(),
                            notice: PrecacheNotice::Failed,
                        })
                        .await;
                    }
                    result
                }
                None => {
                    self.adopted_prefetch_task = Some((track.id.clone(), task));
                    None
                }
            },
            Some((key, task)) => {
                task.abort();
                self.emit(SessionEvent::PrecacheUpdated {
                    generation,
                    next_track_id: key.next_track_id,
                    notice: PrecacheNotice::Cancelled,
                })
                .await;
                None
            }
            None => None,
        };

        let source = Arc::clone(&self.source);
        let tx = self.internal_tx.clone();
        let track_id = track.id.clone();
        self.playback_tasks.spawn(async move {
            let result = match prepared {
                Some(stream) => Ok(stream),
                None => source.resolve_fast(track).await,
            };
            let _ = tx
                .send(InternalResult::NextTrackPrepared {
                    generation,
                    index,
                    track_id,
                    result,
                })
                .await;
        });
    }

    async fn shutdown(&mut self) -> ShutdownReport {
        if let Some((_, task)) = self.next_prefetch_task.take() {
            task.abort();
        }
        if let Some((_, task)) = self.adopted_prefetch_task.take() {
            task.abort();
        }
        self.next_precache.take();
        let tasks_joined_cleanly = self
            .playback_tasks
            .shutdown(self.config.shutdown_timeout)
            .await;
        if !tasks_joined_cleanly {
            tracing::warn!("playback tasks exceeded shutdown timeout and were aborted");
        }
        self.driver.shutdown().await;
        ShutdownReport {
            tasks_joined_cleanly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_crossfade_secs, compute_crossfade_entry_volume, compute_crossfade_exit_volume,
        PlaybackAudioDriver, PlaybackSession, PlaybackSessionConfig, PlaybackStreamSource,
        SessionCommand, SessionEvent, SessionPolicy,
    };
    use crate::downloads::test_support::TempTestDir;
    use crate::player::mpv::{MpvAudioParams, MpvEvent};
    use sonata_core::player::RepeatMode;
    use sonata_core::providers::hifi::ResolvedStream;
    use sonata_core::providers::{ProviderSource, Track};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[derive(Debug, Clone, PartialEq)]
    enum TestDriverCall {
        Load(String),
        LoadAt(String, f64),
        Stop,
        Paused(bool),
        Seek(f64),
        Volume(u8),
        Speed(f64),
        SkipSilence(bool),
        Gapless(bool),
        VideoMode {
            enabled: bool,
            url: Option<String>,
            headers: Vec<(String, String)>,
            seek_secs: Option<f64>,
        },
        Shutdown,
    }

    #[derive(Default)]
    struct RecordingDriver {
        calls: Mutex<Vec<TestDriverCall>>,
    }

    impl RecordingDriver {
        fn calls(&self) -> Vec<TestDriverCall> {
            self.calls.lock().expect("driver lock").clone()
        }
    }

    impl PlaybackAudioDriver for RecordingDriver {
        fn load_stream(&self, stream: &ResolvedStream) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Load(stream.url.clone()));
        }

        fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::LoadAt(stream.url.clone(), start_secs));
        }

        fn stop(&self) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Stop);
        }

        fn set_paused(&self, paused: bool) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Paused(paused));
        }

        fn seek_absolute(&self, secs: f64) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Seek(secs));
        }

        fn set_volume(&self, volume: u8) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Volume(volume));
        }

        fn set_speed(&self, speed: f64) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Speed(speed));
        }

        fn set_skip_silence(&self, enabled: bool) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::SkipSilence(enabled));
        }

        fn set_gapless(&self, enabled: bool) {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Gapless(enabled));
        }

        fn set_video_mode(
            &self,
            enabled: bool,
            url: Option<String>,
            headers: Vec<(String, String)>,
            seek_secs: Option<f64>,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::VideoMode {
                    enabled,
                    url,
                    headers,
                    seek_secs,
                });
            Box::pin(async { Ok(()) })
        }

        fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.calls
                .lock()
                .expect("driver lock")
                .push(TestDriverCall::Shutdown);
            Box::pin(async {})
        }
    }

    struct StaticSource {
        stream: ResolvedStream,
    }

    impl PlaybackStreamSource for StaticSource {
        fn resolve_fast(
            &self,
            _track: Track,
        ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
            let s = self.stream.clone();
            Box::pin(async move { Ok(s) })
        }

        fn resolve_recovery(
            &self,
            _track: Track,
        ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
            let s = self.stream.clone();
            Box::pin(async move { Ok(s) })
        }

        fn resolve_hifi_upgrade(
            &self,
            _track: Track,
            _current: ResolvedStream,
            _allow_hifi: bool,
        ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
            Box::pin(async { None })
        }

        fn precache_hifi_audio(
            &self,
            _track: Track,
            _allow_hifi: bool,
        ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
            Box::pin(async { None })
        }
    }

    fn make_track(id: &str, duration_secs: u64) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Track {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
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

    fn fast_test_config() -> PlaybackSessionConfig {
        PlaybackSessionConfig {
            client_name: "session-unit-test",
            initial_volume: 80,
            initial_muted: false,
            initial_shuffle: false,
            initial_repeat: RepeatMode::Off,
            initial_gapless: true,
            initial_crossfade_secs: 0,
            policy: SessionPolicy {
                hifi_substitution: false,
                jiosaavn_enabled: false,
                cache_enabled: false,
            },
            stale_error_grace: Duration::from_millis(50),
            shutdown_timeout: Duration::from_millis(500),
            tick_interval: Duration::from_millis(40),
            position_steering: false,
        }
    }

    async fn recv_event_matching<F>(
        events: &mut mpsc::Receiver<SessionEvent>,
        mut pred: F,
    ) -> SessionEvent
    where
        F: FnMut(&SessionEvent) -> bool,
    {
        tokio::time::timeout(Duration::from_secs(4), async {
            while let Some(ev) = events.recv().await {
                if pred(&ev) {
                    return ev;
                }
            }
            panic!("session event channel closed before matching event arrived");
        })
        .await
        .expect("timed out waiting for SessionEvent")
    }

    #[tokio::test]
    async fn emits_measured_audio_quality_for_live_and_local_flac_streams() {
        let dir = TempTestDir::new("session_local_flac");
        let flac_path = dir.path().join("lossless.flac");
        // 198_000 bytes over 1.8s -> 880 kbps exact
        std::fs::write(&flac_path, vec![0u8; 198_000]).expect("write test flac");

        let driver = Arc::new(RecordingDriver::default());
        let source = Arc::new(StaticSource {
            stream: ResolvedStream {
                url: flac_path.to_string_lossy().into_owned(),
                badge: "FLAC · Local".to_string(),
                headers: Vec::new(),
            },
        });
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            fast_test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            source,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![make_track("local-flac", 2)],
                start_index: 0,
                push_history: false,
            })
            .await
            .expect("play queue");

        recv_event_matching(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { .. })
        })
        .await;

        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        mpv_tx.send(MpvEvent::Duration(1.8)).await.unwrap();
        mpv_tx
            .send(MpvEvent::AudioCodec(Some("flac".to_string())))
            .await
            .unwrap();
        mpv_tx
            .send(MpvEvent::AudioParams(Some(MpvAudioParams {
                sample_rate_hz: Some(44100),
                bit_depth: Some(16),
                format: Some("s16".to_string()),
            })))
            .await
            .unwrap();

        let local_ev = recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::AudioQualityUpdated { badge, .. }
                    if badge == "FLAC · 880 kbps · 16-bit/44.1kHz"
            )
        })
        .await;
        assert!(matches!(
            local_ev,
            SessionEvent::AudioQualityUpdated {
                bitrate_kbps: Some(880),
                sample_rate_hz: Some(44100),
                bit_depth: Some(16),
                ..
            }
        ));

        // Live audio-bitrate overrides file-size calculation when present (e.g., 24-bit/96kHz stream)
        mpv_tx
            .send(MpvEvent::AudioBitrate(Some(2_450_000)))
            .await
            .unwrap();
        mpv_tx
            .send(MpvEvent::AudioParams(Some(MpvAudioParams {
                sample_rate_hz: Some(96000),
                bit_depth: Some(24),
                format: Some("s24".to_string()),
            })))
            .await
            .unwrap();

        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::AudioQualityUpdated { badge, .. }
                    if badge == "FLAC · 2450 kbps · 24-bit/96kHz"
            )
        })
        .await;

        session.handle.shutdown().await;
    }

    #[tokio::test]
    async fn handles_speed_skip_silence_and_video_mode_switch() {
        let driver = Arc::new(RecordingDriver::default());
        let source = Arc::new(StaticSource {
            stream: ResolvedStream {
                url: "https://example.com/audio.webm".to_string(),
                badge: "OPUS 256kbps · Extreme".to_string(),
                headers: vec![("User-Agent".to_string(), "SonataTest/1.0".to_string())],
            },
        });
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            fast_test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            source,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![make_track("vid-1", 180)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        mpv_tx.send(MpvEvent::Position(42.5)).await.unwrap();

        // Speed clamping
        session
            .handle
            .send(SessionCommand::SetPlaybackSpeed(4.0))
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged { playback_speed, .. }
                    if (*playback_speed - 2.0).abs() < 1e-6
            )
        })
        .await;

        // Enable skip silence in audio mode
        session
            .handle
            .send(SessionCommand::SetSkipSilence(true))
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    skip_silence: true,
                    video_mode: false,
                    ..
                }
            )
        })
        .await;

        // Switch to video mode: skip_silence auto-disables and preserves headers & position
        session
            .handle
            .send(SessionCommand::SetVideoMode {
                enabled: true,
                url: Some("https://example.com/video720.mp4".to_string()),
                headers: vec![(
                    "Referer".to_string(),
                    "https://music.youtube.com/".to_string(),
                )],
                seek_secs: Some(42.5),
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    skip_silence: false,
                    video_mode: true,
                    ..
                }
            )
        })
        .await;

        // Exit video mode: skip_silence preference resumes
        session
            .handle
            .send(SessionCommand::SetVideoMode {
                enabled: false,
                url: None,
                headers: Vec::new(),
                seek_secs: Some(50.0),
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    skip_silence: true,
                    video_mode: false,
                    ..
                }
            )
        })
        .await;

        let calls = driver.calls();
        assert!(calls.contains(&TestDriverCall::Speed(2.0)));
        assert!(calls.contains(&TestDriverCall::SkipSilence(true)));
        assert!(calls.contains(&TestDriverCall::SkipSilence(false)));
        assert!(calls.iter().any(|c| matches!(
            c,
            TestDriverCall::VideoMode {
                enabled: true,
                url: Some(u),
                headers,
                seek_secs: Some(pos),
            } if u == "https://example.com/video720.mp4"
                && headers == &vec![("Referer".to_string(), "https://music.youtube.com/".to_string())]
                && (*pos - 42.5).abs() < 1e-6
        )));

        session.handle.shutdown().await;
    }

    #[tokio::test]
    async fn sleep_timer_fades_volume_pauses_and_restores_volume() {
        let driver = Arc::new(RecordingDriver::default());
        let source = Arc::new(StaticSource {
            stream: ResolvedStream {
                url: "https://example.com/audio.webm".to_string(),
                badge: "AAC · 320 kbps".to_string(),
                headers: Vec::new(),
            },
        });
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            fast_test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            source,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![make_track("sleep-1", 300)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();

        // 1-second sleep timer enters fade immediately (min(5s, 1s) = 1s fade) and pauses + restores 80
        session
            .handle
            .send(SessionCommand::SetSleepTimer(Some(1)))
            .await
            .unwrap();

        recv_event_matching(&mut session.events, |e| {
            matches!(e, SessionEvent::PausedChanged { paused: true, .. })
        })
        .await;
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    sleep_timer_remaining_secs: None,
                    ..
                }
            )
        })
        .await;

        let volumes: Vec<u8> = driver
            .calls()
            .into_iter()
            .filter_map(|c| match c {
                TestDriverCall::Volume(v) => Some(v),
                _ => None,
            })
            .collect();
        assert!(
            volumes.iter().any(|&v| v < 80),
            "expected smooth fade volumes below 80, got {volumes:?}"
        );
        assert_eq!(
            volumes.last().copied(),
            Some(80),
            "expected volume restored to 80 after pause, got {volumes:?}"
        );

        session.handle.shutdown().await;
    }

    #[tokio::test]
    async fn handles_gapless_and_crossfade_ramp_bypassed_in_video_mode() {
        assert_eq!(clamp_crossfade_secs(0), 0);
        assert_eq!(clamp_crossfade_secs(7), 7);
        assert_eq!(clamp_crossfade_secs(99), 12);
        assert_eq!(compute_crossfade_exit_volume(80, 2.0, 4.0), 40);
        assert_eq!(compute_crossfade_exit_volume(80, 0.0, 4.0), 0);
        assert_eq!(compute_crossfade_entry_volume(80, 1.0, 4.0), 20);
        assert_eq!(compute_crossfade_entry_volume(80, 4.0, 4.0), 80);

        let driver = Arc::new(RecordingDriver::default());
        let source = Arc::new(StaticSource {
            stream: ResolvedStream {
                url: "https://example.com/audio.webm".to_string(),
                badge: "OPUS 256kbps".to_string(),
                headers: Vec::new(),
            },
        });
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            fast_test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            source,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::SetGapless(false))
            .await
            .unwrap();
        session
            .handle
            .send(SessionCommand::SetCrossfade(15))
            .await
            .unwrap();
        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![make_track("cf-1", 20), make_track("cf-2", 20)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        mpv_tx.send(MpvEvent::Duration(20.0)).await.unwrap();

        // Advance into the final 8s (capped at 40% of 20s = 8s) so exit ramp lowers volume below 80.
        mpv_tx.send(MpvEvent::Position(16.0)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;

        let exit_vols: Vec<u8> = driver
            .calls()
            .into_iter()
            .filter_map(|c| match c {
                TestDriverCall::Volume(v) => Some(v),
                _ => None,
            })
            .collect();
        assert!(
            exit_vols.iter().any(|&v| v < 80),
            "expected crossfade exit volume ramp below 80, got {exit_vols:?}"
        );

        // Video mode automatically bypasses crossfade and restores configured volume 80.
        session
            .handle
            .send(SessionCommand::SetVideoMode {
                enabled: true,
                url: Some("https://example.com/vid.mp4".to_string()),
                headers: Vec::new(),
                seek_secs: Some(16.0),
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    video_mode: true,
                    ..
                }
            )
        })
        .await;

        let after_video_vols: Vec<u8> = driver
            .calls()
            .into_iter()
            .filter_map(|c| match c {
                TestDriverCall::Volume(v) => Some(v),
                _ => None,
            })
            .collect();
        assert_eq!(after_video_vols.last().copied(), Some(80));

        // Exit video mode and trigger natural track end -> next track fades in from 0.
        session
            .handle
            .send(SessionCommand::SetVideoMode {
                enabled: false,
                url: None,
                headers: Vec::new(),
                seek_secs: Some(18.0),
            })
            .await
            .unwrap();
        recv_event_matching(&mut session.events, |e| {
            matches!(
                e,
                SessionEvent::PlaybackOptionsChanged {
                    video_mode: false,
                    ..
                }
            )
        })
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        mpv_tx.send(MpvEvent::Position(18.5)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(90)).await;
        mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
        recv_event_matching(
            &mut session.events,
            |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "cf-2"),
        )
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;

        let calls = driver.calls();
        assert!(calls.contains(&TestDriverCall::Gapless(false)));
        assert!(
            calls.contains(&TestDriverCall::Volume(0)),
            "expected entry fade-in starting from 0, got {calls:?}"
        );

        session.handle.shutdown().await;
    }
}
