//! Portable self-hosted music server configuration, shared browse models, and unified client.
//!
//! 1. **Configuration (`ServerProfile`, `ServerKind`)**: Unified server profile persistence in
//!    `paths.servers_config_path()` using `0o700` directories and atomic `0o600` temp-file writes.
//! 2. **Item Addressing (`ServerItemRef`)**: Prefixed ID encoding (`jellyfin:`, `subsonic:`,
//!    `webdav:`, `smb:`) mapping self-hosted items into Sonata's `Track`, `Playlist`, `HomeCard`,
//!    and `AlbumDetail` models without modifying `ProviderSource`.
//! 3. **Protocol Clients (`jellyfin`, `subsonic`, `webdav`, `smb`)**: Unified `ServerClient`
//!    dispatcher for ping, health/speed diagnostics, libraries, artists, albums, playlists,
//!    search, and direct stream URLs/headers.

pub mod jellyfin;
pub mod smb;
pub mod subsonic;
pub mod webdav;

use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::paths::CorePaths;
use crate::providers::ytmusic::{AlbumDetail, HomeCard};
use crate::providers::{Playlist, Track};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const JELLYFIN_ID_PREFIX: &str = "jellyfin:";
pub const SUBSONIC_ID_PREFIX: &str = "subsonic:";
pub const WEBDAV_ID_PREFIX: &str = "webdav:";
pub const SMB_ID_PREFIX: &str = "smb:";

const CONFIG_SCHEMA_VERSION: u32 = 1;
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(12);
const SPEED_TEST_MAX_BYTES: usize = 1024 * 1024;
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Supported self-hosted music server protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    Jellyfin,
    Subsonic,
    WebDav,
    Smb,
}

impl ServerKind {
    /// Returns the 3-letter badge tag shown alongside self-hosted tracks (`JFN`, `SUB`, `DAV`, `SMB`).
    pub const fn short_tag(self) -> &'static str {
        match self {
            Self::Jellyfin => "JFN",
            Self::Subsonic => "SUB",
            Self::WebDav => "DAV",
            Self::Smb => "SMB",
        }
    }

    /// Returns the human-readable protocol name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Jellyfin => "Jellyfin",
            Self::Subsonic => "Subsonic",
            Self::WebDav => "WebDAV",
            Self::Smb => "SMB",
        }
    }

    /// Returns the canonical ID prefix (`jellyfin:`, `subsonic:`, `webdav:`, or `smb:`).
    pub const fn id_prefix(self) -> &'static str {
        match self {
            Self::Jellyfin => JELLYFIN_ID_PREFIX,
            Self::Subsonic => SUBSONIC_ID_PREFIX,
            Self::WebDav => WEBDAV_ID_PREFIX,
            Self::Smb => SMB_ID_PREFIX,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Persistent connection and credential profile for one self-hosted music server.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerProfile {
    pub id: String,
    pub name: String,
    pub kind: ServerKind,
    pub base_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default)]
    pub library_ids: Vec<String>,
    #[serde(default)]
    pub share: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub local_mount_path: Option<PathBuf>,
}

impl fmt::Debug for ServerProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redact = |opt: &Option<String>| -> &'static str {
            if opt.as_deref().is_some_and(|v| !v.is_empty()) {
                "[redacted]"
            } else {
                "<none>"
            }
        };
        f.debug_struct("ServerProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("base_url", &crate::redaction::stream_url(&self.base_url))
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
            .finish()
    }
}

