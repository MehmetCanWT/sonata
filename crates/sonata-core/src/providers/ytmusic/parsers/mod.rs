//! Tolerant JSON-tree parsers for YouTube Music InnerTube responses.
//!
//! 1. **Playlists & Tracks**: extracts `Playlist` and `Track` items along with continuation
//!    tokens and `setVideoId` metadata from responsive/two-row renderers.
//! 2. **Account & Headers**: parses `YtAccountProfile`, header thumbnails, descriptions, and
//!    collection header backfill metadata.
//! 3. **Submodules**: delegates Home feed parsing (`home`), Artist & search browse-ID parsing
//!    (`artist`), and Music-Video counterpart/sync-segment parsing (`video_sync`).

mod artist;
mod home;
mod video_sync;

pub use artist::parse_artist_page_json;
pub(super) use artist::{
    extract_search_album_browse_id, extract_search_artist_browse_id, find_artist_songs_endpoint,
};
pub use home::parse_home_shelves_json;
pub(super) use home::{is_excluded_shelf_title, merge_home_shelves};
pub use video_sync::{
    build_sync_segments_from_offtopic, parse_music_video_counterpart_json, MusicVideoCandidate,
    MusicVideoSyncSegment,
};

use super::{upscale_thumbnail, YtAccountProfile};
use crate::providers::{parse_duration_text, Playlist, ProviderSource, Track};
use home::parse_home_two_row;
use serde_json::Value;

pub(super) fn collect_playlists(val: &Value, out: &mut Vec<Playlist>) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("musicTwoRowItemRenderer") {
                if let Some(pl) = parse_two_row_playlist(renderer) {
                    out.push(pl);
                }
            }
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                if let Some(pl) = parse_responsive_playlist(renderer) {
                    out.push(pl);
                }
            }
            for v in map.values() {
                collect_playlists(v, out);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_playlists(item, out);
            }
        }
        _ => {}
    }
}

fn normalize_playlist_browse_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed == "VLLM"
        || trimmed == "VLSE"
        || trimmed == "LM"
        || trimmed == "SE"
    {
        return None;
    }
    if trimmed.starts_with("VL") {
        return Some(trimmed.to_string());
    }
    if trimmed.starts_with("PL")
        || trimmed.starts_with("OLAK")
        || trimmed.starts_with("RDCLAK")
        || trimmed.starts_with("RDAMPL")
    {
        return Some(format!("VL{trimmed}"));
    }
    None
}

fn value_contains_any_key(val: &Value, keys: &[&str]) -> bool {
    match val {
        Value::Object(map) => {
            if map.keys().any(|k| keys.iter().any(|target| k == *target)) {
                return true;
            }
            map.values().any(|v| value_contains_any_key(v, keys))
        }
        Value::Array(arr) => arr.iter().any(|v| value_contains_any_key(v, keys)),
        _ => false,
    }
}

fn value_has_playlist_like_menu(val: &Value) -> bool {
    match val {
        Value::Object(map) => {
            if let Some(like_ep) = map.get("likeEndpoint") {
                if like_ep
                    .pointer("/target/playlistId")
                    .and_then(|v| v.as_str())
                    .is_some()
                {
                    return true;
                }
            }
            if let Some(toggle_ep) = map.get("toggleMenuServiceItemRenderer") {
                if value_has_playlist_like_menu(toggle_ep) {
                    return true;
                }
            }
            map.values().any(value_has_playlist_like_menu)
        }
        Value::Array(arr) => arr.iter().any(value_has_playlist_like_menu),
        _ => false,
    }
}

fn infer_playlist_ownership(renderer: &Value, browse_id: &str) -> bool {
    let bare = browse_id
        .trim()
        .strip_prefix("VL")
        .unwrap_or(browse_id.trim());
    if !bare.starts_with("PL") {
        return false;
    }
    if let Some(menu) = renderer.get("menu") {
        if value_contains_any_key(
            menu,
            &[
                "deletePlaylistServiceEndpoint",
                "playlistEditorEndpoint",
                "playlistEditEndpoint",
                "ACTION_DELETE_PLAYLIST",
            ],
        ) {
            return true;
        }
        if value_has_playlist_like_menu(menu) {
            return false;
        }
    }
    true
}

fn parse_two_row_playlist(r: &Value) -> Option<Playlist> {
    let raw_id = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| r.pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        .and_then(|v| v.as_str())?;
    let browse_id = normalize_playlist_browse_id(raw_id)?;
    let name = extract_runs_text(r.get("title")?)?;
    let thumb = extract_last_thumbnail(r.get("thumbnailRenderer"));
    let is_owned = infer_playlist_ownership(r, &browse_id);
    Some(Playlist {
        id: browse_id,
        name,
        source: ProviderSource::YouTubeMusic,
        track_count: None,
        thumbnail_url: thumb,
        is_owned,
    })
}

