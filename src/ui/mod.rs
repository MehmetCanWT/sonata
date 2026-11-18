pub mod modals;
pub mod now_playing;
pub mod sidebar;
pub mod track_list;

use crate::app::App;
use crate::providers::format_duration;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.bg).fg(app.theme.fg)),
        area,
    );

    let auto_compact = area.width < 48 || area.height < 12;
    if app.mini_mode || auto_compact {
        render_mini_layout(frame, area, app);
    } else {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(6)])
            .split(area);

        // Mirrored layout: Now Playing + Lyrics (left) | Tracks (center) | Library (right)
        let left_pct = app.config.left_pane_pct.clamp(18, 52);
        let right_pct = app.config.right_pane_pct.clamp(14, 48);
        let center_pct = 100u16.saturating_sub(left_pct + right_pct).max(20);
        let main_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Percentage(center_pct),
                Constraint::Percentage(right_pct),
            ])
            .split(root[0]);

        app.ui_rects.main_area = root[0];
        app.ui_rects.split_left_x = main_cols[1].x;
        app.ui_rects.split_right_x = main_cols[2].x;

        now_playing::render(frame, main_cols[0], app);
        track_list::render(frame, main_cols[1], app);
        sidebar::render(frame, main_cols[2], app);
        render_player_bar(frame, root[1], app);
    }
    modals::render(frame, area, app);
}

fn render_mini_layout(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.search_bar = Rect::default();
    app.ui_rects.tracks_rows = Rect::default();
    app.ui_rects.sidebar_inner = Rect::default();
    // Tier 3: Very small -> Ultra-compact mini player only (track name + timebar + controls)
    if area.width < 36 || area.height < 10 {
        app.ui_rects.album_art_box = Rect::default();
        if app.last_art_rect.is_some() {
            app.last_art_rect = None;
            app.clear_kitty_images();
        }
        render_mini_player_bar(frame, area, app);
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(5)])
        .split(area);

    // Tier 2: Narrow or short -> Album Art only + Mini player bar
    if area.width < 58 || area.height < 15 {
        now_playing::render_artwork(frame, root[0], app);
    } else {
        // Tier 1: Mini Mode -> Album Art (left) + Lyrics (right) + Mini player bar
        let (fw, fh) = app.picker.font_size();
        let inner_h = root[0].height.saturating_sub(2) as u32;
        let ideal_art_w = ((inner_h * fh.max(1) as u32) / fw.max(1) as u32) as u16 + 2;
        let max_art_w = (root[0].width * 55 / 100).max(16);
        let art_w = ideal_art_w.clamp(16.min(max_art_w), max_art_w);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(art_w), Constraint::Min(16)])
            .split(root[0]);
        now_playing::render_artwork(frame, cols[0], app);
        now_playing::render_lyrics(frame, cols[1], app);
    }
    render_mini_player_bar(frame, root[1], app);
}

