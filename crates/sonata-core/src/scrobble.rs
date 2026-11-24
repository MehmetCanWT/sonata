//! Portable Last.fm and ListenBrainz scrobbling client, config store, and threshold tracker.
//!
//! 1. **Configuration (`ScrobbleConfig`)**: Atomic `0o700` directory / `0o600` file persistence at
//!    `paths.scrobble_config_path()` (`scrobble.json`) for Last.fm and ListenBrainz credentials.
//! 2. **Signing & Payloads**: Pure-Rust RFC 1321 MD5 `api_sig` generation for Last.fm
//!    `track.updateNowPlaying` / `track.scrobble` and JSON payloads for ListenBrainz `/1/submit-listens`.
//! 3. **Threshold Tracking (`should_scrobble`, `ScrobbleTracker`)**: Enforces the `40s` / `50%`
//!    playback rule so tracks are scrobbled once after sufficient listening time.

use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::paths::CorePaths;
use crate::providers::servers::subsonic::md5_hex;
use crate::providers::Track;
use crate::redaction::sensitive_text;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
pub const DEFAULT_LISTENBRAINZ_API_URL: &str = "https://api.listenbrainz.org/1/submit-listens";
pub const SCROBBLE_THRESHOLD_SECS: u64 = 40;

const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Last.fm scrobbling account configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LastFmConfig {
    pub enabled: bool,
    pub api_key: String,
    pub api_secret: String,
    pub session_key: String,
    pub username: String,
}

impl LastFmConfig {
    /// Returns `true` when Last.fm scrobbling is enabled and has key, secret, and session key.
    pub fn is_configured(&self) -> bool {
        self.enabled
            && !self.api_key.trim().is_empty()
            && !self.api_secret.trim().is_empty()
            && !self.session_key.trim().is_empty()
    }
}

/// ListenBrainz scrobbling account configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListenBrainzConfig {
    pub enabled: bool,
    pub token: String,
    pub username: String,
}

impl ListenBrainzConfig {
    /// Returns `true` when ListenBrainz scrobbling is enabled and has a user token.
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.token.trim().is_empty()
    }
}

/// Persistent scrobbler configuration covering Last.fm and ListenBrainz.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrobbleConfig {
    pub lastfm: LastFmConfig,
    pub listenbrainz: ListenBrainzConfig,
}

impl ScrobbleConfig {
    /// Loads scrobble settings from `paths.scrobble_config_path()`, returning defaults if missing.
    pub fn load_from_paths(paths: &CorePaths) -> Result<Self, CoreError> {
        Self::load_from(&paths.scrobble_config_path())
    }

    /// Saves scrobble settings atomically to `paths.scrobble_config_path()` (`0o700` dir / `0o600` file).
    pub fn save_to_paths(&self, paths: &CorePaths) -> Result<(), CoreError> {
        self.save_to(&paths.scrobble_config_path())
    }

    /// Loads scrobble settings from an explicit JSON file path, returning defaults if absent.
    pub fn load_from(path: &Path) -> Result<Self, CoreError> {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str::<Self>(&raw)
                .map_err(|err| CoreError::invalid_response("scrobble", err.to_string())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(CoreError::io(err.to_string())),
        }
    }

    /// Writes scrobble settings atomically using a `0o700` parent directory and `0o600` temp file.
    pub fn save_to(&self, path: &Path) -> Result<(), CoreError> {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent).map_err(|err| CoreError::io(err.to_string()))?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|err| CoreError::internal(format!("serialize scrobble config: {err}")))?;
        let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("scrobble.json");
        let tmp_path =
            path.with_file_name(format!(".{file_name}.{}.{seq}.tmp", std::process::id()));

        let write_result = (|| -> std::io::Result<()> {
            let mut file = create_private_file(&tmp_path)?;
            file.write_all(&body)?;
            file.sync_all()?;
            fs::rename(&tmp_path, path)?;
            Ok(())
        })();

        if let Err(err) = write_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoreError::io(err.to_string()));
        }
        Ok(())
    }

    /// Returns `true` when at least one scrobbler backend is enabled and configured.
    pub fn any_configured(&self) -> bool {
        self.lastfm.is_configured() || self.listenbrainz.is_configured()
    }
}

