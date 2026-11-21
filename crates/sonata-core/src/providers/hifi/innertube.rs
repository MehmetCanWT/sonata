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
const FAST_START_TIMEOUT: Duration = Duration::from_millis(2400);
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
            if cfg!(target_os = "android") {
                if let Some(stream) = tokio::time::timeout(
                    Duration::from_millis(6500),
                    self.resolve_youtube_bootstrap(video_id, track.source),
                )
                .await
                .ok()
                .flatten()
                {
                    tracing::info!(
                        "[sonata:resolver] result=ok method=android-innertube url={} badge={}",
                        redact_stream_url(&stream.url),
                        stream.badge
                    );
                    return Ok(stream);
                }
                if let Some(stream) = self
                    .resolve_youtube_alternate_candidates(track, video_id)
                    .await
                {
                    tracing::info!(
                        "[sonata:resolver] result=ok method=android-alternate-fallback url={} badge={}",
                        redact_stream_url(&stream.url),
                        stream.badge
                    );
                    return Ok(stream);
                }
                tracing::warn!(
                    "[sonata:resolver] result=err method=android-innertube track={:?} video_id={}",
                    track.title,
                    video_id
                );
                return Err(anyhow!("Failed to resolve stream for \"{}\"", track.title));
            }

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
        let effective_fast_start = prefer_fast_start && !cfg!(target_os = "android");
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

    async fn resolve_youtube_bootstrap(
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

    pub(super) async fn resolve_youtube_best_innertube(
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
        let prepared = self.ensure_prepared_player_js().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "TVHTML5",
                    "clientVersion": "5.20260707",
                    "deviceMake": "Samsung",
                    "deviceModel": "UKS9800",
                    "userAgent": TVHTML5_USER_AGENT,
                    "osName": "Tizen",
                    "osVersion": "2.4.0",
                    "hl": "en",
                    "gl": "US"
                }
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

        let response = self
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
            .timeout(Duration::from_millis(3200))
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }
        let payload: Value = response.json().await.ok()?;
        let effective_fast_start = prefer_fast_start && !cfg!(target_os = "android");
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

    async fn parse_innertube_ciphered_audio_candidate(
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
            let url = if prefer_fast_start && !cfg!(target_os = "android") {
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
        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20250402.01.00",
                    "hl": "en",
                    "gl": "US"
                }
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });
        let ua = "Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36 Chrome/131.0.0.0 Mobile Safari/537.36";
        self.post_innertube_best(&body, ua, source, true).await
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
        let mut stream = if cfg!(target_os = "android") {
            parse_innertube_best_audio(&payload)?
        } else {
            parse_innertube_fast_audio(&payload)?
        };
        if source == ProviderSource::YouTubeMusic {
            stream.headers = vec![("User-Agent".to_string(), user_agent.to_string())];
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
    let url = if prefer_fast_start && !cfg!(target_os = "android") {
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

pub(super) fn is_bounded_fast_start_url(url: &str) -> bool {
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
