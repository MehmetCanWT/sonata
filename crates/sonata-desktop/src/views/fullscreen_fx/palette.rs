//! Album cover hue clustering and ambient stage palette derivation.
//!
//! 1. **Circular Hue Clustering**: `sample_cover_palette_rgba` samples up to 6,000 pixels into 24 hue bins weighted by saturation and lightness.
//! 2. **Primary & Secondary Selection**: picks the strongest bin cluster as the primary hue and searches >= 3 bins away for a distinct secondary hue.
//! 3. **Wave Palette Resolution**: `resolve_wave_palette` expands the `CoverPalette` into 5 shaded wave colors and a darkened stage base color.

use gpui_kit::{Hsla, Rgba};
use std::f32::consts::TAU;
use std::path::Path;

pub(super) const WAVE_COLORS: usize = 5;

const PALETTE_BINS: usize = 24;
const PALETTE_SAMPLES: usize = 6000;
const PALETTE_MIN_ALPHA: u8 = 128;
const PALETTE_MIN_SATURATION: f32 = 0.14;
const PALETTE_MIN_LIGHTNESS: f32 = 0.10;
const PALETTE_MAX_LIGHTNESS: f32 = 0.94;
const PALETTE_MIN_SHARE: f32 = 0.01;
const PALETTE_MIN_WEIGHT: f32 = 25.0;
const SECONDARY_SHARE: f32 = 0.02;
const SECONDARY_WEIGHT: f32 = 10.0;
const SECONDARY_GAP: usize = 3;
const SECONDARY_HUE: f32 = 0.05;
const SECONDARY_SPREAD: f32 = 0.05;
const SECONDARY_VIVID: f32 = 0.35;

/// Extracted primary and runner-up hues of an album cover.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoverPalette {
    pub primary: Option<Hsla>,
    pub secondary: Option<Hsla>,
}

#[derive(Clone, Copy, Default)]
struct HueBin {
    weight: f32,
    count: f32,
    x: f32,
    y: f32,
    saturation: f32,
    lightness: f32,
}

impl HueBin {
    fn add(&mut self, color: Hsla, weight: f32) {
        let angle = color.h * TAU;
        self.weight += weight;
        self.count += 1.0;
        self.x += angle.cos() * weight;
        self.y += angle.sin() * weight;
        self.saturation += color.s * weight;
        self.lightness += color.l * weight;
    }

    fn merge(&mut self, other: &Self) {
        self.weight += other.weight;
        self.count += other.count;
        self.x += other.x;
        self.y += other.y;
        self.saturation += other.saturation;
        self.lightness += other.lightness;
    }

    fn color(&self) -> Option<Hsla> {
        (self.weight > 0.0).then(|| Hsla {
            h: self.y.atan2(self.x).rem_euclid(TAU) / TAU,
            s: (self.saturation / self.weight).clamp(0.0, 1.0),
            l: (self.lightness / self.weight).clamp(0.0, 1.0),
            a: 1.0,
        })
    }
}

fn bin_score(bins: &[HueBin; PALETTE_BINS], index: usize) -> f32 {
    bins[index].weight
        + (bins[(index + PALETTE_BINS - 1) % PALETTE_BINS].weight
            + bins[(index + 1) % PALETTE_BINS].weight)
            * 0.5
}

fn bin_vivid(bins: &[HueBin; PALETTE_BINS], index: usize, sampled: f32) -> bool {
    let mut cluster = bins[index];
    cluster.merge(&bins[(index + PALETTE_BINS - 1) % PALETTE_BINS]);
    cluster.merge(&bins[(index + 1) % PALETTE_BINS]);
    cluster.weight > 0.0
        && cluster.count >= sampled * SECONDARY_SPREAD
        && cluster.saturation / cluster.weight >= SECONDARY_VIVID
}

