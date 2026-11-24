//! Portable local audio directory scanner and pure-Rust metadata/tag parser.
//!
//! 1. **Directory Scanning**: Recursively scans configured directories for `.flac`, `.mp3`,
//!    `.opus`, `.ogg`, `.m4a`, `.wav`, `.aac`, `.alac`, and `.aiff` files with symlink-loop
//!    protection and zero OS subprocesses or `cfg!(target_os = ...)` branches.
//! 2. **Pure-Rust Metadata Parsers**: Reads FLAC `STREAMINFO` + `VORBIS_COMMENT`, ID3v2/ID3v1 +
//!    MPEG frame headers, Ogg Opus/Vorbis headers + comments, RIFF/WAVE, AIFF, and MP4/M4A/ALAC
//!    atoms, plus `cover.*`/`folder.*`/`front.*` folder artwork discovery.
//! 3. **Grouping & Stream Helpers**: Encodes `local:<encoded_path>` track IDs, computes measured
//!    bitrate from file size and duration for lossless files, and groups tracks by song, album,
//!    and folder.

use crate::error::CoreError;
use crate::providers::ytmusic::{AlbumDetail, HomeCard, HomeCardKind};
use crate::providers::{ProviderSource, Track};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Prefix used for all locally scanned audio track identifiers.
pub const LOCAL_ID_PREFIX: &str = "local:";

/// Supported local audio file extensions (lowercase, without leading dot).
pub const LOCAL_AUDIO_EXTENSIONS: [&str; 9] = [
    "flac", "mp3", "opus", "ogg", "m4a", "wav", "aac", "alac", "aiff",
];

const COVER_BASE_NAMES: [&str; 3] = ["cover", "folder", "front"];
const COVER_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "avif"];
const MAX_SCAN_DEPTH: usize = 16;
const MAX_METADATA_READ_BYTES: usize = 8 * 1024 * 1024;

/// Parsed technical and tag metadata for a single local audio file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAudioMetadata {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album_artist: Option<String>,
    pub album: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<String>,
    pub duration_secs: Option<u64>,
    pub sample_rate: Option<u32>,
    pub bits_per_sample: Option<u16>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub codec: String,
    pub is_lossless: bool,
    pub file_size_bytes: u64,
    pub cover_path: Option<PathBuf>,
}

/// Album grouping synthesized from scanned local tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAlbum {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub year: Option<String>,
    pub cover_path: Option<PathBuf>,
    pub tracks: Vec<Track>,
}

impl LocalAlbum {
    /// Converts this local album into Sonata's shared [`HomeCard`] representation.
    pub fn to_home_card(&self) -> HomeCard {
        HomeCard {
            kind: HomeCardKind::Album,
            id: self.id.clone(),
            title: self.name.clone(),
            subtitle: self.artist.clone(),
            thumbnail_url: self
                .cover_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            track: self.tracks.first().cloned(),
        }
    }

    /// Converts this local album into Sonata's [`AlbumDetail`] view model.
    pub fn to_album_detail(&self) -> AlbumDetail {
        let total_secs: u64 = self.tracks.iter().filter_map(|t| t.duration_secs).sum();
        let duration_text = if total_secs > 0 {
            Some(format!("{}:{:02}", total_secs / 60, total_secs % 60))
        } else {
            None
        };
        AlbumDetail {
            id: self.id.clone(),
            title: self.name.clone(),
            subtitle: self.artist.clone(),
            release_type: "Album".to_string(),
            year: self.year.clone(),
            duration_text,
            cover_url: self
                .cover_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            artists: vec![(self.artist.clone(), None)],
            tracks: self.tracks.clone(),
            other_versions: Vec::new(),
            playlist_id: None,
            saved: true,
        }
    }
}

/// Directory grouping synthesized from scanned local tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFolder {
    pub path: PathBuf,
    pub name: String,
    pub cover_path: Option<PathBuf>,
    pub tracks: Vec<Track>,
}

/// Complete grouped result from scanning one or more local music directories.
#[derive(Debug, Clone, Default)]
pub struct LocalScanResult {
    pub scanned_dirs: Vec<PathBuf>,
    pub tracks: Vec<Track>,
    pub albums: Vec<LocalAlbum>,
    pub folders: Vec<LocalFolder>,
}

#[derive(Debug, Default)]
struct ParsedTags {
    title: Option<String>,
    artist: Option<String>,
    album_artist: Option<String>,
    album: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    year: Option<String>,
    duration_secs_f64: Option<f64>,
    sample_rate: Option<u32>,
    bits_per_sample: Option<u16>,
    channels: Option<u8>,
    bitrate_kbps: Option<u32>,
    codec: Option<String>,
    is_lossless: bool,
}

/// Returns `true` when `path` has a supported local audio file extension.
pub fn is_supported_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            LOCAL_AUDIO_EXTENSIONS
                .iter()
                .any(|cand| ext.eq_ignore_ascii_case(cand))
        })
}

/// Encodes a local filesystem path into a canonical `local:<encoded_path>` track ID.
pub fn encode_local_track_id(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("{LOCAL_ID_PREFIX}{}", urlencoding::encode(&raw))
}

/// Decodes a `local:<encoded_path>` (or `local:file://...`) track ID back into a [`PathBuf`].
pub fn decode_local_track_id(id: &str) -> Option<PathBuf> {
    let raw = id.strip_prefix(LOCAL_ID_PREFIX)?;
    if raw.is_empty() {
        return None;
    }
    let decoded = urlencoding::decode(raw)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let plain = decoded.strip_prefix("file://").unwrap_or(&decoded);
    if plain.is_empty() {
        None
    } else {
        Some(PathBuf::from(plain))
    }
}

/// Computes bitrate in `kbps` from file size in bytes and duration in seconds.
pub fn compute_bitrate_kbps(file_size_bytes: u64, duration_secs: f64) -> Option<u32> {
    if file_size_bytes == 0 || !duration_secs.is_finite() || duration_secs <= 0.0 {
        return None;
    }
    let kbps = ((file_size_bytes as f64) * 8.0) / (duration_secs * 1000.0);
    if kbps.is_finite() && kbps > 0.0 {
        Some(kbps.round().max(1.0) as u32)
    } else {
        None
    }
}

/// Formats a crisp local audio quality badge (e.g. `FLAC · 880 kbps · 16-bit/44.1kHz`).
pub fn format_local_audio_badge(
    codec: &str,
    bitrate_kbps: Option<u32>,
    bits_per_sample: Option<u16>,
    sample_rate: Option<u32>,
) -> String {
    let codec_up = codec.trim().to_ascii_uppercase();
    let mut parts = vec![if codec_up.is_empty() {
        "AUDIO".to_string()
    } else {
        codec_up
    }];
    if let Some(kbps) = bitrate_kbps.filter(|&k| k > 0) {
        parts.push(format!("{kbps} kbps"));
    }
    match (
        bits_per_sample.filter(|&b| b > 0),
        sample_rate.filter(|&r| r > 0),
    ) {
        (Some(bits), Some(sr)) => parts.push(format!("{bits}-bit/{}", format_sample_rate(sr))),
        (None, Some(sr)) => parts.push(format_sample_rate(sr)),
        (Some(bits), None) => parts.push(format!("{bits}-bit")),
        (None, None) => {}
    }
    if parts.len() == 1 {
        parts.push("Local".to_string());
    }
    parts.join(" · ")
}

