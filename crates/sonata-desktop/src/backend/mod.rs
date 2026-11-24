//! Headless playback and state engine behind the GPUI client, running on its own Tokio runtime thread.
//!
//! 1. **Types & Handle (`types`)**: defines `BackendCmd`, `UiWindowCmd`, `DesktopSnapshot`, `BackendHandle`, and `InternalEvent`.
//! 2. **Engine Loop (`mod.rs`)**: `run_backend_loop` multiplexes commands, internal task events, and 40ms playback ticks, and publishes snapshots.
//! 3. **Playback (`playback`)**: drives transitions, stream resolution, Hi-Fi upgrades, cover enrichment, precache, and `mpv` events.
//! 4. **Library (`library`)**: handles authentication, home/artist/collection feeds, search, playlist edits, themes, and thumbnail prefetching.

mod library;
mod playback;
mod types;

pub use sonata_core::providers::ytmusic::{
    AlbumDetail, MoodCategoryTile, MoodChip, RichSearchResult,
};
pub use types::{BackendCmd, BackendHandle, DesktopSnapshot, NavTarget, UiWindowCmd};

use crate::integrations::mpris::{spawn_mpris, MprisUpdate};
use crate::integrations::tray::{spawn_tray, TrayHandle, TrayState};
use reqwest::Client;
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::{enforce_media_cache_limit_async, start_media_cache_maintenance, CacheDb};
use sonata_core::lyrics::LyricsState;
use sonata_core::paths::CorePaths;
use sonata_core::player::{PlaybackState, PlaybackTaskSet};
use sonata_core::providers::hifi::{
    FastStartMode, ResolverProfile, StreamFallbackMode, UpgradeDelivery,
};
use sonata_core::providers::servers::{
    load_server_profiles, ServerClient, ServerLibrary, ServerProfile,
};
use sonata_core::providers::ytmusic::{ArtistPage, HomeShelf, YtAccountProfile, YtMusicClient};
use sonata_core::providers::{build_http_client, Playlist, Track};
use sonata_runtime::auth::{list_supported_browsers, BrowserOption};
use sonata_runtime::config::AppConfig;
use sonata_runtime::discord::{DiscordRpcHandle, DiscordRpcStatus};
use sonata_runtime::downloads::{DownloadStore, DownloadedCollection, DownloadedTrack};
use sonata_runtime::hyprland;
use sonata_runtime::media;
use sonata_runtime::player::{
    MpvController, NoopAudioDriver, PlaybackAudioDriver, PlaybackSession, PlaybackSessionConfig,
    PlaybackSessionHandle, PlaybackStreamSource, SessionCommand, SessionPolicy,
};
use sonata_runtime::process::LinuxStreamResolver;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch, Semaphore};
use types::{ExportFingerprint, InternalEvent};

pub(super) fn linux_core_paths() -> CorePaths {
    AppConfig::default_core_paths()
}

pub(super) fn linux_resolver_profile(config: &AppConfig) -> ResolverProfile {
    ResolverProfile {
        fast_start: FastStartMode::BoundedRange,
        upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
        fallback: StreamFallbackMode::ExternalTool,
        bootstrap_timeout: Duration::from_millis(2400),
        jiosaavn_enabled: config.jiosaavn_enabled,
        cache_limit_bytes: config.cache_limit_bytes(),
    }
}

pub fn spawn_backend(win_tx: mpsc::UnboundedSender<UiWindowCmd>) -> anyhow::Result<BackendHandle> {
    let config = AppConfig::load();
    let paths = linux_core_paths();
    let available_browsers = list_supported_browsers();

    let cache_db = CacheDb::open_with_paths(&paths, config.cache_limit_gb > 0);
    let initial_browser = config.desktop_ytm_browser.clone();
    let ytm_playlists = if initial_browser.is_some() {
        cache_db
            .as_ref()
            .and_then(|db| db.get_library("ytm"))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut playback = PlaybackState::new(config.volume);
    playback.shuffle = config.shuffle;
    playback.repeat = config.repeat_mode;

    let download_store = DownloadStore::open(config.effective_downloads_dir()).ok();
    let (downloaded_tracks, downloaded_collections, downloads_total_bytes) =
        if let Some(ref store) = download_store {
            (
                store.list_tracks().unwrap_or_default(),
                store.list_collections().unwrap_or_default(),
                store.total_bytes().unwrap_or(0),
            )
        } else {
            (Vec::new(), Vec::new(), 0)
        };
    let downloaded_track_ids: HashSet<String> = downloaded_tracks
        .iter()
        .map(|t| t.track_id.clone())
        .collect();
    let server_profiles = load_server_profiles(&paths).unwrap_or_default();
    let active_server_id = server_profiles
        .iter()
        .find(|p| p.enabled)
        .or_else(|| server_profiles.first())
        .map(|p| p.id.clone());
    let initial_discord_status = if config.discord_rpc_enabled {
        DiscordRpcStatus::Disconnected
    } else {
        DiscordRpcStatus::Disabled
    };

    let initial_snapshot = Arc::new(DesktopSnapshot {
        config: config.clone(),
        active_nav: NavTarget::Home,
        status_line: "Loading YouTube Music home...".to_string(),
        available_browsers: available_browsers.clone(),
        signed_in_browser: initial_browser.clone(),
        user_profile: None,
        user_avatar_path: None,
        signing_in: false,
        sign_in_error: None,
        ytm_playlists: ytm_playlists.clone(),
        library_albums: Vec::new(),
        library_artists: Vec::new(),
        library_extras_loading: false,
        home_chips: Vec::new(),
        active_home_chip: None,
        home_shelves: Vec::new(),
        home_loading: true,
        home_error: None,
        explore_shelves: Vec::new(),
        mood_tiles: Vec::new(),
        explore_loading: false,
        mood_page_title: String::new(),
        mood_page_shelves: Vec::new(),
        mood_page_loading: false,
        shelf_detail: None,
        radio_shelves: Vec::new(),
        radio_loading: true,
        history_tracks: Vec::new(),
        history_loading: false,
        last_played_tracks: Vec::new(),
        recent_searches: Vec::new(),
        search_suggestions: Vec::new(),
        search_query: String::new(),
        search_results: Vec::new(),
        search_rich: RichSearchResult::default(),
        search_loading: false,
        playlist_id: None,
        playlist_heading: String::new(),
        playlist_subtitle: String::new(),
        playlist_cover_url: None,
        playlist_editable: false,
        playlist_tracks: Vec::new(),
        playlist_loading: false,
        album_detail: None,
        album_loading: false,
        album_error: None,
        artist_page: None,
        artist_banner_url: None,
        artist_loading: false,
        artist_error: None,
        liked_track_ids: HashSet::new(),
        queue: Vec::new(),
        current_index: None,
        paused: true,
        muted: false,
        position_secs: 0.0,
        duration_secs: 0.0,
        volume: config.volume,
        shuffle: config.shuffle,
        repeat: config.repeat_mode,
        autoplay: config.autoplay,
        stream_badge: "Idle".to_string(),
        upgrading_quality: false,
        loading_track: false,
        lyrics: LyricsState::Idle,
        art_path: None,
        motion_cover: None,
        ambient_rgb: (110, 90, 170),
        spotify_notice: None,
        update_info: None,
        checking_updates: false,
        thumb_rev: 0,
        downloaded_tracks: downloaded_tracks.clone(),
        downloaded_collections: downloaded_collections.clone(),
        downloaded_track_ids: downloaded_track_ids.clone(),
        downloading_track_ids: HashSet::new(),
        downloads_total_bytes,
        discord_status: initial_discord_status,
        server_profiles: server_profiles.clone(),
        server_statuses: HashMap::new(),
        active_server_id: active_server_id.clone(),
        server_libraries: Vec::new(),
        server_artists: Vec::new(),
        server_albums: Vec::new(),
        server_playlists: Vec::new(),
        server_tracks: Vec::new(),
        server_current_folder: None,
        server_loading: false,
        server_test_result: None,
    });

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (snap_tx, snap_rx) = watch::channel(initial_snapshot.clone());
    let (progress_tx, progress_rx) = watch::channel((0.0_f64, 0.0_f64));
    let mpris_tx = spawn_mpris(cmd_tx.clone(), win_tx.clone());
    let _ = mpris_tx.send(MprisUpdate::Sync(initial_snapshot));

    let cmd_tx_thread = cmd_tx.clone();
    let win_tx_thread = win_tx.clone();
    std::thread::Builder::new()
        .name("sonata-backend".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(run_backend_loop(
                config,
                cache_db,
                available_browsers,
                initial_browser,
                ytm_playlists,
                playback,
                download_store,
                downloaded_tracks,
                downloaded_collections,
                downloaded_track_ids,
                downloads_total_bytes,
                server_profiles,
                active_server_id,
                cmd_tx_thread,
                cmd_rx,
                snap_tx,
                progress_tx,
                mpris_tx,
                win_tx_thread,
            ));
        })?;

    Ok(BackendHandle {
        cmd_tx,
        snap_rx,
        progress_rx,
    })
}

