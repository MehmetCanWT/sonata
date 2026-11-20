mod backend;
mod integrations;
mod theme_bridge;
mod views;

use crate::backend::{spawn_backend, BackendCmd, BackendHandle, UiWindowCmd};
use crate::views::SonataDesktopView;
use gpui_kit::assets::{icon_assets, Assets};
use gpui_kit::{
    px, size, AnyWindowHandle, App, AppContext, AssetSource, Bounds, QuitMode, SharedString,
    TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions,
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
    let _ = std::thread::Builder::new()
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
        });
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
        window_min_size: Some(size(px(380.0), px(150.0))),
        window_background: background,
        titlebar: Some(TitlebarOptions {
            title: Some("Sonata".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        ..Default::default()
    };

    let backend_close = backend.clone();
    let backend_view = backend.clone();
    let slot_close = window_slot.clone();
    if let Ok((handle, _view)) = gpui_kit::open_window(options, cx, move |window, cx| {
        window.on_window_should_close(cx, move |_window, _cx| {
            *slot_close.borrow_mut() = None;
            backend_close.send(BackendCmd::SetWindowAddress(None));
            backend_close.send(BackendCmd::SetWindowVisible(false));
            true
        });
        cx.new(|cx| SonataDesktopView::new(backend_view, window, cx))
    }) {
        *window_slot.borrow_mut() = Some(handle);
        backend.send(BackendCmd::SetWindowVisible(true));
        let backend_hypr = backend;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            let addr = hyprland::find_window_address_for_app("sonata-desktop");
            backend_hypr.send(BackendCmd::SetWindowAddress(addr));
        });
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

    let (win_tx, mut win_rx) = mpsc::unbounded_channel::<UiWindowCmd>();
    let backend = spawn_backend(win_tx.clone())?;

    #[cfg(unix)]
    spawn_signal_watcher(win_tx);

    gpui_kit::application()
        .with_assets(SonataAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
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
