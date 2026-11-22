package dev.sonata.android

import android.content.Context
import android.content.Intent
import android.media.audiofx.Equalizer
import android.media.audiofx.LoudnessEnhancer
import android.net.Uri
import android.os.Handler
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MimeTypes
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.TransferListener
import androidx.media3.datasource.cache.CacheDataSink
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import dev.sonata.android.data.AudioPipelineBus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.File
import java.io.InterruptedIOException
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.roundToInt

internal object PlaybackQueueBridge {
    @Volatile
    var canSkipNext: () -> Boolean = { false }

    @Volatile
    var canSkipPrevious: () -> Boolean = { false }

    @Volatile
    var onSkipNext: () -> Unit = {}

    @Volatile
    var onSkipPrevious: () -> Unit = {}

    @Volatile
    var onPlayFromEmpty: () -> Unit = {}
}

/**
 * Thread-safe bridge for applying live audio processing settings (speed, skip silence,
 * loudness normalization, and 5-band equalizer) onto the active [PlaybackService].
 */
object PlaybackAudioBridge {
    @Volatile
    internal var activeService: PlaybackService? = null

    @Volatile
    private var playbackSpeed: Float = 1.0f

    @Volatile
    private var skipSilenceEnabled: Boolean = false

    @Volatile
    private var loudnessNormEnabled: Boolean = false

    @Volatile
    private var eqEnabled: Boolean = false

    @Volatile
    private var eqBandsDb: List<Float> = listOf(0f, 0f, 0f, 0f, 0f)

    fun applyPlaybackSpeed(speed: Float) {
        val clamped = speed.coerceIn(0.5f, 2.0f)
        playbackSpeed = clamped
        AudioPipelineBus.update { it.copy(playbackSpeed = clamped) }
        activeService?.applyPlaybackSpeedInternal(clamped)
    }

    fun applySkipSilence(enabled: Boolean) {
        skipSilenceEnabled = enabled
        AudioPipelineBus.update { it.copy(skipSilenceActive = enabled) }
        activeService?.applySkipSilenceInternal(enabled)
    }

    fun applyLoudnessNormalization(enabled: Boolean) {
        loudnessNormEnabled = enabled
        AudioPipelineBus.update { it.copy(loudnessActive = enabled) }
        activeService?.applyLoudnessInternal(enabled)
    }

    fun applyEqualizer(enabled: Boolean, bandsDb: List<Float>) {
        val sanitized = List(5) { idx -> bandsDb.getOrElse(idx) { 0f }.coerceIn(-6f, 6f) }
        eqEnabled = enabled
        eqBandsDb = sanitized
        AudioPipelineBus.update { it.copy(eqActive = enabled) }
        activeService?.applyEqualizerInternal(enabled, sanitized)
    }

    internal fun syncAllTo(service: PlaybackService) {
        AudioPipelineBus.update { current ->
            val next = current.copy(
                playbackSpeed = playbackSpeed,
                skipSilenceActive = skipSilenceEnabled,
                loudnessActive = loudnessNormEnabled,
                eqActive = eqEnabled,
            )
            if (next == current) current else next
        }
        if (playbackSpeed != 1.0f) {
            service.applyPlaybackSpeedInternal(playbackSpeed)
        }
        if (skipSilenceEnabled) {
            service.applySkipSilenceInternal(true)
        }
        if (loudnessNormEnabled) {
            service.applyLoudnessInternal(true)
        }
        if (eqEnabled) {
            service.applyEqualizerInternal(true, eqBandsDb)
        }
    }
}

class PlaybackService : MediaSessionService() {
    private var mediaSession: MediaSession? = null
    private var exoPlayerInstance: ExoPlayer? = null
    private var playerHandler: Handler? = null
    private var loudnessEnhancer: LoudnessEnhancer? = null
    private var equalizer: Equalizer? = null
    private var boundAudioSessionId: Int = C.AUDIO_SESSION_ID_UNSET
    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    @Volatile
    private var lastCacheCheckKey: String? = null
    @Volatile
    private var lastCacheCheckResult: Boolean = false
    @Volatile
    private var isServiceDestroyed: Boolean = false

