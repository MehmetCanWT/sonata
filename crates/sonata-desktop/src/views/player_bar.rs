//! Transparent bottom player bar (`88 pt`) over the gutter with adaptive compact mode.
//!
//! 1. **Track Info Cluster**: hover-expanding `56 px` album artwork opening Fullscreen, `hover_marquee` title, clickable artist credits, explicit tag, and `ThumbsUp`/`ThumbsDown` feedback.
//! 2. **Transport & Seek**: centered Accent Play/Pause button framed by an animated `LoadingRing` while buffering, `Shuffle`/`Prev`/`Next`/`Repeat`, and seekable `Slider` + `mm:ss` timestamps.
//! 3. **Right Utility Cluster**: lossless quality pill, `Next up` & `Lyrics` floating card toggles, `Mute` + Volume `Slider`, and `Fullscreen` toggle.

use super::SonataDesktopView;
use crate::backend::BackendCmd;
use crate::settings::PLAYER_BAR_H;
use crate::ui::components::{
    artist_links_row, cover_art_tile, explicit_badge, hover_marquee, icon_action_button,
    loading_ring, quality_badge,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window,
};
use sonata_core::media::format_mm_ss;
use sonata_core::player::RepeatMode;

impl SonataDesktopView {
    pub fn render_player_bar(
        &mut self,
        vp_w: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = vp_w < 740.0;
        let current = self.snapshot.current_track().cloned();
        let paused = self.snapshot.paused;
        let loading = self.snapshot.loading_track;
        let art = self
            .media_for_path(self.snapshot.art_path.as_deref(), cx)
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|t| self.media_for_url(t.thumbnail_url.as_deref(), cx))
            });

        let left_track = if let Some(ref track) = current {
            let track_like = track.clone();
            let track_dislike = track.clone();
            let track_album = track.clone();
            let liked = self.snapshot.is_track_liked(track);
            let explicit = track.is_explicit;

            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .id("pb-cover-trigger")
                        .group("pb-cov-grp")
                        .relative()
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.fullscreen_open = !this.fullscreen_open;
                            this.fullscreen_last_activity = std::time::Instant::now();
                            cx.notify();
                        }))
                        .child(cover_art_tile(
                            "pb-cover",
                            art.as_ref(),
                            56.0,
                            false,
                            &self.palette,
                            false,
                            None,
                        ))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(px(self.palette.radii.art(56.0)))
                                .bg(gpui_kit::black().opacity(0.45))
                                .opacity(0.0)
                                .group_hover("pb-cov-grp", |s| s.opacity(1.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Maximize2)
                                        .size(px(18.0))
                                        .text_color(gpui_kit::white()),
                                ),
                        ),
                )
                .child(
                    div()
                        .min_w_0()
                        .max_w(px(if compact { 170.0 } else { 250.0 }))
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .min_w_0()
                                .child(
                                    div()
                                        .id("pb-title-click")
                                        .min_w_0()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !track_album.album.is_empty()
                                                || track_album.album_id.is_some()
                                            {
                                                this.open_album_for_track(&track_album, cx);
                                            }
                                        }))
                                        .child(hover_marquee(
                                            "pb-title-mq",
                                            &track.title,
                                            26,
                                            self.palette.text,
                                            14.0,
                                            window,
                                            cx,
                                        )),
                                )
                                .when(explicit, |r| {
                                    r.child(
                                        div().flex_shrink_0().child(explicit_badge(&self.palette)),
                                    )
                                }),
                        )
                        .child(div().h(px(18.0)).overflow_hidden().child(artist_links_row(
                            "pb-artist",
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
                        ))),
                )
                .when(!compact, |row| {
                    row.child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(icon_action_button(
                                "pb-dislike",
                                IconName::ThumbsDown,
                                false,
                                Some("Dislike"),
                                &self.palette,
                                {
                                    let b = self.backend.clone();
                                    move |_, _| {
                                        b.send(BackendCmd::DislikeTrack(track_dislike.clone()))
                                    }
                                },
                            ))
                            .child(icon_action_button(
                                "pb-like",
                                if liked {
                                    IconName::Heart
                                } else {
                                    IconName::ThumbsUp
                                },
                                liked,
                                Some(if liked { "Remove like" } else { "Like" }),
                                &self.palette,
                                {
                                    let b = self.backend.clone();
                                    move |_, _| b.send(BackendCmd::ToggleLike(track_like.clone()))
                                },
                            )),
                    )
                })
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(cover_art_tile(
                    "pb-empty-cover",
                    None,
                    56.0,
                    false,
                    &self.palette,
                    false,
                    None,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(self.palette.secondary)
                                .child("Nothing playing"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(self.palette.dim)
                                .child("Pick a track or mix to listen"),
                        ),
                )
                .into_any_element()
        };

        let repeat_icon = match self.snapshot.repeat {
            RepeatMode::One => IconName::Repeat1,
            _ => IconName::Repeat,
        };
        let repeat_active = self.snapshot.repeat != RepeatMode::Off;
        let shuffle_active = self.snapshot.shuffle;

        let play_btn = div()
            .id("pb-play-btn")
            .relative()
            .size(px(38.0))
            .rounded_full()
            .bg(self.palette.accent)
            .text_color(self.palette.on_accent)
            .cursor_pointer()
            .hover(|s| s.bg(self.palette.accent_hover))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, _, _| {
                this.backend.send(BackendCmd::TogglePause);
            }))
            .when(loading, |b| {
                b.child(loading_ring("pb-play-ring", 38.0, &self.palette))
            })
            .child(
                Icon::new(if paused {
                    IconName::Play
                } else {
                    IconName::Pause
                })
                .size(px(18.0)),
            );

        let transport_buttons = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .when(!compact, |r| {
                r.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.0))
                        .child(icon_action_button(
                            "pb-shuffle",
                            IconName::Shuffle,
                            shuffle_active,
                            Some(if shuffle_active {
                                "Shuffle: On (S)"
                            } else {
                                "Shuffle: Off (S)"
                            }),
                            &self.palette,
                            {
                                let b = self.backend.clone();
                                move |_, _| b.send(BackendCmd::ToggleShuffle)
                            },
                        ))
                        .child(div().size(px(4.0)).rounded_full().bg(if shuffle_active {
                            self.palette.accent
                        } else {
                            gpui_kit::transparent_black()
                        })),
                )
            })
            .child(icon_action_button(
                "pb-prev",
                IconName::SkipBack,
                false,
                Some("Previous track"),
                &self.palette,
                {
                    let b = self.backend.clone();
                    move |_, _| b.send(BackendCmd::PrevTrack)
                },
            ))
            .child(play_btn)
            .child(icon_action_button(
                "pb-next",
                IconName::SkipForward,
                false,
                Some("Next track"),
                &self.palette,
                {
                    let b = self.backend.clone();
                    move |_, _| b.send(BackendCmd::NextTrack)
                },
            ))
            .when(!compact, |r| {
                let repeat_tip = match self.snapshot.repeat {
                    RepeatMode::Off => "Repeat: Off (R)",
                    RepeatMode::All => "Repeat: All (R)",
                    RepeatMode::One => "Repeat: One (R)",
                };
                r.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.0))
                        .child(icon_action_button(
                            "pb-repeat",
                            repeat_icon,
                            repeat_active,
                            Some(repeat_tip),
                            &self.palette,
                            {
                                let b = self.backend.clone();
                                move |_, _| b.send(BackendCmd::CycleRepeat)
                            },
                        ))
                        .child(div().size(px(4.0)).rounded_full().bg(if repeat_active {
                            self.palette.accent
                        } else {
                            gpui_kit::transparent_black()
                        })),
                )
            });

        let pos_secs = if self.progress_dragging {
            let v = match self.progress_slider.read(cx).value() {
                gpui_kit::component::slider::SliderValue::Single(x) => x,
                gpui_kit::component::slider::SliderValue::Range(_, end) => end,
            };
            v as f64
        } else {
            self.snapshot.position_secs
        };

        let center_controls = div()
            .w(px(if compact { 230.0 } else { 440.0 }))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .child(transport_buttons)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(38.0))
                            .text_size(px(11.0))
                            .text_color(self.palette.dim)
                            .child(format_mm_ss(pos_secs.max(0.0).round() as u64)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Slider::new(&self.progress_slider).horizontal()),
                    )
                    .child(
                        div()
                            .w(px(38.0))
                            .text_size(px(11.0))
                            .text_color(self.palette.dim)
                            .child(format_mm_ss(
                                self.snapshot.duration_secs.max(0.0).round() as u64
                            )),
                    ),
            );

        let vol_icon = if self.snapshot.muted || self.snapshot.volume == 0 {
            IconName::VolumeX
        } else if self.snapshot.volume < 50 {
            IconName::Volume1
        } else {
            IconName::Volume2
        };

        let badge_text = self.snapshot.stream_badge.trim();
        let show_badge = !badge_text.is_empty() && badge_text != "Idle";

        let right_controls = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(6.0))
            .when(!compact && show_badge, |r| {
                r.child(quality_badge(
                    badge_text,
                    self.snapshot.upgrading_quality,
                    &self.palette,
                ))
            })
            .child(
                Button::new("pb-toggle-queue")
                    .ghost()
                    .small()
                    .icon(IconName::ListMusic)
                    .tooltip("Next up queue")
                    .when(self.settings.show_queue_panel, |b| {
                        b.text_color(self.palette.accent)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.show_queue_panel = !this.settings.show_queue_panel;
                        this.settings.save();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("pb-toggle-lyrics")
                    .ghost()
                    .small()
                    .icon(IconName::MicVocal)
                    .tooltip("Lyrics panel")
                    .when(self.settings.show_lyrics_panel, |b| {
                        b.text_color(self.palette.accent)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.show_lyrics_panel = !this.settings.show_lyrics_panel;
                        this.settings.save();
                        cx.notify();
                    })),
            )
            .when(!compact, |r| {
                r.child(icon_action_button(
                    "pb-mute",
                    vol_icon,
                    self.snapshot.muted,
                    Some("Mute (M)"),
                    &self.palette,
                    {
                        let b = self.backend.clone();
                        move |_, _| b.send(BackendCmd::ToggleMute)
                    },
                ))
                .child(
                    div()
                        .w(px(90.0))
                        .child(Slider::new(&self.volume_slider).horizontal()),
                )
            })
            .child(
                Button::new("pb-fullscreen")
                    .ghost()
                    .small()
                    .icon(IconName::Maximize2)
                    .tooltip("Now Playing (F)")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fullscreen_open = true;
                        this.fullscreen_last_activity = std::time::Instant::now();
                        cx.notify();
                    })),
            );

        div()
            .h(px(PLAYER_BAR_H))
            .w_full()
            .flex_shrink_0()
            .px(px(16.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(div().flex_1().min_w_0().child(left_track))
            .child(center_controls)
            .child(div().flex_1().min_w_0().child(right_controls))
            .into_any_element()
    }
}
