use crate::config::EnhancerMode;
use crate::providers::hifi::ResolvedStream;
use crate::redaction::{
    json_payload as sanitize_payload, sensitive_text, stream_url as redact_mpv_url,
};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub enum MpvEvent {
    Position(f64),
    Duration(f64),
    Paused(bool),
    PlaybackStarted,
    TrackEnded,
    PlaybackError { error: String, file: Option<String> },
}

pub struct MpvController {
    child: Mutex<Option<Child>>,
    cmd_tx: mpsc::UnboundedSender<Value>,
    sock_path: PathBuf,
}

impl MpvController {
    pub async fn spawn(
        volume: u8,
        enhancer: EnhancerMode,
        event_tx: mpsc::Sender<MpvEvent>,
    ) -> Result<Self> {
        Self::spawn_inner(volume, enhancer, event_tx).await
    }

    pub async fn spawn_desktop(
        volume: u8,
        enhancer: EnhancerMode,
        event_tx: mpsc::Sender<MpvEvent>,
    ) -> Result<Self> {
        Self::spawn_inner(volume, enhancer, event_tx).await
    }

    async fn spawn_inner(
        volume: u8,
        enhancer: EnhancerMode,
        event_tx: mpsc::Sender<MpvEvent>,
    ) -> Result<Self> {
        let pid = std::process::id();
        let sock_dir = dirs::runtime_dir().unwrap_or_else(crate::config::AppConfig::cache_dir);
        let _ = std::fs::create_dir_all(&sock_dir);
        let sock_path = sock_dir.join(format!("sonata-mpv-{pid}.sock"));
        let _ = std::fs::remove_file(&sock_path);

        let mut cmd = Command::new("mpv");
        cmd.args([
            "--idle=yes",
            "--no-video",
            "--audio-display=no",
            "--really-quiet",
            "--gapless-audio=yes",
            "--cache=yes",
            "--cache-secs=30",
            "--demuxer-max-bytes=64M",
            "--network-timeout=8",
            &format!("--volume={volume}"),
            &format!("--input-ipc-server={}", sock_path.display()),
        ]);
        // The resolver supplies a direct media URL; mpv must not re-resolve it as a page.
        cmd.arg("--ytdl=no");
        let af = enhancer.mpv_af();
        if !af.is_empty() {
            cmd.arg(format!("--af={af}"));
        }
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
            if let Ok(s) = UnixStream::connect(&sock_path).await {
                stream_opt = Some(s);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let Some(stream) = stream_opt else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&sock_path);
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
                if write_half.write_all(payload.as_bytes()).await.is_err() {
                    break;
                }
                if write_half.flush().await.is_err() {
                    break;
                }
            }
        });

        let ctrl = Self {
            child: Mutex::new(Some(child)),
            cmd_tx,
            sock_path,
        };

        // Make mpv snitch on property changes so we never have to poll
        ctrl.send(json!(["observe_property", 1, "time-pos"]));
        ctrl.send(json!(["observe_property", 2, "duration"]));
        ctrl.send(json!(["observe_property", 3, "pause"]));

        tokio::spawn(async move {
            let mut lines = BufReader::new(read_half).lines();
            while let Ok(Some(line)) = lines.next_line().await {
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
                                    let _ = event_tx.try_send(MpvEvent::Duration(dur));
                                }
                            }
                            "pause" => {
                                if let Some(p) = data.and_then(|d| d.as_bool()) {
                                    let _ = event_tx.try_send(MpvEvent::Paused(p));
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

        Ok(ctrl)
    }

    fn send(&self, command: Value) {
        let _ = self.cmd_tx.send(command);
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
        tracing::debug!(
            event = "mpv_load_media",
            start_secs,
            media = %redact_mpv_url(url),
            "loading media into mpv"
        );
        let mut options = serde_json::Map::new();
        if start_secs >= 0.35 {
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
        self.send(json!(["set_property", "pause", false]));
    }

    pub fn stop(&self) {
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
        self.send(json!(["set_property", "volume", volume as f64]));
    }

    pub fn set_enhancer(&self, mode: EnhancerMode) {
        self.send(json!(["set_property", "af", mode.mpv_af()]));
    }

    pub async fn shutdown(&self) {
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
