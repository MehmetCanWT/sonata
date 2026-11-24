//! Portable SMB share reachability probing, mount discovery, and local-mount audio browsing.
//!
//! 1. **Target Parsing & TCP Probe**: Parses `smb://host[:port]/share`, UNC `\\host\share`, or
//!    plain `host[:port]` URLs and verifies reachability via async TCP connect on port 445 (or custom port).
//! 2. **Mount Discovery**: Resolves explicit `local_mount_path` or probes standard GVFS/OS mount
//!    directories purely via filesystem inspection (zero subprocesses or `cfg!(target_os)`).
//! 3. **Catalog & Stream Mapping**: Scans mounted SMB directories using `crate::providers::local`
//!    parsers and maps tracks, M3U/folder playlists, albums, artists, and direct file streams.

use super::{
    encode_server_item_id, format_audio_badge, ServerKind, ServerLibrary, ServerProfile,
    ServerSearchResult, ServerStream,
};
use crate::error::CoreError;
use crate::providers::local::{
    compute_bitrate_kbps, find_folder_cover, format_local_audio_badge, is_supported_audio_path,
    parse_audio_file, LocalAudioMetadata,
};
use crate::providers::ytmusic::{AlbumDetail, HomeCard, HomeCardKind};
use crate::providers::{Playlist, ProviderSource, Track};
use reqwest::Url;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

const DEFAULT_SMB_PORT: u16 = 445;
const DEFAULT_TCP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_SMB_SCAN_DEPTH: usize = 12;

/// Parsed SMB host, TCP port, share name, and optional subpath from a [`ServerProfile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmbTarget {
    pub host: Option<String>,
    pub port: u16,
    pub share: Option<String>,
    pub subpath: Option<String>,
    pub local_path_hint: Option<PathBuf>,
}

