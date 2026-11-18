use crate::config::AppConfig;
use crate::providers::{Playlist, ProviderSource, Track};
use anyhow::{anyhow, Result};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Sha1;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const WEB_PLAYER: &str = "https://open.spotify.com";
const PATHFINDER: &str = "https://api-partner.spotify.com/pathfinder/v2/query";
const CLIENT_TOKEN_URL: &str = "https://clienttoken.spotify.com/v1/clienttoken";
const LIKED_COVER: &str = "https://misc.scdn.co/liked-songs/liked-songs-640.png";
pub const SPOTIFY_LIKED_ID: &str = "spotify:liked";

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebPlayerMeta {
    client_version: String,
    secret_version: u32,
    secret_codes: Vec<u32>,
    hashes: HashMap<String, String>,
    #[serde(default)]
    fetched_at_secs: u64,
}

impl Default for WebPlayerMeta {
    fn default() -> Self {
        let mut hashes = HashMap::new();
        hashes.insert(
            "libraryV3".into(),
            "390c78e5b951029bad359785e69b07b536a509c581cbcd0aded5e5067f187455".into(),
        );
        hashes.insert(
            "fetchPlaylist".into(),
            "21ea11a6ac75a5f3e01fd365d29c18e0a4a8b7dc09f89bbb5d73910190c6acaf".into(),
        );
        hashes.insert(
            "fetchLibraryTracks".into(),
            "087278b20b743578a6262c2b0b4bcd20d879c503cc359a2285baf083ef944240".into(),
        );
        hashes.insert(
            "searchTracks".into(),
            "b02683192a98dde7966b5e6655a79eeb62713eab703eda9902c932818dd52751".into(),
        );
        Self {
            client_version: "1.2.77.0".into(),
            secret_version: 12,
            secret_codes: vec![
                107, 81, 49, 57, 67, 93, 87, 81, 69, 67, 40, 93, 48, 50, 46, 91, 94, 113, 41, 47,
            ],
            hashes,
            fetched_at_secs: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct SessionTokens {
    bearer: String,
    expires_ms: u128,
    client_token: Option<String>,
}

#[derive(Clone)]
pub struct SpotifyClient {
    http: Client,
    sp_dc: Option<String>,
    state: Arc<Mutex<SpotifySession>>,
}

#[derive(Default)]
struct SpotifySession {
    meta: Option<WebPlayerMeta>,
    tokens: Option<SessionTokens>,
    playlist_cache: HashMap<String, (Instant, (String, Vec<Track>))>,
}

impl SpotifyClient {
    pub fn new(http: Client, sp_dc: Option<String>) -> Self {
        Self {
            http,
            sp_dc,
            state: Arc::new(Mutex::new(SpotifySession::default())),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.sp_dc.is_some()
    }

    async fn load_web_player_meta(&self, force: bool) -> WebPlayerMeta {
        {
            let guard = self.state.lock().await;
            if !force {
                if let Some(m) = &guard.meta {
                    return m.clone();
                }
            }
        }

        let cache_path = AppConfig::cache_dir().join("spotify_webplayer.json");
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if !force {
            if let Ok(raw) = std::fs::read_to_string(&cache_path) {
                if let Ok(cached) = serde_json::from_str::<WebPlayerMeta>(&raw) {
                    if now_secs.saturating_sub(cached.fetched_at_secs) < 12 * 3600 {
                        let mut guard = self.state.lock().await;
                        guard.meta = Some(cached.clone());
                        return cached;
                    }
                }
            }
        }

        let mut meta = WebPlayerMeta::default();
        if let Ok(html) = self.http.get(format!("{WEB_PLAYER}/")).send().await {
            if let Ok(html_text) = html.text().await {
                if let Some(ver) = extract_client_version(&html_text) {
                    meta.client_version = ver;
                }
                if let Some(bundle_url) = extract_bundle_url(&html_text) {
                    if let Ok(resp) = self.http.get(&bundle_url).send().await {
                        if let Ok(js) = resp.text().await {
                            if let Some((ver, codes)) = extract_newest_secret(&js) {
                                meta.secret_version = ver;
                                meta.secret_codes = codes;
                            }
                            for op in [
                                "libraryV3",
                                "fetchPlaylist",
                                "fetchLibraryTracks",
                                "searchTracks",
                            ] {
                                if let Some(hash) = extract_query_hash(&js, op) {
                                    meta.hashes.insert(op.to_string(), hash);
                                }
                            }
                            // searchTracks often lives in xpui-routes-search chunk
                            if let Some(search_url) = extract_search_chunk_url(&js) {
                                if let Ok(s_resp) = self.http.get(&search_url).send().await {
                                    if let Ok(s_js) = s_resp.text().await {
                                        if let Some(hash) =
                                            extract_query_hash(&s_js, "searchTracks")
                                        {
                                            meta.hashes.insert("searchTracks".to_string(), hash);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        meta.fetched_at_secs = now_secs;
        if let Ok(serialized) = serde_json::to_string(&meta) {
            let _ = std::fs::write(&cache_path, serialized);
        }

        let mut guard = self.state.lock().await;
        guard.meta = Some(meta.clone());
        meta
    }

    async fn ensure_tokens(&self) -> Result<(SessionTokens, WebPlayerMeta)> {
        let sp_dc = self
            .sp_dc
            .as_deref()
            .ok_or_else(|| anyhow!("Not signed in to Spotify in browser"))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        {
            let guard = self.state.lock().await;
            if let (Some(tok), Some(meta)) = (&guard.tokens, &guard.meta) {
                if now_ms + 30_000 < tok.expires_ms {
                    return Ok((tok.clone(), meta.clone()));
                }
            }
        }

        for force in [false, true] {
            let meta = self.load_web_player_meta(force).await;
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let code = compute_totp(&meta.secret_codes, now_ms);
            let url = format!(
                "{WEB_PLAYER}/api/token?reason=init&productType=web-player&totp={code}&totpServer={code}&totpVer={}",
                meta.secret_version
            );
            let resp = self
                .http
                .get(&url)
                .header("Cookie", format!("sp_dc={sp_dc}"))
                .header("Referer", format!("{WEB_PLAYER}/"))
                .send()
                .await?;
            let json_body: Value = resp.json().await.unwrap_or(Value::Null);
            if json_body.get("totpVerExpired").is_some() && !force {
                continue;
            }
            if json_body.get("isAnonymous").and_then(|v| v.as_bool()) == Some(true) {
                return Err(anyhow!(
                    "Spotify cookie expired; please sign in on open.spotify.com"
                ));
            }
            if let Some(access) = json_body.get("accessToken").and_then(|v| v.as_str()) {
                let expires_ms = json_body
                    .get("accessTokenExpirationTimestampMs")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u128)
                    .unwrap_or(now_ms + 3_600_000);
                let client_id = json_body
                    .get("clientId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let client_token = if let Some(cid) = client_id {
                    self.fetch_client_token(&meta.client_version, &cid)
                        .await
                        .ok()
                } else {
                    None
                };
                let tok = SessionTokens {
                    bearer: access.to_string(),
                    expires_ms,
                    client_token,
                };
                let mut guard = self.state.lock().await;
                guard.tokens = Some(tok.clone());
                return Ok((tok, meta));
            }
        }

        Err(anyhow!("Failed to mint Spotify web-player token"))
    }

    async fn fetch_client_token(&self, client_version: &str, client_id: &str) -> Result<String> {
        let body = json!({
            "client_data": {
                "client_version": client_version,
                "client_id": client_id,
                "js_sdk_data": {
                    "device_brand": "unknown",
                    "device_model": "unknown",
                    "os": "linux",
                    "os_version": "unknown",
                    "device_id": "sonata-linux-tui",
                    "device_type": "computer"
                }
            }
        });
        let resp = self
            .http
            .post(CLIENT_TOKEN_URL)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let data: Value = resp.json().await?;
        data.pointer("/granted_token/token")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Missing client token"))
    }

    async fn gql(&self, op: &str, variables: Value) -> Result<Value> {
        let (tokens, meta) = self.ensure_tokens().await?;
        let hash = meta
            .hashes
            .get(op)
            .cloned()
            .ok_or_else(|| anyhow!("Missing hash for {op}"))?;
        let payload = json!({
            "variables": variables,
            "operationName": op,
            "extensions": {
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": hash
                }
            }
        });
        let mut req = self
            .http
            .post(PATHFINDER)
            .header("Authorization", format!("Bearer {}", tokens.bearer))
            .header("Accept", "application/json")
            .header("app-platform", "WebPlayer")
            .header("Origin", WEB_PLAYER)
            .header("Referer", format!("{WEB_PLAYER}/"))
            .json(&payload);
        if let Some(ct) = tokens.client_token {
            req = req.header("Client-Token", ct);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Spotify Pathfinder HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
    }

    pub async fn fetch_library_playlists(&self) -> Result<Vec<Playlist>> {
        if !self.is_authenticated() {
            return Ok(Vec::new());
        }
        let mut playlists = vec![Playlist {
            id: SPOTIFY_LIKED_ID.to_string(),
            name: "Spotify Liked Songs".to_string(),
            source: ProviderSource::Spotify,
            track_count: None,
            thumbnail_url: Some(LIKED_COVER.to_string()),
        }];

        let mut offset = 0usize;
        loop {
            let vars = json!({
                "filters": ["Playlists"],
                "order": null,
                "textFilter": "",
                "features": ["LIKED_SONGS", "YOUR_EPISODES_V2", "PRERELEASES", "EVENTS"],
                "limit": 50,
                "offset": offset,
                "flatten": true,
                "expandedFolders": [],
                "folderUri": null,
                "includeFoldersWhenFlattening": false
            });
            let root = self.gql("libraryV3", vars).await?;
            let items = root
                .pointer("/data/me/libraryV3/items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let raw_len = items.len();
            for item in items {
                let data = item.pointer("/item/data");
                let uri = item
                    .pointer("/item/_uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let Some(id) = extract_playlist_id(uri) else {
                    continue;
                };
                let name = data
                    .and_then(|d| d.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Spotify Playlist")
                    .to_string();
                let cover = data
                    .and_then(|d| d.pointer("/images/items/0/sources"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.last())
                    .and_then(|s| s.get("url"))
                    .and_then(|u| u.as_str())
                    .map(str::to_string);
                playlists.push(Playlist {
                    id,
                    name,
                    source: ProviderSource::Spotify,
                    track_count: None,
                    thumbnail_url: cover,
                });
            }
            let total = root
                .pointer("/data/me/libraryV3/totalCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            offset += raw_len;
            if raw_len == 0 || offset >= total || offset >= 1000 {
                break;
            }
        }

        Ok(playlists)
    }

    pub async fn fetch_playlist_tracks_progressive<F>(
        &self,
        playlist_id: &str,
        mut on_batch: F,
    ) -> Result<()>
    where
        F: FnMut(String, Vec<Track>, bool) + Send,
    {
        {
            let guard = self.state.lock().await;
            if let Some((at, (name, cached))) = guard.playlist_cache.get(playlist_id) {
                if at.elapsed() < Duration::from_secs(5 * 60) {
                    on_batch(name.clone(), cached.clone(), true);
                    return Ok(());
                }
            }
        }

        let result = if playlist_id == SPOTIFY_LIKED_ID {
            self.fetch_liked_tracks(&mut on_batch).await?
        } else if self.is_authenticated() {
            match self.fetch_gql_playlist(playlist_id, &mut on_batch).await {
                Ok(res) => res,
                Err(_) => {
                    let res = self.fetch_embed_playlist(playlist_id).await?;
                    on_batch(res.0.clone(), res.1.clone(), true);
                    res
                }
            }
        } else {
            let res = self.fetch_embed_playlist(playlist_id).await?;
            on_batch(res.0.clone(), res.1.clone(), true);
            res
        };

        let mut guard = self.state.lock().await;
        guard
            .playlist_cache
            .insert(playlist_id.to_string(), (Instant::now(), result));
        Ok(())
    }

    async fn fetch_liked_tracks<F>(&self, on_batch: &mut F) -> Result<(String, Vec<Track>)>
    where
        F: FnMut(String, Vec<Track>, bool) + Send,
    {
        let mut tracks = Vec::new();
        let mut offset = 0;
        let mut is_first = true;
        loop {
            let root = self
                .gql(
                    "fetchLibraryTracks",
                    json!({ "offset": offset, "limit": 100 }),
                )
                .await?;
            let items = root
                .pointer("/data/me/library/tracks/items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let raw_len = items.len();
            let mut batch = Vec::new();
            for entry in items {
                if let Some(mut track) = parse_gql_track(entry.pointer("/track/data")) {
                    track.liked = true;
                    batch.push(track);
                }
            }
            on_batch("Spotify Liked Songs".to_string(), batch.clone(), is_first);
            is_first = false;
            tracks.extend(batch);

            let total = root
                .pointer("/data/me/library/tracks/totalCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            offset += raw_len;
            if raw_len == 0 || offset >= total || offset >= 3000 {
                break;
            }
        }
        Ok(("Spotify Liked Songs".to_string(), tracks))
    }

    async fn fetch_gql_playlist<F>(
        &self,
        playlist_id: &str,
        on_batch: &mut F,
    ) -> Result<(String, Vec<Track>)>
    where
        F: FnMut(String, Vec<Track>, bool) + Send,
    {
        let mut tracks = Vec::new();
        let mut offset = 0;
        let mut title = "Spotify Playlist".to_string();
        let mut is_first = true;
        loop {
            let root = self
                .gql(
                    "fetchPlaylist",
                    json!({
                        "uri": format!("spotify:playlist:{playlist_id}"),
                        "offset": offset,
                        "limit": 100,
                        "enableWatchFeedEntrypoint": false
                    }),
                )
                .await?;
            if let Some(name) = root
                .pointer("/data/playlistV2/name")
                .and_then(|v| v.as_str())
            {
                title = name.to_string();
            }
            let items = root
                .pointer("/data/playlistV2/content/items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let raw_len = items.len();
            let mut batch = Vec::new();
            for entry in items {
                if let Some(track) = parse_gql_track(entry.pointer("/itemV2/data")) {
                    batch.push(track);
                }
            }
            on_batch(title.clone(), batch.clone(), is_first);
            is_first = false;
            tracks.extend(batch);

            let total = root
                .pointer("/data/playlistV2/content/totalCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            offset += raw_len;
            if raw_len == 0 || offset >= total || offset >= 3000 {
                break;
            }
        }
        Ok((title, tracks))
    }

    /// Fetches public Spotify playlist metadata and tracks with zero login or API key.
    pub async fn fetch_embed_playlist(&self, playlist_id: &str) -> Result<(String, Vec<Track>)> {
        let url = format!("{WEB_PLAYER}/embed/playlist/{playlist_id}");
        let html = self.http.get(&url).send().await?.text().await?;
        parse_embed_playlist_html(&html)
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        if !self.is_authenticated() {
            return Ok(Vec::new());
        }
        let vars = json!({
            "searchTerm": query,
            "offset": 0,
            "limit": 20,
            "numberOfTopResults": 5,
            "includeAudiobooks": false,
            "includePreReleases": false
        });
        let root = self.gql("searchTracks", vars).await?;
        let mut tracks = Vec::new();
        if let Some(items) = root
            .pointer("/data/searchV2/tracksV2/items")
            .and_then(|v| v.as_array())
        {
            for item in items {
                if let Some(track) = parse_gql_track(item.pointer("/item/data")) {
                    tracks.push(track);
                }
            }
        }
        Ok(tracks)
    }
}

pub fn extract_playlist_id(input: &str) -> Option<String> {
    static RE_ID: OnceLock<Regex> = OnceLock::new();
    let re = RE_ID.get_or_init(|| Regex::new(r"^[A-Za-z0-9]{22}$").unwrap());
    let trimmed = input.trim();
    if re.is_match(trimmed) {
        return Some(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        let id = rest.split(['?', '/']).next()?;
        if re.is_match(id) {
            return Some(id.to_string());
        }
    }
    if let Some(idx) = trimmed.find("/playlist/") {
        let after = &trimmed[idx + "/playlist/".len()..];
        let id = after.split(['?', '/', '&']).next()?;
        if re.is_match(id) {
            return Some(id.to_string());
        }
    }
    None
}

pub fn parse_embed_playlist_html(html: &str) -> Result<(String, Vec<Track>)> {
    static RE_NEXT: OnceLock<Regex> = OnceLock::new();
    let re = RE_NEXT.get_or_init(|| {
        Regex::new(r#"(?s)<script id="__NEXT_DATA__" type="application/json">(.*?)</script>"#)
            .unwrap()
    });
    let caps = re
        .captures(html)
        .ok_or_else(|| anyhow!("Could not find Spotify embed playlist data"))?;
    let json_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let root: Value = serde_json::from_str(json_str)?;
    let entity = root
        .pointer("/props/pageProps/state/data/entity")
        .ok_or_else(|| anyhow!("Missing playlist entity in Spotify embed"))?;

    let playlist_name = entity
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Spotify Playlist")
        .to_string();

    let default_cover = entity
        .pointer("/coverArt/sources")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(str::to_string);

    let mut tracks = Vec::new();
    if let Some(list) = entity.get("trackList").and_then(|v| v.as_array()) {
        for item in list {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            let artist = item
                .get("subtitle")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Artist")
                .trim()
                .to_string();
            let duration_secs = item
                .get("duration")
                .and_then(|v| v.as_u64())
                .map(|ms| ms / 1000);
            let uri = item
                .get("uri")
                .and_then(|v| v.as_str())
                .unwrap_or(&title)
                .to_string();
            tracks.push(Track {
                id: uri,
                title,
                artist,
                album: playlist_name.clone(),
                duration_secs,
                thumbnail_url: default_cover.clone(),
                source: ProviderSource::Spotify,
                audio_badge: None,
                set_video_id: None,
                liked: false,
            });
        }
    }

    Ok((playlist_name, tracks))
}

fn parse_gql_track(data: Option<&Value>) -> Option<Track> {
    let d = data?;
    let title = d.get("name").and_then(|v| v.as_str())?.to_string();
    let uri = d
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or(&title)
        .to_string();

    let artists = d
        .pointer("/artists/items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.pointer("/profile/name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = d
        .pointer("/albumOfTrack/name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let duration_secs = d
        .pointer("/duration/totalMilliseconds")
        .or_else(|| d.pointer("/trackDuration/totalMilliseconds"))
        .and_then(|v| v.as_u64())
        .map(|ms| ms / 1000);

    let thumb = d
        .pointer("/albumOfTrack/coverArt/sources")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .max_by_key(|s| s.get("width").and_then(|w| w.as_u64()).unwrap_or(0))
        })
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Some(Track {
        id: uri,
        title,
        artist: artists,
        album,
        duration_secs,
        thumbnail_url: thumb,
        source: ProviderSource::Spotify,
        audio_badge: None,
        set_video_id: None,
        liked: false,
    })
}

pub fn compute_totp(codes: &[u32], epoch_ms: u128) -> String {
    let key_str: String = codes
        .iter()
        .enumerate()
        .map(|(i, c)| ((*c as usize) ^ ((i % 33) + 9)).to_string())
        .collect();
    let step = (epoch_ms / 1000 / 30) as u64;
    let counter = step.to_be_bytes();
    let mut mac =
        HmacSha1::new_from_slice(key_str.as_bytes()).expect("HMAC accepts any key length");
    mac.update(&counter);
    let hash = mac.finalize().into_bytes();
    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let bin = ((hash[offset] as u32 & 0x7f) << 24)
        | ((hash[offset + 1] as u32) << 16)
        | ((hash[offset + 2] as u32) << 8)
        | (hash[offset + 3] as u32);
    format!("{:06}", bin % 1_000_000)
}

fn extract_client_version(html: &str) -> Option<String> {
    let re =
        Regex::new(r#"<script id="appServerConfig" type="text/plain">([^<]+)</script>"#).ok()?;
    let b64 = re.captures(html)?.get(1)?.as_str();
    let decoded = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let val: Value = serde_json::from_slice(&decoded).ok()?;
    val.get("clientVersion")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn extract_bundle_url(html: &str) -> Option<String> {
    let re = Regex::new(
        r#"https://open\.spotifycdn\.com/cdn/build/web-player/web-player\.[0-9a-f]+\.js"#,
    )
    .ok()?;
    re.find(html).map(|m| m.as_str().to_string())
}

fn extract_search_chunk_url(bundle: &str) -> Option<String> {
    let id_re = Regex::new(r#"(\d+):"xpui-routes-search""#).ok()?;
    let id = id_re.captures(bundle)?.get(1)?.as_str();
    let hash_re = Regex::new(&format!(r#"\b{id}:"([0-9a-f]{{8}})""#)).ok()?;
    let part = hash_re.captures(bundle)?.get(1)?.as_str();
    Some(format!(
        "https://open.spotifycdn.com/cdn/build/web-player/xpui-routes-search.{part}.js"
    ))
}

fn unescape_js_literal(raw: &str) -> Vec<u32> {
    let mut out = Vec::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c as u32);
            continue;
        }
        match chars.next() {
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(b) = u32::from_str_radix(&hex, 16) {
                    out.push(b);
                }
            }
            Some('u') => {
                let hex: String = if chars.peek() == Some(&'{') {
                    chars.next();
                    chars.by_ref().take_while(|&ch| ch != '}').collect()
                } else {
                    chars.by_ref().take(4).collect()
                };
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    out.push(cp);
                }
            }
            Some('n') => out.push('\n' as u32),
            Some('r') => out.push('\r' as u32),
            Some('t') => out.push('\t' as u32),
            Some('0') => out.push(0),
            Some(other) => out.push(other as u32),
            None => break,
        }
    }
    out
}

fn extract_newest_secret(js: &str) -> Option<(u32, Vec<u32>)> {
    let re = Regex::new(r#"\{secret:(?:'((?:\\.|[^'\\])*)'|"((?:\\.|[^"\\])*)"),version:(\d+)\}"#)
        .ok()?;
    let mut best: Option<(u32, Vec<u32>)> = None;
    for cap in re.captures_iter(js) {
        let ver: u32 = cap[3].parse().ok()?;
        let body = cap.get(1).or_else(|| cap.get(2))?.as_str();
        let codes = unescape_js_literal(body);
        if best.as_ref().is_none_or(|(b, _)| ver > *b) {
            best = Some((ver, codes));
        }
    }
    best
}

fn extract_query_hash(js: &str, op: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"["']{op}["']\s*,\s*["']query["']\s*,\s*["']([0-9a-f]{{64}})["']"#
    ))
    .ok()?;
    re.captures(js)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_spotify_playlist_ids() {
        let id = "37i9dQZF1DXcBWIGoYBM5M";
        assert_eq!(
            extract_playlist_id(&format!("https://open.spotify.com/playlist/{id}?si=123")),
            Some(id.to_string())
        );
        assert_eq!(
            extract_playlist_id(&format!("https://open.spotify.com/intl-tr/playlist/{id}")),
            Some(id.to_string())
        );
        assert_eq!(
            extract_playlist_id(&format!("spotify:playlist:{id}")),
            Some(id.to_string())
        );
        assert_eq!(extract_playlist_id(id), Some(id.to_string()));
        assert_eq!(extract_playlist_id("invalid"), None);
    }

    #[test]
    fn generates_six_digit_totp() {
        let codes = vec![107, 81, 49, 57, 67, 93, 87, 81];
        let code = compute_totp(&codes, 1_700_000_000_000);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn parses_embed_html_playlist() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Chill Vibes","coverArt":{"sources":[{"url":"https://i.scdn.co/image/cover640"}]},"trackList":[{"uri":"spotify:track:1","title":"Midnight City","subtitle":"M83","duration":243000}]}}}}}}</script></html>"#;
        let (name, tracks) = parse_embed_playlist_html(html).expect("valid embed");
        assert_eq!(name, "Chill Vibes");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Midnight City");
        assert_eq!(tracks[0].artist, "M83");
        assert_eq!(tracks[0].duration_secs, Some(243));
    }
}
