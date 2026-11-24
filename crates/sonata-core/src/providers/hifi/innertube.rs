//! Direct InnerTube player client probing and adaptive audio stream selection.
//!
//! 1. **Authenticated Premium Clients**: Queries `TVHTML5` (`tv_samsung` `5.20260707`) and `WEB_REMIX`
//!    with `SAPISIDHASH` and `signatureTimestamp` to unlock `itag=774` (~256–321 kbps Opus Extreme),
//!    `itag=141` (256 kbps AAC Extreme), and age-restricted tracks without PoToken gates.
//! 2. **Unauthenticated Fast Clients**: Queries `VISIONOS` (`1.02`) and `IOS` (`20.10.4`) for direct
//!    `itag=251` (~140–160 kbps Opus HQ) and `itag=140` (128 kbps AAC) streams.
//! 3. **Format Ranking & Deciphering**: Ranks `adaptiveFormats` by codec/itag quality and deciphers
//!    `signatureCipher` and `n` parameters via [`super::cipher`].

use super::matching::{audio_quality_badge, pick_best_ytm_match};
use super::*;
use crate::redaction::stream_url as redact_stream_url;
use anyhow::anyhow;
use serde_json::{json, Value};

const PLAYER_API: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
const VISIONOS_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const TVHTML5_USER_AGENT: &str =
    "Mozilla/5.0 (SMART-TV; Linux; Tizen 2.4.0) AppleWebKit/538.1 (KHTML, like Gecko) Version/2.4.0 TV Safari/538.1";
const FAST_START_RANGE_END: u64 = 1024 * 1024 - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoQualityStream {
    pub quality_label: String,
    pub height: u32,
    pub fps: u32,
    pub video_url: String,
    pub mime_type: String,
    pub bitrate_bps: u64,
    pub is_muxed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMusicVideo {
    pub video_id: String,
    pub qualities: Vec<VideoQualityStream>,
    pub audio_url: Option<String>,
    pub audio_badge: String,
    pub headers: Vec<(String, String)>,
}

fn video_codec_rank(mime: &str) -> u8 {
    let lower = mime.to_ascii_lowercase();
    if lower.contains("avc1") || lower.contains("video/mp4") && !lower.contains("av01") {
        0
    } else if lower.contains("vp9") || lower.contains("vp09") || lower.contains("video/webm") {
        1
    } else if lower.contains("av01") {
        2
    } else {
        3
    }
}

fn format_direct_or_cipher(format: &Value) -> (Option<String>, Option<String>) {
    let raw_url = format
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|u| u.starts_with("https://"))
        .map(str::to_string);
    let sig_cipher = format
        .get("signatureCipher")
        .or_else(|| format.get("cipher"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    (raw_url, sig_cipher)
}

pub fn parse_innertube_video_formats(
    payload: &Value,
) -> Option<(Vec<VideoQualityStream>, Option<String>, String)> {
    let audio = parse_innertube_best_audio(payload);
    let adaptive = payload
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let muxed = payload
        .pointer("/streamingData/formats")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut by_bucket: std::collections::HashMap<(u32, bool), VideoQualityStream> =
        std::collections::HashMap::new();

    for (is_muxed, list) in [(false, &adaptive), (true, &muxed)] {
        if !is_muxed && audio.is_none() {
            continue;
        }
        for format in list {
            let mime = format
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !mime.starts_with("video/") {
                continue;
            }
            let Some(url) = format
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|u| u.starts_with("https://"))
            else {
                continue;
            };
            let height = format
                .get("height")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(4320) as u32;
            if height < 144 {
                continue;
            }
            let fps = format.get("fps").and_then(Value::as_u64).unwrap_or(30) as u32;
            let quality_label = format
                .get("qualityLabel")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{height}p"));
            let bitrate_bps = format.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
            let cand = VideoQualityStream {
                quality_label,
                height,
                fps,
                video_url: url.to_string(),
                mime_type: mime.to_string(),
                bitrate_bps,
                is_muxed,
            };
            let key = (height, is_muxed);
            let replace = match by_bucket.get(&key) {
                None => true,
                Some(existing) => {
                    let c_rank = video_codec_rank(&cand.mime_type);
                    let e_rank = video_codec_rank(&existing.mime_type);
                    (
                        c_rank,
                        std::cmp::Reverse(cand.fps),
                        std::cmp::Reverse(cand.bitrate_bps),
                    ) < (
                        e_rank,
                        std::cmp::Reverse(existing.fps),
                        std::cmp::Reverse(existing.bitrate_bps),
                    )
                }
            };
            if replace {
                by_bucket.insert(key, cand);
            }
        }
    }

    let mut best_by_height: std::collections::HashMap<u32, VideoQualityStream> =
        std::collections::HashMap::new();
    for ((height, is_muxed), stream) in by_bucket {
        match best_by_height.get(&height) {
            None => {
                best_by_height.insert(height, stream);
            }
            Some(existing) if existing.is_muxed && !is_muxed => {
                best_by_height.insert(height, stream);
            }
            _ => {}
        }
    }

    let mut qualities: Vec<VideoQualityStream> = best_by_height.into_values().collect();
    if qualities.is_empty() {
        return None;
    }
    qualities.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.fps.cmp(&a.fps)));
    let (audio_url, audio_badge) = match audio {
        Some(a) => (Some(a.url), a.badge),
        None => (None, "AAC".to_string()),
    };
    Some((qualities, audio_url, audio_badge))
}

