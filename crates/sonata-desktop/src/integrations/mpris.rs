//! Cross-platform OS media controls integration (MPRIS2 D-Bus on Linux, SMTC/NowPlaying via `souvlaki` on Windows/macOS).
//!
//! 1. **MprisUpdate Channel**: accepts snapshot syncs and explicit seek events from the backend engine.
//! 2. **Linux Backend**: runs `mpris_server::Player` inside a dedicated single-threaded Tokio `LocalSet`.
//! 3. **macOS & Windows Backend**: publishes metadata, playback state, and media key events via `souvlaki::MediaControls`.

use crate::backend::{BackendCmd, DesktopSnapshot, UiWindowCmd};
#[cfg(target_os = "linux")]
use sonata_core::player::RepeatMode;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum MprisUpdate {
    Sync(Arc<DesktopSnapshot>),
    Seeked(f64),
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
static WINDOWS_MEDIA_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Registers the main window HWND on Windows so SMTC can bind cleanly when media controls start.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn set_media_window_hwnd(hwnd: isize) {
    WINDOWS_MEDIA_HWND.store(hwnd, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::{BackendCmd, MprisUpdate, RepeatMode, UiWindowCmd};
    use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
    use tokio::sync::mpsc::{self, UnboundedSender};

    pub(super) fn sanitize_track_id(track_id: &str) -> TrackId {
        let clean: String = track_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let suffix = if clean.is_empty() { "unknown" } else { &clean };
        let path = format!("/xyz/mehmetcanwt/sonata/track/{suffix}");
        TrackId::try_from(path).unwrap_or(TrackId::NO_TRACK)
    }

    fn to_mpris_loop(mode: RepeatMode) -> LoopStatus {
        match mode {
            RepeatMode::Off => LoopStatus::None,
            RepeatMode::All => LoopStatus::Playlist,
            RepeatMode::One => LoopStatus::Track,
        }
    }

    fn from_mpris_loop(status: LoopStatus) -> RepeatMode {
        match status {
            LoopStatus::None => RepeatMode::Off,
            LoopStatus::Playlist => RepeatMode::All,
            LoopStatus::Track => RepeatMode::One,
        }
    }

    fn secs_to_time(secs: f64) -> Time {
        let micros = (secs.max(0.0) * 1_000_000.0).round() as i64;
        Time::from_micros(micros)
    }

    pub(super) fn local_art_url(path: &std::path::Path) -> Option<String> {
        reqwest::Url::from_file_path(path).ok().map(String::from)
    }

    pub(super) fn compute_nav_capabilities(
        queue_len: usize,
        current_index: Option<usize>,
        repeat: RepeatMode,
    ) -> (bool, bool) {
        if queue_len == 0 {
            return (false, false);
        }
        let Some(idx) = current_index else {
            return (false, false);
        };
        let wraps = matches!(repeat, RepeatMode::All | RepeatMode::One);
        let can_next = wraps || idx + 1 < queue_len;
        let can_prev = wraps || idx > 0;
        (can_next, can_prev)
    }

    #[derive(Default, PartialEq)]
    struct LastSent {
        track_id: String,
        title: String,
        artist: String,
        album: String,
        art_url: Option<String>,
        motion_art_url: Option<String>,
        duration_secs: u64,
        status: String,
        loop_status: String,
        shuffle: bool,
        volume_pct: u8,
        can_go_next: bool,
        can_go_prev: bool,
    }

    pub(super) fn build_metadata(
        track: Option<&sonata_core::providers::Track>,
        art_url: Option<&str>,
        motion_art_url: Option<&str>,
        dur_secs: u64,
    ) -> Metadata {
        let mut meta = Metadata::new();
        if let Some(t) = track {
            meta.set_trackid(Some(sanitize_track_id(&t.id)));
            meta.set_title(Some(t.title.clone()));
            if !t.artist.is_empty() {
                meta.set_artist(Some(vec![t.artist.clone()]));
            }
            if !t.album.is_empty() {
                meta.set_album(Some(t.album.clone()));
            }
            if dur_secs > 0 {
                meta.set_length(Some(Time::from_secs(dur_secs as i64)));
            }
            if let Some(art) = art_url {
                meta.set_art_url(Some(art.to_string()));
            }
            if let Some(motion) = motion_art_url {
                #[allow(deprecated)]
                let motion_val = mpris_server::zbus::zvariant::Value::from(motion.to_string());
                meta.set_value("sonata:motionArtUrl", Some(motion_val));
            }
        } else {
            meta.set_trackid(Some(TrackId::NO_TRACK));
        }
        meta
    }

    pub(super) fn spawn_linux_mpris(
        cmd_tx: UnboundedSender<BackendCmd>,
        win_tx: UnboundedSender<UiWindowCmd>,
    ) -> UnboundedSender<MprisUpdate> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MprisUpdate>();

        if let Err(err) = std::thread::Builder::new()
            .name("sonata-mpris".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        tracing::warn!(?err, "failed to build MPRIS tokio runtime");
                        return;
                    }
                };
                let local = tokio::task::LocalSet::new();
                local.block_on(&rt, async move {
                    let player = match Player::builder("sonata")
                        .identity("Sonata")
                        .desktop_entry("sonata-desktop")
                        .can_quit(true)
                        .can_raise(true)
                        .can_control(true)
                        .can_play(true)
                        .can_pause(true)
                        .can_go_next(false)
                        .can_go_previous(false)
                        .can_seek(true)
                        .build()
                        .await
                    {
                        Ok(player) => player,
                        Err(err) => {
                            tracing::warn!(?err, "MPRIS D-Bus unavailable; draining updates");
                            while rx.recv().await.is_some() {}
                            return;
                        }
                    };

                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_play(move |_| {
                            let _ = cmd_tx.send(BackendCmd::SetPaused(false));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_pause(move |_| {
                            let _ = cmd_tx.send(BackendCmd::SetPaused(true));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_play_pause(move |_| {
                            let _ = cmd_tx.send(BackendCmd::TogglePause);
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_stop(move |_| {
                            let _ = cmd_tx.send(BackendCmd::Stop);
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_next(move |_| {
                            let _ = cmd_tx.send(BackendCmd::NextTrack);
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_previous(move |_| {
                            let _ = cmd_tx.send(BackendCmd::PrevTrack);
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_seek(move |_, delta| {
                            let secs = delta.as_micros() as f64 / 1_000_000.0;
                            let _ = cmd_tx.send(BackendCmd::SeekRelative(secs));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_set_position(move |p, track_id, pos| {
                            if let Some(current) = p.metadata().trackid() {
                                if &current != track_id {
                                    return;
                                }
                            }
                            let secs = (pos.as_micros() as f64 / 1_000_000.0).max(0.0);
                            let _ = cmd_tx.send(BackendCmd::SeekAbsolute(secs));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_set_volume(move |_, vol| {
                            let pct = (vol.clamp(0.0, 1.0) * 100.0).round() as u8;
                            let _ = cmd_tx.send(BackendCmd::SetVolume(pct));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_set_shuffle(move |_, shuffle| {
                            let _ = cmd_tx.send(BackendCmd::SetShuffle(shuffle));
                        });
                    }
                    {
                        let cmd_tx = cmd_tx.clone();
                        player.connect_set_loop_status(move |_, loop_status| {
                            let _ =
                                cmd_tx.send(BackendCmd::SetRepeat(from_mpris_loop(loop_status)));
                        });
                    }
                    {
                        let win_tx = win_tx.clone();
                        player.connect_raise(move |_| {
                            let _ = win_tx.send(UiWindowCmd::ShowWindow);
                        });
                    }
                    {
                        let win_tx = win_tx.clone();
                        player.connect_quit(move |_| {
                            let _ = win_tx.send(UiWindowCmd::QuitApp);
                        });
                    }

                    let mut run_task = tokio::task::spawn_local(player.run());
                    let mut last = LastSent::default();

                    loop {
                        let ev = tokio::select! {
                            res = &mut run_task => {
                                tracing::warn!(?res, "MPRIS player D-Bus loop exited; draining updates");
                                while rx.recv().await.is_some() {}
                                return;
                            }
                            maybe_ev = rx.recv() => {
                                match maybe_ev {
                                    Some(ev) => ev,
                                    None => break,
                                }
                            }
                        };
                        match ev {
                            MprisUpdate::Seeked(secs) => {
                                let time = secs_to_time(secs);
                                player.set_position(time);
                                let _ = player.seeked(time).await;
                            }
                            MprisUpdate::Sync(snap) => {
                                player.set_position(secs_to_time(snap.position_secs));

                                let track = snap.current_track();
                                let playback_status = match track {
                                    None => PlaybackStatus::Stopped,
                                    Some(_) if snap.paused => PlaybackStatus::Paused,
                                    Some(_) => PlaybackStatus::Playing,
                                };

                                let eff_vol = if snap.muted { 0 } else { snap.volume };
                                let (can_go_next, can_go_prev) = compute_nav_capabilities(
                                    snap.queue.len(),
                                    snap.current_index,
                                    snap.repeat,
                                );
                                let loop_status = to_mpris_loop(snap.repeat);
                                let art_url = snap
                                    .art_path
                                    .as_deref()
                                    .and_then(local_art_url)
                                    .or_else(|| track.and_then(|t| t.thumbnail_url.clone()));
                                let motion_art_url = snap
                                    .motion_cover_path
                                    .as_deref()
                                    .and_then(local_art_url);

                                let dur_secs = snap
                                    .duration_secs
                                    .max(track.and_then(|t| t.duration_secs).unwrap_or(0) as f64)
                                    .round() as u64;

                                let next_state = LastSent {
                                    track_id: track.map(|t| t.id.clone()).unwrap_or_default(),
                                    title: track.map(|t| t.title.clone()).unwrap_or_default(),
                                    artist: track.map(|t| t.artist.clone()).unwrap_or_default(),
                                    album: track.map(|t| t.album.clone()).unwrap_or_default(),
                                    art_url: art_url.clone(),
                                    motion_art_url: motion_art_url.clone(),
                                    duration_secs: dur_secs,
                                    status: playback_status.as_str().to_string(),
                                    loop_status: loop_status.as_str().to_string(),
                                    shuffle: snap.shuffle,
                                    volume_pct: eff_vol,
                                    can_go_next,
                                    can_go_prev,
                                };

                                if next_state.track_id != last.track_id
                                    || next_state.title != last.title
                                    || next_state.artist != last.artist
                                    || next_state.album != last.album
                                    || next_state.art_url != last.art_url
                                    || next_state.motion_art_url != last.motion_art_url
                                    || next_state.duration_secs != last.duration_secs
                                {
                                    let meta = build_metadata(
                                        track,
                                        art_url.as_deref(),
                                        motion_art_url.as_deref(),
                                        dur_secs,
                                    );
                                    let _ = player.set_metadata(meta).await;
                                }

                                if next_state.status != last.status {
                                    let _ = player.set_playback_status(playback_status).await;
                                }
                                if next_state.loop_status != last.loop_status {
                                    let _ = player.set_loop_status(loop_status).await;
                                }
                                if next_state.shuffle != last.shuffle {
                                    let _ = player.set_shuffle(snap.shuffle).await;
                                }
                                if next_state.volume_pct != last.volume_pct {
                                    let _ = player.set_volume(eff_vol as f64 / 100.0).await;
                                }
                                if next_state.can_go_next != last.can_go_next {
                                    let _ = player.set_can_go_next(can_go_next).await;
                                }
                                if next_state.can_go_prev != last.can_go_prev {
                                    let _ = player.set_can_go_previous(can_go_prev).await;
                                }

                                last = next_state;
                            }
                        }
                    }

                    run_task.abort();
                });
            })
        {
            tracing::warn!(?err, "failed to spawn MPRIS thread");
        }

        tx
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod souvlaki_impl {
    use super::{BackendCmd, MprisUpdate, UiWindowCmd, WINDOWS_MEDIA_HWND};
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig, SeekDirection,
    };
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tokio::sync::mpsc::{self, UnboundedSender};

    pub(super) fn spawn_souvlaki_controls(
        cmd_tx: UnboundedSender<BackendCmd>,
        win_tx: UnboundedSender<UiWindowCmd>,
    ) -> UnboundedSender<MprisUpdate> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MprisUpdate>();

        if let Err(err) = std::thread::Builder::new()
            .name("sonata-media-controls".into())
            .spawn(move || {
                let raw_hwnd = WINDOWS_MEDIA_HWND.load(Ordering::Relaxed);
                let hwnd = if raw_hwnd != 0 {
                    Some(raw_hwnd as *mut c_void)
                } else {
                    None
                };
                let config = PlatformConfig {
                    display_name: "Sonata",
                    dbus_name: "dev.sonata.Desktop",
                    hwnd,
                };
                let Ok(mut controls) = MediaControls::new(config) else {
                    tracing::warn!("failed to initialize OS media controls");
                    while rx.blocking_recv().is_some() {}
                    return;
                };

                if let Err(err) = controls.attach(move |event: MediaControlEvent| match event {
                    MediaControlEvent::Play => {
                        let _ = cmd_tx.send(BackendCmd::SetPaused(false));
                    }
                    MediaControlEvent::Pause => {
                        let _ = cmd_tx.send(BackendCmd::SetPaused(true));
                    }
                    MediaControlEvent::Toggle => {
                        let _ = cmd_tx.send(BackendCmd::TogglePause);
                    }
                    MediaControlEvent::Next => {
                        let _ = cmd_tx.send(BackendCmd::NextTrack);
                    }
                    MediaControlEvent::Previous => {
                        let _ = cmd_tx.send(BackendCmd::PrevTrack);
                    }
                    MediaControlEvent::Stop => {
                        let _ = cmd_tx.send(BackendCmd::Stop);
                    }
                    MediaControlEvent::Seek(dir) => {
                        let delta = match dir {
                            SeekDirection::Forward => 10.0,
                            SeekDirection::Backward => -10.0,
                        };
                        let _ = cmd_tx.send(BackendCmd::SeekRelative(delta));
                    }
                    MediaControlEvent::SeekBy(dir, dur) => {
                        let secs = dur.as_secs_f64();
                        let signed = match dir {
                            SeekDirection::Forward => secs,
                            SeekDirection::Backward => -secs,
                        };
                        let _ = cmd_tx.send(BackendCmd::SeekRelative(signed));
                    }
                    MediaControlEvent::SetPosition(MediaPosition(dur)) => {
                        let _ = cmd_tx.send(BackendCmd::SeekAbsolute(dur.as_secs_f64()));
                    }
                    MediaControlEvent::SetVolume(vol) => {
                        let pct = (vol.clamp(0.0, 1.0) * 100.0).round() as u8;
                        let _ = cmd_tx.send(BackendCmd::SetVolume(pct));
                    }
                    MediaControlEvent::SetShuffle(shuffle) => {
                        let _ = cmd_tx.send(BackendCmd::SetShuffle(shuffle));
                    }
                    MediaControlEvent::Raise => {
                        let _ = win_tx.send(UiWindowCmd::ShowWindow);
                    }
                    MediaControlEvent::Quit => {
                        let _ = win_tx.send(UiWindowCmd::QuitApp);
                    }
                    MediaControlEvent::OpenUri(_) => {}
                    _ => {}
                }) {
                    tracing::warn!(?err, "failed to attach OS media controls callback");
                    while rx.blocking_recv().is_some() {}
                    return;
                }

                let mut last_track_key = String::new();
                while let Some(ev) = rx.blocking_recv() {
                    if let MprisUpdate::Sync(snap) = ev {
                        let track = snap.current_track();
                        let track_key = track
                            .map(|t| format!("{}|{}|{}", t.id, t.title, t.artist))
                            .unwrap_or_default();
                        if track_key != last_track_key {
                            last_track_key = track_key;
                            if let Some(t) = track {
                                let cover_url = snap
                                    .art_path
                                    .as_deref()
                                    .and_then(|p| {
                                        reqwest::Url::from_file_path(p).ok().map(String::from)
                                    })
                                    .or_else(|| t.thumbnail_url.clone());
                                let dur = (snap.duration_secs > 0.5)
                                    .then(|| Duration::from_secs_f64(snap.duration_secs));
                                let _ = controls.set_metadata(MediaMetadata {
                                    title: Some(&t.title),
                                    album: (!t.album.is_empty()).then_some(t.album.as_str()),
                                    artist: (!t.artist.is_empty()).then_some(t.artist.as_str()),
                                    cover_url: cover_url.as_deref(),
                                    duration: dur,
                                });
                            } else {
                                let _ = controls.set_metadata(MediaMetadata::default());
                            }
                        }
                        let progress = Some(MediaPosition(Duration::from_secs_f64(
                            snap.position_secs.max(0.0),
                        )));
                        let playback = match track {
                            None => MediaPlayback::Stopped,
                            Some(_) if snap.paused => MediaPlayback::Paused { progress },
                            Some(_) => MediaPlayback::Playing { progress },
                        };
                        let _ = controls.set_playback(playback);
                    }
                }
            })
        {
            tracing::warn!(?err, "failed to spawn media controls thread");
        }

        tx
    }
}

pub fn spawn_mpris(
    cmd_tx: UnboundedSender<BackendCmd>,
    win_tx: UnboundedSender<UiWindowCmd>,
) -> UnboundedSender<MprisUpdate> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::spawn_linux_mpris(cmd_tx, win_tx)
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        souvlaki_impl::spawn_souvlaki_controls(cmd_tx, win_tx)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::linux_impl::*;
    use super::*;

    #[test]
    fn sanitizes_arbitrary_track_ids_into_valid_dbus_paths() {
        assert_eq!(
            sanitize_track_id("dQw4w9W_gX-c:123").as_str(),
            "/xyz/mehmetcanwt/sonata/track/dQw4w9W_gX_c_123"
        );
        assert_eq!(
            sanitize_track_id("").as_str(),
            "/xyz/mehmetcanwt/sonata/track/unknown"
        );
    }

    #[test]
    fn local_art_url_percent_encodes_special_path_characters() {
        let uri = local_art_url(std::path::Path::new("/tmp/My Cover #1?.jpg"))
            .expect("absolute path converts to file URI");
        assert_eq!(uri, "file:///tmp/My%20Cover%20%231%3F.jpg");
        assert!(local_art_url(std::path::Path::new("relative/cover.jpg")).is_none());
    }

    #[test]
    fn compute_nav_capabilities_respects_bounds_and_repeat_mode() {
        assert_eq!(
            compute_nav_capabilities(0, None, RepeatMode::Off),
            (false, false)
        );
        assert_eq!(
            compute_nav_capabilities(3, None, RepeatMode::Off),
            (false, false)
        );
        assert_eq!(
            compute_nav_capabilities(3, Some(0), RepeatMode::Off),
            (true, false)
        );
        assert_eq!(
            compute_nav_capabilities(3, Some(1), RepeatMode::Off),
            (true, true)
        );
        assert_eq!(
            compute_nav_capabilities(3, Some(2), RepeatMode::Off),
            (false, true)
        );
        assert_eq!(
            compute_nav_capabilities(1, Some(0), RepeatMode::Off),
            (false, false)
        );
        assert_eq!(
            compute_nav_capabilities(1, Some(0), RepeatMode::All),
            (true, true)
        );
        assert_eq!(
            compute_nav_capabilities(1, Some(0), RepeatMode::One),
            (true, true)
        );
    }

    #[test]
    fn build_metadata_preserves_static_art_url_and_sets_motion_art_url() {
        let track = sonata_core::providers::Track {
            id: "test-track-1".to_string(),
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            album_id: None,
            duration_secs: Some(200),
            thumbnail_url: None,
            source: sonata_core::providers::ProviderSource::YouTubeMusic,
            liked: false,
            is_explicit: false,
            set_video_id: None,
            audio_badge: None,
        };
        let meta = build_metadata(
            Some(&track),
            Some("file:///tmp/cover.png"),
            Some("file:///tmp/motion.mp4"),
            200,
        );
        assert_eq!(meta.art_url().as_deref(), Some("file:///tmp/cover.png"));
        assert_eq!(
            meta.get_value("sonata:motionArtUrl")
                .and_then(|v| <&str>::try_from(v).ok()),
            Some("file:///tmp/motion.mp4")
        );
    }
}
