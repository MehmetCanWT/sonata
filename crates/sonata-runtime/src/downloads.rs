//! Shared offline audio downloads manifest and file storage for Linux Sonata clients.
//!
//! 1. **Storage & Permissions** (`DownloadStore`, `default_downloads_dir`): Persists downloaded
//!    audio and optional artwork in the shared XDG data directory (`~/.local/share/sonata/downloads`
//!    or `AppConfig::downloads_dir`) with `0o700` private directories and atomic `.tmp` -> rename writes.
//! 2. **SQLite WAL Manifest** (`downloads.db`): Tracks downloaded tracks, quality metadata
//!    (`audio_badge`, `bitrate_kbps`, `container`), artwork references, and collection membership.
//! 3. **Retention & Lookup**: Provides track and collection queries, explicit deletion, oldest/LRU
//!    size-limit pruning, and `spawn_blocking` async wrappers for UI and `LinuxStreamResolver` paths.

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sonata_core::auth::ensure_private_dir;
use sonata_core::providers::hifi::{extract_badge_kbps, ResolvedStream};
use sonata_core::providers::{ProviderSource, Track};
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DOWNLOADS_DB_FILENAME: &str = "downloads.db";
const DOWNLOADS_SCHEMA_VERSION: i64 = 1;
const AUDIO_SUBDIR: &str = "audio";
const ARTWORK_SUBDIR: &str = "artwork";

/// Returns the default shared XDG offline downloads directory (`$XDG_DATA_HOME/sonata/downloads`).
pub fn default_downloads_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("sonata")
        .join("downloads")
}

/// Ensures the downloads root and its `audio`/`artwork` subdirectories exist with `0o700` permissions.
pub fn ensure_downloads_dir(root: &Path) -> Result<()> {
    ensure_private_dir(root)
        .with_context(|| format!("failed to create downloads root {}", root.display()))?;
    ensure_private_dir(&root.join(AUDIO_SUBDIR))
        .with_context(|| format!("failed to create audio downloads dir in {}", root.display()))?;
    ensure_private_dir(&root.join(ARTWORK_SUBDIR)).with_context(|| {
        format!(
            "failed to create artwork downloads dir in {}",
            root.display()
        )
    })?;
    Ok(())
}

