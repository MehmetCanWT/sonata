use super::{render_track_row, ui_icon, FluentDivExt, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, uniform_list, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};
use std::ops::Range;
use std::sync::Arc;

const QUICK_SEARCH_MOODS: &[&str] = &[
    "Phonk",
    "Lo-Fi Beats",
    "Synthwave",
    "Indie Rock",
    "Electronic",
    "Hip-Hop",
    "Jazz Focus",
    "Ambient",
];

pub fn render_search_page(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let count = snap.search_results.len();
    let heading = if snap.search_query.is_empty() {
        "Search YouTube Music".to_string()
    } else {
        format!("Results for \"{}\"", snap.search_query)
    };
    let subtitle = if snap.search_loading {
        "Searching catalog...".to_string()
    } else if snap.search_query.is_empty() {
        "Find any song, artist, remix, or soundtrack directly from YouTube Music.".to_string()
    } else {
        format!("{count} songs found")
    };

    let body = if snap.search_loading {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(ui_icon(IconName::Search, 30.0, p.accent))
            .child(
                div()
                    .text_sm()
                    .text_color(p.muted)
                    .child(format!("Searching \"{}\"...", snap.search_query)),
            )
            .into_any_element()
    } else if count == 0 {
        let chips = div().flex().flex_wrap().justify_center().gap_2().children(
            QUICK_SEARCH_MOODS.iter().enumerate().map(|(i, term)| {
                let q = (*term).to_string();
                div()
                    .id(SharedString::from(format!("qmood-{i}")))
                    .px_4()
                    .py_2()
                    .rounded_full()
                    .bg(p.card_bg)
                    .border_1()
                    .border_color(p.border_subtle)
                    .hover(|s| s.bg(p.card_hover))
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let q_val = q.clone();
                        this.search_input.update(cx, |input, cx| {
                            input.set_value(&q_val, window, cx);
                        });
                        this.backend.send(BackendCmd::Search(q.clone()));
                    }))
                    .child(*term)
            }),
        );

        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(ui_icon(IconName::Compass, 36.0, p.accent))
            .child(
                div()
                    .text_sm()
                    .text_color(p.muted)
                    .child(if snap.search_query.is_empty() {
                        "Type a query above or jump right into a vibe:"
                    } else {
                        "No songs matched that query. Try another search:"
                    }),
            )
            .child(chips)
            .into_any_element()
    } else {
        uniform_list(
            "search-results-list",
            count,
            cx.processor(|this, range: Range<usize>, _window, cx| {
                let p = this.palette();
                let snap = this.snapshot.clone();
                let cur_id = snap.current_track().map(|t| t.id.as_str());
                let tracks_arc = Arc::new(snap.search_results.clone());

                range
                    .filter_map(|idx| {
                        let track = tracks_arc.get(idx)?.clone();
                        let is_playing = cur_id == Some(track.id.as_str());
                        let is_hovered = this.hovered_track == Some(idx);
                        Some(render_track_row(
                            format!("sr-row-{idx}"),
                            idx,
                            track,
                            tracks_arc.clone(),
                            is_playing,
                            is_hovered,
                            false,
                            &p,
                            cx,
                        ))
                    })
                    .collect()
            }),
        )
        .track_scroll(&view.search_scroll)
        .flex_1()
        .w_full()
        .into_any_element()
    };

    div()
        .size_full()
        .p_6()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(heading),
                        )
                        .child(div().text_xs().text_color(p.muted).child(subtitle)),
                )
                .when(count > 0, |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("search-play-all")
                                    .px_4()
                                    .py_2()
                                    .rounded_full()
                                    .bg(p.accent)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: this.snapshot.search_results.clone(),
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
                                            .child("Play All"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("search-shuffle")
                                    .px_3()
                                    .py_2()
                                    .rounded_full()
                                    .bg(p.pill_bg)
                                    .border_1()
                                    .border_color(p.border_subtle)
                                    .hover(|s| s.bg(p.card_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: this.snapshot.search_results.clone(),
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
                            ),
                    )
                }),
        )
        .child(body)
        .into_any_element()
}
