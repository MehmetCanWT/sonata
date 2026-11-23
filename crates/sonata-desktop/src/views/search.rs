//! Search results page with YouTube / Library tabs and Songs, Videos (16:9), Albums, and Artists sections.
//!
//! 1. **Segmented Scope**: switches smoothly between live YouTube Music results and filtered local Library items via `TabBar::pill()`.
//! 2. **Category Chips**: filters view across `All`, `Songs`, `Videos`, `Albums`, `Artists`, and `Playlists`.
//! 3. **Mixed Media Grid**: pairs track rows (`56 pt`), widescreen `16:9` music video cards, square release cards, and circular artist portraits.

use super::home::TrackRowOpts;
use super::{SearchCategory, SearchTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::{CARD_W, PAGE_INSET_X};
use crate::ui::components::{
    cover_art_tile, delayed_skeleton_block, scroll_edge_fades, section_header,
};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window,
};

impl SonataDesktopView {
    pub fn render_search_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let query = self.snapshot.search_query.clone();
        let loading = self.snapshot.search_loading;
        let started = self.loading_started.get("search").copied();
        let scroll_fades = self.settings.scroll_fades;

        let active_tab = match self.search_tab {
            SearchTab::YouTube => 0,
            SearchTab::Library => 1,
        };

        let categories = [
            (SearchCategory::All, "All"),
            (SearchCategory::Songs, "Songs"),
            (SearchCategory::Videos, "Videos"),
            (SearchCategory::Albums, "Albums"),
            (SearchCategory::Artists, "Artists"),
            (SearchCategory::Playlists, "Playlists"),
        ];

