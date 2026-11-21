//! Responsive compact mini-player view for small windows and manual mini mode.
//!
//! 1. **3-Tier Layout (`MiniTier`)**: adapts between Ultra-compact (`< 380x160`), Medium compact (`< 560x280`), and Expanded mini player (`< 760x460`).
//! 2. **Live Karaoke Lyrics**: renders progressive word-by-word karaoke highlighting using `LyricLine` word timings or proportional line sweeps.
//! 3. **Animated Artwork & Controls**: displays `active_cover_art_path()` with a `MOTION` pill when animated frames are active, plus progress, transport, volume, and clean expand-to-workspace handling.

use super::{
    render_cover_surface, render_icon_button, render_progress_slider, render_volume_slider,
    ui_icon, FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, relative, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, RenderImage, SharedString, Styled,
    Window,
};
use sonata_core::lyrics::{LyricLine, LyricsState};
use sonata_core::player::RepeatMode;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniTier {
    UltraCompact,
    Medium,
    Expanded,
}

pub fn mini_tier(width: f32, height: f32) -> MiniTier {
    if width < 380.0 || height < 160.0 {
        MiniTier::UltraCompact
    } else if width < 560.0 || height < 280.0 {
        MiniTier::Medium
    } else {
        MiniTier::Expanded
    }
}

