//! WebDAV RFC 4918 audio crawler, metadata/artwork/lyrics enrichment, and stream builder.
//!
//! 1. **PROPFIND XML Parsing**: Parses `Depth: 1` and `Depth: infinity` WebDAV `<multistatus>`
//!    responses using `roxmltree`, handling arbitrary namespace prefixes and multi-`<propstat>`
//!    status codes.
//! 2. **Browsing, Albums & Artists**: Supports single-folder browsing, bounded `Depth: infinity` /
//!    BFS directory crawling, album grouping, artist discovery (`artist.json` / `artist.nfo`), and
//!    generic container / multi-disc path inference.
//! 3. **FLAC Metadata, Cover Art & Lyrics**: Uses bounded HTTP `Range` requests to read FLAC
//!    `STREAMINFO`/`VORBIS_COMMENT` tags, discovers folder cover images (`cover.jpg`, `folder.jpg`,
//!    `artist.jpg`) with optional `0o600`/`0o700` authenticated disk caching, and resolves sidecar
//!    `.lrc`/`.txt` or embedded FLAC lyrics.

use super::{
    default_http_timeout, encode_server_item_id, format_audio_badge, map_http_status,
    no_redirect_http_client, ServerKind, ServerProfile, ServerSearchResult, ServerStream,
};
use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::providers::hifi::extract_badge_kbps;
use crate::providers::local::{format_local_audio_badge, parse_flac_header_bytes};
use crate::providers::ytmusic::{AlbumDetail, HomeCard, HomeCardKind};
use crate::providers::{ProviderSource, Track};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Method, StatusCode, Url};
use roxmltree::{Document, Node};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
    <D:getcontenttype/>
    <D:getcontentlength/>
    <D:getlastmodified/>
    <D:getetag/>
  </D:prop>
</D:propfind>"#;

const AUDIO_EXTENSIONS: [&str; 17] = [
    "mp3", "flac", "opus", "ogg", "oga", "m4a", "aac", "wav", "wma", "alac", "aiff", "aif", "wv",
    "ape", "mpc", "dsf", "dff",
];

const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "avif"];
const COVER_PRIORITY_BASENAMES: [&str; 8] = [
    "cover", "folder", "front", "album", "artwork", "thumb", "artist", "avatar",
];
const ARTIST_AVATAR_BASENAMES: [&str; 6] =
    ["artist", "folder", "avatar", "thumb", "cover", "front"];
const GENERIC_CONTAINER_SEGMENTS: [&str; 11] = [
    "share", "music", "audio", "songs", "library", "media", "flac", "mp3", "lossless", "public",
    "files",
];

const DEFAULT_MAX_CRAWL_ITEMS: usize = 2_000;
const DEFAULT_MAX_CRAWL_DEPTH: usize = 6;
const FLAC_HEADER_RANGE_BYTES: usize = 131_072;
const MAX_ART_BYTES: usize = 8 * 1024 * 1024;
const MAX_META_TEXT_BYTES: usize = 128 * 1024;
const MAX_LYRICS_BYTES: usize = 512 * 1024;
const MAX_FLAC_RANGE_READS_PER_FOLDER: usize = 32;

/// One parsed file or collection entry returned by a WebDAV `PROPFIND` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebDavEntry {
    pub url: String,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
}

/// Directory listing returned by `browse_folder` and `browse_folder_enriched`.
#[derive(Debug, Clone, Default)]
pub struct WebDavListing {
    pub folder_url: String,
    pub folders: Vec<WebDavEntry>,
    pub tracks: Vec<Track>,
    pub entries: Vec<WebDavEntry>,
}

/// Parsed artist folder sidecar metadata (`artist.json`, `info.json`, `metadata.json`, `artist.nfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebDavArtistMetadata {
    pub name: Option<String>,
    pub bio: Option<String>,
    pub subtitle: Option<String>,
}

/// Aggregated artist detail resolved from a WebDAV library.
#[derive(Debug, Clone, Default)]
pub struct WebDavArtistProfile {
    pub name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub subtitle: Option<String>,
    pub albums: Vec<HomeCard>,
    pub tracks: Vec<Track>,
    pub related_artists: Vec<HomeCard>,
}

/// Builds the HTTP `Authorization` header (`Bearer` or `Basic`) for a WebDAV profile, if configured.
pub fn auth_headers(profile: &ServerProfile) -> Vec<(String, String)> {
    if let Some(bearer) = profile
        .bearer_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return vec![("Authorization".to_string(), format!("Bearer {bearer}"))];
    }
    let user = profile.username.as_deref().unwrap_or("");
    let pass = profile.password.as_deref().unwrap_or("");
    if !user.is_empty() || !pass.is_empty() {
        let encoded = BASE64_STANDARD.encode(format!("{user}:{pass}").as_bytes());
        return vec![("Authorization".to_string(), format!("Basic {encoded}"))];
    }
    Vec::new()
}

fn apply_profile_headers(
    profile: &ServerProfile,
    extra: &[(&str, &str)],
) -> Result<HeaderMap, CoreError> {
    let mut header_map = HeaderMap::new();
    for (k, v) in extra {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|err| CoreError::internal(format!("invalid WebDAV header `{k}`: {err}")))?;
        let val = HeaderValue::from_str(v)
            .map_err(|err| CoreError::internal(format!("invalid header `{v}`: {err}")))?;
        header_map.insert(name, val);
    }
    for (k, v) in auth_headers(profile) {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|err| CoreError::internal(format!("invalid WebDAV header `{k}`: {err}")))?;
        let val = HeaderValue::from_str(&v).map_err(|err| {
            CoreError::auth_required(format!("invalid WebDAV auth header value: {err}"))
        })?;
        header_map.insert(name, val);
    }
    Ok(header_map)
}

fn is_same_origin(base: &Url, target: &Url) -> bool {
    base.scheme().eq_ignore_ascii_case(target.scheme())
        && base.host().is_some()
        && base.host() == target.host()
        && base.port_or_known_default() == target.port_or_known_default()
}

fn resolve_target_url(
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
) -> Result<Url, CoreError> {
    let base_str = profile.normalized_base_url();
    if base_str.is_empty() {
        return Err(CoreError::invalid_response(
            "webdav",
            "WebDAV base URL is empty",
        ));
    }
    let base_with_slash = format!("{base_str}/");
    let base_url = Url::parse(&base_with_slash).map_err(|err| {
        CoreError::invalid_response("webdav", format!("invalid WebDAV base URL: {err}"))
    })?;
    let Some(target) = folder_or_path.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(base_url);
    };
    let resolved = base_url.join(target).map_err(|err| {
        CoreError::invalid_response("webdav", format!("invalid WebDAV target URL: {err}"))
    })?;
    if !is_same_origin(&base_url, &resolved) {
        return Err(CoreError::invalid_response(
            "webdav",
            "cross-origin WebDAV URL rejected",
        ));
    }
    Ok(resolved)
}

async fn send_propfind(
    _http: &Client,
    profile: &ServerProfile,
    target_url: &Url,
    depth: &str,
) -> Result<(StatusCode, String), CoreError> {
    let method = Method::from_bytes(b"PROPFIND").map_err(|err| {
        CoreError::internal(format!("failed to construct PROPFIND HTTP method: {err}"))
    })?;
    let header_map = apply_profile_headers(
        profile,
        &[
            ("Depth", depth),
            ("Content-Type", "application/xml; charset=utf-8"),
        ],
    )?;

    let resp = no_redirect_http_client()
        .request(method, target_url.clone())
        .timeout(default_http_timeout())
        .headers(header_map)
        .body(PROPFIND_BODY)
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("webdav", &err))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|err| CoreError::from_reqwest("webdav", &err))?;
    Ok((status, body))
}

