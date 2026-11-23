//! Pure YouTube Music InnerTube browse, mood, library card, and artist banner parsers.
//!
//! 1. **Types & Reports**: `MoodChip`, `MoodCategoryTile`, and `ParsedCollection<T>` report parsed
//!    entries alongside `dropped_rows` when unknown or malformed InnerTube nodes are skipped.
//! 2. **Browse Parsers**: `parse_mood_chips_json`, `parse_mood_tiles_json`, `parse_cards_tree_json`,
//!    and `parse_artist_banner_url_json` operate purely on `serde_json::Value` without network I/O.
//! 3. **Shared Node Helpers**: `parse_two_row_card`, `parse_responsive_card`, `parse_responsive_track`,
//!    `collect_runs_text`, and `extract_best_thumbnail` are shared with album and rich-search parsers.

use super::{upscale_thumbnail, HomeCard, HomeCardKind};
use crate::providers::{parse_duration_text, ProviderSource, Track};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoodChip {
    pub label: String,
    pub params: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoodCategoryTile {
    pub title: String,
    pub browse_id: String,
    pub params: String,
    pub accent_rgb: (u8, u8, u8),
}

/// Result of parsing a list of InnerTube items while tracking dropped or malformed rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCollection<T> {
    pub items: Vec<T>,
    pub dropped_rows: usize,
}

impl<T> Default for ParsedCollection<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            dropped_rows: 0,
        }
    }
}

/// Extracts `chipCloudChipRenderer` mood filter pills in document order while counting dropped rows.
pub fn parse_mood_chips_json(val: &Value) -> ParsedCollection<MoodChip> {
    let mut out = ParsedCollection::default();
    collect_chips_recursive(val, &mut out);
    out
}

