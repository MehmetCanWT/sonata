//! Linux MPRIS2 D-Bus media integration for the Sonata terminal client.
//!
//! 1. **MprisUpdate Channel**: accepts snapshot updates (`art_path` + `motion_cover_path`) and explicit seek notifications.
//! 2. **Linux Backend**: publishes `mpris:artUrl` (static cover) and custom `sonata:motionArtUrl` (animated `.mp4` cover) via `mpris_server::Player` on a dedicated `LocalSet`.
//! 3. **Media Key Routing**: forwards OS play/pause/seek/volume/shuffle/loop commands to `AppEvent::Mpris`.

use crate::app::{AppEvent, MprisCmd};
use sonata_core::player::RepeatMode;
use sonata_core::providers::Track;
use std::path::PathBuf;
use tokio::sync::mpsc::{self, UnboundedSender};

#[derive(Debug, Clone)]
pub struct TuiMprisSnapshot {
    pub track: Option<Track>,
    pub art_path: Option<PathBuf>,
    pub motion_cover_path: Option<PathBuf>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub paused: bool,
    pub muted: bool,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub queue_len: usize,
    pub current_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum MprisUpdate {
    Sync(Box<TuiMprisSnapshot>),
    Seeked(f64),
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::{mpsc, AppEvent, MprisCmd, MprisUpdate, RepeatMode, Track, UnboundedSender};
    use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};

    pub(super) fn sanitize_track_id(track_id: &str) -> TrackId {
        let clean: String = track_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let suffix = if clean.is_empty() { "unknown" } else { &clean };
        let path = format!("/xyz/mehmetcanwt/sonata/tui/track/{suffix}");
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
        track: Option<&Track>,
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
        app_tx: UnboundedSender<AppEvent>,
    ) -> UnboundedSender<MprisUpdate> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MprisUpdate>();

        if let Err(err) = std::thread::Builder::new()
            .name("sonata-tui-mpris".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        tracing::warn!(?err, "failed to build TUI MPRIS tokio runtime");
                        return;
                    }
                };
                let local = tokio::task::LocalSet::new();
                local.block_on(&rt, async move {
                    let player = match Player::builder("sonata")
                        .identity("Sonata")
                        .desktop_entry("sonata")
                        .can_quit(true)
                        .can_raise(false)
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
                            tracing::warn!(?err, "TUI MPRIS D-Bus unavailable; draining updates");
                            while rx.recv().await.is_some() {}
                            return;
                        }
                    };

                    {
                        let tx = app_tx.clone();
                        player.connect_play(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SetPaused(false)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_pause(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SetPaused(true)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_play_pause(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::TogglePause));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_stop(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::Stop));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_next(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::NextTrack));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_previous(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::PrevTrack));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_seek(move |_, delta| {
                            let secs = delta.as_micros() as f64 / 1_000_000.0;
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SeekRelative(secs)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_set_position(move |p, track_id, pos| {
                            if let Some(current) = p.metadata().trackid() {
                                if &current != track_id {
                                    return;
                                }
                            }
                            let secs = (pos.as_micros() as f64 / 1_000_000.0).max(0.0);
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SeekAbsolute(secs)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_set_volume(move |_, vol| {
                            let pct = (vol.clamp(0.0, 1.0) * 100.0).round() as u8;
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SetVolume(pct)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_set_shuffle(move |_, shuffle| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SetShuffle(shuffle)));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_set_loop_status(move |_, loop_status| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::SetRepeat(
                                from_mpris_loop(loop_status),
                            )));
                        });
                    }
                    {
                        let tx = app_tx.clone();
                        player.connect_quit(move |_| {
                            let _ = tx.send(AppEvent::Mpris(MprisCmd::Quit));
                        });
                    }

                    let mut run_task = tokio::task::spawn_local(player.run());
                    let mut last = LastSent::default();

                    loop {
                        let ev = tokio::select! {
                            res = &mut run_task => {
                                tracing::warn!(?res, "TUI MPRIS D-Bus loop exited; draining updates");
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

                                let track = snap.track.as_ref();
                                let playback_status = match track {
                                    None => PlaybackStatus::Stopped,
                                    Some(_) if snap.paused => PlaybackStatus::Paused,
                                    Some(_) => PlaybackStatus::Playing,
                                };

                                let eff_vol = if snap.muted { 0 } else { snap.volume };
                                let (can_go_next, can_go_prev) = compute_nav_capabilities(
                                    snap.queue_len,
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
            tracing::warn!(?err, "failed to spawn TUI MPRIS thread");
        }

        tx
    }
}

pub fn spawn_mpris(app_tx: UnboundedSender<AppEvent>) -> UnboundedSender<MprisUpdate> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::spawn_linux_mpris(app_tx)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app_tx;
        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::spawn(async move { while rx.recv().await.is_some() {} });
        tx
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::linux_impl::*;
    use super::*;

    #[test]
    fn tui_mpris_metadata_keeps_static_art_url_and_sets_motion_art_url() {
        let track = Track {
            id: "test-track-1".to_string(),
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            album_id: None,
            duration_secs: Some(180),
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
            180,
        );
        assert_eq!(meta.art_url().as_deref(), Some("file:///tmp/cover.png"));
        assert_eq!(
            meta.get_value("sonata:motionArtUrl")
                .and_then(|v| <&str>::try_from(v).ok()),
            Some("file:///tmp/motion.mp4")
        );
    }
}
