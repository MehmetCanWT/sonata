//! Fullscreen Now Playing stage with flowing ambient wave backdrop and word-synced lyrics:
//!
//! 1. **Ambient Wave Field**: generates a cached, dithered 5-color flowing wave backdrop from the active cover's dominant palette with corner vignette and readability luma ceiling.
//! 2. **Balanced 50/50 Cover Stage**: centers a generous rounded square cover (or motion video), mathematically centered Title + subtle `E` badge + muted Artist, and hover-only minimalist transport dock + bottom-right corner mode pill.
//! 3. **Pinned Depth-of-Field Lyrics**: anchors the active verse cleanly at 34% viewport height with smooth exponential scrolling, progressive karaoke sweeps, 3-note instrumental interludes, dual-voice right alignment, soft optical defocus, and transparent viewport-edge dissolves with zero opaque overlay boxes.

use super::{FullscreenDrawerTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::ui::components::{
    artist_links_row, cover_art_tile, is_parenthetical_bg, loading_ring, lyric_line_end_secs,
    resolve_active_lyrics,
};
use crate::ui::lyrics::{
    anchored_lyrics_offset, ease_out_expo, lag_row_spring, render_fixed_rows, render_karaoke_rows,
    KaraokeRenderStyle, SpringState, SPRING_LYRICS_SCROLL,
};
use crate::ui::media::{rgba_to_render_image, CoverPalette};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, img, point, prelude::FluentBuilder as _, px, AnyElement, BoxShadow, ClickEvent, Context,
    FontWeight, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, RenderImage,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};
use sonata_core::lyrics::{LyricLine, LyricVoice, LyricsState};
use sonata_core::media::format_mm_ss;
use sonata_core::player::RepeatMode;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---- Stage & Lyrics Geometry Constants --------------------------------------

const COVER_TALL_REST: f32 = 0.56;
const COVER_WIDE_REST: f32 = 0.40;
const COVER_TALL_AWAKE: f32 = 0.46;
const COVER_WIDE_AWAKE: f32 = 0.34;
const COVER_MIN: f32 = 160.0;
const COVER_MAX_REST: f32 = 560.0;
const COVER_MAX_AWAKE: f32 = 520.0;

const LYRICS_FRAME_W: f32 = 1152.0;
const LYRICS_FRAME_H: f32 = 648.0;
const LYRICS_PIN: f32 = 0.30;
const VERSE_GAP_FACTOR: f32 = 0.60;
const VERSE_PAD_X_FACTOR: f32 = 0.45;
const VERSE_PAD_Y_FACTOR: f32 = 0.15;
const ACTIVE_VERSE_GROWTH: f32 = 2.0;
const INSTRUMENTAL_BREAK_SECS: f64 = 5.0;
const WHEEL_SETTLE_SECS: u64 = 4;
const VERSE_TRANSITION_SECS: f32 = 0.22;

// ---- Ambient Wave Backdrop (5-color noise pools + vignette + luma ceiling) --