pub fn render_mini_player(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let viewport = window.viewport_size();
    let vw: f32 = viewport.width.into();
    let vh: f32 = viewport.height.into();
    let tier = mini_tier(vw, vh);

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
    let repeat_icon = if view.snapshot.repeat == RepeatMode::One {
        IconName::Repeat1
    } else {
        IconName::Repeat
    };
    let motion_tex = view.active_motion_texture();
    let has_motion = motion_tex.is_some();
    let cover_path = view.snapshot.art_path.clone();

    let inner = match tier {
        MiniTier::UltraCompact => {
            render_ultra_compact(view, p, &title, &artist, play_icon, vw, vh, cx)
        }
        MiniTier::Medium => render_medium_compact(
            view,
            p,
            &title,
            &artist,
            play_icon,
            cover_path.as_ref(),
            motion_tex,
            has_motion,
            vh,
            cx,
        ),
        MiniTier::Expanded => render_expanded_mini(
            view,
            p,
            &title,
            &artist,
            play_icon,
            repeat_icon,
            cover_path.as_ref(),
            motion_tex,
            has_motion,
            cx,
        ),
    };

    div()
        .size_full()
        .top_0()
        .left_0()
        .bg(p.window_bg)
        .child(
            div()
                .size_full()
                .p(px(if tier == MiniTier::UltraCompact {
                    8.0
                } else {
                    12.0
                }))
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
                .child(inner),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_ultra_compact(
    view: &SonataDesktopView,
    p: &GlassPalette,
    title: &str,
    artist: &str,
    play_icon: IconName,
    vw: f32,
    vh: f32,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let active_lyric = view.snapshot.active_lyric_line().map(ToString::to_string);

    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_between()
        .gap_1()
        .child(
            div()
                .id("mini-ultra-drag")
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .cursor_grab()
                .on_mouse_down(MouseButton::Left, |_, window, _| {
                    window.start_window_move();
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(title.to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .truncate()
                                .child(artist.to_string()),
                        ),
                )
                .child(render_icon_button(
                    "mini-expand",
                    IconName::Maximize2,
                    13.0,
                    24.0,
                    p.text,
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    |this, window, cx| this.exit_mini_mode(window, cx),
                    cx,
                )),
        )
        .when(vh >= 132.0, |el| {
            el.child(render_progress_slider(view, p, "mini-progress", true, cx))
        })
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .when(vw >= 340.0, |el| {
                    el.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(p.lyrics_mid)
                            .truncate()
                            .child(active_lyric.unwrap_or_default()),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(render_icon_button(
                            "mini-prev",
                            IconName::SkipBack,
                            14.0,
                            26.0,
                            p.text,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                            cx,
                        ))
                        .child(render_icon_button(
                            "mini-play",
                            play_icon,
                            15.0,
                            30.0,
                            p.accent_fg,
                            p.accent,
                            p.accent,
                            |this, _, _| this.backend.send(BackendCmd::TogglePause),
                            cx,
                        ))
                        .child(render_icon_button(
                            "mini-next",
                            IconName::SkipForward,
                            14.0,
                            26.0,
                            p.text,
                            gpui_kit::transparent_black(),
                            p.pill_bg,
                            |this, _, _| this.backend.send(BackendCmd::NextTrack),
                            cx,
                        )),
                ),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_medium_compact(
    view: &SonataDesktopView,
    p: &GlassPalette,
    title: &str,
    artist: &str,
    play_icon: IconName,
    cover_path: Option<&std::path::PathBuf>,
    motion_tex: Option<Arc<RenderImage>>,
    has_motion: bool,
    vh: f32,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_between()
        .gap_2()
        .child(render_mini_header(view, p, has_motion, false, cx))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .child(render_cover_surface(cover_path, motion_tex, 56.0, 10.0, p))
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
                                .child(title.to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .truncate()
                                .child(artist.to_string()),
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
        .when(view.mini_show_lyric && vh >= 180.0, |el| {
            el.child(
                div()
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded_lg()
                    .bg(p.card_bg)
                    .child(render_mini_karaoke_strip(view, p, 12.5)),
            )
        })
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_expanded_mini(
    view: &SonataDesktopView,
    p: &GlassPalette,
    title: &str,
    artist: &str,
    play_icon: IconName,
    repeat_icon: IconName,
    cover_path: Option<&std::path::PathBuf>,
    motion_tex: Option<Arc<RenderImage>>,
    has_motion: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;

    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_between()
        .gap_3()
        .child(render_mini_header(view, p, has_motion, true, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(render_cover_surface(cover_path, motion_tex, 112.0, 14.0, p)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.highlight_fg)
                                        .truncate()
                                        .child(title.to_string()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(artist.to_string()),
                                ),
                        )
                        .when(view.mini_show_lyric, |el| {
                            el.child(
                                div()
                                    .w_full()
                                    .p(px(10.0))
                                    .rounded_xl()
                                    .bg(p.card_bg)
                                    .border_1()
                                    .border_color(p.border_subtle)
                                    .child(render_mini_karaoke_panel(view, p)),
                            )
                        }),
                ),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_progress_slider(view, p, "mini-progress", false, cx))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_icon_button(
                                    "mini-shuffle",
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
                                    "mini-prev",
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
                                    "mini-play",
                                    play_icon,
                                    17.0,
                                    36.0,
                                    p.accent_fg,
                                    p.accent,
                                    p.accent,
                                    |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                    cx,
                                ))
                                .child(render_icon_button(
                                    "mini-next",
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
                                    "mini-repeat",
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
                        .child(render_volume_slider(view, p, cx)),
                ),
        )
        .into_any_element()
}

fn render_mini_header(
    view: &SonataDesktopView,
    p: &GlassPalette,
    has_motion: bool,
    show_fullscreen_btn: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
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
                )
                .when(has_motion, |el| {
                    el.child(
                        div()
                            .px(px(6.0))
                            .py(px(1.0))
                            .rounded_full()
                            .bg(p.accent_soft)
                            .border_1()
                            .border_color(p.accent)
                            .text_size(px(9.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.accent)
                            .child("MOTION"),
                    )
                }),
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
                        if this.mini_mode {
                            let h = if this.mini_show_lyric { 216.0 } else { 166.0 };
                            window.resize(gpui_kit::size(px(430.0), px(h)));
                        }
                        cx.notify();
                    },
                    cx,
                ))
                .when(show_fullscreen_btn, |el| {
                    el.child(render_icon_button(
                        "mini-fullscreen",
                        IconName::Sparkles,
                        13.0,
                        24.0,
                        p.text,
                        gpui_kit::transparent_black(),
                        p.pill_bg,
                        |this, window, cx| this.toggle_fullscreen_view(window, cx),
                        cx,
                    ))
                })
                .child(render_icon_button(
                    "mini-expand",
                    IconName::Maximize2,
                    14.0,
                    24.0,
                    p.text,
                    gpui_kit::transparent_black(),
                    p.pill_bg,
                    |this, window, cx| this.exit_mini_mode(window, cx),
                    cx,
                )),
        )
        .into_any_element()
}

fn render_mini_karaoke_strip(
    view: &SonataDesktopView,
    p: &GlassPalette,
    font_size: f32,
) -> AnyElement {
    let snap = &view.snapshot;
    if let (LyricsState::Synced(lines), Some(idx)) = (&snap.lyrics, snap.active_lyric_index()) {
        if let Some(line) = lines.get(idx) {
            let end_secs = lines
                .get(idx + 1)
                .map(|n| n.time_secs)
                .unwrap_or(line.time_secs + 4.0);
            return render_karaoke_line(line, snap.position_secs, end_secs, font_size, p);
        }
    }
    div()
        .text_size(px(font_size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted)
        .truncate()
        .child("♪ Instrumental / No synced lyric")
        .into_any_element()
}

fn render_mini_karaoke_panel(view: &SonataDesktopView, p: &GlassPalette) -> AnyElement {
    let snap = &view.snapshot;
    if let (LyricsState::Synced(lines), Some(idx)) = (&snap.lyrics, snap.active_lyric_index()) {
        let prev = idx.checked_sub(1).and_then(|i| lines.get(i));
        let cur = lines.get(idx);
        let next = lines.get(idx + 1);
        let end_secs = next
            .map(|n| n.time_secs)
            .unwrap_or_else(|| cur.map_or(0.0, |c| c.time_secs + 4.0));

        let mut panel = div().w_full().flex().flex_col().gap(px(4.0));
        if let Some(l) = prev {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(p.lyrics_inactive)
                    .truncate()
                    .child(l.text.clone()),
            );
        }
        if let Some(l) = cur {
            panel = panel.child(render_karaoke_line(
                l,
                snap.position_secs,
                end_secs,
                14.0,
                p,
            ));
        }
        if let Some(l) = next {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(p.lyrics_mid)
                    .truncate()
                    .child(l.text.clone()),
            );
        }
        return panel.into_any_element();
    }

    div()
        .text_xs()
        .text_color(p.muted)
        .truncate()
        .child(match &snap.lyrics {
            LyricsState::Loading => "Fetching synced lyrics...".to_string(),
            LyricsState::Plain(lines) if !lines.is_empty() => lines[0].clone(),
            _ => "♪ Instrumental / No synced lyric".to_string(),
        })
        .into_any_element()
}

