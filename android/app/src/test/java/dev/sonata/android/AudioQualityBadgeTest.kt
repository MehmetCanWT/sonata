package dev.sonata.android

import androidx.media3.common.C
import androidx.media3.common.Format
import dev.sonata.android.data.computeLocalFileBitrateKbps
import dev.sonata.android.data.computeMeasuredBitrateKbps
import dev.sonata.android.data.formatBadgeSampleRate
import dev.sonata.android.data.formatMeasuredAudioBadge
import dev.sonata.android.data.isLosslessAudioCodec
import dev.sonata.android.data.mergeStreamBadgeWithMeasurement
import dev.sonata.android.data.normalizeAudioCodecLabel
import dev.sonata.android.data.parseHttpContentBytes
import dev.sonata.android.data.pcmEncodingToBitDepth
import dev.sonata.android.data.resolveMeasuredBitrateKbps
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.RandomAccessFile
import java.nio.file.Files

class AudioQualityBadgeTest {

    @Test
    fun formatMeasuredAudioBadgeFormatsFlac720And880Kbps16Bit44_1kHz() {
        assertEquals(
            "FLAC · 720 kbps · 16-bit/44.1kHz",
            formatMeasuredAudioBadge(
                codec = "FLAC",
                bitrateKbps = 720,
                sampleRateHz = 44_100,
                bitDepthBits = 16,
            ),
        )
        assertEquals(
            "FLAC · 880 kbps · 16-bit/44.1kHz",
            formatMeasuredAudioBadge(
                codec = "flac",
                bitrateKbps = 880,
                sampleRateHz = 44_100,
                bitDepthBits = 16,
            ),
        )
    }

    @Test
    fun formatMeasuredAudioBadgeFormatsHiRes24BitAnd32BitLosslessStreams() {
        assertEquals(
            "FLAC · 2480 kbps · 24-bit/96kHz",
            formatMeasuredAudioBadge(
                codec = "FLAC",
                bitrateKbps = 2480,
                sampleRateHz = 96_000,
                bitDepthBits = 24,
            ),
        )
        assertEquals(
            "ALAC · 4608 kbps · 24-bit/192kHz",
            formatMeasuredAudioBadge(
                codec = "ALAC",
                bitrateKbps = 4608,
                sampleRateHz = 192_000,
                bitDepthBits = 24,
            ),
        )
        assertEquals(
            "WAV · 3072 kbps · 32-bit/48kHz",
            formatMeasuredAudioBadge(
                codec = "WAV",
                bitrateKbps = 3072,
                sampleRateHz = 48_000,
                bitDepthBits = 32,
            ),
        )
    }

    @Test
    fun formatMeasuredAudioBadgeFormatsOpusAndAacWithoutSyntheticPcmBitDepth() {
        assertEquals(
            "OPUS · 256 kbps",
            formatMeasuredAudioBadge(
                codec = "Opus",
                bitrateKbps = 256,
                sampleRateHz = 48_000,
                bitDepthBits = 16,
            ),
        )
        assertEquals(
            "AAC · 320 kbps",
            formatMeasuredAudioBadge(
                codec = "AAC",
                bitrateKbps = 320,
                sampleRateHz = 44_100,
                bitDepthBits = 16,
            ),
        )
        assertEquals(
            "MP3 · 320 kbps",
            formatMeasuredAudioBadge(
                codec = "mp3",
                bitrateKbps = 320,
                sampleRateHz = 44_100,
                bitDepthBits = 0,
            ),
        )
    }

    @Test
    fun formatMeasuredAudioBadgeHandlesMissingMetadataFallback() {
        assertEquals(
            "FLAC · 24-bit/96kHz",
            formatMeasuredAudioBadge(
                codec = "FLAC",
                bitrateKbps = 0,
                sampleRateHz = 96_000,
                bitDepthBits = 24,
            ),
        )
        assertEquals(
            "FLAC · Jellyfin",
            formatMeasuredAudioBadge(
                codec = "FLAC",
                bitrateKbps = 0,
                sampleRateHz = 0,
                bitDepthBits = 0,
                fallbackBadge = "FLAC · Jellyfin",
            ),
        )
        assertEquals(
            "OPUS · 48kHz",
            formatMeasuredAudioBadge(
                codec = "OPUS",
                bitrateKbps = 0,
                sampleRateHz = 48_000,
                bitDepthBits = 0,
            ),
        )
        assertEquals(
            "OPUS 256kbps · Extreme",
            formatMeasuredAudioBadge(
                codec = "OPUS",
                bitrateKbps = 0,
                sampleRateHz = 48_000,
                bitDepthBits = 0,
                fallbackBadge = "OPUS 256kbps · Extreme",
            ),
        )
        assertEquals(
            "Lossless",
            formatMeasuredAudioBadge(
                codec = "—",
                bitrateKbps = 0,
                sampleRateHz = 0,
                bitDepthBits = 0,
                fallbackBadge = "Lossless",
            ),
        )
        assertEquals(
            "",
            formatMeasuredAudioBadge(
                codec = "",
                bitrateKbps = 0,
                sampleRateHz = 0,
                bitDepthBits = 0,
            ),
        )
    }

