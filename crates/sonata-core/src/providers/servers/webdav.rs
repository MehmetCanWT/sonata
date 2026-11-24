//! WebDAV RFC 4918 audio crawler, folder browser, and authenticated stream builder.
//!
//! 1. **PROPFIND XML Parsing**: Parses `Depth: 1` and `Depth: infinity` WebDAV `<multistatus>`
//!    responses using `roxmltree`, handling arbitrary namespace prefixes and multi-`<propstat>`
//!    status codes.
//! 2. **Browsing & Crawling**: Supports single-folder browsing, infinite-depth crawl with
//!    automatic `Depth: 1` BFS fallback when servers forbid `Depth: infinity`, and substring search.
//! 3. **Auth & Track Mapping**: Supports HTTP `Basic` and `Bearer` authentication and infers
//!    title, album, artist, and codec badge from WebDAV paths and content types.

use super::{
    default_http_timeout, encode_server_item_id, format_audio_badge, map_http_status, ServerKind,
    ServerProfile, ServerStream,
};
use crate::error::CoreError;
use crate::providers::{ProviderSource, Track};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Method, StatusCode, Url};
use roxmltree::{Document, Node};
use std::collections::{HashSet, VecDeque};

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

const DEFAULT_MAX_CRAWL_ITEMS: usize = 2_000;
const DEFAULT_MAX_CRAWL_DEPTH: usize = 6;

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

/// Directory listing returned by `browse_folder`.
#[derive(Debug, Clone, Default)]
pub struct WebDavListing {
    pub folder_url: String,
    pub folders: Vec<WebDavEntry>,
    pub tracks: Vec<Track>,
    pub entries: Vec<WebDavEntry>,
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
    if target.starts_with("http://") || target.starts_with("https://") {
        return Url::parse(target).map_err(|err| {
            CoreError::invalid_response("webdav", format!("invalid WebDAV target URL: {err}"))
        });
    }
    base_url.join(target).map_err(|err| {
        CoreError::invalid_response("webdav", format!("invalid WebDAV relative path: {err}"))
    })
}

async fn send_propfind(
    http: &Client,
    profile: &ServerProfile,
    target_url: &Url,
    depth: &str,
) -> Result<(StatusCode, String), CoreError> {
    let method = Method::from_bytes(b"PROPFIND").map_err(|err| {
        CoreError::internal(format!("failed to construct PROPFIND HTTP method: {err}"))
    })?;
    let mut header_map = HeaderMap::new();
    header_map.insert(
        "Depth",
        HeaderValue::from_str(depth)
            .map_err(|err| CoreError::internal(format!("invalid Depth header `{depth}`: {err}")))?,
    );
    header_map.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    for (k, v) in auth_headers(profile) {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|err| CoreError::internal(format!("invalid WebDAV header `{k}`: {err}")))?;
        let val = HeaderValue::from_str(&v).map_err(|err| {
            CoreError::auth_required(format!("invalid WebDAV auth header value: {err}"))
        })?;
        header_map.insert(name, val);
    }

    let resp = http
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

/// Lists immediate subfolders and audio tracks at `folder_or_path` (`Depth: 1`).
pub async fn browse_folder(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
) -> Result<WebDavListing, CoreError> {
    let target_url = resolve_target_url(profile, folder_or_path)?;
    let (status, body) = send_propfind(http, profile, &target_url, "1").await?;
    if !status.is_success() && status.as_u16() != 207 {
        return Err(map_http_status("webdav", status, &body));
    }
    let entries = parse_propfind_xml(&body, target_url.as_str())?;
    Ok(build_listing(profile, &target_url, entries))
}

