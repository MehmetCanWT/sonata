//! Shared English, Turkish, and Russian UI localization catalog for Linux Sonata clients.
//!
//! 1. **Language Preference (`AppLanguage`)**: Serializable `en` / `tr` / `ru` selector defaulting to English (`En`) for Desktop and TUI.
//! 2. **Static String Catalog (`UiStrings`)**: Zero-allocation compile-time verified translation tables (`EN`, `TR`, `RU`) powering menus, settings, modals, empty states, and playback controls.

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

pub const EN: UiStrings = UiStrings {
    home: "Home",
    explore: "Explore",
    library: "Library",
    search: "Search",
    queue: "Queue",
    history: "History",
    settings: "Settings",
    playlists: "Playlists",
    liked_music: "Liked Music",
    favorites: "Favorites",
    tracks: "Tracks",
    songs: "Songs",
    artists: "Artists",
    albums: "Albums",
    next_up: "Next up",
    lyrics: "Lyrics",
    cover: "Cover",
    album: "Album",
    playlist: "Playlist",
    new_playlist: "New playlist",
    show_all: "Show all",
    other_versions: "Other versions",
    suggestions: "Suggestions",

    play: "Play",
    pause: "Pause",
    shuffle: "Shuffle",
    repeat: "Repeat",
    radio: "Radio",
    like: "Like",
    remove_like: "Remove like",
    dislike: "Dislike",
    save_to_library: "Save to library",
    saved: "Saved",
    refresh: "Refresh",
    back: "Back",
    forward: "Forward",
    minimize: "Minimize",
    fullscreen: "Fullscreen",
    cancel: "Cancel",
    create: "Create",
    close: "Close",
    open_folder: "Folder",
    check_updates: "Check for updates",
    sign_in: "Sign in",
    sign_out: "Sign out",
    signed_in: "Signed in",
    not_signed_in: "Not signed in",

    search_placeholder: "Search songs, artists, albums...",
    nothing_playing: "Nothing playing",
    pick_track_hint: "Pick a track or mix to listen",
    no_track_playing: "No track playing",
    loading_cover: "Loading HD cover art...",
    select_song_lyrics: "Select a song to load lyrics",
    fetching_lyrics: "Fetching lyrics...",
    no_lyrics_found: "No lyrics found for this track",
    queue_empty: "Queue is empty",
    playlist_empty: "This playlist has no tracks yet.",
    upgrading: "Upgrading...",
    low_power_mini: "Sonata · Low-Power Mini",
    synced_lyrics: "Synced lyrics",

    previous_track: "Previous track",
    next_track: "Next track",
    shuffle_on: "Shuffle: On (S)",
    shuffle_off: "Shuffle: Off (S)",
    repeat_off: "Repeat: Off (R)",
    repeat_all: "Repeat: All (R)",
    repeat_one: "Repeat: One (R)",
    mute: "Mute (M)",
    next_up_queue: "Next up queue",
    lyrics_panel: "Lyrics panel",
    now_playing_tip: "Now Playing (F)",

    tab_appearance: "Appearance",
    tab_playback: "Playback",
    tab_library: "Library",
    tab_system: "System",
    tab_account: "Account",
    tab_about: "About",
    language: "Language",
    language_desc: "Choose the interface language (defaults to English on Desktop & TUI).",
    theme_mode: "Theme Mode",
    theme_mode_desc: "Follow system dark/light preference or force a mode.",
    mode_system: "System",
    mode_dark: "Dark",
    mode_light: "Light",
    color_theme: "Color Theme",
    color_theme_desc: "Select a built-in skin or custom CSS theme from your themes folder.",
    reload_themes: "Reload CSS themes",
    transparency: "Transparency",
    transparency_desc: "Adjust translucent glass surface opacity over your desktop wallpaper.",
    blur: "Compositor Blur",
    blur_desc: "Enable Wayland backdrop blur behind translucent surfaces.",
    card_outline: "Card Outlines",
    card_outline_desc: "Draw subtle glass edge strokes around floating cards.",
    corners: "Corner Radius",
    corners_desc: "Choose panel and control corner rounding.",
    corner_round: "Round",
    corner_soft: "Soft",
    corner_sharp: "Sharp",
    edge_fades: "Scroll Edge Fades",
    edge_fades_desc: "Apply soft gradient fades at the top and bottom of scrollable pages.",
    adaptive_art_color: "Adaptive Artwork Color",
    adaptive_art_color_desc: "Tint album, playlist, and artist headers from cover artwork.",
    hifi_upgrade: "YouTube Music Quality Upgrade",
    hifi_upgrade_desc:
        "Upgrade cached and live streams in the background to Extreme / peak bitrate.",
    jiosaavn_module: "JioSaavn 320kbps Module",
    jiosaavn_module_desc:
        "Include JioSaavn results in search and upgrade <300kbps tracks to 320kbps AAC.",
    motion_covers: "Animated Album Artwork",
    motion_covers_desc: "Display looping Apple Music, Tidal & Community motion covers.",
    reduce_motion: "Reduce Motion",
    reduce_motion_desc: "Disable animated covers and minimize UI motion effects.",
    cache_limit: "Media Cache Limit",
    cache_limit_desc: "Cap on-disk audio, cover art & lyrics cache size (1–10 GB).",

    tui_album_art: " Album Art ",
    tui_album_art_fallback: " Album Art (Fallback) ",
    tui_motion_cover: " ✦ Motion Cover ",
    tui_lyrics_title: " Lyrics ",
    tui_settings_title: " ✦ Sonata Settings ",
    tui_sec_audio: "AUDIO & SOURCES",
    tui_sec_appearance: "APPEARANCE & COMPOSITOR",
    tui_sec_storage: "STORAGE & SYSTEM",
    tui_providers: "Music Providers",
    tui_providers_desc: "Choose which catalogs appear in Library and Search results.",
    tui_transparent_bg: "Transparent TUI Background",
    tui_transparent_bg_desc:
        "Use terminal background alpha/glass compositing instead of an opaque surface fill.",
    tui_hypr_opacity: "Hyprland Window Opacity",
    tui_hypr_opacity_desc:
        "Adjust dynamic compositor window opacity via hyprctl when running inside Hyprland.",
    tui_browser_session: "Browser Account Session",
    tui_browser_session_desc:
        "Synced cookies for library, Liked Songs & YTM Extreme. Press Enter/r to rescan.",
    tui_version_updates: "Version & Updates",
    tui_version_updates_desc:
        "Installed Sonata version and GitHub release checker (Enter or u to check now).",
    enabled: "Enabled",
    disabled: "Disabled",
    enabled_320k: "Enabled (320k)",
    static_label: "Static",
    enabled_glass: "Enabled (Glass)",
    solid_label: "Solid",
    up_to_date: "Up to date",
    check_update_hint: "Press Enter/u to check",
    tui_help_title: " Keybindings & Help ",
    tui_themes_title: " Color Themes ",
    tui_import_spotify_title: " Import Spotify Playlist ",
    tui_create_playlist_title: " Create Playlist ",
    tui_add_to_playlist_title: " Add to Playlist ",
    autoplay: "Endless Autoplay",
    autoplay_on: "Endless Autoplay: On",
    autoplay_off: "Endless Autoplay: Off",
    autoplay_desc: "Keep playing similar recommended tracks when a playlist or queue ends.",
    radio_mixes: "Radio & Mixes",
    quick_radio: "Quick Radio",
    moods_genres: "Moods & genres",
    recent_searches: "Recent searches",
    library_playlists: "Library Playlists",
    recently_played: "Recently Played",
    start_radio: "Start radio",
    play_next: "Play next",
    add_to_queue: "Add to queue",
    go_to_album: "Go to album",
    go_to_artist: "Go to artist",
    add_to_playlist: "Add to playlist",
    remove_from_playlist: "Remove from playlist",
    play_now: "Play now",
    top_songs: "Top songs",
    releases: "Releases",
    fans_might_like: "Fans might also like",
    you_liked: "You Liked",
    retry: "Retry",
    all_tab: "All",
    videos_tab: "Videos",
    youtube_music: "YouTube Music",
    your_library: "Your Library",
    sign_in_library_title: "Sign in to view your library",
    sign_in_library_desc:
        "Connect your browser YouTube Music session to sync liked songs, playlists, saved albums, and artists.",
    sign_in_browser: "Sign in with browser",
    dbl_click_play: "Double-click to play rows",
    dbl_click_play_desc: "Require a double-click on track rows instead of single-click playback",
    cache_budget: "Audio Media Cache Limit",
    cache_budget_desc: "Bounded SQLite/disk LRU storage for cached audio streams",
    pinned: "Pinned",
    pin: "Pin",
    unpin: "Unpin",
    find_your_mood: "Find your mood",
    in_library: "In Library",
    add_to_library: "Add to Library",
    remove_from_library: "Remove from Library",

    downloads: "Downloads",
    download: "Download",
    downloaded: "Downloaded",
    downloading: "Downloading...",
    remove_download: "Remove download",
    downloads_dir: "Downloads Folder",
    downloads_dir_desc: "Shared offline audio folder used by Sonata Desktop & TUI.",
    downloads_limit: "Offline Downloads Limit",
    downloads_limit_desc: "Cap shared offline downloads storage (0 = Unlimited).",
    downloads_usage: "Offline Storage Usage",
    clear_downloads: "Clear all",
    no_downloads: "No offline downloads yet",
    discord_rpc: "Discord Rich Presence",
    discord_rpc_desc: "Show currently playing track, timestamps, and speed on Discord.",
    discord_app_id: "Discord Application ID",
    discord_app_id_desc: "Optional custom Discord client ID (leave empty for default).",
    servers: "Servers",
    servers_desc: "Connect self-hosted Jellyfin, Subsonic/OpenSubsonic, and WebDAV servers.",
    add_server: "Add server",
    test_connection: "Test",
    remove_server: "Remove",
    no_servers: "No self-hosted music servers configured",
    connect_server_hint: "Add a Jellyfin, Subsonic, or WebDAV server in Settings.",
    unlimited: "Unlimited",
    reset_default: "Reset",
    save: "Save",

    music: "Music",
    video: "Video",
    video_quality: "Video quality",
    video_quality_desc: "Preferred resolution when switching to Music Video mode.",
    translate_lyrics: "Translate lyrics",
    translate_lyrics_desc: "Show line-by-line lyrics translation in your interface language.",
    romanize_lyrics: "Romanize lyrics",
    romanize_lyrics_desc: "Show Latin transliteration beneath non-Latin lyrics.",
    playback_speed: "Playback speed",
    playback_speed_desc: "Adjust playback tempo from 0.75x to 2.0x.",
    skip_silence: "Skip silence",
    skip_silence_desc: "Automatically skip silent gaps during audio-only playback.",
    sleep_timer: "Sleep timer",
    sleep_timer_desc: "Fade out and pause playback after a chosen duration.",
    off: "Off",
    auto_label: "Auto",
    loading_video: "Loading video...",
    video_unavailable: "Video unavailable",

    sources: "Sources",
    sources_desc: "Manage streaming catalogs, local folders, self-hosted servers, and modules.",
    modules: "Modules",
    modules_desc: "Optional provider modules that extend search and playback quality.",
    personal_only_mode: "Personal Only Mode",
    personal_only_mode_desc:
        "Hide streaming catalogs and use only local files and personal servers. Your streaming settings remain preserved.",
    local_music: "Local Music",
    local_music_desc:
        "Scan audio folders on this device (defaults to your Music folder when empty).",
    local_music_dirs: "Local Music Folders",
    add_folder: "Add folder",
    smb: "SMB",
    webdav: "WebDAV",
    jellyfin: "Jellyfin",
    subsonic: "Subsonic",
    server_alive: "Alive",
    server_online: "Online",
    server_offline: "Offline",
    speed_test: "Speed Test",
    latency: "Latency",
    latency_ms: "ms",
    throughput: "Throughput",
    throughput_mbps: "Mbps",
    personal_playlists: "Personal Playlists",
    personal_playlists_desc:
        "Local playlists stored on this device that mix local, server, and streaming tracks.",
    attribute_personal_to_ytm: "Report local/server plays to YouTube Music",
    attribute_personal_to_ytm_desc:
        "When signed in and not in Personal Only Mode, log matching local or server plays to YouTube Music history.",
    scan: "Scan",
    folders: "Folders",
    youtube: "YouTube",
    local: "Local",
    edit: "Edit",
    rename: "Rename",
    delete: "Delete",
    smb_share: "Share (optional)",
    smb_domain: "Domain (optional)",
    smb_mount_path: "Local Mount Path (optional)",
    personal_only_active:
        "Personal Only Mode is active — streaming feeds and playback are paused while your preferences remain preserved.",

    gapless_playback: "Gapless Playback",
    gapless_playback_desc: "Eliminate silent gaps between consecutive tracks during audio playback.",
    crossfade: "Crossfade",
    crossfade_desc: "Smoothly fade between consecutive audio tracks (0–12s, disabled in Video mode).",
    lastfm_scrobbling: "Last.fm Scrobbling",
    lastfm_scrobbling_desc: "Send Now Playing updates and scrobbles to your Last.fm profile.",
    listenbrainz_scrobbling: "ListenBrainz Scrobbling",
    listenbrainz_scrobbling_desc: "Submit listening history to ListenBrainz using your user token.",
    listening_stats: "Listening Stats",
    listening_stats_desc: "Private on-device play counts, streaks, and top charts with zero tracking.",
    total_plays: "Total Plays",
    listening_time: "Listening Time",
    streak_days: "Day Streak",
    top_artists: "Top Artists",
    top_tracks: "Top Tracks",
    top_albums: "Top Albums",
    sonata_connect: "Sonata Connect",
    sonata_connect_desc: "Control playback and transfer queues between Sonata devices over your local network.",
    connect_device_name: "Device Name",
    pairing_pin: "Pairing PIN",
    paired_devices: "Paired Devices",
};

