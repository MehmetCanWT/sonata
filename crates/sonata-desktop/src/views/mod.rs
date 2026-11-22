//! Root GPUI view of the desktop client and the shared widgets used by the other view modules.
//!
//! 1. **View State**: `SonataDesktopView` holds the latest `DesktopSnapshot` plus UI-only state (`CenterMode`, `DesktopModal`, inputs, scroll handles, mini mode).
//! 2. **Snapshot Sync**: a spawned task awaits the backend `watch` channel, swaps in each snapshot, keeps lyrics scrolled to the active line, and notifies GPUI.
//! 3. **Rendering**: the `Render` impl picks the mini player or the full workspace and layers the account popover and modal overlay on top.
//! 4. **Shared Widgets**: icons, cover art, avatars, buttons, track rows, and progress and volume sliders.
//! 5. **Submodules**: `home`, `search`, `artist`, `settings`, `modals`, `mini_player`, `fullscreen`, `fullscreen_fx`, and `root`.

pub mod artist;
pub mod fullscreen;
pub mod fullscreen_fx;
pub mod home;
pub mod mini_player;
pub mod modals;
pub mod root;
pub mod search;
pub mod settings;

use crate::backend::{BackendCmd, BackendHandle, DesktopSnapshot, NavTarget};
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    div, img, px, relative, svg, AnyElement, App, AppContext, Bounds, Context, Entity, FocusHandle,
    Focusable, FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Pixels, Render, RenderImage,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription,
    Svg, UniformListScrollHandle, Window,
};
use sonata_core::media::{art_cache_path, format_mm_ss};
use sonata_core::providers::{format_duration, Track};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterMode {
    Page,
    Lyrics,
    Fullscreen,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DesktopModal {
    None,
    SignInBrowser,
    CreatePlaylist,
    AddToPlaylist(Track),
}

pub struct SonataDesktopView {
    pub backend: BackendHandle,
    pub snapshot: Arc<DesktopSnapshot>,
    pub focus_handle: FocusHandle,
    pub search_input: Entity<InputState>,
    pub modal_input: Entity<InputState>,
    pub track_scroll: UniformListScrollHandle,
    pub search_scroll: UniformListScrollHandle,
    pub lyrics_scroll: UniformListScrollHandle,
    pub queue_scroll: UniformListScrollHandle,
    pub last_synced_lyric_idx: Option<usize>,
    pub center_mode: CenterMode,
    pub show_queue: bool,
    pub account_menu_open: bool,
    pub mini_mode: bool,
    pub mini_show_lyric: bool,
    pub modal: DesktopModal,
    pub hovered_track: Option<usize>,
    pub progress_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub volume_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub dragging_progress: bool,
    pub dragging_volume: bool,
    pub fullscreen_last_activity: Instant,
    pub anim_epoch: Instant,
    pub last_anim_track_id: Option<String>,
    pub motion_textures: Vec<Arc<RenderImage>>,
    pub motion_fps: f32,
    pub motion_source_ptr: Option<usize>,
    pub last_motion_frame_idx: usize,
    pub motion_timer_active: bool,
    pub last_transparent: bool,
    pub fullscreen: fullscreen::FullscreenState,
    _subscriptions: Vec<Subscription>,
}

pub fn is_compact_viewport(width: f32, height: f32) -> bool {
    width < 760.0 || height < 460.0
}

impl SonataDesktopView {
    pub fn new(backend: BackendHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let snapshot = backend.snapshot();
        let focus_handle = cx.focus_handle();
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search songs on YouTube Music... (Ctrl+F)")
        });
        let modal_input = cx.new(|cx| InputState::new(window, cx));

        let mut subscriptions = Vec::new();
        subscriptions.push(
            cx.subscribe(&search_input, |this, state, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let query = state.read(cx).value().to_string();
                    if !query.trim().is_empty() {
                        this.center_mode = CenterMode::Page;
                        this.account_menu_open = false;
                        this.backend.send(BackendCmd::Search(query));
                        cx.notify();
                    }
                }
            }),
        );

        subscriptions.push(cx.subscribe_in(
            &modal_input,
            window,
            |this, state, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let val = state.read(cx).value().to_string();
                    this.submit_text_modal(val, window, cx);
                }
            },
        ));

        let mut snap_rx = backend.subscribe();
        cx.spawn(async move |this, cx| {
            while snap_rx.changed().await.is_ok() {
                let next_snap = snap_rx.borrow_and_update().clone();
                let updated = this.update(cx, |view, cx| {
                    let just_signed_in = !view.snapshot.is_signed_in() && next_snap.is_signed_in();
                    let next_track_id = next_snap.current_track().map(|t| t.id.clone());
                    if next_track_id != view.last_anim_track_id
                        || (view.snapshot.motion_cover.is_none()
                            && next_snap.motion_cover.is_some())
                    {
                        view.last_anim_track_id = next_track_id;
                        view.anim_epoch = Instant::now();
                    }
                    view.snapshot = next_snap;
                    if just_signed_in && matches!(view.modal, DesktopModal::SignInBrowser) {
                        view.modal = DesktopModal::None;
                    }
                    if view.center_mode == CenterMode::Lyrics {
                        let active = view.snapshot.active_lyric_index();
                        if active != view.last_synced_lyric_idx {
                            view.last_synced_lyric_idx = active;
                            if let Some(idx) = active {
                                view.lyrics_scroll
                                    .scroll_to_item(idx, ScrollStrategy::Center);
                            }
                        }
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();

        let initial_transparent = snapshot.effective_transparent();
        Self {
            backend,
            snapshot,
            focus_handle,
            search_input,
            modal_input,
            track_scroll: UniformListScrollHandle::new(),
            search_scroll: UniformListScrollHandle::new(),
            lyrics_scroll: UniformListScrollHandle::new(),
            queue_scroll: UniformListScrollHandle::new(),
            last_synced_lyric_idx: None,
            center_mode: CenterMode::Page,
            show_queue: false,
            account_menu_open: false,
            mini_mode: false,
            mini_show_lyric: true,
            modal: DesktopModal::None,
            hovered_track: None,
            progress_bounds: Rc::new(Cell::new(None)),
            volume_bounds: Rc::new(Cell::new(None)),
            dragging_progress: false,
            dragging_volume: false,
            fullscreen_last_activity: Instant::now(),
            anim_epoch: Instant::now(),
            last_anim_track_id: None,
            motion_textures: Vec::new(),
            motion_fps: 24.0,
            motion_source_ptr: None,
            last_motion_frame_idx: 0,
            motion_timer_active: false,
            last_transparent: initial_transparent,
            fullscreen: fullscreen::FullscreenState::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn palette(&self) -> GlassPalette {
        let transparent = self.snapshot.effective_transparent();
        GlassPalette::from_theme(
            &self.snapshot.base_theme,
            transparent,
            self.snapshot.ambient_rgb,
        )
        .with_glass_opacity(transparent, self.snapshot.effective_hyprland_opacity())
    }

    pub fn navigate_to(&mut self, target: NavTarget, cx: &mut Context<Self>) {
        self.center_mode = CenterMode::Page;
        self.account_menu_open = false;
        self.hovered_track = None;
        self.backend.send(BackendCmd::Navigate(target));
        cx.notify();
    }

    pub fn open_sign_in_modal(&mut self, cx: &mut Context<Self>) {
        self.account_menu_open = false;
        self.backend.send(BackendCmd::ClearSignInError);
        self.modal = DesktopModal::SignInBrowser;
        cx.notify();
    }

    pub fn toggle_lyrics_view(&mut self, cx: &mut Context<Self>) {
        self.account_menu_open = false;
        self.center_mode = match self.center_mode {
            CenterMode::Page => {
                if let Some(idx) = self.snapshot.active_lyric_index() {
                    self.last_synced_lyric_idx = Some(idx);
                    self.lyrics_scroll
                        .scroll_to_item(idx, ScrollStrategy::Center);
                }
                CenterMode::Lyrics
            }
            CenterMode::Lyrics | CenterMode::Fullscreen => CenterMode::Page,
        };
        cx.notify();
    }

    pub fn has_animated_cover(&self) -> bool {
        !self.motion_textures.is_empty() && !self.snapshot.config.reduce_motion
    }

    pub fn active_motion_texture(&self) -> Option<Arc<RenderImage>> {
        if self.motion_textures.is_empty() || self.snapshot.config.reduce_motion {
            return None;
        }
        let fps = self.motion_fps.max(1.0);
        let frame_idx =
            ((self.anim_epoch.elapsed().as_secs_f32() * fps) as usize) % self.motion_textures.len();
        self.motion_textures.get(frame_idx).cloned()
    }

    fn sync_motion_textures(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next_ptr = self
            .snapshot
            .motion_cover
            .as_ref()
            .map(|arc| Arc::as_ptr(arc) as usize);
        if next_ptr != self.motion_source_ptr {
            for old in self.motion_textures.drain(..) {
                let _ = window.drop_image(old);
            }
            self.motion_source_ptr = next_ptr;
            self.last_motion_frame_idx = 0;
            self.anim_epoch = Instant::now();
            if let Some(cover) = &self.snapshot.motion_cover {
                self.motion_fps = cover.fps.clamp(1.0, 60.0);
                self.motion_textures = cover
                    .frames_rgba
                    .iter()
                    .filter_map(|frame| rgba_to_render_image(cover.width, cover.height, frame))
                    .collect();
            } else {
                self.motion_fps = 24.0;
            }
        }

        if self.has_animated_cover() && !self.motion_timer_active {
            self.motion_timer_active = true;
            let mut tick = Duration::from_secs_f32(1.0 / self.motion_fps.clamp(12.0, 30.0));
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(tick).await;
                let keep_running = this.update(cx, |view, cx| {
                    if !view.has_animated_cover() {
                        view.motion_timer_active = false;
                        return false;
                    }
                    let fps = view.motion_fps.clamp(12.0, 30.0);
                    tick = Duration::from_secs_f32(1.0 / fps);
                    if view.snapshot.paused {
                        return true;
                    }
                    let next_idx = ((view.anim_epoch.elapsed().as_secs_f32() * fps) as usize)
                        % view.motion_textures.len();
                    if next_idx != view.last_motion_frame_idx {
                        view.last_motion_frame_idx = next_idx;
                        cx.notify();
                    }
                    true
                });
                if !matches!(keep_running, Ok(true)) {
                    break;
                }
            })
            .detach();
        }
    }

    pub fn toggle_fullscreen_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.account_menu_open = false;
        self.show_queue = false;
        let entering = self.center_mode != CenterMode::Fullscreen;
        if !entering {
            self.center_mode = CenterMode::Page;
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
        } else {
            self.mini_mode = false;
            self.fullscreen_last_activity = Instant::now();
            self.fullscreen.on_enter();
            self.center_mode = CenterMode::Fullscreen;
            let vp = window.viewport_size();
            if is_compact_viewport(vp.width.into(), vp.height.into()) {
                window.resize(gpui_kit::size(px(1240.0), px(800.0)));
            }
        }
        cx.notify();
    }

    pub fn exit_mini_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mini_mode = false;
        self.modal = DesktopModal::None;
        self.account_menu_open = false;
        let vp = window.viewport_size();
        if is_compact_viewport(vp.width.into(), vp.height.into()) {
            window.resize(gpui_kit::size(px(1240.0), px(800.0)));
        }
        cx.notify();
    }

    pub fn note_fullscreen_activity(&mut self, cx: &mut Context<Self>) {
        if self.center_mode == CenterMode::Fullscreen {
            self.fullscreen_last_activity = Instant::now();
            cx.notify();
        }
    }

    pub fn fullscreen_controls_visible(&self) -> bool {
        self.snapshot.config.reduce_motion
            || self.snapshot.paused
            || self.fullscreen_last_activity.elapsed() < Duration::from_secs(2)
    }

    pub fn toggle_mini_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mini_mode = !self.mini_mode;
        self.modal = DesktopModal::None;
        self.account_menu_open = false;
        let target_size = if self.mini_mode {
            let h = if self.mini_show_lyric { 202.0 } else { 166.0 };
            gpui_kit::size(px(430.0), px(h))
        } else {
            gpui_kit::size(px(1240.0), px(800.0))
        };
        window.resize(target_size);
        cx.notify();
    }

    pub fn open_create_playlist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.account_menu_open = false;
        self.modal_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.modal = DesktopModal::CreatePlaylist;
        cx.notify();
    }

    pub fn submit_text_modal(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let trimmed = value.trim().to_string();
        if matches!(self.modal, DesktopModal::CreatePlaylist) {
            if !trimmed.is_empty() {
                self.backend.send(BackendCmd::CreateYtmPlaylist(trimmed));
            }
            self.modal = DesktopModal::None;
            window.focus(&self.focus_handle, cx);
            cx.notify();
        }
    }

    fn text_input_focused(&self, window: &Window, cx: &App) -> bool {
        self.search_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
            || self
                .modal_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
    }

    pub fn handle_key_down(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ctrl = ev.keystroke.modifiers.control;
        let key = ev.keystroke.key.as_str();

        self.note_fullscreen_activity(cx);

        if key == "escape" {
            if !matches!(self.modal, DesktopModal::None) {
                self.modal = DesktopModal::None;
                window.focus(&self.focus_handle, cx);
                cx.notify();
                return;
            }
            if self.account_menu_open {
                self.account_menu_open = false;
                cx.notify();
                return;
            }
            if self.show_queue {
                self.show_queue = false;
                cx.notify();
                return;
            }
            if self.center_mode == CenterMode::Fullscreen {
                self.center_mode = CenterMode::Page;
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
                cx.notify();
                return;
            }
            if self.mini_mode {
                self.toggle_mini_mode(window, cx);
                return;
            }
            if self.text_input_focused(window, cx) {
                window.focus(&self.focus_handle, cx);
                cx.notify();
                return;
            }
        }

        if ctrl {
            match key {
                "f" => {
                    if self.mini_mode {
                        self.toggle_mini_mode(window, cx);
                    }
                    self.modal = DesktopModal::None;
                    self.account_menu_open = false;
                    self.center_mode = CenterMode::Page;
                    self.backend.send(BackendCmd::Navigate(NavTarget::Search));
                    self.search_input.update(cx, |s, cx| s.focus(window, cx));
                    cx.notify();
                    return;
                }
                "m" => {
                    self.toggle_mini_mode(window, cx);
                    return;
                }
                "l" => {
                    if !self.mini_mode {
                        self.toggle_lyrics_view(cx);
                    }
                    return;
                }
                "," => {
                    if self.mini_mode {
                        self.toggle_mini_mode(window, cx);
                    }
                    self.navigate_to(NavTarget::Settings, cx);
                    return;
                }
                "q" => {
                    self.backend
                        .send_window(crate::backend::UiWindowCmd::QuitApp);
                    return;
                }
                _ => {}
            }
        }

        if self.text_input_focused(window, cx) || !matches!(self.modal, DesktopModal::None) {
            return;
        }

        match key {
            "f" | "f11" => self.toggle_fullscreen_view(window, cx),
            "space" => self.backend.send(BackendCmd::TogglePause),
            "right" => self.backend.send(BackendCmd::SeekRelative(5.0)),
            "left" => self.backend.send(BackendCmd::SeekRelative(-5.0)),
            _ => {}
        }
    }

    pub fn ratio_in_bounds(bounds: Option<Bounds<Pixels>>, x: Pixels) -> Option<f32> {
        let b = bounds?;
        let width: f32 = b.size.width.into();
        if width <= 1.0 {
            return None;
        }
        let origin_x: f32 = b.origin.x.into();
        let pos_x: f32 = x.into();
        Some(((pos_x - origin_x) / width).clamp(0.0, 1.0))
    }

    pub fn seek_at_x(&self, x: Pixels) {
        if let Some(ratio) = Self::ratio_in_bounds(self.progress_bounds.get(), x) {
            let dur = self.snapshot.duration_secs;
            if dur > 0.0 {
                self.backend
                    .send(BackendCmd::SeekAbsolute(dur * ratio as f64));
            }
        }
    }

    pub fn volume_at_x(&self, x: Pixels) {
        if let Some(ratio) = Self::ratio_in_bounds(self.volume_bounds.get(), x) {
            let vol = (ratio * 100.0).round().clamp(0.0, 100.0) as u8;
            self.backend.send(BackendCmd::SetVolume(vol));
        }
    }
}

