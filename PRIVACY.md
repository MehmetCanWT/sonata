# Privacy Policy

**Sonata** is a local-first music player across **Android**, **Linux Desktop (`sonata-desktop`)**, and **Linux Terminal (`sonata`)**. It runs directly on your device and does not operate any Sonata backend, telemetry, crash reporting, or tracking servers.

## 1. Zero Telemetry, Ads, or Tracking SDKs

- Sonata collects **zero** analytics, behavioral telemetry, crash logs, advertising IDs, or device identifiers.
- Android builds omit Google Play Services, Firebase, ad SDKs, and encrypted dependency metadata (`dependenciesInfo` disabled for reproducible builds). `REQUEST_INSTALL_PACKAGES` is declared solely so users who explicitly tap **Download & Install** on an in-app GitHub Releases update prompt (`ApkUpdater.kt`) can launch Android's system `PackageInstaller` for the downloaded APK via scoped `FileProvider`.
- All network communication goes directly from your device to the third-party music, lyrics, artwork, or release hosts you interact with.

## 2. Authentication & Session Cookies

- **Desktop & Terminal (Browser Cookie Extraction)**: To authenticate with YouTube Music (and optionally Spotify) without developer API keys, Sonata can read session cookies locally from supported desktop browsers installed on your system (Zen Browser, Firefox, LibreWolf, Floorp, Chrome, Chromium, Brave, Vivaldi, and Edge). Extracted cookies are written locally to `~/.cache/sonata/yt_cookies.txt` using owner-only file creation (`0o600` on Unix) inside `0o700` directories.
- **Android (In-App WebView Sign-In & Account Switching)**: When you sign in to YouTube Music on Android, session cookies are stored privately inside the app's sandboxed `filesDir/sonata/ytm_session.cookie` (`0o600` on Unix). Selecting **Sign Out** or **Account Switching** clears WebView cookies, `WebStorage`, cache, history, and local cached profile/playlist files before launching account selection.
- Cookies and `SAPISIDHASH` headers are sent solely to the corresponding music provider (`music.youtube.com` / `open.spotify.com`) over HTTPS.

## 3. Direct Third-Party Network Requests

Depending on your enabled features and optional modules, Sonata sends direct HTTPS requests from your device to:

- **Music Catalog, Playback & Listening History**:
  - **YouTube Music (`music.youtube.com`, Google Video CDNs)**: Search, Home/Explore/Radio feeds, Library playlists, Artist pages, stream resolution (`TVHTML5` / `WEB_REMIX`), likes, playlist management, and—when signed in—`report_playback` watchtime reporting so your Listening History (`FEmusic_history`) and yearly YouTube Music Replay stay updated (including resolved YouTube Music matches when audio streams come via JioSaavn).
  - **Spotify (`open.spotify.com`)**: Public embed playlists (no login needed) or optional `sp_dc` user library access; tracks resolve against YouTube audio sources.
  - **JioSaavn (`jiosaavn.com`, opt-in module, default OFF)**: Only queried when explicitly enabled under *Settings → Audio Quality* (or *Settings → Downloads → Include JioSaavn* on Android, and Settings on Desktop & Terminal) to locate higher-bitrate AAC streams.
- **Lyrics Providers**: `lrc.red`, BetterLyrics, LyricsPlus v2, Apple Music TTML catalogue proxy (`lyrics-api.binimum.org`), LRCLIB (`lrclib.net`), KuGou (`kugou.com`), NetEase (`music.163.com`), QQ Music (`c.y.qq.com`), YouTube Music native lyrics, and Genius (`genius.com`). Optional Android lyrics translation uses local or device-initiated translation lookups only when requested.
- **Artwork & Motion Covers (Canvas)**: Apple Music HLS (`editorialVideo`), Tidal, community Canvas manifests, Deezer (`api.deezer.com`), iTunes Search (`itunes.apple.com`), and provider CDNs.
- **Release Checks**: GitHub Releases API (`api.github.com/repos/MehmetCanWT/sonata/releases`) when checking for Sonata app updates (can be ignored/dismissed at any time).

Each third-party service is governed by its own privacy policy when your device connects to it.

## 4. On-Device Storage, Backups, Sharing & Redacted Logs

- **Linux Config & Cache**:
  - Configuration and TOML themes: `~/.config/sonata/` (`config.toml`, `themes/*.toml`).
  - Media, artwork, lyrics, and SQLite WAL metadata cache: `~/.cache/sonata/`, bounded by your configured cache limit (or cleared when cache is turned off).
  - Diagnostics logs: `~/.cache/sonata/sonata.log` for Desktop/runtime (capped at 10 MiB at startup, `0o600` permissions) with URLs, cookies, and `Authorization` / `SAPISID` / `sp_dc` values scrubbed via `sonata_core::redaction`, plus `~/.cache/sonata/sonata-tui.log` for Terminal stderr diagnostics (capped at 2 MiB at startup).
  - Local integrations: `$XDG_RUNTIME_DIR/sonata/now_playing.json` (or `~/.cache/sonata/now_playing.json`) for local bars/widgets like Waybar/Hyprlock (removed on clean exit) and session D-Bus MPRIS2 (`org.mpris.MediaPlayer2.sonata`).
- **Android Local Storage, Offline Library, Updates & Share Cards**:
  - Preferences, offline downloads, and local playlists stay inside private app storage (unless you explicitly save a share card image to `Pictures/Sonata` or export downloaded audio via MediaStore). User-initiated app updates temporarily download `Sonata-v<version>-arm64.apk` into app-private `cacheDir/updates/`.
  - JSON settings backup/restore (`SonataPreferences`) excludes all authentication cookies and session tokens.
  - Visual Share Sheet renders PNG cards locally on-device using Android Canvas and shares via scoped `androidx.core.content.FileProvider` URIs only when you tap **Share Link**, **Share Image**, or **Save Image**.
