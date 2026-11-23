# Phase 0 Baseline Measurements (ADR-0001)

- **Snapshot Date:** 2026-11-23
- **Baseline Commit:** `91cbd73` (`main`)
- **Android Test Device:** Samsung Galaxy Tab S9 FE (`SM_X510`, serial `R52Y10CHC7K`, Android 16, `arm64-v8a`)
- **Build Toolchain:** Rust `1.99.0` (clippy), Android NDK `27.2.12479018`, JDK `21.0.13+9` (`/home/mehmetcan/.jdks/jdk-21.0.13+9`)

## 1. Test Suite Baseline

| Suite | Command | Result |
| --- | --- | --- |
| Rust formatting | `cargo fmt --all -- --check` | PASS |
| Rust workspace check | `cargo check --workspace --all-targets --message-format=short` | PASS |
| Rust workspace clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| Rust workspace tests | `cargo test --offline -p sonata-core -p sonata-desktop -p sonata-tui -p sonata-android` | PASS (`136` total: `sonata-core` 102 unit + 5 characterization integration, `sonata-android` 5, `sonata-desktop` 18, `sonata-tui` 6) |
| Android unit tests | `JAVA_HOME=/home/mehmetcan/.jdks/jdk-21.0.13+9 ./gradlew testDebugUnitTest --rerun-tasks --console=plain` | PASS (`19` total: `JioSaavnAndMediaTest` 4, `ShareAndChangelogTest` 3, `SonataDomainLogicTest` 6, `SonataThemeAndPreferencesTest` 6) |

### Environment / Known Baseline Notes

- Default system JVM is OpenJDK `27.0.1.1`, which fails Gradle configuration with `27.0.1.1`. Gradle commands must run with JDK 21 (`JAVA_HOME=/home/mehmetcan/.jdks/jdk-21.0.13+9`).
- No failing Rust or Android unit tests exist at baseline.

## 2. Dependency Reports

- `cargo tree -p sonata-core -e normal`: `395` tree lines, `20` direct dependencies:
  - `anyhow v1.0.104`, `base64 v0.22.1`, `dirs v6.0.0`, `flate2 v1.1.11`, `hmac v0.12.1`, `image v0.25.10`, `regex v1.13.2`, `reqwest v0.12.28`, `roxmltree v0.21.2`, `rusqlite v0.41.1`, `serde v1.0.229`, `serde_json v1.0.154`, `sha1 v0.10.7`, `tokio v1.54.1`, `tokio-util v0.7.20`, `toml v0.8.23`, `tracing v0.1.44`, `tracing-appender v0.2.5`, `tracing-subscriber v0.3.23`, `urlencoding v2.1.3`.
- `cargo tree -p sonata-android -e normal`: `520` tree lines.
- **Desktop/Linux crates currently leaking into `sonata-android` via `sonata-core`:**
  - `dirs v6.0.0`
  - `image v0.25.10` (plus `moxcms`, `png`, etc.)
  - `toml v0.8.23`
  - `tracing-appender v0.2.5`

## 3. Android Artifact Sizes (`91cbd73` Fresh Release Build)

| Artifact | Path | Baseline Size |
| --- | --- | --- |
| Stripped arm64 native library | `android/app/src/main/jniLibs/arm64-v8a/libsonata_android.so` | `7,923,992` bytes (`7.56 MiB`) |
| Release APK (`:app:assembleRelease`) | `android/app/build/outputs/apk/release/app-release.apk` | `18,967,404` bytes (`18.09 MiB`) |
| Debug APK (`:app:assembleDebug`) | `android/app/build/outputs/apk/debug/app-debug.apk` | `65,566,084` bytes (`62.53 MiB`) |

## 4. Real-Device Android Timings & StrictMode Findings

Measured on `R52Y10CHC7K` (Android 16, `arm64-v8a`) after upgrading existing `dev.sonata.android` (`1.0.3` -> `1.0.5` debug build with `FLAG_DEBUGGABLE`-gated `StrictMode.ThreadPolicy`):

- **Cold launch (`am start -W`):** `TotalTime = 2029 ms`, `WaitTime = 2034 ms` (`Displayed dev.sonata.android/.MainActivity: +2s29ms`).
- **Cipher WebView warm-up:** `cipher_webview loaded hash=7f755914 ok=true` at `+2.70 s` post-display.
- **Authenticated Extreme prefetch (`itag=774`):** `OPUS 274kbps · Extreme` (`+4.82 s`), `OPUS 260kbps · Extreme` (`+5.02 s`).
- **Pre-existing StrictMode main-thread violations (recorded, not normalized):**
  - `DiskReadViolation` (~1013 ms): `SonataPreferences.<init>(SonataPreferences.kt:131)` and `SonataPreferences.readState(SonataPreferences.kt:342)` invoked from `SonataViewModel.<init>(SonataViewModel.kt:355)`.
  - `DiskReadViolation` (~1000 ms): `SonataDownloadManager.<init>(DownloadManager.kt:82)` invoked from `SonataViewModel.<init>(SonataViewModel.kt:356)`.
  - Startup framework/disk reads & writes (5–10 ms) during process initialization.

