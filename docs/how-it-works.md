# How Sonata Works

See [architecture.md](architecture.md) for the crate, UniFFI, and process layout.

## 1. Authentication

### YouTube Music
1. **Desktop & Terminal**: [`sonata-runtime/src/auth/browser.rs`](../crates/sonata-runtime/src/auth/browser.rs) extracts logged-in session cookies from supported local browsers (Gecko profiles are copied into a `0o700` temp directory cleaned on drop; Chromium browsers run `yt-dlp --cookies-from-browser` with a bounded timeout). [`sonata-core/src/auth/cookies.rs`](../crates/sonata-core/src/auth/cookies.rs) parses Netscape/header formats and persists `~/.cache/sonata/yt_cookies.txt` using owner-only `0o600` creation permissions.
2. **Android**: In-app `YtMusicLoginSheet.kt` authenticates via WebView and saves the cookie header privately (`ytm_session.cookie`, `0o600`). **Sign Out** and **Account Switching** asynchronously clear WebView cookies, `WebStorage`, cache, history, and local disk caches before loading Google's `prompt=select_account` flow.
3. For every InnerTube request, `YtMusicClient` sends cookies alongside `Authorization: SAPISIDHASH <ts>_<sha1(ts + " " + SAPISID + " " + origin)>` (covering `SAPISID` / `__Secure-1PAPISID` / `__Secure-3PAPISID` and `X-Goog-PageId` for Brand Channels).
4. Logs and surfaced errors pass through `sonata_core::redaction`, scrubbing cookies, authorization headers, and signed media URLs.

### Spotify
- Public playlists work **without credentials** via `fetch_embed_playlist` (`providers/spotify/embed.rs`).
- With an optional `sp_dc` cookie, `SpotifyClient` (`providers/spotify/mod.rs`) authenticates GraphQL requests and matches Spotify tracks to YouTube audio (`providers/hifi/matching.rs`).

## 2. Playback, Quality Upgrades & Attribution

1. **Fast start**: `sonata-core::providers::hifi::StreamResolver` (configured per client via `ResolverProfile`) prefers cached files and pure-Rust InnerTube responses first; on Linux, `sonata-runtime::process::LinuxStreamResolver` adds bounded `yt-dlp` fallback and recovery.
2. **Native Extreme Audio (`itag=774` / `141`)**: `TVHTML5` InnerTube negotiation paired with the zero-AST `base.js` cipher hook (`providers/hifi/cipher.rs`, executed via sandboxed `CliJsCipherBridge` in `sonata-runtime` using `deno` or Node `--permission` on desktop/TUI, or `AndroidJsCipherSolver` WebView on Android) unlocks ~256–321 kbps Opus and 256 kbps AAC Extreme streams.
3. **Hi-Fi & JioSaavn upgrades**: Background quality upgrades (`resolve_hifi_upgrade` plus consolidated pure-Rust `sonata-core::providers::jiosaavn` shared across Linux and Android via UniFFI) check cached, online, and offline tracks against user quality targets (`AudioQualityTier`), hot-swapping cleanly at current playback position (preserving play/pause state and settings) when a strictly higher bitrate stream is verified.
4. **Watchtime, History & Playlist Editing**: `YtMusicClient::report_playback` reports individual played tracks (without playlist IDs, including resolving YouTube Music `videoId` matches when audio streams come from JioSaavn) so Listening History (`FEmusic_history`) and Annual Replay stay accurate across TUI, Desktop, and Android. Desktop and Android also support YTM playlist editing (`browse/edit_playlist`) and 3-step resumable custom cover uploads (`ACTION_SET_CUSTOM_THUMBNAIL`).
5. **High-res artwork & animated Canvas**: `sonata-core::media` upscales artwork URLs and matches Deezer/iTunes covers, while `sonata-core::motion` (`MotionMedia`) discovers Apple Music HLS (`editorialVideo`), Tidal, and Community Canvas streams. Linux decodes frames via `ffmpeg` (`sonata-runtime::media::motion`); Android pairs slim LRU caching in `CanvasResolver.kt` with Media3 `PlayerSurface` and 15 bounded procedural mood animations (`SonataAnimatedArtworks.kt`).

## 3. Lyrics Pipeline

`lyrics/lrclib.rs::fetch_lyrics` resolves word-synced and dual-voice lyrics via concurrent multi-provider scoring:

1. **Local cache**: Checks SQLite + `.lrc`/`.txt` cache files first, upgrading legacy durationless/plain caches lazily via `.wordcheck`.
2. **Concurrent fetch (`tokio::join!`)**: Queries Apple Music TTML sources (`lrc.red` ISRC lookup, BetterLyrics, LyricsPlus v2, `lyrics-api.binimum.org`), LRCLIB, KuGou KRC, NetEase YRC, and QQ Music in parallel while rejecting version mismatches (`Remix`, `Slowed`, `Live`, etc.) and oversized duration deltas.
3. **Scoring & rendering**: Prefers high-density word-level timings and dual-voice `Lead`/`Counter`/`(x-bg)` spans; falls back to LRCLIB plain -> YTM native -> Genius.

## 4. Visual Polish, Localization & Updates

- **Desktop (`sonata-desktop`)**: Cross-platform Tauri v2 host streaming immutable snapshots and 10 Hz playback progress ticks over `tauri::ipc::Channel` into a React 19 + Tailwind CSS v4 + shadcn/ui matte obsidian frontend (`desktop-ui/`) with 60 FPS interpolated progress/lyrics, Album/Artist/Immersive Lyrics views, Dark & Light modes, system tray, and MPRIS2 (Linux) / `souvlaki` (macOS, Windows) media controls.
- **Terminal (`sonata`)**: Ratatui TUI with Kitty/Sixel/Halfblock artwork, double-buffered `360x360` Kitty motion covers, `tachyonfx` region-scoped transitions (`ui/fx.rs`), and 60 FPS spring synced lyrics.
- **Android (`android/`)**: Monochrome **`Basic`** design system (`System`, `Light`, and true-black `#000000` `Dark`) with neutral crystal-clear Liquid Glass (`LiquidGlass.kt`), visual PNG Share Cards (`SonataShareSheet.kt`), hardware 60/90/120 Hz pacing, `en`/`tr`/`ru` UI localization (`SonataStrings.kt`, `AppLanguage.SYSTEM` default; desktop/TUI clients share `sonata-runtime/src/i18n/`), and user-initiated GitHub Releases APK downloads via `ApkUpdater.kt` (`FileProvider` + `PackageInstaller`).
