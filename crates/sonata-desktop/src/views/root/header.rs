//! Top window header bar, search input, tool toggles, and account popover menu.
//!
//! 1. **Top Header**: `render_top_header` hosts the Sonata brand home button, centered search input, lyrics/queue/mini-mode/settings buttons, and account trigger.
//! 2. **Account Popover**: `render_account_popover` displays the active user profile, refresh/settings shortcuts, and sign-out action.

use super::super::{render_avatar, render_icon_button, ui_icon, CenterMode, SonataDesktopView};
use crate::backend::{BackendCmd, NavTarget};
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled,
};

pub(super) fn render_top_header(
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
                    "hdr-fullscreen",
                    IconName::Maximize2,
                    15.0,
                    32.0,
                    if view.center_mode == CenterMode::Fullscreen {
                        p.accent
                    } else {
                        p.text
                    },
                    if view.center_mode == CenterMode::Fullscreen {
                        p.accent_soft
                    } else {
                        p.panel_bg
                    },
                    p.card_hover,
                    |this, window, cx| this.toggle_fullscreen_view(window, cx),
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