fn format_sample_rate(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{}kHz", hz / 1000)
    } else {
        format!("{:.1}kHz", hz as f64 / 1000.0)
    }
}

/// Searches `dir` for a folder cover image (`cover.*`, `folder.*`, or `front.*`).
pub fn find_folder_cover(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut best: Option<(usize, usize, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(name_rank) = COVER_BASE_NAMES
            .iter()
            .position(|cand| stem.eq_ignore_ascii_case(cand))
        else {
            continue;
        };
        let Some(ext_rank) = COVER_EXTENSIONS
            .iter()
            .position(|cand| ext.eq_ignore_ascii_case(cand))
        else {
            continue;
        };
        let replace = match &best {
            None => true,
            Some((best_name, best_ext, _)) => (name_rank, ext_rank) < (*best_name, *best_ext),
        };
        if replace {
            best = Some((name_rank, ext_rank, path));
        }
    }
    best.map(|(_, _, p)| p)
}

/// Parses audio tags, technical stream parameters, and folder cover for a single file.
pub fn parse_audio_file(path: &Path) -> Option<LocalAudioMetadata> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let file_size_bytes = meta.len();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let mut file = File::open(path).ok()?;
    let mut magic = [0u8; 12];
    let n = file.read(&mut magic).ok()?;
    let head = &magic[..n];
    let _ = file.seek(SeekFrom::Start(0));

    let mut parsed = ParsedTags::default();
    if head.len() >= 4 && &head[0..4] == b"fLaC" {
        parse_flac(&mut file, file_size_bytes, &mut parsed);
    } else if head.len() >= 4 && &head[0..4] == b"OggS" {
        parse_ogg(&mut file, file_size_bytes, &mut parsed);
    } else if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE" {
        parse_wav(&mut file, file_size_bytes, &mut parsed);
    } else if head.len() >= 12
        && &head[0..4] == b"FORM"
        && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC")
    {
        parse_aiff(&mut file, file_size_bytes, &mut parsed);
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        parse_mp4(&mut file, file_size_bytes, &ext, &mut parsed);
    } else if (head.len() >= 3 && &head[0..3] == b"ID3") || ext == "mp3" || ext == "aac" {
        parse_mp3_or_aac(&mut file, file_size_bytes, &ext, &mut parsed);
    }

    let codec = parsed
        .codec
        .take()
        .unwrap_or_else(|| default_codec_for_extension(&ext).to_string());
    let is_lossless =
        parsed.is_lossless || matches!(codec.as_str(), "FLAC" | "WAV" | "ALAC" | "AIFF");

    let duration_secs = parsed
        .duration_secs_f64
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| d.round().max(1.0) as u64);

    let bitrate_kbps = if is_lossless {
        parsed
            .duration_secs_f64
            .and_then(|dur| compute_bitrate_kbps(file_size_bytes, dur))
            .or(parsed.bitrate_kbps)
    } else {
        parsed.bitrate_kbps.or_else(|| {
            parsed
                .duration_secs_f64
                .and_then(|dur| compute_bitrate_kbps(file_size_bytes, dur))
        })
    };

    let (fallback_artist, fallback_album, fallback_title) = infer_from_path(path);
    let title = parsed
        .title
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_title);
    let artist = parsed
        .artist
        .or_else(|| parsed.album_artist.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_artist);
    let album = parsed
        .album
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_album);

    let cover_path = path.parent().and_then(find_folder_cover);

    Some(LocalAudioMetadata {
        path: path.to_path_buf(),
        title,
        artist,
        album_artist: parsed.album_artist,
        album,
        track_number: parsed.track_number,
        disc_number: parsed.disc_number,
        year: parsed.year,
        duration_secs,
        sample_rate: parsed.sample_rate,
        bits_per_sample: parsed.bits_per_sample,
        channels: parsed.channels,
        bitrate_kbps,
        codec,
        is_lossless,
        file_size_bytes,
        cover_path,
    })
}

/// Converts [`LocalAudioMetadata`] into a Sonata [`Track`] with `local:<encoded_path>` ID.
pub fn metadata_to_track(meta: &LocalAudioMetadata) -> Track {
    let badge = format_local_audio_badge(
        &meta.codec,
        meta.bitrate_kbps,
        meta.bits_per_sample,
        meta.sample_rate,
    );
    Track {
        id: encode_local_track_id(&meta.path),
        title: meta.title.clone(),
        artist: meta.artist.clone(),
        album: meta.album.clone(),
        duration_secs: meta.duration_secs,
        thumbnail_url: meta
            .cover_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        source: ProviderSource::YouTubeMusic,
        audio_badge: Some(badge),
        set_video_id: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    }
}

/// Scans a single directory recursively into grouped songs, albums, and folders.
pub fn scan_directory(dir: &Path) -> Result<LocalScanResult, CoreError> {
    if !dir.exists() {
        return Err(CoreError::not_found(format!(
            "local music directory does not exist: {}",
            dir.display()
        )));
    }
    if !dir.is_dir() {
        return Err(CoreError::io(format!(
            "local music path is not a directory: {}",
            dir.display()
        )));
    }
    Ok(scan_directories(&[dir.to_path_buf()]))
}

