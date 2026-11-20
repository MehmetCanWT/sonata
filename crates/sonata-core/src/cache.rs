use crate::config::AppConfig;
use crate::providers::{Playlist, Track};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CACHE_SCHEMA_VERSION: i64 = 2;
const MAX_CACHE_DB_PAGES: i64 = 32_768;
const MAX_CACHED_STREAMS: i64 = 2_048;
const MAX_CACHED_PLAYLISTS: i64 = 500;
const MAX_PLAYLIST_JSON_BYTES: usize = 16 * 1024 * 1024;

static MEDIA_CACHE_PRUNE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static LAST_MEDIA_CACHE_PRUNE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
const MEDIA_CACHE_SWEEP_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Default)]
pub struct TrackEnrichment {
    pub cover_url: Option<String>,
    pub cover_source: String,
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    pub lyrics_status: String,
    pub average_bitrate: u32,
    pub updated_at: i64,
}

/// SQLite metadata cache shared by the TUI and desktop clients.
#[derive(Clone)]
pub struct CacheDb {
    conn: Arc<Mutex<Connection>>,
    enabled: Arc<AtomicBool>,
}

impl CacheDb {
    pub fn open_default() -> Option<Self> {
        let path = AppConfig::cache_dir().join("sonata_cache.db");
        let cache = Self::open_at(&path)?;
        let enabled = AppConfig::active_cache_limit_bytes() > 0;
        cache.set_enabled(enabled);
        Some(cache)
    }

