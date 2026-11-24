//! Linux runtime adapters shared by `sonata-tui` and `sonata-desktop`.
//!
//! 1. **Config & Telemetry** (`config`, `telemetry`, `hyprland`): TOML/XDG application config,
//!    cross-distro binary lookup, persistent file logging, and Hyprland compositor IPC.
//! 2. **Browser Auth** (`auth`): Firefox/Chromium browser cookie extraction backed by local
//!    profile discovery and `yt-dlp`.
//! 3. **Media & Motion Decoding** (`media`, `motion`): image decoding, letterbox trimming,
//!    ambient color extraction, `now_playing.json` runtime output, and `ffmpeg` motion-cover decoding.
//! 4. **Playback & Process Runners** (`player`, `process`): `mpv` JSON-IPC driver, CLI JS cipher
//!    bridge (`node`/`bun`/`deno`), and `LinuxStreamResolver` composing portable `sonata-core`
//!    resolution with `yt-dlp`.
//! 5. **Discord Rich Presence & Connect** (`discord`, `connect`): Local IPC v1 Unix socket client
//!    with speed-scaled timestamps plus LAN Sonata Connect discovery and HTTP/JSON control server.

pub mod auth;
pub mod config;
pub mod connect;
pub mod discord;
pub mod downloads;
pub mod hyprland;
pub mod i18n;
pub mod media;
pub mod player;
pub mod process;
pub mod telemetry;

pub use media::motion;
