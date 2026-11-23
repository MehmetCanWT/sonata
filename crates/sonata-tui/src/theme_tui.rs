//! Ratatui color adapter translating `crate::theme::Theme` palettes into TUI styles.
//!
//! 1. **Color Conversion**: Maps `RgbColor` variants (`Rgb`, `Reset`, `Ansi`) to `ratatui::style::Color`.
//! 2. **Palette Fields**: Exposes background, foreground, accent, highlight, semantic, and lyrics colors.

use crate::theme::{AnsiColor, RgbColor, Theme};
use ratatui::style::Color;

pub const fn to_ratatui_color(color: RgbColor) -> Color {
    match color {
        RgbColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        RgbColor::Reset => Color::Reset,
        RgbColor::Ansi(ansi) => match ansi {
            AnsiColor::Black => Color::Black,
            AnsiColor::Red => Color::Red,
            AnsiColor::Green => Color::Green,
            AnsiColor::Yellow => Color::Yellow,
            AnsiColor::Blue => Color::Blue,
            AnsiColor::Magenta => Color::Magenta,
            AnsiColor::Cyan => Color::Cyan,
            AnsiColor::Gray => Color::Gray,
            AnsiColor::DarkGray => Color::DarkGray,
            AnsiColor::White => Color::White,
        },
    }
}

#[derive(Debug, Clone)]
pub struct TuiTheme {
    pub name: String,
    pub bg: Color,
    pub surface: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub border: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub lyrics_active: Color,
    pub lyrics_inactive: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl TuiTheme {
    pub fn from_core(theme: &Theme) -> Self {
        Self {
            name: theme.name.clone(),
            bg: to_ratatui_color(theme.bg),
            surface: to_ratatui_color(theme.surface),
            fg: to_ratatui_color(theme.fg),
            muted: to_ratatui_color(theme.muted),
            accent: to_ratatui_color(theme.accent),
            border: to_ratatui_color(theme.border),
            highlight_bg: to_ratatui_color(theme.highlight_bg),
            highlight_fg: to_ratatui_color(theme.highlight_fg),
            lyrics_active: to_ratatui_color(theme.lyrics_active),
            lyrics_inactive: to_ratatui_color(theme.lyrics_inactive),
            success: to_ratatui_color(theme.success),
            warning: to_ratatui_color(theme.warning),
            error: to_ratatui_color(theme.error),
        }
    }
}
