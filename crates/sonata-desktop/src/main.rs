//! Entry point of the Sonata native GPUI desktop client: CLI flags, frameless blurred window, and shutdown.
//!
//! 1. **CLI**: parses `-h/--help`, `-v/--version`, `--decorated` (escape hatch for native WM titlebars), and `--scheme <name>` (one-run CSS theme override).
//! 2. **Assets & Fonts**: layers `SonataIcons` (`icon_assets!`) + embedded `Inter` faces over `gpui_kit` defaults.
//! 3. **Window Lifecycle**: opens a client-decorated (`WindowDecorations::Client`) compositor-blurred window (`WindowBackgroundAppearance::Blurred` / `Transparent` / `Opaque`) backed by `WindowBorder` + `TitleBar`.
//! 4. **Signals & IPC**: handles SIGINT/SIGTERM, tray, and MPRIS window visibility commands cleanly.

mod backend;
mod integrations;
mod settings;
mod theme;
mod ui;
mod views;

use crate::backend::{spawn_backend, BackendCmd, BackendHandle, UiWindowCmd};
use crate::settings::{
    DesktopSettings, WINDOW_DEFAULT_H, WINDOW_DEFAULT_W, WINDOW_MIN_H, WINDOW_MIN_W,
};
use crate::theme::{sync_gpui_theme, ThemeCatalog};
use crate::views::SonataDesktopView;
use gpui_kit::assets::{icon_assets, Assets};
use gpui_kit::component::TitleBar;
use gpui_kit::{
    point, px, size, AnyWindowHandle, App, AppContext, AssetSource, Bounds, QuitMode, SharedString,
    TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions,
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
        ArrowDown,
        ArrowUp,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronUp,
        CircleAlert,
        Clock,
        Compass,
        Copy,
        Disc3,
        Download,
        Ellipsis,
        ExternalLink,
        Folder,
        Globe,
        GripVertical,
        Heart,
        House,
        Library,
        ListFilter,
        ListMusic,
        ListPlus,
        LogIn,
        LogOut,
        Maximize2,
        MicVocal,
        Minimize2,
        Minus,
        Monitor,
        Moon,
        Music2,
        Palette,
        PanelRight,
        Pause,
        Pencil,
        Play,
        Plus,
        Radio,
        RefreshCw,
        Repeat,
        Repeat1,
        Search,
        Settings,
        Share2,
        Shuffle,
        SkipBack,
        SkipForward,
        SlidersHorizontal,
        Sparkles,
        Square,
        Sun,
        ThumbsDown,
        ThumbsUp,
        Trash,
        User,
        Volume1,
        Volume2,
        VolumeX,
        X,
    ]
);

#[derive(Clone, Copy, Debug, Default)]
struct SonataAssets;

/// Embedded `Inter` font faces (SIL OFL 1.1).
pub(crate) const INTER_FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
];

pub(crate) const LOSSLESS_WAVE_ICON_PATH: &str = "icons/lossless.svg";

