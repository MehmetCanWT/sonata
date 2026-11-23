//! Now Playing column combining HD terminal artwork and vertically centered synced lyrics.
//!
//! 1. **Artwork (`render_artwork`)**: Renders square-fitted album cover art via Kitty, Sixel, or Halfblock protocols.
//! 2. **Centered Lyrics (`render_lyrics`)**: Anchors the active synced lyric line at vertical center (`visible_rows / 2`) with distance-graded brightness.

use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use ratatui_image::picker::ProtocolType;
use ratatui_image::{FilterType, Resize, StatefulImage};
use sonata_core::lyrics::{active_line_index, LyricLine, LyricVoice, LyricsState};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.left_col_area = area;
    let (fw, fh) = app.picker.font_size();
    let inner_w = area.width.saturating_sub(2) as u32;
    let ideal_art_h = ((inner_w * fw.max(1) as u32) / fh.max(1) as u32) as i32 + 2;
    let max_art_h = area.height.saturating_sub(6).max(8) as i32;
    let art_h = (ideal_art_h + app.config.art_height_offset as i32)
        .clamp(8.min(max_art_h), max_art_h) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(art_h), Constraint::Min(6)])
        .split(area);

    app.ui_rects.split_art_y = chunks[1].y;
    render_artwork(frame, chunks[0], app);
    render_lyrics(frame, chunks[1], app);
}

fn center_square_area(inner: Rect, font_size: (u16, u16)) -> Rect {
    let fw = font_size.0.max(1) as u32;
    let fh = font_size.1.max(1) as u32;
    let avail_w = (inner.width as u32) * fw;
    let avail_h = (inner.height as u32) * fh;
    let side = avail_w.min(avail_h);
    let w_cells = ((side / fw) as u16).max(1).min(inner.width);
    let h_cells = ((side / fh) as u16).max(1).min(inner.height);
    Rect {
        x: inner.x + (inner.width.saturating_sub(w_cells)) / 2,
        y: inner.y + (inner.height.saturating_sub(h_cells)) / 2,
        width: w_cells,
        height: h_cells,
    }
}

