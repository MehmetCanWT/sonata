//! Ambient 5-color flowing Perlin noise backdrop generator and drift timer for fullscreen mode.
//!
//! 1. **Palette Resolution**: normalizes primary and secondary cover hues into readable dark-stage
//!    wave pools and smooth exponential color-wash interpolation.
//! 2. **Noise Synthesis & Drift**: synthesizes dithered `160x90` RGBA wave textures off the UI thread
//!    on an adaptive wash/drift cadence while fullscreen is visible and motion is enabled.

use crate::ui::media::{rgba_to_render_image, CoverPalette};
use crate::views::SonataDesktopView;
use gpui_kit::{Context, RenderImage};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WAVE_W: u32 = 160;
const WAVE_H: u32 = 90;
const WAVE_SHADE: f32 = 0.55;
const WAVE_PEAK: f32 = 0.72;
const WAVE_DENSITY: f32 = 3.0;
const WAVE_SOFTNESS: f32 = 2.2;
const WAVE_VIGNETTE: f32 = 0.60;
const WAVE_CEILING: f32 = 0.18;
pub(super) const AMBIENT_WASH_SECS: f32 = 0.8;
const AMBIENT_SETTLED: f32 = 1.0 / 255.0;
const AMBIENT_DRIFT_MS: u64 = 160;
const AMBIENT_WASH_MS: u64 = 50;