/// Scans all provided directories recursively and groups discovered tracks by song, album, and folder.
pub fn scan_directories(dirs: &[PathBuf]) -> LocalScanResult {
    let mut visited_dirs = HashSet::new();
    let mut seen_files = HashSet::new();
    let mut items: Vec<LocalAudioMetadata> = Vec::new();
    let mut scanned_dirs = Vec::new();

    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        scanned_dirs.push(dir.clone());
        collect_dir_recursive(dir, 0, &mut visited_dirs, &mut seen_files, &mut items);
    }

    items.sort_by(|a, b| {
        a.album_artist
            .as_deref()
            .unwrap_or(&a.artist)
            .to_ascii_lowercase()
            .cmp(
                &b.album_artist
                    .as_deref()
                    .unwrap_or(&b.artist)
                    .to_ascii_lowercase(),
            )
            .then_with(|| {
                a.album
                    .to_ascii_lowercase()
                    .cmp(&b.album.to_ascii_lowercase())
            })
            .then_with(|| a.disc_number.unwrap_or(1).cmp(&b.disc_number.unwrap_or(1)))
            .then_with(|| {
                a.track_number
                    .unwrap_or(u32::MAX)
                    .cmp(&b.track_number.unwrap_or(u32::MAX))
            })
            .then_with(|| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            })
    });

    let mut tracks = Vec::with_capacity(items.len());
    let mut album_groups: BTreeMap<(String, String), LocalAlbum> = BTreeMap::new();
    let mut folder_groups: BTreeMap<PathBuf, LocalFolder> = BTreeMap::new();

    for meta in &items {
        let track = metadata_to_track(meta);
        tracks.push(track.clone());

        let album_artist = meta
            .album_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&meta.artist)
            .to_string();
        let album_name = if meta.album.trim().is_empty() {
            "Singles".to_string()
        } else {
            meta.album.clone()
        };
        let album_key = (
            album_name.to_ascii_lowercase(),
            album_artist.to_ascii_lowercase(),
        );
        let album_id = format!(
            "local-album:{}",
            urlencoding::encode(&format!("{album_artist}::{album_name}"))
        );
        let album_entry = album_groups.entry(album_key).or_insert_with(|| LocalAlbum {
            id: album_id,
            name: album_name,
            artist: album_artist,
            year: meta.year.clone(),
            cover_path: meta.cover_path.clone(),
            tracks: Vec::new(),
        });
        if album_entry.cover_path.is_none() {
            album_entry.cover_path = meta.cover_path.clone();
        }
        if album_entry.year.is_none() {
            album_entry.year = meta.year.clone();
        }
        album_entry.tracks.push(track.clone());

        if let Some(parent) = meta.path.parent() {
            let folder_name = parent
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("Music")
                .to_string();
            let folder_entry = folder_groups
                .entry(parent.to_path_buf())
                .or_insert_with(|| LocalFolder {
                    path: parent.to_path_buf(),
                    name: folder_name,
                    cover_path: meta.cover_path.clone(),
                    tracks: Vec::new(),
                });
            if folder_entry.cover_path.is_none() {
                folder_entry.cover_path = meta.cover_path.clone();
            }
            folder_entry.tracks.push(track);
        }
    }

    LocalScanResult {
        scanned_dirs,
        tracks,
        albums: album_groups.into_values().collect(),
        folders: folder_groups.into_values().collect(),
    }
}

fn collect_dir_recursive(
    dir: &Path,
    depth: usize,
    visited_dirs: &mut HashSet<PathBuf>,
    seen_files: &mut HashSet<PathBuf>,
    out: &mut Vec<LocalAudioMetadata>,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited_dirs.insert(canonical) {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    let mut audio_files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.is_file() && is_supported_audio_path(&path) {
            let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen_files.insert(key) {
                audio_files.push(path);
            }
        }
    }
    audio_files.sort();
    subdirs.sort();

    for file_path in audio_files {
        if let Some(meta) = parse_audio_file(&file_path) {
            out.push(meta);
        }
    }
    for sub in subdirs {
        collect_dir_recursive(&sub, depth + 1, visited_dirs, seen_files, out);
    }
}

fn default_codec_for_extension(ext: &str) -> &'static str {
    match ext {
        "flac" => "FLAC",
        "mp3" => "MP3",
        "opus" => "OPUS",
        "ogg" => "VORBIS",
        "m4a" | "aac" => "AAC",
        "alac" => "ALAC",
        "wav" => "WAV",
        "aiff" => "AIFF",
        _ => "AUDIO",
    }
}

fn infer_from_path(path: &Path) -> (String, String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Track");
    let title = clean_filename_title(stem);
    let parent = path.parent();
    let album = parent
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    let artist = parent
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown Artist")
        .to_string();
    (artist, album, title)
}

fn clean_filename_title(stem: &str) -> String {
    let trimmed = stem.trim();
    let bytes = trimmed.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && (bytes[idx].is_ascii_digit() || bytes[idx] == b'-') {
        idx += 1;
    }
    if idx > 0 && idx < trimmed.len() && trimmed[..idx].bytes().any(|b| b.is_ascii_digit()) {
        let rest = trimmed[idx..].trim_start();
        if let Some(after) = rest
            .strip_prefix('-')
            .or_else(|| rest.strip_prefix('.'))
            .map(str::trim)
        {
            if !after.is_empty() {
                return after.to_string();
            }
        }
        if rest.len() < trimmed[idx..].len() && !rest.is_empty() {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

fn parse_flac(file: &mut File, _file_size: u64, out: &mut ParsedTags) {
    out.codec = Some("FLAC".to_string());
    out.is_lossless = true;

    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || &magic != b"fLaC" {
        return;
    }

    for _ in 0..128 {
        let mut hdr = [0u8; 4];
        if file.read_exact(&mut hdr).is_err() {
            break;
        }
        let is_last = (hdr[0] & 0x80) != 0;
        let block_type = hdr[0] & 0x7F;
        let block_len = ((hdr[1] as usize) << 16) | ((hdr[2] as usize) << 8) | (hdr[3] as usize);

        if block_type == 0 && (34..=1024).contains(&block_len) {
            let mut buf = vec![0u8; block_len];
            if file.read_exact(&mut buf).is_ok() {
                let sample_rate =
                    ((buf[10] as u32) << 12) | ((buf[11] as u32) << 4) | ((buf[12] as u32) >> 4);
                let channels = ((buf[12] >> 1) & 0x07) + 1;
                let bits_per_sample = (((buf[12] & 0x01) << 4) | (buf[13] >> 4)) + 1;
                let total_samples = (((buf[13] & 0x0F) as u64) << 32)
                    | (u32::from_be_bytes([buf[14], buf[15], buf[16], buf[17]]) as u64);
                if sample_rate > 0 {
                    out.sample_rate = Some(sample_rate);
                }
                out.channels = Some(channels);
                out.bits_per_sample = Some(bits_per_sample as u16);
                if sample_rate > 0 && total_samples > 0 {
                    out.duration_secs_f64 = Some(total_samples as f64 / sample_rate as f64);
                }
            } else {
                break;
            }
        } else if block_type == 4 && block_len <= MAX_METADATA_READ_BYTES {
            let mut buf = vec![0u8; block_len];
            if file.read_exact(&mut buf).is_ok() {
                parse_vorbis_comment(&buf, out);
            } else {
                break;
            }
        } else if file.seek(SeekFrom::Current(block_len as i64)).is_err() {
            break;
        }

        if is_last {
            break;
        }
    }
}

fn parse_vorbis_comment(data: &[u8], out: &mut ParsedTags) {
    if data.len() < 8 {
        return;
    }
    let vendor_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = match 4usize.checked_add(vendor_len) {
        Some(p) if p + 4 <= data.len() => p,
        _ => return,
    };
    let count =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;

    for _ in 0..count.min(4096) {
        if pos + 4 > data.len() {
            break;
        }
        let len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + len > data.len() {
            break;
        }
        if let Ok(entry) = std::str::from_utf8(&data[pos..pos + len]) {
            if let Some((key, val)) = entry.split_once('=') {
                apply_tag_pair(key, val, out);
            }
        }
        pos += len;
    }
}

fn apply_tag_pair(key: &str, raw_val: &str, out: &mut ParsedTags) {
    let val = raw_val.trim().trim_end_matches('\0').trim();
    if val.is_empty() {
        return;
    }
    match key.trim().to_ascii_uppercase().as_str() {
        "TITLE" => {
            out.title.get_or_insert_with(|| val.to_string());
        }
        "ARTIST" => {
            out.artist.get_or_insert_with(|| val.to_string());
        }
        "ALBUMARTIST" | "ALBUM ARTIST" => {
            out.album_artist.get_or_insert_with(|| val.to_string());
        }
        "ALBUM" => {
            out.album.get_or_insert_with(|| val.to_string());
        }
        "TRACKNUMBER" | "TRACK" if out.track_number.is_none() => {
            out.track_number = parse_leading_u32(val);
        }
        "DISCNUMBER" | "DISC" if out.disc_number.is_none() => {
            out.disc_number = parse_leading_u32(val);
        }
        "DATE" | "YEAR" if out.year.is_none() => {
            let y = val.chars().take(4).collect::<String>();
            if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) {
                out.year = Some(y);
            }
        }
        _ => {}
    }
}

fn parse_leading_u32(val: &str) -> Option<u32> {
    let part = val.split('/').next().unwrap_or(val).trim();
    part.parse::<u32>().ok().filter(|&n| n > 0)
}

fn parse_mp3_or_aac(file: &mut File, file_size: u64, ext: &str, out: &mut ParsedTags) {
    let mut id3_offset = 0u64;
    let mut hdr = [0u8; 10];
    if file.read_exact(&mut hdr).is_ok() && &hdr[0..3] == b"ID3" {
        let major = hdr[3];
        let flags = hdr[5];
        let tag_size = synchsafe_u32(&hdr[6..10]) as usize;
        id3_offset = 10 + tag_size as u64;
        if tag_size > 0 && tag_size <= MAX_METADATA_READ_BYTES {
            let mut body = vec![0u8; tag_size];
            if file.read_exact(&mut body).is_ok() {
                parse_id3v2_body(major, flags, &body, out);
            }
        }
    }

    if file_size >= 128
        && (out.title.is_none() || out.artist.is_none() || out.album.is_none())
        && file.seek(SeekFrom::End(-128)).is_ok()
    {
        let mut v1 = [0u8; 128];
        if file.read_exact(&mut v1).is_ok() && &v1[0..3] == b"TAG" {
            if out.title.is_none() {
                out.title = decode_latin1(&v1[3..33]);
            }
            if out.artist.is_none() {
                out.artist = decode_latin1(&v1[33..63]);
            }
            if out.album.is_none() {
                out.album = decode_latin1(&v1[63..93]);
            }
            if out.year.is_none() {
                out.year = decode_latin1(&v1[93..97])
                    .filter(|y| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()));
            }
        }
    }

    if ext == "aac" {
        out.codec = Some("AAC".to_string());
        return;
    }

    out.codec = Some("MP3".to_string());
    if file.seek(SeekFrom::Start(id3_offset)).is_ok() {
        let mut scan = vec![0u8; 32 * 1024];
        if let Ok(n) = file.read(&mut scan) {
            scan.truncate(n);
            parse_mpeg_audio_frame(&scan, file_size.saturating_sub(id3_offset), out);
        }
    }
}

fn synchsafe_u32(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32 & 0x7F) << 21)
        | ((bytes[1] as u32 & 0x7F) << 14)
        | ((bytes[2] as u32 & 0x7F) << 7)
        | (bytes[3] as u32 & 0x7F)
}

