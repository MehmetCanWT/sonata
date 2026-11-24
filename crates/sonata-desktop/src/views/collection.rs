//! Dedicated Album and Playlist collection pages with scrolling `header_wash` and numbered tracks.
//!
//! 1. **Album View (`render_album_page`)**: displays cover art, adaptive `fit_title_size` headline, clickable artists, release year/type/duration, `Play`/`Shuffle`/`Save` controls, numbered track list, and `Other versions`.
//! 2. **Playlist View (`render_playlist_page`)**: displays artwork, playlist heading/subtitle, `Play`/`Shuffle`/`Refresh` controls, track list with inline removal on writable playlists, and scroll-edge fades.

use super::home::TrackRowOpts;
use super::{DesktopModal, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::{HERO_ASPECT, PAGE_INSET_X};
use crate::ui::components::{
    artist_links_row, cover_art_tile, delayed_skeleton_block, fit_title_size, header_wash,
    scroll_edge_fades, section_header,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Icon;
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};

impl SonataDesktopView {
    pub fn render_album_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let vp_w: f32 = window.viewport_size().width.into();
        let (sb_w, rp_w) = self.settings.resolve_shell_widths(vp_w);
        let content_w = (vp_w - sb_w - rp_w - 64.0).max(320.0);

        let album = self.snapshot.album_detail.clone();
        let loading = self.snapshot.album_loading;
        let error = self.snapshot.album_error.clone();
        let started = self.loading_started.get("album").copied();
        let scroll_fades = self.settings.scroll_fades;
        let adaptive = self.settings.adaptive_art_color;

        let Some(alb) = album else {
            let body = if loading {
                delayed_skeleton_block(
                    "alb-sk",
                    started,
                    None,
                    220.0,
                    self.palette.radii.control,
                    &self.palette,
                )
            } else {
                div()
                    .p(px(20.0))
                    .text_size(px(14.0))
                    .text_color(self.palette.secondary)
                    .child(error.unwrap_or_else(|| "Album not loaded".to_string()))
                    .into_any_element()
            };
            return div().p(px(PAGE_INSET_X)).child(body).into_any_element();
        };

        let cover = self.media_for_url(alb.cover_url.as_deref(), cx);
        let title_pt = fit_title_size(&alb.title);
        let tracks = alb.tracks.clone();
        let tracks_play = tracks.clone();
        let tracks_shuf = tracks.clone();
        let can_save = alb.playlist_id.is_some();
        let is_saved = alb.saved;

        let mut meta_parts = vec![alb.release_type.clone()];
        if let Some(ref y) = alb.year {
            meta_parts.push(y.clone());
        }
        if !tracks.is_empty() {
            meta_parts.push(format!("{} tracks", tracks.len()));
        }
        if let Some(ref dur) = alb.duration_text {
            meta_parts.push(dur.clone());
        }
        let meta_line = meta_parts.join(" • ");

        let mut artist_row = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_wrap()
            .gap(px(6.0));
        if !alb.artists.is_empty() {
            let total = alb.artists.len();
            for (idx, (name, id_opt)) in alb.artists.iter().enumerate() {
                let name_c = name.clone();
                let id_c = id_opt.clone();
                artist_row = artist_row.child(
                    div()
                        .id(SharedString::from(format!("alb-hdr-art-{idx}")))
                        .cursor_pointer()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(self.palette.text)
                        .hover(|s| s.underline())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(ref aid) = id_c {
                                    this.backend.send(BackendCmd::OpenArtist(aid.clone()));
                                } else {
                                    this.open_artist_by_name(&name_c, cx);
                                }
                            }),
                        )
                        .child(name.clone()),
                );
                if idx + 1 < total {
                    artist_row = artist_row.child(
                        div()
                            .text_size(px(14.0))
                            .text_color(self.palette.secondary)
                            .child(","),
                    );
                }
            }
        } else if !alb.subtitle.is_empty() {
            artist_row = artist_row.child(artist_links_row(
                "alb-sub-art",
                &alb.subtitle,
                self.palette.text,
                14.0,
                {
                    let view = cx.entity().downgrade();
                    move |name, _, cx| {
                        let _ = view.update(cx, |this, cx| this.open_artist_by_name(&name, cx));
                    }
                },
            ));
        }

        let hero = div()
            .flex()
            .flex_row()
            .items_end()
            .flex_wrap()
            .gap(px(22.0))
            .child(cover_art_tile(
                "alb-hero-cov",
                cover.as_ref(),
                188.0,
                false,
                &self.palette,
                false,
                None,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(240.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.secondary)
                            .child(alb.release_type.to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(px(title_pt))
                            .line_height(px(title_pt + 6.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child(alb.title.clone()),
                    )
                    .child(artist_row)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(self.palette.secondary)
                            .child(meta_line),
                    )
                    .child(
                        div()
                            .pt(px(6.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(
                                Button::new("alb-play-all")
                                    .primary()
                                    .icon(IconName::Play)
                                    .label(
                                        sonata_runtime::i18n::strings(self.settings.language).play,
                                    )
                                    .when(tracks_play.is_empty(), |b| b.opacity(0.45))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        if !tracks_play.is_empty() {
                                            this.backend.send(BackendCmd::PlayTracks {
                                                tracks: tracks_play.clone(),
                                                start_index: 0,
                                                shuffle: false,
                                            });
                                        }
                                    })),
                            )
                            .child(
                                Button::new("alb-shuf")
                                    .secondary()
                                    .icon(IconName::Shuffle)
                                    .label(
                                        sonata_runtime::i18n::strings(self.settings.language)
                                            .shuffle,
                                    )
                                    .when(tracks_shuf.is_empty(), |b| b.opacity(0.45))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        if !tracks_shuf.is_empty() {
                                            this.backend.send(BackendCmd::PlayTracks {
                                                tracks: tracks_shuf.clone(),
                                                start_index: 0,
                                                shuffle: true,
                                            });
                                        }
                                    })),
                            )
                            .when(can_save, |r| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                r.child(
                                    Button::new("alb-save")
                                        .secondary()
                                        .icon(if is_saved {
                                            IconName::Check
                                        } else {
                                            IconName::Plus
                                        })
                                        .label(if is_saved { s.saved } else { s.save_to_library })
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.backend.send(BackendCmd::ToggleSaveAlbum);
                                        })),
                                )
                            })
                            .child({
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                let is_pinned = self.settings.is_pinned(&alb.id);
                                let pin_item = crate::settings::PinnedCollection {
                                    id: alb.id.clone(),
                                    title: alb.title.clone(),
                                    subtitle: alb.subtitle.clone(),
                                    thumbnail_url: alb.cover_url.clone(),
                                    is_album: true,
                                };
                                Button::new("alb-pin")
                                    .secondary()
                                    .icon(if is_pinned {
                                        IconName::Check
                                    } else {
                                        IconName::ArrowUp
                                    })
                                    .label(if is_pinned { s.pinned } else { s.pin })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_pin_collection(pin_item.clone(), cx);
                                    }))
                            })
                            .when(!tracks.is_empty(), |row| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                let alb_id = alb.id.clone();
                                let alb_title = alb.title.clone();
                                let alb_cover = alb.cover_url.clone();
                                let tracks_dl = tracks.clone();
                                let all_dl = tracks_dl
                                    .iter()
                                    .all(|t| self.snapshot.is_track_downloaded(&t.id));
                                let is_col_dl =
                                    all_dl || self.snapshot.is_collection_downloaded(&alb_id);
                                row.child(
                                    Button::new("alb-dl")
                                        .secondary()
                                        .icon(if is_col_dl {
                                            IconName::Trash
                                        } else {
                                            IconName::Download
                                        })
                                        .label(if is_col_dl {
                                            s.remove_download
                                        } else {
                                            s.download
                                        })
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            if is_col_dl {
                                                this.backend.send(
                                                    BackendCmd::RemoveDownloadedCollection(
                                                        alb_id.clone(),
                                                    ),
                                                );
                                            } else {
                                                this.backend.send(BackendCmd::DownloadCollection {
                                                    collection_id: alb_id.clone(),
                                                    title: alb_title.clone(),
                                                    source:
                                                        sonata_core::providers::ProviderSource::YouTubeMusic,
                                                    artwork_url: alb_cover.clone(),
                                                    tracks: tracks_dl.clone(),
                                                });
                                            }
                                        })),
                                )
                            }),
                    ),
            );

        let tracks_arc: std::sync::Arc<[sonata_core::providers::Track]> =
            std::sync::Arc::from(tracks.as_slice());
        let mut track_list = div().flex().flex_col().gap(px(2.0));
        for i in 0..tracks_arc.len() {
            track_list = track_list.child(self.render_track_row(
                "alb-tr",
                i,
                &tracks_arc,
                TrackRowOpts {
                    show_index: true,
                    ..Default::default()
                },
                cx,
            ));
        }

        let page = div()
            .relative()
            .flex()
            .flex_col()
            .child(header_wash(cover.as_ref(), &self.palette, adaptive, 260.0))
            .child(
                div()
                    .relative()
                    .px(px(PAGE_INSET_X))
                    .pt(px(28.0))
                    .pb(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(div().pb(px(12.0)).child(hero))
                    .child(
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(self.palette.outline.opacity(0.65)),
                    )
                    .child(div().pt(px(6.0)).child(track_list))
                    .when(!alb.other_versions.is_empty(), |p| {
                        let gap = 14.0_f32;
                        let cols = ((content_w / 146.0).floor() as usize).clamp(2, 6);
                        let card_w = ((content_w - gap * (cols.saturating_sub(1) as f32))
                            / cols as f32)
                            .clamp(132.0, 172.0);
                        let mut grid = div().flex().flex_row().flex_wrap().gap(px(gap));
                        for (idx, v) in alb.other_versions.iter().take(6).enumerate() {
                            grid = grid.child(self.render_card_tile(
                                &format!("alb-ov-{idx}"),
                                v,
                                card_w,
                                cx,
                            ));
                        }
                        p.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(section_header(
                                    "alb-ov",
                                    sonata_runtime::i18n::strings(self.settings.language)
                                        .other_versions,
                                    None,
                                    None,
                                    &self.palette,
                                ))
                                .child(grid),
                        )
                    }),
            );

        let _ = HERO_ASPECT;
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("album-page-scroll")
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

    pub fn render_playlist_page(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let heading = self.snapshot.playlist_heading.clone();
        let subtitle = self.snapshot.playlist_subtitle.clone();
        let cover_url = self.snapshot.playlist_cover_url.clone();
        let tracks = self.snapshot.playlist_tracks.clone();
        let editable = self.snapshot.playlist_editable;
        let is_liked = self.snapshot.playlist_id.as_deref() == Some("VLLM");
        let loading = self.snapshot.playlist_loading;
        let started = self.loading_started.get("playlist").copied();
        let scroll_fades = self.settings.scroll_fades;
        let adaptive = self.settings.adaptive_art_color;

        let cover = self.media_for_url(cover_url.as_deref(), cx);
        let title_pt = fit_title_size(&heading);
        let tracks_play = tracks.clone();
        let tracks_shuf = tracks.clone();

        let cover_box = if is_liked && cover.is_none() {
            div()
                .flex_shrink_0()
                .size(px(184.0))
                .rounded(px(self.palette.radii.art(184.0)))
                .bg(self.palette.accent.opacity(0.22))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Heart)
                        .size(px(56.0))
                        .text_color(self.palette.accent),
                )
                .into_any_element()
        } else {
            cover_art_tile(
                "pl-hero-cov",
                cover.as_ref(),
                184.0,
                false,
                &self.palette,
                false,
                None,
            )
        };

        let hero = div()
            .flex()
            .flex_row()
            .items_end()
            .flex_wrap()
            .gap(px(22.0))
            .child(cover_box)
            .child(
                div()
                    .flex_1()
                    .min_w(px(240.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.secondary)
                            .child(
                                sonata_runtime::i18n::strings(self.settings.language)
                                    .playlist
                                    .to_uppercase(),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(title_pt))
                            .line_height(px(title_pt + 6.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child(if heading.is_empty() {
                                "Playlist".to_string()
                            } else {
                                heading
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(self.palette.secondary)
                            .child({
                                let sub_lower = subtitle.to_ascii_lowercase();
                                if subtitle.is_empty() {
                                    format!("{} tracks", tracks.len())
                                } else if sub_lower.contains("track") || sub_lower.contains("song")
                                {
                                    subtitle
                                } else {
                                    format!("{subtitle} • {} tracks", tracks.len())
                                }
                            }),
                    )
                    .child(
                        div()
                            .pt(px(6.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(
                                Button::new("pl-play-all")
                                    .primary()
                                    .icon(IconName::Play)
                                    .label(
                                        sonata_runtime::i18n::strings(self.settings.language).play,
                                    )
                                    .when(tracks_play.is_empty(), |b| b.opacity(0.45))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        if !tracks_play.is_empty() {
                                            this.backend.send(BackendCmd::PlayTracks {
                                                tracks: tracks_play.clone(),
                                                start_index: 0,
                                                shuffle: false,
                                            });
                                        }
                                    })),
                            )
                            .child(
                                Button::new("pl-shuf")
                                    .secondary()
                                    .icon(IconName::Shuffle)
                                    .label(
                                        sonata_runtime::i18n::strings(self.settings.language)
                                            .shuffle,
                                    )
                                    .when(tracks_shuf.is_empty(), |b| b.opacity(0.45))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        if !tracks_shuf.is_empty() {
                                            this.backend.send(BackendCmd::PlayTracks {
                                                tracks: tracks_shuf.clone(),
                                                start_index: 0,
                                                shuffle: true,
                                            });
                                        }
                                    })),
                            )
                            .when(editable && !is_liked, |row| {
                                let pid_edit =
                                    self.snapshot.playlist_id.clone().unwrap_or_default();
                                let cur_title = self.snapshot.playlist_heading.clone();
                                row.child(
                                    Button::new("pl-edit")
                                        .secondary()
                                        .icon(IconName::Pencil)
                                        .tooltip("Edit playlist title & cover")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if !pid_edit.is_empty() {
                                                this.modal = DesktopModal::EditPlaylist {
                                                    playlist_id: pid_edit.clone(),
                                                    cover_path: None,
                                                };
                                                this.playlist_name_input.update(cx, |s, cx| {
                                                    s.set_value(cur_title.clone(), window, cx);
                                                    s.focus(window, cx);
                                                });
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when(!editable && !is_liked, |row| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                let pid = self.snapshot.playlist_id.clone().unwrap_or_default();
                                let in_lib = !pid.is_empty() && self.is_playlist_in_library(&pid);
                                let save_item = crate::settings::PinnedCollection {
                                    id: pid.clone(),
                                    title: self.snapshot.playlist_heading.clone(),
                                    subtitle: self.snapshot.playlist_subtitle.clone(),
                                    thumbnail_url: self.snapshot.playlist_cover_url.clone(),
                                    is_album: false,
                                };
                                row.child(
                                    Button::new("pl-save-lib")
                                        .secondary()
                                        .icon(if in_lib {
                                            IconName::Check
                                        } else {
                                            IconName::Plus
                                        })
                                        .label(if in_lib {
                                            s.in_library
                                        } else {
                                            s.add_to_library
                                        })
                                        .tooltip(if in_lib {
                                            s.remove_from_library
                                        } else {
                                            s.add_to_library
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !save_item.id.is_empty() {
                                                this.toggle_save_external_playlist(
                                                    save_item.clone(),
                                                    cx,
                                                );
                                            }
                                        })),
                                )
                            })
                            .when(self.snapshot.playlist_id.is_some(), |row| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                let pid = self.snapshot.playlist_id.clone().unwrap_or_default();
                                let is_pinned = self.settings.is_pinned(&pid);
                                let pin_item = crate::settings::PinnedCollection {
                                    id: pid,
                                    title: self.snapshot.playlist_heading.clone(),
                                    subtitle: self.snapshot.playlist_subtitle.clone(),
                                    thumbnail_url: self.snapshot.playlist_cover_url.clone(),
                                    is_album: false,
                                };
                                row.child(
                                    Button::new("pl-pin")
                                        .secondary()
                                        .icon(if is_pinned {
                                            IconName::Check
                                        } else {
                                            IconName::ArrowUp
                                        })
                                        .label(if is_pinned { s.pinned } else { s.pin })
                                        .tooltip(if is_pinned { s.unpin } else { s.pin })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_pin_collection(pin_item.clone(), cx);
                                        })),
                                )
                            })
                            .when(self.snapshot.playlist_id.as_deref() != Some("SONATA:DOWNLOADS") && !tracks.is_empty(), |row| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                let pid = self.snapshot.playlist_id.clone().unwrap_or_default();
                                let ptitle = self.snapshot.playlist_heading.clone();
                                let pcover = self.snapshot.playlist_cover_url.clone();
                                let tracks_dl = tracks.clone();
                                let all_dl = tracks_dl
                                    .iter()
                                    .all(|t| self.snapshot.is_track_downloaded(&t.id));
                                let is_col_dl =
                                    all_dl || self.snapshot.is_collection_downloaded(&pid);
                                row.child(
                                    Button::new("pl-dl")
                                        .secondary()
                                        .icon(if is_col_dl {
                                            IconName::Trash
                                        } else {
                                            IconName::Download
                                        })
                                        .label(if is_col_dl {
                                            s.remove_download
                                        } else {
                                            s.download
                                        })
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            if pid.is_empty() {
                                                return;
                                            }
                                            if is_col_dl {
                                                this.backend.send(
                                                    BackendCmd::RemoveDownloadedCollection(
                                                        pid.clone(),
                                                    ),
                                                );
                                            } else {
                                                this.backend.send(BackendCmd::DownloadCollection {
                                                    collection_id: pid.clone(),
                                                    title: ptitle.clone(),
                                                    source:
                                                        sonata_core::providers::ProviderSource::YouTubeMusic,
                                                    artwork_url: pcover.clone(),
                                                    tracks: tracks_dl.clone(),
                                                });
                                            }
                                        })),
                                )
                            })
                            .when(self.snapshot.playlist_id.as_deref() == Some("SONATA:DOWNLOADS") && !tracks.is_empty(), |row| {
                                let s = sonata_runtime::i18n::strings(self.settings.language);
                                row.child(
                                    Button::new("pl-dl-clear")
                                        .secondary()
                                        .icon(IconName::Trash)
                                        .label(s.clear_downloads)
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.backend.send(BackendCmd::ClearAllDownloads);
                                        })),
                                )
                            })
                            .child(
                                Button::new("pl-refresh")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .tooltip(
                                        sonata_runtime::i18n::strings(self.settings.language)
                                            .refresh,
                                    )
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::RefreshActivePlaylist);
                                    })),
                            ),
                    ),
            );

        let mut rows = div().flex().flex_col().gap(px(2.0));
        if loading && tracks.is_empty() {
            rows = rows.child(delayed_skeleton_block(
                "pl-sk-tr",
                started,
                None,
                220.0,
                self.palette.radii.control,
                &self.palette,
            ));
        } else if tracks.is_empty() {
            rows = rows.child(
                div()
                    .py(px(28.0))
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child(sonata_runtime::i18n::strings(self.settings.language).playlist_empty),
            );
        } else {
            // Virtualize large playlists so 200+ / 1000+ track playlists only build visible + overscan rows.
            let tracks_arc: std::sync::Arc<[sonata_core::providers::Track]> =
                std::sync::Arc::from(tracks.as_slice());
            let total = tracks_arc.len();
            const VIRT_THRESHOLD: usize = 64;
            const ROW_STRIDE: f32 = 56.0; // ROW_HEIGHT (54) + gap (2)
            const HERO_OFFSET: f32 = 286.0;
            const OVERSCAN: usize = 16;

            let (win_start, win_end) = if total > VIRT_THRESHOLD {
                let scroll_y = (-f32::from(self.main_scroll.offset().y)).max(0.0);
                let vp_h: f32 = f32::from(self.main_scroll.bounds().size.height).max(900.0);
                let rel_top = (scroll_y - HERO_OFFSET).max(0.0);
                let first_vis = (rel_top / ROW_STRIDE).floor() as usize;
                let vis_count = (vp_h / ROW_STRIDE).ceil() as usize + 1;
                let start = first_vis.saturating_sub(OVERSCAN).min(total);
                let end = (first_vis + vis_count + OVERSCAN).min(total);
                (start, end)
            } else {
                (0, total)
            };

            if win_start > 0 {
                rows = rows.child(div().flex_shrink_0().h(px(win_start as f32 * ROW_STRIDE)));
            }
            for i in win_start..win_end {
                rows = rows.child(self.render_track_row(
                    "pl-tr",
                    i,
                    &tracks_arc,
                    TrackRowOpts {
                        show_index: false,
                        show_album: true,
                        playlist_remove_idx: editable.then_some(i),
                        ..Default::default()
                    },
                    cx,
                ));
            }
            if win_end < total {
                rows = rows.child(
                    div()
                        .flex_shrink_0()
                        .h(px((total - win_end) as f32 * ROW_STRIDE)),
                );
            }
        }

        let content = div()
            .relative()
            .flex()
            .flex_col()
            .child(header_wash(cover.as_ref(), &self.palette, adaptive, 260.0))
            .child(
                div()
                    .relative()
                    .px(px(PAGE_INSET_X))
                    .pt(px(28.0))
                    .pb(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(div().pb(px(12.0)).child(hero))
                    .child(
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(self.palette.outline.opacity(0.65)),
                    )
                    .child(div().pt(px(6.0)).child(rows)),
            );

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("playlist-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(content),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }
}

#[allow(dead_code)]
fn _keep_modal(_: DesktopModal) {}