/// Parses an SMB URL, UNC path, or hostname from `profile` into [`SmbTarget`].
pub fn parse_smb_target(profile: &ServerProfile) -> Result<SmbTarget, CoreError> {
    let raw = profile.normalized_base_url();
    let explicit_share = profile
        .share
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_matches('/').to_string());

    if raw.is_empty() {
        if profile.local_mount_path.is_some() {
            return Ok(SmbTarget {
                host: None,
                port: DEFAULT_SMB_PORT,
                share: explicit_share,
                subpath: None,
                local_path_hint: profile.local_mount_path.clone(),
            });
        }
        return Err(CoreError::invalid_response(
            "smb",
            "SMB host URL or local_mount_path is required",
        ));
    }

    if raw.starts_with("file://") {
        let plain = raw.strip_prefix("file://").unwrap_or(&raw);
        return Ok(SmbTarget {
            host: None,
            port: DEFAULT_SMB_PORT,
            share: explicit_share,
            subpath: None,
            local_path_hint: Some(PathBuf::from(plain)),
        });
    }

    if raw.starts_with('/') && !raw.starts_with("//") {
        return Ok(SmbTarget {
            host: None,
            port: DEFAULT_SMB_PORT,
            share: explicit_share,
            subpath: None,
            local_path_hint: Some(PathBuf::from(&raw)),
        });
    }

    // Handle UNC paths: \\host\share\subpath or //host/share/subpath
    if raw.starts_with("\\\\") || raw.starts_with("//") {
        let stripped = raw.trim_start_matches(['\\', '/']);
        let parts: Vec<&str> = stripped
            .split(['\\', '/'])
            .filter(|s| !s.is_empty())
            .collect();
        let host = parts
            .first()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let (host, port) = split_host_port_opt(host);
        let share = parts.get(1).map(|s| s.to_string()).or(explicit_share);
        let subpath = if parts.len() > 2 {
            Some(parts[2..].join("/"))
        } else {
            None
        };
        return Ok(SmbTarget {
            host,
            port,
            share,
            subpath,
            local_path_hint: profile.local_mount_path.clone(),
        });
    }

    let normalized_url = if raw.contains("://") {
        raw.clone()
    } else {
        format!("smb://{raw}")
    };
    let parsed = Url::parse(&normalized_url).map_err(|err| {
        CoreError::invalid_response("smb", format!("invalid SMB target URL: {err}"))
    })?;
    let host = parsed
        .host_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let port = parsed.port().unwrap_or(DEFAULT_SMB_PORT);
    let mut segments = parsed
        .path_segments()
        .map(|it| {
            it.filter(|s| !s.is_empty())
                .map(|s| {
                    urlencoding::decode(s)
                        .map(|d| d.into_owned())
                        .unwrap_or_else(|_| s.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let share = if !segments.is_empty() {
        Some(segments.remove(0))
    } else {
        explicit_share
    };
    let subpath = (!segments.is_empty()).then(|| segments.join("/"));

    if host.is_none() && profile.local_mount_path.is_none() {
        return Err(CoreError::invalid_response(
            "smb",
            "SMB URL is missing a hostname",
        ));
    }

    Ok(SmbTarget {
        host,
        port,
        share,
        subpath,
        local_path_hint: profile.local_mount_path.clone(),
    })
}

fn split_host_port_opt(raw: Option<String>) -> (Option<String>, u16) {
    let Some(h) = raw else {
        return (None, DEFAULT_SMB_PORT);
    };
    if let Some((host, port_str)) = h.rsplit_once(':') {
        if !host.is_empty() && !host.contains(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                return (Some(host.to_string()), port);
            }
        }
    }
    (Some(h), DEFAULT_SMB_PORT)
}

/// Checks TCP connectivity to an SMB host/port and returns measured connect latency in milliseconds.
pub async fn probe_tcp(host: &str, port: u16, timeout: Duration) -> Result<u64, CoreError> {
    let start = Instant::now();
    let connect = TcpStream::connect((host, port));
    match tokio::time::timeout(timeout, connect).await {
        Ok(Ok(_stream)) => Ok(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)),
        Ok(Err(err)) => Err(CoreError::offline(format!(
            "SMB TCP connect to {host}:{port} failed: {err}"
        ))),
        Err(_) => Err(CoreError::timeout(format!(
            "SMB TCP connect to {host}:{port} timed out after {}ms",
            timeout.as_millis()
        ))),
    }
}

/// Verifies SMB reachability (TCP port 445 or custom port) and reports mount availability.
pub async fn ping(profile: &ServerProfile) -> Result<String, CoreError> {
    let target = parse_smb_target(profile)?;
    let mounted = resolve_mount_path(profile);
    if let Some(host) = target.host.as_deref() {
        let latency_ms = probe_tcp(host, target.port, DEFAULT_TCP_TIMEOUT).await?;
        let mount_note = if mounted.is_some() {
            "mounted"
        } else {
            "tcp-only"
        };
        return Ok(format!(
            "SMB OK ({host}:{}, {latency_ms}ms, {mount_note})",
            target.port
        ));
    }
    if let Some(path) = mounted {
        return Ok(format!("SMB OK (mounted at {})", path.display()));
    }
    Err(CoreError::offline(
        "SMB share has neither a reachable host nor an existing local mount path",
    ))
}

/// Resolves an accessible local directory path backing the SMB share, if mounted.
pub fn resolve_mount_path(profile: &ServerProfile) -> Option<PathBuf> {
    if let Some(ref explicit) = profile.local_mount_path {
        if explicit.is_dir() {
            return Some(explicit.clone());
        }
    }

    let target = parse_smb_target(profile).ok()?;
    if let Some(ref hint) = target.local_path_hint {
        if hint.is_dir() {
            return Some(hint.clone());
        }
    }

    let (host, share) = (target.host.as_deref()?, target.share.as_deref()?);
    let mut candidates = Vec::new();

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if let Some(gvfs_match) =
            discover_gvfs_smb_mount_in(&PathBuf::from(runtime_dir).join("gvfs"), host, share)
        {
            candidates.push(gvfs_match);
        }
    }
    candidates.push(PathBuf::from("/mnt").join(share));
    candidates.push(PathBuf::from("/mnt").join(host).join(share));
    candidates.push(PathBuf::from("/Volumes").join(share));
    if let Ok(user) = std::env::var("USER") {
        if !user.trim().is_empty() {
            candidates.push(PathBuf::from("/media").join(&user).join(share));
            candidates.push(PathBuf::from("/run/media").join(&user).join(share));
        }
    }

    for base in candidates {
        let candidate = match target.subpath.as_deref() {
            Some(sub) => join_safe_relative(&base, sub).unwrap_or(base),
            None => base,
        };
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// Discovers a matching GVFS `smb-share:server=...,share=...` directory inside `gvfs_root`.
pub fn discover_gvfs_smb_mount_in(gvfs_root: &Path, host: &str, share: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(gvfs_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(spec) = name.strip_prefix("smb-share:") else {
            continue;
        };
        let mut found_server = None;
        let mut found_share = None;
        for part in spec.split(',') {
            if let Some((k, v)) = part.split_once('=') {
                if k.eq_ignore_ascii_case("server") {
                    found_server = Some(v);
                } else if k.eq_ignore_ascii_case("share") {
                    found_share = Some(v);
                }
            }
        }
        if found_server.is_some_and(|s| s.eq_ignore_ascii_case(host))
            && found_share.is_some_and(|s| s.eq_ignore_ascii_case(share))
        {
            return Some(path);
        }
    }
    None
}

fn require_mount_root(profile: &ServerProfile) -> Result<PathBuf, CoreError> {
    resolve_mount_path(profile).ok_or_else(|| {
        CoreError::offline(
            "SMB share is not mounted locally; set local_mount_path or mount the smb:// share",
        )
    })
}

fn join_safe_relative(root: &Path, rel: &str) -> Result<PathBuf, CoreError> {
    let trimmed = rel.trim().trim_start_matches('/');
    let rel_path = Path::new(trimmed);
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(CoreError::invalid_response(
                    "smb",
                    "invalid relative SMB path traversal",
                ));
            }
        }
    }
    Ok(if trimmed.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel_path)
    })
}

