use crate::config::EnhancerMode;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub enum MpvEvent {
    Position(f64),
    Duration(f64),
    Paused(bool),
    TrackEnded,
}

pub struct MpvController {
    child: Mutex<Option<Child>>,
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    sock_path: PathBuf,
    req_id: AtomicU64,
}

impl MpvController {
    pub async fn spawn(
        volume: u8,
        enhancer: EnhancerMode,
        event_tx: mpsc::UnboundedSender<MpvEvent>,
    ) -> Result<Self> {
        let pid = std::process::id();
        let sock_path = PathBuf::from(format!("/tmp/sonata-mpv-{pid}.sock"));
        let _ = std::fs::remove_file(&sock_path);

        let mut cmd = Command::new("mpv");
        cmd.args([
            "--idle=yes",
            "--no-video",
            "--audio-display=no",
            "--really-quiet",
            "--gapless-audio=yes",
            "--cache=yes",
            "--demuxer-max-bytes=64M",
            &format!("--volume={volume}"),
            &format!("--input-ipc-server={}", sock_path.display()),
        ]);
        let af = enhancer.mpv_af();
        if !af.is_empty() {
            cmd.arg(format!("--af={af}"));
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to start mpv (is mpv installed?): {e}"))?;

        let mut stream_opt = None;
        for _ in 0..40 {
            if let Ok(s) = UnixStream::connect(&sock_path).await {
                stream_opt = Some(s);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let stream = stream_opt.ok_or_else(|| anyhow!("Timed out waiting for mpv IPC socket"))?;
        let (read_half, write_half) = stream.into_split();

        let ctrl = Self {
            child: Mutex::new(Some(child)),
            writer: Arc::new(Mutex::new(Some(write_half))),
            sock_path,
            req_id: AtomicU64::new(1),
        };

        // Observe key properties so mpv pushes updates only on change (zero polling)
        ctrl.send_command(json!(["observe_property", 1, "time-pos"]))
            .await?;
        ctrl.send_command(json!(["observe_property", 2, "duration"]))
            .await?;
        ctrl.send_command(json!(["observe_property", 3, "pause"]))
            .await?;

        tokio::spawn(async move {
            let mut lines = BufReader::new(read_half).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(val) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                match val.get("event").and_then(|e| e.as_str()) {
                    Some("property-change") => {
                        let name = val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let data = val.get("data");
                        match name {
                            "time-pos" => {
                                if let Some(pos) = data.and_then(|d| d.as_f64()) {
                                    let _ = event_tx.send(MpvEvent::Position(pos));
                                }
                            }
                            "duration" => {
                                if let Some(dur) = data.and_then(|d| d.as_f64()) {
                                    let _ = event_tx.send(MpvEvent::Duration(dur));
                                }
                            }
                            "pause" => {
                                if let Some(p) = data.and_then(|d| d.as_bool()) {
                                    let _ = event_tx.send(MpvEvent::Paused(p));
                                }
                            }
                            _ => {}
                        }
                    }
                    Some("end-file")
                        if val.get("reason").and_then(|r| r.as_str()) == Some("eof") =>
                    {
                        let _ = event_tx.send(MpvEvent::TrackEnded);
                    }
                    _ => {}
                }
            }
        });

        Ok(ctrl)
    }

    async fn send_command(&self, command: Value) -> Result<()> {
        let id = self.req_id.fetch_add(1, Ordering::Relaxed);
        let payload = format!("{}\n", json!({ "command": command, "request_id": id }));
        let mut guard = self.writer.lock().await;
        if let Some(w) = guard.as_mut() {
            w.write_all(payload.as_bytes()).await?;
            w.flush().await?;
        }
        Ok(())
    }

    pub async fn load_url(&self, url: &str) -> Result<()> {
        self.send_command(json!(["loadfile", url, "replace"])).await?;
        self.send_command(json!(["set_property", "pause", false])).await
    }

    pub async fn load_url_at(&self, url: &str, start_secs: f64) -> Result<()> {
        self.load_url(url).await?;
        if start_secs >= 0.5 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _ = self.seek_absolute(start_secs).await;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.send_command(json!(["stop"])).await
    }

    pub async fn set_paused(&self, paused: bool) -> Result<()> {
        self.send_command(json!(["set_property", "pause", paused])).await
    }

    pub async fn seek_relative(&self, secs: f64) -> Result<()> {
        self.send_command(json!(["seek", secs, "relative"])).await
    }

    pub async fn seek_absolute(&self, secs: f64) -> Result<()> {
        self.send_command(json!(["seek", secs, "absolute"])).await
    }

    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        self.send_command(json!(["set_property", "volume", volume as f64]))
            .await
    }

    pub async fn set_enhancer(&self, mode: EnhancerMode) -> Result<()> {
        self.send_command(json!(["set_property", "af", mode.mpv_af()]))
            .await
    }

    pub async fn shutdown(&self) {
        let _ = self.send_command(json!(["quit"])).await;
        let mut guard = self.child.lock().await;
        if let Some(mut c) = guard.take() {
            let _ = c.kill();
            let _ = c.wait();
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
