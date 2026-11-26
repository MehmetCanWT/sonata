package dev.sonata.android.playback

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin

/**
 * Real-time Media3 [BaseAudioProcessor] providing:
 * 1. A 5-band cascaded peaking EQ (`60`, `230`, `910`, `3600`, `14000` Hz, `Q = 1.0`)
 *    with per-channel biquad delay lines and automatic negative headroom trim.
 * 2. Separate, smoothed DSP gain stages for sleep-timer fade-out and crossfade ramps
 *    that never mutate `player.volume`.
 *
 * Supports both [C.ENCODING_PCM_16BIT] and [C.ENCODING_PCM_FLOAT] streams.
 */
@OptIn(UnstableApi::class)
class SonataDspAudioProcessor : BaseAudioProcessor() {

    private var eqEnabled: Boolean = false
    private var eqBandsDb: FloatArray = FloatArray(BAND_COUNT)
    private var coefficientsDirty: Boolean = true

    @Volatile
    private var targetSleepFadeMultiplier: Float = 1.0f

    @Volatile
    private var targetCrossfadeMultiplier: Float = 1.0f

    private val paramLock = Any()

    // Audio-thread-only format & filter state.
    private var encoding: Int = C.ENCODING_INVALID
    private var sampleRate: Int = 0
    private var channelCount: Int = 0
    private var wasEqActive: Boolean = false
    private var currentGain: Double = 1.0

    private val b0 = DoubleArray(BAND_COUNT) { 1.0 }
    private val b1 = DoubleArray(BAND_COUNT)
    private val b2 = DoubleArray(BAND_COUNT)
    private val a1 = DoubleArray(BAND_COUNT)
    private val a2 = DoubleArray(BAND_COUNT)
    private var headroomTrim: Double = 1.0

    // Flattened [band * channelCount + ch] delay lines allocated only on channelCount change.
    private var x1 = DoubleArray(0)
    private var x2 = DoubleArray(0)
    private var y1 = DoubleArray(0)
    private var y2 = DoubleArray(0)

    fun updateEqualizer(enabled: Boolean, bandsDb: List<Float>) {
        val sanitized = FloatArray(BAND_COUNT) { index ->
            val raw = bandsDb.getOrElse(index) { 0f }
            if (raw.isFinite()) raw.coerceIn(MIN_EQ_DB, MAX_EQ_DB) else 0f
        }
        synchronized(paramLock) {
            eqEnabled = enabled
            eqBandsDb = sanitized
            coefficientsDirty = true
        }
    }

    fun setSleepFadeMultiplier(multiplier: Float) {
        targetSleepFadeMultiplier = if (multiplier.isFinite()) {
            multiplier.coerceIn(0f, 1f)
        } else {
            1f
        }
    }

    fun setCrossfadeMultiplier(multiplier: Float) {
        targetCrossfadeMultiplier = if (multiplier.isFinite()) {
            multiplier.coerceIn(0f, 1f)
        } else {
            1f
        }
    }

