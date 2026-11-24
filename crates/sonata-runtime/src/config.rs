//! Application configuration and cross-distro path helpers for Sonata.
//!
//! 1. **Configuration Model**: `AppConfig` stores playback, provider, cache, theme, and glass settings in `config.toml`.
//! 2. **Defaults & Quotas**: Exposes cache limit helpers and default XDG path resolution for Linux clients.
//! 3. **Distro Binary Resolution**: `resolve_distro_binary` locates runtime dependencies (`mpv`, `ffmpeg`, `yt-dlp`, JS runtimes) across NixOS, Arch, Fedora, and Ubuntu paths.

use crate::downloads::default_downloads_dir;
use crate::i18n::AppLanguage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sonata_core::auth::ensure_private_dir;
pub use sonata_core::cache::{
    DEFAULT_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_GB, GIGABYTE, MAX_CACHE_LIMIT_GB,
};
use sonata_core::paths::CorePaths;
use sonata_core::player::RepeatMode;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_DOWNLOADS_LIMIT_GB: u8 = 10;

fn default_true() -> bool {
    true
}

fn default_downloads_limit_gb() -> u8 {
    DEFAULT_DOWNLOADS_LIMIT_GB
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
    /// Optional custom directory for shared offline downloads (`None` uses XDG data dir).
    #[serde(default)]
    pub downloads_dir: Option<PathBuf>,
    /// Maximum offline downloads budget in GiB (`0` disables automatic limit pruning).
    #[serde(default = "default_downloads_limit_gb")]
    pub downloads_limit_gb: u8,
    /// Whether Discord Rich Presence IPC updates are enabled on Linux clients.
    #[serde(default = "default_true")]
    pub discord_rpc_enabled: bool,
    /// Optional custom Discord application client ID overriding the built-in Sonata app ID.
    #[serde(default)]
    pub discord_app_id: Option<String>,
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
            downloads_dir: None,
            downloads_limit_gb: DEFAULT_DOWNLOADS_LIMIT_GB,
            discord_rpc_enabled: true,
            discord_app_id: None,
        }
    }
}

impl AppConfig {
    /// Returns the configured disk cache budget in bytes for this config instance.
    pub fn cache_limit_bytes(&self) -> u64 {
        u64::from(self.cache_limit_gb.min(MAX_CACHE_LIMIT_GB)) * GIGABYTE
    }

    /// Returns the configured offline downloads byte budget, or `None` when unlimited (`0`).
    pub fn downloads_limit_bytes(&self) -> Option<u64> {
        (self.downloads_limit_gb > 0).then(|| u64::from(self.downloads_limit_gb) * GIGABYTE)
    }

    /// Returns a human-readable label for a downloads quota in GiB.
    pub fn downloads_limit_label(gigabytes: u8) -> String {
        match gigabytes {
            0 => "Unlimited".to_string(),
            value => format!("{value} GB"),
        }
    }

    /// Returns the default shared XDG offline downloads directory (`$XDG_DATA_HOME/sonata/downloads`).
    pub fn default_downloads_dir() -> PathBuf {
        default_downloads_dir()
    }

    /// Returns the active offline downloads directory (`downloads_dir` override or XDG default).
    pub fn effective_downloads_dir(&self) -> PathBuf {
        self.downloads_dir
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .unwrap_or_else(Self::default_downloads_dir)
    }

