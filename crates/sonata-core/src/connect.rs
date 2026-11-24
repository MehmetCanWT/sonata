//! Portable Sonata Connect LAN discovery, pairing, state snapshot, and command signing types.
//!
//! 1. **Discovery & Pairing (`ConnectBeacon`, `PairRequest`, `PairAck`)**: Defines UDP port `45389`
//!    broadcast payloads and 6-digit PIN generation/verification for local LAN pairing.
//! 2. **Remote State & Commands (`ConnectStateSnapshot`, `ConnectCommand`)**: Portable models for
//!    mirroring playback state and sending transport or queue-transfer commands.
//! 3. **HMAC-SHA1 Signing (`sign_payload`, `verify_signature`)**: Authenticates control requests
//!    with constant-time comparison so forged LAN packets are rejected.

use crate::player::RepeatMode;
use crate::providers::Track;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::Read as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

/// UDP LAN broadcast discovery port for Sonata Connect.
pub const DISCOVERY_PORT: u16 = 45389;

/// Current Sonata Connect wire protocol version.
pub const CONNECT_PROTOCOL_VERSION: u16 = 1;

static ENTROPY_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    pub device_id: String,
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
    if let Ok(mut file) = File::open("/dev/urandom") {
        if file.read_exact(buf).is_ok() {
            return;
        }
    }
    let mut filled = 0;
    while filled < buf.len() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = ENTROPY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut hasher = Sha1::new();
        hasher.update(format!("{nanos}:{seq}:{}:{filled}", std::process::id()).as_bytes());
        let digest = hasher.finalize();
        let take = (buf.len() - filled).min(digest.len());
        buf[filled..filled + take].copy_from_slice(&digest[..take]);
        filled += take;
    }
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
}
