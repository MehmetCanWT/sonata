//! Stream resolution for Sonata, including fast start and Hi-Fi quality upgrades.
//!
//! 1. **Submodules**: `innertube` (direct InnerTube audio formats), `ytdlp` (yt-dlp fallback), `matching`
//!    (search-title cleanup and quality badges), and `cache` (on-disk audio cache).
//! 2. **Fast Path**: `resolve_fast` prefers a local cached file, then a bounded fast-start URL.
//! 3. **Quality Upgrade**: `needs_hifi_upgrade` and `resolve_hifi_upgrade` look for a higher-bitrate
//!    stream in the background.
//! 4. **Recovery**: `resolve_recovery` re-resolves when a stream URL fails or expires.
//! 5. **Prefetch & Precache**: `precache_hifi_audio` and `prefetch_hifi_audio` download audio ahead of
//!    time, with a guard preventing duplicate concurrent prefetches.

mod cache;
mod cipher;
mod innertube;
mod matching;
mod ytdlp;

use crate::auth::ExtractedCookies;
use crate::cache::CacheDb;
use crate::config::AppConfig;
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use anyhow::Result;
use cache::cached_audio_stream;
use innertube::is_bounded_fast_start_url;
use matching::{extract_badge_kbps, should_upgrade_candidate};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use ytdlp::detect_js_runtime;

#[cfg(test)]
use cache::{insert_bounded_cache, sanitize_track_id};
#[cfg(test)]
use innertube::parse_innertube_fast_audio;
#[cfg(test)]
use matching::audio_quality_badge;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use ytdlp::{parse_ytdlp_audio_line, YtdlpAudioMetadata};

pub use cipher::{JsCipherBridge, PreparedPlayerJs, SolvedCipherPair};
pub use innertube::parse_innertube_best_audio;
pub use matching::clean_search_title;

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    pub badge: String,
    /// Safe request headers yt-dlp or InnerTube needs to fetch this URL.
    pub headers: Vec<(String, String)>,
}

struct PrefetchGuard {
    id: String,
    set: Arc<std::sync::Mutex<HashSet<String>>>,
}

impl Drop for PrefetchGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.set.lock() {
            active.remove(&self.id);
        }
    }
}

#[derive(Clone)]
pub struct StreamResolver {
    http: Client,
    ytm: YtMusicClient,
    cookies: ExtractedCookies,
    cache: Arc<Mutex<HashMap<String, (Instant, ResolvedStream)>>>,
    cache_db: Option<CacheDb>,
    in_flight_prefetch: Arc<std::sync::Mutex<HashSet<String>>>,
    visitor_data: Arc<RwLock<Option<String>>>,
    player_cipher: Arc<RwLock<Option<PreparedPlayerJs>>>,
    player_cipher_init: Arc<Mutex<()>>,
    js_cipher_bridge: Arc<std::sync::RwLock<Option<Arc<dyn JsCipherBridge>>>>,
}

