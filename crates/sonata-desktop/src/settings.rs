//! Persistent desktop UI, layout, and glass-opacity preferences isolated from shared `AppConfig`.
//!
//! 1. **Appearance**: `ThemeModePref`, `CornerStyle`, active dark/light theme names, transparency `opacity`, `blur`, `card_outline`, `compositor_corners`, `scroll_fades`, and `adaptive_art_color`.
//! 2. **Layout Metrics**: shared shell constants (`GUTTER`, `TITLEBAR_H`, `PLAYER_BAR_H`, `SIDEBAR_*`, `UPNEXT_*`, `PAGE_MIN_W`) and responsive panel shedding.
//! 3. **Opacity Solver**: piecewise-linear `gutter` and `card` alpha mapping plus `card_alpha` / `glass_alpha` / `sheer` compensation.
//! 4. **Persistence**: atomic read/write under `~/.config/sonata/desktop-ui.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const GUTTER: f32 = 8.0;
pub const CARD_INSET_X: f32 = 8.0;
pub const PAGE_INSET_X: f32 = 20.0;
pub const TITLEBAR_H: f32 = 56.0;
pub const PLAYER_BAR_H: f32 = 88.0;

pub const SIDEBAR_DEFAULT: f32 = 280.0;
pub const SIDEBAR_MIN: f32 = 200.0;
pub const SIDEBAR_MAX: f32 = 400.0;
pub const SIDEBAR_RAIL: f32 = 72.0;
pub const SIDEBAR_SNAP_AT: f32 = 100.0;

pub const PANEL_DEFAULT: f32 = 288.0;
pub const PANEL_MIN: f32 = 240.0;
pub const PANEL_MAX: f32 = 480.0;

pub const PAGE_MIN_W: f32 = 400.0;
pub const WINDOW_MIN_W: f32 = 496.0;
pub const WINDOW_MIN_H: f32 = 480.0;
pub const WINDOW_DEFAULT_W: f32 = 1240.0;
pub const WINDOW_DEFAULT_H: f32 = 800.0;