fn collect_mount_audio(
    root: &Path,
    subpath: Option<&str>,
    max_items: Option<usize>,
) -> Result<Vec<(String, LocalAudioMetadata)>, CoreError> {
    let scan_root = match subpath {
        Some(sub) if !sub.trim().is_empty() => join_safe_relative(root, sub)?,
        _ => root.to_path_buf(),
    };
    if !scan_root.is_dir() {
        return Err(CoreError::not_found(format!(
            "SMB directory not found: {}",
            scan_root.display()
        )));
    }
    let mut visited = HashSet::new();
    let mut files = Vec::new();
    walk_smb_audio(root, &scan_root, 0, &mut visited, &mut files, max_items);
    let mut items = Vec::with_capacity(files.len());
    for (rel, full) in files {
        if let Some(meta) = parse_audio_file(&full) {
            items.push((rel, meta));
        }
    }
    Ok(items)
}

fn walk_smb_audio(
    mount_root: &Path,
    dir: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<(String, PathBuf)>,
    max_items: Option<usize>,
) {
    if depth > MAX_SMB_SCAN_DEPTH {
        return;
    }
    if max_items.is_some_and(|lim| out.len() >= lim) {
        return;
    }
    let canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited.insert(canon) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    let mut audio = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.is_file() && is_supported_audio_path(&p) {
            audio.push(p);
        }
    }
    audio.sort();
    subdirs.sort();
    for p in audio {
        let rel = p
            .strip_prefix(mount_root)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((rel, p));
        if max_items.is_some_and(|lim| out.len() >= lim) {
            return;
        }
    }
    for sub in subdirs {
        walk_smb_audio(mount_root, &sub, depth + 1, visited, out, max_items);
        if max_items.is_some_and(|lim| out.len() >= lim) {
            return;
        }
    }
}

