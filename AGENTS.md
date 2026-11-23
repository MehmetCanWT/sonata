# AGENTS.md

Guidance for AI coding agents and contributors working in the **Sonata** repository.

## Workspace Layout

Sonata is a 5-crate Cargo workspace (`resolver = "2"`) plus a native Jetpack Compose Android application (`android/`):

- `crates/sonata-core` — Portable Rust engine (no UI dependencies, zero subprocesses, zero Linux-only crates):
  - `paths.rs`, `error.rs` — Explicit instance-scoped `CorePaths` and typed redacted `CoreError`
  - `auth/` — Portable cookie parsing, `SAPISIDHASH`, Spotify `sp_dc`, and private `0o600`/`0o700` session persistence
  - `cache/` & `cache.rs` — SQLite WAL metadata cache & bounded LRU media cache
  - `lyrics/` — Multi-source lyrics resolution (`lrc.red` TTML, BetterLyrics, LyricsPlus v2, KuGou KRC, NetEase YRC, QQ Music, LRCLIB, YouTube Music, Genius)
  - `player/` — Pure `PlaybackCoordinator`, `PlaybackGeneration`, `PlaybackTaskSet`, and `NextTrackPrecache`
  - `providers/` — Portable music providers (`providers/ytmusic/{mod,parsers,browse,album,search}.rs`, `providers/spotify/`, `providers/jiosaavn.rs`, `providers/profile.rs` `ResolverProfile`, Hi-Fi `TVHTML5` stream resolution & zero-AST `base.js` cipher hook)
  - `media.rs`, `motion.rs` — Artwork URL upscaling, Deezer/iTunes cover matching, and platform-neutral `MotionMedia` discovery (Apple Music AMP/HTML, Tidal, Community Canvas)
  - `redaction.rs`, `updater.rs` — Log/error redaction and per-platform GitHub Releases updater
- `crates/sonata-runtime` — Shared Linux runtime adapter crate backing `sonata-tui` and `sonata-desktop` (never depended on by `sonata-core` or `sonata-android`):
  - `config.rs`, `hyprland.rs`, `telemetry.rs` — XDG TOML config, Hyprland opacity IPC, rotating `sonata.log`
  - `auth/browser.rs` — Linux browser cookie extraction (`yt-dlp` / browser DBs)
  - `player/{mpv.rs,session.rs}` — Headless `mpv` JSON-IPC driver and shared Linux `PlaybackSession` (bounded command/event channels, coalesced `PlaybackProgress`, per-generation cancellation, recovery, upgrade swap/fallback, and precache adoption)
  - `process/{cipher.rs,ytdlp.rs}` — `CliJsCipherBridge` and `LinuxStreamResolver` (`yt-dlp` fast-start race & recovery)
  - `media.rs`, `motion.rs` — Image decoding/trimming, ambient color extraction, `now_playing.json`, and `ffmpeg` motion-cover decoding
- `crates/sonata-tui` — **`sonata`** (stable Ratatui terminal client):
  - `src/main.rs`, `src/input.rs`, `src/theme/`, `src/theme_tui.rs`
  - `src/app/` (`mod.rs`, `state.rs`, `playback.rs`, `library.rs`) — Thin adapter over `sonata_runtime::player::PlaybackSession` plus TUI-local lyrics/artwork/Kitty graphics
  - `src/ui/` — Terminal panes, modals, Kitty/Sixel/Halfblock artwork, and word-synced lyrics rendering
- `crates/sonata-desktop` — **`sonata-desktop`** (GPUI Wayland desktop client):
  - `src/main.rs`, `src/theme/`, `src/settings.rs`
  - `src/backend/` (`mod.rs`, `types.rs`, `playback.rs`, `library.rs`) — Thin adapter over `sonata_runtime::player::PlaybackSession` plus `DesktopSnapshot` watch state and desktop-local cover/motion/lyrics tasks
  - `src/views/` (`mod.rs`, `home.rs`, `explore.rs`, `artist.rs`, `album.rs`, `search.rs`, `fullscreen.rs`, `settings.rs`, etc.)
  - `src/integrations/` — MPRIS2 (`mpris.rs`) and StatusNotifierItem system tray (`tray.rs`)
