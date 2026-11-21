# Sonata Android Architecture & Research

> [!NOTE]
> **Status: Coming Soon** — `crates/sonata-android` is currently a reserved workspace placeholder crate. This document captures the verified architectural design and platform research for building the native Android client.

**Target Stack:** **Kotlin (Jetpack Compose)** + **AndroidX Media3** + **Jetpack Glance** + **UniFFI (`uniffi`)** + **`cargo-ndk`** over **`sonata-core`**.

---

## 1. Architecture & UniFFI Boundary

### Crate Layout & Roles

- **`crates/sonata-core`**: Remains pure Rust with zero FFI, JNI, or Android-specific UI dependencies. Desktop-only process helpers (`mpv`, `ytdlp`, `hyprland`, `ffmpeg`-based `motion`) are gated behind `#[cfg(not(target_os = "android"))]` where needed.
- **`crates/sonata-android`**: Converted into the thin UniFFI facade crate (`crate-type = ["cdylib", "staticlib"]`) using `uniffi` proc-macros (no separate `.udl` file required).

### Tokio Runtime & State Ownership

- A single `#[derive(uniffi::Object)] pub struct SonataEngine` owns one shared `Arc<tokio::runtime::Runtime>` (`Builder::new_multi_thread().enable_all().worker_threads(4).build()`).
- Async FFI methods are exported with `#[uniffi::export(async_runtime = "tokio")]`, which UniFFI generates as native Kotlin `suspend fun`s (invoked on `Dispatchers.IO`).
- Synchronous SQLite (`CacheDb`) and image operations are offloaded via `tokio::task::spawn_blocking` so Tokio worker threads never stall.

### Data Model & Typed Errors

- Core models (`Track`, `Playlist`, `HomeShelf`, `HomeCard`, `LyricLine`, `LyricWord`, `LyricVoice`, `ResolvedStream`, `AppConfig`) are mirrored as `#[derive(uniffi::Record)]` and `#[derive(uniffi::Enum)]` value types, crossed once per load rather than per frame.
- Errors use a typed `#[derive(Debug, thiserror::Error, uniffi::Error)] pub enum SonataError` (`AuthExpired`, `Network`, `Playback`, `Cache`) so Kotlin can handle failures with idiomatic `try/catch`.

### Event Streaming (`SonataEventListener` → `callbackFlow`)

- Background updates (progressive playlist pages, lyrics ready, Hi-Fi stream upgrade ready, auth state changes) are broadcast from a `tokio::sync::broadcast` channel to a `#[uniffi::export(callback_interface)] pub trait SonataEventListener: Send + Sync { fn on_event(&self, event: SonataEvent); }`.
- On the Kotlin side, `callbackFlow { trySend(event); awaitClose { engine.unsubscribe(id) } }` bridges Rust events into a lifecycle-aware `StateFlow` in the `ViewModel`.

### Android Paths & WebView Authentication

- **`filesDir` & `cacheDir` Injection**: `AppConfig::config_dir()` and `AppConfig::cache_dir()` currently rely on `dirs::config_dir()` / `dirs::cache_dir()`, which return `None` on Android. `SonataEngine::new(config_dir: String, cache_dir: String)` must inject Android's `context.filesDir` and `context.cacheDir`.
- **In-App WebView Cookie Injection**: Desktop browser cookie scraping (`auth/cookies.rs`) and `yt-dlp` are unavailable on Android. Instead, Android signs in via an in-app `WebView`, reads `ytm_cookies` and Spotify `sp_dc` from `android.webkit.CookieManager`, and passes them into `SonataEngine::set_auth_cookies(...)`.

### `cargo-ndk`, Bundled SQLite, & Binary Size Tuning

- **Targets & NDK Alignment**:
  - Debug/local dev: `cargo ndk -t arm64-v8a -t x86_64` for fast iteration.
  - Release AAB: `arm64-v8a` + `armeabi-v7a` with a pinned modern NDK (`ndkVersion`, NDK r27/r28+) to guarantee **16 KB ELF page alignment** required on Android 15+.
  - Automate `uniffi-bindgen generate --library ... --language kotlin` in a Gradle `preBuild` task with a CI drift check.
