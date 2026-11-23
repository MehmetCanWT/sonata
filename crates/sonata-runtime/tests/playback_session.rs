//! Deterministic state-machine tests for the shared Linux [`PlaybackSession`].
//!
//! 1. **Doubles**: In-memory `FakeAudioDriver` and `FakeStreamSource` capture every driver call and
//!    let tests gate or fail stream resolution, recovery, upgrades, and precaches without spawning `mpv`.
//! 2. **Coverage**: Exercises all 9 required Phase 6 invariants: stale generation rejection, normal
//!    and repeat-one end transitions, previous-history behavior, seek clamping, recovery retry budget,
//!    upgrade swap and fallback, precache lifecycle, bounded shutdown, and simultaneous mpv end/error signals.

use sonata_core::player::RepeatMode;
use sonata_core::providers::hifi::ResolvedStream;
use sonata_core::providers::{ProviderSource, Track};
use sonata_runtime::player::{
    MpvEvent, PlaybackAudioDriver, PlaybackSession, PlaybackSessionConfig, PlaybackStreamSource,
    PrecacheNotice, SessionCommand, SessionEvent, SessionPolicy,
};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Notify};

fn track(id: &str, duration_secs: u64) -> Track {
    Track {
        id: id.to_string(),
        title: format!("Title {id}"),
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

fn stream(url: &str, badge: &str) -> ResolvedStream {
    ResolvedStream {
        url: url.to_string(),
        badge: badge.to_string(),
        headers: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DriverOp {
    Load(String),
    LoadAt(String, f64),
    Stop,
    SetPaused(bool),
    SeekAbsolute(f64),
    SetVolume(u8),
    Shutdown,
}

#[derive(Default)]
struct FakeAudioDriver {
    ops: Mutex<Vec<DriverOp>>,
    shutdown_called: AtomicBool,
}

impl FakeAudioDriver {
    fn snapshot(&self) -> Vec<DriverOp> {
        self.ops.lock().expect("driver lock").clone()
    }

    fn clear(&self) {
        self.ops.lock().expect("driver lock").clear();
    }
}

impl PlaybackAudioDriver for FakeAudioDriver {
    fn load_stream(&self, stream: &ResolvedStream) {
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::Load(stream.url.clone()));
    }

    fn load_stream_at(&self, stream: &ResolvedStream, start_secs: f64) {
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::LoadAt(stream.url.clone(), start_secs));
    }

    fn stop(&self) {
        self.ops.lock().expect("driver lock").push(DriverOp::Stop);
    }

    fn set_paused(&self, paused: bool) {
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::SetPaused(paused));
    }

    fn seek_absolute(&self, secs: f64) {
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::SeekAbsolute(secs));
    }

    fn set_volume(&self, volume: u8) {
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::SetVolume(volume));
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.shutdown_called.store(true, Ordering::SeqCst);
        self.ops
            .lock()
            .expect("driver lock")
            .push(DriverOp::Shutdown);
        Box::pin(async {})
    }
}

type FastResult = Result<ResolvedStream, String>;

struct DropCounter {
    count: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

#[derive(Default)]
struct FakeStreamSource {
    fast_gates: Mutex<HashMap<String, oneshot::Receiver<FastResult>>>,
    fast_results: Mutex<HashMap<String, VecDeque<FastResult>>>,
    recovery_results: Mutex<HashMap<String, VecDeque<FastResult>>>,
    upgrade_gates: Mutex<HashMap<String, oneshot::Receiver<Option<ResolvedStream>>>>,
    upgrade_results: Mutex<HashMap<String, VecDeque<Option<ResolvedStream>>>>,
    precache_gates: Mutex<HashMap<String, oneshot::Receiver<Option<ResolvedStream>>>>,
    precache_results: Mutex<HashMap<String, VecDeque<Option<ResolvedStream>>>>,
    recovery_calls: AtomicUsize,
    precache_calls: AtomicUsize,
    dropped_tasks: Arc<AtomicUsize>,
    drop_notify: Arc<Notify>,
}

impl FakeStreamSource {
    fn gate_fast(&self, id: &str) -> oneshot::Sender<FastResult> {
        let (tx, rx) = oneshot::channel();
        self.fast_gates
            .lock()
            .expect("fast gate lock")
            .insert(id.to_string(), rx);
        tx
    }

