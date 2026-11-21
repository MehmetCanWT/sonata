//! GitHub Releases update checker for Sonata desktop and Android clients.
//!
//! 1. **Release Metadata**: `UpdateCheckResult` holds current/latest versions, installer URLs, and notes.
//! 2. **Semver Comparison**: Parses `v`-prefixed or plain `major.minor.patch` tags cleanly for update checks.
//! 3. **GitHub API Client**: `check_for_updates` queries the latest GitHub Release with a 6s timeout and `404` fallback.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/MehmetCanWT/sonata/releases/latest";
pub const GITHUB_RELEASES_PAGE: &str = "https://github.com/MehmetCanWT/sonata/releases";

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

/// Checks GitHub Releases for a newer version of Sonata.
pub async fn check_for_updates(http: &reqwest::Client) -> Result<UpdateCheckResult> {
    let user_agent = format!("sonata/{CURRENT_VERSION}");
    let response = http
        .get(GITHUB_RELEASES_API)
        .header("User-Agent", user_agent)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(6))
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateCheckResult {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: CURRENT_VERSION.to_string(),
            update_available: false,
            release_url: GITHUB_RELEASES_PAGE.to_string(),
            apk_download_url: None,
            appimage_download_url: None,
            release_notes: "Up to date (no newer release published).".to_string(),
        });
    }

    let response = response.error_for_status()?;
    let payload: Value = response.json().await?;
    Ok(parse_release_json(&payload, CURRENT_VERSION))
}

fn parse_release_json(payload: &Value, current_version: &str) -> UpdateCheckResult {
    let raw_tag = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let stripped_tag = raw_tag
        .strip_prefix('v')
        .or_else(|| raw_tag.strip_prefix('V'))
        .unwrap_or(raw_tag)
        .trim();
    let latest_version = if stripped_tag.is_empty() {
        current_version.to_string()
    } else {
        stripped_tag.to_string()
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
    let without_v = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
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
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("v1.0.0-rc1", "0.1.0"));
        assert!(!is_newer_version("v1.0.0-rc1", "1.0.0"));
        assert!(is_newer_version("1.0.0", "v1.0.0-rc1"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
        assert!(!is_newer_version("invalid", "0.1.0"));
    }

    #[test]
    fn parses_github_release_json_and_platform_assets() {
        let payload = json!({
            "tag_name": "v0.3.2",
            "html_url": "https://github.com/MehmetCanWT/sonata/releases/tag/v0.3.2",
            "body": "### Highlights\n- Faster playback\n",
            "assets": [
                {
                    "name": "sonata-debug.apk.sha256",
                    "browser_download_url": "https://example.com/sonata.apk.sha256"
                },
                {
                    "name": "sonata-universal.apk",
                    "browser_download_url": "https://example.com/sonata-universal.apk"
                },
                {
                    "name": "Sonata-x86_64.AppImage",
                    "browser_download_url": "https://example.com/Sonata-x86_64.AppImage"
                }
            ]
        });

        let parsed = parse_release_json(&payload, "0.1.0");
        assert_eq!(parsed.current_version, "0.1.0");
        assert_eq!(parsed.latest_version, "0.3.2");
        assert!(parsed.update_available);
        assert_eq!(
            parsed.release_url,
            "https://github.com/MehmetCanWT/sonata/releases/tag/v0.3.2"
        );
        assert_eq!(
            parsed.apk_download_url.as_deref(),
            Some("https://example.com/sonata-universal.apk")
        );
        assert_eq!(
            parsed.appimage_download_url.as_deref(),
            Some("https://example.com/Sonata-x86_64.AppImage")
        );
        assert_eq!(parsed.release_notes, "### Highlights\n- Faster playback");

        let same = parse_release_json(&payload, "0.3.2");
        assert!(!same.update_available);
    }
}
