use super::*;
use regex::Regex;
use std::sync::OnceLock;

pub fn clean_search_title(title: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[](official|lyric|music|audio|video|hd|hq|4k|visualizer|remaster)[^\)\]]*[\)\]]").unwrap()
    });
    re.replace_all(title, "").trim().to_string()
}

pub(super) fn pick_best_ytm_match<'a>(
    target: &Track,
    candidates: &'a [Track],
) -> Option<&'a Track> {
    if candidates.is_empty() {
        return None;
    }
    candidates.iter().min_by_key(|candidate| {
        let duration_difference = match (target.duration_secs, candidate.duration_secs) {
            (Some(target), Some(candidate)) => target.abs_diff(candidate),
            _ => 10,
        };
        let title_penalty =
            if normalize_cmp(&candidate.title).contains(&normalize_cmp(&target.title)) {
                0
            } else {
                30
            };
        duration_difference + title_penalty
    })
}

fn normalize_cmp(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn audio_quality_badge(
    source: ProviderSource,
    format_id: &str,
    codec: &str,
    average_kbps: Option<u32>,
) -> String {
    let format_id = format_id.split('-').next().unwrap_or(format_id);
    let codec = codec.to_ascii_uppercase();
    let quality = match source {
        ProviderSource::YouTubeMusic => match format_id {
            "774" | "141" => "Extreme",
            "251" | "328" | "380" => "HQ",
            "140" => "Standard",
            _ if average_kbps.is_some_and(|kbps| kbps <= 128) => "Standard",
            _ => "HQ",
        },
        ProviderSource::Spotify if matches!(codec.as_str(), "FLAC" | "ALAC") => "Hi-Res",
        ProviderSource::Spotify if average_kbps.is_some_and(|kbps| kbps <= 128) => "Standard",
        ProviderSource::Spotify => "HQ",
    };
    match average_kbps.filter(|kbps| *kbps > 0) {
        Some(kbps) => format!("{codec} {kbps}kbps · {quality}"),
        None => format!("{codec} · {quality}"),
    }
}

pub(super) fn extract_badge_kbps(badge: &str) -> Option<u32> {
    if let Some(kbps) = badge.split_whitespace().find_map(|token| {
        let lower = token.to_ascii_lowercase();
        lower
            .strip_suffix("kbps")
            .and_then(|digits| digits.parse::<u32>().ok())
    }) {
        return Some(kbps);
    }
    let mut prev_was_itag = false;
    for token in badge.split(|character: char| !character.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if prev_was_itag {
            prev_was_itag = false;
            continue;
        }
        if token.eq_ignore_ascii_case("itag") {
            prev_was_itag = true;
            continue;
        }
        if let Ok(value) = token.parse::<u32>() {
            if value >= 48 {
                return Some(value);
            }
        }
    }
    None
}

pub(super) fn should_upgrade_candidate(current_badge: &str, candidate_badge: &str) -> bool {
    let current_kbps = extract_badge_kbps(current_badge).unwrap_or(0);
    let candidate_kbps = extract_badge_kbps(candidate_badge).unwrap_or(0);
    quality_rank(candidate_badge) > quality_rank(current_badge)
        || candidate_kbps >= current_kbps.saturating_add(24)
}

fn quality_rank(badge: &str) -> u8 {
    let lower = badge.to_ascii_lowercase();
    if lower.contains("hi-res") || lower.contains("lossless") || lower.contains("flac") {
        3
    } else if lower.contains("extreme") {
        2
    } else if lower.contains("hq") {
        1
    } else {
        0
    }
}
