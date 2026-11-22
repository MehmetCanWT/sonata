use super::{render_avatar, render_pill_button, ui_icon, FluentDivExt, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WindowBackgroundAppearance,
};
use sonata_core::config::AppConfig;
use sonata_core::hyprland;
use sonata_core::updater::CURRENT_VERSION;

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
            "JioSaavn 320kbps Audio Module",
            "Include JioSaavn catalog in search and upgrade <300kbps streams to 320kbps AAC",
            render_pill_button(
                "set-jiosaavn",
                if cfg.jiosaavn_enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                cfg.jiosaavn_enabled,
                p,
                |this, _, _| this.backend.send(BackendCmd::ToggleJioSaavn),
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
            "Animated Album Artwork",
            "Display looping Apple Music, Tidal & Community motion covers in players",
            render_pill_button(
                "set-motion-covers",
                if cfg.motion_covers && !cfg.reduce_motion {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                cfg.motion_covers && !cfg.reduce_motion,
                p,
                |this, _, _| this.backend.send(BackendCmd::ToggleMotionCovers),
                cx,
            ),
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
        ));

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

    let update_subtitle = if snap.checking_updates {
        "Checking GitHub Releases for updates...".to_string()
    } else if let Some(info) = &snap.update_info {
        if info.update_available {
            format!(
                "Update v{} available — {}",
                info.latest_version, info.release_url
            )
        } else {
            format!("Up to date (latest: v{})", info.latest_version)
        }
    } else {
        "Check GitHub Releases for newer desktop builds".to_string()
    };
    let mut updates_card = section_card("UPDATES & ABOUT", p).child(setting_row(
        &format!("Sonata Desktop v{CURRENT_VERSION}"),
        &update_subtitle,
        render_pill_button(
            "set-check-updates",
            if snap.checking_updates {
                "Checking...".to_string()
            } else {
                "Check for Updates".to_string()
            },
            snap.update_info
                .as_ref()
                .is_some_and(|info| info.update_available),
            p,
            |this, _, _| this.backend.send(BackendCmd::CheckForUpdates),
            cx,
        ),
        p,
    ));
    if let Some(info) = snap
        .update_info
        .as_ref()
        .filter(|info| info.update_available)
    {
        let download_url = info
            .appimage_download_url
            .clone()
            .unwrap_or_else(|| info.release_url.clone());
        let release_url = info.release_url.clone();
        updates_card = updates_card.child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(p.accent_soft)
                .border_1()
                .border_color(p.accent)
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.accent)
                                .child(format!(
                                    "New release available: v{} → v{}",
                                    info.current_version, info.latest_version
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .child(release_url.clone()),
                        ),
                )
                .child(
                    div()
                        .id("set-open-release-url")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(p.accent)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.cta_fg)
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            cx.open_url(&download_url);
                        })
                        .child("Download Update ↗"),
                ),
        );
    }

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
        .child(updates_card)
        .into_any_element()
}
