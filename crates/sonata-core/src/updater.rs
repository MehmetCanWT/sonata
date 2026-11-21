//! GitHub Releases update checker for Sonata desktop, TUI, and Android clients.
//!
//! 1. **Per-Platform Filtering**: `UpdateTarget` scopes checks to matching release streams
//!    (`v*` unified releases, `android-v*` Android-only, `desktop-v*` Desktop/TUI-only, `tui-v*`).
//! 2. **Semver Comparison**: Parses plain, `v`-prefixed, and platform-prefixed `major.minor.patch` tags cleanly.
//! 3. **GitHub API Client**: `check_for_updates_for` queries recent GitHub Releases with a 6s timeout and `404` fallback.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_RELEASES_LIST_API: &str =
    "https://api.github.com/repos/MehmetCanWT/sonata/releases?per_page=20";
pub const GITHUB_RELEASES_PAGE: &str = "https://github.com/MehmetCanWT/sonata/releases";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    All,
    Android,
    Desktop,
    Tui,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub apk_download_url: Option<String>,
    pub appimage_download_url: Option<String>,
    pub release_notes: String,
}

/// Checks GitHub Releases for a newer unified release of Sonata.
pub async fn check_for_updates(http: &reqwest::Client) -> Result<UpdateCheckResult> {
    check_for_updates_for(http, UpdateTarget::All, CURRENT_VERSION).await
}

/// Checks GitHub Releases for the newest release applicable to `target`.
pub async fn check_for_updates_for(
    http: &reqwest::Client,
    target: UpdateTarget,
    current_version: &str,
) -> Result<UpdateCheckResult> {
    let effective_current = if current_version.trim().is_empty() {
        CURRENT_VERSION
    } else {
        current_version.trim()
    };
    let user_agent = format!("sonata/{effective_current}");
    let response = http
        .get(GITHUB_RELEASES_LIST_API)
        .header("User-Agent", user_agent)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(6))
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(up_to_date_result(effective_current));
    }

    let response = response.error_for_status()?;
    let payload: Value = response.json().await?;
    Ok(parse_releases_for_target(
        &payload,
        target,
        effective_current,
    ))
}

