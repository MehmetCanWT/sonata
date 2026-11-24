# Changelog

All notable changes to **Sonata** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.7] - 2026-11-24

### Added

- **Desktop Feature Parity: Find Your Mood on Home, Playlist Pinning, Add-to-Library (`+`), Live Refresh & Proactive Personalized Autoplay (`sonata-runtime`, `sonata-desktop`)**:
  - Added **Recently Played**, **Pinned**, and **Find Your Mood** shelves directly on Desktop Home, localized mood chips and tiles (`localize_mood_title`), and rendered both track and card shelves inside `MoodCategory` pages.
  - Enforced playlist ownership (`is_owned`) across Library, Sidebar, and Playlist detail views: non-owned playlists hide Edit controls and display a `+` / `✓` Add-to-Library action synced with YouTube Music (`set_playlist_saved`), plus Pin/Unpin support that promotes pinned playlists to the top of Library/Sidebar and Home.
  - Live-refreshed opened playlists in the background so creator edits appear immediately, updated Liked Music (`VLLM`) cover art from the latest liked song, and added listener artist-affinity ranking + proactive endless autoplay prefetching (`fetch_endless_radio_queue`) for seamless queue continuation.

- **Optional Now Playing Music/Video Mode, Quality Selector, Offline Video Downloads & Fullscreen Video (`sonata-core`, `sonata-android`, `android`)**:
  - Detected official/counterpart YouTube Music videos (`resolve_music_video_candidate` / `parse_music_video_counterpart_json`) and adaptive/muxed video formats (`resolve_music_video_streams`) in `sonata-core` + UniFFI.
  - Added a top **Music / Video** mode toggle inside `NowPlayingScreen` whenever a track has an available music video, with a dismissible audio-quality drop warning dialog (`"Do not show again"` persisted in `SonataPreferences`), selectable video resolution (`Auto`, `1080p`, `720p`, `480p`, `360p`), and an immersive landscape fullscreen video overlay (`FullscreenMusicVideoOverlay`) with stable tap-to-toggle overlay controls.
  - Added **Download videos if available** (`downloadVideosIfAvailable`) and **Video Download Quality** (`1080p` / `720p` / `480p` / `360p`) under Downloads settings with offline companion video storage in `SonataDownloadManager` and offline Video mode playback.
  - Synchronized Music/Video switches and Video-mode lyrics timelines via YouTube Music `segmentMap` and SponsorBlock `music_offtopic` segments, widened manual per-mode lyric offset to `±30s`, disabled `skipSilenceEnabled` during Video playback, and fixed `CanvasVideoSurface` animated artwork resuming when switching back to Music mode.

- **Playlist Pinning, External Add-to-Library (`+`) & Live Collection Refresh (`sonata-core`, `sonata-android`, `android`)**:
  - Added collection/playlist pinning for owned and external playlists, Liked Music, and Downloads, automatically placing pinned items at the top of Library and in a new **Pinned** (`Sabitlenenler`) shelf directly beneath Last Played on Home.
  - Detected YouTube Music playlist ownership from InnerTube metadata (`Playlist.is_owned`) so non-owned playlists hide the Edit button and instead display a `+` / `✓` Add-to-Library action (persisted locally and synced via `like/like` / `like/removelike` when signed in).
  - Bypassed the 5-minute playlist cache (`fetch_collection_tracks_fresh`) whenever opening a playlist so creator updates (added or removed tracks and cover art) immediately appear.

- **YouTube Music Moods, Genres & Dynamic Feed Language (`sonata-core`, `sonata-runtime`, `sonata-desktop`, `sonata-tui`, `sonata-android`, `android`)**:
  - Propagated active UI language (`en`/`tr`/`ru`) dynamically to YouTube Music `hl` context, `Accept-Language`, and playback telemetry URLs, clearing cached artist/playlist metadata on language switch.
  - Exported `fetch_explore_feed`, `fetch_mood_category`, `fetch_home_mood_chips`, and `fetch_home_mood_feed` over UniFFI (`AndroidMoodChip`, `AndroidMoodCategory`, `AndroidExploreFeed`) and wired live Home mood filter chips, dynamic Moods & Genres tiles, and a dedicated `MoodCategoryScreen` browse view.
  - Expanded shelf and section localization (`localizeShelfTitle`, `localizeMoodTitle`) and multilingual (`en`/`tr`/`ru`) shelf classification across Home, Explore, Radio, Artist, and Detail views, plus wired `artworkTintedPages` across Detail and Artist screens.