    fn set_recovery(&self, id: &str, res: FastResult) {
        self.recovery_results
            .lock()
            .expect("recovery lock")
            .entry(id.to_string())
            .or_default()
            .push_back(res);
    }

    fn gate_upgrade(&self, id: &str) -> oneshot::Sender<Option<ResolvedStream>> {
        let (tx, rx) = oneshot::channel();
        self.upgrade_gates
            .lock()
            .expect("upgrade gate lock")
            .insert(id.to_string(), rx);
        tx
    }

    fn gate_precache(&self, id: &str) -> oneshot::Sender<Option<ResolvedStream>> {
        let (tx, rx) = oneshot::channel();
        self.precache_gates
            .lock()
            .expect("precache gate lock")
            .insert(id.to_string(), rx);
        tx
    }
}

impl PlaybackStreamSource for FakeStreamSource {
    fn resolve_fast(&self, track: Track) -> Pin<Box<dyn Future<Output = FastResult> + Send>> {
        let gate = self
            .fast_gates
            .lock()
            .expect("fast gate lock")
            .remove(&track.id);
        let scripted = self
            .fast_results
            .lock()
            .expect("fast lock")
            .get_mut(&track.id)
            .and_then(VecDeque::pop_front);
        let drop_guard = DropCounter {
            count: Arc::clone(&self.dropped_tasks),
            notify: Arc::clone(&self.drop_notify),
        };
        Box::pin(async move {
            let _guard = drop_guard;
            if let Some(rx) = gate {
                rx.await.unwrap_or_else(|_| Err("gate dropped".to_string()))
            } else if let Some(res) = scripted {
                res
            } else {
                Ok(stream(
                    &format!("https://stream.test/{}/base", track.id),
                    "AAC 128kbps",
                ))
            }
        })
    }

    fn resolve_recovery(&self, track: Track) -> Pin<Box<dyn Future<Output = FastResult> + Send>> {
        self.recovery_calls.fetch_add(1, Ordering::SeqCst);
        let scripted = self
            .recovery_results
            .lock()
            .expect("recovery lock")
            .get_mut(&track.id)
            .and_then(VecDeque::pop_front);
        Box::pin(async move {
            scripted.unwrap_or_else(|| {
                Ok(stream(
                    &format!("https://stream.test/{}/recovered", track.id),
                    "AAC 128kbps",
                ))
            })
        })
    }

    fn resolve_hifi_upgrade(
        &self,
        track: Track,
        _current: ResolvedStream,
        _allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        let gate = self
            .upgrade_gates
            .lock()
            .expect("upgrade gate lock")
            .remove(&track.id);
        let scripted = self
            .upgrade_results
            .lock()
            .expect("upgrade lock")
            .get_mut(&track.id)
            .and_then(VecDeque::pop_front);
        Box::pin(async move {
            if let Some(rx) = gate {
                rx.await.ok().flatten()
            } else {
                scripted.flatten()
            }
        })
    }

    fn precache_hifi_audio(
        &self,
        track: Track,
        _allow_hifi: bool,
    ) -> Pin<Box<dyn Future<Output = Option<ResolvedStream>> + Send>> {
        self.precache_calls.fetch_add(1, Ordering::SeqCst);
        let gate = self
            .precache_gates
            .lock()
            .expect("precache gate lock")
            .remove(&track.id);
        let scripted = self
            .precache_results
            .lock()
            .expect("precache lock")
            .get_mut(&track.id)
            .and_then(VecDeque::pop_front);
        let drop_guard = DropCounter {
            count: Arc::clone(&self.dropped_tasks),
            notify: Arc::clone(&self.drop_notify),
        };
        Box::pin(async move {
            let _guard = drop_guard;
            if let Some(rx) = gate {
                rx.await.ok().flatten()
            } else {
                scripted.flatten()
            }
        })
    }
}

fn test_config() -> PlaybackSessionConfig {
    PlaybackSessionConfig {
        client_name: "test",
        initial_volume: 80,
        initial_muted: false,
        initial_shuffle: false,
        initial_repeat: RepeatMode::Off,
        policy: SessionPolicy {
            hifi_substitution: true,
            jiosaavn_enabled: false,
            cache_enabled: true,
        },
        stale_error_grace: Duration::ZERO,
        shutdown_timeout: Duration::from_millis(250),
        tick_interval: Duration::from_millis(20),
        position_steering: false,
    }
}

async fn recv_until<F>(events: &mut mpsc::Receiver<SessionEvent>, mut predicate: F) -> SessionEvent
where
    F: FnMut(&SessionEvent) -> bool,
{
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(ev) = events.recv().await {
            if predicate(&ev) {
                return ev;
            }
        }
        panic!("event stream closed before expected event arrived");
    })
    .await
    .expect("timed out waiting for expected SessionEvent")
}

