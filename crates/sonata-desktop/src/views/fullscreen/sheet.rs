//! Synced lyrics sheet, smooth 50% vertical auto-scroll, and depth-of-field rendering.
//!
//! 1. **Sheet Construction**: `build_sheet_entries` interleaves lyric verses with `♪ ♪ ♪` instrumental break indicators for gaps >= 5s.
//! 2. **50% Auto-Scroll & 1.2s Settle**: `step_smooth_lyrics_scroll` eases the active verse to exact viewport center (`AUTO_SCROLL_ANCHOR_RATIO = 0.50`), while `on_scroll` arms a 1220ms `manual_scroll_gen` timer so paused/idle views re-blur and recenter automatically.
//! 3. **Pre-Warmed Blur & Karaoke**: pre-warms all 3 fixed blur sigma buckets across `active_idx - 2..=active_idx + 5`, renders active verses with sub-pixel karaoke fill, and right-aligns `LyricVoice::Counter` lines.

use super::super::{ui_icon, FluentDivExt, SonataDesktopView};
use super::{AUTO_SCROLL_ANCHOR_RATIO, AUTO_SCROLL_SNAP_PX, AUTO_SCROLL_SPEED};
use crate::backend::{BackendCmd, DesktopSnapshot};
use crate::theme_bridge::rgb_hsla;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, img, point, px, relative, AnyElement, Context, FontWeight, Hsla, InteractiveElement,
    IntoElement, ObjectFit, ParentElement, ScrollEvent, SharedString, StatefulInteractiveElement,
    Styled, StyledImage, Window,
};
use sonata_core::lyrics::{LyricLine, LyricVoice, LyricsState};
use std::time::{Duration, Instant};

const INSTRUMENTAL_BREAK_SECS: f64 = 5.0;
const DEFAULT_LINE_SWEEP_SECS: f64 = 4.2;

/// Visual item inside the fullscreen lyrics sheet: either a lyric verse or a `♪ ♪ ♪` break.
pub(super) enum SheetEntry<'a> {
    Verse {
        line_index: usize,
        line: &'a LyricLine,
        end_secs: f64,
    },
    Instrumental {
        after_line: Option<usize>,
        start_secs: f64,
        end_secs: f64,
    },
}

pub(super) fn build_sheet_entries<'a>(
    lines: &'a [LyricLine],
    track_duration: f64,
) -> Vec<SheetEntry<'a>> {
    let mut entries = Vec::with_capacity(lines.len() + 4);

    if let Some(first) = lines.first() {
        if first.time_secs >= INSTRUMENTAL_BREAK_SECS {
            entries.push(SheetEntry::Instrumental {
                after_line: None,
                start_secs: 0.0,
                end_secs: first.time_secs,
            });
        }
    }

    for (idx, line) in lines.iter().enumerate() {
        let next_start = lines.get(idx + 1).map(|next| next.time_secs);

        // Empty synced line acts as an explicit instrumental break marker.
        if line.text.trim().is_empty() {
            if let Some(next_t) = next_start {
                if next_t > line.time_secs + 0.5 {
                    entries.push(SheetEntry::Instrumental {
                        after_line: Some(idx),
                        start_secs: line.time_secs,
                        end_secs: next_t,
                    });
                }
            }
            continue;
        }

        let word_end = line
            .words
            .last()
            .and_then(|w| w.end_secs.or(Some(w.start_secs + 0.7)));
        let verse_end = estimate_verse_end_secs(line, next_start, track_duration);

        entries.push(SheetEntry::Verse {
            line_index: idx,
            line,
            end_secs: verse_end,
        });

        // Insert 3-note instrumental break indicator whenever there is a >= 5s gap.
        if let Some(next_t) = next_start {
            let break_start = word_end
                .unwrap_or_else(|| {
                    (line.time_secs + estimate_sung_duration(&line.text)).min(next_t)
                })
                .max(line.time_secs + 0.8);
            if next_t - line.time_secs >= INSTRUMENTAL_BREAK_SECS && next_t - break_start >= 2.0 {
                entries.push(SheetEntry::Instrumental {
                    after_line: Some(idx),
                    start_secs: break_start,
                    end_secs: next_t,
                });
            }
        }
    }

    entries
}

