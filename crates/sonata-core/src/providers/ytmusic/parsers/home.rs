//! Home-feed shelf, mood-chip, and two-row card parsers for YouTube Music InnerTube responses.
//!
//! 1. **Shelf Traversal**: walks carousels, shelves, chip clouds, and grid sections into
//!    deduplicated `HomeShelf` collections while excluding ignored shelves.
//! 2. **Two-Row Cards**: maps `musicTwoRowItemRenderer` nodes into typed `HomeCard` entries
//!    (Album, Playlist, Artist, Song, Video) and `watchPlaylistEndpoint` mixes.

use super::{
    extract_last_thumbnail, extract_runs_text, has_explicit_badge, parse_responsive_track,
};
use crate::providers::ytmusic::{HomeCard, HomeCardKind, HomeShelf};
use crate::providers::{ProviderSource, Track};
use serde_json::Value;

pub(in crate::providers::ytmusic) fn is_excluded_shelf_title(title: &str) -> bool {
    let lower = title.trim().to_lowercase();
    lower.contains("forgotten favorite")
        || lower.contains("unutulan favori")
        || lower.contains("забытые хиты")
        || lower.contains("забытое любимое")
        || lower.contains("давно забытое")
}

pub(in crate::providers::ytmusic) fn merge_home_shelves(
    dst: &mut Vec<HomeShelf>,
    src: Vec<HomeShelf>,
) {
    for shelf in src {
        if is_excluded_shelf_title(&shelf.title) {
            continue;
        }
        if dst
            .iter()
            .any(|existing| existing.title.eq_ignore_ascii_case(&shelf.title))
        {
            continue;
        }
        dst.push(shelf);
    }
}

pub fn parse_home_shelves_json(val: &Value) -> Vec<HomeShelf> {
    let mut shelves = Vec::new();
    collect_home_shelves(val, &mut shelves);
    shelves.retain(|s| !is_excluded_shelf_title(&s.title));
    shelves
}

