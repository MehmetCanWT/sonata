export type PlatformKind = "macos" | "windows" | "linux";
export type AppLanguage = "en" | "tr" | "ru";
export type ThemeModePref = "system" | "dark" | "light";
export type RepeatMode = "Off" | "All" | "One";
export type ProviderSource = "YouTubeMusic" | "Spotify";
export type ProviderMode = "both" | "you_tube_music" | "spotify";
export type ServerKind = "Jellyfin" | "Subsonic" | "Smb" | "WebDav";
export type StatsWindow = "Week" | "Month" | "AllTime";

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  album_id?: string | null;
  duration_secs?: number | null;
  thumbnail_url?: string | null;
  liked: boolean;
  set_video_id?: string | null;
  audio_badge?: string | null;
  is_explicit: boolean;
  source: ProviderSource;
}

export interface Playlist {
  id: string;
  name: string;
  track_count?: number | null;
  thumbnail_url?: string | null;
  is_owned: boolean;
  source: ProviderSource;
}

export interface PersonalPlaylist {
  id: string;
  name: string;
  tracks: Track[];
  created_at_epoch_secs: number;
  updated_at_epoch_secs: number;
}

export type HomeCardKind =
  | "Playlist"
  | "Album"
  | "Artist"
  | "Song"
  | "Video"
  | "Mood";

export interface HomeCard {
  id: string;
  title: string;
  subtitle: string;
  thumbnail_url?: string | null;
  kind?: HomeCardKind;
  is_playlist: boolean;
  is_album: boolean;
  is_artist: boolean;
  source: ProviderSource;
  browse_params?: string | null;
  track?: Track | null;
}

export interface HomeShelf {
  title: string;
  cards: HomeCard[];
  tracks: Track[];
}

export interface MoodChip {
  title: string;
  params: string;
}

export interface MoodCategoryTile {
  title: string;
  browse_id: string;
  params: string;
  color?: string | null;
}

export interface AlbumDetail {
  id: string;
  title: string;
  artist: string;
  artist_id?: string | null;
  year?: string | null;
  thumbnail_url?: string | null;
  tracks: Track[];
  description?: string | null;
  audio_playlist_id?: string | null;
  is_saved?: boolean;
}

export interface ArtistPage {
  id: string;
  name: string;
  description?: string | null;
  thumbnail_url?: string | null;
  banner_url?: string | null;
  subscribers?: string | null;
  top_tracks: Track[];
  songs_browse_id?: string | null;
  albums: HomeCard[];
  albums_browse_id?: string | null;
  singles: HomeCard[];
  singles_browse_id?: string | null;
  playlists: HomeCard[];
  related_artists: HomeCard[];
}

export interface RichSearchResult {
  top_result?: HomeCard | null;
  top_track?: Track | null;
  songs: Track[];
  albums: HomeCard[];
  artists: HomeCard[];
  playlists: HomeCard[];
}

export interface TimedWord {
  text: string;
  start_ms: number;
  end_ms: number;
}

export interface TimedLine {
  text: string;
  start_ms: number;
  end_ms: number;
  words: TimedWord[];
  speaker?: string | null;
  background?: boolean;
}

export type LyricsState =
  | { kind: "Empty" }
  | { kind: "Loading" }
  | { kind: "Synced"; lines: TimedLine[]; source?: string }
  | { kind: "Plain"; text: string; source?: string }
  | { kind: "Unavailable" };

export interface BrowserOption {
  id: string;
  label: string;
  detected: boolean;
}

export interface YtAccountProfile {
  name?: string | null;
  handle?: string | null;
  avatar_url?: string | null;
}

export interface DownloadedTrack {
  track_id: string;
  title: string;
  artist: string;
  album: string;
  duration_secs?: number | null;
  file_path: string;
  artwork_path?: string | null;
  badge: string;
  bytes: number;
}

export interface DownloadedCollection {
  collection_id: string;
  title: string;
  artwork_path?: string | null;
  track_ids: string[];
}

export interface ServerProfile {
  id: string;
  name: string;
  kind: ServerKind;
  base_url: string;
  username?: string | null;
  library_ids: string[];
  share?: string | null;
  domain?: string | null;
  local_mount_path?: string | null;
  enabled: boolean;
}

export interface ServerHealthReport {
  alive: boolean;
  latency_ms?: number | null;
  message: string;
}

