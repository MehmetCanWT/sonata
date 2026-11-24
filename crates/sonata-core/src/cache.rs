//! SQLite-backed metadata cache for Sonata, shared by all front-ends.
//!
//! 1. **Connection & Schema**: `CacheDb` wraps one mutex-guarded `rusqlite` connection, applies schema
//!    versioning, and bounds database size.
//! 2. **Stream Cache**: resolved stream URLs with quality badges, validated against URL expiry
//!    (`extract_url_expire`, `is_stream_url_fresh`).
//! 3. **Playlist & Library Cache**: serialized playlists and library listings with staleness tracking.
//! 4. **Track Enrichment**: per-track cover URLs, synced/plain lyrics, lyrics status, and average bitrate.
//! 5. **Maintenance**: runtime enable/disable (clearing records when disabled) and re-exports of the
//!    on-disk media cache pruning helpers from the `media` submodule.

use crate::paths::CorePaths;
use crate::providers::{Playlist, Track};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod media;

pub use media::{
    enforce_media_cache_limit_async, enforce_media_cache_limit_in, prune_media_cache_async,
    prune_media_cache_in, start_media_cache_maintenance,
};

pub const DEFAULT_CACHE_LIMIT_GB: u8 = 1;
pub const MAX_CACHE_LIMIT_GB: u8 = 10;
pub const GIGABYTE: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_CACHE_LIMIT_BYTES: u64 = (DEFAULT_CACHE_LIMIT_GB as u64) * GIGABYTE;

const CACHE_SCHEMA_VERSION: i64 = 3;
const MAX_CACHE_DB_PAGES: i64 = 32_768;
const MAX_CACHED_STREAMS: i64 = 2_048;
const MAX_CACHED_PLAYLISTS: i64 = 500;
const MAX_PLAYLIST_JSON_BYTES: usize = 16 * 1024 * 1024;
const PRUNE_WRITE_INTERVAL: usize = 32;

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

/// SQLite metadata cache shared by the TUI, desktop, and Android clients.
#[derive(Clone)]
pub struct CacheDb {
    conn: Arc<Mutex<Connection>>,
    enabled: Arc<AtomicBool>,
    write_ops: Arc<AtomicUsize>,
    lyrics_cache_dir: PathBuf,
}

impl CacheDb {
    /// Opens the SQLite metadata cache under explicit `CorePaths`.
    pub fn open_with_paths(paths: &CorePaths, enabled: bool) -> Option<Self> {
        let cache = Self::open_at(&paths.cache_db_path())?;
        cache.set_enabled(enabled);
        Some(cache)
    }

