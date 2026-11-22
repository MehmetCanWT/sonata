//! Alpha-invariant Gaussian depth-of-field lyric rasterization and LRU cache.
//!
//! 1. **Font Matching**: registers the embedded `Inter` font faces in `cosmic_text::FontSystem` so blurred inactive lyrics match sharp GPUI-rendered lyrics.
//! 2. **Alpha-Invariant Cache**: `BlurredLyricKey` omits opacity and caches pure white full-alpha BGRA images (`256`-entry LRU), letting callers apply `.opacity(alpha)` on the wrapper `div()` with zero scroll-frame re-rasterization.
//! 3. **3-Pass Box Blur**: `gaussian_blur_alpha` approximates a true Gaussian kernel in O(W*H) over a single float alpha plane.

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
use gpui_kit::{RenderImage, Window};
use image::{Frame, RgbaImage};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const MAX_BLURRED_CACHE_ENTRIES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlurredLyricKey {
    text_hash: u64,
    width_px: u16,
    font_size_half_px: u16,
    blur_half_px: u16,
    align_right: bool,
    medium_weight: bool,
    scale_tenths: u16,
}

/// Cached Gaussian-blurred lyric image along with its logical display size and blur padding.
#[derive(Clone)]
pub struct BlurredLyricImage {
    pub image: Arc<RenderImage>,
    pub logical_width: f32,
    pub logical_height: f32,
    pub pad_px: f32,
}

/// Rasterizes inactive lyric verses with true Gaussian depth-of-field blur into GPUI `RenderImage`s.
pub struct BlurredLyricCache {
    font_system: FontSystem,
    swash_cache: SwashCache,
    entries: HashMap<BlurredLyricKey, BlurredLyricImage>,
    lru: VecDeque<BlurredLyricKey>,
}

impl Default for BlurredLyricCache {
    fn default() -> Self {
        let mut font_system = FontSystem::new();
        for bytes in crate::INTER_FONTS {
            font_system.db_mut().load_font_data((*bytes).to_vec());
        }
        Self {
            font_system,
            swash_cache: SwashCache::new(),
            entries: HashMap::new(),
            lru: VecDeque::new(),
        }
    }
}

impl BlurredLyricCache {
    fn make_key(
        text: &str,
        width_px: f32,
        font_size_px: f32,
        blur_sigma_px: f32,
        align_right: bool,
        medium_weight: bool,
        scale_factor: f32,
    ) -> BlurredLyricKey {
        let width_q = ((width_px / 8.0).round() as u16).max(8) * 8;
        let font_half_q = (font_size_px * 2.0).round().clamp(16.0, 200.0) as u16;
        let blur_half_q = (blur_sigma_px * 2.0).round().clamp(1.0, 48.0) as u16;
        let scale_q = (scale_factor.clamp(1.0, 3.0) * 10.0).round() as u16;

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        BlurredLyricKey {
            text_hash: hasher.finish(),
            width_px: width_q,
            font_size_half_px: font_half_q,
            blur_half_px: blur_half_q,
            align_right,
            medium_weight,
            scale_tenths: scale_q,
        }
    }

    /// Returns a cached (or freshly rasterized + Gaussian-blurred) full-alpha white image for a lyric verse.
    #[allow(clippy::too_many_arguments)]
    pub fn get_or_render(
        &mut self,
        text: &str,
        width_px: f32,
        font_size_px: f32,
        line_height_px: f32,
        blur_sigma_px: f32,
        align_right: bool,
        medium_weight: bool,
        scale_factor: f32,
        window: &mut Window,
    ) -> Option<BlurredLyricImage> {
        if text.trim().is_empty() || width_px < 40.0 || font_size_px < 8.0 {
            return None;
        }

        let key = Self::make_key(
            text,
            width_px,
            font_size_px,
            blur_sigma_px,
            align_right,
            medium_weight,
            scale_factor,
        );

        if let Some(hit) = self.entries.get(&key).cloned() {
            if self.lru.back() != Some(&key) {
                self.lru.retain(|k| k != &key);
                self.lru.push_back(key);
            }
            return Some(hit);
        }

        let actual_scale = f32::from(key.scale_tenths) / 10.0;
        let actual_width = f32::from(key.width_px);
        let actual_font = f32::from(key.font_size_half_px) * 0.5;
        let actual_blur = f32::from(key.blur_half_px) * 0.5;
        let actual_lh = line_height_px.max(actual_font * 1.15);

        let rendered = self.rasterize_blurred(
            text,
            actual_width,
            actual_font,
            actual_lh,
            actual_blur,
            align_right,
            medium_weight,
            actual_scale,
        )?;

        self.entries.insert(key, rendered.clone());
        self.lru.push_back(key);
        while self.entries.len() > MAX_BLURRED_CACHE_ENTRIES {
            if let Some(oldest) = self.lru.pop_front() {
                if let Some(evicted) = self.entries.remove(&oldest) {
                    let _ = window.drop_image(evicted.image);
                }
            } else {
                break;
            }
        }

        Some(rendered)
    }

