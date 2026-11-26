# Third-Party Integrations (MPRIS2, Animated Artwork, Now Playing & Lyrics)

Sonata exposes live playback metadata, animated album covers, and structured JSON telemetry files on Linux across both `sonata-desktop` and the `sonata` TUI so external desktop shells, status bars, lockscreens, and widgets (Waybar, Quickshell, Hyprlock, eww, scripts) can integrate without scraping or heuristics.

---

## 1. MPRIS2 D-Bus Interface (`org.mpris.MediaPlayer2.sonata`)

Both `sonata-desktop` and `sonata` (`crates/sonata-tui`) register `org.mpris.MediaPlayer2.Player` on the session D-Bus under `org.mpris.MediaPlayer2.sonata`.

### Metadata Keys

| Key | Value Type | Description |
|---|---|---|
| `mpris:trackid` | ObjectPath | Active track identifier (`/dev/sonata/Track/<sha1>` or `NoTrack`) |
| `xesam:title` | String | Track title |
| `xesam:artist` | Array&lt;String&gt; | Track artist(s) |
| `xesam:album` | String | Album title |
| `mpris:length` | Int64 (microseconds) | Track duration |
| `mpris:artUrl` | String (`file://` or `https://`) | High-resolution static album cover (`.png` / CDN URL). Never overwritten by video URLs so standard MPRIS widgets continue working reliably. |
| `sonata:motionArtUrl` | String (`file://`) | Local `file://` URI to the normalized seamless-loop MP4 (`~/.cache/sonata/art/motion_<sha1>.mp4`, 8-bit `yuv420p`, `faststart`, `start_time=0`). |

> [!NOTE]
> Whenever `motion_covers` is enabled and not in Personal Only mode, `sonata:motionArtUrl` is emitted unconditionally—even when the window is in standard view, mini/compact mode, hidden in the system tray, or when `reduce_motion` is enabled in the TUI.

### Quick Shell & `playerctl` Snippets

```sh
# Query active metadata including animated cover URI
playerctl -p sonata metadata --format '{{artist}} — {{title}} | static={{mpris:artUrl}} | motion={{sonata:motionArtUrl}}'

# Read only the animated cover URI cleanly via busctl
busctl --user get-property org.mpris.MediaPlayer2.sonata \
  /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Metadata
```

In **Quickshell**, read `player.metadata["sonata:motionArtUrl"]` directly into a looping `Video` / `MediaPlayer` component (falling back to `player.trackArtUrl` when unset).

---

## 2. Live Runtime Telemetry Files (`$XDG_RUNTIME_DIR/sonata/`)

Both `sonata-desktop` and `sonata` atomically write pretty-formatted JSON (`*.json.tmp` renamed into place) to `$XDG_RUNTIME_DIR/sonata/` (falling back to `$XDG_CACHE_HOME/sonata/` if `XDG_RUNTIME_DIR` is unset), and remove both files cleanly on exit:

- **`$XDG_RUNTIME_DIR/sonata/now_playing.json`**: Playback telemetry, artwork URIs (`static` + `motion`), ambient accent hex/RGB, and the active/surrounding synced lyrics lines with active word progress.
- **`$XDG_RUNTIME_DIR/sonata/lyrics.json`**: Full structured lyrics sheet for the current track (every line, timestamps in milliseconds, per-word timestamps, duet/background flags, plus active translations and romanizations).

### `$XDG_RUNTIME_DIR/sonata/now_playing.json` Schema (`schema_version = 2`)

Top-level flat fields (`status`, `title`, `artist`, `text`, `alt`, `class`, `tooltip`, `position`, `duration`, `progress_percent`, `lyric`, `art_path`, `art_url`, `motion_art_path`, `motion_art_url`) are preserved alongside structured nested objects:

