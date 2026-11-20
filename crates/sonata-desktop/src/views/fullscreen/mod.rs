//! Immersive fullscreen playback workspace built on stock GPUI.
//!
//! 1. **State & Scroll Lifecycle (`mod.rs`)**: `FullscreenState` tracks the ambient wave renderer, alpha-invariant blur cache, smooth scroll spring state, and `1.2s` manual-scroll timer generation.
//! 2. **Artwork & Controls (`controls`)**: `render_cover_column` pairs large rounded artwork and metadata with `render_left_controls` directly on the ambient backdrop without a card box.
//! 3. **Synced Lyrics Sheet (`sheet`)**: `render_fullscreen_lyrics_column` centers the active karaoke verse at 50% viewport height, pre-warms the 3 Gaussian blur buckets, and styles counter/background vocals.

mod controls;
mod sheet;

use super::fullscreen_fx::{AmbientWavesState, BlurredLyricCache};
use super::{render_icon_button, SonataDesktopView};
use crate::theme_bridge::{rgb_hsla, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, img, px, relative, AnyElement, ClickEvent, Context, InteractiveElement, IntoElement,
    ObjectFit, ParentElement, ScrollHandle, StatefulInteractiveElement, Styled, StyledImage,
    Window,
};
use std::time::{Duration, Instant};

pub(super) const MANUAL_SCROLL_SETTLE: Duration = Duration::from_millis(1200);
pub(super) const AUTO_SCROLL_ANCHOR_RATIO: f32 = 0.50;
pub(super) const AUTO_SCROLL_SPEED: f32 = 14.0;
pub(super) const AUTO_SCROLL_SNAP_PX: f32 = 0.6;

/// Persistent UI state for the immersive fullscreen workspace.
pub struct FullscreenState {
    pub scroll: ScrollHandle,
    pub waves: AmbientWavesState,
    pub blur_cache: BlurredLyricCache,
    pub hovered_line: Option<usize>,
    pub manual_scroll_until: Option<Instant>,
    pub manual_scroll_gen: u64,
    pub last_active_line: Option<usize>,
    pub last_scroll_step: Instant,
    pub last_track_id: Option<String>,
    pub snap_next_scroll: bool,
}

impl Default for FullscreenState {
    fn default() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            waves: AmbientWavesState::default(),
            blur_cache: BlurredLyricCache::default(),
            hovered_line: None,
            manual_scroll_until: None,
            manual_scroll_gen: 0,
            last_active_line: None,
            last_scroll_step: Instant::now(),
            last_track_id: None,
            snap_next_scroll: true,
        }
    }
}

impl FullscreenState {
    pub fn on_enter(&mut self) {
        self.manual_scroll_until = None;
        self.manual_scroll_gen = self.manual_scroll_gen.wrapping_add(1);
        self.last_active_line = None;
        self.hovered_line = None;
        self.snap_next_scroll = true;
        self.last_scroll_step = Instant::now();
    }

    pub fn note_manual_scroll(&mut self) -> u64 {
        self.manual_scroll_until = Some(Instant::now() + MANUAL_SCROLL_SETTLE);
        self.manual_scroll_gen = self.manual_scroll_gen.wrapping_add(1);
        self.manual_scroll_gen
    }

    pub(super) fn is_manually_scrolled(&self) -> bool {
        self.manual_scroll_until
            .is_some_and(|until| Instant::now() < until)
    }
}

