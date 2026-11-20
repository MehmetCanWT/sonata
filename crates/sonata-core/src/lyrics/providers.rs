//! External lyrics provider adapters for Sonata (TTML, KuGou KRC/LRC, NetEase YRC/LRC, and Genius).
//!
//! 1. **TTML Catalogue**: `fetch_ttml_synced` queries `lyrics-api.binimum.org` for word-timed and
//!    dual-voice (`Lead`/`Counter`) TTML sheets.
//! 2. **KuGou KRC & LRC**: `fetch_kugou_synced` tries XOR+zlib encrypted `fmt=krc` word-timed lyrics
//!    first and falls back to plain `fmt=lrc`.
//! 3. **NetEase Cloud Music**: `fetch_netease_synced` queries `api/song/lyric/v1` for word-timed YRC
//!    lines and falls back to LRC.
//! 4. **Genius Scraper**: `fetch_genius_lyrics` extracts plain lyrics from Genius HTML containers.

use crate::lyrics::{lyrics_candidate_matches, parse_lrc, ttml, LyricLine, LyricVoice, LyricWord};
use base64::Engine as _;
use flate2::read::ZlibDecoder;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read as _;
use std::sync::OnceLock;
use std::time::Duration;

const KRC_CIPHER: [u8; 16] = *b"@Gaw^2tGQ61-\xce\xd2ni";
const SONATA_LYRICS_UA: &str = "Sonata/0.1.0 (lyrics lookup)";

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

    results
        .sort_by_key(|cand| u8::from(cand.timing_type != "word" && cand.timing_type != "syllable"));

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
                        if target.abs_diff(dur) > 8 {
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
                let w_start = (line_start_ms + off) as f64 / 1000.0;
                let w_end = (line_start_ms + off + dur) as f64 / 1000.0;
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
                if prefix.ends_with(char::is_whitespace) {
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
                let w_end = (w_start_ms + w_dur_ms) as f64 / 1000.0;
                push_timed_token(&mut words, w_start, Some(w_end), token, &mut separated);
            } else {
                // Non-timestamp parenthetical text stays part of the lyrics.
                plain.push_str(&cursor[open..open + shut + 2]);
                cursor = after_paren;
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
        return Some((id.to_string(), key.to_string()));
    }
    None
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
}