impl Focusable for SonataDesktopView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SonataDesktopView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transparent = self.snapshot.effective_transparent();
        if transparent != self.last_transparent {
            self.last_transparent = transparent;
            window.set_background_appearance(if transparent {
                gpui_kit::WindowBackgroundAppearance::Blurred
            } else {
                gpui_kit::WindowBackgroundAppearance::Opaque
            });
        }
        self.sync_motion_textures(window, cx);
        let palette = self.palette();
        let viewport = window.viewport_size();
        let vw: f32 = viewport.width.into();
        let vh: f32 = viewport.height.into();
        let compact_viewport = self.mini_mode || is_compact_viewport(vw, vh);

        let root_content = if compact_viewport {
            mini_player::render_mini_player(self, &palette, window, cx)
        } else {
            root::render_full_workspace(self, &palette, window, cx)
        };

        div()
            .id("sonata-desktop-root")
            .size_full()
            .top_0()
            .left_0()
            .font_family("Inter")
            .track_focus(&self.focus_handle)
            .text_color(palette.text)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.account_menu_open {
                        this.account_menu_open = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.dragging_progress = false;
                    this.dragging_volume = false;
                }),
            )
            .child(root_content)
            .when(self.account_menu_open && !compact_viewport, |el| {
                el.child(root::render_account_popover(self, &palette, cx))
            })
            .when(!matches!(self.modal, DesktopModal::None), |el| {
                el.child(modals::render_modal_overlay(self, &palette, window, cx))
            })
    }
}

