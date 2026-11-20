use crate::auth::ExtractedCookies;
use crate::cache::{is_stream_url_fresh, prune_media_cache, CacheDb};
use crate::config::AppConfig;
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::Mutex;

const YT_INNERTUBE_PLAYER_API: &str =
    "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
const MAX_MEM_CACHE_ENTRIES: usize = 256;
const MEM_CACHE_TTL: Duration = Duration::from_secs(20 * 60);
const MAX_AUDIO_DOWNLOAD_BYTES: usize = 80 * 1024 * 1024;

fn sanitize_track_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "track".to_string()
    } else {
        cleaned
    }
}

fn audio_file_paths(track_id: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let safe = sanitize_track_id(track_id);
    let dir = AppConfig::audio_cache_dir();
    // Versioned paths prevent previously sourced upgrades from being reused as YTM audio.
    (
        dir.join(format!("{safe}.v3.audio")),
        dir.join(format!("{safe}.v3.badge")),
    )
}

fn looks_like_audio(bytes: &[u8]) -> bool {
    bytes.starts_with(b"fLaC")
        || bytes.starts_with(b"OggS")
        || bytes.starts_with(b"ID3")
        || bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
        || (bytes.len() >= 8 && &bytes[4..8] == b"ftyp")
        || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE")
}

fn redact_stream_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        if let Some(scheme_end) = url.find("://") {
            let rest = &url[scheme_end + 3..];
            if let Some(path_end) = rest.find('/') {
                let host = &rest[..path_end];
                return format!("{}://{host}/[redacted-stream-url]", &url[..scheme_end]);
            }
            return format!("{}://{rest}/[redacted-stream-url]", &url[..scheme_end]);
        }
    }
    "[local-or-redacted-url]".to_string()
}

fn redact_sensitive_text(text: &str) -> String {
    let mut result = text.to_string();
    for secret in [
        "Cookie:",
        "cookies=",
        "cookie=",
        "Authorization=",
        "SAPISID=",
    ] {
        if let Some(pos) = result
            .to_ascii_lowercase()
            .find(&secret.to_ascii_lowercase())
        {
            let prefix = &result[..pos];
            let rest = &result[pos + secret.len()..];
            let end = rest.find('\n').unwrap_or(rest.len());
            result = format!("{prefix}[redacted-sensitive-header]{}", &rest[end..]);
        }
    }
    result
        .lines()
        .take(12)
        .collect::<Vec<_>>()
        .join(" | ")
        .replace('\r', "")
}

fn cached_audio_stream(track_id: &str) -> Option<ResolvedStream> {
    let (audio_path, badge_path) = audio_file_paths(track_id);
    let meta = std::fs::metadata(&audio_path).ok()?;
    if meta.len() < 8192 {
        return None;
    }
    let mut header = [0; 12];
    let header_len = std::fs::File::open(&audio_path)
        .ok()?
        .read(&mut header)
        .ok()?;
    if !looks_like_audio(&header[..header_len]) {
        eprintln!("[sonata:resolver] cache=discarded invalid-audio track_id={track_id}");
        let _ = std::fs::remove_file(&audio_path);
        let _ = std::fs::remove_file(&badge_path);
        return None;
    }
    let badge = std::fs::read_to_string(&badge_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "YTM · Cached".to_string());
    Some(ResolvedStream {
        url: audio_path.to_string_lossy().into_owned(),
        badge: if badge.contains("Cached") {
            badge
        } else {
            format!("{badge} · Cached")
        },
        headers: Vec::new(),
    })
}

