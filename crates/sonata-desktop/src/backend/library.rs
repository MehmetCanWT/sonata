//! Account sign-in, library browsing, search, playlist mutations, and theme/thumbnail helpers.
//!
//! 1. **Auth & Account**: `start_sign_in`, `refresh_account`, and `sign_out` manage browser cookie extraction and YouTube Music profile state.
//! 2. **Feeds & Collections**: `load_home_feed`, `open_artist`, `open_collection`, and `execute_search` run stale-guarded background queries.
//! 3. **Library Mutations & UI Sync**: handles likes, playlist edits, thumbnail prefetching, and Hyprland opacity updates.

use super::types::{InternalEvent, NavTarget};
use super::BackendEngine;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sonata_core::auth::ExtractedCookies;
use sonata_core::playlists::{
    is_personal_track_id, PersonalPlaylistStore, PERSONAL_PLAYLIST_ID_PREFIX,
};
use sonata_core::providers::hifi::extract_badge_kbps;
use sonata_core::providers::local::scan_directories;
use sonata_core::providers::servers::{
    jellyfin, parse_server_item_id, save_server_profiles, webdav, ServerClient, ServerKind,
    ServerProfile,
};
use sonata_core::providers::ytmusic::{ArtistPage, HomeShelf, MoodChip, YtMusicClient};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_runtime::auth::extract_ytm_cookies_from_browser;
use sonata_runtime::config::AppConfig;
use sonata_runtime::downloads::{DownloadCollectionRef, DownloadStore, DownloadTrackInput};
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
        self.prefetch_thumbnails_with_priority(urls, false);
    }

    pub(super) fn prefetch_thumbnails_urgent(&self, urls: Vec<String>) {
        self.prefetch_thumbnails_with_priority(urls, true);
    }

    fn prefetch_thumbnails_with_priority(&self, urls: Vec<String>, urgent: bool) {
        let mut candidates = Vec::new();
        let mut existing_to_check = Vec::new();
        let in_flight_cap = if urgent { 256 } else { 192 };
        let candidate_cap = if urgent { 160 } else { 96 };
        {
            let Ok(mut in_flight) = self.in_flight_thumbs.lock() else {
                return;
            };
            let checked = REPAIRED_ART_PATHS.get_or_init(|| Mutex::new(HashSet::new()));
            let mut seen_urls = HashSet::with_capacity(urls.len());
            for url in urls {
                if in_flight.len() >= in_flight_cap {
                    break;
                }
                if url.is_empty() || !seen_urls.insert(url.clone()) {
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
                if candidates.len() >= candidate_cap {
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
        let slots = if urgent {
            self.urgent_thumbnail_slots.clone()
        } else {
            self.thumbnail_slots.clone()
        };
        tokio::spawn(async move {
            let (done_tx, mut done_rx) = mpsc::channel::<()>(16);
            for url in candidates {
                let slots = slots.clone();
                let http = http.clone();
                let paths = paths.clone();
                let in_flight = in_flight.clone();
                let done_tx = done_tx.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = slots.acquire().await else {
                        if let Ok(mut guard) = in_flight.lock() {
                            guard.remove(&url);
                        }
                        return;
                    };
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(12),
                        media::fetch_and_decode_art(&http, &url, &paths, cache_limit),
                    )
                    .await;
                    if let Ok(mut guard) = in_flight.lock() {
                        guard.remove(&url);
                    }
                    let _ = done_tx.send(()).await;
                });
            }
            drop(done_tx);

            let notify_step = if urgent { 2 } else { 4 };
            let mut count = 0usize;
            while done_rx.recv().await.is_some() {
                count += 1;
                if count == 1 || count.is_multiple_of(notify_step) {
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
        let lang_code = self.ytm_language.code();
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

            let ytm = YtMusicClient::new(http.clone(), cookies.clone()).with_language(lang_code);
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
        self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone())
            .with_language(self.ytm_language.code());
        self.resolver = LinuxStreamResolver::with_profile(
            self.http.clone(),
            self.ytm.clone(),
            self.cookies.clone(),
            self.cache_db.clone(),
            super::linux_core_paths(),
            super::linux_resolver_profile(&self.config),
        );
        if let Some(ref store) = self.download_store {
            self.resolver.set_downloads_store(Some(store.clone()));
        }
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
        if self.config.personal_only_mode {
            self.radio_loading = false;
            return;
        }
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
        if self.config.personal_only_mode || !self.ytm.is_authenticated() {
            self.history_loading = false;
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
        if self.config.personal_only_mode {
            self.explore_loading = false;
            return;
        }
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
        if self.config.personal_only_mode {
            self.status_line = "Personal Only Mode is active".to_string();
            return;
        }
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
        self.status_line = format!("Loading {title}...");
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let mut shelves = if browse_id.is_empty() {
                Vec::new()
            } else {
                ytm.fetch_mood_category(&browse_id, &params)
                    .await
                    .unwrap_or_default()
            };
            if shelves.is_empty() && !params.is_empty() {
                if let Ok((_, chip_shelves)) = ytm.fetch_home_with_chips(Some(&params)).await {
                    shelves = chip_shelves;
                }
            }
            if shelves.is_empty() && !title.trim().is_empty() {
                if let Ok(rich) = ytm.search_rich(&title).await {
                    if !rich.playlists.is_empty() {
                        shelves.push(HomeShelf {
                            title: "Playlists".to_string(),
                            tracks: Vec::new(),
                            cards: rich.playlists,
                        });
                    }
                    if !rich.songs.is_empty() {
                        shelves.push(HomeShelf {
                            title: "Songs".to_string(),
                            tracks: rich.songs,
                            cards: Vec::new(),
                        });
                    }
                }
            }
            let _ = int_tx.send(InternalEvent::MoodCategoryLoaded {
                req_id,
                title,
                shelves,
            });
        });
    }

    pub(super) fn open_mood_by_title(&mut self, title: String) {
        let needle = title.trim().to_lowercase();
        if let Some(tile) = self
            .mood_tiles
            .iter()
            .find(|t| {
                t.title.trim().to_lowercase() == needle
                    || sonata_runtime::i18n::localize_mood_title(self.ytm_language, &t.title)
                        .trim()
                        .to_lowercase()
                        == needle
            })
            .cloned()
        {
            let display = sonata_runtime::i18n::localize_mood_title(self.ytm_language, &tile.title)
                .into_owned();
            self.open_mood_category(display, tile.browse_id, tile.params);
            return;
        }
        if let Some(chip) = self
            .home_chips
            .iter()
            .find(|c| {
                c.label.trim().to_lowercase() == needle
                    || sonata_runtime::i18n::localize_mood_title(self.ytm_language, &c.label)
                        .trim()
                        .to_lowercase()
                        == needle
            })
            .cloned()
        {
            self.active_nav = NavTarget::Home;
            self.load_home_with_chip(Some(chip));
            return;
        }
        self.open_mood_category(title, String::new(), String::new());
    }

    pub(super) fn load_library_extras(&mut self) {
        if self.config.personal_only_mode || !self.ytm.is_authenticated() {
            self.library_extras_loading = false;
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
        if self.config.personal_only_mode {
            self.home_loading = false;
            self.home_error = None;
            return;
        }
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

        if let Some(item_ref) = parse_server_item_id(&browse_id) {
            if let Some(profile) = self
                .server_profiles
                .iter()
                .find(|p| p.id == item_ref.server_id)
                .cloned()
            {
                let client = self.server_client.clone();
                let int_tx = self.int_tx.clone();
                let fallback_name = self
                    .server_artists
                    .iter()
                    .find(|a| a.id == browse_id)
                    .map(|a| (a.title.clone(), a.thumbnail_url.clone()));
                tokio::spawn(async move {
                    match client
                        .list_albums(&profile, None, Some(&item_ref.item_id))
                        .await
                    {
                        Ok(albums) => {
                            let (name, thumb) = fallback_name.unwrap_or_else(|| {
                                (
                                    albums
                                        .first()
                                        .map(|a| a.subtitle.clone())
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or_else(|| profile.name.clone()),
                                    albums.first().and_then(|a| a.thumbnail_url.clone()),
                                )
                            });
                            let page = ArtistPage {
                                id: browse_id,
                                name,
                                subtitle: profile.kind.display_name().to_string(),
                                description: None,
                                subscribers: None,
                                monthly_listeners: None,
                                thumbnail_url: thumb,
                                shuffle_playlist_id: None,
                                radio_playlist_id: None,
                                latest_release: albums.first().cloned(),
                                popular_tracks: Vec::new(),
                                releases: albums,
                                related_artists: Vec::new(),
                            };
                            let _ = int_tx.send(InternalEvent::ArtistLoaded {
                                req_id,
                                artist: Some(page),
                                banner_url: None,
                                error: None,
                            });
                        }
                        Err(err) => {
                            let _ = int_tx.send(InternalEvent::ArtistLoaded {
                                req_id,
                                artist: None,
                                banner_url: None,
                                error: Some(err.to_string()),
                            });
                        }
                    }
                });
                return;
            }
        }

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
            .filter(|id| id.starts_with("local-album:"))
        {
            if let Some(local_alb) = self.local_scan.albums.iter().find(|a| a.id == id) {
                self.album_req_id = self.album_req_id.wrapping_add(1);
                self.album_loading = false;
                self.album_error = None;
                self.album_detail = Some(local_alb.to_album_detail());
                self.active_nav = NavTarget::Album(local_alb.id.clone());
                self.status_line = format!("Loaded {}", local_alb.name);
                return;
            }
        }
        if let Some(id) = album_id
            .as_deref()
            .map(str::trim)
            .filter(|id| parse_server_item_id(id).is_some())
            .map(str::to_string)
        {
            if let Some(item_ref) = parse_server_item_id(&id) {
                if let Some(profile) = self
                    .server_profiles
                    .iter()
                    .find(|p| p.id == item_ref.server_id)
                    .cloned()
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
                    let client = self.server_client.clone();
                    let int_tx = self.int_tx.clone();
                    tokio::spawn(async move {
                        match client.get_album(&profile, &id).await {
                            Ok(album) => {
                                let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                                    req_id,
                                    album: Some(album),
                                    error: None,
                                });
                            }
                            Err(err) => {
                                let _ = int_tx.send(InternalEvent::AlbumDetailLoaded {
                                    req_id,
                                    album: None,
                                    error: Some(err.to_string()),
                                });
                            }
                        }
                    });
                    return;
                }
            }
        }

        if let Some(id) = album_id
            .as_deref()
            .map(str::trim)
            .filter(|id| id.starts_with("MPRE"))
            .map(str::to_string)
        {
            if self.config.personal_only_mode {
                self.status_line = "Personal Only Mode is active".to_string();
                return;
            }
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

        if let Some(local_alb) = self.local_scan.albums.iter().find(|a| {
            a.name.eq_ignore_ascii_case(&clean_title)
                && (artist.trim().is_empty() || a.artist.eq_ignore_ascii_case(artist.trim()))
        }) {
            self.album_req_id = self.album_req_id.wrapping_add(1);
            self.album_loading = false;
            self.album_error = None;
            self.album_detail = Some(local_alb.to_album_detail());
            self.active_nav = NavTarget::Album(local_alb.id.clone());
            self.status_line = format!("Loaded {}", local_alb.name);
            return;
        }
        if self.config.personal_only_mode {
            self.status_line = "Personal Only Mode is active".to_string();
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
        if self.config.personal_only_mode {
            self.status_line =
                "Personal Only Mode is active — streaming radio is disabled".to_string();
            return;
        }
        self.status_line = format!("Starting {} radio...", seed.artist);
        self.active_queue_playlist_id = None;
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        let label = format!("{} Radio", seed.artist);
        let excludes: Vec<String> = self
            .last_played_tracks
            .iter()
            .take(25)
            .map(|t| t.id.clone())
            .collect();
        let mut preferred = vec![seed.artist.clone()];
        for artist in self.listener_preferred_artists() {
            if !preferred.iter().any(|a| a.eq_ignore_ascii_case(&artist)) {
                preferred.push(artist);
            }
            if preferred.len() >= 10 {
                break;
            }
        }
        tokio::spawn(async move {
            let mut radio = ytm
                .fetch_endless_radio_queue(
                    std::slice::from_ref(&seed.id),
                    None,
                    &excludes,
                    &preferred,
                )
                .await
                .unwrap_or_default();
            if radio.is_empty() {
                radio = ytm.fetch_radio_queue(&seed.id).await.unwrap_or_default();
            }
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
        if self.config.personal_only_mode {
            self.status_line =
                "Personal Only Mode is active — streaming radio is disabled".to_string();
            return;
        }
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

        if id == "SONATA:DOWNLOADS" {
            self.playlist_tracks = self
                .downloaded_tracks
                .iter()
                .map(|d| d.to_track())
                .collect();
            self.playlist_loading = false;
            self.refresh_downloads();
            return;
        }

        if id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX) {
            if let Some(pl) = self.personal_playlists.iter().find(|p| p.id == id) {
                self.playlist_heading = pl.name.clone();
                self.playlist_cover_url = pl.tracks.iter().find_map(|t| t.thumbnail_url.clone());
                self.playlist_editable = true;
                self.playlist_tracks = pl.tracks.clone();
                self.playlist_loading = false;
            } else {
                self.playlist_tracks.clear();
                self.playlist_loading = false;
            }
            return;
        }

        if let Some(item_ref) = parse_server_item_id(&id) {
            if let Some(profile) = self
                .server_profiles
                .iter()
                .find(|p| p.id == item_ref.server_id)
                .cloned()
            {
                self.playlist_tracks.clear();
                self.playlist_loading = true;
                let client = self.server_client.clone();
                let int_tx = self.int_tx.clone();
                tokio::spawn(async move {
                    match client.get_playlist_tracks(&profile, &id).await {
                        Ok(tracks) => {
                            let _ = int_tx.send(InternalEvent::CollectionLoaded {
                                req_id,
                                id,
                                heading: title,
                                tracks,
                                silent_replace: false,
                            });
                        }
                        Err(err) => {
                            let _ = int_tx.send(InternalEvent::CollectionLoaded {
                                req_id,
                                id,
                                heading: title,
                                tracks: Vec::new(),
                                silent_replace: false,
                            });
                            let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                                "Failed to load server tracks: {err}"
                            )));
                        }
                    }
                });
                return;
            }
        }

        if self.config.personal_only_mode {
            self.playlist_tracks.clear();
            self.playlist_loading = false;
            self.status_line = "Personal Only Mode is active".to_string();
            return;
        }

        let cache_key = format!("YTM:{id}");

        let mut had_cached = false;
        if !force_refresh {
            if let Some((cached_name, cached_tracks, _fresh)) = self
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
            }
            // Always bypass the 5-minute in-memory playlist cache so creator edits appear live.
            ytm.invalidate_cache(Some(&id)).await;

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
        if q.is_empty() || self.config.personal_only_mode {
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
        let personal_only = self.config.personal_only_mode;
        let jiosaavn_enabled = self.config.effective_jiosaavn_enabled();
        self.status_line = if personal_only {
            format!("Searching local & personal servers for \"{q}\"...")
        } else if jiosaavn_enabled {
            format!("Searching YouTube Music & JioSaavn for \"{q}\"...")
        } else {
            format!("Searching YouTube Music for \"{q}\"...")
        };

        let ytm = self.ytm.clone();
        let http = self.http.clone();
        let server_client = self.server_client.clone();
        let needle = q.to_ascii_lowercase();
        let local_matches: Vec<Track> = if self.config.local_music_enabled {
            self.local_scan
                .tracks
                .iter()
                .filter(|t| {
                    t.title.to_ascii_lowercase().contains(&needle)
                        || t.artist.to_ascii_lowercase().contains(&needle)
                        || t.album.to_ascii_lowercase().contains(&needle)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let enabled_servers: Vec<ServerProfile> = self
            .server_profiles
            .iter()
            .filter(|p| p.enabled)
            .cloned()
            .collect();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let servers_fut = async {
                let mut set = tokio::task::JoinSet::new();
                for profile in &enabled_servers {
                    let client = server_client.clone();
                    let profile = profile.clone();
                    let q = q.clone();
                    set.spawn(async move { client.search(&profile, &q, 20).await.ok() });
                }
                let mut out = Vec::new();
                while let Some(res) = set.join_next().await {
                    if let Ok(item) = res {
                        out.push(item);
                    }
                }
                out
            };

            if personal_only {
                let server_results = servers_fut.await;
                let mut tracks = local_matches;
                let mut rich = super::RichSearchResult::default();
                for sr in server_results.into_iter().flatten() {
                    tracks.extend(sr.tracks);
                    rich.albums.extend(sr.albums);
                    rich.artists.extend(sr.artists);
                    rich.playlists.extend(sr.playlists);
                }
                rich.songs = tracks.clone();
                let _ = int_tx.send(InternalEvent::SearchLoaded {
                    req_id,
                    tracks,
                    rich,
                    error: None,
                });
                return;
            }

            let jio_fut = async {
                if jiosaavn_enabled {
                    sonata_core::providers::jiosaavn::search_tracks(&http, &q, 10).await
                } else {
                    Vec::new()
                }
            };
            let (ytm_res, rich_res, jio_tracks, server_results) = tokio::join!(
                ytm.search_songs(&q),
                ytm.search_rich(&q),
                jio_fut,
                servers_fut
            );
            let mut rich = rich_res.unwrap_or_default();
            let mut server_tracks = local_matches;
            for sr in server_results.into_iter().flatten() {
                server_tracks.extend(sr.tracks);
                rich.albums.extend(sr.albums);
                rich.artists.extend(sr.artists);
                rich.playlists.extend(sr.playlists);
            }
            match ytm_res {
                Ok(mut tracks) => {
                    tracks.extend(jio_tracks);
                    tracks.extend(server_tracks);
                    if rich.songs.is_empty() {
                        rich.songs = tracks.clone();
                    } else {
                        for st in &tracks {
                            if (st.is_jiosaavn() || st.is_self_hosted() || st.is_local())
                                && !rich.songs.iter().any(|existing| existing.id == st.id)
                            {
                                rich.songs.push(st.clone());
                            }
                        }
                    }
                    let _ = int_tx.send(InternalEvent::SearchLoaded {
                        req_id,
                        tracks,
                        rich,
                        error: None,
                    });
                }
                Err(e) => {
                    let mut fallback_tracks = jio_tracks;
                    fallback_tracks.extend(server_tracks);
                    if !fallback_tracks.is_empty() {
                        if rich.songs.is_empty() {
                            rich.songs = fallback_tracks.clone();
                        }
                        let _ = int_tx.send(InternalEvent::SearchLoaded {
                            req_id,
                            tracks: fallback_tracks,
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

    pub(super) fn toggle_save_playlist(
        &mut self,
        id: String,
        title: String,
        thumbnail_url: Option<String>,
        save: bool,
    ) {
        let norm_target = crate::settings::normalize_collection_id(&id);
        if save {
            if !self
                .ytm_playlists
                .iter()
                .any(|p| crate::settings::normalize_collection_id(&p.id) == norm_target)
            {
                self.ytm_playlists.push(Playlist {
                    id: id.clone(),
                    name: title.clone(),
                    source: ProviderSource::YouTubeMusic,
                    track_count: None,
                    thumbnail_url,
                    is_owned: false,
                });
            }
            self.status_line = format!("Saved \"{title}\" to Library");
        } else {
            self.ytm_playlists.retain(|p| {
                p.is_owned || crate::settings::normalize_collection_id(&p.id) != norm_target
            });
            self.status_line = format!("Removed \"{title}\" from Library");
        }
        if let Some(db) = &self.cache_db {
            db.put_library("ytm", &self.ytm_playlists);
        }
        if !self.ytm.is_authenticated() {
            return;
        }
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = ytm.set_playlist_saved(&id, save).await {
                let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                    "Library sync failed: {e}"
                )));
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
            let vid = if track.source == ProviderSource::YouTubeMusic
                && !track.is_jiosaavn()
                && !track.is_self_hosted()
            {
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
                        is_owned: true,
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
        if playlist_id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX) {
            if let Some(new_name) = title
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                self.rename_personal_playlist(&playlist_id, &new_name);
            }
            return;
        }
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
        if playlist_id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX) {
            self.add_track_to_personal_playlist(track, playlist_id);
            return;
        }
        if is_personal_track_id(&track.id) {
            self.status_line =
                "Local and server tracks can only be added to Personal Playlists".to_string();
            return;
        }
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music to edit playlists".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = format!("Adding \"{}\" to {}...", track.title, playlist_name);
        tokio::spawn(async move {
            let vid = if track.source == ProviderSource::YouTubeMusic
                && !track.is_jiosaavn()
                && !track.is_self_hosted()
            {
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
        if pl_id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX) {
            self.remove_track_from_personal_playlist(&pl_id, idx);
            return;
        }
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

    pub(super) fn refresh_downloads(&mut self) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let (tracks_res, cols_res, bytes_res) = tokio::join!(
                store.list_tracks_async(),
                store.list_collections_async(),
                store.total_bytes_async(),
            );
            let _ = int_tx.send(InternalEvent::DownloadsLoaded {
                tracks: tracks_res.unwrap_or_default(),
                collections: cols_res.unwrap_or_default(),
                total_bytes: bytes_res.unwrap_or(0),
            });
        });
    }

    fn ensure_download_store(&mut self) -> Option<DownloadStore> {
        if self.download_store.is_none() {
            if let Ok(store) = DownloadStore::open(self.config.effective_downloads_dir()) {
                self.resolver.set_downloads_store(Some(store.clone()));
                self.download_store = Some(store);
            }
        }
        self.download_store.clone()
    }

    pub(super) fn download_track(
        &mut self,
        track: Track,
        collection: Option<DownloadCollectionRef>,
    ) {
        let Some(store) = self.ensure_download_store() else {
            self.status_line = "Failed to open offline downloads store".to_string();
            return;
        };
        let track_id = track.id.clone();
        let title = track.title.clone();
        self.downloading_track_ids.insert(track_id.clone());
        self.status_line = format!("Downloading \"{title}\"...");

        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let hifi = self.config.hifi_substitution;
        let limit_bytes = self.config.downloads_limit_bytes();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let result = download_one_track_into_store(
                &store,
                &resolver,
                &http,
                cache_db.as_ref(),
                &track,
                collection,
                hifi,
                limit_bytes,
            )
            .await
            .map_err(|err| err.to_string());
            let _ = int_tx.send(InternalEvent::TrackDownloadFinished {
                track_id,
                title,
                result,
            });
        });
    }

    pub(super) fn remove_downloaded_track(&mut self, track_id: String) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match store.delete_track_async(track_id).await {
                Ok(true) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(
                        "Removed track from offline downloads".to_string(),
                    ));
                }
                Ok(false) => {}
                Err(err) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Failed to remove download: {err}"
                    )));
                }
            }
            let (tracks_res, cols_res, bytes_res) = tokio::join!(
                store.list_tracks_async(),
                store.list_collections_async(),
                store.total_bytes_async(),
            );
            let _ = int_tx.send(InternalEvent::DownloadsLoaded {
                tracks: tracks_res.unwrap_or_default(),
                collections: cols_res.unwrap_or_default(),
                total_bytes: bytes_res.unwrap_or(0),
            });
        });
    }

    pub(super) fn download_collection(
        &mut self,
        collection_id: String,
        title: String,
        source: ProviderSource,
        artwork_url: Option<String>,
        tracks: Vec<Track>,
    ) {
        if tracks.is_empty() {
            return;
        }
        let Some(store) = self.ensure_download_store() else {
            self.status_line = "Failed to open offline downloads store".to_string();
            return;
        };
        for t in &tracks {
            self.downloading_track_ids.insert(t.id.clone());
        }
        self.status_line = format!("Downloading {} tracks from \"{title}\"...", tracks.len());

        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let hifi = self.config.hifi_substitution;
        let limit_bytes = self.config.downloads_limit_bytes();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let mut saved = 0usize;
            let mut errors = 0usize;
            for (pos, track) in tracks.into_iter().enumerate() {
                let col_ref = DownloadCollectionRef {
                    collection_id: collection_id.clone(),
                    title: title.clone(),
                    source,
                    artwork_url: artwork_url.clone(),
                    position: pos,
                };
                let track_id = track.id.clone();
                let track_title = track.title.clone();
                let res = download_one_track_into_store(
                    &store,
                    &resolver,
                    &http,
                    cache_db.as_ref(),
                    &track,
                    Some(col_ref),
                    hifi,
                    limit_bytes,
                )
                .await
                .map_err(|err| err.to_string());
                match res {
                    Ok(()) => saved += 1,
                    Err(_) => errors += 1,
                }
                let _ = int_tx.send(InternalEvent::TrackDownloadFinished {
                    track_id,
                    title: track_title,
                    result: res,
                });
            }
            let _ = int_tx.send(InternalEvent::CollectionDownloadFinished {
                title,
                saved,
                errors,
            });
        });
    }

    pub(super) fn remove_downloaded_collection(&mut self, collection_id: String) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let removed = store
                .delete_collection_async(collection_id, true)
                .await
                .unwrap_or(0);
            let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                "Removed collection ({removed} tracks freed)"
            )));
            let (tracks_res, cols_res, bytes_res) = tokio::join!(
                store.list_tracks_async(),
                store.list_collections_async(),
                store.total_bytes_async(),
            );
            let _ = int_tx.send(InternalEvent::DownloadsLoaded {
                tracks: tracks_res.unwrap_or_default(),
                collections: cols_res.unwrap_or_default(),
                total_bytes: bytes_res.unwrap_or(0),
            });
        });
    }

    pub(super) fn clear_all_downloads(&mut self) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let int_tx = self.int_tx.clone();
        self.status_line = "Clearing offline downloads...".to_string();
        tokio::spawn(async move {
            let report = store.prune_to_limit_async(0).await.unwrap_or_default();
            let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                "Cleared {} offline tracks",
                report.removed_tracks
            )));
            let (tracks_res, cols_res, bytes_res) = tokio::join!(
                store.list_tracks_async(),
                store.list_collections_async(),
                store.total_bytes_async(),
            );
            let _ = int_tx.send(InternalEvent::DownloadsLoaded {
                tracks: tracks_res.unwrap_or_default(),
                collections: cols_res.unwrap_or_default(),
                total_bytes: bytes_res.unwrap_or(0),
            });
        });
    }

    pub(super) fn set_downloads_dir(&mut self, dir: Option<PathBuf>) {
        let normalized = dir.and_then(|p| {
            let s = p.to_string_lossy().trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(PathBuf::from(s))
            }
        });
        self.config.downloads_dir = normalized;
        let _ = self.config.save();
        let target_dir = self.config.effective_downloads_dir();
        match DownloadStore::open(&target_dir) {
            Ok(store) => {
                self.resolver.set_downloads_store(Some(store.clone()));
                self.download_store = Some(store);
                self.status_line = format!("Downloads folder: {}", target_dir.display());
                self.refresh_downloads();
            }
            Err(err) => {
                self.status_line = format!("Invalid downloads folder: {err}");
            }
        }
    }

    pub(super) fn step_downloads_limit(&mut self, forward: bool) {
        const STEPS: &[u8] = &[0, 1, 2, 5, 10, 20, 50, 100];
        let cur = self.config.downloads_limit_gb;
        let idx = STEPS
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.abs_diff(cur))
            .map(|(i, _)| i)
            .unwrap_or(4);
        let next = if forward {
            (idx + 1) % STEPS.len()
        } else {
            (idx + STEPS.len() - 1) % STEPS.len()
        };
        self.config.downloads_limit_gb = STEPS[next];
        let _ = self.config.save();
        if let (Some(limit), Some(store)) = (
            self.config.downloads_limit_bytes(),
            self.download_store.clone(),
        ) {
            let int_tx = self.int_tx.clone();
            tokio::spawn(async move {
                let _ = store.prune_to_limit_async(limit).await;
                let (tracks_res, cols_res, bytes_res) = tokio::join!(
                    store.list_tracks_async(),
                    store.list_collections_async(),
                    store.total_bytes_async(),
                );
                let _ = int_tx.send(InternalEvent::DownloadsLoaded {
                    tracks: tracks_res.unwrap_or_default(),
                    collections: cols_res.unwrap_or_default(),
                    total_bytes: bytes_res.unwrap_or(0),
                });
            });
        }
        self.status_line = format!(
            "Offline downloads limit: {}",
            AppConfig::downloads_limit_label(self.config.downloads_limit_gb)
        );
    }

    pub(super) fn scan_local_music(&mut self) {
        if !self.config.local_music_enabled {
            self.local_scanning = false;
            self.local_scan = Default::default();
            return;
        }
        self.local_scan_req_id = self.local_scan_req_id.wrapping_add(1);
        let req_id = self.local_scan_req_id;
        let dirs = self.config.effective_local_music_dirs();
        self.local_scanning = true;
        self.status_line = "Scanning local music folders...".to_string();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let scan = tokio::task::spawn_blocking(move || scan_directories(&dirs))
                .await
                .unwrap_or_default();
            let _ = int_tx.send(InternalEvent::LocalMusicScanned { req_id, scan });
        });
    }

    pub(super) fn create_personal_playlist(&mut self, name: &str) {
        let clean = name.trim().to_string();
        if clean.is_empty() {
            return;
        }
        let paths = self.paths.clone();
        let int_tx = self.int_tx.clone();
        self.status_line = format!("Creating personal playlist \"{clean}\"...");
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let store = PersonalPlaylistStore::new(&paths);
                let created = store.create(&clean)?;
                let all = store.list()?;
                Ok::<_, sonata_core::error::CoreError>((created, all))
            })
            .await;
            match res {
                Ok(Ok((created, all))) => {
                    let _ = int_tx.send(InternalEvent::PersonalPlaylistsLoaded(all));
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Created personal playlist \"{}\"",
                        created.name
                    )));
                }
                Ok(Err(e)) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Personal playlist failed: {e}"
                    )));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Personal playlist task failed: {e}"
                    )));
                }
            }
        });
    }

    pub(super) fn rename_personal_playlist(&mut self, playlist_id: &str, name: &str) {
        let clean = name.trim().to_string();
        if clean.is_empty() {
            return;
        }
        let paths = self.paths.clone();
        let pid = playlist_id.to_string();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let store = PersonalPlaylistStore::new(&paths);
                let updated = store.rename(&pid, &clean)?;
                let all = store.list()?;
                Ok::<_, sonata_core::error::CoreError>((updated, all))
            })
            .await;
            if let Ok(Ok((updated, all))) = res {
                let _ = int_tx.send(InternalEvent::PersonalPlaylistsLoaded(all));
                let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                    "Renamed personal playlist to \"{}\"",
                    updated.name
                )));
            }
        });
    }

    pub(super) fn delete_personal_playlist(&mut self, playlist_id: &str) {
        let paths = self.paths.clone();
        let pid = playlist_id.to_string();
        let int_tx = self.int_tx.clone();
        if self.playlist_id.as_deref() == Some(playlist_id) {
            self.active_nav = NavTarget::Library;
            self.playlist_id = None;
            self.playlist_tracks.clear();
        }
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let store = PersonalPlaylistStore::new(&paths);
                let removed = store.delete(&pid)?;
                let all = store.list()?;
                Ok::<_, sonata_core::error::CoreError>((removed, all))
            })
            .await;
            if let Ok(Ok((removed, all))) = res {
                let _ = int_tx.send(InternalEvent::PersonalPlaylistsLoaded(all));
                if removed {
                    let _ = int_tx.send(InternalEvent::StatusMessage(
                        "Deleted personal playlist".to_string(),
                    ));
                }
            }
        });
    }

    pub(super) fn add_track_to_personal_playlist(&mut self, track: Track, playlist_id: String) {
        let paths = self.paths.clone();
        let int_tx = self.int_tx.clone();
        let title = track.title.clone();
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let store = PersonalPlaylistStore::new(&paths);
                let updated = store.add_track(&playlist_id, track)?;
                let all = store.list()?;
                Ok::<_, sonata_core::error::CoreError>((updated, all))
            })
            .await;
            match res {
                Ok(Ok((updated, all))) => {
                    let _ = int_tx.send(InternalEvent::PersonalPlaylistsLoaded(all));
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Added \"{title}\" to {}",
                        updated.name
                    )));
                }
                Ok(Err(e)) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Could not add to personal playlist: {e}"
                    )));
                }
                Err(e) => {
                    let _ = int_tx.send(InternalEvent::StatusMessage(format!(
                        "Personal playlist task failed: {e}"
                    )));
                }
            }
        });
    }

    pub(super) fn remove_track_from_personal_playlist(&mut self, playlist_id: &str, index: usize) {
        let Some(track) = self
            .personal_playlists
            .iter()
            .find(|p| p.id == playlist_id)
            .and_then(|p| p.tracks.get(index))
            .cloned()
        else {
            return;
        };
        let paths = self.paths.clone();
        let pid = playlist_id.to_string();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                let store = PersonalPlaylistStore::new(&paths);
                store.remove_track(&pid, &track.id)?;
                store.list()
            })
            .await;
            if let Ok(Ok(all)) = res {
                let _ = int_tx.send(InternalEvent::PersonalPlaylistsLoaded(all));
            }
        });
    }

    pub(super) fn probe_all_server_health(&mut self) {
        let ids: Vec<String> = self.server_profiles.iter().map(|p| p.id.clone()).collect();
        for sid in ids {
            self.probe_server_health(sid);
        }
    }

    pub(super) fn probe_server_health(&mut self, server_id: String) {
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == server_id)
            .cloned()
        else {
            return;
        };
        let client = self.server_client.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let report = client.probe_server_health(&profile).await;
            let _ = int_tx.send(InternalEvent::ServerHealthProbed { server_id, report });
        });
    }

    pub(super) fn run_server_speed_test(&mut self, server_id: String) {
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == server_id)
            .cloned()
        else {
            return;
        };
        self.server_speed_testing.insert(server_id.clone());
        self.status_line = format!("Running speed test for {}...", profile.name);
        let client = self.server_client.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let report = client.run_speed_test(&profile).await;
            let _ = int_tx.send(InternalEvent::ServerSpeedTestFinished { server_id, report });
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn test_server_draft(
        &mut self,
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
        share: Option<String>,
        domain: Option<String>,
        local_mount_path: Option<PathBuf>,
    ) {
        self.server_test_result = None;
        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let mut profile = build_server_profile(
                None,
                &name,
                kind,
                &base_url,
                username,
                password,
                api_key,
                bearer_token,
                share,
                domain,
                local_mount_path,
            );
            let res = prepare_and_ping_server(&http, &mut profile).await;
            let _ = int_tx.send(InternalEvent::ServerTested {
                server_id: None,
                result: res,
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn save_server_profile(
        &mut self,
        id: Option<String>,
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
        share: Option<String>,
        domain: Option<String>,
        local_mount_path: Option<PathBuf>,
    ) {
        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let mut profile = build_server_profile(
                id.as_deref(),
                &name,
                kind,
                &base_url,
                username,
                password,
                api_key,
                bearer_token,
                share,
                domain,
                local_mount_path,
            );
            let ping_res = prepare_and_ping_server(&http, &mut profile).await;
            let message = match ping_res {
                Ok(msg) => format!("Saved {} ({msg})", profile.name),
                Err(err) => format!("Saved {} (warning: {err})", profile.name),
            };
            let _ = int_tx.send(InternalEvent::ServerSaved { profile, message });
        });
    }

    pub(super) fn toggle_server_enabled(&mut self, server_id: String) {
        if let Some(profile) = self.server_profiles.iter_mut().find(|p| p.id == server_id) {
            profile.enabled = !profile.enabled;
        } else {
            return;
        }
        let profiles = self.server_profiles.clone();
        let paths = self.paths.clone();
        tokio::spawn(async move {
            let _ =
                tokio::task::spawn_blocking(move || save_server_profiles(&paths, &profiles)).await;
        });
    }

    pub(super) fn remove_server_profile(&mut self, server_id: String) {
        self.server_profiles.retain(|p| p.id != server_id);
        self.server_statuses.remove(&server_id);
        if self.active_server_id.as_deref() == Some(server_id.as_str()) {
            self.active_server_id = self.server_profiles.first().map(|p| p.id.clone());
            self.server_libraries.clear();
            self.server_artists.clear();
            self.server_albums.clear();
            self.server_playlists.clear();
            self.server_tracks.clear();
            self.server_current_folder = None;
            if let Some(next_id) = self.active_server_id.clone() {
                self.load_server_browse(next_id, None, None);
            }
        }
        let profiles = self.server_profiles.clone();
        let paths = self.paths.clone();
        tokio::spawn(async move {
            let _ =
                tokio::task::spawn_blocking(move || save_server_profiles(&paths, &profiles)).await;
        });
        self.status_line = "Removed server profile".to_string();
    }

    pub(super) fn load_server_browse(
        &mut self,
        server_id: String,
        library_id: Option<String>,
        folder_path: Option<String>,
    ) {
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == server_id)
            .cloned()
        else {
            return;
        };
        self.server_req_id = self.server_req_id.wrapping_add(1);
        let req_id = self.server_req_id;
        self.active_server_id = Some(server_id.clone());
        self.server_loading = true;
        let client = self.server_client.clone();
        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            if profile.kind == ServerKind::WebDav {
                let raw_folder = folder_path.as_deref().map(|raw| {
                    parse_server_item_id(raw)
                        .map(|r| r.item_id)
                        .unwrap_or_else(|| raw.to_string())
                });
                match webdav::browse_folder(&http, &profile, raw_folder.as_deref()).await {
                    Ok(listing) => {
                        let libs = listing
                            .folders
                            .into_iter()
                            .map(|f| sonata_core::providers::servers::ServerLibrary {
                                id: sonata_core::providers::servers::encode_server_item_id(
                                    ServerKind::WebDav,
                                    &profile.id,
                                    &f.path,
                                ),
                                name: f.name,
                                kind: "folder".to_string(),
                            })
                            .collect();
                        let _ = int_tx.send(InternalEvent::ServerBrowseLoaded {
                            req_id,
                            server_id,
                            folder_path: raw_folder,
                            libraries: libs,
                            artists: Vec::new(),
                            albums: Vec::new(),
                            playlists: Vec::new(),
                            tracks: listing.tracks,
                            error: None,
                        });
                    }
                    Err(err) => {
                        let _ = int_tx.send(InternalEvent::ServerBrowseLoaded {
                            req_id,
                            server_id,
                            folder_path,
                            libraries: Vec::new(),
                            artists: Vec::new(),
                            albums: Vec::new(),
                            playlists: Vec::new(),
                            tracks: Vec::new(),
                            error: Some(err.to_string()),
                        });
                    }
                }
                return;
            }

            let lib_arg = library_id.as_deref();
            let (libs_res, artists_res, albums_res, playlists_res) = tokio::join!(
                client.list_libraries(&profile),
                client.list_artists(&profile, lib_arg),
                client.list_albums(&profile, lib_arg, None),
                client.list_playlists(&profile),
            );
            let tracks = if profile.kind == ServerKind::Smb {
                if let Some(lid) = lib_arg {
                    client
                        .get_playlist_tracks(&profile, lid)
                        .await
                        .unwrap_or_default()
                } else {
                    client
                        .get_playlist_tracks(&profile, ".")
                        .await
                        .unwrap_or_default()
                }
            } else {
                Vec::new()
            };
            let error = libs_res
                .as_ref()
                .err()
                .or(albums_res.as_ref().err())
                .or(playlists_res.as_ref().err())
                .map(|e| e.to_string());
            let _ = int_tx.send(InternalEvent::ServerBrowseLoaded {
                req_id,
                server_id,
                folder_path: None,
                libraries: libs_res.unwrap_or_default(),
                artists: artists_res.unwrap_or_default(),
                albums: albums_res.unwrap_or_default(),
                playlists: playlists_res.unwrap_or_default(),
                tracks,
                error,
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn build_server_profile(
    existing_id: Option<&str>,
    name: &str,
    kind: ServerKind,
    base_url: &str,
    username: Option<String>,
    password: Option<String>,
    api_key: Option<String>,
    bearer_token: Option<String>,
    share: Option<String>,
    domain: Option<String>,
    local_mount_path: Option<PathBuf>,
) -> ServerProfile {
    let clean_name = if name.trim().is_empty() {
        kind.display_name().to_string()
    } else {
        name.trim().to_string()
    };
    let id = existing_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            let slug: String = clean_name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .take(12)
                .collect();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() % 100_000)
                .unwrap_or(0);
            format!("{}-{slug}-{nanos}", kind.short_tag().to_ascii_lowercase())
        });
    let opt = |v: Option<String>| {
        v.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
    };
    let mut profile = ServerProfile::new(id, clean_name, kind, base_url.trim());
    profile.username = opt(username);
    profile.password = opt(password);
    profile.api_key = opt(api_key);
    profile.bearer_token = opt(bearer_token);
    profile.share = opt(share);
    profile.domain = opt(domain);
    profile.local_mount_path = local_mount_path.filter(|p| !p.as_os_str().is_empty());
    profile
}

