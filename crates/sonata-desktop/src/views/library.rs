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
use sonata_core::providers::servers::ServerKind;
use sonata_core::providers::ytmusic::HomeCardKind;

pub(crate) fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / GIB)
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

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
            LibraryTab::Downloads => 3,
            LibraryTab::Servers => 4,
        };
        let scroll_fades = self.settings.scroll_fades;

        let s = sonata_runtime::i18n::strings(self.settings.language);
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
                            .child(s.library),
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
                                    3 => {
                                        this.backend.send(BackendCmd::RefreshDownloads);
                                        LibraryTab::Downloads
                                    }
                                    4 => {
                                        this.backend.send(BackendCmd::RefreshServers);
                                        LibraryTab::Servers
                                    }
                                    _ => LibraryTab::Playlists,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label(s.playlists))
                            .child(Tab::new().label(s.albums))
                            .child(Tab::new().label(s.artists))
                            .child(Tab::new().label(s.downloads))
                            .child(Tab::new().label(s.servers)),
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
                            .label(s.new_playlist)
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

        let body: AnyElement = if self.library_tab == LibraryTab::Downloads {
            self.render_library_downloads(&filter, card_w, gap, cx)
        } else if self.library_tab == LibraryTab::Servers {
            self.render_library_servers(&filter, card_w, gap, cx)
        } else if !self.snapshot.is_signed_in() {
            let dl_card = self.render_downloads_collection_card(card_w, cx);
            div()
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(gap))
                        .child(dl_card),
                )
                .child(
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
                                .child(s.sign_in_library_title),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(self.palette.secondary)
                                .child(s.sign_in_library_desc),
                        )
                        .child(
                            Button::new("lib-signin-cta")
                                .primary()
                                .label(s.sign_in_browser)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.modal = DesktopModal::SignInBrowser;
                                    cx.notify();
                                })),
                        ),
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
                    if filter.is_empty() || s.downloads.to_ascii_lowercase().contains(&filter) {
                        grid = grid.child(self.render_downloads_collection_card(card_w, cx));
                    }
                    for (i, pl) in playlists.into_iter().enumerate() {
                        let cover = self.media_for_url(pl.thumbnail_url.as_deref(), cx);
                        let is_liked = pl.id == "VLLM";
                        let is_pinned = self.settings.is_pinned(&pl.id);
                        let subtitle = {
                            let base = match pl.track_count {
                                Some(n) => format!("{n} tracks"),
                                None if is_liked => s.liked_music.to_string(),
                                None => s.playlist.to_string(),
                            };
                            if is_pinned {
                                format!("{} • {base}", s.pinned)
                            } else {
                                base
                            }
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
                                        editable: pl_open.is_owned && pl_open.id != "VLLM",
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
                LibraryTab::Downloads | LibraryTab::Servers => unreachable!(),
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
            let all: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(tracks.as_slice());
            let total = all.len();
            const ROW_STRIDE: f32 = 56.0;
            const OVERSCAN: usize = 16;
            let (win_start, win_end) = if total > 64 {
                let scroll_y = (-f32::from(self.main_scroll.offset().y)).max(0.0);
                let vp_h: f32 = f32::from(self.main_scroll.bounds().size.height).max(900.0);
                let first_vis = ((scroll_y - 80.0).max(0.0) / ROW_STRIDE).floor() as usize;
                let vis_count = (vp_h / ROW_STRIDE).ceil() as usize + 1;
                (
                    first_vis.saturating_sub(OVERSCAN).min(total),
                    (first_vis + vis_count + OVERSCAN).min(total),
                )
            } else {
                (0, total)
            };
            if win_start > 0 {
                list = list.child(div().flex_shrink_0().h(px(win_start as f32 * ROW_STRIDE)));
            }
            for i in win_start..win_end {
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
            if win_end < total {
                list = list.child(
                    div()
                        .flex_shrink_0()
                        .h(px((total - win_end) as f32 * ROW_STRIDE)),
                );
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
            let all: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(queue.as_slice());
            let total = all.len();
            const ROW_STRIDE: f32 = 56.0;
            const OVERSCAN: usize = 16;
            let (win_start, win_end) = if total > 64 {
                let scroll_y = (-f32::from(self.main_scroll.offset().y)).max(0.0);
                let vp_h: f32 = f32::from(self.main_scroll.bounds().size.height).max(900.0);
                let first_vis = ((scroll_y - 80.0).max(0.0) / ROW_STRIDE).floor() as usize;
                let vis_count = (vp_h / ROW_STRIDE).ceil() as usize + 1;
                (
                    first_vis.saturating_sub(OVERSCAN).min(total),
                    (first_vis + vis_count + OVERSCAN).min(total),
                )
            } else {
                (0, total)
            };
            if win_start > 0 {
                rows = rows.child(div().flex_shrink_0().h(px(win_start as f32 * ROW_STRIDE)));
            }
            for i in win_start..win_end {
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
            if win_end < total {
                rows = rows.child(
                    div()
                        .flex_shrink_0()
                        .h(px((total - win_end) as f32 * ROW_STRIDE)),
                );
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

    fn render_downloads_collection_card(&self, card_w: f32, cx: &mut Context<Self>) -> AnyElement {
        let s = sonata_runtime::i18n::strings(self.settings.language);
        let count = self.snapshot.downloaded_tracks.len();
        let size_str = format_bytes(self.snapshot.downloads_total_bytes);
        let subtitle = format!("{count} {} • {size_str}", s.tracks);
        let heading = s.downloads.to_string();
        let sub_open = subtitle.clone();
        div()
            .id("lib-pl-downloads-shared")
            .w(px(card_w))
            .p(px(4.0))
            .rounded(px(self.palette.radii.control))
            .hover(|st| st.bg(self.palette.surface.opacity(0.45)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.backend.send(BackendCmd::OpenCollection {
                    id: "SONATA:DOWNLOADS".to_string(),
                    title: heading.clone(),
                    subtitle: sub_open.clone(),
                    thumbnail_url: None,
                    editable: false,
                });
                cx.notify();
            }))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .size(px(card_w))
                    .rounded(px(self.palette.radii.art(card_w)))
                    .bg(self.palette.accent.opacity(0.20))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::Download)
                            .size(px(42.0))
                            .text_color(self.palette.accent),
                    ),
            )
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
                            .child(s.downloads),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.secondary)
                            .truncate()
                            .child(subtitle),
                    ),
            )
            .into_any_element()
    }

    fn render_library_downloads(
        &mut self,
        filter: &str,
        card_w: f32,
        gap: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = sonata_runtime::i18n::strings(self.settings.language);
        let dl_tracks: Vec<_> = self
            .snapshot
            .downloaded_tracks
            .iter()
            .filter(|t| {
                filter.is_empty()
                    || t.title.to_ascii_lowercase().contains(filter)
                    || t.artist.to_ascii_lowercase().contains(filter)
                    || t.album.to_ascii_lowercase().contains(filter)
            })
            .cloned()
            .collect();
        let playable_tracks: Vec<sonata_core::providers::Track> =
            dl_tracks.iter().map(|d| d.to_track()).collect();
        let play_all = playable_tracks.clone();
        let shuf_all = playable_tracks.clone();
        let total_size = format_bytes(self.snapshot.downloads_total_bytes);

        let mut col = div().flex().flex_col().gap(px(18.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .flex_wrap()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(self.palette.secondary)
                        .child(format!("{} {} • {total_size}", dl_tracks.len(), s.tracks)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .when(!play_all.is_empty(), |r| {
                            r.child(
                                Button::new("dl-play-all")
                                    .primary()
                                    .small()
                                    .icon(IconName::Play)
                                    .label(s.play)
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: play_all.clone(),
                                            start_index: 0,
                                            shuffle: false,
                                        });
                                    })),
                            )
                            .child(
                                Button::new("dl-shuf-all")
                                    .secondary()
                                    .small()
                                    .icon(IconName::Shuffle)
                                    .label(s.shuffle)
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: shuf_all.clone(),
                                            start_index: 0,
                                            shuffle: true,
                                        });
                                    })),
                            )
                        })
                        .child(
                            Button::new("dl-refresh")
                                .secondary()
                                .small()
                                .icon(IconName::RefreshCw)
                                .label(s.refresh)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::RefreshDownloads);
                                })),
                        ),
                ),
        );

        if !self.snapshot.downloaded_collections.is_empty() {
            let mut cols_grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (idx, dc) in self
                .snapshot
                .downloaded_collections
                .iter()
                .filter(|c| filter.is_empty() || c.title.to_ascii_lowercase().contains(filter))
                .enumerate()
            {
                let cover = self.media_for_url(dc.artwork_url.as_deref(), cx);
                let col_id = dc.collection_id.clone();
                let col_id_rm = dc.collection_id.clone();
                let col_title = dc.title.clone();
                let col_thumb = dc.artwork_url.clone();
                let col_sub = format!(
                    "{} {} • {}",
                    dc.track_count,
                    s.tracks,
                    format_bytes(dc.total_bytes)
                );
                let col_sub_open = col_sub.clone();
                cols_grid = cols_grid.child(
                    div()
                        .id(SharedString::from(format!("dl-col-{idx}")))
                        .w(px(card_w))
                        .p(px(6.0))
                        .rounded(px(self.palette.radii.control))
                        .bg(self.palette.surface.opacity(0.35))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id(SharedString::from(format!("dl-col-open-{idx}")))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.backend.send(BackendCmd::OpenCollection {
                                        id: col_id.clone(),
                                        title: col_title.clone(),
                                        subtitle: col_sub_open.clone(),
                                        thumbnail_url: col_thumb.clone(),
                                        editable: false,
                                    });
                                    cx.notify();
                                }))
                                .child(cover_art_tile(
                                    SharedString::from(format!("dl-col-cov-{idx}")),
                                    cover.as_ref(),
                                    card_w - 12.0,
                                    false,
                                    &self.palette,
                                    false,
                                    None,
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(dc.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(self.palette.secondary)
                                .truncate()
                                .child(col_sub),
                        )
                        .child(
                            Button::new(SharedString::from(format!("dl-col-rm-{idx}")))
                                .secondary()
                                .xsmall()
                                .icon(IconName::Trash)
                                .label(s.remove_download)
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend.send(BackendCmd::RemoveDownloadedCollection(
                                        col_id_rm.clone(),
                                    ));
                                })),
                        ),
                );
            }
            col = col.child(cols_grid);
        }

        if playable_tracks.is_empty() {
            col = col.child(
                div()
                    .p(px(24.0))
                    .rounded(px(self.palette.radii.control))
                    .bg(self.palette.surface)
                    .text_size(px(13.0))
                    .text_color(self.palette.secondary)
                    .child(s.no_downloads),
            );
        } else {
            let tracks_arc: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(playable_tracks.as_slice());
            let mut rows = div().flex().flex_col().gap(px(2.0));
            for (i, dl) in dl_tracks.iter().enumerate() {
                let tid_rm = dl.track_id.clone();
                let badge = format!("{} • {}", dl.audio_badge, format_bytes(dl.file_size_bytes));
                rows = rows.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().flex_1().min_w_0().child(self.render_track_row(
                            "dl-tr",
                            i,
                            &tracks_arc,
                            TrackRowOpts {
                                show_album: true,
                                ..Default::default()
                            },
                            cx,
                        )))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(11.0))
                                .text_color(self.palette.dim)
                                .child(badge),
                        )
                        .child(
                            Button::new(SharedString::from(format!("dl-tr-rm-{i}")))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Trash)
                                .tooltip(s.remove_download)
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend
                                        .send(BackendCmd::RemoveDownloadedTrack(tid_rm.clone()));
                                })),
                        ),
                );
            }
            col = col.child(rows);
        }

        col.into_any_element()
    }

    fn render_library_servers(
        &mut self,
        filter: &str,
        card_w: f32,
        gap: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = sonata_runtime::i18n::strings(self.settings.language);
        let profiles = self.snapshot.server_profiles.clone();
        if profiles.is_empty() {
            return div()
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
                        .child(s.no_servers),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(self.palette.secondary)
                        .child(s.connect_server_hint),
                )
                .child(
                    Button::new("srv-empty-add")
                        .primary()
                        .icon(IconName::Plus)
                        .label(s.add_server)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_add_server_modal(window, cx);
                        })),
                )
                .into_any_element();
        }

        let active_id = self
            .snapshot
            .active_server_id
            .clone()
            .or_else(|| profiles.first().map(|p| p.id.clone()))
            .unwrap_or_default();
        let active_profile = profiles.iter().find(|p| p.id == active_id).cloned();

        let mut server_pills = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(8.0));
        for (i, prof) in profiles.iter().enumerate() {
            let is_active = prof.id == active_id;
            let pid = prof.id.clone();
            server_pills = server_pills.child(
                div()
                    .id(SharedString::from(format!("srv-pill-{i}")))
                    .h(px(32.0))
                    .px(px(12.0))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if is_active {
                        self.palette.accent
                    } else {
                        self.palette.surface
                    })
                    .hover(|st| {
                        if is_active {
                            st.bg(self.palette.accent_hover)
                        } else {
                            st.bg(self.palette.surface_hover)
                        }
                    })
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if is_active {
                        self.palette.on_accent
                    } else {
                        self.palette.text
                    })
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.backend.send(BackendCmd::SelectServer {
                            server_id: pid.clone(),
                            library_id: None,
                            folder_path: None,
                        });
                        cx.notify();
                    }))
                    .child(format!("[{}] {}", prof.kind.short_tag(), prof.name)),
            );
        }
        server_pills = server_pills
            .child(
                Button::new("srv-add-btn")
                    .secondary()
                    .small()
                    .icon(IconName::Plus)
                    .label(s.add_server)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_add_server_modal(window, cx);
                    })),
            )
            .child(
                Button::new("srv-refresh-btn")
                    .secondary()
                    .small()
                    .icon(IconName::RefreshCw)
                    .label(s.refresh)
                    .on_click(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::RefreshServers);
                    })),
            );

        let mut body = div().flex().flex_col().gap(px(18.0)).child(server_pills);

        if self.snapshot.server_loading {
            body = body.child(
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child("Loading server library..."),
            );
        }

        if let Some(ref prof) = active_profile {
            if let Some(status) = self.snapshot.server_statuses.get(&prof.id) {
                body = body.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .child(format!("{} • {}", prof.kind.display_name(), status)),
                );
            }

            if prof.kind == ServerKind::WebDav && self.snapshot.server_current_folder.is_some() {
                let sid_root = prof.id.clone();
                body = body.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            Button::new("srv-dav-root")
                                .secondary()
                                .small()
                                .icon(IconName::ChevronLeft)
                                .label(s.back)
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend.send(BackendCmd::SelectServer {
                                        server_id: sid_root.clone(),
                                        library_id: None,
                                        folder_path: None,
                                    });
                                })),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(self.palette.secondary)
                                .child(
                                    self.snapshot
                                        .server_current_folder
                                        .clone()
                                        .unwrap_or_default(),
                                ),
                        ),
                );
            }
        }

        if !self.snapshot.server_libraries.is_empty() {
            let mut lib_row = div().flex().flex_row().flex_wrap().gap(px(8.0));
            for (i, lib) in self.snapshot.server_libraries.iter().enumerate() {
                if !filter.is_empty() && !lib.name.to_ascii_lowercase().contains(filter) {
                    continue;
                }
                let sid = active_id.clone();
                let lid = lib.id.clone();
                let is_dav = active_profile
                    .as_ref()
                    .is_some_and(|p| p.kind == ServerKind::WebDav);
                lib_row = lib_row.child(
                    Button::new(SharedString::from(format!("srv-lib-{i}")))
                        .secondary()
                        .small()
                        .icon(IconName::Folder)
                        .label(lib.name.clone())
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::SelectServer {
                                server_id: sid.clone(),
                                library_id: (!is_dav).then(|| lid.clone()),
                                folder_path: is_dav.then(|| lid.clone()),
                            });
                        })),
                );
            }
            body = body.child(lib_row);
        }

        if !self.snapshot.server_playlists.is_empty() {
            let server_playlists = self.snapshot.server_playlists.clone();
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, pl) in server_playlists.iter().enumerate() {
                if !filter.is_empty() && !pl.name.to_ascii_lowercase().contains(filter) {
                    continue;
                }
                let card = sonata_core::providers::ytmusic::HomeCard {
                    kind: HomeCardKind::Playlist,
                    id: pl.id.clone(),
                    title: pl.name.clone(),
                    subtitle: pl
                        .track_count
                        .map(|n| format!("{n} {}", s.tracks))
                        .unwrap_or_else(|| s.playlist.to_string()),
                    thumbnail_url: pl.thumbnail_url.clone(),
                    track: None,
                };
                grid = grid.child(self.render_card_tile(&format!("srv-pl-{i}"), &card, card_w, cx));
            }
            body = body
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(s.playlists),
                )
                .child(grid);
        }

        if !self.snapshot.server_albums.is_empty() {
            let server_albums = self.snapshot.server_albums.clone();
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, alb) in server_albums.iter().enumerate() {
                if !filter.is_empty()
                    && !alb.title.to_ascii_lowercase().contains(filter)
                    && !alb.subtitle.to_ascii_lowercase().contains(filter)
                {
                    continue;
                }
                grid = grid.child(self.render_card_tile(&format!("srv-alb-{i}"), alb, card_w, cx));
            }
            body = body
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(s.albums),
                )
                .child(grid);
        }

        if !self.snapshot.server_artists.is_empty() {
            let server_artists = self.snapshot.server_artists.clone();
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, art) in server_artists.iter().enumerate() {
                if !filter.is_empty() && !art.title.to_ascii_lowercase().contains(filter) {
                    continue;
                }
                grid = grid.child(self.render_card_tile(&format!("srv-art-{i}"), art, card_w, cx));
            }
            body = body
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(s.artists),
                )
                .child(grid);
        }

        if !self.snapshot.server_tracks.is_empty() {
            let tracks: Vec<_> = self
                .snapshot
                .server_tracks
                .iter()
                .filter(|t| {
                    filter.is_empty()
                        || t.title.to_ascii_lowercase().contains(filter)
                        || t.artist.to_ascii_lowercase().contains(filter)
                })
                .cloned()
                .collect();
            let play_tracks = tracks.clone();
            let tracks_arc: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(tracks.as_slice());
            let mut rows = div().flex().flex_col().gap(px(2.0));
            for i in 0..tracks_arc.len() {
                rows = rows.child(self.render_track_row(
                    "srv-tr",
                    i,
                    &tracks_arc,
                    TrackRowOpts {
                        show_album: true,
                        ..Default::default()
                    },
                    cx,
                ));
            }
            body = body
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(s.tracks),
                        )
                        .when(!play_tracks.is_empty(), |r| {
                            r.child(
                                Button::new("srv-play-tracks")
                                    .primary()
                                    .small()
                                    .icon(IconName::Play)
                                    .label(s.play)
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: play_tracks.clone(),
                                            start_index: 0,
                                            shuffle: false,
                                        });
                                    })),
                            )
                        }),
                )
                .child(rows);
        }

        body.into_any_element()
    }
}