/// Metadata required for `updateNowPlaying` and `scrobble` submissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrobbleTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
}

impl ScrobbleTrack {
    /// Builds scrobble metadata from a Sonata [`Track`].
    pub fn from_track(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_secs: track.duration_secs,
        }
    }
}

/// Returns `true` when playback meets the `40s` / `50%` scrobble threshold.
pub fn should_scrobble(duration_secs: Option<u64>, played_secs: u64) -> bool {
    if played_secs == 0 {
        return false;
    }
    match duration_secs.filter(|&dur| dur > 0) {
        Some(dur) => played_secs >= SCROBBLE_THRESHOLD_SECS || played_secs.saturating_mul(2) >= dur,
        None => played_secs >= SCROBBLE_THRESHOLD_SECS,
    }
}

/// Stateful per-track scrobble eligibility tracker preventing duplicate submissions.
#[derive(Debug, Clone)]
pub struct ScrobbleTracker {
    track_id: String,
    track: ScrobbleTrack,
    started_at_unix: u64,
    accumulated_secs: f64,
    scrobbled: bool,
}

impl ScrobbleTracker {
    /// Starts tracking a newly started track at `started_at_unix`.
    pub fn start(track_id: impl Into<String>, track: ScrobbleTrack, started_at_unix: u64) -> Self {
        Self {
            track_id: track_id.into(),
            track,
            started_at_unix,
            accumulated_secs: 0.0,
            scrobbled: false,
        }
    }

    /// Advances listened playback time by `delta_secs` and returns a `(track, timestamp)` pair
    /// exactly once when the `40s` / `50%` threshold is reached.
    pub fn advance(&mut self, delta_secs: f64) -> Option<(&ScrobbleTrack, u64)> {
        if self.scrobbled || !delta_secs.is_finite() || delta_secs <= 0.0 {
            return None;
        }
        self.accumulated_secs += delta_secs;
        let played_secs = self.accumulated_secs.floor() as u64;
        if should_scrobble(self.track.duration_secs, played_secs) {
            self.scrobbled = true;
            Some((&self.track, self.started_at_unix))
        } else {
            None
        }
    }

    /// Returns the tracked track ID.
    pub fn track_id(&self) -> &str {
        &self.track_id
    }

    /// Returns `true` once this track has crossed the scrobble threshold.
    pub fn is_scrobbled(&self) -> bool {
        self.scrobbled
    }
}

/// Computes Last.fm's sorted-parameter MD5 `api_sig` excluding `format` and `callback`.
pub fn compute_lastfm_api_sig(params: &BTreeMap<String, String>, api_secret: &str) -> String {
    let mut raw = String::new();
    for (key, value) in params {
        if key == "format" || key == "callback" {
            continue;
        }
        raw.push_str(key);
        raw.push_str(value);
    }
    raw.push_str(api_secret.trim());
    md5_hex(raw.as_bytes())
}

/// Builds signed form parameters for Last.fm `track.updateNowPlaying`.
pub fn build_lastfm_now_playing_params(
    cfg: &LastFmConfig,
    track: &ScrobbleTrack,
) -> Result<Vec<(String, String)>, CoreError> {
    if !cfg.is_configured() {
        return Err(CoreError::auth_required(
            "Last.fm scrobbling is not configured",
        ));
    }
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), "track.updateNowPlaying".to_string());
    params.insert("api_key".to_string(), cfg.api_key.trim().to_string());
    params.insert("sk".to_string(), cfg.session_key.trim().to_string());
    params.insert("artist".to_string(), track.artist.trim().to_string());
    params.insert("track".to_string(), track.title.trim().to_string());
    if !track.album.trim().is_empty() {
        params.insert("album".to_string(), track.album.trim().to_string());
    }
    if let Some(dur) = track.duration_secs.filter(|&d| d > 0) {
        params.insert("duration".to_string(), dur.to_string());
    }
    finalize_lastfm_params(params, &cfg.api_secret)
}

