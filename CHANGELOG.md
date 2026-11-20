# Changelog

All notable changes to **Sonata** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Word-synced and dual-voice lyrics pipeline: TTML (`lyrics-api.binimum.org`) with `Lead`/`Counter` voice alignment, KuGou `.krc` word-level timing, and NetEase `.yrc` support.
- High-resolution artwork upscaling and fallback lookup (`Deezer`, `iTunes`, Spotify/YouTube Music URL rewriting).
- Standalone POSIX installer ([`install.sh`](install.sh)), MIT [`LICENSE`](LICENSE), project documentation ([`docs/`](docs/architecture.md)), [`PRIVACY.md`](PRIVACY.md), [`CONTRIBUTING.md`](CONTRIBUTING.md), and [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md).
- Embedded `Inter` font family in `sonata-desktop` with dev-profile optimizations (`opt-level = 3`) for `image`, `cosmic-text`, and `swash`.

### Changed

- Reorganized oversized source modules in `sonata-core`, `sonata-tui`, and `sonata-desktop` into focused submodules (`backend/`, `app/`, `views/root/`, `views/fullscreen/`, `views/fullscreen_fx/`, `providers/ytmusic/`, `providers/spotify/`).
- Improved `sonata-desktop` fullscreen playback: controls moved under album artwork, alpha-invariant `BlurredLyricCache` with pre-warming, `1.2s` fast return from manual scroll, and `50%` vertical dead-center active lyric anchoring.

### Fixed

- Restricted browser cookie storage (`0o600` on creation) and private runtime/cache directories (`0o700`) to avoid permission races.
- Fixed `sonata-desktop` config save dirty-flag handling, blocking shutdown polling (replaced with `sync_channel::recv_timeout`), and added warning logs for tray/MPRIS/window setup failures.

### Removed

- Removed the unused `crates/sonata-desktop-makepad` prototype from the workspace.

## [0.1.0] - 2026-11-20

### Added

- Initial Cargo workspace with `sonata-core`, `sonata-tui` (`sonata`), `sonata-desktop` (`sonata-desktop`), and `sonata-android` placeholder.
- Browser cookie authentication for YouTube Music and Spotify (`sp_dc`), two-way YouTube Music library sync, and public Spotify playlist import.
- Fast-start `mpv` JSON-IPC playback with background Hi-Fi upgrade, lookahead precaching, DSP presets, and shared bounded SQLite/media cache.
- Terminal UI (`sonata`) with Kitty/Sixel/Halfblock artwork, mouse-resizable panes, synced lyrics, Mini Mode, and TOML themes.
- Experimental GPUI Wayland desktop client (`sonata-desktop`) with ambient glass shaders, MPRIS2, System Tray, and `$XDG_RUNTIME_DIR/sonata/now_playing.json` export.
- Arch Linux `PKGBUILD` and `PKGBUILD.desktop` packaging.

[Unreleased]: https://github.com/MehmetCanWT/sonata/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MehmetCanWT/sonata/releases/tag/v0.1.0
