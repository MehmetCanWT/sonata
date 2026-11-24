//! Application coordinator, state container, and event dispatcher for the Sonata terminal UI.
//!
//! 1. **Submodules**: exposes `state`, `playback`, and `library`, re-exporting all state and event types from `state`.
//! 2. **App Container**: `App` owns UI state, provider clients, cache database, the shared `PlaybackSessionHandle`, and client-local media tasks.
//! 3. **Lifecycle & Events**: `App::new`, `start_background_init`, `handle_app_event`, `handle_session_event`, `tick_playback`, and `shutdown_playback_tasks` drive the TUI loop.

pub mod library;
pub mod playback;
pub mod state;

pub use state::*;

use crate::theme::Theme;
use crate::theme_tui::TuiTheme;
use crate::ui::fx::UiEffects;
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use reqwest::Client;
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::{enforce_media_cache_limit_async, start_media_cache_maintenance, CacheDb};
use sonata_core::lyrics::LyricsState;
use sonata_core::paths::CorePaths;
use sonata_core::player::{PlaybackState, PlaybackTaskSet};
use sonata_core::providers::hifi::{
    FastStartMode, ResolverProfile, StreamFallbackMode, UpgradeDelivery,
};
use sonata_core::providers::spotify::SpotifyClient;
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{build_http_client, Playlist, ProviderSource, Track};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::auth::extract_browser_cookies;
use sonata_runtime::config::AppConfig;
use sonata_runtime::hyprland;
use sonata_runtime::player::{
    MpvEvent, PlaybackAudioDriver, PlaybackSession, PlaybackSessionConfig, PlaybackSessionHandle,
    PlaybackStreamSource, SessionCommand, SessionEvent, SessionPolicy,
};
use sonata_runtime::process::LinuxStreamResolver;
use std::collections::VecDeque;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

fn linux_core_paths() -> CorePaths {
    AppConfig::default_core_paths()
}

fn linux_resolver_profile(config: &AppConfig) -> ResolverProfile {
    ResolverProfile {
        fast_start: FastStartMode::BoundedRange,
        upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
        fallback: StreamFallbackMode::ExternalTool,
        bootstrap_timeout: Duration::from_millis(2400),
        jiosaavn_enabled: config.jiosaavn_enabled,
        cache_limit_bytes: config.cache_limit_bytes(),
    }
}

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
    pub lyrics_scroll_row: f32,
    pub lyrics_scroll_target: f32,
    pub lyrics_scroll_vel: f32,
    pub lyrics_scroll_init: bool,
    pub lyrics_last_step: Instant,
    pub lyrics_prev_active: Option<usize>,
    pub lyrics_cur_active: Option<usize>,
    pub lyrics_changed_at: Instant,
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
    pub paths: CorePaths,
    pub cache_limit_bytes: Arc<AtomicU64>,
    pub cookies: ExtractedCookies,
    pub ytm: YtMusicClient,
    pub spotify: SpotifyClient,
    pub resolver: LinuxStreamResolver,
    pub cache_db: Option<CacheDb>,
    pub session: PlaybackSessionHandle,
    pub pending_session_cmds: VecDeque<SessionCommand>,
    pub tx: mpsc::UnboundedSender<AppEvent>,
    pub tracks_req_id: u64,
    pub play_seq: u64,
    pub media_tasks: PlaybackTaskSet,
    pub cover_started_play_seq: Option<u64>,
    pub config_dirty: bool,
    pub last_exported_state: Option<ExportFingerprint>,
    pub fx: UiEffects,
}

