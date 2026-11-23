//! Pure YouTube Music InnerTube album detail response parser.
//!
//! 1. **Model**: `AlbumDetail` holds release metadata, clickable artist credits, ordered tracks,
//!    alternate versions, and playlist save metadata.
//! 2. **Parser**: `parse_album_detail_json` validates the album header, parses tracks while
//!    reporting `dropped_rows`, and returns `CoreError::InvalidResponse` on unrecognized payloads.

use super::browse::{collect_runs_text, extract_best_thumbnail, parse_responsive_track};
use super::{parse_home_shelves_json, HomeCard};
use crate::error::CoreError;
use crate::providers::Track;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AlbumDetail {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub release_type: String,
    pub year: Option<String>,
    pub duration_text: Option<String>,
    pub cover_url: Option<String>,
    pub artists: Vec<(String, Option<String>)>,
    pub tracks: Vec<Track>,
    pub other_versions: Vec<HomeCard>,
    pub playlist_id: Option<String>,
    pub saved: bool,
}

/// Parsed album detail paired with the count of malformed track rows skipped during parsing.
#[derive(Debug, Clone)]
pub struct ParsedAlbumDetail {
    pub detail: AlbumDetail,
    pub dropped_rows: usize,
}

/// Parses a YouTube Music album browse response into an [`AlbumDetail`].
pub fn parse_album_detail_json(
    browse_id: &str,
    val: &Value,
) -> Result<ParsedAlbumDetail, CoreError> {
    let header = val
        .pointer("/header/musicResponsiveHeaderRenderer")
        .or_else(|| {
            val.pointer(
                "/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicResponsiveHeaderRenderer",
            )
        })
        .or_else(|| val.pointer("/header/musicDetailHeaderRenderer"))
        .ok_or_else(|| {
            CoreError::invalid_response(
                "ytmusic",
                format!("album `{browse_id}` response missing recognized header renderer"),
            )
        })?;

    let title = collect_runs_text(header.get("title")).unwrap_or_else(|| "Album".to_string());
    let subtitle = collect_runs_text(header.get("subtitle")).unwrap_or_default();
    let second_sub = collect_runs_text(header.get("secondSubtitle")).unwrap_or_default();

    let mut artists = Vec::new();
    if let Some(runs) = header
        .pointer("/straplineTextOne/runs")
        .and_then(Value::as_array)
    {
        for run in runs {
            let text = run.get("text").and_then(Value::as_str).unwrap_or("").trim();
            if text.is_empty() || text == "•" || text == "," || text == "&" {
                continue;
            }
            let bid = run
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
                .map(str::to_string);
            artists.push((text.to_string(), bid));
        }
    }
    if artists.is_empty() {
        if let Some(runs) = header.pointer("/subtitle/runs").and_then(Value::as_array) {
            for run in runs {
                if let Some(bid) = run
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                {
                    let name = run.get("text").and_then(Value::as_str).unwrap_or("").trim();
                    if !name.is_empty() {
                        artists.push((name.to_string(), Some(bid.to_string())));
                    }
                }
            }
        }
    }

    let sub_parts: Vec<&str> = subtitle
        .split('•')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let release_type = sub_parts.first().copied().unwrap_or("Album").to_string();
    let year = sub_parts
        .iter()
        .find(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_digit()))
        .map(|s| (*s).to_string());
    let duration_text = (!second_sub.is_empty()).then_some(second_sub);

    let cover_url = extract_best_thumbnail(
        header
            .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
            .or_else(|| {
                header.pointer("/thumbnail/croppedSquareThumbnailRenderer/thumbnail/thumbnails")
            }),
    );

    let playlist_id = find_first_string_at_pointer_name(val, "playlistId");

    let default_artist = if !artists.is_empty() {
        Some(
            artists
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
    } else {
        None
    };

    let mut tracks = Vec::new();
    let mut dropped_rows = 0usize;
    collect_tracks_recursive(
        val,
        &title,
        default_artist.as_deref(),
        &cover_url,
        browse_id,
        &mut tracks,
        &mut dropped_rows,
    );

    let shelves = parse_home_shelves_json(val);
    let other_versions = shelves
        .into_iter()
        .find(|s| s.title.to_ascii_lowercase().contains("other version"))
        .map(|s| s.cards)
        .unwrap_or_default();

    Ok(ParsedAlbumDetail {
        detail: AlbumDetail {
            id: browse_id.to_string(),
            title,
            subtitle,
            release_type,
            year,
            duration_text,
            cover_url,
            artists,
            tracks,
            other_versions,
            playlist_id,
            saved: false,
        },
        dropped_rows,
    })
}

fn find_first_string_at_pointer_name(val: &Value, key: &str) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(s) = map.get(key).and_then(Value::as_str) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            for v in map.values() {
                if let Some(found) = find_first_string_at_pointer_name(v, key) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr
            .iter()
            .find_map(|v| find_first_string_at_pointer_name(v, key)),
        _ => None,
    }
}

fn collect_tracks_recursive(
    val: &Value,
    default_album: &str,
    default_artist: Option<&str>,
    default_cover: &Option<String>,
    default_album_id: &str,
    out: &mut Vec<Track>,
    dropped_rows: &mut usize,
) {
    match val {
        Value::Object(map) => {
            if let Some(resp) = map.get("musicResponsiveListItemRenderer") {
                match parse_responsive_track(
                    resp,
                    Some(default_album),
                    default_artist,
                    default_cover.clone(),
                    Some(default_album_id),
                ) {
                    Some(t) => {
                        if !out.iter().any(|e| e.id == t.id) {
                            out.push(t);
                        }
                    }
                    None => *dropped_rows += 1,
                }
            }
            for v in map.values() {
                collect_tracks_recursive(
                    v,
                    default_album,
                    default_artist,
                    default_cover,
                    default_album_id,
                    out,
                    dropped_rows,
                );
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_tracks_recursive(
                    v,
                    default_album,
                    default_artist,
                    default_cover,
                    default_album_id,
                    out,
                    dropped_rows,
                );
            }
        }
        _ => {}
    }
}