- **Personalized Recommendations, Endless Radio Continuation & Persisted Playback Modes (`sonata-core`, `sonata-runtime`, `sonata-desktop`, `sonata-tui`, `sonata-android`, `android`)**:
  - Added multi-seed personalized `fetch_endless_radio_queue` with `AUTOMIX_SETTING_NORMAL` / persistent playlist panel support, playlist automix continuation (`RDAMPL`), listener artist-affinity weighting, and diversity-balanced ranking (deduplication, non-music suppression, and consecutive/sliding-window artist limits) exported to Desktop, TUI, and Android.
  - Upgraded Android Home, Explore, and Radio recommendations (artist-affinity Top Picks, Trending & Viral scoring, recency-weighted Artist Stations, Discovery & Personal station launches, and `BasicExploreScreen` personalized picks) plus proactive endless continuation prefetching.
  - Persisted endless playback (`autoplay`), `shuffle`, and `repeat` modes across restarts across Desktop/TUI (`AppConfig`) and Android (`SonataPreferences`, excluding secrets from backup, clamping restored repeat mode).
- **Complete Localization (`en`, `tr`, `ru`) & Adaptive Tab Bar Labels (`sonata-runtime`, `sonata-desktop`, `sonata-tui`, `android`)**:
  - Extended `sonata-runtime::i18n` and Android `SonataStrings` (`en`/`tr`/`ru`), including shelf and mood title localization (`localize_shelf_title` / `localizeShelfTitle`, `localizeMoodTitle`), across Settings, Home, Explore, Radio, Library, Search, Artist, Downloads, Now Playing, context menus, share sheets, and ViewModel status messages.
  - Added adaptive density-capped auto-shrinking labels (`NavTabLabel`) and adjusted tab bar insets in `SonataNavigationChrome` / `FloatingTabBar` to eliminate horizontal text clipping on longer Turkish and Russian navigation labels.
- **Multilingual Localization (`en`, `tr`, `ru`) Across All Clients**: Added English, Turkish, and Russian UI translations across Android (`AppLanguage.SYSTEM` default following device locale), Desktop, and TUI (`AppLanguage::En` default) with live language switching in Settings.
- **YouTube Music Playlist Editing & Custom Cover Sync (`sonata-core`, `sonata-desktop`, `sonata-android`, `android`)**: Added playlist rename, description/privacy updates (`browse/edit_playlist`), and custom cover image upload (3-step resumable upload + `ACTION_SET_CUSTOM_THUMBNAIL`) synced directly to the signed-in YouTube Music account on both Desktop and Android.
- **Android In-App APK Auto-Download & Installer (`android`)**: Added automatic arm64 APK download into `cacheDir/updates/` with immediate Android PackageInstaller launch (`FileProvider` + `REQUEST_INSTALL_PACKAGES` permission prompt fallback) from the Update Available dialog.
- **Automated GitHub AI Issue Triage & Templates (`.github`)**: Added `.github/workflows/issue-triage.yml` backed by GitHub Models (`openai/gpt-4o-mini` with multilingual fallback) to classify and label new issues (`bug`, `feature request`, `question`, `android`, `desktop`, `tui`, `needs-info`) and request missing logs, screenshots, version/environment, and reproduction steps, plus structured bug/feature issue forms.

### Fixed

- **Desktop Mood Category Routing & Instant Thumbnail Cache Refresh (`sonata-desktop`)**: Routed `HomeCardKind::Mood` clicks through `OpenMoodByTitle` (`mood_tiles` -> `home_chips` -> `search_rich` fallback) while filtering legacy mood-only shelves, prioritized active detail-page fetches via `prefetch_thumbnails_urgent`, and bumped `thumb_rev` on `ThumbnailsReady` so `MediaCache::clear_missing` immediately invalidates negative cache entries and increases `MAX_CONCURRENT_DECODES` to `16`.
- **Android Library Downloads, Gestures & Smooth Scrim Gradients (`android`)**:
  - Removed the duplicate Downloads manager row in Library and updated the Downloads playlist grid card to display clean full-bleed album artwork from the most recently downloaded song (falling back to the Downloads gradient icon when no cover is available).
  - Fixed `ThinSlider` drag gesture handling with `rememberUpdatedState`, zero-width layout guards, and `try/finally` finish-callback cleanup, and updated `FloatingTabBar` horizontal drag gestures to read `rememberUpdatedState(tabs)`.
  - Replaced `Color.Transparent` gradient stops across UI cards, overlays, and artwork scrims with smoothstep-eased `smoothScrimBrush` / `artworkScrimBrush` and RGB-matched zero-alpha stops to eliminate mid-fade banding.
- **README Download Badge (`README.md`)**: Removed the duplicate lower Android download badge under `# Download` while retaining the top hero badge.
- **Desktop Player Bar Alignment & Collection Header Spacing (`sonata-desktop`)**: Aligned bottom player bar action icons, explicit badge, and volume controls onto a consistent centerline; fixed playlist/album header artwork wash fill and top tracklist spacing so tracks never overlap the cover header.
- **TUI 60 FPS Spring Lyrics Scroll & Luminous Wave (`sonata-tui`)**: Added critically damped spring scroll and upward luminous `tachyonfx` sweep transitions for synced lyrics.

## [1.0.6] - 2026-11-23

### Added

