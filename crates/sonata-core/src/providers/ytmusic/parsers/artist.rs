//! Artist-page and search browse-ID parsers for YouTube Music InnerTube responses.
//!
//! 1. **Search Browse Resolution**: extracts ranked `UC...` artist and `MPRE`/`OLAK` album
//!    browse IDs from search shelf and song-row fallback nodes.
//! 2. **Artist Page**: parses `ArtistPage` hero metadata, listener counts, radio/shuffle
//!    endpoints, top tracks, releases, and related artists.

use super::{
    collect_tracks, extract_runs_text, find_description_subheader, find_description_text,
    find_header_thumbnail, parse_home_two_row,
};
use crate::providers::ytmusic::{ArtistPage, HomeCard, HomeCardKind};
use serde_json::Value;

pub(in crate::providers::ytmusic) fn extract_search_artist_browse_id(
    val: &Value,
    preferred_name: &str,
) -> Option<String> {
    #[derive(Debug)]
    struct Candidate {
        browse_id: String,
        title: String,
        is_artist_item: bool,
    }

    fn push_if_uc(
        out: &mut Vec<Candidate>,
        browse_id: Option<&str>,
        title: Option<String>,
        is_artist_item: bool,
    ) {
        let Some(id) = browse_id.map(str::trim).filter(|id| id.starts_with("UC")) else {
            return;
        };
        out.push(Candidate {
            browse_id: id.to_string(),
            title: title.unwrap_or_default().trim().to_string(),
            is_artist_item,
        });
    }

    fn walk(value: &Value, out: &mut Vec<Candidate>) {
        match value {
            Value::Object(map) => {
                if let Some(card) = map.get("musicCardShelfRenderer") {
                    let title = card
                        .pointer("/title")
                        .and_then(extract_runs_text)
                        .or_else(|| {
                            card.pointer("/header/musicCardShelfHeaderBasicRenderer/title")
                                .and_then(extract_runs_text)
                        });
                    let browse_id = card
                        .pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId")
                        .or_else(|| {
                            card.pointer("/onTap/browseEndpoint/browseId")
                        })
                        .or_else(|| {
                            card.pointer("/header/musicCardShelfHeaderBasicRenderer/title/runs/0/navigationEndpoint/browseEndpoint/browseId")
                        })
                        .and_then(|v| v.as_str());
                    push_if_uc(out, browse_id, title, true);
                }
                if let Some(item) = map.get("musicResponsiveListItemRenderer") {
                    let title = item
                        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")
                        .and_then(extract_runs_text);
                    let browse_id = item
                        .pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .or_else(|| {
                            item.pointer(
                                "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
                            )
                        })
                        .and_then(|v| v.as_str());
                    push_if_uc(out, browse_id, title, true);

                    // Fallback: linked artist runs inside secondary flex columns (e.g. unfiltered song rows).
                    if let Some(cols) = item.pointer("/flexColumns").and_then(|v| v.as_array()) {
                        for col in cols.iter().skip(1) {
                            if let Some(runs) = col
                                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                                .and_then(|v| v.as_array())
                            {
                                for run in runs {
                                    let run_text = run
                                        .get("text")
                                        .and_then(|v| v.as_str())
                                        .map(str::to_string);
                                    let run_id = run
                                        .pointer("/navigationEndpoint/browseEndpoint/browseId")
                                        .and_then(|v| v.as_str());
                                    push_if_uc(out, run_id, run_text, false);
                                }
                            }
                        }
                    }
                }
                for child in map.values() {
                    walk(child, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }

    let mut candidates = Vec::new();
    walk(val, &mut candidates);
    if candidates.is_empty() {
        return None;
    }

    let needle = preferred_name.trim().to_ascii_lowercase();
    if !needle.is_empty() {
        if let Some(exact) = candidates
            .iter()
            .find(|c| c.is_artist_item && c.title.eq_ignore_ascii_case(&needle))
        {
            return Some(exact.browse_id.clone());
        }
        if let Some(exact_any) = candidates
            .iter()
            .find(|c| c.title.eq_ignore_ascii_case(&needle))
        {
            return Some(exact_any.browse_id.clone());
        }
        if let Some(partial) = candidates.iter().find(|c| {
            c.is_artist_item && {
                let lower = c.title.to_ascii_lowercase();
                !lower.is_empty() && (lower.contains(&needle) || needle.contains(&lower))
            }
        }) {
            return Some(partial.browse_id.clone());
        }
    }

    candidates
        .into_iter()
        .find(|c| c.is_artist_item)
        .map(|c| c.browse_id)
}

pub(in crate::providers::ytmusic) fn extract_search_album_browse_id(
    val: &Value,
    preferred_album: &str,
) -> Option<String> {
    #[derive(Debug)]
    struct Candidate {
        browse_id: String,
        title: String,
        is_album_item: bool,
    }

    fn is_album_browse_id(id: &str) -> bool {
        id.starts_with("MPRE") || id.starts_with("OLAK") || id.starts_with("VLOLAK")
    }

    fn walk(value: &Value, out: &mut Vec<Candidate>) {
        match value {
            Value::Object(map) => {
                if let Some(card) = map.get("musicCardShelfRenderer") {
                    let title = card
                        .pointer("/title")
                        .and_then(extract_runs_text)
                        .or_else(|| {
                            card.pointer("/header/musicCardShelfHeaderBasicRenderer/title")
                                .and_then(extract_runs_text)
                        })
                        .unwrap_or_default();
                    let browse_id = card
                        .pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId")
                        .or_else(|| card.pointer("/onTap/browseEndpoint/browseId"))
                        .and_then(|v| v.as_str());
                    if let Some(id) = browse_id.map(str::trim).filter(|id| is_album_browse_id(id)) {
                        out.push(Candidate {
                            browse_id: id.to_string(),
                            title: title.trim().to_string(),
                            is_album_item: true,
                        });
                    }
                }
                if let Some(item) = map.get("musicResponsiveListItemRenderer") {
                    let title = item
                        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")
                        .and_then(extract_runs_text)
                        .unwrap_or_default();
                    let browse_id = item
                        .pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .or_else(|| {
                            item.pointer(
                                "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
                            )
                        })
                        .and_then(|v| v.as_str());
                    if let Some(id) = browse_id.map(str::trim).filter(|id| is_album_browse_id(id)) {
                        out.push(Candidate {
                            browse_id: id.to_string(),
                            title: title.trim().to_string(),
                            is_album_item: true,
                        });
                    }
                    if let Some(cols) = item.pointer("/flexColumns").and_then(|v| v.as_array()) {
                        for col in cols.iter().skip(1) {
                            if let Some(runs) = col
                                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                                .and_then(|v| v.as_array())
                            {
                                for run in runs {
                                    if let Some(id) = run
                                        .pointer("/navigationEndpoint/browseEndpoint/browseId")
                                        .and_then(|v| v.as_str())
                                        .map(str::trim)
                                        .filter(|id| is_album_browse_id(id))
                                    {
                                        let run_title = run
                                            .get("text")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default()
                                            .trim()
                                            .to_string();
                                        out.push(Candidate {
                                            browse_id: id.to_string(),
                                            title: run_title,
                                            is_album_item: false,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                for child in map.values() {
                    walk(child, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }

    let mut candidates = Vec::new();
    walk(val, &mut candidates);
    if candidates.is_empty() {
        return None;
    }

    let needle = preferred_album.trim().to_ascii_lowercase();
    if !needle.is_empty() {
        if let Some(exact) = candidates
            .iter()
            .find(|c| c.is_album_item && c.title.eq_ignore_ascii_case(&needle))
        {
            return Some(exact.browse_id.clone());
        }
        if let Some(exact_any) = candidates
            .iter()
            .find(|c| c.title.eq_ignore_ascii_case(&needle))
        {
            return Some(exact_any.browse_id.clone());
        }
        if let Some(partial) = candidates.iter().find(|c| {
            c.is_album_item && {
                let lower = c.title.to_ascii_lowercase();
                !lower.is_empty() && (lower.contains(&needle) || needle.contains(&lower))
            }
        }) {
            return Some(partial.browse_id.clone());
        }
    }

    candidates
        .into_iter()
        .find(|c| c.is_album_item)
        .map(|c| c.browse_id)
}

pub(in crate::providers::ytmusic) fn find_artist_songs_endpoint(
    val: &Value,
) -> Option<(String, Option<String>)> {
    fn walk(value: &Value) -> Option<(String, Option<String>)> {
        match value {
            Value::Object(map) => {
                if let Some(shelf) = map.get("musicShelfRenderer") {
                    let title = shelf
                        .get("title")
                        .and_then(extract_runs_text)
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase();
                    let is_songs_shelf =
                        title == "songs" || title == "top songs" || title.contains("songs");
                    if is_songs_shelf {
                        if let Some(browse_ep) =
                            shelf.pointer("/bottomEndpoint/browseEndpoint").or_else(|| {
                                shelf.pointer("/title/runs/0/navigationEndpoint/browseEndpoint")
                            })
                        {
                            if let Some(bid) = browse_ep
                                .get("browseId")
                                .and_then(|v| v.as_str())
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                let params = browse_ep
                                    .get("params")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string);
                                return Some((bid.to_string(), params));
                            }
                        }
                    }
                }
                for child in map.values() {
                    if let Some(found) = walk(child) {
                        return Some(found);
                    }
                }
                None
            }
            Value::Array(items) => items.iter().find_map(walk),
            _ => None,
        }
    }
    walk(val)
}

pub fn parse_artist_page_json(browse_id: &str, val: &Value) -> Option<ArtistPage> {
    let immersive_hdr = val.pointer("/header/musicImmersiveHeaderRenderer");
    let visual_hdr = val.pointer("/header/musicVisualHeaderRenderer");
    let name = immersive_hdr
        .and_then(|h| h.get("title"))
        .and_then(extract_runs_text)
        .or_else(|| {
            visual_hdr
                .and_then(|h| h.get("title"))
                .and_then(extract_runs_text)
        })
        .or_else(|| find_first_runs_text(val, "title"))?;
    let subtitle = immersive_hdr
        .and_then(|h| h.get("description"))
        .and_then(extract_runs_text)
        .or_else(|| {
            visual_hdr
                .and_then(|h| h.get("description"))
                .and_then(extract_runs_text)
        })
        .unwrap_or_default();
    let description = find_description_text(val)
        .or_else(|| (!subtitle.trim().is_empty()).then(|| subtitle.clone()));
    let subscribers = immersive_hdr
        .or(visual_hdr)
        .and_then(|h| {
            h.pointer("/subscriptionButton/subscribeButtonRenderer/subscriberCountText")
                .or_else(|| {
                    h.pointer("/subscriptionButton/subscribeButtonRenderer/longSubscriberCountText")
                })
        })
        .and_then(extract_runs_text)
        .filter(|s| !s.trim().is_empty());
    let monthly_listeners = immersive_hdr
        .and_then(|h| h.get("monthlyListenerCount"))
        .and_then(extract_runs_text)
        .or_else(|| find_description_subheader(val))
        .filter(|s| !s.trim().is_empty());
    let shuffle_playlist_id = immersive_hdr
        .and_then(|h| {
            h.pointer("/playButton/buttonRenderer/navigationEndpoint/watchEndpoint/playlistId")
        })
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let radio_playlist_id = immersive_hdr
        .and_then(|h| {
            h.pointer(
                "/startRadioButton/buttonRenderer/navigationEndpoint/watchEndpoint/playlistId",
            )
        })
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let thumbnail_url = find_header_thumbnail(val);
    let mut popular_tracks = Vec::new();
    collect_tracks(val, &mut popular_tracks);
    popular_tracks.truncate(24);
    let mut cards = Vec::new();
    collect_artist_cards(val, &mut cards);
    let mut releases = Vec::new();
    let mut related_artists = Vec::new();
    for card in cards {
        match card.kind {
            HomeCardKind::Album | HomeCardKind::Playlist => releases.push(card),
            HomeCardKind::Artist => related_artists.push(card),
            _ => {}
        }
    }
    releases.truncate(24);
    related_artists.truncate(12);
    let latest_release = releases.first().cloned();
    Some(ArtistPage {
        id: browse_id.to_string(),
        name,
        subtitle,
        description,
        subscribers,
        monthly_listeners,
        thumbnail_url,
        shuffle_playlist_id,
        radio_playlist_id,
        latest_release,
        popular_tracks,
        releases,
        related_artists,
    })
}

fn collect_artist_cards(value: &Value, out: &mut Vec<HomeCard>) {
    match value {
        Value::Object(map) => {
            if let Some(card) = map
                .get("musicTwoRowItemRenderer")
                .and_then(parse_home_two_row)
            {
                out.push(card);
                return;
            }
            for child in map.values() {
                collect_artist_cards(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_artist_cards(item, out);
            }
        }
        _ => {}
    }
}

fn find_first_runs_text(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(text) = map.get(key).and_then(extract_runs_text) {
                return Some(text);
            }
            map.values()
                .find_map(|child| find_first_runs_text(child, key))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| find_first_runs_text(item, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn artist_cards_and_pages_parse_without_guessing_artist_ids() {
        let card = json!({
            "title": { "runs": [{ "text": "Artist One" }] },
            "subtitle": { "runs": [{ "text": "1.2M monthly listeners" }] },
            "navigationEndpoint": { "browseEndpoint": { "browseId": "UCartist" } }
        });
        let parsed_card = parse_home_two_row(&card).expect("artist card");
        assert_eq!(parsed_card.kind, HomeCardKind::Artist);
        assert_eq!(parsed_card.id, "UCartist");

        let page = json!({
            "header": {
                "musicImmersiveHeaderRenderer": {
                    "title": { "runs": [{ "text": "Artist One" }] },
                    "description": { "runs": [{ "text": "Official artist channel" }] }
                }
            },
            "contents": [{
                "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "popular-1" },
                    "flexColumns": [
                        { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Popular Song" }] } } },
                        { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Artist One" }, { "text": " • " }, { "text": "3:20" }] } } }
                    ]
                }
            }, {
                "musicTwoRowItemRenderer": {
                    "title": { "runs": [{ "text": "Album One" }] },
                    "subtitle": { "runs": [{ "text": "Album" }] },
                    "navigationEndpoint": { "browseEndpoint": { "browseId": "MPREalbum" } }
                }
            }]
        });
        let artist = parse_artist_page_json("UCartist", &page).expect("artist page");
        assert_eq!(artist.name, "Artist One");
        assert_eq!(artist.popular_tracks[0].id, "popular-1");
        assert_eq!(artist.releases[0].id, "MPREalbum");
    }

    #[test]
    fn extracts_artist_browse_id_from_artist_search_and_song_fallback() {
        let artist_search = json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicShelfRenderer": {
                                            "contents": [
                                                {
                                                    "musicResponsiveListItemRenderer": {
                                                        "flexColumns": [{
                                                            "musicResponsiveListItemFlexColumnRenderer": {
                                                                "text": { "runs": [{ "text": "The Weeknd Tribute" }] }
                                                            }
                                                        }],
                                                        "navigationEndpoint": {
                                                            "browseEndpoint": { "browseId": "UCtribute000" }
                                                        }
                                                    }
                                                },
                                                {
                                                    "musicResponsiveListItemRenderer": {
                                                        "flexColumns": [{
                                                            "musicResponsiveListItemFlexColumnRenderer": {
                                                                "text": { "runs": [{ "text": "The Weeknd" }] }
                                                            }
                                                        }],
                                                        "navigationEndpoint": {
                                                            "browseEndpoint": { "browseId": "UC0WP5P-ufpRfjbN6OWwLB4Q" }
                                                        }
                                                    }
                                                }
                                            ]
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });
        assert_eq!(
            extract_search_artist_browse_id(&artist_search, "The Weeknd").as_deref(),
            Some("UC0WP5P-ufpRfjbN6OWwLB4Q")
        );

        let song_fallback = json!({
            "contents": [{
                "musicResponsiveListItemRenderer": {
                    "flexColumns": [
                        {
                            "musicResponsiveListItemFlexColumnRenderer": {
                                "text": { "runs": [{ "text": "Starboy" }] }
                            }
                        },
                        {
                            "musicResponsiveListItemFlexColumnRenderer": {
                                "text": {
                                    "runs": [
                                        {
                                            "text": "The Weeknd",
                                            "navigationEndpoint": {
                                                "browseEndpoint": { "browseId": "UC0WP5P-ufpRfjbN6OWwLB4Q" }
                                            }
                                        },
                                        { "text": " • " },
                                        {
                                            "text": "Daft Punk",
                                            "navigationEndpoint": {
                                                "browseEndpoint": { "browseId": "UCdaftpunk" }
                                            }
                                        }
                                    ]
                                }
                            }
                        }
                    ]
                }
            }]
        });
        assert_eq!(
            extract_search_artist_browse_id(&song_fallback, "The Weeknd").as_deref(),
            Some("UC0WP5P-ufpRfjbN6OWwLB4Q")
        );
    }

    #[test]
    fn parses_immersive_artist_hero_songs_endpoint_and_album_search_id() {
        let artist_json = json!({
            "header": {
                "musicImmersiveHeaderRenderer": {
                    "title": { "runs": [{ "text": "The Weeknd" }] },
                    "monthlyListenerCount": { "runs": [{ "text": "114M monthly audience" }] },
                    "thumbnail": {
                        "musicThumbnailRenderer": {
                            "thumbnail": {
                                "thumbnails": [
                                    { "url": "https://lh3.googleusercontent.com/weeknd=w544-h544-l90-rj" }
                                ]
                            }
                        }
                    }
                }
            },
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [
                                        {
                                            "musicShelfRenderer": {
                                                "title": { "runs": [{ "text": "Songs" }] },
                                                "bottomEndpoint": {
                                                    "browseEndpoint": {
                                                        "browseId": "VLOLAK5uy_top_songs",
                                                        "params": "ggM%3D"
                                                    }
                                                }
                                            }
                                        },
                                        {
                                            "musicDescriptionShelfRenderer": {
                                                "description": { "runs": [{ "text": "Abel Tesfaye bio." }] }
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
        let page = parse_artist_page_json("UC0WP5P-ufpRfjbN6OWwLB4Q", &artist_json)
            .expect("immersive artist page");
        assert_eq!(
            page.thumbnail_url.as_deref(),
            Some("https://lh3.googleusercontent.com/weeknd=w1200-h1200-l90-rj")
        );
        assert_eq!(page.description.as_deref(), Some("Abel Tesfaye bio."));
        assert_eq!(
            page.monthly_listeners.as_deref(),
            Some("114M monthly audience")
        );
        assert_eq!(
            find_artist_songs_endpoint(&artist_json),
            Some((
                "VLOLAK5uy_top_songs".to_string(),
                Some("ggM%3D".to_string())
            ))
        );

        let album_search = json!({
            "contents": [{
                "musicResponsiveListItemRenderer": {
                    "flexColumns": [{
                        "musicResponsiveListItemFlexColumnRenderer": {
                            "text": { "runs": [{ "text": "Starboy" }] }
                        }
                    }],
                    "navigationEndpoint": {
                        "browseEndpoint": { "browseId": "MPREb_starboy123" }
                    }
                }
            }]
        });
        assert_eq!(
            extract_search_album_browse_id(&album_search, "Starboy").as_deref(),
            Some("MPREb_starboy123")
        );
    }
}
