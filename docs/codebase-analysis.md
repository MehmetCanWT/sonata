# Sonata Codebase Analysis

Snapshot of workspace structure, modular splits across Rust and Android, resolved fixes, and open refactoring opportunities (`v1.0.6`).

## 1. File-Size and Modularity Breakdown

The Rust workspace contains **55,414 Rust lines** across **5 crates** (`sonata-core`: 19,390 total / 18,628 src; `sonata-runtime`: 7,646 total / 6,201 src; `sonata-desktop`: 20,074; `sonata-tui`: 6,379; `sonata-android`: 1,925 total / 1,760 src), paired with **47,125 Kotlin main lines** (excluding generated UniFFI bindings) and **2,007 Kotlin test lines** under `android/`.

Large Rust and Android monoliths have been split into single-responsibility modules:

| Component | Split modules |
|---|---|
| `sonata-core` engine & providers | `providers/ytmusic/{mod (2,016), parsers (2,222), browse, album, search}.rs`, `providers/spotify/{mod, embed}.rs`, `providers/hifi/{innertube (952), cipher, matching, cache}.rs`, `providers/jiosaavn.rs` (954), `lyrics/providers.rs` (1,519), `motion.rs` (1,260), `player/{mod, coordinator, tasks, precache}.rs` |
| `sonata-runtime` Linux adapters | `player/{mpv, session (2,134)}.rs`, `process/{cipher, ytdlp}.rs`, `media/{mod, motion}.rs`, `auth/browser.rs`, `config.rs`, `hyprland.rs`, `telemetry.rs`, `i18n.rs` |
| `sonata-desktop` backend & views | `backend/{mod (1,861), library (1,272), playback, types}.rs`, `ui/{components (991), lyrics, media, mod}.rs`, `views/{mod (1,684), fullscreen (1,848), home (960), artist, chrome, collection, explore, library, modals, player_bar, right_panel, search, settings, sidebar}.rs`, `theme/{mod, custom, ramp, system}.rs` (+ 29 CSS themes), `integrations/{mpris, tray}.rs` |
| `sonata-tui` engine & UI | `app/{mod, state, playback, library}.rs`, `ui/{mod, fx, modals, now_playing, sidebar, track_list}.rs`, `theme/{mod, tui}.rs` |
| `sonata-android` bridge | [`crates/sonata-android/src/lib.rs`](../crates/sonata-android/src/lib.rs) (1,752 lines, cancellable async UniFFI `0.29.5` Tokio bridge) + `tests/async_cancellation.rs` |
| Android UI, playback & data | `SonataViewModel.kt` (4,153), `CategorizedSettingsSheet.kt` (2,826), `NowPlayingScreen.kt` (2,427), `SonataDesignSystem.kt` (2,209), `ArtistScreen.kt` (1,774), `MainActivity.kt` (1,735), `HomeScreen.kt` (1,485), `playback/PlaybackOrchestrator.kt`, `data/{SonataEngineGateway, SonataPreferences, DownloadManager, CanvasResolver, AudioPipelineState}.kt`, `ui/theme/{BasicTheme, VioletTheme, MoonflowerTheme, LunariaPalette, SonataGlassTweaks}.kt`, `ui/screens/BasicThemeOverhaul.kt`, `ui/i18n/SonataStrings.kt`, `ui/components/{SonataShareSheet, SonataAnimatedArtworks, SonataCards, EditPlaylistDialog, ...}.kt` |

Largest single files to keep an eye on for future modularization:
- Android UI & ViewModel: [`SonataViewModel.kt`](../android/app/src/main/java/dev/sonata/android/SonataViewModel.kt) (4,153 lines), [`CategorizedSettingsSheet.kt`](../android/app/src/main/java/dev/sonata/android/ui/screens/CategorizedSettingsSheet.kt) (2,826 lines), [`NowPlayingScreen.kt`](../android/app/src/main/java/dev/sonata/android/NowPlayingScreen.kt) (2,427 lines), and [`SonataDesignSystem.kt`](../android/app/src/main/java/dev/sonata/android/ui/SonataDesignSystem.kt) (2,209 lines).
- Rust core, runtime & desktop: [`crates/sonata-core/src/providers/ytmusic/parsers.rs`](../crates/sonata-core/src/providers/ytmusic/parsers.rs) (2,222 lines), [`crates/sonata-runtime/src/player/session.rs`](../crates/sonata-runtime/src/player/session.rs) (2,134 lines), [`crates/sonata-core/src/providers/ytmusic/mod.rs`](../crates/sonata-core/src/providers/ytmusic/mod.rs) (2,016 lines), [`crates/sonata-desktop/src/backend/mod.rs`](../crates/sonata-desktop/src/backend/mod.rs) (1,861 lines), [`crates/sonata-desktop/src/views/fullscreen.rs`](../crates/sonata-desktop/src/views/fullscreen.rs) (1,848 lines), and [`crates/sonata-core/src/lyrics/providers.rs`](../crates/sonata-core/src/lyrics/providers.rs) (1,519 lines).

