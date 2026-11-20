# Sonata

A fast, modular Rust music player workspace (**Terminal UI + experimental Wayland/Hyprland GPUI Desktop**) for **YouTube Music** and **Spotify** with browser cookie authentication, fast playback with background Hi-Fi upgrades, HD album artwork, synced lyrics, a shared bounded cache, MPRIS2, and `now_playing.json` integration.

![Sonata Main Player](assets/main-player.png)

> [!WARNING]
> **Client Status**
> - **`sonata` (TUI)** — **Stable, feature-complete, and recommended for daily use.**
> - **`sonata-desktop` (GPUI Desktop GUI)** — **Broken / experimental (under active development).** Expect incomplete features, rough edges, and instability; it is not ready for daily use yet.

## One-Line TUI Install (No Clone Required)

Install **`sonata` (TUI)** without manually cloning the repository:

```sh
curl -fsSL https://raw.githubusercontent.com/MehmetCanWT/sonata/main/install.sh | sh
```

The [`install.sh`](install.sh) script installs system runtime dependencies (`mpv`, `yt-dlp`, `nodejs`, `curl`, `git`, C toolchain) via `pacman`, `apt-get`, `dnf`, or `brew`, installs Rust via `rustup` if missing, builds `sonata-tui` cleanly without leaving a persistent repository checkout behind, and ensures `~/.local/bin` and `~/.cargo/bin` are in `~/.profile`.

### Direct Cargo Git Install

If you already have Rust, `mpv`, `yt-dlp`, and `nodejs` installed:

```sh
cargo install --git https://github.com/MehmetCanWT/sonata.git --locked sonata-tui
```

### Arch Linux / AUR (`makepkg`)

Install the stable terminal client (`PKGBUILD`):

```sh
makepkg -si
```

Install the experimental desktop client (`PKGBUILD.desktop`):

```sh
makepkg -p PKGBUILD.desktop -si
```

## Documentation

- [`docs/architecture.md`](docs/architecture.md) — Workspace crate map, runtime threading & IPC model, and shared filesystem layout.
- [`docs/how-it-works.md`](docs/how-it-works.md) — Browser cookie auth, fast-start + Hi-Fi upgrade pipeline, synced lyrics resolution, and fullscreen visual effects.
- [`docs/codebase-analysis.md`](docs/codebase-analysis.md) — Module size breakdown, cross-crate refactoring opportunities, and concurrency/security analysis.
- [`PRIVACY.md`](PRIVACY.md) — Local-only cookie handling, direct third-party requests, and cache/log storage policy.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) & [`CLAUDE.md`](CLAUDE.md) — Contributor workflow, module header conventions, and verification commands.
- [`CHANGELOG.md`](CHANGELOG.md) — Release history and unreleased changes.

## Workspace Architecture

Sonata is organized as a 4-crate Cargo workspace so clients share the same streaming, caching, authentication, lyrics, and audio core:

- `crates/sonata-core` — Shared Rust engine for browser cookie auth, YouTube Music & Spotify providers, `mpv` IPC playback, multi-source lyrics, themes, SQLite cache, and bounded media cache.
- `crates/sonata-tui` — **`sonata`** (stable): Ratatui terminal client with Kitty/Sixel/Halfblock album art, mouse-resizable panes, synced lyrics, and full library management.
- `crates/sonata-desktop` — **`sonata-desktop`** (experimental / under active development): Native Wayland GUI built with GPUI (`gpui-kit`), MPRIS2, and system tray support.
- `crates/sonata-android` — Reserved placeholder crate for a future Android client.

`sonata-tui` and `sonata-desktop` build independently against `sonata-core` using workspace aliases (`cargo build-tui` or `cargo build-desktop`) and share `~/.config/sonata/` and `~/.cache/sonata/`.

## Screenshots (TUI)