fn rgb_to_hsl(rgb: [u8; 3]) -> (f32, f32, f32) {
    let r = rgb[0] as f32 / 255.0;
    let g = rgb[1] as f32 / 255.0;
    let b = rgb[2] as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d <= 1e-5 {
        return (0.06, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() <= 1e-5 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() <= 1e-5 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h.rem_euclid(1.0), s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= 1e-5 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue2rgb = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [hue2rgb(h + 1.0 / 3.0), hue2rgb(h), hue2rgb(h - 1.0 / 3.0)]
}

#[inline]
fn hash_grad(ix: i32, iy: i32) -> [f32; 2] {
    use std::f32::consts::FRAC_1_SQRT_2 as D;
    // Fast deterministic integer hash onto 16 unit-circle gradient vectors (zero trig in inner loop).
    const GRADS: [[f32; 2]; 16] = [
        [1.0, 0.0],
        [0.923_879_5, 0.382_683_4],
        [D, D],
        [0.382_683_4, 0.923_879_5],
        [0.0, 1.0],
        [-0.382_683_4, 0.923_879_5],
        [-D, D],
        [-0.923_879_5, 0.382_683_4],
        [-1.0, 0.0],
        [-0.923_879_5, -0.382_683_4],
        [-D, -D],
        [-0.382_683_4, -0.923_879_5],
        [0.0, -1.0],
        [0.382_683_4, -0.923_879_5],
        [D, -D],
        [0.923_879_5, -0.382_683_4],
    ];
    let mut h = (ix as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((iy as u32).wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    GRADS[(h & 15) as usize]
}

/// 2D Perlin gradient noise centered at `0.0` in `[-1.0, 1.0]`.
#[inline]
fn gradient_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
    let uy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);

    let g00 = hash_grad(ix, iy);
    let g10 = hash_grad(ix + 1, iy);
    let g01 = hash_grad(ix, iy + 1);
    let g11 = hash_grad(ix + 1, iy + 1);

    let d00 = g00[0] * fx + g00[1] * fy;
    let d10 = g10[0] * (fx - 1.0) + g10[1] * fy;
    let d01 = g01[0] * fx + g01[1] * (fy - 1.0);
    let d11 = g11[0] * (fx - 1.0) + g11[1] * (fy - 1.0);

    let nx0 = d00 + (d10 - d00) * ux;
    let nx1 = d01 + (d11 - d01) * ux;
    (nx0 + (nx1 - nx0) * uy) * std::f32::consts::SQRT_2
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn rotate2(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    (x * c - y * s, x * s + y * c)
}

#[inline]
fn mix_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn shade_for_readability(rgb: [f32; 3]) -> [f32; 3] {
    let kept = 1.0 - WAVE_SHADE;
    [rgb[0] * kept, rgb[1] * kept, rgb[2] * kept]
}

/// Resolves the 5 layered wave colors (`[base, highlight, shadow, deep, second]`) and stage color
/// from a cover's primary + secondary hue palette, already shaded for lyrics contrast.
pub(super) fn resolve_ambient_palette(
    cover_palette: Option<CoverPalette>,
    fallback_rgb: [u8; 3],
) -> ([[f32; 3]; 5], [f32; 3]) {
    let neutral = |light: f32| hsl_to_rgb(0.06, 0.12, light);
    let dark_bg_l = 0.08_f32;
    let floor = (0.13 + dark_bg_l * 0.8).clamp(0.13, 0.30);

    let (primary, secondary) = match cover_palette {
        Some(pal) => (pal.primary, pal.secondary),
        None => {
            let (h, s, l) = rgb_to_hsl(fallback_rgb);
            let lead = (s >= 0.10).then_some((h, s, l));
            (lead, None)
        }
    };

    let (unshaded, stage_unshaded) = match primary {
        None => (
            [
                neutral(floor),
                neutral(floor + 0.025),
                neutral(floor + 0.015),
                neutral(floor + 0.04),
                neutral(floor + 0.05),
            ],
            neutral(dark_bg_l),
        ),
        Some((ph, ps, _pl)) => {
            // Normalize cover hue through dark-stage accent lightness (0.44) with vivid saturation bounds.
            let accent_s = ps.clamp(0.36, 0.86);
            let accent_l = 0.44_f32;
            let base_h = ph;
            let base_s = (accent_s * 0.90 + 0.04).clamp(0.45, 0.75);
            let base_l = (0.40 + (accent_l - 0.45) * 0.5).clamp(0.28, 0.52);

            let base = hsl_to_rgb(base_h, base_s, base_l);
            let highlight = hsl_to_rgb(
                base_h,
                (base_s - 0.08).clamp(0.40, 0.75),
                (base_l + 0.14).clamp(0.30, 0.62),
            );
            let second = match secondary {
                Some((sh, ss, sl)) => hsl_to_rgb(
                    sh,
                    (ss * 0.90).clamp(0.40, 0.72),
                    (0.38 + (sl - 0.45) * 0.5).clamp(0.28, 0.50),
                ),
                None => neutral(floor + 0.03),
            };
            let shadow = hsl_to_rgb(
                base_h,
                (base_s + 0.04).clamp(0.45, 0.80),
                (base_l - 0.13).clamp(0.18, 0.45),
            );
            let deep = hsl_to_rgb(
                (base_h + 0.97).rem_euclid(1.0),
                (base_s - 0.02).clamp(0.40, 0.78),
                (base_l - 0.16).clamp(0.16, 0.40),
            );
            let stage = hsl_to_rgb(base_h, (base_s * 0.35).clamp(0.08, 0.28), dark_bg_l);
            // Lay pools in order [base, highlight, shadow, deep, second] so runner-up floats above shadows.
            ([base, highlight, shadow, deep, second], stage)
        }
    };

    (
        [
            shade_for_readability(unshaded[0]),
            shade_for_readability(unshaded[1]),
            shade_for_readability(unshaded[2]),
            shade_for_readability(unshaded[3]),
            shade_for_readability(unshaded[4]),
        ],
        shade_for_readability(stage_unshaded),
    )
}

pub(super) fn ambient_palette_settled(
    painted: ([[f32; 3]; 5], [f32; 3]),
    target: ([[f32; 3]; 5], [f32; 3]),
) -> bool {
    let stage_ok = (0..3).all(|c| (painted.1[c] - target.1[c]).abs() <= AMBIENT_SETTLED);
    stage_ok
        && painted.0.iter().zip(target.0.iter()).all(|(p, t)| {
            (p[0] - t[0]).abs() <= AMBIENT_SETTLED
                && (p[1] - t[1]).abs() <= AMBIENT_SETTLED
                && (p[2] - t[2]).abs() <= AMBIENT_SETTLED
        })
}

pub(super) fn blend_ambient_palette(
    from: ([[f32; 3]; 5], [f32; 3]),
    to: ([[f32; 3]; 5], [f32; 3]),
    delta: f32,
) -> ([[f32; 3]; 5], [f32; 3]) {
    let colors = std::array::from_fn(|i| mix_rgb(from.0[i], to.0[i], delta));
    let stage = mix_rgb(from.1, to.1, delta);
    (colors, stage)
}

fn build_ambient_wave_rgba(colors: [[f32; 3]; 5], stage: [f32; 3], time: f32) -> Vec<u8> {
    let pool = |qx: f32, qy: f32, scale: f32, sx: f32, sy: f32, dx: f32, dy: f32| {
        let reach = 0.4 * WAVE_SOFTNESS;
        smoothstep(
            0.2 - reach,
            0.2 + reach,
            gradient_noise(qx * scale + sx + dx, qy * scale + sy + dy),
        )
    };

    let w_f = WAVE_W as f32;
    let h_f = WAVE_H as f32;
    let geo = (w_f * h_f).sqrt().max(1.0);
    let pivot_x = 0.23 + 0.30 * (time * 0.031).sin();
    let pivot_y = -0.17 + 0.25 * (time * 0.023).cos();
    let mut rgba = vec![0u8; (WAVE_W * WAVE_H * 4) as usize];

    for py in 0..WAVE_H {
        let uy = (py as f32 + 0.5) / h_f;
        let py_c = ((py as f32 + 0.5) - h_f * 0.5) / geo * WAVE_DENSITY;
        let dy_uv = (uy - 0.5) * 2.0;
        for px_i in 0..WAVE_W {
            let ux = (px_i as f32 + 0.5) / w_f;
            let px_c = ((px_i as f32 + 0.5) - w_f * 0.5) / geo * WAVE_DENSITY;
            let dx_uv = (ux - 0.5) * 2.0;

            let turn = 1.2 * gradient_noise(px_c * 0.55 + time * 0.045, py_c * 0.55 + 3.1);
            let (rx, ry) = rotate2(px_c - pivot_x, py_c - pivot_y, turn);
            let mut qx = rx + pivot_x;
            let mut qy = ry + pivot_y;
            let warp_x = gradient_noise(qx * 1.05, qy * 1.05 + time * 0.06);
            let warp_y = gradient_noise(qx * 1.05 + 5.2, qy * 1.05 - time * 0.05);
            qx += 0.26 * warp_x;
            qy += 0.26 * warp_y;
            qx += 0.07 * (qy * 2.2 + time * 0.21).sin();
            qy += 0.06 * (qx * 2.9 - time * 0.17).sin();

            let mut c = stage;
            c = mix_rgb(
                c,
                colors[0],
                WAVE_PEAK * pool(qx, qy, 1.1, 1.3, 0.0, time * 0.05, 0.0),
            );
            c = mix_rgb(
                c,
                colors[1],
                WAVE_PEAK * pool(qx, qy, 1.3, -4.7, 2.2, 0.0, -time * 0.055),
            );
            c = mix_rgb(
                c,
                colors[2],
                WAVE_PEAK * pool(qx, qy, 1.0, 7.1, -3.4, -time * 0.045, 0.0),
            );
            c = mix_rgb(
                c,
                colors[3],
                WAVE_PEAK * pool(qx, qy, 1.2, -2.6, -6.3, 0.0, time * 0.04),
            );
            c = mix_rgb(
                c,
                colors[4],
                WAVE_PEAK * pool(qx, qy, 1.15, 9.4, 5.8, time * 0.035, -time * 0.03),
            );

            let edge = (dx_uv * dx_uv + dy_uv * dy_uv).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
            let vig = 1.0 - WAVE_VIGNETTE * smoothstep(0.15, 1.0, edge);
            c = [c[0] * vig, c[1] * vig, c[2] * vig];

            let luma = c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
            if luma > WAVE_CEILING {
                let scale = (WAVE_CEILING + (luma - WAVE_CEILING) * 0.35) / luma;
                c = [c[0] * scale, c[1] * scale, c[2] * scale];
            }

            // Interleaved gradient noise dither eliminates dark-gradient 8-bit banding.
            let ign = (52.982_918
                * ((px_i as f32 * 0.067_110_56 + py as f32 * 0.005_837_15).fract()))
            .fract();
            let dither = (ign - 0.5) / 255.0;
            let off = ((py * WAVE_W + px_i) * 4) as usize;
            rgba[off] = ((c[0] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba[off + 1] = ((c[1] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba[off + 2] = ((c[2] + dither).clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba[off + 3] = 255;
        }
    }

    rgba
}

pub(super) fn build_ambient_wave_image(
    colors: [[f32; 3]; 5],
    stage: [f32; 3],
    time: f32,
) -> Option<Arc<RenderImage>> {
    let rgba = build_ambient_wave_rgba(colors, stage, time);
    rgba_to_render_image(WAVE_W, WAVE_H, &rgba)
}

impl SonataDesktopView {
    /// Runs a single background drift/wash loop for the ambient wave backdrop while fullscreen is open.
    pub(super) fn ensure_ambient_drift_timer(&mut self, cx: &mut Context<Self>) {
        if self.fullscreen_ambient_timer_active || self.ui_suspended() {
            return;
        }
        self.fullscreen_ambient_timer_active = true;
        cx.spawn(async move |this, cx| loop {
            let job = this.update(cx, |view, _| {
                if view.ui_suspended()
                    || !view.fullscreen_open
                    || view.snapshot.config.reduce_motion
                    || (!view.fullscreen_ambient_washing && view.snapshot.paused)
                {
                    view.fullscreen_ambient_timer_active = false;
                    return None;
                }
                let wait_ms = if view.fullscreen_ambient_washing {
                    AMBIENT_WASH_MS
                } else {
                    AMBIENT_DRIFT_MS
                };
                let (colors, stage) = view
                    .fullscreen_ambient_painted
                    .or(view.fullscreen_ambient_target)
                    .unwrap_or_else(|| {
                        let (r, g, b) = view.snapshot.ambient_rgb;
                        resolve_ambient_palette(None, [r, g, b])
                    });
                let elapsed = view.fullscreen_ambient_started.elapsed().as_secs_f32();
                Some((wait_ms, colors, stage, elapsed))
            });
            let Ok(Some((wait_ms, colors, stage, elapsed))) = job else {
                break;
            };
            cx.background_executor()
                .timer(Duration::from_millis(wait_ms))
                .await;

            // Advance color wash right before synthesizing the next frame off the UI thread.
            let next_params = this.update(cx, |view, _| {
                if !view.fullscreen_open || view.snapshot.config.reduce_motion {
                    view.fullscreen_ambient_timer_active = false;
                    return None;
                }
                let target = view.fullscreen_ambient_target.unwrap_or((colors, stage));
                let now = Instant::now();
                let step = now
                    .duration_since(view.fullscreen_ambient_stepped)
                    .as_secs_f32()
                    .clamp(0.001, 0.5);
                view.fullscreen_ambient_stepped = now;
                let cur = view.fullscreen_ambient_painted.unwrap_or(target);
                let delta = 1.0 - (-step / AMBIENT_WASH_SECS).exp();
                let next_pal = blend_ambient_palette(cur, target, delta);
                view.fullscreen_ambient_painted = Some(next_pal);
                view.fullscreen_ambient_washing = !ambient_palette_settled(next_pal, target);
                let t = view.fullscreen_ambient_started.elapsed().as_secs_f32();
                Some((next_pal.0, next_pal.1, t.max(elapsed)))
            });
            let Ok(Some((next_colors, next_stage, next_t))) = next_params else {
                break;
            };

            let built = cx
                .background_executor()
                .spawn(async move { build_ambient_wave_image(next_colors, next_stage, next_t) })
                .await;

            let keep = this.update(cx, |view, cx| {
                if !view.fullscreen_open {
                    if let Some(tex) = built {
                        cx.drop_image(tex, None);
                    }
                    view.fullscreen_ambient_timer_active = false;
                    return false;
                }
                if let Some(tex) = built {
                    if let Some(old) = view.fullscreen_ambient_tex.replace(tex) {
                        cx.drop_image(old, None);
                    }
                    cx.notify();
                }
                true
            });
            if !matches!(keep, Ok(true)) {
                break;
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_wave_field_has_centered_noise_and_tonal_contrast() {
        let mut min_n = f32::INFINITY;
        let mut max_n = f32::NEG_INFINITY;
        for iy in -10..10 {
            for ix in -10..10 {
                let v = gradient_noise(ix as f32 * 0.43, iy as f32 * 0.37);
                min_n = min_n.min(v);
                max_n = max_n.max(v);
            }
        }
        assert!(
            min_n < -0.25 && max_n > 0.25,
            "gradient noise must span negative and positive ranges, got [{min_n}, {max_n}]"
        );

        let pal = CoverPalette {
            primary: Some((0.88, 0.90, 0.52)),
            secondary: Some((0.53, 0.88, 0.50)),
            lightness: 0.42,
        };
        let (colors, stage) = resolve_ambient_palette(Some(pal), [220, 40, 160]);
        let rgba = build_ambient_wave_rgba(colors, stage, 1.5);
        let mut min_luma = 255u8;
        let mut max_luma = 0u8;
        for px in rgba.as_chunks::<4>().0 {
            let l = (0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32) as u8;
            min_luma = min_luma.min(l);
            max_luma = max_luma.max(l);
        }
        assert!(
            max_luma.saturating_sub(min_luma) >= 20,
            "ambient wave field should exhibit visible tonal contrast, got min={min_luma} max={max_luma}"
        );
    }
}
