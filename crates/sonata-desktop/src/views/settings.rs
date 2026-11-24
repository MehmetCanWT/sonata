//! Six-tab Settings view (`Appearance`, `Playback`, `Library`, `System`, `Account`, `About`) using `gpui-component` controls.
//!
//! 1. **Appearance**: Mode (`System`/`Dark`/`Light`), CSS Theme `Select` (`GTK`/`Qt` + loaded `.css` files + error/contrast banners + `Open Themes Folder`), Transparency `Slider`, `Blur`, `Card outline`, `Corners` (`Round`/`Soft`/`Sharp`), `Compositor corners`, `Edge fades`, and `Adaptive artwork color`.
//! 2. **Playback & Library**: Hi-Fi lossless upgrades, JioSaavn 320kbps source, Motion Covers, Reduce Motion, Double-click to play, and Media Cache limits.
//! 3. **System, Account & About**: Compositor hints (`--decorated`), MPRIS2/tray info, browser sign-in status, Spotify placeholder, and release updater.

use super::{DesktopModal, SettingsTab, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::{CornerStyle, DesktopSettings, ThemeModePref, PAGE_INSET_X};
use crate::ui::components::scroll_edge_fades;
use crate::views::library::format_bytes;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::select::Select;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};
use sonata_runtime::config::AppConfig;
use sonata_runtime::discord::DiscordRpcStatus;
use sonata_runtime::i18n::{strings, AppLanguage};
use std::path::PathBuf;
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

        let s = strings(self.settings.language);
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
            .child(Tab::new().label(s.tab_appearance))
            .child(Tab::new().label(s.tab_playback))
            .child(Tab::new().label(s.tab_library))
            .child(Tab::new().label(s.tab_system))
            .child(Tab::new().label(s.tab_account))
            .child(Tab::new().label(s.tab_about));

        let body = match self.settings_tab {
            SettingsTab::Appearance => self.render_settings_appearance(cx),
            SettingsTab::Playback => self.render_settings_playback(cx),
            SettingsTab::Library => self.render_settings_library(cx),
            SettingsTab::System => self.render_settings_system(cx),
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
                    .child(s.settings),
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
        title: impl Into<SharedString>,
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
                            .child(title.into()),
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
        let s = strings(self.settings.language);
        let lang_idx = match self.settings.language {
            AppLanguage::En => 0,
            AppLanguage::Tr => 1,
            AppLanguage::Ru => 2,
        };
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

        let lang_control = TabBar::new("set-lang-tabs")
            .pill()
            .small()
            .selected_index(lang_idx)
            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                let lang = match *idx {
                    1 => AppLanguage::Tr,
                    2 => AppLanguage::Ru,
                    _ => AppLanguage::En,
                };
                if this.settings.language != lang {
                    this.settings.language = lang;
                    this.settings.save();
                    this.backend.send(BackendCmd::SetLanguage(lang));
                    cx.notify();
                }
            }))
            .child(Tab::new().label("English"))
            .child(Tab::new().label("Türkçe"))
            .child(Tab::new().label("Русский"))
            .into_any_element();

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
            .child(Tab::new().label(s.mode_system))
            .child(Tab::new().label(s.mode_dark))
            .child(Tab::new().label(s.mode_light))
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
                    .tooltip(s.reload_themes)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.reload_themes_and_sync(window, cx);
                    })),
            )
            .child(
                Button::new("set-open-themes")
                    .secondary()
                    .small()
                    .icon(IconName::Folder)
                    .label(s.open_folder)
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
            .child(Tab::new().label(s.corner_round))
            .child(Tab::new().label(s.corner_soft))
            .child(Tab::new().label(s.corner_sharp))
            .into_any_element();

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(s.language, s.language_desc, lang_control))
            .child(self.settings_row(s.theme_mode, s.theme_mode_desc, mode_control))
            .child(self.settings_row(s.color_theme, s.color_theme_desc, theme_controls));

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

        col.child(self.settings_row(s.transparency, s.transparency_desc, opacity_control))
            .child(
                self.settings_row(
                    s.blur,
                    s.blur_desc,
                    Switch::new("sw-blur")
                        .checked(self.settings.blur)
                        .on_change(cx.listener(|this, _, window, cx| {
                            this.toggle_blur(window, cx);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.card_outline,
                    s.card_outline_desc,
                    Switch::new("sw-outline")
                        .checked(self.settings.card_outline)
                        .on_change(cx.listener(|this, _, window, cx| {
                            this.toggle_card_outline(window, cx);
                        }))
                        .into_any_element(),
                ),
            )
            .child(self.settings_row(s.corners, s.corners_desc, corner_control))
            .child(
                self.settings_row(
                    "Compositor corners",
                    "Match compositor Wayland blur mask radius to active corner curvature",
                    Switch::new("sw-comp-corners")
                        .checked(self.settings.compositor_corners)
                        .on_change(cx.listener(|this, _, window, cx| {
                            this.toggle_compositor_corners(window, cx);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.edge_fades,
                    s.edge_fades_desc,
                    Switch::new("sw-scroll-fades")
                        .checked(self.settings.scroll_fades)
                        .on_change(cx.listener(|this, _, _, cx| {
                            this.toggle_scroll_fades(cx);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.adaptive_art_color,
                    s.adaptive_art_color_desc,
                    Switch::new("sw-adaptive-art")
                        .checked(self.settings.adaptive_art_color)
                        .on_change(cx.listener(|this, _, _, cx| {
                            this.toggle_adaptive_art_color(cx);
                        }))
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }

    fn render_settings_playback(&mut self, cx: &mut Context<Self>) -> AnyElement {
        use crate::backend::playback::{
            format_sleep_countdown, PLAYBACK_SPEED_PRESETS, SLEEP_TIMER_PRESET_SECS,
            VIDEO_QUALITY_OPTIONS,
        };
        let s = strings(self.settings.language);
        let cfg = &self.snapshot.config;
        let speed_idx = PLAYBACK_SPEED_PRESETS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((*a - self.snapshot.playback_speed).abs())
                    .partial_cmp(&((*b - self.snapshot.playback_speed).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(1);
        let speed_control = TabBar::new("set-speed-tabs")
            .pill()
            .small()
            .selected_index(speed_idx)
            .on_click(cx.listener(|this, idx: &usize, _, _| {
                if let Some(&spd) = PLAYBACK_SPEED_PRESETS.get(*idx) {
                    this.backend.send(BackendCmd::SetPlaybackSpeed(spd));
                }
            }))
            .child(Tab::new().label("0.75x"))
            .child(Tab::new().label("1x"))
            .child(Tab::new().label("1.25x"))
            .child(Tab::new().label("1.5x"))
            .child(Tab::new().label("1.75x"))
            .child(Tab::new().label("2x"))
            .into_any_element();

        let sleep_rem = self.snapshot.sleep_timer_remaining_secs;
        let sleep_idx = match sleep_rem {
            None | Some(0) => 0,
            Some(rem) => SLEEP_TIMER_PRESET_SECS
                .iter()
                .enumerate()
                .skip(1)
                .min_by_key(|(_, p)| p.unwrap_or(0).abs_diff(rem))
                .map(|(i, _)| i)
                .unwrap_or(1),
        };
        let sleep_tabs = TabBar::new("set-sleep-tabs")
            .pill()
            .small()
            .selected_index(sleep_idx)
            .on_click(cx.listener(|this, idx: &usize, _, _| {
                if let Some(&preset) = SLEEP_TIMER_PRESET_SECS.get(*idx) {
                    this.backend.send(BackendCmd::SetSleepTimer(preset));
                }
            }))
            .child(Tab::new().label(s.off))
            .child(Tab::new().label("15m"))
            .child(Tab::new().label("30m"))
            .child(Tab::new().label("45m"))
            .child(Tab::new().label("60m"));
        let sleep_control = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .when_some(sleep_rem, |row, rem| {
                row.child(
                    div()
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(self.palette.radii.small))
                        .bg(self.palette.accent.opacity(0.16))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(self.palette.accent)
                        .child(format_sleep_countdown(rem)),
                )
            })
            .child(sleep_tabs)
            .into_any_element();

        let vq_idx = VIDEO_QUALITY_OPTIONS
            .iter()
            .position(|opt| opt.eq_ignore_ascii_case(cfg.video_quality.trim()))
            .unwrap_or(0);
        let vq_control = TabBar::new("set-vq-tabs")
            .pill()
            .small()
            .selected_index(vq_idx)
            .on_click(cx.listener(|this, idx: &usize, _, _| {
                if let Some(&opt) = VIDEO_QUALITY_OPTIONS.get(*idx) {
                    this.backend
                        .send(BackendCmd::SetVideoQuality(opt.to_string()));
                }
            }))
            .child(Tab::new().label(s.auto_label))
            .child(Tab::new().label("1080p"))
            .child(Tab::new().label("720p"))
            .child(Tab::new().label("480p"))
            .child(Tab::new().label("360p"))
            .into_any_element();

        let skip_silence_desc = if self.snapshot.video_mode {
            format!("{} ({})", s.skip_silence_desc, s.video)
        } else {
            s.skip_silence_desc.to_string()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(s.playback_speed, s.playback_speed_desc, speed_control))
            .child(
                self.settings_row(
                    s.skip_silence,
                    skip_silence_desc,
                    Switch::new("sw-skip-silence")
                        .checked(cfg.skip_silence)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleSkipSilence);
                        }))
                        .into_any_element(),
                ),
            )
            .child(self.settings_row(s.sleep_timer, s.sleep_timer_desc, sleep_control))
            .child(
                self.settings_row(
                    s.translate_lyrics,
                    s.translate_lyrics_desc,
                    Switch::new("sw-translate-lyrics")
                        .checked(cfg.lyrics_translate)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleLyricsTranslate);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.romanize_lyrics,
                    s.romanize_lyrics_desc,
                    Switch::new("sw-romanize-lyrics")
                        .checked(cfg.lyrics_romanize)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleLyricsRomanize);
                        }))
                        .into_any_element(),
                ),
            )
            .child(self.settings_row(s.video_quality, s.video_quality_desc, vq_control))
            .child(
                self.settings_row(
                    s.autoplay,
                    s.autoplay_desc,
                    Switch::new("sw-autoplay")
                        .checked(cfg.autoplay)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleAutoplay);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.hifi_upgrade,
                    s.hifi_upgrade_desc,
                    Switch::new("sw-hifi")
                        .checked(cfg.hifi_substitution)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleHiFi);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.jiosaavn_module,
                    s.jiosaavn_module_desc,
                    Switch::new("sw-jio")
                        .checked(cfg.jiosaavn_enabled)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleJioSaavn);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.motion_covers,
                    s.motion_covers_desc,
                    Switch::new("sw-motion")
                        .checked(cfg.motion_covers)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleMotionCovers);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.reduce_motion,
                    s.reduce_motion_desc,
                    Switch::new("sw-reduce")
                        .checked(cfg.reduce_motion)
                        .on_change(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ToggleReduceMotion);
                        }))
                        .into_any_element(),
                ),
            )
            .child(
                self.settings_row(
                    s.dbl_click_play,
                    s.dbl_click_play_desc,
                    Switch::new("sw-dblclick")
                        .checked(self.settings.double_click_to_play)
                        .on_change(cx.listener(|this, _, _, cx| {
                            this.toggle_double_click_to_play(cx);
                        }))
                        .into_any_element(),
                ),
            )
            .into_any_element()
    }

    fn render_settings_library(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = strings(self.settings.language);
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

        let dl_limit_label = if self.snapshot.config.downloads_limit_gb == 0 {
            s.unlimited.to_string()
        } else {
            AppConfig::downloads_limit_label(self.snapshot.config.downloads_limit_gb)
        };
        let dl_limit_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                Button::new("set-dl-dec")
                    .secondary()
                    .small()
                    .icon(IconName::Minus)
                    .on_click(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::StepDownloadsLimit(false));
                    })),
            )
            .child(
                div()
                    .min_w(px(72.0))
                    .flex()
                    .justify_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(dl_limit_label),
            )
            .child(
                Button::new("set-dl-inc")
                    .secondary()
                    .small()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::StepDownloadsLimit(true));
                    })),
            )
            .into_any_element();

        let dl_dir_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .w(px(210.0))
                    .h(px(30.0))
                    .px(px(8.0))
                    .rounded(px(self.palette.radii.small))
                    .bg(self.palette.surface)
                    .border_1()
                    .border_color(self.palette.outline)
                    .flex()
                    .items_center()
                    .child(Input::new(&self.downloads_dir_input).appearance(false)),
            )
            .child(
                Button::new("set-dl-pick")
                    .secondary()
                    .small()
                    .icon(IconName::Folder)
                    .on_click(cx.listener(|_this, _, window, cx| {
                        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                            files: false,
                            directories: true,
                            multiple: false,
                            prompt: Some("Select offline downloads folder".into()),
                        });
                        let weak_win = window.window_handle();
                        cx.spawn(async move |weak, cx| {
                            if let Ok(Ok(Some(paths))) = rx.await {
                                if let Some(path) = paths.into_iter().next() {
                                    let path_str = path.to_string_lossy().into_owned();
                                    let _ = weak.update(cx, |this, cx| {
                                        let _ = weak_win.update(cx, |_, window, cx| {
                                            this.downloads_dir_input.update(cx, |st, cx| {
                                                st.set_value(path_str, window, cx);
                                            });
                                        });
                                        this.backend.send(BackendCmd::SetDownloadsDir(Some(path)));
                                        cx.notify();
                                    });
                                }
                            }
                        })
                        .detach();
                    })),
            )
            .child(
                Button::new("set-dl-save")
                    .secondary()
                    .small()
                    .label(s.save)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let raw = this.downloads_dir_input.read(cx).value().trim().to_string();
                        let dir = if raw.is_empty() {
                            None
                        } else {
                            Some(PathBuf::from(raw))
                        };
                        this.backend.send(BackendCmd::SetDownloadsDir(dir));
                    })),
            )
            .child(
                Button::new("set-dl-reset")
                    .ghost()
                    .small()
                    .label(s.reset_default)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.downloads_dir_input.update(cx, |st, cx| {
                            st.set_value("", window, cx);
                        });
                        this.backend.send(BackendCmd::SetDownloadsDir(None));
                    })),
            )
            .into_any_element();

        let dl_usage_str = format!(
            "{} • {} {}",
            format_bytes(self.snapshot.downloads_total_bytes),
            self.snapshot.downloaded_tracks.len(),
            s.tracks
        );
        let dl_usage_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(dl_usage_str),
            )
            .when(!self.snapshot.downloaded_tracks.is_empty(), |el| {
                el.child(
                    Button::new("set-dl-clear")
                        .secondary()
                        .small()
                        .icon(IconName::Trash)
                        .label(s.clear_downloads)
                        .on_click(cx.listener(|this, _, _, _| {
                            this.backend.send(BackendCmd::ClearAllDownloads);
                        })),
                )
            })
            .into_any_element();

        let mut servers_section = div().flex().flex_col().gap(px(8.0)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pt(px(8.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(self.palette.text)
                                .child(s.servers),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(self.palette.secondary)
                                .child(s.servers_desc),
                        ),
                )
                .child(
                    Button::new("set-add-server")
                        .primary()
                        .small()
                        .icon(IconName::Plus)
                        .label(s.add_server)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_add_server_modal(window, cx);
                        })),
                ),
        );

        if self.snapshot.server_profiles.is_empty() {
            servers_section = servers_section.child(
                div()
                    .px(px(14.0))
                    .py(px(12.0))
                    .rounded(px(self.palette.radii.control))
                    .bg(self.palette.surface)
                    .border_1()
                    .border_color(self.palette.outline)
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(s.no_servers),
            );
        } else {
            for (idx, profile) in self.snapshot.server_profiles.iter().enumerate() {
                let sid_test = profile.id.clone();
                let sid_toggle = profile.id.clone();
                let sid_remove = profile.id.clone();
                let status = self
                    .snapshot
                    .server_statuses
                    .get(&profile.id)
                    .cloned()
                    .unwrap_or_else(|| profile.base_url.clone());
                let subtitle = format!(
                    "[{}] {} • {}",
                    profile.kind.short_tag(),
                    profile.base_url,
                    status
                );
                let ctrl = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        Button::new(("set-srv-test", idx))
                            .secondary()
                            .small()
                            .icon(IconName::RefreshCw)
                            .label(s.test_connection)
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.backend
                                    .send(BackendCmd::TestServerProfile(sid_test.clone()));
                            })),
                    )
                    .child(
                        Switch::new(("set-srv-en", idx))
                            .checked(profile.enabled)
                            .on_change(cx.listener(move |this, _, _, _| {
                                this.backend
                                    .send(BackendCmd::ToggleServerEnabled(sid_toggle.clone()));
                            })),
                    )
                    .child(
                        Button::new(("set-srv-rm", idx))
                            .ghost()
                            .small()
                            .icon(IconName::Trash)
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.backend
                                    .send(BackendCmd::RemoveServerProfile(sid_remove.clone()));
                            })),
                    )
                    .into_any_element();
                servers_section =
                    servers_section.child(self.settings_row(profile.name.clone(), subtitle, ctrl));
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(s.cache_budget, s.cache_budget_desc, cache_ctrl))
            .child(self.settings_row(s.downloads_dir, s.downloads_dir_desc, dl_dir_ctrl))
            .child(self.settings_row(s.downloads_limit, s.downloads_limit_desc, dl_limit_ctrl))
            .child(self.settings_row(s.downloads_usage, "", dl_usage_ctrl))
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
            .child(servers_section)
            .into_any_element()
    }

    fn render_settings_system(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = strings(self.settings.language);
        let wm_mode = if self.decorated {
            "Server-side decorations (--decorated)"
        } else {
            "Frameless client chrome + edge resize"
        };
        let (cpu_pct, rss_mib) = read_sonata_process_metrics();
        let discord_status_text = match &self.snapshot.discord_status {
            DiscordRpcStatus::Disabled => "Off".to_string(),
            DiscordRpcStatus::Disconnected => "Disconnected".to_string(),
            DiscordRpcStatus::Connecting => "Connecting".to_string(),
            DiscordRpcStatus::Connected { socket_path } => {
                let name = socket_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("IPC");
                format!("Connected ({name})")
            }
            DiscordRpcStatus::Error(err) => format!("Error: {err}"),
        };
        let discord_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(self.palette.secondary)
                    .child(discord_status_text),
            )
            .child(
                Switch::new("sw-discord-rpc")
                    .checked(self.snapshot.config.discord_rpc_enabled)
                    .on_change(cx.listener(|this, _, _, _| {
                        this.backend.send(BackendCmd::ToggleDiscordRpc);
                    })),
            )
            .into_any_element();

        let discord_app_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .w(px(190.0))
                    .h(px(30.0))
                    .px(px(8.0))
                    .rounded(px(self.palette.radii.small))
                    .bg(self.palette.surface)
                    .border_1()
                    .border_color(self.palette.outline)
                    .flex()
                    .items_center()
                    .child(Input::new(&self.discord_app_id_input).appearance(false)),
            )
            .child(
                Button::new("set-discord-save")
                    .secondary()
                    .small()
                    .label(s.save)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let raw = this
                            .discord_app_id_input
                            .read(cx)
                            .value()
                            .trim()
                            .to_string();
                        let id = if raw.is_empty() { None } else { Some(raw) };
                        this.backend.send(BackendCmd::SetDiscordAppId(id));
                    })),
            )
            .child(
                Button::new("set-discord-reset")
                    .ghost()
                    .small()
                    .label(s.reset_default)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.discord_app_id_input.update(cx, |st, cx| {
                            st.set_value("", window, cx);
                        });
                        this.backend.send(BackendCmd::SetDiscordAppId(None));
                    })),
            )
            .into_any_element();

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(self.settings_row(s.discord_rpc, s.discord_rpc_desc, discord_ctrl))
            .child(self.settings_row(s.discord_app_id, s.discord_app_id_desc, discord_app_ctrl))
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
