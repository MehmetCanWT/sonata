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
mod mpris;
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
use std::io::{self, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

fn restore_terminal_state(clear_kitty: bool) {
    let mut stdout = io::stdout();
    if clear_kitty {
        let _ = stdout.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\");
        let _ = stdout.flush();
    }
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    );
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    clear_kitty: bool,
    restored: bool,
}

impl TerminalGuard {
    fn enter(clear_kitty: bool) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            restore_terminal_state(clear_kitty);
            return Err(err.into());
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(term) => term,
            Err(err) => {
                restore_terminal_state(clear_kitty);
                return Err(err.into());
            }
        };
        if let Err(err) = terminal.clear() {
            restore_terminal_state(clear_kitty);
            return Err(err.into());
        }
        Ok(Self {
            terminal,
            clear_kitty,
            restored: false,
        })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        restore_terminal_state(self.clear_kitty);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

struct InputThreadGuard {
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl InputThreadGuard {
    fn shutdown_and_join(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for InputThreadGuard {
    fn drop(&mut self) {
        self.shutdown_and_join();
    }
}

#[cfg(unix)]
fn redirect_tui_diagnostics() -> io::Result<()> {
    use sonata_core::auth::ensure_private_dir;
    use std::fs::{self, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

    let cache_dir = AppConfig::cache_dir();
    let _ = ensure_private_dir(&cache_dir);
    let log_path = cache_dir.join("sonata-tui.log");
    let truncate = fs::metadata(&log_path).is_ok_and(|metadata| metadata.len() > MAX_LOG_BYTES);
    let mut options = OpenOptions::new();
    options.create(true).write(true).mode(0o600);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }
    let log_file = options
        .open(&log_path)
        .or_else(|_| OpenOptions::new().write(true).open("/dev/null"))?;
    let _ = fs::set_permissions(&log_path, fs::Permissions::from_mode(0o600));

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

    let is_kitty = picker.protocol_type() == ProtocolType::Kitty;
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal_state(is_kitty);
        tracing::error!(%info, "sonata TUI panicked");
        orig_hook(info);
    }));

    let mut term_guard = TerminalGuard::enter(is_kitty)?;

    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    let (mpv_tx, mpv_rx) = mpsc::channel(64);

    let (driver, mpv_warning): (Arc<dyn PlaybackAudioDriver>, Option<String>) =
        match MpvController::spawn(config.volume, mpv_tx).await {
            Ok(ctrl) => (Arc::new(ctrl), None),
            Err(e) => {
                let s = sonata_runtime::i18n::strings(config.language);
                (
                    Arc::new(NoopAudioDriver),
                    Some(format!("{}: {e}", s.warning_label)),
                )
            }
        };

    let (mut app, mut session_rx) =
        App::new(config.clone(), picker, app_tx.clone(), driver, mpv_rx);
    if let Some(warning) = mpv_warning {
        app.status_line = warning;
    }

    app.start_background_init();
    app.flush_session_commands().await;
    if app.connect_sync_needed {
        app.sync_connect_server().await;
    }

    let input_shutdown = Arc::new(AtomicBool::new(false));
    let input_shutdown_flag = Arc::clone(&input_shutdown);
    let (term_tx, mut term_rx) = mpsc::unbounded_channel::<Event>();
    let input_handle = std::thread::Builder::new()
        .name("sonata-tui-input".into())
        .spawn(move || {
            while !input_shutdown_flag.load(Ordering::Relaxed) {
                match event::poll(std::time::Duration::from_millis(100)) {
                    Ok(true) => match event::read() {
                        Ok(ev) => {
                            if term_tx.send(ev).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        })?;
    let mut input_guard = InputThreadGuard {
        shutdown: input_shutdown,
        handle: Some(input_handle),
    };

    let mut fast_tick = tokio::time::interval(std::time::Duration::from_millis(16));
    fast_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut calm_tick = tokio::time::interval(std::time::Duration::from_millis(250));
    calm_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    #[cfg(unix)]
    let (mut sigterm, mut sighup) = (
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?,
    );

    term_guard
        .terminal_mut()
        .draw(|f| ui::render(f, &mut app))?;

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
                    term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                    app.sync_kitty_artwork();
                    if (app.playback.position_secs.floor() as i64) != prev_sec {
                        app.export_now_playing();
                    }
                }
            }
            _ = calm_tick.tick(), if (playing || app.sleep_timer_deadline.is_some()) && !needs_fast_tick => {
                let tick_dirty = if playing { app.tick_playback() } else { false };
                if tick_dirty || app.sleep_timer_deadline.is_some() {
                    term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                    app.sync_kitty_artwork();
                    app.export_now_playing();
                }
            }
            Some(ccmd) = app::recv_connect_cmd(&mut app.connect_cmd_rx) => {
                app.handle_connect_command(ccmd);
                app.flush_session_commands().await;
                app.publish_connect_snapshot();
                term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            Ok(()) = app::recv_connect_pair_change(&mut app.connect_pair_rx) => {
                app.refresh_connect_pair_state();
                term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
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
                        let _ = term_guard.terminal_mut().clear();
                        app.kitty_drawn_track = None;
                    }
                    _ => continue,
                }
                app.flush_session_commands().await;
                if app.connect_sync_needed {
                    app.sync_connect_server().await;
                }
                app.publish_connect_snapshot();
                term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            Some(sev) = session_rx.recv() => {
                app.handle_session_event(sev);
                while let Ok(more) = session_rx.try_recv() {
                    app.handle_session_event(more);
                }
                app.flush_session_commands().await;
                app.publish_connect_snapshot();
                term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
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
                if app.connect_sync_needed {
                    app.sync_connect_server().await;
                }
                app.publish_connect_snapshot();
                term_guard.terminal_mut().draw(|f| ui::render(f, &mut app))?;
                app.sync_kitty_artwork();
                app.export_now_playing();
            }
            else => break,
        }
    }

    input_guard.shutdown_and_join();
    app.clear_kitty_images();
    app.clear_now_playing();
    app.restore_hyprland_opacity().await;
    let final_cfg = app.config.clone();
    let _ = tokio::task::spawn_blocking(move || final_cfg.save()).await;
    app.shutdown_playback_tasks().await;
    term_guard.restore();

    Ok(())
}
