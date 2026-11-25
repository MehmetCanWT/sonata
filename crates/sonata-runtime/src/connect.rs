//! Linux Sonata Connect LAN discovery broadcaster and authenticated HTTP/JSON control server.
//!
//! 1. **Paired Device Persistence (`PairedDeviceStore`)**: Stores paired client tokens atomically
//!    in `connect_paired.json` using `0o700` private directories and `0o600` temp-file writes.
//! 2. **UDP LAN Discovery (`DISCOVERY_PORT = 45389`)**: Broadcasts `ConnectBeacon` announcements
//!    so mobile and peer Sonata clients can discover this Linux instance on the local network.
//! 3. **HTTP/JSON Control Server (`ConnectHandle`)**: Serves `/connect/info`, `/connect/pair`
//!    (6-digit PIN), `/connect/state`, and HMAC-SHA1 authenticated `/connect/command` requests.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sonata_core::auth::{create_private_file, ensure_private_dir};
use sonata_core::connect::{
    constant_time_eq, generate_device_token, generate_pairing_pin, verify_signature, ConnectBeacon,
    ConnectCommand, ConnectStateSnapshot, PairAck, PairRequest, CONNECT_PROTOCOL_VERSION,
    DISCOVERY_PORT,
};
use sonata_core::redaction::sensitive_text;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

