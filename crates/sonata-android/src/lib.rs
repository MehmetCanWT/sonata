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
use sonata_core::providers::hifi::{
    FastStartMode, JsCipherBridge, ResolvedStream, ResolverProfile, SolvedCipherPair,
    StreamFallbackMode, StreamResolver, UpgradeDelivery,
};
use sonata_core::providers::jiosaavn::{self, ResolvedJioSaavnStream};
use sonata_core::providers::ytmusic::{
    ArtistPage, HomeCard, HomeCardKind, HomeShelf, YtAccountProfile, YtMusicClient,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

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
}

impl From<&Playlist> for AndroidPlaylist {
    fn from(p: &Playlist) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            thumbnail_url: p.thumbnail_url.clone(),
        }
    }
}

impl From<Playlist> for AndroidPlaylist {
    fn from(p: Playlist) -> Self {
        Self {
            id: p.id,
            name: p.name,
            thumbnail_url: p.thumbnail_url,
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
    session: RwLock<AuthSession>,
}

impl SonataAndroidEngine {
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

    fn build_session(
        http: &reqwest::Client,
        paths: &CorePaths,
        cache_db: Option<CacheDb>,
        cookies: ExtractedCookies,
        cipher_bridge: Option<Arc<dyn JsCipherBridge>>,
    ) -> AuthSession {
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
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
        let session = Self::build_session(&http, &paths, cache_db.clone(), cookies, None);

        Ok(Self {
            http,
            paths,
            cache_db,
            cipher_bridge: RwLock::new(None),
            session: RwLock::new(session),
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
        let next_session = Self::build_session(
            &self.http,
            &self.paths,
            self.cache_db.clone(),
            cookies,
            bridge,
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
        let next_session = Self::build_session(
            &self.http,
            &self.paths,
            self.cache_db.clone(),
            ExtractedCookies::default(),
            bridge,
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

    /// Fetches a full YouTube Music artist page (hero, subscribers, top tracks, releases, similar artists).
    pub async fn fetch_artist_page(
        &self,
        browse_id: String,
    ) -> Result<AndroidArtistPage, SonataAndroidError> {
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

    /// Finds an explicit counterpart track when `track` is unbadged or clean/edited.
    pub async fn resolve_explicit_counterpart(&self, track: AndroidTrack) -> Option<AndroidTrack> {
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

    /// Fetches synced or plain lyrics lines for the active track.
    pub async fn fetch_lyrics(&self, track: AndroidTrack) -> Vec<AndroidLyricLine> {
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
        };
        let android_pl = AndroidPlaylist::from(&playlist);
        assert_eq!(android_pl.id, "PL_42");
        assert_eq!(android_pl.name, "Night Mix");
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

        engine.sign_out().await;
        assert!(!engine.is_signed_in());
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
}
