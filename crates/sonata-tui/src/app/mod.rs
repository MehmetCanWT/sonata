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
use sonata_core::connect::ConnectCommand;
use sonata_core::lyrics::LyricsState;
use sonata_core::paths::CorePaths;
use sonata_core::player::{PlaybackState, PlaybackTaskSet};
use sonata_core::providers::hifi::{
    FastStartMode, ResolverProfile, StreamFallbackMode, UpgradeDelivery,
};
use sonata_core::providers::spotify::SpotifyClient;
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{build_http_client, Playlist, ProviderSource, Track};
use sonata_core::redaction::sensitive_text;
use sonata_core::scrobble::{ScrobbleClient, ScrobbleConfig, ScrobbleTracker};
use sonata_core::stats::{StatsDb, StatsSummary, StatsWindow};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::auth::extract_browser_cookies;
use sonata_runtime::config::{sanitize_crossfade_secs, AppConfig};
use sonata_runtime::connect::{ConnectHandle, ConnectServerConfig, PairedDeviceInfo};
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
        jiosaavn_enabled: config.effective_jiosaavn_enabled(),
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
    pub download_store: Option<sonata_runtime::downloads::DownloadStore>,
    pub downloaded_tracks: Vec<sonata_runtime::downloads::DownloadedTrack>,
    pub downloaded_collections: Vec<sonata_runtime::downloads::DownloadedCollection>,
    pub downloaded_track_ids: std::collections::HashSet<String>,
    pub downloading_track_ids: std::collections::HashSet<String>,
    pub downloads_total_bytes: u64,
    pub discord_rpc: sonata_runtime::discord::DiscordRpcHandle,
    pub playback_speed: f64,
    pub skip_silence: bool,
    pub video_mode: bool,
    pub sleep_timer_deadline: Option<Instant>,
    pub translated_lyrics: Option<Vec<String>>,
    pub romanized_lyrics: Option<Vec<String>>,
    pub server_client: sonata_core::providers::servers::ServerClient,
    pub server_profiles: Vec<sonata_core::providers::servers::ServerProfile>,
    pub server_statuses: std::collections::HashMap<String, String>,
    pub server_libraries: Vec<sonata_core::providers::servers::ServerLibrary>,
    pub server_artists: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub server_albums: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub server_playlists: Vec<Playlist>,
    pub server_tracks: Vec<Track>,
    pub server_folders: Vec<String>,
    pub server_loading: bool,
    pub server_error: Option<String>,
    pub active_source: TuiLibrarySource,
    pub personal_playlists: Vec<sonata_core::playlists::PersonalPlaylist>,
    pub local_scan: sonata_core::providers::local::LocalScanResult,
    pub local_scanning: bool,
    pub server_health:
        std::collections::HashMap<String, sonata_core::providers::servers::ServerHealthReport>,
    pub server_speed_tests:
        std::collections::HashMap<String, sonata_core::providers::servers::ServerSpeedTestReport>,
    pub server_speed_testing: std::collections::HashSet<String>,
    pub scrobble_config: ScrobbleConfig,
    pub scrobble_client: ScrobbleClient,
    pub scrobble_tracker: Option<ScrobbleTracker>,
    pub scrobble_last_error: Option<String>,
    pub active_listen_session: Option<TuiListenSession>,
    pub stats_db: Option<StatsDb>,
    pub stats_window: StatsWindow,
    pub stats_summary: StatsSummary,
    pub stats_loading: bool,
    pub connect_handle: Option<ConnectHandle>,
    pub connect_cmd_rx: Option<mpsc::Receiver<ConnectCommand>>,
    pub connect_sync_needed: bool,
    pub connect_pin: Option<String>,
    pub connect_http_port: Option<u16>,
    pub connect_paired_devices: Vec<PairedDeviceInfo>,
    pub connect_error: Option<String>,
    pub pending_connect_seek: Option<(String, f64)>,
    pub last_connect_publish: Instant,
}

pub struct TuiListenSession {
    pub play_seq: u64,
    pub track: Track,
    pub started_at_unix: u64,
    pub accumulated_secs: f64,
    pub last_tick: Option<Instant>,
    pub stats_recorded: bool,
}

