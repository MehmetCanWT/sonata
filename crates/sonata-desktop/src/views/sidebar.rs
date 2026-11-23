//! Floating left navigation card with automatic 72 pt icon-rail snap, tooltips, and reorderable playlists.
//!
//! 1. **Responsive Drawer / Rail**: renders the full 200..400 pt drawer above `100 pt`, collapsing cleanly to a `72 pt` centered icon rail below the threshold.
//! 2. **Navigation**: routes to `Home`, `Explore`, `Radio`, `Library`, `History`, and `Settings` with active accent indicators and rail `Tooltip`s.
//! 3. **Playlists**: pins Liked Music at the top, supports creating playlists (`+`), and allows pointer drag-reordering of custom playlists.

use super::{DesktopModal, SonataDesktopView};
use crate::backend::{BackendCmd, NavTarget};
use crate::settings::CARD_INSET_X;
use crate::ui::components::{card_panel, cover_art_tile, scroll_edge_fades};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};

struct NavEntry {
    target: NavTarget,
    label: &'static str,
    icon: IconName,
}

impl SonataDesktopView {
    pub fn render_sidebar(
        &mut self,
        width_px: f32,
        is_rail: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let nav_items = [
            NavEntry {
                target: NavTarget::Home,
                label: "Home",
                icon: IconName::House,
            },
            NavEntry {
                target: NavTarget::Explore,
                label: "Explore",
                icon: IconName::Compass,
            },
            NavEntry {
                target: NavTarget::Radio,
                label: "Radio",
                icon: IconName::Radio,
            },
            NavEntry {
                target: NavTarget::Library,
                label: "Library",
                icon: IconName::Library,
            },
            NavEntry {
                target: NavTarget::History,
                label: "History",
                icon: IconName::Clock,
            },
            NavEntry {
                target: NavTarget::Settings,
                label: "Settings",
                icon: IconName::Settings,
            },
        ];

        let mut nav_col = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(if is_rail { 10.0 } else { CARD_INSET_X }))
            .pt(px(12.0));

        for (i, item) in nav_items.into_iter().enumerate() {
            let active = self.snapshot.active_nav == item.target;
            let target = item.target.clone();
            let tip: SharedString = item.label.into();

            let row = div()
                .id(SharedString::from(format!("sb-nav-{i}")))
                .h(px(40.0))
                .px(px(if is_rail { 0.0 } else { 12.0 }))
                .rounded(px(self.palette.radii.item))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .when(is_rail, |d| d.justify_center())
                .when(!is_rail, |d| d.gap(px(12.0)))
                .bg(if active {
                    self.palette.surface_active
                } else {
                    gpui_kit::transparent_black()
                })
                .hover(|s| {
                    if active {
                        s
                    } else {
                        s.bg(self.palette.surface_hover)
                    }
                })
                .when(is_rail, |d| {
                    let tip_label = tip.clone();
                    d.tooltip(move |w, cx| Tooltip::new(tip_label.clone()).build(w, cx))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.navigate_to(target.clone(), cx);
                }))
                .child(Icon::new(item.icon).size(px(18.0)).text_color(if active {
                    self.palette.accent
                } else {
                    self.palette.secondary
                }))
                .when(!is_rail, |d| {
                    d.child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(if active {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if active {
                                self.palette.text
                            } else {
                                self.palette.secondary
                            })
                            .child(item.label),
                    )
                });

