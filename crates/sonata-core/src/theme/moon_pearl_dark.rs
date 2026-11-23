//! Moon Pearl Dark desktop theme definition.
//!
//! 1. **Palette**: cold blue-black lunar surfaces (`#080B10` Lunar Black, `#0E1520` Deep Tide, `#182230` Moonstone Surface, `#263446` Slate Glass) with `#8294B3` Cold Accent, `#AEBACB` Silver Mist, and `#EEF3FA` Ice Pearl ink.

use super::{RgbColor, Theme};

impl Theme {
    pub fn moon_pearl_dark() -> Self {
        Self {
            name: "Moon Pearl Dark".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x08, 0x0b, 0x10),
            surface: RgbColor::Rgb(0x0e, 0x15, 0x20),
            fg: RgbColor::Rgb(0xee, 0xf3, 0xfa),
            muted: RgbColor::Rgb(0xae, 0xba, 0xcb),
            accent: RgbColor::Rgb(0x82, 0x94, 0xb3),
            border: RgbColor::Rgb(0x26, 0x34, 0x46),
            highlight_bg: RgbColor::Rgb(0x18, 0x22, 0x30),
            highlight_fg: RgbColor::Rgb(0xee, 0xf3, 0xfa),
            lyrics_active: RgbColor::Rgb(0xee, 0xf3, 0xfa),
            lyrics_inactive: RgbColor::Rgb(0x85, 0x96, 0xb0),
            success: RgbColor::Rgb(0x63, 0xb9, 0x95),
            warning: RgbColor::Rgb(0xd6, 0xb4, 0x6c),
            error: RgbColor::Rgb(0xd6, 0x6d, 0x7c),
        }
    }
}
