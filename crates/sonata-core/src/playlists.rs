//! Portable local personal playlists persisted atomically on disk without YouTube Music writes.
//!
//! 1. **Models (`PersonalPlaylist`, `is_personal_track_id`)**: Stores arbitrary local, FLAC,
//!    self-hosted server, or streaming tracks in user-curated playlists.
//! 2. **Atomic Persistence (`PersonalPlaylistStore`)**: Manages CRUD and track reordering with
//!    `0o700` private directories, `0o600` temporary sibling files, `sync_all`, and atomic rename.

use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::paths::CorePaths;
use crate::providers::local::LOCAL_ID_PREFIX;
use crate::providers::servers::{
    JELLYFIN_ID_PREFIX, SMB_ID_PREFIX, SUBSONIC_ID_PREFIX, WEBDAV_ID_PREFIX,
};
use crate::providers::{Playlist, ProviderSource, Track};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Canonical ID prefix assigned to locally stored personal playlists.
pub const PERSONAL_PLAYLIST_ID_PREFIX: &str = "personal:";

const SCHEMA_VERSION: u32 = 1;
static PLAYLIST_ID_SEQ: AtomicU64 = AtomicU64::new(1);
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns `true` when `id` belongs to a local file or self-hosted server track (`local:`,
/// `jellyfin:`, `subsonic:`, `smb:`, or `webdav:`).
pub fn is_personal_track_id(id: &str) -> bool {
    let trimmed = id.trim();
    trimmed.starts_with(LOCAL_ID_PREFIX)
        || trimmed.starts_with(JELLYFIN_ID_PREFIX)
        || trimmed.starts_with(SUBSONIC_ID_PREFIX)
        || trimmed.starts_with(SMB_ID_PREFIX)
        || trimmed.starts_with(WEBDAV_ID_PREFIX)
}

/// Locally persisted user playlist capable of holding local, server, and streaming tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalPlaylist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub created_at_epoch_secs: u64,
    #[serde(default)]
    pub updated_at_epoch_secs: u64,
}

impl PartialEq for PersonalPlaylist {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.created_at_epoch_secs == other.created_at_epoch_secs
            && self.updated_at_epoch_secs == other.updated_at_epoch_secs
            && self.tracks.len() == other.tracks.len()
            && self
                .tracks
                .iter()
                .zip(other.tracks.iter())
                .all(|(a, b)| a.id == b.id && a.title == b.title && a.artist == b.artist)
    }
}

impl Eq for PersonalPlaylist {}

impl PersonalPlaylist {
    /// Converts this personal playlist into Sonata's shared [`Playlist`] summary model.
    pub fn to_playlist(&self) -> Playlist {
        Playlist {
            id: self.id.clone(),
            name: self.name.clone(),
            source: ProviderSource::YouTubeMusic,
            track_count: Some(self.tracks.len()),
            thumbnail_url: self.tracks.iter().find_map(|t| t.thumbnail_url.clone()),
            is_owned: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPersonalPlaylists {
    #[serde(default = "default_schema_version")]
    version: u32,
    #[serde(default)]
    playlists: Vec<PersonalPlaylist>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn generate_playlist_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = PLAYLIST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{PERSONAL_PLAYLIST_ID_PREFIX}{nanos:x}_{seq}")
}

/// Thread-safe, atomically persisted store for [`PersonalPlaylist`] records.
#[derive(Debug, Clone)]
pub struct PersonalPlaylistStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl PersonalPlaylistStore {
    /// Opens a personal playlist store backed by `paths.personal_playlists_path()`.
    pub fn new(paths: &CorePaths) -> Self {
        Self::from_path(paths.personal_playlists_path())
    }

    /// Opens a personal playlist store backed by an explicit file path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// Returns the backing JSON file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lists all saved personal playlists in creation/stored order.
    pub fn list(&self) -> Result<Vec<PersonalPlaylist>, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        self.read_all_unlocked()
    }

    /// Looks up one personal playlist by its ID.
    pub fn get(&self, playlist_id: &str) -> Result<Option<PersonalPlaylist>, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let items = self.read_all_unlocked()?;
        Ok(items.into_iter().find(|p| p.id == playlist_id))
    }

    /// Creates a new empty personal playlist and persists it atomically.
    pub fn create(&self, name: &str) -> Result<PersonalPlaylist, CoreError> {
        let clean = name.trim();
        if clean.is_empty() {
            return Err(CoreError::invalid_response(
                "playlists",
                "personal playlist name cannot be empty",
            ));
        }

        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let now = now_epoch_secs();
        let created = PersonalPlaylist {
            id: generate_playlist_id(),
            name: clean.to_string(),
            tracks: Vec::new(),
            created_at_epoch_secs: now,
            updated_at_epoch_secs: now,
        };
        items.push(created.clone());
        self.write_all_unlocked(&items)?;
        Ok(created)
    }

    /// Renames an existing personal playlist and persists the change atomically.
    pub fn rename(&self, playlist_id: &str, new_name: &str) -> Result<PersonalPlaylist, CoreError> {
        let clean = new_name.trim();
        if clean.is_empty() {
            return Err(CoreError::invalid_response(
                "playlists",
                "personal playlist name cannot be empty",
            ));
        }

        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let Some(target) = items.iter_mut().find(|p| p.id == playlist_id) else {
            return Err(CoreError::not_found(format!(
                "personal playlist `{playlist_id}` not found"
            )));
        };
        target.name = clean.to_string();
        target.updated_at_epoch_secs = now_epoch_secs();
        let updated = target.clone();
        self.write_all_unlocked(&items)?;
        Ok(updated)
    }

    /// Deletes a personal playlist by ID, returning `true` when an entry was removed.
    pub fn delete(&self, playlist_id: &str) -> Result<bool, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let before = items.len();
        items.retain(|p| p.id != playlist_id);
        let removed = items.len() != before;
        if removed {
            self.write_all_unlocked(&items)?;
        }
        Ok(removed)
    }

