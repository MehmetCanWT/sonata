//! Linux Discord Rich Presence client over the Discord Local IPC v1 Unix socket.
//!
//! 1. **Socket Discovery**: Resolves `discord-ipc-0..9` sockets across `$XDG_RUNTIME_DIR`,
//!    `/run/user/$UID`, Snap (`snap.discord`), Flatpak (`app/com.discordapp.Discord`), and `/tmp`.
//! 2. **Binary IPC Framing**: Encodes and decodes little-endian `(op: u32, len: u32, json)` frames
//!    for `op = 0` (`HANDSHAKE`) and `op = 1` (`SET_ACTIVITY`), plus `PING`/`PONG` and `CLOSE`.
//! 3. **Activity & Speed Scaling**: Builds Discord `type = 2` (Listening) payloads with track
//!    title, optional playback speed badge, artist, speed-scaled `start`/`end` timestamps while
//!    playing, timestamp omission or clean clear-activity on pause/stop, artwork URLs, and buttons.
//! 4. **Async Controller**: Exposes [`DiscordRpcHandle`] and [`DiscordConfig`] (with
//!    [`DEFAULT_SONATA_DISCORD_APP_ID`] override support) for `sonata-tui` and `sonata-desktop`,
//!    handling absent Discord clients and mid-session reconnects gracefully without blocking callers.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sonata_core::media::upscale_artwork_url;
use sonata_core::providers::{ProviderSource, Track};
use sonata_core::redaction::stream_url as redact_url;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

/// Default Discord Application ID for Sonata Rich Presence.
pub const DEFAULT_SONATA_DISCORD_APP_ID: &str = "1346240000000000000";

/// Discord Rich Presence activity type for Listening (`type: 2`).
pub const DISCORD_ACTIVITY_TYPE_LISTENING: u8 = 2;

/// Maximum accepted binary IPC JSON frame size (4 MiB) to guard against corrupt packets.
pub const MAX_IPC_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const INITIAL_RECONNECT_BACKOFF: Duration = Duration::from_secs(2);
const MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(30);
const TIMESTAMP_SEEK_DRIFT_MS: i64 = 2_500;
const MIN_PLAYBACK_SPEED: f64 = 0.25;
const MAX_PLAYBACK_SPEED: f64 = 4.0;
const MAX_DISCORD_TEXT_CHARS: usize = 128;
const MAX_DISCORD_ASSET_KEY_CHARS: usize = 256;
const MAX_DISCORD_BUTTON_LABEL_CHARS: usize = 32;
const MAX_DISCORD_BUTTON_URL_CHARS: usize = 512;
const MAX_DISCORD_BUTTONS: usize = 2;

/// Discord Local IPC v1 frame opcodes.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcOpcode {
    Handshake = 0,
    Frame = 1,
    Close = 2,
    Ping = 3,
    Pong = 4,
}

impl TryFrom<u32> for IpcOpcode {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::Handshake),
            1 => Ok(Self::Frame),
            2 => Ok(Self::Close),
            3 => Ok(Self::Ping),
            4 => Ok(Self::Pong),
            other => bail!("unknown Discord IPC opcode: {other}"),
        }
    }
}

/// Decoded Discord Local IPC v1 packet.
#[derive(Debug, Clone, PartialEq)]
pub struct IpcPacket {
    pub opcode: u32,
    pub payload: Value,
}

/// Encodes a Discord Local IPC v1 binary packet (`op: u32 LE` + `len: u32 LE` + JSON bytes).
pub fn encode_ipc_packet(opcode: IpcOpcode, payload: &impl Serialize) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(payload).context("failed to serialize Discord IPC payload")?;
    if body.len() > MAX_IPC_PAYLOAD_BYTES {
        bail!(
            "Discord IPC payload exceeds max size ({} > {})",
            body.len(),
            MAX_IPC_PAYLOAD_BYTES
        );
    }
    let len = u32::try_from(body.len()).context("Discord IPC payload length overflow")?;
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&(opcode as u32).to_le_bytes());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Decodes a single Discord Local IPC v1 binary packet from `bytes`, returning the packet and
/// total consumed byte count (`8 + payload_len`).
pub fn decode_ipc_packet(bytes: &[u8]) -> Result<(IpcPacket, usize)> {
    if bytes.len() < 8 {
        bail!("incomplete Discord IPC header ({} bytes)", bytes.len());
    }
    let opcode = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let payload_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if payload_len > MAX_IPC_PAYLOAD_BYTES {
        bail!(
            "Discord IPC payload length exceeds max size ({} > {})",
            payload_len,
            MAX_IPC_PAYLOAD_BYTES
        );
    }
    let total_len = 8usize
        .checked_add(payload_len)
        .ok_or_else(|| anyhow!("Discord IPC frame length overflow"))?;
    if bytes.len() < total_len {
        bail!(
            "incomplete Discord IPC payload (expected {} bytes, got {})",
            total_len,
            bytes.len()
        );
    }
    let payload: Value = serde_json::from_slice(&bytes[8..total_len])
        .context("failed to parse Discord IPC JSON payload")?;
    Ok((IpcPacket { opcode, payload }, total_len))
}

async fn write_ipc_packet<W: AsyncWrite + Unpin>(
    writer: &mut W,
    opcode: IpcOpcode,
    payload: &impl Serialize,
) -> Result<()> {
    let encoded = encode_ipc_packet(opcode, payload)?;
    writer
        .write_all(&encoded)
        .await
        .context("failed to write Discord IPC packet")?;
    writer
        .flush()
        .await
        .context("failed to flush Discord IPC packet")?;
    Ok(())
}

async fn read_ipc_packet<R: AsyncRead + Unpin>(reader: &mut R) -> Result<IpcPacket> {
    let mut header = [0u8; 8];
    reader
        .read_exact(&mut header)
        .await
        .context("failed to read Discord IPC packet header")?;
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let payload_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if payload_len > MAX_IPC_PAYLOAD_BYTES {
        bail!(
            "Discord IPC payload length exceeds max size ({} > {})",
            payload_len,
            MAX_IPC_PAYLOAD_BYTES
        );
    }
    let mut body = vec![0u8; payload_len];
    reader
        .read_exact(&mut body)
        .await
        .context("failed to read Discord IPC packet body")?;
    let payload: Value =
        serde_json::from_slice(&body).context("failed to parse Discord IPC JSON payload")?;
    Ok(IpcPacket { opcode, payload })
}

