use super::matching::{audio_quality_badge, pick_best_ytm_match};
use super::*;
use crate::redaction::stream_url as redact_stream_url;
use anyhow::anyhow;
use serde_json::{json, Value};

const PLAYER_API: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
const FAST_START_TIMEOUT: Duration = Duration::from_millis(1200);
const FAST_START_RANGE_END: u64 = 1024 * 1024 - 1;

impl StreamResolver {
    pub(super) async fn resolve_youtube_fast(&self, track: &Track) -> Result<ResolvedStream> {
        let video_id = match track.source {
            ProviderSource::YouTubeMusic if !track.id.is_empty() && !track.id.contains(' ') => {
                Some(track.id.clone())
            }
            _ => {
                let query = format!("{} {}", track.artist, clean_search_title(&track.title));
                self.ytm
                    .search_songs(&query)
                    .await
                    .ok()
                    .and_then(|results| {
                        pick_best_ytm_match(track, &results).map(|best| best.id.clone())
                    })
            }
        };

        tracing::debug!(
            "[sonata:resolver] track={:?} source={:?} video_id={} cookies={}",
            track.title,
            track.source,
            video_id.as_deref().unwrap_or("-"),
            if self.cookies.has_ytm_auth() {
                "available"
            } else {
                "none"
            }
        );

        if let Some(video_id) = &video_id {
            let bootstrap = self.resolve_youtube_bootstrap(video_id, track.source);
            let ytdlp = self.resolve_youtube_ytdlp(track, false, true);
            tokio::pin!(bootstrap);
            tokio::pin!(ytdlp);
            let mut bootstrap_done = false;
            let mut ytdlp_done = false;
            let raced = tokio::time::timeout(FAST_START_TIMEOUT, async {
                while !bootstrap_done || !ytdlp_done {
                    tokio::select! {
                        result = &mut bootstrap, if !bootstrap_done => {
                            bootstrap_done = true;
                            if let Some(stream) = result {
                                return Some(stream);
                            }
                        }
                        result = &mut ytdlp, if !ytdlp_done => {
                            ytdlp_done = true;
                            if let Ok(stream) = result {
                                return Some(stream);
                            }
                        }
                    }
                }
                None
            })
            .await
            .ok()
            .flatten();
            if let Some(stream) = raced {
                tracing::info!(
                    "[sonata:resolver] result=ok method=fast-race url={} badge={}",
                    redact_stream_url(&stream.url),
                    stream.badge
                );
                return Ok(stream);
            }
        }

        if self.cookies.has_ytm_auth() {
            if let Ok(stream) = self.resolve_youtube_ytdlp(track, true, false).await {
                tracing::info!(
                    "[sonata:resolver] result=ok method=yt-dlp cookies-present url={} badge={}",
                    redact_stream_url(&stream.url),
                    stream.badge
                );
                return Ok(stream);
            }
        }
        if let Ok(stream) = self.resolve_youtube_ytdlp(track, false, false).await {
            tracing::info!(
                "[sonata:resolver] result=ok method=yt-dlp no-cookies url={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            return Ok(stream);
        }
        Err(anyhow!("Failed to resolve stream for \"{}\"", track.title))
    }

    async fn resolve_youtube_bootstrap(
        &self,
        video_id: &str,
        source: ProviderSource,
    ) -> Option<ResolvedStream> {
        let ios_body = json!({
            "context": {
                "client": {
                    "clientName": "IOS",
                    "clientVersion": "20.10.4",
                    "deviceMake": "Apple",
                    "deviceModel": "iPhone16,2",
                    "osName": "iPhone",
                    "osVersion": "18.3.2.22D82",
                    "hl": "en",
                    "gl": "US"
                }
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        let vr_body = json!({
            "context": {
                "client": {
                    "clientName": "ANDROID_VR",
                    "clientVersion": "1.60.19",
                    "androidSdkVersion": 32,
                    "osName": "Android",
                    "osVersion": "12L",
                    "hl": "en",
                    "gl": "US"
                }
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        let ios = async {
            self.post_innertube_player(
                &ios_body,
                "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
                source,
            )
            .await
        };
        let android_vr = async {
            self.post_innertube_player(
                &vr_body,
                "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
                source,
            )
            .await
        };
        tokio::pin!(ios);
        tokio::pin!(android_vr);
        let mut ios_done = false;
        let mut vr_done = false;
        while !ios_done || !vr_done {
            tokio::select! {
                result = &mut ios, if !ios_done => {
                    ios_done = true;
                    if result.is_some() {
                        return result;
                    }
                }
                result = &mut android_vr, if !vr_done => {
                    vr_done = true;
                    if result.is_some() {
                        return result;
                    }
                }
            }
        }
        None
    }

    async fn post_innertube_player(
        &self,
        body: &Value,
        user_agent: &str,
        source: ProviderSource,
    ) -> Option<ResolvedStream> {
        let response = self
            .http
            .post(PLAYER_API)
            .header("User-Agent", user_agent)
            .header("Content-Type", "application/json")
            .json(body)
            .timeout(Duration::from_millis(3200))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let mut stream = parse_innertube_fast_audio(&payload)?;
        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![
                ("User-Agent".to_string(), user_agent.to_string()),
                (
                    "Referer".to_string(),
                    "https://music.youtube.com/".to_string(),
                ),
            ];
        }
        Some(stream)
    }
}

pub fn parse_innertube_best_audio(payload: &Value) -> Option<ResolvedStream> {
    parse_innertube_audio(payload, false)
}

pub(super) fn parse_innertube_fast_audio(payload: &Value) -> Option<ResolvedStream> {
    parse_innertube_audio(payload, true)
}

struct AudioCandidate {
    priority: u8,
    average_bps: u64,
    format_id: String,
    url: String,
    codec: String,
    average_kbps: Option<u32>,
    content_length: Option<u64>,
}

fn parse_innertube_audio(payload: &Value, prefer_fast_start: bool) -> Option<ResolvedStream> {
    let formats = payload
        .pointer("/streamingData/adaptiveFormats")?
        .as_array()?;
    let duration_secs = payload
        .pointer("/videoDetails/lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let mut best = None;

    for format in formats {
        let Some(itag) = format.get("itag").and_then(Value::as_u64) else {
            continue;
        };
        let priority = match (prefer_fast_start, itag) {
            (false, 774) => 0,
            (false, 141) => 1,
            (false, 251) => 2,
            (false, 140) => 3,
            (true, 140) => 0,
            (true, 251) => 1,
            (true, 141) => 2,
            (true, 774) => 3,
            _ => continue,
        };
        let mime = format.get("mimeType").and_then(Value::as_str).unwrap_or("");
        if !mime.starts_with("audio/") {
            continue;
        }
        let Some(url) = format
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| url.starts_with("https://"))
        else {
            continue;
        };
        let content_length = format
            .get("contentLength")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u64>().ok());
        let average_bps = format
            .get("averageBitrate")
            .and_then(Value::as_u64)
            .or_else(|| {
                let bytes = content_length?;
                let duration = duration_secs?;
                (duration > 0).then(|| bytes.saturating_mul(8) / duration)
            });
        let average_kbps = average_bps.map(|bps| (bps as f64 / 1000.0).round() as u32);
        let codec = if mime.to_ascii_lowercase().contains("opus") {
            "OPUS"
        } else {
            "AAC"
        };
        let candidate = AudioCandidate {
            priority,
            average_bps: average_bps.unwrap_or(0),
            format_id: itag.to_string(),
            url: url.to_string(),
            codec: codec.to_string(),
            average_kbps,
            content_length,
        };
        if best.as_ref().is_none_or(|current: &AudioCandidate| {
            candidate.priority < current.priority
                || (candidate.priority == current.priority
                    && candidate.average_bps > current.average_bps)
        }) {
            best = Some(candidate);
        }
    }

    let AudioCandidate {
        format_id,
        url,
        codec,
        average_kbps,
        content_length,
        ..
    } = best?;
    let url = if prefer_fast_start {
        bounded_fast_start_url(url, content_length)
    } else {
        url
    };
    Some(ResolvedStream {
        url,
        badge: audio_quality_badge(
            ProviderSource::YouTubeMusic,
            &format_id,
            &codec,
            average_kbps,
        ),
        headers: Vec::new(),
    })
}

fn bounded_fast_start_url(url: String, content_length: Option<u64>) -> String {
    if is_bounded_fast_start_url(&url) {
        return url;
    }
    let end = content_length
        .and_then(|length| length.checked_sub(1))
        .map_or(FAST_START_RANGE_END, |end| end.min(FAST_START_RANGE_END));
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}range=0-{end}")
}

pub(super) fn is_bounded_fast_start_url(url: &str) -> bool {
    url.split_once('?').is_some_and(|(_, query)| {
        query
            .split('&')
            .any(|part| part == "range=0-1048575" || part.starts_with("range=0-"))
    })
}
