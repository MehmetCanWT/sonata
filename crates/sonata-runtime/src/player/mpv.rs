//! mpv process management and IPC control for Sonata bit-perfect playback.
//!
//! 1. **Process Spawn**: starts a headless `mpv` (`--no-config`, `--idle`, direct PipeWire/PulseAudio/ALSA output, gapless, `--ytdl=no`) with a
//!    per-process JSON IPC socket in the runtime directory and configurable audio/video options.
//! 2. **Event Stream**: a reader task parses IPC messages into `MpvEvent`s (including live `audio-bitrate`,
//!    `audio-codec-name`, and `audio-params` telemetry) sent to the front-end; mpv's stderr is forwarded to tracing after redaction.
//! 3. **Quality Formatting**: `format_measured_audio_badge` and `compute_local_file_bitrate_kbps` turn live
//!    codec, bitrate, bit-depth, and sample-rate telemetry into crisp quality badges.
//! 4. **Commands & Modes**: `MpvController` exposes load, stop, pause, seek, volume, playback speed (`0.5..=2.0`),
//!    skip-silence `af` filtering, and video-mode reload (`--vid=auto` / `--no-video`).
//! 5. **Lifecycle**: `shutdown` and `Drop` terminate the child process and clean up the socket.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sonata_core::providers::hifi::{extract_badge_kbps, ResolvedStream};
use sonata_core::redaction::{
    json_payload as sanitize_payload, sensitive_text, stream_url as redact_mpv_url,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};

pub const MIN_MPV_PLAYBACK_SPEED: f64 = 0.5;
pub const MAX_MPV_PLAYBACK_SPEED: f64 = 2.0;
pub const SKIP_SILENCE_AF_FILTER: &str =
    "lavfi=[silenceremove=stop_periods=-1:stop_duration=0.45:stop_threshold=-42dB]";

/// Clamps playback speed to `[0.5, 2.0]`, defaulting non-finite inputs to `1.0`.
pub fn clamp_playback_speed(speed: f64) -> f64 {
    if !speed.is_finite() {
        return 1.0;
    }
    speed.clamp(MIN_MPV_PLAYBACK_SPEED, MAX_MPV_PLAYBACK_SPEED)
}

/// Returns the active `mpv` audio-filter property string for skip-silence (automatically disabled during video playback).
pub fn skip_silence_af_filter(enabled: bool, video_mode: bool) -> &'static str {
    if enabled && !video_mode {
        SKIP_SILENCE_AF_FILTER
    } else {
        ""
    }
}

/// Decoded `mpv` `audio-params` property values.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MpvAudioParams {
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u8>,
    pub format: Option<String>,
}

/// Maps an `mpv` `audio-params/format` string (such as `"s16"`, `"s24"`, `"s32"`, `"float"`) to bit depth.
pub fn bit_depth_from_mpv_format(format: &str) -> Option<u8> {
    let lower = format.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if lower.starts_with("s16") || lower.starts_with("u16") {
        Some(16)
    } else if lower.starts_with("s24") || lower.starts_with("u24") {
        Some(24)
    } else if lower.starts_with("s32") || lower.starts_with("u32") || lower.starts_with("float") {
        Some(32)
    } else if lower.starts_with("double") {
        Some(64)
    } else if lower.starts_with("u8") || lower.starts_with("s8") {
        Some(8)
    } else {
        None
    }
}

/// Parses an `mpv` `audio-params` JSON value into [`MpvAudioParams`].
pub fn parse_mpv_audio_params(data: Option<&Value>) -> Option<MpvAudioParams> {
    let obj = data?.as_object()?;
    let sample_rate_hz = obj
        .get("samplerate")
        .and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_f64()
                    .filter(|f| f.is_finite() && *f > 0.0)
                    .map(|f| f.round() as u64)
            })
        })
        .and_then(|r| u32::try_from(r).ok())
        .filter(|&r| r > 0);
    let format = obj
        .get("format")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let bit_depth = format.as_deref().and_then(bit_depth_from_mpv_format);
    if sample_rate_hz.is_none() && bit_depth.is_none() && format.is_none() {
        return None;
    }
    Some(MpvAudioParams {
        sample_rate_hz,
        bit_depth,
        format,
    })
}