fn parse_id3v2_body(major: u8, flags: u8, body: &[u8], out: &mut ParsedTags) {
    let mut pos = 0usize;
    if (flags & 0x40) != 0 && body.len() >= 4 {
        let ext_len = if major >= 4 {
            synchsafe_u32(&body[0..4]) as usize
        } else {
            u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize + 4
        };
        pos = pos.saturating_add(ext_len).min(body.len());
    }

    if major == 2 {
        while pos + 6 <= body.len() {
            let id = &body[pos..pos + 3];
            if id == [0, 0, 0] {
                break;
            }
            let size = ((body[pos + 3] as usize) << 16)
                | ((body[pos + 4] as usize) << 8)
                | (body[pos + 5] as usize);
            pos += 6;
            if pos + size > body.len() {
                break;
            }
            if let Ok(frame_id) = std::str::from_utf8(id) {
                apply_id3_frame(frame_id, &body[pos..pos + size], out);
            }
            pos += size;
        }
        return;
    }

    while pos + 10 <= body.len() {
        let id = &body[pos..pos + 4];
        if id == [0, 0, 0, 0] {
            break;
        }
        let size = if major >= 4 {
            synchsafe_u32(&body[pos + 4..pos + 8]) as usize
        } else {
            u32::from_be_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]])
                as usize
        };
        pos += 10;
        if size == 0 || pos + size > body.len() {
            break;
        }
        if let Ok(frame_id) = std::str::from_utf8(id) {
            apply_id3_frame(frame_id, &body[pos..pos + size], out);
        }
        pos += size;
    }
}

fn apply_id3_frame(frame_id: &str, payload: &[u8], out: &mut ParsedTags) {
    let Some(text) = decode_id3_text(payload) else {
        return;
    };
    let clean = text.trim();
    if clean.is_empty() {
        return;
    }
    match frame_id {
        "TIT2" | "TT2" => {
            out.title.get_or_insert_with(|| clean.to_string());
        }
        "TPE1" | "TP1" => {
            out.artist.get_or_insert_with(|| clean.to_string());
        }
        "TPE2" | "TP2" => {
            out.album_artist.get_or_insert_with(|| clean.to_string());
        }
        "TALB" | "TAL" => {
            out.album.get_or_insert_with(|| clean.to_string());
        }
        "TRCK" | "TRK" => {
            if out.track_number.is_none() {
                out.track_number = parse_leading_u32(clean);
            }
        }
        "TPOS" | "TPA" => {
            if out.disc_number.is_none() {
                out.disc_number = parse_leading_u32(clean);
            }
        }
        "TDRC" | "TYER" | "TYE" => {
            if out.year.is_none() {
                let y = clean.chars().take(4).collect::<String>();
                if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) {
                    out.year = Some(y);
                }
            }
        }
        "TLEN" if out.duration_secs_f64.is_none() => {
            if let Ok(ms) = clean.parse::<u64>() {
                if ms > 0 {
                    out.duration_secs_f64 = Some(ms as f64 / 1000.0);
                }
            }
        }
        _ => {}
    }
}

fn decode_id3_text(payload: &[u8]) -> Option<String> {
    if payload.is_empty() {
        return None;
    }
    let enc = payload[0];
    let raw = &payload[1..];
    let text = match enc {
        0 => decode_latin1(raw)?,
        1 => decode_utf16_with_bom(raw)?,
        2 => decode_utf16_units(raw, false)?,
        3 => String::from_utf8_lossy(raw).into_owned(),
        _ => return None,
    };
    let trimmed = text.trim_end_matches('\0').trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn decode_latin1(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let s: String = raw[..end].iter().map(|&b| b as char).collect();
    let trimmed = s.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn decode_utf16_with_bom(raw: &[u8]) -> Option<String> {
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        decode_utf16_units(&raw[2..], true)
    } else if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        decode_utf16_units(&raw[2..], false)
    } else {
        decode_utf16_units(raw, true)
    }
}

