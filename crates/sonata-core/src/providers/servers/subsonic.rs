//! Subsonic and OpenSubsonic REST API client with pure-Rust RFC 1321 MD5 salted authentication.
//!
//! 1. **RFC 1321 MD5 & Auth (`md5_hex`, `compute_subsonic_token`)**: Generates zero-dependency MD5
//!    salted tokens (`u`, `t = md5(password + salt)`, `s = salt`, `v = 1.16.1`, `c = Sonata`, `f = json`).
//! 2. **Endpoints**: Implements `ping`, `getMusicFolders`, `getArtists`, `getAlbum`, `getPlaylists`,
//!    `getPlaylist`, `search3`, `getCoverArt`, and `stream`.
//! 3. **Response Parsing**: Tolerates both JSON arrays and XML-converted single-object fields while
//!    mapping Subsonic error codes into typed `CoreError` variants.

use super::{
    default_http_timeout, encode_server_item_id, format_audio_badge, map_http_status,
    no_redirect_http_client, validate_server_transport, validate_server_transport_sync, ServerKind,
    ServerLibrary, ServerProfile, ServerSearchResult, ServerStream,
};
use crate::error::CoreError;
use crate::providers::ytmusic::{AlbumDetail, HomeCard, HomeCardKind};
use crate::providers::{Playlist, ProviderSource, Track};
use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SUBSONIC_API_VERSION: &str = "1.16.1";
const SUBSONIC_CLIENT_NAME: &str = "Sonata";
static SALT_COUNTER: AtomicU64 = AtomicU64::new(1);

const MD5_SHIFTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// Computes the lowercase 32-character RFC 1321 MD5 hex digest of `input`.
pub fn md5_hex(input: &[u8]) -> String {
    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let off = i * 4;
            *word =
                u32::from_le_bytes([chunk[off], chunk[off + 1], chunk[off + 2], chunk[off + 3]]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(MD5_K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(MD5_SHIFTS[i]));
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = String::with_capacity(32);
    for word in [a0, b0, c0, d0] {
        for byte in word.to_le_bytes() {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

/// Computes Subsonic's salted authentication token `md5(password + salt)`.
pub fn compute_subsonic_token(password: &str, salt: &str) -> String {
    let mut combined = String::with_capacity(password.len() + salt.len());
    combined.push_str(password);
    combined.push_str(salt);
    md5_hex(combined.as_bytes())
}

/// Generates a unique per-request random-looking salt string.
pub fn generate_salt() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = SALT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let digest = md5_hex(format!("{nanos}:{seq}:{}", std::process::id()).as_bytes());
    digest[..12].to_string()
}

/// Builds a Subsonic REST endpoint URL with salted token authentication (`u`, `t`, `s`, `v`, `c`, `f`).
pub fn build_auth_url(
    profile: &ServerProfile,
    endpoint: &str,
    salt: &str,
    extra_params: &[(&str, &str)],
) -> Result<String, CoreError> {
    validate_server_transport_sync(profile)?;
    let base = profile.normalized_base_url();
    if base.is_empty() {
        return Err(CoreError::invalid_response(
            "subsonic",
            "Subsonic base URL is empty",
        ));
    }
    let user = profile
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::auth_required("Subsonic profile requires a username"))?;
    let password = profile
        .password
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::auth_required("Subsonic profile requires a password"))?;

    let token = compute_subsonic_token(password, salt);
    let clean_endpoint = endpoint.trim_start_matches('/');
    let mut url = format!(
        "{base}/rest/{clean_endpoint}?u={}&t={token}&s={}&v={SUBSONIC_API_VERSION}&c={SUBSONIC_CLIENT_NAME}&f=json",
        urlencoding::encode(user),
        urlencoding::encode(salt)
    );
    for (k, v) in extra_params {
        url.push('&');
        url.push_str(&urlencoding::encode(k));
        url.push('=');
        url.push_str(&urlencoding::encode(v));
    }
    Ok(url)
}

async fn call_subsonic(
    _http: &Client,
    profile: &ServerProfile,
    endpoint: &str,
    extra_params: &[(&str, &str)],
) -> Result<Value, CoreError> {
    validate_server_transport(profile).await?;
    let salt = generate_salt();
    let url = build_auth_url(profile, endpoint, &salt, extra_params)?;
    let resp = no_redirect_http_client()
        .get(&url)
        .timeout(default_http_timeout())
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("subsonic", &err))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|err| CoreError::from_reqwest("subsonic", &err))?;
    if !status.is_success() {
        return Err(map_http_status("subsonic", status, &body));
    }
    unwrap_subsonic_response(&body)
}