fn collect_home_shelves(val: &Value, out: &mut Vec<HomeShelf>) {
    match val {
        Value::Object(map) => {
            if let Some(chips) = map.get("chipCloudRenderer") {
                if let Some(shelf) = parse_mood_chips_shelf(chips) {
                    out.push(shelf);
                }
                return;
            }
            if let Some(carousel) = map
                .get("musicCarouselShelfRenderer")
                .or_else(|| map.get("musicImmersiveCarouselShelfRenderer"))
            {
                if let Some(shelf) = parse_shelf_node(carousel, true) {
                    out.push(shelf);
                }
                return;
            }
            if let Some(shelf_node) = map.get("musicShelfRenderer") {
                if let Some(shelf) = parse_shelf_node(shelf_node, false) {
                    out.push(shelf);
                }
                return;
            }
            if let Some(grid) = map.get("gridRenderer") {
                if let Some(shelf) = parse_grid_shelf(grid) {
                    out.push(shelf);
                }
                return;
            }
            if let Some(header) = map.get("header") {
                collect_home_shelves(header, out);
            }
            for (k, v) in map {
                if k != "header" {
                    collect_home_shelves(v, out);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_home_shelves(item, out);
            }
        }
        _ => {}
    }
}

fn parse_mood_chips_shelf(node: &Value) -> Option<HomeShelf> {
    let chips = node.get("chips").and_then(|v| v.as_array())?;
    let mut cards = Vec::new();
    for chip in chips {
        let Some(r) = chip.get("chipCloudChipRenderer") else {
            continue;
        };
        let Some(title) = r.get("text").and_then(extract_runs_text) else {
            continue;
        };
        let title = title.trim().to_string();
        if title.is_empty() {
            continue;
        }
        cards.push(HomeCard {
            kind: HomeCardKind::Mood,
            id: title.clone(),
            title,
            subtitle: "Mood".to_string(),
            thumbnail_url: None,
            track: None,
        });
    }
    if cards.is_empty() {
        return None;
    }
    Some(HomeShelf {
        title: "Find your mood".to_string(),
        tracks: Vec::new(),
        cards,
    })
}

fn parse_grid_shelf(node: &Value) -> Option<HomeShelf> {
    let title = node
        .pointer("/header/gridHeaderRenderer/title")
        .and_then(extract_runs_text)
        .unwrap_or_else(|| "Moods & genres".to_string());
    let items = node.get("items").and_then(|v| v.as_array())?;
    let mut cards = Vec::new();
    for entry in items {
        if let Some(two_row) = entry.get("musicTwoRowItemRenderer") {
            if let Some(card) = parse_home_two_row(two_row) {
                cards.push(card);
            }
        } else if let Some(nav_btn) = entry.get("musicNavigationButtonRenderer") {
            if let Some(card) = parse_mood_button(nav_btn) {
                cards.push(card);
            }
        }
    }
    if cards.is_empty() {
        return None;
    }
    Some(HomeShelf {
        title,
        tracks: Vec::new(),
        cards,
    })
}

fn parse_mood_button(r: &Value) -> Option<HomeCard> {
    let title = r.get("buttonText").and_then(extract_runs_text)?;
    let title = title.trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some(HomeCard {
        kind: HomeCardKind::Mood,
        id: title.clone(),
        title,
        subtitle: "Mood & genre".to_string(),
        thumbnail_url: None,
        track: None,
    })
}

fn parse_shelf_node(node: &Value, is_carousel: bool) -> Option<HomeShelf> {
    let title = if is_carousel {
        node.pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
            .and_then(extract_runs_text)
            .or_else(|| {
                node.pointer("/header/musicImmersiveCarouselShelfBasicHeaderRenderer/title")
                    .and_then(extract_runs_text)
            })
    } else {
        node.pointer("/header/musicShelfBasicHeaderRenderer/title")
            .and_then(extract_runs_text)
            .or_else(|| node.get("title").and_then(extract_runs_text))
    }
    .unwrap_or_else(|| "Featured".to_string());

    let contents = node.get("contents").and_then(|v| v.as_array())?;
    let mut tracks = Vec::new();
    let mut cards = Vec::new();

    for entry in contents {
        if let Some(resp) = entry.get("musicResponsiveListItemRenderer") {
            if let Some(t) = parse_responsive_track(resp) {
                tracks.push(t);
            }
        } else if let Some(two_row) = entry.get("musicTwoRowItemRenderer") {
            if let Some(card) = parse_home_two_row(two_row) {
                cards.push(card);
            }
        } else if let Some(nav_btn) = entry.get("musicNavigationButtonRenderer") {
            if let Some(card) = parse_mood_button(nav_btn) {
                cards.push(card);
            }
        }
    }

    if tracks.is_empty() && cards.is_empty() {
        return None;
    }

    Some(HomeShelf {
        title,
        tracks,
        cards,
    })
}

pub(super) fn parse_home_two_row(r: &Value) -> Option<HomeCard> {
    let title = r.get("title").and_then(extract_runs_text)?;
    let subtitle = r
        .get("subtitle")
        .and_then(extract_runs_text)
        .unwrap_or_default();
    let thumbnail_url = extract_last_thumbnail(r.get("thumbnailRenderer"));

    let nav = r.get("navigationEndpoint")?;
    if let Some(browse_id) = nav
        .pointer("/browseEndpoint/browseId")
        .and_then(|v| v.as_str())
    {
        if browse_id.starts_with("MPRE") {
            return Some(HomeCard {
                kind: HomeCardKind::Album,
                id: browse_id.to_string(),
                title,
                subtitle,
                thumbnail_url,
                track: None,
            });
        }
        if browse_id.starts_with("VL")
            || browse_id.starts_with("PL")
            || browse_id.starts_with("OLAK")
            || browse_id.starts_with("RD")
        {
            let norm_id = if browse_id.starts_with("VL") {
                browse_id.to_string()
            } else {
                format!("VL{browse_id}")
            };
            return Some(HomeCard {
                kind: HomeCardKind::Playlist,
                id: norm_id,
                title,
                subtitle,
                thumbnail_url,
                track: None,
            });
        }
        if browse_id.starts_with("UC") {
            return Some(HomeCard {
                kind: HomeCardKind::Artist,
                id: browse_id.to_string(),
                title,
                subtitle,
                thumbnail_url,
                track: None,
            });
        }
    }

    // Personal mixes, Supermix, Replay Mix, Discover Mix, and Radio playlists use watchPlaylistEndpoint.
    if let Some(playlist_id) = nav
        .pointer("/watchPlaylistEndpoint/playlistId")
        .or_else(|| r.pointer("/thumbnailOverlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchPlaylistEndpoint/playlistId"))
        .and_then(|v| v.as_str())
    {
        return Some(HomeCard {
            kind: HomeCardKind::Playlist,
            id: playlist_id.to_string(),
            title,
            subtitle,
            thumbnail_url,
            track: None,
        });
    }

    if let Some(video_id) = nav
        .pointer("/watchEndpoint/videoId")
        .and_then(|v| v.as_str())
    {
        let is_video = r
            .get("aspectRatio")
            .and_then(|v| v.as_str())
            .is_some_and(|a| a.contains("16_9"))
            || subtitle.to_ascii_lowercase().starts_with("video");
        let kind = if is_video {
            HomeCardKind::Video
        } else {
            HomeCardKind::Song
        };
        let artist = extract_artist_from_subtitle(&subtitle);
        let track = Track {
            id: video_id.to_string(),
            title: title.clone(),
            artist,
            album: String::new(),
            duration_secs: None,
            thumbnail_url: thumbnail_url.clone(),
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: has_explicit_badge(r),
        };
        return Some(HomeCard {
            kind,
            id: video_id.to_string(),
            title,
            subtitle,
            thumbnail_url,
            track: Some(track),
        });
    }

    None
}

fn extract_artist_from_subtitle(subtitle: &str) -> String {
    let parts: Vec<&str> = subtitle
        .split('•')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !matches!(*s, "Song" | "Video" | "Single" | "EP"))
        .collect();
    parts
        .first()
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| "YouTube Music".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ytmusic::parsers::find_home_continuation;
    use serde_json::json;

    #[test]
    fn parses_home_shelves_for_cards_and_tracks() {
        let sample = json!({
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [
                                        {
                                            "musicCarouselShelfRenderer": {
                                                "header": {
                                                    "musicCarouselShelfBasicHeaderRenderer": {
                                                        "title": { "runs": [{ "text": "New albums & singles" }] }
                                                    }
                                                },
                                                "contents": [{
                                                    "musicTwoRowItemRenderer": {
                                                        "title": { "runs": [{ "text": "Midnight City" }] },
                                                        "subtitle": { "runs": [{ "text": "Album • M83" }] },
                                                        "thumbnailRenderer": {
                                                            "musicThumbnailRenderer": {
                                                                "thumbnail": { "thumbnails": [{ "url": "https://img.example/alb.jpg" }] }
                                                            }
                                                        },
                                                        "navigationEndpoint": {
                                                            "browseEndpoint": { "browseId": "MPREb_12345" }
                                                        }
                                                    }
                                                }]
                                            }
                                        },
                                        {
                                            "musicCarouselShelfRenderer": {
                                                "header": {
                                                    "musicCarouselShelfBasicHeaderRenderer": {
                                                        "title": { "runs": [{ "text": "Trending" }] }
                                                    }
                                                },
                                                "contents": [{
                                                    "musicResponsiveListItemRenderer": {
                                                        "playlistItemData": { "videoId": "vid_999" },
                                                        "flexColumns": [
                                                            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Track One" }] } } },
                                                            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Artist X" }, { "text": " • " }, { "text": "3:20" }] } } }
                                                        ]
                                                    }
                                                }]
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

        let shelves = parse_home_shelves_json(&sample);
        assert_eq!(shelves.len(), 2);
        assert_eq!(shelves[0].title, "New albums & singles");
        assert_eq!(shelves[0].cards.len(), 1);
        assert_eq!(shelves[0].cards[0].kind, HomeCardKind::Album);
        assert_eq!(shelves[0].cards[0].id, "MPREb_12345");
        assert_eq!(shelves[1].title, "Trending");
        assert_eq!(shelves[1].tracks.len(), 1);
        assert_eq!(shelves[1].tracks[0].id, "vid_999");
        assert_eq!(shelves[1].tracks[0].artist, "Artist X");
        assert_eq!(shelves[1].tracks[0].duration_secs, Some(200));
    }

    #[test]
    fn parses_home_continuations_personal_mixes_and_moods() {
        let sample = json!({
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "header": {
                                        "chipCloudRenderer": {
                                            "chips": [
                                                { "chipCloudChipRenderer": { "text": { "runs": [{ "text": "Energize" }] } } },
                                                { "chipCloudChipRenderer": { "text": { "runs": [{ "text": "Focus" }] } } }
                                            ]
                                        }
                                    },
                                    "contents": [{
                                        "musicCarouselShelfRenderer": {
                                            "header": {
                                                "musicCarouselShelfBasicHeaderRenderer": {
                                                    "title": { "runs": [{ "text": "Mixed for you" }] }
                                                }
                                            },
                                            "contents": [{
                                                "musicTwoRowItemRenderer": {
                                                    "title": { "runs": [{ "text": "My Supermix" }] },
                                                    "subtitle": { "runs": [{ "text": "Playlist • Personalized mix" }] },
                                                    "navigationEndpoint": {
                                                        "watchPlaylistEndpoint": { "playlistId": "RDTMAK5uy_supermix" }
                                                    }
                                                }
                                            }]
                                        }
                                    }],
                                    "continuations": [{
                                        "nextContinuationData": { "continuation": "HOME_CONT_TOKEN_1" }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });

        assert_eq!(
            find_home_continuation(&sample).as_deref(),
            Some("HOME_CONT_TOKEN_1")
        );

        let shelves = parse_home_shelves_json(&sample);
        assert_eq!(shelves.len(), 2);
        assert_eq!(shelves[0].title, "Find your mood");
        assert_eq!(shelves[0].cards.len(), 2);
        assert_eq!(shelves[0].cards[0].kind, HomeCardKind::Mood);
        assert_eq!(shelves[0].cards[0].title, "Energize");

        assert_eq!(shelves[1].title, "Mixed for you");
        assert_eq!(shelves[1].cards.len(), 1);
        assert_eq!(shelves[1].cards[0].kind, HomeCardKind::Playlist);
        assert_eq!(shelves[1].cards[0].id, "RDTMAK5uy_supermix");
    }

    #[test]
    fn drops_forgotten_favorites_home_shelf() {
        let forgotten = json!({
            "contents": [{
                "musicCarouselShelfRenderer": {
                    "header": {
                        "musicCarouselShelfBasicHeaderRenderer": {
                            "title": { "runs": [{ "text": "Forgotten favorites" }] }
                        }
                    },
                    "contents": [{
                        "musicTwoRowItemRenderer": {
                            "title": { "runs": [{ "text": "Old Song" }] },
                            "subtitle": { "runs": [{ "text": "Artist" }] },
                            "navigationEndpoint": { "watchEndpoint": { "videoId": "vid1" } }
                        }
                    }]
                }
            }]
        });
        assert!(parse_home_shelves_json(&forgotten).is_empty());
    }
}