async fn fetch_url_bytes_bounded(
    _http: &Client,
    profile: &ServerProfile,
    url: &str,
    range_end_inclusive: Option<usize>,
    max_bytes: usize,
) -> Result<Vec<u8>, CoreError> {
    let validated_url = resolve_target_url(profile, Some(url))?;
    let extra_headers = range_end_inclusive.map(|end| format!("bytes=0-{end}"));
    let extra = extra_headers
        .as_deref()
        .map(|r| vec![("Range", r)])
        .unwrap_or_default();
    let header_map = apply_profile_headers(profile, &extra)?;
    let mut resp = no_redirect_http_client()
        .get(validated_url)
        .timeout(default_http_timeout())
        .headers(header_map)
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("webdav", &err))?;
    let status = resp.status();
    if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
        return Err(map_http_status("webdav", status, ""));
    }
    let mut buf = Vec::new();
    while buf.len() < max_bytes {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = max_bytes - buf.len();
                let take = chunk.len().min(remaining);
                buf.extend_from_slice(&chunk[..take]);
                if take < chunk.len() {
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                if !buf.is_empty() {
                    break;
                }
                return Err(CoreError::from_reqwest("webdav", &err));
            }
        }
    }
    Ok(buf)
}

async fn cache_authenticated_artwork(
    http: &Client,
    profile: &ServerProfile,
    image_url: &str,
    cache_dir: Option<&Path>,
) -> String {
    let Some(root_cache) = cache_dir else {
        return image_url.to_string();
    };
    let mut hasher = Sha1::new();
    hasher.update(profile.id.as_bytes());
    hasher.update(b"|");
    hasher.update(image_url.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let ext = extension_of(image_url)
        .map(str::to_ascii_lowercase)
        .filter(|e| IMAGE_EXTENSIONS.iter().any(|cand| cand == e))
        .unwrap_or_else(|| "jpg".to_string());
    let art_dir = root_cache.join("webdav_art");
    let target = art_dir.join(format!("{digest}.{ext}"));

    if tokio::fs::metadata(&target)
        .await
        .is_ok_and(|m| m.is_file() && m.len() > 0)
    {
        return format!("file://{}", target.to_string_lossy());
    }

    let Ok(bytes) = fetch_url_bytes_bounded(http, profile, image_url, None, MAX_ART_BYTES).await
    else {
        return image_url.to_string();
    };
    if bytes.is_empty() {
        return image_url.to_string();
    }

    static COVER_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let target_clone = target.clone();
    let written = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        ensure_private_dir(&art_dir)?;
        let seq = COVER_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = art_dir.join(format!("{digest}.{ext}.{}.{seq}.tmp", std::process::id()));
        let mut f = create_private_file(&tmp)?;
        if let Err(err) = f.write_all(&bytes).and_then(|()| f.sync_all()) {
            drop(f);
            let _ = std::fs::remove_file(&tmp);
            return Err(err);
        }
        drop(f);
        if let Err(err) = std::fs::rename(&tmp, &target_clone) {
            let _ = std::fs::remove_file(&tmp);
            return Err(err);
        }
        Ok(())
    })
    .await;

    if matches!(written, Ok(Ok(()))) {
        format!("file://{}", target.to_string_lossy())
    } else {
        image_url.to_string()
    }
}

/// Lists immediate subfolders and audio tracks at `folder_or_path` (`Depth: 1`).
pub async fn browse_folder(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
) -> Result<WebDavListing, CoreError> {
    browse_folder_enriched(http, profile, folder_or_path, None).await
}

/// Lists immediate subfolders and audio tracks at `folder_or_path` (`Depth: 1`), enriching tracks
/// with sibling cover artwork, `album.json`/`metadata.json`, and HTTP `Range` FLAC headers.
pub async fn browse_folder_enriched(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
    cache_dir: Option<&Path>,
) -> Result<WebDavListing, CoreError> {
    let target_url = resolve_target_url(profile, folder_or_path)?;
    let (status, body) = send_propfind(http, profile, &target_url, "1").await?;
    if !status.is_success() && status.as_u16() != 207 {
        return Err(map_http_status("webdav", status, &body));
    }
    let entries = parse_propfind_xml(&body, target_url.as_str())?;
    let mut listing = build_listing(profile, &target_url, entries.clone());

    let folder_meta = if let Some(meta_entry) = find_album_metadata_entry(&entries) {
        fetch_url_bytes_bounded(http, profile, &meta_entry.url, None, MAX_META_TEXT_BYTES)
            .await
            .ok()
            .and_then(|raw| String::from_utf8(raw).ok())
            .map(|text| parse_album_metadata_json(&text))
    } else {
        None
    };

    let mut cached_images: HashMap<String, String> = HashMap::new();
    let default_cover_url = if let Some(cover_entry) = choose_folder_cover(&entries) {
        let resolved =
            cache_authenticated_artwork(http, profile, &cover_entry.url, cache_dir).await;
        cached_images.insert(cover_entry.url.clone(), resolved.clone());
        Some(resolved)
    } else {
        None
    };

    let audio_entries: Vec<&WebDavEntry> = entries
        .iter()
        .filter(|e| !e.is_dir && is_audio_entry(e))
        .collect();
    let mut flac_reads = 0usize;

    for (idx, track) in listing.tracks.iter_mut().enumerate() {
        let Some(entry) = audio_entries.get(idx) else {
            continue;
        };
        let track_stem = entry_file_stem(entry);
        let per_track_img = find_per_track_image(&entries, &track_stem);
        let resolved_thumb = if let Some(img) = per_track_img {
            if let Some(existing) = cached_images.get(&img.url) {
                Some(existing.clone())
            } else {
                let u = cache_authenticated_artwork(http, profile, &img.url, cache_dir).await;
                cached_images.insert(img.url.clone(), u.clone());
                Some(u)
            }
        } else {
            default_cover_url.clone()
        };
        if resolved_thumb.is_some() {
            track.thumbnail_url = resolved_thumb;
        }

        if let Some((meta_artist, meta_album)) = &folder_meta {
            if let Some(a) = meta_artist.as_ref().filter(|s| !s.is_empty()) {
                track.artist = a.clone();
            }
            if let Some(alb) = meta_album.as_ref().filter(|s| !s.is_empty()) {
                track.album = alb.clone();
            }
        }

        if entry.path.to_ascii_lowercase().ends_with(".flac")
            && flac_reads < MAX_FLAC_RANGE_READS_PER_FOLDER
        {
            flac_reads += 1;
            if let Ok(slice) = fetch_url_bytes_bounded(
                http,
                profile,
                &entry.url,
                Some(FLAC_HEADER_RANGE_BYTES - 1),
                FLAC_HEADER_RANGE_BYTES,
            )
            .await
            {
                if let Some(tags) = parse_flac_header_bytes(&slice, entry.content_length) {
                    if let Some(t) = tags.title.filter(|s| !s.trim().is_empty()) {
                        track.title = t;
                    }
                    if let Some(a) = tags
                        .artist
                        .or(tags.album_artist)
                        .filter(|s| !s.trim().is_empty())
                    {
                        track.artist = a;
                    }
                    if let Some(alb) = tags.album.filter(|s| !s.trim().is_empty()) {
                        track.album = alb;
                    }
                    if tags.duration_secs.is_some() {
                        track.duration_secs = tags.duration_secs;
                    }
                    let badge = format_local_audio_badge(
                        "FLAC",
                        tags.bitrate_kbps,
                        tags.bits_per_sample,
                        tags.sample_rate,
                    );
                    if !badge.ends_with("· Local") {
                        track.audio_badge = Some(badge);
                    }
                }
            }
        }
    }

    Ok(listing)
}

