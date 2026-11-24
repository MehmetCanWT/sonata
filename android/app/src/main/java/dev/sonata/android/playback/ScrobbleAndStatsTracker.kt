package dev.sonata.android.playback

import uniffi.sonata_android.AndroidTrack

/**
 * Pure threshold tracker enforcing the `40s` / `50%` playback rule for scrobbling and
 * local listening statistics without touching UI threads or double-counting replays.
 */
class ScrobbleAndStatsTracker {
    data class AdvanceOutcome(
        val scrobbleTriggerTimestampUnix: Long? = null,
        val statsPlayedSecs: Long? = null,
        val track: AndroidTrack? = null,
        val sourceTag: String = "",
    )

    private var activeGeneration: Long = 0L
    private var activeTrack: AndroidTrack? = null
    private var activeSourceTag: String = "YTM"
    private var startedAtUnixSecs: Long = 0L
    private var accumulatedMs: Long = 0L
    private var nowPlayingSent: Boolean = false
    private var thresholdTriggered: Boolean = false

    fun onTrackStarted(
        generation: Long,
        track: AndroidTrack,
        sourceTag: String,
        startedAtUnixSecs: Long,
    ) {
        if (generation <= 0L) return
        if (activeGeneration == generation && activeTrack?.id == track.id) return
        activeGeneration = generation
        activeTrack = track
        activeSourceTag = sourceTag
        this.startedAtUnixSecs = startedAtUnixSecs
        accumulatedMs = 0L
        nowPlayingSent = false
        thresholdTriggered = false
    }

    /**
     * Marks `Now Playing` as needing dispatch once per active track generation.
     * Returns the track if this call is the first `Now Playing` notification for this track.
     */
    fun consumeNowPlaying(generation: Long): AndroidTrack? {
        if (generation <= 0L || generation != activeGeneration || nowPlayingSent) return null
        val track = activeTrack ?: return null
        nowPlayingSent = true
        return track
    }

    /**
     * Advances listened playback time by [deltaMs] while actively playing and returns
     * one-shot triggers once the `40s` / `50%` rule is satisfied.
     */
    fun advance(generation: Long, deltaMs: Long): AdvanceOutcome {
        if (generation <= 0L || generation != activeGeneration || deltaMs <= 0L) {
            return AdvanceOutcome()
        }
        val track = activeTrack ?: return AdvanceOutcome()
        accumulatedMs += deltaMs
        if (thresholdTriggered) return AdvanceOutcome()

        val playedSecs = accumulatedMs / 1_000L
        val durationSecs = track.durationSecs?.toLong()?.takeIf { it > 0L }
        if (!shouldScrobble(durationSecs, playedSecs)) {
            return AdvanceOutcome()
        }

        thresholdTriggered = true
        val recordedSecs = durationSecs ?: playedSecs.coerceAtLeast(1L)
        return AdvanceOutcome(
            scrobbleTriggerTimestampUnix = startedAtUnixSecs,
            statsPlayedSecs = recordedSecs,
            track = track,
            sourceTag = activeSourceTag,
        )
    }

    fun isThresholdTriggered(generation: Long): Boolean =
        generation > 0L && generation == activeGeneration && thresholdTriggered

    fun reset() {
        activeGeneration = 0L
        activeTrack = null
        activeSourceTag = "YTM"
        startedAtUnixSecs = 0L
        accumulatedMs = 0L
        nowPlayingSent = false
        thresholdTriggered = false
    }

    companion object {
        const val SCROBBLE_THRESHOLD_SECS = 40L

        /**
         * Returns `true` when [playedSecs] reaches either 40 seconds or 50% of [durationSecs].
         */
        fun shouldScrobble(durationSecs: Long?, playedSecs: Long): Boolean {
            if (playedSecs <= 0L) return false
            val validDuration = durationSecs?.takeIf { it > 0L }
            return if (validDuration != null) {
                playedSecs >= SCROBBLE_THRESHOLD_SECS || playedSecs * 2L >= validDuration
            } else {
                playedSecs >= SCROBBLE_THRESHOLD_SECS
            }
        }

        fun inferSourceTag(trackId: String): String = when {
            trackId.startsWith("local:") -> "LOC"
            trackId.startsWith("jellyfin:") -> "JFN"
            trackId.startsWith("subsonic:") -> "SUB"
            trackId.startsWith("smb:") -> "SMB"
            trackId.startsWith("webdav:") -> "DAV"
            trackId.startsWith("jiosaavn:") -> "JIO"
            else -> "YTM"
        }
    }
}