    @OptIn(UnstableApi::class)
    override fun onCreate() {
        super.onCreate()
        isServiceDestroyed = false
        val chunkedHttpFactory = ChunkedDataSource.Factory(
            upstream = ClientAwareHttpDataSourceFactory(),
            chunkBytes = GOOGLEVIDEO_CHUNK_BYTES,
        )
        val upstreamFactory = DefaultDataSource.Factory(this, chunkedHttpFactory)
        val simpleCache = getOrCreateCache(this)
        val cacheDataSourceFactory = CacheDataSource.Factory()
            .setCache(simpleCache)
            .setUpstreamDataSourceFactory(upstreamFactory)
            .setCacheWriteDataSinkFactory(
                CacheDataSink.Factory()
                    .setCache(simpleCache)
            )
            .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)

        val loadControl = DefaultLoadControl.Builder()
            .setBufferDurationsMs(
                /* minBufferMs = */ 15_000,
                /* maxBufferMs = */ 50_000,
                /* bufferForPlaybackMs = */ 350,
                /* bufferForPlaybackAfterRebufferMs = */ 800
            )
            .setPrioritizeTimeOverSizeThresholds(true)
            .build()

        val exoPlayer = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(cacheDataSourceFactory))
            .setLoadControl(loadControl)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .setUsage(C.USAGE_MEDIA)
                    .build(),
                true
            )
            .setHandleAudioBecomingNoisy(true)
            .build()
        exoPlayer.repeatMode = Player.REPEAT_MODE_OFF
        exoPlayer.addAnalyticsListener(createPipelineAnalyticsListener(exoPlayer, simpleCache))

        exoPlayerInstance = exoPlayer
        playerHandler = Handler(exoPlayer.applicationLooper)
        PlaybackAudioBridge.activeService = this
        PlaybackAudioBridge.syncAllTo(this)

        val sessionPlayer = QueueAwarePlayer(exoPlayer)
        mediaSession = MediaSession.Builder(this, sessionPlayer).build()
    }

    @OptIn(UnstableApi::class)
    private fun createPipelineAnalyticsListener(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
    ): AnalyticsListener = object : AnalyticsListener {
        override fun onAudioInputFormatChanged(
            eventTime: AnalyticsListener.EventTime,
            format: Format,
            decoderReuseEvaluation: DecoderReuseEvaluation?,
        ) {
            publishFormatTelemetry(exoPlayer, simpleCache, format)
        }

        override fun onTracksChanged(
            eventTime: AnalyticsListener.EventTime,
            tracks: Tracks,
        ) {
            val activeFormat = tracks.groups
                .asSequence()
                .filter { it.type == C.TRACK_TYPE_AUDIO && it.isSelected }
                .mapNotNull { group ->
                    (0 until group.length)
                        .firstOrNull { group.isTrackSelected(it) }
                        ?.let { group.getTrackFormat(it) }
                }
                .firstOrNull()
            if (activeFormat != null) {
                publishFormatTelemetry(exoPlayer, simpleCache, activeFormat)
            } else {
                publishSourceOnly(exoPlayer, simpleCache)
            }
        }

        override fun onIsPlayingChanged(
            eventTime: AnalyticsListener.EventTime,
            isPlaying: Boolean,
        ) {
            publishSourceOnly(exoPlayer, simpleCache)
            dev.sonata.android.widget.SonataWidgetUpdater.pushPlaybackState(
                context = this@PlaybackService,
                isPlaying = isPlaying,
                positionMs = exoPlayer.currentPosition.coerceAtLeast(0L),
                durationMs = exoPlayer.duration.coerceAtLeast(0L),
            )
        }

        override fun onAudioSessionIdChanged(
            eventTime: AnalyticsListener.EventTime,
            audioSessionId: Int,
        ) {
            if (audioSessionId != boundAudioSessionId && audioSessionId != C.AUDIO_SESSION_ID_UNSET && audioSessionId > 0) {
                releaseAudioEffects()
                PlaybackAudioBridge.syncAllTo(this@PlaybackService)
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun queryCacheStatusAsync(
        simpleCache: SimpleCache,
        cacheKey: String?,
        isOffline: Boolean,
    ): Boolean {
        if (isOffline || cacheKey.isNullOrBlank()) return false
        if (cacheKey == lastCacheCheckKey) return lastCacheCheckResult
        serviceScope.launch {
            val cached = runCatching {
                simpleCache.getCachedBytes(cacheKey, 0L, 1L) > 0L
            }.getOrDefault(false)
            lastCacheCheckKey = cacheKey
            lastCacheCheckResult = cached
            AudioPipelineBus.update { current ->
                if (current.fromOfflineDownload || current.fromCache == cached) {
                    current
                } else {
                    current.copy(fromCache = cached)
                }
            }
        }
        return false
    }

    @OptIn(UnstableApi::class)
    private fun publishFormatTelemetry(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
        format: Format,
    ) {
        val uriString = exoPlayer.currentMediaItem?.localConfiguration?.uri?.toString().orEmpty()
        val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
        val cacheKey = exoPlayer.currentMediaItem?.localConfiguration?.customCacheKey
            ?: uriString.takeIf { it.isNotBlank() }
        val isCached = queryCacheStatusAsync(simpleCache, cacheKey, isOffline)
        val sourceLabel = when {
            isOffline -> "Offline Download"
            uriString.contains("googlevideo.com", ignoreCase = true) -> "YouTube Music"
            uriString.isNotBlank() -> "Audio Stream"
            else -> "Idle"
        }
        val bitrate = when {
            format.averageBitrate > 0 -> format.averageBitrate / 1000
            format.bitrate > 0 -> format.bitrate / 1000
            else -> 0
        }
        val codecLabel = formatCodecLabel(format)
        AudioPipelineBus.update { current ->
            val next = current.copy(
                sourceLabel = sourceLabel,
                codec = codecLabel,
                bitrateKbps = bitrate,
                sampleRateHz = format.sampleRate.takeIf { it > 0 } ?: current.sampleRateHz,
                channelCount = format.channelCount.takeIf { it > 0 } ?: current.channelCount,
                fromCache = isCached,
                fromOfflineDownload = isOffline,
            )
            if (next == current) current else next
        }
    }

    @OptIn(UnstableApi::class)
    private fun publishSourceOnly(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
    ) {
        val uriString = exoPlayer.currentMediaItem?.localConfiguration?.uri?.toString().orEmpty()
        val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
        val cacheKey = exoPlayer.currentMediaItem?.localConfiguration?.customCacheKey
            ?: uriString.takeIf { it.isNotBlank() }
        val isCached = queryCacheStatusAsync(simpleCache, cacheKey, isOffline)
        val sourceLabel = when {
            isOffline -> "Offline Download"
            uriString.contains("googlevideo.com", ignoreCase = true) -> "YouTube Music"
            uriString.isNotBlank() -> "Audio Stream"
            else -> "Idle"
        }
        AudioPipelineBus.update { current ->
            val next = current.copy(
                sourceLabel = sourceLabel,
                fromCache = isCached,
                fromOfflineDownload = isOffline,
            )
            if (next == current) current else next
        }
    }

    private fun formatCodecLabel(format: Format): String {
        val mime = format.sampleMimeType.orEmpty()
        val codecs = format.codecs.orEmpty().lowercase(Locale.ROOT)
        return when {
            mime == MimeTypes.AUDIO_OPUS || "opus" in codecs -> "Opus"
            mime == MimeTypes.AUDIO_FLAC || "flac" in codecs -> "FLAC"
            mime == MimeTypes.AUDIO_ALAC || "alac" in codecs -> "ALAC"
            mime == MimeTypes.AUDIO_AAC || "mp4a" in codecs -> "AAC"
            mime == MimeTypes.AUDIO_MPEG -> "MP3"
            mime == MimeTypes.AUDIO_VORBIS -> "Vorbis"
            mime.startsWith("audio/") -> mime.removePrefix("audio/").uppercase(Locale.ROOT)
            codecs.isNotBlank() -> codecs.uppercase(Locale.ROOT)
            else -> "PCM"
        }
    }

    internal fun togglePlayPauseFromWidget(): Boolean {
        val player = exoPlayerInstance ?: return false
        if (player.mediaItemCount <= 0) return false
        if (player.isPlaying) {
            player.pause()
        } else {
            player.play()
        }
        return true
    }

    internal fun applyPlaybackSpeedInternal(speed: Float) {
        val player = exoPlayerInstance ?: return
        postOnPlayer {
            runCatching { player.setPlaybackSpeed(speed.coerceIn(0.5f, 2.0f)) }
        }
    }

    @OptIn(UnstableApi::class)
    internal fun applySkipSilenceInternal(enabled: Boolean) {
        val player = exoPlayerInstance ?: return
        postOnPlayer {
            runCatching { player.skipSilenceEnabled = enabled }
        }
    }

    @OptIn(UnstableApi::class)
    internal fun applyLoudnessInternal(enabled: Boolean) {
        val player = exoPlayerInstance ?: return
        postOnPlayer {
            runCatching {
                if (!enabled) {
                    runCatching { loudnessEnhancer?.enabled = false }
                    runCatching { loudnessEnhancer?.release() }
                    loudnessEnhancer = null
                    return@runCatching
                }
                val sessionId = player.audioSessionId
                if (sessionId == C.AUDIO_SESSION_ID_UNSET || sessionId <= 0) return@runCatching
                if (boundAudioSessionId != sessionId) {
                    releaseAudioEffects()
                    boundAudioSessionId = sessionId
                }
                val enhancer = loudnessEnhancer ?: LoudnessEnhancer(sessionId).also {
                    loudnessEnhancer = it
                }
                enhancer.setTargetGain(LOUDNESS_NORM_TARGET_MB)
                enhancer.enabled = true
            }.onFailure { err ->
                Log.w(TAG, "LoudnessEnhancer update failed: ${err.message}")
            }
        }
    }

    @OptIn(UnstableApi::class)
    internal fun applyEqualizerInternal(enabled: Boolean, bandsDb: List<Float>) {
        val player = exoPlayerInstance ?: return
        postOnPlayer {
            runCatching {
                if (!enabled) {
                    runCatching { equalizer?.enabled = false }
                    runCatching { equalizer?.release() }
                    equalizer = null
                    return@runCatching
                }
                val sessionId = player.audioSessionId
                if (sessionId == C.AUDIO_SESSION_ID_UNSET || sessionId <= 0) return@runCatching
                if (boundAudioSessionId != sessionId) {
                    releaseAudioEffects()
                    boundAudioSessionId = sessionId
                }
                val eq = equalizer ?: Equalizer(0, sessionId).also { equalizer = it }
                val bandCount = eq.numberOfBands.toInt()
                val range = eq.bandLevelRange
                val minMb = range.getOrNull(0)?.toInt() ?: -1500
                val maxMb = range.getOrNull(1)?.toInt() ?: 1500
                for (band in 0 until bandCount) {
                    val uiIdx = if (bandCount <= 1) {
                        0
                    } else {
                        ((band.toFloat() / (bandCount - 1)) * (bandsDb.size - 1))
                            .roundToInt()
                            .coerceIn(0, bandsDb.lastIndex)
                    }
                    val targetMb = (bandsDb[uiIdx] * 100f).roundToInt().coerceIn(minMb, maxMb)
                    eq.setBandLevel(band.toShort(), targetMb.toShort())
                }
                eq.enabled = true
            }.onFailure { err ->
                Log.w(TAG, "Equalizer update failed: ${err.message}")
            }
        }
    }

    private fun postOnPlayer(block: () -> Unit) {
        if (isServiceDestroyed) return
        val handler = playerHandler ?: return
        if (handler.looper.isCurrentThread) {
            if (!isServiceDestroyed && exoPlayerInstance != null) {
                runCatching { block() }
            }
        } else {
            handler.post {
                if (!isServiceDestroyed && exoPlayerInstance != null) {
                    runCatching { block() }
                }
            }
        }
    }

    private fun releaseAudioEffects() {
        runCatching { loudnessEnhancer?.enabled = false }
        runCatching { loudnessEnhancer?.release() }
        loudnessEnhancer = null
        runCatching { equalizer?.enabled = false }
        runCatching { equalizer?.release() }
        equalizer = null
        boundAudioSessionId = C.AUDIO_SESSION_ID_UNSET
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? {
        return mediaSession
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = mediaSession?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        isServiceDestroyed = true
        serviceScope.cancel()
        if (PlaybackAudioBridge.activeService === this) {
            PlaybackAudioBridge.activeService = null
        }
        playerHandler?.removeCallbacksAndMessages(null)
        playerHandler = null
        exoPlayerInstance = null
        releaseAudioEffects()
        mediaSession?.run {
            runCatching { player.release() }
            runCatching { release() }
        }
        mediaSession = null
        super.onDestroy()
    }

    @UnstableApi
    private class QueueAwarePlayer(delegate: ExoPlayer) : ForwardingPlayer(delegate) {
        override fun getAvailableCommands(): Player.Commands {
            val builder = super.getAvailableCommands().buildUpon()
            if (mediaItemCount > 0 || PlaybackQueueBridge.canSkipNext()) {
                builder.add(COMMAND_SEEK_TO_NEXT)
                builder.add(COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
            }
            if (mediaItemCount > 0 || PlaybackQueueBridge.canSkipPrevious()) {
                builder.add(COMMAND_SEEK_TO_PREVIOUS)
                builder.add(COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
            }
            return builder.build()
        }

        override fun isCommandAvailable(command: Int): Boolean {
            return when (command) {
                COMMAND_SEEK_TO_NEXT,
                COMMAND_SEEK_TO_NEXT_MEDIA_ITEM ->
                    super.isCommandAvailable(command) || PlaybackQueueBridge.canSkipNext()
                COMMAND_SEEK_TO_PREVIOUS,
                COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM ->
                    super.isCommandAvailable(command) || PlaybackQueueBridge.canSkipPrevious()
                else -> super.isCommandAvailable(command)
            }
        }

        override fun hasNextMediaItem(): Boolean =
            super.hasNextMediaItem() || PlaybackQueueBridge.canSkipNext()

        override fun hasPreviousMediaItem(): Boolean =
            super.hasPreviousMediaItem() || PlaybackQueueBridge.canSkipPrevious()

        override fun seekToNext() {
            if (super.hasNextMediaItem()) {
                super.seekToNext()
            } else {
                PlaybackQueueBridge.onSkipNext()
            }
        }

        override fun seekToNextMediaItem() {
            if (super.hasNextMediaItem()) {
                super.seekToNextMediaItem()
            } else {
                PlaybackQueueBridge.onSkipNext()
            }
        }

        override fun seekToPrevious() {
            if (super.hasPreviousMediaItem()) {
                super.seekToPrevious()
            } else {
                PlaybackQueueBridge.onSkipPrevious()
            }
        }

        override fun seekToPreviousMediaItem() {
            if (super.hasPreviousMediaItem()) {
                super.seekToPreviousMediaItem()
            } else {
                PlaybackQueueBridge.onSkipPrevious()
            }
        }

        override fun play() {
            if (mediaItemCount == 0) {
                PlaybackQueueBridge.onPlayFromEmpty()
            } else {
                super.play()
            }
        }
    }

    @UnstableApi
    private class ClientAwareHttpDataSourceFactory : DataSource.Factory {
        override fun createDataSource(): DataSource = ClientAwareHttpDataSource()
    }

    @UnstableApi
    private class ClientAwareHttpDataSource : DataSource {
        private val transferListeners = mutableListOf<TransferListener>()
        private var delegate: DefaultHttpDataSource? = null

        override fun addTransferListener(transferListener: TransferListener) {
            transferListeners += transferListener
            delegate?.addTransferListener(transferListener)
        }

        override fun open(dataSpec: DataSpec): Long {
            val profile = resolveHttpProfile(dataSpec.uri)
            val factory = httpFactoriesByUa.getOrPut(profile.userAgent) {
                DefaultHttpDataSource.Factory()
                    .setAllowCrossProtocolRedirects(true)
                    .setConnectTimeoutMs(8_000)
                    .setReadTimeoutMs(15_000)
                    .setUserAgent(profile.userAgent)
            }

            val httpSource = factory.createDataSource()
            transferListeners.forEach { httpSource.addTransferListener(it) }
            delegate = httpSource

            val resolvedSpec = if (profile.isGooglevideo) {
                val filteredHeaders = dataSpec.httpRequestHeaders
                    .filterKeys { key ->
                        !key.equals("Origin", ignoreCase = true) &&
                            !key.equals("Referer", ignoreCase = true) &&
                            !key.equals("User-Agent", ignoreCase = true)
                    }
                    .toMutableMap()
                profile.origin?.let { filteredHeaders["Origin"] = it }
                profile.referer?.let { filteredHeaders["Referer"] = it }
                dataSpec.buildUpon()
                    .setHttpRequestHeaders(filteredHeaders)
                    .build()
            } else {
                dataSpec
            }
            return httpSource.open(resolvedSpec)
        }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
            return delegate?.read(buffer, offset, length) ?: C.RESULT_END_OF_INPUT
        }

        override fun getUri(): Uri? = delegate?.uri

        override fun getResponseHeaders(): Map<String, List<String>> =
            delegate?.responseHeaders ?: emptyMap()

        override fun close() {
            try {
                delegate?.close()
            } finally {
                delegate = null
            }
        }

        private data class HttpProfile(
            val isGooglevideo: Boolean,
            val userAgent: String,
            val origin: String? = null,
            val referer: String? = null,
        )

        private fun resolveHttpProfile(uri: Uri): HttpProfile {
            val host = uri.host?.lowercase(Locale.ROOT).orEmpty()
            if (!host.endsWith("googlevideo.com")) {
                return HttpProfile(
                    isGooglevideo = false,
                    userAgent = DEFAULT_MOBILE_USER_AGENT,
                )
            }
            val client = uri.getQueryParameter("c")?.uppercase(Locale.ROOT).orEmpty()
            val cver = uri.getQueryParameter("cver")
            return when {
                client == "VISIONOS" -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = VISIONOS_USER_AGENT,
                )
                client.startsWith("IOS") -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "com.google.ios.youtube/${cver ?: "20.10.4"} " +
                        "(iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
                )
                client == "ANDROID_VR" -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "com.google.android.apps.youtube.vr.oculus/${cver ?: "1.65.10"} " +
                        "(Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
                )
                client == "ANDROID_MUSIC" -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "com.google.android.apps.youtube.music/${cver ?: "8.39.42"} " +
                        "(Linux; U; Android 15; en_US; Pixel 9 Pro; Build/AP4A.250205.002) gzip",
                )
                client.startsWith("ANDROID") -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "com.google.android.youtube/${cver ?: "21.26.364"} " +
                        "(Linux; U; Android 15; en_US; Pixel 9 Pro; Build/AP4A.250205.002; Cronet/132.0.6834.79) gzip",
                )
                client == "WEB_REMIX" -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = WEB_USER_AGENT,
                    origin = "https://music.youtube.com",
                    referer = "https://music.youtube.com/",
                )
                client.startsWith("TVHTML5") -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "Mozilla/5.0 (SMART-TV; Linux; Tizen 2.4.0) AppleWebKit/538.1 (KHTML, like Gecko) Version/2.4.0 TV Safari/538.1",
                    origin = "https://www.youtube.com",
                    referer = "https://www.youtube.com/",
                )
                client.startsWith("WEB") || client == "MWEB" -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = WEB_USER_AGENT,
                    origin = "https://www.youtube.com",
                    referer = "https://www.youtube.com/",
                )
                else -> HttpProfile(
                    isGooglevideo = true,
                    userAgent = "com.google.ios.youtube/${cver ?: "20.10.4"} " +
                        "(iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)",
                )
            }
        }
    }

    /**
     * Fetches progressive `googlevideo.com` streams as bounded 1 MiB ranges rather than a single
     * open-ended read, avoiding server-side rate throttling while keeping non-googlevideo
     * requests as a direct passthrough.
     */
    @UnstableApi
    private class ChunkedDataSource(
        private val upstream: DataSource,
        private val chunkBytes: Long,
    ) : DataSource {
        private var baseSpec: DataSpec? = null
        private var position = 0L
        private var bytesRemaining = 0L
        private var chunkRemaining = 0L
        private var chunkOpen = false
        private var passthrough = false
        private var effectiveChunkBytes = chunkBytes

        override fun addTransferListener(transferListener: TransferListener) {
            upstream.addTransferListener(transferListener)
        }

        override fun open(dataSpec: DataSpec): Long {
            baseSpec = dataSpec
            position = dataSpec.position

            val host = dataSpec.uri.host?.lowercase(Locale.ROOT).orEmpty()
            val total = dataSpec.uri.getQueryParameter("clen")?.toLongOrNull()
            if (!host.endsWith("googlevideo.com") || total == null) {
                passthrough = true
                chunkOpen = true
                return upstream.open(dataSpec)
            }

            passthrough = false
            val client = dataSpec.uri.getQueryParameter("c")?.uppercase(Locale.ROOT).orEmpty()
            effectiveChunkBytes = if (client == "ANDROID_VR" || client.startsWith("TVHTML5_SIMPLY")) {
                minOf(chunkBytes, NARROW_RANGE_BYTES)
            } else {
                chunkBytes
            }
            val end = if (dataSpec.length == C.LENGTH_UNSET.toLong()) {
                total
            } else {
                minOf(total, position + dataSpec.length)
            }
            bytesRemaining = (end - position).coerceAtLeast(0L)
            if (bytesRemaining > 0L) {
                openChunk()
            }
            return bytesRemaining
        }

        private fun openChunk() {
            val length = minOf(effectiveChunkBytes, bytesRemaining)
            val spec = requireNotNull(baseSpec).buildUpon()
                .setPosition(position)
                .setLength(length)
                .build()
            var lastError: Exception? = null
            for (attempt in 0..MAX_TRANSIENT_CHUNK_RETRIES) {
                try {
                    upstream.open(spec)
                    chunkRemaining = length
                    chunkOpen = true
                    return
                } catch (e: InterruptedIOException) {
                    throw e
                } catch (e: androidx.media3.datasource.HttpDataSource.InvalidResponseCodeException) {
                    val code = e.responseCode
                    val client = spec.uri.getQueryParameter("c") ?: "unknown"
                    Log.w(TAG, "Chunk $position-${position + length - 1} HTTP $code for $client")
                    if (code !in 500..599 || attempt == MAX_TRANSIENT_CHUNK_RETRIES) {
                        throw e
                    }
                    lastError = e
                } catch (e: Exception) {
                    val client = spec.uri.getQueryParameter("c") ?: "unknown"
                    Log.w(TAG, "Chunk $position-${position + length - 1} attempt ${attempt + 1} failed for $client: ${e.message}")
                    if (attempt == MAX_TRANSIENT_CHUNK_RETRIES) {
                        throw e
                    }
                    lastError = e
                }
                runCatching { upstream.close() }
                try {
                    Thread.sleep(if (attempt == 0) 150L else 400L)
                } catch (ie: InterruptedException) {
                    Thread.currentThread().interrupt()
                    throw InterruptedIOException("Interrupted during chunk retry").apply { initCause(ie) }
                }
            }
            throw (lastError ?: java.io.IOException("Failed to open chunk at $position"))
        }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
            if (passthrough) return upstream.read(buffer, offset, length)
            if (bytesRemaining == 0L) return C.RESULT_END_OF_INPUT

            repeat(MAX_EMPTY_RANGES) {
                if (chunkRemaining == 0L) {
                    closeChunk()
                    openChunk()
                }
                val toRead = minOf(length.toLong(), chunkRemaining).toInt()
                val read = upstream.read(buffer, offset, toRead)
                if (read != C.RESULT_END_OF_INPUT) {
                    position += read
                    chunkRemaining -= read
                    bytesRemaining -= read
                    return read
                }
                chunkRemaining = 0L
            }
            throw java.io.IOException("Stream truncated at $position ($bytesRemaining bytes remaining)")
        }

        private fun closeChunk() {
            if (chunkOpen) {
                try {
                    upstream.close()
                } finally {
                    chunkOpen = false
                }
            }
        }

        override fun getUri(): Uri? = upstream.uri ?: baseSpec?.uri

        override fun getResponseHeaders(): Map<String, List<String>> = upstream.responseHeaders

        override fun close() {
            try {
                closeChunk()
            } finally {
                baseSpec = null
                bytesRemaining = 0L
                chunkRemaining = 0L
            }
        }

        class Factory(
            private val upstream: DataSource.Factory,
            private val chunkBytes: Long,
        ) : DataSource.Factory {
            override fun createDataSource(): DataSource =
                ChunkedDataSource(upstream.createDataSource(), chunkBytes)
        }
    }

    companion object {
        private const val TAG = "SonataPlayback"
        private const val LOUDNESS_NORM_TARGET_MB = 150
        private const val MAX_EMPTY_RANGES = 5
        private const val MAX_TRANSIENT_CHUNK_RETRIES = 2
        private const val GOOGLEVIDEO_CHUNK_BYTES = 1024L * 1024L
        private const val NARROW_RANGE_BYTES = 512L * 1024L
        private const val EXO_CACHE_BYTES = 512L * 1024L * 1024L
        private const val VISIONOS_USER_AGENT =
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 " +
                "(KHTML, like Gecko) Version/26.0 Safari/605.1.15"
        private const val WEB_USER_AGENT =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
                "(KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
        private const val DEFAULT_MOBILE_USER_AGENT =
            "Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36 Chrome/131.0.0.0 Mobile Safari/537.36"

        @OptIn(UnstableApi::class)
        private val httpFactoriesByUa = ConcurrentHashMap<String, DefaultHttpDataSource.Factory>()

        @Volatile
        private var sharedCache: SimpleCache? = null

        @OptIn(UnstableApi::class)
        fun getOrCreateCache(context: Context): SimpleCache {
            sharedCache?.let { return it }
            return synchronized(this) {
                sharedCache ?: SimpleCache(
                    File(context.cacheDir, "exo_media").apply { mkdirs() },
                    LeastRecentlyUsedCacheEvictor(EXO_CACHE_BYTES),
                    StandaloneDatabaseProvider(context.applicationContext),
                ).also { sharedCache = it }
            }
        }

        @OptIn(UnstableApi::class)
        fun evictCacheKey(cacheKey: String) {
            val active = sharedCache ?: return
            runCatching { active.removeResource(cacheKey) }
        }

        @OptIn(UnstableApi::class)
        fun clearMediaCache(context: Context) {
            val active = sharedCache
            if (active != null) {
                active.keys.toList().forEach { key ->
                    runCatching { active.removeResource(key) }
                }
            } else {
                File(context.cacheDir, "exo_media").deleteRecursively()
            }
        }
    }
}
