//! Modal dialogs and popups for the Sonata terminal UI.
//!
//! 1. **Settings Modal (`render_settings`)**: Displays aligned, non-truncated preference rows with keyboard/mouse selectors and a compact footer hint.
//! 2. **Library & Theme Modals**: Renders theme picker, Spotify playlist import, playlist creation/addition, image protocol warning, and keybindings help.

use crate::app::{
    format_tui_health_badge, format_tui_speed_badge, ActiveModal, App, ServerBrowserState,
    ServerBrowserTab, ServerFormField, ServerFormState, TuiPlaylistAddTarget,
};
use crate::theme_tui::to_ratatui_color;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;
use sonata_core::providers::servers::ServerKind;
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
        ActiveModal::CreatePlaylist { buf, personal } => {
            render_create_playlist(frame, area, app, buf, *personal)
        }
        ActiveModal::RenamePersonalPlaylist { buf, .. } => {
            render_rename_personal_playlist(frame, area, app, buf)
        }
        ActiveModal::Settings(selected) => render_settings(frame, area, app, *selected),
        ActiveModal::TextInput {
            title, prompt, buf, ..
        } => render_text_input(frame, area, app, title, prompt, buf),
        ActiveModal::LocalMusicManager(selected) => {
            render_local_music_manager(frame, area, app, *selected)
        }
        ActiveModal::ServerManager(selected) => render_server_manager(frame, area, app, *selected),
        ActiveModal::AddServer(form) => render_add_server(frame, area, app, form),
        ActiveModal::ServerBrowser(st) => render_server_browser(frame, area, app, st),
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