    @Test
    fun pcmEncodingToBitDepthMapsAllMedia3Encodings() {
        assertEquals(8, pcmEncodingToBitDepth(C.ENCODING_PCM_8BIT))
        assertEquals(16, pcmEncodingToBitDepth(C.ENCODING_PCM_16BIT))
        assertEquals(16, pcmEncodingToBitDepth(C.ENCODING_PCM_16BIT_BIG_ENDIAN))
        assertEquals(24, pcmEncodingToBitDepth(C.ENCODING_PCM_24BIT))
        assertEquals(24, pcmEncodingToBitDepth(C.ENCODING_PCM_24BIT_BIG_ENDIAN))
        assertEquals(32, pcmEncodingToBitDepth(C.ENCODING_PCM_32BIT))
        assertEquals(32, pcmEncodingToBitDepth(C.ENCODING_PCM_32BIT_BIG_ENDIAN))
        assertEquals(32, pcmEncodingToBitDepth(C.ENCODING_PCM_FLOAT))
        assertEquals(0, pcmEncodingToBitDepth(Format.NO_VALUE))
        assertEquals(0, pcmEncodingToBitDepth(C.ENCODING_INVALID))
    }

    @Test
    fun computeMeasuredBitrateKbpsFromContentBytesAndDurationMs() {
        assertEquals(720, computeMeasuredBitrateKbps(contentBytes = 18_000_000L, durationMs = 200_000L))
        assertEquals(880, computeMeasuredBitrateKbps(contentBytes = 22_000_000L, durationMs = 200_000L))
        // Guilty Conscience.flac lossless benchmark: 22,345,757 bytes over 201,280 ms = 888 kbps
        assertEquals(888, computeMeasuredBitrateKbps(contentBytes = 22_345_757L, durationMs = 201_280L))
        assertEquals(
            "FLAC · 888 kbps · 16-bit/44.1kHz",
            formatMeasuredAudioBadge(
                codec = "FLAC",
                bitrateKbps = computeMeasuredBitrateKbps(22_345_757L, 201_280L),
                sampleRateHz = 44_100,
                bitDepthBits = 16,
            ),
        )
        assertEquals(2480, computeMeasuredBitrateKbps(contentBytes = 55_800_000L, durationMs = 180_000L))
        assertEquals(0, computeMeasuredBitrateKbps(contentBytes = 0L, durationMs = 200_000L))
        assertEquals(0, computeMeasuredBitrateKbps(contentBytes = 22_000_000L, durationMs = 0L))
        assertEquals(0, computeMeasuredBitrateKbps(contentBytes = -1L, durationMs = 200_000L))
    }

    @Test
    fun computeLocalFileBitrateKbpsComputesExactRateFromFileSize() {
        val tempFile = Files.createTempFile("sonata-flac-test", ".flac").toFile()
        try {
            RandomAccessFile(tempFile, "rw").use { raf ->
                raf.setLength(22_000_000L)
            }
            assertEquals(880, computeLocalFileBitrateKbps(tempFile, durationSecs = 200L))
            assertEquals(0, computeLocalFileBitrateKbps(tempFile, durationSecs = 0L))
        } finally {
            tempFile.delete()
        }
    }

    @Test
    fun resolveMeasuredBitrateKbpsFallsBackWhenExoPlayerReportsNoValue() {
        assertEquals(
            880,
            resolveMeasuredBitrateKbps(
                reportedAverageBitrateBps = Format.NO_VALUE,
                reportedBitrateBps = Format.NO_VALUE,
                contentBytes = 22_000_000L,
                durationMs = 200_000L,
            ),
        )
        assertEquals(
            720,
            resolveMeasuredBitrateKbps(
                reportedAverageBitrateBps = 0,
                reportedBitrateBps = -1,
                contentBytes = 18_000_000L,
                durationMs = 200_000L,
            ),
        )
        assertEquals(
            256,
            resolveMeasuredBitrateKbps(
                reportedAverageBitrateBps = 256_000,
                reportedBitrateBps = 320_000,
                contentBytes = 18_000_000L,
                durationMs = 200_000L,
            ),
        )
        assertEquals(
            320,
            resolveMeasuredBitrateKbps(
                reportedAverageBitrateBps = Format.NO_VALUE,
                reportedBitrateBps = 320_000,
                contentBytes = 0L,
                durationMs = 0L,
            ),
        )
    }