            nav_col = nav_col.child(row);
        }

        let playlists = self.ordered_playlists();
        let scroll_fades = self.settings.scroll_fades;

        let mut playlist_list = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(if is_rail { 10.0 } else { CARD_INSET_X }))
            .pb(px(12.0));

        for (idx, pl) in playlists.iter().enumerate() {
            let active =
                matches!(&self.snapshot.active_nav, NavTarget::Playlist(id) if id == &pl.id);
            let is_liked = pl.id == "VLLM";
            let cover = self.media_for_url(pl.thumbnail_url.as_deref(), cx);
            let count_text = match pl.track_count {
                Some(n) => format!("{n} tracks"),
                None if is_liked => "Your likes".to_string(),
                None => "Playlist".to_string(),
            };
            let pl_open = pl.clone();
            let pl_id_drag = pl.id.clone();
            let pl_id_drop = pl.id.clone();
            let tip_name: SharedString = pl.name.clone().into();
            let is_dragging_source = self.dragging_playlist_idx.is_some();

            let art = if is_liked && cover.is_none() {
                div()
                    .flex_shrink_0()
                    .size(px(40.0))
                    .rounded(px(self.palette.radii.art(40.0)))
                    .bg(self.palette.accent.opacity(0.22))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::Heart)
                            .size(px(18.0))
                            .text_color(self.palette.accent),
                    )
                    .into_any_element()
            } else {
                cover_art_tile(
                    SharedString::from(format!("sb-cov-{idx}")),
                    cover.as_ref(),
                    40.0,
                    false,
                    &self.palette,
                    false,
                    None,
                )
            };

            let row = div()
                .id(SharedString::from(format!("sb-pl-{idx}")))
                .group(SharedString::from(format!("sb-pl-grp-{idx}")))
                .h(px(54.0))
                .px(px(if is_rail { 0.0 } else { 8.0 }))
                .rounded(px(self.palette.radii.item))
                .cursor_pointer()
                .bg(if active {
                    self.palette.surface_active
                } else {
                    gpui_kit::transparent_black()
                })
                .hover(|s| {
                    if active {
                        s
                    } else {
                        s.bg(self.palette.surface_hover)
                    }
                })
                .flex()
                .flex_row()
                .items_center()
                .when(is_rail, |d| {
                    let tip = tip_name.clone();
                    d.justify_center()
                        .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                })
                .when(!is_rail, |d| d.gap(px(10.0)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if !is_liked {
                            this.dragging_playlist_idx = Some(idx);
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(from_idx) = this.dragging_playlist_idx.take() {
                            if from_idx != idx {
                                let ordered = this.ordered_playlists();
                                if let Some(from_pl) = ordered.get(from_idx) {
                                    this.reorder_playlist(&from_pl.id, &pl_id_drop, cx);
                                    return;
                                }
                            }
                        }
                        let _ = &pl_id_drag;
                    }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.dragging_playlist_idx = None;
                    let sub = if pl_open.id == "VLLM" {
                        "Your liked YouTube Music tracks".to_string()
                    } else {
                        "YouTube Music Playlist".to_string()
                    };
                    this.fullscreen_open = false;
                    this.backend.send(BackendCmd::OpenCollection {
                        id: pl_open.id.clone(),
                        title: pl_open.name.clone(),
                        subtitle: sub,
                        thumbnail_url: pl_open.thumbnail_url.clone(),
                        editable: true,
                    });
                    cx.notify();
                }))
                .child(art)
                .when(!is_rail, |d| {
                    d.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(if active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if active {
                                        self.palette.text
                                    } else {
                                        self.palette.secondary
                                    })
                                    .truncate()
                                    .child(pl.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(self.palette.dim)
                                    .truncate()
                                    .child(count_text),
                            ),
                    )
                    .when(!is_liked, |row| {
                        row.child(
                            div()
                                .opacity(if is_dragging_source { 0.85 } else { 0.0 })
                                .group_hover(SharedString::from(format!("sb-pl-grp-{idx}")), |s| {
                                    s.opacity(0.75)
                                })
                                .child(
                                    Icon::new(IconName::GripVertical)
                                        .size(px(14.0))
                                        .text_color(self.palette.dim),
                                ),
                        )
                    })
                });

            playlist_list = playlist_list.child(row);
        }

        card_panel(&self.palette)
            .w(px(width_px))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(nav_col)
            .child(
                div()
                    .mx(px(14.0))
                    .my(px(10.0))
                    .h(px(1.0))
                    .bg(self.palette.outline),
            )
            .when(!is_rail, |card| {
                card.child(
                    div()
                        .px(px(16.0))
                        .pb(px(8.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(self.palette.dim)
                                .child("PLAYLISTS"),
                        )
                        .child(
                            Button::new("sb-new-playlist")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Plus)
                                .tooltip("New playlist")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.modal = DesktopModal::CreatePlaylist;
                                    this.playlist_name_input
                                        .update(cx, |s, cx| s.set_value("", window, cx));
                                    this.playlist_name_input
                                        .update(cx, |s, cx| s.focus(window, cx));
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(is_rail, |card| {
                card.child(
                    div().pb(px(6.0)).flex().justify_center().child(
                        Button::new("sb-rail-new-playlist")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .tooltip("New playlist")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.modal = DesktopModal::CreatePlaylist;
                                this.playlist_name_input
                                    .update(cx, |s, cx| s.set_value("", window, cx));
                                this.playlist_name_input
                                    .update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            })),
                    ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("sidebar-playlists-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .child(playlist_list),
                    )
                    .child(scroll_edge_fades(
                        &self.sidebar_scroll,
                        &self.palette,
                        scroll_fades,
                    )),
            )
            .into_any_element()
    }
}
