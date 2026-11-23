//! Instance-scoped characterization tests for Linux and Android resolver policies.
//!
//! 1. **Cache & Profile Policy**: Verifies cached-first fast resolution, cache-disabled
//!    short-circuiting, local-cache-before-upgrade delivery, and parallel multi-instance isolation
//!    with distinct `CorePaths` and `ResolverProfile` configurations in one test process.
//! 2. **Playback Generation Invariants**: Verifies same-track replay allocates a new
//!    `PlaybackGeneration`, cancels stale work, and rejects stale results.

use reqwest::Client;
use sonata_core::auth::ExtractedCookies;
use sonata_core::cache::CacheDb;
use sonata_core::paths::CorePaths;
use sonata_core::player::coordinator::{PlaybackCommand, PlaybackCoordinator, PlaybackTransition};
use sonata_core::player::{PlaybackGeneration, PlaybackState, PlaybackTaskSet};
use sonata_core::providers::hifi::{
    FastStartMode, ResolvedStream, ResolverProfile, StreamFallbackMode, StreamResolver,
    UpgradeDelivery,
};
use sonata_core::providers::ytmusic::YtMusicClient;
use sonata_core::providers::{ProviderSource, Track};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempCoreRoot {
    root: PathBuf,
    paths: CorePaths,
}

impl TempCoreRoot {
    fn new(label: &str) -> Self {
        let seq = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "sonata_p2_{label}_{}_{}_{}",
            std::process::id(),
            seq,
            unique
        ));
        let paths = CorePaths::for_app_roots(root.join("config"), root.join("cache"));
        fs::create_dir_all(&paths.config_dir).expect("create temp config dir");
        fs::create_dir_all(&paths.cache_dir).expect("create temp cache dir");
        Self { root, paths }
    }
}

impl Drop for TempCoreRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn linux_profile(cache_limit_bytes: u64, jiosaavn_enabled: bool) -> ResolverProfile {
    ResolverProfile {
        fast_start: FastStartMode::BoundedRange,
        upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
        fallback: StreamFallbackMode::ExternalTool,
        bootstrap_timeout: Duration::from_millis(2400),
        jiosaavn_enabled,
        cache_limit_bytes,
    }
}

fn android_profile(cache_limit_bytes: u64) -> ResolverProfile {
    ResolverProfile {
        fast_start: FastStartMode::DirectUrl,
        upgrade_delivery: UpgradeDelivery::ReturnRemoteStream,
        fallback: StreamFallbackMode::InnerTubeOnly,
        bootstrap_timeout: Duration::from_millis(8500),
        jiosaavn_enabled: false,
        cache_limit_bytes,
    }
}

fn make_resolver(paths: CorePaths, profile: ResolverProfile) -> StreamResolver {
    let http = Client::new();
    let cookies = ExtractedCookies::default();
    let ytm = YtMusicClient::new(http.clone(), cookies.clone());
    let cache_db = CacheDb::open_with_paths(&paths, profile.cache_enabled());
    StreamResolver::with_profile(http, ytm, cookies, cache_db, paths, profile)
}

fn sample_track(id: &str) -> Track {
    Track {
        id: id.to_string(),
        title: "Characterization Song".to_string(),
        artist: "Sonata Test Artist".to_string(),
        album: "Baseline".to_string(),
        duration_secs: Some(210),
        thumbnail_url: None,
        source: ProviderSource::YouTubeMusic,
        audio_badge: None,
        set_video_id: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    }
}

fn write_valid_audio_cache_entry(paths: &CorePaths, track_id: &str, badge: &str) -> PathBuf {
    let audio_dir = paths.audio_cache_dir();
    fs::create_dir_all(&audio_dir).expect("create audio cache dir");
    let audio_path = audio_dir.join(format!("{track_id}.v3.audio"));
    let badge_path = audio_dir.join(format!("{track_id}.v3.badge"));
    let mut payload = vec![0_u8; 16_384];
    payload[..4].copy_from_slice(b"OggS");
    fs::write(&audio_path, &payload).expect("write valid audio cache payload");
    fs::write(&badge_path, badge).expect("write audio cache badge");
    audio_path
}

