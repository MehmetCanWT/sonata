//! External lyrics provider adapters for Sonata (lrc.red, BetterLyrics, LyricsPlus, TTML, KuGou KRC/LRC, NetEase YRC/LRC, and Genius).
//!
//! 1. **Apple TTML Catalogues**: `fetch_lrcred_synced` (`lrc.red` by ISRC with strict version/duration
//!    matching), `fetch_betterlyrics_synced` (`lyrics-api.boidu.dev`), `fetch_lyricsplus_synced`
//!    (LyricsPlus v2 syllable-timed mirrors), and `fetch_ttml_synced` (`lyrics-api.binimum.org`).
//! 2. **KuGou KRC & LRC**: `fetch_kugou_synced` tries XOR+zlib encrypted `fmt=krc` word-timed lyrics
//!    first and falls back to plain `fmt=lrc`.
//! 3. **NetEase & QQ Music**: `fetch_netease_synced` queries `api/song/lyric/v1` for word-timed
//!    YRC (or LRC fallback), and `fetch_qqmusic_synced` queries QQ Music base64 LRC sheets.
//! 4. **Genius Scraper**: `fetch_genius_lyrics` extracts plain lyrics from Genius HTML containers.

use crate::lyrics::{
    artist_names_set, lyrics_candidate_matches, parse_lrc, title_core, title_version_words, ttml,
    LyricLine, LyricVoice, LyricWord,
};
use base64::Engine as _;
use flate2::read::ZlibDecoder;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read as _;
use std::sync::OnceLock;
use std::time::Duration;

const KRC_CIPHER: [u8; 16] = *b"@Gaw^2tGQ61-\xce\xd2ni";
const SONATA_LYRICS_UA: &str = "Sonata/0.1.0 (lyrics lookup)";
const LRCRED_DURATION_TOLERANCE_SECS: f64 = 3.5;

#[derive(Deserialize)]
struct LrcRedSearchResponse {
    #[serde(default)]
    hits: Vec<LrcRedHit>,
}

#[derive(Deserialize)]
struct LrcRedHit {
    #[serde(default)]
    isrc: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

fn valid_lrcred_url(isrc: &str) -> Option<String> {
    static RE_ISRC: OnceLock<Regex> = OnceLock::new();
    let re = RE_ISRC.get_or_init(|| Regex::new(r"^[A-Z]{2}[A-Z0-9]{3}\d{7}$").unwrap());
    let norm = isrc.trim().to_ascii_uppercase();
    re.is_match(&norm)
        .then(|| format!("https://lrc.red/s/{norm}.ttml"))
}

/// Fetches per-word / dual-voice Apple Music TTML lyrics directly from `lrc.red` by ISRC,
/// enforcing exact core title, identical recording-version words, shared credited artist,
/// and tight fractional duration tolerance (`<= 3.5s`).
pub async fn fetch_lrcred_synced(
    http: &Client,
    title: &str,
    artist: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    let clean_title = title.trim();
    if clean_title.is_empty() {
        return None;
    }
    let query = if artist.trim().is_empty() || artist.eq_ignore_ascii_case("unknown artist") {
        clean_title.to_string()
    } else {
        format!("{clean_title} {}", artist.trim())
    };
    let resp = http
        .get("https://lrc.red/search.json")
        .query(&[("q", query.as_str())])
        .header("User-Agent", SONATA_LYRICS_UA)
        .timeout(Duration::from_millis(3800))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload: LrcRedSearchResponse = resp.json().await.ok()?;
    let wanted_core = title_core(clean_title);
    let wanted_version = title_version_words(clean_title);
    let wanted_artists = artist_names_set(artist);
    let target_dur = duration_secs.filter(|&d| d > 0).map(|d| d as f64);

    let best = payload
        .hits
        .into_iter()
        .filter_map(|hit| {
            let url = valid_lrcred_url(hit.isrc.as_deref()?)?;
            let cand_title = hit.title.as_deref()?;
            if title_core(cand_title) != wanted_core {
                return None;
            }
            if title_version_words(cand_title) != wanted_version {
                return None;
            }
            if !wanted_artists.is_empty() && !wanted_artists.iter().all(|a| a == "unknown artist") {
                let cand_artists = artist_names_set(hit.artist.as_deref().unwrap_or_default());
                if !cand_artists.iter().any(|a| wanted_artists.contains(a)) {
                    return None;
                }
            }
            let dist = match (target_dur, hit.duration.filter(|&d| d > 0.0)) {
                (Some(t), Some(c)) => {
                    let d = (c - t).abs();
                    if d > LRCRED_DURATION_TOLERANCE_SECS {
                        return None;
                    }
                    d
                }
                _ => 0.0,
            };
            Some((dist, url))
        })
        .min_by(|(a, _), (b, _)| a.total_cmp(b))?;

    let ttml_resp = http
        .get(&best.1)
        .header("User-Agent", SONATA_LYRICS_UA)
        .timeout(Duration::from_millis(3800))
        .send()
        .await
        .ok()?;
    if !ttml_resp.status().is_success() {
        return None;
    }
    let xml = ttml_resp.text().await.ok()?;
    ttml::parse_ttml(&xml).filter(|lines| !lines.is_empty())
}

#[derive(Deserialize)]
struct BetterLyricsResponse {
    #[serde(default)]
    ttml: Option<String>,
}

/// Fetches cached Apple Music TTML lyrics from BetterLyrics (`lyrics-api.boidu.dev`).
pub async fn fetch_betterlyrics_synced(
    http: &Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    if title.trim().is_empty() || artist.trim().is_empty() {
        return None;
    }
    let dur_str = duration_secs.filter(|&d| d > 0).map(|d| d.to_string());
    let mut params = vec![("s", title.trim()), ("a", artist.trim())];
    if let Some(d) = dur_str.as_deref() {
        params.push(("d", d));
    }
    if !album.trim().is_empty() {
        params.push(("al", album.trim()));
    }
    let resp = http
        .get("https://lyrics-api.boidu.dev/getLyrics")
        .query(&params)
        .header("User-Agent", SONATA_LYRICS_UA)
        .timeout(Duration::from_millis(3500))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload: BetterLyricsResponse = resp.json().await.ok()?;
    let xml = payload.ttml.as_deref()?.trim();
    if xml.is_empty() {
        return None;
    }
    ttml::parse_ttml(xml).filter(|lines| !lines.is_empty())
}

#[derive(Deserialize)]
struct LyricsPlusResponse {
    #[serde(default)]
    lyrics: Vec<LyricsPlusLine>,
    #[serde(default)]
    metadata: Option<LyricsPlusMetadata>,
}

#[derive(Deserialize)]
struct LyricsPlusMetadata {
    #[serde(default)]
    agents: HashMap<String, LyricsPlusAgent>,
}

#[derive(Deserialize)]
struct LyricsPlusAgent {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Deserialize)]
struct LyricsPlusLine {
    #[serde(default)]
    time: Option<u64>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    syllabus: Vec<LyricsPlusSyllable>,
    #[serde(default)]
    element: Option<Value>,
}

