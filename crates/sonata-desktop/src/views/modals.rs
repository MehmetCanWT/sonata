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
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};
use sonata_core::playlists::is_personal_track_id;
use sonata_core::providers::servers::{ServerKind, ServerProfile};
use std::path::PathBuf;

impl SonataDesktopView {
    pub fn render_account_popover(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = sonata_runtime::i18n::strings(self.settings.language);
        let name = self
            .snapshot
            .user_profile
            .as_ref()
            .map(|p| p.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| s.signed_in.to_string());
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
                        s.your_library,
                        &self.palette,
                        cx.listener(|this, _, _, cx| {
                            this.navigate_to(NavTarget::Library, cx);
                        }),
                    ))
                    .child(menu_item_row(
                        "acc-go-set",
                        IconName::Settings,
                        s.settings,
                        &self.palette,
                        cx.listener(|this, _, _, cx| {
                            this.navigate_to(NavTarget::Settings, cx);
                        }),
                    ))
                    .child(menu_item_row(
                        "acc-signout",
                        IconName::LogOut,
                        s.sign_out,
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

        let s = sonata_runtime::i18n::strings(self.settings.language);
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
                let tr_dl = track.clone();
                let liked = track.liked;
                let is_downloaded = self.snapshot.is_track_downloaded(&track.id);
                let is_downloading = self.snapshot.is_track_downloading(&track.id);

                items.push(menu_item_row(
                    "ctx-play-next",
                    IconName::ListPlus,
                    s.play_next,
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
                    s.add_to_queue,
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.backend.send(BackendCmd::AddTrackToQueue(tr_q.clone()));
                        cx.notify();
                    }),
                ));
                let can_ytm_actions =
                    !self.snapshot.config.personal_only_mode && !is_personal_track_id(&track.id);
                if can_ytm_actions {
                    items.push(menu_item_row(
                        "ctx-radio",
                        IconName::Radio,
                        s.start_radio,
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.backend
                                .send(BackendCmd::StartTrackRadio(tr_rad.clone()));
                            cx.notify();
                        }),
                    ));
                }
                if !tr_alb.album.is_empty() || tr_alb.album_id.is_some() {
                    items.push(menu_item_row(
                        "ctx-album",
                        IconName::Disc3,
                        s.go_to_album,
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.open_album_for_track(&tr_alb, cx);
                        }),
                    ));
                }
                if can_ytm_actions {
                    items.push(menu_item_row(
                        "ctx-artist",
                        IconName::User,
                        s.go_to_artist,
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.open_artist_by_name(&tr_art.artist, cx);
                        }),
                    ));
                    items.push(menu_item_row(
                        "ctx-like",
                        IconName::ThumbsUp,
                        if liked { s.remove_like } else { s.like },
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
                        s.dislike,
                        &self.palette,
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu = None;
                            this.backend.send(BackendCmd::DislikeTrack(tr_dis.clone()));
                            cx.notify();
                        }),
                    ));
                } else {
                    let _ = (tr_art, tr_like, tr_dis, liked);
                }
                items.push(menu_item_row(
                    "ctx-add-pl",
                    IconName::Plus,
                    s.add_to_playlist,
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        this.modal = DesktopModal::AddToPlaylist(tr_add.clone());
                        cx.notify();
                    }),
                ));
                let dl_label = if is_downloading {
                    s.downloading
                } else if is_downloaded {
                    s.remove_download
                } else {
                    s.download
                };
                items.push(menu_item_row(
                    "ctx-dl",
                    if is_downloaded {
                        IconName::Trash
                    } else {
                        IconName::Download
                    },
                    dl_label,
                    &self.palette,
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu = None;
                        if is_downloading {
                            return;
                        }
                        if is_downloaded {
                            this.backend
                                .send(BackendCmd::RemoveDownloadedTrack(tr_dl.id.clone()));
                        } else {
                            this.backend.send(BackendCmd::DownloadTrack {
                                track: tr_dl.clone(),
                                collection: None,
                            });
                        }
                        cx.notify();
                    }),
                ));
                if let Some(rm_idx) = playlist_remove_idx {
                    items.push(menu_item_row(
                        "ctx-rm-pl",
                        IconName::Trash,
                        s.remove_from_playlist,
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
                    s.play_now,
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
                    s.start_radio,
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
            DesktopModal::EditPlaylist {
                playlist_id,
                cover_path,
            } => {
                let s = sonata_runtime::i18n::strings(self.settings.language);
                let pid_save = playlist_id.clone();
                let pid_pick = playlist_id.clone();
                let cover_save = cover_path.clone();
                let cover_label = cover_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| format!("Cover: {n}"))
                    .unwrap_or_else(|| match self.settings.language {
                        sonata_runtime::i18n::AppLanguage::Tr => "Kapak görseli seç...".to_string(),
                        sonata_runtime::i18n::AppLanguage::Ru => "Выбрать обложку...".to_string(),
                        sonata_runtime::i18n::AppLanguage::En => {
                            "Choose cover image...".to_string()
                        }
                    });
                let heading = match self.settings.language {
                    sonata_runtime::i18n::AppLanguage::Tr => "Çalma listesini düzenle",
                    sonata_runtime::i18n::AppLanguage::Ru => "Редактировать плейлист",
                    sonata_runtime::i18n::AppLanguage::En => "Edit playlist",
                };
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
                                    .child(heading),
                            )
                            .child(
                                Button::new("mdl-ep-close")
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
                        Button::new("mdl-ep-pick-cover")
                            .secondary()
                            .icon(IconName::Image)
                            .label(cover_label)
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                                    files: true,
                                    directories: false,
                                    multiple: false,
                                    prompt: Some("Select playlist cover image".into()),
                                });
                                let pid = pid_pick.clone();
                                cx.spawn(async move |weak, cx| {
                                    if let Ok(Ok(Some(paths))) = rx.await {
                                        if let Some(path) = paths.into_iter().next() {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.modal = DesktopModal::EditPlaylist {
                                                    playlist_id: pid,
                                                    cover_path: Some(path),
                                                };
                                                cx.notify();
                                            });
                                        }
                                    }
                                })
                                .detach();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                Button::new("mdl-ep-cancel")
                                    .secondary()
                                    .label(s.cancel)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("mdl-ep-save")
                                    .primary()
                                    .label(match self.settings.language {
                                        sonata_runtime::i18n::AppLanguage::Tr => "Kaydet",
                                        sonata_runtime::i18n::AppLanguage::Ru => "Сохранить",
                                        sonata_runtime::i18n::AppLanguage::En => "Save",
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let name =
                                            this.playlist_name_input.read(cx).value().to_string();
                                        let modal = DesktopModal::EditPlaylist {
                                            playlist_id: pid_save.clone(),
                                            cover_path: cover_save.clone(),
                                        };
                                        if let Some(cmd) = super::playlist_modal_submit_cmd(
                                            &modal,
                                            &name,
                                            this.create_playlist_personal,
                                            this.snapshot.config.personal_only_mode,
                                            this.snapshot.is_signed_in(),
                                        ) {
                                            this.backend.send(cmd);
                                        }
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .into_any_element()
            }
            DesktopModal::CreatePlaylist => {
                let s = sonata_runtime::i18n::strings(self.settings.language);
                let can_ytm =
                    !self.snapshot.config.personal_only_mode && self.snapshot.is_signed_in();
                let is_personal = self.create_playlist_personal || !can_ytm;
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
                                    .child(s.new_playlist),
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
                    .when(can_ytm, |d| {
                        d.child(
                            TabBar::new("mdl-cp-kind")
                                .pill()
                                .small()
                                .selected_index(if is_personal { 0 } else { 1 })
                                .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                    this.create_playlist_personal = *idx == 0;
                                    cx.notify();
                                }))
                                .child(Tab::new().label(s.personal_playlists))
                                .child(Tab::new().label(s.youtube_music)),
                        )
                    })
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
                                    .label(s.cancel)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("mdl-cp-create")
                                    .primary()
                                    .label(s.create)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let name =
                                            this.playlist_name_input.read(cx).value().to_string();
                                        if let Some(cmd) = super::playlist_modal_submit_cmd(
                                            &DesktopModal::CreatePlaylist,
                                            &name,
                                            this.create_playlist_personal,
                                            this.snapshot.config.personal_only_mode,
                                            this.snapshot.is_signed_in(),
                                        ) {
                                            this.backend.send(cmd);
                                        }
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .into_any_element()
            }
            DesktopModal::AddToPlaylist(track) => {
                let s = sonata_runtime::i18n::strings(self.settings.language);
                let targets = self.snapshot.playlist_add_targets(&track);
                let is_personal_track = is_personal_track_id(&track.id);
                let has_any = !targets.personal.is_empty() || !targets.ytm.is_empty();
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
                                    .child(s.add_to_playlist),
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
                    .child(
                        Button::new("mdl-ap-new-personal")
                            .secondary()
                            .small()
                            .icon(IconName::Plus)
                            .label(format!("{} ({})", s.new_playlist, s.personal_playlists))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_playlist_personal = true;
                                this.modal = DesktopModal::CreatePlaylist;
                                this.playlist_name_input
                                    .update(cx, |st, cx| st.set_value("", window, cx));
                                this.playlist_name_input
                                    .update(cx, |st, cx| st.focus(window, cx));
                                cx.notify();
                            })),
                    )
                    .child(if !has_any {
                        div()
                            .py(px(12.0))
                            .text_size(px(13.0))
                            .text_color(self.palette.dim)
                            .child(if is_personal_track {
                                "Create a Personal Playlist first to organize local and server tracks."
                            } else {
                                "Create a Personal Playlist or sign in to YouTube Music."
                            })
                            .into_any_element()
                    } else {
                        let mut list = div()
                            .max_h(px(280.0))
                            .id("mdl-ap-scroll")
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(6.0));
                        if !targets.personal.is_empty() {
                            list = list.child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.secondary)
                                    .child(s.personal_playlists.to_uppercase()),
                            );
                            for (i, pl) in targets.personal.into_iter().enumerate() {
                                let tr = track.clone();
                                let pid = pl.id.clone();
                                list = list.child(
                                    div()
                                        .id(SharedString::from(format!("mdl-ap-ppl-{i}")))
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .rounded(px(self.palette.radii.item))
                                        .bg(self.palette.surface)
                                        .hover(|st| st.bg(self.palette.surface_hover))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.backend.send(
                                                BackendCmd::AddTrackToPersonalPlaylist {
                                                    track: tr.clone(),
                                                    playlist_id: pid.clone(),
                                                },
                                            );
                                            this.modal = DesktopModal::None;
                                            cx.notify();
                                        }))
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(format!("{} ({})", pl.name, pl.tracks.len())),
                                );
                            }
                        }
                        if !targets.ytm.is_empty() {
                            list = list.child(
                                div()
                                    .pt(px(4.0))
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.secondary)
                                    .child(s.youtube_music.to_uppercase()),
                            );
                            for (i, pl) in targets.ytm.into_iter().enumerate() {
                                let tr = track.clone();
                                let pid = pl.id.clone();
                                let pname = pl.name.clone();
                                list = list.child(
                                    div()
                                        .id(SharedString::from(format!("mdl-ap-pl-{i}")))
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .rounded(px(self.palette.radii.item))
                                        .bg(self.palette.surface)
                                        .hover(|st| st.bg(self.palette.surface_hover))
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
                                        .child(pl.name),
                                );
                            }
                        }
                        list.into_any_element()
                    })
                    .into_any_element()
            }
            DesktopModal::AddServer => {
                let s = sonata_runtime::i18n::strings(self.settings.language);
                let editing = self.server_editing_id.is_some();
                let kind_idx = match self.server_modal_kind {
                    ServerKind::Jellyfin => 0,
                    ServerKind::Subsonic => 1,
                    ServerKind::Smb => 2,
                    ServerKind::WebDav => 3,
                };
                let token_label = match self.server_modal_kind {
                    ServerKind::Jellyfin => "API Key (optional if using password)",
                    ServerKind::Subsonic => "API Key (optional)",
                    ServerKind::WebDav => "Bearer Token (optional if using user/pass)",
                    ServerKind::Smb => "",
                };
                let url_label = if self.server_modal_kind == ServerKind::Smb {
                    "SMB Host / URL (smb://host/share or \\\\host\\share)"
                } else {
                    "Server URL"
                };
                let test_res = self.snapshot.server_test_result.clone();
                let kind_cur = self.server_modal_kind;
                let edit_id = self.server_editing_id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
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
                                    .child(if editing {
                                        format!("{} {}", s.edit, s.servers)
                                    } else {
                                        s.add_server.to_string()
                                    }),
                            )
                            .child(
                                Button::new("mdl-srv-close")
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
                        TabBar::new("mdl-srv-kind")
                            .pill()
                            .small()
                            .selected_index(kind_idx)
                            .on_click(cx.listener(|this, idx: &usize, _, cx| {
                                this.server_modal_kind = match *idx {
                                    1 => ServerKind::Subsonic,
                                    2 => ServerKind::Smb,
                                    3 => ServerKind::WebDav,
                                    _ => ServerKind::Jellyfin,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label(s.jellyfin))
                            .child(Tab::new().label(s.subsonic))
                            .child(Tab::new().label(s.smb))
                            .child(Tab::new().label(s.webdav)),
                    )
                    .child(labeled_input(
                        "Name",
                        &self.server_name_input,
                        &self.palette,
                    ))
                    .child(labeled_input(
                        url_label,
                        &self.server_url_input,
                        &self.palette,
                    ))
                    .when(kind_cur == ServerKind::Smb, |d| {
                        d.child(labeled_input(
                            s.smb_share,
                            &self.server_share_input,
                            &self.palette,
                        ))
                        .child(labeled_input(
                            s.smb_domain,
                            &self.server_domain_input,
                            &self.palette,
                        ))
                        .child(labeled_input(
                            s.smb_mount_path,
                            &self.server_mount_input,
                            &self.palette,
                        ))
                    })
                    .child(labeled_input(
                        "Username",
                        &self.server_user_input,
                        &self.palette,
                    ))
                    .child(labeled_input(
                        "Password",
                        &self.server_pass_input,
                        &self.palette,
                    ))
                    .when(
                        kind_cur == ServerKind::Jellyfin
                            || kind_cur == ServerKind::Subsonic
                            || kind_cur == ServerKind::WebDav,
                        |d| {
                            d.child(labeled_input(
                                token_label,
                                &self.server_token_input,
                                &self.palette,
                            ))
                        },
                    )
                    .when_some(test_res, |d, res| {
                        let (ok, msg) = match res {
                            Ok(m) => (true, m),
                            Err(e) => (false, e),
                        };
                        d.child(
                            div()
                                .p(px(8.0))
                                .rounded(px(self.palette.radii.item))
                                .bg(if ok {
                                    self.palette.accent.opacity(0.15)
                                } else {
                                    self.palette.danger.opacity(0.15)
                                })
                                .text_size(px(12.0))
                                .text_color(if ok {
                                    self.palette.accent
                                } else {
                                    self.palette.danger
                                })
                                .child(msg),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                Button::new("mdl-srv-test")
                                    .secondary()
                                    .label(s.test_connection)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let name =
                                            this.server_name_input.read(cx).value().to_string();
                                        let url =
                                            this.server_url_input.read(cx).value().to_string();
                                        let user =
                                            this.server_user_input.read(cx).value().to_string();
                                        let pass =
                                            this.server_pass_input.read(cx).value().to_string();
                                        let tok =
                                            this.server_token_input.read(cx).value().to_string();
                                        let share =
                                            this.server_share_input.read(cx).value().to_string();
                                        let domain =
                                            this.server_domain_input.read(cx).value().to_string();
                                        let mount = this
                                            .server_mount_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_string();
                                        this.backend.send(BackendCmd::TestServerDraft {
                                            name,
                                            kind: kind_cur,
                                            base_url: url,
                                            username: Some(user),
                                            password: Some(pass),
                                            api_key: (kind_cur == ServerKind::Jellyfin
                                                || kind_cur == ServerKind::Subsonic)
                                                .then(|| tok.clone()),
                                            bearer_token: (kind_cur == ServerKind::WebDav)
                                                .then_some(tok),
                                            share: (kind_cur == ServerKind::Smb).then_some(share),
                                            domain: (kind_cur == ServerKind::Smb).then_some(domain),
                                            local_mount_path: (kind_cur == ServerKind::Smb
                                                && !mount.is_empty())
                                            .then(|| PathBuf::from(mount)),
                                        });
                                    })),
                            )
                            .child(
                                Button::new("mdl-srv-cancel")
                                    .secondary()
                                    .label(s.cancel)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.modal = DesktopModal::None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("mdl-srv-save")
                                    .primary()
                                    .label(s.save)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let name =
                                            this.server_name_input.read(cx).value().to_string();
                                        let url =
                                            this.server_url_input.read(cx).value().to_string();
                                        let user =
                                            this.server_user_input.read(cx).value().to_string();
                                        let pass =
                                            this.server_pass_input.read(cx).value().to_string();
                                        let tok =
                                            this.server_token_input.read(cx).value().to_string();
                                        let share =
                                            this.server_share_input.read(cx).value().to_string();
                                        let domain =
                                            this.server_domain_input.read(cx).value().to_string();
                                        let mount = this
                                            .server_mount_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_string();
                                        if !url.trim().is_empty()
                                            || (kind_cur == ServerKind::Smb && !mount.is_empty())
                                        {
                                            this.backend.send(BackendCmd::SaveServerProfile {
                                                id: edit_id.clone(),
                                                name,
                                                kind: kind_cur,
                                                base_url: url,
                                                username: Some(user),
                                                password: Some(pass),
                                                api_key: (kind_cur == ServerKind::Jellyfin
                                                    || kind_cur == ServerKind::Subsonic)
                                                    .then(|| tok.clone()),
                                                bearer_token: (kind_cur == ServerKind::WebDav)
                                                    .then_some(tok),
                                                share: (kind_cur == ServerKind::Smb)
                                                    .then_some(share),
                                                domain: (kind_cur == ServerKind::Smb)
                                                    .then_some(domain),
                                                local_mount_path: (kind_cur == ServerKind::Smb
                                                    && !mount.is_empty())
                                                .then(|| PathBuf::from(mount)),
                                            });
                                            this.modal = DesktopModal::None;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
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

fn labeled_input(
    label: &'static str,
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    palette: &crate::theme::DesktopPalette,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.secondary)
                .child(label),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .rounded(px(palette.radii.control))
                .bg(palette.surface)
                .border_1()
                .border_color(palette.outline)
                .child(Input::new(input).appearance(false)),
        )
        .into_any_element()
}

impl SonataDesktopView {
    pub fn open_add_server_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.server_editing_id = None;
        self.server_modal_kind = ServerKind::Jellyfin;
        self.server_name_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_url_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_user_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_pass_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_token_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_share_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_domain_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.server_mount_input
            .update(cx, |st, cx| st.set_value("", window, cx));
        self.modal = DesktopModal::AddServer;
        self.server_name_input
            .update(cx, |st, cx| st.focus(window, cx));
        cx.notify();
    }

    pub fn open_edit_server_modal(
        &mut self,
        profile: &ServerProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.server_editing_id = Some(profile.id.clone());
        self.server_modal_kind = profile.kind;
        self.server_name_input
            .update(cx, |st, cx| st.set_value(profile.name.clone(), window, cx));
        self.server_url_input.update(cx, |st, cx| {
            st.set_value(profile.base_url.clone(), window, cx);
        });
        self.server_user_input.update(cx, |st, cx| {
            st.set_value(profile.username.clone().unwrap_or_default(), window, cx);
        });
        self.server_pass_input.update(cx, |st, cx| {
            st.set_value(profile.password.clone().unwrap_or_default(), window, cx);
        });
        let tok = profile
            .api_key
            .clone()
            .or_else(|| profile.bearer_token.clone())
            .unwrap_or_default();
        self.server_token_input
            .update(cx, |st, cx| st.set_value(tok, window, cx));
        self.server_share_input.update(cx, |st, cx| {
            st.set_value(profile.share.clone().unwrap_or_default(), window, cx);
        });
        self.server_domain_input.update(cx, |st, cx| {
            st.set_value(profile.domain.clone().unwrap_or_default(), window, cx);
        });
        self.server_mount_input.update(cx, |st, cx| {
            st.set_value(
                profile
                    .local_mount_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                window,
                cx,
            );
        });
        self.modal = DesktopModal::AddServer;
        self.server_name_input
            .update(cx, |st, cx| st.focus(window, cx));
        cx.notify();
    }
}
