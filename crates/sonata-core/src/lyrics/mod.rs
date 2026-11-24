//! Lyrics data model and LRC parsing for Sonata.
//!
//! 1. **Data Model**: `LyricLine`, `LyricWord`, `LyricVoice`, and `LyricsState` (the loading/found/missing
//!    lifecycle observed by front-ends).
//! 2. **LRC Parsing**: `parse_lrc` handles standard and enhanced (per-word) timestamps.
//! 3. **Playback Sync**: `active_line_index` maps a playback position to the current line.
//! 4. **Matching Helpers**: `sanitize_title` strips decorations such as "(Official Video)" and "feat.", and
//!    `lyrics_candidate_matches` rejects unrelated lyrics candidates.
//! 5. **Sources**: `lrclib` orchestrates the lookup chain, `ttml` parses Apple-style TTML sheets,
//!    and `providers` queries TTML/KuGou/NetEase/Genius backends.
//! 6. **Translation & Romanization**: `translate` handles batched `LyricsState`-aligned
//!    translation, portable offline script romanization, and private JSON caching.

pub mod lrclib;
pub mod providers;
pub mod translate;
pub mod ttml;

pub use translate::{
    contains_non_latin_letter, extract_lyrics_lines, romanize_lines_offline, romanize_lyrics_lines,
    romanize_lyrics_state, romanize_text, translate_lyrics_lines, translate_lyrics_state,
    RomanizedText, ScriptKind,
};

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LyricVoice {
    #[default]
    Lead,
    Counter,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricWord {
    pub start_secs: f64,
    pub end_secs: Option<f64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
    pub words: Vec<LyricWord>,
    pub voice: LyricVoice,
}

impl LyricLine {
    pub fn new(time_secs: f64, text: impl Into<String>) -> Self {
        Self {
            time_secs,
            text: text.into(),
            words: Vec::new(),
            voice: LyricVoice::Lead,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsState {
    Idle,
    Loading,
    Synced(Vec<LyricLine>),
    Plain(Vec<String>),
    NotFound,
}

fn parse_timestamp(mins: &str, secs: &str, frac: Option<&str>) -> f64 {
    let m: f64 = mins.parse().unwrap_or(0.0);
    let s: f64 = secs.parse().unwrap_or(0.0);
    let f = frac.map_or(0.0, |raw| {
        let val: f64 = raw.parse().unwrap_or(0.0);
        match raw.len() {
            1 => val / 10.0,
            2 => val / 100.0,
            _ => val / 1000.0,
        }
    });
    m * 60.0 + s + f
}

fn strip_voice_prefix(input: &str) -> (LyricVoice, &str) {
    let trimmed = input.trim_start();
    for prefix in ["v2:", "V2:", "bg:", "BG:"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return (LyricVoice::Counter, rest.trim_start());
        }
    }
    for prefix in ["v1:", "V1:"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return (LyricVoice::Lead, rest.trim_start());
        }
    }
    (LyricVoice::Lead, trimmed)
}

fn parse_enhanced_words(stripped: &str, re_word: &Regex) -> Vec<LyricWord> {
    let matches: Vec<_> = re_word.captures_iter(stripped).collect();
    if matches.is_empty() {
        return Vec::new();
    }

    let mut words = Vec::with_capacity(matches.len());
    for (idx, cap) in matches.iter().enumerate() {
        let whole = cap.get(0).unwrap();
        let start_secs = parse_timestamp(&cap[1], &cap[2], cap.get(3).map(|m| m.as_str()));
        let next_start = matches
            .get(idx + 1)
            .map(|next| next.get(0).unwrap().start());
        let slice_end = next_start.unwrap_or(stripped.len());
        let chunk = stripped[whole.end()..slice_end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if chunk.is_empty() {
            if let Some(prev) = words.last_mut() {
                let prev: &mut LyricWord = prev;
                if prev.end_secs.is_none() {
                    prev.end_secs = Some(start_secs);
                }
            }
            continue;
        }
        let end_secs = matches
            .get(idx + 1)
            .map(|next| parse_timestamp(&next[1], &next[2], next.get(3).map(|m| m.as_str())));
        words.push(LyricWord {
            start_secs,
            end_secs,
            text: chunk,
        });
    }
    words
}

/// Splits trailing parenthesized background vocals (e.g. `Lead text (ad-lib)`) from
/// line-synced LRC rows into separate nested `(...)` [`LyricLine`] entries.
pub fn split_inline_background_vocals(mut lines: Vec<LyricLine>) -> Vec<LyricLine> {
    if lines.is_empty() {
        return lines;
    }
    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    let mut out = Vec::with_capacity(lines.len() + 8);
    for idx in 0..lines.len() {
        let line = &lines[idx];
        if !line.words.is_empty() {
            out.push(line.clone());
            continue;
        }
        let trimmed = line.text.trim();
        if !trimmed.ends_with(')') {
            out.push(line.clone());
            continue;
        }
        let mut depth = 0i32;
        let mut open_idx = None;
        for (byte_idx, ch) in trimmed.char_indices().rev() {
            match ch {
                ')' => depth += 1,
                '(' => {
                    depth -= 1;
                    if depth == 0 {
                        open_idx = Some(byte_idx);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(open) = open_idx else {
            out.push(line.clone());
            continue;
        };
        if open == 0 || !trimmed[..open].ends_with(char::is_whitespace) {
            out.push(line.clone());
            continue;
        }
        let lead = trimmed[..open].trim();
        let inner = trimmed[open + 1..trimmed.len() - 1].trim();
        if lead.is_empty() || inner.is_empty() {
            out.push(line.clone());
            continue;
        }

        let next_t = lines.get(idx + 1).map(|n| n.time_secs);
        let bg_time = match next_t {
            Some(nt) if nt > line.time_secs + 0.25 => {
                let lead_chars = lead.chars().count() as f64;
                let bg_chars = inner.chars().count() as f64;
                let ratio = (lead_chars / (lead_chars + bg_chars).max(1.0)).clamp(0.45, 0.72);
                line.time_secs + (nt - line.time_secs) * ratio
            }
            _ => line.time_secs + 1.2,
        };

        out.push(LyricLine {
            time_secs: line.time_secs,
            text: lead.to_string(),
            words: Vec::new(),
            voice: line.voice,
        });
        out.push(LyricLine {
            time_secs: bg_time,
            text: format!("({inner})"),
            words: Vec::new(),
            voice: line.voice,
        });
    }
    out.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    out
}

/// Parses TTML XML, standard/Enhanced LRC (including `[offset:ms]`), or plain text lyrics.
pub fn parse_lyrics_auto(raw: &str) -> LyricsState {
    let cleaned = raw.trim().trim_start_matches('\u{FEFF}').trim();
    if cleaned.is_empty() {
        return LyricsState::NotFound;
    }
    if cleaned.starts_with('<') && cleaned.contains("<tt") {
        if let Some(lines) = ttml::parse_ttml(cleaned) {
            return LyricsState::Synced(lines);
        }
    }
    let synced = parse_lrc(cleaned);
    if !synced.is_empty() {
        return LyricsState::Synced(synced);
    }
    let plain: Vec<String> = cleaned
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    if plain.is_empty() {
        LyricsState::NotFound
    } else {
        LyricsState::Plain(plain)
    }
}

/// Parses standard LRC `[mm:ss.xx]` and Enhanced A2 LRC `<mm:ss.xx>` content.
pub fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    static RE_LINE: OnceLock<Regex> = OnceLock::new();
    static RE_WORD_TAG: OnceLock<Regex> = OnceLock::new();
    static RE_OFFSET: OnceLock<Regex> = OnceLock::new();
    let re_line =
        RE_LINE.get_or_init(|| Regex::new(r"\[(\d{1,3}):(\d{2})(?:\.(\d{1,3}))?\]").unwrap());
    let re_word =
        RE_WORD_TAG.get_or_init(|| Regex::new(r"<(\d{1,3}):(\d{2})(?:\.(\d{1,3}))?>").unwrap());
    let re_offset =
        RE_OFFSET.get_or_init(|| Regex::new(r"(?i)\[offset:\s*([+-]?\d+)\s*\]").unwrap());

    let mut offset_ms = 0.0_f64;
    let mut lines = Vec::new();
    for raw_line in raw.lines() {
        if let Some(cap) = re_offset.captures(raw_line) {
            if let Ok(parsed_offset) = cap[1].parse::<f64>() {
                offset_ms = parsed_offset;
            }
        }
        let mut timestamps = Vec::new();
        for cap in re_line.captures_iter(raw_line) {
            timestamps.push(parse_timestamp(
                &cap[1],
                &cap[2],
                cap.get(3).map(|m| m.as_str()),
            ));
        }

        if timestamps.is_empty() {
            continue;
        }

        let stripped = re_line.replace_all(raw_line, "");
        let (voice, body) = strip_voice_prefix(&stripped);
        let shift_secs = offset_ms / 1000.0;
        let words: Vec<LyricWord> = parse_enhanced_words(body, re_word)
            .into_iter()
            .map(|mut w| {
                w.start_secs = (w.start_secs - shift_secs).max(0.0);
                w.end_secs = w.end_secs.map(|e| (e - shift_secs).max(w.start_secs));
                w
            })
            .collect();
        let clean_text = re_word
            .replace_all(body, "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for ts in timestamps {
            lines.push(LyricLine {
                time_secs: (ts - shift_secs).max(0.0),
                text: clean_text.clone(),
                words: words.clone(),
                voice,
            });
        }
    }

    split_inline_background_vocals(lines)
}

fn format_lrc_stamp(secs: f64) -> String {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let mins = total_ms / 60_000;
    let rem_ms = total_ms % 60_000;
    let whole_secs = rem_ms / 1000;
    let ms = rem_ms % 1000;
    format!("{mins:02}:{whole_secs:02}.{ms:03}")
}

/// Serializes synced `LyricLine`s into Enhanced LRC (`<mm:ss.xxx>`) with `v2:` counter-voice tags.
pub fn format_lrc(lines: &[LyricLine]) -> String {
    lines
        .iter()
        .map(|line| {
            let line_stamp = format_lrc_stamp(line.time_secs);
            let voice_prefix = match line.voice {
                LyricVoice::Lead => "",
                LyricVoice::Counter => "v2: ",
            };
            if line.words.is_empty() {
                format!("[{line_stamp}] {voice_prefix}{}", line.text)
            } else {
                let mut tokens = Vec::with_capacity(line.words.len() * 2);
                for (idx, w) in line.words.iter().enumerate() {
                    let start_tag = format_lrc_stamp(w.start_secs);
                    tokens.push(format!("<{start_tag}>{}", w.text));
                    if let Some(end_secs) = w.end_secs {
                        let next_start = line.words.get(idx + 1).map(|n| n.start_secs);
                        let emit_end = match next_start {
                            Some(ns) => (end_secs - ns).abs() > 0.002,
                            None => true,
                        };
                        if emit_end {
                            tokens.push(format!("<{}>", format_lrc_stamp(end_secs)));
                        }
                    }
                }
                format!("[{line_stamp}] {voice_prefix}{}", tokens.join(" "))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_bg_line(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2
}

fn line_end_estimate(line: &LyricLine) -> f64 {
    line.words
        .last()
        .and_then(|w| w.end_secs.or(Some(w.start_secs + 0.4)))
        .unwrap_or(line.time_secs + 0.8)
}

/// Returns the index of the active lyric line for `position_secs`.
pub fn active_line_index(lines: &[LyricLine], position_secs: f64) -> Option<usize> {
    const LOOKAHEAD_SECS: f64 = 0.05;
    if lines.is_empty() {
        return None;
    }
    let lookahead = position_secs + LOOKAHEAD_SECS;
    let idx = lines.partition_point(|l| l.time_secs <= lookahead);
    if idx == 0 {
        return None;
    }
    let mut cand = idx - 1;
    // Keep an overlapping lead verse active until its sung words finish rather than jumping
    // mid-sentence onto a nested parenthetical background vocal.
    while cand > 0 && is_bg_line(&lines[cand].text) {
        let prev = &lines[cand - 1];
        if is_bg_line(&prev.text) {
            break;
        }
        let prev_end = line_end_estimate(prev);
        let cand_end = line_end_estimate(&lines[cand]);
        if lookahead < prev_end || (lookahead > cand_end + 0.25 && lookahead <= prev_end + 0.25) {
            cand -= 1;
        } else {
            break;
        }
    }
    Some(cand)
}

/// Returns `true` when synced lyric timestamps fit comfortably within `duration_secs`.
pub fn synced_lyrics_match_duration(lines: &[LyricLine], duration_secs: Option<u64>) -> bool {
    let Some(dur) = duration_secs.filter(|&d| d > 0) else {
        return true;
    };
    let max_ts = lines
        .iter()
        .map(|line| {
            let words_max = line
                .words
                .iter()
                .map(|w| w.end_secs.unwrap_or(w.start_secs))
                .fold(line.time_secs, f64::max);
            line.time_secs.max(words_max)
        })
        .fold(0.0_f64, f64::max);
    max_ts <= dur as f64 + 5.0
}

/// Strips upload packaging and featured credits from YouTube titles while preserving
/// recording-distinguishing qualifiers such as `(Remix)`, `(Live)`, `(Extended Version)`,
/// `(Acoustic)`, `(Slowed)`, and `(Sped Up)`.
pub fn sanitize_title(title: &str) -> String {
    static RE_CREDITS_BRACKET: OnceLock<Regex> = OnceLock::new();
    static RE_CREDITS_TAIL: OnceLock<Regex> = OnceLock::new();
    static RE_PACKAGING: OnceLock<Regex> = OnceLock::new();
    static RE_OFFICIAL: OnceLock<Regex> = OnceLock::new();

    let re_credits_bracket = RE_CREDITS_BRACKET.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[]\s*(?:feat|ft|featuring|with)\b[^\)\]]*[\)\]]").unwrap()
    });
    let re_credits_tail = RE_CREDITS_TAIL
        .get_or_init(|| Regex::new(r"(?i)\s+(?:feat|ft|featuring)\.?\s+.*$").unwrap());
    let re_packaging = RE_PACKAGING.get_or_init(|| {
        Regex::new(
            r"(?i)\s*[\(\[]\s*(?:official\s*)?(?:music\s*)?(?:video|audio|visuali[sz]er|lyrics?\s*video|lyrics?|m/?v|hd|hq|4k|full\s*song|explicit|clean)\s*[\)\]]",
        )
        .unwrap()
    });
    let re_official =
        RE_OFFICIAL.get_or_init(|| Regex::new(r"(?i)\s*[\(\[]\s*official\s*[\)\]]").unwrap());

    let s1 = re_credits_bracket.replace_all(title, " ");
    let s2 = re_credits_tail.replace_all(&s1, " ");
    let s3 = re_packaging.replace_all(&s2, " ");
    let s4 = re_official.replace_all(&s3, " ");
    let cleaned = s4
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches([',', '-', '–', '—'])
        .trim()
        .to_string();
    if cleaned.is_empty() {
        title.trim().to_string()
    } else {
        cleaned
    }
}

/// Strips YouTube's auto-generated `" - Topic"` channel suffix from artist names.
pub fn sanitize_artist(artist: &str) -> String {
    let trimmed = artist.trim();
    let stripped = trimmed
        .strip_suffix(" - Topic")
        .or_else(|| trimmed.strip_suffix(" - topic"))
        .unwrap_or(trimmed)
        .trim();
    if stripped.is_empty() {
        trimmed.to_string()
    } else {
        stripped.to_string()
    }
}

pub(crate) fn norm_words(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const VERSION_WORDS: &[&str] = &[
    "live",
    "remix",
    "remixed",
    "mix",
    "acoustic",
    "unplugged",
    "instrumental",
    "karaoke",
    "cappella",
    "acapella",
    "demo",
    "edit",
    "version",
    "cover",
    "sped",
    "slowed",
    "reverb",
    "nightcore",
    "lofi",
    "orchestral",
    "extended",
];

/// Returns the core song title without bracketed qualifiers or `" - "` suffixes.
pub(crate) fn title_core(title: &str) -> String {
    static RE_BRACKETED: OnceLock<Regex> = OnceLock::new();
    let re = RE_BRACKETED.get_or_init(|| Regex::new(r"[\(\[][^\)\]]*[\)\]]").unwrap());
    let without_brackets = re.replace_all(title, " ");
    let head = without_brackets
        .split(" - ")
        .next()
        .unwrap_or(&without_brackets);
    let core = norm_words(head);
    if core.is_empty() {
        norm_words(title)
    } else {
        core
    }
}

/// Extracts recording-distinguishing version words from bracketed qualifiers or `" - "` suffixes.
pub(crate) fn title_version_words(title: &str) -> std::collections::BTreeSet<String> {
    static RE_BRACKETED: OnceLock<Regex> = OnceLock::new();
    let re = RE_BRACKETED.get_or_init(|| Regex::new(r"[\(\[][^\)\]]*[\)\]]").unwrap());
    let mut extras = String::new();
    for m in re.find_iter(title) {
        extras.push(' ');
        extras.push_str(m.as_str());
    }
    if let Some((_, suffix)) = title.split_once(" - ") {
        extras.push(' ');
        extras.push_str(suffix);
    }
    norm_words(&extras)
        .split_whitespace()
        .filter(|w| VERSION_WORDS.contains(w))
        .map(str::to_string)
        .collect()
}

/// Splits a multi-artist credit string into normalized individual artist names.
pub(crate) fn artist_names_set(artist: &str) -> std::collections::BTreeSet<String> {
    static RE_SPLIT: OnceLock<Regex> = OnceLock::new();
    let re = RE_SPLIT.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:,|&|;|/|\s+and\s+|\s+x\s+|\s+with\s+|\s+feat\.?\s+|\s+ft\.?\s+)\s*")
            .unwrap()
    });
    let clean = sanitize_artist(artist);
    re.split(&clean)
        .map(norm_words)
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn lyrics_candidate_matches(
    req_title: &str,
    req_artist: &str,
    req_dur_secs: Option<u64>,
    cand_title: &str,
    cand_artist: &str,
    cand_dur_secs: Option<u64>,
) -> bool {
    if let (Some(a), Some(b)) = (req_dur_secs, cand_dur_secs) {
        if b > 0 && a.abs_diff(b) > 4 {
            return false;
        }
    }
    // Never cross-match distinct recording versions (e.g. Extended/Slowed/Live vs album edit).
    if title_version_words(req_title) != title_version_words(cand_title) {
        return false;
    }
    let rt = title_core(req_title);
    let ct = title_core(cand_title);
    if rt.is_empty() || ct.is_empty() {
        return false;
    }
    if rt != ct && !ct.contains(&rt) && !rt.contains(&ct) {
        return false;
    }
    let req_artists = artist_names_set(req_artist);
    let cand_artists = artist_names_set(cand_artist);
    if req_artists.is_empty()
        || req_artists.iter().all(|a| a == "unknown artist")
        || cand_artists.is_empty()
    {
        return true;
    }
    if req_artists.iter().any(|ra| cand_artists.contains(ra)) {
        return true;
    }
    let ra = norm_words(&sanitize_artist(req_artist));
    let ca = norm_words(&sanitize_artist(cand_artist));
    ra == ca
        || ca.contains(&ra)
        || ra.contains(&ca)
        || ra
            .split_whitespace()
            .any(|w| w.chars().count() >= 3 && ca.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_and_enhanced_lrc() {
        let sample = "[ar:Artist]\n[00:12.50] First line\n[00:25.100] <00:25.10> Hello <00:25.60> world\n[00:30.00] v2: Going around like a revolver\n";
        let parsed = parse_lrc(sample);
        assert_eq!(parsed.len(), 3);
        assert!((parsed[0].time_secs - 12.5).abs() < 0.001);
        assert_eq!(parsed[0].text, "First line");
        assert!(parsed[0].words.is_empty());
        assert_eq!(parsed[0].voice, LyricVoice::Lead);

        assert!((parsed[1].time_secs - 25.1).abs() < 0.001);
        assert_eq!(parsed[1].text, "Hello world");
        assert_eq!(parsed[1].words.len(), 2);
        assert_eq!(parsed[1].words[0].text, "Hello");
        assert!((parsed[1].words[0].start_secs - 25.1).abs() < 0.001);
        assert!((parsed[1].words[0].end_secs.unwrap() - 25.6).abs() < 0.001);
        assert_eq!(parsed[1].words[1].text, "world");

        assert_eq!(parsed[2].voice, LyricVoice::Counter);
        assert_eq!(parsed[2].text, "Going around like a revolver");
    }

    #[test]
    fn roundtrips_enhanced_lrc_words_and_counter_voice() {
        let lines = vec![
            LyricLine {
                time_secs: 12.345,
                text: "Hello world".to_string(),
                words: vec![
                    LyricWord {
                        start_secs: 12.345,
                        end_secs: Some(12.800),
                        text: "Hello".to_string(),
                    },
                    LyricWord {
                        start_secs: 13.100,
                        end_secs: Some(13.950),
                        text: "world".to_string(),
                    },
                ],
                voice: LyricVoice::Lead,
            },
            LyricLine {
                time_secs: 14.500,
                text: "Counter echo".to_string(),
                words: vec![
                    LyricWord {
                        start_secs: 14.500,
                        end_secs: Some(15.000),
                        text: "Counter".to_string(),
                    },
                    LyricWord {
                        start_secs: 15.000,
                        end_secs: Some(15.750),
                        text: "echo".to_string(),
                    },
                ],
                voice: LyricVoice::Counter,
            },
        ];

        let serialized = format_lrc(&lines);
        let parsed = parse_lrc(&serialized);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].voice, LyricVoice::Lead);
        assert_eq!(parsed[0].text, "Hello world");
        assert_eq!(parsed[0].words.len(), 2);
        assert!((parsed[0].words[0].start_secs - 12.345).abs() < 0.002);
        assert!((parsed[0].words[0].end_secs.unwrap() - 12.800).abs() < 0.002);
        assert!((parsed[0].words[1].start_secs - 13.100).abs() < 0.002);
        assert!((parsed[0].words[1].end_secs.unwrap() - 13.950).abs() < 0.002);

        assert_eq!(parsed[1].voice, LyricVoice::Counter);
        assert_eq!(parsed[1].text, "Counter echo");
        assert_eq!(parsed[1].words.len(), 2);
        assert!((parsed[1].words[0].start_secs - 14.500).abs() < 0.002);
        assert!((parsed[1].words[0].end_secs.unwrap() - 15.000).abs() < 0.002);
        assert!((parsed[1].words[1].start_secs - 15.000).abs() < 0.002);
        assert!((parsed[1].words[1].end_secs.unwrap() - 15.750).abs() < 0.002);
    }

    #[test]
    fn finds_active_line_by_timestamp() {
        let lines = vec![
            LyricLine::new(10.0, "A"),
            LyricLine::new(20.0, "B"),
            LyricLine::new(30.0, "C"),
        ];
        assert_eq!(active_line_index(&lines, 5.0), None);
        assert_eq!(active_line_index(&lines, 12.0), Some(0));
        assert_eq!(active_line_index(&lines, 20.0), Some(1));
        assert_eq!(active_line_index(&lines, 45.0), Some(2));
    }

    #[test]
    fn sanitizes_youtube_titles() {
        assert_eq!(
            sanitize_title("Starboy (Official Video) [Explicit]"),
            "Starboy"
        );
        assert_eq!(
            sanitize_title("Blinding Lights (Official Audio) feat. Someone"),
            "Blinding Lights"
        );
    }

    #[test]
    fn keeps_lead_verse_active_during_overlapping_background_vocal() {
        let lines = vec![
            LyricLine {
                time_secs: 10.0,
                text: "Lead verse words".into(),
                words: vec![
                    LyricWord {
                        start_secs: 10.0,
                        end_secs: Some(11.5),
                        text: "Lead".into(),
                    },
                    LyricWord {
                        start_secs: 12.0,
                        end_secs: Some(14.5),
                        text: "words".into(),
                    },
                ],
                voice: LyricVoice::Lead,
            },
            LyricLine {
                time_secs: 11.8,
                text: "(echo)".into(),
                words: vec![LyricWord {
                    start_secs: 11.8,
                    end_secs: Some(12.6),
                    text: "(echo)".into(),
                }],
                voice: LyricVoice::Lead,
            },
        ];
        assert_eq!(active_line_index(&lines, 12.1), Some(0));
        assert_eq!(active_line_index(&lines, 14.0), Some(0));
    }

    #[test]
    fn rejects_large_duration_mismatch_and_overrunning_lyrics() {
        assert!(lyrics_candidate_matches(
            "Song",
            "Artist",
            Some(200),
            "Song",
            "Artist",
            Some(204)
        ));
        assert!(!lyrics_candidate_matches(
            "Song",
            "Artist",
            Some(200),
            "Song",
            "Artist",
            Some(208)
        ));

        let within = vec![LyricLine::new(10.0, "A"), LyricLine::new(195.0, "B")];
        assert!(synced_lyrics_match_duration(&within, Some(200)));

        let overrun = vec![LyricLine::new(10.0, "A"), LyricLine::new(212.0, "B")];
        assert!(!synced_lyrics_match_duration(&overrun, Some(200)));
    }

    #[test]
    fn preserves_recording_versions_and_rejects_cross_version_matches() {
        assert_eq!(
            sanitize_title("Hands Up (Extended Version) [Official Audio]"),
            "Hands Up (Extended Version)"
        );
        assert_eq!(
            sanitize_title("Dracula (feat. JENNIE) (Official Music Video)"),
            "Dracula"
        );
        assert_eq!(sanitize_artist("The Weeknd - Topic"), "The Weeknd");

        // Must NOT match Extended Version against the standard edit.
        assert!(!lyrics_candidate_matches(
            "Hands Up (Extended Version)",
            "Vanessa Doll & V9P",
            Some(228),
            "Hands Up",
            "Vanessa Doll, V9P",
            Some(228),
        ));
        // Must match identical multi-artist album cuts across separator styles.
        assert!(lyrics_candidate_matches(
            "Tobey",
            "Eminem, Big Sean & BabyTron",
            Some(285),
            "Tobey",
            "Eminem, Big Sean, BabyTron",
            Some(285),
        ));
    }

    #[test]
    fn splits_inline_background_vocals_and_keeps_standalone_and_plurals() {
        let raw = "\
[offset:500]
[00:05.00] Fed up with life (damn, I'm goin' insane)
[00:09.00] Next verse word(s)
[00:12.00] (standalone echo)
";
        let parsed = parse_lrc(raw);
        assert_eq!(parsed.len(), 4);

        assert!((parsed[0].time_secs - 4.5).abs() < 0.01);
        assert_eq!(parsed[0].text, "Fed up with life");

        assert!(parsed[1].time_secs > 4.5 && parsed[1].time_secs < 8.5);
        assert_eq!(parsed[1].text, "(damn, I'm goin' insane)");

        assert!((parsed[2].time_secs - 8.5).abs() < 0.01);
        assert_eq!(parsed[2].text, "Next verse word(s)");

        assert!((parsed[3].time_secs - 11.5).abs() < 0.01);
        assert_eq!(parsed[3].text, "(standalone echo)");
    }

    #[test]
    fn parse_lyrics_auto_supports_ttml_lrc_and_plain() {
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata">
            <head><metadata><ttm:agent type="person" xml:id="v1"/></metadata></head>
            <body><div>
                <p begin="2.10" end="4.60" ttm:agent="v1"><span begin="2.10" end="2.60">Meet</span> <span begin="2.60" end="4.60">Eddie</span><span ttm:role="x-bg"><span begin="3.80" end="4.50">(who?)</span></span></p>
            </div></body>
        </tt>"#;
        match parse_lyrics_auto(ttml) {
            LyricsState::Synced(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0].text, "Meet Eddie");
                assert_eq!(lines[0].words.len(), 2);
                assert_eq!(lines[1].text, "(who?)");
                assert!((lines[1].time_secs - 3.8).abs() < 0.01);
            }
            other => panic!("expected Synced TTML lines, got {other:?}"),
        }

        match parse_lyrics_auto("\u{FEFF}[00:01.00] One (two)\n[00:05.00] Three") {
            LyricsState::Synced(lines) => {
                assert_eq!(lines.len(), 3);
                assert_eq!(lines[0].text, "One");
                assert_eq!(lines[1].text, "(two)");
            }
            other => panic!("expected Synced LRC lines, got {other:?}"),
        }

        match parse_lyrics_auto("First line\nSecond line") {
            LyricsState::Plain(lines) => {
                assert_eq!(lines, vec!["First line", "Second line"]);
            }
            other => panic!("expected Plain lines, got {other:?}"),
        }
    }
}
