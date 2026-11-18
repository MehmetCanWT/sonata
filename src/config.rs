use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Audio DSP enhancer presets inspired by BitChord's audio processor pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnhancerMode {
    #[default]
    HiResCrystal,
    SpatialWide,
    WarmBass,
    BitPerfect,
}

impl EnhancerMode {
    pub fn next(self) -> Self {
        match self {
            Self::HiResCrystal => Self::SpatialWide,
            Self::SpatialWide => Self::WarmBass,
            Self::WarmBass => Self::BitPerfect,
            Self::BitPerfect => Self::HiResCrystal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::HiResCrystal => "Hi-Res Crystal",
            Self::SpatialWide => "Spatial Wide",
            Self::WarmBass => "Warm Bass",
            Self::BitPerfect => "Bit-Perfect",
        }
    }

    /// Returns the mpv `--af` audio filter chain for this preset.
    pub fn mpv_af(self) -> &'static str {
        match self {
            Self::HiResCrystal => {
                "lavfi=[aresample=96000:resampler=soxr:precision=28,crystalizer=i=1.4,extrastereo=m=1.18,alimiter=limit=0.95:level=false]"
            }
            Self::SpatialWide => {
                "lavfi=[extrastereo=m=2.0,bs2b=fcut=700:feed=65,alimiter=limit=0.95:level=false]"
            }
            Self::WarmBass => {
                "lavfi=[bass=g=4.5:f=95:w=0.6,crystalizer=i=1.2,alimiter=limit=0.95:level=false]"
            }
            Self::BitPerfect => "",
        }
    }
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
    pub volume: u8,
    pub provider_mode: ProviderMode,
    pub enhancer_mode: EnhancerMode,
    pub hifi_substitution: bool,
    pub transparent_bg: bool,
    pub hyprland_opacity: f32,
    pub suppress_image_warning: bool,
    pub preferred_browser: Option<String>,
    pub spotify_playlists: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "Catppuccin Mocha".to_string(),
            volume: 85,
            provider_mode: ProviderMode::Both,
            enhancer_mode: EnhancerMode::HiResCrystal,
            hifi_substitution: true,
            transparent_bg: true,
            hyprland_opacity: 0.90,
            suppress_image_warning: false,
            preferred_browser: None,
            spotify_playlists: Vec::new(),
        }
    }
}

impl AppConfig {
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
            if let Ok(cfg) = toml::from_str::<Self>(&raw) {
                return cfg;
            }
        }
        let cfg = Self::default();
        let _ = cfg.save();
        cfg
    }

    pub fn save(&self) -> Result<()> {
        Self::ensure_dirs()?;
        let body = toml::to_string_pretty(self)?;
        fs::write(Self::config_path(), body)?;
        Ok(())
    }
}