pub const ROW_HEIGHT: f32 = 56.0;
pub const ART_SIZE: f32 = 40.0;
pub const CARD_W: f32 = 132.0;
pub const CARD_STRETCH_MAX: f32 = 1.3;
pub const HERO_ASPECT: f32 = 0.36;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeModePref {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CornerStyle {
    #[default]
    Round,
    Soft,
    Sharp,
}

impl CornerStyle {
    pub fn radii(self) -> CornerRadii {
        match self {
            Self::Round => CornerRadii {
                panel: 20.0,
                popup: 12.0,
                control: 8.0,
                item: 6.0,
                small: 4.0,
                art_min: 8.0,
                art_max: 16.0,
            },
            Self::Soft => CornerRadii {
                panel: 10.0,
                popup: 8.0,
                control: 6.0,
                item: 4.0,
                small: 3.0,
                art_min: 4.0,
                art_max: 8.0,
            },
            Self::Sharp => CornerRadii {
                panel: 4.0,
                popup: 4.0,
                control: 2.0,
                item: 2.0,
                small: 2.0,
                art_min: 2.0,
                art_max: 2.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    pub panel: f32,
    pub popup: f32,
    pub control: f32,
    pub item: f32,
    pub small: f32,
    pub art_min: f32,
    pub art_max: f32,
}

impl CornerRadii {
    /// Scales artwork rounding smoothly with thumbnail size, bounded by `art_min..=art_max`.
    pub fn art(&self, size: f32) -> f32 {
        (size * 0.08).clamp(self.art_min, self.art_max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolvedAlpha {
    /// Requested compositor opacity behind the root gutter surface.
    pub gutter_alpha: f32,
    /// Visual target opacity of floating cards before compositing over gutter.
    pub target_card_alpha: f32,
    /// Solved alpha for card paint: `card = gutter + card_alpha * (1 - gutter)`.
    pub card_alpha: f32,
    /// Strength of translucency in `0.0..=1.0`, driving contrast compensation.
    pub sheer: f32,
    /// Multiplicative alpha for secondary glass controls (`chips`, `inputs`, `buttons`).
    pub glass_alpha: f32,
}

/// Computes solved surface and glass alphas from `opacity` (`0.0` = opaque, `1.0` = standard glass, `1.25` = ultra sheer).
pub fn solve_alpha(opacity: f32, blur_enabled: bool) -> SolvedAlpha {
    let t = opacity.clamp(0.0, 1.25);
    if t <= 0.001 {
        return SolvedAlpha {
            gutter_alpha: 1.0,
            target_card_alpha: 1.0,
            card_alpha: 1.0,
            sheer: 0.0,
            glass_alpha: 1.0,
        };
    }

    const GUTTER_KNOTS: [(f32, f32); 5] = [
        (0.0, 1.00),
        (1.0 / 3.0, 0.88),
        (2.0 / 3.0, 0.74),
        (1.0, 0.58),
        (1.25, 0.30),
    ];
    const CARD_KNOTS: [(f32, f32); 5] = [
        (0.0, 1.00),
        (1.0 / 3.0, 0.94),
        (2.0 / 3.0, 0.86),
        (1.0, 0.74),
        (1.25, 0.50),
    ];

    let mut gutter = piecewise_linear(&GUTTER_KNOTS, t);
    let mut card = piecewise_linear(&CARD_KNOTS, t);

    // Without compositor blur behind the window, lift floor alpha slightly so text stays legible.
    if !blur_enabled {
        gutter = gutter.max(0.76);
        card = card.max(0.86);
    }

    let card_alpha = if gutter >= 0.999 {
        1.0
    } else {
        ((card - gutter) / (1.0 - gutter)).clamp(0.0, 1.0)
    };
    let sheer = ((1.0 - gutter) / 0.55).clamp(0.0, 1.0);
    let glass_alpha = (1.0 - 0.60 * sheer).clamp(0.30, 1.0);

    SolvedAlpha {
        gutter_alpha: gutter,
        target_card_alpha: card,
        card_alpha,
        sheer,
        glass_alpha,
    }
}

fn piecewise_linear(knots: &[(f32, f32)], x: f32) -> f32 {
    for win in knots.windows(2) {
        let (x0, y0) = win[0];
        let (x1, y1) = win[1];
        if x <= x1 {
            let f = if (x1 - x0).abs() < f32::EPSILON {
                0.0
            } else {
                (x - x0) / (x1 - x0)
            };
            return y0 + (y1 - y0) * f;
        }
    }
    knots.last().map(|k| k.1).unwrap_or(1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    pub theme_mode: ThemeModePref,
    pub dark_theme: String,
    pub light_theme: String,
    /// Transparency depth in `0.0..=1.25` (UI exposes `0..=100` where `100` maps cleanly to `1.0`).
    pub opacity: f32,
    pub blur: bool,
    pub card_outline: bool,
    pub corners: CornerStyle,
    pub compositor_corners: bool,
    pub scroll_fades: bool,
    pub adaptive_art_color: bool,
    pub sidebar_width: f32,
    pub panel_width: f32,
    pub show_queue_panel: bool,
    pub show_lyrics_panel: bool,
    pub double_click_to_play: bool,
    pub sidebar_playlist_order: Vec<String>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeModePref::System,
            dark_theme: default_dark_theme_name().to_string(),
            light_theme: default_light_theme_name().to_string(),
            opacity: 0.78,
            blur: true,
            card_outline: true,
            corners: CornerStyle::Round,
            compositor_corners: true,
            scroll_fades: true,
            adaptive_art_color: true,
            sidebar_width: SIDEBAR_DEFAULT,
            panel_width: PANEL_DEFAULT,
            show_queue_panel: false,
            show_lyrics_panel: false,
            double_click_to_play: false,
            sidebar_playlist_order: Vec::new(),
        }
    }
}

pub fn default_dark_theme_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS Dark"
    } else if cfg!(target_os = "windows") {
        "Windows Dark"
    } else {
        "Classic Dark"
    }
}

pub fn default_light_theme_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS Light"
    } else if cfg!(target_os = "windows") {
        "Windows Light"
    } else {
        "Classic Light"
    }
}

impl DesktopSettings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sonata")
            .join("desktop-ui.json")
    }

    pub fn themes_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sonata")
            .join("themes")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }

    /// Snaps sidebar width below `SIDEBAR_SNAP_AT` directly onto the 72 pt icon rail, or clamps to full drawer range.
    pub fn effective_sidebar_width(&self) -> f32 {
        if self.sidebar_width < SIDEBAR_SNAP_AT {
            SIDEBAR_RAIL
        } else {
            self.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
        }
    }

    /// Priority-shedding layout calculation protecting `PAGE_MIN_W = 400.0` at all window widths.
    pub fn resolve_shell_widths(&self, window_w: f32) -> (f32, f32) {
        let desired_panel = if self.show_queue_panel || self.show_lyrics_panel {
            self.panel_width.clamp(PANEL_MIN, PANEL_MAX)
        } else {
            0.0
        };
        let desired_sidebar = self.effective_sidebar_width();

        let gaps = if desired_panel > 0.0 {
            GUTTER * 4.0
        } else {
            GUTTER * 3.0
        };
        let available = (window_w - gaps - PAGE_MIN_W).max(0.0);

        if desired_panel > 0.0 {
            // Shrink right panel first down to PANEL_MIN, then shed right panel completely before collapsing sidebar.
            if available >= desired_sidebar + desired_panel {
                return (desired_sidebar, desired_panel);
            }
            if available >= desired_sidebar + PANEL_MIN {
                return (desired_sidebar, available - desired_sidebar);
            }
            // Priority order: hide right panel cleanly when space is constrained below sidebar + min panel,
            // or collapse sidebar to icon rail when narrower still.
            if available >= SIDEBAR_RAIL + PANEL_MIN {
                return (SIDEBAR_RAIL, (available - SIDEBAR_RAIL).min(desired_panel));
            }
            if available >= desired_sidebar {
                return (desired_sidebar, 0.0);
            }
            if available >= SIDEBAR_MIN {
                return (available, 0.0);
            }
            return (SIDEBAR_RAIL, 0.0);
        }

        if available >= desired_sidebar {
            (desired_sidebar, 0.0)
        } else if available >= SIDEBAR_MIN {
            (available, 0.0)
        } else {
            (SIDEBAR_RAIL, 0.0)
        }
    }

    pub fn is_transparent(&self) -> bool {
        self.opacity > 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solve_alpha_opaque_at_zero() {
        let s = solve_alpha(0.0, true);
        assert_eq!(s.gutter_alpha, 1.0);
        assert_eq!(s.card_alpha, 1.0);
        assert_eq!(s.sheer, 0.0);
        assert_eq!(s.glass_alpha, 1.0);
    }

    #[test]
    fn solve_alpha_canonical_knots() {
        let s = solve_alpha(1.0, true);
        assert!((s.gutter_alpha - 0.58).abs() < 1e-4);
        assert!((s.target_card_alpha - 0.74).abs() < 1e-4);
        let expected_card_alpha = (0.74 - 0.58) / (1.0 - 0.58);
        assert!((s.card_alpha - expected_card_alpha).abs() < 1e-4);
    }

    #[test]
    fn shell_width_shedding_preserves_center_min_width() {
        let settings = DesktopSettings {
            show_queue_panel: true,
            ..Default::default()
        };
        let (sb, rp) = settings.resolve_shell_widths(560.0);
        let gaps = GUTTER * if rp > 0.0 { 4.0 } else { 3.0 };
        assert!(560.0 - sb - rp - gaps >= PAGE_MIN_W - 0.01);
        assert_eq!(rp, 0.0);
    }

    #[test]
    fn sidebar_snaps_below_100_to_rail() {
        let rail = DesktopSettings {
            sidebar_width: 90.0,
            ..Default::default()
        };
        assert_eq!(rail.effective_sidebar_width(), SIDEBAR_RAIL);

        let full = DesktopSettings {
            sidebar_width: 150.0,
            ..Default::default()
        };
        assert_eq!(full.effective_sidebar_width(), SIDEBAR_MIN);
    }
}