- **Monochrome Basic Theme, Neutral Crystal-Clear Liquid Glass & Expanded Settings (`android`)**: Added the `Basic` monochrome editorial design theme overhaul (Home, Explore duotone mood sleeves, Album/Playlist detail, and Artist hero) while preserving Violet and Moonflower layouts; unified Liquid Glass across all themes to crystal-clear neutral refraction (removing colored theme glass tints) with working live toggle; rebuilt Settings into a pure monochrome 9-category inset-grouped menu with live search, download quality tiers (`Highest` to `Data Saver` with per-song MB estimates), `Include JioSaavn` offline source toggle, Internal/External download storage selector + live usage meter, Home content visibility controls, full-screen cover/detail toggles, total duration toggle, sleep timer fade-out, status bar/badge toggles, live Nerd Stats telemetry pill, and image cache clearing.
- **Desktop Image Cache Stabilization & Fullscreen Lyrics Overhaul (`sonata-desktop`)**: Fixed image flickering and heavy CPU spikes on large playlists by raising the image cache budget (`160 MiB` soft / `256 MiB` hard cap), adding a 4-second anti-thrash eviction guard, capping concurrent decodes, and windowing the `Next up` queue; redesigned Fullscreen Now Playing lyrics with centered cover/metadata, word-synced highlight sweep, interlude markers, right-aligned counter-vocals, and distance-based fade.
- **Desktop Audio-Only Background Freeze & Low-Power Mini Mode (`sonata-desktop`)**: Observed native GPUI window visibility (`observe_window_visibility`) and window bounds (`observe_window_bounds`) so minimizing to the taskbar, switching workspaces, closing to the system tray, or shrinking into mini-player dimensions (`<= 520x260`) completely freezes all UI timers, karaoke ticks, ambient wave drift, progress-slider redraws, and `ffmpeg` motion-cover decoding while keeping full-quality audio playback, Hi-Fi upgrades, MPRIS2, and tray controls running cleanly.
- **Shared Linux Runtime Crate & `PlaybackSession` (`sonata-runtime`)**: Extracted Linux-only adapters (`mpv`, `yt-dlp`, `ffmpeg`, CLI JS cipher bridge, browser cookie extraction, XDG TOML config, Hyprland IPC, image decoding, and tracing) into a dedicated `sonata-runtime` crate and unified TUI and Desktop playback orchestration under a single `PlaybackSession` state machine with bounded non-dropping command/event channels and coalesced progress updates.
- **Automated Architecture, Security & Android CI Verification**: Added `scripts/check_boundaries.sh` (verifying Cargo dependency direction and forbidden crate/process boundaries), parser fuzz-corpus and adversarial redaction tests in `sonata-core`, Android JVM unit tests (`testDebugUnitTest`) and `arm64-v8a` `cargo ndk` builds in PR CI, and an Android instrumented suite (`NativeEngineInstrumentedTest`) verifying real native library loading, `0700`/`0600` permissions, and async cancellation.

### Changed

- **Cancellable Async Android UniFFI Bridge (`sonata-android`, `android`)**: Converted long-running Android FFI operations to native UniFFI Tokio `async fn` (`#[uniffi::export(async_runtime = "tokio")]`), removing the private Tokio `Runtime` and all `Runtime::block_on` calls so cancelled Kotlin coroutines immediately cancel and drop in-flight Rust futures.
- **Consolidated Provider & Motion Media Parsers (`sonata-core`, `sonata-desktop`, `sonata-android`, `android`)**: Consolidated Desktop `ytm_extra.rs` into pure `sonata-core::providers::ytmusic` parsers (`browse`, `album`, `search`) with shelf-order preservation and dropped-row reporting; consolidated Android JioSaavn search/decryption/upgrade matching and remote Canvas/motion discovery (`MotionMedia`) into `sonata-core` + UniFFI while retaining Android Media3, downloads, and LRU caching in Kotlin.
- **Explicit Core Paths, Profiles & Typed Errors (`sonata-core`)**: Replaced process-wide path/policy statics and provider `cfg!(target_os = "android")` branches with instance-scoped `CorePaths`, explicit `ResolverProfile`, and redacted `CoreError` / `SonataAndroidError` variants, reducing Android transitive dependencies and stripped `arm64-v8a/libsonata_android.so` size below the `1.0.5` baseline.

### Fixed

