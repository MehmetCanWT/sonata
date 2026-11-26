//! Root terminal layout and player bar rendering for `sonata` (TUI).
//!
//! 1. **Workspace Layout (`render`)**: Splits the terminal into Now Playing (artwork + lyrics), Track List, Library Sidebar, and the bottom player bar (or compact mini-player tiers).
//! 2. **Balanced Player Bar (`render_player_bar`)**: Uses symmetric side columns so track metadata and Shuffle/Repeat controls stay dead-center horizontally, paired with a unified thin-track progress and volume bar.

pub mod fx;
pub mod modals;
pub mod now_playing;
pub mod sidebar;
pub mod track_list;

use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use sonata_core::lyrics::LyricsState;
use sonata_core::player::RepeatMode;
use sonata_core::providers::format_duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniLayoutTier {
    SideBySide,
    ArtworkOrLyric,
    UltraCompact,
}

pub fn is_auto_compact(area: Rect) -> bool {
    area.width < 90 || area.height < 20
}

/// Strips ANSI escape sequences (CSI/OSC/DCS/APC/ESC), C0/C1 control characters,
/// and Unicode bidirectional override/isolate codepoints from untrusted terminal text.
pub fn sanitize_tui_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']' | 'P' | '_' | '^' | 'X') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(c) if ('\x20'..='\x7e').contains(&c) => {
                    chars.next();
                }
                _ => {}
            },
            '\u{009B}' => {
                for c in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            '\u{0090}' | '\u{009D}' | '\u{009E}' | '\u{009F}' => {
                while let Some(c) = chars.next() {
                    if c == '\x07' || c == '\u{009C}' {
                        break;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            '\x00'..='\x1f' | '\x7f'..='\u{009f}' => {}
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {}
            _ => out.push(ch),
        }
    }
    out
}

pub fn mini_layout_tier(area: Rect) -> MiniLayoutTier {
    if area.width >= 60 && area.height >= 13 {
        MiniLayoutTier::SideBySide
    } else if area.width >= 36 && area.height >= 10 {
        MiniLayoutTier::ArtworkOrLyric
    } else {
        MiniLayoutTier::UltraCompact
    }
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.bg).fg(app.theme.fg)),
        area,
    );

    if app.mini_mode || is_auto_compact(area) {
        render_mini_layout(frame, area, app);
    } else {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(5)])
            .split(area);

        // Mirrored layout: Now Playing + Lyrics (left) | Tracks (center) | Library (right)
        let left_pct = app.config.left_pane_pct.clamp(18, 52);
        let right_pct = app.config.right_pane_pct.clamp(14, 48);
        let center_pct = 100u16.saturating_sub(left_pct + right_pct).max(20);
        let main_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Percentage(center_pct),
                Constraint::Percentage(right_pct),
            ])
            .split(root[0]);

        app.ui_rects.main_area = root[0];
        app.ui_rects.split_left_x = main_cols[1].x;
        app.ui_rects.split_right_x = main_cols[2].x;

        now_playing::render(frame, main_cols[0], app);
        track_list::render(frame, main_cols[1], app);
        sidebar::render(frame, main_cols[2], app);
        render_player_bar(frame, root[1], app);
    }
    modals::render(frame, area, app);

    let active_lyric_idx = match &app.lyrics {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            let pos = app.interpolated_position();
            sonata_core::lyrics::active_line_index(lines, pos + 0.15)
                .or_else(|| sonata_core::lyrics::active_line_index(lines, pos))
        }
        _ => None,
    };
    let track_id = app.playback.current_track().map(|t| t.id.clone());
    let fx_ctx = fx::FxFrameContext {
        screen_area: area,
        tracks_area: app.ui_rects.tracks_rows,
        lyrics_area: app.ui_rects.lyrics_box,
        player_bar_area: app.ui_rects.player_bar,
        tracks_heading: &app.tracks_heading,
        track_id: track_id.as_deref(),
        active_lyric_idx,
        modal_open: fx::is_modal_open(&app.modal),
        theme: &app.theme,
    };
    app.fx.sync_and_render(&fx_ctx, frame.buffer_mut());
}

