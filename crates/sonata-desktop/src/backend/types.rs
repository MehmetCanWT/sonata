//! Shared command, event, handle, and immutable snapshot types for the desktop backend.
//!
//! 1. **Commands & Navigation**: `NavTarget`, `UiWindowCmd`, and `BackendCmd` define UI-to-backend and window-lifecycle messages.
//! 2. **Snapshot**: `DesktopSnapshot` holds immutable state published via `tokio::sync::watch` for GPUI views and MPRIS.
//! 3. **Handle**: `BackendHandle` sends commands and shuts down synchronously via a bounded `sync_channel` timeout.
//! 4. **Internal Events**: `InternalEvent` and `ExportFingerprint` carry async task completions and deduplicate now-playing exports.

use sonata_core::auth::ExtractedCookies;
use sonata_core::lyrics::{active_line_index, LyricsState};
use sonata_core::player::RepeatMode;
use sonata_core::providers::servers::{ServerKind, ServerLibrary, ServerProfile};
use sonata_core::providers::ytmusic::{
    AlbumDetail, ArtistPage, HomeCard, HomeShelf, MoodCategoryTile, MoodChip, RichSearchResult,
    YtAccountProfile,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::auth::BrowserOption;
use sonata_runtime::config::AppConfig;
use sonata_runtime::discord::DiscordRpcStatus;
use sonata_runtime::downloads::{DownloadCollectionRef, DownloadedCollection, DownloadedTrack};
use sonata_runtime::i18n::AppLanguage;
use sonata_runtime::motion::MotionCover;
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
    StepCacheLimit(bool),
    ToggleMotionCovers,
    ToggleReduceMotion,
    SetLanguage(AppLanguage),
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
    SetDiscordAppId(Option<String>),
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
    },
    TestServerDraft {
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
    },
    TestServerProfile(String),
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
    Shutdown(std::sync::mpsc::SyncSender<()>),
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
    pub lyrics: LyricsState,
    pub art_path: Option<PathBuf>,
    pub motion_cover: Option<Arc<MotionCover>>,
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
    pub active_server_id: Option<String>,
    pub server_libraries: Vec<ServerLibrary>,
    pub server_artists: Vec<HomeCard>,
    pub server_albums: Vec<HomeCard>,
    pub server_playlists: Vec<Playlist>,
    pub server_tracks: Vec<Track>,
    pub server_current_folder: Option<String>,
    pub server_loading: bool,
    pub server_test_result: Option<Result<String, String>>,
}

impl DesktopSnapshot {
    pub fn is_track_downloaded(&self, track_id: &str) -> bool {
        self.downloaded_track_ids.contains(track_id)
    }

    pub fn is_track_downloading(&self, track_id: &str) -> bool {
        self.downloading_track_ids.contains(track_id)
    }

    pub fn is_collection_downloaded(&self, collection_id: &str) -> bool {
        self.downloaded_collections
            .iter()
            .any(|c| c.collection_id == collection_id)
    }
    pub fn is_track_liked(&self, track: &Track) -> bool {
        track.liked || self.liked_track_ids.contains(&track.id)
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    pub fn is_signed_in(&self) -> bool {
        self.signed_in_browser.is_some()
    }

    pub fn active_lyric_index_at(&self, position_secs: f64) -> Option<usize> {
        match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, position_secs),
            _ => None,
        }
    }

    pub fn writable_ytm_playlists(&self) -> Vec<Playlist> {
        self.ytm_playlists
            .iter()
            .filter(|p| p.source == ProviderSource::YouTubeMusic && p.is_owned && p.id != "VLLM")
            .cloned()
            .collect()
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
        profiles: Vec<ServerProfile>,
        active_id: String,
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
