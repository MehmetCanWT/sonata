//! Shared GPUI UI primitives, artwork cards, hover overlays, and micro-interactions.
//!
//! 1. **Surface & Card Styling (`card_panel`, `header_wash`, `scroll_edge_fades`)**: builds floating translucent cards with solved edge outlines, adaptive artwork washes, and smooth translucent scroll edge fades.
//! 2. **Media Tiles (`cover_art_tile`, `wide_thumb_tile`, `card_grid_item`)**: renders square, circular, and 16:9 video thumbnails with GPU hover `play_overlay` scrims and fallback placeholders.
//! 3. **Typography & Micro-Interactions (`fit_title_size`, `artist_links`, `hover_marquee`, `loading_ring`, `delayed_skeleton`)**: adaptive hero text sizing, clickable multi-artist credits, marquee overflow scroll, Play buffering ring, and `150 ms`-delayed `Skeleton` loaders.

use super::media::CachedMedia;
use crate::theme::DesktopPalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, hsla, img, linear_color_stop, linear_gradient, point, prelude::FluentBuilder as _, px,
    Animation, AnimationExt as _, AnyElement, App, BoxShadow, Div, ElementId, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, ScrollHandle,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};
use sonata_core::lyrics::{active_line_index, LyricLine, LyricVoice};
use std::rc::Rc;
use std::time::{Duration, Instant};

const SKELETON_DELAY_MS: u64 = 150;

pub type ActionCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

pub fn card_panel(palette: &DesktopPalette) -> Div {
    let mut d = div()
        .rounded(px(palette.radii.panel))
        .bg(palette.card_bg)
        .overflow_hidden();
    if palette.card_edge.a > 0.001 {
        d = d.border_1().border_color(palette.card_edge);
    }
    d
}

pub fn popup_panel(palette: &DesktopPalette) -> Div {
    div()
        .rounded(px(palette.radii.popup))
        .bg(palette.popup_bg)
        .border_1()
        .border_color(palette.outline)
        .shadow_lg()
}

/// Computes responsive headline font size (`26..=40 pt`) matching title length cleanly.
pub fn fit_title_size(title: &str) -> f32 {
    let chars = title.chars().count();
    if chars <= 16 {
        40.0
    } else if chars <= 28 {
        34.0
    } else if chars <= 42 {
        29.0
    } else {
        25.0
    }
}