const LOSSLESS_WAVE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 53.36"><path fill="currentColor" d="M34.91,0.02 L35.64,0.00 L36.64,0.08 L37.74,0.28 L39.04,0.69 L40.34,1.30 L41.75,2.18 L42.90,3.12 L44.25,4.46 L45.37,5.82 L47.37,8.92 L48.16,10.32 L49.47,13.02 L50.67,15.82 L51.97,19.22 L53.27,22.93 L56.51,32.83 L58.03,37.23 L59.63,41.33 L60.93,44.03 L62.02,45.83 L62.63,46.63 L63.05,47.10 L63.55,47.57 L64.05,47.95 L64.65,48.27 L65.25,48.47 L65.85,48.53 L66.45,48.48 L67.35,48.18 L68.35,47.56 L69.15,46.86 L69.86,46.10 L69.96,46.15 L71.22,48.13 L72.32,49.63 L72.29,49.73 L71.36,50.68 L70.26,51.57 L69.45,52.07 L68.25,52.66 L67.35,52.96 L66.45,53.18 L65.05,53.34 L64.15,53.35 L63.05,53.26 L61.75,52.98 L60.85,52.67 L59.75,52.16 L58.75,51.56 L57.55,50.65 L56.66,49.83 L55.71,48.83 L54.81,47.73 L54.01,46.63 L52.62,44.43 L51.54,42.43 L50.42,40.13 L49.22,37.33 L48.02,34.23 L46.92,31.13 L43.07,19.32 L41.58,15.02 L40.08,11.32 L39.37,9.82 L38.47,8.22 L37.46,6.82 L36.54,5.89 L35.64,5.29 L34.74,4.98 L34.24,4.94 L33.64,4.98 L33.14,5.09 L32.64,5.28 L31.64,5.88 L31.04,6.37 L30.14,7.28 L30.04,7.25 L28.98,5.52 L27.71,3.82 L27.67,3.72 L27.83,3.52 L28.44,2.89 L29.14,2.29 L29.94,1.70 L30.94,1.11 L31.84,0.69 L32.74,0.38 L34.04,0.10 Z M54.04,0.02 L54.85,0.00 L55.85,0.08 L57.05,0.31 L58.45,0.78 L59.15,1.10 L60.05,1.60 L61.55,2.69 L62.76,3.82 L64.17,5.42 L65.48,7.22 L66.59,9.02 L67.89,11.42 L68.88,13.52 L70.19,16.62 L71.18,19.22 L72.48,22.93 L75.56,32.47 L75.69,32.23 L76.27,29.53 L76.77,26.73 L77.32,23.13 L77.64,22.03 L77.93,21.62 L78.26,21.41 L78.66,21.31 L79.16,21.29 L79.66,21.38 L79.96,21.50 L80.39,21.82 L80.68,22.23 L80.84,22.83 L80.80,23.53 L80.09,27.93 L78.89,33.93 L78.18,36.93 L77.71,38.63 L78.33,40.23 L79.12,42.03 L79.93,43.63 L80.63,44.83 L81.44,46.03 L82.26,46.99 L82.96,47.65 L83.86,48.25 L84.76,48.56 L85.46,48.60 L86.26,48.47 L86.96,48.15 L87.66,47.67 L88.46,46.89 L89.09,46.13 L89.89,44.93 L90.87,43.13 L91.90,40.83 L92.90,38.13 L93.79,35.33 L94.59,32.43 L95.40,29.03 L96.01,26.03 L96.52,22.73 L96.63,22.33 L96.86,21.92 L97.17,21.62 L97.56,21.40 L98.07,21.29 L98.47,21.30 L99.07,21.47 L99.45,21.72 L99.79,22.13 L99.99,22.63 L100.00,23.33 L99.11,28.73 L98.50,32.03 L97.80,35.23 L97.29,37.23 L96.69,39.33 L96.00,41.43 L95.39,43.03 L94.59,44.83 L93.98,46.03 L93.10,47.53 L92.18,48.83 L91.42,49.73 L90.26,50.87 L89.36,51.57 L88.26,52.24 L87.36,52.66 L86.46,52.97 L85.16,53.25 L84.06,53.35 L83.16,53.35 L82.06,53.25 L80.66,52.95 L79.76,52.66 L78.86,52.26 L77.96,51.76 L76.96,51.06 L75.89,50.13 L75.01,49.23 L74.33,48.43 L73.44,47.23 L72.23,45.33 L71.03,43.13 L69.72,40.43 L68.52,37.63 L67.43,34.83 L66.22,31.43 L62.49,20.02 L60.98,15.62 L59.98,13.02 L59.29,11.42 L58.47,9.72 L57.67,8.32 L56.67,6.92 L55.65,5.88 L54.75,5.29 L54.25,5.09 L53.75,4.98 L52.85,4.98 L52.35,5.09 L51.65,5.37 L50.65,6.00 L49.35,7.26 L49.25,7.27 L48.08,5.42 L46.84,3.72 L47.65,2.87 L48.45,2.18 L49.25,1.61 L50.15,1.09 L51.25,0.60 L52.25,0.29 L53.25,0.09 Z M15.00,0.12 L16.64,0.03 L17.54,0.09 L18.24,0.20 L19.04,0.38 L19.94,0.68 L21.44,1.40 L22.64,2.19 L24.14,3.48 L25.47,4.92 L26.76,6.62 L27.98,8.52 L29.19,10.72 L30.26,12.92 L31.67,16.22 L32.67,18.82 L33.78,21.92 L37.41,33.13 L39.01,37.83 L40.21,40.93 L40.92,42.53 L41.63,43.93 L42.72,45.73 L43.70,46.93 L44.25,47.45 L44.95,47.96 L45.35,48.16 L45.95,48.37 L46.45,48.46 L47.05,48.46 L47.65,48.35 L48.15,48.16 L48.85,47.78 L49.45,47.33 L50.75,46.10 L51.91,47.93 L53.17,49.63 L52.45,50.44 L51.25,51.45 L50.65,51.86 L49.55,52.46 L48.55,52.85 L47.75,53.08 L46.75,53.26 L45.55,53.36 L44.75,53.34 L43.45,53.18 L42.55,52.97 L41.65,52.66 L40.74,52.26 L39.84,51.75 L39.14,51.27 L38.24,50.56 L37.36,49.73 L36.42,48.73 L35.01,46.93 L33.41,44.43 L32.12,42.03 L30.82,39.23 L29.71,36.53 L28.41,33.03 L26.92,28.63 L24.44,20.90 L24.34,21.07 L24.20,21.62 L23.62,24.43 L23.01,27.93 L22.67,30.53 L22.45,31.23 L22.15,31.63 L21.84,31.87 L21.34,32.06 L21.04,32.09 L20.54,32.06 L20.04,31.88 L19.74,31.68 L19.41,31.33 L19.20,30.83 L19.15,30.33 L19.80,26.03 L20.41,22.73 L21.10,19.42 L22.29,14.72 L20.87,11.22 L20.15,9.72 L19.37,8.32 L18.46,7.02 L17.84,6.32 L17.14,5.70 L16.24,5.19 L15.54,4.98 L14.44,4.98 L13.64,5.18 L12.93,5.50 L11.93,6.20 L11.29,6.82 L10.52,7.72 L9.70,8.92 L8.70,10.72 L7.72,12.92 L6.90,15.12 L6.02,17.92 L5.30,20.62 L4.60,23.73 L3.99,27.03 L3.48,30.63 L3.37,31.03 L3.16,31.43 L2.83,31.76 L2.53,31.94 L2.13,32.07 L1.53,32.08 L1.03,31.96 L0.63,31.73 L0.32,31.43 L0.09,31.03 L0.00,30.73 L0.00,30.03 L0.89,24.33 L1.70,20.12 L2.41,17.12 L3.32,13.92 L4.11,11.62 L5.19,9.02 L6.10,7.22 L7.22,5.42 L8.20,4.12 L9.56,2.72 L10.83,1.71 L12.03,1.01 L13.34,0.48 L14.04,0.29 Z"/></svg>"#;

