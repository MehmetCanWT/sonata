package dev.sonata.android.data

import androidx.compose.runtime.Immutable
import androidx.media3.common.C
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.io.File
import java.util.Locale
import kotlin.math.roundToInt

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
    val bitDepthBits: Int = 0,
    val mediaId: String = "",
    val measuredBadge: String = "",
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
            else -> String.format(Locale.US, "%.1f kHz", sampleRateHz / 1000.0)
        }

    val bitDepthLabel: String
        get() = if (bitDepthBits > 0) "${bitDepthBits}-bit" else ""

    val formattedBadge: String
        get() = measuredBadge.ifBlank {
            formatMeasuredAudioBadge(
                codec = codec,
                bitrateKbps = bitrateKbps,
                sampleRateHz = sampleRateHz,
                bitDepthBits = bitDepthBits,
            )
        }
}

private val BADGE_KBPS_REGEX = Regex("(\\d{2,5})\\s*kbps", RegexOption.IGNORE_CASE)
private val BADGE_SAMPLE_SPEC_REGEX = Regex("(?:\\d+-bit|\\d+(?:\\.\\d+)?\\s*kHz)", RegexOption.IGNORE_CASE)

private val LOSSLESS_CODECS = setOf(
    "FLAC",
    "ALAC",
    "WAV",
    "PCM",
    "AIFF",
    "APE",
    "WV",
    "DSD",
)

private val SERVER_LABELS = setOf(
    "Jellyfin",
    "Subsonic",
    "WebDAV",
)

private val PRESERVED_QUALITY_AND_SOURCE_TAGS = mapOf(
    "hi-res" to "Hi-Res",
    "hi-fi" to "Hi-Fi",
    "hifi" to "Hi-Fi",
    "extreme" to "Extreme",
    "hq" to "HQ",
    "standard" to "Standard",
    "jellyfin" to "Jellyfin",
    "subsonic" to "Subsonic",
    "webdav" to "WebDAV",
    "jiosaavn" to "JioSaavn",
    "tidal" to "Tidal",
    "qobuz" to "Qobuz",
    "deezer" to "Deezer",
    "cached" to "Cached",
)

/**
 * Maps a Media3 [C.PcmEncoding] constant to integer bit depth (`8`, `16`, `24`, `32`),
 * or `0` when unknown/unset.
 */
fun pcmEncodingToBitDepth(pcmEncoding: Int): Int = when (pcmEncoding) {
    C.ENCODING_PCM_8BIT -> 8
    C.ENCODING_PCM_16BIT, C.ENCODING_PCM_16BIT_BIG_ENDIAN -> 16
    C.ENCODING_PCM_24BIT, C.ENCODING_PCM_24BIT_BIG_ENDIAN -> 24
    C.ENCODING_PCM_32BIT, C.ENCODING_PCM_32BIT_BIG_ENDIAN, C.ENCODING_PCM_FLOAT -> 32
    else -> 0
}

/**
 * Normalizes a Media3 MIME type and/or RFC 6381 codec tag into a concise uppercase codec badge label.
 */
fun normalizeAudioCodecLabel(sampleMimeType: String?, codecs: String? = null): String {
    val mime = sampleMimeType.orEmpty().trim().lowercase(Locale.ROOT)
    val codecStr = codecs.orEmpty().trim().lowercase(Locale.ROOT)
    return when {
        mime == "audio/opus" || "opus" in codecStr -> "OPUS"
        mime == "audio/flac" || mime == "audio/x-flac" || "flac" in codecStr -> "FLAC"
        mime == "audio/alac" || "alac" in codecStr -> "ALAC"
        mime == "audio/wav" || mime == "audio/x-wav" || mime == "audio/vnd.wave" -> "WAV"
        mime == "audio/raw" || mime == "audio/l16" || mime == "audio/l24" -> "PCM"
        mime == "audio/aiff" || mime == "audio/x-aiff" -> "AIFF"
        mime == "audio/mp4a-latm" || mime == "audio/aac" || "mp4a" in codecStr || "aac" in codecStr -> "AAC"
        mime == "audio/mpeg" || mime == "audio/mp3" || "mp3" in codecStr -> "MP3"
        mime == "audio/vorbis" || "vorbis" in codecStr -> "VORBIS"
        mime.startsWith("audio/") -> mime.removePrefix("audio/").removePrefix("x-").uppercase(Locale.ROOT)
        codecStr.isNotBlank() && codecStr != "—" -> codecStr.uppercase(Locale.ROOT)
        else -> ""
    }
}

