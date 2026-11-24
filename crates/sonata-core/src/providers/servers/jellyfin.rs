//! Jellyfin REST API client and pure JSON parsers for Sonata.
//!
//! 1. **Authentication**: Supports `Users/AuthenticateByName` credential exchange and static
//!    `api_key` / `access_token` headers (`X-Emby-Token`, `Authorization: MediaBrowser ...`).
//! 2. **Library Browsing**: Queries music libraries, album artists, albums, album detail tracks,
//!    playlists, playlist tracks, and global catalog search.
//! 3. **Media & Stream Mapping**: Maps Jellyfin items into `Track`, `Playlist`, `HomeCard`,
//!    `AlbumDetail`, `ServerLibrary`, and `ServerStream` with direct stream URLs and auth headers.

use super::{
    default_http_timeout, encode_server_item_id, format_audio_badge, map_http_status, ServerKind,
    ServerLibrary, ServerProfile, ServerSearchResult, ServerStream,
};
use crate::error::CoreError;
use crate::providers::ytmusic::{AlbumDetail, HomeCard, HomeCardKind};
use crate::providers::{Playlist, ProviderSource, Track};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_DEVICE_ID: &str = "sonata-core-client";
const CLIENT_NAME: &str = "Sonata";
const CLIENT_VERSION: &str = "1.0";

/// Result of calling Jellyfin `POST /Users/AuthenticateByName`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JellyfinAuthResult {
    pub access_token: String,
    pub user_id: String,
    pub user_name: Option<String>,
    pub server_id: Option<String>,
}

/// Builds the Jellyfin `MediaBrowser` authorization header value.
pub fn build_mediabrowser_header(device_id: &str, token: Option<&str>) -> String {
    let dev = if device_id.trim().is_empty() {
        DEFAULT_DEVICE_ID
    } else {
        device_id.trim()
    };
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(tok) => format!(
            "MediaBrowser Client=\"{CLIENT_NAME}\", Device=\"{CLIENT_NAME}\", DeviceId=\"{dev}\", Version=\"{CLIENT_VERSION}\", Token=\"{tok}\""
        ),
        None => format!(
            "MediaBrowser Client=\"{CLIENT_NAME}\", Device=\"{CLIENT_NAME}\", DeviceId=\"{dev}\", Version=\"{CLIENT_VERSION}\""
        ),
    }
}

fn active_token(profile: &ServerProfile) -> Option<&str> {
    profile
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            profile
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
}

fn auth_headers(profile: &ServerProfile) -> Result<Vec<(String, String)>, CoreError> {
    let token = active_token(profile).ok_or_else(|| {
        CoreError::auth_required("Jellyfin profile requires an access_token or api_key")
    })?;
    Ok(vec![
        ("X-Emby-Token".to_string(), token.to_string()),
        (
            "Authorization".to_string(),
            build_mediabrowser_header(&profile.id, Some(token)),
        ),
    ])
}

fn apply_auth(req: RequestBuilder, profile: &ServerProfile) -> Result<RequestBuilder, CoreError> {
    let headers = auth_headers(profile)?;
    let mut header_map = HeaderMap::new();
    for (name, value) in headers {
        let hdr_name = reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
            CoreError::internal(format!("invalid Jellyfin header name `{name}`: {err}"))
        })?;
        let hdr_val = HeaderValue::from_str(&value).map_err(|err| {
            CoreError::auth_required(format!("invalid Jellyfin token header value: {err}"))
        })?;
        header_map.insert(hdr_name, hdr_val);
    }
    Ok(req.headers(header_map))
}

async fn send_json_get(
    http: &Client,
    url: &str,
    profile: &ServerProfile,
) -> Result<Value, CoreError> {
    let req = http.get(url).timeout(default_http_timeout());
    let req = apply_auth(req, profile)?;
    let resp = req
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("jellyfin", &err))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|err| CoreError::from_reqwest("jellyfin", &err))?;
    if !status.is_success() {
        return Err(map_http_status("jellyfin", status, &body));
    }
    serde_json::from_str(&body).map_err(|err| {
        CoreError::invalid_response("jellyfin", format!("malformed Jellyfin JSON: {err}"))
    })
}