export interface ServerSpeedTestReport {
  alive: boolean;
  latency_ms: number;
  download_mbps?: number | null;
  bytes_tested: number;
  sample_note: string;
}

export interface ScrobbleUiStatus {
  lastfm_enabled: boolean;
  lastfm_configured: boolean;
  lastfm_username: string;
  lastfm_api_key: string;
  listenbrainz_enabled: boolean;
  listenbrainz_configured: boolean;
  listenbrainz_username: string;
  last_error?: string | null;
}

export interface StatsSummary {
  total_plays: number;
  total_listening_secs: number;
  unique_tracks: number;
  unique_artists: number;
  top_tracks: Array<{ title: string; artist: string; play_count: number; total_secs: number }>;
  top_artists: Array<{ artist: string; play_count: number; total_secs: number }>;
}

export interface PairedDeviceInfo {
  device_id: string;
  device_name: string;
  paired_at_unix: number;
}

export interface PendingPairRequest {
  request_id: string;
  device_id: string;
  device_name: string;
  created_at_unix: number;
}

export interface UpdateCheckResult {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_url: string;
  notes?: string | null;
}

export type NavTargetDto =
  | { kind: "Home" }
  | { kind: "Explore" }
  | { kind: "Radio" }
  | { kind: "Library" }
  | { kind: "History" }
  | { kind: "Stats" }
  | { kind: "Search" }
  | { kind: "Queue" }
  | { kind: "Playlist"; id: string }
  | { kind: "Album"; id: string }
  | { kind: "Artist"; id: string }
  | { kind: "MoodCategory"; title: string; browse_id: string; params: string }
  | { kind: "ShelfDetail"; title: string }
  | { kind: "Settings" };

export interface DesktopConfigDto {
  language: AppLanguage;
  volume: number;
  shuffle: boolean;
  repeat_mode: RepeatMode;
  autoplay: boolean;
  provider_mode: ProviderMode;
  hifi_substitution: boolean;
  jiosaavn_enabled: boolean;
  cache_limit_gb: number;
  motion_covers: boolean;
  reduce_motion: boolean;
  preferred_browser?: string | null;
  desktop_ytm_browser?: string | null;
  downloads_dir?: string | null;
  downloads_limit_gb: number;
  discord_rpc_enabled: boolean;
  playback_speed: number;
  skip_silence: boolean;
  lyrics_translate: boolean;
  lyrics_romanize: boolean;
  video_quality: string;
  scrobble_servers_to_ytm: boolean;
  personal_only_mode: boolean;
  local_music_enabled: boolean;
  local_music_dirs: string[];
  gapless_enabled: boolean;
  crossfade_secs: number;
  connect_enabled: boolean;
  connect_device_name: string;
}

export interface DesktopSettingsDto {
  language: AppLanguage;
  theme_mode: ThemeModePref;
  adaptive_art_color: boolean;
  show_queue_panel: boolean;
  show_lyrics_panel: boolean;
  double_click_to_play: boolean;
  pinned_collections: Array<{
    id: string;
    title: string;
    subtitle: string;
    thumbnail_url?: string | null;
    is_album: boolean;
  }>;
}

