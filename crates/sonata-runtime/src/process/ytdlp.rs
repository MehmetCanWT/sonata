//! Linux stream resolver composing portable `sonata-core` InnerTube resolution with `yt-dlp`.
//!
//! 1. **Composition**: `LinuxStreamResolver` wraps `sonata_core::providers::hifi::StreamResolver`
//!    with [`CliJsCipherBridge`] registered automatically.
//! 2. **Fast Start & Fallback**: Races InnerTube bootstrap against unauthenticated `yt-dlp` on
//!    Linux when `ResolverProfile::allows_external_fallback()` is enabled, falling back cleanly
//!    to authenticated/unauthenticated `yt-dlp` on failure or recovery.
//! 3. **Subprocess Hygiene**: Runs `yt-dlp` with `kill_on_drop(true)`, strict timeouts, redacted
//!    stderr logging, and `0o600` temporary Netscape cookie copies cleaned up via RAII guards.

use crate::config::{
    configure_background_std_command, configure_background_tokio_command, resolve_distro_binary,
    AppConfig,
};
use crate::downloads::DownloadStore;
use crate::player::mpv::{
    compute_local_file_bitrate_kbps, format_measured_audio_badge, normalize_audio_codec_label,
};
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
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Semaphore;

static JS_RUNTIMES_UNSUPPORTED: AtomicBool = AtomicBool::new(false);

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

    /// Returns whether lookahead precaching is permitted for `profile` and `allow_hifi`.
    pub const fn precache_policy_allows(profile: &ResolverProfile, allow_hifi: bool) -> bool {
        profile.cache_enabled() && (allow_hifi || profile.jiosaavn_enabled)
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

        if is_local_track_id(&track.id) {
            return self.resolve_local_file_stream(track).await;
        }

        if is_smb_track_id(&track.id) {
            return self.resolve_smb_stream(track).await;
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
        if is_local_track_id(&track.id) {
            return self.resolve_local_file_stream(track).await;
        }

        if is_smb_track_id(&track.id) {
            return self.resolve_smb_stream(track).await;
        }

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

        if is_local_or_smb_track_id(&track.id) || track.is_self_hosted() {
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

        if is_local_or_smb_track_id(&track.id) || track.is_self_hosted() {
            return None;
        }

        let profile = self.profile();
        if !Self::precache_policy_allows(&profile, allow_hifi) {
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
            if let Some(upgraded) = self.resolve_hifi_upgrade(track, &fast, allow_hifi).await {
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

    async fn resolve_local_file_stream(&self, track: &Track) -> Result<ResolvedStream> {
        let track_clone = track.clone();
        tokio::task::spawn_blocking(move || {
            let raw = track_clone
                .id
                .strip_prefix("local:")
                .unwrap_or(&track_clone.id);
            let path = resolve_existing_local_path(raw).ok_or_else(|| {
                anyhow!("Local audio file not found for track '{}'", track_clone.id)
            })?;
            let canonical = std::fs::canonicalize(&path).map_err(|err| {
                anyhow!(
                    "Failed to canonicalize local audio file '{}': {err}",
                    track_clone.id
                )
            })?;
            if !canonical.is_file()
                || !sonata_core::providers::local::is_supported_audio_path(&canonical)
            {
                return Err(anyhow!(
                    "Local path for track '{}' is not a supported audio file",
                    track_clone.id
                ));
            }
            let stream = build_local_resolved_stream(&canonical, &track_clone, "Local");
            tracing::debug!(
                "[sonata:resolver] result=ok method=local path={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            Ok(stream)
        })
        .await
        .map_err(|err| anyhow!("Local file resolve task panicked: {err}"))?
    }

    async fn resolve_smb_stream(&self, track: &Track) -> Result<ResolvedStream> {
        let paths = self.paths().clone();
        let track_clone = track.clone();
        let mounted =
            tokio::task::spawn_blocking(move || resolve_smb_mounted_sync(&paths, &track_clone))
                .await
                .map_err(|err| anyhow!("SMB file resolve task panicked: {err}"))??;

        if let Some(stream) = mounted {
            tracing::debug!(
                "[sonata:resolver] result=ok method=smb-local path={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            return Ok(stream);
        }

        if track.is_self_hosted() {
            return self.resolve_self_hosted_stream(track).await;
        }

        Err(anyhow!(
            "SMB audio file not found or share is not mounted for track '{}'",
            track.id
        ))
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
        let source_label = profile.kind.display_name().to_string();
        let base_badge = stream
            .audio_badge
            .clone()
            .or_else(|| track.audio_badge.clone())
            .unwrap_or_else(|| source_label.clone());

        if stream.url.starts_with("file://") || stream.url.starts_with('/') {
            let track_clone = track.clone();
            let raw_url = stream.url.clone();
            let headers = stream.headers.clone();
            if let Ok(Some(local_stream)) = tokio::task::spawn_blocking(move || {
                resolve_existing_local_path(&raw_url).map(|path| {
                    let mut resolved =
                        build_local_resolved_stream(&path, &track_clone, &source_label);
                    resolved.headers = headers;
                    resolved
                })
            })
            .await
            {
                tracing::debug!(
                    "[sonata:resolver] result=ok method=self-hosted-local kind={:?} url={} badge={}",
                    profile.kind,
                    redact_stream_url(&local_stream.url),
                    local_stream.badge
                );
                return Ok(local_stream);
            }
        }

        tracing::debug!(
            "[sonata:resolver] result=ok method=self-hosted kind={:?} url={} badge={}",
            profile.kind,
            redact_stream_url(&stream.url),
            base_badge
        );
        Ok(ResolvedStream {
            url: stream.url,
            badge: base_badge,
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
        let js_runtime = if JS_RUNTIMES_UNSUPPORTED.load(Ordering::Relaxed) {
            None
        } else {
            tokio::task::spawn_blocking(detect_js_runtime_arg)
                .await
                .ok()
                .flatten()
        };
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
        let mut cookie_guard = None;
        let cookie_path = if use_cookies {
            if let Some(cookie_file) = self.core.cookies().netscape_file.clone() {
                if let Some(run_copy) = copy_cookie_file_private(cookie_file).await {
                    cookie_guard = Some(TempFileGuard(run_copy.clone()));
                    Some(run_copy)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut output = run_ytdlp_invocation(
            &ytdlp_bin,
            js_runtime,
            format_selector,
            output_template,
            cookie_path.as_deref(),
            &target_url,
            use_cookies,
        )
        .await?;

        if !output.status.success() && js_runtime.is_some() {
            let stderr_raw = String::from_utf8_lossy(&output.stderr);
            if is_js_runtimes_unsupported_stderr(&stderr_raw) {
                JS_RUNTIMES_UNSUPPORTED.store(true, Ordering::Relaxed);
                tracing::info!(
                    "[sonata:resolver] yt-dlp does not support --js-runtimes; caching and retrying without it"
                );
                output = run_ytdlp_invocation(
                    &ytdlp_bin,
                    None,
                    format_selector,
                    output_template,
                    cookie_path.as_deref(),
                    &target_url,
                    use_cookies,
                )
                .await?;
            }
        }
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

async fn run_ytdlp_invocation(
    ytdlp_bin: &str,
    js_runtime: Option<&str>,
    format_selector: &str,
    output_template: &str,
    cookie_file: Option<&Path>,
    target_url: &str,
    use_cookies: bool,
) -> Result<std::process::Output> {
    let mut command = Command::new(ytdlp_bin);
    configure_background_tokio_command(&mut command);
    command.kill_on_drop(true);
    command.args([
        "--no-playlist",
        "--no-warnings",
        "--quiet",
        "--socket-timeout",
        "5",
    ]);
    if let Some(runtime) = js_runtime {
        command.arg("--js-runtimes").arg(runtime);
        tracing::debug!("[sonata:resolver] yt-dlp js-runtime={runtime}");
    }
    command.args(["-f", format_selector, "--print", output_template]);
    if let Some(path) = cookie_file {
        command.arg("--cookies").arg(path);
    }
    command.arg("--").arg(target_url);

    tracing::debug!(
        "[sonata:resolver] method=yt-dlp attempt=1/1 cookies={}",
        if use_cookies { "present" } else { "none" }
    );
    tokio::time::timeout(Duration::from_secs(8), command.output())
        .await
        .map_err(|_| anyhow!("yt-dlp timed out while resolving stream"))?
        .map_err(Into::into)
}

fn is_js_runtimes_unsupported_stderr(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("no such option: --js-runtimes")
        || (lower.contains("no such option") && lower.contains("--js-runtimes"))
}

fn format_js_runtime_arg(
    candidate: &str,
    resolved_bin: &str,
    path_env: Option<&std::ffi::OsStr>,
) -> String {
    let resolved_path = Path::new(resolved_bin);
    let in_path = path_env.is_some_and(|env_val| {
        std::env::split_paths(env_val).any(|dir| {
            resolved_path.parent() == Some(dir.as_path()) && dir.join(candidate).is_file()
        })
    });
    if !in_path && resolved_bin != candidate && resolved_path.is_absolute() {
        format!("{candidate}:{resolved_bin}")
    } else {
        candidate.to_string()
    }
}

fn detect_js_runtime_entry() -> Option<(&'static str, String)> {
    static ENTRY: OnceLock<Option<(&'static str, String)>> = OnceLock::new();
    ENTRY
        .get_or_init(|| {
            let path_env = std::env::var_os("PATH");
            ["deno", "node", "bun", "quickjs"]
                .into_iter()
                .find_map(|candidate| {
                    let bin = resolve_distro_binary(candidate);
                    let mut cmd = std::process::Command::new(&bin);
                    configure_background_std_command(&mut cmd);
                    let ok = cmd
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .is_ok_and(|status| status.success());
                    if ok {
                        let arg = format_js_runtime_arg(candidate, &bin, path_env.as_deref());
                        Some((candidate, arg))
                    } else {
                        None
                    }
                })
        })
        .clone()
}

/// Detects the first installed JS runtime supported by `yt-dlp --js-runtimes`.
pub fn detect_js_runtime() -> Option<&'static str> {
    detect_js_runtime_entry().map(|(name, _)| name)
}

fn detect_js_runtime_arg() -> Option<&'static str> {
    static ARG: OnceLock<Option<String>> = OnceLock::new();
    ARG.get_or_init(|| detect_js_runtime_entry().map(|(_, arg)| arg))
        .as_deref()
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

fn is_local_track_id(track_id: &str) -> bool {
    track_id.starts_with("local:") || track_id.starts_with("file://")
}

fn is_smb_track_id(track_id: &str) -> bool {
    track_id.starts_with("smb:")
}

fn is_local_or_smb_track_id(track_id: &str) -> bool {
    is_local_track_id(track_id) || is_smb_track_id(track_id)
}

fn strip_file_scheme(raw: &str) -> &str {
    if raw.starts_with("file://localhost/") {
        // Re-anchor to root `/` when stripping `file://localhost`.
        &raw["file://localhost".len()..]
    } else if let Some(rest) = raw.strip_prefix("file://") {
        rest
    } else {
        raw
    }
}

fn percent_decode_str(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (h1, h2) {
                out.push(((hi << 4) | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve_existing_local_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = strip_file_scheme(trimmed);
    if !stripped.is_empty() && Path::new(stripped).is_file() {
        return Some(PathBuf::from(stripped));
    }
    let decoded = percent_decode_str(stripped);
    let decoded_stripped = strip_file_scheme(decoded.trim());
    if !decoded_stripped.is_empty() && Path::new(decoded_stripped).is_file() {
        return Some(PathBuf::from(decoded_stripped));
    }
    None
}

fn join_mounted_relative_path(mount_root: &Path, raw_item: &str) -> Option<PathBuf> {
    let canonical_root = std::fs::canonicalize(mount_root).ok()?;
    let try_join = |candidate: &str| -> Option<PathBuf> {
        let trimmed = candidate.trim();
        if trimmed.is_empty() || trimmed.starts_with("file://") || trimmed.starts_with('/') {
            return None;
        }
        let mut joined = canonical_root.clone();
        for comp in Path::new(trimmed).components() {
            match comp {
                Component::Normal(part) => joined.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        let canonical_target = std::fs::canonicalize(&joined).ok()?;
        (canonical_target.starts_with(&canonical_root) && canonical_target.is_file())
            .then_some(canonical_target)
    };

    try_join(raw_item).or_else(|| {
        let decoded = percent_decode_str(raw_item);
        try_join(&decoded)
    })
}

fn contains_smb_escape_attempt(raw_item: &str) -> bool {
    let check = |s: &str| {
        let trimmed = s.trim();
        trimmed.starts_with("file://")
            || Path::new(trimmed)
                .components()
                .any(|c| matches!(c, Component::ParentDir))
    };
    check(raw_item) || check(&percent_decode_str(raw_item))
}

fn infer_local_codec(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.trim();
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();
    let mapped = match ext_lower.as_str() {
        "ogg" | "oga" => "vorbis",
        "m4a" => "aac",
        other => other,
    };
    Some(normalize_audio_codec_label(mapped))
}

fn build_local_resolved_stream(path: &Path, track: &Track, source_label: &str) -> ResolvedStream {
    let codec = infer_local_codec(path);
    let measured_kbps = track
        .duration_secs
        .filter(|&secs| secs > 0)
        .and_then(|secs| compute_local_file_bitrate_kbps(path, secs as f64));
    let badge = format_measured_audio_badge(
        codec.as_deref(),
        measured_kbps,
        None,
        None,
        track.audio_badge.as_deref(),
    )
    .or_else(|| track.audio_badge.clone())
    .or_else(|| codec.map(|c| format!("{c} · {source_label}")))
    .unwrap_or_else(|| source_label.to_string());
    ResolvedStream {
        url: path.to_string_lossy().into_owned(),
        badge,
        headers: Vec::new(),
    }
}

fn resolve_smb_mounted_sync(paths: &CorePaths, track: &Track) -> Result<Option<ResolvedStream>> {
    let rest = track
        .id
        .strip_prefix("smb:")
        .ok_or_else(|| anyhow!("Track is not an SMB item"))?;

    let rest_trimmed = rest.trim();
    let rest_decoded = percent_decode_str(rest_trimmed);
    if rest_trimmed.starts_with("file://")
        || rest_trimmed.starts_with('/')
        || rest_decoded.trim().starts_with("file://")
        || rest_decoded.trim().starts_with('/')
    {
        return Err(anyhow!(
            "Invalid SMB track path '{}': external absolute and file:// paths are rejected",
            track.id
        ));
    }

    let Some((raw_server_id, raw_item_id)) = rest_trimmed.split_once(':') else {
        return Ok(None);
    };
    if contains_smb_escape_attempt(raw_item_id) {
        return Err(anyhow!(
            "Invalid SMB track path '{}': traversal and file:// URIs are rejected",
            track.id
        ));
    }
    let item_trimmed = raw_item_id.trim();
    let item_decoded = percent_decode_str(item_trimmed);
    let is_abs_item =
        Path::new(item_trimmed).is_absolute() || Path::new(item_decoded.trim()).is_absolute();

    let server_id = percent_decode_str(raw_server_id);
    let mut mount_root: Option<PathBuf> = None;

    let servers_path = paths.servers_config_path();
    if servers_path.is_file() {
        if let Ok(raw_json) = std::fs::read_to_string(&servers_path) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&raw_json) {
                if let Some(entries) = parsed.get("servers").and_then(Value::as_array) {
                    if let Some(profile) = entries
                        .iter()
                        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(&server_id))
                    {
                        if profile.get("enabled").and_then(Value::as_bool) == Some(false) {
                            let name = profile
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or(&server_id);
                            return Err(anyhow!("Server '{name}' is disabled"));
                        }
                        mount_root = profile
                            .get("local_mount_path")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(PathBuf::from)
                            .or_else(|| {
                                profile
                                    .get("base_url")
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|s| s.starts_with("file://") || s.starts_with('/'))
                                    .map(strip_file_scheme)
                                    .filter(|s| !s.is_empty())
                                    .map(PathBuf::from)
                            });
                    }
                }
            }
        }
    }

    if is_abs_item {
        let Some(root) = mount_root else {
            return Err(anyhow!(
                "Invalid SMB track path '{}': external absolute path outside mount",
                track.id
            ));
        };
        let canonical_root = std::fs::canonicalize(&root).map_err(|err| {
            anyhow!(
                "Failed to canonicalize SMB mount root '{}': {err}",
                root.display()
            )
        })?;
        for candidate in [item_trimmed, item_decoded.trim()] {
            if let Ok(canonical_target) = std::fs::canonicalize(candidate) {
                if canonical_target.starts_with(&canonical_root) && canonical_target.is_file() {
                    return Ok(Some(build_local_resolved_stream(
                        &canonical_target,
                        track,
                        "SMB",
                    )));
                }
            }
        }
        return Err(anyhow!(
            "Invalid SMB track path '{}': path escapes configured SMB mount",
            track.id
        ));
    }

    if let Some(root) = mount_root {
        if let Some(path) = join_mounted_relative_path(&root, raw_item_id) {
            return Ok(Some(build_local_resolved_stream(&path, track, "SMB")));
        }
    }
    Ok(None)
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

    #[tokio::test]
    async fn resolve_fast_resolves_local_files_with_measured_bitrate_without_network() {
        use crate::downloads::test_support::TempTestDir;
        use sonata_core::providers::profile::{FastStartMode, StreamFallbackMode, UpgradeDelivery};

        let core_dir = TempTestDir::new("ytdlp_local_core");
        let music_dir = TempTestDir::new("ytdlp_local_music");
        let flac_path = music_dir.path().join("01 Test Song.flac");
        // 220,000 bytes over 2 seconds = 880 kbps exact
        std::fs::write(&flac_path, vec![0u8; 220_000]).expect("write test flac");

        let http = sonata_core::providers::build_http_client();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let paths = CorePaths::for_app_roots(core_dir.path(), core_dir.path());
        let profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 1024 * 1024,
        };
        let resolver = LinuxStreamResolver::with_profile(http, ytm, cookies, None, paths, profile);

        let encoded_path = flac_path.to_string_lossy().replace(' ', "%20");
        let track = Track {
            id: format!("local:{encoded_path}"),
            title: "Test Song".to_string(),
            artist: "Local Artist".to_string(),
            album: "Local Album".to_string(),
            duration_secs: Some(2),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("FLAC · Local".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };

        let resolved = resolver
            .resolve_fast(&track)
            .await
            .expect("local file should resolve offline");
        assert_eq!(resolved.url, flac_path.to_string_lossy());
        assert_eq!(resolved.badge, "FLAC · 880 kbps");
        assert!(resolved.headers.is_empty());

        let recovered = resolver
            .resolve_recovery(&track)
            .await
            .expect("local recovery should resolve offline");
        assert_eq!(recovered.url, flac_path.to_string_lossy());
        assert_eq!(recovered.badge, "FLAC · 880 kbps");

        assert!(resolver
            .resolve_hifi_upgrade(&track, &resolved, true)
            .await
            .is_none());
        assert!(resolver.precache_hifi_audio(&track, true).await.is_none());

        // Also accept `local:file://...` URI form.
        let file_uri_track = Track {
            id: format!("local:file://{encoded_path}"),
            ..track.clone()
        };
        let uri_resolved = resolver
            .resolve_fast(&file_uri_track)
            .await
            .expect("file:// local track should resolve");
        assert_eq!(uri_resolved.url, flac_path.to_string_lossy());
        assert_eq!(uri_resolved.badge, "FLAC · 880 kbps");

        // Missing local file must error cleanly without invoking yt-dlp.
        let missing = Track {
            id: format!("local:{}/missing.flac", music_dir.path().display()),
            ..track.clone()
        };
        assert!(resolver.resolve_fast(&missing).await.is_err());

        // Non-audio local file must be rejected.
        let secret_txt = music_dir.path().join("notes.txt");
        std::fs::write(&secret_txt, b"private data").expect("write txt");
        let non_audio = Track {
            id: format!("local:{}", secret_txt.display()),
            ..track
        };
        assert!(resolver.resolve_fast(&non_audio).await.is_err());
    }

    #[tokio::test]
    async fn resolve_fast_resolves_mounted_smb_files_and_rejects_disabled_or_missing() {
        use crate::downloads::test_support::TempTestDir;
        use sonata_core::providers::profile::{FastStartMode, StreamFallbackMode, UpgradeDelivery};

        let core_dir = TempTestDir::new("ytdlp_smb_core");
        let smb_mount = TempTestDir::new("ytdlp_smb_mount");
        let album_dir = smb_mount.path().join("Artist/Album");
        std::fs::create_dir_all(&album_dir).expect("create album dir");
        let mp3_path = album_dir.join("02 Track.mp3");
        // 80,000 bytes over 2 seconds = 320 kbps exact
        std::fs::write(&mp3_path, vec![0u8; 80_000]).expect("write test mp3");

        let paths = CorePaths::for_app_roots(core_dir.path(), core_dir.path());
        std::fs::create_dir_all(&paths.config_dir).expect("create config dir");
        let servers_json = serde_json::json!({
            "version": 1,
            "servers": [
                {
                    "id": "nas-smb",
                    "name": "Home NAS",
                    "kind": "smb",
                    "base_url": "smb://nas.local/music",
                    "enabled": true,
                    "local_mount_path": smb_mount.path().to_string_lossy(),
                },
                {
                    "id": "nas-disabled",
                    "name": "Disabled NAS",
                    "kind": "smb",
                    "base_url": "smb://nas.local/disabled",
                    "enabled": false,
                    "local_mount_path": smb_mount.path().to_string_lossy(),
                }
            ]
        });
        std::fs::write(
            paths.servers_config_path(),
            serde_json::to_string_pretty(&servers_json).expect("serialize servers"),
        )
        .expect("write servers.json");

        let http = sonata_core::providers::build_http_client();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 1024 * 1024,
        };
        let resolver = LinuxStreamResolver::with_profile(http, ytm, cookies, None, paths, profile);

        let smb_track = Track {
            id: "smb:nas-smb:Artist/Album/02%20Track.mp3".to_string(),
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(2),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("MP3 · SMB".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };

        let resolved = resolver
            .resolve_fast(&smb_track)
            .await
            .expect("mounted SMB track should resolve offline");
        assert_eq!(resolved.url, mp3_path.to_string_lossy());
        assert_eq!(resolved.badge, "MP3 · 320 kbps");
        assert!(resolved.headers.is_empty());
        assert!(resolver
            .resolve_hifi_upgrade(&smb_track, &resolved, true)
            .await
            .is_none());
        assert!(resolver
            .precache_hifi_audio(&smb_track, true)
            .await
            .is_none());

        // Path traversal must be rejected.
        let traversal_track = Track {
            id: "smb:nas-smb:../etc/passwd".to_string(),
            ..smb_track.clone()
        };
        assert!(resolver.resolve_fast(&traversal_track).await.is_err());

        // External absolute paths, file:// URIs, and symlink escapes must be rejected.
        let outside_dir = TempTestDir::new("ytdlp_smb_outside");
        let outside_mp3 = outside_dir.path().join("outside.mp3");
        std::fs::write(&outside_mp3, vec![0u8; 40_000]).expect("write outside mp3");

        for bad_id in [
            format!("smb:file://{}", outside_mp3.display()),
            format!("smb:{}", outside_mp3.display()),
            format!("smb:nas-smb:{}", outside_mp3.display()),
            format!("smb:nas-smb:file://{}", outside_mp3.display()),
            format!("smb:nas-smb:file://{}", mp3_path.display()),
        ] {
            let bad_track = Track {
                id: bad_id.clone(),
                ..smb_track.clone()
            };
            assert!(
                resolver.resolve_fast(&bad_track).await.is_err(),
                "expected SMB escape id `{bad_id}` to be rejected"
            );
        }

        #[cfg(unix)]
        {
            let escape_link = album_dir.join("escape.mp3");
            std::os::unix::fs::symlink(&outside_mp3, &escape_link).expect("create symlink");
            let symlink_track = Track {
                id: "smb:nas-smb:Artist/Album/escape.mp3".to_string(),
                ..smb_track.clone()
            };
            assert!(resolver.resolve_fast(&symlink_track).await.is_err());
        }

        // Disabled SMB server must return an error.
        let disabled_track = Track {
            id: "smb:nas-disabled:Artist/Album/02%20Track.mp3".to_string(),
            ..smb_track
        };
        let err = resolver
            .resolve_fast(&disabled_track)
            .await
            .expect_err("disabled SMB server must fail");
        assert!(err.to_string().contains("disabled"));
    }

    #[test]
    fn js_runtime_arg_includes_colon_path_for_out_of_path_binaries() {
        use std::ffi::OsStr;

        assert_eq!(
            format_js_runtime_arg(
                "node",
                "/run/current-system/sw/bin/node",
                Some(OsStr::new("/nonexistent/bin"))
            ),
            "node:/run/current-system/sw/bin/node"
        );
        assert_eq!(
            format_js_runtime_arg("node", "/usr/bin/node", None),
            "node:/usr/bin/node"
        );
        assert_eq!(format_js_runtime_arg("node", "node", None), "node");

        let dir = crate::downloads::test_support::TempTestDir::new("js_rt_in_path");
        let bin_path = dir.path().join("node");
        std::fs::write(&bin_path, b"").expect("create dummy node in path");
        assert_eq!(
            format_js_runtime_arg(
                "node",
                &bin_path.to_string_lossy(),
                Some(dir.path().as_os_str())
            ),
            "node"
        );

        assert!(is_js_runtimes_unsupported_stderr(
            "Usage: yt-dlp [OPTIONS] URL...\n\nyt-dlp: error: no such option: --js-runtimes"
        ));
        assert!(!is_js_runtimes_unsupported_stderr("HTTP Error 403"));
    }

    #[tokio::test]
    async fn jiosaavn_only_precache_runs_when_hifi_substitution_is_false() {
        use crate::downloads::test_support::TempTestDir;
        use crate::downloads::{DownloadStore, DownloadTrackInput};
        use sonata_core::providers::profile::{FastStartMode, StreamFallbackMode, UpgradeDelivery};

        let core_dir = TempTestDir::new("ytdlp_jio_precache_core");
        let downloads_dir = TempTestDir::new("ytdlp_jio_precache_dl");
        let store = DownloadStore::open(downloads_dir.path()).expect("open download store");

        let track = Track {
            id: "ytm_jio_1".to_string(),
            title: "Jio Only Precache".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
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
                b"ftypM4A_jiosaavn_payload",
                None,
            )
            .expect("save offline track");

        // Also seed a cached opus stream to test non-offline precache gate when allow_hifi=false.
        let http = sonata_core::providers::build_http_client();
        let cookies = ExtractedCookies::default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let paths = CorePaths::for_app_roots(core_dir.path(), core_dir.path());
        let profile = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(500),
            jiosaavn_enabled: true,
            cache_limit_bytes: 1024 * 1024,
        };
        assert!(LinuxStreamResolver::precache_policy_allows(&profile, false));
        let disabled_both = ResolverProfile {
            jiosaavn_enabled: false,
            ..profile.clone()
        };
        assert!(!LinuxStreamResolver::precache_policy_allows(
            &disabled_both,
            false
        ));
        let no_cache = ResolverProfile {
            cache_limit_bytes: 0,
            ..profile.clone()
        };
        assert!(!LinuxStreamResolver::precache_policy_allows(
            &no_cache, false
        ));

        let resolver = LinuxStreamResolver::with_profile(http, ytm, cookies, None, paths, profile)
            .with_downloads_dir(downloads_dir.path());

        let hit = resolver
            .precache_hifi_audio(&track, false)
            .await
            .expect("jiosaavn-only precache should proceed when allow_hifi is false");
        assert_eq!(hit.url, saved.audio_path.to_string_lossy());
    }
}