/// Splits combined credit lines (e.g., `"Artist A, Artist B & Artist C"`) into individual clickable names.
pub fn split_artist_credits(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split([',', ';']) {
        for sub in part.split(" & ") {
            let trimmed = sub.trim();
            if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    if out.is_empty() && !raw.trim().is_empty() {
        out.push(raw.trim().to_string());
    }
    out
}

pub fn should_show_skeleton(started_at: Option<Instant>) -> bool {
    started_at.is_some_and(|t| t.elapsed() >= Duration::from_millis(SKELETON_DELAY_MS))
}

pub fn delayed_skeleton_block(
    id: impl Into<ElementId>,
    started_at: Option<Instant>,
    width_px: Option<f32>,
    height_px: f32,
    radius_px: f32,
    palette: &DesktopPalette,
) -> AnyElement {
    let id = id.into();
    let mut shell = div().id(id).h(px(height_px)).rounded(px(radius_px));
    if let Some(w) = width_px {
        shell = shell.w(px(w));
    } else {
        shell = shell.w_full();
    }
    if should_show_skeleton(started_at) {
        shell
            .bg(palette.surface.opacity(0.65))
            .overflow_hidden()
            .child(Skeleton::new().size_full().rounded(px(radius_px)))
            .into_any_element()
    } else {
        shell.into_any_element()
    }
}

/// Renders an artwork box (square or circle) with optional hover `play_overlay` scrim and accent Play badge.
pub fn cover_art_tile(
    id: impl Into<ElementId>,
    media: Option<&CachedMedia>,
    size_px: f32,
    circular: bool,
    palette: &DesktopPalette,
    is_active_playing: bool,
    on_play: Option<ActionCallback>,
) -> AnyElement {
    let radius = if circular {
        size_px * 0.5
    } else {
        palette.radii.art(size_px)
    };
    let elem_id: ElementId = id.into();
    let group_name: SharedString = format!("cov-{elem_id:?}").into();
    let show_overlay = on_play.is_some();

    div()
        .id(elem_id)
        .group(group_name.clone())
        .relative()
        .flex_shrink_0()
        .size(px(size_px))
        .rounded(px(radius))
        .bg(palette.surface)
        .overflow_hidden()
        .child(if let Some(m) = media {
            img(m.image.clone())
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        } else {
            div()
                .size_full()
                .rounded(px(radius))
                .flex()
                .items_center()
                .justify_center()
                .text_color(palette.dim)
                .child(Icon::new(IconName::Music2).size(px((size_px * 0.36).clamp(14.0, 40.0))))
                .into_any_element()
        })
        .when(show_overlay, |d| {
            let icon = if is_active_playing {
                IconName::Pause
            } else {
                IconName::Play
            };
            let btn_size = (size_px * 0.34).clamp(24.0, 44.0);
            let on_play_cb = on_play.clone();
            d.child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .rounded(px(radius))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::black().opacity(0.42))
                    .opacity(if is_active_playing { 1.0 } else { 0.0 })
                    .group_hover(group_name, |s| s.opacity(1.0))
                    .when_some(on_play_cb, |el, cb| {
                        el.cursor_pointer().on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            move |_, window, cx| {
                                cx.stop_propagation();
                                cb(window, cx);
                            },
                        )
                    })
                    .child(
                        div()
                            .size(px(btn_size))
                            .rounded_full()
                            .bg(palette.accent)
                            .text_color(palette.on_accent)
                            .shadow_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(icon).size(px(btn_size * 0.48))),
                    ),
            )
        })
        .into_any_element()
}

/// Renders a procedural multi-stop editorial artwork tile for Mood / Genre / Explore cards lacking a thumbnail URL.
pub fn mood_art_tile(
    id: impl Into<ElementId>,
    title: &str,
    size_px: f32,
    palette: &DesktopPalette,
    is_active_playing: bool,
    on_play: Option<ActionCallback>,
) -> AnyElement {
    let radius = palette.radii.art(size_px);
    let elem_id: ElementId = id.into();
    let group_name: SharedString = format!("mood-{elem_id:?}").into();
    let show_overlay = on_play.is_some();
    let (c1, c2, icon) = mood_gradient_and_icon(title);

    div()
        .id(elem_id)
        .group(group_name.clone())
        .relative()
        .flex_shrink_0()
        .size(px(size_px))
        .rounded(px(radius))
        .overflow_hidden()
        .bg(linear_gradient(
            135.,
            linear_color_stop(c1, 0.0),
            linear_color_stop(c2, 1.0),
        ))
        .child(
            // Soft diagonal glossy highlight sheen
            div()
                .absolute()
                .inset_0()
                .rounded(px(radius))
                .bg(linear_gradient(
                    155.,
                    linear_color_stop(gpui_kit::white().opacity(0.22), 0.0),
                    linear_color_stop(gpui_kit::black().opacity(0.24), 1.0),
                )),
        )
        .child(
            // Subtle decorative geometric accent ring in bottom-right corner
            div()
                .absolute()
                .right(px(-size_px * 0.16))
                .bottom(px(-size_px * 0.16))
                .size(px(size_px * 0.62))
                .rounded_full()
                .border_2()
                .border_color(gpui_kit::white().opacity(0.16)),
        )
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(gpui_kit::white().opacity(0.92))
                .child(Icon::new(icon).size(px((size_px * 0.34).clamp(22.0, 48.0)))),
        )
        .when(show_overlay, |d| {
            let icon = if is_active_playing {
                IconName::Pause
            } else {
                IconName::Play
            };
            let btn_size = (size_px * 0.34).clamp(24.0, 44.0);
            let on_play_cb = on_play.clone();
            d.child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .rounded(px(radius))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::black().opacity(0.42))
                    .opacity(if is_active_playing { 1.0 } else { 0.0 })
                    .group_hover(group_name, |s| s.opacity(1.0))
                    .when_some(on_play_cb, |el, cb| {
                        el.cursor_pointer().on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            move |_, window, cx| {
                                cx.stop_propagation();
                                cb(window, cx);
                            },
                        )
                    })
                    .child(
                        div()
                            .size(px(btn_size))
                            .rounded_full()
                            .bg(palette.accent)
                            .text_color(palette.on_accent)
                            .shadow_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(icon).size(px(btn_size * 0.48))),
                    ),
            )
        })
        .into_any_element()
}