- `crates/sonata-android` — Cancellable async UniFFI (`0.29.5`, `async_runtime = "tokio"`, zero `Runtime::block_on`) Rust bridge backing the Jetpack Compose Android app (`android/`).
- `android/app/` — Native Jetpack Compose + Media3 ExoPlayer Android client (JDK 21, `minSdk = 31`, `compileSdk = 35`, `targetSdk = 35`):
  - Top-level screens & chrome (`MainActivity.kt`, `SonataViewModel.kt`, `HomeScreen.kt`, `ExploreScreen.kt`, `LibraryScreen.kt`, `DetailScreen.kt`, `NowPlayingScreen.kt`, `SearchScreen.kt`, `SonataNavigationChrome.kt`, `YtMusicLoginSheet.kt`, `PlaybackService.kt`, `ApkUpdater.kt`)
  - `playback/PlaybackOrchestrator.kt` — Generation counter, generation-keyed recovery budget, typed rate-limit policy, atomic stream swap, and narrow ended-state transition guard
  - `ui/theme/` (`LunariaPalette.kt`, `BasicTheme.kt`, `VioletTheme.kt`, `MoonflowerTheme.kt`, `SonataGlassTweaks.kt`) — `Basic`, `Violet` & `Moonflower` light/dark/AMOLED theme systems (default: `Basic` + `ThemeMode.SYSTEM`)
  - `ui/components/` (`SonataShareSheet.kt`, `SonataAnimatedArtworks.kt`, `SonataCards.kt`, `HeldContextMenu.kt`, `TrackOptionsMenu.kt`, `UserMenuSheet.kt`, `AddToPlaylistSheet.kt`)
  - `ui/screens/` (`ArtistScreen.kt`, `RadioMixesTab.kt`, `CategorizedSettingsSheet.kt`, `DownloadsScreen.kt`), `ui/player/`, `ui/lyrics/`, `floatingtabbar/`, `backdrop/`, `widget/`
  - `data/` (`SonataEngineGateway.kt`, `SonataPreferences.kt`, `DownloadManager.kt`, `CanvasResolver.kt`, `LyricsTranslator.kt`, `AudioPipelineState.kt`, `ThumbnailFallback.kt`)
  - `src/test/java/dev/sonata/android/` — Kotlin/JVM JUnit tests (`SonataThemeAndPreferencesTest.kt`, `SonataDomainLogicTest.kt`, `JioSaavnAndMediaTest.kt`, `ShareAndChangelogTest.kt`, `SonataEngineGatewayCancellationTest.kt`, `PlaybackOrchestrationTest.kt`)
  - `src/androidTest/java/dev/sonata/android/` — Instrumented Android native-library & cancellation tests (`NativeEngineInstrumentedTest.kt`)

Note: Local UI/UX design references and Liquid Glass guidelines (`research/PROJECT.md`, `research/liquidglass.md`, `research/images/`, `research/theme/`) reside in `research/` (gitignored).

## Strict Dependency Boundaries

- `sonata-tui` and `sonata-desktop` depend on `sonata-core` and `sonata-runtime`.
- `sonata-android` depends on `sonata-core` and must **never** depend on `sonata-runtime` (or `image`, `dirs`, `toml`, `tracing-appender`).
- `sonata-core` must **never** depend on `sonata-runtime`, `sonata-tui`, `sonata-desktop`, `sonata-android`, `ratatui`, `crossterm`, `gpui-kit`, or `uniffi`, and must contain zero `(std|tokio)::process::Command` launches or provider `cfg!(target_os = "android")` branches.
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
./scripts/check_boundaries.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline -p sonata-core -p sonata-runtime -p sonata-desktop -p sonata-tui -p sonata-android
```

For Android changes (Kotlin/Compose and UniFFI integration — run with JDK 21):

```sh
cd android && ./gradlew testDebugUnitTest --console=plain
```

When an Android device or emulator is attached, also run the native instrumented suite:

```sh
cd android && ./gradlew connectedDebugAndroidTest --console=plain
```

When modifying `crates/sonata-android/src/lib.rs` UniFFI exports, also regenerate `android/app/src/main/java/uniffi/sonata_android/sonata_android.kt` from an unstripped debug cdylib (`target/debug/libsonata_android.so`) and rebuild `jniLibs` via `cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p sonata-android --release --locked`.

For fast single-client iteration during development, use `.cargo/config.toml` aliases (`cargo check-tui`, `cargo check-desktop`, `cargo check-android`, `cargo test-android`).

**Git policy**: Never run `git push` or rewrite shared remote history unless the user explicitly asks you to.

## Security & Sensitive Data

- **Cookie files**: Persist extracted browser cookies (`~/.cache/sonata/yt_cookies.txt`) and Android WebView session cookies (`ytm_session.cookie`) using atomic/restricted creation (`0o600` file mode on Unix before writing secrets) — never write with default permissions and `chmod` afterward.
- **Temporary & runtime directories**: Create private cache/runtime directories with `0o700` permissions on Unix.
- **Log & error redaction**: Always pass URLs, cookie headers, and auth tokens through `sonata_core::redaction` before logging (`~/.cache/sonata/sonata.log`) or surfacing errors. Never log raw `Cookie`, `Authorization`, `SAPISID`, or `sp_dc` values.
- **Settings backup**: `SonataPreferences.exportBackupJson()` must exclude all session cookies and credentials.
- **Blocking I/O**: Never call blocking `std::process::Command` or synchronous file/SQLite operations on async/UI threads without bounding or offloading via `tokio::task::spawn_blocking` (or `Dispatchers.IO` on Android).

## Commits, PRs & Changelog

- Use **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, `perf:`, `test:`, `chore:`), optionally scoped by crate/platform (e.g., `feat(core): add TTML dual-voice lyrics parser`, `feat(runtime): share Linux playback session`).
- Keep pull requests focused, small enough to review cleanly, and backed by tests for bug fixes or new parsers.
- Record user-facing changes under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) following the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.
