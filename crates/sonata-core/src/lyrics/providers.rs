use crate::lyrics::{lyrics_candidate_matches, parse_lrc, LyricLine};
use base64::Engine as _;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

/// Fetches time-synced LRC lyrics from KuGou's public lyrics API.
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
}