const COMMAND_CHANNEL_CAPACITY: usize = 64;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(5);
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Public device summary exposed by `/connect/info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectInfoResponse {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub protocol_version: u16,
    pub http_port: u16,
    pub requires_pin: bool,
}

/// Persisted paired device entry containing its shared HMAC token (`0o600` on disk).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDeviceRecord {
    pub device_id: String,
    pub device_name: String,
    pub token: String,
    pub paired_at_unix: u64,
}

/// Public paired device summary without exposing the raw HMAC secret token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub paired_at_unix: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedPairedDevices {
    #[serde(default)]
    devices: Vec<PairedDeviceRecord>,
}

/// Configuration for starting [`ConnectHandle`].
#[derive(Debug, Clone)]
pub struct ConnectServerConfig {
    pub device_id: String,
    pub device_name: String,
    pub app_version: String,
    pub http_bind_addr: SocketAddr,
    pub discovery_port: u16,
    pub enable_udp_beacon: bool,
    pub beacon_interval: Duration,
    pub paired_store_path: PathBuf,
}

impl Default for ConnectServerConfig {
    fn default() -> Self {
        let paths = crate::config::AppConfig::default_core_paths();
        Self {
            device_id: default_linux_device_id(),
            device_name: crate::config::DEFAULT_CONNECT_DEVICE_NAME.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: true,
            beacon_interval: Duration::from_secs(2),
            paired_store_path: paths.connect_paired_path(),
        }
    }
}

fn default_linux_device_id() -> String {
    let host = fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "linux".to_string());
    let sanitized: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("sonata-linux-{sanitized}")
}

struct ConnectServerState {
    config: ConnectServerConfig,
    http_port: u16,
    active_pin: Mutex<String>,
    paired_devices: Mutex<HashMap<String, PairedDeviceRecord>>,
    persist_lock: tokio::sync::Mutex<()>,
    file_lock: Arc<Mutex<()>>,
    state_rx: watch::Receiver<ConnectStateSnapshot>,
    cmd_tx: mpsc::Sender<ConnectCommand>,
}

impl ConnectServerState {
    fn current_pin(&self) -> String {
        self.active_pin.lock().expect("connect pin lock").clone()
    }

    fn regenerate_pin(&self) -> String {
        let next = generate_pairing_pin();
        *self.active_pin.lock().expect("connect pin lock") = next.clone();
        next
    }

    fn snapshot_and_save_sync(
        paired_devices: &Mutex<HashMap<String, PairedDeviceRecord>>,
        file_lock: &Mutex<()>,
        path: &Path,
    ) -> Result<()> {
        let _file_guard = file_lock.lock().expect("connect file lock");
        let records = paired_devices
            .lock()
            .expect("paired devices lock")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        save_paired_devices(path, &records)
    }

    async fn persist_current_devices_async(self: &Arc<Self>) -> Result<()> {
        let _persist_guard = self.persist_lock.lock().await;
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            Self::snapshot_and_save_sync(
                &state.paired_devices,
                &state.file_lock,
                &state.config.paired_store_path,
            )
        })
        .await
        .context("save_paired_devices task panicked")?
    }

    async fn verify_and_record_pair(self: &Arc<Self>, req: PairRequest) -> Result<PairAck> {
        let trimmed_id = req.device_id.trim();
        let trimmed_name = req.device_name.trim();
        if trimmed_id.is_empty() || trimmed_name.is_empty() {
            return Ok(PairAck {
                accepted: false,
                device_id: self.config.device_id.clone(),
                device_name: self.config.device_name.clone(),
                token: None,
                message: Some("device_id and device_name are required".to_string()),
            });
        }

        let expected_pin = self.current_pin();
        if !constant_time_eq(expected_pin.as_bytes(), req.pin.trim().as_bytes()) {
            return Ok(PairAck {
                accepted: false,
                device_id: self.config.device_id.clone(),
                device_name: self.config.device_name.clone(),
                token: None,
                message: Some("Invalid pairing PIN".to_string()),
            });
        }

        let token = generate_device_token();
        let record = PairedDeviceRecord {
            device_id: trimmed_id.to_string(),
            device_name: trimmed_name.to_string(),
            token: token.clone(),
            paired_at_unix: now_unix_secs(),
        };
        let _persist_guard = self.persist_lock.lock().await;
        {
            let mut guard = self.paired_devices.lock().expect("paired devices lock");
            guard.insert(trimmed_id.to_string(), record);
        }
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            Self::snapshot_and_save_sync(
                &state.paired_devices,
                &state.file_lock,
                &state.config.paired_store_path,
            )
        })
        .await
        .context("save_paired_devices task panicked")??;

        Ok(PairAck {
            accepted: true,
            device_id: self.config.device_id.clone(),
            device_name: self.config.device_name.clone(),
            token: Some(token),
            message: None,
        })
    }

    fn token_for_device(&self, device_id: &str) -> Option<String> {
        self.paired_devices
            .lock()
            .expect("paired devices lock")
            .get(device_id.trim())
            .map(|r| r.token.clone())
    }
}

/// Running Sonata Connect server handle with state broadcaster, PIN controls, and clean shutdown.
pub struct ConnectHandle {
    shared: Arc<ConnectServerState>,
    state_tx: watch::Sender<ConnectStateSnapshot>,
    local_addr: SocketAddr,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl ConnectHandle {
    /// Starts the TCP HTTP/JSON server and optional UDP LAN beacon broadcaster.
    pub async fn start(
        config: ConnectServerConfig,
    ) -> Result<(Self, mpsc::Receiver<ConnectCommand>)> {
        let loaded_devices = load_paired_devices(&config.paired_store_path)
            .unwrap_or_default()
            .into_iter()
            .map(|rec| (rec.device_id.clone(), rec))
            .collect::<HashMap<_, _>>();

        let listener = TcpListener::bind(config.http_bind_addr)
            .await
            .with_context(|| {
                format!(
                    "failed to bind Sonata Connect HTTP listener on {}",
                    config.http_bind_addr
                )
            })?;
        let local_addr = listener.local_addr()?;
        let http_port = local_addr.port();

        let initial_snapshot = ConnectStateSnapshot {
            device_id: config.device_id.clone(),
            device_name: config.device_name.clone(),
            updated_at_unix: now_unix_secs(),
            ..ConnectStateSnapshot::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_snapshot);
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

        let shared = Arc::new(ConnectServerState {
            config: config.clone(),
            http_port,
            active_pin: Mutex::new(generate_pairing_pin()),
            paired_devices: Mutex::new(loaded_devices),
            persist_lock: tokio::sync::Mutex::new(()),
            file_lock: Arc::new(Mutex::new(())),
            state_rx,
            cmd_tx,
        });

        let mut tasks = Vec::new();

        let accept_state = Arc::clone(&shared);
        tasks.push(tokio::spawn(async move {
            run_http_accept_loop(listener, accept_state).await;
        }));

        if config.enable_udp_beacon {
            let beacon_state = Arc::clone(&shared);
            tasks.push(tokio::spawn(async move {
                run_udp_beacon_loop(beacon_state).await;
            }));
        }

        tracing::info!(
            event = "sonata_connect_started",
            http_port,
            discovery_port = config.discovery_port,
            udp_beacon = config.enable_udp_beacon,
            "Sonata Connect server started"
        );

        Ok((
            Self {
                shared,
                state_tx,
                local_addr,
                tasks: Mutex::new(tasks),
            },
            cmd_rx,
        ))
    }

    /// Publishes an updated playback snapshot returned by `GET /connect/state`.
    pub fn update_state(&self, mut snapshot: ConnectStateSnapshot) {
        if snapshot.device_id.is_empty() {
            snapshot.device_id = self.shared.config.device_id.clone();
        }
        if snapshot.device_name.is_empty() {
            snapshot.device_name = self.shared.config.device_name.clone();
        }
        snapshot.updated_at_unix = now_unix_secs();
        self.state_tx.send_replace(snapshot);
    }

    /// Returns the currently active 6-digit pairing PIN.
    pub fn current_pin(&self) -> String {
        self.shared.current_pin()
    }

    /// Generates and activates a fresh 6-digit pairing PIN.
    pub fn regenerate_pin(&self) -> String {
        self.shared.regenerate_pin()
    }

    /// Returns the bound HTTP port.
    pub fn http_port(&self) -> u16 {
        self.shared.http_port
    }

    /// Returns the bound HTTP socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Lists paired devices without exposing secret HMAC tokens.
    pub fn paired_devices(&self) -> Vec<PairedDeviceInfo> {
        let mut items: Vec<PairedDeviceInfo> = self
            .shared
            .paired_devices
            .lock()
            .expect("paired devices lock")
            .values()
            .map(|r| PairedDeviceInfo {
                device_id: r.device_id.clone(),
                device_name: r.device_name.clone(),
                paired_at_unix: r.paired_at_unix,
            })
            .collect();
        items.sort_by(|a, b| a.device_name.cmp(&b.device_name));
        items
    }

    /// Unpairs `device_id` and persists the updated `connect_paired.json` store.
    pub fn unpair_device(&self, device_id: &str) -> Result<bool> {
        let removed = {
            let mut guard = self
                .shared
                .paired_devices
                .lock()
                .expect("paired devices lock");
            guard.remove(device_id.trim()).is_some()
        };
        if removed {
            ConnectServerState::snapshot_and_save_sync(
                &self.shared.paired_devices,
                &self.shared.file_lock,
                &self.shared.config.paired_store_path,
            )?;
        }
        Ok(removed)
    }

    /// Async variant of [`Self::unpair_device`] serialized via the async persistence mutex and `spawn_blocking`.
    pub async fn unpair_device_async(&self, device_id: &str) -> Result<bool> {
        let removed = {
            let mut guard = self
                .shared
                .paired_devices
                .lock()
                .expect("paired devices lock");
            guard.remove(device_id.trim()).is_some()
        };
        if removed {
            self.shared.persist_current_devices_async().await?;
        }
        Ok(removed)
    }

    /// Aborts background HTTP and UDP broadcaster tasks.
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.tasks.lock() {
            for handle in guard.drain(..) {
                handle.abort();
            }
        }
    }
}

impl Drop for ConnectHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn run_udp_beacon_loop(state: Arc<ConnectServerState>) {
    let Ok(socket) = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)).await
    else {
        tracing::warn!(
            event = "sonata_connect_udp_bind_failed",
            "failed to bind UDP socket for Sonata Connect discovery"
        );
        return;
    };
    if let Err(err) = socket.set_broadcast(true) {
        tracing::warn!(
            event = "sonata_connect_udp_broadcast_failed",
            error = %err,
            "failed to enable broadcast on Sonata Connect discovery socket"
        );
    }
    let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), state.config.discovery_port);
    let beacon = ConnectBeacon {
        device_id: state.config.device_id.clone(),
        device_name: state.config.device_name.clone(),
        platform: "linux".to_string(),
        http_port: state.http_port,
        protocol_version: CONNECT_PROTOCOL_VERSION,
        app_version: state.config.app_version.clone(),
    };
    let Ok(payload) = serde_json::to_vec(&beacon) else {
        return;
    };

    let mut ticker = tokio::time::interval(state.config.beacon_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(err) = socket.send_to(&payload, target).await {
            tracing::debug!(
                event = "sonata_connect_beacon_send_failed",
                error = %err,
                "Sonata Connect UDP beacon send failed"
            );
        }
    }
}

async fn run_http_accept_loop(listener: TcpListener, state: Arc<ConnectServerState>) {
    loop {
        let (stream, _peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::warn!(
                    event = "sonata_connect_accept_error",
                    error = %err,
                    "Sonata Connect HTTP accept failed; retrying"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let conn_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(err) = handle_http_connection(stream, conn_state).await {
                tracing::debug!(
                    event = "sonata_connect_http_error",
                    error = %sensitive_text(&err.to_string()),
                    "Sonata Connect HTTP connection failed"
                );
            }
        });
    }
}

struct ParsedHttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn read_http_request(stream: &mut TcpStream) -> Result<Option<ParsedHttpRequest>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];

    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() >= MAX_HTTP_HEADER_BYTES {
            anyhow::bail!("HTTP request headers exceeded maximum size");
        }
        let n = tokio::time::timeout(HTTP_IO_TIMEOUT, stream.read(&mut chunk))
            .await
            .context("HTTP header read timed out")??;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let header_text = std::str::from_utf8(&buf[..header_end])
        .context("HTTP request headers must be valid UTF-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut req_parts = request_line.split_whitespace();
    let method = req_parts.next().unwrap_or_default().to_ascii_uppercase();
    let raw_target = req_parts.next().unwrap_or_default();
    let path = raw_target
        .split('?')
        .next()
        .unwrap_or(raw_target)
        .to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_HTTP_BODY_BYTES {
        anyhow::bail!("HTTP request body exceeded maximum size");
    }

    let body_start = header_end + 4;
    let mut body = Vec::with_capacity(content_length);
    if buf.len() > body_start {
        let take = (buf.len() - body_start).min(content_length);
        body.extend_from_slice(&buf[body_start..body_start + take]);
    }
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let to_read = remaining.min(chunk.len());
        let n = tokio::time::timeout(HTTP_IO_TIMEOUT, stream.read(&mut chunk[..to_read]))
            .await
            .context("HTTP body read timed out")??;
        if n == 0 {
            anyhow::bail!("unexpected EOF reading HTTP request body");
        }
        body.extend_from_slice(&chunk[..n]);
    }

    Ok(Some(ParsedHttpRequest {
        method,
        path,
        headers,
        body,
    }))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn handle_http_connection(
    mut stream: TcpStream,
    state: Arc<ConnectServerState>,
) -> Result<()> {
    let Some(req) = read_http_request(&mut stream).await? else {
        return Ok(());
    };

    let (status_line, body_json) = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/connect/info") => {
            let info = ConnectInfoResponse {
                device_id: state.config.device_id.clone(),
                device_name: state.config.device_name.clone(),
                platform: "linux".to_string(),
                protocol_version: CONNECT_PROTOCOL_VERSION,
                http_port: state.http_port,
                requires_pin: true,
            };
            ("200 OK", serde_json::to_vec(&info)?)
        }
        ("POST", "/connect/pair") => match serde_json::from_slice::<PairRequest>(&req.body) {
            Ok(pair_req) => match state.verify_and_record_pair(pair_req).await {
                Ok(ack) if ack.accepted => ("200 OK", serde_json::to_vec(&ack)?),
                Ok(ack) => ("403 Forbidden", serde_json::to_vec(&ack)?),
                Err(err) => (
                    "500 Internal Server Error",
                    serde_json::to_vec(
                        &serde_json::json!({ "error": sensitive_text(&err.to_string()) }),
                    )?,
                ),
            },
            Err(err) => (
                "400 Bad Request",
                serde_json::to_vec(&serde_json::json!({ "error": err.to_string() }))?,
            ),
        },
        ("GET", "/connect/state") => {
            let snapshot = state.state_rx.borrow().clone();
            ("200 OK", serde_json::to_vec(&snapshot)?)
        }
        ("POST", "/connect/command") => {
            let device_id = req
                .headers
                .get("x-sonata-device-id")
                .map(String::as_str)
                .unwrap_or_default();
            let signature = req
                .headers
                .get("x-sonata-signature")
                .map(String::as_str)
                .unwrap_or_default();
            let authorized = state
                .token_for_device(device_id)
                .is_some_and(|token| verify_signature(&token, &req.body, signature));
            if !authorized {
                (
                    "401 Unauthorized",
                    serde_json::to_vec(&serde_json::json!({ "error": "unauthorized" }))?,
                )
            } else {
                match serde_json::from_slice::<ConnectCommand>(&req.body) {
                    Ok(cmd) => {
                        if state.cmd_tx.send(cmd).await.is_err() {
                            (
                                "503 Service Unavailable",
                                serde_json::to_vec(
                                    &serde_json::json!({ "error": "command receiver closed" }),
                                )?,
                            )
                        } else {
                            (
                                "200 OK",
                                serde_json::to_vec(&serde_json::json!({ "ok": true }))?,
                            )
                        }
                    }
                    Err(err) => (
                        "400 Bad Request",
                        serde_json::to_vec(&serde_json::json!({ "error": err.to_string() }))?,
                    ),
                }
            }
        }
        _ => (
            "404 Not Found",
            serde_json::to_vec(&serde_json::json!({ "error": "not found" }))?,
        ),
    };

    let response_headers = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_json.len()
    );
    tokio::time::timeout(HTTP_IO_TIMEOUT, async {
        stream.write_all(response_headers.as_bytes()).await?;
        stream.write_all(&body_json).await?;
        stream.flush().await
    })
    .await
    .context("HTTP response write timed out")??;
    Ok(())
}

/// Loads paired Sonata Connect devices from `path`, returning an empty list if missing.
pub fn load_paired_devices(path: &Path) -> Result<Vec<PairedDeviceRecord>> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let parsed: PersistedPairedDevices = serde_json::from_str(&raw)?;
            Ok(parsed.devices)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

/// Persists paired Sonata Connect devices atomically with `0o700` dir and `0o600` file permissions.
pub fn save_paired_devices(path: &Path, devices: &[PairedDeviceRecord]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    let payload = PersistedPairedDevices {
        devices: devices.to_vec(),
    };
    let body = serde_json::to_vec_pretty(&payload)?;
    let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("connect_paired.json");
    let tmp_path = path.with_file_name(format!(".{file_name}.{}.{seq}.tmp", std::process::id()));

    let write_res = (|| -> std::io::Result<()> {
        let mut file = create_private_file(&tmp_path)?;
        file.write_all(&body)?;
        file.sync_all()?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(err.into());
    }
    Ok(())
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
    use crate::downloads::test_support::TempTestDir;
    use sonata_core::connect::sign_payload;

    #[tokio::test]
    async fn pairs_serves_state_and_verifies_hmac_commands_with_private_storage() {
        let dir = TempTestDir::new("connect_server");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-test-1".to_string(),
            device_name: "Sonata Linux Test".to_string(),
            app_version: "1.0.7".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path.clone(),
        };

        let (handle, mut cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();

        // 1. /connect/info
        let info: ConnectInfoResponse = http
            .get(format!("{base}/connect/info"))
            .send()
            .await
            .expect("info request")
            .json()
            .await
            .expect("info json");
        assert_eq!(info.device_id, "linux-test-1");
        assert_eq!(info.http_port, handle.http_port());
        assert!(info.requires_pin);

        // 2. Reject invalid PIN
        let bad_pair = http
            .post(format!("{base}/connect/pair"))
            .json(&PairRequest {
                device_id: "phone-1".to_string(),
                device_name: "Pixel".to_string(),
                pin: "999999".to_string(),
            })
            .send()
            .await
            .expect("bad pair request");
        assert_eq!(bad_pair.status(), reqwest::StatusCode::FORBIDDEN);

        // 3. Accept valid 6-digit PIN and persist 0o600 token file
        let pin = handle.current_pin();
        let ok_pair: PairAck = http
            .post(format!("{base}/connect/pair"))
            .json(&PairRequest {
                device_id: "phone-1".to_string(),
                device_name: "Pixel".to_string(),
                pin,
            })
            .send()
            .await
            .expect("ok pair request")
            .json()
            .await
            .expect("pair ack json");
        assert!(ok_pair.accepted);
        let token = ok_pair.token.expect("issued token");
        assert_eq!(handle.paired_devices().len(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&paired_path)
                .expect("paired metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        // 4. Update and read /connect/state
        handle.update_state(ConnectStateSnapshot {
            playing: true,
            title: Some("Breathe".to_string()),
            artist: Some("The Prodigy".to_string()),
            volume: 70,
            ..ConnectStateSnapshot::default()
        });
        let snapshot: ConnectStateSnapshot = http
            .get(format!("{base}/connect/state"))
            .send()
            .await
            .expect("state request")
            .json()
            .await
            .expect("state json");
        assert!(snapshot.playing);
        assert_eq!(snapshot.title.as_deref(), Some("Breathe"));
        assert_eq!(snapshot.volume, 70);

        // 5. Reject unsigned or forged /connect/command
        let cmd_body = serde_json::to_vec(&ConnectCommand::SetVolume { volume: 55 }).unwrap();
        let unauth = http
            .post(format!("{base}/connect/command"))
            .header("X-Sonata-Device-Id", "phone-1")
            .header(
                "X-Sonata-Signature",
                "0000000000000000000000000000000000000000",
            )
            .body(cmd_body.clone())
            .send()
            .await
            .expect("forged command");
        assert_eq!(unauth.status(), reqwest::StatusCode::UNAUTHORIZED);

        // 6. Accept properly HMAC-SHA1 signed /connect/command
        let sig = sign_payload(&token, &cmd_body);
        let auth_resp = http
            .post(format!("{base}/connect/command"))
            .header("X-Sonata-Device-Id", "phone-1")
            .header("X-Sonata-Signature", sig)
            .body(cmd_body.clone())
            .send()
            .await
            .expect("signed command");
        assert_eq!(auth_resp.status(), reqwest::StatusCode::OK);
        let received = cmd_rx.recv().await.expect("command emitted");
        assert!(matches!(received, ConnectCommand::SetVolume { volume: 55 }));

        // 7. Unpair revokes further commands
        assert!(handle.unpair_device("phone-1").expect("unpair"));
        let sig_after = sign_payload(&token, &cmd_body);
        let revoked = http
            .post(format!("{base}/connect/command"))
            .header("X-Sonata-Device-Id", "phone-1")
            .header("X-Sonata-Signature", sig_after)
            .body(cmd_body)
            .send()
            .await
            .expect("revoked command");
        assert_eq!(revoked.status(), reqwest::StatusCode::UNAUTHORIZED);

        handle.shutdown();
    }

    #[tokio::test]
    async fn concurrent_pair_and_unpair_do_not_clobber_persisted_devices() {
        let dir = TempTestDir::new("connect_concurrent");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-concurrent".to_string(),
            device_name: "Sonata Concurrent".to_string(),
            app_version: "1.0.7".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path.clone(),
        };

        let (handle, _cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();
        let pin = handle.current_pin();

        let mut pair_tasks = Vec::new();
        for idx in 0..8 {
            let client = http.clone();
            let url = format!("{base}/connect/pair");
            let pin_clone = pin.clone();
            pair_tasks.push(tokio::spawn(async move {
                let resp: PairAck = client
                    .post(&url)
                    .json(&PairRequest {
                        device_id: format!("dev-{idx}"),
                        device_name: format!("Device {idx}"),
                        pin: pin_clone,
                    })
                    .send()
                    .await
                    .expect("pair request")
                    .json()
                    .await
                    .expect("pair ack");
                assert!(resp.accepted);
            }));
        }
        for t in pair_tasks {
            t.await.expect("pair task");
        }

        assert!(handle
            .unpair_device_async("dev-0")
            .await
            .expect("unpair dev-0"));
        assert!(handle.unpair_device("dev-1").expect("unpair dev-1"));

        let on_disk = load_paired_devices(&paired_path).expect("load paired devices");
        assert_eq!(on_disk.len(), 6);
        for idx in 2..8 {
            assert!(
                on_disk.iter().any(|d| d.device_id == format!("dev-{idx}")),
                "missing dev-{idx} on disk"
            );
        }

        handle.shutdown();
    }
}
