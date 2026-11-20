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
use sonata_core::lyrics::{active_line_index, LyricLine, LyricsState};

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
    app.ui_rects.album_art_box = area;
    let has_motion = !app.anim_art_frames.is_empty() && !app.config.reduce_motion;
    let art_title = if has_motion {
        " ✦ Motion Cover "
    } else {
        match app.picker.protocol_type() {
            ratatui_image::picker::ProtocolType::Halfblocks => " Album Art (Fallback) ",
            _ => " Album Art ",
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
        if app.kitty_art_png.is_some() {
            app.last_art_rect = Some(center_square_area(inner, app.picker.font_size()));
        } else {
            app.last_art_rect = None;
            let placeholder = if app.playback.current_track().is_some() {
                "Loading HD cover art..."
            } else {
                "No track playing"
            };
            render_centered_message(frame, inner, placeholder, app);
        }
    } else {
        let font_size = app.picker.font_size();
        let art_state = if has_motion {
            let idx = app.anim_art_idx % app.anim_art_frames.len();
            app.anim_art_frames.get_mut(idx).map(|(proto, _)| proto)
        } else {
            app.album_art.as_mut()
        };
        if let Some(art_state) = art_state {
            let art_rect = center_square_area(inner, font_size);
            let widget = StatefulImage::default().resize(Resize::Fit(Some(FilterType::Lanczos3)));
            frame.render_stateful_widget(widget, art_rect, art_state);
        } else {
            let placeholder = if app.playback.current_track().is_some() {
                "Loading HD cover art..."
            } else {
                "No track playing"
            };
            render_centered_message(frame, inner, placeholder, app);
        }
    }
}

pub fn render_lyrics(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Lyrics ")
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
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible_rows = inner.height as usize;

    match &app.lyrics {
        LyricsState::Idle => {
            render_centered_message(frame, inner, "Select a song to load lyrics", app);
        }
        LyricsState::Loading => {
            render_centered_message(frame, inner, "Fetching lyrics...", app);
        }
        LyricsState::NotFound => {
            render_centered_message(frame, inner, "No lyrics found for this track", app);
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
            let pos = app.interpolated_position();
            let active = active_line_index(lines, pos);
            let use_spacers = active.is_some() && visible_rows >= 9;
            let above_budget = (visible_rows / 2).saturating_sub(usize::from(use_spacers));
            let focus_idx = active.unwrap_or(0);
            let line_rows = |idx: usize| -> usize {
                let txt = if lines[idx].text.is_empty() {
                    "♪"
                } else {
                    lines[idx].text.as_str()
                };
                let w = if Some(idx) == active {
                    inner.width.saturating_sub(4).max(1)
                } else {
                    inner.width
                };
                wrapped_rows(txt, w)
            };

            let mut start = focus_idx;
            let mut used_above = 0usize;
            while start > 0 {
                let r = line_rows(start - 1);
                if used_above + r > above_budget {
                    break;
                }
                used_above += r;
                start -= 1;
            }

            let top_pad = above_budget.saturating_sub(used_above);
            let mut used = top_pad;
            let mut rendered: Vec<Line> = vec![Line::raw(""); top_pad];
            for (i, item) in lines.iter().enumerate().skip(start) {
                let text: &str = if item.text.is_empty() {
                    "♪"
                } else {
                    item.text.as_str()
                };
                let dist = active.map_or(3, |a| a.abs_diff(i));
                if dist == 0 && use_spacers && used < visible_rows {
                    rendered.push(Line::raw(""));
                    used += 1;
                }
                let r = line_rows(i);
                if rendered.len() > top_pad && used + r > visible_rows {
                    break;
                }
                used += r;
                if dist == 0 {
                    let next_time = lines.get(i + 1).map(|next| next.time_secs);
                    rendered.push(render_active_tui_lyric_line(
                        item,
                        next_time,
                        pos,
                        app.theme.accent,
                    ));
                    if use_spacers && used + 1 < visible_rows {
                        rendered.push(Line::raw(""));
                        used += 1;
                    }
                } else {
                    let style = match dist {
                        1 => Style::default().fg(Color::Rgb(108, 116, 134)),
                        2 => Style::default()
                            .fg(Color::Rgb(72, 78, 92))
                            .add_modifier(Modifier::DIM),
                        _ => Style::default()
                            .fg(Color::Rgb(48, 52, 64))
                            .add_modifier(Modifier::DIM),
                    };
                    rendered.push(Line::from(Span::styled(text, style)));
                }
                if used >= visible_rows {
                    break;
                }
            }

            let p = Paragraph::new(rendered)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            frame.render_widget(p, inner);
        }
    }
}

pub fn active_karaoke_line(app: &App) -> Option<Line<'static>> {
    let pos = app.interpolated_position();
    match &app.lyrics {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            let idx = active_line_index(lines, pos)?;
            let item = &lines[idx];
            let next_time = lines.get(idx + 1).map(|next| next.time_secs);
            Some(render_active_tui_lyric_line(
                item,
                next_time,
                pos,
                app.theme.accent,
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
                text.to_string(),
                Style::default().fg(app.theme.muted),
            )))
        }
        _ => None,
    }
}

