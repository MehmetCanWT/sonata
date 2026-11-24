use crate::app::{ActiveModal, App, FocusPane, SplitTarget};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    let writable_pl_count = app.playlist_add_targets_for_selected().len();
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
                        0 => app.toggle_personal_only_mode(),
                        1 => app.toggle_jiosaavn(),
                        2 => app.toggle_attribute_personal_to_ytm(),
                        3 => app.modal = ActiveModal::LocalMusicManager(0),
                        4 => app.modal = ActiveModal::ServerManager(0),
                        5 => app.cycle_provider_mode(true),
                        6 => app.toggle_hifi_substitution(),
                        7 => app.toggle_autoplay(),
                        8 => app.step_playback_speed(true),
                        9 => app.toggle_skip_silence(),
                        10 => app.cycle_sleep_timer(true),
                        11 => app.toggle_lyrics_translate(),
                        12 => app.toggle_lyrics_romanize(),
                        13 => app.cycle_language(true),
                        14 => app.cycle_theme(true),
                        15 => app.toggle_motion_covers(),
                        16 => app.toggle_transparent_bg(),
                        17 => app.cycle_hyprland_opacity(true),
                        18 => app.step_cache_limit(true),
                        19 => app.step_downloads_limit(true),
                        20 => {
                            let s = sonata_runtime::i18n::strings(app.config.language);
                            let buf = app
                                .config
                                .downloads_dir
                                .as_ref()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            app.modal = ActiveModal::TextInput {
                                title: s.downloads_dir.to_string(),
                                prompt: "Path (leave empty for default folder):".to_string(),
                                buf,
                                target: crate::app::TextInputTarget::DownloadsDir,
                            };
                        }
                        21 => app.clear_all_downloads(),
                        22 => app.toggle_discord_rpc(),
                        23 => {
                            let s = sonata_runtime::i18n::strings(app.config.language);
                            let buf = app.config.discord_app_id.clone().unwrap_or_default();
                            app.modal = ActiveModal::TextInput {
                                title: s.discord_app_id.to_string(),
                                prompt: "Application ID (empty for Sonata default):".to_string(),
                                buf,
                                target: crate::app::TextInputTarget::DiscordAppId,
                            };
                        }
                        24 => app.refresh_account(),
                        25 => app.check_for_updates(),
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => match *sel {
                    0 => app.toggle_personal_only_mode(),
                    1 => app.toggle_jiosaavn(),
                    2 => app.toggle_attribute_personal_to_ytm(),
                    3 => app.toggle_local_music_enabled(),
                    5 => app.cycle_provider_mode(false),
                    6 => app.toggle_hifi_substitution(),
                    7 => app.toggle_autoplay(),
                    8 => app.step_playback_speed(false),
                    9 => app.toggle_skip_silence(),
                    10 => app.cycle_sleep_timer(false),
                    11 => app.toggle_lyrics_translate(),
                    12 => app.toggle_lyrics_romanize(),
                    13 => app.cycle_language(false),
                    14 => app.cycle_theme(false),
                    15 => app.toggle_motion_covers(),
                    16 => app.toggle_transparent_bg(),
                    17 => app.cycle_hyprland_opacity(false),
                    18 => app.step_cache_limit(false),
                    19 => app.step_downloads_limit(false),
                    22 => app.toggle_discord_rpc(),
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
        ActiveModal::CreatePlaylist { buf, personal } => {
            match key.code {
                KeyCode::Esc => app.modal = ActiveModal::None,
                KeyCode::Tab => {
                    if !app.config.personal_only_mode && app.ytm.is_authenticated() {
                        *personal = !*personal;
                    }
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let title = buf.clone();
                    let is_personal = *personal;
                    app.modal = ActiveModal::None;
                    if is_personal {
                        app.create_personal_playlist(&title);
                    } else {
                        app.create_ytm_playlist(&title);
                    }
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            return;
        }
        ActiveModal::RenamePersonalPlaylist { playlist_id, buf } => {
            match key.code {
                KeyCode::Esc => app.modal = ActiveModal::None,
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let pid = playlist_id.clone();
                    let name = buf.clone();
                    app.modal = ActiveModal::None;
                    app.rename_personal_playlist(&pid, &name);
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            return;
        }
        ActiveModal::TextInput { buf, target, .. } => {
            let target = *target;
            match key.code {
                KeyCode::Esc => {
                    app.modal = match target {
                        crate::app::TextInputTarget::DownloadsDir => ActiveModal::Settings(20),
                        crate::app::TextInputTarget::DiscordAppId => ActiveModal::Settings(23),
                        crate::app::TextInputTarget::LocalMusicDir => {
                            ActiveModal::LocalMusicManager(0)
                        }
                    };
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let val = buf.trim().to_string();
                    app.modal = match target {
                        crate::app::TextInputTarget::DownloadsDir => {
                            let dir = if val.is_empty() {
                                None
                            } else {
                                Some(std::path::PathBuf::from(val))
                            };
                            app.set_downloads_dir(dir);
                            ActiveModal::Settings(20)
                        }
                        crate::app::TextInputTarget::DiscordAppId => {
                            let id = if val.is_empty() { None } else { Some(val) };
                            app.set_discord_app_id(id);
                            ActiveModal::Settings(23)
                        }
                        crate::app::TextInputTarget::LocalMusicDir => {
                            if !val.is_empty() {
                                app.add_local_music_dir(std::path::PathBuf::from(val));
                            }
                            ActiveModal::LocalMusicManager(0)
                        }
                    };
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            return;
        }
        ActiveModal::LocalMusicManager(sel) => {
            let count = app.config.effective_local_music_dirs().len();
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => app.modal = ActiveModal::Settings(3),
                KeyCode::Down | KeyCode::Char('j') if count > 0 => *sel = (*sel + 1) % count,
                KeyCode::Up | KeyCode::Char('k') if count > 0 => *sel = (*sel + count - 1) % count,
                KeyCode::Enter | KeyCode::Char(' ') => app.toggle_local_music_enabled(),
                KeyCode::Char('s') => app.scan_local_music(),
                KeyCode::Char('a') => {
                    let s = sonata_runtime::i18n::strings(app.config.language);
                    app.modal = ActiveModal::TextInput {
                        title: s.folders.to_string(),
                        prompt: "Local music folder path:".to_string(),
                        buf: String::new(),
                        target: crate::app::TextInputTarget::LocalMusicDir,
                    };
                }
                KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    app.remove_local_music_dir(idx);
                    let next_len = app.config.effective_local_music_dirs().len();
                    app.modal = ActiveModal::LocalMusicManager(if next_len == 0 {
                        0
                    } else {
                        idx.min(next_len - 1)
                    });
                }
                _ => {}
            }
            return;
        }
        ActiveModal::ServerManager(sel) => {
            let count = app.server_profiles.len();
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => app.modal = ActiveModal::Settings(4),
                KeyCode::Down | KeyCode::Char('j') if count > 0 => *sel = (*sel + 1) % count,
                KeyCode::Up | KeyCode::Char('k') if count > 0 => *sel = (*sel + count - 1) % count,
                KeyCode::Char('a') => {
                    app.modal = ActiveModal::AddServer(crate::app::ServerFormState::default());
                }
                KeyCode::Char('e') if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let form = crate::app::ServerFormState::from_profile(&app.server_profiles[idx]);
                    app.modal = ActiveModal::AddServer(form);
                }
                KeyCode::Char('t') if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let id = app.server_profiles[idx].id.clone();
                    app.probe_server_health(&id);
                }
                KeyCode::Char('s') if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let id = app.server_profiles[idx].id.clone();
                    app.run_server_speed_test(&id);
                }
                KeyCode::Enter | KeyCode::Char(' ') if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let id = app.server_profiles[idx].id.clone();
                    app.toggle_server_enabled(&id);
                }
                KeyCode::Char('b') if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let profile = app.server_profiles[idx].clone();
                    app.open_server_browser(profile);
                }
                KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete if count > 0 => {
                    let idx = (*sel).min(count - 1);
                    let id = app.server_profiles[idx].id.clone();
                    app.remove_server_profile(&id);
                    let next_len = app.server_profiles.len();
                    app.modal = ActiveModal::ServerManager(if next_len == 0 {
                        0
                    } else {
                        idx.min(next_len - 1)
                    });
                }
                _ => {}
            }
            return;
        }
        ActiveModal::AddServer(form) => {
            use crate::app::ServerFormField;
            use sonata_core::providers::servers::ServerKind;
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
                let snapshot = form.clone();
                app.test_server_form(&snapshot);
                return;
            }
            let cycle_kind = |kind: ServerKind, forward: bool| -> ServerKind {
                match (kind, forward) {
                    (ServerKind::Jellyfin, true) => ServerKind::Subsonic,
                    (ServerKind::Subsonic, true) => ServerKind::Smb,
                    (ServerKind::Smb, true) => ServerKind::WebDav,
                    (ServerKind::WebDav, true) => ServerKind::Jellyfin,
                    (ServerKind::Jellyfin, false) => ServerKind::WebDav,
                    (ServerKind::Subsonic, false) => ServerKind::Jellyfin,
                    (ServerKind::Smb, false) => ServerKind::Subsonic,
                    (ServerKind::WebDav, false) => ServerKind::Smb,
                }
            };
            match key.code {
                KeyCode::Esc => app.modal = ActiveModal::ServerManager(0),
                KeyCode::Tab | KeyCode::Down => {
                    form.focused = form.focused.next_for_kind(form.kind)
                }
                KeyCode::BackTab | KeyCode::Up => {
                    form.focused = form.focused.prev_for_kind(form.kind)
                }
                KeyCode::Left | KeyCode::Right if form.focused == ServerFormField::Kind => {
                    form.kind = cycle_kind(form.kind, key.code == KeyCode::Right);
                }
                KeyCode::Enter => {
                    if form.focused == ServerFormField::Kind {
                        form.kind = cycle_kind(form.kind, true);
                    } else {
                        let snapshot = form.clone();
                        app.modal = ActiveModal::ServerManager(0);
                        app.save_server_form(&snapshot);
                    }
                }
                KeyCode::Backspace => match form.focused {
                    ServerFormField::Kind => {}
                    ServerFormField::Name => {
                        form.name.pop();
                    }
                    ServerFormField::BaseUrl => {
                        form.base_url.pop();
                    }
                    ServerFormField::Share => {
                        form.share.pop();
                    }
                    ServerFormField::Domain => {
                        form.domain.pop();
                    }
                    ServerFormField::MountPath => {
                        form.mount_path.pop();
                    }
                    ServerFormField::Username => {
                        form.username.pop();
                    }
                    ServerFormField::Password => {
                        form.password.pop();
                    }
                    ServerFormField::Token => {
                        form.token.pop();
                    }
                },
                KeyCode::Char(c) => match form.focused {
                    ServerFormField::Kind => {}
                    ServerFormField::Name => form.name.push(c),
                    ServerFormField::BaseUrl => form.base_url.push(c),
                    ServerFormField::Share => form.share.push(c),
                    ServerFormField::Domain => form.domain.push(c),
                    ServerFormField::MountPath => form.mount_path.push(c),
                    ServerFormField::Username => form.username.push(c),
                    ServerFormField::Password => form.password.push(c),
                    ServerFormField::Token => form.token.push(c),
                },
                _ => {}
            }
            return;
        }
        ActiveModal::ServerBrowser(st) => {
            use crate::app::ServerBrowserTab;
            let len = match st.tab {
                ServerBrowserTab::Tracks => app.server_tracks.len(),
                ServerBrowserTab::Playlists => app.server_playlists.len(),
                ServerBrowserTab::Albums => app.server_albums.len(),
                ServerBrowserTab::Artists => app.server_artists.len(),
                ServerBrowserTab::Libraries => app.server_libraries.len(),
            };
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => app.modal = ActiveModal::None,
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                    st.tab = st.tab.next();
                    st.selected = 0;
                }
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                    st.tab = st.tab.prev();
                    st.selected = 0;
                }
                KeyCode::Down | KeyCode::Char('j') if len > 0 => {
                    st.selected = (st.selected + 1) % len;
                }
                KeyCode::Up | KeyCode::Char('k') if len > 0 => {
                    st.selected = (st.selected + len - 1) % len;
                }
                KeyCode::Char('r') => {
                    let sid = st.server_id.clone();
                    let folder = st.folder_path.clone();
                    app.load_server_browse(sid, None, folder);
                }
                KeyCode::Backspace => {
                    let sid = st.server_id.clone();
                    let parent = st.folder_path.as_deref().and_then(|p| {
                        let trimmed = p.trim_end_matches('/');
                        trimmed.rsplit_once('/').map(|(a, _)| a.to_string())
                    });
                    app.load_server_browse(sid, None, parent);
                }
                KeyCode::Enter => {
                    app.activate_server_browser_item();
                }
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
        KeyCode::Char(']') => app.step_playback_speed(true),
        KeyCode::Char('[') => app.step_playback_speed(false),
        KeyCode::Char('T') => app.toggle_lyrics_translate(),
        KeyCode::Char('O') => app.toggle_lyrics_romanize(),
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
        KeyCode::Char('d') => app.toggle_download_selected_track(),
        KeyCode::Char('D') => app.toggle_download_visible_collection(),
        KeyCode::Char('y') => app.toggle_like_selected(),
        KeyCode::Char('a') => app.open_add_to_playlist_modal(),
        KeyCode::Char('v') => app.cycle_library_source(true),
        KeyCode::Char('e') => {
            if let Some(crate::app::SidebarEntry::PersonalPlaylist(ppl)) =
                app.sidebar_items.get(app.sidebar_idx).cloned()
            {
                app.modal = ActiveModal::RenamePersonalPlaylist {
                    playlist_id: ppl.id,
                    buf: ppl.name,
                };
            }
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            if app.focus == FocusPane::Sidebar {
                if let Some(crate::app::SidebarEntry::PersonalPlaylist(ppl)) =
                    app.sidebar_items.get(app.sidebar_idx).cloned()
                {
                    app.delete_personal_playlist(&ppl.id);
                    return;
                }
            }
            app.remove_selected_from_playlist();
        }
        KeyCode::Char('c') => {
            let personal = app.config.personal_only_mode
                || !app.ytm.is_authenticated()
                || app.active_source != crate::app::TuiLibrarySource::YouTube;
            app.modal = ActiveModal::CreatePlaylist {
                buf: String::new(),
                personal,
            };
        }
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
        let writable_count = app.playlist_add_targets_for_selected().len();
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
