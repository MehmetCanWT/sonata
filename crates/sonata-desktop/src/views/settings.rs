use super::{render_avatar, render_pill_button, ui_icon, FluentDivExt, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::theme_bridge::{rgb_hsla, DesktopSkin, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, WindowBackgroundAppearance,
};
use sonata_core::config::AppConfig;
use sonata_core::hyprland;

fn section_card(title: &str, p: &GlassPalette) -> gpui_kit::Div {
    div()
        .w_full()
        .p_4()
        .rounded_2xl()
        .bg(p.card_bg)
        .border_1()
        .border_color(p.border_subtle)
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(p.muted)
                .child(title.to_string()),
        )
}

fn setting_row(label: &str, subtitle: &str, control: AnyElement, p: &GlassPalette) -> AnyElement {
    div()
        .w_full()
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child(subtitle.to_string()),
                ),
        )
        .child(control)
        .into_any_element()
}

pub fn render_settings_page(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let cfg = &snap.config;
    let effective_transparent = snap.effective_transparent();
    let opacity_pct = (snap.effective_hyprland_opacity() * 100.0).round() as u32;
    let is_hypr = hyprland::is_hyprland();

    // 1. YouTube Music Account Card
    let account_card = if snap.is_signed_in() {
        let name = snap
            .user_profile
            .as_ref()
            .map(|u| u.name.clone())
            .unwrap_or_else(|| "YouTube Music Account".to_string());
        let handle_or_email = snap
            .user_profile
            .as_ref()
            .and_then(|u| u.handle.clone().or_else(|| u.email.clone()))
            .unwrap_or_else(|| format!("{} playlists synced", snap.ytm_playlists.len()));
        let browser_badge = snap
            .signed_in_browser
            .as_deref()
            .and_then(|id| {
                snap.available_browsers
                    .iter()
                    .find(|b| b.id == id)
                    .map(|b| b.label)
            })
            .unwrap_or("Browser Session");

        section_card("YOUTUBE MUSIC ACCOUNT", p).child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_avatar(
                            snap.user_avatar_path.as_ref(),
                            &name,
                            54.0,
                            p,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(p.highlight_fg)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py(px(1.0))
                                                .rounded_full()
                                                .bg(p.accent_soft)
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(p.accent)
                                                .child(browser_badge),
                                        ),
                                )
                                .child(div().text_xs().text_color(p.muted).child(handle_or_email)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_pill_button(
                            "acc-refresh",
                            "Refresh".to_string(),
                            false,
                            p,
                            |this, _, _| this.backend.send(BackendCmd::RefreshAccount),
                            cx,
                        ))
                        .child(render_pill_button(
                            "acc-switch-browser",
                            "Switch Browser".to_string(),
                            false,
                            p,
                            |this, _, cx| this.open_sign_in_modal(cx),
                            cx,
                        ))
                        .child(
                            div()
                                .id("acc-signout")
                                .px_3()
                                .py(px(6.0))
                                .rounded_lg()
                                .bg(p.pill_bg)
                                .border_1()
                                .border_color(p.border_subtle)
                                .hover(|s| s.bg(p.card_hover))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_1()
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.backend.send(BackendCmd::SignOut);
                                }))
                                .child(ui_icon(IconName::LogOut, 13.0, p.error))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.error)
                                        .child("Sign Out"),
                                ),
                        ),
                ),
        )
    } else {
        section_card("YOUTUBE MUSIC ACCOUNT", p).child(setting_row(
            "Sign in with Browser Session",
            "Choose which installed browser Sonata should read YouTube Music cookies from to unlock your library, Liked Music, and personalized recommendations.",
            div()
                .id("settings-signin-btn")
                .px_4()
                .py_2()
                .rounded_full()
                .bg(p.accent)
                .cursor_pointer()
                .flex()
                .items_center()
                .gap_2()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_sign_in_modal(cx);
                }))
                .child(ui_icon(IconName::LogIn, 14.0, p.accent_fg))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.accent_fg)
                        .child("Sign In"),
                )
                .into_any_element(),
            p,
        ))
    };

    // 2. Playback & Audio Section
    let audio_card = section_card("AUDIO & STREAMING", p)
        .child(setting_row(
            "YouTube Music Quality Upgrade",
            "After playback starts, switch to a higher-quality YouTube Music format when available",
            render_pill_button(
                "set-hifi",
                if cfg.hifi_substitution {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                cfg.hifi_substitution,
                p,
                |this, _, _| this.backend.send(BackendCmd::ToggleHiFi),
                cx,
            ),
            p,
        ))
        .child(setting_row(
            "Audio Enhancer DSP",
            "SoXR resampling & mpv acoustic preset filter chain",
            render_pill_button(
                "set-dsp",
                snap.enhancer.label().to_string(),
                true,
                p,
                |this, _, _| this.backend.send(BackendCmd::CycleEnhancer),
                cx,
            ),
            p,
        ));

    let cache_control = div()
        .flex()
        .items_center()
        .gap_2()
        .child(render_pill_button(
            "cache-limit-down",
            "−".to_string(),
            false,
            p,
            |this, _, _| this.backend.send(BackendCmd::StepCacheLimit(false)),
            cx,
        ))
        .child(
            div()
                .w(px(160.0))
                .flex()
                .justify_center()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text)
                .child(AppConfig::cache_limit_label(cfg.cache_limit_gb)),
        )
        .child(render_pill_button(
            "cache-limit-up",
            "+".to_string(),
            false,
            p,
            |this, _, _| this.backend.send(BackendCmd::StepCacheLimit(true)),
            cx,
        ))
        .into_any_element();

    let cache_card = section_card("STORAGE & CACHE", p).child(setting_row(
        "Media Cache Limit",
        "Shared by TUI and desktop. Off disables saved media and Hi-Fi staging; not recommended.",
        cache_control,
        p,
    ));

    // 3. Appearance & Hyprland Glass Section
    let opacity_ctrl = if is_hypr {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(render_pill_button(
                "set-op-dec",
                "-".to_string(),
                false,
                p,
                |this, window, _| {
                    if this.snapshot.effective_transparent() {
                        window.set_background_appearance(WindowBackgroundAppearance::Blurred);
                    }
                    this.backend.send(BackendCmd::StepHyprlandOpacity(false));
                },
                cx,
            ))
            .child(
                div()
                    .w(px(44.0))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text)
                    .flex()
                    .justify_center()
                    .child(format!("{opacity_pct}%")),
            )
            .child(render_pill_button(
                "set-op-inc",
                "+".to_string(),
                false,
                p,
                |this, window, _| {
                    if this.snapshot.effective_transparent() {
                        window.set_background_appearance(WindowBackgroundAppearance::Blurred);
                    }
                    this.backend.send(BackendCmd::StepHyprlandOpacity(true));
                },
                cx,
            ))
            .into_any_element()
    } else {
        div()
            .px_3()
            .py(px(6.0))
            .rounded_lg()
            .bg(p.pill_bg)
            .border_1()
            .border_color(p.border_subtle)
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(p.muted)
            .child("Solid (100% · Locked)")
            .into_any_element()
    };

    let mut theme_chips = div().w_full().flex().flex_wrap().gap_3();
    for (idx, t) in snap.themes.iter().enumerate() {
        let selected = t.name.eq_ignore_ascii_case(&snap.base_theme.name);
        let bg_swatch = rgb_hsla(t.bg.to_rgb((0x12, 0x12, 0x16)), 1.0);
        let surface_swatch = rgb_hsla(t.surface.to_rgb((0x1c, 0x1c, 0x22)), 1.0);
        let accent_swatch = rgb_hsla(t.accent.to_rgb((0xfa, 0x2d, 0x48)), 1.0);
        let skin_badge = DesktopSkin::from_theme_name(&t.name).badge_label();
        let t_name = t.name.clone();
        let t_transparent = t.transparent_bg;
        theme_chips = theme_chips.child(
            div()
                .id(SharedString::from(format!("set-theme-{idx}")))
                .min_w(px(172.0))
                .px_3()
                .py(px(10.0))
                .rounded_xl()
                .bg(if selected { p.accent_soft } else { p.pill_bg })
                .border_1()
                .border_color(if selected { p.accent } else { p.border_subtle })
                .hover(|s| s.bg(p.card_hover))
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap_2()
                .on_click(cx.listener(move |this, _, window, _| {
                    let next_transparent =
                        t_transparent.unwrap_or(this.snapshot.config.transparent_bg);
                    window.set_background_appearance(if next_transparent {
                        WindowBackgroundAppearance::Blurred
                    } else {
                        WindowBackgroundAppearance::Opaque
                    });
                    this.backend.send(BackendCmd::SelectTheme(t_name.clone()));
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .size(px(12.0))
                                        .rounded_full()
                                        .bg(bg_swatch)
                                        .border_1()
                                        .border_color(p.border_subtle),
                                )
                                .child(
                                    div()
                                        .size(px(12.0))
                                        .rounded_full()
                                        .bg(surface_swatch)
                                        .border_1()
                                        .border_color(p.border_subtle),
                                )
                                .child(div().size(px(12.0)).rounded_full().bg(accent_swatch)),
                        )
                        .child(
                            div()
                                .px_2()
                                .py(px(1.0))
                                .rounded_full()
                                .bg(p.card_bg)
                                .text_xs()
                                .text_color(if selected { p.accent } else { p.muted })
                                .child(skin_badge),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(if selected {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if selected { p.highlight_fg } else { p.text })
                        .child(t.name.clone()),
                ),
        );
    }

    let appearance_card = section_card("APPEARANCE & HYPRLAND GLASS", p)
        .child(setting_row(
            "Transparent Glass Window",
            "Translucent Wayland backdrop with compositor blur",
            render_pill_button(
                "set-glass",
                if effective_transparent {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                effective_transparent,
                p,
                |this, window, _| {
                    let next_bg = if this.snapshot.effective_transparent() {
                        WindowBackgroundAppearance::Opaque
                    } else {
                        WindowBackgroundAppearance::Blurred
                    };
                    window.set_background_appearance(next_bg);
                    this.backend.send(BackendCmd::ToggleTransparentBg);
                },
                cx,
            ),
            p,
        ))
        .child(setting_row(
            if is_hypr {
                "Hyprland Window Opacity"
            } else {
                "Hyprland Opacity"
            },
            "Dynamic active/inactive opacity synced via hyprctl",
            opacity_ctrl,
            p,
        ))
        .child(setting_row(
            "Reduce Motion",
            "Disable fullscreen motion and automatic control hiding",
            render_pill_button(
                "set-reduce-motion",
                if cfg.reduce_motion {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                cfg.reduce_motion,
                p,
                |this, _, _| this.backend.send(BackendCmd::ToggleReduceMotion),
                cx,
            ),
            p,
        ))
        .child(
            div()
                .pt_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.muted)
                        .child("Desktop Theme & Skin (independent from TUI)"),
                )
                .child(theme_chips),
        );

    // 4. Integrations — Spotify Placeholder
    let spotify_card = section_card("CONNECTED SERVICES", p)
        .child(setting_row(
            "Spotify",
            "Playlist import and Spotify integration will return in a later release.",
            div()
                .id("set-link-spotify")
                .px_3()
                .py(px(7.0))
                .rounded_lg()
                .bg(p.pill_bg)
                .border_1()
                .border_color(p.border_subtle)
                .hover(|s| s.bg(p.card_hover))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap_2()
                .on_click(cx.listener(|this, _, _, _| {
                    this.backend.send(BackendCmd::ClickLinkSpotify);
                }))
                .child(ui_icon(IconName::Link, 13.0, p.muted))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child("Link Spotify"),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded_full()
                        .bg(p.card_bg)
                        .text_xs()
                        .text_color(p.muted)
                        .child("Soon"),
                )
                .into_any_element(),
            p,
        ))
        .when(snap.spotify_notice.is_some(), |el| {
            el.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(p.pill_bg)
                    .text_xs()
                    .text_color(p.muted)
                    .child(snap.spotify_notice.clone().unwrap_or_default()),
            )
        });

    div()
        .id("settings-page-scroll")
        .size_full()
        .overflow_y_scroll()
        .p_6()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child("Settings"),
                )
                .child(div().text_xs().text_color(p.muted).child(
                    "Manage your YouTube Music session, audio engine, and Modern Glass appearance.",
                )),
        )
        .child(account_card)
        .child(audio_card)
        .child(cache_card)
        .child(appearance_card)
        .child(spotify_card)
        .into_any_element()
}