#[derive(Deserialize)]
struct LyricsPlusSyllable {
    #[serde(default)]
    time: Option<u64>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    text: Option<String>,
}

/// Fetches syllable-timed lyrics from LyricsPlus v2 public mirrors concurrently.
pub async fn fetch_lyricsplus_synced(
    http: &Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    if title.trim().is_empty() || artist.trim().is_empty() {
        return None;
    }
    const MIRRORS: [&str; 3] = [
        "https://lyricsplus.prjktla.my.id",
        "https://lyricsplus.atomix.one",
        "https://lyricsplus.binimum.org",
    ];
    let (r0, r1, r2) = tokio::join!(
        query_lyricsplus_mirror(http, MIRRORS[0], title, artist, album, duration_secs),
        query_lyricsplus_mirror(http, MIRRORS[1], title, artist, album, duration_secs),
        query_lyricsplus_mirror(http, MIRRORS[2], title, artist, album, duration_secs),
    );
    r0.or(r1).or(r2)
}

async fn query_lyricsplus_mirror(
    http: &Client,
    host: &str,
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    let dur_str = duration_secs.filter(|&d| d > 0).map(|d| d.to_string());
    let mut params = vec![("title", title.trim()), ("artist", artist.trim())];
    if let Some(d) = dur_str.as_deref() {
        params.push(("duration", d));
    }
    if !album.trim().is_empty() {
        params.push(("album", album.trim()));
    }
    let resp = http
        .get(format!("{host}/v2/lyrics/get"))
        .query(&params)
        .header("User-Agent", SONATA_LYRICS_UA)
        .timeout(Duration::from_millis(3500))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload: LyricsPlusResponse = resp.json().await.ok()?;
    parse_lyricsplus_response(payload)
}

