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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

mod parsers;

use parsers::{
    apply_collection_backfill, collect_playlists, collect_tracks, extract_search_album_browse_id,
    extract_search_artist_browse_id, find_artist_songs_endpoint, find_collection_header,
    find_description_text, find_home_continuation, find_mplyt_browse_id, find_shelf_continuation,
    is_excluded_shelf_title, merge_home_shelves, CollectionHeader,
};
pub use parsers::{parse_account_profile_json, parse_artist_page_json, parse_home_shelves_json};
const INNERTUBE_BASE: &str = "https://music.youtube.com/youtubei/v1";
const WEB_REMIX_VERSION: &str = "1.20260707.12.00";
const WEB_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

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
    pub description: Option<String>,
    pub subscribers: Option<String>,
    pub monthly_listeners: Option<String>,
    pub thumbnail_url: Option<String>,
    pub shuffle_playlist_id: Option<String>,
    pub radio_playlist_id: Option<String>,
    pub latest_release: Option<HomeCard>,
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
type ArtistCache = Arc<Mutex<HashMap<String, (Instant, ArtistPage)>>>;
type ExplicitCache = Arc<Mutex<HashMap<String, (Instant, Option<Track>)>>>;

#[derive(Debug, Clone, Default)]
struct ChannelScope {
    page_id: Option<String>,
    data_sync_id: Option<String>,
}

#[derive(Clone)]
pub struct YtMusicClient {
    http: Client,
    cookies: ExtractedCookies,
    playlist_cache: PlaylistCache,
    artist_cache: ArtistCache,
    explicit_cache: ExplicitCache,
    visitor_data: Arc<RwLock<Option<String>>>,
    channel_scope: Arc<RwLock<Option<ChannelScope>>>,
    scope_lock: Arc<Mutex<()>>,
}