#[derive(Clone, Debug)]
struct LaunchOpts {
    decorated: bool,
    scheme_override: Option<String>,
}

fn parse_launch_opts(args: &[String]) -> LaunchOpts {
    let decorated = args.iter().any(|a| a == "--decorated");
    let mut scheme_override = None;
    let mut i = 0;
    while i < args.len() {
        if let Some(rest) = args[i].strip_prefix("--scheme=") {
            scheme_override = Some(rest.to_string());
        } else if args[i] == "--scheme" {
            if let Some(next) = args.get(i + 1) {
                scheme_override = Some(next.clone());
                i += 1;
            }
        }
        i += 1;
    }
    LaunchOpts {
        decorated,
        scheme_override,
    }
}

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
        if path == LOSSLESS_WAVE_ICON_PATH {
            return Ok(Some(Cow::Borrowed(LOSSLESS_WAVE_SVG)));
        }
        if let Some(bytes) = SonataIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut list = SonataIcons.list(path)?;
        if path == "icons" {
            list.push(SharedString::from(LOSSLESS_WAVE_ICON_PATH));
        }
        list.extend(Assets.list(path)?);
        Ok(list)
    }
}

fn print_help() {
    println!(
        "sonata-desktop {}\n\
         Native GPUI YouTube Music desktop client with compositor blur and live CSS themes.\n\n\
         USAGE:\n    sonata-desktop [OPTIONS]\n\n\
         OPTIONS:\n    --decorated       Use system window decorations instead of frameless client chrome\n    --scheme <NAME>   Override the CSS theme for this run (e.g. catppuccin-frappe)\n    -h, --help        Print help information\n    -v, --version     Print version information\n\n\
         PATHS:\n    Core config:      {}\n    Desktop UI:       {}\n    CSS Themes:       {}",
        env!("CARGO_PKG_VERSION"),
        AppConfig::config_path().display(),
        DesktopSettings::config_path().display(),
        DesktopSettings::themes_dir().display(),
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

pub(crate) fn background_appearance_for(settings: &DesktopSettings) -> WindowBackgroundAppearance {
    if !settings.is_transparent() {
        WindowBackgroundAppearance::Opaque
    } else if settings.blur {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Transparent
    }
}

fn open_main_window(
    backend: BackendHandle,
    launch: LaunchOpts,
    window_slot: Rc<RefCell<Option<AnyWindowHandle>>>,
    cx: &mut App,
) {
    let settings = DesktopSettings::load();
    let mut catalog = ThemeCatalog::load(launch.scheme_override.clone());
    let palette = catalog.resolve_palette(&settings);
    sync_gpui_theme(cx, &palette);

    let background = background_appearance_for(&settings);
    let decorations = if launch.decorated {
        WindowDecorations::Server
    } else {
        WindowDecorations::Client
    };

    let options = WindowOptions {
        app_id: Some("sonata-desktop".into()),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(WINDOW_DEFAULT_W), px(WINDOW_DEFAULT_H)),
            cx,
        ))),
        window_min_size: Some(size(px(WINDOW_MIN_W), px(WINDOW_MIN_H))),
        window_background: background,
        window_decorations: Some(decorations),
        titlebar: if launch.decorated {
            None
        } else {
            Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.0), px(20.0))),
            })
        },
        app_owns_titlebar_drag: !launch.decorated,
        ..TitleBar::window_options()
    };

    let backend_close = backend.clone();
    let backend_view = backend.clone();
    let slot_close = window_slot.clone();
    let scheme_override = launch.scheme_override;
    let decorated = launch.decorated;

    match gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title("Sonata");
        window.set_background_appearance(background);
        window.on_window_should_close(cx, move |_window, _cx| {
            *slot_close.borrow_mut() = None;
            backend_close.send(BackendCmd::SetWindowAddress(None));
            backend_close.send(BackendCmd::SetWindowVisible(false));
            true
        });
        cx.new(|cx| SonataDesktopView::new(backend_view, decorated, scheme_override, window, cx))
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

    let launch = parse_launch_opts(&args);
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

            let window_slot: Rc<RefCell<Option<AnyWindowHandle>>> = Rc::new(RefCell::new(None));
            open_main_window(backend.clone(), launch.clone(), window_slot.clone(), cx);

            let backend_loop = backend.clone();
            let slot_loop = window_slot.clone();
            let launch_loop = launch.clone();
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
                                open_main_window(
                                    backend_loop.clone(),
                                    launch_loop.clone(),
                                    slot_loop.clone(),
                                    cx,
                                );
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
                                open_main_window(
                                    backend_loop.clone(),
                                    launch_loop.clone(),
                                    slot_loop.clone(),
                                    cx,
                                );
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
