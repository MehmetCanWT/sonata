use super::{
    artist, cached_thumb_path, fullscreen, home, render_avatar, render_cover_art,
    render_icon_button, render_progress_slider, render_track_row, render_volume_slider, search,
    settings, ui_icon, CenterMode, FluentDivExt, SonataDesktopView,
};
use crate::backend::{BackendCmd, NavTarget};
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, uniform_list, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, ScrollStrategy, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use sonata_core::lyrics::LyricsState;
use sonata_core::player::RepeatMode;
use std::ops::Range;
use std::sync::Arc;

pub fn render_full_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    if view.center_mode == CenterMode::Fullscreen {
        return fullscreen::render_fullscreen_workspace(view, p, window, cx);
    }

    div()
        .size_full()
        .bg(p.window_bg)
        .child(
            div()
                .size_full()
                .bg(linear_gradient(
                    155.0,
                    linear_color_stop(p.ambient_top, 0.0),
                    linear_color_stop(p.window_bg, 0.72),
                ))
                .flex()
                .flex_col()
                .child(render_top_header(view, p, cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px_3()
                        .pb_3()
                        .flex()
                        .gap_3()
                        .child(render_sidebar(view, p, cx))
                        .child(render_center_stage(view, p, cx))
                        .when(view.show_queue, |el| {
                            el.child(render_queue_drawer(view, p, cx))
                        }),
                )
                .child(render_player_bar(view, p, cx)),
        )
        .into_any_element()
}

fn render_top_header(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let is_lyrics = view.center_mode == CenterMode::Lyrics;
    let is_settings =
        view.center_mode == CenterMode::Page && snap.active_nav == NavTarget::Settings;

    let account_btn = if snap.is_signed_in() {
        let name = snap
            .user_profile
            .as_ref()
            .map(|u| u.name.clone())
            .unwrap_or_else(|| "Account".to_string());
        div()
            .id("hdr-account-btn")
            .h(px(34.0))
            .pl(px(5.0))
            .pr(px(10.0))
            .rounded_full()
            .bg(if view.account_menu_open {
                p.card_hover
            } else {
                p.panel_bg
            })
            .border_1()
            .border_color(if view.account_menu_open {
                p.accent
            } else {
                p.border_subtle
            })
            .hover(|s| s.bg(p.card_hover))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| {
                this.account_menu_open = !this.account_menu_open;
                cx.notify();
            }))
            .child(render_avatar(
                snap.user_avatar_path.as_ref(),
                &name,
                24.0,
                p,
            ))
            .child(
                div()
                    .max_w(px(120.0))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.highlight_fg)
                    .truncate()
                    .child(name),
            )
            .child(ui_icon(IconName::ChevronDown, 13.0, p.muted))
            .into_any_element()
    } else {
        div()
            .id("hdr-signin-btn")
            .h(px(34.0))
            .px_3()
            .rounded_full()
            .bg(p.accent)
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(6.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| {
                this.navigate_to(NavTarget::Settings, cx);
            }))
            .child(ui_icon(IconName::LogIn, 14.0, p.accent_fg))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.accent_fg)
                    .child("Sign In"),
            )
            .into_any_element()
    };

    div()
        .id("top-header")
        .w_full()
        .h(px(58.0))
        .px_4()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_move();
        })
        .child(
            // Sonata Brand -> clicks Home
            div().w(px(236.0)).flex().items_center().gap_2().child(
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
                            .size(px(28.0))
                            .rounded_lg()
                            .bg(p.accent_soft)
                            .border_1()
                            .border_color(p.border_subtle)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui_icon(IconName::Disc3, 16.0, p.accent)),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.highlight_fg)
                            .child("Sonata"),
                    ),
            ),
        )
        .child(
            // Center Search Pill
            div()
                .flex_1()
                .max_w(px(460.0))
                .h(px(36.0))
                .px_3()
                .rounded_full()
                .bg(p.panel_bg)
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .items_center()
                .gap_2()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(ui_icon(IconName::Search, 15.0, p.muted))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&view.search_input).appearance(false)),
                ),
        )
        .child(
            // Top-Right Tools + Settings + User Account
            div()
                .flex()
                .items_center()
                .gap_2()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(render_icon_button(
                    "hdr-lyrics",
                    IconName::MicVocal,
                    15.0,
                    32.0,
                    if is_lyrics { p.accent } else { p.text },
                    if is_lyrics { p.accent_soft } else { p.panel_bg },
                    p.card_hover,
                    |this, _, cx| this.toggle_lyrics_view(cx),
                    cx,
                ))
                .child(render_icon_button(
                    "hdr-queue",
                    IconName::ListMusic,
                    15.0,
                    32.0,
                    if view.show_queue { p.accent } else { p.text },
                    if view.show_queue {
                        p.accent_soft
                    } else {
                        p.panel_bg
                    },
                    p.card_hover,
                    |this, _, cx| {
                        this.show_queue = !this.show_queue;
                        cx.notify();
                    },
                    cx,
                ))
                .child(render_icon_button(
                    "hdr-mini",
                    IconName::Minimize2,
                    15.0,
                    32.0,
                    p.text,
                    p.panel_bg,
                    p.card_hover,
                    |this, window, cx| this.toggle_mini_mode(window, cx),
                    cx,
                ))
                .child(render_icon_button(
                    "hdr-settings",
                    IconName::Settings,
                    15.0,
                    32.0,
                    if is_settings { p.accent } else { p.text },
                    if is_settings {
                        p.accent_soft
                    } else {
                        p.panel_bg
                    },
                    p.card_hover,
                    |this, _, cx| this.navigate_to(NavTarget::Settings, cx),
                    cx,
                ))
                .child(account_btn),
        )
        .into_any_element()
}

