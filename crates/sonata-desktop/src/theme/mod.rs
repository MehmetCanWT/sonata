//! Desktop CSS theme registry, solved translucent palette, and GPUI Component theme bridge.
//!
//! 1. **Catalog & Reload (`ThemeRegistryState`)**: loads CSS themes from `~/.config/sonata/themes/`, merges desktop GTK/Qt options, tracks parse errors/contrast warnings, and handles `--scheme`.
//! 2. **Solved Palette (`DesktopPalette`)**: applies layered `card_alpha` / `glass_alpha` / `sheer` math and text contrast compensation over the active theme tokens.
//! 3. **GPUI Sync (`sync_gpui_theme`)**: maps `DesktopPalette` into `gpui_kit::component::Theme` so native controls (`Slider`, `Switch`, `Select`, `TabBar`, `Skeleton`, `Popover`, `Button`, `Dialog`) match identically.

pub mod custom;
pub mod ramp;
pub mod system;

use crate::settings::{solve_alpha, CornerRadii, DesktopSettings, SolvedAlpha, ThemeModePref};
use custom::{builtin_fallback, load_themes_dir, normalize_scheme_key};
use gpui_kit::component::{Theme as GpuiTheme, ThemeMode as GpuiThemeMode};
use gpui_kit::{px, App, Hsla};
use ramp::{hsla_from_rgb, Rgb, ThemeTokens};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use system::{
    detect_system_is_dark_with_qt, has_gtk_palette, load_gtk_palettes, load_qt_palette, THEME_GTK,
    THEME_QT,
};

#[derive(Debug, Clone)]
pub struct DesktopPalette {
    pub theme_name: String,
    pub is_dark: bool,
    pub solved: SolvedAlpha,
    pub radii: CornerRadii,
    pub window_radius: f32,

    pub base_rgb: Rgb,

    pub gutter_bg: Hsla,
    pub card_bg: Hsla,
    pub card_edge: Hsla,
    pub popup_bg: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub outline: Hsla,

    pub text: Hsla,
    pub secondary: Hsla,
    pub dim: Hsla,

    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
}

impl DesktopPalette {
    pub fn from_tokens(tokens: &ThemeTokens, settings: &DesktopSettings) -> Self {
        let solved = solve_alpha(settings.opacity, settings.blur);
        let transparent = settings.is_transparent();
        let radii = settings.corners.radii();
        let window_radius = if settings.compositor_corners {
            radii.panel
        } else {
            0.0
        };

        let glass = |fill: Rgb| -> Hsla {
            if solved.glass_alpha < 0.999 {
                let tinted = fill.lerp(tokens.text, 0.12);
                hsla_from_rgb(tinted, solved.glass_alpha)
            } else {
                hsla_from_rgb(fill, 1.0)
            }
        };

        // Boost secondary & dim readability as background translucency grows.
        let sec_rgb = tokens.subtext.lerp(tokens.text, 0.25 * solved.sheer);
        let dim_rgb = tokens.muted.lerp(tokens.text, 0.45 * solved.sheer);
        let card_edge = if settings.card_outline {
            hsla_from_rgb(tokens.text, 0.09)
        } else {
            hsla_from_rgb(tokens.text, 0.0)
        };

        Self {
            theme_name: tokens.name.clone(),
            is_dark: tokens.is_dark,
            solved,
            radii,
            window_radius,

            base_rgb: tokens.base,

            gutter_bg: hsla_from_rgb(tokens.gutter, solved.gutter_alpha),
            card_bg: hsla_from_rgb(tokens.base, solved.card_alpha),
            card_edge,
            popup_bg: hsla_from_rgb(tokens.popup, 0.98),
            surface: glass(tokens.surface),
            surface_hover: glass(tokens.surface_hover),
            surface_active: glass(tokens.surface_active),
            outline: hsla_from_rgb(tokens.outline, if transparent { 0.35 } else { 0.85 }),

            text: hsla_from_rgb(tokens.text, 1.0),
            secondary: hsla_from_rgb(sec_rgb, 1.0),
            dim: hsla_from_rgb(dim_rgb, 1.0),

            accent: hsla_from_rgb(tokens.accent, 1.0),
            accent_hover: hsla_from_rgb(tokens.accent_hover, 1.0),
            on_accent: hsla_from_rgb(tokens.on_accent, 1.0),
            danger: hsla_from_rgb(tokens.danger, 1.0),
            warning: hsla_from_rgb(tokens.warning, 1.0),
        }
    }

