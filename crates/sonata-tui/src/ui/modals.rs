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
use sonata_core::updater::CURRENT_VERSION;
use sonata_runtime::config::AppConfig;
use sonata_runtime::hyprland;

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

pub const SETTINGS_ROW_COUNT: usize = 12;

#[cfg_attr(not(test), allow(dead_code))]
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
    let s = sonata_runtime::i18n::strings(app.config.language);
    let popup = sized_popup(40, 68, 8, area);
    frame.render_widget(Clear, popup);

    let text = format!(
        "Paste a Spotify playlist URL or ID (no API key required):\n\n> {input}▏\n\nEnter: {}   Esc: {}",
        s.create, s.cancel
    );
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(s.tui_import_spotify_title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.success))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_add_to_playlist(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let s = sonata_runtime::i18n::strings(app.config.language);
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
                .title(s.tui_add_to_playlist_title)
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
    let s = sonata_runtime::i18n::strings(app.config.language);
    let popup = sized_popup(38, 60, 8, area);
    frame.render_widget(Clear, popup);

    let text = format!(
        "{}:\n\n> {input}▏\n\nEnter: {}   Esc: {}",
        s.new_playlist, s.create, s.cancel
    );
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(s.tui_create_playlist_title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

enum SettingValueKind {
    Toggle(bool, &'static str, &'static str),
    Text(String),
    OpacityBar(f32, bool),
    ThemeSwatch(String),
    UpdateBadge(String, bool),
}

struct SettingEntry {
    section: Option<&'static str>,
    label: &'static str,
    description: &'static str,
    value: SettingValueKind,
    adjustable: bool,
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let selected = selected.min(SETTINGS_ROW_COUNT - 1);
    let transparent = app.effective_transparent_bg();
    let on_hyprland = hyprland::is_hyprland();
    let opacity = app.effective_hyprland_opacity();

    let (update_label, has_update) = match &app.update_info {
        Some(info) if info.update_available => (
            format!("v{CURRENT_VERSION} → v{}", info.latest_version),
            true,
        ),
        Some(_) => (format!("v{CURRENT_VERSION} · {}", s.up_to_date), false),
        None => (
            format!("v{CURRENT_VERSION} · {}", s.check_update_hint),
            false,
        ),
    };

    let entries = [
        SettingEntry {
            section: Some(s.tui_sec_audio),
            label: s.tui_providers,
            description: s.tui_providers_desc,
            value: SettingValueKind::Text(app.config.provider_mode.label().to_string()),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.hifi_upgrade,
            description: s.hifi_upgrade_desc,
            value: SettingValueKind::Toggle(app.config.hifi_substitution, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.jiosaavn_module,
            description: s.jiosaavn_module_desc,
            value: SettingValueKind::Toggle(
                app.config.jiosaavn_enabled,
                s.enabled_320k,
                s.disabled,
            ),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.autoplay,
            description: s.autoplay_desc,
            value: SettingValueKind::Toggle(app.config.autoplay, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: Some(s.tui_sec_appearance),
            label: s.language,
            description: s.language_desc,
            value: SettingValueKind::Text(app.config.language.label().to_string()),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.color_theme,
            description: s.color_theme_desc,
            value: SettingValueKind::ThemeSwatch(app.base_theme.name.clone()),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.motion_covers,
            description: s.motion_covers_desc,
            value: SettingValueKind::Toggle(
                app.config.motion_covers && !app.config.reduce_motion,
                s.enabled,
                s.static_label,
            ),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.tui_transparent_bg,
            description: s.tui_transparent_bg_desc,
            value: SettingValueKind::Toggle(transparent, s.enabled_glass, s.solid_label),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.tui_hypr_opacity,
            description: s.tui_hypr_opacity_desc,
            value: SettingValueKind::OpacityBar(opacity, on_hyprland),
            adjustable: on_hyprland,
        },
        SettingEntry {
            section: Some(s.tui_sec_storage),
            label: s.cache_limit,
            description: s.cache_limit_desc,
            value: SettingValueKind::Text(AppConfig::cache_limit_label(app.config.cache_limit_gb)),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.tui_browser_session,
            description: s.tui_browser_session_desc,
            value: SettingValueKind::Text(app.browser_label.clone()),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.tui_version_updates,
            description: s.tui_version_updates_desc,
            value: SettingValueKind::UpdateBadge(update_label, has_update),
            adjustable: true,
        },
    ];

    // Total inner rows: 3 section headers + 11 items + 2 spacers + divider + description + footer.
    let target_h = (entries.len() as u16 + 11).min(area.height);
    let popup = sized_popup(44, 86, target_h, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(s.tui_settings_title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 4 || inner.width < 12 {
        return;
    }

    let show_desc = inner.height >= 20;
    let show_sections = inner.height >= 17;
    let bottom_reserved: u16 = if show_desc { 4 } else { 2 };
    let list_avail_h = inner.height.saturating_sub(bottom_reserved);

    // Flatten visual lines (optional section headers + setting rows), tracking selected visual row.
    enum VisualRow<'a> {
        Section(&'static str),
        Setting(usize, &'a SettingEntry),
    }

    let mut visual_rows: Vec<VisualRow<'_>> = Vec::with_capacity(entries.len() + 3);
    let mut selected_visual_idx = 0usize;
    for (idx, entry) in entries.iter().enumerate() {
        if show_sections {
            if let Some(sec) = entry.section {
                visual_rows.push(VisualRow::Section(sec));
            }
        }
        if idx == selected {
            selected_visual_idx = visual_rows.len();
        }
        visual_rows.push(VisualRow::Setting(idx, entry));
    }

    let max_rows = list_avail_h as usize;
    let scroll_start = if visual_rows.len() <= max_rows || selected_visual_idx < max_rows / 2 {
        0
    } else {
        (selected_visual_idx - max_rows / 2).min(visual_rows.len() - max_rows)
    };

    for (v_offset, v_row) in visual_rows
        .iter()
        .skip(scroll_start)
        .take(max_rows)
        .enumerate()
    {
        let row_rect = Rect {
            x: inner.x,
            y: inner.y + v_offset as u16,
            width: inner.width,
            height: 1,
        };
        match v_row {
            VisualRow::Section(title) => {
                let title_len = title.chars().count();
                let rule_w = row_rect.width.saturating_sub(title_len as u16 + 6) as usize;
                let line = Line::from(vec![
                    Span::styled(" ── ", Style::default().fg(app.theme.border)),
                    Span::styled(
                        *title,
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {}", "─".repeat(rule_w)),
                        Style::default().fg(app.theme.border),
                    ),
                ]);
                frame.render_widget(Paragraph::new(line), row_rect);
            }
            VisualRow::Setting(index, entry) => {
                let is_selected = *index == selected;
                let row_style = if is_selected {
                    Style::default().bg(app.theme.highlight_bg)
                } else {
                    Style::default().bg(app.theme.surface)
                };
                frame.render_widget(Block::default().style(row_style), row_rect);

                let label_w = settings_label_width(row_rect.width);
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Length(2),
                        Constraint::Length(label_w),
                        Constraint::Min(10),
                        Constraint::Length(1),
                    ])
                    .split(row_rect);

                let marker = if is_selected { "▎" } else { " " };
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        marker,
                        row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD),
                    )),
                    cols[0],
                );
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        if is_selected { "▶" } else { "·" },
                        row_style.fg(if is_selected {
                            app.theme.accent
                        } else {
                            app.theme.muted
                        }),
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
                frame.render_widget(
                    Paragraph::new(Span::styled(entry.label, label_style)),
                    cols[2],
                );

                let arrow_style = if is_selected {
                    row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD)
                } else {
                    row_style.fg(app.theme.muted)
                };

                let mut val_spans: Vec<Span<'_>> = Vec::new();
                if entry.adjustable && is_selected {
                    val_spans.push(Span::styled("◀ ", arrow_style));
                }
                match &entry.value {
                    SettingValueKind::Toggle(enabled, on_lbl, off_lbl) => {
                        let (dot, lbl, color) = if *enabled {
                            ("● ", *on_lbl, app.theme.success)
                        } else {
                            ("○ ", *off_lbl, app.theme.muted)
                        };
                        val_spans.push(Span::styled(
                            dot,
                            row_style.fg(color).add_modifier(Modifier::BOLD),
                        ));
                        val_spans.push(Span::styled(
                            lbl,
                            row_style
                                .fg(if *enabled { color } else { app.theme.muted })
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ));
                    }
                    SettingValueKind::Text(text) => {
                        let color = if entry.adjustable {
                            app.theme.accent
                        } else {
                            app.theme.muted
                        };
                        val_spans.push(Span::styled(
                            text.clone(),
                            row_style.fg(color).add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                        ));
                    }
                    SettingValueKind::ThemeSwatch(name) => {
                        val_spans.push(Span::styled("█", row_style.fg(app.theme.accent)));
                        val_spans.push(Span::styled("█", row_style.fg(app.theme.success)));
                        val_spans.push(Span::styled("█ ", row_style.fg(app.theme.highlight_fg)));
                        val_spans.push(Span::styled(
                            name.clone(),
                            row_style.fg(app.theme.accent).add_modifier(Modifier::BOLD),
                        ));
                    }
                    SettingValueKind::OpacityBar(val, unlocked) => {
                        if *unlocked {
                            let pct = (val * 100.0).round() as u32;
                            let steps = ((val - 0.5).clamp(0.0, 0.5) / 0.5 * 8.0).round() as usize;
                            let fill = "━".repeat(steps.min(8));
                            let rest = "─".repeat(8usize.saturating_sub(steps.min(8)));
                            val_spans.push(Span::styled(fill, row_style.fg(app.theme.accent)));
                            val_spans.push(Span::styled("●", row_style.fg(app.theme.highlight_fg)));
                            val_spans.push(Span::styled(rest, row_style.fg(app.theme.border)));
                            val_spans.push(Span::styled(
                                format!(" {pct}%"),
                                row_style.fg(app.theme.fg).add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            val_spans.push(Span::styled(
                                "Solid (100% · Locked)",
                                row_style.fg(app.theme.muted),
                            ));
                        }
                    }
                    SettingValueKind::UpdateBadge(text, is_new) => {
                        let color = if *is_new {
                            app.theme.warning
                        } else {
                            app.theme.success
                        };
                        val_spans.push(Span::styled(
                            text.clone(),
                            row_style.fg(color).add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                if entry.adjustable && is_selected {
                    val_spans.push(Span::styled(" ▶", arrow_style));
                }

                frame.render_widget(
                    Paragraph::new(Line::from(val_spans)).alignment(Alignment::Right),
                    cols[3],
                );
            }
        }
    }

    let footer_start_y = inner.y + inner.height.saturating_sub(bottom_reserved);
    let divider_rect = Rect {
        x: inner.x,
        y: footer_start_y,
        width: inner.width,
        height: 1,
    };
    if divider_rect.width > 2 {
        let rule = "─".repeat(divider_rect.width.saturating_sub(2) as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(rule, Style::default().fg(app.theme.border)))
                .alignment(Alignment::Center),
            divider_rect,
        );
    }

    if show_desc {
        let desc_rect = Rect {
            x: inner.x + 1,
            y: footer_start_y + 1,
            width: inner.width.saturating_sub(2),
            height: 2,
        };
        let desc_text = entries.get(selected).map(|e| e.description).unwrap_or("");
        frame.render_widget(
            Paragraph::new(Span::styled(
                desc_text,
                Style::default().fg(app.theme.muted),
            ))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center),
            desc_rect,
        );
    }

    let hint_rect = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let hint = if hint_rect.width >= 66 {
        "↑/↓ Navigate   ←/→/Enter Toggle   u Check Updates   r Rescan Browser   Esc Close"
    } else {
        "↑/↓ Nav  ←/→/Enter Change  u Update  Esc Close"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(app.theme.muted)))
            .alignment(Alignment::Center),
        hint_rect,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = vec![
        "Navigation:  Tab / h / l   Switch pane  |  ↑/↓ / j   Move up/down",
        "Resize:      H / L         Resize pane  |  Mouse Drag Drag pane borders",
        "Search:      /             Search       |  Enter     Play",
        "Playback:    Space         Play/Pause   |  n / p     Next / Prev",
        "Seek/Vol:    < / >         Seek ±5s     |  - / +     Volume ±5%",
        "Queue Modes: s             Shuffle      |  r         Repeat (Off/All/One)",
        "YTM Sync:    y             Like / Unlike on YouTube Music account",
        "             a / x (Del)   Add to / Remove from YTM playlist",
        "             c / R         Create YTM playlist / Refresh account",
        "Spotify:     i             Import any Spotify playlist URL",
        "Settings:    S             Providers, Hi-Fi, Transparency, Cache & Theme",
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
