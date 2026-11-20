use crate::integrations::mpris::{spawn_mpris, MprisUpdate};
use crate::integrations::tray::{spawn_tray, TrayHandle, TrayState};
use reqwest::Client;
use sonata_core::auth::{
    extract_ytm_cookies_from_browser, list_supported_browsers, BrowserOption, ExtractedCookies,
};
use sonata_core::cache::{
    enforce_active_media_cache_limit_async, start_media_cache_maintenance, CacheDb,
};
use sonata_core::config::{AppConfig, EnhancerMode};
use sonata_core::hyprland;
use sonata_core::lyrics::{active_line_index, lrclib, LyricsState};
use sonata_core::media;
use sonata_core::player::{MpvController, MpvEvent, PlaybackState, RepeatMode};
use sonata_core::providers::hifi::{ResolvedStream, StreamResolver};
use sonata_core::providers::ytmusic::{HomeShelf, YtAccountProfile, YtMusicClient};
use sonata_core::providers::{build_http_client, Playlist, ProviderSource, Track};
use sonata_core::theme::Theme;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, watch, Semaphore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavTarget {
    Home,
    Search,
    Queue,
    Playlist(String),
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiWindowCmd {
    ShowWindow,
    HideWindow,
    ToggleWindow,
    QuitApp,
}

#[derive(Debug)]
pub enum BackendCmd {
    Navigate(NavTarget),
    OpenCollection {
        id: String,
        title: String,
        subtitle: String,
        thumbnail_url: Option<String>,
        editable: bool,
    },
    RefreshActivePlaylist,
    RefreshHome,
    RefreshAccount,
    SignInWithBrowser(String),
    ClearSignInError,
    SignOut,
    Search(String),
    PlayTracks {
        tracks: Vec<Track>,
        start_index: usize,
        shuffle: bool,
    },
    PlayQueueIndex(usize),
    TogglePause,
    SetPaused(bool),
    Stop,
    NextTrack,
    PrevTrack,
    SeekAbsolute(f64),
    SeekRelative(f64),
    SetVolume(u8),
    ToggleMute,
    ToggleShuffle,
    SetShuffle(bool),
    CycleRepeat,
    SetRepeat(RepeatMode),
    CycleEnhancer,
    ToggleHiFi,
    StepCacheLimit(bool),
    ToggleTransparentBg,
    StepHyprlandOpacity(bool),
    SelectTheme(String),
    ToggleLike(Track),
    CreateYtmPlaylist(String),
    AddTrackToPlaylist {
        track: Track,
        playlist_id: String,
        playlist_name: String,
    },
    RemoveTrackFromPlaylist(usize),
    ClickLinkSpotify,
    SetWindowAddress(Option<String>),
    SetWindowVisible(bool),
    Shutdown(oneshot::Sender<()>),
}

#[derive(Debug, Clone)]
pub struct DesktopSnapshot {
    pub config: AppConfig,
    pub themes: Vec<Theme>,
    pub base_theme: Theme,
    pub active_nav: NavTarget,
    pub status_line: String,
    pub available_browsers: Vec<BrowserOption>,
    pub signed_in_browser: Option<String>,
    pub user_profile: Option<YtAccountProfile>,
    pub user_avatar_path: Option<PathBuf>,
    pub signing_in: bool,
    pub sign_in_error: Option<String>,
    pub ytm_playlists: Vec<Playlist>,
    pub home_shelves: Vec<HomeShelf>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub search_query: String,
    pub search_results: Vec<Track>,
    pub search_loading: bool,
    pub playlist_id: Option<String>,
    pub playlist_heading: String,
    pub playlist_subtitle: String,
    pub playlist_cover_url: Option<String>,
    pub playlist_editable: bool,
    pub playlist_tracks: Vec<Track>,
    pub playlist_loading: bool,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub paused: bool,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub enhancer: EnhancerMode,
    pub stream_badge: String,
    pub lyrics: LyricsState,
    pub art_path: Option<PathBuf>,
    pub ambient_rgb: (u8, u8, u8),
    pub spotify_notice: Option<String>,
}

impl DesktopSnapshot {
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    pub fn is_signed_in(&self) -> bool {
        self.signed_in_browser.is_some()
    }

    pub fn effective_transparent(&self) -> bool {
        self.base_theme
            .transparent_bg
            .unwrap_or(self.config.transparent_bg)
    }

    pub fn effective_hyprland_opacity(&self) -> f32 {
        self.base_theme
            .hyprland_opacity
            .unwrap_or(self.config.hyprland_opacity)
            .clamp(0.50, 1.00)
    }

    pub fn active_lyric_index(&self) -> Option<usize> {
        match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, self.position_secs),
            _ => None,
        }
    }

    pub fn active_lyric_line(&self) -> Option<&str> {
        match &self.lyrics {
            LyricsState::Synced(lines) => self
                .active_lyric_index()
                .and_then(|idx| lines.get(idx))
                .map(|l| l.text.as_str())
                .filter(|s| !s.is_empty()),
            _ => None,
        }
    }

    pub fn writable_ytm_playlists(&self) -> Vec<Playlist> {
        self.ytm_playlists
            .iter()
            .filter(|p| p.source == ProviderSource::YouTubeMusic && p.id != "VLLM")
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
pub struct BackendHandle {
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    win_tx: mpsc::UnboundedSender<UiWindowCmd>,
    snap_rx: watch::Receiver<Arc<DesktopSnapshot>>,
}

impl BackendHandle {
    pub fn send(&self, cmd: BackendCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn send_window(&self, cmd: UiWindowCmd) {
        let _ = self.win_tx.send(cmd);
    }

    pub fn snapshot(&self) -> Arc<DesktopSnapshot> {
        self.snap_rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<DesktopSnapshot>> {
        self.snap_rx.clone()
    }

    pub fn shutdown_sync(&self) {
        let (tx, mut rx) = oneshot::channel();
        if self.cmd_tx.send(BackendCmd::Shutdown(tx)).is_ok() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if rx.try_recv().is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
        }
    }
}

enum InternalEvent {
    SignInFinished {
        browser_id: String,
        cookies: ExtractedCookies,
        profile: Option<YtAccountProfile>,
        avatar_path: Option<PathBuf>,
        ytm_playlists: Vec<Playlist>,
        error: Option<String>,
    },
    AccountRefreshed {
        profile: Option<YtAccountProfile>,
        avatar_path: Option<PathBuf>,
        ytm_playlists: Vec<Playlist>,
    },
    HomeLoaded {
        req_id: u64,
        shelves: Vec<HomeShelf>,
        error: Option<String>,
    },
    SearchLoaded {
        req_id: u64,
        tracks: Vec<Track>,
        error: Option<String>,
    },
    CollectionLoaded {
        req_id: u64,
        id: String,
        heading: String,
        tracks: Vec<Track>,
        silent_replace: bool,
    },
    CollectionAppended {
        req_id: u64,
        tracks: Vec<Track>,
    },
    StreamReady {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamUpgrade {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamError {
        play_seq: u64,
        track_id: String,
        error: String,
    },
    NextTrackPrepared {
        play_seq: u64,
        index: usize,
        track_id: String,
        result: Result<ResolvedStream, String>,
    },
    LyricsReady {
        play_seq: u64,
        track_id: String,
        lyrics: LyricsState,
    },
    ArtworkReady {
        play_seq: u64,
        track_id: String,
        url: String,
        art_path: Option<PathBuf>,
        ambient_rgb: (u8, u8, u8),
    },
    ArtworkEnhanced {
        play_seq: u64,
        track_id: String,
        url: String,
        art_path: Option<PathBuf>,
        ambient_rgb: (u8, u8, u8),
    },
    ThumbnailsReady,
    PlaylistCreated(Playlist),
    StatusMessage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportFingerprint {
    track_id: String,
    status: String,
    lyric: Option<String>,
    art_path: Option<String>,
    pos_secs: u64,
    dur_secs: u64,
}

pub fn spawn_backend(win_tx: mpsc::UnboundedSender<UiWindowCmd>) -> anyhow::Result<BackendHandle> {
    let config = AppConfig::load();
    let themes = Theme::load_all(&AppConfig::themes_dir());
    let base_theme = themes
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&config.theme))
        .cloned()
        .unwrap_or_default();
    let available_browsers = list_supported_browsers();

    let cache_db = CacheDb::open_default();
    let initial_browser = config.desktop_ytm_browser.clone();
    let ytm_playlists = if initial_browser.is_some() {
        cache_db
            .as_ref()
            .and_then(|db| db.get_library("ytm"))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut playback = PlaybackState::new(config.volume, config.enhancer_mode);
    playback.shuffle = config.shuffle;
    playback.repeat = config.repeat_mode;

    let initial_snapshot = Arc::new(DesktopSnapshot {
        config: config.clone(),
        themes: themes.clone(),
        base_theme: base_theme.clone(),
        active_nav: NavTarget::Home,
        status_line: "Loading YouTube Music home...".to_string(),
        available_browsers: available_browsers.clone(),
        signed_in_browser: initial_browser.clone(),
        user_profile: None,
        user_avatar_path: None,
        signing_in: false,
        sign_in_error: None,
        ytm_playlists: ytm_playlists.clone(),
        home_shelves: Vec::new(),
        home_loading: true,
        home_error: None,
        search_query: String::new(),
        search_results: Vec::new(),
        search_loading: false,
        playlist_id: None,
        playlist_heading: String::new(),
        playlist_subtitle: String::new(),
        playlist_cover_url: None,
        playlist_editable: false,
        playlist_tracks: Vec::new(),
        playlist_loading: false,
        queue: Vec::new(),
        current_index: None,
        paused: true,
        muted: false,
        position_secs: 0.0,
        duration_secs: 0.0,
        volume: config.volume,
        shuffle: config.shuffle,
        repeat: config.repeat_mode,
        enhancer: config.enhancer_mode,
        stream_badge: "Idle".to_string(),
        lyrics: LyricsState::Idle,
        art_path: None,
        ambient_rgb: (110, 90, 170),
        spotify_notice: None,
    });

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (snap_tx, snap_rx) = watch::channel(initial_snapshot.clone());
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
                themes,
                base_theme,
                cache_db,
                available_browsers,
                initial_browser,
                ytm_playlists,
                playback,
                cmd_tx_thread,
                cmd_rx,
                snap_tx,
                mpris_tx,
                win_tx_thread,
            ));
        })?;

    Ok(BackendHandle {
        cmd_tx,
        win_tx,
        snap_rx,
    })
}