fn render_mini_player_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.player_bar = area;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    app.ui_rects.player_top = rows[0];
    app.ui_rects.progress_bar = rows[1];
    app.ui_rects.volume_bar = Rect::default();

    let title_text = match app.playback.current_track() {
        Some(t) => format!("{} — {}", t.title, t.artist),
        None => "No track playing".to_string(),
    };
    let top_line = Line::from(Span::styled(
        title_text,
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(top_line).alignment(Alignment::Center),
        rows[0],
    );

    render_progress_row(frame, rows[1], app);

    let play_label = if app.playback.paused {
        " ▶ Play "
    } else {
        " ⏸ Pause "
    };
    let ctrl_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Fill(1),
        ])
        .split(rows[2]);

    app.ui_rects.ctrl_prev = ctrl_cols[1];
    app.ui_rects.ctrl_play = ctrl_cols[2];
    app.ui_rects.ctrl_next = ctrl_cols[3];

    frame.render_widget(
        Paragraph::new(" ⏮ Prev ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.fg)),
        ctrl_cols[1],
    );
    frame.render_widget(
        Paragraph::new(play_label)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ctrl_cols[2],
    );
    frame.render_widget(
        Paragraph::new(" Next ⏭ ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.fg)),
        ctrl_cols[3],
    );
}

fn render_player_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.player_bar = area;
    app.ui_rects.ctrl_prev = Rect::default();
    app.ui_rects.ctrl_play = Rect::default();
    app.ui_rects.ctrl_next = Rect::default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    app.ui_rects.player_top = rows[0];
    app.ui_rects.progress_bar = rows[1];

    let (title, artist) = match app.playback.current_track() {
        Some(t) => (t.title.as_str(), t.artist.as_str()),
        None => ("No track playing", "Select a track and press Enter"),
    };

    let play_icon = if app.playback.loading_track {
        "◌"
    } else if app.playback.paused {
        "▶"
    } else {
        "⏸"
    };

    let badge_str = format!(
        "[{}] [{}] ",
        app.playback.stream_badge,
        app.playback.enhancer.label()
    );
    let badge_w = (badge_str.chars().count() as u16).min(rows[0].width / 2);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(14),
            Constraint::Min(12),
            Constraint::Length(badge_w),
        ])
        .split(rows[0]);

    let ctrl_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Length(4),
        ])
        .split(top_cols[0]);
    app.ui_rects.ctrl_prev = ctrl_cols[0];
    app.ui_rects.ctrl_play = ctrl_cols[1];
    app.ui_rects.ctrl_next = ctrl_cols[2];

    frame.render_widget(
        Paragraph::new(" ⏮ ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.muted)),
        ctrl_cols[0],
    );
    frame.render_widget(
        Paragraph::new(format!(" {play_icon} "))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ctrl_cols[1],
    );
    frame.render_widget(
        Paragraph::new(" ⏭ ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.muted)),
        ctrl_cols[2],
    );

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("— {artist}"), Style::default().fg(app.theme.muted)),
    ]);
    frame.render_widget(
        Paragraph::new(title_line).alignment(Alignment::Center),
        top_cols[1],
    );

    let badges = Line::from(vec![
        Span::styled(
            format!("[{}]", app.playback.stream_badge),
            Style::default().fg(app.theme.success),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}] ", app.playback.enhancer.label()),
            Style::default().fg(app.theme.warning),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(badges).alignment(Alignment::Right),
        top_cols[2],
    );

    render_progress_row(frame, rows[2], app);

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(10)])
        .split(rows[3]);

    let vol_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(12),
            Constraint::Length(7),
        ])
        .split(bottom_cols[0]);
    app.ui_rects.volume_bar = vol_cols[1];

    frame.render_widget(
        Paragraph::new(Span::styled(" Vol ", Style::default().fg(app.theme.muted))),
        vol_cols[0],
    );
    let vol_ratio = (app.playback.volume as f64 / 100.0).clamp(0.0, 1.0);
    frame.render_widget(
        Paragraph::new(volume_line(
            vol_ratio,
            vol_cols[1].width as usize,
            app.theme.accent,
            app.theme.fg,
            app.theme.border,
        )),
        vol_cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {:>3}% ", app.playback.volume),
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )),
        vol_cols[2],
    );

    let bottom_line = Line::from(vec![
        Span::styled(
            format!(
                "│ Shuffle: {} | Repeat: {} | Theme: {} (t) | Settings (S) | Keybinds (k)   ",
                if app.playback.shuffle { "On" } else { "Off" },
                app.playback.repeat.label(),
                app.theme.name
            ),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled(app.status_line.as_str(), Style::default().fg(app.theme.fg)),
    ]);
    frame.render_widget(
        Paragraph::new(bottom_line).alignment(Alignment::Left),
        bottom_cols[1],
    );
}

fn volume_line(
    ratio: f64,
    width: usize,
    fill_fg: ratatui::style::Color,
    thumb_fg: ratatui::style::Color,
    empty_fg: ratatui::style::Color,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    let filled = ((ratio.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    let mut spans = Vec::with_capacity(3);
    if filled > 1 {
        spans.push(Span::styled(
            "━".repeat(filled - 1),
            Style::default().fg(fill_fg),
        ));
    }
    if filled > 0 {
        spans.push(Span::styled("●", Style::default().fg(thumb_fg)));
    }
    let rest = width.saturating_sub(filled);
    if rest > 0 {
        spans.push(Span::styled(
            "─".repeat(rest),
            Style::default().fg(empty_fg),
        ));
    }
    Line::from(spans)
}

fn render_progress_row(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let pos = app.interpolated_position();
    let dur = app.playback.duration_secs.max(1.0);
    let ratio = (pos / dur).clamp(0.0, 1.0);
    let left_label = format!(" {:>5} ", format_duration(Some(pos as u64)));
    let right_label = format!(
        " {:<5} ",
        format_duration(Some(app.playback.duration_secs as u64))
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_label.len() as u16),
            Constraint::Min(8),
            Constraint::Length(right_label.len() as u16),
        ])
        .split(area);

    app.ui_rects.progress_bar = cols[1];

    frame.render_widget(
        Paragraph::new(Span::styled(left_label, Style::default().fg(app.theme.fg))),
        cols[0],
    );
    let bar = smooth_bar(ratio, cols[1].width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(
            bar,
            Style::default()
                .fg(app.theme.accent)
                .bg(app.theme.highlight_bg),
        )),
        cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            right_label,
            Style::default().fg(app.theme.muted),
        )),
        cols[2],
    );
}

fn smooth_bar(ratio: f64, width: usize) -> String {
    const EIGHTHS: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    if width == 0 {
        return String::new();
    }
    let total_eighths =
        ((ratio.clamp(0.0, 1.0) * (width * 8) as f64).round() as usize).min(width * 8);
    let full = total_eighths / 8;
    let rem = total_eighths % 8;
    let mut out = String::with_capacity(width * 3);
    for _ in 0..full {
        out.push('█');
    }
    if full < width {
        out.push_str(EIGHTHS[rem]);
        for _ in (full + 1)..width {
            out.push(' ');
        }
    }
    out
}