## 5. Static Audit Counts (Pre-Migration)

- `cfg!(target_os = "android")` in `crates/sonata-core/src/providers/`: `10` (`hifi.rs`: 4, `hifi/innertube.rs`: 6).
- Process-wide policy/path statics in `crates/sonata-core/src/config.rs`: `4` (`ANDROID_DIRS`, `ACTIVE_CACHE_LIMIT_GB`, `ACTIVE_JIOSAAVN_ENABLED`, `ACTIVE_MOTION_COVERS`).
- `(std|tokio)::process::Command` in `crates/sonata-core/src/`: `7` sites across `motion.rs`, `media.rs`, `providers/hifi/cipher.rs`, `providers/hifi/ytdlp.rs`, `hyprland.rs`.

## 6. Characterization Coverage Added in Phase 0

Sanitized fixtures under `crates/sonata-core/tests/fixtures/`:
- `innertube_anonymous_fast.json`
- `innertube_authenticated_tvhtml5.json`
- `innertube_unavailable.json`

Named characterization tests:
- `providers::hifi::tests::characterizes_anonymous_fixture_bounded_fast_start_vs_direct_best_url`
- `providers::hifi::tests::characterizes_authenticated_tvhtml5_cipher_fixture_best_and_fast_start`
- `providers::hifi::tests::characterizes_unavailable_innertube_fixture_as_none`
- `providers::hifi::tests::characterizes_jiosaavn_upgrade_eligibility_gate`
- `providers::hifi::tests::characterizes_hifi_upgrade_need_for_local_and_remote_badges`
- `resolver_characterization::cached_first_stream_resolution_returns_local_audio_without_network`
- `resolver_characterization::cache_disabled_short_circuits_upgrade_and_precache`
- `resolver_characterization::local_cache_before_upgrade_delivers_higher_quality_and_skips_equal_quality`
- `resolver_characterization::precache_returns_existing_top_quality_cache_hit_offline`
- `resolver_characterization::same_track_replay_creates_new_generation_and_rejects_stale_result`

## 7. Final Post-Migration Comparison (Phases 0–8)

Verified after completing all roadmap phases (Milestones A–C, Phases 0–8) against baseline `91cbd73`:

| Metric | Phase 0 Baseline (`91cbd73`) | Phase 8 Final | Delta |
| --- | --- | --- | --- |
| Workspace crates | `4` | `5` (`+sonata-runtime`) | `+1` |
| `sonata-core` normal dep tree lines (`direct deps`) | `395` (`20`) | `331` (`16`) | `-64` (`-4`) |
| `sonata-android` normal dep tree lines | `520` | `458` | `-62` |
| Desktop/Linux crates leaking into `sonata-android` (`dirs`, `image`, `toml`, `tracing-appender`) | `4` | `0` | `-4` |
| `cfg!(target_os = "android")` in `sonata-core/src/providers/` | `10` | `0` | `-10` |
| Process-wide policy/path statics in `sonata-core` | `4` | `0` | `-4` |
| `(std\|tokio)::process::Command` in `sonata-core/src/` | `7` | `0` | `-7` |
| `Runtime::block_on` in `sonata-android/src/` | Blocking sync bridge | `0` (async UniFFI `0.29.5` Tokio) | Eliminated |
| Stripped arm64 `libsonata_android.so` | `7,923,992` B (`7.56 MiB`) | `7,863,896` B (`7.50 MiB`) | `-60,096` B |
| Android release APK (`app-release.apk`) | `18,967,404` B (`18.09 MiB`) | `18,940,076` B (`18.06 MiB`) | `-27,328` B |
| Linux release binaries (`sonata` / `sonata-desktop`) | N/A | `13.34 MiB` / `51.67 MiB` | Verified (`--release --locked`) |
| Rust workspace tests (`--offline`) | `136` | `170` (`core` 112, `runtime` 21, `desktop` 18, `tui` 10, `android` 9) | `+34` |
| Android JVM unit tests (`testDebugUnitTest`) | `19` | `38` | `+19` |
| Android native instrumented tests (`connectedDebugAndroidTest`) | `0` | `2` (`NativeEngineInstrumentedTest` on `SM-S731B`, Android 17) | `+2` |
| UniFFI Kotlin binding drift | Manual | `0` diff verified against `target/debug/libsonata_android.so` | Clean |

### Real-Device Hardware Proof (`R5GL50938JR`, Samsung `SM-S731B`, Android 17, `arm64-v8a`)

- **Cold launch:** `761 ms` (`0` crashes, `0` StrictMode warnings).
- **Cipher WebView warm-up:** `cipher_webview loaded hash=6b3d0364 ok=true`.
- **Rust UniFFI JioSaavn upgrade:** `OPUS 278kbps -> 320kbps AAC · JioSaavn` in `~720 ms`, `25.3 s` continuous playback & clean pause.
- **Native instrumented suite:** `NativeEngineInstrumentedTest` `2 / 2` PASS (`libsonata_android.so` load, `0700` private dir creation, typed invalid-cookie rejection, SQLite cache roundtrip, and UniFFI coroutine cancellation).