struct BackendEngine {
    config: AppConfig,
    themes: Vec<Theme>,
    base_theme: Theme,
    http: Client,
    cookies: ExtractedCookies,
    ytm: YtMusicClient,
    resolver: StreamResolver,
    cache_db: Option<CacheDb>,
    mpv: Option<Arc<MpvController>>,
    int_tx: mpsc::UnboundedSender<InternalEvent>,
    snap_tx: watch::Sender<Arc<DesktopSnapshot>>,
    mpris_tx: mpsc::UnboundedSender<MprisUpdate>,
    tray_handle: Option<TrayHandle>,
    last_tray_state: TrayState,
    window_visible: bool,
    hyprland_window: Option<String>,
    active_nav: NavTarget,
    status_line: String,
    available_browsers: Vec<BrowserOption>,
    signed_in_browser: Option<String>,
    user_profile: Option<YtAccountProfile>,
    user_avatar_path: Option<PathBuf>,
    signing_in: bool,
    sign_in_error: Option<String>,
    ytm_playlists: Vec<Playlist>,
    home_shelves: Vec<HomeShelf>,
    home_loading: bool,
    home_error: Option<String>,
    search_query: String,
    search_results: Vec<Track>,
    search_loading: bool,
    playlist_id: Option<String>,
    playlist_heading: String,
    playlist_subtitle: String,
    playlist_cover_url: Option<String>,
    playlist_editable: bool,
    playlist_tracks: Vec<Track>,
    playlist_loading: bool,
    playback: PlaybackState,
    muted: bool,
    lyrics: LyricsState,
    art_path: Option<PathBuf>,
    ambient_rgb: (u8, u8, u8),
    spotify_notice: Option<String>,
    pos_anchor: Option<(f64, Instant)>,
    stream_loaded_at: Option<Instant>,
    home_req_id: u64,
    search_req_id: u64,
    playlist_req_id: u64,
    play_seq: u64,
    retried_play_seq: u64,
    active_stream_task: Option<tokio::task::AbortHandle>,
    active_upgrade_task: Option<tokio::task::AbortHandle>,
    next_prefetch_task: Option<(String, tokio::task::JoinHandle<Option<ResolvedStream>>)>,
    pending_auto_advance: Option<(u64, String)>,
    awaiting_playback_start: bool,
    upgrade_started_play_seq: Option<u64>,
    cover_started_play_seq: Option<u64>,
    config_dirty: bool,
    last_exported_state: Option<ExportFingerprint>,
    in_flight_thumbs: Arc<Mutex<HashSet<String>>>,
}

