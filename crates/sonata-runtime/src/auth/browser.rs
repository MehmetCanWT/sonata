//! Firefox/Gecko SQLite and Chromium `yt-dlp` browser cookie extractors for Linux clients.
//!
//! 1. **Browser Discovery**: `list_supported_browsers` detects installed Gecko and Chromium
//!    browser profile directories under the user's home directory.
//! 2. **Gecko Extraction**: Copies `cookies.sqlite` (+ WAL/SHM) into a `0o700` temp directory
//!    and reads YouTube, Google, and Spotify rows with `rusqlite`.
//! 3. **Chromium Extraction**: Runs `yt-dlp --cookies-from-browser` with a bounded timeout and
//!    `0o600` output jar.

use crate::config::{resolve_distro_binary, AppConfig};
use rusqlite::Connection;
use sonata_core::auth::{
    build_from_rows, create_private_file, ensure_private_dir, parse_netscape_cookies, CookieRow,
    ExtractedCookies,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

const YTDLP_COOKIE_TIMEOUT: Duration = Duration::from_secs(15);
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> std::io::Result<Self> {
        ensure_private_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOption {
    pub id: &'static str,
    pub label: &'static str,
    pub detected: bool,
}

#[derive(Debug, Clone, Copy)]
struct BrowserSpec {
    id: &'static str,
    label: &'static str,
    rel_dirs: &'static [&'static str],
    is_gecko: bool,
    ytdlp_name: &'static str,
}

const SUPPORTED_BROWSERS: &[BrowserSpec] = &[
    BrowserSpec {
        id: "zen",
        label: "Zen Browser",
        rel_dirs: &[
            ".zen",
            ".config/zen",
            ".var/app/app.zen_browser.zen/.zen",
            ".var/app/io.github.zen_browser.zen/.zen",
        ],
        is_gecko: true,
        ytdlp_name: "",
    },
    BrowserSpec {
        id: "firefox",
        label: "Firefox",
        rel_dirs: &[
            ".mozilla/firefox",
            ".config/mozilla/firefox",
            ".var/app/org.mozilla.firefox/.mozilla/firefox",
            "snap/firefox/common/.mozilla/firefox",
        ],
        is_gecko: true,
        ytdlp_name: "",
    },
    BrowserSpec {
        id: "librewolf",
        label: "LibreWolf",
        rel_dirs: &[
            ".librewolf",
            ".config/librewolf",
            ".var/app/io.gitlab.librewolf-community/.librewolf",
        ],
        is_gecko: true,
        ytdlp_name: "",
    },
    BrowserSpec {
        id: "floorp",
        label: "Floorp",
        rel_dirs: &[
            ".floorp",
            ".config/floorp",
            ".var/app/one.ablaze.floorp/.floorp",
        ],
        is_gecko: true,
        ytdlp_name: "",
    },
    BrowserSpec {
        id: "chrome",
        label: "Google Chrome",
        rel_dirs: &[
            ".config/google-chrome",
            ".var/app/com.google.Chrome/config/google-chrome",
        ],
        is_gecko: false,
        ytdlp_name: "chrome",
    },
    BrowserSpec {
        id: "chromium",
        label: "Chromium",
        rel_dirs: &[
            ".config/chromium",
            ".var/app/org.chromium.Chromium/config/chromium",
            "snap/chromium/common/chromium",
        ],
        is_gecko: false,
        ytdlp_name: "chromium",
    },
    BrowserSpec {
        id: "brave",
        label: "Brave Browser",
        rel_dirs: &[
            ".config/BraveSoftware/Brave-Browser",
            ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
            "snap/brave/current/.config/BraveSoftware/Brave-Browser",
        ],
        is_gecko: false,
        ytdlp_name: "brave",
    },
    BrowserSpec {
        id: "vivaldi",
        label: "Vivaldi",
        rel_dirs: &[
            ".config/vivaldi",
            ".var/app/com.vivaldi.Vivaldi/config/vivaldi",
        ],
        is_gecko: false,
        ytdlp_name: "vivaldi",
    },
    BrowserSpec {
        id: "edge",
        label: "Microsoft Edge",
        rel_dirs: &[
            ".config/microsoft-edge",
            ".var/app/com.microsoft.Edge/config/microsoft-edge",
        ],
        is_gecko: false,
        ytdlp_name: "edge",
    },
];

fn detect_browser_dir(spec: &BrowserSpec) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    spec.rel_dirs
        .iter()
        .map(|rel| home.join(rel))
        .find(|p| p.exists())
}