/// Builds signed form parameters for Last.fm `track.scrobble`.
pub fn build_lastfm_scrobble_params(
    cfg: &LastFmConfig,
    track: &ScrobbleTrack,
    timestamp_unix: u64,
) -> Result<Vec<(String, String)>, CoreError> {
    if !cfg.is_configured() {
        return Err(CoreError::auth_required(
            "Last.fm scrobbling is not configured",
        ));
    }
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), "track.scrobble".to_string());
    params.insert("api_key".to_string(), cfg.api_key.trim().to_string());
    params.insert("sk".to_string(), cfg.session_key.trim().to_string());
    params.insert("artist".to_string(), track.artist.trim().to_string());
    params.insert("track".to_string(), track.title.trim().to_string());
    params.insert("timestamp".to_string(), timestamp_unix.to_string());
    if !track.album.trim().is_empty() {
        params.insert("album".to_string(), track.album.trim().to_string());
    }
    if let Some(dur) = track.duration_secs.filter(|&d| d > 0) {
        params.insert("duration".to_string(), dur.to_string());
    }
    finalize_lastfm_params(params, &cfg.api_secret)
}

fn finalize_lastfm_params(
    mut params: BTreeMap<String, String>,
    api_secret: &str,
) -> Result<Vec<(String, String)>, CoreError> {
    let api_sig = compute_lastfm_api_sig(&params, api_secret);
    params.insert("api_sig".to_string(), api_sig);
    params.insert("format".to_string(), "json".to_string());
    Ok(params.into_iter().collect())
}

/// Builds a ListenBrainz `/1/submit-listens` JSON payload (`playing_now` or `single`).
pub fn build_listenbrainz_payload(track: &ScrobbleTrack, listened_at_unix: Option<u64>) -> Value {
    let mut additional_info = serde_json::Map::new();
    additional_info.insert("media_player".to_string(), json!("Sonata"));
    if let Some(dur) = track.duration_secs.filter(|&d| d > 0) {
        additional_info.insert("duration_ms".to_string(), json!(dur.saturating_mul(1000)));
    }

    let mut track_meta = serde_json::Map::new();
    track_meta.insert("artist_name".to_string(), json!(track.artist.trim()));
    track_meta.insert("track_name".to_string(), json!(track.title.trim()));
    if !track.album.trim().is_empty() {
        track_meta.insert("release_name".to_string(), json!(track.album.trim()));
    }
    track_meta.insert(
        "additional_info".to_string(),
        Value::Object(additional_info),
    );

    let mut entry = serde_json::Map::new();
    if let Some(ts) = listened_at_unix {
        entry.insert("listened_at".to_string(), json!(ts));
    }
    entry.insert("track_metadata".to_string(), Value::Object(track_meta));

    let listen_type = if listened_at_unix.is_some() {
        "single"
    } else {
        "playing_now"
    };

    json!({
        "listen_type": listen_type,
        "payload": [Value::Object(entry)]
    })
}

/// Portable async HTTP client dispatching now-playing and scrobble calls to Last.fm and ListenBrainz.
#[derive(Clone)]
pub struct ScrobbleClient {
    http: Client,
    lastfm_url: String,
    listenbrainz_url: String,
}