    /// Reads the configured offline downloads directory from `config.toml` if present, otherwise
    /// returns [`Self::default_downloads_dir`] without creating or writing configuration files.
    pub fn configured_downloads_dir() -> PathBuf {
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<Self>(&raw) {
                return cfg.effective_downloads_dir();
            }
        }
        Self::default_downloads_dir()
    }

    /// Returns the configured Discord application ID or `default_app_id` when unset/blank.
    pub fn effective_discord_app_id<'a>(&'a self, default_app_id: &'a str) -> &'a str {
        self.discord_app_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(default_app_id)
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
        Self::load_from(&Self::config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(mut cfg) = toml::from_str::<Self>(&raw) {
                cfg.cache_limit_gb = cfg.cache_limit_gb.min(MAX_CACHE_LIMIT_GB);
                return cfg;
            }
        }
        let cfg = Self::default();
        let _ = cfg.save_to(path);
        cfg
    }

    pub fn save(&self) -> Result<()> {
        Self::ensure_dirs()?;
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent)?;
        }
        let mut persisted = self.clone();
        persisted.cache_limit_gb = persisted.cache_limit_gb.min(MAX_CACHE_LIMIT_GB);
        let body = toml::to_string_pretty(&persisted)?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, body)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, DEFAULT_DOWNLOADS_LIMIT_GB, GIGABYTE};
    use crate::downloads::test_support::TempTestDir;
    use std::path::PathBuf;

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

    #[test]
    fn downloads_and_discord_defaults_and_load_save_preserve_values() {
        let default_cfg = AppConfig::default();
        assert!(default_cfg.downloads_dir.is_none());
        assert_eq!(default_cfg.downloads_limit_gb, DEFAULT_DOWNLOADS_LIMIT_GB);
        assert_eq!(
            default_cfg.downloads_limit_bytes(),
            Some(u64::from(DEFAULT_DOWNLOADS_LIMIT_GB) * GIGABYTE)
        );
        assert!(default_cfg.discord_rpc_enabled);
        assert!(default_cfg.discord_app_id.is_none());
        assert_eq!(
            default_cfg.effective_discord_app_id("1346240000000000000"),
            "1346240000000000000"
        );

        let legacy: AppConfig = toml::from_str("").expect("legacy empty config should parse");
        assert!(legacy.downloads_dir.is_none());
        assert_eq!(legacy.downloads_limit_gb, DEFAULT_DOWNLOADS_LIMIT_GB);
        assert!(legacy.discord_rpc_enabled);
        assert!(legacy.discord_app_id.is_none());

        let dir = TempTestDir::new("cfg_test");
        let cfg_path = dir.path().join("config.toml");
        let custom_downloads = dir.path().join("offline-library");

        let custom = AppConfig {
            downloads_dir: Some(custom_downloads.clone()),
            downloads_limit_gb: 25,
            discord_rpc_enabled: false,
            discord_app_id: Some("987654321098765432".to_string()),
            ..AppConfig::default()
        };
        custom.save_to(&cfg_path).expect("save config");

        let reloaded = AppConfig::load_from(&cfg_path);
        assert_eq!(reloaded.downloads_dir, Some(custom_downloads.clone()));
        assert_eq!(reloaded.effective_downloads_dir(), custom_downloads);
        assert_eq!(reloaded.downloads_limit_gb, 25);
        assert_eq!(reloaded.downloads_limit_bytes(), Some(25 * GIGABYTE));
        assert!(!reloaded.discord_rpc_enabled);
        assert_eq!(
            reloaded.discord_app_id.as_deref(),
            Some("987654321098765432")
        );
        assert_eq!(
            reloaded.effective_discord_app_id("1346240000000000000"),
            "987654321098765432"
        );

        let unlimited: AppConfig = toml::from_str(
            "downloads_limit_gb = 0\ndownloads_dir = \"\"\ndiscord_app_id = \"  \"\n",
        )
        .expect("zero limit and blank overrides should parse");
        assert_eq!(unlimited.downloads_limit_bytes(), None);
        assert_eq!(AppConfig::downloads_limit_label(0), "Unlimited");
        assert_eq!(unlimited.downloads_dir, Some(PathBuf::from("")));
        assert_eq!(
            unlimited.effective_downloads_dir(),
            AppConfig::default_downloads_dir()
        );
        assert_eq!(
            unlimited.effective_discord_app_id("1346240000000000000"),
            "1346240000000000000"
        );
    }
}
