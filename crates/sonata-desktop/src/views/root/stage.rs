//! Center content stage (pages, collections, lyrics) and right-hand queue drawer.
//!
//! 1. **Stage Router**: `render_center_stage` dispatches to Home, Search, Artist, Settings, Collection, Lyrics, or Fullscreen views.
//! 2. **Collection & Lyrics**: `render_collection_stage` renders queue/playlist headers and track lists; `render_lyrics_stage` renders split-pane synced/plain lyrics.
//! 3. **Queue Drawer**: `render_queue_drawer` shows the scrollable "Up Next" list on the right edge.

use super::super::{
    artist, cached_thumb_path, fullscreen, home, render_cover_art, render_icon_button,
    render_track_row, search, settings, ui_icon, CenterMode, FluentDivExt, SonataDesktopView,
};
use crate::backend::{BackendCmd, NavTarget};
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, uniform_list, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use sonata_core::lyrics::LyricsState;
use std::ops::Range;
use std::sync::Arc;

pub(super) fn render_center_stage(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let inner = match view.center_mode {
        CenterMode::Lyrics => render_lyrics_stage(view, p, cx),
        CenterMode::Fullscreen => fullscreen::render_fullscreen(view, p, window, cx),
        CenterMode::Page => match &view.snapshot.active_nav {
            NavTarget::Home => home::render_home_page(view, p, cx),
            NavTarget::Search => search::render_search_page(view, p, cx),
            NavTarget::Queue => render_collection_stage(view, p, true, cx),
            NavTarget::Playlist(_) => render_collection_stage(view, p, false, cx),
            NavTarget::Artist(_) => artist::render_artist_page(view, p, cx),
            NavTarget::Settings => settings::render_settings_page(view, p, cx),
        },
    };

    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .rounded_2xl()
        .bg(p.panel_bg)
        .border_1()
        .border_color(p.border_subtle)
        .overflow_hidden()
        .child(inner)
        .into_any_element()
}