pub trait FluentDivExt: Sized {
    fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }
}

impl<T> FluentDivExt for T {}

pub fn ui_icon(icon: IconName, size_px: f32, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .flex_shrink_0()
        .size(px(size_px))
        .text_color(color)
}

pub fn cached_thumb_path(url: Option<&str>) -> Option<PathBuf> {
    url.map(art_cache_path).filter(|p| p.exists())
}

fn rgba_to_render_image(width: u32, height: u32, rgba: &[u8]) -> Option<Arc<RenderImage>> {
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }
    let mut bgra = vec![0u8; expected];
    for (dst, src) in bgra
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(rgba.as_chunks::<4>().0.iter())
    {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = src[3];
    }
    let img_buf = image::RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(img_buf)])))
}

pub fn render_cover_surface(
    art_path: Option<&PathBuf>,
    motion_tex: Option<Arc<RenderImage>>,
    size_px: f32,
    radius_px: f32,
    p: &GlassPalette,
) -> AnyElement {
    if let Some(tex) = motion_tex {
        return div()
            .flex_shrink_0()
            .size(px(size_px))
            .rounded(px(radius_px))
            .overflow_hidden()
            .border_1()
            .border_color(p.border_subtle)
            .child(
                img(tex)
                    .size(px(size_px))
                    .rounded(px(radius_px))
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element();
    }
    render_cover_art(art_path, size_px, radius_px, p)
}

pub fn render_cover_art(
    art_path: Option<&PathBuf>,
    size_px: f32,
    radius_px: f32,
    p: &GlassPalette,
) -> AnyElement {
    if let Some(path) = art_path.filter(|path| path.exists()) {
        return div()
            .flex_shrink_0()
            .size(px(size_px))
            .rounded(px(radius_px))
            .overflow_hidden()
            .border_1()
            .border_color(p.border_subtle)
            .child(
                img(path.clone())
                    .size(px(size_px))
                    .rounded(px(radius_px))
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element();
    }

    div()
        .flex_shrink_0()
        .size(px(size_px))
        .rounded(px(radius_px))
        .bg(p.card_bg)
        .border_1()
        .border_color(p.border_subtle)
        .flex()
        .items_center()
        .justify_center()
        .child(ui_icon(
            IconName::Disc3,
            (size_px * 0.42).max(15.0),
            p.accent,
        ))
        .into_any_element()
}

pub fn render_avatar(
    avatar_path: Option<&PathBuf>,
    name: &str,
    size_px: f32,
    p: &GlassPalette,
) -> AnyElement {
    if let Some(path) = avatar_path.filter(|path| path.exists()) {
        return div()
            .flex_shrink_0()
            .size(px(size_px))
            .rounded_full()
            .overflow_hidden()
            .border_1()
            .border_color(p.border_subtle)
            .child(
                img(path.clone())
                    .size(px(size_px))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element();
    }

    let initial = name
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "Y".to_string());
    div()
        .flex_shrink_0()
        .size(px(size_px))
        .rounded_full()
        .bg(p.accent_soft)
        .border_1()
        .border_color(p.accent)
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(p.accent)
        .child(initial)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub fn render_icon_button(
    id: impl Into<SharedString>,
    icon: IconName,
    icon_size: f32,
    box_size: f32,
    fg: Hsla,
    bg: Hsla,
    hover_bg: Hsla,
    on_click: impl Fn(&mut SonataDesktopView, &mut Window, &mut Context<SonataDesktopView>) + 'static,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .id(id.into())
        .flex_shrink_0()
        .size(px(box_size))
        .rounded_full()
        .bg(bg)
        .hover(move |s| s.bg(hover_bg))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, window, cx| {
            on_click(this, window, cx);
        }))
        .child(ui_icon(icon, icon_size, fg))
        .into_any_element()
}

pub fn render_pill_button(
    id: impl Into<SharedString>,
    label: String,
    active: bool,
    p: &GlassPalette,
    on_click: impl Fn(&mut SonataDesktopView, &mut Window, &mut Context<SonataDesktopView>) + 'static,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .id(id.into())
        .px_3()
        .py(px(6.0))
        .rounded_lg()
        .bg(if active { p.accent_soft } else { p.pill_bg })
        .border_1()
        .border_color(if active { p.accent } else { p.border_subtle })
        .hover(|s| s.bg(p.card_hover))
        .cursor_pointer()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if active { p.accent } else { p.text })
        .on_click(cx.listener(move |this, _, window, cx| {
            on_click(this, window, cx);
        }))
        .child(label)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub fn render_track_row(
    row_id: String,
    idx: usize,
    track: Track,
    list_source: Arc<Vec<Track>>,
    is_playing: bool,
    is_hovered: bool,
    editable: bool,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let art = cached_thumb_path(track.thumbnail_url.as_deref());
    let dur = format_duration(track.duration_secs);
    let track_like = track.clone();
    let track_add = track.clone();
    let play_source = list_source;

    div()
        .id(SharedString::from(row_id.clone()))
        .w_full()
        .h(px(54.0))
        .px_3()
        .rounded_lg()
        .bg(if is_playing {
            p.accent_soft
        } else if is_hovered {
            p.card_hover
        } else {
            gpui_kit::transparent_black()
        })
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_3()
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.hovered_track = Some(idx);
            } else if this.hovered_track == Some(idx) {
                this.hovered_track = None;
            }
            cx.notify();
        }))
        .on_click(cx.listener(move |this, _, _, _| {
            this.backend.send(BackendCmd::PlayTracks {
                tracks: (*play_source).clone(),
                start_index: idx,
                shuffle: false,
            });
        }))
        .child(
            div()
                .w(px(24.0))
                .flex()
                .justify_center()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_playing { p.accent } else { p.muted })
                .child(if is_playing {
                    "▶".to_string()
                } else {
                    format!("{}", idx + 1)
                }),
        )
        .child(render_cover_art(art.as_ref(), 40.0, 6.0, p))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(1.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(if is_playing {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if is_playing { p.accent } else { p.highlight_fg })
                        .truncate()
                        .child(track.title.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .truncate()
                        .child(track.artist.clone()),
                ),
        )
        .child(
            div()
                .w(px(190.0))
                .text_xs()
                .text_color(p.muted)
                .truncate()
                .child(track.album.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(render_icon_button(
                    SharedString::from(format!("{row_id}-like")),
                    IconName::Heart,
                    14.0,
                    26.0,
                    if track.liked { p.error } else { p.muted },
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    move |this, _, _| {
                        this.backend
                            .send(BackendCmd::ToggleLike(track_like.clone()));
                    },
                    cx,
                ))
                .child(render_icon_button(
                    SharedString::from(format!("{row_id}-add")),
                    IconName::ListPlus,
                    14.0,
                    26.0,
                    p.muted,
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    move |this, _, cx| {
                        this.modal = DesktopModal::AddToPlaylist(track_add.clone());
                        cx.notify();
                    },
                    cx,
                ))
                .when(editable && is_hovered, |el| {
                    el.child(render_icon_button(
                        SharedString::from(format!("{row_id}-rm")),
                        IconName::X,
                        13.0,
                        24.0,
                        p.error,
                        gpui_kit::transparent_black(),
                        p.pill_bg,
                        move |this, _, _| {
                            this.backend.send(BackendCmd::RemoveTrackFromPlaylist(idx));
                        },
                        cx,
                    ))
                }),
        )
        .child(
            div()
                .w(px(46.0))
                .text_xs()
                .text_color(p.muted)
                .flex()
                .justify_end()
                .child(dur),
        )
        .into_any_element()
}

pub fn render_progress_slider(
    view: &SonataDesktopView,
    p: &GlassPalette,
    id: &'static str,
    compact: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let ratio = if view.snapshot.duration_secs > 0.0 {
        (view.snapshot.position_secs / view.snapshot.duration_secs).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let cur_str = format_mm_ss(view.snapshot.position_secs.max(0.0).floor() as u64);
    let dur_str = format_mm_ss(view.snapshot.duration_secs.max(0.0).floor() as u64);
    let bounds_cell = view.progress_bounds.clone();

    div()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(36.0))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.muted)
                .child(cur_str),
        )
        .child(
            div()
                .flex_1()
                .h(px(if compact { 14.0 } else { 18.0 }))
                .on_children_prepainted(move |children, _, _| {
                    if let Some(first) = children.first() {
                        bounds_cell.set(Some(*first));
                    }
                })
                .child(
                    div()
                        .id(id)
                        .size_full()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _, _| {
                                this.dragging_progress = true;
                                this.seek_at_x(ev.position.x);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, _| {
                            if this.dragging_progress
                                && ev.pressed_button == Some(MouseButton::Left)
                            {
                                this.seek_at_x(ev.position.x);
                            }
                        }))
                        .child(
                            div()
                                .w_full()
                                .h(px(if compact { 4.0 } else { 5.0 }))
                                .rounded_full()
                                .bg(p.pill_bg)
                                .overflow_hidden()
                                .relative()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(ratio))
                                        .rounded_full()
                                        .bg(p.accent),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .w(px(36.0))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.muted)
                .flex()
                .justify_end()
                .child(dur_str),
        )
        .into_any_element()
}

