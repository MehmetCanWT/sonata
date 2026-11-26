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
    fetch_byte_range, is_safe_public_https_url, parse_variant_m3u8, resolve_motion_media_url,
    resolve_url_against, select_motion_hls_variant_url, DESKTOP_UA, MAX_DOWNLOAD_BYTES,
    MIN_VALID_MP4_BYTES, SAFARI_UA,
};
use sonata_core::paths::CorePaths;
use sonata_core::providers::Track;
use std::path::{Path, PathBuf};
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

/// Resolves and caches (`motion_<hex>.mp4`) an animated motion cover without decoding raw frames.
pub async fn resolve_motion_cover_path(
    http: &Client,
    track: &Track,
    paths: &CorePaths,
    enabled: bool,
) -> Option<PathBuf> {
    if !enabled || track.artist.trim().is_empty() || track.title.trim().is_empty() {
        return None;
    }

    let hex = motion_cache_key_hex(track);
    let cache_dir = paths.art_cache_dir();
    let cache_path = cache_dir.join(format!("motion_{hex}.mp4"));

    let has_valid_cache = tokio::fs::metadata(&cache_path)
        .await
        .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
        .unwrap_or(false);

    if has_valid_cache {
        if !is_normalized_motion_mp4_file(&cache_path).await {
            let _ = normalize_motion_mp4(&cache_path, &cache_path).await;
        }
    } else {
        download_and_cache_motion_mp4(http, track, &cache_path).await?;
    }

    Some(cache_path)
}