/// Crawls all directories and files recursively, trying `Depth: infinity` first and falling back
/// to bounded `Depth: 1` BFS traversal when the server restricts infinite depth.
pub async fn crawl_entries(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
    max_depth: Option<usize>,
    max_items: Option<usize>,
) -> Result<Vec<WebDavEntry>, CoreError> {
    let start_url = resolve_target_url(profile, folder_or_path)?;
    let limit = max_items
        .unwrap_or(DEFAULT_MAX_CRAWL_ITEMS)
        .saturating_mul(3);
    if limit == 0 {
        return Ok(Vec::new());
    }

    if max_depth.is_none() {
        let (status, body) = send_propfind(http, profile, &start_url, "infinity").await?;
        if status.is_success() || status.as_u16() == 207 {
            let entries = parse_propfind_xml(&body, start_url.as_str())?;
            return Ok(entries.into_iter().take(limit).collect());
        }
        if !matches!(status, StatusCode::FORBIDDEN | StatusCode::BAD_REQUEST) {
            return Err(map_http_status("webdav", status, &body));
        }
    }

    let depth_limit = max_depth.unwrap_or(DEFAULT_MAX_CRAWL_DEPTH);
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(normalize_path_key(start_url.path()));
    queue.push_back((start_url, 0usize));

    let mut all_entries = Vec::new();
    while let Some((dir_url, depth)) = queue.pop_front() {
        if all_entries.len() >= limit {
            break;
        }
        let (status, body) = send_propfind(http, profile, &dir_url, "1").await?;
        if !status.is_success() && status.as_u16() != 207 {
            return Err(map_http_status("webdav", status, &body));
        }
        let entries = parse_propfind_xml(&body, dir_url.as_str())?;
        for entry in entries {
            if entry.is_dir && depth < depth_limit {
                let key = normalize_path_key(&entry.path);
                if visited.insert(key) {
                    if let Ok(next_url) = Url::parse(&entry.url) {
                        queue.push_back((next_url, depth + 1));
                    }
                }
            }
            all_entries.push(entry);
            if all_entries.len() >= limit {
                break;
            }
        }
    }
    Ok(all_entries)
}

/// Crawls audio tracks recursively, attaching folder cover art where available.
pub async fn crawl_audio(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
    max_depth: Option<usize>,
    max_items: Option<usize>,
) -> Result<Vec<Track>, CoreError> {
    let limit = max_items.unwrap_or(DEFAULT_MAX_CRAWL_ITEMS);
    if limit == 0 {
        return Ok(Vec::new());
    }
    let entries = crawl_entries(http, profile, folder_or_path, max_depth, max_items).await?;
    let dir_images = index_images_by_dir(&entries);
    let mut tracks = entries
        .iter()
        .filter(|e| !e.is_dir && is_audio_entry(e))
        .map(|e| {
            let mut t = entry_to_track(profile, e);
            let dir = parent_dir_of_path(&e.path);
            if let Some(cover) = dir_images
                .get(&dir)
                .and_then(|imgs| choose_folder_cover(imgs))
            {
                t.thumbnail_url = Some(cover.url.clone());
            }
            t
        })
        .take(limit)
        .collect::<Vec<_>>();
    tracks.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(tracks)
}

/// Lists inferred album folders across the WebDAV repository as [`HomeCardKind::Album`] cards.
pub async fn list_albums(
    http: &Client,
    profile: &ServerProfile,
    library_id: Option<&str>,
    artist_filter: Option<&str>,
    cache_dir: Option<&Path>,
) -> Result<Vec<HomeCard>, CoreError> {
    let entries = crawl_entries(http, profile, library_id, None, None).await?;
    let dir_images = index_images_by_dir(&entries);
    let needle = artist_filter
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());

    let mut groups: BTreeMap<String, Vec<Track>> = BTreeMap::new();
    for e in entries.iter().filter(|e| !e.is_dir && is_audio_entry(e)) {
        let mut t = entry_to_track(profile, e);
        let dir = parent_dir_of_path(&e.path);
        if let Some(cover) = dir_images
            .get(&dir)
            .and_then(|imgs| choose_folder_cover(imgs))
        {
            t.thumbnail_url = Some(cover.url.clone());
        }
        groups.entry(dir).or_default().push(t);
    }

    let mut cards = Vec::new();
    for (dir, tracks) in groups {
        let Some(first) = tracks.first() else {
            continue;
        };
        let album_title = if !first.album.trim().is_empty() && first.album != "Unknown Album" {
            first.album.clone()
        } else {
            fallback_name_from_path(&dir)
        };
        let artist_name = if !first.artist.trim().is_empty() && first.artist != "Unknown Artist" {
            first.artist.clone()
        } else {
            "Unknown Artist".to_string()
        };
        if let Some(ref target) = needle {
            let clean_target = target.strip_prefix("artist:").unwrap_or(target.as_str());
            if !artist_name.to_ascii_lowercase().contains(clean_target) {
                continue;
            }
        }
        let thumb = match first.thumbnail_url.as_deref() {
            Some(url) => Some(cache_authenticated_artwork(http, profile, url, cache_dir).await),
            None => None,
        };
        let mut first_with_thumb = first.clone();
        first_with_thumb.thumbnail_url = thumb.clone();
        cards.push(HomeCard {
            kind: HomeCardKind::Album,
            id: encode_server_item_id(ServerKind::WebDav, &profile.id, &dir),
            title: album_title,
            subtitle: artist_name,
            thumbnail_url: thumb,
            track: Some(first_with_thumb),
        });
    }
    Ok(cards)
}

/// Fetches album details and enriched tracks for a WebDAV directory path.
pub async fn get_album_detail(
    http: &Client,
    profile: &ServerProfile,
    album_dir_path: &str,
    cache_dir: Option<&Path>,
) -> Result<AlbumDetail, CoreError> {
    let listing = browse_folder_enriched(http, profile, Some(album_dir_path), cache_dir).await?;
    let tracks = listing.tracks;
    let first = tracks.first();
    let (_, inferred_album) = infer_artist_and_album(profile, album_dir_path, false);
    let title = first
        .map(|t| t.album.clone())
        .filter(|a| !a.is_empty() && a != "Unknown Album")
        .unwrap_or_else(|| {
            if !inferred_album.is_empty() {
                inferred_album
            } else {
                fallback_name_from_path(album_dir_path)
            }
        });
    let artist_name = first
        .map(|t| t.artist.clone())
        .filter(|a| !a.is_empty() && a != "Unknown Artist")
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let cover_url = first.and_then(|t| t.thumbnail_url.clone());
    let total_secs: u64 = tracks.iter().filter_map(|t| t.duration_secs).sum();
    let duration_text = if total_secs > 0 {
        Some(format!("{} min", (total_secs + 30) / 60))
    } else {
        None
    };

    Ok(AlbumDetail {
        id: encode_server_item_id(ServerKind::WebDav, &profile.id, album_dir_path),
        title,
        subtitle: format!("Album • {artist_name}"),
        release_type: "Album".to_string(),
        year: None,
        duration_text,
        cover_url,
        artists: vec![(
            artist_name.clone(),
            Some(encode_server_item_id(
                ServerKind::WebDav,
                &profile.id,
                &format!("artist:{artist_name}"),
            )),
        )],
        tracks,
        other_versions: Vec::new(),
        playlist_id: None,
        saved: false,
    })
}

/// Lists artists discovered from WebDAV folder hierarchy, `artist.json`/`artist.nfo`, and artwork.
pub async fn list_artists(
    http: &Client,
    profile: &ServerProfile,
    library_id: Option<&str>,
    cache_dir: Option<&Path>,
) -> Result<Vec<HomeCard>, CoreError> {
    let entries = crawl_entries(http, profile, library_id, None, None).await?;
    let dir_images = index_images_by_dir(&entries);
    let artist_meta_map = collect_artist_folder_metadata(http, profile, &entries).await;

    let mut grouped: BTreeMap<String, Vec<Track>> = BTreeMap::new();
    for e in entries.iter().filter(|e| !e.is_dir && is_audio_entry(e)) {
        let mut t = entry_to_track(profile, e);
        let dir = parent_dir_of_path(&e.path);
        if let Some(cover) = dir_images
            .get(&dir)
            .and_then(|imgs| choose_folder_cover(imgs))
        {
            t.thumbnail_url = Some(cover.url.clone());
        }
        let artist_key = if t.artist.trim().is_empty() {
            "Unknown Artist".to_string()
        } else {
            t.artist.clone()
        };
        grouped.entry(artist_key).or_default().push(t);
    }

    let mut cards = Vec::with_capacity(grouped.len());
    for (artist_name, tracks) in grouped {
        let key_lower = artist_name.trim().to_ascii_lowercase();
        let meta = artist_meta_map.get(&key_lower);
        let raw_avatar = dir_images
            .iter()
            .find(|(dir_path, _)| {
                fallback_name_from_path(dir_path)
                    .trim()
                    .eq_ignore_ascii_case(artist_name.trim())
            })
            .and_then(|(_, imgs)| choose_artist_avatar(imgs))
            .map(|e| e.url.clone())
            .or_else(|| tracks.iter().find_map(|t| t.thumbnail_url.clone()));

        let avatar = match raw_avatar.as_deref() {
            Some(url) => Some(cache_authenticated_artwork(http, profile, url, cache_dir).await),
            None => None,
        };
        let display_name = meta
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| artist_name.clone());
        let subtitle = meta
            .and_then(|m| m.subtitle.clone())
            .unwrap_or_else(|| format!("{} tracks", tracks.len()));

        cards.push(HomeCard {
            kind: HomeCardKind::Artist,
            id: encode_server_item_id(
                ServerKind::WebDav,
                &profile.id,
                &format!("artist:{artist_name}"),
            ),
            title: display_name,
            subtitle,
            thumbnail_url: avatar,
            track: tracks.into_iter().next(),
        });
    }
    Ok(cards)
}