impl YtMusicClient {
    pub fn new(http: Client, cookies: ExtractedCookies) -> Self {
        Self {
            http,
            cookies,
            playlist_cache: Arc::new(Mutex::new(HashMap::new())),
            artist_cache: Arc::new(Mutex::new(HashMap::new())),
            explicit_cache: Arc::new(Mutex::new(HashMap::new())),
            visitor_data: Arc::new(RwLock::new(None)),
            channel_scope: Arc::new(RwLock::new(None)),
            scope_lock: Arc::new(Mutex::new(())),
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

    async fn ensure_channel_scope(&self) {
        if !self.is_authenticated() {
            return;
        }
        if self.channel_scope.read().await.is_some() {
            return;
        }
        let _guard = self.scope_lock.lock().await;
        if self.channel_scope.read().await.is_some() {
            return;
        }
        let detected = self.fetch_active_brand_scope().await.unwrap_or_default();
        *self.channel_scope.write().await = Some(detected);
    }

    async fn fetch_active_brand_scope(&self) -> Option<ChannelScope> {
        let cookie_hdr = self.cookies.ytm_cookie_header()?;
        let auth_hdr = self
            .cookies
            .ytm_sapisidhash_for_origin("https://www.youtube.com")?;
        let resp = self
            .http
            .get("https://www.youtube.com/getAccountSwitcherEndpoint")
            .header("User-Agent", WEB_USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("X-Origin", "https://www.youtube.com")
            .header("Referer", "https://www.youtube.com/")
            .header("X-Goog-AuthUser", "0")
            .header("Cookie", cookie_hdr)
            .header("Authorization", auth_hdr)
            .timeout(Duration::from_secs(4))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let raw = resp.text().await.ok()?;
        let body = raw
            .strip_prefix(")]}'\n")
            .or_else(|| raw.strip_prefix(")]}'"))
            .unwrap_or(&raw)
            .trim();
        let val: Value = serde_json::from_str(body).ok()?;
        find_selected_brand_scope(&val)
    }

    async fn post(&self, endpoint: &str, mut body: Value) -> Result<Value> {
        if self.is_authenticated() {
            self.ensure_channel_scope().await;
        }
        let visitor = self.visitor_data.read().await.clone();
        let scope = self.channel_scope.read().await.clone().unwrap_or_default();

        if let Some(client) = body
            .pointer_mut("/context/client")
            .and_then(Value::as_object_mut)
        {
            if let Some(ref v) = visitor {
                client.insert("visitorData".to_string(), json!(v));
            }
        }
        if let Some(ref dsid) = scope.data_sync_id {
            if let Some(ctx) = body.get_mut("context").and_then(Value::as_object_mut) {
                ctx.insert(
                    "user".to_string(),
                    json!({ "lockedSafetyMode": false, "onBehalfOfUser": dsid }),
                );
            }
        }

        let url = format!("{INNERTUBE_BASE}/{endpoint}?prettyPrint=false");
        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("User-Agent", WEB_USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Origin", "https://music.youtube.com")
            .header("X-Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", WEB_REMIX_VERSION)
            .json(&body);

        if let Some(ref v) = visitor {
            req = req.header("X-Goog-Visitor-Id", v);
        }

        if let (Some(cookie_hdr), Some(auth_hdr)) = (
            self.cookies.ytm_cookie_header(),
            self.cookies.ytm_sapisidhash(),
        ) {
            req = req
                .header("Cookie", cookie_hdr)
                .header("Authorization", auth_hdr)
                .header("X-Goog-AuthUser", "0");
            if let Some(ref page_id) = scope.page_id {
                req = req.header("X-Goog-PageId", page_id);
            }
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("YouTube Music API HTTP {}", resp.status()));
        }
        let data: Value = resp.json().await?;
        if self.visitor_data.read().await.is_none() {
            if let Some(v) = data
                .pointer("/responseContext/visitorData")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                *self.visitor_data.write().await = Some(v.to_string());
            }
        }
        Ok(data)
    }

    fn base_context() -> Value {
        json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": WEB_REMIX_VERSION,
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

        let mut playlists = vec![Playlist {
            id: "VLLM".to_string(),
            name: "Liked Music".to_string(),
            source: ProviderSource::YouTubeMusic,
            track_count: None,
            thumbnail_url: None,
        }];

        let mut body = Self::base_context();
        body["browseId"] = json!("FEmusic_liked_playlists");
        let data = self.post("browse", body).await?;
        collect_playlists(&data, &mut playlists);

        let mut cont = find_shelf_continuation(&data).or_else(|| find_home_continuation(&data));
        for _ in 0..8 {
            let Some(token) = cont.take() else { break };
            let mut next_body = Self::base_context();
            next_body["continuation"] = json!(token);
            let Ok(page) = self.post("browse", next_body).await else {
                break;
            };
            let before = playlists.len();
            collect_playlists(&page, &mut playlists);
            let next_cont =
                find_shelf_continuation(&page).or_else(|| find_home_continuation(&page));
            if next_cont.as_deref() == Some(token.as_str()) || playlists.len() == before {
                break;
            }
            cont = next_cont;
        }

        if playlists.len() <= 1 {
            let mut landing_body = Self::base_context();
            landing_body["browseId"] = json!("FEmusic_library_landing");
            if let Ok(landing) = self.post("browse", landing_body).await {
                collect_playlists(&landing, &mut playlists);
            }
        }

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

        let mut header = CollectionHeader::default();
        let mut page1 = Vec::new();
        let mut cont = None;

        if let Ok(data) = browse_res {
            header = find_collection_header(&data);
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

        apply_collection_backfill(&mut page1, &browse_id, &header);
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
            apply_collection_backfill(&mut batch, &browse_id, &header);
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
            tokio::time::timeout(Duration::from_secs(5), self.post("browse", exp_body))
        );

        let mut shelves = Vec::new();
        let mut cont = None;

        if let Ok(home_data) = home_res {
            merge_home_shelves(&mut shelves, parse_home_shelves_json(&home_data));
            cont = find_home_continuation(&home_data);
        }

        // Follow up to 2 personalized Home continuation pages BEFORE merging Explore so that
        // Listen again, Mixed for you, Forgotten favorites, etc. are never truncated by Explore.
        for _ in 0..2 {
            let Some(token) = cont.take() else { break };
            let mut next_body = Self::base_context();
            next_body["continuation"] = json!(token);
            let page_res = if shelves.is_empty() {
                self.post("browse", next_body).await.ok()
            } else {
                tokio::time::timeout(Duration::from_secs(5), self.post("browse", next_body))
                    .await
                    .ok()
                    .and_then(Result::ok)
            };
            let Some(page) = page_res else {
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

        // Append discovery shelves from Explore (New albums & singles, Moods & genres).
        if let Ok(Ok(exp_data)) = exp_res {
            merge_home_shelves(&mut shelves, parse_home_shelves_json(&exp_data));
        }

        shelves.retain(|s| !is_excluded_shelf_title(&s.title));
        if shelves.is_empty() {
            return Err(anyhow!("YouTube Music home feed returned no shelves"));
        }
        Ok(shelves)
    }

    /// Fetch an artist only when YouTube Music supplied a concrete browse ID.
    pub async fn fetch_artist_page(&self, browse_id: &str) -> Result<ArtistPage> {
        {
            let guard = self.artist_cache.lock().await;
            if let Some((cached_at, cached)) = guard.get(browse_id) {
                if cached_at.elapsed() < Duration::from_secs(900) {
                    return Ok(cached.clone());
                }
            }
        }

        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        let data = self.post("browse", body).await?;
        let mut page = parse_artist_page_json(browse_id, &data)
            .ok_or_else(|| anyhow!("YouTube Music artist page contained no usable content"))?;

        // Expand Top Songs past the 5 preview rows via the artist's Songs shelf `bottomEndpoint`.
        if let Some((songs_browse_id, songs_params)) = find_artist_songs_endpoint(&data) {
            let mut songs_body = Self::base_context();
            songs_body["browseId"] = json!(songs_browse_id);
            if let Some(params) = songs_params {
                songs_body["params"] = json!(params);
            }
            if let Ok(Ok(songs_data)) =
                tokio::time::timeout(Duration::from_secs(5), self.post("browse", songs_body)).await
            {
                let mut full_tracks = Vec::new();
                collect_tracks(&songs_data, &mut full_tracks);
                if !full_tracks.is_empty() {
                    let mut seen = std::collections::HashSet::new();
                    let mut merged = Vec::new();
                    for t in page.popular_tracks.drain(..).chain(full_tracks) {
                        if !t.id.is_empty() && seen.insert(t.id.clone()) {
                            merged.push(t);
                            if merged.len() >= 24 {
                                break;
                            }
                        }
                    }
                    page.popular_tracks = merged;
                }
            }
        }

        {
            let mut guard = self.artist_cache.lock().await;
            if guard.len() >= 32 {
                if let Some(oldest_key) = guard
                    .iter()
                    .min_by_key(|(_, (ts, _))| *ts)
                    .map(|(k, _)| k.clone())
                {
                    guard.remove(&oldest_key);
                }
            }
            guard.insert(browse_id.to_string(), (Instant::now(), page.clone()));
        }

        Ok(page)
    }

    /// Resolves a primary artist's `UC...` browse ID by querying YTM search with the Artists filter.
    pub async fn resolve_artist_browse_id(&self, name: &str) -> Result<Option<String>> {
        let primary = primary_artist_query(name);
        if primary.is_empty() {
            return Ok(None);
        }

        // Primary query with YouTube Music's Artists search filter.
        let mut body = Self::base_context();
        body["query"] = json!(primary);
        body["params"] = json!("EgWKAQIgAWoKEAkQBRAKEAMQBA%3D%3D");
        let data = self.post("search", body).await?;
        if let Some(id) = extract_search_artist_browse_id(&data, &primary) {
            return Ok(Some(id));
        }

        // Fallback to unfiltered search (Top Result card / linked artist run).
        let mut fallback = Self::base_context();
        fallback["query"] = json!(primary);
        let fb_data = self.post("search", fallback).await?;
        Ok(extract_search_artist_browse_id(&fb_data, &primary))
    }

    /// Resolves `name` to a `UC...` channel ID and fetches the full `ArtistPage`.
    pub async fn fetch_artist_by_name(&self, name: &str) -> Result<ArtistPage> {
        let browse_id = self
            .resolve_artist_browse_id(name)
            .await?
            .ok_or_else(|| anyhow!("No YouTube Music artist channel found for '{name}'"))?;
        self.fetch_artist_page(&browse_id).await
    }

    /// Fetches personalized up-next / radio queue tracks (`RDAMVM<video_id>`) via InnerTube `next`.
    pub async fn fetch_radio_queue(&self, video_id: &str) -> Result<Vec<Track>> {
        let mut body = Self::base_context();
        body["videoId"] = json!(video_id);
        body["playlistId"] = json!(format!("RDAMVM{video_id}"));
        body["isAudioOnly"] = json!(true);
        let data = self.post("next", body).await?;
        let mut tracks = Vec::new();
        collect_tracks(&data, &mut tracks);
        let mut seen = std::collections::HashSet::new();
        tracks.retain(|t| !t.id.is_empty() && seen.insert(t.id.clone()));
        Ok(tracks)
    }

    /// Reports track playback + watchtime telemetry to YouTube Music (`playbackTracking`)
    /// so plays, playlists, and artists count toward Listening History (`FEmusic_history`)
    /// and annual YouTube Music Replay/Recap.
    pub async fn report_playback(
        &self,
        video_id: &str,
        _playlist_id: Option<&str>,
        duration_secs: Option<u64>,
    ) -> Result<()> {
        if !self.is_authenticated() {
            return Ok(());
        }
        let clean_vid = video_id.trim();
        if clean_vid.is_empty()
            || clean_vid.starts_with("local:")
            || clean_vid.starts_with("jiosaavn:")
        {
            return Ok(());
        }

        // Report only the individual played song to YouTube Music history (never attach playlistId).
        let mut body = Self::base_context();
        body["videoId"] = json!(clean_vid);
        body["contentCheckOk"] = json!(true);
        body["racyCheckOk"] = json!(true);

        let data = self.post("player", body).await?;

        let Some(pb_base) = data
            .pointer("/playbackTracking/videostatsPlaybackUrl/baseUrl")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            tracing::debug!(
                video_id = %clean_vid,
                "YTM player response omitted videostatsPlaybackUrl"
            );
            return Ok(());
        };
        let wt_base = data
            .pointer("/playbackTracking/videostatsWatchtimeUrl/baseUrl")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());

        let cpn = generate_cpn();
        let playback_url = build_videostats_url(pb_base, &cpn, None, duration_secs, false);
        self.send_videostats_get(&playback_url).await?;

        if let Some(wt_url_base) = wt_base {
            let watchtime_url = build_videostats_url(wt_url_base, &cpn, None, duration_secs, true);
            if let Err(err) = self.send_videostats_get(&watchtime_url).await {
                tracing::debug!(
                    video_id = %clean_vid,
                    error = %err,
                    "YTM watchtime telemetry ping failed"
                );
            }
        }
        Ok(())
    }

    async fn send_videostats_get(&self, url: &str) -> Result<()> {
        let visitor = self.visitor_data.read().await.clone();
        let scope = self.channel_scope.read().await.clone().unwrap_or_default();

        let mut req = self
            .http
            .get(url)
            .header("User-Agent", WEB_USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Origin", "https://music.youtube.com")
            .header("X-Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", WEB_REMIX_VERSION)
            .timeout(Duration::from_secs(6));

        if let Some(ref v) = visitor {
            req = req.header("X-Goog-Visitor-Id", v);
        }
        if let (Some(cookie_hdr), Some(auth_hdr)) = (
            self.cookies.ytm_cookie_header(),
            self.cookies.ytm_sapisidhash(),
        ) {
            req = req
                .header("Cookie", cookie_hdr)
                .header("Authorization", auth_hdr)
                .header("X-Goog-AuthUser", "0");
            if let Some(ref page_id) = scope.page_id {
                req = req.header("X-Goog-PageId", page_id);
            }
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("YouTube Music telemetry HTTP {}", resp.status()));
        }
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
        self.add_tracks_to_playlist(playlist_browse_id, &[video_id.to_string()])
            .await
    }

    /// Appends one or more tracks to a YouTube Music playlist (`browse/edit_playlist`),
    /// routing `"VLLM"` / `"LM"` to `like/like`.
    pub async fn add_tracks_to_playlist(
        &self,
        playlist_browse_id: &str,
        video_ids: &[String],
    ) -> Result<()> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let mut unique_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in video_ids {
            let clean = id.trim();
            if !clean.is_empty() && seen.insert(clean.to_string()) {
                unique_ids.push(clean.to_string());
            }
        }
        if unique_ids.is_empty() {
            return Ok(());
        }

        let raw_id = playlist_browse_id
            .trim()
            .strip_prefix("VL")
            .unwrap_or(playlist_browse_id.trim());
        if raw_id.is_empty() {
            return Err(anyhow!("Missing target playlist ID"));
        }
        if raw_id == "LM" {
            for vid in &unique_ids {
                self.set_like_status(vid, true).await?;
            }
            return Ok(());
        }

        let actions: Vec<Value> = unique_ids
            .iter()
            .map(|vid| {
                json!({
                    "action": "ACTION_ADD_VIDEO",
                    "addedVideoId": vid
                })
            })
            .collect();
        let mut body = Self::base_context();
        body["playlistId"] = json!(raw_id);
        body["actions"] = json!(actions);
        let resp = self.post("browse/edit_playlist", body).await?;
        if resp.get("status").and_then(|v| v.as_str()) == Some("STATUS_FAILED") {
            return Err(anyhow!("YouTube Music rejected playlist update"));
        }
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
        let resp = self.post("browse/edit_playlist", body).await?;
        if resp.get("status").and_then(|v| v.as_str()) == Some("STATUS_FAILED") {
            return Err(anyhow!("YouTube Music rejected playlist item removal"));
        }
        self.invalidate_cache(Some(playlist_browse_id)).await;
        Ok(())
    }

    pub async fn create_playlist(&self, title: &str) -> Result<String> {
        let created = self.create_playlist_with_tracks(title, "", &[]).await?;
        Ok(created.strip_prefix("VL").unwrap_or(&created).to_string())
    }

    /// Creates a private YouTube Music playlist and optionally seeds it with `video_ids`,
    /// returning the normalized `VL...` browse ID.
    pub async fn create_playlist_with_tracks(
        &self,
        title: &str,
        description: &str,
        video_ids: &[String],
    ) -> Result<String> {
        if !self.is_authenticated() {
            return Err(anyhow!("Sign in to YouTube Music in your browser first"));
        }
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err(anyhow!("Playlist title cannot be empty"));
        }
        let mut clean_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in video_ids {
            let trimmed = id.trim();
            if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                clean_ids.push(trimmed.to_string());
            }
        }
        let mut body = Self::base_context();
        body["title"] = json!(clean_title);
        if !description.trim().is_empty() {
            body["description"] = json!(description.trim());
        }
        body["privacyStatus"] = json!("PRIVATE");
        if !clean_ids.is_empty() {
            body["videoIds"] = json!(clean_ids);
        }
        let resp = self.post("playlist/create", body).await?;
        let pid = resp
            .get("playlistId")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing playlistId in response"))?;
        Ok(if pid.starts_with("VL") {
            pid.to_string()
        } else {
            format!("VL{pid}")
        })
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
        prefer_explicit_order(&mut tracks);
        Ok(tracks)
    }

    /// Resolves an explicit counterpart track when `track` is unbadged or clean/edited.
    pub async fn resolve_explicit_counterpart(&self, track: &Track) -> Option<Track> {
        if track.is_explicit && !is_marked_clean(&track.title) {
            return None;
        }
        {
            let guard = self.explicit_cache.lock().await;
            if let Some((ts, cached)) = guard.get(&track.id) {
                if ts.elapsed() < Duration::from_secs(900) {
                    return cached.clone();
                }
            }
        }

        let target_key = canonical_song_key(&track.title, &track.artist);
        let mut found: Option<Track> = None;

        if !target_key.0.is_empty() {
            if let Some(album_id) = track.album_id.as_deref() {
                let mut body = Self::base_context();
                body["browseId"] = json!(album_id);
                if let Ok(album_data) = self.post("browse", body).await {
                    let mut album_tracks = Vec::new();
                    collect_tracks(&album_data, &mut album_tracks);
                    found = album_tracks.into_iter().find(|cand| {
                        cand.id != track.id
                            && cand.is_explicit
                            && !is_marked_clean(&cand.title)
                            && canonical_song_title(&cand.title) == target_key.0
                    });
                }
            }

            if found.is_none() {
                let primary_artist = primary_artist_query(&track.artist);
                let query = if primary_artist.is_empty() {
                    track.title.clone()
                } else {
                    format!("{} {primary_artist}", canonical_song_title(&track.title))
                };
                if let Ok(candidates) = self.search_songs(&query).await {
                    found = candidates.into_iter().find(|cand| {
                        if cand.id == track.id || !cand.is_explicit || is_marked_clean(&cand.title)
                        {
                            return false;
                        }
                        let cand_key = canonical_song_key(&cand.title, &cand.artist);
                        if cand_key.0 != target_key.0 {
                            return false;
                        }
                        if !target_key.1.is_empty()
                            && !cand_key.1.is_empty()
                            && cand_key.1 != target_key.1
                            && !cand_key.1.contains(&target_key.1)
                            && !target_key.1.contains(&cand_key.1)
                        {
                            return false;
                        }
                        match (track.duration_secs, cand.duration_secs) {
                            (Some(a), Some(b)) if a > 0 && b > 0 => a.abs_diff(b) <= 6,
                            _ => true,
                        }
                    });
                }
            }
        }

        let mut guard = self.explicit_cache.lock().await;
        if guard.len() >= 128 {
            if let Some(oldest_key) = guard
                .iter()
                .min_by_key(|(_, (ts, _))| *ts)
                .map(|(k, _)| k.clone())
            {
                guard.remove(&oldest_key);
            }
        }
        guard.insert(track.id.clone(), (Instant::now(), found.clone()));
        found
    }

    /// Fetches the signed-in listener's reverse-chronological YTM listening history (`FEmusic_history`).
    pub async fn fetch_listening_history(&self) -> Result<Vec<Track>> {
        if !self.is_authenticated() {
            return Ok(Vec::new());
        }
        let mut body = Self::base_context();
        body["browseId"] = json!("FEmusic_history");
        let data = self.post("browse", body).await?;
        let mut tracks = Vec::new();
        collect_tracks(&data, &mut tracks);
        let mut seen = std::collections::HashSet::new();
        tracks.retain(|t| !t.id.is_empty() && seen.insert(t.id.clone()));
        tracks.truncate(50);
        Ok(tracks)
    }

    /// Resolves an album title + artist name to its `MPRE...` / `OLAK...` album browse ID.
    pub async fn resolve_album_browse_id(
        &self,
        album_name: &str,
        artist_name: &str,
    ) -> Result<Option<String>> {
        let clean_album = album_name.trim();
        if clean_album.is_empty() {
            return Ok(None);
        }
        let primary_artist = primary_artist_query(artist_name);
        let query = if primary_artist.is_empty() {
            clean_album.to_string()
        } else {
            format!("{clean_album} {primary_artist}")
        };

        let mut body = Self::base_context();
        body["query"] = json!(query);
        // Filter to Albums.
        body["params"] = json!("EgWKAQIYAWoKEAkQBRAKEAMQBA%3D%3D");
        let data = self.post("search", body).await?;
        if let Some(id) = extract_search_album_browse_id(&data, clean_album) {
            return Ok(Some(id));
        }

        if !primary_artist.is_empty() {
            let mut fallback = Self::base_context();
            fallback["query"] = json!(clean_album);
            fallback["params"] = json!("EgWKAQIYAWoKEAkQBRAKEAMQBA%3D%3D");
            if let Ok(fb_data) = self.post("search", fallback).await {
                if let Some(id) = extract_search_album_browse_id(&fb_data, clean_album) {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

    /// Fetches personalized YouTube Music radio & mix shelves (`FEmusic_mixed_for_you` + Home/Explore mixes),
    /// strictly excluding manual `VLPL...` user playlists.
    pub async fn fetch_radio_mixes(&self) -> Result<Vec<HomeShelf>> {
        let mut mixed_body = Self::base_context();
        mixed_body["browseId"] = json!("FEmusic_mixed_for_you");
        let mut home_body = Self::base_context();
        home_body["browseId"] = json!("FEmusic_home");
        let mut exp_body = Self::base_context();
        exp_body["browseId"] = json!("FEmusic_explore");

        let (mixed_res, home_res, exp_res) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(6), self.post("browse", mixed_body)),
            tokio::time::timeout(Duration::from_secs(6), self.post("browse", home_body)),
            tokio::time::timeout(Duration::from_secs(5), self.post("browse", exp_body)),
        );

        let mut mixed_cards: Vec<HomeCard> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        if let Ok(Ok(mixed_data)) = mixed_res {
            for shelf in parse_home_shelves_json(&mixed_data) {
                for card in shelf.cards {
                    if !is_manual_user_playlist_id(&card.id) && seen_ids.insert(card.id.clone()) {
                        mixed_cards.push(card);
                    }
                }
            }
            let mut cont = find_home_continuation(&mixed_data);
            for _ in 0..2 {
                let Some(token) = cont.take() else { break };
                let mut next_body = Self::base_context();
                next_body["continuation"] = json!(token);
                let Ok(Ok(page)) =
                    tokio::time::timeout(Duration::from_secs(4), self.post("browse", next_body))
                        .await
                else {
                    break;
                };
                for shelf in parse_home_shelves_json(&page) {
                    for card in shelf.cards {
                        if !is_manual_user_playlist_id(&card.id) && seen_ids.insert(card.id.clone())
                        {
                            mixed_cards.push(card);
                        }
                    }
                }
                let next_cont = find_home_continuation(&page);
                if next_cont.as_deref() == Some(token.as_str()) {
                    break;
                }
                cont = next_cont;
            }
        }

        let mut signature_cards = Vec::new();
        let mut personal_mix_cards = Vec::new();
        let mut mood_mix_cards = Vec::new();
        for card in mixed_cards {
            match classify_mix_card(&card.title) {
                MixBucket::Signature => signature_cards.push(card),
                MixBucket::Mood => mood_mix_cards.push(card),
                MixBucket::Personal => personal_mix_cards.push(card),
            }
        }

        let mut out = Vec::new();
        if !signature_cards.is_empty() {
            out.push(HomeShelf {
                title: "Signature Mixes".to_string(),
                tracks: Vec::new(),
                cards: signature_cards,
            });
        }
        if !personal_mix_cards.is_empty() {
            out.push(HomeShelf {
                title: "Your Mixes".to_string(),
                tracks: Vec::new(),
                cards: personal_mix_cards,
            });
        }
        if !mood_mix_cards.is_empty() {
            out.push(HomeShelf {
                title: "Mood Mixes".to_string(),
                tracks: Vec::new(),
                cards: mood_mix_cards,
            });
        }

        // Supplement with mix/radio shelves from Home (+1 continuation) and Explore.
        let mut extra_shelves = Vec::new();
        if let Ok(Ok(home_data)) = home_res {
            extra_shelves.extend(parse_home_shelves_json(&home_data));
            if let Some(token) = find_home_continuation(&home_data) {
                let mut next_body = Self::base_context();
                next_body["continuation"] = json!(token);
                if let Ok(Ok(page)) =
                    tokio::time::timeout(Duration::from_secs(4), self.post("browse", next_body))
                        .await
                {
                    extra_shelves.extend(parse_home_shelves_json(&page));
                }
            }
        }
        if let Ok(Ok(exp_data)) = exp_res {
            extra_shelves.extend(parse_home_shelves_json(&exp_data));
        }

        for shelf in extra_shelves {
            if is_excluded_shelf_title(&shelf.title) {
                continue;
            }
            let lower_title = shelf.title.to_ascii_lowercase();
            let shelf_is_mix_or_radio = lower_title.contains("mix")
                || lower_title.contains("radio")
                || lower_title.contains("mood")
                || lower_title.contains("energy")
                || lower_title.contains("discover")
                || lower_title.contains("station");
            let filtered_cards: Vec<HomeCard> = shelf
                .cards
                .into_iter()
                .filter(|card| {
                    !is_manual_user_playlist_id(&card.id)
                        && (is_mix_playlist_id(&card.id) || shelf_is_mix_or_radio)
                        && seen_ids.insert(card.id.clone())
                })
                .collect();
            if !filtered_cards.is_empty() {
                out.push(HomeShelf {
                    title: shelf.title,
                    tracks: Vec::new(),
                    cards: filtered_cards,
                });
            }
        }

        Ok(out)
    }
}

fn find_selected_brand_scope(val: &Value) -> Option<ChannelScope> {
    fn walk(node: &Value, out: &mut Vec<ChannelScope>) {
        match node {
            Value::Object(map) => {
                if let Some(item) = map.get("accountItem") {
                    if item.get("isSelected").and_then(Value::as_bool) == Some(true) {
                        let page_id = find_nested_string(item, "pageId");
                        let data_sync_id = find_nested_string(item, "datasyncIdToken")
                            .map(|s| normalize_datasync_id(&s));
                        if page_id.is_some() {
                            out.push(ChannelScope {
                                page_id,
                                data_sync_id,
                            });
                        }
                    }
                }
                for v in map.values() {
                    walk(v, out);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    walk(v, out);
                }
            }
            _ => {}
        }
    }

    let mut found = Vec::new();
    walk(val, &mut found);
    found.into_iter().next()
}

fn find_nested_string(val: &Value, key: &str) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(s) = map.get(key).and_then(Value::as_str) {
                if !s.trim().is_empty() {
                    return Some(s.trim().to_string());
                }
            }
            map.values().find_map(|v| find_nested_string(v, key))
        }
        Value::Array(arr) => arr.iter().find_map(|v| find_nested_string(v, key)),
        _ => None,
    }
}