impl ServerProfile {
    /// Creates a new enabled server profile with empty optional credentials.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ServerKind,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            base_url: base_url.into(),
            enabled: true,
            username: None,
            password: None,
            api_key: None,
            access_token: None,
            user_id: None,
            bearer_token: None,
            library_ids: Vec::new(),
            share: None,
            domain: None,
            local_mount_path: None,
        }
    }

    /// Returns `base_url` trimmed of surrounding whitespace and trailing slashes.
    pub fn normalized_base_url(&self) -> String {
        self.base_url.trim().trim_end_matches('/').to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedServersFile {
    #[serde(default = "default_config_version")]
    version: u32,
    #[serde(default)]
    servers: Vec<ServerProfile>,
}

fn default_config_version() -> u32 {
    CONFIG_SCHEMA_VERSION
}

/// Loads persisted server profiles from `paths.servers_config_path()`.
/// Returns an empty list if the configuration file does not yet exist.
pub fn load_server_profiles(paths: &CorePaths) -> Result<Vec<ServerProfile>, CoreError> {
    let path = paths.servers_config_path();
    load_server_profiles_from_path(&path)
}

/// Loads persisted server profiles from an explicit file path.
pub fn load_server_profiles_from_path(path: &Path) -> Result<Vec<ServerProfile>, CoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|err| {
        CoreError::io(format!(
            "failed to read servers config {}: {err}",
            path.display()
        ))
    })?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parsed: PersistedServersFile = serde_json::from_str(&raw).map_err(|err| {
        CoreError::invalid_response("servers", format!("malformed servers config JSON: {err}"))
    })?;
    Ok(parsed.servers)
}

/// Persists server profiles atomically into `paths.servers_config_path()` with `0o700` directory
/// and `0o600` file permissions on Unix.
pub fn save_server_profiles(
    paths: &CorePaths,
    profiles: &[ServerProfile],
) -> Result<(), CoreError> {
    let path = paths.servers_config_path();
    save_server_profiles_to_path(&path, profiles)
}

/// Persists server profiles atomically into `path` using a `0o600` temporary sibling file.
pub fn save_server_profiles_to_path(
    path: &Path,
    profiles: &[ServerProfile],
) -> Result<(), CoreError> {
    let parent = path.parent().ok_or_else(|| {
        CoreError::io(format!(
            "servers config path {} has no parent directory",
            path.display()
        ))
    })?;
    ensure_private_dir(parent)
        .map_err(|err| CoreError::io(format!("failed to create {}: {err}", parent.display())))?;

    let payload = PersistedServersFile {
        version: CONFIG_SCHEMA_VERSION,
        servers: profiles.to_vec(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| CoreError::internal(format!("failed to serialize servers config: {err}")))?;

    let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("servers.json");
    let tmp_path = parent.join(format!(".{file_name}.{}.{seq}.tmp", std::process::id()));

    let write_res = (|| -> std::io::Result<()> {
        let mut file = create_private_file(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(CoreError::io(format!(
            "failed to persist servers config {}: {err}",
            path.display()
        )));
    }
    Ok(())
}

/// Parsed reference to an item hosted on a configured self-hosted server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerItemRef {
    pub kind: ServerKind,
    pub server_id: String,
    pub item_id: String,
}

/// Encodes a server-scoped item ID (`<prefix><encoded_server_id>:<item_id>`).
pub fn encode_server_item_id(kind: ServerKind, server_id: &str, item_id: &str) -> String {
    let encoded_server = if server_id.contains(':') || server_id.contains('%') {
        urlencoding::encode(server_id).into_owned()
    } else {
        server_id.to_string()
    };
    format!("{}{encoded_server}:{item_id}", kind.id_prefix())
}

/// Parses a prefixed self-hosted server item ID back into its protocol, server ID, and raw item ID.
pub fn parse_server_item_id(encoded: &str) -> Option<ServerItemRef> {
    let (kind, rest) = if let Some(rest) = encoded.strip_prefix(JELLYFIN_ID_PREFIX) {
        (ServerKind::Jellyfin, rest)
    } else if let Some(rest) = encoded.strip_prefix(SUBSONIC_ID_PREFIX) {
        (ServerKind::Subsonic, rest)
    } else if let Some(rest) = encoded.strip_prefix(SMB_ID_PREFIX) {
        (ServerKind::Smb, rest)
    } else {
        let rest = encoded.strip_prefix(WEBDAV_ID_PREFIX)?;
        (ServerKind::WebDav, rest)
    };

    let (raw_server_id, item_id) = rest.split_once(':')?;
    if raw_server_id.is_empty() || item_id.is_empty() {
        return None;
    }
    let server_id = urlencoding::decode(raw_server_id)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| raw_server_id.to_string());
    Some(ServerItemRef {
        kind,
        server_id,
        item_id: item_id.to_string(),
    })
}

/// Direct playback URL and HTTP headers resolved for a self-hosted track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStream {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub mime_type: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<String>,
    pub audio_badge: Option<String>,
}