/// Exchanges a Jellyfin username and password for an access token via `/Users/AuthenticateByName`.
pub async fn authenticate_by_name(
    http: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    device_id: &str,
) -> Result<JellyfinAuthResult, CoreError> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(CoreError::invalid_response(
            "jellyfin",
            "Jellyfin base URL is empty",
        ));
    }
    let url = format!("{base}/Users/AuthenticateByName");
    let resp = http
        .post(&url)
        .timeout(default_http_timeout())
        .header("Authorization", build_mediabrowser_header(device_id, None))
        .header("Content-Type", "application/json")
        .json(&json!({
            "Username": username,
            "Pw": password,
        }))
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("jellyfin", &err))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|err| CoreError::from_reqwest("jellyfin", &err))?;
    if !status.is_success() {
        return Err(map_http_status("jellyfin", status, &body));
    }
    parse_auth_response_json(&body)
}

/// Checks connectivity and credentials via `GET /System/Info` (falling back to `/Users/Me`).
pub async fn ping(http: &Client, profile: &ServerProfile) -> Result<String, CoreError> {
    let base = profile.normalized_base_url();
    if base.is_empty() {
        return Err(CoreError::invalid_response(
            "jellyfin",
            "Jellyfin base URL is empty",
        ));
    }
    let url = format!("{base}/System/Info");
    let val = send_json_get(http, &url, profile).await?;
    let server_name = val
        .get("ServerName")
        .and_then(Value::as_str)
        .unwrap_or("Jellyfin");
    let version = val
        .get("Version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    Ok(format!("{server_name} ({version})"))
}

/// Fetches music libraries visible to the configured user or API key.
pub async fn get_music_libraries(
    http: &Client,
    profile: &ServerProfile,
) -> Result<Vec<ServerLibrary>, CoreError> {
    let base = profile.normalized_base_url();
    let url = match profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(uid) => format!("{base}/Users/{}/Views", urlencoding::encode(uid)),
        None => format!("{base}/Library/MediaFolders"),
    };
    let val = send_json_get(http, &url, profile).await?;
    parse_libraries_json(&val)
}

/// Fetches album artists from Jellyfin and maps them to `HomeCard` (`HomeCardKind::Artist`).
pub async fn get_artists(
    http: &Client,
    profile: &ServerProfile,
    library_id: Option<&str>,
) -> Result<Vec<HomeCard>, CoreError> {
    let base = profile.normalized_base_url();
    let mut url = format!(
        "{base}/Artists/AlbumArtists?Recursive=true&Fields=PrimaryImageAspectRatio,Overview,Genres&Limit=200"
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        url.push_str(&format!("&UserId={}", urlencoding::encode(uid)));
    }
    if let Some(lib) = library_id.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&ParentId={}", urlencoding::encode(lib)));
    }
    let val = send_json_get(http, &url, profile).await?;
    parse_artists_json(profile, &val)
}

/// Fetches albums from Jellyfin filtered by optional library or artist ID.
pub async fn get_albums(
    http: &Client,
    profile: &ServerProfile,
    library_id: Option<&str>,
    artist_id: Option<&str>,
) -> Result<Vec<HomeCard>, CoreError> {
    let base = profile.normalized_base_url();
    let mut url = format!(
        "{base}/Items?IncludeItemTypes=MusicAlbum&Recursive=true&SortBy=SortName&SortOrder=Ascending&Fields=PrimaryImageAspectRatio,ProductionYear,AlbumArtist,Artists&Limit=200"
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        url.push_str(&format!("&UserId={}", urlencoding::encode(uid)));
    }
    if let Some(lib) = library_id.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&ParentId={}", urlencoding::encode(lib)));
    }
    if let Some(aid) = artist_id.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&AlbumArtistIds={}", urlencoding::encode(aid)));
    }
    let val = send_json_get(http, &url, profile).await?;
    parse_albums_json(profile, &val)
}