fn mood_gradient_and_icon(title: &str) -> (Hsla, Hsla, IconName) {
    let lower = title.to_ascii_lowercase();
    if lower.contains("energize") || lower.contains("workout") || lower.contains("gym") {
        (
            hsla(0.05, 0.84, 0.46, 1.0),
            hsla(0.95, 0.78, 0.34, 1.0),
            IconName::Volume2,
        )
    } else if lower.contains("feel good") || lower.contains("happy") || lower.contains("summer") {
        (
            hsla(0.11, 0.86, 0.48, 1.0),
            hsla(0.42, 0.70, 0.34, 1.0),
            IconName::Music2,
        )
    } else if lower.contains("relax") || lower.contains("chill") || lower.contains("ambient") {
        (
            hsla(0.52, 0.68, 0.40, 1.0),
            hsla(0.62, 0.65, 0.26, 1.0),
            IconName::Compass,
        )
    } else if lower.contains("party") || lower.contains("dance") || lower.contains("club") {
        (
            hsla(0.84, 0.80, 0.48, 1.0),
            hsla(0.72, 0.76, 0.30, 1.0),
            IconName::Disc3,
        )
    } else if lower.contains("sad") || lower.contains("melanch") || lower.contains("blues") {
        (
            hsla(0.60, 0.48, 0.36, 1.0),
            hsla(0.66, 0.46, 0.20, 1.0),
            IconName::MicVocal,
        )
    } else if lower.contains("focus") || lower.contains("study") || lower.contains("work") {
        (
            hsla(0.45, 0.62, 0.38, 1.0),
            hsla(0.56, 0.64, 0.22, 1.0),
            IconName::Library,
        )
    } else if lower.contains("romance") || lower.contains("love") {
        (
            hsla(0.94, 0.74, 0.46, 1.0),
            hsla(0.82, 0.64, 0.28, 1.0),
            IconName::Heart,
        )
    } else if lower.contains("sleep") || lower.contains("night") {
        (
            hsla(0.68, 0.56, 0.32, 1.0),
            hsla(0.74, 0.52, 0.16, 1.0),
            IconName::Clock,
        )
    } else if lower.contains("commute") || lower.contains("drive") || lower.contains("travel") {
        (
            hsla(0.08, 0.76, 0.44, 1.0),
            hsla(0.58, 0.54, 0.28, 1.0),
            IconName::Radio,
        )
    } else if lower.contains("new release") {
        (
            hsla(0.75, 0.76, 0.48, 1.0),
            hsla(0.54, 0.74, 0.34, 1.0),
            IconName::Disc3,
        )
    } else if lower.contains("chart") || lower.contains("top") || lower.contains("trending") {
        (
            hsla(0.09, 0.85, 0.46, 1.0),
            hsla(0.97, 0.78, 0.34, 1.0),
            IconName::ListMusic,
        )
    } else if lower.contains("mood") || lower.contains("genre") {
        (
            hsla(0.56, 0.72, 0.44, 1.0),
            hsla(0.78, 0.70, 0.34, 1.0),
            IconName::Compass,
        )
    } else {
        // Deterministic rich hue from title hash
        let mut h: u32 = 2166136261;
        for b in title.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        let hue1 = (h % 360) as f32 / 360.0;
        let hue2 = (hue1 + 0.12) % 1.0;
        (
            hsla(hue1, 0.66, 0.42, 1.0),
            hsla(hue2, 0.64, 0.24, 1.0),
            IconName::Music2,
        )
    }
}