impl StreamResolver {
    pub async fn resolve_youtube_video_id(&self, track: &Track) -> Option<String> {
        match track.source {
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
        }
    }

    pub(super) async fn resolve_youtube_fast(&self, track: &Track) -> Result<ResolvedStream> {
        let video_id = self.resolve_youtube_video_id(track).await;

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

        let Some(video_id) = video_id else {
            return Err(anyhow!("Failed to resolve stream for \"{}\"", track.title));
        };
        let profile = self.profile();
        if let Some(stream) = tokio::time::timeout(
            profile.bootstrap_timeout,
            self.resolve_youtube_bootstrap(&video_id, track.source),
        )
        .await
        .ok()
        .flatten()
        {
            tracing::info!(
                "[sonata:resolver] result=ok method=innertube-only url={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            return Ok(stream);
        }
        if let Some(stream) = self
            .resolve_youtube_alternate_candidates(track, &video_id)
            .await
        {
            tracing::info!(
                "[sonata:resolver] result=ok method=innertube-alternate-fallback url={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            return Ok(stream);
        }
        tracing::warn!(
            "[sonata:resolver] result=err method=innertube-only track={:?} video_id={}",
            track.title,
            video_id
        );
        Err(anyhow!("Failed to resolve stream for \"{}\"", track.title))
    }

    pub(super) async fn ensure_visitor_data(&self) -> Option<String> {
        if let Some(cached) = self.visitor_data.read().await.clone() {
            return Some(cached);
        }

        let response = self
            .http
            .get("https://www.youtube.com/sw.js_data")
            .header("User-Agent", VISIONOS_USER_AGENT)
            .timeout(Duration::from_millis(3500))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let body = response.text().await.ok()?;
        let visitor = extract_visitor_data_from_sw_js(&body)?;
        *self.visitor_data.write().await = Some(visitor.clone());
        Some(visitor)
    }

    async fn post_innertube_visionos(
        &self,
        video_id: &str,
        source: ProviderSource,
        prefer_fast_start: bool,
    ) -> Option<ResolvedStream> {
        let visitor_data = self.ensure_visitor_data().await;
        self.send_innertube_visionos(video_id, source, prefer_fast_start, visitor_data.as_deref())
            .await
            .map(|(_, _, stream)| stream)
    }

    async fn send_innertube_visionos(
        &self,
        video_id: &str,
        source: ProviderSource,
        prefer_fast_start: bool,
        visitor_data: Option<&str>,
    ) -> Option<(u8, u64, ResolvedStream)> {
        let mut client = serde_json::Map::new();
        client.insert("clientName".to_string(), json!("VISIONOS"));
        client.insert("clientVersion".to_string(), json!("1.02"));
        client.insert("deviceMake".to_string(), json!("Apple"));
        client.insert("deviceModel".to_string(), json!("RealityDevice17,1"));
        client.insert("osName".to_string(), json!("visionOS"));
        client.insert("osVersion".to_string(), json!("26.5.23O471"));
        client.insert("userAgent".to_string(), json!(VISIONOS_USER_AGENT));
        client.insert("hl".to_string(), json!("en"));
        client.insert("gl".to_string(), json!("US"));
        if let Some(visitor) = visitor_data {
            client.insert("visitorData".to_string(), json!(visitor));
        }

        let body = json!({
            "context": {
                "client": Value::Object(client)
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });

        let mut req = self
            .http
            .post(PLAYER_API)
            .header("User-Agent", VISIONOS_USER_AGENT)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "101")
            .header("X-YouTube-Client-Version", "1.02")
            .json(&body)
            .timeout(Duration::from_millis(3200));
        if let Some(visitor) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", visitor);
        }

        let response = req.send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let effective_fast_start = prefer_fast_start && self.profile().uses_bounded_fast_start();
        let (priority, average_bps, mut stream) =
            parse_innertube_audio_candidate(&payload, effective_fast_start)?;
        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![("User-Agent".to_string(), VISIONOS_USER_AGENT.to_string())];
        }
        Some((priority, average_bps, stream))
    }

