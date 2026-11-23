//! Imperial Violet desktop light theme definition.
//!
//! 1. **Palette**: warm pearl-neutral surfaces (`#F6F3F8` base, `#E7E0EC` lavender haze, `#C7B7D1` orchid ash dividers) with `#826A91` controlled violet accents and `#382B3D` imperial plum ink.

use super::{RgbColor, Theme};

impl Theme {
    pub fn imperial_violet() -> Self {
        Self {
            name: "Imperial Violet".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0xf6, 0xf3, 0xf8),
            surface: RgbColor::Rgb(0xe7, 0xe0, 0xec),
            fg: RgbColor::Rgb(0x38, 0x2b, 0x3d),
            muted: RgbColor::Rgb(0x6e, 0x5f, 0x76),
            accent: RgbColor::Rgb(0x82, 0x6a, 0x91),
            border: RgbColor::Rgb(0xc7, 0xb7, 0xd1),
            highlight_bg: RgbColor::Rgb(0xdf, 0xd5, 0xe8),
            highlight_fg: RgbColor::Rgb(0x38, 0x2b, 0x3d),
            lyrics_active: RgbColor::Rgb(0x38, 0x2b, 0x3d),
            lyrics_inactive: RgbColor::Rgb(0x6e, 0x5f, 0x76),
            success: RgbColor::Rgb(0x4d, 0x92, 0x73),
            warning: RgbColor::Rgb(0xb4, 0x7e, 0x38),
            error: RgbColor::Rgb(0xb6, 0x47, 0x62),
        }
    }
}