/// Extracts primary and secondary hues from RGBA pixel bytes.
pub fn sample_cover_palette_rgba(rgba_pixels: &[u8]) -> CoverPalette {
    let pixel_count = rgba_pixels.len() / 4;
    if pixel_count == 0 {
        return CoverPalette::default();
    }
    let stride = (pixel_count / PALETTE_SAMPLES).max(1);
    let mut bins = [HueBin::default(); PALETTE_BINS];
    let mut sampled = 0.0f32;

    for &[red, green, blue, alpha] in rgba_pixels.as_chunks::<4>().0.iter().step_by(stride) {
        if alpha < PALETTE_MIN_ALPHA {
            continue;
        }
        sampled += 1.0;

        let color = Hsla::from(Rgba {
            r: f32::from(red) / 255.0,
            g: f32::from(green) / 255.0,
            b: f32::from(blue) / 255.0,
            a: 1.0,
        });

        if color.s < PALETTE_MIN_SATURATION
            || color.l < PALETTE_MIN_LIGHTNESS
            || color.l > PALETTE_MAX_LIGHTNESS
        {
            continue;
        }

        let idx = ((color.h * PALETTE_BINS as f32) as usize).min(PALETTE_BINS - 1);
        bins[idx].add(color, color.s * (0.3 + 0.7 * color.l));
    }

    let peak =
        (0..PALETTE_BINS).max_by(|&a, &b| bin_score(&bins, a).total_cmp(&bin_score(&bins, b)));
    let primary_idx = peak
        .filter(|&p| bin_score(&bins, p) >= (sampled * PALETTE_MIN_SHARE).max(PALETTE_MIN_WEIGHT));
    let Some(peak) = primary_idx else {
        return CoverPalette::default();
    };

    let mut cluster = bins[peak];
    cluster.merge(&bins[(peak + PALETTE_BINS - 1) % PALETTE_BINS]);
    cluster.merge(&bins[(peak + 1) % PALETTE_BINS]);
    let primary = cluster.color();

    let runner = (0..PALETTE_BINS)
        .filter(|&index| {
            let gap = index
                .abs_diff(peak)
                .min(PALETTE_BINS - index.abs_diff(peak));
            gap >= SECONDARY_GAP
                && (bin_score(&bins, index) >= (sampled * SECONDARY_SHARE).max(SECONDARY_WEIGHT)
                    || bin_vivid(&bins, index, sampled))
        })
        .max_by(|&a, &b| bin_score(&bins, a).total_cmp(&bin_score(&bins, b)));

    let secondary = runner
        .and_then(|index| {
            let mut c = bins[index];
            c.merge(&bins[(index + PALETTE_BINS - 1) % PALETTE_BINS]);
            c.merge(&bins[(index + 1) % PALETTE_BINS]);
            c.color()
        })
        .filter(|color| {
            primary.is_some_and(|lead| {
                let d = (color.h - lead.h).abs();
                d.min(1.0 - d) >= SECONDARY_HUE
            })
        });

    CoverPalette { primary, secondary }
}

/// Decodes the cover image at `path` and extracts its palette.
pub fn extract_cover_palette(path: &Path) -> Option<CoverPalette> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.into_rgba8();
    Some(sample_cover_palette_rgba(rgba.as_raw()))
}

pub(super) fn palette_from_fallback_rgb((r, g, b): (u8, u8, u8)) -> CoverPalette {
    let color = Hsla::from(Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    });
    if color.s < 0.08 {
        CoverPalette::default()
    } else {
        CoverPalette {
            primary: Some(color),
            secondary: None,
        }
    }
}

/// Derives the 5 wave colors and darkened stage color from a cover palette.
pub(super) fn resolve_wave_palette(palette: CoverPalette) -> ([Hsla; WAVE_COLORS], Hsla) {
    let neutral = |light: f32| Hsla {
        h: 0.06,
        s: 0.12,
        l: light,
        a: 1.0,
    };

    let stage_bg = match palette.primary {
        Some(lead) => Hsla {
            h: lead.h,
            s: (lead.s * 0.42).clamp(0.14, 0.38),
            l: 0.075,
            a: 1.0,
        },
        None => Hsla {
            h: 0.06,
            s: 0.08,
            l: 0.07,
            a: 1.0,
        },
    };

    let floor = (0.13 + stage_bg.l * 0.8).clamp(0.13, 0.30);

    let colors = match palette.primary {
        None => [
            neutral(floor),
            neutral(floor + 0.025),
            neutral(floor + 0.05),
            neutral(floor + 0.015),
            neutral(floor + 0.04),
        ],
        Some(accent) => {
            let base = Hsla {
                h: accent.h,
                s: (accent.s * 0.9 + 0.04).clamp(0.45, 0.78),
                l: (0.40 + (accent.l - 0.45) * 0.5).clamp(0.28, 0.52),
                a: 1.0,
            };
            let second = match palette.secondary {
                Some(sec) => Hsla {
                    h: sec.h,
                    s: (sec.s * 0.9).clamp(0.40, 0.72),
                    l: (0.38 + (sec.l - 0.45) * 0.5).clamp(0.28, 0.50),
                    a: 1.0,
                },
                None => neutral(floor + 0.03),
            };
            [
                base,
                Hsla {
                    l: (base.l + 0.14).clamp(0.30, 0.62),
                    s: (base.s - 0.08).clamp(0.40, 0.75),
                    ..base
                },
                second,
                Hsla {
                    l: (base.l - 0.13).clamp(0.18, 0.45),
                    s: (base.s + 0.04).clamp(0.45, 0.80),
                    ..base
                },
                Hsla {
                    h: (base.h + 0.97).rem_euclid(1.0),
                    l: (base.l - 0.16).clamp(0.16, 0.40),
                    s: (base.s - 0.02).clamp(0.40, 0.78),
                    ..base
                },
            ]
        }
    };

    (colors, stage_bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_finds_dominant_magenta_hue() {
        let mut pixels = Vec::with_capacity(1000 * 4);
        for _ in 0..850 {
            pixels.extend_from_slice(&[215, 70, 175, 255]);
        }
        for _ in 0..150 {
            pixels.extend_from_slice(&[24, 24, 28, 255]);
        }
        let pal = sample_cover_palette_rgba(&pixels);
        let primary = pal.primary.expect("magenta cover should yield primary hue");
        assert!(primary.s > 0.4);
        assert!(primary.h > 0.78 && primary.h < 0.96, "hue={}", primary.h);
    }
}
