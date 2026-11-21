//! Personalized YouTube Music Home feed view for the desktop client.
//!
//! 1. **Home Feed**: `render_home_page` displays the user greeting, refresh/sign-in actions, and personalized shelves.
//! 2. **Shelves & Moods**: `render_shelf` renders mood/genre pill rows (`HomeCardKind::Mood`), album/playlist/mix cards, and playable track grids.

use super::{
    cached_thumb_path, render_cover_art, render_icon_button, ui_icon, FluentDivExt,
    SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use sonata_core::providers::format_duration;
use sonata_core::providers::ytmusic::{HomeCard, HomeCardKind, HomeShelf};
use std::sync::Arc;

pub fn render_home_page(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let greeting = if let Some(profile) = &snap.user_profile {
        format!("Welcome back, {} — your YouTube Music mix", profile.name)
    } else if snap.is_signed_in() {
        "Personalized picks from your YouTube Music session".to_string()
    } else {
        "Trending songs, new releases & community playlists on YouTube Music".to_string()
    };

    let mut page = div()
        .id("home-scroll")
        .size_full()
        .overflow_y_scroll()
        .p_6()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .child("Home"),
                        )
                        .child(div().text_xs().text_color(p.muted).child(greeting)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(!snap.is_signed_in(), |el| {
                            el.child(
                                div()
                                    .id("home-signin-cta")
                                    .px_3()
                                    .py(px(7.0))
                                    .rounded_full()
                                    .bg(p.accent)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate_to(crate::backend::NavTarget::Settings, cx);
                                    }))
                                    .child(ui_icon(IconName::LogIn, 14.0, p.accent_fg))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(p.accent_fg)
                                            .child("Sign In for Personal Feed"),
                                    ),
                            )
                        })
                        .child(render_icon_button(
                            "home-refresh-btn",
                            IconName::RefreshCw,
                            15.0,
                            32.0,
                            p.text,
                            p.pill_bg,
                            p.card_hover,
                            |this, _, _| this.backend.send(BackendCmd::RefreshHome),
                            cx,
                        )),
                ),
        );

    if snap.home_loading && snap.home_shelves.is_empty() {
        page = page.child(
            div()
                .py_12()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(ui_icon(IconName::Flame, 28.0, p.accent))
                .child(
                    div()
                        .text_sm()
                        .text_color(p.muted)
                        .child("Loading YouTube Music shelves..."),
                ),
        );
    } else if snap.home_shelves.is_empty() {
        let msg = snap
            .home_error
            .clone()
            .unwrap_or_else(|| "Could not load YouTube Music home shelves.".to_string());
        page = page.child(
            div()
                .p_6()
                .rounded_2xl()
                .bg(p.card_bg)
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(ui_icon(IconName::CircleAlert, 24.0, p.warning))
                .child(div().text_sm().text_color(p.text).child(msg)),
        );
    } else {
        for (s_idx, shelf) in snap.home_shelves.iter().enumerate() {
            page = page.child(render_shelf(view, s_idx, shelf, p, cx));
        }
    }

    page.into_any_element()
}

fn render_shelf(
    view: &SonataDesktopView,
    s_idx: usize,
    shelf: &HomeShelf,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let cur_id = view.snapshot.current_track().map(|t| t.id.as_str());
    let has_tracks = !shelf.tracks.is_empty();
    let shelf_tracks = Arc::new(shelf.tracks.clone());

    let mut container = div().w_full().flex().flex_col().gap_3().child(
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.highlight_fg)
                    .child(shelf.title.clone()),
            )
            .when(has_tracks, |el| {
                let tracks_for_play = shelf_tracks.clone();
                el.child(
                    div()
                        .id(SharedString::from(format!("shelf-play-all-{s_idx}")))
                        .px_3()
                        .py(px(4.0))
                        .rounded_full()
                        .bg(p.pill_bg)
                        .hover(|s| s.bg(p.card_hover))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_1()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::PlayTracks {
                                tracks: (*tracks_for_play).clone(),
                                start_index: 0,
                                shuffle: false,
                            });
                        }))
                        .child(ui_icon(IconName::Play, 12.0, p.accent))
                        .child(div().text_xs().text_color(p.text).child("Play All")),
                )
            }),
    );

    if !shelf.cards.is_empty() {
        let all_moods = shelf
            .cards
            .iter()
            .all(|card| card.kind == HomeCardKind::Mood);
        if all_moods {
            let chips_row = div().w_full().flex().flex_wrap().gap_2().children(
                shelf
                    .cards
                    .iter()
                    .enumerate()
                    .take(18)
                    .map(|(c_idx, card)| render_mood_chip(s_idx, c_idx, card, p, cx)),
            );
            container = container.child(chips_row);
        } else {
            let cards_row = div().w_full().flex().flex_wrap().gap_4().children(
                shelf
                    .cards
                    .iter()
                    .enumerate()
                    .take(16)
                    .map(|(c_idx, card)| render_home_card(s_idx, c_idx, card, cur_id, p, cx)),
            );
            container = container.child(cards_row);
        }
    }

    if !shelf.tracks.is_empty() {
        let mut grid = div().w_full().flex().flex_wrap().gap_2();
        for (t_idx, track) in shelf.tracks.iter().enumerate().take(16) {
            let is_playing = cur_id == Some(track.id.as_str());
            let art = cached_thumb_path(track.thumbnail_url.as_deref());
            let tracks_clone = shelf_tracks.clone();
            let dur = format_duration(track.duration_secs);

            grid = grid.child(
                div()
                    .id(SharedString::from(format!("htr-{s_idx}-{t_idx}")))
                    .w(px(300.0))
                    .p_2()
                    .rounded_xl()
                    .bg(if is_playing { p.accent_soft } else { p.card_bg })
                    .border_1()
                    .border_color(if is_playing {
                        p.accent
                    } else {
                        p.border_subtle
                    })
                    .hover(|s| s.bg(p.card_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_3()
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.backend.send(BackendCmd::PlayTracks {
                            tracks: (*tracks_clone).clone(),
                            start_index: t_idx,
                            shuffle: false,
                        });
                    }))
                    .child(render_cover_art(art.as_ref(), 46.0, 7.0, p))
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
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if is_playing { p.accent } else { p.highlight_fg })
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
                    )
                    .child(div().text_xs().text_color(p.muted).child(dur)),
            );
        }
        container = container.child(grid);
    }

    container.into_any_element()
}