/// Music library or folder collection advertised by a self-hosted server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerLibrary {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// Unified search results returned by a self-hosted server.
#[derive(Debug, Clone, Default)]
pub struct ServerSearchResult {
    pub tracks: Vec<Track>,
    pub albums: Vec<HomeCard>,
    pub artists: Vec<HomeCard>,
    pub playlists: Vec<HomeCard>,
}

/// Reachability and latency report returned by [`ServerClient::probe_server_health`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHealthReport {
    pub alive: bool,
    pub latency_ms: Option<u64>,
    pub message: String,
}

/// Reachability, latency, and download throughput report returned by [`ServerClient::run_speed_test`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerSpeedTestReport {
    pub alive: bool,
    pub latency_ms: u64,
    pub download_mbps: Option<f64>,
    pub bytes_tested: u64,
    pub sample_note: String,
}

/// Computes download throughput in megabits per second (`Mbps`) from tested bytes and duration.
pub fn compute_download_mbps(bytes_tested: u64, elapsed: Duration) -> Option<f64> {
    if bytes_tested == 0 {
        return None;
    }
    let secs = elapsed.as_secs_f64();
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    let bits = (bytes_tested as f64) * 8.0;
    let mbps = bits / (secs * 1_000_000.0);
    mbps.is_finite().then_some(mbps)
}

/// Maps HTTP error status codes from self-hosted servers into redacted `CoreError` variants.
pub(crate) fn map_http_status(provider: &str, status: StatusCode, body: &str) -> CoreError {
    let snippet = body.trim().chars().take(240).collect::<String>();
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => CoreError::auth_required(format!(
            "{provider} returned HTTP {}: {snippet}",
            status.as_u16()
        )),
        StatusCode::NOT_FOUND => {
            CoreError::not_found(format!("{provider} returned HTTP 404: {snippet}"))
        }
        StatusCode::TOO_MANY_REQUESTS => {
            CoreError::rate_limited(None, format!("{provider} returned HTTP 429: {snippet}"))
        }
        _ => CoreError::invalid_response(
            provider,
            format!("{provider} returned HTTP {}: {snippet}", status.as_u16()),
        ),
    }
}

/// Formats a normalized audio badge string without server protocol names (e.g. `"FLAC · 980 kbps"` or `"FLAC"`).
pub(crate) fn format_audio_badge(
    codec: Option<&str>,
    bitrate_kbps: Option<u32>,
    _fallback_label: &str,
) -> Option<String> {
    let clean_codec = codec
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_uppercase);
    match (clean_codec, bitrate_kbps.filter(|&b| b > 0)) {
        (Some(c), Some(kbps)) => Some(format!("{c} · {kbps} kbps")),
        (Some(c), None) => Some(c),
        (None, Some(kbps)) => Some(format!("{kbps} kbps")),
        (None, None) => None,
    }
}

pub(crate) fn default_http_timeout() -> Duration {
    DEFAULT_HTTP_TIMEOUT
}

/// Unified async facade over Jellyfin, Subsonic, WebDAV, and SMB self-hosted servers.
#[derive(Clone)]
pub struct ServerClient {
    http: Client,
    cache_dir: Option<PathBuf>,
}

impl ServerClient {
    pub fn new(http: Client) -> Self {
        Self {
            http,
            cache_dir: None,
        }
    }

    pub fn with_cache_dir(mut self, cache_dir: PathBuf) -> Self {
        self.cache_dir = Some(cache_dir);
        self
    }