fn parse_responsive_playlist(r: &Value) -> Option<Playlist> {
    let raw_id = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| {
            r.pointer(
                "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId",
            )
        })
        .and_then(|v| v.as_str())?;
    let browse_id = normalize_playlist_browse_id(raw_id)?;
    let name = r
        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")
        .and_then(extract_runs_text)?;
    let thumb = extract_last_thumbnail(r.get("thumbnail"));
    let is_owned = infer_playlist_ownership(r, &browse_id);
    Some(Playlist {
        id: browse_id,
        name,
        source: ProviderSource::YouTubeMusic,
        track_count: None,
        thumbnail_url: thumb,
        is_owned,
    })
}

pub(super) fn collect_tracks(val: &Value, out: &mut Vec<Track>) {
    match val {
        Value::Object(map) => {
            // Unwrap playlistPanelVideoWrapperRenderer using ONLY primaryRenderer so OMV
            // counterparts never duplicate or inject music videos alongside official audio tracks.
            if let Some(wrapper) = map.get("playlistPanelVideoWrapperRenderer") {
                if let Some(primary) =
                    wrapper.pointer("/primaryRenderer/playlistPanelVideoRenderer")
                {
                    if let Some(track) = parse_panel_video_track(primary) {
                        out.push(track);
                    }
                }
                return;
            }
            // Extract Top Result card without descending into "More from YouTube" sub-items.
            if let Some(card) = map.get("musicCardShelfRenderer") {
                if let Some(track) = super::search::parse_card_shelf_track(card) {
                    out.push(track);
                }
                return;
            }
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                if let Some(track) = parse_responsive_track(renderer) {
                    out.push(track);
                }
            }
            if let Some(renderer) = map.get("playlistPanelVideoRenderer") {
                if let Some(track) = parse_panel_video_track(renderer) {
                    out.push(track);
                }
            }
            for v in map.values() {
                collect_tracks(v, out);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_tracks(item, out);
            }
        }
        _ => {}
    }
}

fn parse_panel_video_track(r: &Value) -> Option<Track> {
    let video_id = r.get("videoId").and_then(|v| v.as_str())?.to_string();
    let title = r.get("title").and_then(extract_runs_text)?;

    let mut artist = String::new();
    let mut album = String::new();
    let mut album_id: Option<String> = None;
    if let Some(runs) = r
        .pointer("/longBylineText/runs")
        .or_else(|| r.pointer("/shortBylineText/runs"))
        .and_then(|v| v.as_array())
    {
        let mut parts: Vec<String> = Vec::new();
        for run in runs {
            if let Some(t) = run.get("text").and_then(|v| v.as_str()) {
                let t = t.trim();
                if t.is_empty() || t == "•" || t == "," || t == "&" {
                    continue;
                }
                let run_browse_id = run
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(|v| v.as_str());
                if let Some(page_type) = run.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(|v| v.as_str()) {
                    if page_type == "MUSIC_PAGE_TYPE_ALBUM" {
                        album = t.to_string();
                        if let Some(bid) = run_browse_id.filter(|id| id.starts_with("MPRE") || id.starts_with("OLAK")) {
                            album_id = Some(bid.to_string());
                        }
                    } else if artist.is_empty() {
                        artist = t.to_string();
                    } else {
                        artist = format!("{artist}, {t}");
                    }
                } else if let Some(bid) = run_browse_id.filter(|id| id.starts_with("MPRE") || id.starts_with("OLAK")) {
                    if album.is_empty() {
                        album = t.to_string();
                    }
                    album_id = Some(bid.to_string());
                } else {
                    parts.push(t.to_string());
                }
            }
        }
        if artist.is_empty() && !parts.is_empty() {
            artist = parts[0].clone();
            if album.is_empty() && parts.len() > 1 {
                album = parts[1].clone();
            }
        }
    }

    let duration_secs = r
        .get("lengthText")
        .and_then(extract_runs_text)
        .and_then(|t| parse_duration_text(&t));

    let thumb = r
        .pointer("/thumbnail/thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|v| v.as_str())
        .map(upscale_thumbnail);
    let is_explicit = has_explicit_badge(r);

    Some(Track {
        id: video_id,
        title,
        artist: if artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            artist
        },
        album,
        duration_secs,
        thumbnail_url: thumb,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit,
    })
}

