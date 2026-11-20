# CLAUDE.md

Guidance for AI coding agents and contributors working in the **Sonata** repository.

## Workspace Layout

Sonata is a 4-crate Cargo workspace (`resolver = "2"`):

- `crates/sonata-core` — Shared Rust engine (no UI dependencies):
  - `auth/` — Browser cookie extraction & session auth (`cookies.rs`)
  - `cache/` & `cache.rs` — SQLite WAL metadata cache & bounded LRU media cache
  - `lyrics/` — Multi-source lyrics resolution (TTML, KuGou KRC, NetEase YRC, LRCLIB, YouTube Music, Genius)
  - `player/` — `mpv` JSON-IPC driver, playback coordinator, Hi-Fi upgrade & lookahead precache
  - `providers/` — Music providers (`providers/ytmusic/`, `providers/spotify/`, Hi-Fi stream resolution)
  - `media.rs` — High-res artwork lookup & URL upscaling
  - `config.rs`, `theme.rs`, `hyprland.rs`, `redaction.rs`, `telemetry.rs`
- `crates/sonata-tui` — **`sonata`** (stable Ratatui terminal client):
  - `src/main.rs`, `src/input.rs`, `src/theme_tui.rs`
  - `src/app/` (`mod.rs`, `state.rs`, `playback.rs`, `library.rs`)
  - `src/ui/` — Terminal panes, modals, artwork, and synced lyrics rendering
- `crates/sonata-desktop` — **`sonata-desktop`** (experimental GPUI Wayland desktop client):
  - `src/main.rs`, `src/theme_bridge.rs`
  - `src/backend/` (`mod.rs`, `types.rs`, `playback.rs`, `library.rs`)
  - `src/views/` (`mod.rs`, `root/` (`mod.rs`, `header.rs`, `sidebar.rs`, `stage.rs`, `player_bar.rs`), `fullscreen/` (`mod.rs`, `sheet.rs`, `controls.rs`), `fullscreen_fx/` (`mod.rs`, `palette.rs`, `waves.rs`, `blur_cache.rs`))
  - `src/integrations/` — MPRIS2 (`mpris.rs`) and StatusNotifierItem system tray (`tray.rs`)
- `crates/sonata-android` — Reserved placeholder crate for a future Android client.

## Strict Dependency Boundaries

- `sonata-tui` and `sonata-desktop` depend on `sonata-core`.
- `sonata-core` must **never** depend on `sonata-tui`, `sonata-desktop`, `ratatui`, `crossterm`, or `gpui-kit`.
- Workspace-wide dependencies live in root `Cargo.toml` under `[workspace.dependencies]`; add crate dependencies via `<dep>.workspace = true`.
- Workspace lints (`unsafe_op_in_unsafe_fn = "deny"`, `dbg_macro = "deny"`, `todo = "deny"`, `undocumented_unsafe_blocks = "deny"`) apply to all workspace crates.

## Module Documentation Standard

Every Rust source module (`*.rs`) must begin with a structured `//!` module doc comment containing:

1. A concise 1–2 sentence overview of what the module owns.
2. A numbered breakdown (`//! 1. **Component**: ...`) describing its primary responsibilities:

```rust
//! Brief 1–2 sentence module overview.
//!
//! 1. **Component**: Responsibility summary.
//! 2. **Pipeline**: Responsibility summary.
```

All code comments, docstrings, documentation files, UI strings, and logs must be written in English.

## Required Verification Checks

Run these checks before finishing any Rust change:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --message-format=short
cargo test --offline -p sonata-core -p sonata-desktop -p sonata-tui
```

For fast single-client iteration during development, use the `.cargo/config.toml` aliases (`cargo check-tui`, `cargo check-desktop`, `cargo check-android`).

**Git policy**: Never run `git push` or rewrite shared remote history unless the user explicitly asks you to.

## Security & Sensitive Data

- **Cookie files**: Persist extracted browser cookies (such as `~/.cache/sonata/yt_cookies.txt`) using atomic/restricted creation (`0o600` file mode on Unix before writing secrets) — never write with default permissions and `chmod` afterward.
- **Temporary & runtime directories**: Create private cache/runtime directories with `0o700` permissions on Unix.
- **Log & error redaction**: Always pass URLs, cookie headers, and auth tokens through `sonata_core::redaction` before logging (`~/.cache/sonata/sonata.log`) or surfacing errors. Never log raw `Cookie`, `Authorization`, `SAPISID`, or `sp_dc` values.
- **Blocking I/O**: Never call blocking `std::process::Command` or synchronous file/SQLite operations on async/UI threads without bounding or offloading via `tokio::task::spawn_blocking`.

## Commits, PRs & Changelog

- Use **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, `perf:`, `test:`, `chore:`), optionally scoped by crate (e.g., `feat(core): add TTML dual-voice lyrics parser`).
- Keep pull requests focused, small enough to review cleanly, and backed by tests for bug fixes or new parsers.
- Record user-facing changes under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) following the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.
