use super::{
    render_icon_button, render_pill_button, ui_icon, DesktopModal, FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::GlassPalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::{
    div, px, rgba, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};
use sonata_core::providers::Track;

pub fn render_modal_overlay(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    _window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let card = match view.modal.clone() {
        DesktopModal::None => return div().into_any_element(),
        DesktopModal::SignInBrowser => render_sign_in_modal(view, p, cx),
        DesktopModal::CreatePlaylist => render_text_modal(
            view,
            p,
            "New YouTube Music Playlist",
            "Enter a title for the new playlist on your YouTube Music account.",
            "Create",
            cx,
        ),
        DesktopModal::AddToPlaylist(track) => render_add_to_playlist_modal(view, p, &track, cx),
    };

    div()
        .id("modal-backdrop")
        .absolute()
        .inset_0()
        .bg(rgba(0x00000090))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                this.modal = DesktopModal::None;
                window.focus(&this.focus_handle, cx);
                cx.notify();
            }),
        )
        .child(card)
        .into_any_element()
}

fn modal_shell(
    title: &str,
    width_px: f32,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> gpui_kit::Div {
    div()
        .w(px(width_px))
        .max_w(px(620.0))
        .p_5()
        .rounded_2xl()
        .bg(p.modal_bg)
        .border_1()
        .border_color(p.border)
        .flex()
        .flex_col()
        .gap_4()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.highlight_fg)
                        .child(title.to_string()),
                )
                .child(render_icon_button(
                    "modal-close",
                    IconName::X,
                    16.0,
                    28.0,
                    p.muted,
                    p.pill_bg,
                    p.card_hover,
                    |this, window, cx| {
                        this.modal = DesktopModal::None;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    },
                    cx,
                )),
        )
}

fn render_sign_in_modal(
    view: &SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let mut browser_list = div()
        .id("signin-browser-scroll")
        .w_full()
        .max_h(px(320.0))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_2();

    for (idx, opt) in snap.available_browsers.iter().enumerate() {
        let browser_id = opt.id.to_string();
        let active_browser = snap.signed_in_browser.as_deref() == Some(opt.id);

        browser_list = browser_list.child(
            div()
                .id(SharedString::from(format!("signin-br-{idx}")))
                .w_full()
                .px_3()
                .py_3()
                .rounded_xl()
                .bg(if active_browser {
                    p.accent_soft
                } else {
                    p.card_bg
                })
                .border_1()
                .border_color(if active_browser {
                    p.accent
                } else {
                    p.border_subtle
                })
                .hover(|s| s.bg(p.card_hover))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(move |this, _, _, _| {
                    if !this.snapshot.signing_in {
                        this.backend
                            .send(BackendCmd::SignInWithBrowser(browser_id.clone()));
                    }
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(ui_icon(
                            IconName::Globe,
                            16.0,
                            if opt.detected { p.accent } else { p.muted },
                        ))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if opt.detected {
                                    p.highlight_fg
                                } else {
                                    p.muted
                                })
                                .child(opt.label),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(active_browser, |el| {
                            el.child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(p.accent_soft)
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.accent)
                                    .child("Active"),
                            )
                        })
                        .child(if opt.detected {
                            div()
                                .px_2()
                                .py(px(2.0))
                                .rounded_full()
                                .bg(p.pill_bg)
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.success)
                                .child("Detected")
                        } else {
                            div()
                                .px_2()
                                .py(px(2.0))
                                .rounded_full()
                                .bg(p.pill_bg)
                                .text_xs()
                                .text_color(p.muted)
                                .child("Not Found")
                        }),
                ),
        );
    }

    modal_shell("Sign In to YouTube Music", 480.0, p, cx)
        .child(div().text_xs().text_color(p.muted).child(
            "Choose which browser Sonata should extract your YouTube Music session cookies from:",
        ))
        .when(snap.signing_in, |el| {
            el.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_xl()
                    .bg(p.accent_soft)
                    .border_1()
                    .border_color(p.accent)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.accent)
                    .child("Reading browser cookies and syncing your YouTube Music profile..."),
            )
        })
        .when(snap.sign_in_error.is_some() && !snap.signing_in, |el| {
            el.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_xl()
                    .bg(p.pill_bg)
                    .border_1()
                    .border_color(p.error)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(ui_icon(IconName::CircleAlert, 15.0, p.error))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(p.error)
                            .child(snap.sign_in_error.clone().unwrap_or_default()),
                    ),
            )
        })
        .child(browser_list)
        .child(
            div()
                .w_full()
                .flex()
                .justify_end()
                .child(render_pill_button(
                    "signin-cancel",
                    "Cancel".to_string(),
                    false,
                    p,
                    |this, window, cx| {
                        this.modal = DesktopModal::None;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    },
                    cx,
                )),
        )
        .into_any_element()
}