fn render_collection_stage(
    view: &SonataDesktopView,
    p: &GlassPalette,
    is_queue: bool,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let tracks_arc = if is_queue {
        Arc::new(snap.queue.clone())
    } else {
        Arc::new(snap.playlist_tracks.clone())
    };
    let count = tracks_arc.len();

    let heading = if is_queue {
        "Playback Queue".to_string()
    } else {
        snap.playlist_heading.clone()
    };
    let eyebrow = if is_queue {
        "ACTIVE QUEUE"
    } else if snap
        .playlist_id
        .as_deref()
        .is_some_and(|id| id.starts_with("MPRE"))
    {
        "YOUTUBE MUSIC ALBUM"
    } else {
        "YOUTUBE MUSIC PLAYLIST"
    };
    let subtitle = if is_queue {
        format!("{count} tracks queued")
    } else if !snap.playlist_subtitle.is_empty() {
        format!("{} · {count} tracks", snap.playlist_subtitle)
    } else {
        format!("{count} tracks")
    };

    let cover_art = if is_queue {
        snap.art_path.clone()
    } else {
        cached_thumb_path(snap.playlist_cover_url.as_deref())
            .or_else(|| {
                tracks_arc
                    .first()
                    .and_then(|t| cached_thumb_path(t.thumbnail_url.as_deref()))
            })
            .or_else(|| snap.art_path.clone())
    };

    let editable = !is_queue && snap.playlist_editable;
    let loading = !is_queue && snap.playlist_loading;
    let tracks_for_play = tracks_arc.clone();
    let tracks_for_shuffle = tracks_arc.clone();

    let body = if loading && count == 0 {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(p.muted)
            .child("Loading tracks from YouTube Music...")
            .into_any_element()
    } else if count == 0 {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(ui_icon(IconName::Music2, 34.0, p.muted))
            .child(div().text_sm().text_color(p.muted).child(if is_queue {
                "Your queue is empty. Play something from Home or Search!"
            } else {
                "No tracks found in this collection."
            }))
            .into_any_element()
    } else {
        uniform_list(
            if is_queue {
                "queue-stage-list"
            } else {
                "playlist-stage-list"
            },
            count,
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                let p = this.palette();
                let snap = this.snapshot.clone();
                let cur_id = snap.current_track().map(|t| t.id.as_str());
                let tracks = if is_queue {
                    Arc::new(snap.queue.clone())
                } else {
                    Arc::new(snap.playlist_tracks.clone())
                };

                range
                    .filter_map(|idx| {
                        let track = tracks.get(idx)?.clone();
                        let is_playing = if is_queue {
                            snap.current_index == Some(idx)
                        } else {
                            cur_id == Some(track.id.as_str())
                        };
                        let is_hovered = this.hovered_track == Some(idx);
                        Some(render_track_row(
                            format!("col-row-{idx}"),
                            idx,
                            track,
                            tracks.clone(),
                            is_playing,
                            is_hovered,
                            editable,
                            &p,
                            cx,
                        ))
                    })
                    .collect()
            }),
        )
        .track_scroll(&view.track_scroll)
        .flex_1()
        .w_full()
        .into_any_element()
    };

    div()
        .size_full()
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            // Highlight the selected collection with its artwork and title.
            div()
                .w_full()
                .p_4()
                .rounded_2xl()
                .bg(linear_gradient(
                    130.0,
                    linear_color_stop(p.accent_soft, 0.0),
                    linear_color_stop(p.card_bg, 0.88),
                ))
                .border_1()
                .border_color(p.border_subtle)
                .flex()
                .items_center()
                .gap_5()
                .child(render_cover_art(cover_art.as_ref(), 132.0, 14.0, p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.accent)
                                .child(eyebrow),
                        )
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(heading),
                        )
                        .child(div().text_xs().text_color(p.muted).child(subtitle))
                        .child(
                            div()
                                .pt_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(count > 0, |el| {
                                    el.child(
                                        div()
                                            .id("hero-play-btn")
                                            .px_4()
                                            .py(px(7.0))
                                            .rounded_full()
                                            .bg(p.accent)
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.backend.send(BackendCmd::PlayTracks {
                                                    tracks: (*tracks_for_play).clone(),
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
                                                    .child("Play"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("hero-shuffle-btn")
                                            .px_3()
                                            .py(px(7.0))
                                            .rounded_full()
                                            .bg(p.pill_bg)
                                            .border_1()
                                            .border_color(p.border_subtle)
                                            .hover(|s| s.bg(p.card_hover))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.backend.send(BackendCmd::PlayTracks {
                                                    tracks: (*tracks_for_shuffle).clone(),
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
                                    )
                                })
                                .when(!is_queue, |el| {
                                    el.child(render_icon_button(
                                        "hero-refresh-btn",
                                        IconName::RefreshCw,
                                        15.0,
                                        32.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| {
                                            this.backend.send(BackendCmd::RefreshActivePlaylist);
                                        },
                                        cx,
                                    ))
                                }),
                        ),
                ),
        )
        .child(body)
        .into_any_element()
}

fn render_lyrics_stage(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let track = snap.current_track().cloned();
    let title = track
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "No Active Track".to_string());
    let artist = track
        .as_ref()
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| "Pick a song from Home or Search".to_string());
    let album = track.as_ref().map(|t| t.album.clone()).unwrap_or_default();

    let right_lyrics = match &snap.lyrics {
        LyricsState::Idle => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("Start playback to view synced lyrics.")
            .into_any_element(),
        LyricsState::Loading => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("Fetching synced lyrics...")
            .into_any_element(),
        LyricsState::NotFound => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_base()
            .text_color(p.muted)
            .child("No lyrics found for this track.")
            .into_any_element(),
        LyricsState::Plain(lines) => {
            let count = lines.len();
            uniform_list(
                "lyrics-plain-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, _cx| {
                    let p = this.palette();
                    let LyricsState::Plain(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|idx| {
                            let line = lines.get(idx)?;
                            Some(
                                div()
                                    .w_full()
                                    .py(px(6.0))
                                    .px_4()
                                    .text_lg()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.lyrics_mid)
                                    .child(line.clone()),
                            )
                        })
                        .collect()
                }),
            )
            .track_scroll(&view.lyrics_scroll)
            .flex_1()
            .w_full()
            .into_any_element()
        }
        LyricsState::Synced(lines) => {
            let count = lines.len();
            uniform_list(
                "lyrics-synced-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, cx| {
                    let p = this.palette();
                    let active = this.snapshot.active_lyric_index();
                    let LyricsState::Synced(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|idx| {
                            let line = lines.get(idx)?;
                            let dist = active.map(|a| a.abs_diff(idx)).unwrap_or(10);
                            let (color, size_px, weight, row_bg) = match dist {
                                0 => (p.lyrics_active, 24.0, FontWeight::BOLD, p.ambient_glow),
                                1 => (
                                    p.lyrics_mid,
                                    19.0,
                                    FontWeight::SEMIBOLD,
                                    gpui_kit::transparent_black(),
                                ),
                                _ => (
                                    p.lyrics_inactive,
                                    17.0,
                                    FontWeight::MEDIUM,
                                    gpui_kit::transparent_black(),
                                ),
                            };
                            let text = if line.text.trim().is_empty() {
                                "♪".to_string()
                            } else {
                                line.text.clone()
                            };
                            let target_secs = line.time_secs;

                            Some(
                                div()
                                    .id(SharedString::from(format!("lyr-row-{idx}")))
                                    .w_full()
                                    .min_h(px(50.0))
                                    .px_4()
                                    .py_2()
                                    .rounded_xl()
                                    .bg(row_bg)
                                    .hover(|s| s.bg(p.card_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(target_secs));
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(size_px))
                                            .font_weight(weight)
                                            .text_color(color)
                                            .child(text),
                                    ),
                            )
                        })
                        .collect()
                }),
            )
            .track_scroll(&view.lyrics_scroll)
            .flex_1()
            .w_full()
            .into_any_element()
        }
    };

    div()
        .size_full()
        .bg(linear_gradient(
            140.0,
            linear_color_stop(p.ambient_glow, 0.0),
            linear_color_stop(p.panel_bg, 0.78),
        ))
        .p_6()
        .flex()
        .gap_8()
        .child(
            div()
                .w(px(330.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(render_cover_art(snap.art_path.as_ref(), 280.0, 20.0, p))
                .child(
                    div()
                        .w_full()
                        .px_2()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(title),
                        )
                        .child(div().text_sm().text_color(p.muted).truncate().child(artist))
                        .when(!album.is_empty(), |el| {
                            el.child(div().text_xs().text_color(p.muted).truncate().child(album))
                        }),
                )
                .child(
                    div().flex().flex_wrap().justify_center().gap_2().child(
                        div()
                            .px_3()
                            .py(px(4.0))
                            .rounded_full()
                            .bg(p.pill_bg)
                            .border_1()
                            .border_color(p.border_subtle)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.success)
                            .child(snap.stream_badge.clone()),
                    ),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .px_4()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.muted)
                        .child("SYNCED LYRICS (CLICK ANY LINE TO SEEK)"),
                )
                .child(right_lyrics),
        )
        .into_any_element()
}

