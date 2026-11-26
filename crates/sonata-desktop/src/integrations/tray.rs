//! Cross-platform system tray integration: pure-Rust StatusNotifierItem (`ksni`) on Linux and Tauri `TrayIcon` on macOS/Windows.
//!
//! 1. **TrayState & Readiness**: mirrors playback metadata, pause state, and window visibility alongside `is_tray_active()`.
//! 2. **Linux Native Tray (`ksni`)**: rasterizes a matte obsidian circular badge (`SONATA_TRAY_ICON`) and exports D-Bus StatusNotifierItem without requiring `libayatana-appindicator3`.
//! 3. **Desktop Bridge (macOS / Windows)**: forwards `TrayHandle` updates to Tauri's `TrayIconBuilder` sink.

use crate::backend::{BackendCmd, UiWindowCmd};
#[cfg(target_os = "linux")]
use sonata_runtime::i18n::strings;
use sonata_runtime::i18n::AppLanguage;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(not(target_os = "linux"))]
use std::sync::{Arc, OnceLock, RwLock};

static TRAY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Returns `true` when an active system tray host/D-Bus item is registered.
pub fn is_tray_active() -> bool {
    TRAY_ACTIVE.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayState {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub paused: bool,
    pub has_track: bool,
    pub window_visible: bool,
    pub language: AppLanguage,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            title: None,
            artist: None,
            paused: true,
            has_track: false,
            window_visible: true,
            language: AppLanguage::En,
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_tray {
    use super::*;
    use ksni::TrayMethods;
    use std::sync::LazyLock;
    use std::time::Duration;

    pub struct TrayHandle {
        inner: ksni::Handle<SonataTray>,
    }

    impl TrayHandle {
        pub async fn update(&self, state: TrayState) {
            if self
                .inner
                .update(move |tray| {
                    tray.state = state;
                })
                .await
                .is_none()
            {
                tracing::warn!("system tray handle unavailable during state update");
            }
        }

        pub async fn shutdown(&self) {
            TRAY_ACTIVE.store(false, Ordering::Relaxed);
            let awaiter = self.inner.shutdown();
            if tokio::time::timeout(Duration::from_millis(500), awaiter)
                .await
                .is_err()
            {
                tracing::warn!("timed out waiting for system tray shutdown");
            }
        }
    }

    static SONATA_TRAY_ICON: LazyLock<Vec<ksni::Icon>> = LazyLock::new(|| {
        const SIZE: i32 = 32;
        let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        let center = 15.5_f32;

        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();

                let mut a: u8 = 0;
                let mut r: u8 = 0;
                let mut g: u8 = 0;
                let mut b: u8 = 0;

                if dist <= 14.5 {
                    let edge_alpha = (14.5 - dist).clamp(0.0, 1.0);
                    // Matte obsidian badge (#141418 -> #1C1C22) with subtle rim (#36363E).
                    let t = (x as f32 + y as f32) / 62.0;
                    let bg_r = 20.0 + t * 8.0;
                    let bg_g = 20.0 + t * 8.0;
                    let bg_b = 24.0 + t * 10.0;

                    let rim = if dist >= 12.7 { 0.45 } else { 0.0 };
                    let fr = (bg_r + (54.0 - bg_r) * rim).round() as u8;
                    let fg = (bg_g + (54.0 - bg_g) * rim).round() as u8;
                    let fb = (bg_b + (62.0 - bg_b) * rim).round() as u8;

                    let nx = x as f32;
                    let ny = y as f32;
                    let head_dist =
                        (((nx - 12.8) / 3.6).powi(2) + ((ny - 21.0) / 2.7).powi(2)).sqrt();
                    let in_head = head_dist <= 1.0;
                    let in_stem = (15.0..=17.1).contains(&nx) && (8.5..=21.0).contains(&ny);
                    let in_flag = (16.5..=22.0).contains(&nx)
                        && (ny >= 8.5 + (nx - 16.5) * 0.55)
                        && (ny <= 11.4 + (nx - 16.5) * 0.55);

                    if in_head || in_stem || in_flag {
                        r = 245;
                        g = 245;
                        b = 247;
                        a = (edge_alpha * 255.0).round() as u8;
                    } else {
                        r = fr;
                        g = fg;
                        b = fb;
                        a = (edge_alpha * 244.0).round() as u8;
                    }
                }

                // StatusNotifierItem expects network byte order ARGB32.
                data.extend_from_slice(&[a, r, g, b]);
            }
        }

        vec![ksni::Icon {
            width: SIZE,
            height: SIZE,
            data,
        }]
    });

    pub struct SonataTray {
        state: TrayState,
        cmd_tx: UnboundedSender<BackendCmd>,
        win_tx: UnboundedSender<UiWindowCmd>,
    }

    impl ksni::Tray for SonataTray {
        fn id(&self) -> String {
            "sonata-desktop".to_string()
        }

        fn category(&self) -> ksni::Category {
            ksni::Category::ApplicationStatus
        }

        fn title(&self) -> String {
            "Sonata".to_string()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            SONATA_TRAY_ICON.clone()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            let s = strings(self.state.language);
            let description = match (&self.state.title, &self.state.artist) {
                (Some(title), Some(artist)) if !artist.is_empty() => format!("{title} — {artist}"),
                (Some(title), _) => title.clone(),
                _ => s.tray_ready.to_string(),
            };
            ksni::ToolTip {
                title: "Sonata".to_string(),
                description,
                ..Default::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.win_tx.send(UiWindowCmd::ToggleWindow);
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::{MenuItem, StandardItem};

            let s = strings(self.state.language);
            let (toggle_label, toggle_cmd) = if self.state.window_visible {
                (s.tray_hide_window, UiWindowCmd::HideWindow)
            } else {
                (s.tray_show_window, UiWindowCmd::ShowWindow)
            };
            let play_label = if self.state.paused || !self.state.has_track {
                s.play
            } else {
                s.pause
            };

            vec![
                MenuItem::Standard(StandardItem {
                    label: toggle_label.to_string(),
                    activate: Box::new(move |tray: &mut Self| {
                        let _ = tray.win_tx.send(toggle_cmd);
                    }),
                    ..Default::default()
                }),
                MenuItem::Separator,
                MenuItem::Standard(StandardItem {
                    label: play_label.to_string(),
                    enabled: self.state.has_track,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.cmd_tx.send(BackendCmd::TogglePause);
                    }),
                    ..Default::default()
                }),
                MenuItem::Standard(StandardItem {
                    label: s.next_track.to_string(),
                    enabled: self.state.has_track,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.cmd_tx.send(BackendCmd::NextTrack);
                    }),
                    ..Default::default()
                }),
                MenuItem::Standard(StandardItem {
                    label: s.previous_track.to_string(),
                    enabled: self.state.has_track,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.cmd_tx.send(BackendCmd::PrevTrack);
                    }),
                    ..Default::default()
                }),
                MenuItem::Separator,
                MenuItem::Standard(StandardItem {
                    label: s.tray_quit.to_string(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.win_tx.send(UiWindowCmd::QuitApp);
                    }),
                    ..Default::default()
                }),
            ]
        }
    }

    pub async fn spawn_tray(
        cmd_tx: UnboundedSender<BackendCmd>,
        win_tx: UnboundedSender<UiWindowCmd>,
    ) -> Option<TrayHandle> {
        let tray = SonataTray {
            state: TrayState::default(),
            cmd_tx,
            win_tx,
        };
        match tray.spawn().await {
            Ok(inner) => {
                TRAY_ACTIVE.store(true, Ordering::Relaxed);
                Some(TrayHandle { inner })
            }
            Err(err) => {
                TRAY_ACTIVE.store(false, Ordering::Relaxed);
                tracing::warn!(?err, "failed to spawn Linux StatusNotifierItem system tray");
                None
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod tauri_tray {
    use super::*;

    type TraySink = Box<dyn Fn(&TrayState) + Send + Sync>;

    static TRAY_SINK: OnceLock<RwLock<Option<Arc<TraySink>>>> = OnceLock::new();

    fn sink_lock() -> &'static RwLock<Option<Arc<TraySink>>> {
        TRAY_SINK.get_or_init(|| RwLock::new(None))
    }

    /// Registers the UI-side Tauri tray updater invoked whenever backend [`TrayState`] changes.
    pub fn register_tray_sink<F>(updater: F)
    where
        F: Fn(&TrayState) + Send + Sync + 'static,
    {
        if let Ok(mut guard) = sink_lock().write() {
            *guard = Some(Arc::new(Box::new(updater)));
            TRAY_ACTIVE.store(true, Ordering::Relaxed);
        }
    }

    /// Clears the registered tray sink during shutdown.
    pub fn clear_tray_sink() {
        TRAY_ACTIVE.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = sink_lock().write() {
            *guard = None;
        }
    }

    pub struct TrayHandle {
        state: tokio::sync::Mutex<TrayState>,
    }

    impl TrayHandle {
        pub async fn update(&self, state: TrayState) {
            {
                let mut guard = self.state.lock().await;
                *guard = state.clone();
            }
            let sink = sink_lock().read().ok().and_then(|g| g.clone());
            if let Some(cb) = sink {
                cb(&state);
            }
        }

        pub async fn shutdown(&self) {
            clear_tray_sink();
        }
    }

    pub async fn spawn_tray(
        _cmd_tx: UnboundedSender<BackendCmd>,
        _win_tx: UnboundedSender<UiWindowCmd>,
    ) -> Option<TrayHandle> {
        Some(TrayHandle {
            state: tokio::sync::Mutex::new(TrayState::default()),
        })
    }
}

#[cfg(target_os = "linux")]
pub use linux_tray::{spawn_tray, TrayHandle};

#[cfg(not(target_os = "linux"))]
pub use tauri_tray::{clear_tray_sink, register_tray_sink, spawn_tray, TrayHandle};