impl App {
    pub fn new(
        config: AppConfig,
        picker: Picker,
        tx: mpsc::UnboundedSender<AppEvent>,
        driver: Arc<dyn PlaybackAudioDriver>,
        mpv_rx: mpsc::Receiver<MpvEvent>,
    ) -> (Self, mpsc::Receiver<SessionEvent>) {
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
        let paths = linux_core_paths();
        let cache_limit_bytes = Arc::new(AtomicU64::new(config.cache_limit_bytes()));
        let cookies = ExtractedCookies::default();
        let cache_db = CacheDb::open_with_paths(&paths, config.cache_limit_gb > 0);
        let ytm =
            YtMusicClient::new(http.clone(), cookies.clone()).with_language(config.language.code());
        let spotify = SpotifyClient::with_paths(http.clone(), None, paths.clone());
        let resolver = LinuxStreamResolver::with_profile(
            http.clone(),
            ytm.clone(),
            cookies.clone(),
            cache_db.clone(),
            paths.clone(),
            linux_resolver_profile(&config),
        );
        let mut playback = PlaybackState::new(config.volume);
        playback.shuffle = config.shuffle;
        playback.repeat = config.repeat_mode;

        let mut session_cfg = PlaybackSessionConfig::for_client("tui");
        session_cfg.initial_volume = config.volume;
        session_cfg.initial_shuffle = config.shuffle;
        session_cfg.initial_repeat = config.repeat_mode;
        session_cfg.policy = SessionPolicy {
            hifi_substitution: config.hifi_substitution,
            jiosaavn_enabled: config.jiosaavn_enabled,
            cache_enabled: config.cache_limit_gb > 0,
        };
        let PlaybackSession {
            handle: session,
            events: session_rx,
        } = PlaybackSession::spawn(
            session_cfg,
            driver,
            Arc::new(resolver.clone()) as Arc<dyn PlaybackStreamSource>,
            mpv_rx,
        );

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
            lyrics_scroll_row: 0.0,
            lyrics_scroll_target: 0.0,
            lyrics_scroll_vel: 0.0,
            lyrics_scroll_init: false,
            lyrics_last_step: Instant::now(),
            lyrics_prev_active: None,
            lyrics_cur_active: None,
            lyrics_changed_at: Instant::now(),
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
            fx: UiEffects::new(),
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
            paths,
            cache_limit_bytes,
            cookies,
            ytm,
            spotify,
            resolver,
            cache_db,
            session,
            pending_session_cmds: VecDeque::new(),
            tx,
            tracks_req_id: 0,
            play_seq: 0,
            media_tasks: PlaybackTaskSet::default(),
            cover_started_play_seq: None,
            config_dirty: false,
            last_exported_state: None,
        };
        app.rebuild_sidebar();
        app.sync_appearance();
        (app, session_rx)
    }

    pub(super) fn session_policy(&self) -> SessionPolicy {
        SessionPolicy {
            hifi_substitution: self.config.hifi_substitution,
            jiosaavn_enabled: self.config.jiosaavn_enabled,
            cache_enabled: self.config.cache_limit_gb > 0,
        }
    }

    pub(super) fn enqueue_session_cmd(&mut self, cmd: SessionCommand) {
        self.pending_session_cmds.push_back(cmd);
    }

    /// Delivers all queued transport commands to [`PlaybackSession`] in strict FIFO order without dropping.
    pub async fn flush_session_commands(&mut self) {
        while let Some(cmd) = self.pending_session_cmds.pop_front() {
            if self.session.send(cmd).await.is_err() {
                self.pending_session_cmds.clear();
                break;
            }
        }
    }

    pub fn start_background_init(&self) {
        start_media_cache_maintenance(self.paths.clone(), Arc::clone(&self.cache_limit_bytes));
        let init_paths = self.paths.clone();
        let init_limit = self.config.cache_limit_bytes();
        tokio::spawn(async move {
            enforce_media_cache_limit_async(init_paths, init_limit).await;
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
        let paths = self.paths.clone();
        let lang_code = self.config.language.code();

        tokio::spawn(async move {
            let cookies = tokio::task::spawn_blocking(move || {
                extract_browser_cookies(pref_browser.as_deref())
            })
            .await
            .unwrap_or_default();

            let ytm = YtMusicClient::new(http.clone(), cookies.clone()).with_language(lang_code);
            let spotify = SpotifyClient::with_paths(http, cookies.spotify_sp_dc.clone(), paths);

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
                        is_owned: false,
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
                self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone())
                    .with_language(self.config.language.code());
                self.spotify = SpotifyClient::with_paths(
                    self.http.clone(),
                    self.cookies.spotify_sp_dc.clone(),
                    self.paths.clone(),
                );
                self.resolver = LinuxStreamResolver::with_profile(
                    self.http.clone(),
                    self.ytm.clone(),
                    self.cookies.clone(),
                    self.cache_db.clone(),
                    self.paths.clone(),
                    linux_resolver_profile(&self.config),
                );
                self.enqueue_session_cmd(SessionCommand::UpdateStreamSource(Arc::new(
                    self.resolver.clone(),
                )
                    as Arc<dyn PlaybackStreamSource>));

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
                    self.lyrics_scroll_init = false;
                    self.lyrics_scroll_target = 0.0;
                    self.lyrics_scroll_vel = 0.0;
                    self.lyrics_prev_active = None;
                    self.lyrics_cur_active = None;
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
                    track.thumbnail_url = Some(url.clone());
                }
                self.enqueue_session_cmd(SessionCommand::UpdateTrackThumbnail {
                    track_id: track_id.clone(),
                    url,
                });
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
            AppEvent::PlaylistCreated(pl) => {
                self.status_line = format!("Created YouTube Music playlist: {}", pl.name);
                self.ytm_playlists.push(pl);
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &self.ytm_playlists);
                }
                self.rebuild_sidebar();
            }
            AppEvent::AutoplayTracksReady { play_seq, tracks } => {
                if play_seq != self.play_seq {
                    return;
                }
                if tracks.is_empty() {
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    return;
                }
                let mut combined = self.playback.queue.clone();
                let start_index = combined.len();
                let mut seen: std::collections::HashSet<String> =
                    combined.iter().map(|t| t.id.clone()).collect();
                for tr in tracks {
                    if seen.insert(tr.id.clone()) {
                        combined.push(tr);
                    }
                }
                if start_index >= combined.len() {
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    return;
                }
                self.enqueue_session_cmd(SessionCommand::PlayQueue {
                    tracks: combined,
                    start_index,
                    push_history: false,
                });
                self.status_line = "Autoplay: playing similar tracks".to_string();
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
        if self
            .update_info
            .as_ref()
            .is_some_and(|info| info.update_available)
        {
            let url = self
                .update_info
                .as_ref()
                .map(|info| info.release_url.as_str())
                .filter(|u| !u.is_empty())
                .unwrap_or("https://github.com/MehmetCanWT/sonata/releases");
            let _ = std::process::Command::new("xdg-open")
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            self.status_line = "Opened GitHub Releases in browser".to_string();
            return;
        }
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

    pub fn cycle_language(&mut self, forward: bool) {
        self.config.language = if forward {
            self.config.language.next()
        } else {
            self.config.language.prev()
        };
        let _ = self.config.save();
        let ytm = self.ytm.clone();
        let lang_code = self.config.language.code();
        tokio::spawn(async move {
            ytm.set_language(lang_code).await;
        });
        if let Some(db) = &self.cache_db {
            db.invalidate_playlist(None);
        }
        self.status_line = format!(
            "{}: {}",
            sonata_runtime::i18n::strings(self.config.language).language,
            self.config.language.label()
        );
    }

    pub fn is_lyrics_scrolling(&self) -> bool {
        self.lyrics_scroll_init
            && ((self.lyrics_scroll_row - self.lyrics_scroll_target).abs() > 0.015
                || self.lyrics_scroll_vel.abs() > 0.08)
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
        true
    }

    pub async fn shutdown_playback_tasks(&mut self) {
        if !self
            .media_tasks
            .shutdown(std::time::Duration::from_secs(2))
            .await
        {
            tracing::warn!("tui media tasks exceeded shutdown timeout and were aborted");
        }
        let report = self.session.shutdown().await;
        if !report.tasks_joined_cleanly {
            tracing::warn!("playback session tasks exceeded shutdown timeout and were aborted");
        }
    }
}
