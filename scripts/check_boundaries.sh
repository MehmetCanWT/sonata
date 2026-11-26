#!/usr/bin/env bash
set -euo pipefail

# Enforce Sonata crate boundaries, dependency direction, and platform hygiene.

fail() {
  echo "BOUNDARY CHECK FAILED: $1" >&2
  exit 1
}

echo "[1/7] Checking sonata-core for forbidden subprocess launches..."
if rg -n '(std|tokio)::process::Command' crates/sonata-core/src; then
  fail "sonata-core must never spawn subprocesses directly"
fi

echo "[2/7] Checking sonata-core/src/providers for target_os policy branches..."
if rg -n 'cfg!\(target_os = "android"\)' crates/sonata-core/src/providers; then
  fail "sonata-core providers must use ResolverProfile instead of target_os branches"
fi

echo "[3/7] Checking sonata-core and sonata-android for forbidden sonata-runtime references..."
if rg -n 'sonata-runtime' crates/sonata-core crates/sonata-android; then
  fail "sonata-core and sonata-android must never depend on or reference sonata-runtime"
fi

echo "[4/7] Checking sonata-android dependency tree for desktop/Linux crates..."
if cargo tree -p sonata-android -e normal | rg -n 'image v|dirs v|toml v0\.8|tracing-appender v'; then
  fail "sonata-android pulled a forbidden Linux/desktop dependency"
fi

echo "[5/7] Checking sonata-android for synchronous Runtime::block_on calls..."
if rg -n 'block_on' crates/sonata-android/src; then
  fail "sonata-android must expose cancellable async UniFFI methods without Runtime::block_on"
fi

echo "[6/7] Checking Cargo metadata dependency direction..."
python3 - <<'PY'
import json, subprocess, sys

meta = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], text=True
    )
)
by_name = {pkg["name"]: {dep["name"] for dep in pkg["dependencies"] if dep.get("kind") is None} for pkg in meta["packages"]}

forbidden_core = {"sonata-runtime", "sonata-tui", "sonata-desktop", "sonata-android", "ratatui", "crossterm", "tauri", "uniffi"}
core_bad = by_name["sonata-core"] & forbidden_core
if core_bad:
    sys.exit(f"sonata-core has forbidden normal dependencies: {sorted(core_bad)}")

if "sonata-core" not in by_name["sonata-android"] or "sonata-runtime" in by_name["sonata-android"]:
    sys.exit(f"sonata-android normal dependencies invalid: {sorted(by_name['sonata-android'])}")

for linux_client in ("sonata-tui", "sonata-desktop"):
    deps = by_name[linux_client]
    if "sonata-core" not in deps or "sonata-runtime" not in deps:
        sys.exit(f"{linux_client} must depend on both sonata-core and sonata-runtime: {sorted(deps)}")

if "sonata-core" not in by_name["sonata-runtime"]:
    sys.exit("sonata-runtime must depend on sonata-core")

print("Cargo metadata dependency direction verified:", {k: sorted(v & {"sonata-core", "sonata-runtime"}) for k, v in by_name.items()})
PY

echo "[7/7] Checking deleted duplicate protocols stay removed..."
if [[ -e crates/sonata-desktop/src/backend/ytm_extra.rs ]]; then
  fail "crates/sonata-desktop/src/backend/ytm_extra.rs must stay consolidated in sonata-core"
fi
if [[ -e android/app/src/main/java/dev/sonata/android/data/JioSaavnSource.kt ]]; then
  fail "JioSaavnSource.kt must stay consolidated in sonata-core + UniFFI"
fi
if rg -n 'APPLE_AMP_BASE|TIDAL_SEARCH_URL|COMMUNITY_MANIFEST_URL|obtainAppleJwt' android/app/src; then
  fail "Duplicated remote motion discovery found in Android Kotlin sources"
fi

echo "ALL BOUNDARY CHECKS PASSED"
