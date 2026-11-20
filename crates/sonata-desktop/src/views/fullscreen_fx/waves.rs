//! Pure-Rust CPU 5-color flowing ambient wave field renderer for fullscreen mode.
//!
//! 1. **Gradient Noise Field**: quintic 2D Perlin-style noise layers 5 organic wave pools with domain warping, radial vignette, luma ceiling, and 1-LSB dithering.
//! 2. **Compact Frame Sizing**: renders at a lightweight `112x64` base resolution so GPU upscaling stays smooth with minimal CPU overhead.
//! 3. **AmbientWavesState**: caches frames at 10 Hz (30 Hz during 0.8s color-wash track transitions) and drops superseded `RenderImage`s.

use super::palette::{
    extract_cover_palette, palette_from_fallback_rgb, resolve_wave_palette, CoverPalette,
    WAVE_COLORS,
};
use gpui_kit::{Hsla, RenderImage, Rgba, Window};
use image::{Frame, RgbaImage};
use std::f32::consts::{FRAC_1_SQRT_2, TAU};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WAVE_SHADE: f32 = 0.55;
const WAVE_WASH_SECS: f32 = 0.8;
const WAVE_SETTLED: f32 = 1.0 / 255.0;
const WAVE_SPEED: f32 = 1.0;
const WAVE_SWELL: f32 = 1.0;
const WAVE_MORPH: f32 = 1.0;
const WAVE_PEAK: f32 = 0.72;
const WAVE_DENSITY: f32 = 3.0;
const WAVE_SOFTNESS: f32 = 2.2;
const WAVE_VIGNETTE: f32 = 0.6;
const WAVE_CEILING: f32 = 0.18;
const WAVE_BASE_WIDTH: u32 = 112;
const WAVE_BASE_HEIGHT: u32 = 64;
const WAVE_DRIFT_INTERVAL: Duration = Duration::from_millis(100); // 10 Hz
const WAVE_WASH_INTERVAL: Duration = Duration::from_millis(33); // 30 Hz

fn layered_colors(colors: [Hsla; WAVE_COLORS]) -> [Hsla; WAVE_COLORS] {
    let [base, highlight, second, shadow, deep] = colors;
    [base, highlight, shadow, deep, second]
}

fn shaded_rgb(color: Hsla) -> [f32; 3] {
    let rgba = Rgba::from(color);
    let kept = 1.0 - WAVE_SHADE;
    [rgba.r * kept, rgba.g * kept, rgba.b * kept]
}

fn blend_hsla_rgb(from: Hsla, to: Hsla, delta: f32) -> Hsla {
    let (a, b) = (Rgba::from(from), Rgba::from(to));
    Hsla::from(Rgba {
        r: a.r + (b.r - a.r) * delta,
        g: a.g + (b.g - a.g) * delta,
        b: a.b + (b.b - a.b) * delta,
        a: a.a + (b.a - a.a) * delta,
    })
}

fn palette_settled(painted: &[Hsla; WAVE_COLORS], target: &[Hsla; WAVE_COLORS]) -> bool {
    painted.iter().zip(target.iter()).all(|(a, b)| {
        let (a, b) = (Rgba::from(*a), Rgba::from(*b));
        (a.r - b.r).abs() <= WAVE_SETTLED
            && (a.g - b.g).abs() <= WAVE_SETTLED
            && (a.b - b.b).abs() <= WAVE_SETTLED
    })
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn hash2_grad(ix: i32, iy: i32) -> (f32, f32) {
    let mut h = (ix.wrapping_mul(374_761_393)).wrapping_add(iy.wrapping_mul(668_265_263)) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    let angle = (h as f32) * (TAU / 4_294_967_296.0);
    (angle.cos(), angle.sin())
}

#[inline]
fn gradient_noise(x: f32, y: f32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let ix = xi as i32;
    let iy = yi as i32;
    let fx = x - xi;
    let fy = y - yi;

    // Quintic Perlin interpolation curve.
    let ux = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
    let uy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);

    let (g00x, g00y) = hash2_grad(ix, iy);
    let (g10x, g10y) = hash2_grad(ix + 1, iy);
    let (g01x, g01y) = hash2_grad(ix, iy + 1);
    let (g11x, g11y) = hash2_grad(ix + 1, iy + 1);

    let n00 = g00x * fx + g00y * fy;
    let n10 = g10x * (fx - 1.0) + g10y * fy;
    let n01 = g01x * fx + g01y * (fy - 1.0);
    let n11 = g11x * (fx - 1.0) + g11y * (fy - 1.0);

    let nx0 = n00 + (n10 - n00) * ux;
    let nx1 = n01 + (n11 - n01) * ux;
    nx0 + (nx1 - nx0) * uy
}

#[inline]
fn rotate2(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    (x * c - y * s, x * s + y * c)
}

#[inline]
fn wave_pool(qx: f32, qy: f32, scale: f32, sx: f32, sy: f32, dx: f32, dy: f32) -> f32 {
    let reach = 0.4 * WAVE_SOFTNESS;
    let n = gradient_noise(qx * scale + sx + dx, qy * scale + sy + dy);
    smoothstep(0.2 - reach, 0.2 + reach, n)
}