fn render_mini_layout(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.search_bar = Rect::default();
    app.ui_rects.tracks_rows = Rect::default();
    app.ui_rects.sidebar_inner = Rect::default();
    app.ui_rects.main_area = Rect::default();
    app.ui_rects.left_col_area = Rect::default();
    app.ui_rects.split_left_x = 0;
    app.ui_rects.split_right_x = 0;
    app.ui_rects.split_art_y = 0;

    match mini_layout_tier(area) {
        MiniLayoutTier::UltraCompact => {
            app.ui_rects.album_art_box = Rect::default();
            if app.last_art_rect.is_some() {
                app.last_art_rect = None;
                app.clear_kitty_images();
            }
            render_mini_player_bar(frame, area, app, area.height >= 6);
        }
        MiniLayoutTier::ArtworkOrLyric => {
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(5), Constraint::Length(5)])
                .split(area);
            let has_lyrics = match &app.lyrics {
                LyricsState::Synced(lines) => !lines.is_empty(),
                LyricsState::Plain(lines) => !lines.is_empty(),
                _ => false,
            };
            if area.height < 14 && has_lyrics {
                app.ui_rects.album_art_box = Rect::default();
                if app.last_art_rect.is_some() {
                    app.last_art_rect = None;
                    app.clear_kitty_images();
                }
                now_playing::render_lyrics(frame, root[0], app);
            } else {
                now_playing::render_artwork(frame, root[0], app);
            }
            render_mini_player_bar(frame, root[1], app, false);
        }
        MiniLayoutTier::SideBySide => {
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(5), Constraint::Length(5)])
                .split(area);
            let (fw, fh) = app.picker.font_size();
            let inner_h = root[0].height.saturating_sub(2) as u32;
            let ideal_art_w = ((inner_h * fh.max(1) as u32) / fw.max(1) as u32) as u16 + 2;
            let max_art_w = (root[0].width * 55 / 100).max(16);
            let art_w = ideal_art_w.clamp(16.min(max_art_w), max_art_w);
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(art_w), Constraint::Min(16)])
                .split(root[0]);
            now_playing::render_artwork(frame, cols[0], app);
            now_playing::render_lyrics(frame, cols[1], app);
            render_mini_player_bar(frame, root[1], app, false);
        }
    }
}

