//! Shader-like terminal animations powered by `tachyonfx` for Sonata TUI.
//!
//! 1. **Effect Manager (`UiEffects`)**: tracks active unique effects keyed by UI region (`Startup`, `TrackList`, `LyricsLine`, `PlayerBar`, `Modal`) and advances them using clamped frame deltas.
//! 2. **Automatic State Transitions (`sync_and_render`)**: detects playlist switches, active lyric line advances, track changes, and modal openings automatically without touching Kitty/Sixel artwork regions.

use crate::app::ActiveModal;
use crate::theme_tui::TuiTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::time::Instant;
use tachyonfx::{fx, CellFilter, Duration as FxDuration, EffectManager, Interpolation, Motion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub enum EffectKey {
    #[default]
    Startup,
    TrackList,
    LyricsLine,
    PlayerBar,
    Modal,
}

/// Snapshot of UI regions and state signals used to trigger and clip `tachyonfx` transitions.
pub struct FxFrameContext<'a> {
    pub screen_area: Rect,
    pub tracks_area: Rect,
    pub lyrics_area: Rect,
    pub player_bar_area: Rect,
    pub tracks_heading: &'a str,
    pub track_id: Option<&'a str>,
    pub active_lyric_idx: Option<usize>,
    pub modal_open: bool,
    pub theme: &'a TuiTheme,
}

pub struct UiEffects {
    manager: EffectManager<EffectKey>,
    last_frame: Instant,
    started: bool,
    last_heading: String,
    last_track_id: Option<String>,
    last_lyric_idx: Option<usize>,
    last_modal_open: bool,
}

impl Default for UiEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl UiEffects {
    pub fn new() -> Self {
        Self {
            manager: EffectManager::default(),
            last_frame: Instant::now(),
            started: false,
            last_heading: String::new(),
            last_track_id: None,
            last_lyric_idx: None,
            last_modal_open: false,
        }
    }

    /// Returns true while any `tachyonfx` transition is actively running.
    pub fn is_running(&self) -> bool {
        self.manager.is_running()
    }

    /// Detects UI state changes, queues corresponding region-scoped effects, and renders them into `buf`.
    pub fn sync_and_render(&mut self, ctx: &FxFrameContext<'_>, buf: &mut Buffer) {
        let now = Instant::now();
        let elapsed_ms = now
            .saturating_duration_since(self.last_frame)
            .as_millis()
            .min(100) as u32;
        self.last_frame = now;

        self.sync_triggers(ctx);
        self.process_with_elapsed(
            FxDuration::from_millis(elapsed_ms.max(1)),
            ctx.screen_area,
            buf,
        );
    }

