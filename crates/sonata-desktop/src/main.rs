//! Entry point of the Sonata native GPUI desktop client: CLI flags, window lifecycle, and shutdown.
//!
//! 1. **CLI**: `-h/--help` and `-v/--version` are handled before any runtime or window is created.
//! 2. **Assets**: `SonataAssets` layers the app icon set (`icon_assets!`) over the `gpui_kit` defaults.
//! 3. **Backend Bootstrap**: `spawn_backend` starts the playback/provider engine on its own thread and returns a `BackendHandle`.
//! 4. **Window Lifecycle**: `open_main_window` builds the blurred or opaque main window and records its Hyprland address; a `UiWindowCmd` loop (fed by the tray, MPRIS, and signals) shows, hides, toggles, or quits.
//! 5. **Signals**: a dedicated thread turns SIGINT/SIGTERM into `UiWindowCmd::QuitApp` for a clean backend shutdown.

mod backend;
mod integrations;
mod theme_bridge;
mod views;

use crate::backend::{spawn_backend, BackendCmd, BackendHandle, UiWindowCmd};
use crate::views::SonataDesktopView;
use gpui_kit::assets::{icon_assets, Assets};
use gpui_kit::{
    px, size, AnyWindowHandle, App, AppContext, AssetSource, Bounds, QuitMode, SharedString,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions,
};
use sonata_core::config::AppConfig;
use sonata_core::hyprland;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc;

icon_assets!(
    SonataIcons,
    [
        Play,
        Pause,
        SkipBack,
        SkipForward,
        Shuffle,
        Repeat,
        Repeat1,
        Heart,
        Volume2,
        Volume1,
        VolumeX,
        Music2,
        Disc3,
        Search,
        Settings,
        Plus,
        ListMusic,
        ListPlus,
        MicVocal,
        Minimize2,
        Maximize2,
        RefreshCw,
        Sparkles,
        X,
        House,
        LogIn,
        LogOut,
        ChevronDown,
        Globe,
        Link,
        Flame,
        Compass,
        CircleAlert,
    ]
);

#[derive(Clone, Copy, Debug, Default)]
struct SonataAssets;

/// Embedded `Inter` font faces (SIL OFL 1.1), shared with `fullscreen_fx::BlurredLyricCache`.
pub(crate) const INTER_FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
];

fn load_embedded_fonts(cx: &App) {
    let fonts = INTER_FONTS
        .iter()
        .map(|bytes| Cow::Borrowed(*bytes))
        .collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        tracing::warn!(?err, "failed to register embedded Inter fonts");
    }
}

impl AssetSource for SonataAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = SonataIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut list = SonataIcons.list(path)?;
        list.extend(Assets.list(path)?);
        Ok(list)
    }
}

fn print_help() {
    println!(
        "sonata-desktop {}\n\
         Native Wayland/Hyprland GPUI music player for YouTube Music and Spotify.\n\n\
         USAGE:\n    sonata-desktop [OPTIONS]\n\n\
         OPTIONS:\n    -h, --help       Print help information\n    -v, --version    Print version information\n\n\
         CONFIG:\n    Config file:     {}\n    Custom themes:   {}/*.toml",
        env!("CARGO_PKG_VERSION"),
        AppConfig::config_path().display(),
        AppConfig::themes_dir().display()
    );
}

