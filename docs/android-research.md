# Sonata Android — Architecture & Research

> [!NOTE]
> **Status: Coming Soon**
> Android support is planned and will be built on top of the shared `sonata-core` Rust engine.

## Chosen Stack

We will build the Sonata Android client using **Kotlin (Jetpack Compose) + UniFFI (`uniffi`)**:

- **UI Layer — Kotlin & Jetpack Compose**:
  - Native Android UI built with Jetpack Compose and Material 3, supporting adaptive glass/dark surfaces, dynamic cover-art color extraction, smooth synced karaoke lyrics, and responsive mini/fullscreen player sheets.
- **Core Bridge — Mozilla UniFFI (`uniffi`)**:
  - `crates/sonata-android` will expose a clean, typed UniFFI boundary over `crates/sonata-core` so Android reuses the exact same Rust engine for:
    - YouTube Music & Spotify session authentication and library sync
    - Personalized Home feed, search, playlists, and artist pages
    - Multi-source synced lyrics (TTML, KuGou KRC, NetEase YRC, LRCLIB, YouTube Music, Genius)
    - High-res artwork & Apple Music animated motion cover resolution
    - SQLite WAL metadata cache and bounded LRU media cache
- **Audio & System Integration**:
  - Android Media3 (`ExoPlayer`) backed by a foreground `MediaSessionService` for lock-screen controls, Bluetooth/headset media buttons, Android Auto compatibility, and gapless/background playback.

## Planned Workspace Structure

```text
crates/
├── sonata-core/       # Shared Rust engine (UI-agnostic)
└── sonata-android/    # UniFFI Rust bridge crate (FFI exports + Kotlin bindings)
android/               # Gradle / Kotlin (Jetpack Compose) Android application
```