/// Renders the full-window immersive fullscreen playback workspace.
pub fn render_fullscreen_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let viewport = window.viewport_size();
    let width: f32 = viewport.width.into();
    let height: f32 = viewport.height.into();
    let scale_factor = window.scale_factor();

    let current_track_id = view.snapshot.current_track().map(|t| t.id.clone());
    if view.fullscreen.last_track_id != current_track_id {
        view.fullscreen.last_track_id = current_track_id;
        view.fullscreen.blur_cache.clear(window);
        view.fullscreen.on_enter();
    }

    // Advancing to a new active line immediately cancels manual-scroll hold so lyrics re-blur & center.
    let active_lyric = view.snapshot.active_lyric_index();
    if view.fullscreen.last_active_line != active_lyric {
        view.fullscreen.last_active_line = active_lyric;
        view.fullscreen.manual_scroll_until = None;
    }

    let reduce_motion = view.snapshot.config.reduce_motion;
    let art_path = view.snapshot.art_path.clone();
    let ambient_rgb = view.snapshot.ambient_rgb;

    // 1. Seamless 5-color flowing ambient waves background (CPU-rendered wave field, see `fullscreen_fx`).
    let waves_frame = view.fullscreen.waves.frame(
        art_path.as_deref(),
        ambient_rgb,
        width,
        height,
        reduce_motion,
        window,
    );
    let accent_hsla = view.fullscreen.waves.stage_accent_hsla();

    // 2. Responsive geometry calibrated to a 1556x967 reference layout.
    let wide = width >= 820.0;
    let cover_size = if wide {
        (height * 0.50)
            .min(width * 0.31)
            .min((height - 230.0).max(160.0))
            .clamp(180.0, 480.0)
    } else {
        (height * 0.25).min(width * 0.52).clamp(130.0, 240.0)
    };
    let cover_radius = (cover_size * 0.044).clamp(14.0, 24.0);

    let frame_scale = (width / 1152.0).min(height / 648.0).clamp(0.82, 1.55);
    let title_size = (21.5 * frame_scale).clamp(20.0, 30.0);
    let artist_size = (13.0 * frame_scale).clamp(13.0, 17.0);
    let base_verse_size = (26.5 * frame_scale).clamp(22.0, 38.0);
    let active_verse_size = base_verse_size + 2.0;
    let verse_line_height = active_verse_size * 1.18;
    let verse_gap = base_verse_size * 0.60;

    // 3. Smoothly ease vertical lyrics scroll toward the 50% center anchor.
    let scroll_animating = sheet::step_smooth_lyrics_scroll(view, height, reduce_motion);
    let waves_washing = view.fullscreen.waves.is_washing();
    if (scroll_animating || waves_washing) && !reduce_motion {
        window.request_animation_frame();
    }

    let snap = view.snapshot.clone();
    let track = snap.current_track();
    let raw_title = track
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "No Active Track".to_string());
    let explicit = controls::has_explicit_marker(&raw_title);
    let display_title = if explicit {
        controls::strip_explicit_suffix(&raw_title)
    } else {
        raw_title
    };
    let artist = track
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| "Choose a song to begin".to_string());

    let controls_visible = view.fullscreen_controls_visible();
    let paused = snap.paused;
    let lyrics_col_width = if wide {
        (width * 0.53 - 68.0).clamp(320.0, 820.0)
    } else {
        (width - 48.0).clamp(260.0, 680.0)
    };

    let lyrics_panel = sheet::render_fullscreen_lyrics_column(
        view,
        &snap,
        lyrics_col_width,
        height,
        base_verse_size,
        active_verse_size,
        verse_line_height,
        verse_gap,
        accent_hsla,
        scale_factor,
        window,
        cx,
    );

    let left_column = controls::render_cover_column(
        view,
        art_path.as_ref(),
        cover_size,
        cover_radius,
        &display_title,
        &artist,
        explicit,
        title_size,
        artist_size,
        wide,
        controls_visible,
        paused,
        p,
        cx,
    );

    let stage = if wide {
        div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .child(
                div()
                    .w(relative(0.47))
                    .h_full()
                    .pl(px(36.0))
                    .pr(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(left_column),
            )
            .child(
                div()
                    .w(relative(0.53))
                    .h_full()
                    .pl(px(12.0))
                    .pr(px(56.0))
                    .flex()
                    .flex_col()
                    .child(lyrics_panel),
            )
    } else {
        div()
            .relative()
            .size_full()
            .px_6()
            .py_6()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(left_column)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(lyrics_panel),
            )
    };

    let mut root = div()
        .id("fullscreen-playback")
        .relative()
        .size_full()
        .overflow_hidden()
        .font_family("Inter")
        .bg(rgb_hsla((10, 6, 14), 1.0))
        .on_mouse_move(cx.listener(|this, _, _, cx| this.note_fullscreen_activity(cx)))
        .on_click(|ev: &ClickEvent, window, _| {
            if ev.click_count() == 2 {
                window.toggle_fullscreen();
            }
        })
        // Full-bleed edge-to-edge flowing ambient waves image.
        .child(
            img(waves_frame)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Fill),
        )
        .child(stage);

    // Subtle top-right minimize trigger appears on activity / pause without shifting layout.
    if controls_visible {
        root = root.child(div().absolute().top(px(20.0)).right(px(24.0)).child(
            render_icon_button(
                "fullscreen-close",
                IconName::Minimize2,
                16.0,
                36.0,
                rgb_hsla((255, 255, 255), 0.88),
                rgb_hsla((16, 12, 22), 0.34),
                rgb_hsla((255, 255, 255), 0.14),
                |this, _, cx| this.toggle_fullscreen_view(cx),
                cx,
            ),
        ));
    }

    root.into_any_element()
}

/// Compatibility entry point when called from a nested pane.
pub fn render_fullscreen(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    render_fullscreen_workspace(view, p, window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonata_core::lyrics::LyricLine;

    #[test]
    fn sheet_entries_insert_instrumental_break_on_long_gap() {
        let lines = vec![
            LyricLine::new(1.0, "The emptiness machine"),
            LyricLine::new(9.5, "Going around like a revolver"),
            LyricLine::new(12.0, "It's been decided how we lose"),
        ];
        let entries = sheet::build_sheet_entries(&lines, 180.0);
        assert_eq!(entries.len(), 4);
        assert!(matches!(
            entries[0],
            sheet::SheetEntry::Verse { line_index: 0, .. }
        ));
        assert!(matches!(entries[1], sheet::SheetEntry::Instrumental { .. }));
        assert!(matches!(
            entries[2],
            sheet::SheetEntry::Verse { line_index: 1, .. }
        ));
        assert!(matches!(
            entries[3],
            sheet::SheetEntry::Verse { line_index: 2, .. }
        ));
    }

    #[test]
    fn explicit_marker_detection_and_strip() {
        assert!(controls::has_explicit_marker(
            "The Emptiness Machine (Explicit)"
        ));
        assert_eq!(
            controls::strip_explicit_suffix("The Emptiness Machine [Explicit]"),
            "The Emptiness Machine"
        );
    }

    #[test]
    fn background_lyric_detection_requires_parentheses() {
        assert!(sheet::is_background_lyric("(Ooh, ooh)"));
        assert!(!sheet::is_background_lyric("Ooh, ooh"));
        assert!(!sheet::is_background_lyric("()"));
    }

    #[test]
    fn scroll_anchor_and_blur_buckets_match_spec() {
        assert!((AUTO_SCROLL_ANCHOR_RATIO - 0.50).abs() < f32::EPSILON);
        assert_eq!(MANUAL_SCROLL_SETTLE, Duration::from_millis(1200));
        let b1 = sheet::blur_sigma_for_distance(1, 30.0);
        let b2 = sheet::blur_sigma_for_distance(2, 30.0);
        let b3 = sheet::blur_sigma_for_distance(3, 30.0);
        let b6 = sheet::blur_sigma_for_distance(6, 30.0);
        assert!(b1 < b2 && b2 < b3);
        assert!((b3 - b6).abs() < f32::EPSILON);
    }
}