/// Parses and validates the outer `subsonic-response` envelope, mapping API error codes.
pub fn unwrap_subsonic_response(body: &str) -> Result<Value, CoreError> {
    let root: Value = serde_json::from_str(body).map_err(|err| {
        CoreError::invalid_response("subsonic", format!("malformed Subsonic JSON: {err}"))
    })?;
    let inner = root.get("subsonic-response").ok_or_else(|| {
        CoreError::invalid_response("subsonic", "missing `subsonic-response` object")
    })?;
    let status = inner.get("status").and_then(Value::as_str).unwrap_or("");
    if !status.eq_ignore_ascii_case("ok") {
        let code = inner
            .pointer("/error/code")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let msg = inner
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Subsonic request failed");
        return Err(match code {
            40 | 41 | 50 => CoreError::auth_required(format!("Subsonic error {code}: {msg}")),
            70 => CoreError::not_found(format!("Subsonic error {code}: {msg}")),
            _ => CoreError::invalid_response("subsonic", format!("Subsonic error {code}: {msg}")),
        });
    }
    Ok(inner.clone())
}

/// Verifies server connectivity and credentials using `ping.view`.
pub async fn ping(http: &Client, profile: &ServerProfile) -> Result<String, CoreError> {
    let inner = call_subsonic(http, profile, "ping.view", &[]).await?;
    let version = inner
        .get("serverVersion")
        .or_else(|| inner.get("version"))
        .and_then(Value::as_str)
        .unwrap_or(SUBSONIC_API_VERSION);
    let server_type = inner
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("Subsonic");
    Ok(format!("{server_type} ({version})"))
}

/// Fetches configured music folders using `getMusicFolders.view`.
pub async fn get_music_folders(
    http: &Client,
    profile: &ServerProfile,
) -> Result<Vec<ServerLibrary>, CoreError> {
    let inner = call_subsonic(http, profile, "getMusicFolders.view", &[]).await?;
    parse_music_folders_json(&inner)
}

/// Fetches indexed artists using `getArtists.view` (optionally scoped to `musicFolderId`).
pub async fn get_artists(
    http: &Client,
    profile: &ServerProfile,
    music_folder_id: Option<&str>,
) -> Result<Vec<HomeCard>, CoreError> {
    let params: Vec<(&str, &str)> = music_folder_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|fid| vec![("musicFolderId", fid)])
        .unwrap_or_default();
    let inner = call_subsonic(http, profile, "getArtists.view", &params).await?;
    parse_artists_json(profile, &inner)
}

/// Fetches albums for a specific artist via `getArtist.view`.
pub async fn get_artist_albums(
    http: &Client,
    profile: &ServerProfile,
    artist_id: &str,
) -> Result<Vec<HomeCard>, CoreError> {
    let inner = call_subsonic(http, profile, "getArtist.view", &[("id", artist_id)]).await?;
    parse_artist_albums_json(profile, &inner)
}

/// Fetches albums via `getAlbumList2.view`.
pub async fn get_album_list(
    http: &Client,
    profile: &ServerProfile,
    list_type: &str,
    size: usize,
) -> Result<Vec<HomeCard>, CoreError> {
    let size_str = size.to_string();
    let inner = call_subsonic(
        http,
        profile,
        "getAlbumList2.view",
        &[("type", list_type), ("size", &size_str)],
    )
    .await?;
    parse_album_list_json(profile, &inner)
}

/// Fetches album details and tracks via `getAlbum.view`.
pub async fn get_album(
    http: &Client,
    profile: &ServerProfile,
    album_id: &str,
) -> Result<AlbumDetail, CoreError> {
    let inner = call_subsonic(http, profile, "getAlbum.view", &[("id", album_id)]).await?;
    parse_album_json(profile, &inner)
}

/// Fetches user playlists via `getPlaylists.view`.
pub async fn get_playlists(
    http: &Client,
    profile: &ServerProfile,
) -> Result<Vec<Playlist>, CoreError> {
    let inner = call_subsonic(http, profile, "getPlaylists.view", &[]).await?;
    parse_playlists_json(profile, &inner)
}

