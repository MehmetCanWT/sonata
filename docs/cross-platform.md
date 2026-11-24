# Sonata Cross-Platform Architecture & Porting Plan

Sonata separates its portable Rust engine (`sonata-core`) from OS-specific runtime adapters (`sonata-runtime`) and platform UI shells (`sonata-desktop`, `sonata-tui`, and `android/`). This layout allows Android, Linux, macOS, and Windows clients to share one networking, provider, caching, lyrics, scrobbling, and playback-state core without duplicating business logic.

## 1. Why `sonata-core` Is Already OS-Portable

Enforced automatically by [`scripts/check_boundaries.sh`](../scripts/check_boundaries.sh), `crates/sonata-core` maintains strict platform neutrality:

- **Zero UI or FFI dependencies**: `sonata-core` never depends on `ratatui`, `crossterm`, `gpui-kit`, `uniffi`, or `sonata-runtime`.
- **Zero subprocesses**: `std::process::Command` and `tokio::process::Command` are forbidden inside `crates/sonata-core/src`.
- **Zero OS-policy branching in providers**: Provider modules use explicit configuration rather than `cfg!(target_os = ...)` checks.
- **Injected paths (`CorePaths`)**: [`crates/sonata-core/src/paths.rs`](../crates/sonata-core/src/paths.rs) accepts client-resolved `config_dir` and `cache_dir` roots without calling `dirs` or assuming XDG conventions:
  - **Linux**: `~/.config/sonata` and `~/.cache/sonata` (or `$XDG_CONFIG_HOME` / `$XDG_CACHE_HOME`)
  - **Android**: app-private `filesDir/sonata` and `cacheDir/sonata`
  - **macOS (planned)**: `~/Library/Application Support/sonata` and `~/Library/Caches/sonata`
  - **Windows (planned)**: `%APPDATA%\sonata` and `%LOCALAPPDATA%\sonata`
- **Explicit stream policy (`ResolverProfile`)**: [`crates/sonata-core/src/providers/profile.rs`](../crates/sonata-core/src/providers/profile.rs) parameterizes `FastStartMode` (`BoundedRange` vs `DirectUrl`), `UpgradeDelivery` (`CacheBeforeReturn` vs `ReturnRemoteStream`), `StreamFallbackMode` (`InnerTubeOnly` vs `ExternalTool`), `bootstrap_timeout`, `jiosaavn_enabled`, and `cache_limit_bytes`.
- **Portable pure-Rust subsystems**:
  - **Providers**: YouTube Music InnerTube (`providers/ytmusic/`), Spotify GraphQL/embed (`providers/spotify/`), JioSaavn DES-ECB (`providers/jiosaavn.rs`), Hi-Fi `TVHTML5` + zero-AST `base.js` cipher hook (`providers/hifi/`), and self-hosted Subsonic/Jellyfin/SMB (`providers/servers/`).
  - **Storage & State**: SQLite WAL metadata cache (`cache.rs`, `cache/media.rs`), `StatsStore` (`stats.rs`), `ScrobbleClient` (`scrobble.rs`), `ConnectMessage` + HMAC-SHA1 PIN pairing (`connect.rs`), `MotionMedia` discovery (`motion.rs`), multi-provider word-synced lyrics (`lyrics/`), `PlaybackCoordinator` / `PlaybackGeneration` / `PlaybackTaskSet` / `NextTrackPrecache` (`player/`), `redaction.rs`, and `updater.rs`.

## 2. Thin `sonata-runtime` Platform Shim Plan

`crates/sonata-runtime` already isolates OS-specific integrations behind clean traits and focused modules (`PlaybackAudioDriver`, `PlaybackStreamSource`, `auth/browser.rs`, `config.rs`, `discord.rs`, `connect.rs`). Extending desktop/TUI support to macOS and Windows requires adding `#[cfg(target_os = ...)]` backend shims in `sonata-runtime` and window-chrome hooks in `sonata-desktop` while leaving `sonata-core` untouched.

### 2.1 Shared Trait Seams in `sonata-runtime`

| Subsystem | Current Linux Implementation | Cross-Platform Seam |
|---|---|---|
| Audio playback | `MpvController` (`player/mpv.rs`) via Unix JSON-IPC socket | `PlaybackAudioDriver` trait (`player/session/mod.rs`) |
| Stream fallback & JS cipher | `LinuxStreamResolver` (`process/ytdlp.rs`) & `CliJsCipherBridge` (`process/cipher.rs`) | `PlaybackStreamSource` trait + `ResolverProfile` |
| Secret & cookie persistence | `0o600`/`0o700` files in `auth/cookies.rs` + `auth/browser.rs` | OS keychain / credential store shim + browser DB reader |
| Media keys & OS integration | `mpris.rs` (`mpris-server` D-Bus) & `tray.rs` (`ksni` StatusNotifierItem) | Platform media-session & tray adapters in `sonata-desktop/src/integrations/` |
| Discord Rich Presence | Unix domain sockets `$XDG_RUNTIME_DIR/discord-ipc-*` (`discord.rs`) | Transport enum: Unix socket (Linux/macOS) vs named pipe (Windows) |

### 2.2 macOS Shim Plan (`target_os = "macos"`)

