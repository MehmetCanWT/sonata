//! Local-only SQLite WAL listening statistics engine with zero external telemetry.
//!
//! 1. **Storage (`StatsDb`)**: Opens `paths.stats_db_path()` (`stats.db`) in WAL mode with `0o700`
//!    parent directories and `0o600` database permissions on creation.
//! 2. **Recording (`record_play`)**: Persists track metadata, source tag, duration, listened seconds,
//!    and Unix timestamp strictly on-device.
//! 3. **Aggregations (`query_summary`)**: Computes totals, unique counts, daily listening streak,
//!    top artists/tracks/albums, and source breakdowns across `Week`, `Month`, `Year`, and `AllTime`.

use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::paths::CorePaths;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const STATS_SCHEMA_VERSION: i64 = 1;
const TOP_LIMIT: usize = 20;

/// Time range filter for [`StatsDb::query_summary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatsWindow {
    Week,
    Month,
    Year,
    AllTime,
}

impl StatsWindow {
    fn start_unix(self, now_unix: u64) -> u64 {
        match self {
            Self::Week => now_unix.saturating_sub(7 * 86_400),
            Self::Month => now_unix.saturating_sub(30 * 86_400),
            Self::Year => now_unix.saturating_sub(365 * 86_400),
            Self::AllTime => 0,
        }
    }
}

/// Top-artist aggregation item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistStat {
    pub artist: String,
    pub plays: u64,
    pub minutes: u64,
}

/// Top-track aggregation item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackStat {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source_tag: String,
    pub plays: u64,
    pub minutes: u64,
}

/// Top-album aggregation item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumStat {
    pub album: String,
    pub artist: String,
    pub plays: u64,
    pub minutes: u64,
}

/// Source-tag breakdown item (`YTM`, `LOC`, `FLAC`, `JFN`, `SUB`, `SMB`, `DAV`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStat {
    pub source_tag: String,
    pub plays: u64,
    pub minutes: u64,
}

/// Aggregated on-device listening report for a requested [`StatsWindow`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsSummary {
    pub total_plays: u64,
    pub total_minutes: u64,
    pub unique_tracks: u64,
    pub unique_artists: u64,
    pub streak_days: u64,
    pub top_artists: Vec<ArtistStat>,
    pub top_tracks: Vec<TrackStat>,
    pub top_albums: Vec<AlbumStat>,
    pub source_breakdown: Vec<SourceStat>,
}

/// Thread-safe, offline SQLite WAL listening statistics store.
#[derive(Clone)]
pub struct StatsDb {
    conn: Arc<Mutex<Connection>>,
}

impl StatsDb {
    /// Opens the listening stats SQLite database under `paths.stats_db_path()`.
    pub fn open_with_paths(paths: &CorePaths) -> Result<Self, CoreError> {
        Self::open_at(&paths.stats_db_path())
    }