    async fn resolve_youtube_alternate_candidates(
        &self,
        track: &Track,
        primary_video_id: &str,
    ) -> Option<ResolvedStream> {
        let query = format!("{} {}", track.artist, clean_search_title(&track.title));
        let results = self.ytm.search_songs(&query).await.ok()?;
        for candidate in results.iter().filter(|c| c.id != primary_video_id).take(4) {
            if let Some(stream) = self
                .resolve_youtube_bootstrap(&candidate.id, track.source)
                .await
            {
                tracing::info!(
                    "[sonata:resolver] resolved alternate candidate_id={} for primary_id={} track={:?}",
                    candidate.id,
                    primary_video_id,
                    track.title
                );
                return Some(stream);
            }
        }
        None
    }

    pub async fn resolve_youtube_bootstrap(
        &self,
        video_id: &str,
        source: ProviderSource,
    ) -> Option<ResolvedStream> {
        if self.cookies.has_ytm_auth() {
            if let Some(stream) = self
                .post_innertube_tv_authenticated(video_id, source, false)
                .await
            {
                return Some(stream);
            }
        }

        if let Some(stream) = self.post_innertube_visionos(video_id, source, true).await {
            return Some(stream);
        }

        if self.cookies.has_ytm_auth() {
            if let Some(stream) = self.post_innertube_authenticated(video_id, source).await {
                return Some(stream);
            }
        }

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
        self.post_innertube_player(
            &ios_body,
            "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
            source,
        )
        .await
    }

    pub async fn resolve_youtube_best_innertube(
        &self,
        track: &Track,
        use_cookies: bool,
    ) -> Option<ResolvedStream> {
        let video_id = match track.source {
            ProviderSource::YouTubeMusic if !track.id.is_empty() && !track.id.contains(' ') => {
                track.id.clone()
            }
            _ => {
                let query = format!("{} {}", track.artist, clean_search_title(&track.title));
                let results = self.ytm.search_songs(&query).await.ok()?;
                pick_best_ytm_match(track, &results)?.id.clone()
            }
        };

        if use_cookies && self.cookies.has_ytm_auth() {
            if let Some(tv_stream) = self
                .post_innertube_tv_authenticated(&video_id, track.source, false)
                .await
            {
                return Some(tv_stream);
            }
        }

        if let Some(stream) = self
            .post_innertube_visionos(&video_id, track.source, false)
            .await
        {
            return Some(stream);
        }

        if use_cookies && self.cookies.has_ytm_auth() {
            if let Some(web_stream) = self
                .post_innertube_authenticated(&video_id, track.source)
                .await
            {
                return Some(web_stream);
            }
        }

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
        if let Some(ios_stream) = self
            .post_innertube_best(
                &ios_body,
                "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
                track.source,
                false,
            )
            .await
        {
            return Some(ios_stream);
        }
        self.resolve_youtube_alternate_candidates(track, &video_id)
            .await
    }

