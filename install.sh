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

install_standalone_ytdlp() {
    mkdir -p "${install_root}/bin"
    curl -fsSL "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp" -o "${install_root}/bin/yt-dlp"
    chmod 0755 "${install_root}/bin/yt-dlp"
}

install_system_deps() {
    if command -v pacman >/dev/null 2>&1; then
        run_as_root pacman -S --needed --noconfirm base-devel ca-certificates curl ffmpeg git mpv nodejs yt-dlp
    elif command -v apt-get >/dev/null 2>&1; then
        run_as_root apt-get update
        run_as_root apt-get install -y build-essential ca-certificates curl ffmpeg git mpv nodejs python3
        if ! run_as_root apt-get install -y yt-dlp; then
            echo "apt-get yt-dlp unavailable; installing standalone yt-dlp binary..." >&2
            install_standalone_ytdlp
        fi
    elif command -v dnf >/dev/null 2>&1; then
        if ! run_as_root dnf install -y gcc gcc-c++ make ca-certificates curl ffmpeg-free git mpv nodejs yt-dlp; then
            run_as_root dnf install -y gcc gcc-c++ make ca-certificates curl ffmpeg git mpv nodejs yt-dlp
        fi
    elif command -v zypper >/dev/null 2>&1; then
        run_as_root zypper --non-interactive install -y gcc gcc-c++ make ca-certificates curl ffmpeg git mpv nodejs yt-dlp
    elif command -v brew >/dev/null 2>&1; then
        brew install ffmpeg mpv node yt-dlp git curl
    else
        echo "Unsupported package manager. Install mpv, yt-dlp, ffmpeg, nodejs, git, curl, and a C compiler first." >&2
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

install_sonata_tui() {
    export PATH="${install_root}/bin:${HOME}/.cargo/bin:${PATH}"

    if cargo install --git "${repo_url}" --locked --root "${install_root}" --force sonata-tui; then
        return 0
    fi

    echo "Direct cargo git install failed; falling back to temporary shallow clone..." >&2
    tmp_dir="$(mktemp -d)"
    git clone --depth=1 "${repo_url}" "${tmp_dir}/sonata"
    cargo install --locked --path "${tmp_dir}/sonata/crates/sonata-tui" --root "${install_root}" --force
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
install_sonata_tui
ensure_profile_path

echo
echo "Sonata TUI installed: ${install_root}/bin/sonata"
echo "Run it with: sonata"
echo "If 'sonata' is not found in your current shell, run: export PATH=\"\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH\""
