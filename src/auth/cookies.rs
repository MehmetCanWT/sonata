use crate::config::AppConfig;
use rusqlite::Connection;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct ExtractedCookies {
    pub browser_name: Option<String>,
    pub ytm_cookies: HashMap<String, String>,
    pub spotify_sp_dc: Option<String>,
    pub netscape_file: Option<PathBuf>,
}

impl ExtractedCookies {
    pub fn ytm_cookie_header(&self) -> Option<String> {
        if self.ytm_cookies.is_empty() {
            return None;
        }
        let header = self
            .ytm_cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        Some(header)
    }

    pub fn ytm_sapisid(&self) -> Option<&str> {
        self.ytm_cookies
            .get("SAPISID")
            .or_else(|| self.ytm_cookies.get("__Secure-3PAPISID"))
            .map(String::as_str)
    }

    pub fn ytm_sapisidhash(&self) -> Option<String> {
        let sapisid = self.ytm_sapisid()?;
        Some(compute_sapisidhash(sapisid, "https://music.youtube.com"))
    }

    pub fn has_ytm_auth(&self) -> bool {
        self.ytm_sapisid().is_some()
    }

    pub fn has_spotify_auth(&self) -> bool {
        self.spotify_sp_dc.is_some()
    }
}

pub fn compute_sapisidhash(sapisid: &str, origin: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut hasher = Sha1::new();
    hasher.update(format!("{ts} {sapisid} {origin}").as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("SAPISIDHASH {ts}_{hex}")
}

#[derive(Debug, Clone)]
struct CookieRow {
    host: String,
    is_subdomain: bool,
    path: String,
    is_secure: bool,
    expiry: i64,
    name: String,
    value: String,
}

/// Discovers and extracts YouTube Music & Spotify cookies automatically from installed browsers.
pub fn extract_browser_cookies(preferred_browser: Option<&str>) -> ExtractedCookies {
    let mut result = ExtractedCookies::default();

    let gecko_candidates = [
        ("Zen Browser", ".config/zen"),
        ("Firefox", ".mozilla/firefox"),
        ("LibreWolf", ".librewolf"),
        ("Floorp", ".floorp"),
    ];

    let chromium_candidates = [
        ("chrome", ".config/google-chrome"),
        ("chromium", ".config/chromium"),
        ("brave", ".config/BraveSoftware/Brave-Browser"),
        ("vivaldi", ".config/vivaldi"),
        ("edge", ".config/microsoft-edge"),
    ];

    if let Some(pref) = preferred_browser {
        let low = pref.to_ascii_lowercase();
        for (name, rel) in gecko_candidates {
            if name.to_ascii_lowercase().contains(&low) {
                if let Some(found) = try_gecko_dir(name, rel) {
                    return found;
                }
            }
        }
        for (name, _) in chromium_candidates {
            if name == low {
                if let Some(found) = try_chromium_via_ytdlp(name) {
                    return found;
                }
            }
        }
    }

    // 1. Try Gecko-based browsers first (pure Rust SQLite read, zero keyring prompts).
    for (name, rel) in gecko_candidates {
        if let Some(found) = try_gecko_dir(name, rel) {
            if found.has_ytm_auth() || found.has_spotify_auth() {
                merge_cookies(&mut result, found);
                if result.has_ytm_auth() && result.has_spotify_auth() {
                    return result;
                }
            }
        }
    }

    // 2. Try Chromium-based browsers via yt-dlp cookie extraction if anything is still missing.
    for (browser, rel) in chromium_candidates {
        if let Some(home) = dirs::home_dir() {
            let exists = home.join(rel).exists();
            if exists {
                if let Some(found) = try_chromium_via_ytdlp(browser) {
                    if found.has_ytm_auth() || found.has_spotify_auth() {
                        merge_cookies(&mut result, found);
                        if result.has_ytm_auth() && result.has_spotify_auth() {
                            return result;
                        }
                    }
                }
            }
        }
    }

    result
}

fn merge_cookies(dst: &mut ExtractedCookies, src: ExtractedCookies) {
    if !dst.has_ytm_auth() && src.has_ytm_auth() {
        dst.ytm_cookies = src.ytm_cookies;
        if src.netscape_file.is_some() {
            dst.netscape_file = src.netscape_file;
        }
    }
    if dst.spotify_sp_dc.is_none() && src.spotify_sp_dc.is_some() {
        dst.spotify_sp_dc = src.spotify_sp_dc;
    }
    if let Some(b) = src.browser_name {
        match &dst.browser_name {
            None => dst.browser_name = Some(b),
            Some(prev) if !prev.contains(&b) => {
                dst.browser_name = Some(format!("{prev} + {b}"));
            }
            _ => {}
        }
    }
}

fn try_gecko_dir(browser_name: &str, rel_dir: &str) -> Option<ExtractedCookies> {
    let home = dirs::home_dir()?;
    let base = home.join(rel_dir);
    if !base.exists() {
        return None;
    }

    let mut dbs: Vec<(SystemTime, PathBuf)> = fs::read_dir(&base)
        .ok()?
        .flatten()
        .map(|e| e.path().join("cookies.sqlite"))
        .filter(|p| p.exists())
        .filter_map(|p| {
            let mtime = fs::metadata(&p).and_then(|m| m.modified()).ok()?;
            Some((mtime, p))
        })
        .collect();

    // Check most recently modified profiles first.
    dbs.sort_by_key(|a| std::cmp::Reverse(a.0));

    let mut best = ExtractedCookies::default();
    for (_, db_path) in dbs {
        if let Some(parsed) = read_gecko_sqlite(browser_name, &db_path) {
            if parsed.has_ytm_auth() || parsed.has_spotify_auth() {
                merge_cookies(&mut best, parsed);
                if best.has_ytm_auth() && best.has_spotify_auth() {
                    break;
                }
            }
        }
    }

    if best.has_ytm_auth() || best.has_spotify_auth() {
        Some(best)
    } else {
        None
    }
}

fn read_gecko_sqlite(browser_name: &str, db_path: &Path) -> Option<ExtractedCookies> {
    let tmp_dir = AppConfig::cache_dir().join("tmp_cookies");
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_db = tmp_dir.join("cookies.sqlite");
    fs::copy(db_path, &tmp_db).ok()?;

    // Copy WAL and SHM files if present so live browser sessions are captured cleanly.
    for ext in ["wal", "shm"] {
        let src_sidecar = PathBuf::from(format!("{}-{ext}", db_path.display()));
        let dst_sidecar = PathBuf::from(format!("{}-{ext}", tmp_db.display()));
        if src_sidecar.exists() {
            let _ = fs::copy(src_sidecar, dst_sidecar);
        } else {
            let _ = fs::remove_file(dst_sidecar);
        }
    }

    let conn = Connection::open(&tmp_db).ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT host, path, isSecure, expiry, name, value FROM moz_cookies \
             WHERE host LIKE '%youtube.com%' OR host LIKE '%google.com%' OR host LIKE '%spotify.com%'",
        )
        .ok()?;

    let rows = stmt
        .query_map([], |row| {
            let host: String = row.get(0)?;
            Ok(CookieRow {
                is_subdomain: host.starts_with('.'),
                host,
                path: row.get(1)?,
                is_secure: row.get::<_, i64>(2)? != 0,
                expiry: row.get(3)?,
                name: row.get(4)?,
                value: row.get(5)?,
            })
        })
        .ok()?;

    let mut collected_rows = Vec::new();
    for r in rows.flatten() {
        collected_rows.push(r);
    }

    let out = build_from_rows(browser_name, &collected_rows, true);
    let _ = fs::remove_dir_all(&tmp_dir);
    Some(out)
}

