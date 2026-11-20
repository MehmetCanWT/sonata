//! Account sign-in, library browsing, search, playlist mutations, and theme/thumbnail helpers.
//!
//! 1. **Auth & Account**: `start_sign_in`, `refresh_account`, and `sign_out` manage browser cookie extraction and YouTube Music profile state.
//! 2. **Feeds & Collections**: `load_home_feed`, `open_artist`, `open_collection`, and `execute_search` run stale-guarded background queries.
//! 3. **Library Mutations & UI Sync**: handles likes, playlist edits, thumbnail prefetching, themes, and Hyprland opacity updates.

use super::types::{InternalEvent, NavTarget};
use super::BackendEngine;
use sonata_core::auth::{extract_ytm_cookies_from_browser, ExtractedCookies};
use sonata_core::hyprland;
use sonata_core::media;
use sonata_core::providers::hifi::StreamResolver;
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{Playlist, ProviderSource, Track};
use tokio::sync::mpsc;

impl BackendEngine {
    pub(super) fn prefetch_thumbnails(&self, urls: Vec<String>) {
        let mut candidates = Vec::new();
        {
            let Ok(mut in_flight) = self.in_flight_thumbs.lock() else {
                return;
            };
            for url in urls {
                if in_flight.len() >= 96 {
                    break;
                }
                if url.is_empty() {
                    continue;
                }
                let path = media::art_cache_path(&url);
                if path.exists() || in_flight.contains(&url) {
                    continue;
                }
                in_flight.insert(url.clone());
                candidates.push(url);
                if candidates.len() >= 48 {
                    break;
                }
            }
        }
        if candidates.is_empty() {
            return;
        }

        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        let in_flight = self.in_flight_thumbs.clone();
        let slots = self.thumbnail_slots.clone();
        tokio::spawn(async move {
            let (done_tx, mut done_rx) = mpsc::channel::<()>(8);
            for url in candidates {
                let slots = slots.clone();
                let http = http.clone();
                let in_flight = in_flight.clone();
                let done_tx = done_tx.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = slots.acquire().await else {
                        return;
                    };
                    let _ = media::fetch_and_decode_art(&http, &url).await;
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
                let _ = media::fetch_and_decode_art(&http, url).await;
                let path = media::art_cache_path(url);
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
                let _ = media::fetch_and_decode_art(&http, url).await;
                let path = media::art_cache_path(url);
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
    }

    pub(super) fn sign_out(&mut self) {
        self.config.desktop_ytm_browser = None;
        let _ = self.config.save();
        self.cookies = ExtractedCookies::default();
        self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone());
        self.resolver = StreamResolver::new(
            self.http.clone(),
            self.ytm.clone(),
            self.cookies.clone(),
            self.cache_db.clone(),
        );
        self.signed_in_browser = None;
        self.user_profile = None;
        self.user_avatar_path = None;
        self.ytm_playlists.clear();
        self.sign_in_error = None;
        if let Some(db) = &self.cache_db {
            db.put_library("ytm", &[]);
            db.invalidate_playlist(Some("YTM:VLLM"));
        }
        if matches!(self.active_nav, NavTarget::Playlist(_)) {
            self.active_nav = NavTarget::Home;
        }
        self.status_line = "Signed out of YouTube Music".to_string();
        self.load_home_feed();
    }

    pub(super) fn load_home_feed(&mut self) {
        self.home_req_id = self.home_req_id.wrapping_add(1);
        let req_id = self.home_req_id;
        self.home_loading = true;
        self.home_error = None;

        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match ytm.fetch_home_feed().await {
                Ok(shelves) => {
                    let _ = int_tx.send(InternalEvent::HomeLoaded {
                        req_id,
                        shelves,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::HomeLoaded {
                        req_id,
                        shelves: Vec::new(),
                        error: Some(e.to_string()),
                    });
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
        self.status_line = "Loading artist...".to_string();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match ytm.fetch_artist_page(&browse_id).await {
                Ok(artist) => {
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: Some(artist),
                        error: None,
                    });
                }
                Err(error) => {
                    let _ = int_tx.send(InternalEvent::ArtistLoaded {
                        req_id,
                        artist: None,
                        error: Some(error.to_string()),
                    });
                }
            }
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

    pub(super) fn execute_search(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.active_nav = NavTarget::Search;
        self.search_query = q.clone();
        self.search_results.clear();
        self.search_loading = true;
        self.search_req_id = self.search_req_id.wrapping_add(1);
        let req_id = self.search_req_id;
        self.status_line = format!("Searching YouTube Music for \"{q}\"...");

        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match ytm.search_songs(&q).await {
                Ok(tracks) => {
                    let _ = int_tx.send(InternalEvent::SearchLoaded {
                        req_id,
                        tracks,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::SearchLoaded {
                        req_id,
                        tracks: Vec::new(),
                        error: Some(e.to_string()),
                    });
                }
            }
        });
    }

    pub(super) fn apply_theme_by_name(&mut self, name: &str) {
        if let Some(t) = self
            .themes
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .cloned()
        {
            self.base_theme = t;
            self.config.theme = self.base_theme.name.clone();
            let _ = self.config.save();
            self.sync_hyprland_opacity();
            self.status_line = format!("Theme: {}", self.base_theme.name);
        }
    }

    pub(super) fn sync_hyprland_opacity(&mut self) {
        if hyprland::is_hyprland() {
            if let Some(fresh) = hyprland::find_window_address_for_app("sonata-desktop") {
                self.hyprland_window = Some(fresh);
            }
        }
        let Some(addr) = self.hyprland_window.clone() else {
            return;
        };
        let transparent = self
            .base_theme
            .transparent_bg
            .unwrap_or(self.config.transparent_bg);
        let opacity = if transparent {
            self.base_theme
                .hyprland_opacity
                .unwrap_or(self.config.hyprland_opacity)
                .clamp(0.50, 1.00)
        } else {
            1.0
        };
        tokio::spawn(async move {
            hyprland::set_window_opacity(&addr, opacity).await;
        });
    }

    pub(super) fn apply_config_volume(&self) {
        let eff = if self.muted { 0 } else { self.config.volume };
        if let Some(mpv) = &self.mpv {
            mpv.set_volume(eff);
        }
    }

    pub(super) fn toggle_like_track(&mut self, track: Track) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to like songs".to_string();
            return;
        }
        let target_liked = !track.liked;
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

        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        let cache_db = self.cache_db.clone();
        let title = track.title.clone();
        tokio::spawn(async move {
            match ytm.set_like_status(&track.id, target_liked).await {
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
            match ytm.add_to_playlist(&playlist_id, &track.id).await {
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
