use crate::auth::{extract_browser_cookies, ExtractedCookies};
use crate::cache::CacheDb;
use crate::config::{AppConfig, ProviderMode};
use crate::hyprland;
use crate::lyrics::{lrclib, LyricsState};
use crate::player::{MpvController, MpvEvent, PlaybackState};
use crate::providers::hifi::{ResolvedStream, StreamResolver};
use crate::providers::spotify::{extract_playlist_id, SpotifyClient};
use crate::providers::ytmusic::YtMusicClient;
use crate::providers::{build_http_client, Playlist, ProviderSource, Track};
use crate::theme::Theme;
use base64::Engine as _;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{ListState, TableState};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, Default)]
pub struct UiRects {
    pub search_bar: Rect,
    pub tracks_rows: Rect,
    pub sidebar_inner: Rect,
    pub album_art_box: Rect,
    pub player_bar: Rect,
    pub player_top: Rect,
    pub progress_bar: Rect,
    pub volume_bar: Rect,
    pub ctrl_prev: Rect,
    pub ctrl_play: Rect,
    pub ctrl_next: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Tracks,
    SearchInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEntry {
    Search,
    Queue,
    Playlist(Playlist),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    ImageWarning(usize),
    ThemePicker(usize, String),
    SpotifyImport(String),
    AddToPlaylist(usize),
    CreatePlaylist(String),
    Settings(usize),
    Help,
}

pub enum AppEvent {
    AuthReady {
        cookies: ExtractedCookies,
        ytm_playlists: Vec<Playlist>,
        spotify_playlists: Vec<Playlist>,
        imported_playlists: Vec<Playlist>,
    },
    TracksLoaded {
        playlist_id: Option<String>,
        heading: String,
        tracks: Vec<Track>,
    },
    TracksAppended {
        heading: String,
        tracks: Vec<Track>,
    },
    StreamReady {
        track_id: String,
        stream: ResolvedStream,
    },
    StreamUpgrade {
        track_id: String,
        stream: ResolvedStream,
    },
    StreamError {
        track_id: String,
        error: String,
    },
    LyricsReady {
        track_id: String,
        lyrics: LyricsState,
    },
    ArtworkReady {
        track_id: String,
        image: DynamicImage,
        png_bytes: Vec<u8>,
    },
    Mpv(MpvEvent),
    PlaylistCreated(Playlist),
    StatusMessage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFingerprint {
    track_id: String,
    status: String,
    lyric: Option<String>,
    art_path: Option<String>,
    pos_secs: u64,
    dur_secs: u64,
}

pub struct App {
    pub config: AppConfig,
    pub themes: Vec<Theme>,
    pub base_theme: Theme,
    pub theme: Theme,
    pub hyprland_window: Option<String>,
    pub picker: Picker,
    pub album_art: Option<StatefulProtocol>,
    pub album_art_track_id: Option<String>,
    pub kitty_art_png: Option<Vec<u8>>,
    pub last_art_rect: Option<Rect>,
    pub kitty_drawn_track: Option<(String, Rect)>,
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
    pub last_track_click: Option<(Instant, usize)>,
    pub last_art_click: Option<Instant>,
    pub mini_mode: bool,
    pub search_query: String,
    pub status_line: String,
    pub browser_label: String,
    pub should_quit: bool,

