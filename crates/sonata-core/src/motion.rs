//! Multi-provider animated album artwork downloader and in-memory RGBA8 decoder.
//!
//! 1. **Multi-Provider Discovery**: Resolves looping motion covers in priority order: Apple Music
//!    AMP catalog `editorialVideo`, Apple Music album HTML fallback, Tidal `album.videoCover`
//!    (`1280x1280.mp4`), and the Community Canvas JSON manifest.
//! 2. **HLS & Direct MP4 Download**: Downloads Apple Music `768x768` `avc1` `#EXT-X-BYTERANGE` fMP4
//!    segments directly, falls back to `ffmpeg` remuxing for segmented HLS, and caches direct `.mp4`
//!    streams up to 16 MB in `art_cache_dir()/motion_<sha1>.mp4`.
//! 3. **Zero-PNG RAM Decode**: Pipes the cached `.mp4` through `ffmpeg` (`-f rawvideo -pix_fmt rgba
//!    pipe:1`) into `MotionCover { width, height, fps, frames_rgba }`.

use crate::config::{resolve_distro_binary, AppConfig};
use crate::media::{extract_apple_motion_video_url, fetch_apple_album_urls, motion_cache_key_hex};
use crate::providers::Track;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const SAFARI_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
const DESKTOP_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

const APPLE_BROWSE_URL: &str = "https://music.apple.com/us/browse";
const APPLE_AMP_BASE: &str = "https://amp-api.music.apple.com/v1/catalog/us";
const APPLE_TOKEN_RETRY_BACKOFF: Duration = Duration::from_secs(30 * 60);

const TIDAL_SEARCH_URL: &str = "https://api.tidal.com/v1/search";
const TIDAL_EMBED_TOKEN: &str = "vNVdglQOjFJJGG2U";

const COMMUNITY_MANIFEST_URL: &str = "https://vivimusicanvas.mkmdevilmi.workers.dev/canvas.json";
const COMMUNITY_TTL: Duration = Duration::from_secs(30 * 60);

const MAX_DOWNLOAD_BYTES: u64 = 16_000_000;
const MIN_VALID_MP4_BYTES: u64 = 16_384;

const COMPILATION_KEYWORDS: &[&str] = &[
    "playlist",
    "set list",
    "essentials",
    "dj mix",
    "apple music",
    "today's hits",
];

const EDITION_WORDS: &[&str] = &[
    "deluxe",
    "expanded",
    "remastered",
    "remix",
    "version",
    "edit",
    "mix",
    "live",
    "acoustic",
];

#[derive(Debug, Clone)]
pub struct MotionCover {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Tightly packed RGBA8 frames (`width * height * 4` bytes each).
    pub frames_rgba: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub length: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HlsSegmentSpec {
    pub map_uri: String,
    pub map_range: ByteRange,
    pub segments: Vec<(String, ByteRange, f32)>,
}

#[derive(Default)]
struct AppleJwtState {
    token: Option<String>,
    expiry_epoch_ms: u64,
    retry_after: Option<Instant>,
    rejected: HashSet<String>,
}

#[derive(Clone)]
struct CommunityEntry {
    song: String,
    artist: String,
    album: String,
    url: String,
}

#[derive(Default)]
struct CommunityManifestState {
    entries: Vec<CommunityEntry>,
    fetched_at: Option<Instant>,
}

static APPLE_JWT_STATE: OnceLock<Mutex<AppleJwtState>> = OnceLock::new();
static COMMUNITY_STATE: OnceLock<Mutex<CommunityManifestState>> = OnceLock::new();

fn apple_jwt_mutex() -> &'static Mutex<AppleJwtState> {
    APPLE_JWT_STATE.get_or_init(|| Mutex::new(AppleJwtState::default()))
}

fn community_mutex() -> &'static Mutex<CommunityManifestState> {
    COMMUNITY_STATE.get_or_init(|| Mutex::new(CommunityManifestState::default()))
}