fn parse_lyricsplus_response(payload: LyricsPlusResponse) -> Option<Vec<LyricLine>> {
    let mut person_aliases: HashMap<String, bool> = HashMap::new();
    if let Some(meta) = payload.metadata {
        for (id, agent) in meta.agents {
            let is_person = agent.kind.as_deref() == Some("person");
            let key = agent.alias.unwrap_or(id);
            person_aliases.insert(key, is_person);
        }
    }
    let mut lead_singer: Option<String> = None;
    let mut out = Vec::new();
    for line in payload.lyrics {
        let Some(time_ms) = line.time else {
            continue;
        };
        let singer = line
            .element
            .as_ref()
            .and_then(|el| el.get("singer"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let says_opposite = line
            .element
            .as_ref()
            .and_then(|el| el.as_array())
            .is_some_and(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .any(|tag| tag == "opposite" || tag == "right")
            });
        let voice = if says_opposite {
            LyricVoice::Counter
        } else if let Some(ref s) = singer {
            let is_person = person_aliases.get(s).copied().unwrap_or(true);
            if is_person {
                if lead_singer.is_none() {
                    lead_singer = Some(s.clone());
                }
                if lead_singer.as_deref() != Some(s.as_str()) {
                    LyricVoice::Counter
                } else {
                    LyricVoice::Lead
                }
            } else {
                LyricVoice::Lead
            }
        } else {
            LyricVoice::Lead
        };

        let words = merge_lyricsplus_syllables(&line.syllabus);
        let text = if !words.is_empty() {
            words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            line.text.as_deref().unwrap_or_default().trim().to_string()
        };
        if text.is_empty() {
            continue;
        }
        let start_secs = words
            .first()
            .map(|w| w.start_secs.min(time_ms as f64 / 1000.0))
            .unwrap_or(time_ms as f64 / 1000.0);
        out.push(LyricLine {
            time_secs: start_secs,
            text,
            words,
            voice,
        });
    }
    out.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    (!out.is_empty()).then_some(out)
}

fn merge_lyricsplus_syllables(syllables: &[LyricsPlusSyllable]) -> Vec<LyricWord> {
    let mut words: Vec<LyricWord> = Vec::new();
    let mut current_text = String::new();
    let mut current_start = 0.0f64;
    let mut current_end = 0.0f64;
    let mut has_active = false;

    let flush = |words: &mut Vec<LyricWord>,
                 text: &mut String,
                 start: f64,
                 end: &mut f64,
                 active: &mut bool| {
        let trimmed = text.trim();
        if *active && !trimmed.is_empty() {
            words.push(LyricWord {
                start_secs: start,
                end_secs: Some((*end).max(start + 0.04)),
                text: trimmed.to_string(),
            });
        }
        text.clear();
        *end = 0.0;
        *active = false;
    };

    for syl in syllables {
        let Some(raw) = syl.text.as_deref() else {
            continue;
        };
        let Some(t_ms) = syl.time else {
            continue;
        };
        let s_secs = t_ms as f64 / 1000.0;
        let e_secs = t_ms.saturating_add(syl.duration.unwrap_or(0)) as f64 / 1000.0;
        if raw.trim().is_empty() {
            flush(
                &mut words,
                &mut current_text,
                current_start,
                &mut current_end,
                &mut has_active,
            );
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            flush(
                &mut words,
                &mut current_text,
                current_start,
                &mut current_end,
                &mut has_active,
            );
        }
        if !has_active {
            current_start = s_secs;
            current_end = e_secs;
            has_active = true;
        } else {
            current_end = current_end.max(e_secs);
        }
        current_text.push_str(raw.trim());
        if raw.ends_with(char::is_whitespace) {
            flush(
                &mut words,
                &mut current_text,
                current_start,
                &mut current_end,
                &mut has_active,
            );
        }
    }
    flush(
        &mut words,
        &mut current_text,
        current_start,
        &mut current_end,
        &mut has_active,
    );
    words
}

#[derive(Deserialize)]
struct TtmlSearchResponse {
    #[serde(default)]
    results: Vec<TtmlSearchCandidate>,
}

#[derive(Deserialize)]
struct TtmlSearchCandidate {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    timing_type: String,
    #[serde(rename = "lyricsUrl")]
    lyrics_url: Option<String>,
}

/// Fetches word-synced and dual-voice TTML lyrics from the public TTML lyrics catalogue.
pub async fn fetch_ttml_synced(
    http: &Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    if title.trim().is_empty() || artist.trim().is_empty() {
        return None;
    }
    let album_opt = (!album.trim().is_empty()).then_some(album.trim());
    let mut results = query_ttml_catalogue(http, title, artist, album_opt, duration_secs).await;
    if results.is_empty() && album_opt.is_some() {
        results = query_ttml_catalogue(http, title, artist, None, duration_secs).await;
    }
    if results.is_empty() {
        return None;
    }

    results.sort_by_key(|cand| {
        let timing_rank = u8::from(cand.timing_type != "word" && cand.timing_type != "syllable");
        let dur_diff = match (duration_secs, (cand.duration > 0).then_some(cand.duration)) {
            (Some(target), Some(cd)) => target.abs_diff(cd),
            (Some(_), None) => 50,
            _ => 0,
        };
        (timing_rank, dur_diff)
    });

    for cand in results {
        let cand_dur = (cand.duration > 0).then_some(cand.duration);
        if !lyrics_candidate_matches(
            title,
            artist,
            duration_secs,
            &cand.track_name,
            &cand.artist_name,
            cand_dur,
        ) {
            continue;
        }
        let Some(url) = cand.lyrics_url.as_deref() else {
            continue;
        };
        if !url.starts_with("https://") {
            continue;
        }
        let Ok(resp) = http
            .get(url)
            .header("User-Agent", SONATA_LYRICS_UA)
            .timeout(Duration::from_millis(3500))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(xml) = resp.text().await else {
            continue;
        };
        if let Some(lines) = ttml::parse_ttml(&xml) {
            if !lines.is_empty() {
                return Some(lines);
            }
        }
    }
    None
}

async fn query_ttml_catalogue(
    http: &Client,
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_secs: Option<u64>,
) -> Vec<TtmlSearchCandidate> {
    let dur_str = duration_secs.map(|d| d.to_string());
    let mut params = vec![("track", title), ("artist", artist)];
    if let Some(album) = album {
        params.push(("album", album));
    }
    if let Some(dur) = dur_str.as_deref() {
        params.push(("duration", dur));
    }
    let Ok(resp) = http
        .get("https://lyrics-api.binimum.org/")
        .query(&params)
        .header("User-Agent", SONATA_LYRICS_UA)
        .timeout(Duration::from_millis(3500))
        .send()
        .await
    else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    resp.json::<TtmlSearchResponse>()
        .await
        .map(|payload| payload.results)
        .unwrap_or_default()
}

/// Fetches time-synced KRC (word-level) or LRC lyrics from KuGou's public lyrics API.
pub async fn fetch_kugou_synced(
    http: &Client,
    title: &str,
    artist: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    let keyword = if artist.is_empty() || artist == "Unknown Artist" {
        title.to_string()
    } else {
        format!("{title} - {artist}")
    };

    let song_resp = http
        .get("https://mobileservice.kugou.com/api/v3/search/song")
        .query(&[
            ("version", "9108"),
            ("plat", "0"),
            ("pagesize", "6"),
            ("showtype", "0"),
            ("keyword", keyword.as_str()),
        ])
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json::<Value>()
        .await
        .ok()?;

    let mut hashes: Vec<(u64, String)> = song_resp
        .pointer("/data/info")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let hash = item.get("hash").and_then(|v| v.as_str())?.to_string();
                    let dur = item.get("duration").and_then(|v| v.as_u64()).unwrap_or(0);
                    if let Some(target) = duration_secs {
                        if target.abs_diff(dur) > 4 {
                            return None;
                        }
                    }
                    Some((duration_secs.map_or(0, |t| t.abs_diff(dur)), hash))
                })
                .collect()
        })
        .unwrap_or_default();
    hashes.sort_by_key(|(diff, _)| *diff);

    let mut cand_opt: Option<(String, String)> = None;
    for (_, hash) in hashes.into_iter().take(3) {
        if let Some(c) =
            search_kugou_candidate(http, Some(&hash), None, title, artist, duration_secs).await
        {
            cand_opt = Some(c);
            break;
        }
    }
    if cand_opt.is_none() {
        cand_opt =
            search_kugou_candidate(http, None, Some(&keyword), title, artist, duration_secs).await;
    }

    let (id, accesskey) = cand_opt?;
    if let Some(krc_lines) = download_kugou_krc(http, &id, &accesskey).await {
        return Some(krc_lines);
    }

    let dl: Value = http
        .get("https://lyrics.kugou.com/download")
        .query(&[
            ("fmt", "lrc"),
            ("charset", "utf8"),
            ("client", "pc"),
            ("ver", "1"),
            ("id", id.as_str()),
            ("accesskey", accesskey.as_str()),
        ])
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let b64 = dl.get("content").and_then(|v| v.as_str())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    let lrc_text = String::from_utf8(bytes).ok()?;
    let parsed = parse_lrc(&lrc_text);
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