    pub http: Client,
    pub cookies: ExtractedCookies,
    pub ytm: YtMusicClient,
    pub spotify: SpotifyClient,
    pub resolver: StreamResolver,
    pub cache_db: Option<CacheDb>,
    pub mpv: Option<Arc<MpvController>>,
    pub tx: mpsc::UnboundedSender<AppEvent>,
    pub last_exported_state: Option<ExportFingerprint>,
}

impl App {
    pub fn new(config: AppConfig, picker: Picker, tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        let themes = Theme::load_all(&AppConfig::themes_dir());
        let base_theme = themes
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&config.theme))
            .cloned()
            .unwrap_or_default();
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
        let playback = PlaybackState::new(config.volume, config.enhancer_mode);

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
            theme: base_theme.clone(),
            base_theme,
            hyprland_window,
            picker,
            album_art: None,
            album_art_track_id: None,
            kitty_art_png: None,
            last_art_rect: None,
            kitty_drawn_track: None,
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
            last_track_click: None,
            last_art_click: None,
            mini_mode: false,
            search_query: String::new(),
            status_line: "Scanning browser cookies...".to_string(),
            browser_label: "Scanning...".to_string(),
            should_quit: false,
            http,
            cookies,
            ytm,
            spotify,
            resolver,
            cache_db,
            mpv: None,
            tx,
            last_exported_state: None,
        };
        app.rebuild_sidebar();
        app.sync_appearance();
        app
    }

    pub fn start_background_init(&self) {
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
                playlist_id,
                heading,
                tracks,
            } => {
                self.current_playlist_id = playlist_id;
                self.tracks_heading = heading;
                self.visible_tracks = tracks;
                self.track_table_state
                    .select(if self.visible_tracks.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                self.status_line = format!("Loaded {} tracks", self.visible_tracks.len());
            }
            AppEvent::TracksAppended { heading, tracks } => {
                if self.tracks_heading == heading {
                    self.visible_tracks.extend(tracks);
                    self.status_line = format!("Loaded {} tracks", self.visible_tracks.len());
                }
            }
            AppEvent::StreamReady { track_id, stream } => {
                let current_track = self.playback.current_track().cloned();
                if let Some(track) = current_track.filter(|t| t.id == track_id) {
                    self.playback.loading_track = false;
                    self.playback.paused = false;
                    self.playback.stream_badge = stream.badge.clone();
                    let is_remote = stream.url.starts_with("http");
                    if let Some(mpv) = &self.mpv {
                        let mpv = Arc::clone(mpv);
                        let url = stream.url;
                        let tx = self.tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = mpv.load_url(&url).await {
                                let _ = tx.send(AppEvent::StatusMessage(format!("mpv error: {e}")));
                            }
                        });
                    }
                    if self.config.hifi_substitution && is_remote {
                        let resolver = self.resolver.clone();
                        let tx = self.tx.clone();
                        let badge = stream.badge;
                        tokio::spawn(async move {
                            if let Some(upgraded) =
                                resolver.resolve_hifi_upgrade(&track, &badge).await
                            {
                                let _ = tx.send(AppEvent::StreamUpgrade {
                                    track_id: track.id,
                                    stream: upgraded,
                                });
                            }
                        });
                    }
                    self.prefetch_next();
                }
            }
            AppEvent::StreamUpgrade { track_id, stream } => {
                let is_current = self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id);
                if is_current {
                    let pos = self.playback.position_secs.max(0.0);
                    self.playback.stream_badge = stream.badge.clone();
                    self.status_line = format!("Upgraded stream to {}", stream.badge);
                    if let Some(mpv) = &self.mpv {
                        let mpv = Arc::clone(mpv);
                        let url = stream.url;
                        tokio::spawn(async move {
                            let _ = mpv.load_url_at(&url, pos).await;
                        });
                    }
                }
            }
            AppEvent::StreamError { track_id, error } => {
                if self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    self.playback.loading_track = false;
                    self.playback.stream_badge = "Stream Error".to_string();
                    self.status_line = format!("Failed to resolve stream: {error}");
                }
            }
            AppEvent::LyricsReady { track_id, lyrics } => {
                if self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    self.lyrics = lyrics;
                }
            }
            AppEvent::ArtworkReady {
                track_id,
                image,
                png_bytes,
            } => {
                if self
                    .playback
                    .current_track()
                    .is_some_and(|t| t.id == track_id)
                {
                    self.album_art = Some(self.picker.new_resize_protocol(image));
                    self.kitty_art_png = Some(png_bytes);
                    self.kitty_drawn_track = None;
                    self.album_art_track_id = Some(track_id);
                }
            }
            AppEvent::Mpv(mev) => match mev {
                MpvEvent::Position(pos) => {
                    if !self.playback.loading_track {
                        self.playback.position_secs = pos;
                    }
                }
                MpvEvent::Duration(dur) => {
                    if !self.playback.loading_track {
                        self.playback.duration_secs = dur;
                    }
                }
                MpvEvent::Paused(p) => {
                    if !self.playback.loading_track {
                        self.playback.paused = p;
                    }
                }
                MpvEvent::TrackEnded => {
                    if !self.playback.loading_track {
                        if let Some(next_idx) = self.playback.next_index() {
                            self.play_queue_index(next_idx);
                        } else {
                            self.playback.paused = true;
                        }
                    }
                }
            },
            AppEvent::PlaylistCreated(pl) => {
                self.status_line = format!("Created YouTube Music playlist: {}", pl.name);
                self.ytm_playlists.push(pl);
                self.rebuild_sidebar();
            }
            AppEvent::StatusMessage(msg) => {
                self.status_line = msg;
            }
        }
    }

    pub fn rebuild_sidebar(&mut self) {
        let mut items = vec![SidebarEntry::Search, SidebarEntry::Queue];
        if self.config.provider_mode.uses_ytm() {
            for p in &self.ytm_playlists {
                items.push(SidebarEntry::Playlist(p.clone()));
            }
        }
        if self.config.provider_mode.uses_spotify() {
            for p in &self.spotify_playlists {
                items.push(SidebarEntry::Playlist(p.clone()));
            }
            for p in &self.imported_playlists {
                if !items.iter().any(|existing| match existing {
                    SidebarEntry::Playlist(ep) => ep.id == p.id,
                    _ => false,
                }) {
                    items.push(SidebarEntry::Playlist(p.clone()));
                }
            }
        }
        self.sidebar_items = items;
        if self.sidebar_idx >= self.sidebar_items.len() {
            self.sidebar_idx = 0;
        }
    }

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

    pub fn toggle_hifi_substitution(&mut self) {
        self.config.hifi_substitution = !self.config.hifi_substitution;
        let _ = self.config.save();
        self.status_line = format!(
            "Hi-Fi 320kbps Substitution: {}",
            if self.config.hifi_substitution {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    pub fn trigger_search(&mut self) {
        let q = self.search_query.trim().to_string();
        if q.is_empty() {
            return;
        }
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
                playlist_id: None,
                heading: format!("Search: {q}"),
                tracks: combined,
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
                self.current_playlist_id = None;
                self.tracks_heading = "Playback Queue".to_string();
                self.visible_tracks = self.playback.queue.clone();
                self.track_table_state
                    .select(self.playback.current_index.or(Some(0)));
                self.focus = FocusPane::Tracks;
            }
            SidebarEntry::Playlist(pl) => {
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
                    if let Some((cached_name, cached_tracks, is_fresh)) =
                        cache_db.as_ref().and_then(|db| db.get_playlist(&db_key))
                    {
                        let _ = tx.send(AppEvent::TracksLoaded {
                            playlist_id: ytm_pid.clone(),
                            heading: format!("[{}] {}", pl.source.short_tag(), cached_name),
                            tracks: cached_tracks,
                        });
                        if is_fresh {
                            return;
                        }
                    }

                    let heading = format!("[{}] {}", pl.source.short_tag(), pl.name);
                    let mut cached_name = pl.name.clone();
                    let mut all_tracks: Vec<Track> = Vec::new();
                    let res = match pl.source {
                        ProviderSource::YouTubeMusic => {
                            ytm.fetch_playlist_tracks_progressive(&pl.id, |batch, is_first| {
                                all_tracks.extend(batch.iter().cloned());
                                if is_first {
                                    let _ = tx.send(AppEvent::TracksLoaded {
                                        playlist_id: ytm_pid.clone(),
                                        heading: heading.clone(),
                                        tracks: batch,
                                    });
                                } else {
                                    let _ = tx.send(AppEvent::TracksAppended {
                                        heading: heading.clone(),
                                        tracks: batch,
                                    });
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
                                        all_tracks.extend(batch.iter().cloned());
                                        let resolved_heading = format!("[SPF] {name}");
                                        if is_first {
                                            let _ = tx.send(AppEvent::TracksLoaded {
                                                playlist_id: None,
                                                heading: resolved_heading,
                                                tracks: batch,
                                            });
                                        } else {
                                            let _ = tx.send(AppEvent::TracksAppended {
                                                heading: resolved_heading,
                                                tracks: batch,
                                            });
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

    pub fn play_selected_track(&mut self) {
        let Some(selected) = self.track_table_state.selected() else {
            return;
        };
        if self.visible_tracks.is_empty() || selected >= self.visible_tracks.len() {
            return;
        }
        self.playback.queue = self.visible_tracks.clone();
        self.play_queue_index(selected);
    }

    pub fn toggle_mini_mode(&mut self) {
        self.mini_mode = !self.mini_mode;
        self.clear_kitty_images();
        self.status_line = if self.mini_mode {
            "Mini Mode (double-click Album Art or press m to restore)".to_string()
        } else {
            "Full Mode".to_string()
        };
    }

    pub fn clear_kitty_images(&mut self) {
        if self.picker.protocol_type() == ProtocolType::Kitty {
            let mut out = std::io::stdout();
            let _ = out.write_all(b"\x1b_Ga=d,d=i,i=1,q=2\x1b\\");
            let _ = out.flush();
            self.kitty_drawn_track = None;
        }
    }

    pub fn sync_kitty_artwork(&mut self) {
        if self.picker.protocol_type() != ProtocolType::Kitty {
            return;
        }
        if self.modal != ActiveModal::None {
            if self.kitty_drawn_track.is_some() {
                self.clear_kitty_images();
            }
            return;
        }
        let (Some(track_id), Some(rect), Some(png)) = (
            self.album_art_track_id.clone(),
            self.last_art_rect,
            self.kitty_art_png.as_ref(),
        ) else {
            return;
        };
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let target = (track_id, rect);
        if self.kitty_drawn_track.as_ref() == Some(&target) {
            return;
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let bytes = b64.as_bytes();
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x1b_Ga=d,d=i,i=1,q=2\x1b\\");
        let _ = write!(out, "\x1b[s\x1b[{};{}H", rect.y + 1, rect.x + 1);

        let chunks: Vec<&[u8]> = bytes.chunks(4096).collect();
        for (idx, chunk) in chunks.iter().enumerate() {
            let more = usize::from(idx + 1 < chunks.len());
            if idx == 0 {
                let _ = write!(
                    out,
                    "\x1b_Ga=T,f=100,i=1,q=2,C=1,c={},r={},m={};",
                    rect.width, rect.height, more
                );
            } else {
                let _ = write!(out, "\x1b_Gm={more};");
            }
            let _ = out.write_all(chunk);
            let _ = out.write_all(b"\x1b\\");
        }
        let _ = out.write_all(b"\x1b[u");
        let _ = out.flush();
        self.kitty_drawn_track = Some(target);
    }

    pub fn play_queue_index(&mut self, index: usize) {
        let Some(track) = self.playback.queue.get(index).cloned() else {
            return;
        };
        self.clear_kitty_images();
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.stop().await;
            });
        }
        self.playback.current_index = Some(index);
        self.playback.position_secs = 0.0;
        self.playback.duration_secs = track.duration_secs.unwrap_or(0) as f64;
        self.playback.loading_track = true;
        self.playback.paused = true;
        self.playback.stream_badge = "Loading YTM...".to_string();
        self.lyrics = LyricsState::Loading;
        self.album_art = None;
        self.kitty_art_png = None;
        self.album_art_track_id = Some(track.id.clone());
        self.status_line = format!("Loading: {} — {}", track.title, track.artist);

        let resolver = self.resolver.clone();
        let hifi = self.config.hifi_substitution;
        let http = self.http.clone();
        let tx = self.tx.clone();
        let track_clone = track.clone();

        tokio::spawn(async move {
            match resolver.resolve_fast(&track_clone, hifi).await {
                Ok(stream) => {
                    let _ = tx.send(AppEvent::StreamReady {
                        track_id: track_clone.id.clone(),
                        stream,
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        track_id: track_clone.id.clone(),
                        error: e.to_string(),
                    });
                }
            }
        });

        let lyrics_tx = self.tx.clone();
        let lyrics_http = http.clone();
        let lyrics_ytm = self.ytm.clone();
        let lyrics_track = track.clone();
        tokio::spawn(async move {
            let lyrics = lrclib::fetch_lyrics(&lyrics_http, &lyrics_ytm, &lyrics_track).await;
            let _ = lyrics_tx.send(AppEvent::LyricsReady {
                track_id: lyrics_track.id,
                lyrics,
            });
        });

        if let Some(url) = track.thumbnail_url.clone() {
            let art_tx = self.tx.clone();
            let track_id = track.id.clone();
            tokio::spawn(async move {
                if let Some((img, png_bytes)) = fetch_and_decode_art(&http, &url).await {
                    let _ = art_tx.send(AppEvent::ArtworkReady {
                        track_id,
                        image: img,
                        png_bytes,
                    });
                }
            });
        }
    }

    fn prefetch_next(&self) {
        let Some(next_idx) = self.playback.next_index() else {
            return;
        };
        let Some(next_track) = self.playback.queue.get(next_idx).cloned() else {
            return;
        };
        let resolver = self.resolver.clone();
        let hifi = self.config.hifi_substitution;
        let http = self.http.clone();
        let art_url = next_track.thumbnail_url.clone();
        tokio::spawn(async move {
            resolver.prefetch_hifi_audio(&next_track, hifi).await;
        });
        if let Some(url) = art_url {
            tokio::spawn(async move {
                let _ = fetch_and_decode_art(&http, &url).await;
            });
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
        let spotify = self.spotify.clone();
        let tx = self.tx.clone();
        self.status_line = "Importing Spotify playlist...".to_string();
        tokio::spawn(async move {
            let res = spotify
                .fetch_playlist_tracks_progressive(&id, |name, batch, is_first| {
                    let heading = format!("[SPF] {name}");
                    if is_first {
                        let _ = tx.send(AppEvent::TracksLoaded {
                            playlist_id: None,
                            heading,
                            tracks: batch,
                        });
                    } else {
                        let _ = tx.send(AppEvent::TracksAppended {
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

    pub fn effective_transparent_bg(&self) -> bool {
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

    pub fn sync_appearance(&mut self) {
        let transparent = self.effective_transparent_bg();
        self.theme = self.base_theme.with_transparency(transparent);
        if let Color::Rgb(r, g, b) = self.theme.bg {
            self.picker.set_background_color([r, g, b, 255]);
        }
        if let Some(addr) = self.hyprland_window.clone() {
            let opacity = if transparent {
                self.effective_hyprland_opacity()
            } else {
                1.0
            };
            tokio::spawn(async move {
                hyprland::set_window_opacity(&addr, opacity).await;
            });
        }
    }

    pub fn restore_hyprland_opacity(&self) {
        if let Some(addr) = &self.hyprland_window {
            hyprland::set_window_opacity_sync(addr, 1.0);
        }
    }

    pub fn toggle_transparent_bg(&mut self) {
        self.config.transparent_bg = !self.config.transparent_bg;
        let _ = self.config.save();
        self.sync_appearance();
        self.status_line = format!(
            "Transparent TUI Background: {}",
            if self.config.transparent_bg {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    pub fn cycle_hyprland_opacity(&mut self, forward: bool) {
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
            (idx + 1) % STEPS.len()
        } else {
            (idx + STEPS.len() - 1) % STEPS.len()
        };
        self.config.hyprland_opacity = STEPS[next];
        let _ = self.config.save();
        self.sync_appearance();
        self.status_line = format!(
            "Hyprland Window Opacity: {:.0}%",
            self.config.hyprland_opacity * 100.0
        );
    }

    pub fn cycle_theme(&mut self, forward: bool) {
        if self.themes.is_empty() {
            return;
        }
        let cur = self
            .themes
            .iter()
            .position(|t| t.name == self.base_theme.name)
            .unwrap_or(0);
        let next = if forward {
            (cur + 1) % self.themes.len()
        } else {
            (cur + self.themes.len() - 1) % self.themes.len()
        };
        self.apply_theme_by_index(next);
        self.config.theme = self.base_theme.name.clone();
        let _ = self.config.save();
        self.status_line = format!("Theme: {}", self.base_theme.name);
    }

    pub fn apply_theme_by_index(&mut self, idx: usize) {
        if let Some(t) = self.themes.get(idx).cloned() {
            self.base_theme = t;
            self.sync_appearance();
        }
    }

    pub fn cycle_enhancer(&mut self) {
        let next = self.playback.enhancer.next();
        self.playback.enhancer = next;
        self.config.enhancer_mode = next;
        let _ = self.config.save();
        self.status_line = format!("Audio Enhancer: {}", next.label());
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.set_enhancer(next).await;
            });
        }
    }

    pub fn toggle_pause(&mut self) {
        let new_paused = !self.playback.paused;
        self.playback.paused = new_paused;
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.set_paused(new_paused).await;
            });
        }
    }

    pub fn set_volume(&mut self, vol: u8) {
        let next = vol.min(100);
        if self.playback.volume == next {
            return;
        }
        self.playback.volume = next;
        self.config.volume = next;
        let _ = self.config.save();
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.set_volume(next).await;
            });
        }
    }

    pub fn adjust_volume(&mut self, delta: i8) {
        let next = (self.playback.volume as i16 + delta as i16).clamp(0, 100) as u8;
        self.set_volume(next);
    }

    pub fn tick_playback(&mut self, dt_secs: f64) -> bool {
        if self.playback.paused
            || self.playback.loading_track
            || self.playback.current_track().is_none()
        {
            return false;
        }
        let dur = self.playback.duration_secs;
        if dur > 0.0 {
            self.playback.position_secs = (self.playback.position_secs + dt_secs).min(dur);
        } else {
            self.playback.position_secs += dt_secs;
        }
        true
    }

    pub fn seek(&self, secs: f64) {
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.seek_relative(secs).await;
            });
        }
    }

    pub fn seek_to(&self, secs: f64) {
        if let Some(mpv) = &self.mpv {
            let mpv = Arc::clone(mpv);
            tokio::spawn(async move {
                let _ = mpv.seek_absolute(secs).await;
            });
        }
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

    fn selected_or_playing_track(&self) -> Option<Track> {
        if self.focus == FocusPane::Tracks {
            if let Some(idx) = self.track_table_state.selected() {
                if let Some(t) = self.visible_tracks.get(idx) {
                    return Some(t.clone());
                }
            }
        }
        self.playback.current_track().cloned()
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

    pub fn now_playing_path() -> std::path::PathBuf {
        dirs::runtime_dir()
            .unwrap_or_else(AppConfig::cache_dir)
            .join("sonata")
            .join("now_playing.json")
    }

    pub fn export_now_playing(&mut self) {
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
            LyricsState::Synced(lines) => {
                crate::lyrics::active_line_index(lines, self.playback.position_secs)
                    .and_then(|idx| lines.get(idx))
                    .map(|l| l.text.clone())
                    .filter(|s| !s.is_empty())
            }
            _ => None,
        };

        let art_url = track.and_then(|t| t.thumbnail_url.clone());
        let art_path = art_url
            .as_deref()
            .map(art_cache_path)
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned());

        let fingerprint = ExportFingerprint {
            track_id,
            status: status.to_string(),
            lyric: active_lyric.clone(),
            art_path: art_path.clone(),
            pos_secs,
            dur_secs,
        };
        if self.last_exported_state.as_ref() == Some(&fingerprint) {
            return;
        }
        self.last_exported_state = Some(fingerprint);

        let payload = build_now_playing_json(
            status,
            track,
            &self.playback.stream_badge,
            pos_secs,
            dur_secs,
            active_lyric.as_deref(),
            art_path.as_deref(),
            art_url.as_deref(),
        );

        let path = Self::now_playing_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec(&payload) {
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    pub fn clear_now_playing(&self) {
        let _ = std::fs::remove_file(Self::now_playing_path());
    }
}

fn format_mm_ss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[allow(clippy::too_many_arguments)]
fn build_now_playing_json(
    status: &str,
    track: Option<&Track>,
    quality: &str,
    position_secs: u64,
    duration_secs: u64,
    lyric: Option<&str>,
    art_path: Option<&str>,
    art_url: Option<&str>,
) -> serde_json::Value {
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    serde_json::json!({
        "status": status,
        "title": track.map(|t| t.title.as_str()),
        "artist": track.map(|t| t.artist.as_str()),
        "album": track.map(|t| t.album.as_str()).filter(|s| !s.is_empty()),
        "source": track.map(|t| t.source.short_tag()),
        "quality": if track.is_some() && !quality.is_empty() { Some(quality) } else { None },
        "position_secs": position_secs,
        "duration_secs": duration_secs,
        "position": format_mm_ss(position_secs),
        "duration": format_mm_ss(duration_secs),
        "lyric": lyric,
        "art_path": art_path,
        "art_url": art_url,
        "updated_at": updated_at,
    })
}

fn art_cache_path(url: &str) -> std::path::PathBuf {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    AppConfig::art_cache_dir().join(format!("{hex}.png"))
}

async fn fetch_and_decode_art(http: &Client, url: &str) -> Option<(DynamicImage, Vec<u8>)> {
    let cache_path = art_cache_path(url);

    if let Ok(disk_bytes) = std::fs::read(&cache_path) {
        let bytes_clone = disk_bytes.clone();
        if let Ok(Some(img)) =
            tokio::task::spawn_blocking(move || image::load_from_memory(&bytes_clone).ok()).await
        {
            return Some((img, disk_bytes));
        }
    }

    let resp = http.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let raw_bytes = resp.bytes().await.ok()?.to_vec();

    tokio::task::spawn_blocking(move || {
        let decoded = image::load_from_memory(&raw_bytes).ok()?;
        let scaled = if decoded.width() > 480 || decoded.height() > 480 {
            decoded.resize(480, 480, image::imageops::FilterType::Lanczos3)
        } else {
            decoded
        };
        let mut png_bytes = Vec::new();
        scaled
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .ok()?;
        let _ = std::fs::write(&cache_path, &png_bytes);
        Some((scaled, png_bytes))
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_external_now_playing_json() {
        let track = Track {
            id: "abc".into(),
            title: "Midnight".into(),
            artist: "M83".into(),
            album: "HUWD".into(),
            duration_secs: Some(243),
            source: ProviderSource::YouTubeMusic,
            thumbnail_url: Some("https://img.example/1.jpg".into()),
            audio_badge: None,
            liked: false,
            set_video_id: None,
        };
        let v = build_now_playing_json(
            "Playing",
            Some(&track),
            "320kbps AAC · Hi-Fi",
            65,
            243,
            Some("Waiting in a car"),
            Some("/tmp/art.png"),
            track.thumbnail_url.as_deref(),
        );
        assert_eq!(v["status"], "Playing");
        assert_eq!(v["title"], "Midnight");
        assert_eq!(v["artist"], "M83");
        assert_eq!(v["position"], "1:05");
        assert_eq!(v["duration"], "4:03");
        assert_eq!(v["lyric"], "Waiting in a car");
        assert_eq!(v["art_path"], "/tmp/art.png");
    }
}
