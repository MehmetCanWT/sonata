//! Fullscreen transport dock, playback options pills, corner mode switcher, and queue drawer.
//!
//! 1. **Track Meta & Transport**: renders centered track title, explicit badge, artist links,
//!    progress scrubber, transport buttons, speed/silence/sleep pills, and video quality chips.
//! 2. **Corner Pill & Queue Drawer**: renders the bottom-left minimize, Cover/Lyrics/Queue mode
//!    switcher, `Rm`/`Tr` lyric toggles, and windowed queue track list.

use crate::backend::BackendCmd;
use crate::ui::components::{artist_links_row, cover_art_tile, loading_ring};
use crate::views::{FullscreenDrawerTab, SonataDesktopView};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _,
};
use sonata_core::media::format_mm_ss;
use sonata_core::player::RepeatMode;
use sonata_core::providers::Track;

impl SonataDesktopView {
    pub(super) fn render_fullscreen_track_meta_and_controls(
        &mut self,
        current: Option<&Track>,
        art_size: f32,
        controls_visible: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Balanced 3-part title row keeps title mathematically centered under artwork
        let Some(track) = current else {
            return div()
                .text_size(px(18.0))
                .text_color(gpui_kit::white().opacity(0.52))
                .child(sonata_runtime::i18n::strings(self.settings.language).nothing_playing)
                .into_any_element();
        };

        let pos_secs = if self.progress_dragging {
            match self.progress_slider.read(cx).value() {
                gpui_kit::component::slider::SliderValue::Single(x) => x as f64,
                gpui_kit::component::slider::SliderValue::Range(_, e) => e as f64,
            }
        } else {
            self.live_position_secs
        };
        let track_like = track.clone();
        let repeat_icon = match self.snapshot.repeat {
            RepeatMode::One => IconName::Repeat1,
            _ => IconName::Repeat,
        };
        let title_size = if track.title.chars().count() > 34 {
            20.0
        } else {
            23.0
        };

        let explicit_el = |visible: bool| {
            div()
                .flex_shrink_0()
                .h(px(16.0))
                .min_w(px(16.0))
                .px(px(4.0))
                .rounded(px(3.0))
                .bg(gpui_kit::white().opacity(if visible { 0.14 } else { 0.0 }))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.5))
                .line_height(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(gpui_kit::white().opacity(if visible { 0.62 } else { 0.0 }))
                .child("E")
        };