pub fn render_artwork(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    app.ui_rects.album_art_box = area;
    let has_motion =
        !app.anim_art_frames.is_empty() && app.config.motion_covers && !app.config.reduce_motion;
    let art_title = if has_motion {
        s.tui_motion_cover
    } else {
        match app.picker.protocol_type() {
            ratatui_image::picker::ProtocolType::Halfblocks => s.tui_album_art_fallback,
            _ => s.tui_album_art,
        }
    };
    let block = Block::default()
        .title(art_title)
        .title_style(
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    if app.picker.protocol_type() == ProtocolType::Kitty {
        if app.kitty_art_png.is_some() || has_motion {
            app.last_art_rect = Some(center_square_area(inner, app.picker.font_size()));
        } else {
            app.last_art_rect = None;
            let placeholder = if app.playback.current_track().is_some() {
                s.loading_cover
            } else {
                s.no_track_playing
            };
            render_centered_message(frame, inner, placeholder, app);
        }
    } else {
        let font_size = app.picker.font_size();
        let art_state = if has_motion {
            let idx = app.anim_art_idx % app.anim_art_frames.len();
            app.anim_art_frames
                .get_mut(idx)
                .and_then(|(proto, _)| proto.as_mut())
        } else {
            app.album_art.as_mut()
        };
        if let Some(art_state) = art_state {
            let art_rect = center_square_area(inner, font_size);
            let widget = StatefulImage::default().resize(Resize::Fit(Some(FilterType::Lanczos3)));
            frame.render_stateful_widget(widget, art_rect, art_state);
        } else {
            let placeholder = if app.playback.current_track().is_some() {
                s.loading_cover
            } else {
                s.no_track_playing
            };
            render_centered_message(frame, inner, placeholder, app);
        }
    }
}

pub fn render_lyrics(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let block = Block::default()
        .title(s.tui_lyrics_title)
        .title_style(
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.bg).fg(app.theme.fg));

    let inner = block.inner(area);
    app.ui_rects.lyrics_box = inner;
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible_rows = inner.height as usize;

    match &app.lyrics {
        LyricsState::Idle => {
            render_centered_message(frame, inner, s.select_song_lyrics, app);
        }
        LyricsState::Loading => {
            render_centered_message(frame, inner, s.fetching_lyrics, app);
        }
        LyricsState::NotFound => {
            render_centered_message(frame, inner, s.no_lyrics_found, app);
        }
        LyricsState::Plain(lines) => {
            let pos = app.interpolated_position();
            let progress = if app.playback.duration_secs > 1.0 {
                (pos / app.playback.duration_secs).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let max_scroll = lines.len().saturating_sub(visible_rows.saturating_sub(1));
            let start = ((max_scroll as f64) * progress).round() as usize;
            let end = (start + visible_rows.saturating_sub(1)).min(lines.len());
            let mut rendered: Vec<Line> = vec![Line::raw("")];
            for line in &lines[start..end] {
                let is_header = line.starts_with('[') && line.ends_with(']');
                let style = if is_header {
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg)
                };
                rendered.push(Line::from(Span::styled(line.as_str(), style)));
            }
            let p = Paragraph::new(rendered)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            frame.render_widget(p, inner);
        }
        LyricsState::Synced(lines) => {
            const SCROLL_LEAD_SECS: f64 = 0.15;
            let pos = app.interpolated_position();
            let active = active_line_index(lines, pos);
            let scroll_active = active_line_index(lines, pos + SCROLL_LEAD_SECS).or(active);
            let focus_idx = scroll_active.unwrap_or(0);
            let active_bg_indices = resolve_tui_active_bg(lines, active, pos);

            // Track departing vs arriving active verse for smooth color cross-fade.
            if app.lyrics_cur_active != active {
                app.lyrics_prev_active = app.lyrics_cur_active;
                app.lyrics_cur_active = active;
                app.lyrics_changed_at = std::time::Instant::now();
            }

            // Build invariant flattened visual rows (word-wrapped cleanly with uniform spacing so
            // rows never jump or reflow when a verse becomes active).
            let use_gaps = visible_rows >= 9;
            let mut visual_rows: Vec<Option<(usize, usize, usize)>> =
                Vec::with_capacity(lines.len() * 2);
            let mut focus_start_row = 0usize;
            let mut focus_row_count = 1usize;

            for (i, item) in lines.iter().enumerate() {
                let text = if item.text.trim().is_empty() {
                    "♪"
                } else {
                    item.text.trim()
                };
                let ranges = wrap_char_ranges(text, inner.width);
                if i == focus_idx {
                    focus_start_row = visual_rows.len();
                    focus_row_count = ranges.len().max(1);
                }
                for (cs, ce) in ranges {
                    visual_rows.push(Some((i, cs, ce)));
                }
                if use_gaps && i + 1 < lines.len() {
                    visual_rows.push(None);
                }
            }

            // Critically damped spring glides the viewport smoothly into center ahead of the beat.
            let focus_center = focus_start_row as f32 + (focus_row_count as f32 - 1.0) * 0.5;
            let target_scroll = focus_center - (visible_rows.saturating_sub(1) as f32) * 0.5;
            app.lyrics_scroll_target = target_scroll;
            let dt = app
                .lyrics_last_step
                .elapsed()
                .as_secs_f32()
                .clamp(0.001, 0.05);
            app.lyrics_last_step = std::time::Instant::now();

            if !app.lyrics_scroll_init
                || app.config.reduce_motion
                || (target_scroll - app.lyrics_scroll_row).abs() > (visible_rows as f32 * 1.5)
            {
                app.lyrics_scroll_row = target_scroll;
                app.lyrics_scroll_vel = 0.0;
                app.lyrics_scroll_init = true;
            } else {
                let (next_pos, next_vel) = step_tui_scroll_spring(
                    app.lyrics_scroll_row,
                    app.lyrics_scroll_vel,
                    target_scroll,
                    dt,
                );
                app.lyrics_scroll_row = next_pos;
                app.lyrics_scroll_vel = next_vel;
            }

            let scroll_f = app.lyrics_scroll_row;
            let scroll_top = scroll_f.round() as isize;
            let sub_row = scroll_f - scroll_top as f32;
            let center_y_f = (visible_rows.saturating_sub(1) as f32) * 0.5;
            let sigma = (visible_rows as f32 * 0.25).max(2.2);
            let hop_prox = (sub_row.abs() * 2.0).clamp(0.0, 1.0);
            let motion_veil =
                (app.lyrics_scroll_vel.abs() / 30.0).clamp(0.0, 0.24) * hop_prox * hop_prox;

            let elapsed_since_change = app.lyrics_changed_at.elapsed().as_secs_f32();
            let arrive_eased = ease_out_cubic((elapsed_since_change / 0.22).clamp(0.0, 1.0));
            let depart_eased = ease_out_cubic((elapsed_since_change / 0.30).clamp(0.0, 1.0));

            let mut rendered: Vec<Line> = Vec::with_capacity(visible_rows);
            for sy in 0..visible_rows {
                let doc_idx = scroll_top + sy as isize;
                if doc_idx < 0 || (doc_idx as usize) >= visual_rows.len() {
                    rendered.push(Line::raw(""));
                    continue;
                }
                let Some((line_idx, row_c_start, row_c_end)) = visual_rows[doc_idx as usize] else {
                    rendered.push(Line::raw(""));
                    continue;
                };

                let item = &lines[line_idx];
                let text = if item.text.trim().is_empty() {
                    "♪"
                } else {
                    item.text.trim()
                };

                // Continuous floating-point screen coordinate drives liquid 60 FPS luminance flow.
                let sy_f = sy as f32 - sub_row;
                let dy = sy_f - center_y_f;
                let center_glow = (-(dy * dy) / (2.0 * sigma * sigma)).exp();
                let edge_fade = if visible_rows >= 7 {
                    let edge_dist = sy_f
                        .max(0.0)
                        .min(((visible_rows.saturating_sub(1) as f32) - sy_f).max(0.0));
                    1.0 - ease_out_cubic((edge_dist / 2.4).clamp(0.0, 1.0))
                } else {
                    0.0
                };

                let is_active = active == Some(line_idx);
                let is_active_bg = active_bg_indices.contains(&line_idx);
                if is_active || is_active_bg {
                    let next_time = Some(tui_line_end_secs(lines, line_idx));
                    let active_fg = if is_active && arrive_eased < 1.0 {
                        blend_color(
                            app.theme.lyrics_inactive,
                            app.theme.lyrics_active,
                            0.45 + 0.55 * arrive_eased,
                        )
                    } else {
                        app.theme.lyrics_active
                    };
                    rendered.push(render_active_tui_lyric_slice(
                        item,
                        next_time,
                        pos,
                        row_c_start,
                        row_c_end,
                        active_fg,
                        app.theme.lyrics_inactive,
                    ));
                } else {
                    let is_bg = is_tui_bg_line(text);
                    let is_departing =
                        app.lyrics_prev_active == Some(line_idx) && depart_eased < 1.0;
                    let spotlight_fg =
                        blend_color(app.theme.muted, app.theme.lyrics_inactive, center_glow);
                    let base_fg = if is_departing {
                        blend_color(app.theme.lyrics_active, spotlight_fg, depart_eased)
                    } else {
                        spotlight_fg
                    };
                    let fade = (edge_fade + (1.0 - edge_fade) * motion_veil).clamp(0.0, 0.88);
                    let fg = if fade > 0.005 {
                        blend_color(base_fg, app.theme.bg, fade)
                    } else {
                        base_fg
                    };
                    let mut style = Style::default().fg(fg);
                    if center_glow < 0.20 && !is_departing {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    if is_bg || item.voice == LyricVoice::Counter {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    let slice = char_slice_bounds(text, row_c_start, row_c_end);
                    rendered.push(Line::from(Span::styled(slice, style)));
                }
            }

            let p = Paragraph::new(rendered).alignment(Alignment::Center);
            frame.render_widget(p, inner);
        }
    }
}

pub fn active_karaoke_line(app: &App) -> Option<Line<'_>> {
    let pos = app.interpolated_position();
    match &app.lyrics {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            let idx = active_line_index(lines, pos)?;
            let item = &lines[idx];
            let next_time = lines.get(idx + 1).map(|n| n.time_secs);
            Some(render_active_tui_lyric_line(
                item,
                next_time,
                pos,
                app.theme.lyrics_active,
                app.theme.lyrics_inactive,
            ))
        }
        LyricsState::Plain(lines) if !lines.is_empty() => {
            let dur = app.playback.duration_secs.max(1.0);
            let progress = (pos / dur).clamp(0.0, 1.0);
            let idx = (((lines.len().saturating_sub(1)) as f64) * progress).round() as usize;
            let text = lines.get(idx)?.trim();
            if text.is_empty() {
                return None;
            }
            Some(Line::from(Span::styled(
                text,
                Style::default().fg(app.theme.muted),
            )))
        }
        _ => None,
    }
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::White => (245, 245, 250),
        Color::Gray => (168, 168, 178),
        Color::DarkGray => (110, 110, 122),
        Color::Cyan => (110, 225, 250),
        Color::LightCyan => (170, 240, 255),
        Color::Magenta => (220, 130, 255),
        Color::Yellow => (250, 225, 110),
        Color::Green => (130, 235, 145),
        Color::Blue => (120, 175, 255),
        Color::Red => (245, 115, 125),
        _ => (210, 210, 220),
    }
}

