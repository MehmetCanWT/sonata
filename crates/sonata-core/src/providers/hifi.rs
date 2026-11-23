//! Portable stream resolution for Sonata, including fast start and Hi-Fi quality upgrades.
//!
//! 1. **Submodules**: `innertube` (direct InnerTube audio formats), `cipher` (zero-AST `base.js`
//!    cipher hook + `JsCipherBridge`), `matching` (search-title cleanup and quality badges), and
//!    `cache` (on-disk audio cache).
//! 2. **Fast Path**: `resolve_fast` prefers a local cached file, then a bounded or direct InnerTube URL.
//! 3. **Quality Upgrade**: `needs_hifi_upgrade` and `resolve_hifi_upgrade` look for a higher-bitrate
//!    InnerTube or JioSaavn stream in the background.
//! 4. **Recovery**: `resolve_recovery` re-resolves via InnerTube or JioSaavn when a stream fails.
//! 5. **Prefetch & Precache**: `precache_hifi_audio` and `prefetch_hifi_audio` download audio ahead of
//!    time, with a guard and semaphore preventing duplicate or unbounded concurrent prefetches.

mod cache;
mod cipher;
mod innertube;
mod matching;

use crate::auth::ExtractedCookies;
use crate::cache::CacheDb;
use crate::error::CoreError;
use crate::paths::CorePaths;
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use anyhow::Result;
use cache::cached_audio_stream_async;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};

#[cfg(test)]
use cache::{insert_bounded_cache, sanitize_track_id};
#[cfg(test)]
use innertube::parse_innertube_fast_audio;
#[cfg(test)]
use serde_json::json;

pub use crate::providers::profile::{
    FastStartMode, ResolverProfile, StreamFallbackMode, UpgradeDelivery,
};
pub use cipher::{
    apply_cipher_solution_to_url, JsCipherBridge, PreparedPlayerJs, SolvedCipherPair,
};
pub use innertube::{is_bounded_fast_start_url, parse_innertube_best_audio};
pub use matching::{
    audio_quality_badge, clean_search_title, extract_badge_kbps, pick_best_ytm_match,
    should_upgrade_candidate,
};

const MAX_CONCURRENT_PREFETCHES: usize = 4;

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
    prefetch_semaphore: Arc<Semaphore>,
    visitor_data: Arc<RwLock<Option<String>>>,
    player_cipher: Arc<RwLock<Option<PreparedPlayerJs>>>,
    player_cipher_init: Arc<Mutex<()>>,
    js_cipher_bridge: Arc<std::sync::RwLock<Option<Arc<dyn JsCipherBridge>>>>,
    paths: CorePaths,
    profile: Arc<std::sync::RwLock<ResolverProfile>>,
}

