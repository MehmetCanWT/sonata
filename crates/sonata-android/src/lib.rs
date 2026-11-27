//! Cancellable async UniFFI Android bridge exposing `sonata-core` to Kotlin and Jetpack Compose.
//!
//! 1. **Engine Lifecycle**: `SonataAndroidEngine` owns the shared HTTP client, `CorePaths`,
//!    SQLite `CacheDb`, `YtMusicClient`, and `StreamResolver`, initialized with Android's private
//!    directories and any persisted YouTube Music WebView session cookies. Async methods execute on
//!    UniFFI's Tokio runtime (`#[uniffi::export(async_runtime = "tokio")]`) with native Kotlin
//!    coroutine cancellation and no private blocking executor.
//! 2. **FFI Models & Errors**: `AndroidTrack`, `AndroidPlaylist`, `AndroidAccountProfile`,
//!    `AndroidHomeShelf`, `AndroidHomeCard`, `AndroidLyricLine`, `AndroidResolvedStream`, and typed
//!    redacted `SonataAndroidError` variants mirror `sonata-core` contracts.
//! 3. **Async Operations & Blocking Hygiene**: Network, resolver, session, and disk/SQLite cache
//!    operations are exposed as `async fn` (with blocking I/O offloaded to `tokio::task::spawn_blocking`),
//!    while tiny memory-only state checks (`is_signed_in`, `set_js_cipher_solver`) remain synchronous.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::{CacheDb, DEFAULT_CACHE_LIMIT_BYTES};
use sonata_core::error::CoreError;
use sonata_core::lyrics::{lrclib, LyricsState};
use sonata_core::motion::{MotionMedia, MotionMediaKind};
use sonata_core::paths::CorePaths;
use sonata_core::playlists::{PersonalPlaylist, PersonalPlaylistStore};
use sonata_core::providers::hifi::{
    FastStartMode, JsCipherBridge, ResolvedMusicVideo, ResolvedStream, ResolverProfile,
    SolvedCipherPair, StreamFallbackMode, StreamResolver, UpgradeDelivery, VideoQualityStream,
};
use sonata_core::providers::jiosaavn::{self, ResolvedJioSaavnStream};
use sonata_core::providers::servers::{
    encode_server_item_id, jellyfin, load_server_profiles, parse_server_item_id,
    save_server_profiles, webdav, ServerClient, ServerHealthReport, ServerKind, ServerLibrary,
    ServerProfile, ServerSearchResult, ServerSpeedTestReport, ServerStream,
};
use sonata_core::providers::ytmusic::{
    ArtistPage, HomeCard, HomeCardKind, HomeShelf, MoodCategoryTile, MoodChip, MusicVideoCandidate,
    MusicVideoSyncSegment, YtAccountProfile, YtMusicClient,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::scrobble::{
    LastFmConfig, ListenBrainzConfig, ScrobbleClient, ScrobbleConfig, ScrobbleTrack,
};
use sonata_core::stats::{
    AlbumStat, ArtistStat, SourceStat, StatsDb, StatsSummary, StatsWindow, TrackStat,
};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_PROFILE_FILE: &str = "android_profile.json";
const CACHE_HOME_FILE: &str = "android_home.json";
const CACHE_PLAYLISTS_FILE: &str = "android_playlists.json";

fn save_cached_json_in<T: Serialize>(dir: &Path, file_name: &str, value: &T) {
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    #[cfg(unix)]
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));

    let Ok(bytes) = serde_json::to_vec(value) else {
        return;
    };
    let target = dir.join(file_name);
    let tmp = dir.join(format!("{file_name}.{}.tmp", std::process::id()));
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    if let Ok(mut file) = opts.open(&tmp) {
        if file.write_all(&bytes).is_ok() && file.sync_all().is_ok() {
            #[cfg(unix)]
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
            if fs::rename(&tmp, &target).is_err() {
                let _ = fs::remove_file(&tmp);
            }
        } else {
            let _ = fs::remove_file(&tmp);
        }
    }
}

fn load_cached_json_from<T: DeserializeOwned>(dir: &Path, file_name: &str) -> Option<T> {
    let path = dir.join(file_name);
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn remove_cached_json_from(dir: &Path, file_name: &str) {
    let path: PathBuf = Path::join(dir, file_name);
    let _ = fs::remove_file(path);
}

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SonataAndroidError {
    #[error("{reason}")]
    Engine { reason: String },

    #[error("{reason}")]
    AuthRequired { reason: String },

    #[error("{reason}")]
    RateLimited {
        retry_after_secs: Option<u64>,
        reason: String,
    },

    #[error("{reason}")]
    Offline { reason: String },

    #[error("{reason}")]
    Timeout { reason: String },

    #[error("{reason}")]
    Cancelled { reason: String },

    #[error("{reason}")]
    NotFound { reason: String },

    #[error("{reason}")]
    InvalidResponse { provider: String, reason: String },

    #[error("{reason}")]
    Cache { reason: String },

    #[error("{reason}")]
    Io { reason: String },

    #[error("{reason}")]
    Internal { reason: String },
}

impl From<CoreError> for SonataAndroidError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::AuthRequired { detail } => Self::AuthRequired { reason: detail },
            CoreError::RateLimited {
                retry_after,
                detail,
            } => Self::RateLimited {
                retry_after_secs: retry_after.map(|d| d.as_secs()),
                reason: detail,
            },
            CoreError::Offline { detail } => Self::Offline { reason: detail },
            CoreError::Timeout { detail } => Self::Timeout { reason: detail },
            CoreError::Cancelled { detail } => Self::Cancelled { reason: detail },
            CoreError::NotFound { detail } => Self::NotFound { reason: detail },
            CoreError::InvalidResponse { provider, detail } => Self::InvalidResponse {
                provider,
                reason: detail,
            },
            CoreError::ExternalTool { detail, .. } | CoreError::Internal { detail } => {
                Self::Internal { reason: detail }
            }
            CoreError::Cache { detail } => Self::Cache { reason: detail },
            CoreError::Io { detail } => Self::Io { reason: detail },
        }
    }
}

impl From<anyhow::Error> for SonataAndroidError {
    fn from(err: anyhow::Error) -> Self {
        Self::from(CoreError::classify_anyhow(&err))
    }
}

fn android_core_paths(config_dir: &str, cache_dir: &str) -> CorePaths {
    CorePaths::for_app_roots(config_dir, cache_dir)
}

