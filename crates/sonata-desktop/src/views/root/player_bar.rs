//! Bottom persistent playback bar with track info, transport controls, progress, and volume.
//!
//! 1. **Now Playing**: artwork thumbnail (clickable to enter fullscreen), title, artist, and stream quality badge.
//! 2. **Transport & Sliders**: shuffle, prev, play/pause, next, repeat, progress slider, lyrics toggle, and volume slider.

use super::super::{
    render_cover_art, render_icon_button, render_progress_slider, render_volume_slider, CenterMode,
    SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use sonata_core::player::RepeatMode;

pub(super) fn render_player_bar(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let track = snap.current_track().cloned();
    let title = track
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "Not Playing".to_string());
    let artist = track
        .as_ref()
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| snap.status_line.clone());

    let play_icon = if snap.paused {
        IconName::Play
    } else {
        IconName::Pause
    };
    let repeat_icon = if snap.repeat == RepeatMode::One {
        IconName::Repeat1
    } else {
        IconName::Repeat
    };

    div()
        .w_full()
        .px_3()
        .pb_3()
        .child(
            div()
                .w_full()
                .h(px(82.0))
                .px_4()
                .rounded_2xl()
                .bg(p.player_bar_bg)
                .border_1()
                .border_color(p.border)
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .w(px(295.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("pb-art-toggle")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_fullscreen_view(window, cx);
                                }))
                                .child(render_cover_art(
                                    view.active_cover_art_path(),
                                    52.0,
                                    10.0,
                                    p,
                                )),
                        )
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
                                .child(div().text_xs().text_color(p.muted).truncate().child(artist))
                                .child(
                                    div().flex().items_center().gap_2().child(
                                        div()
                                            .text_xs()
                                            .text_color(p.success)
                                            .truncate()
                                            .child(snap.stream_badge.clone()),
                                    ),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .max_w(px(540.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(render_icon_button(
                                    "pb-shuffle",
                                    IconName::Shuffle,
                                    15.0,
                                    28.0,
                                    if snap.shuffle { p.accent } else { p.muted },
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::ToggleShuffle),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-prev",
                                    IconName::SkipBack,
                                    17.0,
                                    32.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-play-pause",
                                    play_icon,
                                    18.0,
                                    38.0,
                                    p.accent_fg,
                                    p.accent,
                                    p.accent,
                                    |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-next",
                                    IconName::SkipForward,
                                    17.0,
                                    32.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::NextTrack),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-repeat",
                                    repeat_icon,
                                    15.0,
                                    28.0,
                                    if snap.repeat != RepeatMode::Off {
                                        p.accent
                                    } else {
                                        p.muted
                                    },
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::CycleRepeat),
                                    cx,
                                )),
                        )
                        .child(render_progress_slider(view, p, "pb-progress", false, cx)),
                )
                .child(
                    div()
                        .w(px(270.0))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(render_icon_button(
                            "pb-lyrics",
                            IconName::MicVocal,
                            15.0,
                            30.0,
                            if view.center_mode == CenterMode::Lyrics {
                                p.accent
                            } else {
                                p.muted
                            },
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, cx| this.toggle_lyrics_view(cx),
                            cx,
                        ))
                        .child(render_volume_slider(view, p, cx)),
                ),
        )
        .into_any_element()
}
