use super::{
    render_cover_art, render_icon_button, render_progress_slider, ui_icon, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window,
};

pub fn render_mini_player(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let track = view.snapshot.current_track().cloned();
    let title = track
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "No Track Playing".to_string());
    let artist = track
        .as_ref()
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| "Select a song in Library".to_string());
    let play_icon = if view.snapshot.paused {
        IconName::Play
    } else {
        IconName::Pause
    };
    let active_lyric = if view.mini_show_lyric {
        view.snapshot.active_lyric_line().map(ToString::to_string)
    } else {
        None
    };

    let _ = window;

    div()
        .size_full()
        .bg(p.window_bg)
        .child(
            div()
                .size_full()
                .p_3()
                .bg(linear_gradient(
                    135.0,
                    linear_color_stop(p.ambient_top, 0.0),
                    linear_color_stop(p.window_bg, 0.85),
                ))
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .flex_col()
                .justify_between()
                .gap_2()
                .child(
                    // Draggable header strip + Mini controls
                    div()
                        .id("mini-drag-bar")
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor_grab()
                        .on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.start_window_move();
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(ui_icon(IconName::Disc3, 14.0, p.accent))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.muted)
                                        .child("Sonata Mini"),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(1.0))
                                        .rounded_full()
                                        .bg(p.pill_bg)
                                        .text_xs()
                                        .text_color(p.accent)
                                        .child(view.snapshot.stream_badge.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(render_icon_button(
                                    "mini-toggle-lyric",
                                    IconName::MicVocal,
                                    14.0,
                                    24.0,
                                    if view.mini_show_lyric {
                                        p.accent
                                    } else {
                                        p.muted
                                    },
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, window, cx| {
                                        this.mini_show_lyric = !this.mini_show_lyric;
                                        let h = if this.mini_show_lyric { 202.0 } else { 166.0 };
                                        window.resize(gpui_kit::size(px(430.0), px(h)));
                                        cx.notify();
                                    },
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "mini-expand",
                                    IconName::Maximize2,
                                    14.0,
                                    24.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, window, cx| this.toggle_mini_mode(window, cx),
                                    cx,
                                )),
                        ),
                )
                .child(
                    // Track cover, metadata, and transport buttons
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_cover_art(
                            view.snapshot.art_path.as_ref(),
                            60.0,
                            10.0,
                            p,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.highlight_fg)
                                        .truncate()
                                        .child(title),
                                )
                                .child(
                                    div().text_xs().text_color(p.muted).truncate().child(artist),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(render_icon_button(
                                    "mini-prev",
                                    IconName::SkipBack,
                                    15.0,
                                    28.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "mini-play",
                                    play_icon,
                                    16.0,
                                    34.0,
                                    p.accent_fg,
                                    p.accent,
                                    p.accent,
                                    |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "mini-next",
                                    IconName::SkipForward,
                                    15.0,
                                    28.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::NextTrack),
                                    cx,
                                )),
                        ),
                )
                .child(render_progress_slider(view, p, "mini-progress", true, cx))
                .child(div().when(view.mini_show_lyric, |el| {
                    el.w_full()
                        .px_2()
                        .py_1()
                        .rounded_lg()
                        .bg(p.card_bg)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if active_lyric.is_some() {
                            p.lyrics_active
                        } else {
                            p.muted
                        })
                        .truncate()
                        .child(
                            active_lyric
                                .unwrap_or_else(|| "♪ Instrumental / No synced lyric".to_string()),
                        )
                })),
        )
        .into_any_element()
}

trait MiniFluentExt: Sized {
    fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }
}

impl<T> MiniFluentExt for T {}