/// Fetches playlist tracks via `getPlaylist.view`.
pub async fn get_playlist(
    http: &Client,
    profile: &ServerProfile,
    playlist_id: &str,
) -> Result<Vec<Track>, CoreError> {
    let inner = call_subsonic(http, profile, "getPlaylist.view", &[("id", playlist_id)]).await?;
    parse_playlist_tracks_json(profile, &inner)
}

/// Searches artists, albums, and songs via `search3.view`.
pub async fn search3(
    http: &Client,
    profile: &ServerProfile,
    query: &str,
    limit: usize,
) -> Result<ServerSearchResult, CoreError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(ServerSearchResult::default());
    }
    let count = limit.to_string();
    let inner = call_subsonic(
        http,
        profile,
        "search3.view",
        &[
            ("query", trimmed),
            ("artistCount", &count),
            ("albumCount", &count),
            ("songCount", &count),
        ],
    )
    .await?;
    parse_search3_json(profile, &inner)
}

/// Builds a signed `getCoverArt.view` URL for `cover_art_id`.
pub fn cover_art_url(
    profile: &ServerProfile,
    cover_art_id: &str,
    size: u32,
) -> Result<String, CoreError> {
    let salt = generate_salt();
    let size_str = size.to_string();
    build_auth_url(
        profile,
        "getCoverArt.view",
        &salt,
        &[("id", cover_art_id), ("size", &size_str)],
    )
}

/// Builds a signed direct `stream.view` request URL for `song_id`.
pub fn stream_request(
    profile: &ServerProfile,
    song_id: &str,
    track: &Track,
) -> Result<ServerStream, CoreError> {
    let salt = generate_salt();
    let url = build_auth_url(profile, "stream.view", &salt, &[("id", song_id)])?;
    Ok(ServerStream {
        url,
        headers: Vec::new(),
        mime_type: None,
        bitrate_kbps: None,
        codec: None,
        audio_badge: track.audio_badge.clone(),
    })
}

/// Pure parser for `getMusicFolders.view` inner `subsonic-response` object.
pub fn parse_music_folders_json(inner: &Value) -> Result<Vec<ServerLibrary>, CoreError> {
    let items = array_or_single(inner.pointer("/musicFolders/musicFolder"));
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let id = json_id_string(item.get("id")?)?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Music")
                .to_string();
            Some(ServerLibrary {
                id,
                name,
                kind: "music".to_string(),
            })
        })
        .collect())
}

/// Pure parser for `getArtists.view` inner `subsonic-response` object.
pub fn parse_artists_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<Vec<HomeCard>, CoreError> {
    let indices = array_or_single(inner.pointer("/artists/index"));
    let mut out = Vec::new();
    for idx in indices {
        for artist in array_or_single(idx.get("artist")) {
            if let Some(card) = parse_artist_card(profile, artist) {
                out.push(card);
            }
        }
    }
    Ok(out)
}

/// Pure parser for `getArtist.view` inner `subsonic-response` object.
pub fn parse_artist_albums_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<Vec<HomeCard>, CoreError> {
    let albums = array_or_single(inner.pointer("/artist/album"));
    Ok(albums
        .into_iter()
        .filter_map(|album| parse_album_card(profile, album))
        .collect())
}

/// Pure parser for `getAlbumList2.view` inner `subsonic-response` object.
pub fn parse_album_list_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<Vec<HomeCard>, CoreError> {
    let albums = array_or_single(inner.pointer("/albumList2/album"));
    Ok(albums
        .into_iter()
        .filter_map(|album| parse_album_card(profile, album))
        .collect())
}

