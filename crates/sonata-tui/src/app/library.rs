//! Library browsing, search, playlist mutations, and cache/account management for the Sonata TUI.
//!
//! 1. **Search & Browse**: provider-mode switching, song search across YouTube Music and Spotify, and progressive playlist loading with cache backing.
//! 2. **Library Mutations**: liking/unliking tracks, creating YouTube Music playlists, adding/removing playlist items, and importing Spotify playlists.
//! 3. **Cache & Account**: adjusts active media cache limits and refreshes browser session cookies and playlists.

use super::{
    resolve_tui_playlist_add_targets, ActiveModal, App, AppEvent, FocusPane, ServerBrowserState,
    ServerBrowserTab, ServerFormState, SidebarEntry, TuiPlaylistAddTarget,
};
use sonata_core::cache::enforce_media_cache_limit_async;
use sonata_core::playlists::{
    is_personal_track_id, PersonalPlaylistStore, PERSONAL_PLAYLIST_ID_PREFIX,
};
use sonata_core::providers::servers::ServerKind;
use sonata_core::providers::spotify::extract_playlist_id;
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_runtime::config::{AppConfig, ProviderMode};
use sonata_runtime::player::SessionCommand;
use std::sync::atomic::Ordering;

impl App {
    pub fn cycle_provider_mode(&mut self, forward: bool) {
        let next = if forward {
            self.config.provider_mode.next()
        } else {
            self.config.provider_mode.prev()
        };
        self.config.provider_mode = next;
        let _ = self.config.save();
        self.rebuild_sidebar();
        self.status_line = format!("Music Providers: {}", next.label());
    }

    pub fn step_cache_limit(&mut self, forward: bool) {
        self.config.cache_limit_gb =
            AppConfig::step_cache_limit_gb(self.config.cache_limit_gb, forward);
        let _ = self.config.save();
        let limit_bytes = self.config.cache_limit_bytes();
        self.cache_limit_bytes.store(limit_bytes, Ordering::Relaxed);
        self.resolver
            .update_profile(|profile| profile.cache_limit_bytes = limit_bytes);
        if let Some(cache) = &self.cache_db {
            cache.set_enabled(self.config.cache_limit_gb > 0);
        }
        self.enqueue_session_cmd(SessionCommand::UpdatePolicy {
            policy: self.session_policy(),
            trigger_upgrade: false,
        });
        let paths = self.paths.clone();
        tokio::spawn(async move {
            enforce_media_cache_limit_async(paths, limit_bytes).await;
        });
        self.status_line = if self.config.cache_limit_gb == 0 {
            "Cache disabled; media cleanup started".to_string()
        } else {
            format!(
                "Cache limit: {}",
                AppConfig::cache_limit_label(self.config.cache_limit_gb)
            )
        };
    }

