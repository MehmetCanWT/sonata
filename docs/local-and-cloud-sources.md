# Local Music, Personal Only Mode & Self-Hosted Cloud Sources

Sonata supports offline local audio libraries and four self-hosted cloud server protocols (**Jellyfin**, **Subsonic / OpenSubsonic / Navidrome**, **WebDAV**, and **SMB2/SMB3**) alongside optional **Personal Only Mode** network isolation.

---

## Table of Contents

- [Sources & Modules Overview](#sources--modules-overview)
- [Personal Only Mode Isolation](#personal-only-mode-isolation)
- [Recommended Folder Structure](#recommended-folder-structure)
- [How Metadata Is Resolved per Source](#how-metadata-is-resolved-per-source)
- [Artist Folders (`artist.json` / `artist.nfo`) & Artwork](#artist-folders-artistjson--artistnfo--artwork)
- [Album Artwork & Thumbnail Priority](#album-artwork--thumbnail-priority)
- [Lyrics Resolution Order](#lyrics-resolution-order)
- [Real-Time Audio Quality Badges (`kbps` & `itag`)](#real-time-audio-quality-badges-kbps--itag)
- [Troubleshooting & Re-Scanning](#troubleshooting--re-scanning)

---

## Sources & Modules Overview

Open **Settings -> Sources & Modules** (Android, Linux Desktop, and Terminal TUI) to configure:

| Source / Module | Protocol / Backing | Transport & Capabilities |
|---|---|---|
| **Local Music** | Device Storage / XDG Music | Android `MediaStore` + SAF folder picker + junk exclusion filters; Linux pure-Rust tag & cover scanner (`sonata-core/src/providers/local.rs`) |
| **Jellyfin** | Jellyfin REST API | Server-managed artists, albums, playlists, artwork, native `/Audio/{id}/Lyrics`, direct `?static=true` lossless/compressed streams |
| **Subsonic** | Subsonic / OpenSubsonic / Navidrome | Salted RFC 1321 MD5 auth, artists, albums, playlists, cover art, bitrate metadata |
| **WebDAV** | RFC 4918 HTTP/HTTPS (`Basic` / `Bearer`) | `PROPFIND` folder/album/artist crawl, HTTP `Range` FLAC tag reader, sidecar artwork & `.ttml`/`.lrc` lyrics, `0o600`/`0o700` cover cache |
| **SMB** | SMB2 / SMB3 (`smbj` on Android, GVFS/mount on Linux) | Direct TCP port `445` streaming on Android (no OS mount required), FLAC header & `PICTURE` reader, sidecar cover & `.ttml`/`.lrc` lyrics |
| **JioSaavn (Module)** | Optional Streaming Module | Opt-in 320 kbps AAC matching; disabled automatically in Personal Only Mode |

Every configured server includes **Health Check** (latency in `ms`), **Speed Test** (real byte-range throughput in `Mbps`), and **Rescan** controls to clear cached metadata and rebuild source indexes on demand.

---

## Personal Only Mode Isolation

Enabling **Personal Only Mode** seals Sonata cleanly to local device files and user-configured servers without deleting any signed-in accounts, cookies, or preferences:

1. **Zero Online Catalog Requests**: Home, Explore, Radio, Search, and Recommendations stop querying YouTube Music, Spotify metadata APIs, and JioSaavn. Home displays only **Recently Played**, **Pinned** local/personal playlists, **Local Music**, and active server shelves.
2. **Zero External Autoplay & Quality Upgrades**: Tracks playing before or during Personal Only Mode never pull external recommendations or swap streams via YouTube Music / JioSaavn upgrade hooks.
3. **Zero External Lyrics Leakage**: Lyrics lookups check only sidecar `.ttml`/`.lrc`/`.txt` files, embedded FLAC `LYRICS`/`UNSYNCEDLYRICS` tags, and Jellyfin's own server `/Audio/{id}/Lyrics` endpoint. External providers (LRCLIB, BetterLyrics, etc.) and external animated artwork (Canvas) / Music Video resolvers are never queried.
4. **Isolated Guest vs. Account History**: Signed-out / Personal Only playback history is isolated from signed-in YouTube Music `Last Played` shelves so tracks played offline never pollute another account view, and items can be removed directly from **Last Played** via the track context menu.
5. **Non-Destructive Re-Enable**: Turning Personal Only Mode off immediately restores your preserved YouTube Music session, JioSaavn module toggle, and audio settings.

---

## Recommended Folder Structure

For **SMB**, **WebDAV**, and **Local Music** folder libraries, organize files in a clean artist-first hierarchy:

```text
Music/                          # Or share root such as share/Music/
└── Eminem/                     # Artist folder (optional artist.jpg + artist.json / artist.nfo)
    ├── artist.jpg              # Artist portrait / hero photo
    ├── artist.json             # Optional artist subtitle, genre, origin, biography
    └── The Slim Shady LP/      # Album folder
        ├── album.json          # Optional album / albumArtist override
        ├── cover.jpg           # Album cover art (or folder.jpg / front.jpg)
        ├── 01 - Public Service Announcement.flac
        ├── 02 - Guilty Conscience.flac
        ├── 02 - Guilty Conscience.ttml  # Optional Apple-style TTML (word-synced + x-bg)
        ├── 02 - Guilty Conscience.lrc   # Synced LRC / enhanced karaoke lyrics
        └── CD2/                # Multi-disc subfolders are folded back into the parent album
            └── 01 - Bonus Track.flac
```

### Path Inference & Container Stripping

When embedded tags (such as FLAC `VORBIS_COMMENT`) are missing, Sonata infers `(Artist, Album)` from directory segments while avoiding false names:

- **Leading container folders skipped automatically**: `share`, `music`, `audio`, `songs`, `library`, `media`, `flac`, `mp3`, `lossless`, `public`, `files`.
- **Multi-disc folders folded automatically**: Trailing segments matching `CD1`, `CD 2`, `Disc 1`, `Disk 2`, etc. resolve back to the parent album folder (`MMLP/CD1/track.flac` -> Album: `MMLP`, Artist: parent folder).
- **Safe fallbacks**: If no artist folder exists above the album folder, artist falls back to **`Unknown Artist`** (and never uses your server's display name). Missing album names fall back to **`Unknown Album`**.

---

## How Metadata Is Resolved per Source

### Jellyfin
Jellyfin manages your library metadata on the server side:
- **Track & Album Tags**: Read directly from Jellyfin (`Name`, `Artists`, `AlbumArtist`, `Album`, `ProductionYear`, `RunTimeTicks`, `MediaSources[].BitRate` & `Codec`).
- **Artwork**: Loaded from `/Items/{id}/Images/Primary` using the server's `ImageTags.Primary` hash.
- **Lyrics**: Fetched natively from Jellyfin `GET /Audio/{itemId}/Lyrics` (supports synced `Start` 100ns tick entries formatted as LRC as well as plain text lines).

### Subsonic / OpenSubsonic / Navidrome
- Authenticates with per-request salted MD5 tokens (`t = md5(password + salt)` + `s = salt`) — never transmitting cleartext passwords in query strings.
- Reads artist, album, playlist, `coverArt`, `suffix` codec, and `bitRate` directly from OpenSubsonic JSON endpoints.

### SMB (`SMB2` / `SMB3`)
- On Android, Sonata uses a native `smbj` TCP client + seekable Media3 `SmbDataSource` (`sonata-smb://`) over port `445` (no kernel mount or third-party file manager required).
- For `.flac` files, Sonata reads the header block (`STREAMINFO`, `VORBIS_COMMENT`, `PICTURE`) to extract exact `TITLE`, `ARTIST`, `ALBUMARTIST`, `ALBUM`, sample rate, bit depth, duration, bitrate, embedded `PICTURE` cover art, and `LYRICS`/`UNSYNCEDLYRICS`.
- If a tag is omitted, Sonata checks folder `album.json` / `metadata.json` and then folder hierarchy.

### WebDAV
- Browses directories with RFC 4918 `PROPFIND` (`Depth: 1`), and crawls albums/artists using `Depth: infinity` with automatic `Depth: 1` BFS fallback when servers forbid infinite depth.
- Reads `.flac` headers using bounded HTTP `Range: bytes=0-131071` GET requests with configured `Basic` or `Bearer` auth headers to populate full tags (`TITLE`, `ARTIST`, `ALBUM`, `duration`, `bitrate_kbps`, `16-bit/44.1kHz`, embedded `LYRICS`).
- Caches password-protected WebDAV artwork locally under `<cache_dir>/webdav_art/` with private `0o700`/`0o600` permissions so UI image loaders display covers securely without embedding credentials in URLs.

### Local Music (Android Junk Exclusion & Folder Picker)
- On Android, **Exclude Non-Music Audio** is enabled by default in **Settings -> Sources & Modules** to filter out voice notes, call recordings, ringtones, notifications, WhatsApp/Telegram audio, alarm tones, OEM sample tracks (`Over the Horizon`), and short clips (`< 30s`, configurable).
- Use **Music Folders** in the same section to add or restrict scan roots via Android's Storage Access Framework (`OpenDocumentTree`), or leave empty to scan standard music directories.

---

## Artist Folders (`artist.json` / `artist.nfo`) & Artwork

To give self-hosted **SMB** and **WebDAV** artists rich **Artist Pages** (hero portrait, genre/origin subtitle, biography, albums, and tracks — even in Personal Only Mode), place metadata files directly inside `<Artist>/`:

### `artist.json` (also `info.json` / `metadata.json`)

```json
{
  "name": "Eminem",
  "genre": "Hip-Hop / Rap",
  "origin": "Detroit, Michigan",
  "bio": "Marshall Bruce Mathers III is an American rapper, songwriter, and record producer."
}
```

Supported JSON fields:
- `name`: Override display artist name.
- `subtitle`: Optional explicit subtitle line; if omitted, Sonata builds `<genre> · <origin>`.
- `genre`, `origin` / `country` / `formed`: Combined into the subtitle line.
- `bio` / `biography` / `description`: Artist biography displayed on the Artist Page.

### `artist.nfo` (Kodi / Jellyfin XML Compatible)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<artist>
  <name>Eminem</name>
  <genre>Hip-Hop / Rap</genre>
  <formed>Detroit, Michigan</formed>
  <biography>Marshall Bruce Mathers III is an American rapper, songwriter, and record producer.</biography>
</artist>
```

### Album Sidecar Override (`album.json` / `metadata.json`)

```json
{
  "albumArtist": "Eminem",
  "album": "The Slim Shady LP"
}
```

### Artist Portrait Priority

Inside the artist directory, Sonata looks for the first matching image (`jpg`, `jpeg`, `png`, `webp`, `bmp`, `gif`, `avif`):
1. `artist.<ext>`
2. `folder.<ext>`
3. `avatar.<ext>`
4. `thumb.<ext>`
5. `cover.<ext>`
6. `front.<ext>`

---

## Album Artwork & Thumbnail Priority

When resolving track and album artwork on **SMB**, **WebDAV**, or **Local Music**:

1. **Per-track image** matching the audio filename stem (`02 - Guilty Conscience.jpg`)
2. **Album folder image** in priority order: `cover.*`, `folder.*`, `front.*`, `album.*`, `artwork.*`, `thumb.*`, `artist.*`, `avatar.*`, or any image file in the album folder
3. **Embedded FLAC `PICTURE` block** (cached cleanly on disk when no folder image exists)

---

## Lyrics Resolution Order

For self-hosted server tracks, Sonata checks server-local sources before any external lyrics service:

1. **Sidecar `.ttml` (Timed Text Markup Language)** in the same directory as the audio file:
   - Exact filename stem match (`02 - Guilty Conscience.ttml`) or clean title stem match (`Guilty Conscience.ttml`), falling back to `lyrics.ttml` or a single `.ttml` in a single-track folder.
   - Supports syllable/word-level `<span begin end>` karaoke sweeps, `<span ttm:role="x-bg">` background vocals, and `<ttm:agent type="person">` duet alignment.
2. **Sidecar `.lrc` / Enhanced Karaoke `.lrc`** in the same directory as the audio file:
   - Exact filename stem match: `02 - Guilty Conscience.lrc`
   - Clean title stem match (without track number prefix): `Guilty Conscience.lrc`
   - Supports standard `[mm:ss.xx]` timestamps, `[offset:ms]`, multi-timestamps `[00:12.00][01:02.00]`, word-level `<mm:ss.xx>` karaoke tags, and automatic splitting of trailing inline `(background vocals)` into separate nested lines.
3. **Sidecar `.txt` / Generic `lyrics.lrc`** (`<stem>.txt`, `<clean-title>.txt`, `lyrics.lrc`, or a single `.lrc` in a single-track folder).
4. **Embedded Vorbis Comments (`LYRICS` / `UNSYNCEDLYRICS`)** inside `.flac` files.
5. **Jellyfin Server Lyrics** (`GET /Audio/{itemId}/Lyrics`).
6. **External Providers** (*Online Mode only* — skipped completely in Personal Only Mode): `lrc.red` TTML, BetterLyrics, LyricsPlus v2, LRCLIB, KuGou KRC, NetEase YRC, QQ Music, YouTube Music, Genius.

---

## Real-Time Audio Quality Badges (`kbps` & `itag`)

Sonata strips server protocol names (`SMB`, `Jellyfin`, `Subsonic`, `WebDAV`) from quality pills so playback UI shows only verified technical audio telemetry:

| Stream / Source | Typical Badge | Notes |
|---|---|---|
| Lossless 16-bit FLAC | `FLAC · 888 kbps · 16-bit/44.1kHz` | Bitrate measured live from `(file_bytes * 8) / duration_ms` via `SmbDataSource`, HTTP `Content-Length`/`Content-Range`, `mpv file-size`, or FLAC `STREAMINFO` |
| Hi-Res 24-bit FLAC | `FLAC · 2480 kbps · 24-bit/96kHz` | 24-bit PCM sample format read directly from decoder / `mpv audio-params` |
| YouTube Music (`TVHTML5` Extreme) | `OPUS · 256 kbps · Extreme` / `AAC · 256 kbps · Extreme` | Unlocked via `itag=774` (Opus Extreme ~256–321 kbps) or `itag=141` (AAC 256 kbps) |
| YouTube Music (High / Standard) | `OPUS · 137 kbps · HQ` / `AAC · 130 kbps · Standard` | Unlocked via `itag=251` (Opus) / `itag=140` (AAC) |
| JioSaavn Module | `AAC · 320 kbps · JioSaavn` | Hot-swapped only when strictly higher bitrate than base online stream; never used on local/server tracks |

Local and self-hosted tracks **never** trigger background YouTube Music or JioSaavn quality upgrades.

---

## Troubleshooting & Re-Scanning

- **Edited tags, added `artist.json`, or dropped in a new `.lrc` / `cover.jpg`?** Tap **Rescan** right next to **Speed Test** on the server card in **Settings -> Sources & Modules** (or pull/re-scan Local Music) to evict cached folder snapshots and artwork caches immediately.
- **No tracks appearing on Android SMB?** Verify **Host/IP**, **Port** (`445`), and **Share Name** separately in the server editor (e.g. Share `music`, optional Subfolder `Lossless`), and tap **Test Connection** to inspect the live `SMB2/3 OK` latency diagnostic.
- **Protected WebDAV thumbnails not loading?** Ensure your WebDAV server permits `PROPFIND` (`Depth: 1`) and `GET` with `Basic` or `Bearer` auth; Sonata downloads thumbnails into its private cache rather than passing unauthenticated URLs to image decoders.