## 2. Resolved Security, Reliability & Architecture Milestones

| Area | Verified implementation in codebase |
|---|---|
| **ADR-0001 crate boundaries** | Extracted Linux-only adapters into [`crates/sonata-runtime`](../crates/sonata-runtime); eliminated subprocess launches, `cfg!(target_os = "android")` provider branches, and global path/policy statics from [`crates/sonata-core`](../crates/sonata-core); enforced automatically by [`scripts/check_boundaries.sh`](../scripts/check_boundaries.sh). |
| **Shared Linux `PlaybackSession` & `watch` progress** | Consolidated TUI and Desktop `mpv` orchestration into [`sonata-runtime/src/player/session.rs`](../crates/sonata-runtime/src/player/session.rs) with bounded channels and a separate coalesced `PlaybackProgress` `watch` channel (`sonata-desktop` no longer clones shelf vectors on progress ticks). |
| **Cancellable async UniFFI bridge** | [`crates/sonata-android/src/lib.rs`](../crates/sonata-android/src/lib.rs) exports `#[uniffi::export(async_runtime = "tokio")]` futures with zero `Runtime::block_on`, paired with `SonataEngineGateway.kt` and `PlaybackOrchestrator.kt` generation isolation. |
| **Consolidated providers & motion discovery** | Removed Desktop `ytm_extra.rs` and Android `JioSaavnSource.kt`; unified YTM parsers, JioSaavn DES-ECB/320 kbps matching, and `MotionMedia` discovery in `sonata-core` (with explicit `CorePaths`, `ResolverProfile`, and redacted `CoreError`). |
| **Cookie permissions, backups & redaction** | [`crates/sonata-core/src/auth/cookies.rs`](../crates/sonata-core/src/auth/cookies.rs) enforces `0o600`/`0o700` permissions; [`redaction.rs`](../crates/sonata-core/src/redaction.rs) scrubs headers/URLs; `SonataPreferences.exportBackupJson()` excludes credentials. |
| **Android FOSS posture & APK updater** | Disabled `dependenciesInfo` in `android/app/build.gradle.kts`, omitted tracking/analytics SDKs, and scoped `REQUEST_INSTALL_PACKAGES` strictly to user-initiated GitHub Releases APK updates via `ApkUpdater.kt` + `FileProvider`. |
| **Themes, localization & YTM playlist editing** | Added monochrome `Basic` theme (`BasicTheme.kt`, `BasicThemeOverhaul.kt`) alongside `Violet` and `Moonflower` under neutral Liquid Glass (`LiquidGlass.kt`), `en`/`tr`/`ru` localization (`SonataStrings.kt`, `sonata-runtime/src/i18n.rs`), and YTM playlist editing + custom cover uploads (`EditPlaylistDialog.kt`). |
| **Cross-platform test coverage** | **179 Rust tests** (`sonata-core`: 92 unit + 16 integration, `sonata-runtime`: 13 unit + 10 integration, `sonata-desktop`: 25 unit, `sonata-tui`: 14 unit, `sonata-android`: 7 unit + 2 integration) and **43 Android Kotlin tests** (`41` JVM unit tests across 6 suites + `2` instrumented tests in `NativeEngineInstrumentedTest`). |

## 3. Current Open Maintenance & Refactoring Opportunities

1. **Split remaining oversized UI & orchestrator files**: `SonataViewModel.kt` (4,153 lines), `CategorizedSettingsSheet.kt` (2,826 lines), `NowPlayingScreen.kt` (2,427 lines), `ytmusic/parsers.rs` (2,222 lines), `sonata-runtime/src/player/session.rs` (2,134 lines), and `sonata-desktop/src/views/fullscreen.rs` (1,848 lines) would benefit from targeted sub-module decomposition.
2. **Align Android preference schema with bit-transparent audio**: [`SonataPreferences.kt`](../android/app/src/main/java/dev/sonata/android/data/SonataPreferences.kt) and [`CategorizedSettingsSheet.kt`](../android/app/src/main/java/dev/sonata/android/ui/screens/CategorizedSettingsSheet.kt) still retain legacy `eqEnabled` / `loudnessNormEnabled` settings entries even though [`PlaybackService.kt`](../android/app/src/main/java/dev/sonata/android/PlaybackService.kt) intentionally enforces a bit-transparent path (applying only `playbackSpeed` and `skipSilenceEnabled`).
3. **Expand Android Compose UI / instrumented coverage**: Build on `NativeEngineInstrumentedTest.kt` and the `41` JVM unit tests with focused Compose UI behavior tests for navigation, settings search, and player sheets.