pub(in crate::views) fn estimate_verse_end_secs(
    line: &LyricLine,
    next_start: Option<f64>,
    track_duration: f64,
) -> f64 {
    let word_end = line
        .words
        .last()
        .and_then(|w| w.end_secs.or(Some(w.start_secs + 0.7)));
    match (word_end, next_start) {
        (Some(we), Some(ns)) => we.min(ns).max(line.time_secs + 0.4),
        (Some(we), None) => we.max(line.time_secs + 0.4),
        (None, Some(ns)) => {
            let est = line.time_secs + estimate_sung_duration(&line.text);
            let gap_cap = line.time_secs + ((ns - line.time_secs) * 0.82).max(0.45);
            est.min(gap_cap)
        }
        (None, None) => {
            let est = line.time_secs + estimate_sung_duration(&line.text);
            if track_duration > line.time_secs + 0.5 {
                track_duration.min(est.max(line.time_secs + DEFAULT_LINE_SWEEP_SECS))
            } else {
                est.max(line.time_secs + DEFAULT_LINE_SWEEP_SECS)
            }
        }
    }
}

fn estimate_sung_duration(text: &str) -> f64 {
    let chars = text.chars().count().max(3) as f64;
    (chars * 0.062).clamp(1.1, 3.4)
}

fn active_sheet_child_index(
    entries: &[SheetEntry<'_>],
    active_line: Option<usize>,
    position_secs: f64,
) -> Option<usize> {
    // Child 0 in the scroll container is the top spacer, so entry `i` is child `i + 1`.
    if let Some(active) = active_line {
        // Prefer an active instrumental break if we are currently inside one.
        for (i, entry) in entries.iter().enumerate() {
            if let SheetEntry::Instrumental {
                start_secs,
                end_secs,
                ..
            } = entry
            {
                if position_secs >= *start_secs && position_secs < *end_secs {
                    return Some(i + 1);
                }
            }
        }
        for (i, entry) in entries.iter().enumerate() {
            if let SheetEntry::Verse { line_index, .. } = entry {
                if *line_index == active {
                    return Some(i + 1);
                }
            }
        }
    }
    (!entries.is_empty()).then_some(1)
}

pub(super) fn blur_sigma_for_distance(distance: usize, base_size: f32) -> f32 {
    match distance {
        1 => (base_size * 0.058).clamp(1.2, 2.2),
        2 => (base_size * 0.105).clamp(2.4, 3.6),
        _ => (base_size * 0.150).clamp(3.6, 5.2),
    }
}

pub(super) fn step_smooth_lyrics_scroll(
    view: &mut SonataDesktopView,
    viewport_h: f32,
    reduce_motion: bool,
) -> bool {
    let LyricsState::Synced(lines) = &view.snapshot.lyrics else {
        return false;
    };
    if lines.is_empty() || view.fullscreen.is_manually_scrolled() {
        return false;
    }

    let entries = build_sheet_entries(lines, view.snapshot.duration_secs);
    let active = view.snapshot.active_lyric_index();
    let Some(child_idx) = active_sheet_child_index(&entries, active, view.snapshot.position_secs)
    else {
        return false;
    };

    let scroll = &view.fullscreen.scroll;
    let view_bounds = scroll.bounds();
    let Some(item_bounds) = scroll.bounds_for_item(child_idx) else {
        return false;
    };

    let vh: f32 = view_bounds.size.height.into();
    if vh < 40.0 {
        return false;
    }

    let anchor_y = f32::from(view_bounds.top()) + vh * AUTO_SCROLL_ANCHOR_RATIO;
    let item_center_y = f32::from(item_bounds.top()) + f32::from(item_bounds.size.height) * 0.5;
    let raw_target_y = anchor_y - item_center_y;

    let max_off: f32 = scroll.max_offset().y.into();
    let min_y = -(max_off.abs()).max(viewport_h * 0.50);
    let target_y = raw_target_y.clamp(min_y, 0.0);

    let current = scroll.offset();
    let cur_y: f32 = current.y.into();
    let diff = target_y - cur_y;

    let now = Instant::now();
    let dt = now
        .duration_since(view.fullscreen.last_scroll_step)
        .as_secs_f32()
        .clamp(0.001, 0.1);
    view.fullscreen.last_scroll_step = now;

    if view.fullscreen.snap_next_scroll || reduce_motion || diff.abs() <= AUTO_SCROLL_SNAP_PX {
        view.fullscreen.snap_next_scroll = false;
        if diff.abs() > 0.05 {
            scroll.set_offset(point(current.x, px(target_y)));
        }
        return false;
    }

    let factor = 1.0 - (-dt * AUTO_SCROLL_SPEED).exp();
    let next_y = cur_y + diff * factor;
    scroll.set_offset(point(current.x, px(next_y)));
    true
}

#[allow(clippy::too_many_arguments)]
fn prewarm_blur_buckets(
    view: &mut SonataDesktopView,
    entries: &[SheetEntry<'_>],
    active_idx: usize,
    content_w: f32,
    base_size: f32,
    active_size: f32,
    line_height: f32,
    scale_factor: f32,
    window: &mut Window,
) {
    let min_idx = active_idx.saturating_sub(2);
    let max_idx = active_idx + 5;
    let sigmas = [
        blur_sigma_for_distance(1, base_size),
        blur_sigma_for_distance(2, base_size),
        blur_sigma_for_distance(3, base_size),
    ];

    for entry in entries {
        let SheetEntry::Verse {
            line_index, line, ..
        } = entry
        else {
            continue;
        };
        let idx = *line_index;
        if idx < min_idx || idx > max_idx {
            continue;
        }
        let is_background = is_background_lyric(&line.text);
        let align_right = line.voice == LyricVoice::Counter;
        let line_base = if is_background {
            base_size * 0.84
        } else {
            base_size
        };
        let line_lh = if is_background {
            active_size * 0.84 * 1.18
        } else {
            line_height
        };
        for sigma in sigmas {
            let _ = view.fullscreen.blur_cache.get_or_render(
                &line.text,
                content_w,
                line_base,
                line_lh,
                sigma,
                align_right,
                is_background,
                scale_factor,
                window,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_fullscreen_lyrics_column(
    view: &mut SonataDesktopView,
    snap: &DesktopSnapshot,
    lyrics_width: f32,
    viewport_h: f32,
    base_size: f32,
    active_size: f32,
    line_height: f32,
    verse_gap: f32,
    accent_hsla: Hsla,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let LyricsState::Synced(lines) = &snap.lyrics else {
        return render_unsynced_lyrics_state(&snap.lyrics, base_size);
    };
    if lines.is_empty() {
        return render_unsynced_lyrics_state(&LyricsState::NotFound, base_size);
    }

    let entries = build_sheet_entries(lines, snap.duration_secs);
    let active_idx = snap.active_lyric_index().unwrap_or(0);
    let pos = snap.position_secs;
    let hovered = view.fullscreen.hovered_line;
    let manual_browse = view.fullscreen.is_manually_scrolled();
    let reduce_motion = snap.config.reduce_motion;

    let top_spacer_h = (viewport_h * 0.50).max(180.0);
    let bottom_spacer_h = (viewport_h * 0.50).max(180.0);
    // 20px side padding inside each row ensures wide Gaussian blur tails never clip horizontally.
    let content_w = (lyrics_width - 40.0).max(200.0);

    if !reduce_motion {
        prewarm_blur_buckets(
            view,
            &entries,
            active_idx,
            content_w,
            base_size,
            active_size,
            line_height,
            scale_factor,
            window,
        );
    }

    let scroll_handle = view.fullscreen.scroll.clone();
    let scroll_bounds = scroll_handle.bounds();
    let scroll_offset_y: f32 = scroll_handle.offset().y.into();
    let scroll_top: f32 = scroll_bounds.top().into();
    let scroll_vh: f32 = scroll_bounds.size.height.into();
    let edge_fade_for_child = |child_idx: usize| -> f32 {
        if scroll_vh < 80.0 {
            return 1.0;
        }
        let Some(cb) = scroll_handle.bounds_for_item(child_idx) else {
            return 1.0;
        };
        let center_y =
            f32::from(cb.top()) + f32::from(cb.size.height) * 0.5 + scroll_offset_y - scroll_top;
        let rel = (center_y / scroll_vh).clamp(0.0, 1.0);
        if rel < 0.16 {
            0.30 + 0.70 * (rel / 0.16)
        } else if rel > 0.84 {
            0.30 + 0.70 * ((1.0 - rel) / 0.16)
        } else {
            1.0
        }
    };

    let mut children: Vec<AnyElement> = Vec::with_capacity(entries.len() + 2);
    children.push(
        div()
            .w_full()
            .h(px(top_spacer_h))
            .flex_none()
            .into_any_element(),
    );

    for (entry_idx, entry) in entries.iter().enumerate() {
        let child_idx = entry_idx + 1;
        let edge_fade = edge_fade_for_child(child_idx);
        match entry {
            SheetEntry::Instrumental {
                after_line,
                start_secs,
                end_secs,
            } => {
                let is_active_break = pos >= *start_secs && pos < *end_secs;
                let is_past = pos >= *end_secs;
                let progress = if *end_secs > *start_secs {
                    ((pos - *start_secs) / (*end_secs - *start_secs)).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                // Keep instrumental notes sharp when right next to active line or actively playing.
                let near_active = is_active_break
                    || after_line.is_some_and(|idx| idx == active_idx || idx + 1 == active_idx);
                let row_opacity = (if is_active_break {
                    0.92
                } else if near_active {
                    0.46
                } else {
                    0.22
                }) * edge_fade;
                let seek_time = *start_secs;
                children.push(
                    div()
                        .id(SharedString::from(format!("fs-break-{entry_idx}")))
                        .w_full()
                        .px_5()
                        .py(px(verse_gap * 0.35))
                        .flex_none()
                        .cursor_pointer()
                        .opacity(row_opacity)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.fullscreen.manual_scroll_until = None;
                            this.backend.send(BackendCmd::SeekAbsolute(seek_time));
                            cx.notify();
                        }))
                        .child(render_instrumental_notes(
                            progress,
                            is_active_break,
                            is_past,
                            base_size,
                            accent_hsla,
                        ))
                        .into_any_element(),
                );
            }
            SheetEntry::Verse {
                line_index,
                line,
                end_secs,
            } => {
                let idx = *line_index;
                let is_active = idx == active_idx && pos >= line.time_secs - 0.25;
                let is_past = idx < active_idx;
                let is_hovered = hovered == Some(idx);
                let distance = idx.abs_diff(active_idx);
                let align_right = line.voice == LyricVoice::Counter;
                let is_background = is_background_lyric(&line.text);
                let seek_time = line.time_secs;

                let (blur_sigma, alpha) = if is_active {
                    (0.0f32, 1.0f32)
                } else if is_hovered {
                    (0.0, 0.90)
                } else if manual_browse || reduce_motion {
                    let a = match distance {
                        1 => 0.56,
                        2 => 0.42,
                        _ => 0.30,
                    };
                    (0.0, a)
                } else {
                    let sigma = blur_sigma_for_distance(distance, base_size);
                    let a = if is_past {
                        match distance {
                            1 => 0.34,
                            2 => 0.24,
                            _ => 0.16,
                        }
                    } else {
                        match distance {
                            1 => 0.46,
                            2 => 0.38,
                            3 => 0.28,
                            _ => 0.18,
                        }
                    };
                    (sigma, (a * edge_fade).clamp(0.08, 1.0))
                };

                let line_base_size = if is_background {
                    base_size * 0.84
                } else {
                    base_size
                };
                let line_active_size = if is_background {
                    active_size * 0.84
                } else {
                    active_size
                };
                let line_lh = if is_background {
                    line_active_size * 1.18
                } else {
                    line_height
                };
                let line_alpha = if is_background { alpha * 0.78 } else { alpha };

                let verse_body = if is_active {
                    render_active_karaoke_verse(
                        line,
                        pos,
                        *end_secs,
                        line_active_size,
                        line_lh,
                        align_right,
                        is_background,
                    )
                } else if blur_sigma >= 0.5 {
                    if let Some(blurred) = view.fullscreen.blur_cache.get_or_render(
                        &line.text,
                        content_w,
                        line_base_size,
                        line_lh,
                        blur_sigma,
                        align_right,
                        is_background,
                        scale_factor,
                        window,
                    ) {
                        let visible_h =
                            (blurred.logical_height - blurred.pad_px * 2.0).max(line_lh);
                        div()
                            .relative()
                            .w(px(content_w))
                            .h(px(visible_h))
                            .opacity(line_alpha)
                            .child(
                                img(blurred.image)
                                    .absolute()
                                    .top(px(-blurred.pad_px))
                                    .left(px(-blurred.pad_px))
                                    .w(px(blurred.logical_width))
                                    .h(px(blurred.logical_height))
                                    .object_fit(ObjectFit::Fill),
                            )
                            .into_any_element()
                    } else {
                        render_plain_verse_text(
                            &line.text,
                            line_base_size,
                            line_lh,
                            line_alpha,
                            align_right,
                            is_background,
                        )
                    }
                } else {
                    render_plain_verse_text(
                        &line.text,
                        line_base_size,
                        line_lh,
                        line_alpha,
                        align_right,
                        is_background,
                    )
                };

                let row = div()
                    .id(SharedString::from(format!("fs-verse-{idx}")))
                    .w_full()
                    .px_5()
                    .py(px(verse_gap * 0.46))
                    .rounded_xl()
                    .flex_none()
                    .cursor_pointer()
                    .when(is_hovered && !is_active, |el| {
                        el.bg(rgb_hsla((255, 255, 255), 0.06))
                    })
                    .on_hover(cx.listener(move |this, is_over: &bool, _, cx| {
                        if *is_over {
                            if this.fullscreen.hovered_line != Some(idx) {
                                this.fullscreen.hovered_line = Some(idx);
                                cx.notify();
                            }
                        } else if this.fullscreen.hovered_line == Some(idx) {
                            this.fullscreen.hovered_line = None;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.fullscreen.manual_scroll_until = None;
                        this.fullscreen.snap_next_scroll = false;
                        this.backend.send(BackendCmd::SeekAbsolute(seek_time));
                        cx.notify();
                    }))
                    .child(verse_body)
                    .into_any_element();

                children.push(row);
            }
        }
    }

    children.push(
        div()
            .w_full()
            .h(px(bottom_spacer_h))
            .flex_none()
            .into_any_element(),
    );

    div()
        .id("fullscreen-lyrics-scroll")
        .relative()
        .w_full()
        .max_w(px(lyrics_width))
        .h_full()
        .overflow_y_scroll()
        .track_scroll(&view.fullscreen.scroll)
        .on_scroll(cx.listener(|this, _ev: &ScrollEvent, _, cx| {
            let gen = this.fullscreen.note_manual_scroll();
            this.note_fullscreen_activity(cx);
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1220))
                    .await;
                let _ = this.update(cx, |view, cx| {
                    if view.fullscreen.manual_scroll_gen == gen {
                        view.fullscreen.manual_scroll_until = None;
                        cx.notify();
                    }
                });
            })
            .detach();
        }))
        .flex()
        .flex_col()
        .children(children)
        .into_any_element()
}

pub(super) fn is_background_lyric(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2
}

pub(in crate::views) struct KaraokePalette {
    pub font_size: f32,
    pub line_height: f32,
    pub align_right: bool,
    pub is_background: bool,
    pub dim_color: Hsla,
    pub lit_color: Hsla,
    pub active_glow_color: Hsla,
    pub sung_glow_color: Hsla,
}

/// Renders the active lyric verse with a sub-pixel progressive karaoke reveal
/// (`"The empt"` bright glowing white + `"iness machine"` translucent gray).
fn render_active_karaoke_verse(
    line: &LyricLine,
    position_secs: f64,
    line_end_secs: f64,
    font_size: f32,
    line_height: f32,
    align_right: bool,
    is_background: bool,
) -> AnyElement {
    render_active_karaoke_verse_styled(
        line,
        position_secs,
        line_end_secs,
        KaraokePalette {
            font_size,
            line_height,
            align_right,
            is_background,
            dim_color: rgb_hsla((255, 255, 255), if is_background { 0.22 } else { 0.28 }),
            lit_color: rgb_hsla((255, 255, 255), if is_background { 0.92 } else { 1.0 }),
            active_glow_color: rgb_hsla((255, 255, 255), if is_background { 0.34 } else { 0.48 }),
            sung_glow_color: rgb_hsla((255, 255, 255), if is_background { 0.16 } else { 0.26 }),
        },
    )
}

pub(in crate::views) fn render_active_karaoke_verse_styled(
    line: &LyricLine,
    position_secs: f64,
    line_end_secs: f64,
    style: KaraokePalette,
) -> AnyElement {
    const KARAOKE_LEAD_SECS: f64 = 0.22;

    let weight = if style.is_background {
        FontWeight::SEMIBOLD
    } else {
        FontWeight::BOLD
    };
    let sweep_pos = position_secs + KARAOKE_LEAD_SECS;

    // Build `(token_with_space, progress 0.0..=1.0)` pairs either from Enhanced LRC word tags
    // or proportional character-weighted line sweep so standard LRCLIB lines still sweep smoothly.
    let tokens: Vec<(String, f32)> = if !line.words.is_empty() {
        let count = line.words.len();
        line.words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let start = w.start_secs;
                let raw_end = w
                    .end_secs
                    .or_else(|| line.words.get(i + 1).map(|next| next.start_secs))
                    .unwrap_or(line_end_secs)
                    .max(start + 0.14);
                let max_word_span = (w.text.chars().count().max(2) as f64 * 0.11).clamp(0.28, 1.35);
                let effective_end = if w.end_secs.is_some() {
                    raw_end
                } else {
                    raw_end.min(start + max_word_span)
                };
                let linear = ((sweep_pos - start) / (effective_end - start).max(0.12))
                    .clamp(0.0, 1.0) as f32;
                let share = linear.powf(0.68);
                let mut label = w.text.clone();
                if i + 1 < count {
                    label.push(' ');
                }
                (label, share)
            })
            .collect()
    } else {
        let words: Vec<&str> = line.text.split_whitespace().collect();
        if words.is_empty() {
            Vec::new()
        } else {
            let total_chars: f64 = words.iter().map(|w| (w.chars().count() + 1) as f64).sum();
            let span = (line_end_secs - line.time_secs).max(0.5);
            let line_progress = ((sweep_pos - line.time_secs) / span).clamp(0.0, 1.0);
            let mut cursor = 0.0f64;
            let count = words.len();
            words
                .into_iter()
                .enumerate()
                .map(|(i, w)| {
                    let weight = (w.chars().count() + 1) as f64 / total_chars.max(1.0);
                    let w_start = cursor;
                    let w_end = (cursor + weight).min(1.0);
                    cursor = w_end;
                    let linear = if w_end > w_start {
                        ((line_progress - w_start) / (w_end - w_start)).clamp(0.0, 1.0) as f32
                    } else {
                        1.0
                    };
                    let share = linear.powf(0.72);
                    let mut label = w.to_string();
                    if i + 1 < count {
                        label.push(' ');
                    }
                    (label, share)
                })
                .collect()
        }
    };

    let lit_color = style.lit_color;
    let active_glow = style.active_glow_color;
    let sung_glow = style.sung_glow_color;

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .when(style.align_right, |el| el.justify_end())
        .text_size(px(style.font_size))
        .line_height(px(style.line_height))
        .font_weight(weight)
        .text_color(style.dim_color)
        .children(tokens.into_iter().map(move |(token, share)| {
            let shared = SharedString::from(token);
            let is_sweeping = share > 0.01 && share < 0.99;
            let glow_color = if is_sweeping { active_glow } else { sung_glow };
            div()
                .relative()
                .flex_none()
                .whitespace_nowrap()
                .when(is_sweeping, |el| el.top(px(-1.0)))
                .child(shared.clone())
                .when(share > 0.005, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top(px(-2.0))
                            .bottom(px(-2.0))
                            .left_0()
                            .w(relative(share))
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .top(px(1.4))
                                    .left(px(0.5))
                                    .whitespace_nowrap()
                                    .text_color(glow_color)
                                    .child(shared.clone()),
                            )
                            .child(
                                div()
                                    .relative()
                                    .top(px(2.0))
                                    .whitespace_nowrap()
                                    .text_color(lit_color)
                                    .child(shared),
                            ),
                    )
                })
        }))
        .into_any_element()
}