    @Test
    fun parseHttpContentBytesPrefersContentRangeTotalOverChunkContentLength() {
        val rangeHeaders = mapOf(
            "Content-Length" to listOf("1048576"),
            "Content-Range" to listOf("bytes 0-1048575/22000000"),
        )
        assertEquals(22_000_000L, parseHttpContentBytes(rangeHeaders))

        val fullHeaders = mapOf(
            "content-length" to listOf("18000000"),
        )
        assertEquals(18_000_000L, parseHttpContentBytes(fullHeaders))
        assertEquals(0L, parseHttpContentBytes(emptyMap()))
    }

    @Test
    fun normalizeAudioCodecLabelAndLosslessClassification() {
        assertEquals("FLAC", normalizeAudioCodecLabel("audio/flac", null))
        assertEquals("FLAC", normalizeAudioCodecLabel("audio/x-flac", null))
        assertEquals("ALAC", normalizeAudioCodecLabel("audio/alac", null))
        assertEquals("OPUS", normalizeAudioCodecLabel("audio/opus", null))
        assertEquals("AAC", normalizeAudioCodecLabel("audio/mp4a-latm", "mp4a.40.2"))
        assertEquals("MP3", normalizeAudioCodecLabel("audio/mpeg", null))
        assertEquals("VORBIS", normalizeAudioCodecLabel("audio/vorbis", null))
        assertEquals("PCM", normalizeAudioCodecLabel("audio/raw", null))
        assertEquals("", normalizeAudioCodecLabel(null, null))

        assertTrue(isLosslessAudioCodec("FLAC"))
        assertTrue(isLosslessAudioCodec("alac"))
        assertTrue(isLosslessAudioCodec("WAV"))
        assertFalse(isLosslessAudioCodec("OPUS"))
        assertFalse(isLosslessAudioCodec("AAC"))

        assertEquals("44.1kHz", formatBadgeSampleRate(44_100))
        assertEquals("48kHz", formatBadgeSampleRate(48_000))
        assertEquals("96kHz", formatBadgeSampleRate(96_000))
        assertEquals("192kHz", formatBadgeSampleRate(192_000))
    }

    @Test
    fun mergeStreamBadgeWithMeasurementPreservesHifiAndStripsServerNames() {
        assertEquals(
            "FLAC · 880 kbps · 16-bit/44.1kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "FLAC · WebDAV",
                measuredBadge = "FLAC · 880 kbps · 16-bit/44.1kHz",
                sourceLabel = "WebDAV",
            ),
        )
        assertEquals(
            "FLAC · 2480 kbps · 24-bit/96kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "FLAC · 980kbps",
                measuredBadge = "FLAC · 2480 kbps · 24-bit/96kHz",
                sourceLabel = "Jellyfin",
            ),
        )
        assertEquals(
            "FLAC · 720 kbps · 16-bit/44.1kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "Subsonic",
                measuredBadge = "FLAC · 720 kbps · 16-bit/44.1kHz",
                sourceLabel = "Subsonic",
            ),
        )
        assertEquals(
            "FLAC · 887 kbps · 16-bit/44.1kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "FLAC · SMB",
                measuredBadge = "FLAC · 887 kbps · 16-bit/44.1kHz",
                sourceLabel = "SMB",
            ),
        )
        assertEquals(
            "OPUS · 251 kbps · Extreme",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "OPUS 256kbps · Extreme",
                measuredBadge = "OPUS · 251 kbps",
                sourceLabel = "YouTube Music",
            ),
        )
        assertEquals(
            "OPUS · 256 kbps · Hi-Fi · Cached",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "OPUS 256kbps · Hi-Fi · Cached",
                measuredBadge = "OPUS · 256 kbps",
                sourceLabel = "YouTube Music",
            ),
        )
        assertEquals(
            "AAC · 320 kbps · JioSaavn",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "AAC 320kbps · JioSaavn",
                measuredBadge = "AAC · 320 kbps",
                sourceLabel = "Audio Stream",
            ),
        )
        assertEquals(
            "Offline · FLAC · 720 kbps · 16-bit/44.1kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "Offline · FLAC",
                measuredBadge = "FLAC · 720 kbps · 16-bit/44.1kHz",
                sourceLabel = "Offline Download",
            ),
        )
    }

    @Test
    fun mergeStreamBadgeWithMeasurementPreservesExistingBitrateWhenMeasuredBitrateIsPending() {
        assertEquals(
            "FLAC · 980kbps",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "FLAC · 980kbps · Jellyfin",
                measuredBadge = "FLAC · 16-bit/44.1kHz",
                sourceLabel = "Jellyfin",
            ),
        )
        assertEquals(
            "FLAC · 880 kbps · 16-bit/44.1kHz",
            mergeStreamBadgeWithMeasurement(
                initialBadge = "FLAC · 880 kbps · 16-bit/44.1kHz · Jellyfin",
                measuredBadge = "FLAC · 880 kbps · 16-bit/44.1kHz",
                sourceLabel = "Jellyfin",
            ),
        )
    }
}
