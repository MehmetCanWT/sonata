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
use sonata_core::lyrics::{active_line_index, LyricsState};

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
    let art_title = match app.picker.protocol_type() {
        ratatui_image::picker::ProtocolType::Halfblocks => " Album Art (Fallback) ",
        _ => " Album Art ",
    };
    let block = Block::default()
        .title(art_title)
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
    } else if let Some(art_state) = &mut app.album_art {
        let art_rect = center_square_area(inner, app.picker.font_size());
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

pub fn render_lyrics(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Lyrics ")
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
            let progress = if app.playback.duration_secs > 1.0 {
                (app.playback.position_secs / app.playback.duration_secs).clamp(0.0, 1.0)
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
            let active = active_line_index(lines, app.playback.position_secs);
            let above_budget = visible_rows / 2;
            let focus_idx = active.unwrap_or(0);
            let line_rows = |idx: usize| -> usize {
                let txt = if lines[idx].text.is_empty() {
                    "♪"
                } else {
                    lines[idx].text.as_str()
                };
                wrapped_rows(txt, inner.width)
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
                let r = wrapped_rows(text, inner.width);
                if rendered.len() > top_pad && used + r > visible_rows {
                    break;
                }
                used += r;
                let dist = active.map_or(3, |a| a.abs_diff(i));
                let style = match dist {
                    0 => Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .add_modifier(Modifier::BOLD),
                    1 => Style::default().fg(Color::Rgb(185, 192, 206)),
                    2 => Style::default().fg(Color::Rgb(120, 126, 140)),
                    _ => Style::default().fg(Color::Rgb(74, 79, 92)),
                };
                rendered.push(Line::from(Span::styled(text, style)));
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