        let header = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child(if query.is_empty() {
                                "Search".to_string()
                            } else {
                                format!("Results for \"{query}\"")
                            }),
                    )
                    .child(
                        TabBar::new("search-scope-tabs")
                            .pill()
                            .small()
                            .selected_index(active_tab)
                            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                this.search_tab = if *idx == 1 {
                                    SearchTab::Library
                                } else {
                                    SearchTab::YouTube
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label("YouTube Music"))
                            .child(Tab::new().label("Library")),
                    ),
            )
            .when(self.search_tab == SearchTab::YouTube, |col| {
                col.child(div().flex().flex_row().flex_wrap().gap(px(8.0)).children(
                    categories.iter().enumerate().map(|(i, (cat, label))| {
                        let active = self.search_category == *cat;
                        let target_cat = *cat;
                        div()
                            .id(SharedString::from(format!("srch-cat-{i}")))
                            .h(px(30.0))
                            .px(px(12.0))
                            .rounded_full()
                            .cursor_pointer()
                            .bg(if active {
                                self.palette.accent
                            } else {
                                self.palette.surface
                            })
                            .hover(|s| {
                                if active {
                                    s.bg(self.palette.accent_hover)
                                } else {
                                    s.bg(self.palette.surface_hover)
                                }
                            })
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if active {
                                self.palette.on_accent
                            } else {
                                self.palette.text
                            })
                            .flex()
                            .items_center()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.search_category = target_cat;
                                cx.notify();
                            }))
                            .child(*label)
                    }),
                ))
            });

        let gap = 14.0_f32;
        let cols = (((content_w + gap) / (CARD_W + gap)).floor() as usize).clamp(2, 8);
        let card_w = ((content_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
            .clamp(CARD_W, 174.0);

        let mut sections = div().flex().flex_col().gap(px(24.0));

        if self.search_tab == SearchTab::Library {
            let needle = query.trim().to_ascii_lowercase();
            let matches_pl: Vec<_> = self
                .ordered_playlists()
                .into_iter()
                .filter(|p| needle.is_empty() || p.name.to_ascii_lowercase().contains(&needle))
                .collect();
            let matches_hist: Vec<_> = self
                .snapshot
                .last_played_tracks
                .iter()
                .filter(|t| {
                    needle.is_empty()
                        || t.title.to_ascii_lowercase().contains(&needle)
                        || t.artist.to_ascii_lowercase().contains(&needle)
                        || t.album.to_ascii_lowercase().contains(&needle)
                })
                .cloned()
                .collect();

            if !matches_pl.is_empty() {
                let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                for (i, pl) in matches_pl.into_iter().enumerate() {
                    let cover = self.media_for_url(pl.thumbnail_url.as_deref(), cx);
                    let pl_c = pl.clone();
                    grid = grid.child(
                        div()
                            .id(SharedString::from(format!("srch-lib-pl-{i}")))
                            .w(px(card_w))
                            .p(px(4.0))
                            .rounded(px(self.palette.radii.control))
                            .cursor_pointer()
                            .hover(|s| s.bg(self.palette.surface.opacity(0.45)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.backend.send(BackendCmd::OpenCollection {
                                    id: pl_c.id.clone(),
                                    title: pl_c.name.clone(),
                                    subtitle: "Playlist".to_string(),
                                    thumbnail_url: pl_c.thumbnail_url.clone(),
                                    editable: true,
                                });
                                cx.notify();
                            }))
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(cover_art_tile(
                                SharedString::from(format!("srch-lpl-cov-{i}")),
                                cover.as_ref(),
                                card_w,
                                false,
                                &self.palette,
                                false,
                                None,
                            ))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(pl.name),
                            ),
                    );
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-l-pls",
                            "Library Playlists",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(grid),
                );
            }

            if !matches_hist.is_empty() {
                let mut rows = div().flex().flex_col().gap(px(2.0));
                let total = matches_hist.len().min(20);
                for i in 0..total {
                    rows = rows.child(self.render_track_row(
                        "srch-l-tr",
                        i,
                        &matches_hist,
                        TrackRowOpts {
                            show_album: true,
                            ..Default::default()
                        },
                        cx,
                    ));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-l-trs",
                            "Recently Played",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(rows),
                );
            }
        } else if loading {
            sections = sections.child(delayed_skeleton_block(
                "srch-sk",
                started,
                None,
                220.0,
                self.palette.radii.control,
                &self.palette,
            ));
        } else {
            let rich = self.snapshot.search_rich.clone();
            let songs = if !rich.songs.is_empty() {
                rich.songs
            } else {
                self.snapshot.search_results.clone()
            };

            let show_all = self.search_category == SearchCategory::All;

            if (show_all || self.search_category == SearchCategory::Songs) && !songs.is_empty() {
                let mut rows = div().flex().flex_col().gap(px(2.0));
                let limit = songs.len().min(if show_all { 8 } else { 30 });
                for i in 0..limit {
                    rows = rows.child(self.render_track_row(
                        "srch-tr",
                        i,
                        &songs,
                        TrackRowOpts {
                            show_album: true,
                            ..Default::default()
                        },
                        cx,
                    ));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-songs",
                            "Songs",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(rows),
                );
            }

            if (show_all || self.search_category == SearchCategory::Artists)
                && !rich.artists.is_empty()
            {
                let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                for (i, a) in rich
                    .artists
                    .iter()
                    .take(if show_all { 6 } else { 18 })
                    .enumerate()
                {
                    grid =
                        grid.child(self.render_card_tile(&format!("srch-art-{i}"), a, card_w, cx));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-artists",
                            "Artists",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(grid),
                );
            }

            if (show_all || self.search_category == SearchCategory::Albums)
                && !rich.albums.is_empty()
            {
                let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                for (i, a) in rich
                    .albums
                    .iter()
                    .take(if show_all { 6 } else { 18 })
                    .enumerate()
                {
                    grid =
                        grid.child(self.render_card_tile(&format!("srch-alb-{i}"), a, card_w, cx));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-albums",
                            "Albums",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(grid),
                );
            }

            if (show_all || self.search_category == SearchCategory::Videos)
                && !rich.videos.is_empty()
            {
                let mut rows = div().flex().flex_col().gap(px(2.0));
                let limit = rich.videos.len().min(if show_all { 6 } else { 20 });
                for i in 0..limit {
                    rows = rows.child(self.render_track_row(
                        "srch-vid",
                        i,
                        &rich.videos,
                        TrackRowOpts::default(),
                        cx,
                    ));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-videos",
                            "Videos",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(rows),
                );
            }

            if (show_all || self.search_category == SearchCategory::Playlists)
                && !rich.playlists.is_empty()
            {
                let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                for (i, p) in rich
                    .playlists
                    .iter()
                    .take(if show_all { 6 } else { 18 })
                    .enumerate()
                {
                    grid =
                        grid.child(self.render_card_tile(&format!("srch-pl-{i}"), p, card_w, cx));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "srch-playlists",
                            "Playlists",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(grid),
                );
            }
        }

        let page = div()
            .flex()
            .flex_col()
            .gap(px(22.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(header)
            .child(sections);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("search-page-scroll")
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