- **Android Playback Lifecycle & Foreground Service Transitions (`android`)**: Introduced `PlaybackOrchestrator` with monotonic playback generations, generation-keyed recovery budgets, typed `RateLimited` handling, atomic stream swaps preserving play/pause state and position, narrow `STATE_ENDED -> STATE_BUFFERING` suppression across pending queue transitions, and full job/bridge teardown in `onCleared()`.
- **Idle Play/Pause State in Shared Linux Session (`sonata-runtime`, `sonata-desktop`, `sonata-tui`)**: Ignored initial `mpv --idle=yes` `pause=false` property notifications when no track is loaded so Desktop and TUI accurately display the Play icon instead of Pause while idle.
- **Android Cold-Start StrictMode & UI Contrast (`android`)**: Moved `SonataPreferences` and `SonataDownloadManager` `SharedPreferences` reads off the main thread to eliminate startup `DiskReadViolation` stalls; added top clearance for the Home sign-in banner under the floating action pill, lifted `ArtistCollageArt` thumbnails with a bottom readability scrim, and added a top status-bar protection scrim over bright Now Playing covers.
- **Narrow-Window TUI Text Clipping (`sonata-tui`)**: Wrapped centered lyrics placeholders, adapted the Search placeholder to available bar width, and truncated narrow footer status strings with an ellipsis.
- **Desktop Fullscreen Ambient Waves, Subpixel Karaoke, Spring Scroll & TUI Character Sweep (`sonata-desktop`, `sonata-tui`)**: Replaced coarse byte-boundary highlights with a 45–60 FPS subpixel feathered letter-by-letter karaoke sweep (`ui/lyrics.rs`) backed by `shape_line`-measured wrapping (eliminating right-edge text overflow) and critically damped spring scroll + staggered per-row wave transitions across both Fullscreen and the Right Panel; upgraded TUI synced lyrics to 30 FPS character-by-character luminous sweep with parallel `tachyonfx` line transitions; anchored the search suggestions popover to live measured search-pill bounds; upgraded cover color extraction to a 24-bin dual-hue palette with centered `[-1, 1]` Perlin gradient noise, layered runner-up pools, smooth cover-change color washes, and background-rendered animated wave drift.

## [1.0.5] - 2026-11-23

### Added

- **Floating Glass Desktop Redesign (`sonata-desktop`)**: Rebuilt the desktop shell with `gpui-kit` components, compositor blur (`WindowBackgroundAppearance::Blurred`), frameless client chrome (`--decorated` / `--scheme`), 29 live-reloading CSS themes with WCAG contrast lifting and GTK/Qt palette sync, procedural gradient covers for Moods & Genres, and a live Sonata-only CPU & RAM (RSS) usage readout in Settings.
- **`tachyonfx` Terminal Animations (`sonata-tui`)**: Added region-scoped shader-like transitions (`Startup`, `TrackList`, `LyricsLine`, `PlayerBar`, `Modal`) powered by `tachyonfx` with automatic idle frame gating so CPU usage drops to zero when animations settle.
- **Multi-Distro Linux Release Packaging**: Extended `.github/workflows/release.yml`, `PKGBUILD`, `PKGBUILD.desktop`, and `flake.nix` to ship native `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.tar.gz` + AUR `PKGBUILD`s (Arch), NixOS Flake, and AppImage + `.zsync` bundles.
- **Direct GitHub Releases Update Actions**: Wired update buttons in Desktop and TUI Settings to open the GitHub Releases page directly.

### Changed

- **Full-Sentence Glowing Synced Lyrics (`sonata-desktop`, `sonata-tui`)**: Replaced word-by-word underlining with full-sentence lighting; on Desktop, active lines bloom smoothly over `320 ms` in pure white with left/right lead & counter-voice alignment and harmonious overlapping `(...)` background vocal highlights.
- **Isolated Album-Art Fullscreen (`sonata-desktop`)**: Hid the underlying app shell when Fullscreen Now Playing is open to prevent bleed-through and background clicks, and derived an opaque album-art color theme and accent from the active cover.
- **Single-Track YouTube Music History Reporting (`sonata-core`, `sonata-tui`, `sonata-desktop`, `sonata-android`)**: Removed `playlistId` / `list=` parameters from YouTube Music `report_playback` telemetry so only individual played songs are reported to listening history.
- **Bit-Transparent Android Playback (`android`)**: Removed the built-in 5-band Equalizer and LoudnessEnhancer DSP effects and their UI controls from Android to match the unfiltered audio path on Desktop and TUI.

### Performance

- **Major Desktop & TUI RAM/CPU Reduction (`sonata-core`, `sonata-desktop`, `sonata-tui`)**: Reclaimed evicted GPUI GPU atlas textures via `cx.drop_image(...)`, capped motion covers to `360x360 @ 15 fps` (lazy-loaded only while Fullscreen is open), lowered playback tick frequency from `60 Hz` to `4 Hz` (`250 ms`), reduced SQLite `mmap_size` to `8 MiB`, and split TUI ticks into `33 ms` active animation vs `250 ms` calm playback (dropping Desktop RSS from `~854 MiB` to `~325–356 MiB` and settled CPU to `0.00%`).

### Fixed

