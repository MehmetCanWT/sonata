# How Sonata Works

See [architecture.md](architecture.md) for the crate, UniFFI, and process layout.

## 1. Authentication

### YouTube Music
1. **Desktop & Terminal**: `auth/cookies.rs` extracts logged-in session cookies from supported browsers (`SUPPORTED_BROWSERS`). Gecko profiles are copied into a `0o700` `TempDirGuard` cleaned on drop; Chromium browsers run `yt-dlp --cookies-from-browser` with a 15-second timeout.
2. **Android**: In-app `YtMusicLoginSheet.kt` authenticates via WebView and saves the cookie header privately (`ytm_session.cookie`, `0o600`). **Sign Out** and **Account Switching** asynchronously clear WebView cookies, `WebStorage`, cache, history, and local disk caches before loading Google's `prompt=select_account` flow.
3. For every InnerTube request, `YtMusicClient` sends cookies alongside `Authorization: SAPISIDHASH <ts>_<sha1(ts + " " + SAPISID + " " + origin)>` (covering `SAPISID` / `__Secure-1PAPISID` / `__Secure-3PAPISID` and `X-Goog-PageId` for Brand Channels).
4. Logs pass through `redaction.rs`, scrubbing cookies, authorization headers, and signed media URLs.

### Spotify
- Public playlists work **without credentials** via `fetch_embed_playlist` (`providers/spotify/embed.rs`).
- With an optional `sp_dc` cookie, `SpotifyClient` (`providers/spotify/mod.rs`) authenticates GraphQL requests and matches Spotify tracks to YouTube audio (`providers/hifi/matching.rs`).

## 2. Playback, Quality Upgrades & Attribution

1. **Fast start**: `StreamResolver::resolve_fast` prefers cached files or InnerTube responses first (`yt-dlp` fallback on Linux).
2. **Native Extreme Audio (`itag=774` / `141`)**: `TVHTML5` InnerTube negotiation paired with the zero-AST `base.js` cipher hook (`providers/hifi/cipher.rs`, executed via Node on Linux or `AndroidJsCipherSolver` WebView on Android) unlocks ~256–321 kbps Opus and 256 kbps AAC Extreme streams.
3. **Hi-Fi & JioSaavn upgrades**: Background quality upgrades (`resolve_hifi_upgrade`, plus opt-in `JioSaavnSource.kt` on Android) check cached, online, and offline tracks against user quality targets (`AudioQualityTier`), hot-swapping cleanly at current playback position when a strictly higher bitrate stream is verified.
4. **Watchtime & Replay Attribution**: `YtMusicClient::report_playback` reports active playbacks (with normalized playlist IDs and CPNs) across TUI, Desktop, and Android—including resolving YouTube Music `videoId` matches when playing JioSaavn-upgraded streams—so Listening History (`FEmusic_history`) and Annual Replay stay in sync.
5. **High-res artwork & animated Canvas**: `media.rs` / `SonataDesignSystem.kt` upscale Google/YouTube/Apple artwork URLs, while Android `CanvasResolver.kt` and core `motion.rs` load looping Canvas video/HLS motion covers alongside 15 bounded procedural mood animations (`SonataAnimatedArtworks.kt`).

## 3. Lyrics Pipeline

`lyrics/lrclib.rs::fetch_lyrics` resolves word-synced and dual-voice lyrics via concurrent multi-provider scoring:

1. **Local cache**: Checks SQLite + `.lrc`/`.txt` cache files first, upgrading legacy durationless/plain caches lazily via `.wordcheck`.
2. **Concurrent fetch (`tokio::join!`)**: Queries Apple Music TTML sources (`lrc.red` ISRC lookup, BetterLyrics, LyricsPlus v2, `lyrics-api.binimum.org`), LRCLIB, KuGou KRC, NetEase YRC, and QQ Music in parallel while rejecting version mismatches (`Remix`, `Slowed`, `Live`, etc.) and oversized duration deltas.
3. **Scoring & rendering**: Prefers high-density word-level timings and dual-voice `Lead`/`Counter`/`(x-bg)` spans; falls back to LRCLIB plain -> YTM native -> Genius.

## 4. Visual Polish (Desktop & Android)

- **Desktop Fullscreen (`sonata-desktop`)**: Hue-clustered ambient waves (`112x64` BGRA upscaled on GPU) + cached multi-bucket Gaussian blurred inactive lyrics + dead-center auto-scroll (`AUTO_SCROLL_ANCHOR_RATIO = 0.50`).
- **Android UI (`android/`)**: Modular **`Violet`** (default) and **`Moonflower`** design systems across Light, Dark, and AMOLED with real-time optical Liquid Glass shaders, pinkish active tab domes (`GlassSelectionPill`), visual PNG Share Cards (`SonataShareSheet.kt`), and hardware 60/90/120 Hz pacing (`SteeredLyricClock`).
