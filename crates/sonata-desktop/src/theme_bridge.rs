//! Bridges a `sonata-core` `Theme` into the skin-aware color palette used by the GPUI views.
//!
//! 1. **DesktopSkin**: classifies the active theme into `Sonata` (Apple Music / Sonora glass), `Spotify`, or `YouTubeMusic` for authentic shell and control styling.
//! 2. **GlassPalette**: the named `Hsla` surface, divider, text, accent, and control tokens consumed by every view.
//! 3. **Derivation & Helpers**: `GlassPalette::from_theme` computes luminance-aware hairlines, ambient tinting, and readable foreground colors.

use gpui_kit::{rgba, Hsla};
use sonata_core::theme::{RgbColor, Theme};

/// Visual layout skin derived from the active desktop theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopSkin {
    /// Apple Music / Sonora-inspired borderless frosted glass workspace.
    Sonata,
    /// Authentic Spotify Desktop layout (`#000000` shell, `#121212` rounded panels, `#1ED760` green accents).
    Spotify,
    /// Authentic YouTube Music Desktop layout (`#030303` canvas, hairline dividers, `#212121` bar with top red scrubber).
    YouTubeMusic,
}

impl DesktopSkin {
    pub fn from_theme_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("spotify") {
            Self::Spotify
        } else if lower.contains("youtube") || lower.contains("ytm") {
            Self::YouTubeMusic
        } else {
            Self::Sonata
        }
    }

    pub fn badge_label(self) -> &'static str {
        match self {
            Self::Sonata => "Glass",
            Self::Spotify => "Spotify UI",
            Self::YouTubeMusic => "YTM UI",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GlassPalette {
    pub skin: DesktopSkin,
    #[allow(dead_code)]
    pub is_light: bool,
    pub shell_bg: Hsla,
    pub window_bg: Hsla,
    pub ambient_top: Hsla,
    pub ambient_glow: Hsla,
    pub sidebar_bg: Hsla,
    pub panel_bg: Hsla,
    pub card_bg: Hsla,
    pub card_hover: Hsla,
    pub search_bg: Hsla,
    pub player_bar_bg: Hsla,
    pub modal_bg: Hsla,
    pub pill_bg: Hsla,
    pub divider: Hsla,
    pub border: Hsla,
    pub border_subtle: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub accent_fg: Hsla,
    pub play_btn_bg: Hsla,
    pub play_btn_fg: Hsla,
    pub cta_bg: Hsla,
    pub cta_fg: Hsla,
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

fn relative_luminance(rgb: (u8, u8, u8)) -> f32 {
    let linear = |channel: u8| {
        let channel = f32::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb.0) + 0.7152 * linear(rgb.1) + 0.0722 * linear(rgb.2)
}

fn readable_foreground(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    if relative_luminance(rgb) > 0.35 {
        (16, 16, 20)
    } else {
        (255, 255, 255)
    }
}

impl GlassPalette {
    pub fn from_theme(base: &Theme, transparent: bool, ambient_rgb: (u8, u8, u8)) -> Self {
        let skin = DesktopSkin::from_theme_name(&base.name);
        let base_bg = base.bg.to_rgb((0x0d, 0x0e, 0x12));
        let base_surface = base.surface.to_rgb((0x14, 0x15, 0x1b));
        let base_hl = base.highlight_bg.to_rgb((0x22, 0x24, 0x2e));
        let base_fg = base.fg.to_rgb((0xf2, 0xf4, 0xf8));
        let base_accent = base.accent.to_rgb((0xfa, 0x2d, 0x48));
        let is_light = relative_luminance(base_bg) > 0.45;
        let line_rgb = if is_light { (0, 0, 0) } else { (255, 255, 255) };

        let (w_alpha, side_alpha, panel_alpha, bar_alpha, amb_alpha) = if transparent {
            match skin {
                DesktopSkin::Sonata => (0.68, 0.48, 0.28, 0.78, 0.24),
                DesktopSkin::Spotify => (0.78, 0.72, 0.72, 0.84, 0.22),
                DesktopSkin::YouTubeMusic => (0.78, 0.62, 0.50, 0.88, 0.20),
            }
        } else {
            match skin {
                DesktopSkin::Sonata => (0.98, 0.96, 0.94, 0.96, 0.28),
                DesktopSkin::Spotify => (1.0, 1.0, 1.0, 1.0, 0.26),
                DesktopSkin::YouTubeMusic => (1.0, 1.0, 1.0, 1.0, 0.22),
            }
        };

        let tinted_ambient = match skin {
            DesktopSkin::Spotify if !is_light => mix_rgb(base_surface, ambient_rgb, 0.42),
            DesktopSkin::YouTubeMusic if !is_light => mix_rgb(base_bg, ambient_rgb, 0.26),
            _ => mix_rgb(base_bg, ambient_rgb, 0.58),
        };
        let modal_rgb = if is_light {
            base_surface
        } else {
            mix_rgb(base_surface, (16, 16, 20), 0.22)
        };

        let (shell_bg, sidebar_bg, panel_bg, card_bg, card_hover, search_bg, player_bar_bg) =
            match skin {
                DesktopSkin::Spotify => {
                    let card_rgb = if is_light {
                        mix_rgb(base_surface, (0, 0, 0), 0.04)
                    } else {
                        (0x18, 0x18, 0x18)
                    };
                    let hover_rgb = if is_light {
                        mix_rgb(base_surface, (0, 0, 0), 0.08)
                    } else {
                        (0x28, 0x28, 0x28)
                    };
                    let search_rgb = if is_light {
                        (0xe8, 0xeb, 0xe8)
                    } else {
                        (0x24, 0x24, 0x24)
                    };
                    (
                        rgb_hsla(base_bg, w_alpha),
                        rgb_hsla(base_surface, side_alpha),
                        rgb_hsla(base_surface, panel_alpha),
                        rgb_hsla(card_rgb, if transparent { 0.65 } else { 1.0 }),
                        rgb_hsla(hover_rgb, if transparent { 0.78 } else { 1.0 }),
                        rgb_hsla(search_rgb, if transparent { 0.75 } else { 1.0 }),
                        rgb_hsla(base_bg, bar_alpha),
                    )
                }
                DesktopSkin::YouTubeMusic => {
                    let bar_rgb = if is_light {
                        base_surface
                    } else {
                        (0x21, 0x21, 0x21)
                    };
                    let card_rgb = if is_light {
                        mix_rgb(base_surface, (0, 0, 0), 0.04)
                    } else {
                        (0x16, 0x16, 0x16)
                    };
                    let hover_rgb = if is_light {
                        mix_rgb(base_surface, (0, 0, 0), 0.08)
                    } else {
                        (0x27, 0x27, 0x27)
                    };
                    let search_rgb = if is_light {
                        (0xee, 0xee, 0xee)
                    } else {
                        (0x21, 0x21, 0x21)
                    };
                    (
                        rgb_hsla(base_bg, w_alpha),
                        rgb_hsla(base_bg, side_alpha),
                        rgb_hsla(base_bg, panel_alpha),
                        rgb_hsla(card_rgb, if transparent { 0.52 } else { 0.92 }),
                        rgb_hsla(hover_rgb, if transparent { 0.74 } else { 1.0 }),
                        rgb_hsla(search_rgb, if transparent { 0.72 } else { 1.0 }),
                        rgb_hsla(bar_rgb, bar_alpha),
                    )
                }
                DesktopSkin::Sonata => (
                    rgb_hsla(base_bg, w_alpha),
                    rgb_hsla(base_surface, side_alpha),
                    rgb_hsla(base_surface, panel_alpha),
                    rgb_hsla(base_hl, if transparent { 0.34 } else { 0.62 }),
                    rgb_hsla(base_hl, if transparent { 0.56 } else { 0.85 }),
                    rgb_hsla(line_rgb, if is_light { 0.06 } else { 0.07 }),
                    rgb_hsla(mix_rgb(base_surface, ambient_rgb, 0.10), bar_alpha),
                ),
            };

        let (play_btn_bg, play_btn_fg) = match skin {
            DesktopSkin::Spotify => {
                if is_light {
                    (
                        rgb_hsla((0x12, 0x12, 0x12), 1.0),
                        rgb_hsla((255, 255, 255), 1.0),
                    )
                } else {
                    (rgb_hsla((255, 255, 255), 1.0), rgb_hsla((0, 0, 0), 1.0))
                }
            }
            DesktopSkin::YouTubeMusic => (
                rgb_hsla(base_fg, 1.0),
                rgb_hsla(readable_foreground(base_fg), 1.0),
            ),
            DesktopSkin::Sonata => (
                rgb_hsla(base_accent, 1.0),
                rgb_hsla(readable_foreground(base_accent), 1.0),
            ),
        };

        let (cta_bg, cta_fg) = match skin {
            DesktopSkin::YouTubeMusic => (
                rgb_hsla(base_fg, 1.0),
                rgb_hsla(readable_foreground(base_fg), 1.0),
            ),
            _ => (
                rgb_hsla(base_accent, 1.0),
                rgb_hsla(readable_foreground(base_accent), 1.0),
            ),
        };

        let divider = match skin {
            DesktopSkin::Spotify => rgb_hsla(line_rgb, if is_light { 0.08 } else { 0.06 }),
            DesktopSkin::YouTubeMusic => rgb_hsla(line_rgb, 0.12),
            DesktopSkin::Sonata => rgb_hsla(line_rgb, if is_light { 0.09 } else { 0.08 }),
        };

        Self {
            skin,
            is_light,
            shell_bg,
            window_bg: rgb_hsla(base_bg, w_alpha),
            ambient_top: rgb_hsla(tinted_ambient, amb_alpha),
            ambient_glow: rgb_hsla(ambient_rgb, if transparent { 0.22 } else { 0.28 }),
            sidebar_bg,
            panel_bg,
            card_bg,
            card_hover,
            search_bg,
            player_bar_bg,
            modal_bg: rgb_hsla(modal_rgb, 0.96),
            pill_bg: rgb_hsla(line_rgb, if is_light { 0.07 } else { 0.09 }),
            divider,
            border: core_hsla(
                base.border,
                if transparent { 0.42 } else { 0.65 },
                (0x36, 0x38, 0x46),
            ),
            border_subtle: rgb_hsla(line_rgb, if is_light || transparent { 0.10 } else { 0.08 }),
            text: core_hsla(base.fg, 1.0, (0xf2, 0xf4, 0xf8)),
            muted: core_hsla(base.muted, 1.0, (0x8e, 0x92, 0xa4)),
            accent: rgb_hsla(base_accent, 1.0),
            accent_soft: rgb_hsla(base_accent, if is_light { 0.14 } else { 0.20 }),
            accent_fg: rgb_hsla(readable_foreground(base_accent), 1.0),
            play_btn_bg,
            play_btn_fg,
            cta_bg,
            cta_fg,
            highlight_bg: rgb_hsla(base_hl, if transparent { 0.68 } else { 0.92 }),
            highlight_fg: core_hsla(base.highlight_fg, 1.0, (0xff, 0xff, 0xff)),
            lyrics_active: core_hsla(base.lyrics_active, 1.0, (255, 255, 255)),
            lyrics_mid: rgb_hsla(base_fg, 0.62),
            lyrics_inactive: core_hsla(base.lyrics_inactive, 0.44, (0x7c, 0x80, 0x92)),
            success: core_hsla(base.success, 1.0, (0x30, 0xd1, 0x58)),
            warning: core_hsla(base.warning, 1.0, (0xff, 0xd6, 0x0a)),
            error: core_hsla(base.error, 1.0, (0xff, 0x45, 0x3a)),
        }
    }

    pub fn with_glass_opacity(mut self, transparent: bool, hyprland_opacity: f32) -> Self {
        if !transparent {
            return self;
        }
        let scale = (hyprland_opacity * 0.95).clamp(0.35, 1.0);
        self.shell_bg.a *= scale;
        self.window_bg.a *= scale;
        self.sidebar_bg.a *= scale;
        self.panel_bg.a *= scale;
        self.player_bar_bg.a *= scale;
        self.card_bg.a *= scale;
        self.ambient_top.a *= scale;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_palette_adapts_alpha_and_ambient_tint() {
        let theme = Theme::sonata_glass();
        let glass = GlassPalette::from_theme(&theme, true, (240, 80, 140));
        let opaque = GlassPalette::from_theme(&theme, false, (240, 80, 140));
        assert!(glass.window_bg.a < opaque.window_bg.a);
        assert!(glass.ambient_top.a > 0.18);
        assert_eq!(glass.skin, DesktopSkin::Sonata);
    }

    #[test]
    fn detects_spotify_and_youtube_music_skins() {
        let sp = GlassPalette::from_theme(&Theme::spotify_desktop(), false, (30, 215, 96));
        assert_eq!(sp.skin, DesktopSkin::Spotify);
        assert!(!sp.is_light);

        let ytm = GlassPalette::from_theme(&Theme::youtube_music_desktop(), false, (255, 0, 51));
        assert_eq!(ytm.skin, DesktopSkin::YouTubeMusic);
        assert!(!ytm.is_light);

        let sp_light = GlassPalette::from_theme(&Theme::spotify_light(), false, (30, 215, 96));
        assert_eq!(sp_light.skin, DesktopSkin::Spotify);
        assert!(sp_light.is_light);
    }

    #[test]
    fn glass_opacity_scales_translucent_surfaces_only_when_transparent() {
        let theme = Theme::sonata_glass();
        let high =
            GlassPalette::from_theme(&theme, true, (180, 90, 220)).with_glass_opacity(true, 1.0);
        let low =
            GlassPalette::from_theme(&theme, true, (180, 90, 220)).with_glass_opacity(true, 0.50);
        assert!(low.window_bg.a < high.window_bg.a);
        assert!(low.sidebar_bg.a < high.sidebar_bg.a);
        assert!(low.panel_bg.a < high.panel_bg.a);
        assert!(low.player_bar_bg.a < high.player_bar_bg.a);
        assert!(low.card_bg.a < high.card_bg.a);
        assert!(low.ambient_top.a < high.ambient_top.a);

        let opaque = GlassPalette::from_theme(&theme, false, (180, 90, 220));
        let opaque_unchanged = opaque.clone().with_glass_opacity(false, 0.50);
        assert!((opaque.window_bg.a - opaque_unchanged.window_bg.a).abs() < f32::EPSILON);
    }
}
