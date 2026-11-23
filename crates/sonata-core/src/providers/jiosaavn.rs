//! Optional JioSaavn 320kbps AAC external audio provider and metadata matcher.
//!
//! 1. **Catalog Search & Track Details**: Queries JioSaavn's public JSON API for tracks and encrypted stream URLs.
//! 2. **DES-ECB Stream Decryption**: Pure-Rust FIPS 46-3 DES-ECB decryptor with PKCS#5 padding validation (`38346591` key) and `320kbps` AAC URL selection.
//! 3. **Strict Upgrade Matching**: Matches track title, artist overlap, and duration before upgrading non-lossless playback streams.

use super::{ProviderSource, Track};
use crate::error::CoreError;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

const API_BASE: &str = "https://www.jiosaavn.com/api.php";
const DES_KEY: &[u8; 8] = b"38346591";
const MIN_USABLE_KBPS: u32 = 160;
const MAX_DURATION_DIFF_SECS: u64 = 6;
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(6);

pub const ID_PREFIX: &str = "jiosaavn:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJioSaavnStream {
    pub url: String,
    pub bitrate_kbps: u32,
    pub badge: String,
    pub title: String,
    pub artist: String,
    pub duration_secs: u64,
}

/// Searches JioSaavn for tracks matching `query` and returns `Track` models tagged with `jiosaavn:<id>`.
pub async fn search_tracks(client: &Client, query: &str, limit: usize) -> Vec<Track> {
    search_tracks_typed(client, query, limit)
        .await
        .unwrap_or_default()
}

/// Typed JioSaavn catalog search returning `CoreError` on timeout, network, or schema failures.
pub async fn search_tracks_typed(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<Track>, CoreError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let encoded = urlencoding::encode(trimmed);
    let url = format!(
        "{API_BASE}?__call=search.getResults&_format=json&_marker=0&api_version=4&ctx=android&q={encoded}&p=1&n={limit}"
    );
    let body = http_get_with_timeout(client, &url, DEFAULT_HTTP_TIMEOUT).await?;
    parse_search_results_json(&body)
}

/// Pure JSON parser for JioSaavn `search.getResults` responses.
pub fn parse_search_results_json(body: &str) -> Result<Vec<Track>, CoreError> {
    let root: Value = serde_json::from_str(body).map_err(|err| {
        CoreError::invalid_response("jiosaavn", format!("malformed search JSON: {err}"))
    })?;
    let results = root
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_response("jiosaavn", "search payload missing `results` array")
        })?;

    let mut tracks = Vec::with_capacity(results.len());
    for item in results {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let raw_title = item
            .get("title")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| item.get("song").and_then(Value::as_str))
            .unwrap_or("");
        let title = decode_html(raw_title);
        if id.is_empty() || title.is_empty() {
            continue;
        }
        let more = item.get("more_info");
        let extracted_artist = extract_artists(item, more);
        let artist = if extracted_artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            extracted_artist
        };
        let album = more
            .and_then(|m| m.get("album"))
            .and_then(Value::as_str)
            .map(decode_html)
            .unwrap_or_default();
        let duration_secs = json_u64(more.and_then(|m| m.get("duration")));
        let thumbnail_url = item
            .get("image")
            .and_then(Value::as_str)
            .and_then(upgrade_artwork);
        let is_explicit = item
            .get("explicit_content")
            .and_then(Value::as_str)
            .is_some_and(|v| v.trim() == "1");

        tracks.push(Track {
            id: format!("{ID_PREFIX}{id}"),
            title,
            artist,
            album,
            duration_secs,
            thumbnail_url,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("AAC 320kbps · JioSaavn".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit,
        });
    }
    Ok(tracks)
}

/// Resolves a direct JioSaavn stream by track ID (with or without the `jiosaavn:` prefix).
pub async fn resolve_by_id(client: &Client, raw_id: &str) -> Option<ResolvedJioSaavnStream> {
    resolve_by_id_typed(client, raw_id).await.ok().flatten()
}

/// Typed direct JioSaavn stream resolution by track ID.
pub async fn resolve_by_id_typed(
    client: &Client,
    raw_id: &str,
) -> Result<Option<ResolvedJioSaavnStream>, CoreError> {
    let song_id = raw_id.strip_prefix(ID_PREFIX).unwrap_or(raw_id).trim();
    if song_id.is_empty() {
        return Ok(None);
    }
    fetch_song_details_stream_typed(client, song_id).await
}