struct BackendEngine {
    pub(super) config: AppConfig,
    pub(super) paths: CorePaths,
    pub(super) cache_limit_bytes: Arc<AtomicU64>,
    pub(super) http: Client,
    pub(super) cookies: ExtractedCookies,
    pub(super) ytm: YtMusicClient,
    pub(super) ytm_language: sonata_runtime::i18n::AppLanguage,
    pub(super) resolver: LinuxStreamResolver,
    pub(super) cache_db: Option<CacheDb>,
    pub(super) session: PlaybackSessionHandle,
    pub(super) int_tx: mpsc::UnboundedSender<InternalEvent>,
    pub(super) snap_tx: watch::Sender<Arc<DesktopSnapshot>>,
    pub(super) progress_tx: watch::Sender<(f64, f64)>,
    pub(super) mpris_tx: mpsc::UnboundedSender<MprisUpdate>,
    pub(super) tray_handle: Option<TrayHandle>,
    pub(super) last_tray_state: TrayState,
    pub(super) window_visible: bool,
    pub(super) ui_compact: bool,
    pub(super) deferred_media_play_seq: Option<u64>,
    pub(super) hyprland_window: Option<String>,
    pub(super) active_nav: NavTarget,
    pub(super) status_line: String,
    pub(super) available_browsers: Vec<BrowserOption>,
    pub(super) signed_in_browser: Option<String>,
    pub(super) user_profile: Option<YtAccountProfile>,
    pub(super) user_avatar_path: Option<PathBuf>,
    pub(super) signing_in: bool,
    pub(super) sign_in_error: Option<String>,
    pub(super) ytm_playlists: Vec<Playlist>,
    pub(super) library_albums: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub(super) library_artists: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub(super) library_extras_loading: bool,
    pub(super) home_chips: Vec<MoodChip>,
    pub(super) active_home_chip: Option<MoodChip>,
    pub(super) home_shelves: Vec<HomeShelf>,
    pub(super) home_loading: bool,
    pub(super) home_error: Option<String>,
    pub(super) explore_shelves: Vec<HomeShelf>,
    pub(super) mood_tiles: Vec<MoodCategoryTile>,
    pub(super) explore_loading: bool,
    pub(super) mood_page_title: String,
    pub(super) mood_page_shelves: Vec<HomeShelf>,
    pub(super) mood_page_loading: bool,
    pub(super) shelf_detail: Option<HomeShelf>,
    pub(super) radio_shelves: Vec<HomeShelf>,
    pub(super) radio_loading: bool,
    pub(super) history_tracks: Vec<Track>,
    pub(super) history_loading: bool,
    pub(super) last_played_tracks: Vec<Track>,
    pub(super) recent_searches: Vec<String>,
    pub(super) search_suggestions: Vec<String>,
    pub(super) search_query: String,
    pub(super) search_results: Vec<Track>,
    pub(super) search_rich: RichSearchResult,
    pub(super) search_loading: bool,
    pub(super) playlist_id: Option<String>,
    pub(super) playlist_heading: String,
    pub(super) playlist_subtitle: String,
    pub(super) playlist_cover_url: Option<String>,
    pub(super) playlist_editable: bool,
    pub(super) playlist_tracks: Vec<Track>,
    pub(super) playlist_loading: bool,
    pub(super) active_queue_playlist_id: Option<String>,
    pub(super) prefetched_autoplay_seed: Option<String>,
    pub(super) prefetched_autoplay_tracks: Vec<Track>,
    pub(super) album_detail: Option<AlbumDetail>,
    pub(super) album_loading: bool,
    pub(super) album_error: Option<String>,
    pub(super) artist_page: Option<ArtistPage>,
    pub(super) artist_banner_url: Option<String>,
    pub(super) artist_loading: bool,
    pub(super) artist_error: Option<String>,
    pub(super) liked_track_ids: HashSet<String>,
    pub(super) playback: PlaybackState,
    pub(super) muted: bool,
    pub(super) lyrics: LyricsState,
    pub(super) art_path: Option<PathBuf>,
    pub(super) motion_cover: Option<Arc<sonata_runtime::motion::MotionCover>>,
    pub(super) ambient_rgb: (u8, u8, u8),
    pub(super) spotify_notice: Option<String>,
    pub(super) update_info: Option<sonata_core::updater::UpdateCheckResult>,
    pub(super) checking_updates: bool,
    pub(super) home_req_id: u64,
    pub(super) explore_req_id: u64,
    pub(super) mood_req_id: u64,
    pub(super) radio_req_id: u64,
    pub(super) search_req_id: u64,
    pub(super) suggest_req_id: u64,
    pub(super) playlist_req_id: u64,
    pub(super) album_req_id: u64,
    pub(super) artist_req_id: u64,
    pub(super) play_seq: u64,
    pub(super) media_tasks: PlaybackTaskSet,
    pub(super) upgrading_quality: bool,
    pub(super) cover_started_play_seq: Option<u64>,
    pub(super) config_dirty: bool,
    pub(super) last_exported_state: Option<ExportFingerprint>,
    pub(super) in_flight_thumbs: Arc<Mutex<HashSet<String>>>,
    pub(super) thumbnail_slots: Arc<Semaphore>,
    pub(super) urgent_thumbnail_slots: Arc<Semaphore>,
    pub(super) thumb_rev: u64,
    pub(super) playback_speed: f64,
    pub(super) discord_rpc: DiscordRpcHandle,
    pub(super) download_store: Option<DownloadStore>,
    pub(super) downloaded_tracks: Vec<DownloadedTrack>,
    pub(super) downloaded_collections: Vec<DownloadedCollection>,
    pub(super) downloaded_track_ids: HashSet<String>,
    pub(super) downloading_track_ids: HashSet<String>,
    pub(super) downloads_total_bytes: u64,
    pub(super) server_client: ServerClient,
    pub(super) server_profiles: Vec<ServerProfile>,
    pub(super) server_statuses: HashMap<String, String>,
    pub(super) active_server_id: Option<String>,
    pub(super) server_libraries: Vec<ServerLibrary>,
    pub(super) server_artists: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub(super) server_albums: Vec<sonata_core::providers::ytmusic::HomeCard>,
    pub(super) server_playlists: Vec<Playlist>,
    pub(super) server_tracks: Vec<Track>,
    pub(super) server_current_folder: Option<String>,
    pub(super) server_loading: bool,
    pub(super) server_req_id: u64,
    pub(super) server_test_result: Option<Result<String, String>>,
}