fn render_mini_player_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App, allow_lyric_row: bool) {
    app.ui_rects.player_bar = area;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        app.ui_rects.player_top = Rect::default();
        app.ui_rects.progress_bar = Rect::default();
        app.ui_rects.volume_bar = Rect::default();
        app.ui_rects.ctrl_prev = Rect::default();
        app.ui_rects.ctrl_play = Rect::default();
        app.ui_rects.ctrl_next = Rect::default();
        return;
    }

    let show_lyric = allow_lyric_row && inner.height >= 4;
    let row_constraints = if show_lyric {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else if inner.height >= 3 {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else if inner.height == 2 {
        vec![Constraint::Length(1), Constraint::Length(1)]
    } else {
        vec![Constraint::Length(1)]
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(inner);

    app.ui_rects.player_top = rows[0];
    app.ui_rects.volume_bar = Rect::default();

    let s = sonata_runtime::i18n::strings(app.config.language);
    let title_text = match app.playback.current_track() {
        Some(t) => format!(
            "{} — {}",
            sanitize_tui_text(&t.title),
            sanitize_tui_text(&t.artist)
        ),
        None => s.no_track_playing.to_string(),
    };
    let top_line = Line::from(Span::styled(
        title_text,
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(top_line).alignment(Alignment::Center),
        rows[0],
    );

    let (progress_row, ctrl_row) = if show_lyric {
        let lyric_line = now_playing::active_karaoke_line(app)
            .unwrap_or_else(|| Line::from(Span::styled("♪", Style::default().fg(app.theme.muted))));
        frame.render_widget(
            Paragraph::new(lyric_line).alignment(Alignment::Center),
            rows[1],
        );
        (Some(rows[2]), Some(rows[3]))
    } else if rows.len() >= 3 {
        (Some(rows[1]), Some(rows[2]))
    } else if rows.len() == 2 {
        (Some(rows[1]), None)
    } else {
        (None, None)
    };

    if let Some(p_row) = progress_row {
        render_progress_row(frame, p_row, app);
    } else {
        app.ui_rects.progress_bar = Rect::default();
    }

    let Some(c_row) = ctrl_row else {
        app.ui_rects.ctrl_prev = Rect::default();
        app.ui_rects.ctrl_play = Rect::default();
        app.ui_rects.ctrl_next = Rect::default();
        return;
    };

    let full_prev = format!(" ⏮ {} ", s.prev_short);
    let full_play = if app.playback.paused {
        format!(" ▶ {} ", s.play)
    } else {
        format!(" ⏸ {} ", s.pause)
    };
    let full_next = format!(" {} ⏭ ", s.next_short);
    let full_prev_w = (full_prev.chars().count() as u16).max(8);
    let full_play_w = (full_play.chars().count() as u16 + 2).max(12);
    let full_next_w = (full_next.chars().count() as u16).max(8);
    let (prev_lbl, play_lbl, next_lbl, prev_w, play_w, next_w) =
        if inner.width >= full_prev_w + full_play_w + full_next_w + 2 {
            (
                full_prev,
                full_play,
                full_next,
                full_prev_w,
                full_play_w,
                full_next_w,
            )
        } else {
            (
                " ⏮ ".to_string(),
                if app.playback.paused {
                    " ▶ ".to_string()
                } else {
                    " ⏸ ".to_string()
                },
                " ⏭ ".to_string(),
                4u16,
                5u16,
                4u16,
            )
        };

    let ctrl_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(prev_w),
            Constraint::Length(play_w),
            Constraint::Length(next_w),
            Constraint::Fill(1),
        ])
        .split(c_row);

    app.ui_rects.ctrl_prev = ctrl_cols[1];
    app.ui_rects.ctrl_play = ctrl_cols[2];
    app.ui_rects.ctrl_next = ctrl_cols[3];

    frame.render_widget(
        Paragraph::new(prev_lbl)
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.fg)),
        ctrl_cols[1],
    );
    frame.render_widget(
        Paragraph::new(play_lbl).alignment(Alignment::Center).style(
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        ctrl_cols[2],
    );
    frame.render_widget(
        Paragraph::new(next_lbl)
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.fg)),
        ctrl_cols[3],
    );
}