pub fn render_account_popover(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let name = snap
        .user_profile
        .as_ref()
        .map(|u| u.name.clone())
        .unwrap_or_else(|| "YouTube Music".to_string());
    let detail = snap
        .user_profile
        .as_ref()
        .and_then(|u| u.handle.clone().or_else(|| u.email.clone()))
        .unwrap_or_else(|| "Signed in via browser cookies".to_string());

    div()
        .id("account-popover")
        .absolute()
        .top(px(58.0))
        .right(px(16.0))
        .w(px(264.0))
        .p_3()
        .rounded_2xl()
        .bg(p.modal_bg)
        .border_1()
        .border_color(p.border)
        .flex()
        .flex_col()
        .gap_2()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .pb_2()
                .border_b_1()
                .border_color(p.border_subtle)
                .flex()
                .items_center()
                .gap_3()
                .child(render_avatar(
                    snap.user_avatar_path.as_ref(),
                    &name,
                    36.0,
                    p,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(name),
                        )
                        .child(div().text_xs().text_color(p.muted).truncate().child(detail)),
                ),
        )
        .child(popover_menu_row(
            "pop-settings",
            "Account & Settings",
            IconName::Settings,
            p.text,
            p,
            |this, cx| this.navigate_to(NavTarget::Settings, cx),
            cx,
        ))
        .child(popover_menu_row(
            "pop-refresh",
            "Refresh Library",
            IconName::RefreshCw,
            p.text,
            p,
            |this, cx| {
                this.account_menu_open = false;
                this.backend.send(BackendCmd::RefreshAccount);
                cx.notify();
            },
            cx,
        ))
        .child(popover_menu_row(
            "pop-switch",
            "Browser Settings",
            IconName::Globe,
            p.text,
            p,
            |this, cx| this.navigate_to(NavTarget::Settings, cx),
            cx,
        ))
        .child(
            div()
                .pt_1()
                .border_t_1()
                .border_color(p.border_subtle)
                .child(popover_menu_row(
                    "pop-signout",
                    "Sign Out",
                    IconName::LogOut,
                    p.error,
                    p,
                    |this, cx| {
                        this.account_menu_open = false;
                        this.backend.send(BackendCmd::SignOut);
                        cx.notify();
                    },
                    cx,
                )),
        )
        .into_any_element()
}