impl Default for ScrobbleClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrobbleClient {
    /// Creates a scrobble client with default Last.fm and ListenBrainz API endpoints.
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .connect_timeout(Duration::from_secs(5))
            .user_agent("Sonata/1.0")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            lastfm_url: DEFAULT_LASTFM_API_URL.to_string(),
            listenbrainz_url: DEFAULT_LISTENBRAINZ_API_URL.to_string(),
        }
    }

    /// Creates a scrobble client pointing at custom base endpoints (useful for local tests).
    pub fn with_endpoints(
        lastfm_url: impl Into<String>,
        listenbrainz_url: impl Into<String>,
    ) -> Self {
        let mut client = Self::new();
        client.lastfm_url = lastfm_url.into();
        client.listenbrainz_url = listenbrainz_url.into();
        client
    }

    /// Sends a `Now Playing` notification to every configured scrobbler backend.
    pub async fn update_now_playing(
        &self,
        config: &ScrobbleConfig,
        track: &ScrobbleTrack,
    ) -> Result<(), CoreError> {
        if track.title.trim().is_empty() || track.artist.trim().is_empty() {
            return Ok(());
        }
        let mut first_err = None;
        if config.lastfm.is_configured() {
            if let Err(err) = self.lastfm_update_now_playing(&config.lastfm, track).await {
                first_err.get_or_insert(err);
            }
        }
        if config.listenbrainz.is_configured() {
            if let Err(err) = self
                .listenbrainz_submit(&config.listenbrainz, track, None)
                .await
            {
                first_err.get_or_insert(err);
            }
        }
        match first_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Submits a completed scrobble to every configured backend.
    pub async fn scrobble(
        &self,
        config: &ScrobbleConfig,
        track: &ScrobbleTrack,
        timestamp_unix: Option<u64>,
    ) -> Result<(), CoreError> {
        if track.title.trim().is_empty() || track.artist.trim().is_empty() {
            return Ok(());
        }
        let ts = timestamp_unix.unwrap_or_else(now_unix_secs);
        let mut first_err = None;
        if config.lastfm.is_configured() {
            if let Err(err) = self.lastfm_scrobble(&config.lastfm, track, ts).await {
                first_err.get_or_insert(err);
            }
        }
        if config.listenbrainz.is_configured() {
            if let Err(err) = self
                .listenbrainz_submit(&config.listenbrainz, track, Some(ts))
                .await
            {
                first_err.get_or_insert(err);
            }
        }
        match first_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Sends Last.fm `track.updateNowPlaying`.
    pub async fn lastfm_update_now_playing(
        &self,
        cfg: &LastFmConfig,
        track: &ScrobbleTrack,
    ) -> Result<(), CoreError> {
        let params = build_lastfm_now_playing_params(cfg, track)?;
        self.post_lastfm_form(&params).await
    }

    /// Sends Last.fm `track.scrobble`.
    pub async fn lastfm_scrobble(
        &self,
        cfg: &LastFmConfig,
        track: &ScrobbleTrack,
        timestamp_unix: u64,
    ) -> Result<(), CoreError> {
        let params = build_lastfm_scrobble_params(cfg, track, timestamp_unix)?;
        self.post_lastfm_form(&params).await
    }

    async fn post_lastfm_form(&self, params: &[(String, String)]) -> Result<(), CoreError> {
        let resp = self
            .http
            .post(&self.lastfm_url)
            .form(params)
            .send()
            .await
            .map_err(|err| CoreError::from_reqwest("lastfm", &err))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|err| CoreError::from_reqwest("lastfm", &err))?;

        if !status.is_success() {
            return Err(map_scrobble_http_status("lastfm", status.as_u16(), &body));
        }

        if let Ok(json) = serde_json::from_str::<Value>(&body) {
            if let Some(code) = json.get("error").and_then(|v| v.as_u64()) {
                let message = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Last.fm API error");
                return Err(match code {
                    4 | 9 | 10 | 26 => {
                        CoreError::auth_required(format!("Last.fm error {code}: {message}"))
                    }
                    29 => CoreError::rate_limited(
                        None,
                        format!("Last.fm rate limit ({code}): {message}"),
                    ),
                    _ => CoreError::invalid_response(
                        "lastfm",
                        format!("Last.fm error {code}: {message}"),
                    ),
                });
            }
        }
        Ok(())
    }

    /// Sends ListenBrainz `/1/submit-listens` with `playing_now` (`None`) or `single` (`Some(ts)`).
    pub async fn listenbrainz_submit(
        &self,
        cfg: &ListenBrainzConfig,
        track: &ScrobbleTrack,
        listened_at_unix: Option<u64>,
    ) -> Result<(), CoreError> {
        if !cfg.is_configured() {
            return Err(CoreError::auth_required(
                "ListenBrainz scrobbling is not configured",
            ));
        }
        let payload = build_listenbrainz_payload(track, listened_at_unix);
        let resp = self
            .http
            .post(&self.listenbrainz_url)
            .header("Authorization", format!("Token {}", cfg.token.trim()))
            .json(&payload)
            .send()
            .await
            .map_err(|err| CoreError::from_reqwest("listenbrainz", &err))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        Err(map_scrobble_http_status(
            "listenbrainz",
            status.as_u16(),
            &body,
        ))
    }
}

