//! Music-video counterpart discovery and timeline synchronization parsers.
//!
//! 1. **Counterpart Extraction**: locates OMV/UGC music-video counterparts inside InnerTube
//!    `playlistPanelVideoWrapperRenderer` and standalone panel nodes while rejecting ATV-only tracks.
//! 2. **Timeline Alignment**: parses `segmentMap` entries and builds `MusicVideoSyncSegment`
//!    mappings from SponsorBlock `music_offtopic` skip intervals.

use super::extract_runs_text;
use crate::providers::parse_duration_text;
use crate::providers::ytmusic::upscale_thumbnail;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicVideoSyncSegment {
    pub music_start_ms: u64,
    pub video_start_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicVideoCandidate {
    pub video_id: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub duration_secs: Option<u64>,
    pub sync_segments: Vec<MusicVideoSyncSegment>,
}

fn parse_ms_value(v: Option<&Value>) -> Option<u64> {
    let val = v?;
    val.as_u64()
        .or_else(|| val.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

fn parse_counterpart_segment_map(
    entry: &Value,
    wrapper: &Value,
    primary_is_video: bool,
) -> Vec<MusicVideoSyncSegment> {
    let Some(arr) = entry
        .pointer("/segmentMap/segment")
        .or_else(|| entry.pointer("/counterpartRenderer/segmentMap/segment"))
        .or_else(|| wrapper.pointer("/segmentMap/segment"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    let mut segments: Vec<MusicVideoSyncSegment> = arr
        .iter()
        .filter_map(|seg| {
            let primary_ms = parse_ms_value(seg.get("primaryVideoStartTimeMilliseconds"))?;
            let counterpart_ms = parse_ms_value(seg.get("counterpartVideoStartTimeMilliseconds"))?;
            let duration_ms = parse_ms_value(seg.get("durationMilliseconds")).unwrap_or(0);
            if duration_ms == 0 {
                return None;
            }
            let (music_start_ms, video_start_ms) = if primary_is_video {
                (counterpart_ms, primary_ms)
            } else {
                (primary_ms, counterpart_ms)
            };
            Some(MusicVideoSyncSegment {
                music_start_ms,
                video_start_ms,
                duration_ms,
            })
        })
        .collect();
    segments.sort_by_key(|s| s.video_start_ms);
    segments
}

/// Converts SponsorBlock `music_offtopic` `[start_ms, end_ms]` skip intervals into aligned
/// Music<->Video playback segments so lyrics and mode switches skip non-music intros/interludes.
pub fn build_sync_segments_from_offtopic(
    offtopic_ms: &[(u64, u64)],
    total_video_ms: u64,
) -> Vec<MusicVideoSyncSegment> {
    let mut sorted: Vec<(u64, u64)> = offtopic_ms
        .iter()
        .filter_map(|&(s, e)| {
            let clamped_e = if total_video_ms > 0 {
                e.min(total_video_ms)
            } else {
                e
            };
            (clamped_e > s).then_some((s, clamped_e))
        })
        .collect();
    if sorted.is_empty() {
        return Vec::new();
    }
    sorted.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(sorted.len());
    for (s, e) in sorted {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }

    let max_end = if total_video_ms > 0 {
        total_video_ms
    } else {
        merged.last().map_or(0, |&(_, e)| e).saturating_add(600_000)
    };

    let mut result = Vec::new();
    let mut video_cursor = 0u64;
    let mut music_cursor = 0u64;
    for (skip_start, skip_end) in merged {
        if skip_start > video_cursor {
            let dur = skip_start - video_cursor;
            result.push(MusicVideoSyncSegment {
                music_start_ms: music_cursor,
                video_start_ms: video_cursor,
                duration_ms: dur,
            });
            music_cursor = music_cursor.saturating_add(dur);
        }
        video_cursor = video_cursor.max(skip_end);
    }
    if max_end > video_cursor {
        result.push(MusicVideoSyncSegment {
            music_start_ms: music_cursor,
            video_start_ms: video_cursor,
            duration_ms: max_end - video_cursor,
        });
    }
    result
}

fn extract_panel_music_video_type(panel: &Value) -> Option<&str> {
    panel
        .pointer("/navigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        .and_then(|v| v.as_str())
}

fn is_music_video_type(mv_type: Option<&str>) -> bool {
    matches!(
        mv_type,
        Some("MUSIC_VIDEO_TYPE_OMV" | "MUSIC_VIDEO_TYPE_UGC")
    )
}

fn panel_to_music_video_candidate(panel: &Value) -> Option<MusicVideoCandidate> {
    let video_id = panel
        .get("videoId")
        .or_else(|| panel.pointer("/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())?
        .to_string();
    let title = panel
        .get("title")
        .and_then(extract_runs_text)
        .unwrap_or_default();
    let thumb = panel
        .pointer("/thumbnail/thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|v| v.as_str())
        .map(upscale_thumbnail);
    let duration_secs = panel
        .get("lengthText")
        .and_then(extract_runs_text)
        .and_then(|t| parse_duration_text(&t));
    Some(MusicVideoCandidate {
        video_id,
        title,
        thumbnail_url: thumb,
        duration_secs,
        sync_segments: Vec::new(),
    })
}

pub fn parse_music_video_counterpart_json(
    val: &Value,
    current_video_id: &str,
) -> Option<MusicVideoCandidate> {
    fn walk_wrappers(
        node: &Value,
        current_video_id: &str,
        fallback: &mut Option<MusicVideoCandidate>,
    ) -> Option<MusicVideoCandidate> {
        match node {
            Value::Object(map) => {
                if let Some(wrapper) = map.get("playlistPanelVideoWrapperRenderer") {
                    let primary = wrapper
                        .pointer("/primaryRenderer/playlistPanelVideoRenderer")
                        .filter(|v| !v.is_null());
                    let primary_matches = primary
                        .and_then(|p| {
                            p.get("videoId")
                                .or_else(|| p.pointer("/navigationEndpoint/watchEndpoint/videoId"))
                                .and_then(|v| v.as_str())
                        })
                        .is_some_and(|id| id == current_video_id);

                    let mut found: Option<MusicVideoCandidate> = None;
                    if let Some(counterparts) =
                        wrapper.get("counterpart").and_then(|v| v.as_array())
                    {
                        for entry in counterparts {
                            let Some(cp) =
                                entry.pointer("/counterpartRenderer/playlistPanelVideoRenderer")
                            else {
                                continue;
                            };
                            let mv_type = extract_panel_music_video_type(cp);
                            if is_music_video_type(mv_type)
                                || mv_type.is_some_and(|t| t != "MUSIC_VIDEO_TYPE_ATV")
                            {
                                if let Some(mut cand) = panel_to_music_video_candidate(cp) {
                                    cand.sync_segments =
                                        parse_counterpart_segment_map(entry, wrapper, false);
                                    found = Some(cand);
                                    break;
                                }
                            }
                        }
                    }
                    if found.is_none() {
                        if let Some(p) = primary {
                            if is_music_video_type(extract_panel_music_video_type(p)) {
                                if let Some(mut cand) = panel_to_music_video_candidate(p) {
                                    if let Some(first_cp) = wrapper
                                        .get("counterpart")
                                        .and_then(|v| v.as_array())
                                        .and_then(|arr| arr.first())
                                    {
                                        cand.sync_segments =
                                            parse_counterpart_segment_map(first_cp, wrapper, true);
                                    }
                                    found = Some(cand);
                                }
                            }
                        }
                    }
                    if let Some(cand) = found {
                        if primary_matches || current_video_id.is_empty() {
                            return Some(cand);
                        }
                        if fallback.is_none() {
                            *fallback = Some(cand);
                        }
                    }
                }
                for v in map.values() {
                    if let Some(hit) = walk_wrappers(v, current_video_id, fallback) {
                        return Some(hit);
                    }
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    if let Some(hit) = walk_wrappers(v, current_video_id, fallback) {
                        return Some(hit);
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn walk_standalone(node: &Value, current_video_id: &str) -> Option<MusicVideoCandidate> {
        match node {
            Value::Object(map) => {
                if let Some(panel) = map.get("playlistPanelVideoRenderer") {
                    let matches = panel
                        .get("videoId")
                        .or_else(|| panel.pointer("/navigationEndpoint/watchEndpoint/videoId"))
                        .and_then(|v| v.as_str())
                        .is_some_and(|id| id == current_video_id);
                    if matches && is_music_video_type(extract_panel_music_video_type(panel)) {
                        if let Some(cand) = panel_to_music_video_candidate(panel) {
                            return Some(cand);
                        }
                    }
                }
                for v in map.values() {
                    if let Some(hit) = walk_standalone(v, current_video_id) {
                        return Some(hit);
                    }
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    if let Some(hit) = walk_standalone(v, current_video_id) {
                        return Some(hit);
                    }
                }
            }
            _ => {}
        }
        None
    }

    let mut fallback = None;
    walk_wrappers(val, current_video_id, &mut fallback)
        .or(fallback)
        .or_else(|| walk_standalone(val, current_video_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_music_video_counterpart_for_omv_and_rejects_atv_only() {
        let with_counterpart = json!({
            "contents": {
                "singleColumnMusicWatchNextResultsRenderer": {
                    "tabbedRenderer": {
                        "watchNextTabbedResultsRenderer": {
                            "tabs": [{
                                "tabRenderer": {
                                    "content": {
                                        "musicQueueRenderer": {
                                            "content": {
                                                "playlistPanelRenderer": {
                                                    "contents": [{
                                                        "playlistPanelVideoWrapperRenderer": {
                                                            "primaryRenderer": {
                                                                "playlistPanelVideoRenderer": {
                                                                    "videoId": "atv_song_1",
                                                                    "title": { "runs": [{ "text": "Blinding Lights" }] },
                                                                    "navigationEndpoint": {
                                                                        "watchEndpoint": {
                                                                            "videoId": "atv_song_1",
                                                                            "watchEndpointMusicSupportedConfigs": {
                                                                                "watchEndpointMusicConfig": {
                                                                                    "musicVideoType": "MUSIC_VIDEO_TYPE_ATV"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            },
                                                            "counterpart": [{
                                                                "counterpartRenderer": {
                                                                    "playlistPanelVideoRenderer": {
                                                                        "videoId": "omv_video_1",
                                                                        "title": { "runs": [{ "text": "Blinding Lights (Official Video)" }] },
                                                                        "lengthText": { "runs": [{ "text": "4:22" }] },
                                                                        "thumbnail": {
                                                                            "thumbnails": [{ "url": "https://i.ytimg.com/vi/omv_video_1/hqdefault.jpg" }]
                                                                        },
                                                                        "navigationEndpoint": {
                                                                            "watchEndpoint": {
                                                                                "videoId": "omv_video_1",
                                                                                "watchEndpointMusicSupportedConfigs": {
                                                                                    "watchEndpointMusicConfig": {
                                                                                        "musicVideoType": "MUSIC_VIDEO_TYPE_OMV"
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }]
                                                        }
                                                    }]
                                                }
                                            }
                                        }
                                    }
                                }
                            }]
                        }
                    }
                }
            }
        });

        let found = parse_music_video_counterpart_json(&with_counterpart, "atv_song_1")
            .expect("should find OMV counterpart");
        assert_eq!(found.video_id, "omv_video_1");
        assert_eq!(found.duration_secs, Some(262));

        let atv_only = json!({
            "playlistPanelVideoRenderer": {
                "videoId": "atv_only_1",
                "title": { "runs": [{ "text": "Audio Track" }] },
                "navigationEndpoint": {
                    "watchEndpoint": {
                        "videoId": "atv_only_1",
                        "watchEndpointMusicSupportedConfigs": {
                            "watchEndpointMusicConfig": {
                                "musicVideoType": "MUSIC_VIDEO_TYPE_ATV"
                            }
                        }
                    }
                }
            }
        });
        assert!(parse_music_video_counterpart_json(&atv_only, "atv_only_1").is_none());

        let omv_primary = json!({
            "playlistPanelVideoRenderer": {
                "videoId": "omv_direct_1",
                "title": { "runs": [{ "text": "Direct Music Video" }] },
                "navigationEndpoint": {
                    "watchEndpoint": {
                        "videoId": "omv_direct_1",
                        "watchEndpointMusicSupportedConfigs": {
                            "watchEndpointMusicConfig": {
                                "musicVideoType": "MUSIC_VIDEO_TYPE_OMV"
                            }
                        }
                    }
                }
            }
        });
        assert_eq!(
            parse_music_video_counterpart_json(&omv_primary, "omv_direct_1")
                .map(|c| c.video_id)
                .as_deref(),
            Some("omv_direct_1")
        );
    }

    #[test]
    fn parses_segment_map_and_builds_sponsorblock_offtopic_sync_segments() {
        let with_segment_map = json!({
            "playlistPanelVideoWrapperRenderer": {
                "primaryRenderer": {
                    "playlistPanelVideoRenderer": {
                        "videoId": "atv_sync_1",
                        "title": { "runs": [{ "text": "Song" }] }
                    }
                },
                "counterpart": [{
                    "counterpartRenderer": {
                        "playlistPanelVideoRenderer": {
                            "videoId": "omv_sync_1",
                            "title": { "runs": [{ "text": "Song (Official Video)" }] },
                            "navigationEndpoint": {
                                "watchEndpoint": {
                                    "videoId": "omv_sync_1",
                                    "watchEndpointMusicSupportedConfigs": {
                                        "watchEndpointMusicConfig": {
                                            "musicVideoType": "MUSIC_VIDEO_TYPE_OMV"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "segmentMap": {
                        "segment": [
                            {
                                "primaryVideoStartTimeMilliseconds": "0",
                                "counterpartVideoStartTimeMilliseconds": "18500",
                                "durationMilliseconds": "92000"
                            },
                            {
                                "primaryVideoStartTimeMilliseconds": "92000",
                                "counterpartVideoStartTimeMilliseconds": "125000",
                                "durationMilliseconds": "110000"
                            }
                        ]
                    }
                }]
            }
        });

        let parsed = parse_music_video_counterpart_json(&with_segment_map, "atv_sync_1")
            .expect("should parse counterpart with segmentMap");
        assert_eq!(parsed.sync_segments.len(), 2);
        assert_eq!(
            parsed.sync_segments[0],
            MusicVideoSyncSegment {
                music_start_ms: 0,
                video_start_ms: 18_500,
                duration_ms: 92_000,
            }
        );
        assert_eq!(
            parsed.sync_segments[1],
            MusicVideoSyncSegment {
                music_start_ms: 92_000,
                video_start_ms: 125_000,
                duration_ms: 110_000,
            }
        );

        // Intro (0..15s), mid-video dialogue (75s..90s), outro (210s..240s)
        let built = build_sync_segments_from_offtopic(
            &[(0, 15_000), (75_000, 90_000), (210_000, 240_000)],
            240_000,
        );
        assert_eq!(
            built,
            vec![
                MusicVideoSyncSegment {
                    music_start_ms: 0,
                    video_start_ms: 15_000,
                    duration_ms: 60_000,
                },
                MusicVideoSyncSegment {
                    music_start_ms: 60_000,
                    video_start_ms: 90_000,
                    duration_ms: 120_000,
                },
            ]
        );
        assert!(build_sync_segments_from_offtopic(&[], 200_000).is_empty());
    }
}
