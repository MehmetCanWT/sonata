//! Zero-AST YouTube `base.js` cipher hook and stream URL deciphering for Sonata.
//!
//! 1. **Player Script Discovery**: Fetches the active YouTube HTML5 player script (`base.js`) and extracts
//!    its `player_hash` and `signatureTimestamp` (`sts`).
//! 2. **Zero-AST Hook Injection**: Locate the internal URL-signing builder via a compact regex and expose
//!    `globalThis._yt_solve(sig, n)` without fragile AST parsing.
//! 3. **Cross-Platform Cipher Evaluation**: Evaluates `_yt_solve(sig, n)` through an injected
//!    platform [`JsCipherBridge`] (Android WebView bridge or desktop CLI JS runtime).

use super::StreamResolver;
use crate::paths::CorePaths;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static CIPHER_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const PLAYER_CACHE_TTL_SECS: u64 = 86_400;
const DEFAULT_SIGNATURE_TIMESTAMP: u64 = 20_774;
const EMBED_PROBE_URL: &str = "https://www.youtube.com/embed/fHI8X4OXluQ";
const DESKTOP_BROWSER_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

const GLOBAL_POLYFILL_PRELUDE: &str = concat!(
    "var window=globalThis,self=globalThis,",
    "document={createElement:function(){return{style:{}}},body:{},head:{},addEventListener:function(){},",
    "location:{href:\"https://www.youtube.com/watch?v=yt-dlp-wins\",hostname:\"www.youtube.com\",protocol:\"https:\"}},",
    "navigator={userAgent:\"Mozilla/5.0\"},",
    "location=new URL(\"https://www.youtube.com/watch?v=yt-dlp-wins\"),",
    "XMLHttpRequest=function(){};XMLHttpRequest.prototype={open:function(){},send:function(){}};\n"
);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedPlayerJs {
    pub player_hash: String,
    pub signature_timestamp: u64,
    pub hooked_script: String,
    pub cached_at_epoch_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SolvedCipherPair {
    pub sig: Option<String>,
    pub n: Option<String>,
}

pub trait JsCipherBridge: Send + Sync {
    fn ensure_loaded(&self, player_hash: &str, hooked_script: &str) -> bool;
    fn solve(&self, player_hash: &str, sig: &str, n: &str) -> Option<SolvedCipherPair>;
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_private_cache_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cipher_cache");
    let seq = CIPHER_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!("{file_name}.{}.{}.tmp", std::process::id(), seq));

    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let write_res = (|| -> std::io::Result<()> {
        let mut file = opts.open(&tmp)?;
        #[cfg(unix)]
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if write_res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_res
}

fn player_cache_json_path(paths: &CorePaths) -> PathBuf {
    paths.cipher_player_cache_path()
}

fn player_cache_js_path(paths: &CorePaths, player_hash: &str) -> PathBuf {
    let safe_hash: String = player_hash
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    paths.cipher_script_cache_path(&safe_hash)
}

fn load_disk_cached_player_js(paths: &CorePaths) -> Option<PreparedPlayerJs> {
    let raw = fs::read_to_string(player_cache_json_path(paths)).ok()?;
    let parsed: PreparedPlayerJs = serde_json::from_str(&raw).ok()?;
    let age = now_epoch_secs().saturating_sub(parsed.cached_at_epoch_secs);
    (age < PLAYER_CACHE_TTL_SECS
        && !parsed.player_hash.is_empty()
        && parsed.hooked_script.contains("globalThis._yt_solve="))
    .then_some(parsed)
}

fn save_disk_cached_player_js(paths: &CorePaths, prepared: &PreparedPlayerJs) {
    if let Ok(json) = serde_json::to_vec(prepared) {
        let _ = write_private_cache_file(&player_cache_json_path(paths), &json);
    }
    let js_path = player_cache_js_path(paths, &prepared.player_hash);
    if !js_path.exists() {
        let _ = write_private_cache_file(&js_path, prepared.hooked_script.as_bytes());
    }
}

pub(super) fn extract_player_js_url(embed_html: &str) -> Option<(String, String)> {
    static FULL_PATH_RE: OnceLock<Regex> = OnceLock::new();
    static HASH_ONLY_RE: OnceLock<Regex> = OnceLock::new();

    let full_re = FULL_PATH_RE.get_or_init(|| {
        Regex::new(r#"(/s/player/([a-zA-Z0-9_-]+)/[^"'\s\\]+/base\.js)"#).expect("valid regex")
    });
    if let Some(caps) = full_re.captures(embed_html) {
        let path = caps.get(1)?.as_str();
        let hash = caps.get(2)?.as_str().to_string();
        return Some((hash, format!("https://www.youtube.com{path}")));
    }

    let hash_re = HASH_ONLY_RE
        .get_or_init(|| Regex::new(r#"/s/player/([a-zA-Z0-9_-]+)/"#).expect("valid regex"));
    let caps = hash_re.captures(embed_html)?;
    let hash = caps.get(1)?.as_str().to_string();
    let url = format!("https://www.youtube.com/s/player/{hash}/player_es6.vflset/en_US/base.js");
    Some((hash, url))
}

pub(super) fn inject_zero_ast_cipher_hook(
    player_hash: &str,
    raw_base_js: &str,
) -> Option<PreparedPlayerJs> {
    static STS_RE: OnceLock<Regex> = OnceLock::new();
    static BUILDER_RE: OnceLock<Regex> = OnceLock::new();

    let sts_re =
        STS_RE.get_or_init(|| Regex::new(r#"signatureTimestamp\s*:\s*(\d+)"#).expect("valid sts"));
    let signature_timestamp = sts_re
        .captures(raw_base_js)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SIGNATURE_TIMESTAMP);

    let builder_re = BUILDER_RE.get_or_init(|| {
        Regex::new(
            r#"([a-zA-Z0-9_$]+)\s*=\s*function\([^)]*\)\s*\{\s*[a-zA-Z0-9_$]+\s*=\s*new\s+[a-zA-Z0-9_.$]+\([^)]*\);\s*[a-zA-Z0-9_$]+\.set\("alr",\s*"yes"\)"#,
        )
        .expect("valid builder regex")
    });
    let fn_name = builder_re.captures(raw_base_js)?.get(1)?.as_str();
    let tail_idx = raw_base_js.rfind("})(_yt_player)")?;

    let hook = format!(
        ";globalThis._yt_solve=function(sig,n){{var u={fn_name}(\"https://youtube.com/watch?v=yt-dlp-wins\",\"s\",sig?encodeURIComponent(sig):undefined);if(n)u.set(\"n\",n);var p=Object.getPrototypeOf(u),k=Object.keys(p).concat(Object.getOwnPropertyNames(p));for(var i=0;i<k.length;i++){{var x=k[i];if(x!==\"constructor\"&&x!==\"set\"&&x!==\"get\"&&x!==\"clone\"){{try{{u[x]();break}}catch(e){{}}}}}}var s=u.get(\"s\");return JSON.stringify({{sig:s?decodeURIComponent(s):null,n:u.get(\"n\")||null}})}};"
    );

    let mut hooked_script =
        String::with_capacity(GLOBAL_POLYFILL_PRELUDE.len() + raw_base_js.len() + hook.len() + 4);
    hooked_script.push_str(GLOBAL_POLYFILL_PRELUDE);
    hooked_script.push_str(&raw_base_js[..tail_idx]);
    hooked_script.push_str(&hook);
    hooked_script.push_str(&raw_base_js[tail_idx..]);

    Some(PreparedPlayerJs {
        player_hash: player_hash.to_string(),
        signature_timestamp,
        hooked_script,
        cached_at_epoch_secs: now_epoch_secs(),
    })
}

pub fn apply_cipher_solution_to_url(
    raw_url: Option<&str>,
    signature_cipher: Option<&str>,
    mut solve_fn: impl FnMut(Option<&str>, Option<&str>) -> Option<SolvedCipherPair>,
) -> Option<String> {
    let mut base_stream_url: Option<String> = raw_url
        .map(str::trim)
        .filter(|u| u.starts_with("https://"))
        .map(str::to_string);
    let mut raw_sig: Option<String> = None;
    let mut sp_param = "sig".to_string();

    if let Some(sc) = signature_cipher {
        for pair in sc.split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            let decoded = urlencoding::decode(v)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| v.to_string());
            match k {
                "s" if !decoded.is_empty() => raw_sig = Some(decoded),
                "sp" if !decoded.is_empty() => sp_param = decoded,
                "url" if decoded.starts_with("https://") => base_stream_url = Some(decoded),
                _ => {}
            }
        }
    }

    let stream_url = base_stream_url?;
    let (base_path, query_str) = stream_url.split_once('?')?;
    let mut query_parts: Vec<(String, String)> = Vec::new();
    let mut raw_n: Option<String> = None;

    for part in query_str.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            if k == "n" && raw_n.is_none() {
                let decoded_n = urlencoding::decode(v)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| v.to_string());
                if !decoded_n.is_empty() {
                    raw_n = Some(decoded_n);
                }
            }
            query_parts.push((k.to_string(), v.to_string()));
        } else {
            query_parts.push((part.to_string(), String::new()));
        }
    }

    if raw_sig.is_none() && raw_n.is_none() {
        return Some(stream_url);
    }

    let solved = solve_fn(raw_sig.as_deref(), raw_n.as_deref())?;
    if raw_sig.is_some() && solved.sig.as_deref().unwrap_or("").is_empty() {
        return None;
    }
    if raw_n.is_some() && solved.n.as_deref().unwrap_or("").is_empty() {
        return None;
    }

    if let Some(new_n) = solved.n.as_deref().filter(|s| !s.is_empty()) {
        let encoded_n = urlencoding::encode(new_n).into_owned();
        for (k, v) in &mut query_parts {
            if k == "n" {
                *v = encoded_n.clone();
            }
        }
    }

    if let Some(new_sig) = solved.sig.as_deref().filter(|s| !s.is_empty()) {
        let encoded_sig = urlencoding::encode(new_sig).into_owned();
        query_parts.retain(|(k, _)| k != &sp_param);
        query_parts.push((sp_param, encoded_sig));
    }

    let rebuilt_query = query_parts
        .into_iter()
        .map(|(k, v)| if v.is_empty() { k } else { format!("{k}={v}") })
        .collect::<Vec<_>>()
        .join("&");

    Some(format!("{base_path}?{rebuilt_query}"))
}

impl StreamResolver {
    pub fn set_js_cipher_bridge(&self, bridge: Arc<dyn JsCipherBridge>) {
        if let Ok(mut guard) = self.js_cipher_bridge.write() {
            *guard = Some(bridge);
        }
    }

    pub async fn ensure_prepared_player_js(&self) -> Option<PreparedPlayerJs> {
        if let Some(cached) = self.player_cipher.read().await.clone() {
            let age = now_epoch_secs().saturating_sub(cached.cached_at_epoch_secs);
            if age < PLAYER_CACHE_TTL_SECS {
                return Some(cached);
            }
        }

        // Single-flight initialization prevents concurrent base.js downloads, cache races,
        // and duplicate WebView script evaluations.
        let _init_guard = self.player_cipher_init.lock().await;
        if let Some(cached) = self.player_cipher.read().await.clone() {
            let age = now_epoch_secs().saturating_sub(cached.cached_at_epoch_secs);
            if age < PLAYER_CACHE_TTL_SECS {
                return Some(cached);
            }
        }

        let paths_for_load = self.paths.clone();
        if let Some(disk) =
            tokio::task::spawn_blocking(move || load_disk_cached_player_js(&paths_for_load))
                .await
                .ok()
                .flatten()
        {
            *self.player_cipher.write().await = Some(disk.clone());
            self.warm_bridge_with_player(&disk).await;
            return Some(disk);
        }

        let fetched = self.fetch_and_hook_player_js().await?;
        let to_save = fetched.clone();
        let paths_for_save = self.paths.clone();
        let _ = tokio::task::spawn_blocking(move || {
            save_disk_cached_player_js(&paths_for_save, &to_save)
        })
        .await;
        *self.player_cipher.write().await = Some(fetched.clone());
        self.warm_bridge_with_player(&fetched).await;
        Some(fetched)
    }

    async fn warm_bridge_with_player(&self, prepared: &PreparedPlayerJs) {
        let bridge = self.js_cipher_bridge.read().ok().and_then(|g| g.clone());
        if let Some(bridge) = bridge {
            let hash = prepared.player_hash.clone();
            let script = prepared.hooked_script.clone();
            let _ = tokio::task::spawn_blocking(move || bridge.ensure_loaded(&hash, &script)).await;
        }
    }

    async fn fetch_and_hook_player_js(&self) -> Option<PreparedPlayerJs> {
        let embed_resp = self
            .http
            .get(EMBED_PROBE_URL)
            .header("User-Agent", DESKTOP_BROWSER_UA)
            .timeout(Duration::from_millis(4500))
            .send()
            .await
            .ok()?;
        if !embed_resp.status().is_success() {
            return None;
        }
        let embed_html = embed_resp.text().await.ok()?;
        let (player_hash, base_js_url) = extract_player_js_url(&embed_html)?;

        let js_resp = self
            .http
            .get(&base_js_url)
            .header("User-Agent", DESKTOP_BROWSER_UA)
            .timeout(Duration::from_millis(6500))
            .send()
            .await
            .ok()?;
        if !js_resp.status().is_success() {
            return None;
        }
        let raw_base_js = js_resp.text().await.ok()?;
        let prepared = tokio::task::spawn_blocking(move || {
            inject_zero_ast_cipher_hook(&player_hash, &raw_base_js)
        })
        .await
        .ok()??;

        tracing::info!(
            player_hash = %prepared.player_hash,
            signature_timestamp = prepared.signature_timestamp,
            "prepared zero-AST YouTube player cipher script"
        );
        Some(prepared)
    }

    pub(super) async fn solve_player_cipher(
        &self,
        prepared: &PreparedPlayerJs,
        sig: Option<&str>,
        n: Option<&str>,
    ) -> Option<SolvedCipherPair> {
        let bridge = self.js_cipher_bridge.read().ok().and_then(|g| g.clone())?;
        let hash = prepared.player_hash.clone();
        let script = prepared.hooked_script.clone();
        let sig_owned = sig.unwrap_or("").to_string();
        let n_owned = n.unwrap_or("").to_string();
        tokio::task::spawn_blocking(move || {
            if !bridge.ensure_loaded(&hash, &script) {
                return None;
            }
            bridge.solve(&hash, &sig_owned, &n_owned)
        })
        .await
        .ok()
        .flatten()
    }

    pub(super) async fn decipher_format_url(
        &self,
        prepared: &PreparedPlayerJs,
        raw_url: Option<&str>,
        signature_cipher: Option<&str>,
    ) -> Option<String> {
        // Pre-parse signature_cipher / raw_url to extract (raw_sig, raw_n) once before awaiting.
        let mut base_stream_url: Option<String> = raw_url
            .map(str::trim)
            .filter(|u| u.starts_with("https://"))
            .map(str::to_string);
        let mut raw_sig: Option<String> = None;

        if let Some(sc) = signature_cipher {
            for pair in sc.split('&') {
                let Some((k, v)) = pair.split_once('=') else {
                    continue;
                };
                let decoded = urlencoding::decode(v)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| v.to_string());
                match k {
                    "s" if !decoded.is_empty() => raw_sig = Some(decoded),
                    "url" if decoded.starts_with("https://") => base_stream_url = Some(decoded),
                    _ => {}
                }
            }
        }

        let stream_url = base_stream_url?;
        let (_, query_str) = stream_url.split_once('?')?;
        let mut raw_n: Option<String> = None;
        for part in query_str.split('&') {
            if let Some(("n", v)) = part.split_once('=') {
                let decoded_n = urlencoding::decode(v)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| v.to_string());
                if !decoded_n.is_empty() {
                    raw_n = Some(decoded_n);
                    break;
                }
            }
        }

        if raw_sig.is_none() && raw_n.is_none() {
            return Some(stream_url);
        }

        let solved = self
            .solve_player_cipher(prepared, raw_sig.as_deref(), raw_n.as_deref())
            .await?;

        apply_cipher_solution_to_url(raw_url, signature_cipher, |_, _| Some(solved.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_player_hash_and_hooks_synthetic_base_js() {
        let html = r#"<script src="/s/player/6b3d0364/player_es6.vflset/en_US/base.js"></script>"#;
        let (hash, url) = extract_player_js_url(html).expect("extract player url");
        assert_eq!(hash, "6b3d0364");
        assert_eq!(
            url,
            "https://www.youtube.com/s/player/6b3d0364/player_es6.vflset/en_US/base.js"
        );

        let synthetic_js = r#"var _yt_player={};(function(g){var sts={signatureTimestamp:20774};hw=function(a,b,c){d=new g.Url(a);d.set("alr","yes");if(b&&c)d.set(b,c);return d};})(_yt_player);"#;
        let prepared =
            inject_zero_ast_cipher_hook(&hash, synthetic_js).expect("inject zero-ast hook");
        assert_eq!(prepared.player_hash, "6b3d0364");
        assert_eq!(prepared.signature_timestamp, 20774);
        assert!(prepared
            .hooked_script
            .contains("globalThis._yt_solve=function(sig,n){var u=hw("));
    }

    #[test]
    fn rebuilds_deciphered_url_with_sig_and_transformed_n() {
        let sc = "s=raw_sig_challenge&sp=sig&url=https%3A%2F%2Frr3.googlevideo.com%2Fvideoplayback%3Fitag%3D774%26n%3Draw_n_challenge%26sparams%3Did%252Citag";
        let resolved = apply_cipher_solution_to_url(None, Some(sc), |sig, n| {
            assert_eq!(sig, Some("raw_sig_challenge"));
            assert_eq!(n, Some("raw_n_challenge"));
            Some(SolvedCipherPair {
                sig: Some("SOLVED_SIG_774".to_string()),
                n: Some("SOLVED_N_774".to_string()),
            })
        })
        .expect("deciphered url");

        assert_eq!(
            resolved,
            "https://rr3.googlevideo.com/videoplayback?itag=774&n=SOLVED_N_774&sparams=id%2Citag&sig=SOLVED_SIG_774"
        );
    }
}