fn decode_utf16_units(raw: &[u8], little_endian: bool) -> Option<String> {
    let mut units = Vec::with_capacity(raw.len() / 2);
    for pair in raw.as_chunks::<2>().0 {
        let code = if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        };
        if code == 0 {
            break;
        }
        units.push(code);
    }
    String::from_utf16(&units).ok()
}

fn parse_mpeg_audio_frame(buf: &[u8], audio_bytes: u64, out: &mut ParsedTags) {
    const V1_L3_BITRATES: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    const V2_L3_BITRATES: [u32; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];
    const V1_RATES: [u32; 4] = [44_100, 48_000, 32_000, 0];
    const V2_RATES: [u32; 4] = [22_050, 24_000, 16_000, 0];
    const V25_RATES: [u32; 4] = [11_025, 12_000, 8_000, 0];

    for i in 0..buf.len().saturating_sub(4) {
        if buf[i] != 0xFF || (buf[i + 1] & 0xE0) != 0xE0 {
            continue;
        }
        let version_bits = (buf[i + 1] >> 3) & 0x03;
        let layer_bits = (buf[i + 1] >> 1) & 0x03;
        let bitrate_idx = ((buf[i + 2] >> 4) & 0x0F) as usize;
        let rate_idx = ((buf[i + 2] >> 2) & 0x03) as usize;
        let channel_mode = (buf[i + 3] >> 6) & 0x03;

        if version_bits == 1
            || layer_bits != 1
            || bitrate_idx == 0
            || bitrate_idx == 15
            || rate_idx == 3
        {
            continue;
        }
        let sample_rate = match version_bits {
            3 => V1_RATES[rate_idx],
            2 => V2_RATES[rate_idx],
            0 => V25_RATES[rate_idx],
            _ => 0,
        };
        let kbps = if version_bits == 3 {
            V1_L3_BITRATES[bitrate_idx]
        } else {
            V2_L3_BITRATES[bitrate_idx]
        };
        if sample_rate == 0 || kbps == 0 {
            continue;
        }

        out.sample_rate = Some(sample_rate);
        out.channels = Some(if channel_mode == 3 { 1 } else { 2 });

        let side_info = match (version_bits == 3, channel_mode == 3) {
            (true, true) => 17,
            (true, false) => 32,
            (false, true) => 9,
            (false, false) => 17,
        };
        let xing_pos = i + 4 + side_info;
        if xing_pos + 12 <= buf.len() {
            let tag = &buf[xing_pos..xing_pos + 4];
            if tag == b"Xing" || tag == b"Info" {
                let xing_flags = u32::from_be_bytes([
                    buf[xing_pos + 4],
                    buf[xing_pos + 5],
                    buf[xing_pos + 6],
                    buf[xing_pos + 7],
                ]);
                if (xing_flags & 0x01) != 0 {
                    let frames = u32::from_be_bytes([
                        buf[xing_pos + 8],
                        buf[xing_pos + 9],
                        buf[xing_pos + 10],
                        buf[xing_pos + 11],
                    ]);
                    let samples_per_frame = if version_bits == 3 { 1152 } else { 576 };
                    if frames > 0 {
                        let dur = (frames as f64 * samples_per_frame as f64) / sample_rate as f64;
                        out.duration_secs_f64 = Some(dur);
                        if let Some(avg) = compute_bitrate_kbps(audio_bytes, dur) {
                            out.bitrate_kbps = Some(avg);
                            return;
                        }
                    }
                }
            }
        }

        out.bitrate_kbps = Some(kbps);
        if out.duration_secs_f64.is_none() && audio_bytes > 0 {
            out.duration_secs_f64 = Some((audio_bytes as f64 * 8.0) / (kbps as f64 * 1000.0));
        }
        return;
    }
}

fn parse_ogg(file: &mut File, file_size: u64, out: &mut ParsedTags) {
    let read_len = file_size.min(256 * 1024) as usize;
    let mut head_buf = vec![0u8; read_len];
    if file.read_exact(&mut head_buf).is_err() {
        return;
    }

    let packets = extract_ogg_packets(&head_buf, 3);
    if packets.is_empty() {
        return;
    }

    let mut pre_skip = 0u64;
    let mut ogg_sample_rate = 0u32;

    if packets[0].starts_with(b"OpusHead") && packets[0].len() >= 19 {
        out.codec = Some("OPUS".to_string());
        out.channels = Some(packets[0][9]);
        pre_skip = u16::from_le_bytes([packets[0][10], packets[0][11]]) as u64;
        ogg_sample_rate = 48_000;
        out.sample_rate = Some(48_000);
        if let Some(pkt1) = packets.get(1) {
            if pkt1.starts_with(b"OpusTags") && pkt1.len() > 8 {
                parse_vorbis_comment(&pkt1[8..], out);
            }
        }
    } else if packets[0].starts_with(b"\x01vorbis") && packets[0].len() >= 28 {
        out.codec = Some("VORBIS".to_string());
        out.channels = Some(packets[0][11]);
        let sr = u32::from_le_bytes([
            packets[0][12],
            packets[0][13],
            packets[0][14],
            packets[0][15],
        ]);
        let nom = i32::from_le_bytes([
            packets[0][20],
            packets[0][21],
            packets[0][22],
            packets[0][23],
        ]);
        if sr > 0 {
            ogg_sample_rate = sr;
            out.sample_rate = Some(sr);
        }
        if nom > 0 {
            out.bitrate_kbps = Some((nom as u32) / 1000);
        }
        for pkt in &packets[1..] {
            if pkt.starts_with(b"\x03vorbis") && pkt.len() > 7 {
                parse_vorbis_comment(&pkt[7..], out);
                break;
            }
        }
    }

    if ogg_sample_rate > 0 {
        let tail_len = file_size.min(64 * 1024);
        let tail_start = file_size.saturating_sub(tail_len);
        if file.seek(SeekFrom::Start(tail_start)).is_ok() {
            let mut tail = vec![0u8; tail_len as usize];
            if file.read_exact(&mut tail).is_ok() {
                if let Some(granule) = find_last_ogg_granule(&tail) {
                    let effective = granule.saturating_sub(pre_skip);
                    if effective > 0 {
                        out.duration_secs_f64 = Some(effective as f64 / ogg_sample_rate as f64);
                    }
                }
            }
        }
    }
}

fn extract_ogg_packets(buf: &[u8], max_packets: usize) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let mut current = Vec::new();
    let mut pos = 0usize;

    while pos + 27 <= buf.len() && packets.len() < max_packets {
        if &buf[pos..pos + 4] != b"OggS" {
            break;
        }
        let seg_count = buf[pos + 26] as usize;
        if pos + 27 + seg_count > buf.len() {
            break;
        }
        let seg_table = &buf[pos + 27..pos + 27 + seg_count];
        let mut data_pos = pos + 27 + seg_count;
        for &lacing in seg_table {
            let len = lacing as usize;
            if data_pos + len > buf.len() {
                return packets;
            }
            current.extend_from_slice(&buf[data_pos..data_pos + len]);
            data_pos += len;
            if lacing < 255 {
                packets.push(std::mem::take(&mut current));
                if packets.len() >= max_packets {
                    return packets;
                }
            }
        }
        pos = data_pos;
    }
    if !current.is_empty() && packets.len() < max_packets {
        packets.push(current);
    }
    packets
}

