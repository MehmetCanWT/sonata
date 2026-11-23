//! Subpixel word-wrapped karaoke renderer and physical spring scroll engine for desktop lyrics.
//!
//! 1. **Physical Springs (`SpringConfig`, `SpringState`)**: Critically damped scroll and staggered per-row springs for fluid verse transitions.
//! 2. **Measured Wrapping (`LyricsWrapCache`, `WrappedVerse`)**: Shapes each verse once via GPUI `shape_line` to break rows strictly inside the available width with exact kerning and zero right-edge overflow.
//! 3. **Subpixel Karaoke Sweep (`render_karaoke_rows`)**: Clips layered crisp and feathered highlight slices to subpixel pixel widths driven by word-synced or synthesized cadence timings.

use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, FontWeight, Hsla, IntoElement,
    ParentElement as _, SharedString, Styled as _, Window,
};
use sonata_core::lyrics::{LyricLine, LyricVoice, LyricWord};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::ops::Range;

/// Scroll spring matching tuned lyrics-follow physics (zero overshoot, fluid settle).
pub const SPRING_LYRICS_SCROLL: SpringConfig = SpringConfig::new(170.0, 23.0, 1.0);
/// Base per-row spring before viewport staggering lowers frequency for incoming rows.
pub const SPRING_LYRICS_ROW: SpringConfig = SpringConfig::new(210.0, 22.0, 1.0);

const LAG_STAGGER: f32 = 0.35;
const SWEEP_LEAST_SECS: f64 = 0.18;
const SWEEP_STRETCH: f64 = 1.4;
const SWEPT_SNAP: f32 = 0.98;
const REVEAL_FADE_FACTOR: f32 = 0.62;

/// Second-order damped harmonic spring parameters.
#[derive(Clone, Copy, Debug)]
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl SpringConfig {
    pub const fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            stiffness,
            damping,
            mass,
        }
    }

    /// Advances `state` toward `target` over `dt_secs` using stable 4 ms substeps.
    pub fn step(self, mut state: SpringState, target: f32, dt_secs: f32) -> SpringState {
        let mut remaining = dt_secs.clamp(0.0, 0.064);
        let inv_mass = 1.0 / self.mass.max(0.001);
        while remaining > 1e-6 {
            let h = remaining.min(0.004);
            let displacement = state.position - target;
            let accel = (-self.stiffness * displacement - self.damping * state.velocity) * inv_mass;
            state.velocity += accel * h;
            state.position += state.velocity * h;
            remaining -= h;
        }
        state
    }

    /// Returns true once both position error and velocity are within rest tolerance.
    pub fn is_settled(self, state: SpringState, target: f32, epsilon: f32) -> bool {
        (state.position - target).abs() <= epsilon && state.velocity.abs() <= epsilon * 8.0
    }
}

/// Instantaneous position and velocity of a scalar spring.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpringState {
    pub position: f32,
    pub velocity: f32,
}

impl SpringState {
    pub const fn at(position: f32) -> Self {
        Self {
            position,
            velocity: 0.0,
        }
    }
}

/// Returns a staggered per-row spring whose natural frequency gently lowers down the viewport.
pub fn lag_row_spring(along: f32) -> SpringConfig {
    let freq = 1.0 - LAG_STAGGER * along.clamp(0.0, 1.0);
    SpringConfig::new(
        SPRING_LYRICS_ROW.stiffness * freq * freq,
        SPRING_LYRICS_ROW.damping * freq,
        SPRING_LYRICS_ROW.mass,
    )
}

