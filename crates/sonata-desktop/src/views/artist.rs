//! Dedicated Artist page featuring a 0.36-aspect wide channel hero, Top Songs, You Liked, and shelves.
//!
//! 1. **Wide Hero (`0.36` aspect, `220..=440 px`)**: renders YouTube's wide channel banner with a dark bottom readability gradient scrim and overlaid title/subscribers/actions, falling back cleanly to a circular portrait header.
//! 2. **Top Songs & You Liked**: splits artist top songs cleanly alongside liked/played tracks matching the artist.
//! 3. **Releases & Related Artists**: renders Albums/Singles/Videos shelves and circular `Fans might also like` portraits.

use super::home::TrackRowOpts;
use super::SonataDesktopView;
use crate::backend::BackendCmd;
use crate::settings::{CARD_W, HERO_ASPECT, PAGE_INSET_X};
use crate::ui::components::{
    cover_art_tile, delayed_skeleton_block, fit_title_size, header_wash, scroll_edge_fades,
    section_header,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, AnyElement,
    Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};

impl SonataDesktopView {
    pub fn render_artist_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let page_opt = self.snapshot.artist_page.clone();
        let banner_url = self.snapshot.artist_banner_url.clone();
        let loading = self.snapshot.artist_loading;
        let error = self.snapshot.artist_error.clone();
        let started = self.loading_started.get("artist").copied();
        let scroll_fades = self.settings.scroll_fades;
        let adaptive = self.settings.adaptive_art_color;

        let Some(artist) = page_opt else {
            let body = if loading {
                delayed_skeleton_block(
                    "art-sk-hero",
                    started,
                    None,
                    260.0,
                    self.palette.radii.control,
                    &self.palette,
                )
            } else {
                div()
                    .p(px(24.0))
                    .text_size(px(14.0))
                    .text_color(self.palette.secondary)
                    .child(error.unwrap_or_else(|| "Artist not available".to_string()))
                    .into_any_element()
            };
            return div().p(px(PAGE_INSET_X)).child(body).into_any_element();
        };

        let banner_media = banner_url
            .as_deref()
            .and_then(|u| self.media_for_url(Some(u), cx));
        let thumb_media = self.media_for_url(artist.thumbnail_url.as_deref(), cx);
        let title_pt = fit_title_size(&artist.name);
        let meta = artist
            .monthly_listeners
            .clone()
            .or_else(|| artist.subscribers.clone())
            .unwrap_or_else(|| artist.subtitle.clone());

        let shuffle_id = artist.shuffle_playlist_id.clone();
        let radio_id = artist.radio_playlist_id.clone();
        let top_tracks = artist.popular_tracks.clone();
        let artist_name_header = artist.name.clone();