        div()
            .w(px(art_size.max(340.0)))
            .max_w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .when(track.is_explicit, |r| r.child(explicit_el(false))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .max_w(px(art_size))
                            .text_size(px(title_size))
                            .line_height(px(title_size + 6.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(gpui_kit::white().opacity(0.96))
                            .text_center()
                            .truncate()
                            .child(track.title.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_start()
                            .when(track.is_explicit, |r| r.child(explicit_el(true))),
                    ),
            )
            .child(div().flex().justify_center().child(artist_links_row(
                "fs-artist",
                &track.artist,
                gpui_kit::white().opacity(0.56),
                14.0,
                {
                    let view = cx.entity().downgrade();
                    move |name, _, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.open_artist_by_name(&name, cx);
                        });
                    }
                },
            )))
            .when(controls_visible, |col| {
                col.child(
                    div()
                        .id("fs-controls-dock")
                        .w_full()
                        .max_w(px(380.0))
                        .pt(px(14.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(Slider::new(&self.progress_slider).horizontal())
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .text_size(px(11.0))
                                        .text_color(gpui_kit::white().opacity(0.50))
                                        .child(format_mm_ss(pos_secs.max(0.0).round() as u64))
                                        .child(format_mm_ss(
                                            self.live_duration_secs.max(0.0).round() as u64,
                                        )),
                                ),
                        )
                        .child(
                            // Three-column symmetric transport row keeps the Play/Pause button
                            // mathematically centered under the cover, title, and scrubber.
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(14.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(12.0))
                                        .child(
                                            Button::new("fs-shuf")
                                                .ghost()
                                                .small()
                                                .icon(IconName::Shuffle)
                                                .text_color(if self.snapshot.shuffle {
                                                    gpui_kit::white()
                                                } else {
                                                    gpui_kit::white().opacity(0.54)
                                                })
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    this.backend.send(BackendCmd::ToggleShuffle);
                                                })),
                                        )
                                        .child(
                                            Button::new("fs-prev")
                                                .ghost()
                                                .icon(IconName::SkipBack)
                                                .text_color(gpui_kit::white().opacity(0.90))
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    this.backend.send(BackendCmd::PrevTrack);
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("fs-play-btn")
                                        .flex_shrink_0()
                                        .relative()
                                        .size(px(42.0))
                                        .rounded_full()
                                        .bg(gpui_kit::white().opacity(0.94))
                                        .text_color(gpui_kit::black())
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.86))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.backend.send(BackendCmd::TogglePause);
                                        }))
                                        .when(self.snapshot.loading_track, |b| {
                                            b.child(loading_ring("fs-ring", 42.0, &self.palette))
                                        })
                                        .child(
                                            Icon::new(if self.snapshot.paused {
                                                IconName::Play
                                            } else {
                                                IconName::Pause
                                            })
                                            .size(px(18.0)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_start()
                                        .gap(px(12.0))
                                        .child(
                                            Button::new("fs-next")
                                                .ghost()
                                                .icon(IconName::SkipForward)
                                                .text_color(gpui_kit::white().opacity(0.90))
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    this.backend.send(BackendCmd::NextTrack);
                                                })),
                                        )
                                        .child(
                                            Button::new("fs-rep")
                                                .ghost()
                                                .small()
                                                .icon(repeat_icon)
                                                .text_color(
                                                    if self.snapshot.repeat != RepeatMode::Off {
                                                        gpui_kit::white()
                                                    } else {
                                                        gpui_kit::white().opacity(0.54)
                                                    },
                                                )
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    this.backend.send(BackendCmd::CycleRepeat);
                                                })),
                                        )
                                        .child(
                                            Button::new("fs-autoplay")
                                                .ghost()
                                                .small()
                                                .icon(IconName::Infinity)
                                                .text_color(if self.snapshot.autoplay {
                                                    gpui_kit::white()
                                                } else {
                                                    gpui_kit::white().opacity(0.54)
                                                })
                                                .on_click(cx.listener(|this, _, _, _| {
                                                    this.backend.send(BackendCmd::ToggleAutoplay);
                                                })),
                                        )
                                        .child(
                                            Button::new("fs-like")
                                                .ghost()
                                                .small()
                                                .icon(IconName::ThumbsUp)
                                                .text_color(if track.liked {
                                                    gpui_kit::white()
                                                } else {
                                                    gpui_kit::white().opacity(0.54)
                                                })
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend.send(BackendCmd::ToggleLike(
                                                        track_like.clone(),
                                                    ));
                                                })),
                                        ),
                                ),
                        )
                        .child({
                            use crate::backend::playback::{
                                format_sleep_countdown, next_sleep_preset, step_speed_preset,
                            };
                            let s = sonata_runtime::i18n::strings(self.settings.language);
                            let has_mv = self.snapshot.music_video_candidate.is_some();
                            let video_on = self.snapshot.video_mode;
                            let video_loading = self.snapshot.video_loading;
                            let cur_speed = self.snapshot.playback_speed;
                            let speed_active = (cur_speed - 1.0).abs() > 0.01;
                            let skip_on = self.snapshot.skip_silence;
                            let sleep_rem = self.snapshot.sleep_timer_remaining_secs;
                            let cur_vq = self.snapshot.config.video_quality.clone();
                            let vq_options = available_video_quality_options(
                                &self.snapshot.available_video_qualities,
                            );

                            let pill_chip = |id: &'static str, label: String, active: bool| {
                                div()
                                    .id(id)
                                    .h(px(24.0))
                                    .px(px(9.0))
                                    .rounded_full()
                                    .cursor_pointer()
                                    .bg(if active {
                                        gpui_kit::white().opacity(0.22)
                                    } else {
                                        gpui_kit::white().opacity(0.08)
                                    })
                                    .border_1()
                                    .border_color(if active {
                                        gpui_kit::white().opacity(0.45)
                                    } else {
                                        gpui_kit::white().opacity(0.12)
                                    })
                                    .hover(|st| st.bg(gpui_kit::white().opacity(0.18)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if active {
                                        gpui_kit::white()
                                    } else {
                                        gpui_kit::white().opacity(0.76)
                                    })
                                    .child(label)
                            };

                            let speed_lbl = if (cur_speed - cur_speed.round()).abs() < 0.01 {
                                format!("{cur_speed:.0}x")
                            } else {
                                format!("{cur_speed:.2}x")
                                    .trim_end_matches('0')
                                    .trim_end_matches('.')
                                    .to_string()
                                    + "x"
                            };
                            let sleep_lbl = match sleep_rem {
                                Some(rem) => format_sleep_countdown(rem),
                                None => s.sleep_timer.to_string(),
                            };

                            div()
                                .w_full()
                                .pt(px(2.0))
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .flex_wrap()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(6.0))
                                        .when(has_mv, |row| {
                                            row.child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(2.0))
                                                    .p(px(2.0))
                                                    .rounded_full()
                                                    .bg(gpui_kit::black().opacity(0.32))
                                                    .border_1()
                                                    .border_color(gpui_kit::white().opacity(0.12))
                                                    .child(
                                                        pill_chip(
                                                            "fs-mode-music",
                                                            s.music.to_string(),
                                                            !video_on,
                                                        )
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.backend.send(
                                                                BackendCmd::SetVideoMode(false),
                                                            );
                                                            this.stir_fullscreen(cx);
                                                        })),
                                                    )
                                                    .child(
                                                        pill_chip(
                                                            "fs-mode-video",
                                                            if video_loading {
                                                                s.loading_video.to_string()
                                                            } else {
                                                                s.video.to_string()
                                                            },
                                                            video_on,
                                                        )
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.backend.send(
                                                                BackendCmd::SetVideoMode(true),
                                                            );
                                                            this.stir_fullscreen(cx);
                                                        })),
                                                    ),
                                            )
                                        })
                                        .child(
                                            pill_chip("fs-speed", speed_lbl, speed_active)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    let next = step_speed_preset(cur_speed, true);
                                                    this.backend
                                                        .send(BackendCmd::SetPlaybackSpeed(next));
                                                    this.stir_fullscreen(cx);
                                                }))
                                                .on_mouse_down(
                                                    MouseButton::Right,
                                                    cx.listener(|this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        this.backend.send(
                                                            BackendCmd::SetPlaybackSpeed(1.0),
                                                        );
                                                        this.stir_fullscreen(cx);
                                                    }),
                                                ),
                                        )
                                        .child(
                                            pill_chip(
                                                "fs-skip-silence",
                                                s.skip_silence.to_string(),
                                                skip_on,
                                            )
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.backend
                                                        .send(BackendCmd::ToggleSkipSilence);
                                                    this.stir_fullscreen(cx);
                                                }),
                                            ),
                                        )
                                        .child(
                                            pill_chip(
                                                "fs-sleep-timer",
                                                sleep_lbl,
                                                sleep_rem.is_some(),
                                            )
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    let next = next_sleep_preset(sleep_rem, true);
                                                    this.backend
                                                        .send(BackendCmd::SetSleepTimer(next));
                                                    this.stir_fullscreen(cx);
                                                }),
                                            ),
                                        ),
                                )
                                .when(has_mv && video_on, |col| {
                                    col.child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_center()
                                            .gap(px(4.0))
                                            .children(vq_options.into_iter().map(|opt| {
                                                let opt_owned = opt.to_string();
                                                let active =
                                                    cur_vq.eq_ignore_ascii_case(opt.trim());
                                                let label = if opt == "Auto" {
                                                    s.auto_label.to_string()
                                                } else {
                                                    opt.to_string()
                                                };
                                                div()
                                                    .id(SharedString::from(format!("fs-vq-{opt}")))
                                                    .h(px(22.0))
                                                    .px(px(7.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .bg(if active {
                                                        gpui_kit::white().opacity(0.22)
                                                    } else {
                                                        gpui_kit::black().opacity(0.28)
                                                    })
                                                    .border_1()
                                                    .border_color(if active {
                                                        gpui_kit::white().opacity(0.45)
                                                    } else {
                                                        gpui_kit::white().opacity(0.10)
                                                    })
                                                    .hover(|st| {
                                                        st.bg(gpui_kit::white().opacity(0.16))
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(10.5))
                                                    .text_color(if active {
                                                        gpui_kit::white()
                                                    } else {
                                                        gpui_kit::white().opacity(0.68)
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.backend.send(
                                                            BackendCmd::SetVideoQuality(
                                                                opt_owned.clone(),
                                                            ),
                                                        );
                                                        this.stir_fullscreen(cx);
                                                    }))
                                                    .child(label)
                                            })),
                                    )
                                })
                        }),
                )
            })
            .into_any_element()
    }

    pub(super) fn render_fullscreen_corner_pill(&self, cx: &mut Context<Self>) -> AnyElement {
        let pill_btn = |id: &'static str,
                        icon: IconName,
                        active: bool,
                        tab: FullscreenDrawerTab,
                        cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(32.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .bg(if active {
                    gpui_kit::white().opacity(0.16)
                } else {
                    gpui_kit::transparent_black()
                })
                .hover(|s| s.bg(gpui_kit::white().opacity(0.12)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.fullscreen_tab = tab;
                    this.stir_fullscreen(cx);
                    cx.notify();
                }))
                .child(Icon::new(icon).size(px(15.0)).text_color(if active {
                    gpui_kit::white()
                } else {
                    gpui_kit::white().opacity(0.58)
                }))
        };

        div()
            .id("fs-corner")
            .absolute()
            .bottom(px(20.0))
            .left(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("fs-minimize-btn")
                    .size(px(38.0))
                    .rounded(px(11.0))
                    .bg(gpui_kit::black().opacity(0.36))
                    .border_1()
                    .border_color(gpui_kit::white().opacity(0.10))
                    .cursor_pointer()
                    .hover(|s| s.bg(gpui_kit::white().opacity(0.14)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.fullscreen_open = false;
                        if window.is_fullscreen() {
                            window.toggle_fullscreen();
                        }
                        cx.notify();
                    }))
                    .child(
                        Icon::new(IconName::Minimize2)
                            .size(px(15.0))
                            .text_color(gpui_kit::white().opacity(0.86)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .p(px(3.0))
                    .rounded(px(11.0))
                    .bg(gpui_kit::black().opacity(0.36))
                    .border_1()
                    .border_color(gpui_kit::white().opacity(0.10))
                    .child(pill_btn(
                        "fs-tab-cover",
                        IconName::Disc3,
                        self.fullscreen_tab == FullscreenDrawerTab::Cover,
                        FullscreenDrawerTab::Cover,
                        cx,
                    ))
                    .child(pill_btn(
                        "fs-tab-lyrics",
                        IconName::Mic,
                        self.fullscreen_tab == FullscreenDrawerTab::Lyrics,
                        FullscreenDrawerTab::Lyrics,
                        cx,
                    ))
                    .child(pill_btn(
                        "fs-tab-queue",
                        IconName::ListMusic,
                        self.fullscreen_tab == FullscreenDrawerTab::Queue,
                        FullscreenDrawerTab::Queue,
                        cx,
                    )),
            )
            .when(self.fullscreen_tab == FullscreenDrawerTab::Lyrics, |row| {
                let tr_on = self.snapshot.config.lyrics_translate;
                let rm_on = self.snapshot.config.lyrics_romanize;
                row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(2.0))
                        .p(px(3.0))
                        .rounded(px(11.0))
                        .bg(gpui_kit::black().opacity(0.36))
                        .border_1()
                        .border_color(gpui_kit::white().opacity(0.10))
                        .child(
                            div()
                                .id("fs-lyr-rm")
                                .h(px(32.0))
                                .px(px(9.0))
                                .rounded(px(8.0))
                                .cursor_pointer()
                                .bg(if rm_on {
                                    gpui_kit::white().opacity(0.18)
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.12)))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if rm_on {
                                    gpui_kit::white()
                                } else {
                                    gpui_kit::white().opacity(0.62)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.backend.send(BackendCmd::ToggleLyricsRomanize);
                                    this.stir_fullscreen(cx);
                                }))
                                .child("Rm"),
                        )
                        .child(
                            div()
                                .id("fs-lyr-tr")
                                .h(px(32.0))
                                .px(px(9.0))
                                .rounded(px(8.0))
                                .cursor_pointer()
                                .bg(if tr_on {
                                    gpui_kit::white().opacity(0.18)
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.12)))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if tr_on {
                                    gpui_kit::white()
                                } else {
                                    gpui_kit::white().opacity(0.62)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.backend.send(BackendCmd::ToggleLyricsTranslate);
                                    this.stir_fullscreen(cx);
                                }))
                                .child("Tr"),
                        ),
                )
            })
            .into_any_element()
    }

    pub(super) fn render_fullscreen_queue(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let queue = self.snapshot.queue.clone();
        let current_idx = self.snapshot.current_index;
        let mut rows = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(12.0))
            .py(px(40.0));
        const MAX_FS_QUEUE: usize = 80;
        let (win_start, win_end) = if queue.len() > MAX_FS_QUEUE {
            let cur = current_idx.unwrap_or(0);
            let start = cur.saturating_sub(15);
            let end = (start + MAX_FS_QUEUE).min(queue.len());
            (end.saturating_sub(MAX_FS_QUEUE), end)
        } else {
            (0, queue.len())
        };
        for (i, t) in queue
            .into_iter()
            .enumerate()
            .skip(win_start)
            .take(win_end - win_start)
        {
            let is_cur = current_idx == Some(i);
            let cover = self.media_for_url(t.thumbnail_url.as_deref(), cx);
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("fs-q-{i}")))
                    .h(px(54.0))
                    .px(px(12.0))
                    .rounded(px(10.0))
                    .cursor_pointer()
                    .bg(if is_cur {
                        gpui_kit::white().opacity(0.12)
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .hover(|s| s.bg(gpui_kit::white().opacity(0.08)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.backend.send(BackendCmd::PlayQueueIndex(i));
                    }))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(cover_art_tile(
                        SharedString::from(format!("fs-qc-{i}")),
                        cover.as_ref(),
                        40.0,
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
                                    .text_size(px(14.5))
                                    .font_weight(if is_cur {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_cur {
                                        gpui_kit::white()
                                    } else {
                                        gpui_kit::white().opacity(0.84)
                                    })
                                    .truncate()
                                    .child(t.title),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(gpui_kit::white().opacity(0.52))
                                    .truncate()
                                    .child(t.artist),
                            ),
                    ),
            );
        }

        div()
            .flex_1()
            .h_full()
            .min_h_0()
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
}

