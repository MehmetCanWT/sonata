/*
 * Bidirectional timeline mapper between official album Audio (Music) and companion Music Video.
 *
 * Music videos frequently contain silent/cinematic intros, mid-video skits/pauses, or outro
 * credits that shift vocal timestamps relative to lyrics synced against the album audio track.
 */
package dev.sonata.android.playback

import uniffi.sonata_android.AndroidMusicVideoSyncSegment

/**
 * Maps an album-audio playback position [musicMs] onto the corresponding Music Video timestamp
 * using piecewise aligned [segments]. Returns [musicMs] clamped >= 0 when [segments] is empty.
 */
fun mapMusicToVideoMs(
    musicMs: Long,
    segments: List<AndroidMusicVideoSyncSegment>,
): Long {
    val clamped = musicMs.coerceAtLeast(0L)
    if (segments.isEmpty()) return clamped
    val ordered = segments
        .filter { it.durationMs > 0uL }
        .sortedBy { it.musicStartMs }
    if (ordered.isEmpty()) return clamped

    val first = ordered.first()
    val firstMusicStart = first.musicStartMs.toLong()
    val firstVideoStart = first.videoStartMs.toLong()
    if (clamped < firstMusicStart) {
        return firstVideoStart.coerceAtLeast(0L)
    }

    for (seg in ordered) {
        val mStart = seg.musicStartMs.toLong()
        val vStart = seg.videoStartMs.toLong()
        val dur = seg.durationMs.toLong()
        val mEnd = mStart + dur
        if (clamped < mStart) {
            return vStart.coerceAtLeast(0L)
        }
        if (clamped <= mEnd) {
            return (vStart + (clamped - mStart)).coerceAtLeast(0L)
        }
    }

    val last = ordered.last()
    val lastMusicEnd = last.musicStartMs.toLong() + last.durationMs.toLong()
    val lastVideoEnd = last.videoStartMs.toLong() + last.durationMs.toLong()
    return (lastVideoEnd + (clamped - lastMusicEnd)).coerceAtLeast(0L)
}

/**
 * Maps a Music Video playback position [videoMs] onto the canonical album-audio timeline used by
 * synced lyrics and Music mode resume. During non-music intros/interludes/outros, holds the
 * upcoming or trailing music position cleanly instead of drifting lyrics out of sync.
 */
fun mapVideoToMusicMs(
    videoMs: Long,
    segments: List<AndroidMusicVideoSyncSegment>,
): Long {
    val clamped = videoMs.coerceAtLeast(0L)
    if (segments.isEmpty()) return clamped
    val ordered = segments
        .filter { it.durationMs > 0uL }
        .sortedBy { it.videoStartMs }
    if (ordered.isEmpty()) return clamped

    val first = ordered.first()
    if (clamped < first.videoStartMs.toLong()) {
        return first.musicStartMs.toLong().coerceAtLeast(0L)
    }

    for (seg in ordered) {
        val vStart = seg.videoStartMs.toLong()
        val mStart = seg.musicStartMs.toLong()
        val dur = seg.durationMs.toLong()
        val vEnd = vStart + dur
        if (clamped < vStart) {
            // Inside a mid-video non-music silence/skit: hold at the upcoming music boundary.
            return mStart.coerceAtLeast(0L)
        }
        if (clamped <= vEnd) {
            return (mStart + (clamped - vStart)).coerceAtLeast(0L)
        }
    }

    val last = ordered.last()
    return (last.musicStartMs.toLong() + last.durationMs.toLong()).coerceAtLeast(0L)
}