fn meta_to_smb_track(profile: &ServerProfile, rel_path: &str, meta: &LocalAudioMetadata) -> Track {
    let badge = if meta.bitrate_kbps.is_some() || meta.bits_per_sample.is_some() {
        format!(
            "{} · SMB",
            format_local_audio_badge(
                &meta.codec,
                meta.bitrate_kbps,
                meta.bits_per_sample,
                meta.sample_rate
            )
        )
    } else {
        format_audio_badge(Some(&meta.codec), meta.bitrate_kbps, "SMB")
            .unwrap_or_else(|| "SMB".to_string())
    };
    Track {
        id: encode_server_item_id(ServerKind::Smb, &profile.id, rel_path),
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

/// Lists top-level directories inside the mounted SMB share as [`ServerLibrary`] entries.
pub fn list_libraries(profile: &ServerProfile) -> Result<Vec<ServerLibrary>, CoreError> {
    let root = require_mount_root(profile)?;
    let entries = fs::read_dir(&root).map_err(|err| {
        CoreError::io(format!(
            "failed to read SMB share {}: {err}",
            root.display()
        ))
    })?;
    let mut libs = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        libs.push(ServerLibrary {
            id: encode_server_item_id(ServerKind::Smb, &profile.id, name),
            name: name.to_string(),
            kind: "folder".to_string(),
        });
    }
    libs.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(libs)
}

/// Groups mounted SMB audio tracks into artist [`HomeCard`] items.
pub fn list_artists(
    profile: &ServerProfile,
    library_id: Option<&str>,
) -> Result<Vec<HomeCard>, CoreError> {
    let root = require_mount_root(profile)?;
    let items = collect_mount_audio(&root, library_id, None)?;
    let mut by_artist: BTreeMap<String, (String, Option<String>, usize)> = BTreeMap::new();
    for (_, meta) in items {
        let name = meta
            .album_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&meta.artist)
            .to_string();
        let key = name.to_ascii_lowercase();
        let cover = meta
            .cover_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        let entry = by_artist.entry(key).or_insert((name, cover, 0));
        entry.2 += 1;
    }
    Ok(by_artist
        .into_values()
        .map(|(name, cover, count)| HomeCard {
            kind: HomeCardKind::Artist,
            id: encode_server_item_id(ServerKind::Smb, &profile.id, &format!("artist:{name}")),
            title: name,
            subtitle: format!("{count} tracks · SMB"),
            thumbnail_url: cover,
            track: None,
        })
        .collect())
}

/// Groups mounted SMB audio tracks into album [`HomeCard`] items.
pub fn list_albums(
    profile: &ServerProfile,
    library_id: Option<&str>,
    artist_id: Option<&str>,
) -> Result<Vec<HomeCard>, CoreError> {
    let root = require_mount_root(profile)?;
    let items = collect_mount_audio(&root, library_id, None)?;
    let artist_filter = artist_id
        .map(|s| s.strip_prefix("artist:").unwrap_or(s))
        .map(str::to_ascii_lowercase);

    struct AlbumGroup {
        album: String,
        artist: String,
        cover: Option<String>,
        first_track: Option<Track>,
    }

    let mut albums: BTreeMap<(String, String), AlbumGroup> = BTreeMap::new();
    for (rel, meta) in items {
        let artist = meta
            .album_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&meta.artist)
            .to_string();
        if let Some(ref wanted) = artist_filter {
            if artist.to_ascii_lowercase() != *wanted {
                continue;
            }
        }
        let album = if meta.album.trim().is_empty() {
            "Singles".to_string()
        } else {
            meta.album.clone()
        };
        let track = meta_to_smb_track(profile, &rel, &meta);
        let key = (album.to_ascii_lowercase(), artist.to_ascii_lowercase());
        albums.entry(key).or_insert(AlbumGroup {
            album,
            artist,
            cover: track.thumbnail_url.clone(),
            first_track: Some(track),
        });
    }

    Ok(albums
        .into_values()
        .map(|g| HomeCard {
            kind: HomeCardKind::Album,
            id: encode_server_item_id(
                ServerKind::Smb,
                &profile.id,
                &format!("album:{}::{}", g.artist, g.album),
            ),
            title: g.album,
            subtitle: g.artist,
            thumbnail_url: g.cover,
            track: g.first_track,
        })
        .collect())
}

