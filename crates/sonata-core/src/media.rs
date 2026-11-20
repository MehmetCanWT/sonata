//! External now-playing telemetry, high-resolution artwork resolution, and ambient tinting for Sonata.
//!
//! 1. **Now Playing JSON**: serializes playback metadata, lyric line, and art path into
//!    `$XDG_RUNTIME_DIR/sonata/now_playing.json`.
//! 2. **Cover Resolution**: `resolve_track_cover` queries Deezer, iTunes, and upscaled provider
//!    thumbnails (`upscale_artwork_url`) for both YouTube Music and Spotify tracks.
//! 3. **Artwork Caching & Scaling**: `fetch_and_decode_art` caches 480px PNG artwork on disk and
//!    prunes the media cache against the configured byte limit.
//! 4. **Ambient Color Extraction**: `extract_ambient_rgb` computes a saturated midtone accent color
//!    from album artwork.

use crate::cache::{prune_media_cache, CacheDb};
use crate::config::AppConfig;
use crate::providers::{ProviderSource, Track};
use image::DynamicImage;
use regex::Regex;
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::path::PathBuf;

pub fn now_playing_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(AppConfig::cache_dir)
        .join("sonata")
        .join("now_playing.json")
}

pub fn write_now_playing_json(payload: &serde_json::Value) {
    let path = now_playing_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec(payload) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn clear_now_playing() {
    let _ = std::fs::remove_file(now_playing_path());
}

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
    serde_json::json!({
        "status": status,
        "title": track.map(|t| t.title.as_str()),
        "artist": track.map(|t| t.artist.as_str()),
        "album": track.map(|t| t.album.as_str()).filter(|s| !s.is_empty()),
        "source": track.map(|t| t.source.short_tag()),
        "quality": if track.is_some() && !quality.is_empty() { Some(quality) } else { None },
        "position_secs": position_secs,
        "duration_secs": duration_secs,
        "position": format_mm_ss(position_secs),
        "duration": format_mm_ss(duration_secs),
        "lyric": lyric,
        "art_path": art_path,
        "art_url": art_url,
        "updated_at": updated_at,
    })
}

pub fn art_cache_path(url: &str) -> PathBuf {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    AppConfig::art_cache_dir().join(format!("{hex}.png"))
}

/// Resolve one high-resolution cover and persist its source for future plays.
pub async fn resolve_track_cover(
    http: &Client,
    cache_db: Option<&CacheDb>,
    track: &Track,
) -> Option<(String, String)> {
    let fallback_source = match track.source {
        ProviderSource::YouTubeMusic => "ytm",
        ProviderSource::Spotify => "spotify",
    };
    if let Some(metadata) = cache_db.and_then(|db| db.get_track_enrichment(&track.id)) {
        if matches!(
            metadata.cover_source.as_str(),
            "deezer" | "apple" | "ytm" | "spotify"
        ) {
            if let Some(url) = metadata.cover_url.filter(|url| !is_legacy_cover(url)) {
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
                .filter(|url| !is_legacy_cover(url))
                .map(upscale_artwork_url)?;
            (url, fallback_source)
        }
    } else {
        let url = track
            .thumbnail_url
            .as_deref()
            .filter(|url| !is_legacy_cover(url))
            .map(upscale_artwork_url)?;
        (url, fallback_source)
    };

    if let Some(cache_db) = cache_db {
        cache_db.set_track_cover(&track.id, &url, source);
    }
    Some((url, source.to_string()))
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

/// Upgrades provider thumbnail URLs (YouTube/Google, Spotify, iTunes) to high resolution.
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

fn is_legacy_cover(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("saavncdn") || lower.contains("jiosaavn")
}

pub async fn fetch_and_decode_art(http: &Client, url: &str) -> Option<(DynamicImage, Vec<u8>)> {
    let cache_path = art_cache_path(url);
    let cache_enabled = AppConfig::active_cache_limit_bytes() > 0;

    if cache_enabled {
        if let Ok(disk_bytes) = tokio::fs::read(&cache_path).await {
            let bytes_clone = disk_bytes.clone();
            if let Ok(Some(img)) =
                tokio::task::spawn_blocking(move || image::load_from_memory(&bytes_clone).ok())
                    .await
            {
                return Some((img, disk_bytes));
            }
        }
    }

    let resp = http.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let raw_bytes = resp.bytes().await.ok()?.to_vec();

    tokio::task::spawn_blocking(move || {
        let decoded = image::load_from_memory(&raw_bytes).ok()?;
        let scaled = if decoded.width() > 480 || decoded.height() > 480 {
            decoded.resize(480, 480, image::imageops::FilterType::Lanczos3)
        } else {
            decoded
        };
        let mut png_bytes = Vec::new();
        scaled
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .ok()?;
        if cache_enabled {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let temp_path =
                cache_path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
            if std::fs::write(&temp_path, &png_bytes).is_ok() {
                if std::fs::rename(&temp_path, &cache_path).is_ok() {
                    prune_media_cache(AppConfig::active_cache_limit_bytes());
                } else {
                    let _ = std::fs::remove_file(&temp_path);
                }
            }
        }
        Some((scaled, png_bytes))
    })
    .await
    .ok()
    .flatten()
}

/// Extracts a pleasant ambient tint color from album artwork, ignoring pure black/white pixels.
pub fn extract_ambient_rgb(img: &DynamicImage) -> (u8, u8, u8) {
    let small = img.resize_exact(16, 16, image::imageops::FilterType::Triangle);
    let rgb = small.to_rgb8();

    let mut weighted_r = 0.0_f64;
    let mut weighted_g = 0.0_f64;
    let mut weighted_b = 0.0_f64;
    let mut total_weight = 0.0_f64;

    for pixel in rgb.pixels() {
        let [r, g, b] = pixel.0;
        let rf = r as f64 / 255.0;
        let gf = g as f64 / 255.0;
        let bf = b as f64 / 255.0;

        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);
        let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;

        if !(0.12..=0.86).contains(&luma) {
            continue;
        }
        let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
        if sat < 0.12 {
            continue;
        }

        // Strongly favor vibrant saturated midtones for pleasing Apple Music-style ambient glow
        let mid_bias = 1.0 - (luma - 0.48).abs();
        let weight = sat * sat * mid_bias.max(0.2);
        weighted_r += r as f64 * weight;
        weighted_g += g as f64 * weight;
        weighted_b += b as f64 * weight;
        total_weight += weight;
    }

    if total_weight > 0.05 {
        (
            (weighted_r / total_weight).clamp(0.0, 255.0).round() as u8,
            (weighted_g / total_weight).clamp(0.0, 255.0).round() as u8,
            (weighted_b / total_weight).clamp(0.0, 255.0).round() as u8,
        )
    } else {
        (110, 90, 170)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderSource;
    use image::{ImageBuffer, Rgb};

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
    fn extracts_vibrant_ambient_rgb_and_handles_monochrome() {
        let vibrant = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(16, 16, Rgb([210, 75, 130])));
        let rgb = extract_ambient_rgb(&vibrant);
        assert!(rgb.0 > 180 && rgb.2 > 100 && rgb.1 < 100, "got {rgb:?}");

        let black = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(16, 16, Rgb([0, 0, 0])));
        assert_eq!(extract_ambient_rgb(&black), (110, 90, 170));
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
}
