# Sonata Codebase Analysis

Snapshot of workspace structure, modular splits across Rust and Android, resolved fixes, and ongoing technical roadmap.

## 1. File-Size and Modularity Breakdown

The Rust workspace contains **37,989 lines** across 4 crates (`sonata-core`, `sonata-tui`, `sonata-desktop`, `sonata-android`), paired with a modular Kotlin/Jetpack Compose Android application under `android/`.

Large Rust and Android monoliths have been split into single-responsibility modules:

| Component | Split modules |
|---|---|
| `sonata-desktop` backend & views | `backend/{mod (1,419), playback, library, types}.rs`, `views/root/{stage, header, player_bar, sidebar, mod}.rs`, `views/fullscreen/{sheet, mod, controls}.rs`, `views/fullscreen_fx/{blur_cache, waves, palette, mod}.rs` |
| `sonata-tui` engine & UI | `app/{playback (1,112), mod (687), library (502), state (350)}.rs`, `ui/{mod, modals, now_playing}.rs` |
| `sonata-core` providers | `providers/ytmusic/{parsers (2,222), mod (1,636)}.rs`, `providers/spotify/{mod (711), embed}.rs`, `providers/hifi/{innertube (1,006), cipher (573), ytdlp, matching}.rs` |
| `sonata-android` bridge | `crates/sonata-android/src/lib.rs` (`1,331` lines with `#[cfg(test)]` UniFFI engine & cipher adapter coverage) |
| Android UI & Themes | Split `MainActivity.kt` and `SonataDesignSystem.kt` into dedicated screen/component files (`HomeScreen.kt`, `ExploreScreen.kt`, `LibraryScreen.kt`, `DetailScreen.kt`, `NowPlayingScreen.kt`, `SearchScreen.kt`, `SonataNavigationChrome.kt`, `SonataShareSheet.kt`, `SonataAnimatedArtworks.kt`, `SonataCards.kt`, `VioletTheme.kt`, `MoonflowerTheme.kt`, `LunariaPalette.kt`) |

Remaining large single files worth monitoring for further splitting:
- [`crates/sonata-core/src/providers/ytmusic/parsers.rs`](../crates/sonata-core/src/providers/ytmusic/parsers.rs) (2,222 lines) and [`crates/sonata-core/src/lyrics/providers.rs`](../crates/sonata-core/src/lyrics/providers.rs) (1,519 lines).
- [`crates/sonata-desktop/src/backend/mod.rs`](../crates/sonata-desktop/src/backend/mod.rs) (1,419 lines) and [`android/app/src/main/java/dev/sonata/android/SonataViewModel.kt`](../android/app/src/main/java/dev/sonata/android/SonataViewModel.kt).

## 2. Resolved Security, Reliability & Architecture Fixes

| Area | Fix verified in code |
|---|---|
| **Cookie permissions & redaction** | [`crates/sonata-core/src/auth/cookies.rs`](../crates/sonata-core/src/auth/cookies.rs): `0o600` file creation mode, `0o700` dir permissions, `TempDirGuard` cleanup; [`crates/sonata-android/src/lib.rs`](../crates/sonata-android/src/lib.rs) enforces `0o700`/`0o600` on private JSON caches; [`redaction.rs`](../crates/sonata-core/src/redaction.rs) scrubs headers/URLs. |
| **Android release & FOSS posture** | Removed F-Droid/Fastlane scaffolding, omitted `REQUEST_INSTALL_PACKAGES`, disabled `dependenciesInfo` in `build.gradle.kts`, enabled Gradle configuration/build cache on JDK 21. |
| **YTM playback telemetry & history** | Added `YtMusicClient::report_playback` + `normalize_playback_playlist_id` across TUI, Desktop, and Android, JioSaavn-to-YTM ID matching, `ListeningHistoryScreen`, and live *Library → Artists* promotion. |
| **Android auth switching & share toast** | Fixed async cookie/`WebStorage` purge + `visitedAuthFlow` gate in `YtMusicLoginSheet.kt` and auto-clearing `addToPlaylistStatus` to eliminate sticky save banners. |
| **Theme separation & detail layout** | Separated `VioletTheme.kt` and `MoonflowerTheme.kt` with pinkish `GlassSelectionPill`, blurred `ArtistHeroHeader`, and non-overlapping `SonataSplitHeroHeader`. |
| **Cross-platform test coverage** | Expanded tests to **118 Rust unit tests** (`sonata-core`: 88, `sonata-desktop`: 20, `sonata-tui`: 5, `sonata-android`: 5) and **19 Android Kotlin/JVM unit tests** (`android/app/src/test/java/dev/sonata/android/`). |

## 3. Remaining Open Improvements

1. **Shared `PlaybackSession` across TUI & Desktop**: `sonata-tui/src/app/playback.rs` and `sonata-desktop/src/backend/playback.rs` still mirror `mpv` event state machines (`play_seq`, Hi-Fi upgrade swaps, lookahead precache adoption). Consolidating into `sonata-core::player` would reduce duplication.
2. **40 ms `DesktopSnapshot` cloning & sync `config.save()`**: Splitting `PlaybackProgress` into a separate `watch` channel in `sonata-desktop/src/backend/mod.rs` would avoid cloning shelf vectors on every tick.
3. **Typed core error enum**: Replacing coarse `anyhow::Error` boundaries in `sonata-core` with typed error variants (auth expired, HTTP 429 rate-limited, offline) would let frontends react uniformly without string matching.