/// Fetches full album metadata and ordered tracks for a Jellyfin `MusicAlbum` item.
pub async fn get_album_detail(
    http: &Client,
    profile: &ServerProfile,
    album_id: &str,
) -> Result<AlbumDetail, CoreError> {
    let base = profile.normalized_base_url();
    let encoded_album = urlencoding::encode(album_id);
    let mut meta_url = format!(
        "{base}/Items?Ids={encoded_album}&Fields=PrimaryImageAspectRatio,ProductionYear,AlbumArtist,Artists,Overview"
    );
    let mut tracks_url = format!(
        "{base}/Items?ParentId={encoded_album}&IncludeItemTypes=Audio&Recursive=true&SortBy=ParentIndexNumber,IndexNumber,SortName&Fields=MediaSources,RunTimeTicks,Artists,Album,AlbumId,AlbumPrimaryImageTag"
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let enc_uid = urlencoding::encode(uid);
        meta_url.push_str(&format!("&UserId={enc_uid}"));
        tracks_url.push_str(&format!("&UserId={enc_uid}"));
    }

    let (meta_res, tracks_res) = tokio::join!(
        send_json_get(http, &meta_url, profile),
        send_json_get(http, &tracks_url, profile),
    );
    let meta_val = meta_res?;
    let tracks_val = tracks_res?;
    parse_album_detail_json(profile, album_id, &meta_val, &tracks_val)
}

/// Fetches Jellyfin playlists mapped to Sonata `Playlist` entries.
pub async fn get_playlists(
    http: &Client,
    profile: &ServerProfile,
) -> Result<Vec<Playlist>, CoreError> {
    let base = profile.normalized_base_url();
    let mut url = format!(
        "{base}/Items?IncludeItemTypes=Playlist&Recursive=true&Fields=PrimaryImageAspectRatio,ChildCount&Limit=200"
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        url.push_str(&format!("&UserId={}", urlencoding::encode(uid)));
    }
    let val = send_json_get(http, &url, profile).await?;
    parse_playlists_json(profile, &val)
}

/// Fetches ordered audio tracks inside a Jellyfin playlist.
pub async fn get_playlist_tracks(
    http: &Client,
    profile: &ServerProfile,
    playlist_id: &str,
) -> Result<Vec<Track>, CoreError> {
    let base = profile.normalized_base_url();
    let mut url = format!(
        "{base}/Playlists/{}/Items?Fields=MediaSources,RunTimeTicks,Artists,Album,AlbumId,AlbumPrimaryImageTag",
        urlencoding::encode(playlist_id)
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        url.push_str(&format!("&UserId={}", urlencoding::encode(uid)));
    }
    let val = send_json_get(http, &url, profile).await?;
    parse_tracks_json(profile, &val)
}

/// Searches Jellyfin audio tracks, albums, artists, and playlists.
pub async fn search(
    http: &Client,
    profile: &ServerProfile,
    query: &str,
    limit: usize,
) -> Result<ServerSearchResult, CoreError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(ServerSearchResult::default());
    }
    let base = profile.normalized_base_url();
    let mut url = format!(
        "{base}/Items?SearchTerm={}&IncludeItemTypes=Audio,MusicAlbum,MusicArtist,Playlist&Recursive=true&Limit={limit}&Fields=MediaSources,RunTimeTicks,Artists,Album,AlbumId,AlbumPrimaryImageTag,ProductionYear,AlbumArtist,ChildCount",
        urlencoding::encode(trimmed)
    );
    if let Some(uid) = profile
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        url.push_str(&format!("&UserId={}", urlencoding::encode(uid)));
    }
    let val = send_json_get(http, &url, profile).await?;
    parse_search_json(profile, &val)
}

/// Builds a Jellyfin primary artwork URL for `item_id` with optional image cache tag.
pub fn artwork_url(base_url: &str, item_id: &str, tag: Option<&str>, max_width: u32) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let mut url = format!(
        "{base}/Items/{}/Images/Primary?maxWidth={max_width}&quality=90",
        urlencoding::encode(item_id)
    );
    if let Some(t) = tag.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&tag={}", urlencoding::encode(t)));
    }
    url
}

/// Builds a direct Jellyfin audio stream URL and authentication headers for `item_id`.
pub fn stream_request(
    profile: &ServerProfile,
    item_id: &str,
    track: &Track,
) -> Result<ServerStream, CoreError> {
    let base = profile.normalized_base_url();
    if base.is_empty() {
        return Err(CoreError::invalid_response(
            "jellyfin",
            "Jellyfin base URL is empty",
        ));
    }
    let headers = auth_headers(profile)?;
    let url = format!(
        "{base}/Audio/{}/stream?static=true",
        urlencoding::encode(item_id)
    );
    Ok(ServerStream {
        url,
        headers,
        mime_type: None,
        bitrate_kbps: None,
        codec: None,
        audio_badge: track.audio_badge.clone(),
    })
}

