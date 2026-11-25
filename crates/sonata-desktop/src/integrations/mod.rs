//! Linux desktop shell integrations for MPRIS2 media controls and system tray presence.
//!
//! 1. **MPRIS2 (`mpris`)**: Exposes playback metadata, transport controls, and position updates over D-Bus.
//! 2. **System Tray (`tray`)**: Registers the StatusNotifierItem icon and quick-action menu.

pub mod mpris;
pub mod tray;