fn render_mood_chip(
    s_idx: usize,
    c_idx: usize,
    card: &HomeCard,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let mood_label = card.title.clone();
    let mood_query = format!("{} music", card.title);
    div()
        .id(SharedString::from(format!("hmood-{s_idx}-{c_idx}")))
        .px_4()
        .py(px(8.0))
        .rounded_full()
        .bg(p.pill_bg)
        .border_1()
        .border_color(p.border_subtle)
        .hover(|s| s.bg(p.card_hover).border_color(p.accent))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .on_click(cx.listener(move |this, _, window, cx| {
            let query = mood_query.clone();
            this.center_mode = super::CenterMode::Page;
            this.search_input.update(cx, |input, cx| {
                input.set_value(&query, window, cx);
            });
            this.backend.send(BackendCmd::Search(query));
            cx.notify();
        }))
        .child(div().size(px(6.0)).rounded_full().bg(p.accent))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text)
                .child(mood_label),
        )
        .into_any_element()
}

fn render_home_card(
    s_idx: usize,
    c_idx: usize,
    card: &HomeCard,
    cur_id: Option<&str>,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let art = cached_thumb_path(card.thumbnail_url.as_deref());
    let card_clone = card.clone();
    let is_playing_card = card
        .track
        .as_ref()
        .is_some_and(|t| cur_id == Some(t.id.as_str()));
    let kind_badge = match card.kind {
        HomeCardKind::Album => "Album",
        HomeCardKind::Playlist => "Playlist",
        HomeCardKind::Song => "Song",
        HomeCardKind::Video => "Video",
        HomeCardKind::Artist => "Artist",
        HomeCardKind::Mood => "Mood",
    };

    div()
        .id(SharedString::from(format!("hcard-{s_idx}-{c_idx}")))
        .w(px(164.0))
        .p(px(10.0))
        .rounded_2xl()
        .bg(if is_playing_card {
            p.accent_soft
        } else {
            p.card_bg
        })
        .border_1()
        .border_color(if is_playing_card {
            p.accent
        } else {
            p.border_subtle
        })
        .hover(|s| s.bg(p.card_hover))
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap_2()
        .on_click(
            cx.listener(move |this, _, window, cx| match card_clone.kind {
                HomeCardKind::Playlist | HomeCardKind::Album => {
                    this.center_mode = super::CenterMode::Page;
                    this.backend.send(BackendCmd::OpenCollection {
                        id: card_clone.id.clone(),
                        title: card_clone.title.clone(),
                        subtitle: card_clone.subtitle.clone(),
                        thumbnail_url: card_clone.thumbnail_url.clone(),
                        editable: false,
                    });
                    cx.notify();
                }
                HomeCardKind::Artist => {
                    this.center_mode = super::CenterMode::Page;
                    this.backend
                        .send(BackendCmd::OpenArtist(card_clone.id.clone()));
                    cx.notify();
                }
                HomeCardKind::Mood => {
                    let query = format!("{} music", card_clone.title);
                    this.center_mode = super::CenterMode::Page;
                    this.search_input.update(cx, |input, cx| {
                        input.set_value(&query, window, cx);
                    });
                    this.backend.send(BackendCmd::Search(query));
                    cx.notify();
                }
                HomeCardKind::Song | HomeCardKind::Video => {
                    if let Some(track) = card_clone.track.clone() {
                        this.backend.send(BackendCmd::PlayTracks {
                            tracks: vec![track],
                            start_index: 0,
                            shuffle: false,
                        });
                    }
                }
            }),
        )
        .child(
            div()
                .relative()
                .child(render_cover_art(art.as_ref(), 142.0, 11.0, p))
                .child(
                    div()
                        .absolute()
                        .bottom(px(8.0))
                        .left(px(8.0))
                        .size(px(28.0))
                        .rounded_full()
                        .bg(p.player_bar_bg)
                        .border_1()
                        .border_color(p.border_subtle)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(ui_icon(IconName::Play, 12.0, p.highlight_fg)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_playing_card {
                            p.accent
                        } else {
                            p.highlight_fg
                        })
                        .truncate()
                        .child(card.title.clone()),
                )
                .child(div().text_xs().text_color(p.muted).truncate().child(
                    if card.subtitle.is_empty() {
                        kind_badge.to_string()
                    } else {
                        card.subtitle.clone()
                    },
                )),
        )
        .into_any_element()
}