/// Configuration for Linux Discord Rich Presence integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    /// Whether Discord Rich Presence updates are enabled.
    pub enabled: bool,
    /// Optional Discord Application ID overriding [`DEFAULT_SONATA_DISCORD_APP_ID`].
    pub app_id: Option<String>,
    /// When `true`, pausing playback sends a clean clear-activity update; when `false`,
    /// activity remains visible without progress timestamps.
    pub clear_on_pause: bool,
    /// Whether interactive action buttons (e.g. "Listen on YouTube Music") are included.
    pub show_buttons: bool,
    /// Whether non-1.0x playback speed badges (e.g. `[1.25x]`) are appended to `details`.
    pub show_playback_speed: bool,
    /// Optional Discord `status_display_type` (`0` = Name, `1` = State, `2` = Details).
    pub status_display_type: Option<u8>,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            app_id: None,
            clear_on_pause: true,
            show_buttons: true,
            show_playback_speed: true,
            status_display_type: None,
        }
    }
}

impl DiscordConfig {
    /// Returns the trimmed custom Application ID when non-empty, or
    /// [`DEFAULT_SONATA_DISCORD_APP_ID`] otherwise.
    pub fn effective_app_id(&self) -> &str {
        self.app_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(DEFAULT_SONATA_DISCORD_APP_ID)
    }

    /// Sets an Application ID override (empty/whitespace overrides revert to the default ID).
    pub fn with_app_id(mut self, app_id: Option<String>) -> Self {
        self.app_id = app_id.and_then(|raw| {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        self
    }
}

/// Interactive button attached to a Discord Rich Presence activity (max 2 per activity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordButton {
    pub label: String,
    pub url: String,
}

/// Track metadata projected into Discord Rich Presence payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscordTrackInfo {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub track_id: Option<String>,
    pub source: Option<ProviderSource>,
    pub artwork_url: Option<String>,
    pub listen_url: Option<String>,
    pub duration_secs: Option<f64>,
    pub buttons: Vec<DiscordButton>,
}

impl DiscordTrackInfo {
    /// Builds [`DiscordTrackInfo`] from a core provider [`Track`].
    pub fn from_track(track: &Track) -> Self {
        let artwork_url = track
            .thumbnail_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(upscale_artwork_url)
            .or_else(|| {
                if track.source == ProviderSource::YouTubeMusic
                    && !track.is_jiosaavn()
                    && !track.id.trim().is_empty()
                {
                    Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", track.id))
                } else {
                    None
                }
            });

        let listen_url = derive_listen_url(track);
        let buttons = listen_url
            .as_ref()
            .map(|url| {
                let label = match track.source {
                    ProviderSource::YouTubeMusic if !track.is_jiosaavn() => {
                        "Listen on YouTube Music"
                    }
                    ProviderSource::Spotify if !track.is_jiosaavn() => "Listen on Spotify",
                    _ => "Listen on Sonata",
                };
                vec![DiscordButton {
                    label: label.to_string(),
                    url: url.clone(),
                }]
            })
            .unwrap_or_default();

        let album = if track.album.trim().is_empty() {
            None
        } else {
            Some(track.album.clone())
        };

        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album,
            track_id: Some(track.id.clone()),
            source: Some(track.source),
            artwork_url,
            listen_url,
            duration_secs: track.duration_secs.map(|s| s as f64),
            buttons,
        }
    }
}

fn derive_listen_url(track: &Track) -> Option<String> {
    if track.is_jiosaavn() {
        return None;
    }
    let id = track.id.trim();
    if id.is_empty() {
        return None;
    }
    match track.source {
        ProviderSource::YouTubeMusic => Some(format!("https://music.youtube.com/watch?v={id}")),
        ProviderSource::Spotify => {
            let clean = id.strip_prefix("spotify:track:").unwrap_or(id);
            if clean.chars().all(|c| c.is_ascii_alphanumeric()) {
                Some(format!("https://open.spotify.com/track/{clean}"))
            } else {
                None
            }
        }
    }
}

/// Snapshot of current playback state sent to [`DiscordRpcHandle`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiscordPlaybackState {
    pub track: Option<DiscordTrackInfo>,
    pub paused: bool,
    pub position_secs: f64,
    pub playback_speed: f64,
}

impl Default for DiscordPlaybackState {
    fn default() -> Self {
        Self {
            track: None,
            paused: true,
            position_secs: 0.0,
            playback_speed: 1.0,
        }
    }
}

impl DiscordPlaybackState {
    /// Creates a playing state for the given track, position, and playback speed.
    pub fn playing(track: DiscordTrackInfo, position_secs: f64, playback_speed: f64) -> Self {
        Self {
            track: Some(track),
            paused: false,
            position_secs,
            playback_speed,
        }
    }

    /// Creates a paused state for the given track.
    pub fn paused(
        track: Option<DiscordTrackInfo>,
        position_secs: f64,
        playback_speed: f64,
    ) -> Self {
        Self {
            track,
            paused: true,
            position_secs,
            playback_speed,
        }
    }

    /// Creates a stopped/cleared state with no active track.
    pub fn cleared() -> Self {
        Self::default()
    }
}

/// Discord activity timestamps in Unix epoch milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordTimestamps {
    pub start: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

/// Discord activity image assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordAssets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

/// Discord Rich Presence activity object sent in `SET_ACTIVITY`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordActivity {
    #[serde(rename = "type")]
    pub activity_type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<DiscordTimestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<DiscordAssets>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<DiscordButton>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_display_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<bool>,
}

/// Connection lifecycle state observed by TUI and Desktop clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscordRpcStatus {
    Disabled,
    Disconnected,
    Connecting,
    Connected { socket_path: PathBuf },
    Error(String),
}