/// Renders a 16:9 widescreen video thumbnail with hover `play_overlay`.
pub fn wide_thumb_tile(
    id: impl Into<ElementId>,
    media: Option<&CachedMedia>,
    width_px: f32,
    height_px: f32,
    palette: &DesktopPalette,
    is_active_playing: bool,
    on_play: Option<ActionCallback>,
) -> AnyElement {
    let radius = palette.radii.art(height_px);
    let elem_id: ElementId = id.into();
    let group_name: SharedString = format!("wide-{elem_id:?}").into();
    let show_overlay = on_play.is_some();

    div()
        .id(elem_id)
        .group(group_name.clone())
        .relative()
        .flex_shrink_0()
        .w(px(width_px))
        .h(px(height_px))
        .rounded(px(radius))
        .bg(palette.surface)
        .overflow_hidden()
        .child(if let Some(m) = media {
            img(m.image.clone())
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        } else {
            div()
                .size_full()
                .rounded(px(radius))
                .flex()
                .items_center()
                .justify_center()
                .text_color(palette.dim)
                .child(Icon::new(IconName::Play).size(px(28.0)))
                .into_any_element()
        })
        .when(show_overlay, |d| {
            let icon = if is_active_playing {
                IconName::Pause
            } else {
                IconName::Play
            };
            d.child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .rounded(px(radius))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::black().opacity(0.40))
                    .opacity(if is_active_playing { 1.0 } else { 0.0 })
                    .group_hover(group_name, |s| s.opacity(1.0))
                    .when_some(on_play, |el, cb| {
                        el.cursor_pointer().on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            move |_, window, cx| {
                                cx.stop_propagation();
                                cb(window, cx);
                            },
                        )
                    })
                    .child(
                        div()
                            .size(px(40.0))
                            .rounded_full()
                            .bg(palette.accent)
                            .text_color(palette.on_accent)
                            .shadow_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(icon).size(px(18.0))),
                    ),
            )
        })
        .into_any_element()
}

/// Top scrolling `header_wash` blending the cover's dominant color and vertically alpha-faded blurred backdrop seamlessly into the page.
pub fn header_wash(
    media: Option<&CachedMedia>,
    palette: &DesktopPalette,
    adaptive: bool,
    height_px: f32,
) -> Div {
    let mut wrapper = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(height_px))
        .overflow_hidden();

    if !adaptive {
        return wrapper;
    }

    let tint = media
        .map(|m| palette.washed_surface(m.dominant_rgb, 0.50))
        .unwrap_or_else(|| palette.surface.opacity(0.24));

    if let Some(m) = media {
        wrapper = wrapper.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(height_px))
                .opacity(0.30)
                .child(
                    img(m.header_blurred.clone())
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                ),
        );
    }

    wrapper.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(height_px))
            .bg(linear_gradient(
                180.,
                linear_color_stop(tint.opacity(0.36), 0.0),
                linear_color_stop(tint.opacity(0.0), 1.0),
            )),
    )
}

/// Translucent edge fade strips shown along top/bottom of a scrolled container when `scroll_fades` is enabled.
pub fn scroll_edge_fades(
    handle: &ScrollHandle,
    palette: &DesktopPalette,
    enabled: bool,
) -> AnyElement {
    if !enabled {
        return div().into_any_element();
    }
    let off = handle.offset();
    let max_off = handle.max_offset();
    let scrolled_down = off.y < px(-4.0);
    let can_scroll_down = max_off.y > px(4.0) && off.y > -(max_off.y - px(4.0));
    let edge_color = palette
        .card_bg
        .opacity((palette.solved.card_alpha * 0.92).max(0.45));

    div()
        .absolute()
        .inset_0()
        .when(scrolled_down, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(18.0))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(edge_color, 0.0),
                        linear_color_stop(edge_color.opacity(0.0), 1.0),
                    )),
            )
        })
        .when(can_scroll_down, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(22.0))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(edge_color.opacity(0.0), 0.0),
                        linear_color_stop(edge_color, 1.0),
                    )),
            )
        })
        .into_any_element()
}

