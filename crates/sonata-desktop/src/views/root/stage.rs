//! Center content stage (pages, collections, lyrics) and right-hand queue drawer.
//!
//! 1. **Stage Router**: `render_center_stage` dispatches to Home, Search, Artist, Settings, Collection, Lyrics, or Fullscreen views.
//! 2. **Collection & Lyrics**: `render_collection_stage` renders queue/playlist headers and track lists; `render_lyrics_stage` renders split-pane synced/plain lyrics.
//! 3. **Queue Drawer**: `render_queue_drawer` shows the scrollable "Up Next" list on the right edge.

use super::super::{
    artist, cached_thumb_path, fullscreen, home, render_cover_art, render_cover_surface,
    render_icon_button, render_quality_tag, render_track_row, search, settings, ui_icon,
    CenterMode, FluentDivExt, SonataDesktopView,
};
use crate::backend::{BackendCmd, NavTarget};
use crate::theme_bridge::{DesktopSkin, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, uniform_list, AnyElement, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use sonata_core::lyrics::{LyricVoice, LyricsState};
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

    match p.skin {
        DesktopSkin::Spotify => div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .rounded_xl()
            .bg(p.panel_bg)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .size_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(p.ambient_top, 0.0),
                        linear_color_stop(p.panel_bg, 0.32),
                    ))
                    .child(inner),
            )
            .into_any_element(),
        DesktopSkin::YouTubeMusic => div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .bg(p.panel_bg)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .size_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(p.ambient_top, 0.0),
                        linear_color_stop(p.panel_bg, 0.28),
                    ))
                    .child(inner),
            )
            .into_any_element(),
        DesktopSkin::Sonata => div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(inner)
            .into_any_element(),
    }
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
        .min_h_0()
        .w_full()
        .into_any_element()
    };

    let play_cta = if p.skin == DesktopSkin::Spotify {
        // Spotify's signature 48px circular green Play button.
        div()
            .id("hero-play-btn")
            .size(px(48.0))
            .rounded_full()
            .bg(p.accent)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, _| {
                this.backend.send(BackendCmd::PlayTracks {
                    tracks: (*tracks_for_play).clone(),
                    start_index: 0,
                    shuffle: false,
                });
            }))
            .child(ui_icon(IconName::Play, 20.0, p.accent_fg))
            .into_any_element()
    } else {
        div()
            .id("hero-play-btn")
            .px_5()
            .py(px(9.0))
            .rounded_full()
            .bg(p.cta_bg)
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
            .child(ui_icon(IconName::Play, 14.0, p.cta_fg))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.cta_fg)
                    .child(if p.skin == DesktopSkin::YouTubeMusic {
                        "Play all"
                    } else {
                        "Play"
                    }),
            )
            .into_any_element()
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(
            // Borderless full-bleed collection hero header.
            div()
                .w_full()
                .px_6()
                .pt_6()
                .pb_5()
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(p.ambient_top, 0.0),
                    linear_color_stop(gpui_kit::transparent_black(), 1.0),
                ))
                .flex()
                .items_end()
                .gap_6()
                .child(render_cover_art(
                    cover_art.as_ref(),
                    156.0,
                    if p.skin == DesktopSkin::Sonata {
                        14.0
                    } else {
                        8.0
                    },
                    p,
                ))
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
                                .text_color(if p.skin == DesktopSkin::Sonata {
                                    p.accent
                                } else {
                                    p.muted
                                })
                                .child(eyebrow),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.highlight_fg)
                                .truncate()
                                .child(heading),
                        )
                        .child(div().text_xs().text_color(p.muted).child(subtitle))
                        .child(
                            div()
                                .pt_2()
                                .flex()
                                .items_center()
                                .gap_3()
                                .when(count > 0, |el| {
                                    el.child(play_cta).child(
                                        div()
                                            .id("hero-shuffle-btn")
                                            .px_4()
                                            .py(px(8.0))
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
                                        34.0,
                                        p.muted,
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
        .child(
            // Subtle table column header
            div()
                .w_full()
                .px_6()
                .pb_2()
                .border_b_1()
                .border_color(p.divider)
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(24.0))
                        .flex()
                        .justify_center()
                        .text_xs()
                        .text_color(p.muted)
                        .child("#"),
                )
                .child(div().w(px(40.0)))
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.muted)
                        .child("Title"),
                )
                .child(
                    div()
                        .w(px(190.0))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.muted)
                        .child("Album"),
                )
                .child(div().w(px(64.0)))
                .child(
                    div()
                        .w(px(46.0))
                        .flex()
                        .justify_end()
                        .text_xs()
                        .text_color(p.muted)
                        .child("Time"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .px_3()
                .pt_2()
                .flex()
                .flex_col()
                .child(body),
        )
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
                            let target_secs = line.time_secs;
                            let next_start = lines.get(idx + 1).map(|next| next.time_secs);
                            let align_right = line.voice == LyricVoice::Counter;
                            let trimmed = line.text.trim();
                            let is_bg = trimmed.starts_with('(')
                                && trimmed.ends_with(')')
                                && trimmed.len() > 2;

                            let (row_bg, row_opacity, row_content) = if dist == 0 {
                                let end_secs = super::fullscreen::sheet::estimate_verse_end_secs(
                                    line,
                                    next_start,
                                    this.snapshot.duration_secs,
                                );
                                let body = if trimmed.is_empty() {
                                    div()
                                        .w_full()
                                        .flex()
                                        .when(align_right, |el| el.justify_end())
                                        .text_size(px(22.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.lyrics_active)
                                        .child("♪")
                                        .into_any_element()
                                } else {
                                    super::fullscreen::sheet::render_active_karaoke_verse_styled(
                                        line,
                                        this.snapshot.position_secs,
                                        end_secs,
                                        super::fullscreen::sheet::KaraokePalette {
                                            font_size: if is_bg { 19.0 } else { 22.0 },
                                            line_height: if is_bg { 24.0 } else { 28.0 },
                                            align_right,
                                            is_background: is_bg,
                                            dim_color: p.lyrics_inactive,
                                            lit_color: p.lyrics_active,
                                            active_glow_color: p.accent,
                                            sung_glow_color: p.lyrics_mid,
                                        },
                                    )
                                };
                                (p.ambient_glow, 1.0f32, body)
                            } else {
                                let (color, size_px, weight, opacity) = match dist {
                                    1 => (
                                        p.lyrics_mid,
                                        if is_bg { 15.0 } else { 17.5 },
                                        FontWeight::MEDIUM,
                                        if is_bg { 0.58 } else { 0.78 },
                                    ),
                                    _ => (
                                        p.lyrics_inactive,
                                        if is_bg { 14.0 } else { 16.0 },
                                        FontWeight::NORMAL,
                                        if is_bg { 0.42 } else { 0.56 },
                                    ),
                                };
                                let text = if trimmed.is_empty() {
                                    "♪".to_string()
                                } else {
                                    line.text.clone()
                                };
                                (
                                    gpui_kit::transparent_black(),
                                    opacity,
                                    div()
                                        .w_full()
                                        .whitespace_normal()
                                        .when(align_right, |el| el.text_right())
                                        .text_size(px(size_px))
                                        .line_height(px(size_px * 1.25))
                                        .font_weight(weight)
                                        .text_color(color)
                                        .child(text)
                                        .into_any_element(),
                                )
                            };

                            Some(
                                div()
                                    .id(SharedString::from(format!("lyr-row-{idx}")))
                                    .w_full()
                                    .min_h(px(46.0))
                                    .px_4()
                                    .py(px(6.0))
                                    .rounded_xl()
                                    .bg(row_bg)
                                    .opacity(row_opacity)
                                    .hover(|s| s.bg(p.card_hover).opacity(1.0))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(target_secs));
                                    }))
                                    .child(row_content),
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
                .child(render_cover_surface(
                    snap.art_path.as_ref(),
                    view.active_motion_texture(),
                    280.0,
                    20.0,
                    p,
                ))
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
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap_2()
                        .child(render_quality_tag(view, p, false)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
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

    let drawer = match p.skin {
        DesktopSkin::Spotify => div()
            .w(px(284.0))
            .flex_shrink_0()
            .h_full()
            .rounded_xl()
            .bg(p.sidebar_bg)
            .p_3(),
        DesktopSkin::Sonata | DesktopSkin::YouTubeMusic => div()
            .w(px(284.0))
            .flex_shrink_0()
            .h_full()
            .bg(p.sidebar_bg)
            .border_l_1()
            .border_color(p.divider)
            .p_4(),
    };

    drawer
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
