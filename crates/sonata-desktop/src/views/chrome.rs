//! Transparent top chrome (`56 pt`) over the blurred gutter: history arrows, search pill, profile, and window controls.
//!
//! 1. **TitleBar Integration**: uses `gpui_kit::component::TitleBar` for native drag move, double-click maximize, right-click WM menu, and platform window buttons (`--decorated` hides client controls cleanly).
//! 2. **Navigation & Search**: Back / Forward arrows (`< >`), Refresh (`⟳`), and centered Search pill (`Input`) with live suggestions + recent history dropdown.
//! 3. **Account Trigger**: signed-in user avatar chip or clean `Sign in` pill opening the account popover.

use super::SonataDesktopView;
use crate::backend::{BackendCmd, NavTarget};
use crate::settings::TITLEBAR_H;
use crate::ui::components::popup_panel;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, Sizable as _, TitleBar};
use gpui_kit::{
    div, img, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ObjectFit, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};

impl SonataDesktopView {
    pub fn render_top_bar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let can_back = self.can_go_back();
        let can_fwd = self.can_go_forward();
        let signed_in = self.snapshot.is_signed_in();
        let avatar = self.media_for_path(self.snapshot.user_avatar_path.as_deref(), cx);
        let profile_name = self
            .snapshot
            .user_profile
            .as_ref()
            .map(|p| p.name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Account".to_string());

        let center_search = div()
            .w(px(420.0))
            .max_w_full()
            .h(px(36.0))
            .rounded_full()
            .bg(self.palette.surface)
            .border_1()
            .border_color(self.palette.outline)
            .px(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .child(
                Icon::new(IconName::Search)
                    .size(px(15.0))
                    .text_color(self.palette.dim),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.search_input).appearance(false)),
            )
            .when(!self.search_input.read(cx).value().is_empty(), |row| {
                row.child(
                    Button::new("topbar-clear-search")
                        .ghost()
                        .xsmall()
                        .icon(IconName::X)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search_input
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.search_dropdown_open = false;
                            cx.notify();
                        })),
                )
            });

        let account_btn = div()
            .id("topbar-account-btn")
            .h(px(34.0))
            .px(px(if signed_in { 6.0 } else { 12.0 }))
            .pr(px(12.0))
            .rounded_full()
            .bg(self.palette.surface)
            .border_1()
            .border_color(self.palette.outline)
            .cursor_pointer()
            .hover(|s| s.bg(self.palette.surface_hover))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(cx.listener(|this, _, _, cx| {
                if this.snapshot.is_signed_in() {
                    this.account_menu_open = !this.account_menu_open;
                } else {
                    this.modal = super::DesktopModal::SignInBrowser;
                }
                cx.notify();
            }))
            .child(if let Some(av) = avatar {
                img(av.image)
                    .size(px(24.0))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element()
            } else {
                div()
                    .size(px(24.0))
                    .rounded_full()
                    .bg(self.palette.surface_active)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::User)
                            .size(px(13.0))
                            .text_color(self.palette.secondary),
                    )
                    .into_any_element()
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.palette.text)
                    .child(if signed_in {
                        profile_name
                    } else {
                        "Sign in".to_string()
                    }),
            );

        let left_nav = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .child(
                Button::new("nav-back")
                    .ghost()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .tooltip("Back (Alt+Left)")
                    .when(!can_back, |b| b.opacity(0.35))
                    .on_click(cx.listener(|this, _, _, cx| this.go_back(cx))),
            )
            .child(
                Button::new("nav-forward")
                    .ghost()
                    .small()
                    .icon(IconName::ChevronRight)
                    .tooltip("Forward (Alt+Right)")
                    .when(!can_fwd, |b| b.opacity(0.35))
                    .on_click(cx.listener(|this, _, _, cx| this.go_forward(cx))),
            )
            .child(
                Button::new("nav-refresh")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .tooltip("Refresh current view")
                    .on_click(cx.listener(|this, _, _, cx| {
                        match this.snapshot.active_nav {
                            NavTarget::Explore => this.backend.send(BackendCmd::RefreshExplore),
                            NavTarget::Radio => this.backend.send(BackendCmd::RefreshRadio),
                            NavTarget::Library => this.backend.send(BackendCmd::RefreshLibrary),
                            NavTarget::History => this.backend.send(BackendCmd::RefreshHistory),
                            NavTarget::Playlist(_) => {
                                this.backend.send(BackendCmd::RefreshActivePlaylist)
                            }
                            _ => this.backend.send(BackendCmd::RefreshHome),
                        }
                        cx.notify();
                    })),
            );

        let bar_body = div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(left_nav),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .child(center_search),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .child(account_btn),
            );

        TitleBar::new()
            .h(px(TITLEBAR_H))
            .pl(px(if cfg!(target_os = "macos") {
                80.0
            } else {
                12.0
            }))
            .pr(px(10.0))
            .border_b_0()
            .bg(gpui_kit::transparent_black())
            .on_close_window(|_, window, _| {
                window.remove_window();
            })
            .child(bar_body)
            .into_any_element()
    }

    pub fn render_search_dropdown(
        &mut self,
        vp_w: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = self.search_input.read(cx).value().to_string();
        let suggestions = self.snapshot.search_suggestions.clone();
        let recents = self.snapshot.recent_searches.clone();
        if suggestions.is_empty() && recents.is_empty() {
            return div().into_any_element();
        }
        let width = 420.0_f32.min((vp_w - 40.0).max(260.0));
        // Account for TitleBar horizontal insets + right window controls so the dropdown stays locked under the search bar.
        let right_inset = if self.decorated || cfg!(target_os = "macos") {
            10.0
        } else {
            116.0
        };
        let left_inset = if cfg!(target_os = "macos") {
            80.0
        } else {
            12.0
        };

        div()
            .id("search-dropdown-backdrop")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.search_dropdown_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(TITLEBAR_H + 2.0))
                    .left(px(left_inset))
                    .right(px(right_inset))
                    .flex()
                    .justify_center()
                    .child(
                        popup_panel(&self.palette)
                            .id("search-dropdown-card")
                            .w(px(width))
                            .p(px(8.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .when(
                                !query.trim().is_empty() && !suggestions.is_empty(),
                                |card| {
                                    card.child(
                                        div()
                                            .px(px(10.0))
                                            .py(px(4.0))
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(self.palette.dim)
                                            .child("Suggestions"),
                                    )
                                    .children(
                                        suggestions.into_iter().enumerate().map(|(i, s)| {
                                            let picked = s.clone();
                                            div()
                                                .id(SharedString::from(format!("sug-{i}")))
                                                .px(px(10.0))
                                                .py(px(8.0))
                                                .rounded(px(self.palette.radii.item))
                                                .cursor_pointer()
                                                .hover(|st| st.bg(self.palette.surface_hover))
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(10.0))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.search_dropdown_open = false;
                                                        let q = picked.clone();
                                                        this.search_input.update(cx, |st, cx| {
                                                            st.set_value(q.clone(), window, cx)
                                                        });
                                                        this.navigate_to(NavTarget::Search, cx);
                                                        this.backend.send(BackendCmd::Search(q));
                                                    },
                                                ))
                                                .child(
                                                    Icon::new(IconName::Search)
                                                        .size(px(14.0))
                                                        .text_color(self.palette.dim),
                                                )
                                                .child(div().text_size(px(13.0)).child(s))
                                        }),
                                    )
                                },
                            )
                            .when(query.trim().is_empty() && !recents.is_empty(), |card| {
                                let _ = window;
                                card.child(
                                    div()
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(self.palette.dim)
                                        .child("Recent searches"),
                                )
                                .children(
                                    recents.into_iter().take(6).enumerate().map(|(i, r)| {
                                        let picked = r.clone();
                                        div()
                                            .id(SharedString::from(format!("rec-s-{i}")))
                                            .px(px(10.0))
                                            .py(px(8.0))
                                            .rounded(px(self.palette.radii.item))
                                            .cursor_pointer()
                                            .hover(|st| st.bg(self.palette.surface_hover))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(10.0))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.search_dropdown_open = false;
                                                let q = picked.clone();
                                                this.search_input.update(cx, |st, cx| {
                                                    st.set_value(q.clone(), window, cx)
                                                });
                                                this.navigate_to(NavTarget::Search, cx);
                                                this.backend.send(BackendCmd::Search(q));
                                            }))
                                            .child(
                                                Icon::new(IconName::Clock)
                                                    .size(px(14.0))
                                                    .text_color(self.palette.dim),
                                            )
                                            .child(div().text_size(px(13.0)).child(r))
                                    }),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}