| Hyprland Glass Transparency | Mini Mode (Art + Lyrics) | Compact Mini Player |
|---|---|---|
| ![Hyprland Glass](assets/hyprland-glass.png) | ![Mini Mode](assets/mini-lyrics.png) | ![Mini Player](assets/mini-player.png) |

## Shared Engine Highlights

- **Zero-Config Browser Auth**: Automatically extracts session cookies from Zen Browser, Firefox, LibreWolf, Floorp, Chrome, Chromium, Brave, Vivaldi, and Edge.
- **YouTube Music & Spotify (No API Keys)**:
  - Two-way YouTube Music library sync: browse playlists and Liked Music, like/unlike tracks, add/remove tracks, and create playlists.
  - Keyless Spotify library loading (`sp_dc`) and instant public Spotify playlist URL import.
  - Switch active providers anytime in Settings: **Both (YTM + Spotify)**, **YouTube Music Only**, or **Spotify Only**.
- **Instant Playback + Background Hi-Fi Upgrade**:
  - Starts streaming immediately via YouTube Music (`yt-dlp`), then upgrades seamlessly via `mpv` JSON IPC (`loadfile` native `start` timestamp resume) to 320kbps AAC without interrupting playback, with automatic retry on expired stream URLs.
  - Consistent Shuffle lookahead & playback history, persisted Shuffle/Repeat/Volume settings, and live-switchable `mpv` audio DSP presets: `Hi-Res Crystal` (SoXR 96kHz + Crystalizer + Stereo + Limiter), `Spatial Wide`, `Warm Bass`, and `Bit-Perfect`.
- **Shared Cache & Hi-Res Prefetch**:
  - The TUI and desktop share artwork, audio, lyrics, and SQLite cache under `~/.cache/sonata/`.
  - Media cache defaults to 1 GB and can be set from Off to 10 GB in Settings. Off clears saved cache data and disables cache-dependent Hi-Fi staging.
  - SQLite uses WAL mode, schema versioning, expired-stream cleanup, and bounded cache eviction by least-recent access.
- **Real-Time Synced Lyrics**:
  - Multi-source fetcher (LRCLIB, KuGou, YouTube Music native, Genius) with strict title/artist/duration match validation, Enhanced LRC word-level timestamps, local caching, and synced active-line tracking.

## TUI Keybindings & Mouse (`sonata`)

| Key / Input | Action |
|---|---|
| `Tab` / `h` / `l` | Switch between Sidebar and Track List |
| `↑` / `↓` / `j` / `g` / `G` | Navigate lists |
| `H` / `L` | Resize focused pane width |
| `/` | Search YouTube Music & Spotify |
| `Enter` | Open playlist / Play selected track |
| `Space` | Play / Pause |
| `n` / `p` | Next / Previous track (follows Shuffle history) |
| `<` / `>` (or `,` / `.`) | Seek -5s / +5s |
| `-` / `+` (or `_` / `=`) | Volume down / up |
| `e` | Cycle Audio Enhancer DSP preset |
| `s` / `r` | Toggle Shuffle / Cycle Repeat (`Off` / `All` / `One`) |
| `y` | Like / Unlike track on YouTube Music |
| `a` / `x` (`Delete`) | Add to / Remove from YouTube Music playlist |
| `c` / `R` | Create YouTube Music playlist / Refresh account & cache |
| `i` | Import Spotify playlist URL or ID |
| `m` / Double-Click Art | Toggle Mini Mode |
| `S` | Open Settings (Providers, Hi-Fi, DSP, Transparency, Hyprland Opacity, Theme) |
| `t` | Open Theme Picker (live preview) |
| `k` / `?` | Open / Close Keybindings menu |
| `q` / `Ctrl+C` | Quit |

**Mouse**: Drag panel borders or the Album Art/Lyrics divider to resize; click or drag the `0–100%` volume slider and progress bar; click tracks/playlists/controls; double-click Album Art for Mini Mode; scroll lists, player bar volume, and inside Theme/Settings/Playlist modals.