    pub fn trigger_search(&mut self) {
        let q = self.search_query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
        let req_id = self.tracks_req_id;
        self.status_line = format!("Searching for \"{q}\"...");
        let ytm = self.ytm.clone();
        let spotify = self.spotify.clone();
        let http = self.http.clone();
        let personal_only = self.config.personal_only_mode;
        let jiosaavn_enabled = self.config.effective_jiosaavn_enabled();
        let mode = self.config.provider_mode;
        let local_matches: Vec<Track> = if self.config.local_music_enabled {
            let needle = q.to_lowercase();
            self.local_scan
                .tracks
                .iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&needle)
                        || t.artist.to_lowercase().contains(&needle)
                        || t.album.to_lowercase().contains(&needle)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let server_client = self.server_client.clone();
        let enabled_servers: Vec<sonata_core::providers::servers::ServerProfile> = self
            .server_profiles
            .iter()
            .filter(|p| p.enabled)
            .cloned()
            .collect();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut combined = local_matches;
            let jio_fut = async {
                if !personal_only && jiosaavn_enabled {
                    sonata_core::providers::jiosaavn::search_tracks(&http, &q, 8).await
                } else {
                    Vec::new()
                }
            };
            let servers_fut = async {
                let mut tracks = Vec::new();
                for srv in &enabled_servers {
                    if let Ok(res) = server_client.search(srv, &q, 10).await {
                        tracks.extend(res.tracks);
                    }
                }
                tracks
            };
            if personal_only {
                combined.extend(servers_fut.await);
            } else {
                match mode {
                    ProviderMode::Both => {
                        let (ytm_tracks, spot_tracks, jio_tracks, srv_tracks) = tokio::join!(
                            ytm.search_songs(&q),
                            spotify.search_tracks(&q),
                            jio_fut,
                            servers_fut
                        );
                        combined.extend(ytm_tracks.unwrap_or_default());
                        combined.extend(spot_tracks.unwrap_or_default());
                        combined.extend(jio_tracks);
                        combined.extend(srv_tracks);
                    }
                    ProviderMode::YouTubeMusic => {
                        let (ytm_tracks, jio_tracks, srv_tracks) =
                            tokio::join!(ytm.search_songs(&q), jio_fut, servers_fut);
                        combined.extend(ytm_tracks.unwrap_or_default());
                        combined.extend(jio_tracks);
                        combined.extend(srv_tracks);
                    }
                    ProviderMode::Spotify => {
                        let (spot_tracks, jio_tracks, srv_tracks) =
                            tokio::join!(spotify.search_tracks(&q), jio_fut, servers_fut);
                        combined.extend(spot_tracks.unwrap_or_default());
                        combined.extend(jio_tracks);
                        combined.extend(srv_tracks);
                    }
                }
            }
            let _ = tx.send(AppEvent::TracksLoaded {
                req_id,
                playlist_id: None,
                heading: format!("Search: {q}"),
                tracks: combined,
                silent_replace: false,
            });
        });
    }

    pub fn activate_sidebar_selection(&mut self) {
        let Some(entry) = self.sidebar_items.get(self.sidebar_idx).cloned() else {
            return;
        };
        match entry {
            SidebarEntry::Search => {
                self.focus = FocusPane::SearchInput;
            }
            SidebarEntry::Queue => {
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                self.current_playlist_id = None;
                self.tracks_heading = "Playback Queue".to_string();
                self.visible_tracks = self.playback.queue.clone();
                self.track_table_state
                    .select(self.playback.current_index.or(Some(0)));
                self.focus = FocusPane::Tracks;
            }
            SidebarEntry::Downloads => {
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                self.current_playlist_id = Some("SONATA:DOWNLOADS".to_string());
                self.tracks_heading = format!("Downloads ({})", self.downloaded_tracks.len());
                self.visible_tracks = self
                    .downloaded_tracks
                    .iter()
                    .map(|d| d.to_track())
                    .collect();
                self.track_table_state.select(Some(0));
                self.focus = FocusPane::Tracks;
                self.refresh_downloads();
            }
            SidebarEntry::LocalSongs => {
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                self.current_playlist_id = Some("SONATA:LOCAL_SONGS".to_string());
                self.tracks_heading = format!("Local Songs ({})", self.local_scan.tracks.len());
                self.visible_tracks = self.local_scan.tracks.clone();
                self.track_table_state
                    .select((!self.visible_tracks.is_empty()).then_some(0));
                self.focus = FocusPane::Tracks;
            }
            SidebarEntry::LocalAlbum(idx) => {
                if let Some(album) = self.local_scan.albums.get(idx).cloned() {
                    self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                    self.current_playlist_id = Some(format!("local_album:{}", album.id));
                    self.tracks_heading = format!("[Local Album] {}", album.name);
                    self.visible_tracks = album.tracks;
                    self.track_table_state
                        .select((!self.visible_tracks.is_empty()).then_some(0));
                    self.focus = FocusPane::Tracks;
                }
            }
            SidebarEntry::LocalFolder(idx) => {
                if let Some(folder) = self.local_scan.folders.get(idx).cloned() {
                    self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                    self.current_playlist_id = Some(format!("local_folder:{}", folder.name));
                    self.tracks_heading = format!("[Folder] {}", folder.name);
                    self.visible_tracks = folder.tracks;
                    self.track_table_state
                        .select((!self.visible_tracks.is_empty()).then_some(0));
                    self.focus = FocusPane::Tracks;
                }
            }
            SidebarEntry::PersonalPlaylist(ppl) => {
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                let live = self
                    .personal_playlists
                    .iter()
                    .find(|p| p.id == ppl.id)
                    .cloned()
                    .unwrap_or(ppl);
                self.current_playlist_id = Some(live.id.clone());
                self.tracks_heading = format!("[Personal] {}", live.name);
                self.visible_tracks = live.tracks;
                self.track_table_state
                    .select((!self.visible_tracks.is_empty()).then_some(0));
                self.focus = FocusPane::Tracks;
            }
            SidebarEntry::Server(profile) => {
                self.open_server_browser(profile);
            }
            SidebarEntry::Playlist(pl) => {
                if self.config.personal_only_mode {
                    self.status_line =
                        "Personal Only Mode active: streaming playlists disabled".to_string();
                    return;
                }
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                let req_id = self.tracks_req_id;
                self.status_line = format!("Loading {}...", pl.name);
                let ytm = self.ytm.clone();
                let spotify = self.spotify.clone();
                let cache_db = self.cache_db.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let ytm_pid = if pl.source == ProviderSource::YouTubeMusic {
                        Some(pl.id.clone())
                    } else {
                        None
                    };
                    let db_key = format!("{}:{}", pl.source.short_tag(), pl.id);
                    let mut had_cached = false;
                    let cached_playlist = if let Some(db) = cache_db.clone() {
                        let key = db_key.clone();
                        tokio::task::spawn_blocking(move || db.get_playlist(&key))
                            .await
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                    if let Some((cached_name, cached_tracks, is_fresh)) = cached_playlist {
                        had_cached = !cached_tracks.is_empty();
                        let _ = tx.send(AppEvent::TracksLoaded {
                            req_id,
                            playlist_id: ytm_pid.clone(),
                            heading: format!("[{}] {}", pl.source.short_tag(), cached_name),
                            tracks: cached_tracks,
                            silent_replace: false,
                        });
                        if is_fresh {
                            return;
                        }
                    }

                    let heading = format!("[{}] {}", pl.source.short_tag(), pl.name);
                    let mut cached_name = pl.name.clone();
                    let mut resolved_heading = heading.clone();
                    let mut all_tracks: Vec<Track> = Vec::new();
                    let res = match pl.source {
                        ProviderSource::YouTubeMusic => {
                            ytm.fetch_playlist_tracks_progressive(&pl.id, |batch, is_first| {
                                all_tracks.extend(batch.iter().cloned());
                                if !had_cached {
                                    if is_first {
                                        let _ = tx.send(AppEvent::TracksLoaded {
                                            req_id,
                                            playlist_id: ytm_pid.clone(),
                                            heading: heading.clone(),
                                            tracks: batch,
                                            silent_replace: false,
                                        });
                                    } else {
                                        let _ = tx.send(AppEvent::TracksAppended {
                                            req_id,
                                            heading: heading.clone(),
                                            tracks: batch,
                                        });
                                    }
                                }
                            })
                            .await
                        }
                        ProviderSource::Spotify => {
                            spotify
                                .fetch_playlist_tracks_progressive(
                                    &pl.id,
                                    |name, batch, is_first| {
                                        cached_name = name.to_string();
                                        resolved_heading = format!("[SPF] {name}");
                                        all_tracks.extend(batch.iter().cloned());
                                        if !had_cached {
                                            if is_first {
                                                let _ = tx.send(AppEvent::TracksLoaded {
                                                    req_id,
                                                    playlist_id: None,
                                                    heading: resolved_heading.clone(),
                                                    tracks: batch,
                                                    silent_replace: false,
                                                });
                                            } else {
                                                let _ = tx.send(AppEvent::TracksAppended {
                                                    req_id,
                                                    heading: resolved_heading.clone(),
                                                    tracks: batch,
                                                });
                                            }
                                        }
                                    },
                                )
                                .await
                        }
                    };
                    match res {
                        Ok(()) => {
                            if let Some(db) = cache_db.clone() {
                                let key = db_key.clone();
                                let name = cached_name.clone();
                                let tracks_to_cache = all_tracks.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    db.put_playlist(&key, &name, &tracks_to_cache);
                                })
                                .await;
                            }
                            if had_cached && !all_tracks.is_empty() {
                                let _ = tx.send(AppEvent::TracksLoaded {
                                    req_id,
                                    playlist_id: ytm_pid,
                                    heading: resolved_heading,
                                    tracks: all_tracks,
                                    silent_replace: true,
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::StatusMessage(format!(
                                "Failed to load playlist: {e}"
                            )));
                        }
                    }
                });
                self.focus = FocusPane::Tracks;
            }
        }
    }

    pub fn import_spotify_playlist(&mut self, input: &str) {
        let Some(id) = extract_playlist_id(input) else {
            self.status_line = "Invalid Spotify playlist URL or ID".to_string();
            return;
        };
        if !self.config.spotify_playlists.contains(&id) {
            self.config.spotify_playlists.push(id.clone());
            let _ = self.config.save();
        }
        self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
        let req_id = self.tracks_req_id;
        let spotify = self.spotify.clone();
        let tx = self.tx.clone();
        self.status_line = "Importing Spotify playlist...".to_string();
        tokio::spawn(async move {
            let res = spotify
                .fetch_playlist_tracks_progressive(&id, |name, batch, is_first| {
                    let heading = format!("[SPF] {name}");
                    if is_first {
                        let _ = tx.send(AppEvent::TracksLoaded {
                            req_id,
                            playlist_id: None,
                            heading,
                            tracks: batch,
                            silent_replace: false,
                        });
                    } else {
                        let _ = tx.send(AppEvent::TracksAppended {
                            req_id,
                            heading,
                            tracks: batch,
                        });
                    }
                })
                .await;
            if let Err(e) = res {
                let _ = tx.send(AppEvent::StatusMessage(format!(
                    "Spotify import failed: {e}"
                )));
            }
        });
    }

    pub fn playlist_add_targets_for_selected(&self) -> Vec<TuiPlaylistAddTarget> {
        let Some(track) = self.selected_or_playing_track() else {
            return Vec::new();
        };
        resolve_tui_playlist_add_targets(
            &track,
            &self.personal_playlists,
            &self.ytm_playlists,
            self.config.personal_only_mode,
            self.ytm.is_authenticated(),
        )
    }

    pub fn toggle_like_selected(&mut self) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music in your browser to sync likes".to_string();
            return;
        }
        let Some(track) = self.selected_or_playing_track() else {
            return;
        };
        if track.is_self_hosted() {
            self.status_line =
                "Self-hosted server tracks cannot be synced to YouTube Music likes".to_string();
            return;
        }
        let new_liked = !track.liked;
        for t in &mut self.visible_tracks {
            if t.id == track.id {
                t.liked = new_liked;
            }
        }
        for t in &mut self.playback.queue {
            if t.id == track.id {
                t.liked = new_liked;
            }
        }

        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let tx = self.tx.clone();
        let title = track.title.clone();
        self.status_line = format!(
            "{} \"{}\" on YouTube Music...",
            if new_liked { "Liking" } else { "Unliking" },
            title
        );

        tokio::spawn(async move {
            let video_id = if track.source == ProviderSource::YouTubeMusic
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
            let Some(vid) = video_id else {
                let _ = tx.send(AppEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.set_like_status(&vid, new_liked).await {
                Ok(()) => {
                    if let Some(db) = cache_db.clone() {
                        let _ = tokio::task::spawn_blocking(move || {
                            db.invalidate_playlist(Some("YTM:VLLM"));
                        })
                        .await;
                    }
                    let msg = if new_liked {
                        format!("Saved \"{title}\" to YouTube Music Liked Songs")
                    } else {
                        format!("Removed \"{title}\" from Liked Songs")
                    };
                    let _ = tx.send(AppEvent::StatusMessage(msg));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StatusMessage(format!("Like sync failed: {e}")));
                }
            }
        });
    }

    pub fn open_add_to_playlist_modal(&mut self) {
        if self.selected_or_playing_track().is_none() {
            self.status_line = "Select a track first".to_string();
            return;
        }
        if self.playlist_add_targets_for_selected().is_empty() {
            self.status_line =
                "No playlists available — press c to create a Personal Playlist".to_string();
            return;
        }
        self.modal = ActiveModal::AddToPlaylist(0);
    }

    pub fn confirm_add_to_playlist(&mut self, pl_idx: usize) {
        let targets = self.playlist_add_targets_for_selected();
        let Some(target) = targets.get(pl_idx).cloned() else {
            return;
        };
        let Some(track) = self.selected_or_playing_track() else {
            return;
        };
        let pl = match target {
            TuiPlaylistAddTarget::Personal(ppl) => {
                self.add_track_to_personal_playlist(&ppl.id, track);
                return;
            }
            TuiPlaylistAddTarget::Ytm(pl) => pl,
        };
        if self.config.personal_only_mode
            || is_personal_track_id(&track.id)
            || track.is_self_hosted()
        {
            self.status_line =
                "Personal/local/server tracks can only be added to Personal Playlists".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Adding \"{}\" to {}...", track.title, pl.name);

        tokio::spawn(async move {
            let video_id = if track.source == ProviderSource::YouTubeMusic
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
            let Some(vid) = video_id else {
                let _ = tx.send(AppEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.add_to_playlist(&pl.id, &vid).await {
                Ok(()) => {
                    if let Some(db) = cache_db.clone() {
                        let key = format!("YTM:{}", pl.id);
                        let _ = tokio::task::spawn_blocking(move || {
                            db.invalidate_playlist(Some(&key));
                        })
                        .await;
                    }
                    let _ = tx.send(AppEvent::StatusMessage(format!(
                        "Added \"{}\" to {}",
                        track.title, pl.name
                    )));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StatusMessage(format!("Add failed: {e}")));
                }
            }
        });
    }

    pub fn remove_selected_from_playlist(&mut self) {
        let Some(pl_id) = self.current_playlist_id.clone() else {
            self.status_line = "Open a playlist to remove tracks".to_string();
            return;
        };
        if pl_id == "VLLM" {
            self.toggle_like_selected();
            return;
        }
        let Some(idx) = self.track_table_state.selected() else {
            return;
        };
        if pl_id.starts_with(PERSONAL_PLAYLIST_ID_PREFIX) {
            let Some(removed_track) = self.visible_tracks.get(idx).cloned() else {
                return;
            };
            self.visible_tracks.remove(idx);
            if self.visible_tracks.is_empty() {
                self.track_table_state.select(None);
            } else {
                self.track_table_state
                    .select(Some(idx.min(self.visible_tracks.len() - 1)));
            }
            self.remove_track_from_personal_playlist(&pl_id, &removed_track.id);
            return;
        }
        let Some(track) = self.visible_tracks.get(idx).cloned() else {
            return;
        };
        let Some(set_vid) = track.set_video_id.clone() else {
            self.status_line =
                "Track missing playlistSetVideoId; press R to refresh playlist".to_string();
            return;
        };

        self.visible_tracks.remove(idx);
        if self.visible_tracks.is_empty() {
            self.track_table_state.select(None);
        } else {
            self.track_table_state
                .select(Some(idx.min(self.visible_tracks.len() - 1)));
        }

        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Removing \"{}\" from playlist...", track.title);
        tokio::spawn(async move {
            match ytm.remove_from_playlist(&pl_id, &track.id, &set_vid).await {
                Ok(()) => {
                    if let Some(db) = cache_db.clone() {
                        let key = format!("YTM:{pl_id}");
                        let _ = tokio::task::spawn_blocking(move || {
                            db.invalidate_playlist(Some(&key));
                        })
                        .await;
                    }
                    let _ = tx.send(AppEvent::StatusMessage(format!(
                        "Removed \"{}\" from YouTube Music playlist",
                        track.title
                    )));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StatusMessage(format!("Remove failed: {e}")));
                }
            }
        });
    }

    pub fn create_ytm_playlist(&mut self, title: &str) {
        if self.config.personal_only_mode {
            self.create_personal_playlist(title);
            return;
        }
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music in your browser first".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        let tx = self.tx.clone();
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
                    let _ = tx.send(AppEvent::PlaylistCreated(pl));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StatusMessage(format!(
                        "Create playlist failed: {e}"
                    )));
                }
            }
        });
    }

    pub fn create_personal_playlist(&mut self, name: &str) {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let store = PersonalPlaylistStore::new(&self.paths);
        match store.create(&trimmed) {
            Ok(created) => {
                self.personal_playlists = store.list().unwrap_or_default();
                self.rebuild_sidebar();
                self.status_line = format!("Created Personal Playlist \"{}\"", created.name);
            }
            Err(err) => {
                self.status_line = format!("Create Personal Playlist failed: {err}");
            }
        }
    }

    pub fn rename_personal_playlist(&mut self, playlist_id: &str, name: &str) {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let store = PersonalPlaylistStore::new(&self.paths);
        match store.rename(playlist_id, &trimmed) {
            Ok(updated) => {
                self.personal_playlists = store.list().unwrap_or_default();
                self.rebuild_sidebar();
                if self.current_playlist_id.as_deref() == Some(playlist_id) {
                    self.tracks_heading = format!("[Personal] {}", updated.name);
                }
                self.status_line = format!("Renamed Personal Playlist to \"{}\"", updated.name);
            }
            Err(err) => {
                self.status_line = format!("Rename failed: {err}");
            }
        }
    }

    pub fn delete_personal_playlist(&mut self, playlist_id: &str) {
        let store = PersonalPlaylistStore::new(&self.paths);
        match store.delete(playlist_id) {
            Ok(removed) => {
                if removed {
                    self.personal_playlists = store.list().unwrap_or_default();
                    self.rebuild_sidebar();
                    if self.current_playlist_id.as_deref() == Some(playlist_id) {
                        self.current_playlist_id = None;
                        self.visible_tracks.clear();
                    }
                    self.status_line = "Deleted Personal Playlist".to_string();
                }
            }
            Err(err) => {
                self.status_line = format!("Delete failed: {err}");
            }
        }
    }

    pub fn add_track_to_personal_playlist(&mut self, playlist_id: &str, track: Track) {
        let title = track.title.clone();
        let store = PersonalPlaylistStore::new(&self.paths);
        match store.add_track(playlist_id, track) {
            Ok(updated) => {
                self.personal_playlists = store.list().unwrap_or_default();
                self.rebuild_sidebar();
                if self.current_playlist_id.as_deref() == Some(playlist_id) {
                    self.visible_tracks = updated.tracks;
                }
                self.status_line = format!("Added \"{title}\" to {}", updated.name);
            }
            Err(err) => {
                self.status_line = format!("Add failed: {err}");
            }
        }
    }

    pub fn remove_track_from_personal_playlist(&mut self, playlist_id: &str, track_id: &str) {
        let store = PersonalPlaylistStore::new(&self.paths);
        match store.remove_track(playlist_id, track_id) {
            Ok(updated) => {
                self.personal_playlists = store.list().unwrap_or_default();
                self.rebuild_sidebar();
                if self.current_playlist_id.as_deref() == Some(playlist_id) {
                    self.visible_tracks = updated.tracks;
                }
                self.status_line = format!("Removed track from {}", updated.name);
            }
            Err(err) => {
                self.status_line = format!("Remove failed: {err}");
            }
        }
    }

    pub fn scan_local_music(&mut self) {
        if !self.config.local_music_enabled {
            self.local_scanning = false;
            return;
        }
        let dirs = self.config.effective_local_music_dirs();
        self.local_scanning = true;
        self.status_line = "Scanning local music folders...".to_string();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let scan = tokio::task::spawn_blocking(move || {
                sonata_core::providers::local::scan_directories(&dirs)
            })
            .await
            .unwrap_or_default();
            let _ = tx.send(AppEvent::LocalMusicScanned(scan));
        });
    }

    pub fn probe_all_server_health(&self) {
        for profile in self.server_profiles.iter().filter(|p| p.enabled).cloned() {
            let client = self.server_client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let report = client.probe_server_health(&profile).await;
                let _ = tx.send(AppEvent::ServerHealthProbed {
                    server_id: profile.id,
                    report,
                });
            });
        }
    }

    pub fn probe_server_health(&mut self, server_id: &str) {
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == server_id)
            .cloned()
        else {
            return;
        };
        let client = self.server_client.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Checking {}...", profile.name);
        tokio::spawn(async move {
            let report = client.probe_server_health(&profile).await;
            let badge = super::format_tui_health_badge(&report);
            let _ = tx.send(AppEvent::ServerHealthProbed {
                server_id: profile.id,
                report,
            });
            let _ = tx.send(AppEvent::StatusMessage(format!(
                "{}: {badge}",
                profile.name
            )));
        });
    }

    pub fn run_server_speed_test(&mut self, server_id: &str) {
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == server_id)
            .cloned()
        else {
            return;
        };
        self.server_speed_testing.insert(server_id.to_string());
        self.status_line = format!("Running speed test for {}...", profile.name);
        let client = self.server_client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let report = client.run_speed_test(&profile).await;
            let _ = tx.send(AppEvent::ServerSpeedTestFinished {
                server_id: profile.id,
                report,
            });
        });
    }

    pub fn refresh_account(&mut self) {
        if self.config.personal_only_mode {
            self.status_line = "Personal Only Mode active: sign-in refresh skipped".to_string();
            return;
        }
        let ytm = self.ytm.clone();
        if let Some(db) = &self.cache_db {
            db.invalidate_playlist(None);
        }
        tokio::spawn(async move {
            ytm.invalidate_cache(None).await;
        });
        self.status_line = "Refreshing browser session & playlists...".to_string();
        self.start_background_init();
    }

    pub fn refresh_downloads(&self) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let tracks = store.list_tracks_async().await.unwrap_or_default();
            let collections = store.list_collections_async().await.unwrap_or_default();
            let total_bytes = store.total_bytes_async().await.unwrap_or(0);
            let _ = tx.send(AppEvent::DownloadsLoaded {
                tracks,
                collections,
                total_bytes,
            });
        });
    }

    pub fn toggle_download_selected_track(&mut self) {
        let Some(track) = self.selected_or_playing_track() else {
            return;
        };
        let Some(store) = self.download_store.clone() else {
            self.status_line = "Downloads store unavailable".to_string();
            return;
        };
        if self.downloaded_track_ids.contains(&track.id) {
            let tid = track.id.clone();
            let title = track.title.clone();
            let tx = self.tx.clone();
            self.status_line = format!("Removing \"{title}\" from downloads...");
            tokio::spawn(async move {
                match store.delete_track_async(tid).await {
                    Ok(_) => {
                        let tracks = store.list_tracks_async().await.unwrap_or_default();
                        let collections = store.list_collections_async().await.unwrap_or_default();
                        let total_bytes = store.total_bytes_async().await.unwrap_or(0);
                        let _ = tx.send(AppEvent::DownloadsLoaded {
                            tracks,
                            collections,
                            total_bytes,
                        });
                        let _ = tx.send(AppEvent::StatusMessage(format!(
                            "Removed \"{title}\" from offline downloads"
                        )));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::StatusMessage(format!(
                            "Failed to remove download: {err}"
                        )));
                    }
                }
            });
            return;
        }
        if !self.downloading_track_ids.insert(track.id.clone()) {
            return;
        }
        self.status_line = format!("Downloading \"{}\"...", track.title);
        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let hifi = self.config.hifi_substitution;
        let limit_bytes = self.config.downloads_limit_bytes();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let tid = track.id.clone();
            let title = track.title.clone();
            let res = download_tui_track_into_store(
                &store,
                &resolver,
                &http,
                cache_db.as_ref(),
                &track,
                None,
                hifi,
                limit_bytes,
            )
            .await
            .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::TrackDownloadFinished {
                track_id: tid,
                title,
                result: res,
            });
        });
    }

    pub fn toggle_download_visible_collection(&mut self) {
        let Some(store) = self.download_store.clone() else {
            self.status_line = "Downloads store unavailable".to_string();
            return;
        };
        let collection_id = self
            .current_playlist_id
            .clone()
            .unwrap_or_else(|| format!("tui:{}", self.tracks_heading));
        if collection_id == "SONATA:DOWNLOADS" {
            self.clear_all_downloads();
            return;
        }
        if self
            .downloaded_collections
            .iter()
            .any(|c| c.collection_id == collection_id)
        {
            let title = self.tracks_heading.clone();
            let tx = self.tx.clone();
            self.status_line = format!("Removing downloaded collection \"{title}\"...");
            tokio::spawn(async move {
                let _ = store.delete_collection_async(collection_id, true).await;
                let tracks = store.list_tracks_async().await.unwrap_or_default();
                let collections = store.list_collections_async().await.unwrap_or_default();
                let total_bytes = store.total_bytes_async().await.unwrap_or(0);
                let _ = tx.send(AppEvent::DownloadsLoaded {
                    tracks,
                    collections,
                    total_bytes,
                });
                let _ = tx.send(AppEvent::StatusMessage(format!(
                    "Removed \"{title}\" from offline downloads"
                )));
            });
            return;
        }
        if self.visible_tracks.is_empty() {
            return;
        }
        let title = self.tracks_heading.clone();
        let tracks = self.visible_tracks.clone();
        let source = tracks
            .first()
            .map(|t| t.source)
            .unwrap_or(ProviderSource::YouTubeMusic);
        let artwork_url = tracks.first().and_then(|t| t.thumbnail_url.clone());
        for t in &tracks {
            self.downloading_track_ids.insert(t.id.clone());
        }
        self.status_line = format!("Downloading {} tracks from \"{title}\"...", tracks.len());
        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let hifi = self.config.hifi_substitution;
        let limit_bytes = self.config.downloads_limit_bytes();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut saved = 0usize;
            let mut errors = 0usize;
            for (idx, track) in tracks.iter().enumerate() {
                let col_ref = sonata_runtime::downloads::DownloadCollectionRef {
                    collection_id: collection_id.clone(),
                    title: title.clone(),
                    source,
                    artwork_url: artwork_url.clone(),
                    position: idx,
                };
                match download_tui_track_into_store(
                    &store,
                    &resolver,
                    &http,
                    cache_db.as_ref(),
                    track,
                    Some(col_ref),
                    hifi,
                    limit_bytes,
                )
                .await
                {
                    Ok(()) => saved += 1,
                    Err(_) => errors += 1,
                }
            }
            let _ = tx.send(AppEvent::CollectionDownloadFinished {
                title,
                saved,
                errors,
            });
        });
    }

    pub fn clear_all_downloads(&mut self) {
        let Some(store) = self.download_store.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.status_line = "Clearing all offline downloads...".to_string();
        tokio::spawn(async move {
            let _ = store.prune_to_limit_async(0).await;
            let _ = tx.send(AppEvent::DownloadsLoaded {
                tracks: Vec::new(),
                collections: Vec::new(),
                total_bytes: 0,
            });
            let _ = tx.send(AppEvent::StatusMessage(
                "Cleared all offline downloads".to_string(),
            ));
        });
    }

    pub fn step_downloads_limit(&mut self, forward: bool) {
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
        let label = if self.config.downloads_limit_gb == 0 {
            sonata_runtime::i18n::strings(self.config.language)
                .unlimited
                .to_string()
        } else {
            AppConfig::downloads_limit_label(self.config.downloads_limit_gb)
        };
        self.status_line = format!("Offline Downloads Limit: {label}");
        if let Some(limit) = self.config.downloads_limit_bytes() {
            if let Some(store) = self.download_store.clone() {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let _ = store.prune_to_limit_async(limit).await;
                    let tracks = store.list_tracks_async().await.unwrap_or_default();
                    let collections = store.list_collections_async().await.unwrap_or_default();
                    let total_bytes = store.total_bytes_async().await.unwrap_or(0);
                    let _ = tx.send(AppEvent::DownloadsLoaded {
                        tracks,
                        collections,
                        total_bytes,
                    });
                });
            }
        }
    }

    pub fn set_downloads_dir(&mut self, dir: Option<std::path::PathBuf>) {
        let dir = dir.filter(|p| !p.as_os_str().is_empty());
        self.config.downloads_dir = dir;
        let _ = self.config.save();
        let target_dir = self.config.effective_downloads_dir();
        if let Ok(store) = sonata_runtime::downloads::DownloadStore::open(target_dir.clone()) {
            self.download_store = Some(store.clone());
            self.resolver.set_downloads_store(Some(store));
            self.enqueue_session_cmd(sonata_runtime::player::SessionCommand::UpdateStreamSource(
                std::sync::Arc::new(self.resolver.clone()),
            ));
            self.refresh_downloads();
        }
        self.status_line = format!("Downloads folder: {}", target_dir.display());
    }

    pub fn toggle_discord_rpc(&mut self) {
        self.config.discord_rpc_enabled = !self.config.discord_rpc_enabled;
        let _ = self.config.save();
        self.discord_rpc
            .try_update_config(Self::discord_config_from(&self.config));
        self.sync_discord_rpc();
        self.status_line = if self.config.discord_rpc_enabled {
            "Discord Rich Presence enabled".to_string()
        } else {
            "Discord Rich Presence disabled".to_string()
        };
    }

    pub fn set_discord_app_id(&mut self, app_id: Option<String>) {
        let cleaned = app_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self.config.discord_app_id = cleaned.clone();
        let _ = self.config.save();
        self.discord_rpc
            .try_update_config(Self::discord_config_from(&self.config));
        self.sync_discord_rpc();
        self.status_line = match cleaned {
            Some(id) => format!("Discord Application ID: {id}"),
            None => "Discord Application ID reset to default".to_string(),
        };
    }

    pub fn test_server_form(&mut self, form: &ServerFormState) {
        let mut profile = build_tui_server_profile(form);
        let http = self.http.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Testing {}...", profile.name);
        tokio::spawn(async move {
            let res = prepare_and_ping_tui_server(&http, &mut profile).await;
            let _ = tx.send(AppEvent::ServerTested {
                server_id: profile.id,
                result: res,
            });
        });
    }

    pub fn save_server_form(&mut self, form: &ServerFormState) {
        let mut profile = build_tui_server_profile(form);
        let smb_has_mount = profile.kind == ServerKind::Smb && profile.local_mount_path.is_some();
        if profile.base_url.trim().is_empty() && !smb_has_mount {
            self.status_line = "Server URL cannot be empty".to_string();
            return;
        }
        let mut profiles = self.server_profiles.clone();
        let paths = self.paths.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Saving {}...", profile.name);
        tokio::spawn(async move {
            let ping_res = prepare_and_ping_tui_server(&http, &mut profile).await;
            if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
                *existing = profile.clone();
            } else {
                profiles.push(profile.clone());
            }
            let save_paths = paths.clone();
            let save_list = profiles.clone();
            let save_res = tokio::task::spawn_blocking(move || {
                sonata_core::providers::servers::save_server_profiles(&save_paths, &save_list)
            })
            .await
            .unwrap_or_else(|e| Err(sonata_core::error::CoreError::internal(e.to_string())));
            match save_res {
                Ok(()) => {
                    let message = match ping_res {
                        Ok(msg) => format!("Saved {} ({msg})", profile.name),
                        Err(err) => format!("Saved {} (warning: {err})", profile.name),
                    };
                    let _ = tx.send(AppEvent::ServerSaved { profiles, message });
                }
                Err(err) => {
                    let _ = tx.send(AppEvent::StatusMessage(format!(
                        "Failed to save server: {err}"
                    )));
                }
            }
        });
    }

    pub fn toggle_server_enabled(&mut self, server_id: &str) {
        let mut changed = None;
        if let Some(profile) = self.server_profiles.iter_mut().find(|p| p.id == server_id) {
            profile.enabled = !profile.enabled;
            changed = Some((profile.enabled, profile.name.clone()));
        }
        if let Some((enabled, name)) = changed {
            let list = self.server_profiles.clone();
            let paths = self.paths.clone();
            self.rebuild_sidebar();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tokio::task::spawn_blocking(move || {
                    sonata_core::providers::servers::save_server_profiles(&paths, &list)
                })
                .await;
                let _ = tx.send(AppEvent::StatusMessage(if enabled {
                    format!("Enabled server {name}")
                } else {
                    format!("Disabled server {name}")
                }));
            });
        }
    }

    pub fn remove_server_profile(&mut self, server_id: &str) {
        let before = self.server_profiles.len();
        self.server_profiles.retain(|p| p.id != server_id);
        if self.server_profiles.len() == before {
            return;
        }
        self.rebuild_sidebar();
        let list = self.server_profiles.clone();
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || {
                sonata_core::providers::servers::save_server_profiles(&paths, &list)
            })
            .await;
            let _ = tx.send(AppEvent::StatusMessage(
                "Removed music server profile".to_string(),
            ));
        });
    }

    pub fn open_server_browser(&mut self, profile: sonata_core::providers::servers::ServerProfile) {
        let tab = if profile.kind == sonata_core::providers::servers::ServerKind::WebDav {
            ServerBrowserTab::Libraries
        } else {
            ServerBrowserTab::Tracks
        };
        let state = ServerBrowserState {
            server_id: profile.id.clone(),
            server_name: profile.name.clone(),
            server_kind: profile.kind,
            tab,
            selected: 0,
            folder_path: None,
        };
        self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
        self.current_playlist_id = Some(format!("server:{}", profile.id));
        self.tracks_heading = format!("{} [{}]", profile.name, profile.kind.short_tag());
        self.modal = ActiveModal::ServerBrowser(state);
        self.load_server_browse(profile.id, None, None);
    }

    pub fn load_server_browse(
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
        self.server_loading = true;
        self.server_error = None;
        let client = self.server_client.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if profile.kind == sonata_core::providers::servers::ServerKind::WebDav {
                let raw_folder = folder_path.as_deref().map(|raw| {
                    sonata_core::providers::servers::parse_server_item_id(raw)
                        .map(|r| r.item_id)
                        .unwrap_or_else(|| raw.to_string())
                });
                match sonata_core::providers::servers::webdav::browse_folder(
                    &http,
                    &profile,
                    raw_folder.as_deref(),
                )
                .await
                {
                    Ok(listing) => {
                        let libs = listing
                            .folders
                            .iter()
                            .map(|f| sonata_core::providers::servers::ServerLibrary {
                                id: sonata_core::providers::servers::encode_server_item_id(
                                    sonata_core::providers::servers::ServerKind::WebDav,
                                    &profile.id,
                                    &f.path,
                                ),
                                name: f.name.clone(),
                                kind: "folder".to_string(),
                            })
                            .collect();
                        let folders = listing.folders.into_iter().map(|f| f.path).collect();
                        let _ = tx.send(AppEvent::ServerBrowseLoaded {
                            server_id,
                            folder_path: raw_folder,
                            libraries: libs,
                            artists: Vec::new(),
                            albums: Vec::new(),
                            playlists: Vec::new(),
                            tracks: listing.tracks,
                            folders,
                            error: None,
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::ServerBrowseLoaded {
                            server_id,
                            folder_path: raw_folder,
                            libraries: Vec::new(),
                            artists: Vec::new(),
                            albums: Vec::new(),
                            playlists: Vec::new(),
                            tracks: Vec::new(),
                            folders: Vec::new(),
                            error: Some(err.to_string()),
                        });
                    }
                }
                return;
            }

            let lib_arg = folder_path.as_deref().or(library_id.as_deref());
            let libs_fut = client.list_libraries(&profile);
            let artists_fut = client.list_artists(&profile, lib_arg);
            let albums_fut = client.list_albums(&profile, lib_arg, None);
            let playlists_fut = client.list_playlists(&profile);
            let smb_tracks_fut = async {
                if profile.kind == ServerKind::Smb {
                    client
                        .get_playlist_tracks(&profile, lib_arg.unwrap_or("."))
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            };
            let (libs, artists, albums, playlists, smb_tracks) = tokio::join!(
                libs_fut,
                artists_fut,
                albums_fut,
                playlists_fut,
                smb_tracks_fut
            );
            let error = libs
                .as_ref()
                .err()
                .or(albums.as_ref().err())
                .or(playlists.as_ref().err())
                .map(|e| e.to_string());
            let folders = if profile.kind == ServerKind::Smb {
                libs.as_ref()
                    .map(|items| items.iter().map(|l| l.id.clone()).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let _ = tx.send(AppEvent::ServerBrowseLoaded {
                server_id,
                folder_path: None,
                libraries: libs.unwrap_or_default(),
                artists: artists.unwrap_or_default(),
                albums: albums.unwrap_or_default(),
                playlists: playlists.unwrap_or_default(),
                tracks: smb_tracks,
                folders,
                error,
            });
        });
    }

    pub fn activate_server_browser_item(&mut self) {
        let ActiveModal::ServerBrowser(st) = self.modal.clone() else {
            return;
        };
        let Some(profile) = self
            .server_profiles
            .iter()
            .find(|p| p.id == st.server_id)
            .cloned()
        else {
            return;
        };
        match st.tab {
            ServerBrowserTab::Tracks => {
                if self.server_tracks.is_empty() {
                    return;
                }
                let idx = st.selected.min(self.server_tracks.len().saturating_sub(1));
                self.visible_tracks = self.server_tracks.clone();
                self.track_table_state.select(Some(idx));
                self.modal = ActiveModal::None;
                self.focus = FocusPane::Tracks;
                self.play_selected_track();
            }
            ServerBrowserTab::Libraries => {
                if let Some(lib) = self.server_libraries.get(st.selected).cloned() {
                    if st.server_kind == sonata_core::providers::servers::ServerKind::WebDav {
                        self.load_server_browse(st.server_id, None, Some(lib.id));
                    } else {
                        self.load_server_browse(st.server_id, Some(lib.id), None);
                    }
                }
            }
            ServerBrowserTab::Playlists => {
                let Some(pl) = self.server_playlists.get(st.selected).cloned() else {
                    return;
                };
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                let req_id = self.tracks_req_id;
                let heading = format!("[{}] {}", profile.kind.short_tag(), pl.name);
                self.current_playlist_id = Some(pl.id.clone());
                self.modal = ActiveModal::None;
                self.focus = FocusPane::Tracks;
                self.status_line = format!("Loading {}...", pl.name);
                let client = self.server_client.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let tracks = client
                        .get_playlist_tracks(&profile, &pl.id)
                        .await
                        .unwrap_or_default();
                    let _ = tx.send(AppEvent::TracksLoaded {
                        req_id,
                        playlist_id: Some(pl.id),
                        heading,
                        tracks,
                        silent_replace: false,
                    });
                });
            }
            ServerBrowserTab::Albums => {
                let Some(card) = self.server_albums.get(st.selected).cloned() else {
                    return;
                };
                let album_id = card.id.clone();
                if album_id.is_empty() {
                    return;
                }
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                let req_id = self.tracks_req_id;
                let heading = format!("[{}] {}", profile.kind.short_tag(), card.title);
                self.current_playlist_id = Some(album_id.clone());
                self.modal = ActiveModal::None;
                self.focus = FocusPane::Tracks;
                self.status_line = format!("Loading {}...", card.title);
                let client = self.server_client.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let tracks = client
                        .get_album(&profile, &album_id)
                        .await
                        .map(|a| a.tracks)
                        .unwrap_or_default();
                    let _ = tx.send(AppEvent::TracksLoaded {
                        req_id,
                        playlist_id: Some(album_id),
                        heading,
                        tracks,
                        silent_replace: false,
                    });
                });
            }
            ServerBrowserTab::Artists => {
                let Some(card) = self.server_artists.get(st.selected).cloned() else {
                    return;
                };
                let artist_id = card.id.clone();
                if artist_id.is_empty() {
                    return;
                }
                self.tracks_req_id = self.tracks_req_id.wrapping_add(1);
                let req_id = self.tracks_req_id;
                let heading = format!("[{}] {}", profile.kind.short_tag(), card.title);
                self.current_playlist_id = Some(artist_id.clone());
                self.modal = ActiveModal::None;
                self.focus = FocusPane::Tracks;
                self.status_line = format!("Loading {}...", card.title);
                let client = self.server_client.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let raw_artist_id =
                        sonata_core::providers::servers::parse_server_item_id(&artist_id)
                            .map(|r| r.item_id)
                            .unwrap_or_else(|| artist_id.clone());
                    let albums = client
                        .list_albums(&profile, None, Some(&raw_artist_id))
                        .await
                        .unwrap_or_default();
                    let mut tracks = Vec::new();
                    for album_card in albums.into_iter().take(5) {
                        if !album_card.id.is_empty() {
                            if let Ok(detail) = client.get_album(&profile, &album_card.id).await {
                                tracks.extend(detail.tracks);
                            }
                        }
                    }
                    let _ = tx.send(AppEvent::TracksLoaded {
                        req_id,
                        playlist_id: Some(artist_id),
                        heading,
                        tracks,
                        silent_replace: false,
                    });
                });
            }
        }
    }
}