#[allow(clippy::too_many_arguments)]
async fn run_backend_loop(
    config: AppConfig,
    cache_db: Option<CacheDb>,
    available_browsers: Vec<BrowserOption>,
    initial_browser: Option<String>,
    ytm_playlists: Vec<Playlist>,
    playback: PlaybackState,
    download_store: Option<DownloadStore>,
    downloaded_tracks: Vec<DownloadedTrack>,
    downloaded_collections: Vec<DownloadedCollection>,
    downloaded_track_ids: HashSet<String>,
    downloads_total_bytes: u64,
    server_profiles: Vec<ServerProfile>,
    active_server_id: Option<String>,
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    mut cmd_rx: mpsc::UnboundedReceiver<BackendCmd>,
    snap_tx: watch::Sender<Arc<DesktopSnapshot>>,
    progress_tx: watch::Sender<(f64, f64)>,
    mpris_tx: mpsc::UnboundedSender<MprisUpdate>,
    win_tx: mpsc::UnboundedSender<UiWindowCmd>,
) {
    // Cache maintenance uses Tokio tasks, so start it inside the backend runtime.
    let paths = linux_core_paths();
    let cache_limit_bytes = Arc::new(AtomicU64::new(config.cache_limit_bytes()));
    start_media_cache_maintenance(paths.clone(), Arc::clone(&cache_limit_bytes));
    let init_paths = paths.clone();
    let init_limit = config.cache_limit_bytes();
    tokio::spawn(async move {
        enforce_media_cache_limit_async(init_paths, init_limit).await;
    });

    let http = build_http_client();
    let cookies = ExtractedCookies::default();
    let initial_lang = crate::settings::DesktopSettings::load().language;
    let ytm = YtMusicClient::new(http.clone(), cookies.clone()).with_language(initial_lang.code());
    let mut resolver = LinuxStreamResolver::with_profile(
        http.clone(),
        ytm.clone(),
        cookies.clone(),
        cache_db.clone(),
        paths.clone(),
        linux_resolver_profile(&config),
    );
    if let Some(ref store) = download_store {
        resolver = resolver.with_downloads_store(store.clone());
    }
    let discord_rpc = DiscordRpcHandle::spawn(playback::discord_config_from(&config));
    let mut discord_status_rx = discord_rpc.subscribe_status();
    let server_client = ServerClient::new(http.clone());
    let warm_resolver = resolver.clone();
    tokio::spawn(async move {
        warm_resolver.warm().await;
    });

    let (mpv_tx, mpv_rx) = mpsc::channel(64);
    let (driver, initial_status): (Arc<dyn PlaybackAudioDriver>, String) =
        match MpvController::spawn_desktop(config.volume, mpv_tx).await {
            Ok(ctrl) => (Arc::new(ctrl), "Loading YouTube Music...".to_string()),
            Err(e) => (Arc::new(NoopAudioDriver), format!("Audio warning: {e}")),
        };

    let mut session_cfg = PlaybackSessionConfig::for_client("desktop");
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
        events: mut session_rx,
    } = PlaybackSession::spawn(
        session_cfg,
        driver,
        Arc::new(resolver.clone()) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    let (int_tx, mut int_rx) = mpsc::unbounded_channel();
    let tray_handle = spawn_tray(cmd_tx, win_tx).await;
    let initial_liked_ids: HashSet<String> = cache_db
        .as_ref()
        .and_then(|db| db.get_playlist("YTM:VLLM"))
        .map(|(_, tracks, _)| tracks.into_iter().map(|t| t.id).collect())
        .unwrap_or_default();

    let mut engine = BackendEngine {
        config,
        paths,
        cache_limit_bytes,
        http,
        cookies,
        ytm,
        ytm_language: initial_lang,
        resolver,
        cache_db,
        session,
        int_tx,
        snap_tx,
        progress_tx,
        mpris_tx,
        tray_handle,
        last_tray_state: TrayState::default(),
        window_visible: true,
        ui_compact: false,
        deferred_media_play_seq: None,
        hyprland_window: None,
        active_queue_playlist_id: None,
        prefetched_autoplay_seed: None,
        prefetched_autoplay_tracks: Vec::new(),
        active_nav: NavTarget::Home,
        status_line: initial_status,
        available_browsers,
        signed_in_browser: initial_browser.clone(),
        user_profile: None,
        user_avatar_path: None,
        signing_in: false,
        sign_in_error: None,
        ytm_playlists,
        library_albums: Vec::new(),
        library_artists: Vec::new(),
        library_extras_loading: false,
        home_chips: Vec::new(),
        active_home_chip: None,
        home_shelves: Vec::new(),
        home_loading: true,
        home_error: None,
        explore_shelves: Vec::new(),
        mood_tiles: Vec::new(),
        explore_loading: false,
        mood_page_title: String::new(),
        mood_page_shelves: Vec::new(),
        mood_page_loading: false,
        shelf_detail: None,
        radio_shelves: Vec::new(),
        radio_loading: true,
        history_tracks: Vec::new(),
        history_loading: false,
        last_played_tracks: Vec::new(),
        recent_searches: Vec::new(),
        search_suggestions: Vec::new(),
        search_query: String::new(),
        search_results: Vec::new(),
        search_rich: RichSearchResult::default(),
        search_loading: false,
        playlist_id: None,
        playlist_heading: String::new(),
        playlist_subtitle: String::new(),
        playlist_cover_url: None,
        playlist_editable: false,
        playlist_tracks: Vec::new(),
        playlist_loading: false,
        album_detail: None,
        album_loading: false,
        album_error: None,
        artist_page: None,
        artist_banner_url: None,
        artist_loading: false,
        artist_error: None,
        liked_track_ids: initial_liked_ids,
        playback,
        muted: false,
        lyrics: LyricsState::Idle,
        art_path: None,
        motion_cover: None,
        ambient_rgb: (110, 90, 170),
        spotify_notice: None,
        update_info: None,
        checking_updates: false,
        home_req_id: 0,
        explore_req_id: 0,
        mood_req_id: 0,
        radio_req_id: 0,
        search_req_id: 0,
        suggest_req_id: 0,
        playlist_req_id: 0,
        album_req_id: 0,
        artist_req_id: 0,
        play_seq: 0,
        media_tasks: PlaybackTaskSet::default(),
        upgrading_quality: false,
        cover_started_play_seq: None,
        config_dirty: false,
        last_exported_state: None,
        in_flight_thumbs: Arc::new(Mutex::new(HashSet::new())),
        thumbnail_slots: Arc::new(Semaphore::new(8)),
        urgent_thumbnail_slots: Arc::new(Semaphore::new(12)),
        thumb_rev: 0,
        playback_speed: 1.0,
        discord_rpc,
        download_store,
        downloaded_tracks,
        downloaded_collections,
        downloaded_track_ids,
        downloading_track_ids: HashSet::new(),
        downloads_total_bytes,
        server_client,
        server_profiles,
        server_statuses: HashMap::new(),
        active_server_id: active_server_id.clone(),
        server_libraries: Vec::new(),
        server_artists: Vec::new(),
        server_albums: Vec::new(),
        server_playlists: Vec::new(),
        server_tracks: Vec::new(),
        server_current_folder: None,
        server_loading: false,
        server_req_id: 0,
        server_test_result: None,
    };

    if let Some(sid) = active_server_id {
        engine.load_server_browse(sid, None, None);
    }

    let cached_playlist_thumbs: Vec<String> = engine
        .ytm_playlists
        .iter()
        .filter_map(|p| p.thumbnail_url.clone())
        .collect();
    if !cached_playlist_thumbs.is_empty() {
        engine.prefetch_thumbnails(cached_playlist_thumbs);
    }

    if let Some(browser_id) = initial_browser {
        engine.start_sign_in(browser_id);
    } else {
        engine.load_home_feed();
        engine.load_explore();
        engine.load_radio_mixes();
    }
    engine.spawn_update_check();
    engine.sync_hyprland_opacity(None);
    engine.publish(true).await;

    let mut smooth_tick = tokio::time::interval(Duration::from_millis(250));
    smooth_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let active_play = !engine.playback.paused
            && !engine.playback.loading_track
            && engine.playback.current_track().is_some();

        tokio::select! {
            _ = smooth_tick.tick(), if active_play && engine.window_visible && !engine.ui_compact => {
                if engine.tick_playback() {
                    let _ = engine
                        .progress_tx
                        .send((engine.playback.position_secs, engine.playback.duration_secs));
                    engine.sync_discord_rpc();
                }
            }
            Ok(()) = discord_status_rx.changed() => {
                let _ = discord_status_rx.borrow_and_update();
                engine.publish(false).await;
            }
            Some(cmd) = cmd_rx.recv() => {
                if let BackendCmd::Shutdown(done) = cmd {
                    engine.shutdown().await;
                    let _ = done.try_send(());
                    break;
                }
                engine.handle_command(cmd).await;
                while let Ok(more) = cmd_rx.try_recv() {
                    if let BackendCmd::Shutdown(done) = more {
                        engine.shutdown().await;
                        let _ = done.try_send(());
                        return;
                    }
                    engine.handle_command(more).await;
                }
                engine.publish(true).await;
            }
            Some(sev) = session_rx.recv() => {
                engine.handle_session_event(sev);
                while let Ok(more) = session_rx.try_recv() {
                    engine.handle_session_event(more);
                }
                engine.publish(true).await;
            }
            Some(ev) = int_rx.recv() => {
                engine.handle_internal_event(ev).await;
                while let Ok(more) = int_rx.try_recv() {
                    engine.handle_internal_event(more).await;
                }
                engine.publish(true).await;
            }
            else => break,
        }
    }
}

impl BackendEngine {
    fn session_policy(&self) -> SessionPolicy {
        SessionPolicy {
            hifi_substitution: self.config.hifi_substitution,
            jiosaavn_enabled: self.config.jiosaavn_enabled,
            cache_enabled: self.config.cache_limit_gb > 0,
        }
    }

