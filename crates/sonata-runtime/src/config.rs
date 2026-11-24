//! Application configuration and cross-distro path helpers for Sonata.
//!
//! 1. **Configuration Model**: `AppConfig` stores playback, provider, cache, theme, and glass settings in `config.toml`.
//! 2. **Defaults & Quotas**: Exposes cache limit helpers and default XDG path resolution for Linux clients.
//! 3. **Distro Binary Resolution**: `resolve_distro_binary` locates runtime dependencies (`mpv`, `ffmpeg`, `yt-dlp`, JS runtimes) across NixOS, Arch, Fedora, and Ubuntu paths.

use crate::i18n::AppLanguage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
pub use sonata_core::cache::{
    DEFAULT_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_GB, GIGABYTE, MAX_CACHE_LIMIT_GB,
};
use sonata_core::paths::CorePaths;
use sonata_core::player::RepeatMode;
use std::fs;
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

/// Resolves an external CLI dependency across standard Linux, NixOS, Flatpak, and Snap binary paths.
pub fn resolve_distro_binary(bin_name: &str) -> String {
    if bin_name.is_empty() || bin_name.contains('/') {
        return bin_name.to_string();
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    let mut fallback_dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        fallback_dirs.push(home.join(".local/bin"));
        fallback_dirs.push(home.join(".nix-profile/bin"));
        fallback_dirs.push(home.join(".local/state/nix/profiles/profile/bin"));
        fallback_dirs.push(home.join(".local/share/flatpak/exports/bin"));
    }
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() && !user.contains('/') {
            fallback_dirs.push(PathBuf::from(format!("/etc/profiles/per-user/{user}/bin")));
        }
    }
    fallback_dirs.push(PathBuf::from("/run/current-system/sw/bin"));
    fallback_dirs.push(PathBuf::from("/nix/var/nix/profiles/default/bin"));
    fallback_dirs.push(PathBuf::from("/usr/local/bin"));
    fallback_dirs.push(PathBuf::from("/usr/bin"));
    fallback_dirs.push(PathBuf::from("/bin"));
    fallback_dirs.push(PathBuf::from("/var/lib/flatpak/exports/bin"));
    fallback_dirs.push(PathBuf::from("/snap/bin"));

    for dir in fallback_dirs {
        let candidate = dir.join(bin_name);
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }

    bin_name.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMode {
    #[default]
    Both,
    YouTubeMusic,
    Spotify,
}

impl ProviderMode {
    pub fn next(self) -> Self {
        match self {
            Self::Both => Self::YouTubeMusic,
            Self::YouTubeMusic => Self::Spotify,
            Self::Spotify => Self::Both,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Both => Self::Spotify,
            Self::YouTubeMusic => Self::Both,
            Self::Spotify => Self::YouTubeMusic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Both (YTM + Spotify)",
            Self::YouTubeMusic => "YouTube Music Only",
            Self::Spotify => "Spotify Only",
        }
    }

    pub fn uses_ytm(self) -> bool {
        matches!(self, Self::Both | Self::YouTubeMusic)
    }

    pub fn uses_spotify(self) -> bool {
        matches!(self, Self::Both | Self::Spotify)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub theme: String,
    pub desktop_theme: String,
    #[serde(default)]
    pub language: AppLanguage,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
    #[serde(default = "default_true")]
    pub autoplay: bool,
    pub provider_mode: ProviderMode,
    pub hifi_substitution: bool,
    #[serde(default)]
    pub jiosaavn_enabled: bool,
    /// Maximum retained media cache size in GiB. Zero disables persistent caching.
    pub cache_limit_gb: u8,
    pub transparent_bg: bool,
    #[serde(default)]
    pub liquid_glass: bool,
    #[serde(default = "default_true")]
    pub motion_covers: bool,
    pub reduce_motion: bool,
    pub hyprland_opacity: f32,
    pub left_pane_pct: u16,
    pub right_pane_pct: u16,
    pub art_height_offset: i16,
    pub suppress_image_warning: bool,
    pub preferred_browser: Option<String>,
    pub desktop_ytm_browser: Option<String>,
    pub spotify_playlists: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "Catppuccin Mocha".to_string(),
            desktop_theme: "Imperial Violet Dark".to_string(),
            language: AppLanguage::En,
            volume: 85,
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            autoplay: true,
            provider_mode: ProviderMode::Both,
            hifi_substitution: true,
            jiosaavn_enabled: false,
            cache_limit_gb: DEFAULT_CACHE_LIMIT_GB,
            transparent_bg: true,
            liquid_glass: false,
            motion_covers: true,
            reduce_motion: false,
            hyprland_opacity: 0.90,
            left_pane_pct: 32,
            right_pane_pct: 22,
            art_height_offset: 0,
            suppress_image_warning: false,
            preferred_browser: None,
            desktop_ytm_browser: None,
            spotify_playlists: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Returns the configured disk cache budget in bytes for this config instance.
    pub fn cache_limit_bytes(&self) -> u64 {
        u64::from(self.cache_limit_gb.min(MAX_CACHE_LIMIT_GB)) * GIGABYTE
    }

    /// Returns the default XDG-derived [`CorePaths`] for Linux desktop/TUI clients.
    pub fn default_core_paths() -> CorePaths {
        CorePaths::new(Self::config_dir(), Self::cache_dir())
    }

    pub fn cache_limit_label(gigabytes: u8) -> String {
        match gigabytes.min(MAX_CACHE_LIMIT_GB) {
            0 => "Off (not recommended)".to_string(),
            value => format!("{value} GB"),
        }
    }

    /// Step within Off and the supported 1 GB to 10 GB cache limits.
    pub fn step_cache_limit_gb(gigabytes: u8, forward: bool) -> u8 {
        let current = gigabytes.min(MAX_CACHE_LIMIT_GB);
        if forward {
            current.saturating_add(1).min(MAX_CACHE_LIMIT_GB)
        } else {
            current.saturating_sub(1)
        }
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sonata")
    }

    pub fn themes_dir() -> PathBuf {
        Self::config_dir().join("themes")
    }

    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("sonata")
    }

