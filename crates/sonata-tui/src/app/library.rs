//! Library browsing, search, playlist mutations, and cache/account management for the Sonata TUI.
//!
//! 1. **Search & Browse**: provider-mode switching, song search across YouTube Music and Spotify, and progressive playlist loading with cache backing.
//! 2. **Library Mutations**: liking/unliking tracks, creating YouTube Music playlists, adding/removing playlist items, and importing Spotify playlists.
//! 3. **Cache & Account**: adjusts active media cache limits and refreshes browser session cookies and playlists.

use super::{ActiveModal, App, AppEvent, FocusPane, SidebarEntry};
use sonata_core::cache::enforce_active_media_cache_limit_async;
use sonata_core::config::{AppConfig, ProviderMode};
use sonata_core::providers::spotify::extract_playlist_id;
use sonata_core::providers::{Playlist, ProviderSource, Track};

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
        if let Some(cache) = &self.cache_db {
            cache.set_enabled(self.config.cache_limit_gb > 0);
        }
        tokio::spawn(async move {
            enforce_active_media_cache_limit_async().await;
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
        let mode = self.config.provider_mode;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut combined = Vec::new();
            match mode {
                ProviderMode::Both => {
                    let (ytm_tracks, spot_tracks) =
                        tokio::join!(ytm.search_songs(&q), spotify.search_tracks(&q));
                    combined.extend(ytm_tracks.unwrap_or_default());
                    combined.extend(spot_tracks.unwrap_or_default());
                }
                ProviderMode::YouTubeMusic => {
                    combined.extend(ytm.search_songs(&q).await.unwrap_or_default());
                }
                ProviderMode::Spotify => {
                    combined.extend(spotify.search_tracks(&q).await.unwrap_or_default());
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
            SidebarEntry::Playlist(pl) => {
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
                    if let Some((cached_name, cached_tracks, is_fresh)) =
                        cache_db.as_ref().and_then(|db| db.get_playlist(&db_key))
                    {
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
                            if let Some(db) = &cache_db {
                                db.put_playlist(&db_key, &cached_name, &all_tracks);
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

    pub fn writable_ytm_playlists(&self) -> Vec<Playlist> {
        self.sidebar_items
            .iter()
            .filter_map(|entry| match entry {
                SidebarEntry::Playlist(p)
                    if p.source == ProviderSource::YouTubeMusic && p.id != "VLLM" =>
                {
                    Some(p.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn toggle_like_selected(&mut self) {
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music in your browser to sync likes".to_string();
            return;
        }
        let Some(track) = self.selected_or_playing_track() else {
            return;
        };
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
            let video_id = match track.source {
                ProviderSource::YouTubeMusic => Some(track.id),
                ProviderSource::Spotify => {
                    let q = format!("{} {}", track.artist, track.title);
                    ytm.search_songs(&q)
                        .await
                        .ok()
                        .and_then(|v| v.into_iter().next())
                        .map(|t| t.id)
                }
            };
            let Some(vid) = video_id else {
                let _ = tx.send(AppEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.set_like_status(&vid, new_liked).await {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some("YTM:VLLM"));
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
        if !self.ytm.is_authenticated() {
            self.status_line = "Sign in to YouTube Music in your browser first".to_string();
            return;
        }
        if self.selected_or_playing_track().is_none() {
            self.status_line = "Select a track first".to_string();
            return;
        }
        if self.writable_ytm_playlists().is_empty() {
            self.status_line =
                "No writable YouTube Music playlists found (press c to create)".to_string();
            return;
        }
        self.modal = ActiveModal::AddToPlaylist(0);
    }

    pub fn confirm_add_to_playlist(&mut self, pl_idx: usize) {
        let playlists = self.writable_ytm_playlists();
        let Some(pl) = playlists.get(pl_idx).cloned() else {
            return;
        };
        let Some(track) = self.selected_or_playing_track() else {
            return;
        };
        let ytm = self.ytm.clone();
        let cache_db = self.cache_db.clone();
        let tx = self.tx.clone();
        self.status_line = format!("Adding \"{}\" to {}...", track.title, pl.name);

        tokio::spawn(async move {
            let video_id = match track.source {
                ProviderSource::YouTubeMusic => Some(track.id),
                ProviderSource::Spotify => {
                    let q = format!("{} {}", track.artist, track.title);
                    ytm.search_songs(&q)
                        .await
                        .ok()
                        .and_then(|v| v.into_iter().next())
                        .map(|t| t.id)
                }
            };
            let Some(vid) = video_id else {
                let _ = tx.send(AppEvent::StatusMessage(
                    "Could not match track on YouTube Music".to_string(),
                ));
                return;
            };
            match ytm.add_to_playlist(&pl.id, &vid).await {
                Ok(()) => {
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some(&format!("YTM:{}", pl.id)));
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
            self.status_line = "Open a YouTube Music playlist to remove tracks".to_string();
            return;
        };
        if pl_id == "VLLM" {
            self.toggle_like_selected();
            return;
        }
        let Some(idx) = self.track_table_state.selected() else {
            return;
        };
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
                    if let Some(db) = &cache_db {
                        db.invalidate_playlist(Some(&format!("YTM:{pl_id}")));
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

    pub fn refresh_account(&mut self) {
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
}
