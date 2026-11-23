//! Asynchronous bounded image cache, dominant-color extractor, and blurred backdrop generator.
//!
//! 1. **LRU Storage**: bounds decoded crisp and blurred `RenderImage` textures to `96 MiB` total pixel bytes with hit-first lookup (zero `stat()` syscalls on cache hits).
//! 2. **Async Decode & Coalesced Redraw**: decodes artwork files via `cx.background_executor()`, negative-caches broken/missing paths to prevent spin loops, and batches `cx.notify()` redraws.
//! 3. **Backdrop Blur & Tint**: produces `48x48` 2-pass box-blurred textures for `header_wash` and fullscreen backdrops plus dominant RGB extraction.

use gpui_kit::{App, RenderImage};
use image::{imageops::FilterType, Frame, GenericImageView, RgbaImage};
use sonata_core::media::art_cache_path;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_IMAGE_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_SQUARE_ART_DIM: u32 = 256;
const MAX_WIDE_BANNER_W: u32 = 1600;
const MAX_WIDE_BANNER_H: u32 = 720;
const BLUR_TEX_SIZE: u32 = 48;
const FAILED_RETRY_COOLDOWN: Duration = Duration::from_secs(30);
const MISSING_RETRY_COOLDOWN: Duration = Duration::from_secs(2);
const NOTIFY_COALESCE_MS: u64 = 16;

#[derive(Clone)]
pub struct CachedMedia {
    pub image: Arc<RenderImage>,
    /// Full-frame blurred texture (used for fullscreen album-art backdrop).
    pub blurred: Arc<RenderImage>,
    /// Vertically faded blurred texture whose alpha reaches 0 at the bottom edge (seamless `header_wash`).
    pub header_blurred: Arc<RenderImage>,
    pub dominant_rgb: (u8, u8, u8),
    bytes: usize,
}

#[derive(Default)]
struct ImageCacheInner {
    entries: HashMap<PathBuf, CachedMedia>,
    order: VecDeque<PathBuf>,
    in_flight: HashSet<PathBuf>,
    failed: HashMap<PathBuf, Instant>,
    missing: HashMap<PathBuf, Instant>,
    notify_pending: bool,
    total_bytes: usize,
}

impl ImageCacheInner {
    fn touch(&mut self, path: &Path) {
        if let Some(pos) = self.order.iter().position(|p| p == path) {
            if let Some(item) = self.order.remove(pos) {
                self.order.push_back(item);
            }
        }
    }

    fn insert(&mut self, path: PathBuf, entry: CachedMedia) -> Vec<CachedMedia> {
        let mut evicted = Vec::new();
        self.in_flight.remove(&path);
        self.failed.remove(&path);
        self.missing.remove(&path);
        if let Some(old) = self.entries.remove(&path) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
            if let Some(pos) = self.order.iter().position(|p| p == &path) {
                self.order.remove(pos);
            }
            evicted.push(old);
        }
        while self.total_bytes + entry.bytes > MAX_IMAGE_CACHE_BYTES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.bytes);
                evicted.push(removed);
            }
        }
        self.total_bytes += entry.bytes;
        self.order.push_back(path.clone());
        self.entries.insert(path, entry);
        evicted
    }
}

#[derive(Clone, Default)]
pub struct MediaCache {
    inner: Arc<Mutex<ImageCacheInner>>,
}

