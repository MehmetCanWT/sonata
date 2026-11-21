//! Shared core engine for Sonata, consumed by the TUI and desktop front-ends. It owns everything that
//! is not presentation: provider clients, stream resolution, playback state, caching, lyrics, and diagnostics.
//!
//! 1. **Auth & Providers** (`auth`, `providers`): browser cookie extraction plus the YouTube Music, Spotify,
//!    and Hi-Fi stream-resolution clients behind shared `Track`/`Playlist` models.
//! 2. **Playback** (`player`): queue/shuffle/repeat state, a command coordinator, and the mpv IPC controller.
//! 3. **Persistence** (`cache`, `config`): SQLite metadata cache, on-disk media cache, and TOML configuration.
//! 4. **Content & Appearance** (`lyrics`, `media`, `theme`): synced lyrics, artwork handling, and theme loading.
//! 5. **Platform & Diagnostics** (`hyprland`, `telemetry`, `updater`, internal `redaction`):
//!    compositor integration, persistent logging, release update checks, and scrubbing of credentials
//!    from log output.

pub mod auth;
pub mod cache;
pub mod config;
pub mod hyprland;
pub mod lyrics;
pub mod media;
pub mod motion;
pub mod player;
pub mod providers;
pub mod telemetry;
pub mod theme;
pub mod updater;

pub(crate) mod redaction;
