//! Application coordinator, state container, and event dispatcher for the Sonata terminal UI.
//!
//! 1. **Submodules**: exposes `state`, `playback`, and `library`, re-exporting all state and event types from `state`.
//! 2. **App Container**: `App` owns UI state, provider clients, cache database, mpv handle, and background playback tasks.
//! 3. **Lifecycle & Events**: `App::new`, `start_background_init`, `handle_app_event`, `tick_playback`, and `shutdown_playback_tasks` drive the TUI loop.

pub mod library;
pub mod playback;
pub mod state;

pub use state::*;

use crate::theme_tui::TuiTheme;
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use reqwest::Client;
use sonata_core::auth::{extract_browser_cookies, ExtractedCookies};
use sonata_core::cache::{
    enforce_active_media_cache_limit_async, start_media_cache_maintenance, CacheDb,
};
use sonata_core::config::AppConfig;
use sonata_core::hyprland;
use sonata_core::lyrics::LyricsState;
use sonata_core::player::{
    MpvController, NextTrackPrecache, PlaybackState, PlaybackTask, PlaybackTaskSet, PrecacheKey,
};
use sonata_core::providers::hifi::{ResolvedStream, StreamResolver};
use sonata_core::providers::spotify::SpotifyClient;
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{build_http_client, Playlist, ProviderSource, Track};
use sonata_core::theme::Theme;
use sonata_core::updater::UpdateCheckResult;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

pub struct App {
    pub config: AppConfig,
    pub themes: Vec<Theme>,
    pub base_theme: Theme,
    pub theme: TuiTheme,
    pub hyprland_window: Option<String>,
    pub picker: Picker,
    pub album_art: Option<StatefulProtocol>,
    pub album_art_track_id: Option<String>,
    pub kitty_art_png: Option<Vec<u8>>,
    pub anim_art_frames: Vec<(Option<StatefulProtocol>, Vec<u8>)>,
    pub anim_art_idx: usize,
    pub anim_fps: f32,
    pub last_anim_step: Instant,
    pub last_art_rect: Option<Rect>,
    pub kitty_drawn_track: Option<(String, Rect)>,
    pub kitty_motion_uploaded_track: Option<String>,
    pub kitty_motion_count: usize,
    pub kitty_motion_last_drawn: Option<(usize, Rect)>,
    pub kitty_motion_buf_slot: u8,
    pub lyrics: LyricsState,
    pub playback: PlaybackState,
    pub focus: FocusPane,
    pub modal: ActiveModal,
    pub sidebar_items: Vec<SidebarEntry>,
    pub sidebar_idx: usize,
    pub sidebar_state: ListState,
    pub ytm_playlists: Vec<Playlist>,
    pub spotify_playlists: Vec<Playlist>,
    pub imported_playlists: Vec<Playlist>,
    pub visible_tracks: Vec<Track>,
    pub current_playlist_id: Option<String>,
    pub tracks_heading: String,
    pub track_table_state: TableState,
    pub ui_rects: UiRects,
    pub dragging_split: Option<SplitTarget>,
    pub last_track_click: Option<(Instant, usize)>,
    pub last_art_click: Option<Instant>,
    pub mini_mode: bool,
    pub search_query: String,
    pub status_line: String,
    pub browser_label: String,
    pub update_info: Option<UpdateCheckResult>,
    pub should_quit: bool,

    pub http: Client,
    pub cookies: ExtractedCookies,
    pub ytm: YtMusicClient,
    pub spotify: SpotifyClient,
    pub resolver: StreamResolver,
    pub cache_db: Option<CacheDb>,
    pub mpv: Option<Arc<MpvController>>,
    pub tx: mpsc::UnboundedSender<AppEvent>,
    pub pos_anchor: Option<(f64, Instant)>,
    pub tracks_req_id: u64,
    pub play_seq: u64,
    pub retried_play_seq: u64,
    pub playback_tasks: PlaybackTaskSet,
    pub next_precache: NextTrackPrecache,
    pub next_prefetch_task: Option<(PrecacheKey, PlaybackTask<Option<ResolvedStream>>)>,
    pub adopted_prefetch_task: Option<(String, PlaybackTask<Option<ResolvedStream>>)>,
    pub pending_auto_advance: Option<(u64, String)>,
    pub awaiting_playback_start: bool,
    pub upgrade_started_play_seq: Option<u64>,
    pub cover_started_play_seq: Option<u64>,
    pub config_dirty: bool,
    pub last_exported_state: Option<ExportFingerprint>,
    pub play_requested_at: Option<Instant>,
}