/// Computes measured bitrate in kbps from raw file size in bytes and duration in seconds.
pub fn compute_bitrate_kbps_from_bytes(file_bytes: u64, duration_secs: f64) -> Option<u32> {
    if file_bytes == 0 || !duration_secs.is_finite() || duration_secs <= 0.5 {
        return None;
    }
    let kbps = ((file_bytes as f64) * 8.0 / duration_secs / 1000.0).round();
    if !kbps.is_finite() || kbps < 1.0 {
        return None;
    }
    Some(kbps.min(u32::MAX as f64) as u32)
}

/// Computes measured bitrate in kbps from a local audio file on disk and its duration in seconds.
pub fn compute_local_file_bitrate_kbps(local_path: &Path, duration_secs: f64) -> Option<u32> {
    let bytes = std::fs::metadata(local_path).ok()?.len();
    compute_bitrate_kbps_from_bytes(bytes, duration_secs)
}

/// Normalizes an `mpv` codec identifier into a crisp display label.
pub fn normalize_audio_codec_label(codec: &str) -> String {
    let trimmed = codec.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "flac" => "FLAC".to_string(),
        "opus" => "OPUS".to_string(),
        "aac" => "AAC".to_string(),
        "alac" => "ALAC".to_string(),
        "mp3" | "mp3float" => "MP3".to_string(),
        "vorbis" => "VORBIS".to_string(),
        "wavpack" => "WV".to_string(),
        "ape" => "APE".to_string(),
        s if s.starts_with("pcm_") => "PCM".to_string(),
        s if s.starts_with("dsd_") => "DSD".to_string(),
        _ => trimmed.to_ascii_uppercase(),
    }
}

/// Returns whether a normalized or raw codec name represents a lossless audio format.
pub fn is_lossless_audio_codec(codec: &str) -> bool {
    let lower = codec.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "flac" | "alac" | "wav" | "aiff" | "pcm" | "dsd" | "wv" | "wavpack" | "ape"
    ) || lower.starts_with("pcm_")
        || lower.starts_with("dsd_")
}

/// Formats a sample rate in Hz as a compact kHz label (`44100` -> `"44.1kHz"`, `96000` -> `"96kHz"`).
pub fn format_sample_rate_khz(sample_rate_hz: u32) -> String {
    if sample_rate_hz.is_multiple_of(1000) {
        format!("{}kHz", sample_rate_hz / 1000)
    } else {
        let khz = sample_rate_hz as f64 / 1000.0;
        let formatted = format!("{khz:.1}");
        format!(
            "{}kHz",
            formatted.trim_end_matches('0').trim_end_matches('.')
        )
    }
}

fn infer_codec_from_badge(badge: &str) -> Option<String> {
    let token = badge
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|t| !t.is_empty())?;
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "FLAC" | "OPUS" | "AAC" | "ALAC" | "MP3" | "VORBIS" | "PCM" | "WAV" | "AIFF" | "DSD"
        | "APE" | "WV" => Some(upper),
        _ => None,
    }
}

