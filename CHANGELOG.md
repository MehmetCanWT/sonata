# Changelog

All notable changes to **Sonata** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Standalone Desktop GUI one-line installer ([`install-desktop.sh`](install-desktop.sh)) that installs runtime/Wayland/Vulkan dependencies, builds `sonata-desktop` using `nproc - 2` jobs, and registers the Moonlight Sonata SVG icon and `.desktop` launcher.
- Separate TUI and Desktop built-in themes with authentic **Sonata Glass**, **Spotify**, and **YouTube Music** skins in `sonata-desktop`.
- Multi-page personalized YouTube Music Home feed with continuation pagination, personal mixes (`watchPlaylistEndpoint` / `next` fallback), and interactive *Find your mood* / *Moods & genres* chips.
- In-RAM `MotionCover` animated Apple Music album artwork pipeline (`768x768` H.264 HLS segment cache + direct `ffmpeg` raw RGBA8 pipe) for smooth looping playback in both `sonata-desktop` (24 fps GPU textures) and `sonata-tui` (20 fps Kitty Graphics GPU frame swapping).
- Android architecture & research plan ([`docs/android-research.md`](docs/android-research.md)) targeting **Kotlin (Jetpack Compose) + UniFFI** (Coming Soon).
- Word-synced and dual-voice lyrics pipeline: TTML (`lyrics-api.binimum.org`) with `Lead`/`Counter` voice alignment, KuGou `.krc` word-level timing, NetEase `.yrc`, and QQ Music base64 LRC support, queried concurrently and ranked via `score_synced_lyrics`.
- High-resolution artwork upscaling and fallback lookup (`Deezer`, `iTunes`, Spotify/YouTube Music URL rewriting).
- Waybar/Hyprlock-ready fields (`track_id`, `is_playing`, `is_paused`, `liked`, `progress_ratio`, `progress_percent`, `remaining_secs`, `remaining`, `text`, `alt`, `class`, `tooltip`) in `now_playing.json`.
- Standalone POSIX TUI installer ([`install.sh`](install.sh)) and updated [`PKGBUILD`](PKGBUILD) / [`PKGBUILD.desktop`](PKGBUILD.desktop) that automatically install all required runtime tools (`mpv`, `yt-dlp`, `ffmpeg`, `nodejs`), MIT [`LICENSE`](LICENSE), project documentation ([`docs/`](docs/architecture.md)), [`PRIVACY.md`](PRIVACY.md), [`CONTRIBUTING.md`](CONTRIBUTING.md), and [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md).
- Embedded `Inter` font family in `sonata-desktop` with dev-profile optimizations (`opt-level = 3`) for `image`, `cosmic-text`, and `swash`.

### Changed

- Promoted `sonata-desktop` to daily-use status in [`README.md`](README.md) and [`PKGBUILD.desktop`](PKGBUILD.desktop), and tuned `.cargo/config.toml` build parallelism (`jobs = 10` on 12-thread hosts, leaving 2 threads for system responsiveness).
- Reorganized oversized source modules in `sonata-core`, `sonata-tui`, and `sonata-desktop` into focused submodules (`backend/`, `app/`, `views/root/`, `views/fullscreen/`, `views/fullscreen_fx/`, `providers/ytmusic/`, `providers/spotify/`).
- Redesigned the `sonata-tui` Settings modal so labels never truncate, added 3-tier responsive Mini Player auto-activation (`< 90x20` in TUI, `< 760x460` in Desktop), and improved `sonata-desktop` fullscreen playback (controls under album artwork, alpha-invariant `BlurredLyricCache` with pre-warming, `1.2s` fast return from manual scroll, `50%` vertical dead-center active lyric anchoring).

### Fixed

- Fixed blank track lists in `sonata-desktop` playlist/queue views caused by a missing flex column layout on the `uniform_list` container, and tightened the bottom player bar Like button placement next to the track title.
- Fixed premature EOF track-skipping when seeking forward on uncached fast-start `googlevideo.com` streams by enabling `mpv` HTTP reconnection (`--force-seekable=yes`, `--stream-lavf-o=reconnect=1,...`) and recovering at the seeked timestamp in both `sonata-tui` and `sonata-desktop`.
- Fixed Hyprland `0.57.2` window opacity control (`hyprctl eval` Lua `set_prop` primary with `hyprctl --batch` fallback) and locked opacity settings to Solid outside Hyprland.
- Fixed `sonata-desktop` Wayland/Hyprland fullscreen and top-gap geometry desync (`WindowDecorations::Server` + `titlebar: None`, `280x110` min size) and `sonata-tui` resize/Kitty placement artifacts.
- Added `libxkbcommon-x11-dev` and `libxcb1-dev` to `.github/workflows/ci.yml` to fix `sonata-desktop` linking on CI.
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
