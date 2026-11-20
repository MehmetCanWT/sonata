use super::{
    cached_thumb_path, render_cover_art, render_track_row, FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use std::sync::Arc;

pub fn render_artist_page(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    if snap.artist_loading {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(p.muted)
            .child("Loading artist...")
            .into_any_element();
    }
    if let Some(error) = &snap.artist_error {
        return div()
            .size_full()
            .p_6()
            .text_sm()
            .text_color(p.error)
            .child(format!("Could not load artist: {error}"))
            .into_any_element();
    }
    let Some(artist) = &snap.artist_page else {
        return div().size_full().into_any_element();
    };

    let art = cached_thumb_path(artist.thumbnail_url.as_deref());
    let tracks = Arc::new(artist.popular_tracks.clone());
    let current_id = snap.current_track().map(|track| track.id.as_str());
    let mut page = div()
        .id("artist-page")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .w_full()
                .p_6()
                .bg(p.ambient_glow)
                .flex()
                .items_end()
                .gap_5()
                .child(render_cover_art(art.as_ref(), 152.0, 76.0, p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_xs().text_color(p.muted).child("ARTIST"))
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(artist.name.clone()),
                        )
                        .when(!artist.subtitle.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_sm()
                                    .text_color(p.muted)
                                    .child(artist.subtitle.clone()),
                            )
                        }),
                ),
        );

    if !artist.popular_tracks.is_empty() {
        page = page.child(
            div()
                .px_6()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child("Popular tracks"),
                )
                .children(artist.popular_tracks.iter().cloned().enumerate().map(
                    |(index, track)| {
                        render_track_row(
                            format!("artist-track-{index}"),
                            index,
                            track.clone(),
                            Arc::clone(&tracks),
                            current_id == Some(track.id.as_str()),
                            view.hovered_track == Some(index),
                            false,
                            p,
                            cx,
                        )
                    },
                )),
        );
    }

    if !artist.releases.is_empty() {
        page = page.child(
            div()
                .px_6()
                .pb_6()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child("Releases"),
                )
                .child(div().flex().flex_wrap().gap_3().children(
                    artist.releases.iter().enumerate().map(|(index, release)| {
                        let release = release.clone();
                        let release_for_click = release.clone();
                        let art = cached_thumb_path(release.thumbnail_url.as_deref());
                        div()
                            .id(SharedString::from(format!("artist-release-{index}")))
                            .w(px(150.0))
                            .p_2()
                            .rounded_xl()
                            .bg(p.card_bg)
                            .hover(|style| style.bg(p.card_hover))
                            .cursor_pointer()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.backend.send(BackendCmd::OpenCollection {
                                    id: release_for_click.id.clone(),
                                    title: release_for_click.title.clone(),
                                    subtitle: release_for_click.subtitle.clone(),
                                    thumbnail_url: release_for_click.thumbnail_url.clone(),
                                    editable: false,
                                });
                            }))
                            .child(render_cover_art(art.as_ref(), 132.0, 12.0, p))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(p.text)
                                    .truncate()
                                    .child(release.title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(release.subtitle),
                            )
                    }),
                )),
        );
    }
    page.into_any_element()
}