fn sanitize_speed(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= 0.0 {
        1.0
    } else {
        speed.clamp(MIN_PLAYBACK_SPEED, MAX_PLAYBACK_SPEED)
    }
}

fn sanitize_non_negative_secs(secs: f64) -> f64 {
    if !secs.is_finite() || secs < 0.0 {
        0.0
    } else {
        secs
    }
}

/// Formats a compact playback speed badge (e.g. `1.25x`, `1.5x`, `2x`) when speed differs from
/// `1.0x`, returning `None` at normal speed.
pub fn format_speed_badge(playback_speed: f64) -> Option<String> {
    let speed = sanitize_speed(playback_speed);
    if (speed - 1.0).abs() < 0.005 {
        return None;
    }
    let formatted = format!("{speed:.2}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    Some(format!("{trimmed}x"))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    if max_chars <= 1 {
        return trimmed.chars().take(max_chars).collect();
    }
    let mut out: String = trimmed.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn normalize_discord_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let clamped = truncate_chars(trimmed, MAX_DISCORD_TEXT_CHARS);
    // Discord rejects single-character activity strings; pad with a space when needed.
    if clamped.chars().count() == 1 {
        Some(format!("{clamped} "))
    } else {
        Some(clamped)
    }
}

/// Formats `details` (track title) with an optional `[1.25x]` speed badge while respecting
/// Discord's 128-character limit.
pub fn format_details_with_speed(
    title: &str,
    playback_speed: f64,
    show_playback_speed: bool,
) -> Option<String> {
    let clean_title = title.trim();
    if clean_title.is_empty() {
        return None;
    }
    let Some(badge) = show_playback_speed
        .then(|| format_speed_badge(playback_speed))
        .flatten()
    else {
        return normalize_discord_text(clean_title);
    };

    let suffix = format!(" [{badge}]");
    let suffix_chars = suffix.chars().count();
    let max_title_chars = MAX_DISCORD_TEXT_CHARS.saturating_sub(suffix_chars);
    let truncated_title = truncate_chars(clean_title, max_title_chars);
    normalize_discord_text(&format!("{truncated_title}{suffix}"))
}

/// Computes speed-adjusted Unix epoch millisecond `start` and `end` timestamps for active
/// playback.
///
/// At playback speed `s`, `position_secs` of media time corresponds to `position_secs / s`
/// wall-clock seconds elapsed, and `(duration - position_secs) / s` wall-clock seconds remaining.
pub fn compute_speed_scaled_timestamps(
    position_secs: f64,
    duration_secs: Option<f64>,
    playback_speed: f64,
    now_ms: i64,
) -> DiscordTimestamps {
    let speed = sanitize_speed(playback_speed);
    let valid_duration = duration_secs.filter(|d| d.is_finite() && *d > 0.0);
    let pos = sanitize_non_negative_secs(position_secs);
    let clamped_pos = valid_duration.map_or(pos, |dur| pos.min(dur));

    let elapsed_wall_ms = ((clamped_pos / speed) * 1000.0).round() as i64;
    let start = now_ms.saturating_sub(elapsed_wall_ms);

    let end = valid_duration.map(|dur| {
        let remaining_media_secs = (dur - clamped_pos).max(0.0);
        let remaining_wall_ms = ((remaining_media_secs / speed) * 1000.0).round() as i64;
        now_ms.saturating_add(remaining_wall_ms)
    });

    DiscordTimestamps { start, end }
}

fn sanitize_http_url(url: &str, max_chars: usize) -> Option<String> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return None;
    }
    if trimmed.chars().count() > max_chars {
        return None;
    }
    Some(trimmed.to_string())
}

fn sanitize_buttons(buttons: &[DiscordButton]) -> Vec<DiscordButton> {
    buttons
        .iter()
        .filter_map(|button| {
            let label = truncate_chars(&button.label, MAX_DISCORD_BUTTON_LABEL_CHARS);
            if label.is_empty() {
                return None;
            }
            let url = sanitize_http_url(&button.url, MAX_DISCORD_BUTTON_URL_CHARS)?;
            Some(DiscordButton { label, url })
        })
        .take(MAX_DISCORD_BUTTONS)
        .collect()
}

