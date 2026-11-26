---
name: sonata-desktop-tauri
description: >-
  Architectural invariants, Wayland/Hyprland tray & window lifecycle rules,
  UI/IPC contracts, and local build runbook for Sonata's Tauri v2 desktop app
  (crates/sonata-desktop and desktop-ui/). Use when modifying desktop backend,
  tray, window chrome, React UI, lyrics sync, or local desktop binaries.
---

# Sonata Tauri v2 Desktop Invariants

Follow these Sonata-specific desktop guardrails whenever modifying `crates/sonata-desktop/` or `desktop-ui/`.

## 1. Linux System Tray (`ksni`, Never Tauri `tray-icon` on Linux)

- Tauri's `tray-icon` feature pulls `libappindicator-sys`, which panics at startup (`EXIT=101`) when `libayatana-appindicator3` is missing (default Arch/Hyprland setups).
- Keep pure-Rust `ksni` (`crates/sonata-desktop/src/integrations/tray.rs`) on Linux and restrict Tauri's `"tray-icon"` feature strictly to `#[cfg(any(target_os = "macos", target_os = "windows"))]` in `crates/sonata-desktop/Cargo.toml`.

## 2. Close-to-Tray on Tiling WMs (`Meta+Q` / WM Close)

- In `crates/sonata-desktop/src/main.rs`, intercept both `WindowEvent::CloseRequested` (`api.prevent_close()` + `window.hide()`) and `RunEvent::ExitRequested` (`api.prevent_exit()`) whenever `!EXPLICIT_QUIT && is_tray_active()`.
- Track visibility via `WINDOW_VISIBLE` atomic because GTK `is_visible()` can lag on Wayland.
- Only explicit tray `Quit` (`UiWindowCmd::QuitApp`), `SIGINT`, or `SIGTERM` may set `EXPLICIT_QUIT`.

## 3. Tiling WM Window Chrome

- Keep `should_show_window_controls` in `crates/sonata-desktop/src/ipc.rs` enabled to hide custom minimize/maximize/close buttons on macOS and Linux tiling WMs (`HYPRLAND_INSTANCE_SIGNATURE`, `SWAYSOCK`, `I3SOCK`, `NIRI_SOCKET`, `XDG_CURRENT_DESKTOP`).
- Never overlap window control buttons over `TopBar` pills.

## 4. UI Craft, Responsive Layout & IPC Contracts

- **Visual language**: Preserve matte obsidian surfaces (zero Liquid Glass, zero violet hues).
- **Zero sample/preview data**: Initialize `desktop-ui/src/lib/ipc.ts` with `createEmptySnapshot()` (no hardcoded sample tracks or real artist/song names anywhere in app code).
- **Responsive containers**: Use CSS `@container` queries on `<main>` and `PlayerDock`; keep `shrink-0` on horizontal rows inside `flex-col` parents so items never collapse vertically; wrap horizontal carousels/chips in `HorizontalScroller` with `.no-scrollbar`; use `w-full` (`layout="grid"`) for `MediaCard` inside CSS grids.
- **Home card types**: Preserve `HomeCardKind` (`Song`/`Video`/`Mood`/`Album`/`Artist`/`Playlist`) and `track` in `UiHomeCardDto` (`crates/sonata-desktop/src/ipc.rs`). Clicking `Song`/`Video` cards must trigger `PlayTracks` directly instead of opening an empty playlist view.
- **Discord RPC**: Connect/disconnect directly using built-in `DEFAULT_SONATA_DISCORD_APP_ID`; never expose a manual `Custom Discord App ID` input in `SettingsView.tsx`. `"Listen on Sonata"` buttons must always open `SONATA_PROJECT_URL` (`https://github.com/MehmetCanWT/sonata`), not YouTube Music URLs; `assets.large_image` only supports static/animated image URLs (never `.mp4`/HLS motion video URIs).
- **PlayerDock & Fullscreen Karaoke**: Keep `PlayerDock` uncluttered (static cover art only, no animated `<video>`, no `kbps`/sleep-timer clutter, and only one `Mic2` lyrics toggle without a duplicate `Maximize2` button). Render `ImmersiveLyricsView` as the clean 50/50 stage with letter-by-letter glowing karaoke powered by `desktop-ui/src/lib/lyricsSync.ts`, accurate `timed_line_from` end times in `crates/sonata-desktop/src/ipc.rs`, and 45ms progress ticks.
- **VBR Bitrate Stability**: Preserve `stabilized_live_kbps` in `crates/sonata-runtime/src/player/session/quality.rs` so VBR streams do not flicker kbps every tick.

## 5. Animated Artwork, MPRIS & View Stability Invariants

- **Linux WebKitGTK green-frame prevention**: Keep the startup `GST_PLUGIN_FEATURE_RANK` override in `crates/sonata-desktop/src/main.rs` on Linux (when unset: `avdec_h264:MAX`, NVDEC/VA-API decoders `NONE`) so hybrid Wayland GPUs do not hit zero-copy NV12 neon-green chroma glitches in `<video>`.
- **Seamless motion-cover looping**: Normalize cached/downloaded motion `.mp4` files in `crates/sonata-runtime/src/media/motion.rs` to `+faststart` `moov`, `start_time=0`, zero B-frames (`-bf 0`), `yuv420p`, tagged `sonata-loop-v1`; preload into RAM `blob:` URLs via `ArtworkImage.tsx`'s bounded LRU cache with synchronous `onEnded` rewind so loops never pause or stall.
- **Always-on MPRIS & telemetry**: Never abort artwork/motion/lyrics tasks or clear `motion_cover_path` on `SetWindowVisible(false)` or `SetUiCompact(true)`. Emit static `mpris:artUrl`, custom `sonata:motionArtUrl`, and structured `$XDG_RUNTIME_DIR/sonata/{now_playing,lyrics}.json` (`schema_version = 2`) in both desktop and TUI across all view modes (including tray and `reduce_motion`).
- **Lyrics auto-follow & Library Artists**: Never use bare `onScroll` to pause `autoFollow` in `ImmersiveLyricsView.tsx` (breaks on Hyprland `Super`+drag and clicks); pause only on vertical wheel (`|deltaY| >= 4`) or touch drag (`>= 12px`), re-center via `ResizeObserver`, and surface a top `Sync` pill. In `ArtistView.tsx`, apply `@xl:grid-cols-[248px_1fr]` only when `latestRelease` exists (render full-width `TrackRow` lists for Library Artists), strip `MPLA` before `UC` in `fetch_artist_banner`, and keep download actions available.

## 6. Local Build & Smoke Test Runbook

Tauri embeds `desktop-ui/dist/` at Rust compile time—always build the frontend **before** `cargo build`:

```sh
cd desktop-ui && npm run typecheck && npm run build && npm test && cd ..
cargo build --release --locked -p sonata-desktop
install -Dm755 target/release/sonata-desktop ~/.local/bin/sonata-desktop
timeout 4 ~/.local/bin/sonata-desktop
```

Expect `timeout` exit code `124` (healthy running event loop), never `101`.