#[tokio::test]
async fn cached_first_stream_resolution_works_for_both_linux_and_android_profiles() {
    for (label, profile) in [
        ("linux", linux_profile(2 * 1024 * 1024 * 1024, false)),
        ("android", android_profile(2 * 1024 * 1024 * 1024)),
    ] {
        let tmp = TempCoreRoot::new(label);
        let track = sample_track("cached_hit_01");
        let expected_path =
            write_valid_audio_cache_entry(&tmp.paths, &track.id, "OPUS 256kbps · Extreme · Cached");

        let resolver = make_resolver(tmp.paths.clone(), profile);
        let resolved = resolver
            .resolve_fast(&track)
            .await
            .expect("cached-first resolve_fast should succeed offline");
        assert_eq!(resolved.url, expected_path.to_string_lossy());
        assert_eq!(resolved.badge, "OPUS 256kbps · Extreme");
        assert!(resolved.headers.is_empty());
    }
}

#[tokio::test]
async fn cache_disabled_short_circuits_upgrade_and_precache() {
    let tmp = TempCoreRoot::new("cache_off");
    let track = sample_track("cache_off_01");
    let _ = write_valid_audio_cache_entry(&tmp.paths, &track.id, "OPUS 256kbps · Extreme");

    let resolver = make_resolver(tmp.paths.clone(), linux_profile(0, false));
    let current = ResolvedStream {
        url: "https://rr1.googlevideo.com/videoplayback?itag=140&range=0-1048575".to_string(),
        badge: "AAC 129kbps · Standard".to_string(),
        headers: Vec::new(),
    };

    assert!(!resolver.profile().cache_enabled());
    assert!(
        resolver
            .resolve_hifi_upgrade(&track, &current, true)
            .await
            .is_none(),
        "cache-disabled resolver must not attempt local-cache upgrades"
    );
    assert!(
        resolver.precache_hifi_audio(&track, true).await.is_none(),
        "cache-disabled resolver must not run next-track precache"
    );
}

#[tokio::test]
async fn local_cache_before_upgrade_delivers_higher_quality_and_skips_equal_quality() {
    let tmp = TempCoreRoot::new("local_upgrade");
    let track = sample_track("local_upgrade_01");
    let cached_path =
        write_valid_audio_cache_entry(&tmp.paths, &track.id, "OPUS 256kbps · Extreme");
    let resolver = make_resolver(
        tmp.paths.clone(),
        linux_profile(2 * 1024 * 1024 * 1024, false),
    );

    let low_current = ResolvedStream {
        url: "https://rr1.googlevideo.com/videoplayback?itag=140&range=0-1048575".to_string(),
        badge: "AAC 129kbps · Standard".to_string(),
        headers: Vec::new(),
    };
    let upgraded = resolver
        .resolve_hifi_upgrade(&track, &low_current, true)
        .await
        .expect("higher quality cached file should satisfy upgrade immediately");
    assert_eq!(upgraded.url, cached_path.to_string_lossy());
    assert_eq!(upgraded.badge, "OPUS 256kbps · Extreme");

    let already_extreme = ResolvedStream {
        url: cached_path.to_string_lossy().into_owned(),
        badge: "OPUS 256kbps · Extreme".to_string(),
        headers: Vec::new(),
    };
    assert!(
        resolver
            .resolve_hifi_upgrade(&track, &already_extreme, true)
            .await
            .is_none(),
        "equal-quality cached file must not trigger a redundant swap"
    );
}

#[tokio::test]
async fn precache_returns_existing_top_quality_cache_hit_offline() {
    let tmp = TempCoreRoot::new("precache_hit");
    let track = sample_track("precache_hit_01");
    let cached_path =
        write_valid_audio_cache_entry(&tmp.paths, &track.id, "OPUS 256kbps · Extreme");
    let resolver = make_resolver(tmp.paths.clone(), android_profile(2 * 1024 * 1024 * 1024));

    let hit = resolver
        .precache_hifi_audio(&track, true)
        .await
        .expect("existing extreme cached audio should return directly from precache");
    assert_eq!(hit.url, cached_path.to_string_lossy());
    assert_eq!(hit.badge, "OPUS 256kbps · Extreme");
}

