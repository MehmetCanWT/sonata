use crate::auth::ExtractedCookies;
use crate::cache::CacheDb;
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{ProviderSource, Track};
use anyhow::{anyhow, Result};
use base64::Engine as _;
use cipher::{BlockDecrypt, KeyInit};
use des::Des;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::Mutex;

const SAAVN_API: &str = "https://www.jiosaavn.com/api.php";

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    pub badge: String,
}

#[derive(Clone)]
pub struct StreamResolver {
    http: Client,
    ytm: YtMusicClient,
    cookies: ExtractedCookies,
    cache: Arc<Mutex<HashMap<String, (Instant, ResolvedStream)>>>,
    cache_db: Option<CacheDb>,
}

impl StreamResolver {
    pub fn new(
        http: Client,
        ytm: YtMusicClient,
        cookies: ExtractedCookies,
        cache_db: Option<CacheDb>,
    ) -> Self {
        Self {
            http,
            ytm,
            cookies,
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_db,
        }
    }

    /// Stage 1: Instant playback path — returns cached Hi-Fi/YT stream or resolves YouTube immediately.
    pub async fn resolve_fast(&self, track: &Track, prefer_hifi_cache: bool) -> Result<ResolvedStream> {
        let hifi_key = format!("hifi:{}", track.id);
        if prefer_hifi_cache {
            {
                let guard = self.cache.lock().await;
                if let Some((at, stream)) = guard.get(&hifi_key) {
                    if at.elapsed() < Duration::from_secs(20 * 60) {
                        return Ok(stream.clone());
                    }
                }
            }
            if let Some(db) = &self.cache_db {
                if let Some((url, badge)) = db.get_stream(&hifi_key) {
                    let stream = ResolvedStream { url, badge };
                    self.cache
                        .lock()
                        .await
                        .insert(hifi_key, (Instant::now(), stream.clone()));
                    return Ok(stream);
                }
            }
        }
        let yt_key = format!("yt:{}", track.id);
        {
            let guard = self.cache.lock().await;
            if let Some((at, stream)) = guard.get(&yt_key) {
                if at.elapsed() < Duration::from_secs(20 * 60) {
                    return Ok(stream.clone());
                }
            }
        }
        if let Some(db) = &self.cache_db {
            if let Some((url, badge)) = db.get_stream(&yt_key) {
                let stream = ResolvedStream { url, badge };
                self.cache
                    .lock()
                    .await
                    .insert(yt_key, (Instant::now(), stream.clone()));
                return Ok(stream);
            }
        }

        let res = self.resolve_youtube(track).await?;
        if let Some(db) = &self.cache_db {
            db.put_stream(&yt_key, &res.url, &res.badge);
        }
        let mut guard = self.cache.lock().await;
        guard.insert(yt_key, (Instant::now(), res.clone()));
        Ok(res)
    }

    /// Stage 2: Background quality upgrade (BitChord pattern) — upgrades to 320kbps AAC if better than current stream.
    pub async fn resolve_hifi_upgrade(
        &self,
        track: &Track,
        current_badge: &str,
    ) -> Option<ResolvedStream> {
        let lower = current_badge.to_ascii_lowercase();
        if lower.contains("flac") || lower.contains("alac") || lower.contains("320") {
            return None;
        }
        if let Some(kbps) = extract_badge_kbps(current_badge) {
            if kbps >= 256 {
                return None;
            }
        }

        let hifi_key = format!("hifi:{}", track.id);
        {
            let guard = self.cache.lock().await;
            if let Some((at, stream)) = guard.get(&hifi_key) {
                if at.elapsed() < Duration::from_secs(20 * 60) {
                    return Some(stream.clone());
                }
            }
        }
        if let Some(db) = &self.cache_db {
            if let Some((url, badge)) = db.get_stream(&hifi_key) {
                let stream = ResolvedStream { url, badge };
                self.cache
                    .lock()
                    .await
                    .insert(hifi_key, (Instant::now(), stream.clone()));
                return Some(stream);
            }
        }

        let saavn = self.try_jiosaavn(track).await?;
        if let Some(db) = &self.cache_db {
            db.put_stream(&hifi_key, &saavn.url, &saavn.badge);
        }
        let mut guard = self.cache.lock().await;
        guard.insert(hifi_key, (Instant::now(), saavn.clone()));
        Some(saavn)
    }

