//! Desktop-only YouTube Music InnerTube browse, explore, album, mood, and search extensions.
//!
//! 1. **Request Transport (`ExtraYtmClient`)**: issues authenticated/anonymous `WEB_REMIX` InnerTube requests using `ExtractedCookies` without altering `sonata-core`.
//! 2. **Home Chips & Explore**: parses `chipCloudChipRenderer` mood pills, `FEmusic_explore` shelves, and `FEmusic_moods_and_genres` tiles.
//! 3. **Album & Artist Enhancements**: builds rich `AlbumDetail` (`Other versions`, clickable artists, save state) and wide `0.36` aspect artist channel banners.
//! 4. **Typed Search & Library**: parses live `music/get_search_suggestions`, typed search groups (`Songs`, `Videos`, `Albums`, `Artists`, `Playlists`), and library albums/artists.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use sonata_core::auth::ExtractedCookies;
use sonata_core::providers::ytmusic::{
    parse_home_shelves_json, upscale_thumbnail, HomeCard, HomeCardKind, HomeShelf,
};
use sonata_core::providers::{parse_duration_text, ProviderSource, Track};
use std::time::Duration;

const INNERTUBE_BASE: &str = "https://music.youtube.com/youtubei/v1";
const WEB_REMIX_VERSION: &str = "1.20260707.12.00";
const WEB_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoodChip {
    pub label: String,
    pub params: String,
}

#[derive(Debug, Clone)]
pub struct MoodCategoryTile {
    pub title: String,
    pub browse_id: String,
    pub params: String,
    pub accent_rgb: (u8, u8, u8),
}

#[derive(Debug, Clone)]
pub struct AlbumDetail {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub release_type: String,
    pub year: Option<String>,
    pub duration_text: Option<String>,
    pub cover_url: Option<String>,
    pub artists: Vec<(String, Option<String>)>,
    pub tracks: Vec<Track>,
    pub other_versions: Vec<HomeCard>,
    pub playlist_id: Option<String>,
    pub saved: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RichSearchResult {
    pub songs: Vec<Track>,
    pub videos: Vec<Track>,
    pub albums: Vec<HomeCard>,
    pub artists: Vec<HomeCard>,
    pub playlists: Vec<HomeCard>,
}

#[derive(Clone)]
pub struct ExtraYtmClient {
    http: Client,
    cookies: ExtractedCookies,
}

impl ExtraYtmClient {
    pub fn new(http: Client, cookies: ExtractedCookies) -> Self {
        Self { http, cookies }
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

    async fn post(&self, endpoint: &str, body: Value) -> Result<Value> {
        let url = format!("{INNERTUBE_BASE}/{endpoint}?prettyPrint=false");
        let mut req = self
            .http
            .post(&url)
            .header("User-Agent", WEB_USER_AGENT)
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("X-Origin", "https://music.youtube.com")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", WEB_REMIX_VERSION)
            .header("X-Goog-AuthUser", "0")
            .timeout(Duration::from_secs(10));

        if let (Some(cookie), Some(auth)) = (
            self.cookies.ytm_cookie_header(),
            self.cookies.ytm_sapisidhash(),
        ) {
            req = req.header("Cookie", cookie).header("Authorization", auth);
        }

        let resp = req.json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("InnerTube `{endpoint}` returned {}", resp.status()));
        }
        Ok(resp.json::<Value>().await?)
    }

    pub async fn fetch_home_with_chips(
        &self,
        params: Option<&str>,
    ) -> Result<(Vec<MoodChip>, Vec<HomeShelf>)> {
        let mut body = Self::base_context();
        body["browseId"] = json!("FEmusic_home");
        if let Some(p) = params.filter(|s| !s.is_empty()) {
            body["params"] = json!(p);
        }
        let data = self.post("browse", body).await?;
        let chips = extract_mood_chips(&data);
        let shelves = parse_home_shelves_json(&data);
        Ok((chips, shelves))
    }