async fn download_kugou_krc(http: &Client, id: &str, accesskey: &str) -> Option<Vec<LyricLine>> {
    let dl: Value = http
        .get("https://lyrics.kugou.com/download")
        .query(&[
            ("fmt", "krc"),
            ("charset", "utf8"),
            ("client", "pc"),
            ("ver", "1"),
            ("id", id),
            ("accesskey", accesskey),
        ])
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let b64 = dl.get("content").and_then(|v| v.as_str())?;
    let decoded = decode_krc(b64.trim())?;
    let parsed = parse_krc(&decoded);
    (!parsed.is_empty()).then_some(parsed)
}

/// Decodes base64-encoded KuGou KRC (`krc1` header + 16-byte XOR cipher + zlib stream).
pub fn decode_krc(content_b64: &str) -> Option<String> {
    let packed = base64::engine::general_purpose::STANDARD
        .decode(content_b64.trim())
        .ok()?;
    let body = packed.strip_prefix(b"krc1").or_else(|| packed.get(4..))?;
    let unmasked: Vec<u8> = body
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KRC_CIPHER[i % KRC_CIPHER.len()])
        .collect();
    let mut text = String::new();
    ZlibDecoder::new(unmasked.as_slice())
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

fn is_wide_token(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ac00}'..='\u{d7af}'
        )
    })
}

fn push_timed_token(
    words: &mut Vec<LyricWord>,
    start_secs: f64,
    end_secs: Option<f64>,
    raw_token: &str,
    separated_from_prev: &mut bool,
) {
    let starts_ws = raw_token.starts_with(char::is_whitespace);
    let ends_ws = raw_token.ends_with(char::is_whitespace);
    let trimmed = raw_token.trim();
    if trimmed.is_empty() {
        if !raw_token.is_empty() {
            *separated_from_prev = true;
        }
        return;
    }
    let join_prev = !*separated_from_prev && !starts_ws && !is_wide_token(trimmed);
    if join_prev {
        if let Some(prev) = words.last_mut() {
            prev.text.push_str(trimmed);
            if let Some(end) = end_secs {
                prev.end_secs = Some(prev.end_secs.map_or(end, |e| e.max(end)));
            }
            *separated_from_prev = ends_ws;
            return;
        }
    }
    words.push(LyricWord {
        start_secs,
        end_secs,
        text: trimmed.to_string(),
    });
    *separated_from_prev = ends_ws;
}

/// Parses decrypted KuGou KRC (`[line_start_ms,line_dur_ms]<word_offset_ms,word_dur_ms,0>word`) text.
pub fn parse_krc(krc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in krc.lines() {
        let trimmed = raw.trim();
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some((header, body)) = rest.split_once(']') else {
            continue;
        };
        let Some((start_str, _dur_str)) = header.split_once(',') else {
            continue;
        };
        let Ok(line_start_ms) = start_str.trim().parse::<u64>() else {
            continue;
        };
        let line_start_secs = line_start_ms as f64 / 1000.0;

        let mut words: Vec<LyricWord> = Vec::new();
        let mut plain = String::new();
        let mut cursor = body;
        let mut separated = true;

        while let Some(open) = cursor.find('<') {
            let prefix = &cursor[..open];
            if !prefix.is_empty() {
                plain.push_str(prefix);
                if prefix.ends_with(char::is_whitespace) {
                    separated = true;
                }
            }
            let tail = &cursor[open + 1..];
            let Some(shut) = tail.find('>') else {
                break;
            };
            let stamp_str = &tail[..shut];
            cursor = &tail[shut + 1..];
            let token_end = cursor.find('<').unwrap_or(cursor.len());
            let token = &cursor[..token_end];
            cursor = &cursor[token_end..];
            plain.push_str(token);

            let mut parts = stamp_str.split(',');
            if let (Some(off), Some(dur)) = (
                parts.next().and_then(|s| s.trim().parse::<u64>().ok()),
                parts.next().and_then(|s| s.trim().parse::<u64>().ok()),
            ) {
                let start_ms = line_start_ms.saturating_add(off);
                let w_start = start_ms as f64 / 1000.0;
                let w_end = start_ms.saturating_add(dur) as f64 / 1000.0;
                push_timed_token(&mut words, w_start, Some(w_end), token, &mut separated);
            }
        }
        if !cursor.is_empty() {
            plain.push_str(cursor);
        }

        let clean_text = if words.is_empty() {
            plain.split_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };
        if clean_text.is_empty() {
            continue;
        }
        lines.push(LyricLine {
            time_secs: line_start_secs,
            text: clean_text,
            words,
            voice: LyricVoice::Lead,
        });
    }
    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    lines
}

