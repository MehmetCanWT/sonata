//! YouTube Music client for Sonata, built on the authenticated InnerTube web API.
//!
//! 1. **Client & Auth**: `YtMusicClient` signs requests with the extracted cookies (`SAPISIDHASH`) and
//!    keeps short-lived in-memory response caches.
//! 2. **Library & Playlists**: library playlists and progressively paginated playlist tracks.
//! 3. **Discovery**: account profile, home-feed shelves and cards, artist pages, and song search.
//! 4. **Mutations**: like/unlike, add or remove playlist items, and playlist creation.
//! 5. **Lyrics**: native YouTube Music lyrics lookup used by the lyrics chain.
//! 6. **Response Parsing**: tolerant JSON-tree parsers (`parse_home_shelves_json`,
//!    `parse_account_profile_json`, `parse_artist_page_json`) covered by unit tests.

use crate::auth::ExtractedCookies;
use crate::providers::{Playlist, ProviderSource, Track};
use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

mod parsers;

use parsers::{
    collect_playlists, collect_tracks, find_description_text, find_header_thumbnail,
    find_home_continuation, find_mplyt_browse_id, find_shelf_continuation, merge_home_shelves,
};
pub use parsers::{parse_account_profile_json, parse_artist_page_json, parse_home_shelves_json};
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
    Mood,
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
        let browse_res = self.post("browse", body).await;

        let mut header_thumb = None;
        let mut page1 = Vec::new();
        let mut cont = None;

        if let Ok(data) = browse_res {
            header_thumb = find_header_thumbnail(&data);
            collect_tracks(&data, &mut page1);
            cont = find_shelf_continuation(&data);
        }

        // Personal mixes / radio playlists (e.g. RDTMAK*, RDAMVM*, RDCLAK*, OLAK*) often
        // resolve via the `next` watch-playlist endpoint rather than `browse`.
        if page1.is_empty() {
            let raw_playlist_id = browse_id.strip_prefix("VL").unwrap_or(&browse_id);
            let mut next_body = Self::base_context();
            next_body["playlistId"] = json!(raw_playlist_id);
            if let Ok(next_data) = self.post("next", next_body).await {
                collect_tracks(&next_data, &mut page1);
                cont = find_shelf_continuation(&next_data);
            }
        }

        if page1.is_empty() && cont.is_none() {
            return Err(anyhow!(
                "YouTube Music returned no tracks for collection {browse_id}"
            ));
        }

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
        let mut home_body = Self::base_context();
        home_body["browseId"] = json!("FEmusic_home");
        let mut exp_body = Self::base_context();
        exp_body["browseId"] = json!("FEmusic_explore");

        let (home_res, exp_res) = tokio::join!(
            self.post("browse", home_body),
            self.post("browse", exp_body)
        );

        let mut shelves = Vec::new();
        let mut cont = None;

        if let Ok(home_data) = home_res {
            merge_home_shelves(&mut shelves, parse_home_shelves_json(&home_data));
            cont = find_home_continuation(&home_data);
        }

        // Merge discovery shelves from Explore (e.g. New albums & singles, Moods & genres)
        // before deciding whether an additional Home continuation page is needed.
        if let Ok(exp_data) = exp_res {
            merge_home_shelves(&mut shelves, parse_home_shelves_json(&exp_data));
        }

        // Follow at most 1 continuation page, and skip altogether if >= 6 shelves are already loaded.
        if shelves.len() < 6 {
            for _ in 0..1 {
                let Some(token) = cont.take() else { break };
                let mut next_body = Self::base_context();
                next_body["continuation"] = json!(token);
                let Ok(page) = self.post("browse", next_body).await else {
                    break;
                };
                let before = shelves.len();
                merge_home_shelves(&mut shelves, parse_home_shelves_json(&page));
                let next_cont = find_home_continuation(&page);
                if next_cont.as_deref() == Some(token.as_str()) || shelves.len() == before {
                    break;
                }
                cont = next_cont;
            }
        }

        if shelves.is_empty() {
            return Err(anyhow!("YouTube Music home feed returned no shelves"));
        }
        Ok(shelves)
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
