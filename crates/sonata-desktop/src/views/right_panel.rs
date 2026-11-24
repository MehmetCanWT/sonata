//! Stacked floating right side panels for `Next up` (queue) and live `Lyrics`.
//!
//! 1. **Stacked Cards**: renders one or both independent floating cards stacked vertically over an `8 pt` gutter whenever toggled on.
//! 2. **Queue Card**: displays active track highlight, track-level reordering/removal, queue shuffle, clear, and right-click menu.
//! 3. **Lyrics Card**: displays synced/plain/instrumental lyrics with click-to-seek, wheel-browse `3s` auto-scroll pause, and scroll-edge fades.

use super::{ContextMenuKind, ContextMenuState, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::GUTTER;
use crate::ui::components::{
    artist_links_row, card_panel, cover_art_tile, is_parenthetical_bg, lyric_line_end_secs,
    resolve_active_lyrics, scroll_edge_fades, secondary_lyric_slices,
};
use crate::ui::lyrics::{
    anchored_lyrics_offset, is_counter_voice, render_fixed_rows, render_karaoke_rows,
    KaraokeRenderStyle, SpringState, SPRING_LYRICS_SCROLL,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, point, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
};
use sonata_core::lyrics::LyricsState;
use sonata_core::providers::format_duration;
use std::time::{Duration, Instant};

const PANEL_LYRICS_PIN: f32 = 0.36;
const PANEL_WHEEL_SETTLE_SECS: u64 = 3;

impl SonataDesktopView {
    pub fn render_right_panel(
        &mut self,
        width_px: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let show_q = self.settings.show_queue_panel;
        let show_l = self.settings.show_lyrics_panel;
        if !show_q && !show_l {
            return div().into_any_element();
        }

        div()
            .w(px(width_px))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(GUTTER))
            .when(show_q, |col| col.child(self.render_queue_card(show_l, cx)))
            .when(show_l, |col| col.child(self.render_lyrics_card(window, cx)))
            .into_any_element()
    }

    fn render_queue_card(&mut self, stacked: bool, cx: &mut Context<Self>) -> AnyElement {
        let queue = self.snapshot.queue.clone();
        let current_idx = self.snapshot.current_index;
        let scroll_fades = self.settings.scroll_fades;

        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px(px(8.0))
            .pb(px(12.0));
        let s = sonata_runtime::i18n::strings(self.settings.language);
        if queue.is_empty() {
            list = list.child(
                div()
                    .px(px(12.0))
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child(s.queue_empty),
            );
        } else {
            // Window large queues around the active track so 200+ track queues stay 60fps.
            const MAX_QUEUE_WINDOW: usize = 64;
            let (win_start, win_end) = if queue.len() > MAX_QUEUE_WINDOW {
                let cur = current_idx.unwrap_or(0);
                let start = cur.saturating_sub(12);
                let end = (start + MAX_QUEUE_WINDOW).min(queue.len());
                let start = end.saturating_sub(MAX_QUEUE_WINDOW);
                (start, end)
            } else {
                (0, queue.len())
            };
            if win_start > 0 {
                list = list.child(
                    div()
                        .px(px(12.0))
                        .py(px(6.0))
                        .text_size(px(11.0))
                        .text_color(self.palette.dim)
                        .child(format!("{win_start} earlier tracks in queue")),
                );
            }
            for (idx, track) in queue
                .iter()
                .enumerate()
                .skip(win_start)
                .take(win_end - win_start)
            {
                let is_cur = current_idx == Some(idx);
                let cover = self.media_for_url(track.thumbnail_url.as_deref(), cx);
                let track_menu = track.clone();
                let artist_str = track.artist.clone();
                let len = queue.len();

                list = list.child(
                    div()
                        .id(SharedString::from(format!("rp-q-{idx}")))
                        .group(SharedString::from(format!("rp-q-grp-{idx}")))
                        .h(px(52.0))
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
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::PlayQueueIndex(idx));
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
                                    kind: ContextMenuKind::QueueItem {
                                        index: idx,
                                        track: track_menu.clone(),
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
                            SharedString::from(format!("rp-q-art-{idx}")),
                            cover.as_ref(),
                            38.0,
                            false,
                            &self.palette,
                            is_cur && !self.snapshot.paused,
                            Some(std::rc::Rc::new({
                                let b = self.backend.clone();
                                move |_, _| {
                                    if is_cur {
                                        b.send(BackendCmd::TogglePause);
                                    } else {
                                        b.send(BackendCmd::PlayQueueIndex(idx));
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
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
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
                                .child(div().h(px(16.0)).overflow_hidden().child(
                                    artist_links_row(
                                        &format!("rp-q-a-{idx}"),
                                        &artist_str,
                                        self.palette.secondary,
                                        11.0,
                                        {
                                            let view = cx.entity().downgrade();
                                            move |name, _, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.open_artist_by_name(&name, cx);
                                                });
                                            }
                                        },
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .opacity(0.0)
                                        .group_hover(
                                            SharedString::from(format!("rp-q-grp-{idx}")),
                                            |s| s.opacity(1.0),
                                        )
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .when(idx > 0, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-up-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ArrowUp)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend.send(BackendCmd::MoveQueueItem {
                                                        from: idx,
                                                        to: idx - 1,
                                                    });
                                                })),
                                            )
                                        })
                                        .when(idx + 1 < len, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-dn-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ArrowDown)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend.send(BackendCmd::MoveQueueItem {
                                                        from: idx,
                                                        to: idx + 1,
                                                    });
                                                })),
                                            )
                                        })
                                        .when(!is_cur, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-rm-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::X)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend
                                                        .send(BackendCmd::RemoveFromQueue(idx));
                                                })),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(self.palette.dim)
                                        .child(format_duration(track.duration_secs)),
                                ),
                        ),
                );
            }
            if win_end < queue.len() {
                list = list.child(
                    div()
                        .px(px(12.0))
                        .py(px(8.0))
                        .text_size(px(11.0))
                        .text_color(self.palette.dim)
                        .child(format!("+{} more tracks in queue", queue.len() - win_end)),
                );
            }
        }

        let card = card_panel(&self.palette)
            .flex_1()
            .min_h(px(if stacked { 180.0 } else { 220.0 }))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(46.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(self.palette.outline)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                Icon::new(IconName::ListMusic)
                                    .size(px(15.0))
                                    .text_color(self.palette.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .child(
                                        sonata_runtime::i18n::strings(self.settings.language)
                                            .next_up,
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                Button::new("rp-q-shuf")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Shuffle)
                                    .tooltip("Shuffle upcoming tracks")
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ShuffleQueue);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-clear")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Trash)
                                    .tooltip("Clear upcoming")
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ClearQueue);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-expand")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Maximize2)
                                    .tooltip("Open full Queue view")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate_to(crate::backend::NavTarget::Queue, cx);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.show_queue_panel = false;
                                        this.settings.save();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("rp-queue-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.queue_scroll)
                            .pt(px(6.0))
                            .child(list),
                    )
                    .child(scroll_edge_fades(
                        &self.queue_scroll,
                        &self.palette,
                        scroll_fades,
                    )),
            );

        card.into_any_element()
    }

    fn render_lyrics_card(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lyrics = self.snapshot.lyrics.clone();
        let translated_lyrics = self.snapshot.translated_lyrics.clone();
        let romanized_lyrics = self.snapshot.romanized_lyrics.clone();
        let tr_enabled = self.snapshot.config.lyrics_translate;
        let rm_enabled = self.snapshot.config.lyrics_romanize;
        let scroll_fades = self.settings.scroll_fades;
        let scroll_bounds = self.lyrics_scroll.bounds();
        let view_h: f32 = scroll_bounds.size.height.into();
        let view_top: f32 = scroll_bounds.origin.y.into();
        let effective_h = if view_h > 40.0 { view_h } else { 320.0 };
        let user_scrolling = self
            .last_wheel_lyrics
            .is_some_and(|t| t.elapsed() < Duration::from_secs(PANEL_WHEEL_SETTLE_SECS));

        let s = sonata_runtime::i18n::strings(self.settings.language);
        let scroll_body: AnyElement = match lyrics {
            LyricsState::Idle => div()
                .id("rp-lyrics-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.lyrics_scroll)
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child(s.select_song_lyrics)
                .into_any_element(),
            LyricsState::Loading => div()
                .id("rp-lyrics-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.lyrics_scroll)
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child(s.fetching_lyrics)
                .into_any_element(),
            LyricsState::NotFound => div()
                .id("rp-lyrics-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.lyrics_scroll)
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child(s.no_lyrics_found)
                .into_any_element(),
            LyricsState::Plain(lines) => {
                let mut col = div()
                    .id("rp-lyrics-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.lyrics_scroll)
                    .p(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0));
                for (i, line) in lines.iter().enumerate() {
                    let (rom, tr) = secondary_lyric_slices(
                        i,
                        line,
                        romanized_lyrics.as_deref(),
                        translated_lyrics.as_deref(),
                    );
                    let mut entry = div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .line_height(px(20.0))
                                .text_color(self.palette.secondary)
                                .child(line.clone()),
                        );
                    if let Some(rom_text) = rom {
                        entry = entry.child(
                            div()
                                .text_size(px(11.5))
                                .line_height(px(16.0))
                                .text_color(self.palette.dim)
                                .child(rom_text.to_string()),
                        );
                    }
                    if let Some(tr_text) = tr {
                        entry = entry.child(
                            div()
                                .text_size(px(12.0))
                                .line_height(px(16.0))
                                .text_color(self.palette.secondary.opacity(0.75))
                                .child(tr_text.to_string()),
                        );
                    }
                    col = col.child(entry);
                }
                col.into_any_element()
            }
            LyricsState::Synced(lines) => {
                if !self.snapshot.paused && !self.snapshot.loading_track && !lines.is_empty() {
                    self.ensure_karaoke_pace(cx);
                }

                let pos = self.snapshot.lyrics_position_secs(self.live_position_now());
                let (active_idx, active_bg_indices) = resolve_active_lyrics(&lines, pos);
                let pad_top = (effective_h * PANEL_LYRICS_PIN).max(48.0);
                let pad_bottom = (effective_h * (1.0 - PANEL_LYRICS_PIN)).max(96.0);
                let view_w: f32 = scroll_bounds.size.width.into();
                let base_size = 14.5_f32;
                let active_size = 16.5_f32;
                let line_h = 23.0_f32;
                let pad_x = 8.0_f32;
                let wrap_width = if view_w > 60.0 {
                    (view_w - 28.0 - pad_x * 2.0).max(80.0)
                } else {
                    220.0
                };
                if self
                    .panel_lyrics_wrap
                    .sync_layout(&lines, wrap_width, active_size)
                {
                    window.request_animation_frame();
                }

                // Smooth physical spring scroll keeping the active lyric centered in the side panel.
                let dt = self
                    .lyrics_panel_last_step
                    .elapsed()
                    .as_secs_f32()
                    .clamp(0.001, 0.05);
                self.lyrics_panel_last_step = Instant::now();
                let target_idx = active_idx.unwrap_or(0);
                let cur_off: f32 = self.lyrics_scroll.offset().y.into();

                if user_scrolling {
                    self.panel_scroll_spring = SpringState::at(cur_off);
                } else if view_h > 40.0 {
                    let child_ix = target_idx + 1;
                    if let Some(item_bounds) = self.lyrics_scroll.bounds_for_item(child_ix) {
                        let item_top: f32 = item_bounds.origin.y.into();
                        let item_h: f32 = item_bounds.size.height.into();
                        let max_off: f32 = self.lyrics_scroll.max_offset().y.into();
                        let goal = anchored_lyrics_offset(
                            view_top,
                            item_top,
                            view_h,
                            max_off,
                            PANEL_LYRICS_PIN,
                            item_h * 0.20,
                        );
                        if (self.panel_scroll_spring.position - cur_off).abs() > 4.0
                            && self.panel_scroll_spring.velocity.abs() < 1.0
                        {
                            self.panel_scroll_spring = SpringState::at(cur_off);
                        }
                        let diff = goal - self.panel_scroll_spring.position;
                        if diff.abs() > view_h * 1.6 || self.snapshot.config.reduce_motion {
                            self.panel_scroll_spring = SpringState::at(goal);
                        } else {
                            let next_spring =
                                SPRING_LYRICS_SCROLL.step(self.panel_scroll_spring, goal, dt);
                            if SPRING_LYRICS_SCROLL.is_settled(next_spring, goal, 0.35) {
                                self.panel_scroll_spring = SpringState::at(goal);
                            } else {
                                self.panel_scroll_spring = next_spring;
                                window.request_animation_frame();
                            }
                        }
                        let next_y = self
                            .panel_scroll_spring
                            .position
                            .clamp(-max_off.max(0.0), 0.0);
                        self.lyrics_scroll.set_offset(point(px(0.0), px(next_y)));
                    } else {
                        window.request_animation_frame();
                    }
                }

                let mut col = div()
                    .id("rp-lyrics-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.lyrics_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .px(px(14.0))
                    .child(div().flex_shrink_0().h(px(pad_top)));

                for (i, line) in lines.iter().enumerate() {
                    let is_active_bg = active_bg_indices.contains(&i);
                    let is_active = active_idx == Some(i) || is_active_bg;
                    let glow = self.lyric_glow_factor(i, active_idx, &active_bg_indices);
                    let is_past = !is_active_bg && active_idx.is_some_and(|a| i < a);
                    let is_bg = is_parenthetical_bg(&line.text);
                    let is_counter = is_counter_voice(line.voice);
                    let seek_secs = line.time_secs;
                    let next_time = Some(lyric_line_end_secs(&lines, i));

                    let base_sz = if is_bg { base_size * 0.88 } else { base_size };
                    let act_sz = if is_bg {
                        active_size * 0.90
                    } else {
                        active_size
                    };
                    let sz = base_sz + (act_sz - base_sz) * glow;
                    let wrapped = self.panel_lyrics_wrap.get_or_measure(
                        i,
                        line,
                        next_time,
                        active_size,
                        wrap_width,
                        window,
                    );

                    let content: AnyElement = if is_active {
                        let base_color = self.palette.secondary.opacity(0.46);
                        let lit_color = self.palette.text;
                        if let Some(plan) = wrapped {
                            render_karaoke_rows(
                                plan,
                                line,
                                pos,
                                KaraokeRenderStyle {
                                    font_size: sz,
                                    line_height: line_h,
                                    base_color,
                                    lit_color,
                                    is_counter,
                                    is_bg,
                                },
                            )
                        } else {
                            div()
                                .w_full()
                                .min_w_0()
                                .text_size(px(sz))
                                .line_height(px(line_h))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(lit_color)
                                .child(line.text.clone())
                                .into_any_element()
                        }
                    } else {
                        let color = if is_past {
                            self.palette.dim.opacity(0.70)
                        } else {
                            self.palette.secondary.opacity(0.76)
                        };
                        if let Some(plan) = wrapped {
                            render_fixed_rows(&plan.text, sz, line_h, color, 0.0, is_counter, is_bg)
                        } else {
                            let fallback = [SharedString::from(line.text.clone())];
                            render_fixed_rows(&fallback, sz, line_h, color, 0.0, is_counter, is_bg)
                        }
                    };

                    let (rom, tr) = secondary_lyric_slices(
                        i,
                        &line.text,
                        romanized_lyrics.as_deref(),
                        translated_lyrics.as_deref(),
                    );
                    let sec_color = if is_active {
                        self.palette.secondary.opacity(0.86)
                    } else if is_past {
                        self.palette.dim.opacity(0.56)
                    } else {
                        self.palette.dim.opacity(0.74)
                    };

                    col = col.child(
                        div()
                            .id(SharedString::from(format!("rp-lyr-{i}")))
                            .w_full()
                            .min_w_0()
                            .px(px(pad_x))
                            .py(px(5.0))
                            .rounded(px(self.palette.radii.small))
                            .cursor_pointer()
                            .hover(|st| st.bg(self.palette.surface_hover))
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.last_wheel_lyrics = None;
                                this.backend.send(BackendCmd::SeekAbsolute(seek_secs));
                            }))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .when(!is_counter, |d| d.items_start())
                            .when(is_counter, |d| d.items_end())
                            .child(content)
                            .when_some(rom, |d, rom_text| {
                                d.child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .text_size(px(11.5))
                                        .line_height(px(15.5))
                                        .text_color(sec_color)
                                        .when(is_counter, |t| t.text_right())
                                        .child(rom_text.to_string()),
                                )
                            })
                            .when_some(tr, |d, tr_text| {
                                d.child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .text_size(px(12.0))
                                        .line_height(px(16.0))
                                        .text_color(sec_color)
                                        .when(is_counter, |t| t.text_right())
                                        .child(tr_text.to_string()),
                                )
                            }),
                    );
                }
                col.child(div().flex_shrink_0().h(px(pad_bottom)))
                    .into_any_element()
            }
        };

        card_panel(&self.palette)
            .flex_1()
            .min_h(px(180.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(46.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(self.palette.outline)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                Icon::new(IconName::MicVocal)
                                    .size(px(15.0))
                                    .text_color(self.palette.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .child(
                                        sonata_runtime::i18n::strings(self.settings.language)
                                            .lyrics,
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("rp-lyr-rm")
                                    .ghost()
                                    .xsmall()
                                    .label("Rm")
                                    .text_color(if rm_enabled {
                                        self.palette.accent
                                    } else {
                                        self.palette.dim
                                    })
                                    .tooltip(s.romanize_lyrics)
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ToggleLyricsRomanize);
                                    })),
                            )
                            .child(
                                Button::new("rp-lyr-tr")
                                    .ghost()
                                    .xsmall()
                                    .label("Tr")
                                    .text_color(if tr_enabled {
                                        self.palette.accent
                                    } else {
                                        self.palette.dim
                                    })
                                    .tooltip(s.translate_lyrics)
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ToggleLyricsTranslate);
                                    })),
                            )
                            .child(
                                Button::new("rp-lyr-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.show_lyrics_panel = false;
                                        this.settings.save();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .on_scroll(cx.listener(|this, _, _, _| {
                        this.last_wheel_lyrics = Some(Instant::now());
                    }))
                    .child(scroll_body)
                    .child(scroll_edge_fades(
                        &self.lyrics_scroll,
                        &self.palette,
                        scroll_fades,
                    )),
            )
            .into_any_element()
    }
}