fn normalize_datasync_id(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.split_once("||") {
        Some((left, "")) => left.to_string(),
        _ => trimmed.to_string(),
    }
}

fn primary_artist_query(raw: &str) -> String {
    static PAREN_FEAT_RE: OnceLock<Regex> = OnceLock::new();
    let paren_re = PAREN_FEAT_RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[]\s*(?:feat\.?|ft\.?|featuring|with)\b[^\)\]]*[\)\]]").unwrap()
    });
    let cleaned = paren_re.replace_all(raw.trim(), "");

    static INLINE_FEAT_RE: OnceLock<Regex> = OnceLock::new();
    let inline_re = INLINE_FEAT_RE
        .get_or_init(|| Regex::new(r"(?i)\s+(?:feat\.?|ft\.?|featuring|x|×)\s+.*$").unwrap());
    let no_feat = inline_re.replace(cleaned.trim(), "").trim().to_string();

    let lower = no_feat.to_ascii_lowercase();
    if !lower.starts_with("tyler, the creator") && !lower.starts_with("earth, wind") {
        if let Some((first, _)) = no_feat.split_once(',') {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    no_feat
}

fn is_marked_clean(title: &str) -> bool {
    static CLEAN_TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = CLEAN_TAG_RE.get_or_init(|| {
        Regex::new(r"(?i)[\(\[]\s*(?:clean|edited|radio\s+edit)\s*[\)\]]").unwrap()
    });
    re.is_match(title)
}

fn canonical_song_title(title: &str) -> String {
    static VERSION_TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = VERSION_TAG_RE.get_or_init(|| {
        Regex::new(
            r"(?i)\s*[\(\[]\s*(?:clean|edited|explicit|radio\s+edit|album\s+version)\s*[\)\]]",
        )
        .unwrap()
    });
    re.replace_all(title.trim(), "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn canonical_song_key(title: &str, artist: &str) -> (String, String) {
    (
        canonical_song_title(title),
        primary_artist_query(artist).to_ascii_lowercase(),
    )
}

/// Promotes explicit tracks ahead of clean/unbadged duplicates sharing the same `(title, artist)`
/// while preserving overall search relevance order.
pub fn prefer_explicit_order(tracks: &mut Vec<Track>) {
    if tracks.len() <= 1 {
        return;
    }
    let mut group_first_pos: HashMap<(String, String), usize> = HashMap::new();
    for (idx, t) in tracks.iter().enumerate() {
        let key = canonical_song_key(&t.title, &t.artist);
        if !key.0.is_empty() && !key.1.is_empty() {
            group_first_pos.entry(key).or_insert(idx);
        }
    }

    let mut indexed: Vec<(usize, bool, usize, Track)> = tracks
        .drain(..)
        .enumerate()
        .map(|(orig_idx, t)| {
            let key = canonical_song_key(&t.title, &t.artist);
            let group_pos = group_first_pos.get(&key).copied().unwrap_or(orig_idx);
            let explicit_rank = t.is_explicit && !is_marked_clean(&t.title);
            (group_pos, !explicit_rank, orig_idx, t)
        })
        .collect();

    indexed.sort_by_key(|(group_pos, non_explicit, orig_idx, _)| {
        (*group_pos, *non_explicit, *orig_idx)
    });
    *tracks = indexed.into_iter().map(|(_, _, _, t)| t).collect();
}

fn is_manual_user_playlist_id(id: &str) -> bool {
    let trimmed = id.trim();
    trimmed.starts_with("VLPL")
        || trimmed.starts_with("PL")
        || trimmed == "VLLM"
        || trimmed == "VLSE"
        || trimmed == "LM"
        || trimmed == "SE"
}

fn is_mix_playlist_id(id: &str) -> bool {
    let stripped = id.trim().strip_prefix("VL").unwrap_or(id.trim());
    stripped.starts_with("RDCLAK")
        || stripped.starts_with("RDTMAK")
        || stripped.starts_with("RDEM")
        || stripped.starts_with("RDAMPL")
        || stripped.starts_with("RDAT")
        || stripped.starts_with("RDAMVM")
        || stripped.starts_with("RD")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixBucket {
    Signature,
    Mood,
    Personal,
}

fn classify_mix_card(title: &str) -> MixBucket {
    let lower = title.trim().to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "my supermix"
            | "supermix"
            | "discover mix"
            | "new release mix"
            | "replay mix"
            | "archive mix"
    ) || lower.contains("supermix")
        && !lower.contains("chill")
        && !lower.contains("feel good")
        && !lower.contains("focus")
        && !lower.contains("energy")
        && !lower.contains("workout")
        && !lower.contains("party")
        && !lower.contains("romance")
    {
        return MixBucket::Signature;
    }
    if lower.contains("chill")
        || lower.contains("feel good")
        || lower.contains("focus")
        || lower.contains("energy")
        || lower.contains("workout")
        || lower.contains("party")
        || lower.contains("romance")
        || lower.contains("sleep")
        || lower.contains("commute")
        || lower.contains("sad")
        || lower.contains("mood")
    {
        return MixBucket::Mood;
    }
    MixBucket::Personal
}

#[cfg(test)]
fn normalize_playback_playlist_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let stripped = trimmed.strip_prefix("VL").unwrap_or(trimmed).trim();
    if stripped.is_empty() {
        return None;
    }
    if stripped == "LM"
        || stripped.starts_with("PL")
        || stripped.starts_with("RD")
        || stripped.starts_with("OLAK")
        || stripped.starts_with("UU")
        || stripped.starts_with("LL")
        || stripped.starts_with("FL")
    {
        Some(stripped.to_string())
    } else {
        None
    }
}

fn generate_cpn() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    static CPN_SEQ: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let seq = CPN_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut state = nanos ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA076_1D64_78BD_642F;
    let mut out = String::with_capacity(16);
    for _ in 0..16 {
        // SplitMix64 step ensures uniform 6-bit indexing across the CPN alphabet.
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        out.push(ALPHABET[(z as usize) & 63] as char);
    }
    out
}