fn render_player_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    app.ui_rects.player_bar = area;
    app.ui_rects.ctrl_prev = Rect::default();
    app.ui_rects.ctrl_play = Rect::default();
    app.ui_rects.ctrl_next = Rect::default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.surface).fg(app.theme.fg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    app.ui_rects.progress_bar = rows[1];

    let quality_badge = app
        .playback
        .stream_badge
        .strip_suffix(" · Cached")
        .unwrap_or(&app.playback.stream_badge);

    // Symmetric side widths keep the center title and mode toggles dead-center in the terminal.
    let badge_chars = quality_badge.chars().count() as u16 + 4;
    let desired_side = badge_chars.max(36);
    let side_w = if inner.width >= 96 {
        desired_side
            .max(inner.width * 28 / 100)
            .min(56)
            .min(inner.width.saturating_sub(24) / 2)
    } else {
        desired_side
            .clamp(18, 48)
            .min(inner.width.saturating_sub(12) / 2)
    };

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(side_w),
            Constraint::Min(12),
            Constraint::Length(side_w),
        ])
        .split(rows[0]);

    app.ui_rects.player_top = top_cols[1];

    let s = sonata_runtime::i18n::strings(app.config.language);
    let (title, artist) = match app.playback.current_track() {
        Some(t) => (t.title.as_str(), t.artist.as_str()),
        None => (s.no_track_playing, s.pick_track_hint),
    };

    let play_icon = if app.playback.loading_track {
        "◌"
    } else if app.playback.paused {
        "▶"
    } else {
        "⏸"
    };

    let ctrl_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Fill(1),
        ])
        .split(top_cols[0]);
    app.ui_rects.ctrl_prev = ctrl_cols[0];
    app.ui_rects.ctrl_play = ctrl_cols[1];
    app.ui_rects.ctrl_next = ctrl_cols[2];

    frame.render_widget(
        Paragraph::new(" ⏮ ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.muted)),
        ctrl_cols[0],
    );
    frame.render_widget(
        Paragraph::new(format!(" {play_icon} "))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ctrl_cols[1],
    );
    frame.render_widget(
        Paragraph::new(" ⏭ ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.muted)),
        ctrl_cols[2],
    );

    let title_line = Line::from(vec![
        Span::styled(
            sanitize_tui_text(title),
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {}", sanitize_tui_text(artist)),
            Style::default().fg(app.theme.muted),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(title_line).alignment(Alignment::Center),
        top_cols[1],
    );

    let badges = Line::from(vec![Span::styled(
        format!("[{}] ", sanitize_tui_text(quality_badge)),
        Style::default().fg(app.theme.success),
    )]);
    frame.render_widget(
        Paragraph::new(badges).alignment(Alignment::Right),
        top_cols[2],
    );

    render_progress_row(frame, rows[1], app);

    let bot_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(side_w),
            Constraint::Min(12),
            Constraint::Length(side_w),
        ])
        .split(rows[2]);

    let vol_lbl = format!(" {} ", s.vol_short);
    let vol_lbl_w = (vol_lbl.chars().count() as u16).max(5);
    let vol_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(vol_lbl_w),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Fill(1),
        ])
        .split(bot_cols[0]);
    app.ui_rects.volume_bar = vol_cols[1];

    frame.render_widget(
        Paragraph::new(Span::styled(vol_lbl, Style::default().fg(app.theme.muted))),
        vol_cols[0],
    );
    let vol_ratio = (app.playback.volume as f64 / 100.0).clamp(0.0, 1.0);
    frame.render_widget(
        Paragraph::new(slider_line(
            vol_ratio,
            vol_cols[1].width as usize,
            app.theme.accent,
            app.theme.highlight_fg,
            app.theme.border,
        )),
        vol_cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {:>3}% ", app.playback.volume),
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )),
        vol_cols[2],
    );

    let shuffle_style = if app.playback.shuffle {
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.muted)
    };
    let repeat_style = if app.playback.repeat != RepeatMode::Off {
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.muted)
    };

    let mut mode_spans = if bot_cols[1].width >= 33 {
        vec![
            Span::styled(
                if app.playback.shuffle {
                    format!("⇄ {}", s.shuffle_on)
                } else {
                    format!("⇄ {}", s.shuffle_off)
                },
                shuffle_style,
            ),
            Span::styled(" · ", Style::default().fg(app.theme.border)),
            Span::styled(
                match app.playback.repeat {
                    RepeatMode::Off => format!("↻ {}", s.repeat_off),
                    RepeatMode::All => format!("↻ {}", s.repeat_all),
                    RepeatMode::One => format!("↻ {}", s.repeat_one),
                },
                repeat_style,
            ),
            Span::styled(" · ", Style::default().fg(app.theme.border)),
            Span::styled(
                if app.config.autoplay { "∞" } else { "∞·" },
                if app.config.autoplay {
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.muted)
                },
            ),
        ]
    } else {
        vec![
            Span::styled(
                format!(
                    "⇄ {}",
                    if app.playback.shuffle {
                        s.on_label
                    } else {
                        s.off
                    }
                ),
                shuffle_style,
            ),
            Span::styled(" · ", Style::default().fg(app.theme.border)),
            Span::styled(
                format!(
                    "↻ {}",
                    sonata_runtime::i18n::repeat_mode_label(
                        app.config.language,
                        app.playback.repeat
                    )
                ),
                repeat_style,
            ),
        ]
    };
    if (app.playback_speed - 1.0).abs() > 0.01 {
        mode_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        mode_spans.push(Span::styled(
            format!("{:.2}x", app.playback_speed),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.skip_silence {
        mode_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        mode_spans.push(Span::styled(
            s.skip_label,
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(rem) = app.sleep_timer_remaining_secs() {
        mode_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        mode_spans.push(Span::styled(
            format!("⏲ {}", crate::app::playback::format_sleep_countdown(rem)),
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.config.connect_enabled {
        let conn_txt = if let Some(port) = app.connect_http_port {
            if !app.connect_paired_devices.is_empty() {
                format!("◎ {}:{}", s.connect_short, app.connect_paired_devices.len())
            } else {
                format!("◎ :{port}")
            }
        } else {
            format!("◎ {}", s.connect_short)
        };
        mode_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        mode_spans.push(Span::styled(
            conn_txt,
            Style::default()
                .fg(if app.connect_handle.is_some() {
                    app.theme.success
                } else {
                    app.theme.muted
                })
                .add_modifier(Modifier::BOLD),
        ));
    }
    let modes_line = Line::from(mode_spans);
    frame.render_widget(
        Paragraph::new(modes_line).alignment(Alignment::Center),
        bot_cols[1],
    );

    let has_update = app
        .update_info
        .as_ref()
        .is_some_and(|info| info.update_available);
    let right_footer = if !app.status_line.trim().is_empty() {
        let max_chars = bot_cols[2].width.saturating_sub(1) as usize;
        let clean_status = sanitize_tui_text(app.status_line.trim());
        let status = truncate_footer_text(&clean_status, max_chars);
        Line::from(Span::styled(
            format!("{status} "),
            Style::default().fg(app.theme.fg),
        ))
    } else if bot_cols[2].width >= 36 {
        let mut spans = Vec::new();
        if has_update {
            spans.push(Span::styled(
                format!("● {} · ", s.update_label),
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.extend([
            Span::styled(
                format!("{} ", s.theme_label),
                Style::default().fg(app.theme.muted),
            ),
            Span::styled("(t)", Style::default().fg(app.theme.fg)),
            Span::styled(
                format!(" · {} ", s.settings),
                Style::default().fg(app.theme.muted),
            ),
            Span::styled("(S)", Style::default().fg(app.theme.fg)),
            Span::styled(
                format!(" · {} ", s.keys_label),
                Style::default().fg(app.theme.muted),
            ),
            Span::styled("(k) ", Style::default().fg(app.theme.fg)),
        ]);
        Line::from(spans)
    } else {
        let mut spans = Vec::new();
        if has_update {
            spans.push(Span::styled(
                "● ",
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.extend([
            Span::styled("(t)", Style::default().fg(app.theme.fg)),
            Span::styled(" · ", Style::default().fg(app.theme.muted)),
            Span::styled("(S)", Style::default().fg(app.theme.fg)),
            Span::styled(" · ", Style::default().fg(app.theme.muted)),
            Span::styled("(k) ", Style::default().fg(app.theme.fg)),
        ]);
        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(right_footer).alignment(Alignment::Right),
        bot_cols[2],
    );
}

fn truncate_footer_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn slider_line(
    ratio: f64,
    width: usize,
    fill_fg: Color,
    thumb_fg: Color,
    empty_fg: Color,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    let thumb_idx = ((ratio.clamp(0.0, 1.0) * (width.saturating_sub(1)) as f64).round() as usize)
        .min(width - 1);
    let mut spans = Vec::with_capacity(3);
    if thumb_idx > 0 {
        spans.push(Span::styled(
            "━".repeat(thumb_idx),
            Style::default().fg(fill_fg),
        ));
    }
    spans.push(Span::styled("●", Style::default().fg(thumb_fg)));
    let rest = width.saturating_sub(thumb_idx + 1);
    if rest > 0 {
        spans.push(Span::styled(
            "─".repeat(rest),
            Style::default().fg(empty_fg),
        ));
    }
    Line::from(spans)
}

fn render_progress_row(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let pos = app.interpolated_position();
    let dur = app.playback.duration_secs.max(1.0);
    let ratio = (pos / dur).clamp(0.0, 1.0);
    let left_label = format!(" {:>5} ", format_duration(Some(pos as u64)));
    let right_label = format!(
        " {:<5} ",
        format_duration(Some(app.playback.duration_secs as u64))
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_label.chars().count() as u16),
            Constraint::Min(8),
            Constraint::Length(right_label.chars().count() as u16),
        ])
        .split(area);

    app.ui_rects.progress_bar = cols[1];

    frame.render_widget(
        Paragraph::new(Span::styled(left_label, Style::default().fg(app.theme.fg))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(slider_line(
            ratio,
            cols[1].width as usize,
            app.theme.accent,
            app.theme.highlight_fg,
            app.theme.border,
        )),
        cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            right_label,
            Style::default().fg(app.theme.muted),
        )),
        cols[2],
    );
}

#[cfg(test)]
mod tests {
    use super::{
        is_auto_compact, mini_layout_tier, sanitize_tui_text, slider_line, MiniLayoutTier,
    };
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    #[test]
    fn slider_line_renders_thumb_at_exact_endpoints_and_midpoint() {
        let start = slider_line(0.0, 10, Color::Red, Color::White, Color::Gray);
        assert_eq!(start.to_string(), "●─────────");

        let mid = slider_line(0.5, 11, Color::Red, Color::White, Color::Gray);
        assert_eq!(mid.to_string(), "━━━━━●─────");

        let end = slider_line(1.0, 10, Color::Red, Color::White, Color::Gray);
        assert_eq!(end.to_string(), "━━━━━━━━━●");
    }

    #[test]
    fn auto_compact_and_mini_tiers_follow_responsive_thresholds() {
        assert!(!is_auto_compact(Rect::new(0, 0, 90, 20)));
        assert!(is_auto_compact(Rect::new(0, 0, 89, 20)));
        assert!(is_auto_compact(Rect::new(0, 0, 90, 19)));

        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 60, 13)),
            MiniLayoutTier::SideBySide
        );
        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 59, 13)),
            MiniLayoutTier::ArtworkOrLyric
        );
        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 60, 12)),
            MiniLayoutTier::ArtworkOrLyric
        );
        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 36, 10)),
            MiniLayoutTier::ArtworkOrLyric
        );
        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 35, 10)),
            MiniLayoutTier::UltraCompact
        );
        assert_eq!(
            mini_layout_tier(Rect::new(0, 0, 36, 9)),
            MiniLayoutTier::UltraCompact
        );
    }

    #[test]
    fn sanitize_tui_text_strips_ansi_control_and_bidi_overrides() {
        assert_eq!(
            sanitize_tui_text("\x1b[31mRed\x1b[0m \x1b[2J\x1b[HTitle"),
            "Red Title"
        );
        assert_eq!(
            sanitize_tui_text("\x1b]0;Pwned Title\x07Song \x1b]52;c;ZGF0YQ==\x1b\\Name"),
            "Song Name"
        );
        assert_eq!(
            sanitize_tui_text("\x1b_Ga=T,f=100;AAAA\x1b\\Clean"),
            "Clean"
        );
        assert_eq!(
            sanitize_tui_text("a\x00b\rc\nd\x1fe\x7ff\u{0085}g\u{009b}31mh"),
            "abcdefgh"
        );
        assert_eq!(
            sanitize_tui_text("safe\u{202E}evil\u{202C}\u{2066}iso\u{2069}ok"),
            "safeevilisook"
        );
        assert_eq!(
            sanitize_tui_text("İstanbul şarkı · こんにちは 🎵"),
            "İstanbul şarkı · こんにちは 🎵"
        );
    }
}