fn popover_menu_row(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    fg: gpui_kit::Hsla,
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
        .hover(|s| s.bg(p.card_hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        .child(ui_icon(icon, 14.0, fg))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .child(label),
        )
        .into_any_element()
}

fn render_sidebar(
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

fn render_center_stage(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let inner = match view.center_mode {
        CenterMode::Lyrics => render_lyrics_stage(view, p, cx),
        CenterMode::Fullscreen => fullscreen::render_fullscreen(view, p, cx),
        CenterMode::Page => match &view.snapshot.active_nav {
            NavTarget::Home => home::render_home_page(view, p, cx),
            NavTarget::Search => search::render_search_page(view, p, cx),
            NavTarget::Queue => render_collection_stage(view, p, true, cx),
            NavTarget::Playlist(_) => render_collection_stage(view, p, false, cx),
            NavTarget::Artist(_) => artist::render_artist_page(view, p, cx),
            NavTarget::Settings => settings::render_settings_page(view, p, cx),
        },
    };

    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .rounded_2xl()
        .bg(p.panel_bg)
        .border_1()
        .border_color(p.border_subtle)
        .overflow_hidden()
        .child(inner)
        .into_any_element()
}

fn render_collection_stage(
    view: &SonataDesktopView,
    p: &GlassPalette,
    is_queue: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let tracks_arc = if is_queue {
        Arc::new(snap.queue.clone())
    } else {
        Arc::new(snap.playlist_tracks.clone())
    };
    let count = tracks_arc.len();

    let heading = if is_queue {
        "Playback Queue".to_string()
    } else {
        snap.playlist_heading.clone()
    };
    let eyebrow = if is_queue {
        "ACTIVE QUEUE"
    } else if snap
        .playlist_id
        .as_deref()
        .is_some_and(|id| id.starts_with("MPRE"))
    {
        "YOUTUBE MUSIC ALBUM"
    } else {
        "YOUTUBE MUSIC PLAYLIST"
    };
    let subtitle = if is_queue {
        format!("{count} tracks queued")
    } else if !snap.playlist_subtitle.is_empty() {
        format!("{} · {count} tracks", snap.playlist_subtitle)
    } else {
        format!("{count} tracks")
    };

    let cover_art = if is_queue {
        snap.art_path.clone()
    } else {
        cached_thumb_path(snap.playlist_cover_url.as_deref())
            .or_else(|| {
                tracks_arc
                    .first()
                    .and_then(|t| cached_thumb_path(t.thumbnail_url.as_deref()))
            })
            .or_else(|| snap.art_path.clone())
    };

    let editable = !is_queue && snap.playlist_editable;
    let loading = !is_queue && snap.playlist_loading;
    let tracks_for_play = tracks_arc.clone();
    let tracks_for_shuffle = tracks_arc.clone();

    let body = if loading && count == 0 {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(p.muted)
            .child("Loading tracks from YouTube Music...")
            .into_any_element()
    } else if count == 0 {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(ui_icon(IconName::Music2, 34.0, p.muted))
            .child(div().text_sm().text_color(p.muted).child(if is_queue {
                "Your queue is empty. Play something from Home or Search!"
            } else {
                "No tracks found in this collection."
            }))
            .into_any_element()
    } else {
        uniform_list(
            if is_queue {
                "queue-stage-list"
            } else {
                "playlist-stage-list"
            },
            count,
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                let p = this.palette();
                let snap = this.snapshot.clone();
                let cur_id = snap.current_track().map(|t| t.id.as_str());
                let tracks = if is_queue {
                    Arc::new(snap.queue.clone())
                } else {
                    Arc::new(snap.playlist_tracks.clone())
                };

                range
                    .filter_map(|idx| {
                        let track = tracks.get(idx)?.clone();
                        let is_playing = if is_queue {
                            snap.current_index == Some(idx)
                        } else {
                            cur_id == Some(track.id.as_str())
                        };
                        let is_hovered = this.hovered_track == Some(idx);
                        Some(render_track_row(
                            format!("col-row-{idx}"),
                            idx,
                            track,
                            tracks.clone(),
                            is_playing,
                            is_hovered,
                            editable,
                            &p,
                            cx,
                        ))
                    })
                    .collect()
            }),
        )
        .track_scroll(&view.track_scroll)
        .flex_1()
        .w_full()
        .into_any_element()
    };

    div()
        .size_full()
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            // Highlight the selected collection with its artwork and title.
            div()
                .w_full()
                .p_4()
                .rounded_2xl()
                .bg(linear_gradient(
                    130.0,
                    linear_color_stop(p.accent_soft, 0.0),
                    linear_color_stop(p.card_bg, 0.88),
                ))
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .items_center()
                .gap_5()
                .child(render_cover_art(cover_art.as_ref(), 132.0, 14.0, p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.accent)
                                .child(eyebrow),
                        )
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(heading),
                        )
                        .child(div().text_xs().text_color(p.muted).child(subtitle))
                        .child(
                            div()
                                .pt_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(count > 0, |el| {
                                    el.child(
                                        div()
                                            .id("hero-play-btn")
                                            .px_4()
                                            .py(px(7.0))
                                            .rounded_full()
                                            .bg(p.accent)
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.backend.send(BackendCmd::PlayTracks {
                                                    tracks: (*tracks_for_play).clone(),
                                                    start_index: 0,
                                                    shuffle: false,
                                                });
                                            }))
                                            .child(ui_icon(IconName::Play, 14.0, p.accent_fg))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(p.accent_fg)
                                                    .child("Play"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("hero-shuffle-btn")
                                            .px_3()
                                            .py(px(7.0))
                                            .rounded_full()
                                            .bg(p.pill_bg)
                                            .border_1()
                                            .border_color(p.border_subtle)
                                            .hover(|s| s.bg(p.card_hover))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.backend.send(BackendCmd::PlayTracks {
                                                    tracks: (*tracks_for_shuffle).clone(),
                                                    start_index: 0,
                                                    shuffle: true,
                                                });
                                            }))
                                            .child(ui_icon(IconName::Shuffle, 14.0, p.text))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(p.text)
                                                    .child("Shuffle"),
                                            ),
                                    )
                                })
                                .when(!is_queue, |el| {
                                    el.child(render_icon_button(
                                        "hero-refresh-btn",
                                        IconName::RefreshCw,
                                        15.0,
                                        32.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| {
                                            this.backend.send(BackendCmd::RefreshActivePlaylist);
                                        },
                                        cx,
                                    ))
                                }),
                        ),
                ),
        )
        .child(body)
        .into_any_element()
}