export interface UiSnapshotDto {
  config: DesktopConfigDto;
  active_nav: NavTargetDto;
  status_line: string;
  available_browsers: BrowserOption[];
  signed_in_browser?: string | null;
  user_profile?: YtAccountProfile | null;
  user_avatar_path?: string | null;
  signing_in: boolean;
  sign_in_error?: string | null;
  ytm_playlists: Playlist[];
  library_albums: HomeCard[];
  library_artists: HomeCard[];
  library_extras_loading: boolean;
  home_chips: MoodChip[];
  active_home_chip?: MoodChip | null;
  home_shelves: HomeShelf[];
  home_loading: boolean;
  home_error?: string | null;
  explore_shelves: HomeShelf[];
  mood_tiles: MoodCategoryTile[];
  explore_loading: boolean;
  mood_page_title: string;
  mood_page_shelves: HomeShelf[];
  mood_page_loading: boolean;
  shelf_detail?: HomeShelf | null;
  radio_shelves: HomeShelf[];
  radio_loading: boolean;
  history_tracks: Track[];
  history_loading: boolean;
  last_played_tracks: Track[];
  recent_searches: string[];
  search_suggestions: string[];
  search_query: string;
  search_results: Track[];
  search_rich: RichSearchResult;
  search_loading: boolean;
  playlist_id?: string | null;
  playlist_heading: string;
  playlist_subtitle: string;
  playlist_cover_url?: string | null;
  playlist_editable: boolean;
  playlist_tracks: Track[];
  playlist_loading: boolean;
  album_detail?: AlbumDetail | null;
  album_loading: boolean;
  album_error?: string | null;
  artist_page?: ArtistPage | null;
  artist_banner_url?: string | null;
  artist_loading: boolean;
  artist_error?: string | null;
  liked_track_ids: string[];
  queue: Track[];
  current_index?: number | null;
  paused: boolean;
  muted: boolean;
  position_secs: number;
  duration_secs: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  autoplay: boolean;
  stream_badge: string;
  upgrading_quality: boolean;
  loading_track: boolean;
  playback_speed: number;
  skip_silence: boolean;
  sleep_timer_remaining_secs?: number | null;
  video_mode: boolean;
  video_loading: boolean;
  lyrics: LyricsState;
  translated_lyrics?: string[] | null;
  romanized_lyrics?: string[] | null;
  art_path?: string | null;
  ambient_rgb: [number, number, number];
  has_motion_cover: boolean;
  motion_cover_path?: string | null;
  spotify_notice?: string | null;
  update_info?: UpdateCheckResult | null;
  checking_updates: boolean;
  downloaded_tracks: DownloadedTrack[];
  downloaded_collections: DownloadedCollection[];
  downloaded_track_ids: string[];
  downloading_track_ids: string[];
  downloads_total_bytes: number;
  discord_status: string;
  server_profiles: ServerProfile[];
  server_statuses: Record<string, string>;
  server_health: Record<string, ServerHealthReport>;
  server_speed_tests: Record<string, ServerSpeedTestReport>;
  server_speed_testing: string[];
  active_server_id?: string | null;
  server_artists: HomeCard[];
  server_albums: HomeCard[];
  server_playlists: Playlist[];
  server_tracks: Track[];
  server_loading: boolean;
  local_tracks: Track[];
  local_scanning: boolean;
  personal_playlists: PersonalPlaylist[];
  scrobble_status: ScrobbleUiStatus;
  stats_window: StatsWindow;
  stats_summary: StatsSummary;
  stats_loading: boolean;
  connect_running: boolean;
  connect_http_port?: number | null;
  connect_pin?: string | null;
  connect_paired_devices: PairedDeviceInfo[];
  connect_pending_pair?: PendingPairRequest | null;
  connect_error?: string | null;
}

export interface ProgressTickDto {
  position_secs: number;
  duration_secs: number;
  paused: boolean;
  loading_track: boolean;
  playback_speed: number;
}

export type BackendStreamEvent =
  | { type: "Snapshot"; data: UiSnapshotDto }
  | { type: "Progress"; data: ProgressTickDto };

