//! Left-column cover artwork, track metadata, and backdrop-integrated playback controls.
//!
//! 1. **Cover & Metadata**: renders rounded album art with dual shadows alongside a balanced explicit-badge title row and medium-weight artist subtitle.
//! 2. **Backdrop Controls**: `render_left_controls` places progress, transport, and volume sliders directly over the ambient wave field with no card box or border.
//! 3. **Layout Stability**: a fixed-height `CONTROLS_DOCK_HEIGHT` container prevents artwork jumping when controls auto-hide.

use super::super::{
    render_cover_art, render_icon_button, render_progress_slider, render_volume_slider,
    FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::{rgb_hsla, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, point, px, AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};

const CONTROLS_DOCK_HEIGHT: f32 = 84.0;

pub(super) fn has_explicit_marker(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    lower.contains("[explicit]") || lower.contains("(explicit)")
}

pub(super) fn strip_explicit_suffix(title: &str) -> String {
    let mut cleaned = title.to_string();
    for marker in [
        "[Explicit]",
        "(Explicit)",
        "[explicit]",
        "(explicit)",
        "[EXPLICIT]",
        "(EXPLICIT)",
    ] {
        cleaned = cleaned.replace(marker, "");
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fullscreen_controls_palette(p: &GlassPalette) -> GlassPalette {
    let mut ctrl = p.clone();
    ctrl.text = rgb_hsla((255, 255, 255), 0.94);
    ctrl.muted = rgb_hsla((255, 255, 255), 0.58);
    ctrl.accent = rgb_hsla((255, 255, 255), 0.92);
    ctrl.pill_bg = rgb_hsla((255, 255, 255), 0.22);
    ctrl
}

fn render_left_controls(
    view: &SonataDesktopView,
    p: &GlassPalette,
    wide: bool,
    controls_visible: bool,
    paused: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let dock = div()
        .w_full()
        .h(px(CONTROLS_DOCK_HEIGHT))
        .flex()
        .flex_col()
        .justify_start();

    if !controls_visible {
        return dock.into_any_element();
    }

    let ctrl_p = fullscreen_controls_palette(p);
    let play_icon = if paused {
        IconName::Play
    } else {
        IconName::Pause
    };

    let transport = div()
        .flex()
        .items_center()
        .gap_2()
        .child(render_icon_button(
            "fullscreen-prev",
            IconName::SkipBack,
            18.0,
            36.0,
            rgb_hsla((255, 255, 255), 0.88),
            gpui_kit::transparent_black(),
            rgb_hsla((255, 255, 255), 0.14),
            |this, _, _| this.backend.send(BackendCmd::PrevTrack),
            cx,
        ))
        .child(render_icon_button(
            "fullscreen-play",
            play_icon,
            19.0,
            42.0,
            rgb_hsla((14, 10, 22), 0.96),
            rgb_hsla((255, 255, 255), 0.92),
            rgb_hsla((255, 255, 255), 1.0),
            |this, _, _| this.backend.send(BackendCmd::TogglePause),
            cx,
        ))
        .child(render_icon_button(
            "fullscreen-next",
            IconName::SkipForward,
            18.0,
            36.0,
            rgb_hsla((255, 255, 255), 0.88),
            gpui_kit::transparent_black(),
            rgb_hsla((255, 255, 255), 0.14),
            |this, _, _| this.backend.send(BackendCmd::NextTrack),
            cx,
        ));

    let volume = render_volume_slider(view, &ctrl_p, cx);

    let bottom_row = if wide {
        div()
            .relative()
            .w_full()
            .h(px(42.0))
            .flex()
            .items_center()
            .justify_center()
            .child(transport)
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(volume),
            )
    } else {
        div()
            .w_full()
            .h(px(42.0))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(transport)
            .child(volume)
    };

    dock.id("fullscreen-controls")
        .pt(px(4.0))
        .gap(px(12.0))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(render_progress_slider(
            view,
            &ctrl_p,
            "fullscreen-progress",
            false,
            cx,
        ))
        .child(bottom_row)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_cover_column(
    view: &SonataDesktopView,
    art_path: Option<&std::path::PathBuf>,
    cover_size: f32,
    cover_radius: f32,
    title: &str,
    artist: &str,
    explicit: bool,
    title_size: f32,
    artist_size: f32,
    wide: bool,
    controls_visible: bool,
    paused: bool,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let explicit_badge = || {
        div()
            .px(px(5.0))
            .py(px(1.5))
            .rounded(px(4.0))
            .bg(rgb_hsla((255, 255, 255), 0.10))
            .text_size(px((artist_size * 0.72).clamp(9.5, 11.5)))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb_hsla((255, 255, 255), 0.56))
            .child("E")
    };

    let col_width = if wide {
        cover_size.max(360.0) + 40.0
    } else {
        cover_size.max(320.0) + 24.0
    };

    div()
        .w(px(col_width))
        .max_w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(18.0))
        .child(
            div()
                .rounded(px(cover_radius))
                .shadow(vec![
                    BoxShadow {
                        color: rgb_hsla((0, 0, 0), 0.44),
                        offset: point(px(0.0), px(18.0)),
                        blur_radius: px(42.0),
                        spread_radius: px(-4.0),
                        inset: false,
                    },
                    BoxShadow {
                        color: rgb_hsla((0, 0, 0), 0.24),
                        offset: point(px(0.0), px(4.0)),
                        blur_radius: px(12.0),
                        spread_radius: px(0.0),
                        inset: false,
                    },
                ])
                .child(render_cover_art(art_path, cover_size, cover_radius, p)),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .child(
                    // Three-part balanced row so the explicit badge never shifts the title off-center.
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_end()
                                .invisible()
                                .when(explicit, |el| el.child(explicit_badge())),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(title_size))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb_hsla((255, 255, 255), 0.96))
                                .text_center()
                                .truncate()
                                .child(SharedString::from(title.to_string())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .when(explicit, |el| el.child(explicit_badge())),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(artist_size))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb_hsla((255, 255, 255), 0.58))
                        .text_center()
                        .truncate()
                        .child(SharedString::from(artist.to_string())),
                ),
        )
        .child(render_left_controls(
            view,
            p,
            wide,
            controls_visible,
            paused,
            cx,
        ))
        .into_any_element()
}