/// Computes the exact GPUI scroll offset that pins unscrolled child `item_top` at `view_h * pin`.
///
/// GPUI's `ScrollHandle::bounds_for_item` returns unscrolled layout bounds, and scroll offsets lie
/// in `[-max_off, 0.0]`. Screen-relative Y is `item_top + scroll_offset - view_top`.
pub fn anchored_lyrics_offset(
    view_top: f32,
    item_top: f32,
    view_h: f32,
    max_off: f32,
    pin: f32,
    optical_shift: f32,
) -> f32 {
    (view_top - item_top + view_h * pin - optical_shift).clamp(-max_off.max(0.0), 0.0)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

/// Measured, wrapped representation of one verse ready for 60 FPS subpixel karaoke rendering.
#[derive(Clone)]
pub struct WrappedVerse {
    pub spoken: Vec<usize>,
    pub rows: Vec<Range<usize>>,
    pub widths: Vec<f32>,
    pub spans: Vec<(f32, f32)>,
    pub evenly: Vec<bool>,
    pub text: Vec<SharedString>,
    pub synthetic_windows: Vec<(f64, f64, bool)>,
}

/// Width- and font-size-keyed cache of shaped lyric verses so 60 FPS frames never re-shape text.
#[derive(Default)]
pub struct LyricsWrapCache {
    fingerprint: u64,
    width_key: u32,
    size_key: u32,
    entries: HashMap<usize, WrappedVerse>,
}

impl LyricsWrapCache {
    /// Invalidates cached measurements whenever the lyrics list, wrap width, or font size changes.
    pub fn sync_layout(&mut self, lines: &[LyricLine], wrap_width: f32, font_size: f32) -> bool {
        let fp = lyrics_fingerprint(lines);
        let w_key = (wrap_width.max(0.0) * 2.0).round() as u32;
        let s_key = (font_size.max(0.0) * 4.0).round() as u32;
        if self.fingerprint != fp || self.width_key != w_key || self.size_key != s_key {
            self.fingerprint = fp;
            self.width_key = w_key;
            self.size_key = s_key;
            self.entries.clear();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.fingerprint = 0;
    }

    /// Returns a cached `WrappedVerse` for `line_idx`, shaping it once on first request.
    pub fn get_or_measure(
        &mut self,
        line_idx: usize,
        line: &LyricLine,
        next_time: Option<f64>,
        font_size: f32,
        wrap_width: f32,
        window: &mut Window,
    ) -> Option<&WrappedVerse> {
        if wrap_width <= 8.0 {
            return None;
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = self.entries.entry(line_idx) {
            let measured = measure_verse(line, next_time, font_size, wrap_width, window)?;
            slot.insert(measured);
        }
        self.entries.get(&line_idx)
    }
}

fn lyrics_fingerprint(lines: &[LyricLine]) -> u64 {
    let mut h = DefaultHasher::new();
    lines.len().hash(&mut h);
    if let Some(first) = lines.first() {
        first.text.hash(&mut h);
        first.time_secs.to_bits().hash(&mut h);
        first.words.len().hash(&mut h);
    }
    if let Some(mid) = lines.get(lines.len() / 2) {
        mid.text.hash(&mut h);
        mid.time_secs.to_bits().hash(&mut h);
    }
    if let Some(last) = lines.last() {
        last.text.hash(&mut h);
        last.time_secs.to_bits().hash(&mut h);
    }
    h.finish()
}

fn sanitize_single_line(raw: &str) -> String {
    if !raw.contains(['\n', '\r']) {
        return raw.to_string();
    }
    raw.replace(['\r', '\n'], " ")
}

fn measure_verse(
    line: &LyricLine,
    next_time: Option<f64>,
    font_size: f32,
    wrap_width: f32,
    window: &mut Window,
) -> Option<WrappedVerse> {
    let clean = sanitize_single_line(&line.text);
    if clean.trim().is_empty() {
        return None;
    }

    let parts = if !line.words.is_empty() {
        karaoke_parts(&clean, &line.words)
    } else {
        plain_lyrics_fragments(&clean)
            .into_iter()
            .enumerate()
            .map(|(idx, piece)| (piece, idx))
            .collect()
    };
    if parts.is_empty() {
        return None;
    }

    let mut style = window.text_style();
    style.font_weight = FontWeight::SEMIBOLD;

    let whole_str: String = parts.iter().map(|(t, _)| t.as_str()).collect();
    let run = style.to_run(whole_str.len());
    let shaped =
        window
            .text_system()
            .shape_line(SharedString::from(whole_str), px(font_size), &[run], None);

    let mut widths = Vec::with_capacity(parts.len());
    let mut cursor = 0usize;
    let mut left_x: f32 = shaped.x_for_index(0).into();
    for (piece, _) in &parts {
        cursor += piece.len();
        let right_x: f32 = shaped.x_for_index(cursor).into();
        widths.push((right_x - left_x).max(0.0));
        left_x = right_x;
    }

    let fragments: Vec<String> = parts.iter().map(|(t, _)| t.clone()).collect();
    let spoken: Vec<usize> = parts.iter().map(|(_, w)| *w).collect();
    let breaks: Vec<bool> = fragments
        .iter()
        .enumerate()
        .map(|(idx, frag)| match idx.checked_sub(1) {
            Some(prev) => separable(&fragments[prev], frag),
            None => true,
        })
        .collect();

    let rows = wrap_fragment_widths(&widths, &breaks, wrap_width);
    let text = rows
        .iter()
        .map(|range| {
            let mut s = String::new();
            for (piece, _) in &parts[range.clone()] {
                s.push_str(piece);
            }
            SharedString::from(s.trim_end().to_string())
        })
        .collect::<Vec<_>>();

    let spans = word_spans(&spoken, &widths);
    let evenly = evenly_filled(&fragments, &spoken);
    let synthetic_windows = if line.words.is_empty() {
        synthesize_word_windows(&fragments, line.time_secs, next_time)
    } else {
        Vec::new()
    };

    Some(WrappedVerse {
        spoken,
        rows,
        widths,
        spans,
        evenly,
        text,
        synthetic_windows,
    })
}

/// Computes natural syllable-weighted timing windows for line-synced verses without word timings.
pub fn synthesize_word_windows(
    fragments: &[String],
    line_start: f64,
    next_time: Option<f64>,
) -> Vec<(f64, f64, bool)> {
    if fragments.is_empty() {
        return Vec::new();
    }
    let raw_span = next_time
        .map(|n| (n - line_start - 0.08).clamp(0.55, 6.2))
        .unwrap_or(2.8);
    let weights: Vec<f64> = fragments
        .iter()
        .map(|f| {
            let trimmed = f.trim();
            let base = trimmed.chars().count().max(1) as f64;
            let pause = match trimmed.chars().last() {
                Some(',' | ';' | ':') => 0.55,
                Some('.' | '!' | '?') => 0.85,
                _ => 0.0,
            };
            base + pause
        })
        .collect();
    let total_w: f64 = weights.iter().sum::<f64>().max(1.0);
    let mut t = line_start;
    let last_idx = fragments.len().saturating_sub(1);
    weights
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let dur = (raw_span * (w / total_w)).max(0.06);
            let start = t;
            let end = (t + dur).min(line_start + raw_span);
            t = end;
            (start, end, i == last_idx)
        })
        .collect()
}

fn karaoke_window(line_start: f64, words: &[LyricWord], index: usize) -> (f64, f64) {
    let word = &words[index];
    let start = if index == 0 {
        line_start.min(word.start_secs)
    } else {
        word.start_secs
    };
    let sung = word.end_secs.unwrap_or(start).max(start);
    let end = match words.get(index + 1) {
        Some(next) => {
            if sung > start {
                next.start_secs.max(start).min(sung)
            } else {
                next.start_secs.max(start)
            }
        }
        None => {
            if sung > start {
                sung
            } else {
                start + 0.42
            }
        }
    };
    (start, end)
}

fn progress_between(start: f64, end: f64, pos: f64) -> f32 {
    if pos < start {
        return 0.0;
    }
    if pos >= end {
        return 1.0;
    }
    let span = (end - start).max(1e-4);
    ((pos - start) / span).clamp(0.0, 1.0) as f32
}

fn swept(start: f64, end: f64, pos: f64, tail: bool) -> f32 {
    let span = (end - start).max(0.0);
    let travel = if tail {
        span.max(SWEEP_LEAST_SECS)
    } else {
        (span * SWEEP_STRETCH).max(SWEEP_LEAST_SECS)
    };
    let eased = ease_out_cubic(progress_between(start, start + travel, pos));
    if eased >= SWEPT_SNAP {
        1.0
    } else {
        eased
    }
}

#[derive(Clone, Copy)]
struct RowReveal {
    shown: bool,
    front_px: f32,
    row_width_px: f32,
    landing: f32,
}

fn compute_row_reveal(
    plan: &WrappedVerse,
    row: usize,
    windows: &[(f64, f64, bool)],
    pos: f64,
    fade_px: f32,
) -> RowReveal {
    let mut front = 0.0_f32;
    let mut offset = 0.0_f32;
    for index in plan.rows[row].clone() {
        let mine = plan.widths.get(index).copied().unwrap_or(0.0);
        let word = plan.spoken.get(index).copied().unwrap_or(index);
        let Some(&(start, end, is_last)) = windows.get(word) else {
            offset += mine;
            continue;
        };
        let even = plan.evenly.get(word).copied().unwrap_or(false);
        let share = if even {
            progress_between(start, end, pos)
        } else {
            swept(start, end, pos, is_last)
        };
        if share > 0.0 {
            let (before, whole) = plan.spans.get(index).copied().unwrap_or((0.0, mine));
            let part = if mine > 0.0 {
                ((whole * share - before) / mine).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let reach = offset + mine * part;
            if part > 0.0 && reach > front {
                front = reach;
            }
        }
        offset += mine;
    }

    let landing = if fade_px > 0.0 {
        ((offset - front) / fade_px).clamp(0.0, 1.0)
    } else {
        0.0
    };

    RowReveal {
        shown: front > 0.0,
        front_px: front.min(offset),
        row_width_px: offset,
        landing,
    }
}

/// Visual options for rendering wrapped karaoke verses across Fullscreen and Right Panel.
#[derive(Clone, Copy)]
pub struct KaraokeRenderStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub base_color: Hsla,
    pub lit_color: Hsla,
    pub is_counter: bool,
    pub is_bg: bool,
}

/// Renders an active verse with 60 FPS subpixel feathered letter-by-letter karaoke sweep.
pub fn render_karaoke_rows(
    plan: &WrappedVerse,
    line: &LyricLine,
    pos: f64,
    style: KaraokeRenderStyle,
) -> AnyElement {
    let owned_windows;
    let windows: &[(f64, f64, bool)] = if !line.words.is_empty() {
        owned_windows = (0..line.words.len())
            .map(|wi| {
                let (s, e) = karaoke_window(line.time_secs, &line.words, wi);
                (s, e, wi + 1 >= line.words.len())
            })
            .collect::<Vec<_>>();
        &owned_windows
    } else {
        &plan.synthetic_windows
    };

    let fade_px = style.font_size * REVEAL_FADE_FACTOR;
    let mut rows_col = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .when(style.is_counter, |d| d.items_end())
        .when(!style.is_counter, |d| d.items_start());

    for row_i in 0..plan.rows.len() {
        let row_text = plan.text[row_i].clone();
        let rev = compute_row_reveal(plan, row_i, windows, pos, fade_px);
        let feather = fade_px * rev.landing;

        let base_line = |text: SharedString, color: Hsla| {
            let mut d = div()
                .whitespace_nowrap()
                .text_size(px(style.font_size))
                .line_height(px(style.line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color);
            if style.is_bg {
                d = d.italic();
            }
            d.child(text)
        };

        let mut row_box = div()
            .relative()
            .flex_none()
            .whitespace_nowrap()
            .child(base_line(row_text.clone(), style.base_color));

        if rev.shown {
            // Soft outer and mid feathered slices create a silky glowing wavefront right at `front_px`.
            if feather > 1.0 && rev.front_px < rev.row_width_px {
                let outer_w = (rev.front_px + feather * 0.72).min(rev.row_width_px);
                let mid_w = (rev.front_px + feather * 0.36).min(rev.row_width_px);
                row_box = row_box
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(outer_w))
                            .overflow_hidden()
                            .child(base_line(row_text.clone(), style.lit_color.opacity(0.24))),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(mid_w))
                            .overflow_hidden()
                            .child(base_line(row_text.clone(), style.lit_color.opacity(0.52))),
                    );
            }
            // Crisp lit core clipped to exact subpixel reach.
            row_box = row_box.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(rev.front_px))
                    .overflow_hidden()
                    .child(base_line(row_text, style.lit_color)),
            );
        }

        rows_col = rows_col.child(row_box);
    }

    rows_col.into_any_element()
}