/// Pure parser for `getAlbum.view` inner `subsonic-response` object.
pub fn parse_album_json(profile: &ServerProfile, inner: &Value) -> Result<AlbumDetail, CoreError> {
    let album = inner.get("album").ok_or_else(|| {
        CoreError::invalid_response("subsonic", "getAlbum missing `album` object")
    })?;
    let raw_id = album
        .get("id")
        .and_then(json_id_string)
        .ok_or_else(|| CoreError::invalid_response("subsonic", "album missing `id`"))?;
    let title = album
        .get("name")
        .or_else(|| album.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Album")
        .to_string();
    let artist_name = album
        .get("artist")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown Artist");
    let artist_id = album
        .get("artistId")
        .and_then(json_id_string)
        .map(|aid| encode_server_item_id(ServerKind::Subsonic, &profile.id, &aid));
    let year = album
        .get("year")
        .and_then(Value::as_u64)
        .map(|y| y.to_string());
    let cover_url = album
        .get("coverArt")
        .and_then(json_id_string)
        .and_then(|cid| cover_art_url(profile, &cid, 600).ok());

    let tracks: Vec<Track> = array_or_single(album.get("song"))
        .into_iter()
        .filter_map(|song| parse_song_track(profile, song))
        .collect();
    let total_secs = album
        .get("duration")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| tracks.iter().filter_map(|t| t.duration_secs).sum());
    let duration_text = if total_secs > 0 {
        Some(format!("{} min", (total_secs + 30) / 60))
    } else {
        None
    };

    let subtitle = match &year {
        Some(y) => format!("Album • {artist_name} • {y}"),
        None => format!("Album • {artist_name}"),
    };

    Ok(AlbumDetail {
        id: encode_server_item_id(ServerKind::Subsonic, &profile.id, &raw_id),
        title,
        subtitle,
        release_type: "Album".to_string(),
        year,
        duration_text,
        cover_url,
        artists: vec![(artist_name.to_string(), artist_id)],
        tracks,
        other_versions: Vec::new(),
        playlist_id: None,
        saved: false,
    })
}

/// Pure parser for `getPlaylists.view` inner `subsonic-response` object.
pub fn parse_playlists_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<Vec<Playlist>, CoreError> {
    let playlists = array_or_single(inner.pointer("/playlists/playlist"));
    Ok(playlists
        .into_iter()
        .filter_map(|pl| {
            let raw_id = json_id_string(pl.get("id")?)?;
            let name = pl
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Playlist")
                .to_string();
            let track_count = pl
                .get("songCount")
                .and_then(Value::as_u64)
                .map(|c| c as usize);
            let thumbnail_url = pl
                .get("coverArt")
                .and_then(json_id_string)
                .and_then(|cid| cover_art_url(profile, &cid, 600).ok());
            Some(Playlist {
                id: encode_server_item_id(ServerKind::Subsonic, &profile.id, &raw_id),
                name,
                source: ProviderSource::YouTubeMusic,
                track_count,
                thumbnail_url,
                is_owned: true,
            })
        })
        .collect())
}

/// Pure parser for `getPlaylist.view` inner `subsonic-response` object.
pub fn parse_playlist_tracks_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<Vec<Track>, CoreError> {
    let entries = array_or_single(inner.pointer("/playlist/entry"));
    Ok(entries
        .into_iter()
        .filter_map(|song| parse_song_track(profile, song))
        .collect())
}

/// Pure parser for `search3.view` inner `subsonic-response` object.
pub fn parse_search3_json(
    profile: &ServerProfile,
    inner: &Value,
) -> Result<ServerSearchResult, CoreError> {
    let sr = inner.get("searchResult3").unwrap_or(&Value::Null);
    let tracks = array_or_single(sr.get("song"))
        .into_iter()
        .filter_map(|song| parse_song_track(profile, song))
        .collect();
    let albums = array_or_single(sr.get("album"))
        .into_iter()
        .filter_map(|album| parse_album_card(profile, album))
        .collect();
    let artists = array_or_single(sr.get("artist"))
        .into_iter()
        .filter_map(|artist| parse_artist_card(profile, artist))
        .collect();
    Ok(ServerSearchResult {
        tracks,
        albums,
        artists,
        playlists: Vec::new(),
    })
}

fn array_or_single(val: Option<&Value>) -> Vec<&Value> {
    match val {
        Some(Value::Array(arr)) => arr.iter().collect(),
        Some(obj @ Value::Object(_)) => vec![obj],
        _ => Vec::new(),
    }
}