/// Returns [`AlbumDetail`] for a grouped SMB album or subfolder path.
pub fn get_album_detail(profile: &ServerProfile, raw_id: &str) -> Result<AlbumDetail, CoreError> {
    let root = require_mount_root(profile)?;
    if let Some(spec) = raw_id.strip_prefix("album:") {
        let (target_artist, target_album) = spec.split_once("::").unwrap_or(("", spec));
        let items = collect_mount_audio(&root, None, None)?;
        let mut tracks = Vec::new();
        let mut cover_url = None;
        let mut year = None;
        for (rel, meta) in items {
            let artist = meta
                .album_artist
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&meta.artist);
            let album = if meta.album.trim().is_empty() {
                "Singles"
            } else {
                meta.album.as_str()
            };
            if album.eq_ignore_ascii_case(target_album)
                && (target_artist.is_empty() || artist.eq_ignore_ascii_case(target_artist))
            {
                if cover_url.is_none() {
                    cover_url = meta
                        .cover_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned());
                }
                if year.is_none() {
                    year = meta.year.clone();
                }
                tracks.push(meta_to_smb_track(profile, &rel, &meta));
            }
        }
        if tracks.is_empty() {
            return Err(CoreError::not_found(format!(
                "SMB album `{target_album}` not found"
            )));
        }
        return Ok(AlbumDetail {
            id: encode_server_item_id(ServerKind::Smb, &profile.id, raw_id),
            title: target_album.to_string(),
            subtitle: target_artist.to_string(),
            release_type: "Album".to_string(),
            year,
            duration_text: None,
            cover_url,
            artists: vec![(target_artist.to_string(), None)],
            tracks,
            other_versions: Vec::new(),
            playlist_id: None,
            saved: false,
        });
    }

    let items = collect_mount_audio(&root, Some(raw_id), None)?;
    if items.is_empty() {
        return Err(CoreError::not_found(format!(
            "SMB folder `{raw_id}` has no audio files"
        )));
    }
    let tracks: Vec<Track> = items
        .iter()
        .map(|(rel, meta)| meta_to_smb_track(profile, rel, meta))
        .collect();
    let cover_url = join_safe_relative(&root, raw_id)
        .ok()
        .and_then(|d| find_folder_cover(&d))
        .map(|p| p.to_string_lossy().into_owned());
    Ok(AlbumDetail {
        id: encode_server_item_id(ServerKind::Smb, &profile.id, raw_id),
        title: Path::new(raw_id)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(raw_id)
            .to_string(),
        subtitle: profile.name.clone(),
        release_type: "Folder".to_string(),
        year: None,
        duration_text: None,
        cover_url,
        artists: Vec::new(),
        tracks,
        other_versions: Vec::new(),
        playlist_id: None,
        saved: false,
    })
}

/// Discovers `.m3u`/`.m3u8` playlists and top-level music folders on the mounted SMB share.
pub fn list_playlists(profile: &ServerProfile) -> Result<Vec<Playlist>, CoreError> {
    let root = require_mount_root(profile)?;
    let mut visited = HashSet::new();
    let mut playlists = Vec::new();
    collect_m3u_playlists(&root, &root, 0, &mut visited, profile, &mut playlists);

    if playlists.is_empty() {
        for lib in list_libraries(profile)? {
            playlists.push(Playlist {
                id: lib.id,
                name: lib.name,
                source: ProviderSource::YouTubeMusic,
                track_count: None,
                thumbnail_url: None,
                is_owned: false,
            });
        }
    }
    Ok(playlists)
}

