//! Deterministic unit tests for [`PlaybackSession`] audio quality, transport controls,
//! sleep-timer fades, and crossfade ramps.
//!
//! 1. **Test Doubles**: `RecordingDriver` and `StaticSource` isolate actor transitions without
//!    spawning live `mpv` or `yt-dlp` subprocesses.
//! 2. **Coverage**: verifies FLAC bitrate measurement, speed/silence/video-mode switching, sleep
//!    timer fade/restore, and crossfade ramp + video-mode bypass behavior.

use super::{
    clamp_crossfade_secs, compute_crossfade_entry_volume, compute_crossfade_exit_volume,
    PlaybackAudioDriver, PlaybackSession, PlaybackSessionConfig, PlaybackStreamSource,
    SessionCommand, SessionEvent, SessionPolicy,
};
use crate::downloads::test_support::TempTestDir;
use crate::player::mpv::{MpvAudioParams, MpvEvent};
use sonata_core::player::RepeatMode;
use sonata_core::providers::hifi::ResolvedStream;
use sonata_core::providers::{ProviderSource, Track};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
enum TestDriverCall {
    Load(String),
    LoadAt(String, f64),
    Stop,
    Paused(bool),
    Seek(f64),
    Volume(u8),
    Speed(f64),
    SkipSilence(bool),
    Gapless(bool),
    VideoMode {
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    },
    Shutdown,
}

#[derive(Default)]
struct RecordingDriver {
    calls: Mutex<Vec<TestDriverCall>>,
}

impl RecordingDriver {
    fn calls(&self) -> Vec<TestDriverCall> {
        self.calls.lock().expect("driver lock").clone()
    }
}

impl PlaybackAudioDriver for RecordingDriver {
    fn load_stream(&self, stream: &ResolvedStream) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Load(stream.url.clone()));
    }

    fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::LoadAt(stream.url.clone(), start_secs));
    }

    fn stop(&self) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Stop);
    }

    fn set_paused(&self, paused: bool) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Paused(paused));
    }

    fn seek_absolute(&self, secs: f64) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Seek(secs));
    }

    fn set_volume(&self, volume: u8) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Volume(volume));
    }

    fn set_speed(&self, speed: f64) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Speed(speed));
    }

    fn set_skip_silence(&self, enabled: bool) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::SkipSilence(enabled));
    }

    fn set_gapless(&self, enabled: bool) {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Gapless(enabled));
    }

    fn set_video_mode(
        &self,
        enabled: bool,
        url: Option<String>,
        headers: Vec<(String, String)>,
        seek_secs: Option<f64>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::VideoMode {
                enabled,
                url,
                headers,
                seek_secs,
            });
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.calls
            .lock()
            .expect("driver lock")
            .push(TestDriverCall::Shutdown);
        Box::pin(async {})
    }
}

struct StaticSource {
    stream: ResolvedStream,
}

impl PlaybackStreamSource for StaticSource {
    fn resolve_fast(
        &self,
        _track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
        let s = self.stream.clone();
        Box::pin(async move { Ok(s) })
    }

    fn resolve_recovery(
        &self,
        _track: Track,
    ) -> Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>> {
        let s = self.stream.clone();
        Box::pin(async move { Ok(s) })
    }

    fn resolve_hifi_upgrade(
        &self,
        _track: Track,
        _current: ResolvedStream,
        _allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        Box::pin(async { None })
    }

    fn precache_hifi_audio(
        &self,
        _track: Track,
        _allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        Box::pin(async { None })
    }
}

fn make_track(id: &str, duration_secs: u64) -> Track {
    Track {
        id: id.to_string(),
        title: format!("Track {id}"),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        duration_secs: Some(duration_secs),
        thumbnail_url: None,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    }
}

fn fast_test_config() -> PlaybackSessionConfig {
    PlaybackSessionConfig {
        client_name: "session-unit-test",
        initial_volume: 80,
        initial_muted: false,
        initial_shuffle: false,
        initial_repeat: RepeatMode::Off,
        initial_gapless: true,
        initial_crossfade_secs: 0,
        policy: SessionPolicy {
            hifi_substitution: false,
            jiosaavn_enabled: false,
            cache_enabled: false,
        },
        stale_error_grace: Duration::from_millis(50),
        shutdown_timeout: Duration::from_millis(500),
        tick_interval: Duration::from_millis(40),
        position_steering: false,
    }
}

async fn recv_event_matching<F>(
    events: &mut mpsc::Receiver<SessionEvent>,
    mut pred: F,
) -> SessionEvent
where
    F: FnMut(&SessionEvent) -> bool,
{
    tokio::time::timeout(Duration::from_secs(4), async {
        while let Some(ev) = events.recv().await {
            if pred(&ev) {
                return ev;
            }
        }
        panic!("session event channel closed before matching event arrived");
    })
    .await
    .expect("timed out waiting for SessionEvent")
}

