//! Account sign-in, library browsing, search, playlist mutations, and theme/thumbnail helpers.
//!
//! 1. **Auth & Account**: `start_sign_in`, `refresh_account`, and `sign_out` manage browser cookie extraction and YouTube Music profile state.
//! 2. **Feeds & Collections**: `load_home_feed`, `open_artist`, `open_collection`, and `execute_search` run stale-guarded background queries.
//! 3. **Library Mutations & UI Sync**: handles likes, playlist edits, thumbnail prefetching, and Hyprland opacity updates.

use super::types::{InternalEvent, NavTarget};
use super::BackendEngine;
use sonata_core::auth::ExtractedCookies;
use sonata_core::providers::ytmusic::{MoodChip, YtMusicClient};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_runtime::auth::extract_ytm_cookies_from_browser;
use sonata_runtime::hyprland;
use sonata_runtime::media;
use sonata_runtime::process::LinuxStreamResolver;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;

static REPAIRED_ART_PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

fn extract_primary_artist_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains(" play")
        || lower.ends_with("plays")
        || lower.contains(" view")
        || lower.ends_with("views")
        || lower.contains(" listener")
    {
        return String::new();
    }
    let first = trimmed
        .split(['•', ',', ';'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    let no_feat = first
        .split(" feat.")
        .next()
        .unwrap_or(first)
        .split(" ft.")
        .next()
        .unwrap_or(first)
        .split(" & ")
        .next()
        .unwrap_or(first)
        .trim();
    no_feat.to_string()
}

impl BackendEngine {
    pub(super) fn prefetch_thumbnails(&self, urls: Vec<String>) {
        let mut candidates = Vec::new();
        let mut existing_to_check = Vec::new();
        {
            let Ok(mut in_flight) = self.in_flight_thumbs.lock() else {
                return;
            };
            let checked = REPAIRED_ART_PATHS.get_or_init(|| Mutex::new(HashSet::new()));
            for url in urls {
                if in_flight.len() >= 128 {
                    break;
                }
                if url.is_empty() {
                    continue;
                }
                let path = media::art_cache_path(&url);
                let stale_small_banner = url.contains("=w1600")
                    && image::image_dimensions(&path).is_ok_and(|(w, _)| w <= 640);
                if path.exists() && !stale_small_banner {
                    if existing_to_check.len() < 48 {
                        if let Ok(mut guard) = checked.lock() {
                            if guard.insert(path.clone()) {
                                existing_to_check.push(path);
                            }
                        }
                    }
                    continue;
                }
                if in_flight.contains(&url) {
                    continue;
                }
                in_flight.insert(url.clone());
                candidates.push(url);
                if candidates.len() >= 96 {
                    break;
                }
            }
        }
        if !existing_to_check.is_empty() {
            let int_tx = self.int_tx.clone();
            tokio::spawn(async move {
                let mut repaired_any = false;
                for path in existing_to_check {
                    if media::repair_cached_art_if_bars(path).await {
                        repaired_any = true;
                    }
                }
                if repaired_any {
                    let _ = int_tx.send(InternalEvent::ThumbnailsReady);
                }
            });
        }
        if candidates.is_empty() {
            return;
        }

        let http = self.http.clone();
        let paths = self.paths.clone();
        let cache_limit = self.config.cache_limit_bytes();
        let int_tx = self.int_tx.clone();
        let in_flight = self.in_flight_thumbs.clone();
        let slots = self.thumbnail_slots.clone();
        tokio::spawn(async move {
            let (done_tx, mut done_rx) = mpsc::channel::<()>(8);
            for url in candidates {
                let slots = slots.clone();
                let http = http.clone();
                let paths = paths.clone();
                let in_flight = in_flight.clone();
                let done_tx = done_tx.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = slots.acquire().await else {
                        return;
                    };
                    let _ = media::fetch_and_decode_art(&http, &url, &paths, cache_limit).await;
                    if let Ok(mut guard) = in_flight.lock() {
                        guard.remove(&url);
                    }
                    let _ = done_tx.send(()).await;
                });
            }
            drop(done_tx);

            let mut count = 0usize;
            while done_rx.recv().await.is_some() {
                count += 1;
                if count.is_multiple_of(4) {
                    let _ = int_tx.send(InternalEvent::ThumbnailsReady);
                }
            }
            let _ = int_tx.send(InternalEvent::ThumbnailsReady);
        });
    }

    pub(super) fn start_sign_in(&mut self, browser_id: String) {
        self.signing_in = true;
        self.sign_in_error = None;
        self.status_line = format!("Signing in via {browser_id}...");

        let http = self.http.clone();
        let paths = self.paths.clone();
        let cache_limit = self.config.cache_limit_bytes();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let bid = browser_id.clone();
            let cookies =
                tokio::task::spawn_blocking(move || extract_ytm_cookies_from_browser(&bid))
                    .await
                    .unwrap_or_default();

            if !cookies.has_ytm_auth() {
                let label = cookies
                    .browser_name
                    .clone()
                    .unwrap_or_else(|| browser_id.clone());
                let _ = int_tx.send(InternalEvent::SignInFinished {
                    browser_id,
                    cookies,
                    profile: None,
                    avatar_path: None,
                    ytm_playlists: Vec::new(),
                    error: Some(format!(
                        "No active YouTube Music login found in {label}. Sign in on music.youtube.com in your browser and try again."
                    )),
                });
                return;
            }

            let ytm = YtMusicClient::new(http.clone(), cookies.clone());
            let (profile_res, playlists_res) =
                tokio::join!(ytm.fetch_account_profile(), ytm.fetch_library_playlists());

            let profile = profile_res.ok();
            let mut avatar_path = None;
            if let Some(url) = profile.as_ref().and_then(|p| p.avatar_url.as_ref()) {
                let _ = media::fetch_and_decode_art(&http, url, &paths, cache_limit).await;
                let path = media::art_cache_path_in(&paths, url);
                if path.exists() {
                    avatar_path = Some(path);
                }
            }

            let _ = int_tx.send(InternalEvent::SignInFinished {
                browser_id,
                cookies,
                profile,
                avatar_path,
                ytm_playlists: playlists_res.unwrap_or_default(),
                error: None,
            });
        });
    }

    pub(super) fn refresh_account(&mut self) {
        if !self.ytm.is_authenticated() {
            if let Some(browser_id) = self.signed_in_browser.clone() {
                self.start_sign_in(browser_id);
            }
            return;
        }
        let ytm = self.ytm.clone();
        let http = self.http.clone();
        let paths = self.paths.clone();
        let cache_limit = self.config.cache_limit_bytes();
        let int_tx = self.int_tx.clone();
        if let Some(db) = &self.cache_db {
            db.invalidate_playlist(None);
        }
        self.status_line = "Refreshing YouTube Music library...".to_string();

        tokio::spawn(async move {
            ytm.invalidate_cache(None).await;
            let (profile_res, playlists_res) =
                tokio::join!(ytm.fetch_account_profile(), ytm.fetch_library_playlists());
            let profile = profile_res.ok();
            let mut avatar_path = None;
            if let Some(url) = profile.as_ref().and_then(|p| p.avatar_url.as_ref()) {
                let _ = media::fetch_and_decode_art(&http, url, &paths, cache_limit).await;
                let path = media::art_cache_path_in(&paths, url);
                if path.exists() {
                    avatar_path = Some(path);
                }
            }
            let _ = int_tx.send(InternalEvent::AccountRefreshed {
                profile,
                avatar_path,
                ytm_playlists: playlists_res.unwrap_or_default(),
            });
        });
        self.load_home_feed();
        self.load_explore();
        self.load_radio_mixes();
        self.load_library_extras();
        self.load_listening_history();
    }

    pub(super) async fn sign_out(&mut self) {
        self.config.desktop_ytm_browser = None;
        let _ = self.config.save();
        self.cookies = ExtractedCookies::default();
        self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone());
        self.resolver = LinuxStreamResolver::with_profile(
            self.http.clone(),
            self.ytm.clone(),
            self.cookies.clone(),
            self.cache_db.clone(),
            super::linux_core_paths(),
            super::linux_resolver_profile(&self.config),
        );
        let _ = self
            .session
            .send(sonata_runtime::player::SessionCommand::UpdateStreamSource(
                std::sync::Arc::new(self.resolver.clone()),
            ))
            .await;
        self.signed_in_browser = None;
        self.user_profile = None;
        self.user_avatar_path = None;
        self.ytm_playlists.clear();
        self.library_albums.clear();
        self.library_artists.clear();
        self.history_tracks.clear();
        self.sign_in_error = None;
        if let Some(db) = &self.cache_db {
            db.put_library("ytm", &[]);
            db.invalidate_playlist(Some("YTM:VLLM"));
        }
        if matches!(self.active_nav, NavTarget::Playlist(_)) {
            self.active_nav = NavTarget::Home;
        }
        self.status_line = "Signed out of YouTube Music".to_string();
        self.last_played_tracks.clear();
        self.load_home_feed();
        self.load_explore();
        self.load_radio_mixes();
    }

    pub(super) fn load_radio_mixes(&mut self) {
        self.radio_req_id = self.radio_req_id.wrapping_add(1);
        let req_id = self.radio_req_id;
        self.radio_loading = true;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let shelves = ytm.fetch_radio_mixes().await.unwrap_or_default();
            let _ = int_tx.send(InternalEvent::RadioLoaded { req_id, shelves });
        });
    }

    pub(super) fn load_listening_history(&mut self) {
        if !self.ytm.is_authenticated() {
            return;
        }
        self.history_loading = true;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let tracks = ytm.fetch_listening_history().await.unwrap_or_default();
            let _ = int_tx.send(InternalEvent::HistoryLoaded(tracks));
        });
    }

    pub(super) fn load_explore(&mut self) {
        self.explore_req_id = self.explore_req_id.wrapping_add(1);
        let req_id = self.explore_req_id;
        self.explore_loading = true;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            if let Ok((shelves, mood_tiles)) = ytm.fetch_explore().await {
                let _ = int_tx.send(InternalEvent::ExploreLoaded {
                    req_id,
                    shelves,
                    mood_tiles,
                });
            } else {
                let _ = int_tx.send(InternalEvent::ExploreLoaded {
                    req_id,
                    shelves: Vec::new(),
                    mood_tiles: Vec::new(),
                });
            }
        });
    }

    pub(super) fn open_mood_category(&mut self, title: String, browse_id: String, params: String) {
        self.mood_req_id = self.mood_req_id.wrapping_add(1);
        let req_id = self.mood_req_id;
        self.active_nav = NavTarget::MoodCategory {
            title: title.clone(),
            browse_id: browse_id.clone(),
            params: params.clone(),
        };
        self.mood_page_title = title.clone();
        self.mood_page_shelves.clear();
        self.mood_page_loading = true;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let shelves = ytm
                .fetch_mood_category(&browse_id, &params)
                .await
                .unwrap_or_default();
            let _ = int_tx.send(InternalEvent::MoodCategoryLoaded {
                req_id,
                title,
                shelves,
            });
        });
    }

    pub(super) fn load_library_extras(&mut self) {
        if !self.ytm.is_authenticated() {
            return;
        }
        self.library_extras_loading = true;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let (albums, artists) = ytm.fetch_library_albums_and_artists().await;
            let _ = int_tx.send(InternalEvent::LibraryExtrasLoaded { albums, artists });
        });
    }

    pub(super) fn record_played_track(&mut self, track: Track) {
        self.last_played_tracks.retain(|t| t.id != track.id);
        self.last_played_tracks.insert(0, track);
        self.last_played_tracks.truncate(50);
    }

    pub(super) fn load_home_feed(&mut self) {
        self.load_home_with_chip(None);
    }

    pub(super) fn load_home_with_chip(&mut self, chip: Option<MoodChip>) {
        self.home_req_id = self.home_req_id.wrapping_add(1);
        let req_id = self.home_req_id;
        self.home_loading = true;
        self.home_error = None;
        self.active_home_chip = chip.clone();

        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            if let Some(ref c) = chip {
                match ytm.fetch_home_with_chips(Some(&c.params)).await {
                    Ok((chips, shelves)) => {
                        let _ = int_tx.send(InternalEvent::HomeLoaded {
                            req_id,
                            chips,
                            chip_selected: chip,
                            shelves,
                            error: None,
                        });
                    }
                    Err(e) => {
                        let _ = int_tx.send(InternalEvent::HomeLoaded {
                            req_id,
                            chips: Vec::new(),
                            chip_selected: chip,
                            shelves: Vec::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
            } else {
                let (chips_res, feed_res) =
                    tokio::join!(ytm.fetch_home_with_chips(None), ytm.fetch_home_feed());
                let chips = chips_res.map(|(c, _)| c).unwrap_or_default();
                match feed_res {
                    Ok(shelves) => {
                        let _ = int_tx.send(InternalEvent::HomeLoaded {
                            req_id,
                            chips,
                            chip_selected: None,
                            shelves,
                            error: None,
                        });
                    }
                    Err(e) => {
                        let _ = int_tx.send(InternalEvent::HomeLoaded {
                            req_id,
                            chips,
                            chip_selected: None,
                            shelves: Vec::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        });
    }

    pub(super) fn open_artist(&mut self, browse_id: String) {
        self.artist_req_id = self.artist_req_id.wrapping_add(1);
        let req_id = self.artist_req_id;
        self.active_nav = NavTarget::Artist(browse_id.clone());
        self.artist_loading = true;
        self.artist_error = None;
        self.artist_page = None;
        self.artist_banner_url = None;
        self.status_line = "Loading artist...".to_string();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let (artist_res, banner_url) = tokio::join!(
                ytm.fetch_artist_page(&browse_id),
                ytm.fetch_artist_banner(&browse_id),
            );
            match artist_res {
                Ok(artist) => {
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: Some(artist),
                        banner_url,
                        error: None,
                    });
                }
                Err(error) => {
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: None,
                        banner_url: None,
                        error: Some(error.to_string()),
                    });
                }
            }
        });
    }

    pub(super) fn open_artist_by_name(&mut self, name: String) {
        let raw = name.trim();
        if raw.is_empty() {
            return;
        }
        if raw.starts_with("UC") && !raw.contains(' ') {
            self.open_artist(raw.to_string());
            return;
        }
        // Extract primary artist name (avoid passing "Artist A, Artist B" or play-count noise).
        let clean = extract_primary_artist_name(raw);
        if clean.is_empty() || clean.eq_ignore_ascii_case("unknown artist") {
            self.status_line = "Artist information not available for this track".to_string();
            return;
        }
        self.artist_req_id = self.artist_req_id.wrapping_add(1);
        let req_id = self.artist_req_id;
        self.active_nav = NavTarget::Artist(format!("name:{clean}"));
        self.artist_loading = true;
        self.artist_error = None;
        self.artist_page = None;
        self.artist_banner_url = None;
        self.status_line = format!("Loading artist {clean}...");
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let mut resolved = ytm.fetch_artist_by_name(&clean).await.ok();
            // Secondary lookup via categorized artist search so we open the Artist page directly instead of Search.
            if resolved.is_none() {
                if let Ok(cat) = ytm.search_rich(&clean).await {
                    if let Some(first_artist) =
                        cat.artists.into_iter().find(|a| a.id.starts_with("UC"))
                    {
                        resolved = ytm.fetch_artist_page(&first_artist.id).await.ok();
                    }
                }
            }
            match resolved {
                Some(artist) => {
                    let banner_url = ytm.fetch_artist_banner(&artist.id).await;
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: Some(artist),
                        banner_url,
                        error: None,
                    });
                }
                None => {
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: None,
                        banner_url: None,
                        error: Some(format!("Could not find artist page for \"{clean}\"")),
                    });
                }
            }
        });
    }

    pub(super) fn open_album(
        &mut self,
        album_id: Option<String>,
        title: String,
        artist: String,
        thumbnail_url: Option<String>,
    ) {
        if let Some(id) = album_id
            .as_deref()
            .map(str::trim)
            .filter(|id| id.starts_with("MPRE"))
            .map(str::to_string)
        {
            self.album_req_id = self.album_req_id.wrapping_add(1);
            let req_id = self.album_req_id;
            self.active_nav = NavTarget::Album(id.clone());
            self.album_loading = true;
            self.album_error = None;
            self.album_detail = None;
            self.status_line = format!(
                "Loading {}...",
                if title.trim().is_empty() {
                    "album"
                } else {
                    title.trim()
                }
            );
            let ytm = self.ytm.clone();
            let int_tx = self.int_tx.clone();
            tokio::spawn(async move {
                match ytm.fetch_album_detail(&id).await {
                    Ok(album) => {
                        let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                            req_id,
                            album: Some(album),
                            error: None,
                        });
                    }
                    Err(e) => {
                        let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                            req_id,
                            album: None,
                            error: Some(e.to_string()),
                        });
                    }
                }
            });
            return;
        }

        if let Some(id) = album_id
            .map(|s| s.trim().to_string())
            .filter(|id| id.starts_with("OLAK") || id.starts_with("VL") || id.starts_with("PL"))
        {
            let heading = if title.trim().is_empty() {
                "Album".to_string()
            } else {
                title
            };
            self.open_collection(id, heading, artist, thumbnail_url, false, false);
            return;
        }

        let clean_title = title.trim().to_string();
        if clean_title.is_empty() {
            if !artist.trim().is_empty() {
                self.open_artist_by_name(artist);
            }
            return;
        }

        self.album_req_id = self.album_req_id.wrapping_add(1);
        let req_id = self.album_req_id;
        self.status_line = format!("Opening album {clean_title}...");
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match ytm.resolve_album_browse_id(&clean_title, &artist).await {
                Ok(Some(resolved_id)) if resolved_id.starts_with("MPRE") => {
                    match ytm.fetch_album_detail(&resolved_id).await {
                        Ok(album) => {
                            let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                                req_id,
                                album: Some(album),
                                error: None,
                            });
                        }
                        Err(e) => {
                            let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                                req_id,
                                album: None,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                _ => {
                    let query = if artist.trim().is_empty() {
                        clean_title
                    } else {
                        format!("{clean_title} {artist}")
                    };
                    let _ = int_tx.send(InternalEvent::ArtistResolveFallback(query));
                }
            }
        });
    }

    pub(super) fn start_track_radio(&mut self, seed: Track) {
        self.status_line = format!("Starting {} radio...", seed.artist);
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        let label = format!("{} Radio", seed.artist);
        tokio::spawn(async move {
            let mut radio = ytm.fetch_radio_queue(&seed.id).await.unwrap_or_default();
            if radio.is_empty() {
                let query = format!("{} {} mix", seed.artist, seed.title);
                radio = ytm.search_songs(&query).await.unwrap_or_default();
            }
            let mut combined = Vec::with_capacity(radio.len() + 1);
            let mut seen = std::collections::HashSet::new();
            seen.insert(seed.id.clone());
            combined.push(seed);
            for tr in radio {
                if seen.insert(tr.id.clone()) {
                    combined.push(tr);
                }
            }
            let _ = int_tx.send(InternalEvent::RadioQueueReady {
                tracks: combined,
                label,
            });
        });
    }

    pub(super) fn start_quick_radio(&mut self) {
        if let Some(cur) = self.playback.current_track().cloned() {
            self.start_track_radio(cur);
            return;
        }
        if let Some(recent) = self.last_played_tracks.first().cloned() {
            self.start_track_radio(recent);
            return;
        }
        if let Some(first_track) = self
            .home_shelves
            .iter()
            .flat_map(|s| s.tracks.iter())
            .next()
            .cloned()
        {
            self.start_track_radio(first_track);
            return;
        }
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = "Starting Quick Radio...".to_string();
        tokio::spawn(async move {
            let tracks = ytm
                .search_songs("top hits radio mix")
                .await
                .unwrap_or_default();
            let _ = int_tx.send(InternalEvent::RadioQueueReady {
                tracks,
                label: "Quick Radio".to_string(),
            });
        });
    }

    pub(super) fn open_collection(
        &mut self,
        id: String,
        title: String,
        subtitle: String,
        thumbnail_url: Option<String>,
        editable: bool,
        force_refresh: bool,
    ) {
        self.active_nav = NavTarget::Playlist(id.clone());
        self.playlist_id = Some(id.clone());
        self.playlist_heading = title.clone();
        self.playlist_subtitle = subtitle;
        self.playlist_cover_url = thumbnail_url;
        self.playlist_editable = editable;

        self.playlist_req_id = self.playlist_req_id.wrapping_add(1);
        let req_id = self.playlist_req_id;
        let cache_key = format!("YTM:{id}");

        let mut had_cached = false;
        if !force_refresh {
            if let Some((cached_name, cached_tracks, fresh)) = self
                .cache_db
                .as_ref()
                .and_then(|db| db.get_playlist(&cache_key))
            {
                had_cached = true;
                if !cached_name.is_empty() {
                    self.playlist_heading = cached_name;
                }
                let urls: Vec<String> = cached_tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                self.playlist_tracks = cached_tracks;
                self.playlist_loading = false;
                if fresh {
                    self.status_line = format!("Loaded {title} (cached)");
                    return;
                }
            }
        }

        if !had_cached {
            self.playlist_tracks.clear();
            self.playlist_loading = true;
        }

        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        let silent_replace = had_cached && !force_refresh;

        tokio::spawn(async move {
            if force_refresh {
                if let Some(db) = &cache_db {
                    db.invalidate_playlist(Some(&cache_key));
                }
                ytm.invalidate_cache(Some(&id)).await;
            }

            let mut all_tracks: Vec<Track> = Vec::new();
            let res = ytm
                .fetch_playlist_tracks_progressive(&id, |batch, is_first| {
                    if batch.is_empty() {
                        return;
                    }
                    all_tracks.extend(batch.iter().cloned());
                    if !silent_replace {
                        if is_first {
                            let _ = int_tx.send(InternalEvent::CollectionLoaded {
                                req_id,
                                id: id.clone(),
                                heading: title.clone(),
                                tracks: batch,
                                silent_replace: false,
                            });
                        } else {
                            let _ = int_tx.send(InternalEvent::CollectionAppended {
                                req_id,
                                tracks: batch,
                            });
                        }
                    }
                })
                .await;

            match res {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.put_playlist(&cache_key, &title, &all_tracks);
                    }
                    let _ = int_tx.send(InternalEvent::CollectionLoaded {
                        req_id,
                        id,
                        heading: title,
                        tracks: all_tracks,
                        silent_replace,
                    });
                }
                Err(e) if !had_cached => {
                    let _ = int_tx.send(InternalEvent::CollectionLoaded {
                        req_id,
                        id,
                        heading: title,
                        tracks: Vec::new(),
                        silent_replace: false,
                    });
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Failed to load tracks: {e}"
                    )));
                }
                Err(_) => {}
            }
        });
    }

    pub(super) fn fetch_search_suggestions(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() {
            self.search_suggestions.clear();
            return;
        }
        self.suggest_req_id = self.suggest_req_id.wrapping_add(1);
        let req_id = self.suggest_req_id;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let suggestions = ytm.fetch_search_suggestions(&q).await;
            let _ = int_tx.send(InternalEvent::SearchSuggestionsLoaded {
                req_id,
                suggestions,
            });
        });
    }

    pub(super) fn execute_search(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.active_nav = NavTarget::Search;
        self.search_query = q.clone();
        self.search_suggestions.clear();
        self.recent_searches
            .retain(|existing| !existing.eq_ignore_ascii_case(&q));
        self.recent_searches.insert(0, q.clone());
        self.recent_searches.truncate(10);
        self.search_results.clear();
        self.search_rich = Default::default();
        self.search_loading = true;
        self.search_req_id = self.search_req_id.wrapping_add(1);
        let req_id = self.search_req_id;
        let jiosaavn_enabled = self.config.jiosaavn_enabled;
        self.status_line = if jiosaavn_enabled {
            format!("Searching YouTube Music & JioSaavn for \"{q}\"...")
        } else {
            format!("Searching YouTube Music for \"{q}\"...")
        };

        let ytm = self.ytm.clone();
        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let jio_fut = async {
                if jiosaavn_enabled {
                    sonata_core::providers::jiosaavn::search_tracks(&http, &q, 10).await
                } else {
                    Vec::new()
                }
            };
            let (ytm_res, rich_res, jio_tracks) =
                tokio::join!(ytm.search_songs(&q), ytm.search_rich(&q), jio_fut);
            let mut rich = rich_res.unwrap_or_default();
            match ytm_res {
                Ok(mut tracks) => {
                    tracks.extend(jio_tracks);
                    if rich.songs.is_empty() {
                        rich.songs = tracks.clone();
                    }
                    let _ = int_tx.send(InternalEvent::SearchLoaded {
                        req_id,
                        tracks,
                        rich,
                        error: None,
                    });
                }
                Err(e) => {
                    if !jio_tracks.is_empty() {
                        if rich.songs.is_empty() {
                            rich.songs = jio_tracks.clone();
                        }
                        let _ = int_tx.send(InternalEvent::SearchLoaded {
                            req_id,
                            tracks: jio_tracks,
                            rich,
                            error: None,
                        });
                    } else {
                        let _ = int_tx.send(InternalEvent::SearchLoaded {
                            req_id,
                            tracks: Vec::new(),
                            rich,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        });
    }

    pub(super) fn dislike_track(&mut self, track: Track) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to rate songs".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        let id = track.id.clone();
        let title = track.title.clone();
        tokio::spawn(async move {
            match ytm.set_dislike_status(&id).await {
                Ok(()) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Marked \"{title}\" as disliked"
                    )));
                }
                Err(e) => {
                    let _ =
                        int_tx.send(InternalEvent::StatusMessage(format!("Dislike failed: {e}")));
                }
            }
        });
    }

    pub(super) fn toggle_save_album(&mut self) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to save albums".to_string();
            return;
        }
        let Some(ref mut album) = self.album_detail else {
            return;
        };
        let Some(pl_id) = album.playlist_id.clone() else {
            self.status_line = "This release cannot be saved directly".to_string();
            return;
        };
        let target_save = !album.saved;
        album.saved = target_save;
        let title = album.title.clone();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match ytm.set_playlist_saved(&pl_id, target_save).await {
                Ok(()) => {
                    let msg = if target_save {
                        format!("Saved \"{title}\" to Library")
                    } else {
                        format!("Removed \"{title}\" from Library")
                    };
                    let _ = int_tx.send(InternalEvent::StatusMessage(msg));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Library save failed: {e}"
                    )));
                }
            }
        });
    }

    pub(super) fn sync_hyprland_opacity(&mut self, opacity_override: Option<f32>) {
        if hyprland::is_hyprland() {
            if let Some(fresh) = hyprland::find_window_address_for_app("sonata-desktop") {
                self.hyprland_window = Some(fresh);
            }
        }
        let Some(addr) = self.hyprland_window.clone() else {
            return;
        };
        // Keep Hyprland compositor opacity at 1.0 when native Wayland blur/alpha paints the window directly,
        // or respect explicit Hyprland windowrule/opacity sync if requested.
        let opacity = opacity_override.unwrap_or(1.0).clamp(0.50, 1.00);
        tokio::spawn(async move {
            hyprland::set_window_opacity(&addr, opacity).await;
        });
    }

    pub(super) fn toggle_like_track(&mut self, track: Track) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to like songs".to_string();
            return;
        }
        let currently_liked = track.liked || self.liked_track_ids.contains(&track.id);
        let target_liked = !currently_liked;
        if target_liked {
            self.liked_track_ids.insert(track.id.clone());
        } else {
            self.liked_track_ids.remove(&track.id);
        }
        for list in [
            &mut self.playlist_tracks,
            &mut self.search_results,
            &mut self.playback.queue,
        ] {
            for t in list.iter_mut() {
                if t.id == track.id {
                    t.liked = target_liked;
                }
            }
        }
        if let Some(ref mut art) = self.artist_page {
            for t in &mut art.popular_tracks {
                if t.id == track.id {
                    t.liked = target_liked;
                }
            }
        }
        if let Some(ref mut alb) = self.album_detail {
            for t in &mut alb.tracks {
                if t.id == track.id {
                    t.liked = target_liked;
                }
            }
        }

        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        let cache_db = self.cache_db.clone();
        let title = track.title.clone();
        tokio::spawn(async move {
            let vid = if track.source == ProviderSource::YouTubeMusic && !track.is_jiosaavn() {
                Some(track.id)
            } else {
                let q = format!("{} {}", track.artist, track.title);
                ytm.search_songs(&q)
                    .await
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .map(|t| t.id)
            };
            let Some(vid) = vid else {
                let _ = int_tx.send(InternalEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.set_like_status(&vid, target_liked).await {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some("YTM:VLLM"));
                    }
                    let msg = if target_liked {
                        format!("Liked \"{title}\"")
                    } else {
                        format!("Removed like on \"{title}\"")
                    };
                    let _ = int_tx.send(InternalEvent::StatusMessage(msg));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Like update failed: {e}"
                    )));
                }
            }
        });
    }

    pub(super) fn create_ytm_playlist(&mut self, title: &str) {
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music first".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = format!("Creating playlist \"{trimmed}\"...");
        tokio::spawn(async move {
            match ytm.create_playlist(&trimmed).await {
                Ok(pid) => {
                    let pl = Playlist {
                        id: format!("VL{pid}"),
                        name: trimmed,
                        source: ProviderSource::YouTubeMusic,
                        track_count: Some(0),
                        thumbnail_url: None,
                    };
                    let _ = int_tx.send(InternalEvent::PlaylistCreated(pl));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Create playlist failed: {e}"
                    )));
                }
            }
        });
    }

    pub(super) fn edit_playlist(
        &mut self,
        playlist_id: String,
        title: Option<String>,
        cover_path: Option<PathBuf>,
    ) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music first".to_string();
            return;
        }
        let clean_title = title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if clean_title.is_none() && cover_path.is_none() {
            return;
        }
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = "Updating playlist on YouTube Music...".to_string();
        tokio::spawn(async move {
            if let Some(ref new_title) = clean_title {
                if let Err(e) = ytm
                    .edit_playlist_metadata(&playlist_id, Some(new_title), None, None)
                    .await
                {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Rename playlist failed: {e}"
                    )));
                    return;
                }
            }
            let mut cover_updated = false;
            if let Some(path) = cover_path {
                let mime = match path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .as_deref()
                {
                    Some("png") => "image/png",
                    Some("webp") => "image/webp",
                    _ => "image/jpeg",
                };
                match tokio::fs::read(&path).await {
                    Ok(bytes) => {
                        if let Err(e) = ytm.upload_playlist_cover(&playlist_id, &bytes, mime).await
                        {
                            let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                                "Cover upload failed: {e}"
                            )));
                            return;
                        }
                        cover_updated = true;
                    }
                    Err(e) => {
                        let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                            "Could not read cover image: {e}"
                        )));
                        return;
                    }
                }
            }
            let _ = int_tx.send(InternalEvent::PlaylistUpdated {
                playlist_id,
                title: clean_title,
                cover_updated,
            });
        });
    }

    pub(super) fn add_track_to_playlist(
        &mut self,
        track: Track,
        playlist_id: String,
        playlist_name: String,
    ) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to edit playlists".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = format!("Adding \"{}\" to {}...", track.title, playlist_name);
        tokio::spawn(async move {
            let vid = if track.source == ProviderSource::YouTubeMusic && !track.is_jiosaavn() {
                Some(track.id)
            } else {
                let q = format!("{} {}", track.artist, track.title);
                ytm.search_songs(&q)
                    .await
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .map(|t| t.id)
            };
            let Some(vid) = vid else {
                let _ = int_tx.send(InternalEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.add_to_playlist(&playlist_id, &vid).await {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some(&format!("YTM:{playlist_id}")));
                    }
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Added \"{}\" to {playlist_name}",
                        track.title
                    )));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!("Add failed: {e}")));
                }
            }
        });
    }

    pub(super) fn remove_track_from_playlist(&mut self, idx: usize) {
        let Some(pl_id) = self.playlist_id.clone() else {
            return;
        };
        let Some(track) = self.playlist_tracks.get(idx).cloned() else {
            return;
        };
        if pl_id == "VLLM" {
            self.toggle_like_track(track);
            return;
        }
        let Some(set_vid) = track.set_video_id.clone() else {
            self.status_line = "Track missing playlist item ID; refresh playlist first".to_string();
            return;
        };
        self.playlist_tracks.remove(idx);

        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = format!("Removing \"{}\" from playlist...", track.title);
        tokio::spawn(async move {
            match ytm.remove_from_playlist(&pl_id, &track.id, &set_vid).await {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some(&format!("YTM:{pl_id}")));
                    }
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Removed \"{}\" from playlist",
                        track.title
                    )));
                }
                Err(e) => {
                    let _ =
                        int_tx.send(InternalEvent::StatusMessage(format!("Remove failed: {e}")));
                }
            }
        });
    }
}
