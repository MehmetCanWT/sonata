//! Linux artwork decoding, ambient tint extraction, and runtime `now_playing.json` output.
//!
//! 1. **Now Playing Telemetry**: Writes atomic `$XDG_RUNTIME_DIR/sonata/now_playing.json` updates
//!    for Waybar and external status widgets.
//! 2. **Artwork Decoding & Trimming**: Fetches, letterbox-trims, scales, and caches PNG album
//!    artwork on disk while pruning against the configured cache byte budget.
//! 3. **Ambient Tint**: Extracts a vibrant midtone RGB accent color from decoded album covers.
//! 4. **Motion Covers** (`motion`): Downloads and pipes animated album artwork via `ffmpeg`.

pub mod motion;

use crate::config::AppConfig;
use image::DynamicImage;
use reqwest::Client;
use sonata_core::auth::{create_private_file, ensure_private_dir};
use sonata_core::cache::prune_media_cache_in;
use sonata_core::paths::CorePaths;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub use sonata_core::media::{
    art_cache_path_in, build_now_playing_json, extract_apple_motion_video_url, format_mm_ss,
    motion_cache_key_hex, resolve_track_cover, select_fast_hls_variant_url, upscale_artwork_url,
    ExportedLyricLine, ExportedLyricWord, LyricsExport, NowPlayingArtwork, NowPlayingExport,
    NowPlayingInput, NowPlayingLyricLine, NowPlayingLyricPreview, NowPlayingLyricWord,
    NowPlayingLyrics, NowPlayingPlayback, NOW_PLAYING_SCHEMA_VERSION,
};

const MAX_ARTWORK_BYTES: usize = 16 * 1024 * 1024;

pub fn now_playing_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(AppConfig::cache_dir)
        .join("sonata")
        .join("now_playing.json")
}

pub fn lyrics_export_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(AppConfig::cache_dir)
        .join("sonata")
        .join("lyrics.json")
}