    pub fn open_at(path: &PathBuf) -> Option<Self> {
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "[sonata:cache] create-directory failed path={} error={error}",
                    parent.display()
                );
                return None;
            }
        }
        let conn = match Connection::open(path) {
            Ok(conn) => conn,
            Err(error) => {
                eprintln!(
                    "[sonata:cache] open failed path={} error={error}",
                    path.display()
                );
                return None;
            }
        };
        let _ = conn.busy_timeout(Duration::from_secs(5));
        if let Err(error) = conn.pragma_update(None, "max_page_count", MAX_CACHE_DB_PAGES) {
            eprintln!("[sonata:cache] database-size-limit failed error={error}");
        }
        let schema_version: i64 =
            match conn.pragma_query_value(None, "user_version", |row| row.get(0)) {
                Ok(version) => version,
                Err(error) => {
                    eprintln!("[sonata:cache] schema-version-read failed error={error}");
                    return None;
                }
            };
        if schema_version > CACHE_SCHEMA_VERSION {
            eprintln!(
                "[sonata:cache] schema-too-new found={schema_version} supported={CACHE_SCHEMA_VERSION} path={}",
                path.display()
            );
            return None;
        }
        if let Err(error) = conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -8192;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 16777216;
             PRAGMA max_page_count = 32768;
             BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS stream_cache (
                 cache_key TEXT PRIMARY KEY,
                 url TEXT NOT NULL,
                 badge TEXT NOT NULL,
                 expires_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS playlist_cache (
                 playlist_key TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 tracks_json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS library_cache (
                 library_key TEXT PRIMARY KEY,
                 playlists_json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS track_enrichment (
                 track_id TEXT PRIMARY KEY,
                 cover_url TEXT,
                 cover_source TEXT NOT NULL DEFAULT 'none',
                 synced_lyrics TEXT,
                 plain_lyrics TEXT,
                 lyrics_status TEXT NOT NULL DEFAULT 'NONE',
                 average_bitrate INTEGER NOT NULL DEFAULT 0,
                 updated_at INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS stream_cache_expiry_idx ON stream_cache(expires_at);
             CREATE INDEX IF NOT EXISTS playlist_cache_updated_idx ON playlist_cache(updated_at);
             CREATE INDEX IF NOT EXISTS track_enrichment_lyrics_idx ON track_enrichment(lyrics_status, updated_at);
             PRAGMA user_version = 2;
             COMMIT;",
        ) {
            eprintln!(
                "[sonata:cache] schema-initialize failed path={} error={error}",
                path.display()
            );
            return None;
        }
        let now = now_secs() as i64;
        if let Err(error) = conn.execute(
            "DELETE FROM stream_cache WHERE expires_at <= ?1",
            params![now],
        ) {
            eprintln!("[sonata:cache] stream-expiry-cleanup failed error={error}");
        }
        if let Err(error) = conn.execute(
            "DELETE FROM stream_cache WHERE lower(url) LIKE '%googlevideo.com%'",
            [],
        ) {
            eprintln!("[sonata:cache] volatile-stream-cleanup failed error={error}");
        }
        if let Err(error) = conn.execute(
            "UPDATE track_enrichment SET cover_url = NULL, cover_source = 'none'
             WHERE lower(COALESCE(cover_url, '')) LIKE '%saavncdn%'
                OR lower(COALESCE(cover_url, '')) LIKE '%jiosaavn%'",
            [],
        ) {
            eprintln!("[sonata:cache] legacy-cover-invalidation failed error={error}");
        }
        invalidate_legacy_covers(&conn);
        Some(Self {
            conn: Arc::new(Mutex::new(conn)),
            enabled: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Disable cache reads and writes, and discard retained database records when turned off.
    pub fn set_enabled(&self, enabled: bool) {
        let was_enabled = self.enabled.swap(enabled, Ordering::Relaxed);
        if !enabled && was_enabled {
            let conn = Arc::clone(&self.conn);
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(move || Self::clear_cached_records(&conn));
            } else {
                Self::clear_cached_records(&conn);
            }
        }
    }

    fn clear_cached_records(conn: &Mutex<Connection>) {
        let Ok(conn) = conn.lock() else {
            eprintln!("[sonata:cache] clear skipped reason=database-lock-poisoned");
            return;
        };
        if let Err(error) = conn.execute_batch(
            "BEGIN IMMEDIATE;
             DELETE FROM stream_cache;
             DELETE FROM playlist_cache;
             DELETE FROM library_cache;
             DELETE FROM track_enrichment;
             COMMIT;
             PRAGMA wal_checkpoint(TRUNCATE);
             VACUUM;",
        ) {
            eprintln!("[sonata:cache] clear failed error={error}");
        }
    }

    pub fn invalidate_stream(&self, track_id: &str) {
        if let Ok(guard) = self.conn.lock() {
            if let Err(error) = guard.execute(
                "DELETE FROM stream_cache WHERE cache_key IN (?1, ?2)",
                params![format!("yt:{track_id}"), format!("hifi:{track_id}")],
            ) {
                eprintln!("[sonata:cache] stream-invalidate failed error={error}");
            }
        }
    }

    pub fn get_track_enrichment(&self, track_id: &str) -> Option<TrackEnrichment> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let guard = self.conn.lock().ok()?;
        guard
            .query_row(
                "SELECT cover_url, cover_source, synced_lyrics, plain_lyrics, lyrics_status, average_bitrate, updated_at
                 FROM track_enrichment WHERE track_id = ?1",
                params![track_id],
                |row| {
                    Ok(TrackEnrichment {
                        cover_url: row.get(0)?,
                        cover_source: row.get(1)?,
                        synced_lyrics: row.get(2)?,
                        plain_lyrics: row.get(3)?,
                        lyrics_status: row.get(4)?,
                        average_bitrate: row.get::<_, i64>(5)?.max(0) as u32,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .ok()
    }

    pub fn set_track_cover(&self, track_id: &str, url: &str, source: &str) {
        if !self.enabled.load(Ordering::Relaxed) || !matches!(source, "apple" | "ytm") {
            return;
        }
        if let Ok(conn) = self.conn.lock() {
            if let Err(error) = conn.execute(
                "INSERT INTO track_enrichment (track_id, cover_url, cover_source)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_id) DO UPDATE SET cover_url = excluded.cover_url, cover_source = excluded.cover_source",
                params![track_id, url, source],
            ) {
                eprintln!("[sonata:cache] cover-write failed track_id={track_id} error={error}");
            }
        }
    }

    pub fn set_track_lyrics(
        &self,
        track_id: &str,
        synced: Option<&str>,
        plain: Option<&str>,
        status: &str,
        updated_at: i64,
    ) {
        if !self.enabled.load(Ordering::Relaxed) || !matches!(status, "FOUND" | "NOT_FOUND") {
            return;
        }
        if let Ok(conn) = self.conn.lock() {
            if let Err(error) = conn.execute(
                "INSERT INTO track_enrichment (track_id, synced_lyrics, plain_lyrics, lyrics_status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO UPDATE SET synced_lyrics = excluded.synced_lyrics,
                     plain_lyrics = excluded.plain_lyrics, lyrics_status = excluded.lyrics_status,
                     updated_at = excluded.updated_at",
                params![track_id, synced, plain, status, updated_at],
            ) {
                eprintln!("[sonata:cache] lyrics-write failed track_id={track_id} error={error}");
            }
        }
    }

    pub fn set_average_bitrate(&self, track_id: &str, bitrate: u32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(conn) = self.conn.lock() {
            if let Err(error) = conn.execute(
                "INSERT INTO track_enrichment (track_id, average_bitrate)
                 VALUES (?1, ?2)
                 ON CONFLICT(track_id) DO UPDATE SET average_bitrate = excluded.average_bitrate",
                params![track_id, bitrate],
            ) {
                eprintln!("[sonata:cache] bitrate-write failed track_id={track_id} error={error}");
            }
        }
    }

    pub fn get_stream(&self, key: &str) -> Option<(String, String)> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let now = now_secs() as i64;
        let guard = self.conn.lock().ok()?;
        guard
            .query_row(
                "SELECT url, badge, expires_at FROM stream_cache WHERE cache_key = ?1",
                params![key],
                |row| {
                    let url: String = row.get(0)?;
                    let badge: String = row.get(1)?;
                    let exp: i64 = row.get(2)?;
                    Ok((url, badge, exp))
                },
            )
            .ok()
            .and_then(|(url, badge, exp)| if exp > now { Some((url, badge)) } else { None })
    }

    pub fn put_stream(&self, key: &str, url: &str, badge: &str) {
        if !self.enabled.load(Ordering::Relaxed)
            || url.to_ascii_lowercase().contains("googlevideo.com")
        {
            return;
        }
        let now = now_secs() as i64;
        let max_ttl = now + 25 * 60;
        let expires_at = extract_url_expire(url)
            .map(|e| (e as i64).saturating_sub(180).min(max_ttl))
            .unwrap_or(max_ttl);
        if expires_at <= now {
            return;
        }
        if let Ok(guard) = self.conn.lock() {
            if let Err(error) = guard.execute(
                "INSERT INTO stream_cache (cache_key, url, badge, expires_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(cache_key) DO UPDATE SET url = excluded.url, badge = excluded.badge, expires_at = excluded.expires_at",
                params![key, url, badge, expires_at],
            ) {
                eprintln!("[sonata:cache] stream-write failed error={error}");
            } else if let Err(error) = guard.execute(
                "DELETE FROM stream_cache WHERE cache_key IN
                 (SELECT cache_key FROM stream_cache ORDER BY expires_at DESC LIMIT -1 OFFSET ?1)",
                params![MAX_CACHED_STREAMS],
            ) {
                eprintln!("[sonata:cache] stream-prune failed error={error}");
            }
        }
    }

    /// Returns `(name, tracks, is_fresh)` if cached on disk.
    pub fn get_playlist(&self, key: &str) -> Option<(String, Vec<Track>, bool)> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let now = now_secs() as i64;
        let guard = self.conn.lock().ok()?;
        let (name, json, updated_at): (String, String, i64) = guard
            .query_row(
                "SELECT name, tracks_json, updated_at FROM playlist_cache WHERE playlist_key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok()?;
        drop(guard);
        let tracks: Vec<Track> = match serde_json::from_str(&json) {
            Ok(tracks) => tracks,
            Err(error) => {
                eprintln!(
                    "[sonata:cache] playlist-discarded key={key} reason=invalid-json error={error}"
                );
                if let Ok(conn) = self.conn.lock() {
                    let _ = conn.execute(
                        "DELETE FROM playlist_cache WHERE playlist_key = ?1",
                        params![key],
                    );
                }
                return None;
            }
        };
        let is_fresh = updated_at >= 0 && now >= updated_at && now - updated_at < 300;
        Some((name, tracks, is_fresh))
    }

    pub fn put_playlist(&self, key: &str, name: &str, tracks: &[Track]) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let Ok(json) = serde_json::to_string(tracks) else {
            return;
        };
        if json.len() > MAX_PLAYLIST_JSON_BYTES {
            eprintln!("[sonata:cache] playlist-write skipped key={key} reason=cache-entry-too-large bytes={}", json.len());
            return;
        }
        let now = now_secs() as i64;
        if let Ok(guard) = self.conn.lock() {
            if let Err(error) = guard.execute(
                "INSERT INTO playlist_cache (playlist_key, name, tracks_json, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(playlist_key) DO UPDATE SET name = excluded.name, tracks_json = excluded.tracks_json, updated_at = excluded.updated_at",
                params![key, name, json, now],
            ) {
                eprintln!("[sonata:cache] playlist-write failed key={key} error={error}");
            } else if let Err(error) = guard.execute(
                "DELETE FROM playlist_cache WHERE playlist_key IN
                 (SELECT playlist_key FROM playlist_cache ORDER BY updated_at DESC LIMIT -1 OFFSET ?1)",
                params![MAX_CACHED_PLAYLISTS],
            ) {
                eprintln!("[sonata:cache] playlist-prune failed error={error}");
            }
        }
    }

    pub fn invalidate_playlist(&self, key: Option<&str>) {
        if let Ok(guard) = self.conn.lock() {
            let result = match key {
                Some(k) => guard.execute(
                    "DELETE FROM playlist_cache WHERE playlist_key = ?1",
                    params![k],
                ),
                None => guard.execute("DELETE FROM playlist_cache", []),
            };
            if let Err(error) = result {
                eprintln!("[sonata:cache] playlist-invalidate failed error={error}");
            }
        }
    }

    pub fn get_library(&self, key: &str) -> Option<Vec<Playlist>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let guard = self.conn.lock().ok()?;
        let json: String = guard
            .query_row(
                "SELECT playlists_json FROM library_cache WHERE library_key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()?;
        match serde_json::from_str(&json) {
            Ok(playlists) => Some(playlists),
            Err(error) => {
                eprintln!(
                    "[sonata:cache] library-discarded key={key} reason=invalid-json error={error}"
                );
                let _ = guard.execute(
                    "DELETE FROM library_cache WHERE library_key = ?1",
                    params![key],
                );
                None
            }
        }
    }

    pub fn put_library(&self, key: &str, playlists: &[Playlist]) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let Ok(json) = serde_json::to_string(playlists) else {
            return;
        };
        if json.len() > MAX_PLAYLIST_JSON_BYTES {
            eprintln!("[sonata:cache] library-write skipped key={key} reason=cache-entry-too-large bytes={}", json.len());
            return;
        }
        let now = now_secs() as i64;
        if let Ok(guard) = self.conn.lock() {
            if let Err(error) = guard.execute(
                "INSERT INTO library_cache (library_key, playlists_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(library_key) DO UPDATE SET playlists_json = excluded.playlists_json, updated_at = excluded.updated_at",
                params![key, json, now],
            ) {
                eprintln!("[sonata:cache] library-write failed key={key} error={error}");
            }
        }
    }
}

fn invalidate_legacy_covers(conn: &Connection) {
    let playlist_rows = conn
        .prepare(
            "SELECT playlist_key, tracks_json FROM playlist_cache
             WHERE lower(tracks_json) LIKE '%saavncdn%' OR lower(tracks_json) LIKE '%jiosaavn%'",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        });
    if let Ok(rows) = playlist_rows {
        for (key, json) in rows {
            let Ok(mut tracks) = serde_json::from_str::<Vec<Track>>(&json) else {
                continue;
            };
            let mut changed = false;
            for track in &mut tracks {
                if track
                    .thumbnail_url
                    .as_deref()
                    .is_some_and(is_legacy_cover_url)
                {
                    track.thumbnail_url = None;
                    changed = true;
                }
            }
            if changed {
                if let Ok(cleaned) = serde_json::to_string(&tracks) {
                    let _ = conn.execute(
                        "UPDATE playlist_cache SET tracks_json = ?1 WHERE playlist_key = ?2",
                        params![cleaned, key],
                    );
                }
            }
        }
    }

    let library_rows = conn
        .prepare(
            "SELECT library_key, playlists_json FROM library_cache
             WHERE lower(playlists_json) LIKE '%saavncdn%' OR lower(playlists_json) LIKE '%jiosaavn%'",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()
        });
    if let Ok(rows) = library_rows {
        for (key, json) in rows {
            let Ok(mut playlists) = serde_json::from_str::<Vec<Playlist>>(&json) else {
                continue;
            };
            let mut changed = false;
            for playlist in &mut playlists {
                if playlist
                    .thumbnail_url
                    .as_deref()
                    .is_some_and(is_legacy_cover_url)
                {
                    playlist.thumbnail_url = None;
                    changed = true;
                }
            }
            if changed {
                if let Ok(cleaned) = serde_json::to_string(&playlists) {
                    let _ = conn.execute(
                        "UPDATE library_cache SET playlists_json = ?1 WHERE library_key = ?2",
                        params![cleaned, key],
                    );
                }
            }
        }
    }
}

fn is_legacy_cover_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("saavncdn") || lower.contains("jiosaavn")
}

struct MediaCacheEntry {
    paths: Vec<PathBuf>,
    bytes: u64,
    last_used: SystemTime,
}

/// Keep shared artwork, audio, and lyric files under the configured byte budget.
/// Metadata remains in the bounded SQLite database; disabling cache removes all media files.
pub fn prune_media_cache(limit_bytes: u64) -> u64 {
    prune_media_cache_inner(limit_bytes, false)
}

/// Enforce a newly selected limit immediately, bypassing the normal sweep interval.
pub fn enforce_media_cache_limit(limit_bytes: u64) -> u64 {
    prune_media_cache_inner(limit_bytes, true)
}

fn prune_media_cache_inner(limit_bytes: u64, force: bool) -> u64 {
    let lock = MEDIA_CACHE_PRUNE_LOCK.get_or_init(|| Mutex::new(()));
    let Ok(_guard) = lock.lock() else {
        eprintln!("[sonata:cache] prune skipped reason=cache-lock-poisoned");
        return 0;
    };
    let last_sweep = LAST_MEDIA_CACHE_PRUNE.get_or_init(|| Mutex::new(None));
    let Ok(mut last_sweep) = last_sweep.lock() else {
        eprintln!("[sonata:cache] prune skipped reason=sweep-clock-poisoned");
        return 0;
    };
    if !force
        && limit_bytes > 0
        && last_sweep.is_some_and(|last| last.elapsed() < MEDIA_CACHE_SWEEP_INTERVAL)
    {
        return 0;
    }
    *last_sweep = Some(Instant::now());

    let mut grouped: HashMap<(String, String), MediaCacheEntry> = HashMap::new();
    for (kind, dir) in [
        ("audio", AppConfig::audio_cache_dir()),
        ("art", AppConfig::art_cache_dir()),
        ("lyrics", AppConfig::lyrics_cache_dir()),
    ] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
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

            let key = (kind.to_string(), group.to_string());
            let cache_entry = grouped.entry(key).or_insert_with(|| MediaCacheEntry {
                paths: Vec::new(),
                bytes: 0,
                last_used: metadata
                    .accessed()
                    .or_else(|_| metadata.modified())
                    .unwrap_or(UNIX_EPOCH),
            });
            cache_entry.bytes = cache_entry.bytes.saturating_add(metadata.len());
            cache_entry.last_used = cache_entry.last_used.min(
                metadata
                    .accessed()
                    .or_else(|_| metadata.modified())
                    .unwrap_or(UNIX_EPOCH),
            );
            cache_entry.paths.push(path.clone());

            if kind == "audio" {
                let badge_path = path.with_extension("badge");
                if let Ok(badge_metadata) = std::fs::symlink_metadata(&badge_path) {
                    if badge_metadata.file_type().is_file() {
                        cache_entry.bytes = cache_entry.bytes.saturating_add(badge_metadata.len());
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
                    let size = metadata.len();
                    removed = removed.saturating_add(size);
                    remaining = remaining.saturating_sub(size);
                }
            }
        }
    }

    if removed > 0 {
        eprintln!(
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn extract_url_expire(url: &str) -> Option<u64> {
    let idx = url.find("expire=")?;
    let rest = &url[idx + "expire=".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
}

pub fn is_stream_url_fresh(url: &str) -> bool {
    if !url.starts_with("http") {
        return std::path::Path::new(url).exists();
    }
    if let Some(exp) = extract_url_expire(url) {
        exp > now_secs().saturating_add(180)
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    #[test]
    fn extracts_youtube_expire_param() {
        let url = "https://rr4.googlevideo.com/videoplayback?expire=1795030319&itag=774";
        assert_eq!(extract_url_expire(url), Some(1795030319));
        assert_eq!(extract_url_expire("https://example.com/test.mp4"), None);
    }

    #[test]
    fn persists_streams_and_playlists_in_sqlite() {
        let tmp = std::env::temp_dir().join(format!("sonata_test_{}.db", std::process::id()));
        let db = CacheDb::open_at(&tmp).expect("open sqlite cache");
        db.put_stream("yt:123", "https://example.com/audio.webm", "OPUS 321kbps");
        let got = db.get_stream("yt:123").expect("cached stream");
        assert_eq!(got.0, "https://example.com/audio.webm");
        assert_eq!(got.1, "OPUS 321kbps");

        let tracks = vec![Track {
            id: "123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: String::new(),
            duration_secs: Some(180),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
        }];
        db.put_playlist("ytm:VLLM", "Liked Music", &tracks);
        let (name, loaded, fresh) = db.get_playlist("ytm:VLLM").expect("cached playlist");
        assert_eq!(name, "Liked Music");
        assert_eq!(loaded.len(), 1);
        assert!(fresh);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn excludes_volatile_googlevideo_streams() {
        let tmp = std::env::temp_dir().join(format!("sonata_volatile_{}.db", std::process::id()));
        let db = CacheDb::open_at(&tmp).expect("open sqlite cache");
        db.put_stream(
            "yt:123",
            "https://rr1.googlevideo.com/videoplayback?expire=1795030319&itag=251",
            "OPUS 263kbps",
        );
        assert!(db.get_stream("yt:123").is_none());
        let _ = std::fs::remove_file(&tmp);
    }
}