fn parse_responsive_track(r: &Value) -> Option<Track> {
    let video_id = r
        .pointer("/playlistItemData/videoId")
        .or_else(|| r.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId"))
        .or_else(|| r.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(|v| v.as_str())?
        .to_string();

    let flex = r.get("flexColumns")?.as_array()?;
    let title = flex
        .first()
        .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
        .and_then(extract_runs_text)?;

    let mut artist = String::new();
    let mut album = String::new();
    let mut album_id: Option<String> = None;
    let mut duration_secs = None;

    if let Some(runs) = flex
        .get(1)
        .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs"))
        .and_then(|v| v.as_array())
    {
        let mut parts: Vec<String> = Vec::new();
        for run in runs {
            if let Some(t) = run.get("text").and_then(|v| v.as_str()) {
                let t = t.trim();
                if t.is_empty() || t == "•" || t == "," || t == "&" {
                    continue;
                }
                let run_browse_id = run
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(|v| v.as_str());
                if let Some(dur) = parse_duration_text(t) {
                    duration_secs = Some(dur);
                } else if let Some(page_type) = run.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(|v| v.as_str()) {
                    if page_type == "MUSIC_PAGE_TYPE_ALBUM" {
                        album = t.to_string();
                        if let Some(bid) = run_browse_id.filter(|id| id.starts_with("MPRE") || id.starts_with("OLAK")) {
                            album_id = Some(bid.to_string());
                        }
                    } else if artist.is_empty() {
                        artist = t.to_string();
                    } else {
                        artist = format!("{artist}, {t}");
                    }
                } else if let Some(bid) = run_browse_id.filter(|id| id.starts_with("MPRE") || id.starts_with("OLAK")) {
                    if album.is_empty() {
                        album = t.to_string();
                    }
                    album_id = Some(bid.to_string());
                } else {
                    parts.push(t.to_string());
                }
            }
        }
        if artist.is_empty() && !parts.is_empty() {
            // Filter out localized category/stats badges like "Song", "Şarkı", "Песня", "1M plays".
            let filtered: Vec<_> = parts
                .into_iter()
                .filter(|p| !super::browse::is_metadata_noise_token(p))
                .collect();
            if let Some(a) = filtered.first() {
                artist = a.clone();
            }
            if album.is_empty() && filtered.len() > 1 {
                album = filtered[1].clone();
            }
        }
    }

    // Also inspect extra flexColumns (e.g. search/album rows put album link in flexColumns[2]).
    if album_id.is_none() {
        for col in flex.iter().skip(2) {
            if let Some(runs) = col
                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(|v| v.as_array())
            {
                for run in runs {
                    if let Some(bid) = run
                        .pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(|v| v.as_str())
                        .filter(|id| id.starts_with("MPRE") || id.starts_with("OLAK"))
                    {
                        album_id = Some(bid.to_string());
                        if album.is_empty() {
                            if let Some(t) = run.get("text").and_then(|v| v.as_str()) {
                                album = t.trim().to_string();
                            }
                        }
                        break;
                    }
                }
            }
            if album_id.is_some() {
                break;
            }
        }
    }

    if let Some(fixed) = r.get("fixedColumns").and_then(|v| v.as_array()) {
        for col in fixed {
            if let Some(t) = col
                .pointer("/musicResponsiveListItemFixedColumnRenderer/text")
                .and_then(extract_runs_text)
            {
                if let Some(dur) = parse_duration_text(&t) {
                    duration_secs = Some(dur);
                }
            }
        }
    }

    let thumb = extract_last_thumbnail(r.get("thumbnail"));
    let set_video_id = r
        .pointer("/playlistItemData/playlistSetVideoId")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let liked = has_like_status(r);
    let is_explicit = has_explicit_badge(r);

    Some(Track {
        id: video_id,
        title,
        artist: if artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            artist
        },
        album,
        duration_secs,
        thumbnail_url: thumb,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id,
        liked,
        album_id,
        is_explicit,
    })
}

fn has_explicit_badge(r: &Value) -> bool {
    fn contains_explicit(val: &Value) -> bool {
        match val {
            Value::Object(map) => {
                if map.get("iconType").and_then(|v| v.as_str()) == Some("MUSIC_EXPLICIT_BADGE") {
                    return true;
                }
                map.values().any(contains_explicit)
            }
            Value::Array(arr) => arr.iter().any(contains_explicit),
            _ => false,
        }
    }

    r.get("badges").is_some_and(contains_explicit)
        || r.get("subtitleBadges").is_some_and(contains_explicit)
}

fn has_like_status(val: &Value) -> bool {
    match val {
        Value::Object(map) => {
            if map.get("likeStatus").and_then(|v| v.as_str()) == Some("LIKE") {
                return true;
            }
            map.values().any(has_like_status)
        }
        Value::Array(arr) => arr.iter().any(has_like_status),
        _ => false,
    }
}

fn extract_runs_text(val: &Value) -> Option<String> {
    if let Some(simple) = val.get("simpleText").and_then(|v| v.as_str()) {
        return Some(simple.to_string());
    }
    let runs = val.get("runs")?.as_array()?;
    let joined: String = runs
        .iter()
        .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
        .collect();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn extract_last_thumbnail(val: Option<&Value>) -> Option<String> {
    let v = val?;
    let thumbs = v
        .pointer("/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| v.pointer("/croppedSquareThumbnailRenderer/thumbnail/thumbnails"))
        .and_then(|t| t.as_array())?;
    let raw = thumbs.last()?.get("url")?.as_str()?;
    Some(upscale_thumbnail(raw))
}

fn extract_continuation_token(renderer: &Value) -> Option<String> {
    let endpoint = renderer.get("continuationEndpoint")?;
    if let Some(tok) = endpoint
        .pointer("/continuationCommand/token")
        .and_then(|v| v.as_str())
    {
        return Some(tok.to_string());
    }
    if let Some(commands) = endpoint
        .pointer("/commandExecutorCommand/commands")
        .and_then(|v| v.as_array())
    {
        for cmd in commands {
            if let Some(tok) = cmd
                .pointer("/continuationCommand/token")
                .and_then(|v| v.as_str())
            {
                return Some(tok.to_string());
            }
        }
    }
    None
}

pub(super) fn find_shelf_continuation(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            for shelf_key in [
                "musicPlaylistShelfRenderer",
                "musicShelfRenderer",
                "musicPlaylistShelfContinuation",
                "musicShelfContinuation",
                "gridRenderer",
                "gridContinuation",
                "playlistPanelRenderer",
                "playlistPanelContinuation",
            ] {
                if let Some(shelf) = map.get(shelf_key) {
                    if let Some(tok) = shelf
                        .pointer("/continuations/0/nextRadioContinuationData/continuation")
                        .or_else(|| {
                            shelf.pointer("/continuations/0/nextContinuationData/continuation")
                        })
                        .and_then(|v| v.as_str())
                    {
                        return Some(tok.to_string());
                    }
                }
            }
            if let Some(tok) = map
                .get("continuationItemRenderer")
                .and_then(extract_continuation_token)
            {
                return Some(tok);
            }
            for v in map.values() {
                if let Some(found) = find_shelf_continuation(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_shelf_continuation),
        _ => None,
    }
}

pub(super) fn find_radio_continuation(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            for key in ["playlistPanelRenderer", "playlistPanelContinuation"] {
                if let Some(panel) = map.get(key) {
                    if let Some(tok) = panel
                        .pointer("/continuations/0/nextRadioContinuationData/continuation")
                        .or_else(|| {
                            panel.pointer("/continuations/0/nextContinuationData/continuation")
                        })
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        return Some(tok.to_string());
                    }
                }
            }
            for v in map.values() {
                if let Some(found) = find_radio_continuation(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_radio_continuation),
        _ => None,
    }
}

pub(super) fn find_home_continuation(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            for key in ["sectionListRenderer", "sectionListContinuation"] {
                if let Some(section) = map.get(key) {
                    if let Some(tok) = section
                        .pointer("/continuations/0/nextContinuationData/continuation")
                        .and_then(|v| v.as_str())
                    {
                        return Some(tok.to_string());
                    }
                }
            }
            if let Some(tok) = map
                .get("continuationItemRenderer")
                .and_then(extract_continuation_token)
            {
                return Some(tok);
            }
            for v in map.values() {
                if let Some(found) = find_home_continuation(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(find_home_continuation),
        _ => None,
    }
}

pub(super) fn find_mplyt_browse_id(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(bid) = map.get("browseId").and_then(|v| v.as_str()) {
                if bid.starts_with("MPLYt") {
                    return Some(bid.to_string());
                }
            }
            map.values().find_map(find_mplyt_browse_id)
        }
        Value::Array(arr) => arr.iter().find_map(find_mplyt_browse_id),
        _ => None,
    }
}

pub(super) fn find_description_text(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(shelf) = map.get("musicDescriptionShelfRenderer") {
                if let Some(desc) = shelf.get("description").and_then(extract_runs_text) {
                    if !desc.trim().is_empty() {
                        return Some(desc);
                    }
                }
            }
            map.values().find_map(find_description_text)
        }
        Value::Array(arr) => arr.iter().find_map(find_description_text),
        _ => None,
    }
}

