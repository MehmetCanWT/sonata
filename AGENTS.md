# AGENTS.md

Guidance for AI coding agents and contributors working in the **Sonata** repository.

## Workspace Layout

Sonata is a 4-crate Cargo workspace (`resolver = "2"`) plus a native Jetpack Compose Android application (`android/`):

- `crates/sonata-core` — Shared Rust engine (no UI dependencies):
  - `auth/` — Browser cookie extraction & session auth (`cookies.rs`: `SAPISIDHASH`, Spotify `sp_dc`)
  - `cache/` & `cache.rs` — SQLite WAL metadata cache & bounded LRU media cache
  - `lyrics/` — Multi-source lyrics resolution (`lrc.red` TTML, BetterLyrics, LyricsPlus v2, KuGou KRC, NetEase YRC, QQ Music, LRCLIB, YouTube Music, Genius)
  - `player/` — `mpv` JSON-IPC driver, playback coordinator, Hi-Fi upgrade & lookahead precache
  - `providers/` — Music providers (`providers/ytmusic/`, `providers/spotify/`, Hi-Fi `TVHTML5` stream resolution & zero-AST `base.js` cipher hook)
  - `media.rs`, `motion.rs` — High-res artwork lookup, URL upscaling, Apple Music HLS motion covers
  - `config.rs`, `theme.rs`, `hyprland.rs`, `redaction.rs`, `telemetry.rs`, `updater.rs`
- `crates/sonata-tui` — **`sonata`** (stable Ratatui terminal client):
  - `src/main.rs`, `src/input.rs`, `src/theme_tui.rs`
  - `src/app/` (`mod.rs`, `state.rs`, `playback.rs`, `library.rs`)
  - `src/ui/` — Terminal panes, modals, Kitty/Sixel/Halfblock artwork, and word-synced lyrics rendering
- `crates/sonata-desktop` — **`sonata-desktop`** (experimental GPUI Wayland desktop client):
  - `src/main.rs`, `src/theme_bridge.rs`
  - `src/backend/` (`mod.rs`, `types.rs`, `playback.rs`, `library.rs`)
  - `src/views/` (`mod.rs`, `root/{mod,header,sidebar,stage,player_bar}.rs`, `fullscreen/{mod,sheet,controls}.rs`, `fullscreen_fx/{mod,palette,waves,blur_cache}.rs`)
  - `src/integrations/` — MPRIS2 (`mpris.rs`) and StatusNotifierItem system tray (`tray.rs`)
- `crates/sonata-android` — UniFFI (`0.29`) Rust bridge backing the Jetpack Compose Android app (`android/`).
- `android/app/` — Native Jetpack Compose + Media3 ExoPlayer Android client (JDK 21, `minSdk = 31`, `compileSdk = 35`, `targetSdk = 35`):
  - Top-level screens & chrome (`MainActivity.kt`, `SonataViewModel.kt`, `HomeScreen.kt`, `ExploreScreen.kt`, `LibraryScreen.kt`, `DetailScreen.kt`, `NowPlayingScreen.kt`, `SearchScreen.kt`, `SonataNavigationChrome.kt`, `YtMusicLoginSheet.kt`, `PlaybackService.kt`, `ApkUpdater.kt`)
  - `ui/theme/` (`LunariaPalette.kt`, `VioletTheme.kt`, `MoonflowerTheme.kt`, `SonataGlassTweaks.kt`) — Dual `Violet` & `Moonflower` light/dark/AMOLED theme systems (default: `Violet` + `ThemeMode.SYSTEM`)
  - `ui/components/` (`SonataShareSheet.kt`, `SonataAnimatedArtworks.kt`, `SonataCards.kt`, `HeldContextMenu.kt`, `TrackOptionsMenu.kt`, `UserMenuSheet.kt`, `AddToPlaylistSheet.kt`)
  - `ui/screens/` (`ArtistScreen.kt`, `RadioMixesTab.kt`, `CategorizedSettingsSheet.kt`, `DownloadsScreen.kt`), `ui/player/`, `ui/lyrics/`, `floatingtabbar/`, `backdrop/`, `widget/`
  - `data/` (`SonataPreferences.kt`, `DownloadManager.kt`, `CanvasResolver.kt`, `JioSaavnSource.kt`, `LyricsTranslator.kt`, `AudioPipelineState.kt`, `ThumbnailFallback.kt`)
  - `src/test/java/dev/sonata/android/` — Kotlin/JVM JUnit tests (`SonataThemeAndPreferencesTest.kt`, `SonataDomainLogicTest.kt`, `JioSaavnAndMediaTest.kt`, `ShareAndChangelogTest.kt`)