struct TempFileGuard {
    path: PathBuf,
    persisted: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            persisted: false,
        }
    }

    fn mark_persisted(&mut self) {
        self.persisted = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.persisted {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Writes `bytes` to `target_path` atomically using a unique `.tmp` staging file and `rename`.
pub fn write_atomic_file(target_path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target_path
        .parent()
        .ok_or_else(|| anyhow!("target path has no parent: {}", target_path.display()))?;
    ensure_private_dir(parent)?;

    let tmp_path = unique_tmp_path(target_path);
    let mut guard = TempFileGuard::new(tmp_path.clone());

    let mut file = open_private_new_file(&tmp_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    fs::rename(&tmp_path, target_path).with_context(|| {
        format!(
            "failed to rename {} -> {}",
            tmp_path.display(),
            target_path.display()
        )
    })?;
    guard.mark_persisted();
    Ok(())
}

/// Copies `source_path` into `target_path` atomically using a unique `.tmp` staging file and `rename`.
pub fn copy_atomic_file(source_path: &Path, target_path: &Path) -> Result<u64> {
    let parent = target_path
        .parent()
        .ok_or_else(|| anyhow!("target path has no parent: {}", target_path.display()))?;
    ensure_private_dir(parent)?;

    let tmp_path = unique_tmp_path(target_path);
    let mut guard = TempFileGuard::new(tmp_path.clone());

    let mut src = fs::File::open(source_path)
        .with_context(|| format!("failed to open source file {}", source_path.display()))?;
    let mut dst = open_private_new_file(&tmp_path)?;
    let copied = std::io::copy(&mut src, &mut dst)?;
    dst.sync_all()?;
    drop(dst);

    fs::rename(&tmp_path, target_path).with_context(|| {
        format!(
            "failed to rename {} -> {}",
            tmp_path.display(),
            target_path.display()
        )
    })?;
    guard.mark_persisted();
    Ok(copied)
}

/// Removes leftover `.tmp` files inside `audio/` and `artwork/` subdirectories.
pub fn cleanup_stale_tmp_files(root: &Path) -> usize {
    let mut removed = 0;
    for subdir in [AUDIO_SUBDIR, ARTWORK_SUBDIR] {
        let dir = root.join(subdir);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_tmp = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tmp"));
            if is_tmp && path.is_file() && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

fn open_private_new_file(path: &Path) -> Result<fs::File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);
    opts.open(path)
        .with_context(|| format!("failed to create temp file {}", path.display()))
}

fn unique_tmp_path(target_path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    target_path.with_file_name(format!("{file_name}.{}.{nanos}.tmp", std::process::id()))
}

fn sanitize_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
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

fn sanitize_container(container: &str) -> String {
    let cleaned: String = container
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    if cleaned.is_empty() {
        "audio".to_string()
    } else {
        cleaned
    }
}

fn detect_artwork_extension(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "img"
    }
}

fn source_to_str(source: ProviderSource) -> &'static str {
    match source {
        ProviderSource::YouTubeMusic => "ytm",
        ProviderSource::Spotify => "spotify",
    }
}

fn source_from_str(raw: &str) -> ProviderSource {
    match raw {
        "spotify" => ProviderSource::Spotify,
        _ => ProviderSource::YouTubeMusic,
    }
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Optional collection association when persisting a downloaded track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadCollectionRef {
    pub collection_id: String,
    pub title: String,
    pub source: ProviderSource,
    pub artwork_url: Option<String>,
    pub position: usize,
}

/// Metadata required to save a downloaded audio track into the shared offline manifest.
#[derive(Debug, Clone)]
pub struct DownloadTrackInput {
    pub track: Track,
    pub audio_badge: String,
    pub bitrate_kbps: Option<u32>,
    pub container: String,
    pub artwork_url: Option<String>,
    pub collection: Option<DownloadCollectionRef>,
}

/// Persisted offline track entry with resolved local file paths and quality metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadedTrack {
    pub track_id: String,
    pub source: ProviderSource,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
    pub audio_path: PathBuf,
    pub relative_path: String,
    pub file_size_bytes: u64,
    pub audio_badge: String,
    pub bitrate_kbps: Option<u32>,
    pub container: String,
    pub artwork_url: Option<String>,
    pub artwork_path: Option<PathBuf>,
    pub collection_id: Option<String>,
    pub collection_title: Option<String>,
    pub downloaded_at: i64,
    pub last_played_at: i64,
}

impl DownloadedTrack {
    /// Converts this downloaded record into a playable [`Track`] model.
    pub fn to_track(&self) -> Track {
        Track {
            id: self.track_id.clone(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration_secs: self.duration_secs,
            thumbnail_url: self.artwork_url.clone(),
            source: self.source,
            audio_badge: Some(self.audio_badge.clone()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        }
    }

    /// Converts this downloaded record into a local [`ResolvedStream`] for `mpv`.
    pub fn to_resolved_stream(&self) -> ResolvedStream {
        ResolvedStream {
            url: self.audio_path.to_string_lossy().into_owned(),
            badge: self.audio_badge.clone(),
            headers: Vec::new(),
        }
    }
}

/// Aggregated metadata for a collection (playlist or album) with offline downloaded tracks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadedCollection {
    pub collection_id: String,
    pub title: String,
    pub source: ProviderSource,
    pub artwork_url: Option<String>,
    pub track_count: usize,
    pub total_bytes: u64,
    pub updated_at: i64,
}

/// Summary returned after pruning offline downloads to a byte budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PruneReport {
    pub removed_tracks: usize,
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
}

/// Thread-safe handle to the shared SQLite WAL offline downloads manifest and file store.
#[derive(Clone)]
pub struct DownloadStore {
    root: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl DownloadStore {
    /// Opens or initializes the shared offline downloads manifest under `root` (`0o700` dirs).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        ensure_downloads_dir(&root)?;
        Self::open_db_only(root)
    }

    /// Opens an existing `downloads.db` under `root` without creating directories or files when absent.
    pub fn open_existing(root: impl Into<PathBuf>) -> Result<Option<Self>> {
        let root = root.into();
        let db_path = root.join(DOWNLOADS_DB_FILENAME);
        if !db_path.is_file() {
            return Ok(None);
        }
        Self::open_db_only(root).map(Some)
    }

    /// Async counterpart to [`Self::open`] executed on `tokio::task::spawn_blocking`.
    pub async fn open_async(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        tokio::task::spawn_blocking(move || Self::open(root))
            .await
            .context("download store open task panicked")?
    }

    /// Async counterpart to [`Self::open_existing`] executed on `tokio::task::spawn_blocking`.
    pub async fn open_existing_async(root: impl Into<PathBuf>) -> Result<Option<Self>> {
        let root = root.into();
        tokio::task::spawn_blocking(move || Self::open_existing(root))
            .await
            .context("download store open_existing task panicked")?
    }

    fn open_db_only(root: PathBuf) -> Result<Self> {
        let db_path = root.join(DOWNLOADS_DB_FILENAME);
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open downloads manifest {}", db_path.display()))?;
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let schema_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("failed to read downloads manifest schema version")?;
        if schema_version > DOWNLOADS_SCHEMA_VERSION {
            return Err(anyhow!(
                "downloads manifest schema version {schema_version} exceeds supported {DOWNLOADS_SCHEMA_VERSION}"
            ));
        }
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -2048;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 16777216;
             BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS downloaded_tracks (
                 track_id TEXT PRIMARY KEY,
                 source TEXT NOT NULL,
                 title TEXT NOT NULL,
                 artist TEXT NOT NULL,
                 album TEXT NOT NULL,
                 duration_secs INTEGER,
                 relative_path TEXT NOT NULL,
                 file_size_bytes INTEGER NOT NULL,
                 audio_badge TEXT NOT NULL,
                 bitrate_kbps INTEGER,
                 container TEXT NOT NULL,
                 artwork_url TEXT,
                 artwork_relative_path TEXT,
                 collection_id TEXT,
                 collection_title TEXT,
                 downloaded_at INTEGER NOT NULL,
                 last_played_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS downloaded_collections (
                 collection_id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 source TEXT NOT NULL,
                 artwork_url TEXT,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS collection_tracks (
                 collection_id TEXT NOT NULL,
                 track_id TEXT NOT NULL,
                 position INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (collection_id, track_id),
                 FOREIGN KEY (collection_id) REFERENCES downloaded_collections(collection_id) ON DELETE CASCADE,
                 FOREIGN KEY (track_id) REFERENCES downloaded_tracks(track_id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS downloaded_tracks_lru_idx
                 ON downloaded_tracks(last_played_at, downloaded_at);
             CREATE INDEX IF NOT EXISTS collection_tracks_track_idx
                 ON collection_tracks(track_id);
             PRAGMA user_version = 1;
             COMMIT;",
        )
        .with_context(|| format!("failed to initialize downloads schema at {}", db_path.display()))?;

        Ok(Self {
            root,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Returns the downloads root directory path for this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("[sonata:downloads] recovering from poisoned manifest mutex");
                poisoned.into_inner()
            }
        }
    }

    /// Writes `audio_bytes` (and optional `artwork_bytes`) atomically and records the track in `downloads.db`.
    pub fn save_track_bytes(
        &self,
        input: &DownloadTrackInput,
        audio_bytes: &[u8],
        artwork_bytes: Option<&[u8]>,
    ) -> Result<DownloadedTrack> {
        if audio_bytes.is_empty() {
            return Err(anyhow!("cannot save empty audio payload"));
        }
        self.save_track_with_writer(input, artwork_bytes, |target| {
            write_atomic_file(target, audio_bytes)?;
            Ok(audio_bytes.len() as u64)
        })
    }

    /// Async counterpart to [`Self::save_track_bytes`] executed on `tokio::task::spawn_blocking`.
    pub async fn save_track_bytes_async(
        &self,
        input: DownloadTrackInput,
        audio_bytes: Vec<u8>,
        artwork_bytes: Option<Vec<u8>>,
    ) -> Result<DownloadedTrack> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.save_track_bytes(&input, &audio_bytes, artwork_bytes.as_deref())
        })
        .await
        .context("download save_track_bytes task panicked")?
    }

    /// Copies an existing downloaded file atomically into the store and records it in `downloads.db`.
    pub fn save_track_from_file(
        &self,
        input: &DownloadTrackInput,
        source_audio_path: &Path,
        artwork_bytes: Option<&[u8]>,
    ) -> Result<DownloadedTrack> {
        let meta = fs::metadata(source_audio_path).with_context(|| {
            format!("missing source audio file {}", source_audio_path.display())
        })?;
        if meta.len() == 0 {
            return Err(anyhow!(
                "source audio file is empty: {}",
                source_audio_path.display()
            ));
        }
        self.save_track_with_writer(input, artwork_bytes, |target| {
            copy_atomic_file(source_audio_path, target)
        })
    }

    /// Async counterpart to [`Self::save_track_from_file`] executed on `tokio::task::spawn_blocking`.
    pub async fn save_track_from_file_async(
        &self,
        input: DownloadTrackInput,
        source_audio_path: PathBuf,
        artwork_bytes: Option<Vec<u8>>,
    ) -> Result<DownloadedTrack> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.save_track_from_file(&input, &source_audio_path, artwork_bytes.as_deref())
        })
        .await
        .context("download save_track_from_file task panicked")?
    }

    fn save_track_with_writer<F>(
        &self,
        input: &DownloadTrackInput,
        artwork_bytes: Option<&[u8]>,
        write_audio: F,
    ) -> Result<DownloadedTrack>
    where
        F: FnOnce(&Path) -> Result<u64>,
    {
        if input.track.id.trim().is_empty() {
            return Err(anyhow!("cannot save downloaded track with empty track id"));
        }
        ensure_downloads_dir(&self.root)?;

        let safe_id = sanitize_id(&input.track.id);
        let container = sanitize_container(&input.container);
        let audio_rel = format!("{AUDIO_SUBDIR}/{safe_id}.{container}");
        let audio_path = self.root.join(&audio_rel);
        let file_size_bytes = write_audio(&audio_path)?;

        let mut artwork_rel: Option<String> = None;
        let mut artwork_path: Option<PathBuf> = None;
        if let Some(bytes) = artwork_bytes.filter(|bytes| !bytes.is_empty()) {
            let ext = detect_artwork_extension(bytes);
            let rel = format!("{ARTWORK_SUBDIR}/{safe_id}.{ext}");
            let full = self.root.join(&rel);
            write_atomic_file(&full, bytes)?;
            artwork_rel = Some(rel);
            artwork_path = Some(full);
        }

        let artwork_url = input
            .artwork_url
            .clone()
            .or_else(|| input.track.thumbnail_url.clone())
            .filter(|url| !url.trim().is_empty());
        let bitrate_kbps = input
            .bitrate_kbps
            .or_else(|| extract_badge_kbps(&input.audio_badge));
        let now = now_unix_secs();

        let (prev_audio_rel, prev_art_rel): (Option<String>, Option<String>) = {
            let conn = self.lock_conn();
            let prev: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT relative_path, artwork_relative_path FROM downloaded_tracks WHERE track_id = ?1",
                    params![&input.track.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            let db_result: Result<()> = (|| {
                let tx = conn.unchecked_transaction()?;
                let existing_col: Option<(Option<String>, Option<String>)> = tx
                    .query_row(
                        "SELECT collection_id, collection_title FROM downloaded_tracks WHERE track_id = ?1",
                        params![&input.track.id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;

                let (col_id, col_title) = match &input.collection {
                    Some(col) => (Some(col.collection_id.clone()), Some(col.title.clone())),
                    None => existing_col.unwrap_or((None, None)),
                };

                let effective_art_rel = artwork_rel
                    .as_deref()
                    .or_else(|| prev.as_ref().and_then(|(_, old_art)| old_art.as_deref()));

                tx.execute(
                    "INSERT INTO downloaded_tracks (
                         track_id, source, title, artist, album, duration_secs,
                         relative_path, file_size_bytes, audio_badge, bitrate_kbps,
                         container, artwork_url, artwork_relative_path,
                         collection_id, collection_title, downloaded_at, last_played_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                     ON CONFLICT(track_id) DO UPDATE SET
                         source = excluded.source,
                         title = excluded.title,
                         artist = excluded.artist,
                         album = excluded.album,
                         duration_secs = excluded.duration_secs,
                         relative_path = excluded.relative_path,
                         file_size_bytes = excluded.file_size_bytes,
                         audio_badge = excluded.audio_badge,
                         bitrate_kbps = excluded.bitrate_kbps,
                         container = excluded.container,
                         artwork_url = COALESCE(excluded.artwork_url, downloaded_tracks.artwork_url),
                         artwork_relative_path = COALESCE(excluded.artwork_relative_path, downloaded_tracks.artwork_relative_path),
                         collection_id = COALESCE(excluded.collection_id, downloaded_tracks.collection_id),
                         collection_title = COALESCE(excluded.collection_title, downloaded_tracks.collection_title),
                         downloaded_at = excluded.downloaded_at,
                         last_played_at = excluded.last_played_at",
                    params![
                        &input.track.id,
                        source_to_str(input.track.source),
                        &input.track.title,
                        &input.track.artist,
                        &input.track.album,
                        input.track.duration_secs.map(|secs| secs as i64),
                        &audio_rel,
                        file_size_bytes as i64,
                        &input.audio_badge,
                        bitrate_kbps.map(i64::from),
                        &container,
                        artwork_url.as_deref(),
                        effective_art_rel,
                        col_id.as_deref(),
                        col_title.as_deref(),
                        now,
                        now,
                    ],
                )?;

                if let Some(col) = &input.collection {
                    tx.execute(
                        "INSERT INTO downloaded_collections (collection_id, title, source, artwork_url, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT(collection_id) DO UPDATE SET
                             title = excluded.title,
                             source = excluded.source,
                             artwork_url = COALESCE(excluded.artwork_url, downloaded_collections.artwork_url),
                             updated_at = excluded.updated_at",
                        params![
                            &col.collection_id,
                            &col.title,
                            source_to_str(col.source),
                            col.artwork_url.as_deref(),
                            now,
                        ],
                    )?;
                    tx.execute(
                        "INSERT INTO collection_tracks (collection_id, track_id, position)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT(collection_id, track_id) DO UPDATE SET
                             position = excluded.position",
                        params![&col.collection_id, &input.track.id, col.position as i64],
                    )?;
                }

                tx.commit()?;
                Ok(())
            })();

            if let Err(err) = db_result {
                let _ = fs::remove_file(&audio_path);
                if let Some(art) = &artwork_path {
                    let _ = fs::remove_file(art);
                }
                return Err(err);
            }

            match prev {
                Some((old_audio, old_art)) => (Some(old_audio), old_art),
                None => (None, None),
            }
        };

        if let Some(old_audio) = prev_audio_rel.filter(|old| old != &audio_rel) {
            let _ = fs::remove_file(self.root.join(old_audio));
        }
        if let (Some(new_art), Some(old_art)) = (&artwork_rel, &prev_art_rel) {
            if new_art != old_art {
                let _ = fs::remove_file(self.root.join(old_art));
            }
        }

        self.get_track(&input.track.id)?
            .ok_or_else(|| anyhow!("saved download disappeared immediately: {}", input.track.id))
    }

    /// Associates an already-downloaded track with a collection without rewriting the audio file.
    pub fn attach_track_to_collection(
        &self,
        track_id: &str,
        collection: &DownloadCollectionRef,
    ) -> Result<()> {
        let now = now_unix_secs();
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO downloaded_collections (collection_id, title, source, artwork_url, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(collection_id) DO UPDATE SET
                 title = excluded.title,
                 source = excluded.source,
                 artwork_url = COALESCE(excluded.artwork_url, downloaded_collections.artwork_url),
                 updated_at = excluded.updated_at",
            params![
                &collection.collection_id,
                &collection.title,
                source_to_str(collection.source),
                collection.artwork_url.as_deref(),
                now,
            ],
        )?;
        tx.execute(
            "INSERT INTO collection_tracks (collection_id, track_id, position)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(collection_id, track_id) DO UPDATE SET
                 position = excluded.position",
            params![
                &collection.collection_id,
                track_id,
                collection.position as i64
            ],
        )?;
        tx.execute(
            "UPDATE downloaded_tracks
             SET collection_id = COALESCE(collection_id, ?2),
                 collection_title = COALESCE(collection_title, ?3)
             WHERE track_id = ?1",
            params![track_id, &collection.collection_id, &collection.title],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Async counterpart to [`Self::attach_track_to_collection`] executed on `spawn_blocking`.
    pub async fn attach_track_to_collection_async(
        &self,
        track_id: String,
        collection: DownloadCollectionRef,
    ) -> Result<()> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.attach_track_to_collection(&track_id, &collection)
        })
        .await
        .context("download attach_track_to_collection task panicked")?
    }

    /// Looks up a downloaded track by `track_id`, returning `None` if missing or absent on disk.
    pub fn get_track(&self, track_id: &str) -> Result<Option<DownloadedTrack>> {
        let conn = self.lock_conn();
        let row = conn
            .query_row(
                "SELECT track_id, source, title, artist, album, duration_secs,
                        relative_path, file_size_bytes, audio_badge, bitrate_kbps,
                        container, artwork_url, artwork_relative_path,
                        collection_id, collection_title, downloaded_at, last_played_at
                 FROM downloaded_tracks
                 WHERE track_id = ?1",
                params![track_id],
                |row| self.row_to_downloaded_track(row),
            )
            .optional()?;
        Ok(row.filter(|entry| is_valid_audio_file(&entry.audio_path)))
    }

    /// Async counterpart to [`Self::get_track`] executed on `tokio::task::spawn_blocking`.
    pub async fn get_track_async(&self, track_id: String) -> Result<Option<DownloadedTrack>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.get_track(&track_id))
            .await
            .context("download get_track task panicked")?
    }

    /// Resolves a playable local [`ResolvedStream`] for `track_id` and bumps `last_played_at` for LRU pruning.
    pub fn resolve_offline_stream(&self, track_id: &str) -> Result<Option<ResolvedStream>> {
        let Some(track) = self.get_track(track_id)? else {
            return Ok(None);
        };
        let now = now_unix_secs();
        let conn = self.lock_conn();
        let _ = conn.execute(
            "UPDATE downloaded_tracks SET last_played_at = ?2 WHERE track_id = ?1",
            params![track_id, now],
        );
        Ok(Some(track.to_resolved_stream()))
    }

    /// Async counterpart to [`Self::resolve_offline_stream`] executed on `tokio::task::spawn_blocking`.
    pub async fn resolve_offline_stream_async(
        &self,
        track_id: String,
    ) -> Result<Option<ResolvedStream>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.resolve_offline_stream(&track_id))
            .await
            .context("download resolve_offline_stream task panicked")?
    }

    /// Lists all downloaded tracks present on disk, ordered newest-first.
    pub fn list_tracks(&self) -> Result<Vec<DownloadedTrack>> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT track_id, source, title, artist, album, duration_secs,
                    relative_path, file_size_bytes, audio_badge, bitrate_kbps,
                    container, artwork_url, artwork_relative_path,
                    collection_id, collection_title, downloaded_at, last_played_at
             FROM downloaded_tracks
             ORDER BY downloaded_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| self.row_to_downloaded_track(row))?;
        let mut tracks = Vec::new();
        for row in rows {
            let track = row?;
            if is_valid_audio_file(&track.audio_path) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    /// Async counterpart to [`Self::list_tracks`] executed on `tokio::task::spawn_blocking`.
    pub async fn list_tracks_async(&self) -> Result<Vec<DownloadedTrack>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_tracks())
            .await
            .context("download list_tracks task panicked")?
    }

    /// Lists all downloaded collections (playlists/albums) with track counts and byte totals.
    pub fn list_collections(&self) -> Result<Vec<DownloadedCollection>> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT c.collection_id, c.title, c.source, c.artwork_url, c.updated_at,
                    COUNT(t.track_id) AS track_count,
                    COALESCE(SUM(t.file_size_bytes), 0) AS total_bytes
             FROM downloaded_collections c
             INNER JOIN collection_tracks ct ON ct.collection_id = c.collection_id
             INNER JOIN downloaded_tracks t ON t.track_id = ct.track_id
             GROUP BY c.collection_id
             ORDER BY c.updated_at DESC, c.rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let source_raw: String = row.get(2)?;
            let track_count: i64 = row.get(5)?;
            let total_bytes: i64 = row.get(6)?;
            Ok(DownloadedCollection {
                collection_id: row.get(0)?,
                title: row.get(1)?,
                source: source_from_str(&source_raw),
                artwork_url: row.get(3)?,
                track_count: track_count.max(0) as usize,
                total_bytes: total_bytes.max(0) as u64,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Async counterpart to [`Self::list_collections`] executed on `tokio::task::spawn_blocking`.
    pub async fn list_collections_async(&self) -> Result<Vec<DownloadedCollection>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_collections())
            .await
            .context("download list_collections task panicked")?
    }

    /// Lists all downloaded tracks belonging to `collection_id` in collection order.
    pub fn list_collection_tracks(&self, collection_id: &str) -> Result<Vec<DownloadedTrack>> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT t.track_id, t.source, t.title, t.artist, t.album, t.duration_secs,
                    t.relative_path, t.file_size_bytes, t.audio_badge, t.bitrate_kbps,
                    t.container, t.artwork_url, t.artwork_relative_path,
                    c.collection_id, c.title, t.downloaded_at, t.last_played_at
             FROM collection_tracks ct
             INNER JOIN downloaded_tracks t ON t.track_id = ct.track_id
             INNER JOIN downloaded_collections c ON c.collection_id = ct.collection_id
             WHERE ct.collection_id = ?1
             ORDER BY ct.position ASC, t.downloaded_at ASC",
        )?;
        let rows = stmt.query_map(params![collection_id], |row| {
            self.row_to_downloaded_track(row)
        })?;
        let mut tracks = Vec::new();
        for row in rows {
            let track = row?;
            if is_valid_audio_file(&track.audio_path) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    /// Async counterpart to [`Self::list_collection_tracks`] executed on `tokio::task::spawn_blocking`.
    pub async fn list_collection_tracks_async(
        &self,
        collection_id: String,
    ) -> Result<Vec<DownloadedTrack>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_collection_tracks(&collection_id))
            .await
            .context("download list_collection_tracks task panicked")?
    }

    /// Returns total bytes occupied by downloaded tracks in the manifest.
    pub fn total_bytes(&self) -> Result<u64> {
        let conn = self.lock_conn();
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(file_size_bytes), 0) FROM downloaded_tracks",
            [],
            |row| row.get(0),
        )?;
        Ok(total.max(0) as u64)
    }

    /// Async counterpart to [`Self::total_bytes`] executed on `tokio::task::spawn_blocking`.
    pub async fn total_bytes_async(&self) -> Result<u64> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.total_bytes())
            .await
            .context("download total_bytes task panicked")?
    }

    /// Deletes a single downloaded track from the manifest and disk.
    pub fn delete_track(&self, track_id: &str) -> Result<bool> {
        let deleted_paths: Option<(String, Option<String>)> = {
            let conn = self.lock_conn();
            let tx = conn.unchecked_transaction()?;
            let paths = tx
                .query_row(
                    "SELECT relative_path, artwork_relative_path FROM downloaded_tracks WHERE track_id = ?1",
                    params![track_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if paths.is_some() {
                tx.execute(
                    "DELETE FROM downloaded_tracks WHERE track_id = ?1",
                    params![track_id],
                )?;
                cleanup_empty_collections_tx(&tx)?;
            }
            tx.commit()?;
            paths
        };

        let Some((audio_rel, art_rel)) = deleted_paths else {
            return Ok(false);
        };
        let _ = fs::remove_file(self.root.join(audio_rel));
        if let Some(art_rel) = art_rel {
            let _ = fs::remove_file(self.root.join(art_rel));
        }
        Ok(true)
    }

    /// Async counterpart to [`Self::delete_track`] executed on `tokio::task::spawn_blocking`.
    pub async fn delete_track_async(&self, track_id: String) -> Result<bool> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.delete_track(&track_id))
            .await
            .context("download delete_track task panicked")?
    }

    /// Removes a collection from the manifest and optionally deletes orphaned tracks no longer in any collection.
    pub fn delete_collection(
        &self,
        collection_id: &str,
        delete_orphan_tracks: bool,
    ) -> Result<usize> {
        let orphan_files: Vec<(String, Option<String>)> = {
            let conn = self.lock_conn();
            let tx = conn.unchecked_transaction()?;
            let candidate_tracks: Vec<String> = if delete_orphan_tracks {
                let mut stmt =
                    tx.prepare("SELECT track_id FROM collection_tracks WHERE collection_id = ?1")?;
                let ids = stmt
                    .query_map(params![collection_id], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<String>, _>>()?;
                ids
            } else {
                Vec::new()
            };

            tx.execute(
                "DELETE FROM downloaded_collections WHERE collection_id = ?1",
                params![collection_id],
            )?;
            tx.execute(
                "UPDATE downloaded_tracks SET collection_id = NULL, collection_title = NULL WHERE collection_id = ?1",
                params![collection_id],
            )?;

            let mut to_remove = Vec::new();
            for track_id in candidate_tracks {
                let still_referenced: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM collection_tracks WHERE track_id = ?1",
                    params![&track_id],
                    |row| row.get(0),
                )?;
                if still_referenced == 0 {
                    if let Some(paths) = tx
                        .query_row(
                            "SELECT relative_path, artwork_relative_path FROM downloaded_tracks WHERE track_id = ?1",
                            params![&track_id],
                            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                        )
                        .optional()?
                    {
                        tx.execute(
                            "DELETE FROM downloaded_tracks WHERE track_id = ?1",
                            params![&track_id],
                        )?;
                        to_remove.push(paths);
                    }
                }
            }
            tx.commit()?;
            to_remove
        };

        let removed_count = orphan_files.len();
        for (audio_rel, art_rel) in orphan_files {
            let _ = fs::remove_file(self.root.join(audio_rel));
            if let Some(art_rel) = art_rel {
                let _ = fs::remove_file(self.root.join(art_rel));
            }
        }
        Ok(removed_count)
    }

    /// Async counterpart to [`Self::delete_collection`] executed on `tokio::task::spawn_blocking`.
    pub async fn delete_collection_async(
        &self,
        collection_id: String,
        delete_orphan_tracks: bool,
    ) -> Result<usize> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.delete_collection(&collection_id, delete_orphan_tracks)
        })
        .await
        .context("download delete_collection task panicked")?
    }

    /// Prunes the oldest/least-recently-played downloads until total bytes are at or below `limit_bytes`.
    pub fn prune_to_limit(&self, limit_bytes: u64) -> Result<PruneReport> {
        cleanup_stale_tmp_files(&self.root);

        let (victims, remaining_bytes) = {
            let conn = self.lock_conn();
            let tx = conn.unchecked_transaction()?;

            let mut entries: Vec<(String, String, Option<String>, u64)> = {
                let mut stmt = tx.prepare(
                    "SELECT track_id, relative_path, artwork_relative_path, file_size_bytes
                     FROM downloaded_tracks
                     ORDER BY last_played_at ASC, downloaded_at ASC, rowid ASC",
                )?;
                let mapped = stmt.query_map([], |row| {
                    let size: i64 = row.get(3)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        size.max(0) as u64,
                    ))
                })?;
                mapped.collect::<std::result::Result<Vec<_>, _>>()?
            };

            let mut remaining = entries
                .iter()
                .fold(0_u64, |acc, item| acc.saturating_add(item.3));
            let mut victims = Vec::new();
            for (track_id, audio_rel, art_rel, size) in entries.drain(..) {
                if remaining <= limit_bytes {
                    break;
                }
                tx.execute(
                    "DELETE FROM downloaded_tracks WHERE track_id = ?1",
                    params![&track_id],
                )?;
                remaining = remaining.saturating_sub(size);
                victims.push((audio_rel, art_rel, size));
            }
            if !victims.is_empty() {
                cleanup_empty_collections_tx(&tx)?;
            }
            tx.commit()?;
            (victims, remaining)
        };

        let mut removed_bytes = 0_u64;
        for (audio_rel, art_rel, size) in &victims {
            let _ = fs::remove_file(self.root.join(audio_rel));
            if let Some(art_rel) = art_rel {
                let _ = fs::remove_file(self.root.join(art_rel));
            }
            removed_bytes = removed_bytes.saturating_add(*size);
        }

        if !victims.is_empty() {
            tracing::info!(
                "[sonata:downloads] pruned count={} removed_bytes={} remaining_bytes={} limit_bytes={}",
                victims.len(),
                removed_bytes,
                remaining_bytes,
                limit_bytes
            );
        }

        Ok(PruneReport {
            removed_tracks: victims.len(),
            removed_bytes,
            remaining_bytes,
        })
    }

    /// Async counterpart to [`Self::prune_to_limit`] executed on `tokio::task::spawn_blocking`.
    pub async fn prune_to_limit_async(&self, limit_bytes: u64) -> Result<PruneReport> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.prune_to_limit(limit_bytes))
            .await
            .context("download prune_to_limit task panicked")?
    }

    fn row_to_downloaded_track(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<DownloadedTrack> {
        let source_raw: String = row.get(1)?;
        let duration_secs: Option<i64> = row.get(5)?;
        let relative_path: String = row.get(6)?;
        let file_size_bytes: i64 = row.get(7)?;
        let bitrate_kbps: Option<i64> = row.get(9)?;
        let artwork_rel: Option<String> = row.get(12)?;
        Ok(DownloadedTrack {
            track_id: row.get(0)?,
            source: source_from_str(&source_raw),
            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            duration_secs: duration_secs
                .filter(|secs| *secs >= 0)
                .map(|secs| secs as u64),
            audio_path: self.root.join(&relative_path),
            relative_path,
            file_size_bytes: file_size_bytes.max(0) as u64,
            audio_badge: row.get(8)?,
            bitrate_kbps: bitrate_kbps
                .filter(|kbps| *kbps >= 0)
                .map(|kbps| kbps as u32),
            container: row.get(10)?,
            artwork_url: row.get(11)?,
            artwork_path: artwork_rel.map(|rel| self.root.join(rel)),
            collection_id: row.get(13)?,
            collection_title: row.get(14)?,
            downloaded_at: row.get(15)?,
            last_played_at: row.get(16)?,
        })
    }
}

