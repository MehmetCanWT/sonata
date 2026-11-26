//! Pure YouTube Music InnerTube search suggestion and typed rich-search parsers.
//!
//! 1. **Suggestions**: `parse_search_suggestions_json` extracts deduplicated autocomplete suggestions
//!    from `music/get_search_suggestions` responses while counting malformed suggestion nodes.
//! 2. **Rich Search**: `parse_rich_search_json` extracts `musicCardShelfRenderer` (Top Result),
//!    `itemSectionRenderer`, and `musicShelfRenderer` items into structurally classified songs,
//!    videos, albums, artists, and playlists while preserving order and reporting `dropped_rows`.

use super::browse::{
    collect_runs_text, is_metadata_noise_token, parse_responsive_card, parse_responsive_track,
    ParsedCollection,
};
use super::{upscale_thumbnail, HomeCard, HomeCardKind};
use crate::error::CoreError;
use crate::providers::{ProviderSource, Track};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct RichSearchResult {
    pub songs: Vec<Track>,
    pub videos: Vec<Track>,
    pub albums: Vec<HomeCard>,
    pub artists: Vec<HomeCard>,
    pub playlists: Vec<HomeCard>,
    /// Original heading order of recognized search sections in the InnerTube payload.
    pub shelf_order: Vec<String>,
    /// Number of shelf rows that could not be parsed into a valid track or card.
    pub dropped_rows: usize,
}

/// Extracts up to 8 deduplicated autocomplete suggestions while counting malformed entries.
pub fn parse_search_suggestions_json(val: &Value) -> ParsedCollection<String> {
    let mut out = ParsedCollection::default();
    collect_suggestion_runs(val, &mut out);
    out.items.truncate(8);
    out
}

fn collect_suggestion_runs(val: &Value, out: &mut ParsedCollection<String>) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("searchSuggestionRenderer") {
                match collect_runs_text(renderer.get("suggestion")) {
                    Some(text) => {
                        if !out.items.contains(&text) {
                            out.items.push(text);
                        }
                    }
                    None => out.dropped_rows += 1,
                }
            }
            for v in map.values() {
                collect_suggestion_runs(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_suggestion_runs(v, out);
            }
        }
        _ => {}
    }
}

/// Parses typed YouTube Music search shelves, preserving shelf order and reporting dropped rows.
///
/// Supports modern unfiltered `musicCardShelfRenderer` (Top Result) + `itemSectionRenderer`
/// layouts as well as legacy `musicShelfRenderer` sections across all locales.
/// Returns `CoreError::InvalidResponse` when the payload is not a recognized InnerTube search
/// response object (missing `contents` and any recognized search shelf).
pub fn parse_rich_search_json(val: &Value) -> Result<RichSearchResult, CoreError> {
    if !val.is_object() {
        return Err(CoreError::invalid_response(
            "ytmusic",
            "rich search response was not a JSON object",
        ));
    }
    let mut out = RichSearchResult::default();
    let mut saw_shelf = false;
    collect_rich_search_shelves(val, &mut out, &mut saw_shelf);
    if !saw_shelf && val.get("contents").is_none() {
        return Err(CoreError::invalid_response(
            "ytmusic",
            "rich search response missing recognized `contents` or `musicShelfRenderer`",
        ));
    }
    Ok(out)
}

fn push_unique_track(list: &mut Vec<Track>, track: Track, at_front: bool) {
    if list.iter().any(|existing| existing.id == track.id) {
        return;
    }
    if at_front {
        list.insert(0, track);
    } else {
        list.push(track);
    }
}

fn push_unique_card(list: &mut Vec<HomeCard>, card: HomeCard, at_front: bool) {
    if list.iter().any(|existing| existing.id == card.id) {
        return;
    }
    if at_front {
        list.insert(0, card);
    } else {
        list.push(card);
    }
}

