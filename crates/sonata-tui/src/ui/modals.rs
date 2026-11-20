//! Modal dialogs and popups for the Sonata terminal UI.
//!
//! 1. **Settings Modal (`render_settings`)**: Displays aligned, non-truncated preference rows with keyboard/mouse selectors and a compact footer hint.
//! 2. **Library & Theme Modals**: Renders theme picker, Spotify playlist import, playlist creation/addition, image protocol warning, and keybindings help.

use crate::app::{ActiveModal, App};
use crate::theme_tui::to_ratatui_color;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;
use sonata_core::config::AppConfig;
use sonata_core::hyprland;

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

pub fn sized_popup(min_w: u16, max_w: u16, target_h: u16, area: Rect) -> Rect {
    let avail_w = area.width.saturating_sub(2).max(1);
    let clamped_max = max_w.min(avail_w).max(1);
    let clamped_min = min_w.min(clamped_max);
    let width = area.width.saturating_sub(4).clamp(clamped_min, clamped_max);
    let height = target_h.min(area.height).max(1.min(area.height));
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub fn settings_popup_rect(area: Rect, row_count: usize) -> Rect {
    let target_h = (row_count as u16 + 5).min(area.height);
    sized_popup(40, 84, target_h, area)
}

pub fn settings_label_width(row_width: u16) -> u16 {
    32.min(row_width.saturating_sub(16))
}

fn render_image_warning(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let popup = sized_popup(42, 74, 13, area);
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

    if inner.height < 3 || inner.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(2)])
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

    let labels = if inner.width >= 56 {
        [" Okay (o) ", " Don't Show Again (d) ", " Exit (q) "]
    } else {
        [" Okay (o) ", " Hide (d) ", " Quit (q) "]
    };
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
            vec![Span::styled(*label, style), Span::raw("  ")]
        })
        .collect();

    let btns = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(btns, chunks[1]);
}

fn render_theme_picker(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let target_h = (app.themes.len() as u16 + 4).clamp(8, 18);
    let popup = sized_popup(36, 72, target_h, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .themes
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::styled(" ■ ", Style::default().fg(to_ratatui_color(t.accent))),
                Span::styled(t.name.clone(), Style::default().fg(app.theme.fg)),
            ]))
        })
        .collect();

    let title = if popup.width >= 52 {
        format!(" Themes (custom: {}) ", AppConfig::themes_dir().display())
    } else {
        " Color Themes ".to_string()
    };
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
    let popup = sized_popup(40, 68, 8, area);
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
    let playlists = app.writable_ytm_playlists();
    let target_h = (playlists.len() as u16 + 4).clamp(8, 16);
    let popup = sized_popup(38, 62, target_h, area);
    frame.render_widget(Clear, popup);

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
    let popup = sized_popup(38, 60, 8, area);
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
    let transparent_label = if app.effective_transparent_bg() {
        "Enabled (Glass)"
    } else {
        "Disabled (Solid)"
    };
    let on_hyprland = hyprland::is_hyprland();
    let opacity_label = if on_hyprland {
        format!("{:.0}%", app.effective_hyprland_opacity() * 100.0)
    } else {
        "Solid (100% · Locked)".to_string()
    };

    let rows = [
        (
            "Music Providers",
            app.config.provider_mode.label().to_string(),
            true,
        ),
        (
            "YouTube Music Quality Upgrade",
            if app.config.hifi_substitution {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
            true,
        ),
        (
            "Audio Enhancer DSP",
            app.playback.enhancer.label().to_string(),
            true,
        ),
        ("Transparent TUI Bg", transparent_label.to_string(), true),
        ("Hyprland Opacity", opacity_label, on_hyprland),
        (
            "Media Cache Limit",
            AppConfig::cache_limit_label(app.config.cache_limit_gb),
            true,
        ),
        ("Color Theme", app.base_theme.name.clone(), true),
        ("Browser Session", app.browser_label.clone(), false),
    ];

    let popup = settings_popup_rect(area, rows.len());
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Settings ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let mut constraints = Vec::with_capacity(rows.len() + 3);
    constraints.push(Constraint::Length(1));
    for _ in &rows {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Length(1));

    let layout_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (index, (label, value, adjustable)) in rows.iter().enumerate() {
        let Some(row) = layout_areas.get(index + 1) else {
            break;
        };
        if row.height == 0 || row.width < 8 {
            continue;
        }
        let is_selected = index == selected;
        let row_style = if is_selected {
            Style::default().bg(app.theme.highlight_bg)
        } else {
            Style::default().bg(app.theme.surface)
        };
        frame.render_widget(Block::default().style(row_style), *row);

        let label_w = settings_label_width(row.width);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(label_w),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(*row);

        let marker = if is_selected { "▶ " } else { "  " };
        frame.render_widget(
            Paragraph::new(Span::styled(
                marker,
                row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD),
            )),
            cols[1],
        );

        let label_style = if is_selected {
            row_style
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            row_style.fg(app.theme.fg)
        };
        frame.render_widget(Paragraph::new(Span::styled(*label, label_style)), cols[2]);

        let value_line = if *adjustable {
            let arrow_style = if is_selected {
                row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD)
            } else {
                row_style.fg(app.theme.muted)
            };
            let val_style = if is_selected {
                row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD)
            } else {
                row_style.fg(app.theme.accent)
            };
            Line::from(vec![
                Span::styled("◀  ", arrow_style),
                Span::styled(value.as_str(), val_style),
                Span::styled("  ▶", arrow_style),
            ])
        } else {
            Line::from(vec![
                Span::styled(value.as_str(), row_style.fg(app.theme.muted)),
                Span::raw("   "),
            ])
        };
        frame.render_widget(
            Paragraph::new(value_line).alignment(Alignment::Right),
            cols[3],
        );
    }

    if let Some(divider_area) = layout_areas.get(rows.len() + 1) {
        if divider_area.width > 2 {
            let rule = "─".repeat(divider_area.width.saturating_sub(2) as usize);
            frame.render_widget(
                Paragraph::new(Span::styled(rule, Style::default().fg(app.theme.border)))
                    .alignment(Alignment::Center),
                *divider_area,
            );
        }
    }

    if let Some(hint_area) = layout_areas.get(rows.len() + 2) {
        let hint = if hint_area.width >= 50 {
            "↑/↓: Navigate   ←/→/Enter: Change   Esc/S: Close"
        } else {
            "↑/↓ Nav  ←/→ Change  Esc Close"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(app.theme.muted)))
                .alignment(Alignment::Center),
            *hint_area,
        );
    }
}

