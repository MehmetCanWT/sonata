//! Fullscreen / Now Playing overlay with washed blurred backdrop, glowing synced lyrics, and queue drawer.
//!
//! 1. **Washed Backdrop**: paints dominant cover pigments + `96x96` multi-pass blurred artwork texture + theme readability veil.
//! 2. **Stage & Chrome**: displays responsive cover/motion art, clickable artists, full transport + seek/volume `Slider`s, and auto-hiding top bar.
//! 3. **Synced Lyrics & Queue**: supports large click-to-seek synced lyrics with `3s` wheel-browse settle pause alongside an interactive `Next up` queue tab.

use super::{FullscreenDrawerTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::ui::components::{
    album_art_theme_colors, artist_links_row, card_panel, cover_art_tile, explicit_badge,
    fit_title_size, glowing_lyric_row, is_parenthetical_bg, loading_ring, quality_badge,
    resolve_active_lyric_pair, LyricRowStyle,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, AnyElement,
    Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};
use sonata_core::lyrics::LyricsState;
use sonata_core::media::format_mm_ss;
use sonata_core::player::RepeatMode;
use std::time::{Duration, Instant};

impl SonataDesktopView {
    pub fn render_fullscreen_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
        let vp_h: f32 = vp.height.into();
        let stacked = vp_w < 860.0;
        let current = self.snapshot.current_track().cloned();
        let motion_tex = self.current_motion_image();
        let art_media = self
            .media_for_path(self.snapshot.art_path.as_deref(), cx)
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|t| self.media_for_url(t.thumbnail_url.as_deref(), cx))
            });

        let dom_rgb = art_media
            .as_ref()
            .map(|m| m.dominant_rgb)
            .unwrap_or(self.snapshot.ambient_rgb);
        let (deep_canvas, top_wash, album_accent) = album_art_theme_colors(dom_rgb);
        let veil_color = deep_canvas.opacity(0.76);
        let idle_chrome = self.fullscreen_last_activity.elapsed() > Duration::from_secs(4)
            && !self.snapshot.paused;

        let art_size =
            ((vp_h - 320.0).min(if stacked { 240.0 } else { 360.0 })).clamp(180.0, 380.0);

        let cover_box = if let Some(tex) = motion_tex {
            div()
                .flex_shrink_0()
                .size(px(art_size))
                .rounded(px(self.palette.radii.art(art_size)))
                .overflow_hidden()
                .shadow_xl()
                .child(img(tex).size_full().object_fit(ObjectFit::Cover))
                .into_any_element()
        } else {
            div()
                .flex_shrink_0()
                .rounded(px(self.palette.radii.art(art_size)))
                .shadow_xl()
                .child(cover_art_tile(
                    "fs-art",
                    art_media.as_ref(),
                    art_size,
                    false,
                    &self.palette,
                    false,
                    None,
                ))
                .into_any_element()
        };

        let top_bar = div()
            .h(px(54.0))
            .px(px(22.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .opacity(if idle_chrome { 0.25 } else { 1.0 })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        Button::new("fs-close")
                            .secondary()
                            .small()
                            .icon(IconName::Minimize2)
                            .label("Minimize")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.fullscreen_open = false;
                                if window.is_fullscreen() {
                                    window.toggle_fullscreen();
                                }
                                cx.notify();
                            })),
                    )
                    .when(
                        !self.snapshot.stream_badge.is_empty()
                            && self.snapshot.stream_badge != "Idle",
                        |r| {
                            r.child(quality_badge(
                                &self.snapshot.stream_badge,
                                self.snapshot.upgrading_quality,
                                &self.palette,
                            ))
                        },
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        TabBar::new("fs-drawer-tabs")
                            .pill()
                            .small()
                            .selected_index(match self.fullscreen_tab {
                                FullscreenDrawerTab::Lyrics => 0,
                                FullscreenDrawerTab::Queue => 1,
                            })
                            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                this.fullscreen_tab = if *idx == 1 {
                                    FullscreenDrawerTab::Queue
                                } else {
                                    FullscreenDrawerTab::Lyrics
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label("Lyrics"))
                            .child(Tab::new().label("Next up")),
                    )
                    .child(
                        Button::new("fs-win-full")
                            .secondary()
                            .small()
                            .icon(IconName::Maximize2)
                            .tooltip("Toggle true window fullscreen (F11)")
                            .on_click(cx.listener(|_, _, window, _| {
                                window.toggle_fullscreen();
                            })),
                    ),
            );

        let track_meta_and_controls = if let Some(ref track) = current {
            let title_pt = fit_title_size(&track.title).min(30.0);
            let pos_secs = if self.progress_dragging {
                match self.progress_slider.read(cx).value() {
                    gpui_kit::component::slider::SliderValue::Single(x) => x as f64,
                    gpui_kit::component::slider::SliderValue::Range(_, e) => e as f64,
                }
            } else {
                self.snapshot.position_secs
            };
            let track_like = track.clone();
            let track_dislike = track.clone();
            let repeat_icon = match self.snapshot.repeat {
                RepeatMode::One => IconName::Repeat1,
                _ => IconName::Repeat,
            };

            div()
                .w(px(art_size.max(320.0)))
                .max_w_full()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .justify_between()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(title_pt))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(self.palette.text)
                                                .truncate()
                                                .child(track.title.clone()),
                                        )
                                        .when(track.is_explicit, |r| {
                                            r.child(explicit_badge(&self.palette))
                                        }),
                                )
                                .child(artist_links_row(
                                    "fs-artist",
                                    &track.artist,
                                    self.palette.secondary,
                                    15.0,
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
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    Button::new("fs-dislike")
                                        .ghost()
                                        .small()
                                        .icon(IconName::ThumbsDown)
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.backend.send(BackendCmd::DislikeTrack(
                                                track_dislike.clone(),
                                            ));
                                        })),
                                )
                                .child(
                                    Button::new("fs-like")
                                        .ghost()
                                        .small()
                                        .icon(IconName::ThumbsUp)
                                        .when(track.liked, |b| b.text_color(self.palette.accent))
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.backend
                                                .send(BackendCmd::ToggleLike(track_like.clone()));
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(Slider::new(&self.progress_slider).horizontal())
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .text_size(px(12.0))
                                .text_color(self.palette.secondary)
                                .child(format_mm_ss(pos_secs.max(0.0).round() as u64))
                                .child(format_mm_ss(
                                    self.snapshot.duration_secs.max(0.0).round() as u64
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap(px(16.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    Button::new("fs-shuf")
                                        .ghost()
                                        .icon(IconName::Shuffle)
                                        .when(self.snapshot.shuffle, |b| b.text_color(album_accent))
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.backend.send(BackendCmd::ToggleShuffle);
                                        })),
                                )
                                .child(div().size(px(4.0)).rounded_full().bg(
                                    if self.snapshot.shuffle {
                                        album_accent
                                    } else {
                                        gpui_kit::transparent_black()
                                    },
                                )),
                        )
                        .child(
                            Button::new("fs-prev")
                                .ghost()
                                .icon(IconName::SkipBack)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::PrevTrack);
                                })),
                        )
                        .child(
                            div()
                                .id("fs-play-btn")
                                .relative()
                                .size(px(52.0))
                                .rounded_full()
                                .bg(album_accent)
                                .text_color(gpui_kit::black())
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.90))
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::TogglePause);
                                }))
                                .when(self.snapshot.loading_track, |b| {
                                    b.child(loading_ring("fs-ring", 52.0, &self.palette))
                                })
                                .child(
                                    Icon::new(if self.snapshot.paused {
                                        IconName::Play
                                    } else {
                                        IconName::Pause
                                    })
                                    .size(px(24.0)),
                                ),
                        )
                        .child(
                            Button::new("fs-next")
                                .ghost()
                                .icon(IconName::SkipForward)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::NextTrack);
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    Button::new("fs-rep")
                                        .ghost()
                                        .icon(repeat_icon)
                                        .when(self.snapshot.repeat != RepeatMode::Off, |b| {
                                            b.text_color(album_accent)
                                        })
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.backend.send(BackendCmd::CycleRepeat);
                                        })),
                                )
                                .child(div().size(px(4.0)).rounded_full().bg(
                                    if self.snapshot.repeat != RepeatMode::Off {
                                        album_accent
                                    } else {
                                        gpui_kit::transparent_black()
                                    },
                                )),
                        ),
                )
        } else {
            div()
                .text_size(px(18.0))
                .text_color(self.palette.secondary)
                .child("Nothing playing")
        };

        let left_stage = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(20.0))
            .child(cover_box)
            .child(track_meta_and_controls);

        let right_drawer: AnyElement = match self.fullscreen_tab {
            FullscreenDrawerTab::Queue => {
                let queue = self.snapshot.queue.clone();
                let current_idx = self.snapshot.current_index;
                let mut rows = div().flex().flex_col().gap(px(4.0)).p(px(16.0));
                for (i, t) in queue.into_iter().enumerate() {
                    let is_cur = current_idx == Some(i);
                    let cover = self.media_for_url(t.thumbnail_url.as_deref(), cx);
                    rows = rows.child(
                        div()
                            .id(SharedString::from(format!("fs-q-{i}")))
                            .h(px(52.0))
                            .px(px(10.0))
                            .rounded(px(self.palette.radii.item))
                            .cursor_pointer()
                            .bg(if is_cur {
                                self.palette.surface_active
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .hover(|s| s.bg(self.palette.surface_hover))
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.backend.send(BackendCmd::PlayQueueIndex(i));
                            }))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(cover_art_tile(
                                SharedString::from(format!("fs-qc-{i}")),
                                cover.as_ref(),
                                38.0,
                                false,
                                &self.palette,
                                is_cur && !self.snapshot.paused,
                                None,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(if is_cur {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_cur {
                                                self.palette.accent
                                            } else {
                                                self.palette.text
                                            })
                                            .truncate()
                                            .child(t.title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(self.palette.secondary)
                                            .truncate()
                                            .child(t.artist),
                                    ),
                            ),
                    );
                }
                card_panel(&self.palette)
                    .flex_1()
                    .h_full()
                    .child(
                        div()
                            .id("fs-q-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.fullscreen_queue_scroll)
                            .child(rows),
                    )
                    .into_any_element()
            }
            FullscreenDrawerTab::Lyrics => {
                let lyrics = self.snapshot.lyrics.clone();
                let pos = self.snapshot.position_secs;
                let body = match lyrics {
                    LyricsState::Synced(lines) => {
                        let (active_idx, active_bg_idx) = resolve_active_lyric_pair(&lines, pos);
                        let mut col = div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .px(px(20.0))
                            .py(px(64.0));
                        for (i, line) in lines.into_iter().enumerate() {
                            let glow = self.lyric_glow_factor(i, active_idx, active_bg_idx);
                            let is_past = active_idx.is_some_and(|a| i < a);
                            let is_bg = is_parenthetical_bg(&line.text);
                            let seek = line.time_secs;
                            col = col.child(
                                glowing_lyric_row(
                                    SharedString::from(format!("fs-lyr-{i}")),
                                    &line.text,
                                    line.voice,
                                    glow,
                                    is_past,
                                    is_bg,
                                    LyricRowStyle {
                                        base_font_size: 21.0,
                                        active_font_size: 28.0,
                                        base_line_height: 29.0,
                                        active_line_height: 36.0,
                                        pad_x: 12.0,
                                        pad_y: 6.0,
                                        radius: self.palette.radii.control,
                                        hover_bg: gpui_kit::white().opacity(0.08),
                                        pure_white_glow: true,
                                    },
                                    &self.palette,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(seek));
                                    },
                                )),
                            );
                        }
                        col.into_any_element()
                    }
                    LyricsState::Plain(lines) => div()
                        .p(px(28.0))
                        .text_size(px(18.0))
                        .line_height(px(28.0))
                        .text_color(self.palette.text)
                        .child(lines.join("\n"))
                        .into_any_element(),
                    LyricsState::Loading => div()
                        .p(px(28.0))
                        .text_size(px(16.0))
                        .text_color(self.palette.secondary)
                        .child("Loading lyrics...")
                        .into_any_element(),
                    _ => div()
                        .p(px(28.0))
                        .text_size(px(16.0))
                        .text_color(self.palette.dim)
                        .child("No lyrics available for this track")
                        .into_any_element(),
                };

                div()
                    .flex_1()
                    .h_full()
                    .min_h_0()
                    .relative()
                    .on_scroll(cx.listener(|this, _, _, _| {
                        this.last_wheel_lyrics = Some(Instant::now());
                    }))
                    .child(
                        div()
                            .id("fs-lyr-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.fullscreen_lyrics_scroll)
                            .child(body),
                    )
                    .into_any_element()
            }
        };

        div()
            .id("fullscreen-overlay")
            .absolute()
            .inset_0()
            .bg(deep_canvas)
            .when_some(art_media, |root, m| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .opacity(0.46)
                        .child(img(m.blurred).size_full().object_fit(ObjectFit::Cover)),
                )
            })
            .child(div().absolute().inset_0().bg(linear_gradient(
                145.,
                linear_color_stop(top_wash.opacity(0.62), 0.0),
                linear_color_stop(veil_color, 1.0),
            )))
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(top_bar)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .px(px(36.0))
                            .pb(px(28.0))
                            .pt(px(12.0))
                            .flex()
                            .when(!stacked, |el| el.flex_row().gap(px(40.0)))
                            .when(stacked, |el| el.flex_col().gap(px(20.0)))
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .when(!stacked, |d| d.w(px((vp_w * 0.44).clamp(320.0, 480.0))))
                                    .child(left_stage),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .h_full()
                                    .flex()
                                    .child(right_drawer),
                            ),
                    ),
            )
            .into_any_element()
    }
}