    /// Computes a gentle translucent cover-art wash mixed with the theme base.
    pub fn washed_surface(&self, rgb: (u8, u8, u8), strength: f32) -> Hsla {
        let cover = Rgb::from_u8(rgb.0, rgb.1, rgb.2);
        let blended = self.base_rgb.lerp(cover, strength.clamp(0.0, 0.75));
        hsla_from_rgb(blended, self.solved.card_alpha.max(0.32))
    }
}

#[derive(Debug, Clone)]
pub struct ThemeCatalogSnapshot {
    pub themes: Vec<ThemeTokens>,
    pub errors: Vec<String>,
    pub warnings_by_name: HashMap<String, Vec<String>>,
    pub system_is_dark: bool,
    pub gtk_dark: ThemeTokens,
    pub gtk_light: ThemeTokens,
    pub qt_palette: Option<ThemeTokens>,
}

#[derive(Debug, Clone)]
pub struct ThemeCatalog {
    pub themes_dir: PathBuf,
    pub themes: Vec<ThemeTokens>,
    pub errors: Vec<String>,
    pub warnings_by_name: HashMap<String, Vec<String>>,
    pub fallback_notice: Option<String>,
    pub scheme_override: Option<String>,
    pub system_is_dark: bool,
    pub gtk_dark: ThemeTokens,
    pub gtk_light: ThemeTokens,
    pub qt_palette: Option<ThemeTokens>,
}

impl ThemeCatalog {
    pub fn load_snapshot(dir: &Path) -> ThemeCatalogSnapshot {
        let loaded = load_themes_dir(dir);
        let qt_palette = load_qt_palette();
        let system_is_dark = detect_system_is_dark_with_qt(qt_palette.as_ref());
        let (gtk_dark, gtk_light) = load_gtk_palettes();
        ThemeCatalogSnapshot {
            themes: loaded.themes,
            errors: loaded.errors,
            warnings_by_name: loaded.warnings_by_name,
            system_is_dark,
            gtk_dark,
            gtk_light,
            qt_palette,
        }
    }

    pub fn load(scheme_override: Option<String>) -> Self {
        let dir = DesktopSettings::themes_dir();
        let snap = Self::load_snapshot(&dir);
        Self {
            themes_dir: dir,
            themes: snap.themes,
            errors: snap.errors,
            warnings_by_name: snap.warnings_by_name,
            fallback_notice: None,
            scheme_override,
            system_is_dark: snap.system_is_dark,
            gtk_dark: snap.gtk_dark,
            gtk_light: snap.gtk_light,
            qt_palette: snap.qt_palette,
        }
    }

    pub fn apply_snapshot(&mut self, snap: ThemeCatalogSnapshot) -> bool {
        let changed = self.themes != snap.themes
            || self.errors != snap.errors
            || self.warnings_by_name != snap.warnings_by_name
            || self.system_is_dark != snap.system_is_dark
            || self.gtk_dark != snap.gtk_dark
            || self.gtk_light != snap.gtk_light
            || self.qt_palette != snap.qt_palette;
        if changed {
            self.themes = snap.themes;
            self.errors = snap.errors;
            self.warnings_by_name = snap.warnings_by_name;
            self.system_is_dark = snap.system_is_dark;
            self.gtk_dark = snap.gtk_dark;
            self.gtk_light = snap.gtk_light;
            self.qt_palette = snap.qt_palette;
        }
        changed
    }

    pub fn resolved_is_dark(&self, settings: &DesktopSettings) -> bool {
        if let Some(ref ov) = self.scheme_override {
            let key = normalize_scheme_key(ov);
            if let Some(found) = self
                .themes
                .iter()
                .find(|t| normalize_scheme_key(&t.name) == key)
            {
                return found.is_dark;
            }
        }
        match settings.theme_mode {
            ThemeModePref::Dark => true,
            ThemeModePref::Light => false,
            ThemeModePref::System => self.system_is_dark,
        }
    }

    /// Returns dropdown theme names matching the active mode (starting with GTK / Qt when available on Linux).
    pub fn available_names_for_mode(&self, settings: &DesktopSettings) -> Vec<String> {
        let target_dark = self.resolved_is_dark(settings);
        let mut names = Vec::new();
        if has_gtk_palette() {
            names.push(THEME_GTK.to_string());
        }
        if self.qt_palette.is_some() {
            names.push(THEME_QT.to_string());
        }
        for t in &self.themes {
            if t.is_dark == target_dark {
                names.push(t.name.clone());
            }
        }
        names
    }