export type BackendCmdDto =
  | { type: "Navigate"; target: NavTargetDto }
  | {
      type: "OpenCollection";
      id: string;
      title: string;
      subtitle: string;
      thumbnail_url?: string | null;
      editable: boolean;
    }
  | { type: "OpenArtist"; id: string }
  | { type: "OpenArtistByName"; name: string }
  | {
      type: "OpenAlbum";
      album_id?: string | null;
      title: string;
      artist: string;
      thumbnail_url?: string | null;
    }
  | { type: "OpenMoodCategory"; title: string; browse_id: string; params: string }
  | { type: "OpenShelfDetail"; shelf: HomeShelf }
  | { type: "SelectHomeChip"; chip?: MoodChip | null }
  | { type: "RefreshActivePlaylist" }
  | { type: "RefreshHome" }
  | { type: "RefreshExplore" }
  | { type: "RefreshRadio" }
  | { type: "RefreshLibrary" }
  | { type: "RefreshHistory" }
  | { type: "StartTrackRadio"; track: Track }
  | { type: "StartQuickRadio" }
  | { type: "SignInWithBrowser"; browser_id: string }
  | { type: "ClearSignInError" }
  | { type: "SignOut" }
  | { type: "FetchSearchSuggestions"; query: string }
  | { type: "Search"; query: string }
  | { type: "PlayTracks"; tracks: Track[]; start_index: number; shuffle: boolean }
  | { type: "PlayQueueIndex"; index: number }
  | { type: "PlayTrackNext"; track: Track }
  | { type: "AddTrackToQueue"; track: Track }
  | { type: "RemoveFromQueue"; index: number }
  | { type: "MoveQueueItem"; from: number; to: number }
  | { type: "ClearQueue" }
  | { type: "ShuffleQueue" }
  | { type: "TogglePause" }
  | { type: "Stop" }
  | { type: "NextTrack" }
  | { type: "PrevTrack" }
  | { type: "SeekAbsolute"; secs: number }
  | { type: "SeekRelative"; secs: number }
  | { type: "SetVolume"; volume: number }
  | { type: "ToggleMute" }
  | { type: "ToggleShuffle" }
  | { type: "CycleRepeat" }
  | { type: "ToggleAutoplay" }
  | { type: "ToggleHiFi" }
  | { type: "ToggleJioSaavn" }
  | { type: "TogglePersonalOnlyMode" }
  | { type: "ToggleLocalMusicEnabled" }
  | { type: "AddLocalMusicDir"; path: string }
  | { type: "RemoveLocalMusicDir"; path: string }
  | { type: "ScanLocalMusic" }
  | { type: "ToggleAttributePersonalToYtm" }
  | { type: "StepCacheLimit"; up: boolean }
  | { type: "ToggleMotionCovers" }
  | { type: "ToggleReduceMotion" }
  | { type: "SetLanguage"; language: AppLanguage }
  | { type: "SetPlaybackSpeed"; speed: number }
  | { type: "ToggleSkipSilence" }
  | { type: "SetSleepTimer"; minutes?: number | null }
  | { type: "ToggleLyricsTranslate" }
  | { type: "ToggleLyricsRomanize" }
  | { type: "ToggleLike"; track: Track }
  | { type: "DislikeTrack"; track: Track }
  | { type: "ToggleSaveAlbum" }
  | {
      type: "ToggleSavePlaylist";
      id: string;
      title: string;
      thumbnail_url?: string | null;
      save: boolean;
    }
  | { type: "CreateYtmPlaylist"; name: string }
  | { type: "CreatePersonalPlaylist"; name: string }
  | { type: "DeletePersonalPlaylist"; id: string }
  | { type: "EditPlaylist"; playlist_id: string; title?: string | null }
  | { type: "AddTrackToPlaylist"; track: Track; playlist_id: string; playlist_name: string }
  | { type: "AddTrackToPersonalPlaylist"; track: Track; playlist_id: string }
  | { type: "RemoveTrackFromPlaylist"; index: number }
  | { type: "CheckForUpdates" }
  | { type: "DownloadTrack"; track: Track; collection_id?: string | null; collection_title?: string | null }
  | { type: "RemoveDownloadedTrack"; track_id: string }
  | { type: "StepDownloadsLimit"; up: boolean }
  | { type: "ClearAllDownloads" }
  | { type: "ToggleDiscordRpc" }
  | { type: "RefreshServers" }
  | {
      type: "SaveServerProfile";
      id?: string | null;
      name: string;
      kind: ServerKind;
      base_url: string;
      username?: string | null;
      password?: string | null;
      api_key?: string | null;
      bearer_token?: string | null;
    }
  | { type: "ProbeServerHealth"; id: string }
  | { type: "RunServerSpeedTest"; id: string }
  | { type: "ToggleServerEnabled"; id: string }
  | { type: "RemoveServerProfile"; id: string }
  | { type: "SelectServer"; server_id: string }
  | { type: "ToggleGapless" }
  | { type: "SetCrossfade"; secs: number }
  | {
      type: "SaveScrobbleConfig";
      lastfm_enabled: boolean;
      lastfm_api_key: string;
      lastfm_api_secret?: string | null;
      lastfm_session_key?: string | null;
      lastfm_username: string;
      listenbrainz_enabled: boolean;
      listenbrainz_token?: string | null;
      listenbrainz_username: string;
    }
  | { type: "ClearScrobbleBackend"; lastfm: boolean; listenbrainz: boolean }
  | { type: "LoadListeningStats"; window: StatsWindow }
  | { type: "ToggleConnect" }
  | { type: "SetConnectDeviceName"; name: string }
  | { type: "RegenerateConnectPin" }
  | { type: "UnpairConnectDevice"; device_id: string }
  | { type: "ApproveConnectPair"; request_id: string }
  | { type: "RejectConnectPair"; request_id: string };
