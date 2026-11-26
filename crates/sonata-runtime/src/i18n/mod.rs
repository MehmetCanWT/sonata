//! Shared English, Turkish, and Russian UI localization catalog for Linux Sonata clients.
//!
//! 1. **Language Preference (`AppLanguage`)**: Serializable `en` / `tr` / `ru` selector defaulting to English (`En`) for Desktop and TUI.
//! 2. **Locale Modules (`en`, `tr`, `ru`)**: Modular per-language string tables (`EN`, `TR`, `RU`) enforcing complete field parity at compile time.
//! 3. **Localization Helpers**: `strings()`, `localize_mood_title()`, and `localize_shelf_title()`.

mod en;
mod ru;
mod tr;

pub use en::EN;
pub use ru::RU;
pub use tr::TR;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Supported UI languages for Sonata Desktop and TUI (defaults to English).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    #[default]
    En,
    Tr,
    Ru,
}

impl AppLanguage {
    /// Returns the native display label for this language.
    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Tr => "Türkçe",
            Self::Ru => "Русский",
        }
    }

    /// Returns the BCP-47 language code.
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Tr => "tr",
            Self::Ru => "ru",
        }
    }

    /// Cycles forward through supported languages.
    pub fn next(self) -> Self {
        match self {
            Self::En => Self::Tr,
            Self::Tr => Self::Ru,
            Self::Ru => Self::En,
        }
    }

    /// Cycles backward through supported languages.
    pub fn prev(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Tr => Self::En,
            Self::Ru => Self::Tr,
        }
    }
}

/// Returns the static UI translation catalog for the requested language.
pub fn strings(lang: AppLanguage) -> &'static UiStrings {
    match lang {
        AppLanguage::En => &EN,
        AppLanguage::Tr => &TR,
        AppLanguage::Ru => &RU,
    }
}

/// Compile-time checked UI strings shared across Sonata Desktop and TUI.
#[derive(Debug, Clone, Copy)]
pub struct UiStrings {
    // Navigation & Sections
    pub home: &'static str,
    pub explore: &'static str,
    pub library: &'static str,
    pub search: &'static str,
    pub queue: &'static str,
    pub history: &'static str,
    pub settings: &'static str,
    pub playlists: &'static str,
    pub liked_music: &'static str,
    pub favorites: &'static str,
    pub tracks: &'static str,
    pub songs: &'static str,
    pub artists: &'static str,
    pub albums: &'static str,
    pub next_up: &'static str,
    pub lyrics: &'static str,
    pub cover: &'static str,
    pub album: &'static str,
    pub playlist: &'static str,
    pub new_playlist: &'static str,
    pub show_all: &'static str,
    pub other_versions: &'static str,
    pub suggestions: &'static str,

    // Playback & Actions
    pub play: &'static str,
    pub pause: &'static str,
    pub shuffle: &'static str,
    pub repeat: &'static str,
    pub radio: &'static str,
    pub like: &'static str,
    pub remove_like: &'static str,
    pub dislike: &'static str,
    pub save_to_library: &'static str,
    pub saved: &'static str,
    pub refresh: &'static str,
    pub back: &'static str,
    pub forward: &'static str,
    pub minimize: &'static str,
    pub fullscreen: &'static str,
    pub cancel: &'static str,
    pub create: &'static str,
    pub close: &'static str,
    pub open_folder: &'static str,
    pub check_updates: &'static str,
    pub sign_in: &'static str,
    pub sign_out: &'static str,
    pub signed_in: &'static str,
    pub not_signed_in: &'static str,

    // Search & Empty / Loading States
    pub search_placeholder: &'static str,
    pub nothing_playing: &'static str,
    pub pick_track_hint: &'static str,
    pub no_track_playing: &'static str,
    pub loading_cover: &'static str,
    pub select_song_lyrics: &'static str,
    pub fetching_lyrics: &'static str,
    pub no_lyrics_found: &'static str,
    pub queue_empty: &'static str,
    pub playlist_empty: &'static str,
    pub upgrading: &'static str,
    pub low_power_mini: &'static str,
    pub synced_lyrics: &'static str,

    // Tooltips & Transport
    pub previous_track: &'static str,
    pub next_track: &'static str,
    pub shuffle_on: &'static str,
    pub shuffle_off: &'static str,
    pub repeat_off: &'static str,
    pub repeat_all: &'static str,
    pub repeat_one: &'static str,
    pub mute: &'static str,
    pub next_up_queue: &'static str,
    pub lyrics_panel: &'static str,
    pub now_playing_tip: &'static str,

    // Desktop Settings Tabs & Language
    pub tab_appearance: &'static str,
    pub tab_playback: &'static str,
    pub tab_library: &'static str,
    pub tab_system: &'static str,
    pub tab_account: &'static str,
    pub tab_about: &'static str,
    pub language: &'static str,
    pub language_desc: &'static str,
    pub theme_mode: &'static str,
    pub theme_mode_desc: &'static str,
    pub mode_system: &'static str,
    pub mode_dark: &'static str,
    pub mode_light: &'static str,
    pub color_theme: &'static str,
    pub color_theme_desc: &'static str,
    pub reload_themes: &'static str,
    pub transparency: &'static str,
    pub transparency_desc: &'static str,
    pub blur: &'static str,
    pub blur_desc: &'static str,
    pub card_outline: &'static str,
    pub card_outline_desc: &'static str,
    pub corners: &'static str,
    pub corners_desc: &'static str,
    pub corner_round: &'static str,
    pub corner_soft: &'static str,
    pub corner_sharp: &'static str,
    pub edge_fades: &'static str,
    pub edge_fades_desc: &'static str,
    pub adaptive_art_color: &'static str,
    pub adaptive_art_color_desc: &'static str,
    pub hifi_upgrade: &'static str,
    pub hifi_upgrade_desc: &'static str,
    pub jiosaavn_module: &'static str,
    pub jiosaavn_module_desc: &'static str,
    pub motion_covers: &'static str,
    pub motion_covers_desc: &'static str,
    pub reduce_motion: &'static str,
    pub reduce_motion_desc: &'static str,
    pub cache_limit: &'static str,
    pub cache_limit_desc: &'static str,

    // TUI Specific Labels & Modals
    pub tui_album_art: &'static str,
    pub tui_album_art_fallback: &'static str,
    pub tui_motion_cover: &'static str,
    pub tui_lyrics_title: &'static str,
    pub tui_settings_title: &'static str,
    pub tui_sec_audio: &'static str,
    pub tui_sec_appearance: &'static str,
    pub tui_sec_storage: &'static str,
    pub tui_providers: &'static str,
    pub tui_providers_desc: &'static str,
    pub tui_transparent_bg: &'static str,
    pub tui_transparent_bg_desc: &'static str,
    pub tui_hypr_opacity: &'static str,
    pub tui_hypr_opacity_desc: &'static str,
    pub tui_browser_session: &'static str,
    pub tui_browser_session_desc: &'static str,
    pub tui_version_updates: &'static str,
    pub tui_version_updates_desc: &'static str,
    pub enabled: &'static str,
    pub disabled: &'static str,
    pub enabled_320k: &'static str,
    pub static_label: &'static str,
    pub enabled_glass: &'static str,
    pub solid_label: &'static str,
    pub up_to_date: &'static str,
    pub check_update_hint: &'static str,
    pub tui_help_title: &'static str,
    pub tui_themes_title: &'static str,
    pub tui_import_spotify_title: &'static str,
    pub tui_create_playlist_title: &'static str,
    pub tui_add_to_playlist_title: &'static str,
    pub autoplay: &'static str,
    pub autoplay_on: &'static str,
    pub autoplay_off: &'static str,
    pub autoplay_desc: &'static str,
    pub radio_mixes: &'static str,
    pub quick_radio: &'static str,
    pub moods_genres: &'static str,
    pub recent_searches: &'static str,
    pub library_playlists: &'static str,
    pub recently_played: &'static str,
    pub start_radio: &'static str,
    pub play_next: &'static str,
    pub add_to_queue: &'static str,
    pub go_to_album: &'static str,
    pub go_to_artist: &'static str,
    pub add_to_playlist: &'static str,
    pub remove_from_playlist: &'static str,
    pub play_now: &'static str,
    pub top_songs: &'static str,
    pub releases: &'static str,
    pub fans_might_like: &'static str,
    pub you_liked: &'static str,
    pub retry: &'static str,
    pub all_tab: &'static str,
    pub videos_tab: &'static str,
    pub youtube_music: &'static str,
    pub your_library: &'static str,
    pub sign_in_library_title: &'static str,
    pub sign_in_library_desc: &'static str,
    pub sign_in_browser: &'static str,
    pub dbl_click_play: &'static str,
    pub dbl_click_play_desc: &'static str,
    pub cache_budget: &'static str,
    pub cache_budget_desc: &'static str,
    pub pinned: &'static str,
    pub pin: &'static str,
    pub unpin: &'static str,
    pub find_your_mood: &'static str,
    pub in_library: &'static str,
    pub add_to_library: &'static str,
    pub remove_from_library: &'static str,