/// Fetches word-synced YRC (or LRC fallback) lyrics from NetEase Cloud Music.
pub async fn fetch_netease_synced(
    http: &Client,
    title: &str,
    artist: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    let query = if artist.trim().is_empty() || artist == "Unknown Artist" {
        title.trim().to_string()
    } else {
        format!("{} {}", title.trim(), artist.trim())
    };
    if query.is_empty() {
        return None;
    }

    let search: Value = http
        .get("https://music.163.com/api/search/get")
        .query(&[("s", query.as_str()), ("type", "1"), ("limit", "5")])
        .header("User-Agent", SONATA_LYRICS_UA)
        .header("Referer", "https://music.163.com")
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let songs = search.pointer("/result/songs")?.as_array()?;
    let mut matched_ids: Vec<(u64, u64)> = songs
        .iter()
        .filter_map(|song| {
            let id = song.get("id")?.as_u64()?;
            let name = song.get("name")?.as_str()?;
            let cand_artist = song
                .get("artists")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let dur_s = song
                .get("duration")
                .and_then(|v| v.as_u64())
                .map(|ms| ms / 1000)
                .filter(|&s| s > 0);
            if !lyrics_candidate_matches(title, artist, duration_secs, name, &cand_artist, dur_s) {
                return None;
            }
            let diff = match (duration_secs, dur_s) {
                (Some(a), Some(b)) => a.abs_diff(b),
                _ => 0,
            };
            Some((diff, id))
        })
        .collect();
    matched_ids.sort_by_key(|(diff, _)| *diff);

    for (_, id) in matched_ids.into_iter().take(3) {
        let id_str = id.to_string();
        let Ok(resp) = http
            .get("https://music.163.com/api/song/lyric/v1")
            .query(&[
                ("id", id_str.as_str()),
                ("cp", "false"),
                ("lv", "0"),
                ("tv", "0"),
                ("rv", "0"),
                ("kv", "0"),
                ("yv", "0"),
                ("ytv", "0"),
                ("yrv", "0"),
            ])
            .header("User-Agent", SONATA_LYRICS_UA)
            .header("Referer", "https://music.163.com")
            .timeout(Duration::from_secs(4))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(sheet) = resp.json::<Value>().await else {
            continue;
        };
        if let Some(yrc) = sheet.pointer("/yrc/lyric").and_then(|v| v.as_str()) {
            let parsed = parse_netease_yrc(yrc);
            if !parsed.is_empty() {
                return Some(parsed);
            }
        }
        if let Some(lrc_raw) = sheet.pointer("/lrc/lyric").and_then(|v| v.as_str()) {
            let parsed = parse_lrc(lrc_raw);
            if !parsed.is_empty() {
                return Some(parsed);
            }
        }
    }
    None
}

/// Parses NetEase YRC (`[start_ms,dur_ms](word_start_ms,word_dur_ms,0)word`) text.
pub fn parse_netease_yrc(yrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in yrc.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('{') {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some((header, body)) = rest.split_once(']') else {
            continue;
        };
        let Some((start_str, _dur_str)) = header.split_once(',') else {
            continue;
        };
        let Ok(line_start_ms) = start_str.trim().parse::<u64>() else {
            continue;
        };
        let line_start_secs = line_start_ms as f64 / 1000.0;

        let mut words: Vec<LyricWord> = Vec::new();
        let mut plain = String::new();
        let mut cursor = body;
        let mut separated = true;

        while let Some(open) = cursor.find('(') {
            let prefix = &cursor[..open];
            if !prefix.is_empty() {
                plain.push_str(prefix);
                if !words.is_empty() && !prefix.trim().is_empty() {
                    let fallback_start = words
                        .last()
                        .and_then(|w| w.end_secs)
                        .unwrap_or(line_start_secs);
                    push_timed_token(&mut words, fallback_start, None, prefix, &mut separated);
                } else if prefix.ends_with(char::is_whitespace) {
                    separated = true;
                }
            }
            let tail = &cursor[open + 1..];
            let Some(shut) = tail.find(')') else {
                break;
            };
            let stamp_str = &tail[..shut];
            let after_paren = &tail[shut + 1..];

            let mut parts = stamp_str.split(',');
            let parsed_stamp = match (
                parts.next().and_then(|s| s.trim().parse::<u64>().ok()),
                parts.next().and_then(|s| s.trim().parse::<u64>().ok()),
                parts.next(),
            ) {
                (Some(ws), Some(wd), Some(_)) => Some((ws, wd)),
                _ => None,
            };

            if let Some((w_start_ms, w_dur_ms)) = parsed_stamp {
                let token_end = after_paren.find('(').unwrap_or(after_paren.len());
                let token = &after_paren[..token_end];
                cursor = &after_paren[token_end..];
                plain.push_str(token);
                let w_start = w_start_ms as f64 / 1000.0;
                let w_end = w_start_ms.saturating_add(w_dur_ms) as f64 / 1000.0;
                push_timed_token(&mut words, w_start, Some(w_end), token, &mut separated);
            } else {
                // Non-timestamp parenthetical text stays part of the lyrics.
                let paren_text = &cursor[open..open + shut + 2];
                plain.push_str(paren_text);
                if !words.is_empty() {
                    let fallback_start = words
                        .last()
                        .and_then(|w| w.end_secs)
                        .unwrap_or(line_start_secs);
                    push_timed_token(&mut words, fallback_start, None, paren_text, &mut separated);
                }
                cursor = after_paren;
            }
        }
        if !cursor.is_empty() {
            plain.push_str(cursor);
            if !words.is_empty() && !cursor.trim().is_empty() {
                let fallback_start = words
                    .last()
                    .and_then(|w| w.end_secs)
                    .unwrap_or(line_start_secs);
                push_timed_token(&mut words, fallback_start, None, cursor, &mut separated);
            }
        }

        let clean_text = plain.split_whitespace().collect::<Vec<_>>().join(" ");
        if clean_text.is_empty() {
            continue;
        }
        lines.push(LyricLine {
            time_secs: line_start_secs,
            text: clean_text,
            words,
            voice: LyricVoice::Lead,
        });
    }
    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    lines
}

