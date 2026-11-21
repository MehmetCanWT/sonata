//! Left navigation sidebar and YouTube Music playlists list.
//!
//! 1. **Skin-Native Shell**: `render_sidebar` renders an Apple Music / Sonora full-height sidebar (`Sonata`), a `#121212` rounded "Your Library" card (`Spotify`), or a hairline-divided `#030303` rail (`YouTubeMusic`).
//! 2. **Playlists**: lists synced YouTube Music playlists with skin-authentic row styling, a create-playlist trigger, or a sign-in prompt when logged out.

use super::super::{ui_icon, CenterMode, FluentDivExt, SonataDesktopView};
use crate::backend::NavTarget;
use crate::theme_bridge::{rgb_hsla, DesktopSkin, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, ScrollStrategy, SharedString,
    StatefulInteractiveElement, Styled,
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

    let mut shell = match p.skin {
        DesktopSkin::Sonata => div()
            .w(px(236.0))
            .flex_shrink_0()
            .h_full()
            .bg(p.sidebar_bg)
            .border_r_1()
            .border_color(p.divider)
            .px_3()
            .pb_3()
            .flex()
            .flex_col()
            .gap_3(),
        DesktopSkin::Spotify => div()
            .w(px(264.0))
            .flex_shrink_0()
            .h_full()
            .rounded_xl()
            .bg(p.sidebar_bg)
            .p_3()
            .flex()
            .flex_col()
            .gap_3(),
        DesktopSkin::YouTubeMusic => div()
            .w(px(240.0))
            .flex_shrink_0()
            .h_full()
            .bg(p.sidebar_bg)
            .border_r_1()
            .border_color(p.divider)
            .p_3()
            .flex()
            .flex_col()
            .gap_3(),
    };

    // Sonata full-height sidebar owns the top-left brand row (matching Apple Music / macOS).
    if p.skin == DesktopSkin::Sonata {
        shell = shell.child(
            div()
                .h(px(56.0))
                .px_1()
                .flex()
                .items_center()
                .on_mouse_down(MouseButton::Left, |_, window, _| {
                    window.start_window_move();
                })
                .child(
                    div()
                        .id("hdr-brand-home")
                        .px_2()
                        .py_1()
                        .rounded_lg()
                        .hover(|s| s.bg(p.pill_bg))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigate_to(NavTarget::Home, cx);
                        }))
                        .child(
                            div()
                                .size(px(26.0))
                                .rounded_lg()
                                .bg(p.accent_soft)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(ui_icon(IconName::Disc3, 15.0, p.accent)),
                        )
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .child("Sonata"),
                        ),
                ),
        );
    }

    // Primary navigation section (Spotify keeps Home in top bar and highlights "Your Library" here).
    if p.skin == DesktopSkin::Spotify {
        shell = shell
            .child(
                div()
                    .px_2()
                    .pt_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(ui_icon(IconName::ListMusic, 18.0, p.muted))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.text)
                                    .child("Your Library"),
                            ),
                    )
                    .when(snap.is_signed_in(), |el| {
                        el.child(
                            div()
                                .id("sb-new-ytm")
                                .size(px(30.0))
                                .rounded_full()
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_create_playlist(window, cx);
                                }))
                                .child(ui_icon(IconName::Plus, 16.0, p.muted)),
                        )
                    }),
            )
            .child(
                // Compact Spotify filter pills for quick navigation
                div()
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(render_spotify_pill(
                        "sb-home",
                        "Home",
                        home_sel,
                        p,
                        |this, cx| this.navigate_to(NavTarget::Home, cx),
                        cx,
                    ))
                    .child(render_spotify_pill(
                        "sb-search",
                        "Search",
                        search_sel,
                        p,
                        |this, cx| this.navigate_to(NavTarget::Search, cx),
                        cx,
                    ))
                    .child(render_spotify_pill(
                        "sb-queue",
                        "Queue",
                        queue_sel,
                        p,
                        |this, cx| this.navigate_to(NavTarget::Queue, cx),
                        cx,
                    )),
            );
    } else {
        shell = shell.child(
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
                    "Explore & Search".to_string(),
                    IconName::Compass,
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
        );

        // Divider + YouTube Music "New playlist" pill or Sonata "Playlists" header
        shell = shell.child(div().w_full().h(px(1.0)).bg(p.divider));

        if p.skin == DesktopSkin::YouTubeMusic && snap.is_signed_in() {
            shell = shell.child(
                div()
                    .id("sb-new-ytm")
                    .w_full()
                    .h(px(36.0))
                    .px_4()
                    .rounded_full()
                    .bg(p.search_bg)
                    .hover(|s| s.bg(p.card_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_create_playlist(window, cx);
                    }))
                    .child(ui_icon(IconName::Plus, 15.0, p.highlight_fg))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.highlight_fg)
                            .child("New playlist"),
                    ),
            );
        }
    }

    // Scrollable playlists list
    let mut list = div()
        .id("sidebar-playlists-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(2.0));

    if p.skin == DesktopSkin::Sonata {
        list = list.child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.muted)
                        .child("Playlists"),
                )
                .when(snap.is_signed_in(), |el| {
                    el.child(
                        div()
                            .id("sb-new-ytm")
                            .size(px(22.0))
                            .rounded_md()
                            .hover(|s| s.bg(p.pill_bg))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_playlist(window, cx);
                            }))
                            .child(ui_icon(IconName::Plus, 13.0, p.muted)),
                    )
                }),
        );
    }

    list = list.children(if !snap.is_signed_in() {
        vec![div()
            .p_3()
            .mt_1()
            .rounded_xl()
            .bg(p.card_bg)
            .border_1()
            .border_color(p.border_subtle)
            .flex()
            .flex_col()
            .gap_3()
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
                    .py(px(7.0))
                    .rounded_full()
                    .bg(p.cta_bg)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigate_to(NavTarget::Settings, cx);
                    }))
                    .child(ui_icon(IconName::LogIn, 13.0, p.cta_fg))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.cta_fg)
                            .child("Sign in"),
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
                let active =
                    is_page && matches!(&snap.active_nav, NavTarget::Playlist(id) if id == &pl_id);
                let is_liked = pl_id == "VLLM";

                if p.skin == DesktopSkin::Spotify {
                    // Authentic Spotify Library row: 42px square icon tile + Title + "Playlist" subtitle.
                    let tile = if is_liked {
                        div()
                            .size(px(40.0))
                            .flex_shrink_0()
                            .rounded_md()
                            .bg(linear_gradient(
                                135.0,
                                linear_color_stop(rgb_hsla((0x45, 0x0a, 0xf5), 1.0), 0.0),
                                linear_color_stop(rgb_hsla((0xc4, 0xef, 0xd9), 1.0), 1.0),
                            ))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui_icon(
                                IconName::Heart,
                                16.0,
                                rgb_hsla((255, 255, 255), 1.0),
                            ))
                    } else {
                        div()
                            .size(px(40.0))
                            .flex_shrink_0()
                            .rounded_md()
                            .bg(p.search_bg)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui_icon(
                                IconName::Music2,
                                16.0,
                                if active { p.accent } else { p.muted },
                            ))
                    };

                    div()
                        .id(SharedString::from(format!("sb-ytm-pl-{idx}")))
                        .w_full()
                        .px_2()
                        .py(px(6.0))
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
                        .gap_3()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigate_to(NavTarget::Playlist(pl_id.clone()), cx);
                            this.track_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        }))
                        .child(tile)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if active { p.accent } else { p.highlight_fg })
                                        .truncate()
                                        .child(pl.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(if is_liked { "Liked Music" } else { "Playlist" }),
                                ),
                        )
                        .into_any_element()
                } else {
                    let icon = if is_liked {
                        IconName::Heart
                    } else {
                        IconName::Music2
                    };
                    div()
                        .id(SharedString::from(format!("sb-ytm-pl-{idx}")))
                        .w_full()
                        .px_2()
                        .py(px(8.0))
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
                        .gap_3()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigate_to(NavTarget::Playlist(pl_id.clone()), cx);
                            this.track_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        }))
                        .child(ui_icon(
                            icon,
                            15.0,
                            if active || is_liked {
                                p.accent
                            } else {
                                p.muted
                            },
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
                }
            })
            .collect()
    });

    shell.child(list).into_any_element()
}

fn render_spotify_pill(
    id: &'static str,
    label: &'static str,
    selected: bool,
    p: &GlassPalette,
    on_click: impl Fn(&mut SonataDesktopView, &mut Context<SonataDesktopView>) + 'static,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .id(id)
        .px_3()
        .py(px(5.0))
        .rounded_full()
        .bg(if selected { p.play_btn_bg } else { p.search_bg })
        .hover(|s| {
            s.bg(if selected {
                p.play_btn_bg
            } else {
                p.card_hover
            })
        })
        .cursor_pointer()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if selected { p.play_btn_fg } else { p.text })
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        .child(label)
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
        .px_3()
        .py(px(9.0))
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
        .gap_3()
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        .child(ui_icon(
            icon,
            16.0,
            if selected {
                if p.skin == DesktopSkin::YouTubeMusic {
                    p.highlight_fg
                } else {
                    p.accent
                }
            } else {
                p.muted
            },
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if selected { p.highlight_fg } else { p.text })
                .truncate()
                .child(label),
        )
        .into_any_element()
}