fn build_tui_server_profile(
    form: &ServerFormState,
) -> sonata_core::providers::servers::ServerProfile {
    let base_url = form.base_url.trim().trim_end_matches('/').to_string();
    let name = if form.name.trim().is_empty() {
        form.kind.display_name().to_string()
    } else {
        form.name.trim().to_string()
    };
    let clean = |s: &str| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    };
    let token = clean(&form.token);
    let slug: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(12)
        .collect();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let id = form.editing_id.clone().unwrap_or_else(|| {
        format!(
            "{}-{}-{ts}",
            form.kind.short_tag().to_lowercase(),
            if slug.is_empty() { "srv" } else { &slug },
        )
    });
    sonata_core::providers::servers::ServerProfile {
        id,
        name,
        kind: form.kind,
        base_url,
        username: clean(&form.username),
        password: clean(&form.password),
        api_key: if matches!(form.kind, ServerKind::Jellyfin | ServerKind::Subsonic) {
            token.clone()
        } else {
            None
        },
        access_token: None,
        user_id: None,
        bearer_token: if form.kind == ServerKind::WebDav {
            token
        } else {
            None
        },
        library_ids: Vec::new(),
        share: clean(&form.share),
        domain: clean(&form.domain),
        local_mount_path: clean(&form.mount_path).map(std::path::PathBuf::from),
        enabled: true,
    }
}

