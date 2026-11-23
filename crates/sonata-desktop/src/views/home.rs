//! Home feed with mood filter chips, 3-column `Quick picks` grid, horizontal carousels, and shared shelf/row helpers.
//!
//! 1. **Home Feed**: renders `chipCloud` mood pills (with active clear), `Quick picks` compact 3-column grid, horizontal `HomeShelf` rows with `Show all`, and `150 ms`-delayed skeletons.
//! 2. **Shared Card Grid (`render_card_tile`)**: renders square Release/Playlist cards, circular Artist portraits, and 16:9 Video thumbnails with hover `play_overlay` and right-click context menus.
//! 3. **Shared Track Row (`render_track_row`)**: renders standardized `56 pt` rows with index/artwork, hover `play_overlay`, explicit `[E]` tag, clickable artist & album links, like/queue buttons, and duration.

use super::{ContextMenuKind, ContextMenuState, SonataDesktopView};
use crate::backend::{ytm_extra::MoodChip, BackendCmd};
use crate::settings::{ART_SIZE, CARD_STRETCH_MAX, CARD_W, PAGE_INSET_X, ROW_HEIGHT};
use crate::ui::components::{
    artist_links_row, cover_art_tile, delayed_skeleton_block, explicit_badge, mood_art_tile,
    scroll_edge_fades, section_header, wide_thumb_tile,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};
use sonata_core::providers::ytmusic::{HomeCard, HomeCardKind, HomeShelf};
use sonata_core::providers::{format_duration, Track};
use std::rc::Rc;

