//! Integration tests proving cancellable async futures on `sonata-android` drop cleanly and
//! never deadlock engine state or deliver stale results after cancellation.
//!
//! 1. **UniFFI Future Cancellation**: Drives `uniffi::rust_future_new` / `rust_future_cancel` /
//!    `rust_future_free` (the exact FFI lifecycle used by Kotlin `uniffiRustCallAsync`) and proves
//!    cancelling drops the Rust future immediately and blocks result delivery.
//! 2. **Engine Async Drop Safety**: Cancels in-flight `SonataAndroidEngine` async calls via
//!    `tokio::time::timeout` and verifies no `RwLock` is held across `.await` and subsequent
//!    engine calls succeed immediately.

use sonata_android::{AndroidTrack, SonataAndroidEngine};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct DropGuard {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

fn temp_engine_dirs(label: &str) -> (std::path::PathBuf, String, String) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let base = std::env::temp_dir().join(format!(
        "sonata_android_cancel_{label}_{}_{}",
        std::process::id(),
        nanos
    ));
    let cfg = base.join("config");
    let cache = base.join("cache");
    (
        base,
        cfg.to_string_lossy().into_owned(),
        cache.to_string_lossy().into_owned(),
    )
}

#[test]
fn uniffi_rust_future_cancel_and_free_drops_inner_future_without_delivering_result() {
    extern "C" fn noop_continuation(_data: u64, _poll_result: uniffi::RustFuturePoll) {}

    let started = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicBool::new(false));

    let started_clone = Arc::clone(&started);
    let dropped_clone = Arc::clone(&dropped);
    let completed_clone = Arc::clone(&completed);
    let future = async move {
        let _guard = DropGuard {
            dropped: dropped_clone,
        };
        started_clone.store(true, Ordering::SeqCst);
        std::future::pending::<()>().await;
        completed_clone.store(true, Ordering::SeqCst);
        Ok::<String, uniffi::LiftArgsError>("stale".to_string())
    };

    // Mirrors the generated UniFFI scaffolding lifecycle invoked by Kotlin's `uniffiRustCallAsync`.
    let handle = uniffi::rust_future_new::<_, String, sonata_android::UniFfiTag>(
        future,
        sonata_android::UniFfiTag,
    );

    // SAFETY: `handle` was freshly allocated by `rust_future_new` for `String` (whose UniFFI
    // FFI return type is `RustBuffer`) with `sonata_android::UniFfiTag`. It is polled once so the
    // future begins running and registers its drop guard, then cancelled and freed once.
    unsafe {
        uniffi::rust_future_poll::<uniffi::RustBuffer, sonata_android::UniFfiTag>(
            handle,
            noop_continuation,
            0,
        );
    }
    assert!(
        started.load(Ordering::SeqCst),
        "first poll must advance the Rust future into its suspended state"
    );
    assert!(
        !dropped.load(Ordering::SeqCst),
        "future should remain alive while pending"
    );

    // SAFETY: `handle` is still live and owned by this test; cancelling and freeing transfers
    // ownership back to UniFFI and drops the underlying future.
    unsafe {
        uniffi::rust_future_cancel::<uniffi::RustBuffer, sonata_android::UniFfiTag>(handle);
        uniffi::rust_future_free::<uniffi::RustBuffer, sonata_android::UniFfiTag>(handle);
    }

    assert!(
        dropped.load(Ordering::SeqCst),
        "cancelling and freeing the UniFFI handle must drop the inner Rust future immediately"
    );
    assert!(
        !completed.load(Ordering::SeqCst),
        "cancelled future must never deliver a completion result"
    );
}

#[tokio::test]
async fn cancelling_inflight_engine_calls_drops_futures_and_keeps_engine_responsive() {
    let (base, cfg, cache) = temp_engine_dirs("engine_drop");
    let engine = Arc::new(SonataAndroidEngine::new(cfg, cache).expect("engine should initialize"));

    let track = AndroidTrack {
        id: "cancel_probe_track".to_string(),
        title: "Cancel Probe".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        duration_secs: Some(210),
        thumbnail_url: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    };

    let search_dropped = Arc::new(AtomicBool::new(false));
    let search_flag = Arc::clone(&search_dropped);
    let engine_for_search = Arc::clone(&engine);
    let search_handle = tokio::spawn(async move {
        let search_fut = async move {
            struct Guard(Arc<AtomicBool>);
            impl Drop for Guard {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }
            let _guard = Guard(search_flag);
            let _ = engine_for_search
                .search_songs("cancellation test".to_string())
                .await;
        };
        let _ = tokio::time::timeout(Duration::from_millis(1), search_fut).await;
    });
    search_handle.await.expect("search task should finish");
    assert!(
        search_dropped.load(Ordering::SeqCst),
        "timed-out/cancelled search future must be dropped"
    );

    let stream_res = tokio::time::timeout(
        Duration::from_millis(1),
        engine.resolve_stream(track.clone()),
    )
    .await;
    let lyrics_res =
        tokio::time::timeout(Duration::from_millis(1), engine.fetch_lyrics(track)).await;
    let _ = (stream_res, lyrics_res);

    // Engine locks must remain completely uncontended and responsive after cancellations.
    assert!(!engine.is_signed_in());
    assert!(engine.get_cached_account_profile().await.is_none());
    assert!(engine.get_cached_library_playlists().await.is_empty());
    engine.clear_lyrics_cache().await;
    engine.sign_out().await;

    let _ = std::fs::remove_dir_all(&base);
}
