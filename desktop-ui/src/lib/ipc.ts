import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  BackendCmdDto,
  BackendStreamEvent,
  DesktopSettingsDto,
  PlatformKind,
  UiSnapshotDto,
} from "@/types/sonata";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && typeof window.__TAURI_INTERNALS__ !== "undefined";
}

export function resolveArtworkSrc(urlOrPath?: string | null): string | undefined {
  if (!urlOrPath) return undefined;
  const trimmed = urlOrPath.trim();
  if (!trimmed) return undefined;
  if (
    trimmed.startsWith("https://") ||
    trimmed.startsWith("data:") ||
    trimmed.startsWith("asset://") ||
    trimmed.startsWith("http://asset.localhost/")
  ) {
    return trimmed;
  }
  if (trimmed.startsWith("http://")) {
    return undefined;
  }
  if (isTauriRuntime()) {
    try {
      return convertFileSrc(trimmed);
    } catch {
      return undefined;
    }
  }
  return trimmed;
}

export async function subscribeBackendState(
  onEvent: (ev: BackendStreamEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    onEvent({ type: "Snapshot", data: createEmptySnapshot() });
    return () => {};
  }
  const channel = new Channel<BackendStreamEvent>();
  channel.onmessage = onEvent;
  await invoke("subscribe_state", { onEvent: channel });
  return () => {
    channel.onmessage = () => {};
  };
}

export async function dispatchBackendCmd(cmd: BackendCmdDto): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  await invoke("dispatch_cmd", { cmd });
}

export async function fetchDesktopSettings(): Promise<DesktopSettingsDto> {
  if (!isTauriRuntime()) {
    return {
      language: "en",
      theme_mode: "dark",
      adaptive_art_color: true,
      show_queue_panel: true,
      show_lyrics_panel: false,
      double_click_to_play: false,
      pinned_collections: [],
    };
  }
  return invoke<DesktopSettingsDto>("get_desktop_settings");
}

export async function saveDesktopSettings(settings: DesktopSettingsDto): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  await invoke("update_desktop_settings", { settings });
}

export async function fetchPlatform(): Promise<PlatformKind> {
  if (!isTauriRuntime()) {
    return "linux";
  }
  return invoke<PlatformKind>("get_platform");
}

export async function fetchShowWindowControls(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("should_show_window_controls");
}

export async function windowMinimize(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_minimize");
}

export async function windowToggleMaximize(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_toggle_maximize");
}

export async function windowClose(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_close");
}

export async function windowStartDragging(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_start_dragging");
}

export async function windowToggleFullscreen(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  return invoke<boolean>("window_toggle_fullscreen");
}

export function createEmptySnapshot(): UiSnapshotDto {
  return {
    config: {
      language: "en",
      volume: 85,
      shuffle: false,
      repeat_mode: "Off",
      autoplay: true,
      provider_mode: "both",
      hifi_substitution: true,
      jiosaavn_enabled: false,
      cache_limit_gb: 2,
      motion_covers: true,
      reduce_motion: false,
      downloads_limit_gb: 10,
      discord_rpc_enabled: true,
      playback_speed: 1.0,
      skip_silence: false,
      lyrics_translate: false,
      lyrics_romanize: false,
      video_quality: "Auto",
      scrobble_servers_to_ytm: false,
      personal_only_mode: false,
      local_music_enabled: true,
      local_music_dirs: [],
      gapless_enabled: true,
      crossfade_secs: 0,
      connect_enabled: false,
      connect_device_name: "Sonata Desktop",
    },
    active_nav: { kind: "Home" },
    status_line: "",
    available_browsers: [],
    signing_in: false,
    ytm_playlists: [],
    library_albums: [],
    library_artists: [],
    library_extras_loading: false,
    home_chips: [],
    home_shelves: [],
    home_loading: false,
    explore_shelves: [],
    mood_tiles: [],
    explore_loading: false,
    mood_page_title: "",
    mood_page_shelves: [],
    mood_page_loading: false,
    radio_shelves: [],
    radio_loading: false,
    history_tracks: [],
    history_loading: false,
    last_played_tracks: [],
    recent_searches: [],
    search_suggestions: [],
    search_query: "",
    search_results: [],
    search_rich: { songs: [], albums: [], artists: [], playlists: [] },
    search_loading: false,
    playlist_heading: "",
    playlist_subtitle: "",
    playlist_editable: false,
    playlist_tracks: [],
    playlist_loading: false,
    album_detail: undefined,
    album_loading: false,
    artist_loading: false,
    liked_track_ids: [],
    queue: [],
    current_index: undefined,
    paused: true,
    muted: false,
    position_secs: 0,
    duration_secs: 0,
    volume: 85,
    shuffle: false,
    repeat: "Off",
    autoplay: true,
    stream_badge: "",
    upgrading_quality: false,
    loading_track: false,
    playback_speed: 1.0,
    skip_silence: false,
    video_mode: false,
    video_loading: false,
    lyrics: { kind: "Empty" },
    ambient_rgb: [28, 28, 34],
    has_motion_cover: false,
    motion_cover_path: null,
    checking_updates: false,
    downloaded_tracks: [],
    downloaded_collections: [],
    downloaded_track_ids: [],
    downloading_track_ids: [],
    downloads_total_bytes: 0,
    discord_status: "",
    server_profiles: [],
    server_statuses: {},
    server_health: {},
    server_speed_tests: {},
    server_speed_testing: [],
    server_artists: [],
    server_albums: [],
    server_playlists: [],
    server_tracks: [],
    server_loading: false,
    local_tracks: [],
    local_scanning: false,
    personal_playlists: [],
    scrobble_status: {
      lastfm_enabled: false,
      lastfm_configured: false,
      lastfm_username: "",
      lastfm_api_key: "",
      listenbrainz_enabled: false,
      listenbrainz_configured: false,
      listenbrainz_username: "",
    },
    stats_window: "Week",
    stats_summary: {
      total_plays: 0,
      total_listening_secs: 0,
      unique_tracks: 0,
      unique_artists: 0,
      top_tracks: [],
      top_artists: [],
    },
    stats_loading: false,
    connect_running: false,
    connect_paired_devices: [],
  };
}