/// Fetches synced LRC lyrics from QQ Music's public search and lyric endpoints.
pub async fn fetch_qqmusic_synced(
    http: &Client,
    title: &str,
    artist: &str,
    duration_secs: Option<u64>,
) -> Option<Vec<LyricLine>> {
    let query = if artist.trim().is_empty() || artist == "Unknown Artist" {
        title.trim().to_string()
    } else {
        format!("{} {}", title.trim(), artist.trim())
    };
    if query.is_empty() {
        return None;
    }

    let search: Value = http
        .get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
        .query(&[("w", query.as_str()), ("format", "json"), ("n", "5")])
        .header("User-Agent", SONATA_LYRICS_UA)
        .header("Referer", "https://y.qq.com/")
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let list = search.pointer("/data/song/list")?.as_array()?;
    let mut mids: Vec<(u64, String)> = list
        .iter()
        .filter_map(|song| {
            let songmid = song.get("songmid")?.as_str()?.trim();
            if songmid.is_empty() {
                return None;
            }
            let cand_title = song.get("songname").and_then(|v| v.as_str()).unwrap_or("");
            let cand_artist = song
                .get("singer")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let cand_dur = song
                .get("interval")
                .and_then(|v| v.as_u64())
                .filter(|&d| d > 0);
            if !lyrics_candidate_matches(
                title,
                artist,
                duration_secs,
                cand_title,
                &cand_artist,
                cand_dur,
            ) {
                return None;
            }
            let diff = match (duration_secs, cand_dur) {
                (Some(a), Some(b)) => a.abs_diff(b),
                _ => 0,
            };
            Some((diff, songmid.to_string()))
        })
        .collect();
    mids.sort_by_key(|(diff, _)| *diff);

    for (_, songmid) in mids.into_iter().take(3) {
        let Ok(resp) = http
            .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
            .query(&[
                ("songmid", songmid.as_str()),
                ("format", "json"),
                ("nobase64", "0"),
            ])
            .header("User-Agent", SONATA_LYRICS_UA)
            .header("Referer", "https://y.qq.com/")
            .timeout(Duration::from_secs(4))
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(payload) = resp.json::<Value>().await else {
            continue;
        };
        if let Some(b64) = payload.get("lyric").and_then(|v| v.as_str()) {
            let parsed = decode_qqmusic_lyric(b64);
            if !parsed.is_empty() {
                return Some(parsed);
            }
        }
    }
    None
}

/// Decodes a base64-encoded QQ Music LRC payload into `LyricLine`s.
pub fn decode_qqmusic_lyric(b64: &str) -> Vec<LyricLine> {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) else {
        return Vec::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Vec::new();
    };
    parse_lrc(&text)
}

async fn search_kugou_candidate(
    http: &Client,
    hash: Option<&str>,
    keyword: Option<&str>,
    req_title: &str,
    req_artist: &str,
    duration_secs: Option<u64>,
) -> Option<(String, String)> {
    let mut req = http.get("https://lyrics.kugou.com/search").query(&[
        ("ver", "1"),
        ("man", "yes"),
        ("client", "pc"),
    ]);
    if let Some(h) = hash {
        req = req.query(&[("hash", h)]);
    } else if let Some(kw) = keyword {
        req = req.query(&[("keyword", kw)]);
        if let Some(dur) = duration_secs {
            req = req.query(&[("duration", (dur * 1000).to_string().as_str())]);
        }
    }
    let val: Value = req
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let candidates = val.get("candidates")?.as_array()?;
    pick_matching_kugou_candidate(candidates, req_title, req_artist, duration_secs)
}

fn pick_matching_kugou_candidate(
    candidates: &[Value],
    req_title: &str,
    req_artist: &str,
    duration_secs: Option<u64>,
) -> Option<(String, String)> {
    let mut best: Option<(u64, String, String)> = None;
    for item in candidates {
        let song = item.get("song").and_then(|v| v.as_str()).unwrap_or("");
        let singer = item.get("singer").and_then(|v| v.as_str()).unwrap_or("");
        let cand_dur_secs = item
            .get("duration")
            .and_then(|v| v.as_u64())
            .map(|ms| ms / 1000)
            .filter(|&s| s > 0);
        if !lyrics_candidate_matches(
            req_title,
            req_artist,
            duration_secs,
            song,
            singer,
            cand_dur_secs,
        ) {
            continue;
        }
        let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(key) = item.get("accesskey").and_then(|v| v.as_str()) else {
            continue;
        };
        let diff = match (duration_secs, cand_dur_secs) {
            (Some(t), Some(c)) => t.abs_diff(c),
            (Some(_), None) => 50,
            _ => 0,
        };
        if best
            .as_ref()
            .is_none_or(|(best_diff, _, _)| diff < *best_diff)
        {
            best = Some((diff, id.to_string(), key.to_string()));
        }
    }
    best.map(|(_, id, key)| (id, key))
}