- **Bundled SQLite (`rusqlite`)**: `Cargo.toml` uses `rusqlite = { version = "0.41", features = ["bundled"] }`, which cross-compiles SQLite C cleanly via NDK `clang` (`cargo-ndk` configures `CC_<target>` automatically) with no system `libsqlite3.so` version mismatch. Pass `CFLAGS="-Oz -flto -ffunction-sections -fdata-sections"` to shrink SQLite C code.
- **Size Profile**: Use a dedicated release profile for Android (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`) plus `llvm-strip --strip-unneeded` on the output `.so` files.
- **Zero-JNI Render Loop**: Never cross JNI inside 60/120fps UI ticks. Kotlin owns playback position polling (`exoPlayer.currentPosition`) and active lyric line lookup against the `List<LyricLine>` already in Kotlin memory.

---

## 2. Media3 `ExoPlayer` Audio & `MotionCover` Handoff

### Replacing `mpv` with Media3 `ExoPlayer`

- `StreamResolver` (`sonata-core::providers::hifi`) resolves `ResolvedStream { url, badge, headers: Vec<(String, String)> }` via pure-Rust InnerTube (`innertube.rs`) and `CacheDb` (with `ytdlp.rs` disabled on Android).
- Kotlin calls `engine.resolveFast(track)` (`suspend fun`):
  - **Remote URLs**: Build a Media3 `DefaultHttpDataSource.Factory().setDefaultRequestProperties(stream.headers.toMap())` and load `MediaItem.fromUri(stream.url)` into `ExoPlayer`.
  - **Cached Local Files** (`!url.starts_with("http")`): Media3 reads directly via `DefaultDataSource` with zero HTTP overhead.

### Seamless Hi-Fi Upgrade Swap

- `resolve_hifi_upgrade` runs asynchronously in Rust and emits `SonataEvent::HifiUpgradeReady { track_id, stream }`.
- When `track_id` matches the active `MediaItem`, Kotlin captures `exoPlayer.currentPosition`, prepares the higher-bitrate `MediaItem` at that exact seek offset (`setMediaItem(newItem, currentPosMs)`), and resumes seamlessly.

### Animated `MotionCover` via Media3 `PlayerSurface`

- Instead of invoking `ffmpeg` (`motion.rs`), Rust exposes the resolved Apple Music `.m3u8` / `.mp4` motion cover URL directly to Kotlin.
- Kotlin plays the loop in a single pooled, muted `ExoPlayer` (`repeatMode = Player.REPEAT_MODE_ONE`, `volume = 0f`) bound to the currently visible player/fullscreen surface:
  - **`SURFACE_TYPE_SURFACE_VIEW`**: Default for flat fullscreen and card backgrounds (lowest power, direct Hardware Composer path).
  - **`SURFACE_TYPE_TEXTURE_VIEW` (`AndroidExternalSurface`)**: Used only during Compose clipping, alpha fades, or shared-element mini-player ↔ fullscreen morphs.
  - Pair with `rememberPresentationState(player)` so static Coil 3 artwork stays visible until the first video frame renders (zero black flash), and call `clearVideoSurface()` / pause when off-screen or backgrounded.

---

## 3. Jetpack Compose UI, 3-Skin Token System, Dynamic Palette, & Fullscreen Lyrics

### 3-Skin Token Architecture (`CompositionLocal`)

Port `DesktopSkin` (`Sonata`, `Spotify`, `YouTubeMusic`) and `GlassPalette` into an `@Immutable data class SonataThemeTokens` provided via `LocalSonataTokens` (wrapping `MaterialTheme.colorScheme` for system sheets and dialogs):

- **Sonata Glass**: Translucent surface alphas (`0.68` / `0.48` / `0.28` / `0.78`), ambient top tint (`ambient_rgb` mixed at 58%), and `1.dp` hairline borders (`white.copy(alpha = 0.08f)`). On Android 12+ (`API 31+`), apply `RenderEffect.createBlurEffect` / `Modifier.blur(...)` selectively on floating bars and sheets, falling back to higher-alpha scrims on `API < 31`.
- **Spotify**: `#000000` shell, `#121212` rounded panels, `#181818` cards, `#1ED760` accent, inverted circular Play CTA, `0.42` ambient surface mix.
- **YouTube Music**: `#030303` canvas, `#212121` bottom bar with `#FF0033` top scrubber, `#161616` cards, `0.12` hairline dividers, high-contrast Play/CTA buttons.

### Dynamic Album-Art Palette

- Sample a downscaled `64x64` cover bitmap off the main thread (`Dispatchers.Default`) using `androidx.palette:palette-ktx` (or Sonata's 24-bin circular hue clustering) to extract leading and runner-up hues, feeding `animateColorAsState` (`800ms` color-wash) into `ambientTop`/`ambientGlow` and optional Material 3 `dynamicColorScheme` blending.

### AGSL `RuntimeShader` Waves & Hardware Blur (Android 13+ / API 33+)

- Replace CPU-rendered ambient waves with a GPU `RuntimeShader` (`AGSL`) drawn in `Modifier.drawWithCache` / `graphicsLayer` via `ShaderBrush`. Drive `iTime` with `withInfiniteAnimationFrameNanos` and update uniforms inside the draw phase (zero Compose recompositions per frame).
- Avoid multi-sample blur loops inside AGSL at 120Hz; pair Android's hardware `RenderEffect.createBlurEffect(sigma, sigma, TileMode.CLAMP)` for the 3 fixed depth-of-field lyric blur buckets with a lightweight 5-color wave AGSL shader on `API 33+`, and a static radial-gradient + alpha fallback on `API < 33`.

### Fullscreen Synced Karaoke Lyrics

- Model `SheetEntry` (`Verse` and `Instrumental` `♪ ♪ ♪` breaks for `>= 5s` gaps) as `@Immutable` items in a `LazyColumn` with stable `key` and `contentType`.
- Center the active verse at 50% viewport height via `LazyListState.animateScrollToItem` + `scrollOffset`, using `snapshotFlow { listState.isScrollInProgress }` to arm a `1220ms` settle timer before re-centering and re-blurring.
- Render word-by-word karaoke sweeps with `drawWithContent` + `clipRect` / `ShaderBrush` gradient masks driven in the draw phase (no per-frame text relayout), and right-align `LyricVoice::Counter` duet lines.

---

## 4. 120fps / 60fps Adaptive Refresh Rate (ARR) & Runtime Performance

- **Android 15+ ARR (`FrameRateCategory` & `Modifier.preferredFrameRate`)**: Apply `FrameRateCategory.High` (`120f`) **only** on active high-velocity surfaces (seekbar drag-scrubbing, lyric sheet auto-scroll, visible AGSL wave backdrop, shared-element player expansion) and `FrameRateCategory.Low` / `Normal` on idle controls so ARR devices drop refresh rate cleanly instead of locking at 120Hz.
- **Compose Compiler & State Hygiene**: Use Kotlin 2.0.20+ Strong Skipping Mode, mark all UI/snapshot models (`HomeShelf`, `HomeCard`, `LyricLine`, `SonataThemeTokens`) `@Immutable`, separate high-frequency `playbackPositionMs` `StateFlow` from structural `PlayerUiState`, and provide stable `key` + `contentType` on every `LazyColumn` / `LazyRow`.
- **Coil 3 Art Caching**: Configure a singleton `ImageLoader` with bounded memory cache (~20–25% app heap), disk cache, `crossfade(true)`, and exact `SizeResolver` targets so shelf thumbnails decode at display size rather than `1200x1200`.
- **Baseline Profiles + R8**: Include a `:baselineprofile` Macrobenchmark module covering cold start, Home shelf flings, and Fullscreen player + lyrics open, compiled with R8 full mode.

---

## 5. AOD (Always-On Display) & Lockscreen Widgets

- **System AOD & Lockscreen `MediaSession`**: Stock Android does not allow third-party apps to draw custom UI on hardware/system AOD; SystemUI renders lockscreen and AOD media controls from the active Media3 `MediaSession`. Android 15/16 Live Updates (`POST_PROMOTED_NOTIFICATIONS` / `ProgressStyle`) explicitly exclude media playback—continue using `MediaStyle` / `MediaSessionService`.
- **Lockscreen Widgets**: Supported on Android 15 QPR1+ tablets and Android 16+ by declaring `android:widgetCategory="home_screen|keyguard"` in `<appwidget-provider>`.
- **Opt-In In-App Nightstand / Dock AOD Mode (AMOLED-Safe)**:
  - Full-screen mode using `FLAG_KEEP_SCREEN_ON` (`view.keepScreenOn = true`) and low screen brightness (`0.02f..0.08f`) over a `#000000` AMOLED background (`< 5%` lit pixels).
  - **Burn-in protection**: Periodic `offset(x, y)` micro-shift (`±4..12 dp` every 30–60s) on the clock, track title, and active lyric line.
  - **Power throttling**: Disable animated waves and `MotionCover` loops, update only on track/lyric changes (or 1Hz clock tick), and request `FrameRateCategory.LOW` on API 35+.

---

## 6. Music Widgets (`Jetpack Glance`)

- **Responsive Layouts (`SizeMode.Responsive`)**: Provide `2x1` (`110x56 dp`: title + play/pause + next), `4x1` (`250x56 dp`: thumbnail + title/artist + prev/play-pause/next), and `4x2` (`250x120 dp`: cover + title/artist + like + transport + active lyric/progress snapshot).
- **Interactions**: Wire transport and like actions via `clickable(actionRunCallback<TransportActionCallback>(...))` forwarding lightweight commands to `MediaSessionService`.
- **`RemoteViews` Bitmap Limits**: Android enforces a strict IPC cap (`6 * screen_width * screen_height` bytes total across `RemoteViews`). Prefer `FileProvider` `content://` `Uri`s where possible; when using `ImageProvider(bitmap)`, downsample to widget bounds (`<= 200x200` px) with `Bitmap.Config.RGB_565` and hoist decoding into `provideGlance` outside `provideContent`.
- **Battery-Safe Updates**: Never poll via `updatePeriodMillis` or 1-second progress ticks. Trigger `MusicWidget().updateAll(context)` strictly on state transitions (`onMediaItemTransition`, `onIsPlayingChanged`, `onRepeatModeChanged`, like toggle) backed by a lightweight `DataStore` snapshot.

---

## 7. Music Notifications, Bluetooth AVRCP, & Android Auto

- **Foreground `MediaSessionService` + `DefaultMediaNotificationProvider`**: On Android 13+ (`API 33+`), SystemUI automatically renders the native media card with artwork, squiggly animated seekbar, and transport controls from the `MediaSession`.
- **Custom `CommandButton`s Without Breaking Android Auto**: Register custom `SessionCommand`s (e.g., `ACTION_LIKE`) in `MediaSession.Callback.onConnect()` and push via `setCustomLayout(...)` + `onCustomCommand()`. Do **not** strip `Player.Commands` globally; subclass `DefaultMediaNotificationProvider` (`getMediaButtons` / `addNotificationActions`) to arrange notification slots independently of Android Auto.
- **Bluetooth AVRCP**: Populate `MediaMetadata.Builder()` (`title`, `artist`, `albumTitle`, `artworkUri`, `durationMs`) on every `MediaItem`; standard `Player.COMMAND_*` actions map automatically to AVRCP.
- **Android Auto (`MediaLibraryService`)**: Upgrade `MediaSessionService` to `MediaLibraryService` and implement `MediaLibrarySession.Callback` (`onGetLibraryRoot`, `onGetChildren`, `onSearch`, `onGetSearchResult`) backed by `sonata-core` (Home, Liked Music, Playlists, Search).

---

## 8. Concrete Next-Step Checklist & `sonata-core` Prep Gaps

1. **Inject Storage Paths in `sonata-core::config`**: Add `AppConfig::init_paths(config_dir, cache_dir)` (or pass explicit paths into `CacheDb::open_at` and `telemetry::init`) so Android does not fall back to non-writable `~/.config` / `~/.cache`.
2. **Gate Desktop-Only Subprocesses**: Gate `mpv`, `ytdlp`, `hyprland`, and `ffmpeg`-piped `motion` behind `#[cfg(not(target_os = "android"))]` in `sonata-core`, and expose the resolved Apple Music motion `.m3u8`/`.mp4` URL for Media3 playback.
3. **Add Android Auth Injection**: Add a clean method on `YtMusicClient` / `SpotifyClient` / `StreamResolver` to accept `WebView`-extracted cookies (`ytm_cookies` + `sp_dc`) without invoking desktop browser SQLite/`yt-dlp` scraping.
4. **Scaffold `crates/sonata-android` UniFFI Facade**: Configure `cdylib` + `staticlib`, add `uniffi` + `thiserror`, implement `SonataEngine` with a shared Tokio runtime, `SonataEventListener` broadcast bridge, and `cargo-ndk` build scripts.
5. **Scaffold Android Gradle Project**: Wire `MediaSessionService` / `MediaLibraryService` + `ExoPlayer`, Compose 3-skin tokens + AGSL waves + ARR `LazyColumn` lyrics, Coil 3, `GlanceAppWidget`, opt-in AMOLED Nightstand AOD, and `:baselineprofile`.

---

## 9. Kotlin & Jetpack Compose Component Ecosystem ("shadcn for Compose")

Sonata's Android UI supports three distinct visual skins (**Sonata Glass**, **Spotify**, and **YouTube Music**) with zero Material visual lock-in on custom player surfaces. Rather than fighting `androidx.compose.material3` defaults or reinventing focus/accessibility plumbing, Sonata adopts a layered **headless primitives + expressive motion + GPU glass** stack.

> [!IMPORTANT]
> **Standards Compliance (English-Only & Zero External References)**: All UI copy, resource strings, logs, package/symbol names, code comments, and documentation in Sonata MUST remain strictly in **English**. Never reference, port comments from, or expose external third-party music app names or branding inside Sonata's source files, UI tokens, or documentation.

### 1. Headless & Copy-Paste Component Kits (shadcn / Radix Equivalents)

- **Compose Unstyled (`com.composables:core` / `com.composables:composeunstyled-*`)**:
  - **Artifacts**: `com.composables:core:1.49.9` (1.x monolithic package) or the modular 2.x artifacts (`com.composables:composeunstyled-theming`, `composeunstyled-button`, `composeunstyled-text-field`, etc., latest **`2.10.2`** on Maven Central).
  - **Role in Sonata**: Provides renderless, WCAG/ARIA-accessible primitives (`ModalBottomSheet` / flexible drawers, `Dialog`, `DropdownMenu` / `Menu`, `TabGroup` / `Tabs`, `TextField`, `Tooltip` with `Portal` anchoring) that manage gestures, focus trapping, outside-click dismissal, and keyboard navigation while emitting **zero default visual chrome**.
  - **Why it fits Sonata**: Sonata wraps these unstyled primitives once in `LocalSonataTokens`-aware components (`SonataSheet`, `SonataDialog`, `SonataContextMenu`, `SonataSearchField`) so the exact same accessible behavior renders as frosted refractive glass in **Sonata Glass**, `#121212` flat cards in **Spotify**, or `#212121` sharp-hairline panels in **YouTube Music**.
- **shadcn-Inspired Compose Kits & Copy-Paste Pattern**:
  - **`io.github.ronjunevaldoz:shadcn-compose:0.2.9`**: A pre-styled Compose Multiplatform port of shadcn/ui useful when rapid desktop/mobile admin or settings forms are desired.
  - **Recommended Sonata Pattern (Copy-Paste Own-Your-Components)**: Follow the core shadcn/Radix philosophy—keep **Compose Unstyled** for behavioral primitives and own thin, copy-pastable styling wrappers inside `dev.sonata.android.ui.components` driven by `CompositionLocal` theme tokens (`SonataThemeTokens`) instead of adopting an opinionated external design library.

### 2. Material 3 Expressive (`material3` 1.4+) & `graphics-shapes`

`android/app/build.gradle.kts` currently pins `androidx.compose:compose-bom:2025.04.01`. Bumping to a newer BOM (`2026.11.00`) or explicitly targeting **`androidx.compose.material3:material3`** **`1.4.0` / `1.5.0`** (with `1.5.0` promoting the full Expressive suite to stable) unlocks:

- **Physics-Based Motion (`MotionScheme.expressive()`)**: Replaces ad-hoc `tween(...)` durations with unified spatial/effects spring schemes (`MaterialTheme(motionScheme = MotionScheme.expressive())`), ideal for mini-player expansion, tab pill snaps, and queue item settlement.
- **Morphing Polygons (`androidx.graphics:graphics-shapes:1.1.0`)**: Build `RoundedPolygon` primitives (play/pause polygons, star/pill badges, expressive equalizer blobs) and animate smoothly between them via `Morph(startPolygon, endPolygon).toPath(progress).asComposePath()`.
- **`HorizontalFloatingToolbar` / `VerticalFloatingToolbar`**: Built-in floating action capsules with `FloatingToolbarScrollBehavior` and `FloatingToolbarDefaults` that complement Sonata's floating navigation and contextual multi-select playlist bars.
- **`LinearWavyProgressIndicator`**: Expressive determinate/indeterminate wavy progress bars that pair naturally with playing-track progress indicators and squiggly audio scrubbers.

### 3. Liquid Glass, Continuous Curves & Backdrop Blur

- **`Kyant0/backdrop` (`io.github.kyant0:backdrop:2.0.1`)**:
  - **Architecture**: Uses Android 12+ `RenderNode` / `RenderEffect` for offscreen backdrop capture and blur, paired with Android 13+ AGSL `RuntimeShader` for refractive `lens` displacement, `colorControls`, directional specular `Highlight`, and `Shadow`.
  - **Sonata Integration**: Sonata already maintains a vendored source copy of v2.0.0 under `dev.sonata.android.backdrop` (adding `GLASS_RESOLUTION_SCALE = 0.33f` / `FROST_RESOLUTION_SCALE = 0.25f` and unified Android `Platform.kt` SDK checks) wired through `dev.sonata.android.LiquidGlass` (`LocalAppBackdrop`, `Modifier.liquidGlass`, `Modifier.lightweightLiquidGlass`). Keeping it vendored avoids Compose binary ABI mismatches across BOM bumps while allowing custom resolution scaling.
- **`Kyant0/Capsule` (`io.github.kyant0:capsule:2.1.3`)**:
  - Provides **G2 continuous squircle** shapes (`ContinuousRoundedRectangle`) with smoother curvature continuity than Compose's standard `RoundedCornerShape`, matching modern glass/pill geometry on album cards, floating mini-player bars, and selection pills.
- **Chris Banes Haze (`dev.chrisbanes.haze`)**:
  - **Artifacts**: `android/app/build.gradle.kts` currently declares `dev.chrisbanes.haze:haze:1.3.1` + `haze-materials:1.3.1` (latest 1.x is `1.6.10`; Haze 2.x uses `dev.chrisbanes.haze:haze:2.1.0` + `haze-blur-materials:2.1.0`).
  - **Role in Sonata**: Best suited for general progressive-blur top/bottom bars, sticky headers, and legacy/lightweight frosted surfaces where refractive `lens` distortion is unnecessary or disabled via `LocalGlassPerformance`.

### 4. Music Player Specialized UI Packages

- **`sh.calvin.reorderable:reorderable:3.1.0`**: Smooth 120fps drag-and-drop reordering (`ReorderableItem`, `rememberReorderableLazyListState`) for the **Up Next** playback queue and custom playlist tracks, with haptic feedback hooks and zero item-key flicker.
- **Custom Waveform / Squiggly Scrubbers**: Implement in Compose `Canvas` / `Modifier.drawWithCache` using a phase-driven sine wave path when `isPlaying == true` (collapsing to a flat line via `animateFloatAsState` when paused) so scrubbing runs entirely in the draw phase with zero recompositions.
- **Artwork Color Extraction & HCT Tinting**:
  - `androidx.palette:palette-ktx:1.0.0` (currently in `build.gradle.kts`; merged into `androidx.palette:palette:1.1.0`) for fast dominant/vibrant swatch extraction from downscaled `64x64` artwork bitmaps.
  - **`com.materialkolor:material-color-utilities`** / **`com.materialkolor:material-kolor`** for Google's perceptual **HCT / CAM16** tonal palettes when blending dynamic album art into Sonata's `ambientTop` / `ambientGlow` without muddy mid-tones.
- **Coil 3 (`io.coil-kt.coil3:coil-compose:3.6.3` + `coil-network-okhttp:3.6.3`)**: Upgrade target from `3.1.0` currently in `build.gradle.kts`; configure `SizeResolver` per shelf card size and shared `ImageLoader` memory/disk limits.

### Ecosystem Comparison Matrix

| Library | Maven Coordinate (Verified Latest) | Role in Sonata | API Requirement | Cost / Overhead |
| :--- | :--- | :--- | :--- | :--- |
| **Compose Unstyled** | `com.composables:core:1.49.9` / `composeunstyled-*:2.10.2` | Headless accessible `Dialog`, `ModalBottomSheet`, `Menu`, `Tabs`, `TextField`, `Tooltip` for 3 skins | API 21+ | Very low; zero visual bloat, modular 2.x artifacts |
| **shadcn-compose** | `io.github.ronjunevaldoz:shadcn-compose:0.2.9` | Optional pre-styled shadcn/ui components for rapid utility/settings screens | API 24+ | Low–Medium; opinionated styles if used outside settings |
| **Material 3 Expressive** | `androidx.compose.material3:material3:1.5.0` (BOM `2026.11.00`) | `MotionScheme.expressive()`, `HorizontalFloatingToolbar`, `LinearWavyProgressIndicator` | API 21+ | Low; already in Compose BOM |
| **Graphics Shapes** | `androidx.graphics:graphics-shapes:1.1.0` | `RoundedPolygon` & `Morph` animated play/pause, badge, and equalizer shapes | API 21+ | Minimal; small runtime footprint |
| **Kyant0 Backdrop** | `io.github.kyant0:backdrop:2.0.1` (vendored in `dev.sonata.android.backdrop`) | Refractive Liquid Glass (`lens`, `Highlight`, `blur`, `Shadow`) on tab bar & mini-player | API 31+ (`RenderEffect`), API 33+ (AGSL `lens`) | Medium–High GPU pass; mitigated via `0.33f` resolution scale |
| **Kyant0 Capsule** | `io.github.kyant0:capsule:2.1.3` | G2 continuous squircles (`ContinuousRoundedRectangle`) for cards, pills & mini-player | API 21+ | Negligible; pure geometry/path calculation |
| **Haze** | `dev.chrisbanes.haze:haze:1.6.10` / `2.1.0` + `haze-blur-materials:2.1.0` | Progressive frosted blur on sticky headers, sheets & performance fallback mode | API 31+ (`RenderEffect`), scrim fallback `< 31` | Low–Medium; cheaper than refractive glass |
| **Reorderable** | `sh.calvin.reorderable:reorderable:3.1.0` | 120fps drag-and-drop Up Next queue & playlist track reordering | API 21+ | Negligible; state-driven inside `LazyColumn` |
| **Palette / Material Kolor** | `androidx.palette:palette:1.1.0` / `com.materialkolor:material-color-utilities` | Off-thread `64x64` cover swatch & perceptual HCT tonal palette extraction | API 21+ | Negligible when run on `Dispatchers.Default` |
| **Coil 3** | `io.coil-kt.coil3:coil-compose:3.6.3` + `coil-network-okhttp:3.6.3` | Async artwork decoding, disk/RAM LRU cache, exact `SizeResolver` downscaling | API 21+ | Low; bounded heap cache (~20–25%) |

---

## 10. Base Android SDK Strategy (`minSdk` & Progressive Feature Tiers)

Currently, `android/app/build.gradle.kts` configures `minSdk = 26`, `targetSdk = 35`, `compileSdk = 35`, `ndkVersion = "27.2.12479018"`, `abiFilters += listOf("arm64-v8a")`, and `packaging.jniLibs.useLegacyPackaging = false`. Defining the long-term `minSdk` floor and progressive runtime gating is essential to deliver flagship visuals without regressing older hardware.

### 1. Comparing Candidate `minSdk` Baselines (`26` vs `31` vs `33`)

| Candidate `minSdk` | Android Version | Key Advantages | Trade-offs & Maintenance Cost | Verdict |
| :--- | :--- | :--- | :--- | :--- |
| **`minSdk = 26`** *(Current)* | Android 8.0 Oreo (2017) | Broadest legacy device compatibility; baseline notification channels & `AudioAttributes` | Forces maintaining translucent alpha scrims (`lightweightLiquidGlass`) for API 26–30, misses guaranteed hardware `RenderEffect` blur, lacks native `SplashScreen` & Android 12+ foreground-service media guarantees, widens QA surface | Keep **only** if Android 8–11 support is an explicit requirement |
| **`minSdk = 31`** *(Recommended)* | Android 12 (2021) | Guarantees hardware `RenderEffect.createBlurEffect` + `RenderNode` glass on **100%** of supported devices, Material You dynamic color, modern `SplashScreen`, foreground-service stability fixes; covers ~5-year-old devices cleanly | Drops Android 8–11 devices (minor share for modern Hi-Fi / Compose audio audience) | **Recommended Production Floor** |
| **`minSdk = 33`** | Android 13 (2022) | Guarantees AGSL `RuntimeShader` (refractive lens + animated waves), `POST_NOTIFICATIONS`, and native squiggly `MediaStyle` card on every install | Unnecessarily excludes Android 12 / 12L devices that already run hardware `RenderEffect` blur well | **Too restrictive** as `minSdk`; gate API 33+ features at runtime instead |

> [!TIP]
> **Recommendation**: Raise production `minSdk` to **`31` (Android 12)** while targeting **`targetSdk = 35` / `compileSdk = 35+`** (advancing to `36` as Google Play deadlines progress). If `minSdk = 26` is temporarily retained during early testing, ensure every `RenderEffect`, `RuntimeShader`, and `preferredFrameRate` call sits behind the capability tier resolver below.

### 2. Feature-by-API Matrix (`API 31` → `API 37`)

| API Level | Android Release | Platform Capabilities Unlocked for Sonata |
| :--- | :--- | :--- |
| **API 31** | **Android 12** | Hardware `RenderEffect.createBlurEffect` + `RenderNode` backdrop blur (`dev.sonata.android.backdrop`), Material You dynamic color (`dynamicDarkColorScheme` / `dynamicLightColorScheme`), unified `SplashScreen` API (`androidx.core:core-splashscreen`), `PendingIntent.FLAG_IMMUTABLE` |
| **API 33** | **Android 13** | Programmable **AGSL `RuntimeShader`** (`Build.VERSION_CODES.TIRAMISU`) for Liquid Glass refractive `lens` dispersion and 5-color ambient wave backdrops; native Media3 `MediaStyle` squiggly seekbar in SystemUI; `POST_NOTIFICATIONS` runtime permission; per-app language picker (`LocaleManager`) |
| **API 34** | **Android 14** | **Predictive Back** animations (`androidx.activity.compose.PredictiveBackHandler`) for fluid Fullscreen Player ↔ Mini-Player collapse and sheet dismissal; **USB lossless DAC / bit-perfect audio routing** (`AudioManager.getSupportedMixerAttributes` / `setPreferredMixerAttributes` with `MIXER_BEHAVIOR_BIT_PERFECT`); `FOREGROUND_SERVICE_MEDIA_PLAYBACK` enforcement |
| **API 35–37** | **Android 15–17** | **120Hz / 60Hz Adaptive Refresh Rate (ARR)** via `androidx.compose.ui.FrameRateCategory` (`High`, `Normal`, `Low`) and `Modifier.preferredFrameRate(...)` (Compose UI 1.9+); **mandatory 16 KB ELF page size alignment** (`ndkVersion = "27.2.12479018"`+, `useLegacyPackaging = false`, `cargo-ndk` 16 KB linker alignment for `libsonata_android.so` & bundled SQLite); **lockscreen widgets** (`android:widgetCategory="home_screen|keyguard"` on Android 15 QPR1+ tablets & Android 16+) |

### 3. Runtime Feature Gating Pattern (`VisualCapabilityTier`)

Rather than scattering `Build.VERSION.SDK_INT` checks across composables, resolve a single immutable `VisualCapabilityTier` at startup (paired with user accessibility/performance overrides like `LocalGlassPerformance` and `LocalReduceDynamicBlur`) and provide it via `CompositionLocal`:

```kotlin
package dev.sonata.android

import android.os.Build
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.FrameRateCategory
import androidx.compose.ui.Modifier
import androidx.compose.ui.preferredFrameRate

/**
 * Progressive rendering tiers balancing visual fidelity with GPU/battery budget.
 * Note: HARDWARE_BLUR_31..FLAGSHIP_ARR_35 form the 3 active tiers under minSdk = 31;
 * LEGACY_SCRIM remains available whenever minSdk < 31 is supported.
 */
@Immutable
enum class VisualCapabilityTier {
    LEGACY_SCRIM,         // API 26-30: High-alpha tinted scrims, static gradient backdrop
    HARDWARE_BLUR_31,     // API 31-32: RenderEffect.createBlurEffect + RenderNode frosted glass
    AGSL_LIQUID_GLASS_33, // API 33-34: Hardware blur + AGSL RuntimeShader lens & ambient waves
    FLAGSHIP_ARR_35;      // API 35+:   Full AGSL Liquid Glass + Adaptive Refresh Rate voting

    val supportsHardwareBlur: Boolean
        get() = ordinal >= HARDWARE_BLUR_31.ordinal

    val supportsAgslShaders: Boolean
        get() = ordinal >= AGSL_LIQUID_GLASS_33.ordinal

    val supportsAdaptiveRefreshRate: Boolean
        get() = ordinal >= FLAGSHIP_ARR_35.ordinal

    companion object {
        fun current(sdkInt: Int = Build.VERSION.SDK_INT): VisualCapabilityTier = when {
            sdkInt >= Build.VERSION_CODES.VANILLA_ICE_CREAM -> FLAGSHIP_ARR_35
            sdkInt >= Build.VERSION_CODES.TIRAMISU -> AGSL_LIQUID_GLASS_33
            sdkInt >= Build.VERSION_CODES.S -> HARDWARE_BLUR_31
            else -> LEGACY_SCRIM
        }
    }
}

val LocalVisualCapabilityTier = staticCompositionLocalOf { VisualCapabilityTier.current() }

/**
 * Gates Adaptive Refresh Rate voting to API 35+ while remaining a no-op on older platforms.
 */
@Composable
fun Modifier.sonataFrameRate(highVelocity: Boolean): Modifier {
    val tier = LocalVisualCapabilityTier.current
    if (!tier.supportsAdaptiveRefreshRate) return this
    return preferredFrameRate(
        if (highVelocity) FrameRateCategory.High else FrameRateCategory.Normal,
    )
}
```

#### Graceful Degradation in `MainActivity` & `LiquidGlass.kt`

1. **`LEGACY_SCRIM` (`API < 31`)**: `Modifier.liquidGlass` routes directly to `Modifier.lightweightLiquidGlass` (translucent surface fill + `GLASS_EDGE_WIDTH` hairline), and the fullscreen ambient backdrop falls back to a static radial gradient wash.
2. **`HARDWARE_BLUR_31` (`API 31–32`)**: Enables `RenderNode` backdrop capture and `RenderEffect.createBlurEffect` at `FROST_RESOLUTION_SCALE` (`0.25f`), omitting AGSL refractive `lens` distortion and animated shader waves.
3. **`AGSL_LIQUID_GLASS_33` (`API 33–34`)**: Unlocks full `dev.sonata.android.backdrop` refractive `lens` + specular `Highlight` + `0.33f` resolution scaling + AGSL `RuntimeShader` animated ambient waves, alongside `PredictiveBackHandler` and USB bit-perfect mixer detection on API 34.
4. **`FLAGSHIP_ARR_35` (`API 35+`)**: Adds `Modifier.sonataFrameRate(highVelocity = true)` strictly to active surfaces (seekbar scrub, lyric auto-scroll, visible AGSL waves, player expansion) so static screens and AMOLED Nightstand AOD drop cleanly to low refresh rates, backed by 16 KB-aligned native `.so` binaries.
