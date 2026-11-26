//! Linux Sonata Connect LAN discovery broadcaster and authenticated HTTP/JSON control server.
//!
//! 1. **Paired Device Persistence (`PairedDeviceStore`)**: Stores paired client tokens atomically
//!    in `connect_paired.json` using `0o700` private directories and `0o600` temp-file writes.
//! 2. **UDP LAN Discovery (`DISCOVERY_PORT = 45389`)**: Broadcasts `ConnectBeacon` announcements
//!    with live device-name updates so mobile and peer Sonata clients can discover this host.
//! 3. **HTTP/JSON Control Server (`ConnectHandle`)**: Serves `/connect/info`, `/connect/pair`
//!    (6-digit PIN), interactive `/connect/pair/request` + `/connect/pair/status`, `/connect/trust`,
//!    `/connect/state`, and HMAC-SHA1 authenticated `/connect/command` requests.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sonata_core::auth::{create_private_file, ensure_private_dir};
pub use sonata_core::connect::PendingPairRequest;
use sonata_core::connect::{
    constant_time_eq, generate_device_token, generate_pairing_pin, generate_request_id,
    verify_signature, ConnectBeacon, ConnectCommand, ConnectStateSnapshot, PairAck,
    PairApprovalQueued, PairApprovalRequest, PairApprovalStatus, PairRequest, TrustCheckResponse,
    CONNECT_PROTOCOL_VERSION, DEFAULT_CONNECT_HTTP_PORT, DISCOVERY_PORT,
};
use sonata_core::redaction::sensitive_text;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinHandle;

const COMMAND_CHANNEL_CAPACITY: usize = 64;
const MAX_CONCURRENT_HTTP_CONNECTIONS: usize = 32;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(5);
const PIN_MAX_FAILED_ATTEMPTS: u32 = 5;
const PIN_LOCKOUT_DURATION: Duration = Duration::from_secs(60);
const PIN_BACKOFF_BASE_MS: u64 = 50;
const PAIR_REQUEST_TTL: Duration = Duration::from_secs(60);
const PAIR_SWEEP_INTERVAL: Duration = Duration::from_secs(5);
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
            device_id: default_device_id(),
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

fn default_device_id() -> String {
    let os = std::env::consts::OS;
    let host = fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| os.to_string());
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
    format!("sonata-{os}-{sanitized}")
}

#[derive(Default)]
struct PinAttemptState {
    failed_attempts: u32,
    locked_until: Option<Instant>,
}

enum PairOutcome {
    Accepted(PairAck),
    Rejected(PairAck),
    RateLimited(PairAck),
}

enum PairRequestLifecycle {
    Pending,
    Approved {
        token: String,
        decided_at: Instant,
    },
    Rejected {
        message: String,
        decided_at: Instant,
    },
}

struct PairRequestEntry {
    request_id: String,
    device_id: String,
    device_name: String,
    created_at: Instant,
    created_at_unix: u64,
    lifecycle: PairRequestLifecycle,
}

fn normalize_device_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        crate::config::DEFAULT_CONNECT_DEVICE_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

struct ConnectServerState {
    config: ConnectServerConfig,
    device_name: Mutex<String>,
    http_port: u16,
    active_pin: Mutex<String>,
    pin_attempts: Mutex<PinAttemptState>,
    paired_devices: Mutex<HashMap<String, PairedDeviceRecord>>,
    pending_pairs: Mutex<HashMap<String, PairRequestEntry>>,
    pair_requests_tx: watch::Sender<Vec<PendingPairRequest>>,
    persist_lock: tokio::sync::Mutex<()>,
    file_lock: Arc<Mutex<()>>,
    state_rx: watch::Receiver<ConnectStateSnapshot>,
    cmd_tx: mpsc::Sender<ConnectCommand>,
}

impl ConnectServerState {
    fn current_device_name(&self) -> String {
        self.device_name
            .lock()
            .expect("connect device_name lock")
            .clone()
    }

    fn set_device_name(&self, name: String) -> String {
        let normalized = normalize_device_name(&name);
        *self.device_name.lock().expect("connect device_name lock") = normalized.clone();
        normalized
    }

    fn current_pin(&self) -> String {
        self.active_pin.lock().expect("connect pin lock").clone()
    }

