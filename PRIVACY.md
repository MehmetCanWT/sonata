# Privacy Policy

**Sonata** is a local-first music player. It runs completely on your device and does not operate any telemetry, analytics, user accounts, or remote Sonata servers.

## 1. No Telemetry or Sonata Servers

- Sonata collects **zero** analytics, crash reports, usage metrics, or device identifiers.
- There is no Sonata backend, relay, or cloud sync service. All requests go directly from your machine to the third-party music and metadata services you use.

## 2. Browser Cookie Extraction

- To authenticate with YouTube Music (and optionally load your Spotify library) without requiring developer API keys, Sonata can read session cookies locally from supported browsers installed on your system (Zen Browser, Firefox, LibreWolf, Floorp, Chrome, Chromium, Brave, Vivaldi, and Edge).
- Extracted YouTube cookies are written locally to `~/.cache/sonata/yt_cookies.txt` with owner-only permissions (`0o600` on Unix) so `yt-dlp` and the playback resolver can authenticate streams on your behalf.
- Cookies never leave your machine except in direct HTTPS requests to the corresponding provider (`music.youtube.com` / `open.spotify.com`).

## 3. Direct Third-Party Network Requests

Depending on your enabled providers and settings, Sonata makes direct HTTPS requests from your device to:

- **Music & Stream Providers**: YouTube Music (`music.youtube.com`, Google Media CDNs), Spotify (`open.spotify.com`, Spotify APIs), and JioSaavn (for optional Hi-Fi stream matching).
- **Lyrics Providers**: LRCLIB (`lrclib.net`), Apple Music TTML catalogue proxy (`lyrics-api.binimum.org`), KuGou (`kugou.com`), NetEase (`music.163.com`), YouTube Music native lyrics, and Genius (`genius.com`).
- **Artwork Metadata**: Deezer (`api.deezer.com`), iTunes Search (`itunes.apple.com`), and provider CDNs.

Each third-party service is governed by its own privacy policy when you send requests to it.

## 4. Local Storage & Logs

- **Configuration & Themes**: Stored locally under `~/.config/sonata/` (`config.toml` and `themes/*.toml`).
- **Cache & Database**: Artwork, audio prefetch, lyrics, and SQLite WAL metadata are stored locally under `~/.cache/sonata/` and bounded by your configured cache limit (Settings → Media Cache, including `Off` to clear cached media).
- **Logs**: Runtime diagnostics are written locally to `~/.cache/sonata/sonata.log` (capped at 10 MiB at startup, user-only permissions on Unix) with sensitive tokens, cookies, and signed URLs redacted via `sonata_core::redaction`.
- **Desktop / Status Integrations**:
  - Playback metadata is written atomically to `$XDG_RUNTIME_DIR/sonata/now_playing.json` (fallback: `~/.cache/sonata/now_playing.json`) for local bars/widgets (such as Waybar or Hyprlock) and removed on clean exit.
  - `sonata-desktop` exposes playback controls and metadata locally over your session D-Bus (`org.mpris.MediaPlayer2.sonata`).