#[inline]
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Renders one frame of the 5-color wave field into a straight-BGRA `RenderImage`.
pub fn render_waves_frame(
    width: u32,
    height: u32,
    colors: [Hsla; WAVE_COLORS],
    stage: Hsla,
    time: f32,
) -> Arc<RenderImage> {
    let w = width.max(16);
    let h = height.max(16);
    let wf = w as f32;
    let hf = h as f32;
    let geom_scale = (wf * hf).sqrt().max(1.0);
    let t = time * WAVE_SPEED;

    let pivot_x = 0.23 + 0.3 * (t * 0.031).sin();
    let pivot_y = -0.17 + 0.25 * (t * 0.023).cos();

    let layered = layered_colors(colors);
    let c0 = shaded_rgb(layered[0]);
    let c1 = shaded_rgb(layered[1]);
    let c2 = shaded_rgb(layered[2]);
    let c3 = shaded_rgb(layered[3]);
    let c4 = shaded_rgb(layered[4]);
    let stage_rgb = shaded_rgb(stage);

    let mut buffer = RgbaImage::new(w, h);
    let raw: &mut [u8] = buffer.as_mut();

    for py in 0..h {
        let fy = py as f32 + 0.5;
        let uv_y = fy / hf;
        let p_y = (fy - hf * 0.5) / geom_scale * WAVE_DENSITY;
        let row_offset = (py * w * 4) as usize;

        for px_i in 0..w {
            let fx = px_i as f32 + 0.5;
            let uv_x = fx / wf;
            let p_x = (fx - wf * 0.5) / geom_scale * WAVE_DENSITY;

            let turn = WAVE_MORPH * 1.2 * gradient_noise(p_x * 0.55 + t * 0.045, p_y * 0.55 + 3.1);
            let (rx, ry) = rotate2(p_x - pivot_x, p_y - pivot_y, turn);
            let mut qx = rx + pivot_x;
            let mut qy = ry + pivot_y;

            let sx = gradient_noise(qx * 1.05, qy * 1.05 + t * 0.06);
            let sy = gradient_noise(qx * 1.05 + 5.2, qy * 1.05 - t * 0.05);
            qx += WAVE_SWELL * 0.26 * sx;
            qy += WAVE_SWELL * 0.26 * sy;
            qx += WAVE_SWELL * 0.07 * (qy * 2.2 + t * 0.21).sin();
            qy += WAVE_SWELL * 0.06 * (qx * 2.9 - t * 0.17).sin();

            let mut col = stage_rgb;
            col = mix3(
                col,
                c0,
                WAVE_PEAK * wave_pool(qx, qy, 1.1, 1.3, 0.0, t * 0.05, 0.0),
            );
            col = mix3(
                col,
                c1,
                WAVE_PEAK * wave_pool(qx, qy, 1.3, -4.7, 2.2, 0.0, -t * 0.055),
            );
            col = mix3(
                col,
                c2,
                WAVE_PEAK * wave_pool(qx, qy, 1.0, 7.1, -3.4, -t * 0.045, 0.0),
            );
            col = mix3(
                col,
                c3,
                WAVE_PEAK * wave_pool(qx, qy, 1.2, -2.6, -6.3, 0.0, t * 0.04),
            );
            col = mix3(
                col,
                c4,
                WAVE_PEAK * wave_pool(qx, qy, 1.15, 9.4, 5.8, t * 0.035, -t * 0.03),
            );

            // Radial vignette keeping the center open and darkening outer corners.
            let ex = (uv_x - 0.5) * 2.0;
            let ey = (uv_y - 0.5) * 2.0;
            let edge = (ex * ex + ey * ey).sqrt() * FRAC_1_SQRT_2;
            let vig = 1.0 - WAVE_VIGNETTE * smoothstep(0.15, 1.0, edge);
            col[0] *= vig;
            col[1] *= vig;
            col[2] *= vig;

            // Soft luma ceiling so vivid covers never wash out white lyrics.
            let luma = col[0] * 0.2126 + col[1] * 0.7152 + col[2] * 0.0722;
            if WAVE_CEILING > 0.0 && luma > WAVE_CEILING {
                let scale = (WAVE_CEILING + (luma - WAVE_CEILING) * 0.35) / luma;
                col[0] *= scale;
                col[1] *= scale;
                col[2] *= scale;
            }

            // Interleaved gradient noise dither (1 LSB) to eliminate dark banding.
            let ign = (52.982_918 * (0.067_110_56 * fx + 0.005_837_15 * fy).fract()).fract();
            let dither = (ign - 0.5) / 255.0;

            let r = ((col[0] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;
            let g = ((col[1] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;
            let b = ((col[2] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;

            // GPUI RenderImage expects straight BGRA byte order.
            let idx = row_offset + (px_i * 4) as usize;
            raw[idx] = b;
            raw[idx + 1] = g;
            raw[idx + 2] = r;
            raw[idx + 3] = 255;
        }
    }

    Arc::new(RenderImage::new([Frame::new(buffer)]))
}

/// Persistent ambient-wave state owned by the desktop view across frames.
pub struct AmbientWavesState {
    started: Instant,
    stepped: Instant,
    last_render: Option<Instant>,
    art_path: Option<PathBuf>,
    fallback_rgb: (u8, u8, u8),
    palette: CoverPalette,
    painted: Option<[Hsla; WAVE_COLORS]>,
    painted_stage: Option<Hsla>,
    washing: bool,
    frame: Option<Arc<RenderImage>>,
    last_dims: (u32, u32),
}

impl Default for AmbientWavesState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            stepped: now,
            last_render: None,
            art_path: None,
            fallback_rgb: (168, 85, 247),
            palette: CoverPalette::default(),
            painted: None,
            painted_stage: None,
            washing: false,
            frame: None,
            last_dims: (0, 0),
        }
    }
}

impl AmbientWavesState {
    /// Returns the current ambient wave frame, regenerating at 10 Hz (or 30 Hz during color wash).
    pub fn frame(
        &mut self,
        art_path: Option<&Path>,
        fallback_rgb: (u8, u8, u8),
        viewport_w: f32,
        viewport_h: f32,
        reduce_motion: bool,
        window: &mut Window,
    ) -> Arc<RenderImage> {
        let art_changed = self.art_path.as_deref() != art_path
            || (art_path.is_none() && self.fallback_rgb != fallback_rgb);
        if art_changed {
            self.art_path = art_path.map(Path::to_path_buf);
            self.fallback_rgb = fallback_rgb;
            self.palette = art_path
                .and_then(extract_cover_palette)
                .unwrap_or_else(|| palette_from_fallback_rgb(fallback_rgb));
            self.washing = true;
            self.last_render = None;
        }

        // Preserve viewport aspect ratio while keeping pixel count compact.
        let aspect = (viewport_w / viewport_h.max(1.0)).clamp(0.5, 3.0);
        let (rw, rh) = if aspect >= 1.0 {
            (
                WAVE_BASE_WIDTH,
                ((WAVE_BASE_WIDTH as f32 / aspect).round() as u32).clamp(48, 128),
            )
        } else {
            (
                ((WAVE_BASE_HEIGHT as f32 * aspect).round() as u32).clamp(48, 128),
                WAVE_BASE_HEIGHT,
            )
        };
        let dims_changed = self.last_dims != (rw, rh);

        let now = Instant::now();
        let interval = if self.washing {
            WAVE_WASH_INTERVAL
        } else {
            WAVE_DRIFT_INTERVAL
        };
        let due = match (self.frame.is_some(), self.last_render) {
            (false, _) => true,
            (true, _) if dims_changed => true,
            (true, _) if reduce_motion && !self.washing => false,
            (true, Some(last)) => now.duration_since(last) >= interval,
            (true, None) => true,
        };

        if !due {
            if let Some(frame) = &self.frame {
                return frame.clone();
            }
        }

        let (target_colors, target_stage) = resolve_wave_palette(self.palette);
        let dt = now.duration_since(self.stepped).as_secs_f32();
        self.stepped = now;

        let animates = !reduce_motion;
        let (colors, stage) = match (self.painted, self.painted_stage) {
            (Some(prev_c), Some(prev_s)) if animates && dt < 2.0 => {
                let delta = 1.0 - (-dt / WAVE_WASH_SECS).exp();
                let next_c =
                    std::array::from_fn(|i| blend_hsla_rgb(prev_c[i], target_colors[i], delta));
                let next_s = blend_hsla_rgb(prev_s, target_stage, delta);
                (next_c, next_s)
            }
            _ => (target_colors, target_stage),
        };

        self.washing = animates && !palette_settled(&colors, &target_colors);
        self.painted = Some(colors);
        self.painted_stage = Some(stage);
        self.last_render = Some(now);
        self.last_dims = (rw, rh);

        let elapsed = if animates {
            self.started.elapsed().as_secs_f32()
        } else {
            0.0
        };
        let next_frame = render_waves_frame(rw, rh, colors, stage, elapsed);
        if let Some(old) = self.frame.replace(next_frame.clone()) {
            let _ = window.drop_image(old);
        }
        next_frame
    }

    pub fn is_washing(&self) -> bool {
        self.washing
    }

    pub fn stage_accent_hsla(&self) -> Hsla {
        self.palette.primary.unwrap_or(Hsla {
            h: 0.78,
            s: 0.55,
            l: 0.68,
            a: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_frame_renders_opaque_bgra_without_panicking() {
        let pal = palette_from_fallback_rgb((195, 72, 165));
        let (colors, stage) = resolve_wave_palette(pal);
        let frame = render_waves_frame(WAVE_BASE_WIDTH, WAVE_BASE_HEIGHT, colors, stage, 1.25);
        let bytes = frame.as_bytes(0).expect("frame bytes");
        assert_eq!(
            bytes.len(),
            (WAVE_BASE_WIDTH * WAVE_BASE_HEIGHT * 4) as usize
        );
        assert!(bytes.as_chunks::<4>().0.iter().all(|px| px[3] == 255));
    }
}