struct TempFileGuard(std::path::PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn insert_bounded_cache(
    map: &mut HashMap<String, (Instant, ResolvedStream)>,
    key: String,
    stream: ResolvedStream,
) {
    map.retain(|_, (at, _)| at.elapsed() < MEM_CACHE_TTL);
    if map.len() >= MAX_MEM_CACHE_ENTRIES {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        {
            map.remove(&oldest);
        }
    }
    map.insert(key, (Instant::now(), stream));
}

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
        }
    }

    pub async fn warm(&self) {
        let _ = self
            .http
            .get("https://music.youtube.com/")
            .timeout(Duration::from_secs(3))
            .send()
            .await;
        let _ = detect_js_runtime();
        eprintln!("[sonata:resolver] warm=prerequisites ready");
    }

    /// Recovery after mpv rejects a stream. It does not repeat the cached
    /// resolver or wait through a second copy of the same fallback:
    /// authenticated yt-dlp first, then one unauthenticated attempt.
    pub async fn resolve_recovery(&self, track: &Track) -> Result<ResolvedStream> {
        self.invalidate_stream(&track.id).await;
        if self.cookies.has_ytm_auth() {
            if let Ok(stream) = self.resolve_youtube_ytdlp(track, true).await {
                eprintln!(
                    "[sonata:resolver] recovery=ok method=yt-dlp cookies-present track_id={} badge={}",
                    track.id,
                    stream.badge
                );
                let key = format!("yt:{}", track.id);
                self.put_cached(key, stream.clone()).await;
                return Ok(stream);
            }
        }
        match self.resolve_youtube_ytdlp(track, false).await {
            Ok(stream) => {
                eprintln!(
                    "[sonata:resolver] recovery=ok method=yt-dlp no-cookies track_id={} badge={}",
                    track.id, stream.badge
                );
                let key = format!("yt:{}", track.id);
                self.put_cached(key, stream.clone()).await;
                Ok(stream)
            }
            Err(error) => Err(error),
        }
    }

    pub fn needs_hifi_upgrade(badge: &str, url: &str) -> bool {
        let is_local = !url.starts_with("http");
        let lower = badge.to_ascii_lowercase();
        if lower.contains("flac") || lower.contains("alac") {
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
        if is_aac_320 || is_lossless_local || is_top_youtube_quality {
            return false;
        }
        true
    }

    pub async fn invalidate_stream(&self, track_id: &str) {
        {
            let mut guard = self.cache.lock().await;
            guard.remove(&format!("yt:{track_id}"));
            guard.remove(&format!("hifi:{track_id}"));
        }
        if let Some(db) = &self.cache_db {
            db.invalidate_stream(track_id);
        }
        let (audio_path, badge_path) = audio_file_paths(track_id);
        let _ = tokio::fs::remove_file(audio_path).await;
        let _ = tokio::fs::remove_file(badge_path).await;
    }

    async fn get_cached_fresh(&self, key: &str) -> Option<ResolvedStream> {
        {
            let guard = self.cache.lock().await;
            if let Some((at, stream)) = guard.get(key) {
                if at.elapsed() < MEM_CACHE_TTL
                    && is_stream_url_fresh(&stream.url)
                    && !stream.url.to_ascii_lowercase().contains("googlevideo.com")
                {
                    eprintln!(
                        "[sonata:resolver] cache-hit=memory key={key} source={}",
                        if stream.url.starts_with("http") {
                            "remote"
                        } else {
                            "local"
                        }
                    );
                    return Some(stream.clone());
                }
            }
        }
        if let Some(db) = &self.cache_db {
            if let Some((url, badge)) = db.get_stream(key) {
                if is_stream_url_fresh(&url)
                    && !url.to_ascii_lowercase().contains("googlevideo.com")
                {
                    let stream = ResolvedStream {
                        url,
                        badge,
                        headers: Vec::new(),
                    };
                    let mut guard = self.cache.lock().await;
                    insert_bounded_cache(&mut guard, key.to_string(), stream.clone());
                    eprintln!(
                        "[sonata:resolver] cache-hit=disk key={key} source={}",
                        if stream.url.starts_with("http") {
                            "remote"
                        } else {
                            "local"
                        }
                    );
                    return Some(stream);
                }
            }
        }
        None
    }

    async fn put_cached(&self, key: String, stream: ResolvedStream) {
        if stream.url.to_ascii_lowercase().contains("googlevideo.com") {
            eprintln!(
                "[sonata:resolver] cache=excluded volatile-source key={key} source=googlevideo"
            );
            return;
        }
        if let Some(db) = &self.cache_db {
            db.put_stream(&key, &stream.url, &stream.badge);
        }
        let mut guard = self.cache.lock().await;
        insert_bounded_cache(&mut guard, key, stream);
    }

    /// Resolve a direct YouTube media URL before any optional quality upgrade starts.
    pub async fn resolve_fast(&self, track: &Track) -> Result<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() > 0 {
            if let Some(local) = cached_audio_stream(&track.id) {
                eprintln!(
                    "[sonata:resolver] cache-hit=local track_id={} source=local",
                    track.id
                );
                self.cache_average_bitrate(&track.id, &local.badge);
                return Ok(local);
            }
        }
        let yt_key = format!("yt:{}", track.id);
        if let Some(stream) = self.get_cached_fresh(&yt_key).await {
            self.cache_average_bitrate(&track.id, &stream.badge);
            return Ok(stream);
        }

        let res = self.resolve_youtube_fast(track).await?;
        self.cache_average_bitrate(&track.id, &res.badge);
        self.put_cached(yt_key, res.clone()).await;
        Ok(res)
    }

    fn cache_average_bitrate(&self, track_id: &str, badge: &str) {
        if let (Some(db), Some(kbps)) = (&self.cache_db, extract_badge_kbps(badge)) {
            db.set_average_bitrate(track_id, kbps);
        }
    }

    /// Cache a higher-bitrate YTM format, or the current stream, before switching playback.
    pub async fn resolve_hifi_upgrade(
        &self,
        track: &Track,
        current: &ResolvedStream,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 {
            return None;
        }
        if track.source == ProviderSource::Spotify {
            if !allow_hifi || !Self::needs_hifi_upgrade(&current.badge, &current.url) {
                return None;
            }
            let current_kbps = extract_badge_kbps(&current.badge).unwrap_or(128);
            let mut candidate = None;
            if self.cookies.has_ytm_auth() {
                candidate = self.resolve_youtube_ytdlp(track, true).await.ok();
            }
            if candidate.is_none() {
                candidate = self.resolve_youtube_ytdlp(track, false).await.ok();
            }
            let candidate = candidate.filter(|stream| {
                should_upgrade_candidate(
                    current_kbps,
                    extract_badge_kbps(&stream.badge).unwrap_or(0),
                )
            })?;
            let local = self.download_to_audio_cache(&track.id, &candidate).await?;
            self.cache_average_bitrate(&track.id, &candidate.badge);
            return (local.url != current.url && local.badge != current.badge).then_some(local);
        }
        if let Some(local) = cached_audio_stream(&track.id) {
            if !Self::needs_hifi_upgrade(&local.badge, &local.url) && local.url != current.url {
                return Some(local);
            }
        }

        if allow_hifi && Self::needs_hifi_upgrade(&current.badge, &current.url) {
            let current_kbps = extract_badge_kbps(&current.badge).unwrap_or(128);
            let mut candidate = None;
            if self.cookies.has_ytm_auth() {
                candidate = self.resolve_youtube_ytdlp(track, true).await.ok();
            }
            if candidate.is_none() {
                candidate = self.resolve_youtube_ytdlp(track, false).await.ok();
            }
            if let Some(candidate) = candidate.filter(|stream| {
                should_upgrade_candidate(
                    current_kbps,
                    extract_badge_kbps(&stream.badge).unwrap_or(0),
                )
            }) {
                if candidate.url.starts_with("http") {
                    if let Some(local) = self.download_to_audio_cache(&track.id, &candidate).await {
                        self.cache_average_bitrate(&track.id, &candidate.badge);
                        return Some(local);
                    }
                }
            }
        }

        if track.source == ProviderSource::YouTubeMusic && current.url.starts_with("http") {
            return self.download_to_audio_cache(&track.id, current).await;
        }
        None
    }

    /// Cache a usable stream before queue advance; return remote stream if caching fails.
    pub async fn prefetch_hifi_audio(
        &self,
        track: &Track,
        allow_hifi: bool,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 {
            return self.resolve_fast(track).await.ok();
        }
        if let Some(local) = cached_audio_stream(&track.id) {
            if !allow_hifi || !Self::needs_hifi_upgrade(&local.badge, &local.url) {
                return Some(local);
            }
        }
        {
            let mut active = self.in_flight_prefetch.lock().ok()?;
            if !active.insert(track.id.clone()) {
                return cached_audio_stream(&track.id);
            }
        }
        let prefetch_guard = PrefetchGuard {
            id: track.id.clone(),
            set: Arc::clone(&self.in_flight_prefetch),
        };
        let result = async {
            let fast = self.resolve_fast(track).await.ok()?;
            if allow_hifi {
                if let Some(upgraded) = self.resolve_hifi_upgrade(track, &fast, allow_hifi).await {
                    return Some(upgraded);
                }
            }
            if fast.url.starts_with("http") {
                if let Some(local) = self.download_to_audio_cache(&track.id, &fast).await {
                    return Some(local);
                }
            }
            cached_audio_stream(&track.id).or(Some(fast))
        }
        .await;
        drop(prefetch_guard);
        result
    }

    async fn download_to_audio_cache(
        &self,
        track_id: &str,
        stream: &ResolvedStream,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 {
            return None;
        }
        let mut request = self.http.get(&stream.url);
        for (name, value) in &stream.headers {
            request = request.header(name, value);
        }
        let resp = request.timeout(Duration::from_secs(20)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let is_text = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.to_ascii_lowercase().starts_with("text/"));
        if is_text {
            return None;
        }
        if resp
            .content_length()
            .is_some_and(|len| len > MAX_AUDIO_DOWNLOAD_BYTES as u64)
        {
            return None;
        }
        let bytes = tokio::time::timeout(Duration::from_secs(20), resp.bytes())
            .await
            .ok()?
            .ok()?;
        if bytes.len() < 8192 || bytes.len() > MAX_AUDIO_DOWNLOAD_BYTES {
            return None;
        }
        if !looks_like_audio(&bytes) {
            return None;
        }
        let (audio_path, badge_path) = audio_file_paths(track_id);
        let clean_badge = stream
            .badge
            .strip_suffix(" · Cached")
            .unwrap_or(&stream.badge)
            .trim()
            .to_string();
        let saved = tokio::task::spawn_blocking(move || {
            let _ = std::fs::create_dir_all(AppConfig::audio_cache_dir());
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let pid = std::process::id();
            let part_path = audio_path.with_extension(format!("{pid}.{unique}.part"));
            if std::fs::write(&part_path, &bytes).is_err()
                || std::fs::rename(&part_path, &audio_path).is_err()
            {
                let _ = std::fs::remove_file(&part_path);
                return false;
            }
            if std::fs::write(&badge_path, &clean_badge).is_err() {
                let _ = std::fs::remove_file(&audio_path);
                let _ = std::fs::remove_file(&badge_path);
                return false;
            }
            prune_media_cache(AppConfig::active_cache_limit_bytes());
            true
        })
        .await
        .ok()?;
        if !saved {
            return None;
        }
        cached_audio_stream(track_id)
    }

    async fn resolve_youtube_fast(&self, track: &Track) -> Result<ResolvedStream> {
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
                    .and_then(|results| pick_best_ytm_match(track, &results).map(|b| b.id.clone()))
            }
        };

        eprintln!(
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

        if track.source == ProviderSource::YouTubeMusic {
            let has_cookies = self.cookies.has_ytm_auth();
            let ytdlp = async {
                if has_cookies {
                    if let Ok(stream) = self.resolve_youtube_ytdlp(track, true).await {
                        return Ok(stream);
                    }
                }
                self.resolve_youtube_ytdlp(track, false).await
            };
            tokio::pin!(ytdlp);

            if let Some(video_id) = video_id.as_deref() {
                let innertube = self.resolve_youtube_innertube(video_id, track.source);
                tokio::pin!(innertube);
                return tokio::select! {
                    stream = &mut innertube => match stream {
                        Some(stream) => {
                            eprintln!("[sonata:resolver] result=ok method=innertube url={} badge={}", redact_stream_url(&stream.url), stream.badge);
                            Ok(stream)
                        }
                        None => ytdlp.await,
                    },
                    result = &mut ytdlp => match result {
                        Ok(stream) => {
                            eprintln!("[sonata:resolver] result=ok method=yt-dlp url={} badge={}", redact_stream_url(&stream.url), stream.badge);
                            Ok(stream)
                        }
                        Err(_) => innertube.await.ok_or_else(|| anyhow!("Failed to resolve stream for \"{}\"", track.title)),
                    },
                };
            }
            return ytdlp.await;
        }

        // Keep the existing Spotify-to-YouTube resolution path unchanged.
        if let Some(vid) = &video_id {
            if let Some(fast) = self.resolve_youtube_innertube(vid, track.source).await {
                eprintln!(
                    "[sonata:resolver] result=ok method=innertube url={} badge={}",
                    redact_stream_url(&fast.url),
                    fast.badge
                );
                return Ok(fast);
            }
        }
        // yt-dlp is the robust fallback for tracks/ciphers not covered by InnerTube.
        if self.cookies.has_ytm_auth() {
            if let Ok(stream) = self.resolve_youtube_ytdlp(track, true).await {
                eprintln!(
                    "[sonata:resolver] result=ok method=yt-dlp cookies-present url={} badge={}",
                    redact_stream_url(&stream.url),
                    stream.badge
                );
                return Ok(stream);
            }
        }
        if let Ok(stream) = self.resolve_youtube_ytdlp(track, false).await {
            eprintln!(
                "[sonata:resolver] result=ok method=yt-dlp no-cookies url={} badge={}",
                redact_stream_url(&stream.url),
                stream.badge
            );
            return Ok(stream);
        }
        Err(anyhow!("Failed to resolve stream for \"{}\"", track.title))
    }

    async fn resolve_youtube_innertube(
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
                    if result.is_some() { return result; }
                }
                result = &mut android_vr, if !vr_done => {
                    vr_done = true;
                    if result.is_some() { return result; }
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
        let resp = self
            .http
            .post(YT_INNERTUBE_PLAYER_API)
            .header("User-Agent", user_agent)
            .header("Content-Type", "application/json")
            .json(body)
            .timeout(Duration::from_millis(3200))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let payload: Value = resp.json().await.ok()?;
        let mut stream = match source {
            ProviderSource::YouTubeMusic => parse_innertube_best_audio(&payload),
            ProviderSource::Spotify => parse_spotify_innertube_audio(&payload),
        }?;
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

    async fn resolve_youtube_ytdlp(
        &self,
        track: &Track,
        use_cookies: bool,
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

        eprintln!(
            "[sonata:resolver] method=yt-dlp start cookies={} target={}",
            if use_cookies { "present" } else { "none" },
            if target_url.starts_with("ytsearch1:") {
                "[ytsearch]"
            } else {
                "https://music.youtube.com/watch?v=[redacted-video-id]"
            }
        );

        let mut cmd = Command::new("yt-dlp");
        cmd.kill_on_drop(true);
        cmd.args([
            "--no-playlist",
            "--no-warnings",
            "--quiet",
            "--socket-timeout",
            "5",
        ]);
        if let Some(rt) = detect_js_runtime() {
            cmd.arg("--js-runtimes").arg(rt);
            eprintln!("[sonata:resolver] yt-dlp js-runtime={rt}");
        }
        let format_selector = match track.source {
            ProviderSource::YouTubeMusic => "774/141/251/140",
            ProviderSource::Spotify => {
                "bestaudio[acodec=flac]/bestaudio[acodec=opus]/bestaudio[acodec=aac]/bestaudio/best"
            }
        };
        let output_template = match track.source {
            ProviderSource::YouTubeMusic => {
                "%(acodec)s|%(abr)s|%(audio_ext)s|%(format_id)s|%(url)s|%(http_headers)j"
            }
            ProviderSource::Spotify => "%(acodec)s|%(abr)s|%(audio_ext)s|%(format_id)s|%(url)s",
        };
        cmd.args(["-f", format_selector, "--print", output_template]);
        let mut _temp_cookie_guard: Option<TempFileGuard> = None;
        if use_cookies {
            if let Some(cookie_file) = &self.cookies.netscape_file {
                if cookie_file.exists() {
                    let unique = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
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
                        cmd.arg("--cookies").arg(&run_copy);
                        _temp_cookie_guard = Some(TempFileGuard(run_copy));
                    }
                }
            }
        }
        cmd.arg("--").arg(&target_url);

        eprintln!(
            "[sonata:resolver] method=yt-dlp attempt=1/1 cookies={}",
            if use_cookies { "present" } else { "none" }
        );
        let output = tokio::time::timeout(Duration::from_secs(8), cmd.output())
            .await
            .map_err(|_| anyhow!("yt-dlp timed out while resolving stream"))??;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let err = redact_sensitive_text(err.trim());
            eprintln!(
                "[sonata:resolver] result=error method=yt-dlp exit={} stderr={}",
                output.status, err
            );
            return Err(anyhow!("yt-dlp failed: {err}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|l| l.contains('|'))
            .ok_or_else(|| anyhow!("No audio stream returned by yt-dlp"))?;
        let (codec, abr_num, format_id, stream_url, headers) = parse_ytdlp_audio_line(line)
            .ok_or_else(|| anyhow!("Unexpected yt-dlp stream metadata"))?;

        let lower_codec = codec.to_ascii_lowercase();
        let badge = match track.source {
            ProviderSource::YouTubeMusic => {
                youtube_quality_badge(&format_id, &lower_codec, abr_num)
            }
            ProviderSource::Spotify => match (lower_codec.as_str(), abr_num) {
                ("flac" | "alac", _) => "FLAC · Hi-Res".to_string(),
                (codec, Some(kbps)) if kbps >= 240 => {
                    format!("{} {}kbps · Hi-Res", codec.to_uppercase(), kbps)
                }
                (codec, Some(kbps)) if kbps > 0 => {
                    format!("{} {}kbps · YTM", codec.to_uppercase(), kbps)
                }
                (codec, _) => format!("{} · YTM", codec.to_uppercase()),
            },
        };

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

pub fn parse_innertube_best_audio(payload: &Value) -> Option<ResolvedStream> {
    let formats = payload
        .pointer("/streamingData/adaptiveFormats")?
        .as_array()?;
    let duration_secs = payload
        .pointer("/videoDetails/lengthSeconds")
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<u64>().ok());
    let mut best: Option<(u8, u64, String, String, String, Option<u32>)> = None;

    for format in formats {
        let Some(itag) = format.get("itag").and_then(Value::as_u64) else {
            continue;
        };
        let priority = match itag {
            774 => 0,
            141 => 1,
            251 => 2,
            140 => 3,
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
        let average_bps = format
            .get("averageBitrate")
            .and_then(Value::as_u64)
            .or_else(|| {
                let bytes = format
                    .get("contentLength")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<u64>().ok())?;
                let duration = duration_secs?;
                (duration > 0).then(|| bytes.saturating_mul(8) / duration)
            });
        let average_kbps = average_bps.map(|bps| (bps as f64 / 1000.0).round() as u32);
        let codec = if mime.to_ascii_lowercase().contains("opus") {
            "OPUS"
        } else {
            "AAC"
        };
        let candidate = (
            priority,
            average_bps.unwrap_or(0),
            itag.to_string(),
            url.to_string(),
            codec.to_string(),
            average_kbps,
        );
        if best.as_ref().is_none_or(|current| {
            candidate.0 < current.0 || (candidate.0 == current.0 && candidate.1 > current.1)
        }) {
            best = Some(candidate);
        }
    }

    let (_, _, format_id, url, codec, average_kbps) = best?;
    let badge = youtube_quality_badge(&format_id, &codec.to_ascii_lowercase(), average_kbps);
    Some(ResolvedStream {
        url,
        badge,
        headers: Vec::new(),
    })
}

fn parse_spotify_innertube_audio(payload: &Value) -> Option<ResolvedStream> {
    let adaptive = payload.pointer("/streamingData/adaptiveFormats");
    let legacy = payload.pointer("/streamingData/formats");
    let mut best: Option<(u64, String, String, u32)> = None;

    for formats in [adaptive, legacy].into_iter().flatten() {
        let Some(formats) = formats.as_array() else {
            continue;
        };
        for format in formats {
            let mime = format.get("mimeType").and_then(Value::as_str).unwrap_or("");
            if !mime.starts_with("audio/") {
                continue;
            }
            let Some(url) = format
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|url| url.starts_with("http"))
            else {
                continue;
            };
            let bitrate = format
                .get("averageBitrate")
                .and_then(Value::as_u64)
                .or_else(|| format.get("bitrate").and_then(Value::as_u64))
                .unwrap_or(128_000);
            let lower_mime = mime.to_ascii_lowercase();
            let (codec_rank, _) = rank_innertube_audio(&lower_mime, bitrate);
            let codec = if lower_mime.contains("flac") {
                "FLAC"
            } else if lower_mime.contains("opus") {
                "OPUS"
            } else {
                "AAC"
            };
            let kbps = ((bitrate as f64) / 1000.0).round() as u32;
            let score = ((codec_rank as u64) << 32).saturating_add(bitrate);
            if best
                .as_ref()
                .is_none_or(|(best_score, _, _, _)| score > *best_score)
            {
                best = Some((score, url.to_string(), codec.to_string(), kbps));
            }
        }
    }

    let (_, url, codec, kbps) = best?;
    let badge = if codec == "FLAC" {
        "FLAC · Hi-Res".to_string()
    } else if kbps >= 240 {
        format!("{codec} {kbps}kbps · Hi-Res")
    } else if kbps > 0 {
        format!("{codec} {kbps}kbps · YTM")
    } else {
        format!("{codec} · YTM")
    };
    Some(ResolvedStream {
        url,
        badge,
        headers: Vec::new(),
    })
}

fn rank_innertube_audio(mime: &str, bitrate: u64) -> (u8, u64) {
    if mime.contains("flac") {
        (3, bitrate)
    } else if mime.contains("opus") {
        (2, bitrate)
    } else if mime.contains("aac") || mime.contains("mp4a") {
        (1, bitrate)
    } else {
        (0, bitrate)
    }
}

fn youtube_quality_badge(format_id: &str, codec: &str, average_kbps: Option<u32>) -> String {
    let format_id = format_id.split('-').next().unwrap_or(format_id);
    let quality = match format_id {
        "774" | "141" => "Extreme",
        "251" => "HQ",
        "140" => "Standard",
        _ if average_kbps.is_some_and(|kbps| kbps <= 128) => "Standard",
        _ => "HQ",
    };
    match average_kbps.filter(|kbps| *kbps > 0) {
        Some(kbps) => format!("{} {kbps}kbps · {quality}", codec.to_ascii_uppercase()),
        None => format!("{} · {quality}", codec.to_ascii_uppercase()),
    }
}

fn extract_badge_kbps(badge: &str) -> Option<u32> {
    badge
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .find(|&n| n >= 48)
}

fn detect_js_runtime() -> Option<&'static str> {
    static RT: OnceLock<Option<&'static str>> = OnceLock::new();
    *RT.get_or_init(|| {
        ["deno", "node", "bun", "quickjs"].into_iter().find(|&c| {
            std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    })
}

fn pick_best_ytm_match<'a>(target: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    if candidates.is_empty() {
        return None;
    }
    candidates.iter().min_by_key(|cand| {
        let dur_diff = match (target.duration_secs, cand.duration_secs) {
            (Some(a), Some(b)) => a.abs_diff(b),
            _ => 10,
        };
        let title_penalty = if normalize_cmp(&cand.title).contains(&normalize_cmp(&target.title)) {
            0
        } else {
            30
        };
        dur_diff + title_penalty
    })
}