async fn drain_events(events: &mut mpsc::Receiver<SessionEvent>) -> Vec<SessionEvent> {
    tokio::time::sleep(Duration::from_millis(35)).await;
    let mut out = Vec::new();
    while let Ok(ev) = events.try_recv() {
        out.push(ev);
    }
    out
}

#[tokio::test]
async fn stale_generation_rejection() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let gate_a = source.gate_fast("a");
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180), track("b", 200)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();

    let ev_a = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::TrackTransition { track, .. } if track.id == "a"),
    )
    .await;
    let gen_a = match ev_a {
        SessionEvent::TrackTransition { generation, .. } => generation,
        _ => unreachable!(),
    };

    // Request track B before A finishes resolving.
    session
        .handle
        .send(SessionCommand::PlayIndex {
            index: 1,
            push_history: true,
        })
        .await
        .unwrap();

    let loaded_b = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "b"),
    )
    .await;
    let gen_b = match loaded_b {
        SessionEvent::StreamLoaded { generation, .. } => generation,
        _ => unreachable!(),
    };
    assert!(gen_b > gen_a);

    // Late resolution of A cannot overwrite B.
    let _ = gate_a.send(Ok(stream("https://stream.test/a/late", "AAC 128kbps")));
    let trailing = drain_events(&mut session.events).await;
    assert!(
        !trailing
            .iter()
            .any(|e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a")),
        "stale A stream must be rejected"
    );
    assert!(
        !driver.snapshot().iter().any(
            |op| matches!(op, DriverOp::Load(u) | DriverOp::LoadAt(u, _) if u.contains("/a/"))
        ),
        "audio driver must never load stale A"
    );

    // Same-track replay rejects stale upgrade from earlier generation.
    let upgrade_gate = source.gate_upgrade("b");
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::UpgradeStarted { generation, .. } if *generation == gen_b),
    )
    .await;

    // Replay B (same track ID, new generation).
    session
        .handle
        .send(SessionCommand::PlayIndex {
            index: 1,
            push_history: false,
        })
        .await
        .unwrap();
    let loaded_b2 = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { generation, track_id, .. } if track_id == "b" && *generation > gen_b)
    })
    .await;
    let gen_b2 = match loaded_b2 {
        SessionEvent::StreamLoaded { generation, .. } => generation,
        _ => unreachable!(),
    };
    assert!(gen_b2 > gen_b);

    let _ = upgrade_gate.send(Some(stream(
        "https://stream.test/b/old_hifi",
        "OPUS 274kbps",
    )));
    let after_replay = drain_events(&mut session.events).await;
    assert!(!after_replay
        .iter()
        .any(|e| matches!(e, SessionEvent::StreamUpgraded { stream, .. } if stream.url.contains("old_hifi"))));

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn normal_and_repeat_one_end_transitions() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 100), track("b", 120)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
    )
    .await;

    // Repeat One: natural end replays A on a newer generation.
    session
        .handle
        .send(SessionCommand::SetRepeat(RepeatMode::One))
        .await
        .unwrap();
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let gen1 = match recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await
    {
        SessionEvent::PlaybackStarted { generation, .. } => generation,
        _ => unreachable!(),
    };
    mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();

    let replayed = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::TrackTransition { generation, index: 0, track, .. } if track.id == "a" && *generation > gen1)
    })
    .await;
    assert!(matches!(
        replayed,
        SessionEvent::TrackTransition { index: 0, .. }
    ));
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
    )
    .await;

    // Repeat Off: A ends -> advances to B; B ends -> QueueFinished.
    session
        .handle
        .send(SessionCommand::SetRepeat(RepeatMode::Off))
        .await
        .unwrap();
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(98.5)).await.unwrap();
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();

    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::TrackTransition { index: 1, track, .. } if track.id == "b"),
    )
    .await;
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "b"),
    )
    .await;

    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(119.0)).await.unwrap();
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();

    let ended = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::TrackEnded { track_id, advancing: false, .. } if track_id == "b")
    })
    .await;
    assert!(matches!(
        ended,
        SessionEvent::TrackEnded {
            advancing: false,
            ..
        }
    ));
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::QueueFinished { .. })
    })
    .await;
    assert!(session.handle.progress().paused);

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn previous_history_behavior() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180), track("b", 180), track("c", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
    )
    .await;

    // A -> Next -> B (history: [0]) -> PlayIndex(2, push_history: true) -> C (history: [0, 1])
    session.handle.send(SessionCommand::Next).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "b"),
    )
    .await;

    session
        .handle
        .send(SessionCommand::PlayIndex {
            index: 2,
            push_history: true,
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "c"),
    )
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(12.0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(15)).await;

    // Desktop-style previous after 4s restarts C to 0s without popping history.
    session
        .handle
        .send(SessionCommand::Previous {
            restart_if_after_secs: Some(4.0),
        })
        .await
        .unwrap();
    let seeked = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    assert!(matches!(seeked, SessionEvent::Seeked { position_secs, .. } if position_secs == 0.0));

    // Now at 0s: Previous pops B from history.
    session
        .handle
        .send(SessionCommand::Previous {
            restart_if_after_secs: Some(4.0),
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::TrackTransition { index: 1, track, .. } if track.id == "b"),
    )
    .await;

    // TUI-style previous (no threshold) pops A from history.
    session
        .handle
        .send(SessionCommand::Previous {
            restart_if_after_secs: None,
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::TrackTransition { index: 0, track, .. } if track.id == "a"),
    )
    .await;

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn seek_clamping() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    driver.clear();

    session
        .handle
        .send(SessionCommand::SeekAbsolute(999.0))
        .await
        .unwrap();
    let ev = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    assert!(
        matches!(ev, SessionEvent::Seeked { position_secs, .. } if (position_secs - 180.0).abs() < 1e-6)
    );

    session
        .handle
        .send(SessionCommand::SeekAbsolute(-40.0))
        .await
        .unwrap();
    let ev = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    assert!(matches!(ev, SessionEvent::Seeked { position_secs, .. } if position_secs == 0.0));

    session
        .handle
        .send(SessionCommand::SeekRelative(-10.0))
        .await
        .unwrap();
    let ev = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    assert!(matches!(ev, SessionEvent::Seeked { position_secs, .. } if position_secs == 0.0));

    session
        .handle
        .send(SessionCommand::SeekAbsolute(175.0))
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    session
        .handle
        .send(SessionCommand::SeekRelative(25.0))
        .await
        .unwrap();
    let ev = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::Seeked { .. })
    })
    .await;
    assert!(
        matches!(ev, SessionEvent::Seeked { position_secs, .. } if (position_secs - 180.0).abs() < 1e-6)
    );

    let seeks: Vec<f64> = driver
        .snapshot()
        .into_iter()
        .filter_map(|op| match op {
            DriverOp::SeekAbsolute(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(seeks, vec![180.0, 0.0, 0.0, 175.0, 180.0]);

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn recovery_retry_budget() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    source.set_recovery(
        "a",
        Ok(stream("https://stream.test/a/retry1", "AAC 128kbps")),
    );
    source.set_recovery(
        "a",
        Ok(stream("https://stream.test/a/retry2", "AAC 128kbps")),
    );
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(42.0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(15)).await;

    // 1st error on gen 1: consumes recovery budget and resumes at ~42s.
    mpv_tx
        .send(MpvEvent::PlaybackError {
            error: "403 Forbidden".to_string(),
            file: Some("https://stream.test/a/base".to_string()),
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::RecoveryStarted { .. })
    })
    .await;
    let recovered = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { is_retry: true, .. })
    })
    .await;
    match recovered {
        SessionEvent::StreamLoaded { resume_at, .. } => {
            assert!((42.0..43.0).contains(&resume_at))
        }
        _ => unreachable!(),
    }
    assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 1);

    // 2nd error on same gen 1: budget exhausted, emits PlaybackError without calling recovery again.
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx
        .send(MpvEvent::PlaybackError {
            error: "403 again".to_string(),
            file: Some("https://stream.test/a/retry1".to_string()),
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::PlaybackError { error, .. } if error == "403 again"),
    )
    .await;
    assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 1);

    // Replay A -> fresh generation -> fresh recovery budget.
    session
        .handle
        .send(SessionCommand::PlayIndex {
            index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::StreamLoaded {
                is_retry: false,
                ..
            }
        )
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;
    mpv_tx
        .send(MpvEvent::PlaybackError {
            error: "network reset".to_string(),
            file: Some("https://stream.test/a/base".to_string()),
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { is_retry: true, .. })
    })
    .await;
    assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 2);

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn upgrade_swap_and_fallback() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let upgrade_gate = source.gate_upgrade("a");
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamLoaded { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::UpgradeStarted { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(30.0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(15)).await;

    // Pause before upgrade completes: upgrade must preserve paused=true.
    session
        .handle
        .send(SessionCommand::SetPaused(true))
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PausedChanged { paused: true, .. })
    })
    .await;
    driver.clear();

    upgrade_gate
        .send(Some(stream(
            "https://stream.test/a/hifi",
            "OPUS 274kbps · Extreme",
        )))
        .unwrap();
    let upgraded = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamUpgraded { .. })
    })
    .await;
    match upgraded {
        SessionEvent::StreamUpgraded {
            stream, resume_at, ..
        } => {
            assert_eq!(stream.url, "https://stream.test/a/hifi");
            assert!((resume_at - 30.0).abs() < 0.5);
        }
        _ => unreachable!(),
    }
    let ops = driver.snapshot();
    assert!(ops
        .iter()
        .any(|op| matches!(op, DriverOp::LoadAt(u, _) if u == "https://stream.test/a/hifi")));
    assert_eq!(ops.last(), Some(&DriverOp::SetPaused(true)));

    // Upgraded stream errors -> restores original base stream without consuming recovery budget.
    driver.clear();
    mpv_tx
        .send(MpvEvent::PlaybackError {
            error: "codec error".to_string(),
            file: Some("https://stream.test/a/hifi".to_string()),
        })
        .await
        .unwrap();
    let fallback = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::UpgradeFallback { .. })
    })
    .await;
    match fallback {
        SessionEvent::UpgradeFallback { stream, .. } => {
            assert_eq!(stream.url, "https://stream.test/a/base");
        }
        _ => unreachable!(),
    }
    assert!(driver
        .snapshot()
        .iter()
        .any(|op| matches!(op, DriverOp::LoadAt(u, _) if u == "https://stream.test/a/base")));
    assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 0);

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn precache_schedule_cancellation_adoption_and_failed_result() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let gate_b = source.gate_precache("b");
    let gate_c = source.gate_precache("c");
    let gate_d = source.gate_precache("d");
    let (mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 100), track("b", 100), track("d", 100)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
    )
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::PlaybackStarted { .. })
    })
    .await;

    // Outside 10s lead window -> no precache.
    mpv_tx.send(MpvEvent::Position(85.0)).await.unwrap();
    let quiet = drain_events(&mut session.events).await;
    assert!(!quiet
        .iter()
        .any(|e| matches!(e, SessionEvent::PrecacheUpdated { .. })));

    // Inside 10s lead window -> schedules B.
    mpv_tx.send(MpvEvent::Position(92.0)).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Scheduled,
                ..
            } if next_track_id == "b"
        )
    })
    .await;

    // Insert C next -> cancels B and schedules C!
    session
        .handle
        .send(SessionCommand::PlayTrackNext(track("c", 100)))
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Cancelled,
                ..
            } if next_track_id == "b"
        )
    })
    .await;
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Scheduled,
                ..
            } if next_track_id == "c"
        )
    })
    .await;
    drop(gate_b);

    // Track A ends while C's precache is still pending -> C starts fast and adopts pending precache.
    mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "c"),
    )
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::PlaybackStarted { track, .. } if track.id == "c"),
    )
    .await;

    // Now finish C's adopted precache with a Hi-Fi stream -> adopted & upgraded!
    gate_c
        .send(Some(stream("/cache/c.opus", "OPUS 274kbps · Cached")))
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Adopted,
                ..
            } if next_track_id == "c"
        )
    })
    .await;
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::StreamUpgraded { track_id, stream, .. } if track_id == "c" && stream.url == "/cache/c.opus")
    })
    .await;

    // Now test adopted precache returning None (failed result): queue is [a, c, b, d], remove b so next is d.
    session
        .handle
        .send(SessionCommand::RemoveFromQueue(2))
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::QueueChanged { .. })
    })
    .await;
    mpv_tx.send(MpvEvent::Position(92.0)).await.unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Scheduled,
                ..
            } if next_track_id == "d"
        )
    })
    .await;
    mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
    mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "d"),
    )
    .await;
    mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
    let _ = recv_until(
        &mut session.events,
        |e| matches!(e, SessionEvent::PlaybackStarted { track, .. } if track.id == "d"),
    )
    .await;
    gate_d.send(None).unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(
            e,
            SessionEvent::PrecacheUpdated {
                next_track_id,
                notice: PrecacheNotice::Failed,
                ..
            } if next_track_id == "d"
        )
    })
    .await;
    assert!(!session.handle.progress().paused);

    let _ = session.handle.shutdown().await;
}

