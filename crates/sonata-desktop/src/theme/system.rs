//! Desktop environment integration for dark/light mode detection, KDE Qt (`kdeglobals`), and GTK palettes.
//!
//! 1. **System Appearance**: detects user dark/light preference from `xdg-desktop-portal` / `gsettings` / `kdeglobals`.
//! 2. **Qt Palette**: parses `[Colors:*]` sections from `~/.config/kdeglobals` into `ThemeTokens` when KDE is configured.
//! 3. **GTK Palette**: reads GTK 3 settings (`gtk-application-prefer-dark-theme`, `gtk-theme-name`) or GNOME `gsettings` into `ThemeTokens`.

use super::ramp::{derive_theme_tokens, Rgb, ThemeTokens};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const THEME_GTK: &str = "GTK";
pub const THEME_QT: &str = "Qt";

fn kdeglobals_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("kdeglobals"))
}

pub fn has_gtk_palette() -> bool {
    cfg!(target_os = "linux")
}

/// Detects whether the desktop prefers dark appearance using an already-loaded Qt palette to avoid re-reading `kdeglobals`.
pub fn detect_system_is_dark_with_qt(qt_palette: Option<&ThemeTokens>) -> bool {
    if cfg!(target_os = "linux") {
        // 1. Check gsettings color-scheme (`prefer-dark` / `prefer-light`).
        if let Ok(out) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            if text.contains("prefer-dark") {
                return true;
            }
            if text.contains("prefer-light") {
                return false;
            }
        }
        // 2. Check GTK3 prefer-dark setting.
        if let Some(cfg) = dirs::config_dir() {
            let gtk3 = cfg.join("gtk-3.0/settings.ini");
            if let Ok(raw) = fs::read_to_string(gtk3) {
                for line in raw.lines() {
                    let lower = line.to_ascii_lowercase();
                    if lower.contains("gtk-application-prefer-dark-theme") {
                        return lower.contains("=1") || lower.contains("true");
                    }
                    if lower.contains("gtk-theme-name") && lower.contains("dark") {
                        return true;
                    }
                }
            }
        }
        // 3. Check KDE kdeglobals BackgroundNormal luminance.
        if let Some(qt) = qt_palette {
            return qt.is_dark;
        }
    }
    true
}

/// Reads KDE `kdeglobals` `[Colors:View]`, `[Colors:Window]`, and `[Colors:Selection]` into `ThemeTokens`.
pub fn load_qt_palette() -> Option<ThemeTokens> {
    let path = kdeglobals_path()?;
    let raw = fs::read_to_string(path).ok()?;

    let mut section = String::new();
    let mut view_bg = None;
    let mut view_fg = None;
    let mut win_bg = None;
    let mut sel_bg = None;
    let mut sel_fg = None;
    let mut inactive_fg = None;
    let mut neg_fg = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len() - 1].to_string();
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let Some(rgb) = parse_kde_rgb(v.trim()) else {
            continue;
        };
        match (section.as_str(), key) {
            ("Colors:View", "BackgroundNormal") => view_bg = Some(rgb),
            ("Colors:View", "ForegroundNormal") => view_fg = Some(rgb),
            ("Colors:View", "ForegroundInactive") => inactive_fg = Some(rgb),
            ("Colors:View", "ForegroundNegative") => neg_fg = Some(rgb),
            ("Colors:Window", "BackgroundNormal") => win_bg = Some(rgb),
            ("Colors:Selection", "BackgroundNormal") => sel_bg = Some(rgb),
            ("Colors:Selection", "ForegroundNormal") => sel_fg = Some(rgb),
            _ => {}
        }
    }

    let base = view_bg?;
    let text = view_fg?;
    let accent = sel_bg?;
    let mut vars = HashMap::new();
    vars.insert("--base".to_string(), base);
    vars.insert("--text".to_string(), text);
    vars.insert("--accent".to_string(), accent);
    if let Some(g) = win_bg {
        vars.insert("--gutter".to_string(), g);
        vars.insert("--surface".to_string(), g);
    }
    if let Some(fg) = sel_fg {
        vars.insert("--on-accent".to_string(), fg);
    }
    if let Some(sub) = inactive_fg {
        vars.insert("--subtext".to_string(), sub);
    }
    if let Some(neg) = neg_fg {
        vars.insert("--danger".to_string(), neg);
    }
    derive_theme_tokens(THEME_QT.to_string(), Some(base.is_dark()), &vars).ok()
}

struct GtkPaletteInputs {
    prefer_dark: Option<bool>,
    custom_bg: Option<Rgb>,
    custom_fg: Option<Rgb>,
    custom_accent: Option<Rgb>,
    custom_win: Option<Rgb>,
}