/** Returns true when [codec] represents an uncompressed or lossless audio format. */
fun isLosslessAudioCodec(codec: String): Boolean =
    codec.trim().uppercase(Locale.ROOT) in LOSSLESS_CODECS

/** Formats sample rate in Hz for compact badges (e.g. `44.1kHz`, `48kHz`, `96kHz`). */
fun formatBadgeSampleRate(sampleRateHz: Int): String = when {
    sampleRateHz <= 0 -> ""
    sampleRateHz % 1000 == 0 -> "${sampleRateHz / 1000}kHz"
    else -> String.format(Locale.US, "%.1fkHz", sampleRateHz / 1000.0)
}

/**
 * Computes exact measured bitrate in kbps from media size in bytes and duration in milliseconds.
 */
fun computeMeasuredBitrateKbps(contentBytes: Long, durationMs: Long): Int {
    if (contentBytes <= 0L || durationMs <= 0L) return 0
    return ((contentBytes.toDouble() * 8.0) / durationMs.toDouble()).roundToInt().coerceAtLeast(0)
}

/**
 * Computes exact measured bitrate in kbps from a local [File] and track duration in seconds.
 */
fun computeLocalFileBitrateKbps(file: File, durationSecs: Long): Int {
    if (durationSecs <= 0L || !file.isFile) return 0
    val lengthBytes = runCatching { file.length() }.getOrDefault(0L)
    if (lengthBytes <= 0L) return 0
    return computeMeasuredBitrateKbps(contentBytes = lengthBytes, durationMs = durationSecs * 1_000L)
}

/**
 * Resolves measured bitrate in kbps, preferring ExoPlayer's reported `averageBitrate`/`bitrate`
 * when positive and falling back to `(contentBytes * 8) / durationMs` when ExoPlayer reports
 * `Format.NO_VALUE` or `<= 0`.
 */
fun resolveMeasuredBitrateKbps(
    reportedAverageBitrateBps: Int,
    reportedBitrateBps: Int,
    contentBytes: Long = 0L,
    durationMs: Long = 0L,
): Int {
    if (reportedAverageBitrateBps > 0) {
        val avgKbps = (reportedAverageBitrateBps / 1000.0).roundToInt()
        if (avgKbps > 0) return avgKbps
    }
    if (reportedBitrateBps > 0) {
        val peakKbps = (reportedBitrateBps / 1000.0).roundToInt()
        if (peakKbps > 0) return peakKbps
    }
    return computeMeasuredBitrateKbps(contentBytes = contentBytes, durationMs = durationMs)
}

/**
 * Parses total entity length in bytes from an HTTP `Content-Range` header (e.g. `bytes 0-1023/22000000`).
 */
fun parseHttpContentRangeTotal(responseHeaders: Map<String, List<String>>): Long {
    val header = responseHeaders.entries
        .firstOrNull { it.key.equals("Content-Range", ignoreCase = true) }
        ?.value
        ?.firstOrNull()
        ?.trim()
        ?: return 0L
    val total = header.substringAfter('/', "").trim().toLongOrNull() ?: return 0L
    return total.takeIf { it > 0L } ?: 0L
}

/**
 * Parses `Content-Length` in bytes from HTTP response headers.
 */
fun parseHttpContentLength(responseHeaders: Map<String, List<String>>): Long {
    val header = responseHeaders.entries
        .firstOrNull { it.key.equals("Content-Length", ignoreCase = true) }
        ?.value
        ?.firstOrNull()
        ?.trim()
        ?.toLongOrNull()
        ?: return 0L
    return header.takeIf { it > 0L } ?: 0L
}

/**
 * Extracts total entity size in bytes from HTTP response headers, preferring `Content-Range`
 * total length over `Content-Length` so 206 partial-range chunks do not mask the full stream size.
 */
fun parseHttpContentBytes(responseHeaders: Map<String, List<String>>): Long {
    val rangeTotal = parseHttpContentRangeTotal(responseHeaders)
    if (rangeTotal > 0L) return rangeTotal
    return parseHttpContentLength(responseHeaders)
}

