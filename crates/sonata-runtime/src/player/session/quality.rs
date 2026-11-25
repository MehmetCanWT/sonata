//! Audio quality measurement and background Hi-Fi stream upgrade orchestration.
//!
//! 1. **Quality Tracker**: tracks local file byte sizes, live bitrates, codecs, sample rates, and
//!    bit depths to emit normalized `AudioQualityUpdated` badges without duplicate churn.
//! 2. **Hi-Fi Upgrades**: spawns generation-scoped upgrade resolution tasks against the active
//!    `PlaybackStreamSource` once playback starts.

use super::{InternalResult, SessionActor, SessionEvent};
use crate::player::mpv::{
    compute_bitrate_kbps_from_bytes, format_measured_audio_badge, normalize_audio_codec_label,
};
use sonata_core::player::PlaybackGeneration;
use sonata_core::providers::hifi::{extract_badge_kbps, ResolvedStream};
use sonata_core::providers::ProviderSource;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub(super) struct AudioQualityTracker {
    pub track_id: String,
    pub base_badge: String,
    pub local_file_bytes: Option<u64>,
    pub codec: Option<String>,
    pub live_bitrate_bps: Option<u64>,
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u8>,
    pub last_emitted_badge: Option<String>,
}

fn local_path_from_stream_url(url: &str) -> Option<PathBuf> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return None;
    }
    let path_str = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    (!path_str.is_empty()).then(|| PathBuf::from(path_str))
}

impl SessionActor {
    pub(super) fn begin_audio_quality_tracking(
        &mut self,
        generation: PlaybackGeneration,
        track_id: &str,
        stream: &ResolvedStream,
    ) {
        self.audio_quality = AudioQualityTracker {
            track_id: track_id.to_string(),
            base_badge: stream.badge.clone(),
            ..AudioQualityTracker::default()
        };
        if let Some(local_path) = local_path_from_stream_url(&stream.url) {
            let tx = self.internal_tx.clone();
            let owned_track_id = track_id.to_string();
            self.playback_tasks.spawn(async move {
                let bytes = tokio::task::spawn_blocking(move || {
                    std::fs::metadata(&local_path)
                        .ok()
                        .map(|m| m.len())
                        .filter(|&n| n > 0)
                })
                .await
                .ok()
                .flatten();
                if let Some(bytes) = bytes {
                    let _ = tx
                        .send(InternalResult::LocalFileSize {
                            generation,
                            track_id: owned_track_id,
                            bytes,
                        })
                        .await;
                }
            });
        }
    }

    pub(super) async fn refresh_measured_audio_quality(&mut self) {
        if self.audio_quality.track_id.is_empty() || self.playback.loading_track {
            return;
        }
        let live_kbps = self.audio_quality.live_bitrate_bps.and_then(|bps| {
            let kbps = ((bps as f64) / 1000.0).round() as u32;
            (kbps > 0).then_some(kbps)
        });
        let local_kbps = self
            .audio_quality
            .local_file_bytes
            .and_then(|bytes| compute_bitrate_kbps_from_bytes(bytes, self.playback.duration_secs));
        let fallback_kbps = extract_badge_kbps(&self.audio_quality.base_badge);
        let measured_kbps = live_kbps.or(local_kbps).or(fallback_kbps);

        let Some(badge) = format_measured_audio_badge(
            self.audio_quality.codec.as_deref(),
            measured_kbps,
            self.audio_quality.bit_depth,
            self.audio_quality.sample_rate_hz,
            Some(&self.audio_quality.base_badge),
        ) else {
            return;
        };

        if self.audio_quality.last_emitted_badge.as_deref() == Some(badge.as_str()) {
            return;
        }
        self.audio_quality.last_emitted_badge = Some(badge.clone());
        self.playback.stream_badge = badge.clone();

        let codec = self
            .audio_quality
            .codec
            .as_deref()
            .map(normalize_audio_codec_label);
        let track_id = self.audio_quality.track_id.clone();
        let sample_rate_hz = self.audio_quality.sample_rate_hz;
        let bit_depth = self.audio_quality.bit_depth;

        self.emit(SessionEvent::AudioQualityUpdated {
            generation: self.generation,
            track_id,
            badge,
            codec,
            bitrate_kbps: measured_kbps,
            sample_rate_hz,
            bit_depth,
        })
        .await;
    }

    pub(super) async fn start_hifi_upgrade(&mut self) {
        if self.video_mode
            || self.playback.paused
            || self.playback.loading_track
            || self.upgrade_started_generation == Some(self.generation)
        {
            return;
        }
        let (Some(track), Some(stream)) = (
            self.playback.current_track().cloned(),
            self.playback.active_stream.clone(),
        ) else {
            return;
        };
        if track.source == ProviderSource::Spotify
            && (!self.config.policy.hifi_substitution
                || !self.source.needs_hifi_upgrade(&stream.badge, &stream.url))
        {
            return;
        }

        let generation = self.generation;
        self.upgrade_started_generation = Some(generation);
        self.upgrading_quality = true;
        tracing::info!(
            event = "upgrade_resolving",
            client = self.config.client_name,
            track_id = %track.id,
            title = %track.title,
            artist = %track.artist,
            provider = ?track.source,
            play_seq = generation.value(),
            current_quality = %stream.badge,
            "resolving higher quality stream"
        );
        self.emit(SessionEvent::UpgradeStarted {
            generation,
            track_id: track.id.clone(),
        })
        .await;

        let source = Arc::clone(&self.source);
        let tx = self.internal_tx.clone();
        let allow_hifi = self.config.policy.hifi_substitution;
        let client_name = self.config.client_name;
        let track_id = track.id.clone();
        self.playback_tasks.spawn(async move {
            if let Some(upgraded) = source.resolve_hifi_upgrade(track, stream, allow_hifi).await {
                let _ = tx
                    .send(InternalResult::StreamUpgrade {
                        generation,
                        track_id,
                        stream: upgraded,
                    })
                    .await;
            } else {
                tracing::info!(
                    event = "upgrade_unavailable",
                    client = client_name,
                    track_id = %track_id,
                    play_seq = generation.value(),
                    "no higher quality stream available"
                );
                let _ = tx
                    .send(InternalResult::UpgradeFinished {
                        generation,
                        track_id,
                    })
                    .await;
            }
        });
    }
}