pub(super) fn find_description_subheader(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(shelf) = map.get("musicDescriptionShelfRenderer") {
                if let Some(sub) = shelf.get("subheader").and_then(extract_runs_text) {
                    if !sub.trim().is_empty() {
                        return Some(sub.trim().to_string());
                    }
                }
            }
            map.values().find_map(find_description_subheader)
        }
        Value::Array(arr) => arr.iter().find_map(find_description_subheader),
        _ => None,
    }
}

fn upscale_hero_thumbnail(url: String) -> String {
    if url.contains("googleusercontent.com") && url.contains("=w800-h800-l90-rj") {
        url.replace("=w800-h800-l90-rj", "=w1200-h1200-l90-rj")
    } else {
        url
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectionHeader {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub cover_url: Option<String>,
}

fn upscale_collection_cover(url: String) -> String {
    if url.contains("googleusercontent.com") && url.contains("=w800-h800-l90-rj") {
        url.replace("=w800-h800-l90-rj", "=w544-h544-l90-rj")
    } else {
        url
    }
}

fn extract_artist_from_collection_node(node: &Value) -> Option<String> {
    if let Some(runs) = node
        .pointer("/straplineTextOne/runs")
        .and_then(|v| v.as_array())
    {
        let mut names: Vec<String> = Vec::new();
        for run in runs {
            let Some(t) = run.get("text").and_then(|v| v.as_str()).map(str::trim) else {
                continue;
            };
            if t.is_empty() || matches!(t, "•" | "," | "&" | "and") {
                continue;
            }
            if !names.iter().any(|n| n.eq_ignore_ascii_case(t)) {
                names.push(t.to_string());
            }
        }
        if !names.is_empty() {
            return Some(names.join(", "));
        }
    }

    if let Some(runs) = node.pointer("/subtitle/runs").and_then(|v| v.as_array()) {
        let mut candidates: Vec<String> = Vec::new();
        for run in runs {
            let Some(t) = run.get("text").and_then(|v| v.as_str()).map(str::trim) else {
                continue;
            };
            if t.is_empty() || matches!(t, "•" | "," | "&") {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "album" | "single" | "ep" | "playlist" | "compilation" | "audiobook"
            ) {
                continue;
            }
            if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            // Prefer runs linked to an artist channel or fallback non-metadata text runs.
            let is_artist_link = run
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.starts_with("UC"));
            if is_artist_link
                || (candidates.is_empty()
                    && !lower.contains("song")
                    && !lower.contains("track")
                    && !lower.contains("minute")
                    && !lower.contains("hour")
                    && !lower.contains("view"))
            {
                candidates.push(t.to_string());
            }
        }
        if !candidates.is_empty() {
            return Some(candidates.join(", "));
        }
    }
    None
}

pub(super) fn find_collection_header(val: &Value) -> CollectionHeader {
    let mut node: Option<&Value> = None;
    for key in [
        "musicResponsiveHeaderRenderer",
        "musicDetailHeaderRenderer",
        "musicEditablePlaylistDetailHeaderRenderer",
    ] {
        if let Some(found) = val
            .pointer(&format!("/header/{key}"))
            .or_else(|| find_key_object(val, key))
        {
            node = Some(found);
            break;
        }
    }
    let Some(raw_node) = node else {
        return CollectionHeader {
            title: None,
            artist: None,
            cover_url: find_header_thumbnail(val).map(upscale_collection_cover),
        };
    };

    // `musicEditablePlaylistDetailHeaderRenderer` wraps the real header inside `/header/*`.
    let resolved_node = raw_node
        .pointer("/header/musicDetailHeaderRenderer")
        .or_else(|| raw_node.pointer("/header/musicResponsiveHeaderRenderer"))
        .unwrap_or(raw_node);

    let title = resolved_node
        .get("title")
        .and_then(extract_runs_text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let artist = extract_artist_from_collection_node(resolved_node);
    let cover_url = extract_last_thumbnail(resolved_node.get("thumbnail"))
        .or_else(|| extract_last_thumbnail(resolved_node.get("foregroundThumbnail")))
        .or_else(|| find_header_thumbnail(val))
        .map(upscale_collection_cover);

    CollectionHeader {
        title,
        artist,
        cover_url,
    }
}

pub(super) fn apply_collection_backfill(
    tracks: &mut [Track],
    browse_id: &str,
    header: &CollectionHeader,
) {
    let resolved_album_id = if browse_id.starts_with("MPRE")
        || browse_id.starts_with("OLAK")
        || browse_id.starts_with("VLOLAK")
    {
        Some(
            browse_id
                .strip_prefix("VL")
                .unwrap_or(browse_id)
                .to_string(),
        )
    } else {
        None
    };

    for t in tracks {
        if browse_id == "VLLM" {
            t.liked = true;
        }
        if (t.artist.trim().is_empty() || t.artist.eq_ignore_ascii_case("Unknown Artist"))
            && header.artist.as_deref().is_some_and(|a| !a.is_empty())
        {
            t.artist = header.artist.clone().unwrap_or_default();
        }
        if t.thumbnail_url
            .as_deref()
            .is_none_or(|u| u.trim().is_empty())
        {
            t.thumbnail_url = header.cover_url.clone();
        }
        if t.album.trim().is_empty() {
            if let Some(title) = header.title.as_ref() {
                t.album = title.clone();
            }
        }
        if t.album_id.is_none() {
            t.album_id = resolved_album_id.clone();
        }
    }
}

pub(super) fn find_header_thumbnail(val: &Value) -> Option<String> {
    if let Some(hdr) = val.get("header") {
        for key in [
            "musicImmersiveHeaderRenderer",
            "musicVisualHeaderRenderer",
            "musicResponsiveHeaderRenderer",
            "musicDetailHeaderRenderer",
        ] {
            if let Some(node) = hdr.get(key) {
                if let Some(t) = extract_last_thumbnail(node.get("foregroundThumbnail"))
                    .or_else(|| extract_last_thumbnail(node.get("thumbnail")))
                {
                    return Some(upscale_hero_thumbnail(t));
                }
            }
        }
        if let Some(t) = extract_last_thumbnail(Some(hdr)) {
            return Some(upscale_hero_thumbnail(t));
        }
    }
    for key in [
        "musicResponsiveHeaderRenderer",
        "musicDetailHeaderRenderer",
        "musicImmersiveHeaderRenderer",
        "musicVisualHeaderRenderer",
    ] {
        if let Some(node) = find_key_object(val, key) {
            if let Some(t) = extract_last_thumbnail(node.get("foregroundThumbnail"))
                .or_else(|| extract_last_thumbnail(node.get("thumbnail")))
            {
                return Some(upscale_hero_thumbnail(t));
            }
        }
    }
    None
}

fn find_key_object<'a>(val: &'a Value, target: &str) -> Option<&'a Value> {
    match val {
        Value::Object(map) => {
            if let Some(found) = map.get(target) {
                if found.is_object() {
                    return Some(found);
                }
            }
            for v in map.values() {
                if let Some(found) = find_key_object(v, target) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr.iter().find_map(|v| find_key_object(v, target)),
        _ => None,
    }
}

pub fn parse_account_profile_json(val: &Value) -> Option<YtAccountProfile> {
    let header = find_key_object(val, "activeAccountHeaderRenderer")?;
    let name = header
        .get("accountName")
        .and_then(extract_runs_text)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "YouTube Music".to_string());
    let email = header
        .get("email")
        .and_then(extract_runs_text)
        .filter(|s| !s.trim().is_empty());
    let handle = header
        .get("channelHandle")
        .and_then(extract_runs_text)
        .filter(|s| !s.trim().is_empty());
    let avatar_url = header
        .pointer("/accountPhoto/thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|v| v.as_str())
        .map(upscale_thumbnail);

    Some(YtAccountProfile {
        name,
        email,
        handle,
        avatar_url,
    })
}

/// Extracts an automix watch-playlist endpoint (`playlistId`, optional `params`) from an
/// InnerTube `next` response (`automixPreviewVideoRenderer`).
pub(super) fn find_automix_endpoint(val: &Value) -> Option<(String, Option<String>)> {
    match val {
        Value::Object(map) => {
            if let Some(automix) = map.get("automixPreviewVideoRenderer") {
                if let Some(ep) = automix.pointer(
                    "/content/automixPlaylistVideoRenderer/navigationEndpoint/watchPlaylistEndpoint",
                ) {
                    if let Some(pid) = ep
                        .get("playlistId")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        let params = ep
                            .get("params")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(ToString::to_string);
                        return Some((pid.to_string(), params));
                    }
                }
            }
            map.values().find_map(find_automix_endpoint)
        }
        Value::Array(arr) => arr.iter().find_map(find_automix_endpoint),
        _ => None,
    }
}

pub(crate) fn collect_history_feedback_tokens(
    val: &Value,
    out: &mut std::collections::HashMap<String, Vec<String>>,
) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                if let Some(video_id) = renderer
                    .pointer("/playlistItemData/videoId")
                    .or_else(|| renderer.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId"))
                    .or_else(|| renderer.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    if let Some(items) = renderer
                        .pointer("/menu/menuRenderer/items")
                        .and_then(|v| v.as_array())
                    {
                        for item in items {
                            let (entry, token_ptr, icon_ptr, text_ptr) =
                                if let Some(s) = item.get("menuServiceItemRenderer") {
                                    (
                                        s,
                                        "/serviceEndpoint/feedbackEndpoint/feedbackToken",
                                        "/icon/iconType",
                                        "/text",
                                    )
                                } else if let Some(t) = item.get("toggleMenuServiceItemRenderer") {
                                    (
                                        t,
                                        "/defaultServiceEndpoint/feedbackEndpoint/feedbackToken",
                                        "/defaultIcon/iconType",
                                        "/defaultText",
                                    )
                                } else {
                                    continue;
                                };
                            let Some(token) = entry
                                .pointer(token_ptr)
                                .and_then(|v| v.as_str())
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            else {
                                continue;
                            };
                            let icon_type = entry
                                .pointer(icon_ptr)
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let label = entry
                                .pointer(text_ptr)
                                .and_then(extract_runs_text)
                                .unwrap_or_default()
                                .to_lowercase();
                            let matches_icon = matches!(
                                icon_type,
                                "DELETE" | "REMOVE_FROM_HISTORY" | "DELETE_SWEEP" | "REMOVE"
                            );
                            let matches_label = label.contains("history")
                                || label.contains("geçmiş")
                                || label.contains("gecmis")
                                || label.contains("истори");
                            if matches_icon || matches_label {
                                let bucket = out.entry(video_id.to_string()).or_default();
                                if !bucket.iter().any(|existing| existing == token) {
                                    bucket.push(token.to_string());
                                }
                            }
                        }
                    }
                }
            }
            for v in map.values() {
                collect_history_feedback_tokens(v, out);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_history_feedback_tokens(item, out);
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
    fn parses_account_profile_from_account_menu() {
        let sample = json!({
            "actions": [{
                "openPopupAction": {
                    "popup": {
                        "multiPageMenuRenderer": {
                            "header": {
                                "activeAccountHeaderRenderer": {
                                    "accountName": { "runs": [{ "text": "Mehmet" }] },
                                    "email": { "runs": [{ "text": "mehmet@example.com" }] },
                                    "channelHandle": { "runs": [{ "text": "@mehmet" }] },
                                    "accountPhoto": {
                                        "thumbnails": [
                                            { "url": "https://lh3.googleusercontent.com/a/abc=s88-c-k", "width": 88, "height": 88 }
                                        ]
                                    }
                                }
                            }
                        }
                    }
                }
            }]
        });

        let profile = parse_account_profile_json(&sample).expect("account profile");
        assert_eq!(profile.name, "Mehmet");
        assert_eq!(profile.email.as_deref(), Some("mehmet@example.com"));
        assert_eq!(profile.handle.as_deref(), Some("@mehmet"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://lh3.googleusercontent.com/a/abc=w800-h800-l90-rj")
        );
    }

    #[test]
    fn collects_tracks_from_playlist_panel_video_renderer() {
        let next_response = json!({
            "contents": {
                "singleColumnMusicWatchNextResultsRenderer": {
                    "playlist": {
                        "playlistPanelRenderer": {
                            "contents": [{
                                "playlistPanelVideoRenderer": {
                                    "videoId": "mix_vid_1",
                                    "title": { "runs": [{ "text": "Nightcall" }] },
                                    "longBylineText": { "runs": [{ "text": "Kavinsky" }, { "text": " • " }, { "text": "OutRun" }] },
                                    "lengthText": { "runs": [{ "text": "4:18" }] },
                                    "thumbnail": { "thumbnails": [{ "url": "https://img.example/nightcall.jpg" }] }
                                }
                            }]
                        }
                    }
                }
            }
        });

        let mut tracks = Vec::new();
        collect_tracks(&next_response, &mut tracks);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "mix_vid_1");
        assert_eq!(tracks[0].title, "Nightcall");
        assert_eq!(tracks[0].artist, "Kavinsky");
        assert_eq!(tracks[0].album, "OutRun");
        assert_eq!(tracks[0].duration_secs, Some(258));
    }

    #[test]
    fn collects_playlists_from_both_grid_and_list_renderers_and_command_executor_continuations() {
        let sample = json!({
            "contents": [
                {
                    "musicTwoRowItemRenderer": {
                        "title": { "runs": [{ "text": "Grid Mix" }] },
                        "navigationEndpoint": { "browseEndpoint": { "browseId": "VLPLgrid123" } }
                    }
                },
                {
                    "musicResponsiveListItemRenderer": {
                        "flexColumns": [{
                            "musicResponsiveListItemFlexColumnRenderer": {
                                "text": {
                                    "runs": [{
                                        "text": "List Playlist",
                                        "navigationEndpoint": { "browseEndpoint": { "browseId": "PLlist456" } }
                                    }]
                                }
                            }
                        }]
                    }
                },
                {
                    "continuationItemRenderer": {
                        "continuationEndpoint": {
                            "commandExecutorCommand": {
                                "commands": [
                                    { "continuationCommand": { "token": "EXEC_CONT_TOKEN" } }
                                ]
                            }
                        }
                    }
                }
            ]
        });

        let mut playlists = Vec::new();
        collect_playlists(&sample, &mut playlists);
        assert_eq!(playlists.len(), 2);
        assert_eq!(playlists[0].id, "VLPLgrid123");
        assert_eq!(playlists[0].name, "Grid Mix");
        assert_eq!(playlists[1].id, "VLPLlist456");
        assert_eq!(playlists[1].name, "List Playlist");
        assert_eq!(
            find_shelf_continuation(&sample).as_deref(),
            Some("EXEC_CONT_TOKEN")
        );
    }

    #[test]
    fn parses_explicit_badge_and_album_id_on_tracks() {
        let track_json = json!({
            "contents": [{
                "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "3_g2un5M350" },
                    "badges": [{
                        "musicInlineBadgeRenderer": {
                            "icon": { "iconType": "MUSIC_EXPLICIT_BADGE" }
                        }
                    }],
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
                                        { "text": "The Weeknd" },
                                        { "text": " • " },
                                        {
                                            "text": "Starboy",
                                            "navigationEndpoint": {
                                                "browseEndpoint": {
                                                    "browseId": "MPREb_starboy123",
                                                    "browseEndpointContextSupportedConfigs": {
                                                        "browseEndpointContextMusicConfig": {
                                                            "pageType": "MUSIC_PAGE_TYPE_ALBUM"
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                        { "text": " • " },
                                        { "text": "3:50" }
                                    ]
                                }
                            }
                        }
                    ]
                }
            }]
        });

        let mut tracks = Vec::new();
        collect_tracks(&track_json, &mut tracks);
        assert_eq!(tracks.len(), 1);
        assert!(tracks[0].is_explicit);
        assert_eq!(tracks[0].album, "Starboy");
        assert_eq!(tracks[0].album_id.as_deref(), Some("MPREb_starboy123"));
    }

    #[test]
    fn album_tracks_inherit_header_artist_cover_and_album_id() {
        let album_json = json!({
            "contents": {
                "twoColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicResponsiveHeaderRenderer": {
                                            "title": { "runs": [{ "text": "Starboy" }] },
                                            "straplineTextOne": {
                                                "runs": [{ "text": "The Weeknd" }]
                                            },
                                            "subtitle": {
                                                "runs": [
                                                    { "text": "Album" },
                                                    { "text": " • " },
                                                    { "text": "2016" }
                                                ]
                                            },
                                            "thumbnail": {
                                                "musicThumbnailRenderer": {
                                                    "thumbnail": {
                                                        "thumbnails": [
                                                            { "url": "https://lh3.googleusercontent.com/starboy_cover=w226-h226-l90-rj" }
                                                        ]
                                                    }
                                                }
                                            }
                                        }
                                    }]
                                }
                            }
                        }
                    }],
                    "secondaryContents": {
                        "sectionListRenderer": {
                            "contents": [{
                                "musicShelfRenderer": {
                                    "contents": [{
                                        "musicResponsiveListItemRenderer": {
                                            "playlistItemData": { "videoId": "3_g2un5M350" },
                                            "flexColumns": [
                                                {
                                                    "musicResponsiveListItemFlexColumnRenderer": {
                                                        "text": { "runs": [{ "text": "Starboy" }] }
                                                    }
                                                },
                                                {
                                                    "musicResponsiveListItemFlexColumnRenderer": {
                                                        "text": { "runs": [{ "text": "3:50" }] }
                                                    }
                                                }
                                            ]
                                        }
                                    }]
                                }
                            }]
                        }
                    }
                }
            }
        });

        let header = find_collection_header(&album_json);
        assert_eq!(header.title.as_deref(), Some("Starboy"));
        assert_eq!(header.artist.as_deref(), Some("The Weeknd"));
        assert_eq!(
            header.cover_url.as_deref(),
            Some("https://lh3.googleusercontent.com/starboy_cover=w544-h544-l90-rj")
        );

        let mut tracks = Vec::new();
        collect_tracks(&album_json, &mut tracks);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].artist, "Unknown Artist");
        assert!(tracks[0].thumbnail_url.is_none());

        apply_collection_backfill(&mut tracks, "MPREb_starboy123", &header);
        assert_eq!(tracks[0].artist, "The Weeknd");
        assert_eq!(tracks[0].album, "Starboy");
        assert_eq!(tracks[0].album_id.as_deref(), Some("MPREb_starboy123"));
        assert_eq!(
            tracks[0].thumbnail_url.as_deref(),
            Some("https://lh3.googleusercontent.com/starboy_cover=w544-h544-l90-rj")
        );
    }

    #[test]
    fn infers_playlist_ownership_from_menu_endpoints_and_browse_prefix() {
        let sample = json!({
            "contents": [
                {
                    "musicTwoRowItemRenderer": {
                        "title": { "runs": [{ "text": "My Mix" }] },
                        "navigationEndpoint": { "browseEndpoint": { "browseId": "VLPLowned123" } },
                        "menu": {
                            "menuRenderer": {
                                "items": [{
                                    "menuServiceItemRenderer": {
                                        "serviceEndpoint": {
                                            "deletePlaylistServiceEndpoint": { "playlistId": "PLowned123" }
                                        }
                                    }
                                }]
                            }
                        }
                    }
                },
                {
                    "musicTwoRowItemRenderer": {
                        "title": { "runs": [{ "text": "Community Chill" }] },
                        "navigationEndpoint": { "browseEndpoint": { "browseId": "VLPLsaved456" } },
                        "menu": {
                            "menuRenderer": {
                                "items": [{
                                    "toggleMenuServiceItemRenderer": {
                                        "defaultServiceEndpoint": {
                                            "likeEndpoint": {
                                                "status": "LIKE",
                                                "target": { "playlistId": "PLsaved456" }
                                            }
                                        }
                                    }
                                }]
                            }
                        }
                    }
                },
                {
                    "musicTwoRowItemRenderer": {
                        "title": { "runs": [{ "text": "Supermix" }] },
                        "navigationEndpoint": { "browseEndpoint": { "browseId": "VLRDCLAK5uy_mix" } }
                    }
                }
            ]
        });

        let mut playlists = Vec::new();
        collect_playlists(&sample, &mut playlists);
        assert_eq!(playlists.len(), 3);
        assert!(playlists[0].is_owned);
        assert!(!playlists[1].is_owned);
        assert!(!playlists[2].is_owned);
    }

    #[test]
    fn collects_history_feedback_tokens_from_responsive_items_and_ignores_unrelated_menus() {
        let sample = json!({
            "contents": [{
                "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "hist_vid_1" },
                    "menu": {
                        "menuRenderer": {
                            "items": [
                                {
                                    "menuServiceItemRenderer": {
                                        "text": { "runs": [{ "text": "Remove from history" }] },
                                        "icon": { "iconType": "REMOVE_FROM_HISTORY" },
                                        "serviceEndpoint": {
                                            "feedbackEndpoint": { "feedbackToken": "tok_remove_1" }
                                        }
                                    }
                                },
                                {
                                    "toggleMenuServiceItemRenderer": {
                                        "defaultText": { "runs": [{ "text": "Geçmişten kaldır" }] },
                                        "defaultIcon": { "iconType": "DELETE" },
                                        "defaultServiceEndpoint": {
                                            "feedbackEndpoint": { "feedbackToken": "tok_remove_2" }
                                        }
                                    }
                                },
                                {
                                    "menuServiceItemRenderer": {
                                        "text": { "runs": [{ "text": "Add to library" }] },
                                        "icon": { "iconType": "LIBRARY_ADD" },
                                        "serviceEndpoint": {
                                            "feedbackEndpoint": { "feedbackToken": "tok_unrelated" }
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            }]
        });

        let mut tokens = std::collections::HashMap::new();
        collect_history_feedback_tokens(&sample, &mut tokens);
        assert_eq!(
            tokens.get("hist_vid_1").map(Vec::as_slice),
            Some(["tok_remove_1".to_string(), "tok_remove_2".to_string()].as_slice())
        );
    }
}
