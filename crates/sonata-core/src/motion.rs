//! Apple Music motion artwork (`editorialVideo`) fMP4 downloader and in-memory RGBA8 decoder.
//!
//! 1. **HLS & Byte-Range Discovery**: selects the `768x768` `avc1` variant from the Apple Music
//!    master `.m3u8` (never oversized `1080x1080` or low-res `360x360`), then parses `#EXT-X-MAP`
//!    and `#EXT-X-BYTERANGE` segments.
//! 2. **Validated Disk Cache**: downloads `init + segment` bytes into
//!    `art_cache_dir()/motion_<sha1>.mp4`, refusing to cache init-only or <16 KB payloads.
//! 3. **Zero-PNG RAM Decode**: pipes the local cached `.mp4` through `ffmpeg` (`-f rawvideo
//!    -pix_fmt rgba pipe:1`) into `MotionCover { width, height, fps, frames_rgba }` with full H.264
//!    High Profile and B-frame support.

use crate::config::AppConfig;
use crate::media::{extract_apple_motion_video_url, fetch_apple_album_urls, motion_cache_key_hex};
use crate::providers::Track;
use reqwest::Client;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

const SAFARI_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
const MAX_DOWNLOAD_BYTES: u64 = 6_000_000;
const MIN_VALID_MP4_BYTES: u64 = 16_384;

