//! Portable Sonata Connect LAN discovery, pairing, state snapshot, and command signing types.
//!
//! 1. **Discovery & Pairing (`ConnectBeacon`, `PairRequest`, `PairAck`, `PendingPairRequest`)**: Defines UDP port `45389`
//!    broadcast payloads, default TCP control port `45390`, 6-digit PIN generation/verification, and interactive host-approval pairing.
//! 2. **Remote State & Commands (`ConnectStateSnapshot`, `ConnectCommand`)**: Portable models for
//!    mirroring playback state and sending transport or queue-transfer commands.
//! 3. **HMAC-SHA1 Signing (`sign_payload`, `verify_signature`, `sign_request`)**: Authenticates control requests
//!    with timestamp/nonce replay protection and constant-time comparison so forged or replayed LAN packets are rejected.

use crate::player::RepeatMode;
use crate::providers::Track;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// UDP LAN broadcast discovery port for Sonata Connect.
pub const DISCOVERY_PORT: u16 = 45389;

/// Default TCP HTTP/JSON control port for Sonata Connect (falls back to an ephemeral port if busy).
pub const DEFAULT_CONNECT_HTTP_PORT: u16 = 45390;

/// Current Sonata Connect wire protocol version.
pub const CONNECT_PROTOCOL_VERSION: u16 = 1;

/// Maximum permitted timestamp skew (in seconds) for replay-protected Sonata Connect requests.
pub const CONNECT_MAX_CLOCK_SKEW_SECS: u64 = 60;

/// UDP broadcast beacon announcing a reachable Sonata Connect instance on the LAN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectBeacon {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub http_port: u16,
    pub protocol_version: u16,
    pub app_version: String,
}

/// Request sent to `/connect/pair` with the user-entered 6-digit PIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRequest {
    #[serde(alias = "client_device_id")]
    pub device_id: String,
    #[serde(alias = "client_device_name")]
    pub device_name: String,
    pub pin: String,
}

/// Response returned by `/connect/pair` containing the shared HMAC token on success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairAck {
    pub accepted: bool,
    pub device_id: String,
    pub device_name: String,
    pub token: Option<String>,
    pub message: Option<String>,
}

/// Request sent to `POST /connect/pair/request` to trigger interactive host approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairApprovalRequest {
    #[serde(alias = "client_device_id")]
    pub device_id: String,
    #[serde(alias = "client_device_name")]
    pub device_name: String,
}

/// Pending interactive pairing request surfaced to the host UI for approval or rejection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PendingPairRequest {
    pub request_id: String,
    pub device_id: String,
    pub device_name: String,
    pub created_at_unix: u64,
}

/// Immediate `202 Accepted` response returned by `POST /connect/pair/request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairApprovalQueued {
    pub request_id: String,
    pub status: String,
    pub device_id: String,
    pub device_name: String,
}

/// Polling response returned by `GET /connect/pair/status?request_id=<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairApprovalStatus {
    pub status: String,
    pub accepted: bool,
    pub device_id: String,
    pub device_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response returned by `GET /connect/trust`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustCheckResponse {
    pub trusted: bool,
}

/// Serialized playback snapshot served by `/connect/state`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectStateSnapshot {
    pub device_id: String,
    pub device_name: String,
    pub playing: bool,
    pub paused: bool,
    pub track_id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub thumbnail_url: Option<String>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub updated_at_unix: u64,
}

/// Remote transport and queue-handoff commands accepted by `/connect/command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ConnectCommand {
    Play,
    Pause,
    Next,
    Prev,
    SeekAbsolute {
        position_secs: f64,
    },
    SetVolume {
        volume: u8,
    },
    TransferQueue {
        tracks: Vec<Track>,
        start_index: usize,
        #[serde(default)]
        position_secs: f64,
    },
}

/// Constant-time byte-slice comparison avoiding early-exit timing side channels.
pub fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    let max_len = lhs.len().max(rhs.len());
    let mut diff = lhs.len() ^ rhs.len();
    for i in 0..max_len {
        let a = *lhs.get(i).unwrap_or(&0);
        let b = *rhs.get(i).unwrap_or(&0);
        diff |= usize::from(a ^ b);
    }
    diff == 0
}

/// Generates a random 6-digit zero-padded pairing PIN (`"000000"`..`"999999"`).
pub fn generate_pairing_pin() -> String {
    let val = random_u64() % 1_000_000;
    format!("{val:06}")
}

