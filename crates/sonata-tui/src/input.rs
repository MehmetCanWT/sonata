use crate::app::{ActiveModal, App, FocusPane, SplitTarget};
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
            let n = crate::ui::modals::SETTINGS_ROW_COUNT;
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('S') => {
                    app.modal = ActiveModal::None;
                }
                KeyCode::Char('u') => {
                    app.check_for_updates();
                }
                KeyCode::Char('r') => {
                    app.refresh_account();
                }
                KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1) % n,
                KeyCode::Up | KeyCode::Char('k') => *sel = (*sel + n - 1) % n,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    match *sel {
                        0 => app.cycle_provider_mode(true),
                        1 => app.toggle_hifi_substitution(),
                        2 => app.toggle_jiosaavn(),
                        3 => app.toggle_autoplay(),
                        4 => app.cycle_language(true),
                        5 => app.cycle_theme(true),
                        6 => app.toggle_motion_covers(),
                        7 => app.toggle_transparent_bg(),
                        8 => app.cycle_hyprland_opacity(true),
                        9 => app.step_cache_limit(true),
                        10 => app.refresh_account(),
                        11 => app.check_for_updates(),
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => match *sel {
                    0 => app.cycle_provider_mode(false),
                    1 => app.toggle_hifi_substitution(),
                    2 => app.toggle_jiosaavn(),
                    3 => app.toggle_autoplay(),
                    4 => app.cycle_language(false),
                    5 => app.cycle_theme(false),
                    6 => app.toggle_motion_covers(),
                    7 => app.toggle_transparent_bg(),
                    8 => app.cycle_hyprland_opacity(false),
                    9 => app.step_cache_limit(false),
                    _ => {}
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
        KeyCode::Char('H') => app.adjust_focused_pane_width(-2),
        KeyCode::Char('L') => app.adjust_focused_pane_width(2),
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
        KeyCode::Char('n') => app.play_next_track(),
        KeyCode::Char('p') => app.play_prev_track(),
        KeyCode::Char('>') | KeyCode::Char('.') => app.seek(5.0),
        KeyCode::Char('<') | KeyCode::Char(',') => app.seek(-5.0),
        KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_volume(5),
        KeyCode::Char('-') | KeyCode::Char('_') => app.adjust_volume(-5),
        KeyCode::Char('s') => app.toggle_shuffle(),
        KeyCode::Char('r') => app.cycle_repeat(),
        KeyCode::Char('E') => app.toggle_autoplay(),
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
        let delta: isize = match mouse.kind {
            MouseEventKind::ScrollUp => -1,
            MouseEventKind::ScrollDown => 1,
            _ => return,
        };
        let theme_count = app.themes.len();
        let writable_count = app.writable_ytm_playlists().len();
        match &mut app.modal {
            ActiveModal::ThemePicker(sel, _) if theme_count > 0 => {
                let next = ((*sel as isize + delta).rem_euclid(theme_count as isize)) as usize;
                *sel = next;
                app.apply_theme_by_index(next);
            }
            ActiveModal::Settings(sel) => {
                let n = crate::ui::modals::SETTINGS_ROW_COUNT as isize;
                *sel = ((*sel as isize + delta).rem_euclid(n)) as usize;
            }
            ActiveModal::AddToPlaylist(sel) if writable_count > 0 => {
                *sel = ((*sel as isize + delta).rem_euclid(writable_count as isize)) as usize;
            }
            _ => {}
        }
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
                app.adjust_volume(2);
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
                app.adjust_volume(-2);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(target) = app.dragging_split {
                resize_split(app, target, x, y);
            } else if contains(rects.volume_bar, x, y) && rects.volume_bar.width > 0 {
                let denom = rects.volume_bar.width.saturating_sub(1).max(1) as f64;
                let rel = x.saturating_sub(rects.volume_bar.x) as f64 / denom;
                app.set_volume((rel.clamp(0.0, 1.0) * 100.0).round() as u8, false);
            } else if contains(rects.progress_bar, x, y)
                && app.playback.duration_secs > 1.0
                && rects.progress_bar.width > 0
            {
                let rel = (x.saturating_sub(rects.progress_bar.x)) as f64
                    / rects.progress_bar.width as f64;
                app.seek_to((rel.clamp(0.0, 1.0)) * app.playback.duration_secs);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.dragging_split.take().is_some() {
                let _ = app.config.save();
            }
            app.flush_config();
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if rects.main_area.width > 0
                && y >= rects.main_area.y
                && y < rects.main_area.y + rects.main_area.height
            {
                if rects.split_left_x > 0 && x.abs_diff(rects.split_left_x) <= 1 {
                    app.dragging_split = Some(SplitTarget::LeftCol);
                    return;
                }
                if rects.split_right_x > 0 && x.abs_diff(rects.split_right_x) <= 1 {
                    app.dragging_split = Some(SplitTarget::RightCol);
                    return;
                }
                if rects.split_art_y > 0
                    && x < rects.split_left_x
                    && y.abs_diff(rects.split_art_y) <= 1
                {
                    app.dragging_split = Some(SplitTarget::ArtLyrics);
                    return;
                }
            }
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
                app.play_prev_track();
            } else if contains(rects.ctrl_play, x, y) {
                app.toggle_pause();
            } else if contains(rects.ctrl_next, x, y) {
                app.play_next_track();
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
            } else if contains(rects.volume_bar, x, y) && rects.volume_bar.width > 0 {
                let denom = rects.volume_bar.width.saturating_sub(1).max(1) as f64;
                let rel = x.saturating_sub(rects.volume_bar.x) as f64 / denom;
                app.set_volume((rel.clamp(0.0, 1.0) * 100.0).round() as u8, true);
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

fn resize_split(app: &mut App, target: SplitTarget, x: u16, y: u16) {
    let main = app.ui_rects.main_area;
    match target {
        SplitTarget::LeftCol if main.width > 0 => {
            let pct = (((x.saturating_sub(main.x)) as u32 * 100) / main.width as u32).clamp(18, 52)
                as u16;
            if 100u16.saturating_sub(pct + app.config.right_pane_pct) >= 20 {
                app.config.left_pane_pct = pct;
            }
        }
        SplitTarget::RightCol if main.width > 0 => {
            let right_w = (main.x + main.width).saturating_sub(x);
            let pct = ((right_w as u32 * 100) / main.width as u32).clamp(14, 48) as u16;
            if 100u16.saturating_sub(app.config.left_pane_pct + pct) >= 20 {
                app.config.right_pane_pct = pct;
            }
        }
        SplitTarget::ArtLyrics if app.ui_rects.left_col_area.height > 0 => {
            let area = app.ui_rects.left_col_area;
            let (fw, fh) = app.picker.font_size();
            let inner_w = area.width.saturating_sub(2) as u32;
            let ideal_art_h = ((inner_w * fw.max(1) as u32) / fh.max(1) as u32) as i32 + 2;
            let desired_h = y.saturating_sub(area.y) as i32;
            let max_art_h = area.height.saturating_sub(6).max(8) as i32;
            let clamped_h = desired_h.clamp(8.min(max_art_h), max_art_h);
            app.config.art_height_offset = (clamped_h - ideal_art_h) as i16;
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
