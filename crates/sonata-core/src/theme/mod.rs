//! Theme model, color parsing, TOML loading, and TUI/Desktop built-in registries.
//!
//! 1. **Color Types (`RgbColor`, `AnsiColor`)**: color representation and hex/named color parsing.
//! 2. **Built-in Separation**: `tui` holds terminal themes while `imperial_violet_dark`, `imperial_violet`, `moon_pearl_dark`, `moon_pearl`, `spotify`, and `youtube_music` hold desktop themes.
//! 3. **Loaders**: `load_for_tui`, `load_for_desktop`, and `from_toml_str` resolve built-in and custom TOML themes.

mod imperial_violet;
mod imperial_violet_dark;
mod moon_pearl;
mod moon_pearl_dark;
mod spotify;
mod tui;
mod youtube_music;

use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbColor {
    Rgb(u8, u8, u8),
    Reset,
    Ansi(AnsiColor),
}

impl RgbColor {
    pub const fn to_rgb(self, fallback: (u8, u8, u8)) -> (u8, u8, u8) {
        match self {
            Self::Rgb(r, g, b) => (r, g, b),
            Self::Reset => fallback,
            Self::Ansi(ansi) => match ansi {
                AnsiColor::Black => (0x18, 0x18, 0x20),
                AnsiColor::Red => (0xf3, 0x8b, 0xa8),
                AnsiColor::Green => (0xa6, 0xe3, 0xa1),
                AnsiColor::Yellow => (0xf9, 0xe2, 0xaf),
                AnsiColor::Blue => (0x89, 0xb4, 0xfa),
                AnsiColor::Magenta => (0xcb, 0xa6, 0xf7),
                AnsiColor::Cyan => (0x94, 0xe2, 0xd5),
                AnsiColor::Gray => (0xa6, 0xad, 0xc8),
                AnsiColor::DarkGray => (0x58, 0x5b, 0x70),
                AnsiColor::White => (0xeb, 0xef, 0xf5),
            },
        }
    }