    pub async fn fetch_explore(&self) -> Result<(Vec<HomeShelf>, Vec<MoodCategoryTile>)> {
        let mut exp_body = Self::base_context();
        exp_body["browseId"] = json!("FEmusic_explore");
        let mut moods_body = Self::base_context();
        moods_body["browseId"] = json!("FEmusic_moods_and_genres");

        let (exp_res, moods_res) = tokio::join!(
            self.post("browse", exp_body),
            tokio::time::timeout(Duration::from_secs(6), self.post("browse", moods_body)),
        );

        let shelves = exp_res
            .as_ref()
            .map(parse_home_shelves_json)
            .unwrap_or_default();
        let mut tiles = moods_res
            .ok()
            .and_then(Result::ok)
            .map(|v| extract_mood_tiles(&v))
            .unwrap_or_default();
        if tiles.is_empty() {
            if let Ok(ref v) = exp_res {
                tiles = extract_mood_tiles(v);
            }
        }
        Ok((shelves, tiles))
    }

    pub async fn fetch_mood_category(
        &self,
        browse_id: &str,
        params: &str,
    ) -> Result<Vec<HomeShelf>> {
        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        if !params.is_empty() {
            body["params"] = json!(params);
        }
        let data = self.post("browse", body).await?;
        Ok(parse_home_shelves_json(&data))
    }

    pub async fn fetch_library_albums_and_artists(&self) -> (Vec<HomeCard>, Vec<HomeCard>) {
        let mut albums_body = Self::base_context();
        albums_body["browseId"] = json!("FEmusic_liked_albums");
        let mut artists_body = Self::base_context();
        artists_body["browseId"] = json!("FEmusic_library_corpus_track_artists");

        let (alb_res, art_res) = tokio::join!(
            self.post("browse", albums_body),
            self.post("browse", artists_body),
        );

        let albums = alb_res
            .map(|v| parse_cards_tree(&v, HomeCardKind::Album))
            .unwrap_or_default();
        let artists = art_res
            .map(|v| parse_cards_tree(&v, HomeCardKind::Artist))
            .unwrap_or_default();
        (albums, artists)
    }

    pub async fn fetch_album_detail(&self, browse_id: &str) -> Result<AlbumDetail> {
        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        let data = self.post("browse", body).await?;
        parse_album_detail_json(browse_id, &data)
            .ok_or_else(|| anyhow!("Could not parse YouTube Music album details"))
    }

    pub async fn fetch_artist_banner(&self, browse_id: &str) -> Option<String> {
        let mut body = Self::base_context();
        body["browseId"] = json!(browse_id);
        let data = self.post("browse", body).await.ok()?;
        extract_artist_banner_url(&data)
    }

    pub async fn set_dislike_status(&self, video_id: &str) -> Result<()> {
        let mut body = Self::base_context();
        body["target"] = json!({ "videoId": video_id });
        let _ = self.post("like/dislike", body).await?;
        Ok(())
    }

    pub async fn set_playlist_saved(&self, playlist_id: &str, save: bool) -> Result<()> {
        let mut body = Self::base_context();
        body["target"] = json!({ "playlistId": playlist_id });
        let endpoint = if save { "like/like" } else { "like/removelike" };
        let _ = self.post(endpoint, body).await?;
        Ok(())
    }

    pub async fn fetch_search_suggestions(&self, input: &str) -> Vec<String> {
        if input.trim().is_empty() {
            return Vec::new();
        }
        let mut body = Self::base_context();
        body["input"] = json!(input.trim());
        let Ok(data) = self.post("music/get_search_suggestions", body).await else {
            return Vec::new();
        };
        let mut out = Vec::new();
        collect_suggestion_runs(&data, &mut out);
        out.truncate(8);
        out
    }

    pub async fn search_rich(&self, query: &str) -> Result<RichSearchResult> {
        let mut body = Self::base_context();
        body["query"] = json!(query.trim());
        let data = self.post("search", body).await?;
        Ok(parse_rich_search_json(&data))
    }
}

