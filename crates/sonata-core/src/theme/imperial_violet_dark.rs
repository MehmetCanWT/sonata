//! Imperial Violet Dark desktop theme definition.
//!
//! 1. **Palette**: warm theatrical aubergine surfaces (`#120A12` Velvet Night, `#1C101E` Aubergine, `#2B1930` Imperial Plum Surface, `#452A49` Orchid Smoke) with `#A67BB0` Violet Accent, `#C5A8C9` Antique Lilac, `#C7A86B` Antique Gold glint, and `#F6EFF6` Pearl Text.

use super::{RgbColor, Theme};

impl Theme {
    pub fn imperial_violet_dark() -> Self {
        Self {
            name: "Imperial Violet Dark".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x12, 0x0a, 0x12),
            surface: RgbColor::Rgb(0x1c, 0x10, 0x1e),
            fg: RgbColor::Rgb(0xf6, 0xef, 0xf6),
            muted: RgbColor::Rgb(0xc5, 0xa8, 0xc9),
            accent: RgbColor::Rgb(0xa6, 0x7b, 0xb0),
            border: RgbColor::Rgb(0x45, 0x2a, 0x49),
            highlight_bg: RgbColor::Rgb(0x2b, 0x19, 0x30),
            highlight_fg: RgbColor::Rgb(0xf6, 0xef, 0xf6),
            lyrics_active: RgbColor::Rgb(0xf6, 0xef, 0xf6),
            lyrics_inactive: RgbColor::Rgb(0xa3, 0x86, 0xa8),
            success: RgbColor::Rgb(0x6b, 0xb8, 0x93),
            warning: RgbColor::Rgb(0xc7, 0xa8, 0x6b),
            error: RgbColor::Rgb(0xd6, 0x6a, 0x82),
        }
    }
}
