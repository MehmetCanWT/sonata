pub mod modals;
pub mod now_playing;
pub mod sidebar;
pub mod track_list;

use crate::app::App;
use crate::providers::format_duration;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};
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
            .constraints([Constraint::Min(10), Constraint::Length(5)])
            .split(area);

        // Mirrored layout: Now Playing + Lyrics (left) | Tracks (center) | Library (right)
        let main_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(32),
                Constraint::Percentage(46),
                Constraint::Percentage(22),
            ])
            .split(root[0]);

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
        // Tier 1: Mini Mode -> Album Art (left) + Synced Lyrics (right) + Mini player bar
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
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
    frame.render_widget(Paragraph::new(top_line).alignment(Alignment::Center), rows[0]);

    let pos = app.playback.position_secs.max(0.0);
    let dur = app.playback.duration_secs.max(1.0);
    let ratio = (pos / dur).clamp(0.0, 1.0);
    let time_label = format!(
        "{} / {}",
        format_duration(Some(pos as u64)),
        format_duration(Some(app.playback.duration_secs as u64))
    );
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(app.theme.accent)
                .bg(app.theme.highlight_bg),
        )
        .ratio(ratio)
        .label(time_label);
    frame.render_widget(gauge, rows[1]);

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
        "⏸"
    } else {
        "▶"
    };

    let top_line = Line::from(vec![
        Span::styled(
            format!(" {play_icon} {title} "),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("— {artist}   "), Style::default().fg(app.theme.muted)),
        Span::styled(
            format!("[{}]", app.playback.stream_badge),
            Style::default().fg(app.theme.success),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[DSP: {} (e)]", app.playback.enhancer.label()),
            Style::default().fg(app.theme.warning),
        ),
    ]);
    frame.render_widget(Paragraph::new(top_line).alignment(Alignment::Center), rows[0]);

    let pos = app.playback.position_secs.max(0.0);
    let dur = app.playback.duration_secs.max(1.0);
    let ratio = (pos / dur).clamp(0.0, 1.0);
    let time_label = format!(
        "{} / {}",
        format_duration(Some(pos as u64)),
        format_duration(Some(app.playback.duration_secs as u64))
    );

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(app.theme.accent)
                .bg(app.theme.highlight_bg),
        )
        .ratio(ratio)
        .label(time_label);
    frame.render_widget(gauge, rows[1]);

    let bottom_line = Line::from(vec![
        Span::styled(
            format!(" Vol: {}% ", app.playback.volume),
            Style::default().fg(app.theme.fg),
        ),
        Span::styled(
            format!(
                "| Shuffle: {} | Repeat: {} | Theme: {} (t) | Settings (S) | Keybinds (k)   ",
                if app.playback.shuffle { "On" } else { "Off" },
                app.playback.repeat.label(),
                app.theme.name
            ),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled(
            app.status_line.as_str(),
            Style::default().fg(app.theme.fg),
        ),
    ]);
    frame.render_widget(Paragraph::new(bottom_line).alignment(Alignment::Center), rows[2]);
}
