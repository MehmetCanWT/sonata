# Sonata Codebase Analysis

Post-refactor snapshot of workspace size, modularity, cross-crate duplication, resolved fixes, and remaining technical work.

## 1. File-size and modularity breakdown

The workspace contains **28,359 lines** of Rust across 4 crates (`sonata-core`, `sonata-tui`, `sonata-desktop`, `sonata-android`; the experimental `sonata-desktop-makepad` prototype was removed).

All previously monolithic 900–2,900 line files have been split into focused submodules (each beginning with a structured `//!` overview and numbered breakdown):

| Original monolith | Split modules (current `wc -l`) |
|---|---|
| `sonata-desktop/src/backend.rs` (~2,905 L) | `backend/{mod.rs (1,364), playback.rs (843), library.rs (609), types.rs}` |
| `sonata-tui/src/app.rs` (~2,244 L) | `app/{playback.rs (959), mod.rs (640), library.rs (502), state.rs}` |
| `sonata-desktop/src/views/root.rs` (~1,552 L) | `views/root/{stage.rs (733), header.rs, player_bar.rs, sidebar.rs, mod.rs}` |
| `sonata-core/src/providers/ytmusic.rs` (~1,188 L) | `providers/ytmusic/{parsers.rs (772), mod.rs (435)}` |
| `sonata-desktop/src/views/fullscreen.rs` (~1,172 L) | `views/fullscreen/{sheet.rs (896), mod.rs, controls.rs}` |
| `sonata-desktop/src/views/fullscreen_fx.rs` (~1,014 L) | `views/fullscreen_fx/{blur_cache.rs (408), waves.rs, palette.rs, mod.rs}` |
| `sonata-core/src/providers/spotify.rs` (~905 L) | `providers/spotify/{mod.rs (711), embed.rs}` |

Remaining large single files worth monitoring:
- [`crates/sonata-desktop/src/backend/mod.rs`](../crates/sonata-desktop/src/backend/mod.rs) (1,364 lines) — still holds the Tokio event loop, `handle_command`, `handle_internal_event`, `publish`, and `export_now_playing`; `handle_command` / `handle_internal_event` could be split out further.
- [`crates/sonata-core/src/lyrics/providers.rs`](../crates/sonata-core/src/lyrics/providers.rs) (1,110 lines — TTML/Binimum, KuGou KRC, NetEase YRC, QQ Music, Genius) and [`crates/sonata-desktop/src/views/mod.rs`](../crates/sonata-desktop/src/views/mod.rs) (1,051 lines).

## 2. Resolved security, performance, and reliability fixes