async fn prepare_and_ping_server(
    http: &reqwest::Client,
    profile: &mut ServerProfile,
) -> Result<String, String> {
    if profile.normalized_base_url().is_empty()
        && !(profile.kind == ServerKind::Smb && profile.local_mount_path.is_some())
    {
        return Err("Server URL is required".to_string());
    }
    if profile.kind == ServerKind::Jellyfin
        && profile
            .api_key
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
        && profile
            .access_token
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        let user = profile.username.as_deref().unwrap_or("").trim();
        let pass = profile.password.as_deref().unwrap_or("");
        if !user.is_empty() {
            let auth = jellyfin::authenticate_by_name(
                http,
                &profile.normalized_base_url(),
                user,
                pass,
                &profile.id,
            )
            .await
            .map_err(|err| err.to_string())?;
            profile.access_token = Some(auth.access_token);
            profile.user_id = Some(auth.user_id);
        }
    }
    let client = ServerClient::new(http.clone());
    client.ping(profile).await.map_err(|err| err.to_string())
}

fn infer_container(url: &str, content_type: Option<&str>, badge: &str) -> String {
    if let Some(ct) = content_type {
        let lower = ct.to_ascii_lowercase();
        if lower.contains("flac") {
            return "flac".to_string();
        }
        if lower.contains("opus") || lower.contains("ogg") {
            return "opus".to_string();
        }
        if lower.contains("webm") {
            return "webm".to_string();
        }
        if lower.contains("mpeg") || lower.contains("mp3") {
            return "mp3".to_string();
        }
        if lower.contains("mp4") || lower.contains("aac") || lower.contains("m4a") {
            return "m4a".to_string();
        }
        if lower.contains("wav") {
            return "wav".to_string();
        }
    }
    let path_part = url.split('?').next().unwrap_or(url);
    if let Some(ext) = path_part.rsplit('.').next() {
        let lower = ext.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "flac" | "opus" | "ogg" | "webm" | "m4a" | "mp3" | "aac" | "wav"
        ) {
            return lower;
        }
    }
    let badge_lower = badge.to_ascii_lowercase();
    if badge_lower.contains("flac") {
        "flac".to_string()
    } else if badge_lower.contains("opus") {
        "opus".to_string()
    } else if badge_lower.contains("mp3") {
        "mp3".to_string()
    } else {
        "m4a".to_string()
    }
}