/// Formats a measured audio quality badge such as `FLAC · 880 kbps · 16-bit/44.1kHz`,
/// `FLAC · 2450 kbps · 24-bit/96kHz`, `OPUS · 256 kbps`, or `AAC · 320 kbps`.
pub fn format_measured_audio_badge(
    codec: Option<&str>,
    bitrate_kbps: Option<u32>,
    bit_depth: Option<u8>,
    sample_rate_hz: Option<u32>,
    fallback_badge: Option<&str>,
) -> Option<String> {
    let codec_label = codec
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_audio_codec_label)
        .or_else(|| fallback_badge.and_then(infer_codec_from_badge))?;

    let resolved_kbps = bitrate_kbps
        .filter(|&k| k > 0)
        .or_else(|| fallback_badge.and_then(extract_badge_kbps));

    let mut parts = vec![codec_label.clone()];
    if let Some(kbps) = resolved_kbps {
        parts.push(format!("{kbps} kbps"));
    }

    if is_lossless_audio_codec(&codec_label) {
        let pcm_detail = match (
            bit_depth.filter(|&d| d > 0),
            sample_rate_hz.filter(|&r| r > 0),
        ) {
            (Some(depth), Some(rate)) => {
                Some(format!("{depth}-bit/{}", format_sample_rate_khz(rate)))
            }
            (Some(depth), None) => Some(format!("{depth}-bit")),
            (None, Some(rate)) => Some(format_sample_rate_khz(rate)),
            (None, None) => None,
        };
        if let Some(detail) = pcm_detail {
            parts.push(detail);
        }
    }

    if parts.len() == 1 && resolved_kbps.is_none() {
        return None;
    }

    Some(parts.join(" · "))
}

/// Builds the `mpv` CLI argument list for spawning an IPC-controlled player instance.
pub fn build_mpv_spawn_args(
    volume: u8,
    speed: f64,
    skip_silence: bool,
    video_mode: bool,
    sock_path: &Path,
) -> Vec<String> {
    let clamped_speed = clamp_playback_speed(speed);
    let mut args = vec![
        "--no-config".to_string(),
        "--idle=yes".to_string(),
        "--really-quiet".to_string(),
        "--gapless-audio=yes".to_string(),
        "--ao=pipewire,pulse,alsa".to_string(),
        "--audio-client-name=Sonata".to_string(),
        "--aid=auto".to_string(),
        "--mute=no".to_string(),
        "--cache=yes".to_string(),
        "--cache-secs=30".to_string(),
        "--demuxer-max-bytes=64M".to_string(),
        "--network-timeout=8".to_string(),
        "--force-seekable=yes".to_string(),
        "--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_on_network_error=1,reconnect_delay_max=2".to_string(),
        format!("--volume={volume}"),
        format!("--speed={clamped_speed:.2}"),
        format!("--input-ipc-server={}", sock_path.display()),
        // The resolver supplies a direct media URL; mpv must not re-resolve it as a page.
        "--ytdl=no".to_string(),
    ];
    if video_mode {
        args.push("--vid=auto".to_string());
    } else {
        args.push("--no-video".to_string());
        args.push("--audio-display=no".to_string());
    }
    let af = skip_silence_af_filter(skip_silence, video_mode);
    if !af.is_empty() {
        args.push(format!("--af={af}"));
    }
    args
}

#[derive(Debug, Clone)]
pub enum MpvEvent {
    Position(f64),
    Duration(f64),
    Paused(bool),
    PlaybackStarted,
    TrackEnded,
    PlaybackError { error: String, file: Option<String> },
    AudioBitrate(Option<u64>),
    AudioCodec(Option<String>),
    AudioParams(Option<MpvAudioParams>),
}

#[derive(Debug, Clone)]
struct MpvRuntimeState {
    volume: u8,
    speed: f64,
    skip_silence: bool,
    video_mode: bool,
    last_url: Option<String>,
    last_headers: Vec<(String, String)>,
    ipc_generation: u64,
}

pub struct MpvController {
    child: Mutex<Option<Child>>,
    cmd_tx: std::sync::Mutex<mpsc::UnboundedSender<Value>>,
    sock_path: PathBuf,
    event_tx: mpsc::Sender<MpvEvent>,
    runtime: Arc<std::sync::Mutex<MpvRuntimeState>>,
}

struct SpawnedMpvProcess {
    child: Child,
    cmd_tx: mpsc::UnboundedSender<Value>,
}

