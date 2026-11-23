//! Moon Pearl desktop light theme definition.
//!
//! 1. **Palette**: cool silvery moonstone surfaces (`#F7F5FA` base, `#EEEAF2` surface, `#D8D0E2` silver-lilac dividers) with `#8F7FA5` moonstone accents and `#342C3A` deep plum ink.

use super::{RgbColor, Theme};

impl Theme {
    pub fn moon_pearl() -> Self {
        Self {
            name: "Moon Pearl".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0xf7, 0xf5, 0xfa),
            surface: RgbColor::Rgb(0xee, 0xea, 0xf2),
            fg: RgbColor::Rgb(0x34, 0x2c, 0x3a),
            muted: RgbColor::Rgb(0x6d, 0x64, 0x77),
            accent: RgbColor::Rgb(0x8f, 0x7f, 0xa5),
            border: RgbColor::Rgb(0xd8, 0xd0, 0xe2),
            highlight_bg: RgbColor::Rgb(0xe5, 0xdf, 0xee),
            highlight_fg: RgbColor::Rgb(0x34, 0x2c, 0x3a),
            lyrics_active: RgbColor::Rgb(0x34, 0x2c, 0x3a),
            lyrics_inactive: RgbColor::Rgb(0x6d, 0x64, 0x77),
            success: RgbColor::Rgb(0x4b, 0x95, 0x7a),
            warning: RgbColor::Rgb(0xb8, 0x84, 0x36),
            error: RgbColor::Rgb(0xb8, 0x4b, 0x64),
        }
    }
}