    pub async fn ping(&self, profile: &ServerProfile) -> Result<String, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::ping(&self.http, profile).await,
            ServerKind::Subsonic => subsonic::ping(&self.http, profile).await,
            ServerKind::WebDav => {
                let listing = webdav::browse_folder(&self.http, profile, None).await?;
                Ok(format!(
                    "WebDAV OK ({} folders, {} tracks)",
                    listing.folders.len(),
                    listing.tracks.len()
                ))
            }
            ServerKind::Smb => smb::ping(profile).await,
        }
    }

    /// Probes server reachability and measures round-trip latency in milliseconds.
    pub async fn probe_server_health(&self, profile: &ServerProfile) -> ServerHealthReport {
        let start = Instant::now();
        match self.ping(profile).await {
            Ok(message) => ServerHealthReport {
                alive: true,
                latency_ms: Some(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)),
                message,
            },
            Err(err) => ServerHealthReport {
                alive: false,
                latency_ms: None,
                message: err.to_string(),
            },
        }
    }

    /// Measures server ping latency and real download throughput (`Mbps`) across all server kinds.
    pub async fn run_speed_test(&self, profile: &ServerProfile) -> ServerSpeedTestReport {
        let ping_start = Instant::now();
        let ping_res = self.ping(profile).await;
        let latency_ms = u64::try_from(ping_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        if let Err(err) = ping_res {
            return ServerSpeedTestReport {
                alive: false,
                latency_ms,
                download_mbps: None,
                bytes_tested: 0,
                sample_note: err.to_string(),
            };
        }

        if profile.kind == ServerKind::Smb {
            let prof = profile.clone();
            let smb_res = tokio::task::spawn_blocking(move || {
                smb::read_speed_test_sample(&prof, SPEED_TEST_MAX_BYTES)
            })
            .await;
            return match smb_res {
                Ok(Ok((bytes_tested, elapsed, sample_note))) => ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: compute_download_mbps(bytes_tested, elapsed),
                    bytes_tested,
                    sample_note,
                },
                Ok(Err(err)) => ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: None,
                    bytes_tested: 0,
                    sample_note: err.to_string(),
                },
                Err(err) => ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: None,
                    bytes_tested: 0,
                    sample_note: format!("SMB benchmark task failed: {err}"),
                },
            };
        }

        let sample_track = match self.find_sample_track(profile).await {
            Ok(Some(track)) => track,
            Ok(None) => {
                return ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: None,
                    bytes_tested: 0,
                    sample_note: "Server reachable; no sample track available for throughput test"
                        .to_string(),
                };
            }
            Err(err) => {
                return ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: None,
                    bytes_tested: 0,
                    sample_note: format!("Server reachable; failed to locate sample track: {err}"),
                };
            }
        };

        let stream = match self.resolve_stream(profile, &sample_track) {
            Ok(s) => s,
            Err(err) => {
                return ServerSpeedTestReport {
                    alive: true,
                    latency_ms,
                    download_mbps: None,
                    bytes_tested: 0,
                    sample_note: format!("Server reachable; stream build failed: {err}"),
                };
            }
        };

        match self.benchmark_http_stream(&stream).await {
            Ok((bytes_tested, elapsed)) => ServerSpeedTestReport {
                alive: true,
                latency_ms,
                download_mbps: compute_download_mbps(bytes_tested, elapsed),
                bytes_tested,
                sample_note: format!(
                    "Downloaded {bytes_tested} bytes from \"{}\" in {}ms",
                    sample_track.title,
                    elapsed.as_millis()
                ),
            },
            Err(err) => ServerSpeedTestReport {
                alive: true,
                latency_ms,
                download_mbps: None,
                bytes_tested: 0,
                sample_note: format!("Server reachable; sample download failed: {err}"),
            },
        }
    }

    async fn find_sample_track(&self, profile: &ServerProfile) -> Result<Option<Track>, CoreError> {
        if profile.kind == ServerKind::WebDav {
            let listing = webdav::browse_folder(&self.http, profile, None).await?;
            if let Some(first) = listing.tracks.into_iter().next() {
                return Ok(Some(first));
            }
            let crawled = webdav::crawl_audio(&self.http, profile, None, Some(2), Some(1)).await?;
            return Ok(crawled.into_iter().next());
        }

        let albums = self.list_albums(profile, None, None).await?;
        for album in albums.into_iter().take(3) {
            if let Some(track) = album.track {
                return Ok(Some(track));
            }
            if let Ok(detail) = self.get_album(profile, &album.id).await {
                if let Some(track) = detail.tracks.into_iter().next() {
                    return Ok(Some(track));
                }
            }
        }

        let playlists = self.list_playlists(profile).await?;
        for pl in playlists.into_iter().take(2) {
            if let Ok(tracks) = self.get_playlist_tracks(profile, &pl.id).await {
                if let Some(track) = tracks.into_iter().next() {
                    return Ok(Some(track));
                }
            }
        }

        Ok(None)
    }

    async fn benchmark_http_stream(
        &self,
        stream: &ServerStream,
    ) -> Result<(u64, Duration), CoreError> {
        let mut req = self
            .http
            .get(&stream.url)
            .timeout(default_http_timeout())
            .header(
                reqwest::header::RANGE,
                format!("bytes=0-{}", SPEED_TEST_MAX_BYTES - 1),
            );
        for (k, v) in &stream.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let start = Instant::now();
        let mut resp = req
            .send()
            .await
            .map_err(|err| CoreError::from_reqwest("server-speed-test", &err))?;
        let status = resp.status();
        if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
            return Err(map_http_status("server-speed-test", status, ""));
        }

        let mut bytes_tested = 0u64;
        while bytes_tested < SPEED_TEST_MAX_BYTES as u64 {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    let remaining = (SPEED_TEST_MAX_BYTES as u64) - bytes_tested;
                    bytes_tested += (chunk.len() as u64).min(remaining);
                }
                Ok(None) => break,
                Err(err) => {
                    if bytes_tested > 0 {
                        break;
                    }
                    return Err(CoreError::from_reqwest("server-speed-test", &err));
                }
            }
        }
        Ok((bytes_tested, start.elapsed()))
    }

    pub async fn list_libraries(
        &self,
        profile: &ServerProfile,
    ) -> Result<Vec<ServerLibrary>, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::get_music_libraries(&self.http, profile).await,
            ServerKind::Subsonic => subsonic::get_music_folders(&self.http, profile).await,
            ServerKind::WebDav => {
                let listing = webdav::browse_folder(&self.http, profile, None).await?;
                Ok(listing
                    .folders
                    .into_iter()
                    .map(|folder| ServerLibrary {
                        id: encode_server_item_id(ServerKind::WebDav, &profile.id, &folder.path),
                        name: folder.name,
                        kind: "folder".to_string(),
                    })
                    .collect())
            }
            ServerKind::Smb => {
                let prof = profile.clone();
                tokio::task::spawn_blocking(move || smb::list_libraries(&prof))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn list_artists(
        &self,
        profile: &ServerProfile,
        library_id: Option<&str>,
    ) -> Result<Vec<HomeCard>, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::get_artists(&self.http, profile, library_id).await,
            ServerKind::Subsonic => subsonic::get_artists(&self.http, profile, library_id).await,
            ServerKind::WebDav => {
                webdav::list_artists(&self.http, profile, library_id, self.cache_dir.as_deref())
                    .await
            }
            ServerKind::Smb => {
                let prof = profile.clone();
                let lib = library_id.map(|id| unwrap_item_id(ServerKind::Smb, id).to_string());
                tokio::task::spawn_blocking(move || smb::list_artists(&prof, lib.as_deref()))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn list_albums(
        &self,
        profile: &ServerProfile,
        library_id: Option<&str>,
        artist_id: Option<&str>,
    ) -> Result<Vec<HomeCard>, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => {
                jellyfin::get_albums(&self.http, profile, library_id, artist_id).await
            }
            ServerKind::Subsonic => {
                if let Some(aid) = artist_id {
                    subsonic::get_artist_albums(&self.http, profile, aid).await
                } else {
                    subsonic::get_album_list(&self.http, profile, "alphabeticalByName", 100).await
                }
            }
            ServerKind::WebDav => {
                webdav::list_albums(
                    &self.http,
                    profile,
                    library_id,
                    artist_id,
                    self.cache_dir.as_deref(),
                )
                .await
            }
            ServerKind::Smb => {
                let prof = profile.clone();
                let lib = library_id.map(|id| unwrap_item_id(ServerKind::Smb, id).to_string());
                let art = artist_id.map(|id| unwrap_item_id(ServerKind::Smb, id).to_string());
                tokio::task::spawn_blocking(move || {
                    smb::list_albums(&prof, lib.as_deref(), art.as_deref())
                })
                .await
                .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn get_album(
        &self,
        profile: &ServerProfile,
        album_id: &str,
    ) -> Result<AlbumDetail, CoreError> {
        let raw_id = unwrap_item_id(profile.kind, album_id);
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::get_album_detail(&self.http, profile, raw_id).await,
            ServerKind::Subsonic => subsonic::get_album(&self.http, profile, raw_id).await,
            ServerKind::WebDav => {
                webdav::get_album_detail(&self.http, profile, raw_id, self.cache_dir.as_deref())
                    .await
            }
            ServerKind::Smb => {
                let prof = profile.clone();
                let raw = raw_id.to_string();
                tokio::task::spawn_blocking(move || smb::get_album_detail(&prof, &raw))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn list_playlists(
        &self,
        profile: &ServerProfile,
    ) -> Result<Vec<Playlist>, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::get_playlists(&self.http, profile).await,
            ServerKind::Subsonic => subsonic::get_playlists(&self.http, profile).await,
            ServerKind::WebDav => Ok(Vec::new()),
            ServerKind::Smb => {
                let prof = profile.clone();
                tokio::task::spawn_blocking(move || smb::list_playlists(&prof))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn get_playlist_tracks(
        &self,
        profile: &ServerProfile,
        playlist_id: &str,
    ) -> Result<Vec<Track>, CoreError> {
        let raw_id = unwrap_item_id(profile.kind, playlist_id);
        match profile.kind {
            ServerKind::Jellyfin => {
                jellyfin::get_playlist_tracks(&self.http, profile, raw_id).await
            }
            ServerKind::Subsonic => subsonic::get_playlist(&self.http, profile, raw_id).await,
            ServerKind::WebDav => {
                let listing = webdav::browse_folder(&self.http, profile, Some(raw_id)).await?;
                Ok(listing.tracks)
            }
            ServerKind::Smb => {
                let prof = profile.clone();
                let raw = raw_id.to_string();
                tokio::task::spawn_blocking(move || smb::get_playlist_tracks(&prof, &raw))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub async fn search(
        &self,
        profile: &ServerProfile,
        query: &str,
        limit: usize,
    ) -> Result<ServerSearchResult, CoreError> {
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::search(&self.http, profile, query, limit).await,
            ServerKind::Subsonic => subsonic::search3(&self.http, profile, query, limit).await,
            ServerKind::WebDav => webdav::search(&self.http, profile, query, limit).await,
            ServerKind::Smb => {
                let prof = profile.clone();
                let q = query.to_string();
                tokio::task::spawn_blocking(move || smb::search(&prof, &q, limit))
                    .await
                    .map_err(|err| CoreError::internal(format!("SMB task panicked: {err}")))?
            }
        }
    }

    pub fn resolve_stream(
        &self,
        profile: &ServerProfile,
        track: &Track,
    ) -> Result<ServerStream, CoreError> {
        let raw_id = unwrap_item_id(profile.kind, &track.id);
        match profile.kind {
            ServerKind::Jellyfin => jellyfin::stream_request(profile, raw_id, track),
            ServerKind::Subsonic => subsonic::stream_request(profile, raw_id, track),
            ServerKind::WebDav => webdav::stream_request(profile, raw_id, track),
            ServerKind::Smb => smb::stream_request(profile, raw_id, track),
        }
    }
}

pub(crate) fn unwrap_item_id(kind: ServerKind, id: &str) -> &str {
    if let Some(parsed) = parse_server_item_id(id) {
        if parsed.kind == kind {
            let prefix_len = kind.id_prefix().len();
            if let Some((_, raw)) = id[prefix_len..].split_once(':') {
                return raw;
            }
        }
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "sonata_servers_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn encodes_and_parses_server_item_ids_including_colon_and_slashes() {
        let encoded = encode_server_item_id(ServerKind::Jellyfin, "nas:main", "item:42");
        assert_eq!(encoded, "jellyfin:nas%3Amain:item:42");
        let parsed = parse_server_item_id(&encoded).expect("valid server item id");
        assert_eq!(parsed.kind, ServerKind::Jellyfin);
        assert_eq!(parsed.server_id, "nas:main");
        assert_eq!(parsed.item_id, "item:42");

        let dav_id =
            encode_server_item_id(ServerKind::WebDav, "cloud", "/Music/Artist/01%20Song.flac");
        let parsed_dav = parse_server_item_id(&dav_id).expect("valid webdav item id");
        assert_eq!(parsed_dav.kind, ServerKind::WebDav);
        assert_eq!(parsed_dav.server_id, "cloud");
        assert_eq!(parsed_dav.item_id, "/Music/Artist/01%20Song.flac");

        assert!(parse_server_item_id("jiosaavn:xyz").is_none());
        assert!(parse_server_item_id("jellyfin:onlyserver").is_none());
    }

    #[test]
    fn track_reports_self_hosted_source_tags() {
        let make_track = |id: &str| Track {
            id: id.to_string(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(180),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };

        let jf = make_track("jellyfin:srv1:item1");
        assert!(jf.is_self_hosted());
        assert_eq!(jf.source_tag(), "JFN");

        let sub = make_track("subsonic:srv2:item2");
        assert!(sub.is_self_hosted());
        assert_eq!(sub.source_tag(), "SUB");

        let dav = make_track("webdav:srv3:/Music/track.flac");
        assert!(dav.is_self_hosted());
        assert_eq!(dav.source_tag(), "DAV");

        let smb = make_track("smb:srv4:Album/track.flac");
        assert!(smb.is_self_hosted());
        assert_eq!(smb.source_tag(), "SMB");

        let ytm = make_track("dQw4w9WgXcQ");
        assert!(!ytm.is_self_hosted());
        assert_eq!(ytm.source_tag(), "YTM");
    }

    #[test]
    fn backwards_compatible_server_profile_deserialization_and_throughput_math() {
        let legacy_json = r#"{
            "version": 1,
            "servers": [{
                "id": "jf-legacy",
                "name": "Legacy JF",
                "kind": "jellyfin",
                "base_url": "https://jf.internal"
            }]
        }"#;
        let root = unique_temp_root("legacy");
        let path = root.join("servers.json");
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(&path, legacy_json).expect("write legacy");
        let loaded = load_server_profiles_from_path(&path).expect("load legacy");
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].enabled);
        assert!(loaded[0].share.is_none());
        assert!(loaded[0].domain.is_none());
        assert!(loaded[0].local_mount_path.is_none());
        let _ = fs::remove_dir_all(&root);

        // 1,000,000 bytes in 0.5 seconds = 16.0 Mbps
        let mbps = compute_download_mbps(1_000_000, Duration::from_millis(500)).expect("mbps");
        assert!((mbps - 16.0).abs() < 0.01, "mbps={mbps}");
        assert!(compute_download_mbps(0, Duration::from_secs(1)).is_none());
        assert!(compute_download_mbps(1024, Duration::ZERO).is_none());
    }

    #[tokio::test]
    async fn probes_health_and_measures_http_download_speed_on_loopback() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("addr");

        let server = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    if req.starts_with("PROPFIND ") {
                        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/Music/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>/Music/song.flac</d:href>
    <d:propstat><d:prop><d:getcontenttype>audio/flac</d:getcontenttype><d:getcontentlength>131072</d:getcontentlength></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
</d:multistatus>"#;
                        let resp = format!(
                            "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            xml.len(),
                            xml
                        );
                        let _ = sock.write_all(resp.as_bytes()).await;
                    } else if req.starts_with("GET /Music/song.flac") {
                        let payload = vec![0x5Au8; 131_072];
                        let hdr = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        );
                        let _ = sock.write_all(hdr.as_bytes()).await;
                        let _ = sock.write_all(&payload).await;
                    }
                });
            }
        });

        let client = ServerClient::new(crate::providers::build_http_client());
        let dav_profile = ServerProfile::new(
            "dav-loop",
            "Loopback DAV",
            ServerKind::WebDav,
            format!("http://{addr}/Music"),
        );

        let health = client.probe_server_health(&dav_profile).await;
        assert!(health.alive, "health={health:?}");
        assert!(health.latency_ms.is_some());

        let speed = client.run_speed_test(&dav_profile).await;
        assert!(speed.alive, "speed={speed:?}");
        assert_eq!(speed.bytes_tested, 131_072);
        assert!(speed.download_mbps.is_some_and(|m| m > 0.0));

        server.abort();
    }

    #[tokio::test]
    async fn probes_smb_health_and_measures_mounted_speed_test() {
        use tokio::net::TcpListener;

        let root = unique_temp_root("smb_speed");
        fs::create_dir_all(&root).expect("mkdir");
        let sample_file = root.join("01 - Sample.flac");
        fs::write(&sample_file, vec![0x42u8; 262_144]).expect("write sample");

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        let accept_task = tokio::spawn(async move {
            loop {
                if listener.accept().await.is_err() {
                    break;
                }
            }
        });

        let mut smb_profile = ServerProfile::new(
            "smb-loop",
            "Loopback SMB",
            ServerKind::Smb,
            format!("smb://127.0.0.1:{port}/Music"),
        );
        smb_profile.local_mount_path = Some(root.clone());

        let client = ServerClient::new(crate::providers::build_http_client());
        let health = client.probe_server_health(&smb_profile).await;
        assert!(health.alive, "health={health:?}");

        let speed = client.run_speed_test(&smb_profile).await;
        assert!(speed.alive, "speed={speed:?}");
        assert_eq!(speed.bytes_tested, 262_144);
        assert!(speed.download_mbps.is_some_and(|m| m > 0.0));

        accept_task.abort();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn saves_and_loads_server_profiles_with_private_permissions() {
        let root = unique_temp_root("persist");
        let paths = CorePaths::for_app_roots(root.join("cfg"), root.join("cache"));

        assert!(load_server_profiles(&paths)
            .expect("missing config returns empty")
            .is_empty());

        let mut jf = ServerProfile::new(
            "jf-1",
            "Home Jellyfin",
            ServerKind::Jellyfin,
            "https://jf.internal/",
        );
        jf.api_key = Some("super-secret-api-key".to_string());
        jf.user_id = Some("user-01".to_string());

        let mut sub = ServerProfile::new(
            "sub-1",
            "Navidrome",
            ServerKind::Subsonic,
            "https://nav.internal",
        );
        sub.username = Some("alice".to_string());
        sub.password = Some("sub-password".to_string());

        save_server_profiles(&paths, &[jf.clone(), sub.clone()]).expect("save succeeds");

        let config_path = paths.servers_config_path();
        assert!(config_path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let dir_mode = fs::metadata(&paths.config_dir)
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            let file_mode = fs::metadata(&config_path)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }

        // Verify debug formatting never leaks secrets.
        let dbg_text = format!("{jf:?} {sub:?}");
        assert!(!dbg_text.contains("super-secret-api-key"));
        assert!(!dbg_text.contains("sub-password"));

        let loaded = load_server_profiles(&paths).expect("reload succeeds");
        assert_eq!(loaded, vec![jf, sub]);

        let _ = fs::remove_dir_all(&root);
    }
}