    // Offline Downloads, Discord Rich Presence & Self-Hosted Servers
    pub downloads: &'static str,
    pub download: &'static str,
    pub downloaded: &'static str,
    pub downloading: &'static str,
    pub remove_download: &'static str,
    pub downloads_dir: &'static str,
    pub downloads_dir_desc: &'static str,
    pub downloads_limit: &'static str,
    pub downloads_limit_desc: &'static str,
    pub downloads_usage: &'static str,
    pub clear_downloads: &'static str,
    pub no_downloads: &'static str,
    pub discord_rpc: &'static str,
    pub discord_rpc_desc: &'static str,
    pub discord_app_id: &'static str,
    pub discord_app_id_desc: &'static str,
    pub servers: &'static str,
    pub servers_desc: &'static str,
    pub add_server: &'static str,
    pub test_connection: &'static str,
    pub remove_server: &'static str,
    pub no_servers: &'static str,
    pub connect_server_hint: &'static str,
    pub unlimited: &'static str,
    pub reset_default: &'static str,
    pub save: &'static str,

    // Playback Options, Lyrics Transforms & Music Video Mode
    pub music: &'static str,
    pub video: &'static str,
    pub video_quality: &'static str,
    pub video_quality_desc: &'static str,
    pub translate_lyrics: &'static str,
    pub translate_lyrics_desc: &'static str,
    pub romanize_lyrics: &'static str,
    pub romanize_lyrics_desc: &'static str,
    pub playback_speed: &'static str,
    pub playback_speed_desc: &'static str,
    pub skip_silence: &'static str,
    pub skip_silence_desc: &'static str,
    pub sleep_timer: &'static str,
    pub sleep_timer_desc: &'static str,
    pub off: &'static str,
    pub auto_label: &'static str,
    pub loading_video: &'static str,
    pub video_unavailable: &'static str,

    // Sources, Modules, Personal Only Mode & Personal Playlists
    pub sources: &'static str,
    pub sources_desc: &'static str,
    pub modules: &'static str,
    pub modules_desc: &'static str,
    pub personal_only_mode: &'static str,
    pub personal_only_mode_desc: &'static str,
    pub local_music: &'static str,
    pub local_music_desc: &'static str,
    pub local_music_dirs: &'static str,
    pub add_folder: &'static str,
    pub smb: &'static str,
    pub webdav: &'static str,
    pub jellyfin: &'static str,
    pub subsonic: &'static str,
    pub server_alive: &'static str,
    pub server_online: &'static str,
    pub server_offline: &'static str,
    pub speed_test: &'static str,
    pub latency: &'static str,
    pub latency_ms: &'static str,
    pub throughput: &'static str,
    pub throughput_mbps: &'static str,
    pub personal_playlists: &'static str,
    pub personal_playlists_desc: &'static str,
    pub attribute_personal_to_ytm: &'static str,
    pub attribute_personal_to_ytm_desc: &'static str,
    pub scan: &'static str,
    pub folders: &'static str,
    pub youtube: &'static str,
    pub local: &'static str,
    pub edit: &'static str,
    pub rename: &'static str,
    pub delete: &'static str,
    pub smb_share: &'static str,
    pub smb_domain: &'static str,
    pub smb_mount_path: &'static str,
    pub personal_only_active: &'static str,

    // Gapless, Crossfade, Scrobbling, Listening Stats & Sonata Connect
    pub gapless_playback: &'static str,
    pub gapless_playback_desc: &'static str,
    pub crossfade: &'static str,
    pub crossfade_desc: &'static str,
    pub lastfm_scrobbling: &'static str,
    pub lastfm_scrobbling_desc: &'static str,
    pub listenbrainz_scrobbling: &'static str,
    pub listenbrainz_scrobbling_desc: &'static str,
    pub listening_stats: &'static str,
    pub listening_stats_desc: &'static str,
    pub total_plays: &'static str,
    pub listening_time: &'static str,
    pub streak_days: &'static str,
    pub top_artists: &'static str,
    pub top_tracks: &'static str,
    pub top_albums: &'static str,
    pub sonata_connect: &'static str,
    pub sonata_connect_desc: &'static str,
    pub connect_device_name: &'static str,
    pub pairing_pin: &'static str,
    pub paired_devices: &'static str,
    pub stats_week: &'static str,
    pub stats_month: &'static str,
    pub stats_year: &'static str,
    pub stats_all_time: &'static str,
    pub regenerate_pin: &'static str,
    pub unpair: &'static str,
    pub no_stats_yet: &'static str,
    pub no_paired_devices: &'static str,
    pub scrobbling: &'static str,
    pub col_title: &'static str,
    pub col_artist: &'static str,
    pub col_time: &'static str,
    pub theme_label: &'static str,
    pub folder_label: &'static str,
    pub skip_label: &'static str,
    pub sync_lyrics: &'static str,
    pub keys_label: &'static str,
    pub update_label: &'static str,

    // Interactive Pairing, Tray, Discord Status & Runtime/TUI Labels
    pub approve: &'static str,
    pub reject: &'static str,
    pub status_label: &'static str,
    pub error_label: &'static str,
    pub default_label: &'static str,
    pub incomplete: &'static str,
    pub scanning: &'static str,
    pub none_enter_to_add: &'static str,
    pub configured_enter: &'static str,
    pub provider_both: &'static str,
    pub provider_ytm_only: &'static str,
    pub provider_spotify_only: &'static str,
    pub cache_off_not_recommended: &'static str,
    pub tray_ready: &'static str,
    pub tray_show_hide: &'static str,
    pub tray_hide_window: &'static str,
    pub tray_show_window: &'static str,
    pub tray_play_pause: &'static str,
    pub tray_quit: &'static str,
    pub discord_waiting: &'static str,
    pub discord_connecting: &'static str,
    pub discord_connected: &'static str,
    pub connect_device_updated: &'static str,
    pub connect_pin_refreshed: &'static str,
    pub connect_device_unpaired: &'static str,
    pub connect_unpair_failed: &'static str,
    pub connect_start_failed: &'static str,
    pub connect_handoff_ignored: &'static str,
    pub connect_queue_adopted: &'static str,
    pub connect_pair_request_title: &'static str,
    pub connect_pair_request_prompt: &'static str,
    pub connect_pair_approved: &'static str,
    pub connect_pair_rejected: &'static str,
    pub connect_pair_failed: &'static str,
    pub connect_name_prompt: &'static str,
    pub connect_modal_hints: &'static str,
    pub connect_pair_hints: &'static str,
    pub connect_short: &'static str,
    pub downloads_path_prompt: &'static str,
    pub local_folder_prompt: &'static str,
    pub playing_prefix: &'static str,
    pub loading_ytm_home: &'static str,
    pub loading_ytm: &'static str,
    pub audio_warning: &'static str,
    pub scrobble_cleared: &'static str,
    pub scrobble_save_failed: &'static str,
    pub no_tracks_to_play: &'static str,
    pub personal_only_streaming_disabled: &'static str,
    pub personal_only_no_playable: &'static str,
    pub idle: &'static str,
    pub tui_help_ecosystem: &'static str,

