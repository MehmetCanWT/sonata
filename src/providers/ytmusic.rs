use crate::auth::ExtractedCookies;
use crate::providers::{parse_duration_text, Playlist, ProviderSource, Track};
use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const INNERTUBE_BASE: &str = "https://music.youtube.com/youtubei/v1";

pub fn upscale_thumbnail(url: &str) -> String {
    static RE_LH3: OnceLock<Regex> = OnceLock::new();
    let re = RE_LH3.get_or_init(|| Regex::new(r"=w\d+-h\d+[^&\s]*|=s\d+[^&\s]*").unwrap());
    if url.contains("googleusercontent.com") {
        if re.is_match(url) {
            return re.replace(url, "=w800-h800-l90-rj").into_owned();
        }
        return format!("{url}=w800-h800-l90-rj");
    }
    url.to_string()
}

type PlaylistCache = Arc<Mutex<HashMap<String, (Instant, Vec<Track>)>>>;

#[derive(Clone)]
pub struct YtMusicClient {
    http: Client,
    cookies: ExtractedCookies,
    playlist_cache: PlaylistCache,
}

impl YtMusicClient {
    pub fn new(http: Client, cookies: ExtractedCookies) -> Self {
        Self {
            http,
            cookies,
            playlist_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn invalidate_cache(&self, browse_id: Option<&str>) {
        let mut guard = self.playlist_cache.lock().await;
        match browse_id {
            Some(id) => {
                guard.remove(id);
                guard.remove(&format!("VL{id}"));
                if let Some(stripped) = id.strip_prefix("VL") {
                    guard.remove(stripped);
                }
            }
            None => guard.clear(),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.cookies.has_ytm_auth()
    }

    async fn post(&self, endpoint: &str, body: Value) -> Result<Value> {
        let url = format!("{INNERTUBE_BASE}/{endpoint}?prettyPrint=false");
        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("X-Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", "1.20260401.01.00")
            .json(&body);

        if let (Some(cookie_hdr), Some(auth_hdr)) = (
            self.cookies.ytm_cookie_header(),
            self.cookies.ytm_sapisidhash(),
        ) {
            req = req
                .header("Cookie", cookie_hdr)
                .header("Authorization", auth_hdr)
                .header("X-Goog-AuthUser", "0");
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("YouTube Music API HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
    }

    fn base_context() -> Value {
        json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20260401.01.00",
                    "hl": "en",
                    "gl": "US"
                }
            }
        })
    }

    pub async fn fetch_library_playlists(&self) -> Result<Vec<Playlist>> {
        if !self.is_authenticated() {
            return Ok(Vec::new());
        }

        let mut body = Self::base_context();
        body["browseId"] = json!("FEmusic_liked_playlists");
        let data = self.post("browse", body).await?;

        let mut playlists = vec![Playlist {
            id: "VLLM".to_string(),
            name: "Liked Music".to_string(),
            source: ProviderSource::YouTubeMusic,
            track_count: None,
            thumbnail_url: None,
        }];

        collect_playlists(&data, &mut playlists);
        // Deduplicate preserving order
        let mut seen = std::collections::HashSet::new();
        playlists.retain(|p| seen.insert(p.id.clone()));
        Ok(playlists)
    }

    pub async fn fetch_playlist_tracks_progressive<F>(
        &self,
        browse_or_id: &str,
        mut on_batch: F,
    ) -> Result<()>
    where
        F: FnMut(Vec<Track>, bool) + Send,
    {
        let browse_id = if browse_or_id.starts_with("VL") || browse_or_id.starts_with("FE") {
            browse_or_id.to_string()
        } else {
            format!("VL{browse_or_id}")
        };

        {
            let guard = self.playlist_cache.lock().await;
            if let Some((at, cached)) = guard.get(&browse_id) {
                if at.elapsed() < Duration::from_secs(5 * 60) {
                    on_batch(cached.clone(), true);
                    return Ok(());
                }
            }
        }

        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        let data = self.post("browse", body).await?;

        let mut page1 = Vec::new();
        collect_tracks(&data, &mut page1);
        if browse_id == "VLLM" {
            for t in &mut page1 {
                t.liked = true;
            }
        }
        on_batch(page1.clone(), true);

        let mut all_tracks = page1;
        let mut cont = find_shelf_continuation(&data);
        for _ in 0..20 {
            let Some(token) = cont else { break };
            let mut next_body = Self::base_context();
            next_body["continuation"] = json!(token);
            let Ok(page) = self.post("browse", next_body).await else {
                break;
            };
            let mut batch = Vec::new();
            collect_tracks(&page, &mut batch);
            if batch.is_empty() {
                break;
            }
            if browse_id == "VLLM" {
                for t in &mut batch {
                    t.liked = true;
                }
            }
            on_batch(batch.clone(), false);
            all_tracks.extend(batch);
            let next_cont = find_shelf_continuation(&page);
            if next_cont.as_deref() == Some(token.as_str()) {
                break;
            }
            cont = next_cont;
        }

        let mut guard = self.playlist_cache.lock().await;
        guard.insert(browse_id, (Instant::now(), all_tracks));

        Ok(())
    }

    pub async fn fetch_native_lyrics(&self, video_id: &str) -> Option<String> {
        let mut body = Self::base_context();
        body["videoId"] = json!(video_id);
        let next = self.post("next", body).await.ok()?;
        let lyrics_browse_id = find_mplyt_browse_id(&next)?;
        let mut b_body = Self::base_context();
        b_body["browseId"] = json!(lyrics_browse_id);
        let page = self.post("browse", b_body).await.ok()?;
        find_description_text(&page)
    }

    pub async fn set_like_status(&self, video_id: &str, like: bool) -> Result<()> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let endpoint = if like { "like/like" } else { "like/removelike" };
        let mut body = Self::base_context();
        body["target"] = json!({ "videoId": video_id });
        self.post(endpoint, body).await?;
        self.invalidate_cache(Some("VLLM")).await;
        Ok(())
    }