    /// Opens or initializes the listening stats SQLite database at `path`.
    pub fn open_at(path: &Path) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent).map_err(|err| CoreError::io(err.to_string()))?;
        }
        if !path.exists() {
            create_private_file(path).map_err(|err| CoreError::io(err.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|err| CoreError::cache(err.to_string()))?;
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let schema_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|err| CoreError::cache(err.to_string()))?;
        if schema_version > STATS_SCHEMA_VERSION {
            return Err(CoreError::cache(format!(
                "stats.db schema version {schema_version} exceeds supported {STATS_SCHEMA_VERSION}"
            )));
        }

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS play_events (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 track_id TEXT NOT NULL,
                 title TEXT NOT NULL,
                 artist TEXT NOT NULL,
                 album TEXT NOT NULL DEFAULT '',
                 source_tag TEXT NOT NULL DEFAULT '',
                 duration_secs INTEGER NOT NULL DEFAULT 0,
                 played_secs INTEGER NOT NULL DEFAULT 0,
                 played_at_unix INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS play_events_played_at_idx ON play_events(played_at_unix);
             CREATE INDEX IF NOT EXISTS play_events_artist_idx ON play_events(artist);
             CREATE INDEX IF NOT EXISTS play_events_track_idx ON play_events(track_id);
             PRAGMA user_version = 1;
             COMMIT;",
        )
        .map_err(|err| CoreError::cache(err.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Records a single completed or significant track listen locally in SQLite.
    #[allow(clippy::too_many_arguments)]
    pub fn record_play(
        &self,
        track_id: &str,
        title: &str,
        artist: &str,
        album: &str,
        source_tag: &str,
        duration_secs: u64,
        played_secs: u64,
        played_at_unix: u64,
    ) -> Result<(), CoreError> {
        let conn = self.lock_conn();
        conn.prepare_cached(
            "INSERT INTO play_events (
                track_id, title, artist, album, source_tag, duration_secs, played_secs, played_at_unix
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .and_then(|mut stmt| {
            stmt.execute(params![
                track_id.trim(),
                title.trim(),
                artist.trim(),
                album.trim(),
                source_tag.trim(),
                duration_secs.min(i64::MAX as u64) as i64,
                played_secs.min(i64::MAX as u64) as i64,
                played_at_unix.min(i64::MAX as u64) as i64,
            ])
        })
        .map(|_| ())
        .map_err(|err| CoreError::cache(err.to_string()))
    }

    /// Queries an offline listening summary for `window` relative to the current system clock.
    pub fn query_summary(&self, window: StatsWindow) -> Result<StatsSummary, CoreError> {
        self.query_summary_at(window, now_unix_secs())
    }

    /// Queries an offline listening summary for `window` anchored at `now_unix`.
    pub fn query_summary_at(
        &self,
        window: StatsWindow,
        now_unix: u64,
    ) -> Result<StatsSummary, CoreError> {
        let start_unix = window.start_unix(now_unix).min(i64::MAX as u64) as i64;
        let end_unix = now_unix.min(i64::MAX as u64) as i64;
        let conn = self.lock_conn();

        let (total_plays, total_played_secs, unique_tracks, unique_artists): (i64, i64, i64, i64) =
            conn.prepare_cached(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(played_secs), 0),
                    COUNT(DISTINCT NULLIF(track_id, '')),
                    COUNT(DISTINCT NULLIF(artist, ''))
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2",
            )
            .and_then(|mut stmt| {
                stmt.query_row(params![start_unix, end_unix], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let top_artists = conn
            .prepare_cached(
                "SELECT artist, COUNT(*) AS plays, COALESCE(SUM(played_secs), 0) AS secs
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2 AND artist != ''
                 GROUP BY artist
                 ORDER BY plays DESC, secs DESC, artist ASC
                 LIMIT ?3",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![start_unix, end_unix, TOP_LIMIT as i64], |row| {
                    let secs = row.get::<_, i64>(2)?.max(0) as u64;
                    Ok(ArtistStat {
                        artist: row.get(0)?,
                        plays: row.get::<_, i64>(1)?.max(0) as u64,
                        minutes: secs / 60,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let top_tracks = conn
            .prepare_cached(
                "SELECT track_id, title, artist, album, source_tag, COUNT(*) AS plays, COALESCE(SUM(played_secs), 0) AS secs
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2 AND title != ''
                 GROUP BY track_id, title, artist
                 ORDER BY plays DESC, secs DESC, title ASC
                 LIMIT ?3",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![start_unix, end_unix, TOP_LIMIT as i64], |row| {
                    let secs = row.get::<_, i64>(6)?.max(0) as u64;
                    Ok(TrackStat {
                        track_id: row.get(0)?,
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        album: row.get(3)?,
                        source_tag: row.get(4)?,
                        plays: row.get::<_, i64>(5)?.max(0) as u64,
                        minutes: secs / 60,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let top_albums = conn
            .prepare_cached(
                "SELECT album, artist, COUNT(*) AS plays, COALESCE(SUM(played_secs), 0) AS secs
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2 AND album != ''
                 GROUP BY album, artist
                 ORDER BY plays DESC, secs DESC, album ASC
                 LIMIT ?3",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![start_unix, end_unix, TOP_LIMIT as i64], |row| {
                    let secs = row.get::<_, i64>(3)?.max(0) as u64;
                    Ok(AlbumStat {
                        album: row.get(0)?,
                        artist: row.get(1)?,
                        plays: row.get::<_, i64>(2)?.max(0) as u64,
                        minutes: secs / 60,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let source_breakdown = conn
            .prepare_cached(
                "SELECT CASE WHEN source_tag = '' THEN 'UNK' ELSE source_tag END AS tag,
                        COUNT(*) AS plays,
                        COALESCE(SUM(played_secs), 0) AS secs
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2
                 GROUP BY tag
                 ORDER BY plays DESC, secs DESC, tag ASC",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![start_unix, end_unix], |row| {
                    let secs = row.get::<_, i64>(2)?.max(0) as u64;
                    Ok(SourceStat {
                        source_tag: row.get(0)?,
                        plays: row.get::<_, i64>(1)?.max(0) as u64,
                        minutes: secs / 60,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let days: BTreeSet<u64> = conn
            .prepare_cached(
                "SELECT DISTINCT played_at_unix / 86400
                 FROM play_events
                 WHERE played_at_unix >= ?1 AND played_at_unix <= ?2",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![start_unix, end_unix], |row| {
                    Ok(row.get::<_, i64>(0)?.max(0) as u64)
                })?
                .collect::<rusqlite::Result<BTreeSet<u64>>>()
            })
            .map_err(|err| CoreError::cache(err.to_string()))?;

        let streak_days = compute_streak_days(&days, now_unix / 86_400);

        Ok(StatsSummary {
            total_plays: total_plays.max(0) as u64,
            total_minutes: (total_played_secs.max(0) as u64) / 60,
            unique_tracks: unique_tracks.max(0) as u64,
            unique_artists: unique_artists.max(0) as u64,
            streak_days,
            top_artists,
            top_tracks,
            top_albums,
            source_breakdown,
        })
    }
}

/// Counts consecutive UTC days with plays ending at `today_day` or `today_day - 1`.
pub fn compute_streak_days(days: &BTreeSet<u64>, today_day: u64) -> u64 {
    let Some(&latest) = days.iter().next_back() else {
        return 0;
    };
    if latest < today_day.saturating_sub(1) {
        return 0;
    }
    let mut streak = 0u64;
    let mut cursor = latest;
    loop {
        if days.contains(&cursor) {
            streak += 1;
            if cursor == 0 {
                break;
            }
            cursor -= 1;
        } else {
            break;
        }
    }
    streak
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_stats_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonata_stats_{name}_{}_{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn records_plays_and_queries_windows_streaks_and_breakdowns() {
        let root = temp_stats_dir("summary");
        let paths = CorePaths::for_app_roots(&root, &root);
        let db = StatsDb::open_with_paths(&paths).expect("open stats db");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(paths.stats_db_path())
                .expect("db metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let now = 1_700_500_000u64;
        let day = 86_400u64;

        // 3-day consecutive streak ending today (`now`, `now - 1d`, `now - 2d`)
        db.record_play(
            "t1",
            "Song A",
            "Artist 1",
            "Album X",
            "YTM",
            200,
            180,
            now - 60,
        )
        .unwrap();
        db.record_play(
            "t1",
            "Song A",
            "Artist 1",
            "Album X",
            "YTM",
            200,
            180,
            now - day,
        )
        .unwrap();
        db.record_play(
            "local:1",
            "Song B",
            "Artist 2",
            "Album Y",
            "LOC",
            240,
            240,
            now - 2 * day,
        )
        .unwrap();

        // Older play in Month window (15 days ago) — breaks streak gap before 2d ago
        db.record_play(
            "jf:1",
            "Song C",
            "Artist 1",
            "Album X",
            "JFN",
            300,
            300,
            now - 15 * day,
        )
        .unwrap();

        // Older play in Year window (120 days ago)
        db.record_play(
            "t4",
            "Song D",
            "Artist 3",
            "Album Z",
            "SPF",
            180,
            180,
            now - 120 * day,
        )
        .unwrap();

        // Older play only in AllTime (500 days ago)
        db.record_play(
            "t5",
            "Song E",
            "Artist 4",
            "Album W",
            "SUB",
            120,
            120,
            now - 500 * day,
        )
        .unwrap();

        let week = db.query_summary_at(StatsWindow::Week, now).unwrap();
        assert_eq!(week.total_plays, 3);
        assert_eq!(week.total_minutes, 10);
        assert_eq!(week.unique_tracks, 2);
        assert_eq!(week.unique_artists, 2);
        assert_eq!(week.streak_days, 3);
        assert_eq!(week.top_artists[0].artist, "Artist 1");
        assert_eq!(week.top_artists[0].plays, 2);
        assert_eq!(week.top_tracks[0].title, "Song A");
        assert_eq!(week.top_albums[0].album, "Album X");
        assert_eq!(week.source_breakdown.len(), 2);

        let month = db.query_summary_at(StatsWindow::Month, now).unwrap();
        assert_eq!(month.total_plays, 4);
        assert_eq!(month.total_minutes, 15);
        assert_eq!(month.streak_days, 3);

        let year = db.query_summary_at(StatsWindow::Year, now).unwrap();
        assert_eq!(year.total_plays, 5);
        assert_eq!(year.total_minutes, 18);

        let all = db.query_summary_at(StatsWindow::AllTime, now).unwrap();
        assert_eq!(all.total_plays, 6);
        assert_eq!(all.total_minutes, 20);
        assert_eq!(all.unique_tracks, 5);
        assert_eq!(all.unique_artists, 4);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn streak_resets_when_last_play_older_than_yesterday() {
        let days = BTreeSet::from([100, 101, 102]);
        assert_eq!(compute_streak_days(&days, 102), 3);
        assert_eq!(compute_streak_days(&days, 103), 3);
        assert_eq!(compute_streak_days(&days, 104), 0);
        assert_eq!(compute_streak_days(&BTreeSet::new(), 104), 0);
    }
}