    fn purge_and_collect_pending(
        map: &mut HashMap<String, PairRequestEntry>,
        now: Instant,
    ) -> Vec<PendingPairRequest> {
        map.retain(|_, entry| match &entry.lifecycle {
            PairRequestLifecycle::Pending => {
                now.saturating_duration_since(entry.created_at) < PAIR_REQUEST_TTL
            }
            PairRequestLifecycle::Approved { decided_at, .. }
            | PairRequestLifecycle::Rejected { decided_at, .. } => {
                now.saturating_duration_since(*decided_at) < PAIR_REQUEST_TTL
            }
        });
        let mut pending: Vec<PendingPairRequest> = map
            .values()
            .filter(|e| matches!(e.lifecycle, PairRequestLifecycle::Pending))
            .map(|e| PendingPairRequest {
                request_id: e.request_id.clone(),
                device_id: e.device_id.clone(),
                device_name: e.device_name.clone(),
                created_at_unix: e.created_at_unix,
            })
            .collect();
        pending.sort_by(|a, b| {
            a.created_at_unix
                .cmp(&b.created_at_unix)
                .then_with(|| a.request_id.cmp(&b.request_id))
        });
        pending
    }

    fn notify_pending_pairs(&self, next: Vec<PendingPairRequest>) {
        self.pair_requests_tx.send_if_modified(|current| {
            if *current != next {
                *current = next;
                true
            } else {
                false
            }
        });
    }

