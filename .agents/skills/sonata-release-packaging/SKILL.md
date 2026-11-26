---
name: sonata-release-packaging
description: >-
  Coordinate Sonata version bumps across workspace manifests, GitHub Actions release automation,
  Linux packaging desktop-ID parity, signed Android release APKs, and offline Flathub builds.
  Use when bumping Sonata versions, troubleshooting release.yml, or preparing Linux/Android/Flathub packages.
---

# Sonata Release & Packaging

Use this runbook when preparing a version bump, debugging `.github/workflows/release.yml`, building a signed Android release APK, or preparing a Flathub submission.

## 8-Way Version Sync (Desktop & TUI)

Update all eight locations together for Desktop/TUI releases (`X.Y.Z`):

1. `Cargo.toml` — `[workspace.package] version = "X.Y.Z"`
2. `PKGBUILD` — `pkgver=X.Y.Z`
3. `PKGBUILD.desktop` — `pkgver=X.Y.Z`
4. `flake.nix` — `version = "X.Y.Z"` in both `packages.sonata` and `packages.sonata-desktop`
5. `packaging/flatpak/dev.sonata.Desktop.metainfo.xml` — add `<release version="X.Y.Z" date="YYYY-MM-DD">`
6. `CHANGELOG.md` — move items from `## [Unreleased]` under `## [X.Y.Z] - YYYY-MM-DD`
7. `crates/sonata-desktop/tauri.conf.json` — `"version": "X.Y.Z"`
8. `desktop-ui/package.json` — `"version": "X.Y.Z"`

For Android version bumps, also update `appVersionName` fallback in `android/app/build.gradle.kts` (`val appVersionName = ... ?: "X.Y.Z"`) and fallback `appVersionCode` (`MAJOR * 10000 + MINOR * 100 + PATCH`).

## `.github/workflows/release.yml` Behavior

- Triggers on `push` to `main` and `workflow_dispatch`.
- Reads `DV` from `Cargo.toml` (`^version = `) and `AV` from `android/app/build.gradle.kts` (`appVersionName`), deriving `AV_CODE`.
- **4-platform asset-skip guard**: Checks GitHub Release `v${DV}` for Linux (`Sonata-v${DV}-x86_64.AppImage`), Windows (`Sonata-v${DV}-windows-x64.exe`), and macOS (`Sonata-v${DV}-macos-arm64.dmg`), plus `v${AV}` for Android (`Sonata-v${AV}-arm64.apk`). Bump the version before pushing to `main` if you expect fresh release artifacts.
- **Release notes & artifact publishing**: `publish` extracts `CHANGELOG.md` lines between `## [VER]` and the next `## [` via `awk` (falling back to `--generate-notes`), uploads and hashes all 11 platform binaries (`.AppImage`, `.zsync`, `.deb`, `.rpm`, `.tar.gz`, `-windows-x64.exe`, `-windows-x64-setup.exe`, `-windows-x64.msi`, `-macos-arm64.dmg`, `-macos-arm64.app.tar.gz`, `-arm64.apk`) into `SHA256SUMS.txt`, and asserts expected artifacts exist.
- **Workflow hardening & zero-warning CI hygiene**:
  - Keep root `permissions: contents: read` (write only on `publish`), pin all `uses:` actions to 40-char commit SHAs, and pin `appimagetool` to an immutable release tag + verified `APPIMAGETOOL_SHA256` (never `continuous`).
  - Use `macos-latest` (never deprecated `macos-14`).
  - Pin `actions/setup-node` to `v7.1.0`+ to avoid Node 20 deprecation warnings.
  - On Ubuntu AppImage jobs, install `libglib2.0-dev` and WebKitGTK build dependencies directly via `apt-get` (avoid `awalsh128/cache-apt-pkgs-action`, which can leave `glib-2.0.pc` missing).
  - On Android NDK jobs, export both `ANDROID_NDK_HOME` and `ANDROID_NDK_ROOT`, and never pass root-owned absolute paths like `/usr/local/lib/android/sdk` to `actions/cache` (`tar` permission/leading-slash warnings).
  - Keep Vite `manualChunks` configured in `desktop-ui/vite.config.ts` so every JS bundle stays under Vite's 500 kB warning limit.
  - Before pushing a release, audit Windows/macOS `#[cfg]` code paths (`mpris.rs`, `tray.rs`, `hyprland.rs`) so cross-platform `-D warnings` jobs pass cleanly; after pushing, inspect GitHub Actions logs for both errors and warnings.

