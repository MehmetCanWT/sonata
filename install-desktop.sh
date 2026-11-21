#!/bin/sh
set -eu

repo_url="${SONATA_REPO_URL:-https://github.com/MehmetCanWT/sonata.git}"
install_root="${HOME}/.local"
tmp_dir=""

cleanup() {
    if [ -n "${tmp_dir}" ] && [ -d "${tmp_dir}" ]; then
        rm -rf "${tmp_dir}"
    fi
}
trap cleanup EXIT INT TERM

run_as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        sudo "$@"
    fi
}

calc_build_jobs() {
    cpus=4
    if command -v nproc >/dev/null 2>&1; then
        cpus="$(nproc)"
    elif command -v getconf >/dev/null 2>&1; then
        cpus="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
    fi
    if [ "${cpus}" -gt 2 ]; then
        echo "$((cpus - 2))"
    else
        echo "1"
    fi
}

install_standalone_ytdlp() {
    mkdir -p "${install_root}/bin"
    curl -fsSL "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp" -o "${install_root}/bin/yt-dlp"
    chmod 0755 "${install_root}/bin/yt-dlp"
}

install_system_deps() {
    if command -v pacman >/dev/null 2>&1; then
        run_as_root pacman -S --needed --noconfirm \
            base-devel pkgconf ca-certificates curl ffmpeg git mpv nodejs yt-dlp \
            wayland vulkan-icd-loader libxkbcommon libxkbcommon-x11 libxcb fontconfig alsa-lib dbus
    elif command -v apt-get >/dev/null 2>&1; then
        run_as_root apt-get update
        run_as_root apt-get install -y \
            build-essential pkg-config ca-certificates curl ffmpeg git mpv nodejs python3 \
            libwayland-dev libvulkan-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev \
            libfontconfig1-dev libasound2-dev libdbus-1-dev libssl-dev
        if ! run_as_root apt-get install -y yt-dlp; then
            echo "apt-get yt-dlp unavailable; installing standalone yt-dlp binary..." >&2
            install_standalone_ytdlp
        fi
    elif command -v dnf >/dev/null 2>&1; then
        if ! run_as_root dnf install -y \
            gcc gcc-c++ make pkgconf-pkg-config ca-certificates curl ffmpeg-free git mpv nodejs yt-dlp \
            wayland-devel vulkan-loader-devel libxkbcommon-devel libxkbcommon-x11-devel libxcb-devel \
            fontconfig-devel alsa-lib-devel dbus-devel openssl-devel; then
            run_as_root dnf install -y \
                gcc gcc-c++ make pkgconf-pkg-config ca-certificates curl ffmpeg git mpv nodejs yt-dlp \
                wayland-devel vulkan-loader-devel libxkbcommon-devel libxkbcommon-x11-devel libxcb-devel \
                fontconfig-devel alsa-lib-devel dbus-devel openssl-devel
        fi
    elif command -v zypper >/dev/null 2>&1; then
        run_as_root zypper --non-interactive install -y \
            gcc gcc-c++ make pkg-config ca-certificates curl ffmpeg git mpv nodejs yt-dlp \
            wayland-devel vulkan-devel libxkbcommon-devel libxkbcommon-x11-devel libxcb-devel \
            fontconfig-devel alsa-devel dbus-1-devel libopenssl-devel
    else
        echo "Unsupported package manager. Install mpv, yt-dlp, ffmpeg, nodejs, Wayland/Vulkan/XKB/ALSA/D-Bus dev libraries, git, curl, and a C compiler first." >&2
        exit 1
    fi
}

verify_runtime_tools() {
    export PATH="${install_root}/bin:${HOME}/.cargo/bin:${PATH}"

    if ! command -v yt-dlp >/dev/null 2>&1; then
        echo "yt-dlp not found on PATH; installing standalone yt-dlp binary..." >&2
        install_standalone_ytdlp
    fi

    for tool in mpv ffmpeg yt-dlp; do
        if ! command -v "${tool}" >/dev/null 2>&1; then
            echo "Required runtime tool '${tool}' is missing after dependency installation." >&2
            exit 1
        fi
    done

    echo "Verified runtime tools:"
    echo "  mpv:    $(mpv --version | head -n 1)"
    echo "  ffmpeg: $(ffmpeg -version | head -n 1)"
    echo "  yt-dlp: $(yt-dlp --version | head -n 1)"
    if command -v node >/dev/null 2>&1; then
        echo "  node:   $(node --version | head -n 1)"
    else
        echo "  node:   not found (optional extractor fallback)"
    fi
}

ensure_rust() {
    if [ -f "${HOME}/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "${HOME}/.cargo/env"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    fi

    if [ -f "${HOME}/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "${HOME}/.cargo/env"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        echo "Cargo is unavailable after Rust installation." >&2
        exit 1
    fi
}

install_sonata_desktop() {
    export PATH="${install_root}/bin:${HOME}/.cargo/bin:${PATH}"
    jobs="$(calc_build_jobs)"
    echo "Building sonata-desktop with ${jobs} parallel jobs..."

    tmp_dir="$(mktemp -d)"
    git clone --depth=1 "${repo_url}" "${tmp_dir}/sonata"
    (
        cd "${tmp_dir}/sonata"
        cargo build --locked --release -j "${jobs}" -p sonata-desktop
    )

    install -Dm755 "${tmp_dir}/sonata/target/release/sonata-desktop" "${install_root}/bin/sonata-desktop"
    install -Dm644 "${tmp_dir}/sonata/assets/sonata-desktop.svg" "${install_root}/share/icons/hicolor/scalable/apps/sonata-desktop.svg"
    install -Dm644 "${tmp_dir}/sonata/assets/sonata-desktop.desktop" "${install_root}/share/applications/sonata-desktop.desktop"

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${install_root}/share/applications" >/dev/null 2>&1 || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f "${install_root}/share/icons/hicolor" >/dev/null 2>&1 || true
    fi
}

ensure_profile_path() {
    profile_file="${HOME}/.profile"
    path_line='export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
    if ! grep -Fqx "${path_line}" "${profile_file}" 2>/dev/null; then
        printf '\n%s\n' "${path_line}" >> "${profile_file}"
    fi
}

install_system_deps
verify_runtime_tools
ensure_rust
install_sonata_desktop
ensure_profile_path

echo
echo "Sonata Desktop installed: ${install_root}/bin/sonata-desktop"
echo "Desktop launcher installed: ${install_root}/share/applications/sonata-desktop.desktop"
echo "Run it from your app launcher or with: sonata-desktop"
echo "If 'sonata-desktop' is not found in your current shell, run: export PATH=\"\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH\""
