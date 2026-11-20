//! Lyrics lookup orchestration for Sonata, centered on word-synced TTML/KRC/YRC and LRCLIB.
//!
//! 1. **Lookup Chain**: `fetch_lyrics` checks SQLite enrichment and on-disk `.lrc`/`.txt` caches,
//!    upgrading legacy line-only `.lrc` entries once when word-level or dual-voice lyrics exist.
//! 2. **Concurrent Queries**: queries TTML (`lyrics-api.binimum.org`) and LRCLIB concurrently,
//!    preferring word-timed or dual-voice (`Lead`/`Counter`) sheets, then falling back to KuGou
//!    KRC and NetEase YRC before line-only or plain lyrics.
//! 3. **Candidate Selection**: `pick_best_lrclib_result` accepts only results whose artist and title
//!    match the track, preferring synced lyrics.
//! 4. **Quality Filtering**: credit and metadata lines are stripped from lyrics, and unusable results
//!    are rejected.
//! 5. **Persistence**: found lyrics are saved to the cache directory as Enhanced LRC (`v2:` +
//!    `<mm:ss.xxx>`) and to the enrichment table, with the media cache pruned afterwards.

use crate::cache::{prune_active_media_cache_async, CacheDb};
use crate::config::AppConfig;
use crate::lyrics::{
    format_lrc, lyrics_candidate_matches, parse_lrc, providers, sanitize_title, LyricLine,
    LyricVoice, LyricsState,
};
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use reqwest::Client;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

type LrclibParsed = (Option<Vec<LyricLine>>, Option<Vec<String>>);

fn has_rich_timing(lines: &[LyricLine]) -> bool {
    lines
        .iter()
        .any(|l| !l.words.is_empty() || l.voice == LyricVoice::Counter)
}

fn word_upgrade_checked(cache_key: &str, check_path: &Path) -> bool {
    static SESSION_CHECKED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = SESSION_CHECKED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(guard) = set.lock() {
        if guard.contains(cache_key) {
            return true;
        }
    }
    if let Ok(raw) = std::fs::read_to_string(check_path) {
        if let Ok(ts) = raw.trim().parse::<u64>() {
            let now = now_secs();
            if now >= ts && now - ts < 7 * 24 * 60 * 60 {
                return true;
            }
        }
    }
    false
}

async fn mark_word_upgrade_checked(cache_key: &str, check_path: &Path) {
    static SESSION_CHECKED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = SESSION_CHECKED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut guard) = set.lock() {
        guard.insert(cache_key.to_string());
    }
    if AppConfig::active_cache_limit_bytes() > 0 {
        if let Some(parent) = check_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(check_path, now_secs().to_string()).await;
    }
}