/// Resolves, caches (`motion_<hex>.mp4`), and decodes an animated motion cover into RGBA8 frames,
/// returning both the cached `.mp4` path and decoded [`MotionCover`].
pub async fn resolve_motion_cover_with_path(
    http: &Client,
    track: &Track,
    paths: &CorePaths,
    enabled: bool,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<(PathBuf, MotionCover)> {
    if max_frames == 0 {
        return None;
    }

    let cache_path = resolve_motion_cover_path(http, track, paths, enabled).await?;
    let cover = decode_mp4_via_ffmpeg_pipe(&cache_path, max_dim, target_fps, max_frames).await?;
    Some((cache_path, cover))
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
    resolve_motion_cover_with_path(http, track, paths, enabled, max_dim, target_fps, max_frames)
        .await
        .map(|(_, cover)| cover)
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

const FFMPEG_HLS_PROTOCOL_WHITELIST: &str = "https,http,tls,tcp,crypto";
const SONATA_LOOP_MARKER: &[u8] = b"sonata-loop-v1";

fn build_ffmpeg_hls_remux_attempts(hls_url: &str, out_path: &Path) -> Vec<Vec<String>> {
    let out = out_path.to_string_lossy().into_owned();
    let vf = "scale=480:480:force_original_aspect_ratio=increase:flags=lanczos,crop=480:480";
    vec![
        vec![
            "-y".to_string(),
            "-v".to_string(),
            "error".to_string(),
            "-protocol_whitelist".to_string(),
            FFMPEG_HLS_PROTOCOL_WHITELIST.to_string(),
            "-i".to_string(),
            hls_url.to_string(),
            "-t".to_string(),
            "5.0".to_string(),
            "-an".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "ultrafast".to_string(),
            "-tune".to_string(),
            "fastdecode".to_string(),
            "-profile:v".to_string(),
            "main".to_string(),
            "-bf".to_string(),
            "0".to_string(),
            "-g".to_string(),
            "30".to_string(),
            "-crf".to_string(),
            "21".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-vf".to_string(),
            vf.to_string(),
            "-metadata".to_string(),
            "comment=sonata-loop-v1".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-f".to_string(),
            "mp4".to_string(),
            out.clone(),
        ],
        vec![
            "-y".to_string(),
            "-v".to_string(),
            "error".to_string(),
            "-protocol_whitelist".to_string(),
            FFMPEG_HLS_PROTOCOL_WHITELIST.to_string(),
            "-i".to_string(),
            hls_url.to_string(),
            "-t".to_string(),
            "5.0".to_string(),
            "-an".to_string(),
            "-c:v".to_string(),
            "libopenh264".to_string(),
            "-bf".to_string(),
            "0".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-vf".to_string(),
            vf.to_string(),
            "-metadata".to_string(),
            "comment=sonata-loop-v1".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-f".to_string(),
            "mp4".to_string(),
            out.clone(),
        ],
        vec![
            "-y".to_string(),
            "-v".to_string(),
            "error".to_string(),
            "-protocol_whitelist".to_string(),
            FFMPEG_HLS_PROTOCOL_WHITELIST.to_string(),
            "-i".to_string(),
            hls_url.to_string(),
            "-t".to_string(),
            "5.0".to_string(),
            "-an".to_string(),
            "-c:v".to_string(),
            "mpeg4".to_string(),
            "-q:v".to_string(),
            "5".to_string(),
            "-vf".to_string(),
            vf.to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-f".to_string(),
            "mp4".to_string(),
            out.clone(),
        ],
        vec![
            "-y".to_string(),
            "-v".to_string(),
            "error".to_string(),
            "-protocol_whitelist".to_string(),
            FFMPEG_HLS_PROTOCOL_WHITELIST.to_string(),
            "-i".to_string(),
            hls_url.to_string(),
            "-t".to_string(),
            "5.0".to_string(),
            "-an".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-f".to_string(),
            "mp4".to_string(),
            out,
        ],
    ]
}

fn is_ffmpeg_encoder_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("unknown encoder")
        || lower.contains("encoder")
        || lower.contains("codec")
        || lower.contains("libx264")
        || lower.contains("libopenh264")
}

async fn transcode_hls_via_ffmpeg(hls_url: &str, cache_path: &Path) -> Option<()> {
    if !is_safe_public_https_url(hls_url) {
        return None;
    }
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let tmp = cache_path.with_extension("mp4.tmp");
    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let attempts = build_ffmpeg_hls_remux_attempts(hls_url, &tmp);

    let mut succeeded = false;
    for args in attempts {
        let _ = tokio::fs::remove_file(&tmp).await;
        let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
        crate::config::configure_background_tokio_command(&mut cmd);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(12), cmd.output()).await
        else {
            break;
        };
        if output.status.success() {
            succeeded = true;
            break;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !is_ffmpeg_encoder_failure(&stderr) {
            break;
        }
    }

    if !succeeded {
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

fn is_fragmented_mp4_header(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|w| w == b"moof" || w == b"hlsf")
}

fn is_normalized_motion_mp4_header(bytes: &[u8]) -> bool {
    !is_fragmented_mp4_header(bytes)
        && bytes
            .windows(SONATA_LOOP_MARKER.len())
            .any(|w| w == SONATA_LOOP_MARKER)
}

async fn is_normalized_motion_mp4_file(path: &Path) -> bool {
    use tokio::io::AsyncReadExt;

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut buf = [0u8; 16_384];
    let mut total = 0usize;
    while total < buf.len() {
        match file.read(&mut buf[total..]).await {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
    is_normalized_motion_mp4_header(&buf[..total])
}

async fn normalize_motion_mp4(src: &Path, dst: &Path) -> bool {
    let tmp = dst.with_extension("remux.mp4.tmp");
    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let src_str = src.to_string_lossy().into_owned();
    let tmp_str = tmp.to_string_lossy().into_owned();
    let vf = "scale='min(720,iw)':'min(720,ih)':force_original_aspect_ratio=decrease:flags=lanczos,pad=ceil(iw/2)*2:ceil(ih/2)*2";
    let attempts = [
        vec![
            "-y",
            "-v",
            "error",
            "-fflags",
            "+genpts",
            "-i",
            src_str.as_str(),
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "fastdecode",
            "-profile:v",
            "main",
            "-bf",
            "0",
            "-g",
            "30",
            "-crf",
            "21",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            vf,
            "-metadata",
            "comment=sonata-loop-v1",
            "-movflags",
            "+faststart",
            "-avoid_negative_ts",
            "make_zero",
            "-f",
            "mp4",
            tmp_str.as_str(),
        ],
        vec![
            "-y",
            "-v",
            "error",
            "-fflags",
            "+genpts",
            "-i",
            src_str.as_str(),
            "-an",
            "-c:v",
            "libopenh264",
            "-bf",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-metadata",
            "comment=sonata-loop-v1",
            "-movflags",
            "+faststart",
            "-avoid_negative_ts",
            "make_zero",
            "-f",
            "mp4",
            tmp_str.as_str(),
        ],
        vec![
            "-y",
            "-v",
            "error",
            "-fflags",
            "+genpts",
            "-i",
            src_str.as_str(),
            "-an",
            "-c:v",
            "copy",
            "-movflags",
            "+faststart",
            "-avoid_negative_ts",
            "make_zero",
            "-f",
            "mp4",
            tmp_str.as_str(),
        ],
    ];

    for args in attempts {
        let _ = tokio::fs::remove_file(&tmp).await;
        let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
        crate::config::configure_background_tokio_command(&mut cmd);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let Ok(Ok(status)) = tokio::time::timeout(Duration::from_secs(12), cmd.status()).await
        else {
            continue;
        };
        let valid = status.success()
            && tokio::fs::metadata(&tmp)
                .await
                .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
                .unwrap_or(false);
        if valid && tokio::fs::rename(&tmp, dst).await.is_ok() {
            return true;
        }
    }

    let _ = tokio::fs::remove_file(&tmp).await;
    false
}

async fn persist_motion_mp4(cache_path: &Path, bytes: &[u8]) {
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let raw_tmp = cache_path.with_extension("raw.tmp");
    if tokio::fs::write(&raw_tmp, bytes).await.is_err() {
        let _ = tokio::fs::remove_file(&raw_tmp).await;
        return;
    }
    if normalize_motion_mp4(&raw_tmp, cache_path).await {
        let _ = tokio::fs::remove_file(&raw_tmp).await;
        return;
    }
    if tokio::fs::rename(&raw_tmp, cache_path).await.is_err() {
        let _ = tokio::fs::remove_file(&raw_tmp).await;
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
    crate::config::configure_background_tokio_command(&mut cmd);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_hls_attempts_pass_mp4_format_before_tmp_path_and_include_fallbacks() {
        let out = Path::new("/tmp/motion_abc.mp4.tmp");
        let attempts = build_ffmpeg_hls_remux_attempts("https://example.com/loop.m3u8", out);
        assert_eq!(attempts.len(), 4);

        for attempt in &attempts {
            let len = attempt.len();
            assert!(len >= 3);
            assert_eq!(attempt[len - 3], "-f");
            assert_eq!(attempt[len - 2], "mp4");
            assert_eq!(attempt[len - 1], out.to_string_lossy());
            let wl_pos = attempt
                .windows(2)
                .position(|w| w == ["-protocol_whitelist", FFMPEG_HLS_PROTOCOL_WHITELIST])
                .expect("protocol whitelist flag must be present");
            let input_pos = attempt
                .iter()
                .position(|arg| arg == "-i")
                .expect("-i flag must be present");
            assert!(wl_pos < input_pos);
        }

        assert!(attempts[0].windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(attempts[0].windows(2).any(|w| w == ["-bf", "0"]));
        assert!(attempts[0].windows(2).any(|w| w == ["-pix_fmt", "yuv420p"]));
        assert!(attempts[1].windows(2).any(|w| w == ["-c:v", "libopenh264"]));
        assert!(attempts[1].windows(2).any(|w| w == ["-pix_fmt", "yuv420p"]));
        assert!(attempts[2].windows(2).any(|w| w == ["-c:v", "mpeg4"]));
        assert!(attempts[3].windows(2).any(|w| w == ["-c:v", "copy"]));
        assert!(!attempts[3].iter().any(|arg| arg == "-vf"));
    }

    #[test]
    fn fragmented_mp4_header_detection_distinguishes_fmp4_from_standard_mp4() {
        assert!(is_fragmented_mp4_header(
            b"\x00\x00\x00\x1cftypiso5\x00\x00\x00\x01isomiso5hlsf"
        ));
        assert!(is_fragmented_mp4_header(b"\x00\x00\x00\x18moof\x00\x00"));
        assert!(!is_fragmented_mp4_header(
            b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2avc1moov"
        ));
        assert!(is_normalized_motion_mp4_header(
            b"\x00\x00\x00\x20ftypisom\x00\x00moov...sonata-loop-v1"
        ));
        assert!(!is_normalized_motion_mp4_header(
            b"\x00\x00\x00\x1cftypiso5hlsfsonata-loop-v1"
        ));
    }

    #[tokio::test]
    async fn transcode_hls_rejects_non_https_and_private_urls() {
        let dir = crate::downloads::test_support::TempTestDir::new("motion_ssrf");
        let out = dir.path().join("motion.mp4");
        for bad in [
            "http://example.com/loop.m3u8",
            "file:///etc/passwd",
            "https://127.0.0.1/loop.m3u8",
            "https://169.254.169.254/latest/meta-data",
            "https://192.168.1.50/loop.m3u8",
            "https://localhost/loop.m3u8",
        ] {
            assert!(
                transcode_hls_via_ffmpeg(bad, &out).await.is_none(),
                "expected `{bad}` to be rejected before ffmpeg"
            );
        }
    }
}