#[allow(clippy::too_many_arguments)]
async fn download_one_track_into_store(
    store: &DownloadStore,
    resolver: &LinuxStreamResolver,
    http: &reqwest::Client,
    cache_db: Option<&sonata_core::cache::CacheDb>,
    track: &Track,
    collection: Option<DownloadCollectionRef>,
    hifi: bool,
    limit_bytes: Option<u64>,
) -> anyhow::Result<()> {
    if store.get_track_async(track.id.clone()).await?.is_some() {
        if let Some(col) = collection {
            store
                .attach_track_to_collection_async(track.id.clone(), col)
                .await?;
        }
        return Ok(());
    }

    let mut resolved = resolver.resolve_fast(track).await?;
    if !track.is_self_hosted() && hifi {
        if let Some(upgraded) = resolver.resolve_hifi_upgrade(track, &resolved, true).await {
            resolved = upgraded;
        }
    }

    let artwork_url = match track
        .thumbnail_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(url) => Some(media::upscale_artwork_url(url)),
        None => media::resolve_track_cover(http, cache_db, track)
            .await
            .map(|(url, _)| url),
    };

    let artwork_bytes = if let Some(ref art_url) = artwork_url {
        let mut req = http.get(art_url);
        if urls_same_origin(art_url, &resolved.url) {
            for (k, v) in &resolved.headers {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    req = req.header(name, val);
                }
            }
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => resp.bytes().await.ok().map(|b| b.to_vec()),
            _ => None,
        }
    } else {
        None
    };

    let badge = if resolved.badge.trim().is_empty() {
        track
            .audio_badge
            .clone()
            .unwrap_or_else(|| "Audio".to_string())
    } else {
        resolved.badge.clone()
    };
    let bitrate_kbps = extract_badge_kbps(&badge);

    if resolved.url.starts_with("http://") || resolved.url.starts_with("https://") {
        let mut header_map = HeaderMap::new();
        for (k, v) in &resolved.headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                header_map.insert(name, val);
            }
        }
        let resp = http
            .get(&resolved.url)
            .headers(header_map)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {}", resp.status().as_u16());
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let container = infer_container(&resolved.url, content_type.as_deref(), &badge);
        let audio_bytes = resp.bytes().await?.to_vec();
        let input = DownloadTrackInput {
            track: track.clone(),
            audio_badge: badge,
            bitrate_kbps,
            container,
            artwork_url,
            collection,
        };
        store
            .save_track_bytes_async(input, audio_bytes, artwork_bytes)
            .await?;
    } else {
        let source_path = PathBuf::from(&resolved.url);
        let container = infer_container(&resolved.url, None, &badge);
        let input = DownloadTrackInput {
            track: track.clone(),
            audio_badge: badge,
            bitrate_kbps,
            container,
            artwork_url,
            collection,
        };
        store
            .save_track_from_file_async(input, source_path, artwork_bytes)
            .await?;
    }

    if let Some(limit) = limit_bytes {
        let _ = store.prune_to_limit_async(limit).await;
    }
    Ok(())
}