/// Scrape plain lyrics from Genius after validating title and artist matches.
pub async fn fetch_genius_lyrics(http: &Client, title: &str, artist: &str) -> Option<Vec<String>> {
    let query = if artist.is_empty() || artist == "Unknown Artist" {
        title.to_string()
    } else {
        format!("{artist} {title}")
    };

    let search: Value = http
        .get("https://genius.com/api/search/multi")
        .query(&[("q", query.as_str())])
        .header("User-Agent", "Sonata/0.1 (lyrics lookup)")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let sections = search.pointer("/response/sections")?.as_array()?;
    let mut song_url: Option<String> = None;
    for sec in sections {
        let is_song = sec.get("type").and_then(|v| v.as_str()) == Some("song");
        if !is_song {
            continue;
        }
        if let Some(hits) = sec.get("hits").and_then(|v| v.as_array()) {
            for hit in hits {
                let Some(res) = hit.get("result") else {
                    continue;
                };
                let hit_title = res.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let hit_artist = res
                    .get("artist_names")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if lyrics_candidate_matches(title, artist, None, hit_title, hit_artist, None) {
                    song_url = res.get("url").and_then(|v| v.as_str()).map(str::to_string);
                    if song_url.is_some() {
                        break;
                    }
                }
            }
        }
    }

    let url = song_url?;
    let html = http
        .get(&url)
        .header("User-Agent", "Sonata/0.1 (lyrics lookup)")
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    extract_genius_lyrics_from_html(&html)
}