    pub fn selected_theme_name(&self, settings: &DesktopSettings) -> String {
        if let Some(ref ov) = self.scheme_override {
            let key = normalize_scheme_key(ov);
            if let Some(found) = self
                .themes
                .iter()
                .find(|t| normalize_scheme_key(&t.name) == key)
            {
                return found.name.clone();
            }
        }
        if self.resolved_is_dark(settings) {
            settings.dark_theme.clone()
        } else {
            settings.light_theme.clone()
        }
    }

    pub fn resolve_palette(&mut self, settings: &DesktopSettings) -> DesktopPalette {
        let is_dark = self.resolved_is_dark(settings);
        let desired = self.selected_theme_name(settings);
        self.fallback_notice = None;

        if desired.eq_ignore_ascii_case(THEME_GTK) && has_gtk_palette() {
            let gtk = if is_dark {
                &self.gtk_dark
            } else {
                &self.gtk_light
            };
            return DesktopPalette::from_tokens(gtk, settings);
        }
        if desired.eq_ignore_ascii_case(THEME_QT) {
            if let Some(ref qt) = self.qt_palette {
                return DesktopPalette::from_tokens(qt, settings);
            }
        }

        let desired_key = normalize_scheme_key(&desired);
        if let Some(exact) = self
            .themes
            .iter()
            .find(|t| normalize_scheme_key(&t.name) == desired_key)
        {
            return DesktopPalette::from_tokens(exact, settings);
        }

        // Fall back to another loaded theme in the active mode while preserving user's setting.
        if let Some(mode_match) = self.themes.iter().find(|t| t.is_dark == is_dark) {
            self.fallback_notice = Some(format!(
                "\"{desired}\" unavailable; using \"{}\"",
                mode_match.name
            ));
            return DesktopPalette::from_tokens(mode_match, settings);
        }

        self.fallback_notice = Some(format!(
            "No theme file matched \"{desired}\"; using built-in Classic fallback"
        ));
        let fb = builtin_fallback(is_dark);
        DesktopPalette::from_tokens(&fb, settings)
    }

    pub fn warnings_for_active(&self, active_name: &str) -> Vec<String> {
        let mut out = self
            .warnings_by_name
            .get(active_name)
            .cloned()
            .unwrap_or_default();
        if let Some(ref note) = self.fallback_notice {
            out.push(note.clone());
        }
        out
    }
}