pub async fn fetch_lyrics(
    http: &Client,
    ytm: &YtMusicClient,
    track: &Track,
    cache_db: Option<&CacheDb>,
) -> LyricsState {
    let clean_title = sanitize_title(&track.title);
    let artist = track.artist.trim();
    let (lrc_path, txt_path, check_path) = cache_paths(artist, &clean_title);
    let cache_key = format!("{}::{}", artist.to_lowercase(), clean_title.to_lowercase());
    let is_ytm = track.source == ProviderSource::YouTubeMusic;

    let mut cached_synced_fallback: Option<Vec<LyricLine>> = None;
    let mut cached_plain_fallback: Option<Vec<String>> = None;

    if is_ytm {
        if let Some(metadata) = cache_db.and_then(|db| db.get_track_enrichment(&track.id)) {
            if metadata.lyrics_status == "FOUND" {
                if let Some(synced) = metadata.synced_lyrics.as_deref() {
                    if let Some(parsed) = usable_synced_lyrics(parse_lrc(synced), track) {
                        if has_rich_timing(&parsed) {
                            return LyricsState::Synced(parsed);
                        }
                        cached_synced_fallback = Some(parsed);
                    }
                }
                if cached_synced_fallback.is_none() {
                    if let Some(plain) = metadata.plain_lyrics.as_deref() {
                        if let Some(lines) =
                            usable_plain_lyrics(plain.lines().map(str::to_string), track)
                        {
                            cached_plain_fallback = Some(lines);
                        }
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
            if let Some(parsed) = usable_synced_lyrics(parse_lrc(&cached), track) {
                if has_rich_timing(&parsed) {
                    persist_lyrics(cache_db, track, Some(&parsed), None, "FOUND");
                    return LyricsState::Synced(parsed);
                }
                if cached_synced_fallback.is_none() {
                    cached_synced_fallback = Some(parsed);
                }
            }
        }
        if cached_synced_fallback.is_none() && cached_plain_fallback.is_none() {
            if let Ok(cached_txt) = tokio::fs::read_to_string(&txt_path).await {
                if let Some(plain) =
                    usable_plain_lyrics(cached_txt.lines().map(str::to_string), track)
                {
                    cached_plain_fallback = Some(plain);
                }
            }
        }
    }

    // If we already have line-only synced lyrics and already tried upgrading once, return them.
    if let Some(ref lines) = cached_synced_fallback {
        if word_upgrade_checked(&cache_key, &check_path) {
            return LyricsState::Synced(lines.clone());
        }
    } else if let Some(ref plain) = cached_plain_fallback {
        return LyricsState::Plain(plain.clone());
    }

    // Query TTML (word-timed & dual-voice) and LRCLIB concurrently.
    let (ttml_raw, lrclib_raw) = tokio::join!(
        providers::fetch_ttml_synced(
            http,
            &clean_title,
            artist,
            &track.album,
            track.duration_secs
        ),
        query_lrclib(http, artist, &clean_title, track),
    );

    let ttml_synced = ttml_raw.and_then(|lines| usable_synced_lyrics(lines, track));
    let (lrc_synced, lrc_plain) = match lrclib_raw {
        Some((synced, plain)) => (
            synced.and_then(|lines| usable_synced_lyrics(lines, track)),
            plain.and_then(|lines| usable_plain_lyrics(lines, track)),
        ),
        None => (None, None),
    };

    if let Some(lines) = ttml_synced.as_ref().filter(|l| has_rich_timing(l)) {
        save_lrc(&lrc_path, lines).await;
        persist_lyrics(cache_db, track, Some(lines), None, "FOUND");
        return LyricsState::Synced(lines.clone());
    }
    if let Some(lines) = lrc_synced.as_ref().filter(|l| has_rich_timing(l)) {
        save_lrc(&lrc_path, lines).await;
        persist_lyrics(cache_db, track, Some(lines), None, "FOUND");
        return LyricsState::Synced(lines.clone());
    }

    // Neither TTML nor LRCLIB returned word-level timings: try KuGou KRC & NetEase YRC.
    let (kugou_raw, netease_raw) = tokio::join!(
        providers::fetch_kugou_synced(http, &clean_title, artist, track.duration_secs),
        providers::fetch_netease_synced(http, &clean_title, artist, track.duration_secs),
    );
    let kugou_synced = kugou_raw.and_then(|lines| usable_synced_lyrics(lines, track));
    let netease_synced = netease_raw.and_then(|lines| usable_synced_lyrics(lines, track));

    if let Some(lines) = kugou_synced.as_ref().filter(|l| has_rich_timing(l)) {
        save_lrc(&lrc_path, lines).await;
        persist_lyrics(cache_db, track, Some(lines), None, "FOUND");
        return LyricsState::Synced(lines.clone());
    }
    if let Some(lines) = netease_synced.as_ref().filter(|l| has_rich_timing(l)) {
        save_lrc(&lrc_path, lines).await;
        persist_lyrics(cache_db, track, Some(lines), None, "FOUND");
        return LyricsState::Synced(lines.clone());
    }

    if let Some(lines) = cached_synced_fallback {
        mark_word_upgrade_checked(&cache_key, &check_path).await;
        persist_lyrics(cache_db, track, Some(&lines), None, "FOUND");
        return LyricsState::Synced(lines);
    }

    if let Some(lines) = ttml_synced
        .or(lrc_synced)
        .or(kugou_synced)
        .or(netease_synced)
    {
        mark_word_upgrade_checked(&cache_key, &check_path).await;
        save_lrc(&lrc_path, &lines).await;
        persist_lyrics(cache_db, track, Some(&lines), None, "FOUND");
        return LyricsState::Synced(lines);
    }

    // LRCLIB plain lyrics remain useful when no synchronized version exists.
    if let Some(plain) = lrc_plain {
        save_text(&txt_path, &plain.join("\n")).await;
        persist_lyrics(cache_db, track, None, Some(&plain), "FOUND");
        return LyricsState::Plain(plain);
    }

    // YTM native or Genius plain fallback.
    if is_ytm {
        if let Some(raw_text) = ytm.fetch_native_lyrics(&track.id).await {
            if let Some(lines) =
                usable_plain_lyrics(raw_text.lines().map(|line| line.trim().to_string()), track)
            {
                save_text(&txt_path, &lines.join("\n")).await;
                persist_lyrics(cache_db, track, None, Some(&lines), "FOUND");
                return LyricsState::Plain(lines);
            }
        }
        persist_lyrics(cache_db, track, None, None, "NOT_FOUND");
    } else if let Some(lines) = providers::fetch_genius_lyrics(http, &clean_title, artist).await {
        if let Some(lines) = usable_plain_lyrics(lines, track) {
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

fn usable_synced_lyrics(lines: Vec<LyricLine>, track: &Track) -> Option<Vec<LyricLine>> {
    let lines: Vec<_> = lines
        .into_iter()
        .filter(|line| !is_metadata_line(&line.text, track))
        .collect();
    (lines.len() >= 2).then_some(lines)
}

fn usable_plain_lyrics(
    lines: impl IntoIterator<Item = String>,
    track: &Track,
) -> Option<Vec<String>> {
    let lines: Vec<_> = lines
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !is_metadata_line(line, track))
        .collect();
    (lines.len() >= 2).then_some(lines)
}

fn is_metadata_line(line: &str, track: &Track) -> bool {
    let normalized = normalize_lyric_text(line);
    if normalized.is_empty() {
        return true;
    }
    let title = normalize_lyric_text(&sanitize_title(&track.title));
    let artist = normalize_lyric_text(&track.artist);
    if normalized == title || normalized == artist {
        return true;
    }
    [
        "lyrics by",
        "written by",
        "produced by",
        "composed by",
        "music by",
        "artist",
        "title",
        "album",
        "paroles",
        "letra",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
}

fn normalize_lyric_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn cache_paths(artist: &str, title: &str) -> (PathBuf, PathBuf, PathBuf) {
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
        dir.join(format!("{hex}.wordcheck")),
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

    #[test]
    fn rejects_credit_and_track_metadata_from_synced_lyrics() {
        let track = Track {
            id: "track".into(),
            title: "The Emptiness Machine".into(),
            artist: "Linkin Park".into(),
            album: String::new(),
            duration_secs: None,
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
        };
        let lyrics = parse_lrc(
            "[00:01.00] Lyrics by: Someone\n[00:03.00] Linkin Park\n[00:05.00] The Emptiness Machine\n[00:07.00] Real line",
        );
        assert!(usable_synced_lyrics(lyrics, &track).is_none());
    }
}