fn render_karaoke_line(
    line: &LyricLine,
    position_secs: f64,
    line_end_secs: f64,
    font_size: f32,
    p: &GlassPalette,
) -> AnyElement {
    let tokens: Vec<(String, f32)> = if !line.words.is_empty() {
        let count = line.words.len();
        line.words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let start = w.start_secs;
                let end = w
                    .end_secs
                    .or_else(|| line.words.get(i + 1).map(|next| next.start_secs))
                    .unwrap_or(line_end_secs)
                    .max(start + 0.18);
                let share = ((position_secs - start) / (end - start)).clamp(0.0, 1.0) as f32;
                let mut label = w.text.clone();
                if i + 1 < count {
                    label.push(' ');
                }
                (label, share)
            })
            .collect()
    } else {
        let words: Vec<&str> = line.text.split_whitespace().collect();
        if words.is_empty() {
            Vec::new()
        } else {
            let total_chars: f64 = words.iter().map(|w| (w.chars().count() + 1) as f64).sum();
            let span = (line_end_secs - line.time_secs).max(0.6);
            let line_progress = ((position_secs - line.time_secs) / span).clamp(0.0, 1.0);
            let mut cursor = 0.0f64;
            let count = words.len();
            words
                .into_iter()
                .enumerate()
                .map(|(i, w)| {
                    let weight = (w.chars().count() + 1) as f64 / total_chars.max(1.0);
                    let w_start = cursor;
                    let w_end = (cursor + weight).min(1.0);
                    cursor = w_end;
                    let share = if w_end > w_start {
                        ((line_progress - w_start) / (w_end - w_start)).clamp(0.0, 1.0) as f32
                    } else {
                        1.0
                    };
                    let mut label = w.to_string();
                    if i + 1 < count {
                        label.push(' ');
                    }
                    (label, share)
                })
                .collect()
        }
    };

    if tokens.is_empty() {
        return div()
            .text_size(px(font_size))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(p.lyrics_active)
            .child("♪")
            .into_any_element();
    }

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .text_size(px(font_size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.lyrics_mid)
        .children(tokens.into_iter().map(move |(token, share)| {
            let shared = SharedString::from(token);
            div()
                .relative()
                .flex_none()
                .whitespace_nowrap()
                .child(shared.clone())
                .when(share > 0.005, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(relative(share))
                            .overflow_hidden()
                            .text_color(p.lyrics_active)
                            .child(div().whitespace_nowrap().child(shared)),
                    )
                })
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::is_compact_viewport;

    #[test]
    fn mini_tiers_match_responsive_thresholds() {
        assert_eq!(mini_tier(280.0, 110.0), MiniTier::UltraCompact);
        assert_eq!(mini_tier(379.0, 220.0), MiniTier::UltraCompact);
        assert_eq!(mini_tier(430.0, 210.0), MiniTier::Medium);
        assert_eq!(mini_tier(640.0, 340.0), MiniTier::Expanded);
    }

    #[test]
    fn compact_viewport_auto_activates_below_760x460() {
        assert!(is_compact_viewport(759.0, 500.0));
        assert!(is_compact_viewport(900.0, 459.0));
        assert!(!is_compact_viewport(760.0, 460.0));
    }
}
