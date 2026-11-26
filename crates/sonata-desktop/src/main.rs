//! Cross-platform Tauri v2 entry point of Sonata Desktop: frameless/overlay window, tray, and backend lifecycle.
//!
//! 1. **CLI**: handles `-h`/`--help` and `-v`/`--version` flags cleanly before launching the webview runtime.
//! 2. **Tauri Setup**: configures macOS `TitleBarStyle::Overlay` vs Windows/Linux window, sets up the native
//!    system tray (`ksni` StatusNotifierItem on Linux, `TrayIconBuilder` on macOS/Windows), and spawns `BackendHandle`.
//! 3. **Window & Signal Coordination**: routes tray/MPRIS `UiWindowCmd` actions and SIGINT/SIGTERM clean shutdowns.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod integrations;
mod ipc;
mod settings;

use crate::backend::{spawn_backend, BackendCmd, UiWindowCmd};
use crate::integrations::tray::is_tray_active;
use crate::ipc::{
    dispatch_cmd, get_desktop_settings, get_platform, should_show_window_controls, subscribe_state,
    update_desktop_settings, window_close, window_minimize, window_start_dragging,
    window_toggle_fullscreen, window_toggle_maximize, AppState,
};
use crate::settings::DesktopSettings;
use sonata_runtime::config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, RunEvent, WindowEvent};
use tokio::sync::mpsc;

static EXPLICIT_QUIT: AtomicBool = AtomicBool::new(false);
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

#[cfg(not(target_os = "linux"))]
use tauri::AppHandle;

#[cfg(not(target_os = "linux"))]
use crate::backend::BackendHandle;
#[cfg(not(target_os = "linux"))]
use crate::integrations::tray::{clear_tray_sink, register_tray_sink, TrayState};
#[cfg(not(target_os = "linux"))]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[cfg(not(target_os = "linux"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

fn print_help() {
    println!(
        "sonata-desktop {}\n\
         Cross-platform Tauri v2 Sonata music player (Linux, macOS, Windows).\n\n\
         USAGE:\n    sonata-desktop [OPTIONS]\n\n\
         OPTIONS:\n    -h, --help        Print help information\n    -v, --version     Print version information\n\n\
         PATHS:\n    Core config:      {}\n    Desktop UI:       {}",
        env!("CARGO_PKG_VERSION"),
        AppConfig::config_path().display(),
        DesktopSettings::config_path().display(),
    );
}

#[cfg(unix)]
fn spawn_signal_watcher(win_tx: mpsc::UnboundedSender<UiWindowCmd>) {
    if let Err(err) = std::thread::Builder::new()
        .name("sonata-signals".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async move {
                let Ok(mut sigterm) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                else {
                    return;
                };
                let Ok(mut sigint) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                else {
                    return;
                };
                tokio::select! {
                    _ = sigterm.recv() => {}
                    _ = sigint.recv() => {}
                }
                let _ = win_tx.send(UiWindowCmd::QuitApp);
            });
        })
    {
        tracing::warn!(?err, "failed to spawn signal watcher thread");
    }
}

