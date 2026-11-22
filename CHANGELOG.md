# Changelog

All notable changes to **Sonata** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- **Liquid Glass Reference Guide ([`liquidglass.md`](liquidglass.md))**: Documented Sonata's optical refraction shader parameters, surface rules, and `LayerBackdrop` sibling guardrails.

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

[Unreleased]: https://github.com/MehmetCanWT/sonata/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/MehmetCanWT/sonata/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.0
