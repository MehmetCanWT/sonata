//! Cross-platform desktop shell integrations for OS media controls and system tray presence.
//!
//! 1. **Media Controls (`mpris`)**: Exposes MPRIS2 on Linux and OS media keys (`souvlaki`) on macOS/Windows.
//! 2. **System Tray (`tray`)**: Coordinates state and menu actions for Tauri's cross-platform tray icon.

pub mod mpris;
pub mod tray;