```json
{
  "schema_version": 2,
  "client": "desktop",
  "status": "Playing",
  "track_id": "...",
  "title": "Track Title",
  "artist": "Artist Name",
  "album": "Album Name",
  "source": "YTM",
  "quality": "AAC · 256 kbps",
  "is_playing": true,
  "is_paused": false,
  "liked": true,
  "position_secs": 42,
  "duration_secs": 215,
  "remaining_secs": 173,
  "position": "0:42",
  "duration": "3:35",
  "remaining": "-2:53",
  "progress_ratio": 0.195,
  "progress_percent": 19,
  "lyric": "Current active lyric line",
  "art_path": "/home/user/.cache/sonata/art/<sha1>.png",
  "art_url": "https://...",
  "motion_art_path": "/home/user/.cache/sonata/art/motion_<sha1>.mp4",
  "motion_art_url": "file:///home/user/.cache/sonata/art/motion_<sha1>.mp4",
  "text": "Artist Name — Track Title",
  "alt": "playing",
  "class": "playing",
  "tooltip": "...",
  "updated_at": 1795725000,
  "playback": {
    "status": "Playing",
    "position_ms": 42300,
    "duration_ms": 215000,
    "remaining_ms": 172700,
    "progress_ratio": 0.1967,
    "progress_percent": 19
  },
  "artwork": {
    "static_path": "/home/user/.cache/sonata/art/<sha1>.png",
    "static_url": "https://...",
    "motion_path": "/home/user/.cache/sonata/art/motion_<sha1>.mp4",
    "motion_url": "file:///home/user/.cache/sonata/art/motion_<sha1>.mp4",
    "ambient_rgb": [16, 185, 129],
    "ambient_hex": "#10b981"
  },
  "lyrics": {
    "state": "Synced",
    "is_synced": true,
    "line_count": 48,
    "active_index": 12,
    "current": {
      "index": 12,
      "text": "Current active lyric line",
      "start_ms": 41200,
      "end_ms": 44800,
      "progress": 0.305,
      "is_counter_voice": false,
      "is_background": false,
      "translation": "Translated line when enabled",
      "romanization": "Romanized line when enabled",
      "active_word": {
        "index": 2,
        "text": "lyric",
        "start_ms": 42050,
        "end_ms": 42600,
        "progress": 0.454
      }
    },
    "previous": {
      "index": 11,
      "text": "Previous line",
      "start_ms": 37000,
      "end_ms": 41200,
      "is_counter_voice": false,
      "is_background": false,
      "translation": null,
      "romanization": null
    },
    "next": {
      "index": 13,
      "text": "(Backing vocal)",
      "start_ms": 44900,
      "end_ms": 47100,
      "is_counter_voice": true,
      "is_background": true,
      "translation": null,
      "romanization": null
    }
  }
}
```

### `$XDG_RUNTIME_DIR/sonata/lyrics.json` Schema

Contains the complete track lyrics sheet for rich synced overlays:

- `schema_version` (`2`), `client` (`"desktop"` / `"tui"`), `track_id`, `title`, `artist`
- `state` (`"Idle"`, `"Loading"`, `"Synced"`, `"Plain"`, `"NotFound"`), `is_synced`, `active_index`
- `lines`: array of `{ index, text, start_ms, end_ms, is_counter_voice, is_background, translation, romanization, words: [{ index, text, start_ms, end_ms }] }`

### Example Waybar Module

`now_playing.json` already provides `text`, `alt`, `class`, and `tooltip`, so Waybar can consume it directly:

```jsonc
"custom/sonata": {
  "exec": "cat \"$XDG_RUNTIME_DIR/sonata/now_playing.json\" 2>/dev/null || echo '{\"text\":\"\",\"class\":\"stopped\"}'",
  "return-type": "json",
  "interval": 1,
  "max-length": 48,
  "on-click": "playerctl -p sonata play-pause",
  "on-scroll-up": "playerctl -p sonata next",
  "on-scroll-down": "playerctl -p sonata previous"
}
```

---

## 3. Sonata Connect (LAN Remote Control & Queue Handoff)

`sonata-desktop` and `sonata` (TUI) host a local LAN discovery broadcaster and HTTP/JSON control server when Sonata Connect is enabled:

- **UDP discovery broadcast**: port `45389` (`ConnectBeacon` JSON payload every ~2 seconds).
- **HTTP/JSON control server**: binds TCP port `45390` by default (automatically falling back to an ephemeral port if `45390` is already held by another local Sonata instance).
- **Endpoints**:
  - `GET /connect/info` — device identity, protocol version, and PIN requirement flag.
  - `POST /connect/pair` — 6-digit PIN pairing exchange returning the per-device 64-hex HMAC token.
  - `GET /connect/state` — live playback snapshot (`playing`, `position_secs`, `duration_secs`, `volume` `0..=100`, active track metadata, and queue).
  - `POST /connect/command` — HMAC-SHA1 authenticated (`X-Sonata-Device-Id`, lowercase hex `X-Sonata-Signature` over raw request body) `snake_case` commands (`play`, `pause`, `next`, `prev`, `seek_absolute`, `set_volume`, `transfer_queue`).

On Linux hosts with an active firewall (such as `ufw`), allow LAN TCP traffic to port `45390`:

```sh
sudo ufw allow from 192.168.1.0/24 to any port 45390 proto tcp comment 'Sonata Connect'
```