1. **Audio Output (CoreAudio & AVFoundation)**:
   - Implement a native `CoreAudioDriver` / `AvFoundationDriver` behind `PlaybackAudioDriver`, or use `mpv --ao=coreaudio` over a temporary Unix domain socket in `$TMPDIR` as an interim driver.
   - Native path: decode via `AVAssetReader` / AudioToolbox (`AudioUnit` HAL output) or `AVQueuePlayer` for sample-accurate gapless transitions, software crossfade gain ramps (`0..=12` s), and bit-transparent sample-rate switching when exclusive hog mode is enabled.
   - Integrate `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` (via `objc2-media-player`) to expose artwork, elapsed time, and play/pause/seek commands to macOS Control Center and media keys.
2. **Credential Storage (Keychain) & Browser Cookies**:
   - Store YouTube Music `SAPISID` cookies, Spotify `sp_dc`, and Last.fm / ListenBrainz API secrets in macOS Keychain Services (`SecItemAdd`, `SecItemCopyMatching`, `SecItemUpdate`) under service `dev.sonata.Desktop`.
   - Extend `auth/browser.rs` to probe macOS browser profiles (`~/Library/Application Support/Google/Chrome`, `Firefox`, `Arc`, `BraveSoftware`, and `~/Library/Cookies/Cookies.binarycookies` when Full Disk Access is granted).
3. **Window Vibrancy (`NSWindow`)**:
   - In `sonata-desktop`, configure `gpui-kit` macOS window options with `titlebarAppearsTransparent = true`, `NSVisualEffectView` (`NSVisualEffectMaterial::UnderWindowBackground` / `HudWindow` controlled by `DesktopSettings.opacity`), native traffic-light padding in `views/chrome.rs`, and `NSAppearance` synchronization for Light/Dark mode.
4. **Packaging & Updates**:
   - Bundle `sonata-desktop` and `sonata` into `Sonata.app` (universal `aarch64-apple-darwin` + `x86_64-apple-darwin` `.dmg`), notarized via `codesign` + `notarytool`, with `sonata_core::updater` targeting the `.dmg` / `.tar.gz` GitHub Release assets.

### 2.3 Windows Shim Plan (`target_os = "windows"`)

1. **Audio Output (WASAPI) & Media Controls**:
   - Implement `WasapiAudioDriver` behind `PlaybackAudioDriver` using Windows Audio Session API (`IAudioClient` / `IAudioRenderClient` in shared or exclusive event-driven mode), or `mpv --ao=wasapi` over a Windows named pipe (`\\.\pipe\sonata-mpv-<pid>`) as an interim backend.
   - Integrate `Windows.Media.SystemMediaTransportControls` (SMTC) in `sonata-desktop/src/integrations/` for the Windows volume flyout, lock screen, and hardware media keys.
2. **Credential Manager & Restricted File ACLs**:
   - Store session tokens and scrobbling secrets in Windows Credential Manager (`CredWriteW` / `CredReadW` / `CredDeleteW` via `windows` crate), or encrypt local fallback files via DPAPI (`CryptProtectData`) with explicit user-only NTFS DACLs (`LocalAlloc` + `SetNamedSecurityInfoW`) wherever Unix `0o600`/`0o700` modes do not apply.
   - Extend `auth/browser.rs` to locate Chromium/Edge/Firefox profiles under `%LOCALAPPDATA%` and `%APPDATA%` (handling Chromium App-Bound Encryption via DPAPI/COM elevation or WebView2 login fallback).
3. **Window Material (Mica), Taskbar Identity (`AppUserModelID`), & IPC**:
   - On Windows 11 (`>= 22621`), call `DwmSetWindowAttribute` with `DWMWA_SYSTEMBACKDROP_TYPE` (`DWMSBT_MAINWINDOW` for Mica or `DWMSBT_TRANSIENTWINDOW` for Acrylic) and `DWMWA_USE_IMMERSIVE_DARK_MODE` to match `Sonata Glass` translucency.
   - Call `SetCurrentProcessExplicitAppUserModelID("dev.sonata.Desktop")` at startup in `sonata-desktop/src/main.rs` so taskbar pinning, Start Menu shortcuts, SMTC attribution, and Windows toast notifications bind to a single identity.
   - Implement a `Shell_NotifyIconW` tray adapter alongside `ksni`, and connect `sonata-runtime/src/discord.rs` to Discord's Windows named pipes (`\\.\pipe\discord-ipc-0`..`9`).
4. **Packaging**:
   - Package `sonata-desktop.exe` and `sonata.exe` via WiX / MSIX installer or portable `.zip` on `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`.

## 3. Verification Strategy for New Targets

- Keep `sonata-core` 100% host-testable (`cargo test -p sonata-core`) with zero OS conditionals in provider/business logic.
- Run `cargo check --workspace --target <triple>` in CI matrix jobs (`x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`) once `#[cfg(target_os = "macos")]` and `#[cfg(target_os = "windows")]` runtime modules are introduced.
- Keep `PlaybackAudioDriver` and `PlaybackStreamSource` deterministic test doubles (`RecordingDriver`, `StaticSource` in `sonata-runtime/src/player/session/tests.rs`) shared across all OS targets so state-machine invariants remain verified on every platform.
