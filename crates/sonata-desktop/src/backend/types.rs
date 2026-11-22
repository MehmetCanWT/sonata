//! Shared command, event, handle, and immutable snapshot types for the desktop backend.
//!
//! 1. **Commands & Navigation**: `NavTarget`, `UiWindowCmd`, and `BackendCmd` define UI-to-backend and window-lifecycle messages.
//! 2. **Snapshot**: `DesktopSnapshot` holds immutable state published via `tokio::sync::watch` for GPUI views and MPRIS.
//! 3. **Handle**: `BackendHandle` sends commands and shuts down synchronously via a bounded `sync_channel` timeout.
//! 4. **Internal Events**: `InternalEvent` and `ExportFingerprint` carry async task completions and deduplicate now-playing exports.

use sonata_core::auth::{BrowserOption, ExtractedCookies};
use sonata_core::config::AppConfig;
use sonata_core::lyrics::{active_line_index, LyricsState};
use sonata_core::motion::MotionCover;
use sonata_core::player::RepeatMode;
use sonata_core::providers::hifi::ResolvedStream;
use sonata_core::providers::ytmusic::{ArtistPage, HomeShelf, YtAccountProfile};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::theme::Theme;
use sonata_core::updater::UpdateCheckResult;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavTarget {
    Home,
    Search,
    Queue,
    Playlist(String),
    Artist(String),
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiWindowCmd {
    ShowWindow,
    HideWindow,
    ToggleWindow,
    QuitApp,
}

#[derive(Debug)]
pub enum BackendCmd {
    Navigate(NavTarget),
    OpenCollection {
        id: String,
        title: String,
        subtitle: String,
        thumbnail_url: Option<String>,
        editable: bool,
    },
    OpenArtist(String),
    RefreshActivePlaylist,
    RefreshHome,
    RefreshAccount,
    SignInWithBrowser(String),
    ClearSignInError,
    SignOut,
    Search(String),
    PlayTracks {
        tracks: Vec<Track>,
        start_index: usize,
        shuffle: bool,
    },
    PlayQueueIndex(usize),
    TogglePause,
    SetPaused(bool),
    Stop,
    NextTrack,
    PrevTrack,
    SeekAbsolute(f64),
    SeekRelative(f64),
    SetVolume(u8),
    ToggleMute,
    ToggleShuffle,
    SetShuffle(bool),
    CycleRepeat,
    SetRepeat(RepeatMode),
    ToggleHiFi,
    ToggleJioSaavn,
    StepCacheLimit(bool),
    ToggleTransparentBg,
    ToggleMotionCovers,
    ToggleReduceMotion,
    StepHyprlandOpacity(bool),
    ToggleLike(Track),
    CreateYtmPlaylist(String),
    AddTrackToPlaylist {
        track: Track,
        playlist_id: String,
        playlist_name: String,
    },
    RemoveTrackFromPlaylist(usize),
    ClickLinkSpotify,
    CheckForUpdates,
    SetWindowAddress(Option<String>),
    SetWindowVisible(bool),
    Shutdown(std::sync::mpsc::SyncSender<()>),
}

#[derive(Debug, Clone)]
pub struct DesktopSnapshot {
    pub config: AppConfig,
    pub base_theme: Theme,
    pub active_nav: NavTarget,
    pub status_line: String,
    pub available_browsers: Vec<BrowserOption>,
    pub signed_in_browser: Option<String>,
    pub user_profile: Option<YtAccountProfile>,
    pub user_avatar_path: Option<PathBuf>,
    pub signing_in: bool,
    pub sign_in_error: Option<String>,
    pub ytm_playlists: Vec<Playlist>,
    pub home_shelves: Vec<HomeShelf>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub search_query: String,
    pub search_results: Vec<Track>,
    pub search_loading: bool,
    pub playlist_id: Option<String>,
    pub playlist_heading: String,
    pub playlist_subtitle: String,
    pub playlist_cover_url: Option<String>,
    pub playlist_editable: bool,
    pub playlist_tracks: Vec<Track>,
    pub playlist_loading: bool,
    pub artist_page: Option<ArtistPage>,
    pub artist_loading: bool,
    pub artist_error: Option<String>,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub paused: bool,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub stream_badge: String,
    pub upgrading_quality: bool,
    pub loading_track: bool,
    pub lyrics: LyricsState,
    pub art_path: Option<PathBuf>,
    pub motion_cover: Option<Arc<MotionCover>>,
    pub ambient_rgb: (u8, u8, u8),
    pub spotify_notice: Option<String>,
    pub update_info: Option<UpdateCheckResult>,
    pub checking_updates: bool,
}

impl DesktopSnapshot {
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    pub fn is_signed_in(&self) -> bool {
        self.signed_in_browser.is_some()
    }

    pub fn effective_transparent(&self) -> bool {
        self.base_theme
            .transparent_bg
            .unwrap_or(self.config.transparent_bg)
    }

    pub fn effective_hyprland_opacity(&self) -> f32 {
        self.base_theme
            .hyprland_opacity
            .unwrap_or(self.config.hyprland_opacity)
            .clamp(0.50, 1.00)
    }

    pub fn active_lyric_index(&self) -> Option<usize> {
        match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, self.position_secs),
            _ => None,
        }
    }

    pub fn active_lyric_line(&self) -> Option<&str> {
        match &self.lyrics {
            LyricsState::Synced(lines) => self
                .active_lyric_index()
                .and_then(|idx| lines.get(idx))
                .map(|l| l.text.as_str())
                .filter(|s| !s.is_empty()),
            _ => None,
        }
    }

    pub fn writable_ytm_playlists(&self) -> Vec<Playlist> {
        self.ytm_playlists
            .iter()
            .filter(|p| p.source == ProviderSource::YouTubeMusic && p.id != "VLLM")
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
pub struct BackendHandle {
    pub(super) cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    pub(super) win_tx: mpsc::UnboundedSender<UiWindowCmd>,
    pub(super) snap_rx: watch::Receiver<Arc<DesktopSnapshot>>,
}

impl BackendHandle {
    pub fn send(&self, cmd: BackendCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn send_window(&self, cmd: UiWindowCmd) {
        let _ = self.win_tx.send(cmd);
    }

    pub fn snapshot(&self) -> Arc<DesktopSnapshot> {
        self.snap_rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<DesktopSnapshot>> {
        self.snap_rx.clone()
    }

    pub fn shutdown_sync(&self) {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        if self.cmd_tx.send(BackendCmd::Shutdown(tx)).is_ok() {
            if let Err(err) = rx.recv_timeout(Duration::from_secs(2)) {
                tracing::warn!(?err, "backend shutdown timed out");
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub(super) enum InternalEvent {
    SignInFinished {
        browser_id: String,
        cookies: ExtractedCookies,
        profile: Option<YtAccountProfile>,
        avatar_path: Option<PathBuf>,
        ytm_playlists: Vec<Playlist>,
        error: Option<String>,
    },
    AccountRefreshed {
        profile: Option<YtAccountProfile>,
        avatar_path: Option<PathBuf>,
        ytm_playlists: Vec<Playlist>,
    },
    HomeLoaded {
        req_id: u64,
        shelves: Vec<HomeShelf>,
        error: Option<String>,
    },
    SearchLoaded {
        req_id: u64,
        tracks: Vec<Track>,
        error: Option<String>,
    },
    CollectionLoaded {
        req_id: u64,
        id: String,
        heading: String,
        tracks: Vec<Track>,
        silent_replace: bool,
    },
    CollectionAppended {
        req_id: u64,
        tracks: Vec<Track>,
    },
    ArtistLoaded {
        req_id: u64,
        artist: Option<ArtistPage>,
        error: Option<String>,
    },
    StreamReady {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamUpgrade {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    UpgradeFinished {
        play_seq: u64,
    },
    StreamError {
        play_seq: u64,
        track_id: String,
        error: String,
    },
    NextTrackPrepared {
        play_seq: u64,
        index: usize,
        track_id: String,
        result: Result<ResolvedStream, String>,
    },
    LyricsReady {
        play_seq: u64,
        track_id: String,
        lyrics: LyricsState,
    },
    ArtworkReady {
        play_seq: u64,
        track_id: String,
        url: String,
        art_path: Option<PathBuf>,
        ambient_rgb: (u8, u8, u8),
    },
    ArtworkEnhanced {
        play_seq: u64,
        track_id: String,
        url: String,
        art_path: Option<PathBuf>,
        ambient_rgb: (u8, u8, u8),
    },
    AnimatedArtworkReady {
        play_seq: u64,
        track_id: String,
        cover: Arc<MotionCover>,
    },
    ThumbnailsReady,
    PlaylistCreated(Playlist),
    UpdateChecked(UpdateCheckResult),
    UpdateCheckFailed(String),
    StatusMessage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExportFingerprint {
    pub(super) track_id: String,
    pub(super) status: String,
    pub(super) lyric: Option<String>,
    pub(super) art_path: Option<String>,
    pub(super) pos_secs: u64,
    pub(super) dur_secs: u64,
}
