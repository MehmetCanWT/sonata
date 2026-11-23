//! Library (Playlists, Albums, Artists tabs + filter), Listening History, and full Queue views.
//!
//! 1. **Library Page**: uses `TabBar::pill()` to switch between user Playlists, saved Albums, and followed Artists with instant search filtering and `+ New Playlist`.
//! 2. **History Page**: displays synced YouTube Music listening history and session played tracks cleanly.
//! 3. **Queue View**: renders a full-page queue inspection fallback.

use super::home::TrackRowOpts;
use super::{DesktopModal, LibraryTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::{CARD_W, PAGE_INSET_X};
use crate::ui::components::{cover_art_tile, scroll_edge_fades};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window,
};

impl SonataDesktopView {
    pub fn render_library_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let filter = self
            .library_filter_input
            .read(cx)
            .value()
            .trim()
            .to_ascii_lowercase();
        let active_tab = match self.library_tab {
            LibraryTab::Playlists => 0,
            LibraryTab::Albums => 1,
            LibraryTab::Artists => 2,
        };
        let scroll_fades = self.settings.scroll_fades;

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .flex_wrap()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .text_size(px(26.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child("Library"),
                    )
                    .child(
                        TabBar::new("lib-tabs")
                            .pill()
                            .small()
                            .selected_index(active_tab)
                            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                this.library_tab = match *idx {
                                    1 => LibraryTab::Albums,
                                    2 => LibraryTab::Artists,
                                    _ => LibraryTab::Playlists,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label("Playlists"))
                            .child(Tab::new().label("Albums"))
                            .child(Tab::new().label("Artists")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(210.0))
                            .h(px(32.0))
                            .rounded(px(self.palette.radii.control))
                            .bg(self.palette.surface)
                            .border_1()
                            .border_color(self.palette.outline)
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .child(Input::new(&self.library_filter_input).appearance(false)),
                    )
                    .child(
                        Button::new("lib-new-pl")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label("New playlist")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.modal = DesktopModal::CreatePlaylist;
                                this.playlist_name_input
                                    .update(cx, |s, cx| s.set_value("", window, cx));
                                this.playlist_name_input
                                    .update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            })),
                    ),
            );

        let gap = 14.0_f32;
        let cols = (((content_w + gap) / (CARD_W + gap)).floor() as usize).clamp(2, 8);
        let card_w = ((content_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
            .clamp(CARD_W, 176.0);

        let body: AnyElement = if !self.snapshot.is_signed_in() {
            div()
                .p(px(28.0))
                .rounded(px(self.palette.radii.control))
                .bg(self.palette.surface)
                .flex()
                .flex_col()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Sign in to view your library"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(self.palette.secondary)
                        .child("Connect your browser YouTube Music session to sync liked songs, playlists, saved albums, and artists."),
                )
                .child(
                    Button::new("lib-signin-cta")
                        .primary()
                        .label("Sign in with browser")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.modal = DesktopModal::SignInBrowser;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            match self.library_tab {
                LibraryTab::Playlists => {
                    let playlists: Vec<_> = self
                        .ordered_playlists()
                        .into_iter()
                        .filter(|p| {
                            filter.is_empty() || p.name.to_ascii_lowercase().contains(&filter)
                        })
                        .collect();
                    let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                    for (i, pl) in playlists.into_iter().enumerate() {
                        let cover = self.media_for_url(pl.thumbnail_url.as_deref(), cx);
                        let is_liked = pl.id == "VLLM";
                        let subtitle = match pl.track_count {
                            Some(n) => format!("{n} tracks"),
                            None if is_liked => "Liked Music".to_string(),
                            None => "Playlist".to_string(),
                        };
                        let pl_open = pl.clone();
                        grid = grid.child(
                            div()
                                .id(SharedString::from(format!("lib-pl-{i}")))
                                .w(px(card_w))
                                .p(px(4.0))
                                .rounded(px(self.palette.radii.control))
                                .hover(|s| s.bg(self.palette.surface.opacity(0.45)))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.backend.send(BackendCmd::OpenCollection {
                                        id: pl_open.id.clone(),
                                        title: pl_open.name.clone(),
                                        subtitle: "YouTube Music Playlist".to_string(),
                                        thumbnail_url: pl_open.thumbnail_url.clone(),
                                        editable: true,
                                    });
                                    cx.notify();
                                }))
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(if is_liked && cover.is_none() {
                                    div()
                                        .size(px(card_w))
                                        .rounded(px(self.palette.radii.art(card_w)))
                                        .bg(self.palette.accent.opacity(0.20))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(IconName::Heart)
                                                .size(px(42.0))
                                                .text_color(self.palette.accent),
                                        )
                                        .into_any_element()
                                } else {
                                    cover_art_tile(
                                        SharedString::from(format!("lib-pl-cov-{i}")),
                                        cover.as_ref(),
                                        card_w,
                                        false,
                                        &self.palette,
                                        false,
                                        None,
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .truncate()
                                                .child(pl.name),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(self.palette.secondary)
                                                .truncate()
                                                .child(subtitle),
                                        ),
                                ),
                        );
                    }
                    grid.into_any_element()
                }
                LibraryTab::Albums => {
                    let albums: Vec<_> = self
                        .snapshot
                        .library_albums
                        .iter()
                        .filter(|a| {
                            filter.is_empty()
                                || a.title.to_ascii_lowercase().contains(&filter)
                                || a.subtitle.to_ascii_lowercase().contains(&filter)
                        })
                        .cloned()
                        .collect();
                    if albums.is_empty() {
                        div()
                            .py(px(24.0))
                            .text_size(px(13.0))
                            .text_color(self.palette.dim)
                            .child(if self.snapshot.library_extras_loading {
                                "Loading saved albums..."
                            } else {
                                "No saved albums found."
                            })
                            .into_any_element()
                    } else {
                        let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                        for (i, a) in albums.iter().enumerate() {
                            grid = grid.child(self.render_card_tile(
                                &format!("lib-alb-{i}"),
                                a,
                                card_w,
                                cx,
                            ));
                        }
                        grid.into_any_element()
                    }
                }
                LibraryTab::Artists => {
                    let artists: Vec<_> = self
                        .snapshot
                        .library_artists
                        .iter()
                        .filter(|a| {
                            filter.is_empty() || a.title.to_ascii_lowercase().contains(&filter)
                        })
                        .cloned()
                        .collect();
                    if artists.is_empty() {
                        div()
                            .py(px(24.0))
                            .text_size(px(13.0))
                            .text_color(self.palette.dim)
                            .child(if self.snapshot.library_extras_loading {
                                "Loading artists..."
                            } else {
                                "No library artists found."
                            })
                            .into_any_element()
                    } else {
                        let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                        for (i, a) in artists.iter().enumerate() {
                            grid = grid.child(self.render_card_tile(
                                &format!("lib-art-{i}"),
                                a,
                                card_w,
                                cx,
                            ));
                        }
                        grid.into_any_element()
                    }
                }
            }
        };

        let page = div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(header)
            .child(body);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("library-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(page),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }

    pub fn render_history_page(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut tracks = self.snapshot.history_tracks.clone();
        if tracks.is_empty() {
            tracks = self.snapshot.last_played_tracks.clone();
        }
        let scroll_fades = self.settings.scroll_fades;

        let mut list = div().flex().flex_col().gap(px(2.0));
        if tracks.is_empty() {
            list = list.child(
                div()
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child(if self.snapshot.history_loading {
                        "Loading listening history..."
                    } else {
                        "No recently played tracks yet."
                    }),
            );
        } else {
            let all = tracks.clone();
            for i in 0..all.len() {
                list = list.child(self.render_track_row(
                    "hist-tr",
                    i,
                    &all,
                    TrackRowOpts {
                        show_album: true,
                        ..Default::default()
                    },
                    cx,
                ));
            }
        }

        let page = div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(26.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child("History"),
                    )
                    .when(!tracks.is_empty(), |r| {
                        r.child(
                            Button::new("hist-play-all")
                                .primary()
                                .small()
                                .icon(IconName::Play)
                                .label("Play all")
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend.send(BackendCmd::PlayTracks {
                                        tracks: tracks.clone(),
                                        start_index: 0,
                                        shuffle: false,
                                    });
                                })),
                        )
                    }),
            )
            .child(list);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("history-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(page),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }

    pub fn render_queue_page(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let queue = self.snapshot.queue.clone();
        let scroll_fades = self.settings.scroll_fades;
        let mut rows = div().flex().flex_col().gap(px(2.0));
        if queue.is_empty() {
            rows = rows.child(
                div()
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child("Queue is empty"),
            );
        } else {
            let all = queue.clone();
            for i in 0..all.len() {
                rows = rows.child(self.render_track_row(
                    "fullq-tr",
                    i,
                    &all,
                    TrackRowOpts {
                        show_index: true,
                        show_album: true,
                        ..Default::default()
                    },
                    cx,
                ));
            }
        }

        let page = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(
                div()
                    .text_size(px(26.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Queue"),
            )
            .child(rows);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("fullq-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(page),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }
}