fn map_scrobble_http_status(provider: &str, status: u16, body: &str) -> CoreError {
    let safe_body = sensitive_text(body);
    let detail = if safe_body.trim().is_empty() {
        format!("{provider} HTTP {status}")
    } else {
        format!("{provider} HTTP {status}: {safe_body}")
    };
    match status {
        401 | 403 => CoreError::auth_required(detail),
        429 => CoreError::rate_limited(None, detail),
        404 => CoreError::not_found(detail),
        _ => CoreError::invalid_response(provider, detail),
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn temp_config_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonata_scrobble_{name}_{}_{}",
            std::process::id(),
            TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn persists_scrobble_config_atomically_with_private_permissions() {
        let root = temp_config_dir("perms");
        let paths = CorePaths::for_app_roots(&root, &root);
        let cfg = ScrobbleConfig {
            lastfm: LastFmConfig {
                enabled: true,
                api_key: "lfm_key".to_string(),
                api_secret: "lfm_secret".to_string(),
                session_key: "lfm_sk".to_string(),
                username: "alice".to_string(),
            },
            listenbrainz: ListenBrainzConfig {
                enabled: true,
                token: "lb_token".to_string(),
                username: "alice_lb".to_string(),
            },
        };
        cfg.save_to_paths(&paths).expect("save scrobble config");

        let reloaded = ScrobbleConfig::load_from_paths(&paths).expect("load scrobble config");
        assert_eq!(reloaded, cfg);
        assert!(reloaded.any_configured());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let dir_mode = fs::metadata(&paths.config_dir)
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            let file_mode = fs::metadata(paths.scrobble_config_path())
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn computes_sorted_lastfm_api_sig_excluding_format_and_callback() {
        let mut params = BTreeMap::new();
        params.insert("track".to_string(), "Strobe".to_string());
        params.insert("artist".to_string(), "deadmau5".to_string());
        params.insert("method".to_string(), "track.scrobble".to_string());
        params.insert("api_key".to_string(), "KEY123".to_string());
        params.insert("sk".to_string(), "SK456".to_string());
        params.insert("timestamp".to_string(), "1700000000".to_string());
        params.insert("format".to_string(), "json".to_string());
        params.insert("callback".to_string(), "ignored".to_string());

        let sig = compute_lastfm_api_sig(&params, "SECRET789");
        let expected_plain = concat!(
            "api_keyKEY123",
            "artistdeadmau5",
            "methodtrack.scrobble",
            "skSK456",
            "timestamp1700000000",
            "trackStrobe",
            "SECRET789"
        );
        assert_eq!(sig, md5_hex(expected_plain.as_bytes()));
    }

    #[test]
    fn enforces_40s_and_50_percent_scrobble_threshold() {
        // Long track: 40s comes before 50% (100s)
        assert!(!should_scrobble(Some(200), 39));
        assert!(should_scrobble(Some(200), 40));

        // Short track: 50% (30s) comes before 40s
        assert!(!should_scrobble(Some(60), 29));
        assert!(should_scrobble(Some(60), 30));

        // Unknown duration: requires full 40s
        assert!(!should_scrobble(None, 39));
        assert!(should_scrobble(None, 40));
        assert!(!should_scrobble(Some(0), 0));

        let track = ScrobbleTrack {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(200),
        };
        let mut tracker = ScrobbleTracker::start("t-1", track, 1_700_000_000);
        assert!(tracker.advance(25.0).is_none());
        let (scrobbled_track, ts) = tracker.advance(15.5).expect("should scrobble at 40.5s");
        assert_eq!(scrobbled_track.title, "Song");
        assert_eq!(ts, 1_700_000_000);
        assert!(tracker.is_scrobbled());
        assert!(tracker.advance(50.0).is_none(), "must not double-scrobble");
    }

    #[test]
    fn builds_listenbrainz_playing_now_and_single_payloads() {
        let track = ScrobbleTrack {
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: "Hurry Up, We're Dreaming".to_string(),
            duration_secs: Some(243),
        };
        let now_playing = build_listenbrainz_payload(&track, None);
        assert_eq!(now_playing["listen_type"], "playing_now");
        assert!(now_playing["payload"][0].get("listened_at").is_none());
        assert_eq!(
            now_playing["payload"][0]["track_metadata"]["artist_name"],
            "M83"
        );
        assert_eq!(
            now_playing["payload"][0]["track_metadata"]["additional_info"]["duration_ms"],
            243_000
        );

        let single = build_listenbrainz_payload(&track, Some(1_700_000_123));
        assert_eq!(single["listen_type"], "single");
        assert_eq!(single["payload"][0]["listened_at"], 1_700_000_123);
    }

    #[tokio::test]
    async fn submits_now_playing_and_scrobble_over_http_to_mock_backends() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let mut captured = Vec::new();
            for _ in 0..4 {
                let (mut sock, _) = listener.accept().await.expect("accept");
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.expect("read");
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                captured.push(req);
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                sock.write_all(resp.as_bytes()).await.expect("write");
            }
            captured
        });

        let client = ScrobbleClient::with_endpoints(
            format!("http://{addr}/lastfm"),
            format!("http://{addr}/lb"),
        );
        let cfg = ScrobbleConfig {
            lastfm: LastFmConfig {
                enabled: true,
                api_key: "key-1".to_string(),
                api_secret: "sec-1".to_string(),
                session_key: "sk-1".to_string(),
                username: "u1".to_string(),
            },
            listenbrainz: ListenBrainzConfig {
                enabled: true,
                token: "lb-tok-1".to_string(),
                username: "u2".to_string(),
            },
        };
        let track = ScrobbleTrack {
            title: "Teardrop".to_string(),
            artist: "Massive Attack".to_string(),
            album: "Mezzanine".to_string(),
            duration_secs: Some(330),
        };

        client
            .update_now_playing(&cfg, &track)
            .await
            .expect("now playing");
        client
            .scrobble(&cfg, &track, Some(1_700_000_500))
            .await
            .expect("scrobble");

        let requests = server.await.expect("server join");
        assert_eq!(requests.len(), 4);
        assert!(requests
            .iter()
            .any(|r| r.contains("track.updateNowPlaying") && r.contains("api_sig=")));
        assert!(requests
            .iter()
            .any(|r| r.contains("track.scrobble") && r.contains("timestamp=1700000500")));
        assert!(requests.iter().any(|r| r
            .to_ascii_lowercase()
            .contains("authorization: token lb-tok-1")
            && r.contains("\"playing_now\"")));
        assert!(requests
            .iter()
            .any(|r| r.contains("\"single\"") && r.contains("1700000500")));
    }
}