        let action_buttons = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                Button::new("art-play")
                    .primary()
                    .icon(IconName::Play)
                    .label("Play")
                    .when(top_tracks.is_empty() && shuffle_id.is_none(), |b| {
                        b.opacity(0.45)
                    })
                    .on_click(cx.listener({
                        let tracks = top_tracks.clone();
                        let shuf = shuffle_id.clone();
                        let aname = artist_name_header.clone();
                        let thumb = artist.thumbnail_url.clone();
                        move |this, _, _, _| {
                            if !tracks.is_empty() {
                                this.backend.send(BackendCmd::PlayTracks {
                                    tracks: tracks.clone(),
                                    start_index: 0,
                                    shuffle: false,
                                });
                            } else if let Some(ref pid) = shuf {
                                this.backend.send(BackendCmd::OpenCollection {
                                    id: pid.clone(),
                                    title: format!("{aname} Mix"),
                                    subtitle: "Artist Radio".to_string(),
                                    thumbnail_url: thumb.clone(),
                                    editable: false,
                                });
                            }
                        }
                    })),
            )
            .when(radio_id.is_some() || !top_tracks.is_empty(), |r| {
                let tracks_rad = top_tracks.clone();
                let rad_id = radio_id.clone();
                let aname = artist_name_header.clone();
                let thumb = artist.thumbnail_url.clone();
                r.child(
                    Button::new("art-radio")
                        .secondary()
                        .icon(IconName::Radio)
                        .label("Radio")
                        .on_click(cx.listener(move |this, _, _, _| {
                            if let Some(first) = tracks_rad.first().cloned() {
                                this.backend.send(BackendCmd::StartTrackRadio(first));
                            } else if let Some(ref pid) = rad_id {
                                this.backend.send(BackendCmd::OpenCollection {
                                    id: pid.clone(),
                                    title: format!("{aname} Radio"),
                                    subtitle: "Artist Radio".to_string(),
                                    thumbnail_url: thumb.clone(),
                                    editable: false,
                                });
                            }
                        })),
                )
            });

        let hero_banner: AnyElement = if let Some(banner) = banner_media {
            let banner_h = (content_w * HERO_ASPECT).clamp(240.0, 440.0);
            div()
                .relative()
                .w_full()
                .h(px(banner_h))
                .overflow_hidden()
                .child(img(banner.image).size_full().object_fit(ObjectFit::Cover))
                .child(div().absolute().inset_0().bg(linear_gradient(
                    180.,
                    linear_color_stop(gpui_kit::black().opacity(0.0), 0.0),
                    linear_color_stop(gpui_kit::black().opacity(0.42), 1.0),
                )))
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .px(px(PAGE_INSET_X))
                        .pb(px(22.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(title_pt))
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui_kit::white())
                                .child(artist.name.clone()),
                        )
                        .when(!meta.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(gpui_kit::white().opacity(0.82))
                                    .child(meta.clone()),
                            )
                        })
                        .child(action_buttons),
                )
                .into_any_element()
        } else {
            div()
                .relative()
                .child(header_wash(
                    thumb_media.as_ref(),
                    &self.palette,
                    adaptive,
                    250.0,
                ))
                .child(
                    div()
                        .relative()
                        .px(px(PAGE_INSET_X))
                        .pt(px(24.0))
                        .pb(px(14.0))
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(px(22.0))
                        .child(cover_art_tile(
                            "art-fallback-avatar",
                            thumb_media.as_ref(),
                            156.0,
                            true,
                            &self.palette,
                            false,
                            None,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(self.palette.secondary)
                                        .child("ARTIST"),
                                )
                                .child(
                                    div()
                                        .text_size(px(title_pt))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(self.palette.text)
                                        .child(artist.name.clone()),
                                )
                                .when(!meta.is_empty(), |col| {
                                    col.child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(self.palette.secondary)
                                            .child(meta.clone()),
                                    )
                                })
                                .child(action_buttons),
                        ),
                )
                .into_any_element()
        };

        // Match liked / recently played tracks by this artist for the `You Liked` split section.
        let lower_name = artist.name.to_ascii_lowercase();
        let you_liked: Vec<_> = self
            .snapshot
            .last_played_tracks
            .iter()
            .filter(|t| t.liked && t.artist.to_ascii_lowercase().contains(&lower_name))
            .take(5)
            .cloned()
            .collect();

        let mut sections = div()
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .flex()
            .flex_col()
            .gap(px(26.0));

        if !artist.popular_tracks.is_empty() {
            let pop_tracks: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(artist.popular_tracks.as_slice());
            let show_split = !you_liked.is_empty() && content_w >= 760.0;
            let mut top_rows = div().flex().flex_col().gap(px(2.0));
            for i in 0..pop_tracks.len().min(8) {
                top_rows = top_rows.child(self.render_track_row(
                    "art-pop",
                    i,
                    &pop_tracks,
                    TrackRowOpts {
                        show_index: true,
                        show_thumb: true,
                        show_album: true,
                        ..Default::default()
                    },
                    cx,
                ));
            }

            if show_split {
                let liked_arc: std::sync::Arc<[sonata_core::providers::Track]> =
                    std::sync::Arc::from(you_liked.as_slice());
                let mut liked_rows = div().flex().flex_col().gap(px(2.0));
                for i in 0..liked_arc.len() {
                    liked_rows = liked_rows.child(self.render_track_row(
                        "art-lik",
                        i,
                        &liked_arc,
                        TrackRowOpts::default(),
                        cx,
                    ));
                }
                sections = sections.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(20.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(section_header(
                                    "art-top-h",
                                    "Top songs",
                                    None,
                                    None,
                                    &self.palette,
                                ))
                                .child(top_rows),
                        )
                        .child(
                            div()
                                .w(px(300.0))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .child(section_header(
                                    "art-lik-h",
                                    "You Liked",
                                    None,
                                    None,
                                    &self.palette,
                                ))
                                .child(liked_rows),
                        ),
                );
            } else {
                sections = sections.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(section_header(
                            "art-top-h",
                            "Top songs",
                            None,
                            None,
                            &self.palette,
                        ))
                        .child(top_rows),
                );
            }
        }

        let gap = 14.0_f32;
        let cols = (((content_w + gap) / (CARD_W + gap)).floor() as usize).clamp(2, 8);
        let card_w = ((content_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
            .clamp(CARD_W, 174.0);

        if !artist.releases.is_empty() {
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, r) in artist.releases.iter().enumerate() {
                grid = grid.child(self.render_card_tile(&format!("art-rel-{i}"), r, card_w, cx));
            }
            sections = sections.child(
                div()
                    .flex()
                    .flex_col()
                    .child(section_header(
                        "art-rel-h",
                        "Releases",
                        None,
                        None,
                        &self.palette,
                    ))
                    .child(grid),
            );
        }

        if !artist.related_artists.is_empty() {
            let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
            for (i, a) in artist.related_artists.iter().enumerate() {
                grid = grid.child(self.render_card_tile(&format!("art-sim-{i}"), a, card_w, cx));
            }
            sections = sections.child(
                div()
                    .flex()
                    .flex_col()
                    .child(section_header(
                        "art-sim-h",
                        "Fans might also like",
                        None,
                        None,
                        &self.palette,
                    ))
                    .child(grid),
            );
        }

        let page = div().flex().flex_col().child(hero_banner).child(sections);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("artist-page-scroll")
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
