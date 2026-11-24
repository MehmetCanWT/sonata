//! Fullscreen pinned depth-of-field lyrics renderer with karaoke sweep and spring scroll.
//!
//! 1. **Display Rows**: inserts instrumental break rows for >=5 s gaps alongside synced verses.
//! 2. **Spring Scroll & Culling**: pins the active verse at 30% viewport height, applies per-row
//!    cascading stagger drift, culls offscreen rows, and dissolves viewport edges cleanly.

use crate::backend::BackendCmd;
use crate::ui::components::{
    is_parenthetical_bg, lyric_line_end_secs, resolve_active_lyrics, secondary_lyric_slices,
};
use crate::ui::lyrics::{
    anchored_lyrics_offset, ease_out_expo, lag_row_spring, render_fixed_rows, render_karaoke_rows,
    KaraokeRenderStyle, SpringState, SPRING_LYRICS_SCROLL,
};
use crate::views::SonataDesktopView;
use gpui_kit::{
    div, point, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
};
use sonata_core::lyrics::{LyricLine, LyricVoice, LyricsState};
use std::time::{Duration, Instant};

const LYRICS_FRAME_W: f32 = 1152.0;
const LYRICS_FRAME_H: f32 = 648.0;
const LYRICS_PIN: f32 = 0.30;
const VERSE_GAP_FACTOR: f32 = 0.60;
const VERSE_PAD_X_FACTOR: f32 = 0.45;
const VERSE_PAD_Y_FACTOR: f32 = 0.15;
const ACTIVE_VERSE_GROWTH: f32 = 2.0;
const INSTRUMENTAL_BREAK_SECS: f64 = 5.0;
const WHEEL_SETTLE_SECS: u64 = 4;
const VERSE_TRANSITION_SECS: f32 = 0.22;

#[derive(Clone)]
enum DisplayLyricRow {
    Instrumental {
        start_secs: f64,
        end_secs: f64,
    },
    Verse {
        line_idx: usize,
        line: LyricLine,
        next_time: Option<f64>,
    },
}

fn build_display_rows(lines: &[LyricLine]) -> Vec<DisplayLyricRow> {
    let mut out = Vec::with_capacity(lines.len() + 6);
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 && line.time_secs >= INSTRUMENTAL_BREAK_SECS {
            out.push(DisplayLyricRow::Instrumental {
                start_secs: 0.0,
                end_secs: line.time_secs,
            });
        } else if idx > 0 {
            let prev = &lines[idx - 1];
            let prev_end = prev
                .words
                .last()
                .and_then(|w| w.end_secs)
                .unwrap_or(prev.time_secs + 3.0);
            if line.time_secs - prev_end >= INSTRUMENTAL_BREAK_SECS {
                out.push(DisplayLyricRow::Instrumental {
                    start_secs: prev_end,
                    end_secs: line.time_secs,
                });
            }
        }
        if !line.text.trim().is_empty() {
            let next_time = Some(lyric_line_end_secs(lines, idx));
            out.push(DisplayLyricRow::Verse {
                line_idx: idx,
                line: line.clone(),
                next_time,
            });
        }
    }
    out
}

