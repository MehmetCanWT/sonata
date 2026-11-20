# Architecture & Refactoring Plan: Login-Free YouTube Music Core

## Objective
Refactor the music player to operate exclusively on a **login-free, client-side, SQLite-cached architecture** centered on **YouTube Music (`WEB_REMIX`)**. Completely remove JioSaavn dependencies, implement strict metadata matching for high-resolution cover art and synced lyrics, and fix audio quality badge accuracy.

---

## 1. Database Additions (SQLite Schema Changes)
Do not drop or recreate existing user tables. Add only the following columns to the existing tracks/songs table (or link via a 1:1 enrichment table using `track_id`):

| Column Name | SQLite Type | Default Value | Description & Purpose |
| :--- | :--- | :--- | :--- |
| `cover_source` | `TEXT` | `'none'` | Tracks where the cached cover came from: `'apple'`, `'ytm'`, or `'none'`. |
| `synced_lyrics` | `TEXT` | `NULL` | Stores time-synced lyrics in `[mm:ss.xx]` LRC format. |
| `plain_lyrics` | `TEXT` | `NULL` | Fallback un-synced lyrics text (e.g., from YTM `MPLYt_`). |
| `lyrics_status` | `TEXT` | `'NONE'` | Cache state: `'FOUND'`, `'NOT_FOUND'`, or `'NONE'` (prevents repeated 404 API calls). |
| `average_bitrate` | `INTEGER` | `0` | True average bitrate in `kbps` (replaces peak `bitrate` values). |
| `updated_at` | `INTEGER` | `0` | Unix timestamp (seconds) used for negative-cache TTL validation. |

### Legacy Data Invalidation Rule
* On migration/startup, nullify any existing `cover_url` containing `%saavncdn%` or `%jiosaavn%` and ensure `cover_source` is set to `'none'` so they re-resolve lazily on next playback.

---

## 2. Audio Engine & Stream Selection Refactoring

### Remove JioSaavn
* Delete all JioSaavn API clients, search providers, URL decryptors, and fallback resolvers from the codebase.

### YouTube Music (`WEB_REMIX`) Stream Rules
1. **Catalog & Search:** Query YouTube Music (`youtubei/v1/search`) with the **"Songs" filter** (`params: "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D"`) to prioritize official studio masters (Art Tracks / Topic uploads) over music videos.
2. **Stream Priority Order:** Select audio streams from `adaptiveFormats` strictly in this priority order:
   * `itag 774` (256 kbps OPUS, 48 kHz — if user session is present)
   * `itag 141` (256 kbps AAC, 44.1 kHz — if user session is present)
   * **`itag 251`** (~130–160 kbps VBR OPUS, 48 kHz — **default login-free ceiling**)
   * `itag 140` (128 kbps AAC — fallback only)
3. **Playback Enhancements:**
   * Read `playerConfig.audioConfig.loudnessDb` from the player JSON and apply volume normalization.
   * Pre-resolve the next queued track's stream URL ~15 seconds before the current track ends to guarantee gapless playback.
   * Keep chunk-based progressive audio caching enabled (`SimpleCache` or equivalent).

---

## 3. Quality Badge & Bitrate Display Logic
* **Bitrate Value:** Parse `averageBitrate` (or calculate `contentLength * 8 / durationSeconds`) instead of reading the peak `bitrate` field.
* **Badge Assignment Rules:**
  * **`Hi-Res` / `Lossless`:** Strictly reserved for local `.flac` / `.wav` files where `bitDepth >= 24` and `sampleRate >= 48000` (or `16-bit / 44.1kHz+` for standard `Lossless`). **Never** display `Hi-Res` on YouTube streams.
  * **`Extreme`:** `itag 774` or `itag 141` (`256 kbps`).
  * **`HQ`:** `itag 251` (`OPUS 48kHz`).
  * **`Standard`:** `averageBitrate <= 128 kbps` (e.g., `itag 140`).

---

## 4. High-Res Cover Art Pipeline (Apple Music Primary → YTM Fallback)
Execute this pipeline **lazily on playback** (with a **2–3 second debounce** after playback starts to avoid rate limits during rapid skipping), and **only** when `cover_source == 'none'`.

### Step 1: Query iTunes Search API (Client-Side)
* Sanitize track title by stripping brackets/parentheses like `(Official Audio)`, `[Lyrics]`, `(Visualizer)`.
* Request: `GET https://itunes.apple.com/search?term={encodedArtist+encodedCleanTitle}&entity=song&limit=5`

### Step 2: Strict Candidate Validation (Prevent Random Covers)
Iterate through `results` and pick the first item that satisfies **all** conditions:
1. **Duration Match:** `abs(item.trackTimeMillis - ytDurationMs) <= 4000` (4 seconds tolerance).
2. **Blacklist Filter:** Reject if `artistName` or `trackName` contains `"Karaoke"`, `"Tribute"`, `"Ringtone"`, or `"Made Famous"`.
3. **Modifier Parity:** Version keywords (`remix`, `live`, `acoustic`, `instrumental`, `sped up`, `slowed`) must match between the YouTube title and the iTunes candidate.
4. **Text Similarity:** Normalized string similarity must be `>= 0.75` for artist name and `>= 0.80` for track title.

### Step 3: Resolution & Persistence
* **If valid match found:** Replace `100x100bb.jpg` in `artworkUrl100` with **`1200x1200bb.jpg`** (force `https://`), save to SQLite, and set `cover_source = 'apple'`.
* **If no match / API error:** Take the YouTube Music thumbnail URL, replace its size suffix (`=w...-h...`) with **`=w1200-h1200-l90-rj`**, save to SQLite, and set `cover_source = 'ytm'`.

---

## 5. Synced Lyrics Pipeline (LRCLIB Primary → YTM Fallback)
Execute lazily when lyrics are requested or during track load:

1. **SQLite Cache Check:**
   * If `lyrics_status == 'FOUND'`, return `synced_lyrics` (or `plain_lyrics`) immediately without network calls.
   * If `lyrics_status == 'NOT_FOUND'` and `(currentUnixTime - updated_at) < 604800` (7 days), abort network request.
2. **LRCLIB Exact Match (Primary):**
   * Include a custom `User-Agent` header identifying the app.
   * Request: `GET https://lrclib.net/api/get?artist_name={artist}&track_name={cleanTitle}&album_name={album}&duration={durationSeconds}`
3. **LRCLIB Search (Secondary):**
   * On `404`, query `GET https://lrclib.net/api/search?q={artist+cleanTitle}` and pick the top result where `abs(result.duration - durationSeconds) <= 4`.
4. **YouTube Music Fallback:**
   * If LRCLIB returns no valid match, extract the `MPLYt_` browseId from YouTube Music's `next` response and fetch plain text lyrics via `youtubei/v1/browse`.
5. **Persist to SQLite:**
   * Save `synced_lyrics` / `plain_lyrics`, update `lyrics_status` (`'FOUND'` or `'NOT_FOUND'`), and set `updated_at = currentUnixTime`.

---

## 6. Linux Companion Tooling Standard (If Applicable)
If maintaining local test scripts or desktop companion utilities:
* **Player (`mpv`):** Configure `ao=pipewire`, `gapless-audio=yes`, and `mpv-lrc` for local `.lrc` synchronization.
* **Downloader (`yt-dlp`):** Never transcode OPUS to MP3. Copy raw `bestaudio` stream, crop embedded thumbnails to 1:1 square via FFmpeg, and fetch matching `.lrc` files via `syncedlyrics`.