/// Generates a random 32-byte (64-char hex) device pairing token.
pub fn generate_device_token() -> String {
    let mut bytes = [0u8; 32];
    fill_entropy(&mut bytes);
    encode_hex(&bytes)
}

/// Generates a random 16-byte (32-char hex) interactive pairing request identifier.
pub fn generate_request_id() -> String {
    let mut bytes = [0u8; 16];
    fill_entropy(&mut bytes);
    encode_hex(&bytes)
}

/// Generates a random 16-byte (32-char hex) per-request replay protection nonce.
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    fill_entropy(&mut bytes);
    encode_hex(&bytes)
}

/// Parses a Unix timestamp header expressed in either seconds or milliseconds into seconds.
pub fn parse_timestamp_unix_secs(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = trimmed.parse::<u64>().ok()?;
    if parsed == 0 {
        return None;
    }
    if parsed >= 1_000_000_000_000 {
        Some(parsed / 1_000)
    } else {
        Some(parsed)
    }
}

/// Constructs the canonical signing payload `{METHOD}\n{PATH}\n{TIMESTAMP}\n{NONCE}\n{BODY}`.
pub fn build_canonical_request_payload(
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> Vec<u8> {
    let clean_method = method.trim().to_ascii_uppercase();
    let clean_path = path.trim();
    let clean_ts = timestamp.trim();
    let clean_nonce = nonce.trim();
    let header_len = clean_method.len() + clean_path.len() + clean_ts.len() + clean_nonce.len() + 4;
    let mut out = Vec::with_capacity(header_len + body.len());
    out.extend_from_slice(clean_method.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(clean_path.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(clean_ts.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(clean_nonce.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(body);
    out
}

/// Computes lowercase hex HMAC-SHA1 over canonical Sonata Connect request metadata and body.
pub fn sign_request(
    secret: &str,
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    let canonical = build_canonical_request_payload(method, path, timestamp, nonce, body);
    sign_payload(secret, &canonical)
}

/// Verifies a hex-encoded HMAC-SHA1 signature over canonical Sonata Connect request metadata and body.
pub fn verify_request_signature(
    secret: &str,
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
    signature_hex: &str,
) -> bool {
    let canonical = build_canonical_request_payload(method, path, timestamp, nonce, body);
    verify_signature(secret, &canonical, signature_hex)
}

/// Computes lowercase hex HMAC-SHA1 over `payload` using `secret`.
pub fn sign_payload(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha1::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(payload);
    let result = mac.finalize().into_bytes();
    encode_hex(&result)
}

/// Verifies a hex-encoded HMAC-SHA1 signature in constant time.
pub fn verify_signature(secret: &str, payload: &[u8], signature_hex: &str) -> bool {
    let trimmed = signature_hex.trim();
    if trimmed.len() != 40 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let expected = sign_payload(secret, payload);
    let provided = trimmed.to_ascii_lowercase();
    constant_time_eq(expected.as_bytes(), provided.as_bytes())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn fill_entropy(buf: &mut [u8]) {
    getrandom::fill(buf).expect("OS CSPRNG unavailable");
}

fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    fill_entropy(&mut bytes);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    #[test]
    fn generates_valid_6_digit_pin_and_token() {
        for _ in 0..32 {
            let pin = generate_pairing_pin();
            assert_eq!(pin.len(), 6);
            assert!(pin.chars().all(|c| c.is_ascii_digit()));
        }
        let tok_a = generate_device_token();
        let tok_b = generate_device_token();
        assert_eq!(tok_a.len(), 64);
        assert_ne!(tok_a, tok_b);
        let req_a = generate_request_id();
        let req_b = generate_request_id();
        assert_eq!(req_a.len(), 32);
        assert_ne!(req_a, req_b);
    }

    #[test]
    fn signs_and_verifies_payload_with_constant_time_compare() {
        let secret = "connect-shared-secret";
        let payload = br#"{"command":"set_volume","volume":65}"#;
        let sig = sign_payload(secret, payload);
        assert_eq!(sig.len(), 40);
        assert!(verify_signature(secret, payload, &sig));
        assert!(verify_signature(secret, payload, &sig.to_ascii_uppercase()));
        assert!(!verify_signature("wrong-secret", payload, &sig));
        assert!(!verify_signature(secret, b"tampered", &sig));
        assert!(!verify_signature(secret, payload, "not-hex"));
        assert!(!verify_signature(secret, payload, ""));
    }

    #[test]
    fn signs_verifies_canonical_requests_and_parses_timestamps() {
        let secret = "connect-shared-secret";
        let nonce = generate_nonce();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.bytes().all(|b| b.is_ascii_hexdigit()));

        let body = br#"{"command":"next"}"#;
        let sig = sign_request(
            secret,
            "POST",
            "/connect/command",
            "1700000000",
            &nonce,
            body,
        );
        assert!(verify_request_signature(
            secret,
            "post",
            "/connect/command",
            "1700000000",
            &nonce,
            body,
            &sig,
        ));
        assert!(!verify_request_signature(
            secret,
            "GET",
            "/connect/command",
            "1700000000",
            &nonce,
            body,
            &sig,
        ));
        assert!(!verify_request_signature(
            secret,
            "POST",
            "/connect/state",
            "1700000000",
            &nonce,
            body,
            &sig,
        ));
        assert!(!verify_request_signature(
            secret,
            "POST",
            "/connect/command",
            "1700000001",
            &nonce,
            body,
            &sig,
        ));
        assert!(!verify_request_signature(
            secret,
            "POST",
            "/connect/command",
            "1700000000",
            "differentnonce000000000000000000",
            body,
            &sig,
        ));

        assert_eq!(parse_timestamp_unix_secs("1700000000"), Some(1_700_000_000));
        assert_eq!(
            parse_timestamp_unix_secs("1700000000999"),
            Some(1_700_000_000)
        );
        assert_eq!(parse_timestamp_unix_secs("0"), None);
        assert_eq!(parse_timestamp_unix_secs("abc"), None);
    }

    #[test]
    fn roundtrips_beacon_pair_and_command_json() {
        assert_eq!(DISCOVERY_PORT, 45389);
        let beacon = ConnectBeacon {
            device_id: "linux-1".to_string(),
            device_name: "Sonata Linux".to_string(),
            platform: "linux".to_string(),
            http_port: 41234,
            protocol_version: CONNECT_PROTOCOL_VERSION,
            app_version: "1.0.7".to_string(),
        };
        let beacon_json = serde_json::to_string(&beacon).unwrap();
        assert_eq!(
            serde_json::from_str::<ConnectBeacon>(&beacon_json).unwrap(),
            beacon
        );

        let cmd = ConnectCommand::TransferQueue {
            tracks: vec![Track {
                id: "t1".to_string(),
                title: "Track 1".to_string(),
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
            }],
            start_index: 0,
            position_secs: 14.5,
        };
        let cmd_json = serde_json::to_string(&cmd).unwrap();
        let decoded: ConnectCommand = serde_json::from_str(&cmd_json).unwrap();
        assert!(matches!(
            decoded,
            ConnectCommand::TransferQueue {
                ref tracks,
                start_index: 0,
                position_secs,
            } if tracks.len() == 1 && tracks[0].id == "t1" && (position_secs - 14.5).abs() < 1e-6
        ));
    }

    #[test]
    fn deserializes_canonical_and_legacy_pair_requests_and_minimal_transfer_tracks() {
        assert_eq!(DEFAULT_CONNECT_HTTP_PORT, 45390);
        let canonical = r#"{"device_id":"android-1","device_name":"Test Phone","pin":"123456"}"#;
        let parsed_canonical: PairRequest = serde_json::from_str(canonical).unwrap();
        assert_eq!(parsed_canonical.device_id, "android-1");
        assert_eq!(parsed_canonical.device_name, "Test Phone");

        let legacy = r#"{"client_device_id":"android-2","client_device_name":"Legacy Phone","pin":"654321"}"#;
        let parsed_legacy: PairRequest = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed_legacy.device_id, "android-2");
        assert_eq!(parsed_legacy.device_name, "Legacy Phone");
        assert_eq!(parsed_legacy.pin, "654321");

        let minimal_transfer = r#"{
            "command": "transfer_queue",
            "tracks": [
                {
                    "id": "test-track-1",
                    "title": "Test Track",
                    "artist": "Test Artist",
                    "album": "Test Album",
                    "duration_secs": 210,
                    "thumbnail_url": "https://example.invalid/cover.jpg"
                }
            ],
            "start_index": 0,
            "position_secs": 12.5
        }"#;
        let cmd: ConnectCommand = serde_json::from_str(minimal_transfer).unwrap();
        assert!(matches!(
            cmd,
            ConnectCommand::TransferQueue {
                ref tracks,
                start_index: 0,
                position_secs,
            } if tracks.len() == 1
                && tracks[0].id == "test-track-1"
                && tracks[0].source == ProviderSource::YouTubeMusic
                && tracks[0].audio_badge.is_none()
                && (position_secs - 12.5).abs() < 1e-6
        ));
    }
}
