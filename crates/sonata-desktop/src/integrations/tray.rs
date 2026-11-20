use crate::backend::{BackendCmd, UiWindowCmd};
use ksni::TrayMethods;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayState {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub paused: bool,
    pub has_track: bool,
    pub window_visible: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            title: None,
            artist: None,
            paused: true,
            has_track: false,
            window_visible: true,
        }
    }
}

pub struct TrayHandle {
    inner: ksni::Handle<SonataTray>,
}

impl TrayHandle {
    pub async fn update(&self, state: TrayState) {
        let _ = self
            .inner
            .update(move |tray| {
                tray.state = state;
            })
            .await;
    }

    pub async fn shutdown(&self) {
        let awaiter = self.inner.shutdown();
        let _ = tokio::time::timeout(Duration::from_millis(500), awaiter).await;
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
                // Smooth anti-aliased edge + plum/slate gradient background
                let edge_alpha = (14.5 - dist).clamp(0.0, 1.0);
                let t = (x as f32 + y as f32) / 62.0;
                let bg_r = 34.0 + t * 54.0;
                let bg_g = 28.0 + t * 24.0;
                let bg_b = 58.0 + t * 72.0;

                // Crisp rim highlight
                let rim = if dist >= 12.7 { 0.45 } else { 0.0 };
                let fr = (bg_r + (203.0 - bg_r) * rim).round() as u8;
                let fg = (bg_g + (166.0 - bg_g) * rim).round() as u8;
                let fb = (bg_b + (247.0 - bg_b) * rim).round() as u8;

                // Eighth-note glyph inside center (stem, notehead, flag)
                let nx = x as f32;
                let ny = y as f32;
                let head_dist = (((nx - 12.8) / 3.6).powi(2) + ((ny - 21.0) / 2.7).powi(2)).sqrt();
                let in_head = head_dist <= 1.0;
                let in_stem = (15.0..=17.1).contains(&nx) && (8.5..=21.0).contains(&ny);
                let in_flag = (16.5..=22.0).contains(&nx)
                    && (ny >= 8.5 + (nx - 16.5) * 0.55)
                    && (ny <= 11.4 + (nx - 16.5) * 0.55);

                if in_head || in_stem || in_flag {
                    r = 245;
                    g = 240;
                    b = 255;
                    a = (edge_alpha * 255.0).round() as u8;
                } else {
                    r = fr;
                    g = fg;
                    b = fb;
                    a = (edge_alpha * 240.0).round() as u8;
                }
            }

            // StatusNotifierItem expects network byte order ARGB32
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
        let description = match (&self.state.title, &self.state.artist) {
            (Some(title), Some(artist)) if !artist.is_empty() => format!("{title} — {artist}"),
            (Some(title), _) => title.clone(),
            _ => "Ready".to_string(),
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

        let (toggle_label, toggle_cmd) = if self.state.window_visible {
            ("Hide Window", UiWindowCmd::HideWindow)
        } else {
            ("Show Window", UiWindowCmd::ShowWindow)
        };
        let play_label = if self.state.paused || !self.state.has_track {
            "Play"
        } else {
            "Pause"
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
                label: "Next Track".to_string(),
                enabled: self.state.has_track,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.cmd_tx.send(BackendCmd::NextTrack);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "Previous Track".to_string(),
                enabled: self.state.has_track,
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.cmd_tx.send(BackendCmd::PrevTrack);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".to_string(),
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
    tray.spawn().await.ok().map(|inner| TrayHandle { inner })
}
