use crate::app::{ActiveModal, App};
use crate::config::AppConfig;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match &app.modal {
        ActiveModal::None => {}
        ActiveModal::ImageWarning(selected) => render_image_warning(frame, area, app, *selected),
        ActiveModal::ThemePicker(selected, _) => render_theme_picker(frame, area, app, *selected),
        ActiveModal::SpotifyImport(input) => render_spotify_import(frame, area, app, input),
        ActiveModal::AddToPlaylist(selected) => render_add_to_playlist(frame, area, app, *selected),
        ActiveModal::CreatePlaylist(input) => render_create_playlist(frame, area, app, input),
        ActiveModal::Settings(selected) => render_settings(frame, area, app, *selected),
        ActiveModal::Help => render_help(frame, area, app),
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height.min(area.height)),
            Constraint::Fill(1),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

fn render_image_warning(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let popup = centered_rect(64, 13, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Terminal Image Protocol Notice ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.warning))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(3)])
        .split(inner);

    let body = Paragraph::new(
        "Your current terminal does not advertise Kitty, Sixel, or iTerm2 image support.\n\
         Album covers will fall back to coarse Unicode half-blocks.\n\n\
         For crisp HD album artwork, use Ghostty, Kitty, WezTerm, or Foot.",
    )
    .wrap(Wrap { trim: true })
    .alignment(Alignment::Center)
    .style(Style::default().fg(app.theme.fg));
    frame.render_widget(body, chunks[0]);

    let labels = [" Okay (o) ", " Don't Show Again (d) ", " Exit (q) "];
    let spans: Vec<Span> = labels
        .iter()
        .enumerate()
        .flat_map(|(i, label)| {
            let style = if i == selected {
                Style::default()
                    .bg(app.theme.accent)
                    .fg(match app.theme.bg {
                        Color::Reset => Color::Black,
                        c => c,
                    })
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(app.theme.highlight_bg).fg(app.theme.fg)
            };
            vec![Span::styled(*label, style), Span::raw("   ")]
        })
        .collect();

    let btns = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(btns, chunks[1]);
}

fn render_theme_picker(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let popup = centered_rect(48, 16, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .themes
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::styled(" ■ ", Style::default().fg(t.accent)),
                Span::styled(t.name.clone(), Style::default().fg(app.theme.fg)),
            ]))
        })
        .collect();

    let title = format!(" Themes (custom: {}) ", AppConfig::themes_dir().display());
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.accent))
                .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_spotify_import(frame: &mut Frame<'_>, area: Rect, app: &App, input: &str) {
    let popup = centered_rect(60, 8, area);
    frame.render_widget(Clear, popup);

    let text = format!(
        "Paste a Spotify playlist URL or ID (no API key required):\n\n> {input}▏\n\nEnter: Import   Esc: Cancel"
    );
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(" Import Spotify Playlist ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.success))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_add_to_playlist(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let popup = centered_rect(50, 14, area);
    frame.render_widget(Clear, popup);

    let playlists = app.writable_ytm_playlists();
    let items: Vec<ListItem> = playlists
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(" + ", Style::default().fg(app.theme.accent)),
                Span::styled(p.name.clone(), Style::default().fg(app.theme.fg)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Add to YouTube Music Playlist ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.accent))
                .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_create_playlist(frame: &mut Frame<'_>, area: Rect, app: &App, input: &str) {
    let popup = centered_rect(54, 8, area);
    frame.render_widget(Clear, popup);

    let text =
        format!("New YouTube Music playlist title:\n\n> {input}▏\n\nEnter: Create   Esc: Cancel");
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(" Create YouTube Music Playlist ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let popup = centered_rect(64, 14, area);
    frame.render_widget(Clear, popup);

    let transparent_label = if app.effective_transparent_bg() {
        "Enabled (Glass)"
    } else {
        "Disabled (Solid)"
    };
    let opacity_label = if crate::hyprland::is_hyprland() {
        format!("{:.0}%", app.effective_hyprland_opacity() * 100.0)
    } else {
        format!(
            "{:.0}% (Hyprland only)",
            app.effective_hyprland_opacity() * 100.0
        )
    };

    let rows = [
        (
            "Music Providers",
            app.config.provider_mode.label().to_string(),
        ),
        (
            "Hi-Fi 320kbps Mode",
            if app.config.hifi_substitution {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
        (
            "Audio Enhancer DSP",
            app.playback.enhancer.label().to_string(),
        ),
        ("Transparent TUI Bg", transparent_label.to_string()),
        ("Hyprland Opacity", opacity_label),
        ("Color Theme", app.base_theme.name.clone()),
    ];

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, (label, val))| {
            let prefix = if i == selected { " ▶ " } else { "   " };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(app.theme.accent)),
                Span::styled(format!("{label:<22}"), Style::default().fg(app.theme.fg)),
                Span::styled(
                    format!("◀  {val}  ▶"),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Settings (←/→/Enter: Change · Esc: Close) ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.accent))
                .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_help(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(68, 22, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        "Navigation:  Tab / h / l   Switch pane  |  ↑/↓ / j   Move up/down",
        "Resize:      H / L         Resize pane  |  Mouse Drag Drag pane borders",
        "Search:      /             Search       |  Enter     Play",
        "Playback:    Space         Play/Pause   |  n / p     Next / Prev",
        "Seek/Vol:    < / >         Seek ±5s     |  - / +     Volume ±5%",
        "Audio DSP:   e             Cycle Enhancer (Hi-Res Crystal / Spatial / Bass / Bit-Perfect)",
        "Queue Modes: s             Shuffle      |  r         Repeat (Off/All/One)",
        "YTM Sync:    y             Like / Unlike on YouTube Music account",
        "             a / x (Del)   Add to / Remove from YTM playlist",
        "             c / R         Create YTM playlist / Refresh account",
        "Spotify:     i             Import any Spotify playlist URL",
        "Settings:    S             Providers, Hi-Fi, DSP, Transparency, Hyprland Opacity & Theme",
        "Keybinds:    k / ?         Show or close this Keybindings menu",
        "Mini Mode:   m / Dbl-Click Toggle Mini Mode (Art + Lyrics / Art / Mini Player)",
        "Themes:      t             Open Theme Picker (live preview)",
        "             Custom themes & transparency: ~/.config/sonata/themes/*.toml",
        "Quit:        q / Ctrl+C    Exit Sonata cleanly",
    ];

    let p = Paragraph::new(lines.join("\n"))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(" Sonata Keybindings (Esc, k, or ? to close) ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.accent))
                .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
        );
    frame.render_widget(p, popup);
}