    async fn post_innertube_tv_authenticated(
        &self,
        video_id: &str,
        source: ProviderSource,
        prefer_fast_start: bool,
    ) -> Option<ResolvedStream> {
        let cookie_header = self.cookies.ytm_cookie_header()?;
        let auth_header = self
            .cookies
            .ytm_sapisidhash_for_origin("https://www.youtube.com")?;
        let visitor_data = self.ensure_visitor_data().await;
        let prepared = self.ensure_prepared_player_js().await?;

        let mut client_obj = serde_json::Map::new();
        client_obj.insert("clientName".to_string(), json!("TVHTML5"));
        client_obj.insert("clientVersion".to_string(), json!("5.20260707"));
        client_obj.insert("deviceMake".to_string(), json!("Samsung"));
        client_obj.insert("deviceModel".to_string(), json!("UKS9800"));
        client_obj.insert("userAgent".to_string(), json!(TVHTML5_USER_AGENT));
        client_obj.insert("osName".to_string(), json!("Tizen"));
        client_obj.insert("osVersion".to_string(), json!("2.4.0"));
        client_obj.insert("hl".to_string(), json!("en"));
        client_obj.insert("gl".to_string(), json!("US"));
        if let Some(ref visitor) = visitor_data {
            client_obj.insert("visitorData".to_string(), json!(visitor));
        }

        let body = json!({
            "context": {
                "client": Value::Object(client_obj)
            },
            "videoId": video_id,
            "playbackContext": {
                "contentPlaybackContext": {
                    "signatureTimestamp": prepared.signature_timestamp,
                    "html5Preference": "HTML5_PREF_WANTS"
                }
            },
            "contentCheckOk": true,
            "racyCheckOk": true
        });

        let mut req = self
            .http
            .post(PLAYER_API)
            .header("User-Agent", TVHTML5_USER_AGENT)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "7")
            .header("X-YouTube-Client-Version", "5.20260707")
            .header("Origin", "https://www.youtube.com")
            .header("Referer", "https://www.youtube.com/")
            .header("X-Origin", "https://www.youtube.com")
            .header("X-Goog-AuthUser", "0")
            .header("Cookie", cookie_header)
            .header("Authorization", auth_header)
            .json(&body)
            .timeout(Duration::from_millis(4500));
        if let Some(ref visitor) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", visitor);
        }
        let response = req.send().await.ok()?;

        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let effective_fast_start = prefer_fast_start && self.profile().uses_bounded_fast_start();
        let (_, _, mut stream) = self
            .parse_innertube_ciphered_audio_candidate(&payload, &prepared, effective_fast_start)
            .await?;

        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![
                ("User-Agent".to_string(), TVHTML5_USER_AGENT.to_string()),
                ("Origin".to_string(), "https://www.youtube.com".to_string()),
                (
                    "Referer".to_string(),
                    "https://www.youtube.com/".to_string(),
                ),
            ];
        }
        Some(stream)
    }

    pub(super) async fn parse_innertube_ciphered_audio_candidate(
        &self,
        payload: &Value,
        prepared: &PreparedPlayerJs,
        prefer_fast_start: bool,
    ) -> Option<(u8, u64, ResolvedStream)> {
        let formats = payload
            .pointer("/streamingData/adaptiveFormats")?
            .as_array()?;
        let duration_secs = payload
            .pointer("/videoDetails/lengthSeconds")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u64>().ok());

        struct RawCipherCandidate {
            priority: u8,
            average_bps: u64,
            format_id: String,
            raw_url: Option<String>,
            signature_cipher: Option<String>,
            codec: String,
            average_kbps: Option<u32>,
            content_length: Option<u64>,
        }

        let mut candidates: Vec<RawCipherCandidate> = Vec::new();
        let mut available_itags: Vec<u64> = Vec::new();

        for format in formats {
            let Some(itag) = format.get("itag").and_then(Value::as_u64) else {
                continue;
            };
            let mime = format.get("mimeType").and_then(Value::as_str).unwrap_or("");
            if !mime.starts_with("audio/") {
                continue;
            }
            available_itags.push(itag);
            let priority = match (prefer_fast_start, itag) {
                (false, 774) => 0,
                (false, 141) => 1,
                (false, 251) => 2,
                (false, 328 | 380) => 3,
                (false, 140) => 4,
                (false, 250) => 5,
                (false, 249) => 6,
                (false, 139) => 7,
                (false, _) => 8,
                (true, 140) => 0,
                (true, 251) => 1,
                (true, 141) => 2,
                (true, 774) => 3,
                (true, 328 | 380) => 4,
                (true, 250) => 5,
                (true, 249) => 6,
                (true, 139) => 7,
                (true, _) => 8,
            };
            let raw_url = format
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|u| u.starts_with("https://"))
                .map(str::to_string);
            let signature_cipher = format
                .get("signatureCipher")
                .or_else(|| format.get("cipher"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if raw_url.is_none() && signature_cipher.is_none() {
                continue;
            }
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
            candidates.push(RawCipherCandidate {
                priority,
                average_bps: average_bps.unwrap_or(0),
                format_id: itag.to_string(),
                raw_url,
                signature_cipher,
                codec: codec.to_string(),
                average_kbps,
                content_length,
            });
        }

        candidates.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| b.average_bps.cmp(&a.average_bps))
        });

        for candidate in candidates {
            let Some(deciphered_url) = self
                .decipher_format_url(
                    prepared,
                    candidate.raw_url.as_deref(),
                    candidate.signature_cipher.as_deref(),
                )
                .await
            else {
                continue;
            };
            tracing::info!(
                selected_itag = %candidate.format_id,
                codec = %candidate.codec,
                average_kbps = ?candidate.average_kbps,
                available_itags = ?available_itags,
                client = "TVHTML5",
                "selected InnerTube authenticated audio format"
            );
            let url = if prefer_fast_start && self.profile().uses_bounded_fast_start() {
                bounded_fast_start_url(deciphered_url, candidate.content_length)
            } else {
                deciphered_url
            };
            return Some((
                candidate.priority,
                candidate.average_bps,
                ResolvedStream {
                    url,
                    badge: audio_quality_badge(
                        ProviderSource::YouTubeMusic,
                        &candidate.format_id,
                        &candidate.codec,
                        candidate.average_kbps,
                    ),
                    headers: Vec::new(),
                },
            ));
        }

        None
    }

    async fn post_innertube_authenticated(
        &self,
        video_id: &str,
        source: ProviderSource,
    ) -> Option<ResolvedStream> {
        let prepared = self.ensure_prepared_player_js().await;
        let visitor_data = self.ensure_visitor_data().await;
        let mut client_obj = serde_json::Map::new();
        client_obj.insert("clientName".to_string(), json!("WEB_REMIX"));
        client_obj.insert("clientVersion".to_string(), json!("1.20260707.12.00"));
        client_obj.insert("hl".to_string(), json!("en"));
        client_obj.insert("gl".to_string(), json!("US"));
        if let Some(ref visitor) = visitor_data {
            client_obj.insert("visitorData".to_string(), json!(visitor));
        }

        let mut body = json!({
            "context": {
                "client": Value::Object(client_obj)
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        if let Some(ref p) = prepared {
            body["playbackContext"] = json!({
                "contentPlaybackContext": {
                    "signatureTimestamp": p.signature_timestamp,
                    "html5Preference": "HTML5_PREF_WANTS"
                }
            });
        }
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
        let mut req = self
            .http
            .post("https://music.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("User-Agent", ua)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("X-Origin", "https://music.youtube.com")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", "1.20260707.12.00")
            .header("X-Goog-AuthUser", "0")
            .json(&body)
            .timeout(Duration::from_millis(3500));
        if let Some(ref visitor) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", visitor);
        }
        if let Some(cookie) = self.cookies.ytm_cookie_header() {
            req = req.header("Cookie", cookie);
        }
        if let Some(auth) = self.cookies.ytm_sapisidhash() {
            req = req.header("Authorization", auth);
        }
        let response = req.send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let mut stream = if let Some(ref p) = prepared {
            self.parse_innertube_ciphered_audio_candidate(&payload, p, false)
                .await
                .map(|(_, _, s)| s)
                .or_else(|| parse_innertube_best_audio(&payload))?
        } else {
            parse_innertube_best_audio(&payload)?
        };
        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![
                ("User-Agent".to_string(), ua.to_string()),
                (
                    "Origin".to_string(),
                    "https://music.youtube.com".to_string(),
                ),
                (
                    "Referer".to_string(),
                    "https://music.youtube.com/".to_string(),
                ),
            ];
        }
        Some(stream)
    }

    async fn post_innertube_best(
        &self,
        body: &Value,
        user_agent: &str,
        source: ProviderSource,
        with_cookies: bool,
    ) -> Option<ResolvedStream> {
        let endpoint = if with_cookies {
            "https://music.youtube.com/youtubei/v1/player?prettyPrint=false"
        } else {
            PLAYER_API
        };
        let mut req = self
            .http
            .post(endpoint)
            .header("User-Agent", user_agent)
            .header("Content-Type", "application/json")
            .json(body)
            .timeout(Duration::from_millis(2200));
        if with_cookies {
            req = req
                .header("Origin", "https://music.youtube.com")
                .header("Referer", "https://music.youtube.com/");
            if let Some(cookie) = self.cookies.ytm_cookie_header() {
                req = req.header("Cookie", cookie);
            }
            if let Some(auth) = self.cookies.ytm_sapisidhash() {
                req = req.header("Authorization", auth);
                req = req.header("X-Origin", "https://music.youtube.com");
            }
        }
        let response = req.send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let mut stream = parse_innertube_best_audio(&payload)?;
        if source == ProviderSource::YouTubeMusic {
            stream.headers = if with_cookies {
                vec![
                    ("User-Agent".to_string(), user_agent.to_string()),
                    (
                        "Origin".to_string(),
                        "https://music.youtube.com".to_string(),
                    ),
                    (
                        "Referer".to_string(),
                        "https://music.youtube.com/".to_string(),
                    ),
                ]
            } else {
                vec![("User-Agent".to_string(), user_agent.to_string())]
            };
        }
        Some(stream)
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
        let mut stream = if self.profile().uses_bounded_fast_start() {
            parse_innertube_fast_audio(&payload)?
        } else {
            parse_innertube_best_audio(&payload)?
        };
        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![("User-Agent".to_string(), user_agent.to_string())];
        }
        Some(stream)
    }

    async fn parse_innertube_video_payload(
        &self,
        video_id: &str,
        payload: &Value,
        prepared: Option<&PreparedPlayerJs>,
        headers: Vec<(String, String)>,
    ) -> Option<ResolvedMusicVideo> {
        let adaptive = payload
            .pointer("/streamingData/adaptiveFormats")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let muxed = payload
            .pointer("/streamingData/formats")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let audio = if let Some(prep) = prepared {
            self.parse_innertube_ciphered_audio_candidate(payload, prep, false)
                .await
                .map(|(_, _, s)| s)
                .or_else(|| parse_innertube_best_audio(payload))
        } else {
            parse_innertube_best_audio(payload)
        };

        struct RawVideoCandidate {
            quality_label: String,
            height: u32,
            fps: u32,
            raw_url: Option<String>,
            sig_cipher: Option<String>,
            mime_type: String,
            bitrate_bps: u64,
            is_muxed: bool,
        }

        let mut grouped: std::collections::HashMap<(u32, bool), RawVideoCandidate> =
            std::collections::HashMap::new();

        for (is_muxed, list) in [(false, &adaptive), (true, &muxed)] {
            if !is_muxed && audio.is_none() {
                continue;
            }
            for format in list {
                let mime = format
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if !mime.starts_with("video/") {
                    continue;
                }
                let (raw_url, sig_cipher) = format_direct_or_cipher(format);
                if raw_url.is_none() && (sig_cipher.is_none() || prepared.is_none()) {
                    continue;
                }
                let height = format
                    .get("height")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .min(4320) as u32;
                if height < 144 {
                    continue;
                }
                let fps = format.get("fps").and_then(Value::as_u64).unwrap_or(30) as u32;
                let quality_label = format
                    .get("qualityLabel")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{height}p"));
                let bitrate_bps = format.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
                let key = (height, is_muxed);
                let cand = RawVideoCandidate {
                    quality_label,
                    height,
                    fps,
                    raw_url,
                    sig_cipher,
                    mime_type: mime.to_string(),
                    bitrate_bps,
                    is_muxed,
                };
                let replace = match grouped.get(&key) {
                    None => true,
                    Some(existing) => {
                        let c_rank = video_codec_rank(&cand.mime_type);
                        let e_rank = video_codec_rank(&existing.mime_type);
                        (
                            c_rank,
                            std::cmp::Reverse(cand.fps),
                            std::cmp::Reverse(cand.bitrate_bps),
                        ) < (
                            e_rank,
                            std::cmp::Reverse(existing.fps),
                            std::cmp::Reverse(existing.bitrate_bps),
                        )
                    }
                };
                if replace {
                    grouped.insert(key, cand);
                }
            }
        }

        let mut best_by_height: std::collections::HashMap<u32, VideoQualityStream> =
            std::collections::HashMap::new();
        for ((height, is_muxed), cand) in grouped {
            if let Some(existing) = best_by_height.get(&height) {
                if !existing.is_muxed || is_muxed {
                    continue;
                }
            }
            let url = if let Some(prep) = prepared {
                self.decipher_format_url(prep, cand.raw_url.as_deref(), cand.sig_cipher.as_deref())
                    .await
            } else {
                cand.raw_url
            };
            let Some(video_url) = url else {
                continue;
            };
            best_by_height.insert(
                height,
                VideoQualityStream {
                    quality_label: cand.quality_label,
                    height: cand.height,
                    fps: cand.fps,
                    video_url,
                    mime_type: cand.mime_type,
                    bitrate_bps: cand.bitrate_bps,
                    is_muxed: cand.is_muxed,
                },
            );
        }

        let mut qualities: Vec<VideoQualityStream> = best_by_height.into_values().collect();
        if qualities.is_empty() {
            return None;
        }
        qualities.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.fps.cmp(&a.fps)));
        let (audio_url, audio_badge) = match audio {
            Some(a) => (Some(a.url), a.badge),
            None => (None, "AAC".to_string()),
        };
        Some(ResolvedMusicVideo {
            video_id: video_id.to_string(),
            qualities,
            audio_url,
            audio_badge,
            headers,
        })
    }

    /// Resolves adaptive and fallback music video qualities alongside the video's paired audio stream.
    pub async fn resolve_music_video_streams(&self, video_id: &str) -> Result<ResolvedMusicVideo> {
        let clean_id = video_id.trim();
        if clean_id.is_empty() {
            return Err(anyhow!("Missing video ID"));
        }

        if self.cookies.has_ytm_auth() {
            if let (Some(cookie_header), Some(auth_header), Some(prepared)) = (
                self.cookies.ytm_cookie_header(),
                self.cookies
                    .ytm_sapisidhash_for_origin("https://www.youtube.com"),
                self.ensure_prepared_player_js().await,
            ) {
                let visitor_data = self.ensure_visitor_data().await;
                let mut client_obj = serde_json::Map::new();
                client_obj.insert("clientName".to_string(), json!("TVHTML5"));
                client_obj.insert("clientVersion".to_string(), json!("5.20260707"));
                client_obj.insert("deviceMake".to_string(), json!("Samsung"));
                client_obj.insert("deviceModel".to_string(), json!("UKS9800"));
                client_obj.insert("userAgent".to_string(), json!(TVHTML5_USER_AGENT));
                client_obj.insert("osName".to_string(), json!("Tizen"));
                client_obj.insert("osVersion".to_string(), json!("2.4.0"));
                client_obj.insert("hl".to_string(), json!("en"));
                client_obj.insert("gl".to_string(), json!("US"));
                if let Some(ref visitor) = visitor_data {
                    client_obj.insert("visitorData".to_string(), json!(visitor));
                }
                let body = json!({
                    "context": { "client": Value::Object(client_obj) },
                    "videoId": clean_id,
                    "playbackContext": {
                        "contentPlaybackContext": {
                            "signatureTimestamp": prepared.signature_timestamp,
                            "html5Preference": "HTML5_PREF_WANTS"
                        }
                    },
                    "contentCheckOk": true,
                    "racyCheckOk": true
                });
                let mut req = self
                    .http
                    .post(PLAYER_API)
                    .header("User-Agent", TVHTML5_USER_AGENT)
                    .header("Content-Type", "application/json")
                    .header("X-YouTube-Client-Name", "7")
                    .header("X-YouTube-Client-Version", "5.20260707")
                    .header("Origin", "https://www.youtube.com")
                    .header("Referer", "https://www.youtube.com/")
                    .header("X-Origin", "https://www.youtube.com")
                    .header("X-Goog-AuthUser", "0")
                    .header("Cookie", cookie_header)
                    .header("Authorization", auth_header)
                    .json(&body)
                    .timeout(Duration::from_millis(5000));
                if let Some(ref visitor) = visitor_data {
                    req = req.header("X-Goog-Visitor-Id", visitor);
                }
                if let Ok(resp) = req.send().await {
                    if resp.status().is_success() {
                        if let Ok(payload) = resp.json::<Value>().await {
                            let headers = vec![
                                ("User-Agent".to_string(), TVHTML5_USER_AGENT.to_string()),
                                ("Origin".to_string(), "https://www.youtube.com".to_string()),
                                (
                                    "Referer".to_string(),
                                    "https://www.youtube.com/".to_string(),
                                ),
                            ];
                            if let Some(resolved) = self
                                .parse_innertube_video_payload(
                                    clean_id,
                                    &payload,
                                    Some(&prepared),
                                    headers,
                                )
                                .await
                            {
                                return Ok(resolved);
                            }
                        }
                    }
                }
            }
        }

        let visitor_data = self.ensure_visitor_data().await;
        let mut visionos_client = serde_json::Map::new();
        visionos_client.insert("clientName".to_string(), json!("VISIONOS"));
        visionos_client.insert("clientVersion".to_string(), json!("1.02"));
        visionos_client.insert("deviceMake".to_string(), json!("Apple"));
        visionos_client.insert("deviceModel".to_string(), json!("RealityDevice17,1"));
        visionos_client.insert("osName".to_string(), json!("visionOS"));
        visionos_client.insert("osVersion".to_string(), json!("26.5.23O471"));
        visionos_client.insert("userAgent".to_string(), json!(VISIONOS_USER_AGENT));
        visionos_client.insert("hl".to_string(), json!("en"));
        visionos_client.insert("gl".to_string(), json!("US"));
        if let Some(ref v) = visitor_data {
            visionos_client.insert("visitorData".to_string(), json!(v));
        }
        let visionos_body = json!({
            "context": { "client": Value::Object(visionos_client) },
            "videoId": clean_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        let mut v_req = self
            .http
            .post(PLAYER_API)
            .header("User-Agent", VISIONOS_USER_AGENT)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "101")
            .header("X-YouTube-Client-Version", "1.02")
            .json(&visionos_body)
            .timeout(Duration::from_millis(4000));
        if let Some(ref v) = visitor_data {
            v_req = v_req.header("X-Goog-Visitor-Id", v);
        }
        if let Ok(resp) = v_req.send().await {
            if resp.status().is_success() {
                if let Ok(payload) = resp.json::<Value>().await {
                    if let Some(resolved) = self
                        .parse_innertube_video_payload(
                            clean_id,
                            &payload,
                            None,
                            vec![("User-Agent".to_string(), VISIONOS_USER_AGENT.to_string())],
                        )
                        .await
                    {
                        return Ok(resolved);
                    }
                }
            }
        }

        let ios_ua = "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)";
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
            "videoId": clean_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        if let Ok(resp) = self
            .http
            .post(PLAYER_API)
            .header("User-Agent", ios_ua)
            .header("Content-Type", "application/json")
            .json(&ios_body)
            .timeout(Duration::from_millis(4000))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(payload) = resp.json::<Value>().await {
                    if let Some(resolved) = self
                        .parse_innertube_video_payload(
                            clean_id,
                            &payload,
                            None,
                            vec![("User-Agent".to_string(), ios_ua.to_string())],
                        )
                        .await
                    {
                        return Ok(resolved);
                    }
                }
            }
        }

        Err(anyhow!(
            "No playable music video streams found for '{clean_id}'"
        ))
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
    parse_innertube_audio_candidate(payload, prefer_fast_start).map(|(_, _, stream)| stream)
}

