//! UniFFI Android bridge exposing `sonata-core` to Kotlin and Jetpack Compose.
//!
//! 1. **Engine Lifecycle**: `SonataAndroidEngine` owns a multi-threaded Tokio runtime, HTTP client,
//!    SQLite `CacheDb`, `YtMusicClient`, and `StreamResolver`, initialized with Android's private directories
//!    and any persisted YouTube Music WebView session cookies.
//! 2. **FFI Models**: `AndroidTrack`, `AndroidPlaylist`, `AndroidAccountProfile`, `AndroidHomeShelf`,
//!    `AndroidHomeCard`, `AndroidLyricLine`, and `AndroidResolvedStream` mirror `sonata-core` types.
//! 3. **Auth & Async Operations**: `set_ytm_cookie_header`, `sign_out`, `fetch_home_feed`, `fetch_library_playlists`,
//!    `search_songs`, `resolve_stream`, `resolve_hifi_upgrade`, `fetch_lyrics`, and disk-persisted startup
//!    cache readers (`get_cached_account_profile`, `get_cached_library_playlists`). Home shelves are always
//!    fetched live and never persisted to disk.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::CacheDb;
use sonata_core::config::AppConfig;
use sonata_core::lyrics::{lrclib, LyricsState};
use sonata_core::providers::hifi::{
    JsCipherBridge, ResolvedStream, SolvedCipherPair, StreamResolver,
};
use sonata_core::providers::ytmusic::{
    HomeCard, HomeCardKind, HomeShelf, YtAccountProfile, YtMusicClient,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::runtime::{Builder, Runtime};

const CACHE_PROFILE_FILE: &str = "android_profile.json";
const CACHE_HOME_FILE: &str = "android_home.json";
const CACHE_PLAYLISTS_FILE: &str = "android_playlists.json";

fn save_cached_json<T: Serialize>(file_name: &str, value: &T) {
    save_cached_json_in(&AppConfig::cache_dir(), file_name, value);
}

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

fn load_cached_json<T: DeserializeOwned>(file_name: &str) -> Option<T> {
    load_cached_json_from(&AppConfig::cache_dir(), file_name)
}

fn load_cached_json_from<T: DeserializeOwned>(dir: &Path, file_name: &str) -> Option<T> {
    let path = dir.join(file_name);
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn remove_cached_json(file_name: &str) {
    remove_cached_json_from(&AppConfig::cache_dir(), file_name);
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
}

impl From<anyhow::Error> for SonataAndroidError {
    fn from(err: anyhow::Error) -> Self {
        Self::Engine {
            reason: err.to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct AndroidTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub liked: bool,
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
        let mut guard = self
            .loaded_hash
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.as_deref() == Some(player_hash) {
            return true;
        }
        let ok = self
            .solver
            .ensure_loaded(player_hash.to_string(), hooked_script.to_string());
        if ok {
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
    rt: Arc<Runtime>,
    http: reqwest::Client,
    cache_db: Option<CacheDb>,
    cipher_bridge: RwLock<Option<Arc<dyn JsCipherBridge>>>,
    session: RwLock<AuthSession>,
}

impl SonataAndroidEngine {
    fn current_ytm(&self) -> YtMusicClient {
        self.session.read().expect("session lock").ytm.clone()
    }

    fn current_resolver(&self) -> StreamResolver {
        self.session.read().expect("session lock").resolver.clone()
    }

    fn build_session(
        http: &reqwest::Client,
        cache_db: Option<CacheDb>,
        cookies: ExtractedCookies,
        cipher_bridge: Option<Arc<dyn JsCipherBridge>>,
    ) -> AuthSession {
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let resolver = StreamResolver::new(http.clone(), ytm.clone(), cookies.clone(), cache_db);
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

#[uniffi::export]
impl SonataAndroidEngine {
    /// Creates a new Sonata Android engine rooted at the app's private `filesDir` and `cacheDir`.
    #[uniffi::constructor]
    pub fn new(config_dir: String, cache_dir: String) -> Result<Self, SonataAndroidError> {
        AppConfig::set_android_dirs(PathBuf::from(config_dir), PathBuf::from(cache_dir));
        let rt = Builder::new_multi_thread()
            .enable_all()
            .worker_threads(4)
            .build()
            .map_err(|e| SonataAndroidError::Engine {
                reason: format!("Failed to start Tokio runtime: {e}"),
            })?;

        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(6))
            .timeout(std::time::Duration::from_secs(20))
            .tcp_nodelay(true)
            .pool_max_idle_per_host(8)
            .build()
            .map_err(|e| SonataAndroidError::Engine {
                reason: format!("Failed to initialize HTTP client: {e}"),
            })?;

        // Home shelves are always fetched live; purge any legacy home cache file from prior builds.
        remove_cached_json(CACHE_HOME_FILE);

        let cache_db = CacheDb::open_default();
        let cookies = ExtractedCookies::load_saved_ytm_header().unwrap_or_default();
        let session = Self::build_session(&http, cache_db.clone(), cookies, None);

        Ok(Self {
            rt: Arc::new(rt),
            http,
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
        if let Ok(mut guard) = self.cipher_bridge.write() {
            *guard = Some(Arc::clone(&bridge));
        }
        self.current_resolver().set_js_cipher_bridge(bridge);
    }

    /// Pre-fetches, hooks, caches, and loads YouTube's `base.js` into the registered Android cipher solver.
    pub fn warm_cipher_solver(&self) -> bool {
        let resolver = self.current_resolver();
        self.rt
            .block_on(async move { resolver.ensure_prepared_player_js().await })
            .is_some()
    }

    /// Returns whether valid YouTube Music session cookies are loaded.
    pub fn is_signed_in(&self) -> bool {
        self.session
            .read()
            .expect("session lock")
            .cookies
            .has_ytm_auth()
    }

    /// Reads the cached YouTube Music account profile from disk for instant startup display.
    pub fn get_cached_account_profile(&self) -> Option<AndroidAccountProfile> {
        if !self.is_signed_in() {
            return None;
        }
        load_cached_json(CACHE_PROFILE_FILE)
    }

    /// Reads the cached library playlists from disk for instant startup display.
    pub fn get_cached_library_playlists(&self) -> Vec<AndroidPlaylist> {
        if !self.is_signed_in() {
            return Vec::new();
        }
        load_cached_json(CACHE_PLAYLISTS_FILE).unwrap_or_default()
    }

    /// Installs YouTube Music WebView cookies, persists them privately, and returns the account profile.
    pub fn set_ytm_cookie_header(
        &self,
        cookie_header: String,
    ) -> Result<Option<AndroidAccountProfile>, SonataAndroidError> {
        let cookies = ExtractedCookies::from_ytm_cookie_header(&cookie_header);
        if !cookies.has_ytm_auth() {
            return Err(SonataAndroidError::Engine {
                reason: "Signed-in YouTube Music cookies (SAPISID) were not found".to_string(),
            });
        }
        let _ = cookies.save_ytm_header();
        let bridge = self.cipher_bridge.read().ok().and_then(|g| g.clone());
        let next_session = Self::build_session(&self.http, self.cache_db.clone(), cookies, bridge);
        let ytm = next_session.ytm.clone();
        *self.session.write().expect("session lock") = next_session;
        let profile = self
            .rt
            .block_on(async move { ytm.fetch_account_profile().await })
            .ok()
            .map(AndroidAccountProfile::from);
        if let Some(ref prof) = profile {
            save_cached_json(CACHE_PROFILE_FILE, prof);
        }
        Ok(profile)
    }

    /// Fetches the signed-in YouTube Music account profile, if authenticated.
    pub fn get_account_profile(&self) -> Option<AndroidAccountProfile> {
        if !self.is_signed_in() {
            return None;
        }
        let ytm = self.current_ytm();
        let profile = self
            .rt
            .block_on(async move { ytm.fetch_account_profile().await })
            .ok()
            .map(AndroidAccountProfile::from);
        if let Some(ref prof) = profile {
            save_cached_json(CACHE_PROFILE_FILE, prof);
        }
        profile
    }

    /// Clears saved YouTube Music session cookies and resets to anonymous discovery.
    pub fn sign_out(&self) {
        ExtractedCookies::clear_saved_ytm_header();
        remove_cached_json(CACHE_PROFILE_FILE);
        remove_cached_json(CACHE_HOME_FILE);
        remove_cached_json(CACHE_PLAYLISTS_FILE);
        let bridge = self.cipher_bridge.read().ok().and_then(|g| g.clone());
        let next_session = Self::build_session(
            &self.http,
            self.cache_db.clone(),
            ExtractedCookies::default(),
            bridge,
        );
        *self.session.write().expect("session lock") = next_session;
    }

    /// Fetches the signed-in user's YouTube Music library playlists.
    pub fn fetch_library_playlists(&self) -> Result<Vec<AndroidPlaylist>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let playlists = self
            .rt
            .block_on(async move { ytm.fetch_library_playlists().await })?;
        let out: Vec<AndroidPlaylist> = playlists.iter().map(AndroidPlaylist::from).collect();
        save_cached_json(CACHE_PLAYLISTS_FILE, &out);
        Ok(out)
    }

    /// Fetches fresh YouTube Music home/discovery shelves live without persisting to disk.
    pub fn fetch_home_feed(&self) -> Result<Vec<AndroidHomeShelf>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let shelves = self
            .rt
            .block_on(async move { ytm.fetch_home_feed().await })?;
        Ok(shelves.iter().map(AndroidHomeShelf::from).collect())
    }

    /// Searches YouTube Music songs for the given query.
    pub fn search_songs(&self, query: String) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = self
            .rt
            .block_on(async move { ytm.search_songs(&query).await })?;
        Ok(tracks.iter().map(AndroidTrack::from).collect())
    }

    /// Fetches all tracks for a YouTube Music playlist, album, or mix browse ID.
    pub fn fetch_collection_tracks(
        &self,
        browse_id: String,
    ) -> Result<Vec<AndroidTrack>, SonataAndroidError> {
        let ytm = self.current_ytm();
        let tracks = self.rt.block_on(async move {
            let mut out = Vec::new();
            ytm.fetch_playlist_tracks_progressive(&browse_id, |batch, _| {
                out.extend(batch);
            })
            .await?;
            Ok::<_, anyhow::Error>(out)
        })?;
        Ok(tracks.iter().map(AndroidTrack::from).collect())
    }

    /// Resolves a fast-start audio stream URL and HTTP headers for Media3 `ExoPlayer`.
    pub fn resolve_stream(
        &self,
        track: AndroidTrack,
    ) -> Result<AndroidResolvedStream, SonataAndroidError> {
        let resolver = self.current_resolver();
        let core_track: Track = track.into();
        let stream = self
            .rt
            .block_on(async move { resolver.resolve_fast(&core_track).await })?;
        Ok(stream.into())
    }

    /// Attempts a background Hi-Fi stream upgrade and local cache download for an active track.
    pub fn resolve_hifi_upgrade(
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
        self.rt
            .block_on(async move {
                resolver
                    .resolve_hifi_upgrade(&core_track, &core_stream, true)
                    .await
            })
            .map(AndroidResolvedStream::from)
    }

    /// Fetches synced or plain lyrics lines for the active track.
    pub fn fetch_lyrics(&self, track: AndroidTrack) -> Vec<AndroidLyricLine> {
        let http = self.http.clone();
        let ytm = self.current_ytm();
        let cache = self.cache_db.clone();
        let core_track: Track = track.into();
        let state = self.rt.block_on(async move {
            lrclib::fetch_lyrics(&http, &ytm, &core_track, cache.as_ref()).await
        });
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
    pub fn clear_lyrics_cache(&self) {
        if let Some(ref db) = self.cache_db {
            db.clear_lyrics_cache();
        } else {
            let _ = std::fs::remove_dir_all(AppConfig::lyrics_cache_dir());
        }
    }

    /// Checks GitHub Releases for a newer Sonata Android APK release.
    pub fn check_for_updates(&self) -> Result<AndroidUpdateInfo, SonataAndroidError> {
        let http = self.http.clone();
        let res = self.rt.block_on(async move {
            sonata_core::updater::check_for_updates_for(
                &http,
                sonata_core::updater::UpdateTarget::Android,
                sonata_core::updater::CURRENT_VERSION,
            )
            .await
        })?;
        Ok(res.into())
    }

    /// Checks GitHub Releases for a newer Sonata Android APK release against an explicit installed versionName.
    pub fn check_for_updates_for_version(
        &self,
        current_version: String,
    ) -> Result<AndroidUpdateInfo, SonataAndroidError> {
        let http = self.http.clone();
        let res = self.rt.block_on(async move {
            sonata_core::updater::check_for_updates_for(
                &http,
                sonata_core::updater::UpdateTarget::Android,
                &current_version,
            )
            .await
        })?;
        Ok(res.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