fn android_resolver_profile() -> ResolverProfile {
    ResolverProfile {
        fast_start: FastStartMode::DirectUrl,
        upgrade_delivery: UpgradeDelivery::ReturnRemoteStream,
        fallback: StreamFallbackMode::InnerTubeOnly,
        bootstrap_timeout: std::time::Duration::from_millis(8500),
        jiosaavn_enabled: false,
        cache_limit_bytes: DEFAULT_CACHE_LIMIT_BYTES,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidJioSaavnStream {
    pub url: String,
    pub bitrate_kbps: u32,
    pub badge: String,
    pub title: String,
    pub artist: String,
    pub duration_secs: u64,
}

impl From<ResolvedJioSaavnStream> for AndroidJioSaavnStream {
    fn from(s: ResolvedJioSaavnStream) -> Self {
        Self {
            url: s.url,
            bitrate_kbps: s.bitrate_kbps,
            badge: s.badge,
            title: s.title,
            artist: s.artist,
            duration_secs: s.duration_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidMotionMedia {
    pub url: String,
    pub fallback_url: Option<String>,
    pub portrait_url: Option<String>,
    pub is_hls: bool,
    pub source: String,
}

impl From<MotionMedia> for AndroidMotionMedia {
    fn from(media: MotionMedia) -> Self {
        Self {
            url: media.url,
            fallback_url: media.fallback_url,
            portrait_url: media.portrait_url,
            is_hls: media.kind == MotionMediaKind::Hls,
            source: media.source.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidAccountProfile {
    pub name: String,
    pub email: Option<String>,
    pub handle: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<YtAccountProfile> for AndroidAccountProfile {
    fn from(p: YtAccountProfile) -> Self {
        Self {
            name: p.name,
            email: p.email,
            handle: p.handle,
            avatar_url: p.avatar_url,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidPlaylist {
    pub id: String,
    pub name: String,
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    #[uniffi(default = false)]
    pub is_owned: bool,
}

impl From<&Playlist> for AndroidPlaylist {
    fn from(p: &Playlist) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            thumbnail_url: p.thumbnail_url.clone(),
            is_owned: p.is_owned,
        }
    }
}

impl From<Playlist> for AndroidPlaylist {
    fn from(p: Playlist) -> Self {
        Self {
            id: p.id,
            name: p.name,
            thumbnail_url: p.thumbnail_url,
            is_owned: p.is_owned,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub liked: bool,
    #[serde(default)]
    #[uniffi(default = None)]
    pub album_id: Option<String>,
    #[serde(default)]
    #[uniffi(default = false)]
    pub is_explicit: bool,
}

impl From<&Track> for AndroidTrack {
    fn from(t: &Track) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            duration_secs: t.duration_secs,
            thumbnail_url: t.thumbnail_url.clone(),
            liked: t.liked,
            album_id: t.album_id.clone(),
            is_explicit: t.is_explicit,
        }
    }
}

impl From<Track> for AndroidTrack {
    fn from(t: Track) -> Self {
        Self {
            id: t.id,
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_secs: t.duration_secs,
            thumbnail_url: t.thumbnail_url,
            liked: t.liked,
            album_id: t.album_id,
            is_explicit: t.is_explicit,
        }
    }
}

impl From<AndroidTrack> for Track {
    fn from(t: AndroidTrack) -> Self {
        Self {
            id: t.id,
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_secs: t.duration_secs,
            thumbnail_url: t.thumbnail_url,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: t.liked,
            album_id: t.album_id,
            is_explicit: t.is_explicit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidHomeCardKind {
    Playlist,
    Album,
    Artist,
    Song,
    Video,
    Mood,
}

impl From<HomeCardKind> for AndroidHomeCardKind {
    fn from(k: HomeCardKind) -> Self {
        match k {
            HomeCardKind::Playlist => Self::Playlist,
            HomeCardKind::Album => Self::Album,
            HomeCardKind::Artist => Self::Artist,
            HomeCardKind::Song => Self::Song,
            HomeCardKind::Video => Self::Video,
            HomeCardKind::Mood => Self::Mood,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidHomeCard {
    pub kind: AndroidHomeCardKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub track: Option<AndroidTrack>,
}

impl From<&HomeCard> for AndroidHomeCard {
    fn from(c: &HomeCard) -> Self {
        Self {
            kind: c.kind.into(),
            id: c.id.clone(),
            title: c.title.clone(),
            subtitle: c.subtitle.clone(),
            thumbnail_url: c.thumbnail_url.clone(),
            track: c.track.as_ref().map(AndroidTrack::from),
        }
    }
}

impl From<HomeCard> for AndroidHomeCard {
    fn from(c: HomeCard) -> Self {
        Self {
            kind: c.kind.into(),
            id: c.id,
            title: c.title,
            subtitle: c.subtitle,
            thumbnail_url: c.thumbnail_url,
            track: c.track.map(AndroidTrack::from),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidHomeShelf {
    pub title: String,
    pub tracks: Vec<AndroidTrack>,
    pub cards: Vec<AndroidHomeCard>,
}

impl From<&HomeShelf> for AndroidHomeShelf {
    fn from(s: &HomeShelf) -> Self {
        Self {
            title: s.title.clone(),
            tracks: s.tracks.iter().map(AndroidTrack::from).collect(),
            cards: s.cards.iter().map(AndroidHomeCard::from).collect(),
        }
    }
}

impl From<HomeShelf> for AndroidHomeShelf {
    fn from(s: HomeShelf) -> Self {
        Self {
            title: s.title,
            tracks: s.tracks.into_iter().map(AndroidTrack::from).collect(),
            cards: s.cards.into_iter().map(AndroidHomeCard::from).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidMoodChip {
    pub label: String,
    pub params: String,
}

impl From<MoodChip> for AndroidMoodChip {
    fn from(c: MoodChip) -> Self {
        Self {
            label: c.label,
            params: c.params,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidMoodCategory {
    pub title: String,
    pub browse_id: String,
    pub params: String,
    pub accent_color: u32, // 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

impl From<MoodCategoryTile> for AndroidMoodCategory {
    fn from(t: MoodCategoryTile) -> Self {
        let (r, g, b) = t.accent_rgb;
        Self {
            title: t.title,
            browse_id: t.browse_id,
            params: t.params,
            accent_color: 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidExploreFeed {
    pub shelves: Vec<AndroidHomeShelf>,
    pub moods: Vec<AndroidMoodCategory>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidMusicVideoSyncSegment {
    pub music_start_ms: u64,
    pub video_start_ms: u64,
    pub duration_ms: u64,
}

impl From<MusicVideoSyncSegment> for AndroidMusicVideoSyncSegment {
    fn from(s: MusicVideoSyncSegment) -> Self {
        Self {
            music_start_ms: s.music_start_ms,
            video_start_ms: s.video_start_ms,
            duration_ms: s.duration_ms,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidMusicVideoCandidate {
    pub video_id: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub duration_secs: Option<u64>,
    pub sync_segments: Vec<AndroidMusicVideoSyncSegment>,
}

impl From<MusicVideoCandidate> for AndroidMusicVideoCandidate {
    fn from(c: MusicVideoCandidate) -> Self {
        Self {
            video_id: c.video_id,
            title: c.title,
            thumbnail_url: c.thumbnail_url,
            duration_secs: c.duration_secs,
            sync_segments: c
                .sync_segments
                .into_iter()
                .map(AndroidMusicVideoSyncSegment::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidVideoQualityStream {
    pub quality_label: String,
    pub height: u32,
    pub fps: u32,
    pub video_url: String,
    pub mime_type: String,
    pub bitrate_bps: u64,
    pub is_muxed: bool,
}

impl From<VideoQualityStream> for AndroidVideoQualityStream {
    fn from(q: VideoQualityStream) -> Self {
        Self {
            quality_label: q.quality_label,
            height: q.height,
            fps: q.fps,
            video_url: q.video_url,
            mime_type: q.mime_type,
            bitrate_bps: q.bitrate_bps,
            is_muxed: q.is_muxed,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidResolvedMusicVideo {
    pub video_id: String,
    pub qualities: Vec<AndroidVideoQualityStream>,
    pub audio_url: Option<String>,
    pub audio_badge: String,
    pub headers: Vec<AndroidHeader>,
}

impl From<ResolvedMusicVideo> for AndroidResolvedMusicVideo {
    fn from(v: ResolvedMusicVideo) -> Self {
        Self {
            video_id: v.video_id,
            qualities: v
                .qualities
                .into_iter()
                .map(AndroidVideoQualityStream::from)
                .collect(),
            audio_url: v.audio_url,
            audio_badge: v.audio_badge,
            headers: v
                .headers
                .into_iter()
                .map(|(name, value)| AndroidHeader { name, value })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidArtistPage {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    #[serde(default)]
    #[uniffi(default = None)]
    pub description: Option<String>,
    pub subscribers: Option<String>,
    #[serde(default)]
    #[uniffi(default = None)]
    pub monthly_listeners: Option<String>,
    pub thumbnail_url: Option<String>,
    pub shuffle_playlist_id: Option<String>,
    pub radio_playlist_id: Option<String>,
    pub latest_release: Option<AndroidHomeCard>,
    pub popular_tracks: Vec<AndroidTrack>,
    pub releases: Vec<AndroidHomeCard>,
    pub related_artists: Vec<AndroidHomeCard>,
}

impl From<&ArtistPage> for AndroidArtistPage {
    fn from(a: &ArtistPage) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            subtitle: a.subtitle.clone(),
            description: a.description.clone(),
            subscribers: a.subscribers.clone(),
            monthly_listeners: a.monthly_listeners.clone(),
            thumbnail_url: a.thumbnail_url.clone(),
            shuffle_playlist_id: a.shuffle_playlist_id.clone(),
            radio_playlist_id: a.radio_playlist_id.clone(),
            latest_release: a.latest_release.as_ref().map(AndroidHomeCard::from),
            popular_tracks: a.popular_tracks.iter().map(AndroidTrack::from).collect(),
            releases: a.releases.iter().map(AndroidHomeCard::from).collect(),
            related_artists: a
                .related_artists
                .iter()
                .map(AndroidHomeCard::from)
                .collect(),
        }
    }
}

impl From<ArtistPage> for AndroidArtistPage {
    fn from(a: ArtistPage) -> Self {
        Self {
            id: a.id,
            name: a.name,
            subtitle: a.subtitle,
            description: a.description,
            subscribers: a.subscribers,
            monthly_listeners: a.monthly_listeners,
            thumbnail_url: a.thumbnail_url,
            shuffle_playlist_id: a.shuffle_playlist_id,
            radio_playlist_id: a.radio_playlist_id,
            latest_release: a.latest_release.map(AndroidHomeCard::from),
            popular_tracks: a
                .popular_tracks
                .into_iter()
                .map(AndroidTrack::from)
                .collect(),
            releases: a.releases.into_iter().map(AndroidHomeCard::from).collect(),
            related_artists: a
                .related_artists
                .into_iter()
                .map(AndroidHomeCard::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidResolvedStream {
    pub url: String,
    pub badge: String,
    pub headers: Vec<AndroidHeader>,
}

impl From<ResolvedStream> for AndroidResolvedStream {
    fn from(s: ResolvedStream) -> Self {
        Self {
            url: s.url,
            badge: s.badge,
            headers: s
                .headers
                .into_iter()
                .map(|(name, value)| AndroidHeader { name, value })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidLyricWord {
    pub start_secs: f64,
    pub end_secs: Option<f64>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidLyricLine {
    pub time_secs: f64,
    pub text: String,
    pub words: Vec<AndroidLyricWord>,
    pub is_counter_voice: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub apk_download_url: Option<String>,
    pub release_notes: String,
}

impl From<sonata_core::updater::UpdateCheckResult> for AndroidUpdateInfo {
    fn from(res: sonata_core::updater::UpdateCheckResult) -> Self {
        Self {
            current_version: res.current_version,
            latest_version: res.latest_version,
            update_available: res.update_available,
            release_url: res.release_url,
            apk_download_url: res.apk_download_url,
            release_notes: res.release_notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidServerKind {
    Jellyfin,
    Subsonic,
    WebDav,
    Smb,
}

impl From<ServerKind> for AndroidServerKind {
    fn from(kind: ServerKind) -> Self {
        match kind {
            ServerKind::Jellyfin => Self::Jellyfin,
            ServerKind::Subsonic => Self::Subsonic,
            ServerKind::WebDav => Self::WebDav,
            ServerKind::Smb => Self::Smb,
        }
    }
}

impl From<AndroidServerKind> for ServerKind {
    fn from(kind: AndroidServerKind) -> Self {
        match kind {
            AndroidServerKind::Jellyfin => Self::Jellyfin,
            AndroidServerKind::Subsonic => Self::Subsonic,
            AndroidServerKind::WebDav => Self::WebDav,
            AndroidServerKind::Smb => Self::Smb,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidServerProfile {
    pub id: String,
    pub name: String,
    pub kind: AndroidServerKind,
    pub base_url: String,
    pub enabled: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub api_key: Option<String>,
    pub access_token: Option<String>,
    pub user_id: Option<String>,
    pub bearer_token: Option<String>,
    pub library_ids: Vec<String>,
    #[serde(default)]
    #[uniffi(default = None)]
    pub share: Option<String>,
    #[serde(default)]
    #[uniffi(default = None)]
    pub domain: Option<String>,
    #[serde(default)]
    #[uniffi(default = None)]
    pub local_mount_path: Option<String>,
    #[serde(default)]
    #[uniffi(default = false)]
    pub allow_insecure_lan_http: bool,
}

impl std::fmt::Debug for AndroidServerProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |opt: &Option<String>| -> &'static str {
            if opt.as_deref().is_some_and(|v| !v.is_empty()) {
                "[redacted]"
            } else {
                "<none>"
            }
        };
        f.debug_struct("AndroidServerProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field(
                "base_url",
                &sonata_core::redaction::stream_url(&self.base_url),
            )
            .field("enabled", &self.enabled)
            .field("username", &self.username)
            .field("password", &redact(&self.password))
            .field("api_key", &redact(&self.api_key))
            .field("access_token", &redact(&self.access_token))
            .field("user_id", &self.user_id)
            .field("bearer_token", &redact(&self.bearer_token))
            .field("library_ids", &self.library_ids)
            .field("share", &self.share)
            .field("domain", &self.domain)
            .field("local_mount_path", &self.local_mount_path)
            .field("allow_insecure_lan_http", &self.allow_insecure_lan_http)
            .finish()
    }
}

impl From<ServerProfile> for AndroidServerProfile {
    fn from(p: ServerProfile) -> Self {
        Self {
            id: p.id,
            name: p.name,
            kind: p.kind.into(),
            base_url: p.base_url,
            enabled: p.enabled,
            username: p.username,
            password: p.password,
            api_key: p.api_key,
            access_token: p.access_token,
            user_id: p.user_id,
            bearer_token: p.bearer_token,
            library_ids: p.library_ids,
            share: p.share,
            domain: p.domain,
            local_mount_path: p
                .local_mount_path
                .map(|path| path.to_string_lossy().into_owned()),
            allow_insecure_lan_http: p.allow_insecure_lan_http,
        }
    }
}

impl From<AndroidServerProfile> for ServerProfile {
    fn from(p: AndroidServerProfile) -> Self {
        let clean_opt = |opt: Option<String>| -> Option<String> {
            opt.and_then(|s| {
                let trimmed = s.trim().to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            })
        };
        Self {
            id: p.id,
            name: p.name,
            kind: p.kind.into(),
            base_url: p.base_url,
            enabled: p.enabled,
            username: p.username,
            password: p.password,
            api_key: p.api_key,
            access_token: p.access_token,
            user_id: p.user_id,
            bearer_token: p.bearer_token,
            library_ids: p.library_ids,
            share: clean_opt(p.share),
            domain: clean_opt(p.domain),
            local_mount_path: clean_opt(p.local_mount_path).map(PathBuf::from),
            allow_insecure_lan_http: p.allow_insecure_lan_http,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidServerHealthReport {
    pub alive: bool,
    pub latency_ms: Option<u64>,
    pub message: String,
}

impl From<ServerHealthReport> for AndroidServerHealthReport {
    fn from(report: ServerHealthReport) -> Self {
        Self {
            alive: report.alive,
            latency_ms: report.latency_ms,
            message: report.message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidServerSpeedTestReport {
    pub alive: bool,
    pub latency_ms: u64,
    pub download_mbps: Option<f64>,
    pub bytes_tested: u64,
    pub sample_note: String,
}

impl From<ServerSpeedTestReport> for AndroidServerSpeedTestReport {
    fn from(report: ServerSpeedTestReport) -> Self {
        Self {
            alive: report.alive,
            latency_ms: report.latency_ms,
            download_mbps: report.download_mbps,
            bytes_tested: report.bytes_tested,
            sample_note: report.sample_note,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidPersonalPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<AndroidTrack>,
    pub created_at_epoch_secs: u64,
    pub updated_at_epoch_secs: u64,
}

impl From<PersonalPlaylist> for AndroidPersonalPlaylist {
    fn from(playlist: PersonalPlaylist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            tracks: playlist
                .tracks
                .into_iter()
                .map(AndroidTrack::from)
                .collect(),
            created_at_epoch_secs: playlist.created_at_epoch_secs,
            updated_at_epoch_secs: playlist.updated_at_epoch_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidServerLibrary {
    pub id: String,
    pub name: String,
    pub kind: String,
}

impl From<ServerLibrary> for AndroidServerLibrary {
    fn from(lib: ServerLibrary) -> Self {
        Self {
            id: lib.id,
            name: lib.name,
            kind: lib.kind,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidServerAlbum {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub year: Option<String>,
    pub cover_url: Option<String>,
    pub tracks: Vec<AndroidTrack>,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct AndroidServerSearchResult {
    pub tracks: Vec<AndroidTrack>,
    pub albums: Vec<AndroidHomeCard>,
    pub artists: Vec<AndroidHomeCard>,
    pub playlists: Vec<AndroidHomeCard>,
}

impl From<ServerSearchResult> for AndroidServerSearchResult {
    fn from(res: ServerSearchResult) -> Self {
        Self {
            tracks: res.tracks.into_iter().map(AndroidTrack::from).collect(),
            albums: res.albums.into_iter().map(AndroidHomeCard::from).collect(),
            artists: res.artists.into_iter().map(AndroidHomeCard::from).collect(),
            playlists: res
                .playlists
                .into_iter()
                .map(AndroidHomeCard::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidServerStream {
    pub url: String,
    pub headers: Vec<AndroidHeader>,
    pub mime_type: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<String>,
    pub audio_badge: Option<String>,
}

impl From<ServerStream> for AndroidServerStream {
    fn from(stream: ServerStream) -> Self {
        Self {
            url: stream.url,
            headers: stream
                .headers
                .into_iter()
                .map(|(name, value)| AndroidHeader { name, value })
                .collect(),
            mime_type: stream.mime_type,
            bitrate_kbps: stream.bitrate_kbps,
            codec: stream.codec,
            audio_badge: stream.audio_badge,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidWebDavListing {
    pub folders: Vec<AndroidServerLibrary>,
    pub tracks: Vec<AndroidTrack>,
}

#[derive(Clone, uniffi::Record)]
pub struct AndroidJellyfinAuth {
    pub access_token: String,
    pub user_id: String,
    pub user_name: Option<String>,
    pub server_id: Option<String>,
}

impl std::fmt::Debug for AndroidJellyfinAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidJellyfinAuth")
            .field("access_token", &"[redacted]")
            .field("user_id", &self.user_id)
            .field("user_name", &self.user_name)
            .field("server_id", &self.server_id)
            .finish()
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidLastFmConfig {
    pub enabled: bool,
    pub api_key: String,
    pub api_secret: String,
    pub session_key: String,
    pub username: String,
}

impl std::fmt::Debug for AndroidLastFmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |v: &str| -> &'static str {
            if v.trim().is_empty() {
                "<none>"
            } else {
                "[redacted]"
            }
        };
        f.debug_struct("AndroidLastFmConfig")
            .field("enabled", &self.enabled)
            .field("api_key", &redact(&self.api_key))
            .field("api_secret", &redact(&self.api_secret))
            .field("session_key", &redact(&self.session_key))
            .field("username", &self.username)
            .finish()
    }
}

impl From<LastFmConfig> for AndroidLastFmConfig {
    fn from(c: LastFmConfig) -> Self {
        Self {
            enabled: c.enabled,
            api_key: c.api_key,
            api_secret: c.api_secret,
            session_key: c.session_key,
            username: c.username,
        }
    }
}

impl From<AndroidLastFmConfig> for LastFmConfig {
    fn from(c: AndroidLastFmConfig) -> Self {
        Self {
            enabled: c.enabled,
            api_key: c.api_key.trim().to_string(),
            api_secret: c.api_secret.trim().to_string(),
            session_key: c.session_key.trim().to_string(),
            username: c.username.trim().to_string(),
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidListenBrainzConfig {
    pub enabled: bool,
    pub token: String,
    pub username: String,
}

impl std::fmt::Debug for AndroidListenBrainzConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token_status = if self.token.trim().is_empty() {
            "<none>"
        } else {
            "[redacted]"
        };
        f.debug_struct("AndroidListenBrainzConfig")
            .field("enabled", &self.enabled)
            .field("token", &token_status)
            .field("username", &self.username)
            .finish()
    }
}

impl From<ListenBrainzConfig> for AndroidListenBrainzConfig {
    fn from(c: ListenBrainzConfig) -> Self {
        Self {
            enabled: c.enabled,
            token: c.token,
            username: c.username,
        }
    }
}

impl From<AndroidListenBrainzConfig> for ListenBrainzConfig {
    fn from(c: AndroidListenBrainzConfig) -> Self {
        Self {
            enabled: c.enabled,
            token: c.token.trim().to_string(),
            username: c.username.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidScrobbleConfig {
    pub lastfm: AndroidLastFmConfig,
    pub listenbrainz: AndroidListenBrainzConfig,
}

impl From<ScrobbleConfig> for AndroidScrobbleConfig {
    fn from(c: ScrobbleConfig) -> Self {
        Self {
            lastfm: AndroidLastFmConfig::from(c.lastfm),
            listenbrainz: AndroidListenBrainzConfig::from(c.listenbrainz),
        }
    }
}

impl From<AndroidScrobbleConfig> for ScrobbleConfig {
    fn from(c: AndroidScrobbleConfig) -> Self {
        Self {
            lastfm: LastFmConfig::from(c.lastfm),
            listenbrainz: ListenBrainzConfig::from(c.listenbrainz),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidStatsWindow {
    Week,
    Month,
    Year,
    AllTime,
}

impl From<AndroidStatsWindow> for StatsWindow {
    fn from(w: AndroidStatsWindow) -> Self {
        match w {
            AndroidStatsWindow::Week => Self::Week,
            AndroidStatsWindow::Month => Self::Month,
            AndroidStatsWindow::Year => Self::Year,
            AndroidStatsWindow::AllTime => Self::AllTime,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidArtistStat {
    pub artist: String,
    pub plays: u64,
    pub minutes: u64,
}

impl From<ArtistStat> for AndroidArtistStat {
    fn from(s: ArtistStat) -> Self {
        Self {
            artist: s.artist,
            plays: s.plays,
            minutes: s.minutes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidTrackStat {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source_tag: String,
    pub plays: u64,
    pub minutes: u64,
}

impl From<TrackStat> for AndroidTrackStat {
    fn from(s: TrackStat) -> Self {
        Self {
            track_id: s.track_id,
            title: s.title,
            artist: s.artist,
            album: s.album,
            source_tag: s.source_tag,
            plays: s.plays,
            minutes: s.minutes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidAlbumStat {
    pub album: String,
    pub artist: String,
    pub plays: u64,
    pub minutes: u64,
}

impl From<AlbumStat> for AndroidAlbumStat {
    fn from(s: AlbumStat) -> Self {
        Self {
            album: s.album,
            artist: s.artist,
            plays: s.plays,
            minutes: s.minutes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidSourceStat {
    pub source_tag: String,
    pub plays: u64,
    pub minutes: u64,
}

impl From<SourceStat> for AndroidSourceStat {
    fn from(s: SourceStat) -> Self {
        Self {
            source_tag: s.source_tag,
            plays: s.plays,
            minutes: s.minutes,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidStatsSummary {
    pub total_plays: u64,
    pub total_minutes: u64,
    pub unique_tracks: u64,
    pub unique_artists: u64,
    pub streak_days: u64,
    pub top_artists: Vec<AndroidArtistStat>,
    pub top_tracks: Vec<AndroidTrackStat>,
    pub top_albums: Vec<AndroidAlbumStat>,
    pub source_breakdown: Vec<AndroidSourceStat>,
}

impl From<StatsSummary> for AndroidStatsSummary {
    fn from(s: StatsSummary) -> Self {
        Self {
            total_plays: s.total_plays,
            total_minutes: s.total_minutes,
            unique_tracks: s.unique_tracks,
            unique_artists: s.unique_artists,
            streak_days: s.streak_days,
            top_artists: s
                .top_artists
                .into_iter()
                .map(AndroidArtistStat::from)
                .collect(),
            top_tracks: s
                .top_tracks
                .into_iter()
                .map(AndroidTrackStat::from)
                .collect(),
            top_albums: s
                .top_albums
                .into_iter()
                .map(AndroidAlbumStat::from)
                .collect(),
            source_breakdown: s
                .source_breakdown
                .into_iter()
                .map(AndroidSourceStat::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidSolvedCipher {
    pub sig: Option<String>,
    pub n: Option<String>,
}

#[uniffi::export(callback_interface)]
pub trait AndroidJsCipherSolver: Send + Sync {
    fn ensure_loaded(&self, player_hash: String, hooked_script: String) -> bool;
    fn solve(&self, player_hash: String, sig: String, n: String) -> Option<AndroidSolvedCipher>;
}

struct AndroidCipherBridgeAdapter {
    solver: Box<dyn AndroidJsCipherSolver>,
    loaded_hash: std::sync::Mutex<Option<String>>,
}

impl JsCipherBridge for AndroidCipherBridgeAdapter {
    fn ensure_loaded(&self, player_hash: &str, hooked_script: &str) -> bool {
        {
            let guard = self
                .loaded_hash
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if guard.as_deref() == Some(player_hash) {
                return true;
            }
        }
        let ok = self
            .solver
            .ensure_loaded(player_hash.to_string(), hooked_script.to_string());
        if ok {
            let mut guard = self
                .loaded_hash
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = Some(player_hash.to_string());
        }
        ok
    }

    fn solve(&self, player_hash: &str, sig: &str, n: &str) -> Option<SolvedCipherPair> {
        let solved = self
            .solver
            .solve(player_hash.to_string(), sig.to_string(), n.to_string())?;
        Some(SolvedCipherPair {
            sig: solved.sig,
            n: solved.n,
        })
    }
}

struct AuthSession {
    cookies: ExtractedCookies,
    ytm: YtMusicClient,
    resolver: StreamResolver,
}

#[derive(uniffi::Object)]
pub struct SonataAndroidEngine {
    http: reqwest::Client,
    paths: CorePaths,
    cache_db: Option<CacheDb>,
    cipher_bridge: RwLock<Option<Arc<dyn JsCipherBridge>>>,
    language: RwLock<String>,
    session: RwLock<AuthSession>,
    server_lock: Arc<tokio::sync::Mutex<()>>,
    server_id_seq: std::sync::atomic::AtomicU64,
    personal_playlists: PersonalPlaylistStore,
    scrobble_lock: Arc<tokio::sync::Mutex<()>>,
    scrobble_client: ScrobbleClient,
    stats_db: Arc<std::sync::Mutex<Option<StatsDb>>>,
}

impl SonataAndroidEngine {
    fn server_client(&self) -> ServerClient {
        ServerClient::new(self.http.clone()).with_cache_dir(self.paths.cache_dir.clone())
    }

    async fn fetch_server_lyrics_internal(&self, track_id: &str) -> Vec<AndroidLyricLine> {
        let Some(item_ref) = parse_server_item_id(track_id) else {
            return Vec::new();
        };
        let Ok(profile) = self.find_server_profile(&item_ref.server_id).await else {
            return Vec::new();
        };
        let raw_opt = match item_ref.kind {
            ServerKind::WebDav => webdav::fetch_lyrics(&self.http, &profile, &item_ref.item_id)
                .await
                .ok()
                .flatten(),
            ServerKind::Jellyfin => jellyfin::fetch_lyrics(&self.http, &profile, &item_ref.item_id)
                .await
                .ok()
                .flatten(),
            ServerKind::Subsonic | ServerKind::Smb => None,
        };
        let Some(raw) = raw_opt else {
            return Vec::new();
        };
        match sonata_core::lyrics::parse_lyrics_auto(&raw) {
            LyricsState::Synced(lines) => lines
                .into_iter()
                .map(|l| AndroidLyricLine {
                    time_secs: l.time_secs,
                    text: l.text,
                    words: l
                        .words
                        .into_iter()
                        .map(|w| AndroidLyricWord {
                            start_secs: w.start_secs,
                            end_secs: w.end_secs,
                            text: w.text,
                        })
                        .collect(),
                    is_counter_voice: l.voice == sonata_core::lyrics::LyricVoice::Counter,
                })
                .collect(),
            LyricsState::Plain(lines) => lines
                .into_iter()
                .map(|text| AndroidLyricLine {
                    time_secs: -1.0,
                    text,
                    words: Vec::new(),
                    is_counter_voice: false,
                })
                .collect(),
            LyricsState::Idle | LyricsState::Loading | LyricsState::NotFound => Vec::new(),
        }
    }

    async fn load_servers_internal(&self) -> Result<Vec<ServerProfile>, SonataAndroidError> {
        let paths = self.paths.clone();
        let profiles = tokio::task::spawn_blocking(move || load_server_profiles(&paths))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Server config read task failed: {err}"),
            })??;
        Ok(profiles)
    }

    async fn find_server_profile(
        &self,
        server_id: &str,
    ) -> Result<ServerProfile, SonataAndroidError> {
        let profiles = self.load_servers_internal().await?;
        profiles
            .into_iter()
            .find(|p| p.id == server_id)
            .ok_or_else(|| SonataAndroidError::NotFound {
                reason: format!("Music server `{server_id}` was not found"),
            })
    }

    async fn find_server_for_item(
        &self,
        encoded_id: &str,
    ) -> Result<
        (
            ServerProfile,
            sonata_core::providers::servers::ServerItemRef,
        ),
        SonataAndroidError,
    > {
        let item_ref =
            parse_server_item_id(encoded_id).ok_or_else(|| SonataAndroidError::NotFound {
                reason: format!("Invalid self-hosted server item ID `{encoded_id}`"),
            })?;
        let profile = self.find_server_profile(&item_ref.server_id).await?;
        if profile.kind != item_ref.kind {
            return Err(SonataAndroidError::NotFound {
                reason: format!(
                    "Server `{}` protocol mismatch for item `{encoded_id}`",
                    profile.id
                ),
            });
        }
        Ok((profile, item_ref))
    }

    fn generate_server_id(&self, kind: ServerKind) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let seq = self
            .server_id_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let prefix = match kind {
            ServerKind::Jellyfin => "jf",
            ServerKind::Subsonic => "sub",
            ServerKind::WebDav => "dav",
            ServerKind::Smb => "smb",
        };
        format!("{prefix}-{nanos}-{seq}")
    }

    async fn ensure_jellyfin_auth_if_needed(
        &self,
        mut profile: ServerProfile,
    ) -> Result<ServerProfile, SonataAndroidError> {
        if profile.kind != ServerKind::Jellyfin {
            return Ok(profile);
        }
        let has_token = profile
            .access_token
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty())
            || profile
                .api_key
                .as_deref()
                .is_some_and(|k| !k.trim().is_empty());
        if has_token {
            return Ok(profile);
        }
        let username = profile
            .username
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let password = profile.password.as_deref().filter(|s| !s.is_empty());
        if let (Some(user), Some(pass)) = (username, password) {
            let device_id = if profile.id.trim().is_empty() {
                "sonata-android".to_string()
            } else {
                profile.id.clone()
            };
            let auth = jellyfin::authenticate_by_name_with_policy(
                &self.http,
                &profile.base_url,
                user,
                pass,
                &device_id,
                profile.allow_insecure_lan_http,
            )
            .await?;
            profile.access_token = Some(auth.access_token);
            if profile
                .user_id
                .as_deref()
                .is_none_or(|id| id.trim().is_empty())
                && !auth.user_id.trim().is_empty()
            {
                profile.user_id = Some(auth.user_id);
            }
        }
        Ok(profile)
    }

    fn current_ytm(&self) -> YtMusicClient {
        self.session
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ytm
            .clone()
    }

    fn current_resolver(&self) -> StreamResolver {
        self.session
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resolver
            .clone()
    }

    fn current_language(&self) -> String {
        self.language
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn build_session(
        http: &reqwest::Client,
        paths: &CorePaths,
        cache_db: Option<CacheDb>,
        cookies: ExtractedCookies,
        cipher_bridge: Option<Arc<dyn JsCipherBridge>>,
        language: &str,
    ) -> AuthSession {
        let ytm = YtMusicClient::new(http.clone(), cookies.clone()).with_language(language);
        let resolver = StreamResolver::with_profile(
            http.clone(),
            ytm.clone(),
            cookies.clone(),
            cache_db,
            paths.clone(),
            android_resolver_profile(),
        );
        if let Some(bridge) = cipher_bridge {
            resolver.set_js_cipher_bridge(bridge);
        }
        AuthSession {
            cookies,
            ytm,
            resolver,
        }
    }

    async fn load_scrobble_config_internal(&self) -> Result<ScrobbleConfig, SonataAndroidError> {
        let paths = self.paths.clone();
        let cfg = tokio::task::spawn_blocking(move || ScrobbleConfig::load_from_paths(&paths))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Scrobble config read task failed: {err}"),
            })??;
        Ok(cfg)
    }

    fn open_or_get_stats_db_blocking(
        stats_db: &Arc<std::sync::Mutex<Option<StatsDb>>>,
        paths: &CorePaths,
    ) -> Result<StatsDb, SonataAndroidError> {
        let mut guard = stats_db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = guard.as_ref() {
            return Ok(existing.clone());
        }
        let opened = StatsDb::open_with_paths(paths)?;
        *guard = Some(opened.clone());
        Ok(opened)
    }

    fn infer_source_tag(track_id: &str, explicit_tag: &str) -> String {
        let trimmed = explicit_tag.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        if track_id.starts_with("local:") {
            "LOC".to_string()
        } else if track_id.starts_with("jellyfin:") {
            "JFN".to_string()
        } else if track_id.starts_with("subsonic:") {
            "SUB".to_string()
        } else if track_id.starts_with("smb:") {
            "SMB".to_string()
        } else if track_id.starts_with("webdav:") {
            "DAV".to_string()
        } else if track_id.starts_with("jiosaavn:") {
            "JIO".to_string()
        } else {
            "YTM".to_string()
        }
    }

    fn current_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl SonataAndroidEngine {
    /// Creates a new Sonata Android engine rooted at the app's private `filesDir` and `cacheDir`.
    #[uniffi::constructor]
    pub fn new(config_dir: String, cache_dir: String) -> Result<Self, SonataAndroidError> {
        let paths = android_core_paths(&config_dir, &cache_dir);
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(6))
            .timeout(std::time::Duration::from_secs(20))
            .tcp_nodelay(true)
            .pool_max_idle_per_host(8)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| SonataAndroidError::Internal {
                reason: format!("Failed to initialize HTTP client: {e}"),
            })?;

        // Home shelves are always fetched live; purge any legacy home cache file from prior builds.
        remove_cached_json_from(&paths.cache_dir, CACHE_HOME_FILE);

        let cache_db = CacheDb::open_with_paths(&paths, true);
        let cookies = ExtractedCookies::load_saved_ytm_header(&paths).unwrap_or_default();
        let session = Self::build_session(&http, &paths, cache_db.clone(), cookies, None, "en");
        let personal_playlists = PersonalPlaylistStore::new(&paths);
        let scrobble_client = ScrobbleClient::new();

        Ok(Self {
            http,
            paths,
            cache_db,
            cipher_bridge: RwLock::new(None),
            language: RwLock::new("en".to_string()),
            session: RwLock::new(session),
            server_lock: Arc::new(tokio::sync::Mutex::new(())),
            server_id_seq: std::sync::atomic::AtomicU64::new(1),
            personal_playlists,
            scrobble_lock: Arc::new(tokio::sync::Mutex::new(())),
            scrobble_client,
            stats_db: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Registers an Android WebView JavaScript evaluator for deciphering `TVHTML5` `itag=774` / `141` streams.
    pub fn set_js_cipher_solver(&self, solver: Box<dyn AndroidJsCipherSolver>) {
        let bridge: Arc<dyn JsCipherBridge> = Arc::new(AndroidCipherBridgeAdapter {
            solver,
            loaded_hash: std::sync::Mutex::new(None),
        });
        {
            let mut guard = self
                .cipher_bridge
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = Some(Arc::clone(&bridge));
        }
        self.current_resolver().set_js_cipher_bridge(bridge);
    }

    /// Pre-fetches, hooks, caches, and loads YouTube's `base.js` into the registered Android cipher solver.
    pub async fn warm_cipher_solver(&self) -> bool {
        let resolver = self.current_resolver();
        resolver.ensure_prepared_player_js().await.is_some()
    }

    /// Returns whether valid YouTube Music session cookies are loaded.
    pub fn is_signed_in(&self) -> bool {
        self.session
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cookies
            .has_ytm_auth()
    }

    /// Reads the cached YouTube Music account profile from disk off the calling thread.
    pub async fn get_cached_account_profile(&self) -> Option<AndroidAccountProfile> {
        if !self.is_signed_in() {
            return None;
        }
        let cache_dir = self.paths.cache_dir.clone();
        tokio::task::spawn_blocking(move || load_cached_json_from(&cache_dir, CACHE_PROFILE_FILE))
            .await
            .ok()
            .flatten()
    }

    /// Reads the cached library playlists from disk off the calling thread.
    pub async fn get_cached_library_playlists(&self) -> Vec<AndroidPlaylist> {
        if !self.is_signed_in() {
            return Vec::new();
        }
        let cache_dir = self.paths.cache_dir.clone();
        tokio::task::spawn_blocking(move || {
            load_cached_json_from(&cache_dir, CACHE_PLAYLISTS_FILE).unwrap_or_default()
        })
        .await
        .unwrap_or_default()
    }

    /// Installs YouTube Music WebView cookies, persists them privately, and returns the account profile.
    pub async fn set_ytm_cookie_header(
        &self,
        cookie_header: String,
    ) -> Result<Option<AndroidAccountProfile>, SonataAndroidError> {
        let cookies = ExtractedCookies::from_ytm_cookie_header(&cookie_header);
        if !cookies.has_ytm_auth() {
            return Err(SonataAndroidError::AuthRequired {
                reason: "Signed-in YouTube Music cookies (SAPISID) were not found".to_string(),
            });
        }
        let paths_for_save = self.paths.clone();
        let cookies_for_save = cookies.clone();
        let cache_db_for_clear = self.cache_db.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = cookies_for_save.save_ytm_header(&paths_for_save);
            if let Some(ref db) = cache_db_for_clear {
                db.clear_stream_cache();
            }
        })
        .await;

        let bridge = self
            .cipher_bridge
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let lang = self.current_language();
        let next_session = Self::build_session(
            &self.http,
            &self.paths,
            self.cache_db.clone(),
            cookies,
            bridge,
            &lang,
        );
        let ytm = next_session.ytm.clone();
        {
            let mut guard = self
                .session
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = next_session;
        }

        let profile = ytm
            .fetch_account_profile()
            .await
            .ok()
            .map(AndroidAccountProfile::from);
        if let Some(ref prof) = profile {
            let cache_dir = self.paths.cache_dir.clone();
            let prof_clone = prof.clone();
            let _ = tokio::task::spawn_blocking(move || {
                save_cached_json_in(&cache_dir, CACHE_PROFILE_FILE, &prof_clone);
            })
            .await;
        }
        Ok(profile)
    }

    /// Fetches the signed-in YouTube Music account profile, if authenticated.
    pub async fn get_account_profile(&self) -> Option<AndroidAccountProfile> {
        if !self.is_signed_in() {
            return None;
        }
        let ytm = self.current_ytm();
        let profile = ytm
            .fetch_account_profile()
            .await
            .ok()
            .map(AndroidAccountProfile::from);
        if let Some(ref prof) = profile {
            let cache_dir = self.paths.cache_dir.clone();
            let prof_clone = prof.clone();
            let _ = tokio::task::spawn_blocking(move || {
                save_cached_json_in(&cache_dir, CACHE_PROFILE_FILE, &prof_clone);
            })
            .await;
        }
        profile
    }

    /// Clears saved YouTube Music session cookies and resets to anonymous discovery.
    pub async fn sign_out(&self) {
        let paths = self.paths.clone();
        let cache_db = self.cache_db.clone();
        let _ = tokio::task::spawn_blocking(move || {
            ExtractedCookies::clear_saved_ytm_header(&paths);
            remove_cached_json_from(&paths.cache_dir, CACHE_PROFILE_FILE);
            remove_cached_json_from(&paths.cache_dir, CACHE_HOME_FILE);
            remove_cached_json_from(&paths.cache_dir, CACHE_PLAYLISTS_FILE);
            if let Some(ref db) = cache_db {
                db.clear_stream_cache();
            }
        })
        .await;

        let bridge = self
            .cipher_bridge
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let lang = self.current_language();
        let next_session = Self::build_session(
            &self.http,
            &self.paths,
            self.cache_db.clone(),
            ExtractedCookies::default(),
            bridge,
            &lang,
        );
        {
            let mut guard = self
                .session
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = next_session;
        }
    }

    /// Fetches the signed-in user's YouTube Music library playlists.
    pub async fn fetch_library_playlists(
        &self,
    ) -> Result<Vec<AndroidPlaylist>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let playlists = ytm.fetch_library_playlists().await?;
        let out: Vec<AndroidPlaylist> = playlists.into_iter().map(AndroidPlaylist::from).collect();
        let cache_dir = self.paths.cache_dir.clone();
        let to_save = out.clone();
        let _ = tokio::task::spawn_blocking(move || {
            save_cached_json_in(&cache_dir, CACHE_PLAYLISTS_FILE, &to_save);
        })
        .await;
        Ok(out)
    }

    /// Fetches fresh YouTube Music home/discovery shelves live without persisting to disk.
    pub async fn fetch_home_feed(&self) -> Result<Vec<AndroidHomeShelf>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let shelves = ytm.fetch_home_feed().await?;
        Ok(shelves.into_iter().map(AndroidHomeShelf::from).collect())
    }

    /// Updates the active YouTube Music `hl` language (`"tr"`, `"ru"`, or `"en"`) and clears
    /// cached playlists and artist pages when changed.
    pub async fn set_language(&self, language_code: String) {
        {
            let mut guard = self
                .language
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = language_code.clone();
        }
        let ytm = self.current_ytm();
        ytm.set_language(&language_code).await;
    }

    /// Fetches `FEmusic_explore` shelves alongside `FEmusic_moods_and_genres` category tiles.
    pub async fn fetch_explore_feed(&self) -> Result<AndroidExploreFeed, SonataAndroidError> {
        let ytm = self.current_ytm();
        let (shelves, moods) = ytm.fetch_explore().await?;
        Ok(AndroidExploreFeed {
            shelves: shelves.into_iter().map(AndroidHomeShelf::from).collect(),
            moods: moods.into_iter().map(AndroidMoodCategory::from).collect(),
        })
    }

    /// Fetches curated shelves for a mood or genre category browse endpoint.
    pub async fn fetch_mood_category(
        &self,
        browse_id: String,
        params: String,
    ) -> Result<Vec<AndroidHomeShelf>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let shelves = ytm.fetch_mood_category(&browse_id, &params).await?;
        Ok(shelves.into_iter().map(AndroidHomeShelf::from).collect())
    }

    /// Fetches top-level YouTube Music home mood filter chips.
    pub async fn fetch_home_mood_chips(&self) -> Result<Vec<AndroidMoodChip>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let (chips, _) = ytm.fetch_home_with_chips(None).await?;
        Ok(chips.into_iter().map(AndroidMoodChip::from).collect())
    }

    /// Fetches the home feed filtered by an optional mood chip `params` token, or the full
    /// home feed when `params` is `None` or empty.
    pub async fn fetch_home_mood_feed(
        &self,
        params: Option<String>,
    ) -> Result<Vec<AndroidHomeShelf>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let shelves = match params.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(p) => {
                let (_, shelves) = ytm.fetch_home_with_chips(Some(p)).await?;
                shelves
            }
            None => ytm.fetch_home_feed().await?,
        };
        Ok(shelves.into_iter().map(AndroidHomeShelf::from).collect())
    }

    /// Searches YouTube Music songs for the given query.
    pub async fn search_songs(
        &self,
        query: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = ytm.search_songs(&query).await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Fetches all tracks for a YouTube Music playlist, album, or mix browse ID.
    pub async fn fetch_collection_tracks(
        &self,
        browse_id: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let mut tracks = Vec::new();
        ytm.fetch_playlist_tracks_progressive(&browse_id, |batch, _| {
            tracks.extend(batch);
        })
        .await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Invalidates any cached playlist entry for `browse_id` and fetches live tracks from YouTube Music.
    pub async fn fetch_collection_tracks_fresh(
        &self,
        browse_id: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.invalidate_cache(Some(&browse_id)).await;
        let mut tracks = Vec::new();
        ytm.fetch_playlist_tracks_progressive(&browse_id, |batch, _| {
            tracks.extend(batch);
        })
        .await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Fetches a full YouTube Music or WebDAV artist page (hero, metadata, albums, tracks, similar artists).
    pub async fn fetch_artist_page(
        &self,
        browse_id: String,
    ) -> Result<AndroidArtistPage, SonataAndroidError> {
        if let Some(item_ref) = parse_server_item_id(&browse_id) {
            if item_ref.kind == ServerKind::WebDav {
                let profile = self.find_server_profile(&item_ref.server_id).await?;
                if let Some(art) = webdav::fetch_artist_profile(
                    &self.http,
                    &profile,
                    &item_ref.item_id,
                    Some(&self.paths.cache_dir),
                )
                .await?
                {
                    let subtitle = art.subtitle.unwrap_or_else(|| {
                        let mut parts = Vec::new();
                        if !art.albums.is_empty() {
                            parts.push(format!("{} albums", art.albums.len()));
                        }
                        if !art.tracks.is_empty() {
                            parts.push(format!("{} tracks", art.tracks.len()));
                        }
                        parts.join(" · ")
                    });
                    return Ok(AndroidArtistPage {
                        id: browse_id,
                        name: art.name,
                        subtitle,
                        description: art.bio,
                        subscribers: None,
                        monthly_listeners: None,
                        thumbnail_url: art.avatar_url,
                        shuffle_playlist_id: None,
                        radio_playlist_id: None,
                        latest_release: art.albums.first().cloned().map(AndroidHomeCard::from),
                        popular_tracks: art.tracks.into_iter().map(AndroidTrack::from).collect(),
                        releases: art.albums.into_iter().map(AndroidHomeCard::from).collect(),
                        related_artists: art
                            .related_artists
                            .into_iter()
                            .map(AndroidHomeCard::from)
                            .collect(),
                    });
                }
                return Err(SonataAndroidError::NotFound {
                    reason: format!("WebDAV artist `{browse_id}` was not found"),
                });
            }
        }
        let ytm = self.current_ytm();
        let page = ytm.fetch_artist_page(&browse_id).await?;
        Ok(AndroidArtistPage::from(page))
    }

    /// Resolves an artist name (e.g. `"The Weeknd"` or `"The Weeknd, Daft Punk"`) to its `UC...` browse ID.
    pub async fn resolve_artist_browse_id(
        &self,
        name: String,
    ) -> Result<Option<String>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let id = ytm.resolve_artist_browse_id(&name).await?;
        Ok(id)
    }

    /// Resolves an artist name to its `UC...` browse ID and fetches its full `AndroidArtistPage`.
    pub async fn fetch_artist_by_name(
        &self,
        name: String,
    ) -> Result<AndroidArtistPage, SonataAndroidError> {
        let ytm = self.current_ytm();
        let page = ytm.fetch_artist_by_name(&name).await?;
        Ok(AndroidArtistPage::from(page))
    }

    /// Resolves an album title + artist name to its `MPRE...` / `OLAK...` browse ID.
    pub async fn resolve_album_browse_id(
        &self,
        album_name: String,
        artist_name: String,
    ) -> Result<Option<String>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let id = ytm
            .resolve_album_browse_id(&album_name, &artist_name)
            .await?;
        Ok(id)
    }

    /// Fetches personalized YouTube Music radio & mix shelves (excluding manual user playlists).
    pub async fn fetch_radio_mixes(&self) -> Result<Vec<AndroidHomeShelf>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let shelves = ytm.fetch_radio_mixes().await?;
        Ok(shelves.into_iter().map(AndroidHomeShelf::from).collect())
    }

    /// Fetches the signed-in user's reverse-chronological YTM listening history (`FEmusic_history`).
    pub async fn fetch_listening_history(&self) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = ytm.fetch_listening_history().await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Removes `video_id` from the signed-in user's YouTube Music listening history (`FEmusic_history`).
    pub async fn remove_from_listening_history(
        &self,
        video_id: String,
    ) -> Result<bool, SonataAndroidError> {
        let trimmed = video_id.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("local:")
            || trimmed.starts_with("jiosaavn:")
            || parse_server_item_id(trimmed).is_some()
        {
            return Ok(false);
        }
        let ytm = self.current_ytm();
        Ok(ytm.remove_from_listening_history(trimmed).await?)
    }

    /// Finds an explicit counterpart track when `track` is unbadged or clean/edited.
    pub async fn resolve_explicit_counterpart(&self, track: AndroidTrack) -> Option<AndroidTrack> {
        if parse_server_item_id(&track.id).is_some() {
            return None;
        }
        let ytm = self.current_ytm();
        let core_track: Track = track.into();
        ytm.resolve_explicit_counterpart(&core_track)
            .await
            .map(AndroidTrack::from)
    }

    /// Fetches personalized radio / up-next tracks seeded by `video_id` (`RDAMVM<video_id>`).
    pub async fn fetch_radio_queue(
        &self,
        video_id: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = ytm.fetch_radio_queue(&video_id).await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Fetches personalized endless continuation tracks combining playlist automix and seed
    /// radios, excluding played/seed IDs and diversifying artists.
    pub async fn fetch_endless_radio_queue(
        &self,
        seed_video_ids: Vec<String>,
        playlist_id: Option<String>,
        exclude_video_ids: Vec<String>,
        preferred_artists: Vec<String>,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = ytm
            .fetch_endless_radio_queue(
                &seed_video_ids,
                playlist_id.as_deref(),
                &exclude_video_ids,
                &preferred_artists,
            )
            .await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Persists like/unlike status for a YouTube Music track.
    pub async fn set_like_status(
        &self,
        video_id: String,
        liked: bool,
    ) -> Result<(), SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.set_like_status(&video_id, liked).await?;
        Ok(())
    }

    /// Reports single-track playback and watchtime telemetry for `video_id`
    /// so YouTube Music updates Listening History (`FEmusic_history`) without attaching playlist IDs.
    pub async fn report_playback(
        &self,
        video_id: String,
        playlist_id: Option<String>,
        duration_secs: Option<u64>,
    ) -> Result<(), SonataAndroidError> {
        let _ = playlist_id;
        let ytm = self.current_ytm();
        ytm.report_playback(&video_id, None, duration_secs).await?;
        Ok(())
    }

    /// Appends one or more tracks to a YouTube Music playlist (strips `VL` prefix when needed).
    pub async fn add_tracks_to_playlist(
        &self,
        playlist_id: String,
        video_ids: Vec<String>,
    ) -> Result<(), SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.add_tracks_to_playlist(&playlist_id, &video_ids).await?;
        Ok(())
    }

    /// Creates a new private YouTube Music playlist and optionally seeds it with `video_ids`.
    pub async fn create_playlist(
        &self,
        title: String,
        description: String,
        video_ids: Vec<String>,
    ) -> Result<String, SonataAndroidError> {
        let ytm = self.current_ytm();
        let pid = ytm
            .create_playlist_with_tracks(&title, &description, &video_ids)
            .await?;
        Ok(pid)
    }

    /// Saves or removes an external playlist in the signed-in user's YouTube Music library.
    pub async fn set_playlist_saved(
        &self,
        playlist_id: String,
        saved: bool,
    ) -> Result<(), SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.set_playlist_saved(&playlist_id, saved).await?;
        Ok(())
    }

    /// Updates a user-owned YouTube Music playlist's title, description, and/or privacy status.
    pub async fn edit_playlist_metadata(
        &self,
        playlist_id: String,
        title: Option<String>,
        description: Option<String>,
        privacy: Option<String>,
    ) -> Result<(), SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.edit_playlist_metadata(
            &playlist_id,
            title.as_deref(),
            description.as_deref(),
            privacy.as_deref(),
        )
        .await?;
        Ok(())
    }

    /// Uploads a custom cover image (`JPEG`/`PNG`/`WebP` bytes) to YouTube Music and binds it to the playlist.
    pub async fn upload_playlist_cover(
        &self,
        playlist_id: String,
        image_bytes: Vec<u8>,
        mime_type: String,
    ) -> Result<(), SonataAndroidError> {
        let ytm = self.current_ytm();
        ytm.upload_playlist_cover(&playlist_id, &image_bytes, &mime_type)
            .await?;
        Ok(())
    }

    /// Resolves a fast-start audio stream URL and HTTP headers for Media3 `ExoPlayer`.
    pub async fn resolve_stream(
        &self,
        track: AndroidTrack,
    ) -> Result<AndroidResolvedStream, SonataAndroidError> {
        if parse_server_item_id(&track.id).is_some() {
            let (profile, _item_ref) = self.find_server_for_item(&track.id).await?;
            let core_track: Track = track.into();
            let stream = self.server_client().resolve_stream(&profile, &core_track)?;
            let badge = stream
                .audio_badge
                .clone()
                .filter(|b| !b.trim().is_empty())
                .or_else(|| stream.codec.clone().map(|c| c.to_ascii_uppercase()))
                .unwrap_or_default();
            return Ok(AndroidResolvedStream {
                url: stream.url,
                badge,
                headers: stream
                    .headers
                    .into_iter()
                    .map(|(name, value)| AndroidHeader { name, value })
                    .collect(),
            });
        }
        let resolver = self.current_resolver();
        let core_track: Track = track.into();
        let stream = resolver.resolve_fast_typed(&core_track).await?;
        Ok(stream.into())
    }

    /// Invalidates stale cached streams and re-resolves a fresh recovery stream after mid-track error.
    pub async fn resolve_recovery(
        &self,
        track: AndroidTrack,
    ) -> Result<AndroidResolvedStream, SonataAndroidError> {
        if parse_server_item_id(&track.id).is_some() {
            return self.resolve_stream(track).await;
        }
        let resolver = self.current_resolver();
        let core_track: Track = track.into();
        let stream = resolver.resolve_recovery_typed(&core_track).await?;
        Ok(stream.into())
    }

    /// Attempts a background Hi-Fi stream upgrade and local cache download for an active track.
    pub async fn resolve_hifi_upgrade(
        &self,
        track: AndroidTrack,
        current: AndroidResolvedStream,
    ) -> Option<AndroidResolvedStream> {
        if parse_server_item_id(&track.id).is_some() {
            return None;
        }
        let resolver = self.current_resolver();
        let core_track: Track = track.into();
        let core_stream = ResolvedStream {
            url: current.url,
            badge: current.badge,
            headers: current
                .headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .collect(),
        };
        resolver
            .resolve_hifi_upgrade(&core_track, &core_stream, true)
            .await
            .map(AndroidResolvedStream::from)
    }

    /// Checks whether an official or counterpart Music Video exists for `track`.
    pub async fn resolve_music_video_candidate(
        &self,
        track: AndroidTrack,
    ) -> Option<AndroidMusicVideoCandidate> {
        if parse_server_item_id(&track.id).is_some() {
            return None;
        }
        let ytm = self.current_ytm();
        let core_track: Track = track.into();
        ytm.resolve_music_video_candidate(&core_track)
            .await
            .map(AndroidMusicVideoCandidate::from)
    }

    /// Resolves adaptive/fallback video quality streams and companion audio for `video_id`.
    pub async fn resolve_music_video_streams(
        &self,
        video_id: String,
    ) -> Result<AndroidResolvedMusicVideo, SonataAndroidError> {
        let resolver = self.current_resolver();
        let resolved = resolver
            .resolve_music_video_streams(&video_id)
            .await
            .map_err(|err| CoreError::classify_anyhow(&err))?;
        Ok(resolved.into())
    }

    /// Fetches lyrics hosted directly on a self-hosted server (WebDAV sidecar/FLAC or Jellyfin Lyrics) without online lookup.
    pub async fn fetch_server_lyrics(&self, track: AndroidTrack) -> Vec<AndroidLyricLine> {
        self.fetch_server_lyrics_internal(&track.id).await
    }

    /// Fetches synced or plain lyrics lines for the active track (preferring self-hosted server lyrics first).
    pub async fn fetch_lyrics(&self, track: AndroidTrack) -> Vec<AndroidLyricLine> {
        let server_lyrics = self.fetch_server_lyrics_internal(&track.id).await;
        if !server_lyrics.is_empty() {
            return server_lyrics;
        }
        let http = self.http.clone();
        let ytm = self.current_ytm();
        let cache = self.cache_db.clone();
        let paths = self.paths.clone();
        let core_track: Track = track.into();
        let state = lrclib::fetch_lyrics(
            &http,
            &ytm,
            &core_track,
            cache.as_ref(),
            &paths,
            DEFAULT_CACHE_LIMIT_BYTES,
        )
        .await;
        match state {
            LyricsState::Synced(lines) => lines
                .into_iter()
                .map(|l| AndroidLyricLine {
                    time_secs: l.time_secs,
                    text: l.text,
                    words: l
                        .words
                        .into_iter()
                        .map(|w| AndroidLyricWord {
                            start_secs: w.start_secs,
                            end_secs: w.end_secs,
                            text: w.text,
                        })
                        .collect(),
                    is_counter_voice: l.voice == sonata_core::lyrics::LyricVoice::Counter,
                })
                .collect(),
            LyricsState::Plain(lines) => lines
                .into_iter()
                .map(|text| AndroidLyricLine {
                    time_secs: -1.0,
                    text,
                    words: Vec::new(),
                    is_counter_voice: false,
                })
                .collect(),
            LyricsState::Idle | LyricsState::Loading | LyricsState::NotFound => Vec::new(),
        }
    }

    /// Clears persisted lyrics from SQLite enrichment and on-disk `.lrc`/`.txt` cache files.
    pub async fn clear_lyrics_cache(&self) {
        let cache_db = self.cache_db.clone();
        let lyrics_dir = self.paths.lyrics_cache_dir();
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(ref db) = cache_db {
                db.clear_lyrics_cache();
            } else {
                let _ = std::fs::remove_dir_all(lyrics_dir);
            }
        })
        .await;
    }

    /// Checks GitHub Releases for a newer Sonata Android APK release.
    pub async fn check_for_updates(&self) -> Result<AndroidUpdateInfo, SonataAndroidError> {
        let http = self.http.clone();
        let res = sonata_core::updater::check_for_updates_for(
            &http,
            sonata_core::updater::UpdateTarget::Android,
            sonata_core::updater::CURRENT_VERSION,
        )
        .await?;
        Ok(res.into())
    }

    /// Checks GitHub Releases for a newer Sonata Android APK release against an explicit installed versionName.
    pub async fn check_for_updates_for_version(
        &self,
        current_version: String,
    ) -> Result<AndroidUpdateInfo, SonataAndroidError> {
        let http = self.http.clone();
        let res = sonata_core::updater::check_for_updates_for(
            &http,
            sonata_core::updater::UpdateTarget::Android,
            &current_version,
        )
        .await?;
        Ok(res.into())
    }

    /// Searches JioSaavn catalog using the shared Rust provider parser and YTM attribution.
    pub async fn search_jiosaavn(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let http = self.http.clone();
        let tracks = jiosaavn::search_tracks_typed(&http, &query, limit as usize).await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Resolves a direct `jiosaavn:<id>` track stream via shared Rust DES-ECB decryption.
    pub async fn resolve_jiosaavn_by_id(
        &self,
        raw_id: String,
    ) -> Result<Option<AndroidJioSaavnStream>, SonataAndroidError> {
        let http = self.http.clone();
        let resolved = jiosaavn::resolve_by_id_typed(&http, &raw_id).await?;
        Ok(resolved.map(AndroidJioSaavnStream::from))
    }

    /// Searches JioSaavn for a strict metadata match whose verified bitrate beats `current_bitrate_kbps`.
    pub async fn find_jiosaavn_upgrade(
        &self,
        track: AndroidTrack,
        current_bitrate_kbps: u32,
    ) -> Result<Option<AndroidJioSaavnStream>, SonataAndroidError> {
        let http = self.http.clone();
        let core_track = Track::from(track);
        let resolved =
            jiosaavn::find_higher_quality_stream_typed(&http, &core_track, current_bitrate_kbps)
                .await?;
        Ok(resolved.map(AndroidJioSaavnStream::from))
    }

    /// Discovers platform-neutral animated album artwork across Apple Music, Tidal, and Community Canvas.
    pub async fn resolve_motion_media(
        &self,
        track: AndroidTrack,
        storefront: Option<String>,
        country_code: Option<String>,
    ) -> Result<Option<AndroidMotionMedia>, SonataAndroidError> {
        let http = self.http.clone();
        let core_track = Track::from(track);
        let found = sonata_core::motion::resolve_motion_media(
            &http,
            &core_track,
            storefront.as_deref(),
            country_code.as_deref(),
        )
        .await;
        Ok(found.map(AndroidMotionMedia::from))
    }

    /// Lists all configured self-hosted music servers (`Jellyfin`, `Subsonic`, `WebDAV`, `SMB`).
    pub async fn list_servers(&self) -> Result<Vec<AndroidServerProfile>, SonataAndroidError> {
        let _guard = self.server_lock.lock().await;
        let profiles = self.load_servers_internal().await?;
        Ok(profiles
            .into_iter()
            .map(AndroidServerProfile::from)
            .collect())
    }

    /// Creates or updates a self-hosted music server profile and persists it with `0o600` permissions.
    pub async fn upsert_server(
        &self,
        profile: AndroidServerProfile,
    ) -> Result<Vec<AndroidServerProfile>, SonataAndroidError> {
        let mut core_profile: ServerProfile = profile.into();
        core_profile.name = core_profile.name.trim().to_string();
        core_profile.base_url = core_profile.normalized_base_url();
        if core_profile.name.is_empty() {
            return Err(SonataAndroidError::InvalidResponse {
                provider: "servers".to_string(),
                reason: "Server name must not be empty".to_string(),
            });
        }
        let has_smb_mount = core_profile.kind == ServerKind::Smb
            && core_profile
                .local_mount_path
                .as_ref()
                .is_some_and(|p| !p.as_os_str().is_empty());
        if core_profile.base_url.is_empty() && !has_smb_mount {
            return Err(SonataAndroidError::InvalidResponse {
                provider: "servers".to_string(),
                reason: "Server base URL must not be empty".to_string(),
            });
        }
        if core_profile.id.trim().is_empty() {
            core_profile.id = self.generate_server_id(core_profile.kind);
        } else {
            core_profile.id = core_profile.id.trim().to_string();
        }
        sonata_core::providers::servers::validate_server_transport(&core_profile).await?;
        core_profile = self.ensure_jellyfin_auth_if_needed(core_profile).await?;

        let _guard = self.server_lock.lock().await;
        let mut existing = self.load_servers_internal().await?;
        if let Some(pos) = existing.iter().position(|p| p.id == core_profile.id) {
            existing[pos] = core_profile;
        } else {
            existing.push(core_profile);
        }
        let paths = self.paths.clone();
        let to_save = existing.clone();
        tokio::task::spawn_blocking(move || save_server_profiles(&paths, &to_save))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Server config write task failed: {err}"),
            })??;
        Ok(existing
            .into_iter()
            .map(AndroidServerProfile::from)
            .collect())
    }

    /// Deletes a configured self-hosted music server profile by `server_id`.
    pub async fn delete_server(
        &self,
        server_id: String,
    ) -> Result<Vec<AndroidServerProfile>, SonataAndroidError> {
        let _guard = self.server_lock.lock().await;
        let mut existing = self.load_servers_internal().await?;
        existing.retain(|p| p.id != server_id);
        let paths = self.paths.clone();
        let to_save = existing.clone();
        tokio::task::spawn_blocking(move || save_server_profiles(&paths, &to_save))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Server config write task failed: {err}"),
            })??;
        Ok(existing
            .into_iter()
            .map(AndroidServerProfile::from)
            .collect())
    }

    /// Enables or disables an existing self-hosted music server profile.
    pub async fn set_server_enabled(
        &self,
        server_id: String,
        enabled: bool,
    ) -> Result<Vec<AndroidServerProfile>, SonataAndroidError> {
        let _guard = self.server_lock.lock().await;
        let mut existing = self.load_servers_internal().await?;
        let target = existing
            .iter_mut()
            .find(|p| p.id == server_id)
            .ok_or_else(|| SonataAndroidError::NotFound {
                reason: format!("Music server `{server_id}` was not found"),
            })?;
        target.enabled = enabled;
        let paths = self.paths.clone();
        let to_save = existing.clone();
        tokio::task::spawn_blocking(move || save_server_profiles(&paths, &to_save))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Server config write task failed: {err}"),
            })??;
        Ok(existing
            .into_iter()
            .map(AndroidServerProfile::from)
            .collect())
    }

    /// Tests connectivity and credentials against a server profile without persisting it.
    pub async fn test_server_connection(
        &self,
        profile: AndroidServerProfile,
    ) -> Result<String, SonataAndroidError> {
        let mut core_profile: ServerProfile = profile.into();
        core_profile.base_url = core_profile.normalized_base_url();
        sonata_core::providers::servers::validate_server_transport(&core_profile).await?;
        core_profile = self.ensure_jellyfin_auth_if_needed(core_profile).await?;
        let status = self.server_client().ping(&core_profile).await?;
        Ok(status)
    }

    /// Probes reachability and measures round-trip ping latency for a persisted server.
    pub async fn probe_server_health(
        &self,
        server_id: String,
    ) -> Result<AndroidServerHealthReport, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let report = self.server_client().probe_server_health(&profile).await;
        Ok(AndroidServerHealthReport::from(report))
    }

    /// Runs a real sample download/read throughput benchmark (`Mbps`) against a persisted server.
    pub async fn run_server_speed_test(
        &self,
        server_id: String,
    ) -> Result<AndroidServerSpeedTestReport, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let report = self.server_client().run_speed_test(&profile).await;
        Ok(AndroidServerSpeedTestReport::from(report))
    }

    /// Authenticates against a Jellyfin server using username and password, returning an access token and user ID.
    pub async fn authenticate_jellyfin(
        &self,
        base_url: String,
        username: String,
        password: String,
        device_id: String,
    ) -> Result<AndroidJellyfinAuth, SonataAndroidError> {
        let auth =
            jellyfin::authenticate_by_name(&self.http, &base_url, &username, &password, &device_id)
                .await?;
        Ok(AndroidJellyfinAuth {
            access_token: auth.access_token,
            user_id: auth.user_id,
            user_name: auth.user_name,
            server_id: auth.server_id,
        })
    }

    /// Lists music libraries or top-level WebDAV folders exposed by `server_id`.
    pub async fn list_server_libraries(
        &self,
        server_id: String,
    ) -> Result<Vec<AndroidServerLibrary>, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let libraries = self.server_client().list_libraries(&profile).await?;
        Ok(libraries
            .into_iter()
            .map(AndroidServerLibrary::from)
            .collect())
    }

    /// Lists album artists from `server_id`, optionally filtered by `library_id`.
    pub async fn list_server_artists(
        &self,
        server_id: String,
        library_id: Option<String>,
    ) -> Result<Vec<AndroidHomeCard>, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let cards = self
            .server_client()
            .list_artists(&profile, library_id.as_deref())
            .await?;
        Ok(cards.into_iter().map(AndroidHomeCard::from).collect())
    }

    /// Lists albums from `server_id`, optionally filtered by `library_id` or `artist_id`.
    pub async fn list_server_albums(
        &self,
        server_id: String,
        library_id: Option<String>,
        artist_id: Option<String>,
    ) -> Result<Vec<AndroidHomeCard>, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let raw_artist = artist_id
            .as_deref()
            .map(|aid| parse_server_item_id(aid).map_or_else(|| aid.to_string(), |r| r.item_id));
        let cards = self
            .server_client()
            .list_albums(&profile, library_id.as_deref(), raw_artist.as_deref())
            .await?;
        Ok(cards.into_iter().map(AndroidHomeCard::from).collect())
    }

    /// Fetches album metadata and tracks for a prefixed self-hosted `album_id`.
    pub async fn get_server_album(
        &self,
        album_id: String,
    ) -> Result<AndroidServerAlbum, SonataAndroidError> {
        let (profile, _) = self.find_server_for_item(&album_id).await?;
        let detail = self.server_client().get_album(&profile, &album_id).await?;
        Ok(AndroidServerAlbum {
            id: detail.id,
            title: detail.title,
            subtitle: detail.subtitle,
            year: detail.year,
            cover_url: detail.cover_url,
            tracks: detail.tracks.into_iter().map(AndroidTrack::from).collect(),
        })
    }

    /// Lists playlists exposed by `server_id`.
    pub async fn list_server_playlists(
        &self,
        server_id: String,
    ) -> Result<Vec<AndroidPlaylist>, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let playlists = self.server_client().list_playlists(&profile).await?;
        Ok(playlists.into_iter().map(AndroidPlaylist::from).collect())
    }

    /// Fetches tracks for a prefixed self-hosted `playlist_id` (or WebDAV folder item ID).
    pub async fn get_server_playlist_tracks(
        &self,
        playlist_id: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let (profile, _) = self.find_server_for_item(&playlist_id).await?;
        let tracks = self
            .server_client()
            .get_playlist_tracks(&profile, &playlist_id)
            .await?;
        Ok(tracks.into_iter().map(AndroidTrack::from).collect())
    }

    /// Lists immediate subfolders and audio tracks for a WebDAV folder path (`None` = root).
    pub async fn browse_webdav_folder(
        &self,
        server_id: String,
        folder_path: Option<String>,
    ) -> Result<AndroidWebDavListing, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        if profile.kind != ServerKind::WebDav {
            return Err(SonataAndroidError::NotFound {
                reason: format!("Server `{}` is not a WebDAV server", profile.id),
            });
        }
        let target = folder_path.as_deref().map(|raw| {
            parse_server_item_id(raw)
                .filter(|item| item.kind == ServerKind::WebDav)
                .map_or_else(|| raw.to_string(), |item| item.item_id)
        });
        let listing = webdav::browse_folder_enriched(
            &self.http,
            &profile,
            target.as_deref(),
            Some(&self.paths.cache_dir),
        )
        .await?;
        let folders = listing
            .folders
            .into_iter()
            .map(|folder| AndroidServerLibrary {
                id: encode_server_item_id(ServerKind::WebDav, &profile.id, &folder.path),
                name: folder.name,
                kind: "folder".to_string(),
            })
            .collect();
        let tracks = listing.tracks.into_iter().map(AndroidTrack::from).collect();
        Ok(AndroidWebDavListing { folders, tracks })
    }

    /// Searches a single configured self-hosted server (`server_id`).
    pub async fn search_server(
        &self,
        server_id: String,
        query: String,
        limit: u32,
    ) -> Result<AndroidServerSearchResult, SonataAndroidError> {
        let profile = self.find_server_profile(&server_id).await?;
        let res = self
            .server_client()
            .search(&profile, &query, limit as usize)
            .await?;
        Ok(AndroidServerSearchResult::from(res))
    }

    /// Searches all enabled self-hosted servers concurrently and merges results.
    pub async fn search_all_servers(
        &self,
        query: String,
        limit_per_server: u32,
    ) -> Result<AndroidServerSearchResult, SonataAndroidError> {
        let clean_query = query.trim().to_string();
        if clean_query.is_empty() {
            return Ok(AndroidServerSearchResult::default());
        }
        let profiles = self.load_servers_internal().await?;
        let enabled: Vec<ServerProfile> = profiles.into_iter().filter(|p| p.enabled).collect();
        if enabled.is_empty() {
            return Ok(AndroidServerSearchResult::default());
        }

        let client = self.server_client();
        let mut join_set = tokio::task::JoinSet::new();
        for profile in enabled {
            let client = client.clone();
            let q = clean_query.clone();
            let lim = limit_per_server as usize;
            join_set.spawn(async move { client.search(&profile, &q, lim).await });
        }

        let mut merged = AndroidServerSearchResult::default();
        while let Some(joined) = join_set.join_next().await {
            if let Ok(Ok(res)) = joined {
                merged
                    .tracks
                    .extend(res.tracks.into_iter().map(AndroidTrack::from));
                merged
                    .albums
                    .extend(res.albums.into_iter().map(AndroidHomeCard::from));
                merged
                    .artists
                    .extend(res.artists.into_iter().map(AndroidHomeCard::from));
                merged
                    .playlists
                    .extend(res.playlists.into_iter().map(AndroidHomeCard::from));
            }
        }
        Ok(merged)
    }

    /// Resolves a direct stream URL and HTTP auth headers for a prefixed self-hosted `track`.
    pub async fn resolve_server_stream(
        &self,
        track: AndroidTrack,
    ) -> Result<AndroidServerStream, SonataAndroidError> {
        let (profile, _) = self.find_server_for_item(&track.id).await?;
        sonata_core::providers::servers::validate_server_transport(&profile).await?;
        let core_track: Track = track.into();
        let stream = self.server_client().resolve_stream(&profile, &core_track)?;
        Ok(AndroidServerStream::from(stream))
    }

    /// Lists all locally persisted personal playlists.
    pub async fn list_personal_playlists(
        &self,
    ) -> Result<Vec<AndroidPersonalPlaylist>, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let items = tokio::task::spawn_blocking(move || store.list())
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Personal playlists read task failed: {err}"),
            })??;
        Ok(items
            .into_iter()
            .map(AndroidPersonalPlaylist::from)
            .collect())
    }

    /// Creates a new locally persisted personal playlist.
    pub async fn create_personal_playlist(
        &self,
        name: String,
    ) -> Result<AndroidPersonalPlaylist, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let created = tokio::task::spawn_blocking(move || store.create(&name))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Personal playlists create task failed: {err}"),
            })??;
        Ok(AndroidPersonalPlaylist::from(created))
    }

    /// Renames an existing locally persisted personal playlist.
    pub async fn rename_personal_playlist(
        &self,
        playlist_id: String,
        new_name: String,
    ) -> Result<AndroidPersonalPlaylist, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let updated = tokio::task::spawn_blocking(move || store.rename(&playlist_id, &new_name))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Personal playlists rename task failed: {err}"),
            })??;
        Ok(AndroidPersonalPlaylist::from(updated))
    }

    /// Deletes a locally persisted personal playlist by ID.
    pub async fn delete_personal_playlist(
        &self,
        playlist_id: String,
    ) -> Result<bool, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let removed = tokio::task::spawn_blocking(move || store.delete(&playlist_id))
            .await
            .map_err(|err| SonataAndroidError::Internal {
                reason: format!("Personal playlists delete task failed: {err}"),
            })??;
        Ok(removed)
    }

    /// Appends a local, server, or streaming track to a personal playlist without calling YouTube Music APIs.
    pub async fn add_track_to_personal_playlist(
        &self,
        playlist_id: String,
        track: AndroidTrack,
    ) -> Result<AndroidPersonalPlaylist, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let core_track: Track = track.into();
        let updated =
            tokio::task::spawn_blocking(move || store.add_track(&playlist_id, core_track))
                .await
                .map_err(|err| SonataAndroidError::Internal {
                    reason: format!("Personal playlists add-track task failed: {err}"),
                })??;
        Ok(AndroidPersonalPlaylist::from(updated))
    }

    /// Removes a track by ID from a locally persisted personal playlist.
    pub async fn remove_track_from_personal_playlist(
        &self,
        playlist_id: String,
        track_id: String,
    ) -> Result<AndroidPersonalPlaylist, SonataAndroidError> {
        let store = self.personal_playlists.clone();
        let updated =
            tokio::task::spawn_blocking(move || store.remove_track(&playlist_id, &track_id))
                .await
                .map_err(|err| SonataAndroidError::Internal {
                    reason: format!("Personal playlists remove-track task failed: {err}"),
                })??;
        Ok(AndroidPersonalPlaylist::from(updated))
    }

    /// Attributes a local or self-hosted track play to YouTube Music listening history when signed in.
    pub async fn attribute_personal_track_to_ytm(
        &self,
        track: AndroidTrack,
    ) -> Result<Option<String>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let core_track: Track = track.into();
        let matched = ytm.attribute_personal_track_to_ytm(&core_track).await?;
        Ok(matched)
    }

    /// Loads persisted Last.fm and ListenBrainz scrobbling configuration (`scrobble.json`).
    pub async fn get_scrobble_config(&self) -> Result<AndroidScrobbleConfig, SonataAndroidError> {
        let _guard = self.scrobble_lock.lock().await;
        let cfg = self.load_scrobble_config_internal().await?;
        Ok(AndroidScrobbleConfig::from(cfg))
    }

    /// Atomically saves Last.fm and ListenBrainz scrobbling configuration (`0o700` dir / `0o600` file).
    pub async fn save_scrobble_config(
        &self,
        config: AndroidScrobbleConfig,
    ) -> Result<AndroidScrobbleConfig, SonataAndroidError> {
        let _guard = self.scrobble_lock.lock().await;
        let paths = self.paths.clone();
        let core_cfg: ScrobbleConfig = config.into();
        let saved = tokio::task::spawn_blocking(move || -> Result<ScrobbleConfig, CoreError> {
            core_cfg.save_to_paths(&paths)?;
            Ok(core_cfg)
        })
        .await
        .map_err(|err| SonataAndroidError::Internal {
            reason: format!("Scrobble config write task failed: {err}"),
        })??;
        Ok(AndroidScrobbleConfig::from(saved))
    }

    /// Sends a `Now Playing` update to configured Last.fm / ListenBrainz scrobblers.
    pub async fn scrobble_now_playing(
        &self,
        track: AndroidTrack,
    ) -> Result<(), SonataAndroidError> {
        let cfg = {
            let _guard = self.scrobble_lock.lock().await;
            self.load_scrobble_config_internal().await?
        };
        if !cfg.any_configured() {
            return Ok(());
        }
        let core_track: Track = track.into();
        let scrobble_track = ScrobbleTrack::from_track(&core_track);
        self.scrobble_client
            .update_now_playing(&cfg, &scrobble_track)
            .await?;
        Ok(())
    }

    /// Submits a completed scrobble to configured Last.fm / ListenBrainz scrobblers.
    pub async fn scrobble_submit(
        &self,
        track: AndroidTrack,
        timestamp_unix: Option<u64>,
    ) -> Result<(), SonataAndroidError> {
        let cfg = {
            let _guard = self.scrobble_lock.lock().await;
            self.load_scrobble_config_internal().await?
        };
        if !cfg.any_configured() {
            return Ok(());
        }
        let core_track: Track = track.into();
        let scrobble_track = ScrobbleTrack::from_track(&core_track);
        self.scrobble_client
            .scrobble(&cfg, &scrobble_track, timestamp_unix)
            .await?;
        Ok(())
    }

    /// Records a single on-device listening event into SQLite `stats.db` via `spawn_blocking`.
    pub async fn record_listening_stat(
        &self,
        track: AndroidTrack,
        source_tag: String,
        played_secs: u64,
        played_at_unix: Option<u64>,
    ) -> Result<(), SonataAndroidError> {
        let stats_db = Arc::clone(&self.stats_db);
        let paths = self.paths.clone();
        let resolved_tag = Self::infer_source_tag(&track.id, &source_tag);
        let duration_secs = track.duration_secs.unwrap_or(0);
        let timestamp = played_at_unix.unwrap_or_else(Self::current_unix_secs);
        tokio::task::spawn_blocking(move || {
            let db = Self::open_or_get_stats_db_blocking(&stats_db, &paths)?;
            db.record_play(
                &track.id,
                &track.title,
                &track.artist,
                &track.album,
                &resolved_tag,
                duration_secs,
                played_secs,
                timestamp,
            )?;
            Ok::<(), SonataAndroidError>(())
        })
        .await
        .map_err(|err| SonataAndroidError::Internal {
            reason: format!("Listening stats record task failed: {err}"),
        })??;
        Ok(())
    }

    /// Queries on-device listening statistics from SQLite `stats.db` for the requested time window.
    pub async fn query_listening_stats(
        &self,
        window: AndroidStatsWindow,
    ) -> Result<AndroidStatsSummary, SonataAndroidError> {
        let stats_db = Arc::clone(&self.stats_db);
        let paths = self.paths.clone();
        let core_window: StatsWindow = window.into();
        let summary = tokio::task::spawn_blocking(move || {
            let db = Self::open_or_get_stats_db_blocking(&stats_db, &paths)?;
            db.query_summary(core_window)
                .map_err(SonataAndroidError::from)
        })
        .await
        .map_err(|err| SonataAndroidError::Internal {
            reason: format!("Listening stats query task failed: {err}"),
        })??;
        Ok(AndroidStatsSummary::from(summary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_jiosaavn_stream_and_motion_media_records() {
        let js = AndroidJioSaavnStream::from(ResolvedJioSaavnStream {
            url: "https://aac.saavncdn.com/123/song_320.mp4".to_string(),
            bitrate_kbps: 320,
            badge: "AAC 320kbps · JioSaavn".to_string(),
            title: "Starboy".to_string(),
            artist: "The Weeknd".to_string(),
            duration_secs: 230,
        });
        assert_eq!(js.bitrate_kbps, 320);
        assert_eq!(js.badge, "AAC 320kbps · JioSaavn");

        let motion = AndroidMotionMedia::from(MotionMedia {
            url: "https://mvod.itunes.apple.com/square.m3u8".to_string(),
            fallback_url: Some("https://mvod.itunes.apple.com/tall.m3u8".to_string()),
            portrait_url: Some("https://mvod.itunes.apple.com/tall.m3u8".to_string()),
            kind: MotionMediaKind::Hls,
            source: sonata_core::motion::MotionMediaSource::AppleMusic,
        });
        assert!(motion.is_hls);
        assert_eq!(motion.source, "Apple Music");
        assert_eq!(
            motion.portrait_url.as_deref(),
            Some("https://mvod.itunes.apple.com/tall.m3u8")
        );
    }

    #[test]
    fn disk_cache_roundtrip_permissions_and_removal() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "sonata_android_cache_test_{}_{}",
            std::process::id(),
            nanos
        ));

        let profile = AndroidAccountProfile {
            name: "Tester".to_string(),
            email: Some("tester@example.com".to_string()),
            handle: Some("@tester".to_string()),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
        };
        save_cached_json_in(&dir, CACHE_PROFILE_FILE, &profile);

        #[cfg(unix)]
        {
            let dir_mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
            let file_mode = fs::metadata(dir.join(CACHE_PROFILE_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
        }

        let loaded_profile: AndroidAccountProfile =
            load_cached_json_from(&dir, CACHE_PROFILE_FILE).expect("profile should load");
        assert_eq!(loaded_profile.name, profile.name);
        assert_eq!(loaded_profile.email, profile.email);
        assert_eq!(loaded_profile.handle, profile.handle);
        assert_eq!(loaded_profile.avatar_url, profile.avatar_url);

        // Simulate a legacy android_home.json left by an older build and confirm cleanup removes it.
        let legacy_shelves = vec![AndroidHomeShelf {
            title: "Quick picks".to_string(),
            tracks: vec![],
            cards: vec![AndroidHomeCard {
                kind: AndroidHomeCardKind::Song,
                id: "vid123".to_string(),
                title: "Song".to_string(),
                subtitle: "Artist".to_string(),
                thumbnail_url: None,
                track: None,
            }],
        }];
        save_cached_json_in(&dir, CACHE_HOME_FILE, &legacy_shelves);
        assert!(dir.join(CACHE_HOME_FILE).exists());
        remove_cached_json_from(&dir, CACHE_HOME_FILE);
        assert!(!dir.join(CACHE_HOME_FILE).exists());

        let playlists = vec![AndroidPlaylist {
            id: "PL123".to_string(),
            name: "Favorites".to_string(),
            thumbnail_url: None,
            is_owned: true,
        }];
        save_cached_json_in(&dir, CACHE_PLAYLISTS_FILE, &playlists);
        let loaded_playlists: Vec<AndroidPlaylist> =
            load_cached_json_from(&dir, CACHE_PLAYLISTS_FILE).expect("playlists should load");
        assert_eq!(loaded_playlists.len(), 1);
        assert_eq!(loaded_playlists[0].id, "PL123");

        remove_cached_json_from(&dir, CACHE_PROFILE_FILE);
        assert!(load_cached_json_from::<AndroidAccountProfile>(&dir, CACHE_PROFILE_FILE).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn track_card_and_shelf_conversions_preserve_all_fields() {
        let track = Track {
            id: "vid_123".to_string(),
            title: "Starboy".to_string(),
            artist: "The Weeknd".to_string(),
            album: "Starboy".to_string(),
            duration_secs: Some(230),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("256kbps".to_string()),
            set_video_id: Some("set_9".to_string()),
            liked: true,
            album_id: Some("MPREb_123".to_string()),
            is_explicit: true,
        };

        let android_track = AndroidTrack::from(&track);
        assert_eq!(android_track.id, "vid_123");
        assert_eq!(android_track.title, "Starboy");
        assert_eq!(android_track.artist, "The Weeknd");
        assert_eq!(android_track.album, "Starboy");
        assert_eq!(android_track.duration_secs, Some(230));
        assert_eq!(
            android_track.thumbnail_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert!(android_track.liked);
        assert_eq!(android_track.album_id.as_deref(), Some("MPREb_123"));
        assert!(android_track.is_explicit);

        let roundtrip_track = Track::from(android_track.clone());
        assert_eq!(roundtrip_track.id, track.id);
        assert_eq!(roundtrip_track.title, track.title);
        assert_eq!(roundtrip_track.artist, track.artist);
        assert_eq!(roundtrip_track.album, track.album);
        assert_eq!(roundtrip_track.duration_secs, track.duration_secs);
        assert_eq!(roundtrip_track.thumbnail_url, track.thumbnail_url);
        assert_eq!(roundtrip_track.liked, track.liked);
        assert_eq!(roundtrip_track.album_id, track.album_id);
        assert_eq!(roundtrip_track.is_explicit, track.is_explicit);

        let playlist = Playlist {
            id: "PL_42".to_string(),
            name: "Night Mix".to_string(),
            source: ProviderSource::YouTubeMusic,
            track_count: Some(12),
            thumbnail_url: Some("https://example.com/pl.jpg".to_string()),
            is_owned: true,
        };
        let android_pl = AndroidPlaylist::from(&playlist);
        assert_eq!(android_pl.id, "PL_42");
        assert_eq!(android_pl.name, "Night Mix");
        assert!(android_pl.is_owned);
        assert_eq!(
            android_pl.thumbnail_url.as_deref(),
            Some("https://example.com/pl.jpg")
        );

        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Playlist),
            AndroidHomeCardKind::Playlist
        );
        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Album),
            AndroidHomeCardKind::Album
        );
        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Artist),
            AndroidHomeCardKind::Artist
        );
        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Song),
            AndroidHomeCardKind::Song
        );
        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Video),
            AndroidHomeCardKind::Video
        );
        assert_eq!(
            AndroidHomeCardKind::from(HomeCardKind::Mood),
            AndroidHomeCardKind::Mood
        );

        let home_card = HomeCard {
            kind: HomeCardKind::Song,
            id: "vid_123".to_string(),
            title: "Starboy".to_string(),
            subtitle: "The Weeknd".to_string(),
            thumbnail_url: Some("https://example.com/card.jpg".to_string()),
            track: Some(track.clone()),
        };
        let shelf = HomeShelf {
            title: "Quick picks".to_string(),
            tracks: vec![track],
            cards: vec![home_card],
        };
        let android_shelf = AndroidHomeShelf::from(&shelf);
        assert_eq!(android_shelf.title, "Quick picks");
        assert_eq!(android_shelf.tracks.len(), 1);
        assert_eq!(android_shelf.cards.len(), 1);
        assert_eq!(android_shelf.cards[0].kind, AndroidHomeCardKind::Song);
        assert_eq!(
            android_shelf.cards[0].track.as_ref().map(|t| t.id.as_str()),
            Some("vid_123")
        );
    }

    #[test]
    fn artist_page_stream_and_update_info_conversions() {
        let artist_page = ArtistPage {
            id: "UC12345".to_string(),
            name: "The Weeknd".to_string(),
            subtitle: "Artist".to_string(),
            description: Some("Bio".to_string()),
            subscribers: Some("36M subscribers".to_string()),
            monthly_listeners: Some("110M monthly listeners".to_string()),
            thumbnail_url: Some("https://example.com/artist.jpg".to_string()),
            shuffle_playlist_id: Some("RDAM_1".to_string()),
            radio_playlist_id: Some("RD_1".to_string()),
            latest_release: Some(HomeCard {
                kind: HomeCardKind::Album,
                id: "MPREb_1".to_string(),
                title: "Hurry Up Tomorrow".to_string(),
                subtitle: "2025".to_string(),
                thumbnail_url: None,
                track: None,
            }),
            popular_tracks: Vec::new(),
            releases: Vec::new(),
            related_artists: Vec::new(),
        };
        let android_artist = AndroidArtistPage::from(&artist_page);
        assert_eq!(android_artist.id, "UC12345");
        assert_eq!(android_artist.name, "The Weeknd");
        assert_eq!(android_artist.description.as_deref(), Some("Bio"));
        assert_eq!(
            android_artist.monthly_listeners.as_deref(),
            Some("110M monthly listeners")
        );
        assert_eq!(
            android_artist
                .latest_release
                .as_ref()
                .map(|c| c.title.as_str()),
            Some("Hurry Up Tomorrow")
        );

        let stream = ResolvedStream {
            url: "https://example.com/audio.webm".to_string(),
            badge: "Opus 251 · Extreme".to_string(),
            headers: vec![("User-Agent".to_string(), "SonataTest/1.0".to_string())],
        };
        let android_stream = AndroidResolvedStream::from(stream);
        assert_eq!(android_stream.url, "https://example.com/audio.webm");
        assert_eq!(android_stream.badge, "Opus 251 · Extreme");
        assert_eq!(android_stream.headers.len(), 1);
        assert_eq!(android_stream.headers[0].name, "User-Agent");
        assert_eq!(android_stream.headers[0].value, "SonataTest/1.0");

        let update = sonata_core::updater::UpdateCheckResult {
            current_version: "1.0.1".to_string(),
            latest_version: "1.0.2".to_string(),
            update_available: true,
            release_url: "https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.2".to_string(),
            apk_download_url: Some(
                "https://github.com/MehmetCanWT/sonata/releases/download/v1.0.2/Sonata-v1.0.2-arm64.apk"
                    .to_string(),
            ),
            appimage_download_url: None,
            release_notes: "## Highlights".to_string(),
        };
        let android_update = AndroidUpdateInfo::from(update);
        assert!(android_update.update_available);
        assert_eq!(android_update.current_version, "1.0.1");
        assert_eq!(android_update.latest_version, "1.0.2");
        assert!(android_update
            .apk_download_url
            .as_deref()
            .unwrap()
            .contains("Sonata-v1.0.2-arm64.apk"));

        let err = SonataAndroidError::from(anyhow::anyhow!("boom"));
        assert_eq!(err.to_string(), "boom");

        let chip = AndroidMoodChip::from(MoodChip {
            label: "Energize".to_string(),
            params: "ggMPOg1uX1lMbVZ".to_string(),
        });
        assert_eq!(chip.label, "Energize");
        assert_eq!(chip.params, "ggMPOg1uX1lMbVZ");

        let mood = AndroidMoodCategory::from(MoodCategoryTile {
            title: "Focus".to_string(),
            browse_id: "FEmusic_moods_and_genres_category".to_string(),
            params: "ggMPOg1uX1JOQWZ".to_string(),
            accent_rgb: (0x12, 0x34, 0x56),
        });
        assert_eq!(mood.title, "Focus");
        assert_eq!(mood.browse_id, "FEmusic_moods_and_genres_category");
        assert_eq!(mood.params, "ggMPOg1uX1JOQWZ");
        assert_eq!(mood.accent_color, 0xFF12_3456);
    }

    struct MockJsCipherSolver {
        load_calls: std::sync::atomic::AtomicUsize,
        fail_next_load: std::sync::atomic::AtomicBool,
    }

    impl AndroidJsCipherSolver for MockJsCipherSolver {
        fn ensure_loaded(&self, _player_hash: String, _hooked_script: String) -> bool {
            self.load_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            !self
                .fail_next_load
                .swap(false, std::sync::atomic::Ordering::SeqCst)
        }

        fn solve(
            &self,
            player_hash: String,
            sig: String,
            n: String,
        ) -> Option<AndroidSolvedCipher> {
            if player_hash == "hash_a" {
                Some(AndroidSolvedCipher {
                    sig: Some(format!("decoded_{sig}")),
                    n: Some(format!("transformed_{n}")),
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn cipher_bridge_adapter_caches_loaded_hash_and_delegates_solve() {
        let solver = Arc::new(MockJsCipherSolver {
            load_calls: std::sync::atomic::AtomicUsize::new(0),
            fail_next_load: std::sync::atomic::AtomicBool::new(true),
        });

        struct SharedSolver(Arc<MockJsCipherSolver>);
        impl AndroidJsCipherSolver for SharedSolver {
            fn ensure_loaded(&self, player_hash: String, hooked_script: String) -> bool {
                self.0.ensure_loaded(player_hash, hooked_script)
            }
            fn solve(
                &self,
                player_hash: String,
                sig: String,
                n: String,
            ) -> Option<AndroidSolvedCipher> {
                self.0.solve(player_hash, sig, n)
            }
        }

        let adapter = AndroidCipherBridgeAdapter {
            solver: Box::new(SharedSolver(Arc::clone(&solver))),
            loaded_hash: std::sync::Mutex::new(None),
        };

        // Failed load is not cached, so the second attempt invokes ensure_loaded again.
        assert!(!adapter.ensure_loaded("hash_a", "script_a"));
        assert!(adapter.ensure_loaded("hash_a", "script_a"));
        // Subsequent call with the same loaded hash hits the cache without calling ensure_loaded.
        assert!(adapter.ensure_loaded("hash_a", "script_a"));
        assert_eq!(
            solver.load_calls.load(std::sync::atomic::Ordering::SeqCst),
            2
        );

        let solved = adapter
            .solve("hash_a", "s1", "n1")
            .expect("solve should return mapped pair");
        assert_eq!(solved.sig.as_deref(), Some("decoded_s1"));
        assert_eq!(solved.n.as_deref(), Some("transformed_n1"));

        // Different player hash invalidates cache and triggers a fresh load.
        assert!(adapter.ensure_loaded("hash_b", "script_b"));
        assert_eq!(
            solver.load_calls.load(std::sync::atomic::Ordering::SeqCst),
            3
        );
        assert!(adapter.solve("hash_b", "s1", "n1").is_none());
    }

    #[tokio::test]
    async fn engine_lifecycle_auth_validation_and_sign_out() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base_dir = std::env::temp_dir().join(format!(
            "sonata_android_engine_test_{}_{}",
            std::process::id(),
            nanos
        ));
        let cfg_dir = base_dir.join("config");
        let cache_dir = base_dir.join("cache");
        let legacy_home = cache_dir.join("sonata").join(CACHE_HOME_FILE);
        fs::create_dir_all(legacy_home.parent().unwrap()).unwrap();
        fs::write(&legacy_home, b"[]").unwrap();
        assert!(legacy_home.exists());

        let engine = SonataAndroidEngine::new(
            cfg_dir.to_string_lossy().into_owned(),
            cache_dir.to_string_lossy().into_owned(),
        )
        .expect("engine should initialize cleanly");

        // Verify legacy home cache removal on init using the engine's isolated cache dir.
        assert!(!engine.paths.cache_dir.join(CACHE_HOME_FILE).exists());
        assert!(!engine.is_signed_in());
        assert!(engine.get_cached_account_profile().await.is_none());
        assert!(engine.get_cached_library_playlists().await.is_empty());

        // Seed profile and playlists cache files directly into engine.paths.cache_dir and confirm sign_out purges them.
        save_cached_json_in(
            &engine.paths.cache_dir,
            CACHE_PROFILE_FILE,
            &AndroidAccountProfile {
                name: "Cached".to_string(),
                email: None,
                handle: None,
                avatar_url: None,
            },
        );
        save_cached_json_in(
            &engine.paths.cache_dir,
            CACHE_PLAYLISTS_FILE,
            &vec![AndroidPlaylist {
                id: "PL1".to_string(),
                name: "Mix".to_string(),
                thumbnail_url: None,
                is_owned: true,
            }],
        );
        assert!(engine.paths.cache_dir.join(CACHE_PROFILE_FILE).exists());
        assert!(engine.paths.cache_dir.join(CACHE_PLAYLISTS_FILE).exists());

        let missing_auth = engine
            .set_ytm_cookie_header("VISITOR_INFO1_LIVE=abc".to_string())
            .await;
        assert!(matches!(
            missing_auth,
            Err(SonataAndroidError::AuthRequired { .. })
        ));

        engine.set_language("tr-TR".to_string()).await;
        assert_eq!(engine.current_language(), "tr-TR");

        engine.sign_out().await;
        assert!(!engine.is_signed_in());
        assert_eq!(engine.current_language(), "tr-TR");
        assert!(!engine.paths.cache_dir.join(CACHE_PROFILE_FILE).exists());
        assert!(!engine.paths.cache_dir.join(CACHE_PLAYLISTS_FILE).exists());

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn maps_core_errors_into_typed_redacted_android_errors_and_builds_android_profile() {
        let dirty =
            "Cookie: SAPISID=secret-sapi; https://rr1.googlevideo.com/videoplayback?sig=secret-sig";
        let rate_err = SonataAndroidError::from(CoreError::rate_limited(
            Some(std::time::Duration::from_secs(45)),
            dirty,
        ));
        assert!(matches!(
            rate_err,
            SonataAndroidError::RateLimited {
                retry_after_secs: Some(45),
                ..
            }
        ));
        let rate_msg = rate_err.to_string();
        assert!(!rate_msg.contains("secret-sapi"));
        assert!(!rate_msg.contains("secret-sig"));

        let auth_err = SonataAndroidError::from(CoreError::auth_required("HTTP 401"));
        assert!(matches!(auth_err, SonataAndroidError::AuthRequired { .. }));
        assert_eq!(auth_err.to_string(), "HTTP 401");

        let offline_err = SonataAndroidError::from(CoreError::offline("dns failure"));
        assert!(matches!(offline_err, SonataAndroidError::Offline { .. }));

        let timeout_err = SonataAndroidError::from(CoreError::timeout("request timeout"));
        assert!(matches!(timeout_err, SonataAndroidError::Timeout { .. }));

        let cancel_err = SonataAndroidError::from(CoreError::cancelled("aborted"));
        assert!(matches!(cancel_err, SonataAndroidError::Cancelled { .. }));

        let not_found_err = SonataAndroidError::from(CoreError::not_found("missing track"));
        assert!(matches!(not_found_err, SonataAndroidError::NotFound { .. }));

        let profile = android_resolver_profile();
        assert_eq!(profile.fast_start, FastStartMode::DirectUrl);
        assert_eq!(
            profile.upgrade_delivery,
            UpgradeDelivery::ReturnRemoteStream
        );
        assert_eq!(
            profile.bootstrap_timeout,
            std::time::Duration::from_millis(8500)
        );
        assert!(!profile.jiosaavn_enabled);
    }

    #[tokio::test]
    async fn server_crud_redaction_and_stream_routing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base_dir = std::env::temp_dir().join(format!(
            "sonata_android_servers_test_{}_{}",
            std::process::id(),
            nanos
        ));
        let engine = SonataAndroidEngine::new(
            base_dir.join("cfg").to_string_lossy().into_owned(),
            base_dir.join("cache").to_string_lossy().into_owned(),
        )
        .expect("engine should initialize");

        assert!(engine.list_servers().await.expect("list empty").is_empty());

        let jf = AndroidServerProfile {
            id: "jf-home".to_string(),
            name: "Home Jellyfin".to_string(),
            kind: AndroidServerKind::Jellyfin,
            base_url: "https://jf.internal/".to_string(),
            enabled: true,
            username: Some("alice".to_string()),
            password: Some("jf-secret-pw".to_string()),
            api_key: Some("jf-secret-key".to_string()),
            access_token: Some("jf-secret-tok".to_string()),
            user_id: Some("uid-1".to_string()),
            bearer_token: None,
            library_ids: vec!["lib-1".to_string()],
            share: None,
            domain: None,
            local_mount_path: None,
            allow_insecure_lan_http: false,
        };
        let dav = AndroidServerProfile {
            id: "".to_string(),
            name: "NAS WebDAV".to_string(),
            kind: AndroidServerKind::WebDav,
            base_url: "https://dav.internal/music/".to_string(),
            enabled: true,
            username: Some("bob".to_string()),
            password: Some("dav-secret-pw".to_string()),
            api_key: None,
            access_token: None,
            user_id: None,
            bearer_token: Some("dav-secret-bearer".to_string()),
            library_ids: vec![],
            share: None,
            domain: None,
            local_mount_path: None,
            allow_insecure_lan_http: false,
        };

        // Credentialed http:// must be rejected by default without allow_insecure_lan_http.
        let mut rejected_http = jf.clone();
        rejected_http.base_url = "http://192.168.1.50:8096".to_string();
        assert!(engine.upsert_server(rejected_http).await.is_err());

        // Public http:// must remain rejected even when allow_insecure_lan_http is true.
        let mut rejected_public = jf.clone();
        rejected_public.base_url = "http://93.184.216.34:8096".to_string();
        rejected_public.allow_insecure_lan_http = true;
        assert!(engine.upsert_server(rejected_public).await.is_err());

        let dbg_jf = format!("{jf:?}");
        assert!(!dbg_jf.contains("jf-secret-pw"));
        assert!(!dbg_jf.contains("jf-secret-key"));
        assert!(!dbg_jf.contains("jf-secret-tok"));

        let auth_dbg = format!(
            "{:?}",
            AndroidJellyfinAuth {
                access_token: "jf-secret-tok".to_string(),
                user_id: "uid-1".to_string(),
                user_name: Some("alice".to_string()),
                server_id: Some("srv-1".to_string()),
            }
        );
        assert!(!auth_dbg.contains("jf-secret-tok"));

        let after_jf = engine.upsert_server(jf).await.expect("upsert jf");
        assert_eq!(after_jf.len(), 1);
        assert_eq!(after_jf[0].base_url, "https://jf.internal");

        let after_dav = engine.upsert_server(dav).await.expect("upsert dav");
        assert_eq!(after_dav.len(), 2);
        let dav_id = after_dav[1].id.clone();
        assert!(dav_id.starts_with("dav-"));

        let toggled = engine
            .set_server_enabled(dav_id.clone(), false)
            .await
            .expect("disable dav");
        assert!(!toggled.iter().find(|s| s.id == dav_id).unwrap().enabled);

        let jf_track = AndroidTrack {
            id: "jellyfin:jf-home:item-99".to_string(),
            title: "Track 1".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(200),
            thumbnail_url: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let resolved = engine
            .resolve_stream(jf_track.clone())
            .await
            .expect("resolve jellyfin stream");
        assert_eq!(
            resolved.url,
            "https://jf.internal/Audio/item-99/stream?static=true"
        );
        assert!(resolved
            .headers
            .iter()
            .any(|h| h.name == "X-Emby-Token" && h.value == "jf-secret-tok"));

        let dav_track = AndroidTrack {
            id: format!("webdav:{dav_id}:/Music/song.flac"),
            title: "Song".to_string(),
            artist: "Bob".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(180),
            thumbnail_url: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let dav_resolved = engine
            .resolve_stream(dav_track)
            .await
            .expect("resolve webdav stream");
        assert_eq!(dav_resolved.url, "https://dav.internal/Music/song.flac");
        assert!(dav_resolved
            .headers
            .iter()
            .any(|h| h.name == "Authorization" && h.value == "Bearer dav-secret-bearer"));

        let server_stream = engine
            .resolve_server_stream(jf_track.clone())
            .await
            .expect("resolve_server_stream");
        assert_eq!(server_stream.url, resolved.url);

        let recovered = engine
            .resolve_recovery(jf_track.clone())
            .await
            .expect("resolve_recovery");
        assert_eq!(recovered.url, resolved.url);
        assert!(engine
            .resolve_hifi_upgrade(jf_track.clone(), resolved)
            .await
            .is_none());
        assert!(engine
            .resolve_explicit_counterpart(jf_track.clone())
            .await
            .is_none());
        assert!(engine
            .resolve_music_video_candidate(jf_track)
            .await
            .is_none());

        let after_del = engine.delete_server(dav_id).await.expect("delete dav");
        assert_eq!(after_del.len(), 1);
        assert_eq!(after_del[0].id, "jf-home");

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[tokio::test]
    async fn smb_health_speed_test_personal_playlists_and_attribution() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base_dir = std::env::temp_dir().join(format!(
            "sonata_android_faz3_test_{}_{}",
            std::process::id(),
            nanos
        ));
        let smb_mount = base_dir.join("smb_mount");
        let album_dir = smb_mount.join("Artist").join("Album");
        fs::create_dir_all(&album_dir).expect("create smb mount");
        let sample_audio = album_dir.join("01 - Lossless.flac");
        let payload = vec![0x66u8; 64 * 1024];
        fs::write(&sample_audio, &payload).expect("write dummy flac");

        let engine = SonataAndroidEngine::new(
            base_dir.join("cfg").to_string_lossy().into_owned(),
            base_dir.join("cache").to_string_lossy().into_owned(),
        )
        .expect("engine should initialize");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral tcp listener");
        let port = listener.local_addr().expect("local addr").port();
        let accept_task = tokio::spawn(async move {
            for _ in 0..3 {
                let _ = listener.accept().await;
            }
        });

        let smb = AndroidServerProfile {
            id: String::new(),
            name: "Home NAS SMB".to_string(),
            kind: AndroidServerKind::Smb,
            base_url: format!("smb://127.0.0.1:{port}/Music"),
            enabled: true,
            username: Some("nasuser".to_string()),
            password: Some("nas-secret".to_string()),
            api_key: None,
            access_token: None,
            user_id: None,
            bearer_token: None,
            library_ids: Vec::new(),
            share: Some("Music".to_string()),
            domain: Some("WORKGROUP".to_string()),
            local_mount_path: Some(smb_mount.to_string_lossy().into_owned()),
            allow_insecure_lan_http: false,
        };

        let servers = engine.upsert_server(smb).await.expect("upsert smb");
        assert_eq!(servers.len(), 1);
        let smb_id = servers[0].id.clone();
        assert!(smb_id.starts_with("smb-"));
        assert_eq!(servers[0].kind, AndroidServerKind::Smb);
        assert_eq!(servers[0].share.as_deref(), Some("Music"));
        assert_eq!(servers[0].domain.as_deref(), Some("WORKGROUP"));

        let health = engine
            .probe_server_health(smb_id.clone())
            .await
            .expect("probe smb health");
        assert!(health.alive, "expected SMB health alive: {health:?}");
        assert!(health.latency_ms.is_some());

        let speed = engine
            .run_server_speed_test(smb_id.clone())
            .await
            .expect("run smb speed test");
        assert!(speed.alive, "expected SMB speed test alive: {speed:?}");
        assert_eq!(speed.bytes_tested, payload.len() as u64);
        assert!(speed.download_mbps.is_some_and(|v| v > 0.0));

        accept_task.abort();

        // Personal playlists CRUD & track routing
        assert!(engine
            .list_personal_playlists()
            .await
            .expect("list empty personal playlists")
            .is_empty());

        let created = engine
            .create_personal_playlist("FLAC Favorites".to_string())
            .await
            .expect("create personal playlist");
        assert!(created.id.starts_with("personal:"));
        assert_eq!(created.name, "FLAC Favorites");

        let renamed = engine
            .rename_personal_playlist(created.id.clone(), "Lossless Rotation".to_string())
            .await
            .expect("rename personal playlist");
        assert_eq!(renamed.name, "Lossless Rotation");

        let local_track = AndroidTrack {
            id: "local:42".to_string(),
            title: "Midnight Groove".to_string(),
            artist: "Sonata Quartet".to_string(),
            album: "Sessions".to_string(),
            duration_secs: Some(215),
            thumbnail_url: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let with_track = engine
            .add_track_to_personal_playlist(created.id.clone(), local_track.clone())
            .await
            .expect("add track to personal playlist");
        assert_eq!(with_track.tracks.len(), 1);
        assert_eq!(with_track.tracks[0].id, "local:42");

        let after_remove = engine
            .remove_track_from_personal_playlist(created.id.clone(), "local:42".to_string())
            .await
            .expect("remove track from personal playlist");
        assert!(after_remove.tracks.is_empty());

        assert!(engine
            .delete_personal_playlist(created.id.clone())
            .await
            .expect("delete personal playlist"));
        assert!(engine
            .list_personal_playlists()
            .await
            .expect("list after delete")
            .is_empty());

        // Unauthenticated YTM attribution returns Ok(None) cleanly without network calls.
        let attributed = engine
            .attribute_personal_track_to_ytm(local_track)
            .await
            .expect("attribute_personal_track_to_ytm");
        assert!(attributed.is_none());

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[tokio::test]
    async fn scrobble_config_redaction_and_listening_stats_windows() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base_dir = std::env::temp_dir().join(format!(
            "sonata_android_faz4_test_{}_{}",
            std::process::id(),
            nanos
        ));
        let engine = SonataAndroidEngine::new(
            base_dir.join("cfg").to_string_lossy().into_owned(),
            base_dir.join("cache").to_string_lossy().into_owned(),
        )
        .expect("engine should initialize");

        // Default scrobble config is disabled/empty.
        let initial_cfg = engine
            .get_scrobble_config()
            .await
            .expect("get default scrobble config");
        assert!(!initial_cfg.lastfm.enabled);
        assert!(!initial_cfg.listenbrainz.enabled);

        let scrobble_cfg = AndroidScrobbleConfig {
            lastfm: AndroidLastFmConfig {
                enabled: true,
                api_key: "secret-lfm-key".to_string(),
                api_secret: "secret-lfm-sec".to_string(),
                session_key: "secret-lfm-sk".to_string(),
                username: "alice".to_string(),
            },
            listenbrainz: AndroidListenBrainzConfig {
                enabled: true,
                token: "secret-lb-tok".to_string(),
                username: "alice_lb".to_string(),
            },
        };
        let dbg = format!("{scrobble_cfg:?}");
        assert!(!dbg.contains("secret-lfm-key"));
        assert!(!dbg.contains("secret-lfm-sec"));
        assert!(!dbg.contains("secret-lfm-sk"));
        assert!(!dbg.contains("secret-lb-tok"));
        assert!(dbg.contains("alice"));
        assert!(dbg.contains("[redacted]"));

        let saved_cfg = engine
            .save_scrobble_config(scrobble_cfg.clone())
            .await
            .expect("save scrobble config");
        assert_eq!(saved_cfg, scrobble_cfg);
        assert_eq!(
            engine.get_scrobble_config().await.expect("reload scrobble"),
            scrobble_cfg
        );

        // Unconfigured scrobble calls are no-ops without network.
        let track = AndroidTrack {
            id: "yt_faz4_1".to_string(),
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: "Hurry Up".to_string(),
            duration_secs: Some(240),
            thumbnail_url: None,
            liked: true,
            album_id: None,
            is_explicit: false,
        };
        engine
            .save_scrobble_config(AndroidScrobbleConfig::default())
            .await
            .expect("reset scrobble config");
        engine
            .scrobble_now_playing(track.clone())
            .await
            .expect("unconfigured now playing succeeds");
        engine
            .scrobble_submit(track.clone(), Some(1_700_000_000))
            .await
            .expect("unconfigured submit succeeds");

        // Listening stats record + window queries.
        let now = SonataAndroidEngine::current_unix_secs();
        let day = 86_400u64;
        engine
            .record_listening_stat(track.clone(), String::new(), 240, Some(now - 60))
            .await
            .expect("record ytm stat");
        engine
            .record_listening_stat(
                AndroidTrack {
                    id: "local:9".to_string(),
                    title: "Local Echo".to_string(),
                    artist: "Studio Artist".to_string(),
                    album: "Sessions".to_string(),
                    duration_secs: Some(180),
                    thumbnail_url: None,
                    liked: false,
                    album_id: None,
                    is_explicit: false,
                },
                String::new(),
                180,
                Some(now - 20 * day),
            )
            .await
            .expect("record local stat");

        let week = engine
            .query_listening_stats(AndroidStatsWindow::Week)
            .await
            .expect("query week");
        assert_eq!(week.total_plays, 1);
        assert_eq!(week.total_minutes, 4);
        assert_eq!(week.unique_tracks, 1);
        assert_eq!(week.source_breakdown[0].source_tag, "YTM");

        let month = engine
            .query_listening_stats(AndroidStatsWindow::Month)
            .await
            .expect("query month");
        assert_eq!(month.total_plays, 2);
        assert_eq!(month.total_minutes, 7);
        assert_eq!(month.unique_artists, 2);
        assert!(month
            .source_breakdown
            .iter()
            .any(|s| s.source_tag == "LOC" && s.plays == 1));

        let all = engine
            .query_listening_stats(AndroidStatsWindow::AllTime)
            .await
            .expect("query all time");
        assert_eq!(all.total_plays, 2);

        let _ = fs::remove_dir_all(&base_dir);
    }
}
