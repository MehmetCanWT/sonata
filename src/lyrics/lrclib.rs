use crate::config::AppConfig;
use crate::lyrics::{parse_lrc, providers, sanitize_title, LyricLine, LyricsState};
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use reqwest::Client;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub async fn fetch_lyrics(http: &Client, ytm: &YtMusicClient, track: &Track) -> LyricsState {
    let clean_title = sanitize_title(&track.title);
    let artist = track.artist.trim();
    let (lrc_path, txt_path) = cache_paths(artist, &clean_title);

    if let Ok(cached) = fs::read_to_string(&lrc_path) {
        let parsed = parse_lrc(&cached);
        if !parsed.is_empty() {
            return LyricsState::Synced(parsed);
        }
    }
    if let Ok(cached_txt) = fs::read_to_string(&txt_path) {
        let plain: Vec<String> = cached_txt.lines().map(str::to_string).collect();
        if !plain.is_empty() {
            return LyricsState::Plain(plain);
        }
    }

    // 1. LRCLIB (Synced preferred; keep Plain only if synced fallbacks miss)
    let mut plain_fallback: Option<Vec<String>> = None;
    if let Some((synced, plain)) = query_lrclib(http, artist, &clean_title, track).await {
        if let Some(lines) = synced {
            save_lrc(&lrc_path, &lines);
            return LyricsState::Synced(lines);
        }
        plain_fallback = plain;
    }

    // 2. KuGou (time-synced LRC fallback)
    if let Some(lines) =
        providers::fetch_kugou_synced(http, &clean_title, artist, track.duration_secs).await
    {
        save_lrc(&lrc_path, &lines);
        return LyricsState::Synced(lines);
    }

    // 3. LRCLIB plain lyrics if available
    if let Some(plain) = plain_fallback {
        let _ = fs::write(&txt_path, plain.join("\n"));
        return LyricsState::Plain(plain);
    }

    // 4. YouTube Music native lyrics tab
    if track.source == ProviderSource::YouTubeMusic {
        if let Some(raw_text) = ytm.fetch_native_lyrics(&track.id).await {
            let lines: Vec<String> = raw_text.lines().map(|l| l.trim().to_string()).collect();
            if !lines.is_empty() {
                let _ = fs::write(&txt_path, lines.join("\n"));
                return LyricsState::Plain(lines);
            }
        }
    }

    // 5. Genius web scraper last resort
    if let Some(lines) = providers::fetch_genius_lyrics(http, &clean_title, artist).await {
        let _ = fs::write(&txt_path, lines.join("\n"));
        return LyricsState::Plain(lines);
    }

    LyricsState::NotFound
}

async fn query_lrclib(
    http: &Client,
    artist: &str,
    clean_title: &str,
    track: &Track,
) -> Option<(Option<Vec<LyricLine>>, Option<Vec<String>>)> {
    if !artist.is_empty() && !clean_title.is_empty() {
        let mut req = http
            .get("https://lrclib.net/api/get")
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
        .query(&[("q", q.as_str())])
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let arr = resp.json::<Vec<Value>>().await.ok()?;
    for item in &arr {
        let (synced, _) = parse_lrclib_item(item);
        if synced.is_some() {
            return Some((synced, None));
        }
    }
    arr.first().map(parse_lrclib_item)
}

fn parse_lrclib_item(item: &Value) -> (Option<Vec<LyricLine>>, Option<Vec<String>>) {
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

fn save_lrc(path: &PathBuf, lines: &[LyricLine]) {
    let body: String = lines
        .iter()
        .map(|l| {
            let mins = (l.time_secs / 60.0).floor() as u64;
            let secs = l.time_secs - (mins as f64 * 60.0);
            format!("[{mins:02}:{secs:05.2}] {}\n", l.text)
        })
        .collect();
    let _ = fs::write(path, body);
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
