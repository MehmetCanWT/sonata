//! Shared music provider models, HTTP client builder, and submodule exports.
//!
//! 1. **Provider Modules**: `hifi` (stream resolution), `jiosaavn` (optional 320kbps AAC module), `spotify`, and `ytmusic`.
//! 2. **Core Models**: `Track`, `Playlist`, and `ProviderSource` represent cross-provider items uniformly.

pub mod hifi;
pub mod jiosaavn;
pub mod profile;
pub mod spotify;
pub mod ytmusic;

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderSource {
    YouTubeMusic,
    Spotify,
}

impl ProviderSource {
    pub fn short_tag(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "YTM",
            Self::Spotify => "SPF",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub source: ProviderSource,
    /// Optional resolved audio quality label (for example, "OPUS 137kbps · HQ").
    pub audio_badge: Option<String>,
    /// Playlist item identifier required by YouTube Music to remove a song from a playlist.
    #[serde(default)]
    pub set_video_id: Option<String>,
    #[serde(default)]
    pub liked: bool,
    /// YouTube Music album browse ID (e.g. `MPREb_...`) when supplied by the source item.
    #[serde(default)]
    pub album_id: Option<String>,
    /// Whether YouTube Music marks this track with `MUSIC_EXPLICIT_BADGE`.
    #[serde(default)]
    pub is_explicit: bool,
}

impl Track {
    /// Returns true when this track originates from the optional JioSaavn module.
    pub fn is_jiosaavn(&self) -> bool {
        self.id.starts_with(jiosaavn::ID_PREFIX)
    }

    /// Returns the short 3-letter source badge displayed in track lists (`YTM`, `SPF`, `JIO`).
    pub fn source_tag(&self) -> &'static str {
        if self.is_jiosaavn() {
            "JIO"
        } else {
            self.source.short_tag()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub source: ProviderSource,
    pub track_count: Option<usize>,
    pub thumbnail_url: Option<String>,
    /// True when this playlist is owned by the signed-in user rather than a saved community/auto playlist.
    #[serde(default)]
    pub is_owned: bool,
}

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(6))
        .pool_max_idle_per_host(12)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub fn format_duration(secs: Option<u64>) -> String {
    match secs {
        Some(s) => format!("{}:{:02}", s / 60, s % 60),
        None => "--:--".to_string(),
    }
}

pub fn parse_duration_text(text: &str) -> Option<u64> {
    let parts: Vec<u64> = text
        .trim()
        .split(':')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect();
    match parts.as_slice() {
        [m, s] => Some(m * 60 + s),
        [h, m, s] => Some(h * 3600 + m * 60 + s),
        _ => None,
    }
}
