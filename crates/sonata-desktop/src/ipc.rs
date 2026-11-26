//! Typed Tauri v2 IPC layer bridging `BackendHandle` / `DesktopSnapshot` to the React webview.
//!
//! 1. **DTO Projections**: Serializes core/runtime snapshot structures (`UiSnapshotDto`, `ProgressTickDto`,
//!    `BackendStreamEvent`) while stripping raw in-memory frame buffers and secret scrobbler fields.
//! 2. **Streaming State**: Deduplicates snapshots so 40ms `position_secs` playback ticks only emit lightweight
//!    ~10Hz `ProgressTickDto` events over `tauri::ipc::Channel`.
//! 3. **Commands**: Exposes `dispatch_cmd`, settings persistence, platform identification, and frameless
//!    window controls to the frontend.

use crate::backend::{BackendCmd, BackendHandle, DesktopSnapshot, NavTarget, UiWindowCmd};
use crate::settings::DesktopSettings;
use serde::{Deserialize, Serialize};
use sonata_core::lyrics::{LyricLine, LyricVoice, LyricsState};
use sonata_core::player::RepeatMode;
use sonata_core::playlists::PersonalPlaylist;
use sonata_core::providers::hifi::VideoQualityStream;
use sonata_core::providers::servers::{
    ServerHealthReport, ServerKind, ServerLibrary, ServerProfile, ServerSpeedTestReport,
};
use sonata_core::providers::ytmusic::{
    AlbumDetail, ArtistPage, HomeCard, HomeCardKind, HomeShelf, MoodCategoryTile, MoodChip,
    RichSearchResult, YtAccountProfile,
};
use sonata_core::providers::{Playlist, ProviderSource, Track};
use sonata_core::stats::{StatsSummary, StatsWindow};
use sonata_core::updater::UpdateCheckResult;
use sonata_runtime::auth::BrowserOption;
use sonata_runtime::config::{AppConfig, ProviderMode};
use sonata_runtime::connect::{PairedDeviceInfo, PendingPairRequest};
use sonata_runtime::discord::DiscordRpcStatus;
use sonata_runtime::downloads::{DownloadedCollection, DownloadedTrack};
use sonata_runtime::i18n::AppLanguage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