    // Extended TUI & Desktop UI/Status Localization
    pub warning_label: &'static str,
    pub on_label: &'static str,
    pub on_upper: &'static str,
    pub off_upper: &'static str,
    pub prev_short: &'static str,
    pub next_short: &'static str,
    pub vol_short: &'static str,
    pub col_source: &'static str,
    pub custom_label: &'static str,
    pub themes_label: &'static str,
    pub theme_saved: &'static str,
    pub switch_label: &'static str,
    pub personal_tag: &'static str,
    pub mode_label: &'static str,
    pub volume_label: &'static str,
    pub audio_muted: &'static str,
    pub repeat_mode_off: &'static str,
    pub repeat_mode_all: &'static str,
    pub repeat_mode_one: &'static str,
    pub min_short: &'static str,
    pub day_short: &'static str,
    pub plays_word: &'static str,
    pub tracks_word: &'static str,
    pub albums_word: &'static str,
    pub libraries_label: &'static str,
    pub playback_queue: &'static str,
    pub local_songs: &'static str,
    pub local_album: &'static str,
    pub guest_no_browser_login: &'static str,
    pub scanning_browser_cookies: &'static str,
    pub personal_only_streaming_paused: &'static str,
    pub personal_only_enabled_preserved: &'static str,
    pub personal_only_playlists_disabled: &'static str,
    pub personal_only_signin_skipped: &'static str,
    pub personal_only_short: &'static str,
    pub personal_only_radio_disabled: &'static str,
    pub streaming_restored: &'static str,
    pub local_music_disabled: &'static str,
    pub saved_on_paused_personal: &'static str,
    pub cache_disabled_cleanup: &'static str,
    pub checking_updates: &'static str,
    pub update_available_settings_hint: &'static str,
    pub sonata_up_to_date: &'static str,
    pub opened_github_releases: &'static str,
    pub mini_mode_hint: &'static str,
    pub full_mode: &'static str,
    pub hyprland_opacity_locked_msg: &'static str,
    pub tui_opacity_locked: &'static str,
    pub tui_search_heading_prompt: &'static str,
    pub tui_search_hint_wide: &'static str,
    pub tui_search_hint_medium: &'static str,
    pub tui_search_hint_short: &'static str,
    pub tui_badge_romanize: &'static str,
    pub tui_badge_translate: &'static str,
    pub tui_image_notice_title: &'static str,
    pub tui_image_notice_body: &'static str,
    pub tui_btn_okay: &'static str,
    pub tui_btn_dont_show: &'static str,
    pub tui_btn_exit: &'static str,
    pub tui_btn_hide: &'static str,
    pub tui_btn_quit: &'static str,
    pub tui_spotify_import_prompt: &'static str,
    pub tui_local_no_folders: &'static str,
    pub tui_local_hints: &'static str,
    pub tui_settings_hints_wide: &'static str,
    pub tui_settings_hints_narrow: &'static str,
    pub tui_help_modal_title: &'static str,
    pub tui_help_lines: &'static [&'static str],
    pub tui_server_manager_hints: &'static str,
    pub tui_add_server_hints: &'static str,
    pub tui_server_browser_hints: &'static str,
    pub tui_stats_hints: &'static str,
    pub tui_scrobble_form_hints: &'static str,
    pub smb_cifs_share: &'static str,
    pub field_protocol: &'static str,
    pub field_name: &'static str,
    pub field_base_url: &'static str,
    pub field_username: &'static str,
    pub field_password: &'static str,
    pub field_api_key_token: &'static str,
    pub field_api_key: &'static str,
    pub field_api_secret: &'static str,
    pub field_session_key: &'static str,
    pub field_user_token: &'static str,
    pub loading_server_library: &'static str,
    pub no_items_in_section: &'static str,
    pub loading_stats: &'static str,
    pub testing_speed: &'static str,
    pub no_sample: &'static str,
    pub badge_resolving: &'static str,
    pub badge_preparing_next: &'static str,
    pub badge_retrying: &'static str,
    pub badge_radio: &'static str,
    pub badge_finished: &'static str,
    pub badge_stopped: &'static str,
    pub loading_prefix: &'static str,
    pub starting_prefix: &'static str,
    pub caching_next_track: &'static str,
    pub upgraded_audio_quality: &'static str,
    pub upgrade_failed_continuing: &'static str,
    pub stream_expired_retrying: &'static str,
    pub playback_error: &'static str,
    pub playback_failed: &'static str,
    pub queue_finished: &'static str,
    pub finding_similar_tracks: &'static str,
    pub playing_similar_tracks: &'static str,
    pub autoplay_playing_similar: &'static str,
    pub switched_to_music_mode: &'static str,
    pub video_mode_label: &'static str,
    pub sleep_timer_off: &'static str,
    pub ytm_liked_subtitle: &'static str,
    pub ytm_playlist_subtitle: &'static str,
    pub spotify_preview_notice: &'static str,
    pub scrobble_saved: &'static str,
    pub lastfm_saved: &'static str,
    pub listenbrainz_saved: &'static str,
    pub signed_in_ytm: &'static str,
    pub signed_out_ytm: &'static str,
    pub refreshing_ytm_library: &'static str,
    pub refreshing_browser_session: &'static str,
    pub home_ready: &'static str,
    pub artist_ready: &'static str,
    pub artist_failed: &'static str,
    pub loading_artist_generic: &'static str,
    pub artist_info_unavailable: &'static str,
    pub search_failed: &'static str,
    pub playlist_cover_updated_ytm: &'static str,
    pub playlist_updated_ytm: &'static str,
    pub updating_playlist_ytm: &'static str,
    pub created_ytm_playlist: &'static str,
    pub create_playlist_failed: &'static str,
    pub create_personal_playlist_failed: &'static str,
    pub rename_playlist_failed: &'static str,
    pub rename_failed: &'static str,
    pub delete_failed: &'static str,
    pub deleted_personal_playlist: &'static str,
    pub cover_upload_failed: &'static str,
    pub cover_read_failed: &'static str,
    pub personal_playlist_failed: &'static str,
    pub personal_playlist_task_failed: &'static str,
    pub personal_playlist_add_failed: &'static str,
    pub add_failed: &'static str,
    pub remove_failed: &'static str,
    pub open_playlist_to_remove: &'static str,
    pub track_missing_set_video_id_tui: &'static str,
    pub track_missing_set_video_id_desktop: &'static str,
    pub select_track_first: &'static str,
    pub no_playlists_create_hint: &'static str,
    pub personal_tracks_only_personal_playlists: &'static str,
    pub sign_in_ytm_first: &'static str,
    pub sign_in_ytm_browser_first: &'static str,
    pub sign_in_ytm_sync_likes: &'static str,
    pub sign_in_ytm_like_songs: &'static str,
    pub sign_in_ytm_rate_songs: &'static str,
    pub sign_in_ytm_save_albums: &'static str,
    pub sign_in_ytm_edit_playlists: &'static str,
    pub self_hosted_no_ytm_likes: &'static str,
    pub could_not_match_ytm: &'static str,
    pub like_sync_failed: &'static str,
    pub like_update_failed: &'static str,
    pub dislike_failed: &'static str,
    pub release_cannot_be_saved: &'static str,
    pub library_save_failed: &'static str,
    pub library_sync_failed: &'static str,
    pub invalid_spotify_url: &'static str,
    pub importing_spotify_playlist: &'static str,
    pub spotify_import_failed: &'static str,
    pub failed_load_playlist: &'static str,
    pub failed_load_tracks: &'static str,
    pub failed_load_server_tracks: &'static str,
    pub starting_quick_radio: &'static str,
    pub downloads_store_unavailable: &'static str,
    pub downloads_store_open_failed: &'static str,
    pub failed_remove_download: &'static str,
    pub removed_track_from_downloads: &'static str,
    pub clearing_offline_downloads: &'static str,
    pub clearing_all_offline_downloads: &'static str,
    pub cleared_all_offline_downloads: &'static str,
    pub invalid_downloads_folder: &'static str,
    pub scanning_local_folders: &'static str,
    pub server_connected: &'static str,
    pub server_ok: &'static str,
    pub server_connection_failed: &'static str,
    pub server_save_failed: &'static str,
    pub server_error: &'static str,
    pub server_url_empty: &'static str,
    pub server_url_required: &'static str,
    pub removed_server_profile: &'static str,
    pub removed_music_server_profile: &'static str,
    pub connected_label: &'static str,
}