#[allow(clippy::too_many_arguments)]
async fn run_backend_loop(
    config: AppConfig,
    themes: Vec<Theme>,
    base_theme: Theme,
    cache_db: Option<CacheDb>,
    available_browsers: Vec<BrowserOption>,
    initial_browser: Option<String>,
    ytm_playlists: Vec<Playlist>,
    playback: PlaybackState,
    cmd_tx: mpsc::UnboundedSender<BackendCmd>,
    mut cmd_rx: mpsc::UnboundedReceiver<BackendCmd>,
    snap_tx: watch::Sender<Arc<DesktopSnapshot>>,
    mpris_tx: mpsc::UnboundedSender<MprisUpdate>,
    win_tx: mpsc::UnboundedSender<UiWindowCmd>,
) {
    // Cache maintenance uses Tokio tasks, so start it inside the backend runtime.
    start_media_cache_maintenance();
    tokio::spawn(async {
        enforce_active_media_cache_limit_async().await;
    });

    let http = build_http_client();
    let cookies = ExtractedCookies::default();
    let ytm = YtMusicClient::new(http.clone(), cookies.clone());
    let resolver =
        StreamResolver::new(http.clone(), ytm.clone(), cookies.clone(), cache_db.clone());
    let warm_resolver = resolver.clone();
    tokio::spawn(async move {
        warm_resolver.warm().await;
    });

    let (mpv_tx, mut mpv_rx) = mpsc::unbounded_channel();
    let (mpv, initial_status) =
        match MpvController::spawn_desktop(config.volume, config.enhancer_mode, mpv_tx).await {
            Ok(ctrl) => (Some(Arc::new(ctrl)), "Loading YouTube Music...".to_string()),
            Err(e) => (None, format!("Audio warning: {e}")),
        };

    let (int_tx, mut int_rx) = mpsc::unbounded_channel();
    let tray_handle = spawn_tray(cmd_tx, win_tx).await;

    let mut engine = BackendEngine {
        config,
        themes,
        base_theme,
        http,
        cookies,
        ytm,
        resolver,
        cache_db,
        mpv,
        int_tx,
        snap_tx,
        mpris_tx,
        tray_handle,
        last_tray_state: TrayState::default(),
        window_visible: true,
        hyprland_window: None,
        active_nav: NavTarget::Home,
        status_line: initial_status,
        available_browsers,
        signed_in_browser: initial_browser.clone(),
        user_profile: None,
        user_avatar_path: None,
        signing_in: false,
        sign_in_error: None,
        ytm_playlists,
        home_shelves: Vec::new(),
        home_loading: true,
        home_error: None,
        search_query: String::new(),
        search_results: Vec::new(),
        search_loading: false,
        playlist_id: None,
        playlist_heading: String::new(),
        playlist_subtitle: String::new(),
        playlist_cover_url: None,
        playlist_editable: false,
        playlist_tracks: Vec::new(),
        playlist_loading: false,
        playback,
        muted: false,
        lyrics: LyricsState::Idle,
        art_path: None,
        ambient_rgb: (110, 90, 170),
        spotify_notice: None,
        pos_anchor: None,
        stream_loaded_at: None,
        home_req_id: 0,
        search_req_id: 0,
        playlist_req_id: 0,
        play_seq: 0,
        retried_play_seq: 0,
        active_stream_task: None,
        active_upgrade_task: None,
        next_prefetch_task: None,
        pending_auto_advance: None,
        awaiting_playback_start: false,
        upgrade_started_play_seq: None,
        cover_started_play_seq: None,
        config_dirty: false,
        last_exported_state: None,
        in_flight_thumbs: Arc::new(Mutex::new(HashSet::new())),
    };

    if let Some(browser_id) = initial_browser {
        engine.start_sign_in(browser_id);
    } else {
        engine.load_home_feed();
    }
    engine.publish(true).await;

    let mut smooth_tick = tokio::time::interval(Duration::from_millis(40));
    smooth_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let active_play = !engine.playback.paused
            && !engine.playback.loading_track
            && engine.playback.current_track().is_some();

        tokio::select! {
            _ = smooth_tick.tick(), if active_play => {
                if engine.tick_playback() {
                    engine.publish(false).await;
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                if let BackendCmd::Shutdown(done) = cmd {
                    engine.shutdown().await;
                    let _ = done.send(());
                    break;
                }
                engine.handle_command(cmd).await;
                while let Ok(more) = cmd_rx.try_recv() {
                    if let BackendCmd::Shutdown(done) = more {
                        engine.shutdown().await;
                        let _ = done.send(());
                        return;
                    }
                    engine.handle_command(more).await;
                }
                engine.publish(true).await;
            }
            Some(mpv_ev) = mpv_rx.recv() => {
                engine.handle_mpv_event(mpv_ev);
                while let Ok(more) = mpv_rx.try_recv() {
                    engine.handle_mpv_event(more);
                }
                engine.publish(true).await;
            }
            Some(ev) = int_rx.recv() => {
                engine.handle_internal_event(ev);
                while let Ok(more) = int_rx.try_recv() {
                    engine.handle_internal_event(more);
                }
                engine.publish(true).await;
            }
            else => break,
        }
    }
}

impl BackendEngine {
    fn is_natural_track_end(position_secs: f64, duration_secs: f64) -> bool {
        if duration_secs <= 0.0 {
            return false;
        }
        position_secs >= (duration_secs - 2.5).max(0.0)
    }

    fn should_ignore_stale_playback_error(
        loading_track: bool,
        stream_loaded_at: Option<Instant>,
        grace_elapsed: Duration,
    ) -> bool {
        loading_track || stream_loaded_at.is_none_or(|loaded| loaded.elapsed() < grace_elapsed)
    }

    fn should_attempt_recovery(retried_play_seq: u64, play_seq: u64) -> bool {
        retried_play_seq != play_seq
    }

    fn start_hifi_upgrade(&mut self) {
        if self.playback.paused
            || self.playback.loading_track
            || self.upgrade_started_play_seq == Some(self.play_seq)
        {
            return;
        }
        let (Some(track), Some(stream)) = (
            self.playback.current_track().cloned(),
            self.playback.active_stream.clone(),
        ) else {
            return;
        };
        if track.source == ProviderSource::Spotify
            && (!self.config.hifi_substitution
                || !StreamResolver::needs_hifi_upgrade(&stream.badge, &stream.url))
        {
            return;
        }

        let play_seq = self.play_seq;
        self.upgrade_started_play_seq = Some(play_seq);
        let resolver = self.resolver.clone();
        let int_tx = self.int_tx.clone();
        let allow_hifi = self.config.hifi_substitution;
        let join = tokio::spawn(async move {
            if let Some(upgraded) = resolver
                .resolve_hifi_upgrade(&track, &stream, allow_hifi)
                .await
            {
                let _ = int_tx.send(InternalEvent::StreamUpgrade {
                    play_seq,
                    track_id: track.id,
                    stream: upgraded,
                });
            }
        });
        self.active_upgrade_task = Some(join.abort_handle());
    }