impl StreamResolver {
    /// Creates a stream resolver with explicit injected paths and platform resolution profile.
    pub fn with_profile(
        http: Client,
        ytm: YtMusicClient,
        cookies: ExtractedCookies,
        cache_db: Option<CacheDb>,
        paths: CorePaths,
        profile: ResolverProfile,
    ) -> Self {
        Self {
            http,
            ytm,
            cookies,
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_db,
            in_flight_prefetch: Arc::new(std::sync::Mutex::new(HashSet::new())),
            prefetch_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PREFETCHES)),
            visitor_data: Arc::new(RwLock::new(None)),
            player_cipher: Arc::new(RwLock::new(None)),
            player_cipher_init: Arc::new(Mutex::new(())),
            js_cipher_bridge: Arc::new(std::sync::RwLock::new(None)),
            paths,
            profile: Arc::new(std::sync::RwLock::new(profile)),
        }
    }

    /// Returns the explicit directory roots injected into this resolver.
    pub fn paths(&self) -> &CorePaths {
        &self.paths
    }

    /// Returns a snapshot of the active resolution policy profile for this resolver.
    pub fn profile(&self) -> ResolverProfile {
        self.profile
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Updates this resolver instance's runtime policy profile without touching global state.
    pub fn update_profile(&self, update: impl FnOnce(&mut ResolverProfile)) {
        let mut guard = self
            .profile
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        update(&mut guard);
    }

    /// Typed counterpart to [`Self::resolve_fast`] returning redacted [`CoreError`] categories.
    pub async fn resolve_fast_typed(
        &self,
        track: &Track,
    ) -> std::result::Result<ResolvedStream, CoreError> {
        self.resolve_fast(track)
            .await
            .map_err(|err| CoreError::classify_anyhow(&err))
    }

    /// Typed counterpart to [`Self::resolve_recovery`] returning redacted [`CoreError`] categories.
    pub async fn resolve_recovery_typed(
        &self,
        track: &Track,
    ) -> std::result::Result<ResolvedStream, CoreError> {
        self.resolve_recovery(track)
            .await
            .map_err(|err| CoreError::classify_anyhow(&err))
    }

    pub fn http_client(&self) -> &Client {
        &self.http
    }

    pub fn cookies(&self) -> &ExtractedCookies {
        &self.cookies
    }

    pub fn ytm_client(&self) -> &YtMusicClient {
        &self.ytm
    }

    pub async fn cached_audio_stream_for(&self, track_id: &str) -> Option<ResolvedStream> {
        cached_audio_stream_async(&self.paths, track_id).await
    }

    pub async fn get_cached_fresh_stream(&self, key: &str) -> Option<ResolvedStream> {
        self.get_cached_fresh(key).await
    }

    pub async fn put_cached_stream(&self, key: String, stream: ResolvedStream) {
        self.put_cached(key, stream).await;
    }

    pub async fn cache_average_bitrate_for(&self, track_id: &str, badge: &str) {
        self.cache_average_bitrate(track_id, badge).await;
    }

    pub async fn download_to_audio_cache_for(
        &self,
        track_id: &str,
        stream: &ResolvedStream,
    ) -> Option<ResolvedStream> {
        self.download_to_audio_cache(track_id, stream).await
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
        tracing::info!(event = "resolver_warmed", "resolver prerequisites ready");
    }

    pub async fn clear_all_stream_caches(&self) {
        self.cache.lock().await.clear();
        if let Some(database) = self.cache_db.clone() {
            let _ = tokio::task::spawn_blocking(move || database.clear_stream_cache()).await;
        }
    }

    /// Recovery after a player rejects a stream. It does not repeat the cached resolver.
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
        let stream = self
            .resolve_youtube_best_innertube(track, self.cookies.has_ytm_auth())
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to recover stream for \"{}\"", track.title))?;
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

    pub fn can_upgrade_with_jiosaavn_for(profile: &ResolverProfile, badge: &str) -> bool {
        if !profile.jiosaavn_enabled {
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
        let profile = self.profile();
        if profile.cache_enabled() {
            if let Some(local) = cached_audio_stream_async(&self.paths, &track.id).await {
                tracing::debug!(
                    "[sonata:resolver] cache-hit=local track_id={} source=local badge={}",
                    track.id,
                    local.badge
                );
                self.cache_average_bitrate(&track.id, &local.badge).await;
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
            self.cache_average_bitrate(&track.id, &stream.badge).await;
            self.put_cached(format!("jio:{}", track.id), stream.clone())
                .await;
            return Ok(stream);
        }

        let key = format!("yt:{}", track.id);
        if let Some(stream) = self.get_cached_fresh(&key).await {
            self.cache_average_bitrate(&track.id, &stream.badge).await;
            return Ok(stream);
        }

        let stream = self.resolve_youtube_fast(track).await?;
        self.cache_average_bitrate(&track.id, &stream.badge).await;
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
        let profile = self.profile();
        if !profile.cache_enabled() {
            return None;
        }
        if track.is_jiosaavn() {
            if current.url.starts_with("http") {
                let _ = self.download_to_audio_cache(&track.id, current).await;
            }
            return None;
        }
        if let Some(local) = cached_audio_stream_async(&self.paths, &track.id).await {
            if local.url != current.url && should_upgrade_candidate(&current.badge, &local.badge) {
                return Some(local);
            }
        }

        let can_try_ytm = allow_hifi && Self::needs_hifi_upgrade(&current.badge, &current.url);
        let can_try_jiosaavn = Self::can_upgrade_with_jiosaavn_for(&profile, &current.badge);

        if can_try_ytm || can_try_jiosaavn {
            let mut candidate: Option<ResolvedStream> = None;

            if can_try_ytm {
                candidate = self
                    .resolve_youtube_best_innertube(track, self.cookies.has_ytm_auth())
                    .await
                    .filter(|s| should_upgrade_candidate(&current.badge, &s.badge));
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
                        self.cache_average_bitrate(&track.id, &cand.badge).await;
                        if local.url != current.url || local.badge != current.badge {
                            return Some(local);
                        }
                    }
                    if profile.allows_remote_upgrade_fallback() || cand.badge.contains("JioSaavn") {
                        self.cache_average_bitrate(&track.id, &cand.badge).await;
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
        let profile = self.profile();
        if !profile.cache_enabled() || !allow_hifi {
            return None;
        }
        if let Some(local) = cached_audio_stream_async(&self.paths, &track.id).await {
            if !Self::needs_hifi_upgrade(&local.badge, &local.url)
                && !Self::can_upgrade_with_jiosaavn_for(&profile, &local.badge)
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
        let _permit = self.prefetch_semaphore.acquire().await.ok()?;
        let inserted = {
            let mut active = self.in_flight_prefetch.lock().ok()?;
            active.insert(track.id.clone())
        };
        if !inserted {
            return cached_audio_stream_async(&self.paths, &track.id).await;
        }
        let guard = PrefetchGuard {
            id: track.id.clone(),
            set: Arc::clone(&self.in_flight_prefetch),
        };
        let result = async {
            let fast = self.resolve_fast(track).await.ok()?;
            if let Some(upgraded) = self.resolve_hifi_upgrade(track, &fast, true).await {
                Some(upgraded)
            } else {
                cached_audio_stream_async(&self.paths, &track.id).await
            }
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

    struct StaticCipherBridge;

    impl JsCipherBridge for StaticCipherBridge {
        fn ensure_loaded(&self, _player_hash: &str, _hooked_script: &str) -> bool {
            true
        }

        fn solve(&self, _player_hash: &str, sig: &str, n: &str) -> Option<SolvedCipherPair> {
            Some(SolvedCipherPair {
                sig: (!sig.is_empty()).then(|| format!("solved_{sig}")),
                n: (!n.is_empty()).then(|| format!("solved_{n}")),
            })
        }
    }

    #[test]
    fn characterizes_anonymous_fixture_bounded_fast_start_vs_direct_best_url() {
        let payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/innertube_anonymous_fast.json"
        ))
        .expect("anonymous fixture JSON");

        let fast = parse_innertube_fast_audio(&payload).expect("fast-start audio");
        assert_eq!(fast.badge, "AAC 130kbps · Standard");
        assert!(is_bounded_fast_start_url(&fast.url));
        assert!(fast.url.ends_with("&range=0-1048575"));

        let best = parse_innertube_best_audio(&payload).expect("best audio");
        assert_eq!(best.badge, "OPUS 137kbps · HQ");
        assert!(!is_bounded_fast_start_url(&best.url));
        assert!(StreamResolver::needs_hifi_upgrade(&best.badge, &best.url));
    }

    #[tokio::test]
    async fn characterizes_authenticated_tvhtml5_cipher_fixture_best_and_fast_start_for_linux_and_android_profiles(
    ) {
        let payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/innertube_authenticated_tvhtml5.json"
        ))
        .expect("authenticated TVHTML5 fixture JSON");

        let http = Client::new();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let paths = CorePaths::for_app_roots(
            std::env::temp_dir().join("sonata_hifi_unit_cfg"),
            std::env::temp_dir().join("sonata_hifi_unit_cache"),
        );

        let linux_profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::ExternalTool,
            bootstrap_timeout: Duration::from_millis(2400),
            jiosaavn_enabled: true,
            cache_limit_bytes: 1024 * 1024 * 1024,
        };
        let linux_resolver = StreamResolver::with_profile(
            http.clone(),
            ytm.clone(),
            cookies.clone(),
            None,
            paths.clone(),
            linux_profile,
        );
        linux_resolver.set_js_cipher_bridge(Arc::new(StaticCipherBridge));

        let android_profile = ResolverProfile {
            fast_start: FastStartMode::DirectUrl,
            upgrade_delivery: UpgradeDelivery::ReturnRemoteStream,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(8500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 1024 * 1024 * 1024,
        };
        let android_resolver =
            StreamResolver::with_profile(http, ytm, cookies, None, paths, android_profile);
        android_resolver.set_js_cipher_bridge(Arc::new(StaticCipherBridge));

        let prepared = PreparedPlayerJs {
            player_hash: "fixturehash".to_string(),
            signature_timestamp: 20_774,
            hooked_script: String::new(),
            cached_at_epoch_secs: 0,
        };

        let (_, _, linux_best) = linux_resolver
            .parse_innertube_ciphered_audio_candidate(&payload, &prepared, false)
            .await
            .expect("authenticated Linux best audio");
        assert_eq!(linux_best.badge, "OPUS 256kbps · Extreme");
        assert!(linux_best.url.contains("sig=solved_rawsig774"));
        assert!(linux_best.url.contains("n=solved_rawN774"));
        assert!(!is_bounded_fast_start_url(&linux_best.url));
        assert!(!StreamResolver::needs_hifi_upgrade(
            &linux_best.badge,
            &linux_best.url
        ));

        let (_, _, linux_fast) = linux_resolver
            .parse_innertube_ciphered_audio_candidate(&payload, &prepared, true)
            .await
            .expect("authenticated Linux fast audio");
        assert_eq!(linux_fast.badge, "AAC 129kbps · Standard");
        assert!(linux_fast.url.contains("sig=solved_rawsig140"));
        assert!(linux_fast.url.contains("n=solved_rawN140"));
        assert!(is_bounded_fast_start_url(&linux_fast.url));

        let (_, _, android_fast) = android_resolver
            .parse_innertube_ciphered_audio_candidate(&payload, &prepared, true)
            .await
            .expect("authenticated Android direct audio");
        assert_eq!(android_fast.badge, "AAC 129kbps · Standard");
        assert!(android_fast.url.contains("sig=solved_rawsig140"));
        assert!(!is_bounded_fast_start_url(&android_fast.url));
    }

    #[test]
    fn characterizes_unavailable_innertube_fixture_as_none() {
        let payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/innertube_unavailable.json"
        ))
        .expect("unavailable fixture JSON");
        assert!(parse_innertube_fast_audio(&payload).is_none());
        assert!(parse_innertube_best_audio(&payload).is_none());
    }

    #[test]
    fn characterizes_jiosaavn_upgrade_eligibility_gate() {
        let mut profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::ExternalTool,
            bootstrap_timeout: Duration::from_millis(2400),
            jiosaavn_enabled: false,
            cache_limit_bytes: 1024 * 1024 * 1024,
        };
        assert!(!StreamResolver::can_upgrade_with_jiosaavn_for(
            &profile,
            "OPUS 137kbps · HQ"
        ));

        profile.jiosaavn_enabled = true;
        assert!(StreamResolver::can_upgrade_with_jiosaavn_for(
            &profile,
            "OPUS 137kbps · HQ"
        ));
        assert!(!StreamResolver::can_upgrade_with_jiosaavn_for(
            &profile,
            "AAC 320kbps · Extreme"
        ));
        assert!(!StreamResolver::can_upgrade_with_jiosaavn_for(
            &profile,
            "FLAC 900kbps · Lossless"
        ));
    }

    #[test]
    fn characterizes_hifi_upgrade_need_for_local_and_remote_badges() {
        assert!(StreamResolver::needs_hifi_upgrade(
            "AAC 129kbps · Standard",
            "https://rr1.googlevideo.com/videoplayback?itag=140&range=0-1048575"
        ));
        assert!(StreamResolver::needs_hifi_upgrade(
            "OPUS 137kbps · HQ",
            "https://rr1.googlevideo.com/videoplayback?itag=251"
        ));
        assert!(!StreamResolver::needs_hifi_upgrade(
            "OPUS 256kbps · Extreme",
            "https://rr1.googlevideo.com/videoplayback?itag=774"
        ));
        assert!(!StreamResolver::needs_hifi_upgrade(
            "AAC 320kbps · JioSaavn",
            "https://aac.saavncdn.com/track_320.mp4"
        ));
        assert!(!StreamResolver::needs_hifi_upgrade(
            "OPUS 256kbps · Hi-Fi",
            "/tmp/local.v3.audio"
        ));
        assert!(StreamResolver::needs_hifi_upgrade(
            "OPUS 256kbps · Hi-Fi",
            "https://rr1.googlevideo.com/videoplayback?itag=251"
        ));
    }
}