- **Desktop Image Flickering, HD Artist Banners & Hover Corners (`sonata-desktop`, `sonata-core`)**: Fixed `MediaCache` LRU thrashing with `32 MiB` budget, `256px` square covers, hit-first lookups (zero `stat()` syscalls on hits), negative caching, coalesced `16 ms` redraws, rounded hover scrims, uniform square sizing on mixed shelves (`Listen again`), and preserved `1600x900` HD artist banners with self-healing disk cache upgrade.
- **Artist, Album & Playlist Navigation & Visual Polish (`sonata-desktop`)**: Fixed album track parsing so play counts (`"802M plays"`) are never treated as artist names, resolved primary artists directly to the Artist page instead of falling back to Search, added true per-pixel bottom alpha fades for artist banners and `header_wash` blurs, added song thumbnails and persistent liked-track heart indicators on Artist Top Songs, fixed `[E]` explicit badge and Like/Dislike alignment in the player bar, added Shuffle/Repeat active indicator dots, and centered the search suggestions dropdown directly beneath the search bar.

## [1.0.4] - 2026-11-23

### Added

- **Optional JioSaavn 320kbps AAC Module (`sonata-core`, `sonata-tui`, `sonata-desktop`)**: Added pure-Rust DES-ECB JioSaavn stream decryption, catalog search integration, and automatic background quality upgrades (`<300kbps -> 320kbps AAC`), toggleable from Settings in both TUI and Desktop.
- **Multi-Provider Animated Album Artwork (`sonata-core`)**: Expanded motion cover resolution to query Apple Music AMP catalog `editorialVideo` (via anonymous web-player JWT), Apple Music HTML fallback, Tidal `album.videoCover` (`1280x1280.mp4`), and Community Canvas JSON manifest, with HLS `ffmpeg` fallback alongside byte-range fMP4 segment downloads.
- **Android-Style Now Playing Quality Tag (`sonata-desktop`)**: Added the frosted lossless-wave quality pill (`Upgrading Quality` while upgrading, then active stream codec/bitrate) centered beneath the progress bar between elapsed and remaining time in Fullscreen Now Playing, as well as the player bar, lyrics stage, and mini player.
- **Categorized TUI Settings & Toggleable Motion Artwork**: Redesigned the TUI Settings modal into categorized sections (`AUDIO & SOURCES`, `APPEARANCE & COMPOSITOR`, `STORAGE & SYSTEM`) with color swatches, visual opacity slider bar, contextual setting descriptions, full mouse scroll support, and an **Animated Album Artwork** toggle in both TUI and Desktop Settings.
- **NixOS Flake & AppImage `zsync` Auto-Updates**: Added `flake.nix` for NixOS/Nix builds, expanded `install.sh` / `install-desktop.sh` and browser cookie discovery for NixOS, Arch, Fedora (Flatpak), and Ubuntu (Snap/Flatpak), and configured AppImage `.zsync` update metadata in `.github/workflows/release.yml` for Gear Lever automatic updates.

### Changed

- **Direct Bit-Perfect Audio Playback (`sonata-core`, `sonata-tui`, `sonata-desktop`)**: Completely removed `EnhancerMode` / `--af` lavfi DSP filters so audio plays directly unfiltered as decoded.
- **Default Desktop Theme Only (`sonata-core`, `sonata-desktop`)**: Removed extra built-in desktop theme variants (`Spotify`, `YouTube Music`, light skins, and theme selector chips) so `sonata-desktop` uses the default `Sonata Glass` theme exclusively, while keeping custom/built-in TUI themes intact.
- **Cleaner Quality Badges (`sonata-core`, `sonata-desktop`, `sonata-tui`)**: Removed `" · Cached"` suffixes from cached stream badges across Desktop and TUI (showing only the quality in TUI) and removed `"MOTION"` pills from Desktop artwork/chrome.
- **Background Quality Upgrade From Local Cache**: `StreamResolver::resolve_fast` returns local cached audio immediately while `resolve_hifi_upgrade` and `PlaybackState::set_upgraded_stream` still upgrade cached tracks in place to peak YTM Extreme or JioSaavn 320kbps AAC when available.
- **TUI Player Bar & Version Hygiene**: Removed version strings from the main TUI player bar/footer so only the unclipped stream quality badge is shown, while keeping version and update details inside Settings (with a compact version-free update indicator on the Settings footer hint when an update is available).
- **Android-Aligned Lyrics Sync & Desktop/TUI Rendering**: Aligned `active_line_index` and word-by-word karaoke sweep timing (`0.05s` line lookahead, linear `0ms`-lead word sweeps without artificial span truncation), validated `.lrc` and SQLite caches against newly discovered track durations, enabled counter-voice alignment & italic background-vocal styling in Desktop root stage lyrics, and switched TUI lyrics to theme-aware palette colors.

### Fixed