    fn start_cover_enrichment(&mut self) {
        if self.cover_started_play_seq == Some(self.play_seq) {
            return;
        }
        let Some(track) = self.playback.current_track().cloned() else {
            return;
        };
        if track.source != ProviderSource::YouTubeMusic {
            return;
        }

        let play_seq = self.play_seq;
        self.cover_started_play_seq = Some(play_seq);
        let http = self.http.clone();
        let cache_db = self.cache_db.clone();
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let Some((url, _source)) =
                media::resolve_track_cover(&http, cache_db.as_ref(), &track).await
            else {
                return;
            };
            let Some((image, _)) = media::fetch_and_decode_art(&http, &url).await else {
                return;
            };
            let ambient_rgb = media::extract_ambient_rgb(&image);
            let path = media::art_cache_path(&url);
            let _ = int_tx.send(InternalEvent::ArtworkEnhanced {
                play_seq,
                track_id: track.id,
                url,
                art_path: path.exists().then_some(path),
                ambient_rgb,
            });
        });
    }

    fn interpolated_position(&self) -> f64 {
        let pos = if let Some((base, at)) = self.pos_anchor {
            if !self.playback.paused && !self.playback.loading_track {
                base + at.elapsed().as_secs_f64()
            } else {
                base
            }
        } else {
            self.playback.position_secs
        };
        if self.playback.duration_secs > 0.0 {
            pos.clamp(0.0, self.playback.duration_secs)
        } else {
            pos.max(0.0)
        }
    }

    fn tick_playback(&mut self) -> bool {
        if self.playback.paused
            || self.playback.loading_track
            || self.playback.current_track().is_none()
        {
            return false;
        }
        let next = self.interpolated_position();
        if (next - self.playback.position_secs).abs() >= 0.03 {
            self.playback.position_secs = next;
            true
        } else {
            false
        }
    }

    fn prefetch_thumbnails(&self, urls: Vec<String>) {
        let mut candidates = Vec::new();
        {
            let Ok(mut in_flight) = self.in_flight_thumbs.lock() else {
                return;
            };
            for url in urls {
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
        tokio::spawn(async move {
            let sem = Arc::new(Semaphore::new(6));
            let (done_tx, mut done_rx) = mpsc::unbounded_channel::<()>();
            for url in candidates {
                let sem = sem.clone();
                let http = http.clone();
                let in_flight = in_flight.clone();
                let done_tx = done_tx.clone();
                tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    let _ = media::fetch_and_decode_art(&http, &url).await;
                    if let Ok(mut guard) = in_flight.lock() {
                        guard.remove(&url);
                    }
                    let _ = done_tx.send(());
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

    fn start_sign_in(&mut self, browser_id: String) {
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

    fn refresh_account(&mut self) {
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

    fn sign_out(&mut self) {
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

    fn load_home_feed(&mut self) {
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

    fn open_collection(
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

    fn execute_search(&mut self, query: &str) {
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

    fn abort_active_stream_tasks(&mut self) {
        if let Some(handle) = self.active_stream_task.take() {
            handle.abort();
        }
        if let Some(handle) = self.active_upgrade_task.take() {
            handle.abort();
        }
    }

    fn play_track_at(&mut self, idx: usize, push_history: bool) {
        self.play_track_with_stream(idx, push_history, None);
    }

    fn play_track_with_stream(
        &mut self,
        idx: usize,
        push_history: bool,
        prepared_stream: Option<ResolvedStream>,
    ) {
        let Some(track) = self.playback.queue.get(idx).cloned() else {
            return;
        };
        self.abort_active_stream_tasks();
        self.playback.commit_play(idx, push_history);
        self.playback.loading_track = true;
        self.playback.paused = true;
        self.playback.position_secs = 0.0;
        self.pos_anchor = None;
        self.stream_loaded_at = None;
        self.retried_play_seq = 0;
        self.upgrade_started_play_seq = None;
        self.playback.duration_secs = track.duration_secs.unwrap_or(0) as f64;
        self.playback.stream_badge = "Resolving...".to_string();
        self.lyrics = LyricsState::Loading;

        if let Some(url) = &track.thumbnail_url {
            let cached = media::art_cache_path(url);
            self.art_path = cached.exists().then_some(cached);
        } else {
            self.art_path = None;
        }

        if let Some(mpv) = &self.mpv {
            mpv.stop();
        }

        self.play_seq = self.play_seq.wrapping_add(1);
        let play_seq = self.play_seq;
        self.awaiting_playback_start = false;
        self.pending_auto_advance = None;
        self.cover_started_play_seq = None;
        self.status_line = format!("Loading: {} — {}", track.title, track.artist);

        let resolver = self.resolver.clone();
        let http = self.http.clone();
        let ytm = self.ytm.clone();
        let int_tx = self.int_tx.clone();

        let track_for_stream = track.clone();
        if let Some(stream) = prepared_stream {
            let _ = int_tx.send(InternalEvent::StreamReady {
                play_seq,
                track_id: track_for_stream.id.clone(),
                stream,
            });
        } else {
            let stream_tx = int_tx.clone();
            let join = tokio::spawn(async move {
                match resolver.resolve_fast(&track_for_stream).await {
                    Ok(stream) => {
                        let _ = stream_tx.send(InternalEvent::StreamReady {
                            play_seq,
                            track_id: track_for_stream.id,
                            stream,
                        });
                    }
                    Err(e) => {
                        let _ = stream_tx.send(InternalEvent::StreamError {
                            play_seq,
                            track_id: track_for_stream.id,
                            error: e.to_string(),
                        });
                    }
                }
            });
            self.active_stream_task = Some(join.abort_handle());
        }

        let lyrics_track = track.clone();
        let lyrics_http = http.clone();
        let lyrics_cache = self.cache_db.clone();
        let lyrics_tx = int_tx.clone();
        tokio::spawn(async move {
            let lyrics =
                lrclib::fetch_lyrics(&lyrics_http, &ytm, &lyrics_track, lyrics_cache.as_ref())
                    .await;
            let _ = lyrics_tx.send(InternalEvent::LyricsReady {
                play_seq,
                track_id: lyrics_track.id,
                lyrics,
            });
        });

        if let Some(url) = track.thumbnail_url.clone() {
            let art_tx = int_tx;
            let track_id = track.id.clone();
            tokio::spawn(async move {
                if let Some((img, _)) = media::fetch_and_decode_art(&http, &url).await {
                    let ambient = media::extract_ambient_rgb(&img);
                    let path = media::art_cache_path(&url);
                    let _ = art_tx.send(InternalEvent::ArtworkReady {
                        play_seq,
                        track_id,
                        url: url.clone(),
                        art_path: path.exists().then_some(path),
                        ambient_rgb: ambient,
                    });
                }
            });
        }
    }

    fn prefetch_next(&mut self) {
        let Some(next_idx) = self.playback.peek_next_index() else {
            return;
        };
        let Some(next_track) = self.playback.queue.get(next_idx).cloned() else {
            return;
        };
        if self
            .next_prefetch_task
            .as_ref()
            .is_some_and(|(track_id, _)| track_id == &next_track.id)
        {
            return;
        }
        if let Some((_, task)) = self.next_prefetch_task.take() {
            task.abort();
        }
        let resolver = self.resolver.clone();
        let hifi = self.config.hifi_substitution;
        let http = self.http.clone();
        let art_url = next_track.thumbnail_url.clone();
        let track_id = next_track.id.clone();
        let task =
            tokio::spawn(async move { resolver.prefetch_hifi_audio(&next_track, hifi).await });
        self.next_prefetch_task = Some((track_id, task));
        if let Some(url) = art_url {
            tokio::spawn(async move {
                let _ = media::fetch_and_decode_art(&http, &url).await;
            });
        }
    }

    fn prepare_next_before_advance(&mut self, index: usize) {
        // Natural EOF waits for this task so the next stream can use its completed cache file.
        let Some(track) = self.playback.queue.get(index).cloned() else {
            return;
        };
        if self
            .pending_auto_advance
            .as_ref()
            .is_some_and(|(seq, id)| *seq == self.play_seq && id == &track.id)
        {
            return;
        }
        self.pending_auto_advance = Some((self.play_seq, track.id.clone()));
        self.playback.loading_track = true;
        self.playback.paused = true;
        self.playback.stream_badge = "Preparing next track...".to_string();
        self.pos_anchor = None;
        self.status_line = format!("Caching next track: {}", track.title);

        let matching_task = match self.next_prefetch_task.take() {
            Some((track_id, task)) if track_id == track.id => Some(task),
            Some((_, task)) => {
                task.abort();
                None
            }
            None => None,
        };
        let play_seq = self.play_seq;
        let resolver = self.resolver.clone();
        let allow_hifi = self.config.hifi_substitution;
        let int_tx = self.int_tx.clone();
        tokio::spawn(async move {
            let cached = match matching_task {
                Some(task) => task.await.ok().flatten(),
                None => resolver.prefetch_hifi_audio(&track, allow_hifi).await,
            };
            let result = match cached {
                Some(stream) => Ok(stream),
                None => resolver
                    .resolve_fast(&track)
                    .await
                    .map_err(|error| error.to_string()),
            };
            let _ = int_tx.send(InternalEvent::NextTrackPrepared {
                play_seq,
                index,
                track_id: track.id,
                result,
            });
        });
    }

    fn apply_theme_by_name(&mut self, name: &str) {
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

    fn sync_hyprland_opacity(&self) {
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

    fn apply_config_volume(&self) {
        let eff = if self.muted { 0 } else { self.config.volume };
        if let Some(mpv) = &self.mpv {
            mpv.set_volume(eff);
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
                self.load_home_feed();
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
                self.sign_out();
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
                self.playback.history.clear();
                self.playback.queue = tracks;
                let idx = if shuffle {
                    self.playback.shuffle = true;
                    self.config.shuffle = true;
                    let _ = self.config.save();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0);
                    nanos % self.playback.queue.len()
                } else {
                    start_index.min(self.playback.queue.len() - 1)
                };
                self.play_track_at(idx, false);
            }
            BackendCmd::PlayQueueIndex(idx) => {
                if idx < self.playback.queue.len() {
                    self.play_track_at(idx, true);
                }
            }
            BackendCmd::TogglePause => {
                self.set_paused(!self.playback.paused);
            }
            BackendCmd::SetPaused(paused) => {
                self.set_paused(paused);
            }
            BackendCmd::Stop => {
                self.abort_active_stream_tasks();
                self.playback.paused = true;
                self.playback.loading_track = false;
                self.playback.position_secs = 0.0;
                self.pos_anchor = None;
                self.playback.stream_badge = "Stopped".to_string();
                if let Some(mpv) = &self.mpv {
                    mpv.stop();
                }
            }
            BackendCmd::NextTrack => {
                if let Some(next) = self.playback.peek_next_index() {
                    self.play_track_at(next, true);
                }
            }
            BackendCmd::PrevTrack => {
                if self.interpolated_position() > 4.0 {
                    self.seek_absolute(0.0);
                } else if let Some(prev) = self.playback.pop_prev_index() {
                    self.play_track_at(prev, false);
                }
            }
            BackendCmd::SeekAbsolute(secs) => {
                self.seek_absolute(secs);
            }
            BackendCmd::SeekRelative(delta) => {
                let target = self.interpolated_position() + delta;
                self.seek_absolute(target);
            }
            BackendCmd::SetVolume(vol) => {
                let clamped = vol.min(100);
                self.muted = false;
                self.playback.volume = clamped;
                self.config.volume = clamped;
                self.config_dirty = true;
                self.apply_config_volume();
            }
            BackendCmd::ToggleMute => {
                self.muted = !self.muted;
                self.apply_config_volume();
                self.status_line = if self.muted {
                    "Audio muted".to_string()
                } else {
                    format!("Volume: {}%", self.config.volume)
                };
            }
            BackendCmd::ToggleShuffle => {
                self.playback.shuffle = !self.playback.shuffle;
                self.playback.clear_upcoming();
                self.config.shuffle = self.playback.shuffle;
                let _ = self.config.save();
                self.status_line = format!(
                    "Shuffle: {}",
                    if self.playback.shuffle { "On" } else { "Off" }
                );
            }
            BackendCmd::SetShuffle(on) => {
                self.playback.shuffle = on;
                self.playback.clear_upcoming();
                self.config.shuffle = on;
                let _ = self.config.save();
            }
            BackendCmd::CycleRepeat => {
                let next = self.playback.repeat.next();
                self.playback.repeat = next;
                self.playback.clear_upcoming();
                self.config.repeat_mode = next;
                let _ = self.config.save();
                self.status_line = format!("Repeat: {}", next.label());
            }
            BackendCmd::SetRepeat(mode) => {
                self.playback.repeat = mode;
                self.playback.clear_upcoming();
                self.config.repeat_mode = mode;
                let _ = self.config.save();
            }
            BackendCmd::CycleEnhancer => {
                let next = self.playback.enhancer.next();
                self.playback.enhancer = next;
                self.config.enhancer_mode = next;
                let _ = self.config.save();
                if let Some(mpv) = &self.mpv {
                    mpv.set_enhancer(next);
                }
                self.status_line = format!("Audio Enhancer: {}", next.label());
            }
            BackendCmd::ToggleHiFi => {
                self.config.hifi_substitution = !self.config.hifi_substitution;
                let _ = self.config.save();
                if self.config.hifi_substitution {
                    self.start_hifi_upgrade();
                }
                self.status_line = format!(
                    "YouTube Music Quality Upgrade: {}",
                    if self.config.hifi_substitution {
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
            BackendCmd::ToggleTransparentBg => {
                self.config.transparent_bg = !self.config.transparent_bg;
                self.base_theme.transparent_bg = None;
                let _ = self.config.save();
                self.sync_hyprland_opacity();
                self.status_line = format!(
                    "Transparent Glass UI: {}",
                    if self.config.transparent_bg {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
            BackendCmd::StepHyprlandOpacity(forward) => {
                const STEPS: &[f32] = &[0.50, 0.60, 0.70, 0.80, 0.90, 1.00];
                let cur = self.config.hyprland_opacity;
                let idx = STEPS
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        ((*a - cur).abs())
                            .partial_cmp(&((*b - cur).abs()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(4);
                let next = if forward {
                    (idx + 1).min(STEPS.len() - 1)
                } else {
                    idx.saturating_sub(1)
                };
                self.config.hyprland_opacity = STEPS[next];
                self.base_theme.hyprland_opacity = None;
                let _ = self.config.save();
                self.sync_hyprland_opacity();
                self.status_line = format!(
                    "Hyprland Window Opacity: {:.0}%",
                    self.config.hyprland_opacity * 100.0
                );
            }
            BackendCmd::SelectTheme(name) => {
                self.apply_theme_by_name(&name);
            }
            BackendCmd::ToggleLike(track) => {
                self.toggle_like_track(track);
            }
            BackendCmd::CreateYtmPlaylist(title) => {
                self.create_ytm_playlist(&title);
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
            BackendCmd::SetWindowAddress(addr) => {
                self.hyprland_window = addr;
                self.sync_hyprland_opacity();
            }
            BackendCmd::SetWindowVisible(vis) => {
                self.window_visible = vis;
            }
            BackendCmd::Shutdown(_) => {}
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if paused == self.playback.paused {
            return;
        }
        if paused {
            self.playback.position_secs = self.interpolated_position();
            self.pos_anchor = None;
        } else if !self.playback.loading_track && self.playback.current_track().is_some() {
            self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
        }
        self.playback.paused = paused;
        if let Some(mpv) = &self.mpv {
            mpv.set_paused(paused);
        }
    }

    fn seek_absolute(&mut self, secs: f64) {
        let clamped = if self.playback.duration_secs > 0.0 {
            secs.clamp(0.0, self.playback.duration_secs)
        } else {
            secs.max(0.0)
        };
        self.playback.position_secs = clamped;
        if !self.playback.paused && !self.playback.loading_track {
            self.pos_anchor = Some((clamped, Instant::now()));
        }
        if let Some(mpv) = &self.mpv {
            mpv.seek_absolute(clamped);
        }
        let _ = self.mpris_tx.send(MprisUpdate::Seeked(clamped));
    }

    fn toggle_like_track(&mut self, track: Track) {
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

    fn create_ytm_playlist(&mut self, title: &str) {
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

    fn add_track_to_playlist(&mut self, track: Track, playlist_id: String, playlist_name: String) {
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

    fn remove_track_from_playlist(&mut self, idx: usize) {
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

    fn handle_mpv_event(&mut self, ev: MpvEvent) {
        match ev {
            MpvEvent::Position(pos) => {
                if self.awaiting_playback_start {
                    return;
                }
                self.playback.position_secs = pos;
                if !self.playback.paused && !self.playback.loading_track {
                    self.pos_anchor = Some((pos, Instant::now()));
                }
            }
            MpvEvent::Duration(dur) => {
                if dur > 0.0 {
                    self.playback.duration_secs = dur;
                }
            }
            MpvEvent::Paused(paused) => {
                if self.awaiting_playback_start {
                    if paused {
                        self.playback.paused = true;
                    }
                    return;
                }
                if paused {
                    self.playback.position_secs = self.interpolated_position();
                    self.pos_anchor = None;
                } else if !self.playback.loading_track {
                    self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                }
                self.playback.paused = paused;
            }
            MpvEvent::PlaybackStarted => {
                let was_starting = self.awaiting_playback_start;
                self.awaiting_playback_start = false;
                self.playback.paused = false;
                self.pos_anchor = Some((self.playback.position_secs, Instant::now()));
                if was_starting {
                    if let Some(track) = self.playback.current_track() {
                        self.status_line = format!("Playing: {} — {}", track.title, track.artist);
                    }
                }
                self.start_hifi_upgrade();
                self.start_cover_enrichment();
            }
            MpvEvent::TrackEnded => {
                eprintln!(
                    "[sonata:retry] mpv=end-file reason=eof play_seq={} track_id={}",
                    self.play_seq,
                    self.playback
                        .current_track()
                        .map(|t| t.id.as_str())
                        .unwrap_or("-")
                );
                self.pos_anchor = None;
                if Self::should_ignore_stale_playback_error(
                    self.playback.loading_track,
                    self.stream_loaded_at,
                    Duration::from_millis(750),
                ) || !Self::is_natural_track_end(
                    self.interpolated_position(),
                    self.playback.duration_secs,
                ) {
                    eprintln!(
                        "[sonata:retry] ignoring stale eof play_seq={} loading={} loaded_at={}",
                        self.play_seq,
                        self.playback.loading_track,
                        self.stream_loaded_at.is_some()
                    );
                    return;
                }
                if let Some(next) = self.playback.peek_next_index() {
                    self.prepare_next_before_advance(next);
                } else {
                    self.playback.paused = true;
                    self.playback.position_secs = 0.0;
                    self.playback.stream_badge = "Finished".to_string();
                    self.status_line = "Queue finished".to_string();
                }
            }
            MpvEvent::PlaybackError { error: err, file } => {
                eprintln!(
                    "[sonata:retry] mpv=end-file reason=error play_seq={} track_id={} error={}",
                    self.play_seq,
                    self.playback
                        .current_track()
                        .map(|t| t.id.as_str())
                        .unwrap_or("-"),
                    err
                );
                if let (Some(file), Some(active)) =
                    (file.as_deref(), self.playback.active_stream.as_ref())
                {
                    if active.url != file {
                        eprintln!("[sonata:retry] ignoring stale mpv error for another file");
                        return;
                    }
                }
                if self.playback.loading_track {
                    eprintln!(
                        "[sonata:retry] ignoring stale playback-error play_seq={} loading={} loaded_at={}",
                        self.play_seq,
                        self.playback.loading_track,
                        self.stream_loaded_at.is_some()
                    );
                    return;
                }
                if let Some(stream) = self.playback.restore_base_stream() {
                    let resume_at = self.interpolated_position();
                    self.playback.position_secs = resume_at;
                    self.playback.paused = true;
                    self.awaiting_playback_start = true;
                    self.pos_anchor = None;
                    self.playback.stream_badge = stream.badge.clone();
                    self.stream_loaded_at = Some(Instant::now());
                    self.status_line =
                        "Quality upgrade failed; continuing original stream".to_string();
                    if let Some(mpv) = &self.mpv {
                        mpv.load_stream_at(&stream, resume_at);
                    }
                    return;
                }
                if let Some(track) = self.playback.current_track().cloned() {
                    if Self::should_attempt_recovery(self.retried_play_seq, self.play_seq) {
                        self.retried_play_seq = self.play_seq;
                        self.upgrade_started_play_seq = None;
                        let play_seq = self.play_seq;
                        self.abort_active_stream_tasks();
                        self.playback.loading_track = true;
                        self.awaiting_playback_start = false;
                        self.pos_anchor = None;
                        self.playback.stream_badge = "Retrying...".to_string();
                        self.status_line =
                            format!("Stream expired; refreshing \"{}\"...", track.title);
                        let resolver = self.resolver.clone();
                        let int_tx = self.int_tx.clone();
                        eprintln!(
                            "[sonata:retry] transition=resolve-and-reload play_seq={play_seq} track_id={} title={:?}",
                            track.id,
                            track.title
                        );
                        let join = tokio::spawn(async move {
                            match resolver.resolve_recovery(&track).await {
                                Ok(stream) => {
                                    let _ = int_tx.send(InternalEvent::StreamReady {
                                        play_seq,
                                        track_id: track.id,
                                        stream,
                                    });
                                }
                                Err(e) => {
                                    let _ = int_tx.send(InternalEvent::StreamError {
                                        play_seq,
                                        track_id: track.id,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        });
                        self.active_stream_task = Some(join.abort_handle());
                        return;
                    }
                }
                self.playback.loading_track = false;
                self.playback.paused = true;
                self.awaiting_playback_start = false;
                self.pos_anchor = None;
                self.playback.stream_badge = "Error".to_string();
                self.status_line = format!("Playback failed: {err}");
                if let Some(mpv) = &self.mpv {
                    mpv.set_paused(true);
                }
            }
        }
    }

    fn handle_internal_event(&mut self, ev: InternalEvent) {
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
                    return;
                }

                self.sign_in_error = None;
                self.cookies = cookies;
                self.ytm = YtMusicClient::new(self.http.clone(), self.cookies.clone());
                self.resolver = StreamResolver::new(
                    self.http.clone(),
                    self.ytm.clone(),
                    self.cookies.clone(),
                    self.cache_db.clone(),
                );

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
                self.status_line = format!(
                    "Synced {} YouTube Music playlists",
                    self.ytm_playlists.len()
                );
            }
            InternalEvent::HomeLoaded {
                req_id,
                shelves,
                error,
            } => {
                if req_id != self.home_req_id {
                    return;
                }
                self.home_loading = false;
                self.home_error = error;
                if !shelves.is_empty() {
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
            InternalEvent::SearchLoaded {
                req_id,
                tracks,
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
                let urls: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.thumbnail_url.clone())
                    .collect();
                self.prefetch_thumbnails(urls);
                let count = tracks.len();
                self.search_results = tracks;
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
                self.prefetch_thumbnails(urls);
                let count = tracks.len();
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
                self.prefetch_thumbnails(urls);
                self.playlist_tracks.extend(tracks);
            }
            InternalEvent::StreamReady {
                play_seq,
                track_id,
                stream,
            } => {
                eprintln!(
                    "[sonata:retry] stream-ready play_seq={play_seq} track_id={track_id} source={} badge={}",
                    if stream.url.starts_with("http") { "remote" } else { "local" },
                    stream.badge
                );
                if play_seq != self.play_seq {
                    return;
                }
                let Some(track) = self
                    .playback
                    .current_track()
                    .filter(|t| t.id == track_id)
                    .cloned()
                else {
                    return;
                };
                self.active_stream_task = None;
                self.playback.set_active_stream(stream.clone());
                self.playback.loading_track = false;
                self.playback.paused = true;
                self.awaiting_playback_start = true;
                let resume_at = if self.retried_play_seq == play_seq {
                    self.playback.position_secs
                } else {
                    0.0
                };
                self.playback.position_secs = resume_at;
                self.playback.stream_badge = stream.badge.clone();
                self.pos_anchor = None;
                self.stream_loaded_at = Some(Instant::now());
                self.status_line = format!("Starting: {} — {}", track.title, track.artist);
                if let Some(mpv) = &self.mpv {
                    if resume_at >= 0.35 {
                        mpv.load_stream_at(&stream, resume_at);
                    } else {
                        mpv.load_stream(&stream);
                    }
                    mpv.set_paused(false);
                }
                self.prefetch_next();
            }
            InternalEvent::NextTrackPrepared {
                play_seq,
                index,
                track_id,
                result,
            } => {
                if play_seq != self.play_seq
                    || self
                        .pending_auto_advance
                        .as_ref()
                        .is_none_or(|(seq, id)| *seq != play_seq || id != &track_id)
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
                    Ok(stream) => self.play_track_with_stream(index, true, Some(stream)),
                    Err(error) => {
                        self.playback.loading_track = false;
                        self.playback.paused = true;
                        self.status_line = format!("Could not prepare next track: {error}");
                    }
                }
            }
            InternalEvent::StreamUpgrade {
                play_seq,
                track_id,
                stream,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if !self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    return;
                }
                if !self.playback.set_upgraded_stream(stream.clone()) {
                    return;
                }
                self.active_upgrade_task = None;
                let resume_at = self.interpolated_position();
                let was_paused = self.playback.paused;
                self.playback.position_secs = resume_at;
                self.stream_loaded_at = Some(Instant::now());
                if !was_paused {
                    self.pos_anchor = Some((resume_at, Instant::now()));
                }
                self.playback.stream_badge = stream.badge.clone();
                if let Some(mpv) = &self.mpv {
                    mpv.load_stream_at(&stream, resume_at);
                    mpv.set_paused(was_paused);
                }
                self.status_line = format!("Upgraded audio quality: {}", stream.badge);
            }
            InternalEvent::StreamError {
                play_seq,
                track_id,
                error,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                if !self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    return;
                }
                self.playback.loading_track = false;
                self.playback.paused = true;
                self.pos_anchor = None;
                self.playback.stream_badge = "Error".to_string();
                self.status_line = format!("Stream error: {error}");
                if let Some(mpv) = &self.mpv {
                    mpv.set_paused(true);
                }
            }
            InternalEvent::LyricsReady {
                play_seq,
                track_id,
                lyrics,
            } => {
                if play_seq == self.play_seq
                    && self
                        .playback
                        .current_track()
                        .is_some_and(|t| t.id == track_id)
                {
                    self.lyrics = lyrics;
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
                        track.id == track_id && track.thumbnail_url.as_deref() == Some(url.as_str())
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
                    track.thumbnail_url = Some(url);
                }
                if self
                    .playback
                    .current_track()
                    .is_some_and(|track| track.id == track_id)
                {
                    self.art_path = art_path;
                    self.ambient_rgb = ambient_rgb;
                }
            }
            InternalEvent::ThumbnailsReady => {}
            InternalEvent::PlaylistCreated(pl) => {
                self.ytm_playlists.push(pl.clone());
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &self.ytm_playlists);
                }
                self.status_line = format!("Created playlist \"{}\"", pl.name);
            }
            InternalEvent::StatusMessage(msg) => {
                self.status_line = msg;
            }
        }
    }

    fn export_now_playing(&mut self) {
        let track = self.playback.current_track();
        let status = match track {
            None => "Stopped",
            Some(_) if self.playback.loading_track => "Loading",
            Some(_) if self.playback.paused => "Paused",
            Some(_) => "Playing",
        };
        let track_id = track.map(|t| t.id.clone()).unwrap_or_default();
        let pos_secs = self.playback.position_secs.max(0.0).floor() as u64;
        let dur_secs = self.playback.duration_secs.max(0.0).floor() as u64;

        let active_lyric = match &self.lyrics {
            LyricsState::Synced(lines) => active_line_index(lines, self.playback.position_secs)
                .and_then(|idx| lines.get(idx))
                .map(|l| l.text.clone())
                .filter(|s| !s.is_empty()),
            _ => None,
        };

        let art_url = track.and_then(|t| t.thumbnail_url.clone());
        let art_path_str = self
            .art_path
            .as_ref()
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned());

        let fingerprint = ExportFingerprint {
            track_id,
            status: status.to_string(),
            lyric: active_lyric.clone(),
            art_path: art_path_str.clone(),
            pos_secs,
            dur_secs,
        };
        if self.last_exported_state.as_ref() == Some(&fingerprint) {
            return;
        }
        self.last_exported_state = Some(fingerprint);

        let payload = media::build_now_playing_json(
            status,
            track,
            &self.playback.stream_badge,
            pos_secs,
            dur_secs,
            active_lyric.as_deref(),
            art_path_str.as_deref(),
            art_url.as_deref(),
        );
        media::write_now_playing_json(&payload);
    }

    async fn publish(&mut self, notify_integrations: bool) {
        self.export_now_playing();
        if self.config_dirty {
            self.config_dirty = false;
            let _ = self.config.save();
        }

        let snap = Arc::new(DesktopSnapshot {
            config: self.config.clone(),
            themes: self.themes.clone(),
            base_theme: self.base_theme.clone(),
            active_nav: self.active_nav.clone(),
            status_line: self.status_line.clone(),
            available_browsers: self.available_browsers.clone(),
            signed_in_browser: self.signed_in_browser.clone(),
            user_profile: self.user_profile.clone(),
            user_avatar_path: self.user_avatar_path.clone(),
            signing_in: self.signing_in,
            sign_in_error: self.sign_in_error.clone(),
            ytm_playlists: self.ytm_playlists.clone(),
            home_shelves: self.home_shelves.clone(),
            home_loading: self.home_loading,
            home_error: self.home_error.clone(),
            search_query: self.search_query.clone(),
            search_results: self.search_results.clone(),
            search_loading: self.search_loading,
            playlist_id: self.playlist_id.clone(),
            playlist_heading: self.playlist_heading.clone(),
            playlist_subtitle: self.playlist_subtitle.clone(),
            playlist_cover_url: self.playlist_cover_url.clone(),
            playlist_editable: self.playlist_editable,
            playlist_tracks: self.playlist_tracks.clone(),
            playlist_loading: self.playlist_loading,
            queue: self.playback.queue.clone(),
            current_index: self.playback.current_index,
            paused: self.playback.paused,
            muted: self.muted,
            position_secs: self.playback.position_secs,
            duration_secs: self.playback.duration_secs,
            volume: self.playback.volume,
            shuffle: self.playback.shuffle,
            repeat: self.playback.repeat,
            enhancer: self.playback.enhancer,
            stream_badge: self.playback.stream_badge.clone(),
            lyrics: self.lyrics.clone(),
            art_path: self.art_path.clone(),
            ambient_rgb: self.ambient_rgb,
            spotify_notice: self.spotify_notice.clone(),
        });

        let _ = self.snap_tx.send(snap.clone());
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
        let _ = self.config.save();
        if let Some(mpv) = self.mpv.take() {
            mpv.shutdown().await;
        }
        if let Some(tray) = self.tray_handle.take() {
            tray.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_track_end_requires_near_duration() {
        assert!(BackendEngine::is_natural_track_end(178.0, 180.0));
        assert!(!BackendEngine::is_natural_track_end(12.0, 180.0));
        assert!(!BackendEngine::is_natural_track_end(0.0, 0.0));
    }

    #[test]
    fn loading_or_recent_streams_ignore_stale_errors() {
        assert!(BackendEngine::should_ignore_stale_playback_error(
            true,
            None,
            Duration::from_millis(750),
        ));
        assert!(BackendEngine::should_ignore_stale_playback_error(
            false,
            None,
            Duration::from_millis(750),
        ));
        assert!(BackendEngine::should_ignore_stale_playback_error(
            false,
            Some(Instant::now()),
            Duration::from_millis(750),
        ));
    }

    #[test]
    fn one_recovery_attempt_per_play_sequence() {
        assert!(BackendEngine::should_attempt_recovery(0, 7));
        assert!(!BackendEngine::should_attempt_recovery(7, 7));
    }
}