pub(super) fn render_queue_drawer(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let count = view.snapshot.queue.len();
    let list_el = if count == 0 {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(p.muted)
            .child("Queue is empty")
            .into_any_element()
    } else {
        uniform_list(
            "queue-drawer-list",
            count,
            cx.processor(|this, range: Range<usize>, _window, cx| {
                let p = this.palette();
                let snap = this.snapshot.clone();
                range
                    .filter_map(|idx| {
                        let track = snap.queue.get(idx)?;
                        let active = snap.current_index == Some(idx);
                        Some(
                            div()
                                .id(SharedString::from(format!("q-item-{idx}")))
                                .w_full()
                                .h(px(48.0))
                                .px_2()
                                .rounded_lg()
                                .bg(if active {
                                    p.accent_soft
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend.send(BackendCmd::PlayQueueIndex(idx));
                                }))
                                .child(
                                    div()
                                        .w(px(20.0))
                                        .text_xs()
                                        .text_color(if active { p.accent } else { p.muted })
                                        .child(if active {
                                            "▶".to_string()
                                        } else {
                                            format!("{}", idx + 1)
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(if active {
                                                    FontWeight::BOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(if active { p.accent } else { p.text })
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
                                ),
                        )
                    })
                    .collect()
            }),
        )
        .track_scroll(&view.queue_scroll)
        .flex_1()
        .w_full()
        .into_any_element()
    };

    div()
        .w(px(274.0))
        .flex_shrink_0()
        .h_full()
        .rounded_2xl()
        .bg(p.sidebar_bg)
        .border_1()
        .border_color(p.border_subtle)
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child(format!("Up Next ({count})")),
                )
                .child(render_icon_button(
                    "q-close",
                    IconName::X,
                    14.0,
                    24.0,
                    p.muted,
                    p.pill_bg,
                    p.card_hover,
                    |this, _, cx| {
                        this.show_queue = false;
                        cx.notify();
                    },
                    cx,
                )),
        )
        .child(list_el)
        .into_any_element()
}