fn up_to_date_result(current_version: &str) -> UpdateCheckResult {
    UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version: current_version.to_string(),
        update_available: false,
        release_url: GITHUB_RELEASES_PAGE.to_string(),
        apk_download_url: None,
        appimage_download_url: None,
        release_notes: "Up to date (no newer release published).".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagScope {
    Unified,
    AndroidOnly,
    DesktopAndTui,
    TuiOnly,
}

fn classify_tag(raw_tag: &str) -> Option<(TagScope, &str)> {
    let trimmed = raw_tag.trim();
    if let Some(rest) = trimmed
        .strip_prefix("android-v")
        .or_else(|| trimmed.strip_prefix("android-V"))
    {
        return Some((TagScope::AndroidOnly, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("desktop-v")
        .or_else(|| trimmed.strip_prefix("desktop-V"))
    {
        return Some((TagScope::DesktopAndTui, rest.trim()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("tui-v")
        .or_else(|| trimmed.strip_prefix("tui-V"))
    {
        return Some((TagScope::TuiOnly, rest.trim()));
    }
    let rest = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed)
        .trim();
    (!rest.is_empty()).then_some((TagScope::Unified, rest))
}

fn scope_matches_target(
    scope: TagScope,
    target: UpdateTarget,
    has_apk: bool,
    has_appimage: bool,
) -> bool {
    match (target, scope) {
        (UpdateTarget::All, _) => true,
        (UpdateTarget::Android, TagScope::AndroidOnly) => true,
        (UpdateTarget::Android, TagScope::Unified) => has_apk,
        (UpdateTarget::Android, TagScope::DesktopAndTui | TagScope::TuiOnly) => false,
        (UpdateTarget::Desktop, TagScope::DesktopAndTui) => true,
        (UpdateTarget::Desktop, TagScope::Unified) => has_appimage,
        (UpdateTarget::Desktop, TagScope::AndroidOnly | TagScope::TuiOnly) => false,
        (UpdateTarget::Tui, TagScope::TuiOnly | TagScope::DesktopAndTui | TagScope::Unified) => {
            true
        }
        (UpdateTarget::Tui, TagScope::AndroidOnly) => false,
    }
}

fn parse_releases_for_target(
    payload: &Value,
    target: UpdateTarget,
    current_version: &str,
) -> UpdateCheckResult {
    let releases: Vec<&Value> = match payload.as_array() {
        Some(arr) => arr.iter().collect(),
        None if payload.is_object() => vec![payload],
        None => return up_to_date_result(current_version),
    };

    let mut best: Option<(ParsedSemver, UpdateCheckResult)> = None;
    for release in releases {
        if release
            .get("draft")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || release
                .get("prerelease")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            continue;
        }
        let Some(raw_tag) = release.get("tag_name").and_then(Value::as_str) else {
            continue;
        };
        let Some((scope, ver_str)) = classify_tag(raw_tag) else {
            continue;
        };
        let Some(parsed_semver) = parse_semver(ver_str) else {
            continue;
        };
        let candidate = parse_release_json(release, ver_str, current_version);
        if !scope_matches_target(
            scope,
            target,
            candidate.apk_download_url.is_some(),
            candidate.appimage_download_url.is_some(),
        ) {
            continue;
        }
        let replace = match &best {
            Some((best_ver, _)) => parsed_semver > *best_ver,
            None => true,
        };
        if replace {
            best = Some((parsed_semver, candidate));
        }
    }

    best.map(|(_, result)| result)
        .unwrap_or_else(|| up_to_date_result(current_version))
}

fn parse_release_json(
    payload: &Value,
    normalized_version: &str,
    current_version: &str,
) -> UpdateCheckResult {
    let latest_version = if normalized_version.is_empty() {
        current_version.to_string()
    } else {
        normalized_version.to_string()
    };

    let release_url = payload
        .get("html_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(GITHUB_RELEASES_PAGE)
        .to_string();

    let release_notes = payload
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();

    let mut apk_download_url = None;
    let mut appimage_download_url = None;
    if let Some(assets) = payload.get("assets").and_then(Value::as_array) {
        for asset in assets {
            let Some(url) = asset
                .get("browser_download_url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|url| !url.is_empty())
            else {
                continue;
            };
            let lower = url.to_ascii_lowercase();
            if apk_download_url.is_none() && lower.ends_with(".apk") {
                apk_download_url = Some(url.to_string());
            } else if appimage_download_url.is_none() && lower.ends_with(".appimage") {
                appimage_download_url = Some(url.to_string());
            }
        }
    }

    let update_available = is_newer_version(&latest_version, current_version);
    UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version,
        update_available,
        release_url,
        apk_download_url,
        appimage_download_url,
        release_notes,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ParsedSemver {
    major: u64,
    minor: u64,
    patch: u64,
    is_stable: bool,
}

fn parse_semver(version: &str) -> Option<ParsedSemver> {
    let trimmed = version.trim();
    let without_platform = trimmed
        .strip_prefix("android-")
        .or_else(|| trimmed.strip_prefix("desktop-"))
        .or_else(|| trimmed.strip_prefix("tui-"))
        .unwrap_or(trimmed);
    let without_v = without_platform
        .strip_prefix('v')
        .or_else(|| without_platform.strip_prefix('V'))
        .unwrap_or(without_platform);
    let without_build = without_v.split('+').next()?;
    let (core, is_stable) = match without_build.split_once('-') {
        Some((core, _pre)) => (core, false),
        None => (without_build, true),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ParsedSemver {
        major,
        minor,
        patch,
        is_stable,
    })
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(latest_ver), Some(current_ver)) => latest_ver > current_ver,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_semver_versions_and_prereleases() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("android-v1.0.1", "1.0.0"));
        assert!(is_newer_version("desktop-v1.1.0", "1.0.0"));
        assert!(is_newer_version("v1.0.0-rc1", "0.1.0"));
        assert!(!is_newer_version("v1.0.0-rc1", "1.0.0"));
        assert!(is_newer_version("1.0.0", "v1.0.0-rc1"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("v1.0.0", "1.0.0"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
        assert!(!is_newer_version("invalid", "1.0.0"));
    }

    #[test]
    fn filters_android_and_desktop_releases_independently() {
        let payload = json!([
            {
                "tag_name": "android-v1.0.1",
                "html_url": "https://github.com/MehmetCanWT/sonata/releases/tag/android-v1.0.1",
                "body": "Android performance release",
                "assets": [
                    {
                        "name": "Sonata-android-v1.0.1-arm64.apk",
                        "browser_download_url": "https://example.com/Sonata-android-v1.0.1-arm64.apk"
                    }
                ]
            },
            {
                "tag_name": "v1.0.0",
                "html_url": "https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.0",
                "body": "Unified 1.0.0 release",
                "assets": [
                    {
                        "name": "Sonata-v1.0.0-arm64.apk",
                        "browser_download_url": "https://example.com/Sonata-v1.0.0-arm64.apk"
                    },
                    {
                        "name": "Sonata-v1.0.0-x86_64.AppImage",
                        "browser_download_url": "https://example.com/Sonata-v1.0.0-x86_64.AppImage"
                    }
                ]
            }
        ]);

        // Android sees android-v1.0.1 as an update!
        let android = parse_releases_for_target(&payload, UpdateTarget::Android, "1.0.0");
        assert!(android.update_available);
        assert_eq!(android.latest_version, "1.0.1");
        assert_eq!(
            android.apk_download_url.as_deref(),
            Some("https://example.com/Sonata-android-v1.0.1-arm64.apk")
        );

        // Desktop & TUI MUST NOT be notified for android-v1.0.1!
        let desktop = parse_releases_for_target(&payload, UpdateTarget::Desktop, "1.0.0");
        assert!(!desktop.update_available);
        assert_eq!(desktop.latest_version, "1.0.0");
        assert_eq!(
            desktop.appimage_download_url.as_deref(),
            Some("https://example.com/Sonata-v1.0.0-x86_64.AppImage")
        );

        let tui = parse_releases_for_target(&payload, UpdateTarget::Tui, "1.0.0");
        assert!(!tui.update_available);
        assert_eq!(tui.latest_version, "1.0.0");
    }

    #[test]
    fn desktop_only_release_notifies_desktop_and_tui_but_not_android() {
        let payload = json!([
            {
                "tag_name": "desktop-v1.0.2",
                "html_url": "https://github.com/MehmetCanWT/sonata/releases/tag/desktop-v1.0.2",
                "body": "Desktop shader polish",
                "assets": [
                    {
                        "name": "Sonata-desktop-v1.0.2-x86_64.AppImage",
                        "browser_download_url": "https://example.com/Sonata-desktop-v1.0.2-x86_64.AppImage"
                    }
                ]
            },
            {
                "tag_name": "v1.0.0",
                "html_url": "https://github.com/MehmetCanWT/sonata/releases/tag/v1.0.0",
                "body": "Unified 1.0.0",
                "assets": [
                    {
                        "name": "Sonata-v1.0.0-arm64.apk",
                        "browser_download_url": "https://example.com/Sonata-v1.0.0-arm64.apk"
                    },
                    {
                        "name": "Sonata-v1.0.0-x86_64.AppImage",
                        "browser_download_url": "https://example.com/Sonata-v1.0.0-x86_64.AppImage"
                    }
                ]
            }
        ]);

        assert!(
            parse_releases_for_target(&payload, UpdateTarget::Desktop, "1.0.0").update_available
        );
        assert!(parse_releases_for_target(&payload, UpdateTarget::Tui, "1.0.0").update_available);
        assert!(
            !parse_releases_for_target(&payload, UpdateTarget::Android, "1.0.0").update_available
        );
    }
}
