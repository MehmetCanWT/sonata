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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtAccountProfile {
    pub name: String,
    pub email: Option<String>,
    pub handle: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeCardKind {
    Playlist,
    Album,
    Artist,
    Song,
    Video,
}

#[derive(Debug, Clone)]
pub struct HomeCard {
    pub kind: HomeCardKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub track: Option<Track>,
}

#[derive(Debug, Clone)]
pub struct HomeShelf {
    pub title: String,
    pub tracks: Vec<Track>,
    pub cards: Vec<HomeCard>,
}

#[derive(Debug, Clone, Default)]
pub struct ArtistPage {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub popular_tracks: Vec<Track>,
    pub releases: Vec<HomeCard>,
    pub related_artists: Vec<HomeCard>,
}

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
        let browse_id = if browse_or_id.starts_with("VL")
            || browse_or_id.starts_with("FE")
            || browse_or_id.starts_with("MPRE")
        {
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
        let header_thumb = find_header_thumbnail(&data);

        let mut page1 = Vec::new();
        collect_tracks(&data, &mut page1);
        for t in &mut page1 {
            if browse_id == "VLLM" {
                t.liked = true;
            }
            if t.thumbnail_url.is_none() {
                t.thumbnail_url = header_thumb.clone();
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
            for t in &mut batch {
                if browse_id == "VLLM" {
                    t.liked = true;
                }
                if t.thumbnail_url.is_none() {
                    t.thumbnail_url = header_thumb.clone();
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

    pub async fn fetch_account_profile(&self) -> Result<YtAccountProfile> {
        if !self.is_authenticated() {
            return Err(anyhow!("Not signed in to YouTube Music"));
        }
        let data = self
            .post("account/account_menu", Self::base_context())
            .await?;
        parse_account_profile_json(&data)
            .ok_or_else(|| anyhow!("Could not read YouTube Music profile from session cookies"))
    }

    pub async fn fetch_home_feed(&self) -> Result<Vec<HomeShelf>> {
        if self.is_authenticated() {
            let mut home_body = Self::base_context();
            home_body["browseId"] = json!("FEmusic_home");
            let mut shelves = match self.post("browse", home_body).await {
                Ok(data) => parse_home_shelves_json(&data),
                Err(_) => Vec::new(),
            };
            if shelves.len() < 3 {
                let mut exp_body = Self::base_context();
                exp_body["browseId"] = json!("FEmusic_explore");
                if let Ok(exp_data) = self.post("browse", exp_body).await {
                    merge_home_shelves(&mut shelves, parse_home_shelves_json(&exp_data));
                }
            }
            if shelves.is_empty() {
                return Err(anyhow!("YouTube Music home feed returned no shelves"));
            }
            Ok(shelves)
        } else {
            let mut exp_body = Self::base_context();
            exp_body["browseId"] = json!("FEmusic_explore");
            let mut home_body = Self::base_context();
            home_body["browseId"] = json!("FEmusic_home");
            let (exp_res, home_res) = tokio::join!(
                self.post("browse", exp_body),
                self.post("browse", home_body)
            );
            let mut shelves = Vec::new();
            if let Ok(exp_data) = exp_res {
                merge_home_shelves(&mut shelves, parse_home_shelves_json(&exp_data));
            }
            if let Ok(home_data) = home_res {
                merge_home_shelves(&mut shelves, parse_home_shelves_json(&home_data));
            }
            if shelves.is_empty() {
                return Err(anyhow!("Failed to load public YouTube Music feed"));
            }
            Ok(shelves)
        }
    }

    /// Fetch an artist only when YouTube Music supplied a concrete browse ID.
    pub async fn fetch_artist_page(&self, browse_id: &str) -> Result<ArtistPage> {
        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        let data = self.post("browse", body).await?;
        parse_artist_page_json(browse_id, &data)
            .ok_or_else(|| anyhow!("YouTube Music artist page contained no usable content"))
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

fn find_header_thumbnail(val: &Value) -> Option<String> {
    let hdr = val.get("header")?;
    for key in [
        "musicResponsiveHeaderRenderer",
        "musicDetailHeaderRenderer",
        "musicVisualHeaderRenderer",
    ] {
        if let Some(node) = hdr.get(key) {
            if let Some(t) = extract_last_thumbnail(node.get("thumbnail")) {
                return Some(t);
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

fn merge_home_shelves(dst: &mut Vec<HomeShelf>, src: Vec<HomeShelf>) {
    for shelf in src {
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
    shelves
}

fn collect_home_shelves(val: &Value, out: &mut Vec<HomeShelf>) {
    match val {
        Value::Object(map) => {
            if let Some(carousel) = map.get("musicCarouselShelfRenderer") {
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
            for v in map.values() {
                collect_home_shelves(v, out);
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

fn parse_shelf_node(node: &Value, is_carousel: bool) -> Option<HomeShelf> {
    let title = if is_carousel {
        node.pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
            .and_then(extract_runs_text)
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
        if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
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
        return None;
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

pub fn parse_artist_page_json(browse_id: &str, val: &Value) -> Option<ArtistPage> {
    let name = val
        .pointer("/header/musicImmersiveHeaderRenderer/title")
        .and_then(extract_runs_text)
        .or_else(|| {
            val.pointer("/header/musicVisualHeaderRenderer/title")
                .and_then(extract_runs_text)
        })
        .or_else(|| find_first_runs_text(val, "title"))?;
    let subtitle = val
        .pointer("/header/musicImmersiveHeaderRenderer/description")
        .and_then(extract_runs_text)
        .or_else(|| {
            val.pointer("/header/musicVisualHeaderRenderer/description")
                .and_then(extract_runs_text)
        })
        .unwrap_or_default();
    let thumbnail_url = find_header_thumbnail(val);
    let mut popular_tracks = Vec::new();
    collect_tracks(val, &mut popular_tracks);
    popular_tracks.truncate(10);
    let mut cards = Vec::new();
    collect_artist_cards(val, &mut cards);
    let mut releases = Vec::new();
    let mut related_artists = Vec::new();
    for card in cards {
        match card.kind {
            HomeCardKind::Album => releases.push(card),
            HomeCardKind::Artist => related_artists.push(card),
            _ => {}
        }
    }
    releases.truncate(24);
    related_artists.truncate(12);
    Some(ArtistPage {
        id: browse_id.to_string(),
        name,
        subtitle,
        thumbnail_url,
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
}
