//! Fullscreen Now Playing stage with flowing ambient wave backdrop and word-synced lyrics.
//!
//! 1. **Ambient Wave Field (`ambient`)**: generates a cached, dithered 5-color flowing wave backdrop
//!    from the active cover's dominant palette with corner vignette and readability luma ceiling.
//! 2. **Balanced 50/50 Cover Stage & Controls (`controls`)**: centers a rounded square cover (or
//!    motion video), title/artist metadata, transport dock, playback options, queue drawer, and corner pill.
//! 3. **Pinned Depth-of-Field Lyrics (`lyrics`)**: anchors the active verse at 30% viewport height
//!    with spring scrolling, karaoke sweeps, instrumental breaks, dual-voice alignment, and edge dissolves.

mod ambient;
mod controls;
mod lyrics;

use super::{FullscreenDrawerTab, SonataDesktopView};
use crate::ui::components::cover_art_tile;
use ambient::{
    ambient_palette_settled, blend_ambient_palette, build_ambient_wave_image,
    resolve_ambient_palette, AMBIENT_WASH_SECS,
};
use gpui_kit::{
    div, img, point, prelude::FluentBuilder as _, px, AnyElement, BoxShadow, ClickEvent, Context,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
};
use std::time::Instant;

const COVER_TALL_REST: f32 = 0.56;
const COVER_WIDE_REST: f32 = 0.40;
const COVER_TALL_AWAKE: f32 = 0.46;
const COVER_WIDE_AWAKE: f32 = 0.34;
const COVER_MIN: f32 = 160.0;
const COVER_MAX_REST: f32 = 560.0;
const COVER_MAX_AWAKE: f32 = 520.0;

