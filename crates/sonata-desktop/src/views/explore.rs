//! Explore (New Releases, Moods & Genres tiles, Charts), Mood Category, Shelf `Show all`, and Radio views.
//!
//! 1. **Explore Page**: renders quick discovery tiles (`MoodCategoryTile` color-accented pills), `Music videos`, `New albums & singles`, and `Charts`.
//! 2. **Mood Detail Page**: displays shelves returned for any clicked mood or genre tile.
//! 3. **Shelf Detail Page**: expands any carousel ("Show all") into a responsive multi-column grid or track list.
//! 4. **Radio Page**: renders curated radio mix shelves plus a Quick Radio launcher.

use super::home::TrackRowOpts;
use super::SonataDesktopView;
use crate::backend::BackendCmd;
use crate::settings::{CARD_W, PAGE_INSET_X};
use crate::theme::ramp::{hsla_from_rgb, Rgb};
use crate::ui::components::{delayed_skeleton_block, scroll_edge_fades, section_header};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

impl SonataDesktopView {
    pub fn render_explore_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let shelves = self.snapshot.explore_shelves.clone();
        let mood_tiles = self.snapshot.mood_tiles.clone();
        let loading = self.snapshot.explore_loading;
        let started = self.loading_started.get("explore").copied();
        let scroll_fades = self.settings.scroll_fades;

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(26.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(
                div()
                    .text_size(px(26.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.palette.text)
                    .child("Explore"),
            );

        if loading && shelves.is_empty() && mood_tiles.is_empty() {
            page = page
                .child(delayed_skeleton_block(
                    "exp-sk-1",
                    started,
                    None,
                    120.0,
                    self.palette.radii.control,
                    &self.palette,
                ))
                .child(delayed_skeleton_block(
                    "exp-sk-2",
                    started,
                    None,
                    180.0,
                    self.palette.radii.control,
                    &self.palette,
                ));
        } else {
            if !mood_tiles.is_empty() {
                let mut tiles_grid = div().flex().flex_row().flex_wrap().gap(px(10.0));
                for (i, tile) in mood_tiles.into_iter().enumerate() {
                    let stripe = hsla_from_rgb(
                        Rgb::from_u8(tile.accent_rgb.0, tile.accent_rgb.1, tile.accent_rgb.2),
                        1.0,
                    );
                    let title = tile.title.clone();
                    let browse_id = tile.browse_id.clone();
                    let params = tile.params.clone();
                    tiles_grid = tiles_grid.child(
                        div()
                            .id(SharedString::from(format!("mood-tile-{i}")))
                            .w(px(176.0))
                            .h(px(46.0))
                            .rounded(px(self.palette.radii.item))
                            .bg(self.palette.surface)
                            .hover(|s| s.bg(self.palette.surface_hover))
                            .overflow_hidden()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.fullscreen_open = false;
                                this.backend.send(BackendCmd::OpenMoodCategory {
                                    title: title.clone(),
                                    browse_id: browse_id.clone(),
                                    params: params.clone(),
                                });
                                cx.notify();
                            }))
                            .child(div().w(px(6.0)).h_full().bg(stripe))
                            .child(
                                div()
                                    .px(px(12.0))
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .truncate()
                                    .child(tile.title),
                            ),
                    );
                }
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "exp-moods",
                            "Moods & genres",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(tiles_grid),
                );
            }

            for (i, shelf) in shelves.into_iter().enumerate() {
                if !shelf.cards.is_empty() {
                    page = page.child(self.render_cards_shelf(
                        &format!("exp-sh-{i}"),
                        shelf,
                        content_w,
                        cx,
                    ));
                } else if !shelf.tracks.is_empty() {
                    let tracks = shelf.tracks.clone();
                    let mut rows = div().flex().flex_col().gap(px(2.0));
                    for t_i in 0..tracks.len().min(12) {
                        rows = rows.child(self.render_track_row(
                            &format!("exp-tr-{i}"),
                            t_i,
                            &tracks,
                            TrackRowOpts {
                                show_album: true,
                                ..Default::default()
                            },
                            cx,
                        ));
                    }
                    page = page.child(
                        div()
                            .flex()
                            .flex_col()
                            .child(section_header(
                                &format!("exp-ts-{i}"),
                                &shelf.title,
                                None,
                                None,
                                &self.palette,
                            ))
                            .child(rows),
                    );
                }
            }
        }

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("explore-page-scroll")
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

    pub fn render_mood_category_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let title = self.snapshot.mood_page_title.clone();
        let shelves = self.snapshot.mood_page_shelves.clone();
        let loading = self.snapshot.mood_page_loading;
        let scroll_fades = self.settings.scroll_fades;

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(
                div()
                    .text_size(px(26.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.palette.text)
                    .child(title),
            );

        if loading && shelves.is_empty() {
            page = page.child(
                div()
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child("Loading category..."),
            );
        } else {
            for (i, shelf) in shelves.into_iter().enumerate() {
                if !shelf.cards.is_empty() {
                    page = page.child(self.render_cards_shelf(
                        &format!("mood-sh-{i}"),
                        shelf,
                        content_w,
                        cx,
                    ));
                }
            }
        }

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("mood-page-scroll")
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

    pub fn render_shelf_detail_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(shelf) = self.snapshot.shelf_detail.clone() else {
            return div()
                .p(px(24.0))
                .child("Shelf unavailable")
                .into_any_element();
        };
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);
        let scroll_fades = self.settings.scroll_fades;

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .child(
                div()
                    .text_size(px(26.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.palette.text)
                    .child(shelf.title.clone()),
            );

        if !shelf.tracks.is_empty() {
            let tracks = shelf.tracks.clone();
            let mut rows = div().flex().flex_col().gap(px(2.0));
            for i in 0..tracks.len() {
                rows = rows.child(self.render_track_row(
                    "shd-tr",
                    i,
                    &tracks,
                    TrackRowOpts {
                        show_album: true,
                        ..Default::default()
                    },
                    cx,
                ));
            }
            page = page.child(rows);
        }

        if !shelf.cards.is_empty() {
            let gap = 14.0_f32;
            let cols = (((content_w + gap) / (CARD_W + gap)).floor() as usize).clamp(2, 8);
            let card_w = ((content_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
                .clamp(CARD_W, 176.0);
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, c) in shelf.cards.iter().enumerate() {
                grid = grid.child(self.render_card_tile(&format!("shd-c-{i}"), c, card_w, cx));
            }
            page = page.child(grid);
        }

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("shelf-detail-scroll")
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

    pub fn render_radio_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let shelves = self.snapshot.radio_shelves.clone();
        let loading = self.snapshot.radio_loading;
        let started = self.loading_started.get("radio").copied();
        let scroll_fades = self.settings.scroll_fades;

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(24.0))
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
                            .child("Radio & Mixes"),
                    )
                    .child(
                        Button::new("radio-quick-start")
                            .primary()
                            .small()
                            .icon(IconName::Radio)
                            .label("Quick Radio")
                            .on_click(cx.listener(|this, _, _, _| {
                                this.backend.send(BackendCmd::StartQuickRadio);
                            })),
                    ),
            );

        if loading && shelves.is_empty() {
            page = page.child(delayed_skeleton_block(
                "rad-sk-1",
                started,
                None,
                160.0,
                self.palette.radii.control,
                &self.palette,
            ));
        } else {
            for (i, shelf) in shelves.into_iter().enumerate() {
                if !shelf.cards.is_empty() {
                    page = page.child(self.render_cards_shelf(
                        &format!("rad-sh-{i}"),
                        shelf,
                        content_w,
                        cx,
                    ));
                }
            }
        }

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("radio-page-scroll")
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

#[allow(dead_code)]
fn _keep_icon(_: Icon) {}