impl MpvController {
    pub async fn spawn(volume: u8, event_tx: mpsc::Sender<MpvEvent>) -> Result<Self> {
        Self::spawn_inner(volume, event_tx).await
    }

    pub async fn spawn_desktop(volume: u8, event_tx: mpsc::Sender<MpvEvent>) -> Result<Self> {
        Self::spawn_inner(volume, event_tx).await
    }

    async fn spawn_inner(volume: u8, event_tx: mpsc::Sender<MpvEvent>) -> Result<Self> {
        let pid = std::process::id();
        let sock_dir = dirs::runtime_dir().unwrap_or_else(crate::config::AppConfig::cache_dir);
        let _ = std::fs::create_dir_all(&sock_dir);
        let sock_path = sock_dir.join(format!("sonata-mpv-{pid}.sock"));
        let runtime = Arc::new(std::sync::Mutex::new(MpvRuntimeState {
            volume: volume.min(100),
            speed: 1.0,
            skip_silence: false,
            video_mode: false,
            last_url: None,
            last_headers: Vec::new(),
            ipc_generation: 1,
        }));

        let spawned = Self::spawn_process(&sock_path, &runtime, event_tx.clone(), 1).await?;

        Ok(Self {
            child: Mutex::new(Some(spawned.child)),
            cmd_tx: std::sync::Mutex::new(spawned.cmd_tx),
            sock_path,
            event_tx,
            runtime,
        })
    }

