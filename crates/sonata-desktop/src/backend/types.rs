//! Shared command, event, handle, and immutable snapshot types for the desktop backend.
//!
//! 1. **Commands & Navigation**: `NavTarget`, `UiWindowCmd`, and `BackendCmd` define UI-to-backend and window-lifecycle messages.
//! 2. **Snapshot**: `DesktopSnapshot` holds immutable state published via `tokio::sync::watch` for Tauri IPC and OS media controls.
//! 3. **Handle**: `BackendHandle` sends commands and shuts down synchronously via a bounded `sync_channel` timeout.
//! 4. **Internal Events**: `InternalEvent` and `ExportFingerprint` carry async task completions and deduplicate now-playing exports.

use sonata_core::auth::ExtractedCookies;
use sonata_core::connect::ConnectStateSnapshot;
use sonata_core::lyrics::LyricsState;
use sonata_core::player::{PlaybackState, RepeatMode};
use sonata_core::playlists::{is_personal_track_id, PersonalPlaylist};
use sonata_core::providers::hifi::{ResolvedMusicVideo, VideoQualityStream};
use sonata_core::providers::local::LocalScanResult;
use sonata_core::providers::servers::{
    ServerHealthReport, ServerKind, ServerLibrary, ServerProfile, ServerSpeedTestReport,
};
use sonata_core::providers::ytmusic::{
    AlbumDetail, ArtistPage, HomeCard, HomeShelf, MoodCategoryTile, MoodChip, MusicVideoCandidate,
    RichSearchResult, YtAccountProfile,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::stats::{StatsSummary, StatsWindow};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::auth::BrowserOption;
use sonata_runtime::config::AppConfig;
use sonata_runtime::connect::{PairedDeviceInfo, PendingPairRequest};
use sonata_runtime::discord::DiscordRpcStatus;
use sonata_runtime::downloads::{DownloadCollectionRef, DownloadedCollection, DownloadedTrack};
use sonata_runtime::i18n::AppLanguage;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavTarget {
    Home,
    Explore,
    Radio,
    Library,
    History,
    Stats,
    Search,
    Queue,
    Playlist(String),
    Album(String),
    Artist(String),
    MoodCategory {
        title: String,
        browse_id: String,
        params: String,
    },
    ShelfDetail {
        title: String,
    },
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiWindowCmd {
    ShowWindow,
    HideWindow,
    ToggleWindow,
    QuitApp,
    AllowAssetDir(PathBuf),
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
    OpenArtistByName(String),
    OpenAlbum {
        album_id: Option<String>,
        title: String,
        artist: String,
        thumbnail_url: Option<String>,
    },
    OpenMoodCategory {
        title: String,
        browse_id: String,
        params: String,
    },
    OpenMoodByTitle(String),
    OpenShelfDetail(HomeShelf),
    SelectHomeChip(Option<MoodChip>),
    RefreshActivePlaylist,
    RefreshHome,
    RefreshExplore,
    RefreshRadio,
    RefreshLibrary,
    RefreshHistory,
    RefreshAccount,
    StartTrackRadio(Track),
    StartQuickRadio,
    SignInWithBrowser(String),
    ClearSignInError,
    SignOut,
    FetchSearchSuggestions(String),
    Search(String),
    PlayTracks {
        tracks: Vec<Track>,
        start_index: usize,
        shuffle: bool,
    },
    PlayQueueIndex(usize),
    PlayTrackNext(Track),
    AddTrackToQueue(Track),
    RemoveFromQueue(usize),
    MoveQueueItem {
        from: usize,
        to: usize,
    },
    ClearQueue,
    ShuffleQueue,
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
    ToggleAutoplay,
    ToggleHiFi,
    ToggleJioSaavn,
    TogglePersonalOnlyMode,
    ToggleLocalMusicEnabled,
    AddLocalMusicDir(PathBuf),
    RemoveLocalMusicDir(PathBuf),
    ScanLocalMusic,
    ToggleAttributePersonalToYtm,
    StepCacheLimit(bool),
    ToggleMotionCovers,
    ToggleReduceMotion,
    SetLanguage(AppLanguage),
    SetPlaybackSpeed(f64),
    ToggleSkipSilence,
    SetSleepTimer(Option<u64>),
    ToggleLyricsTranslate,
    ToggleLyricsRomanize,
    SetVideoMode(bool),
    SetVideoQuality(String),
    ToggleLike(Track),
    DislikeTrack(Track),
    ToggleSaveAlbum,
    ToggleSavePlaylist {
        id: String,
        title: String,
        thumbnail_url: Option<String>,
        save: bool,
    },
    CreateYtmPlaylist(String),
    CreatePersonalPlaylist(String),
    DeletePersonalPlaylist(String),
    EditPlaylist {
        playlist_id: String,
        title: Option<String>,
        cover_path: Option<PathBuf>,
    },
    AddTrackToPlaylist {
        track: Track,
        playlist_id: String,
        playlist_name: String,
    },
    AddTrackToPersonalPlaylist {
        track: Track,
        playlist_id: String,
    },
    RemoveTrackFromPlaylist(usize),
    ClickLinkSpotify,
    CheckForUpdates,
    RefreshDownloads,
    DownloadTrack {
        track: Track,
        collection: Option<DownloadCollectionRef>,
    },
    RemoveDownloadedTrack(String),
    DownloadCollection {
        collection_id: String,
        title: String,
        source: ProviderSource,
        artwork_url: Option<String>,
        tracks: Vec<Track>,
    },
    RemoveDownloadedCollection(String),
    SetDownloadsDir(Option<PathBuf>),
    StepDownloadsLimit(bool),
    ClearAllDownloads,
    ToggleDiscordRpc,
    RefreshServers,
    SaveServerProfile {
        id: Option<String>,
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
        share: Option<String>,
        domain: Option<String>,
        local_mount_path: Option<PathBuf>,
    },
    TestServerDraft {
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
        share: Option<String>,
        domain: Option<String>,
        local_mount_path: Option<PathBuf>,
    },
    ProbeServerHealth(String),
    RunServerSpeedTest(String),
    ToggleServerEnabled(String),
    RemoveServerProfile(String),
    SelectServer {
        server_id: String,
        library_id: Option<String>,
        folder_path: Option<String>,
    },
    SetWindowAddress(Option<String>),
    SetWindowVisible(bool),
    SetUiCompact(bool),
    ToggleGapless,
    SetCrossfade(u8),
    SaveScrobbleConfig {
        lastfm_enabled: bool,
        lastfm_api_key: String,
        lastfm_api_secret: Option<String>,
        lastfm_session_key: Option<String>,
        lastfm_username: String,
        listenbrainz_enabled: bool,
        listenbrainz_token: Option<String>,
        listenbrainz_username: String,
    },
    ClearScrobbleBackend {
        lastfm: bool,
        listenbrainz: bool,
    },
    LoadListeningStats(StatsWindow),
    ToggleConnect,
    SetConnectDeviceName(String),
    RegenerateConnectPin,
    UnpairConnectDevice(String),
    ApproveConnectPair(String),
    RejectConnectPair(String),
    Shutdown(std::sync::mpsc::SyncSender<()>),
}

/// Redacted scrobbler status exposed to the UI without exposing API secrets or tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScrobbleUiStatus {
    pub lastfm_enabled: bool,
    pub lastfm_configured: bool,
    pub lastfm_username: String,
    pub lastfm_api_key: String,
    pub listenbrainz_enabled: bool,
    pub listenbrainz_configured: bool,
    pub listenbrainz_username: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DesktopSnapshot {
    pub config: AppConfig,
    pub active_nav: NavTarget,
    pub status_line: String,
    pub available_browsers: Vec<BrowserOption>,
    pub signed_in_browser: Option<String>,
    pub user_profile: Option<YtAccountProfile>,
    pub user_avatar_path: Option<PathBuf>,
    pub signing_in: bool,
    pub sign_in_error: Option<String>,
    pub ytm_playlists: Vec<Playlist>,
    pub library_albums: Vec<HomeCard>,
    pub library_artists: Vec<HomeCard>,
    pub library_extras_loading: bool,
    pub home_chips: Vec<MoodChip>,
    pub active_home_chip: Option<MoodChip>,
    pub home_shelves: Vec<HomeShelf>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub explore_shelves: Vec<HomeShelf>,
    pub mood_tiles: Vec<MoodCategoryTile>,
    pub explore_loading: bool,
    pub mood_page_title: String,
    pub mood_page_shelves: Vec<HomeShelf>,
    pub mood_page_loading: bool,
    pub shelf_detail: Option<HomeShelf>,
    pub radio_shelves: Vec<HomeShelf>,
    pub radio_loading: bool,
    pub history_tracks: Vec<Track>,
    pub history_loading: bool,
    pub last_played_tracks: Vec<Track>,
    pub recent_searches: Vec<String>,
    pub search_suggestions: Vec<String>,
    pub search_query: String,
    pub search_results: Vec<Track>,
    pub search_rich: RichSearchResult,
    pub search_loading: bool,
    pub playlist_id: Option<String>,
    pub playlist_heading: String,
    pub playlist_subtitle: String,
    pub playlist_cover_url: Option<String>,
    pub playlist_editable: bool,
    pub playlist_tracks: Vec<Track>,
    pub playlist_loading: bool,
    pub album_detail: Option<AlbumDetail>,
    pub album_loading: bool,
    pub album_error: Option<String>,
    pub artist_page: Option<ArtistPage>,
    pub artist_banner_url: Option<String>,
    pub artist_loading: bool,
    pub artist_error: Option<String>,
    pub liked_track_ids: std::collections::HashSet<String>,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub paused: bool,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub autoplay: bool,
    pub stream_badge: String,
    pub upgrading_quality: bool,
    pub loading_track: bool,
    pub playback_speed: f64,
    pub skip_silence: bool,
    pub sleep_timer_remaining_secs: Option<u64>,
    pub video_mode: bool,
    pub video_loading: bool,
    pub music_video_candidate: Option<MusicVideoCandidate>,
    pub available_video_qualities: Vec<VideoQualityStream>,
    pub lyrics: LyricsState,
    pub translated_lyrics: Option<Vec<String>>,
    pub romanized_lyrics: Option<Vec<String>>,
    pub art_path: Option<PathBuf>,
    pub motion_cover_path: Option<PathBuf>,
    pub ambient_rgb: (u8, u8, u8),
    pub spotify_notice: Option<String>,
    pub update_info: Option<UpdateCheckResult>,
    pub checking_updates: bool,
    pub thumb_rev: u64,
    pub downloaded_tracks: Vec<DownloadedTrack>,
    pub downloaded_collections: Vec<DownloadedCollection>,
    pub downloaded_track_ids: HashSet<String>,
    pub downloading_track_ids: HashSet<String>,
    pub downloads_total_bytes: u64,
    pub discord_status: DiscordRpcStatus,
    pub server_profiles: Vec<ServerProfile>,
    pub server_statuses: HashMap<String, String>,
    pub server_health: HashMap<String, ServerHealthReport>,
    pub server_speed_tests: HashMap<String, ServerSpeedTestReport>,
    pub server_speed_testing: HashSet<String>,
    pub active_server_id: Option<String>,
    pub server_libraries: Vec<ServerLibrary>,
    pub server_artists: Vec<HomeCard>,
    pub server_albums: Vec<HomeCard>,
    pub server_playlists: Vec<Playlist>,
    pub server_tracks: Vec<Track>,
    pub server_current_folder: Option<String>,
    pub server_loading: bool,
    pub server_test_result: Option<Result<String, String>>,
    pub local_scan: LocalScanResult,
    pub local_scanning: bool,
    pub personal_playlists: Vec<PersonalPlaylist>,
    pub scrobble_status: ScrobbleUiStatus,
    pub stats_window: StatsWindow,
    pub stats_summary: StatsSummary,
    pub stats_loading: bool,
    pub connect_running: bool,
    pub connect_http_port: Option<u16>,
    pub connect_pin: Option<String>,
    pub connect_paired_devices: Vec<PairedDeviceInfo>,
    pub connect_pending_pair: Option<PendingPairRequest>,
    pub connect_error: Option<String>,
}

/// Active source tab shown in the Desktop Library view.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibrarySource {
    YouTube,
    Local,
    Jellyfin,
    Subsonic,
    Smb,
    WebDav,
    Downloads,
}

#[cfg(test)]
impl LibrarySource {
    pub fn from_server_kind(kind: ServerKind) -> Self {
        match kind {
            ServerKind::Jellyfin => Self::Jellyfin,
            ServerKind::Subsonic => Self::Subsonic,
            ServerKind::Smb => Self::Smb,
            ServerKind::WebDav => Self::WebDav,
        }
    }
}

/// Computes the ordered list of active Library source tabs for the current config and servers.
#[cfg(test)]
pub fn compute_active_library_sources(
    streaming_enabled: bool,
    local_music_enabled: bool,
    server_profiles: &[ServerProfile],
) -> Vec<LibrarySource> {
    let mut tabs = Vec::new();
    if streaming_enabled {
        tabs.push(LibrarySource::YouTube);
    }
    if local_music_enabled {
        tabs.push(LibrarySource::Local);
    }
    for kind in [
        ServerKind::Jellyfin,
        ServerKind::Subsonic,
        ServerKind::Smb,
        ServerKind::WebDav,
    ] {
        if server_profiles.iter().any(|p| p.enabled && p.kind == kind) {
            tabs.push(LibrarySource::from_server_kind(kind));
        }
    }
    tabs.push(LibrarySource::Downloads);
    tabs
}

fn percent_decode_lossy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((hi << 4) | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn has_parent_traversal(path_str: &str) -> bool {
    path_str
        .split(['/', '\\'])
        .any(|seg| seg == ".." || seg.contains('\0'))
}

fn is_malformed_local_or_smb_id(track_id: &str) -> bool {
    if track_id.contains('\0')
        || track_id.starts_with("file://")
        || track_id.starts_with('/')
        || track_id.starts_with('\\')
    {
        return true;
    }
    if let Some(rest) = track_id.strip_prefix("local:") {
        let decoded = percent_decode_lossy(rest);
        return rest.starts_with("file://")
            || decoded.starts_with("file://")
            || has_parent_traversal(rest)
            || has_parent_traversal(&decoded);
    }
    if let Some(rest) = track_id.strip_prefix("smb:") {
        if rest.starts_with("file://") || rest.starts_with('/') || rest.starts_with('\\') {
            return true;
        }
        let Some((server_id, rel_path)) = rest.split_once(':') else {
            return true;
        };
        if server_id.is_empty() || rel_path.is_empty() {
            return true;
        }
        let decoded = percent_decode_lossy(rel_path);
        return rel_path.starts_with("file://")
            || decoded.starts_with("file://")
            || rel_path.starts_with('/')
            || decoded.starts_with('/')
            || rel_path.starts_with('\\')
            || decoded.starts_with('\\')
            || has_parent_traversal(rel_path)
            || has_parent_traversal(&decoded);
    }
    false
}

/// Validates a track ID received from a remote Sonata Connect `TransferQueue` request.
pub fn is_safe_transfer_track_id(
    track_id: &str,
    personal_only_mode: bool,
    local_roots: &[PathBuf],
    server_profiles: &[ServerProfile],
) -> bool {
    if track_id.is_empty() || is_malformed_local_or_smb_id(track_id) {
        return false;
    }
    if let Some(rest) = track_id.strip_prefix("local:") {
        if personal_only_mode || local_roots.is_empty() {
            return false;
        }
        let decoded = percent_decode_lossy(rest);
        let Ok(canon_target) = std::fs::canonicalize(&decoded) else {
            return false;
        };
        return local_roots.iter().any(|root| {
            std::fs::canonicalize(root)
                .map(|canon_root| canon_target.starts_with(&canon_root))
                .unwrap_or(false)
        });
    }
    if let Some(rest) = track_id.strip_prefix("smb:") {
        if personal_only_mode {
            return false;
        }
        let Some((server_id, _rel_path)) = rest.split_once(':') else {
            return false;
        };
        return server_profiles
            .iter()
            .any(|p| p.enabled && p.kind == ServerKind::Smb && p.id == server_id);
    }
    true
}

/// Returns `true` when `track` is permitted for playback under the active Personal Only setting.
pub fn can_play_track_in_mode(
    track: &Track,
    personal_only_mode: bool,
    is_downloaded: bool,
) -> bool {
    if is_malformed_local_or_smb_id(&track.id) {
        return false;
    }
    !personal_only_mode || is_personal_track_id(&track.id) || is_downloaded
}

/// Separated playlist destinations for the Add-to-Playlist modal.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct PlaylistAddTargets {
    pub personal: Vec<PersonalPlaylist>,
    pub ytm: Vec<Playlist>,
}

/// Routes personal/local/server tracks strictly to Personal Playlists while allowing YTM tracks
/// into both Personal Playlists and YouTube Music playlists (when streaming and signed in).
#[cfg(test)]
pub fn resolve_playlist_add_targets(
    track: &Track,
    personal_playlists: &[PersonalPlaylist],
    ytm_playlists: &[Playlist],
    personal_only_mode: bool,
    signed_in: bool,
) -> PlaylistAddTargets {
    let personal = personal_playlists.to_vec();
    let ytm = if is_personal_track_id(&track.id) || personal_only_mode || !signed_in {
        Vec::new()
    } else {
        ytm_playlists
            .iter()
            .filter(|p| p.source == ProviderSource::YouTubeMusic && p.is_owned && p.id != "VLLM")
            .cloned()
            .collect()
    };
    PlaylistAddTargets { personal, ytm }
}

/// Formats a localized compact health indicator string from a [`ServerHealthReport`].
pub fn format_server_health_badge_for(
    report: &ServerHealthReport,
    lang: sonata_runtime::i18n::AppLanguage,
) -> String {
    sonata_runtime::i18n::format_health_badge(lang, report.alive, report.latency_ms, true)
}

/// Formats a compact English health indicator string from a [`ServerHealthReport`].
#[cfg(test)]
pub fn format_server_health_badge(report: &ServerHealthReport) -> String {
    format_server_health_badge_for(report, sonata_runtime::i18n::AppLanguage::En)
}

/// Formats a localized compact download speed string from a [`ServerSpeedTestReport`].
pub fn format_server_speed_badge_for(
    report: &ServerSpeedTestReport,
    lang: sonata_runtime::i18n::AppLanguage,
) -> String {
    sonata_runtime::i18n::format_speed_badge(
        lang,
        report.alive,
        report.latency_ms,
        report.download_mbps,
        true,
    )
}

/// Formats a compact English download speed string from a [`ServerSpeedTestReport`].
#[cfg(test)]
pub fn format_server_speed_badge(report: &ServerSpeedTestReport) -> String {
    format_server_speed_badge_for(report, sonata_runtime::i18n::AppLanguage::En)
}

impl DesktopSnapshot {
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }
}

