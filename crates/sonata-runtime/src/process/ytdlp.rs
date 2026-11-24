//! Linux stream resolver composing portable `sonata-core` InnerTube resolution with `yt-dlp`.
//!
//! 1. **Composition**: `LinuxStreamResolver` wraps `sonata_core::providers::hifi::StreamResolver`
//!    with [`CliJsCipherBridge`] registered automatically.
//! 2. **Fast Start & Fallback**: Races InnerTube bootstrap against unauthenticated `yt-dlp` on
//!    Linux when `ResolverProfile::allows_external_fallback()` is enabled, falling back cleanly
//!    to authenticated/unauthenticated `yt-dlp` on failure or recovery.
//! 3. **Subprocess Hygiene**: Runs `yt-dlp` with `kill_on_drop(true)`, strict timeouts, redacted
//!    stderr logging, and `0o600` temporary Netscape cookie copies cleaned up via RAII guards.

use crate::config::{resolve_distro_binary, AppConfig};
use crate::downloads::DownloadStore;
use crate::process::CliJsCipherBridge;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::CacheDb;
use sonata_core::error::CoreError;
use sonata_core::paths::CorePaths;
use sonata_core::providers::hifi::{
    audio_quality_badge, clean_search_title, extract_badge_kbps, is_bounded_fast_start_url,
    pick_best_ytm_match, should_upgrade_candidate, ResolvedStream, ResolverProfile, StreamResolver,
};
use sonata_core::providers::servers::{load_server_profiles, parse_server_item_id, ServerClient};
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{jiosaavn, ProviderSource, Track};
use sonata_core::redaction::{
    sensitive_text as redact_sensitive_text, stream_url as redact_stream_url,
};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Semaphore;

const FAST_START_TIMEOUT: Duration = Duration::from_millis(2400);
const MAX_CONCURRENT_PREFETCHES: usize = 4;

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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

/// Linux stream resolver composing portable `sonata-core` resolution with `CliJsCipherBridge`
/// and `yt-dlp` subprocess fallback.
#[derive(Clone)]
pub struct LinuxStreamResolver {
    core: StreamResolver,
    in_flight_prefetch: Arc<std::sync::Mutex<HashSet<String>>>,
    prefetch_semaphore: Arc<Semaphore>,
    downloads_dir: Arc<std::sync::RwLock<Option<PathBuf>>>,
    downloads_store: Arc<std::sync::Mutex<Option<DownloadStore>>>,
}

impl LinuxStreamResolver {
    /// Creates a Linux stream resolver with explicit injected paths and resolution profile.
    pub fn with_profile(
        http: Client,
        ytm: YtMusicClient,
        cookies: ExtractedCookies,
        cache_db: Option<CacheDb>,
        paths: CorePaths,
        profile: ResolverProfile,
    ) -> Self {
        let core =
            StreamResolver::with_profile(http, ytm, cookies, cache_db, paths.clone(), profile);
        core.set_js_cipher_bridge(Arc::new(CliJsCipherBridge::new(paths)));
        Self {
            core,
            in_flight_prefetch: Arc::new(std::sync::Mutex::new(HashSet::new())),
            prefetch_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PREFETCHES)),
            downloads_dir: Arc::new(std::sync::RwLock::new(None)),
            downloads_store: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Overrides the shared offline downloads directory used before network stream resolution.
    pub fn with_downloads_dir(self, dir: impl Into<PathBuf>) -> Self {
        self.set_downloads_dir(Some(dir.into()));
        self
    }

    /// Injects an already-opened shared offline [`DownloadStore`] for this resolver.
    pub fn with_downloads_store(self, store: DownloadStore) -> Self {
        self.set_downloads_store(Some(store));
        self
    }

    /// Updates the offline downloads directory override and clears any cached store handle.
    pub fn set_downloads_dir(&self, dir: Option<PathBuf>) {
        if let Ok(mut guard) = self.downloads_dir.write() {
            *guard = dir;
        }
        if let Ok(mut guard) = self.downloads_store.lock() {
            *guard = None;
        }
    }

