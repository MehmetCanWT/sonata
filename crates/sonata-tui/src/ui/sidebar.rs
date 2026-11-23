use crate::app::{App, FocusPane, SidebarEntry};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Padding};
use ratatui::Frame;
use sonata_core::providers::ProviderSource;
use std::borrow::Cow;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let is_focused = app.focus == FocusPane::Sidebar;
    let border_color = if is_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let s = sonata_runtime::i18n::strings(app.config.language);
    let items: Vec<ListItem> = app
        .sidebar_items
        .iter()
        .map(|entry| {
            let (prefix, color) = match entry {
                SidebarEntry::Search => (" / ", app.theme.accent),
                SidebarEntry::Queue => (" ≡ ", app.theme.fg),
                SidebarEntry::Playlist(p) => match p.source {
                    ProviderSource::YouTubeMusic => (" ▶ ", app.theme.error),
                    ProviderSource::Spotify => (" ● ", app.theme.success),
                },
            };
            let label: Cow<'_, str> = match entry {
                SidebarEntry::Search => Cow::Borrowed(s.search),
                SidebarEntry::Queue => {
                    Cow::Owned(format!("{} ({})", s.queue, app.playback.queue.len()))
                }
                SidebarEntry::Playlist(p) => Cow::Borrowed(p.name.as_str()),
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::styled(label, Style::default().fg(app.theme.fg)),
            ]))
        })
        .collect();

    let title_style = Style::default()
        .fg(if is_focused {
            app.theme.accent
        } else {
            app.theme.fg
        })
        .add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(format!(" {} ", s.library))
        .title_style(title_style)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));

    app.ui_rects.sidebar_inner = block.inner(area);

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(app.theme.highlight_bg)
            .fg(app.theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );

    app.sidebar_state.select(Some(app.sidebar_idx));
    frame.render_stateful_widget(list, area, &mut app.sidebar_state);
}