/// Animated pulsing ring around the Play button while buffering a track stream.
pub fn loading_ring(
    id: impl Into<ElementId>,
    size_px: f32,
    palette: &DesktopPalette,
) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .inset(px(-3.0))
        .size(px(size_px + 6.0))
        .rounded_full()
        .border_2()
        .border_color(palette.accent.opacity(0.85))
        .with_animation(
            "play-loading-ring",
            Animation::new(Duration::from_millis(850)).repeat(),
            move |el, delta| {
                let pulse = 0.25 + 0.75 * (1.0 - (delta * 2.0 - 1.0).abs());
                el.opacity(pulse)
            },
        )
        .into_any_element()
}

/// Single-line semibold title that activates an animated horizontal marquee scroll only while hovered when text overflows `max_chars`.
pub fn hover_marquee(
    id: impl Into<ElementId>,
    text: &str,
    max_chars: usize,
    color: Hsla,
    size_px: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let weight = FontWeight::SEMIBOLD;
    let len = text.chars().count();
    let label: SharedString = text.to_string().into();
    if len <= max_chars {
        return div()
            .text_size(px(size_px))
            .font_weight(weight)
            .text_color(color)
            .truncate()
            .child(label)
            .into_any_element();
    }
    let elem_id: ElementId = id.into();
    let hover_state = window.use_keyed_state(
        SharedString::from(format!("mq-state-{elem_id:?}")),
        cx,
        |_, _| false,
    );
    let is_hovered = *hover_state.read(cx);
    let shift = ((len.saturating_sub(max_chars)) as f32 * (size_px * 0.45)).min(180.0);

    let hover_writer = hover_state.clone();
    div()
        .id(elem_id)
        .overflow_hidden()
        .on_hover(move |hovered, _, cx| {
            hover_writer.update(cx, |s, cx| {
                if *s != *hovered {
                    *s = *hovered;
                    cx.notify();
                }
            });
        })
        .child(if is_hovered {
            div()
                .text_size(px(size_px))
                .font_weight(weight)
                .text_color(color)
                .whitespace_nowrap()
                .child(label)
                .with_animation(
                    "marquee-scroll",
                    Animation::new(Duration::from_secs(5)).repeat(),
                    move |el, delta| {
                        let phase = if delta < 0.15 {
                            0.0
                        } else if delta < 0.80 {
                            (delta - 0.15) / 0.65
                        } else {
                            1.0 - (delta - 0.80) / 0.20
                        };
                        el.ml(px(-shift * phase))
                    },
                )
                .into_any_element()
        } else {
            div()
                .text_size(px(size_px))
                .font_weight(weight)
                .text_color(color)
                .truncate()
                .child(label)
                .into_any_element()
        })
        .into_any_element()
}

pub fn artist_links_row(
    id_prefix: &str,
    artists_raw: &str,
    text_color: Hsla,
    size_px: f32,
    on_artist: impl Fn(String, &mut Window, &mut App) + Clone + 'static,
) -> AnyElement {
    let names = split_artist_credits(artists_raw);
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .flex_wrap()
        .gap_x(px(4.0))
        .text_size(px(size_px))
        .text_color(text_color);

    let total = names.len();
    for (idx, name) in names.into_iter().enumerate() {
        let artist_click = name.clone();
        let cb = on_artist.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("{id_prefix}-art-{idx}")))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_mouse_down(gpui_kit::MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    cb(artist_click.clone(), window, cx);
                })
                .child(name),
        );
        if idx + 1 < total {
            row = row.child(div().text_color(text_color.opacity(0.6)).child(","));
        }
    }
    row.into_any_element()
}

pub fn explicit_badge(palette: &DesktopPalette) -> AnyElement {
    div()
        .flex_shrink_0()
        .px(px(4.0))
        .py(px(1.0))
        .rounded(px(palette.radii.small))
        .bg(palette.surface_active)
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .text_color(palette.secondary)
        .child("E")
        .into_any_element()
}

pub fn quality_badge(label: &str, upgrading: bool, palette: &DesktopPalette) -> AnyElement {
    let text: SharedString = if upgrading {
        "Upgrading...".into()
    } else {
        label.to_string().into()
    };
    div()
        .flex_shrink_0()
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(palette.radii.control))
        .bg(palette.surface)
        .border_1()
        .border_color(palette.outline)
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if upgrading {
            palette.accent
        } else {
            palette.secondary
        })
        .child(text)
        .into_any_element()
}

