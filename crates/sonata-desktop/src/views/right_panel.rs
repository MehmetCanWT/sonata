//! Stacked floating right side panels for `Next up` (queue) and live `Lyrics`.
//!
//! 1. **Stacked Cards**: renders one or both independent floating cards stacked vertically over an `8 pt` gutter whenever toggled on.
//! 2. **Queue Card**: displays active track highlight, track-level reordering/removal, queue shuffle, clear, and right-click menu.
//! 3. **Lyrics Card**: displays synced/plain/instrumental lyrics with click-to-seek, wheel-browse `3s` auto-scroll pause, and scroll-edge fades.

use super::{ContextMenuKind, ContextMenuState, SonataDesktopView};
use crate::backend::BackendCmd;
use crate::settings::GUTTER;
use crate::ui::components::{
    artist_links_row, card_panel, cover_art_tile, glowing_lyric_row, is_parenthetical_bg,
    resolve_active_lyric_pair, scroll_edge_fades, LyricRowStyle,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
};
use sonata_core::lyrics::LyricsState;
use sonata_core::providers::format_duration;
use std::time::Instant;

impl SonataDesktopView {
    pub fn render_right_panel(
        &mut self,
        width_px: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let show_q = self.settings.show_queue_panel;
        let show_l = self.settings.show_lyrics_panel;
        if !show_q && !show_l {
            return div().into_any_element();
        }

        div()
            .w(px(width_px))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(GUTTER))
            .when(show_q, |col| col.child(self.render_queue_card(show_l, cx)))
            .when(show_l, |col| col.child(self.render_lyrics_card(cx)))
            .into_any_element()
    }

    fn render_queue_card(&mut self, stacked: bool, cx: &mut Context<Self>) -> AnyElement {
        let queue = self.snapshot.queue.clone();
        let current_idx = self.snapshot.current_index;
        let scroll_fades = self.settings.scroll_fades;

        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px(px(8.0))
            .pb(px(12.0));
        if queue.is_empty() {
            list = list.child(
                div()
                    .px(px(12.0))
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child("Queue is empty"),
            );
        } else {
            for (idx, track) in queue.iter().enumerate() {
                let is_cur = current_idx == Some(idx);
                let cover = self.media_for_url(track.thumbnail_url.as_deref(), cx);
                let track_menu = track.clone();
                let artist_str = track.artist.clone();
                let len = queue.len();

                list = list.child(
                    div()
                        .id(SharedString::from(format!("rp-q-{idx}")))
                        .group(SharedString::from(format!("rp-q-grp-{idx}")))
                        .h(px(52.0))
                        .px(px(8.0))
                        .rounded(px(self.palette.radii.item))
                        .cursor_pointer()
                        .bg(if is_cur {
                            self.palette.surface_active
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .hover(|s| {
                            if is_cur {
                                s
                            } else {
                                s.bg(self.palette.surface_hover)
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::PlayQueueIndex(idx));
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.context_menu = Some(ContextMenuState {
                                    pos: gpui_kit::point(
                                        ev.position.x.into(),
                                        ev.position.y.into(),
                                    ),
                                    kind: ContextMenuKind::QueueItem {
                                        index: idx,
                                        track: track_menu.clone(),
                                    },
                                });
                                cx.notify();
                            }),
                        )
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(cover_art_tile(
                            SharedString::from(format!("rp-q-art-{idx}")),
                            cover.as_ref(),
                            38.0,
                            false,
                            &self.palette,
                            is_cur && !self.snapshot.paused,
                            Some(std::rc::Rc::new({
                                let b = self.backend.clone();
                                move |_, _| {
                                    if is_cur {
                                        b.send(BackendCmd::TogglePause);
                                    } else {
                                        b.send(BackendCmd::PlayQueueIndex(idx));
                                    }
                                }
                            })),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(if is_cur {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::MEDIUM
                                        })
                                        .text_color(if is_cur {
                                            self.palette.accent
                                        } else {
                                            self.palette.text
                                        })
                                        .truncate()
                                        .child(track.title.clone()),
                                )
                                .child(div().h(px(16.0)).overflow_hidden().child(
                                    artist_links_row(
                                        &format!("rp-q-a-{idx}"),
                                        &artist_str,
                                        self.palette.secondary,
                                        11.0,
                                        {
                                            let view = cx.entity().downgrade();
                                            move |name, _, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.open_artist_by_name(&name, cx);
                                                });
                                            }
                                        },
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .opacity(0.0)
                                        .group_hover(
                                            SharedString::from(format!("rp-q-grp-{idx}")),
                                            |s| s.opacity(1.0),
                                        )
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .when(idx > 0, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-up-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ArrowUp)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend.send(BackendCmd::MoveQueueItem {
                                                        from: idx,
                                                        to: idx - 1,
                                                    });
                                                })),
                                            )
                                        })
                                        .when(idx + 1 < len, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-dn-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ArrowDown)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend.send(BackendCmd::MoveQueueItem {
                                                        from: idx,
                                                        to: idx + 1,
                                                    });
                                                })),
                                            )
                                        })
                                        .when(!is_cur, |btns| {
                                            btns.child(
                                                Button::new(SharedString::from(format!(
                                                    "q-rm-{idx}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::X)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.backend
                                                        .send(BackendCmd::RemoveFromQueue(idx));
                                                })),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(self.palette.dim)
                                        .child(format_duration(track.duration_secs)),
                                ),
                        ),
                );
            }
        }

        let card = card_panel(&self.palette)
            .flex_1()
            .min_h(px(if stacked { 180.0 } else { 220.0 }))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(46.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(self.palette.outline)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                Icon::new(IconName::ListMusic)
                                    .size(px(15.0))
                                    .text_color(self.palette.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .child("Next up"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                Button::new("rp-q-shuf")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Shuffle)
                                    .tooltip("Shuffle upcoming tracks")
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ShuffleQueue);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-clear")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Trash)
                                    .tooltip("Clear upcoming")
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.backend.send(BackendCmd::ClearQueue);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-expand")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Maximize2)
                                    .tooltip("Open full Queue view")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate_to(crate::backend::NavTarget::Queue, cx);
                                    })),
                            )
                            .child(
                                Button::new("rp-q-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.show_queue_panel = false;
                                        this.settings.save();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("rp-queue-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.queue_scroll)
                            .pt(px(6.0))
                            .child(list),
                    )
                    .child(scroll_edge_fades(
                        &self.queue_scroll,
                        &self.palette,
                        scroll_fades,
                    )),
            );

        card.into_any_element()
    }

    fn render_lyrics_card(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lyrics = self.snapshot.lyrics.clone();
        let scroll_fades = self.settings.scroll_fades;

        let body: AnyElement = match lyrics {
            LyricsState::Idle => div()
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child("Play a track to display lyrics.")
                .into_any_element(),
            LyricsState::Loading => div()
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child("Loading lyrics...")
                .into_any_element(),
            LyricsState::NotFound => div()
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(self.palette.dim)
                .child("No lyrics available for this track.")
                .into_any_element(),
            LyricsState::Plain(lines) => div()
                .p(px(16.0))
                .text_size(px(14.0))
                .line_height(px(22.0))
                .text_color(self.palette.secondary)
                .child(lines.join("\n"))
                .into_any_element(),
            LyricsState::Synced(lines) => {
                let pos = self.snapshot.position_secs;
                let (active_idx, active_bg_idx) = resolve_active_lyric_pair(&lines, pos);
                let mut col = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .px(px(14.0))
                    .py(px(14.0));
                for (i, line) in lines.into_iter().enumerate() {
                    let glow = self.lyric_glow_factor(i, active_idx, active_bg_idx);
                    let is_past = active_idx.is_some_and(|a| i < a);
                    let is_bg = is_parenthetical_bg(&line.text);
                    let seek_secs = line.time_secs;
                    col = col.child(
                        glowing_lyric_row(
                            SharedString::from(format!("rp-lyr-{i}")),
                            &line.text,
                            line.voice,
                            glow,
                            is_past,
                            is_bg,
                            LyricRowStyle {
                                base_font_size: 14.0,
                                active_font_size: 16.0,
                                base_line_height: 21.0,
                                active_line_height: 23.0,
                                pad_x: 8.0,
                                pad_y: 4.0,
                                radius: self.palette.radii.small,
                                hover_bg: self.palette.surface_hover,
                                pure_white_glow: false,
                            },
                            &self.palette,
                        )
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.backend.send(BackendCmd::SeekAbsolute(seek_secs));
                        })),
                    );
                }
                col.into_any_element()
            }
        };

        card_panel(&self.palette)
            .flex_1()
            .min_h(px(180.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(46.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(self.palette.outline)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                Icon::new(IconName::MicVocal)
                                    .size(px(15.0))
                                    .text_color(self.palette.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.palette.text)
                                    .child("Lyrics"),
                            ),
                    )
                    .child(
                        Button::new("rp-lyr-close")
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.show_lyrics_panel = false;
                                this.settings.save();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .on_scroll(cx.listener(|this, _, _, _| {
                        this.last_wheel_lyrics = Some(Instant::now());
                    }))
                    .child(
                        div()
                            .id("rp-lyrics-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.lyrics_scroll)
                            .child(body),
                    )
                    .child(scroll_edge_fades(
                        &self.lyrics_scroll,
                        &self.palette,
                        scroll_fades,
                    )),
            )
            .into_any_element()
    }
}
