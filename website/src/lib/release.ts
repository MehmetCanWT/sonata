import { useEffect, useState } from "react";
import type { PlatformId } from "@/i18n";

export interface ReleaseInfo {
  version: string;
  tag: string;
  releaseUrl: string;
  checksumsUrl: string;
  assets: {
    androidApk: string;
    linuxAppImage: string;
    linuxDeb: string;
    linuxRpm: string;
    linuxTarGz: string;
    macosDmg: string;
    macosTarGz: string;
    windowsSetup: string;
    windowsMsi: string;
    windowsExe: string;
  };
}

const DEFAULT_VERSION = "1.0.10";
const REPO_BASE = "https://github.com/MehmetCanWT/sonata";

export function buildReleaseInfo(rawVersion: string): ReleaseInfo {
  const version = rawVersion.replace(/^v/, "").trim() || DEFAULT_VERSION;
  const tag = `v${version}`;
  const dl = `${REPO_BASE}/releases/download/${tag}`;

  return {
    version,
    tag,
    releaseUrl: `${REPO_BASE}/releases/tag/${tag}`,
    checksumsUrl: `${dl}/SHA256SUMS.txt`,
    assets: {
      androidApk: `${dl}/Sonata-${tag}-arm64.apk`,
      linuxAppImage: `${dl}/Sonata-${tag}-x86_64.AppImage`,
      linuxDeb: `${dl}/sonata-${tag}-amd64.deb`,
      linuxRpm: `${dl}/sonata-${tag}-1.x86_64.rpm`,
      linuxTarGz: `${dl}/sonata-${tag}-linux-x86_64.tar.gz`,
      macosDmg: `${dl}/Sonata-${tag}-macos-arm64.dmg`,
      macosTarGz: `${dl}/Sonata-${tag}-macos-arm64.app.tar.gz`,
      windowsSetup: `${dl}/Sonata-${tag}-windows-x64-setup.exe`,
      windowsMsi: `${dl}/Sonata-${tag}-windows-x64.msi`,
      windowsExe: `${dl}/Sonata-${tag}-windows-x64.exe`,
    },
  };
}

export function detectPlatform(): PlatformId {
  if (typeof navigator === "undefined") {
    return "linux";
  }
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("android")) return "android";
  if (ua.includes("mac os") || ua.includes("macintosh")) return "macos";
  if (ua.includes("windows")) return "windows";
  return "linux";
}

export function useLatestRelease(): ReleaseInfo {
  const [release, setRelease] = useState<ReleaseInfo>(() =>
    buildReleaseInfo(DEFAULT_VERSION),
  );

  useEffect(() => {
    const controller = new AbortController();
    fetch("https://api.github.com/repos/MehmetCanWT/sonata/releases/latest", {
      signal: controller.signal,
      headers: { Accept: "application/vnd.github+json" },
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((data: { tag_name?: string } | null) => {
        if (data?.tag_name) {
          setRelease(buildReleaseInfo(data.tag_name));
        }
      })
      .catch(() => {
        // Keep verified fallback release URLs when offline or rate-limited.
      });

    return () => controller.abort();
  }, []);

  return release;
}
