//! Portable shared core engine for Sonata, consumed by Linux desktop/TUI clients and Android.
//! It owns protocol and domain logic without OS-specific subprocesses, XDG directory probing,
//! image decoding, or UI dependencies.
//!
//! 1. **Auth & Providers** (`auth`, `providers`): portable cookie/SAPISIDHASH handling plus
//!    YouTube Music, Spotify, JioSaavn, and InnerTube stream resolution behind shared models.
//! 2. **Playback State** (`player`): queue/shuffle/repeat state, command coordinator, and
//!    monotonically increasing playback generation tokens.
//! 3. **Persistence & Paths** (`cache`, `paths`, `playlists`): SQLite metadata cache, bounded
//!    LRU media cache, explicit caller-injected directory roots, and local personal playlists.
//! 4. **Content & Media Discovery** (`lyrics`, `media`, `motion`): multi-source synced lyrics,
//!    artwork URL resolution/upscaling, and animated cover manifest discovery.
//! 5. **Errors, Updates & Redaction** (`error`, `updater`, `redaction`): typed core error model,
//!    release update checks, and scrubbing of secrets from logs and diagnostics.

pub mod auth;
pub mod cache;
pub mod error;
pub mod lyrics;
pub mod media;
pub mod motion;
pub mod paths;
pub mod player;
pub mod playlists;
pub mod providers;
pub mod redaction;
pub mod updater;