/// Lists supported browsers sorted with locally installed/detected browsers first.
pub fn list_supported_browsers() -> Vec<BrowserOption> {
    let mut list: Vec<BrowserOption> = SUPPORTED_BROWSERS
        .iter()
        .map(|spec| BrowserOption {
            id: spec.id,
            label: spec.label,
            detected: detect_browser_dir(spec).is_some(),
        })
        .collect();
    list.sort_by_key(|b| !b.detected);
    list
}

/// Extracts YouTube Music cookies strictly from the user-selected browser (`id`), without falling back.
pub fn extract_ytm_cookies_from_browser(browser_id: &str) -> ExtractedCookies {
    let low = browser_id.trim().to_ascii_lowercase();
    let Some(spec) = SUPPORTED_BROWSERS
        .iter()
        .find(|s| s.id == low || s.label.to_ascii_lowercase() == low)
    else {
        return ExtractedCookies::default();
    };

    let extracted = if spec.is_gecko {
        try_gecko_dirs(spec.label, spec.rel_dirs)
    } else if let Some(dir) = detect_browser_dir(spec) {
        try_chromium_via_ytdlp(spec.ytdlp_name, Some(&dir)).map(|mut c| {
            c.browser_name = Some(spec.label.to_string());
            c
        })
    } else {
        None
    };

    match extracted {
        Some(mut c) if c.has_ytm_auth() => {
            c.spotify_sp_dc = None;
            c.browser_name = Some(spec.label.to_string());
            c
        }
        _ => ExtractedCookies::default(),
    }
}