fn find_last_ogg_granule(buf: &[u8]) -> Option<u64> {
    let mut last = None;
    for i in 0..buf.len().saturating_sub(27) {
        if &buf[i..i + 4] == b"OggS" && buf[i + 4] == 0 {
            let g = i64::from_le_bytes([
                buf[i + 6],
                buf[i + 7],
                buf[i + 8],
                buf[i + 9],
                buf[i + 10],
                buf[i + 11],
                buf[i + 12],
                buf[i + 13],
            ]);
            if g > 0 {
                last = Some(g as u64);
            }
        }
    }
    last
}

fn parse_wav(file: &mut File, _file_size: u64, out: &mut ParsedTags) {
    out.codec = Some("WAV".to_string());
    out.is_lossless = true;

    let mut hdr = [0u8; 12];
    if file.read_exact(&mut hdr).is_err() || &hdr[0..4] != b"RIFF" || &hdr[8..12] != b"WAVE" {
        return;
    }

    let mut byte_rate = 0u32;
    let mut data_len = 0u32;
    for _ in 0..64 {
        let mut chunk_hdr = [0u8; 8];
        if file.read_exact(&mut chunk_hdr).is_err() {
            break;
        }
        let id = &chunk_hdr[0..4];
        let size = u32::from_le_bytes([chunk_hdr[4], chunk_hdr[5], chunk_hdr[6], chunk_hdr[7]]);
        let padded = size as u64 + u64::from(size & 1);
        if id == b"fmt " && (16..=4096).contains(&size) {
            let mut fmt = vec![0u8; size as usize];
            if file.read_exact(&mut fmt).is_ok() {
                let channels = u16::from_le_bytes([fmt[2], fmt[3]]) as u8;
                let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                byte_rate = u32::from_le_bytes([fmt[8], fmt[9], fmt[10], fmt[11]]);
                let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
                if channels > 0 {
                    out.channels = Some(channels);
                }
                if sample_rate > 0 {
                    out.sample_rate = Some(sample_rate);
                }
                if bits > 0 {
                    out.bits_per_sample = Some(bits);
                }
            }
            if (size & 1) == 1 {
                let _ = file.seek(SeekFrom::Current(1));
            }
        } else if id == b"data" {
            data_len = size;
            if file.seek(SeekFrom::Current(padded as i64)).is_err() {
                break;
            }
        } else if file.seek(SeekFrom::Current(padded as i64)).is_err() {
            break;
        }
    }

    if byte_rate > 0 && data_len > 0 {
        out.duration_secs_f64 = Some(data_len as f64 / byte_rate as f64);
    }
}

fn parse_aiff(file: &mut File, _file_size: u64, out: &mut ParsedTags) {
    out.codec = Some("AIFF".to_string());
    out.is_lossless = true;

    let mut hdr = [0u8; 12];
    if file.read_exact(&mut hdr).is_err() || &hdr[0..4] != b"FORM" {
        return;
    }

    for _ in 0..64 {
        let mut chunk_hdr = [0u8; 8];
        if file.read_exact(&mut chunk_hdr).is_err() {
            break;
        }
        let id = &chunk_hdr[0..4];
        let size = u32::from_be_bytes([chunk_hdr[4], chunk_hdr[5], chunk_hdr[6], chunk_hdr[7]]);
        let padded = size as u64 + u64::from(size & 1);
        if id == b"COMM" && (18..=4096).contains(&size) {
            let mut comm = vec![0u8; size as usize];
            if file.read_exact(&mut comm).is_ok() {
                let channels = i16::from_be_bytes([comm[0], comm[1]]).max(0) as u8;
                let frames = u32::from_be_bytes([comm[2], comm[3], comm[4], comm[5]]);
                let bits = i16::from_be_bytes([comm[6], comm[7]]).max(0) as u16;
                let sr = decode_ieee80_extended(&comm[8..18]);
                if channels > 0 {
                    out.channels = Some(channels);
                }
                if bits > 0 {
                    out.bits_per_sample = Some(bits);
                }
                if sr > 0.0 {
                    out.sample_rate = Some(sr.round() as u32);
                    if frames > 0 {
                        out.duration_secs_f64 = Some(frames as f64 / sr);
                    }
                }
            }
            if (size & 1) == 1 {
                let _ = file.seek(SeekFrom::Current(1));
            }
        } else if file.seek(SeekFrom::Current(padded as i64)).is_err() {
            break;
        }
    }
}

fn decode_ieee80_extended(bytes: &[u8]) -> f64 {
    if bytes.len() < 10 {
        return 0.0;
    }
    let exp = (((bytes[0] & 0x7F) as i32) << 8) | (bytes[1] as i32);
    let mantissa = u64::from_be_bytes([
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9],
    ]);
    if exp == 0 && mantissa == 0 {
        return 0.0;
    }
    let sign = if (bytes[0] & 0x80) != 0 { -1.0 } else { 1.0 };
    let shift = exp - 16383 - 63;
    sign * (mantissa as f64) * (2.0_f64).powi(shift)
}

fn parse_mp4(file: &mut File, file_size: u64, ext: &str, out: &mut ParsedTags) {
    if ext == "alac" {
        out.codec = Some("ALAC".to_string());
        out.is_lossless = true;
    } else {
        out.codec = Some("AAC".to_string());
    }

    let mut pos = 0u64;
    while pos + 8 <= file_size {
        if file.seek(SeekFrom::Start(pos)).is_err() {
            break;
        }
        let mut hdr = [0u8; 8];
        if file.read_exact(&mut hdr).is_err() {
            break;
        }
        let mut size = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as u64;
        let kind = &hdr[4..8];
        let header_len = if size == 1 {
            let mut ext_sz = [0u8; 8];
            if file.read_exact(&mut ext_sz).is_err() {
                break;
            }
            size = u64::from_be_bytes(ext_sz);
            16u64
        } else if size == 0 {
            file_size - pos
        } else {
            8u64
        };
        if size < header_len || pos + size > file_size {
            break;
        }
        if kind == b"moov" {
            let payload_len = size - header_len;
            if payload_len <= MAX_METADATA_READ_BYTES as u64 {
                let mut moov = vec![0u8; payload_len as usize];
                if file.read_exact(&mut moov).is_ok() {
                    parse_mp4_moov(&moov, out);
                }
            }
            break;
        }
        pos += size;
    }
}

fn parse_mp4_moov(data: &[u8], out: &mut ParsedTags) {
    walk_mp4_boxes(data, &mut |kind, payload| match kind {
        b"mvhd" => parse_mp4_mvhd(payload, out),
        b"trak" | b"mdia" | b"minf" | b"stbl" | b"udta" => parse_mp4_moov(payload, out),
        b"meta" => {
            if payload.len() > 4 {
                parse_mp4_moov(&payload[4..], out);
            }
        }
        b"ilst" => parse_mp4_ilst(payload, out),
        b"stsd" => parse_mp4_stsd(payload, out),
        _ => {}
    });
}