/// Translates common YouTube Music mood and genre chip/tile titles into `Tr` / `Ru`.
pub fn localize_mood_title(lang: AppLanguage, raw: &str) -> Cow<'_, str> {
    if lang == AppLanguage::En {
        return Cow::Borrowed(raw);
    }
    let key = raw.trim().to_ascii_lowercase();
    let translated = match (lang, key.as_str()) {
        (AppLanguage::Tr, "chill") => Some("Rahatlatıcı"),
        (AppLanguage::Tr, "focus") => Some("Odaklanma"),
        (AppLanguage::Tr, "workout") => Some("Antrenman"),
        (AppLanguage::Tr, "energize" | "energy") => Some("Enerjik"),
        (AppLanguage::Tr, "party") => Some("Parti"),
        (AppLanguage::Tr, "romance") => Some("Romantik"),
        (AppLanguage::Tr, "sleep") => Some("Uyku"),
        (AppLanguage::Tr, "sad") => Some("Hüzünlü"),
        (AppLanguage::Tr, "commute") => Some("Yolculuk"),
        (AppLanguage::Tr, "feel good") => Some("İyi Hisset"),
        (AppLanguage::Tr, "gaming") => Some("Oyun"),
        (AppLanguage::Tr, "classical") => Some("Klasik"),
        (AppLanguage::Tr, "electronic") => Some("Elektronik"),
        (AppLanguage::Tr, "rock") => Some("Rock"),
        (AppLanguage::Tr, "pop") => Some("Pop"),
        (AppLanguage::Tr, "jazz") => Some("Caz"),
        (AppLanguage::Tr, "hip-hop" | "hip hop") => Some("Hip-Hop"),
        (AppLanguage::Tr, "indie & alternative" | "indie") => Some("Indie ve Alternatif"),
        (AppLanguage::Tr, "metal") => Some("Metal"),
        (AppLanguage::Tr, "folk & acoustic") => Some("Folk ve Akustik"),
        (AppLanguage::Tr, "decades") => Some("Dönemler"),
        (AppLanguage::Ru, "chill") => Some("Спокойная"),
        (AppLanguage::Ru, "focus") => Some("Фокус"),
        (AppLanguage::Ru, "workout") => Some("Тренировка"),
        (AppLanguage::Ru, "energize" | "energy") => Some("Энергия"),
        (AppLanguage::Ru, "party") => Some("Вечеринка"),
        (AppLanguage::Ru, "romance") => Some("Романтика"),
        (AppLanguage::Ru, "sleep") => Some("Сон"),
        (AppLanguage::Ru, "sad") => Some("Грустная"),
        (AppLanguage::Ru, "commute") => Some("В дороге"),
        (AppLanguage::Ru, "feel good") => Some("Хорошее настроение"),
        (AppLanguage::Ru, "gaming") => Some("Для игр"),
        (AppLanguage::Ru, "classical") => Some("Классика"),
        (AppLanguage::Ru, "electronic") => Some("Электроника"),
        (AppLanguage::Ru, "jazz") => Some("Джаз"),
        (AppLanguage::Ru, "hip-hop" | "hip hop") => Some("Хип-хоп"),
        (AppLanguage::Ru, "indie & alternative" | "indie") => Some("Инди и альтернатива"),
        (AppLanguage::Ru, "metal") => Some("Метал"),
        (AppLanguage::Ru, "folk & acoustic") => Some("Фолк и акустика"),
        (AppLanguage::Ru, "decades") => Some("Эпохи"),
        _ => None,
    };
    match translated {
        Some(t) => Cow::Borrowed(t),
        None => Cow::Borrowed(raw),
    }
}

/// Translates common YouTube Music shelf headings into `Tr` / `Ru` while keeping unknown
/// or dynamic headings intact.
pub fn localize_shelf_title(lang: AppLanguage, raw: &str) -> Cow<'_, str> {
    if lang == AppLanguage::En {
        return Cow::Borrowed(raw);
    }
    let trimmed = raw.trim();
    let key = trimmed.to_ascii_lowercase();

    if trimmed.len() > 11
        && trimmed.is_char_boundary(11)
        && trimmed[..11].eq_ignore_ascii_case("similar to ")
    {
        let subject = trimmed[11..].trim();
        if !subject.is_empty() {
            return match lang {
                AppLanguage::En => Cow::Borrowed(raw),
                AppLanguage::Tr => Cow::Owned(format!("{subject} benzeri")),
                AppLanguage::Ru => Cow::Owned(format!("Похожее на {subject}")),
            };
        }
    }

    if trimmed.len() > 10
        && trimmed.is_char_boundary(10)
        && trimmed[..10].eq_ignore_ascii_case("more from ")
    {
        let subject = trimmed[10..].trim();
        if !subject.is_empty() {
            return match lang {
                AppLanguage::En => Cow::Borrowed(raw),
                AppLanguage::Tr => Cow::Owned(format!("{subject} sanatçısından daha fazlası")),
                AppLanguage::Ru => Cow::Owned(format!("Ещё от {subject}")),
            };
        }
    }

    let translated = match (lang, key.as_str()) {
        (AppLanguage::Tr, "quick picks") => Some("Hızlı seçimler"),
        (AppLanguage::Tr, "listen again") => Some("Tekrar dinle"),
        (AppLanguage::Tr, "new releases") => Some("Yeni çıkanlar"),
        (AppLanguage::Tr, "new albums & singles") => Some("Yeni albümler ve tekliler"),
        (AppLanguage::Tr, "trending") => Some("Trendler"),
        (AppLanguage::Tr, "charts") => Some("Listeler"),
        (AppLanguage::Tr, "mixed for you") => Some("Senin için miksler"),
        (AppLanguage::Tr, "featured playlists for you") => {
            Some("Senin için öne çıkan çalma listeleri")
        }
        (AppLanguage::Tr, "from the community") => Some("Topluluktan"),
        (AppLanguage::Tr, "top songs") => Some("Popüler şarkılar"),
        (AppLanguage::Tr, "albums") => Some("Albümler"),
        (AppLanguage::Tr, "singles") => Some("Tekliler"),
        (AppLanguage::Tr, "singles & eps") => Some("Tekliler ve EP'ler"),
        (AppLanguage::Tr, "videos") => Some("Videolar"),
        (AppLanguage::Tr, "playlists") => Some("Çalma listeleri"),
        (AppLanguage::Tr, "your mixes") => Some("Mikslerin"),
        (AppLanguage::Tr, "signature mixes") => Some("Öne çıkan miksler"),
        (AppLanguage::Tr, "mood mixes") => Some("Ruh hali miksleri"),
        (AppLanguage::Tr, "featured mixes") => Some("Öne çıkan miksler"),
        (AppLanguage::Tr, "recommended albums") => Some("Önerilen albümler"),
        (AppLanguage::Tr, "recommended music videos") => Some("Önerilen müzik videoları"),
        (AppLanguage::Tr, "similar artists") => Some("Benzer sanatçılar"),
        (AppLanguage::Tr, "from your library") => Some("Kitaplığından"),
        (AppLanguage::Tr, "long listening") => Some("Uzun dinleme"),
        (AppLanguage::Tr, "moods & genres") => Some("Ruh halleri ve türler"),
        (AppLanguage::Tr, "start radio from a song") => Some("Bir şarkıdan radyo başlat"),
        (AppLanguage::Ru, "quick picks") => Some("Быстрый выбор"),
        (AppLanguage::Ru, "listen again") => Some("Послушать снова"),
        (AppLanguage::Ru, "new releases") => Some("Новинки"),
        (AppLanguage::Ru, "new albums & singles") => Some("Новые альбомы и синглы"),
        (AppLanguage::Ru, "trending") => Some("В тренде"),
        (AppLanguage::Ru, "charts") => Some("Чарты"),
        (AppLanguage::Ru, "mixed for you") => Some("Миксы для вас"),
        (AppLanguage::Ru, "featured playlists for you") => Some("Избранные плейлисты для вас"),
        (AppLanguage::Ru, "from the community") => Some("От сообщества"),
        (AppLanguage::Ru, "top songs") => Some("Популярные треки"),
        (AppLanguage::Ru, "albums") => Some("Альбомы"),
        (AppLanguage::Ru, "singles") => Some("Синглы"),
        (AppLanguage::Ru, "singles & eps") => Some("Синглы и мини-альбомы"),
        (AppLanguage::Ru, "videos") => Some("Видео"),
        (AppLanguage::Ru, "playlists") => Some("Плейлисты"),
        (AppLanguage::Ru, "your mixes") => Some("Ваши миксы"),
        (AppLanguage::Ru, "signature mixes") => Some("Фирменные миксы"),
        (AppLanguage::Ru, "mood mixes") => Some("Миксы по настроению"),
        (AppLanguage::Ru, "featured mixes") => Some("Избранные миксы"),
        (AppLanguage::Ru, "recommended albums") => Some("Рекомендуемые альбомы"),
        (AppLanguage::Ru, "recommended music videos") => Some("Рекомендуемые клипы"),
        (AppLanguage::Ru, "similar artists") => Some("Похожие исполнители"),
        (AppLanguage::Ru, "from your library") => Some("Из вашей медиатеки"),
        (AppLanguage::Ru, "long listening") => Some("Для долгого прослушивания"),
        (AppLanguage::Ru, "moods & genres") => Some("Настроения и жанры"),
        (AppLanguage::Ru, "start radio from a song") => Some("Радио на основе песни"),
        _ => None,
    };
    match translated {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(raw),
    }
}