#[cfg(unix)]
fn spawn_signal_watcher(win_tx: mpsc::UnboundedSender<UiWindowCmd>) {
    if let Err(err) = std::thread::Builder::new()
        .name("sonata-signals".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    tracing::warn!(?err, "failed to build signal watcher tokio runtime");
                    return;
                }
            };
            rt.block_on(async move {
                let mut sigterm =
                    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    {
                        Ok(sig) => sig,
                        Err(err) => {
                            tracing::warn!(?err, "failed to register SIGTERM handler");
                            return;
                        }
                    };
                let mut sigint =
                    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                    {
                        Ok(sig) => sig,
                        Err(err) => {
                            tracing::warn!(?err, "failed to register SIGINT handler");
                            return;
                        }
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

fn open_main_window(
    backend: BackendHandle,
    window_slot: Rc<RefCell<Option<AnyWindowHandle>>>,
    cx: &mut App,
) {
    let snap = backend.snapshot();
    let background = if snap.effective_transparent() {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    };

    let options = WindowOptions {
        app_id: Some("sonata-desktop".into()),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1240.0), px(800.0)),
            cx,
        ))),
        window_min_size: Some(size(px(280.0), px(110.0))),
        window_background: background,
        window_decorations: Some(WindowDecorations::Server),
        titlebar: None,
        ..Default::default()
    };

    let backend_close = backend.clone();
    let backend_view = backend.clone();
    let slot_close = window_slot.clone();
    match gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title("Sonata");
        window.on_window_should_close(cx, move |_window, _cx| {
            *slot_close.borrow_mut() = None;
            backend_close.send(BackendCmd::SetWindowAddress(None));
            backend_close.send(BackendCmd::SetWindowVisible(false));
            true
        });
        cx.new(|cx| SonataDesktopView::new(backend_view, window, cx))
    }) {
        Ok((handle, _view)) => {
            *window_slot.borrow_mut() = Some(handle);
            backend.send(BackendCmd::SetWindowVisible(true));
            let backend_hypr = backend;
            std::thread::spawn(move || {
                for _ in 0..20 {
                    std::thread::sleep(Duration::from_millis(150));
                    if let Some(addr) = hyprland::find_window_address_for_app("sonata-desktop") {
                        backend_hypr.send(BackendCmd::SetWindowAddress(Some(addr)));
                        break;
                    }
                }
            });
        }
        Err(err) => {
            tracing::warn!(?err, "failed to open main desktop window");
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("sonata-desktop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let _telemetry_guard = sonata_core::telemetry::init("desktop");

    let (win_tx, mut win_rx) = mpsc::unbounded_channel::<UiWindowCmd>();
    let backend = spawn_backend(win_tx.clone())?;

    #[cfg(unix)]
    spawn_signal_watcher(win_tx);

    gpui_kit::application()
        .with_assets(SonataAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            load_embedded_fonts(cx);
            gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);

            let window_slot: Rc<RefCell<Option<AnyWindowHandle>>> = Rc::new(RefCell::new(None));
            open_main_window(backend.clone(), window_slot.clone(), cx);

            let backend_loop = backend.clone();
            let slot_loop = window_slot.clone();
            cx.spawn(async move |cx| {
                while let Some(cmd) = win_rx.recv().await {
                    let should_break = matches!(cmd, UiWindowCmd::QuitApp);
                    cx.update(|cx| match cmd {
                        UiWindowCmd::ShowWindow => {
                            let existing = *slot_loop.borrow();
                            if let Some(handle) = existing {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.activate_window();
                                });
                            } else {
                                open_main_window(backend_loop.clone(), slot_loop.clone(), cx);
                            }
                        }
                        UiWindowCmd::HideWindow => {
                            let existing = slot_loop.borrow_mut().take();
                            if let Some(handle) = existing {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.remove_window();
                                });
                                backend_loop.send(BackendCmd::SetWindowAddress(None));
                                backend_loop.send(BackendCmd::SetWindowVisible(false));
                            }
                        }
                        UiWindowCmd::ToggleWindow => {
                            let existing = *slot_loop.borrow();
                            if let Some(handle) = existing {
                                slot_loop.borrow_mut().take();
                                let _ = handle.update(cx, |_, window, _| {
                                    window.remove_window();
                                });
                                backend_loop.send(BackendCmd::SetWindowAddress(None));
                                backend_loop.send(BackendCmd::SetWindowVisible(false));
                            } else {
                                open_main_window(backend_loop.clone(), slot_loop.clone(), cx);
                            }
                        }
                        UiWindowCmd::QuitApp => {
                            if let Some(handle) = slot_loop.borrow_mut().take() {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.remove_window();
                                });
                            }
                            backend_loop.shutdown_sync();
                            cx.quit();
                        }
                    });
                    if should_break {
                        break;
                    }
                }
            })
            .detach();
        });

    Ok(())
}
