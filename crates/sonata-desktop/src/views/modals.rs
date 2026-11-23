//! Interactive context menus, top-bar account popover, browser sign-in, and playlist modals.
//!
//! 1. **Right-Click Context Menu**: offers track, queue, collection, and artist quick actions (`Play next`, `Add to queue`, `Start radio`, `Go to album/artist`, `Like`, `Dislike`, `Add to playlist`, `Remove`).
//! 2. **Account Popover**: switches to Library / Settings / Sign-out cleanly without blocking navigation.
//! 3. **Modal Dialogs**: handles browser cookie selection (`SignInBrowser`), `CreatePlaylist`, and `AddToPlaylist`.

use super::{ContextMenuKind, DesktopModal, SonataDesktopView};
use crate::backend::{BackendCmd, NavTarget};
use crate::ui::components::popup_panel;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};

impl SonataDesktopView {
    pub fn render_account_popover(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = self
            .snapshot
            .user_profile
            .as_ref()
            .map(|p| p.name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Signed in".to_string());
        let handle = self
            .snapshot
            .user_profile
            .as_ref()
            .and_then(|p| p.handle.clone())
            .unwrap_or_default();

        div()
            .id("acc-pop-backdrop")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.account_menu_open = false;
                    cx.notify();
                }),
            )
            .child(
                popup_panel(&self.palette)
                    .id("acc-pop-card")
                    .absolute()
                    .top(px(58.0))
                    .right(px(16.0))
                    .w(px(230.0))
                    .p(px(8.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
                            .border_b_1()
                            .border_color(self.palette.outline)
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(name),
                            )
                            .when(!handle.is_empty(), |col| {
                                col.child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(self.palette.secondary)
                                        .truncate()
                                        .child(handle),
                                )
                            }),
                    )
                    .child(menu_item_row(
                        "acc-go-lib",
                        IconName::Library,
                        "Your Library",
                        &self.palette,
                        cx.listener(|this, _, _, cx| {
                            this.navigate_to(NavTarget::Library, cx);
                        }),
                    ))
                    .child(menu_item_row(
                        "acc-go-set",
                        IconName::Settings,
                        "Settings",
                        &self.palette,
                        cx.listener(|this, _, _, cx| {
                            this.navigate_to(NavTarget::Settings, cx);
                        }),
                    ))
                    .child(menu_item_row(
                        "acc-signout",
                        IconName::LogOut,
                        "Sign out",
                        &self.palette,
                        cx.listener(|this, _, _, cx| {
                            this.account_menu_open = false;
                            this.backend.send(BackendCmd::SignOut);
                            cx.notify();
                        }),
                    )),
            )
            .into_any_element()
    }

    pub fn render_context_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(menu_state) = self.context_menu.clone() else {
            return div().into_any_element();
        };
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
        let vp_h: f32 = vp.height.into();
        let left = menu_state.pos.x.min((vp_w - 230.0).max(8.0));
        let top = menu_state.pos.y.min((vp_h - 320.0).max(8.0));

        let mut items: Vec<AnyElement> = Vec::new();
        match menu_state.kind {
            ContextMenuKind::Track {
                track,
                playlist_remove_idx,
            } => {
                let tr_next = track.clone();
                let tr_q = track.clone();
                let tr_rad = track.clone();
                let tr_alb = track.clone();
                let tr_art = track.clone();
                let tr_like = track.clone();
                let tr_dis = track.clone();
                let tr_add = track.clone();
                let liked = track.liked;

                items.push(menu_item_row(
                    "ctx-play-next",
                    IconName::ListPlus,
                    "Play next",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend
                            .send(BackendCmd::PlayTrackNext(tr_next.clone()));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-add-q",
                    IconName::ListMusic,
                    "Add to queue",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::AddTrackToQueue(tr_q.clone()));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-radio",
                    IconName::Radio,
                    "Start radio",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend
                            .send(BackendCmd::StartTrackRadio(tr_rad.clone()));
                        cx.notify();
                    }),
                ));
                if !tr_alb.album.is_empty() || tr_alb.album_id.is_some() {
                    items.push(menu_item_row(
                        "ctx-album",
                        IconName::Disc3,
                        "Go to album",
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.open_album_for_track(&tr_alb, cx);
                        }),
                    ));
                }
                items.push(menu_item_row(
                    "ctx-artist",
                    IconName::User,
                    "Go to artist",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.open_artist_by_name(&tr_art.artist, cx);
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-like",
                    IconName::ThumbsUp,
                    if liked { "Remove like" } else { "Like song" },
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::ToggleLike(tr_like.clone()));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-dislike",
                    IconName::ThumbsDown,
                    "Dislike",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::DislikeTrack(tr_dis.clone()));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-add-pl",
                    IconName::Plus,
                    "Add to playlist",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.modal = DesktopModal::AddToPlaylist(tr_add.clone());
                        cx.notify();
                    }),
                ));
                if let Some(rm_idx) = playlist_remove_idx {
                    items.push(menu_item_row(
                        "ctx-rm-pl",
                        IconName::Trash,
                        "Remove from playlist",
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.backend
                                .send(BackendCmd::RemoveTrackFromPlaylist(rm_idx));
                            cx.notify();
                        }),
                    ));
                }
            }
            ContextMenuKind::QueueItem { index, track } => {
                let tr_rad = track.clone();
                let tr_add = track.clone();
                items.push(menu_item_row(
                    "ctx-q-play",
                    IconName::Play,
                    "Play now",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::PlayQueueIndex(index));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-q-rad",
                    IconName::Radio,
                    "Start radio",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend
                            .send(BackendCmd::StartTrackRadio(tr_rad.clone()));
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-q-add",
                    IconName::Plus,
                    "Add to playlist",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.modal = DesktopModal::AddToPlaylist(tr_add.clone());
                        cx.notify();
                    }),
                ));
                items.push(menu_item_row(
                    "ctx-q-rm",
                    IconName::X,
                    "Remove from queue",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::RemoveFromQueue(index));
                        cx.notify();
                    }),
                ));
            }
            ContextMenuKind::Collection {
                id,
                title,
                subtitle,
                thumbnail_url,
                is_album,
            } => {
                items.push(menu_item_row(
                    "ctx-col-open",
                    IconName::ExternalLink,
                    if is_album {
                        "Open album"
                    } else {
                        "Open playlist"
                    },
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        if is_album {
                            this.backend.send(BackendCmd::OpenAlbum {
                                album_id: Some(id.clone()),
                                title: title.clone(),
                                artist: subtitle.clone(),
                                thumbnail_url: thumbnail_url.clone(),
                            });
                        } else {
                            this.backend.send(BackendCmd::OpenCollection {
                                id: id.clone(),
                                title: title.clone(),
                                subtitle: subtitle.clone(),
                                thumbnail_url: thumbnail_url.clone(),
                                editable: false,
                            });
                        }
                        cx.notify();
                    }),
                ));
            }
            ContextMenuKind::Artist { id, name } => {
                items.push(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(self.palette.dim)
                        .truncate()
                        .child(name)
                        .into_any_element(),
                );
                items.push(menu_item_row(
                    "ctx-art-open",
                    IconName::User,
                    "Go to artist",
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::OpenArtist(id.clone()));
                        cx.notify();
                    }),
                ));
            }
        }

        div()
            .id("ctx-menu-backdrop")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }),
            )
            .child(
                popup_panel(&self.palette)
                    .id("ctx-menu-box")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(216.0))
                    .p(px(6.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(items),
            )
            .into_any_element()
    }

    pub fn render_modal_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog_content: AnyElement = match self.modal.clone() {
            DesktopModal::None => return div().into_any_element(),
            DesktopModal::SignInBrowser => {
                let browsers = self.snapshot.available_browsers.clone();
                let signing_in = self.snapshot.signing_in;
                let err = self.snapshot.sign_in_error.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Sign in to YouTube Music"),
                            )
                            .child(
                                Button::new("mdl-close")
                                    .ghost()
                                    .small()
                                    .icon(IconName::X)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.backend.send(BackendCmd::ClearSignInError);
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(self.palette.secondary)
                            .child(
                                "Select the browser where you are signed into music.youtube.com:",
                            ),
                    )
                    .when_some(err, |d, msg| {
                        d.child(
                            div()
                                .p(px(10.0))
                                .rounded(px(self.palette.radii.item))
                                .bg(self.palette.danger.opacity(0.15))
                                .text_size(px(12.0))
                                .text_color(self.palette.danger)
                                .child(msg),
                        )
                    })
                    .child(div().flex().flex_col().gap(px(6.0)).children(
                        browsers.into_iter().enumerate().map(|(i, b)| {
                            let bid = b.id;
                            Button::new(SharedString::from(format!("mdl-br-{i}")))
                                .secondary()
                                .label(b.label)
                                .when(signing_in, |btn| btn.opacity(0.5))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.backend
                                        .send(BackendCmd::SignInWithBrowser(bid.to_string()));
                                    this.modal = DesktopModal::None;
                                    cx.notify();
                                }))
                        }),
                    ))
                    .into_any_element()
            }
            DesktopModal::CreatePlaylist => div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .child("Create playlist"),
                        )
                        .child(
                            Button::new("mdl-cp-close")
                                .ghost()
                                .small()
                                .icon(IconName::X)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.modal = DesktopModal::None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(self.palette.radii.control))
                        .bg(self.palette.surface)
                        .border_1()
                        .border_color(self.palette.outline)
                        .child(Input::new(&self.playlist_name_input).appearance(false)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            Button::new("mdl-cp-cancel")
                                .secondary()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.modal = DesktopModal::None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("mdl-cp-create")
                                .primary()
                                .label("Create")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let name = this
                                        .playlist_name_input
                                        .read(cx)
                                        .value()
                                        .trim()
                                        .to_string();
                                    if !name.is_empty() {
                                        this.backend.send(BackendCmd::CreateYtmPlaylist(name));
                                    }
                                    this.modal = DesktopModal::None;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
            DesktopModal::AddToPlaylist(track) => {
                let playlists = self.snapshot.writable_ytm_playlists();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Add to playlist"),
                            )
                            .child(
                                Button::new("mdl-ap-close")
                                    .ghost()
                                    .small()
                                    .icon(IconName::X)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.secondary)
                            .truncate()
                            .child(format!("{} • {}", track.title, track.artist)),
                    )
                    .child(if playlists.is_empty() {
                        div()
                            .py(px(14.0))
                            .text_size(px(13.0))
                            .text_color(self.palette.dim)
                            .child("Sign in and sync playlists first.")
                            .into_any_element()
                    } else {
                        div()
                            .max_h(px(280.0))
                            .id("mdl-ap-scroll")
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .children(playlists.into_iter().enumerate().map(|(i, pl)| {
                                let tr = track.clone();
                                let pid = pl.id.clone();
                                let pname = pl.name.clone();
                                div()
                                    .id(SharedString::from(format!("mdl-ap-pl-{i}")))
                                    .px(px(12.0))
                                    .py(px(10.0))
                                    .rounded(px(self.palette.radii.item))
                                    .bg(self.palette.surface)
                                    .hover(|s| s.bg(self.palette.surface_hover))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.backend.send(BackendCmd::AddTrackToPlaylist {
                                            track: tr.clone(),
                                            playlist_id: pid.clone(),
                                            playlist_name: pname.clone(),
                                        });
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    }))
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(pl.name)
                            }))
                            .into_any_element()
                    })
                    .into_any_element()
            }
        };

        div()
            .id("modal-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui_kit::black().opacity(0.55))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.modal = DesktopModal::None;
                    cx.notify();
                }),
            )
            .child(
                popup_panel(&self.palette)
                    .id("modal-card")
                    .w(px(420.0))
                    .max_w_full()
                    .p(px(20.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(dialog_content),
            )
            .into_any_element()
    }
}

fn menu_item_row(
    id: impl Into<gpui_kit::ElementId>,
    icon: IconName,
    label: &'static str,
    palette: &crate::theme::DesktopPalette,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(34.0))
        .px(px(10.0))
        .rounded(px(palette.radii.item))
        .cursor_pointer()
        .hover(|s| s.bg(palette.surface_hover))
        .on_click(on_click)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(Icon::new(icon).size(px(15.0)).text_color(palette.secondary))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.text)
                .child(label),
        )
        .into_any_element()
}
