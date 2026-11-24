//! Root GPUI view (`SonataDesktopView`) orchestrating the floating card-on-gutter shell, live CSS themes, and all desktop subviews.
//!
//! 1. **State & Observers**: holds `DesktopSnapshot`, `DesktopSettings`, `ThemeCatalog`, solved `DesktopPalette`, bounded `MediaCache`, input/slider/select entities, and live window activation theme reload.
//! 2. **Navigation & Layout**: maintains back/forward history, `resolve_shell_widths` priority shedding, sidebar rail snapping (`< 100 pt` -> `72 pt`), and drag resizing across the `8 pt` gutter.
//! 3. **Submodules**: `chrome`, `sidebar`, `right_panel`, `player_bar`, `home`, `explore`, `library`, `collection`, `artist`, `search`, `fullscreen`, `settings`, `modals`.

pub mod artist;
pub mod chrome;
pub mod collection;
pub mod explore;
pub mod fullscreen;
pub mod home;
pub mod library;
pub mod modals;
pub mod player_bar;
pub mod right_panel;
pub mod search;
pub mod settings;
pub mod sidebar;

use crate::backend::{BackendCmd, BackendHandle, DesktopSnapshot, LibrarySource, NavTarget};
use crate::background_appearance_for;
use crate::settings::{
    is_mini_viewport, CornerStyle, DesktopSettings, ThemeModePref, GUTTER, PANEL_MAX, PANEL_MIN,
    SIDEBAR_MAX, SIDEBAR_MIN, SIDEBAR_RAIL, SIDEBAR_SNAP_AT,
};
use crate::theme::{sync_gpui_theme, DesktopPalette, ThemeCatalog};
use crate::ui::components::{card_panel, cover_art_tile};
use crate::ui::lyrics::{LyricsWrapCache, SpringState};
use crate::ui::media::{rgba_to_render_image, MediaCache};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{Icon, Sizable as _, TitleBar, WindowBorder};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, AppContext, Bounds, Context, Entity,
    FocusHandle, Focusable, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseMoveEvent, ParentElement as _, Pixels, Point, Render, RenderImage,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};
