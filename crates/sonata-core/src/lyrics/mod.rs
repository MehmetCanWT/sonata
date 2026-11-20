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

pub mod lrclib;
pub mod providers;
pub mod ttml;

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

/// Parses standard LRC `[mm:ss.xx]` and Enhanced A2 LRC `<mm:ss.xx>` content.
pub fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    static RE_LINE: OnceLock<Regex> = OnceLock::new();
    static RE_WORD_TAG: OnceLock<Regex> = OnceLock::new();
    let re_line =
        RE_LINE.get_or_init(|| Regex::new(r"\[(\d{1,3}):(\d{2})(?:\.(\d{1,3}))?\]").unwrap());
    let re_word =
        RE_WORD_TAG.get_or_init(|| Regex::new(r"<(\d{1,3}):(\d{2})(?:\.(\d{1,3}))?>").unwrap());

    let mut lines = Vec::new();
    for raw_line in raw.lines() {
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
        let words = parse_enhanced_words(body, re_word);
        let clean_text = re_word
            .replace_all(body, "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for ts in timestamps {
            lines.push(LyricLine {
                time_secs: ts,
                text: clean_text.clone(),
                words: words.clone(),
                voice,
            });
        }
    }

    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    lines
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

/// Returns the index of the active lyric line for `position_secs`.
pub fn active_line_index(lines: &[LyricLine], position_secs: f64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let idx = lines.partition_point(|l| l.time_secs <= position_secs + 0.15);
    if idx == 0 {
        None
    } else {
        Some(idx - 1)
    }
}

/// Strips YouTube/release noise from titles before querying LRCLIB.
pub fn sanitize_title(title: &str) -> String {
    static RE_BRACKETS: OnceLock<Regex> = OnceLock::new();
    static RE_FEAT: OnceLock<Regex> = OnceLock::new();
    let re_brackets = RE_BRACKETS.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[](official|lyric|music|audio|video|hd|hq|4k|visualizer|remaster|explicit|clean)[^\)\]]*[\)\]]").unwrap()
    });
    let re_feat =
        RE_FEAT.get_or_init(|| Regex::new(r"(?i)\s*[\(\[]?(feat\.|ft\.)[^\)\]]*[\)\]]?").unwrap());
    let step1 = re_brackets.replace_all(title, "");
    let step2 = re_feat.replace_all(&step1, "");
    step2.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn norm_words(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
        if b > 0 && a.abs_diff(b) > 12 {
            return false;
        }
    }
    let rt = norm_words(req_title);
    let ct = norm_words(cand_title);
    if rt.is_empty() || ct.is_empty() {
        return false;
    }
    if rt != ct && !ct.contains(&rt) && !rt.contains(&ct) {
        return false;
    }
    let ra = norm_words(req_artist);
    let ca = norm_words(cand_artist);
    if ra.is_empty() || ra == "unknown artist" || ca.is_empty() {
        return true;
    }
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
}