fn parse_innertube_audio_candidate(
    payload: &Value,
    prefer_fast_start: bool,
) -> Option<(u8, u64, ResolvedStream)> {
    let formats = payload
        .pointer("/streamingData/adaptiveFormats")?
        .as_array()?;
    let duration_secs = payload
        .pointer("/videoDetails/lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let mut best = None;
    let mut available_itags: Vec<u64> = Vec::new();

    for format in formats {
        let Some(itag) = format.get("itag").and_then(Value::as_u64) else {
            continue;
        };
        let mime = format.get("mimeType").and_then(Value::as_str).unwrap_or("");
        if !mime.starts_with("audio/") {
            continue;
        }
        available_itags.push(itag);
        let priority = match (prefer_fast_start, itag) {
            (false, 774) => 0,
            (false, 141) => 1,
            (false, 251) => 2,
            (false, 328 | 380) => 3,
            (false, 140) => 4,
            (false, 250) => 5,
            (false, 249) => 6,
            (false, 139) => 7,
            (false, _) => 8,
            (true, 140) => 0,
            (true, 251) => 1,
            (true, 141) => 2,
            (true, 774) => 3,
            (true, 328 | 380) => 4,
            (true, 250) => 5,
            (true, 249) => 6,
            (true, 139) => 7,
            (true, _) => 8,
        };
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
        priority,
        average_bps,
        format_id,
        url,
        codec,
        average_kbps,
        content_length,
    } = best?;
    tracing::info!(
        selected_itag = %format_id,
        codec = %codec,
        average_kbps = ?average_kbps,
        available_itags = ?available_itags,
        "selected InnerTube audio format"
    );
    let url = if prefer_fast_start {
        bounded_fast_start_url(url, content_length)
    } else {
        url
    };
    Some((
        priority,
        average_bps,
        ResolvedStream {
            url,
            badge: audio_quality_badge(
                ProviderSource::YouTubeMusic,
                &format_id,
                &codec,
                average_kbps,
            ),
            headers: Vec::new(),
        },
    ))
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

pub fn is_bounded_fast_start_url(url: &str) -> bool {
    url.split_once('?').is_some_and(|(_, query)| {
        query
            .split('&')
            .any(|part| part == "range=0-1048575" || part.starts_with("range=0-"))
    })
}

fn extract_visitor_data_from_sw_js(body: &str) -> Option<String> {
    let trimmed = body
        .strip_prefix(")]}'\n")
        .or_else(|| body.strip_prefix(")]}'"))
        .unwrap_or(body)
        .trim_start();
    let val: Value = serde_json::from_str(trimmed).ok()?;
    if let Some(visitor) = val.pointer("/0/2/0/0/13").and_then(Value::as_str) {
        if !visitor.is_empty() {
            return Some(visitor.to_string());
        }
    }
    find_visitor_data_recursive(&val).map(str::to_string)
}

fn find_visitor_data_recursive(val: &Value) -> Option<&str> {
    match val {
        Value::String(s) if s.starts_with("Cgt") && s.len() >= 20 => Some(s.as_str()),
        Value::Array(arr) => arr.iter().find_map(find_visitor_data_recursive),
        Value::Object(map) => map.values().find_map(find_visitor_data_recursive),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_visitor_data_from_sw_js_pointer_and_fallback() {
        let mut inner = vec![Value::Null; 14];
        inner[13] = Value::String("CgtExactPointerVisitor12345".to_string());
        let root = json!([[Value::Null, Value::Null, [[Value::Array(inner)]]]]);
        let payload = format!(")]}}'\n{root}");
        assert_eq!(
            extract_visitor_data_from_sw_js(&payload).as_deref(),
            Some("CgtExactPointerVisitor12345")
        );

        let fallback = ")]}'\n{\"nested\":[{\"short\":\"CgtTooShort\",\"token\":\"CgtFallbackVisitorData987654321\"}]}";
        assert_eq!(
            extract_visitor_data_from_sw_js(fallback).as_deref(),
            Some("CgtFallbackVisitorData987654321")
        );
    }
}