fn collect_m3u_playlists(
    mount_root: &Path,
    dir: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    profile: &ServerProfile,
    out: &mut Vec<Playlist>,
) {
    if depth > 4 {
        return;
    }
    let canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited.insert(canon) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_m3u_playlists(mount_root, &p, depth + 1, visited, profile, out);
        } else if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
        {
            let rel = p
                .strip_prefix(mount_root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Playlist")
                .to_string();
            out.push(Playlist {
                id: encode_server_item_id(ServerKind::Smb, &profile.id, &rel),
                name,
                source: ProviderSource::YouTubeMusic,
                track_count: None,
                thumbnail_url: None,
                is_owned: false,
            });
        }
    }
}

/// Loads tracks from an `.m3u`/`.m3u8` file or folder inside the mounted SMB share.
pub fn get_playlist_tracks(profile: &ServerProfile, raw_id: &str) -> Result<Vec<Track>, CoreError> {
    let root = require_mount_root(profile)?;
    let target = join_safe_relative(&root, raw_id)?;
    if target.is_dir() {
        let items = collect_mount_audio(&root, Some(raw_id), None)?;
        return Ok(items
            .iter()
            .map(|(rel, meta)| meta_to_smb_track(profile, rel, meta))
            .collect());
    }
    if target.is_file() {
        let raw = fs::read_to_string(&target).map_err(|err| {
            CoreError::io(format!(
                "failed to read SMB playlist {}: {err}",
                target.display()
            ))
        })?;
        let base_dir = target.parent().unwrap_or(&root);
        let mut tracks = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let normalized = trimmed.replace('\\', "/");
            if let Ok(resolved) = join_safe_relative(base_dir, &normalized) {
                if resolved.is_file() && is_supported_audio_path(&resolved) {
                    let rel = resolved
                        .strip_prefix(&root)
                        .unwrap_or(&resolved)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if let Some(meta) = parse_audio_file(&resolved) {
                        tracks.push(meta_to_smb_track(profile, &rel, &meta));
                    }
                }
            }
        }
        return Ok(tracks);
    }
    Err(CoreError::not_found(format!(
        "SMB playlist `{raw_id}` not found"
    )))
}

/// Searches mounted SMB audio tracks, albums, and artists by substring query.
pub fn search(
    profile: &ServerProfile,
    query: &str,
    limit: usize,
) -> Result<ServerSearchResult, CoreError> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() || limit == 0 {
        return Ok(ServerSearchResult::default());
    }
    let root = require_mount_root(profile)?;
    let items = collect_mount_audio(&root, None, None)?;
    let tracks = items
        .iter()
        .filter(|(rel, meta)| {
            meta.title.to_ascii_lowercase().contains(&needle)
                || meta.artist.to_ascii_lowercase().contains(&needle)
                || meta.album.to_ascii_lowercase().contains(&needle)
                || rel.to_ascii_lowercase().contains(&needle)
        })
        .take(limit)
        .map(|(rel, meta)| meta_to_smb_track(profile, rel, meta))
        .collect();
    Ok(ServerSearchResult {
        tracks,
        ..Default::default()
    })
}