/// Pure JSON parser for `POST /Users/AuthenticateByName` responses.
pub fn parse_auth_response_json(body: &str) -> Result<JellyfinAuthResult, CoreError> {
    let val: Value = serde_json::from_str(body).map_err(|err| {
        CoreError::invalid_response("jellyfin", format!("malformed auth JSON: {err}"))
    })?;
    let access_token = val
        .get("AccessToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            CoreError::invalid_response("jellyfin", "auth response missing `AccessToken`")
        })?
        .to_string();
    let user_id = val
        .pointer("/User/Id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::invalid_response("jellyfin", "auth response missing `User.Id`"))?
        .to_string();
    let user_name = val
        .pointer("/User/Name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let server_id = val
        .get("ServerId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(JellyfinAuthResult {
        access_token,
        user_id,
        user_name,
        server_id,
    })
}

/// Pure JSON parser for Jellyfin music library views.
pub fn parse_libraries_json(val: &Value) -> Result<Vec<ServerLibrary>, CoreError> {
    let items = val.get("Items").and_then(Value::as_array).ok_or_else(|| {
        CoreError::invalid_response("jellyfin", "libraries response missing `Items` array")
    })?;
    let mut all = Vec::new();
    let mut music_only = Vec::new();
    for item in items {
        let Some(id) = item.get("Id").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let name = item
            .get("Name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Music")
            .to_string();
        let kind = item
            .get("CollectionType")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let lib = ServerLibrary {
            id: id.to_string(),
            name,
            kind: kind.clone(),
        };
        if kind.eq_ignore_ascii_case("music") {
            music_only.push(lib.clone());
        }
        all.push(lib);
    }
    Ok(if music_only.is_empty() {
        all
    } else {
        music_only
    })
}

/// Pure JSON parser for Jellyfin artist list responses.
pub fn parse_artists_json(
    profile: &ServerProfile,
    val: &Value,
) -> Result<Vec<HomeCard>, CoreError> {
    let items = items_array(val)?;
    Ok(items
        .iter()
        .filter_map(|item| parse_artist_card(profile, item))
        .collect())
}

/// Pure JSON parser for Jellyfin album list responses.
pub fn parse_albums_json(profile: &ServerProfile, val: &Value) -> Result<Vec<HomeCard>, CoreError> {
    let items = items_array(val)?;
    Ok(items
        .iter()
        .filter_map(|item| parse_album_card(profile, item))
        .collect())
}

/// Pure JSON parser for Jellyfin playlist list responses.
pub fn parse_playlists_json(
    profile: &ServerProfile,
    val: &Value,
) -> Result<Vec<Playlist>, CoreError> {
    let items = items_array(val)?;
    let base = profile.normalized_base_url();
    let mut playlists = Vec::new();
    for item in items {
        let Some(raw_id) = item_id_str(item) else {
            continue;
        };
        let name = item_name_str(item).unwrap_or("Playlist").to_string();
        let track_count = item
            .get("ChildCount")
            .and_then(Value::as_u64)
            .map(|c| c as usize);
        let thumbnail_url = item_primary_thumb(&base, item, raw_id);
        playlists.push(Playlist {
            id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, raw_id),
            name,
            source: ProviderSource::YouTubeMusic,
            track_count,
            thumbnail_url,
            is_owned: true,
        });
    }
    Ok(playlists)
}

/// Pure JSON parser for Jellyfin audio track list responses.
pub fn parse_tracks_json(profile: &ServerProfile, val: &Value) -> Result<Vec<Track>, CoreError> {
    let items = items_array(val)?;
    Ok(items
        .iter()
        .filter_map(|item| parse_audio_track(profile, item))
        .collect())
}