- **Fedora PipeWire / `ffmpeg-free` No-Audio Issue (#1)**: Fixed silent playback on Fedora Linux by removing `--af` DSP filters and launching `mpv` with `--no-config`, `--ao=pipewire,pulse,alsa`, `--audio-client-name=Sonata`, `--aid=auto`, and `--mute=no`.
- **TUI Kitty Animated Artwork Rendering**: Fixed `last_art_rect` detection when motion frames arrive and replaced the 60-frame bulk Kitty pre-upload with compact double-buffered (`i=2`/`i=3`) `360x360` transmission to prevent terminal graphics memory overflow and event-loop stalls.

## [1.0.3] - 2026-11-22

### Performance

- **Release Profile & `sonata-core`**: Enabled thin LTO (`lto = "thin"`) and single codegen unit (`codegen-units = 1`) for release builds; fixed `CACHE_SCHEMA_VERSION` (`3`) so existing SQLite caches reopen properly, enabled 64 MiB `mmap_size`, switched hot queries to `prepare_cached`, amortized `stream_cache`/`playlist_cache` pruning to every 32 writes, eliminated per-call heap allocations in URL helpers and `fetch_and_decode_art`, cached Spotify bundle regexes in `LazyLock` and replaced dynamic regexes with slice scanning, tuned `reqwest` connection pooling, parallelized `StreamResolver::warm` with `tokio::join!`, and gated `MpvController` debug payload redaction behind `tracing::enabled!(DEBUG)`.
- **`sonata-android` UniFFI Bridge**: Added owned `From<T>` conversions to move playlists, tracks, home shelves, and artist pages across FFI without deep cloning, and released the `loaded_hash` mutex before calling the Android WebView JS cipher solver.
- **`sonata-tui` & `sonata-desktop`**: Short-circuited `export_now_playing()` when playback state is unchanged and reused cached artwork paths to eliminate 30 Hz `stat()` syscalls; eliminated per-word `String` allocations in `render_active_tui_lyric_line`; gated `sonata-desktop` motion cover frame ticks on `!paused`; upgraded `BlurredLyricCache` to refresh LRU recency on hit and bounded Gaussian blur rasterization to nearby verses (`distance <= 8`).
- **Android Compose UI, Data & R8**: Deferred `ThinSlider` sheen and `MeshGradientBackground` color/brush animations to draw-phase reads with cached radial gradient brushes; removed duplicate `drawThemeAtmosphericBackdrop` overdraw in `MainActivity`; cached `VioletTheme`/`MoonflowerTheme` atmospheric `Path`/`Brush`/`Stroke` objects across scroll frames; deferred `boundsInRoot()` calculations via `LayoutBoundsHolder` until long-press/options menu open; memoized `GlassSelectionPill` effect lambdas with `0.5f` lens backdrop scale; added `contentType`, hoisted scroll state, and remembered `TextStyle` in `EnhancedLyricsPanel`; short-circuited no-op `PlaybackClockState.sync` ticks; offloaded `SonataWidgetUpdater` and backup file I/O to `Dispatchers.IO`; guarded `POST_NOTIFICATIONS` launcher checks on cold start; and tightened `proguard-rules.pro` keep rules to enable full R8 shrinking and optimization across Media3, Coil 3, and app classes.

## [1.0.2] - 2026-11-22

### Added

- **Dual `Violet` & `Moonflower` Themes (Light & Dark, Default = `Violet` + `System`)**: Split theme styling into dedicated modular theme definitions (`VioletTheme.kt`, `MoonflowerTheme.kt`) with distinct light and dark atmospheric canvases, typography treatments, layout shapes, and Liquid Glass surfaces.
- **Pinkish Liquid Glass Navigation Pill (`GlassSelectionPill.kt`)**: Added refractive pinkish Liquid Glass styling for the active bottom navigation tab pill and translucent bottom chrome in both `Violet` and `Moonflower` light/dark modes.
- **Theme-Specific `"YOUR LISTENING EXPERIENCE"` Raster Artwork**: Generated high-resolution `1200x756` PNG illustrations (`bg_listening_exp_violet*.png` purple moon editions and `bg_listening_exp_moonflower*.png` pink blooming flower editions) in `drawable-nodpi` for Library summary cards.
- **YouTube Music Playback Reporting (`report_playback`) & Live `Library -> Artists` Sync**: Wired YouTube Music playback history telemetry across `sonata-core`, `sonata-tui`, `sonata-desktop`, and `sonata-android` (including YouTube Music `videoId` resolution for JioSaavn-upgraded streams so Annual Replay / Watch History stays accurate), plus immediate promotion of played artists to the top of `Library -> Artists`.
- **Top-Right `[History | Avatar]` Liquid Glass Capsule & `ListeningHistoryScreen`**: Removed the Home top-left moon logo, added a Liquid Glass listening history button next to the profile avatar on Home, and introduced a dedicated `ListeningHistoryScreen`.
- **Visual Share Sheet (`SonataShareSheet.kt`)**: Added artwork-and-theme-reactive preview cards tailored per Track, Album, Playlist, and Artist with a subtle bottom-right Sonata watermark and one-tap **Share Link**, **Share Image** (`FileProvider`), and **Save Image** (`Pictures/Sonata`) actions.
- **15 Unique Centered `Find Your Mood` Animations & Moon Badges**: Gave every mood card its own seamlessly looping centered artwork animation (`SonataAnimatedArtworks.kt`) with small top-right moon emblem badges across `Top Picks for You` and `Find Your Mood`.

### Changed

- **Background Quality Upgrade for Cached & Offline Tracks**: Unified `launchBackgroundQualityUpgrade` in `SonataViewModel.kt` so cached and offline-downloaded tracks still check for higher-bitrate streams matching the user's `AudioQualityTier` and seamlessly upgrade playback.
- **Modular Android UI Architecture & Streamlined Dependencies**: Split oversized Android screens into focused components (`HomeScreen.kt`, `ExploreScreen.kt`, `LibraryScreen.kt`, `DetailScreen.kt`, `NowPlayingScreen.kt`, `SearchScreen.kt`, `SonataNavigationChrome.kt`, `SonataIcons.kt`, `SonataPalette.kt`, `SonataShimmer.kt`), enabled Gradle build/configuration caches on JDK 21, and removed F-Droid/IzzyOnDroid/Fastlane metadata plus in-app `REQUEST_INSTALL_PACKAGES` permission (updates open external browser download).
- **Refreshed `Radio`, `Explore` & `Settings` Layouts**: Removed stale caching and merged/deduplicated `New Releases` across `Explore`, filled out `Radio` mixes density, refined the Settings hub, added progressive blur dissolve on `ArtistScreen` hero headers, and exact-fitted `Top Picks for You` and `Find Your Mood` rows.

### Fixed

- **Account Switching WebView Reset**: Cleared WebView cookies, `WebStorage`, cache, and history asynchronously before launching Google Sign-In (`prompt=select_account`) and gated auto-capture on visiting the auth flow so switching accounts never bounces back into the previous session.
- **Sticky Share Notification Banner**: Auto-cleared the share save status notification after 2 seconds so `"Saved image to Pictures/Sonata"` does not persist across tab switches.
- **Detail Header Text/Cover Overlap & Library Artist Keys**: Refactored `SonataSplitHeroHeader` (`SonataCards.kt`, `DetailScreen.kt`) so titles never overlap cover art or placeholder icons on Downloads, Albums, and Playlists, and fixed duplicate LazyRow keys when synthesizing un-enriched recent artist cards in `LibraryScreen.kt`.

## [1.0.1] - 2026-11-22

### Added

- **Lunaria Moonflower Theme & Typography**: Introduced a dedicated moon-themed Lunaria color palette (`LunariaPalette.kt`: `#B79CED` Petal Lavender, `#8F96D9` Lunar Mist, `#EDE8F8` Moon Glow) and bundled open-source **Plus Jakarta Sans** (SIL OFL-1.1) app-wide typography while preserving the dedicated Lyrics display font.
- **Full-Screen Artist Page & Backend (`ArtistScreen.kt`)**: Added native in-app Artist pages powered by `fetch_artist_page`, `fetch_artist_by_name`, and `resolve_artist_browse_id` with full-bleed hero artwork dissolving seamlessly into the page background, centered sculptural Shuffle / `PLAY` / Subscribe controls, tilted layered-sleeve **Latest release** spotlight card, numbered multi-column **Popular tracks** with quick Play button, Albums & Singles, Similar Artists, About bio, and 15-minute LRU caching.
- **Editorial Explore & Material 3 Expressive Search Pages**: Redesigned **Explore** with an editorial hero carousel and 4-row horizontal paging **Trending & Viral Hits** columns, and redesigned **Search** with wrapped quick-search chips, scalloped **Recent searches** cards, and full-width responsive **Samples** portrait grids with zero right-edge overflow or portrait dead space.
- **Modules Settings & JioSaavn High-Bitrate Source (`JioSaavnSource.kt`)**: Added a **Modules** category in Settings with an opt-in **JioSaavn** toggle (default OFF, with a matching accuracy notice) that searches JioSaavn in parallel and upgrades playback seamlessly whenever a strictly higher-bitrate (up to 320 kbps AAC) stream is found.
- **Downloaded Track Badges**: Surfaced crisp downloaded track icons (`↓`) beside artist names across album/playlist details, Quick Picks, Search, Library, and Artist Popular tracks.
- **Radio & Mixes Tab (`RadioMixesTab.kt`)**: Added a 5th bottom tab with dedicated YouTube Music Mix fetching (`fetch_radio_mixes`: Featured Mixes, Signature Mixes, Mood Mixes) and personalized quick-start radio (`fetch_radio_queue`).
- **Held Context Menu, Anchored 3-Dot Menu & Add to Playlist (`HeldContextMenu.kt`, `TrackOptionsMenu.kt`, `AddToPlaylistSheet.kt`)**: Added iOS-style lifted long-press context menus across tracks, albums, playlists, and Top Picks; anchored Liquid Glass 3-dot track menus with no dimming scrim; and full YouTube Music `Add to Playlist` / `Create Playlist` support (`add_tracks_to_playlist`, `create_playlist`).
- **In-App Downloads Playlist & Offline Playback (`DownloadManager.kt`, `DownloadsScreen.kt`)**: Added permanent offline track/collection downloads surfaced as a first-class in-app **Downloads** playlist inside Library with offline-first playback and cache eviction (`Remove from Cache`).
- **Animated Album Art (Canvas) & Procedural Animated Covers (`CanvasResolver.kt`, `CanvasVideoPlayer.kt`, `SonataAnimatedArtworks.kt`)**: Multi-provider Canvas video resolver (Apple Music HLS, Tidal, Community MP4 with local `CanvasClipFileCache` for non-faststart MP4s) plus 14 original centered, bounds-clipped animated covers for Personal/Discovery Stations and Find Your Mood.
- **Now Playing Options Menu, Audio Pipeline & Full-Screen Categorized Settings (`NowPlayingOptionsMenu.kt`, `AudioPipelineModal.kt`, `CategorizedSettingsSheet.kt`, `UserMenuSheet.kt`)**: Added Liquid Glass Now Playing `⋯` options popover (housing Audio Controls, Sleep Timer, Lyrics Offset, Romanize, Translate, Download, Open Album/Artist), 4-stage Audio Signal Chain inspector, Liquid Glass Account menu, and clean neutral full-screen Settings with **60 / 90 / 120 FPS** hardware refresh rate support.
- **Liquid Glass Reference Guide (`research/liquidglass.md`)**: Documented Sonata's optical refraction shader parameters, surface rules, and `LayerBackdrop` sibling guardrails.

### Changed

- **Android 60 / 90 / 120 Hz Performance & R8 Release Pipeline**: Added `androidx.profileinstaller:1.4.1`, zero-recomposition `ScrollActivityTracker`, `graphicsLayer`-driven `MarqueeText` & lyric spring lift, `0.20f` Liquid Glass backdrop scaling, hardware-accelerated `ReleaseArt` scrims, cached `SonataPreferences` / `ThumbnailFallback`, off-main-thread `DownloadManager` & `CanvasCache` init, async `ExoPlayer.release()`, and stable LazyList keys with `SaveableStateProvider`.
- **Personalized Home Ordering & Lyrics Controls**: Reordered Home (`Top Picks for You` -> `Last Played` -> `Son Dinlenenler` -> `Find Your Mood` -> `Your Playlists` -> `New Albums & Singles` -> `Trending`), removed `Forgotten Favorites`, placed the Lyrics `Translate` toggle cleanly between `Lyrics` and `Queue` (automatically swapping to `Shuffle` / `Repeat` / `Autoplay` when Queue is open), positioned the Now Playing `⋯` menu in the upper-right with full natural height (no unnecessary scrolling), and refined the quality badge into a steady gray base with a slow left-to-right white glowing sweep.
- **YouTube Music URL Link Sharing & Collection `⋯` Menu**: Replaced plain-text share snippets with clean YouTube Music URLs (`https://music.youtube.com/watch?v=...`, `/playlist?list=...`, `/browse/...`, `/channel/...`) so Android Share Sheet previews links properly, and wired the album/playlist header `⋯` button to a full collection options menu.

### Fixed

- **Security (`RUSTSEC-2026-0592`)**: Upgraded `url` (`2.5.8 -> 2.5.9`) and `form_urlencoded` (`1.2.2 -> 1.2.3`) in `Cargo.lock` to resolve the nested `blob:` URL stack overflow advisory in CI `cargo audit`.
- **Album/Playlist Downloads, Overlapping Lyrics & Card Corner Clipping**: Stripped fast-start `&range=` query parameters and added `resolveRecovery` retries + live progress/checkmark feedback for album/playlist downloads; enabled simultaneous active glow for overlapping background vocals like `(hey)` in synced lyrics; and removed unpadded rounded clipping from outer card columns so corner letters are never shaved off.

- **Account Auth, Empty Library Playlists & Explicit Audio Quality**: Added `__Secure-1PAPISID` + triple-hash `SAPISIDHASH`, Brand Channel `X-Goog-PageId` scoping, grid+list playlist parsing with continuations, explicit track preference (`prefer_explicit_order` / `resolve_explicit_counterpart`), mid-track recovery (`resolveRecovery`), and 512 KiB `ChunkedDataSource` range caps.
- **Album Track Metadata Backfill & Open Album**: Backfilled omitted album track artist names, cover artwork, and `album_id` from collection headers, and routed `Open Album` directly to the real album detail page instead of Search.
- **Stability, Deduplication & Navigation**: Fixed `RenderNode` self-referential `SIGSEGV` crashes on Liquid Glass overlays, strictly deduplicated `Last Played` by ID and canonical title+artist, fixed `SweptKaraokeLyricText` line-wrap clipping, fixed auto-search on type, and added hierarchical `BackHandler` navigation across Settings submenus, Search, and tabs.

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

[Unreleased]: https://github.com/MehmetCanWT/sonata/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/MehmetCanWT/sonata/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/MehmetCanWT/sonata/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.0
