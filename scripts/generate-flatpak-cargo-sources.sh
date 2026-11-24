#!/usr/bin/env bash
# Generates packaging/flatpak/cargo-sources.json for offline Flathub builds.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_FILE="${1:-${REPO_ROOT}/packaging/flatpak/cargo-sources.json}"
GENERATOR="${FLATPAK_CARGO_GENERATOR:-flatpak-cargo-generator.py}"

if ! command -v "${GENERATOR}" >/dev/null 2>&1; then
  echo "Error: '${GENERATOR}' not found in PATH." >&2
  echo "Download it from https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo" >&2
  exit 1
fi

python3 "$(command -v "${GENERATOR}")" "${REPO_ROOT}/Cargo.lock" -o "${OUT_FILE}"
echo "Wrote offline Cargo sources manifest to ${OUT_FILE}"