#[tokio::test]
async fn shutdown_cancellation_and_bounded_join() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let _hung_fast = source.gate_fast("a");
    let (_mpv_tx, mpv_rx) = mpsc::channel(32);

    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    session
        .handle
        .send(SessionCommand::PlayQueue {
            tracks: vec![track("a", 180)],
            start_index: 0,
            push_history: false,
        })
        .await
        .unwrap();
    let _ = recv_until(&mut session.events, |e| {
        matches!(e, SessionEvent::TrackTransition { .. })
    })
    .await;
    tokio::time::sleep(Duration::from_millis(15)).await;

    let started = std::time::Instant::now();
    let report = session.handle.shutdown().await;
    assert!(report.tasks_joined_cleanly);
    assert!(started.elapsed() < Duration::from_millis(250));
    assert!(driver.shutdown_called.load(Ordering::SeqCst));
    assert!(source.dropped_tasks.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn event_ordering_under_simultaneous_mpv_end_and_error_signals() {
    // Case A: Natural TrackEnded immediately followed by trailing PlaybackError -> advances cleanly.
    {
        let driver = Arc::new(FakeAudioDriver::default());
        let source = Arc::new(FakeStreamSource::default());
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![track("a", 100), track("b", 100)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        let _ = recv_until(
            &mut session.events,
            |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
        )
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::PlaybackStarted { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Enqueue both TrackEnded and PlaybackError back-to-back.
        mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
        mpv_tx
            .send(MpvEvent::PlaybackError {
                error: "trailing eof error".to_string(),
                file: Some("https://stream.test/a/base".to_string()),
            })
            .await
            .unwrap();

        let _ = recv_until(
            &mut session.events,
            |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "b"),
        )
        .await;
        assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 0);
        let _ = session.handle.shutdown().await;
    }

    // Case B: Active PlaybackError immediately followed by TrackEnded -> recovers cleanly, no QueueFinished.
    {
        let driver = Arc::new(FakeAudioDriver::default());
        let source = Arc::new(FakeStreamSource::default());
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![track("a", 100)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        let _ = recv_until(
            &mut session.events,
            |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
        )
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::PlaybackStarted { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        mpv_tx
            .send(MpvEvent::PlaybackError {
                error: "connection reset".to_string(),
                file: Some("https://stream.test/a/base".to_string()),
            })
            .await
            .unwrap();
        mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();

        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { is_retry: true, .. })
        })
        .await;
        let rest = drain_events(&mut session.events).await;
        assert!(!rest
            .iter()
            .any(|e| matches!(e, SessionEvent::QueueFinished { .. })));
        assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 1);
        let _ = session.handle.shutdown().await;
    }

    // Case C: Premature EOF followed immediately by duplicate PlaybackError -> single recovery only.
    {
        let driver = Arc::new(FakeAudioDriver::default());
        let source = Arc::new(FakeStreamSource::default());
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            test_config(),
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![track("a", 180)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        let _ = recv_until(
            &mut session.events,
            |e| matches!(e, SessionEvent::StreamLoaded { track_id, .. } if track_id == "a"),
        )
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::PlaybackStarted { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::Position(25.0)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();
        mpv_tx
            .send(MpvEvent::PlaybackError {
                error: "duplicate network error".to_string(),
                file: Some("https://stream.test/a/base".to_string()),
            })
            .await
            .unwrap();

        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { is_retry: true, .. })
        })
        .await;
        assert_eq!(source.recovery_calls.load(Ordering::SeqCst), 1);
        let _ = session.handle.shutdown().await;
    }

    // Case D: Stale TrackEnded inside stale_error_grace window is ignored.
    {
        let mut cfg = test_config();
        cfg.stale_error_grace = Duration::from_secs(5);
        let driver = Arc::new(FakeAudioDriver::default());
        let source = Arc::new(FakeStreamSource::default());
        let (mpv_tx, mpv_rx) = mpsc::channel(32);
        let mut session = PlaybackSession::spawn(
            cfg,
            Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
            Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
            mpv_rx,
        );

        session
            .handle
            .send(SessionCommand::PlayQueue {
                tracks: vec![track("a", 100)],
                start_index: 0,
                push_history: false,
            })
            .await
            .unwrap();
        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::StreamLoaded { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::PlaybackStarted).await.unwrap();
        let _ = recv_until(&mut session.events, |e| {
            matches!(e, SessionEvent::PlaybackStarted { .. })
        })
        .await;
        mpv_tx.send(MpvEvent::Position(99.0)).await.unwrap();
        mpv_tx.send(MpvEvent::TrackEnded).await.unwrap();

        let rest = drain_events(&mut session.events).await;
        assert!(!rest.iter().any(|e| matches!(
            e,
            SessionEvent::TrackEnded { .. } | SessionEvent::QueueFinished { .. }
        )));
        let _ = session.handle.shutdown().await;
    }
}