/// Returns the localized label for a [`sonata_core::player::RepeatMode`].
pub fn repeat_mode_label(lang: AppLanguage, mode: sonata_core::player::RepeatMode) -> &'static str {
    let s = strings(lang);
    match mode {
        sonata_core::player::RepeatMode::Off => s.repeat_mode_off,
        sonata_core::player::RepeatMode::All => s.repeat_mode_all,
        sonata_core::player::RepeatMode::One => s.repeat_mode_one,
    }
}

/// Localizes the `" (Custom)"` suffix on custom theme display names without altering theme IDs.
pub fn display_theme_name(lang: AppLanguage, name: &str) -> Cow<'_, str> {
    if lang == AppLanguage::En {
        return Cow::Borrowed(name);
    }
    if let Some(stem) = name.strip_suffix(" (Custom)") {
        let s = strings(lang);
        Cow::Owned(format!("{stem} ({})", s.custom_label))
    } else {
        Cow::Borrowed(name)
    }
}

/// Formats a localized server health indicator string.
pub fn format_health_badge(
    lang: AppLanguage,
    alive: bool,
    latency_ms: Option<u64>,
    desktop_style: bool,
) -> String {
    let s = strings(lang);
    if alive {
        match latency_ms {
            Some(ms) if desktop_style => format!("{} • {ms} {}", s.server_alive, s.latency_ms),
            Some(ms) => format!("{} {ms}{}", s.server_alive, s.latency_ms),
            None => s.server_alive.to_string(),
        }
    } else {
        s.server_offline.to_string()
    }
}

/// Formats a localized server speed test badge string.
pub fn format_speed_badge(
    lang: AppLanguage,
    alive: bool,
    latency_ms: u64,
    download_mbps: Option<f64>,
    desktop_style: bool,
) -> String {
    let s = strings(lang);
    if !alive {
        return s.server_offline.to_string();
    }
    match download_mbps {
        Some(mbps) if mbps >= 100.0 => {
            if desktop_style {
                format!(
                    "{mbps:.0} {} • {latency_ms} {}",
                    s.throughput_mbps, s.latency_ms
                )
            } else {
                format!(
                    "{mbps:.0} {} ({latency_ms}{})",
                    s.throughput_mbps, s.latency_ms
                )
            }
        }
        Some(mbps) => {
            if desktop_style {
                format!(
                    "{mbps:.1} {} • {latency_ms} {}",
                    s.throughput_mbps, s.latency_ms
                )
            } else {
                format!(
                    "{mbps:.1} {} ({latency_ms}{})",
                    s.throughput_mbps, s.latency_ms
                )
            }
        }
        None => {
            if desktop_style {
                format!("{latency_ms} {} ({})", s.latency_ms, s.no_sample)
            } else {
                format!("{latency_ms}{}", s.latency_ms)
            }
        }
    }
}

pub fn fmt_downloaded_track(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Downloaded \"{title}\" for offline playback"),
        AppLanguage::Tr => format!("\"{title}\" çevrimdışı oynatma için indirildi"),
        AppLanguage::Ru => format!("«{title}» скачан для офлайн-воспроизведения"),
    }
}

pub fn fmt_download_failed(lang: AppLanguage, title: &str, err: &str) -> String {
    match lang {
        AppLanguage::En => format!("Download failed for \"{title}\": {err}"),
        AppLanguage::Tr => format!("\"{title}\" indirilemedi: {err}"),
        AppLanguage::Ru => format!("Не удалось скачать «{title}»: {err}"),
    }
}

pub fn fmt_downloaded_collection(
    lang: AppLanguage,
    saved: usize,
    title: &str,
    errors: usize,
) -> String {
    match (lang, errors == 0) {
        (AppLanguage::En, true) => format!("Downloaded {saved} tracks from \"{title}\""),
        (AppLanguage::En, false) => {
            format!("Downloaded {saved} tracks from \"{title}\" ({errors} failed)")
        }
        (AppLanguage::Tr, true) => format!("\"{title}\" kaynağından {saved} parça indirildi"),
        (AppLanguage::Tr, false) => {
            format!("\"{title}\" kaynağından {saved} parça indirildi ({errors} başarısız)")
        }
        (AppLanguage::Ru, true) => format!("Скачано {saved} треков из «{title}»"),
        (AppLanguage::Ru, false) => {
            format!("Скачано {saved} треков из «{title}» (ошибок: {errors})")
        }
    }
}

pub fn fmt_downloading_collection(lang: AppLanguage, count: usize, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Downloading {count} tracks from \"{title}\"..."),
        AppLanguage::Tr => format!("\"{title}\" kaynağından {count} parça indiriliyor..."),
        AppLanguage::Ru => format!("Скачивание {count} треков из «{title}»..."),
    }
}

pub fn fmt_downloading_item(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Downloading \"{title}\"..."),
        AppLanguage::Tr => format!("\"{title}\" indiriliyor..."),
        AppLanguage::Ru => format!("Скачивание «{title}»..."),
    }
}

pub fn fmt_removing_download(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removing \"{title}\" from downloads..."),
        AppLanguage::Tr => format!("\"{title}\" indirilenlerden kaldırılıyor..."),
        AppLanguage::Ru => format!("Удаление «{title}» из загрузок..."),
    }
}

pub fn fmt_removed_download(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed \"{title}\" from offline downloads"),
        AppLanguage::Tr => format!("\"{title}\" çevrimdışı indirmelerden kaldırıldı"),
        AppLanguage::Ru => format!("«{title}» удалён из офлайн-загрузок"),
    }
}

pub fn fmt_removing_collection_download(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removing downloaded collection \"{title}\"..."),
        AppLanguage::Tr => format!("İndirilen \"{title}\" koleksiyonu kaldırılıyor..."),
        AppLanguage::Ru => format!("Удаление скачанной коллекции «{title}»..."),
    }
}

pub fn fmt_removed_collection_freed(lang: AppLanguage, removed: usize) -> String {
    match lang {
        AppLanguage::En => format!("Removed collection ({removed} tracks freed)"),
        AppLanguage::Tr => format!("Koleksiyon kaldırıldı ({removed} parça temizlendi)"),
        AppLanguage::Ru => format!("Коллекция удалена (освобождено треков: {removed})"),
    }
}

pub fn fmt_cleared_offline_tracks(lang: AppLanguage, count: usize) -> String {
    match lang {
        AppLanguage::En => format!("Cleared {count} offline tracks"),
        AppLanguage::Tr => format!("{count} çevrimdışı parça temizlendi"),
        AppLanguage::Ru => format!("Очищено офлайн-треков: {count}"),
    }
}

pub fn fmt_local_scan_summary(lang: AppLanguage, tracks: usize, albums: usize) -> String {
    match lang {
        AppLanguage::En => format!("Local Music: {tracks} tracks across {albums} albums"),
        AppLanguage::Tr => format!("Yerel Müzik: {albums} albümde {tracks} parça"),
        AppLanguage::Ru => format!("Локальная музыка: {tracks} треков в {albums} альбомах"),
    }
}

pub fn fmt_searching_for(lang: AppLanguage, query: &str) -> String {
    match lang {
        AppLanguage::En => format!("Searching for \"{query}\"..."),
        AppLanguage::Tr => format!("\"{query}\" aranıyor..."),
        AppLanguage::Ru => format!("Поиск «{query}»..."),
    }
}

pub fn fmt_searching_personal(lang: AppLanguage, query: &str) -> String {
    match lang {
        AppLanguage::En => format!("Searching local & personal servers for \"{query}\"..."),
        AppLanguage::Tr => format!("Yerel ve kişisel sunucularda \"{query}\" aranıyor..."),
        AppLanguage::Ru => format!("Поиск «{query}» локально и на серверах..."),
    }
}

