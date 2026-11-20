use crate::cache::{prune_active_media_cache_async, CacheDb};
use crate::config::AppConfig;
use crate::lyrics::{
    lyrics_candidate_matches, parse_lrc, providers, sanitize_title, LyricLine, LyricsState,
};
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use reqwest::Client;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::time::Duration;

type LrclibParsed = (Option<Vec<LyricLine>>, Option<Vec<String>>);

pub async fn fetch_lyrics(
    http: &Client,
    ytm: &YtMusicClient,
    track: &Track,
    cache_db: Option<&CacheDb>,
) -> LyricsState {
    let clean_title = sanitize_title(&track.title);
    let artist = track.artist.trim();
    let (lrc_path, txt_path) = cache_paths(artist, &clean_title);
    let is_ytm = track.source == ProviderSource::YouTubeMusic;

    if is_ytm {
        if let Some(metadata) = cache_db.and_then(|db| db.get_track_enrichment(&track.id)) {
            if metadata.lyrics_status == "FOUND" {
                if let Some(synced) = metadata.synced_lyrics.as_deref() {
                    let parsed = parse_lrc(synced);
                    if !parsed.is_empty() {
                        return LyricsState::Synced(parsed);
                    }
                }
                if let Some(plain) = metadata.plain_lyrics.as_deref() {
                    let lines: Vec<String> = plain.lines().map(str::to_string).collect();
                    if !lines.is_empty() {
                        return LyricsState::Plain(lines);
                    }
                }
            }
            let now = now_secs();
            if metadata.lyrics_status == "NOT_FOUND"
                && metadata.updated_at >= 0
                && now >= metadata.updated_at as u64
                && now - (metadata.updated_at as u64) < 7 * 24 * 60 * 60
            {
                return LyricsState::NotFound;
            }
        }
    }

    if AppConfig::active_cache_limit_bytes() > 0 {
        if let Ok(cached) = tokio::fs::read_to_string(&lrc_path).await {
            let parsed = parse_lrc(&cached);
            if !parsed.is_empty() {
                persist_lyrics(cache_db, track, Some(&parsed), None, "FOUND");
                return LyricsState::Synced(parsed);
            }
        }
        if let Ok(cached_txt) = tokio::fs::read_to_string(&txt_path).await {
            let plain: Vec<String> = cached_txt.lines().map(str::to_string).collect();
            if !plain.is_empty() {
                persist_lyrics(cache_db, track, None, Some(&plain), "FOUND");
                return LyricsState::Plain(plain);
            }
        }
    }

    // 1. LRCLIB synced first; keep plain lyrics only as a backup parachute
    let mut plain_fallback: Option<Vec<String>> = None;
    if let Some((synced, plain)) = query_lrclib(http, artist, &clean_title, track).await {
        if let Some(lines) = synced {
            save_lrc(&lrc_path, &lines).await;
            persist_lyrics(cache_db, track, Some(&lines), None, "FOUND");
            return LyricsState::Synced(lines);
        }
        plain_fallback = plain;
    }

    // LRCLIB plain lyrics remain useful when no synchronized version exists.
    if let Some(plain) = plain_fallback {
        save_text(&txt_path, &plain.join("\n")).await;
        persist_lyrics(cache_db, track, None, Some(&plain), "FOUND");
        return LyricsState::Plain(plain);
    }

    // YTM's native lyrics are the only network fallback for YTM tracks.
    if is_ytm {
        if let Some(raw_text) = ytm.fetch_native_lyrics(&track.id).await {
            let lines: Vec<String> = raw_text.lines().map(|l| l.trim().to_string()).collect();
            if !lines.is_empty() {
                save_text(&txt_path, &lines.join("\n")).await;
                persist_lyrics(cache_db, track, None, Some(&lines), "FOUND");
                return LyricsState::Plain(lines);
            }
        }
        persist_lyrics(cache_db, track, None, None, "NOT_FOUND");
    } else {
        if let Some(lines) =
            providers::fetch_kugou_synced(http, &clean_title, artist, track.duration_secs).await
        {
            save_lrc(&lrc_path, &lines).await;
            return LyricsState::Synced(lines);
        }
        if let Some(lines) = providers::fetch_genius_lyrics(http, &clean_title, artist).await {
            save_text(&txt_path, &lines.join("\n")).await;
            return LyricsState::Plain(lines);
        }
    }

    LyricsState::NotFound
}

