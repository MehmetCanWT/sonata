pub mod lrclib;
pub mod providers;

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsState {
    Idle,
    Loading,
    Synced(Vec<LyricLine>),
    Plain(Vec<String>),
    NotFound,
}

/// Parses standard LRC `[mm:ss.xx]` and Enhanced A2 LRC `<mm:ss.xx>` content.
pub fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    static RE_LINE: OnceLock<Regex> = OnceLock::new();
    static RE_WORD_TAG: OnceLock<Regex> = OnceLock::new();
    let re_line =
        RE_LINE.get_or_init(|| Regex::new(r"\[(\d{1,3}):(\d{2})(?:\.(\d{1,3}))?\]").unwrap());
    let re_word = RE_WORD_TAG.get_or_init(|| Regex::new(r"<\d{1,3}:\d{2}(?:\.\d{1,3})?>").unwrap());

    let mut lines = Vec::new();
    for raw_line in raw.lines() {
        let mut timestamps = Vec::new();
        for cap in re_line.captures_iter(raw_line) {
            let mins: f64 = cap[1].parse().unwrap_or(0.0);
            let secs: f64 = cap[2].parse().unwrap_or(0.0);
            let frac = cap.get(3).map_or(0.0, |m| {
                let s = m.as_str();
                let val: f64 = s.parse().unwrap_or(0.0);
                match s.len() {
                    1 => val / 10.0,
                    2 => val / 100.0,
                    _ => val / 1000.0,
                }
            });
            timestamps.push(mins * 60.0 + secs + frac);
        }

        if timestamps.is_empty() {
            continue;
        }

        let stripped = re_line.replace_all(raw_line, "");
        let clean_text = re_word
            .replace_all(&stripped, "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for ts in timestamps {
            lines.push(LyricLine {
                time_secs: ts,
                text: clean_text.clone(),
            });
        }
    }

    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    lines
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
        let sample =
            "[ar:Artist]\n[00:12.50] First line\n[00:25.100] <00:25.10> Hello <00:25.60> world\n";
        let parsed = parse_lrc(sample);
        assert_eq!(parsed.len(), 2);
        assert!((parsed[0].time_secs - 12.5).abs() < 0.001);
        assert_eq!(parsed[0].text, "First line");
        assert!((parsed[1].time_secs - 25.1).abs() < 0.001);
        assert_eq!(parsed[1].text, "Hello world");
    }

    #[test]
    fn finds_active_line_by_timestamp() {
        let lines = vec![
            LyricLine {
                time_secs: 10.0,
                text: "A".into(),
            },
            LyricLine {
                time_secs: 20.0,
                text: "B".into(),
            },
            LyricLine {
                time_secs: 30.0,
                text: "C".into(),
            },
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