/// Renders pre-wrapped rows of an inactive verse (with optional 8-tap soft optical defocus).
pub fn render_fixed_rows(
    rows: &[SharedString],
    font_size: f32,
    line_height: f32,
    color: Hsla,
    blur_radius: f32,
    is_counter: bool,
    is_bg: bool,
) -> AnyElement {
    let render_stack = |tint: Hsla| {
        let mut col = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .when(is_counter, |d| d.items_end())
            .when(!is_counter, |d| d.items_start());
        for row_text in rows {
            let mut d = div()
                .whitespace_nowrap()
                .text_size(px(font_size))
                .line_height(px(line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tint);
            if is_bg {
                d = d.italic();
            }
            col = col.child(d.child(row_text.clone()));
        }
        col
    };

    if blur_radius < 0.65 || color.a < 0.015 {
        return render_stack(color).into_any_element();
    }

    let r = blur_radius.clamp(0.65, 2.35);
    let diag = r * std::f32::consts::FRAC_1_SQRT_2;
    let taps: [(f32, f32, f32); 8] = [
        (r, 0.0, 0.14),
        (-r, 0.0, 0.14),
        (0.0, r, 0.14),
        (0.0, -r, 0.14),
        (diag, diag, 0.11),
        (-diag, diag, 0.11),
        (diag, -diag, 0.11),
        (-diag, -diag, 0.11),
    ];

    let mut stack = div()
        .relative()
        .w_full()
        .min_w_0()
        .child(render_stack(color.opacity(0.24)));

    for (ox, oy, weight) in taps {
        stack = stack.child(
            div()
                .absolute()
                .top(px(oy))
                .left(px(ox))
                .w_full()
                .child(render_stack(color.opacity(weight))),
        );
    }

    stack.into_any_element()
}

// ---- Fragmentation & Width-Wrapping Helpers ---------------------------------

fn karaoke_parts(line: &str, words: &[LyricWord]) -> Vec<(String, usize)> {
    let mut starts = Vec::with_capacity(words.len());
    let mut cursor = 0usize;
    for word in words {
        if word.text.is_empty() {
            return spaced_words(words);
        }
        let Some(rem) = line.get(cursor..) else {
            return spaced_words(words);
        };
        let Some(rel) = rem.find(&word.text) else {
            return spaced_words(words);
        };
        let start = cursor + rel;
        starts.push(start);
        cursor = start + word.text.len();
    }

    starts
        .iter()
        .enumerate()
        .flat_map(|(idx, start)| {
            let s = if idx == 0 { 0 } else { *start };
            let e = starts.get(idx + 1).copied().unwrap_or(line.len());
            plain_lyrics_fragments(&line[s..e])
                .into_iter()
                .map(move |piece| (piece, idx))
        })
        .collect()
}

fn spaced_words(words: &[LyricWord]) -> Vec<(String, usize)> {
    words
        .iter()
        .enumerate()
        .flat_map(|(idx, word)| {
            let mut text = word.text.clone();
            if words
                .get(idx + 1)
                .is_some_and(|next| needs_space(&word.text, &next.text))
            {
                text.push(' ');
            }
            plain_lyrics_fragments(&text)
                .into_iter()
                .map(move |piece| (piece, idx))
        })
        .collect()
}

fn needs_space(left: &str, right: &str) -> bool {
    let Some(last) = left.chars().next_back() else {
        return false;
    };
    let Some(first) = right.chars().next() else {
        return false;
    };
    if last.is_whitespace() || first.is_whitespace() || is_wide_char(last) || is_wide_char(first) {
        return false;
    }
    !matches!(last, '(' | '[' | '{' | '\'' | '’' | '-' | '—')
        && !matches!(
            first,
            ')' | ']' | '}' | ',' | '.' | '!' | '?' | ';' | ':' | '%' | '\'' | '’' | '-' | '—'
        )
}

pub fn plain_lyrics_fragments(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut spacing = false;
    let mut previous = None;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            spacing = true;
        } else if spacing || previous.is_some_and(|p| can_split_cjk(p, ch)) {
            out.push(line[start..idx].to_owned());
            start = idx;
            spacing = false;
        }
        previous = Some(ch);
    }
    if start < line.len() {
        out.push(line[start..].to_owned());
    }
    out
}