/// Builds a [`DiscordActivity`] for the given playback state and configuration, returning `None`
/// when playback is stopped or paused with `clear_on_pause = true`.
pub fn build_activity(
    state: &DiscordPlaybackState,
    config: &DiscordConfig,
    now_ms: i64,
) -> Option<DiscordActivity> {
    if !config.enabled {
        return None;
    }
    let track = state.track.as_ref()?;
    if state.paused && config.clear_on_pause {
        return None;
    }

    let details = format_details_with_speed(
        &track.title,
        state.playback_speed,
        config.show_playback_speed,
    )
    .or_else(|| Some("Listening on Sonata".to_string()));
    let artist_state = normalize_discord_text(&track.artist);

    let timestamps = if state.paused {
        None
    } else {
        Some(compute_speed_scaled_timestamps(
            state.position_secs,
            track.duration_secs,
            state.playback_speed,
            now_ms,
        ))
    };

    let large_image = track
        .artwork_url
        .as_deref()
        .and_then(|url| sanitize_http_url(url, MAX_DISCORD_ASSET_KEY_CHARS));
    let large_text = track
        .album
        .as_deref()
        .and_then(normalize_discord_text)
        .or_else(|| Some("Sonata".to_string()));
    let small_text = if state.paused {
        Some("Paused".to_string())
    } else {
        None
    };

    let assets = if large_image.is_some() || large_text.is_some() || small_text.is_some() {
        Some(DiscordAssets {
            large_image,
            large_text,
            small_image: None,
            small_text,
        })
    } else {
        None
    };

    let buttons = if config.show_buttons {
        if !track.buttons.is_empty() {
            sanitize_buttons(&track.buttons)
        } else if let Some(url) = track.listen_url.as_deref() {
            sanitize_buttons(&[DiscordButton {
                label: "Listen on Sonata".to_string(),
                url: url.to_string(),
            }])
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Some(DiscordActivity {
        activity_type: DISCORD_ACTIVITY_TYPE_LISTENING,
        details,
        state: artist_state,
        timestamps,
        assets,
        buttons,
        status_display_type: config.status_display_type,
        instance: Some(false),
    })
}

/// Builds the JSON payload for an `op = 0` Discord IPC handshake frame.
pub fn build_handshake_payload(client_id: &str) -> Value {
    json!({
        "v": 1,
        "client_id": client_id.trim(),
    })
}

/// Builds the JSON payload for an `op = 1` `SET_ACTIVITY` frame (`activity: None` clears activity).
pub fn build_set_activity_payload(
    pid: u32,
    activity: Option<&DiscordActivity>,
    nonce: &str,
) -> Value {
    let args = match activity {
        Some(act) => json!({
            "pid": pid,
            "activity": act,
        }),
        None => json!({
            "pid": pid,
        }),
    };
    json!({
        "cmd": "SET_ACTIVITY",
        "args": args,
        "nonce": nonce,
    })
}

fn current_uid() -> Option<u32> {
    if let Ok(raw) = std::env::var("UID") {
        if let Ok(uid) = raw.trim().parse::<u32>() {
            return Some(uid);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            return Some(meta.uid());
        }
    }
    None
}

/// Returns ordered candidate Unix socket paths for Discord Local IPC (`discord-ipc-0..9`) across
/// `$XDG_RUNTIME_DIR`, `/run/user/$UID`, Snap (`snap.discord`), Flatpak
/// (`app/com.discordapp.Discord`), and `$TMPDIR` / `/tmp`.
pub fn candidate_ipc_socket_paths() -> Vec<PathBuf> {
    let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let uid = current_uid();
    let tmp_dir = std::env::var_os("TMPDIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from("/tmp")));
    candidate_ipc_socket_paths_from(xdg_runtime_dir.as_deref(), uid, tmp_dir.as_deref())
}

/// Pure helper constructing ordered Discord IPC socket candidates from explicit runtime inputs.
pub fn candidate_ipc_socket_paths_from(
    xdg_runtime_dir: Option<&Path>,
    uid: Option<u32>,
    tmp_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut base_dirs: Vec<PathBuf> = Vec::new();
    let mut push_unique_dir = |dir: PathBuf| {
        if !dir.as_os_str().is_empty() && !base_dirs.contains(&dir) {
            base_dirs.push(dir);
        }
    };

    if let Some(xdg) = xdg_runtime_dir {
        push_unique_dir(xdg.to_path_buf());
    }
    if let Some(uid) = uid {
        push_unique_dir(PathBuf::from(format!("/run/user/{uid}")));
    }
    if let Some(tmp) = tmp_dir {
        push_unique_dir(tmp.to_path_buf());
    }

    let mut candidates = Vec::with_capacity(base_dirs.len() * 30);
    let mut seen = HashSet::new();

    for base in base_dirs {
        let subdirs = [
            base.clone(),
            base.join("app/com.discordapp.Discord"),
            base.join("snap.discord"),
        ];
        for dir in subdirs {
            for idx in 0..10 {
                let socket = dir.join(format!("discord-ipc-{idx}"));
                if seen.insert(socket.clone()) {
                    candidates.push(socket);
                }
            }
        }
    }

    candidates
}

/// Discovers currently existing Discord IPC socket paths on disk.
pub fn discover_existing_ipc_sockets() -> Vec<PathBuf> {
    candidate_ipc_socket_paths()
        .into_iter()
        .filter(|path| is_socket_path(path))
        .collect()
}

fn is_socket_path(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        std::fs::metadata(path)
            .map(|meta| meta.file_type().is_socket())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.exists()
    }
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
struct SentActivitySnapshot {
    track: Option<DiscordTrackInfo>,
    paused: bool,
    playback_speed: f64,
    start_ms: Option<i64>,
    show_buttons: bool,
    show_playback_speed: bool,
    clear_on_pause: bool,
    status_display_type: Option<u8>,
}

impl SentActivitySnapshot {
    fn from_state(
        state: &DiscordPlaybackState,
        config: &DiscordConfig,
        activity: Option<&DiscordActivity>,
    ) -> Self {
        Self {
            track: state.track.clone(),
            paused: state.paused,
            playback_speed: sanitize_speed(state.playback_speed),
            start_ms: activity.and_then(|a| a.timestamps.map(|t| t.start)),
            show_buttons: config.show_buttons,
            show_playback_speed: config.show_playback_speed,
            clear_on_pause: config.clear_on_pause,
            status_display_type: config.status_display_type,
        }
    }

    fn requires_update(
        &self,
        next_state: &DiscordPlaybackState,
        next_config: &DiscordConfig,
        next_activity: Option<&DiscordActivity>,
    ) -> bool {
        if self.track != next_state.track
            || self.paused != next_state.paused
            || (self.playback_speed - sanitize_speed(next_state.playback_speed)).abs() >= 0.005
            || self.show_buttons != next_config.show_buttons
            || self.show_playback_speed != next_config.show_playback_speed
            || self.clear_on_pause != next_config.clear_on_pause
            || self.status_display_type != next_config.status_display_type
        {
            return true;
        }

        let next_start = next_activity.and_then(|a| a.timestamps.map(|t| t.start));
        match (self.start_ms, next_start) {
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
            (Some(prev), Some(curr)) => (curr - prev).abs() >= TIMESTAMP_SEEK_DRIFT_MS,
        }
    }
}

struct IpcConnection {
    stream: UnixStream,
    socket_path: PathBuf,
    app_id: String,
    pid: u32,
    nonce_counter: AtomicU64,
}

impl IpcConnection {
    async fn connect(sockets: &[PathBuf], app_id: &str) -> Result<Self> {
        let mut last_err: Option<anyhow::Error> = None;
        for path in sockets {
            match Self::try_connect_one(path, app_id).await {
                Ok(conn) => return Ok(conn),
                Err(err) => {
                    tracing::debug!(
                        socket = %path.display(),
                        error = %err,
                        "Discord IPC socket connection failed"
                    );
                    last_err = Some(err);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("no Discord IPC sockets found")))
    }

    async fn try_connect_one(path: &Path, app_id: &str) -> Result<Self> {
        let mut stream = timeout(HANDSHAKE_TIMEOUT, UnixStream::connect(path))
            .await
            .context("timed out connecting to Discord IPC socket")?
            .with_context(|| format!("failed to connect to {}", path.display()))?;

        let handshake = build_handshake_payload(app_id);
        timeout(
            HANDSHAKE_TIMEOUT,
            write_ipc_packet(&mut stream, IpcOpcode::Handshake, &handshake),
        )
        .await
        .context("timed out sending Discord IPC handshake")??;

        let reply = timeout(HANDSHAKE_TIMEOUT, read_ipc_packet(&mut stream))
            .await
            .context("timed out reading Discord IPC handshake reply")??;
        if reply.opcode == IpcOpcode::Close as u32 {
            let msg = reply
                .payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("handshake rejected by Discord IPC");
            bail!("{msg}");
        }
        if reply.opcode != IpcOpcode::Frame as u32 {
            bail!("unexpected Discord IPC handshake opcode: {}", reply.opcode);
        }
        if reply.payload.get("evt").and_then(Value::as_str) == Some("ERROR") {
            let msg = reply
                .payload
                .get("data")
                .and_then(|d| d.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Discord IPC returned READY error");
            bail!("{msg}");
        }

        Ok(Self {
            stream,
            socket_path: path.to_path_buf(),
            app_id: app_id.to_string(),
            pid: std::process::id(),
            nonce_counter: AtomicU64::new(1),
        })
    }

    async fn send_activity(&mut self, activity: Option<&DiscordActivity>) -> Result<()> {
        let seq = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
        let nonce = format!("sonata-{}-{seq}", self.pid);
        let payload = build_set_activity_payload(self.pid, activity, &nonce);

        timeout(
            RESPONSE_TIMEOUT,
            write_ipc_packet(&mut self.stream, IpcOpcode::Frame, &payload),
        )
        .await
        .context("timed out writing Discord SET_ACTIVITY")??;

        // Drain the immediate response and handle any PING/CLOSE frames from Discord.
        loop {
            let reply = timeout(RESPONSE_TIMEOUT, read_ipc_packet(&mut self.stream))
                .await
                .context("timed out reading Discord SET_ACTIVITY reply")??;
            match IpcOpcode::try_from(reply.opcode) {
                Ok(IpcOpcode::Ping) => {
                    write_ipc_packet(&mut self.stream, IpcOpcode::Pong, &reply.payload).await?;
                }
                Ok(IpcOpcode::Close) => {
                    bail!("Discord IPC closed connection");
                }
                Ok(IpcOpcode::Frame) => {
                    if reply.payload.get("evt").and_then(Value::as_str) == Some("ERROR") {
                        let msg = reply
                            .payload
                            .get("data")
                            .and_then(|d| d.get("message"))
                            .and_then(Value::as_str)
                            .unwrap_or("SET_ACTIVITY rejected");
                        tracing::debug!(error = msg, "Discord IPC SET_ACTIVITY error");
                    }
                    break;
                }
                _ => break,
            }
        }
        Ok(())
    }
}

/// Async handle controlling Sonata's background Discord Rich Presence actor.
#[derive(Clone)]
pub struct DiscordRpcHandle {
    state_tx: watch::Sender<DiscordPlaybackState>,
    config_tx: watch::Sender<DiscordConfig>,
    shutdown_tx: watch::Sender<bool>,
    status_rx: watch::Receiver<DiscordRpcStatus>,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DiscordRpcHandle {
    /// Spawns the background Discord Rich Presence actor using standard Linux IPC socket
    /// discovery (`$XDG_RUNTIME_DIR`, `/run/user/$UID`, Snap, Flatpak, `/tmp`).
    pub fn spawn(config: DiscordConfig) -> Self {
        Self::spawn_with_socket_discovery(config, discover_existing_ipc_sockets)
    }

    /// Spawns the background actor with a custom socket discovery callback (primarily for tests).
    pub fn spawn_with_socket_discovery<F>(config: DiscordConfig, discover_sockets: F) -> Self
    where
        F: Fn() -> Vec<PathBuf> + Send + Sync + 'static,
    {
        let initial_status = if config.enabled {
            DiscordRpcStatus::Disconnected
        } else {
            DiscordRpcStatus::Disabled
        };

        let (state_tx, state_rx) = watch::channel(DiscordPlaybackState::default());
        let (config_tx, config_rx) = watch::channel(config);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (status_tx, status_rx) = watch::channel(initial_status);

        let handle = tokio::spawn(run_discord_actor(
            state_rx,
            config_rx,
            shutdown_rx,
            status_tx,
            discover_sockets,
        ));

        Self {
            state_tx,
            config_tx,
            shutdown_tx,
            status_rx,
            task: Arc::new(Mutex::new(Some(handle))),
        }
    }

    /// Updates playback state, coalescing frequent position ticks automatically.
    pub async fn update_playback(&self, state: DiscordPlaybackState) {
        self.try_update_playback(state);
    }

    /// Non-blocking playback state update suitable for synchronous UI or tick callbacks.
    pub fn try_update_playback(&self, state: DiscordPlaybackState) {
        let _ = self.state_tx.send(state);
    }

    /// Clears the active Discord presence immediately.
    pub async fn clear_activity(&self) {
        self.try_clear_activity();
    }

    /// Non-blocking clear-activity helper.
    pub fn try_clear_activity(&self) {
        let _ = self.state_tx.send(DiscordPlaybackState::cleared());
    }

    /// Applies updated Discord Rich Presence configuration (including Application ID overrides).
    pub async fn update_config(&self, config: DiscordConfig) {
        self.try_update_config(config);
    }

    /// Non-blocking configuration update helper.
    pub fn try_update_config(&self, config: DiscordConfig) {
        let _ = self.config_tx.send(config);
    }

    /// Returns the current connection status snapshot.
    pub fn status(&self) -> DiscordRpcStatus {
        self.status_rx.borrow().clone()
    }

    /// Subscribes to status updates for UI indicators.
    pub fn subscribe_status(&self) -> watch::Receiver<DiscordRpcStatus> {
        self.status_rx.clone()
    }

    /// Gracefully clears Discord activity (if connected) and stops the background actor.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let handle = self.task.lock().await.take();
        if let Some(join_handle) = handle {
            if timeout(SHUTDOWN_TIMEOUT, join_handle).await.is_err() {
                tracing::debug!("Discord RPC actor shutdown timed out");
            }
        }
    }
}

impl Drop for DiscordRpcHandle {
    fn drop(&mut self) {
        // Only signal shutdown when the final handle is dropped so clones keep the actor alive.
        if Arc::strong_count(&self.task) <= 1 {
            let _ = self.shutdown_tx.send(true);
        }
    }
}

async fn run_discord_actor<F>(
    mut state_rx: watch::Receiver<DiscordPlaybackState>,
    mut config_rx: watch::Receiver<DiscordConfig>,
    mut shutdown_rx: watch::Receiver<bool>,
    status_tx: watch::Sender<DiscordRpcStatus>,
    discover_sockets: F,
) where
    F: Fn() -> Vec<PathBuf> + Send + Sync + 'static,
{
    let mut conn: Option<IpcConnection> = None;
    let mut last_sent: Option<SentActivitySnapshot> = None;
    let mut backoff = INITIAL_RECONNECT_BACKOFF;

    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        let current_config = config_rx.borrow_and_update().clone();
        if !current_config.enabled {
            if let Some(mut active) = conn.take() {
                let _ = active.send_activity(None).await;
            }
            last_sent = None;
            let _ = status_tx.send(DiscordRpcStatus::Disabled);

            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                res = config_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                }
                res = state_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                    let _ = state_rx.borrow_and_update();
                }
            }
            continue;
        }

        let desired_app_id = current_config.effective_app_id().to_string();
        if conn
            .as_ref()
            .is_some_and(|active| active.app_id != desired_app_id)
        {
            if let Some(mut active) = conn.take() {
                let _ = active.send_activity(None).await;
            }
            last_sent = None;
        }

        if conn.is_none() {
            let sockets = discover_sockets();
            if sockets.is_empty() {
                let _ = status_tx.send(DiscordRpcStatus::Disconnected);
            } else {
                let _ = status_tx.send(DiscordRpcStatus::Connecting);
                match IpcConnection::connect(&sockets, &desired_app_id).await {
                    Ok(established) => {
                        tracing::info!(
                            socket = %established.socket_path.display(),
                            "connected to Discord Local IPC"
                        );
                        let _ = status_tx.send(DiscordRpcStatus::Connected {
                            socket_path: established.socket_path.clone(),
                        });
                        conn = Some(established);
                        last_sent = None;
                        backoff = INITIAL_RECONNECT_BACKOFF;
                    }
                    Err(err) => {
                        let safe_err = redact_url(&err.to_string());
                        tracing::debug!(error = %safe_err, "Discord Local IPC unavailable");
                        let _ = status_tx.send(DiscordRpcStatus::Disconnected);
                    }
                }
            }
        }

        if let Some(active) = conn.as_mut() {
            let current_state = state_rx.borrow_and_update().clone();
            let now_ms = now_unix_ms();
            let activity = build_activity(&current_state, &current_config, now_ms);
            let should_send = last_sent.as_ref().is_none_or(|prev| {
                prev.requires_update(&current_state, &current_config, activity.as_ref())
            });

            if should_send {
                match active.send_activity(activity.as_ref()).await {
                    Ok(()) => {
                        last_sent = Some(SentActivitySnapshot::from_state(
                            &current_state,
                            &current_config,
                            activity.as_ref(),
                        ));
                    }
                    Err(err) => {
                        tracing::debug!(
                            error = %err,
                            "Discord IPC connection dropped while updating activity"
                        );
                        conn = None;
                        last_sent = None;
                        let _ = status_tx.send(DiscordRpcStatus::Disconnected);
                        continue;
                    }
                }
            }

            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                res = state_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                }
                res = config_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                }
            }
        } else {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                res = config_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                    backoff = INITIAL_RECONNECT_BACKOFF;
                }
                res = state_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                    let _ = state_rx.borrow_and_update();
                }
                () = sleep(backoff) => {
                    backoff = (backoff * 2).min(MAX_RECONNECT_BACKOFF);
                }
            }
        }
    }

    if let Some(mut active) = conn.take() {
        let _ = active.send_activity(None).await;
    }
    let _ = status_tx.send(DiscordRpcStatus::Disconnected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    fn sample_track() -> DiscordTrackInfo {
        DiscordTrackInfo {
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: Some("Hurry Up, We're Dreaming".to_string()),
            track_id: Some("dX3k_QDnzHE".to_string()),
            source: Some(ProviderSource::YouTubeMusic),
            artwork_url: Some("https://lh3.googleusercontent.com/cover=w544-h544".to_string()),
            listen_url: Some("https://music.youtube.com/watch?v=dX3k_QDnzHE".to_string()),
            duration_secs: Some(243.0),
            buttons: vec![DiscordButton {
                label: "Listen on YouTube Music".to_string(),
                url: "https://music.youtube.com/watch?v=dX3k_QDnzHE".to_string(),
            }],
        }
    }

    #[test]
    fn binary_ipc_packet_roundtrip_and_header_layout() {
        let handshake = build_handshake_payload(DEFAULT_SONATA_DISCORD_APP_ID);
        let encoded = encode_ipc_packet(IpcOpcode::Handshake, &handshake).unwrap();

        let op = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(encoded[4..8].try_into().unwrap()) as usize;
        assert_eq!(op, 0);
        assert_eq!(len, encoded.len() - 8);

        let (decoded, consumed) = decode_ipc_packet(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.opcode, 0);
        assert_eq!(decoded.payload["v"], 1);
        assert_eq!(
            decoded.payload["client_id"],
            Value::String(DEFAULT_SONATA_DISCORD_APP_ID.to_string())
        );

        let act = build_activity(
            &DiscordPlaybackState::playing(sample_track(), 30.0, 1.0),
            &DiscordConfig::default(),
            1_700_000_000_000,
        )
        .unwrap();
        let set_activity = build_set_activity_payload(4242, Some(&act), "nonce-1");
        let encoded_frame = encode_ipc_packet(IpcOpcode::Frame, &set_activity).unwrap();
        let (decoded_frame, frame_consumed) = decode_ipc_packet(&encoded_frame).unwrap();
        assert_eq!(frame_consumed, encoded_frame.len());
        assert_eq!(decoded_frame.opcode, 1);
        assert_eq!(decoded_frame.payload["cmd"], "SET_ACTIVITY");
        assert_eq!(decoded_frame.payload["args"]["pid"], 4242);
        assert_eq!(
            decoded_frame.payload["args"]["activity"]["type"],
            DISCORD_ACTIVITY_TYPE_LISTENING
        );
    }

    #[test]
    fn decode_ipc_packet_rejects_truncated_and_oversized_frames() {
        assert!(decode_ipc_packet(&[0, 0, 0]).is_err());

        let oversized_len = (MAX_IPC_PAYLOAD_BYTES as u32 + 1).to_le_bytes();
        let mut bad_header = vec![1, 0, 0, 0];
        bad_header.extend_from_slice(&oversized_len);
        assert!(decode_ipc_packet(&bad_header).is_err());

        let mut truncated = encode_ipc_packet(IpcOpcode::Frame, &json!({"cmd": "PING"})).unwrap();
        truncated.pop();
        assert!(decode_ipc_packet(&truncated).is_err());
    }

    #[test]
    fn timestamps_scale_with_playback_speed() {
        let now_ms = 1_700_000_000_000i64;

        let ts_1x = compute_speed_scaled_timestamps(30.0, Some(120.0), 1.0, now_ms);
        assert_eq!(ts_1x.start, now_ms - 30_000);
        assert_eq!(ts_1x.end, Some(now_ms + 90_000));

        // At 2.0x speed, 30s elapsed media = 15s wall time; 90s remaining media = 45s wall time.
        let ts_2x = compute_speed_scaled_timestamps(30.0, Some(120.0), 2.0, now_ms);
        assert_eq!(ts_2x.start, now_ms - 15_000);
        assert_eq!(ts_2x.end, Some(now_ms + 45_000));

        // At 0.5x speed, 30s elapsed media = 60s wall time; 90s remaining media = 180s wall time.
        let ts_half = compute_speed_scaled_timestamps(30.0, Some(120.0), 0.5, now_ms);
        assert_eq!(ts_half.start, now_ms - 60_000);
        assert_eq!(ts_half.end, Some(now_ms + 180_000));

        // Missing duration omits `end`.
        let ts_no_dur = compute_speed_scaled_timestamps(12.0, None, 1.5, now_ms);
        assert_eq!(ts_no_dur.start, now_ms - 8_000);
        assert_eq!(ts_no_dur.end, None);
    }

    #[test]
    fn paused_and_clear_payload_shaping_and_speed_badge() {
        let playing_fast = DiscordPlaybackState::playing(sample_track(), 45.0, 1.25);
        let cfg_keep_paused = DiscordConfig {
            clear_on_pause: false,
            ..DiscordConfig::default()
        };
        let playing_act = build_activity(&playing_fast, &cfg_keep_paused, 1_700_000_000_000)
            .expect("playing activity should exist");
        assert_eq!(playing_act.activity_type, 2);
        assert_eq!(
            playing_act.details.as_deref(),
            Some("Midnight City [1.25x]")
        );
        assert_eq!(playing_act.state.as_deref(), Some("M83"));
        assert!(playing_act.timestamps.is_some());
        assert_eq!(
            playing_act
                .assets
                .as_ref()
                .and_then(|a| a.large_image.as_deref()),
            Some("https://lh3.googleusercontent.com/cover=w544-h544")
        );
        assert_eq!(playing_act.buttons.len(), 1);

        // Pausing with clear_on_pause = false keeps activity but omits timestamps.
        let paused_state = DiscordPlaybackState::paused(Some(sample_track()), 45.0, 1.25);
        let paused_act = build_activity(&paused_state, &cfg_keep_paused, 1_700_000_000_000)
            .expect("paused activity should exist when clear_on_pause is false");
        assert!(paused_act.timestamps.is_none());
        let serialized_paused = serde_json::to_value(&paused_act).unwrap();
        assert!(serialized_paused.get("timestamps").is_none());

        // Pausing with clear_on_pause = true produces clear-activity (`activity: None`).
        let cfg_clear_paused = DiscordConfig::default();
        assert!(build_activity(&paused_state, &cfg_clear_paused, 1_700_000_000_000).is_none());

        let clear_payload = build_set_activity_payload(999, None, "clear-1");
        assert_eq!(clear_payload["cmd"], "SET_ACTIVITY");
        assert_eq!(clear_payload["args"]["pid"], 999);
        assert!(clear_payload["args"].get("activity").is_none());
    }

    #[test]
    fn config_app_id_override_and_track_conversion() {
        let default_cfg = DiscordConfig::default();
        assert_eq!(
            default_cfg.effective_app_id(),
            DEFAULT_SONATA_DISCORD_APP_ID
        );

        let blank_override = DiscordConfig::default().with_app_id(Some("   ".to_string()));
        assert_eq!(
            blank_override.effective_app_id(),
            DEFAULT_SONATA_DISCORD_APP_ID
        );

        let custom_cfg = DiscordConfig::default().with_app_id(Some(" 987654321012345678 ".into()));
        assert_eq!(custom_cfg.effective_app_id(), "987654321012345678");

        let ytm_track = Track {
            id: "abc123xyz".to_string(),
            title: "Resonance".to_string(),
            artist: "HOME".to_string(),
            album: "Odyssey".to_string(),
            duration_secs: Some(212),
            thumbnail_url: Some("https://lh3.googleusercontent.com/x=w120-h120".to_string()),
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let info = DiscordTrackInfo::from_track(&ytm_track);
        assert_eq!(
            info.listen_url.as_deref(),
            Some("https://music.youtube.com/watch?v=abc123xyz")
        );
        assert!(info
            .artwork_url
            .as_deref()
            .is_some_and(|u| u.starts_with("https://")));
        assert_eq!(info.buttons.len(), 1);
    }

    #[test]
    fn socket_path_discovery_covers_xdg_uid_flatpak_and_snap() {
        let paths = candidate_ipc_socket_paths_from(
            Some(Path::new("/run/user/1000")),
            Some(1000),
            Some(Path::new("/tmp")),
        );

        assert!(paths.contains(&PathBuf::from("/run/user/1000/discord-ipc-0")));
        assert!(paths.contains(&PathBuf::from("/run/user/1000/discord-ipc-9")));
        assert!(paths.contains(&PathBuf::from(
            "/run/user/1000/app/com.discordapp.Discord/discord-ipc-0"
        )));
        assert!(paths.contains(&PathBuf::from("/run/user/1000/snap.discord/discord-ipc-0")));
        assert!(paths.contains(&PathBuf::from("/tmp/discord-ipc-0")));

        // Verify deduplication when XDG_RUNTIME_DIR == /run/user/$UID (3 subdirs * 10 * 2 bases = 60).
        assert_eq!(paths.len(), 60);

        // When XDG_RUNTIME_DIR differs from /run/user/$UID, both base directories are searched.
        let distinct = candidate_ipc_socket_paths_from(
            Some(Path::new("/custom/runtime")),
            Some(1001),
            Some(Path::new("/tmp")),
        );
        assert!(distinct.contains(&PathBuf::from("/custom/runtime/discord-ipc-0")));
        assert!(distinct.contains(&PathBuf::from("/run/user/1001/snap.discord/discord-ipc-4")));
        assert!(distinct.contains(&PathBuf::from(
            "/run/user/1001/app/com.discordapp.Discord/discord-ipc-9"
        )));
        assert_eq!(distinct.len(), 90);
    }

    #[tokio::test]
    async fn controller_handshake_set_activity_and_clear_over_unix_socket() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sock_dir = std::env::temp_dir().join(format!("sonata-discord-test-{unique}"));
        std::fs::create_dir_all(&sock_dir).unwrap();
        let sock_path = sock_dir.join("discord-ipc-0");
        let listener = UnixListener::bind(&sock_path).unwrap();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let hs = read_ipc_packet(&mut stream).await.unwrap();
            assert_eq!(hs.opcode, IpcOpcode::Handshake as u32);
            assert_eq!(hs.payload["v"], 1);
            assert_eq!(hs.payload["client_id"], "555555555555555555");

            write_ipc_packet(
                &mut stream,
                IpcOpcode::Frame,
                &json!({"cmd": "DISPATCH", "evt": "READY", "data": {"v": 1}}),
            )
            .await
            .unwrap();

            // Expect initial cleared snapshot or playing update, then playing update, then clear.
            let mut observed = Vec::new();
            while let Ok(pkt) = timeout(Duration::from_secs(2), read_ipc_packet(&mut stream)).await
            {
                let Ok(pkt) = pkt else {
                    break;
                };
                observed.push(pkt.clone());
                let nonce = pkt.payload["nonce"].as_str().unwrap_or("").to_string();
                let _ = write_ipc_packet(
                    &mut stream,
                    IpcOpcode::Frame,
                    &json!({"cmd": "SET_ACTIVITY", "evt": null, "nonce": nonce}),
                )
                .await;
                if observed
                    .iter()
                    .any(|p| p.payload["args"]["activity"]["details"] == "Midnight City [1.5x]")
                    && observed
                        .last()
                        .is_some_and(|p| p.payload["args"].get("activity").is_none())
                {
                    break;
                }
            }
            observed
        });

        let cfg = DiscordConfig::default().with_app_id(Some("555555555555555555".to_string()));
        let sock_clone = sock_path.clone();
        let handle =
            DiscordRpcHandle::spawn_with_socket_discovery(cfg, move || vec![sock_clone.clone()]);

        // Wait until connected.
        let mut status_rx = handle.subscribe_status();
        timeout(Duration::from_secs(2), async {
            loop {
                if matches!(
                    &*status_rx.borrow(),
                    DiscordRpcStatus::Connected { socket_path } if socket_path == &sock_path
                ) {
                    break;
                }
                status_rx.changed().await.unwrap();
            }
        })
        .await
        .expect("controller should connect to test Unix socket");

        handle
            .update_playback(DiscordPlaybackState::playing(sample_track(), 20.0, 1.5))
            .await;

        sleep(Duration::from_millis(80)).await;
        handle.clear_activity().await;
        sleep(Duration::from_millis(80)).await;
        handle.shutdown().await;

        let frames = server_task.await.unwrap();
        let playing_frame = frames
            .iter()
            .find(|p| p.payload["args"]["activity"]["details"] == "Midnight City [1.5x]")
            .expect("expected SET_ACTIVITY frame with speed badge");
        assert_eq!(playing_frame.payload["args"]["activity"]["type"], 2);
        assert_eq!(playing_frame.payload["args"]["activity"]["state"], "M83");
        assert!(playing_frame.payload["args"]["activity"]["timestamps"]["start"].is_i64());
        assert!(playing_frame.payload["args"]["activity"]["timestamps"]["end"].is_i64());
        assert!(frames
            .last()
            .is_some_and(|p| p.payload["args"].get("activity").is_none()));

        let _ = std::fs::remove_dir_all(&sock_dir);
    }

    #[tokio::test]
    async fn controller_handles_absent_discord_client_gracefully() {
        let handle =
            DiscordRpcHandle::spawn_with_socket_discovery(DiscordConfig::default(), Vec::new);
        handle
            .update_playback(DiscordPlaybackState::playing(sample_track(), 10.0, 1.0))
            .await;
        sleep(Duration::from_millis(40)).await;
        assert_eq!(handle.status(), DiscordRpcStatus::Disconnected);
        handle.shutdown().await;
    }
}
