---
name: sonata-uniffi-android
description: >-
  Develop and verify Sonata's UniFFI Rust-to-Kotlin bridge, native arm64-v8a library,
  R8/Compose rules, Media3 PlaybackOrchestrator invariants, and Android test suites.
  Use when editing crates/sonata-android, UniFFI bindings, SonataEngineGateway, or Android playback.
---

# Sonata UniFFI & Android Verification

Use this runbook when modifying `crates/sonata-android/src/lib.rs`, Kotlin FFI bindings, R8/Compose configuration, or Android playback orchestration.

## UniFFI Bridge Rules (`crates/sonata-android/src/lib.rs`)

- **Async runtime**: Keep `uniffi = { version = "0.29", features = ["cli", "tokio"] }` (`cdylib` + `rlib`) and expose network/resolver/disk/SQLite methods with `#[uniffi::export(async_runtime = "tokio")]`. Never introduce `Runtime::block_on`; offload synchronous disk/SQLite calls via `tokio::task::spawn_blocking`.
- **Paths & permissions**: Construct storage roots via `CorePaths::for_app_roots(config_dir, cache_dir)` and persist sensitive files with `0o600` inside `0o700` directories.
- **Kotlin gateway**: Keep `android/app/src/main/java/dev/sonata/android/data/SonataEngineGateway.kt` cancellation-safe via `runSuspendCatching` so `CancellationException` rethrows immediately to cancel the underlying UniFFI future.

## Regenerate Kotlin Bindings & Check CI Drift

Whenever `crates/sonata-android/src/lib.rs` exports, records, errors, or callback interfaces change:

```sh
cargo build --locked -p sonata-android --lib
cargo run --locked -p sonata-android --bin uniffi-bindgen -- generate \
  --no-format \
  --library target/debug/libsonata_android.so \
  --language kotlin \
  --out-dir /tmp/uniffi-check
diff -u android/app/src/main/java/uniffi/sonata_android/sonata_android.kt \
  /tmp/uniffi-check/uniffi/sonata_android/sonata_android.kt
```

To update the checked-in binding file directly (keep `--no-format` so CI `diff -u` matches):

```sh
cargo run --locked -p sonata-android --bin uniffi-bindgen -- generate \
  --no-format \
  --library target/debug/libsonata_android.so \
  --language kotlin \
  --out-dir android/app/src/main/java
```

## Rebuild Native Android Library (`arm64-v8a`)

Using Android NDK `27.2.12479018` (`minSdk = 31`):

```sh
cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p sonata-android --release --locked
```

## Compose Stability & R8 Keep Rules

- `android/app/compose_compiler_config.conf` marks `uniffi.sonata_android.*` and `dev.sonata.android.data.*` stable because UniFFI emits `var` properties on records that Sonata treats as immutable snapshots.
- `android/app/proguard-rules.pro` keeps `com.sun.jna.**`, `uniffi.sonata_android.**`, and `smbj` (`com.hierynomus.**`, `net.engio.mbassy.**`, `org.bouncycastle.crypto.**`). Do not re-add redundant `kotlinx.coroutines` consumer rules or unused `WebViewJsCipherSolver`/`@JavascriptInterface` rules.
- `android/app/src/main/res/xml/network_security_config.xml` is the single source of truth (no `usesCleartextTraffic="true"` in manifest): public API/auth/update/streaming domains enforce `cleartextTrafficPermitted="false"`, while LAN self-hosted IPs remain supported.

## `PlaybackOrchestrator` Invariants

Preserve all five contracts in `android/app/src/main/java/dev/sonata/android/playback/PlaybackOrchestrator.kt`:

1. `PlaybackGenerationCounter`: Share one monotonic generation across play, stream resolve, recovery, upgrade, lyrics, canvas, and prefetch; reject stale callbacks with `PlaybackEligibility.canAcceptGenerationResult`.
2. `RecoveryBudget`: Keep retry attempts (`DEFAULT_MAX_ATTEMPTS = 2`) keyed by generation so replays start fresh and stale errors never consume active attempts.
3. `RateLimitPolicy`: Classify rate limits via typed `SonataAndroidException.RateLimited` (`retryAfterSecs`), never error-message substring matching.
4. `EndedStateTransitionGuard`: Report `Player.STATE_BUFFERING` on `Player.STATE_ENDED` only while a next-track transition is pending; clear on `STATE_READY`, `STATE_IDLE`, pause (`playWhenReady == false`), stop, or end-of-queue.
5. `performAtomicStreamSwap`: Verify `isStillCurrent()`, snapshot position and play intent, execute `replaceMediaItem` -> `prepare()` -> `play()`/`pause()`, and invoke `onCommitted` only after the swap succeeds.

## Verification Commands

Gradle 9.6.0 + AGP 9.5.3 (built-in Kotlin 2.2.10, `sdklib 32.5.3` with native SDK XML v4 support) requires JDK 21 on hosts defaulting to newer JDKs (e.g. Java 27). Ensure `JAVA_HOME` points to JDK 21 before running Gradle:

```sh
./scripts/check_boundaries.sh
cargo check-android
cargo test-android
cd android && ./gradlew testDebugUnitTest --console=plain
```

## Build & Install Signed Release APK via ADB

```sh
ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/27.2.12479018" \
  cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p sonata-android --release --locked
(cd android && JAVA_HOME="$HOME/.jdks/jdk-21.0.13+9" ./gradlew :app:assembleRelease --console=plain)
adb -s <serial> install -r -d android/app/build/outputs/apk/release/app-release.apk
```

When an Android device or arm64 emulator is attached, also run:

```sh
cd android && ./gradlew connectedDebugAndroidTest --console=plain
```