    pub fn open_at(path: &Path) -> Option<Self> {
        let lyrics_cache_dir = path
            .parent()
            .map(|parent| parent.join("lyrics"))
            .unwrap_or_else(|| PathBuf::from("lyrics"));
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                tracing::warn!(
                    "[sonata:cache] create-directory failed path={} error={error}",
                    parent.display()
                );
                return None;
            }
        }
        let conn = match Connection::open(path) {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    "[sonata:cache] open failed path={} error={error}",
                    path.display()
                );
                return None;
            }
        };
        let _ = conn.busy_timeout(Duration::from_secs(5));
        if let Err(error) = conn.pragma_update(None, "max_page_count", MAX_CACHE_DB_PAGES) {
            tracing::warn!("[sonata:cache] database-size-limit failed error={error}");
        }
        let schema_version: i64 =
            match conn.pragma_query_value(None, "user_version", |row| row.get(0)) {
                Ok(version) => version,
                Err(error) => {
                    tracing::warn!("[sonata:cache] schema-version-read failed error={error}");
                    return None;
                }
            };
        if schema_version > CACHE_SCHEMA_VERSION {
            tracing::warn!(
                "[sonata:cache] schema-too-new found={schema_version} supported={CACHE_SCHEMA_VERSION} path={}",
                path.display()
            );
            return None;
        }
        let prev_user_version = schema_version;
        if let Err(error) = conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -4096;
             PRAGMA mmap_size = 8388608;
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
             PRAGMA user_version = 3;
             COMMIT;",
        ) {
            tracing::warn!(
                "[sonata:cache] schema-initialize failed path={} error={error}",
                path.display()
            );
            return None;
        }
        if prev_user_version > 0 && prev_user_version < 3 {
            // One-time invalidation of legacy durationless / pre-lrc.red cached lyrics.
            let _ = conn.execute(
                "UPDATE track_enrichment SET synced_lyrics = NULL, plain_lyrics = NULL, lyrics_status = 'NONE'",
                [],
            );
            let _ = std::fs::remove_dir_all(&lyrics_cache_dir);
        }
        let now = now_secs() as i64;
        if let Err(error) = conn.execute(
            "DELETE FROM stream_cache WHERE expires_at <= ?1",
            params![now],
        ) {
            tracing::warn!("[sonata:cache] stream-expiry-cleanup failed error={error}");
        }
        if let Err(error) = conn.execute(
            "DELETE FROM stream_cache WHERE lower(url) LIKE '%googlevideo.com%'",
            [],
        ) {
            tracing::warn!("[sonata:cache] volatile-stream-cleanup failed error={error}");
        }
        if let Err(error) = conn.execute(
            "UPDATE track_enrichment SET cover_url = NULL, cover_source = 'none'
             WHERE track_id NOT LIKE 'jiosaavn:%'
               AND (lower(COALESCE(cover_url, '')) LIKE '%saavncdn%'
                 OR lower(COALESCE(cover_url, '')) LIKE '%jiosaavn%')",
            [],
        ) {
            tracing::warn!("[sonata:cache] legacy-cover-invalidation failed error={error}");
        }
        invalidate_legacy_covers(&conn);
        Some(Self {
            conn: Arc::new(Mutex::new(conn)),
            enabled: Arc::new(AtomicBool::new(true)),
            write_ops: Arc::new(AtomicUsize::new(0)),
            lyrics_cache_dir,
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

    fn lock_mutex(conn: &Mutex<Connection>) -> std::sync::MutexGuard<'_, Connection> {
        match conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("[sonata:cache] recovering from poisoned database mutex");
                poisoned.into_inner()
            }
        }
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        Self::lock_mutex(&self.conn)
    }

    fn clear_cached_records(conn: &Mutex<Connection>) {
        let conn = Self::lock_mutex(conn);
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
            tracing::warn!("[sonata:cache] clear failed error={error}");
        }
    }

    /// Clears all persisted synced/plain lyrics from SQLite enrichment and on-disk `.lrc`/`.txt` files.
    pub fn clear_lyrics_cache(&self) {
        {
            let guard = self.lock_conn();
            if let Err(error) = guard.execute(
                "UPDATE track_enrichment SET synced_lyrics = NULL, plain_lyrics = NULL, lyrics_status = 'NONE'",
                [],
            ) {
                tracing::warn!("[sonata:cache] lyrics-clear failed error={error}");
            }
        }
        let _ = std::fs::remove_dir_all(&self.lyrics_cache_dir);
    }

    pub fn invalidate_stream(&self, track_id: &str) {
        let guard = self.lock_conn();
        if let Err(error) = guard.execute(
            "DELETE FROM stream_cache WHERE cache_key IN (?1, ?2)",
            params![format!("yt:{track_id}"), format!("hifi:{track_id}")],
        ) {
            tracing::warn!("[sonata:cache] stream-invalidate failed error={error}");
        }
    }

    /// Purges all cached stream URLs (for example on sign-in / sign-out transitions).
    pub fn clear_stream_cache(&self) {
        let guard = self.lock_conn();
        if let Err(error) = guard.execute("DELETE FROM stream_cache", []) {
            tracing::warn!("[sonata:cache] stream-clear failed error={error}");
        }
    }

    pub fn get_track_enrichment(&self, track_id: &str) -> Option<TrackEnrichment> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let guard = self.lock_conn();
        let mut stmt = guard
            .prepare_cached(
                "SELECT cover_url, cover_source, synced_lyrics, plain_lyrics, lyrics_status, average_bitrate, updated_at
                 FROM track_enrichment WHERE track_id = ?1",
            )
            .ok()?;
        stmt.query_row(params![track_id], |row| {
            Ok(TrackEnrichment {
                cover_url: row.get(0)?,
                cover_source: row.get(1)?,
                synced_lyrics: row.get(2)?,
                plain_lyrics: row.get(3)?,
                lyrics_status: row.get(4)?,
                average_bitrate: row.get::<_, i64>(5)?.max(0) as u32,
                updated_at: row.get(6)?,
            })
        })
        .ok()
    }

    pub fn set_track_cover(&self, track_id: &str, url: &str, source: &str) {
        if !self.enabled.load(Ordering::Relaxed)
            || !matches!(source, "deezer" | "apple" | "ytm" | "spotify")
        {
            return;
        }
        let conn = self.lock_conn();
        if let Err(error) = conn
            .prepare_cached(
                "INSERT INTO track_enrichment (track_id, cover_url, cover_source)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_id) DO UPDATE SET cover_url = excluded.cover_url, cover_source = excluded.cover_source",
            )
            .and_then(|mut stmt| stmt.execute(params![track_id, url, source]))
        {
            tracing::warn!("[sonata:cache] cover-write failed track_id={track_id} error={error}");
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
        let conn = self.lock_conn();
        if let Err(error) = conn
            .prepare_cached(
                "INSERT INTO track_enrichment (track_id, synced_lyrics, plain_lyrics, lyrics_status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(track_id) DO UPDATE SET synced_lyrics = excluded.synced_lyrics,
                     plain_lyrics = excluded.plain_lyrics, lyrics_status = excluded.lyrics_status,
                     updated_at = excluded.updated_at",
            )
            .and_then(|mut stmt| stmt.execute(params![track_id, synced, plain, status, updated_at]))
        {
            tracing::warn!("[sonata:cache] lyrics-write failed track_id={track_id} error={error}");
        }
    }

    pub fn set_average_bitrate(&self, track_id: &str, bitrate: u32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let conn = self.lock_conn();
        if let Err(error) = conn
            .prepare_cached(
                "INSERT INTO track_enrichment (track_id, average_bitrate)
                 VALUES (?1, ?2)
                 ON CONFLICT(track_id) DO UPDATE SET average_bitrate = excluded.average_bitrate",
            )
            .and_then(|mut stmt| stmt.execute(params![track_id, bitrate]))
        {
            tracing::warn!("[sonata:cache] bitrate-write failed track_id={track_id} error={error}");
        }
    }

    pub fn get_stream(&self, key: &str) -> Option<(String, String)> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let now = now_secs() as i64;
        let guard = self.lock_conn();
        let mut stmt = guard
            .prepare_cached("SELECT url, badge, expires_at FROM stream_cache WHERE cache_key = ?1")
            .ok()?;
        stmt.query_row(params![key], |row| {
            let url: String = row.get(0)?;
            let badge: String = row.get(1)?;
            let exp: i64 = row.get(2)?;
            Ok((url, badge, exp))
        })
        .ok()
        .and_then(|(url, badge, exp)| if exp > now { Some((url, badge)) } else { None })
    }

    pub fn put_stream(&self, key: &str, url: &str, badge: &str) {
        if !self.enabled.load(Ordering::Relaxed)
            || contains_ignore_ascii_case(url, b"googlevideo.com")
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
        let guard = self.lock_conn();
        if let Err(error) = guard
            .prepare_cached(
                "INSERT INTO stream_cache (cache_key, url, badge, expires_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(cache_key) DO UPDATE SET url = excluded.url, badge = excluded.badge, expires_at = excluded.expires_at",
            )
            .and_then(|mut stmt| stmt.execute(params![key, url, badge, expires_at]))
        {
            tracing::warn!("[sonata:cache] stream-write failed error={error}");
        } else if self
            .write_ops
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(PRUNE_WRITE_INTERVAL)
        {
            if let Err(error) = guard
                .prepare_cached(
                    "DELETE FROM stream_cache WHERE cache_key IN
                     (SELECT cache_key FROM stream_cache ORDER BY expires_at DESC LIMIT -1 OFFSET ?1)",
                )
                .and_then(|mut stmt| stmt.execute(params![MAX_CACHED_STREAMS]))
            {
                tracing::warn!("[sonata:cache] stream-prune failed error={error}");
            }
        }
    }

    /// Returns `(name, tracks, is_fresh)` if cached on disk.
    pub fn get_playlist(&self, key: &str) -> Option<(String, Vec<Track>, bool)> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let now = now_secs() as i64;
        let guard = self.lock_conn();
        let (name, json, updated_at): (String, String, i64) = guard
            .prepare_cached(
                "SELECT name, tracks_json, updated_at FROM playlist_cache WHERE playlist_key = ?1",
            )
            .ok()?
            .query_row(params![key], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .ok()?;
        drop(guard);
        let tracks: Vec<Track> = match serde_json::from_str(&json) {
            Ok(tracks) => tracks,
            Err(error) => {
                tracing::warn!(
                    "[sonata:cache] playlist-discarded key={key} reason=invalid-json error={error}"
                );
                let conn = self.lock_conn();
                let _ = conn.execute(
                    "DELETE FROM playlist_cache WHERE playlist_key = ?1",
                    params![key],
                );
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
            tracing::warn!("[sonata:cache] playlist-write skipped key={key} reason=cache-entry-too-large bytes={}", json.len());
            return;
        }
        let now = now_secs() as i64;
        let guard = self.lock_conn();
        if let Err(error) = guard
            .prepare_cached(
                "INSERT INTO playlist_cache (playlist_key, name, tracks_json, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(playlist_key) DO UPDATE SET name = excluded.name, tracks_json = excluded.tracks_json, updated_at = excluded.updated_at",
            )
            .and_then(|mut stmt| stmt.execute(params![key, name, json, now]))
        {
            tracing::warn!("[sonata:cache] playlist-write failed key={key} error={error}");
        } else if self
            .write_ops
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(PRUNE_WRITE_INTERVAL)
        {
            if let Err(error) = guard
                .prepare_cached(
                    "DELETE FROM playlist_cache WHERE playlist_key IN
                     (SELECT playlist_key FROM playlist_cache ORDER BY updated_at DESC LIMIT -1 OFFSET ?1)",
                )
                .and_then(|mut stmt| stmt.execute(params![MAX_CACHED_PLAYLISTS]))
            {
                tracing::warn!("[sonata:cache] playlist-prune failed error={error}");
            }
        }
    }

    pub fn invalidate_playlist(&self, key: Option<&str>) {
        let guard = self.lock_conn();
        let result = match key {
            Some(k) => guard.execute(
                "DELETE FROM playlist_cache WHERE playlist_key = ?1",
                params![k],
            ),
            None => guard.execute("DELETE FROM playlist_cache", []),
        };
        if let Err(error) = result {
            tracing::warn!("[sonata:cache] playlist-invalidate failed error={error}");
        }
    }

    pub fn get_library(&self, key: &str) -> Option<Vec<Playlist>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let guard = self.lock_conn();
        let json: String = guard
            .prepare_cached("SELECT playlists_json FROM library_cache WHERE library_key = ?1")
            .ok()?
            .query_row(params![key], |row| row.get(0))
            .ok()?;
        match serde_json::from_str(&json) {
            Ok(playlists) => Some(playlists),
            Err(error) => {
                tracing::warn!(
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
            tracing::warn!("[sonata:cache] library-write skipped key={key} reason=cache-entry-too-large bytes={}", json.len());
            return;
        }
        let now = now_secs() as i64;
        let guard = self.lock_conn();
        if let Err(error) = guard
            .prepare_cached(
                "INSERT INTO library_cache (library_key, playlists_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(library_key) DO UPDATE SET playlists_json = excluded.playlists_json, updated_at = excluded.updated_at",
            )
            .and_then(|mut stmt| stmt.execute(params![key, json, now]))
        {
            tracing::warn!("[sonata:cache] library-write failed key={key} error={error}");
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
                if !track.is_jiosaavn()
                    && track
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

fn contains_ignore_ascii_case(haystack: &str, needle: &[u8]) -> bool {
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn is_legacy_cover_url(url: &str) -> bool {
    contains_ignore_ascii_case(url, b"saavncdn") || contains_ignore_ascii_case(url, b"jiosaavn")
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
    let len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    rest.get(..len)?.parse::<u64>().ok()
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
    use std::sync::atomic::AtomicU64;

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_db_path(prefix: &str) -> PathBuf {
        let seq = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("sonata_{prefix}_{}_{}.db", std::process::id(), seq))
    }

    fn cleanup_test_db(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn extracts_youtube_expire_param() {
        let url = "https://rr4.googlevideo.com/videoplayback?expire=1795030319&itag=774";
        assert_eq!(extract_url_expire(url), Some(1795030319));
        assert_eq!(extract_url_expire("https://example.com/test.mp4"), None);
    }

    #[test]
    fn persists_streams_and_playlists_in_sqlite() {
        let tmp = unique_test_db_path("test");
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
            album_id: None,
            is_explicit: false,
        }];
        db.put_playlist("ytm:VLLM", "Liked Music", &tracks);
        let (name, loaded, fresh) = db.get_playlist("ytm:VLLM").expect("cached playlist");
        assert_eq!(name, "Liked Music");
        assert_eq!(loaded.len(), 1);
        assert!(fresh);

        cleanup_test_db(&tmp);
    }

    #[test]
    fn excludes_volatile_googlevideo_streams() {
        let tmp = unique_test_db_path("volatile");
        let db = CacheDb::open_at(&tmp).expect("open sqlite cache");
        db.put_stream(
            "yt:123",
            "https://rr1.googlevideo.com/videoplayback?expire=1795030319&itag=251",
            "OPUS 263kbps",
        );
        assert!(db.get_stream("yt:123").is_none());
        cleanup_test_db(&tmp);
    }

    #[test]
    fn reopens_existing_v3_database() {
        let tmp = unique_test_db_path("reopen_v3");
        {
            let db = CacheDb::open_at(&tmp).expect("initial open");
            db.put_stream("yt:abc", "https://example.com/a.m4a", "AAC 256kbps");
        }
        let reopened = CacheDb::open_at(&tmp).expect("reopen v3 cache");
        let got = reopened.get_stream("yt:abc").expect("persisted stream");
        assert_eq!(got.0, "https://example.com/a.m4a");
        assert_eq!(got.1, "AAC 256kbps");
        cleanup_test_db(&tmp);
    }
}