pub async fn recv_connect_cmd(
    rx: &mut Option<mpsc::Receiver<ConnectCommand>>,
) -> Option<ConnectCommand> {
    match rx {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
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
        let download_store =
            sonata_runtime::downloads::DownloadStore::open(config.effective_downloads_dir()).ok();
        let discord_rpc =
            sonata_runtime::discord::DiscordRpcHandle::spawn(Self::discord_config_from(&config));
        let server_client = sonata_core::providers::servers::ServerClient::new(http.clone());
        let server_profiles =
            sonata_core::providers::servers::load_server_profiles(&paths).unwrap_or_default();
        let personal_playlists = sonata_core::playlists::PersonalPlaylistStore::new(&paths)
            .list()
            .unwrap_or_default();
        let active_source = compute_tui_library_sources(
            config.streaming_enabled(),
            config.local_music_enabled,
            &server_profiles,
        )
        .into_iter()
        .next()
        .unwrap_or(TuiLibrarySource::Local);
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
        if let Some(ref store) = download_store {
            resolver.set_downloads_store(Some(store.clone()));
        }
        let mut playback = PlaybackState::new(config.volume);
        playback.shuffle = config.shuffle;
        playback.repeat = config.repeat_mode;

        let mut session_cfg = PlaybackSessionConfig::for_client("tui");
        session_cfg.initial_volume = config.volume;
        session_cfg.initial_shuffle = config.shuffle;
        session_cfg.initial_repeat = config.repeat_mode;
        session_cfg.initial_gapless = config.gapless_enabled;
        session_cfg.initial_crossfade_secs = sanitize_crossfade_secs(config.crossfade_secs);
        session_cfg.policy = SessionPolicy {
            hifi_substitution: config.hifi_substitution,
            jiosaavn_enabled: config.effective_jiosaavn_enabled(),
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

        let initial_speed = sonata_runtime::config::sanitize_playback_speed(config.playback_speed);
        let initial_skip_silence = config.skip_silence;
        let initial_gapless = config.gapless_enabled;
        let scrobble_config = ScrobbleConfig::load_from_paths(&paths).unwrap_or_default();
        let scrobble_client = ScrobbleClient::new();
        let connect_sync_needed = config.connect_enabled;
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
            download_store,
            downloaded_tracks: Vec::new(),
            downloaded_collections: Vec::new(),
            downloaded_track_ids: std::collections::HashSet::new(),
            downloading_track_ids: std::collections::HashSet::new(),
            downloads_total_bytes: 0,
            discord_rpc,
            playback_speed: initial_speed,
            skip_silence: initial_skip_silence,
            video_mode: false,
            sleep_timer_deadline: None,
            translated_lyrics: None,
            romanized_lyrics: None,
            server_client,
            server_profiles,
            server_statuses: std::collections::HashMap::new(),
            server_libraries: Vec::new(),
            server_artists: Vec::new(),
            server_albums: Vec::new(),
            server_playlists: Vec::new(),
            server_tracks: Vec::new(),
            server_folders: Vec::new(),
            server_loading: false,
            server_error: None,
            active_source,
            personal_playlists,
            local_scan: sonata_core::providers::local::LocalScanResult::default(),
            local_scanning: false,
            server_health: std::collections::HashMap::new(),
            server_speed_tests: std::collections::HashMap::new(),
            server_speed_testing: std::collections::HashSet::new(),
            scrobble_config,
            scrobble_client,
            scrobble_tracker: None,
            scrobble_last_error: None,
            active_listen_session: None,
            stats_db: None,
            stats_window: StatsWindow::Week,
            stats_summary: StatsSummary::default(),
            stats_loading: true,
            connect_handle: None,
            connect_cmd_rx: None,
            connect_sync_needed,
            connect_pin: None,
            connect_http_port: None,
            connect_paired_devices: Vec::new(),
            connect_error: None,
            pending_connect_seek: None,
            last_connect_publish: Instant::now(),
        };
        app.rebuild_sidebar();
        app.refresh_downloads();
        app.sync_appearance();
        if app.config.local_music_enabled {
            app.scan_local_music();
        }
        app.probe_all_server_health();
        if (initial_speed - sonata_runtime::config::DEFAULT_PLAYBACK_SPEED).abs() > 1e-6 {
            app.enqueue_session_cmd(SessionCommand::SetPlaybackSpeed(initial_speed));
        }
        if initial_skip_silence {
            app.enqueue_session_cmd(SessionCommand::SetSkipSilence(true));
        }
        if !initial_gapless {
            app.enqueue_session_cmd(SessionCommand::SetGapless(false));
        }
        (app, session_rx)
    }

    pub(super) fn session_policy(&self) -> SessionPolicy {
        SessionPolicy {
            hifi_substitution: self.config.hifi_substitution,
            jiosaavn_enabled: self.config.effective_jiosaavn_enabled(),
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

        let stats_paths = self.paths.clone();
        let stats_tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let db = match StatsDb::open_with_paths(&stats_paths) {
                Ok(db) => Some(db),
                Err(err) => {
                    tracing::warn!(client = "tui", error = %err, "failed to open stats.db");
                    None
                }
            };
            let _ = stats_tx.send(AppEvent::StatsDbOpened(db));
        });

        Self::spawn_update_check(self.http.clone(), self.tx.clone());

        if self.config.personal_only_mode {
            let _ = self.tx.send(AppEvent::StatusMessage(
                "Personal Only Mode active (streaming paused)".to_string(),
            ));
            return;
        }

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
                let resolver = LinuxStreamResolver::with_profile(
                    self.http.clone(),
                    self.ytm.clone(),
                    self.cookies.clone(),
                    self.cache_db.clone(),
                    self.paths.clone(),
                    linux_resolver_profile(&self.config),
                );
                if let Some(ref store) = self.download_store {
                    resolver.set_downloads_store(Some(store.clone()));
                }
                self.resolver = resolver;
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
                    self.translated_lyrics = None;
                    self.romanized_lyrics = None;
                    self.lyrics_scroll_init = false;
                    self.lyrics_scroll_target = 0.0;
                    self.lyrics_scroll_vel = 0.0;
                    self.lyrics_prev_active = None;
                    self.lyrics_cur_active = None;
                    self.refresh_lyrics_transforms();
                }
            }
            AppEvent::LyricsTranslated {
                play_seq,
                track_id,
                target_lang,
                lines,
            } => {
                if play_seq == self.play_seq
                    && self.config.lyrics_translate
                    && self.config.language.code() == target_lang
                    && self
                        .playback
                        .current_track()
                        .is_some_and(|t| t.id == track_id)
                {
                    self.translated_lyrics = lines;
                }
            }
            AppEvent::LyricsRomanized {
                play_seq,
                track_id,
                lines,
            } => {
                if play_seq == self.play_seq
                    && self.config.lyrics_romanize
                    && self
                        .playback
                        .current_track()
                        .is_some_and(|t| t.id == track_id)
                {
                    self.romanized_lyrics = lines;
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
                self.sync_discord_rpc();
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
            AppEvent::DownloadsLoaded {
                tracks,
                collections,
                total_bytes,
            } => {
                self.downloaded_track_ids = tracks.iter().map(|t| t.track_id.clone()).collect();
                self.downloaded_tracks = tracks;
                self.downloaded_collections = collections;
                self.downloads_total_bytes = total_bytes;
                if matches!(
                    self.sidebar_items.get(self.sidebar_idx),
                    Some(SidebarEntry::Downloads)
                ) && self.tracks_heading.starts_with("Downloads")
                {
                    self.visible_tracks = self
                        .downloaded_tracks
                        .iter()
                        .map(|d| d.to_track())
                        .collect();
                }
                self.rebuild_sidebar();
            }
            AppEvent::TrackDownloadFinished {
                track_id,
                title,
                result,
            } => {
                self.downloading_track_ids.remove(&track_id);
                match result {
                    Ok(()) => {
                        self.status_line = format!("Downloaded \"{title}\" for offline playback");
                        self.refresh_downloads();
                    }
                    Err(err) => {
                        self.status_line = format!("Download failed for \"{title}\": {err}");
                    }
                }
            }
            AppEvent::CollectionDownloadFinished {
                title,
                saved,
                errors,
            } => {
                self.downloading_track_ids.clear();
                self.status_line = if errors == 0 {
                    format!("Downloaded {saved} tracks from \"{title}\"")
                } else {
                    format!("Downloaded {saved} tracks from \"{title}\" ({errors} failed)")
                };
                self.refresh_downloads();
            }
            AppEvent::ServerTested { server_id, result } => {
                match result {
                    Ok(summary) => {
                        self.server_statuses.insert(server_id, summary.clone());
                        self.status_line = format!("Server connected: {summary}");
                    }
                    Err(err) => {
                        self.server_statuses
                            .insert(server_id, format!("Error: {err}"));
                        self.status_line = format!("Server connection failed: {err}");
                    }
                }
                if let ActiveModal::AddServer(ref mut form) = self.modal {
                    form.status = Some(self.status_line.clone());
                }
            }
            AppEvent::ServerSaved { profiles, message } => {
                self.server_profiles = profiles;
                self.rebuild_sidebar();
                self.probe_all_server_health();
                self.status_line = message;
            }
            AppEvent::ServerHealthProbed { server_id, report } => {
                self.server_statuses
                    .insert(server_id.clone(), format_tui_health_badge(&report));
                self.server_health.insert(server_id, report);
            }
            AppEvent::ServerSpeedTestFinished { server_id, report } => {
                self.server_speed_testing.remove(&server_id);
                let badge = format_tui_speed_badge(&report);
                self.server_speed_tests.insert(server_id, report);
                self.status_line = format!("Speed test: {badge}");
            }
            AppEvent::LocalMusicScanned(scan) => {
                self.local_scanning = false;
                let count = scan.tracks.len();
                let albums = scan.albums.len();
                self.local_scan = scan;
                self.rebuild_sidebar();
                if matches!(
                    self.sidebar_items.get(self.sidebar_idx),
                    Some(SidebarEntry::LocalSongs)
                ) {
                    self.visible_tracks = self.local_scan.tracks.clone();
                }
                self.status_line = format!("Local Music: {count} tracks across {albums} albums");
            }
            AppEvent::ServerBrowseLoaded {
                server_id,
                folder_path,
                libraries,
                artists,
                albums,
                playlists,
                tracks,
                folders,
                error,
            } => {
                if let ActiveModal::ServerBrowser(ref mut st) = self.modal {
                    if st.server_id != server_id {
                        return;
                    }
                    st.folder_path = folder_path;
                    st.selected = 0;
                }
                self.server_loading = false;
                self.server_error = error.clone();
                self.server_libraries = libraries;
                self.server_artists = artists;
                self.server_albums = albums;
                self.server_playlists = playlists;
                self.server_tracks = tracks.clone();
                self.server_folders = folders;
                if let Some(err) = error {
                    self.status_line = format!("Server error: {err}");
                } else if !tracks.is_empty() {
                    self.visible_tracks = tracks;
                    self.track_table_state.select(Some(0));
                }
            }
            AppEvent::StatsDbOpened(db_opt) => {
                self.stats_db = db_opt;
                if self.stats_db.is_some() {
                    self.refresh_listening_stats(self.stats_window);
                } else {
                    self.stats_loading = false;
                }
            }
            AppEvent::StatsLoaded { window, summary } => {
                if window == self.stats_window {
                    self.stats_summary = summary;
                    self.stats_loading = false;
                }
            }
            AppEvent::ScrobbleError(err) => {
                self.scrobble_last_error = err;
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
        if self.config.lyrics_translate {
            self.translated_lyrics = None;
            self.refresh_lyrics_transforms();
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
        self.advance_listen_tracking();
        if self.last_connect_publish.elapsed() >= Duration::from_secs(1) {
            self.publish_connect_snapshot();
        }
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
        self.finalize_listen_stats();
        if let Some(handle) = self.connect_handle.take() {
            handle.shutdown();
        }
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
        self.discord_rpc.shutdown().await;
    }

    pub fn refresh_listening_stats(&mut self, window: StatsWindow) {
        self.stats_window = window;
        let Some(db) = self.stats_db.clone() else {
            self.stats_loading = false;
            return;
        };
        self.stats_loading = true;
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let summary = db.query_summary(window).unwrap_or_default();
            let _ = tx.send(AppEvent::StatsLoaded { window, summary });
        });
    }

    pub fn save_lastfm_form(&mut self, form: &LastFmFormState) {
        self.scrobble_config.lastfm.enabled = form.enabled;
        self.scrobble_config.lastfm.username = form.username.trim().to_string();
        self.scrobble_config.lastfm.api_key = form.api_key.trim().to_string();
        self.scrobble_config.lastfm.api_secret = form.api_secret.trim().to_string();
        self.scrobble_config.lastfm.session_key = form.session_key.trim().to_string();
        self.persist_scrobble_config("Last.fm settings saved");
    }

    pub fn save_listenbrainz_form(&mut self, form: &ListenBrainzFormState) {
        self.scrobble_config.listenbrainz.enabled = form.enabled;
        self.scrobble_config.listenbrainz.username = form.username.trim().to_string();
        self.scrobble_config.listenbrainz.token = form.token.trim().to_string();
        self.persist_scrobble_config("ListenBrainz settings saved");
    }

    pub fn toggle_lastfm_enabled(&mut self) {
        self.scrobble_config.lastfm.enabled = !self.scrobble_config.lastfm.enabled;
        let s = sonata_runtime::i18n::strings(self.config.language);
        let msg = format!(
            "{}: {}",
            s.lastfm_scrobbling,
            if self.scrobble_config.lastfm.enabled {
                s.enabled
            } else {
                s.disabled
            }
        );
        self.persist_scrobble_config(&msg);
    }

    pub fn toggle_listenbrainz_enabled(&mut self) {
        self.scrobble_config.listenbrainz.enabled = !self.scrobble_config.listenbrainz.enabled;
        let s = sonata_runtime::i18n::strings(self.config.language);
        let msg = format!(
            "{}: {}",
            s.listenbrainz_scrobbling,
            if self.scrobble_config.listenbrainz.enabled {
                s.enabled
            } else {
                s.disabled
            }
        );
        self.persist_scrobble_config(&msg);
    }

    fn persist_scrobble_config(&mut self, ok_msg: &str) {
        match self.scrobble_config.save_to_paths(&self.paths) {
            Ok(()) => {
                self.scrobble_last_error = None;
                self.status_line = ok_msg.to_string();
            }
            Err(err) => {
                let safe = sensitive_text(&err.to_string());
                self.scrobble_last_error = Some(safe.clone());
                self.status_line = format!("Failed to save scrobble config: {safe}");
            }
        }
    }

    pub fn toggle_connect(&mut self) {
        self.config.connect_enabled = !self.config.connect_enabled;
        let _ = self.config.save();
        self.connect_sync_needed = true;
        let s = sonata_runtime::i18n::strings(self.config.language);
        self.status_line = format!(
            "{}: {}",
            s.sonata_connect,
            if self.config.connect_enabled {
                s.enabled
            } else {
                s.disabled
            }
        );
    }

    pub fn set_connect_device_name(&mut self, name: &str) {
        let trimmed = name.trim();
        self.config.connect_device_name = if trimmed.is_empty() {
            sonata_runtime::config::DEFAULT_CONNECT_DEVICE_NAME.to_string()
        } else {
            trimmed.to_string()
        };
        let _ = self.config.save();
        if self.config.connect_enabled {
            self.connect_sync_needed = true;
        }
        self.status_line = format!(
            "Sonata Connect device: {}",
            self.config.effective_connect_device_name()
        );
    }

    pub fn regenerate_connect_pin(&mut self) {
        if let Some(ref handle) = self.connect_handle {
            self.connect_pin = Some(handle.regenerate_pin());
            self.status_line = "Sonata Connect pairing PIN refreshed".to_string();
        }
    }

    pub fn unpair_connect_device(&mut self, device_id: &str) {
        if let Some(ref handle) = self.connect_handle {
            match handle.unpair_device(device_id) {
                Ok(removed) => {
                    self.connect_paired_devices = handle.paired_devices();
                    if removed {
                        self.status_line = "Paired device removed".to_string();
                    }
                }
                Err(err) => {
                    let safe = sensitive_text(&err.to_string());
                    self.connect_error = Some(safe.clone());
                    self.status_line = format!("Unpair failed: {safe}");
                }
            }
        }
    }

    pub async fn sync_connect_server(&mut self) {
        self.connect_sync_needed = false;
        if let Some(prev) = self.connect_handle.take() {
            prev.shutdown();
        }
        self.connect_cmd_rx = None;
        self.connect_pin = None;
        self.connect_http_port = None;
        self.connect_error = None;
        if !self.config.connect_enabled {
            self.connect_paired_devices.clear();
            return;
        }
        let cfg = ConnectServerConfig {
            device_name: self.config.effective_connect_device_name().to_string(),
            paired_store_path: self.paths.connect_paired_path(),
            ..ConnectServerConfig::default()
        };
        match ConnectHandle::start(cfg).await {
            Ok((handle, cmd_rx)) => {
                self.connect_pin = Some(handle.current_pin());
                self.connect_http_port = Some(handle.http_port());
                self.connect_paired_devices = handle.paired_devices();
                self.connect_handle = Some(handle);
                self.connect_cmd_rx = Some(cmd_rx);
                self.publish_connect_snapshot();
            }
            Err(err) => {
                let safe = sensitive_text(&err.to_string());
                tracing::warn!(client = "tui", error = %safe, "failed to start Sonata Connect");
                self.connect_error = Some(safe);
            }
        }
    }

    pub fn publish_connect_snapshot(&mut self) {
        let Some(ref handle) = self.connect_handle else {
            return;
        };
        self.last_connect_publish = Instant::now();
        self.connect_paired_devices = handle.paired_devices();
        self.connect_pin = Some(handle.current_pin());
        let snapshot =
            build_tui_connect_snapshot(self.config.effective_connect_device_name(), &self.playback);
        handle.update_state(snapshot);
    }

    pub fn handle_connect_command(&mut self, cmd: ConnectCommand) {
        match cmd {
            ConnectCommand::Play => {
                self.enqueue_session_cmd(SessionCommand::SetPaused(false));
            }
            ConnectCommand::Pause => {
                self.enqueue_session_cmd(SessionCommand::SetPaused(true));
            }
            ConnectCommand::Next => {
                self.enqueue_session_cmd(SessionCommand::Next);
            }
            ConnectCommand::Prev => {
                self.enqueue_session_cmd(SessionCommand::Previous {
                    restart_if_after_secs: Some(4.0),
                });
            }
            ConnectCommand::SeekAbsolute { position_secs } => {
                if position_secs.is_finite() {
                    self.enqueue_session_cmd(SessionCommand::SeekAbsolute(position_secs.max(0.0)));
                }
            }
            ConnectCommand::SetVolume { volume } => {
                self.set_volume(volume, true);
            }
            ConnectCommand::TransferQueue {
                tracks,
                start_index,
                position_secs,
            } => {
                let Some((filtered, idx)) = filter_tui_transfer_tracks(
                    tracks,
                    start_index,
                    self.config.personal_only_mode,
                    &self.downloaded_track_ids,
                ) else {
                    self.status_line =
                        "Connect handoff ignored: no playable tracks in current mode".to_string();
                    return;
                };
                self.pending_connect_seek = filtered
                    .get(idx)
                    .filter(|_| position_secs.is_finite() && position_secs > 0.5)
                    .map(|t| (t.id.clone(), position_secs.max(0.0)));
                self.status_line = "Adopted queue from Sonata Connect".to_string();
                self.enqueue_session_cmd(SessionCommand::PlayQueue {
                    tracks: filtered,
                    start_index: idx,
                    push_history: true,
                });
            }
        }
    }
}