impl MediaCache {
    /// Clears cached missing-file entries so newly downloaded thumbnails are picked up immediately.
    pub fn clear_missing(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.missing.clear();
        }
    }

    pub fn get_for_url(
        &self,
        url: Option<&str>,
        cx: &mut App,
        on_ready: impl FnOnce(&mut App) + Send + 'static,
    ) -> Option<CachedMedia> {
        let u = url.filter(|s| !s.is_empty())?;
        let path = art_cache_path(u);
        self.get_for_path(&path, cx, on_ready)
    }

    pub fn get_for_path(
        &self,
        path: &Path,
        cx: &mut App,
        on_ready: impl FnOnce(&mut App) + Send + 'static,
    ) -> Option<CachedMedia> {
        let now = Instant::now();
        {
            let Ok(mut guard) = self.inner.lock() else {
                return None;
            };
            if let Some(hit) = guard.entries.get(path).cloned() {
                guard.touch(path);
                return Some(hit);
            }
            if guard.in_flight.contains(path) {
                return None;
            }
            if guard
                .failed
                .get(path)
                .is_some_and(|t| now.saturating_duration_since(*t) < FAILED_RETRY_COOLDOWN)
            {
                return None;
            }
            if guard
                .missing
                .get(path)
                .is_some_and(|t| now.saturating_duration_since(*t) < MISSING_RETRY_COOLDOWN)
            {
                return None;
            }
            if !path.exists() {
                guard.missing.insert(path.to_path_buf(), now);
                return None;
            }
            guard.missing.remove(path);
            guard.in_flight.insert(path.to_path_buf());
        }

        let cache = self.clone();
        let path_owned = path.to_path_buf();
        cx.spawn(async move |cx| {
            let path_bg = path_owned.clone();
            let decoded = cx
                .background_executor()
                .spawn(async move { decode_and_build_media(&path_bg) })
                .await;
            let (should_schedule_notify, evicted) = {
                let Ok(mut guard) = cache.inner.lock() else {
                    return;
                };
                if let Some(entry) = decoded {
                    let evicted = guard.insert(path_owned, entry);
                    if !guard.notify_pending {
                        guard.notify_pending = true;
                        (true, evicted)
                    } else {
                        (false, evicted)
                    }
                } else {
                    guard.in_flight.remove(&path_owned);
                    guard.failed.insert(path_owned, Instant::now());
                    (false, Vec::new())
                }
            };
            if !evicted.is_empty() {
                cx.update(|cx| {
                    for old in evicted {
                        cx.drop_image(old.image, None);
                        cx.drop_image(old.blurred, None);
                        cx.drop_image(old.header_blurred, None);
                    }
                });
            }
            if should_schedule_notify {
                cx.background_executor()
                    .timer(Duration::from_millis(NOTIFY_COALESCE_MS))
                    .await;
                if let Ok(mut guard) = cache.inner.lock() {
                    guard.notify_pending = false;
                }
                cx.update(|cx| {
                    on_ready(cx);
                });
            }
        })
        .detach();
        None
    }
}

fn decode_and_build_media(path: &Path) -> Option<CachedMedia> {
    let bytes = std::fs::read(path).ok()?;
    let mut img = image::load_from_memory(&bytes).ok()?;
    let (w, h) = img.dimensions();
    // Clamp covers to 384x384 (~589 KB) and wide channel banners to 800x450 so 150+ covers fit cleanly.
    let is_wide_banner = (w as f32) > (h as f32 * 1.45);
    if is_wide_banner {
        if w > MAX_WIDE_BANNER_W || h > MAX_WIDE_BANNER_H {
            img = img.resize(MAX_WIDE_BANNER_W, MAX_WIDE_BANNER_H, FilterType::Triangle);
        }
    } else if w > MAX_SQUARE_ART_DIM || h > MAX_SQUARE_ART_DIM {
        img = img.resize(MAX_SQUARE_ART_DIM, MAX_SQUARE_ART_DIM, FilterType::Triangle);
    }
    let mut rgba = img.to_rgba8();
    let (rw, rh) = rgba.dimensions();

    // Compute dominant color from compact downsampled pixels before any bottom alpha fade.
    let small = image::imageops::resize(&rgba, BLUR_TEX_SIZE, BLUR_TEX_SIZE, FilterType::Triangle);
    let dominant_rgb = extract_dominant_rgb(small.as_raw());

    // For wide artist banners, bake the smooth bottom alpha fade directly into `image` (avoiding a 2nd full texture).
    if is_wide_banner {
        apply_vertical_alpha_fade(rgba.as_mut(), rw as usize, rh as usize, 0.56, 1.8);
    }
    let crisp = rgba_to_render_image(rw, rh, rgba.as_raw())?;

    // Fast 2-pass separable box blur over 48x48 thumbnail (~16x faster than 96x96 3-pass).
    let blurred_rgba = box_blur_rgba(
        small.as_raw(),
        BLUR_TEX_SIZE as usize,
        BLUR_TEX_SIZE as usize,
        4,
    );
    let blurred = rgba_to_render_image(BLUR_TEX_SIZE, BLUR_TEX_SIZE, &blurred_rgba)?;

    // Vertically faded blur texture (alpha fades smoothly to 0.0 at bottom) for seamless header washes.
    let mut header_rgba = blurred_rgba;
    apply_vertical_alpha_fade(
        &mut header_rgba,
        BLUR_TEX_SIZE as usize,
        BLUR_TEX_SIZE as usize,
        0.0,
        2.1,
    );
    let header_blurred = rgba_to_render_image(BLUR_TEX_SIZE, BLUR_TEX_SIZE, &header_rgba)?;

    let pixel_bytes =
        (rw as usize * rh as usize * 4) + (BLUR_TEX_SIZE as usize * BLUR_TEX_SIZE as usize * 4 * 2);
    Some(CachedMedia {
        image: crisp,
        blurred,
        header_blurred,
        dominant_rgb,
        bytes: pixel_bytes,
    })
}