fn walk_mp4_boxes(data: &[u8], f: &mut dyn FnMut(&[u8; 4], &[u8])) {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        if size < 8 || pos + size > data.len() {
            break;
        }
        let kind: &[u8; 4] = data[pos + 4..pos + 8].try_into().unwrap();
        f(kind, &data[pos + 8..pos + size]);
        pos += size;
    }
}

fn parse_mp4_mvhd(payload: &[u8], out: &mut ParsedTags) {
    if payload.len() < 20 {
        return;
    }
    let version = payload[0];
    let (timescale, duration) = if version == 1 && payload.len() >= 28 {
        let ts = u32::from_be_bytes([payload[20], payload[21], payload[22], payload[23]]);
        let dur = u64::from_be_bytes([
            payload[24],
            payload[25],
            payload[26],
            payload[27],
            payload[28],
            payload[29],
            payload[30],
            payload[31],
        ]);
        (ts, dur)
    } else {
        let ts = u32::from_be_bytes([payload[12], payload[13], payload[14], payload[15]]);
        let dur = u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]) as u64;
        (ts, dur)
    };
    if timescale > 0 && duration > 0 {
        out.duration_secs_f64 = Some(duration as f64 / timescale as f64);
    }
}

fn parse_mp4_stsd(payload: &[u8], out: &mut ParsedTags) {
    if payload.len() < 8 {
        return;
    }
    walk_mp4_boxes(&payload[8..], &mut |kind, entry| {
        if kind == b"alac" {
            out.codec = Some("ALAC".to_string());
            out.is_lossless = true;
            if entry.len() >= 28 {
                let channels = u16::from_be_bytes([entry[16], entry[17]]) as u8;
                let bits = u16::from_be_bytes([entry[18], entry[19]]);
                let sr = u16::from_be_bytes([entry[24], entry[25]]) as u32;
                if channels > 0 {
                    out.channels = Some(channels);
                }
                if bits > 0 {
                    out.bits_per_sample = Some(bits);
                }
                if sr > 0 {
                    out.sample_rate = Some(sr);
                }
            }
        } else if kind == b"mp4a" {
            if out.codec.as_deref() != Some("ALAC") {
                out.codec = Some("AAC".to_string());
            }
            if entry.len() >= 28 {
                let channels = u16::from_be_bytes([entry[16], entry[17]]) as u8;
                let sr = u16::from_be_bytes([entry[24], entry[25]]) as u32;
                if channels > 0 {
                    out.channels = Some(channels);
                }
                if sr > 0 {
                    out.sample_rate = Some(sr);
                }
            }
        }
    });
}