pub fn section_header(
    id_prefix: &str,
    title: &str,
    subtitle: Option<&str>,
    on_show_all: Option<ActionCallback>,
    palette: &DesktopPalette,
) -> Div {
    let mut bar = div()
        .w_full()
        .flex()
        .flex_row()
        .items_end()
        .justify_between()
        .mb(px(12.0));

    let mut left = div().flex().flex_col().gap(px(2.0));
    if let Some(sub) = subtitle.filter(|s| !s.is_empty()) {
        left = left.child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.dim)
                .child(sub.to_string()),
        );
    }
    left = left.child(
        div()
            .text_size(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(palette.text)
            .child(title.to_string()),
    );
    bar = bar.child(left);

    if let Some(cb) = on_show_all {
        bar = bar.child(
            Button::new(SharedString::from(format!("{id_prefix}-show-all")))
                .ghost()
                .small()
                .label("Show all")
                .on_click(move |_, window, cx| cb(window, cx)),
        );
    }
    bar
}

pub fn icon_action_button(
    id: impl Into<ElementId>,
    icon: IconName,
    active: bool,
    tooltip: Option<&'static str>,
    palette: &DesktopPalette,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut btn = Button::new(id).ghost().small().icon(icon);
    if let Some(tip) = tooltip {
        btn = btn.tooltip(tip);
    }
    btn.when(active, |b| b.text_color(palette.accent))
        .on_click(move |_, window, cx| on_click(window, cx))
        .into_any_element()
}

/// Visual sizing parameters for [`glowing_lyric_row`].
pub struct LyricRowStyle {
    pub base_font_size: f32,
    pub active_font_size: f32,
    pub base_line_height: f32,
    pub active_line_height: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    pub radius: f32,
    pub hover_bg: Hsla,
    pub pure_white_glow: bool,
}

/// Returns true when a lyric line is a parenthetical background vocal / ad-lib like `(yeah)`.
pub fn is_parenthetical_bg(text: &str) -> bool {
    let t = text.trim();
    t.len() >= 3 && t.starts_with('(') && t.ends_with(')')
}

/// Resolves the active lead lyric index alongside any currently overlapping parenthetical `(...)` background vocal.
pub fn resolve_active_lyric_pair(
    lines: &[LyricLine],
    position_secs: f64,
) -> (Option<usize>, Option<usize>) {
    let lead = active_line_index(lines, position_secs);
    let Some(lead_idx) = lead else {
        return (None, None);
    };
    let lookahead = position_secs + 0.08;
    // Check immediate following parenthetical background vocal(s) that started while lead is still sung.
    let mut bg = None;
    for cand in (lead_idx + 1)..lines.len().min(lead_idx + 3) {
        let line = &lines[cand];
        if !is_parenthetical_bg(&line.text) {
            break;
        }
        if line.time_secs <= lookahead {
            let end_est = lines
                .get(cand + 1)
                .map(|n| n.time_secs)
                .unwrap_or(line.time_secs + 3.2)
                .min(line.time_secs + 4.0);
            if position_secs <= end_est + 0.15 {
                bg = Some(cand);
            }
        }
    }
    (Some(lead_idx), bg)
}

/// Derives an opaque deep album-art backdrop canvas, atmospheric top wash, and vibrant album accent from `rgb`.
pub fn album_art_theme_colors(rgb: (u8, u8, u8)) -> (Hsla, Hsla, Hsla) {
    let r = rgb.0 as f32 / 255.0;
    let g = rgb.1 as f32 / 255.0;
    let b = rgb.2 as f32 / 255.0;
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let delta = max_c - min_c;
    let h = if delta < 1e-4 {
        0.65
    } else if (max_c - r).abs() < 1e-4 {
        (((g - b) / delta) % 6.0) / 6.0
    } else if (max_c - g).abs() < 1e-4 {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let h = if h < 0.0 { h + 1.0 } else { h };
    let l = (max_c + min_c) * 0.5;
    let s = if delta < 1e-4 {
        0.18
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs()).max(0.01)
    };

    let deep_canvas = hsla(h, (s * 0.58).clamp(0.16, 0.52), 0.09, 1.0);
    let top_wash = hsla(h, (s * 0.72).clamp(0.22, 0.68), 0.24, 0.78);
    let accent = hsla(h, s.clamp(0.42, 0.85), 0.64, 1.0);
    (deep_canvas, top_wash, accent)
}

