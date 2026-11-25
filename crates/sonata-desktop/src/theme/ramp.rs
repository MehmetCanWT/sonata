//! Color math, token derivation ramp, and WCAG AA contrast checking for CSS themes.
//!
//! 1. **Color Primitives**: `Rgb` struct with hex/`rgb()`/`rgba()` parsing, sRGB/linear conversion, interpolation, and lightness adjustment.
//! 2. **Contrast Ratio**: WCAG 2.1 relative luminance and contrast ratio computation (`4.5:1` minimum check and `4.8:1` minimum lift for derived labels).
//! 3. **Theme Derivation**: derives all 15 palette tokens from the 3 required variables (`--base`, `--text`, `--accent`) and fills any omitted optional CSS tokens.

use gpui_kit::{hsla, Hsla, Rgba};
use std::collections::HashMap;

pub const BODY_CONTRAST_WARN_THRESHOLD: f32 = 4.5;
pub const DERIVED_CONTRAST_FLOOR: f32 = 4.8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let f = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * f,
            g: self.g + (other.g - self.g) * f,
            b: self.b + (other.b - self.b) * f,
        }
    }

    pub fn to_rgba(self, alpha: f32) -> Rgba {
        Rgba {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
            a: alpha.clamp(0.0, 1.0),
        }
    }

    pub fn to_hsla(self, alpha: f32) -> Hsla {
        Hsla::from(self.to_rgba(alpha))
    }

    pub fn luminance(self) -> f32 {
        fn lin(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    pub fn contrast_ratio(self, other: Self) -> f32 {
        let l1 = self.luminance();
        let l2 = other.luminance();
        let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }

    pub fn is_dark(self) -> bool {
        self.luminance() < 0.179
    }

    /// Parses `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, or `rgba(r, g, b, a)`.
    pub fn parse_css(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if let Some(hex) = s.strip_prefix('#') {
            return match hex.len() {
                3 => {
                    let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                    let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                    let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                    Some(Self::from_u8(r, g, b))
                }
                6 | 8 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some(Self::from_u8(r, g, b))
                }
                _ => None,
            };
        }
        let lower = s.to_ascii_lowercase();
        let inner = lower
            .strip_prefix("rgba(")
            .or_else(|| lower.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = inner
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let parse_chan = |token: &str| -> Option<f32> {
            if let Some(pct) = token.strip_suffix('%') {
                Some((pct.parse::<f32>().ok()? / 100.0).clamp(0.0, 1.0))
            } else {
                Some((token.parse::<f32>().ok()? / 255.0).clamp(0.0, 1.0))
            }
        };
        Some(Self::new(
            parse_chan(parts[0])?,
            parse_chan(parts[1])?,
            parse_chan(parts[2])?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeTokens {
    pub name: String,
    pub is_dark: bool,
    pub base: Rgb,
    pub gutter: Rgb,
    pub popup: Rgb,
    pub surface: Rgb,
    pub surface_hover: Rgb,
    pub surface_active: Rgb,
    pub outline: Rgb,
    pub text: Rgb,
    pub subtext: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub accent_hover: Rgb,
    pub on_accent: Rgb,
    pub danger: Rgb,
    pub warning: Rgb,
    pub low_contrast_warning: Option<String>,
}

/// Ensures `fg` reaches `min_contrast` against `bg` by nudging toward `target` (usually `text`).
pub fn lift_contrast(fg: Rgb, bg: Rgb, target: Rgb, min_contrast: f32) -> Rgb {
    if fg.contrast_ratio(bg) >= min_contrast {
        return fg;
    }
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for _ in 0..16 {
        let mid = (lo + hi) * 0.5;
        if fg.lerp(target, mid).contrast_ratio(bg) >= min_contrast {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    fg.lerp(target, hi)
}

pub fn derive_theme_tokens(
    name: String,
    color_scheme_dark: Option<bool>,
    vars: &HashMap<String, Rgb>,
) -> Result<ThemeTokens, String> {
    let base = *vars
        .get("--base")
        .ok_or_else(|| "missing required --base variable".to_string())?;
    let text = *vars
        .get("--text")
        .ok_or_else(|| "missing required --text variable".to_string())?;
    let accent = *vars
        .get("--accent")
        .ok_or_else(|| "missing required --accent variable".to_string())?;

    let is_dark = color_scheme_dark.unwrap_or_else(|| base.is_dark());
    let black = Rgb::new(0.0, 0.0, 0.0);
    let white = Rgb::new(1.0, 1.0, 1.0);

    let gutter = vars.get("--gutter").copied().unwrap_or_else(|| {
        if is_dark {
            base.lerp(black, 0.35)
        } else {
            base.lerp(black, 0.06)
        }
    });

    let popup = vars
        .get("--popup")
        .copied()
        .unwrap_or_else(|| gutter.lerp(base, 0.55));

    let surface = vars
        .get("--surface")
        .copied()
        .unwrap_or_else(|| base.lerp(text, if is_dark { 0.08 } else { 0.06 }));

    let surface_hover = vars
        .get("--surface-hover")
        .copied()
        .unwrap_or_else(|| surface.lerp(text, 0.07));

    let surface_active = vars
        .get("--surface-active")
        .copied()
        .unwrap_or_else(|| surface.lerp(text, 0.14));

    let outline = vars
        .get("--outline")
        .copied()
        .unwrap_or_else(|| surface.lerp(text, 0.06));

    let subtext_raw = vars
        .get("--subtext")
        .copied()
        .unwrap_or_else(|| text.lerp(base, 0.32));
    let subtext = if vars.contains_key("--subtext") {
        subtext_raw
    } else {
        lift_contrast(subtext_raw, base, text, DERIVED_CONTRAST_FLOOR)
    };

    let muted_raw = vars
        .get("--muted")
        .copied()
        .unwrap_or_else(|| text.lerp(base, 0.54));
    let muted = if vars.contains_key("--muted") {
        muted_raw
    } else {
        lift_contrast(muted_raw, base, text, 3.3)
    };

    let accent_hover = vars.get("--accent-hover").copied().unwrap_or_else(|| {
        if is_dark {
            accent.lerp(white, 0.16)
        } else {
            accent.lerp(black, 0.16)
        }
    });

    let on_accent = vars.get("--on-accent").copied().unwrap_or_else(|| {
        if accent.contrast_ratio(white) >= accent.contrast_ratio(black) {
            white
        } else {
            black
        }
    });

    let danger = vars.get("--danger").copied().unwrap_or_else(|| {
        if is_dark {
            Rgb::from_u8(245, 113, 127)
        } else {
            Rgb::from_u8(214, 59, 76)
        }
    });

    let warning = vars.get("--warning").copied().unwrap_or_else(|| {
        if is_dark {
            Rgb::from_u8(242, 184, 92)
        } else {
            Rgb::from_u8(184, 122, 20)
        }
    });

    let body_contrast = text.contrast_ratio(base);
    let low_contrast_warning = if body_contrast < BODY_CONTRAST_WARN_THRESHOLD {
        Some(format!(
            "Low contrast: --text on --base is {body_contrast:.1}:1 (recommended >= 4.5:1)"
        ))
    } else {
        None
    };

    Ok(ThemeTokens {
        name,
        is_dark,
        base,
        gutter,
        popup,
        surface,
        surface_hover,
        surface_active,
        outline,
        text,
        subtext,
        muted,
        accent,
        accent_hover,
        on_accent,
        danger,
        warning,
        low_contrast_warning,
    })
}

pub fn hsla_from_rgb(rgb: Rgb, alpha: f32) -> Hsla {
    hsla(
        rgb.to_hsla(alpha).h,
        rgb.to_hsla(alpha).s,
        rgb.to_hsla(alpha).l,
        alpha.clamp(0.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_css_colors_supports_hex_and_rgb() {
        assert_eq!(Rgb::parse_css("#fff"), Some(Rgb::from_u8(255, 255, 255)));
        assert_eq!(Rgb::parse_css("#141619"), Some(Rgb::from_u8(20, 22, 25)));
        assert_eq!(
            Rgb::parse_css("rgb(255, 51, 61)"),
            Some(Rgb::from_u8(255, 51, 61))
        );
        assert!(Rgb::parse_css("tomato").is_none());
    }

    #[test]
    fn derive_tokens_lifts_subtext_contrast_when_omitted() {
        let mut vars = HashMap::new();
        vars.insert("--base".to_string(), Rgb::from_u8(20, 22, 25));
        vars.insert("--text".to_string(), Rgb::from_u8(243, 244, 246));
        vars.insert("--accent".to_string(), Rgb::from_u8(255, 51, 61));
        let tokens = derive_theme_tokens("Test".to_string(), Some(true), &vars).unwrap();
        assert!(tokens.subtext.contrast_ratio(tokens.base) >= DERIVED_CONTRAST_FLOOR - 0.05);
        assert!(tokens.low_contrast_warning.is_none());
    }

    #[test]
    fn low_contrast_body_text_sets_warning() {
        let mut vars = HashMap::new();
        vars.insert("--base".to_string(), Rgb::from_u8(50, 50, 50));
        vars.insert("--text".to_string(), Rgb::from_u8(85, 85, 85));
        vars.insert("--accent".to_string(), Rgb::from_u8(255, 51, 61));
        let tokens = derive_theme_tokens("Dim".to_string(), Some(true), &vars).unwrap();
        assert!(tokens.low_contrast_warning.is_some());
    }
}