    fn sync_triggers(&mut self, ctx: &FxFrameContext<'_>) {
        if !self.started {
            self.started = true;
            self.last_heading = ctx.tracks_heading.to_string();
            self.last_track_id = ctx.track_id.map(str::to_string);
            self.last_lyric_idx = ctx.active_lyric_idx;
            self.last_modal_open = ctx.modal_open;
            if ctx.screen_area.width > 0 && ctx.screen_area.height > 0 {
                let fx = fx::sweep_in(
                    Motion::LeftToRight,
                    14,
                    0,
                    ctx.theme.bg,
                    (300, Interpolation::ExpoOut),
                )
                .with_area(ctx.screen_area);
                self.manager.add_unique_effect(EffectKey::Startup, fx);
            }
            return;
        }

        if ctx.tracks_heading != self.last_heading {
            self.last_heading = ctx.tracks_heading.to_string();
            if ctx.tracks_area.width > 0 && ctx.tracks_area.height > 0 {
                let fx = fx::fade_from_fg(ctx.theme.bg, (220, Interpolation::CubicOut))
                    .with_filter(CellFilter::Text)
                    .with_area(ctx.tracks_area);
                self.manager.add_unique_effect(EffectKey::TrackList, fx);
            }
        }

        let track_changed = self.last_track_id.as_deref() != ctx.track_id;
        if track_changed {
            self.last_track_id = ctx.track_id.map(str::to_string);
            if ctx.track_id.is_some()
                && ctx.player_bar_area.width > 0
                && ctx.player_bar_area.height > 0
            {
                let fx = fx::fade_from_fg(ctx.theme.accent, (260, Interpolation::CubicOut))
                    .with_filter(CellFilter::Text)
                    .with_area(ctx.player_bar_area);
                self.manager.add_unique_effect(EffectKey::PlayerBar, fx);
            }
        }

        if ctx.active_lyric_idx != self.last_lyric_idx {
            self.last_lyric_idx = ctx.active_lyric_idx;
            if ctx.active_lyric_idx.is_some()
                && ctx.lyrics_area.width > 0
                && ctx.lyrics_area.height > 0
            {
                // Upward luminous glide shader synchronized with the anticipatory scroll:
                // a soft bell-curved light ribbon sweeps bottom-to-top across text cells with
                // zero start/end pop and zero border/background disturbance.
                let target_rgb = color_to_rgb(ctx.theme.lyrics_active);
                let fx = fx::effect_fn((), (280, Interpolation::SineOut), move |_, sctx, cells| {
                    let alpha = sctx.alpha();
                    let env = (std::f32::consts::PI * alpha).sin().max(0.0);
                    if env <= 0.004 || sctx.area.height == 0 {
                        return;
                    }
                    let top = sctx.area.y as f32;
                    let height = sctx.area.height as f32;
                    let wave_y = top + height * (1.0 - alpha);
                    let spread = (height * 0.24).clamp(1.6, 3.4);
                    let inv_2sig2 = 1.0 / (2.0 * spread * spread);
                    cells.for_each_cell(|pos, cell| {
                        let dy = pos.y as f32 - wave_y;
                        let band = (-(dy * dy) * inv_2sig2).exp();
                        let mix = (band * env * 0.28).clamp(0.0, 0.30);
                        if mix > 0.01 {
                            let (r, g, b) = color_to_rgb(cell.fg);
                            let lerp = |a: u8, b: u8| {
                                ((a as f32) + ((b as f32) - (a as f32)) * mix).round() as u8
                            };
                            cell.set_fg(Color::Rgb(
                                lerp(r, target_rgb.0),
                                lerp(g, target_rgb.1),
                                lerp(b, target_rgb.2),
                            ));
                        }
                    });
                })
                .with_filter(CellFilter::Text)
                .with_area(ctx.lyrics_area);
                self.manager.add_unique_effect(EffectKey::LyricsLine, fx);
            }
        }

        if ctx.modal_open
            && !self.last_modal_open
            && ctx.screen_area.width > 0
            && ctx.screen_area.height > 0
        {
            let fx = fx::coalesce((180, Interpolation::CubicOut)).with_area(ctx.screen_area);
            self.manager.add_unique_effect(EffectKey::Modal, fx);
        }
        self.last_modal_open = ctx.modal_open;
    }

    fn process_with_elapsed(&mut self, elapsed: FxDuration, screen_area: Rect, buf: &mut Buffer) {
        if self.manager.is_running() {
            self.manager.process_effects(elapsed, buf, screen_area);
        }
    }
}

pub fn is_modal_open(modal: &ActiveModal) -> bool {
    !matches!(modal, ActiveModal::None)
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::White => (245, 245, 250),
        Color::Gray => (168, 168, 178),
        Color::DarkGray => (110, 110, 122),
        Color::Cyan => (110, 225, 250),
        Color::LightCyan => (170, 240, 255),
        Color::Magenta => (220, 130, 255),
        Color::Yellow => (250, 225, 110),
        Color::Green => (130, 235, 145),
        Color::Blue => (120, 175, 255),
        Color::Red => (245, 115, 125),
        _ => (210, 210, 220),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn sample_theme() -> TuiTheme {
        TuiTheme::from_core(&Theme::default())
    }

    #[test]
    fn startup_and_state_transitions_enqueue_and_finish_effects() {
        let theme = sample_theme();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut effects = UiEffects::new();

        let ctx = FxFrameContext {
            screen_area: area,
            tracks_area: Rect::new(20, 0, 40, 19),
            lyrics_area: Rect::new(0, 10, 20, 9),
            player_bar_area: Rect::new(0, 19, 80, 5),
            tracks_heading: "Liked Music",
            track_id: None,
            active_lyric_idx: None,
            modal_open: false,
            theme: &theme,
        };

        effects.sync_triggers(&ctx);
        assert!(effects.is_running());

        effects.process_with_elapsed(FxDuration::from_millis(400), area, &mut buf);
        assert!(!effects.is_running());

        let ctx_next = FxFrameContext {
            tracks_heading: "Favorites",
            track_id: Some("trk-1"),
            active_lyric_idx: Some(3),
            modal_open: true,
            ..ctx
        };
        effects.sync_triggers(&ctx_next);
        assert!(effects.is_running());

        effects.process_with_elapsed(FxDuration::from_millis(400), area, &mut buf);
        assert!(!effects.is_running());
    }
}
