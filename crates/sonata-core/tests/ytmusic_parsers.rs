//! Fixture-backed parser and error-degradation tests for YouTube Music browse, album, and search payloads.
//!
//! 1. **Fixture Parity**: Verifies mood chips, mood category tiles, album details, artist wide banners,
//!    library cards, autocomplete suggestions, and rich search categorization against sanitized JSON fixtures.
//! 2. **Degradation & Reporting**: Verifies shelf-order preservation, `dropped_rows` accounting on malformed
//!    entries, and typed `CoreError::InvalidResponse` on unrecognized response envelopes.

use serde_json::{json, Value};
use sonata_core::error::CoreError;
use sonata_core::providers::ytmusic::{
    parse_album_detail_json, parse_artist_banner_url_json, parse_cards_tree_json,
    parse_mood_chips_json, parse_mood_tiles_json, parse_rich_search_json,
    parse_search_suggestions_json, HomeCardKind,
};

fn fixture(name: &str) -> Value {
    let raw = match name {
        "ytm_home_chips.json" => include_str!("fixtures/ytm_home_chips.json"),
        "ytm_explore_moods.json" => include_str!("fixtures/ytm_explore_moods.json"),
        "ytm_album_detail.json" => include_str!("fixtures/ytm_album_detail.json"),
        "ytm_search_rich.json" => include_str!("fixtures/ytm_search_rich.json"),
        other => panic!("unknown fixture: {other}"),
    };
    serde_json::from_str(raw).expect("valid fixture json")
}

#[test]
fn parses_mood_chips_and_reports_dropped_rows() {
    let parsed = parse_mood_chips_json(&fixture("ytm_home_chips.json"));
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.items[0].label, "Relax");
    assert_eq!(parsed.items[1].label, "Focus");
    assert_eq!(parsed.dropped_rows, 1);
}

#[test]
fn parses_mood_tiles_with_rgb_accent_and_dropped_rows() {
    let parsed = parse_mood_tiles_json(&fixture("ytm_explore_moods.json"));
    assert_eq!(parsed.items.len(), 1);
    assert_eq!(parsed.items[0].title, "Chill");
    assert_eq!(
        parsed.items[0].browse_id,
        "FEmusic_moods_and_genres_category"
    );
    // 4288234210 == 0xFF9942E2 -> (0x99, 0x42, 0xE2) == (153, 66, 226)
    assert_eq!(parsed.items[0].accent_rgb, (153, 66, 226));
    assert_eq!(parsed.dropped_rows, 1);
}

#[test]
fn parses_album_detail_and_rejects_unknown_envelopes() {
    let parsed = parse_album_detail_json("MPREb_afterhours", &fixture("ytm_album_detail.json"))
        .expect("album detail parses");
    assert_eq!(parsed.detail.title, "After Hours");
    assert_eq!(parsed.detail.release_type, "Album");
    assert_eq!(parsed.detail.year.as_deref(), Some("2020"));
    assert_eq!(
        parsed.detail.playlist_id.as_deref(),
        Some("OLAK5uy_example123")
    );
    assert_eq!(parsed.detail.artists.len(), 1);
    assert_eq!(parsed.detail.artists[0].0, "The Weeknd");
    assert_eq!(parsed.detail.tracks.len(), 1);
    assert_eq!(parsed.detail.tracks[0].id, "fHI8X4OXluQ");
    assert_eq!(parsed.detail.tracks[0].duration_secs, Some(200));
    assert!(parsed.detail.tracks[0].is_explicit);
    assert_eq!(parsed.dropped_rows, 1);

    let err = parse_album_detail_json("MPREb_bad", &json!({ "unexpected": true }))
        .expect_err("unknown album schema must fail with typed error");
    assert!(matches!(
        err,
        CoreError::InvalidResponse { ref provider, .. } if provider == "ytmusic"
    ));
}

#[test]
fn parses_rich_search_preserving_shelf_order_and_dropped_rows() {
    let parsed =
        parse_rich_search_json(&fixture("ytm_search_rich.json")).expect("rich search parses");
    assert_eq!(parsed.shelf_order, vec!["Songs", "Artists", "Albums"]);
    assert_eq!(parsed.songs.len(), 1);
    assert_eq!(parsed.songs[0].id, "song_vid_1");
    assert_eq!(parsed.songs[0].artist, "The Weeknd");
    assert_eq!(parsed.songs[0].album, "Starboy");
    assert_eq!(parsed.songs[0].album_id.as_deref(), Some("MPREb_starboy"));
    assert_eq!(parsed.songs[0].duration_secs, Some(230));
    assert_eq!(parsed.artists.len(), 1);
    assert_eq!(parsed.artists[0].id, "UC0WP5P-ufpRfjbNrmOWwLBQ");
    assert_eq!(parsed.albums.len(), 1);
    assert_eq!(parsed.albums[0].id, "MPREb_afterhours");
    assert_eq!(parsed.dropped_rows, 1);

    let err = parse_rich_search_json(&json!({ "error": "schema drift" }))
        .expect_err("unrecognized search response must not silently succeed");
    assert!(matches!(err, CoreError::InvalidResponse { .. }));
}

#[test]
fn parses_artist_wide_banner_library_cards_and_suggestions() {
    let artist_payload = json!({
        "header": {
            "musicImmersiveHeaderRenderer": {
                "thumbnail": {
                    "musicThumbnailRenderer": {
                        "thumbnail": {
                            "thumbnails": [
                                { "url": "https://lh3.googleusercontent.com/sq=w544-h544-l90-rj", "width": 544, "height": 544 },
                                { "url": "https://lh3.googleusercontent.com/banner=w1200-h500-p-l90-rj", "width": 1200, "height": 500 }
                            ]
                        }
                    }
                }
            }
        }
    });
    assert_eq!(
        parse_artist_banner_url_json(&artist_payload).as_deref(),
        Some("https://lh3.googleusercontent.com/banner=w1600-h666-p-l90-rj")
    );

    let suggestions_payload = json!({
        "contents": [
            { "searchSuggestionRenderer": { "suggestion": { "runs": [{ "text": "after hours" }] } } },
            { "searchSuggestionRenderer": { "suggestion": { "runs": [{ "text": "after hours" }] } } },
            { "searchSuggestionRenderer": {} }
        ]
    });
    let sugg = parse_search_suggestions_json(&suggestions_payload);
    assert_eq!(sugg.items, vec!["after hours"]);
    assert_eq!(sugg.dropped_rows, 1);

    let cards_payload = json!([
        {
            "musicTwoRowItemRenderer": {
                "title": { "runs": [{ "text": "Dawn FM" }] },
                "subtitle": { "runs": [{ "text": "Album • 2022" }] },
                "navigationEndpoint": { "browseEndpoint": { "browseId": "MPREb_dawnfm" } }
            }
        },
        {
            "musicTwoRowItemRenderer": {
                "title": { "runs": [{ "text": "Broken Card" }] }
            }
        }
    ]);
    let cards = parse_cards_tree_json(&cards_payload, HomeCardKind::Album);
    assert_eq!(cards.items.len(), 1);
    assert_eq!(cards.items[0].kind, HomeCardKind::Album);
    assert_eq!(cards.dropped_rows, 1);
}