pub fn fmt_searching_ytm_jiosaavn(lang: AppLanguage, query: &str) -> String {
    match lang {
        AppLanguage::En => format!("Searching YouTube Music & JioSaavn for \"{query}\"..."),
        AppLanguage::Tr => format!("YouTube Music ve JioSaavn'da \"{query}\" aranıyor..."),
        AppLanguage::Ru => format!("Поиск «{query}» в YouTube Music и JioSaavn..."),
    }
}

pub fn fmt_searching_ytm(lang: AppLanguage, query: &str) -> String {
    match lang {
        AppLanguage::En => format!("Searching YouTube Music for \"{query}\"..."),
        AppLanguage::Tr => format!("YouTube Music'te \"{query}\" aranıyor..."),
        AppLanguage::Ru => format!("Поиск «{query}» в YouTube Music..."),
    }
}

pub fn fmt_loading_item(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Loading {name}..."),
        AppLanguage::Tr => format!("{name} yükleniyor..."),
        AppLanguage::Ru => format!("Загрузка {name}..."),
    }
}

pub fn fmt_loaded_item(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Loaded {name}"),
        AppLanguage::Tr => format!("{name} yüklendi"),
        AppLanguage::Ru => format!("Загружено: {name}"),
    }
}

pub fn fmt_loaded_tracks(lang: AppLanguage, count: usize) -> String {
    match lang {
        AppLanguage::En => format!("Loaded {count} tracks"),
        AppLanguage::Tr => format!("{count} parça yüklendi"),
        AppLanguage::Ru => format!("Загружено треков: {count}"),
    }
}

pub fn fmt_synced_tracks(lang: AppLanguage, count: usize) -> String {
    match lang {
        AppLanguage::En => format!("Synced {count} tracks"),
        AppLanguage::Tr => format!("{count} parça eşitlendi"),
        AppLanguage::Ru => format!("Синхронизировано треков: {count}"),
    }
}

pub fn fmt_synced_ytm_playlists(lang: AppLanguage, count: usize) -> String {
    match lang {
        AppLanguage::En => format!("Synced {count} YouTube Music playlists"),
        AppLanguage::Tr => format!("{count} YouTube Music çalma listesi eşitlendi"),
        AppLanguage::Ru => format!("Синхронизировано плейлистов YouTube Music: {count}"),
    }
}

pub fn fmt_found_ytm_songs(lang: AppLanguage, count: usize) -> String {
    match lang {
        AppLanguage::En => format!("Found {count} songs on YouTube Music"),
        AppLanguage::Tr => format!("YouTube Music'te {count} şarkı bulundu"),
        AppLanguage::Ru => format!("Найдено песен в YouTube Music: {count}"),
    }
}

pub fn fmt_liking_on_ytm(lang: AppLanguage, liking: bool, title: &str) -> String {
    match (lang, liking) {
        (AppLanguage::En, true) => format!("Liking \"{title}\" on YouTube Music..."),
        (AppLanguage::En, false) => format!("Unliking \"{title}\" on YouTube Music..."),
        (AppLanguage::Tr, true) => format!("\"{title}\" YouTube Music'te beğeniliyor..."),
        (AppLanguage::Tr, false) => {
            format!("\"{title}\" YouTube Music beğenilerinden kaldırılıyor...")
        }
        (AppLanguage::Ru, true) => format!("Добавление «{title}» в понравившиеся YouTube Music..."),
        (AppLanguage::Ru, false) => {
            format!("Удаление «{title}» из понравившихся YouTube Music...")
        }
    }
}

pub fn fmt_saved_to_liked(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Saved \"{title}\" to YouTube Music Liked Songs"),
        AppLanguage::Tr => format!("\"{title}\" YouTube Music Beğenilen Şarkılara eklendi"),
        AppLanguage::Ru => format!("«{title}» сохранён в «Понравившиеся» YouTube Music"),
    }
}

pub fn fmt_removed_from_liked(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed \"{title}\" from Liked Songs"),
        AppLanguage::Tr => format!("\"{title}\" Beğenilen Şarkılardan kaldırıldı"),
        AppLanguage::Ru => format!("«{title}» удалён из «Понравившихся»"),
    }
}

pub fn fmt_liked_track(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Liked \"{title}\""),
        AppLanguage::Tr => format!("\"{title}\" beğenildi"),
        AppLanguage::Ru => format!("Понравилось: «{title}»"),
    }
}

pub fn fmt_removed_like(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed like on \"{title}\""),
        AppLanguage::Tr => format!("\"{title}\" beğenisi kaldırıldı"),
        AppLanguage::Ru => format!("Лайк убран: «{title}»"),
    }
}

pub fn fmt_disliked_track(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Marked \"{title}\" as disliked"),
        AppLanguage::Tr => format!("\"{title}\" beğenilmedi olarak işaretlendi"),
        AppLanguage::Ru => format!("«{title}» отмечен как «Не нравится»"),
    }
}

pub fn fmt_saved_to_library(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Saved \"{title}\" to Library"),
        AppLanguage::Tr => format!("\"{title}\" Kitaplığa kaydedildi"),
        AppLanguage::Ru => format!("«{title}» сохранено в медиатеку"),
    }
}

pub fn fmt_removed_from_library(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed \"{title}\" from Library"),
        AppLanguage::Tr => format!("\"{title}\" Kitaplıktan çıkarıldı"),
        AppLanguage::Ru => format!("«{title}» удалено из медиатеки"),
    }
}

pub fn fmt_adding_to_playlist(lang: AppLanguage, title: &str, playlist: &str) -> String {
    match lang {
        AppLanguage::En => format!("Adding \"{title}\" to {playlist}..."),
        AppLanguage::Tr => format!("\"{title}\", {playlist} listesine ekleniyor..."),
        AppLanguage::Ru => format!("Добавление «{title}» в {playlist}..."),
    }
}

pub fn fmt_added_to_playlist(lang: AppLanguage, title: &str, playlist: &str) -> String {
    match lang {
        AppLanguage::En => format!("Added \"{title}\" to {playlist}"),
        AppLanguage::Tr => format!("\"{title}\", {playlist} listesine eklendi"),
        AppLanguage::Ru => format!("«{title}» добавлен в {playlist}"),
    }
}

pub fn fmt_removing_from_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removing \"{title}\" from playlist..."),
        AppLanguage::Tr => format!("\"{title}\" çalma listesinden kaldırılıyor..."),
        AppLanguage::Ru => format!("Удаление «{title}» из плейлиста..."),
    }
}

pub fn fmt_removed_from_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed \"{title}\" from playlist"),
        AppLanguage::Tr => format!("\"{title}\" çalma listesinden kaldırıldı"),
        AppLanguage::Ru => format!("«{title}» удалён из плейлиста"),
    }
}

pub fn fmt_removed_from_ytm_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed \"{title}\" from YouTube Music playlist"),
        AppLanguage::Tr => format!("\"{title}\" YouTube Music çalma listesinden kaldırıldı"),
        AppLanguage::Ru => format!("«{title}» удалён из плейлиста YouTube Music"),
    }
}

pub fn fmt_removed_track_from_playlist(lang: AppLanguage, playlist: &str) -> String {
    match lang {
        AppLanguage::En => format!("Removed track from {playlist}"),
        AppLanguage::Tr => format!("Parça {playlist} listesinden kaldırıldı"),
        AppLanguage::Ru => format!("Трек удалён из {playlist}"),
    }
}

pub fn fmt_creating_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Creating playlist \"{title}\"..."),
        AppLanguage::Tr => format!("\"{title}\" çalma listesi oluşturuluyor..."),
        AppLanguage::Ru => format!("Создание плейлиста «{title}»..."),
    }
}

pub fn fmt_created_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Created playlist \"{title}\""),
        AppLanguage::Tr => format!("\"{title}\" çalma listesi oluşturuldu"),
        AppLanguage::Ru => format!("Создан плейлист «{title}»"),
    }
}

pub fn fmt_creating_personal_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Creating personal playlist \"{title}\"..."),
        AppLanguage::Tr => format!("\"{title}\" kişisel listesi oluşturuluyor..."),
        AppLanguage::Ru => format!("Создание личного плейлиста «{title}»..."),
    }
}

pub fn fmt_created_personal_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Created personal playlist \"{title}\""),
        AppLanguage::Tr => format!("\"{title}\" kişisel listesi oluşturuldu"),
        AppLanguage::Ru => format!("Создан личный плейлист «{title}»"),
    }
}