    async fn try_jiosaavn(&self, track: &Track) -> Option<ResolvedStream> {
        let clean_title = clean_search_title(&track.title);
        let query = format!("{} {}", track.artist, clean_title);
        let resp = self
            .http
            .get(SAAVN_API)
            .query(&[
                ("__call", "search.getResults"),
                ("_format", "json"),
                ("_marker", "0"),
                ("api_version", "4"),
                ("ctx", "android"),
                ("q", query.as_str()),
                ("p", "1"),
                ("n", "8"),
            ])
            .header("Accept", "application/json")
            .header("Cookie", "explicit_content=1")
            .timeout(Duration::from_millis(1800))
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let body: Value = resp.json().await.ok()?;
        let results = body.get("results")?.as_array()?;

        // Sort explicit first, matching BitChord's prioritizeExplicit
        let mut candidates: Vec<&Value> = results.iter().collect();
        candidates.sort_by_key(|r| {
            let exp = r
                .get("explicit_content")
                .and_then(|v| v.as_str())
                .unwrap_or("0");
            if exp == "1" { 0 } else { 1 }
        });

        for item in candidates {
            let cand_title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let cand_artists = item
                .pointer("/more_info/artistMap/primary_artists")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let cand_dur = item
                .pointer("/more_info/duration")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok());

            if !is_confident_match(track, cand_title, &cand_artists, cand_dur) {
                continue;
            }

            let supports_320 = item
                .pointer("/more_info/320kbps")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("true"));
            if !supports_320 {
                continue;
            }

            let enc = item
                .pointer("/more_info/encrypted_media_url")
                .and_then(|v| v.as_str())?;
            let decrypted = decrypt_jiosaavn_url(enc)?;
            let (url, kbps) = select_best_saavn_stream(&decrypted, supports_320)?;
            if kbps.unwrap_or(0) >= 320 {
                return Some(ResolvedStream {
                    url,
                    badge: "320kbps AAC · Hi-Fi".to_string(),
                });
            }
        }
        None
    }

    async fn resolve_youtube(&self, track: &Track) -> Result<ResolvedStream> {
        let mut target_url = match track.source {
            ProviderSource::YouTubeMusic => {
                format!("https://music.youtube.com/watch?v={}", track.id)
            }
            ProviderSource::Spotify => {
                let query = format!("{} {}", track.artist, clean_search_title(&track.title));
                if let Ok(results) = self.ytm.search_songs(&query).await {
                    if let Some(best) = pick_best_ytm_match(track, &results) {
                        format!("https://music.youtube.com/watch?v={}", best.id)
                    } else {
                        format!("ytsearch1:{query} official audio")
                    }
                } else {
                    format!("ytsearch1:{query} official audio")
                }
            }
        };

        if target_url.is_empty() {
            target_url = format!("ytsearch1:{} {}", track.artist, track.title);
        }

        let mut cmd = Command::new("yt-dlp");
        cmd.args([
            "--no-playlist",
            "--no-warnings",
            "--quiet",
        ]);
        if let Some(rt) = detect_js_runtime() {
            cmd.arg("--js-runtimes").arg(rt);
        }
        cmd.args([
            "-f",
            "bestaudio[acodec=flac]/bestaudio[acodec=opus]/bestaudio/best",
            "--print",
            "%(acodec)s|%(abr)s|%(url)s",
        ]);
        let mut temp_cookie_copy: Option<std::path::PathBuf> = None;
        if let Some(cookie_file) = &self.cookies.netscape_file {
            if cookie_file.exists() {
                let unique = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let run_copy = cookie_file.with_file_name(format!("yt_cookies_run_{unique}.txt"));
                if std::fs::copy(cookie_file, &run_copy).is_ok() {
                    cmd.arg("--cookies").arg(&run_copy);
                    temp_cookie_copy = Some(run_copy);
                }
            }
        }
        cmd.arg(&target_url);

        let output = cmd.output().await?;
        if let Some(tmp) = temp_cookie_copy {
            let _ = std::fs::remove_file(tmp);
        }
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("yt-dlp failed: {}", err.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|l| l.contains('|'))
            .ok_or_else(|| anyhow!("No audio stream returned by yt-dlp"))?;
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() != 3 {
            return Err(anyhow!("Unexpected yt-dlp stream metadata"));
        }

        let codec = parts[0].trim();
        let abr_num = parts[1].trim().parse::<f64>().ok().map(|v| v.round() as u32);
        let stream_url = parts[2].trim().to_string();

        let badge = match (codec, abr_num) {
            ("flac" | "alac", _) => "FLAC · Lossless".to_string(),
            (c, Some(kbps)) if kbps > 0 => format!("{} {}kbps · High", c.to_uppercase(), kbps),
            (c, _) => format!("{} · High", c.to_uppercase()),
        };

        Ok(ResolvedStream {
            url: stream_url,
            badge,
        })
    }
}

fn extract_badge_kbps(badge: &str) -> Option<u32> {
    badge
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .find(|&n| n >= 48)
}