fn render_text_modal(
    view: &SonataDesktopView,
    p: &GlassPalette,
    title: &str,
    hint: &str,
    confirm_label: &str,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let confirm = confirm_label.to_string();
    modal_shell(title, 460.0, p, cx)
        .child(div().text_xs().text_color(p.muted).child(hint.to_string()))
        .child(
            div()
                .w_full()
                .px_3()
                .py_2()
                .rounded_xl()
                .bg(p.card_bg)
                .border_1()
                .border_color(p.accent)
                .child(Input::new(&view.modal_input).appearance(false)),
        )
        .child(
            div()
                .w_full()
                .flex()
                .justify_end()
                .gap_2()
                .child(render_pill_button(
                    "modal-cancel",
                    "Cancel".to_string(),
                    false,
                    p,
                    |this, window, cx| {
                        this.modal = DesktopModal::None;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    },
                    cx,
                ))
                .child(
                    div()
                        .id("modal-submit")
                        .px_4()
                        .py(px(7.0))
                        .rounded_lg()
                        .bg(p.accent)
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.accent_fg)
                        .on_click(cx.listener(|this, _, window, cx| {
                            let val = this.modal_input.read(cx).value().to_string();
                            this.submit_text_modal(val, window, cx);
                        }))
                        .child(confirm),
                ),
        )
        .into_any_element()
}

fn render_add_to_playlist_modal(
    view: &SonataDesktopView,
    p: &GlassPalette,
    track: &Track,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let playlists = view.snapshot.writable_ytm_playlists();
    let mut list = div()
        .id("add-playlist-scroll")
        .w_full()
        .max_h(px(280.0))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_1();

    if playlists.is_empty() {
        list = list.child(
            div()
                .py_6()
                .flex()
                .justify_center()
                .text_sm()
                .text_color(p.muted)
                .child(if view.snapshot.is_signed_in() {
                    "No writable YouTube Music playlists found."
                } else {
                    "Sign in to YouTube Music to add tracks to playlists."
                }),
        );
    } else {
        for (idx, pl) in playlists.into_iter().enumerate() {
            let track_clone = track.clone();
            let pl_id = pl.id.clone();
            let pl_name = pl.name.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("add-pl-{idx}")))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(p.card_bg)
                    .hover(|s| s.bg(p.card_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.backend.send(BackendCmd::AddTrackToPlaylist {
                            track: track_clone.clone(),
                            playlist_id: pl_id.clone(),
                            playlist_name: pl_name.clone(),
                        });
                        this.modal = DesktopModal::None;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(ui_icon(IconName::ListPlus, 15.0, p.accent))
                            .child(div().text_sm().text_color(p.text).child(pl.name.clone())),
                    )
                    .child(
                        div().text_xs().text_color(p.muted).child(
                            pl.track_count
                                .map(|c| format!("{c} tracks"))
                                .unwrap_or_default(),
                        ),
                    ),
            );
        }
    }

    modal_shell("Add Track to Playlist", 440.0, p, cx)
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(p.pill_bg)
                .text_xs()
                .text_color(p.text)
                .truncate()
                .child(format!("{} — {}", track.title, track.artist)),
        )
        .child(list)
        .into_any_element()
}