fn blend_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = color_to_rgb(a);
    let (br, bg, bb) = color_to_rgb(b);
    let mix = |x: u8, y: u8| ((x as f32) + ((y as f32) - (x as f32)) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// Computes continuous illuminated character position (`0.0..=char_count`) for smooth karaoke sweep.
fn compute_tui_sweep_chars(item: &LyricLine, next_time: Option<f64>, pos: f64) -> f32 {
    let text = item.text.trim();
    let total_chars = text.chars().count();
    if total_chars == 0 {
        return 0.0;
    }

    if !item.words.is_empty() {
        let mut cursor_byte = 0usize;
        for (wi, word) in item.words.iter().enumerate() {
            let tok = word.text.trim();
            let start_byte = if tok.is_empty() {
                cursor_byte
            } else {
                text[cursor_byte..]
                    .find(tok)
                    .map(|off| cursor_byte + off)
                    .unwrap_or(cursor_byte)
            };
            let end_byte = (start_byte + tok.len()).min(text.len());
            let w_start = if wi == 0 {
                item.time_secs.min(word.start_secs)
            } else {
                word.start_secs
            };
            let w_end = word
                .end_secs
                .or_else(|| item.words.get(wi + 1).map(|n| n.start_secs))
                .or(next_time)
                .unwrap_or(w_start + 0.42)
                .max(w_start + 0.14);

            if pos < w_start {
                return text[..start_byte].chars().count() as f32;
            }
            if pos < w_end {
                let raw = ((pos - w_start) / (w_end - w_start)).clamp(0.0, 1.0) as f32;
                let eased = ease_out_cubic(raw);
                let before = text[..start_byte].chars().count() as f32;
                let span_chars = text[start_byte..end_byte].chars().count() as f32;
                return before + span_chars * eased;
            }
            cursor_byte = end_byte;
        }
        return total_chars as f32;
    }

    // Weighted word-by-word cadence for line-synced tracks without per-word timings.
    let raw_span = next_time
        .map(|n| (n - item.time_secs - 0.08).clamp(0.55, 6.2))
        .unwrap_or(2.8);
    let elapsed = (pos - item.time_secs).max(0.0);
    if elapsed <= 0.0 {
        return 0.0;
    }
    if elapsed >= raw_span {
        return total_chars as f32;
    }

    let words: Vec<&str> = text.split_inclusive(char::is_whitespace).collect();
    let weights: Vec<f64> = words
        .iter()
        .map(|w| {
            let trimmed = w.trim();
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
    let mut t_cursor = 0.0_f64;
    let mut char_cursor = 0usize;
    for (w_str, weight) in words.into_iter().zip(weights) {
        let w_dur = (raw_span * (weight / total_w)).max(0.06);
        let w_chars = w_str.chars().count();
        if elapsed < t_cursor + w_dur {
            let raw = ((elapsed - t_cursor) / w_dur).clamp(0.0, 1.0) as f32;
            return char_cursor as f32 + (w_chars as f32) * ease_out_cubic(raw);
        }
        t_cursor += w_dur;
        char_cursor += w_chars;
    }
    total_chars as f32
}

fn char_slice_bounds(s: &str, start_char: usize, end_char: usize) -> &str {
    let mut start_byte = s.len();
    let mut end_byte = s.len();
    for (ci, (bi, _)) in s.char_indices().enumerate() {
        if ci == start_char {
            start_byte = bi;
        }
        if ci == end_char {
            end_byte = bi;
            break;
        }
    }
    if start_char >= end_char || start_byte >= end_byte {
        ""
    } else {
        &s[start_byte..end_byte]
    }
}

fn is_tui_bg_line(text: &str) -> bool {
    let t = text.trim();
    t.len() >= 3 && t.starts_with('(') && t.ends_with(')')
}

fn tui_line_end_secs(lines: &[LyricLine], idx: usize) -> f64 {
    let line = &lines[idx];
    let word_end = line
        .words
        .iter()
        .map(|w| w.end_secs.unwrap_or(w.start_secs + 0.45))
        .reduce(f64::max);
    if is_tui_bg_line(&line.text) {
        let natural =
            line.time_secs + (line.text.trim().chars().count() as f64 * 0.085).clamp(1.4, 3.6);
        return word_end.map_or(natural, |w| w.max(line.time_secs + 1.1));
    }
    let next_lead = lines
        .iter()
        .skip(idx + 1)
        .find(|n| !is_tui_bg_line(&n.text))
        .map(|n| n.time_secs);
    match (word_end, next_lead) {
        (Some(w), Some(next)) => w
            .max(next.min(line.time_secs + 6.5))
            .max(line.time_secs + 0.6),
        (Some(w), None) => w.max(line.time_secs + 0.8),
        (None, Some(next)) => next.max(line.time_secs + 0.6),
        (None, None) => line.time_secs + 3.2,
    }
}

fn resolve_tui_active_bg(lines: &[LyricLine], active: Option<usize>, pos: f64) -> Vec<usize> {
    let Some(lead_idx) = active else {
        return Vec::new();
    };
    let start = lead_idx.saturating_sub(2);
    let end = lines.len().min(lead_idx + 3);
    let mut out = Vec::new();
    for cand in start..end {
        if cand == lead_idx || !is_tui_bg_line(&lines[cand].text) {
            continue;
        }
        if lines[cand].time_secs <= pos + 0.06 && pos <= tui_line_end_secs(lines, cand) + 0.42 {
            out.push(cand);
        }
    }
    out
}

/// Critically damped spring step for crisp, silky TUI lyric row scrolling (no overshoot, no stall).
fn step_tui_scroll_spring(mut pos: f32, mut vel: f32, target: f32, dt_secs: f32) -> (f32, f32) {
    const STIFFNESS: f32 = 340.0;
    const DAMPING: f32 = 37.0;
    let mut rem = dt_secs.clamp(0.0, 0.05);
    while rem > 1e-6 {
        let h = rem.min(0.004);
        let acc = -STIFFNESS * (pos - target) - DAMPING * vel;
        vel += acc * h;
        pos += vel * h;
        rem -= h;
    }
    if (pos - target).abs() < 0.015 && vel.abs() < 0.08 {
        (target, 0.0)
    } else {
        (pos, vel)
    }
}

/// Breaks `text` into word-wrapped character index ranges `(start_char, end_char)` fitting `width`.
fn wrap_char_ranges(text: &str, width: u16) -> Vec<(usize, usize)> {
    let w = width.max(1) as usize;
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![(0, 0)];
    }

    let mut rows = Vec::new();
    let mut i = 0usize;
    let len = chars.len();
    while i < len {
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }
        let line_start = i;
        let mut line_end = i;
        let mut cur = i;
        while cur < len {
            let ws_start = cur;
            while cur < len && chars[cur].is_whitespace() {
                cur += 1;
            }
            let word_start = cur;
            while cur < len && !chars[cur].is_whitespace() {
                cur += 1;
            }
            let word_end = cur;
            if word_start == word_end {
                break;
            }
            if word_end - line_start <= w {
                line_end = word_end;
            } else if line_end == line_start {
                // Single word longer than viewport width: split at exact viewport boundary.
                line_end = (line_start + w).min(word_end);
                cur = line_end;
                break;
            } else {
                cur = ws_start;
                break;
            }
        }
        rows.push((line_start, line_end));
        i = cur;
    }
    if rows.is_empty() {
        rows.push((0, len));
    }
    rows
}

fn render_active_tui_lyric_line(
    item: &LyricLine,
    next_time: Option<f64>,
    pos: f64,
    theme_accent: Color,
    inactive_color: Color,
) -> Line<'_> {
    let text = item.text.trim();
    render_active_tui_lyric_slice(
        item,
        next_time,
        pos,
        0,
        text.chars().count(),
        theme_accent,
        inactive_color,
    )
}