impl SonataDesktopView {
    pub fn render_home_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let chips = self.snapshot.home_chips.clone();
        let active_chip = self.snapshot.active_home_chip.clone();
        let shelves = self.snapshot.home_shelves.clone();
        let loading = self.snapshot.home_loading;
        let error = self.snapshot.home_error.clone();
        let started = self.loading_started.get("home").copied();
        let scroll_fades = self.settings.scroll_fades;

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(26.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0));

        // Mood filter chips bar
        if !chips.is_empty() {
            let mut chip_row = div()
                .flex()
                .flex_row()
                .items_center()
                .flex_wrap()
                .gap(px(8.0));
            for (idx, chip) in chips.into_iter().enumerate() {
                let is_sel = active_chip
                    .as_ref()
                    .is_some_and(|c| c.params == chip.params);
                let chip_click = chip.clone();
                chip_row = chip_row.child(
                    div()
                        .id(SharedString::from(format!("home-chip-{idx}")))
                        .h(px(32.0))
                        .px(px(14.0))
                        .rounded_full()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .bg(if is_sel {
                            self.palette.accent
                        } else {
                            self.palette.surface
                        })
                        .hover(|s| {
                            if is_sel {
                                s.bg(self.palette.accent_hover)
                            } else {
                                s.bg(self.palette.surface_hover)
                            }
                        })
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if is_sel {
                            self.palette.on_accent
                        } else {
                            self.palette.text
                        })
                        .on_click(cx.listener(move |this, _, _, _| {
                            let next = if is_sel {
                                None
                            } else {
                                Some(chip_click.clone())
                            };
                            this.backend.send(BackendCmd::SelectHomeChip(next));
                        }))
                        .child(chip.label.clone())
                        .when(is_sel, |c| c.child(Icon::new(IconName::X).size(px(13.0)))),
                );
            }
            page = page.child(chip_row);
        }

        if let Some(err) = error {
            page = page.child(
                div()
                    .p(px(14.0))
                    .rounded(px(self.palette.radii.control))
                    .bg(self.palette.surface)
                    .border_1()
                    .border_color(self.palette.danger.opacity(0.45))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(self.palette.text)
                            .child(err),
                    )
                    .child(
                        Button::new("home-retry")
                            .primary()
                            .small()
                            .label("Retry")
                            .on_click(cx.listener(|this, _, _, _| {
                                this.backend.send(BackendCmd::RefreshHome);
                            })),
                    ),
            );
        }

        if loading && shelves.is_empty() {
            for idx in 0..3 {
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .child(delayed_skeleton_block(
                            SharedString::from(format!("h-sk-t-{idx}")),
                            started,
                            Some(180.0),
                            22.0,
                            self.palette.radii.small,
                            &self.palette,
                        ))
                        .child(delayed_skeleton_block(
                            SharedString::from(format!("h-sk-r-{idx}")),
                            started,
                            None,
                            164.0,
                            self.palette.radii.control,
                            &self.palette,
                        )),
                );
            }
        } else {
            for (s_idx, shelf) in shelves.into_iter().enumerate() {
                if !shelf.tracks.is_empty() && shelf.cards.is_empty() {
                    page = page.child(self.render_quick_picks_shelf(s_idx, shelf, content_w, cx));
                } else if !shelf.cards.is_empty() {
                    page = page.child(self.render_cards_shelf(
                        &format!("home-sh-{s_idx}"),
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
                    .id("home-page-scroll")
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

    fn render_quick_picks_shelf(
        &mut self,
        shelf_idx: usize,
        shelf: HomeShelf,
        content_w: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cols = if content_w >= 840.0 {
            3
        } else if content_w >= 540.0 {
            2
        } else {
            1
        };
        let rows_per_col = 4usize;
        let show_all_shelf = shelf.clone();
        let all_tracks = shelf.tracks.clone();
        let active_id = self
            .snapshot
            .current_track()
            .map(|t| t.id.clone())
            .unwrap_or_default();

        let header = section_header(
            &format!("qp-{shelf_idx}"),
            &shelf.title,
            Some("START RADIO FROM A SONG"),
            Some(Rc::new({
                let b = self.backend.clone();
                let sh = show_all_shelf;
                move |_, _| b.send(BackendCmd::OpenShelfDetail(sh.clone()))
            })),
            &self.palette,
        );

        let mut grid = div().w_full().flex().flex_row().gap(px(14.0));
        for col_i in 0..cols {
            let start = col_i * rows_per_col;
            if start >= all_tracks.len() {
                break;
            }
            let end = (start + rows_per_col).min(all_tracks.len());
            let mut col_box = div().flex_1().min_w_0().flex().flex_col().gap(px(4.0));
            for (row_offset, track) in all_tracks[start..end].iter().enumerate() {
                let abs_idx = start + row_offset;
                let is_cur = track.id == active_id;
                let cover = self.media_for_url(track.thumbnail_url.as_deref(), cx);
                let play_tracks = all_tracks.clone();
                let play_track_dbl = track.clone();
                let track_menu = track.clone();
                let track_alb = track.clone();
                let dbl = self.settings.double_click_to_play;

                col_box = col_box.child(
                    div()
                        .id(SharedString::from(format!("qp-{shelf_idx}-{abs_idx}")))
                        .h(px(ROW_HEIGHT))
                        .px(px(8.0))
                        .rounded(px(self.palette.radii.item))
                        .cursor_pointer()
                        .bg(if is_cur {
                            self.palette.surface_active
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .hover(|s| {
                            if is_cur {
                                s
                            } else {
                                s.bg(self.palette.surface_hover)
                            }
                        })
                        .on_click(cx.listener(move |this, ev: &gpui_kit::ClickEvent, _, _| {
                            if !dbl || ev.click_count() >= 2 {
                                this.backend.send(BackendCmd::PlayTracks {
                                    tracks: play_tracks.clone(),
                                    start_index: abs_idx,
                                    shuffle: false,
                                });
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.context_menu = Some(ContextMenuState {
                                    pos: gpui_kit::point(
                                        ev.position.x.into(),
                                        ev.position.y.into(),
                                    ),
                                    kind: ContextMenuKind::Track {
                                        track: track_menu.clone(),
                                        playlist_remove_idx: None,
                                    },
                                });
                                cx.notify();
                            }),
                        )
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(cover_art_tile(
                            SharedString::from(format!("qp-cov-{shelf_idx}-{abs_idx}")),
                            cover.as_ref(),
                            44.0,
                            false,
                            &self.palette,
                            is_cur && !self.snapshot.paused,
                            Some(Rc::new({
                                let b = self.backend.clone();
                                let tr = play_track_dbl;
                                move |_, _| {
                                    if is_cur {
                                        b.send(BackendCmd::TogglePause);
                                    } else {
                                        b.send(BackendCmd::PlayTracks {
                                            tracks: vec![tr.clone()],
                                            start_index: 0,
                                            shuffle: false,
                                        });
                                    }
                                }
                            })),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(if is_cur {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(if is_cur {
                                                    self.palette.accent
                                                } else {
                                                    self.palette.text
                                                })
                                                .truncate()
                                                .child(track.title.clone()),
                                        )
                                        .when(track.is_explicit, |r| {
                                            r.child(explicit_badge(&self.palette))
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.0))
                                        .min_w_0()
                                        .child(artist_links_row(
                                            &format!("qp-art-{shelf_idx}-{abs_idx}"),
                                            &track.artist,
                                            self.palette.secondary,
                                            12.0,
                                            {
                                                let view = cx.entity().downgrade();
                                                move |name, _, cx| {
                                                    let _ = view.update(cx, |this, cx| {
                                                        this.open_artist_by_name(&name, cx);
                                                    });
                                                }
                                            },
                                        ))
                                        .when(!track_alb.album.is_empty(), |r| {
                                            let alb = track_alb.clone();
                                            r.child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(self.palette.dim)
                                                    .child("•"),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "qp-alb-{shelf_idx}-{abs_idx}"
                                                    )))
                                                    .text_size(px(12.0))
                                                    .text_color(self.palette.dim)
                                                    .truncate()
                                                    .cursor_pointer()
                                                    .hover(|s| s.underline())
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.open_album_for_track(&alb, cx);
                                                        }),
                                                    )
                                                    .child(track.album.clone()),
                                            )
                                        }),
                                ),
                        ),
                );
            }
            grid = grid.child(col_box);
        }

        div()
            .flex()
            .flex_col()
            .child(header)
            .child(grid)
            .into_any_element()
    }

    pub fn render_cards_shelf(
        &mut self,
        id_prefix: &str,
        shelf: HomeShelf,
        content_w: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let gap = 14.0_f32;
        let max_cards = (((content_w + gap) / (CARD_W + gap)).floor() as usize).clamp(2, 8);
        let card_w = ((content_w - gap * (max_cards.saturating_sub(1) as f32)) / max_cards as f32)
            .clamp(CARD_W, CARD_W * CARD_STRETCH_MAX);

        let show_all_shelf = shelf.clone();
        let has_more = shelf.cards.len() > max_cards;
        let header = section_header(
            id_prefix,
            &shelf.title,
            None,
            if has_more || shelf.cards.len() >= 4 {
                Some(Rc::new({
                    let b = self.backend.clone();
                    move |_, _| b.send(BackendCmd::OpenShelfDetail(show_all_shelf.clone()))
                }))
            } else {
                None
            },
            &self.palette,
        );

        let all_videos =
            !shelf.cards.is_empty() && shelf.cards.iter().all(|c| c.kind == HomeCardKind::Video);
        let mut row = div().w_full().flex().flex_row().gap(px(gap));
        for (i, card) in shelf.cards.iter().take(max_cards).enumerate() {
            row = row.child(self.render_card_tile_inner(
                &format!("{id_prefix}-c-{i}"),
                card,
                card_w,
                all_videos,
                cx,
            ));
        }

        div()
            .flex()
            .flex_col()
            .child(header)
            .child(row)
            .into_any_element()
    }

    pub fn render_card_tile(
        &mut self,
        id: &str,
        card: &HomeCard,
        card_w: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_card_tile_inner(id, card, card_w, false, cx)
    }

    fn render_card_tile_inner(
        &mut self,
        id: &str,
        card: &HomeCard,
        card_w: f32,
        allow_wide_video: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_artist = card.kind == HomeCardKind::Artist;
        let is_video = allow_wide_video && card.kind == HomeCardKind::Video;
        let cover = self.media_for_url(card.thumbnail_url.as_deref(), cx);
        let active_id = self
            .snapshot
            .current_track()
            .map(|t| t.id.as_str())
            .unwrap_or("");
        let is_playing = card
            .track
            .as_ref()
            .is_some_and(|t| t.id == active_id && !self.snapshot.paused);

        let open_card = card.clone();
        let play_card = card.clone();
        let menu_card = card.clone();
        let vid_w = if is_video {
            (card_w * 1.45).min(240.0)
        } else {
            card_w
        };
        let art_w = (vid_w - 8.0).max(64.0);
        let vid_h = (art_w * 9.0 / 16.0).round();

        let artwork = if is_video {
            wide_thumb_tile(
                SharedString::from(format!("{id}-wide")),
                cover.as_ref(),
                art_w,
                vid_h,
                &self.palette,
                is_playing,
                Some(Rc::new({
                    let b = self.backend.clone();
                    let tr = play_card.track.clone();
                    let cid = play_card.id.clone();
                    let ttl = play_card.title.clone();
                    let sub = play_card.subtitle.clone();
                    let thumb = play_card.thumbnail_url.clone();
                    move |_, _| {
                        if let Some(ref t) = tr {
                            b.send(BackendCmd::PlayTracks {
                                tracks: vec![t.clone()],
                                start_index: 0,
                                shuffle: false,
                            });
                        } else {
                            b.send(BackendCmd::OpenCollection {
                                id: cid.clone(),
                                title: ttl.clone(),
                                subtitle: sub.clone(),
                                thumbnail_url: thumb.clone(),
                                editable: false,
                            });
                        }
                    }
                })),
            )
        } else if cover.is_none()
            && (card.kind == HomeCardKind::Mood
                || card.thumbnail_url.as_deref().is_none_or(str::is_empty))
        {
            mood_art_tile(
                SharedString::from(format!("{id}-mood")),
                &card.title,
                art_w,
                &self.palette,
                is_playing,
                if is_artist {
                    None
                } else {
                    Some(Rc::new({
                        let b = self.backend.clone();
                        let card_p = play_card.clone();
                        move |_, _| trigger_card_action(&b, &card_p)
                    }))
                },
            )
        } else {
            cover_art_tile(
                SharedString::from(format!("{id}-art")),
                cover.as_ref(),
                art_w,
                is_artist,
                &self.palette,
                is_playing,
                if is_artist {
                    None
                } else {
                    Some(Rc::new({
                        let b = self.backend.clone();
                        let card_p = play_card.clone();
                        move |_, _| trigger_card_action(&b, &card_p)
                    }))
                },
            )
        };

        div()
            .id(SharedString::from(id.to_string()))
            .w(px(vid_w))
            .flex_shrink_0()
            .cursor_pointer()
            .rounded(px(self.palette.radii.control))
            .p(px(4.0))
            .hover(|s| s.bg(self.palette.surface.opacity(0.45)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_home_card(&open_card, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let pos = gpui_kit::point(ev.position.x.into(), ev.position.y.into());
                    let kind = if let Some(ref t) = menu_card.track {
                        ContextMenuKind::Track {
                            track: t.clone(),
                            playlist_remove_idx: None,
                        }
                    } else if menu_card.kind == HomeCardKind::Artist {
                        ContextMenuKind::Artist {
                            id: menu_card.id.clone(),
                            name: menu_card.title.clone(),
                        }
                    } else {
                        ContextMenuKind::Collection {
                            id: menu_card.id.clone(),
                            title: menu_card.title.clone(),
                            subtitle: menu_card.subtitle.clone(),
                            thumbnail_url: menu_card.thumbnail_url.clone(),
                            is_album: menu_card.kind == HomeCardKind::Album,
                        }
                    };
                    this.context_menu = Some(ContextMenuState { pos, kind });
                    cx.notify();
                }),
            )
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(artwork)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .when(is_artist, |c| c.items_center())
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text)
                            .truncate()
                            .child(card.title.clone()),
                    )
                    .when(!card.subtitle.is_empty(), |c| {
                        c.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(self.palette.secondary)
                                .truncate()
                                .child(card.subtitle.clone()),
                        )
                    }),
            )
            .into_any_element()
    }

    pub fn open_home_card(&mut self, card: &HomeCard, cx: &mut Context<Self>) {
        self.fullscreen_open = false;
        trigger_card_action(&self.backend, card);
        cx.notify();
    }
}