fn word_spans(spoken: &[usize], widths: &[f32]) -> Vec<(f32, f32)> {
    let count = spoken.iter().copied().max().map_or(0, |last| last + 1);
    let mut wholes = vec![0.0_f32; count];
    for (idx, &word) in spoken.iter().enumerate() {
        wholes[word] += widths.get(idx).copied().unwrap_or(0.0);
    }
    let mut befores = vec![0.0_f32; count];
    spoken
        .iter()
        .enumerate()
        .map(|(idx, &word)| {
            let before = befores[word];
            befores[word] += widths.get(idx).copied().unwrap_or(0.0);
            (before, wholes[word])
        })
        .collect()
}

fn evenly_filled(fragments: &[String], spoken: &[usize]) -> Vec<bool> {
    let count = spoken.iter().copied().max().map_or(0, |last| last + 1);
    let mut pieces = vec![0usize; count];
    let mut broad = vec![false; count];
    for (idx, &word) in spoken.iter().enumerate() {
        pieces[word] += 1;
        broad[word] |= fragments[idx].chars().any(is_wide_char);
    }
    pieces
        .into_iter()
        .zip(broad)
        .map(|(p, b)| p > 1 || b)
        .collect()
}

fn separable(left: &str, right: &str) -> bool {
    if right.starts_with(char::is_whitespace) {
        return false;
    }
    if left.ends_with(char::is_whitespace) {
        return true;
    }
    match (left.chars().next_back(), right.chars().next()) {
        (Some(l), Some(r)) => can_split_cjk(l, r),
        _ => false,
    }
}

