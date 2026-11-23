//! YouTube Music Desktop dark theme definition.
//!
//! 1. **YouTube Music Dark**: `#030303` canvas, `#0F0F0F` surfaces, and `#FF0033` accents.

use super::{RgbColor, Theme};

impl Theme {
    pub fn youtube_music_desktop() -> Self {
        Self {
            name: "YouTube Music".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x03, 0x03, 0x03),
            surface: RgbColor::Rgb(0x0f, 0x0f, 0x0f),
            fg: RgbColor::Rgb(0xff, 0xff, 0xff),
            muted: RgbColor::Rgb(0xaa, 0xaa, 0xaa),
            accent: RgbColor::Rgb(0xff, 0x00, 0x33),
            border: RgbColor::Rgb(0x27, 0x27, 0x27),
            highlight_bg: RgbColor::Rgb(0x21, 0x21, 0x21),
            highlight_fg: RgbColor::Rgb(0xff, 0xff, 0xff),
            lyrics_active: RgbColor::Rgb(0xff, 0xff, 0xff),
            lyrics_inactive: RgbColor::Rgb(0x71, 0x71, 0x71),
            success: RgbColor::Rgb(0x3d, 0xdc, 0x84),
            warning: RgbColor::Rgb(0xf9, 0xab, 0x00),
            error: RgbColor::Rgb(0xff, 0x4e, 0x45),
        }
    }
}
