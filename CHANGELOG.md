# Changelog

All notable changes to **Sonata** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-21

### Added

- **Android Client (`Sonata` Mobile & Tablet)**: Native Jetpack Compose + Media3 ExoPlayer client powered by `sonata-core` via UniFFI (`crates/sonata-android`), featuring full-bleed adaptive portrait Now Playing, two-column tablet landscape Now Playing with persistent media controls, multi-column tablet Explore/Library grids, interactive YouTube Music sign-in, fresh live Home feed on every launch, and an offscreen V8 WebView JS cipher bridge.
- **120 Hz Android Performance & R8 Release Build**: Isolated `@Stable PlaybackClockState` (eliminating 10 Hz root recomposition storms), Compose compiler stability config for UniFFI types, Coil 3 hardware-bitmap & exact pixel downsampling cache, cached Liquid Glass shadow/highlight layers, zero Main-thread cold-start blocking (`~202 ms` cold launch, `< 1%` jank at 120 Hz), and R8 minification + resource shrinking (`~19 MB` release APK).
- **In-App Android APK Updater & Per-Platform Update Notifications**: Automatic startup update popup with live background download progress (`MB` / `%`) and seamless Android `FileProvider` package installer handoff (`ApkUpdater.kt`), plus per-platform release stream filtering (`v*` unified, `android-v*` Android-only, `desktop-v*` Desktop/TUI-only) in `crates/sonata-core/src/updater.rs`.
- **Native Extreme Audio (`itag=774` / `itag=141`)**: Authenticated `TVHTML5` (`tv_samsung` `5.20260707`) InnerTube client and zero-AST `base.js` cipher hook (`crates/sonata-core/src/providers/hifi/cipher.rs`) that unlocks YouTube Music Premium **`itag=774` (~256–321 kbps Opus Extreme)**, **`itag=141` (256 kbps AAC Extreme)**, and age-restricted streams across Android, Desktop, and TUI.
- **High-Precision Word-Synced & Duet Lyrics**: Concurrent `lrc.red` (ISRC-keyed Apple Music TTML), BetterLyrics, LyricsPlus v2, and LRCLIB/KuGou/NetEase/QQ lookup with strict version-word isolation (`Extended`, `Remix`, `Slowed`, `Live`, etc.), tight `<= 3.5s` duration matching, duet `Lead`/`Counter` alignment, nested TTML leaf-span parsing, and a steered monotonic karaoke clock.
- **Built-in Update Notifications (Desktop & TUI)**: Automatic startup GitHub Releases checks in `sonata-desktop` (header pill + clickable **Download Update** button in Settings) and `sonata` TUI (`[Update vX.Y.Z]` player bar badge + status alert + interactive Settings check).
- **Automated GitHub Release Builds (`.github/workflows/release-builds.yml`)**: Pushing `vX.Y.Z`, `android-vX.Y.Z`, or `desktop-vX.Y.Z` tags automatically builds and publishes versioned **Linux `x86_64` AppImage**, **Android `arm64-v8a` APK**, `SHA256SUMS.txt`, and structured `CHANGELOG.md` release notes.
- **Unified Moonlight Crescent Brand Identity**: New minimalist crescent moon emblem (`assets/sonata-logo.svg` / `assets/sonata-desktop.svg`) shared across README, Linux AppImage, desktop `.desktop` entry, and Android adaptive launcher icon.
- **Desktop GUI Skins & Personalized Home Feed**: Selectable **Sonata Glass**, **Spotify**, and **YouTube Music** skins in `sonata-desktop`, multi-page personalized YouTube Music Home feed with continuation pagination, personal mixes, and interactive *Find your mood* pills.
- **Animated Motion Covers**: In-RAM Apple Music `768x768` H.264 HLS animated album artwork pipeline for smooth looping playback in both `sonata-desktop` (24 fps GPU textures) and `sonata-tui` (20 fps Kitty Graphics GPU frame swapping).
- **One-Line Installers & Arch Packaging**: Standalone POSIX installers ([`install.sh`](install.sh), [`install-desktop.sh`](install-desktop.sh)) and updated [`PKGBUILD`](PKGBUILD) / [`PKGBUILD.desktop`](PKGBUILD.desktop) targeting `1.0.0`.
- **Waybar / Hyprlock Rich Telemetry**: Extended `now_playing.json` fields (`track_id`, `is_playing`, `is_paused`, `liked`, `progress_ratio`, `progress_percent`, `remaining_secs`, `remaining`, `text`, `alt`, `class`, `tooltip`).

### Changed

- Adopted Semantic Versioning starting at **`1.0.0`** across Cargo workspace crates, Android `versionName` / `versionCode` (`MAJOR * 10000 + MINOR * 100 + PATCH`), AUR `PKGBUILD`s, and GitHub Release artifacts.
- Bumped SQLite cache schema (`PRAGMA user_version = 3`) with automatic one-time invalidation of legacy durationless lyrics cache entries and unified `clear_lyrics_cache()` support.
- Reorganized `sonata-core`, `sonata-tui`, and `sonata-desktop` into modular submodules and improved 3-tier responsive Mini Player auto-activation and fullscreen playback ergonomics.

### Fixed

- Fixed blank track lists in `sonata-desktop` playlist/queue views and tightened player bar Like button placement.
- Fixed premature EOF track-skipping when seeking forward on uncached `googlevideo.com` streams via `mpv` HTTP reconnection and timestamp recovery.
- Fixed Hyprland `0.57.2` window opacity control, Wayland fullscreen geometry desync, and TUI resize/Kitty placement artifacts.
- Fixed Android lyric startup/buffering drift, stale SQLite lyrics cache persistence, and cross-version lyrics mismatches.
- Enforced `0o600` cookie file permissions and `0o700` private cache/runtime directories on creation.

## [0.1.0] - 2026-11-20

### Added

- Initial Cargo workspace with `sonata-core`, `sonata-tui` (`sonata`), `sonata-desktop` (`sonata-desktop`), and `sonata-android` placeholder.
- Browser cookie authentication for YouTube Music and Spotify (`sp_dc`), two-way YouTube Music library sync, and public Spotify playlist import.
- Fast-start `mpv` JSON-IPC playback with background Hi-Fi upgrade, lookahead precaching, DSP presets, and shared bounded SQLite/media cache.
- Terminal UI (`sonata`) with Kitty/Sixel/Halfblock artwork, mouse-resizable panes, synced lyrics, Mini Mode, and TOML themes.
- Initial GPUI Wayland desktop client (`sonata-desktop`) with ambient glass shaders, MPRIS2, System Tray, and `now_playing.json` export.
- Arch Linux `PKGBUILD` and `PKGBUILD.desktop` packaging.

[Unreleased]: https://github.com/MehmetCanWT/sonata/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.0