#[tokio::test]
async fn parallel_instances_with_distinct_paths_and_profiles_do_not_interfere() {
    let linux_root = TempCoreRoot::new("coexist_linux");
    let android_root = TempCoreRoot::new("coexist_android");

    let track = sample_track("shared_track_id");
    let linux_audio =
        write_valid_audio_cache_entry(&linux_root.paths, &track.id, "OPUS 256kbps · Extreme");
    let android_audio =
        write_valid_audio_cache_entry(&android_root.paths, &track.id, "AAC 129kbps · Standard");

    let linux_cookies = ExtractedCookies::from_ytm_cookie_header("SAPISID=linux_sapi");
    let android_cookies = ExtractedCookies::from_ytm_cookie_header("SAPISID=android_sapi");
    linux_cookies
        .save_ytm_header(&linux_root.paths)
        .expect("save linux cookie");
    android_cookies
        .save_ytm_header(&android_root.paths)
        .expect("save android cookie");

    let linux_resolver = make_resolver(
        linux_root.paths.clone(),
        linux_profile(2 * 1024 * 1024 * 1024, true),
    );
    let android_resolver = make_resolver(
        android_root.paths.clone(),
        android_profile(1024 * 1024 * 1024),
    );

    let (linux_res, android_res) = tokio::join!(
        linux_resolver.resolve_fast(&track),
        android_resolver.resolve_fast(&track),
    );
    let linux_stream = linux_res.expect("linux resolve");
    let android_stream = android_res.expect("android resolve");

    assert_eq!(linux_stream.url, linux_audio.to_string_lossy());
    assert_eq!(linux_stream.badge, "OPUS 256kbps · Extreme");
    assert_eq!(android_stream.url, android_audio.to_string_lossy());
    assert_eq!(android_stream.badge, "AAC 129kbps · Standard");

    // Mutating one resolver's runtime profile must not affect the sibling instance.
    linux_resolver.update_profile(|p| p.jiosaavn_enabled = false);
    assert!(!linux_resolver.profile().jiosaavn_enabled);
    assert_eq!(
        android_resolver.profile().fallback,
        StreamFallbackMode::InnerTubeOnly
    );
    assert_eq!(
        linux_resolver.profile().fallback,
        StreamFallbackMode::ExternalTool
    );

    // Persisted cookie headers remain isolated per CorePaths root.
    let reloaded_linux =
        ExtractedCookies::load_saved_ytm_header(&linux_root.paths).expect("reload linux cookies");
    let reloaded_android = ExtractedCookies::load_saved_ytm_header(&android_root.paths)
        .expect("reload android cookies");
    assert_eq!(
        reloaded_linux.ytm_cookie_header().as_deref(),
        Some("SAPISID=linux_sapi")
    );
    assert_eq!(
        reloaded_android.ytm_cookie_header().as_deref(),
        Some("SAPISID=android_sapi")
    );
}

#[tokio::test]
async fn same_track_replay_creates_new_generation_and_rejects_stale_result() {
    let mut state = PlaybackState::new(100);
    let track = sample_track("replay_same_track_01");
    assert!(matches!(
        PlaybackCoordinator::apply(
            &mut state,
            PlaybackCommand::QueueChanged(vec![track.clone()])
        ),
        Some(PlaybackTransition::QueueChanged)
    ));

    let mut generation = PlaybackGeneration::default();
    let mut tasks = PlaybackTaskSet::default();

    // First play request for track A.
    let first_transition = PlaybackCoordinator::apply(
        &mut state,
        PlaybackCommand::Play {
            index: 0,
            push_history: false,
        },
    );
    assert!(matches!(
        first_transition,
        Some(PlaybackTransition::Play { ref track, .. }) if track.id == "replay_same_track_01"
    ));
    let gen1 = generation.advance();
    let mut gen1_task = tasks.spawn_result(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        (gen1, "replay_same_track_01", "stale-url")
    });

    // Replay the exact same track A before gen1 finishes.
    let second_transition = PlaybackCoordinator::apply(
        &mut state,
        PlaybackCommand::Play {
            index: 0,
            push_history: false,
        },
    );
    assert!(matches!(
        second_transition,
        Some(PlaybackTransition::Play { ref track, .. }) if track.id == "replay_same_track_01"
    ));
    tasks.restart();
    let gen2 = generation.advance();
    assert!(gen2 > gen1);

    let mut gen2_task =
        tasks.spawn_result(async move { (gen2, "replay_same_track_01", "fresh-url") });

    tokio::time::sleep(Duration::from_millis(15)).await;
    assert_eq!(gen1_task.try_take(), None);
    let fresh = gen2_task
        .try_take()
        .expect("current generation result should arrive");
    assert_eq!(fresh, (gen2, "replay_same_track_01", "fresh-url"));
    assert_ne!(fresh.0, gen1);

    assert!(tasks.shutdown(Duration::from_millis(100)).await);
}
