//! Six-tab Settings view (`Appearance`, `Playback`, `Library`, `System`, `Account`, `About`) using `gpui-component` controls.
//!
//! 1. **Appearance**: Mode (`System`/`Dark`/`Light`), CSS Theme `Select` (`GTK`/`Qt` + loaded `.css` files + error/contrast banners + `Open Themes Folder`), Transparency `Slider`, `Blur`, `Card outline`, `Corners` (`Round`/`Soft`/`Sharp`), `Compositor corners`, `Edge fades`, and `Adaptive artwork color`.
//! 2. **Playback & Library**: Hi-Fi lossless upgrades, JioSaavn 320kbps source, Motion Covers, Reduce Motion, Double-click to play, and Media Cache limits.
//! 3. **System, Account & About**: Compositor hints (`--decorated`), MPRIS2/tray info, browser sign-in status, Spotify placeholder, and release updater.

use super::{DesktopModal, SettingsTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::{CornerStyle, DesktopSettings, ThemeModePref, PAGE_INSET_X};
use crate::ui::components::scroll_edge_fades;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};
use sonata_core::config::AppConfig;
use std::process::Command;

impl SonataDesktopView {
    pub fn render_settings_page(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active_tab = match self.settings_tab {
            SettingsTab::Appearance => 0,
            SettingsTab::Playback => 1,
            SettingsTab::Library => 2,
            SettingsTab::System => 3,
            SettingsTab::Account => 4,
            SettingsTab::About => 5,
        };
        let scroll_fades = self.settings.scroll_fades;

        let tabs = TabBar::new("settings-tabs")
            .pill()
            .small()
            .selected_index(active_tab)
            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                this.settings_tab = match *idx {
                    1 => SettingsTab::Playback,
                    2 => SettingsTab::Library,
                    3 => SettingsTab::System,
                    4 => SettingsTab::Account,
                    5 => SettingsTab::About,
                    _ => SettingsTab::Appearance,
                };
                cx.notify();
            }))
            .child(Tab::new().label("Appearance"))
            .child(Tab::new().label("Playback"))
            .child(Tab::new().label("Library"))
            .child(Tab::new().label("System"))
            .child(Tab::new().label("Account"))
            .child(Tab::new().label("About"));

        let body = match self.settings_tab {
            SettingsTab::Appearance => self.render_settings_appearance(cx),
            SettingsTab::Playback => self.render_settings_playback(cx),
            SettingsTab::Library => self.render_settings_library(cx),
            SettingsTab::System => self.render_settings_system(),
            SettingsTab::Account => self.render_settings_account(cx),
            SettingsTab::About => self.render_settings_about(cx),
        };

        let page = div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .px(px(PAGE_INSET_X))
            .py(px(20.0))
            .max_w(px(780.0))
            .child(
                div()
                    .text_size(px(26.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.palette.text)
                    .child("Settings"),
            )
            .child(tabs)
            .child(body);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("settings-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(page),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }

    fn settings_row(
        &self,
        title: &'static str,
        subtitle: impl Into<SharedString>,
        control: AnyElement,
    ) -> AnyElement {
        div()
            .w_full()
            .py(px(12.0))
            .px(px(14.0))
            .rounded(px(self.palette.radii.control))
            .bg(self.palette.surface.opacity(0.45))
            .border_1()
            .border_color(self.palette.outline)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.secondary)
                            .child(subtitle.into()),
                    ),
            )
            .child(div().flex_shrink_0().child(control))
            .into_any_element()
    }

    fn render_settings_appearance(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mode_idx = match self.settings.theme_mode {
            ThemeModePref::System => 0,
            ThemeModePref::Dark => 1,
            ThemeModePref::Light => 2,
        };
        let corner_idx = match self.settings.corners {
            CornerStyle::Round => 0,
            CornerStyle::Soft => 1,
            CornerStyle::Sharp => 2,
        };
        let opacity_pct = (self.settings.opacity * 100.0).round() as u32;
        let errors = self.theme_catalog.errors.clone();
        let warnings = self
            .theme_catalog
            .warnings_for_active(&self.palette.theme_name);
        let themes_dir = DesktopSettings::themes_dir();

        let mode_control = TabBar::new("set-mode-tabs")
            .pill()
            .small()
            .selected_index(mode_idx)
            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                let mode = match *idx {
                    1 => ThemeModePref::Dark,
                    2 => ThemeModePref::Light,
                    _ => ThemeModePref::System,
                };
                this.set_theme_mode(mode, window, cx);
            }))
            .child(Tab::new().label("System"))
            .child(Tab::new().label("Dark"))
            .child(Tab::new().label("Light"))
            .into_any_element();

        let theme_controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(210.0))
                    .child(Select::new(&self.theme_select).small()),
            )
            .child(
                Button::new("set-reload-themes")
                    .secondary()
                    .small()
                    .icon(IconName::RefreshCw)
                    .tooltip("Reload CSS themes")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.reload_themes_and_sync(window, cx);
                    })),
            )
            .child(
                Button::new("set-open-themes")
                    .secondary()
                    .small()
                    .icon(IconName::Folder)
                    .label("Folder")
                    .on_click(move |_, _, _| {
                        let dir = themes_dir.clone();
                        std::thread::spawn(move || {
                            let _ = Command::new("xdg-open").arg(dir).spawn();
                        });
                    }),
            )
            .into_any_element();

        let opacity_control = div()
            .w(px(220.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .flex_1()
                    .child(Slider::new(&self.opacity_slider).horizontal()),
            )
            .child(
                div()
                    .w(px(40.0))
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(format!("{opacity_pct}%")),
            )
            .into_any_element();

        let corner_control = TabBar::new("set-corner-tabs")
            .pill()
            .small()
            .selected_index(corner_idx)
            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                let c = match *idx {
                    1 => CornerStyle::Soft,
                    2 => CornerStyle::Sharp,
                    _ => CornerStyle::Round,
                };
                this.set_corners(c, window, cx);
            }))
            .child(Tab::new().label("Round"))
            .child(Tab::new().label("Soft"))
            .child(Tab::new().label("Sharp"))
            .into_any_element();

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(
                "Mode",
                "Choose dark, light, or follow desktop appearance",
                mode_control,
            ))
            .child(self.settings_row(
                "Theme",
                format!(
                    "CSS palettes from {} (includes GTK / Qt on Linux)",
                    DesktopSettings::themes_dir().display()
                ),
                theme_controls,
            ));

        if !errors.is_empty() || !warnings.is_empty() {
            col = col.child(
                div()
                    .p(px(12.0))
                    .rounded(px(self.palette.radii.control))
                    .bg(self.palette.danger.opacity(0.14))
                    .border_1()
                    .border_color(self.palette.danger.opacity(0.4))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .children(errors.into_iter().map(|e| {
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.danger)
                            .child(e)
                    }))
                    .children(warnings.into_iter().map(|w| {
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.warning)
                            .child(w)
                    })),
            );
        }

        col.child(self.settings_row(
            "Transparency",
            "Layered glass opacity solving gutter and card alphas together",
            opacity_control,
        ))
        .child(self.settings_row(
            "Background Blur",
            "Request native Wayland ext-background-effect / KWin / Acrylic blur behind translucent glass",
            Switch::new("sw-blur")
                .checked(self.settings.blur)
                .on_change(cx.listener(|this, _, window, cx| {
                    this.toggle_blur(window, cx);
                }))
                .into_any_element(),
        ))
        .child(self.settings_row(
            "Card outline",
            "Crisp 1px translucent hairline separating floating cards over blurred gutters",
            Switch::new("sw-outline")
                .checked(self.settings.card_outline)
                .on_change(cx.listener(|this, _, window, cx| {
                    this.toggle_card_outline(window, cx);
                }))
                .into_any_element(),
        ))
        .child(self.settings_row(
            "Corners",
            "Panel, control, and artwork corner curvature across desktop cards",
            corner_control,
        ))
        .child(self.settings_row(
            "Compositor corners",
            "Match compositor Wayland blur mask radius to active corner curvature",
            Switch::new("sw-comp-corners")
                .checked(self.settings.compositor_corners)
                .on_change(cx.listener(|this, _, window, cx| {
                    this.toggle_compositor_corners(window, cx);
                }))
                .into_any_element(),
        ))
        .child(self.settings_row(
            "Scroll edge fades",
            "Smoothly fade scrolling lists, playlists, and lyrics toward panel edges",
            Switch::new("sw-scroll-fades")
                .checked(self.settings.scroll_fades)
                .on_change(cx.listener(|this, _, _, cx| {
                    this.toggle_scroll_fades(cx);
                }))
                .into_any_element(),
        ))
        .child(self.settings_row(
            "Adaptive artwork color",
            "Tint collection headers and Now Playing backgrounds using cover pigment",
            Switch::new("sw-adaptive-art")
                .checked(self.settings.adaptive_art_color)
                .on_change(cx.listener(|this, _, _, cx| {
                    this.toggle_adaptive_art_color(cx);
                }))
                .into_any_element(),
        ))
        .into_any_element()
    }

    fn render_settings_playback(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cfg = &self.snapshot.config;
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(
                "Hi-Fi Lossless Upgrades",
                "Automatically upgrade tracks to lossless FLAC / high-bitrate audio when available",
                Switch::new("sw-hifi")
                    .checked(cfg.hifi_substitution)
                    .on_change(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::ToggleHiFi);
                    }))
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "JioSaavn 320kbps Fallback",
                "Enable optional 320kbps AAC resolution and search source",
                Switch::new("sw-jio")
                    .checked(cfg.jiosaavn_enabled)
                    .on_change(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::ToggleJioSaavn);
                    }))
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Animated Motion Covers",
                "Stream looping cover animations in Now Playing when available",
                Switch::new("sw-motion")
                    .checked(cfg.motion_covers)
                    .on_change(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::ToggleMotionCovers);
                    }))
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Reduce Motion",
                "Prefer static artwork and minimize continuous animations",
                Switch::new("sw-reduce")
                    .checked(cfg.reduce_motion)
                    .on_change(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::ToggleReduceMotion);
                    }))
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Double-click to play rows",
                "Require a double-click on track rows instead of single-click playback",
                Switch::new("sw-dblclick")
                    .checked(self.settings.double_click_to_play)
                    .on_change(cx.listener(|this, _, _, cx| {
                        this.toggle_double_click_to_play(cx);
                    }))
                    .into_any_element(),
            ))
            .into_any_element()
    }

    fn render_settings_library(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let limit_gb = self.snapshot.config.cache_limit_gb;
        let label = AppConfig::cache_limit_label(limit_gb);

        let cache_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                Button::new("set-cache-dec")
                    .secondary()
                    .small()
                    .icon(IconName::Minus)
                    .on_click(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::StepCacheLimit(false));
                    })),
            )
            .child(
                div()
                    .min_w(px(72.0))
                    .flex()
                    .justify_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(
                Button::new("set-cache-inc")
                    .secondary()
                    .small()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::StepCacheLimit(true));
                    })),
            )
            .into_any_element();

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(
                "Audio Media Cache Limit",
                "Bounded SQLite/disk LRU storage for cached audio streams",
                cache_ctrl,
            ))
            .child(
                self.settings_row(
                    "Artwork Memory Budget",
                    "32 MiB DPR-aware crisp & blurred RenderImage cache with GPU atlas reclamation",
                    div()
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .child("32 MB")
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }

    fn render_settings_system(&self) -> AnyElement {
        let wm_mode = if self.decorated {
            "Server-side decorations (--decorated)"
        } else {
            "Frameless client chrome + edge resize"
        };
        let (cpu_pct, rss_mib) = read_sonata_process_metrics();
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(
                "Sonata Resource Usage",
                "Live CPU & resident memory (RSS) used exclusively by this Sonata Desktop process",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(self.palette.radii.small))
                            .bg(self.palette.surface)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text)
                            .child(format!("CPU {cpu_pct:.1}%")),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(self.palette.radii.small))
                            .bg(self.palette.surface)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text)
                            .child(format!("RAM {rss_mib:.1} MiB")),
                    )
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Window Decorations",
                "Start with `--decorated` to hand titlebars back to your window manager",
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(wm_mode)
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Compositor Protocols",
                "Supports Wayland ext-background-effect-v1, KDE KWin blur, and Hyprland opacity sync",
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(if self.settings.blur { "Active" } else { "Off" })
                    .into_any_element(),
            ))
            .child(self.settings_row(
                "Desktop Integrations",
                "MPRIS2 D-Bus media controls and StatusNotifierItem system tray icon",
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child("Running")
                    .into_any_element(),
            ))
            .into_any_element()
    }

    fn render_settings_account(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let signed_in = self.snapshot.is_signed_in();
        let account_label = self
            .snapshot
            .user_profile
            .as_ref()
            .map(|p| p.name.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| self.snapshot.signed_in_browser.clone())
            .unwrap_or_else(|| "Not signed in".to_string());
        let handle = self
            .snapshot
            .user_profile
            .as_ref()
            .and_then(|p| p.handle.clone())
            .unwrap_or_else(|| "Extract browser session cookies".to_string());

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                self.settings_row(
                    "Background Status",
                    self.snapshot.status_line.clone(),
                    div()
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .child("Active")
                        .into_any_element(),
                ),
            )
            .child(self.settings_row(
                "YouTube Music Session",
                format!("{account_label} • {handle}"),
                if signed_in {
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(
                            Button::new("set-acc-refresh")
                                .secondary()
                                .small()
                                .icon(IconName::RefreshCw)
                                .label("Sync")
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::RefreshAccount);
                                })),
                        )
                        .child(
                            Button::new("set-acc-switch")
                                .secondary()
                                .small()
                                .label("Switch browser")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.modal = DesktopModal::SignInBrowser;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("set-acc-signout")
                                .danger()
                                .small()
                                .label("Sign out")
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::SignOut);
                                })),
                        )
                        .into_any_element()
                } else {
                    Button::new("set-acc-signin")
                        .primary()
                        .small()
                        .label("Sign in")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.modal = DesktopModal::SignInBrowser;
                            cx.notify();
                        }))
                        .into_any_element()
                },
            ))
            .child(
                self.settings_row(
                    "Spotify",
                    self.snapshot
                        .spotify_notice
                        .clone()
                        .unwrap_or_else(|| "Optional secondary provider".to_string()),
                    Button::new("set-spf-link")
                        .secondary()
                        .small()
                        .label("Connect")
                        .on_click(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ClickLinkSpotify);
                        }))
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }

    fn render_settings_about(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let update_available = self
            .snapshot
            .update_info
            .as_ref()
            .is_some_and(|info| info.update_available);
        let status = match &self.snapshot.update_info {
            Some(info) => format!(
                "Latest: {} (current {})",
                info.latest_version,
                env!("CARGO_PKG_VERSION")
            ),
            None => format!("Current version: {}", env!("CARGO_PKG_VERSION")),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                self.settings_row(
                    "Sonata Desktop",
                    status,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            Button::new("set-check-upd")
                                .secondary()
                                .small()
                                .icon(IconName::RefreshCw)
                                .label(if self.snapshot.checking_updates {
                                    "Checking..."
                                } else {
                                    "Check updates"
                                })
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::CheckForUpdates);
                                })),
                        )
                        .child({
                            let btn = if update_available {
                                Button::new("set-open-repo").primary()
                            } else {
                                Button::new("set-open-repo").secondary()
                            };
                            btn.small()
                                .icon(IconName::ExternalLink)
                                .label(if update_available {
                                    "Update on GitHub"
                                } else {
                                    "Open GitHub"
                                })
                                .on_click(|_, _, _| {
                                    let _ = Command::new("xdg-open")
                                        .arg("https://github.com/MehmetCanWT/sonata/releases")
                                        .spawn();
                                })
                        })
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    "License & Build",
                    "MIT Licensed • Built natively in Rust with GPUI & gpui-kit",
                    div()
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .child("FOSS")
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }
}