/// Pushes the active `DesktopPalette` into `gpui-component`'s global `Theme` so built-in components match.
pub fn sync_gpui_theme(cx: &mut App, palette: &DesktopPalette) {
    let target_mode = if palette.is_dark {
        GpuiThemeMode::Dark
    } else {
        GpuiThemeMode::Light
    };
    if GpuiTheme::global(cx).mode != target_mode {
        GpuiTheme::change(target_mode, None, cx);
    }

    GpuiTheme::update(cx, |theme| {
        theme.font_family = "Inter".into();
        theme.radius = px(palette.radii.control);
        theme.radius_lg = px(palette.radii.popup);
        theme.window_radius = px(palette.window_radius);
        theme.transparent = gpui_kit::transparent_black();

        let c = &mut theme.colors;
        c.background = palette.card_bg;
        c.foreground = palette.text;
        c.primary = palette.accent;
        c.primary_hover = palette.accent_hover;
        c.primary_active = palette.accent_hover;
        c.primary_foreground = palette.on_accent;
        c.button_primary = palette.accent;
        c.button_primary_hover = palette.accent_hover;
        c.button_primary_active = palette.accent_hover;
        c.button_primary_foreground = palette.on_accent;

        c.secondary = palette.surface;
        c.secondary_hover = palette.surface_hover;
        c.secondary_active = palette.surface_active;
        c.secondary_foreground = palette.text;
        c.button_secondary = palette.surface;
        c.button_secondary_hover = palette.surface_hover;
        c.button_secondary_active = palette.surface_active;
        c.button_secondary_foreground = palette.text;
        c.button = palette.surface;
        c.button_hover = palette.surface_hover;
        c.button_active = palette.surface_active;
        c.button_foreground = palette.text;

        c.muted = palette.surface;
        c.muted_foreground = palette.secondary;
        c.accent = palette.surface_hover;
        c.accent_foreground = palette.text;
        c.border = palette.card_edge;
        c.input = palette.surface;
        c.ring = palette.accent;
        c.caret = palette.accent;
        c.selection = palette.accent.opacity(0.28);

        c.popover = palette.popup_bg;
        c.popover_foreground = palette.text;
        c.list = gpui_kit::transparent_black();
        c.list_hover = palette.surface_hover;
        c.list_active = palette.surface_active;
        c.list_active_border = palette.accent;
        c.list_even = palette.surface.opacity(0.35);

        c.slider_bar = palette.accent;
        c.slider_thumb = palette.text;
        c.switch = palette.surface_active;
        c.switch_thumb = palette.text;
        c.skeleton = palette.surface;
        c.tab = gpui_kit::transparent_black();
        c.tab_active = palette.surface_active;
        c.tab_active_foreground = palette.text;
        c.tab_bar = palette.surface.opacity(0.65);
        c.tab_bar_segmented = palette.surface.opacity(0.65);
        c.tab_foreground = palette.secondary;

        c.sidebar = palette.card_bg;
        c.sidebar_foreground = palette.text;
        c.sidebar_accent = palette.surface_hover;
        c.sidebar_accent_foreground = palette.text;
        c.sidebar_border = palette.card_edge;
        c.sidebar_primary = palette.accent;
        c.sidebar_primary_foreground = palette.on_accent;

        c.title_bar = gpui_kit::transparent_black();
        c.title_bar_border = gpui_kit::transparent_black();
        c.window_border = palette.card_edge;

        c.danger = palette.danger;
        c.danger_foreground = palette.on_accent;
        c.warning = palette.warning;
        c.warning_foreground = palette.on_accent;
        c.link = palette.accent;
        c.link_hover = palette.accent_hover;
        c.link_active = palette.accent_hover;
        c.scrollbar = gpui_kit::transparent_black();
        c.scrollbar_thumb = palette.text.opacity(0.18);
        c.scrollbar_thumb_hover = palette.text.opacity(0.34);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramp::Rgb;

    fn sample_tokens(name: &str, is_dark: bool, accent: Rgb) -> ThemeTokens {
        let mut vars = HashMap::new();
        vars.insert(
            "--base".to_string(),
            if is_dark {
                Rgb::from_u8(20, 20, 24)
            } else {
                Rgb::from_u8(250, 250, 252)
            },
        );
        vars.insert(
            "--text".to_string(),
            if is_dark {
                Rgb::from_u8(245, 245, 247)
            } else {
                Rgb::from_u8(30, 30, 35)
            },
        );
        vars.insert("--accent".to_string(), accent);
        ramp::derive_theme_tokens(name.to_string(), Some(is_dark), &vars).unwrap()
    }

    #[test]
    fn resolve_palette_uses_cached_gtk_tokens_for_opacity_adjustments() {
        let custom_accent = Rgb::from_u8(12, 199, 111);
        let gtk_dark = sample_tokens(THEME_GTK, true, custom_accent);
        let gtk_light = sample_tokens(THEME_GTK, false, Rgb::from_u8(53, 132, 228));
        let mut catalog = ThemeCatalog {
            themes_dir: PathBuf::from("/nonexistent"),
            themes: Vec::new(),
            errors: Vec::new(),
            warnings_by_name: HashMap::new(),
            fallback_notice: None,
            scheme_override: None,
            system_is_dark: true,
            gtk_dark: gtk_dark.clone(),
            gtk_light: gtk_light.clone(),
            qt_palette: None,
        };
        let mut settings = DesktopSettings {
            theme_mode: ThemeModePref::Dark,
            dark_theme: THEME_GTK.to_string(),
            opacity: 0.72,
            ..DesktopSettings::default()
        };
        let p1 = catalog.resolve_palette(&settings);
        settings.opacity = 0.90;
        let p2 = catalog.resolve_palette(&settings);
        assert_eq!(p1.theme_name, THEME_GTK);
        assert_eq!(p1.accent, p2.accent);
        assert!(p2.solved.card_alpha < p1.solved.card_alpha);

        let same = ThemeCatalogSnapshot {
            themes: Vec::new(),
            errors: Vec::new(),
            warnings_by_name: HashMap::new(),
            system_is_dark: true,
            gtk_dark,
            gtk_light,
            qt_palette: None,
        };
        assert!(!catalog.apply_snapshot(same));
    }
}
