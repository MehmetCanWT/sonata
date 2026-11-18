# Sonata

A fast, modular Rust terminal music player for **YouTube Music** and **Spotify** with automatic browser cookie extraction, instant playback with background Hi-Fi upgrades, direct Kitty HD album artwork, synced lyrics, persistent SQLite caching, and Hyprland-ready glass transparency — **no `systemd` services or background daemons required**.

![Sonata Main Player](assets/main-player.png)

## Screenshots

| Hyprland Glass Transparency | Mini Mode (Art + Lyrics) | Compact Mini Player |
|---|---|---|
| ![Hyprland Glass](assets/hyprland-glass.png) | ![Mini Mode](assets/mini-lyrics.png) | ![Mini Player](assets/mini-player.png) |

## Features

- **Zero-Config Browser Auth**: Automatically extracts session cookies from Zen Browser, Firefox, LibreWolf, Floorp, Chrome, Chromium, Brave, Vivaldi, and Edge.
- **YouTube Music & Spotify (No API Keys)**:
  - Two-way YouTube Music library sync: browse playlists and Liked Music, like/unlike tracks (`y`), add/remove tracks (`a` / `x`), and create playlists (`c`).
  - Keyless Spotify library loading (`sp_dc`) and instant public Spotify playlist URL import (`i`).
  - Switch active providers anytime in Settings (`S`): **Both (YTM + Spotify)**, **YouTube Music Only**, or **Spotify Only**.
- **Instant Playback + Background Hi-Fi Upgrade**:
  - Starts streaming immediately via YouTube Music (`yt-dlp`), then upgrades seamlessly in the background to 320kbps AAC (JioSaavn bit-exact match) from the current playback timestamp without interrupting listening.
  - Live-switchable `mpv` audio DSP presets (`e`): `Hi-Res Crystal` (SoXR 96kHz + Crystalizer + Stereo + Limiter), `Spatial Wide`, `Warm Bass`, and `Bit-Perfect`.
- **Persistent SQLite Disk Cache**:
  - Caches resolved stream URLs (respecting YouTube `expire=` timestamps) and playlist tracks in `~/.cache/sonata/sonata_cache.db` (WAL mode) for instant startup and playlist switching.
- **Pixel-Perfect HD Album Art**:
  - Direct chunked Kitty Graphics Protocol rendering (optimized for Ghostty, Kitty, WezTerm, Foot), with Sixel/iTerm2/Halfblock support.
- **Real-Time Synced Lyrics**:
  - Multi-source fetcher (LRCLIB, KuGou, YouTube Music native, Genius) with local caching and high-contrast pure-white active line highlighting.
- **Hyprland Glass Transparency & Custom Themes**:
  - Transparent TUI background (`Color::Reset`) plus automatic Hyprland active & inactive window opacity (`50%`–`100%` in `10%` steps via `hl.dispatch(hl.dsp.window.set_prop(...))`, restored to `1.0` on exit).
  - Ships with `Catppuccin Mocha`, `Ghostty Dark`, `Tokyo Night`, `Gruvbox Dark`, `Rosé Pine`, and `Nord`, plus custom `.toml` themes in `~/.config/sonata/themes/*.toml`.
- **Full Mouse Support & 3-Tier Responsive Mini Mode**:
  - Click tracks/playlists/buttons, scroll lists and volume, seek on the progress bar, and double-click Album Art (or press `m`) to toggle Mini Mode.

## Keybindings & Mouse

| Key / Input | Action |
|---|---|
| `Tab` / `h` / `l` | Switch between Sidebar and Track List |
| `↑` / `↓` / `j` / `g` / `G` | Navigate lists |
| `/` | Search YouTube Music & Spotify |
| `Enter` | Open playlist / Play selected track |
| `Space` | Play / Pause |
| `n` / `p` | Next / Previous track |
| `<` / `>` | Seek -5s / +5s |
| `-` / `+` | Volume down / up |
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

**Mouse**: Left-click to select/play tracks or control mini-player buttons, click the progress bar to seek, scroll over lists to navigate, and scroll over the player bar to adjust volume.

## Custom Themes & Transparency Config

Create a `.toml` file under `~/.config/sonata/themes/my-theme.toml`:

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

Global settings are stored in `~/.config/sonata/config.toml` and can also be adjusted live in the Settings modal (`S`).

## External Integrations (`now_playing.json`)

Sonata atomically exports live playback state to `$XDG_RUNTIME_DIR/sonata/now_playing.json` (fallback: `~/.cache/sonata/now_playing.json`) for Waybar, Hyprlock, Eww, or custom scripts (`status`, `title`, `artist`, `album`, `source`, `quality`, `position_secs`, `duration_secs`, `position`, `duration`, `lyric`, `art_path`, `art_url`, `updated_at`), and removes the file on exit.

## Requirements

- `mpv` (audio playback & DSP filters)
- `yt-dlp` and `nodejs` (stream resolution)
- A terminal with Kitty Graphics Protocol support (such as **Ghostty**, **Kitty**, or **WezTerm**) for HD album art

## Installation

### From Source
```bash
cargo build --release
install -Dm755 target/release/sonata ~/.local/bin/sonata
```

### Arch / CachyOS (`PKGBUILD`)
```bash
makepkg -si
```