    pub fn lyrics_cache_dir() -> PathBuf {
        Self::cache_dir().join("lyrics")
    }

    pub fn art_cache_dir() -> PathBuf {
        Self::cache_dir().join("art")
    }

    pub fn audio_cache_dir() -> PathBuf {
        Self::cache_dir().join("audio")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn ensure_dirs() -> Result<()> {
        fs::create_dir_all(Self::config_dir())?;
        fs::create_dir_all(Self::themes_dir())?;
        fs::create_dir_all(Self::lyrics_cache_dir())?;
        fs::create_dir_all(Self::art_cache_dir())?;

        let example_theme = Self::themes_dir().join("example-custom.toml");
        if !example_theme.exists() {
            let template = r##"# Color-only custom theme for Sonata.
# Copy and rename this file (e.g. my-theme.toml) to add your own theme.
# Only hex (#RRGGBB), "reset"/"transparent", or terminal color names are used.
transparent_bg = true
hyprland_opacity = 0.88

[colors]
bg = "#11111b"
surface = "#181825"
fg = "#cdd6f4"
muted = "#6c7086"
accent = "#cba6f7"
border = "#45475a"
highlight_bg = "#313244"
highlight_fg = "#f5e0dc"
lyrics_active = "#f5c2e7"
lyrics_inactive = "#6c7086"
success = "#a6e3a1"
warning = "#f9e2af"
error = "#f38ba8"
"##;
            let _ = fs::write(example_theme, template);
        }
        Ok(())
    }

    pub fn load() -> Self {
        let _ = Self::ensure_dirs();
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = toml::from_str::<Self>(&raw) {
                cfg.cache_limit_gb = cfg.cache_limit_gb.min(MAX_CACHE_LIMIT_GB);
                return cfg;
            }
        }
        let cfg = Self::default();
        let _ = cfg.save();
        cfg
    }

    pub fn save(&self) -> Result<()> {
        Self::ensure_dirs()?;
        let mut persisted = self.clone();
        persisted.cache_limit_gb = persisted.cache_limit_gb.min(MAX_CACHE_LIMIT_GB);
        let body = toml::to_string_pretty(&persisted)?;
        let path = Self::config_path();
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, body)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn liquid_glass_defaults_false_and_imperial_violet_dark_is_default_theme() {
        let default_cfg = AppConfig::default();
        assert!(!default_cfg.liquid_glass);
        assert_eq!(default_cfg.desktop_theme, "Imperial Violet Dark");

        let legacy: AppConfig = toml::from_str("").expect("empty legacy config should parse");
        assert!(!legacy.liquid_glass);
        assert_eq!(legacy.desktop_theme, "Imperial Violet Dark");

        let enabled: AppConfig =
            toml::from_str("liquid_glass = true\n").expect("enabled liquid_glass should parse");
        assert!(enabled.liquid_glass);
        assert!(default_cfg.autoplay);
        assert!(legacy.autoplay);
        let autoplay_off: AppConfig =
            toml::from_str("autoplay = false\nshuffle = true\nrepeat_mode = \"all\"\n")
                .expect("autoplay/shuffle/repeat should parse");
        assert!(!autoplay_off.autoplay);
        assert!(autoplay_off.shuffle);
        assert_eq!(
            autoplay_off.repeat_mode,
            sonata_core::player::RepeatMode::All
        );
    }
}