pub fn fmt_renamed_personal_playlist(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Renamed personal playlist to \"{title}\""),
        AppLanguage::Tr => format!("Kişisel liste \"{title}\" olarak adlandırıldı"),
        AppLanguage::Ru => format!("Личный плейлист переименован в «{title}»"),
    }
}

pub fn fmt_checking_item(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Checking {name}..."),
        AppLanguage::Tr => format!("{name} denetleniyor..."),
        AppLanguage::Ru => format!("Проверка {name}..."),
    }
}

pub fn fmt_testing_item(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Testing {name}..."),
        AppLanguage::Tr => format!("{name} test ediliyor..."),
        AppLanguage::Ru => format!("Проверка {name}..."),
    }
}

pub fn fmt_saving_item(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Saving {name}..."),
        AppLanguage::Tr => format!("{name} kaydediliyor..."),
        AppLanguage::Ru => format!("Сохранение {name}..."),
    }
}

pub fn fmt_saved_server(lang: AppLanguage, name: &str, detail: &str) -> String {
    match lang {
        AppLanguage::En => format!("Saved {name} ({detail})"),
        AppLanguage::Tr => format!("{name} kaydedildi ({detail})"),
        AppLanguage::Ru => format!("Сохранено: {name} ({detail})"),
    }
}

pub fn fmt_saved_server_warning(lang: AppLanguage, name: &str, err: &str) -> String {
    match lang {
        AppLanguage::En => format!("Saved {name} (warning: {err})"),
        AppLanguage::Tr => format!("{name} kaydedildi (uyarı: {err})"),
        AppLanguage::Ru => format!("Сохранено: {name} (предупреждение: {err})"),
    }
}

pub fn fmt_server_toggled(lang: AppLanguage, enabled: bool, name: &str) -> String {
    match (lang, enabled) {
        (AppLanguage::En, true) => format!("Enabled server {name}"),
        (AppLanguage::En, false) => format!("Disabled server {name}"),
        (AppLanguage::Tr, true) => format!("{name} sunucusu açıldı"),
        (AppLanguage::Tr, false) => format!("{name} sunucusu kapatıldı"),
        (AppLanguage::Ru, true) => format!("Сервер {name} включён"),
        (AppLanguage::Ru, false) => format!("Сервер {name} отключён"),
    }
}

pub fn fmt_speed_test_for(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Running speed test for {name}..."),
        AppLanguage::Tr => format!("{name} için hız testi çalıştırılıyor..."),
        AppLanguage::Ru => format!("Тест скорости для {name}..."),
    }
}

pub fn fmt_signing_in_via(lang: AppLanguage, browser: &str) -> String {
    match lang {
        AppLanguage::En => format!("Signing in via {browser}..."),
        AppLanguage::Tr => format!("{browser} ile giriş yapılıyor..."),
        AppLanguage::Ru => format!("Вход через {browser}..."),
    }
}

pub fn fmt_no_ytm_login_in_browser(lang: AppLanguage, label: &str) -> String {
    match lang {
        AppLanguage::En => format!(
            "No active YouTube Music login found in {label}. Sign in on music.youtube.com in your browser and try again."
        ),
        AppLanguage::Tr => format!(
            "{label} tarayıcısında etkin YouTube Music oturumu bulunamadı. Tarayıcınızda music.youtube.com adresinde oturum açıp tekrar deneyin."
        ),
        AppLanguage::Ru => format!(
            "В браузере {label} не найдена активная сессия YouTube Music. Войдите на music.youtube.com в браузере и повторите попытку."
        ),
    }
}

pub fn fmt_loading_artist(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Loading artist {name}..."),
        AppLanguage::Tr => format!("{name} sanatçısı yükleniyor..."),
        AppLanguage::Ru => format!("Загрузка исполнителя {name}..."),
    }
}

pub fn fmt_artist_not_found(lang: AppLanguage, name: &str) -> String {
    match lang {
        AppLanguage::En => format!("Could not find artist page for \"{name}\""),
        AppLanguage::Tr => format!("\"{name}\" için sanatçı sayfası bulunamadı"),
        AppLanguage::Ru => format!("Не удалось найти страницу исполнителя «{name}»"),
    }
}

pub fn fmt_opening_album(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Opening album {title}..."),
        AppLanguage::Tr => format!("{title} albümü açılıyor..."),
        AppLanguage::Ru => format!("Открытие альбома {title}..."),
    }
}

pub fn fmt_starting_artist_radio(lang: AppLanguage, artist: &str) -> String {
    match lang {
        AppLanguage::En => format!("Starting {artist} radio..."),
        AppLanguage::Tr => format!("{artist} radyosu başlatılıyor..."),
        AppLanguage::Ru => format!("Запуск радио {artist}..."),
    }
}

pub fn fmt_artist_radio_label(lang: AppLanguage, artist: &str) -> String {
    match lang {
        AppLanguage::En => format!("{artist} Radio"),
        AppLanguage::Tr => format!("{artist} Radyosu"),
        AppLanguage::Ru => format!("Радио {artist}"),
    }
}

pub fn fmt_could_not_start(lang: AppLanguage, label: &str) -> String {
    match lang {
        AppLanguage::En => format!("Could not start {label}"),
        AppLanguage::Tr => format!("{label} başlatılamadı"),
        AppLanguage::Ru => format!("Не удалось запустить {label}"),
    }
}

pub fn fmt_playing_item(lang: AppLanguage, label: &str) -> String {
    match lang {
        AppLanguage::En => format!("Playing {label}"),
        AppLanguage::Tr => format!("{label} çalınıyor"),
        AppLanguage::Ru => format!("Играет: {label}"),
    }
}

pub fn fmt_update_available_ver(lang: AppLanguage, version: &str) -> String {
    match lang {
        AppLanguage::En => format!("Update v{version} available"),
        AppLanguage::Tr => format!("v{version} güncellemesi mevcut"),
        AppLanguage::Ru => format!("Доступно обновление v{version}"),
    }
}

pub fn fmt_stream_expired_refreshing(lang: AppLanguage, title: &str) -> String {
    match lang {
        AppLanguage::En => format!("Stream expired; refreshing \"{title}\"..."),
        AppLanguage::Tr => format!("Akış süresi doldu; \"{title}\" yenileniyor..."),
        AppLanguage::Ru => format!("Срок действия потока истёк; обновление «{title}»..."),
    }
}