    override fun onConfigure(
        inputAudioFormat: AudioProcessor.AudioFormat,
    ): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT &&
            inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT
        ) {
            return AudioProcessor.AudioFormat.NOT_SET
        }
        if (inputAudioFormat.sampleRate <= 0 || inputAudioFormat.channelCount <= 0) {
            return AudioProcessor.AudioFormat.NOT_SET
        }
        encoding = inputAudioFormat.encoding
        sampleRate = inputAudioFormat.sampleRate
        channelCount = inputAudioFormat.channelCount
        ensureChannelState(channelCount)
        clearFilterState()
        synchronized(paramLock) {
            coefficientsDirty = true
        }
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val remaining = inputBuffer.remaining()
        if (remaining == 0) return

        val activeEncoding = encoding
        val channels = channelCount
        val rate = sampleRate
        if ((activeEncoding != C.ENCODING_PCM_16BIT && activeEncoding != C.ENCODING_PCM_FLOAT) ||
            channels <= 0 ||
            rate <= 0
        ) {
            val output = replaceOutputBuffer(remaining)
            copyBuffer(inputBuffer, output, remaining)
            return
        }

        val eqOn: Boolean
        val bandsToRecompute: FloatArray?
        synchronized(paramLock) {
            eqOn = eqEnabled
            if (eqOn && coefficientsDirty) {
                bandsToRecompute = eqBandsDb.copyOf()
                coefficientsDirty = false
            } else {
                bandsToRecompute = null
            }
        }
        if (eqOn != wasEqActive) {
            // Clear delay lines on enable/disable transition to prevent residual filter thumps.
            clearFilterState()
            wasEqActive = eqOn
        }
        if (bandsToRecompute != null) {
            recomputeCoefficients(rate, bandsToRecompute)
        }

        val targetGain = (targetSleepFadeMultiplier * targetCrossfadeMultiplier)
            .coerceIn(0f, 1f)
            .toDouble()

        if (!eqOn && targetGain >= 1.0 && currentGain >= 1.0) {
            val output = replaceOutputBuffer(remaining)
            copyBuffer(inputBuffer, output, remaining)
            return
        }

        val output = replaceOutputBuffer(remaining)
        inputBuffer.order(ByteOrder.nativeOrder())
        output.order(ByteOrder.nativeOrder())

        val slewStep = 1.0 / (rate * GAIN_SLEW_SECONDS).coerceAtLeast(1.0)
        val trim = if (eqOn) headroomTrim else 1.0

        when (activeEncoding) {
            C.ENCODING_PCM_16BIT -> {
                val bytesPerFrame = channels * 2
                while (inputBuffer.remaining() >= bytesPerFrame) {
                    val g = stepGain(targetGain, slewStep) * trim
                    for (ch in 0 until channels) {
                        var sample = inputBuffer.short.toDouble()
                        if (eqOn) {
                            sample = processChannelBiquads(ch, channels, sample)
                        }
                        val outVal = (sample * g).let { scaled ->
                            if (scaled.isFinite()) {
                                scaled.roundToInt()
                                    .coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt())
                            } else {
                                0
                            }
                        }
                        output.putShort(outVal.toShort())
                    }
                }
            }
            C.ENCODING_PCM_FLOAT -> {
                val bytesPerFrame = channels * 4
                while (inputBuffer.remaining() >= bytesPerFrame) {
                    val g = stepGain(targetGain, slewStep) * trim
                    for (ch in 0 until channels) {
                        var sample = inputBuffer.float.toDouble()
                        if (eqOn) {
                            sample = processChannelBiquads(ch, channels, sample)
                        }
                        val outVal = (sample * g).toFloat().let { scaled ->
                            if (scaled.isFinite()) scaled.coerceIn(-1.0f, 1.0f) else 0f
                        }
                        output.putFloat(outVal)
                    }
                }
            }
        }

        while (inputBuffer.hasRemaining()) {
            output.put(inputBuffer.get())
        }
        output.flip()
    }

    override fun onFlush() {
        // Seek or format change: clear filter history only, keep user EQ & fade settings intact.
        clearFilterState()
        currentGain = (targetSleepFadeMultiplier * targetCrossfadeMultiplier)
            .coerceIn(0f, 1f)
            .toDouble()
    }

    override fun onReset() {
        encoding = C.ENCODING_INVALID
        sampleRate = 0
        channelCount = 0
        wasEqActive = false
        clearFilterState()
        currentGain = (targetSleepFadeMultiplier * targetCrossfadeMultiplier)
            .coerceIn(0f, 1f)
            .toDouble()
        synchronized(paramLock) {
            coefficientsDirty = true
        }
    }

    private fun ensureChannelState(channels: Int) {
        val required = BAND_COUNT * channels
        if (x1.size != required) {
            x1 = DoubleArray(required)
            x2 = DoubleArray(required)
            y1 = DoubleArray(required)
            y2 = DoubleArray(required)
        }
    }

    private fun clearFilterState() {
        x1.fill(0.0)
        x2.fill(0.0)
        y1.fill(0.0)
        y2.fill(0.0)
    }

    private fun recomputeCoefficients(rate: Int, bandsSnapshot: FloatArray) {
        var maxPositiveDb = 0f
        val nyquistLimit = (rate / 2.0) - 1.0

        for (band in 0 until BAND_COUNT) {
            val gainDb = bandsSnapshot[band]
            if (gainDb > maxPositiveDb) {
                maxPositiveDb = gainDb
            }
            val rawFreq = BAND_FREQUENCIES_HZ[band].toDouble()
            // Keep fixed stage count by substituting identity coefficients when Nyquist is exceeded.
            if (nyquistLimit < 20.0 || rawFreq >= rate * 0.495 || abs(gainDb) < 0.01f) {
                b0[band] = 1.0
                b1[band] = 0.0
                b2[band] = 0.0
                a1[band] = 0.0
                a2[band] = 0.0
                continue
            }

            val fc = rawFreq.coerceIn(20.0, nyquistLimit)
            val w0 = 2.0 * Math.PI * fc / rate.toDouble()
            val cosW0 = cos(w0)
            val sinW0 = sin(w0)
            val alpha = sinW0 / (2.0 * Q_FACTOR)
            val amp = 10.0.pow(gainDb / 40.0)

            val b0Raw = 1.0 + alpha * amp
            val b1Raw = -2.0 * cosW0
            val b2Raw = 1.0 - alpha * amp
            val a0Raw = 1.0 + alpha / amp
            val a1Raw = -2.0 * cosW0
            val a2Raw = 1.0 - alpha / amp

            val invA0 = 1.0 / a0Raw
            b0[band] = b0Raw * invA0
            b1[band] = b1Raw * invA0
            b2[band] = b2Raw * invA0
            a1[band] = a1Raw * invA0
            a2[band] = a2Raw * invA0
        }

        headroomTrim = if (maxPositiveDb > 0f) {
            10.0.pow(-maxPositiveDb.toDouble() / 20.0)
        } else {
            1.0
        }
    }

    private fun processChannelBiquads(channel: Int, channels: Int, input: Double): Double {
        var sample = input
        for (band in 0 until BAND_COUNT) {
            val idx = band * channels + channel
            val x1Prev = x1[idx]
            val x2Prev = x2[idx]
            val y1Prev = y1[idx]
            val y2Prev = y2[idx]

            val y = b0[band] * sample +
                b1[band] * x1Prev +
                b2[band] * x2Prev -
                a1[band] * y1Prev -
                a2[band] * y2Prev
            val safeY = if (y.isFinite()) y else 0.0

            x2[idx] = x1Prev
            x1[idx] = sample
            y2[idx] = y1Prev
            y1[idx] = safeY
            sample = safeY
        }
        return sample
    }

    private fun stepGain(target: Double, slewStep: Double): Double {
        val diff = target - currentGain
        if (abs(diff) <= slewStep) {
            currentGain = target
        } else if (diff > 0.0) {
            currentGain += slewStep
        } else {
            currentGain -= slewStep
        }
        return currentGain
    }

    private fun copyBuffer(src: ByteBuffer, dst: ByteBuffer, size: Int) {
        if (src === dst) {
            dst.position(0)
            dst.limit(size)
            return
        }
        val pos = src.position()
        for (i in 0 until size) {
            dst.put(src.get(pos + i))
        }
        src.position(pos + size)
        dst.flip()
    }

    companion object {
        const val BAND_COUNT = 5
        val BAND_FREQUENCIES_HZ = floatArrayOf(60f, 230f, 910f, 3600f, 14000f)
        const val Q_FACTOR = 1.0
        const val MIN_EQ_DB = -12f
        const val MAX_EQ_DB = 12f
        private const val GAIN_SLEW_SECONDS = 0.010
    }
}
