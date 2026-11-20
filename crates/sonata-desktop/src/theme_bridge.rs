//! Bridges a `sonata-core` `Theme` into the translucent "glass" color palette used by the GPUI views.
//!
//! 1. **GlassPalette**: the named `Hsla` colors (window, sidebar, panel, card, player bar, modal, text, accent, lyrics, and more) consumed by every view.
//! 2. **Derivation**: `GlassPalette::from_theme` mixes the theme base colors with the cover-derived ambient color, using lower alphas when the window is transparent.
//! 3. **Color Helpers**: RGB-to-`Hsla` conversion, RGB mixing, and a readable-foreground picker for text on accent surfaces.

use gpui_kit::{rgba, Hsla};
use sonata_core::theme::{RgbColor, Theme};

#[derive(Clone, Debug)]
pub struct GlassPalette {
    pub window_bg: Hsla,
    pub ambient_top: Hsla,
    pub ambient_glow: Hsla,
    pub sidebar_bg: Hsla,
    pub panel_bg: Hsla,
    pub card_bg: Hsla,
    pub card_hover: Hsla,
    pub player_bar_bg: Hsla,
    pub modal_bg: Hsla,
    pub pill_bg: Hsla,
    pub border: Hsla,
    pub border_subtle: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub accent_fg: Hsla,
    pub highlight_bg: Hsla,
    pub highlight_fg: Hsla,
    pub lyrics_active: Hsla,
    pub lyrics_mid: Hsla,
    pub lyrics_inactive: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
}

fn to_u32_rgba((r, g, b): (u8, u8, u8), alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a
}

pub fn rgb_hsla(rgb: (u8, u8, u8), alpha: f32) -> Hsla {
    rgba(to_u32_rgba(rgb, alpha)).into()
}

fn core_hsla(c: RgbColor, alpha: f32, fallback: (u8, u8, u8)) -> Hsla {
    rgb_hsla(c.to_rgb(fallback), alpha)
}

fn mix_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    (
        (a.0 as f32 * inv + b.0 as f32 * t).round() as u8,
        (a.1 as f32 * inv + b.1 as f32 * t).round() as u8,
        (a.2 as f32 * inv + b.2 as f32 * t).round() as u8,
    )
}

fn readable_foreground(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let linear = |channel: u8| {
        let channel = f32::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(rgb.0) + 0.7152 * linear(rgb.1) + 0.0722 * linear(rgb.2);
    if luminance > 0.35 {
        (18, 18, 24)
    } else {
        (255, 255, 255)
    }
}

impl GlassPalette {
    pub fn from_theme(base: &Theme, transparent: bool, ambient_rgb: (u8, u8, u8)) -> Self {
        let base_bg = base.bg.to_rgb((0x1a, 0x1a, 0x24));
        let base_surface = base.surface.to_rgb((0x14, 0x15, 0x1e));
        let base_hl = base.highlight_bg.to_rgb((0x2e, 0x30, 0x42));
        let base_fg = base.fg.to_rgb((0xe6, 0xed, 0xf5));
        let base_accent = base.accent.to_rgb((0xcb, 0xa6, 0xf7));

        let (w_alpha, side_alpha, panel_alpha, bar_alpha, amb_alpha) = if transparent {
            (0.66, 0.50, 0.42, 0.72, 0.28)
        } else {
            (0.98, 0.94, 0.92, 0.96, 0.34)
        };

        let tinted_ambient = mix_rgb(base_bg, ambient_rgb, 0.72);
        let modal_rgb = mix_rgb(base_surface, (16, 16, 22), 0.25);

        Self {
            window_bg: rgb_hsla(base_bg, w_alpha),
            ambient_top: rgb_hsla(tinted_ambient, amb_alpha),
            ambient_glow: rgb_hsla(ambient_rgb, if transparent { 0.22 } else { 0.28 }),
            sidebar_bg: rgb_hsla(base_surface, side_alpha),
            panel_bg: rgb_hsla(base_surface, panel_alpha),
            card_bg: rgb_hsla(base_hl, if transparent { 0.36 } else { 0.65 }),
            card_hover: rgb_hsla(base_hl, if transparent { 0.58 } else { 0.85 }),
            player_bar_bg: rgb_hsla(mix_rgb(base_surface, ambient_rgb, 0.08), bar_alpha),
            modal_bg: rgb_hsla(modal_rgb, 0.95),
            pill_bg: rgb_hsla(base_fg, 0.09),
            border: core_hsla(
                base.border,
                if transparent { 0.42 } else { 0.65 },
                (0x45, 0x47, 0x5a),
            ),
            border_subtle: rgb_hsla((255, 255, 255), if transparent { 0.09 } else { 0.07 }),
            text: core_hsla(base.fg, 1.0, (0xcd, 0xd6, 0xf4)),
            muted: core_hsla(base.muted, 1.0, (0x7c, 0x82, 0x99)),
            accent: rgb_hsla(base_accent, 1.0),
            accent_soft: rgb_hsla(base_accent, 0.22),
            accent_fg: rgb_hsla(readable_foreground(base_accent), 1.0),
            highlight_bg: rgb_hsla(base_hl, if transparent { 0.68 } else { 0.90 }),
            highlight_fg: core_hsla(base.highlight_fg, 1.0, (0xff, 0xff, 0xff)),
            lyrics_active: core_hsla(base.lyrics_active, 1.0, (255, 255, 255)),
            lyrics_mid: rgb_hsla(base_fg, 0.62),
            lyrics_inactive: core_hsla(base.lyrics_inactive, 0.44, (0x6c, 0x70, 0x86)),
            success: core_hsla(base.success, 1.0, (0xa6, 0xe3, 0xa1)),
            warning: core_hsla(base.warning, 1.0, (0xf9, 0xe2, 0xaf)),
            error: core_hsla(base.error, 1.0, (0xf3, 0x8b, 0xa8)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_palette_adapts_alpha_and_ambient_tint() {
        let theme = Theme::catppuccin_mocha();
        let glass = GlassPalette::from_theme(&theme, true, (240, 80, 140));
        let opaque = GlassPalette::from_theme(&theme, false, (240, 80, 140));
        assert!(glass.window_bg.a < opaque.window_bg.a);
        assert!(glass.ambient_top.a > 0.2);
    }
}
