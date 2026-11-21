//! Bottom persistent playback bar with skin-native layouts for Sonata, Spotify, and YouTube Music.
//!
//! 1. **Sonata Glass**: floating frosted bar with cover art, center transport + scrubber, and right-aligned tools.
//! 2. **Spotify**: flush black bar with 56px artwork + like on the left, white circular Play/Pause + scrubber in the center, and compact controls on the right.
//! 3. **YouTube Music**: flush `#212121` bar with a full-width top-edge red progress scrubber, left transport + timecode, centered track info, and right volume/mode controls.

use super::super::{
    format_mm_ss, render_cover_surface, render_icon_button, render_progress_slider,
    render_volume_slider, CenterMode, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::{DesktopSkin, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, relative, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, StatefulInteractiveElement, Styled,
};
use sonata_core::player::RepeatMode;

pub(super) fn render_player_bar(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    match p.skin {
        DesktopSkin::YouTubeMusic => render_ytm_player_bar(view, p, cx),
        DesktopSkin::Spotify => render_spotify_player_bar(view, p, cx),
        DesktopSkin::Sonata => render_sonata_player_bar(view, p, cx),
    }
}

fn render_sonata_player_bar(
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
        .px_4()
        .pb_3()
        .pt_1()
        .child(
            div()
                .w_full()
                .h(px(76.0))
                .px_4()
                .rounded_2xl()
                .bg(p.player_bar_bg)
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .w(px(280.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("pb-art-toggle")
                                .flex_shrink_0()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_fullscreen_view(window, cx);
                                }))
                                .child(render_cover_surface(
                                    snap.art_path.as_ref(),
                                    view.active_motion_texture(),
                                    50.0,
                                    10.0,
                                    p,
                                )),
                        )
                        .child(
                            div()
                                .max_w(px(176.0))
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
                        .children(track.map(|t| {
                            let liked = t.liked;
                            render_icon_button(
                                "pb-like",
                                IconName::Heart,
                                15.0,
                                28.0,
                                if liked { p.accent } else { p.muted },
                                gpui_kit::transparent_black(),
                                p.pill_bg,
                                move |this, _, _| {
                                    this.backend.send(BackendCmd::ToggleLike(t.clone()));
                                },
                                cx,
                            )
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .max_w(px(520.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(render_icon_button(
                                    "pb-shuffle",
                                    IconName::Shuffle,
                                    14.0,
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
                                    16.0,
                                    30.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-play-pause",
                                    play_icon,
                                    17.0,
                                    36.0,
                                    p.play_btn_fg,
                                    p.play_btn_bg,
                                    p.play_btn_bg,
                                    |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-next",
                                    IconName::SkipForward,
                                    16.0,
                                    30.0,
                                    p.text,
                                    gpui_kit::transparent_black(),
                                    p.pill_bg,
                                    |this, _, _| this.backend.send(BackendCmd::NextTrack),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "pb-repeat",
                                    repeat_icon,
                                    14.0,
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
                        .child(render_progress_slider(view, p, "pb-progress", true, cx)),
                )
                .child(
                    div()
                        .w(px(280.0))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_1()
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

/// Authentic Spotify Desktop bottom player bar:
/// flush `#000000` bar, 56px rounded-md cover + title/artist + green heart on left,
/// white circular Play/Pause + scrubber in center, lyrics/queue/volume/fullscreen on right.
fn render_spotify_player_bar(
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
        .h(px(84.0))
        .px_4()
        .bg(p.player_bar_bg)
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .w(px(300.0))
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .id("pb-art-toggle")
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_fullscreen_view(window, cx);
                        }))
                        .child(render_cover_surface(
                            snap.art_path.as_ref(),
                            view.active_motion_texture(),
                            56.0,
                            6.0,
                            p,
                        )),
                )
                .child(
                    div()
                        .max_w(px(190.0))
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(title),
                        )
                        .child(div().text_xs().text_color(p.muted).truncate().child(artist)),
                )
                .children(track.map(|t| {
                    let liked = t.liked;
                    render_icon_button(
                        "pb-like",
                        IconName::Heart,
                        15.0,
                        28.0,
                        if liked { p.accent } else { p.muted },
                        gpui_kit::transparent_black(),
                        p.pill_bg,
                        move |this, _, _| {
                            this.backend.send(BackendCmd::ToggleLike(t.clone()));
                        },
                        cx,
                    )
                })),
        )
        .child(
            div()
                .flex_1()
                .max_w(px(580.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
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
                            30.0,
                            p.text,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-play-pause",
                            play_icon,
                            16.0,
                            34.0,
                            p.play_btn_fg,
                            p.play_btn_bg,
                            p.play_btn_bg,
                            |this, _, _| this.backend.send(BackendCmd::TogglePause),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-next",
                            IconName::SkipForward,
                            17.0,
                            30.0,
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
                .child(render_progress_slider(view, p, "pb-progress", true, cx)),
        )
        .child(
            div()
                .w(px(300.0))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(render_icon_button(
                    "pb-lyrics",
                    IconName::MicVocal,
                    15.0,
                    28.0,
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
                .child(render_icon_button(
                    "pb-queue",
                    IconName::ListMusic,
                    15.0,
                    28.0,
                    if view.show_queue { p.accent } else { p.muted },
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    |this, _, cx| {
                        this.show_queue = !this.show_queue;
                        cx.notify();
                    },
                    cx,
                ))
                .child(render_volume_slider(view, p, cx))
                .child(render_icon_button(
                    "pb-fullscreen",
                    IconName::Maximize2,
                    14.0,
                    28.0,
                    p.muted,
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    |this, window, cx| this.toggle_fullscreen_view(window, cx),
                    cx,
                )),
        )
        .into_any_element()
}

/// Authentic YouTube Music Desktop bottom player bar:
/// full-width top-edge red progress scrubber, left transport + `cur / dur`,
/// centered artwork + title/artist + like, right volume + repeat + shuffle + lyrics + fullscreen.
fn render_ytm_player_bar(
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

    let ratio = if snap.duration_secs > 0.0 {
        (snap.position_secs / snap.duration_secs).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let cur_str = format_mm_ss(snap.position_secs.max(0.0).floor() as u64);
    let dur_str = format_mm_ss(snap.duration_secs.max(0.0).floor() as u64);
    let bounds_cell = view.progress_bounds.clone();

    div()
        .w_full()
        .bg(p.player_bar_bg)
        .flex()
        .flex_col()
        .child(
            // Signature YouTube Music edge-to-edge top progress scrubber
            div()
                .w_full()
                .h(px(6.0))
                .on_children_prepainted(move |children, _, _| {
                    if let Some(first) = children.first() {
                        bounds_cell.set(Some(*first));
                    }
                })
                .child(
                    div()
                        .id("pb-progress")
                        .size_full()
                        .flex()
                        .items_start()
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
                                .h(px(3.0))
                                .bg(p.pill_bg)
                                .overflow_hidden()
                                .child(div().h_full().w(relative(ratio)).bg(p.accent)),
                        ),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(66.0))
                .px_4()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    // Left: Prev, Play/Pause, Next + timestamp (exact YTM left cluster)
                    div()
                        .w(px(260.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon_button(
                            "pb-prev",
                            IconName::SkipBack,
                            18.0,
                            34.0,
                            p.highlight_fg,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-play-pause",
                            play_icon,
                            24.0,
                            40.0,
                            p.highlight_fg,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::TogglePause),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-next",
                            IconName::SkipForward,
                            18.0,
                            34.0,
                            p.highlight_fg,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::NextTrack),
                            cx,
                        ))
                        .child(
                            div()
                                .pl_2()
                                .text_xs()
                                .text_color(p.muted)
                                .child(format!("{cur_str} / {dur_str}")),
                        ),
                )
                .child(
                    // Center: 42px cover + Title + Artist + Like (exact YTM center cluster)
                    div()
                        .flex_1()
                        .max_w(px(460.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(
                            div()
                                .id("pb-art-toggle")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_fullscreen_view(window, cx);
                                }))
                                .child(render_cover_surface(
                                    snap.art_path.as_ref(),
                                    view.active_motion_texture(),
                                    42.0,
                                    4.0,
                                    p,
                                )),
                        )
                        .child(
                            div()
                                .max_w(px(300.0))
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
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
                        .children(track.map(|t| {
                            let liked = t.liked;
                            render_icon_button(
                                "pb-like",
                                IconName::Heart,
                                16.0,
                                30.0,
                                if liked { p.accent } else { p.muted },
                                gpui_kit::transparent_black(),
                                p.pill_bg,
                                move |this, _, _| {
                                    this.backend.send(BackendCmd::ToggleLike(t.clone()));
                                },
                                cx,
                            )
                        })),
                )
                .child(
                    // Right: Volume, Repeat, Shuffle, Lyrics, Fullscreen (exact YTM right cluster)
                    div()
                        .w(px(320.0))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_1()
                        .child(render_volume_slider(view, p, cx))
                        .child(render_icon_button(
                            "pb-repeat",
                            repeat_icon,
                            16.0,
                            30.0,
                            if snap.repeat != RepeatMode::Off {
                                p.highlight_fg
                            } else {
                                p.muted
                            },
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::CycleRepeat),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-shuffle",
                            IconName::Shuffle,
                            16.0,
                            30.0,
                            if snap.shuffle {
                                p.highlight_fg
                            } else {
                                p.muted
                            },
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::ToggleShuffle),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-lyrics",
                            IconName::MicVocal,
                            16.0,
                            30.0,
                            if view.center_mode == CenterMode::Lyrics {
                                p.highlight_fg
                            } else {
                                p.muted
                            },
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, cx| this.toggle_lyrics_view(cx),
                            cx,
                        ))
                        .child(render_icon_button(
                            "pb-fullscreen",
                            IconName::Maximize2,
                            15.0,
                            30.0,
                            p.muted,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, window, cx| this.toggle_fullscreen_view(window, cx),
                            cx,
                        )),
                ),
        )
        .into_any_element()
}
