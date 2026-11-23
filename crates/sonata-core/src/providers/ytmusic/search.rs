//! Pure YouTube Music InnerTube search suggestion and typed rich-search parsers.
//!
//! 1. **Suggestions**: `parse_search_suggestions_json` extracts deduplicated autocomplete suggestions
//!    from `music/get_search_suggestions` responses while counting malformed suggestion nodes.
//! 2. **Rich Search**: `parse_rich_search_json` categorizes `musicShelfRenderer` rows into songs,
//!    videos, albums, artists, and playlists while preserving shelf order and reporting `dropped_rows`.

use super::browse::{
    collect_runs_text, parse_responsive_card, parse_responsive_track, ParsedCollection,
};
use super::{HomeCard, HomeCardKind};
use crate::error::CoreError;
use crate::providers::Track;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct RichSearchResult {
    pub songs: Vec<Track>,
    pub videos: Vec<Track>,
    pub albums: Vec<HomeCard>,
    pub artists: Vec<HomeCard>,
    pub playlists: Vec<HomeCard>,
    /// Original heading order of recognized `musicShelfRenderer` sections in the InnerTube payload.
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
/// Returns `CoreError::InvalidResponse` when the payload is not a recognized InnerTube search
/// response object (missing both `contents` and any `musicShelfRenderer`), preventing silent
/// empty success on unexpected schema changes.
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

fn collect_rich_search_shelves(val: &Value, out: &mut RichSearchResult, saw_shelf: &mut bool) {
    match val {
        Value::Object(map) => {
            if let Some(shelf) = map.get("musicShelfRenderer") {
                *saw_shelf = true;
                let raw_heading = collect_runs_text(shelf.get("title")).unwrap_or_default();
                if !raw_heading.is_empty() {
                    out.shelf_order.push(raw_heading.clone());
                }
                let heading = raw_heading.to_ascii_lowercase();
                if let Some(contents) = shelf.get("contents").and_then(Value::as_array) {
                    for item in contents {
                        if let Some(resp) = item.get("musicResponsiveListItemRenderer") {
                            if heading.contains("video") {
                                match parse_responsive_track(resp, None, None, None, None) {
                                    Some(t) => out.videos.push(t),
                                    None => out.dropped_rows += 1,
                                }
                            } else if heading.contains("album") {
                                match parse_responsive_card(resp, HomeCardKind::Album) {
                                    Some(c) => out.albums.push(c),
                                    None => out.dropped_rows += 1,
                                }
                            } else if heading.contains("artist") {
                                match parse_responsive_card(resp, HomeCardKind::Artist) {
                                    Some(c) => out.artists.push(c),
                                    None => out.dropped_rows += 1,
                                }
                            } else if heading.contains("playlist") {
                                match parse_responsive_card(resp, HomeCardKind::Playlist) {
                                    Some(c) => out.playlists.push(c),
                                    None => out.dropped_rows += 1,
                                }
                            } else if let Some(t) =
                                parse_responsive_track(resp, None, None, None, None)
                            {
                                out.songs.push(t);
                            } else if let Some(c) = parse_responsive_card(resp, HomeCardKind::Album)
                            {
                                match c.kind {
                                    HomeCardKind::Artist => out.artists.push(c),
                                    HomeCardKind::Playlist => out.playlists.push(c),
                                    _ => out.albums.push(c),
                                }
                            } else {
                                out.dropped_rows += 1;
                            }
                        } else {
                            out.dropped_rows += 1;
                        }
                    }
                }
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
