//! Built-in terminal themes tailored for `sonata` (TUI).
//!
//! 1. **Palettes**: Catppuccin Mocha, Ghostty Dark, Tokyo Night, Gruvbox Dark, Rosé Pine, and Nord.

use super::{RgbColor, Theme};

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x1e, 0x1e, 0x2e),
            surface: RgbColor::Rgb(0x18, 0x18, 0x25),
            fg: RgbColor::Rgb(0xcd, 0xd6, 0xf4),
            muted: RgbColor::Rgb(0x6c, 0x70, 0x86),
            accent: RgbColor::Rgb(0xcb, 0xa6, 0xf7),
            border: RgbColor::Rgb(0x45, 0x47, 0x5a),
            highlight_bg: RgbColor::Rgb(0x31, 0x32, 0x44),
            highlight_fg: RgbColor::Rgb(0xf5, 0xe0, 0xdc),
            lyrics_active: RgbColor::Rgb(0xf5, 0xc2, 0xe7),
            lyrics_inactive: RgbColor::Rgb(0x6c, 0x70, 0x86),
            success: RgbColor::Rgb(0xa6, 0xe3, 0xa1),
            warning: RgbColor::Rgb(0xf9, 0xe2, 0xaf),
            error: RgbColor::Rgb(0xf3, 0x8b, 0xa8),
        }
    }

    pub fn ghostty_dark() -> Self {
        Self {
            name: "Ghostty Dark".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x14, 0x16, 0x1b),
            surface: RgbColor::Rgb(0x1a, 0x1d, 0x24),
            fg: RgbColor::Rgb(0xe6, 0xed, 0xf3),
            muted: RgbColor::Rgb(0x7d, 0x85, 0x90),
            accent: RgbColor::Rgb(0x79, 0xc0, 0xff),
            border: RgbColor::Rgb(0x30, 0x36, 0x3d),
            highlight_bg: RgbColor::Rgb(0x21, 0x26, 0x2d),
            highlight_fg: RgbColor::Rgb(0xff, 0xff, 0xff),
            lyrics_active: RgbColor::Rgb(0x79, 0xc0, 0xff),
            lyrics_inactive: RgbColor::Rgb(0x6e, 0x76, 0x81),
            success: RgbColor::Rgb(0x56, 0xd3, 0x64),
            warning: RgbColor::Rgb(0xe3, 0xb3, 0x41),
            error: RgbColor::Rgb(0xf8, 0x51, 0x49),
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x1a, 0x1b, 0x26),
            surface: RgbColor::Rgb(0x16, 0x16, 0x1e),
            fg: RgbColor::Rgb(0xc0, 0xca, 0xf5),
            muted: RgbColor::Rgb(0x56, 0x5f, 0x89),
            accent: RgbColor::Rgb(0x7a, 0xa2, 0xf7),
            border: RgbColor::Rgb(0x3b, 0x42, 0x61),
            highlight_bg: RgbColor::Rgb(0x28, 0x34, 0x57),
            highlight_fg: RgbColor::Rgb(0xc0, 0xca, 0xf5),
            lyrics_active: RgbColor::Rgb(0xbb, 0x9a, 0xf7),
            lyrics_inactive: RgbColor::Rgb(0x56, 0x5f, 0x89),
            success: RgbColor::Rgb(0x9e, 0xce, 0x6a),
            warning: RgbColor::Rgb(0xe0, 0xaf, 0x68),
            error: RgbColor::Rgb(0xf7, 0x76, 0x8e),
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x28, 0x28, 0x28),
            surface: RgbColor::Rgb(0x1d, 0x20, 0x21),
            fg: RgbColor::Rgb(0xeb, 0xdb, 0xb2),
            muted: RgbColor::Rgb(0x92, 0x83, 0x74),
            accent: RgbColor::Rgb(0xfe, 0x80, 0x19),
            border: RgbColor::Rgb(0x50, 0x49, 0x45),
            highlight_bg: RgbColor::Rgb(0x3c, 0x38, 0x36),
            highlight_fg: RgbColor::Rgb(0xfb, 0xf1, 0xc7),
            lyrics_active: RgbColor::Rgb(0xfa, 0xbd, 0x2f),
            lyrics_inactive: RgbColor::Rgb(0x92, 0x83, 0x74),
            success: RgbColor::Rgb(0xb8, 0xbb, 0x26),
            warning: RgbColor::Rgb(0xfa, 0xbd, 0x2f),
            error: RgbColor::Rgb(0xfb, 0x49, 0x34),
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            name: "Rosé Pine".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x19, 0x17, 0x24),
            surface: RgbColor::Rgb(0x1f, 0x1d, 0x2e),
            fg: RgbColor::Rgb(0xe0, 0xde, 0xf4),
            muted: RgbColor::Rgb(0x6e, 0x6a, 0x86),
            accent: RgbColor::Rgb(0xeb, 0xbc, 0xba),
            border: RgbColor::Rgb(0x40, 0x3d, 0x52),
            highlight_bg: RgbColor::Rgb(0x26, 0x23, 0x3a),
            highlight_fg: RgbColor::Rgb(0xe0, 0xde, 0xf4),
            lyrics_active: RgbColor::Rgb(0xc4, 0xa7, 0xe7),
            lyrics_inactive: RgbColor::Rgb(0x6e, 0x6a, 0x86),
            success: RgbColor::Rgb(0x9c, 0xcf, 0xd8),
            warning: RgbColor::Rgb(0xf6, 0xc1, 0x77),
            error: RgbColor::Rgb(0xeb, 0x6f, 0x92),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "Nord".into(),
            transparent_bg: None,
            hyprland_opacity: None,
            bg: RgbColor::Rgb(0x2e, 0x34, 0x40),
            surface: RgbColor::Rgb(0x27, 0x2c, 0x36),
            fg: RgbColor::Rgb(0xec, 0xef, 0xf4),
            muted: RgbColor::Rgb(0x61, 0x6e, 0x88),
            accent: RgbColor::Rgb(0x88, 0xc0, 0xd0),
            border: RgbColor::Rgb(0x4c, 0x56, 0x6a),
            highlight_bg: RgbColor::Rgb(0x3b, 0x42, 0x52),
            highlight_fg: RgbColor::Rgb(0xec, 0xef, 0xf4),
            lyrics_active: RgbColor::Rgb(0x88, 0xc0, 0xd0),
            lyrics_inactive: RgbColor::Rgb(0x61, 0x6e, 0x88),
            success: RgbColor::Rgb(0xa3, 0xbe, 0x8c),
            warning: RgbColor::Rgb(0xeb, 0xcb, 0x8b),
            error: RgbColor::Rgb(0xbf, 0x61, 0x6a),
        }
    }
}