fn render_plain_verse_text(
    text: &str,
    font_size: f32,
    line_height: f32,
    alpha: f32,
    align_right: bool,
    is_background: bool,
) -> AnyElement {
    let weight = if is_background {
        FontWeight::MEDIUM
    } else {
        FontWeight::SEMIBOLD
    };
    div()
        .w_full()
        .text_size(px(font_size))
        .line_height(px(line_height))
        .font_weight(weight)
        .text_color(rgb_hsla((255, 255, 255), alpha))
        .whitespace_normal()
        .when(align_right, |el| el.text_right())
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// Renders the 3-note instrumental break indicator (`♪ ♪ ♪`) lighting left-to-right.
fn render_instrumental_notes(
    progress: f32,
    active: bool,
    past: bool,
    verse_size: f32,
    _accent: Hsla,
) -> AnyElement {
    let note_size = (verse_size * 0.84).clamp(18.0, 32.0);
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .children((0..3).map(|i| {
            let note_fill = if past {
                0.28
            } else if active {
                let p = (progress * 3.0 - i as f32).clamp(0.0, 1.0);
                0.32 + 0.64 * p
            } else {
                0.36
            };
            ui_icon(
                IconName::Music2,
                note_size,
                rgb_hsla((255, 255, 255), note_fill),
            )
        }))
        .into_any_element()
}

fn render_unsynced_lyrics_state(state: &LyricsState, base_size: f32) -> AnyElement {
    match state {
        LyricsState::Plain(lines) if !lines.is_empty() => div()
            .id("fullscreen-plain-lyrics")
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .py(px(84.0))
            .flex()
            .flex_col()
            .gap(px(base_size * 0.45))
            .children(lines.iter().map(|line| {
                div()
                    .w_full()
                    .text_size(px((base_size * 0.85).clamp(18.0, 28.0)))
                    .line_height(px(base_size * 1.15))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb_hsla((255, 255, 255), 0.72))
                    .child(SharedString::from(line.clone()))
            }))
            .into_any_element(),
        _ => {
            let message = match state {
                LyricsState::Loading => "Fetching synced lyrics...",
                LyricsState::NotFound => "No lyrics found for this track.",
                LyricsState::Idle => "Start playback to view synced lyrics.",
                _ => "No lyrics available for this track.",
            };
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .text_color(rgb_hsla((255, 255, 255), 0.42))
                .child(message)
                .into_any_element()
        }
    }
}
