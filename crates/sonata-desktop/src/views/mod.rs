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

use crate::backend::{BackendCmd, BackendHandle, DesktopSnapshot, NavTarget};
use crate::background_appearance_for;
use crate::settings::{
    CornerStyle, DesktopSettings, ThemeModePref, GUTTER, PANEL_MAX, PANEL_MIN, SIDEBAR_MAX,
    SIDEBAR_MIN, SIDEBAR_RAIL, SIDEBAR_SNAP_AT,
};
use crate::theme::{sync_gpui_theme, DesktopPalette, ThemeCatalog};
use crate::ui::components::card_panel;
use crate::ui::media::{rgba_to_render_image, MediaCache};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::WindowBorder;
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, AppContext, Context, Entity,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseMoveEvent, ParentElement as _, Point, Render, RenderImage, ScrollHandle, SharedString,
    Styled as _, Subscription, Window,
};
use sonata_core::lyrics::LyricsState;
use sonata_core::providers::Track;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Playlists,
    Albums,
    Artists,
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
    Library,
    System,
    Account,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenDrawerTab {
    Lyrics,
    Queue,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DesktopModal {
    None,
    SignInBrowser,
    CreatePlaylist,
    AddToPlaylist(Track),
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
    pub decorated: bool,
    pub settings: DesktopSettings,
    pub theme_catalog: ThemeCatalog,
    pub palette: DesktopPalette,
    pub media: MediaCache,
    pub focus_handle: FocusHandle,

    pub nav_history: Vec<NavTarget>,
    pub nav_cursor: usize,

    pub library_tab: LibraryTab,
    pub search_tab: SearchTab,
    pub search_category: SearchCategory,
    pub settings_tab: SettingsTab,
    pub fullscreen_open: bool,
    pub fullscreen_tab: FullscreenDrawerTab,
    pub fullscreen_last_activity: Instant,
    pub fullscreen_art_fade: f32,

    pub search_input: Entity<InputState>,
    pub library_filter_input: Entity<InputState>,
    pub playlist_name_input: Entity<InputState>,
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
            if window.is_window_active() {
                this.reload_themes_and_sync(window, cx);
            }
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

        Self {
            backend,
            snapshot,
            decorated,
            settings,
            theme_catalog,
            palette,
            media: MediaCache::default(),
            focus_handle,
            nav_history: vec![NavTarget::Home],
            nav_cursor: 0,
            library_tab: LibraryTab::Playlists,
            search_tab: SearchTab::YouTube,
            search_category: SearchCategory::All,
            settings_tab: SettingsTab::Appearance,
            fullscreen_open: false,
            fullscreen_tab: FullscreenDrawerTab::Lyrics,
            fullscreen_last_activity: Instant::now(),
            fullscreen_art_fade: 1.0,
            search_input,
            library_filter_input,
            playlist_name_input,
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

    fn on_snapshot(
        &mut self,
        next: Arc<DesktopSnapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nav_changed = self.snapshot.active_nav != next.active_nav;
        let track_changed = self.snapshot.current_track().map(|t| t.id.as_str())
            != next.current_track().map(|t| t.id.as_str());

        self.track_loading_flag("home", next.home_loading);
        self.track_loading_flag("explore", next.explore_loading);
        self.track_loading_flag("artist", next.artist_loading);
        self.track_loading_flag("album", next.album_loading);
        self.track_loading_flag("playlist", next.playlist_loading);
        self.track_loading_flag("search", next.search_loading);
        self.track_loading_flag("radio", next.radio_loading);
        self.track_loading_flag("history", next.history_loading);

        self.snapshot = next;
        self.media.clear_missing();

        if nav_changed {
            self.push_history_if_needed(self.snapshot.active_nav.clone());
            self.main_scroll
                .set_offset(gpui_kit::point(px(0.0), px(0.0)));
        }

        if track_changed {
            self.last_active_lyric = None;
            self.lyric_prev_idx = None;
            self.fullscreen_art_fade = 1.0;
        }

        if !self.progress_dragging {
            let max_dur = self.snapshot.duration_secs.max(1.0) as f32;
            let cur_pos = (self.snapshot.position_secs as f32).clamp(0.0, max_dur);
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
        let mut all = self.snapshot.ytm_playlists.clone();
        if self.settings.sidebar_playlist_order.is_empty() {
            return all;
        }
        let order_map: HashMap<&str, usize> = self
            .settings
            .sidebar_playlist_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        all.sort_by_key(|p| {
            if p.id == "VLLM" {
                (0, 0)
            } else if let Some(&idx) = order_map.get(p.id.as_str()) {
                (1, idx)
            } else {
                (2, usize::MAX)
            }
        });
        all
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
        self.fullscreen_open
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
        if !self.fullscreen_open || self.snapshot.config.reduce_motion {
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

    fn sync_lyric_transition(&mut self, cx: &mut Context<Self>) {
        let next_idx = self.snapshot.active_lyric_index();
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
        active_bg: Option<usize>,
    ) -> f32 {
        let raw = (self.lyric_changed_at.elapsed().as_secs_f32() / 0.32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - raw).powi(3);
        if active_lead == Some(idx) {
            eased
        } else if active_bg == Some(idx) {
            0.85
        } else if self.lyric_prev_idx == Some(idx) && raw < 1.0 {
            1.0 - eased
        } else {
            0.0
        }
    }

    fn maybe_auto_scroll_lyrics(&mut self) {
        let paused_by_wheel = self
            .last_wheel_lyrics
            .is_some_and(|t| t.elapsed() < Duration::from_secs(3));
        if paused_by_wheel {
            return;
        }
        let Some(active_idx) = self.snapshot.active_lyric_index() else {
            return;
        };
        if self.last_active_lyric == Some(active_idx) {
            return;
        }
        self.last_active_lyric = Some(active_idx);
        let len = match &self.snapshot.lyrics {
            LyricsState::Synced(lines) => lines.len(),
            _ => 0,
        };
        if len > 0 {
            self.lyrics_scroll
                .scroll_to_item(active_idx.saturating_sub(2));
            self.fullscreen_lyrics_scroll
                .scroll_to_item(active_idx.saturating_sub(2));
        }
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
            self.fullscreen_last_activity = Instant::now();
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
                self.fullscreen_last_activity = Instant::now();
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
}

impl Focusable for SonataDesktopView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SonataDesktopView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_motion_cover_textures(cx);
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
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
                    this.fullscreen_last_activity = Instant::now();
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