    /// Replaces the cached offline [`DownloadStore`] handle for this resolver.
    pub fn set_downloads_store(&self, store: Option<DownloadStore>) {
        let dir = store.as_ref().map(|entry| entry.root().to_path_buf());
        if let Ok(mut guard) = self.downloads_dir.write() {
            *guard = dir;
        }
        if let Ok(mut guard) = self.downloads_store.lock() {
            *guard = store;
        }
    }

    /// Checks the shared offline downloads manifest off the async executor thread.
    pub async fn resolve_offline_download(&self, track_id: &str) -> Option<ResolvedStream> {
        let track_id = track_id.to_string();
        let downloads_dir = Arc::clone(&self.downloads_dir);
        let downloads_store = Arc::clone(&self.downloads_store);

        tokio::task::spawn_blocking(move || {
            let existing = downloads_store.lock().ok().and_then(|guard| guard.clone());
            let store = match existing {
                Some(store) => store,
                None => {
                    let root = downloads_dir
                        .read()
                        .ok()
                        .and_then(|guard| guard.clone())
                        .unwrap_or_else(AppConfig::configured_downloads_dir);
                    let opened = DownloadStore::open_existing(root).ok().flatten()?;
                    if let Ok(mut guard) = downloads_store.lock() {
                        if guard.is_none() {
                            *guard = Some(opened.clone());
                        }
                    }
                    opened
                }
            };
            store.resolve_offline_stream(&track_id).ok().flatten()
        })
        .await
        .ok()
        .flatten()
    }

    pub fn inner(&self) -> &StreamResolver {
        &self.core
    }

    pub fn paths(&self) -> &CorePaths {
        self.core.paths()
    }

    pub fn profile(&self) -> ResolverProfile {
        self.core.profile()
    }

    pub fn update_profile(&self, update: impl FnOnce(&mut ResolverProfile)) {
        self.core.update_profile(update);
    }

    pub fn http_client(&self) -> &Client {
        self.core.http_client()
    }

    pub fn needs_hifi_upgrade(badge: &str, url: &str) -> bool {
        StreamResolver::needs_hifi_upgrade(badge, url)
    }

    pub async fn warm(&self) {
        let (_, _) = tokio::join!(
            self.core.warm(),
            tokio::task::spawn_blocking(detect_js_runtime)
        );
    }

    pub async fn clear_all_stream_caches(&self) {
        self.core.clear_all_stream_caches().await;
    }

    pub async fn invalidate_stream(&self, track_id: &str) {
        self.core.invalidate_stream(track_id).await;
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

    /// Resolves an immediately playable stream before any background quality upgrade starts.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn resolve_fast(&self, track: &Track) -> Result<ResolvedStream> {
        if let Some(offline) = self.resolve_offline_download(&track.id).await {
            tracing::debug!(
                "[sonata:resolver] cache-hit=offline track_id={} source=offline badge={}",
                track.id,
                offline.badge
            );
            self.core
                .cache_average_bitrate_for(&track.id, &offline.badge)
                .await;
            return Ok(offline);
        }

        if track.is_self_hosted() {
            return self.resolve_self_hosted_stream(track).await;
        }

        let profile = self.profile();
        if !profile.allows_external_fallback() || track.is_jiosaavn() {
            return self.core.resolve_fast(track).await;
        }

        if profile.cache_enabled() {
            if let Some(local) = self.core.cached_audio_stream_for(&track.id).await {
                tracing::debug!(
                    "[sonata:resolver] cache-hit=local track_id={} source=local badge={}",
                    track.id,
                    local.badge
                );
                self.core
                    .cache_average_bitrate_for(&track.id, &local.badge)
                    .await;
                return Ok(local);
            }
        }

        let key = format!("yt:{}", track.id);
        if let Some(stream) = self.core.get_cached_fresh_stream(&key).await {
            self.core
                .cache_average_bitrate_for(&track.id, &stream.badge)
                .await;
            return Ok(stream);
        }

        let stream = self
            .resolve_youtube_fast_with_ytdlp(track, &profile)
            .await?;
        self.core
            .cache_average_bitrate_for(&track.id, &stream.badge)
            .await;
        self.core.put_cached_stream(key, stream.clone()).await;
        Ok(stream)
    }