#[tokio::test]
async fn emits_measured_audio_quality_for_live_and_local_flac_streams() {
    let dir = TempTestDir::new("session_local_flac");
    let flac_path = dir.path().join("lossless.flac");
    // 198_000 bytes over 1.8s -> 880 kbps exact
    std::fs::write(&flac_path, vec![0u8; 198_000]).expect("write test flac");

    let driver = Arc::new(RecordingDriver::default());
    let source = Arc::new(StaticSource {
        stream: ResolvedStream {
            url: flac_path.to_string_lossy().into_owned(),
            badge: "FLAC · Local".to_string(),
            headers: Vec::new(),
        },
    });
    let (mpv_tx, mpv_rx) = mpsc::channel(32);
    let mut session = PlaybackSession::spawn(
        fast_test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        source,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![make_track("local-flac", 2)],
            start_index: 0,
            push_history: false,
        })
        .await
        .expect("play queue");

    recv_event_matching(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;

    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    mpv_tx.send(MpvEvent::Duration(1.8)).await.unwrap();
    mpv_tx
        .send(MpvEvent::AudioCodec(Some("flac".to_string())))
        .await
        .unwrap();
    mpv_tx
        .send(MpvEvent::AudioParams(Some(MpvAudioParams {
            sample_rate_hz: Some(44100),
            bit_depth: Some(16),
            format: Some("s16".to_string()),
        })))
        .await
        .unwrap();

    let local_ev = recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::AudioQualityUpdated { badge, .. }
                if badge == "FLAC · 880 kbps · 16-bit/44.1kHz"
        )
    })
    .await;
    assert!(matches!(
        local_ev,
        SessionEvent::AudioQualityUpdated {
            bitrate_kbps: Some(880),
            sample_rate_hz: Some(44100),
            bit_depth: Some(16),
            ..
        }
    ));

    // Live audio-bitrate overrides file-size calculation when present (e.g., 24-bit/96kHz stream)
    mpv_tx
        .send(MpvEvent::AudioBitrate(Some(2_450_000)))
        .await
        .unwrap();
    mpv_tx
        .send(MpvEvent::AudioParams(Some(MpvAudioParams {
            sample_rate_hz: Some(96000),
            bit_depth: Some(24),
            format: Some("s24".to_string()),
        })))
        .await
        .unwrap();

    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::AudioQualityUpdated { badge, .. }
                if badge == "FLAC · 2450 kbps · 24-bit/96kHz"
        )
    })
    .await;

    session.handle.shutdown().await;
}

#[tokio::test]
async fn handles_speed_skip_silence_and_video_mode_switch() {
    let driver = Arc::new(RecordingDriver::default());
    let source = Arc::new(StaticSource {
        stream: ResolvedStream {
            url: "https://example.com/audio.webm".to_string(),
            badge: "OPUS 256kbps · Extreme".to_string(),
            headers: vec![("User-Agent".to_string(), "SonataTest/1.0".to_string())],
        },
    });
    let (mpv_tx, mpv_rx) = mpsc::channel(32);
    let mut session = PlaybackSession::spawn(
        fast_test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        source,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![make_track("vid-1", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    mpv_tx.send(MpvEvent::Position(42.5)).await.unwrap();

    // Speed clamping
    session
        .handle
        .send(SessionCommand::SetPlaybackSpeed(4.0))
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged { playback_speed, .. }
                if (*playback_speed - 2.0).abs() < 1e-6
        )
    })
    .await;

    // Enable skip silence in audio mode
    session
        .handle
        .send(SessionCommand::SetSkipSilence(true))
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                skip_silence: true,
                video_mode: false,
                ..
            }
        )
    })
    .await;

    // Switch to video mode: skip_silence auto-disables and preserves headers & position
    session
        .handle
        .send(SessionCommand::SetVideoMode {
            enabled: true,
            url: Some("https://example.com/video720.mp4".to_string()),
            headers: vec![(
                "Referer".to_string(),
                "https://music.youtube.com/".to_string(),
            )],
            seek_secs: Some(42.5),
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                skip_silence: false,
                video_mode: true,
                ..
            }
        )
    })
    .await;

    // Exit video mode: skip_silence preference resumes
    session
        .handle
        .send(SessionCommand::SetVideoMode {
            enabled: false,
            url: None,
            headers: Vec::new(),
            seek_secs: Some(50.0),
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                skip_silence: true,
                video_mode: false,
                ..
            }
        )
    })
    .await;

    let calls = driver.calls();
    assert!(calls.contains(&TestDriverCall::Speed(2.0)));
    assert!(calls.contains(&TestDriverCall::SkipSilence(true)));
    assert!(calls.contains(&TestDriverCall::SkipSilence(false)));
    assert!(calls.iter().any(|c| matches!(
        c,
        TestDriverCall::VideoMode {
            enabled: true,
            url: Some(u),
            headers,
            seek_secs: Some(pos),
        } if u == "https://example.com/video720.mp4"
            && headers == &vec![("Referer".to_string(), "https://music.youtube.com/".to_string())]
            && (*pos - 42.5).abs() < 1e-6
    )));

    session.handle.shutdown().await;
}