fn render_active_tui_lyric_slice(
    item: &LyricLine,
    next_time: Option<f64>,
    pos: f64,
    row_start: usize,
    row_end: usize,
    theme_accent: Color,
    inactive_color: Color,
) -> Line<'_> {
    let lit_style = Style::default()
        .fg(theme_accent)
        .add_modifier(Modifier::BOLD);
    let unsung_style = Style::default()
        .fg(inactive_color)
        .add_modifier(Modifier::DIM);

    let text = item.text.trim();
    if text.is_empty() {
        return Line::from(Span::styled("♪", lit_style));
    }

    let total_chars = text.chars().count();
    let rs = row_start.min(total_chars);
    let re = row_end.min(total_chars);
    if rs >= re {
        return Line::raw("");
    }

    let lit_f = compute_tui_sweep_chars(item, next_time, pos).clamp(0.0, total_chars as f32);
    let full_chars = (lit_f.floor() as usize).min(total_chars);
    let frac = (lit_f - full_chars as f32).clamp(0.0, 1.0);

    let mut spans = Vec::with_capacity(4);
    let lit_end = full_chars.clamp(rs, re);
    if lit_end > rs {
        spans.push(Span::styled(
            char_slice_bounds(text, rs, lit_end),
            lit_style,
        ));
    }

    // Soft 2-character luminous wavefront right at the active letter.
    let mut tail_start = lit_end;
    if full_chars >= rs && full_chars < re && frac > 0.02 {
        let lead_color = blend_color(
            blend_color(inactive_color, theme_accent, 0.35 + 0.65 * frac),
            Color::Rgb(255, 255, 255),
            0.30 * frac,
        );
        spans.push(Span::styled(
            char_slice_bounds(text, full_chars, full_chars + 1),
            Style::default().fg(lead_color).add_modifier(Modifier::BOLD),
        ));
        tail_start = full_chars + 1;
        if tail_start < re && frac > 0.35 {
            let soft_color = blend_color(inactive_color, theme_accent, (frac - 0.35) * 0.55);
            spans.push(Span::styled(
                char_slice_bounds(text, tail_start, tail_start + 1),
                Style::default().fg(soft_color),
            ));
            tail_start += 1;
        }
    }

    if tail_start < re {
        spans.push(Span::styled(
            char_slice_bounds(text, tail_start, re),
            unsung_style,
        ));
    }

    Line::from(spans)
}