pub const SETTINGS_ROW_COUNT: usize = 26;

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
    let targets = app.playlist_add_targets_for_selected();
    let target_h = (targets.len() as u16 + 4).clamp(8, 16);
    let popup = sized_popup(42, 68, target_h, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = targets
        .iter()
        .map(|target| match target {
            TuiPlaylistAddTarget::Personal(ppl) => ListItem::new(Line::from(vec![
                Span::styled(" ★ [Personal] ", Style::default().fg(app.theme.accent)),
                Span::styled(ppl.name.clone(), Style::default().fg(app.theme.fg)),
            ])),
            TuiPlaylistAddTarget::Ytm(p) => ListItem::new(Line::from(vec![
                Span::styled(" + [YTM]      ", Style::default().fg(app.theme.muted)),
                Span::styled(p.name.clone(), Style::default().fg(app.theme.fg)),
            ])),
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
    if !targets.is_empty() {
        state.select(Some(selected.min(targets.len() - 1)));
    }
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_create_playlist(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    input: &str,
    personal: bool,
) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let popup = sized_popup(42, 64, 9, area);
    frame.render_widget(Clear, popup);

    let mode_label = if personal {
        s.personal_playlists
    } else {
        s.youtube
    };
    let switch_hint = if !app.config.personal_only_mode && app.ytm.is_authenticated() {
        "   Tab: Switch"
    } else {
        ""
    };
    let text = format!(
        "[{}] {}:\n\n> {input}▏\n\nEnter: {}   Esc: {}{}",
        mode_label, s.new_playlist, s.create, s.cancel, switch_hint
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

fn render_rename_personal_playlist(frame: &mut Frame<'_>, area: Rect, app: &App, input: &str) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let popup = sized_popup(42, 64, 8, area);
    frame.render_widget(Clear, popup);

    let text = format!(
        "{}:\n\n> {input}▏\n\nEnter: {}   Esc: {}",
        s.rename, s.save, s.cancel
    );
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(format!(" {} ", s.rename))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_local_music_manager(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let dirs = app.config.effective_local_music_dirs();
    let target_h = (dirs.len() as u16 + 7).clamp(10, 18).min(area.height);
    let popup = sized_popup(48, 84, target_h, area);
    frame.render_widget(Clear, popup);

    let status_tag = if !app.config.local_music_enabled {
        s.disabled.to_string()
    } else if app.local_scanning {
        "Scanning...".to_string()
    } else {
        format!(
            "{} tracks · {} albums",
            app.local_scan.tracks.len(),
            app.local_scan.albums.len()
        )
    };
    let block = Block::default()
        .title(format!(" {} ({}) ", s.local_music, status_tag))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(2)])
        .split(inner);

    if dirs.is_empty() {
        let p = Paragraph::new("No folders configured. Press 'a' to add a music folder.")
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(app.theme.muted));
        frame.render_widget(p, chunks[0]);
    } else {
        let sel = selected.min(dirs.len().saturating_sub(1));
        let items: Vec<ListItem> = dirs
            .iter()
            .map(|dir| ListItem::new(format!("▤ {}", dir.to_string_lossy())))
            .collect();
        let mut state = ListState::default();
        state.select(Some(sel));
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(app.theme.highlight_bg)
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");
        frame.render_stateful_widget(list, chunks[0], &mut state);
    }

    let hint = Paragraph::new("Enter: Toggle Local  a: Add folder  s: Scan  d: Remove  Esc: Back")
        .style(Style::default().fg(app.theme.muted))
        .wrap(Wrap { trim: false });
    frame.render_widget(hint, chunks[1]);
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

    let dl_limit_text = if app.config.downloads_limit_gb == 0 {
        s.unlimited.to_string()
    } else {
        AppConfig::downloads_limit_label(app.config.downloads_limit_gb)
    };
    let dl_dir_text = app
        .config
        .effective_downloads_dir()
        .to_string_lossy()
        .into_owned();
    let dl_usage_text = format!(
        "{:.1} MB · {} {}",
        app.downloads_total_bytes as f64 / (1024.0 * 1024.0),
        app.downloaded_tracks.len(),
        s.tracks
    );
    let discord_app_text = app
        .config
        .discord_app_id
        .clone()
        .unwrap_or_else(|| "Default".to_string());
    let servers_text = if app.server_profiles.is_empty() {
        "None (Enter to add)".to_string()
    } else {
        format!("{} configured (Enter)", app.server_profiles.len())
    };
    let local_text = if !app.config.local_music_enabled {
        s.disabled.to_string()
    } else if app.local_scanning {
        "Scanning...".to_string()
    } else {
        format!(
            "{} {} · {} {}",
            app.config.effective_local_music_dirs().len(),
            s.folders,
            app.local_scan.tracks.len(),
            s.tracks
        )
    };
    let jio_status = if app.config.personal_only_mode && app.config.jiosaavn_enabled {
        s.off
    } else {
        s.enabled_320k
    };

    let entries = [
        SettingEntry {
            section: Some(s.sources),
            label: s.personal_only_mode,
            description: s.personal_only_mode_desc,
            value: SettingValueKind::Toggle(app.config.personal_only_mode, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.jiosaavn_module,
            description: s.jiosaavn_module_desc,
            value: SettingValueKind::Toggle(app.config.jiosaavn_enabled, jio_status, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.attribute_personal_to_ytm,
            description: s.attribute_personal_to_ytm_desc,
            value: SettingValueKind::Toggle(
                app.config.scrobble_servers_to_ytm,
                s.enabled,
                s.disabled,
            ),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.local_music,
            description: s.local_music_desc,
            value: SettingValueKind::Text(local_text),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.servers,
            description: s.servers_desc,
            value: SettingValueKind::Text(servers_text),
            adjustable: true,
        },
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
            label: s.autoplay,
            description: s.autoplay_desc,
            value: SettingValueKind::Toggle(app.config.autoplay, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.playback_speed,
            description: s.playback_speed_desc,
            value: SettingValueKind::Text(format!("{:.2}x", app.playback_speed)),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.skip_silence,
            description: s.skip_silence_desc,
            value: SettingValueKind::Toggle(app.config.skip_silence, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.sleep_timer,
            description: s.sleep_timer_desc,
            value: SettingValueKind::Text(match app.sleep_timer_remaining_secs() {
                Some(secs) => crate::app::playback::format_sleep_countdown(secs),
                None => s.off.to_string(),
            }),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.translate_lyrics,
            description: s.translate_lyrics_desc,
            value: SettingValueKind::Toggle(app.config.lyrics_translate, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.romanize_lyrics,
            description: s.romanize_lyrics_desc,
            value: SettingValueKind::Toggle(app.config.lyrics_romanize, s.enabled, s.disabled),
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
            label: s.downloads_limit,
            description: s.downloads_limit_desc,
            value: SettingValueKind::Text(dl_limit_text),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.downloads_dir,
            description: s.downloads_dir_desc,
            value: SettingValueKind::Text(dl_dir_text),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.clear_downloads,
            description: s.downloads_usage,
            value: SettingValueKind::Text(dl_usage_text),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.discord_rpc,
            description: s.discord_rpc_desc,
            value: SettingValueKind::Toggle(app.config.discord_rpc_enabled, s.enabled, s.disabled),
            adjustable: true,
        },
        SettingEntry {
            section: None,
            label: s.discord_app_id,
            description: s.discord_app_id_desc,
            value: SettingValueKind::Text(discord_app_text),
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
        "Speed/Seek:  [ / ]         Speed ±step  |  < / >     Seek ±5s",
        "Volume:      - / +         Volume ±5%   |  T / O     Translate / Romanize",
        "Queue Modes: s             Shuffle      |  r         Repeat (Off/All/One)",
        "YTM Sync:    y             Like / Unlike on YouTube Music account",
        "             a / x (Del)   Add to / Remove from playlist (Personal/YTM)",
        "             c / e / R     Create playlist / Rename Personal / Refresh",
        "Sources:     v             Switch active Library source tab",
        "Spotify:     i             Import any Spotify playlist URL",
        "Offline:     d / D         Download/remove track or collection",
        "Settings:    S             Providers, Downloads, Servers, Discord & Theme",
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

fn render_text_input(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    title: &str,
    prompt: &str,
    buf: &str,
) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let popup = sized_popup(42, 72, 8, area);
    frame.render_widget(Clear, popup);

    let text = format!(
        "{prompt}\n\n> {buf}▏\n\nEnter: {}   Esc: {}",
        s.save, s.cancel
    );
    let p = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(format!(" {title} "))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_server_manager(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let item_count = app.server_profiles.len().max(1);
    let target_h = (item_count as u16 + 6).clamp(9, 18).min(area.height);
    let popup = sized_popup(48, 84, target_h, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {} ", s.servers))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(2)])
        .split(inner);

    if app.server_profiles.is_empty() {
        let p = Paragraph::new(s.no_servers)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(app.theme.muted));
        frame.render_widget(p, chunks[0]);
    } else {
        let sel = selected.min(app.server_profiles.len().saturating_sub(1));
        let items: Vec<ListItem> = app
            .server_profiles
            .iter()
            .map(|srv| {
                let state_tag = if srv.enabled { "ON" } else { "OFF" };
                let health = app
                    .server_health
                    .get(&srv.id)
                    .map(format_tui_health_badge)
                    .or_else(|| app.server_statuses.get(&srv.id).cloned())
                    .map(|st| format!(" · {st}"))
                    .unwrap_or_default();
                let speed = if app.server_speed_testing.contains(&srv.id) {
                    " · Testing speed...".to_string()
                } else {
                    app.server_speed_tests
                        .get(&srv.id)
                        .map(|sp| format!(" · {}", format_tui_speed_badge(sp)))
                        .unwrap_or_default()
                };
                let loc = if srv.base_url.trim().is_empty() {
                    srv.local_mount_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    srv.base_url.clone()
                };
                ListItem::new(format!(
                    "[{}] {} ({}) {}{}{}",
                    srv.kind.short_tag(),
                    srv.name,
                    state_tag,
                    loc,
                    health,
                    speed
                ))
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(sel));
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(app.theme.highlight_bg)
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");
        frame.render_stateful_widget(list, chunks[0], &mut state);
    }

    let hint = Paragraph::new(
        "a: Add  e: Edit  t: Health  s: Speed  Enter: Toggle  b: Browse  d: Del  Esc",
    )
    .style(Style::default().fg(app.theme.muted))
    .wrap(Wrap { trim: false });
    frame.render_widget(hint, chunks[1]);
}

fn render_add_server(frame: &mut Frame<'_>, area: Rect, app: &App, form: &ServerFormState) {
    let s = sonata_runtime::i18n::strings(app.config.language);
    let target_h = if form.kind == ServerKind::Smb { 18 } else { 15 };
    let popup = sized_popup(50, 80, target_h, area);
    frame.render_widget(Clear, popup);

    let kind_label = match form.kind {
        ServerKind::Jellyfin => "Jellyfin / Emby",
        ServerKind::Subsonic => "Subsonic / Navidrome",
        ServerKind::Smb => "SMB / CIFS Share",
        ServerKind::WebDav => "WebDAV",
    };
    let mask = "*".repeat(form.password.chars().count());
    let mut rows = vec![
        (ServerFormField::Kind, "Protocol", kind_label),
        (ServerFormField::Name, "Name", form.name.as_str()),
        (ServerFormField::BaseUrl, "Base URL", form.base_url.as_str()),
    ];
    if form.kind == ServerKind::Smb {
        rows.push((ServerFormField::Share, s.smb_share, form.share.as_str()));
        rows.push((ServerFormField::Domain, s.smb_domain, form.domain.as_str()));
        rows.push((
            ServerFormField::MountPath,
            s.smb_mount_path,
            form.mount_path.as_str(),
        ));
    }
    rows.push((
        ServerFormField::Username,
        "Username",
        form.username.as_str(),
    ));
    rows.push((ServerFormField::Password, "Password", mask.as_str()));
    if form.kind != ServerKind::Smb {
        rows.push((
            ServerFormField::Token,
            "API Key / Token",
            form.token.as_str(),
        ));
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(rows.len() + 4);
    for (field, label, val) in rows {
        let active = form.focused == field;
        let cursor = if active && field != ServerFormField::Kind {
            "▏"
        } else {
            ""
        };
        let label_style = if active {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.muted)
        };
        lines.push(Line::from(vec![
            Span::styled(if active { "▸ " } else { "  " }, label_style),
            Span::styled(format!("{label:<16} "), label_style),
            Span::styled(format!("{val}{cursor}"), Style::default().fg(app.theme.fg)),
        ]));
    }
    if let Some(ref status) = form.status {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            status.clone(),
            Style::default().fg(app.theme.accent),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab/↑↓: Field  ←/→: Protocol  Ctrl+T: Test  Enter: Save  Esc: Cancel",
        Style::default().fg(app.theme.muted),
    )));

    let modal_title = if form.editing_id.is_some() {
        format!(" {} {} ", s.edit, s.servers)
    } else {
        format!(" {} ", s.add_server)
    };
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(modal_title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface).fg(app.theme.fg)),
    );
    frame.render_widget(p, popup);
}

fn render_server_browser(frame: &mut Frame<'_>, area: Rect, app: &App, st: &ServerBrowserState) {
    let popup = sized_popup(52, 88, 20, area);
    frame.render_widget(Clear, popup);

    let title = format!(" {} [{}] ", st.server_name, st.server_kind.short_tag());
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .split(inner);

    let tabs = [
        (ServerBrowserTab::Tracks, "Tracks"),
        (ServerBrowserTab::Playlists, "Playlists"),
        (ServerBrowserTab::Albums, "Albums"),
        (ServerBrowserTab::Artists, "Artists"),
        (
            ServerBrowserTab::Libraries,
            if matches!(st.server_kind, ServerKind::WebDav | ServerKind::Smb) {
                "Folders"
            } else {
                "Libraries"
            },
        ),
    ];
    let tab_spans: Vec<Span<'static>> = tabs
        .iter()
        .map(|(tab, label)| {
            if *tab == st.tab {
                Span::styled(
                    format!(" [{label}] "),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(format!("  {label}  "), Style::default().fg(app.theme.muted))
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

    if app.server_loading {
        frame.render_widget(
            Paragraph::new("Loading server library...").style(Style::default().fg(app.theme.muted)),
            chunks[1],
        );
    } else if let Some(ref err) = app.server_error {
        frame.render_widget(
            Paragraph::new(format!("Error: {err}"))
                .style(Style::default().fg(app.theme.error))
                .wrap(Wrap { trim: false }),
            chunks[1],
        );
    } else {
        let labels: Vec<String> = match st.tab {
            ServerBrowserTab::Tracks => app
                .server_tracks
                .iter()
                .map(|t| format!("{} — {}", t.title, t.artist))
                .collect(),
            ServerBrowserTab::Playlists => app
                .server_playlists
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            ServerBrowserTab::Albums => app
                .server_albums
                .iter()
                .map(|a| format!("{} — {}", a.title, a.subtitle))
                .collect(),
            ServerBrowserTab::Artists => {
                app.server_artists.iter().map(|a| a.title.clone()).collect()
            }
            ServerBrowserTab::Libraries => app
                .server_libraries
                .iter()
                .map(|l| format!("{} ({})", l.name, l.kind))
                .collect(),
        };
        if labels.is_empty() {
            frame.render_widget(
                Paragraph::new("No items in this section.")
                    .style(Style::default().fg(app.theme.muted)),
                chunks[1],
            );
        } else {
            let sel = st.selected.min(labels.len().saturating_sub(1));
            let items: Vec<ListItem> = labels.into_iter().map(ListItem::new).collect();
            let mut state = ListState::default();
            state.select(Some(sel));
            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(app.theme.highlight_bg)
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▸ ");
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }
    }

    frame.render_widget(
        Paragraph::new("←/→/Tab: Section  ↑/↓: Select  Enter: Open/Play  r: Refresh  Esc: Close")
            .style(Style::default().fg(app.theme.muted)),
        chunks[2],
    );
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