fn has_query_param(url: &str, key: &str) -> bool {
    let Some((_, query)) = url.split_once('?') else {
        return false;
    };
    query.split('&').any(|pair| {
        let name = pair.split('=').next().unwrap_or("");
        name == key
    })
}

fn append_query_param(url: &mut String, key: &str, encoded_value: &str) {
    if has_query_param(url, key) {
        return;
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str(key);
    url.push('=');
    url.push_str(encoded_value);
}

fn build_videostats_url(
    base_url: &str,
    cpn: &str,
    _playlist_id: Option<&str>,
    duration_secs: Option<u64>,
    is_watchtime: bool,
) -> String {
    let mut url = if let Some(rest) = base_url.strip_prefix("https://s.youtube.com/") {
        format!("https://music.youtube.com/{rest}")
    } else if let Some(rest) = base_url.strip_prefix("http://s.youtube.com/") {
        format!("https://music.youtube.com/{rest}")
    } else {
        base_url.to_string()
    };

    append_query_param(&mut url, "cpn", cpn);
    append_query_param(&mut url, "ver", "2");
    append_query_param(&mut url, "c", "WEB_REMIX");
    append_query_param(&mut url, "cver", WEB_REMIX_VERSION);
    append_query_param(&mut url, "hl", "en");
    append_query_param(&mut url, "gl", "US");
    if let Some(dur) = duration_secs.filter(|&d| d > 0) {
        append_query_param(&mut url, "len", &format!("{dur}.000"));
    }

    if is_watchtime {
        let watch_secs = duration_secs
            .filter(|&d| d > 0)
            .map_or(30, |d| d.clamp(5, 35));
        let formatted = format!("{watch_secs}.000");
        append_query_param(&mut url, "st", "0.000");
        append_query_param(&mut url, "et", &formatted);
        append_query_param(&mut url, "cmt", &formatted);
        append_query_param(&mut url, "rt", &formatted);
        append_query_param(&mut url, "state", "playing");
        append_query_param(&mut url, "final", "1");
    } else {
        append_query_param(&mut url, "cmt", "0");
    }

    url
}

#[cfg(test)]
mod tests {
    use super::{
        build_videostats_url, generate_cpn, is_manual_user_playlist_id, is_mix_playlist_id,
        normalize_playback_playlist_id, prefer_explicit_order, primary_artist_query,
    };
    use crate::providers::{ProviderSource, Track};

    fn dummy(id: &str, title: &str, artist: &str, is_explicit: bool) -> Track {
        Track {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration_secs: Some(200),
            thumbnail_url: None,
            source: ProviderSource::YouTubeMusic,
            audio_badge: None,
            set_video_id: None,
            liked: false,
            album_id: None,
            is_explicit,
        }
    }

    #[test]
    fn extracts_primary_artist_name_without_features() {
        assert_eq!(primary_artist_query("The Weeknd, Daft Punk"), "The Weeknd");
        assert_eq!(
            primary_artist_query("The Weeknd feat. Daft Punk"),
            "The Weeknd"
        );
        assert_eq!(
            primary_artist_query("The Weeknd (ft. Ariana Grande)"),
            "The Weeknd"
        );
        assert_eq!(
            primary_artist_query("Tyler, The Creator"),
            "Tyler, The Creator"
        );
    }

    #[test]
    fn promotes_explicit_tracks_above_clean_duplicates() {
        let mut tracks = vec![
            dummy("other", "Blinding Lights", "The Weeknd", false),
            dummy("clean", "Starboy (Clean)", "The Weeknd", false),
            dummy("explicit", "Starboy", "The Weeknd", true),
        ];
        prefer_explicit_order(&mut tracks);
        let ids: Vec<&str> = tracks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["other", "explicit", "clean"]);
    }

    #[test]
    fn filters_manual_playlists_and_recognizes_ytm_mixes() {
        assert!(is_manual_user_playlist_id("VLPLabc123"));
        assert!(is_manual_user_playlist_id("PLabc123"));
        assert!(!is_manual_user_playlist_id("RDCLAK5uy_abc"));
        assert!(is_mix_playlist_id("RDCLAK5uy_abc"));
        assert!(is_mix_playlist_id("RDTMAK5uy_xyz"));
        assert!(!is_mix_playlist_id("VLPLabc123"));
    }

    #[test]
    fn normalizes_playback_playlist_ids_and_builds_videostats_urls() {
        assert_eq!(
            normalize_playback_playlist_id("VLPLabc123").as_deref(),
            Some("PLabc123")
        );
        assert_eq!(
            normalize_playback_playlist_id("OLAK5uy_xyz").as_deref(),
            Some("OLAK5uy_xyz")
        );
        assert_eq!(normalize_playback_playlist_id("LM").as_deref(), Some("LM"));
        assert_eq!(normalize_playback_playlist_id("MPREb_album"), None);

        let cpn1 = generate_cpn();
        let cpn2 = generate_cpn();
        assert_eq!(cpn1.len(), 16);
        assert_ne!(cpn1, cpn2);

        let pb = build_videostats_url(
            "https://s.youtube.com/api/stats/playback?docid=abc&ei=123",
            "CPN1234567890123",
            Some("VLPLtest99"),
            Some(210),
            false,
        );
        assert!(pb.starts_with("https://music.youtube.com/api/stats/playback?"));
        assert!(pb.contains("cpn=CPN1234567890123"));
        assert!(pb.contains("c=WEB_REMIX"));
        assert!(!pb.contains("list="));
        assert!(pb.contains("cmt=0"));
        assert!(!pb.contains("final=1"));

        let wt = build_videostats_url(
            "https://s.youtube.com/api/stats/watchtime?docid=abc&ver=2",
            "CPN1234567890123",
            Some("MPREb_album"),
            Some(210),
            true,
        );
        assert!(wt.starts_with("https://music.youtube.com/api/stats/watchtime?"));
        assert!(wt.contains("st=0.000"));
        assert!(wt.contains("et=35.000"));
        assert!(wt.contains("final=1"));
        assert!(!wt.contains("list="));
        assert_eq!(wt.matches("ver=2").count(), 1);
    }
}