/// Pure JSON parser combining Jellyfin album metadata and tracks into [`AlbumDetail`].
pub fn parse_album_detail_json(
    profile: &ServerProfile,
    album_id: &str,
    meta_val: &Value,
    tracks_val: &Value,
) -> Result<AlbumDetail, CoreError> {
    let tracks = parse_tracks_json(profile, tracks_val)?;
    let header = items_array(meta_val).ok().and_then(|arr| arr.first());
    let base = profile.normalized_base_url();

    let title = header
        .and_then(item_name_str)
        .or_else(|| {
            tracks
                .first()
                .map(|t| t.album.as_str())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("Album")
        .to_string();
    let year = header
        .and_then(|h| h.get("ProductionYear").and_then(Value::as_u64))
        .map(|y| y.to_string());
    let artist_name = header
        .and_then(|h| {
            h.get("AlbumArtist")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| tracks.first().map(|t| t.artist.as_str()))
        .unwrap_or("Unknown Artist");
    let cover_url = header
        .and_then(|h| item_primary_thumb(&base, h, album_id))
        .or_else(|| tracks.first().and_then(|t| t.thumbnail_url.clone()));

    let total_secs: u64 = tracks.iter().filter_map(|t| t.duration_secs).sum();
    let duration_text = if total_secs > 0 {
        Some(format!("{} min", (total_secs + 30) / 60))
    } else {
        None
    };

    let mut artists = Vec::new();
    if let Some(h) = header {
        if let Some(arr) = h.get("AlbumArtists").and_then(Value::as_array) {
            for a in arr {
                if let Some(name) = a.get("Name").and_then(Value::as_str).map(str::trim) {
                    if !name.is_empty() {
                        let aid = a
                            .get("Id")
                            .and_then(Value::as_str)
                            .map(|id| encode_server_item_id(ServerKind::Jellyfin, &profile.id, id));
                        artists.push((name.to_string(), aid));
                    }
                }
            }
        }
    }
    if artists.is_empty() {
        artists.push((artist_name.to_string(), None));
    }

    let subtitle = match &year {
        Some(y) => format!("Album • {artist_name} • {y}"),
        None => format!("Album • {artist_name}"),
    };

    Ok(AlbumDetail {
        id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, album_id),
        title,
        subtitle,
        release_type: "Album".to_string(),
        year,
        duration_text,
        cover_url,
        artists,
        tracks,
        other_versions: Vec::new(),
        playlist_id: None,
        saved: false,
    })
}

/// Pure JSON parser for Jellyfin mixed search responses (`Audio`, `MusicAlbum`, `MusicArtist`, `Playlist`).
pub fn parse_search_json(
    profile: &ServerProfile,
    val: &Value,
) -> Result<ServerSearchResult, CoreError> {
    let items = items_array(val)?;
    let base = profile.normalized_base_url();
    let mut out = ServerSearchResult::default();
    for item in items {
        let item_type = item.get("Type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "Audio" => {
                if let Some(track) = parse_audio_track(profile, item) {
                    out.tracks.push(track);
                }
            }
            "MusicAlbum" => {
                if let Some(card) = parse_album_card(profile, item) {
                    out.albums.push(card);
                }
            }
            "MusicArtist" => {
                if let Some(card) = parse_artist_card(profile, item) {
                    out.artists.push(card);
                }
            }
            "Playlist" => {
                if let Some(raw_id) = item_id_str(item) {
                    let title = item_name_str(item).unwrap_or("Playlist").to_string();
                    out.playlists.push(HomeCard {
                        kind: HomeCardKind::Playlist,
                        id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, raw_id),
                        title,
                        subtitle: "Jellyfin Playlist".to_string(),
                        thumbnail_url: item_primary_thumb(&base, item, raw_id),
                        track: None,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

fn items_array(val: &Value) -> Result<&Vec<Value>, CoreError> {
    val.get("Items")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::invalid_response("jellyfin", "response missing `Items` array"))
}

fn item_id_str(item: &Value) -> Option<&str> {
    item.get("Id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn item_name_str(item: &Value) -> Option<&str> {
    item.get("Name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn item_primary_thumb(base_url: &str, item: &Value, item_id: &str) -> Option<String> {
    let tag = item
        .pointer("/ImageTags/Primary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if tag.is_some() {
        return Some(artwork_url(base_url, item_id, tag, 800));
    }
    let album_id = item
        .get("AlbumId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let album_tag = item
        .get("AlbumPrimaryImageTag")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match (album_id, album_tag) {
        (Some(aid), Some(atag)) => Some(artwork_url(base_url, aid, Some(atag), 800)),
        _ => None,
    }
}

fn parse_artist_card(profile: &ServerProfile, item: &Value) -> Option<HomeCard> {
    let raw_id = item_id_str(item)?;
    let title = item_name_str(item)?.to_string();
    let base = profile.normalized_base_url();
    Some(HomeCard {
        kind: HomeCardKind::Artist,
        id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, raw_id),
        title,
        subtitle: "Artist".to_string(),
        thumbnail_url: item_primary_thumb(&base, item, raw_id),
        track: None,
    })
}

fn parse_album_card(profile: &ServerProfile, item: &Value) -> Option<HomeCard> {
    let raw_id = item_id_str(item)?;
    let title = item_name_str(item)?.to_string();
    let base = profile.normalized_base_url();
    let artist = item
        .get("AlbumArtist")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            item.get("Artists")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
        })
        .unwrap_or("Unknown Artist");
    let subtitle = match item.get("ProductionYear").and_then(Value::as_u64) {
        Some(year) => format!("{artist} • {year}"),
        None => artist.to_string(),
    };
    Some(HomeCard {
        kind: HomeCardKind::Album,
        id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, raw_id),
        title,
        subtitle,
        thumbnail_url: item_primary_thumb(&base, item, raw_id),
        track: None,
    })
}

fn parse_audio_track(profile: &ServerProfile, item: &Value) -> Option<Track> {
    let raw_id = item_id_str(item)?;
    let title = item_name_str(item)?.to_string();
    let base = profile.normalized_base_url();

    let artist = item
        .get("Artists")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .or_else(|| {
            item.get("AlbumArtist")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = item
        .get("Album")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let duration_secs = item
        .get("RunTimeTicks")
        .and_then(Value::as_u64)
        .map(|ticks| ticks / 10_000_000);
    let album_id = item
        .get("AlbumId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|aid| encode_server_item_id(ServerKind::Jellyfin, &profile.id, aid));

    let source_meta = item
        .get("MediaSources")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first());
    let audio_stream = source_meta
        .and_then(|ms| ms.get("MediaStreams").and_then(Value::as_array))
        .and_then(|streams| {
            streams.iter().find(|s| {
                s.get("Type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.eq_ignore_ascii_case("Audio"))
            })
        });
    let codec = audio_stream
        .and_then(|s| s.get("Codec").and_then(Value::as_str))
        .or_else(|| source_meta.and_then(|ms| ms.get("Container").and_then(Value::as_str)));
    let bitrate_kbps = audio_stream
        .and_then(|s| s.get("BitRate").and_then(Value::as_u64))
        .or_else(|| source_meta.and_then(|ms| ms.get("Bitrate").and_then(Value::as_u64)))
        .map(|bps| (bps / 1000) as u32);
    let audio_badge = format_audio_badge(codec, bitrate_kbps, "Jellyfin");

    Some(Track {
        id: encode_server_item_id(ServerKind::Jellyfin, &profile.id, raw_id),
        title,
        artist,
        album,
        duration_secs,
        thumbnail_url: item_primary_thumb(&base, item, raw_id),
        source: ProviderSource::YouTubeMusic,
        audio_badge,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> ServerProfile {
        let mut p = ServerProfile::new(
            "jf-home",
            "Home Jellyfin",
            ServerKind::Jellyfin,
            "https://jf.example.com/",
        );
        p.access_token = Some("token-abc".to_string());
        p.user_id = Some("user-1".to_string());
        p
    }

    #[test]
    fn parses_authenticate_by_name_response() {
        let body = r#"{
            "AccessToken": "jf-tok-999",
            "ServerId": "srv-xyz",
            "User": { "Id": "usr-42", "Name": "alice" }
        }"#;
        let auth = parse_auth_response_json(body).expect("valid auth response");
        assert_eq!(auth.access_token, "jf-tok-999");
        assert_eq!(auth.user_id, "usr-42");
        assert_eq!(auth.user_name.as_deref(), Some("alice"));
        assert_eq!(auth.server_id.as_deref(), Some("srv-xyz"));
    }

    #[test]
    fn parses_libraries_filtering_music_collections() {
        let val = json!({
            "Items": [
                { "Id": "lib-movies", "Name": "Movies", "CollectionType": "movies" },
                { "Id": "lib-music", "Name": "Lossless Music", "CollectionType": "music" }
            ]
        });
        let libs = parse_libraries_json(&val).expect("parse libraries");
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].id, "lib-music");
        assert_eq!(libs[0].name, "Lossless Music");
    }

    #[test]
    fn parses_album_detail_tracks_and_builds_stream_request() {
        let profile = test_profile();
        let meta = json!({
            "Items": [{
                "Id": "alb-1",
                "Name": "Midnight Signals",
                "ProductionYear": 2024,
                "AlbumArtist": "M83",
                "AlbumArtists": [{ "Id": "art-1", "Name": "M83" }],
                "ImageTags": { "Primary": "img-tag-1" }
            }]
        });
        let tracks = json!({
            "Items": [{
                "Id": "trk-1",
                "Name": "Intro",
                "Artists": ["M83"],
                "Album": "Midnight Signals",
                "AlbumId": "alb-1",
                "AlbumPrimaryImageTag": "img-tag-1",
                "RunTimeTicks": 2_450_000_000_u64,
                "MediaSources": [{
                    "Container": "flac",
                    "Bitrate": 980_000,
                    "MediaStreams": [{ "Type": "Audio", "Codec": "flac", "BitRate": 980_000 }]
                }]
            }]
        });

        let detail = parse_album_detail_json(&profile, "alb-1", &meta, &tracks).expect("album");
        assert_eq!(detail.id, "jellyfin:jf-home:alb-1");
        assert_eq!(detail.title, "Midnight Signals");
        assert_eq!(detail.year.as_deref(), Some("2024"));
        assert_eq!(detail.tracks.len(), 1);
        let t = &detail.tracks[0];
        assert_eq!(t.id, "jellyfin:jf-home:trk-1");
        assert_eq!(t.source_tag(), "JFN");
        assert_eq!(t.duration_secs, Some(245));
        assert_eq!(t.audio_badge.as_deref(), Some("FLAC · 980kbps"));
        assert_eq!(
            t.thumbnail_url.as_deref(),
            Some("https://jf.example.com/Items/alb-1/Images/Primary?maxWidth=800&quality=90&tag=img-tag-1")
        );

        let stream = stream_request(&profile, "trk-1", t).expect("stream request");
        assert_eq!(
            stream.url,
            "https://jf.example.com/Audio/trk-1/stream?static=true"
        );
        assert!(stream
            .headers
            .iter()
            .any(|(k, v)| k == "X-Emby-Token" && v == "token-abc"));
    }

    #[test]
    fn parses_mixed_search_and_playlists() {
        let profile = test_profile();
        let search_val = json!({
            "Items": [
                { "Id": "trk-9", "Name": "Echoes", "Type": "Audio", "Artists": ["Artist A"], "RunTimeTicks": 1_800_000_000_u64 },
                { "Id": "alb-9", "Name": "Horizon", "Type": "MusicAlbum", "AlbumArtist": "Artist A", "ProductionYear": 2021 },
                { "Id": "art-9", "Name": "Artist A", "Type": "MusicArtist" },
                { "Id": "pl-9", "Name": "Favorites", "Type": "Playlist", "ChildCount": 14 }
            ]
        });
        let res = parse_search_json(&profile, &search_val).expect("parse search");
        assert_eq!(res.tracks.len(), 1);
        assert_eq!(res.albums.len(), 1);
        assert_eq!(res.artists.len(), 1);
        assert_eq!(res.playlists.len(), 1);
        assert_eq!(res.albums[0].kind, HomeCardKind::Album);
        assert_eq!(res.artists[0].kind, HomeCardKind::Artist);
        assert_eq!(res.playlists[0].kind, HomeCardKind::Playlist);

        let playlists_val = json!({
            "Items": [{ "Id": "pl-9", "Name": "Favorites", "ChildCount": 14 }]
        });
        let playlists = parse_playlists_json(&profile, &playlists_val).expect("playlists");
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, "jellyfin:jf-home:pl-9");
        assert_eq!(playlists[0].track_count, Some(14));
    }
}