pub fn path_to_file_url(path: &Path) -> Option<String> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let raw = abs.to_str()?;
    let mut encoded = String::with_capacity(raw.len() + 8);
    encoded.push_str("file://");
    for byte in raw.as_bytes() {
        if matches!(
            *byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~'
        ) {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    Some(encoded)
}

fn write_atomic_pretty_json<T: serde::Serialize>(path: PathBuf, payload: &T) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec_pretty(payload) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn write_now_playing_json(payload: &serde_json::Value) {
    write_atomic_pretty_json(now_playing_path(), payload);
}

pub fn write_now_playing(payload: &NowPlayingExport) {
    write_atomic_pretty_json(now_playing_path(), payload);
}

pub fn write_lyrics_export(payload: &LyricsExport) {
    write_atomic_pretty_json(lyrics_export_path(), payload);
}

pub fn clear_now_playing() {
    let _ = std::fs::remove_file(now_playing_path());
    let _ = std::fs::remove_file(lyrics_export_path());
}

pub fn art_cache_path(url: &str) -> PathBuf {
    art_cache_path_in(&AppConfig::default_core_paths(), url)
}

/// Trims symmetric near-black pillarbox (left/right) or letterbox (top/bottom) bars commonly baked
/// into YouTube 16:9 (`480x270`) and 4:3 (`480x360`) video thumbnails while preserving full-bleed
/// square and naturally dark covers.
pub fn trim_black_bars(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w < 80 || h < 80 {
        return img;
    }
    let rgb = img.to_rgb8();

    let is_black_col = |x: u32| -> bool {
        let y_start = h / 10;
        let y_end = h.saturating_sub(h / 10).max(y_start + 1);
        let samples = 24u32.min(y_end - y_start).max(1);
        let mut dark = 0u32;
        for i in 0..samples {
            let y = y_start + (i * (y_end - y_start)) / samples;
            let [r, g, b] = rgb.get_pixel(x, y).0;
            if r <= 14 && g <= 14 && b <= 14 {
                dark += 1;
            }
        }
        dark * 100 >= samples * 92
    };

    let is_black_row = |y: u32| -> bool {
        let x_start = w / 10;
        let x_end = w.saturating_sub(w / 10).max(x_start + 1);
        let samples = 24u32.min(x_end - x_start).max(1);
        let mut dark = 0u32;
        for i in 0..samples {
            let x = x_start + (i * (x_end - x_start)) / samples;
            let [r, g, b] = rgb.get_pixel(x, y).0;
            if r <= 14 && g <= 14 && b <= 14 {
                dark += 1;
            }
        }
        dark * 100 >= samples * 92
    };

    let mut x0 = 0u32;
    let mut x1 = w;
    if w > h + 8 {
        let max_side = w / 3;
        let mut left = 0u32;
        while left < max_side && is_black_col(left) {
            left += 1;
        }
        let mut right = 0u32;
        while right < max_side && is_black_col(w - 1 - right) {
            right += 1;
        }
        let min_bar = (w / 16).max(8);
        if left >= min_bar && right >= min_bar {
            let remaining = w.saturating_sub(left + right);
            if remaining >= (h * 3) / 4 {
                x0 = left;
                x1 = w - right;
            }
        }
    }

    let mut y0 = 0u32;
    let mut y1 = h;
    let max_vert = h / 4;
    let mut top = 0u32;
    while top < max_vert && is_black_row(top) {
        top += 1;
    }
    let mut bottom = 0u32;
    while bottom < max_vert && is_black_row(h - 1 - bottom) {
        bottom += 1;
    }
    let min_vbar = (h / 14).max(8);
    if top >= min_vbar && bottom >= min_vbar {
        let remaining = h.saturating_sub(top + bottom);
        let cw = x1 - x0;
        if remaining >= (cw * 2) / 5 {
            y0 = top;
            y1 = h - bottom;
        }
    }

    let cw = x1.saturating_sub(x0);
    let ch = y1.saturating_sub(y0);
    if (x0 == 0 && y0 == 0 && cw == w && ch == h) || cw < 40 || ch < 40 {
        return img;
    }

    // Verify the interior has visible content so pure-black artworks are left untouched.
    let mut lit = 0u32;
    let total = 16u32;
    for row in 0..4u32 {
        for col in 0..4u32 {
            let sx = x0 + ((col + 1) * cw) / 5;
            let sy = y0 + ((row + 1) * ch) / 5;
            let [r, g, b] = rgb.get_pixel(sx.min(w - 1), sy.min(h - 1)).0;
            if r.max(g).max(b) > 28 {
                lit += 1;
            }
        }
    }
    if lit * 4 < total {
        return img;
    }

    img.crop_imm(x0, y0, cw, ch)
}

/// Checks an already-cached non-square PNG on disk and atomically trims baked black pillarbox or
/// letterbox bars if present. Returns `true` when the file was updated.
pub async fn repair_cached_art_if_bars(path: PathBuf) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok((w, h)) = image::image_dimensions(&path) else {
            return false;
        };
        if !(w > h + 8 || (w == 480 && h == 360)) {
            return false;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            return false;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            return false;
        };
        let trimmed = trim_black_bars(decoded);
        if trimmed.width() == w && trimmed.height() == h {
            return false;
        }
        let is_wide_banner = (trimmed.width() as f32) > (trimmed.height() as f32 * 1.45);
        let scaled = if is_wide_banner {
            if trimmed.width() > 1600 || trimmed.height() > 900 {
                trimmed.resize(1600, 900, image::imageops::FilterType::Lanczos3)
            } else {
                trimmed
            }
        } else if trimmed.width() > 480 || trimmed.height() > 480 {
            trimmed.resize(480, 480, image::imageops::FilterType::Lanczos3)
        } else {
            trimmed
        };
        let mut png_bytes = Vec::new();
        if scaled
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .is_err()
        {
            return false;
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let temp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
        if std::fs::write(&temp_path, &png_bytes).is_ok() {
            if std::fs::rename(&temp_path, &path).is_ok() {
                return true;
            }
            let _ = std::fs::remove_file(&temp_path);
        }
        false
    })
    .await
    .unwrap_or(false)
}

/// Reads an already-cached artwork file on disk and returns its ambient RGB tint, if readable.
pub async fn cached_art_ambient_rgb(path: PathBuf) -> Option<(u8, u8, u8)> {
    tokio::task::spawn_blocking(move || {
        image::open(&path).ok().map(|img| extract_ambient_rgb(&img))
    })
    .await
    .ok()
    .flatten()
}

/// Decodes, trims, resizes, and optionally caches raw artwork bytes to `paths.art_cache_dir()`.
pub async fn decode_and_cache_art_bytes(
    raw_bytes: Vec<u8>,
    url: &str,
    paths: &CorePaths,
    cache_limit_bytes: u64,
) -> Option<(DynamicImage, Vec<u8>, PathBuf)> {
    let cache_path = art_cache_path_in(paths, url);
    let cache_enabled = cache_limit_bytes > 0;
    let paths_for_prune = paths.clone();

    tokio::task::spawn_blocking(move || {
        let decoded = trim_black_bars(image::load_from_memory(&raw_bytes).ok()?);
        let is_wide_banner = (decoded.width() as f32) > (decoded.height() as f32 * 1.45);
        let scaled = if is_wide_banner {
            if decoded.width() > 1600 || decoded.height() > 900 {
                decoded.resize(1600, 900, image::imageops::FilterType::Lanczos3)
            } else {
                decoded
            }
        } else if decoded.width() > 480 || decoded.height() > 480 {
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
                let _ = ensure_private_dir(parent);
            }
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let temp_path =
                cache_path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
            let wrote = create_private_file(&temp_path)
                .and_then(|mut f| {
                    f.write_all(&png_bytes)?;
                    f.sync_all()
                })
                .is_ok();
            if wrote {
                if std::fs::rename(&temp_path, &cache_path).is_ok() {
                    prune_media_cache_in(&paths_for_prune, cache_limit_bytes);
                } else {
                    let _ = std::fs::remove_file(&temp_path);
                }
            } else {
                let _ = std::fs::remove_file(&temp_path);
            }
        }
        Some((scaled, png_bytes, cache_path))
    })
    .await
    .ok()
    .flatten()
}

