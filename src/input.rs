use crate::app::{ActiveModal, App, FocusPane};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    let writable_pl_count = app.writable_ytm_playlists().len();
    match &mut app.modal {
        ActiveModal::ImageWarning(sel) => {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => *sel = (*sel + 2) % 3,
                KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => *sel = (*sel + 1) % 3,
                KeyCode::Char('o') => app.modal = ActiveModal::None,
                KeyCode::Char('d') => {
                    app.config.suppress_image_warning = true;
                    let _ = app.config.save();
                    app.modal = ActiveModal::None;
                }
                KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                KeyCode::Enter => match *sel {
                    0 => app.modal = ActiveModal::None,
                    1 => {
                        app.config.suppress_image_warning = true;
                        let _ = app.config.save();
                        app.modal = ActiveModal::None;
                    }
                    _ => app.should_quit = true,
                },
                _ => {}
            }
            return;
        }
        ActiveModal::ThemePicker(sel, orig) => {
            let count = app.themes.len();
            match key.code {
                KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                    let next = (*sel + 1) % count;
                    *sel = next;
                    app.apply_theme_by_index(next);
                }
                KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                    let next = (*sel + count - 1) % count;
                    *sel = next;
                    app.apply_theme_by_index(next);
                }
                KeyCode::Enter => {
                    app.config.theme = app.theme.name.clone();
                    let _ = app.config.save();
                    app.status_line = format!("Theme saved: {}", app.theme.name);
                    app.modal = ActiveModal::None;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    let orig_name = orig.clone();
                    if let Some(idx) = app.themes.iter().position(|t| t.name == orig_name) {
                        app.apply_theme_by_index(idx);
                    }
                    app.modal = ActiveModal::None;
                }
                _ => {}
            }
            return;
        }
        ActiveModal::Settings(sel) => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('S') => {
                    app.modal = ActiveModal::None;
                }
                KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1) % 6,
                KeyCode::Up | KeyCode::Char('k') => *sel = (*sel + 5) % 6,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    match *sel {
                        0 => app.cycle_provider_mode(true),
                        1 => app.toggle_hifi_substitution(),
                        2 => app.cycle_enhancer(),
                        3 => app.toggle_transparent_bg(),
                        4 => app.cycle_hyprland_opacity(true),
                        _ => app.cycle_theme(true),
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => match *sel {
                    0 => app.cycle_provider_mode(false),
                    1 => app.toggle_hifi_substitution(),
                    2 => app.cycle_enhancer(),
                    3 => app.toggle_transparent_bg(),
                    4 => app.cycle_hyprland_opacity(false),
                    _ => app.cycle_theme(false),
                },
                _ => {}
            }
            return;
        }
        ActiveModal::SpotifyImport(buf) => {
            match key.code {
                KeyCode::Esc => app.modal = ActiveModal::None,
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let url = buf.clone();
                    app.modal = ActiveModal::None;
                    app.import_spotify_playlist(&url);
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            return;
        }
        ActiveModal::AddToPlaylist(sel) => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => app.modal = ActiveModal::None,
                KeyCode::Down | KeyCode::Char('j') if writable_pl_count > 0 => {
                    *sel = (*sel + 1) % writable_pl_count;
                }
                KeyCode::Up | KeyCode::Char('k') if writable_pl_count > 0 => {
                    *sel = (*sel + writable_pl_count - 1) % writable_pl_count;
                }
                KeyCode::Enter => {
                    let idx = *sel;
                    app.modal = ActiveModal::None;
                    app.confirm_add_to_playlist(idx);
                }
                _ => {}
            }
            return;
        }
        ActiveModal::CreatePlaylist(buf) => {
            match key.code {
                KeyCode::Esc => app.modal = ActiveModal::None,
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let title = buf.clone();
                    app.modal = ActiveModal::None;
                    app.create_ytm_playlist(&title);
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            return;
        }
        ActiveModal::Help => {
            if matches!(
                key.code,
                KeyCode::Esc
                    | KeyCode::Char('q')
                    | KeyCode::Char('?')
                    | KeyCode::Char('k')
                    | KeyCode::Char('K')
                    | KeyCode::Enter
            ) {
                app.modal = ActiveModal::None;
            }
            return;
        }
        ActiveModal::None => {}
    }

    if app.focus == FocusPane::SearchInput {
        match key.code {
            KeyCode::Esc => app.focus = FocusPane::Tracks,
            KeyCode::Tab => app.focus = FocusPane::Sidebar,
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Enter => {
                app.trigger_search();
                app.focus = FocusPane::Tracks;
            }
            KeyCode::Char(c) => app.search_query.push(c),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Char('?') => {
            app.modal = ActiveModal::Help;
        }
        KeyCode::Char('/') => app.focus = FocusPane::SearchInput,
        KeyCode::Tab => {
            app.focus = match app.focus {
                FocusPane::Sidebar => FocusPane::Tracks,
                FocusPane::Tracks | FocusPane::SearchInput => FocusPane::Sidebar,
            };
        }
        KeyCode::Left | KeyCode::Char('h') => app.focus = FocusPane::Tracks,
        KeyCode::Right | KeyCode::Char('l') => app.focus = FocusPane::Sidebar,
        KeyCode::Down | KeyCode::Char('j') => move_cursor(app, 1),
        KeyCode::Up => move_cursor(app, -1),
        KeyCode::Char('g') => jump_cursor(app, true),
        KeyCode::Char('G') => jump_cursor(app, false),
        KeyCode::Enter => match app.focus {
            FocusPane::Sidebar => app.activate_sidebar_selection(),
            FocusPane::Tracks => app.play_selected_track(),
            FocusPane::SearchInput => {}
        },
        KeyCode::Char(' ') => app.toggle_pause(),
        KeyCode::Char('n') => {
            if let Some(next) = app.playback.next_index() {
                app.play_queue_index(next);
            }
        }
        KeyCode::Char('p') => {
            if let Some(prev) = app.playback.prev_index() {
                app.play_queue_index(prev);
            }
        }
        KeyCode::Char('>') | KeyCode::Char('.') => app.seek(5.0),
        KeyCode::Char('<') | KeyCode::Char(',') => app.seek(-5.0),
        KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_volume(5),
        KeyCode::Char('-') | KeyCode::Char('_') => app.adjust_volume(-5),
        KeyCode::Char('e') => app.cycle_enhancer(),
        KeyCode::Char('s') => {
            app.playback.shuffle = !app.playback.shuffle;
            app.status_line = format!(
                "Shuffle: {}",
                if app.playback.shuffle { "On" } else { "Off" }
            );
        }
        KeyCode::Char('r') => {
            app.playback.repeat = app.playback.repeat.next();
            app.status_line = format!("Repeat: {}", app.playback.repeat.label());
        }
        KeyCode::Char('t') => {
            let cur_idx = app
                .themes
                .iter()
                .position(|t| t.name == app.theme.name)
                .unwrap_or(0);
            app.modal = ActiveModal::ThemePicker(cur_idx, app.theme.name.clone());
        }
        KeyCode::Char('i') => app.modal = ActiveModal::SpotifyImport(String::new()),
        KeyCode::Char('y') => app.toggle_like_selected(),
        KeyCode::Char('a') => app.open_add_to_playlist_modal(),
        KeyCode::Char('x') | KeyCode::Delete => app.remove_selected_from_playlist(),
        KeyCode::Char('c') => app.modal = ActiveModal::CreatePlaylist(String::new()),
        KeyCode::Char('S') => app.modal = ActiveModal::Settings(0),
        KeyCode::Char('m') => app.toggle_mini_mode(),
        KeyCode::Char('R') => app.refresh_account(),
        _ => {}
    }
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.modal != ActiveModal::None {
        return;
    }

    let x = mouse.column;
    let y = mouse.row;
    let rects = app.ui_rects;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if contains(rects.tracks_rows, x, y) {
                app.focus = FocusPane::Tracks;
                move_cursor(app, -1);
            } else if contains(rects.sidebar_inner, x, y) {
                app.focus = FocusPane::Sidebar;
                move_cursor(app, -1);
            } else if contains(rects.player_bar, x, y) {
                app.adjust_volume(5);
            }
        }
        MouseEventKind::ScrollDown => {
            if contains(rects.tracks_rows, x, y) {
                app.focus = FocusPane::Tracks;
                move_cursor(app, 1);
            } else if contains(rects.sidebar_inner, x, y) {
                app.focus = FocusPane::Sidebar;
                move_cursor(app, 1);
            } else if contains(rects.player_bar, x, y) {
                app.adjust_volume(-5);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if contains(rects.album_art_box, x, y) {
                let now = Instant::now();
                let is_double = app
                    .last_art_click
                    .is_some_and(|t| now.duration_since(t) <= Duration::from_millis(450));
                if is_double {
                    app.last_art_click = None;
                    app.toggle_mini_mode();
                } else {
                    app.last_art_click = Some(now);
                }
            } else if contains(rects.ctrl_prev, x, y) {
                if let Some(prev) = app.playback.prev_index() {
                    app.play_queue_index(prev);
                }
            } else if contains(rects.ctrl_play, x, y) {
                app.toggle_pause();
            } else if contains(rects.ctrl_next, x, y) {
                if let Some(next) = app.playback.next_index() {
                    app.play_queue_index(next);
                }
            } else if contains(rects.search_bar, x, y) {
                app.focus = FocusPane::SearchInput;
            } else if contains(rects.sidebar_inner, x, y) {
                app.focus = FocusPane::Sidebar;
                let idx = app.sidebar_state.offset() + (y - rects.sidebar_inner.y) as usize;
                if idx < app.sidebar_items.len() {
                    app.sidebar_idx = idx;
                    app.activate_sidebar_selection();
                }
            } else if contains(rects.tracks_rows, x, y) {
                app.focus = FocusPane::Tracks;
                let idx = app.track_table_state.offset() + (y - rects.tracks_rows.y) as usize;
                if idx < app.visible_tracks.len() {
                    let now = Instant::now();
                    let is_double = app.last_track_click.is_some_and(|(t, prev)| {
                        prev == idx && now.duration_since(t) <= Duration::from_millis(450)
                    });
                    app.track_table_state.select(Some(idx));
                    if is_double {
                        app.last_track_click = None;
                        app.play_selected_track();
                    } else {
                        app.last_track_click = Some((now, idx));
                    }
                }
            } else if contains(rects.progress_bar, x, y) {
                if app.playback.duration_secs > 1.0 && rects.progress_bar.width > 0 {
                    let rel = (x.saturating_sub(rects.progress_bar.x)) as f64
                        / rects.progress_bar.width as f64;
                    let target = (rel.clamp(0.0, 1.0)) * app.playback.duration_secs;
                    app.seek_to(target);
                }
            } else if contains(rects.player_top, x, y) {
                app.toggle_pause();
            }
        }
        MouseEventKind::Down(MouseButton::Right) if contains(rects.tracks_rows, x, y) => {
            app.focus = FocusPane::Tracks;
            let idx = app.track_table_state.offset() + (y - rects.tracks_rows.y) as usize;
            if idx < app.visible_tracks.len() {
                app.track_table_state.select(Some(idx));
                app.open_add_to_playlist_modal();
            }
        }
        _ => {}
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn move_cursor(app: &mut App, delta: isize) {
    match app.focus {
        FocusPane::Sidebar => {
            if app.sidebar_items.is_empty() {
                return;
            }
            let len = app.sidebar_items.len() as isize;
            app.sidebar_idx = ((app.sidebar_idx as isize + delta).clamp(0, len - 1)) as usize;
        }
        FocusPane::Tracks => {
            if app.visible_tracks.is_empty() {
                return;
            }
            let len = app.visible_tracks.len() as isize;
            let cur = app.track_table_state.selected().unwrap_or(0) as isize;
            let next = (cur + delta).clamp(0, len - 1) as usize;
            app.track_table_state.select(Some(next));
        }
        FocusPane::SearchInput => {}
    }
}

fn jump_cursor(app: &mut App, top: bool) {
    match app.focus {
        FocusPane::Sidebar if !app.sidebar_items.is_empty() => {
            app.sidebar_idx = if top { 0 } else { app.sidebar_items.len() - 1 };
        }
        FocusPane::Tracks if !app.visible_tracks.is_empty() => {
            let idx = if top { 0 } else { app.visible_tracks.len() - 1 };
            app.track_table_state.select(Some(idx));
        }
        _ => {}
    }
}