impl SonataDesktopView {
    pub(super) fn render_fullscreen_lyrics(
        &mut self,
        vp_w: f32,
        vp_h: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lyrics = self.snapshot.lyrics.clone();
        let translated_lyrics = self.snapshot.translated_lyrics.clone();
        let romanized_lyrics = self.snapshot.romanized_lyrics.clone();
        let pos = self.snapshot.lyrics_position_secs(self.live_position_now());
        let frame_scale = (vp_w / LYRICS_FRAME_W)
            .min(vp_h / LYRICS_FRAME_H)
            .clamp(1.0, 1.75);
        let verse_size = (25.0 * frame_scale).clamp(22.0, 36.0);
        let active_size = verse_size + ACTIVE_VERSE_GROWTH;
        let line_height = active_size * 1.30;
        let phrase_gap = verse_size * VERSE_GAP_FACTOR;
        let verse_pad_x = verse_size * VERSE_PAD_X_FACTOR;
        let verse_pad_y = verse_size * VERSE_PAD_Y_FACTOR;
        let sheet_pad_l = 8.0_f32;
        let sheet_pad_r = 28.0_f32;

        let scroll_bounds = self.fullscreen_lyrics_scroll.bounds();
        let view_w: f32 = scroll_bounds.size.width.into();
        let view_h: f32 = scroll_bounds.size.height.into();
        let view_top: f32 = scroll_bounds.origin.y.into();
        let effective_h = if view_h > 40.0 { view_h } else { vp_h };
        let effective_w = if view_w > 80.0 {
            view_w
        } else {
            (vp_w * 0.5 - 64.0).max(240.0)
        };
        let wrap_width = (effective_w - sheet_pad_l - sheet_pad_r - verse_pad_x * 2.0).max(120.0);

        let pad_top = (effective_h * LYRICS_PIN).max(96.0);
        let pad_bottom = (effective_h * (1.0 - LYRICS_PIN)).max(180.0);

        let user_scrolling = self
            .last_wheel_lyrics
            .is_some_and(|t| t.elapsed() < Duration::from_secs(WHEEL_SETTLE_SECS));

        let scroll_content = match lyrics {
            LyricsState::Synced(lines) => {
                if !self.snapshot.paused && !self.snapshot.loading_track && !lines.is_empty() {
                    self.ensure_karaoke_pace(cx);
                }

                let rewrapped =
                    self.fullscreen_lyrics_wrap
                        .sync_layout(&lines, wrap_width, active_size);
                if rewrapped {
                    window.request_animation_frame();
                }

                let (active_line_idx, active_bg_indices) = resolve_active_lyrics(&lines, pos);
                let rows = build_display_rows(&lines);

                // Find active display row (either active verse or active instrumental break)
                let active_display_row: Option<usize> = rows.iter().position(|r| match r {
                    DisplayLyricRow::Verse { line_idx, .. } => active_line_idx == Some(*line_idx),
                    DisplayLyricRow::Instrumental {
                        start_secs,
                        end_secs,
                    } => active_line_idx.is_none() && pos >= *start_secs && pos < *end_secs,
                });

                // Track arriving / departing active verse for smooth 220ms size & brightness transitions.
                if self.fullscreen_active_row_prev != active_display_row {
                    if self.fullscreen_active_row_prev.is_some() {
                        self.fullscreen_departing_row = self.fullscreen_active_row_prev;
                        self.fullscreen_row_departed = Instant::now();
                    }
                    if active_display_row.is_some() {
                        self.fullscreen_row_arrived = Instant::now();
                    }
                    self.fullscreen_active_row_prev = active_display_row;
                }
                if self.fullscreen_departing_row.is_some()
                    && self.fullscreen_row_departed.elapsed().as_secs_f32() >= VERSE_TRANSITION_SECS
                {
                    self.fullscreen_departing_row = None;
                }

                // Physical spring scroll toward PIN = 0.30 + cascading per-row lag stagger.
                let dt = self
                    .fullscreen_lyrics_last_step
                    .elapsed()
                    .as_secs_f32()
                    .clamp(0.001, 0.05);
                self.fullscreen_lyrics_last_step = Instant::now();

                let target_row = active_display_row.unwrap_or(0);
                let mut active_scroll_off: f32 = self.fullscreen_lyrics_scroll.offset().y.into();
                let mut scroll_animating = false;

                if user_scrolling {
                    self.fullscreen_scroll_spring = SpringState::at(active_scroll_off);
                    self.fullscreen_row_drifts.clear();
                } else if view_h > 40.0 {
                    // Direct children of fs-lyr-scroll: child 0 is top spacer, rows start at index 1.
                    let child_ix = target_row + 1;
                    if let Some(item_bounds) =
                        self.fullscreen_lyrics_scroll.bounds_for_item(child_ix)
                    {
                        let item_top: f32 = item_bounds.origin.y.into();
                        let item_h: f32 = item_bounds.size.height.into();
                        let max_off: f32 = self.fullscreen_lyrics_scroll.max_offset().y.into();
                        let goal = anchored_lyrics_offset(
                            view_top,
                            item_top,
                            view_h,
                            max_off,
                            LYRICS_PIN,
                            item_h * 0.16,
                        );
                        if (self.fullscreen_scroll_spring.position - active_scroll_off).abs() > 4.0
                            && self.fullscreen_scroll_spring.velocity.abs() < 1.0
                        {
                            self.fullscreen_scroll_spring = SpringState::at(active_scroll_off);
                        }
                        let diff = goal - self.fullscreen_scroll_spring.position;
                        if diff.abs() > view_h * 1.6 || self.snapshot.config.reduce_motion {
                            self.fullscreen_scroll_spring = SpringState::at(goal);
                            self.fullscreen_row_drifts.clear();
                        } else {
                            let next_spring =
                                SPRING_LYRICS_SCROLL.step(self.fullscreen_scroll_spring, goal, dt);
                            if SPRING_LYRICS_SCROLL.is_settled(next_spring, goal, 0.35) {
                                self.fullscreen_scroll_spring = SpringState::at(goal);
                            } else {
                                self.fullscreen_scroll_spring = next_spring;
                                scroll_animating = true;
                            }
                        }
                        let next_y = self
                            .fullscreen_scroll_spring
                            .position
                            .clamp(-max_off.max(0.0), 0.0);
                        active_scroll_off = next_y;
                        self.fullscreen_lyrics_scroll_y = next_y;
                        self.fullscreen_lyrics_scroll
                            .set_offset(point(px(0.0), px(next_y)));
                    } else {
                        scroll_animating = true;
                    }
                }

                // Cascading per-row spring stagger capped strictly below half the phrase gap so
                // parenthetical background lines never visually collide with neighboring verses.
                let scroll_step = active_scroll_off - self.fullscreen_last_seen_scroll;
                self.fullscreen_last_seen_scroll = active_scroll_off;
                let max_drift = (phrase_gap * 0.40).clamp(4.0, 12.0);
                let downward = scroll_step < 0.0;
                let mut any_drifting = false;

                let mut col = div()
                    .id("fs-lyr-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.fullscreen_lyrics_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(phrase_gap))
                    .pl(px(sheet_pad_l))
                    .pr(px(sheet_pad_r))
                    .child(div().flex_shrink_0().h(px(pad_top)));

                let edge_fade_px = (verse_size * 2.2).clamp(60.0, 132.0);
                let pin_y = effective_h * LYRICS_PIN;
                let arrive_t = ease_out_expo(
                    (self.fullscreen_row_arrived.elapsed().as_secs_f32() / VERSE_TRANSITION_SECS)
                        .clamp(0.0, 1.0),
                );
                let depart_t = ease_out_expo(
                    (self.fullscreen_row_departed.elapsed().as_secs_f32() / VERSE_TRANSITION_SECS)
                        .clamp(0.0, 1.0),
                );
                if arrive_t < 1.0 || (self.fullscreen_departing_row.is_some() && depart_t < 1.0) {
                    scroll_animating = true;
                }

                for (row_i, row) in rows.iter().enumerate() {
                    let child_ix = row_i + 1;
                    let hovered = self.fullscreen_lyrics_hovered_row == Some(row_i);

                    // Compute smooth transparent edge dissolve, distance haze, and offscreen culling from real viewport Y.
                    let mut culled_height: Option<f32> = None;
                    let (edge_alpha, haze, along) = if view_h > 40.0 {
                        if let Some(ib) = self.fullscreen_lyrics_scroll.bounds_for_item(child_ix) {
                            let rel_top = f32::from(ib.origin.y) + active_scroll_off - view_top;
                            let ih = f32::from(ib.size.height);
                            if !hovered
                                && active_display_row != Some(row_i)
                                && self.fullscreen_departing_row != Some(row_i)
                                && ih > 1.0
                                && (rel_top + ih < -140.0 || rel_top > view_h + 140.0)
                            {
                                culled_height = Some(ih);
                            }
                            let rel_mid = rel_top + ih * 0.5;
                            let top_fade = (rel_mid / edge_fade_px).clamp(0.0, 1.0);
                            let bot_fade = ((view_h - rel_mid) / edge_fade_px).clamp(0.0, 1.0);
                            let edge = (top_fade * bot_fade).clamp(0.0, 1.0);
                            let travel = (rel_mid - pin_y) / (view_h * 0.62).max(1.0);
                            let h_val = travel.abs().clamp(0.0, 1.0).powf(0.45);
                            let along_norm = (rel_top / view_h).clamp(0.0, 1.0);
                            let wave_along = if downward {
                                along_norm
                            } else {
                                1.0 - along_norm
                            };
                            (edge, h_val, wave_along)
                        } else {
                            let dist = active_display_row
                                .map(|a| row_i.abs_diff(a))
                                .unwrap_or(row_i);
                            (1.0, (dist as f32 / 5.0).clamp(0.0, 1.0).powf(0.45), 0.5)
                        }
                    } else {
                        let dist = active_display_row
                            .map(|a| row_i.abs_diff(a))
                            .unwrap_or(row_i);
                        (1.0, (dist as f32 / 5.0).clamp(0.0, 1.0).powf(0.45), 0.5)
                    };

                    if let Some(ih) = culled_height {
                        self.fullscreen_row_drifts.remove(&row_i);
                        col = col.child(div().flex_shrink_0().w_full().h(px(ih)));
                        continue;
                    }

                    // Step per-row stagger drift (wrapped so scroll handle keeps measuring rest bounds).
                    let row_drift_y = if !user_scrolling && !self.snapshot.config.reduce_motion {
                        let kick = if scroll_step.abs() > 0.15 {
                            (-scroll_step * 0.42 * along).clamp(-max_drift, max_drift)
                        } else {
                            0.0
                        };
                        let entry = self
                            .fullscreen_row_drifts
                            .entry(row_i)
                            .or_insert_with(|| SpringState::at(0.0));
                        entry.position = (entry.position + kick).clamp(-max_drift, max_drift);
                        let spring = lag_row_spring(along);
                        *entry = spring.step(*entry, 0.0, dt);
                        if spring.is_settled(*entry, 0.0, 0.15) {
                            self.fullscreen_row_drifts.remove(&row_i);
                            0.0
                        } else {
                            any_drifting = true;
                            entry.position
                        }
                    } else {
                        0.0
                    };

                    let wrap_with_drift = |inner: AnyElement, drift: f32| -> AnyElement {
                        if drift.abs() < 0.1 {
                            inner
                        } else {
                            div()
                                .w_full()
                                .min_w_0()
                                .child(
                                    div()
                                        .relative()
                                        .top(px(drift))
                                        .w_full()
                                        .min_w_0()
                                        .child(inner),
                                )
                                .into_any_element()
                        }
                    };

                    match row {
                        DisplayLyricRow::Instrumental {
                            start_secs,
                            end_secs,
                        } => {
                            let progress = if end_secs > start_secs {
                                ((pos - *start_secs) / (*end_secs - *start_secs)).clamp(0.0, 1.0)
                                    as f32
                            } else {
                                0.0
                            };
                            let past = pos >= *end_secs;
                            let is_active_break = active_display_row == Some(row_i);
                            let seek = *start_secs;
                            let base_alpha = if is_active_break || hovered {
                                1.0
                            } else {
                                edge_alpha * (1.0 - 0.30 * haze)
                            };

                            let mut notes = div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(12.0))
                                .py(px(verse_size * 0.32));

                            for note_idx in 0..3 {
                                let note_prog = (progress * 3.0 - note_idx as f32).clamp(0.0, 1.0);
                                let note_alpha = if past {
                                    0.26 * base_alpha
                                } else {
                                    (0.32 + 0.62 * note_prog) * base_alpha
                                };
                                notes = notes.child(
                                    div()
                                        .text_size(px(verse_size * 0.92))
                                        .line_height(px(verse_size * 1.1))
                                        .text_color(gpui_kit::white().opacity(note_alpha))
                                        .child("♪"),
                                );
                            }

                            let el = div()
                                .id(SharedString::from(format!("fs-brk-{row_i}")))
                                .w_full()
                                .min_w_0()
                                .px(px(verse_pad_x))
                                .rounded(px(10.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.07)))
                                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                                    this.fullscreen_lyrics_hovered_row =
                                        if *over { Some(row_i) } else { None };
                                    cx.notify();
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.last_wheel_lyrics = None;
                                    this.backend.send(BackendCmd::SeekAbsolute(seek));
                                }))
                                .child(notes)
                                .into_any_element();
                            col = col.child(wrap_with_drift(el, row_drift_y));
                        }
                        DisplayLyricRow::Verse {
                            line_idx,
                            line,
                            next_time,
                        } => {
                            let is_active_bg = active_bg_indices.contains(line_idx);
                            let is_active = active_line_idx == Some(*line_idx) || is_active_bg;
                            let is_departing =
                                !is_active && self.fullscreen_departing_row == Some(row_i);
                            let is_past =
                                !is_active_bg && active_line_idx.is_some_and(|a| *line_idx < a);
                            let is_bg = is_parenthetical_bg(&line.text);
                            let is_counter = line.voice == LyricVoice::Counter;
                            let seek = line.time_secs;

                            let base_sz = if is_bg { verse_size * 0.82 } else { verse_size };
                            let act_sz = if is_bg {
                                active_size * 0.86
                            } else {
                                active_size
                            };
                            let size = if is_active_bg {
                                act_sz
                            } else if is_active {
                                base_sz + (act_sz - base_sz) * arrive_t
                            } else if is_departing {
                                act_sz - (act_sz - base_sz) * depart_t
                            } else {
                                base_sz
                            };
                            let lh = act_sz * 1.32;

                            let wrapped = self.fullscreen_lyrics_wrap.get_or_measure(
                                *line_idx,
                                line,
                                *next_time,
                                active_size,
                                wrap_width,
                                window,
                            );

                            let content: AnyElement = if is_active {
                                let dim_alpha = if is_active_bg {
                                    0.42
                                } else {
                                    0.36 + 0.06 * arrive_t
                                };
                                let lit_alpha = if is_active_bg {
                                    0.98
                                } else {
                                    0.72 + 0.26 * arrive_t
                                };
                                if let Some(plan) = wrapped {
                                    render_karaoke_rows(
                                        plan,
                                        line,
                                        pos,
                                        KaraokeRenderStyle {
                                            font_size: size,
                                            line_height: lh,
                                            base_color: gpui_kit::white().opacity(dim_alpha),
                                            lit_color: gpui_kit::white().opacity(lit_alpha),
                                            is_counter,
                                            is_bg,
                                        },
                                    )
                                } else {
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .text_size(px(size))
                                        .line_height(px(lh))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(gpui_kit::white().opacity(lit_alpha))
                                        .child(line.text.clone())
                                        .into_any_element()
                                }
                            } else {
                                let rest_shade = if hovered {
                                    0.86
                                } else if is_past {
                                    0.40 * (1.0 - 0.30 * haze)
                                } else {
                                    0.58 * (1.0 - 0.30 * haze)
                                };
                                let shade = if is_departing {
                                    0.96 + (rest_shade - 0.96) * depart_t
                                } else {
                                    rest_shade
                                };
                                let alpha = if hovered {
                                    shade * edge_alpha.max(0.35)
                                } else {
                                    let bg_mul = if is_bg { 0.82 } else { 1.0 };
                                    (shade * bg_mul * edge_alpha).clamp(0.0, 1.0)
                                };
                                let blur_r = if hovered || user_scrolling {
                                    0.0
                                } else {
                                    let b = (verse_size * 0.08 * haze).clamp(0.0, 2.35);
                                    if is_departing {
                                        b * depart_t
                                    } else {
                                        b
                                    }
                                };
                                if let Some(plan) = wrapped {
                                    render_fixed_rows(
                                        &plan.text,
                                        size,
                                        lh,
                                        gpui_kit::white().opacity(alpha),
                                        blur_r,
                                        is_counter,
                                        is_bg,
                                    )
                                } else {
                                    let fallback = [SharedString::from(line.text.clone())];
                                    render_fixed_rows(
                                        &fallback,
                                        size,
                                        lh,
                                        gpui_kit::white().opacity(alpha),
                                        blur_r,
                                        is_counter,
                                        is_bg,
                                    )
                                }
                            };

                            let (rom, tr) = secondary_lyric_slices(
                                *line_idx,
                                &line.text,
                                romanized_lyrics.as_deref(),
                                translated_lyrics.as_deref(),
                            );
                            let sec_alpha = if is_active || is_active_bg {
                                0.72 * arrive_t.max(0.5)
                            } else if hovered {
                                0.72 * edge_alpha.max(0.35)
                            } else {
                                (0.48 * (1.0 - 0.30 * haze) * edge_alpha).clamp(0.0, 1.0)
                            };
                            let sec_size = (verse_size * 0.56).clamp(13.0, 20.0);
                            let sec_lh = sec_size * 1.32;

                            let el = div()
                                .id(SharedString::from(format!("fs-lyr-{row_i}")))
                                .w_full()
                                .min_w_0()
                                .px(px(verse_pad_x))
                                .py(px(verse_pad_y))
                                .rounded(px(10.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui_kit::white().opacity(0.07)))
                                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                                    this.fullscreen_lyrics_hovered_row =
                                        if *over { Some(row_i) } else { None };
                                    cx.notify();
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.last_wheel_lyrics = None;
                                    this.backend.send(BackendCmd::SeekAbsolute(seek));
                                }))
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .when(!is_counter, |el| el.items_start())
                                .when(is_counter, |el| el.items_end())
                                .child(content)
                                .when_some(rom, |el, rom_text| {
                                    el.child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .text_size(px(sec_size))
                                            .line_height(px(sec_lh))
                                            .text_color(gpui_kit::white().opacity(sec_alpha))
                                            .when(is_counter, |t| t.text_right())
                                            .child(rom_text.to_string()),
                                    )
                                })
                                .when_some(tr, |el, tr_text| {
                                    el.child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .text_size(px(sec_size))
                                            .line_height(px(sec_lh))
                                            .text_color(gpui_kit::white().opacity(sec_alpha))
                                            .when(is_counter, |t| t.text_right())
                                            .child(tr_text.to_string()),
                                    )
                                })
                                .into_any_element();
                            col = col.child(wrap_with_drift(el, row_drift_y));
                        }
                    }
                }

                if scroll_animating || any_drifting {
                    window.request_animation_frame();
                }

                col.child(div().flex_shrink_0().h(px(pad_bottom)))
                    .into_any_element()
            }
            LyricsState::Plain(lines) => {
                let mut col = div()
                    .id("fs-lyr-plain-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.fullscreen_lyrics_scroll)
                    .px(px(24.0))
                    .py(px(pad_top * 0.5))
                    .flex()
                    .flex_col()
                    .gap(px(10.0));
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
                                .text_size(px(22.0))
                                .line_height(px(line_height))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(gpui_kit::white().opacity(0.84))
                                .child(line.clone()),
                        );
                    if let Some(rom_text) = rom {
                        entry = entry.child(
                            div()
                                .text_size(px(14.5))
                                .line_height(px(20.0))
                                .text_color(gpui_kit::white().opacity(0.58))
                                .child(rom_text.to_string()),
                        );
                    }
                    if let Some(tr_text) = tr {
                        entry = entry.child(
                            div()
                                .text_size(px(15.0))
                                .line_height(px(20.0))
                                .text_color(gpui_kit::white().opacity(0.66))
                                .child(tr_text.to_string()),
                        );
                    }
                    col = col.child(entry);
                }
                col.into_any_element()
            }
            LyricsState::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(24.0))
                .text_color(gpui_kit::white().opacity(0.45))
                .child("♪  ♪  ♪")
                .into_any_element(),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(17.0))
                .text_color(gpui_kit::white().opacity(0.40))
                .child(sonata_runtime::i18n::strings(self.settings.language).no_lyrics_found)
                .into_any_element(),
        };

        div()
            .flex_1()
            .h_full()
            .min_h_0()
            .relative()
            .on_scroll(cx.listener(|this, _, _, cx| {
                this.last_wheel_lyrics = Some(Instant::now());
                this.stir_fullscreen(cx);
            }))
            .child(scroll_content)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchored_lyrics_offset_pins_active_row_without_drift() {
        let view_top = 0.0;
        let view_h = 900.0;
        let max_off = 2400.0;
        let item_top = 980.0; // Unscrolled GPUI child origin.y
        let optical_shift = 8.0;

        let goal1 = anchored_lyrics_offset(
            view_top,
            item_top,
            view_h,
            max_off,
            LYRICS_PIN,
            optical_shift,
        );
        // Repeated frames with the same unscrolled bounds must yield the exact same target offset.
        let goal2 = anchored_lyrics_offset(
            view_top,
            item_top,
            view_h,
            max_off,
            LYRICS_PIN,
            optical_shift,
        );
        assert_eq!(goal1, goal2);
        let screen_rel_top = item_top + goal1 - view_top;
        assert!(
            (screen_rel_top - (view_h * LYRICS_PIN - optical_shift)).abs() < 1e-3,
            "expected pinned screen Y {}, got {}",
            view_h * LYRICS_PIN - optical_shift,
            screen_rel_top
        );
    }

    #[test]
    fn anchored_lyrics_offset_clamps_to_scroll_bounds() {
        assert_eq!(
            anchored_lyrics_offset(0.0, 120.0, 900.0, 1000.0, LYRICS_PIN, 0.0),
            0.0
        );
        assert_eq!(
            anchored_lyrics_offset(0.0, 4000.0, 900.0, 1000.0, LYRICS_PIN, 0.0),
            -1000.0
        );
    }
}