/// 10. Idle `mpv --idle=yes` reporting `pause=false` must not flip an idle session to playing.
#[tokio::test]
async fn idle_mpv_unpaused_event_does_not_mark_session_playing() {
    let driver = Arc::new(FakeAudioDriver::default());
    let source = Arc::new(FakeStreamSource::default());
    let (mpv_tx, mpv_rx) = mpsc::channel(16);
    let mut session = PlaybackSession::spawn(
        test_config(),
        Arc::clone(&driver) as Arc<dyn PlaybackAudioDriver>,
        Arc::clone(&source) as Arc<dyn PlaybackStreamSource>,
        mpv_rx,
    );

    // Simulate mpv's initial `observe_property("pause")` report while idle.
    mpv_tx.send(MpvEvent::Paused(false)).await.unwrap();
    let drained = drain_events(&mut session.events).await;

    assert!(
        !drained
            .iter()
            .any(|e| matches!(e, SessionEvent::PausedChanged { paused: false, .. })),
        "idle session must not emit PausedChanged(false), got: {drained:?}"
    );
    let progress = session.handle.progress();
    assert!(progress.paused, "idle session must remain paused");
    assert_eq!(progress.position_secs, 0.0);
    assert!(progress.anchor_at.is_none());

    let _ = session.handle.shutdown().await;
}