fn normalize_cmp(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn clean_search_title(title: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[](official|lyric|music|audio|video|hd|hq|4k|visualizer|remaster)[^\)\]]*[\)\]]").unwrap()
    });
    re.replace_all(title, "").trim().to_string()
}

fn should_upgrade_candidate(current_kbps: u32, candidate_kbps: u32) -> bool {
    candidate_kbps >= current_kbps.saturating_add(24)
}

fn parse_ytdlp_audio_line(
    line: &str,
) -> Option<(String, Option<u32>, String, String, Vec<(String, String)>)> {
    let parts: Vec<&str> = line.splitn(6, '|').collect();
    if parts.len() < 5 {
        return None;
    }
    let codec = parts[0].trim();
    let abr = parts[1]
        .trim()
        .parse::<f64>()
        .ok()
        .map(|v| v.round() as u32);
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
    Some((
        codec.to_string(),
        abr,
        format_id.to_string(),
        stream_url.to_string(),
        headers,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_320_aac_upgrade_over_263_opus() {
        assert!(should_upgrade_candidate(263, 320));
        assert!(!should_upgrade_candidate(320, 320));
    }

    #[test]
    fn parses_aac_metadata_from_ytdlp() {
        let parsed = parse_ytdlp_audio_line("aac|323.7|m4a|140|https://example.com/audio.m4a|{}");
        assert_eq!(
            parsed,
            Some((
                "aac".to_string(),
                Some(324),
                "140".to_string(),
                "https://example.com/audio.m4a".to_string(),
                Vec::new()
            ))
        );
    }

    #[test]
    fn bounds_in_memory_stream_cache() {
        let mut map = HashMap::new();
        for i in 0..(MAX_MEM_CACHE_ENTRIES + 10) {
            insert_bounded_cache(
                &mut map,
                format!("yt:{i}"),
                ResolvedStream {
                    url: format!("https://example.com/{i}"),
                    badge: "OPUS".to_string(),
                    headers: Vec::new(),
                },
            );
        }
        assert_eq!(map.len(), MAX_MEM_CACHE_ENTRIES);
    }

    #[test]
    fn sanitizes_track_id_for_audio_cache() {
        assert_eq!(sanitize_track_id("../evil/id?x=1"), "___evil_id_x_1");
        assert_eq!(sanitize_track_id("dQw4w9WgXcQ"), "dQw4w9WgXcQ");
    }

    #[test]
    fn parses_best_audio_from_innertube_player_payload() {
        let payload = json!({
            "streamingData": {
                "adaptiveFormats": [
                    {
                        "itag": 140,
                        "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
                        "averageBitrate": 129_500,
                        "url": "https://rr1.googlevideo.com/videoplayback?itag=140"
                    },
                    {
                        "itag": 251,
                        "mimeType": "audio/webm; codecs=\"opus\"",
                        "averageBitrate": 137_400,
                        "url": "https://rr1.googlevideo.com/videoplayback?itag=251"
                    }
                ]
            }
        });
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
    }
}