fn is_wide_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{2E80}'..='\u{303E}'
            | '\u{3041}'..='\u{33FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{A000}'..='\u{A4CF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF00}'..='\u{FF60}'
    )
}

fn can_split_cjk(left: char, right: char) -> bool {
    if !is_wide_char(left) || !is_wide_char(right) {
        return false;
    }
    !matches!(
        right,
        '、' | '。'
            | '，'
            | '．'
            | '！'
            | '？'
            | '：'
            | '；'
            | '」'
            | '』'
            | '）'
            | '】'
            | '〉'
            | '》'
            | '〕'
            | '・'
            | 'ー'
            | '…'
            | '々'
            | 'ゝ'
            | 'ゞ'
            | 'っ'
            | 'ッ'
    ) && !matches!(left, '「' | '『' | '（' | '【' | '〈' | '《' | '〔')
}

fn wrap_fragment_widths(widths: &[f32], breaks: &[bool], max_width: f32) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut used = 0.0_f32;

    for (idx, &frag_w) in widths.iter().enumerate() {
        if idx > start && used + frag_w > max_width {
            if let Some(split) = (start + 1..=idx)
                .rev()
                .find(|&at| breaks.get(at).copied().unwrap_or(true))
            {
                rows.push(start..split);
                used = widths[split..idx].iter().copied().sum();
                start = split;
            } else {
                // Force wrap inside unbroken ultra-long runs so text never overflows off-screen.
                rows.push(start..idx);
                used = 0.0;
                start = idx;
            }
        }
        used += frag_w;
    }
    if start < widths.len() {
        rows.push(start..widths.len());
    }
    rows
}

