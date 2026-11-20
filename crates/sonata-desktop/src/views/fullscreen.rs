use super::{render_cover_art, render_icon_button, FluentDivExt, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled,
};
use sonata_core::lyrics::LyricsState;

/// Render the desktop-only immersive playback workspace.
///
/// Flex wrapping keeps artwork, lyrics, and controls usable in narrow windows without imposing an
/// Android layout contract.
pub fn render_fullscreen_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .size_full()
        .on_mouse_move(cx.listener(|this, _, _, cx| this.note_fullscreen_activity(cx)))
        .bg(linear_gradient(
            140.0,
            linear_color_stop(p.ambient_top, 0.0),
            linear_color_stop(p.window_bg, 0.72),
        ))
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
                .h(px(48.0))
                .flex()
                .items_center()
                .justify_center()
                .gap_3()
                .when(controls_visible, |el| {
                    el.child(render_icon_button(
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
                    ))
                }),
        )
        .into_any_element()
}

fn render_lyrics_panel(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let rows = match &view.snapshot.lyrics {
        LyricsState::Synced(lines) => {
            let active = view.snapshot.active_lyric_index();
            lines
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let distance = active.map(|value| value.abs_diff(index)).unwrap_or(9);
                    let color = if distance == 0 {
                        p.lyrics_active
                    } else if distance <= 1 {
                        p.lyrics_mid
                    } else {
                        p.lyrics_inactive
                    };
                    let text = if line.text.trim().is_empty() {
                        "♪".to_string()
                    } else {
                        line.text.clone()
                    };
                    let time_secs = line.time_secs;
                    div()
                        .id(SharedString::from(format!("fullscreen-lyric-{index}")))
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(if distance == 0 {
                            p.ambient_glow
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .hover(|style| style.bg(p.card_hover))
                        .cursor_pointer()
                        .text_size(px(if distance == 0 { 25.0 } else { 18.0 }))
                        .font_weight(if distance == 0 {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(color)
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::SeekAbsolute(time_secs));
                        }))
                        .child(text)
                })
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };

    let content = if rows.is_empty() {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(p.muted)
            .child("Synced lyrics are unavailable for this track.")
            .into_any_element()
    } else {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(rows)
            .into_any_element()
    };

    div()
        .id("fullscreen-lyrics")
        .w(px(480.0))
        .max_w_full()
        .h_full()
        .min_h(px(240.0))
        .overflow_y_scroll()
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
                .child("SYNCED LYRICS"),
        )
        .child(content)
        .into_any_element()
}
