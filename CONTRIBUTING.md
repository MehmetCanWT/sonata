# Contributing to Sonata

Thank you for your interest in contributing to **Sonata**!

## Project Expectations

- **Primary language**: All code, comments, module headers, documentation, UI text, and logs must be in **English**.
- **Crate boundaries**: Shared logic belongs in `crates/sonata-core` (no UI dependencies). Terminal UI belongs in `crates/sonata-tui`, and experimental Wayland GUI code belongs in `crates/sonata-desktop`.
- **Module headers**: Every `*.rs` file should start with a `//!` overview and numbered component breakdown (see [CLAUDE.md](CLAUDE.md)).
- **Security**: Keep cookie/auth files owner-only (`0o600`), private directories `0o700`, and pass URLs/tokens through `sonata_core::redaction` before logging.

## Development Setup

Ensure you have the Rust stable toolchain (`rustfmt` and `clippy` components), `mpv`, `yt-dlp`, `nodejs`, and a C compiler installed. For `sonata-desktop`, you will also need Wayland, Vulkan/EGL, `fontconfig`, `libxkbcommon`, ALSA, and D-Bus development libraries.

```sh
git clone https://github.com/MehmetCanWT/sonata.git
cd sonata

# Fast single-client checks
cargo check-tui
cargo check-desktop
```

## Verification Before Opening a Pull Request

All pull requests must pass formatting, workspace checks, Clippy with zero warnings, and unit tests:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --message-format=short -- -D warnings
cargo test --offline -p sonata-core -p sonata-desktop -p sonata-tui
```

## Commit & Pull Request Guidelines

- Follow **Conventional Commits**:
  - `feat(core): ...`, `fix(tui): ...`, `perf(desktop): ...`, `docs: ...`, `refactor: ...`, `test: ...`
- Update [CHANGELOG.md](CHANGELOG.md) under `## [Unreleased]` for user-visible changes.
- Include unit tests for parsers, state transitions, and bug fixes whenever feasible.
- Keep pull requests focused on a single concern so they are easy to review.