fn parse_mp4_ilst(payload: &[u8], out: &mut ParsedTags) {
    walk_mp4_boxes(payload, &mut |kind, item| {
        walk_mp4_boxes(item, &mut |sub_kind, data_box| {
            if sub_kind != b"data" || data_box.len() <= 8 {
                return;
            }
            let type_ind = u32::from_be_bytes([data_box[0], data_box[1], data_box[2], data_box[3]]);
            let val_bytes = &data_box[8..];
            if kind == b"trkn" && val_bytes.len() >= 4 {
                let trk = u16::from_be_bytes([val_bytes[2], val_bytes[3]]) as u32;
                if trk > 0 {
                    out.track_number = Some(trk);
                }
                return;
            }
            if kind == b"disk" && val_bytes.len() >= 4 {
                let disc = u16::from_be_bytes([val_bytes[2], val_bytes[3]]) as u32;
                if disc > 0 {
                    out.disc_number = Some(disc);
                }
                return;
            }
            if type_ind != 1 {
                return;
            }
            let text = String::from_utf8_lossy(val_bytes).trim().to_string();
            if text.is_empty() {
                return;
            }
            match kind {
                b"\xa9nam" => {
                    out.title.get_or_insert(text);
                }
                b"\xa9ART" => {
                    out.artist.get_or_insert(text);
                }
                b"aART" => {
                    out.album_artist.get_or_insert(text);
                }
                b"\xa9alb" => {
                    out.album.get_or_insert(text);
                }
                b"\xa9day" if out.year.is_none() => {
                    let y = text.chars().take(4).collect::<String>();
                    if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) {
                        out.year = Some(y);
                    }
                }
                _ => {}
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "sonata_local_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn build_synthetic_flac(
        sample_rate: u32,
        channels: u8,
        bits_per_sample: u16,
        total_samples: u64,
        tags: &[(&str, &str)],
        padding_audio_bytes: usize,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"fLaC");

        // STREAMINFO block (type 0, 34 bytes, not last)
        out.push(0x00);
        out.extend_from_slice(&[0x00, 0x00, 34]);
        let mut si = [0u8; 34];
        si[10] = ((sample_rate >> 12) & 0xFF) as u8;
        si[11] = ((sample_rate >> 4) & 0xFF) as u8;
        let ch_bits = (channels.saturating_sub(1)) & 0x07;
        let bps_val = bits_per_sample.saturating_sub(1) & 0x1F;
        si[12] = (((sample_rate & 0x0F) as u8) << 4) | (ch_bits << 1) | ((bps_val >> 4) as u8 & 1);
        si[13] = (((bps_val & 0x0F) as u8) << 4) | (((total_samples >> 32) & 0x0F) as u8);
        let low32 = (total_samples as u32).to_be_bytes();
        si[14..18].copy_from_slice(&low32);
        out.extend_from_slice(&si);

        // VORBIS_COMMENT block (type 4, last = true)
        let mut vc = Vec::new();
        let vendor = b"SonataTest";
        vc.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        vc.extend_from_slice(vendor);
        vc.extend_from_slice(&(tags.len() as u32).to_le_bytes());
        for (k, v) in tags {
            let s = format!("{k}={v}");
            vc.extend_from_slice(&(s.len() as u32).to_le_bytes());
            vc.extend_from_slice(s.as_bytes());
        }
        let vc_len = vc.len() as u32;
        out.push(0x84);
        out.push(((vc_len >> 16) & 0xFF) as u8);
        out.push(((vc_len >> 8) & 0xFF) as u8);
        out.push((vc_len & 0xFF) as u8);
        out.extend_from_slice(&vc);

        out.resize(out.len() + padding_audio_bytes, 0xAA);
        out
    }

    fn build_id3v2_mp3(title: &str, artist: &str, album: &str) -> Vec<u8> {
        let mut frames = Vec::new();
        for (id, text) in [
            ("TIT2", title),
            ("TPE1", artist),
            ("TALB", album),
            ("TLEN", "120000"),
        ] {
            let mut payload = vec![0x03]; // UTF-8
            payload.extend_from_slice(text.as_bytes());
            frames.extend_from_slice(id.as_bytes());
            frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            frames.extend_from_slice(&[0x00, 0x00]);
            frames.extend_from_slice(&payload);
        }
        let sz = frames.len() as u32;
        let synchsafe = [
            ((sz >> 21) & 0x7F) as u8,
            ((sz >> 14) & 0x7F) as u8,
            ((sz >> 7) & 0x7F) as u8,
            (sz & 0x7F) as u8,
        ];
        let mut out = Vec::new();
        out.extend_from_slice(b"ID3");
        out.extend_from_slice(&[3, 0, 0]);
        out.extend_from_slice(&synchsafe);
        out.extend_from_slice(&frames);
        // Append valid MPEG-1 Layer III 320kbps 44.1kHz stereo frame header
        out.extend_from_slice(&[0xFF, 0xFB, 0xE0, 0x00]);
        out.resize(out.len() + 2048, 0);
        out
    }

    #[test]
    fn parses_flac_streaminfo_vorbis_comment_and_computes_bitrate() {
        let root = unique_temp_dir("flac");
        let album_dir = root.join("M83").join("Hurry Up");
        fs::create_dir_all(&album_dir).expect("mkdir");
        fs::write(album_dir.join("cover.jpg"), b"jpeg").expect("cover");

        // 10 seconds at 44100 Hz, 16-bit stereo, ~1,100,000 bytes -> ~880 kbps
        let flac_bytes = build_synthetic_flac(
            44_100,
            2,
            16,
            441_000,
            &[
                ("TITLE", "Midnight City"),
                ("ARTIST", "M83"),
                ("ALBUM", "Hurry Up, We're Dreaming"),
                ("TRACKNUMBER", "02/10"),
                ("DATE", "2011-10-18"),
            ],
            1_100_000,
        );
        let file_path = album_dir.join("02 - Midnight City.flac");
        fs::write(&file_path, &flac_bytes).expect("write flac");

        let meta = parse_audio_file(&file_path).expect("parse flac");
        assert_eq!(meta.title, "Midnight City");
        assert_eq!(meta.artist, "M83");
        assert_eq!(meta.album, "Hurry Up, We're Dreaming");
        assert_eq!(meta.track_number, Some(2));
        assert_eq!(meta.year.as_deref(), Some("2011"));
        assert_eq!(meta.duration_secs, Some(10));
        assert_eq!(meta.sample_rate, Some(44_100));
        assert_eq!(meta.bits_per_sample, Some(16));
        assert_eq!(meta.channels, Some(2));
        assert_eq!(meta.codec, "FLAC");
        assert!(meta.is_lossless);
        let kbps = meta.bitrate_kbps.expect("bitrate computed");
        assert!((875..=890).contains(&kbps), "unexpected kbps: {kbps}");
        assert_eq!(
            meta.cover_path.as_deref(),
            Some(album_dir.join("cover.jpg").as_path())
        );

        let track = metadata_to_track(&meta);
        assert!(track.is_local());
        assert_eq!(track.source_tag(), "LOC");
        assert_eq!(
            decode_local_track_id(&track.id).as_deref(),
            Some(file_path.as_path())
        );
        assert!(track
            .audio_badge
            .as_deref()
            .is_some_and(|b| b.contains("FLAC") && b.contains("16-bit/44.1kHz")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_id3v2_and_id3v1_mp3_tags_and_groups_directory() {
        let root = unique_temp_dir("mp3_scan");
        let dir = root.join("Artist").join("Album");
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("Front.PNG"), b"png").expect("cover");

        let mp3_path = dir.join("01 - Track One.mp3");
        fs::write(
            &mp3_path,
            build_id3v2_mp3("Track One", "Synth Artist", "Neon Album"),
        )
        .expect("write mp3");

        // Also write a second file with only ID3v1 tags
        let v1_path = dir.join("02 - Fallback.mp3");
        let mut v1_file = File::create(&v1_path).expect("create v1");
        v1_file.write_all(&[0u8; 512]).expect("pad");
        let mut tag = [0u8; 128];
        tag[0..3].copy_from_slice(b"TAG");
        tag[3..11].copy_from_slice(b"V1 Title");
        tag[33..42].copy_from_slice(b"V1 Artist");
        tag[63..71].copy_from_slice(b"V1 Album");
        tag[93..97].copy_from_slice(b"1999");
        v1_file.write_all(&tag).expect("write id3v1");

        let scan = scan_directory(&root).expect("scan dir");
        assert_eq!(scan.tracks.len(), 2);
        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.albums.len(), 2);

        let t1 = scan
            .tracks
            .iter()
            .find(|t| t.title == "Track One")
            .expect("t1");
        assert_eq!(t1.artist, "Synth Artist");
        assert_eq!(t1.album, "Neon Album");
        assert_eq!(t1.duration_secs, Some(120));
        assert!(t1
            .audio_badge
            .as_deref()
            .is_some_and(|b| b.contains("MP3") && b.contains("320 kbps")));

        let t2 = scan
            .tracks
            .iter()
            .find(|t| t.title == "V1 Title")
            .expect("t2");
        assert_eq!(t2.artist, "V1 Artist");
        assert_eq!(t2.album, "V1 Album");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_synthetic_opus_ogg_headers_and_tags() {
        let root = unique_temp_dir("opus");
        let opus_path = root.join("sample.opus");

        let mut pkt0 = Vec::new();
        pkt0.extend_from_slice(b"OpusHead");
        pkt0.push(1); // version
        pkt0.push(2); // channels
        pkt0.extend_from_slice(&312u16.to_le_bytes()); // pre_skip
        pkt0.extend_from_slice(&48_000u32.to_le_bytes());
        pkt0.extend_from_slice(&0u16.to_le_bytes());
        pkt0.push(0);

        let mut pkt1 = Vec::new();
        pkt1.extend_from_slice(b"OpusTags");
        pkt1.extend_from_slice(&6u32.to_le_bytes());
        pkt1.extend_from_slice(b"Sonata");
        pkt1.extend_from_slice(&2u32.to_le_bytes());
        for tag in ["TITLE=Opus Sky", "ARTIST=Test Band"] {
            pkt1.extend_from_slice(&(tag.len() as u32).to_le_bytes());
            pkt1.extend_from_slice(tag.as_bytes());
        }

        let make_page = |granule: u64, seq: u32, payload: &[u8]| {
            let mut p = Vec::new();
            p.extend_from_slice(b"OggS");
            p.push(0); // version
            p.push(0); // type
            p.extend_from_slice(&granule.to_le_bytes());
            p.extend_from_slice(&1u32.to_le_bytes());
            p.extend_from_slice(&seq.to_le_bytes());
            p.extend_from_slice(&0u32.to_le_bytes());
            p.push(1);
            p.push(payload.len() as u8);
            p.extend_from_slice(payload);
            p
        };

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&make_page(0, 0, &pkt0));
        bytes.extend_from_slice(&make_page(0, 1, &pkt1));
        bytes.extend_from_slice(&make_page(48_000 * 5 + 312, 2, &[0xAA; 100]));
        fs::write(&opus_path, &bytes).expect("write opus");

        let meta = parse_audio_file(&opus_path).expect("parse opus");
        assert_eq!(meta.codec, "OPUS");
        assert_eq!(meta.title, "Opus Sky");
        assert_eq!(meta.artist, "Test Band");
        assert_eq!(meta.channels, Some(2));
        assert_eq!(meta.sample_rate, Some(48_000));
        assert_eq!(meta.duration_secs, Some(5));

        let _ = fs::remove_dir_all(&root);
    }
}