    async fn spawn_process(
        sock_path: &Path,
        runtime: &Arc<std::sync::Mutex<MpvRuntimeState>>,
        event_tx: mpsc::Sender<MpvEvent>,
        ipc_generation: u64,
    ) -> Result<SpawnedMpvProcess> {
        let _ = std::fs::remove_file(sock_path);
        let (volume, speed, skip_silence, video_mode) = {
            let guard = runtime.lock().expect("mpv runtime lock");
            (
                guard.volume,
                guard.speed,
                guard.skip_silence,
                guard.video_mode,
            )
        };

        let mpv_bin = crate::config::resolve_distro_binary("mpv");
        let args = build_mpv_spawn_args(volume, speed, skip_silence, video_mode, sock_path);
        let mut cmd = Command::new(&mpv_bin);
        cmd.args(&args);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to start mpv (is mpv installed?): {e}"))?;
        let mpv_pid = child.id();
        tracing::info!(
            event = "mpv_started",
            pid = mpv_pid,
            video_mode,
            ipc_path = %sock_path.display(),
            "mpv process started"
        );
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let clean = sensitive_text(&line);
                    if !clean.trim().is_empty() {
                        tracing::warn!(
                            event = "mpv_stderr",
                            pid = mpv_pid,
                            message = clean.trim(),
                            "mpv reported a diagnostic"
                        );
                    }
                }
                tracing::debug!(
                    event = "mpv_stderr_closed",
                    pid = mpv_pid,
                    "mpv stderr closed"
                );
            });
        }

        let mut stream_opt = None;
        for _ in 0..40 {
            if let Ok(s) = UnixStream::connect(sock_path).await {
                stream_opt = Some(s);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let Some(stream) = stream_opt else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(sock_path);
            return Err(anyhow!("Timed out waiting for mpv IPC socket"));
        };
        let (read_half, mut write_half) = stream.into_split();

        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Value>();
        tokio::spawn(async move {
            let mut req_id: u64 = 1;
            while let Some(command) = cmd_rx.recv().await {
                let id = req_id;
                req_id = req_id.wrapping_add(1);
                let payload = format!("{}\n", json!({ "command": command, "request_id": id }));
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let safe_command = sanitize_payload(&json!({ "command": command }));
                    tracing::debug!(
                        event = "mpv_ipc_send",
                        request_id = id,
                        command = %safe_command
                            .get("command")
                            .map(|v| serde_json::to_string(v)
                                .unwrap_or_else(|_| "[invalid]".to_string()))
                            .unwrap_or_else(|| "[invalid]".to_string())
                            .replace(&json!(null).to_string(), "[null]"),
                        "sending mpv command"
                    );
                }
                if write_half.write_all(payload.as_bytes()).await.is_err() {
                    break;
                }
                if write_half.flush().await.is_err() {
                    break;
                }
            }
        });

        // Observe playback and audio quality properties so we never have to poll.
        for (id, prop) in [
            (1, "time-pos"),
            (2, "duration"),
            (3, "pause"),
            (4, "audio-bitrate"),
            (5, "audio-codec-name"),
            (6, "audio-params"),
        ] {
            let _ = cmd_tx.send(json!(["observe_property", id, prop]));
        }

        let runtime_reader = Arc::clone(runtime);
        tokio::spawn(async move {
            let mut lines = BufReader::new(read_half).lines();
            let mut last_bitrate_kbps: Option<Option<u32>> = None;
            let mut last_codec: Option<Option<String>> = None;
            let mut last_params: Option<Option<MpvAudioParams>> = None;
            while let Ok(Some(line)) = lines.next_line().await {
                if runtime_reader
                    .lock()
                    .is_ok_and(|g| g.ipc_generation != ipc_generation)
                {
                    break;
                }
                let Ok(val) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                match val.get("event").and_then(|e| e.as_str()) {
                    Some("end-file") => {
                        tracing::debug!(
                            event = "mpv_end_file",
                            payload = %sanitize_payload(&val),
                            "mpv ended media"
                        );
                    }
                    Some("property-change") => {}
                    Some(event) => {
                        tracing::debug!(event = "mpv_event", mpv_event = event, "mpv event");
                    }
                    None => {}
                }
                match val.get("event").and_then(|e| e.as_str()) {
                    Some("property-change") => {
                        let name = val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let data = val.get("data");
                        match name {
                            "time-pos" => {
                                if let Some(pos) = data.and_then(|d| d.as_f64()) {
                                    let _ = event_tx.try_send(MpvEvent::Position(pos));
                                }
                            }
                            "duration" => {
                                if let Some(dur) = data.and_then(|d| d.as_f64()) {
                                    let _ = event_tx.send(MpvEvent::Duration(dur)).await;
                                }
                            }
                            "pause" => {
                                if let Some(p) = data.and_then(|d| d.as_bool()) {
                                    let _ = event_tx.send(MpvEvent::Paused(p)).await;
                                }
                            }
                            "audio-bitrate" => {
                                let bps = data
                                    .and_then(|d| {
                                        d.as_u64().or_else(|| {
                                            d.as_f64()
                                                .filter(|f| f.is_finite() && *f >= 0.0)
                                                .map(|f| f.round() as u64)
                                        })
                                    })
                                    .filter(|&v| v > 0);
                                let kbps = bps
                                    .map(|v| ((v as f64) / 1000.0).round() as u32)
                                    .filter(|&k| k > 0);
                                if last_bitrate_kbps != Some(kbps) {
                                    last_bitrate_kbps = Some(kbps);
                                    let _ = event_tx.send(MpvEvent::AudioBitrate(bps)).await;
                                }
                            }
                            "audio-codec-name" => {
                                let codec = data
                                    .and_then(|d| d.as_str())
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string);
                                if last_codec.as_ref() != Some(&codec) {
                                    last_codec = Some(codec.clone());
                                    let _ = event_tx.send(MpvEvent::AudioCodec(codec)).await;
                                }
                            }
                            "audio-params" => {
                                let params = parse_mpv_audio_params(data);
                                if last_params.as_ref() != Some(&params) {
                                    last_params = Some(params.clone());
                                    let _ = event_tx.send(MpvEvent::AudioParams(params)).await;
                                }
                            }
                            _ => {}
                        }
                    }
                    Some("end-file") => match val.get("reason").and_then(|r| r.as_str()) {
                        Some("eof") => {
                            let _ = event_tx.send(MpvEvent::TrackEnded).await;
                        }
                        Some("error") => {
                            let msg = val
                                .get("file_error")
                                .and_then(|e| e.as_str())
                                .unwrap_or("mpv playback error")
                                .to_string();
                            let file = val
                                .pointer("/playlist_entry/filename")
                                .and_then(|v| v.as_str())
                                .or_else(|| val.get("file").and_then(|v| v.as_str()))
                                .map(str::to_string);
                            let _ = event_tx
                                .send(MpvEvent::PlaybackError { error: msg, file })
                                .await;
                        }
                        _ => {}
                    },
                    Some("playback-restart") => {
                        let _ = event_tx.send(MpvEvent::PlaybackStarted).await;
                    }
                    _ => {}
                }
            }
        });

        Ok(SpawnedMpvProcess { child, cmd_tx })
    }

    fn send(&self, command: Value) {
        if let Ok(tx) = self.cmd_tx.lock() {
            let _ = tx.send(command);
        }
    }

    pub fn load_url(&self, url: &str) {
        self.load_url_at(url, 0.0);
    }

    pub fn load_url_at(&self, url: &str, start_secs: f64) {
        self.load_media_at(url, &[], start_secs);
    }

    pub fn load_stream(&self, stream: &ResolvedStream) {
        self.load_stream_at(stream, 0.0);
    }

    pub fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64) {
        self.load_media_at(&stream.url, &stream.headers, start_secs);
    }

    fn load_media_at(&self, url: &str, headers: &[(String, String)], start_secs: f64) {
        if let Ok(mut guard) = self.runtime.lock() {
            guard.last_url = Some(url.to_string());
            guard.last_headers = headers.to_vec();
        }
        tracing::debug!(
            event = "mpv_load_media",
            start_secs,
            media = %redact_mpv_url(url),
            "loading media into mpv"
        );
        let mut options = serde_json::Map::new();
        if start_secs >= 0.05 {
            options.insert("start".to_string(), json!(format!("{start_secs:.2}")));
        }
        if !headers.is_empty() {
            let fields = headers
                .iter()
                .map(|(name, value)| {
                    let escape = |text: &str| text.replace('\\', "\\\\").replace(',', "\\,");
                    format!("{}={}", escape(name), escape(value))
                })
                .collect::<Vec<_>>()
                .join(",");
            options.insert("http-header-fields".to_string(), json!(fields));
        }
        if !options.is_empty() {
            self.send(json!([
                "loadfile",
                url,
                "replace",
                -1,
                Value::Object(options)
            ]));
        } else {
            self.send(json!(["loadfile", url, "replace"]));
        }
        self.send(json!(["set_property", "mute", false]));
        self.send(json!(["set_property", "pause", false]));
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.runtime.lock() {
            guard.last_url = None;
            guard.last_headers.clear();
        }
        self.send(json!(["stop"]));
    }

    pub fn set_paused(&self, paused: bool) {
        self.send(json!(["set_property", "pause", paused]));
    }

    pub fn seek_relative(&self, secs: f64) {
        self.send(json!(["seek", secs, "relative"]));
    }

    pub fn seek_absolute(&self, secs: f64) {
        self.send(json!(["seek", secs, "absolute"]));
    }

    pub fn set_volume(&self, volume: u8) {
        let clamped = volume.min(100);
        if let Ok(mut guard) = self.runtime.lock() {
            guard.volume = clamped;
        }
        self.send(json!(["set_property", "volume", clamped as f64]));
    }

    pub fn set_speed(&self, speed: f64) {
        let clamped = clamp_playback_speed(speed);
        if let Ok(mut guard) = self.runtime.lock() {
            guard.speed = clamped;
        }
        self.send(json!(["set_property", "speed", clamped]));
    }

    pub fn set_skip_silence(&self, enabled: bool) {
        let video_mode = if let Ok(mut guard) = self.runtime.lock() {
            guard.skip_silence = enabled;
            guard.video_mode
        } else {
            false
        };
        let filter = skip_silence_af_filter(enabled, video_mode);
        self.send(json!(["set_property", "af", filter]));
    }

    /// Reloads the `mpv` child process with `--vid=auto` or `--no-video`, preserving playback position and headers.
    pub async fn set_video_mode(
        &self,
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    ) -> Result<()> {
        let mut child_guard = self.child.lock().await;
        let (next_gen, target_media) = {
            let mut state = self.runtime.lock().expect("mpv runtime lock");
            state.ipc_generation = state.ipc_generation.wrapping_add(1);
            state.video_mode = enabled;
            let resolved_media = match url {
                Some(u) if !u.is_empty() => {
                    state.last_url = Some(u.clone());
                    state.last_headers = headers.clone();
                    Some((u, headers))
                }
                _ => state.last_url.clone().map(|prev| {
                    let hdrs = if headers.is_empty() {
                        state.last_headers.clone()
                    } else {
                        state.last_headers = headers.clone();
                        headers
                    };
                    (prev, hdrs)
                }),
            };
            (state.ipc_generation, resolved_media)
        };

        self.send(json!(["quit"]));
        tokio::time::sleep(Duration::from_millis(40)).await;
        if let Some(mut old_child) = child_guard.take() {
            if old_child.try_wait().ok().flatten().is_none() {
                let _ = old_child.kill();
                let _ = old_child.wait();
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);

        let spawned = Self::spawn_process(
            &self.sock_path,
            &self.runtime,
            self.event_tx.clone(),
            next_gen,
        )
        .await
        .map_err(|err| anyhow!("Failed to reload mpv for video mode: {err}"))?;

        *child_guard = Some(spawned.child);
        if let Ok(mut tx_guard) = self.cmd_tx.lock() {
            *tx_guard = spawned.cmd_tx;
        }
        drop(child_guard);

        if let Some((media_url, media_headers)) = target_media {
            self.load_media_at(&media_url, &media_headers, seek_secs.unwrap_or(0.0));
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        if let Ok(mut guard) = self.runtime.lock() {
            guard.ipc_generation = guard.ipc_generation.wrapping_add(1);
        }
        self.send(json!(["quit"]));
        tokio::time::sleep(Duration::from_millis(40)).await;
        let mut guard = self.child.lock().await;
        if let Some(mut c) = guard.take() {
            if c.try_wait().ok().flatten().is_none() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
    }
}

impl Drop for MpvController {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.try_lock() {
            if let Some(mut c) = guard.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bit_depth_from_mpv_format, build_mpv_spawn_args, clamp_playback_speed,
        compute_bitrate_kbps_from_bytes, compute_local_file_bitrate_kbps,
        format_measured_audio_badge, format_sample_rate_khz, parse_mpv_audio_params,
        skip_silence_af_filter, MpvAudioParams, SKIP_SILENCE_AF_FILTER,
    };
    use crate::downloads::test_support::TempTestDir;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn formats_measured_flac_opus_and_aac_badges() {
        assert_eq!(
            format_measured_audio_badge(Some("flac"), Some(880), Some(16), Some(44100), None),
            Some("FLAC · 880 kbps · 16-bit/44.1kHz".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(Some("flac"), Some(720), Some(16), Some(44100), None),
            Some("FLAC · 720 kbps · 16-bit/44.1kHz".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(Some("flac"), Some(2450), Some(24), Some(96000), None),
            Some("FLAC · 2450 kbps · 24-bit/96kHz".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(Some("opus"), Some(256), Some(32), Some(48000), None),
            Some("OPUS · 256 kbps".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(Some("aac"), Some(320), Some(16), Some(44100), None),
            Some("AAC · 320 kbps".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(
                Some("opus"),
                None,
                None,
                None,
                Some("OPUS 256kbps · Extreme")
            ),
            Some("OPUS · 256 kbps".to_string())
        );
        assert_eq!(
            format_measured_audio_badge(None, None, None, None, Some("Stopped")),
            None
        );
    }

    #[test]
    fn computes_measured_bitrate_from_local_file_size() {
        // 19,800,000 bytes over 180 seconds = 880 kbps exact
        assert_eq!(
            compute_bitrate_kbps_from_bytes(19_800_000, 180.0),
            Some(880)
        );
        // 16,200,000 bytes over 180 seconds = 720 kbps exact
        assert_eq!(
            compute_bitrate_kbps_from_bytes(16_200_000, 180.0),
            Some(720)
        );
        assert_eq!(compute_bitrate_kbps_from_bytes(0, 180.0), None);
        assert_eq!(compute_bitrate_kbps_from_bytes(1024, 0.0), None);

        let dir = TempTestDir::new("mpv_bitrate");
        let file_path = dir.path().join("track.flac");
        // 220_000 bytes over 2.0 seconds = 880 kbps
        std::fs::write(&file_path, vec![0u8; 220_000]).expect("write test audio file");
        assert_eq!(compute_local_file_bitrate_kbps(&file_path, 2.0), Some(880));
    }

    #[test]
    fn parses_mpv_audio_params_and_bit_depth() {
        assert_eq!(bit_depth_from_mpv_format("s16"), Some(16));
        assert_eq!(bit_depth_from_mpv_format("s16p"), Some(16));
        assert_eq!(bit_depth_from_mpv_format("s24"), Some(24));
        assert_eq!(bit_depth_from_mpv_format("s32p"), Some(32));
        assert_eq!(bit_depth_from_mpv_format("float"), Some(32));
        assert_eq!(format_sample_rate_khz(44100), "44.1kHz");
        assert_eq!(format_sample_rate_khz(48000), "48kHz");
        assert_eq!(format_sample_rate_khz(96000), "96kHz");

        let parsed = parse_mpv_audio_params(Some(&json!({
            "format": "s24",
            "samplerate": 96000,
            "channels": "stereo"
        })));
        assert_eq!(
            parsed,
            Some(MpvAudioParams {
                sample_rate_hz: Some(96000),
                bit_depth: Some(24),
                format: Some("s24".to_string()),
            })
        );
        assert_eq!(parse_mpv_audio_params(Some(&json!(null))), None);
    }

    #[test]
    fn clamps_speed_and_formats_skip_silence_and_video_args() {
        assert_eq!(clamp_playback_speed(0.1), 0.5);
        assert_eq!(clamp_playback_speed(1.25), 1.25);
        assert_eq!(clamp_playback_speed(3.5), 2.0);
        assert_eq!(clamp_playback_speed(f64::NAN), 1.0);

        assert_eq!(skip_silence_af_filter(false, false), "");
        assert_eq!(skip_silence_af_filter(true, false), SKIP_SILENCE_AF_FILTER);
        assert_eq!(skip_silence_af_filter(true, true), "");

        let audio_args = build_mpv_spawn_args(85, 1.5, true, false, Path::new("/tmp/test.sock"));
        assert!(audio_args.contains(&"--no-video".to_string()));
        assert!(audio_args.contains(&"--audio-display=no".to_string()));
        assert!(audio_args.contains(&"--speed=1.50".to_string()));
        assert!(audio_args.contains(&format!("--af={SKIP_SILENCE_AF_FILTER}")));

        let video_args = build_mpv_spawn_args(85, 1.25, true, true, Path::new("/tmp/test.sock"));
        assert!(video_args.contains(&"--vid=auto".to_string()));
        assert!(!video_args.contains(&"--no-video".to_string()));
        assert!(!video_args.iter().any(|arg| arg.starts_with("--af=")));
    }
}