impl StreamResolver {
    pub fn new(
        http: Client,
        ytm: YtMusicClient,
        cookies: ExtractedCookies,
        cache_db: Option<CacheDb>,
    ) -> Self {
        Self {
            http,
            ytm,
            cookies,
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_db,
            in_flight_prefetch: Arc::new(std::sync::Mutex::new(HashSet::new())),
            visitor_data: Arc::new(RwLock::new(None)),
            player_cipher: Arc::new(RwLock::new(None)),
            player_cipher_init: Arc::new(Mutex::new(())),
            js_cipher_bridge: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn http_client(&self) -> &Client {
        &self.http
    }

    pub async fn warm(&self) {
        let home_req = self
            .http
            .get("https://music.youtube.com/")
            .timeout(Duration::from_secs(3))
            .send();
        let (_, _, _) = tokio::join!(
            home_req,
            self.ensure_visitor_data(),
            self.ensure_prepared_player_js()
        );
        let _ = detect_js_runtime();
        tracing::info!(event = "resolver_warmed", "resolver prerequisites ready");
    }

    pub async fn clear_all_stream_caches(&self) {
        self.cache.lock().await.clear();
        if let Some(database) = &self.cache_db {
            database.clear_stream_cache();
        }
    }

    /// Recovery after mpv/ExoPlayer rejects a stream. It does not repeat the cached resolver.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn resolve_recovery(&self, track: &Track) -> Result<ResolvedStream> {
        self.invalidate_stream(&track.id).await;
        if track.is_jiosaavn() {
            let js = crate::providers::jiosaavn::resolve_by_id(&self.http, &track.id)
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to recover JioSaavn stream for \"{}\"", track.title)
                })?;
            let stream = ResolvedStream {
                url: js.url,
                badge: js.badge,
                headers: Vec::new(),
            };
            self.put_cached(format!("jio:{}", track.id), stream.clone())
                .await;
            return Ok(stream);
        }
        if cfg!(target_os = "android") {
            let stream = self
                .resolve_youtube_best_innertube(track, self.cookies.has_ytm_auth())
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to recover stream for \"{}\"", track.title)
                })?;
            self.put_cached(format!("yt:{}", track.id), stream.clone())
                .await;
            return Ok(stream);
        }
        if self.cookies.has_ytm_auth() {
            if let Ok(stream) = self.resolve_youtube_ytdlp(track, true, false).await {
                tracing::info!(
                    "[sonata:resolver] recovery=ok method=yt-dlp cookies-present track_id={} badge={}",
                    track.id,
                    stream.badge
                );
                self.put_cached(format!("yt:{}", track.id), stream.clone())
                    .await;
                return Ok(stream);
            }
        }
        let stream = self.resolve_youtube_ytdlp(track, false, false).await?;
        tracing::info!(
            "[sonata:resolver] recovery=ok method=yt-dlp no-cookies track_id={} badge={}",
            track.id,
            stream.badge
        );
        self.put_cached(format!("yt:{}", track.id), stream.clone())
            .await;
        Ok(stream)
    }

    pub fn needs_hifi_upgrade(badge: &str, url: &str) -> bool {
        let is_local = !url.starts_with("http");
        let lower = badge.to_ascii_lowercase();
        if lower.contains("flac") || lower.contains("alac") || lower.contains("lossless") {
            return false;
        }
        let kbps = extract_badge_kbps(badge).unwrap_or(0);
        let is_aac_320 = lower.contains("aac") && kbps >= 320;
        let is_top_youtube_quality = lower.contains("extreme");
        let is_lossless_local = is_local
            && (lower.contains("hi-res")
                || lower.contains("hi-fi")
                || lower.contains("flac")
                || lower.contains("alac"));
        !(is_aac_320 || is_lossless_local || is_top_youtube_quality)
    }

    fn can_upgrade_with_jiosaavn(badge: &str) -> bool {
        if cfg!(target_os = "android") || !AppConfig::jiosaavn_enabled() {
            return false;
        }
        let lower = badge.to_ascii_lowercase();
        if lower.contains("flac") || lower.contains("alac") || lower.contains("lossless") {
            return false;
        }
        extract_badge_kbps(badge).unwrap_or(0) < 300
    }

    /// Resolve an immediately playable stream before any background quality upgrade starts.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn resolve_fast(&self, track: &Track) -> Result<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() > 0 {
            if let Some(local) = cached_audio_stream(&track.id) {
                tracing::debug!(
                    "[sonata:resolver] cache-hit=local track_id={} source=local badge={}",
                    track.id,
                    local.badge
                );
                self.cache_average_bitrate(&track.id, &local.badge);
                return Ok(local);
            }
        }
        if track.is_jiosaavn() {
            let js = crate::providers::jiosaavn::resolve_by_id(&self.http, &track.id)
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to resolve JioSaavn stream for \"{}\"", track.title)
                })?;
            let stream = ResolvedStream {
                url: js.url,
                badge: js.badge,
                headers: Vec::new(),
            };
            self.cache_average_bitrate(&track.id, &stream.badge);
            self.put_cached(format!("jio:{}", track.id), stream.clone())
                .await;
            return Ok(stream);
        }

        let key = format!("yt:{}", track.id);
        if let Some(stream) = self.get_cached_fresh(&key).await {
            self.cache_average_bitrate(&track.id, &stream.badge);
            return Ok(stream);
        }

        let stream = self.resolve_youtube_fast(track).await?;
        self.cache_average_bitrate(&track.id, &stream.badge);
        self.put_cached(key, stream.clone()).await;
        Ok(stream)
    }

    /// Cache a higher-bitrate YTM or JioSaavn format before switching playback.
    #[tracing::instrument(
        level = "debug",
        skip(self, track, current),
        fields(track_id = %track.id, provider = ?track.source)
    )]
    pub async fn resolve_hifi_upgrade(
        &self,
        track: &Track,
        current: &ResolvedStream,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 {
            return None;
        }
        if track.is_jiosaavn() {
            if current.url.starts_with("http") {
                let _ = self.download_to_audio_cache(&track.id, current).await;
            }
            return None;
        }
        if let Some(local) = cached_audio_stream(&track.id) {
            if local.url != current.url && should_upgrade_candidate(&current.badge, &local.badge) {
                return Some(local);
            }
        }

        let can_try_ytm = allow_hifi && Self::needs_hifi_upgrade(&current.badge, &current.url);
        let can_try_jiosaavn = Self::can_upgrade_with_jiosaavn(&current.badge);

        if can_try_ytm || can_try_jiosaavn {
            let mut candidate: Option<ResolvedStream> = None;

            if can_try_ytm {
                if track.source == ProviderSource::Spotify {
                    if self.cookies.has_ytm_auth() {
                        candidate = self
                            .resolve_youtube_ytdlp(track, true, false)
                            .await
                            .ok()
                            .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                    }
                    if candidate.is_none() {
                        candidate = self
                            .resolve_youtube_ytdlp(track, false, false)
                            .await
                            .ok()
                            .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                    }
                } else if cfg!(target_os = "android") {
                    candidate = self
                        .resolve_youtube_best_innertube(track, self.cookies.has_ytm_auth())
                        .await
                        .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                } else {
                    if self.cookies.has_ytm_auth() {
                        candidate = self
                            .resolve_youtube_best_innertube(track, true)
                            .await
                            .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                        if candidate.is_none() {
                            candidate = self
                                .resolve_youtube_ytdlp(track, true, false)
                                .await
                                .ok()
                                .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                        }
                    }
                    if candidate.is_none() {
                        candidate = self
                            .resolve_youtube_ytdlp(track, false, false)
                            .await
                            .ok()
                            .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
                    }
                }
            }

            if can_try_jiosaavn {
                let best_badge = candidate
                    .as_ref()
                    .map(|c| c.badge.as_str())
                    .unwrap_or(&current.badge);
                let best_kbps = extract_badge_kbps(best_badge).unwrap_or(0);
                if best_kbps < 300 {
                    if let Some(js) = crate::providers::jiosaavn::find_higher_quality_stream(
                        &self.http, track, best_kbps,
                    )
                    .await
                    {
                        let js_stream = ResolvedStream {
                            url: js.url,
                            badge: js.badge,
                            headers: Vec::new(),
                        };
                        if should_upgrade_candidate(best_badge, &js_stream.badge) {
                            candidate = Some(js_stream);
                        }
                    }
                }
            }

            if let Some(cand) =
                candidate.filter(|s| should_upgrade_candidate(&current.badge, &s.badge))
            {
                if cand.url.starts_with("http") {
                    if let Some(local) = self.download_to_audio_cache(&track.id, &cand).await {
                        self.cache_average_bitrate(&track.id, &cand.badge);
                        if local.url != current.url || local.badge != current.badge {
                            return Some(local);
                        }
                    }
                    if cfg!(target_os = "android") || cand.badge.contains("JioSaavn") {
                        self.cache_average_bitrate(&track.id, &cand.badge);
                        return Some(cand);
                    }
                }
            }
        }

        if track.source == ProviderSource::YouTubeMusic
            && current.url.starts_with("http")
            && !is_bounded_fast_start_url(&current.url)
        {
            let _ = self.download_to_audio_cache(&track.id, current).await;
        }
        None
    }

    /// Cache only a higher-quality local stream for a future queue transition.
    ///
    /// The foreground player must use [`resolve_fast`] when this returns `None`.
    /// Keeping that fallback outside this method prevents a precache failure from
    /// delaying audible playback.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn precache_hifi_audio(
        &self,
        track: &Track,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 || !allow_hifi {
            return None;
        }
        if let Some(local) = cached_audio_stream(&track.id) {
            if !Self::needs_hifi_upgrade(&local.badge, &local.url)
                && !Self::can_upgrade_with_jiosaavn(&local.badge)
            {
                tracing::info!(
                    event = "next_precache_cache_hit",
                    track_id = %track.id,
                    quality = %local.badge,
                    "next-track quality cache hit"
                );
                return Some(local);
            }
        }
        {
            let mut active = self.in_flight_prefetch.lock().ok()?;
            if !active.insert(track.id.clone()) {
                return cached_audio_stream(&track.id);
            }
        }
        let guard = PrefetchGuard {
            id: track.id.clone(),
            set: Arc::clone(&self.in_flight_prefetch),
        };
        let result = async {
            let fast = self.resolve_fast(track).await.ok()?;
            self.resolve_hifi_upgrade(track, &fast, true)
                .await
                .or_else(|| cached_audio_stream(&track.id))
        }
        .await;
        drop(guard);
        result
    }

    /// Backward-compatible name for callers that want a future-track quality cache.
    pub async fn prefetch_hifi_audio(
        &self,
        track: &Track,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        self.precache_hifi_audio(track, allow_hifi).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache::MAX_MEM_CACHE_ENTRIES;

    #[test]
    fn accepts_320_aac_upgrade_over_263_opus() {
        assert!(should_upgrade_candidate(
            "OPUS 263kbps · HQ",
            "AAC 320kbps · Extreme"
        ));
        assert!(!should_upgrade_candidate(
            "AAC 320kbps · Extreme",
            "AAC 320kbps · Extreme"
        ));
    }

    #[test]
    fn accepts_hq_upgrade_when_bitrate_delta_is_small() {
        assert!(should_upgrade_candidate(
            "AAC 129kbps · Standard",
            "OPUS 137kbps · HQ"
        ));
    }

    #[test]
    fn rejects_cached_copy_when_audio_quality_is_unchanged() {
        assert!(!should_upgrade_candidate(
            "OPUS 253kbps · Extreme",
            "OPUS 253kbps · Extreme · Cached"
        ));
        assert!(should_upgrade_candidate(
            "OPUS 137kbps · HQ · Cached",
            "OPUS 256kbps · Extreme · Cached"
        ));
        assert!(should_upgrade_candidate(
            "OPUS 256kbps · Extreme · Cached",
            "AAC 320kbps · JioSaavn"
        ));
    }

    #[test]
    fn parses_aac_metadata_from_ytdlp() {
        assert_eq!(
            parse_ytdlp_audio_line("aac|323.7|m4a|140|https://example.com/audio.m4a|{}"),
            Some(YtdlpAudioMetadata {
                codec: "aac".to_string(),
                average_kbps: Some(324),
                format_id: "140".to_string(),
                stream_url: "https://example.com/audio.m4a".to_string(),
                headers: Vec::new(),
            })
        );
    }

    #[test]
    fn classifies_audio_quality_from_source_codec_bitrate_and_format() {
        let cases = [
            (
                ProviderSource::YouTubeMusic,
                "140",
                "aac",
                Some(129),
                "AAC 129kbps · Standard",
            ),
            (
                ProviderSource::YouTubeMusic,
                "251",
                "opus",
                Some(137),
                "OPUS 137kbps · HQ",
            ),
            (
                ProviderSource::YouTubeMusic,
                "774",
                "opus",
                Some(256),
                "OPUS 256kbps · Extreme",
            ),
            (
                ProviderSource::YouTubeMusic,
                "custom",
                "flac",
                Some(900),
                "FLAC 900kbps · HQ",
            ),
            (
                ProviderSource::Spotify,
                "native",
                "flac",
                Some(900),
                "FLAC 900kbps · Hi-Res",
            ),
        ];
        for (source, format, codec, kbps, expected) in cases {
            let badge = audio_quality_badge(source, format, codec, kbps);
            assert_eq!(badge, expected);
            if source == ProviderSource::YouTubeMusic {
                assert!(!badge.contains("Hi-Res"));
            }
        }
    }

    #[test]
    fn rejects_malformed_stream_metadata_and_sensitive_headers() {
        assert_eq!(parse_ytdlp_audio_line("broken"), None);
        assert_eq!(parse_ytdlp_audio_line("aac|128|m4a|140||{}"), None);
        let parsed = parse_ytdlp_audio_line("aac|128|m4a|140|https://media.test/audio|{\"Cookie\":\"secret\",\"Authorization\":\"Bearer secret\",\"User-Agent\":\"Sonata\"}").expect("valid metadata");
        assert_eq!(
            parsed.headers,
            vec![("User-Agent".to_string(), "Sonata".to_string())]
        );
    }

    #[test]
    fn bounds_in_memory_stream_cache() {
        let mut cache = HashMap::new();
        for index in 0..(MAX_MEM_CACHE_ENTRIES + 10) {
            insert_bounded_cache(
                &mut cache,
                format!("yt:{index}"),
                ResolvedStream {
                    url: format!("https://example.com/{index}"),
                    badge: "OPUS".to_string(),
                    headers: Vec::new(),
                },
            );
        }
        assert_eq!(cache.len(), MAX_MEM_CACHE_ENTRIES);
    }

    #[test]
    fn sanitizes_track_id_for_audio_cache() {
        assert_eq!(sanitize_track_id("../evil/id?x=1"), "___evil_id_x_1");
        assert_eq!(sanitize_track_id("dQw4w9WgXcQ"), "dQw4w9WgXcQ");
    }

    #[test]
    fn parses_best_audio_from_innertube_player_payload() {
        let payload = json!({"streamingData": {"adaptiveFormats": [
            {"itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "averageBitrate": 129_500, "url": "https://rr1.googlevideo.com/videoplayback?itag=140"},
            {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 137_400, "url": "https://rr1.googlevideo.com/videoplayback?itag=251"}
        ]}});
        let stream = parse_innertube_best_audio(&payload).expect("best audio");
        assert_eq!(
            stream.url,
            "https://rr1.googlevideo.com/videoplayback?itag=251"
        );
        assert_eq!(stream.badge, "OPUS 137kbps · HQ");
        assert!(StreamResolver::needs_hifi_upgrade(
            &stream.badge,
            &stream.url
        ));

        let extreme_payload = json!({"streamingData": {"adaptiveFormats": [
            {"itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "averageBitrate": 129_500, "url": "https://rr1.googlevideo.com/videoplayback?itag=140"},
            {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 160_000, "url": "https://rr1.googlevideo.com/videoplayback?itag=251"},
            {"itag": 141, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "averageBitrate": 256_000, "url": "https://rr1.googlevideo.com/videoplayback?itag=141"},
            {"itag": 774, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 256_400, "url": "https://rr1.googlevideo.com/videoplayback?itag=774"}
        ]}});
        let extreme = parse_innertube_best_audio(&extreme_payload).expect("774 audio");
        assert_eq!(
            extreme.url,
            "https://rr1.googlevideo.com/videoplayback?itag=774"
        );
        assert_eq!(extreme.badge, "OPUS 256kbps · Extreme");
        assert!(!StreamResolver::needs_hifi_upgrade(
            &extreme.badge,
            &extreme.url
        ));
    }

    #[test]
    fn parses_fast_start_audio_before_higher_quality_formats() {
        let payload = json!({"streamingData": {"adaptiveFormats": [
            {"itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "averageBitrate": 129_500, "url": "https://rr1.googlevideo.com/videoplayback?itag=140"},
            {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 137_400, "url": "https://rr1.googlevideo.com/videoplayback?itag=251"}
        ]}});
        let stream = parse_innertube_fast_audio(&payload).expect("fast audio");
        assert_eq!(
            stream.url,
            "https://rr1.googlevideo.com/videoplayback?itag=140&range=0-1048575"
        );
    }

    #[test]
    fn rejects_malformed_innertube_payloads_and_non_audio_formats() {
        assert!(parse_innertube_best_audio(&json!({"streamingData": {}})).is_none());
        assert!(parse_innertube_best_audio(&json!({"streamingData": {"adaptiveFormats": [{"itag": 251, "mimeType": "video/webm", "url": "https://media.test/video"}]}})).is_none());
    }
}