/// Renders one synced lyric sentence row with pure white soft glow, smooth `glow` (`0..=1`) transition,
/// left/right duet alignment (`Lead` vs `Counter`), and subtle parenthetical `(...)` background vocal styling.
#[allow(clippy::too_many_arguments)]
pub fn glowing_lyric_row(
    id: impl Into<ElementId>,
    raw_text: &str,
    voice: LyricVoice,
    glow: f32,
    is_past: bool,
    is_background: bool,
    style: LyricRowStyle,
    palette: &DesktopPalette,
) -> Stateful<Div> {
    let g = glow.clamp(0.0, 1.0);
    let text = if raw_text.trim().is_empty() {
        "• • •".to_string()
    } else {
        raw_text.to_string()
    };

    let bg_scale = if is_background { 0.78 } else { 1.0 };
    let font_size =
        (style.base_font_size + (style.active_font_size - style.base_font_size) * g) * bg_scale;
    let line_height = (style.base_line_height
        + (style.active_line_height - style.base_line_height) * g)
        * bg_scale;

    let white_base = if style.pure_white_glow || palette.is_dark {
        gpui_kit::white()
    } else {
        palette.text
    };
    let rest_alpha = if is_background {
        if is_past {
            0.30
        } else {
            0.42
        }
    } else if is_past {
        0.34
    } else {
        0.54
    };
    let active_alpha = if is_background { 0.88 } else { 1.0 };
    let text_alpha = rest_alpha + (active_alpha - rest_alpha) * g;
    let is_counter = voice == LyricVoice::Counter;

    let mut row = div()
        .id(id)
        .w_full()
        .cursor_pointer()
        .rounded(px(style.radius))
        .px(px(style.pad_x))
        .py(px(if is_background {
            (style.pad_y * 0.55).max(2.0)
        } else {
            style.pad_y
        }))
        .flex()
        .flex_row()
        .when(!is_counter, |el| el.justify_start())
        .when(is_counter, |el| el.justify_end())
        .hover(move |s| s.bg(style.hover_bg));

    let mut inner = div()
        .max_w_full()
        .px(px(if g > 0.02 { 8.0 } else { 0.0 }))
        .py(px(if g > 0.02 { 3.0 } else { 0.0 }))
        .rounded(px(style.radius))
        .when(g > 0.02, |el| {
            let aura = white_base.opacity((if is_background { 0.08 } else { 0.14 }) * g);
            el.bg(white_base.opacity((if is_background { 0.03 } else { 0.06 }) * g))
                .shadow(vec![BoxShadow {
                    color: aura,
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(20.0),
                    spread_radius: px(0.5),
                    inset: false,
                }])
        })
        .text_size(px(font_size))
        .line_height(px(line_height))
        .font_weight(if g > 0.45 && !is_background {
            FontWeight::BOLD
        } else {
            FontWeight::MEDIUM
        })
        .text_color(white_base.opacity(text_alpha));

    if is_counter {
        inner = inner.text_right();
    } else {
        inner = inner.text_left();
    }
    if is_background {
        inner = inner.italic();
    }

    row = row.child(inner.child(text));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_artist_credits_handles_multiple_collaborators() {
        assert_eq!(
            split_artist_credits("Daft Punk, Pharrell Williams & Nile Rodgers"),
            vec!["Daft Punk", "Pharrell Williams", "Nile Rodgers"]
        );
    }

    #[test]
    fn fit_title_size_steps_down_for_long_titles() {
        assert!(
            fit_title_size("Short")
                > fit_title_size("A Rather Much Longer Deluxe Album Edition Title")
        );
    }
}