const WAVE_W: u32 = 160;
const WAVE_H: u32 = 90;
const WAVE_SHADE: f32 = 0.55;
const WAVE_PEAK: f32 = 0.72;
const WAVE_DENSITY: f32 = 3.0;
const WAVE_SOFTNESS: f32 = 2.2;
const WAVE_VIGNETTE: f32 = 0.60;
const WAVE_CEILING: f32 = 0.18;
const AMBIENT_WASH_SECS: f32 = 0.8;
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
fn resolve_ambient_palette(
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

fn ambient_palette_settled(
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

fn blend_ambient_palette(
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

fn build_ambient_wave_image(
    colors: [[f32; 3]; 5],
    stage: [f32; 3],
    time: f32,
) -> Option<Arc<RenderImage>> {
    let rgba = build_ambient_wave_rgba(colors, stage, time);
    rgba_to_render_image(WAVE_W, WAVE_H, &rgba)
}

// ---- Lyrics Display Rows ----------------------------------------------------

#[derive(Clone)]
enum DisplayLyricRow {
    Instrumental {
        start_secs: f64,
        end_secs: f64,
    },
    Verse {
        line_idx: usize,
        line: LyricLine,
        next_time: Option<f64>,
    },
}

fn build_display_rows(lines: &[LyricLine]) -> Vec<DisplayLyricRow> {
    let mut out = Vec::with_capacity(lines.len() + 6);
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 && line.time_secs >= INSTRUMENTAL_BREAK_SECS {
            out.push(DisplayLyricRow::Instrumental {
                start_secs: 0.0,
                end_secs: line.time_secs,
            });
        } else if idx > 0 {
            let prev = &lines[idx - 1];
            let prev_end = prev
                .words
                .last()
                .and_then(|w| w.end_secs)
                .unwrap_or(prev.time_secs + 3.0);
            if line.time_secs - prev_end >= INSTRUMENTAL_BREAK_SECS {
                out.push(DisplayLyricRow::Instrumental {
                    start_secs: prev_end,
                    end_secs: line.time_secs,
                });
            }
        }
        if !line.text.trim().is_empty() {
            let next_time = Some(lyric_line_end_secs(lines, idx));
            out.push(DisplayLyricRow::Verse {
                line_idx: idx,
                line: line.clone(),
                next_time,
            });
        }
    }
    out
}

impl SonataDesktopView {
    pub fn render_fullscreen_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
        let vp_h: f32 = vp.height.into();
        let stacked = vp_w < 820.0;
        let current = self.snapshot.current_track().cloned();
        let motion_tex = self.current_motion_image();
        let art_media = self
            .media_for_path(self.snapshot.art_path.as_deref(), cx)
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|t| self.media_for_url(t.thumbnail_url.as_deref(), cx))
            });

        let (dr, dg, db) = art_media
            .as_ref()
            .map(|m| m.dominant_rgb)
            .unwrap_or(self.snapshot.ambient_rgb);
        let dom_rgb = [dr, dg, db];
        let cover_palette = art_media.as_ref().map(|m| m.cover_palette);
        let target_ambient = resolve_ambient_palette(cover_palette, dom_rgb);
        let animates = !self.snapshot.config.reduce_motion;

        let target_changed = self.fullscreen_ambient_target != Some(target_ambient);
        self.fullscreen_ambient_target = Some(target_ambient);

        let now = Instant::now();
        let step_secs = now
            .duration_since(self.fullscreen_ambient_stepped)
            .as_secs_f32();
        let painted = match self.fullscreen_ambient_painted {
            Some(prev) if animates && step_secs < 1.5 => {
                let delta = 1.0 - (-step_secs / AMBIENT_WASH_SECS).exp();
                blend_ambient_palette(prev, target_ambient, delta)
            }
            _ => target_ambient,
        };
        self.fullscreen_ambient_stepped = now;
        self.fullscreen_ambient_washing =
            animates && !ambient_palette_settled(painted, target_ambient);
        self.fullscreen_ambient_painted = Some(painted);

        // Render immediately on first fullscreen frame or when static palette changes.
        if self.fullscreen_ambient_tex.is_none() || (!animates && target_changed) {
            let elapsed = if animates {
                self.fullscreen_ambient_started.elapsed().as_secs_f32()
            } else {
                0.0
            };
            if let Some(tex) = build_ambient_wave_image(painted.0, painted.1, elapsed) {
                if let Some(old) = self.fullscreen_ambient_tex.replace(tex) {
                    cx.drop_image(old, None);
                }
            }
        }
        if animates && (self.fullscreen_ambient_washing || !self.snapshot.paused) {
            self.ensure_ambient_drift_timer(cx);
        }
        let ambient_tex = self.fullscreen_ambient_tex.clone();

        let controls_visible = self.fullscreen_awake
            || self.progress_dragging
            || self.fullscreen_tab == FullscreenDrawerTab::Queue;
        let show_side_panel = self.fullscreen_tab != FullscreenDrawerTab::Cover;

        let art_size = if stacked {
            (vp_h * 0.34).clamp(160.0, 260.0)
        } else if controls_visible {
            (vp_h * COVER_TALL_AWAKE)
                .min(vp_w * COVER_WIDE_AWAKE)
                .clamp(COVER_MIN, COVER_MAX_AWAKE)
        } else {
            (vp_h * COVER_TALL_REST)
                .min(vp_w * COVER_WIDE_REST)
                .clamp(COVER_MIN, COVER_MAX_REST)
        };
        let art_radius = 16.0_f32;

        let cover_shadow = vec![BoxShadow {
            color: gpui_kit::black().opacity(0.48),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(42.0),
            spread_radius: px(0.0),
            inset: false,
        }];

        let cover_box = if let Some(tex) = motion_tex {
            div()
                .flex_shrink_0()
                .size(px(art_size))
                .rounded(px(art_radius))
                .overflow_hidden()
                .shadow(cover_shadow)
                .child(img(tex).size_full().object_fit(ObjectFit::Cover))
                .into_any_element()
        } else if let Some(ref media) = art_media {
            div()
                .flex_shrink_0()
                .size(px(art_size))
                .rounded(px(art_radius))
                .overflow_hidden()
                .shadow(cover_shadow)
                .child(
                    img(media.image.clone())
                        .size_full()
                        .rounded(px(art_radius))
                        .object_fit(ObjectFit::Cover),
                )
                .into_any_element()
        } else {
            div()
                .flex_shrink_0()
                .rounded(px(art_radius))
                .shadow(cover_shadow)
                .child(cover_art_tile(
                    "fs-art",
                    None,
                    art_size,
                    false,
                    &self.palette,
                    false,
                    None,
                ))
                .into_any_element()
        };

        // Balanced 3-part title row keeps title mathematically centered under artwork
        let track_meta_and_controls = if let Some(ref track) = current {
            let pos_secs = if self.progress_dragging {
                match self.progress_slider.read(cx).value() {
                    gpui_kit::component::slider::SliderValue::Single(x) => x as f64,
                    gpui_kit::component::slider::SliderValue::Range(_, e) => e as f64,
                }
            } else {
                self.live_position_secs
            };
            let track_like = track.clone();
            let repeat_icon = match self.snapshot.repeat {
                RepeatMode::One => IconName::Repeat1,
                _ => IconName::Repeat,
            };
            let title_size = if track.title.chars().count() > 34 {
                20.0
            } else {
                23.0
            };

            let explicit_el = |visible: bool| {
                div()
                    .flex_shrink_0()
                    .h(px(16.0))
                    .min_w(px(16.0))
                    .px(px(4.0))
                    .rounded(px(3.0))
                    .bg(gpui_kit::white().opacity(if visible { 0.14 } else { 0.0 }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(9.5))
                    .line_height(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(gpui_kit::white().opacity(if visible { 0.62 } else { 0.0 }))
                    .child("E")
            };

            div()
                .w(px(art_size.max(340.0)))
                .max_w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_end()
                                .when(track.is_explicit, |r| r.child(explicit_el(false))),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .max_w(px(art_size))
                                .text_size(px(title_size))
                                .line_height(px(title_size + 6.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(gpui_kit::white().opacity(0.96))
                                .text_center()
                                .truncate()
                                .child(track.title.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_start()
                                .when(track.is_explicit, |r| r.child(explicit_el(true))),
                        ),
                )
                .child(div().flex().justify_center().child(artist_links_row(
                    "fs-artist",
                    &track.artist,
                    gpui_kit::white().opacity(0.56),
                    14.0,
                    {
                        let view = cx.entity().downgrade();
                        move |name, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.open_artist_by_name(&name, cx);
                            });
                        }
                    },
                )))
                .when(controls_visible, |col| {
                    col.child(
                        div()
                            .id("fs-controls-dock")
                            .w_full()
                            .max_w(px(380.0))
                            .pt(px(14.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .on_click(|_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(Slider::new(&self.progress_slider).horizontal())
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_between()
                                            .text_size(px(11.0))
                                            .text_color(gpui_kit::white().opacity(0.50))
                                            .child(format_mm_ss(pos_secs.max(0.0).round() as u64))
                                            .child(format_mm_ss(
                                                self.live_duration_secs.max(0.0).round() as u64,
                                            )),
                                    ),
                            )
                            .child(
                                // Three-column symmetric transport row keeps the Play/Pause button
                                // mathematically centered under the cover, title, and scrubber.
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(14.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_end()
                                            .gap(px(12.0))
                                            .child(
                                                Button::new("fs-shuf")
                                                    .ghost()
                                                    .small()
                                                    .icon(IconName::Shuffle)
                                                    .text_color(if self.snapshot.shuffle {
                                                        gpui_kit::white()
                                                    } else {
                                                        gpui_kit::white().opacity(0.54)
                                                    })
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.backend
                                                            .send(BackendCmd::ToggleShuffle);
                                                    })),
                                            )
                                            .child(
                                                Button::new("fs-prev")
                                                    .ghost()
                                                    .icon(IconName::SkipBack)
                                                    .text_color(gpui_kit::white().opacity(0.90))
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.backend.send(BackendCmd::PrevTrack);
                                                    })),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("fs-play-btn")
                                            .flex_shrink_0()
                                            .relative()
                                            .size(px(42.0))
                                            .rounded_full()
                                            .bg(gpui_kit::white().opacity(0.94))
                                            .text_color(gpui_kit::black())
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.86))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.backend.send(BackendCmd::TogglePause);
                                            }))
                                            .when(self.snapshot.loading_track, |b| {
                                                b.child(loading_ring(
                                                    "fs-ring",
                                                    42.0,
                                                    &self.palette,
                                                ))
                                            })
                                            .child(
                                                Icon::new(if self.snapshot.paused {
                                                    IconName::Play
                                                } else {
                                                    IconName::Pause
                                                })
                                                .size(px(18.0)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_start()
                                            .gap(px(12.0))
                                            .child(
                                                Button::new("fs-next")
                                                    .ghost()
                                                    .icon(IconName::SkipForward)
                                                    .text_color(gpui_kit::white().opacity(0.90))
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.backend.send(BackendCmd::NextTrack);
                                                    })),
                                            )
                                            .child(
                                                Button::new("fs-rep")
                                                    .ghost()
                                                    .small()
                                                    .icon(repeat_icon)
                                                    .text_color(
                                                        if self.snapshot.repeat != RepeatMode::Off {
                                                            gpui_kit::white()
                                                        } else {
                                                            gpui_kit::white().opacity(0.54)
                                                        },
                                                    )
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.backend.send(BackendCmd::CycleRepeat);
                                                    })),
                                            )
                                            .child(
                                                Button::new("fs-like")
                                                    .ghost()
                                                    .small()
                                                    .icon(IconName::ThumbsUp)
                                                    .text_color(if track.liked {
                                                        gpui_kit::white()
                                                    } else {
                                                        gpui_kit::white().opacity(0.54)
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, _| {
                                                        this.backend.send(BackendCmd::ToggleLike(
                                                            track_like.clone(),
                                                        ));
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                })
        } else {
            div()
                .text_size(px(18.0))
                .text_color(gpui_kit::white().opacity(0.52))
                .child("Nothing playing")
        };

        let left_column = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(20.0))
            .child(cover_box)
            .child(track_meta_and_controls);

        let right_panel: Option<AnyElement> = match self.fullscreen_tab {
            FullscreenDrawerTab::Cover => None,
            FullscreenDrawerTab::Queue => Some(self.render_fullscreen_queue(cx)),
            FullscreenDrawerTab::Lyrics => {
                Some(self.render_fullscreen_lyrics(vp_w, vp_h, window, cx))
            }
        };

        // Subtle hover-only bottom-right mode pill + minimize button (zero top bar)
        let corner_pill = controls_visible.then(|| {
            let pill_btn = |id: &'static str,
                            icon: IconName,
                            active: bool,
                            tab: FullscreenDrawerTab,
                            cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .size(px(32.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .bg(if active {
                        gpui_kit::white().opacity(0.16)
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .hover(|s| s.bg(gpui_kit::white().opacity(0.12)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.fullscreen_tab = tab;
                        this.stir_fullscreen(cx);
                        cx.notify();
                    }))
                    .child(Icon::new(icon).size(px(15.0)).text_color(if active {
                        gpui_kit::white()
                    } else {
                        gpui_kit::white().opacity(0.58)
                    }))
            };

            div()
                .id("fs-corner")
                .absolute()
                .bottom(px(20.0))
                .left(px(24.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("fs-minimize-btn")
                        .size(px(38.0))
                        .rounded(px(11.0))
                        .bg(gpui_kit::black().opacity(0.36))
                        .border_1()
                        .border_color(gpui_kit::white().opacity(0.10))
                        .cursor_pointer()
                        .hover(|s| s.bg(gpui_kit::white().opacity(0.14)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.fullscreen_open = false;
                            if window.is_fullscreen() {
                                window.toggle_fullscreen();
                            }
                            cx.notify();
                        }))
                        .child(
                            Icon::new(IconName::Minimize2)
                                .size(px(15.0))
                                .text_color(gpui_kit::white().opacity(0.86)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(2.0))
                        .p(px(3.0))
                        .rounded(px(11.0))
                        .bg(gpui_kit::black().opacity(0.36))
                        .border_1()
                        .border_color(gpui_kit::white().opacity(0.10))
                        .child(pill_btn(
                            "fs-tab-cover",
                            IconName::Disc3,
                            self.fullscreen_tab == FullscreenDrawerTab::Cover,
                            FullscreenDrawerTab::Cover,
                            cx,
                        ))
                        .child(pill_btn(
                            "fs-tab-lyrics",
                            IconName::Mic,
                            self.fullscreen_tab == FullscreenDrawerTab::Lyrics,
                            FullscreenDrawerTab::Lyrics,
                            cx,
                        ))
                        .child(pill_btn(
                            "fs-tab-queue",
                            IconName::ListMusic,
                            self.fullscreen_tab == FullscreenDrawerTab::Queue,
                            FullscreenDrawerTab::Queue,
                            cx,
                        )),
                )
        });

        div()
            .id("fullscreen-overlay")
            .absolute()
            .inset_0()
            .bg(gpui_kit::rgb(0x0c0a0e))
            .on_click(cx.listener(|_, ev: &ClickEvent, window, _| {
                if ev.click_count() == 2 {
                    window.toggle_fullscreen();
                }
            }))
            .when_some(ambient_tex, |root, tex| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(img(tex).size_full().object_fit(ObjectFit::Fill)),
                )
            })
            .child(
                div()
                    .relative()
                    .size_full()
                    .px(px(if stacked { 24.0 } else { 32.0 }))
                    .flex()
                    .when(!stacked, |el| el.flex_row().gap(px(32.0)))
                    .when(stacked, |el| el.flex_col().gap(px(16.0)).py(px(20.0)))
                    .items_center()
                    .child(left_column)
                    .when_some(right_panel, |row, panel| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .h_full()
                                .flex()
                                .when(!show_side_panel, |d| d.hidden())
                                .child(panel),
                        )
                    }),
            )
            .children(corner_pill)
            .into_any_element()
    }

    fn render_fullscreen_lyrics(
        &mut self,
        vp_w: f32,
        vp_h: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lyrics = self.snapshot.lyrics.clone();
        let pos = self.live_position_now();
        let frame_scale = (vp_w / LYRICS_FRAME_W)
            .min(vp_h / LYRICS_FRAME_H)
            .clamp(1.0, 1.75);
        let verse_size = (25.0 * frame_scale).clamp(22.0, 36.0);
        let active_size = verse_size + ACTIVE_VERSE_GROWTH;
        let line_height = active_size * 1.30;
        let phrase_gap = verse_size * VERSE_GAP_FACTOR;
        let verse_pad_x = verse_size * VERSE_PAD_X_FACTOR;
        let verse_pad_y = verse_size * VERSE_PAD_Y_FACTOR;
        let sheet_pad_l = 8.0_f32;
        let sheet_pad_r = 28.0_f32;

        let scroll_bounds = self.fullscreen_lyrics_scroll.bounds();
        let view_w: f32 = scroll_bounds.size.width.into();
        let view_h: f32 = scroll_bounds.size.height.into();
        let view_top: f32 = scroll_bounds.origin.y.into();
        let effective_h = if view_h > 40.0 { view_h } else { vp_h };
        let effective_w = if view_w > 80.0 {
            view_w
        } else {
            (vp_w * 0.5 - 64.0).max(240.0)
        };
        let wrap_width = (effective_w - sheet_pad_l - sheet_pad_r - verse_pad_x * 2.0).max(120.0);

        let pad_top = (effective_h * LYRICS_PIN).max(96.0);
        let pad_bottom = (effective_h * (1.0 - LYRICS_PIN)).max(180.0);

        let user_scrolling = self
            .last_wheel_lyrics
            .is_some_and(|t| t.elapsed() < Duration::from_secs(WHEEL_SETTLE_SECS));

        let scroll_content = match lyrics {
            LyricsState::Synced(lines) => {
                if !self.snapshot.paused && !self.snapshot.loading_track && !lines.is_empty() {
                    self.ensure_karaoke_pace(cx);
                }

                let rewrapped =
                    self.fullscreen_lyrics_wrap
                        .sync_layout(&lines, wrap_width, active_size);
                if rewrapped {
                    window.request_animation_frame();
                }

                let (active_line_idx, active_bg_indices) = resolve_active_lyrics(&lines, pos);
                let rows = build_display_rows(&lines);

                // Find active display row (either active verse or active instrumental break)
                let active_display_row: Option<usize> = rows.iter().position(|r| match r {
                    DisplayLyricRow::Verse { line_idx, .. } => active_line_idx == Some(*line_idx),
                    DisplayLyricRow::Instrumental {
                        start_secs,
                        end_secs,
                    } => active_line_idx.is_none() && pos >= *start_secs && pos < *end_secs,
                });

                // Track arriving / departing active verse for smooth 220ms size & brightness transitions.
                if self.fullscreen_active_row_prev != active_display_row {
                    if self.fullscreen_active_row_prev.is_some() {
                        self.fullscreen_departing_row = self.fullscreen_active_row_prev;
                        self.fullscreen_row_departed = Instant::now();
                    }
                    if active_display_row.is_some() {
                        self.fullscreen_row_arrived = Instant::now();
                    }
                    self.fullscreen_active_row_prev = active_display_row;
                }
                if self.fullscreen_departing_row.is_some()
                    && self.fullscreen_row_departed.elapsed().as_secs_f32() >= VERSE_TRANSITION_SECS
                {
                    self.fullscreen_departing_row = None;
                }

                // Physical spring scroll toward PIN = 0.30 + cascading per-row lag stagger.
                let dt = self
                    .fullscreen_lyrics_last_step
                    .elapsed()
                    .as_secs_f32()
                    .clamp(0.001, 0.05);
                self.fullscreen_lyrics_last_step = Instant::now();

                let target_row = active_display_row.unwrap_or(0);
                let mut active_scroll_off: f32 = self.fullscreen_lyrics_scroll.offset().y.into();
                let mut scroll_animating = false;

                if user_scrolling {
                    self.fullscreen_scroll_spring = SpringState::at(active_scroll_off);
                    self.fullscreen_row_drifts.clear();
                } else if view_h > 40.0 {
                    // Direct children of fs-lyr-scroll: child 0 is top spacer, rows start at index 1.
                    let child_ix = target_row + 1;
                    if let Some(item_bounds) =
                        self.fullscreen_lyrics_scroll.bounds_for_item(child_ix)
                    {
                        let item_top: f32 = item_bounds.origin.y.into();
                        let item_h: f32 = item_bounds.size.height.into();
                        let max_off: f32 = self.fullscreen_lyrics_scroll.max_offset().y.into();
                        let goal = anchored_lyrics_offset(
                            view_top,
                            item_top,
                            view_h,
                            max_off,
                            LYRICS_PIN,
                            item_h * 0.16,
                        );
                        if (self.fullscreen_scroll_spring.position - active_scroll_off).abs() > 4.0
                            && self.fullscreen_scroll_spring.velocity.abs() < 1.0
                        {
                            self.fullscreen_scroll_spring = SpringState::at(active_scroll_off);
                        }
                        let diff = goal - self.fullscreen_scroll_spring.position;
                        if diff.abs() > view_h * 1.6 || self.snapshot.config.reduce_motion {
                            self.fullscreen_scroll_spring = SpringState::at(goal);
                            self.fullscreen_row_drifts.clear();
                        } else {
                            let next_spring =
                                SPRING_LYRICS_SCROLL.step(self.fullscreen_scroll_spring, goal, dt);
                            if SPRING_LYRICS_SCROLL.is_settled(next_spring, goal, 0.35) {
                                self.fullscreen_scroll_spring = SpringState::at(goal);
                            } else {
                                self.fullscreen_scroll_spring = next_spring;
                                scroll_animating = true;
                            }
                        }
                        let next_y = self
                            .fullscreen_scroll_spring
                            .position
                            .clamp(-max_off.max(0.0), 0.0);
                        active_scroll_off = next_y;
                        self.fullscreen_lyrics_scroll_y = next_y;
                        self.fullscreen_lyrics_scroll
                            .set_offset(point(px(0.0), px(next_y)));
                    } else {
                        scroll_animating = true;
                    }
                }

                // Cascading per-row spring stagger capped strictly below half the phrase gap so
                // parenthetical background lines never visually collide with neighboring verses.
                let scroll_step = active_scroll_off - self.fullscreen_last_seen_scroll;
                self.fullscreen_last_seen_scroll = active_scroll_off;
                let max_drift = (phrase_gap * 0.40).clamp(4.0, 12.0);
                let downward = scroll_step < 0.0;
                let mut any_drifting = false;

                let mut col = div()
                    .id("fs-lyr-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.fullscreen_lyrics_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(phrase_gap))
                    .pl(px(sheet_pad_l))
                    .pr(px(sheet_pad_r))
                    .child(div().flex_shrink_0().h(px(pad_top)));

                let edge_fade_px = (verse_size * 2.2).clamp(60.0, 132.0);
                let pin_y = effective_h * LYRICS_PIN;
                let arrive_t = ease_out_expo(
                    (self.fullscreen_row_arrived.elapsed().as_secs_f32() / VERSE_TRANSITION_SECS)
                        .clamp(0.0, 1.0),
                );
                let depart_t = ease_out_expo(
                    (self.fullscreen_row_departed.elapsed().as_secs_f32() / VERSE_TRANSITION_SECS)
                        .clamp(0.0, 1.0),
                );
                if arrive_t < 1.0 || (self.fullscreen_departing_row.is_some() && depart_t < 1.0) {
                    scroll_animating = true;
                }

                for (row_i, row) in rows.iter().enumerate() {
                    let child_ix = row_i + 1;
                    let hovered = self.fullscreen_lyrics_hovered_row == Some(row_i);

                    // Compute smooth transparent edge dissolve, distance haze, and offscreen culling from real viewport Y.
                    let mut culled_height: Option<f32> = None;
                    let (edge_alpha, haze, along) = if view_h > 40.0 {
                        if let Some(ib) = self.fullscreen_lyrics_scroll.bounds_for_item(child_ix) {
                            let rel_top = f32::from(ib.origin.y) + active_scroll_off - view_top;
                            let ih = f32::from(ib.size.height);
                            if !hovered
                                && active_display_row != Some(row_i)
                                && self.fullscreen_departing_row != Some(row_i)
                                && ih > 1.0
                                && (rel_top + ih < -140.0 || rel_top > view_h + 140.0)
                            {
                                culled_height = Some(ih);
                            }
                            let rel_mid = rel_top + ih * 0.5;
                            let top_fade = (rel_mid / edge_fade_px).clamp(0.0, 1.0);
                            let bot_fade = ((view_h - rel_mid) / edge_fade_px).clamp(0.0, 1.0);
                            let edge = (top_fade * bot_fade).clamp(0.0, 1.0);
                            let travel = (rel_mid - pin_y) / (view_h * 0.62).max(1.0);
                            let h_val = travel.abs().clamp(0.0, 1.0).powf(0.45);
                            let along_norm = (rel_top / view_h).clamp(0.0, 1.0);
                            let wave_along = if downward {
                                along_norm
                            } else {
                                1.0 - along_norm
                            };
                            (edge, h_val, wave_along)
                        } else {
                            let dist = active_display_row
                                .map(|a| row_i.abs_diff(a))
                                .unwrap_or(row_i);
                            (1.0, (dist as f32 / 5.0).clamp(0.0, 1.0).powf(0.45), 0.5)
                        }
                    } else {
                        let dist = active_display_row
                            .map(|a| row_i.abs_diff(a))
                            .unwrap_or(row_i);
                        (1.0, (dist as f32 / 5.0).clamp(0.0, 1.0).powf(0.45), 0.5)
                    };

                    if let Some(ih) = culled_height {
                        self.fullscreen_row_drifts.remove(&row_i);
                        col = col.child(div().flex_shrink_0().w_full().h(px(ih)));
                        continue;
                    }

                    // Step per-row stagger drift (wrapped so scroll handle keeps measuring rest bounds).
                    let row_drift_y = if !user_scrolling && !self.snapshot.config.reduce_motion {
                        let kick = if scroll_step.abs() > 0.15 {
                            (-scroll_step * 0.42 * along).clamp(-max_drift, max_drift)
                        } else {
                            0.0
                        };
                        let entry = self
                            .fullscreen_row_drifts
                            .entry(row_i)
                            .or_insert_with(|| SpringState::at(0.0));
                        entry.position = (entry.position + kick).clamp(-max_drift, max_drift);
                        let spring = lag_row_spring(along);
                        *entry = spring.step(*entry, 0.0, dt);
                        if spring.is_settled(*entry, 0.0, 0.15) {
                            self.fullscreen_row_drifts.remove(&row_i);
                            0.0
                        } else {
                            any_drifting = true;
                            entry.position
                        }
                    } else {
                        0.0
                    };

                    let wrap_with_drift = |inner: AnyElement, drift: f32| -> AnyElement {
                        if drift.abs() < 0.1 {
                            inner
                        } else {
                            div()
                                .w_full()
                                .min_w_0()
                                .child(
                                    div()
                                        .relative()
                                        .top(px(drift))
                                        .w_full()
                                        .min_w_0()
                                        .child(inner),
                                )
                                .into_any_element()
                        }
                    };

                    match row {
                        DisplayLyricRow::Instrumental {
                            start_secs,
                            end_secs,
                        } => {
                            let progress = if end_secs > start_secs {
                                ((pos - *start_secs) / (*end_secs - *start_secs)).clamp(0.0, 1.0)
                                    as f32
                            } else {
                                0.0
                            };
                            let past = pos >= *end_secs;
                            let is_active_break = active_display_row == Some(row_i);
                            let seek = *start_secs;
                            let base_alpha = if is_active_break || hovered {
                                1.0
                            } else {
                                edge_alpha * (1.0 - 0.30 * haze)
                            };

                            let mut notes = div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(12.0))
                                .py(px(verse_size * 0.32));

                            for note_idx in 0..3 {
                                let note_prog = (progress * 3.0 - note_idx as f32).clamp(0.0, 1.0);
                                let note_alpha = if past {
                                    0.26 * base_alpha
                                } else {
                                    (0.32 + 0.62 * note_prog) * base_alpha
                                };
                                notes = notes.child(
                                    div()
                                        .text_size(px(verse_size * 0.92))
                                        .line_height(px(verse_size * 1.1))
                                        .text_color(gpui_kit::white().opacity(note_alpha))
                                        .child("♪"),
                                );
                            }

                            let el = div()
                                .id(SharedString::from(format!("fs-brk-{row_i}")))
                                .w_full()
                                .min_w_0()
                                .px(px(verse_pad_x))
                                .rounded(px(10.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.07)))
                                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                                    this.fullscreen_lyrics_hovered_row =
                                        if *over { Some(row_i) } else { None };
                                    cx.notify();
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.last_wheel_lyrics = None;
                                    this.backend.send(BackendCmd::SeekAbsolute(seek));
                                }))
                                .child(notes)
                                .into_any_element();
                            col = col.child(wrap_with_drift(el, row_drift_y));
                        }
                        DisplayLyricRow::Verse {
                            line_idx,
                            line,
                            next_time,
                        } => {
                            let is_active_bg = active_bg_indices.contains(line_idx);
                            let is_active = active_line_idx == Some(*line_idx) || is_active_bg;
                            let is_departing =
                                !is_active && self.fullscreen_departing_row == Some(row_i);
                            let is_past =
                                !is_active_bg && active_line_idx.is_some_and(|a| *line_idx < a);
                            let is_bg = is_parenthetical_bg(&line.text);
                            let is_counter = line.voice == LyricVoice::Counter;
                            let seek = line.time_secs;

                            let base_sz = if is_bg { verse_size * 0.82 } else { verse_size };
                            let act_sz = if is_bg {
                                active_size * 0.86
                            } else {
                                active_size
                            };
                            let size = if is_active_bg {
                                act_sz
                            } else if is_active {
                                base_sz + (act_sz - base_sz) * arrive_t
                            } else if is_departing {
                                act_sz - (act_sz - base_sz) * depart_t
                            } else {
                                base_sz
                            };
                            let lh = act_sz * 1.32;

                            let wrapped = self.fullscreen_lyrics_wrap.get_or_measure(
                                *line_idx,
                                line,
                                *next_time,
                                active_size,
                                wrap_width,
                                window,
                            );

                            let content: AnyElement = if is_active {
                                let dim_alpha = if is_active_bg {
                                    0.42
                                } else {
                                    0.36 + 0.06 * arrive_t
                                };
                                let lit_alpha = if is_active_bg {
                                    0.98
                                } else {
                                    0.72 + 0.26 * arrive_t
                                };
                                if let Some(plan) = wrapped {
                                    render_karaoke_rows(
                                        plan,
                                        line,
                                        pos,
                                        KaraokeRenderStyle {
                                            font_size: size,
                                            line_height: lh,
                                            base_color: gpui_kit::white().opacity(dim_alpha),
                                            lit_color: gpui_kit::white().opacity(lit_alpha),
                                            is_counter,
                                            is_bg,
                                        },
                                    )
                                } else {
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .text_size(px(size))
                                        .line_height(px(lh))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(gpui_kit::white().opacity(lit_alpha))
                                        .child(line.text.clone())
                                        .into_any_element()
                                }
                            } else {
                                let rest_shade = if hovered {
                                    0.86
                                } else if is_past {
                                    0.40 * (1.0 - 0.30 * haze)
                                } else {
                                    0.58 * (1.0 - 0.30 * haze)
                                };
                                let shade = if is_departing {
                                    0.96 + (rest_shade - 0.96) * depart_t
                                } else {
                                    rest_shade
                                };
                                let alpha = if hovered {
                                    shade * edge_alpha.max(0.35)
                                } else {
                                    let bg_mul = if is_bg { 0.82 } else { 1.0 };
                                    (shade * bg_mul * edge_alpha).clamp(0.0, 1.0)
                                };
                                let blur_r = if hovered || user_scrolling {
                                    0.0
                                } else {
                                    let b = (verse_size * 0.08 * haze).clamp(0.0, 2.35);
                                    if is_departing {
                                        b * depart_t
                                    } else {
                                        b
                                    }
                                };
                                if let Some(plan) = wrapped {
                                    render_fixed_rows(
                                        &plan.text,
                                        size,
                                        lh,
                                        gpui_kit::white().opacity(alpha),
                                        blur_r,
                                        is_counter,
                                        is_bg,
                                    )
                                } else {
                                    let fallback = [SharedString::from(line.text.clone())];
                                    render_fixed_rows(
                                        &fallback,
                                        size,
                                        lh,
                                        gpui_kit::white().opacity(alpha),
                                        blur_r,
                                        is_counter,
                                        is_bg,
                                    )
                                }
                            };

                            let el = div()
                                .id(SharedString::from(format!("fs-lyr-{row_i}")))
                                .w_full()
                                .min_w_0()
                                .px(px(verse_pad_x))
                                .py(px(verse_pad_y))
                                .rounded(px(10.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.07)))
                                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                                    this.fullscreen_lyrics_hovered_row =
                                        if *over { Some(row_i) } else { None };
                                    cx.notify();
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.last_wheel_lyrics = None;
                                    this.backend.send(BackendCmd::SeekAbsolute(seek));
                                }))
                                .flex()
                                .flex_row()
                                .when(!is_counter, |el| el.justify_start())
                                .when(is_counter, |el| el.justify_end())
                                .child(content)
                                .into_any_element();
                            col = col.child(wrap_with_drift(el, row_drift_y));
                        }
                    }
                }

                if scroll_animating || any_drifting {
                    window.request_animation_frame();
                }

                col.child(div().flex_shrink_0().h(px(pad_bottom)))
                    .into_any_element()
            }
            LyricsState::Plain(lines) => div()
                .id("fs-lyr-plain-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.fullscreen_lyrics_scroll)
                .px(px(24.0))
                .py(px(pad_top * 0.5))
                .text_size(px(22.0))
                .line_height(px(line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui_kit::white().opacity(0.84))
                .child(lines.join("\n"))
                .into_any_element(),
            LyricsState::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(24.0))
                .text_color(gpui_kit::white().opacity(0.45))
                .child("♪  ♪  ♪")
                .into_any_element(),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(17.0))
                .text_color(gpui_kit::white().opacity(0.40))
                .child("No lyrics available for this track")
                .into_any_element(),
        };

        div()
            .flex_1()
            .h_full()
            .min_h_0()
            .relative()
            .on_scroll(cx.listener(|this, _, _, cx| {
                this.last_wheel_lyrics = Some(Instant::now());
                this.stir_fullscreen(cx);
            }))
            .child(scroll_content)
            .into_any_element()
    }

    fn render_fullscreen_queue(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let queue = self.snapshot.queue.clone();
        let current_idx = self.snapshot.current_index;
        let mut rows = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(12.0))
            .py(px(40.0));
        const MAX_FS_QUEUE: usize = 80;
        let (win_start, win_end) = if queue.len() > MAX_FS_QUEUE {
            let cur = current_idx.unwrap_or(0);
            let start = cur.saturating_sub(15);
            let end = (start + MAX_FS_QUEUE).min(queue.len());
            (end.saturating_sub(MAX_FS_QUEUE), end)
        } else {
            (0, queue.len())
        };
        for (i, t) in queue
            .into_iter()
            .enumerate()
            .skip(win_start)
            .take(win_end - win_start)
        {
            let is_cur = current_idx == Some(i);
            let cover = self.media_for_url(t.thumbnail_url.as_deref(), cx);
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("fs-q-{i}")))
                    .h(px(54.0))
                    .px(px(12.0))
                    .rounded(px(10.0))
                    .cursor_pointer()
                    .bg(if is_cur {
                        gpui_kit::white().opacity(0.12)
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .hover(|s| s.bg(gpui_kit::white().opacity(0.08)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.backend.send(BackendCmd::PlayQueueIndex(i));
                    }))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(cover_art_tile(
                        SharedString::from(format!("fs-qc-{i}")),
                        cover.as_ref(),
                        40.0,
                        false,
                        &self.palette,
                        is_cur && !self.snapshot.paused,
                        None,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(if is_cur {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_cur {
                                        gpui_kit::white()
                                    } else {
                                        gpui_kit::white().opacity(0.84)
                                    })
                                    .truncate()
                                    .child(t.title),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(gpui_kit::white().opacity(0.52))
                                    .truncate()
                                    .child(t.artist),
                            ),
                    ),
            );
        }

        div()
            .flex_1()
            .h_full()
            .min_h_0()
            .child(
                div()
                    .id("fs-q-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.fullscreen_queue_scroll)
                    .child(rows),
            )
            .into_any_element()
    }

    /// Runs a single background drift/wash loop for the ambient wave backdrop while fullscreen is open.
    fn ensure_ambient_drift_timer(&mut self, cx: &mut Context<Self>) {
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
    fn anchored_lyrics_offset_pins_active_row_without_drift() {
        let view_top = 0.0;
        let view_h = 900.0;
        let max_off = 2400.0;
        let item_top = 980.0; // Unscrolled GPUI child origin.y
        let optical_shift = 8.0;

        let goal1 = anchored_lyrics_offset(
            view_top,
            item_top,
            view_h,
            max_off,
            LYRICS_PIN,
            optical_shift,
        );
        // Repeated frames with the same unscrolled bounds must yield the exact same target offset.
        let goal2 = anchored_lyrics_offset(
            view_top,
            item_top,
            view_h,
            max_off,
            LYRICS_PIN,
            optical_shift,
        );
        assert_eq!(goal1, goal2);
        let screen_rel_top = item_top + goal1 - view_top;
        assert!(
            (screen_rel_top - (view_h * LYRICS_PIN - optical_shift)).abs() < 1e-3,
            "expected pinned screen Y {}, got {}",
            view_h * LYRICS_PIN - optical_shift,
            screen_rel_top
        );
    }

    #[test]
    fn anchored_lyrics_offset_clamps_to_scroll_bounds() {
        assert_eq!(
            anchored_lyrics_offset(0.0, 120.0, 900.0, 1000.0, LYRICS_PIN, 0.0),
            0.0
        );
        assert_eq!(
            anchored_lyrics_offset(0.0, 4000.0, 900.0, 1000.0, LYRICS_PIN, 0.0),
            -1000.0
        );
    }

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