/// Resolves, caches (`motion_<hex>.mp4`), and decodes an animated motion cover into RGBA8 frames.
pub async fn resolve_motion_cover(
    http: &Client,
    track: &Track,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<MotionCover> {
    if !AppConfig::motion_covers_enabled()
        || track.artist.trim().is_empty()
        || track.title.trim().is_empty()
        || max_frames == 0
    {
        return None;
    }

    let hex = motion_cache_key_hex(track);
    let cache_dir = AppConfig::art_cache_dir();
    let cache_path = cache_dir.join(format!("motion_{hex}.mp4"));

    let has_valid_cache = tokio::fs::metadata(&cache_path)
        .await
        .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
        .unwrap_or(false);

    if !has_valid_cache {
        download_and_cache_motion_mp4(http, track, &cache_path).await?;
    }

    decode_mp4_via_ffmpeg_pipe(&cache_path, max_dim, target_fps, max_frames).await
}

async fn resolve_motion_media_url(http: &Client, track: &Track) -> Option<String> {
    let clean_title = clean_title_packaging(&track.title);
    let clean_artist = clean_artist_packaging(&track.artist);
    let clean_album = (!track.album.trim().is_empty())
        .then(|| clean_title_packaging(&track.album))
        .filter(|s| !s.is_empty());

    if clean_title.is_empty() || clean_artist.is_empty() {
        return None;
    }

    if let Some(url) =
        resolve_apple_amp_motion_url(http, &clean_title, &clean_artist, clean_album.as_deref())
            .await
    {
        tracing::debug!(
            "[sonata:motion] hit provider=apple_amp track={:?}",
            track.title
        );
        return Some(url);
    }

    if let Some(url) = resolve_apple_html_motion_url(http, track).await {
        tracing::debug!(
            "[sonata:motion] hit provider=apple_html track={:?}",
            track.title
        );
        return Some(url);
    }

    if let Some(url) = resolve_tidal_motion_url(http, &clean_title, &clean_artist).await {
        tracing::debug!("[sonata:motion] hit provider=tidal track={:?}", track.title);
        return Some(url);
    }

    if let Some(url) =
        resolve_community_motion_url(http, &clean_title, &clean_artist, clean_album.as_deref())
            .await
    {
        tracing::debug!(
            "[sonata:motion] hit provider=community track={:?}",
            track.title
        );
        return Some(url);
    }

    None
}

async fn resolve_apple_html_motion_url(http: &Client, track: &Track) -> Option<String> {
    let album_urls = fetch_apple_album_urls(http, track).await;
    for album_url in album_urls {
        let Ok(resp) = http
            .get(&album_url)
            .header("User-Agent", SAFARI_UA)
            .timeout(Duration::from_secs(4))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(html) = resp.text().await else {
            continue;
        };
        if let Some(found) = extract_apple_motion_video_url(&html) {
            return Some(found);
        }
    }
    None
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn decode_jwt_segment(seg: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(seg)
        .or_else(|_| URL_SAFE.decode(seg))
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn is_apple_web_player_jwt(jwt: &str) -> bool {
    let mut parts = jwt.split('.');
    let (Some(header_seg), Some(payload_seg)) = (parts.next(), parts.next()) else {
        return false;
    };
    let header = decode_jwt_segment(header_seg).unwrap_or_default();
    let payload = decode_jwt_segment(payload_seg).unwrap_or_default();
    header.contains("WebPlayKid") || payload.contains("AMPWebPlay")
}

fn parse_jwt_expiry_ms(jwt: &str) -> Option<u64> {
    let payload_seg = jwt.split('.').nth(1)?;
    let payload_str = decode_jwt_segment(payload_seg)?;
    let parsed: Value = serde_json::from_str(&payload_str).ok()?;
    let exp = parsed.get("exp").and_then(Value::as_u64).unwrap_or(0);
    (exp > 0).then_some(exp.saturating_mul(1000))
}

async fn obtain_apple_jwt(http: &Client) -> Option<String> {
    let mut guard = apple_jwt_mutex().lock().await;
    let now_ms = now_epoch_ms();
    if let Some(cached) = &guard.token {
        if now_ms + 60_000 < guard.expiry_epoch_ms && !guard.rejected.contains(cached) {
            return Some(cached.clone());
        }
    }
    if let Some(retry_at) = guard.retry_after {
        if Instant::now() < retry_at {
            return None;
        }
    }

    let browse_html = http
        .get(APPLE_BROWSE_URL)
        .header("User-Agent", DESKTOP_UA)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    let script_re = SCRIPT_RE.get_or_init(|| {
        Regex::new(r"/assets/index(?:-legacy)?[~-][A-Za-z0-9_-]+\.js").expect("script regex")
    });
    let jwt_re = JWT_RE.get_or_init(|| {
        Regex::new(r"ey[A-Za-z0-9_-]+\.ey[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").expect("jwt regex")
    });

    let mut script_paths: Vec<String> = Vec::new();
    for m in script_re.find_iter(&browse_html) {
        let s = m.as_str().to_string();
        if !script_paths.contains(&s) {
            script_paths.push(s);
        }
    }

    for path in script_paths.into_iter().take(4) {
        let url = format!("https://music.apple.com{path}");
        let Ok(resp) = http
            .get(&url)
            .header("User-Agent", DESKTOP_UA)
            .timeout(Duration::from_secs(6))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(js) = resp.text().await else {
            continue;
        };

        let mut candidates: Vec<(String, u64)> = Vec::new();
        for m in jwt_re.find_iter(&js) {
            let tok = m.as_str();
            if guard.rejected.contains(tok) {
                continue;
            }
            if let Some(exp) = parse_jwt_expiry_ms(tok) {
                if exp > now_ms && !candidates.iter().any(|(existing, _)| existing == tok) {
                    candidates.push((tok.to_string(), exp));
                }
            }
        }
        if candidates.is_empty() {
            continue;
        }
        let chosen = candidates
            .iter()
            .find(|(tok, _)| is_apple_web_player_jwt(tok))
            .cloned()
            .unwrap_or_else(|| candidates[0].clone());
        guard.token = Some(chosen.0.clone());
        guard.expiry_epoch_ms = chosen.1;
        guard.retry_after = None;
        return Some(chosen.0);
    }

    guard.retry_after = Some(Instant::now() + APPLE_TOKEN_RETRY_BACKOFF);
    None
}

async fn apple_amp_get(http: &Client, url: &str, jwt: &str) -> Option<Value> {
    let resp = http
        .get(url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("Origin", "https://music.apple.com")
        .header("Referer", "https://music.apple.com/")
        .header("User-Agent", DESKTOP_UA)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if resp.status().as_u16() == 401 {
        let mut guard = apple_jwt_mutex().lock().await;
        guard.rejected.insert(jwt.to_string());
        if guard.token.as_deref() == Some(jwt) {
            guard.token = None;
            guard.expiry_epoch_ms = 0;
        }
        return None;
    }
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<Value>().await.ok()
}

pub(crate) fn extract_amp_editorial_video_url(video_obj: &Value) -> Option<String> {
    let pick_key = |key: &str| -> Option<String> {
        let entry = video_obj.get(key)?;
        for field in ["video", "videoUrl", "hlsUrl", "url"] {
            if let Some(u) = entry
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| s.starts_with("http"))
            {
                return Some(u.to_string());
            }
        }
        None
    };

    pick_key("motionDetailSquare")
        .or_else(|| pick_key("motionSquareVideo1x1"))
        .or_else(|| pick_key("motionDetailRaw"))
        .or_else(|| pick_key("motionDetailTall"))
        .or_else(|| pick_key("motionTallVideo3x4"))
}

fn score_apple_candidate(
    song: &Value,
    wanted_title: &str,
    wanted_artist: &str,
    wanted_album: Option<&str>,
) -> Option<i32> {
    let attrs = song.get("attributes")?;
    let raw_hit_title = attrs.get("name").and_then(Value::as_str).unwrap_or("");
    let hit_artist = attrs
        .get("artistName")
        .and_then(Value::as_str)
        .unwrap_or("");
    let raw_hit_album = attrs.get("albumName").and_then(Value::as_str).unwrap_or("");

    if is_compilation_label(raw_hit_title) || is_compilation_label(raw_hit_album) {
        return None;
    }
    if !artists_match(wanted_artist, hit_artist) {
        return None;
    }

    let norm_want = normalize_text(wanted_title);
    let norm_hit = normalize_text(&clean_title_packaging(raw_hit_title));
    if norm_want.is_empty() || norm_hit.is_empty() {
        return None;
    }

    let mut score: i32 = 10;
    if norm_hit == norm_want {
        score += 15;
    } else if norm_hit.contains(&norm_want) || norm_want.contains(&norm_hit) {
        score += 7;
    } else {
        return None;
    }

    if let Some(want_alb) = wanted_album.filter(|a| !a.trim().is_empty()) {
        if !raw_hit_album.is_empty() {
            let norm_want_alb = normalize_text(want_alb);
            let norm_hit_alb = normalize_text(&clean_title_packaging(raw_hit_album));
            if norm_hit_alb == norm_want_alb {
                score += 20;
            } else if norm_hit_alb.contains(&norm_want_alb) || norm_want_alb.contains(&norm_hit_alb)
            {
                score += 10;
            }
        }
    }

    let want_low = wanted_title.to_ascii_lowercase();
    let hit_low = raw_hit_title.to_ascii_lowercase();
    for word in EDITION_WORDS {
        let in_want = want_low.contains(word);
        let in_hit = hit_low.contains(word);
        if in_want && in_hit {
            score += 5;
        } else if in_hit && !in_want {
            score -= 4;
        }
    }

    Some(score)
}

fn extract_apple_album_id(song: &Value) -> Option<String> {
    if let Some(rel_id) = song
        .pointer("/relationships/albums/data/0/id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("pl."))
    {
        return Some(rel_id.to_string());
    }
    let web_url = song
        .pointer("/attributes/url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let after_album = web_url.split("/album/").nth(1)?;
    let before_q = after_album.split('?').next().unwrap_or("");
    let last = before_q.rsplit('/').next().unwrap_or("").trim();
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        Some(last.to_string())
    } else {
        None
    }
}

async fn resolve_apple_amp_motion_url(
    http: &Client,
    title: &str,
    artist: &str,
    album: Option<&str>,
) -> Option<String> {
    let jwt = obtain_apple_jwt(http).await?;
    let primary_artist = split_artists(artist).into_iter().next().unwrap_or_default();
    let mut term = String::new();
    if !primary_artist.is_empty() && !normalize_text(title).contains(&primary_artist) {
        term.push_str(&primary_artist);
        term.push(' ');
    }
    term.push_str(title);
    if let Some(alb) = album.filter(|a| !a.is_empty()) {
        if !title
            .to_ascii_lowercase()
            .contains(&alb.to_ascii_lowercase())
        {
            term.push(' ');
            term.push_str(alb);
        }
    }

    let encoded_term = urlencoding::encode(term.trim());
    let search_url = format!(
        "{APPLE_AMP_BASE}/search?term={encoded_term}&types=songs&limit=10&extend=editorialVideo&include=albums"
    );
    let root = apple_amp_get(http, &search_url, &jwt).await?;

    if let Some(songs) = root
        .pointer("/results/songs/data")
        .and_then(Value::as_array)
    {
        let mut ranked: Vec<(i32, &Value)> = songs
            .iter()
            .filter_map(|song| {
                let score = score_apple_candidate(song, title, artist, album)?;
                (score >= 12).then_some((score, song))
            })
            .collect();
        ranked.sort_by_key(|a| std::cmp::Reverse(a.0));

        let mut tried_album_ids = HashSet::new();
        for (_, song) in ranked {
            if let Some(url) = song
                .pointer("/attributes/editorialVideo")
                .and_then(extract_amp_editorial_video_url)
            {
                return Some(url);
            }
            if let Some(url) = song
                .pointer("/relationships/albums/data/0/attributes/editorialVideo")
                .and_then(extract_amp_editorial_video_url)
            {
                return Some(url);
            }
            let Some(album_id) = extract_apple_album_id(song) else {
                continue;
            };
            if !tried_album_ids.insert(album_id.clone()) {
                continue;
            }
            let album_url = format!("{APPLE_AMP_BASE}/albums/{album_id}?extend=editorialVideo");
            if let Some(album_json) = apple_amp_get(http, &album_url, &jwt).await {
                if let Some(attrs) = album_json.pointer("/data/0/attributes") {
                    let alb_name = attrs.get("name").and_then(Value::as_str).unwrap_or("");
                    if !is_compilation_label(alb_name) {
                        if let Some(url) = attrs
                            .get("editorialVideo")
                            .and_then(extract_amp_editorial_video_url)
                        {
                            return Some(url);
                        }
                    }
                }
            }
        }
    }

    if let Some(alb) = album.filter(|a| !a.trim().is_empty()) {
        return search_apple_amp_album(http, alb, artist, &jwt).await;
    }
    None
}

async fn search_apple_amp_album(
    http: &Client,
    album: &str,
    artist: &str,
    jwt: &str,
) -> Option<String> {
    let term = if album
        .to_ascii_lowercase()
        .contains(&artist.to_ascii_lowercase())
    {
        album.to_string()
    } else {
        format!("{artist} {album}")
    };
    let encoded_term = urlencoding::encode(term.trim());
    let url = format!(
        "{APPLE_AMP_BASE}/search?term={encoded_term}&types=albums&limit=8&extend=editorialVideo"
    );
    let root = apple_amp_get(http, &url, jwt).await?;
    let albums = root.pointer("/results/albums/data")?.as_array()?;
    let want_album = normalize_text(album);

    for entry in albums {
        let Some(attrs) = entry.get("attributes") else {
            continue;
        };
        let hit_album = attrs.get("name").and_then(Value::as_str).unwrap_or("");
        let hit_artist = attrs
            .get("artistName")
            .and_then(Value::as_str)
            .unwrap_or("");
        if is_compilation_label(hit_album) || !artists_match(artist, hit_artist) {
            continue;
        }
        if normalize_text(&clean_title_packaging(hit_album)) != want_album {
            continue;
        }
        if let Some(url) = attrs
            .get("editorialVideo")
            .and_then(extract_amp_editorial_video_url)
        {
            return Some(url);
        }
    }
    None
}

pub(crate) fn tidal_video_url_from_uuid(uuid: &str) -> Option<String> {
    let parts: Vec<&str> = uuid.trim().split('-').collect();
    if parts.len() != 5 || parts.iter().any(|p| p.trim().is_empty()) {
        return None;
    }
    Some(format!(
        "https://resources.tidal.com/videos/{}/1280x1280.mp4",
        parts.join("/")
    ))
}

async fn resolve_tidal_motion_url(http: &Client, title: &str, artist: &str) -> Option<String> {
    let primary_artist = split_artists(artist).into_iter().next().unwrap_or_default();
    let query = if primary_artist.is_empty() {
        title.to_string()
    } else {
        format!("{primary_artist} {title}")
    };
    let encoded = urlencoding::encode(query.trim());
    let url = format!("{TIDAL_SEARCH_URL}?query={encoded}&limit=10&types=TRACKS&countryCode=US");
    let resp = http
        .get(&url)
        .header("X-Tidal-Token", TIDAL_EMBED_TOKEN)
        .header("User-Agent", DESKTOP_UA)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let root: Value = resp.json().await.ok()?;
    let items = root.pointer("/tracks/items")?.as_array()?;
    let want_title = normalize_text(title);

    for track in items {
        let raw_title = track.get("title").and_then(Value::as_str).unwrap_or("");
        if normalize_text(&clean_title_packaging(raw_title)) != want_title {
            continue;
        }
        let credited: String = track
            .get("artists")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(Value::as_str).map(str::trim))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if !artists_match(artist, &credited) {
            continue;
        }
        let Some(video_uuid) = track
            .pointer("/album/videoCover")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        else {
            continue;
        };
        if let Some(video_url) = tidal_video_url_from_uuid(video_uuid) {
            return Some(video_url);
        }
    }
    None
}

async fn load_community_manifest(http: &Client) -> Vec<CommunityEntry> {
    let mut guard = community_mutex().lock().await;
    if !guard.entries.is_empty()
        && guard
            .fetched_at
            .is_some_and(|t| t.elapsed() < COMMUNITY_TTL)
    {
        return guard.entries.clone();
    }

    guard.fetched_at = Some(Instant::now());
    let Ok(resp) = http
        .get(COMMUNITY_MANIFEST_URL)
        .header("User-Agent", DESKTOP_UA)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    else {
        return guard.entries.clone();
    };
    if !resp.status().is_success() {
        return guard.entries.clone();
    }
    let Ok(root) = resp.json::<Value>().await else {
        return guard.entries.clone();
    };
    let Some(items) = root.get("items").and_then(Value::as_array) else {
        return guard.entries.clone();
    };

    let parsed: Vec<CommunityEntry> = items
        .iter()
        .filter_map(|obj| {
            let song = obj.get("song").and_then(Value::as_str)?.trim();
            let artist = obj.get("artist").and_then(Value::as_str)?.trim();
            let url = obj.get("url").and_then(Value::as_str)?.trim();
            if song.is_empty() || artist.is_empty() || !url.starts_with("http") {
                return None;
            }
            let album = obj
                .get("album")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            Some(CommunityEntry {
                song: song.to_string(),
                artist: artist.to_string(),
                album,
                url: url.to_string(),
            })
        })
        .collect();

    if !parsed.is_empty() {
        guard.entries = parsed;
    }
    guard.entries.clone()
}

async fn resolve_community_motion_url(
    http: &Client,
    title: &str,
    artist: &str,
    album: Option<&str>,
) -> Option<String> {
    let entries = load_community_manifest(http).await;
    if entries.is_empty() {
        return None;
    }
    let want_title = normalize_text(title);
    let want_album = album.map(normalize_text).unwrap_or_default();

    let matches: Vec<&CommunityEntry> = entries
        .iter()
        .filter(|entry| {
            let got_title = normalize_text(&clean_title_packaging(&entry.song));
            let title_ok = !got_title.is_empty()
                && (got_title == want_title
                    || want_title.contains(&got_title)
                    || got_title.contains(&want_title));
            title_ok && artists_match(artist, &entry.artist)
        })
        .collect();

    if matches.is_empty() {
        return None;
    }

    let hit = matches
        .iter()
        .find(|entry| {
            let got_album = normalize_text(&clean_title_packaging(&entry.album));
            !want_album.is_empty() && !got_album.is_empty() && got_album == want_album
        })
        .copied()
        .unwrap_or(matches[0]);

    Some(hit.url.clone())
}

async fn download_and_cache_motion_mp4(
    http: &Client,
    track: &Track,
    cache_path: &Path,
) -> Option<()> {
    let master_url = resolve_motion_media_url(http, track).await?;

    if !master_url.to_ascii_lowercase().contains(".m3u8") {
        let resp = http
            .get(&master_url)
            .header("User-Agent", DESKTOP_UA)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        if resp
            .content_length()
            .is_some_and(|len| len > MAX_DOWNLOAD_BYTES)
        {
            return None;
        }
        let bytes = resp.bytes().await.ok()?.to_vec();
        if (bytes.len() as u64) <= MAX_DOWNLOAD_BYTES && (bytes.len() as u64) >= MIN_VALID_MP4_BYTES
        {
            persist_motion_mp4(cache_path, &bytes).await;
            return Some(());
        }
        return None;
    }

    let master_text = http
        .get(&master_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let variant_url = select_motion_hls_variant_url(&master_text)
        .map(|rel| resolve_url_against(&master_url, &rel))
        .unwrap_or_else(|| master_url.clone());

    let variant_text = http
        .get(&variant_url)
        .header("User-Agent", SAFARI_UA)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    if let Some(spec) = parse_variant_m3u8(&variant_text) {
        let mut mp4_bytes = Vec::new();
        let init_url = resolve_url_against(&variant_url, &spec.map_uri);
        if let Some(init_bytes) = fetch_byte_range(http, &init_url, &spec.map_range).await {
            mp4_bytes.extend_from_slice(&init_bytes);

            let mut total_dur = 0.0f32;
            let mut segments_downloaded = 0usize;
            for (seg_uri, seg_range, dur) in &spec.segments {
                if mp4_bytes.len() as u64 + seg_range.length > MAX_DOWNLOAD_BYTES {
                    break;
                }
                let seg_url = resolve_url_against(&variant_url, seg_uri);
                if let Some(seg_bytes) = fetch_byte_range(http, &seg_url, seg_range).await {
                    if !seg_bytes.is_empty() {
                        mp4_bytes.extend_from_slice(&seg_bytes);
                        total_dur += *dur;
                        segments_downloaded += 1;
                    }
                } else if segments_downloaded == 0 {
                    break;
                }
                if total_dur >= 6.0 {
                    break;
                }
            }

            if segments_downloaded > 0 && (mp4_bytes.len() as u64) >= MIN_VALID_MP4_BYTES {
                persist_motion_mp4(cache_path, &mp4_bytes).await;
                return Some(());
            }
        }
    }

    // Fallback for segmented HLS playlists without `#EXT-X-BYTERANGE`: materialize a short loop via ffmpeg.
    transcode_hls_via_ffmpeg(&variant_url, cache_path).await
}

async fn transcode_hls_via_ffmpeg(hls_url: &str, cache_path: &Path) -> Option<()> {
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let tmp = cache_path.with_extension("mp4.tmp");
    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
    cmd.args([
        "-y",
        "-v",
        "error",
        "-i",
        hls_url,
        "-t",
        "5.0",
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-vf",
        "scale=480:480:force_original_aspect_ratio=increase:flags=lanczos,crop=480:480",
        &tmp.to_string_lossy(),
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);

    let ok = tokio::time::timeout(Duration::from_secs(12), cmd.status())
        .await
        .ok()
        .and_then(Result::ok)
        .is_some_and(|s| s.success());
    if !ok {
        let _ = tokio::fs::remove_file(&tmp).await;
        return None;
    }
    let valid = tokio::fs::metadata(&tmp)
        .await
        .map(|m| m.is_file() && m.len() >= MIN_VALID_MP4_BYTES)
        .unwrap_or(false);
    if !valid || tokio::fs::rename(&tmp, cache_path).await.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
        return None;
    }
    Some(())
}

async fn persist_motion_mp4(cache_path: &Path, bytes: &[u8]) {
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let tmp = cache_path.with_extension("mp4.tmp");
    if tokio::fs::write(&tmp, bytes).await.is_ok()
        && tokio::fs::rename(&tmp, cache_path).await.is_err()
    {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
}

async fn decode_mp4_via_ffmpeg_pipe(
    cache_path: &Path,
    max_dim: u32,
    target_fps: f32,
    max_frames: usize,
) -> Option<MotionCover> {
    let dim = max_dim.clamp(256, 768);
    let fps = target_fps.clamp(8.0, 24.0);
    let dur_secs = (max_frames as f32 / fps).clamp(2.0, 6.0);
    let vf = format!(
        "fps={fps:.2},scale={dim}:{dim}:force_original_aspect_ratio=increase:flags=lanczos,crop={dim}:{dim}"
    );

    let ffmpeg_bin = resolve_distro_binary("ffmpeg");
    let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
    cmd.args([
        "-v",
        "error",
        "-i",
        &cache_path.to_string_lossy(),
        "-t",
        &format!("{dur_secs:.2}"),
        "-vf",
        &vf,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true);

    let output = tokio::time::timeout(Duration::from_secs(8), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let frame_bytes = (dim as usize) * (dim as usize) * 4;
    let frames_rgba: Vec<Vec<u8>> = output
        .stdout
        .chunks_exact(frame_bytes)
        .take(max_frames)
        .map(|chunk| chunk.to_vec())
        .collect();

    if frames_rgba.len() < 2 {
        return None;
    }

    Some(MotionCover {
        width: dim,
        height: dim,
        fps,
        frames_rgba,
    })
}

// --- Normalization Helpers ---

pub(crate) fn clean_title_packaging(raw: &str) -> String {
    static RE_PKG: OnceLock<Regex> = OnceLock::new();
    static RE_FEAT: OnceLock<Regex> = OnceLock::new();
    let pkg_re = RE_PKG.get_or_init(|| {
        Regex::new(
            r"(?i)\s*[\(\[]\s*(?:from|official|lyrical|video|audio|visualizer|visualiser|hd|hq|4k|full\s*song|feat\.?|ft\.?|featuring|with)[^\)\]]*[\)\]]|\b(?:official\s+(?:video|audio|music\s+video)|lyrical|full\s+song|4k\s+video)\b",
        )
        .expect("title pkg regex")
    });
    let feat_re = RE_FEAT
        .get_or_init(|| Regex::new(r"(?i)\s+(?:feat|ft|featuring)\.?\s+.*$").expect("feat regex"));

    let step1 = pkg_re.replace_all(raw, " ");
    let step2 = feat_re.replace_all(&step1, " ");
    let before_pipe = step2.split(" | ").next().unwrap_or(&step2);
    let collapsed = before_pipe.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_end_matches(['-', '–', '—', ',', ' ']).trim();
    if trimmed.is_empty() {
        raw.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn clean_artist_packaging(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed.strip_suffix(" - Topic").unwrap_or(trimmed).trim();
    if stripped.is_empty() {
        trimmed.to_string()
    } else {
        stripped.to_string()
    }
}

fn normalize_text(input: &str) -> String {
    let mapped: String = input
        .chars()
        .flat_map(|ch| match ch {
            'İ' | 'ı' => vec!['i'],
            'Ş' | 'ş' => vec!['s'],
            'Ğ' | 'ğ' => vec!['g'],
            'Ü' | 'ü' => vec!['u'],
            'Ö' | 'ö' => vec!['o'],
            'Ç' | 'ç' => vec!['c'],
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

fn split_artists(raw: &str) -> Vec<String> {
    static ARTIST_RE: OnceLock<Regex> = OnceLock::new();
    let re = ARTIST_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:\s*,\s*|\s*&\s*|\s*•\s*|\s*/\s*|\s+×\s+|\s+x\s+|\bfeat\.?\b|\bft\.?\b|\bfeaturing\b|\bwith\b)")
            .expect("artist split regex")
    });
    re.split(raw)
        .map(normalize_text)
        .filter(|s| !s.is_empty())
        .collect()
}

fn artists_match(wanted_raw: &str, candidate_raw: &str) -> bool {
    let wanted = split_artists(wanted_raw);
    let credited = split_artists(candidate_raw);
    if wanted.is_empty() || credited.is_empty() {
        return false;
    }
    wanted.iter().all(|w| credited.iter().any(|c| c == w))
        || credited.iter().all(|c| wanted.iter().any(|w| w == c))
}

fn is_compilation_label(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    COMPILATION_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

// --- HLS Parsing & Byte-Range Fetching ---

async fn fetch_byte_range(http: &Client, url: &str, range: &ByteRange) -> Option<Vec<u8>> {
    if range.length == 0 {
        return None;
    }
    let end = range.offset.saturating_add(range.length).saturating_sub(1);
    let resp = http
        .get(url)
        .header("User-Agent", SAFARI_UA)
        .header("Range", format!("bytes={}-{end}", range.offset))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    Some(resp.bytes().await.ok()?.to_vec())
}

fn resolve_url_against(base_url: &str, relative: &str) -> String {
    if relative.starts_with("https://") || relative.starts_with("http://") {
        return relative.to_string();
    }
    let clean_base = base_url.split(['?', '#']).next().unwrap_or(base_url);
    let dir = match clean_base.rfind('/') {
        Some(idx) => &clean_base[..=idx],
        None => clean_base,
    };
    format!("{dir}{}", relative.trim_start_matches('/'))
}

/// Selects the `768x768` `avc1` variant first (`tier 0`), then `486..=767` (`tier 1`),
/// then `960..=1080` (`tier 2`), then any `avc1` (`tier 3`), breaking ties by highest `BANDWIDTH=`.
pub fn select_motion_hls_variant_url(master_m3u8: &str) -> Option<String> {
    let mut pending_inf: Option<&str> = None;
    let mut candidates: Vec<(u8, std::cmp::Reverse<u64>, String)> = Vec::new();

    for raw in master_m3u8.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            pending_inf = Some(rest);
            continue;
        }
        if line.starts_with('#') {
            pending_inf = None;
            continue;
        }
        let Some(inf) = pending_inf.take() else {
            continue;
        };
        if line.ends_with("_iframes.m3u8") || !inf.contains("avc1") {
            continue;
        }
        let res = parse_m3u8_resolution(inf);
        let bw = parse_m3u8_bandwidth(inf).unwrap_or(0);
        let tier = match res {
            Some((768, 768)) => 0u8,
            Some((w, h)) if (486..=767).contains(&w) && (486..=767).contains(&h) => 1u8,
            Some((w, h)) if (960..=1080).contains(&w) && (960..=1080).contains(&h) => 2u8,
            _ => 3u8,
        };
        candidates.push((tier, std::cmp::Reverse(bw), line.to_string()));
    }

    candidates.sort_by_key(|(tier, rev_bw, _)| (*tier, *rev_bw));
    candidates.into_iter().next().map(|(_, _, url)| url)
}

fn parse_m3u8_bandwidth(inf: &str) -> Option<u64> {
    let idx = inf.find("BANDWIDTH=")?;
    let after = &inf[idx + "BANDWIDTH=".len()..];
    let token = after.split([',', ' ']).next()?;
    token.trim().parse().ok()
}

fn parse_m3u8_resolution(inf: &str) -> Option<(u32, u32)> {
    let idx = inf.find("RESOLUTION=")?;
    let after = &inf[idx + "RESOLUTION=".len()..];
    let token = after.split([',', ' ']).next()?;
    let (w, h) = token.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn parse_byterange_value(raw: &str) -> Option<ByteRange> {
    let clean = raw.trim().trim_matches('"');
    let (len_str, off_str) = clean.split_once('@')?;
    Some(ByteRange {
        length: len_str.trim().parse().ok()?,
        offset: off_str.trim().parse().ok()?,
    })
}

/// Parses `#EXT-X-MAP` and `#EXT-X-BYTERANGE` segment entries from an Apple Music variant `.m3u8`.
pub fn parse_variant_m3u8(m3u8: &str) -> Option<HlsSegmentSpec> {
    let mut map_uri: Option<String> = None;
    let mut map_range: Option<ByteRange> = None;
    let mut pending_range: Option<ByteRange> = None;
    let mut pending_dur: f32 = 2.0;
    let mut segments = Vec::new();

    for raw in m3u8.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            for part in rest.split(',') {
                let p = part.trim();
                if let Some(uri) = p.strip_prefix("URI=") {
                    map_uri = Some(uri.trim_matches('"').to_string());
                } else if let Some(br) = p.strip_prefix("BYTERANGE=") {
                    map_range = parse_byterange_value(br);
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            if let Some(dur_str) = rest.split(',').next() {
                if let Ok(d) = dur_str.trim().parse::<f32>() {
                    pending_dur = d;
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = parse_byterange_value(rest);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(range) = pending_range.take() {
            segments.push((line.to_string(), range, pending_dur));
            let total_dur: f32 = segments.iter().map(|(_, _, d)| *d).sum();
            if total_dur >= 6.0 || segments.len() >= 6 {
                break;
            }
        }
    }

    let map_uri = map_uri?;
    let map_range = map_range?;
    if segments.is_empty() {
        return None;
    }
    Some(HlsSegmentSpec {
        map_uri,
        map_range,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_768x768_avc1_over_1080x1080_and_360x360() {
        let master = "#EXTM3U\n\
            #EXT-X-I-FRAME-STREAM-INF:CODECS=\"avc1.64001f\",RESOLUTION=768x768,URI=\"768_iframes.m3u8\"\n\
            #EXT-X-STREAM-INF:BANDWIDTH=450000,CODECS=\"avc1.64001f\",RESOLUTION=360x360\n\
            360x360_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=900000,CODECS=\"avc1.64001f\",RESOLUTION=486x486\n\
            486x486_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=2300000,CODECS=\"avc1.640020\",RESOLUTION=768x768\n\
            768x768_avc.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=10700000,CODECS=\"avc1.640028\",RESOLUTION=1080x1080\n\
            1080x1080_avc.m3u8\n";
        assert_eq!(
            select_motion_hls_variant_url(master).as_deref(),
            Some("768x768_avc.m3u8")
        );
    }

    #[test]
    fn parses_ext_x_map_and_byterange_from_apple_768_variant_m3u8() {
        let variant = "#EXTM3U\n\
            #EXT-X-VERSION:6\n\
            #EXT-X-TARGETDURATION:4\n\
            #EXT-X-MAP:URI=\"video_768.mp4\",BYTERANGE=\"742@0\"\n\
            #EXTINF:3.000,\n\
            #EXT-X-BYTERANGE:421056@742\n\
            video_768.mp4\n";
        let spec = parse_variant_m3u8(variant).expect("valid variant m3u8");
        assert_eq!(spec.map_uri, "video_768.mp4");
        assert_eq!(
            spec.map_range,
            ByteRange {
                length: 742,
                offset: 0
            }
        );
        assert_eq!(spec.segments.len(), 1);
        assert_eq!(spec.segments[0].0, "video_768.mp4");
        assert_eq!(
            spec.segments[0].1,
            ByteRange {
                length: 421056,
                offset: 742
            }
        );
    }

    #[test]
    fn extracts_amp_editorial_video_and_tidal_video_cover() {
        let editorial = json!({
            "motionDetailSquare": {
                "video": "https://mvod.itunes.apple.com/itunes-assets/square.m3u8"
            },
            "motionDetailTall": {
                "video": "https://mvod.itunes.apple.com/itunes-assets/tall.m3u8"
            }
        });
        assert_eq!(
            extract_amp_editorial_video_url(&editorial).as_deref(),
            Some("https://mvod.itunes.apple.com/itunes-assets/square.m3u8")
        );

        assert_eq!(
            tidal_video_url_from_uuid("12345678-90ab-cdef-1234-567890abcdef").as_deref(),
            Some("https://resources.tidal.com/videos/12345678/90ab/cdef/1234/567890abcdef/1280x1280.mp4")
        );
        assert_eq!(tidal_video_url_from_uuid("invalid-uuid"), None);
    }
}
