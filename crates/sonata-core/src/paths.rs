//! Explicitly injected configuration and cache directory roots for `sonata-core`.
//!
//! 1. **Root Container**: `CorePaths` stores already-resolved configuration and cache directories
//!    without calling `dirs` or inspecting the target operating system.
//! 2. **Derived Locations**: Provides canonical methods for every persisted file and subdirectory
//!    used by authentication, SQLite metadata, lyrics, artwork, audio, cipher, and log storage.

use std::path::{Path, PathBuf};

/// Explicit, platform-neutral root directories injected into `sonata-core` components.
///
/// Linux clients construct this from XDG defaults, Android clients from app-private `filesDir`
/// and `cacheDir`, and tests from isolated temporary directories.
///
/// # Examples
///
/// ```
/// use sonata_core::paths::CorePaths;
/// use std::path::PathBuf;
///
/// let paths = CorePaths::for_app_roots(PathBuf::from("/tmp/cfg"), PathBuf::from("/tmp/cache"));
/// assert_eq!(paths.config_dir, PathBuf::from("/tmp/cfg/sonata"));
/// assert_eq!(paths.cache_dir, PathBuf::from("/tmp/cache/sonata"));
/// assert_eq!(paths.audio_cache_dir(), PathBuf::from("/tmp/cache/sonata/audio"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorePaths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl CorePaths {
    /// Creates `CorePaths` from already-resolved Sonata configuration and cache directories.
    pub fn new(config_dir: impl Into<PathBuf>, cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
            cache_dir: cache_dir.into(),
        }
    }

    /// Creates `CorePaths` by appending `"sonata"` to parent base directories
    /// (such as XDG roots on Linux or `filesDir`/`cacheDir` on Android).
    pub fn for_app_roots(config_root: impl AsRef<Path>, cache_root: impl AsRef<Path>) -> Self {
        Self {
            config_dir: config_root.as_ref().join("sonata"),
            cache_dir: cache_root.as_ref().join("sonata"),
        }
    }

    /// Directory containing custom terminal or desktop theme definitions.
    pub fn themes_dir(&self) -> PathBuf {
        self.config_dir.join("themes")
    }

    /// Path to the persistent TOML application configuration file.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Path to the persistent self-hosted music server profiles configuration JSON (`0o600`).
    pub fn servers_config_path(&self) -> PathBuf {
        self.config_dir.join("servers.json")
    }

    /// Path to the persisted YouTube Music WebView session cookie file.
    pub fn ytm_session_cookie_path(&self) -> PathBuf {
        self.config_dir.join("ytm_session.cookie")
    }

    /// Directory containing cached synced and plain lyrics files.
    pub fn lyrics_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("lyrics")
    }

    /// Directory containing cached artwork and motion-cover assets.
    pub fn art_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("art")
    }

    /// Directory containing cached audio stream files and quality sidecars.
    pub fn audio_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("audio")
    }

    /// Path to the shared SQLite metadata cache database.
    pub fn cache_db_path(&self) -> PathBuf {
        self.cache_dir.join("sonata_cache.db")
    }

    /// Path to the extracted Netscape-format YouTube cookie jar.
    pub fn yt_cookies_path(&self) -> PathBuf {
        self.cache_dir.join("yt_cookies.txt")
    }

    /// Path to the cached YouTube `base.js` player metadata JSON.
    pub fn cipher_player_cache_path(&self) -> PathBuf {
        self.cache_dir.join("yt_cipher_player.json")
    }

    /// Path to a hooked YouTube `base.js` script cached by sanitized player hash.
    pub fn cipher_script_cache_path(&self, safe_hash: &str) -> PathBuf {
        self.cache_dir.join(format!("yt_cipher_{safe_hash}.js"))
    }

    /// Path to the cached Spotify web-player bootstrap metadata JSON.
    pub fn spotify_webplayer_cache_path(&self) -> PathBuf {
        self.cache_dir.join("spotify_webplayer.json")
    }

    /// Path to the persistent client diagnostic log file.
    pub fn log_path(&self) -> PathBuf {
        self.cache_dir.join("sonata.log")
    }
}

#[cfg(test)]
mod tests {
    use super::CorePaths;
    use std::path::PathBuf;

    #[test]
    fn derives_expected_sonata_subpaths_from_injected_roots() {
        let paths = CorePaths::for_app_roots("/srv/files", "/srv/cache");
        assert_eq!(paths.config_dir, PathBuf::from("/srv/files/sonata"));
        assert_eq!(paths.cache_dir, PathBuf::from("/srv/cache/sonata"));
        assert_eq!(
            paths.themes_dir(),
            PathBuf::from("/srv/files/sonata/themes")
        );
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/srv/files/sonata/config.toml")
        );
        assert_eq!(
            paths.servers_config_path(),
            PathBuf::from("/srv/files/sonata/servers.json")
        );
        assert_eq!(
            paths.ytm_session_cookie_path(),
            PathBuf::from("/srv/files/sonata/ytm_session.cookie")
        );
        assert_eq!(
            paths.lyrics_cache_dir(),
            PathBuf::from("/srv/cache/sonata/lyrics")
        );
        assert_eq!(
            paths.art_cache_dir(),
            PathBuf::from("/srv/cache/sonata/art")
        );
        assert_eq!(
            paths.audio_cache_dir(),
            PathBuf::from("/srv/cache/sonata/audio")
        );
        assert_eq!(
            paths.cache_db_path(),
            PathBuf::from("/srv/cache/sonata/sonata_cache.db")
        );
        assert_eq!(
            paths.yt_cookies_path(),
            PathBuf::from("/srv/cache/sonata/yt_cookies.txt")
        );
        assert_eq!(
            paths.cipher_player_cache_path(),
            PathBuf::from("/srv/cache/sonata/yt_cipher_player.json")
        );
        assert_eq!(
            paths.cipher_script_cache_path("abc123"),
            PathBuf::from("/srv/cache/sonata/yt_cipher_abc123.js")
        );
        assert_eq!(
            paths.spotify_webplayer_cache_path(),
            PathBuf::from("/srv/cache/sonata/spotify_webplayer.json")
        );
        assert_eq!(
            paths.log_path(),
            PathBuf::from("/srv/cache/sonata/sonata.log")
        );
    }
}
