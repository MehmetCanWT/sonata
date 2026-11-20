# How Sonata Works

See [architecture.md](architecture.md) for the crate and process layout.

## 1. Authentication

### YouTube Music
1. `auth/cookies.rs` extracts the user's logged-in session cookies from a supported browser (`SUPPORTED_BROWSERS`). Gecko browsers are read from a temporary copy of `cookies.sqlite` (plus `-wal`/`-shm` sidecars) inside a `0o700` `TempDirGuard` directory cleaned on drop. Chromium browsers go through `yt-dlp --cookies-from-browser` with a 15-second timeout.
2. Cookies are normalised into a Netscape-format file created with `0o600` permissions (`OpenOptions::mode(0o600)`) in the cache directory (`yt_cookies.txt`) so `yt-dlp` can reuse them.
3. For every InnerTube request, `YtMusicClient` sends the cookie header plus an `Authorization: SAPISIDHASH <ts>_<sha1(ts + " " + SAPISID + " " + origin)>` header (`compute_sapisidhash`).
4. Logs pass through `redaction.rs`, which scrubs cookies, authorization headers and signed media URLs.

### Spotify
- Playlist links work **without credentials**: `fetch_embed_playlist` parses the public embed page, and `extract_playlist_id` accepts URLs or URIs (`providers/spotify/embed.rs`).
- With an optional `sp_dc` cookie, `SpotifyClient` (`providers/spotify/mod.rs`) obtains web-player and client tokens (TOTP-based, `compute_totp`) and uses the GraphQL "pathfinder" API for library playlists, liked songs, and search.
- Spotify tracks are not streamed from Spotify; they are matched to YouTube audio (`providers/hifi/matching.rs`).

## 2. Playback & Artwork

1. **Fast start**: `StreamResolver::resolve_fast` returns a playable stream as quickly as possible, preferring a locally cached file when caching is enabled. InnerTube responses are tried first, `yt-dlp` is the fallback.
2. **Hi-Fi upgrade**: in the background `resolve_hifi_upgrade` looks for a better stream, but only when `needs_hifi_upgrade` says the current one is below target (FLAC/ALAC, AAC at 320 kbps or more, or a top YouTube tier are never upgraded). When the upgrade is ready, the player reloads it via `mpv`'s `loadfile ... replace` with a `start=<secs>` option (`load_stream_at`), so playback resumes at the current position.
3. **Lookahead precache**: `NextTrackPrecache` (`player/precache.rs`) schedules once the current track is within `NEXT_TRACK_PRECACHE_LEAD_SECS` (10 s) of its end. `precache_hifi_audio` downloads the next track (capped at 80 MiB) so the next track can start from a local file.
4. **Recovery**: a playback error triggers at most one recovery attempt per play sequence (`resolve_recovery`, bypassing cached streams).
5. **Cache bounds**: the in-memory stream cache holds at most 256 entries; on-disk files are tracked by last-use time and pruned oldest-first to the configured budget (`cache/media.rs`).
6. **High-res & animated artwork**: `sonata_core::media::upscale_artwork_url` upgrades initial provider thumbnails immediately (YouTube/Google `=w1200-h1200-l90-rj`, Spotify `ab67616d0000b273` 640px, iTunes `1200x1200bb` HTTPS), while `resolve_track_cover` enriches covers across **Deezer (`api.deezer.com/search/track`) → iTunes (`itunes.apple.com/search`) → upscaled provider thumbnail** for both YouTube Music and Spotify tracks. When an Apple Music `editorialVideo` stream is found on the matching album page (`resolve_animated_cover_frames`), `ffmpeg` extracts 20 square `320x320` PNG frames at `8 fps` into `~/.cache/sonata/art/motion_<hash>/` for loop playback in both TUI and desktop (`reduce_motion` disables animation).
7. `PlaybackCoordinator` (`player/coordinator.rs`) holds the pure command → transition rules shared by frontends, and `PlaybackTaskSet` aborts stale async work.

## 3. Lyrics pipeline

`lyrics/lrclib.rs::fetch_lyrics` resolves word-synced and dual-voice lyrics via concurrent multi-provider scoring:

