//! UI state types, events, and appearance/layout helpers for the Sonata terminal UI.
//!
//! 1. **Layout & Focus**: `SplitTarget`, `UiRects`, `FocusPane`, and `SidebarEntry` model pane geometry, focus targets, and sidebar rows.
//! 2. **Modals & Events**: `ActiveModal`, `AppEvent`, and `ExportFingerprint` represent modal dialogs, background task messages, and export deduplication.
//! 3. **Appearance & Navigation**: `App` helpers manage sidebar construction, theme/transparency cycling, Hyprland opacity, pane resizing, and config flushing.

use super::App;
use crate::theme_tui::TuiTheme;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_image::protocol::StatefulProtocol;
use sonata_core::auth::ExtractedCookies;
use sonata_core::hyprland;
use sonata_core::lyrics::LyricsState;
use sonata_core::player::MpvEvent;
use sonata_core::providers::hifi::ResolvedStream;
use sonata_core::providers::{Playlist, Track};
use sonata_core::updater::UpdateCheckResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTarget {
    LeftCol,
    RightCol,
    ArtLyrics,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UiRects {
    pub search_bar: Rect,
    pub tracks_rows: Rect,
    pub sidebar_inner: Rect,
    pub album_art_box: Rect,
    pub main_area: Rect,
    pub left_col_area: Rect,
    pub split_left_x: u16,
    pub split_right_x: u16,
    pub split_art_y: u16,
    pub player_bar: Rect,
    pub player_top: Rect,
    pub progress_bar: Rect,
    pub volume_bar: Rect,
    pub ctrl_prev: Rect,
    pub ctrl_play: Rect,
    pub ctrl_next: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Tracks,
    SearchInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEntry {
    Search,
    Queue,
    Playlist(Playlist),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    ImageWarning(usize),
    ThemePicker(usize, String),
    SpotifyImport(String),
    AddToPlaylist(usize),
    CreatePlaylist(String),
    Settings(usize),
    Help,
}

pub enum AppEvent {
    AuthReady {
        cookies: ExtractedCookies,
        ytm_playlists: Vec<Playlist>,
        spotify_playlists: Vec<Playlist>,
        imported_playlists: Vec<Playlist>,
    },
    TracksLoaded {
        req_id: u64,
        playlist_id: Option<String>,
        heading: String,
        tracks: Vec<Track>,
        silent_replace: bool,
    },
    TracksAppended {
        req_id: u64,
        heading: String,
        tracks: Vec<Track>,
    },
    StreamReady {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamUpgrade {
        play_seq: u64,
        track_id: String,
        stream: ResolvedStream,
    },
    StreamError {
        play_seq: u64,
        track_id: String,
        error: String,
    },
    NextTrackPrepared {
        play_seq: u64,
        index: usize,
        track_id: String,
        result: Result<ResolvedStream, String>,
    },
    LyricsReady {
        play_seq: u64,
        track_id: String,
        lyrics: LyricsState,
    },
    ArtworkReady {
        play_seq: u64,
        track_id: String,
        url: String,
        image: DynamicImage,
        png_bytes: Vec<u8>,
    },
    ArtworkEnhanced {
        play_seq: u64,
        track_id: String,
        url: String,
        image: DynamicImage,
        png_bytes: Vec<u8>,
    },
    AnimatedArtworkReady {
        play_seq: u64,
        track_id: String,
        fps: f32,
        frames: Vec<(Option<StatefulProtocol>, Vec<u8>)>,
    },
    Mpv(MpvEvent),
    PlaylistCreated(Playlist),
    UpdateChecked(UpdateCheckResult),
    StatusMessage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFingerprint {
    pub(super) track_id: String,
    pub(super) status: String,
    pub(super) lyric: Option<String>,
    pub(super) art_path: Option<String>,
    pub(super) pos_secs: u64,
    pub(super) dur_secs: u64,
}

impl App {
    pub fn rebuild_sidebar(&mut self) {
        let mut items = vec![SidebarEntry::Search, SidebarEntry::Queue];
        if self.config.provider_mode.uses_ytm() {
            for p in &self.ytm_playlists {
                items.push(SidebarEntry::Playlist(p.clone()));
            }
        }
        if self.config.provider_mode.uses_spotify() {
            for p in &self.spotify_playlists {
                items.push(SidebarEntry::Playlist(p.clone()));
            }
            for p in &self.imported_playlists {
                if !items.iter().any(|existing| match existing {
                    SidebarEntry::Playlist(ep) => ep.id == p.id,
                    _ => false,
                }) {
                    items.push(SidebarEntry::Playlist(p.clone()));
                }
            }
        }
        self.sidebar_items = items;
        if self.sidebar_idx >= self.sidebar_items.len() {
            self.sidebar_idx = 0;
        }
    }

    pub fn toggle_mini_mode(&mut self) {
        self.mini_mode = !self.mini_mode;
        self.clear_kitty_images();
        self.status_line = if self.mini_mode {
            "Mini Mode (double-click Album Art or press m to restore)".to_string()
        } else {
            "Full Mode".to_string()
        };
    }

    pub fn effective_transparent_bg(&self) -> bool {
        self.base_theme
            .transparent_bg
            .unwrap_or(self.config.transparent_bg)
    }

    pub fn effective_hyprland_opacity(&self) -> f32 {
        self.base_theme
            .hyprland_opacity
            .unwrap_or(self.config.hyprland_opacity)
            .clamp(0.50, 1.00)
    }

    fn ensure_hyprland_window(&mut self) -> Option<String> {
        if hyprland::is_hyprland() {
            if let Some(fresh) = hyprland::find_terminal_window_address() {
                self.hyprland_window = Some(fresh);
            }
        }
        self.hyprland_window.clone()
    }

    pub fn sync_appearance(&mut self) {
        let transparent = self.effective_transparent_bg();
        self.theme = TuiTheme::from_core(&self.base_theme.with_transparency(transparent));
        if let Color::Rgb(r, g, b) = self.theme.bg {
            self.picker.set_background_color([r, g, b, 255]);
        }
        if let Some(addr) = self.ensure_hyprland_window() {
            let opacity = if transparent {
                self.effective_hyprland_opacity()
            } else {
                1.0
            };
            tokio::spawn(async move {
                hyprland::set_window_opacity(&addr, opacity).await;
            });
        }
    }

    pub fn restore_hyprland_opacity(&mut self) {
        if let Some(addr) = self.ensure_hyprland_window() {
            hyprland::set_window_opacity_sync(&addr, 1.0);
        }
    }

    pub fn toggle_jiosaavn(&mut self) {
        self.config.jiosaavn_enabled = !self.config.jiosaavn_enabled;
        sonata_core::config::AppConfig::set_jiosaavn_enabled(self.config.jiosaavn_enabled);
        let _ = self.config.save();
        if self.config.jiosaavn_enabled {
            self.start_hifi_upgrade();
        }
        self.status_line = format!(
            "JioSaavn 320kbps Module: {}",
            if self.config.jiosaavn_enabled {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    pub fn toggle_motion_covers(&mut self) {
        self.config.motion_covers = !self.config.motion_covers;
        sonata_core::config::AppConfig::set_motion_covers_enabled(self.config.motion_covers);
        let _ = self.config.save();
        if !self.config.motion_covers {
            self.anim_art_frames.clear();
            self.clear_kitty_images();
        }
        self.status_line = format!(
            "Animated Album Artwork: {}",
            if self.config.motion_covers {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    pub fn toggle_transparent_bg(&mut self) {
        self.config.transparent_bg = !self.config.transparent_bg;
        self.base_theme.transparent_bg = None;
        let _ = self.config.save();
        self.sync_appearance();
        self.status_line = format!(
            "Transparent TUI Background: {}",
            if self.config.transparent_bg {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    pub fn cycle_hyprland_opacity(&mut self, forward: bool) {
        if !hyprland::is_hyprland() {
            self.status_line =
                "Hyprland Opacity is locked to Solid (100%) outside Hyprland".to_string();
            return;
        }
        const STEPS: &[f32] = &[0.50, 0.60, 0.70, 0.80, 0.90, 1.00];
        let cur = self.config.hyprland_opacity;
        let idx = STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((*a - cur).abs())
                    .partial_cmp(&((*b - cur).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(4);
        let next = if forward {
            (idx + 1) % STEPS.len()
        } else {
            (idx + STEPS.len() - 1) % STEPS.len()
        };
        self.config.hyprland_opacity = STEPS[next];
        self.base_theme.hyprland_opacity = None;
        let _ = self.config.save();
        let _ = self.ensure_hyprland_window();
        self.sync_appearance();
        self.status_line = format!(
            "Hyprland Window Opacity: {:.0}%",
            self.config.hyprland_opacity * 100.0
        );
    }

    pub fn cycle_theme(&mut self, forward: bool) {
        if self.themes.is_empty() {
            return;
        }
        let cur = self
            .themes
            .iter()
            .position(|t| t.name == self.base_theme.name)
            .unwrap_or(0);
        let next = if forward {
            (cur + 1) % self.themes.len()
        } else {
            (cur + self.themes.len() - 1) % self.themes.len()
        };
        self.apply_theme_by_index(next);
        self.config.theme = self.base_theme.name.clone();
        let _ = self.config.save();
        self.status_line = format!("Theme: {}", self.base_theme.name);
    }

    pub fn apply_theme_by_index(&mut self, idx: usize) {
        if let Some(t) = self.themes.get(idx).cloned() {
            self.base_theme = t;
            self.sync_appearance();
        }
    }

    pub fn adjust_focused_pane_width(&mut self, delta_pct: i16) {
        match self.focus {
            FocusPane::Sidebar => {
                let next = (self.config.right_pane_pct as i16 + delta_pct).clamp(14, 48) as u16;
                if 100u16.saturating_sub(self.config.left_pane_pct + next) >= 20 {
                    self.config.right_pane_pct = next;
                    let _ = self.config.save();
                }
            }
            FocusPane::Tracks | FocusPane::SearchInput => {
                let next = (self.config.left_pane_pct as i16 + delta_pct).clamp(18, 52) as u16;
                if 100u16.saturating_sub(next + self.config.right_pane_pct) >= 20 {
                    self.config.left_pane_pct = next;
                    let _ = self.config.save();
                }
            }
        }
    }

    pub fn flush_config(&mut self) {
        if self.config_dirty {
            self.config_dirty = false;
            let _ = self.config.save();
        }
    }

    pub(super) fn selected_or_playing_track(&self) -> Option<Track> {
        if self.focus == FocusPane::Tracks {
            if let Some(idx) = self.track_table_state.selected() {
                if let Some(t) = self.visible_tracks.get(idx) {
                    return Some(t.clone());
                }
            }
        }
        self.playback.current_track().cloned()
    }
}
