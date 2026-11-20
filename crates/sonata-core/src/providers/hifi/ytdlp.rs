use super::matching::{audio_quality_badge, pick_best_ytm_match};
use super::*;
use crate::redaction::sensitive_text as redact_sensitive_text;
use anyhow::anyhow;
use serde_json::Value;
use std::sync::OnceLock;
use tokio::process::Command;

struct TempFileGuard(std::path::PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl StreamResolver {
    pub(super) async fn resolve_youtube_ytdlp(
        &self,
        track: &Track,
        use_cookies: bool,
        prefer_fast_start: bool,
    ) -> Result<ResolvedStream> {
        let mut target_url = match track.source {
            ProviderSource::YouTubeMusic => {
                format!("https://music.youtube.com/watch?v={}", track.id)
            }
            ProviderSource::Spotify => {
                let query = format!("{} {}", track.artist, clean_search_title(&track.title));
                if let Ok(results) = self.ytm.search_songs(&query).await {
                    if let Some(best) = pick_best_ytm_match(track, &results) {
                        format!("https://music.youtube.com/watch?v={}", best.id)
                    } else {
                        format!("ytsearch1:{query} official audio")
                    }
                } else {
                    format!("ytsearch1:{query} official audio")
                }
            }
        };

        if target_url.is_empty() {
            target_url = format!("ytsearch1:{} {}", track.artist, track.title);
        }

        tracing::debug!(
            "[sonata:resolver] method=yt-dlp start cookies={} target={}",
            if use_cookies { "present" } else { "none" },
            if target_url.starts_with("ytsearch1:") {
                "[ytsearch]"
            } else {
                "https://music.youtube.com/watch?v=[redacted-video-id]"
            }
        );

        let mut command = Command::new("yt-dlp");
        command.kill_on_drop(true);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--quiet",
            "--socket-timeout",
            "5",
        ]);
        if let Some(runtime) = detect_js_runtime() {
            command.arg("--js-runtimes").arg(runtime);
            tracing::debug!("[sonata:resolver] yt-dlp js-runtime={runtime}");
        }
        let format_selector = match (track.source, prefer_fast_start) {
            (ProviderSource::YouTubeMusic, true) => "140/251/141/774",
            (ProviderSource::YouTubeMusic, false) => "774/141/251/140",
            (ProviderSource::Spotify, _) => {
                "bestaudio[acodec=flac]/bestaudio[acodec=opus]/bestaudio[acodec=aac]/bestaudio/best"
            }
        };
        let output_template = match track.source {
            ProviderSource::YouTubeMusic => {
                "%(acodec)s|%(abr)s|%(audio_ext)s|%(format_id)s|%(url)s|%(http_headers)j"
            }
            ProviderSource::Spotify => "%(acodec)s|%(abr)s|%(audio_ext)s|%(format_id)s|%(url)s",
        };
        command.args(["-f", format_selector, "--print", output_template]);
        let mut cookie_guard = None;
        if use_cookies {
            if let Some(cookie_file) = &self.cookies.netscape_file {
                if cookie_file.exists() {
                    let unique = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_nanos())
                        .unwrap_or(0);
                    let run_copy =
                        cookie_file.with_file_name(format!("yt_cookies_run_{unique}.txt"));
                    if std::fs::copy(cookie_file, &run_copy).is_ok() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &run_copy,
                                std::fs::Permissions::from_mode(0o600),
                            );
                        }
                        command.arg("--cookies").arg(&run_copy);
                        cookie_guard = Some(TempFileGuard(run_copy));
                    }
                }
            }
        }
        command.arg("--").arg(&target_url);

        tracing::debug!(
            "[sonata:resolver] method=yt-dlp attempt=1/1 cookies={}",
            if use_cookies { "present" } else { "none" }
        );
        let output = tokio::time::timeout(Duration::from_secs(8), command.output())
            .await
            .map_err(|_| anyhow!("yt-dlp timed out while resolving stream"))??;
        drop(cookie_guard);
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let error = redact_sensitive_text(error.trim());
            tracing::warn!(
                "[sonata:resolver] result=error method=yt-dlp exit={} stderr={}",
                output.status,
                error
            );
            return Err(anyhow!("yt-dlp failed: {error}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|line| line.contains('|'))
            .ok_or_else(|| anyhow!("No audio stream returned by yt-dlp"))?;
        let YtdlpAudioMetadata {
            codec,
            average_kbps,
            format_id,
            stream_url,
            headers,
        } = parse_ytdlp_audio_line(line)
            .ok_or_else(|| anyhow!("Unexpected yt-dlp stream metadata"))?;
        let badge = audio_quality_badge(
            ProviderSource::YouTubeMusic,
            &format_id,
            &codec,
            average_kbps,
        );

        Ok(ResolvedStream {
            url: stream_url,
            badge,
            headers: if track.source == ProviderSource::YouTubeMusic {
                headers
            } else {
                Vec::new()
            },
        })
    }
}

pub(super) fn detect_js_runtime() -> Option<&'static str> {
    static RUNTIME: OnceLock<Option<&'static str>> = OnceLock::new();
    *RUNTIME.get_or_init(|| {
        ["deno", "node", "bun", "quickjs"]
            .into_iter()
            .find(|&candidate| {
                std::process::Command::new(candidate)
                    .arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
            })
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct YtdlpAudioMetadata {
    pub(super) codec: String,
    pub(super) average_kbps: Option<u32>,
    pub(super) format_id: String,
    pub(super) stream_url: String,
    pub(super) headers: Vec<(String, String)>,
}

pub(super) fn parse_ytdlp_audio_line(line: &str) -> Option<YtdlpAudioMetadata> {
    let parts: Vec<&str> = line.splitn(6, '|').collect();
    if parts.len() < 5 {
        return None;
    }
    let codec = parts[0].trim();
    let average_kbps = parts[1]
        .trim()
        .parse::<f64>()
        .ok()
        .map(|value| value.round() as u32);
    let format_id = parts[3].trim();
    let stream_url = parts[4].trim();
    if stream_url.is_empty() {
        return None;
    }
    let headers = parts
        .get(5)
        .and_then(|raw| serde_json::from_str::<Value>(raw.trim()).ok())
        .map(safe_stream_headers)
        .unwrap_or_default();
    Some(YtdlpAudioMetadata {
        codec: codec.to_string(),
        average_kbps,
        format_id: format_id.to_string(),
        stream_url: stream_url.to_string(),
        headers,
    })
}

/// Keep only ordinary media request headers. Never forward cookies or credentials.
fn safe_stream_headers(value: Value) -> Vec<(String, String)> {
    let Some(headers) = value.as_object() else {
        return Vec::new();
    };
    headers
        .iter()
        .filter_map(|(name, value)| {
            let allowed = matches!(
                name.to_ascii_lowercase().as_str(),
                "user-agent" | "referer" | "origin" | "accept" | "accept-language"
            );
            let value = value.as_str()?.trim();
            (allowed && !value.is_empty() && !value.contains(['\r', '\n']))
                .then(|| (name.clone(), value.to_string()))
        })
        .collect()
}