1. **Local cache**: checks `.lrc` and `.txt` files first. If a cached `.lrc` has no word-level timestamps and hasn't been checked online yet (`.wordcheck` marker), Sonata attempts a one-time word-synced upgrade.
2. **Concurrent fetch (`tokio::join!`)**: queries five synced sources in parallel:
   - **Apple Music TTML** (`lyrics/providers.rs::fetch_ttml_synced` + `lyrics/ttml.rs`): queries `https://lyrics-api.binimum.org/` (with albumless retry) and parses TTML via `roxmltree` for word/syllable `<span begin=.. end=..>` timings, `<ttm:agent type="person"/>` dual voices (`LyricVoice::Lead` vs `LyricVoice::Counter`), and `<span ttm:role="x-bg">` parenthetical background vocals (skipping translation/romanization spans).
   - **LRCLIB** (`query_lrclib`): `/api/get` plus `/api/search` fallback when `/api/get` lacks word-timed lyrics, preferring Enhanced LRC candidates.
   - **KuGou KRC** (`fetch_kugou_synced`): `fmt=krc` decoded via 4-byte `krc1` header skip, 16-byte XOR cipher, and `flate2` zlib decompression, falling back to `fmt=lrc`.
   - **NetEase YRC** (`fetch_netease_synced`): `api/song/lyric/v1` `yrc.lyric` word timings, falling back to `lrc.lyric`.
   - **QQ Music** (`fetch_qqmusic_synced`): `c.y.qq.com` search + base64-decoded LRC (`fcg_query_lyric_new.fcg`).
3. **Deterministic candidate scoring (`score_synced_lyrics`)**: every `usable_synced_lyrics` hit is scored on word-timing density (`+300` when `>= 1/3` of lines have word spans), dual-voice presence (`+80` for `Counter`), duration-end coverage (`+120` within `[0.65*dur, dur+3s]`, `-150` if `< 0.45*dur`), line count sanity (`+50` for `8..=160` lines), and source bonus (`TTML: 40`, `LRCLIB: 35`, `KuGou: 30`, `NetEase: 28`, `QQMusic: 20`). The highest-scoring sheet wins; plain fallbacks remain LRCLIB plain → **YTM native lyrics** → **Genius** HTML scrape.
4. **Lossless LRC round-trip**: `format_lrc` and `parse_lrc` (`lyrics/mod.rs`) preserve Enhanced LRC `<mm:ss.xxx>` word timestamps and `v2:` counter-voice prefixes across cache writes and reloads.

## 4. Fullscreen visual effects (desktop)

Implemented across [`views/fullscreen_fx/`](../crates/sonata-desktop/src/views/fullscreen_fx) and [`views/fullscreen/`](../crates/sonata-desktop/src/views/fullscreen) on stock GPUI with embedded `Inter` fonts (`crates/sonata-desktop/assets/fonts/`):

1. **Palette** (`palette.rs`): 24-bin circular hue clustering over sampled cover pixels yields a leading hue, distinct secondary hue, and mean lightness.
2. **Wave field** (`waves.rs`): `render_waves_frame` evaluates a five-color procedural wave field on the CPU into a `112x64` base BGRA `RenderImage` (upscaled by GPUI's GPU sampler); `AmbientWavesState` cross-fades smoothly between tracks.
3. **Zero-stutter blurred lyrics** (`blur_cache.rs` & `sheet.rs`): `cosmic-text` shapes and rasterizes inactive lines using embedded `Inter` (`Family::Name("Inter")`) at full white alpha, then applies a 3-pass separable box blur on the alpha plane. `BlurredLyricCache` (256 entries) keys on text/font/blur bucket **without** line opacity—display fade `.opacity(alpha)` is applied by GPUI—and `prewarm_blur_buckets` pre-renders all three distance blur buckets around the active line.
4. **50% dead-center auto-scroll & controls** (`fullscreen/mod.rs`, `sheet.rs`, `controls.rs`): `AUTO_SCROLL_ANCHOR_RATIO = 0.50` with 50% top/bottom spacers keeps even first/last lines centered (`AUTO_SCROLL_SPEED = 14.0`). Manual scroll yields for `1.2s` (`MANUAL_SCROLL_SETTLE = 1200ms`) with a `manual_scroll_gen` timer that re-blurs and recenters even when paused. Transport, seek, and volume controls sit transparently beneath the left-column cover art (`CONTROLS_DOCK_HEIGHT = 84.0`) without overlapping lyrics.