/// Crawls audio tracks recursively, attempting `Depth: infinity` first when `max_depth` is `None`
/// and falling back to bounded `Depth: 1` BFS if the server rejects infinite depth.
pub async fn crawl_audio(
    http: &Client,
    profile: &ServerProfile,
    folder_or_path: Option<&str>,
    max_depth: Option<usize>,
    max_items: Option<usize>,
) -> Result<Vec<Track>, CoreError> {
    let start_url = resolve_target_url(profile, folder_or_path)?;
    let limit = max_items.unwrap_or(DEFAULT_MAX_CRAWL_ITEMS);
    if limit == 0 {
        return Ok(Vec::new());
    }

    if max_depth.is_none() {
        let (status, body) = send_propfind(http, profile, &start_url, "infinity").await?;
        if status.is_success() || status.as_u16() == 207 {
            let entries = parse_propfind_xml(&body, start_url.as_str())?;
            let mut tracks = entries
                .iter()
                .filter(|e| !e.is_dir && is_audio_entry(e))
                .map(|e| entry_to_track(profile, e))
                .take(limit)
                .collect::<Vec<_>>();
            tracks.sort_by(|a, b| a.title.cmp(&b.title));
            return Ok(tracks);
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

    let mut tracks = Vec::new();
    while let Some((dir_url, depth)) = queue.pop_front() {
        if tracks.len() >= limit {
            break;
        }
        let (status, body) = send_propfind(http, profile, &dir_url, "1").await?;
        if !status.is_success() && status.as_u16() != 207 {
            return Err(map_http_status("webdav", status, &body));
        }
        let entries = parse_propfind_xml(&body, dir_url.as_str())?;
        for entry in entries {
            if entry.is_dir {
                if depth < depth_limit {
                    let key = normalize_path_key(&entry.path);
                    if visited.insert(key) {
                        if let Ok(next_url) = Url::parse(&entry.url) {
                            queue.push_back((next_url, depth + 1));
                        }
                    }
                }
            } else if is_audio_entry(&entry) {
                tracks.push(entry_to_track(profile, &entry));
                if tracks.len() >= limit {
                    break;
                }
            }
        }
    }
    Ok(tracks)
}

/// Crawls the configured WebDAV root and returns audio tracks matching `query`.
pub async fn search(
    http: &Client,
    profile: &ServerProfile,
    query: &str,
    limit: usize,
) -> Result<Vec<Track>, CoreError> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let crawled = crawl_audio(http, profile, None, None, Some(DEFAULT_MAX_CRAWL_ITEMS)).await?;
    Ok(crawled
        .into_iter()
        .filter(|t| {
            t.title.to_ascii_lowercase().contains(&needle)
                || t.artist.to_ascii_lowercase().contains(&needle)
                || t.album.to_ascii_lowercase().contains(&needle)
                || t.id.to_ascii_lowercase().contains(&needle)
        })
        .take(limit)
        .collect())
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
    Ok(ServerStream {
        url: target_url.into(),
        headers: auth_headers(profile),
        mime_type,
        bitrate_kbps: None,
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

/// Converts a parsed WebDAV audio [`WebDavEntry`] into Sonata's [`Track`] model.
pub fn entry_to_track(profile: &ServerProfile, entry: &WebDavEntry) -> Track {
    let base_path = Url::parse(&profile.normalized_base_url())
        .map(|u| u.path().to_string())
        .unwrap_or_default();
    let rel_path = entry
        .path
        .strip_prefix(base_path.trim_end_matches('/'))
        .unwrap_or(&entry.path);
    let segments: Vec<String> = rel_path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            urlencoding::decode(s)
                .map(|d| d.into_owned())
                .unwrap_or_else(|_| s.to_string())
        })
        .collect();

    let raw_filename = segments
        .last()
        .cloned()
        .unwrap_or_else(|| entry.name.clone());
    let title = clean_track_title(&raw_filename);
    let album = if segments.len() >= 2 {
        segments[segments.len() - 2].clone()
    } else {
        String::new()
    };
    let artist = if segments.len() >= 3 {
        segments[segments.len() - 3].clone()
    } else {
        "Unknown Artist".to_string()
    };

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
        album_id: None,
        is_explicit: false,
    }
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
    let file = path.rsplit('/').next()?;
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
        assert_eq!(track.audio_badge.as_deref(), Some("FLAC · WebDAV"));
        assert_eq!(
            track.id,
            "webdav:dav-1:/remote.php/dav/files/alice/Music/M83/Hurry%20Up/01%20-%20Midnight%20City.flac"
        );
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
            audio_badge: Some("OPUS · WebDAV".to_string()),
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
}