#[cfg(target_os = "linux")]
fn read_sonata_process_metrics() -> (f32, f32) {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    struct CpuSample {
        ticks: u64,
        at: Instant,
        last_pct: f32,
    }

    static SAMPLE: OnceLock<Mutex<Option<CpuSample>>> = OnceLock::new();

    let rss_mib = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|txt| {
            txt.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<f64>().ok())
                .map(|kb| (kb / 1024.0) as f32)
        })
        .unwrap_or(0.0);

    let ticks = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let after_comm = stat.rsplit_once(')')?.1;
            let fields: Vec<&str> = after_comm.split_whitespace().collect();
            let utime: u64 = fields.get(11)?.parse().ok()?;
            let stime: u64 = fields.get(12)?.parse().ok()?;
            Some(utime.saturating_add(stime))
        })
        .unwrap_or(0);

    let clk_tck = {
        // SAFETY: `sysconf(_SC_CLK_TCK)` is a standard read-only POSIX query with no pointer arguments.
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if hz > 0 {
            hz as f64
        } else {
            100.0
        }
    };

    let now = Instant::now();
    let cpu_pct = {
        let cell = SAMPLE.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = cell.lock() {
            if let Some(prev) = guard.as_mut() {
                let dt = now.saturating_duration_since(prev.at).as_secs_f64();
                if dt >= 0.4 {
                    let delta_ticks = ticks.saturating_sub(prev.ticks) as f64;
                    let pct = ((delta_ticks / clk_tck) / dt * 100.0).clamp(0.0, 999.9) as f32;
                    prev.ticks = ticks;
                    prev.at = now;
                    prev.last_pct = pct;
                    pct
                } else {
                    prev.last_pct
                }
            } else {
                *guard = Some(CpuSample {
                    ticks,
                    at: now,
                    last_pct: 0.0,
                });
                0.0
            }
        } else {
            0.0
        }
    };

    (cpu_pct, rss_mib)
}

#[cfg(not(target_os = "linux"))]
fn read_sonata_process_metrics() -> (f32, f32) {
    (0.0, 0.0)
}
