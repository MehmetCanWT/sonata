//! Linux `ffmpeg`-backed animated album cover caching and RGBA8 decoding.
//!
//! 1. **Download & Cache**: Uses portable `sonata_core::motion` discovery and HLS byte-range
//!    fetching, falling back to `ffmpeg` remuxing for segmented HLS playlists, and caches MP4
//!    loops under `paths.art_cache_dir()/motion_<sha1>.mp4`.
//! 2. **Zero-PNG RAM Decode**: Pipes the cached `.mp4` through `ffmpeg` (`-f rawvideo -pix_fmt rgba
//!    pipe:1`) with `kill_on_drop(true)` and strict timeouts into [`MotionCover`].

use crate::config::resolve_distro_binary;
use reqwest::Client;
use sonata_core::media::motion_cache_key_hex;
use sonata_core::motion::{
    fetch_byte_range, parse_variant_m3u8, resolve_motion_media_url, resolve_url_against,
    select_motion_hls_variant_url, DESKTOP_UA, MAX_DOWNLOAD_BYTES, MIN_VALID_MP4_BYTES, SAFARI_UA,
};
use sonata_core::paths::CorePaths;
use sonata_core::providers::Track;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

pub use sonata_core::motion::{ByteRange, HlsSegmentSpec};

#[derive(Debug, Clone)]
pub struct MotionCover {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Tightly packed RGBA8 frames (`width * height * 4` bytes each).
    pub frames_rgba: Vec<Vec<u8>>,
}

/// Resolves, caches (`motion_<hex>.mp4`), and decodes an animated motion cover into RGBA8 frames.
pub async fn resolve_motion_cover(
    http: &Client,
    track: &Track,
    paths: &CorePaths,
    enabled: bool,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<MotionCover> {
    if !enabled
        || track.artist.trim().is_empty()
        || track.title.trim().is_empty()
        || max_frames == 0
    {
        return None;
    }

    let hex = motion_cache_key_hex(track);
    let cache_dir = paths.art_cache_dir();
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
    let master_url = resolve_motion_media_url(http, track).await?;

    if !master_url.to_ascii_lowercase().contains(".m3u8") {
        let resp = http
            .get(&master_url)
            .header("User-Agent", DESKTOP_UA)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        if resp
            .content_length()
            .is_some_and(|len| len > MAX_DOWNLOAD_BYTES)
        {
            return None;
        }
        let bytes = resp.bytes().await.ok()?.to_vec();
        if (bytes.len() as u64) <= MAX_DOWNLOAD_BYTES && (bytes.len() as u64) >= MIN_VALID_MP4_BYTES
        {
            persist_motion_mp4(cache_path, &bytes).await;
            return Some(());
        }
        return None;
    }

    let master_text = http
        .get(&master_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let variant_url = select_motion_hls_variant_url(&master_text)
        .map(|rel| resolve_url_against(&master_url, &rel))
        .unwrap_or_else(|| master_url.clone());

    let variant_text = http
        .get(&variant_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    if let Some(spec) = parse_variant_m3u8(&variant_text) {
        let mut mp4_bytes = Vec::new();
        let init_url = resolve_url_against(&variant_url, &spec.map_uri);
        if let Some(init_bytes) = fetch_byte_range(http, &init_url, &spec.map_range).await {
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
                    break;
                }
                if total_dur >= 6.0 {
                    break;
                }
            }

            if segments_downloaded > 0 && (mp4_bytes.len() as u64) >= MIN_VALID_MP4_BYTES {
                persist_motion_mp4(cache_path, &mp4_bytes).await;
                return Some(());
            }
        }
    }

    // Fallback for segmented HLS playlists without `#EXT-X-BYTERANGE`: materialize a short loop via ffmpeg.
    transcode_hls_via_ffmpeg(&variant_url, cache_path).await
}

async fn transcode_hls_via_ffmpeg(hls_url: &str, cache_path: &Path) -> Option<()> {
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let tmp = cache_path.with_extension("mp4.tmp");
    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
    cmd.args([
        "-y",
        "-v",
        "error",
        "-i",
        hls_url,
        "-t",
        "5.0",
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-vf",
        "scale=480:480:force_original_aspect_ratio=increase:flags=lanczos,crop=480:480",
        &tmp.to_string_lossy(),
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);

    let ok = tokio::time::timeout(Duration::from_secs(12), cmd.status())
        .await
        .ok()
        .and_then(Result::ok)
        .is_some_and(|s| s.success());
    if !ok {
        let _ = tokio::fs::remove_file(&tmp).await;
        return None;
    }
    let valid = tokio::fs::metadata(&tmp)
        .await
        .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
        .unwrap_or(false);
    if !valid || tokio::fs::rename(&tmp, cache_path).await.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
        return None;
    }
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

    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
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
