use super::*;
use crate::cache::{is_stream_url_fresh, prune_media_cache};
use std::io::Read;
use tokio::io::AsyncWriteExt;

pub(super) const MAX_MEM_CACHE_ENTRIES: usize = 256;
const MEM_CACHE_TTL: Duration = Duration::from_secs(20 * 60);
const MAX_AUDIO_DOWNLOAD_BYTES: usize = 80 * 1024 * 1024;

struct TempAudioPart {
    path: std::path::PathBuf,
    keep: bool,
}

impl TempAudioPart {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TempAudioPart {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) fn sanitize_track_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
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

pub(super) fn audio_file_paths(track_id: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let safe_id = sanitize_track_id(track_id);
    let directory = AppConfig::audio_cache_dir();
    // Versioned paths prevent previously sourced upgrades from being reused as YTM audio.
    (
        directory.join(format!("{safe_id}.v3.audio")),
        directory.join(format!("{safe_id}.v3.badge")),
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

pub(super) fn cached_audio_stream(track_id: &str) -> Option<ResolvedStream> {
    let (audio_path, badge_path) = audio_file_paths(track_id);
    let metadata = std::fs::metadata(&audio_path).ok()?;
    if metadata.len() < 8192 {
        return None;
    }
    let mut header = [0; 12];
    let header_len = std::fs::File::open(&audio_path)
        .ok()?
        .read(&mut header)
        .ok()?;
    if !looks_like_audio(&header[..header_len]) {
        tracing::warn!("[sonata:resolver] cache=discarded invalid-audio track_id={track_id}");
        let _ = std::fs::remove_file(&audio_path);
        let _ = std::fs::remove_file(&badge_path);
        return None;
    }
    let badge = std::fs::read_to_string(&badge_path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "YTM · Cached".to_string());
    // Filesystems commonly mount with noatime. The badge is a small sidecar
    // whose modification time is the explicit LRU access marker.
    let _ = std::fs::write(&badge_path, &badge);
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

pub(super) fn insert_bounded_cache(
    cache: &mut HashMap<String, (Instant, ResolvedStream)>,
    key: String,
    stream: ResolvedStream,
) {
    cache.retain(|_, (inserted_at, _)| inserted_at.elapsed() < MEM_CACHE_TTL);
    if cache.len() >= MAX_MEM_CACHE_ENTRIES {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, (inserted_at, _))| *inserted_at)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest_key);
        }
    }
    cache.insert(key, (Instant::now(), stream));
}

impl StreamResolver {
    pub async fn invalidate_stream(&self, track_id: &str) {
        {
            let mut cache = self.cache.lock().await;
            cache.remove(&format!("yt:{track_id}"));
            cache.remove(&format!("hifi:{track_id}"));
        }
        if let Some(database) = &self.cache_db {
            database.invalidate_stream(track_id);
        }
        let (audio_path, badge_path) = audio_file_paths(track_id);
        let _ = tokio::fs::remove_file(audio_path).await;
        let _ = tokio::fs::remove_file(badge_path).await;
    }