/// Fetches full artist metadata (`artist.json`/`artist.nfo`/`artist.jpg`), albums, and tracks from WebDAV.
pub async fn fetch_artist_profile(
    http: &Client,
    profile: &ServerProfile,
    artist_id_or_name: &str,
    cache_dir: Option<&Path>,
) -> Result<Option<WebDavArtistProfile>, CoreError> {
    let clean_target = artist_id_or_name
        .strip_prefix("artist:")
        .unwrap_or(artist_id_or_name)
        .trim();
    if clean_target.is_empty() {
        return Ok(None);
    }
    let entries = crawl_entries(http, profile, None, None, None).await?;
    let dir_images = index_images_by_dir(&entries);
    let artist_meta_map = collect_artist_folder_metadata(http, profile, &entries).await;

    let target_lower = clean_target.to_ascii_lowercase();
    let mut matched_tracks = Vec::new();
    let mut album_groups: BTreeMap<String, Vec<Track>> = BTreeMap::new();
    let mut other_artists: BTreeMap<String, Option<String>> = BTreeMap::new();

    for e in entries.iter().filter(|e| !e.is_dir && is_audio_entry(e)) {
        let mut t = entry_to_track(profile, e);
        let dir = parent_dir_of_path(&e.path);
        if let Some(cover) = dir_images
            .get(&dir)
            .and_then(|imgs| choose_folder_cover(imgs))
        {
            t.thumbnail_url = Some(cover.url.clone());
        }
        if t.artist.trim().eq_ignore_ascii_case(clean_target) {
            album_groups.entry(dir).or_default().push(t.clone());
            matched_tracks.push(t);
        } else if !t.artist.trim().is_empty() {
            other_artists
                .entry(t.artist.clone())
                .or_insert_with(|| t.thumbnail_url.clone());
        }
    }

    let meta = artist_meta_map.get(&target_lower);
    let raw_avatar = dir_images
        .iter()
        .find(|(dir_path, _)| {
            fallback_name_from_path(dir_path)
                .trim()
                .eq_ignore_ascii_case(clean_target)
        })
        .and_then(|(_, imgs)| choose_artist_avatar(imgs))
        .map(|e| e.url.clone())
        .or_else(|| matched_tracks.iter().find_map(|t| t.thumbnail_url.clone()));

    if matched_tracks.is_empty() && meta.is_none() && raw_avatar.is_none() {
        return Ok(None);
    }

    let avatar_url = match raw_avatar.as_deref() {
        Some(url) => Some(cache_authenticated_artwork(http, profile, url, cache_dir).await),
        None => None,
    };

    for t in &mut matched_tracks {
        if let Some(url) = t.thumbnail_url.clone() {
            t.thumbnail_url =
                Some(cache_authenticated_artwork(http, profile, &url, cache_dir).await);
        }
    }

    let mut albums = Vec::new();
    for (dir, alb_tracks) in album_groups {
        let Some(first) = alb_tracks.first() else {
            continue;
        };
        let album_title = if !first.album.is_empty() && first.album != "Unknown Album" {
            first.album.clone()
        } else {
            fallback_name_from_path(&dir)
        };
        let thumb = match first.thumbnail_url.as_deref() {
            Some(url) => Some(cache_authenticated_artwork(http, profile, url, cache_dir).await),
            None => None,
        };
        albums.push(HomeCard {
            kind: HomeCardKind::Album,
            id: encode_server_item_id(ServerKind::WebDav, &profile.id, &dir),
            title: album_title,
            subtitle: meta
                .and_then(|m| m.name.clone())
                .unwrap_or_else(|| clean_target.to_string()),
            thumbnail_url: thumb,
            track: Some(first.clone()),
        });
    }

    let related_artists = other_artists
        .into_iter()
        .take(12)
        .map(|(name, thumb)| HomeCard {
            kind: HomeCardKind::Artist,
            id: encode_server_item_id(ServerKind::WebDav, &profile.id, &format!("artist:{name}")),
            title: name,
            subtitle: "Artist".to_string(),
            thumbnail_url: thumb,
            track: None,
        })
        .collect();

    Ok(Some(WebDavArtistProfile {
        name: meta
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| clean_target.to_string()),
        avatar_url,
        bio: meta.and_then(|m| m.bio.clone()),
        subtitle: meta.and_then(|m| m.subtitle.clone()),
        albums,
        tracks: matched_tracks,
        related_artists,
    }))
}