/// Searches JioSaavn for a matching recording and returns a stream ONLY when its verified
/// bitrate strictly exceeds `current_bitrate_kbps` and passes strict metadata matching.
pub async fn find_higher_quality_stream(
    client: &Client,
    track: &Track,
    current_bitrate_kbps: u32,
) -> Option<ResolvedJioSaavnStream> {
    find_higher_quality_stream_typed(client, track, current_bitrate_kbps)
        .await
        .ok()
        .flatten()
}

/// Typed counterpart to [`find_higher_quality_stream`].
pub async fn find_higher_quality_stream_typed(
    client: &Client,
    track: &Track,
    current_bitrate_kbps: u32,
) -> Result<Option<ResolvedJioSaavnStream>, CoreError> {
    if current_bitrate_kbps >= 320 {
        return Ok(None);
    }
    if track.id.starts_with(ID_PREFIX) {
        return resolve_by_id_typed(client, &track.id).await;
    }

    let clean_title = clean_title_for_match(&track.title);
    let primary_artist = primary_artist_of(&track.artist);
    if clean_title.is_empty() {
        return Ok(None);
    }

    let query = if primary_artist.is_empty() {
        clean_title.clone()
    } else {
        format!("{clean_title} {primary_artist}")
    };
    let encoded = urlencoding::encode(&query);
    let url = format!(
        "{API_BASE}?__call=search.getResults&_format=json&_marker=0&api_version=4&ctx=android&q={encoded}&p=1&n=10"
    );
    let body = http_get_with_timeout(client, &url, DEFAULT_HTTP_TIMEOUT).await?;
    let Some(best_id) = select_best_upgrade_candidate_id(&body, track)? else {
        return Ok(None);
    };

    let Some(stream) = fetch_song_details_stream_typed(client, &best_id).await? else {
        return Ok(None);
    };
    if stream.bitrate_kbps <= current_bitrate_kbps || stream.bitrate_kbps < MIN_USABLE_KBPS {
        return Ok(None);
    }

    tracing::info!(
        "[sonata:jiosaavn] higher quality match track={:?} ({}kbps -> {}kbps)",
        track.title,
        current_bitrate_kbps,
        stream.bitrate_kbps
    );
    Ok(Some(stream))
}

/// Pure candidate matcher selecting the highest-scoring JioSaavn song ID that strictly matches `track`.
pub fn select_best_upgrade_candidate_id(
    search_json_body: &str,
    track: &Track,
) -> Result<Option<String>, CoreError> {
    let root: Value = serde_json::from_str(search_json_body).map_err(|err| {
        CoreError::invalid_response("jiosaavn", format!("malformed search JSON: {err}"))
    })?;
    let results = root
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_response("jiosaavn", "search payload missing `results` array")
        })?;
    let clean_title = clean_title_for_match(&track.title);
    let primary_artist = primary_artist_of(&track.artist);
    let expected_secs = track.duration_secs.unwrap_or(0);
    let norm_want = normalize_core(&clean_title);
    if norm_want.is_empty() {
        return Ok(None);
    }

    let mut candidates: Vec<(String, i32, bool)> = Vec::new();
    for item in results {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let raw_cand_title = item
            .get("title")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| item.get("song").and_then(Value::as_str))
            .unwrap_or("");
        let cand_title = decode_html(raw_cand_title);
        if id.is_empty() || cand_title.is_empty() {
            continue;
        }
        let more = item.get("more_info");
        let cand_artist = extract_artists(item, more);
        let cand_dur = json_u64(more.and_then(|m| m.get("duration"))).unwrap_or(0);
        let cand_explicit = item
            .get("explicit_content")
            .and_then(Value::as_str)
            .is_some_and(|v| v.trim() == "1");

        let norm_cand = normalize_core(&clean_title_for_match(&cand_title));
        if norm_cand != norm_want {
            continue;
        }
        if !primary_artist.is_empty() && !artists_overlap(&track.artist, &cand_artist) {
            continue;
        }
        if expected_secs > 0
            && cand_dur > 0
            && expected_secs.abs_diff(cand_dur) > MAX_DURATION_DIFF_SECS
        {
            continue;
        }

        let mut score: i32 = 100;
        if expected_secs > 0 && cand_dur > 0 {
            score += ((MAX_DURATION_DIFF_SECS - expected_secs.abs_diff(cand_dur)) as i32) * 5;
        }
        if track.is_explicit && cand_explicit {
            score += 20;
        }
        if json_bool_str(more.and_then(|m| m.get("320kbps"))) {
            score += 15;
        }
        candidates.push((id.to_string(), score, cand_explicit));
    }

    candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));
    Ok(candidates.into_iter().next().map(|(id, _, _)| id))
}

