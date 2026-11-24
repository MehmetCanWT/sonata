package dev.sonata.android.data

import androidx.compose.runtime.Immutable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * Snapshot of Sonata Android's live audio signal chain for the "Stats for Nerds" modal
 * and Now Playing quality badges.
 */
@Immutable
data class AudioPipelineInfo(
    val sourceLabel: String = "Idle",
    val codec: String = "—",
    val bitrateKbps: Int = 0,
    val sampleRateHz: Int = 0,
    val channelCount: Int = 0,
    val outputPath: String = "Media3 AudioTrack",
    val skipSilenceActive: Boolean = false,
    val playbackSpeed: Float = 1.0f,
    val eqActive: Boolean = false,
    val loudnessNormActive: Boolean = false,
    val crossfadeSeconds: Int = 0,
    val fromCache: Boolean = false,
    val fromOfflineDownload: Boolean = false,
) {
    val isBitTransparentDsp: Boolean
        get() = !skipSilenceActive &&
            playbackSpeed == 1.0f &&
            !eqActive &&
            !loudnessNormActive &&
            crossfadeSeconds <= 0

    val channelLabel: String
        get() = when (channelCount) {
            0 -> "—"
            1 -> "Mono"
            2 -> "Stereo"
            6 -> "5.1 Surround"
            8 -> "7.1 Surround"
            else -> "$channelCount ch"
        }

    val sampleRateLabel: String
        get() = when {
            sampleRateHz <= 0 -> "—"
            sampleRateHz % 1000 == 0 -> "${sampleRateHz / 1000} kHz"
            else -> String.format("%.1f kHz", sampleRateHz / 1000.0)
        }
}

/**
 * Process-wide reactive bus connecting Media3 `AnalyticsListener` format callbacks in
 * `PlaybackService` with Jetpack Compose UI observers.
 */
object AudioPipelineBus {
    private val _info = MutableStateFlow(AudioPipelineInfo())
    val info: StateFlow<AudioPipelineInfo> = _info.asStateFlow()

    fun update(transform: (AudioPipelineInfo) -> AudioPipelineInfo) {
        _info.update(transform)
    }

    fun set(next: AudioPipelineInfo) {
        _info.value = next
    }

    fun reset() {
        _info.value = AudioPipelineInfo()
    }
}