    pub async fn add_to_playlist(&self, playlist_browse_id: &str, video_id: &str) -> Result<()> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let raw_id = playlist_browse_id
            .strip_prefix("VL")
            .unwrap_or(playlist_browse_id);
        let mut body = Self::base_context();
        body["playlistId"] = json!(raw_id);
        body["actions"] = json!([{
            "action": "ACTION_ADD_VIDEO",
            "addedVideoId": video_id
        }]);
        self.post("browse/edit_playlist", body).await?;
        self.invalidate_cache(Some(playlist_browse_id)).await;
        Ok(())
    }

    pub async fn remove_from_playlist(
        &self,
        playlist_browse_id: &str,
        video_id: &str,
        set_video_id: &str,
    ) -> Result<()> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let raw_id = playlist_browse_id
            .strip_prefix("VL")
            .unwrap_or(playlist_browse_id);
        let mut body = Self::base_context();
        body["playlistId"] = json!(raw_id);
        body["actions"] = json!([{
            "action": "ACTION_REMOVE_VIDEO",
            "removedVideoId": video_id,
            "setVideoId": set_video_id
        }]);
        self.post("browse/edit_playlist", body).await?;
        self.invalidate_cache(Some(playlist_browse_id)).await;
        Ok(())
    }

    pub async fn create_playlist(&self, title: &str) -> Result<String> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let mut body = Self::base_context();
        body["title"] = json!(title);
        body["privacyStatus"] = json!("PRIVATE");
        let resp = self.post("playlist/create", body).await?;
        let pid = resp
            .get("playlistId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Missing playlistId in response"))?;
        Ok(pid)
    }

    pub async fn search_songs(&self, query: &str) -> Result<Vec<Track>> {
        let mut body = Self::base_context();
        body["query"] = json!(query);
        // Filter strictly to Songs ("EgWKAQIIAWoMEAMQBBAJEAoQBRAV")
        body["params"] = json!("EgWKAQIIAWoMEAMQBBAJEAoQBRAV");
        let data = self.post("search", body).await?;

        let mut tracks = Vec::new();
        collect_tracks(&data, &mut tracks);
        if tracks.is_empty() {
            // Retry unfiltered search if song filter returned empty
            let mut fallback = Self::base_context();
            fallback["query"] = json!(query);
            if let Ok(fb_data) = self.post("search", fallback).await {
                collect_tracks(&fb_data, &mut tracks);
            }
        }
        Ok(tracks)
    }
}

fn collect_playlists(val: &Value, out: &mut Vec<Playlist>) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("musicTwoRowItemRenderer") {
                if let Some(pl) = parse_two_row_playlist(renderer) {
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

fn parse_two_row_playlist(r: &Value) -> Option<Playlist> {
    let browse_id = r
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(|v| v.as_str())?;
    if !browse_id.starts_with("VL") {
        return None;
    }
    if browse_id == "VLLM" || browse_id == "VLSE" {
        return None;
    }
    let name = extract_runs_text(r.get("title")?)?;
    let thumb = extract_last_thumbnail(r.get("thumbnailRenderer"));
    Some(Playlist {
        id: browse_id.to_string(),
        name,
        source: ProviderSource::YouTubeMusic,
        track_count: None,
        thumbnail_url: thumb,
    })
}

fn collect_tracks(val: &Value, out: &mut Vec<Track>) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                if let Some(track) = parse_responsive_track(renderer) {
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
                if let Some(dur) = parse_duration_text(t) {
                    duration_secs = Some(dur);
                } else if let Some(page_type) = run.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(|v| v.as_str()) {
                    if page_type == "MUSIC_PAGE_TYPE_ALBUM" {
                        album = t.to_string();
                    } else if artist.is_empty() {
                        artist = t.to_string();
                    } else {
                        artist = format!("{artist}, {t}");
                    }
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
    })
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

fn find_shelf_continuation(val: &Value) -> Option<String> {
    match val {
        Value::Object(map) => {
            for shelf_key in [
                "musicPlaylistShelfRenderer",
                "musicShelfRenderer",
                "musicPlaylistShelfContinuation",
                "musicShelfContinuation",
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
                .and_then(|c| c.pointer("/continuationEndpoint/continuationCommand/token"))
                .and_then(|v| v.as_str())
            {
                return Some(tok.to_string());
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

fn find_mplyt_browse_id(val: &Value) -> Option<String> {
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

fn find_description_text(val: &Value) -> Option<String> {
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