/// Multiplies pixel alpha by a smooth vertical fade from `start_frac` down to `1.0` (alpha = 0).
fn apply_vertical_alpha_fade(rgba: &mut [u8], w: usize, h: usize, start_frac: f32, power: f32) {
    if h <= 1 {
        return;
    }
    let denom = (1.0 - start_frac).max(0.01);
    for y in 0..h {
        let frac = y as f32 / (h - 1) as f32;
        if frac <= start_frac {
            continue;
        }
        let t = ((frac - start_frac) / denom).clamp(0.0, 1.0);
        let alpha_scale = (1.0 - t).powf(power);
        let row = y * w * 4;
        for x in 0..w {
            let idx = row + x * 4 + 3;
            rgba[idx] = ((rgba[idx] as f32) * alpha_scale).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn extract_dominant_rgb(rgba: &[u8]) -> (u8, u8, u8) {
    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut weight_sum: u64 = 0;

    for px in rgba.as_chunks::<4>().0 {
        let r = px[0] as f32;
        let g = px[1] as f32;
        let b = px[2] as f32;
        let max_c = r.max(g).max(b);
        let min_c = r.min(g).min(b);
        let sat = if max_c > 0.0 {
            (max_c - min_c) / max_c
        } else {
            0.0
        };
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        // Favor saturated, mid-tone artwork pigments over pure black/white margins.
        let mut w = (1.0 + sat * 5.0) as u64;
        if !(24.0..=232.0).contains(&luma) {
            w = 1;
        }
        r_sum += (px[0] as u64) * w;
        g_sum += (px[1] as u64) * w;
        b_sum += (px[2] as u64) * w;
        weight_sum += w;
    }

    if weight_sum == 0 {
        return (110, 90, 170);
    }
    (
        (r_sum / weight_sum) as u8,
        (g_sum / weight_sum) as u8,
        (b_sum / weight_sum) as u8,
    )
}

fn box_blur_rgba(src: &[u8], w: usize, h: usize, radius: usize) -> Vec<u8> {
    let mut a = src.to_vec();
    let mut b = vec![0u8; src.len()];
    for _ in 0..2 {
        // Horizontal pass a -> b
        for y in 0..h {
            let row = y * w * 4;
            for x in 0..w {
                let x0 = x.saturating_sub(radius);
                let x1 = (x + radius).min(w - 1);
                let count = (x1 - x0 + 1) as u32;
                let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
                for xi in x0..=x1 {
                    let idx = row + xi * 4;
                    sr += a[idx] as u32;
                    sg += a[idx + 1] as u32;
                    sb += a[idx + 2] as u32;
                    sa += a[idx + 3] as u32;
                }
                let out = row + x * 4;
                b[out] = (sr / count) as u8;
                b[out + 1] = (sg / count) as u8;
                b[out + 2] = (sb / count) as u8;
                b[out + 3] = (sa / count) as u8;
            }
        }
        // Vertical pass b -> a
        for y in 0..h {
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(h - 1);
            let count = (y1 - y0 + 1) as u32;
            for x in 0..w {
                let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
                for yi in y0..=y1 {
                    let idx = (yi * w + x) * 4;
                    sr += b[idx] as u32;
                    sg += b[idx + 1] as u32;
                    sb += b[idx + 2] as u32;
                    sa += b[idx + 3] as u32;
                }
                let out = (y * w + x) * 4;
                a[out] = (sr / count) as u8;
                a[out + 1] = (sg / count) as u8;
                a[out + 2] = (sb / count) as u8;
                a[out + 3] = (sa / count) as u8;
            }
        }
    }
    a
}

pub fn rgba_to_render_image(width: u32, height: u32, rgba: &[u8]) -> Option<Arc<RenderImage>> {
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }
    let mut bgra = vec![0u8; expected];
    for (dst, src) in bgra
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(rgba.as_chunks::<4>().0)
    {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = src[3];
    }
    let img_buf = RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new([Frame::new(img_buf)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_render_image_swaps_channels_to_bgra() {
        assert!(rgba_to_render_image(2, 2, &[1, 2, 3]).is_none());
        let tex = rgba_to_render_image(1, 1, &[10, 20, 30, 255]).expect("valid texture");
        assert_eq!(tex.as_bytes(0).expect("bgra bytes"), &[30, 20, 10, 255]);
    }
}