/// Counts exact word-wrapped visual rows matching `wrap_char_ranges`.
fn wrapped_rows(text: &str, width: u16) -> usize {
    wrap_char_ranges(text.trim(), width).len().max(1)
}

fn render_centered_message(frame: &mut Frame<'_>, inner: Rect, msg: &str, app: &App) {
    let rows = wrapped_rows(msg, inner.width) as u16;
    let pad_top = inner.height.saturating_sub(rows) / 2;
    let p = Paragraph::new(format!("{}{msg}", "\n".repeat(pad_top as usize)))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(app.theme.muted));
    frame.render_widget(p, inner);
}

#[cfg(test)]
mod tests {
    use super::{render_active_tui_lyric_line, wrapped_rows};
    use ratatui::style::{Color, Modifier};
    use sonata_core::lyrics::{LyricLine, LyricVoice, LyricWord};

    #[test]
    fn active_lyric_sweeps_character_by_character_without_word_underline() {
        let line = LyricLine {
            time_secs: 10.0,
            text: "around like no one knows".to_string(),
            words: vec![
                LyricWord {
                    start_secs: 10.0,
                    end_secs: Some(11.0),
                    text: "around".to_string(),
                },
                LyricWord {
                    start_secs: 11.0,
                    end_secs: Some(12.0),
                    text: "like".to_string(),
                },
            ],
            voice: LyricVoice::Lead,
        };
        let mid =
            render_active_tui_lyric_line(&line, Some(14.0), 10.5, Color::Cyan, Color::DarkGray);
        assert!(mid
            .spans
            .iter()
            .all(|s| !s.style.add_modifier.contains(Modifier::UNDERLINED)));
        let joined: String = mid.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "around like no one knows");
        assert!(mid.spans.len() >= 2);
        assert!(mid.spans[0].style.add_modifier.contains(Modifier::BOLD));

        let done =
            render_active_tui_lyric_line(&line, Some(14.0), 13.5, Color::Cyan, Color::DarkGray);
        let done_texts: Vec<&str> = done.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(done_texts, vec!["around like no one knows"]);
        assert_eq!(wrapped_rows("hello world again", 12), 2);
    }
}