fn read_gtk_palette_inputs() -> GtkPaletteInputs {
    let mut prefer_dark: Option<bool> = None;
    let mut theme_name = String::new();

    if let Some(cfg) = dirs::config_dir() {
        let gtk3 = cfg.join("gtk-3.0/settings.ini");
        if let Ok(raw) = fs::read_to_string(gtk3) {
            for line in raw.lines() {
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("gtk-application-prefer-dark-theme") {
                    prefer_dark = Some(lower.contains("=1") || lower.contains("true"));
                } else if let Some((_, v)) = line.split_once('=') {
                    if lower.starts_with("gtk-theme-name") {
                        theme_name = v.trim().to_string();
                        if theme_name.to_ascii_lowercase().contains("dark") {
                            prefer_dark = Some(true);
                        }
                    }
                }
            }
        }
    }
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        let out_str = String::from_utf8_lossy(&out.stdout);
        let cleaned = out_str.trim().trim_matches('\'');
        if !cleaned.is_empty() {
            theme_name = cleaned.to_string();
            if cleaned.to_ascii_lowercase().contains("dark") {
                prefer_dark = Some(true);
            }
        }
    }

    // Extract GTK CSS custom theme overrides if a user GTK3 gtk.css defines `@define-color` accent/bg/fg.
    let mut custom_bg = None;
    let mut custom_fg = None;
    let mut custom_accent = None;
    let mut custom_win = None;
    if let Some(cfg) = dirs::config_dir() {
        let css_path = cfg.join("gtk-3.0/gtk.css");
        if let Ok(raw) = fs::read_to_string(css_path) {
            for line in raw.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("@define-color ") {
                    let parts: Vec<&str> = rest.trim_end_matches(';').split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Some(rgb) = Rgb::parse_css(parts[1]) {
                            match parts[0] {
                                "theme_base_color" | "view_bg_color" => custom_bg = Some(rgb),
                                "theme_fg_color" | "view_fg_color" => custom_fg = Some(rgb),
                                "theme_selected_bg_color" | "accent_bg_color" | "accent_color" => {
                                    custom_accent = Some(rgb)
                                }
                                "theme_bg_color" | "window_bg_color" => custom_win = Some(rgb),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = theme_name;
    GtkPaletteInputs {
        prefer_dark,
        custom_bg,
        custom_fg,
        custom_accent,
        custom_win,
    }
}

fn derive_gtk_tokens(fallback_dark: bool, inputs: &GtkPaletteInputs) -> ThemeTokens {
    let is_dark = inputs.prefer_dark.unwrap_or(fallback_dark);
    let mut vars = HashMap::new();
    if is_dark {
        vars.insert(
            "--base".to_string(),
            inputs.custom_bg.unwrap_or_else(|| Rgb::from_u8(30, 30, 34)),
        );
        vars.insert(
            "--text".to_string(),
            inputs
                .custom_fg
                .unwrap_or_else(|| Rgb::from_u8(245, 245, 247)),
        );
        vars.insert(
            "--accent".to_string(),
            inputs
                .custom_accent
                .unwrap_or_else(|| Rgb::from_u8(53, 132, 228)),
        );
        vars.insert(
            "--gutter".to_string(),
            inputs
                .custom_win
                .unwrap_or_else(|| Rgb::from_u8(20, 20, 24)),
        );
    } else {
        vars.insert(
            "--base".to_string(),
            inputs
                .custom_bg
                .unwrap_or_else(|| Rgb::from_u8(255, 255, 255)),
        );
        vars.insert(
            "--text".to_string(),
            inputs.custom_fg.unwrap_or_else(|| Rgb::from_u8(36, 31, 49)),
        );
        vars.insert(
            "--accent".to_string(),
            inputs
                .custom_accent
                .unwrap_or_else(|| Rgb::from_u8(53, 132, 228)),
        );
        vars.insert(
            "--gutter".to_string(),
            inputs
                .custom_win
                .unwrap_or_else(|| Rgb::from_u8(246, 245, 244)),
        );
    }

    derive_theme_tokens(THEME_GTK.to_string(), Some(is_dark), &vars)
        .expect("gtk fallback tokens valid")
}

/// Resolves both dark and light GTK 3 palettes with a single `gsettings` and disk read pass.
pub fn load_gtk_palettes() -> (ThemeTokens, ThemeTokens) {
    let inputs = read_gtk_palette_inputs();
    (
        derive_gtk_tokens(true, &inputs),
        derive_gtk_tokens(false, &inputs),
    )
}

fn parse_kde_rgb(raw: &str) -> Option<Rgb> {
    let parts: Vec<&str> = raw.split(',').collect();
    if parts.len() < 3 {
        return None;
    }
    let r: u8 = parts[0].trim().parse().ok()?;
    let g: u8 = parts[1].trim().parse().ok()?;
    let b: u8 = parts[2].trim().parse().ok()?;
    Some(Rgb::from_u8(r, g, b))
}