pub fn extract_genius_lyrics_from_html(html: &str) -> Option<Vec<String>> {
    static RE_HEADER: OnceLock<Regex> = OnceLock::new();
    static RE_BR: OnceLock<Regex> = OnceLock::new();
    static RE_TAG: OnceLock<Regex> = OnceLock::new();
    static RE_EMBED: OnceLock<Regex> = OnceLock::new();
    static RE_LIKE: OnceLock<Regex> = OnceLock::new();

    let re_header = RE_HEADER.get_or_init(|| {
        Regex::new(
            r#"(?is)<div[^>]*class="[^"]*LyricsHeader__Container[^"]*"[^>]*>.*?</div>\s*</div>"#,
        )
        .unwrap()
    });
    let re_br = RE_BR.get_or_init(|| Regex::new(r"(?i)<br\s*/?>|</p>").unwrap());
    let re_tag = RE_TAG.get_or_init(|| Regex::new(r"(?s)<[^>]+>").unwrap());
    let re_embed = RE_EMBED.get_or_init(|| Regex::new(r"(?i)\d*Embed\s*$").unwrap());
    let re_like = RE_LIKE.get_or_init(|| Regex::new(r"(?i)\d*You might also like").unwrap());

    let marker = "data-lyrics-container=\"true\"";
    let mut chunks = Vec::new();
    let mut cursor = 0;

    while let Some(pos) = html[cursor..].find(marker) {
        let abs = cursor + pos;
        let Some(tag_end) = html[abs..].find('>') else {
            break;
        };
        let content_start = abs + tag_end + 1;
        let mut depth = 1usize;
        let mut i = content_start;
        let bytes = html.as_bytes();

        while i < bytes.len() && depth > 0 {
            if html[i..].starts_with("<div") {
                depth += 1;
                i += 4;
            } else if html[i..].starts_with("</div>") {
                depth -= 1;
                if depth == 0 {
                    chunks.push(&html[content_start..i]);
                    cursor = i + 6;
                    break;
                }
                i += 6;
            } else {
                i += html[i..].chars().next().map_or(1, char::len_utf8);
            }
        }
        if depth != 0 {
            break;
        }
    }

    if chunks.is_empty() {
        return None;
    }

    let combined = chunks.join("\n");
    let no_header = re_header.replace_all(&combined, "");
    let with_newlines = re_br.replace_all(&no_header, "\n");
    let stripped = re_tag.replace_all(&with_newlines, "");
    let decoded = decode_html_entities(&stripped);
    let no_like = re_like.replace_all(&decoded, "");

    let mut lines: Vec<String> = Vec::new();
    for raw in no_like.lines() {
        let line = re_embed.replace_all(raw.trim(), "").trim().to_string();
        if line.ends_with("Contributors")
            || line.contains("ContributorsTranslations")
            || line.starts_with("Translations")
        {
            continue;
        }
        if line.is_empty() {
            if lines.last().is_some_and(|l| !l.is_empty()) {
                lines.push(String::new());
            }
        } else {
            lines.push(line);
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace('\u{00a0}', " ")
        .replace(['\u{200b}', '\u{feff}'], "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_genius_lyrics_html() {
        let html = r#"
            <html><body>
            <div data-lyrics-container="true">
                <div class="LyricsHeader__Container"><div><span>3 Contributors</span></div></div>
                [Verse 1]<br/>Line one &amp; two<br/>Line three&#x27;s test
            </div>
            <div data-lyrics-container="true">
                [Chorus]<br/>Chorus line12Embed
            </div>
            </body></html>
        "#;
        let lines = extract_genius_lyrics_from_html(html).expect("valid genius html");
        assert_eq!(
            lines,
            vec![
                "[Verse 1]",
                "Line one & two",
                "Line three's test",
                "",
                "[Chorus]",
                "Chorus line",
            ]
        );
    }

    #[test]
    fn rejects_missing_and_unclosed_genius_lyrics_containers() {
        assert_eq!(extract_genius_lyrics_from_html("<html></html>"), None);
        assert_eq!(
            extract_genius_lyrics_from_html(r#"<div data-lyrics-container="true"><div>unfinished"#),
            None
        );
    }

    #[test]
    fn rejects_mismatched_kugou_candidates() {
        use serde_json::json;
        let candidates = vec![
            json!({
                "id": "1",
                "accesskey": "k1",
                "song": "Totally Different Song",
                "singer": "Someone Else",
                "duration": 166000
            }),
            json!({
                "id": "2",
                "accesskey": "k2",
                "song": "Black",
                "singer": "GAZIROVKA",
                "duration": 166000
            }),
        ];
        assert_eq!(
            pick_matching_kugou_candidate(&candidates, "Black", "GAZIROVKA", Some(166)),
            Some(("2".to_string(), "k2".to_string()))
        );
        assert_eq!(
            pick_matching_kugou_candidate(&candidates[..1], "Black", "GAZIROVKA", Some(166)),
            None
        );
    }

    #[test]
    fn decodes_and_parses_kugou_krc_word_timings() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let raw_krc = "[ti:Demo]\n[1000,2000]<0,400,0>Hel<400,400,0>lo <800,600,0>world\n";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw_krc.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut payload = b"krc1".to_vec();
        payload.extend(
            compressed
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ KRC_CIPHER[i % KRC_CIPHER.len()]),
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload);

        let decoded = decode_krc(&encoded).expect("valid krc payload");
        let lines = parse_krc(&decoded);
        assert_eq!(lines.len(), 1);
        assert!((lines[0].time_secs - 1.0).abs() < 0.001);
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[0].text, "Hello");
        assert!((lines[0].words[0].start_secs - 1.0).abs() < 0.001);
        assert!((lines[0].words[0].end_secs.unwrap() - 1.8).abs() < 0.001);
        assert_eq!(lines[0].words[1].text, "world");
        assert!((lines[0].words[1].start_secs - 1.8).abs() < 0.001);
        assert!((lines[0].words[1].end_secs.unwrap() - 2.4).abs() < 0.001);
    }

    #[test]
    fn parses_netease_yrc_word_timings() {
        let sample = "{\"c\":[{\"tx\":\"作词: Test\"}]}\n\
                      [12000,3200](12000,500,0)Never (12500,700,0)gonna (13200,900,0)stop\n";
        let lines = parse_netease_yrc(sample);
        assert_eq!(lines.len(), 1);
        assert!((lines[0].time_secs - 12.0).abs() < 0.001);
        assert_eq!(lines[0].text, "Never gonna stop");
        assert_eq!(lines[0].words.len(), 3);
        assert_eq!(lines[0].words[0].text, "Never");
        assert!((lines[0].words[0].start_secs - 12.0).abs() < 0.001);
        assert!((lines[0].words[0].end_secs.unwrap() - 12.5).abs() < 0.001);
        assert_eq!(lines[0].words[2].text, "stop");
        assert!((lines[0].words[2].start_secs - 13.2).abs() < 0.001);
        assert!((lines[0].words[2].end_secs.unwrap() - 14.1).abs() < 0.001);
    }

    #[test]
    fn decodes_base64_qqmusic_lrc_payload() {
        let lrc = "[00:05.00]First QQ line\n[00:10.50]Second QQ line\n";
        let encoded = base64::engine::general_purpose::STANDARD.encode(lrc.as_bytes());
        let parsed = decode_qqmusic_lyric(&encoded);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "First QQ line");
        assert!((parsed[1].time_secs - 10.5).abs() < 0.001);
    }

    #[test]
    fn preserves_non_timestamp_parentheticals_and_saturates_yrc_and_krc_timestamps() {
        let yrc = "[1000,3000](1000,500,0)Hello (yeah) (1500,500,0)world\n\
                   [4000,2000](Ad-lib) (4000,500,0)Start\n\
                   [5000,1000](18446744073709551610,100,0)Max\n";
        let yrc_lines = parse_netease_yrc(yrc);
        assert_eq!(yrc_lines.len(), 3);
        assert_eq!(yrc_lines[0].text, "Hello (yeah) world");
        assert_eq!(
            yrc_lines[0]
                .words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Hello", "(yeah)", "world"]
        );
        assert_eq!(yrc_lines[1].text, "(Ad-lib) Start");
        assert_eq!(yrc_lines[2].text, "Max");

        let krc = "[18446744073709551610,100]<10,20,0>Safe\n";
        let krc_lines = parse_krc(krc);
        assert_eq!(krc_lines.len(), 1);
        assert_eq!(krc_lines[0].text, "Safe");
    }

    #[test]
    fn resets_current_end_in_merge_lyricsplus_syllables_and_saturates_duration() {
        let syllables = vec![
            LyricsPlusSyllable {
                time: Some(4000),
                duration: Some(1000),
                text: Some("First ".to_string()),
            },
            LyricsPlusSyllable {
                time: Some(2000),
                duration: Some(500),
                text: Some("Second ".to_string()),
            },
            LyricsPlusSyllable {
                time: Some(u64::MAX - 10),
                duration: Some(500),
                text: Some("Max".to_string()),
            },
        ];
        let words = merge_lyricsplus_syllables(&syllables);
        assert_eq!(words.len(), 3);
        assert!((words[0].end_secs.unwrap() - 5.0).abs() < 0.001);
        assert!((words[1].end_secs.unwrap() - 2.5).abs() < 0.001);
        assert_eq!(words[2].text, "Max");
    }
}
