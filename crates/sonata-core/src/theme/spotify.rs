//! Spotify Desktop dark theme definition.
//!
//! 1. **Spotify Dark**: `#000000` outer shell, `#121212` panel surfaces, and `#1ED760` green accents.

use super::{RgbColor, Theme};

impl Theme {
    pub fn spotify_desktop() -> Self {
        Self {
            name: "Spotify".into(),
            transparent_bg: Some(false),
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x00, 0x00, 0x00),
            surface: RgbColor::Rgb(0x12, 0x12, 0x12),
            fg: RgbColor::Rgb(0xff, 0xff, 0xff),
            muted: RgbColor::Rgb(0xb3, 0xb3, 0xb3),
            accent: RgbColor::Rgb(0x1e, 0xd7, 0x60),
            border: RgbColor::Rgb(0x28, 0x28, 0x28),
            highlight_bg: RgbColor::Rgb(0x2a, 0x2a, 0x2a),
            highlight_fg: RgbColor::Rgb(0xff, 0xff, 0xff),
            lyrics_active: RgbColor::Rgb(0x1e, 0xd7, 0x60),
            lyrics_inactive: RgbColor::Rgb(0x72, 0x72, 0x72),
            success: RgbColor::Rgb(0x1e, 0xd7, 0x60),
            warning: RgbColor::Rgb(0xf9, 0xab, 0x00),
            error: RgbColor::Rgb(0xf1, 0x5e, 0x6c),
        }
    }
}