fn estimate_tui_sung_duration(text: &str) -> f64 {
    let chars = text.chars().count().max(3) as f64;
    (chars * 0.065).clamp(1.1, 3.6)
}

fn is_color_too_white(color: Color) -> bool {
    match color {
        Color::White => true,
        Color::Rgb(r, g, b) => r > 220 && g > 220 && b > 220,
        _ => false,
    }
}

fn render_active_tui_lyric_line(
    item: &LyricLine,
    next_time_secs: Option<f64>,
    position_secs: f64,
    theme_accent: Color,
) -> Line<'static> {
    let accent = if is_color_too_white(theme_accent) {
        Color::Rgb(120, 220, 255)
    } else {
        theme_accent
    };
    let marker_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);

    if item.text.trim().is_empty() {
        return Line::from(vec![
            Span::styled("✦ ", marker_style),
            Span::styled(
                "♪",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ✦", marker_style),
        ]);
    }

    let sweep_pos = position_secs + 0.24;
    let sung_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .add_modifier(Modifier::BOLD);
    let current_word_style = Style::default()
        .fg(accent)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let unsung_style = Style::default()
        .fg(Color::Rgb(150, 160, 182))
        .add_modifier(Modifier::BOLD);

    let line_end_secs = next_time_secs
        .map(|ns| {
            let est = item.time_secs + estimate_tui_sung_duration(&item.text);
            let gap_cap = item.time_secs + ((ns - item.time_secs) * 0.85).max(0.45);
            est.min(gap_cap)
        })
        .unwrap_or_else(|| item.time_secs + estimate_tui_sung_duration(&item.text));

    let word_shares: Vec<(String, f64)> = if !item.words.is_empty() {
        item.words
            .iter()
            .enumerate()
            .map(|(idx, w)| {
                let raw_end = w
                    .end_secs
                    .or_else(|| item.words.get(idx + 1).map(|n| n.start_secs))
                    .unwrap_or(line_end_secs)
                    .max(w.start_secs + 0.14);
                let max_word_span = (w.text.chars().count().max(2) as f64 * 0.11).clamp(0.28, 1.35);
                let end = if w.end_secs.is_some() {
                    raw_end
                } else {
                    raw_end.min(w.start_secs + max_word_span)
                };
                let share =
                    ((sweep_pos - w.start_secs) / (end - w.start_secs).max(0.12)).clamp(0.0, 1.0);
                (w.text.clone(), share)
            })
            .collect()
    } else {
        let words: Vec<&str> = item.text.split_whitespace().collect();
        let total_chars: f64 = words.iter().map(|w| (w.chars().count() + 1) as f64).sum();
        let span = (line_end_secs - item.time_secs).max(0.5);
        let progress = ((sweep_pos - item.time_secs) / span).clamp(0.0, 1.0);
        let mut cursor = 0.0;
        words
            .into_iter()
            .map(|w| {
                let weight = (w.chars().count() + 1) as f64 / total_chars.max(1.0);
                let start = cursor;
                let end = (cursor + weight).min(1.0);
                cursor = end;
                let share = if end > start {
                    ((progress - start) / (end - start)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                (w.to_string(), share)
            })
            .collect()
    };

    let active_word_idx = word_shares
        .iter()
        .position(|(_, share)| *share > 0.0 && *share < 0.99);

    let word_count = word_shares.len();
    let mut spans = Vec::with_capacity(word_count * 2 + 2);
    spans.push(Span::styled("✦ ", marker_style));
    for (idx, (token, share)) in word_shares.into_iter().enumerate() {
        let style = if Some(idx) == active_word_idx {
            current_word_style
        } else if share >= 0.99 {
            sung_style
        } else {
            unsung_style
        };
        spans.push(Span::styled(token, style));
        if idx + 1 < word_count {
            spans.push(Span::raw(" "));
        }
    }
    spans.push(Span::styled(" ✦", marker_style));
    Line::from(spans)
}

fn wrapped_rows(text: &str, width: u16) -> usize {
    let w = width.max(1) as usize;
    text.chars().count().div_ceil(w).max(1)
}

fn render_centered_message(frame: &mut Frame<'_>, inner: Rect, msg: &str, app: &App) {
    let pad_top = inner.height / 2;
    let p = Paragraph::new(format!("{}{msg}", "\n".repeat(pad_top as usize)))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.theme.muted));
    frame.render_widget(p, inner);
}

#[cfg(test)]
mod tests {
    use super::render_active_tui_lyric_line;
    use ratatui::style::{Color, Modifier};
    use sonata_core::lyrics::{LyricLine, LyricVoice, LyricWord};

    #[test]
    fn active_lyric_underline_covers_only_the_word_not_trailing_space() {
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
        let rendered = render_active_tui_lyric_line(&line, Some(14.0), 10.2, Color::Cyan);
        let underlined: Vec<&str> = rendered
            .spans
            .iter()
            .filter(|s| s.style.add_modifier.contains(Modifier::UNDERLINED))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(underlined, vec!["around"]);
    }
}