pub const TR: UiStrings = UiStrings {
    home: "Ana Sayfa",
    explore: "Keşfet",
    library: "Kitaplık",
    search: "Ara",
    queue: "Sıra",
    history: "Geçmiş",
    settings: "Ayarlar",
    playlists: "Çalma Listeleri",
    liked_music: "Beğenilen Müzikler",
    favorites: "Favoriler",
    tracks: "Parçalar",
    songs: "Şarkılar",
    artists: "Sanatçılar",
    albums: "Albümler",
    next_up: "Sıradakiler",
    lyrics: "Şarkı Sözleri",
    cover: "Kapak",
    album: "Albüm",
    playlist: "Çalma Listesi",
    new_playlist: "Yeni çalma listesi",
    show_all: "Tümünü gör",
    other_versions: "Diğer sürümler",
    suggestions: "Öneriler",

    play: "Oynat",
    pause: "Duraklat",
    shuffle: "Karıştır",
    repeat: "Tekrarla",
    radio: "Radyo",
    like: "Beğen",
    remove_like: "Beğeniyi kaldır",
    dislike: "Beğenme",
    save_to_library: "Kitaplığa kaydet",
    saved: "Kaydedildi",
    refresh: "Yenile",
    back: "Geri",
    forward: "İleri",
    minimize: "Küçült",
    fullscreen: "Tam Ekran",
    cancel: "İptal",
    create: "Oluştur",
    close: "Kapat",
    open_folder: "Klasör",
    check_updates: "Güncellemeleri kontrol et",
    sign_in: "Giriş yap",
    sign_out: "Çıkış yap",
    signed_in: "Giriş yapıldı",
    not_signed_in: "Giriş yapılmadı",

    search_placeholder: "Şarkı, sanatçı, albüm ara...",
    nothing_playing: "Çalan parça yok",
    pick_track_hint: "Dinlemek için bir parça veya miks seç",
    no_track_playing: "Çalan parça yok",
    loading_cover: "HD kapak görseli yükleniyor...",
    select_song_lyrics: "Sözleri yüklemek için bir şarkı seç",
    fetching_lyrics: "Şarkı sözleri alınıyor...",
    no_lyrics_found: "Bu parça için şarkı sözü bulunamadı",
    queue_empty: "Sıra boş",
    playlist_empty: "Bu çalma listesinde henüz parça yok.",
    upgrading: "Yükseltiliyor...",
    low_power_mini: "Sonata · Düşük Güç Mini",
    synced_lyrics: "Senkron sözler",

    previous_track: "Önceki parça",
    next_track: "Sonraki parça",
    shuffle_on: "Karıştır: Açık (S)",
    shuffle_off: "Karıştır: Kapalı (S)",
    repeat_off: "Tekrarla: Kapalı (R)",
    repeat_all: "Tekrarla: Tümü (R)",
    repeat_one: "Tekrarla: Tek (R)",
    mute: "Sesi Kapat (M)",
    next_up_queue: "Sıradaki kuyruk",
    lyrics_panel: "Şarkı sözü paneli",
    now_playing_tip: "Şimdi Çalıyor (F)",

    tab_appearance: "Görünüm",
    tab_playback: "Oynatma",
    tab_library: "Kitaplık",
    tab_system: "Sistem",
    tab_account: "Hesap",
    tab_about: "Hakkında",
    language: "Dil",
    language_desc: "Arayüz dilini seç (Masaüstü ve TUI varsayılanı İngilizce).",
    theme_mode: "Tema Modu",
    theme_mode_desc: "Sistem koyu/açık tercihini izle veya bir modu sabitle.",
    mode_system: "Sistem",
    mode_dark: "Koyu",
    mode_light: "Açık",
    color_theme: "Renk Teması",
    color_theme_desc: "Yerleşik bir tema veya themes klasöründen özel CSS seç.",
    reload_themes: "CSS temalarını yenile",
    transparency: "Saydamlık",
    transparency_desc: "Masaüstü arka planı üzerindeki cam saydamlığını ayarla.",
    blur: "Arka Plan Bulanıklığı",
    blur_desc: "Saydam yüzeylerin arkasında Wayland bulanıklığını etkinleştir.",
    card_outline: "Kart Kenarlıkları",
    card_outline_desc: "Yüzen kartların etrafına ince cam kenarlık çiz.",
    corners: "Köşe Yuvarlaklığı",
    corners_desc: "Panel ve denetim köşe yuvarlaklığını seç.",
    corner_round: "Yuvarlak",
    corner_soft: "Yumuşak",
    corner_sharp: "Keskin",
    edge_fades: "Kaydırma Kenar Solması",
    edge_fades_desc: "Kaydırılabilir sayfaların üst ve altında yumuşak geçiş uygula.",
    adaptive_art_color: "Uyarlanabilir Kapak Rengi",
    adaptive_art_color_desc: "Albüm, çalma listesi ve sanatçı başlıklarını kapak rengiyle tonla.",
    hifi_upgrade: "YouTube Music Kalite Yükseltme",
    hifi_upgrade_desc: "Önbellekli ve canlı akışları arka planda en yüksek bit hızına yükselt.",
    jiosaavn_module: "JioSaavn 320kbps Modülü",
    jiosaavn_module_desc: "Aramalara JioSaavn ekle ve <300kbps parçaları 320kbps AAC'ye yükselt.",
    motion_covers: "Hareketli Albüm Kapağı",
    motion_covers_desc: "Apple Music, Tidal ve Topluluk hareketli kapaklarını göster.",
    reduce_motion: "Hareketi Azalt",
    reduce_motion_desc: "Hareketli kapakları kapat ve arayüz animasyonlarını azalt.",
    cache_limit: "Medya Önbellek Sınırı",
    cache_limit_desc: "Disk üzerindeki ses, kapak ve söz önbelleğini sınırla (1–10 GB).",

    tui_album_art: " Albüm Kapağı ",
    tui_album_art_fallback: " Albüm Kapağı (Yedek) ",
    tui_motion_cover: " ✦ Hareketli Kapak ",
    tui_lyrics_title: " Şarkı Sözleri ",
    tui_settings_title: " ✦ Sonata Ayarlar ",
    tui_sec_audio: "SES VE KAYNAKLAR",
    tui_sec_appearance: "GÖRÜNÜM VE DİL",
    tui_sec_storage: "DEPOLAMA VE SİSTEM",
    tui_providers: "Müzik Sağlayıcıları",
    tui_providers_desc: "Kitaplık ve Arama sonuçlarında görünecek katalogları seç.",
    tui_transparent_bg: "Saydam TUI Arka Planı",
    tui_transparent_bg_desc: "Opak dolgu yerine terminal arka plan saydamlığını kullan.",
    tui_hypr_opacity: "Hyprland Pencere Opaklığı",
    tui_hypr_opacity_desc: "Hyprland içinde hyprctl ile dinamik pencere opaklığını ayarla.",
    tui_browser_session: "Tarayıcı Hesap Oturumu",
    tui_browser_session_desc:
        "Kitaplık ve Beğenilenler için çerez oturumu. Yenilemek için Enter/r bas.",
    tui_version_updates: "Sürüm ve Güncellemeler",
    tui_version_updates_desc: "Yüklü Sonata sürümü ve GitHub güncelleme denetimi (Enter veya u).",
    enabled: "Açık",
    disabled: "Kapalı",
    enabled_320k: "Açık (320k)",
    static_label: "Sabit",
    enabled_glass: "Açık (Cam)",
    solid_label: "Opak",
    up_to_date: "Güncel",
    check_update_hint: "Denetlemek için Enter/u bas",
    tui_help_title: " Kısayollar ve Yardım ",
    tui_themes_title: " Renk Temaları ",
    tui_import_spotify_title: " Spotify Çalma Listesi Aktar ",
    tui_create_playlist_title: " Çalma Listesi Oluştur ",
    tui_add_to_playlist_title: " Çalma Listesine Ekle ",
    autoplay: "Sonsuz Çalma",
    autoplay_on: "Sonsuz Çalma: Açık",
    autoplay_off: "Sonsuz Çalma: Kapalı",
    autoplay_desc: "Çalma listesi veya kuyruk bittiğinde benzer önerilen parçaları çalmaya devam et.",
    radio_mixes: "Radyo ve Miksler",
    quick_radio: "Hızlı Radyo",
    moods_genres: "Ruh halleri ve türler",
    recent_searches: "Son aramalar",
    library_playlists: "Kitaplık Çalma Listeleri",
    recently_played: "Son Çalınanlar",
    start_radio: "Radyo başlat",
    play_next: "Sonraki çal",
    add_to_queue: "Sıraya ekle",
    go_to_album: "Albüme git",
    go_to_artist: "Sanatçıya git",
    add_to_playlist: "Çalma listesine ekle",
    remove_from_playlist: "Çalma listesinden kaldır",
    play_now: "Şimdi oynat",
    top_songs: "Popüler şarkılar",
    releases: "Yayınlar",
    fans_might_like: "Benzer sanatçılar",
    you_liked: "Beğendiklerin",
    retry: "Tekrar dene",
    all_tab: "Tümü",
    videos_tab: "Videolar",
    youtube_music: "YouTube Music",
    your_library: "Kitaplığın",
    sign_in_library_title: "Kitaplığını görmek için giriş yap",
    sign_in_library_desc:
        "Beğenilen şarkıları, çalma listelerini, albümleri ve sanatçıları eşitlemek için tarayıcı YouTube Music oturumunu bağla.",
    sign_in_browser: "Tarayıcıyla giriş yap",
    dbl_click_play: "Satırları çift tıklamayla oynat",
    dbl_click_play_desc: "Tek tıklama yerine parça satırlarında çift tıklama iste",
    cache_budget: "Ses Medya Önbellek Sınırı",
    cache_budget_desc: "Önbellekli ses akışları için sınırlı SQLite/disk LRU deposu",
    pinned: "Sabitlenenler",
    pin: "Sabitle",
    unpin: "Sabitlemeyi kaldır",
    find_your_mood: "Ruh halini bul",
    in_library: "Kitaplıkta",
    add_to_library: "Kitaplığa ekle",
    remove_from_library: "Kitaplıktan çıkar",

    downloads: "İndirilenler",
    download: "İndir",
    downloaded: "İndirildi",
    downloading: "İndiriliyor...",
    remove_download: "İndirmeyi kaldır",
    downloads_dir: "İndirilenler Klasörü",
    downloads_dir_desc: "Sonata Masaüstü ve TUI tarafından paylaşılan çevrimdışı ses klasörü.",
    downloads_limit: "Çevrimdışı İndirme Sınırı",
    downloads_limit_desc: "Paylaşılan çevrimdışı indirme depolama sınırını ayarla (0 = Sınırsız).",
    downloads_usage: "Çevrimdışı Depolama Kullanımı",
    clear_downloads: "Tümünü temizle",
    no_downloads: "Henüz çevrimdışı indirme yok",
    discord_rpc: "Discord Rich Presence",
    discord_rpc_desc: "Çalan parçayı, zaman damgalarını ve hızı Discord'da göster.",
    discord_app_id: "Discord Uygulama Kimliği",
    discord_app_id_desc: "İsteğe bağlı özel Discord istemci kimliği (varsayılan için boş bırak).",
    servers: "Sunucular",
    servers_desc: "Kendi barındırdığın Jellyfin, Subsonic/OpenSubsonic ve WebDAV sunucularını bağla.",
    add_server: "Sunucu ekle",
    test_connection: "Test et",
    remove_server: "Kaldır",
    no_servers: "Yapılandırılmış sunucu yok",
    connect_server_hint: "Ayarlar'dan bir Jellyfin, Subsonic veya WebDAV sunucusu ekle.",
    unlimited: "Sınırsız",
    reset_default: "Sıfırla",
    save: "Kaydet",

    music: "Müzik",
    video: "Video",
    video_quality: "Video kalitesi",
    video_quality_desc: "Müzik Videosu moduna geçildiğinde tercih edilen çözünürlük.",
    translate_lyrics: "Sözleri çevir",
    translate_lyrics_desc: "Şarkı sözlerini arayüz diline satır satır çevirerek göster.",
    romanize_lyrics: "Sözleri Latinleştir",
    romanize_lyrics_desc: "Latin alfabesi dışındaki şarkı sözlerinin altında okunuşu göster.",
    playback_speed: "Oynatma hızı",
    playback_speed_desc: "Oynatma hızını 0.75x ile 2.0x arasında ayarla.",
    skip_silence: "Sessizliği atla",
    skip_silence_desc: "Yalnızca ses çalarken sessiz boşlukları otomatik olarak atla.",
    sleep_timer: "Uyku zamanlayıcısı",
    sleep_timer_desc: "Seçilen sürenin sonunda sesi kısarak oynatmayı duraklat.",
    off: "Kapalı",
    auto_label: "Otomatik",
    loading_video: "Video yükleniyor...",
    video_unavailable: "Video kullanılamıyor",

    sources: "Kaynaklar",
    sources_desc: "Akış kataloglarını, yerel klasörleri, kendi sunucularını ve modülleri yönet.",
    modules: "Modüller",
    modules_desc: "Arama ve oynatma kalitesini genişleten isteğe bağlı sağlayıcı modülleri.",
    personal_only_mode: "Yalnızca Kişisel Mod",
    personal_only_mode_desc:
        "Akış kataloglarını gizleyip yalnızca yerel dosyaları ve kişisel sunucuları kullan. Akış ayarların korunur.",
    local_music: "Yerel Müzik",
    local_music_desc:
        "Bu cihazdaki ses klasörlerini tara (boş bırakılırsa Müzik klasörü kullanılır).",
    local_music_dirs: "Yerel Müzik Klasörleri",
    add_folder: "Klasör ekle",
    smb: "SMB",
    webdav: "WebDAV",
    jellyfin: "Jellyfin",
    subsonic: "Subsonic",
    server_alive: "Erişilebilir",
    server_online: "Çevrimiçi",
    server_offline: "Çevrimdışı",
    speed_test: "Hız Testi",
    latency: "Gecikme",
    latency_ms: "ms",
    throughput: "Aktarım Hızı",
    throughput_mbps: "Mbps",
    personal_playlists: "Kişisel Listeler",
    personal_playlists_desc:
        "Bu cihazda saklanan; yerel, sunucu ve akış parçalarını birleştirebilen listeler.",
    attribute_personal_to_ytm: "Yerel/sunucu çalmalarını YouTube Music geçmişine işle",
    attribute_personal_to_ytm_desc:
        "Oturum açıkken ve Yalnızca Kişisel Mod kapalıyken eşleşen yerel veya sunucu çalmalarını YouTube Music geçmişine kaydet.",
    scan: "Tara",
    folders: "Klasörler",
    youtube: "YouTube",
    local: "Yerel",
    edit: "Düzenle",
    rename: "Yeniden Adlandır",
    delete: "Sil",
    smb_share: "Paylaşım (isteğe bağlı)",
    smb_domain: "Etki Alanı (isteğe bağlı)",
    smb_mount_path: "Yerel Bağlama Yolu (isteğe bağlı)",
    personal_only_active:
        "Yalnızca Kişisel Mod etkin — tercihlerin korunurken akış katalogları ve oynatma duraklatıldı.",

    gapless_playback: "Kesintisiz Oynatma",
    gapless_playback_desc: "Ses oynatırken art arda gelen parçalar arasındaki sessiz boşlukları kaldır.",
    crossfade: "Çapraz Geçiş",
    crossfade_desc: "Ardışık ses parçaları arasında yumuşak geçiş yap (0–12sn, Video modunda kapalı).",
    lastfm_scrobbling: "Last.fm Skroblama",
    lastfm_scrobbling_desc: "Şimdi Çalıyor güncellemelerini ve dinleme kayıtlarını Last.fm hesabına gönder.",
    listenbrainz_scrobbling: "ListenBrainz Skroblama",
    listenbrainz_scrobbling_desc: "Kullanıcı belirtecinle dinleme geçmişini ListenBrainz'e gönder.",
    listening_stats: "Dinleme İstatistikleri",
    listening_stats_desc: "İzleyici olmadan cihaz içinde tutulan çalma sayısı, seri ve en çok dinlenenler.",
    total_plays: "Toplam Çalma",
    listening_time: "Dinleme Süresi",
    streak_days: "Günlük Seri",
    top_artists: "En Çok Dinlenen Sanatçılar",
    top_tracks: "En Çok Dinlenen Parçalar",
    top_albums: "En Çok Dinlenen Albümler",
    sonata_connect: "Sonata Connect",
    sonata_connect_desc: "Yerel ağ üzerinden Sonata cihazları arasında oynatmayı kontrol et ve sırayı aktar.",
    connect_device_name: "Cihaz Adı",
    pairing_pin: "Eşleştirme PIN'i",
    paired_devices: "Eşleştirilmiş Cihazlar",
};

