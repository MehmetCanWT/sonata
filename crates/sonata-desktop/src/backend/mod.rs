//! Headless playback and state engine behind the GPUI client, running on its own Tokio runtime thread.
//!
//! 1. **Types & Handle (`types`)**: defines `BackendCmd`, `UiWindowCmd`, `DesktopSnapshot`, `BackendHandle`, and `InternalEvent`.
//! 2. **Engine Loop (`mod.rs`)**: `run_backend_loop` multiplexes commands, internal task events, and 40ms playback ticks, and publishes snapshots.
//! 3. **Playback (`playback`)**: drives transitions, stream resolution, Hi-Fi upgrades, cover enrichment, precache, and `mpv` events.
//! 4. **Library (`library`)**: handles authentication, home/artist/collection feeds, search, playlist edits, themes, and thumbnail prefetching.

mod library;
mod playback;
mod types;
pub mod ytm_extra;

pub use types::{BackendCmd, BackendHandle, DesktopSnapshot, NavTarget, UiWindowCmd};
pub use ytm_extra::{AlbumDetail, MoodCategoryTile, MoodChip, RichSearchResult};

use crate::integrations::mpris::{spawn_mpris, MprisUpdate};
use crate::integrations::tray::{spawn_tray, TrayHandle, TrayState};
use reqwest::Client;
use sonata_core::auth::{list_supported_browsers, BrowserOption, ExtractedCookies};
use sonata_core::cache::{
    enforce_active_media_cache_limit_async, start_media_cache_maintenance, CacheDb,
};
use sonata_core::config::AppConfig;
use sonata_core::hyprland;
use sonata_core::lyrics::LyricsState;
use sonata_core::media;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator};
use sonata_core::player::{
    MpvController, NextTrackPrecache, PlaybackState, PlaybackTask, PlaybackTaskSet, PrecacheKey,
};
use sonata_core::providers::hifi::{ResolvedStream, StreamResolver};
use sonata_core::providers::ytmusic::{ArtistPage, HomeShelf, YtAccountProfile, YtMusicClient};
use sonata_core::providers::{build_http_client, Playlist, Track};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Semaphore};
use types::{ExportFingerprint, InternalEvent};

pub fn spawn_backend(win_tx: mpsc::UnboundedSender<UiWindowCmd>) -> anyhow::Result<BackendHandle> {
    let config = AppConfig::load();
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

    let mut playback = PlaybackState::new(config.volume);
    playback.shuffle = config.shuffle;
    playback.repeat = config.repeat_mode;

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

    Ok(BackendHandle { cmd_tx, snap_rx })
}

struct BackendEngine {
    pub(super) config: AppConfig,
    pub(super) http: Client,
    pub(super) cookies: ExtractedCookies,
    pub(super) ytm: YtMusicClient,
    pub(super) resolver: StreamResolver,
    pub(super) cache_db: Option<CacheDb>,
    pub(super) mpv: Option<Arc<MpvController>>,
    pub(super) int_tx: mpsc::UnboundedSender<InternalEvent>,
    pub(super) snap_tx: watch::Sender<Arc<DesktopSnapshot>>,
    pub(super) mpris_tx: mpsc::UnboundedSender<MprisUpdate>,
    pub(super) tray_handle: Option<TrayHandle>,
    pub(super) last_tray_state: TrayState,
    pub(super) window_visible: bool,
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
    pub(super) motion_cover: Option<Arc<sonata_core::motion::MotionCover>>,
    pub(super) ambient_rgb: (u8, u8, u8),
    pub(super) spotify_notice: Option<String>,
    pub(super) update_info: Option<sonata_core::updater::UpdateCheckResult>,
    pub(super) checking_updates: bool,
    pub(super) pos_anchor: Option<(f64, Instant)>,
    pub(super) stream_loaded_at: Option<Instant>,
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
    pub(super) retried_play_seq: u64,
    pub(super) playback_tasks: PlaybackTaskSet,
    pub(super) next_precache: NextTrackPrecache,
    pub(super) next_prefetch_task: Option<(PrecacheKey, PlaybackTask<Option<ResolvedStream>>)>,
    pub(super) adopted_prefetch_task: Option<(String, PlaybackTask<Option<ResolvedStream>>)>,
    pub(super) pending_auto_advance: Option<(u64, String)>,
    pub(super) awaiting_playback_start: bool,
    pub(super) upgrade_started_play_seq: Option<u64>,
    pub(super) upgrading_quality: bool,
    pub(super) cover_started_play_seq: Option<u64>,
    pub(super) config_dirty: bool,
    pub(super) last_exported_state: Option<ExportFingerprint>,
    pub(super) play_requested_at: Option<Instant>,
    pub(super) in_flight_thumbs: Arc<Mutex<HashSet<String>>>,
    pub(super) thumbnail_slots: Arc<Semaphore>,
}