/// Discovers and extracts YouTube Music & Spotify cookies automatically from installed browsers.
pub fn extract_browser_cookies(preferred_browser: Option<&str>) -> ExtractedCookies {
    let mut result = ExtractedCookies::default();

    if let Some(pref) = preferred_browser {
        let low = pref.to_ascii_lowercase();
        for spec in SUPPORTED_BROWSERS {
            if spec.label.to_ascii_lowercase().contains(&low) || spec.id == low {
                let found = if spec.is_gecko {
                    try_gecko_dirs(spec.label, spec.rel_dirs)
                } else {
                    detect_browser_dir(spec)
                        .and_then(|dir| try_chromium_via_ytdlp(spec.ytdlp_name, Some(&dir)))
                };
                if let Some(c) = found {
                    return c;
                }
            }
        }
    }

    // 1. Try Gecko-based browsers first (pure Rust SQLite read, zero keyring prompts).
    for spec in SUPPORTED_BROWSERS.iter().filter(|s| s.is_gecko) {
        if let Some(found) = try_gecko_dirs(spec.label, spec.rel_dirs) {
            if found.has_ytm_auth() || found.has_spotify_auth() {
                merge_cookies(&mut result, found);
                if result.has_ytm_auth() && result.has_spotify_auth() {
                    return result;
                }
            }
        }
    }

    // 2. Try Chromium-based browsers via yt-dlp cookie extraction if anything is still missing.
    for spec in SUPPORTED_BROWSERS.iter().filter(|s| !s.is_gecko) {
        if let Some(dir) = detect_browser_dir(spec) {
            if let Some(found) = try_chromium_via_ytdlp(spec.ytdlp_name, Some(&dir)) {
                if found.has_ytm_auth() || found.has_spotify_auth() {
                    merge_cookies(&mut result, found);
                    if result.has_ytm_auth() && result.has_spotify_auth() {
                        return result;
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

fn try_gecko_dirs(browser_name: &str, rel_dirs: &[&str]) -> Option<ExtractedCookies> {
    let home = dirs::home_dir()?;
    let mut best = ExtractedCookies::default();
    for rel_dir in rel_dirs {
        let base = home.join(rel_dir);
        if !base.exists() {
            continue;
        }
        if let Some(found) = try_gecko_dir(browser_name, &base) {
            merge_cookies(&mut best, found);
            if best.has_ytm_auth() && best.has_spotify_auth() {
                return Some(best);
            }
        }
    }
    if best.has_ytm_auth() || best.has_spotify_auth() {
        Some(best)
    } else {
        None
    }
}

fn try_gecko_dir(browser_name: &str, base: &Path) -> Option<ExtractedCookies> {
    if !base.exists() {
        return None;
    }

    let mut dbs: Vec<(SystemTime, PathBuf)> = fs::read_dir(base)
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
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let cache_dir = AppConfig::cache_dir();
    let tmp_dir = cache_dir.join(format!("tmp_cookies_{}_{}", std::process::id(), seq));
    let guard = TempDirGuard::new(tmp_dir).ok()?;
    let tmp_db = guard.path().join("cookies.sqlite");
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

    let collected_rows = {
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

        rows.flatten().collect::<Vec<_>>()
    };

    let jar_path = cache_dir.join("yt_cookies.txt");
    let out = build_from_rows(browser_name, &collected_rows, Some(&jar_path));
    Some(out)
}

fn is_valid_ytdlp_browser(browser: &str) -> bool {
    !browser.is_empty()
        && SUPPORTED_BROWSERS
            .iter()
            .any(|spec| !spec.is_gecko && spec.ytdlp_name == browser)
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn format_ytdlp_cookie_arg(
    browser: &str,
    profile_dir: Option<&Path>,
    home_dir: Option<&Path>,
) -> String {
    match profile_dir {
        Some(dir) if !home_dir.is_some_and(|home| dir.starts_with(home.join(".config"))) => {
            format!("{browser}:{}", dir.to_string_lossy())
        }
        _ => browser.to_string(),
    }
}

fn filter_chromium_cookie_dump(
    browser: &str,
    content: &str,
    jar_path: &Path,
) -> Option<ExtractedCookies> {
    let rows = parse_netscape_cookies(content);
    if rows.is_empty() {
        return None;
    }
    let out = build_from_rows(browser, &rows, Some(jar_path));
    if out.has_ytm_auth() || out.has_spotify_auth() {
        Some(out)
    } else {
        None
    }
}

fn try_chromium_via_ytdlp(browser: &str, profile_dir: Option<&Path>) -> Option<ExtractedCookies> {
    if !is_valid_ytdlp_browser(browser) {
        tracing::warn!(
            browser = browser,
            "rejected unsupported yt-dlp cookie browser"
        );
        return None;
    }

    let cache_dir = AppConfig::cache_dir();
    ensure_private_dir(&cache_dir).ok()?;
    let cache_file = cache_dir.join(format!("cookies_{browser}.txt"));
    let _ = fs::remove_file(&cache_file);
    let _ = create_private_file(&cache_file).ok()?;
    let _tmp_guard = TempFileGuard(cache_file.clone());

    let home = dirs::home_dir();
    let cookie_arg = format_ytdlp_cookie_arg(browser, profile_dir, home.as_deref());

    let ytdlp_bin = resolve_distro_binary("yt-dlp");
    let mut child = Command::new(&ytdlp_bin)
        .args([
            "--cookies-from-browser",
            &cookie_arg,
            "--cookies",
            &cache_file.to_string_lossy(),
            "--skip-download",
            "--no-warnings",
            "--quiet",
            "https://music.youtube.com/",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + YTDLP_COOKIE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    tracing::warn!(
                        browser = browser,
                        "yt-dlp --cookies-from-browser timed out after 15s"
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break None,
        }
    }?;

    if !status.success() {
        return None;
    }

    #[cfg(unix)]
    let _ = fs::set_permissions(&cache_file, fs::Permissions::from_mode(0o600));
    let content = fs::read_to_string(&cache_file).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    let jar_path = cache_dir.join("yt_cookies.txt");
    filter_chromium_cookie_dump(browser, &content, &jar_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::auth::write_private_file;

    #[test]
    fn validates_ytdlp_browser_ids() {
        assert!(is_valid_ytdlp_browser("chrome"));
        assert!(is_valid_ytdlp_browser("brave"));
        assert!(!is_valid_ytdlp_browser("firefox"));
        assert!(!is_valid_ytdlp_browser("../evil"));
        assert!(!is_valid_ytdlp_browser(""));
    }

    #[test]
    fn cleans_up_temp_dir_on_drop_and_enforces_private_permissions() {
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "sonata_cookie_guard_{}_{}",
            std::process::id(),
            seq
        ));
        let file = dir.join("yt_cookies.txt");
        {
            let guard = TempDirGuard::new(dir.clone()).expect("create temp dir");
            write_private_file(&file, b"secret=1\n").expect("write private cookie file");
            assert!(guard.path().exists());
            #[cfg(unix)]
            {
                let dir_mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
                let file_mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
                assert_eq!(dir_mode, 0o700);
                assert_eq!(file_mode, 0o600);
            }
        }
        assert!(!dir.exists());
    }

    #[test]
    fn snap_brave_path_is_not_stripped_by_dot_config_check() {
        let home = Path::new("/home/alice");
        let snap_brave =
            Path::new("/home/alice/snap/brave/current/.config/BraveSoftware/Brave-Browser");
        assert_eq!(
            format_ytdlp_cookie_arg("brave", Some(snap_brave), Some(home)),
            format!("brave:{}", snap_brave.display())
        );

        let flatpak_brave =
            Path::new("/home/alice/.var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser");
        assert_eq!(
            format_ytdlp_cookie_arg("brave", Some(flatpak_brave), Some(home)),
            format!("brave:{}", flatpak_brave.display())
        );

        let standard_brave = Path::new("/home/alice/.config/BraveSoftware/Brave-Browser");
        assert_eq!(
            format_ytdlp_cookie_arg("brave", Some(standard_brave), Some(home)),
            "brave"
        );
    }

    #[test]
    fn filter_chromium_cookie_dump_rejects_empty_and_points_to_filtered_jar() {
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "sonata_chromium_filter_{}_{}",
            std::process::id(),
            seq
        ));
        let guard = TempDirGuard::new(dir.clone()).expect("create temp dir");
        let jar_path = guard.path().join("yt_cookies.txt");

        assert!(filter_chromium_cookie_dump("brave", "", &jar_path).is_none());
        assert!(filter_chromium_cookie_dump(
            "brave",
            ".unrelated.org\tTRUE\t/\tTRUE\t1900000000\tSID\tabc\n",
            &jar_path
        )
        .is_none());

        let mixed_dump = concat!(
            ".unrelated.org\tTRUE\t/\tTRUE\t1900000000\tSECRET\tshould_not_leak\n",
            ".youtube.com\tTRUE\t/\tTRUE\t1900000000\tSAPISID\tsapi_val\n",
            ".youtube.com\tTRUE\t/\tTRUE\t1900000000\tSID\tsid_val\n",
        );
        let out =
            filter_chromium_cookie_dump("brave", mixed_dump, &jar_path).expect("valid ytm auth");
        assert_eq!(out.netscape_file.as_deref(), Some(jar_path.as_path()));
        let filtered_text = fs::read_to_string(&jar_path).expect("read filtered jar");
        assert!(filtered_text.contains("SAPISID"));
        assert!(!filtered_text.contains("should_not_leak"));
    }
}