async fn fetch_song_details_stream_typed(
    client: &Client,
    song_id: &str,
) -> Result<Option<ResolvedJioSaavnStream>, CoreError> {
    let encoded = urlencoding::encode(song_id);
    let url = format!(
        "{API_BASE}?__call=song.getDetails&_format=json&_marker=0&api_version=4&ctx=android&pids={encoded}"
    );
    let body = http_get_with_timeout(client, &url, DEFAULT_HTTP_TIMEOUT).await?;
    parse_song_details_json(song_id, &body)
}

/// Pure JSON parser and DES-ECB decryptor for JioSaavn `song.getDetails` responses.
pub fn parse_song_details_json(
    song_id: &str,
    body: &str,
) -> Result<Option<ResolvedJioSaavnStream>, CoreError> {
    let root: Value = serde_json::from_str(body).map_err(|err| {
        CoreError::invalid_response("jiosaavn", format!("malformed song details JSON: {err}"))
    })?;
    let Some(song_obj) = root
        .get("songs")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .or_else(|| root.get(song_id))
    else {
        return Err(CoreError::invalid_response(
            "jiosaavn",
            format!("song details payload missing entry for `{song_id}`"),
        ));
    };
    let Some(more) = song_obj.get("more_info") else {
        return Err(CoreError::invalid_response(
            "jiosaavn",
            "song details entry missing `more_info`",
        ));
    };
    let Some(encrypted) = more
        .get("encrypted_media_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(None);
    };

    let raw_url = decrypt_url(encrypted).ok_or_else(|| {
        CoreError::invalid_response("jiosaavn", "failed to decrypt `encrypted_media_url`")
    })?;
    let supports_320 = json_bool_str(more.get("320kbps"));
    let (final_url, bitrate_kbps) = select_best_stream_url(&raw_url, supports_320);
    if bitrate_kbps < MIN_USABLE_KBPS {
        return Ok(None);
    }

    let raw_title = song_obj
        .get("title")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| song_obj.get("song").and_then(Value::as_str))
        .unwrap_or("");
    let title = decode_html(raw_title);
    let artist = extract_artists(song_obj, Some(more));
    let duration_secs = json_u64(more.get("duration")).unwrap_or(0);

    Ok(Some(ResolvedJioSaavnStream {
        url: final_url,
        bitrate_kbps,
        badge: format!("AAC {bitrate_kbps}kbps · JioSaavn"),
        title,
        artist,
        duration_secs,
    }))
}

async fn http_get_with_timeout(
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<String, CoreError> {
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
        )
        .header("Accept", "application/json")
        .header("Accept-Language", "en-IN,en;q=0.9")
        .header("Cookie", "explicit_content=1")
        .header("X-Forwarded-For", "49.36.0.1")
        .header("X-Real-IP", "49.36.0.1")
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                CoreError::timeout(format!("JioSaavn request timed out: {err}"))
            } else {
                CoreError::offline(format!("JioSaavn network error: {err}"))
            }
        })?;
    let status = resp.status();
    if status.as_u16() == 429 {
        return Err(CoreError::rate_limited(None, "JioSaavn rate limited (429)"));
    }
    if !status.is_success() {
        return Err(CoreError::invalid_response(
            "jiosaavn",
            format!("HTTP status {status}"),
        ));
    }
    resp.text().await.map_err(|err| {
        if err.is_timeout() {
            CoreError::timeout(format!("JioSaavn body read timed out: {err}"))
        } else {
            CoreError::invalid_response("jiosaavn", format!("failed to read response body: {err}"))
        }
    })
}