#[derive(Clone, Copy, Default)]
pub struct TrackRowOpts {
    pub show_index: bool,
    pub show_thumb: bool,
    pub show_album: bool,
    pub playlist_remove_idx: Option<usize>,
}

impl SonataDesktopView {
    pub fn render_track_row(
        &mut self,
        id_prefix: &str,
        index: usize,
        all_tracks: &[Track],
        opts: TrackRowOpts,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let track = &all_tracks[index];
        let show_index = opts.show_index;
        let show_thumb = opts.show_thumb || !show_index;
        let show_album = opts.show_album;
        let playlist_remove_idx = opts.playlist_remove_idx;
        let is_liked = self.snapshot.is_track_liked(track);
        let is_cur = self
            .snapshot
            .current_track()
            .is_some_and(|t| t.id == track.id);
        let cover = if show_thumb {
            self.media_for_url(track.thumbnail_url.as_deref(), cx)
        } else {
            None
        };
        let play_tracks = all_tracks.to_vec();
        let track_like = track.clone();
        let track_menu = track.clone();
        let track_more = track.clone();
        let track_alb = track.clone();
        let dbl = self.settings.double_click_to_play;

        div()
            .id(SharedString::from(format!("{id_prefix}-{index}")))
            .group(SharedString::from(format!("{id_prefix}-grp-{index}")))
            .h(px(ROW_HEIGHT))
            .px(px(10.0))
            .rounded(px(self.palette.radii.item))
            .cursor_pointer()
            .bg(if is_cur {
                self.palette.surface_active
            } else {
                gpui_kit::transparent_black()
            })
            .hover(|s| {
                if is_cur {
                    s
                } else {
                    s.bg(self.palette.surface_hover)
                }
            })
            .on_click(cx.listener(move |this, ev: &gpui_kit::ClickEvent, _, _| {
                if !dbl || ev.click_count() >= 2 {
                    this.backend.send(BackendCmd::PlayTracks {
                        tracks: play_tracks.clone(),
                        start_index: index,
                        shuffle: false,
                    });
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.context_menu = Some(ContextMenuState {
                        pos: gpui_kit::point(ev.position.x.into(), ev.position.y.into()),
                        kind: ContextMenuKind::Track {
                            track: track_menu.clone(),
                            playlist_remove_idx,
                        },
                    });
                    cx.notify();
                }),
            )
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .when(show_index, |row| {
                row.child(
                    div()
                        .w(px(28.0))
                        .flex_shrink_0()
                        .flex()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_weight(if is_cur {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if is_cur {
                            self.palette.accent
                        } else {
                            self.palette.dim
                        })
                        .child(format!("{}", index + 1)),
                )
            })
            .when(show_thumb, |row| {
                row.child(cover_art_tile(
                    SharedString::from(format!("{id_prefix}-cov-{index}")),
                    cover.as_ref(),
                    ART_SIZE,
                    false,
                    &self.palette,
                    is_cur && !self.snapshot.paused,
                    Some(Rc::new({
                        let b = self.backend.clone();
                        let tracks_c = all_tracks.to_vec();
                        move |_, _| {
                            if is_cur {
                                b.send(BackendCmd::TogglePause);
                            } else {
                                b.send(BackendCmd::PlayTracks {
                                    tracks: tracks_c.clone(),
                                    start_index: index,
                                    shuffle: false,
                                });
                            }
                        }
                    })),
                ))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(if is_cur {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_cur {
                                        self.palette.accent
                                    } else {
                                        self.palette.text
                                    })
                                    .truncate()
                                    .child(track.title.clone()),
                            )
                            .when(track.is_explicit, |r| {
                                r.child(explicit_badge(&self.palette))
                            }),
                    )
                    .child(artist_links_row(
                        &format!("{id_prefix}-a-{index}"),
                        &track.artist,
                        self.palette.secondary,
                        12.0,
                        {
                            let view = cx.entity().downgrade();
                            move |name, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.open_artist_by_name(&name, cx);
                                });
                            }
                        },
                    )),
            )
            .when(show_album && !track.album.is_empty(), |row| {
                let alb = track_alb.clone();
                row.child(
                    div()
                        .id(SharedString::from(format!("{id_prefix}-alb-{index}")))
                        .w(px(180.0))
                        .min_w(px(90.0))
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .truncate()
                        .cursor_pointer()
                        .hover(|s| s.underline().text_color(self.palette.text))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.open_album_for_track(&alb, cx);
                            }),
                        )
                        .child(track.album.clone()),
                )
            })
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .rounded_full()
                            .when(is_liked, |d| d.bg(self.palette.accent.opacity(0.18)))
                            .opacity(if is_liked { 1.0 } else { 0.0 })
                            .group_hover(
                                SharedString::from(format!("{id_prefix}-grp-{index}")),
                                |s| s.opacity(1.0),
                            )
                            .child(
                                Button::new(SharedString::from(format!("{id_prefix}-lk-{index}")))
                                    .ghost()
                                    .xsmall()
                                    .icon(if is_liked {
                                        IconName::Heart
                                    } else {
                                        IconName::ThumbsUp
                                    })
                                    .when(is_liked, |b| b.text_color(self.palette.accent))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend
                                            .send(BackendCmd::ToggleLike(track_like.clone()));
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .opacity(0.0)
                            .group_hover(
                                SharedString::from(format!("{id_prefix}-grp-{index}")),
                                |s| s.opacity(1.0),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "{id_prefix}-more-{index}"
                                )))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Ellipsis)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.modal =
                                            super::DesktopModal::AddToPlaylist(track_more.clone());
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .w(px(44.0))
                            .flex()
                            .justify_end()
                            .text_size(px(12.0))
                            .text_color(self.palette.dim)
                            .child(format_duration(track.duration_secs)),
                    ),
            )
            .into_any_element()
    }
}

fn trigger_card_action(backend: &crate::backend::BackendHandle, card: &HomeCard) {
    if let Some(ref tr) = card.track {
        backend.send(BackendCmd::PlayTracks {
            tracks: vec![tr.clone()],
            start_index: 0,
            shuffle: false,
        });
        return;
    }
    match card.kind {
        HomeCardKind::Artist => backend.send(BackendCmd::OpenArtist(card.id.clone())),
        HomeCardKind::Album => backend.send(BackendCmd::OpenAlbum {
            album_id: Some(card.id.clone()),
            title: card.title.clone(),
            artist: card.subtitle.clone(),
            thumbnail_url: card.thumbnail_url.clone(),
        }),
        HomeCardKind::Mood | HomeCardKind::Playlist => backend.send(BackendCmd::OpenCollection {
            id: card.id.clone(),
            title: card.title.clone(),
            subtitle: card.subtitle.clone(),
            thumbnail_url: card.thumbnail_url.clone(),
            editable: false,
        }),
        HomeCardKind::Song | HomeCardKind::Video => {}
    }
}

/// Keeps `MoodChip` publicly referenced for chip helpers cleanly.
#[allow(dead_code)]
fn _keep_mood_chip(_: &MoodChip) {}
