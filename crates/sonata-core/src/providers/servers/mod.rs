//! Portable self-hosted music server configuration, shared browse models, and unified client.
//!
//! 1. **Configuration (`ServerProfile`, `ServerKind`)**: Unified server profile persistence in
//!    `paths.servers_config_path()` using `0o700` directories and atomic `0o600` temp-file writes.
//! 2. **Item Addressing (`ServerItemRef`)**: Prefixed ID encoding (`jellyfin:`, `subsonic:`,
//!    `webdav:`) mapping self-hosted items into Sonata's `Track`, `Playlist`, `HomeCard`, and
//!    `AlbumDetail` models without modifying `ProviderSource`.
//! 3. **Protocol Clients (`jellyfin`, `subsonic`, `webdav`)**: Unified `ServerClient` dispatcher
//!    for ping, libraries, artists, albums, playlists, search, and direct stream URLs/headers.

pub mod jellyfin;
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
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const JELLYFIN_ID_PREFIX: &str = "jellyfin:";
pub const SUBSONIC_ID_PREFIX: &str = "subsonic:";
pub const WEBDAV_ID_PREFIX: &str = "webdav:";

const CONFIG_SCHEMA_VERSION: u32 = 1;
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(12);
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Supported self-hosted music server protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    Jellyfin,
    Subsonic,
    WebDav,
}

impl ServerKind {
    /// Returns the 3-letter badge tag shown alongside self-hosted tracks (`JFN`, `SUB`, `DAV`).
    pub const fn short_tag(self) -> &'static str {
        match self {
            Self::Jellyfin => "JFN",
            Self::Subsonic => "SUB",
            Self::WebDav => "DAV",
        }
    }

    /// Returns the human-readable protocol name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Jellyfin => "Jellyfin",
            Self::Subsonic => "Subsonic",
            Self::WebDav => "WebDAV",
        }
    }

    /// Returns the canonical ID prefix (`jellyfin:`, `subsonic:`, or `webdav:`).
    pub const fn id_prefix(self) -> &'static str {
        match self {
            Self::Jellyfin => JELLYFIN_ID_PREFIX,
            Self::Subsonic => SUBSONIC_ID_PREFIX,
            Self::WebDav => WEBDAV_ID_PREFIX,
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

/// Formats a normalized audio badge string such as `"FLAC · 980kbps"` or `"OPUS · Jellyfin"`.
pub(crate) fn format_audio_badge(
    codec: Option<&str>,
    bitrate_kbps: Option<u32>,
    fallback_label: &str,
) -> Option<String> {
    let clean_codec = codec
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_uppercase);
    match (clean_codec, bitrate_kbps.filter(|&b| b > 0)) {
        (Some(c), Some(kbps)) => Some(format!("{c} · {kbps}kbps")),
        (Some(c), None) => Some(format!("{c} · {fallback_label}")),
        (None, Some(kbps)) => Some(format!("{kbps}kbps · {fallback_label}")),
        (None, None) => None,
    }
}

pub(crate) fn default_http_timeout() -> Duration {
    DEFAULT_HTTP_TIMEOUT
}

/// Unified async facade over Jellyfin, Subsonic, and WebDAV self-hosted servers.
#[derive(Clone)]
pub struct ServerClient {
    http: Client,
}

impl ServerClient {
    pub fn new(http: Client) -> Self {
        Self { http }
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
        }
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
            ServerKind::WebDav => Ok(Vec::new()),
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
            ServerKind::WebDav => Ok(Vec::new()),
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
            ServerKind::WebDav => Err(CoreError::not_found(
                "WebDAV servers expose folder listings rather than album IDs",
            )),
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
            ServerKind::WebDav => {
                let tracks = webdav::search(&self.http, profile, query, limit).await?;
                Ok(ServerSearchResult {
                    tracks,
                    ..Default::default()
                })
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

        let ytm = make_track("dQw4w9WgXcQ");
        assert!(!ytm.is_self_hosted());
        assert_eq!(ytm.source_tag(), "YTM");
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
