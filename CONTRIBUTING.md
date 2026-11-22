# Contributing to Sonata

Thank you for your interest in contributing to **Sonata**!

## Project Expectations

- **Primary language**: All code, comments, module headers, documentation, UI strings, and logs must be written in **English**.
- **Workspace boundaries**:
  - Shared engine logic belongs in `crates/sonata-core` (no UI dependencies).
  - Terminal client lives in `crates/sonata-tui` (`sonata`).
  - Wayland desktop client lives in `crates/sonata-desktop` (`sonata-desktop`).
  - UniFFI Android bridge lives in `crates/sonata-android`, and the Kotlin/Jetpack Compose client lives in `android/`.
- **Module headers**: Every Rust source file (`*.rs`) must begin with a `//!` overview and numbered component breakdown (see [AGENTS.md](AGENTS.md)).
- **Theme & UI parity (Android)**: Keep both **`Violet`** and **`Moonflower`** themes (Light, Dark, and AMOLED modes) structurally distinct and verify translucent **Liquid Glass** chrome across portrait and landscape form factors.
- **Security**: Create cookie/auth files with `0o600` permissions, private cache/runtime directories with `0o700`, keep secrets out of settings backups, and scrub URLs/tokens through `sonata_core::redaction` before logging.

## Development Setup

### Rust Workspace (Desktop, TUI & Core)

Ensure you have the Rust stable toolchain (`rustfmt` and `clippy` components), `mpv`, `yt-dlp`, `ffmpeg`, `nodejs`, and a C compiler installed. For `sonata-desktop`, you also need Wayland, Vulkan/EGL, `fontconfig`, `libxkbcommon`, `libxcb`, ALSA, and D-Bus development libraries.

```sh
git clone https://github.com/MehmetCanWT/sonata.git
cd sonata

# Fast single-client checks (.cargo/config.toml aliases)
cargo check-tui
cargo check-desktop
cargo check-android
cargo test-android
```

### Android Client (`android/`)

Building and testing the Android app locally requires **JDK 21**, **Android SDK 35**, **NDK `27.2.12479018`**, `aarch64-linux-android` Rust target, and `cargo-ndk`:

```sh
rustup target add aarch64-linux-android
cargo ndk -t arm64-v8a -P 26 -o android/app/src/main/jniLibs build -p sonata-android --release --locked

cd android
./gradlew testDebugUnitTest --console=plain
./gradlew :app:assembleDebug --console=plain
```

> [!NOTE]
> If you change `#[uniffi::export]` APIs or records in `crates/sonata-android/src/lib.rs`, regenerate Kotlin bindings from an unstripped debug cdylib (`cargo build -p sonata-android --locked`) into `android/app/src/main/java/uniffi/sonata_android/sonata_android.kt` before building Android.

## Verification Before Opening a Pull Request

All pull requests must pass formatting, workspace checks, Clippy with zero warnings, and all unit tests (Rust + Android JVM when touching `android/` or `crates/sonata-android`):

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --message-format=short
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline -p sonata-core -p sonata-desktop -p sonata-tui -p sonata-android
cd android && ./gradlew testDebugUnitTest --console=plain
```

## Commit & Pull Request Guidelines

- Follow **Conventional Commits**:
  - `feat(core): ...`, `fix(android): ...`, `perf(desktop): ...`, `docs: ...`, `refactor: ...`, `test: ...`
- Update [CHANGELOG.md](CHANGELOG.md) under `## [Unreleased]` for user-visible changes.
- Add unit tests in the relevant crate (`#[cfg(test)]`) or `android/app/src/test/java/dev/sonata/android/` for bug fixes, parsers, theme tokens, and state helpers.
- Keep pull requests focused on a single concern so they remain easy to review.