fn json_u64(val: Option<&Value>) -> Option<u64> {
    match val? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn json_bool_str(val: Option<&Value>) -> bool {
    match val {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn extract_artists(item: &Value, more: Option<&Value>) -> String {
    if let Some(primary) = more
        .and_then(|m| m.get("artistMap"))
        .and_then(|am| am.get("primary_artists"))
        .and_then(Value::as_array)
    {
        let mut names = Vec::new();
        for entry in primary {
            if let Some(name) = entry.get("name").and_then(Value::as_str).map(str::trim) {
                if !name.is_empty() && !names.iter().any(|existing: &String| existing == name) {
                    names.push(name.to_string());
                }
            }
        }
        if !names.is_empty() {
            return decode_html(&names.join(", "));
        }
    }
    if let Some(singers) = more
        .and_then(|m| m.get("singers"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return decode_html(singers);
    }
    let subtitle = item.get("subtitle").and_then(Value::as_str).unwrap_or("");
    decode_html(subtitle.split(" - ").next().unwrap_or("").trim())
}

/// Upgrades JioSaavn cover artwork URLs from 50x50/150x150 to 500x500 HTTPS.
pub fn upgrade_artwork(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .replace("http://", "https://")
            .replace("150x150", "500x500")
            .replace("50x50", "500x500"),
    )
}

/// Decodes common HTML entities returned by JioSaavn JSON payloads.
pub fn decode_html(raw: &str) -> String {
    raw.replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

/// Strips featuring clauses and video/lyric tags so candidate title comparisons match cleanly.
pub fn clean_title_for_match(title: &str) -> String {
    static PARENS_FEAT_RE: OnceLock<Regex> = OnceLock::new();
    static BRACKET_NOISE_RE: OnceLock<Regex> = OnceLock::new();

    let feat_re = PARENS_FEAT_RE.get_or_init(|| {
        Regex::new(r"(?i)\((feat|ft|featuring|with)\.?[^)]*\)").expect("valid feat regex")
    });
    let noise_re = BRACKET_NOISE_RE.get_or_init(|| {
        Regex::new(r"(?i)\[(official|lyric|lyrics|audio|video|hd|4k|hq|mv)[^\]]*\]|\((official|lyric|lyrics|audio|video|hd|4k|hq|mv)[^)]*\)")
            .expect("valid bracket noise regex")
    });

    let step1 = feat_re.replace_all(title, " ");
    let step2 = noise_re.replace_all(&step1, " ");
    step2.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalizes text to ASCII/Unicode lowercase alphanumeric tokens for loose matching.
pub fn normalize_core(text: &str) -> String {
    let mapped: String = text
        .chars()
        .flat_map(|ch| match ch {
            'İ' | 'ı' | 'Í' | 'í' | 'Ì' | 'ì' | 'Î' | 'î' | 'Ï' | 'ï' => vec!['i'],
            'Ş' | 'ş' => vec!['s'],
            'Ğ' | 'ğ' => vec!['g'],
            'Ü' | 'ü' | 'Ú' | 'ú' | 'Ù' | 'ù' | 'Û' | 'û' => vec!['u'],
            'Ö' | 'ö' | 'Ó' | 'ó' | 'Ò' | 'ò' | 'Ô' | 'ô' | 'Õ' | 'õ' => vec!['o'],
            'Ç' | 'ç' => vec!['c'],
            'Á' | 'á' | 'À' | 'à' | 'Â' | 'â' | 'Ä' | 'ä' | 'Ã' | 'ã' => vec!['a'],
            'É' | 'é' | 'È' | 'è' | 'Ê' | 'ê' | 'Ë' | 'ë' => vec!['e'],
            'Ñ' | 'ñ' => vec!['n'],
            other => other.to_lowercase().collect(),
        })
        .collect();
    mapped
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extracts the primary artist name from a comma/feature-delimited artist string.
pub fn primary_artist_of(artist: &str) -> String {
    static SPLIT_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPLIT_RE.get_or_init(|| {
        Regex::new(r"(?i),|&|\bfeat\.|\bft\.|\bfeaturing\b").expect("valid split regex")
    });
    re.split(artist)
        .next()
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// Returns true when at least one artist token overlaps between `a` and `b`.
pub fn artists_overlap(a: &str, b: &str) -> bool {
    static SPLIT_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPLIT_RE.get_or_init(|| {
        Regex::new(r"(?i),|&|\bfeat\.|\bft\.|\bfeaturing\b").expect("valid split regex")
    });
    let collect_set = |raw: &str| -> HashSet<String> {
        re.split(raw)
            .map(normalize_core)
            .filter(|s| s.chars().count() >= 2)
            .collect()
    };
    let set_a = collect_set(a);
    let set_b = collect_set(b);
    if set_a.is_empty() || set_b.is_empty() {
        return false;
    }
    set_a.iter().any(|x| {
        set_b
            .iter()
            .any(|y| x == y || x.contains(y) || y.contains(x))
    })
}

/// Upgrades a decrypted JioSaavn stream URL to `_320.<ext>` when 320kbps is supported.
pub fn select_best_stream_url(url: &str, supports_320: bool) -> (String, u32) {
    static BITRATE_RE: OnceLock<Regex> = OnceLock::new();
    let re = BITRATE_RE.get_or_init(|| {
        Regex::new(r"(?i)_(48|96|160|320)\.(mp4|aac|mp3)([?#]|$)").expect("valid bitrate regex")
    });
    if let Some(caps) = re.captures(url) {
        let offered = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(160);
        let ext = caps.get(2).map(|m| m.as_str()).unwrap_or("mp4");
        let tail = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if supports_320 {
            let upgraded = re
                .replace(url, format!("_320.{ext}{tail}").as_str())
                .into_owned();
            return (upgraded, 320);
        }
        return (url.to_string(), offered);
    }
    (url.to_string(), if supports_320 { 320 } else { 160 })
}

/// Decrypts a Base64-encoded DES-ECB/PKCS#5 JioSaavn `encrypted_media_url` using key `"38346591"`.
pub fn decrypt_url(encrypted_base64: &str) -> Option<String> {
    let compacted: String = encrypted_base64
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let ciphertext = BASE64_STANDARD.decode(compacted.as_bytes()).ok()?;
    let plaintext = des_ecb_decrypt(DES_KEY, &ciphertext)?;
    let raw = String::from_utf8(plaintext).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.replacen("http://", "https://", 1))
}

// --- Pure-Rust FIPS 46-3 DES-ECB Decryption ---

const IP: [u8; 64] = [
    58, 50, 42, 34, 26, 18, 10, 2, 60, 52, 44, 36, 28, 20, 12, 4, 62, 54, 46, 38, 30, 22, 14, 6,
    64, 56, 48, 40, 32, 24, 16, 8, 57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11, 3, 61,
    53, 45, 37, 29, 21, 13, 5, 63, 55, 47, 39, 31, 23, 15, 7,
];

const FP: [u8; 64] = [
    40, 8, 48, 16, 56, 24, 64, 32, 39, 7, 47, 15, 55, 23, 63, 31, 38, 6, 46, 14, 54, 22, 62, 30,
    37, 5, 45, 13, 53, 21, 61, 29, 36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27,
    34, 2, 42, 10, 50, 18, 58, 26, 33, 1, 41, 9, 49, 17, 57, 25,
];

const E_TABLE: [u8; 48] = [
    32, 1, 2, 3, 4, 5, 4, 5, 6, 7, 8, 9, 8, 9, 10, 11, 12, 13, 12, 13, 14, 15, 16, 17, 16, 17, 18,
    19, 20, 21, 20, 21, 22, 23, 24, 25, 24, 25, 26, 27, 28, 29, 28, 29, 30, 31, 32, 1,
];

const P_TABLE: [u8; 32] = [
    16, 7, 20, 21, 29, 12, 28, 17, 1, 15, 23, 26, 5, 18, 31, 10, 2, 8, 24, 14, 32, 27, 3, 9, 19,
    13, 30, 6, 22, 11, 4, 25,
];

const PC1: [u8; 56] = [
    57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18, 10, 2, 59, 51, 43, 35, 27, 19, 11, 3, 60,
    52, 44, 36, 63, 55, 47, 39, 31, 23, 15, 7, 62, 54, 46, 38, 30, 22, 14, 6, 61, 53, 45, 37, 29,
    21, 13, 5, 28, 20, 12, 4,
];

const PC2: [u8; 48] = [
    14, 17, 11, 24, 1, 5, 3, 28, 15, 6, 21, 10, 23, 19, 12, 4, 26, 8, 16, 7, 27, 20, 13, 2, 41, 52,
    31, 37, 47, 55, 30, 40, 51, 45, 33, 48, 44, 49, 39, 56, 34, 53, 46, 42, 50, 36, 29, 32,
];

const KEY_SHIFTS: [u32; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

#[rustfmt::skip]
const S_BOXES: [[[u8; 16]; 4]; 8] = [
    [
        [14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7],
        [0, 15, 7, 4, 14, 2, 13, 1, 10, 6, 12, 11, 9, 5, 3, 8],
        [4, 1, 14, 8, 13, 6, 2, 11, 15, 12, 9, 7, 3, 10, 5, 0],
        [15, 12, 8, 2, 4, 9, 1, 7, 5, 11, 3, 14, 10, 0, 6, 13],
    ],
    [
        [15, 1, 8, 14, 6, 11, 3, 4, 9, 7, 2, 13, 12, 0, 5, 10],
        [3, 13, 4, 7, 15, 2, 8, 14, 12, 0, 1, 10, 6, 9, 11, 5],
        [0, 14, 7, 11, 10, 4, 13, 1, 5, 8, 12, 6, 9, 3, 2, 15],
        [13, 8, 10, 1, 3, 15, 4, 2, 11, 6, 7, 12, 0, 5, 14, 9],
    ],
    [
        [10, 0, 9, 14, 6, 3, 15, 5, 1, 13, 12, 7, 11, 4, 2, 8],
        [13, 7, 0, 9, 3, 4, 6, 10, 2, 8, 5, 14, 12, 11, 15, 1],
        [13, 6, 4, 9, 8, 15, 3, 0, 11, 1, 2, 12, 5, 10, 14, 7],
        [1, 10, 13, 0, 6, 9, 8, 7, 4, 15, 14, 3, 11, 5, 2, 12],
    ],
    [
        [7, 13, 14, 3, 0, 6, 9, 10, 1, 2, 8, 5, 11, 12, 4, 15],
        [13, 8, 11, 5, 6, 15, 0, 3, 4, 7, 2, 12, 1, 10, 14, 9],
        [10, 6, 9, 0, 12, 11, 7, 13, 15, 1, 3, 14, 5, 2, 8, 4],
        [3, 15, 0, 6, 10, 1, 13, 8, 9, 4, 5, 11, 12, 7, 2, 14],
    ],
    [
        [2, 12, 4, 1, 7, 10, 11, 6, 8, 5, 3, 15, 13, 0, 14, 9],
        [14, 11, 2, 12, 4, 7, 13, 1, 5, 0, 15, 10, 3, 9, 8, 6],
        [4, 2, 1, 11, 10, 13, 7, 8, 15, 9, 12, 5, 6, 3, 0, 14],
        [11, 8, 12, 7, 1, 14, 2, 13, 6, 15, 0, 9, 10, 4, 5, 3],
    ],
    [
        [12, 1, 10, 15, 9, 2, 6, 8, 0, 13, 3, 4, 14, 7, 5, 11],
        [10, 15, 4, 2, 7, 12, 9, 5, 6, 1, 13, 14, 0, 11, 3, 8],
        [9, 14, 15, 5, 2, 8, 12, 3, 7, 0, 4, 10, 1, 13, 11, 6],
        [4, 3, 2, 12, 9, 5, 15, 10, 11, 14, 1, 7, 6, 0, 8, 13],
    ],
    [
        [4, 11, 2, 14, 15, 0, 8, 13, 3, 12, 9, 7, 5, 10, 6, 1],
        [13, 0, 11, 7, 4, 9, 1, 10, 14, 3, 5, 12, 2, 15, 8, 6],
        [1, 4, 11, 13, 12, 3, 7, 14, 10, 15, 6, 8, 0, 5, 9, 2],
        [6, 11, 13, 8, 1, 4, 10, 7, 9, 5, 0, 15, 14, 2, 3, 12],
    ],
    [
        [13, 2, 8, 4, 6, 15, 11, 1, 10, 9, 3, 14, 5, 0, 12, 7],
        [1, 15, 13, 8, 10, 3, 7, 4, 12, 5, 6, 11, 0, 14, 9, 2],
        [7, 11, 4, 1, 9, 12, 14, 2, 0, 6, 10, 13, 15, 3, 5, 8],
        [2, 1, 14, 7, 4, 10, 8, 13, 15, 12, 9, 0, 3, 5, 6, 11],
    ],
];

fn permute(input: u64, input_bits: usize, table: &[u8]) -> u64 {
    let mut out = 0u64;
    for &pos in table {
        let shift = input_bits - usize::from(pos);
        out = (out << 1) | ((input >> shift) & 1);
    }
    out
}

fn generate_subkeys(key: &[u8; 8]) -> [u64; 16] {
    let key64 = u64::from_be_bytes(*key);
    let key56 = permute(key64, 64, &PC1);
    let mut c = ((key56 >> 28) & 0x0FFF_FFFF) as u32;
    let mut d = (key56 & 0x0FFF_FFFF) as u32;
    let mut subkeys = [0u64; 16];
    for (idx, &shift) in KEY_SHIFTS.iter().enumerate() {
        c = ((c << shift) | (c >> (28 - shift))) & 0x0FFF_FFFF;
        d = ((d << shift) | (d >> (28 - shift))) & 0x0FFF_FFFF;
        let combined = (u64::from(c) << 28) | u64::from(d);
        subkeys[idx] = permute(combined, 56, &PC2);
    }
    subkeys
}

fn feistel(r: u32, subkey: u64) -> u32 {
    let expanded = permute(u64::from(r), 32, &E_TABLE) ^ subkey;
    let mut out = 0u32;
    for (box_idx, s_box) in S_BOXES.iter().enumerate() {
        let shift = 42 - box_idx * 6;
        let chunk = ((expanded >> shift) & 0x3F) as usize;
        let row = ((chunk & 0x20) >> 4) | (chunk & 0x01);
        let col = (chunk >> 1) & 0x0F;
        let val = u32::from(s_box[row][col]);
        out = (out << 4) | val;
    }
    permute(u64::from(out), 32, &P_TABLE) as u32
}

fn des_block(block: [u8; 8], subkeys: &[u64; 16]) -> [u8; 8] {
    let ip = permute(u64::from_be_bytes(block), 64, &IP);
    let mut l = ((ip >> 32) & 0xFFFF_FFFF) as u32;
    let mut r = (ip & 0xFFFF_FFFF) as u32;
    for &sk in subkeys {
        let next_r = l ^ feistel(r, sk);
        l = r;
        r = next_r;
    }
    let pre_out = (u64::from(r) << 32) | u64::from(l);
    permute(pre_out, 64, &FP).to_be_bytes()
}

fn des_ecb_decrypt(key: &[u8; 8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(8) {
        return None;
    }
    let mut subkeys = generate_subkeys(key);
    subkeys.reverse();
    let mut out = Vec::with_capacity(ciphertext.len());
    let (chunks, _) = ciphertext.as_chunks::<8>();
    for &block in chunks {
        out.extend_from_slice(&des_block(block, &subkeys));
    }
    let pad_len = usize::from(*out.last()?);
    if pad_len == 0 || pad_len > 8 || pad_len > out.len() {
        return None;
    }
    let padding = &out[out.len() - pad_len..];
    if !padding.iter().all(|&b| usize::from(b) == pad_len) {
        return None;
    }
    out.truncate(out.len() - pad_len);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn des_ecb_encrypt_for_test(key: &[u8; 8], plaintext: &[u8]) -> Vec<u8> {
        let subkeys = generate_subkeys(key);
        let pad_len = 8 - (plaintext.len() % 8);
        let mut padded = plaintext.to_vec();
        padded.extend(std::iter::repeat_n(pad_len as u8, pad_len));
        let mut out = Vec::with_capacity(padded.len());
        let (chunks, _) = padded.as_chunks::<8>();
        for &block in chunks {
            out.extend_from_slice(&des_block(block, &subkeys));
        }
        out
    }

    #[test]
    fn matches_fips_46_3_des_test_vector() {
        let key: [u8; 8] = [0x13, 0x34, 0x57, 0x79, 0x9B, 0xBC, 0xDF, 0xF1];
        let pt: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
        let mut subkeys = generate_subkeys(&key);
        let ct = des_block(pt, &subkeys);
        assert_eq!(ct, [0x85, 0xE8, 0x13, 0x54, 0x0F, 0x0A, 0xB4, 0x05]);
        subkeys.reverse();
        assert_eq!(des_block(ct, &subkeys), pt);
    }

    #[test]
    fn decrypts_jiosaavn_url_to_https_and_upgrades_bitrate_to_320() {
        let sample_http = "http://aac.saavncdn.com/123/abc_96.mp4";
        let encrypted_bytes = des_ecb_encrypt_for_test(DES_KEY, sample_http.as_bytes());
        let encrypted_b64 = BASE64_STANDARD.encode(&encrypted_bytes);
        let decrypted = decrypt_url(&encrypted_b64).expect("decrypt url");
        assert_eq!(decrypted, "https://aac.saavncdn.com/123/abc_96.mp4");

        let (upgraded_url, upgraded_kbps) = select_best_stream_url(&decrypted, true);
        assert_eq!(upgraded_url, "https://aac.saavncdn.com/123/abc_320.mp4");
        assert_eq!(upgraded_kbps, 320);

        let (kept_url, kept_kbps) = select_best_stream_url(&decrypted, false);
        assert_eq!(kept_url, decrypted);
        assert_eq!(kept_kbps, 96);

        assert!(decrypt_url("not-valid-base64!!!").is_none());
    }

    #[test]
    fn normalizes_titles_html_artwork_and_artist_overlap() {
        assert_eq!(
            decode_html("Daft Punk &amp; The Weeknd&#039;s &quot;Starboy&quot;"),
            "Daft Punk & The Weeknd's \"Starboy\""
        );
        assert_eq!(
            clean_title_for_match("Midnight Dreams (feat. Guest) [Official Video]"),
            "Midnight Dreams"
        );
        assert_eq!(normalize_core("İstanbul Night!"), "istanbul night");
        assert_eq!(primary_artist_of("The Weeknd, Daft Punk"), "The Weeknd");
        assert!(artists_overlap("The Weeknd, Daft Punk", "The Weeknd"));
        assert!(!artists_overlap("Daft Punk", "Drake"));
        assert_eq!(
            upgrade_artwork("http://c.saavncdn.com/000/cover_150x150.jpg").as_deref(),
            Some("https://c.saavncdn.com/000/cover_500x500.jpg")
        );
        assert_eq!(upgrade_artwork("   "), None);
    }

    #[test]
    fn handles_malformed_json_html_decryption_failure_and_metadata_mismatch() {
        // Malformed HTML/JSON returns InvalidResponse (never silent empty success).
        assert!(matches!(
            parse_search_results_json("<html><body>502 Bad Gateway</body></html>"),
            Err(CoreError::InvalidResponse { ref provider, .. }) if provider == "jiosaavn"
        ));
        assert!(matches!(
            parse_search_results_json(r#"{"status":"ok"}"#),
            Err(CoreError::InvalidResponse { .. })
        ));

        // Encryption decode failures: invalid base64, invalid block size, invalid PKCS#5 padding.
        assert!(decrypt_url("!!!not-base64!!!").is_none());
        assert!(decrypt_url(&BASE64_STANDARD.encode(b"short")).is_none());
        let bad_pad_ct = des_block(
            [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x04],
            &generate_subkeys(DES_KEY),
        );
        assert!(decrypt_url(&BASE64_STANDARD.encode(bad_pad_ct)).is_none());

        // Song details with corrupted encrypted_media_url returns InvalidResponse.
        let bad_details = r#"{"songs":[{"title":"Test","more_info":{"encrypted_media_url":"not-valid","320kbps":"true","duration":"180"}}]}"#;
        assert!(matches!(
            parse_song_details_json("song1", bad_details),
            Err(CoreError::InvalidResponse { ref provider, .. }) if provider == "jiosaavn"
        ));

        // Bitrate selection below 160kbps returns Ok(None).
        let enc_96 = BASE64_STANDARD.encode(des_ecb_encrypt_for_test(
            DES_KEY,
            b"http://aac.saavncdn.com/123/abc_96.mp4?token=1",
        ));
        let low_details = format!(
            r#"{{"songs":[{{"title":"Low &amp; Slow","more_info":{{"encrypted_media_url":"{enc_96}","320kbps":"false","duration":"200","singers":"Artist"}}}}]}}"#
        );
        assert_eq!(
            parse_song_details_json("song2", &low_details).expect("valid parse"),
            None
        );

        // Same payload with 320kbps=true upgrades cleanly while preserving query parameters.
        let hi_details = format!(
            r#"{{"songs":[{{"title":"High &amp; Clear","more_info":{{"encrypted_media_url":"{enc_96}","320kbps":"true","duration":"200","singers":"Artist"}}}}]}}"#
        );
        let resolved = parse_song_details_json("song2", &hi_details)
            .expect("valid parse")
            .expect("320kbps stream");
        assert_eq!(
            resolved.url,
            "https://aac.saavncdn.com/123/abc_320.mp4?token=1"
        );
        assert_eq!(resolved.bitrate_kbps, 320);
        assert_eq!(resolved.title, "High & Clear");

        // Strict metadata mismatch rejects wrong artist, wrong title, or duration diff > 6s.
        let search_payload = r#"{
            "results": [
                {"id":"wrong_artist","title":"Starboy","more_info":{"duration":"230","singers":"Other Guy","320kbps":"true"}},
                {"id":"wrong_dur","title":"Starboy","more_info":{"duration":"260","singers":"The Weeknd","320kbps":"true"}},
                {"id":"exact_hit","title":"Starboy (feat. Daft Punk)","explicit_content":"1","more_info":{"duration":"231","singers":"The Weeknd, Daft Punk","320kbps":"true"}}
            ]
        }"#;
        let wanted = Track {
            id: "yt_starboy".into(),
            title: "Starboy [Official Audio]".into(),
            artist: "The Weeknd".into(),
            album: "Starboy".into(),
            duration_secs: Some(230),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: true,
        };
        assert_eq!(
            select_best_upgrade_candidate_id(search_payload, &wanted)
                .expect("valid search")
                .as_deref(),
            Some("exact_hit")
        );

        let mismatched = Track {
            artist: "Completely Unrelated".into(),
            ..wanted
        };
        assert_eq!(
            select_best_upgrade_candidate_id(search_payload, &mismatched).expect("valid search"),
            None
        );
    }

    #[tokio::test]
    async fn reports_timeout_and_supports_cancellation_on_stalled_http() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local listener");
        let addr = listener.local_addr().expect("local addr");
        let accept_task = tokio::spawn(async move {
            let mut sockets = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                sockets.push(socket);
            }
        });

        let client = Client::new();
        let url = format!("http://{addr}/stalled");

        // Timeout test: stalled server returns CoreError::Timeout.
        let err = http_get_with_timeout(&client, &url, Duration::from_millis(80))
            .await
            .expect_err("stalled endpoint must time out");
        assert!(matches!(err, CoreError::Timeout { .. }));

        // Cancellation test: dropping an in-flight request completes promptly without hanging.
        let fut = http_get_with_timeout(&client, &url, Duration::from_secs(10));
        let cancelled = tokio::time::timeout(Duration::from_millis(40), fut).await;
        assert!(cancelled.is_err(), "outer timeout cancels in-flight future");

        accept_task.abort();
    }
}