fn try_chromium_via_ytdlp(browser: &str) -> Option<ExtractedCookies> {
    let cache_file = AppConfig::cache_dir().join(format!("cookies_{browser}.txt"));
    let status = Command::new("yt-dlp")
        .args([
            "--cookies-from-browser",
            browser,
            "--cookies",
            &cache_file.to_string_lossy(),
            "--skip-download",
            "--no-warnings",
            "--quiet",
            "https://music.youtube.com/",
        ])
        .status()
        .ok()?;

    if !status.success() && !cache_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&cache_file).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&cache_file, fs::Permissions::from_mode(0o600));
    }
    let rows = parse_netscape_cookies(&content);
    let mut out = build_from_rows(browser, &rows, true);
    if out.has_ytm_auth() {
        out.netscape_file = Some(cache_file);
    }
    Some(out)
}

fn domain_matches(host: &str, domain: &str) -> bool {
    let trimmed = host.trim_start_matches('.');
    trimmed == domain || trimmed.ends_with(&format!(".{domain}"))
}

fn build_from_rows(
    browser_name: &str,
    rows: &[CookieRow],
    write_netscape_cache: bool,
) -> ExtractedCookies {
    let mut ytm_cookies = HashMap::new();
    let mut spotify_sp_dc = None;
    let mut yt_netscape_lines = vec!["# Netscape HTTP Cookie File".to_string()];

    for row in rows {
        let host = row.host.to_ascii_lowercase();
        let is_yt = domain_matches(&host, "youtube.com");
        let is_google = domain_matches(&host, "google.com");

        if is_yt {
            ytm_cookies.insert(row.name.clone(), row.value.clone());
        }
        if is_yt || is_google {
            let flag = if row.is_subdomain { "TRUE" } else { "FALSE" };
            let secure = if row.is_secure { "TRUE" } else { "FALSE" };
            yt_netscape_lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.host, flag, row.path, secure, row.expiry, row.name, row.value
            ));
        }
        if domain_matches(&host, "spotify.com") && row.name == "sp_dc" && !row.value.is_empty() {
            spotify_sp_dc = Some(row.value.clone());
        }
    }

    let netscape_file = if write_netscape_cache && yt_netscape_lines.len() > 1 {
        let _ = fs::create_dir_all(AppConfig::cache_dir());
        let path = AppConfig::cache_dir().join("yt_cookies.txt");
        if fs::write(&path, format!("{}\n", yt_netscape_lines.join("\n"))).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
            }
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    ExtractedCookies {
        browser_name: Some(browser_name.to_string()),
        ytm_cookies,
        spotify_sp_dc,
        netscape_file,
    }
}

