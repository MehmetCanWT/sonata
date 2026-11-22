<div align="center">

<br/>

<img src="assets/sonata-icon.png" alt="Sonata logo" width="168" />

# Sonata

### Aesthetic Cross-Platform Music Player — Android, Linux Desktop & Terminal

<br/>

[![Latest release](https://img.shields.io/github/v/release/MehmetCanWT/sonata?style=for-the-badge&color=B79CED&labelColor=0d1117)](https://github.com/MehmetCanWT/sonata/releases/latest)
[![License](https://img.shields.io/github/license/MehmetCanWT/sonata?style=for-the-badge&color=8F96D9&labelColor=0d1117)](LICENSE)
[![Downloads](https://img.shields.io/github/downloads/MehmetCanWT/sonata/total?style=for-the-badge&color=7C6CE0&labelColor=0d1117)](https://github.com/MehmetCanWT/sonata/releases)
[![Engine](https://img.shields.io/badge/Engine-Rust%20%2B%20UniFFI-f97316?style=for-the-badge&labelColor=0d1117)](#architecture)

<br/>

[**Download**](#download) · [**Screenshots**](#screenshots) · [**Features**](#features) · [**Build from Source**](#build-from-source) · [**Disclaimer**](#disclaimer)

<br/>

</div>

> [!IMPORTANT]
> Sonata is an independent open-source music client and is not affiliated with, endorsed by, or connected to Google, YouTube Music, or Spotify in any way. Use it at your own discretion.

---

<div align="center">

<h1><a id="features"></a>Features</h1>

<table>
  <tr>
    <td width="50%" valign="top">

#### Playback & High-Fidelity Audio
- **Native Extreme Audio** — authenticated `TVHTML5` InnerTube client + zero-AST JS cipher unlocks **`itag=774` (~256–321 kbps Opus Extreme)** and **`itag=141` (256 kbps AAC Extreme)** across Android, Desktop, and Terminal.
- **Optional JioSaavn Module** — opt-in 320 kbps AAC source in *Settings → Modules* that searches in parallel and seamlessly upgrades playback when a strictly higher bitrate is found.
- **Explicit Track Preference** — automatically prefers uncensored explicit versions over clean radio edits when available.
- **Gapless Playback & Mid-Track Recovery** — bounded range streaming, lookahead precaching, and automatic stream recovery on network hiccups.
- **Permanent Offline Downloads** — save individual tracks or entire albums/playlists into a first-class in-app **Downloads** playlist with offline-first playback and downloaded track badges.

#### Cross-Platform Ecosystem
- **Android (Phone & Tablet)** — native Jetpack Compose + Media3 ExoPlayer client powered by Rust (`sonata-core` via UniFFI), adaptive portrait & landscape layouts, and **60 / 90 / 120 Hz** support.
- **Linux Wayland Desktop (`sonata-desktop`)** — native GPU-accelerated GPUI (`gpui-kit`) client with ambient glass shaders, selectable skins, MPRIS2, System Tray, and Waybar/Hyprlock telemetry.
- **Terminal Client (`sonata`)** — ultra-fast Ratatui TUI with Kitty/Sixel HD album art, animated motion covers, synced lyrics, mouse-resizable panes, and Mini Mode.
- **YouTube Music & Spotify Sync** — sign in for personalized Home feeds, Mixes, Library playlists, and two-way playlist management (`Add to Playlist` / `Create Playlist`).

    </td>
    <td width="50%" valign="top">

#### Experience & Liquid Glass UI
- **Lunaria Moonflower Design** — custom moon-inspired palette (`#B79CED` Petal Lavender, `#8F96D9` Lunar Mist) paired with bundled **Plus Jakarta Sans** typography.
- **Optical Liquid Glass** — real-time refraction shader on floating navigation bars, mini player, Now Playing options, and anchored track menus.
- **Animated Album Artwork (Canvas)** — motion covers from Apple Music HLS, Tidal, and community MP4s plus procedural animated covers for Stations and Moods.
- **Word-Synced & Duet Karaoke Lyrics** — multi-provider word/syllable-level lyrics (`lrc.red`, BetterLyrics, LyricsPlus, LRCLIB, KuGou, NetEase, Genius) with simultaneous overlapping background vocal (`(hey)`) glow, romanization, translation, and per-track timing offset.
- **Editorial Explore, Artist & Expressive Search** — full-bleed Artist pages with seamless hero dissolve, tilted *Latest Release* spotlight, numbered *Popular tracks*, editorial Explore carousel, Radio Mixes, and fitted Material 3 Expressive Search grids.

#### Controls, Performance & Privacy
- **Audio Controls & Pipeline Inspector** — playback speed, loudness normalization, silence skip, sleep timer, and a 4-stage Audio Signal Chain modal.
- **Zero Trackers & 100% FOSS** — no ads, no analytics, no Google Play Services, no Firebase, deterministic reproducible APK builds (`dependenciesInfo` disabled), and `0o600`/`0o700` private credential handling.
- **Built-in Update Notifications** — automatic GitHub Releases update checks across Android, Desktop, and Terminal.

    </td>
  </tr>
</table>

</div>

---

<div align="center">

<h1><a id="screenshots"></a>Screenshots</h1>

<table>
  <tr>
    <th colspan="2">Android — Now Playing &amp; Word-Synced Karaoke Lyrics</th>
  </tr>
  <tr>
    <td align="center" width="50%">
      <img src="assets/android-mobile-now-playing.png" alt="Sonata Android Now Playing" width="320" />
    </td>
    <td align="center" width="50%">
      <img src="assets/android-mobile-lyrics.png" alt="Sonata Android Synced Karaoke Lyrics" width="320" />
    </td>
  </tr>
  <tr>
    <th width="50%">Linux Wayland Desktop (<code>sonata-desktop</code>)</th>
    <th width="50%">Linux Terminal TUI (<code>sonata</code>)</th>
  </tr>
  <tr>
    <td align="center" width="50%">
      <img src="assets/hyprland-glass-desktop.png" alt="Sonata Linux Wayland Desktop" width="100%" />
    </td>
    <td align="center" width="50%">
      <img src="assets/hyprland-glass.png" alt="Sonata Terminal TUI" width="100%" />
    </td>
  </tr>
</table>

</div>

---

<div align="center">

<h1><a id="download"></a>Download</h1>

<a href="https://github.com/MehmetCanWT/sonata/releases/latest" target="_blank" rel="noopener noreferrer"><img src="https://cdn.jsdelivr.net/gh/JMcrafter26/badges@main/src/assets/supported/android/cozy.svg" alt="Download for Android" height="54"/></a>
<a href="https://github.com/MehmetCanWT/sonata/releases/latest" target="_blank" rel="noopener noreferrer"><img src="https://cdn.jsdelivr.net/gh/JMcrafter26/badges@main/src/assets/supported/linux/cozy.svg" alt="Download for Linux" height="54"/></a>

<br/><br/>

<table>
  <tr>
    <td width="50%" valign="top">

#### Android (Mobile & Tablet, `arm64-v8a`)
- **Obtainium (Auto-Updates):** Add `https://github.com/MehmetCanWT/sonata` with APK filter `Sonata-.*-arm64\.apk$`.
- **Direct APK:** Grab the latest signed **`Sonata-v1.0.2-arm64.apk`** from [GitHub Releases](https://github.com/MehmetCanWT/sonata/releases/latest).

    </td>
    <td width="50%" valign="top">

#### Linux Desktop & Terminal (`x86_64`)
- **AppImage (GUI + TUI):** Download **`Sonata-v1.0.2-x86_64.AppImage`** from [GitHub Releases](https://github.com/MehmetCanWT/sonata/releases/latest), `chmod +x`, and run (`--tui` launches the terminal client).
- **One-Line Installer:**
  ```sh
  # Terminal client (sonata)
  curl -fsSL https://raw.githubusercontent.com/MehmetCanWT/sonata/main/install.sh | sh

  # Desktop GUI (sonata-desktop)
  curl -fsSL https://raw.githubusercontent.com/MehmetCanWT/sonata/main/install-desktop.sh | sh
  ```
- **Arch Linux / AUR:** `makepkg -si` (`PKGBUILD` for TUI, `PKGBUILD.desktop` for Desktop).

    </td>
  </tr>
</table>

</div>

---

<a id="architecture"></a>
<a id="build-from-source"></a>

<details>
<summary><strong>Architecture &amp; Build from Source</strong></summary>

### Workspace Layout

| Crate / Folder | Role |
|---|---|
| `crates/sonata-core` | Shared Rust engine (auth, Hi-Fi cipher & stream resolution, multi-provider TTML/YRC/KRC/LRCLIB lyrics, SQLite WAL cache, `mpv` IPC, updater) |
| `crates/sonata-android` + `android/` | UniFFI Rust bridge (`libsonata_android.so`) + Jetpack Compose / Media3 ExoPlayer Android mobile & tablet app |
| `crates/sonata-desktop` | Native GPUI Wayland/Hyprland desktop client with MPRIS2 & system tray |
| `crates/sonata-tui` | Ratatui terminal client with Kitty/Sixel artwork & synced lyrics |

### Build Commands

```sh
# Rust workspace (Linux Desktop & TUI)
cargo build --release --locked -p sonata-tui -p sonata-desktop

# Android native library + release APK (JDK 21, Android SDK 35, NDK 27.2.12479018, cargo-ndk)
rustup target add aarch64-linux-android
cargo ndk -t arm64-v8a -P 26 -o android/app/src/main/jniLibs build -p sonata-android --release --locked
cd android && ./gradlew :app:assembleRelease --console=plain
```

### Verification Suite

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

</details>

---

<div align="center">

<h1><a id="disclaimer"></a>Disclaimer & Legal Notice</h1>

Sonata is an independent, community-driven third-party audio player and client. It is **not** associated with Google LLC, YouTube Music, Spotify AB, or any of their parent companies.

- **No Media Hosting:** Sonata does not host, upload, or distribute copyrighted music files. It operates strictly as a client interface for local playback or streaming from public and user-authenticated APIs.
- **Fair Use & Responsibility:** This software is provided for personal, educational, and fair-use purposes. Users are solely responsible for complying with local copyright laws and third-party platform Terms of Service.
- **Privacy:** All session cookies and caches are stored locally on your device with restricted permissions (`0o600`/`0o700`) and are never transmitted to any third-party telemetry server.

</div>

---

<div align="center">

<h1><a id="license"></a>License</h1>

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

</div>