fn json_id_string(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_artist_card(profile: &ServerProfile, artist: &Value) -> Option<HomeCard> {
    let raw_id = artist.get("id").and_then(json_id_string)?;
    let title = artist
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let subtitle = match artist.get("albumCount").and_then(Value::as_u64) {
        Some(count) => format!("{count} albums"),
        None => "Artist".to_string(),
    };
    let thumbnail_url = artist
        .get("coverArt")
        .and_then(json_id_string)
        .and_then(|cid| cover_art_url(profile, &cid, 600).ok())
        .or_else(|| {
            artist
                .get("artistImageUrl")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    Some(HomeCard {
        kind: HomeCardKind::Artist,
        id: encode_server_item_id(ServerKind::Subsonic, &profile.id, &raw_id),
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

fn parse_album_card(profile: &ServerProfile, album: &Value) -> Option<HomeCard> {
    let raw_id = album.get("id").and_then(json_id_string)?;
    let title = album
        .get("name")
        .or_else(|| album.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let artist = album
        .get("artist")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown Artist");
    let subtitle = match album.get("year").and_then(Value::as_u64) {
        Some(y) => format!("{artist} • {y}"),
        None => artist.to_string(),
    };
    let thumbnail_url = album
        .get("coverArt")
        .and_then(json_id_string)
        .and_then(|cid| cover_art_url(profile, &cid, 600).ok());
    Some(HomeCard {
        kind: HomeCardKind::Album,
        id: encode_server_item_id(ServerKind::Subsonic, &profile.id, &raw_id),
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

fn parse_song_track(profile: &ServerProfile, song: &Value) -> Option<Track> {
    let raw_id = song.get("id").and_then(json_id_string)?;
    let title = song
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let artist = song
        .get("artist")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown Artist")
        .to_string();
    let album = song
        .get("album")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let duration_secs = song.get("duration").and_then(Value::as_u64);
    let album_id = song
        .get("albumId")
        .and_then(json_id_string)
        .map(|aid| encode_server_item_id(ServerKind::Subsonic, &profile.id, &aid));
    let thumbnail_url = song
        .get("coverArt")
        .and_then(json_id_string)
        .and_then(|cid| cover_art_url(profile, &cid, 600).ok());

    let codec = song.get("suffix").and_then(Value::as_str);
    let bitrate_kbps = song
        .get("bitRate")
        .and_then(Value::as_u64)
        .map(|b| b as u32);
    let audio_badge = format_audio_badge(codec, bitrate_kbps, "Subsonic");

    Some(Track {
        id: encode_server_item_id(ServerKind::Subsonic, &profile.id, &raw_id),
        title,
        artist,
        album,
        duration_secs,
        thumbnail_url,
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
            "navi-1",
            "Navidrome",
            ServerKind::Subsonic,
            "https://sub.example.com/",
        );
        p.username = Some("alice".to_string());
        p.password = Some("sesame".to_string());
        p
    }

    #[test]
    fn matches_official_rfc1321_md5_test_vectors() {
        let vectors = [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ];

        for (input, expected) in vectors {
            assert_eq!(md5_hex(input.as_bytes()), expected, "vector `{input}`");
        }
    }

    #[test]
    fn builds_salted_token_auth_urls_without_cleartext_password() {
        let profile = test_profile();
        let url = build_auth_url(&profile, "stream.view", "c19b2d", &[("id", "tr-77")])
            .expect("auth url");
        // md5("sesamec19b2d")
        let expected_tok = md5_hex(b"sesamec19b2d");
        assert!(url.contains(&format!("t={expected_tok}")));
        assert!(url.contains("u=alice&"));
        assert!(url.contains("&s=c19b2d&"));
        assert!(url.contains("&v=1.16.1&c=Sonata&f=json"));
        assert!(url.contains("&id=tr-77"));
        assert!(!url.contains("sesame"));
    }

    #[test]
    fn maps_subsonic_error_codes_and_parses_catalog() {
        let auth_err = unwrap_subsonic_response(
            r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":40,"message":"Wrong username or password"}}}"#,
        )
        .expect_err("error 40 should map to AuthRequired");
        assert!(matches!(auth_err, CoreError::AuthRequired { .. }));

        let not_found_err = unwrap_subsonic_response(
            r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":70,"message":"The requested data was not found"}}}"#,
        )
        .expect_err("error 70 should map to NotFound");
        assert!(matches!(not_found_err, CoreError::NotFound { .. }));

        let profile = test_profile();
        let search_body = r#"{
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1",
                "searchResult3": {
                    "artist": { "id": 10, "name": "Boards of Canada", "albumCount": 6 },
                    "album": [{ "id": "alb-5", "name": "Geogaddi", "artist": "Boards of Canada", "year": 2002, "coverArt": "al-5" }],
                    "song": [{
                        "id": "s-1",
                        "title": "Music Is Math",
                        "artist": "Boards of Canada",
                        "album": "Geogaddi",
                        "albumId": "alb-5",
                        "duration": 321,
                        "bitRate": 1024,
                        "suffix": "flac",
                        "coverArt": "al-5"
                    }]
                }
            }
        }"#;
        let inner = unwrap_subsonic_response(search_body).expect("valid ok envelope");
        let parsed = parse_search3_json(&profile, &inner).expect("parse search3");
        assert_eq!(parsed.artists.len(), 1);
        assert_eq!(parsed.artists[0].id, "subsonic:navi-1:10");
        assert_eq!(parsed.albums.len(), 1);
        assert_eq!(parsed.tracks.len(), 1);
        let track = &parsed.tracks[0];
        assert_eq!(track.id, "subsonic:navi-1:s-1");
        assert_eq!(track.source_tag(), "SUB");
        assert_eq!(track.audio_badge.as_deref(), Some("FLAC · 1024 kbps"));
        assert!(track
            .thumbnail_url
            .as_deref()
            .is_some_and(|u| u.contains("/rest/getCoverArt.view?")));

        let stream = stream_request(&profile, "s-1", track).expect("stream");
        assert!(stream.url.contains("/rest/stream.view?"));
        assert!(stream.url.contains("&id=s-1"));
    }

    #[test]
    fn parses_folders_album_and_playlists() {
        let profile = test_profile();
        let folders_inner = unwrap_subsonic_response(
            r#"{"subsonic-response":{"status":"ok","version":"1.16.1","musicFolders":{"musicFolder":{"id":1,"name":"FLAC Archive"}}}}"#,
        )
        .expect("folders ok");
        let folders = parse_music_folders_json(&folders_inner).expect("parse folders");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].id, "1");
        assert_eq!(folders[0].name, "FLAC Archive");

        let album_inner = unwrap_subsonic_response(
            r#"{"subsonic-response":{"status":"ok","version":"1.16.1","album":{"id":"al-9","name":"Campfire Headphase","artist":"Boards of Canada","artistId":"10","year":2005,"duration":3720,"song":[{"id":"s-9","title":"Peacock Tail","artist":"Boards of Canada","duration":324,"suffix":"opus","bitRate":160}]}}}"#,
        )
        .expect("album ok");
        let album = parse_album_json(&profile, &album_inner).expect("parse album");
        assert_eq!(album.title, "Campfire Headphase");
        assert_eq!(album.tracks.len(), 1);
        assert_eq!(
            album.tracks[0].audio_badge.as_deref(),
            Some("OPUS · 160 kbps")
        );

        let pl_inner = unwrap_subsonic_response(
            r#"{"subsonic-response":{"status":"ok","version":"1.16.1","playlists":{"playlist":[{"id":"pl-1","name":"Night Drive","songCount":12}]}}}"#,
        )
        .expect("playlists ok");
        let pls = parse_playlists_json(&profile, &pl_inner).expect("parse playlists");
        assert_eq!(pls.len(), 1);
        assert_eq!(pls[0].id, "subsonic:navi-1:pl-1");
        assert_eq!(pls[0].track_count, Some(12));
    }

    #[tokio::test]
    async fn authenticated_requests_do_not_follow_cross_origin_redirects() {
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
            if let Ok((mut sock, _)) = redirect_listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{trap_addr}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let mut profile = ServerProfile::new(
            "sub-redir",
            "Redirect Subsonic",
            ServerKind::Subsonic,
            format!("http://{redirect_addr}"),
        );
        profile.username = Some("alice".to_string());
        profile.password = Some("sesame".to_string());

        let http = crate::providers::build_http_client();
        assert!(ping(&http, &profile).await.is_err());
        profile.allow_insecure_lan_http = true;
        let err = ping(&http, &profile)
            .await
            .expect_err("302 redirect must not be followed");
        assert!(err.to_string().contains("302"));
        assert!(!trap_hit.load(Ordering::SeqCst));

        redirect_task.abort();
        trap_task.abort();
    }
}