/**
 * Formats a clean live audio quality badge from measured track telemetry, e.g.:
 * - `FLAC · 880 kbps · 16-bit/44.1kHz`
 * - `FLAC · 2480 kbps · 24-bit/96kHz`
 * - `OPUS · 256 kbps`
 * - `AAC · 320 kbps`
 */
fun formatMeasuredAudioBadge(
    codec: String,
    bitrateKbps: Int = 0,
    sampleRateHz: Int = 0,
    bitDepthBits: Int = 0,
    fallbackBadge: String = "",
): String {
    val rawCodec = codec.trim()
    val normalizedCodec = when {
        rawCodec.isBlank() || rawCodec == "—" -> ""
        rawCodec.startsWith("audio/", ignoreCase = true) -> normalizeAudioCodecLabel(rawCodec)
        else -> rawCodec.uppercase(Locale.ROOT)
    }

    val lossless = normalizedCodec.isNotEmpty() && isLosslessAudioCodec(normalizedCodec)
    val depthText = if (bitDepthBits > 0) "${bitDepthBits}-bit" else ""
    val rateText = formatBadgeSampleRate(sampleRateHz)
    val pcmSpec = when {
        depthText.isNotEmpty() && rateText.isNotEmpty() -> "$depthText/$rateText"
        depthText.isNotEmpty() -> depthText
        rateText.isNotEmpty() -> rateText
        else -> ""
    }

    if (normalizedCodec.isEmpty()) {
        val parts = buildList {
            if (bitrateKbps > 0) add("$bitrateKbps kbps")
            if (pcmSpec.isNotEmpty()) add(pcmSpec)
        }
        return if (parts.isNotEmpty()) parts.joinToString(" · ") else fallbackBadge.trim()
    }

    if (bitrateKbps <= 0 && (!lossless || pcmSpec.isEmpty())) {
        return if (fallbackBadge.isNotBlank()) {
            fallbackBadge.trim()
        } else if (rateText.isNotEmpty()) {
            "$normalizedCodec · $rateText"
        } else {
            normalizedCodec
        }
    }

    val parts = buildList {
        add(normalizedCodec)
        if (bitrateKbps > 0) add("$bitrateKbps kbps")
        if (lossless && pcmSpec.isNotEmpty()) add(pcmSpec)
    }
    return parts.joinToString(" · ")
}

/**
 * Combines an initial resolved stream badge (preserving Server/Hi-Fi/Offline/Cached facts) with
 * ExoPlayer's live [measuredBadge].
 */
fun mergeStreamBadgeWithMeasurement(
    initialBadge: String,
    measuredBadge: String,
    sourceLabel: String = "",
    measuredBitrateKbps: Int = 0,
): String {
    val cleanInitial = initialBadge.trim()
    val cleanMeasured = measuredBadge.trim()
    if (cleanMeasured.isBlank()) return cleanInitial

    val hasMeasuredBitrate = measuredBitrateKbps > 0 || BADGE_KBPS_REGEX.containsMatchIn(cleanMeasured)
    val hasMeasuredSampleSpec = BADGE_SAMPLE_SPEC_REGEX.containsMatchIn(cleanMeasured)

    if (!hasMeasuredBitrate && !hasMeasuredSampleSpec) {
        return cleanInitial.ifBlank { cleanMeasured }
    }
    if (!hasMeasuredBitrate && BADGE_KBPS_REGEX.containsMatchIn(cleanInitial)) {
        return cleanInitial
    }

    val initialTokens = cleanInitial.split('·').map { it.trim() }.filter { it.isNotEmpty() }
    val isOffline = initialTokens.any { it.equals("Offline", ignoreCase = true) }

    val trailingFacts = LinkedHashSet<String>()
    for (token in initialTokens) {
        if (token.equals("Offline", ignoreCase = true)) continue
        val canonical = PRESERVED_QUALITY_AND_SOURCE_TAGS[token.lowercase(Locale.ROOT)] ?: continue
        if (canonical == "Cached" && isOffline) continue
        trailingFacts.add(canonical)
    }
    if (sourceLabel in SERVER_LABELS) {
        trailingFacts.add(sourceLabel)
    }

    val parts = ArrayList<String>(4)
    if (isOffline) parts.add("Offline")
    parts.add(cleanMeasured)
    for (fact in trailingFacts) {
        if (!cleanMeasured.contains(fact, ignoreCase = true)) {
            parts.add(fact)
        }
    }
    return parts.joinToString(" · ")
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