impl SonataDesktopView {
    pub fn render_fullscreen_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vp = window.viewport_size();
        let vp_w: f32 = vp.width.into();
        let vp_h: f32 = vp.height.into();
        let stacked = vp_w < 820.0;
        let current = self.snapshot.current_track().cloned();
        let motion_tex = self.current_motion_image();
        let art_media = self
            .media_for_path(self.snapshot.art_path.as_deref(), cx)
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|t| self.media_for_url(t.thumbnail_url.as_deref(), cx))
            });

        let (dr, dg, db) = art_media
            .as_ref()
            .map(|m| m.dominant_rgb)
            .unwrap_or(self.snapshot.ambient_rgb);
        let dom_rgb = [dr, dg, db];
        let cover_palette = art_media.as_ref().map(|m| m.cover_palette);
        let target_ambient = resolve_ambient_palette(cover_palette, dom_rgb);
        let animates = !self.snapshot.config.reduce_motion;

        let target_changed = self.fullscreen_ambient_target != Some(target_ambient);
        self.fullscreen_ambient_target = Some(target_ambient);

        let now = Instant::now();
        let step_secs = now
            .duration_since(self.fullscreen_ambient_stepped)
            .as_secs_f32();
        let painted = match self.fullscreen_ambient_painted {
            Some(prev) if animates && step_secs < 1.5 => {
                let delta = 1.0 - (-step_secs / AMBIENT_WASH_SECS).exp();
                blend_ambient_palette(prev, target_ambient, delta)
            }
            _ => target_ambient,
        };
        self.fullscreen_ambient_stepped = now;
        self.fullscreen_ambient_washing =
            animates && !ambient_palette_settled(painted, target_ambient);
        self.fullscreen_ambient_painted = Some(painted);

        // Render immediately on first fullscreen frame or when static palette changes.
        if self.fullscreen_ambient_tex.is_none() || (!animates && target_changed) {
            let elapsed = if animates {
                self.fullscreen_ambient_started.elapsed().as_secs_f32()
            } else {
                0.0
            };
            if let Some(tex) = build_ambient_wave_image(painted.0, painted.1, elapsed) {
                if let Some(old) = self.fullscreen_ambient_tex.replace(tex) {
                    cx.drop_image(old, None);
                }
            }
        }
        if animates && (self.fullscreen_ambient_washing || !self.snapshot.paused) {
            self.ensure_ambient_drift_timer(cx);
        }
        let ambient_tex = self.fullscreen_ambient_tex.clone();

        let controls_visible = self.fullscreen_awake
            || self.progress_dragging
            || self.fullscreen_tab == FullscreenDrawerTab::Queue;
        let show_side_panel = self.fullscreen_tab != FullscreenDrawerTab::Cover;

        let art_size = if stacked {
            (vp_h * 0.34).clamp(160.0, 260.0)
        } else if controls_visible {
            (vp_h * COVER_TALL_AWAKE)
                .min(vp_w * COVER_WIDE_AWAKE)
                .clamp(COVER_MIN, COVER_MAX_AWAKE)
        } else {
            (vp_h * COVER_TALL_REST)
                .min(vp_w * COVER_WIDE_REST)
                .clamp(COVER_MIN, COVER_MAX_REST)
        };
        let art_radius = 16.0_f32;

        let cover_shadow = vec![BoxShadow {
            color: gpui_kit::black().opacity(0.48),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(42.0),
            spread_radius: px(0.0),
            inset: false,
        }];

        let cover_box = if let Some(tex) = motion_tex {
            div()
                .flex_shrink_0()
                .size(px(art_size))
                .rounded(px(art_radius))
                .overflow_hidden()
                .shadow(cover_shadow)
                .child(img(tex).size_full().object_fit(ObjectFit::Cover))
                .into_any_element()
        } else if let Some(ref media) = art_media {
            div()
                .flex_shrink_0()
                .size(px(art_size))
                .rounded(px(art_radius))
                .overflow_hidden()
                .shadow(cover_shadow)
                .child(
                    img(media.image.clone())
                        .size_full()
                        .rounded(px(art_radius))
                        .object_fit(ObjectFit::Cover),
                )
                .into_any_element()
        } else {
            div()
                .flex_shrink_0()
                .rounded(px(art_radius))
                .shadow(cover_shadow)
                .child(cover_art_tile(
                    "fs-art",
                    None,
                    art_size,
                    false,
                    &self.palette,
                    false,
                    None,
                ))
                .into_any_element()
        };

        let track_meta_and_controls = self.render_fullscreen_track_meta_and_controls(
            current.as_ref(),
            art_size,
            controls_visible,
            cx,
        );

        let left_column = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(20.0))
            .child(cover_box)
            .child(track_meta_and_controls);

        let right_panel: Option<AnyElement> = match self.fullscreen_tab {
            FullscreenDrawerTab::Cover => None,
            FullscreenDrawerTab::Queue => Some(self.render_fullscreen_queue(cx)),
            FullscreenDrawerTab::Lyrics => {
                Some(self.render_fullscreen_lyrics(vp_w, vp_h, window, cx))
            }
        };

        let corner_pill = controls_visible.then(|| self.render_fullscreen_corner_pill(cx));

        div()
            .id("fullscreen-overlay")
            .absolute()
            .inset_0()
            .bg(gpui_kit::rgb(0x0c0a0e))
            .on_click(cx.listener(|_, ev: &ClickEvent, window, _| {
                if ev.click_count() == 2 {
                    window.toggle_fullscreen();
                }
            }))
            .when_some(ambient_tex, |root, tex| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(img(tex).size_full().object_fit(ObjectFit::Fill)),
                )
            })
            .child(
                div()
                    .relative()
                    .size_full()
                    .px(px(if stacked { 24.0 } else { 32.0 }))
                    .flex()
                    .when(!stacked, |el| el.flex_row().gap(px(32.0)))
                    .when(stacked, |el| el.flex_col().gap(px(16.0)).py(px(20.0)))
                    .items_center()
                    .child(left_column)
                    .when_some(right_panel, |row, panel| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .h_full()
                                .flex()
                                .when(!show_side_panel, |d| d.hidden())
                                .child(panel),
                        )
                    }),
            )
            .children(corner_pill)
            .into_any_element()
    }
}