    /// Appends `track` (local, server, or streaming) to `playlist_id` if not already present.
    pub fn add_track(
        &self,
        playlist_id: &str,
        track: Track,
    ) -> Result<PersonalPlaylist, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let Some(target) = items.iter_mut().find(|p| p.id == playlist_id) else {
            return Err(CoreError::not_found(format!(
                "personal playlist `{playlist_id}` not found"
            )));
        };
        if !target.tracks.iter().any(|t| t.id == track.id) {
            target.tracks.push(track);
            target.updated_at_epoch_secs = now_epoch_secs();
        }
        let updated = target.clone();
        self.write_all_unlocked(&items)?;
        Ok(updated)
    }

    /// Removes all occurrences of `track_id` from `playlist_id`.
    pub fn remove_track(
        &self,
        playlist_id: &str,
        track_id: &str,
    ) -> Result<PersonalPlaylist, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let Some(target) = items.iter_mut().find(|p| p.id == playlist_id) else {
            return Err(CoreError::not_found(format!(
                "personal playlist `{playlist_id}` not found"
            )));
        };
        let before = target.tracks.len();
        target.tracks.retain(|t| t.id != track_id);
        if target.tracks.len() != before {
            target.updated_at_epoch_secs = now_epoch_secs();
        }
        let updated = target.clone();
        self.write_all_unlocked(&items)?;
        Ok(updated)
    }

    /// Moves a track from `from_index` to `to_index` within `playlist_id`.
    pub fn reorder_track(
        &self,
        playlist_id: &str,
        from_index: usize,
        to_index: usize,
    ) -> Result<PersonalPlaylist, CoreError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| CoreError::internal("personal playlists store lock poisoned"))?;
        let mut items = self.read_all_unlocked()?;
        let Some(target) = items.iter_mut().find(|p| p.id == playlist_id) else {
            return Err(CoreError::not_found(format!(
                "personal playlist `{playlist_id}` not found"
            )));
        };
        let len = target.tracks.len();
        if from_index >= len || to_index >= len {
            return Err(CoreError::invalid_response(
                "playlists",
                format!(
                    "reorder indices ({from_index} -> {to_index}) out of bounds for {len} tracks"
                ),
            ));
        }
        if from_index != to_index {
            let item = target.tracks.remove(from_index);
            target.tracks.insert(to_index, item);
            target.updated_at_epoch_secs = now_epoch_secs();
            let updated = target.clone();
            self.write_all_unlocked(&items)?;
            return Ok(updated);
        }
        Ok(target.clone())
    }

    fn read_all_unlocked(&self) -> Result<Vec<PersonalPlaylist>, CoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path).map_err(|err| {
            CoreError::io(format!(
                "failed to read personal playlists {}: {err}",
                self.path.display()
            ))
        })?;
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let parsed: PersistedPersonalPlaylists = serde_json::from_str(&raw).map_err(|err| {
            CoreError::invalid_response(
                "playlists",
                format!("malformed personal playlists JSON: {err}"),
            )
        })?;
        Ok(parsed.playlists)
    }

    fn write_all_unlocked(&self, playlists: &[PersonalPlaylist]) -> Result<(), CoreError> {
        let parent = self.path.parent().ok_or_else(|| {
            CoreError::io(format!(
                "personal playlists path {} has no parent directory",
                self.path.display()
            ))
        })?;
        ensure_private_dir(parent).map_err(|err| {
            CoreError::io(format!("failed to create {}: {err}", parent.display()))
        })?;

        let payload = PersistedPersonalPlaylists {
            version: SCHEMA_VERSION,
            playlists: playlists.to_vec(),
        };
        let json = serde_json::to_string_pretty(&payload).map_err(|err| {
            CoreError::internal(format!("failed to serialize personal playlists: {err}"))
        })?;

        let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("personal_playlists.json");
        let tmp_path = parent.join(format!(".{file_name}.{}.{seq}.tmp", std::process::id()));

        let write_res = (|| -> std::io::Result<()> {
            let mut file = create_private_file(&tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp_path, &self.path)?;
            Ok(())
        })();

        if let Err(err) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoreError::io(format!(
                "failed to persist personal playlists {}: {err}",
                self.path.display()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "sonata_playlists_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample_track(id: &str, title: &str) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_secs: Some(210),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("FLAC · 880 kbps".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        }
    }

    #[test]
    fn detects_personal_track_id_prefixes() {
        assert!(is_personal_track_id("local:%2Fmusic%2Fsong.flac"));
        assert!(is_personal_track_id("jellyfin:srv1:item42"));
        assert!(is_personal_track_id("subsonic:srv2:tr-1"));
        assert!(is_personal_track_id("smb:nas:Album/01.flac"));
        assert!(is_personal_track_id("webdav:cloud:/Music/01.opus"));
        assert!(!is_personal_track_id("dQw4w9WgXcQ"));
        assert!(!is_personal_track_id("jiosaavn:abc123"));
        assert!(!is_personal_track_id("spotify:track:123"));
    }

    #[test]
    fn personal_playlist_crud_reorder_and_private_permissions() {
        let root = unique_temp_root("crud");
        let paths = CorePaths::for_app_roots(root.join("cfg"), root.join("cache"));
        let store = PersonalPlaylistStore::new(&paths);

        assert!(store.list().expect("empty list").is_empty());

        let p1 = store.create("FLAC Favorites").expect("create p1");
        assert!(p1.id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX));
        assert_eq!(p1.name, "FLAC Favorites");

        let renamed = store.rename(&p1.id, "Lossless Gems").expect("rename");
        assert_eq!(renamed.name, "Lossless Gems");

        let t_local = sample_track("local:%2Ftmp%2F01.flac", "One");
        let t_smb = sample_track("smb:nas:02.flac", "Two");
        let t_ytm = sample_track("dQw4w9WgXcQ", "Three");

        store.add_track(&p1.id, t_local.clone()).expect("add local");
        store.add_track(&p1.id, t_smb.clone()).expect("add smb");
        store.add_track(&p1.id, t_ytm.clone()).expect("add ytm");
        // Duplicate track id is ignored cleanly.
        let after_dup = store.add_track(&p1.id, t_smb.clone()).expect("add dup");
        assert_eq!(after_dup.tracks.len(), 3);

        let reordered = store.reorder_track(&p1.id, 2, 0).expect("reorder");
        assert_eq!(
            reordered
                .tracks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            vec!["dQw4w9WgXcQ", "local:%2Ftmp%2F01.flac", "smb:nas:02.flac"]
        );

        let after_remove = store
            .remove_track(&p1.id, "local:%2Ftmp%2F01.flac")
            .expect("remove track");
        assert_eq!(after_remove.tracks.len(), 2);

        let summary = after_remove.to_playlist();
        assert!(summary.is_owned);
        assert_eq!(summary.track_count, Some(2));
        assert_eq!(summary.name, "Lossless Gems");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let dir_mode = fs::metadata(&paths.config_dir)
                .expect("dir meta")
                .permissions()
                .mode()
                & 0o777;
            let file_mode = fs::metadata(paths.personal_playlists_path())
                .expect("file meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }

        assert!(store.delete(&p1.id).expect("delete"));
        assert!(!store.delete(&p1.id).expect("delete missing"));
        assert!(store.list().expect("list after delete").is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
