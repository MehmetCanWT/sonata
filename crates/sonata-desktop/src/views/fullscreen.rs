use super::{
    render_cover_art, render_icon_button, render_progress_slider, render_volume_slider,
    FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::{rgb_hsla, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, px, uniform_list, AnyElement, Context,
    FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledImage,
};
use sonata_core::lyrics::LyricsState;
use std::ops::Range;

/// Render the desktop-only immersive playback workspace.
///
/// Flex wrapping keeps artwork, lyrics, and controls usable in narrow windows without imposing an
/// Android layout contract.
pub fn render_fullscreen_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let (r, g, b) = view.snapshot.ambient_rgb;
    let backdrop = view
        .snapshot
        .art_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| {
            img(path.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .opacity(0.12)
                .into_any_element()
        });

    let ambient_layer = div().absolute().size_full().bg(linear_gradient(
        140.0,
        linear_color_stop(rgb_hsla((r, g, b), 0.54), 0.0),
        linear_color_stop(p.window_bg, 1.0),
    ));
    let ambient_layer = if let Some(art) = backdrop {
        ambient_layer.child(art)
    } else {
        ambient_layer
    };

    div()
        .relative()
        .size_full()
        .on_mouse_move(cx.listener(|this, _, _, cx| this.note_fullscreen_activity(cx)))
        .bg(p.window_bg)
        .child(ambient_layer)
        .child(render_fullscreen(view, p, cx))
        .into_any_element()
}

pub fn render_fullscreen(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let track = snap.current_track();
    let title = track
        .map(|track| track.title.clone())
        .unwrap_or_else(|| "No Active Track".to_string());
    let artist = track
        .map(|track| track.artist.clone())
        .unwrap_or_else(|| "Choose a song to begin".to_string());
    let play_icon = if snap.paused {
        IconName::Play
    } else {
        IconName::Pause
    };
    let controls_visible = view.fullscreen_controls_visible();

    div()
        .id("fullscreen-playback")
        .size_full()
        .p_6()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .w_full()
                .flex()
                .justify_end()
                .when(controls_visible, |el| {
                    el.child(render_icon_button(
                        "fullscreen-close",
                        IconName::Minimize2,
                        17.0,
                        36.0,
                        p.text,
                        p.pill_bg,
                        p.card_hover,
                        |this, _, cx| this.toggle_fullscreen_view(cx),
                        cx,
                    ))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap_8()
                .child(
                    div()
                        .w(px(360.0))
                        .max_w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_4()
                        .child(render_cover_art(snap.art_path.as_ref(), 320.0, 24.0, p))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.highlight_fg)
                                        .truncate()
                                        .child(title),
                                )
                                .child(div().text_sm().text_color(p.muted).truncate().child(artist))
                                .child(
                                    div()
                                        .px_3()
                                        .py(px(4.0))
                                        .rounded_full()
                                        .bg(p.pill_bg)
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.success)
                                        .child(snap.stream_badge.clone()),
                                ),
                        ),
                )
                .child(render_lyrics_panel(view, p, cx)),
        )
        .child(
            div()
                .w_full()
                .max_w(px(920.0))
                .self_center()
                .flex()
                .flex_col()
                .gap_2()
                .when(controls_visible, |el| {
                    el.child(render_progress_slider(
                        view,
                        p,
                        "fullscreen-progress",
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(render_icon_button(
                                        "fullscreen-prev",
                                        IconName::SkipBack,
                                        19.0,
                                        40.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                                        cx,
                                    ))
                                    .child(render_icon_button(
                                        "fullscreen-play",
                                        play_icon,
                                        21.0,
                                        48.0,
                                        p.accent_fg,
                                        p.accent,
                                        p.accent,
                                        |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                        cx,
                                    ))
                                    .child(render_icon_button(
                                        "fullscreen-next",
                                        IconName::SkipForward,
                                        19.0,
                                        40.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| this.backend.send(BackendCmd::NextTrack),
                                        cx,
                                    )),
                            )
                            .child(render_volume_slider(view, p, cx)),
                    )
                }),
        )
        .into_any_element()
}

fn render_lyrics_panel(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let content = match &view.snapshot.lyrics {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            let count = lines.len();
            uniform_list(
                "fullscreen-lyrics-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, cx| {
                    let palette = this.palette();
                    let active = this.snapshot.active_lyric_index();
                    let LyricsState::Synced(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|index| {
                            let line = lines.get(index)?;
                            let distance = active.map(|value| value.abs_diff(index)).unwrap_or(9);
                            let (size, opacity, color, weight) = match distance {
                                0 => (29.0, 1.0, palette.lyrics_active, FontWeight::BOLD),
                                1 => (22.0, 0.78, palette.lyrics_mid, FontWeight::SEMIBOLD),
                                2 => (19.0, 0.48, palette.lyrics_mid, FontWeight::MEDIUM),
                                _ => (17.0, 0.22, palette.lyrics_inactive, FontWeight::MEDIUM),
                            };
                            let text = if line.text.trim().is_empty() {
                                "♪".to_string()
                            } else {
                                line.text.clone()
                            };
                            let time_secs = line.time_secs;
                            Some(
                                div()
                                    .id(SharedString::from(format!("fullscreen-lyric-{index}")))
                                    .w_full()
                                    .min_h(px(66.0))
                                    .px_4()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_center()
                                    .cursor_pointer()
                                    .opacity(opacity)
                                    .hover(|style| style.opacity(1.0))
                                    .text_size(px(size))
                                    .font_weight(weight)
                                    .text_color(color)
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(time_secs));
                                    }))
                                    .child(text),
                            )
                        })
                        .collect()
                }),
            )
            .track_scroll(&view.lyrics_scroll)
            .flex_1()
            .w_full()
            .into_any_element()
        }
        _ => {
            let message = match &view.snapshot.lyrics {
                LyricsState::Loading => "Fetching synced lyrics...",
                LyricsState::NotFound => "No lyrics found for this track.",
                LyricsState::Plain(_) => "Synced lyrics are unavailable for this track.",
                LyricsState::Idle => "Start playback to view synced lyrics.",
                LyricsState::Synced(_) => "No lyrics found for this track.",
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(p.muted)
                .child(message)
                .into_any_element()
        }
    };

    div()
        .id("fullscreen-lyrics")
        .w(px(520.0))
        .max_w_full()
        .h_full()
        .min_h(px(280.0))
        .p_4()
        .rounded_2xl()
        .bg(p.panel_bg)
        .border_1()
        .border_color(p.border_subtle)
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.muted)
                .child("LYRICS"),
        )
        .child(content)
        .into_any_element()
}