/// Resolves a mounted SMB track item into a playable local-file [`ServerStream`].
pub fn stream_request(
    profile: &ServerProfile,
    raw_id: &str,
    track: &Track,
) -> Result<ServerStream, CoreError> {
    let root = require_mount_root(profile)?;
    let plain = raw_id.strip_prefix("file://").unwrap_or(raw_id);
    let candidate_path = {
        let p = Path::new(plain);
        if p.is_absolute() && p.starts_with(&root) {
            p.to_path_buf()
        } else {
            join_safe_relative(&root, plain)?
        }
    };
    if !candidate_path.is_file() {
        return Err(CoreError::not_found(format!(
            "SMB file `{}` not found on mounted share",
            candidate_path.display()
        )));
    }

    let meta = parse_audio_file(&candidate_path);
    let codec = meta
        .as_ref()
        .map(|m| m.codec.to_ascii_lowercase())
        .or_else(|| {
            candidate_path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
        });
    let bitrate_kbps = meta.as_ref().and_then(|m| m.bitrate_kbps).or_else(|| {
        let bytes = fs::metadata(&candidate_path).ok()?.len();
        let dur = track.duration_secs? as f64;
        compute_bitrate_kbps(bytes, dur)
    });
    let audio_badge = meta
        .as_ref()
        .map(|m| {
            format!(
                "{} · SMB",
                format_local_audio_badge(&m.codec, bitrate_kbps, m.bits_per_sample, m.sample_rate)
            )
        })
        .or_else(|| track.audio_badge.clone());

    Ok(ServerStream {
        url: candidate_path.to_string_lossy().into_owned(),
        headers: Vec::new(),
        mime_type: codec
            .as_deref()
            .and_then(mime_for_smb_ext)
            .map(str::to_string),
        bitrate_kbps,
        codec,
        audio_badge,
    })
}

/// Reads up to `max_bytes` from the first audio file on a mounted SMB share to benchmark throughput.
pub(crate) fn read_speed_test_sample(
    profile: &ServerProfile,
    max_bytes: usize,
) -> Result<(u64, Duration, String), CoreError> {
    let root = require_mount_root(profile)?;
    let items = collect_mount_audio(&root, None, Some(1))?;
    let Some((rel, _)) = items.first() else {
        return Ok((
            0,
            Duration::ZERO,
            "SMB mount accessible; no audio files found to benchmark".to_string(),
        ));
    };
    let full = join_safe_relative(&root, rel)?;
    let mut file = File::open(&full).map_err(|err| {
        CoreError::io(format!(
            "failed to open SMB sample {}: {err}",
            full.display()
        ))
    })?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    let start = Instant::now();
    while total < max_bytes as u64 {
        let to_read = buf.len().min((max_bytes as u64 - total) as usize);
        let n = file
            .read(&mut buf[..to_read])
            .map_err(|err| CoreError::io(format!("failed reading SMB sample: {err}")))?;
        if n == 0 {
            break;
        }
        total += n as u64;
    }
    let elapsed = start.elapsed();
    Ok((
        total,
        elapsed,
        format!("Read {total} bytes from mounted SMB file"),
    ))
}