impl App {
    pub fn new(config: AppConfig, picker: Picker, tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        let themes = Theme::load_for_tui(&AppConfig::themes_dir());
        let base_theme = themes
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&config.theme))
            .cloned()
            .unwrap_or_else(Theme::catppuccin_mocha);
        let hyprland_window = hyprland::find_terminal_window_address();

        let show_warning =
            picker.protocol_type() == ProtocolType::Halfblocks && !config.suppress_image_warning;
        let modal = if show_warning {
            ActiveModal::ImageWarning(0)
        } else {
            ActiveModal::None
        };

        let http = build_http_client();
        let cookies = ExtractedCookies::default();
        let cache_db = CacheDb::open_default();
        let ytm = YtMusicClient::new(http.clone(), cookies.clone());
        let spotify = SpotifyClient::new(http.clone(), None);
        let resolver =
            StreamResolver::new(http.clone(), ytm.clone(), cookies.clone(), cache_db.clone());
        let mut playback = PlaybackState::new(config.volume);
        playback.shuffle = config.shuffle;
        playback.repeat = config.repeat_mode;

        let sidebar_items = vec![SidebarEntry::Search, SidebarEntry::Queue];
        let mut sidebar_state = ListState::default();
        sidebar_state.select(Some(0));
        let mut track_table_state = TableState::default();
        track_table_state.select(Some(0));

        let ytm_playlists = cache_db
            .as_ref()
            .and_then(|db| db.get_library("ytm"))
            .unwrap_or_default();
        let spotify_playlists = cache_db
            .as_ref()
            .and_then(|db| db.get_library("spf"))
            .unwrap_or_default();
        let imported_playlists = cache_db
            .as_ref()
            .and_then(|db| db.get_library("imported"))
            .unwrap_or_default();

        let mut app = Self {
            config,
            themes,
            theme: TuiTheme::from_core(&base_theme),
            base_theme,
            hyprland_window,
            picker,
            album_art: None,
            album_art_track_id: None,
            kitty_art_png: None,
            anim_art_frames: Vec::new(),
            anim_art_idx: 0,
            anim_fps: 16.0,
            last_anim_step: Instant::now(),
            last_art_rect: None,
            kitty_drawn_track: None,
            kitty_motion_uploaded_track: None,
            kitty_motion_count: 0,
            kitty_motion_last_drawn: None,
            kitty_motion_buf_slot: 0,
            lyrics: LyricsState::Idle,
            playback,
            focus: FocusPane::Sidebar,
            modal,
            sidebar_items,
            sidebar_idx: 0,
            sidebar_state,
            ytm_playlists,
            spotify_playlists,
            imported_playlists,
            visible_tracks: Vec::new(),
            current_playlist_id: None,
            tracks_heading: "Search (press /)".to_string(),
            track_table_state,
            ui_rects: UiRects::default(),
            dragging_split: None,
            last_track_click: None,
            last_art_click: None,
            mini_mode: false,
            search_query: String::new(),
            status_line: "Scanning browser cookies...".to_string(),
            browser_label: "Scanning...".to_string(),
            update_info: None,
            should_quit: false,
            http,
            cookies,
            ytm,
            spotify,
            resolver,
            cache_db,
            mpv: None,
            tx,
            pos_anchor: None,
            tracks_req_id: 0,
            play_seq: 0,
            retried_play_seq: 0,
            playback_tasks: PlaybackTaskSet::default(),
            next_precache: NextTrackPrecache::default(),
            next_prefetch_task: None,
            adopted_prefetch_task: None,
            pending_auto_advance: None,
            awaiting_playback_start: false,
            upgrade_started_play_seq: None,
            cover_started_play_seq: None,
            config_dirty: false,
            last_exported_state: None,
            play_requested_at: None,
        };
        app.rebuild_sidebar();
        app.sync_appearance();
        app
    }

    pub fn start_background_init(&self) {
        start_media_cache_maintenance();
        tokio::spawn(async move {
            enforce_active_media_cache_limit_async().await;
        });

        let resolver = self.resolver.clone();
        tokio::spawn(async move {
            resolver.warm().await;
        });

        Self::spawn_update_check(self.http.clone(), self.tx.clone());

        let tx = self.tx.clone();
        let pref_browser = self.config.preferred_browser.clone();
        let saved_spotify_ids = self.config.spotify_playlists.clone();
        let http = self.http.clone();

        tokio::spawn(async move {
            let cookies = tokio::task::spawn_blocking(move || {
                extract_browser_cookies(pref_browser.as_deref())
            })
            .await
            .unwrap_or_default();

            let ytm = YtMusicClient::new(http.clone(), cookies.clone());
            let spotify = SpotifyClient::new(http, cookies.spotify_sp_dc.clone());

            let (ytm_res, spot_res) = tokio::join!(
                ytm.fetch_library_playlists(),
                spotify.fetch_library_playlists()
            );

            let mut imported_playlists = Vec::new();
            for pid in saved_spotify_ids {
                if let Ok((name, tracks)) = spotify.fetch_embed_playlist(&pid).await {
                    imported_playlists.push(Playlist {
                        id: pid,
                        name,
                        source: ProviderSource::Spotify,
                        track_count: Some(tracks.len()),
                        thumbnail_url: None,
                    });
                }
            }

            let _ = tx.send(AppEvent::AuthReady {
                cookies,
                ytm_playlists: ytm_res.unwrap_or_default(),
                spotify_playlists: spot_res.unwrap_or_default(),
                imported_playlists,
            });
        });
    }

    pub fn handle_app_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::AuthReady {
                cookies,
                ytm_playlists,
                spotify_playlists,
                imported_playlists,
            } => {
                self.browser_label = cookies
                    .browser_name
                    .clone()
                    .unwrap_or_else(|| "Guest (No Browser Login)".to_string());
                self.cookies = cookies;
                self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone());
                self.spotify =
                    SpotifyClient::new(self.http.clone(), self.cookies.spotify_sp_dc.clone());
                self.resolver = StreamResolver::new(
                    self.http.clone(),
                    self.ytm.clone(),
                    self.cookies.clone(),
                    self.cache_db.clone(),
                );

                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &ytm_playlists);
                    db.put_library("spf", &spotify_playlists);
                    db.put_library("imported", &imported_playlists);
                }
                self.ytm_playlists = ytm_playlists;
                self.spotify_playlists = spotify_playlists;
                self.imported_playlists = imported_playlists;
                self.rebuild_sidebar();
                self.status_line = format!(
                    "Ready ({}) — [Mode: {}]",
                    self.browser_label,
                    self.config.provider_mode.label()
                );
            }
            AppEvent::TracksLoaded {
                req_id,
                playlist_id,
                heading,
                tracks,
                silent_replace,
            } => {
                if req_id != self.tracks_req_id {
                    return;
                }
                let prev_sel = self.track_table_state.selected();
                self.current_playlist_id = playlist_id;
                self.tracks_heading = heading;
                self.visible_tracks = tracks;
                if self.visible_tracks.is_empty() {
                    self.track_table_state.select(None);
                } else if silent_replace {
                    let next_sel = prev_sel.map_or(0, |s| s.min(self.visible_tracks.len() - 1));
                    self.track_table_state.select(Some(next_sel));
                } else {
                    self.track_table_state.select(Some(0));
                }
                self.status_line = format!("Loaded {} tracks", self.visible_tracks.len());
            }
            AppEvent::TracksAppended {
                req_id,
                heading,
                tracks,
            } => {
                if req_id == self.tracks_req_id && self.tracks_heading == heading {
                    self.visible_tracks.extend(tracks);
                    self.status_line = format!("Loaded {} tracks", self.visible_tracks.len());
                }
            }
            AppEvent::StreamReady {
                play_seq,
                track_id,
                stream,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                let current_track = self.playback.current_track().cloned();
                if let Some(track) = current_track.filter(|t| t.id == track_id) {
                    tracing::info!(
                        event = "stream_ready",
                        client = "tui",
                        track_id = %track.id,
                        title = %track.title,
                        artist = %track.artist,
                        provider = ?track.source,
                        play_seq,
                        attempt = if self.retried_play_seq == play_seq { "retry" } else { "initial" },
                        transport = if stream.url.starts_with("http") { "remote" } else { "local" },
                        quality = %stream.badge,
                        resolve_ms = self.play_requested_at.map_or(0, |at| at.elapsed().as_millis()),
                        "stream resolved"
                    );
                    self.playback.loading_track = false;
                    self.playback.paused = true;
                    self.awaiting_playback_start = true;
                    let resume_at = if self.retried_play_seq == play_seq {
                        self.playback.position_secs
                    } else {
                        0.0
                    };
                    self.playback.position_secs = resume_at;
                    self.pos_anchor = None;
                    self.playback.set_active_stream(stream.clone());
                    self.playback.stream_badge = stream.badge.clone();
                    self.status_line = format!("Starting: {} — {}", track.title, track.artist);
                    if let Some(mpv) = &self.mpv {
                        if resume_at >= 0.05 {
                            mpv.load_stream_at(&stream, resume_at);
                        } else {
                            mpv.load_stream(&stream);
                        }
                    }
                }
            }
            AppEvent::StreamUpgrade {
                play_seq,
                track_id,
                stream,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                let is_current = self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id);
                if is_current {
                    if !self.playback.set_upgraded_stream(stream.clone()) {
                        return;
                    }
                    let was_paused = self.playback.paused;
                    let pos = self.interpolated_position();
                    self.pos_anchor = Some((pos, Instant::now()));
                    self.playback.position_secs = pos;
                    self.playback.stream_badge = stream.badge.clone();
                    tracing::info!(
                        event = "quality_upgraded",
                        client = "tui",
                        track_id = %track_id,
                        play_seq,
                        quality = %stream.badge,
                        resume_secs = pos,
                        "audio quality upgraded"
                    );
                    if let Some(mpv) = &self.mpv {
                        mpv.load_stream_at(&stream, pos);
                        mpv.set_paused(was_paused);
                    }
                }
            }
            AppEvent::StreamError {
                play_seq,
                track_id,
                error,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if let Some(track) = self
                    .playback
                    .current_track()
                    .filter(|track| track.id == track_id)
                {
                    tracing::error!(
                        event = "resolve_failed",
                        client = "tui",
                        track_id = %track.id,
                        title = %track.title,
                        artist = %track.artist,
                        provider = ?track.source,
                        play_seq,
                        error = %error,
                        "stream resolution failed"
                    );
                    self.playback.paused = true;
                    self.pos_anchor = None;
                    self.playback.loading_track = false;
                    self.awaiting_playback_start = false;
                    self.playback.stream_badge = "Stream Error".to_string();
                    self.status_line = format!("Failed to resolve stream: {error}");
                }
            }
            AppEvent::NextTrackPrepared {
                play_seq,
                index,
                track_id,
                result,
            } => {
                if play_seq != self.play_seq
                    || self.pending_auto_advance.as_ref() != Some(&(play_seq, track_id.clone()))
                {
                    return;
                }
                self.pending_auto_advance = None;
                if !self
                    .playback
                    .queue
                    .get(index)
                    .is_some_and(|track| track.id == track_id)
                {
                    self.playback.loading_track = false;
                    return;
                }
                match result {
                    Ok(stream) => {
                        self.play_queue_index_with_stream(index, true, Some(stream));
                    }
                    Err(error) => {
                        self.playback.loading_track = false;
                        self.playback.paused = true;
                        self.status_line = format!("Could not prepare next track: {error}");
                    }
                }
            }
            AppEvent::LyricsReady {
                play_seq,
                track_id,
                lyrics,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if let Some(track) = self.playback.current_track().filter(|t| t.id == track_id) {
                    if let LyricsState::Synced(ref lines) = lyrics {
                        if !sonata_core::lyrics::synced_lyrics_match_duration(
                            lines,
                            track.duration_secs,
                        ) {
                            return;
                        }
                    }
                    self.lyrics = lyrics;
                }
            }
            AppEvent::ArtworkReady {
                play_seq,
                track_id,
                url,
                image,
                png_bytes,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if self.playback.current_track().is_some_and(|track| {
                    track.id == track_id
                        && track.thumbnail_url.as_deref().is_some_and(|orig| {
                            orig == url || sonata_core::media::upscale_artwork_url(orig) == url
                        })
                }) {
                    self.album_art = Some(self.picker.new_resize_protocol(image));
                    self.kitty_art_png = Some(png_bytes);
                    self.kitty_drawn_track = None;
                    self.album_art_track_id = Some(track_id);
                }
            }
            AppEvent::ArtworkEnhanced {
                play_seq,
                track_id,
                url,
                image,
                png_bytes,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if let Some(track) = self
                    .playback
                    .queue
                    .iter_mut()
                    .find(|track| track.id == track_id)
                {
                    track.thumbnail_url = Some(url);
                }
                if self
                    .playback
                    .current_track()
                    .is_some_and(|track| track.id == track_id)
                {
                    self.album_art = Some(self.picker.new_resize_protocol(image));
                    self.kitty_art_png = Some(png_bytes);
                    self.kitty_drawn_track = None;
                    self.album_art_track_id = Some(track_id);
                }
            }
            AppEvent::AnimatedArtworkReady {
                play_seq,
                track_id,
                fps,
                frames,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if self
                    .playback
                    .current_track()
                    .is_some_and(|track| track.id == track_id)
                    && !frames.is_empty()
                {
                    self.anim_art_frames = frames;
                    self.anim_art_idx = 0;
                    self.anim_fps = fps.clamp(8.0, 24.0);
                    self.last_anim_step = Instant::now();
                    self.kitty_motion_uploaded_track = None;
                    self.kitty_motion_count = 0;
                    self.kitty_motion_last_drawn = None;
                    self.kitty_motion_buf_slot = 0;
                    self.album_art_track_id = Some(track_id);
                }
            }
            AppEvent::Mpv(mev) => {
                self.handle_mpv_event(mev);
            }
            AppEvent::PlaylistCreated(pl) => {
                self.status_line = format!("Created YouTube Music playlist: {}", pl.name);
                self.ytm_playlists.push(pl);
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &self.ytm_playlists);
                }
                self.rebuild_sidebar();
            }
            AppEvent::UpdateChecked(info) => {
                if info.update_available {
                    if self.status_line == "Checking for updates..." {
                        self.status_line =
                            "Update available — see details in Settings (S)".to_string();
                    }
                } else if self.status_line == "Checking for updates..." {
                    self.status_line = "Sonata is up to date".to_string();
                }
                self.update_info = Some(info);
            }
            AppEvent::StatusMessage(msg) => {
                self.status_line = msg;
            }
        }
    }

    pub fn check_for_updates(&mut self) {
        self.status_line = "Checking for updates...".to_string();
        Self::spawn_update_check(self.http.clone(), self.tx.clone());
    }

    fn spawn_update_check(http: Client, tx: mpsc::UnboundedSender<AppEvent>) {
        tokio::spawn(async move {
            match sonata_core::updater::check_for_updates_for(
                &http,
                sonata_core::updater::UpdateTarget::Tui,
                sonata_core::updater::CURRENT_VERSION,
            )
            .await
            {
                Ok(info) => {
                    let _ = tx.send(AppEvent::UpdateChecked(info));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "tui update check failed");
                }
            }
        });
    }

    pub fn tick_playback(&mut self) -> bool {
        if self.playback.paused
            || self.playback.loading_track
            || self.playback.current_track().is_none()
        {
            return false;
        }
        self.playback.position_secs = self.interpolated_position();
        let step_interval =
            std::time::Duration::from_secs_f32(1.0 / self.anim_fps.clamp(8.0, 24.0));
        if !self.anim_art_frames.is_empty()
            && self.config.motion_covers
            && !self.config.reduce_motion
            && self.last_anim_step.elapsed() >= step_interval
        {
            self.anim_art_idx = (self.anim_art_idx + 1) % self.anim_art_frames.len();
            self.last_anim_step = Instant::now();
        }
        self.poll_adopted_precache();
        self.maybe_precache_next();
        true
    }

    pub async fn shutdown_playback_tasks(&mut self) {
        if let Some((_, task)) = self.next_prefetch_task.take() {
            task.abort();
        }
        if let Some((_, task)) = self.adopted_prefetch_task.take() {
            task.abort();
        }
        self.next_precache.take();
        if !self
            .playback_tasks
            .shutdown(std::time::Duration::from_secs(2))
            .await
        {
            tracing::warn!("playback tasks exceeded shutdown timeout and were aborted");
        }
    }
}
