//! TUI navigation sidebar rendering source tabs, collections, and server health indicators.
//!
//! 1. **Navigation Entries**: Draws search, queue, downloads, listening stats, local albums/folders, and personal/remote playlists.
//! 2. **Source & Health Badges**: Shows provider icons and live reachability status for configured self-hosted servers.

use crate::app::{App, FocusPane, SidebarEntry, TuiLibrarySource};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph};
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
                SidebarEntry::Downloads => (" ↓ ", app.theme.accent),
                SidebarEntry::ListeningStats => (" ◔ ", app.theme.accent),
                SidebarEntry::LocalSongs => (" ♪ ", app.theme.success),
                SidebarEntry::LocalAlbum(_) => (" ◉ ", app.theme.accent),
                SidebarEntry::LocalFolder(_) => (" ▤ ", app.theme.muted),
                SidebarEntry::PersonalPlaylist(_) => (" ★ ", app.theme.accent),
                SidebarEntry::Server(_) => (" ◈ ", app.theme.accent),
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
                SidebarEntry::Downloads => {
                    Cow::Owned(format!("{} ({})", s.downloads, app.downloaded_tracks.len()))
                }
                SidebarEntry::ListeningStats => Cow::Borrowed(s.listening_stats),
                SidebarEntry::LocalSongs => {
                    Cow::Owned(format!("Songs ({})", app.local_scan.tracks.len()))
                }
                SidebarEntry::LocalAlbum(idx) => app
                    .local_scan
                    .albums
                    .get(*idx)
                    .map(|a| Cow::Owned(a.name.clone()))
                    .unwrap_or(Cow::Borrowed("Album")),
                SidebarEntry::LocalFolder(idx) => app
                    .local_scan
                    .folders
                    .get(*idx)
                    .map(|f| Cow::Owned(format!("{} ({})", f.name, f.tracks.len())))
                    .unwrap_or(Cow::Borrowed("Folder")),
                SidebarEntry::PersonalPlaylist(ppl) => {
                    Cow::Owned(format!("{} ({})", ppl.name, ppl.tracks.len()))
                }
                SidebarEntry::Server(srv) => {
                    let badge = app
                        .server_health
                        .get(&srv.id)
                        .map(|h| if h.alive { "●" } else { "○" })
                        .unwrap_or("◈");
                    Cow::Owned(format!("{badge} {} [{}]", srv.name, srv.kind.short_tag()))
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

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sources = app.active_library_sources();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let mut tab_spans = Vec::new();
    for (idx, src) in sources.iter().enumerate() {
        if idx > 0 {
            tab_spans.push(Span::styled(" ", Style::default().fg(app.theme.muted)));
        }
        let label = match src {
            TuiLibrarySource::YouTube => s.youtube,
            TuiLibrarySource::Local => s.local,
            TuiLibrarySource::Jellyfin => s.jellyfin,
            TuiLibrarySource::Subsonic => s.subsonic,
            TuiLibrarySource::Smb => s.smb,
            TuiLibrarySource::WebDav => s.webdav,
        };
        let active = *src == app.active_source;
        tab_spans.push(Span::styled(
            format!("[{label}]"),
            if active {
                Style::default()
                    .fg(app.theme.highlight_fg)
                    .bg(app.theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.muted)
            },
        ));
    }
    if tab_spans.is_empty() {
        tab_spans.push(Span::styled(
            "[v: Sources]",
            Style::default().fg(app.theme.muted),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).style(Style::default().bg(app.theme.surface)),
        chunks[0],
    );

    app.ui_rects.sidebar_inner = chunks[1];

    let list = List::new(items)
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg))
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

    app.sidebar_state.select(Some(app.sidebar_idx));
    frame.render_stateful_widget(list, chunks[1], &mut app.sidebar_state);
}