pub fn render_volume_slider(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let eff_vol = if view.snapshot.muted {
        0
    } else {
        view.snapshot.volume
    };
    let ratio = (eff_vol as f32 / 100.0).clamp(0.0, 1.0);
    let vol_icon = if view.snapshot.muted || eff_vol == 0 {
        IconName::VolumeX
    } else if eff_vol < 45 {
        IconName::Volume1
    } else {
        IconName::Volume2
    };
    let bounds_cell = view.volume_bounds.clone();

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(render_icon_button(
            "btn-mute",
            vol_icon,
            16.0,
            28.0,
            if view.snapshot.muted {
                p.warning
            } else {
                p.text
            },
            gpui_kit::transparent_black(),
            p.pill_bg,
            |this, _, _| this.backend.send(BackendCmd::ToggleMute),
            cx,
        ))
        .child(
            div()
                .w(px(88.0))
                .h(px(18.0))
                .on_children_prepainted(move |children, _, _| {
                    if let Some(first) = children.first() {
                        bounds_cell.set(Some(*first));
                    }
                })
                .child(
                    div()
                        .id("volume-slider-track")
                        .size_full()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _, _| {
                                this.dragging_volume = true;
                                this.volume_at_x(ev.position.x);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, _| {
                            if this.dragging_volume && ev.pressed_button == Some(MouseButton::Left)
                            {
                                this.volume_at_x(ev.position.x);
                            }
                        }))
                        .child(
                            div()
                                .w_full()
                                .h(px(4.0))
                                .rounded_full()
                                .bg(p.pill_bg)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(ratio))
                                        .rounded_full()
                                        .bg(p.accent),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .w(px(34.0))
                .text_xs()
                .text_color(p.muted)
                .child(format!("{eff_vol}%")),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_render_image_swaps_to_bgra_and_validates_len() {
        assert!(rgba_to_render_image(2, 2, &[1, 2, 3]).is_none());
        let tex = rgba_to_render_image(1, 1, &[10, 20, 30, 255]).expect("valid 1x1 texture");
        let bytes = tex.as_bytes(0).expect("bgra bytes");
        assert_eq!(bytes, &[30, 20, 10, 255]);
    }
}
