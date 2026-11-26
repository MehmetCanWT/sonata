//! Persistent cross-platform desktop UI preferences stored in `desktop-ui.json`.
//!
//! 1. **Appearance**: `ThemeModePref`, active dark/light theme labels, and `adaptive_art_color`.
//! 2. **Layout & Behavior**: Queue/lyrics panel visibility, double-click behavior, and pinned collections.
//! 3. **Persistence**: Atomic read/write under the OS config directory (`desktop-ui.json`).

use serde::{Deserialize, Serialize};
use sonata_runtime::i18n::AppLanguage;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeModePref {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CornerStyle {
    #[default]
    Round,
    Soft,
    Sharp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedCollection {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub is_album: bool,
}

pub fn normalize_collection_id(id: &str) -> String {
    let trimmed = id.trim();
    trimmed.strip_prefix("VL").unwrap_or(trimmed).to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    #[serde(default)]
    pub language: AppLanguage,
    pub theme_mode: ThemeModePref,
    pub dark_theme: String,
    pub light_theme: String,
    pub opacity: f32,
    pub blur: bool,
    pub card_outline: bool,
    pub corners: CornerStyle,
    pub compositor_corners: bool,
    pub scroll_fades: bool,
    pub adaptive_art_color: bool,
    pub sidebar_width: f32,
    pub panel_width: f32,
    pub show_queue_panel: bool,
    pub show_lyrics_panel: bool,
    pub double_click_to_play: bool,
    pub sidebar_playlist_order: Vec<String>,
    #[serde(default)]
    pub pinned_collections: Vec<PinnedCollection>,
    #[serde(default)]
    pub saved_external_playlists: Vec<PinnedCollection>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            language: AppLanguage::En,
            theme_mode: ThemeModePref::Dark,
            dark_theme: default_dark_theme_name().to_string(),
            light_theme: default_light_theme_name().to_string(),
            opacity: 0.0,
            blur: false,
            card_outline: true,
            corners: CornerStyle::Round,
            compositor_corners: false,
            scroll_fades: true,
            adaptive_art_color: true,
            sidebar_width: 216.0,
            panel_width: 312.0,
            show_queue_panel: true,
            show_lyrics_panel: false,
            double_click_to_play: false,
            sidebar_playlist_order: Vec::new(),
            pinned_collections: Vec::new(),
            saved_external_playlists: Vec::new(),
        }
    }
}

pub fn default_dark_theme_name() -> &'static str {
    "Obsidian Dark"
}

pub fn default_light_theme_name() -> &'static str {
    "Matte Light"
}

impl DesktopSettings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sonata")
            .join("desktop-ui.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        let Ok(json) = serde_json::to_string_pretty(self) else {
            return;
        };
        let path = Self::config_path();
        let _ = std::thread::Builder::new()
            .name("sonata-settings-save".into())
            .spawn(move || {
                use std::sync::{Mutex, OnceLock};
                static SAVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
                let _guard = SAVE_LOCK.get_or_init(|| Mutex::new(())).lock();
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let tmp = path.with_extension("json.tmp");
                if fs::write(&tmp, json).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_matte_dark_without_glass_blur() {
        let settings = DesktopSettings::default();
        assert_eq!(settings.theme_mode, ThemeModePref::Dark);
        assert!(!settings.blur);
        assert_eq!(settings.opacity, 0.0);
        assert!(settings.show_queue_panel);
    }

    #[test]
    fn normalize_collection_id_strips_vl_prefix() {
        assert_eq!(normalize_collection_id("VLPL123"), "PL123");
        assert_eq!(normalize_collection_id("  PL456  "), "PL456");
    }
}