fn render_help(frame: &mut Frame<'_>, area: Rect, app: &App) {
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
        "Settings:    S             Providers, Hi-Fi, DSP, Transparency, Cache & Theme",
        "Keybinds:    k / ?         Show or close this Keybindings menu",
        "Mini Mode:   m / Dbl-Click Toggle Mini Mode (Art + Lyrics / Art / Mini Player)",
        "Themes:      t             Open Theme Picker (live preview)",
        "             Custom themes & transparency: ~/.config/sonata/themes/*.toml",
        "Quit:        q / Ctrl+C    Exit Sonata cleanly",
    ];

    let max_line_w = lines
        .iter()
        .map(|l| l.chars().count() as u16)
        .max()
        .unwrap_or(60);
    let target_w = (max_line_w + 4).min(96);
    let popup = sized_popup(44, target_w, lines.len() as u16 + 2, area);
    frame.render_widget(Clear, popup);

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

#[cfg(test)]
mod tests {
    use super::{settings_label_width, settings_popup_rect, sized_popup};
    use ratatui::layout::Rect;

    #[test]
    fn settings_modal_is_widened_and_sized_tightly_to_rows() {
        let screen = Rect::new(0, 0, 120, 40);
        let popup = settings_popup_rect(screen, 8);
        assert_eq!(popup.width, 84);
        assert_eq!(popup.height, 13);

        let inner_w = popup.width - 2;
        assert_eq!(settings_label_width(inner_w), 32);
        assert!(settings_label_width(inner_w) >= "YouTube Music Quality Upgrade".len() as u16);

        let medium = Rect::new(0, 0, 80, 24);
        let med_popup = settings_popup_rect(medium, 8);
        assert_eq!(med_popup.width, 76);
        assert_eq!(med_popup.height, 13);
        assert!(
            settings_label_width(med_popup.width - 2)
                >= "YouTube Music Quality Upgrade".len() as u16
        );
    }

    #[test]
    fn modals_clamp_safely_inside_small_terminals() {
        let tiny = Rect::new(0, 0, 30, 10);
        let settings = settings_popup_rect(tiny, 8);
        assert!(settings.width <= tiny.width);
        assert!(settings.height <= tiny.height);
        assert_eq!(settings_label_width(settings.width.saturating_sub(2)), 10);

        let help = sized_popup(44, 96, 19, tiny);
        assert!(help.width <= tiny.width);
        assert!(help.height <= tiny.height);
    }
}
