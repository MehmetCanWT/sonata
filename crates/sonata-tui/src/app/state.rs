//! UI state types, events, and appearance/layout helpers for the Sonata terminal UI.
//!
//! 1. **Layout & Focus**: `SplitTarget`, `UiRects`, `FocusPane`, and `SidebarEntry` model pane geometry, focus targets, and sidebar rows.
//! 2. **Modals & Events**: `ActiveModal`, `AppEvent`, and `ExportFingerprint` represent modal dialogs, background task messages, and export deduplication.
//! 3. **Appearance & Navigation**: `App` helpers manage sidebar construction, theme/transparency cycling, Hyprland opacity, pane resizing, and config flushing.

use super::App;
use crate::theme_tui::TuiTheme;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_image::protocol::StatefulProtocol;
use sonata_core::auth::ExtractedCookies;
use sonata_core::connect::ConnectStateSnapshot;
use sonata_core::lyrics::LyricsState;
use sonata_core::player::{PlaybackState, RepeatMode};
use sonata_core::playlists::{is_personal_track_id, PersonalPlaylist};
use sonata_core::providers::local::LocalScanResult;
use sonata_core::providers::servers::{
    ServerHealthReport, ServerKind, ServerProfile, ServerSpeedTestReport,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::scrobble::ScrobbleConfig;
use sonata_core::stats::{StatsDb, StatsSummary, StatsWindow};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::hyprland;
use sonata_runtime::player::SessionCommand;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTarget {
    LeftCol,
    RightCol,
    ArtLyrics,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UiRects {
    pub search_bar: Rect,
    pub tracks_rows: Rect,
    pub sidebar_inner: Rect,
    pub album_art_box: Rect,
    pub lyrics_box: Rect,
    pub main_area: Rect,
    pub left_col_area: Rect,
    pub split_left_x: u16,
    pub split_right_x: u16,
    pub split_art_y: u16,
    pub player_bar: Rect,
    pub player_top: Rect,
    pub progress_bar: Rect,
    pub volume_bar: Rect,
    pub ctrl_prev: Rect,
    pub ctrl_play: Rect,
    pub ctrl_next: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Tracks,
    SearchInput,
}

/// Active source tab shown at the top of the TUI Library sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TuiLibrarySource {
    YouTube,
    Local,
    Jellyfin,
    Subsonic,
    Smb,
    WebDav,
}

impl TuiLibrarySource {
    pub fn from_server_kind(kind: ServerKind) -> Self {
        match kind {
            ServerKind::Jellyfin => Self::Jellyfin,
            ServerKind::Subsonic => Self::Subsonic,
            ServerKind::Smb => Self::Smb,
            ServerKind::WebDav => Self::WebDav,
        }
    }

    pub fn as_server_kind(self) -> Option<ServerKind> {
        match self {
            Self::Jellyfin => Some(ServerKind::Jellyfin),
            Self::Subsonic => Some(ServerKind::Subsonic),
            Self::Smb => Some(ServerKind::Smb),
            Self::WebDav => Some(ServerKind::WebDav),
            Self::YouTube | Self::Local => None,
        }
    }
}

/// Computes active library source tabs for the current streaming/local/server configuration.
pub fn compute_tui_library_sources(
    streaming_enabled: bool,
    local_music_enabled: bool,
    server_profiles: &[ServerProfile],
) -> Vec<TuiLibrarySource> {
    let mut out = Vec::new();
    if streaming_enabled {
        out.push(TuiLibrarySource::YouTube);
    }
    if local_music_enabled {
        out.push(TuiLibrarySource::Local);
    }
    for kind in [
        ServerKind::Jellyfin,
        ServerKind::Subsonic,
        ServerKind::Smb,
        ServerKind::WebDav,
    ] {
        if server_profiles.iter().any(|p| p.enabled && p.kind == kind) {
            out.push(TuiLibrarySource::from_server_kind(kind));
        }
    }
    out
}

/// Returns `true` when `track` may be played in the current Personal Only Mode setting.
pub fn can_play_tui_track(track: &Track, personal_only_mode: bool, is_downloaded: bool) -> bool {
    !personal_only_mode || is_personal_track_id(&track.id) || is_downloaded
}

/// Destination playlist item shown in the TUI Add-to-Playlist modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiPlaylistAddTarget {
    Personal(PersonalPlaylist),
    Ytm(Playlist),
}

/// Separates Personal Playlists from YouTube Music playlists, preventing local/server tracks
/// from ever routing to YouTube Music playlist APIs.
pub fn resolve_tui_playlist_add_targets(
    track: &Track,
    personal_playlists: &[PersonalPlaylist],
    ytm_playlists: &[Playlist],
    personal_only_mode: bool,
    ytm_authenticated: bool,
) -> Vec<TuiPlaylistAddTarget> {
    let mut targets: Vec<TuiPlaylistAddTarget> = personal_playlists
        .iter()
        .cloned()
        .map(TuiPlaylistAddTarget::Personal)
        .collect();
    if !is_personal_track_id(&track.id) && !personal_only_mode && ytm_authenticated {
        for pl in ytm_playlists {
            if pl.source == ProviderSource::YouTubeMusic && pl.id != "VLLM" {
                targets.push(TuiPlaylistAddTarget::Ytm(pl.clone()));
            }
        }
    }
    targets
}