## Desktop GUI (`sonata-desktop` — Experimental)

> [!NOTE]
> `sonata-desktop` is under active development and currently unstable. Features listed below reflect work-in-progress capabilities in `crates/sonata-desktop`.

- **YouTube Music Home & Public Feed**: Launches into a discovery view with Trending tracks, New Albums & Singles, and Community Playlists, plus browser-based YTM sign-in.
- **Modern Glass & Fullscreen Lyrics View**: Translucent Wayland surfaces tinted from the active album cover, with procedural wave background shaders, Gaussian depth-of-field lyric rendering, click-to-seek timestamps, and a compact Mini Player (`Ctrl+M`).
- **MPRIS2 & System Tray**: Exposes `org.mpris.MediaPlayer2.sonata` and a StatusNotifierItem tray menu.

### Desktop Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+F` | Focus search bar |
| `Space` | Play / Pause (when search/modals are unfocused) |
| `Left` / `Right` | Seek -5s / +5s |
| `Ctrl+L` | Switch between Library and Lyrics views |
| `Ctrl+M` | Toggle compact Mini Player mode |
| `Ctrl+,` | Open Settings & Theme selector |
| `Esc` | Close modal / Queue drawer / exit Mini Player |
| `Ctrl+Q` | Quit application cleanly (terminates background audio) |

### Optional Hyprland Blur Rule

```ini
windowrule = match:class ^(sonata-desktop)$, blur on
windowrule = match:class ^(sonata-desktop)$, ignore_alpha 0.25
```

## Custom Themes & Shared Config

Create a `.toml` file under `~/.config/sonata/themes/my-theme.toml` (shared by both clients):

```toml
transparent_bg = true
hyprland_opacity = 0.90

[colors]
bg = "#14161b"
surface = "#1a1d24"
fg = "#e6edf3"
muted = "#7d8590"
accent = "#79c0ff"
border = "#30363d"
highlight_bg = "#21262d"
highlight_fg = "#ffffff"
lyrics_active = "#79c0ff"
lyrics_inactive = "#6e7681"
success = "#56d364"
warning = "#e3b341"
error = "#f85149"
```

Global settings are stored in `~/.config/sonata/config.toml` and can also be edited live inside Settings.

## Logs

Runtime logs are written to `~/.cache/sonata/sonata.log`:

```sh
tail -f ~/.cache/sonata/sonata.log
```

Set `RUST_LOG=debug` before launching for resolver and `mpv` IPC details. The log is truncated to 10 MiB at startup and uses user-only permissions on Unix.

## External Integrations (`now_playing.json`)

Both `sonata` and `sonata-desktop` atomically export live playback state to `$XDG_RUNTIME_DIR/sonata/now_playing.json` (fallback: `~/.cache/sonata/now_playing.json`) for Waybar, Hyprlock, Eww, or custom scripts (`status`, `title`, `artist`, `album`, `source`, `quality`, `position_secs`, `duration_secs`, `position`, `duration`, `lyric`, `art_path`, `art_url`, `updated_at`), and remove the file on clean exit.

## Requirements

- `mpv` (audio playback & DSP filters)
- `yt-dlp` and `nodejs` (stream resolution)
- A terminal with Kitty Graphics Protocol support (such as **Ghostty**, **Kitty**, or **WezTerm**) for HD album art in `sonata`
- Wayland + Vulkan/OpenGL drivers (plus ALSA and D-Bus) for experimental `sonata-desktop`

## Building from Source

```sh
git clone https://github.com/MehmetCanWT/sonata.git
cd sonata

# Check or build only the stable TUI client
cargo check-tui
cargo build-tui

# Install the TUI binary locally
cargo install --locked --path crates/sonata-tui

# Or build the experimental desktop client
cargo check-desktop
cargo build-desktop
```

## License

Licensed under the [MIT License](LICENSE).
