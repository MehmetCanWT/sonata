use crate::config::AppConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static PRUNE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static LAST_SWEEP: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
const SWEEP_INTERVAL: Duration = Duration::from_secs(15);

struct MediaCacheEntry {
    paths: Vec<PathBuf>,
    bytes: u64,
    last_used: SystemTime,
}

/// Keep shared artwork, audio, and lyric files under the configured byte budget.
pub fn prune_media_cache(limit_bytes: u64) -> u64 {
    prune_media_cache_inner(limit_bytes, false)
}

/// Enforce a newly selected limit immediately, bypassing the normal sweep interval.
pub fn enforce_media_cache_limit(limit_bytes: u64) -> u64 {
    prune_media_cache_inner(limit_bytes, true)
}

fn prune_media_cache_inner(limit_bytes: u64, force: bool) -> u64 {
    let lock = PRUNE_LOCK.get_or_init(|| Mutex::new(()));
    let Ok(_guard) = lock.lock() else {
        tracing::warn!("[sonata:cache] prune skipped reason=cache-lock-poisoned");
        return 0;
    };
    let last_sweep = LAST_SWEEP.get_or_init(|| Mutex::new(None));
    let Ok(mut last_sweep) = last_sweep.lock() else {
        tracing::warn!("[sonata:cache] prune skipped reason=sweep-clock-poisoned");
        return 0;
    };
    if !force && limit_bytes > 0 && last_sweep.is_some_and(|last| last.elapsed() < SWEEP_INTERVAL) {
        return 0;
    }
    *last_sweep = Some(Instant::now());

    let mut grouped = HashMap::new();
    for (kind, directory) in [
        ("audio", AppConfig::audio_cache_dir()),
        ("art", AppConfig::art_cache_dir()),
        ("lyrics", AppConfig::lyrics_cache_dir()),
    ] {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.file_type().is_file() {
                continue;
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if extension == "tmp" || extension == "part" {
                let age = SystemTime::now()
                    .duration_since(metadata.modified().unwrap_or(UNIX_EPOCH))
                    .unwrap_or_default();
                if limit_bytes == 0 || age >= Duration::from_secs(60 * 60) {
                    let _ = std::fs::remove_file(path);
                }
                continue;
            }
            let group = match (kind, extension.as_str()) {
                ("audio", "audio") => path.file_stem().and_then(|name| name.to_str()),
                ("art", "png") => path.file_name().and_then(|name| name.to_str()),
                ("lyrics", "lrc" | "txt") => path.file_stem().and_then(|name| name.to_str()),
                ("audio", "badge") => {
                    if !path.with_extension("audio").exists() {
                        let _ = std::fs::remove_file(path);
                    }
                    continue;
                }
                _ => None,
            };
            let Some(group) = group else {
                continue;
            };
            let cache_entry = grouped
                .entry((kind.to_string(), group.to_string()))
                .or_insert_with(|| MediaCacheEntry {
                    paths: Vec::new(),
                    bytes: 0,
                    last_used: metadata.modified().unwrap_or(UNIX_EPOCH),
                });
            cache_entry.bytes = cache_entry.bytes.saturating_add(metadata.len());
            cache_entry.last_used = cache_entry
                .last_used
                .max(metadata.modified().unwrap_or(UNIX_EPOCH));
            cache_entry.paths.push(path.clone());
            if kind == "audio" {
                let badge_path = path.with_extension("badge");
                if let Ok(badge_metadata) = std::fs::symlink_metadata(&badge_path) {
                    if badge_metadata.file_type().is_file() {
                        cache_entry.bytes = cache_entry.bytes.saturating_add(badge_metadata.len());
                        cache_entry.last_used = cache_entry
                            .last_used
                            .max(badge_metadata.modified().unwrap_or(UNIX_EPOCH));
                        cache_entry.paths.push(badge_path);
                    }
                }
            }
        }
    }

    let mut entries: Vec<_> = grouped.into_values().collect();
    let mut remaining = entries
        .iter()
        .fold(0_u64, |total, entry| total.saturating_add(entry.bytes));
    let original = remaining;
    if remaining <= limit_bytes {
        return 0;
    }
    entries.sort_by_key(|entry| entry.last_used);
    let mut removed = 0_u64;
    for entry in entries {
        if remaining <= limit_bytes {
            break;
        }
        for path in entry.paths {
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if std::fs::remove_file(&path).is_ok() {
                    removed = removed.saturating_add(metadata.len());
                    remaining = remaining.saturating_sub(metadata.len());
                }
            }
        }
    }
    if removed > 0 {
        tracing::warn!(
            "[sonata:cache] media-pruned bytes_before={original} bytes_after={remaining} limit_bytes={limit_bytes} bytes_removed={removed}"
        );
    }
    removed
}

/// Run filesystem pruning away from UI and async executor threads.
pub async fn prune_active_media_cache_async() -> u64 {
    tokio::task::spawn_blocking(|| prune_media_cache(AppConfig::active_cache_limit_bytes()))
        .await
        .unwrap_or_default()
}

/// Apply the latest saved limit away from the async executor.
pub async fn enforce_active_media_cache_limit_async() -> u64 {
    tokio::task::spawn_blocking(|| enforce_media_cache_limit(AppConfig::active_cache_limit_bytes()))
        .await
        .unwrap_or_default()
}

/// Periodically reconcile shared media files with the active quota.
pub fn start_media_cache_maintenance() {
    tokio::spawn(async {
        let mut sweep = tokio::time::interval(Duration::from_secs(60));
        sweep.tick().await;
        loop {
            sweep.tick().await;
            enforce_active_media_cache_limit_async().await;
        }
    });
}