/// Computes `LyricVoice::Counter` helper.
pub fn is_counter_voice(voice: LyricVoice) -> bool {
    voice == LyricVoice::Counter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_scroll_settles_smoothly_without_overshoot_runaway() {
        let mut s = SpringState::at(0.0);
        let target = -420.0;
        for _ in 0..120 {
            s = SPRING_LYRICS_SCROLL.step(s, target, 1.0 / 60.0);
        }
        assert!(
            (s.position - target).abs() < 0.5,
            "spring should settle near {target}, got {}",
            s.position
        );
    }

    #[test]
    fn wrap_fragment_widths_never_overflows_column() {
        let widths = vec![90.0, 80.0, 110.0, 95.0, 120.0];
        let breaks = vec![true, true, true, true, true];
        let rows = wrap_fragment_widths(&widths, &breaks, 220.0);
        assert_eq!(rows, vec![0..2, 2..4, 4..5]);
        for row in rows {
            let sum: f32 = widths[row].iter().sum();
            assert!(sum <= 220.0, "row sum {sum} exceeded max_width 220");
        }
    }

    #[test]
    fn synthesized_word_windows_advance_monotonically_across_verse() {
        let frags = plain_lyrics_fragments("Tobey Maguire got bit by a spider, but see, me,");
        let wins = synthesize_word_windows(&frags, 10.0, Some(13.5));
        assert_eq!(wins.len(), frags.len());
        assert!(wins.first().unwrap().0 >= 10.0);
        assert!(wins.last().unwrap().1 <= 13.5);
        for pair in wins.windows(2) {
            assert!(pair[0].1 <= pair[1].0 + 1e-6);
        }
    }
}
