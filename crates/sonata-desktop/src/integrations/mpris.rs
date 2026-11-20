use crate::backend::{BackendCmd, DesktopSnapshot, UiWindowCmd};
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
use sonata_core::player::RepeatMode;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};

#[derive(Debug, Clone)]
pub enum MprisUpdate {
    Sync(Arc<DesktopSnapshot>),
    Seeked(f64),
}

fn sanitize_track_id(track_id: &str) -> TrackId {
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

#[derive(Default, PartialEq)]
struct LastSent {
    track_id: String,
    title: String,
    artist: String,
    album: String,
    art_url: Option<String>,
    duration_secs: u64,
    status: String,
    loop_status: String,
    shuffle: bool,
    volume_pct: u8,
    can_go_next: bool,
    can_go_prev: bool,
}

pub fn spawn_mpris(
    cmd_tx: UnboundedSender<BackendCmd>,
    win_tx: UnboundedSender<UiWindowCmd>,
) -> UnboundedSender<MprisUpdate> {
    let (tx, mut rx) = mpsc::unbounded_channel::<MprisUpdate>();

    let _ = std::thread::Builder::new()
        .name("sonata-mpris".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let Ok(player) = Player::builder("sonata")
                    .identity("Sonata")
                    .desktop_entry("sonata-desktop")
                    .can_quit(true)
                    .can_raise(true)
                    .can_control(true)
                    .can_play(true)
                    .can_pause(true)
                    .can_go_next(true)
                    .can_go_previous(true)
                    .can_seek(true)
                    .build()
                    .await
                else {
                    // Drain cleanly if D-Bus session is unavailable.
                    while rx.recv().await.is_some() {}
                    return;
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
                        let _ = cmd_tx.send(BackendCmd::SetRepeat(from_mpris_loop(loop_status)));
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

                let run_task = tokio::task::spawn_local(player.run());
                let mut last = LastSent::default();

                while let Some(ev) = rx.recv().await {
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
                            let can_nav = !snap.queue.is_empty();
                            let loop_status = to_mpris_loop(snap.repeat);
                            let art_url = snap
                                .art_path
                                .as_ref()
                                .map(|p| format!("file://{}", p.to_string_lossy()))
                                .or_else(|| track.and_then(|t| t.thumbnail_url.clone()));

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
                                duration_secs: dur_secs,
                                status: playback_status.as_str().to_string(),
                                loop_status: loop_status.as_str().to_string(),
                                shuffle: snap.shuffle,
                                volume_pct: eff_vol,
                                can_go_next: can_nav,
                                can_go_prev: can_nav,
                            };

                            if next_state.track_id != last.track_id
                                || next_state.title != last.title
                                || next_state.artist != last.artist
                                || next_state.album != last.album
                                || next_state.art_url != last.art_url
                                || next_state.duration_secs != last.duration_secs
                            {
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
                                    if let Some(ref art) = art_url {
                                        meta.set_art_url(Some(art.clone()));
                                    }
                                } else {
                                    meta.set_trackid(Some(TrackId::NO_TRACK));
                                }
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
                                let _ = player.set_can_go_next(can_nav).await;
                            }
                            if next_state.can_go_prev != last.can_go_prev {
                                let _ = player.set_can_go_previous(can_nav).await;
                            }

                            last = next_state;
                        }
                    }
                }

                run_task.abort();
            });
        });

    tx
}

#[cfg(test)]
mod tests {
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
}