#[derive(Clone)]
pub struct BackendHandle {
    pub(super) cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    pub(super) snap_rx: watch::Receiver<Arc<DesktopSnapshot>>,
    pub(super) progress_rx: watch::Receiver<(f64, f64)>,
}

impl BackendHandle {
    pub fn send(&self, cmd: BackendCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn snapshot(&self) -> Arc<DesktopSnapshot> {
        self.snap_rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<DesktopSnapshot>> {
        self.snap_rx.clone()
    }

    pub fn subscribe_progress(&self) -> watch::Receiver<(f64, f64)> {
        self.progress_rx.clone()
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
        chips: Vec<MoodChip>,
        chip_selected: Option<MoodChip>,
        shelves: Vec<HomeShelf>,
        error: Option<String>,
    },
    ExploreLoaded {
        req_id: u64,
        shelves: Vec<HomeShelf>,
        mood_tiles: Vec<MoodCategoryTile>,
    },
    MoodCategoryLoaded {
        req_id: u64,
        title: String,
        shelves: Vec<HomeShelf>,
    },
    LibraryExtrasLoaded {
        albums: Vec<HomeCard>,
        artists: Vec<HomeCard>,
    },
    RadioLoaded {
        req_id: u64,
        shelves: Vec<HomeShelf>,
    },
    HistoryLoaded(Vec<Track>),
    RadioQueueReady {
        tracks: Vec<Track>,
        label: String,
    },
    AutoplayTracksReady {
        play_seq: u64,
        tracks: Vec<Track>,
    },
    AutoplayPrefetched {
        seed_key: String,
        tracks: Vec<Track>,
    },
    ArtistResolveFallback(String),
    AlbumDetailLoaded {
        req_id: u64,
        album: Option<AlbumDetail>,
        error: Option<String>,
    },
    SearchSuggestionsLoaded {
        req_id: u64,
        suggestions: Vec<String>,
    },
    SearchLoaded {
        req_id: u64,
        tracks: Vec<Track>,
        rich: RichSearchResult,
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
        banner_url: Option<String>,
        error: Option<String>,
    },
    LyricsReady {
        play_seq: u64,
        track_id: String,
        lyrics: LyricsState,
    },
    LyricsTranslated {
        play_seq: u64,
        track_id: String,
        target_lang: String,
        lines: Option<Vec<String>>,
    },
    LyricsRomanized {
        play_seq: u64,
        track_id: String,
        lines: Option<Vec<String>>,
    },
    MusicVideoCandidateReady {
        play_seq: u64,
        track_id: String,
        candidate: Option<MusicVideoCandidate>,
    },
    MusicVideoStreamsReady {
        play_seq: u64,
        track_id: String,
        video_id: String,
        result: Result<ResolvedMusicVideo, String>,
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
        path: PathBuf,
    },
    ThumbnailsReady,
    PlaylistCreated(Playlist),
    PlaylistUpdated {
        playlist_id: String,
        title: Option<String>,
        cover_updated: bool,
    },
    UpdateChecked(UpdateCheckResult),
    UpdateCheckFailed(String),
    DownloadsLoaded {
        tracks: Vec<DownloadedTrack>,
        collections: Vec<DownloadedCollection>,
        total_bytes: u64,
    },
    TrackDownloadFinished {
        track_id: String,
        title: String,
        result: Result<(), String>,
    },
    CollectionDownloadFinished {
        title: String,
        saved: usize,
        errors: usize,
    },
    ServerTested {
        server_id: Option<String>,
        result: Result<String, String>,
    },
    ServerSaved {
        profile: ServerProfile,
        message: String,
    },
    ServerBrowseLoaded {
        req_id: u64,
        server_id: String,
        folder_path: Option<String>,
        libraries: Vec<ServerLibrary>,
        artists: Vec<HomeCard>,
        albums: Vec<HomeCard>,
        playlists: Vec<Playlist>,
        tracks: Vec<Track>,
        error: Option<String>,
    },
    ServerHealthProbed {
        server_id: String,
        report: ServerHealthReport,
    },
    ServerSpeedTestFinished {
        server_id: String,
        report: ServerSpeedTestReport,
    },
    LocalMusicScanned {
        req_id: u64,
        scan: LocalScanResult,
    },
    PersonalPlaylistsLoaded(Vec<PersonalPlaylist>),
    StatsDbOpened(Option<sonata_core::stats::StatsDb>),
    StatsLoaded {
        window: StatsWindow,
        summary: StatsSummary,
    },
    ScrobbleError(Option<String>),
    StatusMessage(String),
}

/// Maps a Sonata track to its canonical short source tag (`LOC`, `SMB`, `JFN`, `SUB`, `DAV`, `JIO`, `SPF`, `YTM`).
pub fn track_source_tag(track: &Track) -> &'static str {
    if track.id.starts_with("local:") {
        "LOC"
    } else if track.id.starts_with("smb:") {
        "SMB"
    } else if track.id.starts_with("jellyfin:") {
        "JFN"
    } else if track.id.starts_with("subsonic:") {
        "SUB"
    } else if track.id.starts_with("webdav:") {
        "DAV"
    } else if track.is_jiosaavn() {
        "JIO"
    } else if track.source == ProviderSource::Spotify {
        "SPF"
    } else {
        "YTM"
    }
}

/// Builds a live [`ConnectStateSnapshot`] from current desktop playback state.
pub fn build_connect_snapshot(
    device_id: &str,
    device_name: &str,
    playback: &PlaybackState,
    muted: bool,
    loading_track: bool,
) -> ConnectStateSnapshot {
    let current = playback.current_track();
    let playing = current.is_some() && !playback.paused && !loading_track;
    ConnectStateSnapshot {
        device_id: device_id.to_string(),
        device_name: device_name.to_string(),
        playing,
        paused: playback.paused,
        track_id: current.map(|t| t.id.clone()),
        title: current.map(|t| t.title.clone()),
        artist: current.map(|t| t.artist.clone()),
        album: current.map(|t| t.album.clone()),
        thumbnail_url: current.and_then(|t| t.thumbnail_url.clone()),
        position_secs: playback.position_secs.max(0.0),
        duration_secs: playback.duration_secs.max(0.0),
        volume: if muted { 0 } else { playback.volume.min(100) },
        shuffle: playback.shuffle,
        repeat_mode: playback.repeat,
        queue: playback.queue.clone(),
        current_index: playback.current_index,
        updated_at_unix: 0,
    }
}

/// Filters a Connect `TransferQueue` track list for safety, configured roots, and Personal Only Mode.
pub fn filter_transfer_tracks(
    tracks: Vec<Track>,
    start_index: usize,
    personal_only_mode: bool,
    downloaded_track_ids: &HashSet<String>,
    local_roots: &[PathBuf],
    server_profiles: &[ServerProfile],
) -> Option<(Vec<Track>, usize)> {
    if tracks.is_empty() {
        return None;
    }
    let target_id = tracks.get(start_index).map(|t| t.id.clone());
    let filtered: Vec<Track> = tracks
        .into_iter()
        .filter(|t| {
            is_safe_transfer_track_id(&t.id, personal_only_mode, local_roots, server_profiles)
                && can_play_track_in_mode(
                    t,
                    personal_only_mode,
                    downloaded_track_ids.contains(&t.id),
                )
        })
        .collect();
    if filtered.is_empty() {
        return None;
    }
    let mapped_idx = target_id
        .and_then(|id| filtered.iter().position(|t| t.id == id))
        .unwrap_or(0);
    Some((filtered, mapped_idx))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExportFingerprint {
    pub(super) track_id: String,
    pub(super) status: String,
    pub(super) lyric: Option<String>,
    pub(super) lyric_index: Option<usize>,
    pub(super) lyric_word_index: Option<usize>,
    pub(super) lyrics_state: String,
    pub(super) has_translation: bool,
    pub(super) has_romanization: bool,
    pub(super) art_path: Option<String>,
    pub(super) motion_art_path: Option<String>,
    pub(super) pos_secs: u64,
    pub(super) dur_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::providers::servers::{
        ServerHealthReport, ServerKind, ServerProfile, ServerSpeedTestReport,
    };

    fn dummy_track(id: &str, source: ProviderSource) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Track {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            album_id: None,
            duration_secs: Some(180),
            thumbnail_url: None,
            liked: false,
            set_video_id: None,
            audio_badge: None,
            is_explicit: false,
            source,
        }
    }

    fn dummy_server(id: &str, kind: ServerKind, enabled: bool) -> ServerProfile {
        ServerProfile {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            base_url: "http://localhost:8096".to_string(),
            username: Some("user".to_string()),
            password: None,
            api_key: None,
            access_token: None,
            user_id: None,
            bearer_token: None,
            library_ids: Vec::new(),
            share: None,
            domain: None,
            local_mount_path: None,
            enabled,
        }
    }

    #[test]
    fn active_library_sources_filters_streaming_local_and_servers() {
        let profiles = vec![
            dummy_server("jf", ServerKind::Jellyfin, true),
            dummy_server("sub", ServerKind::Subsonic, false),
            dummy_server("smb", ServerKind::Smb, true),
            dummy_server("dav", ServerKind::WebDav, true),
        ];

        let all = compute_active_library_sources(true, true, &profiles);
        assert_eq!(
            all,
            vec![
                LibrarySource::YouTube,
                LibrarySource::Local,
                LibrarySource::Jellyfin,
                LibrarySource::Smb,
                LibrarySource::WebDav,
                LibrarySource::Downloads,
            ]
        );

        let personal_only = compute_active_library_sources(false, true, &profiles);
        assert_eq!(
            personal_only,
            vec![
                LibrarySource::Local,
                LibrarySource::Jellyfin,
                LibrarySource::Smb,
                LibrarySource::WebDav,
                LibrarySource::Downloads,
            ]
        );
    }

    #[test]
    fn personal_only_mode_allows_local_servers_and_downloads_only() {
        let local = dummy_track("local:/music/song.flac", ProviderSource::YouTubeMusic);
        let smb = dummy_track("smb:nas:song.flac", ProviderSource::YouTubeMusic);
        let jellyfin = dummy_track("jellyfin:jf:123", ProviderSource::YouTubeMusic);
        let ytm = dummy_track("dQw4w9WgXcQ", ProviderSource::YouTubeMusic);
        let jio = dummy_track("jiosaavn:xyz", ProviderSource::YouTubeMusic);

        assert!(can_play_track_in_mode(&local, true, false));
        assert!(can_play_track_in_mode(&smb, true, false));
        assert!(can_play_track_in_mode(&jellyfin, true, false));
        assert!(!can_play_track_in_mode(&ytm, true, false));
        assert!(can_play_track_in_mode(&ytm, true, true));
        assert!(!can_play_track_in_mode(&jio, true, false));
        assert!(can_play_track_in_mode(&ytm, false, false));
        assert!(can_play_track_in_mode(&jio, false, false));
    }

    #[test]
    fn personal_tracks_never_route_to_ytm_playlists() {
        let personal_pl = PersonalPlaylist {
            id: "personal:mix1".to_string(),
            name: "FLAC Mix".to_string(),
            tracks: vec![],
            created_at_epoch_secs: 1,
            updated_at_epoch_secs: 1,
        };
        let ytm_pl = Playlist {
            id: "PL123".to_string(),
            name: "My YTM Mix".to_string(),
            track_count: Some(5),
            thumbnail_url: None,
            is_owned: true,
            source: ProviderSource::YouTubeMusic,
        };
        let vllm = Playlist {
            id: "VLLM".to_string(),
            name: "Liked Music".to_string(),
            track_count: None,
            thumbnail_url: None,
            is_owned: true,
            source: ProviderSource::YouTubeMusic,
        };

        for id in [
            "local:/music/a.flac",
            "smb:nas:a.flac",
            "jellyfin:srv:1",
            "subsonic:srv:2",
            "webdav:srv:3",
        ] {
            let track = dummy_track(id, ProviderSource::YouTubeMusic);
            let targets = resolve_playlist_add_targets(
                &track,
                std::slice::from_ref(&personal_pl),
                &[ytm_pl.clone(), vllm.clone()],
                false,
                true,
            );
            assert_eq!(targets.personal.len(), 1);
            assert!(
                targets.ytm.is_empty(),
                "personal track {id} must not expose YTM playlists"
            );
        }

        let ytm_track = dummy_track("abc123def45", ProviderSource::YouTubeMusic);
        let targets_streaming = resolve_playlist_add_targets(
            &ytm_track,
            std::slice::from_ref(&personal_pl),
            &[ytm_pl.clone(), vllm.clone()],
            false,
            true,
        );
        assert_eq!(targets_streaming.personal.len(), 1);
        assert_eq!(targets_streaming.ytm.len(), 1);
        assert_eq!(targets_streaming.ytm[0].id, "PL123");

        let targets_personal_only =
            resolve_playlist_add_targets(&ytm_track, &[personal_pl], &[ytm_pl, vllm], true, true);
        assert_eq!(targets_personal_only.personal.len(), 1);
        assert!(targets_personal_only.ytm.is_empty());
    }

    #[test]
    fn health_and_speed_badges_format_latency_and_mbps() {
        let alive = ServerHealthReport {
            alive: true,
            latency_ms: Some(14),
            message: "ok".to_string(),
        };
        assert_eq!(format_server_health_badge(&alive), "Alive • 14 ms");

        let offline = ServerHealthReport {
            alive: false,
            latency_ms: None,
            message: "timeout".to_string(),
        };
        assert_eq!(format_server_health_badge(&offline), "Offline");

        let speed_fast = ServerSpeedTestReport {
            alive: true,
            latency_ms: 8,
            download_mbps: Some(245.7),
            bytes_tested: 1024,
            sample_note: "ok".to_string(),
        };
        assert_eq!(format_server_speed_badge(&speed_fast), "246 Mbps • 8 ms");

        let speed_slow = ServerSpeedTestReport {
            alive: true,
            latency_ms: 42,
            download_mbps: Some(18.34),
            bytes_tested: 512,
            sample_note: "ok".to_string(),
        };
        assert_eq!(format_server_speed_badge(&speed_slow), "18.3 Mbps • 42 ms");
    }

    #[test]
    fn connect_snapshot_and_transfer_queue_filtering_respect_mode() {
        let mut playback = PlaybackState::new(72);
        let t1 = dummy_track("local:/music/a.flac", ProviderSource::YouTubeMusic);
        let t2 = dummy_track("ytm123", ProviderSource::YouTubeMusic);
        let t3 = dummy_track("smb:nas:b.flac", ProviderSource::YouTubeMusic);
        playback.queue = vec![t1.clone(), t2.clone(), t3.clone()];
        playback.current_index = Some(2);
        playback.paused = false;
        playback.position_secs = 42.5;
        playback.duration_secs = 180.0;

        let snap = build_connect_snapshot("dev-1", "Sonata Desk", &playback, false, false);
        assert!(snap.playing);
        assert!(!snap.paused);
        assert_eq!(snap.device_name, "Sonata Desk");
        assert_eq!(snap.track_id.as_deref(), Some("smb:nas:b.flac"));
        assert_eq!(snap.volume, 72);
        assert_eq!(snap.queue.len(), 3);
        assert_eq!(snap.current_index, Some(2));

        let muted_snap = build_connect_snapshot("dev-1", "Sonata Desk", &playback, true, false);
        assert_eq!(muted_snap.volume, 0);

        let dl_ids = HashSet::new();
        let smb_profiles = vec![dummy_server("nas", ServerKind::Smb, true)];
        let jfn = dummy_track("jellyfin:s:1", ProviderSource::YouTubeMusic);
        // Remote TransferQueue rejects local:/smb: tracks in Personal Only Mode, keeping server tracks.
        let (filtered, idx) = filter_transfer_tracks(
            vec![t1.clone(), t2.clone(), t3.clone(), jfn.clone()],
            3,
            true,
            &dl_ids,
            &[],
            &smb_profiles,
        )
        .expect("filtered");
        assert_eq!(filtered.len(), 1);
        assert_eq!(idx, 0);
        assert_eq!(filtered[idx].id, "jellyfin:s:1");

        // In normal mode, configured SMB server relative paths are allowed while unconfigured/local are filtered.
        let (normal_filtered, normal_idx) = filter_transfer_tracks(
            vec![t1, t2.clone(), t3],
            2,
            false,
            &dl_ids,
            &[],
            &smb_profiles,
        )
        .expect("normal mode filtered");
        assert_eq!(normal_filtered.len(), 2);
        assert_eq!(normal_idx, 1);
        assert_eq!(normal_filtered[normal_idx].id, "smb:nas:b.flac");

        assert!(filter_transfer_tracks(vec![t2], 0, true, &dl_ids, &[], &smb_profiles).is_none());
        assert_eq!(track_source_tag(&jfn), "JFN");
        assert_eq!(
            track_source_tag(&dummy_track("spf1", ProviderSource::Spotify)),
            "SPF"
        );
    }

    #[test]
    fn transfer_queue_rejects_untrusted_local_and_smb_paths() {
        let dl_ids = HashSet::new();
        let smb_profiles = vec![dummy_server("nas", ServerKind::Smb, true)];
        let malicious = vec![
            dummy_track("file:///etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("/etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("local:file:///etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("local:/etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("local:../etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:file:///etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:/etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:nas:/etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:nas:../etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:nas:%2e%2e/etc/passwd", ProviderSource::YouTubeMusic),
            dummy_track("smb:unknown:song.flac", ProviderSource::YouTubeMusic),
        ];
        assert!(
            filter_transfer_tracks(malicious.clone(), 0, false, &dl_ids, &[], &smb_profiles)
                .is_none()
        );
        assert!(filter_transfer_tracks(malicious, 0, true, &dl_ids, &[], &smb_profiles).is_none());
    }
}