/// Fetches sidecar `.ttml`/`.lrc`/`.txt` lyrics or embedded FLAC `LYRICS` tags for a WebDAV file path.
pub async fn fetch_lyrics(
    http: &Client,
    profile: &ServerProfile,
    raw_file_path: &str,
) -> Result<Option<String>, CoreError> {
    let track_url = resolve_target_url(profile, Some(raw_file_path))?;
    let parent_dir = parent_dir_of_path(raw_file_path);
    let parent_url = if parent_dir.is_empty() {
        resolve_target_url(profile, None)?
    } else {
        resolve_target_url(profile, Some(&format!("{parent_dir}/")))?
    };

    if let Ok((status, xml)) = send_propfind(http, profile, &parent_url, "1").await {
        if status.is_success() || status.as_u16() == 207 {
            if let Ok(entries) = parse_propfind_xml(&xml, parent_url.as_str()) {
                let raw_name = fallback_name_from_path(raw_file_path);
                let file_stem = raw_name
                    .rsplit_once('.')
                    .map_or(raw_name.as_str(), |(s, _)| s);
                let clean_stem = clean_track_title(&raw_name);
                let audio_count = entries
                    .iter()
                    .filter(|e| !e.is_dir && is_audio_entry(e))
                    .count();
                if let Some(lyric_entry) =
                    choose_lyric_entry(&entries, file_stem, &clean_stem, audio_count)
                {
                    if let Ok(raw_bytes) = fetch_url_bytes_bounded(
                        http,
                        profile,
                        &lyric_entry.url,
                        None,
                        MAX_LYRICS_BYTES,
                    )
                    .await
                    {
                        if let Ok(text) = String::from_utf8(raw_bytes) {
                            let clean = text.trim().trim_start_matches('\u{FEFF}').trim();
                            if !clean.is_empty() {
                                return Ok(Some(clean.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    if raw_file_path.to_ascii_lowercase().ends_with(".flac") {
        if let Ok(slice) = fetch_url_bytes_bounded(
            http,
            profile,
            track_url.as_str(),
            Some(FLAC_HEADER_RANGE_BYTES - 1),
            FLAC_HEADER_RANGE_BYTES,
        )
        .await
        {
            if let Some(tags) = parse_flac_header_bytes(&slice, None) {
                if let Some(lyrics) = tags.lyrics.filter(|s| !s.trim().is_empty()) {
                    return Ok(Some(lyrics));
                }
            }
        }
    }

    Ok(None)
}

/// Searches the configured WebDAV root across tracks, inferred albums, and artists.
pub async fn search(
    http: &Client,
    profile: &ServerProfile,
    query: &str,
    limit: usize,
) -> Result<ServerSearchResult, CoreError> {
    let needle = query.trim().to_ascii_lowercase();
    if limit == 0 {
        return Ok(ServerSearchResult::default());
    }
    if needle.is_empty() {
        let crawled = crawl_audio(http, profile, None, None, Some(limit)).await?;
        let albums = list_albums(http, profile, None, None, None)
            .await
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .collect();
        let artists = list_artists(http, profile, None, None)
            .await
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .collect();
        return Ok(ServerSearchResult {
            tracks: crawled,
            albums,
            artists,
            playlists: Vec::new(),
        });
    }

    let crawled = crawl_audio(http, profile, None, None, Some(DEFAULT_MAX_CRAWL_ITEMS)).await?;
    let tracks: Vec<Track> = crawled
        .into_iter()
        .filter(|t| {
            t.title.to_ascii_lowercase().contains(&needle)
                || t.artist.to_ascii_lowercase().contains(&needle)
                || t.album.to_ascii_lowercase().contains(&needle)
                || t.id.to_ascii_lowercase().contains(&needle)
        })
        .take(limit)
        .collect();
    let albums = list_albums(http, profile, None, None, None)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|c| {
            c.title.to_ascii_lowercase().contains(&needle)
                || c.subtitle.to_ascii_lowercase().contains(&needle)
        })
        .take(limit)
        .collect();
    let artists = list_artists(http, profile, None, None)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.title.to_ascii_lowercase().contains(&needle))
        .take(limit)
        .collect();

    Ok(ServerSearchResult {
        tracks,
        albums,
        artists,
        playlists: Vec::new(),
    })
}

/// Builds a direct WebDAV audio stream URL with `Basic` or `Bearer` auth headers.
pub fn stream_request(
    profile: &ServerProfile,
    item_id_or_path: &str,
    track: &Track,
) -> Result<ServerStream, CoreError> {
    let target_url = resolve_target_url(profile, Some(item_id_or_path))?;
    let codec = extension_of(target_url.path()).map(str::to_ascii_lowercase);
    let mime_type = codec
        .as_deref()
        .and_then(mime_for_extension)
        .map(str::to_string);
    let bitrate_kbps = track.audio_badge.as_deref().and_then(extract_badge_kbps);
    Ok(ServerStream {
        url: target_url.into(),
        headers: auth_headers(profile),
        mime_type,
        bitrate_kbps,
        codec,
        audio_badge: track.audio_badge.clone(),
    })
}

/// Pure RFC 4918 `<multistatus>` XML parser using `roxmltree`, skipping the queried root folder itself.
pub fn parse_propfind_xml(xml: &str, request_url: &str) -> Result<Vec<WebDavEntry>, CoreError> {
    let doc = Document::parse(xml).map_err(|err| {
        CoreError::invalid_response("webdav", format!("malformed PROPFIND XML: {err}"))
    })?;
    let base_url = Url::parse(request_url).map_err(|err| {
        CoreError::invalid_response(
            "webdav",
            format!("invalid request URL `{request_url}`: {err}"),
        )
    })?;
    let root_path_key = normalize_path_key(base_url.path());

    let mut entries = Vec::new();
    for response in doc
        .descendants()
        .filter(|n| n.is_element() && local_name_eq(n, "response"))
    {
        let Some(href_raw) = child_text(response, "href") else {
            continue;
        };
        let Ok(resolved_url) = base_url.join(href_raw.trim()) else {
            continue;
        };
        if !is_same_origin(&base_url, &resolved_url) {
            continue;
        }
        let entry_path = resolved_url.path().to_string();
        if normalize_path_key(&entry_path) == root_path_key {
            continue;
        }

        let props = select_ok_prop_node(response);
        let display_name = props
            .and_then(|p| child_text(p, "displayname"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback_name_from_path(&entry_path));

        let is_dir = props
            .and_then(|p| find_child_element(p, "resourcetype"))
            .is_some_and(|rt| {
                rt.children()
                    .any(|c| c.is_element() && local_name_eq(&c, "collection"))
            })
            || href_raw.trim().ends_with('/');

        let content_type = props
            .and_then(|p| child_text(p, "getcontenttype"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let content_length = props
            .and_then(|p| child_text(p, "getcontentlength"))
            .and_then(|s| s.trim().parse::<u64>().ok());
        let last_modified = props
            .and_then(|p| child_text(p, "getlastmodified"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let etag = props
            .and_then(|p| child_text(p, "getetag"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        entries.push(WebDavEntry {
            url: resolved_url.into(),
            path: entry_path,
            name: display_name,
            is_dir,
            content_type,
            content_length,
            last_modified,
            etag,
        });
    }
    Ok(entries)
}

/// Infers `(Artist, Album)` from a WebDAV path, stripping generic root/share segments and disc folders.
pub fn infer_artist_and_album(
    profile: &ServerProfile,
    raw_path: &str,
    is_file: bool,
) -> (String, String) {
    let base_path = Url::parse(&profile.normalized_base_url())
        .map(|u| u.path().to_string())
        .unwrap_or_default();
    let rel = raw_path
        .strip_prefix(base_path.trim_end_matches('/'))
        .unwrap_or(raw_path);
    let mut dirs: Vec<String> = rel
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            urlencoding::decode(s)
                .map(|d| d.into_owned())
                .unwrap_or_else(|_| s.to_string())
        })
        .collect();
    if is_file && !dirs.is_empty() {
        dirs.pop();
    }
    let start_idx = dirs
        .iter()
        .position(|seg| {
            let lower = seg.trim().to_ascii_lowercase();
            !GENERIC_CONTAINER_SEGMENTS.contains(&lower.as_str())
        })
        .unwrap_or(dirs.len());
    let mut clean: Vec<String> = dirs[start_idx..].to_vec();
    if clean.len() >= 3 && is_disc_folder(clean.last().map(String::as_str).unwrap_or("")) {
        clean.pop();
    }
    match clean.len() {
        n if n >= 2 => (clean[n - 2].clone(), clean[n - 1].clone()),
        1 => ("Unknown Artist".to_string(), clean[0].clone()),
        _ => ("Unknown Artist".to_string(), String::new()),
    }
}

/// Converts a parsed WebDAV audio [`WebDavEntry`] into Sonata's [`Track`] model.
pub fn entry_to_track(profile: &ServerProfile, entry: &WebDavEntry) -> Track {
    let raw_filename = fallback_name_from_path(&entry.path);
    let effective_name = if raw_filename.is_empty() {
        entry.name.clone()
    } else {
        raw_filename
    };
    let title = clean_track_title(&effective_name);
    let (artist, album) = infer_artist_and_album(profile, &entry.path, true);
    let parent_dir = parent_dir_of_path(&entry.path);
    let album_id = (!parent_dir.is_empty())
        .then(|| encode_server_item_id(ServerKind::WebDav, &profile.id, &parent_dir));

    let codec = extension_of(&entry.path).or_else(|| {
        entry
            .content_type
            .as_deref()
            .and_then(|ct| ct.strip_prefix("audio/"))
            .map(|s| s.trim_start_matches("x-"))
    });
    let audio_badge = format_audio_badge(codec, None, "WebDAV");

    Track {
        id: encode_server_item_id(ServerKind::WebDav, &profile.id, &entry.path),
        title,
        artist,
        album,
        duration_secs: None,
        thumbnail_url: None,
        source: ProviderSource::YouTubeMusic,
        audio_badge,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit: false,
    }
}

/// Parses artist sidecar metadata from JSON (`artist.json`, `info.json`, `metadata.json`) or XML (`artist.nfo`).
pub fn parse_artist_metadata_text(raw: &str, is_xml: bool) -> WebDavArtistMetadata {
    let cleaned = raw.trim().trim_start_matches('\u{FEFF}').trim();
    if cleaned.is_empty() {
        return WebDavArtistMetadata::default();
    }
    if !is_xml && cleaned.starts_with('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(cleaned) {
            let pick = |keys: &[&str]| -> Option<String> {
                keys.iter().find_map(|k| {
                    val.get(*k)
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
            };
            let name = pick(&["name"]);
            let bio = pick(&["bio", "biography", "description"]);
            let genre = pick(&["genre"]);
            let origin = pick(&["origin", "country", "formed"]);
            let subtitle = pick(&["subtitle"]).or_else(|| {
                let parts: Vec<String> = [genre, origin].into_iter().flatten().collect();
                (!parts.is_empty()).then(|| parts.join(" · "))
            });
            return WebDavArtistMetadata {
                name,
                bio,
                subtitle,
            };
        }
        return WebDavArtistMetadata::default();
    }

    if let Ok(doc) = Document::parse(cleaned) {
        let pick_tag = |tags: &[&str]| -> Option<String> {
            tags.iter().find_map(|tag| {
                doc.descendants()
                    .find(|n| n.is_element() && local_name_eq(n, tag))
                    .and_then(|n| {
                        let txt = n.text().map(str::trim).unwrap_or("");
                        (!txt.is_empty()).then(|| txt.to_string())
                    })
            })
        };
        let name = pick_tag(&["name"]);
        let bio = pick_tag(&["biography", "plot", "description"]);
        let genre = pick_tag(&["genre"]);
        let origin = pick_tag(&["origin", "formed", "country"]);
        let parts: Vec<String> = [genre, origin].into_iter().flatten().collect();
        let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
        return WebDavArtistMetadata {
            name,
            bio,
            subtitle,
        };
    }

    WebDavArtistMetadata::default()
}

fn parse_album_metadata_json(raw: &str) -> (Option<String>, Option<String>) {
    let Ok(val) =
        serde_json::from_str::<serde_json::Value>(raw.trim().trim_start_matches('\u{FEFF}'))
    else {
        return (None, None);
    };
    let artist = ["albumArtist", "artist"].iter().find_map(|k| {
        val.get(*k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    let album = val
        .get("album")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    (artist, album)
}

async fn collect_artist_folder_metadata(
    http: &Client,
    profile: &ServerProfile,
    entries: &[WebDavEntry],
) -> HashMap<String, WebDavArtistMetadata> {
    let mut out = HashMap::new();
    let mut by_dir: HashMap<String, Vec<&WebDavEntry>> = HashMap::new();
    for entry in entries.iter().filter(|e| !e.is_dir) {
        by_dir
            .entry(parent_dir_of_path(&entry.path))
            .or_default()
            .push(entry);
    }
    let mut reads = 0usize;
    for (dir_path, dir_entries) in by_dir {
        if reads >= 32 {
            break;
        }
        let meta_cand = ["artist.json", "artist.nfo", "info.json", "metadata.json"]
            .iter()
            .find_map(|target| {
                dir_entries
                    .iter()
                    .find(|e| fallback_name_from_path(&e.path).eq_ignore_ascii_case(target))
            });
        let Some(meta_entry) = meta_cand else {
            continue;
        };
        reads += 1;
        let is_xml = meta_entry.path.to_ascii_lowercase().ends_with(".nfo");
        if let Ok(raw) =
            fetch_url_bytes_bounded(http, profile, &meta_entry.url, None, MAX_META_TEXT_BYTES).await
        {
            if let Ok(text) = String::from_utf8(raw) {
                let parsed = parse_artist_metadata_text(&text, is_xml);
                let dir_key = fallback_name_from_path(&dir_path)
                    .trim()
                    .to_ascii_lowercase();
                if !dir_key.is_empty() {
                    out.entry(dir_key).or_insert_with(|| parsed.clone());
                }
                if let Some(ref name) = parsed.name {
                    let nk = name.trim().to_ascii_lowercase();
                    if !nk.is_empty() {
                        out.entry(nk).or_insert(parsed);
                    }
                }
            }
        }
    }
    out
}

/// Returns `true` when `entry` is a file whose MIME type or extension identifies audio media.
pub fn is_audio_entry(entry: &WebDavEntry) -> bool {
    if entry.is_dir {
        return false;
    }
    if let Some(ct) = entry.content_type.as_deref() {
        let lower = ct.trim().to_ascii_lowercase();
        if lower.starts_with("audio/")
            || lower == "application/ogg"
            || lower == "application/x-ogg"
            || lower == "application/flac"
        {
            return true;
        }
    }
    extension_of(&entry.path).is_some_and(|ext| {
        AUDIO_EXTENSIONS
            .iter()
            .any(|cand| ext.eq_ignore_ascii_case(cand))
    })
}

fn is_image_entry(entry: &WebDavEntry) -> bool {
    !entry.is_dir
        && extension_of(&entry.path).is_some_and(|ext| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|cand| ext.eq_ignore_ascii_case(cand))
        })
}

fn entry_file_stem(entry: &WebDavEntry) -> String {
    let fname = fallback_name_from_path(&entry.path);
    fname
        .rsplit_once('.')
        .map_or(fname.as_str(), |(s, _)| s)
        .to_string()
}

fn index_images_by_dir(entries: &[WebDavEntry]) -> HashMap<String, Vec<WebDavEntry>> {
    let mut map: HashMap<String, Vec<WebDavEntry>> = HashMap::new();
    for e in entries.iter().filter(|e| is_image_entry(e)) {
        map.entry(parent_dir_of_path(&e.path))
            .or_default()
            .push(e.clone());
    }
    map
}

pub(crate) fn choose_folder_cover(entries: &[WebDavEntry]) -> Option<&WebDavEntry> {
    let imgs: Vec<&WebDavEntry> = entries.iter().filter(|e| is_image_entry(e)).collect();
    if imgs.is_empty() {
        return None;
    }
    for base in COVER_PRIORITY_BASENAMES {
        if let Some(hit) = imgs
            .iter()
            .find(|e| entry_file_stem(e).eq_ignore_ascii_case(base))
        {
            return Some(hit);
        }
    }
    imgs.first().copied()
}

fn choose_artist_avatar(entries: &[WebDavEntry]) -> Option<&WebDavEntry> {
    let imgs: Vec<&WebDavEntry> = entries.iter().filter(|e| is_image_entry(e)).collect();
    for base in ARTIST_AVATAR_BASENAMES {
        if let Some(hit) = imgs
            .iter()
            .find(|e| entry_file_stem(e).eq_ignore_ascii_case(base))
        {
            return Some(hit);
        }
    }
    imgs.first().copied()
}

fn find_per_track_image<'a>(
    entries: &'a [WebDavEntry],
    track_stem: &str,
) -> Option<&'a WebDavEntry> {
    entries
        .iter()
        .find(|e| is_image_entry(e) && entry_file_stem(e).eq_ignore_ascii_case(track_stem))
}

fn find_album_metadata_entry(entries: &[WebDavEntry]) -> Option<&WebDavEntry> {
    ["album.json", "metadata.json"].iter().find_map(|cand| {
        entries
            .iter()
            .find(|e| !e.is_dir && fallback_name_from_path(&e.path).eq_ignore_ascii_case(cand))
    })
}

pub(crate) fn choose_lyric_entry<'a>(
    entries: &'a [WebDavEntry],
    file_stem: &str,
    clean_stem: &str,
    audio_count: usize,
) -> Option<&'a WebDavEntry> {
    let lyrics: Vec<&WebDavEntry> = entries
        .iter()
        .filter(|e| {
            !e.is_dir
                && extension_of(&e.path).is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("ttml")
                        || ext.eq_ignore_ascii_case("lrc")
                        || ext.eq_ignore_ascii_case("txt")
                })
        })
        .collect();
    let is_ttml =
        |e: &WebDavEntry| extension_of(&e.path).is_some_and(|ext| ext.eq_ignore_ascii_case("ttml"));
    let is_lrc =
        |e: &WebDavEntry| extension_of(&e.path).is_some_and(|ext| ext.eq_ignore_ascii_case("lrc"));

    lyrics
        .iter()
        .copied()
        .find(|e| is_ttml(e) && entry_file_stem(e).eq_ignore_ascii_case(file_stem))
        .or_else(|| {
            lyrics
                .iter()
                .copied()
                .find(|e| is_ttml(e) && entry_file_stem(e).eq_ignore_ascii_case(clean_stem))
        })
        .or_else(|| {
            lyrics
                .iter()
                .copied()
                .find(|e| is_lrc(e) && entry_file_stem(e).eq_ignore_ascii_case(file_stem))
        })
        .or_else(|| {
            lyrics
                .iter()
                .copied()
                .find(|e| is_lrc(e) && entry_file_stem(e).eq_ignore_ascii_case(clean_stem))
        })
        .or_else(|| {
            lyrics.iter().copied().find(|e| {
                entry_file_stem(e).eq_ignore_ascii_case(file_stem)
                    || entry_file_stem(e).eq_ignore_ascii_case(clean_stem)
            })
        })
        .or_else(|| {
            lyrics
                .iter()
                .copied()
                .find(|e| fallback_name_from_path(&e.path).eq_ignore_ascii_case("lyrics.ttml"))
        })
        .or_else(|| {
            lyrics
                .iter()
                .copied()
                .find(|e| fallback_name_from_path(&e.path).eq_ignore_ascii_case("lyrics.lrc"))
        })
        .or_else(|| {
            if audio_count == 1 {
                let only_ttml: Vec<&WebDavEntry> =
                    lyrics.iter().copied().filter(|e| is_ttml(e)).collect();
                if only_ttml.len() == 1 {
                    return only_ttml.first().copied();
                }
                let only_lrc: Vec<&WebDavEntry> =
                    lyrics.iter().copied().filter(|e| is_lrc(e)).collect();
                if only_lrc.len() == 1 {
                    only_lrc.first().copied()
                } else {
                    None
                }
            } else {
                None
            }
        })
}

fn parent_dir_of_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some((parent, _)) if !parent.is_empty() => parent.to_string(),
        _ => String::new(),
    }
}

fn is_disc_folder(seg: &str) -> bool {
    let clean = seg.trim().to_ascii_lowercase();
    for prefix in ["cd", "disc", "disk"] {
        if let Some(rest) = clean.strip_prefix(prefix) {
            let trimmed = rest.trim();
            if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn build_listing(
    profile: &ServerProfile,
    folder_url: &Url,
    entries: Vec<WebDavEntry>,
) -> WebDavListing {
    let mut folders = Vec::new();
    let mut tracks = Vec::new();
    for entry in &entries {
        if entry.is_dir {
            folders.push(entry.clone());
        } else if is_audio_entry(entry) {
            tracks.push(entry_to_track(profile, entry));
        }
    }
    WebDavListing {
        folder_url: folder_url.to_string(),
        folders,
        tracks,
        entries,
    }
}

fn local_name_eq(node: &Node<'_, '_>, expected: &str) -> bool {
    node.tag_name().name().eq_ignore_ascii_case(expected)
}

fn find_child_element<'a>(parent: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    parent
        .children()
        .find(|n| n.is_element() && local_name_eq(n, name))
}

fn child_text(parent: Node<'_, '_>, name: &str) -> Option<String> {
    let node = find_child_element(parent, name)?;
    let collected = node
        .children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>();
    (!collected.is_empty()).then_some(collected)
}

fn select_ok_prop_node<'a>(response: Node<'a, 'a>) -> Option<Node<'a, 'a>> {
    let mut fallback = None;
    for ps in response
        .children()
        .filter(|n| n.is_element() && local_name_eq(n, "propstat"))
    {
        let prop = find_child_element(ps, "prop");
        let is_ok = child_text(ps, "status")
            .is_some_and(|status| status.contains(" 200 ") || status.ends_with(" 200"));
        if is_ok {
            return prop;
        }
        if fallback.is_none() {
            fallback = prop;
        }
    }
    fallback
}

fn normalize_path_key(path: &str) -> String {
    let decoded = urlencoding::decode(path)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| path.to_string());
    let trimmed = decoded.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn fallback_name_from_path(path: &str) -> String {
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    urlencoding::decode(last)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| last.to_string())
}

fn extension_of(path: &str) -> Option<&str> {
    let no_query = path.split('?').next().unwrap_or(path);
    let file = no_query.rsplit('/').next()?;
    let (_, ext) = file.rsplit_once('.')?;
    (!ext.is_empty()).then_some(ext)
}

fn clean_track_title(filename: &str) -> String {
    let stem = match filename.rsplit_once('.') {
        Some((s, ext)) if AUDIO_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)) => s,
        _ => filename,
    };
    let trimmed = stem.trim();
    // Strip common leading track numbers such as "01 - Title", "01. Title", "1-02 Title".
    let bytes = trimmed.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && (bytes[idx].is_ascii_digit() || bytes[idx] == b'-') {
        idx += 1;
    }
    if idx > 0 && idx < trimmed.len() && trimmed[..idx].bytes().any(|b| b.is_ascii_digit()) {
        let rest = trimmed[idx..].trim_start();
        if let Some(after_sep) = rest
            .strip_prefix('-')
            .or_else(|| rest.strip_prefix('.'))
            .map(str::trim)
        {
            if !after_sep.is_empty() {
                return after_sep.to_string();
            }
        }
        if rest.len() < trimmed[idx..].len() && !rest.is_empty() {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

fn mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "mp3" => Some("audio/mpeg"),
        "flac" => Some("audio/flac"),
        "opus" => Some("audio/opus"),
        "ogg" | "oga" => Some("audio/ogg"),
        "m4a" | "aac" => Some("audio/mp4"),
        "wav" => Some("audio/wav"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> ServerProfile {
        let mut p = ServerProfile::new(
            "dav-1",
            "Nextcloud Music",
            ServerKind::WebDav,
            "https://dav.example.com/remote.php/dav/files/alice/Music",
        );
        p.username = Some("alice".to_string());
        p.password = Some("dav-secret".to_string());
        p
    }

    #[test]
    fn parses_propfind_multistatus_xml_and_skips_root() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/alice/Music/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/alice/Music/M83/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>M83</d:displayname>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/alice/Music/M83/Hurry%20Up/01%20-%20Midnight%20City.flac</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontenttype>audio/flac</d:getcontenttype>
        <d:getcontentlength>31457280</d:getcontentlength>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
    <d:propstat>
      <d:prop><d:displayname/></d:prop>
      <d:status>HTTP/1.1 404 Not Found</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/alice/Music/cover.jpg</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontenttype>image/jpeg</d:getcontenttype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let req_url = "https://dav.example.com/remote.php/dav/files/alice/Music/";
        let entries = parse_propfind_xml(xml, req_url).expect("parse propfind");
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "M83");
        assert!(!entries[1].is_dir);
        assert!(is_audio_entry(&entries[1]));
        assert_eq!(entries[1].content_length, Some(31_457_280));
        assert!(!is_audio_entry(&entries[2]));

        let profile = test_profile();
        let track = entry_to_track(&profile, &entries[1]);
        assert_eq!(track.title, "Midnight City");
        assert_eq!(track.artist, "M83");
        assert_eq!(track.album, "Hurry Up");
        assert_eq!(track.source_tag(), "DAV");
        assert_eq!(track.audio_badge.as_deref(), Some("FLAC"));
        assert_eq!(
            track.id,
            "webdav:dav-1:/remote.php/dav/files/alice/Music/M83/Hurry%20Up/01%20-%20Midnight%20City.flac"
        );
    }

    #[test]
    fn infers_artist_and_album_skipping_containers_and_disc_folders() {
        let profile = test_profile();
        assert_eq!(
            infer_artist_and_album(
                &profile,
                "/remote.php/dav/files/alice/Music/share/Lossless/Eminem/The%20Slim%20Shady%20LP/CD1/01.flac",
                true
            ),
            ("Eminem".to_string(), "The Slim Shady LP".to_string())
        );
        assert_eq!(
            infer_artist_and_album(
                &profile,
                "/remote.php/dav/files/alice/Music/Singles/01.flac",
                true
            ),
            ("Unknown Artist".to_string(), "Singles".to_string())
        );
    }

    #[test]
    fn parses_artist_json_and_nfo_metadata() {
        let json_raw = r#"{
            "name": "Eminem",
            "genre": "Hip-Hop / Rap",
            "origin": "Detroit, Michigan",
            "bio": "Marshall Bruce Mathers III."
        }"#;
        let jm = parse_artist_metadata_text(json_raw, false);
        assert_eq!(jm.name.as_deref(), Some("Eminem"));
        assert_eq!(
            jm.subtitle.as_deref(),
            Some("Hip-Hop / Rap · Detroit, Michigan")
        );
        assert_eq!(jm.bio.as_deref(), Some("Marshall Bruce Mathers III."));

        let nfo_raw = r#"<artist>
            <name>Daft Punk</name>
            <genre>Electronic</genre>
            <formed>Paris, France</formed>
            <biography>French electronic duo.</biography>
        </artist>"#;
        let xm = parse_artist_metadata_text(nfo_raw, true);
        assert_eq!(xm.name.as_deref(), Some("Daft Punk"));
        assert_eq!(xm.subtitle.as_deref(), Some("Electronic · Paris, France"));
        assert_eq!(xm.bio.as_deref(), Some("French electronic duo."));
    }

    #[test]
    fn parses_flac_header_bytes_and_computes_kbps() {
        let mut raw = Vec::new();
        raw.extend_from_slice(b"fLaC");
        // STREAMINFO (34 bytes): 44100 Hz, 2ch, 16-bit, 8,820,000 samples (= 200s)
        raw.extend_from_slice(&[0x00, 0x00, 0x00, 34]);
        let mut si = [0u8; 34];
        si[10] = 0x0A;
        si[11] = 0xC4;
        si[12] = 0x42;
        si[13] = 0xF0;
        let samples: u32 = 8_820_000;
        si[14..18].copy_from_slice(&samples.to_be_bytes());
        raw.extend_from_slice(&si);

        // VORBIS_COMMENT (last block)
        let mut vc = Vec::new();
        vc.extend_from_slice(&4u32.to_le_bytes());
        vc.extend_from_slice(b"Lavf");
        let tags = [
            "TITLE=Guilty Conscience",
            "ARTIST=Eminem",
            "ALBUM=The Slim Shady LP",
            "LYRICS=[00:02.00] Meet Eddie",
        ];
        vc.extend_from_slice(&(tags.len() as u32).to_le_bytes());
        for t in tags {
            vc.extend_from_slice(&(t.len() as u32).to_le_bytes());
            vc.extend_from_slice(t.as_bytes());
        }
        let len_be = (vc.len() as u32).to_be_bytes();
        raw.extend_from_slice(&[0x84, len_be[1], len_be[2], len_be[3]]);
        raw.extend_from_slice(&vc);

        let parsed = parse_flac_header_bytes(&raw, Some(22_200_000)).expect("flac header");
        assert_eq!(parsed.title.as_deref(), Some("Guilty Conscience"));
        assert_eq!(parsed.artist.as_deref(), Some("Eminem"));
        assert_eq!(parsed.album.as_deref(), Some("The Slim Shady LP"));
        assert_eq!(parsed.duration_secs, Some(200));
        assert_eq!(parsed.sample_rate, Some(44_100));
        assert_eq!(parsed.bits_per_sample, Some(16));
        assert_eq!(parsed.bitrate_kbps, Some(888));
        assert_eq!(parsed.lyrics.as_deref(), Some("[00:02.00] Meet Eddie"));
    }

    #[test]
    fn builds_basic_and_bearer_auth_stream_requests() {
        let basic_profile = test_profile();
        let dummy_track = Track {
            id: "webdav:dav-1:/remote.php/dav/files/alice/Music/song.opus".to_string(),
            title: "song".to_string(),
            artist: "Unknown Artist".to_string(),
            album: String::new(),
            duration_secs: None,
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: Some("OPUS".to_string()),
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit: false,
        };

        let basic_stream = stream_request(
            &basic_profile,
            "/remote.php/dav/files/alice/Music/song.opus",
            &dummy_track,
        )
        .expect("basic stream");
        assert_eq!(
            basic_stream.url,
            "https://dav.example.com/remote.php/dav/files/alice/Music/song.opus"
        );
        assert_eq!(basic_stream.mime_type.as_deref(), Some("audio/opus"));
        assert_eq!(
            basic_stream.headers,
            vec![(
                "Authorization".to_string(),
                format!("Basic {}", BASE64_STANDARD.encode(b"alice:dav-secret"))
            )]
        );

        let mut bearer_profile = basic_profile;
        bearer_profile.bearer_token = Some("jwt-token-xyz".to_string());
        let bearer_stream = stream_request(
            &bearer_profile,
            "/remote.php/dav/files/alice/Music/song.opus",
            &dummy_track,
        )
        .expect("bearer stream");
        assert_eq!(
            bearer_stream.headers,
            vec![(
                "Authorization".to_string(),
                "Bearer jwt-token-xyz".to_string()
            )]
        );
    }

    #[test]
    fn prefers_ttml_sidecar_over_lrc_for_same_track_stem() {
        let entries = vec![
            WebDavEntry {
                url: "https://dav.example.com/Music/track.lrc".to_string(),
                path: "/Music/track.lrc".to_string(),
                name: "track.lrc".to_string(),
                is_dir: false,
                content_type: None,
                content_length: None,
                last_modified: None,
                etag: None,
            },
            WebDavEntry {
                url: "https://dav.example.com/Music/track.ttml".to_string(),
                path: "/Music/track.ttml".to_string(),
                name: "track.ttml".to_string(),
                is_dir: false,
                content_type: None,
                content_length: None,
                last_modified: None,
                etag: None,
            },
        ];
        let chosen = choose_lyric_entry(&entries, "track", "track", 1).expect("ttml chosen");
        assert_eq!(chosen.name, "track.ttml");
    }

    #[test]
    fn rejects_cross_origin_targets_and_propfind_hrefs() {
        let profile = test_profile();
        assert!(resolve_target_url(&profile, Some("https://evil.example/a.flac")).is_err());
        assert!(resolve_target_url(&profile, Some("//attacker.example/a.flac")).is_err());
        assert!(resolve_target_url(&profile, Some("http://dav.example.com/a.flac")).is_err());
        assert!(resolve_target_url(&profile, Some("https://dav.example.com:8443/a.flac")).is_err());
        let ok = resolve_target_url(
            &profile,
            Some("https://dav.example.com/remote.php/dav/files/alice/Music/a.flac"),
        )
        .expect("same-origin url allowed");
        assert_eq!(
            ok.as_str(),
            "https://dav.example.com/remote.php/dav/files/alice/Music/a.flac"
        );

        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>https://evil.example/steal-creds.flac</d:href>
    <d:propstat><d:prop><d:displayname>evil</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>//attacker.example/proto-rel.flac</d:href>
    <d:propstat><d:prop><d:displayname>proto</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/alice/Music/good.flac</d:href>
    <d:propstat><d:prop><d:displayname>good.flac</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
</d:multistatus>"#;
        let entries = parse_propfind_xml(
            xml,
            "https://dav.example.com/remote.php/dav/files/alice/Music/",
        )
        .expect("propfind parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "good.flac");
    }

    #[tokio::test]
    async fn authenticated_requests_do_not_follow_redirects() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let trap_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind trap");
        let trap_addr = trap_listener.local_addr().expect("trap addr");
        let trap_hit = Arc::new(AtomicBool::new(false));
        let trap_flag = Arc::clone(&trap_hit);
        let trap_task = tokio::spawn(async move {
            if let Ok((mut sock, _)) = trap_listener.accept().await {
                trap_flag.store(true, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf).await;
                let _ = sock
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .await;
            }
        });

        let redirect_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect");
        let redirect_addr = redirect_listener.local_addr().expect("redirect addr");
        let redirect_task = tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((mut sock, _)) = redirect_listener.accept().await {
                    let mut buf = [0u8; 2048];
                    let _ = sock.read(&mut buf).await;
                    let resp = format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{trap_addr}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                }
            }
        });

        let mut profile = ServerProfile::new(
            "dav-redir",
            "Redirect WebDAV",
            ServerKind::WebDav,
            format!("http://{redirect_addr}"),
        );
        profile.username = Some("alice".to_string());
        profile.password = Some("test-password".to_string());

        let http = crate::providers::build_http_client();
        let err = fetch_url_bytes_bounded(
            &http,
            &profile,
            &format!("http://{redirect_addr}/Music/track.flac"),
            None,
            1024,
        )
        .await
        .expect_err("302 redirect must not be followed");
        assert!(err.to_string().contains("302"));
        assert!(!trap_hit.load(Ordering::SeqCst));

        redirect_task.abort();
        trap_task.abort();
    }
}
