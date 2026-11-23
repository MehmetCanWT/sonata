# ADR-0001: Portable Core, Linux Runtime, and Android Bridge Boundary

- **Status:** Accepted
- **Date:** 2026-11-23
- **Baseline Commit:** `91cbd73`

## Context

`sonata-core` currently combines portable engine responsibilities (YouTube Music, InnerTube, cipher preparation, JioSaavn, lyrics, SQLite metadata cache, playback state primitives) with Linux-only desktop/terminal runtime adapters (`mpv` IPC, `yt-dlp`, `ffmpeg`, Node/Bun/Deno process runners, browser cookie database readers, image/motion decoding, Hyprland integration, TOML/XDG app config, and file logging).

As a result:

1. `sonata-android` transitively compiles and links desktop-only crates (`image`, `dirs`, `toml`, `tracing-appender`) and dead subprocess code paths.
2. Provider resolution encodes platform policy through ten compile-time `cfg!(target_os = "android")` branches and four process-wide statics (`ANDROID_DIRS`, `ACTIVE_CACHE_LIMIT_GB`, `ACTIVE_JIOSAAVN_ENABLED`, `ACTIVE_MOTION_COVERS`), preventing host-side testing of Android policy and isolated parallel engine instances.
3. Exported `SonataAndroidEngine` network/resolver methods run synchronously via Rust `Runtime::block_on`, preventing true Kotlin coroutine cancellation from dropping in-flight Rust futures.

## Decision

### 1. Keep One Portable `sonata-core`

`sonata-core` remains the single source of truth for portable engine logic: models, YouTube Music / InnerTube / JioSaavn / Spotify protocols, cipher preparation and `JsCipherBridge` contract, lyrics resolution, update protocol, typed `thiserror` public errors, SQLite metadata cache, pure playback coordinator/task/precache primitives, cover URL normalization/lookup, and platform-neutral `MotionMedia` discovery.

`sonata-core` intentionally owns portable HTTP (`reqwest`), SQLite (`rusqlite`), serialization (`serde`/`serde_json`), and filesystem caching. Its boundary is **platform neutrality**: it must never depend on UI frameworks, Linux IPC, external subprocess launchers, browser profile readers, image/video decoders, or desktop file loggers.

### 2. Introduce One Shared Linux Adapter Crate: `sonata-runtime`

Create `crates/sonata-runtime` to hold Linux-only behavior shared by `sonata-tui` and `sonata-desktop`:

- `mpv` JSON-IPC driver and (later, Milestone C) shared `PlaybackSession`
- `yt-dlp` fallback (`LinuxStreamResolver`), `ffmpeg`, and Node/Bun/Deno JS cipher runner
- Firefox/Chromium browser cookie database readers
- Image decode/trim/resize/ambient color extraction, motion frame decoding, and `now_playing.json`
- Hyprland integration, TOML `AppConfig`, XDG path defaults, binary lookup, and persistent Linux client logging

### 3. Enforce Strict Dependency Direction

```text
sonata-tui ───────┐
                  ├─> sonata-runtime ─> sonata-core
sonata-desktop ───┘

sonata-android ────────────────────────> sonata-core
```

- Reverse edges (`sonata-core -> sonata-runtime`, `sonata-core -> client`) are forbidden.
- Client-to-client dependencies (`sonata-tui <-> sonata-desktop`, `sonata-android -> sonata-runtime`) are forbidden.
- Path roots (`CorePaths`) and stream policy (`ResolverProfile`) are injected explicitly per engine instance.

### 4. Final Ownership Table

| Component | Final Owner |
| --- | --- |
| Models, YTM/InnerTube/JioSaavn/Spotify protocols, cipher preparation, lyrics, update protocol, typed errors, SQLite cache, pure playback primitives, portable cover/motion discovery, `CorePaths`, `ResolverProfile` | `sonata-core` |
| `mpv`, `yt-dlp`, `ffmpeg`, Node/Bun/Deno runner, browser DB cookie extraction, image/motion decode, Hyprland, TOML/XDG `AppConfig`, Linux telemetry, shared `PlaybackSession` | `sonata-runtime` |
| UniFFI `0.29.5` records/errors, cancellable async API, Android `ResolverProfile`, WebView cipher adapter, app-private `CorePaths` adapter | `sonata-android` |
| Ratatui UI, terminal input, terminal artwork upload, terminal theme model & TOML loading | `sonata-tui` |
| GPUI UI, CSS themes, MPRIS2, StatusNotifierItem tray, desktop visual effects | `sonata-desktop` |
| Compose UI, Media3/ExoPlayer, foreground `PlaybackService`, downloads, preferences, WebView auth/cipher execution, notifications/widgets | `android/` |

## Milestone A Completion Boundary (Phases 0–3)

Milestone A is independently releasable and complete only when all of the following hold:

1. `crates/sonata-core/src` contains zero `(std|tokio)::process::Command` calls.
2. `crates/sonata-core/src/providers` contains zero `cfg!(target_os = "android")` branches.
3. `crates/sonata-core/src` contains zero global path/policy statics (`ANDROID_DIRS`, `ACTIVE_CACHE_LIMIT_GB`, `ACTIVE_JIOSAAVN_ENABLED`, `ACTIVE_MOTION_COVERS`).
4. `crates/sonata-core` has zero dependency edge to `sonata-runtime`.
5. `cargo tree -p sonata-android -e normal` contains none of `image`, `dirs`, `toml v0.8`, or `tracing-appender`.
6. Both Linux and Android resolver profiles pass host characterization tests, TUI/desktop playback smoke tests pass, and an `arm64-v8a` Android native build and release APK assemble cleanly.

## Explicit Non-Goals

- No UI redesign across TUI, desktop, or Android.
- No Kotlin Multiplatform conversion.
- No replacement of Media3/ExoPlayer or `mpv`.
- No database schema change unless separately justified with migration fixtures.
- No migration of Android queue authority from `SonataViewModel` into Media3.
- No DI framework (Hilt/Koin), Robolectric, MockK, or screenshot framework added solely for this refactor.
- No universal playback abstraction spanning Media3 and `mpv`.
- No speculative single-implementation traits.