async fn query_lrclib(
    http: &Client,
    artist: &str,
    clean_title: &str,
    track: &Track,
) -> Option<LrclibParsed> {
    if !artist.is_empty() && !clean_title.is_empty() {
        let mut req = http
            .get("https://lrclib.net/api/get")
            .header(
                "User-Agent",
                "Sonata/0.1 (https://github.com/MehmetCanWT/sonata)",
            )
            .query(&[("artist_name", artist), ("track_name", clean_title)])
            .timeout(Duration::from_secs(4));
        if !track.album.is_empty() {
            req = req.query(&[("album_name", track.album.as_str())]);
        }
        if let Some(dur) = track.duration_secs {
            req = req.query(&[("duration", dur.to_string().as_str())]);
        }
        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    let parsed = parse_lrclib_item(&val);
                    if parsed.0.is_some() || parsed.1.is_some() {
                        return Some(parsed);
                    }
                }
            }
        }
    }

    let q = if artist.is_empty() || artist == "Unknown Artist" {
        clean_title.to_string()
    } else {
        format!("{artist} {clean_title}")
    };
    let resp = http
        .get("https://lrclib.net/api/search")
        .header(
            "User-Agent",
            "Sonata/0.1 (https://github.com/MehmetCanWT/sonata)",
        )
        .query(&[("q", q.as_str())])
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let arr = resp.json::<Vec<Value>>().await.ok()?;
    pick_best_lrclib_result(&arr, clean_title, artist, track.duration_secs)
}

fn persist_lyrics(
    cache_db: Option<&CacheDb>,
    track: &Track,
    synced: Option<&[LyricLine]>,
    plain: Option<&[String]>,
    status: &str,
) {
    if track.source != ProviderSource::YouTubeMusic {
        return;
    }
    let Some(cache_db) = cache_db else {
        return;
    };
    let synced_lrc = synced.map(format_lrc);
    let plain_text = plain.map(|lines| lines.join("\n"));
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0);
    cache_db.set_track_lyrics(
        &track.id,
        synced_lrc.as_deref(),
        plain_text.as_deref(),
        status,
        updated_at,
    );
}

fn format_lrc(lines: &[LyricLine]) -> String {
    lines
        .iter()
        .map(|line| {
            let mins = (line.time_secs / 60.0).floor() as u64;
            let secs = line.time_secs - mins as f64 * 60.0;
            format!("[{mins:02}:{secs:05.2}] {}", line.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn lrclib_item_matches(
    item: &Value,
    req_title: &str,
    req_artist: &str,
    req_dur: Option<u64>,
) -> bool {
    let cand_title = item.get("trackName").and_then(|v| v.as_str()).unwrap_or("");
    let cand_artist = item
        .get("artistName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let cand_dur = item
        .get("duration")
        .and_then(|v| v.as_f64())
        .map(|d| d.round() as u64);
    lyrics_candidate_matches(
        req_title,
        req_artist,
        req_dur,
        cand_title,
        cand_artist,
        cand_dur,
    )
}

fn pick_best_lrclib_result(
    arr: &[Value],
    req_title: &str,
    req_artist: &str,
    req_dur: Option<u64>,
) -> Option<LrclibParsed> {
    let mut plain_match: Option<LrclibParsed> = None;
    for item in arr {
        if !lrclib_item_matches(item, req_title, req_artist, req_dur) {
            continue;
        }
        let parsed = parse_lrclib_item(item);
        if parsed.0.is_some() {
            return Some(parsed);
        }
        if plain_match.is_none() && parsed.1.is_some() {
            plain_match = Some(parsed);
        }
    }
    plain_match
}

fn parse_lrclib_item(item: &Value) -> LrclibParsed {
    let synced = item
        .get("syncedLyrics")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(parse_lrc)
        .filter(|v| !v.is_empty());
    let plain = item
        .get("plainLyrics")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.lines().map(str::to_string).collect());
    (synced, plain)
}

async fn save_lrc(path: &Path, lines: &[LyricLine]) {
    if AppConfig::active_cache_limit_bytes() == 0 {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let body = format!("{}\n", format_lrc(lines));
    save_text(path, &body).await;
}

async fn save_text(path: &Path, text: &str) {
    if AppConfig::active_cache_limit_bytes() == 0 {
        return;
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
    if tokio::fs::write(&temp_path, text).await.is_ok() {
        if tokio::fs::rename(&temp_path, path).await.is_ok() {
            prune_active_media_cache_async().await;
        } else {
            let _ = tokio::fs::remove_file(temp_path).await;
        }
    }
}

fn cache_paths(artist: &str, title: &str) -> (PathBuf, PathBuf) {
    let mut hasher = Sha1::new();
    hasher.update(format!("{artist}::{title}").to_lowercase().as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let dir = AppConfig::lyrics_cache_dir();
    (
        dir.join(format!("{hex}.lrc")),
        dir.join(format!("{hex}.txt")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unrelated_lrclib_candidates() {
        let arr = vec![
            json!({
                "trackName": "Ikraar Karoon",
                "artistName": "Other Singer",
                "duration": 210.0,
                "syncedLyrics": "[00:01.00] Hindi lyrics"
            }),
            json!({
                "trackName": "Black",
                "artistName": "GAZIROVKA",
                "duration": 166.0,
                "syncedLyrics": "[00:05.00] Мои глаза голодные"
            }),
        ];
        let picked = pick_best_lrclib_result(&arr, "Black", "GAZIROVKA", Some(166))
            .expect("matching track selected");
        let lines = picked.0.expect("synced lyrics");
        assert_eq!(lines[0].text, "Мои глаза голодные");

        let none = pick_best_lrclib_result(&arr[..1], "Black", "GAZIROVKA", Some(166));
        assert!(none.is_none());
    }
}