pub struct AppState {
    pub backend: BackendHandle,
    pub win_tx: tokio::sync::mpsc::UnboundedSender<UiWindowCmd>,
    pub settings: Mutex<DesktopSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NavTargetDto {
    Home,
    Explore,
    Radio,
    Library,
    History,
    Stats,
    Search,
    Queue,
    Playlist {
        id: String,
    },
    Album {
        id: String,
    },
    Artist {
        id: String,
    },
    MoodCategory {
        title: String,
        browse_id: String,
        params: String,
    },
    ShelfDetail {
        title: String,
    },
    Settings,
}

impl From<&NavTarget> for NavTargetDto {
    fn from(nav: &NavTarget) -> Self {
        match nav {
            NavTarget::Home => Self::Home,
            NavTarget::Explore => Self::Explore,
            NavTarget::Radio => Self::Radio,
            NavTarget::Library => Self::Library,
            NavTarget::History => Self::History,
            NavTarget::Stats => Self::Stats,
            NavTarget::Search => Self::Search,
            NavTarget::Queue => Self::Queue,
            NavTarget::Playlist(id) => Self::Playlist { id: id.clone() },
            NavTarget::Album(id) => Self::Album { id: id.clone() },
            NavTarget::Artist(id) => Self::Artist { id: id.clone() },
            NavTarget::MoodCategory {
                title,
                browse_id,
                params,
            } => Self::MoodCategory {
                title: title.clone(),
                browse_id: browse_id.clone(),
                params: params.clone(),
            },
            NavTarget::ShelfDetail { title } => Self::ShelfDetail {
                title: title.clone(),
            },
            NavTarget::Settings => Self::Settings,
        }
    }
}

impl From<NavTargetDto> for NavTarget {
    fn from(nav: NavTargetDto) -> Self {
        match nav {
            NavTargetDto::Home => Self::Home,
            NavTargetDto::Explore => Self::Explore,
            NavTargetDto::Radio => Self::Radio,
            NavTargetDto::Library => Self::Library,
            NavTargetDto::History => Self::History,
            NavTargetDto::Stats => Self::Stats,
            NavTargetDto::Search => Self::Search,
            NavTargetDto::Queue => Self::Queue,
            NavTargetDto::Playlist { id } => Self::Playlist(id),
            NavTargetDto::Album { id } => Self::Album(id),
            NavTargetDto::Artist { id } => Self::Artist(id),
            NavTargetDto::MoodCategory {
                title,
                browse_id,
                params,
            } => Self::MoodCategory {
                title,
                browse_id,
                params,
            },
            NavTargetDto::ShelfDetail { title } => Self::ShelfDetail { title },
            NavTargetDto::Settings => Self::Settings,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiBrowserOptionDto {
    pub id: String,
    pub label: String,
    pub detected: bool,
}

impl From<&BrowserOption> for UiBrowserOptionDto {
    fn from(b: &BrowserOption) -> Self {
        Self {
            id: b.id.to_string(),
            label: b.label.to_string(),
            detected: b.detected,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiYtAccountProfileDto {
    pub name: Option<String>,
    pub handle: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<&YtAccountProfile> for UiYtAccountProfileDto {
    fn from(p: &YtAccountProfile) -> Self {
        Self {
            name: (!p.name.is_empty()).then(|| p.name.clone()),
            handle: p.handle.clone(),
            avatar_url: p.avatar_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiHomeCardDto {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub kind: String,
    pub is_playlist: bool,
    pub is_album: bool,
    pub is_artist: bool,
    pub source: ProviderSource,
    pub browse_params: Option<String>,
    pub track: Option<Track>,
}

impl From<&HomeCard> for UiHomeCardDto {
    fn from(c: &HomeCard) -> Self {
        let source = c
            .track
            .as_ref()
            .map(|t| t.source)
            .unwrap_or(ProviderSource::YouTubeMusic);
        let kind = match c.kind {
            HomeCardKind::Playlist => "Playlist",
            HomeCardKind::Album => "Album",
            HomeCardKind::Artist => "Artist",
            HomeCardKind::Song => "Song",
            HomeCardKind::Video => "Video",
            HomeCardKind::Mood => "Mood",
        };
        Self {
            id: c.id.clone(),
            title: c.title.clone(),
            subtitle: c.subtitle.clone(),
            thumbnail_url: c.thumbnail_url.clone(),
            kind: kind.to_string(),
            is_playlist: c.kind == HomeCardKind::Playlist,
            is_album: c.kind == HomeCardKind::Album,
            is_artist: c.kind == HomeCardKind::Artist,
            source,
            browse_params: None,
            track: c.track.clone(),
        }
    }
}

impl From<UiHomeCardDto> for HomeCard {
    fn from(c: UiHomeCardDto) -> Self {
        let kind = match c.kind.as_str() {
            "Artist" => HomeCardKind::Artist,
            "Album" => HomeCardKind::Album,
            "Song" => HomeCardKind::Song,
            "Video" => HomeCardKind::Video,
            "Mood" => HomeCardKind::Mood,
            "Playlist" => HomeCardKind::Playlist,
            _ if c.is_artist => HomeCardKind::Artist,
            _ if c.is_album => HomeCardKind::Album,
            _ => HomeCardKind::Playlist,
        };
        Self {
            kind,
            id: c.id,
            title: c.title,
            subtitle: c.subtitle,
            thumbnail_url: c.thumbnail_url,
            track: c.track,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiHomeShelfDto {
    pub title: String,
    pub cards: Vec<UiHomeCardDto>,
    pub tracks: Vec<Track>,
}

impl From<&HomeShelf> for UiHomeShelfDto {
    fn from(s: &HomeShelf) -> Self {
        Self {
            title: s.title.clone(),
            cards: s.cards.iter().map(UiHomeCardDto::from).collect(),
            tracks: s.tracks.clone(),
        }
    }
}

impl From<UiHomeShelfDto> for HomeShelf {
    fn from(s: UiHomeShelfDto) -> Self {
        Self {
            title: s.title,
            cards: s.cards.into_iter().map(HomeCard::from).collect(),
            tracks: s.tracks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMoodChipDto {
    pub title: String,
    pub params: String,
}

impl From<&MoodChip> for UiMoodChipDto {
    fn from(c: &MoodChip) -> Self {
        Self {
            title: c.label.clone(),
            params: c.params.clone(),
        }
    }
}

impl From<UiMoodChipDto> for MoodChip {
    fn from(c: UiMoodChipDto) -> Self {
        Self {
            label: c.title,
            params: c.params,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiMoodCategoryTileDto {
    pub title: String,
    pub browse_id: String,
    pub params: String,
    pub color: Option<String>,
}

impl From<&MoodCategoryTile> for UiMoodCategoryTileDto {
    fn from(t: &MoodCategoryTile) -> Self {
        let (r, g, b) = t.accent_rgb;
        Self {
            title: t.title.clone(),
            browse_id: t.browse_id.clone(),
            params: t.params.clone(),
            color: Some(format!("#{r:02x}{g:02x}{b:02x}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiAlbumDetailDto {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub year: Option<String>,
    pub thumbnail_url: Option<String>,
    pub tracks: Vec<Track>,
    pub description: Option<String>,
    pub audio_playlist_id: Option<String>,
    pub is_saved: bool,
}

impl From<&AlbumDetail> for UiAlbumDetailDto {
    fn from(a: &AlbumDetail) -> Self {
        let (artist, artist_id) = a
            .artists
            .first()
            .cloned()
            .unwrap_or_else(|| (a.subtitle.clone(), None));
        Self {
            id: a.id.clone(),
            title: a.title.clone(),
            artist,
            artist_id,
            year: a.year.clone(),
            thumbnail_url: a.cover_url.clone(),
            tracks: a.tracks.clone(),
            description: a.duration_text.clone(),
            audio_playlist_id: a.playlist_id.clone(),
            is_saved: a.saved,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiArtistPageDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub banner_url: Option<String>,
    pub subscribers: Option<String>,
    pub top_tracks: Vec<Track>,
    pub albums: Vec<UiHomeCardDto>,
    pub singles: Vec<UiHomeCardDto>,
    pub playlists: Vec<UiHomeCardDto>,
    pub related_artists: Vec<UiHomeCardDto>,
}

impl From<&ArtistPage> for UiArtistPageDto {
    fn from(a: &ArtistPage) -> Self {
        let mut albums = Vec::new();
        let mut singles = Vec::new();
        let mut playlists = Vec::new();
        for card in &a.releases {
            let dto = UiHomeCardDto::from(card);
            let sub_low = card.subtitle.to_ascii_lowercase();
            if card.kind == HomeCardKind::Playlist {
                playlists.push(dto);
            } else if sub_low.contains("single") || sub_low.contains("ep") {
                singles.push(dto);
            } else {
                albums.push(dto);
            }
        }
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            description: a.description.clone(),
            thumbnail_url: a.thumbnail_url.clone(),
            banner_url: a.thumbnail_url.clone(),
            subscribers: a
                .subscribers
                .clone()
                .or_else(|| a.monthly_listeners.clone()),
            top_tracks: a.popular_tracks.clone(),
            albums,
            singles,
            playlists,
            related_artists: a.related_artists.iter().map(UiHomeCardDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiRichSearchResultDto {
    pub top_result: Option<UiHomeCardDto>,
    pub top_track: Option<Track>,
    pub songs: Vec<Track>,
    pub albums: Vec<UiHomeCardDto>,
    pub artists: Vec<UiHomeCardDto>,
    pub playlists: Vec<UiHomeCardDto>,
}

impl From<&RichSearchResult> for UiRichSearchResultDto {
    fn from(r: &RichSearchResult) -> Self {
        Self {
            top_result: r
                .artists
                .first()
                .or_else(|| r.albums.first())
                .map(UiHomeCardDto::from),
            top_track: r.songs.first().cloned(),
            songs: r.songs.clone(),
            albums: r.albums.iter().map(UiHomeCardDto::from).collect(),
            artists: r.artists.iter().map(UiHomeCardDto::from).collect(),
            playlists: r.playlists.iter().map(UiHomeCardDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiTimedWordDto {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiTimedLineDto {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<UiTimedWordDto>,
    pub speaker: Option<String>,
    pub background: bool,
}

fn is_parenthesized_bg(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2
}

fn timed_line_from(line: &LyricLine, next_line_start_ms: Option<u64>) -> UiTimedLineDto {
    let start_ms = (line.time_secs.max(0.0) * 1000.0).round() as u64;
    let is_bg = is_parenthesized_bg(&line.text);
    let mut words = Vec::with_capacity(line.words.len());
    for (idx, w) in line.words.iter().enumerate() {
        let ws = (w.start_secs.max(0.0) * 1000.0).round() as u64;
        let next_word_start = line
            .words
            .get(idx + 1)
            .map(|nw| (nw.start_secs.max(0.0) * 1000.0).round() as u64);
        let explicit_end = w
            .end_secs
            .map(|e| (e.max(0.0) * 1000.0).round() as u64)
            .filter(|&e| e > ws);
        let we = explicit_end.unwrap_or_else(|| {
            if let Some(nw_start) = next_word_start {
                if nw_start > ws {
                    nw_start.clamp(ws + 100, ws + 2400)
                } else {
                    ws + 260
                }
            } else if let Some(nl_start) = next_line_start_ms {
                if nl_start > ws {
                    nl_start.clamp(ws + 180, ws + 2600)
                } else {
                    ws + 450
                }
            } else {
                ws + 450
            }
        });
        words.push(UiTimedWordDto {
            text: w.text.clone(),
            start_ms: ws,
            end_ms: we,
        });
    }

    let end_ms = if let Some(last) = words.last() {
        last.end_ms.max(start_ms + 200)
    } else if is_bg {
        let natural_end =
            start_ms + ((line.text.trim().chars().count() as u64) * 85).clamp(1400, 3600);
        if let Some(nl_start) = next_line_start_ms {
            if nl_start > start_ms + 250 {
                natural_end.min(nl_start)
            } else {
                natural_end
            }
        } else {
            natural_end
        }
    } else if let Some(nl_start) = next_line_start_ms {
        if nl_start > start_ms {
            nl_start.clamp(start_ms + 300, start_ms + 8000)
        } else {
            start_ms + 3200
        }
    } else {
        start_ms + 3200
    };

    UiTimedLineDto {
        text: line.text.clone(),
        start_ms,
        end_ms,
        words,
        speaker: match line.voice {
            LyricVoice::Lead => Some("Lead".to_string()),
            LyricVoice::Counter => Some("Counter".to_string()),
        },
        background: is_bg,
    }
}

impl From<&LyricLine> for UiTimedLineDto {
    fn from(line: &LyricLine) -> Self {
        timed_line_from(line, None)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum UiLyricsDto {
    Empty,
    Loading,
    Synced { lines: Vec<UiTimedLineDto> },
    Plain { text: String },
    Unavailable,
}

impl From<&LyricsState> for UiLyricsDto {
    fn from(l: &LyricsState) -> Self {
        match l {
            LyricsState::Idle => Self::Empty,
            LyricsState::Loading => Self::Loading,
            LyricsState::Synced(lines) => {
                let converted = lines
                    .iter()
                    .enumerate()
                    .map(|(idx, line)| {
                        let is_bg = is_parenthesized_bg(&line.text);
                        let next_start_ms = if is_bg {
                            lines.get(idx + 1)
                        } else {
                            lines[idx + 1..]
                                .iter()
                                .find(|nl| !is_parenthesized_bg(&nl.text))
                        }
                        .map(|nl| (nl.time_secs.max(0.0) * 1000.0).round() as u64);
                        timed_line_from(line, next_start_ms)
                    })
                    .collect();
                Self::Synced { lines: converted }
            }
            LyricsState::Plain(lines) => Self::Plain {
                text: lines.join("\n"),
            },
            LyricsState::NotFound => Self::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiScrobbleStatusDto {
    pub lastfm_enabled: bool,
    pub lastfm_configured: bool,
    pub lastfm_username: String,
    pub lastfm_api_key: String,
    pub listenbrainz_enabled: bool,
    pub listenbrainz_configured: bool,
    pub listenbrainz_username: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiStatsTopTrackDto {
    pub title: String,
    pub artist: String,
    pub play_count: u64,
    pub total_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiStatsTopArtistDto {
    pub artist: String,
    pub play_count: u64,
    pub total_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiStatsSummaryDto {
    pub total_plays: u64,
    pub total_listening_secs: u64,
    pub unique_tracks: u64,
    pub unique_artists: u64,
    pub top_tracks: Vec<UiStatsTopTrackDto>,
    pub top_artists: Vec<UiStatsTopArtistDto>,
}

impl From<&StatsSummary> for UiStatsSummaryDto {
    fn from(s: &StatsSummary) -> Self {
        Self {
            total_plays: s.total_plays,
            total_listening_secs: s.total_minutes.saturating_mul(60),
            unique_tracks: s.unique_tracks,
            unique_artists: s.unique_artists,
            top_tracks: s
                .top_tracks
                .iter()
                .map(|t| UiStatsTopTrackDto {
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    play_count: t.plays,
                    total_secs: t.minutes.saturating_mul(60),
                })
                .collect(),
            top_artists: s
                .top_artists
                .iter()
                .map(|a| UiStatsTopArtistDto {
                    artist: a.artist.clone(),
                    play_count: a.plays,
                    total_secs: a.minutes.saturating_mul(60),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiUpdateCheckDto {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub notes: Option<String>,
}

impl From<&UpdateCheckResult> for UiUpdateCheckDto {
    fn from(u: &UpdateCheckResult) -> Self {
        Self {
            current_version: u.current_version.clone(),
            latest_version: u.latest_version.clone(),
            update_available: u.update_available,
            release_url: u.release_url.clone(),
            notes: (!u.release_notes.is_empty()).then(|| u.release_notes.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiConfigDto {
    pub language: AppLanguage,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
    pub autoplay: bool,
    pub provider_mode: ProviderMode,
    pub hifi_substitution: bool,
    pub jiosaavn_enabled: bool,
    pub cache_limit_gb: u8,
    pub motion_covers: bool,
    pub reduce_motion: bool,
    pub preferred_browser: Option<String>,
    pub desktop_ytm_browser: Option<String>,
    pub downloads_dir: Option<String>,
    pub downloads_limit_gb: u8,
    pub discord_rpc_enabled: bool,
    pub playback_speed: f64,
    pub skip_silence: bool,
    pub lyrics_translate: bool,
    pub lyrics_romanize: bool,
    pub video_quality: String,
    pub scrobble_servers_to_ytm: bool,
    pub personal_only_mode: bool,
    pub local_music_enabled: bool,
    pub local_music_dirs: Vec<String>,
    pub gapless_enabled: bool,
    pub crossfade_secs: u8,
    pub connect_enabled: bool,
    pub connect_device_name: String,
}

impl From<&AppConfig> for UiConfigDto {
    fn from(c: &AppConfig) -> Self {
        Self {
            language: c.language,
            volume: c.volume,
            shuffle: c.shuffle,
            repeat_mode: c.repeat_mode,
            autoplay: c.autoplay,
            provider_mode: c.provider_mode,
            hifi_substitution: c.hifi_substitution,
            jiosaavn_enabled: c.jiosaavn_enabled,
            cache_limit_gb: c.cache_limit_gb,
            motion_covers: c.motion_covers,
            reduce_motion: c.reduce_motion,
            preferred_browser: c.preferred_browser.clone(),
            desktop_ytm_browser: c.desktop_ytm_browser.clone(),
            downloads_dir: c
                .downloads_dir
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            downloads_limit_gb: c.downloads_limit_gb,
            discord_rpc_enabled: c.discord_rpc_enabled,
            playback_speed: c.playback_speed,
            skip_silence: c.skip_silence,
            lyrics_translate: c.lyrics_translate,
            lyrics_romanize: c.lyrics_romanize,
            video_quality: c.video_quality.clone(),
            scrobble_servers_to_ytm: c.scrobble_servers_to_ytm,
            personal_only_mode: c.personal_only_mode,
            local_music_enabled: c.local_music_enabled,
            local_music_dirs: c
                .local_music_dirs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            gapless_enabled: c.gapless_enabled,
            crossfade_secs: c.crossfade_secs,
            connect_enabled: c.connect_enabled,
            connect_device_name: c.connect_device_name.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshotDto {
    pub config: UiConfigDto,
    pub active_nav: NavTargetDto,
    pub status_line: String,
    pub available_browsers: Vec<UiBrowserOptionDto>,
    pub signed_in_browser: Option<String>,
    pub user_profile: Option<UiYtAccountProfileDto>,
    pub user_avatar_path: Option<String>,
    pub signing_in: bool,
    pub sign_in_error: Option<String>,
    pub ytm_playlists: Vec<Playlist>,
    pub library_albums: Vec<UiHomeCardDto>,
    pub library_artists: Vec<UiHomeCardDto>,
    pub library_extras_loading: bool,
    pub home_chips: Vec<UiMoodChipDto>,
    pub active_home_chip: Option<UiMoodChipDto>,
    pub home_shelves: Vec<UiHomeShelfDto>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub explore_shelves: Vec<UiHomeShelfDto>,
    pub mood_tiles: Vec<UiMoodCategoryTileDto>,
    pub explore_loading: bool,
    pub mood_page_title: String,
    pub mood_page_shelves: Vec<UiHomeShelfDto>,
    pub mood_page_loading: bool,
    pub shelf_detail: Option<UiHomeShelfDto>,
    pub radio_shelves: Vec<UiHomeShelfDto>,
    pub radio_loading: bool,
    pub history_tracks: Vec<Track>,
    pub history_loading: bool,
    pub last_played_tracks: Vec<Track>,
    pub recent_searches: Vec<String>,
    pub search_suggestions: Vec<String>,
    pub search_query: String,
    pub search_results: Vec<Track>,
    pub search_rich: UiRichSearchResultDto,
    pub search_loading: bool,
    pub playlist_id: Option<String>,
    pub playlist_heading: String,
    pub playlist_subtitle: String,
    pub playlist_cover_url: Option<String>,
    pub playlist_editable: bool,
    pub playlist_tracks: Vec<Track>,
    pub playlist_loading: bool,
    pub album_detail: Option<UiAlbumDetailDto>,
    pub album_loading: bool,
    pub album_error: Option<String>,
    pub artist_page: Option<UiArtistPageDto>,
    pub artist_banner_url: Option<String>,
    pub artist_loading: bool,
    pub artist_error: Option<String>,
    pub liked_track_ids: Vec<String>,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
    pub paused: bool,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub autoplay: bool,
    pub stream_badge: String,
    pub upgrading_quality: bool,
    pub loading_track: bool,
    pub playback_speed: f64,
    pub skip_silence: bool,
    pub sleep_timer_remaining_secs: Option<u64>,
    pub video_mode: bool,
    pub video_loading: bool,
    pub has_music_video_candidate: bool,
    pub available_video_qualities: Vec<String>,
    pub thumb_rev: u64,
    pub lyrics: UiLyricsDto,
    pub translated_lyrics: Option<Vec<String>>,
    pub romanized_lyrics: Option<Vec<String>>,
    pub art_path: Option<String>,
    pub ambient_rgb: (u8, u8, u8),
    pub has_motion_cover: bool,
    pub motion_cover_path: Option<String>,
    pub spotify_notice: Option<String>,
    pub update_info: Option<UiUpdateCheckDto>,
    pub checking_updates: bool,
    pub downloaded_tracks: Vec<DownloadedTrack>,
    pub downloaded_collections: Vec<DownloadedCollection>,
    pub downloaded_track_ids: Vec<String>,
    pub downloading_track_ids: Vec<String>,
    pub downloads_total_bytes: u64,
    pub discord_status: String,
    pub server_profiles: Vec<ServerProfile>,
    pub server_statuses: HashMap<String, String>,
    pub server_health: HashMap<String, ServerHealthReport>,
    pub server_speed_tests: HashMap<String, ServerSpeedTestReport>,
    pub server_speed_testing: Vec<String>,
    pub active_server_id: Option<String>,
    pub server_libraries: Vec<ServerLibrary>,
    pub server_artists: Vec<UiHomeCardDto>,
    pub server_albums: Vec<UiHomeCardDto>,
    pub server_playlists: Vec<Playlist>,
    pub server_tracks: Vec<Track>,
    pub server_current_folder: Option<String>,
    pub server_loading: bool,
    pub server_test_result: Option<Result<String, String>>,
    pub local_tracks: Vec<Track>,
    pub local_scanning: bool,
    pub personal_playlists: Vec<PersonalPlaylist>,
    pub scrobble_status: UiScrobbleStatusDto,
    pub stats_window: StatsWindow,
    pub stats_summary: UiStatsSummaryDto,
    pub stats_loading: bool,
    pub connect_running: bool,
    pub connect_http_port: Option<u16>,
    pub connect_pin: Option<String>,
    pub connect_paired_devices: Vec<PairedDeviceInfo>,
    pub connect_pending_pair: Option<PendingPairRequest>,
    pub connect_error: Option<String>,
}

fn sanitize_server_profile(profile: &ServerProfile) -> ServerProfile {
    let mut clean = profile.clone();
    clean.password = None;
    clean.api_key = None;
    clean.access_token = None;
    clean.bearer_token = None;
    clean
}

fn format_discord_status(status: &DiscordRpcStatus, lang: AppLanguage) -> String {
    let s = sonata_runtime::i18n::strings(lang);
    match status {
        DiscordRpcStatus::Disabled => s.disabled.to_string(),
        DiscordRpcStatus::Disconnected => s.discord_waiting.to_string(),
        DiscordRpcStatus::Connecting => s.discord_connecting.to_string(),
        DiscordRpcStatus::Connected { .. } => s.discord_connected.to_string(),
        DiscordRpcStatus::Error(err) => err.clone(),
    }
}

impl From<&DesktopSnapshot> for UiSnapshotDto {
    fn from(s: &DesktopSnapshot) -> Self {
        Self {
            config: UiConfigDto::from(&s.config),
            active_nav: NavTargetDto::from(&s.active_nav),
            status_line: s.status_line.clone(),
            available_browsers: s
                .available_browsers
                .iter()
                .map(UiBrowserOptionDto::from)
                .collect(),
            signed_in_browser: s.signed_in_browser.clone(),
            user_profile: s.user_profile.as_ref().map(UiYtAccountProfileDto::from),
            user_avatar_path: s
                .user_avatar_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            signing_in: s.signing_in,
            sign_in_error: s.sign_in_error.clone(),
            ytm_playlists: s.ytm_playlists.clone(),
            library_albums: s.library_albums.iter().map(UiHomeCardDto::from).collect(),
            library_artists: s.library_artists.iter().map(UiHomeCardDto::from).collect(),
            library_extras_loading: s.library_extras_loading,
            home_chips: s.home_chips.iter().map(UiMoodChipDto::from).collect(),
            active_home_chip: s.active_home_chip.as_ref().map(UiMoodChipDto::from),
            home_shelves: s.home_shelves.iter().map(UiHomeShelfDto::from).collect(),
            home_loading: s.home_loading,
            home_error: s.home_error.clone(),
            explore_shelves: s.explore_shelves.iter().map(UiHomeShelfDto::from).collect(),
            mood_tiles: s
                .mood_tiles
                .iter()
                .map(UiMoodCategoryTileDto::from)
                .collect(),
            explore_loading: s.explore_loading,
            mood_page_title: s.mood_page_title.clone(),
            mood_page_shelves: s
                .mood_page_shelves
                .iter()
                .map(UiHomeShelfDto::from)
                .collect(),
            mood_page_loading: s.mood_page_loading,
            shelf_detail: s.shelf_detail.as_ref().map(UiHomeShelfDto::from),
            radio_shelves: s.radio_shelves.iter().map(UiHomeShelfDto::from).collect(),
            radio_loading: s.radio_loading,
            history_tracks: s.history_tracks.clone(),
            history_loading: s.history_loading,
            last_played_tracks: s.last_played_tracks.clone(),
            recent_searches: s.recent_searches.clone(),
            search_suggestions: s.search_suggestions.clone(),
            search_query: s.search_query.clone(),
            search_results: s.search_results.clone(),
            search_rich: UiRichSearchResultDto::from(&s.search_rich),
            search_loading: s.search_loading,
            playlist_id: s.playlist_id.clone(),
            playlist_heading: s.playlist_heading.clone(),
            playlist_subtitle: s.playlist_subtitle.clone(),
            playlist_cover_url: s.playlist_cover_url.clone(),
            playlist_editable: s.playlist_editable,
            playlist_tracks: s.playlist_tracks.clone(),
            playlist_loading: s.playlist_loading,
            album_detail: s.album_detail.as_ref().map(UiAlbumDetailDto::from),
            album_loading: s.album_loading,
            album_error: s.album_error.clone(),
            artist_page: s.artist_page.as_ref().map(UiArtistPageDto::from),
            artist_banner_url: s.artist_banner_url.clone(),
            artist_loading: s.artist_loading,
            artist_error: s.artist_error.clone(),
            liked_track_ids: s.liked_track_ids.iter().cloned().collect(),
            queue: s.queue.clone(),
            current_index: s.current_index,
            paused: s.paused,
            muted: s.muted,
            position_secs: s.position_secs,
            duration_secs: s.duration_secs,
            volume: s.volume,
            shuffle: s.shuffle,
            repeat: s.repeat,
            autoplay: s.autoplay,
            stream_badge: s.stream_badge.clone(),
            upgrading_quality: s.upgrading_quality,
            loading_track: s.loading_track,
            playback_speed: s.playback_speed,
            skip_silence: s.skip_silence,
            sleep_timer_remaining_secs: s.sleep_timer_remaining_secs,
            video_mode: s.video_mode,
            video_loading: s.video_loading,
            has_music_video_candidate: s.music_video_candidate.is_some(),
            available_video_qualities: s
                .available_video_qualities
                .iter()
                .map(|q: &VideoQualityStream| q.quality_label.clone())
                .collect(),
            thumb_rev: s.thumb_rev,
            lyrics: UiLyricsDto::from(&s.lyrics),
            translated_lyrics: s.translated_lyrics.clone(),
            romanized_lyrics: s.romanized_lyrics.clone(),
            art_path: s
                .art_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            ambient_rgb: s.ambient_rgb,
            has_motion_cover: s.motion_cover_path.is_some(),
            motion_cover_path: s
                .motion_cover_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            spotify_notice: s.spotify_notice.clone(),
            update_info: s.update_info.as_ref().map(UiUpdateCheckDto::from),
            checking_updates: s.checking_updates,
            downloaded_tracks: s.downloaded_tracks.clone(),
            downloaded_collections: s.downloaded_collections.clone(),
            downloaded_track_ids: s.downloaded_track_ids.iter().cloned().collect(),
            downloading_track_ids: s.downloading_track_ids.iter().cloned().collect(),
            downloads_total_bytes: s.downloads_total_bytes,
            discord_status: format_discord_status(&s.discord_status, s.config.language),
            server_profiles: s
                .server_profiles
                .iter()
                .map(sanitize_server_profile)
                .collect(),
            server_statuses: s.server_statuses.clone(),
            server_health: s.server_health.clone(),
            server_speed_tests: s.server_speed_tests.clone(),
            server_speed_testing: s.server_speed_testing.iter().cloned().collect(),
            active_server_id: s.active_server_id.clone(),
            server_libraries: s.server_libraries.clone(),
            server_artists: s.server_artists.iter().map(UiHomeCardDto::from).collect(),
            server_albums: s.server_albums.iter().map(UiHomeCardDto::from).collect(),
            server_playlists: s.server_playlists.clone(),
            server_tracks: s.server_tracks.clone(),
            server_current_folder: s.server_current_folder.clone(),
            server_loading: s.server_loading,
            server_test_result: s.server_test_result.clone(),
            local_tracks: s.local_scan.tracks.clone(),
            local_scanning: s.local_scanning,
            personal_playlists: s.personal_playlists.clone(),
            scrobble_status: UiScrobbleStatusDto {
                lastfm_enabled: s.scrobble_status.lastfm_enabled,
                lastfm_configured: s.scrobble_status.lastfm_configured,
                lastfm_username: s.scrobble_status.lastfm_username.clone(),
                lastfm_api_key: s.scrobble_status.lastfm_api_key.clone(),
                listenbrainz_enabled: s.scrobble_status.listenbrainz_enabled,
                listenbrainz_configured: s.scrobble_status.listenbrainz_configured,
                listenbrainz_username: s.scrobble_status.listenbrainz_username.clone(),
                last_error: s.scrobble_status.last_error.clone(),
            },
            stats_window: s.stats_window,
            stats_summary: UiStatsSummaryDto::from(&s.stats_summary),
            stats_loading: s.stats_loading,
            connect_running: s.connect_running,
            connect_http_port: s.connect_http_port,
            connect_pin: s.connect_pin.clone(),
            connect_paired_devices: s.connect_paired_devices.clone(),
            connect_pending_pair: s.connect_pending_pair.clone(),
            connect_error: s.connect_error.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressTickDto {
    pub position_secs: f64,
    pub duration_secs: f64,
    pub paused: bool,
    pub loading_track: bool,
    pub playback_speed: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum BackendStreamEvent {
    Snapshot(Box<UiSnapshotDto>),
    Progress(ProgressTickDto),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BackendCmdDto {
    Navigate {
        target: NavTargetDto,
    },
    OpenCollection {
        id: String,
        title: String,
        subtitle: String,
        thumbnail_url: Option<String>,
        editable: bool,
    },
    OpenArtist {
        id: String,
    },
    OpenArtistByName {
        name: String,
    },
    OpenAlbum {
        album_id: Option<String>,
        title: String,
        artist: String,
        thumbnail_url: Option<String>,
    },
    OpenMoodCategory {
        title: String,
        browse_id: String,
        params: String,
    },
    OpenMoodByTitle {
        title: String,
    },
    OpenShelfDetail {
        shelf: UiHomeShelfDto,
    },
    SelectHomeChip {
        chip: Option<UiMoodChipDto>,
    },
    RefreshActivePlaylist,
    RefreshHome,
    RefreshExplore,
    RefreshRadio,
    RefreshLibrary,
    RefreshHistory,
    RefreshAccount,
    StartTrackRadio {
        track: Track,
    },
    StartQuickRadio,
    SignInWithBrowser {
        browser_id: String,
    },
    ClearSignInError,
    SignOut,
    FetchSearchSuggestions {
        query: String,
    },
    Search {
        query: String,
    },
    PlayTracks {
        tracks: Vec<Track>,
        start_index: usize,
        shuffle: bool,
    },
    PlayQueueIndex {
        index: usize,
    },
    PlayTrackNext {
        track: Track,
    },
    AddTrackToQueue {
        track: Track,
    },
    RemoveFromQueue {
        index: usize,
    },
    MoveQueueItem {
        from: usize,
        to: usize,
    },
    ClearQueue,
    ShuffleQueue,
    TogglePause,
    SetPaused {
        paused: bool,
    },
    Stop,
    NextTrack,
    PrevTrack,
    SeekAbsolute {
        secs: f64,
    },
    SeekRelative {
        secs: f64,
    },
    SetVolume {
        volume: u8,
    },
    ToggleMute,
    ToggleShuffle,
    SetShuffle {
        shuffle: bool,
    },
    CycleRepeat,
    SetRepeat {
        mode: RepeatMode,
    },
    ToggleAutoplay,
    ToggleHiFi,
    ToggleJioSaavn,
    TogglePersonalOnlyMode,
    ToggleLocalMusicEnabled,
    AddLocalMusicDir {
        path: String,
    },
    RemoveLocalMusicDir {
        path: String,
    },
    ScanLocalMusic,
    ToggleAttributePersonalToYtm,
    StepCacheLimit {
        up: bool,
    },
    ToggleMotionCovers,
    ToggleReduceMotion,
    SetLanguage {
        language: AppLanguage,
    },
    SetPlaybackSpeed {
        speed: f64,
    },
    ToggleSkipSilence,
    SetSleepTimer {
        minutes: Option<u64>,
    },
    ToggleLyricsTranslate,
    ToggleLyricsRomanize,
    SetVideoMode {
        enabled: bool,
    },
    SetVideoQuality {
        quality: String,
    },
    ToggleLike {
        track: Track,
    },
    DislikeTrack {
        track: Track,
    },
    ToggleSaveAlbum,
    ToggleSavePlaylist {
        id: String,
        title: String,
        thumbnail_url: Option<String>,
        save: bool,
    },
    CreateYtmPlaylist {
        name: String,
    },
    CreatePersonalPlaylist {
        name: String,
    },
    DeletePersonalPlaylist {
        id: String,
    },
    EditPlaylist {
        playlist_id: String,
        title: Option<String>,
    },
    AddTrackToPlaylist {
        track: Track,
        playlist_id: String,
        playlist_name: String,
    },
    AddTrackToPersonalPlaylist {
        track: Track,
        playlist_id: String,
    },
    RemoveTrackFromPlaylist {
        index: usize,
    },
    ClickLinkSpotify,
    CheckForUpdates,
    RefreshDownloads,
    DownloadTrack {
        track: Track,
        collection_id: Option<String>,
        collection_title: Option<String>,
    },
    RemoveDownloadedTrack {
        track_id: String,
    },
    DownloadCollection {
        collection_id: String,
        title: String,
        source: ProviderSource,
        artwork_url: Option<String>,
        tracks: Vec<Track>,
    },
    RemoveDownloadedCollection {
        collection_id: String,
    },
    SetDownloadsDir {
        path: Option<String>,
    },
    StepDownloadsLimit {
        up: bool,
    },
    ClearAllDownloads,
    ToggleDiscordRpc,
    RefreshServers,
    SaveServerProfile {
        id: Option<String>,
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
    },
    TestServerDraft {
        name: String,
        kind: ServerKind,
        base_url: String,
        username: Option<String>,
        password: Option<String>,
        api_key: Option<String>,
        bearer_token: Option<String>,
    },
    ProbeServerHealth {
        id: String,
    },
    RunServerSpeedTest {
        id: String,
    },
    ToggleServerEnabled {
        id: String,
    },
    RemoveServerProfile {
        id: String,
    },
    SelectServer {
        server_id: String,
    },
    SetWindowAddress {
        addr: Option<String>,
    },
    SetUiCompact {
        compact: bool,
    },
    ToggleGapless,
    SetCrossfade {
        secs: u8,
    },
    SaveScrobbleConfig {
        lastfm_enabled: bool,
        lastfm_api_key: String,
        lastfm_api_secret: Option<String>,
        lastfm_session_key: Option<String>,
        lastfm_username: String,
        listenbrainz_enabled: bool,
        listenbrainz_token: Option<String>,
        listenbrainz_username: String,
    },
    ClearScrobbleBackend {
        lastfm: bool,
        listenbrainz: bool,
    },
    LoadListeningStats {
        window: StatsWindow,
    },
    ToggleConnect,
    SetConnectDeviceName {
        name: String,
    },
    RegenerateConnectPin,
    UnpairConnectDevice {
        device_id: String,
    },
    ApproveConnectPair {
        request_id: String,
    },
    RejectConnectPair {
        request_id: String,
    },
}

impl From<BackendCmdDto> for BackendCmd {
    fn from(cmd: BackendCmdDto) -> Self {
        match cmd {
            BackendCmdDto::Navigate { target } => Self::Navigate(target.into()),
            BackendCmdDto::OpenCollection {
                id,
                title,
                subtitle,
                thumbnail_url,
                editable,
            } => Self::OpenCollection {
                id,
                title,
                subtitle,
                thumbnail_url,
                editable,
            },
            BackendCmdDto::OpenArtist { id } => Self::OpenArtist(id),
            BackendCmdDto::OpenArtistByName { name } => Self::OpenArtistByName(name),
            BackendCmdDto::OpenAlbum {
                album_id,
                title,
                artist,
                thumbnail_url,
            } => Self::OpenAlbum {
                album_id,
                title,
                artist,
                thumbnail_url,
            },
            BackendCmdDto::OpenMoodCategory {
                title,
                browse_id,
                params,
            } => Self::OpenMoodCategory {
                title,
                browse_id,
                params,
            },
            BackendCmdDto::OpenMoodByTitle { title } => Self::OpenMoodByTitle(title),
            BackendCmdDto::OpenShelfDetail { shelf } => Self::OpenShelfDetail(shelf.into()),
            BackendCmdDto::SelectHomeChip { chip } => Self::SelectHomeChip(chip.map(Into::into)),
            BackendCmdDto::RefreshActivePlaylist => Self::RefreshActivePlaylist,
            BackendCmdDto::RefreshHome => Self::RefreshHome,
            BackendCmdDto::RefreshExplore => Self::RefreshExplore,
            BackendCmdDto::RefreshRadio => Self::RefreshRadio,
            BackendCmdDto::RefreshLibrary => Self::RefreshLibrary,
            BackendCmdDto::RefreshHistory => Self::RefreshHistory,
            BackendCmdDto::RefreshAccount => Self::RefreshAccount,
            BackendCmdDto::StartTrackRadio { track } => Self::StartTrackRadio(track),
            BackendCmdDto::StartQuickRadio => Self::StartQuickRadio,
            BackendCmdDto::SignInWithBrowser { browser_id } => Self::SignInWithBrowser(browser_id),
            BackendCmdDto::ClearSignInError => Self::ClearSignInError,
            BackendCmdDto::SignOut => Self::SignOut,
            BackendCmdDto::FetchSearchSuggestions { query } => Self::FetchSearchSuggestions(query),
            BackendCmdDto::Search { query } => Self::Search(query),
            BackendCmdDto::PlayTracks {
                tracks,
                start_index,
                shuffle,
            } => Self::PlayTracks {
                tracks,
                start_index,
                shuffle,
            },
            BackendCmdDto::PlayQueueIndex { index } => Self::PlayQueueIndex(index),
            BackendCmdDto::PlayTrackNext { track } => Self::PlayTrackNext(track),
            BackendCmdDto::AddTrackToQueue { track } => Self::AddTrackToQueue(track),
            BackendCmdDto::RemoveFromQueue { index } => Self::RemoveFromQueue(index),
            BackendCmdDto::MoveQueueItem { from, to } => Self::MoveQueueItem { from, to },
            BackendCmdDto::ClearQueue => Self::ClearQueue,
            BackendCmdDto::ShuffleQueue => Self::ShuffleQueue,
            BackendCmdDto::TogglePause => Self::TogglePause,
            BackendCmdDto::SetPaused { paused } => Self::SetPaused(paused),
            BackendCmdDto::Stop => Self::Stop,
            BackendCmdDto::NextTrack => Self::NextTrack,
            BackendCmdDto::PrevTrack => Self::PrevTrack,
            BackendCmdDto::SeekAbsolute { secs } => Self::SeekAbsolute(secs),
            BackendCmdDto::SeekRelative { secs } => Self::SeekRelative(secs),
            BackendCmdDto::SetVolume { volume } => Self::SetVolume(volume),
            BackendCmdDto::ToggleMute => Self::ToggleMute,
            BackendCmdDto::ToggleShuffle => Self::ToggleShuffle,
            BackendCmdDto::SetShuffle { shuffle } => Self::SetShuffle(shuffle),
            BackendCmdDto::CycleRepeat => Self::CycleRepeat,
            BackendCmdDto::SetRepeat { mode } => Self::SetRepeat(mode),
            BackendCmdDto::ToggleAutoplay => Self::ToggleAutoplay,
            BackendCmdDto::ToggleHiFi => Self::ToggleHiFi,
            BackendCmdDto::ToggleJioSaavn => Self::ToggleJioSaavn,
            BackendCmdDto::TogglePersonalOnlyMode => Self::TogglePersonalOnlyMode,
            BackendCmdDto::ToggleLocalMusicEnabled => Self::ToggleLocalMusicEnabled,
            BackendCmdDto::AddLocalMusicDir { path } => Self::AddLocalMusicDir(PathBuf::from(path)),
            BackendCmdDto::RemoveLocalMusicDir { path } => {
                Self::RemoveLocalMusicDir(PathBuf::from(path))
            }
            BackendCmdDto::ScanLocalMusic => Self::ScanLocalMusic,
            BackendCmdDto::ToggleAttributePersonalToYtm => Self::ToggleAttributePersonalToYtm,
            BackendCmdDto::StepCacheLimit { up } => Self::StepCacheLimit(up),
            BackendCmdDto::ToggleMotionCovers => Self::ToggleMotionCovers,
            BackendCmdDto::ToggleReduceMotion => Self::ToggleReduceMotion,
            BackendCmdDto::SetLanguage { language } => Self::SetLanguage(language),
            BackendCmdDto::SetPlaybackSpeed { speed } => Self::SetPlaybackSpeed(speed),
            BackendCmdDto::ToggleSkipSilence => Self::ToggleSkipSilence,
            BackendCmdDto::SetSleepTimer { minutes } => Self::SetSleepTimer(minutes),
            BackendCmdDto::ToggleLyricsTranslate => Self::ToggleLyricsTranslate,
            BackendCmdDto::ToggleLyricsRomanize => Self::ToggleLyricsRomanize,
            BackendCmdDto::SetVideoMode { enabled } => Self::SetVideoMode(enabled),
            BackendCmdDto::SetVideoQuality { quality } => Self::SetVideoQuality(quality),
            BackendCmdDto::ToggleLike { track } => Self::ToggleLike(track),
            BackendCmdDto::DislikeTrack { track } => Self::DislikeTrack(track),
            BackendCmdDto::ToggleSaveAlbum => Self::ToggleSaveAlbum,
            BackendCmdDto::ToggleSavePlaylist {
                id,
                title,
                thumbnail_url,
                save,
            } => Self::ToggleSavePlaylist {
                id,
                title,
                thumbnail_url,
                save,
            },
            BackendCmdDto::CreateYtmPlaylist { name } => Self::CreateYtmPlaylist(name),
            BackendCmdDto::CreatePersonalPlaylist { name } => Self::CreatePersonalPlaylist(name),
            BackendCmdDto::DeletePersonalPlaylist { id } => Self::DeletePersonalPlaylist(id),
            BackendCmdDto::EditPlaylist { playlist_id, title } => Self::EditPlaylist {
                playlist_id,
                title,
                cover_path: None,
            },
            BackendCmdDto::AddTrackToPlaylist {
                track,
                playlist_id,
                playlist_name,
            } => Self::AddTrackToPlaylist {
                track,
                playlist_id,
                playlist_name,
            },
            BackendCmdDto::AddTrackToPersonalPlaylist { track, playlist_id } => {
                Self::AddTrackToPersonalPlaylist { track, playlist_id }
            }
            BackendCmdDto::RemoveTrackFromPlaylist { index } => {
                Self::RemoveTrackFromPlaylist(index)
            }
            BackendCmdDto::ClickLinkSpotify => Self::ClickLinkSpotify,
            BackendCmdDto::CheckForUpdates => Self::CheckForUpdates,
            BackendCmdDto::RefreshDownloads => Self::RefreshDownloads,
            BackendCmdDto::DownloadTrack {
                track,
                collection_id,
                collection_title,
            } => {
                let collection =
                    collection_id.map(|id| sonata_runtime::downloads::DownloadCollectionRef {
                        collection_id: id,
                        title: collection_title.unwrap_or_default(),
                        source: track.source,
                        artwork_url: track.thumbnail_url.clone(),
                        position: 0,
                    });
                Self::DownloadTrack { track, collection }
            }
            BackendCmdDto::RemoveDownloadedTrack { track_id } => {
                Self::RemoveDownloadedTrack(track_id)
            }
            BackendCmdDto::DownloadCollection {
                collection_id,
                title,
                source,
                artwork_url,
                tracks,
            } => Self::DownloadCollection {
                collection_id,
                title,
                source,
                artwork_url,
                tracks,
            },
            BackendCmdDto::RemoveDownloadedCollection { collection_id } => {
                Self::RemoveDownloadedCollection(collection_id)
            }
            BackendCmdDto::SetDownloadsDir { path } => {
                Self::SetDownloadsDir(path.map(PathBuf::from))
            }
            BackendCmdDto::StepDownloadsLimit { up } => Self::StepDownloadsLimit(up),
            BackendCmdDto::ClearAllDownloads => Self::ClearAllDownloads,
            BackendCmdDto::ToggleDiscordRpc => Self::ToggleDiscordRpc,
            BackendCmdDto::RefreshServers => Self::RefreshServers,
            BackendCmdDto::SaveServerProfile {
                id,
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
            } => Self::SaveServerProfile {
                id,
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
                share: None,
                domain: None,
                local_mount_path: None,
            },
            BackendCmdDto::TestServerDraft {
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
            } => Self::TestServerDraft {
                name,
                kind,
                base_url,
                username,
                password,
                api_key,
                bearer_token,
                share: None,
                domain: None,
                local_mount_path: None,
            },
            BackendCmdDto::ProbeServerHealth { id } => Self::ProbeServerHealth(id),
            BackendCmdDto::RunServerSpeedTest { id } => Self::RunServerSpeedTest(id),
            BackendCmdDto::ToggleServerEnabled { id } => Self::ToggleServerEnabled(id),
            BackendCmdDto::RemoveServerProfile { id } => Self::RemoveServerProfile(id),
            BackendCmdDto::SelectServer { server_id } => Self::SelectServer {
                server_id,
                library_id: None,
                folder_path: None,
            },
            BackendCmdDto::SetWindowAddress { addr } => Self::SetWindowAddress(addr),
            BackendCmdDto::SetUiCompact { compact } => Self::SetUiCompact(compact),
            BackendCmdDto::ToggleGapless => Self::ToggleGapless,
            BackendCmdDto::SetCrossfade { secs } => Self::SetCrossfade(secs),
            BackendCmdDto::SaveScrobbleConfig {
                lastfm_enabled,
                lastfm_api_key,
                lastfm_api_secret,
                lastfm_session_key,
                lastfm_username,
                listenbrainz_enabled,
                listenbrainz_token,
                listenbrainz_username,
            } => Self::SaveScrobbleConfig {
                lastfm_enabled,
                lastfm_api_key,
                lastfm_api_secret,
                lastfm_session_key,
                lastfm_username,
                listenbrainz_enabled,
                listenbrainz_token,
                listenbrainz_username,
            },
            BackendCmdDto::ClearScrobbleBackend {
                lastfm,
                listenbrainz,
            } => Self::ClearScrobbleBackend {
                lastfm,
                listenbrainz,
            },
            BackendCmdDto::LoadListeningStats { window } => Self::LoadListeningStats(window),
            BackendCmdDto::ToggleConnect => Self::ToggleConnect,
            BackendCmdDto::SetConnectDeviceName { name } => Self::SetConnectDeviceName(name),
            BackendCmdDto::RegenerateConnectPin => Self::RegenerateConnectPin,
            BackendCmdDto::UnpairConnectDevice { device_id } => {
                Self::UnpairConnectDevice(device_id)
            }
            BackendCmdDto::ApproveConnectPair { request_id } => {
                Self::ApproveConnectPair(request_id)
            }
            BackendCmdDto::RejectConnectPair { request_id } => Self::RejectConnectPair(request_id),
        }
    }
}

fn snapshot_structure_fingerprint(snap: &DesktopSnapshot) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", snap.active_nav).hash(&mut h);
    snap.status_line.hash(&mut h);
    snap.signed_in_browser.hash(&mut h);
    snap.signing_in.hash(&mut h);
    snap.sign_in_error.hash(&mut h);
    snap.ytm_playlists.len().hash(&mut h);
    snap.home_loading.hash(&mut h);
    snap.home_shelves.len().hash(&mut h);
    snap.explore_loading.hash(&mut h);
    snap.explore_shelves.len().hash(&mut h);
    snap.radio_loading.hash(&mut h);
    snap.radio_shelves.len().hash(&mut h);
    snap.history_loading.hash(&mut h);
    snap.history_tracks.len().hash(&mut h);
    snap.search_loading.hash(&mut h);
    snap.search_query.hash(&mut h);
    snap.search_results.len().hash(&mut h);
    snap.search_suggestions.hash(&mut h);
    snap.playlist_id.hash(&mut h);
    snap.playlist_loading.hash(&mut h);
    snap.playlist_tracks.len().hash(&mut h);
    snap.album_loading.hash(&mut h);
    snap.album_detail
        .as_ref()
        .map(|a| (&a.id, a.saved, a.tracks.len()))
        .hash(&mut h);
    snap.artist_loading.hash(&mut h);
    snap.artist_page
        .as_ref()
        .map(|a| (&a.id, a.popular_tracks.len()))
        .hash(&mut h);
    snap.queue.len().hash(&mut h);
    for t in &snap.queue {
        t.id.hash(&mut h);
    }
    snap.current_index.hash(&mut h);
    snap.paused.hash(&mut h);
    snap.muted.hash(&mut h);
    snap.volume.hash(&mut h);
    snap.shuffle.hash(&mut h);
    format!("{:?}", snap.repeat).hash(&mut h);
    snap.autoplay.hash(&mut h);
    snap.stream_badge.hash(&mut h);
    snap.loading_track.hash(&mut h);
    snap.upgrading_quality.hash(&mut h);
    snap.playback_speed.to_bits().hash(&mut h);
    snap.skip_silence.hash(&mut h);
    snap.sleep_timer_remaining_secs.hash(&mut h);
    snap.art_path.hash(&mut h);
    snap.motion_cover_path.hash(&mut h);
    snap.ambient_rgb.hash(&mut h);
    snap.liked_track_ids.len().hash(&mut h);
    format!("{:?}", snap.config.language).hash(&mut h);
    snap.config.hifi_substitution.hash(&mut h);
    snap.config.jiosaavn_enabled.hash(&mut h);
    snap.config.gapless_enabled.hash(&mut h);
    snap.config.crossfade_secs.hash(&mut h);
    snap.config.personal_only_mode.hash(&mut h);
    snap.config.motion_covers.hash(&mut h);
    snap.config.reduce_motion.hash(&mut h);
    snap.config.lyrics_translate.hash(&mut h);
    snap.config.lyrics_romanize.hash(&mut h);
    snap.connect_running.hash(&mut h);
    snap.connect_http_port.hash(&mut h);
    snap.connect_pin.hash(&mut h);
    snap.config.connect_device_name.hash(&mut h);
    snap.connect_paired_devices.len().hash(&mut h);
    for dev in &snap.connect_paired_devices {
        dev.device_id.hash(&mut h);
        dev.device_name.hash(&mut h);
    }
    snap.connect_pending_pair.hash(&mut h);
    snap.connect_error.hash(&mut h);
    snap.config.discord_rpc_enabled.hash(&mut h);
    format!("{:?}", snap.discord_status).hash(&mut h);
    format!("{:?}", snap.lyrics).hash(&mut h);
    snap.translated_lyrics
        .as_ref()
        .map(|v| v.len())
        .hash(&mut h);
    snap.romanized_lyrics.as_ref().map(|v| v.len()).hash(&mut h);
    snap.downloaded_track_ids.len().hash(&mut h);
    snap.server_profiles.len().hash(&mut h);
    snap.server_tracks.len().hash(&mut h);
    snap.local_scan.tracks.len().hash(&mut h);
    snap.personal_playlists.len().hash(&mut h);
    snap.stats_summary.total_plays.hash(&mut h);
    snap.stats_window.hash(&mut h);
    h.finish()
}

#[tauri::command]
pub async fn subscribe_state(
    state: State<'_, AppState>,
    on_event: Channel<BackendStreamEvent>,
) -> Result<(), String> {
    let initial = state.backend.snapshot();
    let mut last_struct_hash = snapshot_structure_fingerprint(&initial);
    on_event
        .send(BackendStreamEvent::Snapshot(Box::new(UiSnapshotDto::from(
            initial.as_ref(),
        ))))
        .map_err(|e| e.to_string())?;

    let mut snap_rx = state.backend.subscribe();
    let mut progress_rx = state.backend.subscribe_progress();

    tauri::async_runtime::spawn(async move {
        let mut last_progress_emit = Instant::now() - Duration::from_millis(200);
        let mut last_paused = initial.paused;
        let mut last_pos = initial.position_secs;

        loop {
            tokio::select! {
                changed = snap_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let snap: Arc<DesktopSnapshot> = snap_rx.borrow_and_update().clone();
                    let next_hash = snapshot_structure_fingerprint(&snap);
                    if next_hash != last_struct_hash {
                        last_struct_hash = next_hash;
                        last_paused = snap.paused;
                        let (live_pos, live_dur) = *progress_rx.borrow();
                        let mut dto = UiSnapshotDto::from(snap.as_ref());
                        dto.position_secs = live_pos;
                        if live_dur > 0.0 {
                            dto.duration_secs = live_dur;
                        }
                        last_pos = dto.position_secs;
                        if on_event
                            .send(BackendStreamEvent::Snapshot(Box::new(dto)))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                changed = progress_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let (pos, dur) = *progress_rx.borrow_and_update();
                    let snap = snap_rx.borrow().clone();
                    let paused_changed = snap.paused != last_paused;
                    let jumped = (pos - last_pos).abs() > 0.30;
                    if paused_changed || jumped || last_progress_emit.elapsed() >= Duration::from_millis(45) {
                        last_progress_emit = Instant::now();
                        last_paused = snap.paused;
                        last_pos = pos;
                        if on_event
                            .send(BackendStreamEvent::Progress(ProgressTickDto {
                                position_secs: pos,
                                duration_secs: dur,
                                paused: snap.paused,
                                loading_track: snap.loading_track,
                                playback_speed: snap.playback_speed,
                            }))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn dispatch_cmd(state: State<'_, AppState>, cmd: BackendCmdDto) -> Result<(), String> {
    state.backend.send(cmd.into());
    Ok(())
}

#[tauri::command]
pub fn get_desktop_settings(state: State<'_, AppState>) -> DesktopSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn update_desktop_settings(
    state: State<'_, AppState>,
    settings: DesktopSettings,
) -> Result<(), String> {
    if let Ok(mut guard) = state.settings.lock() {
        *guard = settings.clone();
    }
    settings.save();
    Ok(())
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[cfg(any(target_os = "linux", test))]
fn is_tiling_wm_env(
    hyprland_sig: Option<&str>,
    sway_sock: Option<&str>,
    i3_sock: Option<&str>,
    niri_sock: Option<&str>,
    xdg_current_desktop: Option<&str>,
    xdg_session_desktop: Option<&str>,
    desktop_session: Option<&str>,
) -> bool {
    const TILING_WMS: [&str; 12] = [
        "hyprland",
        "sway",
        "i3",
        "niri",
        "river",
        "bspwm",
        "dwm",
        "awesome",
        "qtile",
        "xmonad",
        "herbstluftwm",
        "leftwm",
    ];
    if [hyprland_sig, sway_sock, i3_sock, niri_sock]
        .into_iter()
        .flatten()
        .any(|v| !v.trim().is_empty())
    {
        return true;
    }
    [xdg_current_desktop, xdg_session_desktop, desktop_session]
        .into_iter()
        .flatten()
        .flat_map(|v| v.split(':'))
        .map(|token| token.trim().to_ascii_lowercase())
        .any(|token| TILING_WMS.contains(&token.as_str()))
}

#[cfg(target_os = "linux")]
fn is_tiling_wm() -> bool {
    let var = |key: &str| std::env::var(key).ok();
    is_tiling_wm_env(
        var("HYPRLAND_INSTANCE_SIGNATURE").as_deref(),
        var("SWAYSOCK").as_deref(),
        var("I3SOCK").as_deref(),
        var("NIRI_SOCKET").as_deref(),
        var("XDG_CURRENT_DESKTOP").as_deref(),
        var("XDG_SESSION_DESKTOP").as_deref(),
        var("DESKTOP_SESSION").as_deref(),
    )
}

#[tauri::command]
pub fn should_show_window_controls() -> bool {
    #[cfg(target_os = "macos")]
    {
        false
    }
    #[cfg(target_os = "linux")]
    {
        !is_tiling_wm()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        true
    }
}

#[tauri::command]
pub fn window_minimize(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn window_toggle_maximize(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_maximized().unwrap_or(false) {
            win.unmaximize().map_err(|e| e.to_string())?;
        } else {
            win.maximize().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn window_close(state: State<'_, AppState>) -> Result<(), String> {
    let cmd = if crate::integrations::tray::is_tray_active() {
        UiWindowCmd::HideWindow
    } else {
        UiWindowCmd::QuitApp
    };
    let _ = state.win_tx.send(cmd);
    Ok(())
}

#[tauri::command]
pub fn window_start_dragging(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn window_toggle_fullscreen(app: AppHandle) -> Result<bool, String> {
    if let Some(win) = app.get_webview_window("main") {
        let next = !win.is_fullscreen().unwrap_or(false);
        win.set_fullscreen(next).map_err(|e| e.to_string())?;
        return Ok(next);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_server_profile_strips_all_credentials() {
        let profile = ServerProfile {
            id: "srv-1".to_string(),
            name: "Home Jellyfin".to_string(),
            kind: ServerKind::Jellyfin,
            base_url: "https://music.example.com".to_string(),
            username: Some("alice".to_string()),
            password: Some("super-secret".to_string()),
            api_key: Some("api-key-secret".to_string()),
            access_token: Some("access-secret".to_string()),
            user_id: Some("user-1".to_string()),
            bearer_token: Some("bearer-secret".to_string()),
            library_ids: vec!["lib-1".to_string()],
            share: None,
            domain: None,
            local_mount_path: None,
            enabled: true,
        };
        let sanitized = sanitize_server_profile(&profile);
        assert!(sanitized.password.is_none());
        assert!(sanitized.api_key.is_none());
        assert!(sanitized.access_token.is_none());
        assert!(sanitized.bearer_token.is_none());
        assert_eq!(sanitized.username.as_deref(), Some("alice"));
    }

    #[test]
    fn nav_target_and_cmd_dto_roundtrip_preserve_variants() {
        let dto = NavTargetDto::MoodCategory {
            title: "Focus".to_string(),
            browse_id: "FEmood".to_string(),
            params: "ggMPO".to_string(),
        };
        let target: NavTarget = dto.into();
        let back = NavTargetDto::from(&target);
        assert!(matches!(back, NavTargetDto::MoodCategory { title, .. } if title == "Focus"));

        let cmd: BackendCmd = BackendCmdDto::SetPlaybackSpeed { speed: 1.25 }.into();
        assert!(matches!(cmd, BackendCmd::SetPlaybackSpeed(s) if (s - 1.25).abs() < f64::EPSILON));
    }

    #[test]
    fn tiling_wm_detection_hides_controls_on_hyprland_and_keeps_on_floating_des() {
        assert!(is_tiling_wm_env(
            Some("abc123"),
            None,
            None,
            None,
            None,
            None,
            None,
        ));
        assert!(is_tiling_wm_env(
            None,
            None,
            None,
            None,
            Some("Hyprland"),
            None,
            None,
        ));
        assert!(is_tiling_wm_env(
            None,
            Some("/run/user/1000/sway-ipc.sock"),
            None,
            None,
            Some("sway"),
            None,
            None,
        ));
        assert!(!is_tiling_wm_env(
            None,
            None,
            None,
            None,
            Some("GNOME"),
            Some("gnome"),
            None,
        ));
        assert!(!is_tiling_wm_env(
            None,
            None,
            None,
            None,
            Some("KDE"),
            Some("plasma"),
            None,
        ));
        assert!(!is_tiling_wm_env(None, None, None, None, None, None, None));
    }

    #[test]
    fn timed_line_from_derives_word_and_line_end_times_from_subsequent_onsets() {
        let lines = vec![
            LyricLine {
                time_secs: 1.0,
                text: "hello world".to_string(),
                voice: LyricVoice::Lead,
                words: vec![
                    sonata_core::lyrics::LyricWord {
                        text: "hello".to_string(),
                        start_secs: 1.0,
                        end_secs: None,
                    },
                    sonata_core::lyrics::LyricWord {
                        text: "world".to_string(),
                        start_secs: 1.85,
                        end_secs: None,
                    },
                ],
            },
            LyricLine {
                time_secs: 4.5,
                text: "plain second line".to_string(),
                voice: LyricVoice::Counter,
                words: Vec::new(),
            },
            LyricLine {
                time_secs: 9.0,
                text: "next line".to_string(),
                voice: LyricVoice::Lead,
                words: Vec::new(),
            },
        ];

        let dto = UiLyricsDto::from(&LyricsState::Synced(lines));
        let UiLyricsDto::Synced { lines: dtos } = dto else {
            panic!("expected synced lyrics");
        };
        assert_eq!(dtos.len(), 3);
        // Word 0 end_ms derives from word 1's onset (1850ms) rather than falling back to +400ms.
        assert_eq!(dtos[0].words[0].end_ms, 1850);
        // Last word end_ms derives from line 1's onset (4500ms, clamped to ws + 2600 = 4450ms).
        assert_eq!(dtos[0].words[1].end_ms, 4450);
        // Plain line end_ms derives from line 2's onset (9000ms) rather than hardcoded +3000ms.
        assert_eq!(dtos[1].end_ms, 9000);
        assert_eq!(dtos[1].speaker.as_deref(), Some("Counter"));
        assert!(!dtos[1].background);

        let overlap_lines = vec![
            LyricLine {
                time_secs: 1.0,
                text: "Lead main verse".to_string(),
                voice: LyricVoice::Lead,
                words: Vec::new(),
            },
            LyricLine {
                time_secs: 2.0,
                text: "(who?)".to_string(),
                voice: LyricVoice::Counter,
                words: Vec::new(),
            },
            LyricLine {
                time_secs: 5.5,
                text: "Next lead verse".to_string(),
                voice: LyricVoice::Lead,
                words: Vec::new(),
            },
        ];
        let UiLyricsDto::Synced {
            lines: overlap_dtos,
        } = UiLyricsDto::from(&LyricsState::Synced(overlap_lines))
        else {
            panic!("expected synced lyrics");
        };
        assert_eq!(overlap_dtos[0].end_ms, 5500);
        assert!(!overlap_dtos[0].background);
        assert!(overlap_dtos[1].background);
        assert_eq!(overlap_dtos[1].speaker.as_deref(), Some("Counter"));
    }
}
