use crate::app::{App, FocusPane};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table};
use ratatui::Frame;
use sonata_core::providers::format_duration;
use std::borrow::Cow;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    app.ui_rects.search_bar = chunks[0];
    render_search_bar(frame, chunks[0], app);
    render_tracks_table(frame, chunks[1], app);
}

fn render_search_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let is_search = app.focus == FocusPane::SearchInput;
    let border_color = if is_search {
        app.theme.accent
    } else {
        app.theme.border
    };

    let inner_width = area.width.saturating_sub(4);
    let text = if app.search_query.is_empty() && !is_search {
        match app.config.language {
            sonata_runtime::i18n::AppLanguage::Tr => {
                if inner_width >= 78 {
                    "YouTube Music & Spotify'da aramak için / veya Spotify listesi için i bas..."
                } else if inner_width >= 42 {
                    "Aramak için / · liste aktarmak için i"
                } else {
                    "Aramak için / bas..."
                }
            }
            sonata_runtime::i18n::AppLanguage::Ru => {
                if inner_width >= 78 {
                    "Нажмите / для поиска в YouTube Music & Spotify или i для импорта Spotify..."
                } else if inner_width >= 42 {
                    "Нажмите / для поиска · i для импорта"
                } else {
                    "Нажмите / для поиска..."
                }
            }
            sonata_runtime::i18n::AppLanguage::En => {
                if inner_width >= 78 {
                    "Press / to search YouTube Music & Spotify, or i to import Spotify playlist URL..."
                } else if inner_width >= 42 {
                    "Press / to search · i to import playlist"
                } else {
                    "Press / to search..."
                }
            }
        }
        .to_string()
    } else if is_search {
        format!("{}▏", app.search_query)
    } else {
        app.search_query.clone()
    };

    let style = if app.search_query.is_empty() && !is_search {
        Style::default().fg(app.theme.muted)
    } else {
        Style::default().fg(app.theme.fg)
    };

    let title_style = Style::default()
        .fg(if is_search {
            app.theme.accent
        } else {
            app.theme.fg
        })
        .add_modifier(Modifier::BOLD);

    let search = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(style)
        .block(
            Block::default()
                .title(format!(" {} ", s.search))
                .title_style(title_style)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1))
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(app.theme.surface)),
        );
    frame.render_widget(search, area);
}

fn render_tracks_table(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let (h_title, h_artist, h_time) = match app.config.language {
        sonata_runtime::i18n::AppLanguage::Tr => ("Başlık", "Sanatçı", "Süre"),
        sonata_runtime::i18n::AppLanguage::Ru => ("Название", "Исполнитель", "Время"),
        sonata_runtime::i18n::AppLanguage::En => ("Title", "Artist", "Time"),
    };
    let is_focused = app.focus == FocusPane::Tracks;
    let border_color = if is_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let current_id = app.playback.current_track().map(|t| t.id.as_str());

    let header = Row::new([
        Cell::from(Line::from("#").centered()),
        Cell::from(h_title),
        Cell::from(h_artist),
        Cell::from(Line::from(h_time).centered()),
        Cell::from(Line::from("Src").centered()),
    ])
    .style(
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .visible_tracks
        .iter()
        .enumerate()
        .map(|(idx, track)| {
            let is_playing = current_id == Some(track.id.as_str());
            let num: Cow<'static, str> = if is_playing {
                Cow::Borrowed("▶")
            } else {
                Cow::Owned(format!("{}", idx + 1))
            };
            let row_style = if is_playing {
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg)
            };

            let dl_prefix = if app.downloading_track_ids.contains(&track.id) {
                "↓ "
            } else if app.downloaded_track_ids.contains(&track.id) {
                "✓ "
            } else {
                ""
            };
            let title_cell: Cow<'_, str> = if track.liked || !dl_prefix.is_empty() {
                let heart = if track.liked { "♥ " } else { "" };
                Cow::Owned(format!("{dl_prefix}{heart}{}", track.title))
            } else {
                Cow::Borrowed(track.title.as_str())
            };

            Row::new([
                Cell::from(Line::from(num).centered()),
                Cell::from(title_cell),
                Cell::from(track.artist.as_str()).style(Style::default().fg(app.theme.muted)),
                Cell::from(Line::from(format_duration(track.duration_secs)).centered()),
                Cell::from(Line::from(track.source_tag()).centered()),
            ])
            .style(row_style)
        })
        .collect();

    let title = format!(" {} ({}) ", app.tracks_heading, app.visible_tracks.len());
    let title_style = Style::default()
        .fg(if is_focused {
            app.theme.accent
        } else {
            app.theme.fg
        })
        .add_modifier(Modifier::BOLD);
    let table_block = Block::default()
        .title(title)
        .title_style(title_style)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg).fg(app.theme.fg));
    let inner = table_block.inner(area);
    // Header takes 1 row + 1 bottom margin row
    app.ui_rects.tracks_rows = Rect {
        x: inner.x,
        y: inner.y.saturating_add(2),
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(7),
            Constraint::Length(5),
        ],
    )
    .header(header)
    .block(table_block)
    .row_highlight_style(
        Style::default()
            .bg(app.theme.highlight_bg)
            .fg(app.theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, area, &mut app.track_table_state);
}
