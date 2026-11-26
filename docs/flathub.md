# Flatpak & Flathub Packaging Guide

Sonata provides an official Tauri v2 desktop + Ratatui TUI Flatpak definition (`dev.sonata.Desktop`) under `packaging/flatpak/`.

---

## Files

| Path | Purpose |
|---|---|
| `packaging/flatpak/dev.sonata.Desktop.yml` | `flatpak-builder` manifest targeting `org.gnome.Platform//47` + `org.freedesktop.Sdk.Extension.rust-stable` + `org.freedesktop.Sdk.Extension.node22` + bundled `mpv` (with LuaJIT + `libass`), `ffmpeg-full`, and `yt-dlp` |
| `packaging/flatpak/dev.sonata.Desktop.metainfo.xml` | AppStream metadata (`dev.sonata.Desktop`), screenshots, OARS `1.1` content rating, brand colors, and release changelog |
| `packaging/flatpak/flathub.json` | Flathub architecture configuration (`only-arches: ["x86_64"]`) |
| `scripts/generate-flatpak-cargo-sources.sh` | Generates `cargo-sources.json` from `Cargo.lock` for fully offline Flathub build workers |

---

## Sandbox Permissions Rationale

| Flag | Purpose |
|---|---|
| `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11`, `--device=dri` | Hardware-accelerated WebKitGTK / Tauri v2 rendering on Wayland and X11 |
| `--socket=pulseaudio`, `--filesystem=xdg-run/pipewire-0:ro` | Low-latency audio output via PipeWire / PulseAudio |
| `--share=network` | YouTube Music, self-hosted Jellyfin/Subsonic/WebDAV/SMB, lyrics, scrobbling, and Sonata Connect LAN handoff |
| `--own-name=org.mpris.MediaPlayer2.sonata` | MPRIS2 lockscreen & media-key controls |
| `--talk-name=org.freedesktop.Notifications` | Desktop track/status notifications |
| `--talk-name=org.kde.StatusNotifierWatcher` | System tray (`StatusNotifierItem`) integration |
| `--filesystem=xdg-run/app/com.discordapp.Discord:create` | Local Discord Rich Presence IPC inside sandboxed/host Discord sockets |
| `--filesystem=xdg-music:ro` | Read-only scanning of the user's XDG Music directory (`~/Music`) for Local Music mode |

---

## Validation & Local Build

Validate the AppStream metainfo and `.desktop` launcher locally:

```sh
appstreamcli validate --pedantic packaging/flatpak/dev.sonata.Desktop.metainfo.xml
desktop-file-validate assets/sonata-desktop.desktop
```

Generate offline Cargo sources from `Cargo.lock` before submitting a PR to Flathub (`flathub/flathub`):

```sh
./scripts/generate-flatpak-cargo-sources.sh packaging/flatpak/cargo-sources.json
```

When submitting to Flathub:
1. Replace the local `type: dir` source in `dev.sonata.Desktop.yml` with the tagged GitHub release tarball or commit (`url` + `sha256` / `tag` + `commit`) alongside `cargo-sources.json` (`- cargo-sources.json`).
2. Test a full sandboxed offline build and run Flathub's linter:

```sh
flatpak-builder --force-clean --sandbox --user --install-deps-from=flathub --repo=repo builddir packaging/flatpak/dev.sonata.Desktop.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/dev.sonata.Desktop.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```