    pub fn to_rgba_u32(self, alpha: f32, fallback: (u8, u8, u8)) -> u32 {
        let (r, g, b) = self.to_rgb(fallback);
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
        ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub transparent_bg: Option<bool>,
    pub hyprland_opacity: Option<f32>,
    pub bg: RgbColor,
    pub surface: RgbColor,
    pub fg: RgbColor,
    pub muted: RgbColor,
    pub accent: RgbColor,
    pub border: RgbColor,
    pub highlight_bg: RgbColor,
    pub highlight_fg: RgbColor,
    pub lyrics_active: RgbColor,
    pub lyrics_inactive: RgbColor,
    pub success: RgbColor,
    pub warning: RgbColor,
    pub error: RgbColor,
}

#[derive(Debug, Deserialize, Default)]
struct ThemeFile {
    transparent_bg: Option<bool>,
    hyprland_opacity: Option<f32>,
    opacity: Option<f32>,
    colors: Option<RawColors>,
}

#[derive(Debug, Deserialize, Default)]
struct RawColors {
    transparent_bg: Option<bool>,
    hyprland_opacity: Option<f32>,
    opacity: Option<f32>,
    bg: Option<String>,
    surface: Option<String>,
    fg: Option<String>,
    muted: Option<String>,
    accent: Option<String>,
    border: Option<String>,
    highlight_bg: Option<String>,
    highlight_fg: Option<String>,
    lyrics_active: Option<String>,
    lyrics_inactive: Option<String>,
    success: Option<String>,
    warning: Option<String>,
    error: Option<String>,
}

/// Parses a `#RRGGBB`, `#RGB`, or basic terminal color name into an [`RgbColor`].
pub fn parse_color(input: &str) -> Option<RgbColor> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(RgbColor::Rgb(r, g, b));
        }
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(RgbColor::Rgb(r, g, b));
        }
        return None;
    }

    match s.to_ascii_lowercase().as_str() {
        "reset" | "default" | "transparent" => Some(RgbColor::Reset),
        "black" => Some(RgbColor::Ansi(AnsiColor::Black)),
        "red" => Some(RgbColor::Ansi(AnsiColor::Red)),
        "green" => Some(RgbColor::Ansi(AnsiColor::Green)),
        "yellow" => Some(RgbColor::Ansi(AnsiColor::Yellow)),
        "blue" => Some(RgbColor::Ansi(AnsiColor::Blue)),
        "magenta" => Some(RgbColor::Ansi(AnsiColor::Magenta)),
        "cyan" => Some(RgbColor::Ansi(AnsiColor::Cyan)),
        "gray" | "grey" => Some(RgbColor::Ansi(AnsiColor::Gray)),
        "darkgray" | "darkgrey" => Some(RgbColor::Ansi(AnsiColor::DarkGray)),
        "white" => Some(RgbColor::Ansi(AnsiColor::White)),
        _ => None,
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl Theme {
    /// Built-in terminal themes tailored for `sonata` (TUI).
    pub fn tui_builtins() -> Vec<Self> {
        vec![
            Self::catppuccin_mocha(),
            Self::ghostty_dark(),
            Self::tokyo_night(),
            Self::gruvbox_dark(),
            Self::rose_pine(),
            Self::nord(),
        ]
    }

    /// Built-in desktop themes tailored for `sonata-desktop`.
    pub fn desktop_builtins() -> Vec<Self> {
        vec![
            Self::imperial_violet_dark(),
            Self::imperial_violet(),
            Self::moon_pearl_dark(),
            Self::moon_pearl(),
            Self::spotify_desktop(),
            Self::youtube_music_desktop(),
        ]
    }

    pub fn builtins() -> Vec<Self> {
        let mut all = Self::tui_builtins();
        all.extend(Self::desktop_builtins());
        all
    }

    pub fn with_transparency(&self, transparent: bool) -> Self {
        let mut out = self.clone();
        if transparent {
            out.bg = RgbColor::Reset;
            out.surface = RgbColor::Reset;
        }
        out
    }

    pub fn from_toml_str(name: String, content: &str) -> Option<Self> {
        let parsed: ThemeFile = toml::from_str(content).ok()?;
        let raw = parsed.colors.unwrap_or_default();
        let base = Self::catppuccin_mocha();
        let pick = |opt: Option<String>, fallback: RgbColor| -> RgbColor {
            opt.and_then(|s| parse_color(&s)).unwrap_or(fallback)
        };
        let transparent_bg = raw.transparent_bg.or(parsed.transparent_bg);
        let hyprland_opacity = raw
            .hyprland_opacity
            .or(raw.opacity)
            .or(parsed.hyprland_opacity)
            .or(parsed.opacity)
            .map(|o| o.clamp(0.50, 1.00));
        Some(Self {
            name,
            transparent_bg,
            hyprland_opacity,
            bg: pick(raw.bg, base.bg),
            surface: pick(raw.surface, base.surface),
            fg: pick(raw.fg, base.fg),
            muted: pick(raw.muted, base.muted),
            accent: pick(raw.accent, base.accent),
            border: pick(raw.border, base.border),
            highlight_bg: pick(raw.highlight_bg, base.highlight_bg),
            highlight_fg: pick(raw.highlight_fg, base.highlight_fg),
            lyrics_active: pick(raw.lyrics_active, base.lyrics_active),
            lyrics_inactive: pick(raw.lyrics_inactive, base.lyrics_inactive),
            success: pick(raw.success, base.success),
            warning: pick(raw.warning, base.warning),
            error: pick(raw.error, base.error),
        })
    }

    fn append_custom_themes(themes: &mut Vec<Self>, themes_dir: &Path) {
        if let Ok(entries) = fs::read_dir(themes_dir) {
            let mut custom_paths: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
                .collect();
            custom_paths.sort();
            for path in custom_paths {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("custom");
                if stem == "example-custom" {
                    continue;
                }
                if let Ok(raw) = fs::read_to_string(&path) {
                    if let Some(theme) = Self::from_toml_str(format!("{stem} (Custom)"), &raw) {
                        themes.push(theme);
                    }
                }
            }
        }
    }

    pub fn load_for_tui(themes_dir: &Path) -> Vec<Self> {
        let mut themes = Self::tui_builtins();
        Self::append_custom_themes(&mut themes, themes_dir);
        themes
    }

    pub fn load_for_desktop(_themes_dir: &Path) -> Vec<Self> {
        Self::desktop_builtins()
    }

    pub fn load_all(themes_dir: &Path) -> Vec<Self> {
        let mut themes = Self::builtins();
        Self::append_custom_themes(&mut themes, themes_dir);
        themes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_named_colors() {
        assert_eq!(
            parse_color("#cba6f7"),
            Some(RgbColor::Rgb(0xcb, 0xa6, 0xf7))
        );
        assert_eq!(parse_color("#abc"), Some(RgbColor::Rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(parse_color("cyan"), Some(RgbColor::Ansi(AnsiColor::Cyan)));
        assert_eq!(parse_color("invalid"), None);
    }

    #[test]
    fn converts_rgb_color_to_rgb_and_rgba_u32() {
        assert_eq!(
            RgbColor::Rgb(0xcb, 0xa6, 0xf7).to_rgb((0, 0, 0)),
            (0xcb, 0xa6, 0xf7)
        );
        assert_eq!(
            RgbColor::Reset.to_rgb((0x1e, 0x1e, 0x2e)),
            (0x1e, 0x1e, 0x2e)
        );
        assert_eq!(
            RgbColor::Rgb(0x10, 0x20, 0x30).to_rgba_u32(1.0, (0, 0, 0)),
            0x102030ff
        );
        assert_eq!(
            RgbColor::Rgb(0xff, 0x00, 0x80).to_rgba_u32(0.0, (0, 0, 0)),
            0xff008000
        );
    }

    #[test]
    fn loads_color_only_toml_theme() {
        let toml = r##"
            [colors]
            bg = "#000000"
            accent = "#ff5500"
            lyrics_active = "#00ffaa"
        "##;
        let theme = Theme::from_toml_str("Neon".into(), toml).expect("valid custom theme");
        assert_eq!(theme.bg, RgbColor::Rgb(0, 0, 0));
        assert_eq!(theme.accent, RgbColor::Rgb(0xff, 0x55, 0x00));
        assert_eq!(theme.lyrics_active, RgbColor::Rgb(0x00, 0xff, 0xaa));
        assert_eq!(theme.fg, Theme::catppuccin_mocha().fg);
    }

    #[test]
    fn parses_transparency_and_hyprland_opacity_in_theme() {
        let toml = r##"
            transparent_bg = true
            hyprland_opacity = 0.85

            [colors]
            bg = "transparent"
        "##;
        let theme = Theme::from_toml_str("Glass".into(), toml).expect("valid glass theme");
        assert_eq!(theme.transparent_bg, Some(true));
        assert_eq!(theme.hyprland_opacity, Some(0.85));
        assert_eq!(theme.bg, RgbColor::Reset);
        let applied = theme.with_transparency(true);
        assert_eq!(applied.surface, RgbColor::Reset);
    }

    #[test]
    fn separates_tui_and_desktop_builtin_themes() {
        let tui_names: Vec<_> = Theme::tui_builtins()
            .into_iter()
            .map(|theme| theme.name)
            .collect();
        assert!(tui_names.contains(&"Catppuccin Mocha".to_string()));
        assert!(!tui_names.contains(&"Spotify".to_string()));
        assert!(!tui_names.contains(&"YouTube Music".to_string()));

        let desktop_names: Vec<_> = Theme::desktop_builtins()
            .into_iter()
            .map(|theme| theme.name)
            .collect();
        assert_eq!(
            desktop_names,
            vec![
                "Imperial Violet Dark".to_string(),
                "Imperial Violet".to_string(),
                "Moon Pearl Dark".to_string(),
                "Moon Pearl".to_string(),
                "Spotify".to_string(),
                "YouTube Music".to_string(),
            ]
        );
        for name in &desktop_names {
            assert!(!tui_names.contains(name));
        }
    }
}
