//! Portable session cookie models, Netscape parsing, and YouTube Music request signing.
//!
//! 1. **Cookie Model**: `ExtractedCookies` holds YouTube Music cookies, the Spotify `sp_dc` token,
//!    and an optional Netscape cookie file path.
//! 2. **Request Signing**: `compute_sapisidhash` builds time-based `SAPISIDHASH` authorization headers.
//! 3. **Private Persistence**: Creates private directories (`0o700`) and session files (`0o600`)
//!    atomically before writing secret bytes.
//! 4. **Netscape Parsing**: `parse_netscape_cookies` and `build_from_rows` parse cookie jars
//!    without any browser profile or OS discovery dependencies.

use crate::paths::CorePaths;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Creates `path` and enforces `0o700` permissions on Unix.
pub fn ensure_private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Creates or truncates a secret file atomically with `0o600` permissions on Unix.
pub fn create_private_file(path: &Path) -> std::io::Result<fs::File> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let file = opts.open(path)?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

/// Writes secret bytes into a `0o600` file and syncs it to disk.
pub fn write_private_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut file = create_private_file(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    Ok(())
}

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
        let mut pairs: Vec<String> = self
            .ytm_cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        pairs.sort();
        Some(pairs.join("; "))
    }

    pub fn ytm_sapisid(&self) -> Option<&str> {
        self.ytm_cookies
            .get("SAPISID")
            .or_else(|| self.ytm_cookies.get("__Secure-3PAPISID"))
            .or_else(|| self.ytm_cookies.get("__Secure-1PAPISID"))
            .map(|s| s.as_str())
    }

    pub fn ytm_1papisid(&self) -> Option<&str> {
        self.ytm_cookies
            .get("__Secure-1PAPISID")
            .map(|s| s.as_str())
    }

    pub fn ytm_3papisid(&self) -> Option<&str> {
        self.ytm_cookies
            .get("__Secure-3PAPISID")
            .map(|s| s.as_str())
    }

    pub fn ytm_sapisidhash(&self) -> Option<String> {
        self.ytm_sapisidhash_for_origin("https://music.youtube.com")
    }

    pub fn ytm_sapisidhash_for_origin(&self, origin: &str) -> Option<String> {
        let sapisid = self.ytm_sapisid()?;
        Some(compute_sapisidhash_variants(
            sapisid,
            self.ytm_1papisid(),
            self.ytm_3papisid(),
            origin,
        ))
    }

    pub fn has_ytm_auth(&self) -> bool {
        self.ytm_sapisid().is_some()
    }

    pub fn has_spotify_auth(&self) -> bool {
        self.spotify_sp_dc.is_some()
    }

    pub fn from_ytm_cookie_header(header: &str) -> Self {
        let mut ytm_cookies = HashMap::new();
        for part in header.split(';') {
            let trimmed = part.trim();
            if let Some((name, value)) = trimmed.split_once('=') {
                let k = name.trim();
                let v = value.trim();
                if !k.is_empty() && !v.is_empty() {
                    ytm_cookies.insert(k.to_string(), v.to_string());
                }
            }
        }
        Self {
            browser_name: Some("Android WebView".to_string()),
            ytm_cookies,
            spotify_sp_dc: None,
            netscape_file: None,
        }
    }

    pub fn save_ytm_header(&self, paths: &CorePaths) -> std::io::Result<()> {
        let path = paths.ytm_session_cookie_path();
        if let Some(header) = self.ytm_cookie_header() {
            write_private_file(&path, header.as_bytes())?;
        }
        Ok(())
    }

    pub fn load_saved_ytm_header(paths: &CorePaths) -> Option<Self> {
        let path = paths.ytm_session_cookie_path();
        let raw = fs::read_to_string(path).ok()?;
        let parsed = Self::from_ytm_cookie_header(&raw);
        parsed.has_ytm_auth().then_some(parsed)
    }

    pub fn clear_saved_ytm_header(paths: &CorePaths) {
        let path = paths.ytm_session_cookie_path();
        let _ = fs::remove_file(path);
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

pub fn compute_sapisidhash_triple(sapisid: &str, origin: &str) -> String {
    compute_sapisidhash_variants(sapisid, None, None, origin)
}

pub fn compute_sapisidhash_variants(
    sapisid: &str,
    one_p: Option<&str>,
    three_p: Option<&str>,
    origin: &str,
) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hash_for = |secret: &str| -> String {
        let mut hasher = Sha1::new();
        hasher.update(format!("{ts} {secret} {origin}").as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    let h_base = hash_for(sapisid);
    let h_1p = one_p.map(hash_for).unwrap_or_else(|| h_base.clone());
    let h_3p = three_p.map(hash_for).unwrap_or_else(|| h_base.clone());
    format!("SAPISIDHASH {ts}_{h_base} SAPISID1PHASH {ts}_{h_1p} SAPISID3PHASH {ts}_{h_3p}")
}

/// One parsed cookie row from either a Netscape cookie jar or a browser cookie database.
#[derive(Debug, Clone)]
pub struct CookieRow {
    pub host: String,
    pub is_subdomain: bool,
    pub path: String,
    pub is_secure: bool,
    pub expiry: i64,
    pub name: String,
    pub value: String,
}

fn domain_matches(host: &str, domain: &str) -> bool {
    let trimmed = host.trim_start_matches('.');
    trimmed == domain || trimmed.ends_with(&format!(".{domain}"))
}

/// Builds an [`ExtractedCookies`] bundle from parsed cookie rows and optionally writes a private
/// Netscape cookie jar at `netscape_cache_path`.
pub fn build_from_rows(
    browser_name: &str,
    rows: &[CookieRow],
    netscape_cache_path: Option<&Path>,
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

    let netscape_file = if yt_netscape_lines.len() > 1 {
        netscape_cache_path.and_then(|path| {
            let payload = format!("{}\n", yt_netscape_lines.join("\n"));
            write_private_file(path, payload.as_bytes())
                .ok()
                .map(|()| path.to_path_buf())
        })
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

/// Parses a Netscape-format cookie file into structured [`CookieRow`] entries.
pub fn parse_netscape_cookies(content: &str) -> Vec<CookieRow> {
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
        let extracted = build_from_rows("test", &rows, None);
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

        let extracted = ExtractedCookies::from_ytm_cookie_header(
            "__Secure-1PAPISID=one_p; __Secure-3PAPISID=three_p",
        );
        assert!(extracted.has_ytm_auth());
        let triple = extracted
            .ytm_sapisidhash_for_origin("https://music.youtube.com")
            .expect("triple auth header");
        assert!(triple.contains("SAPISIDHASH "));
        assert!(triple.contains("SAPISID1PHASH "));
        assert!(triple.contains("SAPISID3PHASH "));
    }
}
