---
name: sonata-verify-and-boundaries
description: >-
  Run Sonata's pre-merge Rust verification pipeline, crate dependency boundary checks,
  module documentation checks, desktop metadata validation, and credential hygiene audits.
  Use when verifying Rust, desktop, or cross-crate changes in Sonata before committing or merging.
---

# Sonata Verification & Boundaries

Execute Sonata's exact pre-merge quality, architecture, and security checks.

## Fast Client Iteration

Use `.cargo/config.toml` aliases (`jobs = 10`) while iterating on a single client:

```sh
cargo check-tui
cargo check-desktop
cargo check-android
cargo test-android
```

## Pre-Edit Hygiene Checks

For every touched Rust or Android file, verify:

1. **Module docs**: Every `*.rs` file begins with a structured `//!` header containing a 1–2 sentence overview and a numbered `//! 1. **Component**: ...` breakdown.
2. **Secret file permissions**: Create cookie/session/credential files with `OpenOptionsExt::mode(0o600)` and private directories with `0o700` on Unix before writing secrets—never write with default permissions and `chmod` afterward.
3. **Log & error redaction**: Pass URLs, auth headers, tokens, and JSON payloads through `sonata_core::redaction` (`stream_url`, `sensitive_text`, `json_payload`) before logging or returning errors.
4. **Blocking I/O offloading**: Never run `std::process::Command` or synchronous file/SQLite work on async or UI threads without `tokio::task::spawn_blocking` (or `Dispatchers.IO` on Android).

## Crate Boundary Gate (`./scripts/check_boundaries.sh`)

Run `./scripts/check_boundaries.sh` before full workspace checks. Its 7 checks enforce:

1. No `(std|tokio)::process::Command` in `crates/sonata-core/src`.
2. No `cfg!(target_os = "android")` in `crates/sonata-core/src/providers` (use `ResolverProfile`).
3. No `sonata-runtime` references in `crates/sonata-core` or `crates/sonata-android`.
4. `cargo tree -p sonata-android -e normal` pulls none of `image`, `dirs`, `toml v0.8`, or `tracing-appender`.
5. No `block_on` in `crates/sonata-android/src`.
6. `cargo metadata` DAG rules: `sonata-core` forbids `sonata-runtime`, `sonata-tui`, `sonata-desktop`, `sonata-android`, `ratatui`, `crossterm`, `tauri`, `uniffi`; `sonata-android` depends on `sonata-core` (never `sonata-runtime`); `sonata-tui` and `sonata-desktop` depend on both `sonata-core` and `sonata-runtime`; `sonata-runtime` depends on `sonata-core`.
7. Consolidated protocols stay removed (`crates/sonata-desktop/src/backend/ytm_extra.rs`, `android/app/src/main/java/dev/sonata/android/data/JioSaavnSource.kt`, and Android Kotlin constants `APPLE_AMP_BASE`, `TIDAL_SEARCH_URL`, `COMMUNITY_MANIFEST_URL`, `obtainAppleJwt`).

## Full Pre-Merge Gate

Run in order after the final Rust edit:

```sh
./scripts/check_boundaries.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline -p sonata-core -p sonata-runtime -p sonata-desktop -p sonata-tui -p sonata-android
```

Note: `.github/workflows/ci.yml` runs `cargo test --workspace --all-targets --locked` and `cargo audit`.

## Desktop Frontend Gate (`desktop-ui/`)

Whenever modifying `desktop-ui/` or `crates/sonata-desktop/`, run the frontend checks before rebuilding `sonata-desktop` (Tauri embeds `desktop-ui/dist/` at compile time):

```sh
cd desktop-ui && npm run typecheck && npm run build && npm test
```

## Desktop Metadata & Supply-Chain Checks

Run whenever editing `.desktop`, AppStream, or Flatpak files, or updating `Cargo.lock`:

```sh
appstreamcli validate --pedantic packaging/flatpak/dev.sonata.Desktop.metainfo.xml
desktop-file-validate assets/sonata-desktop.desktop
! grep -q "0000000000000000" packaging/flatpak/dev.sonata.Desktop.yml
cargo audit
```