fn watch_video_type(node: &Value) -> Option<&str> {
    node.pointer("/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        .or_else(|| {
            node.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        })
        .or_else(|| {
            node.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        })
        .or_else(|| {
            node.pointer("/onTap/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        })
        .or_else(|| {
            node.pointer("/title/runs/0/navigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        })
        .and_then(Value::as_str)
}

fn is_video_music_type(mvt: Option<&str>) -> bool {
    matches!(
        mvt,
        Some("MUSIC_VIDEO_TYPE_OMV" | "MUSIC_VIDEO_TYPE_UGC" | "MUSIC_VIDEO_TYPE_PODCAST_EPISODE")
    )
}

fn heading_kind_hint(raw_heading: &str) -> Option<&'static str> {
    let lower = raw_heading.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    if lower.contains("video") || lower.contains("видео") {
        Some("video")
    } else if lower.contains("album") || lower.contains("albüm") || lower.contains("альбом")
    {
        Some("album")
    } else if lower.contains("artist")
        || lower.contains("sanatçı")
        || lower.contains("sanatci")
        || lower.contains("исполнител")
    {
        Some("artist")
    } else if lower.contains("playlist")
        || lower.contains("oynatma list")
        || lower.contains("плейлист")
    {
        Some("playlist")
    } else if lower.contains("song") || lower.contains("şarkı") || lower.contains("песн") {
        Some("song")
    } else {
        None
    }
}

fn extract_card_shelf_endpoint(card: &Value) -> Option<&Value> {
    card.get("onTap")
        .or_else(|| card.pointer("/title/runs/0/navigationEndpoint"))
        .or_else(|| {
            card.pointer(
                "/thumbnailOverlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint",
            )
        })
        .or_else(|| card.pointer("/buttons/0/buttonRenderer/command"))
        .or_else(|| card.pointer("/buttons/0/buttonRenderer/navigationEndpoint"))
}

fn extract_card_shelf_thumbnail(card: &Value) -> Option<String> {
    card.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .and_then(Value::as_array)
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(Value::as_str)
        .map(upscale_thumbnail)
}

fn parse_duration_secs_from_runs(runs: &[Value]) -> Option<u64> {
    for run in runs {
        if let Some(txt) = run.get("text").and_then(Value::as_str) {
            let trimmed = txt.trim();
            let parts: Vec<&str> = trimmed.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(m), Ok(s)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                    return Some(m * 60 + s);
                }
            } else if parts.len() == 3 {
                if let (Ok(h), Ok(m), Ok(s)) = (
                    parts[0].parse::<u64>(),
                    parts[1].parse::<u64>(),
                    parts[2].parse::<u64>(),
                ) {
                    return Some(h * 3600 + m * 60 + s);
                }
            }
        }
    }
    None
}

/// Parses a watchable `Track` from a `musicCardShelfRenderer` (Top Result) when it targets a song/video.
pub(crate) fn parse_card_shelf_track(card: &Value) -> Option<Track> {
    let ep = extract_card_shelf_endpoint(card)?;
    let video_id = ep
        .pointer("/watchEndpoint/videoId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let title = collect_runs_text(card.get("title"))?;
    let mut artist_parts: Vec<String> = Vec::new();
    let mut album = String::new();
    let mut album_id: Option<String> = None;
    let mut duration_secs = None;

    if let Some(runs) = card.pointer("/subtitle/runs").and_then(Value::as_array) {
        duration_secs = parse_duration_secs_from_runs(runs);
        for r in runs {
            let txt = r.get("text").and_then(Value::as_str).unwrap_or("").trim();
            if txt.is_empty() || txt == "•" || txt == "," || txt == "&" {
                continue;
            }
            if parse_duration_secs_from_runs(std::slice::from_ref(r)).is_some() {
                continue;
            }
            if let Some(bid) = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
            {
                if bid.starts_with("MPRE") || bid.starts_with("OLAK") {
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

    let is_explicit = card
        .get("subtitleBadges")
        .and_then(Value::as_array)
        .is_some_and(|badges| {
            badges.iter().any(|b| {
                b.pointer("/musicInlineBadgeRenderer/icon/iconType")
                    .and_then(Value::as_str)
                    == Some("MUSIC_EXPLICIT_BADGE")
            })
        });

    Some(Track {
        id: video_id,
        title,
        artist: if artist_parts.is_empty() {
            "Unknown Artist".to_string()
        } else {
            artist_parts.join(", ")
        },
        album,
        duration_secs,
        thumbnail_url: extract_card_shelf_thumbnail(card),
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit,
    })
}

fn ingest_card_shelf_top_result(card: &Value, out: &mut RichSearchResult) {
    if let Some(header) =
        collect_runs_text(card.pointer("/header/musicCardShelfHeaderBasicRenderer/title"))
    {
        if !header.is_empty() && !out.shelf_order.contains(&header) {
            out.shelf_order.push(header);
        }
    }
    if let Some(track) = parse_card_shelf_track(card) {
        let ep = extract_card_shelf_endpoint(card);
        let mvt = ep
            .and_then(watch_video_type)
            .or_else(|| watch_video_type(card));
        if is_video_music_type(mvt) {
            push_unique_track(&mut out.videos, track, true);
        } else {
            push_unique_track(&mut out.songs, track, true);
        }
        return;
    }

    // Browseable Top Result (Artist, Album, Playlist).
    let Some(ep) = extract_card_shelf_endpoint(card) else {
        out.dropped_rows += 1;
        return;
    };
    let Some(browse_id) = ep
        .pointer("/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        out.dropped_rows += 1;
        return;
    };
    let Some(title) = collect_runs_text(card.get("title")) else {
        out.dropped_rows += 1;
        return;
    };
    let subtitle = collect_runs_text(card.get("subtitle")).unwrap_or_default();
    let page_type = ep
        .pointer("/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str);
    let kind = match page_type {
        Some("MUSIC_PAGE_TYPE_ARTIST") => HomeCardKind::Artist,
        Some("MUSIC_PAGE_TYPE_ALBUM") => HomeCardKind::Album,
        Some("MUSIC_PAGE_TYPE_PLAYLIST") => HomeCardKind::Playlist,
        _ if browse_id.starts_with("UC") => HomeCardKind::Artist,
        _ if browse_id.starts_with("MPRE") => HomeCardKind::Album,
        _ if browse_id.starts_with("VL")
            || browse_id.starts_with("PL")
            || browse_id.starts_with("OLAK") =>
        {
            HomeCardKind::Playlist
        }
        _ => HomeCardKind::Album,
    };
    let card_item = HomeCard {
        kind,
        id: browse_id.to_string(),
        title,
        subtitle,
        thumbnail_url: extract_card_shelf_thumbnail(card),
        track: None,
    };
    match kind {
        HomeCardKind::Artist => push_unique_card(&mut out.artists, card_item, true),
        HomeCardKind::Playlist => push_unique_card(&mut out.playlists, card_item, true),
        _ => push_unique_card(&mut out.albums, card_item, true),
    }
}

fn ingest_responsive_search_item(
    resp: &Value,
    heading_hint: Option<&str>,
    out: &mut RichSearchResult,
) {
    // Prefer structural browse/watch endpoint classification over locale-dependent headings.
    if let Some(track) = parse_responsive_track(resp, None, None, None, None) {
        let mvt = watch_video_type(resp);
        let is_video = match mvt {
            Some("MUSIC_VIDEO_TYPE_ATV") => false,
            Some(_) if is_video_music_type(mvt) => true,
            _ => heading_hint == Some("video"),
        };
        if is_video {
            push_unique_track(&mut out.videos, track, false);
        } else {
            push_unique_track(&mut out.songs, track, false);
        }
        return;
    }

    let fallback_kind = match heading_hint {
        Some("artist") => HomeCardKind::Artist,
        Some("playlist") => HomeCardKind::Playlist,
        _ => HomeCardKind::Album,
    };
    if let Some(card) = parse_responsive_card(resp, fallback_kind) {
        match card.kind {
            HomeCardKind::Artist => push_unique_card(&mut out.artists, card, false),
            HomeCardKind::Playlist => push_unique_card(&mut out.playlists, card, false),
            _ => push_unique_card(&mut out.albums, card, false),
        }
    } else {
        out.dropped_rows += 1;
    }
}

fn collect_rich_search_shelves(val: &Value, out: &mut RichSearchResult, saw_shelf: &mut bool) {
    match val {
        Value::Object(map) => {
            if let Some(card) = map.get("musicCardShelfRenderer") {
                *saw_shelf = true;
                ingest_card_shelf_top_result(card, out);
                // Intentionally do NOT recurse into `musicCardShelfRenderer.contents`
                // ("More from YouTube" remix/AI-cover sub-items) so they don't bury primary results.
                return;
            }
            if let Some(shelf) = map.get("musicShelfRenderer") {
                *saw_shelf = true;
                let raw_heading = collect_runs_text(shelf.get("title")).unwrap_or_default();
                if !raw_heading.is_empty() {
                    out.shelf_order.push(raw_heading.clone());
                }
                let hint = heading_kind_hint(&raw_heading);
                if let Some(contents) = shelf.get("contents").and_then(Value::as_array) {
                    for item in contents {
                        if let Some(resp) = item.get("musicResponsiveListItemRenderer") {
                            ingest_responsive_search_item(resp, hint, out);
                        } else {
                            out.dropped_rows += 1;
                        }
                    }
                }
                return;
            }
            if let Some(section) = map.get("itemSectionRenderer") {
                if let Some(contents) = section.get("contents").and_then(Value::as_array) {
                    for item in contents {
                        if let Some(resp) = item.get("musicResponsiveListItemRenderer") {
                            *saw_shelf = true;
                            ingest_responsive_search_item(resp, None, out);
                        } else {
                            collect_rich_search_shelves(item, out, saw_shelf);
                        }
                    }
                }
                return;
            }
            for v in map.values() {
                collect_rich_search_shelves(v, out, saw_shelf);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_rich_search_shelves(v, out, saw_shelf);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_card_shelf_top_result_without_more_from_youtube_pollution() {
        let payload = json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [
                                        {
                                            "musicCardShelfRenderer": {
                                                "header": {
                                                    "musicCardShelfHeaderBasicRenderer": {
                                                        "title": { "runs": [{ "text": "En iyi sonuç" }] }
                                                    }
                                                },
                                                "title": {
                                                    "runs": [{
                                                        "text": "Daft Punk - Harder Better Faster Stronger",
                                                        "navigationEndpoint": {
                                                            "watchEndpoint": {
                                                                "videoId": "topVid12345",
                                                                "watchEndpointMusicSupportedConfigs": {
                                                                    "watchEndpointMusicConfig": {
                                                                        "musicVideoType": "MUSIC_VIDEO_TYPE_ATV"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }]
                                                },
                                                "subtitle": {
                                                    "runs": [
                                                        { "text": "Şarkı" },
                                                        { "text": " • " },
                                                        { "text": "Daft Punk" },
                                                        { "text": " • " },
                                                        { "text": "3:44" }
                                                    ]
                                                },
                                                "contents": [
                                                    {
                                                        "musicResponsiveListItemRenderer": {
                                                            "flexColumns": [
                                                                {
                                                                    "musicResponsiveListItemFlexColumnRenderer": {
                                                                        "text": {
                                                                            "runs": [{
                                                                                "text": "AI Cover Slowed Reverb",
                                                                                "navigationEndpoint": {
                                                                                    "watchEndpoint": {
                                                                                        "videoId": "spamVid9999"
                                                                                    }
                                                                                }
                                                                            }]
                                                                        }
                                                                    }
                                                                }
                                                            ]
                                                        }
                                                    }
                                                ]
                                            }
                                        },
                                        {
                                            "musicShelfRenderer": {
                                                "title": { "runs": [{ "text": "Şarkılar" }] },
                                                "contents": [
                                                    {
                                                        "musicResponsiveListItemRenderer": {
                                                            "flexColumns": [
                                                                {
                                                                    "musicResponsiveListItemFlexColumnRenderer": {
                                                                        "text": {
                                                                            "runs": [{
                                                                                "text": "One More Time",
                                                                                "navigationEndpoint": {
                                                                                    "watchEndpoint": {
                                                                                        "videoId": "songVid2222",
                                                                                        "watchEndpointMusicSupportedConfigs": {
                                                                                            "watchEndpointMusicConfig": {
                                                                                                "musicVideoType": "MUSIC_VIDEO_TYPE_ATV"
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }]
                                                                        }
                                                                    }
                                                                },
                                                                {
                                                                    "musicResponsiveListItemFlexColumnRenderer": {
                                                                        "text": {
                                                                            "runs": [
                                                                                { "text": "Daft Punk" },
                                                                                { "text": " • " },
                                                                                { "text": "5:20" }
                                                                            ]
                                                                        }
                                                                    }
                                                                }
                                                            ]
                                                        }
                                                    }
                                                ]
                                            }
                                        }
                                    ]
                                }
                            }
                        }
                    }]
                }
            }
        });

        let result = parse_rich_search_json(&payload).expect("rich search payload should parse");
        assert_eq!(result.songs.len(), 2);
        assert_eq!(result.songs[0].id, "topVid12345");
        assert_eq!(result.songs[0].artist, "Daft Punk");
        assert_eq!(result.songs[1].id, "songVid2222");
        assert!(result.videos.is_empty());
        assert!(
            !result.songs.iter().any(|t| t.id == "spamVid9999"),
            "More from YouTube sub-items inside musicCardShelfRenderer must not pollute songs"
        );
    }
}