fn cleanup_empty_collections_tx(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM downloaded_collections
         WHERE NOT EXISTS (
             SELECT 1 FROM collection_tracks ct
             WHERE ct.collection_id = downloaded_collections.collection_id
         )",
        [],
    )?;
    Ok(())
}

fn is_valid_audio_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    let mut header = [0_u8; 4];
    fs::File::open(path)
        .and_then(|mut file| file.read(&mut header))
        .is_ok_and(|read| read > 0)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    pub(crate) struct TempTestDir {
        path: PathBuf,
    }

    impl TempTestDir {
        pub(crate) fn new(prefix: &str) -> Self {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("sonata_{prefix}_{}_{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create test temp dir");
            Self { path }
        }

        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TempTestDir;
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    fn sample_track(id: &str, title: &str, source: ProviderSource) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            duration_secs: Some(215),
            thumbnail_url: Some(format!("https://img.example/{id}.jpg")),
            source,
            audio_badge: Some("OPUS 160kbps · HQ".to_string()),
            set_video_id: None,
            liked: true,
            album_id: Some("MPREb_test".to_string()),
            is_explicit: false,
        }
    }

    #[test]
    fn manifest_roundtrip_stores_quality_artwork_and_collections() {
        let dir = TempTestDir::new("dl_test");
        let store = DownloadStore::open(dir.path()).expect("open download store");

        #[cfg(unix)]
        {
            let root_mode = fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
            let audio_mode = fs::metadata(dir.path().join(AUDIO_SUBDIR))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(root_mode, 0o700);
            assert_eq!(audio_mode, 0o700);
        }

        let input = DownloadTrackInput {
            track: sample_track("ytm_abc123", "Midnight Echo", ProviderSource::YouTubeMusic),
            audio_badge: "OPUS 160kbps · HQ".to_string(),
            bitrate_kbps: None,
            container: ".opus".to_string(),
            artwork_url: None,
            collection: Some(DownloadCollectionRef {
                collection_id: "PL_offline_mix".to_string(),
                title: "Night Drive".to_string(),
                source: ProviderSource::YouTubeMusic,
                artwork_url: Some("https://img.example/playlist.jpg".to_string()),
                position: 0,
            }),
        };
        let audio_payload = vec![0x4f, 0x67, 0x67, 0x53, 1, 2, 3, 4];
        let art_payload = b"\x89PNG\r\n\x1a\npayload".to_vec();

        let saved = store
            .save_track_bytes(&input, &audio_payload, Some(&art_payload))
            .expect("save track");

        assert_eq!(saved.track_id, "ytm_abc123");
        assert_eq!(saved.audio_badge, "OPUS 160kbps · HQ");
        assert_eq!(saved.bitrate_kbps, Some(160));
        assert_eq!(saved.container, "opus");
        assert_eq!(saved.file_size_bytes, audio_payload.len() as u64);
        assert_eq!(
            saved.artwork_url.as_deref(),
            Some("https://img.example/ytm_abc123.jpg")
        );
        assert!(saved.audio_path.is_file());
        assert!(saved.artwork_path.as_ref().is_some_and(|p| p.is_file()));

        let fetched = store
            .get_track("ytm_abc123")
            .expect("get track")
            .expect("track exists");
        assert_eq!(fetched.title, "Midnight Echo");
        assert_eq!(fetched.collection_id.as_deref(), Some("PL_offline_mix"));
        assert_eq!(fetched.collection_title.as_deref(), Some("Night Drive"));

        let collections = store.list_collections().expect("list collections");
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].collection_id, "PL_offline_mix");
        assert_eq!(collections[0].track_count, 1);
        assert_eq!(collections[0].total_bytes, audio_payload.len() as u64);

        let col_tracks = store
            .list_collection_tracks("PL_offline_mix")
            .expect("list collection tracks");
        assert_eq!(col_tracks.len(), 1);
        assert_eq!(col_tracks[0].track_id, "ytm_abc123");

        let stream = store
            .resolve_offline_stream("ytm_abc123")
            .expect("resolve stream")
            .expect("offline stream");
        assert_eq!(stream.url, saved.audio_path.to_string_lossy());
        assert_eq!(stream.badge, "OPUS 160kbps · HQ");
    }

    #[test]
    fn atomic_write_replaces_existing_file_and_cleans_stale_tmp_files() {
        let dir = TempTestDir::new("dl_test");
        let store = DownloadStore::open(dir.path()).expect("open download store");

        let stale_audio_tmp = dir.path().join(AUDIO_SUBDIR).join("leftover.opus.1.tmp");
        let stale_art_tmp = dir.path().join(ARTWORK_SUBDIR).join("leftover.png.2.tmp");
        fs::write(&stale_audio_tmp, b"partial").expect("write stale audio tmp");
        fs::write(&stale_art_tmp, b"partial").expect("write stale art tmp");

        let target = dir.path().join(AUDIO_SUBDIR).join("atomic_track.m4a");
        write_atomic_file(&target, b"first-version").expect("atomic write 1");
        write_atomic_file(&target, b"second-version").expect("atomic write 2");
        assert_eq!(fs::read(&target).unwrap(), b"second-version");

        let removed = cleanup_stale_tmp_files(dir.path());
        assert_eq!(removed, 2);
        assert!(!stale_audio_tmp.exists());
        assert!(!stale_art_tmp.exists());

        // Also test replacing track container removes obsolete audio file cleanly.
        let input_m4a = DownloadTrackInput {
            track: sample_track("swap_id", "Swap Track", ProviderSource::Spotify),
            audio_badge: "AAC 256kbps · HQ".to_string(),
            bitrate_kbps: Some(256),
            container: "m4a".to_string(),
            artwork_url: None,
            collection: None,
        };
        let first = store
            .save_track_bytes(&input_m4a, b"m4a-bytes", None)
            .expect("save m4a");
        assert!(first.audio_path.is_file());

        let input_flac = DownloadTrackInput {
            track: sample_track("swap_id", "Swap Track", ProviderSource::Spotify),
            audio_badge: "FLAC · Hi-Res".to_string(),
            bitrate_kbps: Some(900),
            container: "flac".to_string(),
            artwork_url: None,
            collection: None,
        };
        let second = store
            .save_track_bytes(&input_flac, b"flac-bytes", None)
            .expect("save flac");
        assert!(!first.audio_path.exists());
        assert!(second.audio_path.is_file());
    }

    #[test]
    fn size_limit_pruning_evicts_oldest_played_tracks_and_empty_collections() {
        let dir = TempTestDir::new("dl_test");
        let store = DownloadStore::open(dir.path()).expect("open download store");

        let make_input = |id: &str, col_id: &str| DownloadTrackInput {
            track: sample_track(id, id, ProviderSource::YouTubeMusic),
            audio_badge: "OPUS 160kbps · HQ".to_string(),
            bitrate_kbps: Some(160),
            container: "opus".to_string(),
            artwork_url: None,
            collection: Some(DownloadCollectionRef {
                collection_id: col_id.to_string(),
                title: col_id.to_string(),
                source: ProviderSource::YouTubeMusic,
                artwork_url: None,
                position: 0,
            }),
        };

        let t1 = store
            .save_track_bytes(&make_input("t1", "col_1"), &[1_u8; 40], None)
            .unwrap();
        let t2 = store
            .save_track_bytes(&make_input("t2", "col_2"), &[2_u8; 40], None)
            .unwrap();
        let t3 = store
            .save_track_bytes(&make_input("t3", "col_2"), &[3_u8; 40], None)
            .unwrap();

        // Force deterministic LRU order: t2 oldest, then t1, then t3 newest.
        {
            let conn = store.lock_conn();
            conn.execute(
                "UPDATE downloaded_tracks SET last_played_at = 100, downloaded_at = 100 WHERE track_id = 't2'",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE downloaded_tracks SET last_played_at = 200, downloaded_at = 200 WHERE track_id = 't1'",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE downloaded_tracks SET last_played_at = 300, downloaded_at = 300 WHERE track_id = 't3'",
                [],
            )
            .unwrap();
        }

        assert_eq!(store.total_bytes().unwrap(), 120);
        let report = store.prune_to_limit(80).expect("prune to 80 bytes");
        assert_eq!(report.removed_tracks, 1);
        assert_eq!(report.removed_bytes, 40);
        assert_eq!(report.remaining_bytes, 80);
        assert!(!t2.audio_path.exists());
        assert!(t1.audio_path.exists());
        assert!(t3.audio_path.exists());

        // Prune to 20 bytes: evicts t1 and t3, and cleans up empty collections.
        let report_all = store.prune_to_limit(20).expect("prune to 20 bytes");
        assert_eq!(report_all.removed_tracks, 2);
        assert_eq!(report_all.remaining_bytes, 0);
        assert!(store.list_tracks().unwrap().is_empty());
        assert!(store.list_collections().unwrap().is_empty());
    }

    #[test]
    fn delete_track_and_collection_removes_files_and_preserves_shared_tracks() {
        let dir = TempTestDir::new("dl_test");
        let store = DownloadStore::open(dir.path()).expect("open download store");

        let input = DownloadTrackInput {
            track: sample_track("shared_1", "Shared Song", ProviderSource::YouTubeMusic),
            audio_badge: "AAC 128kbps · Standard".to_string(),
            bitrate_kbps: Some(128),
            container: "m4a".to_string(),
            artwork_url: None,
            collection: Some(DownloadCollectionRef {
                collection_id: "col_a".to_string(),
                title: "Col A".to_string(),
                source: ProviderSource::YouTubeMusic,
                artwork_url: None,
                position: 0,
            }),
        };
        let saved = store.save_track_bytes(&input, &[7_u8; 32], None).unwrap();
        store
            .attach_track_to_collection(
                "shared_1",
                &DownloadCollectionRef {
                    collection_id: "col_b".to_string(),
                    title: "Col B".to_string(),
                    source: ProviderSource::YouTubeMusic,
                    artwork_url: None,
                    position: 0,
                },
            )
            .unwrap();

        // Deleting col_a with delete_orphan_tracks = true keeps shared_1 because col_b still references it.
        let orphaned = store.delete_collection("col_a", true).unwrap();
        assert_eq!(orphaned, 0);
        assert!(saved.audio_path.is_file());

        // Deleting col_b removes shared_1 as an orphan.
        let orphaned_b = store.delete_collection("col_b", true).unwrap();
        assert_eq!(orphaned_b, 1);
        assert!(!saved.audio_path.exists());
        assert!(store.get_track("shared_1").unwrap().is_none());
    }

    #[tokio::test]
    async fn async_wrappers_run_cleanly_off_executor_thread() {
        let dir = TempTestDir::new("dl_test");
        assert!(DownloadStore::open_existing_async(dir.path())
            .await
            .unwrap()
            .is_none());

        let store = DownloadStore::open_async(dir.path())
            .await
            .expect("open async");
        let input = DownloadTrackInput {
            track: sample_track("async_1", "Async Song", ProviderSource::YouTubeMusic),
            audio_badge: "OPUS 160kbps · HQ".to_string(),
            bitrate_kbps: Some(160),
            container: "opus".to_string(),
            artwork_url: None,
            collection: None,
        };
        let saved = store
            .save_track_bytes_async(input, vec![9_u8; 24], None)
            .await
            .expect("save async");
        assert_eq!(store.total_bytes_async().await.unwrap(), 24);
        let stream = store
            .resolve_offline_stream_async("async_1".to_string())
            .await
            .unwrap()
            .expect("resolve offline stream");
        assert_eq!(stream.url, saved.audio_path.to_string_lossy());
        assert!(store
            .delete_track_async("async_1".to_string())
            .await
            .unwrap());
        assert_eq!(store.total_bytes_async().await.unwrap(), 0);
    }
}