#[cfg(not(target_os = "linux"))]
fn setup_tray_icon(
    app: &AppHandle,
    backend: BackendHandle,
    win_tx: mpsc::UnboundedSender<UiWindowCmd>,
    initial_language: sonata_runtime::i18n::AppLanguage,
) -> tauri::Result<()> {
    let init_s = sonata_runtime::i18n::strings(initial_language);
    let show_item = MenuItem::with_id(app, "show_hide", init_s.tray_show_hide, true, None::<&str>)?;
    let play_item = MenuItem::with_id(
        app,
        "play_pause",
        init_s.tray_play_pause,
        true,
        None::<&str>,
    )?;
    let next_item = MenuItem::with_id(app, "next", init_s.next_track, true, None::<&str>)?;
    let prev_item = MenuItem::with_id(app, "prev", init_s.previous_track, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", init_s.tray_quit, true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &PredefinedMenuItem::separator(app)?,
            &play_item,
            &next_item,
            &prev_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("sonata-tray")
        .menu(&menu)
        .tooltip("Sonata")
        .show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let backend_menu = backend.clone();
    let win_menu = win_tx.clone();
    let win_click = win_tx;
    let tray = builder
        .on_menu_event(move |_app, event| match event.id.as_ref() {
            "show_hide" => {
                let _ = win_menu.send(UiWindowCmd::ToggleWindow);
            }
            "play_pause" => backend_menu.send(BackendCmd::TogglePause),
            "next" => backend_menu.send(BackendCmd::NextTrack),
            "prev" => backend_menu.send(BackendCmd::PrevTrack),
            "quit" => {
                let _ = win_menu.send(UiWindowCmd::QuitApp);
            }
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = win_click.send(UiWindowCmd::ToggleWindow);
            }
        })
        .build(app)?;

    let tray_menu_item = play_item;
    let show_menu_item = show_item;
    let next_menu_item = next_item;
    let prev_menu_item = prev_item;
    let quit_menu_item = quit_item;
    register_tray_sink(move |state: &TrayState| {
        let s = sonata_runtime::i18n::strings(state.language);
        let tip = match (&state.title, &state.artist) {
            (Some(title), Some(artist)) if !artist.is_empty() => format!("{title} — {artist}"),
            (Some(title), _) => title.clone(),
            _ => "Sonata".to_string(),
        };
        let _ = tray.set_tooltip(Some(&tip));
        let _ = show_menu_item.set_text(if state.window_visible {
            s.tray_hide_window
        } else {
            s.tray_show_window
        });
        let _ = tray_menu_item.set_text(if state.paused || !state.has_track {
            s.play
        } else {
            s.pause
        });
        let _ = next_menu_item.set_text(s.next_track);
        let _ = prev_menu_item.set_text(s.previous_track);
        let _ = quit_menu_item.set_text(s.tray_quit);
        let _ = tray_menu_item.set_enabled(state.has_track);
        let _ = next_menu_item.set_enabled(state.has_track);
        let _ = prev_menu_item.set_enabled(state.has_track);
    });

    Ok(())
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("GST_PLUGIN_FEATURE_RANK").is_none() {
        // Avoid NVDEC/VA-API zero-copy NV12 green chroma glitches in WebKitGTK <video>.
        std::env::set_var(
            "GST_PLUGIN_FEATURE_RANK",
            "avdec_h264:MAX,nvh264dec:NONE,nvh265dec:NONE,vah264dec:NONE,vah265dec:NONE,vaapih264dec:NONE,vaapidecodebin:NONE",
        );
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("sonata-desktop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let _telemetry_guard = sonata_runtime::telemetry::init("desktop");
    let desktop_settings = DesktopSettings::load();

    let (win_tx, mut win_rx) = mpsc::unbounded_channel::<UiWindowCmd>();
    #[cfg(unix)]
    spawn_signal_watcher(win_tx.clone());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            subscribe_state,
            dispatch_cmd,
            get_desktop_settings,
            update_desktop_settings,
            get_platform,
            should_show_window_controls,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_start_dragging,
            window_toggle_fullscreen,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !EXPLICIT_QUIT.load(Ordering::Relaxed) && is_tray_active() {
                    api.prevent_close();
                    let _ = window.hide();
                    WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        state.backend.send(BackendCmd::SetWindowVisible(false));
                    }
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            if let Some(win) = handle.get_webview_window("main") {
                let _ = win.set_decorations(true);
                let _ = win.set_title_bar_style(tauri::TitleBarStyle::Overlay);
            }

            #[cfg(target_os = "windows")]
            if let Some(win) = handle.get_webview_window("main") {
                if let Ok(hwnd) = win.hwnd() {
                    crate::integrations::mpris::set_media_window_hwnd(hwnd.0 as isize);
                }
            }

            let backend = spawn_backend(win_tx.clone())
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            backend.send(BackendCmd::SetWindowVisible(true));

            #[cfg(not(target_os = "linux"))]
            if let Err(err) = setup_tray_icon(
                &handle,
                backend.clone(),
                win_tx.clone(),
                desktop_settings.language,
            ) {
                tracing::warn!(?err, "failed to create system tray icon");
            }

            app.manage(AppState {
                backend: backend.clone(),
                win_tx: win_tx.clone(),
                settings: Mutex::new(desktop_settings.clone()),
            });

            let app_win = handle.clone();
            let backend_win = backend;
            tauri::async_runtime::spawn(async move {
                while let Some(cmd) = win_rx.recv().await {
                    match cmd {
                        UiWindowCmd::ShowWindow => {
                            if let Some(win) = app_win.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.unminimize();
                                let _ = win.set_focus();
                            }
                            WINDOW_VISIBLE.store(true, Ordering::Relaxed);
                            backend_win.send(BackendCmd::SetWindowVisible(true));
                        }
                        UiWindowCmd::HideWindow => {
                            if let Some(win) = app_win.get_webview_window("main") {
                                let _ = win.hide();
                            }
                            WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                            backend_win.send(BackendCmd::SetWindowVisible(false));
                        }
                        UiWindowCmd::ToggleWindow => {
                            if let Some(win) = app_win.get_webview_window("main") {
                                let visible = WINDOW_VISIBLE.load(Ordering::Relaxed);
                                if visible {
                                    let _ = win.hide();
                                    WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                                    backend_win.send(BackendCmd::SetWindowVisible(false));
                                } else {
                                    let _ = win.show();
                                    let _ = win.unminimize();
                                    let _ = win.set_focus();
                                    WINDOW_VISIBLE.store(true, Ordering::Relaxed);
                                    backend_win.send(BackendCmd::SetWindowVisible(true));
                                }
                            }
                        }
                        UiWindowCmd::QuitApp => {
                            EXPLICIT_QUIT.store(true, Ordering::Relaxed);
                            #[cfg(not(target_os = "linux"))]
                            clear_tray_sink();
                            backend_win.shutdown_sync();
                            app_win.exit(0);
                            break;
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if !EXPLICIT_QUIT.load(Ordering::Relaxed) && is_tray_active() {
                api.prevent_exit();
                return;
            }
            #[cfg(not(target_os = "linux"))]
            clear_tray_sink();
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.backend.shutdown_sync();
            }
        }
        RunEvent::Exit => {
            #[cfg(not(target_os = "linux"))]
            clear_tray_sink();
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.backend.shutdown_sync();
            }
        }
        _ => {}
    });

    Ok(())
}