fn collect_chips_recursive(val: &Value, out: &mut ParsedCollection<MoodChip>) {
    match val {
        Value::Object(map) => {
            if let Some(chip) = map.get("chipCloudChipRenderer") {
                let label = collect_runs_text(chip.get("text")).unwrap_or_default();
                let params = chip
                    .pointer("/navigationEndpoint/browseEndpoint/params")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if label.is_empty() || params.is_empty() {
                    out.dropped_rows += 1;
                } else if !out.items.iter().any(|c| c.params == params) {
                    out.items.push(MoodChip { label, params });
                }
            }
            for v in map.values() {
                collect_chips_recursive(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_chips_recursive(v, out);
            }
        }
        _ => {}
    }
}

/// Extracts `musicNavigationButtonRenderer` mood/genre tiles in document order.
pub fn parse_mood_tiles_json(val: &Value) -> ParsedCollection<MoodCategoryTile> {
    let mut out = ParsedCollection::default();
    collect_tiles_recursive(val, &mut out);
    out
}

fn collect_tiles_recursive(val: &Value, out: &mut ParsedCollection<MoodCategoryTile>) {
    match val {
        Value::Object(map) => {
            if let Some(btn) = map.get("musicNavigationButtonRenderer") {
                let title = collect_runs_text(btn.get("buttonText")).unwrap_or_default();
                let browse_id = btn
                    .pointer("/clickCommand/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let params = btn
                    .pointer("/clickCommand/browseEndpoint/params")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let stripe = btn
                    .pointer("/solid/leftStripeColor")
                    .and_then(Value::as_u64)
                    .map(|c| {
                        let r = ((c >> 16) & 0xff) as u8;
                        let g = ((c >> 8) & 0xff) as u8;
                        let b = (c & 0xff) as u8;
                        (r, g, b)
                    })
                    .unwrap_or((120, 128, 150));
                if title.is_empty() || browse_id.is_empty() {
                    out.dropped_rows += 1;
                } else if !out
                    .items
                    .iter()
                    .any(|t| t.title == title && t.params == params)
                {
                    out.items.push(MoodCategoryTile {
                        title,
                        browse_id,
                        params,
                        accent_rgb: stripe,
                    });
                }
            }
            for v in map.values() {
                collect_tiles_recursive(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_tiles_recursive(v, out);
            }
        }
        _ => {}
    }
}

/// Extracts the widest banner thumbnail (`aspect >= 1.5`) from an artist header payload.
pub fn parse_artist_banner_url_json(val: &Value) -> Option<String> {
    let header = val
        .pointer("/header/musicVisualHeaderRenderer")
        .or_else(|| val.pointer("/header/musicImmersiveHeaderRenderer"))?;
    let thumbs = header
        .pointer("/foregroundThumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| header.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"))
        .and_then(Value::as_array)?;
    let mut best_wide = None;
    let mut best_wide_w = 0u64;
    for t in thumbs {
        let w = t.get("width").and_then(Value::as_u64).unwrap_or(0);
        let h = t.get("height").and_then(Value::as_u64).unwrap_or(1);
        let url = t.get("url").and_then(Value::as_str).unwrap_or("");
        if w > h.saturating_mul(3) / 2 && w >= best_wide_w && !url.is_empty() {
            best_wide_w = w;
            best_wide = Some(upscale_wide_banner_url(url));
        }
    }
    best_wide
}

pub(crate) fn upscale_wide_banner_url(url: &str) -> String {
    if !(url.contains("googleusercontent.com") || url.contains("ggpht.com")) {
        return url.to_string();
    }
    if let Some((base, _params)) = url.split_once("=w") {
        format!("{base}=w1600-h666-p-l90-rj")
    } else {
        url.to_string()
    }
}

/// Extracts `HomeCard` entries from a library browse tree while tracking dropped rows.
pub fn parse_cards_tree_json(
    val: &Value,
    default_kind: HomeCardKind,
) -> ParsedCollection<HomeCard> {
    let mut out = ParsedCollection::default();
    collect_cards_recursive(val, default_kind, &mut out);
    out
}

fn collect_cards_recursive(
    val: &Value,
    default_kind: HomeCardKind,
    out: &mut ParsedCollection<HomeCard>,
) {
    match val {
        Value::Object(map) => {
            if let Some(two) = map.get("musicTwoRowItemRenderer") {
                match parse_two_row_card(two, default_kind) {
                    Some(card) => {
                        if !out
                            .items
                            .iter()
                            .any(|c| c.id == card.id && c.title == card.title)
                        {
                            out.items.push(card);
                        }
                    }
                    None => out.dropped_rows += 1,
                }
            } else if let Some(resp) = map.get("musicResponsiveListItemRenderer") {
                if default_kind == HomeCardKind::Artist {
                    match parse_responsive_card(resp, default_kind) {
                        Some(card) => {
                            if !out
                                .items
                                .iter()
                                .any(|c| c.id == card.id && c.title == card.title)
                            {
                                out.items.push(card);
                            }
                        }
                        None => out.dropped_rows += 1,
                    }
                }
            }
            for v in map.values() {
                collect_cards_recursive(v, default_kind, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_cards_recursive(v, default_kind, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn parse_two_row_card(two: &Value, default_kind: HomeCardKind) -> Option<HomeCard> {
    let title = collect_runs_text(two.get("title"))?;
    let subtitle = collect_runs_text(two.get("subtitle")).unwrap_or_default();
    let id = two
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let kind = classify_browse_id(&id, default_kind);
    let thumbnail_url = extract_best_thumbnail(
        two.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"),
    );
    Some(HomeCard {
        kind,
        id,
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

pub(crate) fn parse_responsive_card(resp: &Value, default_kind: HomeCardKind) -> Option<HomeCard> {
    let cols = resp.get("flexColumns").and_then(Value::as_array)?;
    let title = cols.first().and_then(|c| {
        collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
    })?;
    let subtitle = cols
        .get(1)
        .and_then(|c| {
            collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
        })
        .unwrap_or_default();
    let id = resp
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| {
            cols.first().and_then(|c| {
                c.pointer(
                    "/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
                )
            })
        })
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let kind = classify_browse_id(&id, default_kind);
    let thumbnail_url = extract_best_thumbnail(
        resp.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
    );
    Some(HomeCard {
        kind,
        id,
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

pub(crate) fn classify_browse_id(id: &str, fallback: HomeCardKind) -> HomeCardKind {
    if id.starts_with("UC") {
        HomeCardKind::Artist
    } else if id.starts_with("MPRE") {
        HomeCardKind::Album
    } else if id.starts_with("VL") || id.starts_with("PL") || id.starts_with("OLAK") {
        HomeCardKind::Playlist
    } else {
        fallback
    }
}

pub(crate) fn is_metadata_noise_token(txt: &str) -> bool {
    let lower = txt.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    if matches!(
        lower.as_str(),
        "song" | "video" | "single" | "ep" | "album" | "playlist" | "explicit" | "artist"
    ) {
        return true;
    }
    if lower.contains(" play")
        || lower.ends_with("plays")
        || lower.contains(" view")
        || lower.ends_with("views")
        || lower.contains(" listener")
        || lower.contains(" subscriber")
        || lower.contains(" track")
        || lower.contains(" song")
    {
        return true;
    }
    if lower.len() == 4 && lower.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    false
}

pub(crate) fn parse_responsive_track(
    resp: &Value,
    default_album: Option<&str>,
    default_artist: Option<&str>,
    default_cover: Option<String>,
    default_album_id: Option<&str>,
) -> Option<Track> {
    let vid = resp
        .pointer("/playlistItemData/videoId")
        .or_else(|| {
            resp.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId")
        })
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();

    let cols = resp.get("flexColumns").and_then(Value::as_array)?;
    let title = cols.first().and_then(|c| {
        collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
    })?;

    let mut artist_parts: Vec<String> = Vec::new();
    let mut album = default_album.unwrap_or("").to_string();
    let mut album_id = default_album_id.map(str::to_string);
    let mut duration_secs = None;

    for col in cols.iter().skip(1) {
        if let Some(runs) = col
            .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
            .and_then(Value::as_array)
        {
            for r in runs {
                let txt = r.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if txt.is_empty() || txt == "•" || txt == "," || txt == "&" {
                    continue;
                }
                if let Some(dur) = parse_duration_text(txt) {
                    duration_secs = Some(dur);
                    continue;
                }
                if let Some(bid) = r
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                {
                    if bid.starts_with("MPRE") {
                        album = txt.to_string();
                        album_id = Some(bid.to_string());
                    } else if !is_metadata_noise_token(txt)
                        && !artist_parts.iter().any(|a| a.eq_ignore_ascii_case(txt))
                    {
                        artist_parts.push(txt.to_string());
                    }
                } else if artist_parts.is_empty() && !is_metadata_noise_token(txt) {
                    artist_parts.push(txt.to_string());
                }
            }
        }
    }

    let artist = if !artist_parts.is_empty() {
        artist_parts.join(", ")
    } else if let Some(def) = default_artist.filter(|s| !s.trim().is_empty()) {
        def.to_string()
    } else {
        "Unknown Artist".to_string()
    };

    if let Some(fixed) = resp.get("fixedColumns").and_then(Value::as_array) {
        for fc in fixed {
            if let Some(txt) =
                collect_runs_text(fc.pointer("/musicResponsiveListItemFixedColumnRenderer/text"))
            {
                if let Some(dur) = parse_duration_text(&txt) {
                    duration_secs = Some(dur);
                }
            }
        }
    }

    let thumbnail_url = extract_best_thumbnail(
        resp.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
    )
    .or(default_cover);
    let is_explicit = resp
        .pointer("/badges/0/musicInlineBadgeRenderer/icon/iconType")
        .and_then(Value::as_str)
        .is_some_and(|t| t == "MUSIC_EXPLICIT_BADGE");

    Some(Track {
        id: vid,
        title,
        artist,
        album,
        duration_secs,
        thumbnail_url,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit,
    })
}

pub(crate) fn extract_best_thumbnail(arr: Option<&Value>) -> Option<String> {
    let arr = arr?.as_array()?;
    let url = arr
        .last()
        .and_then(|v| v.get("url"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?;
    Some(upscale_thumbnail(url))
}

pub(crate) fn collect_runs_text(val: Option<&Value>) -> Option<String> {
    let runs = val?.get("runs")?.as_array()?;
    let mut s = String::new();
    for r in runs {
        if let Some(t) = r.get("text").and_then(Value::as_str) {
            s.push_str(t);
        }
    }
    let trimmed = s.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}