pub async fn fetch_and_decode_art(
    http: &Client,
    url: &str,
    paths: &CorePaths,
    cache_limit_bytes: u64,
) -> Option<(DynamicImage, Vec<u8>)> {
    let cache_path = art_cache_path_in(paths, url);
    let cache_enabled = cache_limit_bytes > 0;

    let expects_hd_banner = url.contains("=w1600") || url.contains("=w1440");
    if cache_enabled {
        if let Ok(disk_bytes) = tokio::fs::read(&cache_path).await {
            if let Ok(Some((img, disk_bytes))) = tokio::task::spawn_blocking(move || {
                image::load_from_memory(&disk_bytes)
                    .ok()
                    .map(|img| (img, disk_bytes))
            })
            .await
            {
                // Self-heal previously downscaled 480px wide artist banners automatically.
                if !expects_hd_banner || img.width() > 640 {
                    return Some((img, disk_bytes));
                }
            }
        }
    }

    let mut resp = http.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if resp
        .content_length()
        .is_some_and(|len| len > MAX_ARTWORK_BYTES as u64)
    {
        return None;
    }
    let mut raw_bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        if raw_bytes.len().saturating_add(chunk.len()) > MAX_ARTWORK_BYTES {
            return None;
        }
        raw_bytes.extend_from_slice(&chunk);
    }
    if raw_bytes.is_empty() {
        return None;
    }

    decode_and_cache_art_bytes(raw_bytes, url, paths, cache_limit_bytes)
        .await
        .map(|(img, bytes, _)| (img, bytes))
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
    use image::{ImageBuffer, Rgb};

    #[test]
    fn extracts_vibrant_ambient_rgb_and_handles_monochrome() {
        let vibrant = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(16, 16, Rgb([210, 75, 130])));
        let rgb = extract_ambient_rgb(&vibrant);
        assert!(rgb.0 > 180 && rgb.2 > 100 && rgb.1 < 100, "got {rgb:?}");

        let black = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(16, 16, Rgb([0, 0, 0])));
        assert_eq!(extract_ambient_rgb(&black), (110, 90, 170));
    }

    #[test]
    fn trims_symmetric_black_pillarboxes_without_touching_full_bleed_covers() {
        // 480x270 thumbnail with a bright 270x270 center square and black pillarboxes.
        let mut pillarboxed = image::RgbImage::from_pixel(480, 270, image::Rgb([0, 0, 0]));
        for y in 0..270 {
            for x in 105..375 {
                pillarboxed.put_pixel(x, y, image::Rgb([180, 120, 220]));
            }
        }
        let cropped = trim_black_bars(DynamicImage::ImageRgb8(pillarboxed));
        assert_eq!(cropped.width(), 270);
        assert_eq!(cropped.height(), 270);

        // Full-bleed dark-border square cover should remain untouched.
        let square = image::RgbImage::from_pixel(480, 480, image::Rgb([40, 30, 50]));
        let kept = trim_black_bars(DynamicImage::ImageRgb8(square));
        assert_eq!(kept.width(), 480);
        assert_eq!(kept.height(), 480);
    }

    #[tokio::test]
    async fn fetch_and_decode_art_rejects_oversized_streaming_response() {
        use crate::downloads::test_support::TempTestDir;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut req_buf = [0u8; 1024];
                let _ = sock.read(&mut req_buf).await;
                let _ = sock
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                let chunk = vec![0u8; 256 * 1024];
                let header = format!("{:x}\r\n", chunk.len());
                for _ in 0..66 {
                    if sock.write_all(header.as_bytes()).await.is_err()
                        || sock.write_all(&chunk).await.is_err()
                        || sock.write_all(b"\r\n").await.is_err()
                    {
                        break;
                    }
                }
                let _ = sock.write_all(b"0\r\n\r\n").await;
            }
        });

        let dir = TempTestDir::new("art_size_cap");
        let paths = CorePaths::for_app_roots(dir.path(), dir.path());
        let http = sonata_core::providers::build_http_client();
        let res = fetch_and_decode_art(&http, &format!("http://{addr}/huge.png"), &paths, 0).await;
        assert!(res.is_none());
        server.abort();
    }
}