pub const RU: UiStrings = UiStrings {
    home: "Главная",
    explore: "Обзор",
    library: "Медиатека",
    search: "Поиск",
    queue: "Очередь",
    history: "История",
    settings: "Настройки",
    playlists: "Плейлисты",
    liked_music: "Понравившиеся",
    favorites: "Избранное",
    tracks: "Треки",
    songs: "Песни",
    artists: "Исполнители",
    albums: "Альбомы",
    next_up: "Далее",
    lyrics: "Текст песни",
    cover: "Обложка",
    album: "Альбом",
    playlist: "Плейлист",
    new_playlist: "Новый плейлист",
    show_all: "Показать все",
    other_versions: "Другие версии",
    suggestions: "Подсказки",

    play: "Слушать",
    pause: "Пауза",
    shuffle: "Перемешать",
    repeat: "Повтор",
    radio: "Радио",
    like: "Нравится",
    remove_like: "Убрать лайк",
    dislike: "Не нравится",
    save_to_library: "В медиатеку",
    saved: "Сохранено",
    refresh: "Обновить",
    back: "Назад",
    forward: "Вперёд",
    minimize: "Свернуть",
    fullscreen: "На весь экран",
    cancel: "Отмена",
    create: "Создать",
    close: "Закрыть",
    open_folder: "Папка",
    check_updates: "Проверить обновления",
    sign_in: "Войти",
    sign_out: "Выйти",
    signed_in: "Вход выполнен",
    not_signed_in: "Не выполнен вход",

    search_placeholder: "Поиск песен, исполнителей, альбомов...",
    nothing_playing: "Ничего не играет",
    pick_track_hint: "Выберите трек или микс для прослушивания",
    no_track_playing: "Ничего не играет",
    loading_cover: "Загрузка HD-обложки...",
    select_song_lyrics: "Выберите песню для загрузки текста",
    fetching_lyrics: "Загрузка текста...",
    no_lyrics_found: "Текст для этого трека не найден",
    queue_empty: "Очередь пуста",
    playlist_empty: "В этом плейлисте пока нет треков.",
    upgrading: "Улучшение...",
    low_power_mini: "Sonata · Эко Мини",
    synced_lyrics: "Синхронный текст",

    previous_track: "Предыдущий трек",
    next_track: "Следующий трек",
    shuffle_on: "Перемешать: Вкл (S)",
    shuffle_off: "Перемешать: Выкл (S)",
    repeat_off: "Повтор: Выкл (R)",
    repeat_all: "Повтор: Все (R)",
    repeat_one: "Повтор: Один (R)",
    mute: "Без звука (M)",
    next_up_queue: "Очередь воспроизведения",
    lyrics_panel: "Панель текста",
    now_playing_tip: "Сейчас играет (F)",

    tab_appearance: "Внешний вид",
    tab_playback: "Воспроизведение",
    tab_library: "Медиатека",
    tab_system: "Система",
    tab_account: "Аккаунт",
    tab_about: "О приложении",
    language: "Язык",
    language_desc: "Выберите язык интерфейса (на ПК и TUI по умолчанию English).",
    theme_mode: "Режим темы",
    theme_mode_desc: "Следовать системной теме или выбрать вручную.",
    mode_system: "Система",
    mode_dark: "Тёмная",
    mode_light: "Светлая",
    color_theme: "Цветовая тема",
    color_theme_desc: "Выберите встроенную тему или свой CSS из папки themes.",
    reload_themes: "Обновить CSS-темы",
    transparency: "Прозрачность",
    transparency_desc: "Настройте прозрачность стеклянных панелей.",
    blur: "Размытие фона",
    blur_desc: "Включить системное размытие Wayland под прозрачными окнами.",
    card_outline: "Границы карточек",
    card_outline_desc: "Рисовать тонкий стеклянный контур вокруг панелей.",
    corners: "Скругление углов",
    corners_desc: "Выберите стиль скругления панелей и элементов.",
    corner_round: "Округлые",
    corner_soft: "Мягкие",
    corner_sharp: "Острые",
    edge_fades: "Затухание краёв",
    edge_fades_desc: "Плавное затухание списков у верхнего и нижнего краёв.",
    adaptive_art_color: "Адаптивный цвет обложки",
    adaptive_art_color_desc: "Окрашивать шапки альбомов и плейлистов в тон обложки.",
    hifi_upgrade: "Улучшение качества YouTube Music",
    hifi_upgrade_desc: "В фоне улучшать потоки и кэш до максимального битрейта.",
    jiosaavn_module: "Модуль JioSaavn 320kbps",
    jiosaavn_module_desc: "Искать в JioSaavn и улучшать треки <300kbps до 320kbps AAC.",
    motion_covers: "Анимированные обложки",
    motion_covers_desc: "Показывать живые обложки Apple Music, Tidal и сообщества.",
    reduce_motion: "Уменьшить движение",
    reduce_motion_desc: "Отключить живые обложки и свести анимации к минимуму.",
    cache_limit: "Лимит медиа-кэша",
    cache_limit_desc: "Ограничить размер кэша аудио, обложек и текстов (1–10 ГБ).",

    tui_album_art: " Обложка ",
    tui_album_art_fallback: " Обложка (Базовая) ",
    tui_motion_cover: " ✦ Живая обложка ",
    tui_lyrics_title: " Текст песни ",
    tui_settings_title: " ✦ Настройки Sonata ",
    tui_sec_audio: "АУДИО И ИСТОЧНИКИ",
    tui_sec_appearance: "ВИД И ЯЗЫК",
    tui_sec_storage: "ХРАНИЛИЩЕ И СИСТЕМА",
    tui_providers: "Музыкальные каталоги",
    tui_providers_desc: "Выберите каталоги для медиатеки и поиска.",
    tui_transparent_bg: "Прозрачный фон TUI",
    tui_transparent_bg_desc: "Использовать прозрачность терминала вместо сплошной заливки.",
    tui_hypr_opacity: "Непрозрачность Hyprland",
    tui_hypr_opacity_desc: "Динамическая прозрачность окна через hyprctl в Hyprland.",
    tui_browser_session: "Сессия браузера",
    tui_browser_session_desc:
        "Синхронизация cookie для медиатеки и YTM Extreme. Enter/r для обновления.",
    tui_version_updates: "Версия и обновления",
    tui_version_updates_desc:
        "Установленная версия Sonata и проверка релизов GitHub (Enter или u).",
    enabled: "Вкл",
    disabled: "Выкл",
    enabled_320k: "Вкл (320k)",
    static_label: "Статично",
    enabled_glass: "Вкл (Стекло)",
    solid_label: "Сплошной",
    up_to_date: "Актуально",
    check_update_hint: "Нажмите Enter/u для проверки",
    tui_help_title: " Клавиши и помощь ",
    tui_themes_title: " Цветовые темы ",
    tui_import_spotify_title: " Импорт плейлиста Spotify ",
    tui_create_playlist_title: " Создать плейлист ",
    tui_add_to_playlist_title: " Добавить в плейлист ",
    autoplay: "Бесконечное воспроизведение",
    autoplay_on: "Бесконечное воспроизведение: Вкл",
    autoplay_off: "Бесконечное воспроизведение: Выкл",
    autoplay_desc: "Продолжать похожими треками после окончания плейлиста или очереди.",
    radio_mixes: "Радио и миксы",
    quick_radio: "Быстрое радио",
    moods_genres: "Настроения и жанры",
    recent_searches: "Недавние запросы",
    library_playlists: "Плейлисты медиатеки",
    recently_played: "Недавно прослушанные",
    start_radio: "Запустить радио",
    play_next: "Включить следующим",
    add_to_queue: "Добавить в очередь",
    go_to_album: "К альбому",
    go_to_artist: "К исполнителю",
    add_to_playlist: "Добавить в плейлист",
    remove_from_playlist: "Удалить из плейлиста",
    play_now: "Слушать сейчас",
    top_songs: "Популярные треки",
    releases: "Релизы",
    fans_might_like: "Похожие исполнители",
    you_liked: "Понравилось",
    retry: "Повторить",
    all_tab: "Все",
    videos_tab: "Видео",
    youtube_music: "YouTube Music",
    your_library: "Медиатека",
    sign_in_library_title: "Войдите, чтобы открыть медиатеку",
    sign_in_library_desc:
        "Подключите сессию YouTube Music из браузера для синхронизации лайков, плейлистов, альбомов и исполнителей.",
    sign_in_browser: "Войти через браузер",
    dbl_click_play: "Двойной клик для воспроизведения",
    dbl_click_play_desc: "Запускать треки двойным кликом вместо одиночного",
    cache_budget: "Лимит кэша аудио",
    cache_budget_desc: "Ограниченное хранилище SQLite/диска для кэшированных потоков",
    pinned: "Закреплённые",
    pin: "Закрепить",
    unpin: "Открепить",
    find_your_mood: "Под настроение",
    in_library: "В медиатеке",
    add_to_library: "Добавить в медиатеку",
    remove_from_library: "Убрать из медиатеки",

    downloads: "Загрузки",
    download: "Скачать",
    downloaded: "Скачано",
    downloading: "Загрузка...",
    remove_download: "Удалить загрузку",
    downloads_dir: "Папка загрузок",
    downloads_dir_desc: "Общая папка офлайн-аудио для Sonata Desktop и TUI.",
    downloads_limit: "Лимит офлайн-загрузок",
    downloads_limit_desc: "Ограничить размер хранилища загрузок (0 = Без лимита).",
    downloads_usage: "Использование хранилища",
    clear_downloads: "Очистить всё",
    no_downloads: "Пока нет офлайн-загрузок",
    discord_rpc: "Discord Rich Presence",
    discord_rpc_desc: "Показывать текущий трек, время и скорость в Discord.",
    discord_app_id: "ID приложения Discord",
    discord_app_id_desc: "Необязательный свой ID клиента Discord (пусто — по умолчанию).",
    servers: "Серверы",
    servers_desc: "Подключите свои серверы Jellyfin, Subsonic/OpenSubsonic и WebDAV.",
    add_server: "Добавить сервер",
    test_connection: "Проверить",
    remove_server: "Удалить",
    no_servers: "Нет настроенных серверов",
    connect_server_hint: "Добавьте сервер Jellyfin, Subsonic или WebDAV в настройках.",
    unlimited: "Без лимита",
    reset_default: "Сбросить",
    save: "Сохранить",

    music: "Музыка",
    video: "Видео",
    video_quality: "Качество видео",
    video_quality_desc: "Предпочитаемое разрешение в режиме музыкального видео.",
    translate_lyrics: "Перевести текст",
    translate_lyrics_desc: "Показывать построчный перевод текста на языке интерфейса.",
    romanize_lyrics: "Транслитерация текста",
    romanize_lyrics_desc: "Показывать латинскую транслитерацию под нелатинским текстом.",
    playback_speed: "Скорость воспроизведения",
    playback_speed_desc: "Настройка темпа воспроизведения от 0.75x до 2.0x.",
    skip_silence: "Пропуск тишины",
    skip_silence_desc: "Автоматически пропускать тихие паузы при воспроизведении аудио.",
    sleep_timer: "Таймер сна",
    sleep_timer_desc: "Плавно уменьшить громкость и поставить на паузу по таймеру.",
    off: "Выкл",
    auto_label: "Авто",
    loading_video: "Загрузка видео...",
    video_unavailable: "Видео недоступно",

    sources: "Источники",
    sources_desc: "Управление каталогами, локальными папками, серверами и модулями.",
    modules: "Модули",
    modules_desc: "Дополнительные модули провайдеров для поиска и качества звука.",
    personal_only_mode: "Только личный режим",
    personal_only_mode_desc:
        "Скрыть потоковые каталоги и использовать только локальные файлы и серверы. Ваши настройки стриминга сохраняются.",
    local_music: "Локальная музыка",
    local_music_desc:
        "Сканировать папки с аудио на устройстве (по умолчанию папка «Музыка»).",
    local_music_dirs: "Папки локальной музыки",
    add_folder: "Добавить папку",
    smb: "SMB",
    webdav: "WebDAV",
    jellyfin: "Jellyfin",
    subsonic: "Subsonic",
    server_alive: "Доступен",
    server_online: "В сети",
    server_offline: "Не в сети",
    speed_test: "Тест скорости",
    latency: "Задержка",
    latency_ms: "ms",
    throughput: "Пропускная способность",
    throughput_mbps: "Mbps",
    personal_playlists: "Личные плейлисты",
    personal_playlists_desc:
        "Локальные плейлисты на устройстве с локальными, серверными и потоковыми треками.",
    attribute_personal_to_ytm: "Отправлять локальные/серверные прослушивания в YouTube Music",
    attribute_personal_to_ytm_desc:
        "При входе и выключенном личном режиме записывать совпавшие треки в историю YouTube Music.",
    scan: "Сканировать",
    folders: "Папки",
    youtube: "YouTube",
    local: "Локальная",
    edit: "Изменить",
    rename: "Переименовать",
    delete: "Удалить",
    smb_share: "Общая папка (необяз.)",
    smb_domain: "Домен (необяз.)",
    smb_mount_path: "Локальный путь монтирования (необяз.)",
    personal_only_active:
        "Включён только личный режим — потоковые каталоги и воспроизведение приостановлены, ваши настройки сохранены.",

    gapless_playback: "Воспроизведение без пауз",
    gapless_playback_desc: "Убирать тишину между соседними треками при воспроизведении аудио.",
    crossfade: "Кроссфейд",
    crossfade_desc: "Плавное перетекание громкости между треками (0–12с, отключён в режиме видео).",
    lastfm_scrobbling: "Скробблинг Last.fm",
    lastfm_scrobbling_desc: "Отправлять статус «Сейчас играет» и скробблы в ваш профиль Last.fm.",
    listenbrainz_scrobbling: "Скробблинг ListenBrainz",
    listenbrainz_scrobbling_desc: "Отправлять историю прослушиваний в ListenBrainz по вашему токену.",
    listening_stats: "Статистика прослушиваний",
    listening_stats_desc: "Локальные счётчики, серии дней и топы на устройстве без телеметрии.",
    total_plays: "Всего прослушиваний",
    listening_time: "Время прослушивания",
    streak_days: "Серия дней",
    top_artists: "Топ исполнителей",
    top_tracks: "Топ треков",
    top_albums: "Топ альбомов",
    sonata_connect: "Sonata Connect",
    sonata_connect_desc: "Управляйте воспроизведением и передавайте очередь между устройствами Sonata в локальной сети.",
    connect_device_name: "Имя устройства",
    pairing_pin: "PIN-код сопряжения",
    paired_devices: "Сопряжённые устройства",
};

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
    }
}
