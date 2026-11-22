//! Tolerant JSON-tree parsers for YouTube Music InnerTube responses.
//!
//! 1. **Playlists & Tracks**: extracts `Playlist` and `Track` items along with continuation
//!    tokens and `setVideoId` metadata from responsive/two-row renderers.
//! 2. **Account & Headers**: parses `YtAccountProfile`, header thumbnails, and descriptions.
//! 3. **Home & Artist Pages**: walks shelf/carousel nodes into `HomeShelf`, `HomeCard`, and
//!    `ArtistPage` structures, covered by unit tests.

use super::{upscale_thumbnail, ArtistPage, HomeCard, HomeCardKind, HomeShelf, YtAccountProfile};
use crate::providers::{parse_duration_text, Playlist, ProviderSource, Track};
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

fn parse_two_row_playlist(r: &Value) -> Option<Playlist> {
    let raw_id = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| r.pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        .and_then(|v| v.as_str())?;
    let browse_id = normalize_playlist_browse_id(raw_id)?;
    let name = extract_runs_text(r.get("title")?)?;
    let thumb = extract_last_thumbnail(r.get("thumbnailRenderer"));
    Some(Playlist {
        id: browse_id,
        name,
        source: ProviderSource::YouTubeMusic,
        track_count: None,
        thumbnail_url: thumb,
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
    Some(Playlist {
        id: browse_id,
        name,
        source: ProviderSource::YouTubeMusic,
        track_count: None,
        thumbnail_url: thumb,
    })
}

pub(super) fn collect_tracks(val: &Value, out: &mut Vec<Track>) {
    match val {
        Value::Object(map) => {
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
            // Filter out category tags like "Song" / "Video"
            let filtered: Vec<_> = parts
                .into_iter()
                .filter(|p| !matches!(p.as_str(), "Song" | "Video" | "Single" | "EP"))
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
            ] {
                if let Some(shelf) = map.get(shelf_key) {
                    if let Some(tok) = shelf
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

pub(super) fn is_excluded_shelf_title(title: &str) -> bool {
    let lower = title.trim().to_ascii_lowercase();
    lower.contains("forgotten favorite") || lower.contains("unutulan favori")
}

pub(super) fn merge_home_shelves(dst: &mut Vec<HomeShelf>, src: Vec<HomeShelf>) {
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

fn parse_home_two_row(r: &Value) -> Option<HomeCard> {
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

pub(super) fn extract_search_artist_browse_id(val: &Value, preferred_name: &str) -> Option<String> {
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

pub(super) fn extract_search_album_browse_id(val: &Value, preferred_album: &str) -> Option<String> {
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

pub(super) fn find_artist_songs_endpoint(val: &Value) -> Option<(String, Option<String>)> {
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
    fn parses_explicit_badge_album_id_and_immersive_artist_hero() {
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

    #[test]
    fn album_tracks_inherit_header_artist_cover_and_album_id_and_drop_forgotten_favorites() {
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