fn parse_netscape_cookies(content: &str) -> Vec<CookieRow> {
    let mut rows = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() < 7 {
            continue;
        }
        rows.push(CookieRow {
            host: parts[0].to_string(),
            is_subdomain: parts[1].eq_ignore_ascii_case("TRUE"),
            path: parts[2].to_string(),
            is_secure: parts[3].eq_ignore_ascii_case("TRUE"),
            expiry: parts[4].parse::<i64>().unwrap_or(0),
            name: parts[5].to_string(),
            value: parts[6].to_string(),
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_netscape_cookie_lines() {
        let sample = "# Netscape HTTP Cookie File\n\
            .youtube.com\tTRUE\t/\tTRUE\t1999999999\tSAPISID\tyt_secret\n\
            .google.com\tTRUE\t/\tTRUE\t1999999999\tSAPISID\tgoogle_secret\n\
            .spotify.com\tTRUE\t/\tTRUE\t1999999999\tsp_dc\tspot_token\n";
        let rows = parse_netscape_cookies(sample);
        assert_eq!(rows.len(), 3);
        let extracted = build_from_rows("test", &rows, false);
        assert!(extracted.has_ytm_auth());
        assert_eq!(extracted.ytm_sapisid(), Some("yt_secret"));
        assert_eq!(extracted.spotify_sp_dc.as_deref(), Some("spot_token"));
    }

    #[test]
    fn computes_valid_sapisidhash_format() {
        let header = compute_sapisidhash("my_secret", "https://music.youtube.com");
        assert!(header.starts_with("SAPISIDHASH "));
        let parts: Vec<_> = header.split('_').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 40);
    }
}