## Desktop-ID & Icon Divergence Across Linux Packages

Keep launcher and icon changes aligned across all four packaging targets:

| Target | Desktop File | Icon Name / Source |
|---|---|---|
| `.deb`, `.rpm`, `PKGBUILD.desktop`, `flake.nix` | `assets/sonata-desktop.desktop` | `Icon=sonata-desktop` (`assets/sonata-desktop.svg`) |
| Flatpak (`packaging/flatpak/dev.sonata.Desktop.yml`) | `dev.sonata.Desktop.desktop` (from `assets/sonata-desktop.desktop`) | `sed` rewrites `Icon=sonata-desktop` to `Icon=dev.sonata.Desktop` (`assets/sonata-icon.png`, `assets/sonata-desktop.svg`) |
| AppImage (`.github/workflows/release.yml`) | Inline `dev.sonata.Sonata.desktop` + `dev.sonata.Sonata.appdata.xml` | `Icon=sonata` (`rsvg-convert` 256px + SVG from `assets/sonata-logo.svg`) |

## Signed Android Release APK

1. Ensure the native `arm64-v8a` library is current:
   ```sh
   cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p sonata-android --release --locked
   ```
2. Provide signing credentials via `ANDROID_KEYSTORE_PATH`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` (or `~/.config/sonata/keystore.properties`; CI decodes `ANDROID_KEYSTORE_BASE64`).
   Both `.github/workflows/release.yml` and `android/app/build.gradle.kts` fail fast if release signing is missing (use `-PallowUnsignedRelease=true` only for explicit local unsigned test builds).
3. Build the reproducible FOSS APK (`dependenciesInfo` disabled in `android/app/build.gradle.kts`):
   ```sh
   cd android && ./gradlew :app:assembleRelease \
     -PsonataVersion="${AV}" \
     -PsonataVersionCode="${AV_CODE}" \
     --console=plain
   ```
   Output path: `android/app/build/outputs/apk/release/app-release.apk`.

## Flathub Offline Prep & Validation

`packaging/flatpak/dev.sonata.Desktop.yml` builds Tauri on `org.gnome.Platform//47` with `org.freedesktop.Sdk.Extension.rust-stable` and `org.freedesktop.Sdk.Extension.node22` (and requires `desktop-ui` offline node/cargo prep before sandboxed builds).

1. Generate offline Cargo sources from `Cargo.lock` (requires `flatpak-cargo-generator.py` in `PATH` or `FLATPAK_CARGO_GENERATOR`):
   ```sh
   ./scripts/generate-flatpak-cargo-sources.sh packaging/flatpak/cargo-sources.json
   ```
2. For Flathub PRs, replace `type: dir` (`path: ../..`) in `packaging/flatpak/dev.sonata.Desktop.yml` with the tagged GitHub release tarball/commit + `sha256` and include `- cargo-sources.json`.
3. Validate metadata, offline build, and Flathub lint rules:
   ```sh
   appstreamcli validate --pedantic packaging/flatpak/dev.sonata.Desktop.metainfo.xml
   desktop-file-validate assets/sonata-desktop.desktop
   ! grep -q "0000000000000000" packaging/flatpak/dev.sonata.Desktop.yml
   flatpak-builder --force-clean --sandbox --user --install-deps-from=flathub --repo=repo builddir packaging/flatpak/dev.sonata.Desktop.yml
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/dev.sonata.Desktop.yml
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
   ```