    #[allow(clippy::too_many_arguments)]
    fn rasterize_blurred(
        &mut self,
        text: &str,
        width_logical: f32,
        font_size_logical: f32,
        line_height_logical: f32,
        blur_sigma_logical: f32,
        align_right: bool,
        medium_weight: bool,
        scale: f32,
    ) -> Option<BlurredLyricImage> {
        let font_px = font_size_logical * scale;
        let lh_px = line_height_logical * scale;
        let width_dev = (width_logical * scale).ceil().max(16.0);
        let sigma_dev = (blur_sigma_logical * scale).max(0.5);
        let pad_dev = (sigma_dev * 3.0).ceil() as usize;

        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_px, lh_px));
        buffer.set_wrap(Wrap::WordOrGlyph);
        buffer.set_size(Some(width_dev), None);

        let attrs = Attrs::new()
            .family(Family::Name("Inter"))
            .weight(if medium_weight {
                Weight::MEDIUM
            } else {
                Weight::SEMIBOLD
            });
        let align = if align_right {
            Some(Align::Right)
        } else {
            Some(Align::Left)
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, align);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let line_count = buffer.layout_runs().count().max(1);
        let content_h_dev = (line_count as f32 * lh_px).ceil() as usize;
        let content_w_dev = width_dev as usize;

        let total_w = content_w_dev + pad_dev * 2;
        let total_h = content_h_dev + pad_dev * 2;
        if total_w == 0 || total_h == 0 || total_w > 4096 || total_h > 2048 {
            return None;
        }

        // Rasterize crisp text coverage into a single alpha plane (0.0..1.0).
        let mut alpha_plane = vec![0.0f32; total_w * total_h];
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgba(255, 255, 255, 255),
            |x, y, _w, _h, color| {
                let a = color.a();
                if a == 0 {
                    return;
                }
                let px_x = x + pad_dev as i32;
                let px_y = y + pad_dev as i32;
                if px_x >= 0 && px_y >= 0 && (px_x as usize) < total_w && (px_y as usize) < total_h
                {
                    let idx = px_y as usize * total_w + px_x as usize;
                    let src = f32::from(a) / 255.0;
                    // Porter-Duff source-over for overlapping glyph pixels.
                    alpha_plane[idx] = src + alpha_plane[idx] * (1.0 - src);
                }
            },
        );

        // Apply 3-pass separable box blur (tightly approximates true Gaussian of std dev sigma_dev).
        gaussian_blur_alpha(&mut alpha_plane, total_w, total_h, sigma_dev);

        // Pack into straight BGRA: pure white RGB (255, 255, 255) with full alpha.
        // Callers apply display opacity on the GPUI element so edge fades never bust the cache.
        let mut img = RgbaImage::new(total_w as u32, total_h as u32);
        let raw: &mut [u8] = img.as_mut();
        for (i, &a) in alpha_plane.iter().enumerate() {
            let out_a = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            let o = i * 4;
            if out_a == 0 {
                raw[o] = 0;
                raw[o + 1] = 0;
                raw[o + 2] = 0;
                raw[o + 3] = 0;
            } else {
                raw[o] = 255;
                raw[o + 1] = 255;
                raw[o + 2] = 255;
                raw[o + 3] = out_a;
            }
        }

        Some(BlurredLyricImage {
            image: Arc::new(RenderImage::new([Frame::new(img)])),
            logical_width: total_w as f32 / scale,
            logical_height: total_h as f32 / scale,
            pad_px: pad_dev as f32 / scale,
        })
    }

    pub fn clear(&mut self, window: &mut Window) {
        for (_, entry) in self.entries.drain() {
            let _ = window.drop_image(entry.image);
        }
        self.lru.clear();
    }
}