pub fn fmt_sleep_timer_set(lang: AppLanguage, mins: u64) -> String {
    match lang {
        AppLanguage::En => format!("Sleep timer set for {mins}m"),
        AppLanguage::Tr => format!("Uyku zamanlayıcısı {mins} dk olarak ayarlandı"),
        AppLanguage::Ru => format!("Таймер сна установлен на {mins} мин"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_and_tui_default_language_is_english_and_cycles_tr_and_ru() {
        let default_lang = AppLanguage::default();
        assert_eq!(default_lang, AppLanguage::En);
        assert_eq!(default_lang.code(), "en");
        assert_eq!(strings(default_lang).home, "Home");

        let tr = default_lang.next();
        assert_eq!(tr, AppLanguage::Tr);
        assert_eq!(tr.code(), "tr");
        assert_eq!(tr.label(), "Türkçe");
        assert_eq!(strings(tr).home, "Ana Sayfa");

        let ru = tr.next();
        assert_eq!(ru, AppLanguage::Ru);
        assert_eq!(ru.code(), "ru");
        assert_eq!(ru.label(), "Русский");
        assert_eq!(strings(ru).home, "Главная");
        assert_eq!(strings(ru).settings, "Настройки");
        assert_eq!(ru.next(), AppLanguage::En);
        assert_eq!(default_lang.prev(), AppLanguage::Ru);
        assert_eq!(
            localize_shelf_title(AppLanguage::Tr, "Quick picks"),
            "Hızlı seçimler"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Ru, "New releases"),
            "Новинки"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::En, "Quick picks"),
            "Quick picks"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Tr, "Featured playlists for you"),
            "Senin için öne çıkan çalma listeleri"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Ru, "From the community"),
            "От сообщества"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Tr, "Singles & EPs"),
            "Tekliler ve EP'ler"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Tr, "Similar to The Weeknd"),
            "The Weeknd benzeri"
        );
        assert_eq!(
            localize_shelf_title(AppLanguage::Ru, "More from Daft Punk"),
            "Ещё от Daft Punk"
        );
        assert_eq!(localize_mood_title(AppLanguage::Tr, "Workout"), "Antrenman");
        assert_eq!(localize_mood_title(AppLanguage::Ru, "Focus"), "Фокус");
        assert_eq!(localize_mood_title(AppLanguage::En, "Chill"), "Chill");
        assert_eq!(strings(AppLanguage::En).downloads, "Downloads");
        assert_eq!(strings(AppLanguage::Tr).downloads, "İndirilenler");
        assert_eq!(strings(AppLanguage::Ru).downloads, "Загрузки");
        assert_eq!(strings(AppLanguage::En).servers, "Servers");
        assert_eq!(strings(AppLanguage::Tr).servers, "Sunucular");
        assert_eq!(strings(AppLanguage::Ru).servers, "Серверы");
        assert_eq!(strings(AppLanguage::En).music, "Music");
        assert_eq!(strings(AppLanguage::Tr).music, "Müzik");
        assert_eq!(strings(AppLanguage::Ru).music, "Музыка");
        assert_eq!(strings(AppLanguage::En).video_quality, "Video quality");
        assert_eq!(strings(AppLanguage::Tr).translate_lyrics, "Sözleri çevir");
        assert_eq!(strings(AppLanguage::Ru).sleep_timer, "Таймер сна");

        // Faz 3 Sources, Modules, Personal Only Mode & Personal Playlists strings
        assert_eq!(strings(AppLanguage::En).sources, "Sources");
        assert_eq!(strings(AppLanguage::Tr).sources, "Kaynaklar");
        assert_eq!(strings(AppLanguage::Ru).sources, "Источники");
        assert_eq!(strings(AppLanguage::En).modules, "Modules");
        assert_eq!(strings(AppLanguage::Tr).modules, "Modüller");
        assert_eq!(strings(AppLanguage::Ru).modules, "Модули");
        assert_eq!(
            strings(AppLanguage::En).personal_only_mode,
            "Personal Only Mode"
        );
        assert_eq!(
            strings(AppLanguage::Tr).personal_only_mode,
            "Yalnızca Kişisel Mod"
        );
        assert!(strings(AppLanguage::En)
            .personal_only_mode_desc
            .contains("preserved"));
        assert!(strings(AppLanguage::Tr)
            .personal_only_mode_desc
            .contains("korunur"));
        assert_eq!(strings(AppLanguage::En).local_music, "Local Music");
        assert_eq!(strings(AppLanguage::Tr).local_music, "Yerel Müzik");
        assert_eq!(strings(AppLanguage::Ru).local_music, "Локальная музыка");
        assert_eq!(strings(AppLanguage::En).smb, "SMB");
        assert_eq!(strings(AppLanguage::En).webdav, "WebDAV");
        assert_eq!(strings(AppLanguage::En).jellyfin, "Jellyfin");
        assert_eq!(strings(AppLanguage::En).subsonic, "Subsonic");
        assert_eq!(strings(AppLanguage::En).server_alive, "Alive");
        assert_eq!(strings(AppLanguage::En).server_online, "Online");
        assert_eq!(strings(AppLanguage::Tr).server_offline, "Çevrimdışı");
        assert_eq!(strings(AppLanguage::En).speed_test, "Speed Test");
        assert_eq!(strings(AppLanguage::Tr).speed_test, "Hız Testi");
        assert_eq!(strings(AppLanguage::Ru).speed_test, "Тест скорости");
        assert_eq!(strings(AppLanguage::En).latency_ms, "ms");
        assert_eq!(strings(AppLanguage::En).throughput_mbps, "Mbps");
        assert_eq!(
            strings(AppLanguage::En).personal_playlists,
            "Personal Playlists"
        );
        assert_eq!(
            strings(AppLanguage::Tr).personal_playlists,
            "Kişisel Listeler"
        );
        assert_eq!(
            strings(AppLanguage::Ru).personal_playlists,
            "Личные плейлисты"
        );
        assert_eq!(
            strings(AppLanguage::En).attribute_personal_to_ytm,
            "Report local/server plays to YouTube Music"
        );
        assert_eq!(
            strings(AppLanguage::Tr).attribute_personal_to_ytm,
            "Yerel/sunucu çalmalarını YouTube Music geçmişine işle"
        );
        assert!(!strings(AppLanguage::Ru)
            .attribute_personal_to_ytm
            .is_empty());

        // Faz 4 Gapless, Crossfade, Scrobbling, Listening Stats & Sonata Connect strings
        assert_eq!(
            strings(AppLanguage::En).gapless_playback,
            "Gapless Playback"
        );
        assert_eq!(
            strings(AppLanguage::Tr).gapless_playback,
            "Kesintisiz Oynatma"
        );
        assert_eq!(
            strings(AppLanguage::Ru).gapless_playback,
            "Воспроизведение без пауз"
        );
        assert_eq!(strings(AppLanguage::En).crossfade, "Crossfade");
        assert_eq!(strings(AppLanguage::Tr).crossfade, "Çapraz Geçiş");
        assert_eq!(strings(AppLanguage::Ru).crossfade, "Кроссфейд");
        assert_eq!(
            strings(AppLanguage::En).lastfm_scrobbling,
            "Last.fm Scrobbling"
        );
        assert_eq!(
            strings(AppLanguage::En).listenbrainz_scrobbling,
            "ListenBrainz Scrobbling"
        );
        assert_eq!(strings(AppLanguage::En).listening_stats, "Listening Stats");
        assert_eq!(
            strings(AppLanguage::Tr).listening_stats,
            "Dinleme İstatistikleri"
        );
        assert_eq!(strings(AppLanguage::En).sonata_connect, "Sonata Connect");
        assert_eq!(strings(AppLanguage::Ru).sonata_connect, "Sonata Connect");
        assert_eq!(strings(AppLanguage::En).approve, "Approve");
        assert_eq!(strings(AppLanguage::Tr).approve, "Onayla");
        assert_eq!(strings(AppLanguage::Ru).approve, "Разрешить");
        assert_eq!(strings(AppLanguage::En).reject, "Reject");
        assert_eq!(strings(AppLanguage::Tr).reject, "Reddet");
        assert_eq!(strings(AppLanguage::Ru).reject, "Отклонить");
        assert_eq!(strings(AppLanguage::Tr).discord_connected, "Bağlandı");
        assert_eq!(strings(AppLanguage::Ru).discord_waiting, "Ожидание Discord");
    }

    #[test]
    fn extended_tui_and_desktop_localization_helpers_cover_en_tr_ru() {
        for lang in [AppLanguage::En, AppLanguage::Tr, AppLanguage::Ru] {
            let s = strings(lang);
            assert!(!s.tui_help_lines.is_empty());
            assert!(!s.badge_resolving.is_empty());
            assert!(!s.personal_only_streaming_disabled.is_empty());
            assert!(!repeat_mode_label(lang, sonata_core::player::RepeatMode::All).is_empty());
            assert!(!format_health_badge(lang, true, Some(12), true).is_empty());
            assert!(!format_health_badge(lang, false, None, false).is_empty());
            assert!(!format_speed_badge(lang, true, 18, Some(42.5), true).is_empty());
            assert!(!format_speed_badge(lang, true, 18, None, false).is_empty());
            assert!(!fmt_downloaded_track(lang, "test-track-1").is_empty());
            assert!(!fmt_downloaded_collection(lang, 3, "test-album", 1).is_empty());
            assert!(!fmt_local_scan_summary(lang, 10, 2).is_empty());
            assert!(!fmt_sleep_timer_set(lang, 30).is_empty());
        }

        assert_eq!(
            display_theme_name(AppLanguage::En, "Nord (Custom)"),
            "Nord (Custom)"
        );
        assert_eq!(
            display_theme_name(AppLanguage::Tr, "Nord (Custom)"),
            "Nord (Özel)"
        );
        assert_eq!(
            display_theme_name(AppLanguage::Ru, "Nord (Custom)"),
            "Nord (Своя)"
        );
        assert_eq!(display_theme_name(AppLanguage::Tr, "Nord"), "Nord");
        assert_eq!(
            repeat_mode_label(AppLanguage::Tr, sonata_core::player::RepeatMode::One),
            "Tek"
        );
        assert_eq!(
            repeat_mode_label(AppLanguage::Ru, sonata_core::player::RepeatMode::All),
            "Все"
        );
    }
}