/// Filters [`crate::backend::playback::VIDEO_QUALITY_OPTIONS`] against resolved stream heights,
/// preserving `"Auto"` and falling back to all presets until streams finish loading.
fn available_video_quality_options(
    streams: &[sonata_core::providers::hifi::VideoQualityStream],
) -> Vec<&'static str> {
    use crate::backend::playback::VIDEO_QUALITY_OPTIONS;
    if streams.is_empty() {
        return VIDEO_QUALITY_OPTIONS.to_vec();
    }
    let filtered: Vec<&'static str> = VIDEO_QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|opt| {
            *opt == "Auto"
                || opt
                    .trim_end_matches('p')
                    .parse::<u32>()
                    .ok()
                    .is_some_and(|h| streams.iter().any(|s| s.height == h))
        })
        .collect();
    if filtered.len() <= 1 {
        VIDEO_QUALITY_OPTIONS.to_vec()
    } else {
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_video_quality_options_filters_to_discovered_heights() {
        use sonata_core::providers::hifi::VideoQualityStream;

        assert_eq!(
            available_video_quality_options(&[]),
            vec!["Auto", "1080p", "720p", "480p", "360p"]
        );

        let streams = vec![
            VideoQualityStream {
                quality_label: "720p".to_string(),
                height: 720,
                fps: 30,
                video_url: "https://example.invalid/720".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate_bps: 1_500_000,
                is_muxed: false,
            },
            VideoQualityStream {
                quality_label: "360p".to_string(),
                height: 360,
                fps: 30,
                video_url: "https://example.invalid/360".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate_bps: 500_000,
                is_muxed: true,
            },
        ];
        assert_eq!(
            available_video_quality_options(&streams),
            vec!["Auto", "720p", "360p"]
        );
    }
}