async fn prepare_and_ping_tui_server(
    http: &reqwest::Client,
    profile: &mut sonata_core::providers::servers::ServerProfile,
) -> Result<String, String> {
    if profile.kind == sonata_core::providers::servers::ServerKind::Jellyfin
        && profile.api_key.is_none()
        && profile.access_token.is_none()
    {
        if let (Some(user), Some(pass)) = (profile.username.as_deref(), profile.password.as_deref())
        {
            match sonata_core::providers::servers::jellyfin::authenticate_by_name(
                http,
                &profile.base_url,
                user,
                pass,
                "sonata-tui",
            )
            .await
            {
                Ok(auth) => {
                    profile.access_token = Some(auth.access_token);
                    profile.user_id = Some(auth.user_id);
                }
                Err(err) => return Err(err.to_string()),
            }
        }
    }
    let client = sonata_core::providers::servers::ServerClient::new(http.clone());
    client.ping(profile).await.map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn download_tui_track_into_store(
    store: &sonata_runtime::downloads::DownloadStore,
    resolver: &sonata_runtime::process::LinuxStreamResolver,
    http: &reqwest::Client,
    cache_db: Option<&sonata_core::cache::CacheDb>,
    track: &Track,
    collection: Option<sonata_runtime::downloads::DownloadCollectionRef>,
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
        Some(url) => Some(sonata_core::media::upscale_artwork_url(url)),
        None => sonata_runtime::media::resolve_track_cover(http, cache_db, track)
            .await
            .map(|(url, _)| url),
    };
    let artwork_bytes = if let Some(ref art_url) = artwork_url {
        let mut req = http.get(art_url);
        for (k, v) in &resolved.headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                req = req.header(name, val);
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
    let bitrate_kbps = badge
        .split_whitespace()
        .find_map(|tok| tok.strip_suffix("kbps").and_then(|n| n.parse::<u32>().ok()));

    if resolved.url.starts_with("http://") || resolved.url.starts_with("https://") {
        let mut header_map = reqwest::header::HeaderMap::new();
        for (k, v) in &resolved.headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
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
        let container = infer_tui_container(&resolved.url, content_type.as_deref(), &badge);
        let audio_bytes = resp.bytes().await?.to_vec();
        store
            .save_track_bytes_async(
                sonata_runtime::downloads::DownloadTrackInput {
                    track: track.clone(),
                    artwork_url,
                    audio_badge: badge,
                    container,
                    bitrate_kbps,
                    collection,
                },
                audio_bytes,
                artwork_bytes,
            )
            .await?;
    } else {
        let src_path = std::path::PathBuf::from(&resolved.url);
        let container = infer_tui_container(&resolved.url, None, &badge);
        store
            .save_track_from_file_async(
                sonata_runtime::downloads::DownloadTrackInput {
                    track: track.clone(),
                    artwork_url,
                    audio_badge: badge,
                    container,
                    bitrate_kbps,
                    collection,
                },
                src_path,
                artwork_bytes,
            )
            .await?;
    }
    if let Some(limit) = limit_bytes {
        let _ = store.prune_to_limit_async(limit).await;
    }
    Ok(())
}

fn infer_tui_container(url: &str, content_type: Option<&str>, badge: &str) -> String {
    if let Some(ct) = content_type {
        let low = ct.to_ascii_lowercase();
        if low.contains("flac") {
            return "flac".to_string();
        }
        if low.contains("mpeg") || low.contains("mp3") {
            return "mp3".to_string();
        }
        if low.contains("ogg") || low.contains("opus") {
            return "opus".to_string();
        }
        if low.contains("mp4") || low.contains("aac") || low.contains("m4a") {
            return "m4a".to_string();
        }
    }
    let path_part = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    for ext in [
        "flac", "opus", "ogg", "m4a", "mp4", "mp3", "aac", "wav", "webm",
    ] {
        if path_part.ends_with(&format!(".{ext}")) {
            return ext.to_string();
        }
    }
    if badge.to_ascii_uppercase().contains("FLAC") {
        "flac".to_string()
    } else if badge.to_ascii_uppercase().contains("OPUS") {
        "opus".to_string()
    } else if badge.to_ascii_uppercase().contains("MP3") {
        "mp3".to_string()
    } else {
        "m4a".to_string()
    }
}