| Area | Fix verified in code |
|---|---|
| **Cookie permissions race & temp cleanup** | [`crates/sonata-core/src/auth/cookies.rs:28-69,395,443,462`](../crates/sonata-core/src/auth/cookies.rs#L28-L69): `OpenOptions::mode(0o600)` at creation, `0o700` dir permissions, `TempDirGuard` RAII cleanup for `tmp_cookies_*`, `SUPPORTED_BROWSERS` id validation, and a 15s timeout around `yt-dlp --cookies-from-browser`. |
| **SQLite lock poison recovery & test isolation** | [`crates/sonata-core/src/cache.rs:189-203,598-600`](../crates/sonata-core/src/cache.rs#L189-L203): `lock_mutex()` / `lock_conn()` recover poisoned mutexes with `tracing::warn!` instead of silently skipping; unit tests use `AtomicU64` (`TEST_DB_COUNTER`) for collision-free DB paths. |
| **Fullscreen lyric blur stutter** | [`crates/sonata-desktop/src/views/fullscreen_fx/blur_cache.rs:17,141`](../crates/sonata-desktop/src/views/fullscreen_fx/blur_cache.rs#L17) & [`fullscreen/sheet.rs:152,224-240`](../crates/sonata-desktop/src/views/fullscreen/sheet.rs#L152): removed `alpha_byte` from `BlurredLyricKey` (GPUI applies `.opacity()` at display time), raised cache to `256` entries, fixed 3 distance blur buckets, added `prewarm_blur_buckets` for `active_idx.saturating_sub(2)..=(active_idx + 5)`, and reduced wave base resolution to `112x64` (`waves.rs:29-30`). |
| **Manual-scroll settle & center anchor** | [`crates/sonata-desktop/src/views/fullscreen/mod.rs:21-23`](../crates/sonata-desktop/src/views/fullscreen/mod.rs#L21-L23) & [`sheet.rs:190,311-312,592`](../crates/sonata-desktop/src/views/fullscreen/sheet.rs#L190): `MANUAL_SCROLL_SETTLE = 1200ms` with a 1220ms `manual_scroll_gen` timer, `AUTO_SCROLL_ANCHOR_RATIO = 0.50` with 50% spacers, and `AUTO_SCROLL_SPEED = 14.0`. |
| **Desktop shutdown & config save** | [`crates/sonata-desktop/src/backend/types.rs:218`](../crates/sonata-desktop/src/backend/types.rs#L218) replaces the 15ms busy-wait loop with `std::sync::mpsc::sync_channel(1)` + `recv_timeout(2s)`; [`backend/mod.rs:1138-1140`](../crates/sonata-desktop/src/backend/mod.rs#L1138-L1140) clears `config_dirty` only on `Ok(())` and logs save errors via `tracing::warn!`. |
| **Silent integration error drops** | [`crates/sonata-desktop/src/main.rs`](../crates/sonata-desktop/src/main.rs), [`integrations/mpris.rs`](../crates/sonata-desktop/src/integrations/mpris.rs), and [`integrations/tray.rs`](../crates/sonata-desktop/src/integrations/tray.rs) log failures with `tracing::warn!`. |
| **Cover enrichment & motion art** | Removed the 2,500ms sleep in `start_cover_enrichment` (TUI & Desktop), enabled it for both YTM and Spotify, added immediate `upscale_artwork_url`, and added `resolve_animated_cover_frames` (`media.rs:298`). |
| **Hyprland opacity & Wayland fullscreen** | [`crates/sonata-core/src/hyprland.rs:162`](../crates/sonata-core/src/hyprland.rs#L162) replaces invalid `hyprctl eval` with `hyprctl --batch` (`opacity_batch_command`), frontends re-detect window addresses live, and `sonata-desktop` uses `WindowDecorations::Server` + `280x110` min size with `<760x460` auto-compact Mini Player. |
| **TUI settings modal & resize artifacts** | [`crates/sonata-tui/src/ui/modals.rs`](../crates/sonata-tui/src/ui/modals.rs) widens the Settings modal (`40..=84`, 32-char label col), [`ui/mod.rs`](../crates/sonata-tui/src/ui/mod.rs) auto-activates Mini Mode below `90x20`, and `Event::Resize` clears Kitty image placements. |

## 3. Remaining cross-crate duplication & open improvements

### Duplicated playback session orchestration
`sonata-tui` (`crates/sonata-tui/src/app/playback.rs`, 959 lines) and `sonata-desktop` (`crates/sonata-desktop/src/backend/playback.rs`, 843 lines) still duplicate:
- Play-sequence gating (`play_seq`, `retried_play_seq`), `start_play_transition`, `start_hifi_upgrade`
- Lookahead precache scheduling/adoption (`maybe_precache_next`, `poll_adopted_precache`, `prepare_next_before_advance`)
- Position interpolation (`interpolated_position`, `tick_playback`), `mpv` event handling (`TrackEnded`, stale `PlaybackError`), and `ExportFingerprint` for `now_playing.json`.

Extracting a shared stateful `PlaybackSession` into `sonata-core::player` would eliminate divergence between the TUI and desktop engines.

### Remaining runtime & test items
1. **40 ms `DesktopSnapshot` cloning & sync `config.save()`**: [`crates/sonata-desktop/src/backend/mod.rs`](../crates/sonata-desktop/src/backend/mod.rs) still rebuilds `DesktopSnapshot` (cloning `config`, `themes`, shelves, tracks, queue, lyrics) every 40 ms during playback and runs synchronous `self.config.save()` on the Tokio thread. Split `PlaybackProgress` into a separate `watch` channel (or `Arc`-wrap large slices) and offload config saves via `spawn_blocking`.
2. **Blocking `StdCommand` & SQLite on async paths**: [`crates/sonata-core/src/hyprland.rs`](../crates/sonata-core/src/hyprland.rs) still calls synchronous `StdCommand::new("hyprctl")` without timeouts, and [`crates/sonata-core/src/cache.rs:213`](../crates/sonata-core/src/cache.rs#L213) runs synchronous SQLite queries + `VACUUM` under a `std::sync::Mutex`.
3. **Error typing**: `sonata-core` uses `anyhow::Result` throughout; introducing a typed core error enum would let frontends distinguish auth expiration from transient network errors.
4. **Test coverage**: `cargo test --offline` currently runs **67** unit tests in `sonata-core`, **17** in `sonata-desktop`, and **4** in `sonata-tui`. Expanding tests for `sonata-tui` input/event state transitions, `player/mpv.rs` IPC framing, and `backend/` command handling remains valuable.
