#!/bin/sh
set -eu

repo_url="${SONATA_REPO_URL:-https://github.com/MehmetCanWT/sonata.git}"
install_root="${HOME}/.local"
tmp_dir=""
ytdlp_tmp_dir=""

validate_repo_url() {
    case "${repo_url}" in
        https://*)
            ;;
        *)
            echo "Invalid SONATA_REPO_URL '${repo_url}': must start with https://" >&2
            exit 1
            ;;
    esac
}

cleanup() {
    if [ -n "${tmp_dir}" ] && [ -d "${tmp_dir}" ]; then
        rm -rf "${tmp_dir}"
    fi
    if [ -n "${ytdlp_tmp_dir}" ] && [ -d "${ytdlp_tmp_dir}" ]; then
        rm -rf "${ytdlp_tmp_dir}"
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

verify_sha256() {
    expected_hash="$1"
    target_file="$2"
    if command -v sha256sum >/dev/null 2>&1; then
        printf '%s  %s\n' "${expected_hash}" "${target_file}" | sha256sum -c - >/dev/null
    elif command -v shasum >/dev/null 2>&1; then
        printf '%s  %s\n' "${expected_hash}" "${target_file}" | shasum -a 256 -c - >/dev/null
    else
        echo "Neither sha256sum nor shasum is available to verify yt-dlp." >&2
        exit 1
    fi
}

install_standalone_ytdlp() {
    mkdir -p "${install_root}/bin"
    ytdlp_tmp_dir="$(mktemp -d)"
    curl --proto '=https' --tlsv1.2 -fsSL \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp" \
        -o "${ytdlp_tmp_dir}/yt-dlp"
    curl --proto '=https' --tlsv1.2 -fsSL \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS" \
        -o "${ytdlp_tmp_dir}/SHA2-256SUMS"
    expected_sha="$(awk '$2 == "yt-dlp" { print $1; exit }' "${ytdlp_tmp_dir}/SHA2-256SUMS")"
    if ! printf '%s\n' "${expected_sha}" | grep -Eq '^[0-9a-fA-F]{64}$'; then
        echo "Failed to parse valid SHA-256 checksum for yt-dlp from SHA2-256SUMS." >&2
        exit 1
    fi
    verify_sha256 "${expected_sha}" "${ytdlp_tmp_dir}/yt-dlp"
    install -m 0755 "${ytdlp_tmp_dir}/yt-dlp" "${install_root}/bin/yt-dlp"
    rm -rf "${ytdlp_tmp_dir}"
    ytdlp_tmp_dir=""
}

install_system_deps() {
    if [ -e /etc/NIXOS ] || command -v nix >/dev/null 2>&1; then
        if command -v nix >/dev/null 2>&1; then
            echo "Nix/NixOS detected. Installing Sonata TUI via Nix Flakes..."
            if nix --extra-experimental-features "nix-command flakes" profile install "github:MehmetCanWT/sonata#sonata"; then
                echo "Installed sonata via Nix Flakes."
                exit 0
            fi
            echo "Nix profile install failed; falling back to system package manager..." >&2
        fi
    fi

    if command -v pacman >/dev/null 2>&1; then
        run_as_root pacman -S --needed --noconfirm base-devel pkgconf openssl sqlite ca-certificates curl ffmpeg git mpv nodejs yt-dlp
    elif command -v apt-get >/dev/null 2>&1; then
        run_as_root apt-get update
        run_as_root apt-get install -y build-essential pkg-config libssl-dev libsqlite3-dev ca-certificates curl ffmpeg git mpv nodejs python3
        if ! run_as_root apt-get install -y yt-dlp; then
            echo "apt-get yt-dlp unavailable; installing standalone yt-dlp binary..." >&2
            install_standalone_ytdlp
        fi
    elif command -v dnf >/dev/null 2>&1; then
        if ! run_as_root dnf install -y gcc gcc-c++ make pkgconf-pkg-config openssl-devel sqlite-devel ca-certificates curl ffmpeg-free git mpv nodejs yt-dlp; then
            run_as_root dnf install -y gcc gcc-c++ make pkgconf-pkg-config openssl-devel sqlite-devel ca-certificates curl ffmpeg git mpv nodejs yt-dlp
        fi
    elif command -v zypper >/dev/null 2>&1; then
        run_as_root zypper --non-interactive install -y gcc gcc-c++ make pkg-config libopenssl-devel sqlite3-devel ca-certificates curl ffmpeg git mpv nodejs yt-dlp
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
    cpus=4
    if command -v nproc >/dev/null 2>&1; then
        cpus="$(nproc)"
    elif command -v getconf >/dev/null 2>&1; then
        cpus="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
    fi
    if [ "${cpus}" -gt 2 ]; then
        jobs="$((cpus - 2))"
    else
        jobs=1
    fi

    if cargo install --git "${repo_url}" --locked -j "${jobs}" --root "${install_root}" --force sonata-tui; then
        return 0
    fi

    echo "Direct cargo git install failed; falling back to temporary shallow clone..." >&2
    tmp_dir="$(mktemp -d)"
    git clone --depth=1 "${repo_url}" "${tmp_dir}/sonata"
    cargo install --locked -j "${jobs}" --path "${tmp_dir}/sonata/crates/sonata-tui" --root "${install_root}" --force
}

ensure_profile_path() {
    profile_file="${HOME}/.profile"
    path_line='export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
    if ! grep -Fqx "${path_line}" "${profile_file}" 2>/dev/null; then
        printf '\n%s\n' "${path_line}" >> "${profile_file}"
    fi
}

validate_repo_url
install_system_deps
verify_runtime_tools
ensure_rust
install_sonata_tui
ensure_profile_path

echo
echo "Sonata TUI installed: ${install_root}/bin/sonata"
echo "Run it with: sonata"
echo "If 'sonata' is not found in your current shell, run: export PATH=\"\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH\""