Note: Local UI/UX design references and Liquid Glass guidelines (`research/PROJECT.md`, `research/liquidglass.md`, `research/images/`, `research/theme/`) reside in `research/` (gitignored).

## Strict Dependency Boundaries

- `sonata-tui`, `sonata-desktop`, and `sonata-android` depend on `sonata-core`.
- `sonata-core` must **never** depend on `sonata-tui`, `sonata-desktop`, `sonata-android`, `ratatui`, `crossterm`, or `gpui-kit`.
- Workspace-wide dependencies live in root `Cargo.toml` under `[workspace.dependencies]`; add crate dependencies via `<dep>.workspace = true`.
- Workspace lints (`unsafe_op_in_unsafe_fn = "deny"`, `dbg_macro = "deny"`, `todo = "deny"`, `undocumented_unsafe_blocks = "deny"`) apply to all workspace crates.
- Android runtime APK remains 100% FOSS without proprietary tracking SDKs or `REQUEST_INSTALL_PACKAGES` (`dependenciesInfo` disabled in `android/app/build.gradle.kts`).

## Module Documentation & Language Standards

Every Rust source module (`*.rs`) must begin with a structured `//!` module doc comment containing:

1. A concise 1–2 sentence overview of what the module owns.
2. A numbered breakdown (`//! 1. **Component**: ...`) describing its primary responsibilities:

```rust
//! Brief 1–2 sentence module overview.
//!
//! 1. **Component**: Responsibility summary.
//! 2. **Pipeline**: Responsibility summary.
```

All code comments, docstrings, documentation files (`*.md`), UI strings, CLI output, and logs must be written in **English**.

## Required Verification Checks

Run these checks before finishing any Rust change:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline -p sonata-core -p sonata-desktop -p sonata-tui -p sonata-android
```

For Android changes (Kotlin/Compose and UniFFI integration):

```sh
cd android && ./gradlew testDebugUnitTest --console=plain
```

When modifying `crates/sonata-android/src/lib.rs` UniFFI exports, also regenerate `android/app/src/main/java/uniffi/sonata_android/sonata_android.kt` from an unstripped debug cdylib (`target/debug/libsonata_android.so`) and rebuild `jniLibs` via `cargo ndk`.

For fast single-client iteration during development, use `.cargo/config.toml` aliases (`cargo check-tui`, `cargo check-desktop`, `cargo check-android`, `cargo test-android`).

**Git policy**: Never run `git push` or rewrite shared remote history unless the user explicitly asks you to.

## Security & Sensitive Data

- **Cookie files**: Persist extracted browser cookies (`~/.cache/sonata/yt_cookies.txt`) and Android WebView session cookies (`ytm_session.cookie`) using atomic/restricted creation (`0o600` file mode on Unix before writing secrets) — never write with default permissions and `chmod` afterward.
- **Temporary & runtime directories**: Create private cache/runtime directories with `0o700` permissions on Unix.
- **Log & error redaction**: Always pass URLs, cookie headers, and auth tokens through `sonata_core::redaction` before logging (`~/.cache/sonata/sonata.log`) or surfacing errors. Never log raw `Cookie`, `Authorization`, `SAPISID`, or `sp_dc` values.
- **Settings backup**: `SonataPreferences.exportBackupJson()` must exclude all session cookies and credentials.
- **Blocking I/O**: Never call blocking `std::process::Command` or synchronous file/SQLite operations on async/UI threads without bounding or offloading via `tokio::task::spawn_blocking` (or `Dispatchers.IO` on Android).

## Commits, PRs & Changelog

- Use **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, `perf:`, `test:`, `chore:`), optionally scoped by crate/platform (e.g., `feat(core): add TTML dual-voice lyrics parser`, `feat(android): add Moonflower & Violet themes`).
- Keep pull requests focused, small enough to review cleanly, and backed by tests for bug fixes or new parsers.
- Record user-facing changes under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) following the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.