#[derive(Debug, Clone)]
pub struct MotionCover {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Tightly packed RGBA8 frames (`width * height * 4` bytes each).
    pub frames_rgba: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub length: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HlsSegmentSpec {
    pub map_uri: String,
    pub map_range: ByteRange,
    pub segments: Vec<(String, ByteRange, f32)>,
}

/// Resolves, caches (`motion_<hex>.mp4`), and decodes an Apple Music motion cover into RGBA8 frames.
pub async fn resolve_motion_cover(
    http: &Client,
    track: &Track,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<MotionCover> {
    if track.artist.trim().is_empty() || track.title.trim().is_empty() || max_frames == 0 {
        return None;
    }

    let hex = motion_cache_key_hex(track);
    let cache_dir = AppConfig::art_cache_dir();
    let cache_path = cache_dir.join(format!("motion_{hex}.mp4"));

    let has_valid_cache = tokio::fs::metadata(&cache_path)
        .await
        .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
        .unwrap_or(false);

    if !has_valid_cache {
        download_and_cache_motion_mp4(http, track, &cache_path).await?;
    }

    decode_mp4_via_ffmpeg_pipe(&cache_path, max_dim, target_fps, max_frames).await
}

async fn download_and_cache_motion_mp4(
    http: &Client,
    track: &Track,
    cache_path: &Path,
) -> Option<()> {
    let album_urls = fetch_apple_album_urls(http, track).await;
    let mut master_url: Option<String> = None;
    for album_url in album_urls {
        let Ok(resp) = http
            .get(&album_url)
            .header("User-Agent", SAFARI_UA)
            .timeout(Duration::from_secs(4))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(html) = resp.text().await else {
            continue;
        };
        if let Some(found) = extract_apple_motion_video_url(&html) {
            master_url = Some(found);
            break;
        }
    }

    let master_url = master_url?;
    if !master_url.contains(".m3u8") {
        let resp = http
            .get(&master_url)
            .header("User-Agent", SAFARI_UA)
            .timeout(Duration::from_secs(6))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().await.ok()?.to_vec();
        if bytes.len() as u64 <= MAX_DOWNLOAD_BYTES && bytes.len() as u64 >= MIN_VALID_MP4_BYTES {
            persist_motion_mp4(cache_path, &bytes).await;
            return Some(());
        }
        return None;
    }

    let master_text = http
        .get(&master_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let variant_rel = select_motion_hls_variant_url(&master_text)?;
    let variant_url = resolve_url_against(&master_url, &variant_rel);

    let variant_text = http
        .get(&variant_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let spec = parse_variant_m3u8(&variant_text)?;

    let mut mp4_bytes = Vec::new();
    let init_url = resolve_url_against(&variant_url, &spec.map_uri);
    let init_bytes = fetch_byte_range(http, &init_url, &spec.map_range).await?;
    mp4_bytes.extend_from_slice(&init_bytes);

    let mut total_dur = 0.0f32;
    let mut segments_downloaded = 0usize;
    for (seg_uri, seg_range, dur) in &spec.segments {
        if mp4_bytes.len() as u64 + seg_range.length > MAX_DOWNLOAD_BYTES {
            break;
        }
        let seg_url = resolve_url_against(&variant_url, seg_uri);
        if let Some(seg_bytes) = fetch_byte_range(http, &seg_url, seg_range).await {
            if !seg_bytes.is_empty() {
                mp4_bytes.extend_from_slice(&seg_bytes);
                total_dur += *dur;
                segments_downloaded += 1;
            }
        } else if segments_downloaded == 0 {
            return None;
        }
        if total_dur >= 6.0 {
            break;
        }
    }

    if segments_downloaded == 0 || (mp4_bytes.len() as u64) < MIN_VALID_MP4_BYTES {
        return None;
    }
    persist_motion_mp4(cache_path, &mp4_bytes).await;
    Some(())
}

async fn persist_motion_mp4(cache_path: &Path, bytes: &[u8]) {
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let tmp = cache_path.with_extension("mp4.tmp");
    if tokio::fs::write(&tmp, bytes).await.is_ok()
        && tokio::fs::rename(&tmp, cache_path).await.is_err()
    {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
}

async fn decode_mp4_via_ffmpeg_pipe(
    cache_path: &Path,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<MotionCover> {
    let dim = max_dim.clamp(256, 768);
    let fps = target_fps.clamp(8.0, 24.0);
    let dur_secs = (max_frames as f32 / fps).clamp(2.0, 6.0);
    let vf = format!(
        "fps={fps:.2},scale={dim}:{dim}:force_original_aspect_ratio=increase:flags=lanczos,crop={dim}:{dim}"
    );

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.args([
        "-v",
        "error",
        "-i",
        &cache_path.to_string_lossy(),
        "-t",
        &format!("{dur_secs:.2}"),
        "-vf",
        &vf,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true);

    let output = tokio::time::timeout(Duration::from_secs(8), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let frame_bytes = (dim as usize) * (dim as usize) * 4;
    let frames_rgba: Vec<Vec<u8>> = output
        .stdout
        .chunks_exact(frame_bytes)
        .take(max_frames)
        .map(|chunk| chunk.to_vec())
        .collect();

    if frames_rgba.len() < 2 {
        return None;
    }

    Some(MotionCover {
        width: dim,
        height: dim,
        fps,
        frames_rgba,
    })
}

async fn fetch_byte_range(http: &Client, url: &str, range: &ByteRange) -> Option<Vec<u8>> {
    if range.length == 0 {
        return None;
    }
    let end = range.offset.saturating_add(range.length).saturating_sub(1);
    let resp = http
        .get(url)
        .header("User-Agent", SAFARI_UA)
        .header("Range", format!("bytes={}-{end}", range.offset))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    Some(resp.bytes().await.ok()?.to_vec())
}

fn resolve_url_against(base_url: &str, relative: &str) -> String {
    if relative.starts_with("https://") || relative.starts_with("http://") {
        return relative.to_string();
    }
    let clean_base = base_url.split(['?', '#']).next().unwrap_or(base_url);
    let dir = match clean_base.rfind('/') {
        Some(idx) => &clean_base[..=idx],
        None => clean_base,
    };
    format!("{dir}{}", relative.trim_start_matches('/'))
}

/// Selects the `768x768` `avc1` variant first (`tier 0`), then `486..=767` (`tier 1`),
/// then `960..=1080` (`tier 2`), then any `avc1` (`tier 3`), breaking ties by highest `BANDWIDTH=`.
pub fn select_motion_hls_variant_url(master_m3u8: &str) -> Option<String> {
    let mut pending_inf: Option<&str> = None;
    let mut candidates: Vec<(u8, std::cmp::Reverse<u64>, String)> = Vec::new();

    for raw in master_m3u8.lines() {
        let line = raw.trim();
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
        if line.ends_with("_iframes.m3u8") || !inf.contains("avc1") {
            continue;
        }
        let res = parse_m3u8_resolution(inf);
        let bw = parse_m3u8_bandwidth(inf).unwrap_or(0);
        let tier = match res {
            Some((768, 768)) => 0u8,
            Some((w, h)) if (486..=767).contains(&w) && (486..=767).contains(&h) => 1u8,
            Some((w, h)) if (960..=1080).contains(&w) && (960..=1080).contains(&h) => 2u8,
            _ => 3u8,
        };
        candidates.push((tier, std::cmp::Reverse(bw), line.to_string()));
    }

    candidates.sort_by_key(|(tier, rev_bw, _)| (*tier, *rev_bw));
    candidates.into_iter().next().map(|(_, _, url)| url)
}

fn parse_m3u8_bandwidth(inf: &str) -> Option<u64> {
    let idx = inf.find("BANDWIDTH=")?;
    let after = &inf[idx + "BANDWIDTH=".len()..];
    let token = after.split([',', ' ']).next()?;
    token.trim().parse().ok()
}

fn parse_m3u8_resolution(inf: &str) -> Option<(u32, u32)> {
    let idx = inf.find("RESOLUTION=")?;
    let after = &inf[idx + "RESOLUTION=".len()..];
    let token = after.split([',', ' ']).next()?;
    let (w, h) = token.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn parse_byterange_value(raw: &str) -> Option<ByteRange> {
    let clean = raw.trim().trim_matches('"');
    let (len_str, off_str) = clean.split_once('@')?;
    Some(ByteRange {
        length: len_str.trim().parse().ok()?,
        offset: off_str.trim().parse().ok()?,
    })
}

/// Parses `#EXT-X-MAP` and `#EXT-X-BYTERANGE` segment entries from an Apple Music variant `.m3u8`.
pub fn parse_variant_m3u8(m3u8: &str) -> Option<HlsSegmentSpec> {
    let mut map_uri: Option<String> = None;
    let mut map_range: Option<ByteRange> = None;
    let mut pending_range: Option<ByteRange> = None;
    let mut pending_dur: f32 = 2.0;
    let mut segments = Vec::new();

    for raw in m3u8.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            for part in rest.split(',') {
                let p = part.trim();
                if let Some(uri) = p.strip_prefix("URI=") {
                    map_uri = Some(uri.trim_matches('"').to_string());
                } else if let Some(br) = p.strip_prefix("BYTERANGE=") {
                    map_range = parse_byterange_value(br);
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            if let Some(dur_str) = rest.split(',').next() {
                if let Ok(d) = dur_str.trim().parse::<f32>() {
                    pending_dur = d;
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = parse_byterange_value(rest);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(range) = pending_range.take() {
            segments.push((line.to_string(), range, pending_dur));
            let total_dur: f32 = segments.iter().map(|(_, _, d)| *d).sum();
            if total_dur >= 6.0 || segments.len() >= 6 {
                break;
            }
        }
    }

    let map_uri = map_uri?;
    let map_range = map_range?;
    if segments.is_empty() {
        return None;
    }
    Some(HlsSegmentSpec {
        map_uri,
        map_range,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_768x768_avc1_over_1080x1080_and_360x360() {
        let master = "#EXTM3U\n\
            #EXT-X-I-FRAME-STREAM-INF:CODECS=\"avc1.64001f\",RESOLUTION=768x768,URI=\"768_iframes.m3u8\"\n\
            #EXT-X-STREAM-INF:BANDWIDTH=450000,CODECS=\"avc1.64001f\",RESOLUTION=360x360\n\
            360x360_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=900000,CODECS=\"avc1.64001f\",RESOLUTION=486x486\n\
            486x486_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=2300000,CODECS=\"avc1.640020\",RESOLUTION=768x768\n\
            768x768_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=10700000,CODECS=\"avc1.640028\",RESOLUTION=1080x1080\n\
            1080x1080_avc.m3u8\n";
        assert_eq!(
            select_motion_hls_variant_url(master).as_deref(),
            Some("768x768_avc.m3u8")
        );
    }

    #[test]
    fn parses_ext_x_map_and_byterange_from_apple_768_variant_m3u8() {
        let variant = "#EXTM3U\n\
            #EXT-X-VERSION:6\n\
            #EXT-X-TARGETDURATION:4\n\
            #EXT-X-MAP:URI=\"video_768.mp4\",BYTERANGE=\"742@0\"\n\
            #EXTINF:3.000,\n\
            #EXT-X-BYTERANGE:421056@742\n\
            video_768.mp4\n";
        let spec = parse_variant_m3u8(variant).expect("valid variant m3u8");
        assert_eq!(spec.map_uri, "video_768.mp4");
        assert_eq!(
            spec.map_range,
            ByteRange {
                length: 742,
                offset: 0
            }
        );
        assert_eq!(spec.segments.len(), 1);
        assert_eq!(spec.segments[0].0, "video_768.mp4");
        assert_eq!(
            spec.segments[0].1,
            ByteRange {
                length: 421056,
                offset: 742
            }
        );
    }
}