fn mime_for_smb_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "flac" => Some("audio/flac"),
        "mp3" => Some("audio/mpeg"),
        "opus" => Some("audio/opus"),
        "ogg" => Some("audio/ogg"),
        "m4a" | "aac" | "alac" => Some("audio/mp4"),
        "wav" => Some("audio/wav"),
        "aiff" => Some("audio/aiff"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use tokio::net::TcpListener;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "sonata_smb_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn parses_smb_urls_unc_paths_and_custom_ports() {
        let mut p1 = ServerProfile::new(
            "nas",
            "Home NAS",
            ServerKind::Smb,
            "smb://192.168.1.20:1445/Music/Lossless",
        );
        p1.domain = Some("WORKGROUP".to_string());
        let t1 = parse_smb_target(&p1).expect("parse smb url");
        assert_eq!(t1.host.as_deref(), Some("192.168.1.20"));
        assert_eq!(t1.port, 1445);
        assert_eq!(t1.share.as_deref(), Some("Music"));
        assert_eq!(t1.subpath.as_deref(), Some("Lossless"));

        let mut p2 = ServerProfile::new("unc", "NAS UNC", ServerKind::Smb, r"\\nas.local\Audio");
        let t2 = parse_smb_target(&p2).expect("parse unc");
        assert_eq!(t2.host.as_deref(), Some("nas.local"));
        assert_eq!(t2.port, 445);
        assert_eq!(t2.share.as_deref(), Some("Audio"));

        p2.base_url = "nas.local".to_string();
        p2.share = Some("SharedMusic".to_string());
        let t3 = parse_smb_target(&p2).expect("parse host with share field");
        assert_eq!(t3.host.as_deref(), Some("nas.local"));
        assert_eq!(t3.port, 445);
        assert_eq!(t3.share.as_deref(), Some("SharedMusic"));
    }

    #[tokio::test]
    async fn probes_tcp_reachability_against_ephemeral_listener() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        let accept_task = tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let mut profile = ServerProfile::new(
            "smb-test",
            "Loopback SMB",
            ServerKind::Smb,
            format!("smb://127.0.0.1:{port}/Music"),
        );
        let msg = ping(&profile).await.expect("smb ping succeeds");
        assert!(msg.contains("SMB OK"));
        assert!(msg.contains(&format!("127.0.0.1:{port}")));

        let _ = accept_task.await;

        // Closed port must fail cleanly.
        profile.base_url = format!("smb://127.0.0.1:{port}/Music");
        let err = ping(&profile).await.expect_err("closed port fails");
        assert!(matches!(err, CoreError::Offline { .. }));
    }

    #[test]
    fn discovers_gvfs_mount_and_browses_mounted_smb_share() {
        let root = unique_temp_dir("mount");
        let gvfs_root = root.join("gvfs");
        let share_dir = gvfs_root.join("smb-share:server=nas.local,share=Music");
        let album_dir = share_dir.join("Daft Punk").join("Discovery");
        fs::create_dir_all(&album_dir).expect("mkdir");
        fs::write(album_dir.join("cover.jpg"), b"jpg").expect("cover");
        fs::write(
            album_dir.join("01 - One More Time.flac"),
            b"fLaC_dummy_payload",
        )
        .expect("write flac");
        fs::write(
            share_dir.join("Favorites.m3u"),
            "#EXTM3U\nDaft Punk/Discovery/01 - One More Time.flac\n",
        )
        .expect("write m3u");

        let found_gvfs = discover_gvfs_smb_mount_in(&gvfs_root, "nas.local", "music");
        assert_eq!(found_gvfs.as_deref(), Some(share_dir.as_path()));

        let mut profile =
            ServerProfile::new("nas-1", "NAS", ServerKind::Smb, "smb://nas.local/Music");
        profile.local_mount_path = Some(share_dir.clone());

        let libs = list_libraries(&profile).expect("libs");
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].name, "Daft Punk");

        let albums = list_albums(&profile, None, None).expect("albums");
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "Discovery");
        assert_eq!(albums[0].subtitle, "Daft Punk");

        let playlists = list_playlists(&profile).expect("playlists");
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Favorites");

        let pl_tracks = get_playlist_tracks(&profile, "Favorites.m3u").expect("m3u tracks");
        assert_eq!(pl_tracks.len(), 1);
        assert_eq!(pl_tracks[0].source_tag(), "SMB");
        assert!(pl_tracks[0].id.starts_with("smb:nas-1:"));

        let search_res = search(&profile, "One More Time", 10).expect("search");
        assert_eq!(search_res.tracks.len(), 1);

        let stream = stream_request(
            &profile,
            "Daft Punk/Discovery/01 - One More Time.flac",
            &pl_tracks[0],
        )
        .expect("stream request");
        assert_eq!(
            stream.url,
            album_dir.join("01 - One More Time.flac").to_string_lossy()
        );
        assert_eq!(stream.mime_type.as_deref(), Some("audio/flac"));

        let _ = fs::remove_dir_all(&root);
    }
}