fn detect_js_runtime() -> Option<&'static str> {
    static RT: OnceLock<Option<&'static str>> = OnceLock::new();
    *RT.get_or_init(|| {
        ["deno", "node", "bun", "quickjs"].into_iter().find(|&c| {
            std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    })
}

fn pick_best_ytm_match<'a>(target: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    if candidates.is_empty() {
        return None;
    }
    candidates.iter().min_by_key(|cand| {
        let dur_diff = match (target.duration_secs, cand.duration_secs) {
            (Some(a), Some(b)) => a.abs_diff(b),
            _ => 10,
        };
        let title_penalty = if normalize_cmp(&cand.title).contains(&normalize_cmp(&target.title)) {
            0
        } else {
            30
        };
        dur_diff + title_penalty
    })
}

fn is_confident_match(
    track: &Track,
    cand_title: &str,
    cand_artists: &str,
    cand_dur: Option<u64>,
) -> bool {
    if let (Some(a), Some(b)) = (track.duration_secs, cand_dur) {
        if a.abs_diff(b) > 5 {
            return false;
        }
    }
    let t_norm = normalize_cmp(&clean_search_title(&track.title));
    let c_norm = normalize_cmp(&clean_search_title(cand_title));
    if t_norm.is_empty() || c_norm.is_empty() {
        return false;
    }
    let title_ok = t_norm == c_norm || c_norm.contains(&t_norm) || t_norm.contains(&c_norm);
    if !title_ok {
        return false;
    }
    let a_norm = normalize_cmp(&track.artist);
    let ca_norm = normalize_cmp(cand_artists);
    a_norm
        .split_whitespace()
        .any(|word| word.len() >= 3 && ca_norm.contains(word))
}

fn normalize_cmp(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn clean_search_title(title: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[](official|lyric|music|audio|video|hd|hq|4k|visualizer|remaster)[^\)\]]*[\)\]]").unwrap()
    });
    re.replace_all(title, "").trim().to_string()
}

/// Decrypts JioSaavn's Base64-encoded DES-ECB `encrypted_media_url` using key `"38346591"`.
pub fn decrypt_jiosaavn_url(encrypted_b64: &str) -> Option<String> {
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(encrypted_b64.trim())
        .ok()?;
    if bytes.is_empty() || bytes.len() % 8 != 0 {
        return None;
    }
    let cipher = Des::new_from_slice(b"38346591").ok()?;
    for chunk in bytes.as_chunks_mut::<8>().0 {
        let block = des::cipher::generic_array::GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
    let pad = *bytes.last()? as usize;
    if pad == 0 || pad > 8 || pad > bytes.len() {
        return None;
    }
    if bytes[bytes.len() - pad..].iter().any(|&b| b as usize != pad) {
        return None;
    }
    bytes.truncate(bytes.len() - pad);
    String::from_utf8(bytes).ok().map(|s| s.trim().to_string())
}

pub fn select_best_saavn_stream(
    decrypted_url: &str,
    supports_320: bool,
) -> Option<(String, Option<u32>)> {
    if decrypted_url.trim().is_empty() {
        return None;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)_(48|96|160|320)\.(mp4|aac|mp3)([?#]|$)").unwrap()
    });
    let Some(caps) = re.captures(decrypted_url) else {
        return Some((decrypted_url.to_string(), None));
    };
    let offered: u32 = caps.get(1)?.as_str().parse().ok()?;
    if !supports_320 {
        return Some((decrypted_url.to_string(), Some(offered)));
    }
    let ext = caps.get(2)?.as_str();
    let suffix = caps.get(3).map_or("", |m| m.as_str());
    let upgraded = re
        .replace(decrypted_url, format!("_320.{ext}{suffix}"))
        .into_owned();
    Some((upgraded, Some(320)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cipher::BlockEncrypt;

    #[test]
    fn decrypts_and_upgrades_jiosaavn_url() {
        let plain = b"https://aac.saavncdn.com/123/song_96.mp4";
        let pad = 8 - (plain.len() % 8);
        let mut buf = plain.to_vec();
        buf.resize(plain.len() + pad, pad as u8);
        let cipher = Des::new_from_slice(b"38346591").unwrap();
        for chunk in buf.as_chunks_mut::<8>().0 {
            let block = des::cipher::generic_array::GenericArray::from_mut_slice(chunk);
            cipher.encrypt_block(block);
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let decrypted = decrypt_jiosaavn_url(&b64).expect("decryption");
        assert_eq!(decrypted, "https://aac.saavncdn.com/123/song_96.mp4");

        let (best, kbps) = select_best_saavn_stream(&decrypted, true).unwrap();
        assert_eq!(kbps, Some(320));
        assert_eq!(best, "https://aac.saavncdn.com/123/song_320.mp4");
    }
}