use sonata_core::providers::servers::ServerKind;
use sonata_core::providers::Track;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Playlists,
    Albums,
    Artists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSubTab {
    Songs,
    Albums,
    Playlists,
    Folders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTab {
    YouTube,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCategory {
    All,
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Playback,
    Sources,
    Library,
    System,
    Account,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenDrawerTab {
    Cover,
    Lyrics,
    Queue,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DesktopModal {
    None,
    SignInBrowser,
    CreatePlaylist,
    EditPlaylist {
        playlist_id: String,
        cover_path: Option<std::path::PathBuf>,
    },
    AddToPlaylist(Track),
    AddServer,
}

#[derive(Debug, Clone)]
pub enum ContextMenuKind {
    Track {
        track: Track,
        playlist_remove_idx: Option<usize>,
    },
    QueueItem {
        index: usize,
        track: Track,
    },
    Collection {
        id: String,
        title: String,
        subtitle: String,
        thumbnail_url: Option<String>,
        is_album: bool,
    },
    Artist {
        id: String,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub pos: Point<f32>,
    pub kind: ContextMenuKind,
}

pub struct SonataDesktopView {
    pub backend: BackendHandle,
    pub snapshot: Arc<DesktopSnapshot>,
    pub live_position_secs: f64,
    pub live_position_at: Instant,
    pub live_duration_secs: f64,
    pub decorated: bool,
    pub window_visible: bool,
    pub mini_mode: bool,
    pub settings: DesktopSettings,
    pub theme_catalog: ThemeCatalog,
    pub palette: DesktopPalette,
    pub media: MediaCache,
    pub focus_handle: FocusHandle,

    pub nav_history: Vec<NavTarget>,
    pub nav_cursor: usize,

    pub library_tab: LibraryTab,
    pub library_source: LibrarySource,
    pub local_sub_tab: LocalSubTab,
    pub selected_local_folder: Option<std::path::PathBuf>,
    pub search_tab: SearchTab,
    pub search_category: SearchCategory,
    pub settings_tab: SettingsTab,
    pub fullscreen_open: bool,
    pub fullscreen_tab: FullscreenDrawerTab,
    pub fullscreen_last_activity: Instant,
    pub fullscreen_awake: bool,
    pub fullscreen_idle_task: Option<gpui_kit::Task<()>>,
    pub fullscreen_art_fade: f32,
    pub fullscreen_ambient_tex: Option<Arc<gpui_kit::RenderImage>>,
    pub fullscreen_ambient_started: Instant,
    pub fullscreen_ambient_stepped: Instant,
    pub fullscreen_ambient_painted: Option<([[f32; 3]; 5], [f32; 3])>,
    pub fullscreen_ambient_target: Option<([[f32; 3]; 5], [f32; 3])>,
    pub fullscreen_ambient_washing: bool,
    pub fullscreen_ambient_timer_active: bool,
    pub fullscreen_lyrics_scroll_y: f32,
    pub fullscreen_lyrics_hovered_row: Option<usize>,
    pub fullscreen_lyrics_last_step: Instant,
    pub fullscreen_lyrics_wrap: LyricsWrapCache,
    pub fullscreen_scroll_spring: SpringState,
    pub fullscreen_row_drifts: HashMap<usize, SpringState>,
    pub fullscreen_last_seen_scroll: f32,
    pub fullscreen_active_row_prev: Option<usize>,
    pub fullscreen_departing_row: Option<usize>,
    pub fullscreen_row_arrived: Instant,
    pub fullscreen_row_departed: Instant,
    pub panel_lyrics_wrap: LyricsWrapCache,
    pub panel_scroll_spring: SpringState,
    pub lyrics_panel_last_step: Instant,
    pub karaoke_timer_active: bool,
    pub search_bar_bounds: Rc<Cell<Bounds<Pixels>>>,

    pub search_input: Entity<InputState>,
    pub library_filter_input: Entity<InputState>,
    pub playlist_name_input: Entity<InputState>,
    pub create_playlist_personal: bool,
    pub downloads_dir_input: Entity<InputState>,
    pub local_dir_input: Entity<InputState>,
    pub discord_app_id_input: Entity<InputState>,
    pub server_name_input: Entity<InputState>,
    pub server_url_input: Entity<InputState>,
    pub server_user_input: Entity<InputState>,
    pub server_pass_input: Entity<InputState>,
    pub server_token_input: Entity<InputState>,
    pub server_share_input: Entity<InputState>,
    pub server_domain_input: Entity<InputState>,
    pub server_mount_input: Entity<InputState>,
    pub server_editing_id: Option<String>,
    pub server_modal_kind: ServerKind,
    pub progress_slider: Entity<SliderState>,
    pub volume_slider: Entity<SliderState>,
    pub opacity_slider: Entity<SliderState>,
    pub theme_select: Entity<SelectState<SearchableVec<SharedString>>>,

    pub progress_dragging: bool,
    pub volume_dragging: bool,
    pub search_dropdown_open: bool,
    pub account_menu_open: bool,
    pub modal: DesktopModal,
    pub context_menu: Option<ContextMenuState>,

    pub sidebar_scroll: ScrollHandle,
    pub main_scroll: ScrollHandle,
    pub queue_scroll: ScrollHandle,
    pub lyrics_scroll: ScrollHandle,
    pub fullscreen_lyrics_scroll: ScrollHandle,
    pub fullscreen_queue_scroll: ScrollHandle,
    pub last_wheel_lyrics: Option<Instant>,
    pub last_active_lyric: Option<usize>,
    pub lyric_prev_idx: Option<usize>,
    pub lyric_changed_at: Instant,
    pub lyric_anim_active: bool,

    pub dragging_sidebar: bool,
    pub dragging_panel: bool,
    pub drag_origin_x: f32,
    pub drag_origin_w: f32,
    pub dragging_playlist_idx: Option<usize>,

    pub loading_started: HashMap<&'static str, Instant>,
    pub motion_source_ptr: Option<usize>,
    pub motion_textures: Vec<Arc<RenderImage>>,
    pub motion_fps: f32,
    pub anim_epoch: Instant,
    pub last_motion_frame_idx: usize,
    pub motion_timer_active: bool,

    pub _subscriptions: Vec<Subscription>,
}

impl SonataDesktopView {
    pub fn new(
        backend: BackendHandle,
        decorated: bool,
        scheme_override: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let snapshot = backend.snapshot();
        let settings = DesktopSettings::load();
        let mut theme_catalog = ThemeCatalog::load(scheme_override);
        let palette = theme_catalog.resolve_palette(&settings);
        sync_gpui_theme(cx, &palette);
        apply_window_appearance(window, &settings, &palette);

        let focus_handle = cx.focus_handle();

        let search_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search songs, artists, albums..."));
        let library_filter_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter playlists, albums, artists...")
        });
        let playlist_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Playlist name"));
        let init_dl_dir = snapshot
            .config
            .downloads_dir
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let downloads_dir_input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).placeholder(
                sonata_runtime::config::AppConfig::default_downloads_dir()
                    .to_string_lossy()
                    .to_string(),
            );
            if !init_dl_dir.is_empty() {
                st.set_value(init_dl_dir, window, cx);
            }
            st
        });
        let init_discord_id = snapshot.config.discord_app_id.clone().unwrap_or_default();
        let discord_app_id_input = cx.new(|cx| {
            let mut st = InputState::new(window, cx)
                .placeholder(sonata_runtime::discord::DEFAULT_SONATA_DISCORD_APP_ID);
            if !init_discord_id.is_empty() {
                st.set_value(init_discord_id, window, cx);
            }
            st
        });
        let server_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Home Music Server"));
        let server_url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://music.example.com"));
        let server_user_input = cx.new(|cx| InputState::new(window, cx).placeholder("Username"));
        let server_pass_input = cx.new(|cx| InputState::new(window, cx).placeholder("Password"));
        let server_token_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("API Key / Bearer Token"));
        let server_share_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Music (optional)"));
        let server_domain_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("WORKGROUP (optional)"));
        let server_mount_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("/mnt/nas/music (optional)"));
        let local_dir_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("/home/user/Music"));

        let dur = snapshot.duration_secs.max(1.0) as f32;
        let pos = (snapshot.position_secs as f32).clamp(0.0, dur);
        let progress_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(dur)
                .step(0.5)
                .default_value(pos)
        });
        let volume_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(if snapshot.muted {
                    0.0
                } else {
                    snapshot.volume as f32
                })
        });
        let opacity_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(125.0)
                .step(1.0)
                .default_value((settings.opacity * 100.0).round().clamp(0.0, 125.0))
        });

        let avail_names: Vec<SharedString> = theme_catalog
            .available_names_for_mode(&settings)
            .into_iter()
            .map(SharedString::from)
            .collect();
        let selected_name: SharedString = theme_catalog.selected_theme_name(&settings).into();
        let theme_select = cx.new(|cx| {
            let mut s = SelectState::new(SearchableVec::new(avail_names), None, window, cx);
            s.set_selected_value(&selected_name, window, cx);
            s
        });

        let mut subscriptions = Vec::new();

        // Reload CSS themes & OS palettes automatically when the window regains focus.
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() && this.window_visible {
                this.reload_themes_and_sync(window, cx);
            }
        }));

        // Freeze all UI work & animations when minimized to taskbar/tray or hidden on another workspace.
        subscriptions.push(
            cx.observe_window_visibility(window, |this, vis, window, cx| {
                this.set_window_visible(vis.is_visible(), window, cx);
            }),
        );

        // Switch into ultra-low-power static mini-player mode when shrunk to mini window dimensions.
        subscriptions.push(cx.observe_window_bounds(window, |this, window, cx| {
            let vp = window.viewport_size();
            let mini = is_mini_viewport(vp.width.into(), vp.height.into());
            this.set_mini_mode(mini, window, cx);
        }));

        subscriptions.push(cx.subscribe_in(
            &search_input,
            window,
            |this, state, event: &InputEvent, _window, cx| match event {
                InputEvent::Focus => {
                    this.search_dropdown_open = true;
                    cx.notify();
                }
                InputEvent::Change => {
                    let q = state.read(cx).value().to_string();
                    this.search_dropdown_open = true;
                    this.backend.send(BackendCmd::FetchSearchSuggestions(q));
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    let q = state.read(cx).value().to_string();
                    if !q.trim().is_empty() {
                        this.search_dropdown_open = false;
                        this.navigate_to(NavTarget::Search, cx);
                        this.backend.send(BackendCmd::Search(q));
                    }
                }
                InputEvent::Blur => {}
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &library_filter_input,
            window,
            |_this, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &playlist_name_input,
            window,
            |this, state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let name = state.read(cx).value().trim().to_string();
                    if !name.is_empty() {
                        this.backend.send(BackendCmd::CreateYtmPlaylist(name));
                        this.modal = DesktopModal::None;
                        cx.notify();
                    }
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &progress_slider,
            window,
            |this, _state, event: &SliderEvent, _window, cx| match event {
                SliderEvent::Change(_) => {
                    this.progress_dragging = true;
                    cx.notify();
                }
                SliderEvent::Release(val) => {
                    this.progress_dragging = false;
                    let secs = slider_scalar(*val).max(0.0) as f64;
                    this.backend.send(BackendCmd::SeekAbsolute(secs));
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &volume_slider,
            window,
            |this, _state, event: &SliderEvent, _window, cx| match event {
                SliderEvent::Change(val) => {
                    this.volume_dragging = true;
                    let v = slider_scalar(*val).round().clamp(0.0, 100.0) as u8;
                    this.backend.send(BackendCmd::SetVolume(v));
                    cx.notify();
                }
                SliderEvent::Release(val) => {
                    this.volume_dragging = false;
                    let v = slider_scalar(*val).round().clamp(0.0, 100.0) as u8;
                    this.backend.send(BackendCmd::SetVolume(v));
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &opacity_slider,
            window,
            |this, _state, event: &SliderEvent, window, cx| {
                let pct = match event {
                    SliderEvent::Change(v) | SliderEvent::Release(v) => slider_scalar(*v),
                };
                this.settings.opacity = (pct / 100.0).clamp(0.0, 1.25);
                this.palette = this.theme_catalog.resolve_palette(&this.settings);
                sync_gpui_theme(cx, &this.palette);
                apply_window_appearance(window, &this.settings, &this.palette);
                if matches!(event, SliderEvent::Release(_)) {
                    this.settings.save();
                }
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &theme_select,
            window,
            |this, _state, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                if let SelectEvent::Confirm(Some(chosen)) = event {
                    let name = chosen.to_string();
                    this.theme_catalog.scheme_override = None;
                    if this.theme_catalog.resolved_is_dark(&this.settings) {
                        this.settings.dark_theme = name;
                    } else {
                        this.settings.light_theme = name;
                    }
                    this.settings.save();
                    this.apply_appearance(window, cx);
                }
            },
        ));

        let mut snap_rx = backend.subscribe();
        let view_weak = cx.entity().downgrade();
        cx.spawn_in(window, async move |_, cx| {
            while snap_rx.changed().await.is_ok() {
                let next = snap_rx.borrow_and_update().clone();
                let res = view_weak.update_in(cx, |this, window, cx| {
                    this.on_snapshot(next, window, cx);
                });
                if res.is_err() {
                    break;
                }
            }
        })
        .detach();

        let mut progress_rx = backend.subscribe_progress();
        let view_prog = cx.entity().downgrade();
        cx.spawn_in(window, async move |_, cx| {
            while progress_rx.changed().await.is_ok() {
                let (pos, dur) = *progress_rx.borrow_and_update();
                let res = view_prog.update_in(cx, |this, window, cx| {
                    this.on_progress(pos, dur, window, cx);
                });
                if res.is_err() {
                    break;
                }
            }
        })
        .detach();

        // Trigger delayed skeleton wakeup once at startup if Home is still loading past 150ms.
        let view_skel = cx.entity().downgrade();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(165))
                .await;
            let _ = view_weak_notify(&view_skel, cx);
        })
        .detach();

        let mut loading_started = HashMap::new();
        if snapshot.home_loading {
            loading_started.insert("home", Instant::now());
        }

        let init_pos = snapshot.position_secs;
        let init_dur = snapshot.duration_secs;
        let init_vp = window.viewport_size();
        let init_mini = is_mini_viewport(init_vp.width.into(), init_vp.height.into());
        let init_sources = snapshot.active_library_sources();
        let initial_source = init_sources
            .first()
            .copied()
            .unwrap_or(LibrarySource::Downloads);
        let init_personal_only = snapshot.config.personal_only_mode;
        let init_signed_in = snapshot.is_signed_in();
        Self {
            backend,
            snapshot,
            live_position_secs: init_pos,
            live_position_at: Instant::now(),
            live_duration_secs: init_dur,
            decorated,
            window_visible: true,
            mini_mode: init_mini,
            settings,
            theme_catalog,
            palette,
            media: MediaCache::default(),
            focus_handle,
            nav_history: vec![if init_personal_only {
                NavTarget::Library
            } else {
                NavTarget::Home
            }],
            nav_cursor: 0,
            library_tab: LibraryTab::Playlists,
            library_source: initial_source,
            local_sub_tab: LocalSubTab::Songs,
            selected_local_folder: None,
            search_tab: if init_personal_only {
                SearchTab::Library
            } else {
                SearchTab::YouTube
            },
            search_category: SearchCategory::All,
            settings_tab: SettingsTab::Appearance,
            fullscreen_open: false,
            fullscreen_tab: FullscreenDrawerTab::Lyrics,
            fullscreen_last_activity: Instant::now(),
            fullscreen_awake: false,
            fullscreen_idle_task: None,
            fullscreen_art_fade: 1.0,
            fullscreen_ambient_tex: None,
            fullscreen_ambient_started: Instant::now(),
            fullscreen_ambient_stepped: Instant::now(),
            fullscreen_ambient_painted: None,
            fullscreen_ambient_target: None,
            fullscreen_ambient_washing: false,
            fullscreen_ambient_timer_active: false,
            fullscreen_lyrics_scroll_y: 0.0,
            fullscreen_lyrics_hovered_row: None,
            fullscreen_lyrics_last_step: Instant::now(),
            fullscreen_lyrics_wrap: LyricsWrapCache::default(),
            fullscreen_scroll_spring: SpringState::default(),
            fullscreen_row_drifts: HashMap::new(),
            fullscreen_last_seen_scroll: 0.0,
            fullscreen_active_row_prev: None,
            fullscreen_departing_row: None,
            fullscreen_row_arrived: Instant::now(),
            fullscreen_row_departed: Instant::now(),
            panel_lyrics_wrap: LyricsWrapCache::default(),
            panel_scroll_spring: SpringState::default(),
            lyrics_panel_last_step: Instant::now(),
            karaoke_timer_active: false,
            search_bar_bounds: Rc::new(Cell::new(Bounds::default())),
            search_input,
            library_filter_input,
            playlist_name_input,
            create_playlist_personal: init_personal_only || !init_signed_in,
            downloads_dir_input,
            local_dir_input,
            discord_app_id_input,
            server_name_input,
            server_url_input,
            server_user_input,
            server_pass_input,
            server_token_input,
            server_share_input,
            server_domain_input,
            server_mount_input,
            server_editing_id: None,
            server_modal_kind: ServerKind::Jellyfin,
            progress_slider,
            volume_slider,
            opacity_slider,
            theme_select,
            progress_dragging: false,
            volume_dragging: false,
            search_dropdown_open: false,
            account_menu_open: false,
            modal: DesktopModal::None,
            context_menu: None,
            sidebar_scroll: ScrollHandle::default(),
            main_scroll: ScrollHandle::default(),
            queue_scroll: ScrollHandle::default(),
            lyrics_scroll: ScrollHandle::default(),
            fullscreen_lyrics_scroll: ScrollHandle::default(),
            fullscreen_queue_scroll: ScrollHandle::default(),
            last_wheel_lyrics: None,
            last_active_lyric: None,
            lyric_prev_idx: None,
            lyric_changed_at: Instant::now(),
            lyric_anim_active: false,
            dragging_sidebar: false,
            dragging_panel: false,
            drag_origin_x: 0.0,
            drag_origin_w: 0.0,
            dragging_playlist_idx: None,
            loading_started,
            motion_source_ptr: None,
            motion_textures: Vec::new(),
            motion_fps: 24.0,
            anim_epoch: Instant::now(),
            last_motion_frame_idx: 0,
            motion_timer_active: false,
            _subscriptions: subscriptions,
        }
    }

    /// Returns true whenever animations, periodic ticks, and heavy UI rendering should stay frozen.
    pub fn ui_suspended(&self) -> bool {
        !self.window_visible || self.mini_mode
    }

    fn suspend_ui_work(&mut self, cx: &mut Context<Self>) {
        self.drop_motion_textures(cx);
        self.fullscreen_idle_task = None;
        self.fullscreen_ambient_timer_active = false;
        self.karaoke_timer_active = false;
        self.lyric_anim_active = false;
        self.motion_timer_active = false;
    }

    fn sync_sliders_from_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.progress_dragging {
            let max_dur = self.live_duration_secs.max(1.0) as f32;
            let cur_pos = (self.live_position_secs as f32).clamp(0.0, max_dur);
            self.progress_slider.update(cx, |s, cx| {
                let min_v = s.min_value();
                if (s.max_value() - max_dur).abs() > 0.5 || min_v != 0.0 {
                    *s = SliderState::new()
                        .min(0.0)
                        .max(max_dur)
                        .step(0.5)
                        .default_value(cur_pos);
                } else {
                    s.set_value(cur_pos, window, cx);
                }
            });
        }
        if !self.volume_dragging {
            let target_vol = if self.snapshot.muted {
                0.0
            } else {
                self.snapshot.volume as f32
            };
            self.volume_slider.update(cx, |s, cx| {
                s.set_value(target_vol, window, cx);
            });
        }
    }

    fn set_window_visible(&mut self, visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_visible == visible {
            return;
        }
        self.window_visible = visible;
        self.backend.send(BackendCmd::SetWindowVisible(visible));
        if !visible {
            self.suspend_ui_work(cx);
        } else {
            self.sync_sliders_from_state(window, cx);
            if window.is_window_active() {
                self.reload_themes_and_sync(window, cx);
            }
            cx.notify();
        }
    }

    fn set_mini_mode(&mut self, mini: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.mini_mode == mini {
            return;
        }
        self.mini_mode = mini;
        self.backend.send(BackendCmd::SetUiCompact(mini));
        if mini {
            self.suspend_ui_work(cx);
            self.search_dropdown_open = false;
            self.account_menu_open = false;
            self.context_menu = None;
        } else if self.window_visible {
            self.sync_sliders_from_state(window, cx);
        }
        if self.window_visible {
            cx.notify();
        }
    }

    fn on_snapshot(
        &mut self,
        next: Arc<DesktopSnapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nav_changed = self.snapshot.active_nav != next.active_nav;
        let track_changed = self.snapshot.current_track().map(|t| t.id.as_str())
            != next.current_track().map(|t| t.id.as_str());
        let thumbs_changed = self.snapshot.thumb_rev != next.thumb_rev;
        let mini_visual_changed = track_changed
            || thumbs_changed
            || self.snapshot.paused != next.paused
            || self.snapshot.loading_track != next.loading_track
            || self.snapshot.art_path != next.art_path;

        self.live_position_secs = next.position_secs;
        self.live_position_at = Instant::now();
        self.live_duration_secs = next.duration_secs;
        self.snapshot = next;

        // While hidden/minimized to taskbar/tray, freeze all UI updates and skip redrawing completely.
        if !self.window_visible {
            return;
        }

        if self.mini_mode {
            if track_changed || thumbs_changed {
                self.media.clear_missing();
            }
            if mini_visual_changed {
                cx.notify();
            }
            return;
        }

        self.track_loading_flag("home", self.snapshot.home_loading);
        self.track_loading_flag("explore", self.snapshot.explore_loading);
        self.track_loading_flag("mood", self.snapshot.mood_page_loading);
        self.track_loading_flag("artist", self.snapshot.artist_loading);
        self.track_loading_flag("album", self.snapshot.album_loading);
        self.track_loading_flag("playlist", self.snapshot.playlist_loading);
        self.track_loading_flag("search", self.snapshot.search_loading);
        self.track_loading_flag("radio", self.snapshot.radio_loading);
        self.track_loading_flag("history", self.snapshot.history_loading);
        if nav_changed || track_changed || thumbs_changed {
            self.media.clear_missing();
        }

        if nav_changed {
            self.push_history_if_needed(self.snapshot.active_nav.clone());
            self.main_scroll
                .set_offset(gpui_kit::point(px(0.0), px(0.0)));
        }

        if track_changed {
            self.last_active_lyric = None;
            self.lyric_prev_idx = None;
            self.fullscreen_art_fade = 1.0;
            self.fullscreen_lyrics_scroll_y = 0.0;
            self.fullscreen_lyrics_hovered_row = None;
            self.fullscreen_lyrics_wrap.clear();
            self.panel_lyrics_wrap.clear();
            self.fullscreen_scroll_spring = SpringState::default();
            self.panel_scroll_spring = SpringState::default();
            self.fullscreen_row_drifts.clear();
            self.fullscreen_last_seen_scroll = 0.0;
            self.fullscreen_active_row_prev = None;
            self.fullscreen_departing_row = None;
            self.lyrics_scroll
                .set_offset(gpui_kit::point(px(0.0), px(0.0)));
            self.fullscreen_lyrics_scroll
                .set_offset(gpui_kit::point(px(0.0), px(0.0)));
        }

        if !self.progress_dragging {
            let max_dur = self.live_duration_secs.max(1.0) as f32;
            let cur_pos = (self.live_position_secs as f32).clamp(0.0, max_dur);
            self.progress_slider.update(cx, |s, cx| {
                // Reconfigure bounds without resetting interaction state.
                let min_v = s.min_value();
                if (s.max_value() - max_dur).abs() > 0.5 || min_v != 0.0 {
                    *s = SliderState::new()
                        .min(0.0)
                        .max(max_dur)
                        .step(0.5)
                        .default_value(cur_pos);
                } else {
                    s.set_value(cur_pos, window, cx);
                }
            });
        }

        if !self.volume_dragging {
            let target_vol = if self.snapshot.muted {
                0.0
            } else {
                self.snapshot.volume as f32
            };
            self.volume_slider.update(cx, |s, cx| {
                if let SliderValue::Single(cur) = s.value() {
                    if (cur - target_vol).abs() > 0.4 {
                        s.set_value(target_vol, window, cx);
                    }
                }
            });
        }

        self.sync_motion_cover_textures(cx);
        self.sync_lyric_transition(cx);
        self.maybe_auto_scroll_lyrics();
        cx.notify();
    }

    fn track_loading_flag(&mut self, key: &'static str, active: bool) {
        if active {
            self.loading_started.entry(key).or_insert_with(Instant::now);
        } else {
            self.loading_started.remove(key);
        }
    }

    pub fn push_history_if_needed(&mut self, target: NavTarget) {
        if self.nav_history.get(self.nav_cursor) == Some(&target) {
            return;
        }
        self.nav_history.truncate(self.nav_cursor + 1);
        self.nav_history.push(target);
        if self.nav_history.len() > 32 {
            let drain = self.nav_history.len() - 32;
            self.nav_history.drain(..drain);
        }
        self.nav_cursor = self.nav_history.len().saturating_sub(1);
    }

    pub fn navigate_to(&mut self, target: NavTarget, cx: &mut Context<Self>) {
        self.search_dropdown_open = false;
        self.context_menu = None;
        self.account_menu_open = false;
        self.fullscreen_open = false;
        self.push_history_if_needed(target.clone());
        self.backend.send(BackendCmd::Navigate(target));
        cx.notify();
    }

    pub fn can_go_back(&self) -> bool {
        self.nav_cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.nav_cursor + 1 < self.nav_history.len()
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.nav_cursor > 0 {
            self.nav_cursor -= 1;
            let target = self.nav_history[self.nav_cursor].clone();
            self.backend.send(BackendCmd::Navigate(target));
            cx.notify();
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.nav_cursor + 1 < self.nav_history.len() {
            self.nav_cursor += 1;
            let target = self.nav_history[self.nav_cursor].clone();
            self.backend.send(BackendCmd::Navigate(target));
            cx.notify();
        }
    }

    pub fn reload_themes_and_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_catalog.reload();
        self.apply_appearance(window, cx);
    }

    pub fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = self.theme_catalog.resolve_palette(&self.settings);
        sync_gpui_theme(cx, &self.palette);
        apply_window_appearance(window, &self.settings, &self.palette);

        let avail_names: Vec<SharedString> = self
            .theme_catalog
            .available_names_for_mode(&self.settings)
            .into_iter()
            .map(SharedString::from)
            .collect();
        let selected_name: SharedString = self
            .theme_catalog
            .selected_theme_name(&self.settings)
            .into();
        self.theme_select.update(cx, |s, cx| {
            s.set_items(SearchableVec::new(avail_names), window, cx);
            s.set_selected_value(&selected_name, window, cx);
        });
        cx.notify();
    }

    pub fn set_theme_mode(
        &mut self,
        mode: ThemeModePref,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.theme_mode = mode;
        self.settings.save();
        self.apply_appearance(window, cx);
    }

    pub fn set_corners(
        &mut self,
        corners: CornerStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.corners = corners;
        self.settings.save();
        self.apply_appearance(window, cx);
    }

    pub fn toggle_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.blur = !self.settings.blur;
        self.settings.save();
        self.apply_appearance(window, cx);
    }

    pub fn toggle_card_outline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.card_outline = !self.settings.card_outline;
        self.settings.save();
        self.apply_appearance(window, cx);
    }

    pub fn toggle_compositor_corners(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.compositor_corners = !self.settings.compositor_corners;
        self.settings.save();
        self.apply_appearance(window, cx);
    }

    pub fn toggle_scroll_fades(&mut self, cx: &mut Context<Self>) {
        self.settings.scroll_fades = !self.settings.scroll_fades;
        self.settings.save();
        cx.notify();
    }

    pub fn toggle_adaptive_art_color(&mut self, cx: &mut Context<Self>) {
        self.settings.adaptive_art_color = !self.settings.adaptive_art_color;
        self.settings.save();
        cx.notify();
    }

    pub fn toggle_double_click_to_play(&mut self, cx: &mut Context<Self>) {
        self.settings.double_click_to_play = !self.settings.double_click_to_play;
        self.settings.save();
        cx.notify();
    }

    pub fn ordered_playlists(&self) -> Vec<sonata_core::providers::Playlist> {
        let mut all: Vec<sonata_core::providers::Playlist> = self
            .snapshot
            .personal_playlists
            .iter()
            .map(|p| p.to_playlist())
            .collect();
        if !self.snapshot.config.personal_only_mode {
            for pl in &self.snapshot.ytm_playlists {
                all.push(pl.clone());
            }
            for saved in &self.settings.saved_external_playlists {
                let norm = crate::settings::normalize_collection_id(&saved.id);
                if !all
                    .iter()
                    .any(|p| crate::settings::normalize_collection_id(&p.id) == norm)
                {
                    all.push(sonata_core::providers::Playlist {
                        id: saved.id.clone(),
                        name: saved.title.clone(),
                        source: sonata_core::providers::ProviderSource::YouTubeMusic,
                        track_count: None,
                        thumbnail_url: saved.thumbnail_url.clone(),
                        is_owned: false,
                    });
                }
            }
        }
        let order_map: HashMap<&str, usize> = self
            .settings
            .sidebar_playlist_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        all.sort_by_key(|p| {
            if let Some(pin_idx) = self.settings.pin_index(&p.id) {
                (0, pin_idx)
            } else if p.id == "VLLM" {
                (1, 0)
            } else if let Some(&idx) = order_map.get(p.id.as_str()) {
                (2, idx)
            } else {
                (3, usize::MAX)
            }
        });
        all
    }

    pub fn toggle_pin_collection(
        &mut self,
        item: crate::settings::PinnedCollection,
        cx: &mut Context<Self>,
    ) {
        self.settings.toggle_pin(item);
        self.settings.save();
        cx.notify();
    }

    pub fn is_playlist_in_library(&self, id: &str) -> bool {
        let norm = crate::settings::normalize_collection_id(id);
        self.settings.is_saved_external(id)
            || self
                .snapshot
                .ytm_playlists
                .iter()
                .any(|p| crate::settings::normalize_collection_id(&p.id) == norm)
    }

    pub fn toggle_save_external_playlist(
        &mut self,
        item: crate::settings::PinnedCollection,
        cx: &mut Context<Self>,
    ) {
        let save = !self.is_playlist_in_library(&item.id);
        self.settings.set_saved_external(item.clone(), save);
        self.settings.save();
        self.backend.send(BackendCmd::ToggleSavePlaylist {
            id: item.id,
            title: item.title,
            thumbnail_url: item.thumbnail_url,
            save,
        });
        cx.notify();
    }

    pub fn reorder_playlist(&mut self, from_id: &str, to_id: &str, cx: &mut Context<Self>) {
        if from_id == to_id || from_id == "VLLM" || to_id == "VLLM" {
            return;
        }
        let mut ids: Vec<String> = self
            .ordered_playlists()
            .into_iter()
            .filter(|p| p.id != "VLLM")
            .map(|p| p.id)
            .collect();
        let Some(from_idx) = ids.iter().position(|id| id == from_id) else {
            return;
        };
        let Some(to_idx) = ids.iter().position(|id| id == to_id) else {
            return;
        };
        let item = ids.remove(from_idx);
        ids.insert(to_idx, item);
        self.settings.sidebar_playlist_order = ids;
        self.settings.save();
        cx.notify();
    }

    pub fn current_motion_image(&self) -> Option<Arc<RenderImage>> {
        if self.snapshot.config.reduce_motion || self.motion_textures.is_empty() {
            return None;
        }
        self.motion_textures
            .get(self.last_motion_frame_idx)
            .cloned()
    }

    fn has_animated_cover(&self) -> bool {
        !self.ui_suspended()
            && self.fullscreen_open
            && !self.snapshot.config.reduce_motion
            && !self.motion_textures.is_empty()
    }

    fn drop_motion_textures(&mut self, cx: &mut Context<Self>) {
        for tex in self.motion_textures.drain(..) {
            cx.drop_image(tex, None);
        }
        self.motion_source_ptr = None;
        self.last_motion_frame_idx = 0;
    }

    fn sync_motion_cover_textures(&mut self, cx: &mut Context<Self>) {
        if self.ui_suspended() || !self.fullscreen_open || self.snapshot.config.reduce_motion {
            if !self.motion_textures.is_empty() {
                self.drop_motion_textures(cx);
            }
            return;
        }
        let next_ptr = self
            .snapshot
            .motion_cover
            .as_ref()
            .map(|c| Arc::as_ptr(c) as usize);
        if next_ptr != self.motion_source_ptr {
            self.drop_motion_textures(cx);
            self.motion_source_ptr = next_ptr;
            self.last_motion_frame_idx = 0;
            self.anim_epoch = Instant::now();
            if let Some(cover) = &self.snapshot.motion_cover {
                self.motion_fps = cover.fps.clamp(1.0, 30.0);
                self.motion_textures = cover
                    .frames_rgba
                    .iter()
                    .filter_map(|frame| rgba_to_render_image(cover.width, cover.height, frame))
                    .collect();
            } else {
                self.motion_fps = 15.0;
            }
        }
        if self.has_animated_cover() && !self.motion_timer_active {
            self.motion_timer_active = true;
            let tick = Duration::from_secs_f32(1.0 / self.motion_fps.clamp(10.0, 24.0));
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(tick).await;
                let keep = this.update(cx, |view, cx| {
                    if !view.has_animated_cover() {
                        view.motion_timer_active = false;
                        return false;
                    }
                    if view.snapshot.paused {
                        return true;
                    }
                    let next_idx = ((view.anim_epoch.elapsed().as_secs_f32()
                        * view.motion_fps.clamp(10.0, 24.0))
                        as usize)
                        % view.motion_textures.len();
                    if next_idx != view.last_motion_frame_idx {
                        view.last_motion_frame_idx = next_idx;
                        cx.notify();
                    }
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            })
            .detach();
        }
    }

    /// Interpolates the live playhead between backend progress ticks for smooth 45–60 FPS karaoke sweeps.
    pub fn live_position_now(&self) -> f64 {
        if self.snapshot.paused
            || self.snapshot.loading_track
            || self.snapshot.current_track().is_none()
        {
            return self.live_position_secs;
        }
        let elapsed = self
            .live_position_at
            .elapsed()
            .as_secs_f64()
            .clamp(0.0, 1.5);
        let max_pos = self.live_duration_secs.max(self.live_position_secs);
        (self.live_position_secs + elapsed).min(max_pos)
    }

    /// Paces smooth 45 Hz karaoke frames only while synced lyrics are actively visible and playing.
    pub fn ensure_karaoke_pace(&mut self, cx: &mut Context<Self>) {
        if self.karaoke_timer_active || self.ui_suspended() {
            return;
        }
        self.karaoke_timer_active = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(22))
                .await;
            let keep = this.update(cx, |view, cx| {
                if view.ui_suspended() {
                    view.karaoke_timer_active = false;
                    return false;
                }
                let lyrics_visible = (view.fullscreen_open
                    && view.fullscreen_tab == FullscreenDrawerTab::Lyrics)
                    || (!view.fullscreen_open && view.settings.show_lyrics_panel);
                let synced = matches!(
                    view.snapshot.lyrics,
                    sonata_core::lyrics::LyricsState::Synced(ref lines) if !lines.is_empty()
                );
                let playing = !view.snapshot.paused
                    && !view.snapshot.loading_track
                    && view.snapshot.current_track().is_some();
                if !lyrics_visible || !synced || !playing {
                    view.karaoke_timer_active = false;
                    return false;
                }
                cx.notify();
                true
            });
            if !matches!(keep, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    fn on_progress(&mut self, pos: f64, dur: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.live_position_secs = pos;
        self.live_position_at = Instant::now();
        if dur > 0.0 {
            self.live_duration_secs = dur;
        }
        if self.ui_suspended() {
            return;
        }
        if !self.progress_dragging {
            let max_dur = self.live_duration_secs.max(1.0) as f32;
            let cur_pos = (self.live_position_secs as f32).clamp(0.0, max_dur);
            self.progress_slider.update(cx, |s, cx| {
                let min_v = s.min_value();
                if (s.max_value() - max_dur).abs() > 0.5 || min_v != 0.0 {
                    *s = SliderState::new()
                        .min(0.0)
                        .max(max_dur)
                        .step(0.5)
                        .default_value(cur_pos);
                } else {
                    s.set_value(cur_pos, window, cx);
                }
            });
        }
        self.sync_lyric_transition(cx);
        self.maybe_auto_scroll_lyrics();
        cx.notify();
    }

    fn sync_lyric_transition(&mut self, cx: &mut Context<Self>) {
        if self.ui_suspended() {
            return;
        }
        let next_idx = self.snapshot.active_lyric_index_at(self.live_position_secs);
        if next_idx != self.last_active_lyric {
            self.lyric_prev_idx = self.last_active_lyric;
            self.lyric_changed_at = Instant::now();
            if !self.lyric_anim_active {
                self.lyric_anim_active = true;
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let keep = this.update(cx, |view, cx| {
                        if view.ui_suspended() {
                            view.lyric_anim_active = false;
                            return false;
                        }
                        let done = view.lyric_changed_at.elapsed() >= Duration::from_millis(340);
                        cx.notify();
                        if done {
                            view.lyric_anim_active = false;
                            false
                        } else {
                            true
                        }
                    });
                    if !matches!(keep, Ok(true)) {
                        break;
                    }
                })
                .detach();
            }
        }
    }

    /// Computes a smooth `0.0..=1.0` glow factor for lyric row `idx` using cubic ease-out over `320 ms`.
    pub fn lyric_glow_factor(
        &self,
        idx: usize,
        active_lead: Option<usize>,
        active_bg: &[usize],
    ) -> f32 {
        let raw = (self.lyric_changed_at.elapsed().as_secs_f32() / 0.32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - raw).powi(3);
        if active_lead == Some(idx) {
            eased
        } else if active_bg.contains(&idx) {
            1.0
        } else if self.lyric_prev_idx == Some(idx) && raw < 1.0 {
            1.0 - eased
        } else {
            0.0
        }
    }

    fn maybe_auto_scroll_lyrics(&mut self) {
        if let Some(active_idx) = self.snapshot.active_lyric_index_at(self.live_position_secs) {
            self.last_active_lyric = Some(active_idx);
        }
    }

    pub fn stir_fullscreen(&mut self, cx: &mut Context<Self>) {
        self.fullscreen_last_activity = Instant::now();
        if !self.fullscreen_awake {
            self.fullscreen_awake = true;
            cx.notify();
        }
        self.fullscreen_idle_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1400))
                .await;
            let _ = this.update(cx, |view, cx| {
                if !view.fullscreen_open {
                    view.fullscreen_awake = false;
                    return;
                }
                if view.fullscreen_last_activity.elapsed() < Duration::from_millis(1350)
                    || view.progress_dragging
                {
                    return;
                }
                if view.fullscreen_awake {
                    view.fullscreen_awake = false;
                    cx.notify();
                }
            });
        }));
    }

    pub fn media_for_url(
        &self,
        url: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Option<crate::ui::media::CachedMedia> {
        let weak = cx.entity().downgrade();
        self.media.get_for_url(url, cx, move |cx| {
            let _ = weak.update(cx, |_, cx| cx.notify());
        })
    }

    pub fn media_for_path(
        &self,
        path: Option<&std::path::Path>,
        cx: &mut Context<Self>,
    ) -> Option<crate::ui::media::CachedMedia> {
        let p = path?;
        let weak = cx.entity().downgrade();
        self.media.get_for_path(p, cx, move |cx| {
            let _ = weak.update(cx, |_, cx| cx.notify());
        })
    }

    pub fn open_album_for_track(&mut self, track: &Track, cx: &mut Context<Self>) {
        self.backend.send(BackendCmd::OpenAlbum {
            album_id: track.album_id.clone(),
            title: track.album.clone(),
            artist: track.artist.clone(),
            thumbnail_url: track.thumbnail_url.clone(),
        });
        cx.notify();
    }

    pub fn open_artist_by_name(&mut self, name: &str, cx: &mut Context<Self>) {
        self.fullscreen_open = false;
        self.backend
            .send(BackendCmd::OpenArtistByName(name.to_string()));
        cx.notify();
    }

    fn handle_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Ignore single-key shortcuts while any input field has keyboard focus.
        let input_focused = self
            .search_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
            || self
                .library_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .playlist_name_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .downloads_dir_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .discord_app_id_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_name_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_url_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_user_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_pass_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_token_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_share_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_domain_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .server_mount_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
            || self
                .local_dir_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);

        let key = ev.keystroke.key.as_str();
        if ev.keystroke.modifiers.control && key.eq_ignore_ascii_case("k") {
            self.fullscreen_open = false;
            self.search_dropdown_open = true;
            self.search_input.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
            return;
        }
        if key == "escape" {
            if self.context_menu.is_some() {
                self.context_menu = None;
            } else if !matches!(self.modal, DesktopModal::None) {
                self.modal = DesktopModal::None;
            } else if self.search_dropdown_open {
                self.search_dropdown_open = false;
            } else if self.account_menu_open {
                self.account_menu_open = false;
            } else if self.fullscreen_open {
                self.fullscreen_open = false;
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
            }
            cx.notify();
            return;
        }
        if key == "f11" {
            window.toggle_fullscreen();
            self.fullscreen_open = window.is_fullscreen() || !self.fullscreen_open;
            if self.fullscreen_open {
                self.stir_fullscreen(cx);
            }
            cx.notify();
            return;
        }
        if input_focused {
            return;
        }

        match key {
            "/" => {
                self.fullscreen_open = false;
                self.search_dropdown_open = true;
                self.search_input.update(cx, |s, cx| s.focus(window, cx));
            }
            " " => self.backend.send(BackendCmd::TogglePause),
            "f" => {
                self.fullscreen_open = !self.fullscreen_open;
                if self.fullscreen_open {
                    self.stir_fullscreen(cx);
                }
            }
            "m" => self.backend.send(BackendCmd::ToggleMute),
            "s" => self.backend.send(BackendCmd::ToggleShuffle),
            "r" => self.backend.send(BackendCmd::CycleRepeat),
            "left" if ev.keystroke.modifiers.alt => self.go_back(cx),
            "right" if ev.keystroke.modifiers.alt => self.go_forward(cx),
            "left" => self.backend.send(BackendCmd::SeekRelative(-5.0)),
            "right" => self.backend.send(BackendCmd::SeekRelative(5.0)),
            _ => return,
        }
        cx.notify();
    }

    pub fn render_center_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.snapshot.active_nav.clone() {
            NavTarget::Home => self.render_home_page(window, cx),
            NavTarget::Explore => self.render_explore_page(window, cx),
            NavTarget::MoodCategory { .. } => self.render_mood_category_page(window, cx),
            NavTarget::ShelfDetail { .. } => self.render_shelf_detail_page(window, cx),
            NavTarget::Radio => self.render_radio_page(window, cx),
            NavTarget::Library => self.render_library_page(window, cx),
            NavTarget::History => self.render_history_page(window, cx),
            NavTarget::Queue => self.render_queue_page(window, cx),
            NavTarget::Playlist(_) => self.render_playlist_page(window, cx),
            NavTarget::Album(_) => self.render_album_page(window, cx),
            NavTarget::Artist(_) => self.render_artist_page(window, cx),
            NavTarget::Search => self.render_search_page(window, cx),
            NavTarget::Settings => self.render_settings_page(window, cx),
        }
    }
    fn render_mini_player(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.snapshot.current_track().cloned();
        let art = self
            .media_for_path(self.snapshot.art_path.as_deref(), cx)
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|t| self.media_for_url(t.thumbnail_url.as_deref(), cx))
            });
        let title = current
            .as_ref()
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "Nothing playing".to_string());
        let artist = current
            .as_ref()
            .map(|t| t.artist.clone())
            .unwrap_or_default();

        // Ultra-lightweight static mini-player: zero timers, zero animations, audio-first idle power.
        div()
            .id("sonata-mini-root")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(self.palette.card_bg)
            .text_color(self.palette.text)
            .font_family("Inter")
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(
                TitleBar::new()
                    .h(px(28.0))
                    .px(px(10.0))
                    .border_b_0()
                    .bg(gpui_kit::transparent_black())
                    .on_close_window(|_, window, _| {
                        window.remove_window();
                    })
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.palette.dim)
                            .child("Sonata · Low-Power Mini"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(12.0))
                    .pb(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(cover_art_tile(
                        "mini-art",
                        art.as_ref(),
                        44.0,
                        false,
                        &self.palette,
                        false,
                        None,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .truncate()
                                    .child(title),
                            )
                            .when(!artist.is_empty(), |col| {
                                col.child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(self.palette.secondary)
                                        .truncate()
                                        .child(artist),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("mini-prev")
                                    .ghost()
                                    .small()
                                    .icon(IconName::SkipBack)
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::PrevTrack);
                                    })),
                            )
                            .child(
                                div()
                                    .id("mini-play")
                                    .size(px(34.0))
                                    .rounded_full()
                                    .bg(self.palette.accent)
                                    .text_color(self.palette.on_accent)
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::TogglePause);
                                    }))
                                    .child(
                                        Icon::new(if self.snapshot.paused {
                                            IconName::Play
                                        } else {
                                            IconName::Pause
                                        })
                                        .size(px(15.0)),
                                    ),
                            )
                            .child(
                                Button::new("mini-next")
                                    .ghost()
                                    .small()
                                    .icon(IconName::SkipForward)
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::NextTrack);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Focusable for SonataDesktopView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SonataDesktopView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.window_visible {
            return WindowBorder::new().child(div().size_full());
        }
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
        let vp_h: f32 = vp.height.into();
        let mini = is_mini_viewport(vp_w, vp_h);
        if mini != self.mini_mode {
            self.mini_mode = mini;
            self.backend.send(BackendCmd::SetUiCompact(mini));
            if mini {
                self.suspend_ui_work(cx);
            }
        }
        if self.mini_mode {
            return WindowBorder::new().child(self.render_mini_player(cx));
        }

        self.sync_motion_cover_textures(cx);
        let (sidebar_w, right_w) = self.settings.resolve_shell_widths(vp_w);
        let is_rail = sidebar_w <= SIDEBAR_RAIL + 1.0;

        let main_shell = div()
            .id("sonata-desktop-root")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(self.palette.gutter_bg)
            .text_color(self.palette.text)
            .font_family("Inter")
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                let mouse_x: f32 = ev.position.x.into();
                if this.fullscreen_open {
                    this.stir_fullscreen(cx);
                }
                if this.dragging_sidebar {
                    let delta = mouse_x - this.drag_origin_x;
                    let raw = this.drag_origin_w + delta;
                    this.settings.sidebar_width = if raw < SIDEBAR_SNAP_AT {
                        SIDEBAR_RAIL
                    } else {
                        raw.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
                    };
                    cx.notify();
                } else if this.dragging_panel {
                    let delta = this.drag_origin_x - mouse_x;
                    this.settings.panel_width =
                        (this.drag_origin_w + delta).clamp(PANEL_MIN, PANEL_MAX);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.dragging_sidebar || this.dragging_panel {
                        this.dragging_sidebar = false;
                        this.dragging_panel = false;
                        this.settings.save();
                        cx.notify();
                    }
                    if this.dragging_playlist_idx.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .when(!self.fullscreen_open, |root| {
                root.child(self.render_top_bar(window, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .px(px(GUTTER))
                            .flex()
                            .flex_row()
                            .child(self.render_sidebar(sidebar_w, is_rail, window, cx))
                            .child(
                                // Left gutter gap acts as an interactive sidebar resize handle.
                                div()
                                    .id("sidebar-resize-gutter")
                                    .w(px(GUTTER))
                                    .h_full()
                                    .flex_shrink_0()
                                    .cursor_col_resize()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |this, ev: &gpui_kit::MouseDownEvent, _, _| {
                                                this.dragging_sidebar = true;
                                                this.drag_origin_x = ev.position.x.into();
                                                this.drag_origin_w = sidebar_w;
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                card_panel(&self.palette)
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .relative()
                                    .child(self.render_center_page(window, cx)),
                            )
                            .when(right_w > 0.0, |row| {
                                row.child(
                                    div()
                                        .id("panel-resize-gutter")
                                        .w(px(GUTTER))
                                        .h_full()
                                        .flex_shrink_0()
                                        .cursor_col_resize()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |this, ev: &gpui_kit::MouseDownEvent, _, _| {
                                                    this.dragging_panel = true;
                                                    this.drag_origin_x = ev.position.x.into();
                                                    this.drag_origin_w = right_w;
                                                },
                                            ),
                                        ),
                                )
                                .child(self.render_right_panel(right_w, window, cx))
                            }),
                    )
                    .child(self.render_player_bar(vp_w, window, cx))
            })
            .when(self.fullscreen_open, |root| {
                root.child(self.render_fullscreen_overlay(window, cx))
            })
            .when(self.search_dropdown_open && !self.fullscreen_open, |root| {
                root.child(self.render_search_dropdown(vp_w, window, cx))
            })
            .when(self.account_menu_open, |root| {
                root.child(self.render_account_popover(window, cx))
            })
            .when(self.context_menu.is_some(), |root| {
                root.child(self.render_context_menu(window, cx))
            })
            .when(!matches!(self.modal, DesktopModal::None), |root| {
                root.child(self.render_modal_overlay(window, cx))
            });

        WindowBorder::new().child(main_shell)
    }
}

fn slider_scalar(val: SliderValue) -> f32 {
    match val {
        SliderValue::Single(v) => v,
        SliderValue::Range(_, end) => end,
    }
}

fn view_weak_notify(
    weak: &gpui_kit::WeakEntity<SonataDesktopView>,
    cx: &mut gpui_kit::AsyncWindowContext,
) -> Result<(), anyhow::Error> {
    weak.update(cx, |_, cx| {
        cx.notify();
    })
}

pub fn apply_window_appearance(
    window: &mut Window,
    settings: &DesktopSettings,
    palette: &DesktopPalette,
) {
    window.set_background_appearance(background_appearance_for(settings));
    window.set_client_corner_radii(gpui_kit::Corners::all(px(palette.window_radius)));
}
