//! Keyless Spotify embed playlist parser and GraphQL track mapper for Sonata.
//!
//! 1. **ID Extraction**: `extract_playlist_id` normalizes Spotify playlist URLs, URIs, and bare IDs.
//! 2. **Embed HTML Import**: `parse_embed_playlist_html` extracts `__NEXT_DATA__` playlist tracks
//!    without authentication or API keys.
//! 3. **Track Mapping**: `parse_gql_track` converts Pathfinder GraphQL track payloads into `Track`.

use crate::providers::{ProviderSource, Track};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub fn extract_playlist_id(input: &str) -> Option<String> {
    static RE_ID: OnceLock<Regex> = OnceLock::new();
    let re = RE_ID.get_or_init(|| Regex::new(r"^[A-Za-z0-9]{22}$").unwrap());
    let trimmed = input.trim();
    if re.is_match(trimmed) {
        return Some(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        let id = rest.split(['?', '/']).next()?;
        if re.is_match(id) {
            return Some(id.to_string());
        }
    }
    if let Some(idx) = trimmed.find("/playlist/") {
        let after = &trimmed[idx + "/playlist/".len()..];
        let id = after.split(['?', '/', '&']).next()?;
        if re.is_match(id) {
            return Some(id.to_string());
        }
    }
    None
}

pub fn parse_embed_playlist_html(html: &str) -> Result<(String, Vec<Track>)> {
    static RE_NEXT: OnceLock<Regex> = OnceLock::new();
    let re = RE_NEXT.get_or_init(|| {
        Regex::new(r#"(?s)<script id="__NEXT_DATA__" type="application/json">(.*?)</script>"#)
            .unwrap()
    });
    let caps = re
        .captures(html)
        .ok_or_else(|| anyhow!("Could not find Spotify embed playlist data"))?;
    let json_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let root: Value = serde_json::from_str(json_str)?;
    let entity = root
        .pointer("/props/pageProps/state/data/entity")
        .ok_or_else(|| anyhow!("Missing playlist entity in Spotify embed"))?;

    let playlist_name = entity
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Spotify Playlist")
        .to_string();

    let default_cover = entity
        .pointer("/coverArt/sources")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(str::to_string);

    let mut tracks = Vec::new();
    if let Some(list) = entity.get("trackList").and_then(|v| v.as_array()) {
        for item in list {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            let artist = item
                .get("subtitle")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Artist")
                .trim()
                .to_string();
            let duration_secs = item
                .get("duration")
                .and_then(|v| v.as_u64())
                .map(|ms| ms / 1000);
            let uri = item
                .get("uri")
                .and_then(|v| v.as_str())
                .unwrap_or(&title)
                .to_string();
            tracks.push(Track {
                id: uri,
                title,
                artist,
                album: playlist_name.clone(),
                duration_secs,
                thumbnail_url: default_cover.clone(),
                source: ProviderSource::Spotify,
                audio_badge: None,
                set_video_id: None,
                liked: false,
                album_id: None,
                is_explicit: false,
            });
        }
    }

    Ok((playlist_name, tracks))
}

pub(super) fn parse_gql_track(data: Option<&Value>) -> Option<Track> {
    let d = data?;
    let title = d.get("name").and_then(|v| v.as_str())?.to_string();
    let uri = d
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or(&title)
        .to_string();

    let artists = d
        .pointer("/artists/items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.pointer("/profile/name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = d
        .pointer("/albumOfTrack/name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let duration_secs = d
        .pointer("/duration/totalMilliseconds")
        .or_else(|| d.pointer("/trackDuration/totalMilliseconds"))
        .and_then(|v| v.as_u64())
        .map(|ms| ms / 1000);

    let thumb = d
        .pointer("/albumOfTrack/coverArt/sources")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .max_by_key(|s| s.get("width").and_then(|w| w.as_u64()).unwrap_or(0))
        })
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Some(Track {
        id: uri,
        title,
        artist: artists,
        album,
        duration_secs,
        thumbnail_url: thumb,
        source: ProviderSource::Spotify,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_spotify_playlist_ids() {
        let id = "37i9dQZF1DXcBWIGoYBM5M";
        assert_eq!(
            extract_playlist_id(&format!("https://open.spotify.com/playlist/{id}?si=123")),
            Some(id.to_string())
        );
        assert_eq!(
            extract_playlist_id(&format!("https://open.spotify.com/intl-tr/playlist/{id}")),
            Some(id.to_string())
        );
        assert_eq!(
            extract_playlist_id(&format!("spotify:playlist:{id}")),
            Some(id.to_string())
        );
        assert_eq!(extract_playlist_id(id), Some(id.to_string()));
        assert_eq!(extract_playlist_id("invalid"), None);
    }

    #[test]
    fn parses_embed_html_playlist() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Chill Vibes","coverArt":{"sources":[{"url":"https://i.scdn.co/image/cover640"}]},"trackList":[{"uri":"spotify:track:1","title":"Midnight City","subtitle":"M83","duration":243000}]}}}}}}</script></html>"#;
        let (name, tracks) = parse_embed_playlist_html(html).expect("valid embed");
        assert_eq!(name, "Chill Vibes");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Midnight City");
        assert_eq!(tracks[0].artist, "M83");
        assert_eq!(tracks[0].duration_secs, Some(243));
    }

    #[test]
    fn rejects_missing_malformed_and_incomplete_embed_data() {
        assert!(parse_embed_playlist_html("<html></html>").is_err());
        assert!(parse_embed_playlist_html(
            r#"<script id="__NEXT_DATA__" type="application/json">{broken}</script>"#
        )
        .is_err());
        assert!(parse_embed_playlist_html(
            r#"<script id="__NEXT_DATA__" type="application/json">{}</script>"#
        )
        .is_err());
    }
}
