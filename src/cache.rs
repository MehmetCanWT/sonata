use crate::config::AppConfig;
use crate::providers::Track;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct CacheDb {
    conn: Arc<Mutex<Connection>>,
}

impl CacheDb {
    pub fn open_default() -> Option<Self> {
        let path = AppConfig::cache_dir().join("sonata_cache.db");
        Self::open_at(&path)
    }

    pub fn open_at(path: &PathBuf) -> Option<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).ok()?;
        let _ = conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
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
             );",
        );
        Some(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn get_stream(&self, key: &str) -> Option<(String, String)> {
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
        let now = now_secs() as i64;
        let expires_at = extract_url_expire(url)
            .map(|e| (e as i64).saturating_sub(120))
            .unwrap_or(now + 4 * 3600);
        if expires_at <= now {
            return;
        }
        if let Ok(guard) = self.conn.lock() {
            let _ = guard.execute(
                "INSERT OR REPLACE INTO stream_cache (cache_key, url, badge, expires_at) VALUES (?1, ?2, ?3, ?4)",
                params![key, url, badge, expires_at],
            );
        }
    }

    /// Returns `(name, tracks, is_fresh)` if cached on disk.
    pub fn get_playlist(&self, key: &str) -> Option<(String, Vec<Track>, bool)> {
        let now = now_secs() as i64;
        let guard = self.conn.lock().ok()?;
        let (name, json, updated_at): (String, String, i64) = guard
            .query_row(
                "SELECT name, tracks_json, updated_at FROM playlist_cache WHERE playlist_key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok()?;
        let tracks: Vec<Track> = serde_json::from_str(&json).ok()?;
        let is_fresh = (now - updated_at) < 300;
        Some((name, tracks, is_fresh))
    }

    pub fn put_playlist(&self, key: &str, name: &str, tracks: &[Track]) {
        let Ok(json) = serde_json::to_string(tracks) else {
            return;
        };
        let now = now_secs() as i64;
        if let Ok(guard) = self.conn.lock() {
            let _ = guard.execute(
                "INSERT OR REPLACE INTO playlist_cache (playlist_key, name, tracks_json, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![key, name, json, now],
            );
        }
    }

    pub fn invalidate_playlist(&self, key: Option<&str>) {
        if let Ok(guard) = self.conn.lock() {
            match key {
                Some(k) => {
                    let _ = guard.execute(
                        "DELETE FROM playlist_cache WHERE playlist_key = ?1",
                        params![k],
                    );
                }
                None => {
                    let _ = guard.execute("DELETE FROM playlist_cache", []);
                }
            }
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn extract_url_expire(url: &str) -> Option<u64> {
    let idx = url.find("expire=")?;
    let rest = &url[idx + "expire=".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    #[test]
    fn extracts_youtube_expire_param() {
        let url = "https://rr4.googlevideo.com/videoplayback?expire=1795030319&itag=774";
        assert_eq!(extract_url_expire(url), Some(1795030319));
        assert_eq!(extract_url_expire("https://aac.saavncdn.com/test.mp4"), None);
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
}