fn collect_suggestion_runs(val: &Value, out: &mut Vec<String>) {
    match val {
        Value::Object(map) => {
            if let Some(renderer) = map.get("searchSuggestionRenderer") {
                if let Some(text) = collect_runs_text(renderer.get("suggestion")) {
                    if !text.is_empty() && !out.contains(&text) {
                        out.push(text);
                    }
                }
            }
            for v in map.values() {
                collect_suggestion_runs(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_suggestion_runs(v, out);
            }
        }
        _ => {}
    }
}

fn extract_mood_chips(val: &Value) -> Vec<MoodChip> {
    let mut chips = Vec::new();
    collect_chips_recursive(val, &mut chips);
    chips
}

fn collect_chips_recursive(val: &Value, out: &mut Vec<MoodChip>) {
    match val {
        Value::Object(map) => {
            if let Some(chip) = map.get("chipCloudChipRenderer") {
                let label = collect_runs_text(chip.get("text")).unwrap_or_default();
                let params = chip
                    .pointer("/navigationEndpoint/browseEndpoint/params")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !label.is_empty()
                    && !params.is_empty()
                    && !out.iter().any(|c| c.params == params)
                {
                    out.push(MoodChip { label, params });
                }
            }
            for v in map.values() {
                collect_chips_recursive(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_chips_recursive(v, out);
            }
        }
        _ => {}
    }
}

fn extract_mood_tiles(val: &Value) -> Vec<MoodCategoryTile> {
    let mut tiles = Vec::new();
    collect_tiles_recursive(val, &mut tiles);
    tiles
}

fn collect_tiles_recursive(val: &Value, out: &mut Vec<MoodCategoryTile>) {
    match val {
        Value::Object(map) => {
            if let Some(btn) = map.get("musicNavigationButtonRenderer") {
                let title = collect_runs_text(btn.get("buttonText")).unwrap_or_default();
                let browse_id = btn
                    .pointer("/clickCommand/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let params = btn
                    .pointer("/clickCommand/browseEndpoint/params")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let stripe = btn
                    .pointer("/solid/leftStripeColor")
                    .and_then(Value::as_u64)
                    .map(|c| {
                        let r = ((c >> 16) & 0xff) as u8;
                        let g = ((c >> 8) & 0xff) as u8;
                        let b = (c & 0xff) as u8;
                        (r, g, b)
                    })
                    .unwrap_or((120, 128, 150));
                if !title.is_empty()
                    && !browse_id.is_empty()
                    && !out.iter().any(|t| t.title == title && t.params == params)
                {
                    out.push(MoodCategoryTile {
                        title,
                        browse_id,
                        params,
                        accent_rgb: stripe,
                    });
                }
            }
            for v in map.values() {
                collect_tiles_recursive(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_tiles_recursive(v, out);
            }
        }
        _ => {}
    }
}

fn extract_artist_banner_url(val: &Value) -> Option<String> {
    let header = val
        .pointer("/header/musicVisualHeaderRenderer")
        .or_else(|| val.pointer("/header/musicImmersiveHeaderRenderer"))?;
    let thumbs = header
        .pointer("/foregroundThumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| header.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"))
        .and_then(Value::as_array)?;
    // Prefer wide banner aspect ratio (>= 1.5 width/height).
    let mut best_wide = None;
    let mut best_wide_w = 0u64;
    for t in thumbs {
        let w = t.get("width").and_then(Value::as_u64).unwrap_or(0);
        let h = t.get("height").and_then(Value::as_u64).unwrap_or(1);
        let url = t.get("url").and_then(Value::as_str).unwrap_or("");
        if w > h.saturating_mul(3) / 2 && w >= best_wide_w && !url.is_empty() {
            best_wide_w = w;
            best_wide = Some(upscale_wide_banner_url(url));
        }
    }
    best_wide
}

fn upscale_wide_banner_url(url: &str) -> String {
    if !(url.contains("googleusercontent.com") || url.contains("ggpht.com")) {
        return url.to_string();
    }
    if let Some((base, _params)) = url.split_once("=w") {
        format!("{base}=w1600-h666-p-l90-rj")
    } else {
        url.to_string()
    }
}

fn parse_album_detail_json(browse_id: &str, val: &Value) -> Option<AlbumDetail> {
    let header = val
        .pointer("/header/musicResponsiveHeaderRenderer")
        .or_else(|| val.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicResponsiveHeaderRenderer"))
        .or_else(|| val.pointer("/header/musicDetailHeaderRenderer"))?;

    let title = collect_runs_text(header.get("title")).unwrap_or_else(|| "Album".to_string());
    let subtitle = collect_runs_text(header.get("subtitle")).unwrap_or_default();
    let second_sub = collect_runs_text(header.get("secondSubtitle")).unwrap_or_default();

    let mut artists = Vec::new();
    if let Some(runs) = header
        .pointer("/straplineTextOne/runs")
        .and_then(Value::as_array)
    {
        for run in runs {
            let text = run.get("text").and_then(Value::as_str).unwrap_or("").trim();
            if text.is_empty() || text == "•" || text == "," || text == "&" {
                continue;
            }
            let bid = run
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
                .map(str::to_string);
            artists.push((text.to_string(), bid));
        }
    }
    if artists.is_empty() {
        if let Some(runs) = header.pointer("/subtitle/runs").and_then(Value::as_array) {
            for run in runs {
                if let Some(bid) = run
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                {
                    let name = run.get("text").and_then(Value::as_str).unwrap_or("").trim();
                    if !name.is_empty() {
                        artists.push((name.to_string(), Some(bid.to_string())));
                    }
                }
            }
        }
    }

    let sub_parts: Vec<&str> = subtitle
        .split('•')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let release_type = sub_parts.first().copied().unwrap_or("Album").to_string();
    let year = sub_parts
        .iter()
        .find(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_digit()))
        .map(|s| (*s).to_string());
    let duration_text = (!second_sub.is_empty()).then_some(second_sub);

    let cover_url = extract_best_thumbnail(
        header
            .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
            .or_else(|| {
                header.pointer("/thumbnail/croppedSquareThumbnailRenderer/thumbnail/thumbnails")
            }),
    );

    let playlist_id = find_first_string_at_pointer_name(val, "playlistId");

    let default_artist = if !artists.is_empty() {
        Some(
            artists
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
    } else {
        None
    };

    let mut tracks = Vec::new();
    collect_tracks_recursive(
        val,
        &title,
        default_artist.as_deref(),
        &cover_url,
        browse_id,
        &mut tracks,
    );

    let shelves = parse_home_shelves_json(val);
    let other_versions = shelves
        .into_iter()
        .find(|s| s.title.to_ascii_lowercase().contains("other version"))
        .map(|s| s.cards)
        .unwrap_or_default();

    Some(AlbumDetail {
        id: browse_id.to_string(),
        title,
        subtitle,
        release_type,
        year,
        duration_text,
        cover_url,
        artists,
        tracks,
        other_versions,
        playlist_id,
        saved: false,
    })
}

fn find_first_string_at_pointer_name(val: &Value, key: &str) -> Option<String> {
    match val {
        Value::Object(map) => {
            if let Some(s) = map.get(key).and_then(Value::as_str) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            for v in map.values() {
                if let Some(found) = find_first_string_at_pointer_name(v, key) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => arr
            .iter()
            .find_map(|v| find_first_string_at_pointer_name(v, key)),
        _ => None,
    }
}

fn parse_cards_tree(val: &Value, default_kind: HomeCardKind) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    collect_cards_recursive(val, default_kind, &mut cards);
    cards
}

fn collect_cards_recursive(val: &Value, default_kind: HomeCardKind, out: &mut Vec<HomeCard>) {
    match val {
        Value::Object(map) => {
            if let Some(two) = map.get("musicTwoRowItemRenderer") {
                if let Some(card) = parse_two_row_card(two, default_kind) {
                    if !out.iter().any(|c| c.id == card.id && c.title == card.title) {
                        out.push(card);
                    }
                }
            } else if let Some(resp) = map.get("musicResponsiveListItemRenderer") {
                if default_kind == HomeCardKind::Artist {
                    if let Some(card) = parse_responsive_card(resp, default_kind) {
                        if !out.iter().any(|c| c.id == card.id && c.title == card.title) {
                            out.push(card);
                        }
                    }
                }
            }
            for v in map.values() {
                collect_cards_recursive(v, default_kind, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_cards_recursive(v, default_kind, out);
            }
        }
        _ => {}
    }
}

fn parse_two_row_card(two: &Value, default_kind: HomeCardKind) -> Option<HomeCard> {
    let title = collect_runs_text(two.get("title"))?;
    let subtitle = collect_runs_text(two.get("subtitle")).unwrap_or_default();
    let id = two
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)?
        .to_string();
    let kind = classify_browse_id(&id, default_kind);
    let thumbnail_url = extract_best_thumbnail(
        two.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"),
    );
    Some(HomeCard {
        kind,
        id,
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

fn parse_responsive_card(resp: &Value, default_kind: HomeCardKind) -> Option<HomeCard> {
    let cols = resp.get("flexColumns").and_then(Value::as_array)?;
    let title = cols.first().and_then(|c| {
        collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
    })?;
    let subtitle = cols
        .get(1)
        .and_then(|c| {
            collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
        })
        .unwrap_or_default();
    let id = resp
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| {
            cols.first()
                .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        })
        .and_then(Value::as_str)?
        .to_string();
    let kind = classify_browse_id(&id, default_kind);
    let thumbnail_url = extract_best_thumbnail(
        resp.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
    );
    Some(HomeCard {
        kind,
        id,
        title,
        subtitle,
        thumbnail_url,
        track: None,
    })
}

fn classify_browse_id(id: &str, fallback: HomeCardKind) -> HomeCardKind {
    if id.starts_with("UC") {
        HomeCardKind::Artist
    } else if id.starts_with("MPRE") {
        HomeCardKind::Album
    } else if id.starts_with("VL") || id.starts_with("PL") || id.starts_with("OLAK") {
        HomeCardKind::Playlist
    } else {
        fallback
    }
}

fn parse_rich_search_json(val: &Value) -> RichSearchResult {
    let mut out = RichSearchResult::default();
    collect_rich_search_shelves(val, &mut out);
    out
}

fn collect_rich_search_shelves(val: &Value, out: &mut RichSearchResult) {
    match val {
        Value::Object(map) => {
            if let Some(shelf) = map.get("musicShelfRenderer") {
                let heading = collect_runs_text(shelf.get("title"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if let Some(contents) = shelf.get("contents").and_then(Value::as_array) {
                    for item in contents {
                        if let Some(resp) = item.get("musicResponsiveListItemRenderer") {
                            if heading.contains("video") {
                                if let Some(t) =
                                    parse_responsive_track(resp, None, None, None, None)
                                {
                                    out.videos.push(t);
                                }
                            } else if heading.contains("album") {
                                if let Some(c) = parse_responsive_card(resp, HomeCardKind::Album) {
                                    out.albums.push(c);
                                }
                            } else if heading.contains("artist") {
                                if let Some(c) = parse_responsive_card(resp, HomeCardKind::Artist) {
                                    out.artists.push(c);
                                }
                            } else if heading.contains("playlist") {
                                if let Some(c) = parse_responsive_card(resp, HomeCardKind::Playlist)
                                {
                                    out.playlists.push(c);
                                }
                            } else if let Some(t) =
                                parse_responsive_track(resp, None, None, None, None)
                            {
                                out.songs.push(t);
                            } else if let Some(c) = parse_responsive_card(resp, HomeCardKind::Album)
                            {
                                match c.kind {
                                    HomeCardKind::Artist => out.artists.push(c),
                                    HomeCardKind::Playlist => out.playlists.push(c),
                                    _ => out.albums.push(c),
                                }
                            }
                        }
                    }
                }
            }
            for v in map.values() {
                collect_rich_search_shelves(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_rich_search_shelves(v, out);
            }
        }
        _ => {}
    }
}

fn collect_tracks_recursive(
    val: &Value,
    default_album: &str,
    default_artist: Option<&str>,
    default_cover: &Option<String>,
    default_album_id: &str,
    out: &mut Vec<Track>,
) {
    match val {
        Value::Object(map) => {
            if let Some(resp) = map.get("musicResponsiveListItemRenderer") {
                if let Some(t) = parse_responsive_track(
                    resp,
                    Some(default_album),
                    default_artist,
                    default_cover.clone(),
                    Some(default_album_id),
                ) {
                    if !out.iter().any(|e| e.id == t.id) {
                        out.push(t);
                    }
                }
            }
            for v in map.values() {
                collect_tracks_recursive(
                    v,
                    default_album,
                    default_artist,
                    default_cover,
                    default_album_id,
                    out,
                );
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_tracks_recursive(
                    v,
                    default_album,
                    default_artist,
                    default_cover,
                    default_album_id,
                    out,
                );
            }
        }
        _ => {}
    }
}

fn is_metadata_noise_token(txt: &str) -> bool {
    let lower = txt.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    if matches!(
        lower.as_str(),
        "song" | "video" | "single" | "ep" | "album" | "playlist" | "explicit" | "artist"
    ) {
        return true;
    }
    if lower.contains(" play")
        || lower.ends_with("plays")
        || lower.contains(" view")
        || lower.ends_with("views")
        || lower.contains(" listener")
        || lower.contains(" subscriber")
        || lower.contains(" track")
        || lower.contains(" song")
    {
        return true;
    }
    if lower.len() == 4 && lower.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    false
}

fn parse_responsive_track(
    resp: &Value,
    default_album: Option<&str>,
    default_artist: Option<&str>,
    default_cover: Option<String>,
    default_album_id: Option<&str>,
) -> Option<Track> {
    let vid = resp
        .pointer("/playlistItemData/videoId")
        .or_else(|| {
            resp.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId")
        })
        .and_then(Value::as_str)?
        .to_string();

    let cols = resp.get("flexColumns").and_then(Value::as_array)?;
    let title = cols.first().and_then(|c| {
        collect_runs_text(c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
    })?;

    let mut artist_parts: Vec<String> = Vec::new();
    let mut album = default_album.unwrap_or("").to_string();
    let mut album_id = default_album_id.map(str::to_string);
    let mut duration_secs = None;

    for col in cols.iter().skip(1) {
        if let Some(runs) = col
            .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
            .and_then(Value::as_array)
        {
            for r in runs {
                let txt = r.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if txt.is_empty() || txt == "•" || txt == "," || txt == "&" {
                    continue;
                }
                if let Some(dur) = parse_duration_text(txt) {
                    duration_secs = Some(dur);
                    continue;
                }
                if let Some(bid) = r
                    .pointer("/navigationEndpoint/browseEndpoint/browseId")
                    .and_then(Value::as_str)
                {
                    if bid.starts_with("MPRE") {
                        album = txt.to_string();
                        album_id = Some(bid.to_string());
                    } else if !is_metadata_noise_token(txt)
                        && !artist_parts.iter().any(|a| a.eq_ignore_ascii_case(txt))
                    {
                        artist_parts.push(txt.to_string());
                    }
                } else if artist_parts.is_empty() && !is_metadata_noise_token(txt) {
                    artist_parts.push(txt.to_string());
                }
            }
        }
    }

    let artist = if !artist_parts.is_empty() {
        artist_parts.join(", ")
    } else if let Some(def) = default_artist.filter(|s| !s.trim().is_empty()) {
        def.to_string()
    } else {
        "Unknown Artist".to_string()
    };

    if let Some(fixed) = resp.get("fixedColumns").and_then(Value::as_array) {
        for fc in fixed {
            if let Some(txt) =
                collect_runs_text(fc.pointer("/musicResponsiveListItemFixedColumnRenderer/text"))
            {
                if let Some(dur) = parse_duration_text(&txt) {
                    duration_secs = Some(dur);
                }
            }
        }
    }

    let thumbnail_url = extract_best_thumbnail(
        resp.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
    )
    .or(default_cover);
    let is_explicit = resp
        .pointer("/badges/0/musicInlineBadgeRenderer/icon/iconType")
        .and_then(Value::as_str)
        .is_some_and(|t| t == "MUSIC_EXPLICIT_BADGE");

    Some(Track {
        id: vid,
        title,
        artist,
        album,
        duration_secs,
        thumbnail_url,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id,
        is_explicit,
    })
}

fn extract_best_thumbnail(arr: Option<&Value>) -> Option<String> {
    let arr = arr?.as_array()?;
    let url = arr
        .last()
        .and_then(|v| v.get("url"))
        .and_then(Value::as_str)?;
    Some(upscale_thumbnail(url))
}

fn collect_runs_text(val: Option<&Value>) -> Option<String> {
    let runs = val?.get("runs")?.as_array()?;
    let mut s = String::new();
    for r in runs {
        if let Some(t) = r.get("text").and_then(Value::as_str) {
            s.push_str(t);
        }
    }
    let trimmed = s.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}