pub(crate) fn urls_same_origin(a: &str, b: &str) -> bool {
    let (Ok(ua), Ok(ub)) = (reqwest::Url::parse(a), reqwest::Url::parse(b)) else {
        return false;
    };
    let (Some(ha), Some(hb)) = (ua.host_str(), ub.host_str()) else {
        return false;
    };
    ua.scheme().eq_ignore_ascii_case(ub.scheme())
        && ha.eq_ignore_ascii_case(hb)
        && ua.port_or_known_default().is_some()
        && ua.port_or_known_default() == ub.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use super::urls_same_origin;

    #[test]
    fn same_origin_matches_exact_scheme_host_and_effective_port() {
        assert!(urls_same_origin(
            "https://music.example.com/Items/1/Images/Primary",
            "https://music.example.com:443/Audio/1/universal"
        ));
        assert!(urls_same_origin(
            "http://NAS.local:8080/art.jpg",
            "http://nas.local:8080/track.flac"
        ));
        assert!(!urls_same_origin(
            "https://cdn-images.dzcdn.net/images/cover/abc/500x500.jpg",
            "https://music.example.com/Audio/1/universal"
        ));
        assert!(!urls_same_origin(
            "https://music.example.com/cover.jpg",
            "https://music.example.com:8920/Audio/1/universal"
        ));
        assert!(!urls_same_origin(
            "http://music.example.com/cover.jpg",
            "https://music.example.com/Audio/1/universal"
        ));
        assert!(!urls_same_origin(
            "https://music.example.com/cover.jpg",
            "/home/user/Music/song.flac"
        ));
    }
}