    fn pending_pair_requests(&self) -> Vec<PendingPairRequest> {
        let pending = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            Self::purge_and_collect_pending(&mut map, Instant::now())
        };
        self.notify_pending_pairs(pending.clone());
        pending
    }

    fn enqueue_pair_request(&self, req: PairApprovalRequest) -> Result<PairApprovalQueued, String> {
        let trimmed_id = req.device_id.trim();
        let trimmed_name = req.device_name.trim();
        if trimmed_id.is_empty() || trimmed_name.is_empty() {
            return Err("device_id and device_name are required".to_string());
        }

        let now = Instant::now();
        let (request_id, pending) = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            let _ = Self::purge_and_collect_pending(&mut map, now);

            let existing_id = map.values_mut().find_map(|entry| {
                if matches!(entry.lifecycle, PairRequestLifecycle::Pending)
                    && entry.device_id == trimmed_id
                {
                    entry.device_name = trimmed_name.to_string();
                    Some(entry.request_id.clone())
                } else {
                    None
                }
            });

            let request_id = if let Some(id) = existing_id {
                id
            } else {
                let id = generate_request_id();
                map.insert(
                    id.clone(),
                    PairRequestEntry {
                        request_id: id.clone(),
                        device_id: trimmed_id.to_string(),
                        device_name: trimmed_name.to_string(),
                        created_at: now,
                        created_at_unix: now_unix_secs(),
                        lifecycle: PairRequestLifecycle::Pending,
                    },
                );
                id
            };
            let pending = Self::purge_and_collect_pending(&mut map, now);
            (request_id, pending)
        };

        self.notify_pending_pairs(pending);
        Ok(PairApprovalQueued {
            request_id,
            status: "pending".to_string(),
            device_id: self.config.device_id.clone(),
            device_name: self.current_device_name(),
        })
    }

    fn poll_pair_status(&self, request_id: &str) -> PairApprovalStatus {
        let host_id = self.config.device_id.clone();
        let host_name = self.current_device_name();
        let key = request_id.trim();
        let now = Instant::now();

        let (status, pending) = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            let _ = Self::purge_and_collect_pending(&mut map, now);

            let outcome = match map.get(key) {
                Some(entry) if matches!(entry.lifecycle, PairRequestLifecycle::Pending) => {
                    PairApprovalStatus {
                        status: "pending".to_string(),
                        accepted: false,
                        device_id: host_id,
                        device_name: host_name,
                        token: None,
                        message: None,
                    }
                }
                Some(_) => match map.remove(key).map(|e| e.lifecycle) {
                    Some(PairRequestLifecycle::Approved { token, .. }) => PairApprovalStatus {
                        status: "approved".to_string(),
                        accepted: true,
                        device_id: host_id,
                        device_name: host_name,
                        token: Some(token),
                        message: None,
                    },
                    Some(PairRequestLifecycle::Rejected { message, .. }) => PairApprovalStatus {
                        status: "rejected".to_string(),
                        accepted: false,
                        device_id: host_id,
                        device_name: host_name,
                        token: None,
                        message: Some(message),
                    },
                    _ => PairApprovalStatus {
                        status: "rejected".to_string(),
                        accepted: false,
                        device_id: host_id,
                        device_name: host_name,
                        token: None,
                        message: Some("expired_or_unknown".to_string()),
                    },
                },
                None => PairApprovalStatus {
                    status: "rejected".to_string(),
                    accepted: false,
                    device_id: host_id,
                    device_name: host_name,
                    token: None,
                    message: Some("expired_or_unknown".to_string()),
                },
            };
            let pending = Self::purge_and_collect_pending(&mut map, now);
            (outcome, pending)
        };

        self.notify_pending_pairs(pending);
        status
    }

    fn begin_approve_pair(&self, request_id: &str) -> Option<(String, String, String)> {
        let key = request_id.trim();
        let now = Instant::now();
        let (result, pending) = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            let _ = Self::purge_and_collect_pending(&mut map, now);
            let found = map.get(key).and_then(|entry| {
                matches!(entry.lifecycle, PairRequestLifecycle::Pending)
                    .then(|| (entry.device_id.clone(), entry.device_name.clone()))
            });
            let pending = Self::purge_and_collect_pending(&mut map, now);
            (found, pending)
        };
        if result.is_none() {
            self.notify_pending_pairs(pending);
            return None;
        }
        let (device_id, device_name) = result?;
        let token = generate_device_token();
        let record = PairedDeviceRecord {
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            token: token.clone(),
            paired_at_unix: now_unix_secs(),
        };
        self.paired_devices
            .lock()
            .expect("paired devices lock")
            .insert(device_id.clone(), record);
        Some((device_id, device_name, token))
    }

    fn finalize_approve_pair(&self, request_id: &str, token: String) -> bool {
        let key = request_id.trim();
        let now = Instant::now();
        let (approved, pending) = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            let approved = if let Some(entry) = map.get_mut(key) {
                if matches!(entry.lifecycle, PairRequestLifecycle::Pending) {
                    entry.lifecycle = PairRequestLifecycle::Approved {
                        token,
                        decided_at: now,
                    };
                    true
                } else {
                    false
                }
            } else {
                false
            };
            let pending = Self::purge_and_collect_pending(&mut map, now);
            (approved, pending)
        };
        self.notify_pending_pairs(pending);
        approved
    }

    fn reject_pair_request(&self, request_id: &str) -> bool {
        let key = request_id.trim();
        let now = Instant::now();
        let (rejected, pending) = {
            let mut map = self
                .pending_pairs
                .lock()
                .expect("connect pending pairs lock");
            let _ = Self::purge_and_collect_pending(&mut map, now);
            let rejected = if let Some(entry) = map.get_mut(key) {
                if matches!(entry.lifecycle, PairRequestLifecycle::Pending) {
                    entry.lifecycle = PairRequestLifecycle::Rejected {
                        message: "rejected".to_string(),
                        decided_at: now,
                    };
                    true
                } else {
                    false
                }
            } else {
                false
            };
            let pending = Self::purge_and_collect_pending(&mut map, now);
            (rejected, pending)
        };
        self.notify_pending_pairs(pending);
        rejected
    }

    fn check_trust(&self, headers: &HashMap<String, String>) -> bool {
        let device_id = headers
            .get("x-sonata-device-id")
            .map(String::as_str)
            .unwrap_or_default();
        let signature = headers
            .get("x-sonata-signature")
            .map(String::as_str)
            .unwrap_or_default();
        let Some(token) = self.token_for_device(device_id) else {
            return false;
        };
        if let Some(timestamp) = headers
            .get("x-sonata-timestamp")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            verify_signature(&token, timestamp.as_bytes(), signature)
                || verify_signature(&token, b"", signature)
        } else {
            verify_signature(&token, b"", signature)
        }
    }

    fn rotate_pin_distinct(&self) -> String {
        let mut pin_guard = self.active_pin.lock().expect("connect pin lock");
        let prev = pin_guard.clone();
        let mut next = generate_pairing_pin();
        while next == prev {
            next = generate_pairing_pin();
        }
        *pin_guard = next.clone();
        next
    }

    fn regenerate_pin(&self) -> String {
        let next = self.rotate_pin_distinct();
        if let Ok(mut attempts) = self.pin_attempts.lock() {
            attempts.failed_attempts = 0;
            attempts.locked_until = None;
        }
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

    async fn verify_and_record_pair(self: &Arc<Self>, req: PairRequest) -> Result<PairOutcome> {
        {
            let mut attempts = self.pin_attempts.lock().expect("connect pin attempts lock");
            if let Some(until) = attempts.locked_until {
                if Instant::now() < until {
                    return Ok(PairOutcome::RateLimited(PairAck {
                        accepted: false,
                        device_id: self.config.device_id.clone(),
                        device_name: self.current_device_name(),
                        token: None,
                        message: Some("pairing_locked".to_string()),
                    }));
                }
                attempts.locked_until = None;
                attempts.failed_attempts = 0;
            }
        }

        let trimmed_id = req.device_id.trim();
        let trimmed_name = req.device_name.trim();
        if trimmed_id.is_empty() || trimmed_name.is_empty() {
            return Ok(PairOutcome::Rejected(PairAck {
                accepted: false,
                device_id: self.config.device_id.clone(),
                device_name: self.current_device_name(),
                token: None,
                message: Some("invalid_request".to_string()),
            }));
        }

        let expected_pin = self.current_pin();
        if !constant_time_eq(expected_pin.as_bytes(), req.pin.trim().as_bytes()) {
            let (locked_now, backoff_ms) = {
                let mut attempts = self.pin_attempts.lock().expect("connect pin attempts lock");
                attempts.failed_attempts = attempts.failed_attempts.saturating_add(1);
                if attempts.failed_attempts >= PIN_MAX_FAILED_ATTEMPTS {
                    attempts.failed_attempts = 0;
                    attempts.locked_until = Some(Instant::now() + PIN_LOCKOUT_DURATION);
                    (true, 0u64)
                } else {
                    let shift = attempts.failed_attempts.saturating_sub(1).min(4);
                    (false, PIN_BACKOFF_BASE_MS.saturating_mul(1u64 << shift))
                }
            };
            if locked_now {
                let _ = self.rotate_pin_distinct();
                tracing::warn!(
                    event = "sonata_connect_pin_locked",
                    lockout_secs = PIN_LOCKOUT_DURATION.as_secs(),
                    "Sonata Connect PIN rotated and pairing locked after repeated failed attempts"
                );
                return Ok(PairOutcome::RateLimited(PairAck {
                    accepted: false,
                    device_id: self.config.device_id.clone(),
                    device_name: self.current_device_name(),
                    token: None,
                    message: Some("pin_locked_rotated".to_string()),
                }));
            }
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            return Ok(PairOutcome::Rejected(PairAck {
                accepted: false,
                device_id: self.config.device_id.clone(),
                device_name: self.current_device_name(),
                token: None,
                message: Some("invalid_pin".to_string()),
            }));
        }

        if let Ok(mut attempts) = self.pin_attempts.lock() {
            attempts.failed_attempts = 0;
            attempts.locked_until = None;
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

        Ok(PairOutcome::Accepted(PairAck {
            accepted: true,
            device_id: self.config.device_id.clone(),
            device_name: self.current_device_name(),
            token: Some(token),
            message: None,
        }))
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
        let paired_store_path = config.paired_store_path.clone();
        let loaded_devices =
            tokio::task::spawn_blocking(move || load_paired_devices(&paired_store_path))
                .await
                .context("load_paired_devices task panicked")?
                .unwrap_or_default()
                .into_iter()
                .map(|rec| (rec.device_id.clone(), rec))
                .collect::<HashMap<_, _>>();

        let listener = if config.http_bind_addr.port() == 0 && config.enable_udp_beacon {
            let preferred_addr =
                SocketAddr::new(config.http_bind_addr.ip(), DEFAULT_CONNECT_HTTP_PORT);
            match TcpListener::bind(preferred_addr).await {
                Ok(listener) => listener,
                Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                    tracing::warn!(
                        port = DEFAULT_CONNECT_HTTP_PORT,
                        "Default Sonata Connect TCP port is already in use; falling back to ephemeral port"
                    );
                    TcpListener::bind(config.http_bind_addr)
                        .await
                        .with_context(|| {
                            format!(
                                "failed to bind Sonata Connect HTTP listener on {}",
                                config.http_bind_addr
                            )
                        })?
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to bind Sonata Connect HTTP listener on {preferred_addr}")
                    });
                }
            }
        } else {
            TcpListener::bind(config.http_bind_addr)
                .await
                .with_context(|| {
                    format!(
                        "failed to bind Sonata Connect HTTP listener on {}",
                        config.http_bind_addr
                    )
                })?
        };
        let local_addr = listener.local_addr()?;
        let http_port = local_addr.port();

        let initial_name = normalize_device_name(&config.device_name);
        let initial_snapshot = ConnectStateSnapshot {
            device_id: config.device_id.clone(),
            device_name: initial_name.clone(),
            updated_at_unix: now_unix_secs(),
            ..ConnectStateSnapshot::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_snapshot);
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (pair_requests_tx, _pair_requests_rx) = watch::channel(Vec::new());

        let shared = Arc::new(ConnectServerState {
            config: config.clone(),
            device_name: Mutex::new(initial_name),
            http_port,
            active_pin: Mutex::new(generate_pairing_pin()),
            pin_attempts: Mutex::new(PinAttemptState::default()),
            paired_devices: Mutex::new(loaded_devices),
            pending_pairs: Mutex::new(HashMap::new()),
            pair_requests_tx,
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

        let sweep_state = Arc::clone(&shared);
        tasks.push(tokio::spawn(async move {
            run_pair_expiry_loop(sweep_state).await;
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

    /// Updates the advertised Sonata Connect device name live without restarting or rotating PIN.
    pub fn set_device_name(&self, name: String) {
        let normalized = self.shared.set_device_name(name);
        self.state_tx.send_modify(|snap| {
            snap.device_name = normalized;
            snap.updated_at_unix = now_unix_secs();
        });
    }

    /// Publishes an updated playback snapshot returned by `GET /connect/state`.
    pub fn update_state(&self, mut snapshot: ConnectStateSnapshot) {
        if snapshot.device_id.is_empty() {
            snapshot.device_id = self.shared.config.device_id.clone();
        }
        if snapshot.device_name.is_empty() {
            snapshot.device_name = self.shared.current_device_name();
        }
        snapshot.updated_at_unix = now_unix_secs();
        self.state_tx.send_replace(snapshot);
    }

    /// Lists currently pending interactive pairing requests, purging expired entries first.
    pub fn pending_pair_requests(&self) -> Vec<PendingPairRequest> {
        self.shared.pending_pair_requests()
    }

    /// Subscribes to live changes in pending interactive pairing requests.
    pub fn subscribe_pair_requests(&self) -> watch::Receiver<Vec<PendingPairRequest>> {
        self.shared.pair_requests_tx.subscribe()
    }

    /// Approves a pending interactive pairing request, persisting the device token (`0o600`).
    pub fn approve_pair_request(&self, request_id: &str) -> Result<bool> {
        let Some((_dev_id, _dev_name, token)) = self.shared.begin_approve_pair(request_id) else {
            return Ok(false);
        };
        let persist = || {
            ConnectServerState::snapshot_and_save_sync(
                &self.shared.paired_devices,
                &self.shared.file_lock,
                &self.shared.config.paired_store_path,
            )
        };
        if tokio::runtime::Handle::try_current()
            .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
        {
            tokio::task::block_in_place(persist)?;
        } else {
            persist()?;
        }
        Ok(self.shared.finalize_approve_pair(request_id, token))
    }

    /// Async variant of [`Self::approve_pair_request`] serialized via the async persistence mutex.
    pub async fn approve_pair_request_async(&self, request_id: &str) -> Result<bool> {
        let Some((_dev_id, _dev_name, token)) = self.shared.begin_approve_pair(request_id) else {
            return Ok(false);
        };
        self.shared.persist_current_devices_async().await?;
        Ok(self.shared.finalize_approve_pair(request_id, token))
    }

    /// Rejects a pending interactive pairing request.
    pub fn reject_pair_request(&self, request_id: &str) -> bool {
        self.shared.reject_pair_request(request_id)
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
            let persist = || {
                ConnectServerState::snapshot_and_save_sync(
                    &self.shared.paired_devices,
                    &self.shared.file_lock,
                    &self.shared.config.paired_store_path,
                )
            };
            if tokio::runtime::Handle::try_current()
                .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
            {
                tokio::task::block_in_place(persist)?;
            } else {
                persist()?;
            }
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

async fn run_pair_expiry_loop(state: Arc<ConnectServerState>) {
    let mut ticker = tokio::time::interval(PAIR_SWEEP_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let _ = state.pending_pair_requests();
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
    let mut cached_name = String::new();
    let mut payload = Vec::new();

    let mut ticker = tokio::time::interval(state.config.beacon_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let current_name = state.current_device_name();
        if payload.is_empty() || current_name != cached_name {
            let beacon = ConnectBeacon {
                device_id: state.config.device_id.clone(),
                device_name: current_name.clone(),
                platform: std::env::consts::OS.to_string(),
                http_port: state.http_port,
                protocol_version: CONNECT_PROTOCOL_VERSION,
                app_version: state.config.app_version.clone(),
            };
            let Ok(encoded) = serde_json::to_vec(&beacon) else {
                continue;
            };
            cached_name = current_name;
            payload = encoded;
        }
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
    let connection_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_HTTP_CONNECTIONS));
    loop {
        let Ok(permit) = Arc::clone(&connection_slots).acquire_owned().await else {
            break;
        };
        let (stream, _peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                drop(permit);
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
            let _permit = permit;
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

fn is_sensitive_connect_query_param(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "u" | "t"
            | "s"
            | "p"
            | "token"
            | "api_key"
            | "apikey"
            | "x-emby-token"
            | "x-mediabrowser-token"
            | "access_token"
            | "sk"
            | "api_sig"
            | "api_secret"
            | "session_key"
    )
}

fn redact_connect_thumbnail_url(raw: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw) else {
        return raw.to_string();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return raw.to_string();
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    if url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !is_sensitive_connect_query_param(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    url.to_string()
}

fn sanitize_connect_state_snapshot(mut snapshot: ConnectStateSnapshot) -> ConnectStateSnapshot {
    if let Some(url) = snapshot.thumbnail_url.take() {
        snapshot.thumbnail_url = Some(redact_connect_thumbnail_url(&url));
    }
    for track in &mut snapshot.queue {
        if let Some(url) = track.thumbnail_url.take() {
            track.thumbnail_url = Some(redact_connect_thumbnail_url(&url));
        }
    }
    snapshot
}

struct ParsedHttpRequest {
    method: String,
    path: String,
    query_params: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn percent_decode_query_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
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
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query_params(raw_query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        params.insert(
            percent_decode_query_component(k),
            percent_decode_query_component(v),
        );
    }
    params
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
    let (path, query_params) = match raw_target.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_query_params(q)),
        None => (raw_target.to_string(), HashMap::new()),
    };

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
        query_params,
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
                device_name: state.current_device_name(),
                platform: std::env::consts::OS.to_string(),
                protocol_version: CONNECT_PROTOCOL_VERSION,
                http_port: state.http_port,
                requires_pin: true,
            };
            ("200 OK", serde_json::to_vec(&info)?)
        }
        ("POST", "/connect/pair/request") => {
            match serde_json::from_slice::<PairApprovalRequest>(&req.body) {
                Ok(pair_req) => match state.enqueue_pair_request(pair_req) {
                    Ok(queued) => ("202 Accepted", serde_json::to_vec(&queued)?),
                    Err(msg) => (
                        "400 Bad Request",
                        serde_json::to_vec(&serde_json::json!({ "error": msg }))?,
                    ),
                },
                Err(err) => (
                    "400 Bad Request",
                    serde_json::to_vec(&serde_json::json!({ "error": err.to_string() }))?,
                ),
            }
        }
        ("GET", "/connect/pair/status") => {
            let request_id = req
                .query_params
                .get("request_id")
                .map(String::as_str)
                .unwrap_or_default();
            let status = state.poll_pair_status(request_id);
            ("200 OK", serde_json::to_vec(&status)?)
        }
        ("GET", "/connect/trust") => {
            let trusted = state.check_trust(&req.headers);
            let status = if trusted {
                "200 OK"
            } else {
                "401 Unauthorized"
            };
            (status, serde_json::to_vec(&TrustCheckResponse { trusted })?)
        }
        ("POST", "/connect/pair") => match serde_json::from_slice::<PairRequest>(&req.body) {
            Ok(pair_req) => match state.verify_and_record_pair(pair_req).await {
                Ok(PairOutcome::Accepted(ack)) => ("200 OK", serde_json::to_vec(&ack)?),
                Ok(PairOutcome::Rejected(ack)) => ("403 Forbidden", serde_json::to_vec(&ack)?),
                Ok(PairOutcome::RateLimited(ack)) => {
                    ("429 Too Many Requests", serde_json::to_vec(&ack)?)
                }
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
            let snapshot = sanitize_connect_state_snapshot(state.state_rx.borrow().clone());
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

    #[tokio::test]
    async fn locks_out_and_rotates_pin_after_repeated_failed_pair_attempts() {
        let dir = TempTestDir::new("connect_pin_lockout");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-lockout".to_string(),
            device_name: "Sonata Lockout".to_string(),
            app_version: "1.0.9".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path,
        };

        let (handle, _cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();

        let initial_pin = handle.current_pin();
        let wrong_pin = if initial_pin == "000000" {
            "111111"
        } else {
            "000000"
        };

        for attempt in 1..PIN_MAX_FAILED_ATTEMPTS {
            let resp = http
                .post(format!("{base}/connect/pair"))
                .json(&PairRequest {
                    device_id: "attacker".to_string(),
                    device_name: "Brute".to_string(),
                    pin: wrong_pin.to_string(),
                })
                .send()
                .await
                .expect("pair attempt");
            assert_eq!(
                resp.status(),
                reqwest::StatusCode::FORBIDDEN,
                "attempt {attempt} should return 403"
            );
        }

        // 5th failure triggers lockout (429) and automatic PIN rotation.
        let lock_resp = http
            .post(format!("{base}/connect/pair"))
            .json(&PairRequest {
                device_id: "attacker".to_string(),
                device_name: "Brute".to_string(),
                pin: wrong_pin.to_string(),
            })
            .send()
            .await
            .expect("lockout attempt");
        assert_eq!(lock_resp.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
        let rotated_pin = handle.current_pin();
        assert_ne!(rotated_pin, initial_pin);

        // Even the newly rotated PIN is rejected while locked out.
        let during_lock = http
            .post(format!("{base}/connect/pair"))
            .json(&PairRequest {
                device_id: "phone-1".to_string(),
                device_name: "Pixel".to_string(),
                pin: rotated_pin,
            })
            .send()
            .await
            .expect("during lockout");
        assert_eq!(during_lock.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

        // Explicit user PIN regeneration clears lockout and allows pairing.
        let fresh_pin = handle.regenerate_pin();
        let ok_pair: PairAck = http
            .post(format!("{base}/connect/pair"))
            .json(&PairRequest {
                device_id: "phone-1".to_string(),
                device_name: "Pixel".to_string(),
                pin: fresh_pin,
            })
            .send()
            .await
            .expect("post-regenerate pair")
            .json()
            .await
            .expect("pair ack");
        assert!(ok_pair.accepted);

        handle.shutdown();
    }

    #[tokio::test]
    async fn redacts_sensitive_query_params_in_connect_state_thumbnails() {
        use sonata_core::providers::{ProviderSource, Track};

        let dir = TempTestDir::new("connect_state_redact");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-redact".to_string(),
            device_name: "Sonata Redact".to_string(),
            app_version: "1.0.9".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path,
        };

        let (handle, _cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();

        handle.update_state(ConnectStateSnapshot {
            playing: true,
            title: Some("Geogaddi".to_string()),
            thumbnail_url: Some(
                "https://alice:pw@sub.local/rest/getCoverArt.view?id=al-1&u=alice&t=deadbeef&s=salt123&p=clearpass&size=600"
                    .to_string(),
            ),
            queue: vec![Track {
                id: "jellyfin:jf-1:trk-1".to_string(),
                title: "Intro".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                duration_secs: Some(120),
                thumbnail_url: Some(
                    "https://jf.local/Items/1/Images/Primary?maxWidth=800&api_key=jf-secret&X-Emby-Token=emby-secret&token=tok-secret"
                        .to_string(),
                ),
                source: ProviderSource::YouTubeMusic,
                audio_badge: None,
                set_video_id: None,
                liked: false,
                album_id: None,
                is_explicit: false,
            }],
            ..ConnectStateSnapshot::default()
        });

        let body = http
            .get(format!("{base}/connect/state"))
            .send()
            .await
            .expect("state request")
            .text()
            .await
            .expect("state text");

        for secret in [
            "alice",
            "pw@",
            "deadbeef",
            "salt123",
            "clearpass",
            "jf-secret",
            "emby-secret",
            "tok-secret",
        ] {
            assert!(
                !body.contains(secret),
                "/connect/state leaked `{secret}` in `{body}`"
            );
        }

        let snapshot: ConnectStateSnapshot = serde_json::from_str(&body).expect("parse state");
        let top_thumb = snapshot.thumbnail_url.as_deref().expect("top thumb");
        assert!(top_thumb.contains("id=al-1"));
        assert!(top_thumb.contains("size=600"));
        let queue_thumb = snapshot.queue[0]
            .thumbnail_url
            .as_deref()
            .expect("queue thumb");
        assert!(queue_thumb.contains("maxWidth=800"));

        handle.shutdown();
    }

    #[tokio::test]
    async fn set_device_name_updates_endpoints_without_changing_pin() {
        let dir = TempTestDir::new("connect_rename");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-rename-1".to_string(),
            device_name: "Initial Desk".to_string(),
            app_version: "1.0.9".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path,
        };

        let (handle, _cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();
        let initial_pin = handle.current_pin();

        handle.set_device_name("Studio Linux".to_string());
        assert_eq!(handle.current_pin(), initial_pin);

        let info: ConnectInfoResponse = http
            .get(format!("{base}/connect/info"))
            .send()
            .await
            .expect("info")
            .json()
            .await
            .expect("info json");
        assert_eq!(info.device_name, "Studio Linux");

        let state: ConnectStateSnapshot = http
            .get(format!("{base}/connect/state"))
            .send()
            .await
            .expect("state")
            .json()
            .await
            .expect("state json");
        assert_eq!(state.device_name, "Studio Linux");
        assert_eq!(handle.current_pin(), initial_pin);

        handle.shutdown();
    }

    #[tokio::test]
    async fn interactive_pair_approval_rejection_and_trust_endpoint_flow() {
        let dir = TempTestDir::new("connect_interactive_pair");
        let paired_path = dir.path().join("connect_paired.json");

        let cfg = ConnectServerConfig {
            device_id: "linux-host-1".to_string(),
            device_name: "Test Linux".to_string(),
            app_version: "1.0.9".to_string(),
            http_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            discovery_port: DISCOVERY_PORT,
            enable_udp_beacon: false,
            beacon_interval: Duration::from_secs(10),
            paired_store_path: paired_path.clone(),
        };

        let (handle, _cmd_rx) = ConnectHandle::start(cfg).await.expect("start connect");
        let base = format!("http://127.0.0.1:{}", handle.http_port());
        let http = reqwest::Client::new();

        // 1. POST /connect/pair/request returns 202 Accepted and surfaces pending request
        let resp = http
            .post(format!("{base}/connect/pair/request"))
            .json(&PairApprovalRequest {
                device_id: "android-1".to_string(),
                device_name: "Test Phone".to_string(),
            })
            .send()
            .await
            .expect("pair request");
        assert_eq!(resp.status(), reqwest::StatusCode::ACCEPTED);
        let queued: PairApprovalQueued = resp.json().await.expect("queued json");
        assert_eq!(queued.status, "pending");
        assert_eq!(queued.device_id, "linux-host-1");
        assert_eq!(queued.device_name, "Test Linux");

        // Duplicate request from same device reuses the existing pending request_id
        let dup_queued: PairApprovalQueued = http
            .post(format!("{base}/connect/pair/request"))
            .json(&PairApprovalRequest {
                device_id: "android-1".to_string(),
                device_name: "Test Phone Renamed".to_string(),
            })
            .send()
            .await
            .expect("dup pair request")
            .json()
            .await
            .expect("dup json");
        assert_eq!(dup_queued.request_id, queued.request_id);

        let pending = handle.pending_pair_requests();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].request_id, queued.request_id);
        assert_eq!(pending[0].device_name, "Test Phone Renamed");

        // 2. Poll while pending
        let status_pending: PairApprovalStatus = http
            .get(format!(
                "{base}/connect/pair/status?request_id={}",
                queued.request_id
            ))
            .send()
            .await
            .expect("poll pending")
            .json()
            .await
            .expect("pending json");
        assert_eq!(status_pending.status, "pending");
        assert!(!status_pending.accepted);
        assert!(status_pending.token.is_none());

        // 3. Approve on host -> persists 0o600, returns one-time token on next poll
        assert!(handle
            .approve_pair_request(&queued.request_id)
            .expect("approve"));
        assert!(handle.pending_pair_requests().is_empty());

        let status_approved: PairApprovalStatus = http
            .get(format!(
                "{base}/connect/pair/status?request_id={}",
                queued.request_id
            ))
            .send()
            .await
            .expect("poll approved")
            .json()
            .await
            .expect("approved json");
        assert_eq!(status_approved.status, "approved");
        assert!(status_approved.accepted);
        let token = status_approved.token.expect("issued token");
        assert_eq!(token.len(), 64);

        // Second poll after retrieval does NOT replay token
        let status_replay: PairApprovalStatus = http
            .get(format!(
                "{base}/connect/pair/status?request_id={}",
                queued.request_id
            ))
            .send()
            .await
            .expect("poll replay")
            .json()
            .await
            .expect("replay json");
        assert_eq!(status_replay.status, "rejected");
        assert!(!status_replay.accepted);
        assert!(status_replay.token.is_none());

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

        // 4. GET /connect/trust accepts signature over X-Sonata-Timestamp and empty body
        let ts = "1700000000";
        let ts_sig = sign_payload(&token, ts.as_bytes());
        let trust_ts = http
            .get(format!("{base}/connect/trust"))
            .header("X-Sonata-Device-Id", "android-1")
            .header("X-Sonata-Timestamp", ts)
            .header("X-Sonata-Signature", ts_sig)
            .send()
            .await
            .expect("trust ts");
        assert_eq!(trust_ts.status(), reqwest::StatusCode::OK);
        let trust_body: TrustCheckResponse = trust_ts.json().await.expect("trust json");
        assert!(trust_body.trusted);

        let empty_sig = sign_payload(&token, b"");
        let trust_empty = http
            .get(format!("{base}/connect/trust"))
            .header("X-Sonata-Device-Id", "android-1")
            .header("X-Sonata-Signature", empty_sig.clone())
            .send()
            .await
            .expect("trust empty");
        assert_eq!(trust_empty.status(), reqwest::StatusCode::OK);

        // Unpair revokes trust
        assert!(handle.unpair_device("android-1").expect("unpair"));
        let trust_revoked = http
            .get(format!("{base}/connect/trust"))
            .header("X-Sonata-Device-Id", "android-1")
            .header("X-Sonata-Signature", empty_sig)
            .send()
            .await
            .expect("trust revoked");
        assert_eq!(trust_revoked.status(), reqwest::StatusCode::UNAUTHORIZED);

        // 5. Reject flow
        let rej_queued: PairApprovalQueued = http
            .post(format!("{base}/connect/pair/request"))
            .json(&PairApprovalRequest {
                device_id: "android-2".to_string(),
                device_name: "Other Phone".to_string(),
            })
            .send()
            .await
            .expect("rej request")
            .json()
            .await
            .expect("rej queued");
        assert!(handle.reject_pair_request(&rej_queued.request_id));
        let rej_status: PairApprovalStatus = http
            .get(format!(
                "{base}/connect/pair/status?request_id={}",
                rej_queued.request_id
            ))
            .send()
            .await
            .expect("rej status")
            .json()
            .await
            .expect("rej status json");
        assert_eq!(rej_status.status, "rejected");
        assert!(!rej_status.accepted);
        assert!(rej_status.token.is_none());

        handle.shutdown();
    }
}