    pub(super) async fn get_cached_fresh(&self, key: &str) -> Option<ResolvedStream> {
        {
            let cache = self.cache.lock().await;
            if let Some((inserted_at, stream)) = cache.get(key) {
                if inserted_at.elapsed() < MEM_CACHE_TTL
                    && is_stream_url_fresh(&stream.url)
                    && !stream.url.to_ascii_lowercase().contains("googlevideo.com")
                {
                    tracing::debug!(
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
        if let Some(database) = &self.cache_db {
            if let Some((url, badge)) = database.get_stream(key) {
                if is_stream_url_fresh(&url)
                    && !url.to_ascii_lowercase().contains("googlevideo.com")
                {
                    let stream = ResolvedStream {
                        url,
                        badge,
                        headers: Vec::new(),
                    };
                    let mut cache = self.cache.lock().await;
                    insert_bounded_cache(&mut cache, key.to_string(), stream.clone());
                    tracing::debug!(
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

    pub(super) async fn put_cached(&self, key: String, stream: ResolvedStream) {
        if stream.url.to_ascii_lowercase().contains("googlevideo.com") {
            tracing::debug!(
                "[sonata:resolver] cache=excluded volatile-source key={key} source=googlevideo"
            );
            return;
        }
        if let Some(database) = &self.cache_db {
            database.put_stream(&key, &stream.url, &stream.badge);
        }
        let mut cache = self.cache.lock().await;
        insert_bounded_cache(&mut cache, key, stream);
    }

    pub(super) fn cache_average_bitrate(&self, track_id: &str, badge: &str) {
        if let (Some(database), Some(kbps)) = (&self.cache_db, extract_badge_kbps(badge)) {
            database.set_average_bitrate(track_id, kbps);
        }
    }

    pub(super) async fn download_to_audio_cache(
        &self,
        track_id: &str,
        stream: &ResolvedStream,
    ) -> Option<ResolvedStream> {
        if AppConfig::active_cache_limit_bytes() == 0 || is_bounded_fast_start_url(&stream.url) {
            return None;
        }
        let mut request = self.http.get(&stream.url);
        for (name, value) in &stream.headers {
            request = request.header(name, value);
        }
        let mut response = request.timeout(Duration::from_secs(20)).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let is_text = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|content_type| content_type.to_ascii_lowercase().starts_with("text/"));
        if is_text
            || response
                .content_length()
                .is_some_and(|length| length > MAX_AUDIO_DOWNLOAD_BYTES as u64)
        {
            return None;
        }
        let (audio_path, badge_path) = audio_file_paths(track_id);
        let clean_badge = stream
            .badge
            .strip_suffix(" · Cached")
            .unwrap_or(&stream.badge)
            .trim()
            .to_string();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let process_id = std::process::id();
        let part_path = audio_path.with_extension(format!("{process_id}.{unique}.part"));
        let mut temporary = TempAudioPart::new(part_path.clone());
        tokio::fs::create_dir_all(AppConfig::audio_cache_dir())
            .await
            .ok()?;
        let mut file = tokio::fs::File::create(&part_path).await.ok()?;
        let mut bytes_written = 0_usize;
        let mut header = [0_u8; 12];
        let mut header_len = 0_usize;

        tracing::info!(
            event = "next_precache_downloading",
            track_id,
            quality = %clean_badge,
            "downloading audio cache entry"
        );
        loop {
            let next_chunk =
                match tokio::time::timeout(Duration::from_secs(20), response.chunk()).await {
                    Ok(Ok(chunk)) => chunk,
                    _ => {
                        tracing::warn!(
                            event = "next_precache_failed",
                            track_id,
                            reason = "download-timeout-or-error",
                            "audio cache download failed"
                        );
                        return None;
                    }
                };
            let Some(chunk) = next_chunk else { break };
            if bytes_written.saturating_add(chunk.len()) > MAX_AUDIO_DOWNLOAD_BYTES {
                tracing::warn!(
                    event = "next_precache_failed",
                    track_id,
                    reason = "download-too-large",
                    "audio cache download exceeded limit"
                );
                return None;
            }
            let available_header = header.len().saturating_sub(header_len).min(chunk.len());
            header[header_len..header_len + available_header]
                .copy_from_slice(&chunk[..available_header]);
            header_len += available_header;
            if file.write_all(&chunk).await.is_err() {
                tracing::warn!(
                    event = "next_precache_failed",
                    track_id,
                    reason = "cache-write-failed",
                    "audio cache write failed"
                );
                return None;
            }
            bytes_written += chunk.len();
        }
        if file.flush().await.is_err()
            || bytes_written < 8192
            || !looks_like_audio(&header[..header_len])
        {
            tracing::warn!(
                event = "next_precache_failed",
                track_id,
                reason = "invalid-audio",
                "downloaded cache entry is not audio"
            );
            return None;
        }
        drop(file);
        if tokio::fs::rename(&part_path, &audio_path).await.is_err()
            || tokio::fs::write(&badge_path, &clean_badge).await.is_err()
        {
            let _ = tokio::fs::remove_file(&audio_path).await;
            let _ = tokio::fs::remove_file(&badge_path).await;
            tracing::warn!(
                event = "next_precache_failed",
                track_id,
                reason = "cache-commit-failed",
                "audio cache commit failed"
            );
            return None;
        }
        temporary.keep();
        let limit = AppConfig::active_cache_limit_bytes();
        let _ = tokio::task::spawn_blocking(move || prune_media_cache(limit)).await;
        tracing::info!(
            event = "next_precache_ready",
            track_id,
            quality = %clean_badge,
            bytes = bytes_written,
            "audio cache entry ready"
        );
        cached_audio_stream(track_id)
    }
}