#[tokio::test]
async fn sleep_timer_fades_volume_pauses_and_restores_volume() {
    let driver = Arc::new(RecordingDriver::default());
    let source = Arc::new(StaticSource {
        stream: ResolvedStream {
            url: "https://example.com/audio.webm".to_string(),
            badge: "AAC · 320 kbps".to_string(),
            headers: Vec::new(),
        },
    });
    let (mpv_tx, mpv_rx) = mpsc::channel(32);
    let mut session = PlaybackSession::spawn(
        fast_test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        source,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![make_track("sleep-1", 300)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();

    // 1-second sleep timer enters fade immediately (min(5s, 1s) = 1s fade) and pauses + restores 80
    session
        .handle
        .send(SessionCommand::SetSleepTimer(Some(1)))
        .await
        .unwrap();

    recv_event_matching(&mut session.events, |e| {
        matches!(e, SessionEvent::PausedChanged { paused: true, .. })
    })
    .await;
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                sleep_timer_remaining_secs: None,
                ..
            }
        )
    })
    .await;

    let volumes: Vec<u8> = driver
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            TestDriverCall::Volume(v) => Some(v),
            _ => None,
        })
        .collect();
    assert!(
        volumes.iter().any(|&v| v < 80),
        "expected smooth fade volumes below 80, got {volumes:?}"
    );
    assert_eq!(
        volumes.last().copied(),
        Some(80),
        "expected volume restored to 80 after pause, got {volumes:?}"
    );

    session.handle.shutdown().await;
}

#[tokio::test]
async fn handles_gapless_and_crossfade_ramp_bypassed_in_video_mode() {
    assert_eq!(clamp_crossfade_secs(0), 0);
    assert_eq!(clamp_crossfade_secs(7), 7);
    assert_eq!(clamp_crossfade_secs(99), 12);
    assert_eq!(compute_crossfade_exit_volume(80, 2.0, 4.0), 40);
    assert_eq!(compute_crossfade_exit_volume(80, 0.0, 4.0), 0);
    assert_eq!(compute_crossfade_entry_volume(80, 1.0, 4.0), 20);
    assert_eq!(compute_crossfade_entry_volume(80, 4.0, 4.0), 80);

    let driver = Arc::new(RecordingDriver::default());
    let source = Arc::new(StaticSource {
        stream: ResolvedStream {
            url: "https://example.com/audio.webm".to_string(),
            badge: "OPUS 256kbps".to_string(),
            headers: Vec::new(),
        },
    });
    let (mpv_tx, mpv_rx) = mpsc::channel(32);
    let mut session = PlaybackSession::spawn(
        fast_test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        source,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::SetGapless(false))
        .await
        .unwrap();
    session
        .handle
        .send(SessionCommand::SetCrossfade(15))
        .await
        .unwrap();
    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![make_track("cf-1", 20), make_track("cf-2", 20)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    mpv_tx.send(MpvEvent::Duration(20.0)).await.unwrap();

    // Advance into the final 8s (capped at 40% of 20s = 8s) so exit ramp lowers volume below 80.
    mpv_tx.send(MpvEvent::Position(16.0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;

    let exit_vols: Vec<u8> = driver
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            TestDriverCall::Volume(v) => Some(v),
            _ => None,
        })
        .collect();
    assert!(
        exit_vols.iter().any(|&v| v < 80),
        "expected crossfade exit volume ramp below 80, got {exit_vols:?}"
    );

    // Video mode automatically bypasses crossfade and restores configured volume 80.
    session
        .handle
        .send(SessionCommand::SetVideoMode {
            enabled: true,
            url: Some("https://example.com/vid.mp4".to_string()),
            headers: Vec::new(),
            seek_secs: Some(16.0),
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                video_mode: true,
                ..
            }
        )
    })
    .await;

    let after_video_vols: Vec<u8> = driver
        .calls()
        .into_iter()
        .filter_map(|c| match c {
            TestDriverCall::Volume(v) => Some(v),
            _ => None,
        })
        .collect();
    assert_eq!(after_video_vols.last().copied(), Some(80));

    // Exit video mode and trigger natural track end -> next track fades in from 0.
    session
        .handle
        .send(SessionCommand::SetVideoMode {
            enabled: false,
            url: None,
            headers: Vec::new(),
            seek_secs: Some(18.0),
        })
        .await
        .unwrap();
    recv_event_matching(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PlaybackOptionsChanged {
                video_mode: false,
                ..
            }
        )
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    mpv_tx.send(MpvEvent::Position(18.5)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(90)).await;
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
    recv_event_matching(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "cf-2"),
    )
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;

    let calls = driver.calls();
    assert!(calls.contains(&TestDriverCall::Gapless(false)));
    assert!(
        calls.contains(&TestDriverCall::Volume(0)),
        "expected entry fade-in starting from 0, got {calls:?}"
    );

    session.handle.shutdown().await;
}