/// Computes the 3 box-filter radii whose convolution approximates a Gaussian with standard deviation `sigma`.
fn box_blur_radii(sigma: f32) -> [usize; 3] {
    let w_ideal = ((12.0 * sigma * sigma / 3.0) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as usize;
    if wl.is_multiple_of(2) {
        wl = wl.saturating_sub(1);
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - (3 * wl * wl) as f32 - (12 * wl) as f32 - 9.0)
        / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round().clamp(0.0, 3.0) as usize;
    let mut radii = [0usize; 3];
    for (i, slot) in radii.iter_mut().enumerate() {
        let width = if i < m { wl } else { wu };
        *slot = width / 2;
    }
    radii
}

/// Fast O(W*H) 3-pass separable box blur over a single float alpha plane.
fn gaussian_blur_alpha(plane: &mut [f32], w: usize, h: usize, sigma: f32) {
    if w == 0 || h == 0 || sigma < 0.4 {
        return;
    }
    let radii = box_blur_radii(sigma);
    let mut scratch = vec![0.0f32; w * h];
    for r in radii {
        if r == 0 {
            continue;
        }
        box_blur_h(plane, &mut scratch, w, h, r);
        box_blur_v(&scratch, plane, w, h, r);
    }
}

fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let inv = 1.0 / (2 * r + 1) as f32;
    let max_x = w - 1;
    for y in 0..h {
        let row = y * w;
        let first = src[row];
        let last = src[row + max_x];
        let mut acc = first * (r + 1) as f32;
        for j in 1..=r {
            acc += src[row + j.min(max_x)];
        }
        for x in 0..w {
            dst[row + x] = acc * inv;
            let add_idx = (x + r + 1).min(max_x);
            let sub_idx = x.saturating_sub(r);
            let add_val = if x + r + 1 > max_x {
                last
            } else {
                src[row + add_idx]
            };
            let sub_val = if x < r { first } else { src[row + sub_idx] };
            acc += add_val - sub_val;
        }
    }
}

fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let inv = 1.0 / (2 * r + 1) as f32;
    let max_y = h - 1;
    for x in 0..w {
        let first = src[x];
        let last = src[max_y * w + x];
        let mut acc = first * (r + 1) as f32;
        for j in 1..=r {
            acc += src[j.min(max_y) * w + x];
        }
        for y in 0..h {
            dst[y * w + x] = acc * inv;
            let add_val = if y + r + 1 > max_y {
                last
            } else {
                src[(y + r + 1) * w + x]
            };
            let sub_val = if y < r { first } else { src[(y - r) * w + x] };
            acc += add_val - sub_val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_blur_spreads_impulse_smoothly() {
        let (w, h) = (31, 31);
        let mut plane = vec![0.0f32; w * h];
        plane[15 * w + 15] = 1.0;
        gaussian_blur_alpha(&mut plane, w, h, 2.5);
        let center = plane[15 * w + 15];
        let neighbor = plane[15 * w + 17];
        assert!(center > neighbor && neighbor > 0.005);
        let sum: f32 = plane.iter().sum();
        assert!((sum - 1.0).abs() < 0.05, "blur should conserve mass: {sum}");
    }

    #[test]
    fn blurred_lyric_rasterizes_unicode_text() {
        let mut cache = BlurredLyricCache::default();
        assert!(
            cache
                .font_system
                .db()
                .faces()
                .any(|f| f.families.iter().any(|(name, _)| name == "Inter")),
            "embedded Inter font should be registered in BlurredLyricCache"
        );
        let img = cache
            .rasterize_blurred(
                "The emptiness machine — Türkçe şarkı",
                420.0,
                32.0,
                40.0,
                3.0,
                false,
                false,
                1.0,
            )
            .expect("blurred lyric image");
        let bytes = img.image.as_bytes(0).expect("bgra bytes");
        assert!(bytes.as_chunks::<4>().0.iter().any(|px| px[3] > 20));
        // Non-transparent pixels should be pure white BGRA to avoid dark halos.
        assert!(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] > 0)
            .all(|px| px[0] == 255 && px[1] == 255 && px[2] == 255));
    }

    #[test]
    fn blur_cache_key_is_alpha_invariant_and_has_expected_fields() {
        let k1 = BlurredLyricCache::make_key("Hello", 420.0, 26.5, 2.0, false, false, 1.0);
        let k2 = BlurredLyricCache::make_key("Hello", 420.0, 26.5, 2.0, false, false, 1.0);
        let k_counter = BlurredLyricCache::make_key("Hello", 420.0, 26.5, 2.0, true, false, 1.0);
        assert_eq!(k1, k2);
        assert_ne!(k1, k_counter);
        assert_eq!(MAX_BLURRED_CACHE_ENTRIES, 256);
    }
}
