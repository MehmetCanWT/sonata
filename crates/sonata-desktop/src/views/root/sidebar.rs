//! Left navigation sidebar and YouTube Music playlists list.
//!
//! 1. **Navigation**: `render_sidebar` builds Home, Search, and Queue entries via `render_nav_button`.
//! 2. **Playlists**: lists synced YouTube Music playlists with a create-playlist button or sign-in prompt when logged out.

use super::super::{ui_icon, CenterMode, FluentDivExt, SonataDesktopView};
use crate::backend::NavTarget;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled,
};

pub(super) fn render_sidebar(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let is_page = view.center_mode == CenterMode::Page;
    let home_sel = is_page && snap.active_nav == NavTarget::Home;
    let search_sel = is_page && snap.active_nav == NavTarget::Search;
    let queue_sel = is_page && snap.active_nav == NavTarget::Queue;

    div()
        .w(px(236.0))
        .flex_shrink_0()
        .h_full()
        .rounded_2xl()
        .bg(p.sidebar_bg)
        .border_1()
        .border_color(p.border_subtle)
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(render_nav_button(
                    "sb-home",
                    "Home".to_string(),
                    IconName::House,
                    home_sel,
                    p,
                    |this, cx| this.navigate_to(NavTarget::Home, cx),
                    cx,
                ))
                .child(render_nav_button(
                    "sb-search",
                    "Search".to_string(),
                    IconName::Search,
                    search_sel,
                    p,
                    |this, cx| this.navigate_to(NavTarget::Search, cx),
                    cx,
                ))
                .child(render_nav_button(
                    "sb-queue",
                    format!("Queue ({})", snap.queue.len()),
                    IconName::ListMusic,
                    queue_sel,
                    p,
                    |this, cx| this.navigate_to(NavTarget::Queue, cx),
                    cx,
                )),
        )
        .child(
            div()
                .id("sidebar-playlists-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.muted)
                                .child("YOUTUBE MUSIC"),
                        )
                        .when(snap.is_signed_in(), |el| {
                            el.child(
                                div()
                                    .id("sb-new-ytm")
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(p.pill_bg)
                                    .hover(|s| s.bg(p.card_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.0))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_create_playlist(window, cx);
                                    }))
                                    .child(ui_icon(IconName::Plus, 11.0, p.accent))
                                    .child(div().text_xs().text_color(p.text).child("New")),
                            )
                        }),
                )
                .children(if !snap.is_signed_in() {
                    vec![div()
                        .p_3()
                        .mt_1()
                        .rounded_xl()
                        .bg(p.card_bg)
                        .border_1()
                        .border_color(p.border_subtle)
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .child("Sign in to sync your Liked Music and playlists."),
                        )
                        .child(
                            div()
                                .id("sb-signin-prompt")
                                .px_3()
                                .py(px(6.0))
                                .rounded_lg()
                                .bg(p.accent_soft)
                                .border_1()
                                .border_color(p.accent)
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_1()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_to(NavTarget::Settings, cx);
                                }))
                                .child(ui_icon(IconName::LogIn, 12.0, p.accent))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.accent)
                                        .child("Sign In"),
                                ),
                        )
                        .into_any_element()]
                } else if snap.ytm_playlists.is_empty() {
                    vec![div()
                        .px_2()
                        .py_2()
                        .text_xs()
                        .text_color(p.muted)
                        .child("Syncing playlists...")
                        .into_any_element()]
                } else {
                    snap.ytm_playlists
                        .iter()
                        .enumerate()
                        .map(|(idx, pl)| {
                            let pl_id = pl.id.clone();
                            let active = is_page
                                && matches!(&snap.active_nav, NavTarget::Playlist(id) if id == &pl_id);
                            let icon = if pl_id == "VLLM" {
                                IconName::Heart
                            } else {
                                IconName::Music2
                            };
                            div()
                                .id(SharedString::from(format!("sb-ytm-pl-{idx}")))
                                .w_full()
                                .px_2()
                                .py(px(7.0))
                                .rounded_lg()
                                .bg(if active {
                                    p.highlight_bg
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.navigate_to(NavTarget::Playlist(pl_id.clone()), cx);
                                    this.track_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                }))
                                .child(ui_icon(
                                    icon,
                                    14.0,
                                    if active { p.accent } else { p.error },
                                ))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .font_weight(if active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if active { p.highlight_fg } else { p.text })
                                        .truncate()
                                        .child(pl.name.clone()),
                                )
                                .into_any_element()
                        })
                        .collect()
                }),
        )
        .into_any_element()
}

fn render_nav_button(
    id: &'static str,
    label: String,
    icon: IconName,
    selected: bool,
    p: &GlassPalette,
    on_click: impl Fn(&mut SonataDesktopView, &mut Context<SonataDesktopView>) + 'static,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .px_2()
        .py(px(7.0))
        .rounded_lg()
        .bg(if selected {
            p.highlight_bg
        } else {
            gpui_kit::transparent_black()
        })
        .hover(|s| s.bg(p.card_hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        .child(ui_icon(
            icon,
            14.0,
            if selected { p.accent } else { p.muted },
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if selected { p.highlight_fg } else { p.text })
                .truncate()
                .child(label),
        )
        .into_any_element()
}