    /// Recovery after `mpv` rejects a stream. Never repeats the cached stream URL.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn resolve_recovery(&self, track: &Track) -> Result<ResolvedStream> {
        if track.is_self_hosted() {
            return self.resolve_self_hosted_stream(track).await;
        }

        let profile = self.profile();
        if !profile.allows_external_fallback() || track.is_jiosaavn() {
            return self.core.resolve_recovery(track).await;
        }

        self.core.invalidate_stream(&track.id).await;
        if self.core.cookies().has_ytm_auth() {
            if let Ok(stream) = self.resolve_youtube_ytdlp(track, true, false).await {
                tracing::info!(
                    "[sonata:resolver] recovery=ok method=yt-dlp cookies-present track_id={} badge={}",
                    track.id,
                    stream.badge
                );
                self.core
                    .put_cached_stream(format!("yt:{}", track.id), stream.clone())
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
        self.core
            .put_cached_stream(format!("yt:{}", track.id), stream.clone())
            .await;
        Ok(stream)
    }

    /// Caches a higher-bitrate YTM or JioSaavn format before switching playback.
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
        if let Some(offline) = self.resolve_offline_download(&track.id).await {
            if offline.url != current.url
                && should_upgrade_candidate(&current.badge, &offline.badge)
            {
                return Some(offline);
            }
            return None;
        }

        if track.is_self_hosted() {
            return None;
        }

        let profile = self.profile();
        if !profile.allows_external_fallback() || track.is_jiosaavn() {
            return self
                .core
                .resolve_hifi_upgrade(track, current, allow_hifi)
                .await;
        }
        if !profile.cache_enabled() {
            return None;
        }
        if let Some(local) = self.core.cached_audio_stream_for(&track.id).await {
            if local.url != current.url && should_upgrade_candidate(&current.badge, &local.badge) {
                return Some(local);
            }
        }

        let can_try_ytm = allow_hifi && Self::needs_hifi_upgrade(&current.badge, &current.url);
        let can_try_jiosaavn =
            StreamResolver::can_upgrade_with_jiosaavn_for(&profile, &current.badge);

        if can_try_ytm || can_try_jiosaavn {
            let mut candidate: Option<ResolvedStream> = None;

            if can_try_ytm {
                if track.source == ProviderSource::Spotify {
                    if self.core.cookies().has_ytm_auth() {
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
                } else {
                    if self.core.cookies().has_ytm_auth() {
                        candidate = self
                            .core
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
                    if let Some(js) =
                        jiosaavn::find_higher_quality_stream(self.http_client(), track, best_kbps)
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
                    if let Some(local) = self
                        .core
                        .download_to_audio_cache_for(&track.id, &cand)
                        .await
                    {
                        self.core
                            .cache_average_bitrate_for(&track.id, &cand.badge)
                            .await;
                        if local.url != current.url || local.badge != current.badge {
                            return Some(local);
                        }
                    }
                    if profile.allows_remote_upgrade_fallback() || cand.badge.contains("JioSaavn") {
                        self.core
                            .cache_average_bitrate_for(&track.id, &cand.badge)
                            .await;
                        return Some(cand);
                    }
                }
            }
        }

        if track.source == ProviderSource::YouTubeMusic
            && current.url.starts_with("http")
            && !is_bounded_fast_start_url(&current.url)
        {
            let _ = self
                .core
                .download_to_audio_cache_for(&track.id, current)
                .await;
        }
        None
    }

    /// Caches a higher-quality local stream for a future queue transition.
    #[tracing::instrument(level = "debug", skip(self, track), fields(track_id = %track.id, provider = ?track.source))]
    pub async fn precache_hifi_audio(
        &self,
        track: &Track,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        if let Some(offline) = self.resolve_offline_download(&track.id).await {
            tracing::debug!(
                event = "next_precache_offline_hit",
                track_id = %track.id,
                quality = %offline.badge,
                "next-track offline download hit"
            );
            return Some(offline);
        }

        if track.is_self_hosted() {
            return None;
        }

        let profile = self.profile();
        if !profile.cache_enabled() || !allow_hifi {
            return None;
        }
        if let Some(local) = self.core.cached_audio_stream_for(&track.id).await {
            if !Self::needs_hifi_upgrade(&local.badge, &local.url)
                && !StreamResolver::can_upgrade_with_jiosaavn_for(&profile, &local.badge)
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
            return self.core.cached_audio_stream_for(&track.id).await;
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
                self.core.cached_audio_stream_for(&track.id).await
            }
        }
        .await;
        drop(guard);
        result
    }

    /// Backward-compatible alias for [`Self::precache_hifi_audio`].
    pub async fn prefetch_hifi_audio(
        &self,
        track: &Track,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        self.precache_hifi_audio(track, allow_hifi).await
    }

    async fn resolve_self_hosted_stream(&self, track: &Track) -> Result<ResolvedStream> {
        let item_ref = parse_server_item_id(&track.id)
            .ok_or_else(|| anyhow!("Track is not a self-hosted server item"))?;
        let paths = self.paths().clone();
        let profiles = tokio::task::spawn_blocking(move || load_server_profiles(&paths))
            .await
            .map_err(|err| anyhow!("Server profile load task panicked: {err}"))?
            .map_err(|err| anyhow!("Failed to load server profiles: {err}"))?;
        let profile = profiles
            .into_iter()
            .find(|p| p.id == item_ref.server_id)
            .ok_or_else(|| anyhow!("Server profile '{}' not found", item_ref.server_id))?;
        if !profile.enabled {
            return Err(anyhow!("Server '{}' is disabled", profile.name));
        }
        let client = ServerClient::new(self.http_client().clone());
        let stream = client
            .resolve_stream(&profile, track)
            .map_err(|err| anyhow!("{err}"))?;
        let badge = stream
            .audio_badge
            .or_else(|| track.audio_badge.clone())
            .unwrap_or_else(|| profile.kind.display_name().to_string());
        tracing::debug!(
            "[sonata:resolver] result=ok method=self-hosted kind={:?} url={} badge={}",
            profile.kind,
            redact_stream_url(&stream.url),
            badge
        );
        Ok(ResolvedStream {
            url: stream.url,
            badge,
            headers: stream.headers,
        })
    }

    async fn resolve_youtube_fast_with_ytdlp(
        &self,
        track: &Track,
        profile: &ResolverProfile,
    ) -> Result<ResolvedStream> {
        let video_id = self.core.resolve_youtube_video_id(track).await;
        tracing::debug!(
            "[sonata:resolver] track={:?} source={:?} video_id={} cookies={}",
            track.title,
            track.source,
            video_id.as_deref().unwrap_or("-"),
            if self.core.cookies().has_ytm_auth() {
                "available"
            } else {
                "none"
            }
        );

        if let Some(video_id) = &video_id {
            let bootstrap = self.core.resolve_youtube_bootstrap(video_id, track.source);
            let ytdlp = self.resolve_youtube_ytdlp(track, false, true);
            tokio::pin!(bootstrap);
            tokio::pin!(ytdlp);
            let mut bootstrap_done = false;
            let mut ytdlp_done = false;
            let raced =
                tokio::time::timeout(profile.bootstrap_timeout.min(FAST_START_TIMEOUT), async {
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

        if self.core.cookies().has_ytm_auth() {
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

    async fn resolve_youtube_ytdlp(
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
                if let Ok(results) = self.core.ytm_client().search_songs(&query).await {
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

        let ytdlp_bin = resolve_distro_binary("yt-dlp");
        let mut command = Command::new(&ytdlp_bin);
        command.kill_on_drop(true);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--quiet",
            "--socket-timeout",
            "5",
        ]);
        if let Some(runtime) = tokio::task::spawn_blocking(detect_js_runtime)
            .await
            .ok()
            .flatten()
        {
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
            if let Some(cookie_file) = self.core.cookies().netscape_file.clone() {
                if let Some(run_copy) = copy_cookie_file_private(cookie_file).await {
                    command.arg("--cookies").arg(&run_copy);
                    cookie_guard = Some(TempFileGuard(run_copy));
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

async fn copy_cookie_file_private(cookie_file: PathBuf) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&cookie_file).ok()?;
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let run_copy = cookie_file.with_file_name(format!(
            "yt_cookies_run_{}_{unique}.txt",
            std::process::id()
        ));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        opts.mode(0o600);
        let mut out = opts.open(&run_copy).ok()?;
        if out.write_all(&bytes).is_err() {
            let _ = std::fs::remove_file(&run_copy);
            return None;
        }
        Some(run_copy)
    })
    .await
    .ok()
    .flatten()
}

/// Detects the first installed JS runtime supported by `yt-dlp --js-runtimes`.
pub fn detect_js_runtime() -> Option<&'static str> {
    static RUNTIME: OnceLock<Option<&'static str>> = OnceLock::new();
    *RUNTIME.get_or_init(|| {
        ["deno", "node", "bun", "quickjs"]
            .into_iter()
            .find(|&candidate| {
                let bin = resolve_distro_binary(candidate);
                std::process::Command::new(&bin)
                    .arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
            })
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct YtdlpAudioMetadata {
    pub codec: String,
    pub average_kbps: Option<u32>,
    pub format_id: String,
    pub stream_url: String,
    pub headers: Vec<(String, String)>,
}

/// Parses one pipe-delimited `yt-dlp --print` audio metadata line while stripping sensitive headers.
pub fn parse_ytdlp_audio_line(line: &str) -> Option<YtdlpAudioMetadata> {
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

/// Keeps only ordinary media request headers. Never forwards cookies or credentials.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rejects_malformed_stream_metadata_and_sensitive_headers() {
        assert_eq!(parse_ytdlp_audio_line("broken"), None);
        assert_eq!(parse_ytdlp_audio_line("aac|128|m4a|140||{}"), None);
        let parsed = parse_ytdlp_audio_line("aac|128|m4a|140|https://media.test/audio|{\"Cookie\":\"secret\",\"Authorization\":\"Bearer secret\",\"User-Agent\":\"Sonata\"}").expect("valid metadata");
        assert_eq!(
            parsed.headers,
            vec![("User-Agent".to_string(), "Sonata".to_string())]
        );
    }

    #[tokio::test]
    async fn resolve_fast_prefers_shared_offline_download_over_cache_and_network() {
        use crate::downloads::test_support::TempTestDir;
        use crate::downloads::{DownloadStore, DownloadTrackInput};
        use sonata_core::providers::profile::{FastStartMode, StreamFallbackMode, UpgradeDelivery};

        let core_dir = TempTestDir::new("ytdlp_core");
        let downloads_dir = TempTestDir::new("ytdlp_dl");
        let store = DownloadStore::open(downloads_dir.path()).expect("open download store");

        let track = Track {
            id: "jiosaavn:offline_1".to_string(),
            title: "Offline Track".to_string(),
            artist: "Sonata Artist".to_string(),
            album: "Offline Album".to_string(),
            duration_secs: Some(180),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };

        let saved = store
            .save_track_bytes(
                &DownloadTrackInput {
                    track: track.clone(),
                    audio_badge: "AAC 320kbps · JioSaavn".to_string(),
                    bitrate_kbps: Some(320),
                    container: "m4a".to_string(),
                    artwork_url: None,
                    collection: None,
                },
                b"ftypM4A_offline_audio_payload",
                None,
            )
            .expect("save offline track");

        let http = sonata_core::providers::build_http_client();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let paths = CorePaths::for_app_roots(core_dir.path(), core_dir.path());
        // Disable cache and external fallback to prove offline downloads still resolve locally.
        let profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 0,
        };

        let resolver = LinuxStreamResolver::with_profile(http, ytm, cookies, None, paths, profile)
            .with_downloads_dir(downloads_dir.path());

        let resolved = resolver
            .resolve_fast(&track)
            .await
            .expect("offline download should resolve without network");
        assert_eq!(resolved.url, saved.audio_path.to_string_lossy());
        assert_eq!(resolved.badge, "AAC 320kbps · JioSaavn");
        assert!(resolved.headers.is_empty());

        // Also verify Hi-Fi upgrade skips redundant network upgrades for the active offline stream.
        assert!(resolver
            .resolve_hifi_upgrade(&track, &resolved, true)
            .await
            .is_none());

        // Verify precache hits the offline download immediately when cache_limit_bytes > 0.
        resolver.update_profile(|prof| prof.cache_limit_bytes = 1024 * 1024);
        let precached = resolver
            .precache_hifi_audio(&track, true)
            .await
            .expect("precache should return offline download");
        assert_eq!(precached.url, saved.audio_path.to_string_lossy());
    }

    #[tokio::test]
    async fn resolve_fast_resolves_self_hosted_servers_with_headers() {
        use crate::downloads::test_support::TempTestDir;
        use sonata_core::providers::profile::{FastStartMode, StreamFallbackMode, UpgradeDelivery};
        use sonata_core::providers::servers::{save_server_profiles, ServerKind, ServerProfile};

        let core_dir = TempTestDir::new("ytdlp_srv_core");
        let paths = CorePaths::for_app_roots(core_dir.path(), core_dir.path());

        let mut jf = ServerProfile::new(
            "jf-home",
            "Home Jellyfin",
            ServerKind::Jellyfin,
            "https://jf.internal",
        );
        jf.api_key = Some("jf-secret-token".to_string());

        let mut dav = ServerProfile::new(
            "dav-home",
            "Nextcloud",
            ServerKind::WebDav,
            "https://dav.internal/Music",
        );
        dav.username = Some("alice".to_string());
        dav.password = Some("dav-pass".to_string());

        save_server_profiles(&paths, &[jf, dav]).expect("save server profiles");

        let http = sonata_core::providers::build_http_client();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 0,
        };
        let resolver = LinuxStreamResolver::with_profile(http, ytm, cookies, None, paths, profile);

        let jf_track = Track {
            id: "jellyfin:jf-home:item-99".to_string(),
            title: "Server Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(200),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("FLAC · Jellyfin".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let jf_stream = resolver
            .resolve_fast(&jf_track)
            .await
            .expect("jellyfin stream");
        assert_eq!(
            jf_stream.url,
            "https://jf.internal/Audio/item-99/stream?static=true"
        );
        assert_eq!(jf_stream.badge, "FLAC · Jellyfin");
        assert!(jf_stream
            .headers
            .iter()
            .any(|(k, v)| k == "X-Emby-Token" && v == "jf-secret-token"));

        let dav_track = Track {
            id: "webdav:dav-home:/Music/track.flac".to_string(),
            title: "DAV Song".to_string(),
            artist: "Artist".to_string(),
            album: String::new(),
            duration_secs: None,
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("FLAC · WebDAV".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };
        let dav_stream = resolver
            .resolve_fast(&dav_track)
            .await
            .expect("webdav stream");
        assert_eq!(dav_stream.url, "https://dav.internal/Music/track.flac");
        assert!(dav_stream
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v.starts_with("Basic ")));
    }
}