fn render_lyrics_stage(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let track = snap.current_track().cloned();
    let title = track
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "No Active Track".to_string());
    let artist = track
        .as_ref()
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| "Pick a song from Home or Search".to_string());
    let album = track.as_ref().map(|t| t.album.clone()).unwrap_or_default();

    let right_lyrics = match &snap.lyrics {
        LyricsState::Idle => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("Start playback to view synced lyrics.")
            .into_any_element(),
        LyricsState::Loading => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("Fetching synced lyrics...")
            .into_any_element(),
        LyricsState::NotFound => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("No lyrics found for this track.")
            .into_any_element(),
        LyricsState::Plain(lines) => {
            let count = lines.len();
            uniform_list(
                "lyrics-plain-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, _cx| {
                    let p = this.palette();
                    let LyricsState::Plain(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|idx| {
                            let line = lines.get(idx)?;
                            Some(
                                div()
                                    .w_full()
                                    .py(px(6.0))
                                    .px_4()
                                    .text_lg()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.lyrics_mid)
                                    .child(line.clone()),
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
        LyricsState::Synced(lines) => {
            let count = lines.len();
            uniform_list(
                "lyrics-synced-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, cx| {
                    let p = this.palette();
                    let active = this.snapshot.active_lyric_index();
                    let LyricsState::Synced(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|idx| {
                            let line = lines.get(idx)?;
                            let dist = active.map(|a| a.abs_diff(idx)).unwrap_or(10);
                            let (color, size_px, weight, row_bg) = match dist {
                                0 => (p.lyrics_active, 24.0, FontWeight::BOLD, p.ambient_glow),
                                1 => (
                                    p.lyrics_mid,
                                    19.0,
                                    FontWeight::SEMIBOLD,
                                    gpui_kit::transparent_black(),
                                ),
                                _ => (
                                    p.lyrics_inactive,
                                    17.0,
                                    FontWeight::MEDIUM,
                                    gpui_kit::transparent_black(),
                                ),
                            };
                            let text = if line.text.trim().is_empty() {
                                "♪".to_string()
                            } else {
                                line.text.clone()
                            };
                            let target_secs = line.time_secs;

                            Some(
                                div()
                                    .id(SharedString::from(format!("lyr-row-{idx}")))
                                    .w_full()
                                    .min_h(px(50.0))
                                    .px_4()
                                    .py_2()
                                    .rounded_xl()
                                    .bg(row_bg)
                                    .hover(|s| s.bg(p.card_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(target_secs));
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(size_px))
                                            .font_weight(weight)
                                            .text_color(color)
                                            .child(text),
                                    ),
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
    };

    div()
        .size_full()
        .bg(linear_gradient(
            140.0,
            linear_color_stop(p.ambient_glow, 0.0),
            linear_color_stop(p.panel_bg, 0.78),
        ))
        .p_6()
        .flex()
        .gap_8()
        .child(
            div()
                .w(px(330.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(render_cover_art(snap.art_path.as_ref(), 280.0, 20.0, p))
                .child(
                    div()
                        .w_full()
                        .px_2()
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
                        .when(!album.is_empty(), |el| {
                            el.child(div().text_xs().text_color(p.muted).truncate().child(album))
                        }),
                )
                .child(
                    div().flex().flex_wrap().justify_center().gap_2().child(
                        div()
                            .px_3()
                            .py(px(4.0))
                            .rounded_full()
                            .bg(p.pill_bg)
                            .border_1()
                            .border_color(p.border_subtle)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.success)
                            .child(snap.stream_badge.clone()),
                    ),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .px_4()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.muted)
                        .child("SYNCED LYRICS (CLICK ANY LINE TO SEEK)"),
                )
                .child(right_lyrics),
        )
        .into_any_element()
}

fn render_queue_drawer(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let count = view.snapshot.queue.len();
    let list_el = if count == 0 {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(p.muted)
            .child("Queue is empty")
            .into_any_element()
    } else {
        uniform_list(
            "queue-drawer-list",
            count,
            cx.processor(|this, range: Range<usize>, _window, cx| {
                let p = this.palette();
                let snap = this.snapshot.clone();
                range
                    .filter_map(|idx| {
                        let track = snap.queue.get(idx)?;
                        let active = snap.current_index == Some(idx);
                        Some(
                            div()
                                .id(SharedString::from(format!("q-item-{idx}")))
                                .w_full()
                                .h(px(48.0))
                                .px_2()
                                .rounded_lg()
                                .bg(if active {
                                    p.accent_soft
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend.send(BackendCmd::PlayQueueIndex(idx));
                                }))
                                .child(
                                    div()
                                        .w(px(20.0))
                                        .text_xs()
                                        .text_color(if active { p.accent } else { p.muted })
                                        .child(if active {
                                            "▶".to_string()
                                        } else {
                                            format!("{}", idx + 1)
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(if active {
                                                    FontWeight::BOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(if active { p.accent } else { p.text })
                                                .truncate()
                                                .child(track.title.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(p.muted)
                                                .truncate()
                                                .child(track.artist.clone()),
                                        ),
                                ),
                        )
                    })
                    .collect()
            }),
        )
        .track_scroll(&view.queue_scroll)
        .flex_1()
        .w_full()
        .into_any_element()
    };

    div()
        .w(px(274.0))
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
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child(format!("Up Next ({count})")),
                )
                .child(render_icon_button(
                    "q-close",
                    IconName::X,
                    14.0,
                    24.0,
                    p.muted,
                    p.pill_bg,
                    p.card_hover,
                    |this, _, cx| {
                        this.show_queue = false;
                        cx.notify();
                    },
                    cx,
                )),
        )
        .child(list_el)
        .into_any_element()
}

fn render_player_bar(
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
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_fullscreen_view(cx);
                                }))
                                .child(render_cover_art(snap.art_path.as_ref(), 52.0, 10.0, p)),
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