    async fn handle_command(&mut self, cmd: BackendCmd) {
        match cmd {
            BackendCmd::Navigate(target) => {
                self.active_nav = target.clone();
                match target {
                    NavTarget::Home if self.home_shelves.is_empty() && !self.home_loading => {
                        self.load_home_feed();
                    }
                    NavTarget::Explore
                        if self.explore_shelves.is_empty() && !self.explore_loading =>
                    {
                        self.load_explore();
                    }
                    NavTarget::Library => {
                        self.refresh_downloads();
                        if self.library_albums.is_empty() && !self.library_extras_loading {
                            self.load_library_extras();
                        }
                        if self.active_server_id.is_some()
                            && self.server_libraries.is_empty()
                            && self.server_playlists.is_empty()
                            && self.server_albums.is_empty()
                            && !self.server_loading
                        {
                            if let Some(sid) = self.active_server_id.clone() {
                                self.load_server_browse(sid, None, None);
                            }
                        }
                    }
                    NavTarget::History if self.history_tracks.is_empty() => {
                        self.load_listening_history();
                    }
                    NavTarget::Radio if self.radio_shelves.is_empty() && !self.radio_loading => {
                        self.load_radio_mixes();
                    }
                    NavTarget::Playlist(pl_id) => {
                        if let Some(pl) = self.ytm_playlists.iter().find(|p| p.id == pl_id).cloned()
                        {
                            let subtitle = if pl.id == "VLLM" {
                                "Your liked YouTube Music tracks".to_string()
                            } else {
                                "YouTube Music Playlist".to_string()
                            };
                            self.open_collection(
                                pl.id,
                                pl.name,
                                subtitle,
                                pl.thumbnail_url,
                                true,
                                false,
                            );
                        }
                    }
                    NavTarget::Artist(id) => {
                        if let Some(name) = id.strip_prefix("name:") {
                            self.open_artist_by_name(name.to_string());
                        } else {
                            self.open_artist(id);
                        }
                    }
                    NavTarget::Album(id) => {
                        self.open_album(Some(id), String::new(), String::new(), None);
                    }
                    NavTarget::MoodCategory {
                        title,
                        browse_id,
                        params,
                    } => {
                        self.open_mood_category(title, browse_id, params);
                    }
                    _ => {}
                }
            }
            BackendCmd::OpenCollection {
                id,
                title,
                subtitle,
                thumbnail_url,
                editable,
            } => {
                self.open_collection(id, title, subtitle, thumbnail_url, editable, false);
            }
            BackendCmd::OpenArtist(id) => self.open_artist(id),
            BackendCmd::OpenArtistByName(name) => self.open_artist_by_name(name),
            BackendCmd::OpenAlbum {
                album_id,
                title,
                artist,
                thumbnail_url,
            } => self.open_album(album_id, title, artist, thumbnail_url),
            BackendCmd::OpenMoodCategory {
                title,
                browse_id,
                params,
            } => self.open_mood_category(title, browse_id, params),
            BackendCmd::OpenMoodByTitle(title) => self.open_mood_by_title(title),
            BackendCmd::OpenShelfDetail(shelf) => {
                self.active_nav = NavTarget::ShelfDetail {
                    title: shelf.title.clone(),
                };
                self.shelf_detail = Some(shelf);
            }
            BackendCmd::SelectHomeChip(chip) => {
                self.load_home_with_chip(chip);
            }
            BackendCmd::RefreshActivePlaylist => {
                if let Some(id) = self.playlist_id.clone() {
                    let heading = self.playlist_heading.clone();
                    let subtitle = self.playlist_subtitle.clone();
                    let cover = self.playlist_cover_url.clone();
                    let editable = self.playlist_editable;
                    self.open_collection(id, heading, subtitle, cover, editable, true);
                }
            }
            BackendCmd::RefreshHome => {
                self.load_home_with_chip(self.active_home_chip.clone());
                self.load_listening_history();
            }
            BackendCmd::RefreshExplore => {
                self.load_explore();
            }
            BackendCmd::RefreshRadio => {
                self.load_radio_mixes();
            }
            BackendCmd::RefreshLibrary => {
                self.refresh_account();
                self.load_library_extras();
                self.refresh_downloads();
                if let Some(sid) = self.active_server_id.clone() {
                    self.load_server_browse(sid, None, self.server_current_folder.clone());
                }
            }
            BackendCmd::RefreshHistory => {
                self.load_listening_history();
            }
            BackendCmd::StartTrackRadio(track) => {
                self.start_track_radio(track);
            }
            BackendCmd::StartQuickRadio => {
                self.start_quick_radio();
            }
            BackendCmd::RefreshAccount => {
                self.refresh_account();
            }
            BackendCmd::SignInWithBrowser(browser_id) => {
                self.start_sign_in(browser_id);
            }
            BackendCmd::ClearSignInError => {
                self.sign_in_error = None;
            }
            BackendCmd::SignOut => {
                self.sign_out().await;
            }
            BackendCmd::FetchSearchSuggestions(q) => {
                self.fetch_search_suggestions(&q);
            }
            BackendCmd::Search(q) => {
                self.execute_search(&q);
            }
            BackendCmd::PlayTracks {
                tracks,
                start_index,
                shuffle,
            } => {
                if tracks.is_empty() {
                    return;
                }
                self.active_queue_playlist_id = match self.active_nav {
                    NavTarget::Playlist(ref pid) => Some(pid.clone()),
                    NavTarget::Album(ref aid) => Some(aid.clone()),
                    _ => None,
                };
                let idx = if shuffle {
                    self.playback.shuffle = true;
                    self.config.shuffle = true;
                    let _ = self.config.save();
                    let _ = self.session.send(SessionCommand::SetShuffle(true)).await;
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0);
                    nanos % tracks.len()
                } else {
                    start_index.min(tracks.len() - 1)
                };
                let _ = self
                    .session
                    .send(SessionCommand::PlayQueue {
                        tracks,
                        start_index: idx,
                        push_history: false,
                    })
                    .await;
            }
            BackendCmd::PlayQueueIndex(idx) => {
                let _ = self
                    .session
                    .send(SessionCommand::PlayIndex {
                        index: idx,
                        push_history: true,
                    })
                    .await;
            }
            BackendCmd::PlayTrackNext(track) => {
                let _ = self
                    .session
                    .send(SessionCommand::PlayTrackNext(track))
                    .await;
            }
            BackendCmd::AddTrackToQueue(track) => {
                let _ = self
                    .session
                    .send(SessionCommand::AddTrackToQueue(track))
                    .await;
            }
            BackendCmd::RemoveFromQueue(idx) => {
                let _ = self
                    .session
                    .send(SessionCommand::RemoveFromQueue(idx))
                    .await;
            }
            BackendCmd::MoveQueueItem { from, to } => {
                let _ = self
                    .session
                    .send(SessionCommand::MoveQueueItem { from, to })
                    .await;
            }
            BackendCmd::ClearQueue => {
                let _ = self.session.send(SessionCommand::ClearQueue).await;
            }
            BackendCmd::ShuffleQueue => {
                let _ = self.session.send(SessionCommand::ShuffleQueue).await;
            }
            BackendCmd::TogglePause => {
                let _ = self.session.send(SessionCommand::TogglePause).await;
            }
            BackendCmd::SetPaused(paused) => {
                let _ = self.session.send(SessionCommand::SetPaused(paused)).await;
            }
            BackendCmd::Stop => {
                let _ = self.session.send(SessionCommand::Stop).await;
            }
            BackendCmd::NextTrack => {
                let _ = self.session.send(SessionCommand::Next).await;
            }
            BackendCmd::PrevTrack => {
                let _ = self
                    .session
                    .send(SessionCommand::Previous {
                        restart_if_after_secs: Some(4.0),
                    })
                    .await;
            }
            BackendCmd::SeekAbsolute(secs) => {
                let _ = self.session.send(SessionCommand::SeekAbsolute(secs)).await;
            }
            BackendCmd::SeekRelative(delta) => {
                let _ = self.session.send(SessionCommand::SeekRelative(delta)).await;
            }
            BackendCmd::SetVolume(vol) => {
                let clamped = vol.min(100);
                self.muted = false;
                self.playback.volume = clamped;
                self.config.volume = clamped;
                self.config_dirty = true;
                let _ = self.session.send(SessionCommand::SetVolume(clamped)).await;
            }
            BackendCmd::ToggleMute => {
                self.muted = !self.muted;
                let _ = self
                    .session
                    .send(SessionCommand::SetMuted(self.muted))
                    .await;
                self.status_line = if self.muted {
                    "Audio muted".to_string()
                } else {
                    format!("Volume: {}%", self.config.volume)
                };
            }
            BackendCmd::ToggleShuffle => {
                self.playback.shuffle = !self.playback.shuffle;
                self.config.shuffle = self.playback.shuffle;
                let _ = self.config.save();
                let _ = self
                    .session
                    .send(SessionCommand::SetShuffle(self.playback.shuffle))
                    .await;
                self.status_line = format!(
                    "Shuffle: {}",
                    if self.playback.shuffle { "On" } else { "Off" }
                );
            }
            BackendCmd::SetShuffle(on) => {
                self.playback.shuffle = on;
                self.config.shuffle = on;
                let _ = self.config.save();
                let _ = self.session.send(SessionCommand::SetShuffle(on)).await;
            }
            BackendCmd::CycleRepeat => {
                let next = self.playback.repeat.next();
                self.playback.repeat = next;
                self.config.repeat_mode = next;
                let _ = self.config.save();
                let _ = self.session.send(SessionCommand::SetRepeat(next)).await;
                self.status_line = format!("Repeat: {}", next.label());
            }
            BackendCmd::SetRepeat(mode) => {
                self.playback.repeat = mode;
                self.config.repeat_mode = mode;
                let _ = self.config.save();
                let _ = self.session.send(SessionCommand::SetRepeat(mode)).await;
            }
            BackendCmd::ToggleAutoplay => {
                self.config.autoplay = !self.config.autoplay;
                let _ = self.config.save();
                self.status_line = format!(
                    "Endless autoplay: {}",
                    if self.config.autoplay { "On" } else { "Off" }
                );
            }
            BackendCmd::ToggleHiFi => {
                self.config.hifi_substitution = !self.config.hifi_substitution;
                let _ = self.config.save();
                let _ = self
                    .session
                    .send(SessionCommand::UpdatePolicy {
                        policy: self.session_policy(),
                        trigger_upgrade: self.config.hifi_substitution,
                    })
                    .await;
                self.status_line = format!(
                    "YouTube Music Quality Upgrade: {}",
                    if self.config.hifi_substitution {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::ToggleJioSaavn => {
                self.config.jiosaavn_enabled = !self.config.jiosaavn_enabled;
                let enabled = self.config.jiosaavn_enabled;
                self.resolver
                    .update_profile(|profile| profile.jiosaavn_enabled = enabled);
                let _ = self.config.save();
                let _ = self
                    .session
                    .send(SessionCommand::UpdatePolicy {
                        policy: self.session_policy(),
                        trigger_upgrade: enabled,
                    })
                    .await;
                self.status_line = format!(
                    "JioSaavn 320kbps module: {}",
                    if self.config.jiosaavn_enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::StepCacheLimit(forward) => {
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
                let _ = self
                    .session
                    .send(SessionCommand::UpdatePolicy {
                        policy: self.session_policy(),
                        trigger_upgrade: false,
                    })
                    .await;
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
            BackendCmd::ToggleMotionCovers => {
                self.config.motion_covers = !self.config.motion_covers;
                let _ = self.config.save();
                if !self.config.motion_covers {
                    self.motion_cover = None;
                } else {
                    self.cover_started_play_seq = None;
                    self.start_cover_enrichment();
                }
                self.status_line = format!(
                    "Animated album artwork: {}",
                    if self.config.motion_covers {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::ToggleReduceMotion => {
                self.config.reduce_motion = !self.config.reduce_motion;
                let _ = self.config.save();
                self.status_line = format!(
                    "Reduced motion: {}",
                    if self.config.reduce_motion {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::SetLanguage(lang) => {
                if self.ytm_language != lang {
                    self.ytm_language = lang;
                    self.config.language = lang;
                    let _ = self.config.save();
                    self.ytm.set_language(lang.code()).await;
                    if let Some(db) = &self.cache_db {
                        db.invalidate_playlist(None);
                    }
                    self.load_home_feed();
                    self.load_explore();
                    self.load_radio_mixes();
                }
            }
            BackendCmd::ToggleLike(track) => {
                self.toggle_like_track(track);
            }
            BackendCmd::DislikeTrack(track) => {
                self.dislike_track(track);
            }
            BackendCmd::ToggleSaveAlbum => {
                self.toggle_save_album();
            }
            BackendCmd::ToggleSavePlaylist {
                id,
                title,
                thumbnail_url,
                save,
            } => {
                self.toggle_save_playlist(id, title, thumbnail_url, save);
            }
            BackendCmd::CreateYtmPlaylist(title) => {
                self.create_ytm_playlist(&title);
            }
            BackendCmd::EditPlaylist {
                playlist_id,
                title,
                cover_path,
            } => {
                self.edit_playlist(playlist_id, title, cover_path);
            }
            BackendCmd::AddTrackToPlaylist {
                track,
                playlist_id,
                playlist_name,
            } => {
                self.add_track_to_playlist(track, playlist_id, playlist_name);
            }
            BackendCmd::RemoveTrackFromPlaylist(idx) => {
                self.remove_track_from_playlist(idx);
            }
            BackendCmd::ClickLinkSpotify => {
                self.spotify_notice = Some(
                    "Spotify integration is a preview placeholder and is not connected yet."
                        .to_string(),
                );
            }
            BackendCmd::CheckForUpdates => {
                self.spawn_update_check();
            }
            BackendCmd::RefreshDownloads => {
                self.refresh_downloads();
            }
            BackendCmd::DownloadTrack { track, collection } => {
                self.download_track(track, collection);
            }
            BackendCmd::RemoveDownloadedTrack(track_id) => {
                self.remove_downloaded_track(track_id);
            }
            BackendCmd::DownloadCollection {
                collection_id,
                title,
                source,
                artwork_url,
                tracks,
            } => {
                self.download_collection(collection_id, title, source, artwork_url, tracks);
            }
            BackendCmd::RemoveDownloadedCollection(collection_id) => {
                self.remove_downloaded_collection(collection_id);
            }
            BackendCmd::SetDownloadsDir(dir) => {
                self.set_downloads_dir(dir);
            }
            BackendCmd::StepDownloadsLimit(forward) => {
                self.step_downloads_limit(forward);
            }
            BackendCmd::ClearAllDownloads => {
                self.clear_all_downloads();
            }
            BackendCmd::ToggleDiscordRpc => {
                self.config.discord_rpc_enabled = !self.config.discord_rpc_enabled;
                let _ = self.config.save();
                self.discord_rpc
                    .try_update_config(playback::discord_config_from(&self.config));
                self.sync_discord_rpc();
                self.status_line = format!(
                    "Discord Rich Presence: {}",
                    if self.config.discord_rpc_enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::SetDiscordAppId(app_id) => {
                let normalized = app_id.and_then(|raw| {
                    let t = raw.trim().to_string();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                });
                self.config.discord_app_id = normalized;
                let _ = self.config.save();
                self.discord_rpc
                    .try_update_config(playback::discord_config_from(&self.config));
                self.sync_discord_rpc();
                self.status_line = match &self.config.discord_app_id {
                    Some(id) => format!("Discord Application ID updated ({id})"),
                    None => "Discord Application ID reset to default".to_string(),
                };
            }
            BackendCmd::RefreshServers => {
                if let Some(sid) = self.active_server_id.clone() {
                    self.load_server_browse(sid, None, None);
                }
            }
            BackendCmd::SaveServerProfile {
                id,
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
            } => {
                self.save_server_profile(
                    id,
                    name,
                    kind,
                    base_url,
                    username,
                    password,
                    api_key,
                    bearer_token,
                );
            }
            BackendCmd::TestServerDraft {
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
            } => {
                self.test_server_draft(
                    name,
                    kind,
                    base_url,
                    username,
                    password,
                    api_key,
                    bearer_token,
                );
            }
            BackendCmd::TestServerProfile(server_id) => {
                self.test_server_profile(server_id);
            }
            BackendCmd::ToggleServerEnabled(server_id) => {
                self.toggle_server_enabled(server_id);
            }
            BackendCmd::RemoveServerProfile(server_id) => {
                self.remove_server_profile(server_id);
            }
            BackendCmd::SelectServer {
                server_id,
                library_id,
                folder_path,
            } => {
                self.load_server_browse(server_id, library_id, folder_path);
            }
            BackendCmd::SetWindowAddress(addr) => {
                self.hyprland_window = addr;
                self.sync_hyprland_opacity(None);
            }
            BackendCmd::SetWindowVisible(vis) => {
                let became_visible = vis && !self.window_visible;
                self.window_visible = vis;
                if !vis {
                    // Abort non-audio visual media tasks while hidden/minimized to taskbar or tray.
                    self.media_tasks.restart();
                    self.motion_cover = None;
                } else if became_visible {
                    self.resume_deferred_media();
                }
            }
            BackendCmd::SetUiCompact(compact) => {
                let expanded = !compact && self.ui_compact;
                self.ui_compact = compact;
                if compact {
                    self.motion_cover = None;
                } else if expanded && self.window_visible {
                    self.resume_deferred_media();
                }
            }
            BackendCmd::Shutdown(_) => {}
        }
    }

    async fn handle_internal_event(&mut self, ev: InternalEvent) {
        match ev {
            InternalEvent::SignInFinished {
                browser_id,
                cookies,
                profile,
                avatar_path,
                ytm_playlists,
                error,
            } => {
                self.signing_in = false;
                if let Some(err) = error {
                    self.sign_in_error = Some(err.clone());
                    self.signed_in_browser = None;
                    if self.config.desktop_ytm_browser.as_deref() == Some(&browser_id) {
                        self.config.desktop_ytm_browser = None;
                        let _ = self.config.save();
                    }
                    self.status_line = err;
                    self.load_home_feed();
                    self.load_explore();
                    return;
                }

                self.sign_in_error = None;
                self.cookies = cookies;
                self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone())
                    .with_language(self.ytm_language.code());
                self.resolver = LinuxStreamResolver::with_profile(
                    self.http.clone(),
                    self.ytm.clone(),
                    self.cookies.clone(),
                    self.cache_db.clone(),
                    linux_core_paths(),
                    linux_resolver_profile(&self.config),
                );
                if let Some(ref store) = self.download_store {
                    self.resolver.set_downloads_store(Some(store.clone()));
                }
                let _ = self
                    .session
                    .send(SessionCommand::UpdateStreamSource(
                        Arc::new(self.resolver.clone()) as Arc<dyn PlaybackStreamSource>,
                    ))
                    .await;

                self.config.desktop_ytm_browser = Some(browser_id.clone());
                let _ = self.config.save();
                self.signed_in_browser = Some(browser_id);
                self.user_profile = profile;
                self.user_avatar_path = avatar_path;

                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &ytm_playlists);
                }
                self.ytm_playlists = ytm_playlists;
                let urls: Vec<String> = self
                    .ytm_playlists
                    .iter()
                    .filter_map(|p| p.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                self.status_line = "Signed in to YouTube Music".to_string();
                self.load_home_feed();
                self.load_explore();
                self.load_radio_mixes();
                self.load_library_extras();
                self.load_listening_history();
            }
            InternalEvent::AccountRefreshed {
                profile,
                avatar_path,
                ytm_playlists,
            } => {
                if let Some(p) = profile {
                    self.user_profile = Some(p);
                }
                if avatar_path.is_some() {
                    self.user_avatar_path = avatar_path;
                }
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &ytm_playlists);
                }
                self.ytm_playlists = ytm_playlists;
                let urls: Vec<String> = self
                    .ytm_playlists
                    .iter()
                    .filter_map(|p| p.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                self.status_line = format!(
                    "Synced {} YouTube Music playlists",
                    self.ytm_playlists.len()
                );
            }
            InternalEvent::HomeLoaded {
                req_id,
                chips,
                chip_selected,
                shelves,
                error,
            } => {
                if req_id != self.home_req_id {
                    return;
                }
                self.home_loading = false;
                self.home_error = error;
                if !chips.is_empty() && chip_selected.is_none() {
                    self.home_chips = chips;
                }
                if !shelves.is_empty() {
                    let mut shelves = shelves;
                    let preferred = self.listener_preferred_artists();
                    if !preferred.is_empty() {
                        let pref_lower: Vec<String> =
                            preferred.iter().map(|a| a.to_ascii_lowercase()).collect();
                        for shelf in &mut shelves {
                            if !shelf.tracks.is_empty() && shelf.cards.is_empty() {
                                shelf.tracks.sort_by_key(|t| {
                                    let a = t.artist.to_ascii_lowercase();
                                    pref_lower
                                        .iter()
                                        .position(|p| a.contains(p.as_str()))
                                        .unwrap_or(usize::MAX)
                                });
                            }
                        }
                    }
                    let mut urls = Vec::new();
                    for shelf in &shelves {
                        for t in &shelf.tracks {
                            if let Some(u) = &t.thumbnail_url {
                                urls.push(u.clone());
                            }
                        }
                        for c in &shelf.cards {
                            if let Some(u) = &c.thumbnail_url {
                                urls.push(u.clone());
                            }
                        }
                    }
                    self.prefetch_thumbnails(urls);
                    self.home_shelves = shelves;
                    self.status_line = "Home ready".to_string();
                }
            }
            InternalEvent::ExploreLoaded {
                req_id,
                shelves,
                mood_tiles,
            } => {
                if req_id != self.explore_req_id {
                    return;
                }
                self.explore_loading = false;
                let mut urls = Vec::new();
                for s in &shelves {
                    for c in &s.cards {
                        if let Some(u) = &c.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    for t in &s.tracks {
                        if let Some(u) = &t.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                }
                self.prefetch_thumbnails(urls);
                self.explore_shelves = shelves;
                self.mood_tiles = mood_tiles;
            }
            InternalEvent::MoodCategoryLoaded {
                req_id,
                title,
                shelves,
            } => {
                if req_id != self.mood_req_id {
                    return;
                }
                self.mood_page_loading = false;
                let mut urls = Vec::new();
                for s in &shelves {
                    for c in &s.cards {
                        if let Some(u) = &c.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    for t in &s.tracks {
                        if let Some(u) = &t.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                }
                self.prefetch_thumbnails_urgent(urls);
                self.status_line = format!("Loaded {title}");
                self.mood_page_title = title;
                self.mood_page_shelves = shelves;
            }
            InternalEvent::LibraryExtrasLoaded { albums, artists } => {
                self.library_extras_loading = false;
                let mut urls = Vec::new();
                for c in albums.iter().chain(artists.iter()) {
                    if let Some(u) = &c.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                self.prefetch_thumbnails(urls);
                self.library_albums = albums;
                self.library_artists = artists;
            }
            InternalEvent::RadioLoaded { req_id, shelves } => {
                if req_id != self.radio_req_id {
                    return;
                }
                self.radio_loading = false;
                if !shelves.is_empty() {
                    let mut urls = Vec::new();
                    for shelf in &shelves {
                        for c in &shelf.cards {
                            if let Some(u) = &c.thumbnail_url {
                                urls.push(u.clone());
                            }
                        }
                        for t in &shelf.tracks {
                            if let Some(u) = &t.thumbnail_url {
                                urls.push(u.clone());
                            }
                        }
                    }
                    self.prefetch_thumbnails(urls);
                    self.radio_shelves = shelves;
                }
            }
            InternalEvent::HistoryLoaded(tracks) => {
                self.history_loading = false;
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                if !tracks.is_empty() {
                    self.history_tracks = tracks.clone();
                }
                let mut seen = HashSet::new();
                let mut merged = Vec::new();
                for t in self.last_played_tracks.drain(..).chain(tracks) {
                    if !t.id.is_empty() && seen.insert(t.id.clone()) {
                        merged.push(t);
                        if merged.len() >= 50 {
                            break;
                        }
                    }
                }
                self.last_played_tracks = merged;
            }
            InternalEvent::RadioQueueReady { tracks, label } => {
                if tracks.is_empty() {
                    self.status_line = format!("Could not start {label}");
                    return;
                }
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                let _ = self
                    .session
                    .send(SessionCommand::PlayQueue {
                        tracks,
                        start_index: 0,
                        push_history: false,
                    })
                    .await;
                self.status_line = format!("Playing {label}");
            }
            InternalEvent::AutoplayTracksReady { play_seq, tracks } => {
                if play_seq != self.play_seq {
                    return;
                }
                if tracks.is_empty() {
                    self.playback.paused = true;
                    self.playback.loading_track = false;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    self.status_line = "Queue finished".to_string();
                    return;
                }
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                let mut combined = self.playback.queue.clone();
                let start_index = combined.len();
                let mut seen: HashSet<String> = combined.iter().map(|t| t.id.clone()).collect();
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
                    self.status_line = "Queue finished".to_string();
                    return;
                }
                let _ = self
                    .session
                    .send(SessionCommand::PlayQueue {
                        tracks: combined,
                        start_index,
                        push_history: false,
                    })
                    .await;
                self.status_line = "Playing similar tracks".to_string();
            }
            InternalEvent::AutoplayPrefetched { seed_key, tracks } => {
                if self.prefetched_autoplay_seed.as_deref() == Some(seed_key.as_str()) {
                    let urls: Vec<String> = tracks
                        .iter()
                        .filter_map(|t| t.thumbnail_url.clone())
                        .collect();
                    self.prefetch_thumbnails(urls);
                    self.prefetched_autoplay_tracks = tracks;
                }
            }
            InternalEvent::ArtistResolveFallback(query) => {
                self.artist_loading = false;
                self.album_loading = false;
                self.artist_page = None;
                self.execute_search(&query);
            }
            InternalEvent::AlbumDetailLoaded {
                req_id,
                album,
                error,
            } => {
                if req_id != self.album_req_id {
                    return;
                }
                self.album_loading = false;
                self.album_error = error;
                if let Some(ref alb) = album {
                    let mut urls = Vec::new();
                    if let Some(u) = &alb.cover_url {
                        urls.push(u.clone());
                    }
                    for v in &alb.other_versions {
                        if let Some(u) = &v.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    self.prefetch_thumbnails_urgent(urls);
                    self.active_nav = NavTarget::Album(alb.id.clone());
                    self.status_line = format!("Loaded {}", alb.title);
                }
                self.album_detail = album;
            }
            InternalEvent::SearchSuggestionsLoaded {
                req_id,
                suggestions,
            } => {
                if req_id == self.suggest_req_id {
                    self.search_suggestions = suggestions;
                }
            }
            InternalEvent::ArtistLoaded {
                req_id,
                artist,
                banner_url,
                error,
            } => {
                if req_id != self.artist_req_id {
                    return;
                }
                self.artist_loading = false;
                self.artist_error = error;
                self.artist_banner_url = banner_url.clone();
                if let Some(ref page) = artist {
                    let mut urls = Vec::new();
                    if let Some(u) = banner_url {
                        urls.push(u);
                    }
                    if let Some(u) = &page.thumbnail_url {
                        urls.push(u.clone());
                    }
                    if let Some(u) = page
                        .latest_release
                        .as_ref()
                        .and_then(|r| r.thumbnail_url.clone())
                    {
                        urls.push(u);
                    }
                    for tr in &page.popular_tracks {
                        if let Some(u) = &tr.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    for rel in &page.releases {
                        if let Some(u) = &rel.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    for sim in &page.related_artists {
                        if let Some(u) = &sim.thumbnail_url {
                            urls.push(u.clone());
                        }
                    }
                    self.prefetch_thumbnails_urgent(urls);
                }
                self.artist_page = artist;
                self.status_line = self
                    .artist_error
                    .as_ref()
                    .map(|error| format!("Artist failed: {error}"))
                    .unwrap_or_else(|| "Artist ready".to_string());
            }
            InternalEvent::SearchLoaded {
                req_id,
                tracks,
                rich,
                error,
            } => {
                if req_id != self.search_req_id {
                    return;
                }
                self.search_loading = false;
                if let Some(err) = error {
                    self.status_line = format!("Search failed: {err}");
                    return;
                }
                let mut urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                for c in rich
                    .albums
                    .iter()
                    .chain(rich.artists.iter())
                    .chain(rich.playlists.iter())
                {
                    if let Some(u) = &c.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                for t in &rich.videos {
                    if let Some(u) = &t.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                self.prefetch_thumbnails_urgent(urls);
                let count = tracks.len();
                self.search_results = tracks;
                self.search_rich = rich;
                self.status_line = format!("Found {count} songs on YouTube Music");
            }
            InternalEvent::CollectionLoaded {
                req_id,
                id,
                heading,
                tracks,
                silent_replace,
            } => {
                if req_id != self.playlist_req_id {
                    return;
                }
                self.playlist_loading = false;
                self.playlist_id = Some(id);
                self.playlist_heading = heading;
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails_urgent(urls);
                let count = tracks.len();
                if self.playlist_id.as_deref() == Some("VLLM") {
                    self.liked_track_ids = tracks.iter().map(|t| t.id.clone()).collect();
                    if let Some(latest_thumb) = tracks.first().and_then(|t| t.thumbnail_url.clone())
                    {
                        self.playlist_cover_url = Some(latest_thumb.clone());
                        if let Some(pl) = self.ytm_playlists.iter_mut().find(|p| p.id == "VLLM") {
                            pl.thumbnail_url = Some(latest_thumb);
                        }
                    }
                } else {
                    for t in &tracks {
                        if t.liked {
                            self.liked_track_ids.insert(t.id.clone());
                        }
                    }
                }
                self.playlist_tracks = tracks;
                self.status_line = if silent_replace {
                    format!("Synced {count} tracks")
                } else {
                    format!("Loaded {count} tracks")
                };
            }
            InternalEvent::CollectionAppended { req_id, tracks } => {
                if req_id != self.playlist_req_id || tracks.is_empty() {
                    return;
                }
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails_urgent(urls);
                self.playlist_tracks.extend(tracks);
            }
            InternalEvent::LyricsReady {
                play_seq,
                track_id,
                lyrics,
            } => {
                if play_seq == self.play_seq {
                    if let Some(track) = self.playback.current_track().filter(|t| t.id == track_id)
                    {
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
            }
            InternalEvent::ArtworkReady {
                play_seq,
                track_id,
                url,
                art_path,
                ambient_rgb,
            } => {
                if play_seq == self.play_seq
                    && self.playback.current_track().is_some_and(|track| {
                        track.id == track_id
                            && (track.thumbnail_url.as_deref() == Some(url.as_str())
                                || track
                                    .thumbnail_url
                                    .as_deref()
                                    .map(media::upscale_artwork_url)
                                    .as_deref()
                                    == Some(url.as_str()))
                    })
                {
                    self.art_path = art_path;
                    self.ambient_rgb = ambient_rgb;
                }
            }
            InternalEvent::ArtworkEnhanced {
                play_seq,
                track_id,
                url,
                art_path,
                ambient_rgb,
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
                let _ = self
                    .session
                    .send(SessionCommand::UpdateTrackThumbnail {
                        track_id: track_id.clone(),
                        url,
                    })
                    .await;
                if self
                    .playback
                    .current_track()
                    .is_some_and(|track| track.id == track_id)
                {
                    self.art_path = art_path;
                    self.ambient_rgb = ambient_rgb;
                    self.sync_discord_rpc();
                }
            }
            InternalEvent::AnimatedArtworkReady {
                play_seq,
                track_id,
                cover,
            } => {
                if play_seq == self.play_seq
                    && !cover.frames_rgba.is_empty()
                    && self
                        .playback
                        .current_track()
                        .is_some_and(|track| track.id == track_id)
                {
                    self.motion_cover = Some(cover);
                }
            }
            InternalEvent::ThumbnailsReady => {
                self.thumb_rev = self.thumb_rev.wrapping_add(1);
            }
            InternalEvent::PlaylistCreated(pl) => {
                self.ytm_playlists.push(pl.clone());
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &self.ytm_playlists);
                }
                self.status_line = format!("Created playlist \"{}\"", pl.name);
            }
            InternalEvent::PlaylistUpdated {
                playlist_id,
                title,
                cover_updated,
            } => {
                if let Some(ref new_title) = title {
                    for pl in &mut self.ytm_playlists {
                        if pl.id == playlist_id
                            || pl.id.strip_prefix("VL") == playlist_id.strip_prefix("VL")
                        {
                            pl.name = new_title.clone();
                        }
                    }
                    if self.playlist_id.as_deref() == Some(playlist_id.as_str()) {
                        self.playlist_heading = new_title.clone();
                    }
                    if let Some(db) = &self.cache_db {
                        db.put_library("ytm", &self.ytm_playlists);
                    }
                }
                self.status_line = if cover_updated {
                    "Playlist & cover updated on YouTube Music".to_string()
                } else {
                    "Playlist updated on YouTube Music".to_string()
                };
                if let Some(id) = self.playlist_id.clone() {
                    let heading = self.playlist_heading.clone();
                    let subtitle = self.playlist_subtitle.clone();
                    let cover = self.playlist_cover_url.clone();
                    let editable = self.playlist_editable;
                    self.open_collection(id, heading, subtitle, cover, editable, true);
                }
                self.load_library_extras();
            }
            InternalEvent::UpdateChecked(info) => {
                self.checking_updates = false;
                if info.update_available {
                    self.status_line = format!("Update v{} available", info.latest_version);
                }
                self.update_info = Some(info);
            }
            InternalEvent::UpdateCheckFailed(err) => {
                self.checking_updates = false;
                tracing::warn!(error = %err, "desktop update check failed");
            }
            InternalEvent::DownloadsLoaded {
                tracks,
                collections,
                total_bytes,
            } => {
                let mut urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.artwork_url.clone())
                    .collect();
                for c in &collections {
                    if let Some(u) = &c.artwork_url {
                        urls.push(u.clone());
                    }
                }
                self.prefetch_thumbnails(urls);
                self.downloaded_track_ids = tracks.iter().map(|t| t.track_id.clone()).collect();
                self.downloaded_tracks = tracks;
                self.downloaded_collections = collections;
                self.downloads_total_bytes = total_bytes;
                if self.playlist_id.as_deref() == Some("SONATA:DOWNLOADS") {
                    self.playlist_tracks = self
                        .downloaded_tracks
                        .iter()
                        .map(|d| d.to_track())
                        .collect();
                }
            }
            InternalEvent::TrackDownloadFinished {
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
            InternalEvent::CollectionDownloadFinished {
                title,
                saved,
                errors,
            } => {
                self.refresh_downloads();
                self.status_line = if errors == 0 {
                    format!("Downloaded {saved} tracks from \"{title}\"")
                } else {
                    format!("Downloaded {saved} tracks from \"{title}\" ({errors} failed)")
                };
            }
            InternalEvent::ServerTested { server_id, result } => {
                if let Some(sid) = server_id {
                    let badge = match &result {
                        Ok(msg) => msg.clone(),
                        Err(err) => format!("Error: {err}"),
                    };
                    self.server_statuses.insert(sid, badge);
                }
                match &result {
                    Ok(msg) => self.status_line = format!("Server OK: {msg}"),
                    Err(err) => self.status_line = format!("Server connection failed: {err}"),
                }
                self.server_test_result = Some(result);
            }
            InternalEvent::ServerSaved {
                profiles,
                active_id,
                message,
            } => {
                self.server_profiles = profiles;
                self.active_server_id = Some(active_id.clone());
                self.server_test_result = Some(Ok(message.clone()));
                self.status_line = message;
                self.load_server_browse(active_id, None, None);
            }
            InternalEvent::ServerBrowseLoaded {
                req_id,
                server_id,
                folder_path,
                libraries,
                artists,
                albums,
                playlists,
                tracks,
                error,
            } => {
                if req_id != self.server_req_id {
                    return;
                }
                self.server_loading = false;
                self.active_server_id = Some(server_id.clone());
                self.server_current_folder = folder_path;
                if let Some(err) = error {
                    self.server_statuses
                        .insert(server_id, format!("Error: {err}"));
                    self.status_line = format!("Server error: {err}");
                    return;
                }
                self.server_statuses
                    .insert(server_id, "Connected".to_string());
                let mut urls = Vec::new();
                for c in artists.iter().chain(albums.iter()) {
                    if let Some(u) = &c.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                for p in &playlists {
                    if let Some(u) = &p.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                for t in &tracks {
                    if let Some(u) = &t.thumbnail_url {
                        urls.push(u.clone());
                    }
                }
                self.prefetch_thumbnails(urls);
                self.server_libraries = libraries;
                self.server_artists = artists;
                self.server_albums = albums;
                self.server_playlists = playlists;
                self.server_tracks = tracks;
            }
            InternalEvent::StatusMessage(msg) => {
                self.status_line = msg;
            }
        }
    }

    fn spawn_update_check(&mut self) {
        if self.checking_updates {
            return;
        }
        self.checking_updates = true;
        let http = self.http.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            match sonata_core::updater::check_for_updates_for(
                &http,
                sonata_core::updater::UpdateTarget::Desktop,
                sonata_core::updater::CURRENT_VERSION,
            )
            .await
            {
                Ok(info) => {
                    let _ = int_tx.send(InternalEvent::UpdateChecked(info));
                }
                Err(err) => {
                    let _ = int_tx.send(InternalEvent::UpdateCheckFailed(err.to_string()));
                }
            }
        });
    }

    async fn publish(&mut self, notify_integrations: bool) {
        self.export_now_playing();
        if self.config_dirty {
            match self.config.save() {
                Ok(()) => self.config_dirty = false,
                Err(err) => tracing::warn!(?err, "failed to save desktop config"),
            }
        }

        let snap = Arc::new(DesktopSnapshot {
            config: self.config.clone(),
            active_nav: self.active_nav.clone(),
            status_line: self.status_line.clone(),
            available_browsers: self.available_browsers.clone(),
            signed_in_browser: self.signed_in_browser.clone(),
            user_profile: self.user_profile.clone(),
            user_avatar_path: self.user_avatar_path.clone(),
            signing_in: self.signing_in,
            sign_in_error: self.sign_in_error.clone(),
            ytm_playlists: self.ytm_playlists.clone(),
            library_albums: self.library_albums.clone(),
            library_artists: self.library_artists.clone(),
            library_extras_loading: self.library_extras_loading,
            home_chips: self.home_chips.clone(),
            active_home_chip: self.active_home_chip.clone(),
            home_shelves: self.home_shelves.clone(),
            home_loading: self.home_loading,
            home_error: self.home_error.clone(),
            explore_shelves: self.explore_shelves.clone(),
            mood_tiles: self.mood_tiles.clone(),
            explore_loading: self.explore_loading,
            mood_page_title: self.mood_page_title.clone(),
            mood_page_shelves: self.mood_page_shelves.clone(),
            mood_page_loading: self.mood_page_loading,
            shelf_detail: self.shelf_detail.clone(),
            radio_shelves: self.radio_shelves.clone(),
            radio_loading: self.radio_loading,
            history_tracks: self.history_tracks.clone(),
            history_loading: self.history_loading,
            last_played_tracks: self.last_played_tracks.clone(),
            recent_searches: self.recent_searches.clone(),
            search_suggestions: self.search_suggestions.clone(),
            search_query: self.search_query.clone(),
            search_results: self.search_results.clone(),
            search_rich: self.search_rich.clone(),
            search_loading: self.search_loading,
            playlist_id: self.playlist_id.clone(),
            playlist_heading: self.playlist_heading.clone(),
            playlist_subtitle: self.playlist_subtitle.clone(),
            playlist_cover_url: self.playlist_cover_url.clone(),
            playlist_editable: self.playlist_editable,
            playlist_tracks: self.playlist_tracks.clone(),
            playlist_loading: self.playlist_loading,
            album_detail: self.album_detail.clone(),
            album_loading: self.album_loading,
            album_error: self.album_error.clone(),
            artist_page: self.artist_page.clone(),
            artist_banner_url: self.artist_banner_url.clone(),
            artist_loading: self.artist_loading,
            artist_error: self.artist_error.clone(),
            liked_track_ids: self.liked_track_ids.clone(),
            queue: self.playback.queue.clone(),
            current_index: self.playback.current_index,
            paused: self.playback.paused,
            muted: self.muted,
            position_secs: self.playback.position_secs,
            duration_secs: self.playback.duration_secs,
            volume: self.playback.volume,
            shuffle: self.playback.shuffle,
            repeat: self.playback.repeat,
            autoplay: self.config.autoplay,
            stream_badge: self.playback.stream_badge.clone(),
            upgrading_quality: self.upgrading_quality,
            loading_track: self.playback.loading_track,
            lyrics: self.lyrics.clone(),
            art_path: self.art_path.clone(),
            motion_cover: self.motion_cover.clone(),
            ambient_rgb: self.ambient_rgb,
            spotify_notice: self.spotify_notice.clone(),
            update_info: self.update_info.clone(),
            checking_updates: self.checking_updates,
            thumb_rev: self.thumb_rev,
            downloaded_tracks: self.downloaded_tracks.clone(),
            downloaded_collections: self.downloaded_collections.clone(),
            downloaded_track_ids: self.downloaded_track_ids.clone(),
            downloading_track_ids: self.downloading_track_ids.clone(),
            downloads_total_bytes: self.downloads_total_bytes,
            discord_status: self.discord_rpc.status(),
            server_profiles: self.server_profiles.clone(),
            server_statuses: self.server_statuses.clone(),
            active_server_id: self.active_server_id.clone(),
            server_libraries: self.server_libraries.clone(),
            server_artists: self.server_artists.clone(),
            server_albums: self.server_albums.clone(),
            server_playlists: self.server_playlists.clone(),
            server_tracks: self.server_tracks.clone(),
            server_current_folder: self.server_current_folder.clone(),
            server_loading: self.server_loading,
            server_test_result: self.server_test_result.clone(),
        });

        if self.window_visible {
            let _ = self
                .progress_tx
                .send((self.playback.position_secs, self.playback.duration_secs));
            let _ = self.snap_tx.send(snap.clone());
        }
        let _ = self.mpris_tx.send(MprisUpdate::Sync(snap));

        if notify_integrations {
            let cur = self.playback.current_track();
            let tray_state = TrayState {
                title: cur.map(|t| t.title.clone()),
                artist: cur.map(|t| t.artist.clone()),
                paused: self.playback.paused,
                has_track: cur.is_some(),
                window_visible: self.window_visible,
            };
            if tray_state != self.last_tray_state {
                self.last_tray_state = tray_state.clone();
                if let Some(tray) = &self.tray_handle {
                    tray.update(tray_state).await;
                }
            }
        }
    }

    async fn shutdown(&mut self) {
        if let Some(addr) = &self.hyprland_window {
            hyprland::set_window_opacity_sync(addr, 1.0);
        }
        media::clear_now_playing();
        if let Err(err) = self.config.save() {
            tracing::warn!(?err, "failed to save desktop config on shutdown");
        }
        if !self.media_tasks.shutdown(Duration::from_secs(2)).await {
            tracing::warn!("desktop media tasks exceeded shutdown timeout and were aborted");
        }
        self.discord_rpc.shutdown().await;
        let report = self.session.shutdown().await;
        if !report.tasks_joined_cleanly {
            tracing::warn!("playback session tasks exceeded shutdown timeout and were aborted");
        }
        if let Some(tray) = self.tray_handle.take() {
            tray.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::player::PlaybackGeneration;
    use std::time::Instant;

    #[test]
    fn natural_track_end_requires_near_duration() {
        assert!(PlaybackSession::is_natural_track_end(178.0, 180.0));
        assert!(!PlaybackSession::is_natural_track_end(12.0, 180.0));
        assert!(!PlaybackSession::is_natural_track_end(0.0, 0.0));
    }

    #[test]
    fn loading_or_recent_streams_ignore_stale_errors() {
        assert!(PlaybackSession::should_ignore_stale_track_end(
            true,
            None,
            Duration::from_millis(750),
        ));
        assert!(PlaybackSession::should_ignore_stale_track_end(
            false,
            None,
            Duration::from_millis(750),
        ));
        assert!(PlaybackSession::should_ignore_stale_track_end(
            false,
            Some(Instant::now()),
            Duration::from_millis(750),
        ));
    }

    #[test]
    fn one_recovery_attempt_per_play_sequence() {
        assert!(PlaybackSession::should_attempt_recovery(
            None,
            PlaybackGeneration::new(7)
        ));
        assert!(!PlaybackSession::should_attempt_recovery(
            Some(PlaybackGeneration::new(7)),
            PlaybackGeneration::new(7)
        ));
    }

    #[test]
    fn shutdown_sync_receives_ack_without_polling() {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let (_win_tx, _win_rx) = mpsc::unbounded_channel::<UiWindowCmd>();
        let (_snap_tx, snap_rx) = watch::channel(Arc::new(DesktopSnapshot {
            config: AppConfig::default(),
            active_nav: NavTarget::Home,
            status_line: String::new(),
            available_browsers: Vec::new(),
            signed_in_browser: None,
            user_profile: None,
            user_avatar_path: None,
            signing_in: false,
            sign_in_error: None,
            ytm_playlists: Vec::new(),
            library_albums: Vec::new(),
            library_artists: Vec::new(),
            library_extras_loading: false,
            home_chips: Vec::new(),
            active_home_chip: None,
            home_shelves: Vec::new(),
            home_loading: false,
            home_error: None,
            explore_shelves: Vec::new(),
            mood_tiles: Vec::new(),
            explore_loading: false,
            mood_page_title: String::new(),
            mood_page_shelves: Vec::new(),
            mood_page_loading: false,
            shelf_detail: None,
            radio_shelves: Vec::new(),
            radio_loading: false,
            history_tracks: Vec::new(),
            history_loading: false,
            last_played_tracks: Vec::new(),
            recent_searches: Vec::new(),
            search_suggestions: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_rich: RichSearchResult::default(),
            search_loading: false,
            playlist_id: None,
            playlist_heading: String::new(),
            playlist_subtitle: String::new(),
            playlist_cover_url: None,
            playlist_editable: false,
            playlist_tracks: Vec::new(),
            playlist_loading: false,
            album_detail: None,
            album_loading: false,
            album_error: None,
            artist_page: None,
            artist_banner_url: None,
            artist_loading: false,
            artist_error: None,
            liked_track_ids: HashSet::new(),
            queue: Vec::new(),
            current_index: None,
            paused: true,
            muted: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 80,
            shuffle: false,
            repeat: sonata_core::player::RepeatMode::Off,
            autoplay: true,
            stream_badge: String::new(),
            upgrading_quality: false,
            loading_track: false,
            lyrics: LyricsState::Idle,
            art_path: None,
            motion_cover: None,
            ambient_rgb: (0, 0, 0),
            spotify_notice: None,
            update_info: None,
            checking_updates: false,
            thumb_rev: 0,
            downloaded_tracks: Vec::new(),
            downloaded_collections: Vec::new(),
            downloaded_track_ids: HashSet::new(),
            downloading_track_ids: HashSet::new(),
            downloads_total_bytes: 0,
            discord_status: DiscordRpcStatus::Disconnected,
            server_profiles: Vec::new(),
            server_statuses: HashMap::new(),
            active_server_id: None,
            server_libraries: Vec::new(),
            server_artists: Vec::new(),
            server_albums: Vec::new(),
            server_playlists: Vec::new(),
            server_tracks: Vec::new(),
            server_current_folder: None,
            server_loading: false,
            server_test_result: None,
        }));
        let (_progress_tx, progress_rx) = watch::channel((0.0_f64, 0.0_f64));
        let handle = BackendHandle {
            cmd_tx,
            snap_rx,
            progress_rx,
        };
        let responder = std::thread::spawn(move || {
            if let Some(BackendCmd::Shutdown(done)) = cmd_rx.blocking_recv() {
                let _ = done.try_send(());
            }
        });
        handle.shutdown_sync();
        responder.join().expect("responder joined");
    }
}