/// Formats a compact server reachability badge from [`ServerHealthReport`].
#[cfg_attr(not(test), allow(dead_code))]
pub fn format_tui_health_badge(report: &ServerHealthReport) -> String {
    format_tui_health_badge_for(sonata_runtime::i18n::AppLanguage::En, report)
}

/// Formats a localized server reachability badge from [`ServerHealthReport`].
pub fn format_tui_health_badge_for(
    lang: sonata_runtime::i18n::AppLanguage,
    report: &ServerHealthReport,
) -> String {
    sonata_runtime::i18n::format_health_badge(lang, report.alive, report.latency_ms, false)
}

/// Formats a compact speed test badge from [`ServerSpeedTestReport`].
#[cfg_attr(not(test), allow(dead_code))]
pub fn format_tui_speed_badge(report: &ServerSpeedTestReport) -> String {
    format_tui_speed_badge_for(sonata_runtime::i18n::AppLanguage::En, report)
}

/// Formats a localized speed test badge from [`ServerSpeedTestReport`].
pub fn format_tui_speed_badge_for(
    lang: sonata_runtime::i18n::AppLanguage,
    report: &ServerSpeedTestReport,
) -> String {
    sonata_runtime::i18n::format_speed_badge(
        lang,
        report.alive,
        report.latency_ms,
        report.download_mbps,
        false,
    )
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEntry {
    Search,
    Queue,
    Downloads,
    ListeningStats,
    LocalSongs,
    LocalAlbum(usize),
    LocalFolder(usize),
    PersonalPlaylist(PersonalPlaylist),
    Server(ServerProfile),
    Playlist(Playlist),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputTarget {
    DownloadsDir,
    LocalMusicDir,
    ConnectDeviceName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastFmField {
    Enabled,
    Username,
    ApiKey,
    ApiSecret,
    SessionKey,
}

impl LastFmField {
    pub fn next(self) -> Self {
        match self {
            Self::Enabled => Self::Username,
            Self::Username => Self::ApiKey,
            Self::ApiKey => Self::ApiSecret,
            Self::ApiSecret => Self::SessionKey,
            Self::SessionKey => Self::Enabled,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Enabled => Self::SessionKey,
            Self::Username => Self::Enabled,
            Self::ApiKey => Self::Username,
            Self::ApiSecret => Self::ApiKey,
            Self::SessionKey => Self::ApiSecret,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastFmFormState {
    pub enabled: bool,
    pub username: String,
    pub api_key: String,
    pub api_secret: String,
    pub session_key: String,
    pub focused: LastFmField,
}

impl LastFmFormState {
    pub fn from_config(cfg: &ScrobbleConfig) -> Self {
        Self {
            enabled: cfg.lastfm.enabled,
            username: cfg.lastfm.username.clone(),
            api_key: cfg.lastfm.api_key.clone(),
            api_secret: cfg.lastfm.api_secret.clone(),
            session_key: cfg.lastfm.session_key.clone(),
            focused: LastFmField::Enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenBrainzField {
    Enabled,
    Username,
    Token,
}

impl ListenBrainzField {
    pub fn next(self) -> Self {
        match self {
            Self::Enabled => Self::Username,
            Self::Username => Self::Token,
            Self::Token => Self::Enabled,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Enabled => Self::Token,
            Self::Username => Self::Enabled,
            Self::Token => Self::Username,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenBrainzFormState {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub focused: ListenBrainzField,
}

impl ListenBrainzFormState {
    pub fn from_config(cfg: &ScrobbleConfig) -> Self {
        Self {
            enabled: cfg.listenbrainz.enabled,
            username: cfg.listenbrainz.username.clone(),
            token: cfg.listenbrainz.token.clone(),
            focused: ListenBrainzField::Enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsModalState {
    pub tab: usize,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerFormField {
    Kind,
    Name,
    BaseUrl,
    Share,
    Domain,
    MountPath,
    Username,
    Password,
    Token,
}

impl ServerFormField {
    pub fn next_for_kind(self, kind: ServerKind) -> Self {
        let is_smb = kind == ServerKind::Smb;
        match self {
            Self::Kind => Self::Name,
            Self::Name => Self::BaseUrl,
            Self::BaseUrl => {
                if is_smb {
                    Self::Share
                } else {
                    Self::Username
                }
            }
            Self::Share => Self::Domain,
            Self::Domain => Self::MountPath,
            Self::MountPath => Self::Username,
            Self::Username => Self::Password,
            Self::Password => {
                if is_smb {
                    Self::Kind
                } else {
                    Self::Token
                }
            }
            Self::Token => Self::Kind,
        }
    }

    pub fn prev_for_kind(self, kind: ServerKind) -> Self {
        let is_smb = kind == ServerKind::Smb;
        match self {
            Self::Kind => {
                if is_smb {
                    Self::Password
                } else {
                    Self::Token
                }
            }
            Self::Name => Self::Kind,
            Self::BaseUrl => Self::Name,
            Self::Share => Self::BaseUrl,
            Self::Domain => Self::Share,
            Self::MountPath => Self::Domain,
            Self::Username => {
                if is_smb {
                    Self::MountPath
                } else {
                    Self::BaseUrl
                }
            }
            Self::Password => Self::Username,
            Self::Token => Self::Password,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerFormState {
    pub editing_id: Option<String>,
    pub kind: ServerKind,
    pub name: String,
    pub base_url: String,
    pub share: String,
    pub domain: String,
    pub mount_path: String,
    pub username: String,
    pub password: String,
    pub token: String,
    pub focused: ServerFormField,
    pub status: Option<String>,
}

impl Default for ServerFormState {
    fn default() -> Self {
        Self {
            editing_id: None,
            kind: ServerKind::Jellyfin,
            name: String::new(),
            base_url: String::new(),
            share: String::new(),
            domain: String::new(),
            mount_path: String::new(),
            username: String::new(),
            password: String::new(),
            token: String::new(),
            focused: ServerFormField::Kind,
            status: None,
        }
    }
}

impl ServerFormState {
    pub fn from_profile(profile: &ServerProfile) -> Self {
        Self {
            editing_id: Some(profile.id.clone()),
            kind: profile.kind,
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            share: profile.share.clone().unwrap_or_default(),
            domain: profile.domain.clone().unwrap_or_default(),
            mount_path: profile
                .local_mount_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            username: profile.username.clone().unwrap_or_default(),
            password: profile.password.clone().unwrap_or_default(),
            token: profile
                .api_key
                .clone()
                .or_else(|| profile.bearer_token.clone())
                .unwrap_or_default(),
            focused: ServerFormField::Name,
            status: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerBrowserTab {
    Tracks,
    Playlists,
    Albums,
    Artists,
    Libraries,
}

impl ServerBrowserTab {
    pub fn next(self) -> Self {
        match self {
            Self::Tracks => Self::Playlists,
            Self::Playlists => Self::Albums,
            Self::Albums => Self::Artists,
            Self::Artists => Self::Libraries,
            Self::Libraries => Self::Tracks,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Tracks => Self::Libraries,
            Self::Playlists => Self::Tracks,
            Self::Albums => Self::Playlists,
            Self::Artists => Self::Albums,
            Self::Libraries => Self::Artists,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerBrowserState {
    pub server_id: String,
    pub server_name: String,
    pub server_kind: sonata_core::providers::servers::ServerKind,
    pub tab: ServerBrowserTab,
    pub selected: usize,
    pub folder_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    ImageWarning(usize),
    ThemePicker(usize, String),
    SpotifyImport(String),
    AddToPlaylist(usize),
    CreatePlaylist {
        buf: String,
        personal: bool,
    },
    RenamePersonalPlaylist {
        playlist_id: String,
        buf: String,
    },
    Settings(usize),
    TextInput {
        title: String,
        prompt: String,
        buf: String,
        target: TextInputTarget,
    },
    LocalMusicManager(usize),
    ServerManager(usize),
    AddServer(ServerFormState),
    ServerBrowser(ServerBrowserState),
    ListeningStats(StatsModalState),
    LastFmConfig(LastFmFormState),
    ListenBrainzConfig(ListenBrainzFormState),
    ConnectManager(usize),
    Help,
}

pub enum MprisCmd {
    SetPaused(bool),
    TogglePause,
    Stop,
    NextTrack,
    PrevTrack,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(u8),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
    Quit,
}

pub enum AppEvent {
    AuthReady {
        cookies: ExtractedCookies,
        ytm_playlists: Vec<Playlist>,
        spotify_playlists: Vec<Playlist>,
        imported_playlists: Vec<Playlist>,
    },
    TracksLoaded {
        req_id: u64,
        playlist_id: Option<String>,
        heading: String,
        tracks: Vec<Track>,
        silent_replace: bool,
    },
    TracksAppended {
        req_id: u64,
        heading: String,
        tracks: Vec<Track>,
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
    ArtworkReady {
        play_seq: u64,
        track_id: String,
        url: String,
        image: DynamicImage,
        png_bytes: Vec<u8>,
    },
    ArtworkEnhanced {
        play_seq: u64,
        track_id: String,
        url: String,
        image: DynamicImage,
        png_bytes: Vec<u8>,
    },
    AnimatedArtworkReady {
        play_seq: u64,
        track_id: String,
        fps: f32,
        motion_path: std::path::PathBuf,
        frames: Vec<(Option<StatefulProtocol>, Vec<u8>)>,
    },
    MotionPathReady {
        play_seq: u64,
        track_id: String,
        motion_path: std::path::PathBuf,
    },
    Mpris(MprisCmd),
    PlaylistCreated(Playlist),
    AutoplayTracksReady {
        play_seq: u64,
        tracks: Vec<Track>,
    },
    UpdateChecked(UpdateCheckResult),
    StatusMessage(String),
    DownloadsLoaded {
        tracks: Vec<sonata_runtime::downloads::DownloadedTrack>,
        collections: Vec<sonata_runtime::downloads::DownloadedCollection>,
        total_bytes: u64,
    },
    TrackDownloadFinished {
        track_id: String,
        title: String,
        result: Result<(), String>,
    },
    CollectionDownloadFinished {
        title: String,
        track_ids: Vec<String>,
        saved: usize,
        errors: usize,
    },
    ServerTested {
        server_id: String,
        result: Result<String, String>,
    },
    ServerSaved {
        profile: sonata_core::providers::servers::ServerProfile,
        message: String,
    },
    ServerBrowseLoaded {
        req_id: u64,
        server_id: String,
        folder_path: Option<String>,
        libraries: Vec<sonata_core::providers::servers::ServerLibrary>,
        artists: Vec<sonata_core::providers::ytmusic::HomeCard>,
        albums: Vec<sonata_core::providers::ytmusic::HomeCard>,
        playlists: Vec<Playlist>,
        tracks: Vec<Track>,
        folders: Vec<String>,
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
    StatsDbOpened(Option<StatsDb>),
    StatsLoaded {
        window: StatsWindow,
        summary: StatsSummary,
    },
    ScrobbleError(Option<String>),
}

/// Maps a Sonata track to its canonical short source tag (`LOC`, `SMB`, `JFN`, `SUB`, `DAV`, `JIO`, `SPF`, `YTM`).
pub fn tui_track_source_tag(track: &Track) -> &'static str {
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
    } else if track
        .id
        .starts_with(sonata_core::providers::jiosaavn::ID_PREFIX)
    {
        "JIO"
    } else if track.source == ProviderSource::Spotify {
        "SPF"
    } else {
        "YTM"
    }
}

/// Builds a live [`ConnectStateSnapshot`] from current TUI playback state.
pub fn build_tui_connect_snapshot(
    device_name: &str,
    playback: &PlaybackState,
) -> ConnectStateSnapshot {
    let current = playback.current_track();
    let playing = current.is_some() && !playback.paused && !playback.loading_track;
    ConnectStateSnapshot {
        device_id: String::new(),
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
        volume: playback.volume.min(100),
        shuffle: playback.shuffle,
        repeat_mode: playback.repeat,
        queue: playback.queue.clone(),
        current_index: playback.current_index,
        updated_at_unix: 0,
    }
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

/// Validates a track ID received from a remote Sonata Connect `TransferQueue` request.
pub fn is_safe_tui_transfer_track_id(
    track_id: &str,
    personal_only_mode: bool,
    local_roots: &[std::path::PathBuf],
    server_profiles: &[ServerProfile],
) -> bool {
    if track_id.is_empty()
        || track_id.contains('\0')
        || track_id.starts_with("file://")
        || track_id.starts_with('/')
        || track_id.starts_with('\\')
    {
        return false;
    }
    if let Some(rest) = track_id.strip_prefix("local:") {
        if personal_only_mode {
            return false;
        }
        let decoded = percent_decode_lossy(rest);
        if rest.starts_with("file://")
            || decoded.starts_with("file://")
            || rest.contains('\0')
            || decoded.contains('\0')
            || has_parent_traversal(rest)
            || has_parent_traversal(&decoded)
        {
            return false;
        }
        if local_roots.is_empty() {
            return false;
        }
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
        if rest.starts_with("file://")
            || rest.starts_with('/')
            || rest.starts_with('\\')
            || rest.contains('\0')
        {
            return false;
        }
        let Some((server_id, rel_path)) = rest.split_once(':') else {
            return false;
        };
        if server_id.is_empty() || rel_path.is_empty() {
            return false;
        }
        let decoded = percent_decode_lossy(rel_path);
        if rel_path.starts_with("file://")
            || decoded.starts_with("file://")
            || rel_path.starts_with('/')
            || decoded.starts_with('/')
            || rel_path.starts_with('\\')
            || decoded.starts_with('\\')
            || has_parent_traversal(rel_path)
            || has_parent_traversal(&decoded)
        {
            return false;
        }
        return server_profiles
            .iter()
            .any(|p| p.enabled && p.kind == ServerKind::Smb && p.id == server_id);
    }
    true
}

/// Filters Connect `TransferQueue` tracks for safety, configured roots, and Personal Only Mode.
pub fn filter_tui_transfer_tracks(
    tracks: Vec<Track>,
    start_index: usize,
    personal_only_mode: bool,
    downloaded_track_ids: &HashSet<String>,
    local_roots: &[std::path::PathBuf],
    server_profiles: &[ServerProfile],
) -> Option<(Vec<Track>, usize)> {
    if tracks.is_empty() {
        return None;
    }
    let target_id = tracks.get(start_index).map(|t| t.id.clone());
    let filtered: Vec<Track> = tracks
        .into_iter()
        .filter(|t| {
            is_safe_tui_transfer_track_id(&t.id, personal_only_mode, local_roots, server_profiles)
                && can_play_tui_track(t, personal_only_mode, downloaded_track_ids.contains(&t.id))
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
pub struct ExportFingerprint {
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

impl App {
    pub fn active_library_sources(&self) -> Vec<TuiLibrarySource> {
        compute_tui_library_sources(
            self.config.streaming_enabled(),
            self.config.local_music_enabled,
            &self.server_profiles,
        )
    }

    pub fn cycle_library_source(&mut self, forward: bool) {
        let sources = self.active_library_sources();
        if sources.is_empty() {
            return;
        }
        let cur = sources
            .iter()
            .position(|s| *s == self.active_source)
            .unwrap_or(0);
        let next = if forward {
            (cur + 1) % sources.len()
        } else {
            (cur + sources.len() - 1) % sources.len()
        };
        self.active_source = sources[next];
        self.rebuild_sidebar();
        let s = sonata_runtime::i18n::strings(self.config.language);
        let label = match self.active_source {
            TuiLibrarySource::YouTube => s.youtube,
            TuiLibrarySource::Local => s.local,
            TuiLibrarySource::Jellyfin => s.jellyfin,
            TuiLibrarySource::Subsonic => s.subsonic,
            TuiLibrarySource::Smb => s.smb,
            TuiLibrarySource::WebDav => s.webdav,
        };
        self.status_line = format!("{}: {label}", s.sources);
    }

    pub fn rebuild_sidebar(&mut self) {
        let sources = self.active_library_sources();
        if !sources.is_empty() && !sources.contains(&self.active_source) {
            self.active_source = sources[0];
        }
        let mut items = vec![
            SidebarEntry::Search,
            SidebarEntry::Queue,
            SidebarEntry::Downloads,
            SidebarEntry::ListeningStats,
        ];
        match self.active_source {
            TuiLibrarySource::YouTube => {
                for ppl in &self.personal_playlists {
                    items.push(SidebarEntry::PersonalPlaylist(ppl.clone()));
                }
                if !self.config.personal_only_mode {
                    if self.config.provider_mode.uses_ytm() {
                        for p in &self.ytm_playlists {
                            items.push(SidebarEntry::Playlist(p.clone()));
                        }
                    }
                    if self.config.provider_mode.uses_spotify() {
                        for p in &self.spotify_playlists {
                            items.push(SidebarEntry::Playlist(p.clone()));
                        }
                        for p in &self.imported_playlists {
                            if !items.iter().any(|existing| match existing {
                                SidebarEntry::Playlist(ep) => ep.id == p.id,
                                _ => false,
                            }) {
                                items.push(SidebarEntry::Playlist(p.clone()));
                            }
                        }
                    }
                }
            }
            TuiLibrarySource::Local => {
                if self.config.local_music_enabled {
                    items.push(SidebarEntry::LocalSongs);
                    for i in 0..self.local_scan.albums.len() {
                        items.push(SidebarEntry::LocalAlbum(i));
                    }
                    for i in 0..self.local_scan.folders.len() {
                        items.push(SidebarEntry::LocalFolder(i));
                    }
                }
                for ppl in &self.personal_playlists {
                    items.push(SidebarEntry::PersonalPlaylist(ppl.clone()));
                }
            }
            TuiLibrarySource::Jellyfin
            | TuiLibrarySource::Subsonic
            | TuiLibrarySource::Smb
            | TuiLibrarySource::WebDav => {
                if let Some(kind) = self.active_source.as_server_kind() {
                    for server in &self.server_profiles {
                        if server.enabled && server.kind == kind {
                            items.push(SidebarEntry::Server(server.clone()));
                        }
                    }
                }
                for ppl in &self.personal_playlists {
                    items.push(SidebarEntry::PersonalPlaylist(ppl.clone()));
                }
            }
        }
        self.sidebar_items = items;
        if self.sidebar_idx >= self.sidebar_items.len() {
            self.sidebar_idx = 0;
        }
    }

    pub fn toggle_mini_mode(&mut self) {
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.mini_mode = !self.mini_mode;
        self.clear_kitty_images();
        self.status_line = if self.mini_mode {
            s.mini_mode_hint.to_string()
        } else {
            s.full_mode.to_string()
        };
    }

    pub fn effective_transparent_bg(&self) -> bool {
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

    fn ensure_hyprland_window(&mut self) -> Option<String> {
        if hyprland::is_hyprland() {
            if let Some(fresh) = hyprland::find_terminal_window_address() {
                self.hyprland_window = Some(fresh);
            }
        }
        self.hyprland_window.clone()
    }

    pub fn sync_appearance(&mut self) {
        let transparent = self.effective_transparent_bg();
        self.theme = TuiTheme::from_core(&self.base_theme.with_transparency(transparent));
        if let Color::Rgb(r, g, b) = self.theme.bg {
            self.picker.set_background_color([r, g, b, 255]);
        }
        if let Some(addr) = self.ensure_hyprland_window() {
            let opacity = if transparent {
                self.effective_hyprland_opacity()
            } else {
                1.0
            };
            tokio::spawn(async move {
                hyprland::set_window_opacity(&addr, opacity).await;
            });
        }
    }

    pub async fn restore_hyprland_opacity(&mut self) {
        if let Some(addr) = self.ensure_hyprland_window() {
            let _ = tokio::time::timeout(
                std::time::Duration::from_millis(500),
                hyprland::set_window_opacity(&addr, 1.0),
            )
            .await;
        }
    }

    pub fn toggle_jiosaavn(&mut self) {
        self.config.jiosaavn_enabled = !self.config.jiosaavn_enabled;
        let effective = self.config.effective_jiosaavn_enabled();
        self.resolver
            .update_profile(|profile| profile.jiosaavn_enabled = effective);
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::UpdatePolicy {
            policy: self.session_policy(),
            trigger_upgrade: effective,
        });
        let i18n = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            i18n.jiosaavn_module,
            if self.config.personal_only_mode && self.config.jiosaavn_enabled {
                i18n.disabled
            } else if self.config.jiosaavn_enabled {
                i18n.enabled
            } else {
                i18n.disabled
            }
        );
    }

    pub fn toggle_personal_only_mode(&mut self) {
        self.config.personal_only_mode = !self.config.personal_only_mode;
        let effective_jio = self.config.effective_jiosaavn_enabled();
        self.resolver
            .update_profile(|profile| profile.jiosaavn_enabled = effective_jio);
        self.save_config_async();
        self.enqueue_session_cmd(SessionCommand::UpdatePolicy {
            policy: self.session_policy(),
            trigger_upgrade: false,
        });
        if self.config.personal_only_mode {
            self.anim_art_frames.clear();
            self.anim_art_idx = 0;
            self.clear_kitty_images();
            if let Some(cur) = self.playback.current_track() {
                if !can_play_tui_track(cur, true, self.downloaded_track_ids.contains(&cur.id)) {
                    self.enqueue_session_cmd(SessionCommand::Stop);
                }
            }
            let s = sonata_runtime::i18n::strings(self.config.language);
            self.status_line = s.personal_only_enabled_preserved.to_string();
        } else {
            let s = sonata_runtime::i18n::strings(self.config.language);
            self.status_line = s.streaming_restored.to_string();
            self.start_background_init();
        }
        self.rebuild_sidebar();
    }

    pub fn toggle_attribute_personal_to_ytm(&mut self) {
        self.config.scrobble_servers_to_ytm = !self.config.scrobble_servers_to_ytm;
        self.save_config_async();
        let i18n = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            i18n.attribute_personal_to_ytm,
            if self.config.scrobble_servers_to_ytm {
                i18n.enabled
            } else {
                i18n.disabled
            }
        );
    }

    pub fn toggle_local_music_enabled(&mut self) {
        self.config.local_music_enabled = !self.config.local_music_enabled;
        self.save_config_async();
        if self.config.local_music_enabled {
            self.scan_local_music();
        } else {
            self.local_scan_req_id = self.local_scan_req_id.wrapping_add(1);
            self.local_scanning = false;
            self.local_scan = LocalScanResult::default();
            let s = sonata_runtime::i18n::strings(self.config.language);
            self.status_line = s.local_music_disabled.to_string();
        }
        self.rebuild_sidebar();
    }

    pub fn add_local_music_dir(&mut self, path: std::path::PathBuf) {
        let trimmed = path.to_string_lossy().trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let clean = std::path::PathBuf::from(trimmed);
        if !self.config.local_music_dirs.contains(&clean) {
            self.config.local_music_dirs.push(clean);
            self.save_config_async();
        }
        self.scan_local_music();
    }

    pub fn remove_local_music_dir(&mut self, idx: usize) {
        if idx < self.config.local_music_dirs.len() {
            self.config.local_music_dirs.remove(idx);
            self.save_config_async();
            if self.config.local_music_enabled {
                self.scan_local_music();
            }
        }
    }

    pub fn toggle_motion_covers(&mut self) {
        self.config.motion_covers = !self.config.motion_covers;
        self.save_config_async();
        if !self.config.motion_covers {
            self.anim_art_frames.clear();
            self.motion_cover_path = None;
            self.clear_kitty_images();
        }
        let i18n = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            i18n.motion_covers,
            if self.config.motion_covers {
                i18n.enabled
            } else {
                i18n.disabled
            }
        );
    }

    pub fn toggle_transparent_bg(&mut self) {
        self.config.transparent_bg = !self.config.transparent_bg;
        self.base_theme.transparent_bg = None;
        self.save_config_async();
        self.sync_appearance();
        let i18n = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            i18n.tui_transparent_bg,
            if self.config.transparent_bg {
                i18n.enabled
            } else {
                i18n.disabled
            }
        );
    }

    pub fn cycle_hyprland_opacity(&mut self, forward: bool) {
        let s = sonata_runtime::i18n::strings(self.config.language);
        if !hyprland::is_hyprland() {
            self.status_line = s.hyprland_opacity_locked_msg.to_string();
            return;
        }
        const STEPS: &[f32] = &[0.50, 0.60, 0.70, 0.80, 0.90, 1.00];
        let cur = self.config.hyprland_opacity;
        let idx = STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((*a - cur).abs())
                    .partial_cmp(&((*b - cur).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(4);
        let next = if forward {
            (idx + 1) % STEPS.len()
        } else {
            (idx + STEPS.len() - 1) % STEPS.len()
        };
        self.config.hyprland_opacity = STEPS[next];
        self.base_theme.hyprland_opacity = None;
        self.save_config_async();
        let _ = self.ensure_hyprland_window();
        self.sync_appearance();
        self.status_line = format!(
            "{}: {:.0}%",
            s.tui_hypr_opacity,
            self.config.hyprland_opacity * 100.0
        );
    }

    pub fn cycle_theme(&mut self, forward: bool) {
        if self.themes.is_empty() {
            return;
        }
        let cur = self
            .themes
            .iter()
            .position(|t| t.name == self.base_theme.name)
            .unwrap_or(0);
        let next = if forward {
            (cur + 1) % self.themes.len()
        } else {
            (cur + self.themes.len() - 1) % self.themes.len()
        };
        self.apply_theme_by_index(next);
        self.config.theme = self.base_theme.name.clone();
        self.save_config_async();
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.theme_label,
            sonata_runtime::i18n::display_theme_name(self.config.language, &self.base_theme.name)
        );
    }

    pub fn apply_theme_by_index(&mut self, idx: usize) {
        if let Some(t) = self.themes.get(idx).cloned() {
            self.base_theme = t;
            self.sync_appearance();
        }
    }

    pub fn adjust_focused_pane_width(&mut self, delta_pct: i16) {
        match self.focus {
            FocusPane::Sidebar => {
                let next = (self.config.right_pane_pct as i16 + delta_pct).clamp(14, 48) as u16;
                if 100u16.saturating_sub(self.config.left_pane_pct + next) >= 20 {
                    self.config.right_pane_pct = next;
                    self.save_config_async();
                }
            }
            FocusPane::Tracks | FocusPane::SearchInput => {
                let next = (self.config.left_pane_pct as i16 + delta_pct).clamp(18, 52) as u16;
                if 100u16.saturating_sub(next + self.config.right_pane_pct) >= 20 {
                    self.config.left_pane_pct = next;
                    self.save_config_async();
                }
            }
        }
    }

    pub fn flush_config(&mut self) {
        if self.config_dirty {
            self.config_dirty = false;
            self.save_config_async();
        }
    }

    pub(super) fn selected_or_playing_track(&self) -> Option<Track> {
        if self.focus == FocusPane::Tracks {
            if let Some(idx) = self.track_table_state.selected() {
                if let Some(t) = self.visible_tracks.get(idx) {
                    return Some(t.clone());
                }
            }
        }
        self.playback.current_track().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tui_connect_snapshot, can_play_tui_track, compute_tui_library_sources,
        filter_tui_transfer_tracks, format_tui_health_badge, format_tui_speed_badge,
        resolve_tui_playlist_add_targets, tui_track_source_tag, ServerBrowserTab, ServerFormField,
        TuiLibrarySource, TuiPlaylistAddTarget,
    };
    use sonata_core::player::PlaybackState;
    use sonata_core::playlists::PersonalPlaylist;
    use sonata_core::providers::servers::{
        ServerHealthReport, ServerKind, ServerProfile, ServerSpeedTestReport,
    };
    use sonata_core::providers::{Playlist, ProviderSource, Track};
    use sonata_runtime::config::AppConfig;
    use std::collections::HashSet;

    fn dummy_track(id: &str) -> Track {
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
            source: ProviderSource::YouTubeMusic,
        }
    }

    fn dummy_server(id: &str, kind: ServerKind, enabled: bool) -> ServerProfile {
        ServerProfile {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            base_url: "http://127.0.0.1:8096".to_string(),
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
    fn server_form_and_browser_tab_cycling_wraps_cleanly() {
        let mut field = ServerFormField::Kind;
        for _ in 0..8 {
            field = field.next_for_kind(ServerKind::Smb);
        }
        assert_eq!(field, ServerFormField::Kind);
        assert_eq!(
            ServerFormField::Kind.prev_for_kind(ServerKind::Smb),
            ServerFormField::Password
        );
        assert_eq!(
            ServerFormField::Kind.prev_for_kind(ServerKind::WebDav),
            ServerFormField::Token
        );

        let mut non_smb = ServerFormField::Kind;
        for _ in 0..6 {
            non_smb = non_smb.next_for_kind(ServerKind::Jellyfin);
        }
        assert_eq!(non_smb, ServerFormField::Kind);

        let mut tab = ServerBrowserTab::Tracks;
        for _ in 0..5 {
            tab = tab.next();
        }
        assert_eq!(tab, ServerBrowserTab::Tracks);
        assert_eq!(ServerBrowserTab::Tracks.prev(), ServerBrowserTab::Libraries);
    }

    #[test]
    fn tui_library_sources_and_personal_only_gating() {
        let profiles = vec![
            dummy_server("jf", ServerKind::Jellyfin, true),
            dummy_server("sub", ServerKind::Subsonic, true),
            dummy_server("smb", ServerKind::Smb, true),
            dummy_server("dav", ServerKind::WebDav, false),
        ];
        assert_eq!(
            compute_tui_library_sources(true, true, &profiles),
            vec![
                TuiLibrarySource::YouTube,
                TuiLibrarySource::Local,
                TuiLibrarySource::Jellyfin,
                TuiLibrarySource::Subsonic,
                TuiLibrarySource::Smb,
            ]
        );
        assert_eq!(
            compute_tui_library_sources(false, true, &profiles),
            vec![
                TuiLibrarySource::Local,
                TuiLibrarySource::Jellyfin,
                TuiLibrarySource::Subsonic,
                TuiLibrarySource::Smb,
            ]
        );

        let local = dummy_track("local:/flac/track.flac");
        let smb = dummy_track("smb:nas:track.flac");
        let ytm = dummy_track("abc123def45");
        assert!(can_play_tui_track(&local, true, false));
        assert!(can_play_tui_track(&smb, true, false));
        assert!(!can_play_tui_track(&ytm, true, false));
        assert!(can_play_tui_track(&ytm, true, true));
        assert!(can_play_tui_track(&ytm, false, false));

        let mut cfg = AppConfig {
            jiosaavn_enabled: true,
            scrobble_servers_to_ytm: true,
            personal_only_mode: true,
            ..Default::default()
        };
        assert!(!cfg.streaming_enabled());
        assert!(!cfg.effective_jiosaavn_enabled());
        assert!(!cfg.effective_attribute_personal_to_ytm());

        cfg.personal_only_mode = false;
        assert!(cfg.streaming_enabled());
        assert!(cfg.effective_jiosaavn_enabled());
        assert!(cfg.effective_attribute_personal_to_ytm());
    }

    #[test]
    fn tui_personal_playlist_routing_blocks_ytm_for_personal_tracks() {
        let personal = PersonalPlaylist {
            id: "personal:1".to_string(),
            name: "Local FLAC".to_string(),
            tracks: vec![],
            created_at_epoch_secs: 1,
            updated_at_epoch_secs: 1,
        };
        let ytm = Playlist {
            id: "VLPL1".to_string(),
            name: "YTM Favorites".to_string(),
            source: ProviderSource::YouTubeMusic,
            track_count: Some(2),
            thumbnail_url: None,
            is_owned: true,
        };

        let smb_track = dummy_track("smb:share:01.flac");
        let targets = resolve_tui_playlist_add_targets(
            &smb_track,
            std::slice::from_ref(&personal),
            std::slice::from_ref(&ytm),
            false,
            true,
        );
        assert_eq!(
            targets,
            vec![TuiPlaylistAddTarget::Personal(personal.clone())]
        );

        let ytm_track = dummy_track("xyz987uvw65");
        let targets_all = resolve_tui_playlist_add_targets(
            &ytm_track,
            std::slice::from_ref(&personal),
            std::slice::from_ref(&ytm),
            false,
            true,
        );
        assert_eq!(
            targets_all,
            vec![
                TuiPlaylistAddTarget::Personal(personal),
                TuiPlaylistAddTarget::Ytm(ytm),
            ]
        );
    }

    #[test]
    fn tui_health_and_speed_badges_format_cleanly() {
        let health = ServerHealthReport {
            alive: true,
            latency_ms: Some(9),
            message: "ok".to_string(),
        };
        assert_eq!(format_tui_health_badge(&health), "Alive 9ms");

        let speed = ServerSpeedTestReport {
            alive: true,
            latency_ms: 11,
            download_mbps: Some(84.2),
            bytes_tested: 1024,
            sample_note: "ok".to_string(),
        };
        assert_eq!(format_tui_speed_badge(&speed), "84.2 Mbps (11ms)");
    }

    #[test]
    fn tui_connect_snapshot_transfer_filter_and_source_tags() {
        let mut playback = PlaybackState::new(64);
        let t1 = dummy_track("local:/a.flac");
        let t2 = dummy_track("ytm_abc");
        let t3 = dummy_track("subsonic:s1:9");
        playback.queue = vec![t1.clone(), t2.clone(), t3.clone()];
        playback.current_index = Some(0);
        playback.paused = false;

        let snap = build_tui_connect_snapshot("TUI Desk", &playback);
        assert!(snap.playing);
        assert_eq!(snap.volume, 64);
        assert_eq!(snap.track_id.as_deref(), Some("local:/a.flac"));

        let dl = HashSet::new();
        let (filtered, idx) =
            filter_tui_transfer_tracks(vec![t1, t2.clone(), t3], 2, true, &dl, &[], &[])
                .expect("filtered");
        assert_eq!(filtered.len(), 1);
        assert_eq!(idx, 0);
        assert_eq!(filtered[idx].id, "subsonic:s1:9");
        assert!(filter_tui_transfer_tracks(vec![t2], 0, true, &dl, &[], &[]).is_none());
        assert!(filter_tui_transfer_tracks(
            vec![
                dummy_track("smb:file:///etc/passwd"),
                dummy_track("smb:/etc/passwd"),
                dummy_track("smb:nas:../etc/passwd"),
                dummy_track("local:/etc/passwd"),
            ],
            0,
            false,
            &dl,
            &[],
            &[],
        )
        .is_none());

        assert_eq!(tui_track_source_tag(&dummy_track("smb:nas:1")), "SMB");
        assert_eq!(tui_track_source_tag(&dummy_track("webdav:d:1")), "DAV");
    }

    #[test]
    fn tui_scrobble_forms_cycle_fields_and_roundtrip_config() {
        use super::{LastFmField, LastFmFormState, ListenBrainzField, ListenBrainzFormState};
        use sonata_core::scrobble::{LastFmConfig, ListenBrainzConfig, ScrobbleConfig};

        let mut f = LastFmField::Enabled;
        for _ in 0..5 {
            f = f.next();
        }
        assert_eq!(f, LastFmField::Enabled);
        assert_eq!(LastFmField::Enabled.prev(), LastFmField::SessionKey);

        let mut lb = ListenBrainzField::Enabled;
        for _ in 0..3 {
            lb = lb.next();
        }
        assert_eq!(lb, ListenBrainzField::Enabled);
        assert_eq!(ListenBrainzField::Enabled.prev(), ListenBrainzField::Token);

        let cfg = ScrobbleConfig {
            lastfm: LastFmConfig {
                enabled: true,
                api_key: "k1".to_string(),
                api_secret: "s1".to_string(),
                session_key: "sk1".to_string(),
                username: "alice".to_string(),
            },
            listenbrainz: ListenBrainzConfig {
                enabled: true,
                token: "tok1".to_string(),
                username: "alice_lb".to_string(),
            },
        };
        let lfm_form = LastFmFormState::from_config(&cfg);
        assert!(lfm_form.enabled);
        assert_eq!(lfm_form.username, "alice");
        assert_eq!(lfm_form.api_key, "k1");

        let lb_form = ListenBrainzFormState::from_config(&cfg);
        assert!(lb_form.enabled);
        assert_eq!(lb_form.username, "alice_lb");
        assert_eq!(lb_form.token, "tok1");
    }
}
