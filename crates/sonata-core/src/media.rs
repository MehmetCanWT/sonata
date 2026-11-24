//! Portable artwork resolution, URL upscaling, and now-playing metadata formatting for Sonata.
//!
//! 1. **Now Playing Payload**: Serializes playback metadata, active lyric line, and artwork links
//!    via [`build_now_playing_json`].
//! 2. **Cover Resolution**: [`resolve_track_cover`] queries Deezer, iTunes, and upscaled provider
//!    thumbnails ([`upscale_artwork_url`]) for YouTube Music and Spotify tracks.
//! 3. **Cache Keys & Motion Discovery Helpers**: Computes deterministic artwork/motion cache keys
//!    and extracts Apple Music animated artwork URLs from HTML and HLS playlists.

use crate::cache::CacheDb;
use crate::paths::CorePaths;
use crate::providers::{ProviderSource, Track};
use regex::Regex;
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::path::PathBuf;

pub fn format_mm_ss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[allow(clippy::too_many_arguments)]
pub fn build_now_playing_json(
    status: &str,
    track: Option<&Track>,
    quality: &str,
    position_secs: u64,
    duration_secs: u64,
    lyric: Option<&str>,
    art_path: Option<&str>,
    art_url: Option<&str>,
) -> serde_json::Value {
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let remaining_secs = duration_secs.saturating_sub(position_secs);
    let progress_ratio = if duration_secs > 0 {
        (position_secs as f64 / duration_secs as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let progress_percent = position_secs
        .saturating_mul(100)
        .checked_div(duration_secs)
        .unwrap_or(0)
        .min(100);
    let status_lower = status.to_ascii_lowercase();
    let text = match track {
        Some(t) if !t.artist.trim().is_empty() => {
            format!("{} — {}", t.artist.trim(), t.title.trim())
        }
        Some(t) => t.title.trim().to_string(),
        None => "Sonata".to_string(),
    };
    let mut tooltip_lines = Vec::new();
    if let Some(t) = track {
        tooltip_lines.push(t.title.clone());
        if !t.artist.trim().is_empty() {
            tooltip_lines.push(format!("Artist: {}", t.artist.trim()));
        }
        if !t.album.trim().is_empty() {
            tooltip_lines.push(format!("Album: {}", t.album.trim()));
        }
        if !quality.is_empty() {
            tooltip_lines.push(format!("Quality: {quality}"));
        }
        tooltip_lines.push(format!(
            "Time: {} / {}",
            format_mm_ss(position_secs),
            format_mm_ss(duration_secs)
        ));
        if let Some(l) = lyric.filter(|s| !s.trim().is_empty()) {
            tooltip_lines.push(format!("♪ {}", l.trim()));
        }
    } else {
        tooltip_lines.push(format!("Sonata ({status})"));
    }
    let tooltip = tooltip_lines.join("\n");

    serde_json::json!({
        "status": status,
        "track_id": track.map(|t| t.id.as_str()),
        "title": track.map(|t| t.title.as_str()),
        "artist": track.map(|t| t.artist.as_str()),
        "album": track.map(|t| t.album.as_str()).filter(|s| !s.is_empty()),
        "source": track.map(|t| t.source.short_tag()),
        "quality": if track.is_some() && !quality.is_empty() { Some(quality) } else { None },
        "is_playing": status == "Playing",
        "is_paused": status == "Paused",
        "liked": track.map(|t| t.liked).unwrap_or(false),
        "position_secs": position_secs,
        "duration_secs": duration_secs,
        "remaining_secs": remaining_secs,
        "position": format_mm_ss(position_secs),
        "duration": format_mm_ss(duration_secs),
        "remaining": format!("-{}", format_mm_ss(remaining_secs)),
        "progress_ratio": progress_ratio,
        "progress_percent": progress_percent,
        "lyric": lyric,
        "art_path": art_path,
        "art_url": art_url,
        "text": text,
        "alt": status_lower,
        "class": status_lower,
        "tooltip": tooltip,
        "updated_at": updated_at,
    })
}

pub fn art_cache_path_in(paths: &CorePaths, url: &str) -> PathBuf {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    paths.art_cache_dir().join(format!("{hex}.png"))
}

/// Resolve one high-resolution cover and persist its source for future plays.
pub async fn resolve_track_cover(
    http: &Client,
    cache_db: Option<&CacheDb>,
    track: &Track,
) -> Option<(String, String)> {
    let is_jio = track.is_jiosaavn();
    let fallback_source = if is_jio {
        "jiosaavn"
    } else {
        match track.source {
            ProviderSource::YouTubeMusic => "ytm",
            ProviderSource::Spotify => "spotify",
        }
    };
    let cached_meta = if let Some(db) = cache_db {
        let db = db.clone();
        let track_id = track.id.clone();
        tokio::task::spawn_blocking(move || db.get_track_enrichment(&track_id))
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    if let Some(metadata) = cached_meta {
        if matches!(
            metadata.cover_source.as_str(),
            "deezer" | "apple" | "ytm" | "spotify" | "jiosaavn"
        ) {
            if let Some(url) = metadata
                .cover_url
                .filter(|url| is_jio || !is_legacy_cover(url))
            {
                return Some((upscale_artwork_url(&url), metadata.cover_source));
            }
        }
    }

    let has_meta = !track.artist.trim().is_empty() && !track.title.trim().is_empty();
    let (url, source) = if has_meta {
        if let Some(url) = fetch_deezer_cover(http, track).await {
            (url, "deezer")
        } else if let Some(url) = fetch_itunes_cover(http, track).await {
            (url, "apple")
        } else {
            let url = track
                .thumbnail_url
                .as_deref()
                .filter(|url| is_jio || !is_legacy_cover(url))
                .map(upscale_artwork_url)?;
            (url, fallback_source)
        }
    } else {
        let url = track
            .thumbnail_url
            .as_deref()
            .filter(|url| is_jio || !is_legacy_cover(url))
            .map(upscale_artwork_url)?;
        (url, fallback_source)
    };

    if let Some(cache_db) = cache_db {
        let db = cache_db.clone();
        let track_id = track.id.clone();
        let cover_url = url.clone();
        let _ = tokio::task::spawn_blocking(move || {
            db.set_track_cover(&track_id, &cover_url, source);
        })
        .await;
    }
    Some((url, source.to_string()))
}

pub fn motion_cache_key_hex(track: &Track) -> String {
    let clean_title = clean_cover_title(&track.title);
    let mut hasher = Sha1::new();
    hasher.update(
        format!("{}::{}", track.artist.trim(), clean_title)
            .to_lowercase()
            .as_bytes(),
    );
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Extracts an Apple Music animated album artwork (`editorialVideo` / `mvod`) HLS or MP4 URL
/// from an Apple Music album page HTML payload.
pub fn extract_apple_motion_video_url(html: &str) -> Option<String> {
    static RE_MVOD: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_MEDIA: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re_mvod = RE_MVOD.get_or_init(|| {
        Regex::new(r#"https://[^"'\s<>\\]+mvod\.itunes\.apple\.com[^"'\s<>\\]+\.(?:m3u8|mp4)(?:\?[^"'\s<>\\]*)?"#)
            .expect("mvod regex")
    });
    let re_media = RE_MEDIA.get_or_init(|| {
        Regex::new(r#"https://[^"'\s<>\\]+\.(?:m3u8|mp4)(?:\?[^"'\s<>\\]*)?"#).expect("media regex")
    });

    let normalized = html.replace("\\/", "/");
    if let Some(m) = re_mvod.find(&normalized) {
        return Some(m.as_str().to_string());
    }

    for marker in ["editorialVideo", "motionArtwork", "motion"] {
        let mut cursor = 0usize;
        while let Some(rel) = normalized[cursor..].find(marker) {
            let start = cursor + rel;
            let end = normalized.floor_char_boundary(normalized.len().min(start + 1500));
            let window = &normalized[start..end];
            if let Some(m) = re_media.find(window) {
                return Some(m.as_str().to_string());
            }
            cursor = start + marker.len();
        }
    }
    None
}

/// Selects a compact H.264 (`avc1`, ~360x360..540x540) variant stream URL from an Apple Music
/// master HLS `.m3u8` playlist so `ffmpeg` does not probe 20+ 4K/HEVC variants.
pub fn select_fast_hls_variant_url(master_m3u8: &str) -> Option<String> {
    let mut pending_inf: Option<&str> = None;
    let mut preferred_avc: Option<(u32, String)> = None;
    let mut any_avc: Option<String> = None;
    let mut any_stream: Option<String> = None;

    for raw_line in master_m3u8.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            pending_inf = Some(rest);
            continue;
        }
        if line.starts_with('#') {
            pending_inf = None;
            continue;
        }
        let Some(inf) = pending_inf.take() else {
            continue;
        };
        if !line.starts_with("https://") || line.ends_with("_iframes.m3u8") {
            continue;
        }

        let is_avc = inf.contains("avc1");
        let res_dim = parse_m3u8_resolution(inf);

        if is_avc {
            if let Some((w, h)) = res_dim {
                if (360..=540).contains(&w) && (360..=540).contains(&h) {
                    let dist = w.abs_diff(480);
                    if preferred_avc
                        .as_ref()
                        .is_none_or(|(best_d, _)| dist < *best_d)
                    {
                        preferred_avc = Some((dist, line.to_string()));
                    }
                }
            }
            if any_avc.is_none() {
                any_avc = Some(line.to_string());
            }
        }
        if any_stream.is_none() {
            any_stream = Some(line.to_string());
        }
    }

    preferred_avc.map(|(_, url)| url).or(any_avc).or(any_stream)
}

fn parse_m3u8_resolution(inf: &str) -> Option<(u32, u32)> {
    let idx = inf.find("RESOLUTION=")?;
    let after = &inf[idx + "RESOLUTION=".len()..];
    let token = after.split([',', ' ']).next()?;
    let (w_str, h_str) = token.split_once('x')?;
    Some((w_str.parse().ok()?, h_str.parse().ok()?))
}

pub(crate) async fn fetch_apple_album_urls(http: &Client, track: &Track) -> Vec<String> {
    let clean_title = clean_cover_title(&track.title);
    let query = format!("{} {clean_title}", track.artist.trim());
    let Ok(response) = http
        .get("https://itunes.apple.com/search")
        .query(&[("term", query.as_str()), ("entity", "song"), ("limit", "8")])
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
    else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(payload) = response.json::<serde_json::Value>().await else {
        return Vec::new();
    };
    let Some(results) = payload.get("results").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };

    // Match by artist + title similarity without strict duration cutoff so YTM video duration differences don't drop the album.
    let mut matched: Vec<(bool, String)> = results
        .iter()
        .filter(|candidate| itunes_candidate_matches(candidate, track, None))
        .filter_map(|candidate| {
            let raw_url = candidate
                .get("collectionViewUrl")
                .and_then(serde_json::Value::as_str)?;
            let clean_url = raw_url
                .split('?')
                .next()
                .unwrap_or(raw_url)
                .replace("itunes.apple.com", "music.apple.com");
            let collection_name = candidate
                .get("collectionName")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let is_single =
                collection_name.ends_with(" - Single") || collection_name.ends_with(" - EP");
            Some((is_single, clean_url))
        })
        .collect();

    // Prefer full albums over singles.
    matched.sort_by_key(|(is_single, _)| *is_single);
    let mut urls = Vec::new();
    for (_, url) in matched {
        if !urls.contains(&url) {
            urls.push(url);
            if urls.len() >= 3 {
                break;
            }
        }
    }
    urls
}

async fn fetch_deezer_cover(http: &Client, track: &Track) -> Option<String> {
    let clean_title = clean_cover_title(&track.title);
    let query = format!("{} {clean_title}", track.artist.trim());
    let response = http
        .get("https://api.deezer.com/search/track")
        .query(&[("q", query.as_str()), ("limit", "5")])
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload: serde_json::Value = response.json().await.ok()?;
    let results = payload.get("data")?.as_array()?;
    results
        .iter()
        .filter(|candidate| deezer_candidate_matches(candidate, track))
        .find_map(|candidate| {
            let album = candidate.get("album")?;
            album
                .get("cover_xl")
                .or_else(|| album.get("cover_big"))
                .and_then(serde_json::Value::as_str)
                .filter(|url| !url.trim().is_empty())
                .map(str::to_string)
        })
}

fn deezer_candidate_matches(candidate: &serde_json::Value, track: &Track) -> bool {
    if let (Some(req_dur), Some(cand_dur)) = (
        track.duration_secs,
        candidate
            .get("duration")
            .and_then(serde_json::Value::as_u64),
    ) {
        if cand_dur > 0 && req_dur.abs_diff(cand_dur) > 6 {
            return false;
        }
    }
    let artist = candidate
        .pointer("/artist/name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let title = candidate
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if [artist, title].iter().any(|text| {
        let lower = text.to_ascii_lowercase();
        ["karaoke", "tribute", "ringtone", "made famous"]
            .iter()
            .any(|blocked| lower.contains(blocked))
    }) {
        return false;
    }
    if !modifier_versions_match(&track.title, title) {
        return false;
    }
    similarity(&track.artist, artist) >= 0.75
        && similarity(&clean_cover_title(&track.title), &clean_cover_title(title)) >= 0.80
}

async fn fetch_itunes_cover(http: &Client, track: &Track) -> Option<String> {
    let clean_title = clean_cover_title(&track.title);
    let query = format!("{} {clean_title}", track.artist.trim());
    let response = http
        .get("https://itunes.apple.com/search")
        .query(&[("term", query.as_str()), ("entity", "song"), ("limit", "5")])
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload: serde_json::Value = response.json().await.ok()?;
    let results = payload.get("results")?.as_array()?;
    let duration_ms = track.duration_secs.map(|s| s.saturating_mul(1000));
    results
        .iter()
        .filter(|candidate| itunes_candidate_matches(candidate, track, duration_ms))
        .find_map(|candidate| {
            candidate
                .get("artworkUrl100")
                .and_then(serde_json::Value::as_str)
                .map(upscale_artwork_url)
        })
}

fn itunes_candidate_matches(
    candidate: &serde_json::Value,
    track: &Track,
    duration_ms: Option<u64>,
) -> bool {
    if let Some(expected_ms) = duration_ms {
        let Some(candidate_duration) = candidate
            .get("trackTimeMillis")
            .and_then(serde_json::Value::as_u64)
        else {
            return false;
        };
        if candidate_duration.abs_diff(expected_ms) > 4000 {
            return false;
        }
    }
    let artist = candidate
        .get("artistName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let title = candidate
        .get("trackName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if [artist, title].iter().any(|text| {
        let lower = text.to_ascii_lowercase();
        ["karaoke", "tribute", "ringtone", "made famous"]
            .iter()
            .any(|blocked| lower.contains(blocked))
    }) {
        return false;
    }
    if !modifier_versions_match(&track.title, title) {
        return false;
    }
    similarity(&track.artist, artist) >= 0.75
        && similarity(&clean_cover_title(&track.title), &clean_cover_title(title)) >= 0.80
}

fn clean_cover_title(title: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\[(]\s*(official\s+(audio|video)|lyrics?|visualizer)\s*[\])]")
            .expect("cover title pattern is valid")
    })
    .replace_all(title, " ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

fn modifier_versions_match(source: &str, candidate: &str) -> bool {
    [
        "remix",
        "live",
        "acoustic",
        "instrumental",
        "sped up",
        "slowed",
    ]
    .iter()
    .all(|keyword| contains_phrase(source, keyword) == contains_phrase(candidate, keyword))
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    normalize_for_match(text).contains(&normalize_for_match(phrase))
}

fn similarity(left: &str, right: &str) -> f64 {
    let left = normalize_for_match(left).chars().collect::<Vec<_>>();
    let right = normalize_for_match(right).chars().collect::<Vec<_>>();
    let width = left.len().max(right.len());
    if width == 0 {
        return 0.0;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != right_char);
            current[right_index + 1] = substitution
                .min(previous[right_index + 1] + 1)
                .min(current[right_index] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    1.0 - previous[right.len()] as f64 / width as f64
}

fn normalize_for_match(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_lowercase().next().unwrap_or(character)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Upgrades provider thumbnail URLs (YouTube/Google, Spotify, iTunes, JioSaavn) to high resolution.
pub fn upscale_artwork_url(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if lower.contains("googleusercontent.com") || lower.contains("ggpht.com") {
        let base = if let Some(idx) = url.rfind("=w") {
            &url[..idx]
        } else if let Some(idx) = url.rfind("=s") {
            &url[..idx]
        } else {
            url
        };
        return format!("{base}=w1200-h1200-l90-rj");
    }
    if lower.contains("i.scdn.co/image/") {
        return url
            .replace("ab67616d00001e02", "ab67616d0000b273")
            .replace("ab67616d00004851", "ab67616d0000b273");
    }
    if lower.contains("saavncdn.com") {
        let https = if url.starts_with("http://") {
            url.replacen("http://", "https://", 1)
        } else {
            url.to_string()
        };
        return https
            .replace("150x150", "500x500")
            .replace("50x50", "500x500");
    }
    if url.contains("100x100bb") {
        let upgraded = url.replace("100x100bb", "1200x1200bb");
        return if upgraded.starts_with("http://") {
            upgraded.replacen("http://", "https://", 1)
        } else {
            upgraded
        };
    }
    url.to_string()
}

fn contains_ignore_ascii_case(haystack: &str, needle: &[u8]) -> bool {
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn is_legacy_cover(url: &str) -> bool {
    contains_ignore_ascii_case(url, b"saavncdn") || contains_ignore_ascii_case(url, b"jiosaavn")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;

    #[test]
    fn builds_external_now_playing_json() {
        let track = Track {
            id: "abc".into(),
            title: "Midnight".into(),
            artist: "M83".into(),
            album: "HUWD".into(),
            duration_secs: Some(243),
            source: ProviderSource::YouTubeMusic,
            thumbnail_url: Some("https://img.example/1.jpg".into()),
            audio_badge: None,
            liked: false,
            set_video_id: None,
            album_id: None,
            is_explicit: false,
        };
        let v = build_now_playing_json(
            "Playing",
            Some(&track),
            "OPUS 137kbps · HQ",
            65,
            243,
            Some("Waiting in a car"),
            Some("/tmp/art.png"),
            track.thumbnail_url.as_deref(),
        );
        assert_eq!(v["status"], "Playing");
        assert_eq!(v["title"], "Midnight");
        assert_eq!(v["artist"], "M83");
        assert_eq!(v["position"], "1:05");
        assert_eq!(v["duration"], "4:03");
        assert_eq!(v["lyric"], "Waiting in a car");
        assert_eq!(v["art_path"], "/tmp/art.png");
    }

    #[test]
    fn upscales_google_spotify_and_itunes_artwork_urls() {
        assert_eq!(
            upscale_artwork_url("https://lh3.googleusercontent.com/abc=w120-h120-l90-rj"),
            "https://lh3.googleusercontent.com/abc=w1200-h1200-l90-rj"
        );
        assert_eq!(
            upscale_artwork_url("https://yt3.ggpht.com/xyz=s60-c-k-c0x00ffffff-no-rj"),
            "https://yt3.ggpht.com/xyz=w1200-h1200-l90-rj"
        );
        assert_eq!(
            upscale_artwork_url("https://i.scdn.co/image/ab67616d00001e02deadbeef12345678"),
            "https://i.scdn.co/image/ab67616d0000b273deadbeef12345678"
        );
        assert_eq!(
            upscale_artwork_url("https://i.scdn.co/image/ab67616d00004851deadbeef12345678"),
            "https://i.scdn.co/image/ab67616d0000b273deadbeef12345678"
        );
        assert_eq!(
            upscale_artwork_url("http://is1-ssl.mzstatic.com/image/thumb/Music/100x100bb.jpg"),
            "https://is1-ssl.mzstatic.com/image/thumb/Music/1200x1200bb.jpg"
        );
        assert_eq!(
            upscale_artwork_url("https://cdn-images.dzcdn.net/images/cover/abc/1000x1000.jpg"),
            "https://cdn-images.dzcdn.net/images/cover/abc/1000x1000.jpg"
        );
    }

    #[test]
    fn builds_enriched_now_playing_waybar_fields() {
        let track = Track {
            id: "yt_123".into(),
            title: "Resonance".into(),
            artist: "HOME".into(),
            album: "Odyssey".into(),
            duration_secs: Some(212),
            source: ProviderSource::YouTubeMusic,
            thumbnail_url: None,
            audio_badge: None,
            liked: true,
            set_video_id: None,
            album_id: None,
            is_explicit: false,
        };
        let v = build_now_playing_json(
            "Playing",
            Some(&track),
            "AAC 320kbps",
            106,
            212,
            Some("Into the night"),
            None,
            None,
        );
        assert_eq!(v["track_id"], "yt_123");
        assert_eq!(v["is_playing"], true);
        assert_eq!(v["is_paused"], false);
        assert_eq!(v["liked"], true);
        assert_eq!(v["progress_percent"], 50);
        assert_eq!(v["remaining_secs"], 106);
        assert_eq!(v["remaining"], "-1:46");
        assert_eq!(v["text"], "HOME — Resonance");
        assert_eq!(v["alt"], "playing");
        assert_eq!(v["class"], "playing");
        assert!(v["tooltip"].as_str().unwrap().contains("Odyssey"));
        assert!(v["tooltip"].as_str().unwrap().contains("Into the night"));
    }

    #[test]
    fn extracts_apple_motion_video_url_from_album_html() {
        let mvod_html = r#"<script type="application/json">{"editorialVideo":{"motionDetailSquare":{"video":"https:\/\/mvod.itunes.apple.com\/itunes-assets\/HLSVideo211\/v4\/ab\/cd\/ef\/playlist.m3u8"}}}</script>"#;
        assert_eq!(
            extract_apple_motion_video_url(mvod_html).as_deref(),
            Some(
                "https://mvod.itunes.apple.com/itunes-assets/HLSVideo211/v4/ab/cd/ef/playlist.m3u8"
            )
        );

        let fallback_html = r#"{"editorialVideo":{"motionSquareVideo1x1":{"hls":"https://cdn.apple.com/video/album_motion.m3u8?token=abc"}}}"#;
        assert_eq!(
            extract_apple_motion_video_url(fallback_html).as_deref(),
            Some("https://cdn.apple.com/video/album_motion.m3u8?token=abc")
        );

        assert_eq!(
            extract_apple_motion_video_url("<html><body>no video</body></html>"),
            None
        );
    }

    #[test]
    fn selects_fast_avc1_hls_variant_over_4k_hevc_and_iframes() {
        let master = "#EXTM3U\n\
            #EXT-X-VERSION:6\n\
            #EXT-X-I-FRAME-STREAM-INF:AVERAGE-BANDWIDTH=89715,CODECS=\"avc1.64001f\",RESOLUTION=486x486,URI=\"https://mvod.itunes.apple.com/itunes-assets/486x486_iframes.m3u8\"\n\
            #EXT-X-STREAM-INF:AVERAGE-BANDWIDTH=12400000,CODECS=\"hvc1.2.4.L150.B0\",RESOLUTION=2160x2160,FRAME-RATE=24.000\n\
            https://mvod.itunes.apple.com/itunes-assets/2160x2160_hevc.m3u8\n\
            #EXT-X-STREAM-INF:AVERAGE-BANDWIDTH=780000,CODECS=\"avc1.64001f\",RESOLUTION=486x486,FRAME-RATE=24.000\n\
            https://mvod.itunes.apple.com/itunes-assets/486x486_avc.m3u8\n\
            #EXT-X-STREAM-INF:AVERAGE-BANDWIDTH=2900000,CODECS=\"avc1.640028\",RESOLUTION=1080x1080,FRAME-RATE=24.000\n\
            https://mvod.itunes.apple.com/itunes-assets/1080x1080_avc.m3u8\n";
        assert_eq!(
            select_fast_hls_variant_url(master).as_deref(),
            Some("https://mvod.itunes.apple.com/itunes-assets/486x486_avc.m3u8")
        );
    }

    #[test]
    fn extract_apple_motion_video_url_handles_multibyte_chars_at_window_boundary() {
        // Place multi-byte Turkish, Russian, and emoji characters straddling byte offset `start + 1500`.
        let prefix = "editorialVideo".to_string();
        let filler_len = 1500 - prefix.len() - 1;
        let padding = "a".repeat(filler_len);
        let multibyte_tail = "ğüşöçıЖЯ🎵";

        let no_url_payload = format!("{prefix}{padding}{multibyte_tail}");
        assert_eq!(extract_apple_motion_video_url(&no_url_payload), None);

        let with_url_payload = format!(
            "{prefix} türkçe русский 🎶 https://cdn.apple.com/video/motion.m3u8 {padding}{multibyte_tail}"
        );
        assert_eq!(
            extract_apple_motion_video_url(&with_url_payload).as_deref(),
            Some("https://cdn.apple.com/video/motion.m3u8")
        );
    }
}
