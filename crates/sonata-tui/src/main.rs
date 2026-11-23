//! Entry point for the Sonata terminal UI.
//!
//! 1. **CLI**: handles `--version` and `--help` before touching the terminal.
//! 2. **Diagnostics**: redirects stderr and initializes persistent telemetry so logs never corrupt the screen.
//! 3. **Terminal Setup**: detects the graphics protocol, enters raw mode and the alternate screen with
//!    mouse capture, and installs a panic hook that restores the terminal.
//! 4. **Runtime Wiring**: creates the `App`, spawns the mpv controller, and connects app and mpv event channels.
//! 5. **Event Loop**: multiplexes input, app events, mpv events, and timer ticks, then shuts down
//!    playback tasks and restores the terminal on exit.

mod app;
mod input;
mod theme;
mod theme_tui;
mod ui;

use anyhow::Result;
use app::{App, AppEvent};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui_image::picker::{Picker, ProtocolType};
use sonata_runtime::config::AppConfig;
use sonata_runtime::player::{MpvController, NoopAudioDriver, PlaybackAudioDriver};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(unix)]
fn redirect_tui_diagnostics() -> io::Result<()> {
    use std::fs::{self, OpenOptions};
    use std::os::fd::AsRawFd;

    const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

    let log_path = AppConfig::cache_dir().join("sonata-tui.log");
    let _ = fs::create_dir_all(AppConfig::cache_dir());
    let truncate = fs::metadata(&log_path).is_ok_and(|metadata| metadata.len() > MAX_LOG_BYTES);
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }
    let log_file = options
        .open(log_path)
        .or_else(|_| OpenOptions::new().write(true).open("/dev/null"))?;

    // Keep async diagnostics out of the terminal's alternate screen.
    // SAFETY: both descriptors are valid for this call. `dup2` duplicates the open
    // file descriptor before `log_file` is dropped, and STDERR_FILENO is process-owned.
    let result = unsafe { libc::dup2(log_file.as_raw_fd(), libc::STDERR_FILENO) };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn redirect_tui_diagnostics() -> io::Result<()> {
    Ok(())
}

fn detect_picker() -> Picker {
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    if picker.protocol_type() == ProtocolType::Halfblocks {
        let term = std::env::var("TERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let prog = std::env::var("TERM_PROGRAM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if term.contains("ghostty")
            || prog.contains("ghostty")
            || term.contains("kitty")
            || std::env::var_os("KITTY_WINDOW_ID").is_some()
        {
            picker.set_protocol_type(ProtocolType::Kitty);
        }
    }
    picker
}

fn print_help() {
    println!(
        "sonata {}\n\
         Fast, modular terminal music player for YouTube Music and Spotify.\n\n\
         USAGE:\n    sonata [OPTIONS]\n\n\
         OPTIONS:\n    -h, --help       Print help information\n    -v, --version    Print version information\n\n\
         CONFIG:\n    Config file:     {}\n    Custom themes:   {}/*.toml",
        env!("CARGO_PKG_VERSION"),
        AppConfig::config_path().display(),
        AppConfig::themes_dir().display()
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("sonata {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let redirect_error = redirect_tui_diagnostics().err();
    let _telemetry_guard = sonata_runtime::telemetry::init("tui");
    if let Some(error) = redirect_error {
        tracing::warn!(%error, "TUI diagnostic redirection failed");
    }
    let config = AppConfig::load();
    // Query terminal graphics protocol and font metrics before entering alternate screen
    let picker = detect_picker();

    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
        orig_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    let (mpv_tx, mpv_rx) = mpsc::channel(64);

    let (driver, mpv_warning): (Arc<dyn PlaybackAudioDriver>, Option<String>) =
        match MpvController::spawn(config.volume, mpv_tx).await {
            Ok(ctrl) => (Arc::new(ctrl), None),
            Err(e) => (Arc::new(NoopAudioDriver), Some(format!("Warning: {e}"))),
        };

    let (mut app, mut session_rx) =
        App::new(config.clone(), picker, app_tx.clone(), driver, mpv_rx);
    if let Some(warning) = mpv_warning {
        app.status_line = warning;
    }

    app.start_background_init();

    let (term_tx, mut term_rx) = mpsc::unbounded_channel::<Event>();
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if term_tx.send(ev).is_err() {
                break;
            }
        }
    });

    let mut fast_tick = tokio::time::interval(std::time::Duration::from_millis(16));
    fast_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut calm_tick = tokio::time::interval(std::time::Duration::from_millis(250));
    calm_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    #[cfg(unix)]
    let (mut sigterm, mut sighup) = (
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?,
    );

    terminal.draw(|f| ui::render(f, &mut app))?;

    while !app.should_quit {
        #[cfg(unix)]
        let term_sig = async {
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sighup.recv() => {}
            }
        };
        #[cfg(not(unix))]
        let term_sig = std::future::pending::<()>();

        let playing = !app.playback.paused
            && !app.playback.loading_track
            && app.playback.current_track().is_some();
        let has_synced_lyrics = matches!(
            app.lyrics,
            sonata_core::lyrics::LyricsState::Synced(ref lines) if !lines.is_empty()
        );
        let needs_fast_tick = app.fx.is_running()
            || app.is_lyrics_scrolling()
            || (playing && !app.anim_art_frames.is_empty())
            || (playing && has_synced_lyrics);

        tokio::select! {
            _ = term_sig => {
                app.should_quit = true;
            }
            _ = fast_tick.tick(), if needs_fast_tick => {
                let prev_sec = app.playback.position_secs.floor() as i64;
                let playback_dirty = if playing { app.tick_playback() } else { false };
                if playback_dirty || app.fx.is_running() || app.is_lyrics_scrolling() {
                    terminal.draw(|f| ui::render(f, &mut app))?;
                    app.sync_kitty_artwork();
                    if (app.playback.position_secs.floor() as i64) != prev_sec {
                        app.export_now_playing();
                    }
                }
            }
            _ = calm_tick.tick(), if playing && !needs_fast_tick => {
                if app.tick_playback() {
                    terminal.draw(|f| ui::render(f, &mut app))?;
                    app.sync_kitty_artwork();
                    app.export_now_playing();
                }
            }
            Some(ev) = term_rx.recv() => {
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        input::handle_key(&mut app, key);
                    }
                    Event::Mouse(mouse) => {
                        input::handle_mouse(&mut app, mouse);
                    }
                    Event::Resize(_, _) => {
                        app.clear_kitty_images();
                        let _ = terminal.clear();
                        app.kitty_drawn_track = None;
                    }
                    _ => continue,
                }
                app.flush_session_commands().await;
                terminal.draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            Some(sev) = session_rx.recv() => {
                app.handle_session_event(sev);
                while let Ok(more) = session_rx.try_recv() {
                    app.handle_session_event(more);
                }
                app.flush_session_commands().await;
                terminal.draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            Some(aev) = app_rx.recv() => {
                app.handle_app_event(aev);
                // Drain queued updates so we don't redraw like a caffeinated squirrel
                while let Ok(more) = app_rx.try_recv() {
                    app.handle_app_event(more);
                }
                app.flush_session_commands().await;
                terminal.draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            else => break,
        }
    }

    app.clear_kitty_images();
    app.clear_now_playing();
    app.restore_hyprland_opacity();
    let _ = app.config.save();
    app.shutdown_playback_tasks().await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