#[allow(clippy::too_many_arguments)]
async fn run_backend_loop(
    config: AppConfig,
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

    let (mpv_tx, mut mpv_rx) = mpsc::channel(64);
    let (mpv, initial_status) = match MpvController::spawn_desktop(config.volume, mpv_tx).await {
        Ok(ctrl) => (Some(Arc::new(ctrl)), "Loading YouTube Music...".to_string()),
        Err(e) => (None, format!("Audio warning: {e}")),
    };

    let (int_tx, mut int_rx) = mpsc::unbounded_channel();
    let tray_handle = spawn_tray(cmd_tx, win_tx).await;
    let initial_liked_ids: HashSet<String> = cache_db
        .as_ref()
        .and_then(|db| db.get_playlist("YTM:VLLM"))
        .map(|(_, tracks, _)| tracks.into_iter().map(|t| t.id).collect())
        .unwrap_or_default();

    let mut engine = BackendEngine {
        config,
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
        pos_anchor: None,
        stream_loaded_at: None,
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
        retried_play_seq: 0,
        playback_tasks: PlaybackTaskSet::default(),
        next_precache: NextTrackPrecache::default(),
        next_prefetch_task: None,
        adopted_prefetch_task: None,
        pending_auto_advance: None,
        awaiting_playback_start: false,
        upgrade_started_play_seq: None,
        upgrading_quality: false,
        cover_started_play_seq: None,
        config_dirty: false,
        last_exported_state: None,
        play_requested_at: None,
        in_flight_thumbs: Arc::new(Mutex::new(HashSet::new())),
        thumbnail_slots: Arc::new(Semaphore::new(6)),
    };

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
            _ = smooth_tick.tick(), if active_play => {
                if engine.tick_playback() {
                    engine.publish(false).await;
                }
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
                    NavTarget::Library
                        if self.library_albums.is_empty() && !self.library_extras_loading =>
                    {
                        self.load_library_extras();
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
                self.sign_out();
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
                PlaybackCoordinator::apply(
                    &mut self.playback,
                    PlaybackCommand::QueueChanged(tracks),
                );
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
            BackendCmd::PlayTrackNext(track) => {
                let title = track.title.clone();
                if self.playback.queue.is_empty() {
                    self.playback.queue.push(track);
                    self.play_track_at(0, false);
                } else {
                    let insert_at = self
                        .playback
                        .current_index
                        .map(|i| (i + 1).min(self.playback.queue.len()))
                        .unwrap_or(0);
                    self.playback.queue.insert(insert_at, track);
                }
                self.status_line = format!("Playing next: {title}");
            }
            BackendCmd::AddTrackToQueue(track) => {
                let title = track.title.clone();
                self.playback.queue.push(track);
                self.status_line = format!("Added to queue: {title}");
            }
            BackendCmd::RemoveFromQueue(idx) => {
                if idx < self.playback.queue.len() && Some(idx) != self.playback.current_index {
                    self.playback.queue.remove(idx);
                    if let Some(cur) = self.playback.current_index {
                        if idx < cur {
                            self.playback.current_index = Some(cur - 1);
                        }
                    }
                    self.playback.clear_upcoming();
                }
            }
            BackendCmd::MoveQueueItem { from, to } => {
                let len = self.playback.queue.len();
                if from < len && to < len && from != to {
                    let cur_id = self.playback.current_track().map(|t| t.id.clone());
                    let item = self.playback.queue.remove(from);
                    self.playback.queue.insert(to, item);
                    if let Some(cid) = cur_id {
                        self.playback.current_index =
                            self.playback.queue.iter().position(|t| t.id == cid);
                    }
                    self.playback.clear_upcoming();
                }
            }
            BackendCmd::ClearQueue => {
                if let Some(cur_idx) = self.playback.current_index {
                    if let Some(cur) = self.playback.queue.get(cur_idx).cloned() {
                        self.playback.queue = vec![cur];
                        self.playback.current_index = Some(0);
                    } else {
                        self.playback.queue.clear();
                        self.playback.current_index = None;
                    }
                } else {
                    self.playback.queue.clear();
                }
                self.playback.clear_upcoming();
                self.status_line = "Cleared upcoming queue".to_string();
            }
            BackendCmd::ShuffleQueue => {
                let len = self.playback.queue.len();
                if len > 2 {
                    let cur = self.playback.current_index;
                    let cur_track = cur.map(|i| self.playback.queue.remove(i));
                    let mut seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(17);
                    for i in (1..self.playback.queue.len()).rev() {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let j = seed % (i + 1);
                        self.playback.queue.swap(i, j);
                    }
                    if let Some(t) = cur_track {
                        self.playback.queue.insert(0, t);
                        self.playback.current_index = Some(0);
                    }
                    self.playback.clear_upcoming();
                    self.status_line = "Shuffled queue".to_string();
                }
            }
            BackendCmd::TogglePause => {
                self.set_paused(!self.playback.paused);
            }
            BackendCmd::SetPaused(paused) => {
                self.set_paused(paused);
            }
            BackendCmd::Stop => {
                self.playback_tasks.restart();
                if let Some((_, task)) = self.next_prefetch_task.take() {
                    task.abort();
                }
                if let Some((_, task)) = self.adopted_prefetch_task.take() {
                    task.abort();
                }
                self.next_precache.take();
                self.playback.paused = true;
                self.playback.loading_track = false;
                self.upgrading_quality = false;
                self.playback.position_secs = 0.0;
                self.pos_anchor = None;
                self.playback.stream_badge = "Stopped".to_string();
                if let Some(mpv) = &self.mpv {
                    mpv.stop();
                }
            }
            BackendCmd::NextTrack => {
                if let Some(transition) =
                    PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Next)
                {
                    self.start_play_transition(transition, None);
                }
            }
            BackendCmd::PrevTrack => {
                if self.interpolated_position() > 4.0 {
                    self.seek_absolute(0.0);
                } else if let Some(transition) =
                    PlaybackCoordinator::apply(&mut self.playback, PlaybackCommand::Previous)
                {
                    self.start_play_transition(transition, None);
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
            BackendCmd::ToggleJioSaavn => {
                self.config.jiosaavn_enabled = !self.config.jiosaavn_enabled;
                AppConfig::set_jiosaavn_enabled(self.config.jiosaavn_enabled);
                let _ = self.config.save();
                if self.config.jiosaavn_enabled {
                    self.start_hifi_upgrade();
                }
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
            BackendCmd::ToggleMotionCovers => {
                self.config.motion_covers = !self.config.motion_covers;
                AppConfig::set_motion_covers_enabled(self.config.motion_covers);
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
            BackendCmd::ToggleLike(track) => {
                self.toggle_like_track(track);
            }
            BackendCmd::DislikeTrack(track) => {
                self.dislike_track(track);
            }
            BackendCmd::ToggleSaveAlbum => {
                self.toggle_save_album();
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
            BackendCmd::CheckForUpdates => {
                self.spawn_update_check();
            }
            BackendCmd::SetWindowAddress(addr) => {
                self.hyprland_window = addr;
                self.sync_hyprland_opacity(None);
            }
            BackendCmd::SetWindowVisible(vis) => {
                self.window_visible = vis;
            }
            BackendCmd::Shutdown(_) => {}
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
                PlaybackCoordinator::apply(
                    &mut self.playback,
                    PlaybackCommand::QueueChanged(tracks),
                );
                self.play_track_at(0, false);
                self.status_line = format!("Playing {label}");
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
                    self.prefetch_thumbnails(urls);
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
                    self.prefetch_thumbnails(urls);
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
                self.prefetch_thumbnails(urls);
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
                self.prefetch_thumbnails(urls);
                let count = tracks.len();
                if self.playlist_id.as_deref() == Some("VLLM") {
                    self.liked_track_ids = tracks.iter().map(|t| t.id.clone()).collect();
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
                self.prefetch_thumbnails(urls);
                self.playlist_tracks.extend(tracks);
            }
            InternalEvent::StreamReady {
                play_seq,
                track_id,
                stream,
            } => {
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
                tracing::info!(
                    event = "stream_ready",
                    client = "desktop",
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
                self.playback.set_active_stream(stream.clone());
                self.playback.loading_track = false;
                self.upgrading_quality = false;
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
                    if resume_at >= 0.05 {
                        mpv.load_stream_at(&stream, resume_at);
                    } else {
                        mpv.load_stream(&stream);
                    }
                    mpv.set_paused(false);
                }
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
                self.upgrading_quality = false;
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
                let resume_at = self.interpolated_position();
                let was_paused = self.playback.paused;
                self.playback.position_secs = resume_at;
                self.stream_loaded_at = Some(Instant::now());
                if !was_paused {
                    self.pos_anchor = Some((resume_at, Instant::now()));
                }
                self.playback.stream_badge = stream.badge.clone();
                tracing::info!(
                    event = "quality_upgraded",
                    client = "desktop",
                    track_id = %track_id,
                    play_seq,
                    quality = %stream.badge,
                    resume_secs = resume_at,
                    "audio quality upgraded"
                );
                if let Some(mpv) = &self.mpv {
                    mpv.load_stream_at(&stream, resume_at);
                    mpv.set_paused(was_paused);
                }
                self.status_line = format!("Upgraded audio quality: {}", stream.badge);
            }
            InternalEvent::UpgradeFinished { play_seq } => {
                if play_seq == self.play_seq {
                    self.upgrading_quality = false;
                }
            }
            InternalEvent::StreamError {
                play_seq,
                track_id,
                error,
            } => {
                if play_seq != self.play_seq {
                    return;
                }
                let Some(track) = self
                    .playback
                    .current_track()
                    .filter(|track| track.id == track_id)
                else {
                    return;
                };
                tracing::error!(
                    event = "resolve_failed",
                    client = "desktop",
                    track_id = %track.id,
                    title = %track.title,
                    artist = %track.artist,
                    provider = ?track.source,
                    play_seq,
                    error = %error,
                    "stream resolution failed"
                );
                self.playback.loading_track = false;
                self.upgrading_quality = false;
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
            InternalEvent::ThumbnailsReady => {}
            InternalEvent::PlaylistCreated(pl) => {
                self.ytm_playlists.push(pl.clone());
                if let Some(db) = &self.cache_db {
                    db.put_library("ytm", &self.ytm_playlists);
                }
                self.status_line = format!("Created playlist \"{}\"", pl.name);
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
        if let Err(err) = self.config.save() {
            tracing::warn!(?err, "failed to save desktop config on shutdown");
        }
        if let Some((_, task)) = self.next_prefetch_task.take() {
            task.abort();
        }
        if let Some((_, task)) = self.adopted_prefetch_task.take() {
            task.abort();
        }
        self.next_precache.take();
        if !self.playback_tasks.shutdown(Duration::from_secs(2)).await {
            tracing::warn!("playback tasks exceeded shutdown timeout and were aborted");
        }
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
        }));
        let handle = BackendHandle { cmd_tx, snap_rx };
        let responder = std::thread::spawn(move || {
            if let Some(BackendCmd::Shutdown(done)) = cmd_rx.blocking_recv() {
                let _ = done.try_send(());
            }
        });
        handle.shutdown_sync();
        responder.join().expect("responder joined");
    }
}
