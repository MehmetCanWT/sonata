package dev.sonata.android

import android.content.Context
import android.content.Intent
import android.media.audiofx.LoudnessEnhancer
import android.net.Uri
import android.os.Handler
import android.provider.OpenableColumns
import android.util.Log
import android.view.TextureView
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.TransferListener
import androidx.media3.datasource.cache.CacheDataSink
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.ContentMetadata
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import androidx.media3.exoplayer.source.MergingMediaSource
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.data.formatMeasuredAudioBadge
import dev.sonata.android.data.normalizeAudioCodecLabel
import dev.sonata.android.data.parseHttpContentLength
import dev.sonata.android.data.parseHttpContentRangeTotal
import dev.sonata.android.data.pcmEncodingToBitDepth
import dev.sonata.android.data.resolveMeasuredBitrateKbps
import dev.sonata.android.playback.EndedStateTransitionGuard
import dev.sonata.android.playback.SonataDspAudioProcessor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import java.io.InterruptedIOException
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.cos
import kotlin.math.sin

internal object PlaybackQueueBridge {
    private val defaultFalse: () -> Boolean = { false }
    private val defaultUnit: () -> Unit = {}

    @Volatile
    var canSkipNext: () -> Boolean = defaultFalse

    @Volatile
    var canSkipPrevious: () -> Boolean = defaultFalse

    @Volatile
    var shouldAutoAdvanceOnEnd: () -> Boolean = defaultFalse

    @Volatile
    var onSkipNext: () -> Unit = defaultUnit

    @Volatile
    var onSkipPrevious: () -> Unit = defaultUnit

    @Volatile
    var onPlayFromEmpty: () -> Unit = defaultUnit

    @Volatile
    var onTrackEnded: () -> Unit = defaultUnit

    fun reset() {
        canSkipNext = defaultFalse
        canSkipPrevious = defaultFalse
        shouldAutoAdvanceOnEnd = defaultFalse
        onSkipNext = defaultUnit
        onSkipPrevious = defaultUnit
        onPlayFromEmpty = defaultUnit
        onTrackEnded = defaultUnit
    }

    fun hasActiveCallbacks(): Boolean {
        return canSkipNext !== defaultFalse ||
            canSkipPrevious !== defaultFalse ||
            shouldAutoAdvanceOnEnd !== defaultFalse ||
            onSkipNext !== defaultUnit ||
            onSkipPrevious !== defaultUnit ||
            onPlayFromEmpty !== defaultUnit ||
            onTrackEnded !== defaultUnit
    }
}

/**
 * Thread-safe bridge for applying live playback settings (speed, skip silence, EQ,
 * loudness normalization, crossfade, and sleep fade gain) onto the active [PlaybackService].
 */
object PlaybackAudioBridge {
    @Volatile
    internal var activeService: PlaybackService? = null

    @Volatile
    private var playbackSpeed: Float = 1.0f

    @Volatile
    private var skipSilenceEnabled: Boolean = false

    @Volatile
    private var eqEnabled: Boolean = false

    @Volatile
    private var eqBandsDb: List<Float> = listOf(0f, 0f, 0f, 0f, 0f)

    @Volatile
    private var loudnessNormEnabled: Boolean = false

    @Volatile
    private var crossfadeEnabled: Boolean = false

    @Volatile
    private var crossfadeSeconds: Int = 4

    @Volatile
    private var sleepFadeMultiplier: Float = 1.0f

    @Volatile
    var stopMusicOnClose: Boolean = false

    private val _videoAspectRatio = MutableStateFlow(16f / 9f)
    val videoAspectRatio: StateFlow<Float> = _videoAspectRatio.asStateFlow()

    internal fun updateVideoAspectRatio(ratio: Float) {
        if (ratio.isFinite() && ratio > 0.1f) {
            _videoAspectRatio.value = ratio
        }
    }

    fun attachVideoTextureView(textureView: TextureView) {
        activeService?.attachVideoTextureViewInternal(textureView)
    }

    fun clearVideoTextureView(textureView: TextureView) {
        activeService?.clearVideoTextureViewInternal(textureView)
    }

    fun playVideoStream(
        videoUrl: String,
        audioUrl: String?,
        mediaItem: MediaItem,
        startPositionMs: Long,
        playWhenReady: Boolean,
    ): Boolean = activeService?.playVideoStreamInternal(
        videoUrl = videoUrl,
        audioUrl = audioUrl,
        mediaItem = mediaItem,
        startPositionMs = startPositionMs,
        playWhenReady = playWhenReady,
    ) ?: false

    fun setVideoModePlayback(active: Boolean) {
        activeService?.setVideoModePlaybackInternal(active)
    }

    internal fun isSkipSilenceEnabled(): Boolean = skipSilenceEnabled

    fun applyPlaybackSpeed(speed: Float) {
        val clamped = if (speed.isFinite()) speed.coerceIn(0.5f, 2.0f) else 1.0f
        playbackSpeed = clamped
        AudioPipelineBus.update { it.copy(playbackSpeed = clamped) }
        activeService?.applyPlaybackSpeedInternal(clamped)
    }

    fun applySkipSilence(enabled: Boolean) {
        skipSilenceEnabled = enabled
        AudioPipelineBus.update { it.copy(skipSilenceActive = enabled) }
        activeService?.applySkipSilenceInternal(enabled)
    }

    fun applyEqualizer(enabled: Boolean, bandsDb: List<Float>) {
        val sanitized = List(SonataDspAudioProcessor.BAND_COUNT) { index ->
            val raw = bandsDb.getOrElse(index) { 0f }
            if (raw.isFinite()) {
                raw.coerceIn(SonataDspAudioProcessor.MIN_EQ_DB, SonataDspAudioProcessor.MAX_EQ_DB)
            } else {
                0f
            }
        }
        eqEnabled = enabled
        eqBandsDb = sanitized
        AudioPipelineBus.update { it.copy(eqActive = enabled) }
        activeService?.applyEqualizerInternal(enabled, sanitized)
    }

    fun applyLoudnessNormalization(enabled: Boolean) {
        loudnessNormEnabled = enabled
        AudioPipelineBus.update { it.copy(loudnessNormActive = enabled) }
        activeService?.applyLoudnessNormalizationInternal(enabled)
    }

    fun applyCrossfade(enabled: Boolean, seconds: Int) {
        val clampedSeconds = seconds.coerceIn(1, 12)
        crossfadeEnabled = enabled
        crossfadeSeconds = clampedSeconds
        AudioPipelineBus.update {
            it.copy(crossfadeSeconds = if (enabled) clampedSeconds else 0)
        }
        activeService?.applyCrossfadeInternal(enabled, clampedSeconds)
    }

    fun applySleepFadeMultiplier(multiplier: Float) {
        val clamped = if (multiplier.isFinite()) multiplier.coerceIn(0f, 1f) else 1f
        sleepFadeMultiplier = clamped
        activeService?.applySleepFadeMultiplierInternal(clamped)
    }

    internal fun clearEndedTransitionSuppression() {
        activeService?.clearEndedTransitionSuppressionInternal()
    }

    private const val MAX_REGISTERED_STREAM_HEADERS = 64
    private val streamHeadersLock = Any()
    private val streamHeadersByUrl = object : LinkedHashMap<String, Map<String, String>>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Map<String, String>>?): Boolean {
            return size > MAX_REGISTERED_STREAM_HEADERS
        }
    }

    fun registerStreamHeaders(url: String, headers: Map<String, String>) {
        val cleanUrl = url.trim()
        if (cleanUrl.isBlank()) return
        val cleanHeaders = headers.entries
            .filter { (k, v) -> k.isNotBlank() && v.isNotBlank() }
            .associate { (k, v) -> k.trim() to v }
        synchronized(streamHeadersLock) {
            if (cleanHeaders.isEmpty()) {
                streamHeadersByUrl.remove(cleanUrl)
            } else {
                streamHeadersByUrl[cleanUrl] = cleanHeaders
            }
        }
    }

    fun headersForStreamUrl(url: String): Map<String, String> {
        val cleanUrl = url.trim()
        if (cleanUrl.isBlank()) return emptyMap()
        synchronized(streamHeadersLock) {
            return streamHeadersByUrl[cleanUrl]?.toMap().orEmpty()
        }
    }

    fun clearStreamHeaders() {
        synchronized(streamHeadersLock) {
            streamHeadersByUrl.clear()
        }
    }

    internal fun syncAllTo(service: PlaybackService) {
        AudioPipelineBus.update { current ->
            val next = current.copy(
                playbackSpeed = playbackSpeed,
                skipSilenceActive = skipSilenceEnabled,
                eqActive = eqEnabled,
                loudnessNormActive = loudnessNormEnabled,
                crossfadeSeconds = if (crossfadeEnabled) crossfadeSeconds else 0,
            )
            if (next == current) current else next
        }
        if (playbackSpeed != 1.0f) {
            service.applyPlaybackSpeedInternal(playbackSpeed)
        }
        if (skipSilenceEnabled) {
            service.applySkipSilenceInternal(true)
        }
        service.applyEqualizerInternal(eqEnabled, eqBandsDb)
        service.applyLoudnessNormalizationInternal(loudnessNormEnabled)
        service.applyCrossfadeInternal(crossfadeEnabled, crossfadeSeconds)
        service.applySleepFadeMultiplierInternal(sleepFadeMultiplier)
    }
}

class PlaybackService : MediaSessionService() {
    private var mediaSession: MediaSession? = null
    private var exoPlayerInstance: ExoPlayer? = null
    private var videoSourceFactory: ProgressiveMediaSource.Factory? = null
    private var attachedTextureView: TextureView? = null
    private var playerHandler: Handler? = null
    private var dspProcessor: SonataDspAudioProcessor? = null
    private var loudnessEnhancer: LoudnessEnhancer? = null
    private var loudnessSessionId: Int = C.AUDIO_SESSION_ID_UNSET

    @Volatile
    private var crossfadeEnabledFlag: Boolean = false

    @Volatile
    private var crossfadeDurationMs: Long = 4_000L

    @Volatile
    private var loudnessEnabledFlag: Boolean = false

    private var crossfadeActiveItemToken: Int = 0
    private var crossfadeFadeOutTriggeredToken: Int = -1
    private var crossfadeAdvanceDispatchedToken: Int = -1
    private var crossfadeFadeOutStartRemainingMs: Long = 0L
    private var pendingCrossfadeFadeIn: Boolean = false
    private var crossfadeFadeInStartMs: Long = 0L
    private var crossfadeFadeInDurationMs: Long = 0L
    private var crossfadeMonitorScheduled: Boolean = false

    private val crossfadeMonitorRunnable = object : Runnable {
        override fun run() {
            crossfadeMonitorScheduled = false
            if (isServiceDestroyed) return
            stepCrossfadeMonitor()
            if (!isServiceDestroyed && shouldRunCrossfadeMonitor()) {
                scheduleCrossfadeMonitor()
            }
        }
    }

    @Volatile
    private var videoModePlaybackActive: Boolean = false

    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val endedTransitionGuard = EndedStateTransitionGuard()
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
        videoSourceFactory = ProgressiveMediaSource.Factory(upstreamFactory)
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

        val processor = SonataDspAudioProcessor()
        dspProcessor = processor
        val renderersFactory = object : DefaultRenderersFactory(this) {
            override fun buildAudioSink(
                context: Context,
                enableFloatOutput: Boolean,
                enableAudioTrackPlaybackParams: Boolean,
            ): AudioSink {
                return DefaultAudioSink.Builder(context)
                    .setEnableFloatOutput(enableFloatOutput)
                    .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                    .setAudioProcessors(arrayOf(processor))
                    .build()
            }
        }

        val exoPlayer = ExoPlayer.Builder(this, renderersFactory)
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
        exoPlayer.addListener(object : Player.Listener {
            override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                crossfadeActiveItemToken += 1
                crossfadeFadeOutTriggeredToken = -1
                crossfadeAdvanceDispatchedToken = -1
                crossfadeFadeOutStartRemainingMs = 0L
                if (!pendingCrossfadeFadeIn || !crossfadeEnabledFlag || mediaItem == null) {
                    pendingCrossfadeFadeIn = false
                    crossfadeFadeInDurationMs = 0L
                    dspProcessor?.setCrossfadeMultiplier(1f)
                }
                resetPipelineForMediaItem(mediaItem)
                syncLoudnessEnhancerForSession(exoPlayer.audioSessionId)
                maybeScheduleCrossfadeMonitor()
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                val pendingAutoAdvance = playbackState == Player.STATE_ENDED &&
                    exoPlayer.repeatMode != Player.REPEAT_MODE_ONE &&
                    PlaybackQueueBridge.shouldAutoAdvanceOnEnd()
                val alreadyDispatchedByCrossfade =
                    crossfadeAdvanceDispatchedToken == crossfadeActiveItemToken
                val suppressed = endedTransitionGuard.onUnderlyingPlaybackStateChanged(
                    rawState = playbackState,
                    hasPendingNextTransition = pendingAutoAdvance,
                )
                if (suppressed && !alreadyDispatchedByCrossfade) {
                    Log.i(TAG, "STATE_ENDED with pending next track; keeping foreground service alive during transition")
                    PlaybackQueueBridge.onTrackEnded()
                }
                if (playbackState == Player.STATE_READY) {
                    refreshFormatTelemetryIfPossible(exoPlayer, simpleCache)
                }
                if (playbackState == Player.STATE_READY && exoPlayer.isPlaying) {
                    maybeStartCrossfadeFadeIn()
                    maybeScheduleCrossfadeMonitor()
                } else if (playbackState == Player.STATE_IDLE || playbackState == Player.STATE_ENDED) {
                    if (playbackState == Player.STATE_IDLE) {
                        resetCrossfadeState()
                    }
                }
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (isPlaying) {
                    maybeStartCrossfadeFadeIn()
                    maybeScheduleCrossfadeMonitor()
                } else if (crossfadeFadeOutTriggeredToken != crossfadeActiveItemToken) {
                    dspProcessor?.setCrossfadeMultiplier(1f)
                }
            }

            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                endedTransitionGuard.onPlayWhenReadyChanged(playWhenReady)
                if (!playWhenReady) {
                    cancelCrossfadeMonitor()
                } else {
                    maybeScheduleCrossfadeMonitor()
                }
            }

            override fun onPlayerError(error: PlaybackException) {
                endedTransitionGuard.clear()
                resetCrossfadeState()
            }

            override fun onVideoSizeChanged(videoSize: VideoSize) {
                if (videoSize.width > 0 && videoSize.height > 0) {
                    val ratio = (videoSize.width.toFloat() * videoSize.pixelWidthHeightRatio) /
                        videoSize.height.toFloat()
                    PlaybackAudioBridge.updateVideoAspectRatio(ratio)
                }
            }
        })

        exoPlayerInstance = exoPlayer
        playerHandler = Handler(exoPlayer.applicationLooper)
        PlaybackAudioBridge.activeService = this
        PlaybackAudioBridge.syncAllTo(this)

        val sessionPlayer = QueueAwarePlayer(exoPlayer, endedTransitionGuard)
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
            val tracksFormat = findSelectedAudioFormat(exoPlayer.currentTracks)
            publishFormatTelemetry(
                exoPlayer = exoPlayer,
                simpleCache = simpleCache,
                tracksFormat = tracksFormat,
                decoderFormat = format,
            )
        }

        override fun onTracksChanged(
            eventTime: AnalyticsListener.EventTime,
            tracks: Tracks,
        ) {
            val activeFormat = findSelectedAudioFormat(tracks)
            val decoderFormat = exoPlayer.audioFormat
            if (activeFormat != null || decoderFormat != null) {
                publishFormatTelemetry(
                    exoPlayer = exoPlayer,
                    simpleCache = simpleCache,
                    tracksFormat = activeFormat,
                    decoderFormat = decoderFormat,
                )
            } else {
                publishSourceOnly(exoPlayer, simpleCache)
            }
        }

        override fun onLoadCompleted(
            eventTime: AnalyticsListener.EventTime,
            loadEventInfo: LoadEventInfo,
            mediaLoadData: MediaLoadData,
        ) {
            if (mediaLoadData.dataType != C.DATA_TYPE_MEDIA) return
            val uriString = exoPlayer.currentMediaItem?.localConfiguration?.uri?.toString()
                ?: loadEventInfo.uri.toString()
            if (uriString.isBlank()) return
            val rangeTotal = parseHttpContentRangeTotal(loadEventInfo.responseHeaders)
            val spec = loadEventInfo.dataSpec
            val isFullRequest = spec.position == 0L &&
                spec.length == C.LENGTH_UNSET.toLong() &&
                !spec.uri.toString().contains("range=", ignoreCase = true)
            val headerLength = if (isFullRequest) {
                parseHttpContentLength(loadEventInfo.responseHeaders)
            } else {
                0L
            }
            val totalBytes = when {
                rangeTotal > 0L -> rangeTotal
                headerLength > 0L -> headerLength
                else -> 0L
            }
            if (totalBytes > 0L) {
                recordMeasuredContentBytes(uriString, totalBytes)
            }
            if (isFullRequest && loadEventInfo.bytesLoaded > 0L) {
                recordCompletedLoadBytes(uriString, loadEventInfo.bytesLoaded)
            }
            if (AudioPipelineBus.info.value.bitrateKbps <= 0) {
                refreshFormatTelemetryIfPossible(exoPlayer, simpleCache)
            }
        }

        override fun onIsPlayingChanged(
            eventTime: AnalyticsListener.EventTime,
            isPlaying: Boolean,
        ) {
            publishSourceOnly(exoPlayer, simpleCache)
            if (isPlaying && AudioPipelineBus.info.value.bitrateKbps <= 0) {
                refreshFormatTelemetryIfPossible(exoPlayer, simpleCache)
            }
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
            if (audioSessionId != C.AUDIO_SESSION_ID_UNSET && audioSessionId > 0) {
                syncLoudnessEnhancerForSession(audioSessionId)
                PlaybackAudioBridge.syncAllTo(this@PlaybackService)
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun findSelectedAudioFormat(tracks: Tracks): Format? {
        return tracks.groups
            .asSequence()
            .filter { it.type == C.TRACK_TYPE_AUDIO && it.isSelected }
            .mapNotNull { group ->
                (0 until group.length)
                    .firstOrNull { group.isTrackSelected(it) }
                    ?.let { group.getTrackFormat(it) }
            }
            .firstOrNull()
    }

    @OptIn(UnstableApi::class)
    private fun resetPipelineForMediaItem(mediaItem: MediaItem?) {
        val nextMediaId = mediaItem?.mediaId.orEmpty()
        val uriString = mediaItem?.localConfiguration?.uri?.toString().orEmpty()
        val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
        val sourceLabel = resolvePipelineSourceLabel(mediaItem)
        AudioPipelineBus.update { current ->
            if (current.mediaId == nextMediaId && current.mediaId.isNotBlank()) {
                current
            } else {
                current.copy(
                    sourceLabel = sourceLabel,
                    codec = "—",
                    bitrateKbps = 0,
                    sampleRateHz = 0,
                    channelCount = 0,
                    fromCache = false,
                    fromOfflineDownload = isOffline,
                    bitDepthBits = 0,
                    mediaId = nextMediaId,
                    measuredBadge = "",
                )
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun refreshFormatTelemetryIfPossible(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
    ) {
        val tracksFormat = findSelectedAudioFormat(exoPlayer.currentTracks)
        val decoderFormat = exoPlayer.audioFormat
        if (tracksFormat != null || decoderFormat != null) {
            publishFormatTelemetry(
                exoPlayer = exoPlayer,
                simpleCache = simpleCache,
                tracksFormat = tracksFormat,
                decoderFormat = decoderFormat,
            )
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
    private fun resolvePipelineSourceLabel(mediaItem: MediaItem?): String =
        Companion.resolvePipelineSourceLabel(
            mediaId = mediaItem?.mediaId.orEmpty(),
            uriString = mediaItem?.localConfiguration?.uri?.toString().orEmpty(),
        )

    @OptIn(UnstableApi::class)
    private fun publishFormatTelemetry(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
        tracksFormat: Format?,
        decoderFormat: Format?,
    ) {
        val mediaItem = exoPlayer.currentMediaItem
        val uri = mediaItem?.localConfiguration?.uri
        val uriString = uri?.toString().orEmpty()
        val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
        val cacheKey = mediaItem?.localConfiguration?.customCacheKey
            ?: uriString.takeIf { it.isNotBlank() }
        val isCached = queryCacheStatusAsync(simpleCache, cacheKey, isOffline)
        val mediaId = mediaItem?.mediaId.orEmpty()
        val sourceLabel = resolvePipelineSourceLabel(mediaItem)

        val mime = tracksFormat?.sampleMimeType ?: decoderFormat?.sampleMimeType
        val codecs = tracksFormat?.codecs ?: decoderFormat?.codecs
        val codecLabel = normalizeAudioCodecLabel(mime, codecs).ifBlank { "PCM" }

        val sampleRateHz = decoderFormat?.sampleRate?.takeIf { it > 0 }
            ?: tracksFormat?.sampleRate?.takeIf { it > 0 }
            ?: 0
        val channelCount = decoderFormat?.channelCount?.takeIf { it > 0 }
            ?: tracksFormat?.channelCount?.takeIf { it > 0 }
            ?: 0
        val bitDepthBits = pcmEncodingToBitDepth(tracksFormat?.pcmEncoding ?: Format.NO_VALUE)
            .takeIf { it > 0 }
            ?: pcmEncodingToBitDepth(decoderFormat?.pcmEncoding ?: Format.NO_VALUE)

        val reportedAvgBps = tracksFormat?.averageBitrate?.takeIf { it > 0 }
            ?: decoderFormat?.averageBitrate?.takeIf { it > 0 }
            ?: Format.NO_VALUE
        val reportedPeakBps = tracksFormat?.bitrate?.takeIf { it > 0 }
            ?: decoderFormat?.bitrate?.takeIf { it > 0 }
            ?: Format.NO_VALUE

        val durationMs = exoPlayer.duration.takeIf { it > 0L && it != C.TIME_UNSET } ?: 0L
        val bufferedFullBytes = if (
            !isOffline &&
            !cacheKey.isNullOrBlank() &&
            durationMs > 0L &&
            exoPlayer.bufferedPosition >= durationMs - 500L
        ) {
            runCatching { simpleCache.getCachedBytes(cacheKey, 0L, Long.MAX_VALUE) }
                .getOrDefault(0L)
                .takeIf { it > 0L } ?: 0L
        } else {
            0L
        }

        val immediateContentBytes = when {
            uriString.isBlank() -> 0L
            else -> lookupMeasuredContentBytes(uriString)
                .takeIf { it > 0L }
                ?: uri?.getQueryParameter("clen")?.toLongOrNull()?.takeIf { it > 0L }
                ?: cacheKey?.let { key ->
                    runCatching { ContentMetadata.getContentLength(simpleCache.getContentMetadata(key)) }
                        .getOrDefault(0L)
                        .takeIf { it > 0L }
                }
                ?: lookupCompletedLoadBytes(uriString).takeIf { it > 0L }
                ?: bufferedFullBytes
        }

        val measuredBitrateKbps = resolveMeasuredBitrateKbps(
            reportedAverageBitrateBps = reportedAvgBps,
            reportedBitrateBps = reportedPeakBps,
            contentBytes = immediateContentBytes,
            durationMs = durationMs,
        )

        AudioPipelineBus.update { current ->
            val sameTrack = current.mediaId.isNotBlank() && current.mediaId == mediaId
            val effectiveCodec = if (codecLabel != "PCM" || !sameTrack || current.codec == "—") {
                codecLabel
            } else {
                current.codec
            }
            val effectiveBitrate = if (measuredBitrateKbps > 0) {
                measuredBitrateKbps
            } else if (sameTrack) {
                current.bitrateKbps
            } else {
                0
            }
            val effectiveSampleRate = if (sampleRateHz > 0) {
                sampleRateHz
            } else if (sameTrack) {
                current.sampleRateHz
            } else {
                0
            }
            val effectiveChannels = if (channelCount > 0) {
                channelCount
            } else if (sameTrack) {
                current.channelCount
            } else {
                0
            }
            val effectiveBitDepth = if (bitDepthBits > 0) {
                bitDepthBits
            } else if (sameTrack) {
                current.bitDepthBits
            } else {
                0
            }
            val badge = formatMeasuredAudioBadge(
                codec = effectiveCodec,
                bitrateKbps = effectiveBitrate,
                sampleRateHz = effectiveSampleRate,
                bitDepthBits = effectiveBitDepth,
            )
            val next = current.copy(
                sourceLabel = sourceLabel,
                codec = effectiveCodec,
                bitrateKbps = effectiveBitrate,
                sampleRateHz = effectiveSampleRate,
                channelCount = effectiveChannels,
                fromCache = isCached,
                fromOfflineDownload = isOffline,
                bitDepthBits = effectiveBitDepth,
                mediaId = mediaId,
                measuredBadge = badge,
            )
            if (next == current) current else next
        }

        // For local files / content URIs with missing container bitrate, measure file size off Main thread.
        if (measuredBitrateKbps <= 0 && isOffline && uri != null && durationMs > 0L) {
            serviceScope.launch {
                val localBytes = resolveLocalContentSizeBytes(uri)
                if (localBytes > 0L) {
                    recordMeasuredContentBytes(uriString, localBytes)
                    val computed = resolveMeasuredBitrateKbps(
                        reportedAverageBitrateBps = Format.NO_VALUE,
                        reportedBitrateBps = Format.NO_VALUE,
                        contentBytes = localBytes,
                        durationMs = durationMs,
                    )
                    if (computed > 0) {
                        AudioPipelineBus.update { current ->
                            if (current.mediaId != mediaId) return@update current
                            val effectiveBitrate = if (current.bitrateKbps > 0) current.bitrateKbps else computed
                            val badge = formatMeasuredAudioBadge(
                                codec = current.codec,
                                bitrateKbps = effectiveBitrate,
                                sampleRateHz = current.sampleRateHz,
                                bitDepthBits = current.bitDepthBits,
                            )
                            val next = current.copy(
                                bitrateKbps = effectiveBitrate,
                                measuredBadge = badge,
                            )
                            if (next == current) current else next
                        }
                    }
                }
            }
        }
    }

    private fun resolveLocalContentSizeBytes(uri: Uri): Long {
        val scheme = uri.scheme?.lowercase(Locale.ROOT)
        if (scheme == null || scheme == "file") {
            val path = uri.path ?: return 0L
            return runCatching { File(path).takeIf { it.isFile }?.length() ?: 0L }.getOrDefault(0L)
        }
        if (scheme == "content") {
            val afdLength = runCatching {
                contentResolver.openAssetFileDescriptor(uri, "r")?.use { it.length } ?: -1L
            }.getOrDefault(-1L)
            if (afdLength > 0L) return afdLength
            return runCatching {
                contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
                    val col = cursor.getColumnIndex(OpenableColumns.SIZE)
                    if (col >= 0 && cursor.moveToFirst()) cursor.getLong(col) else 0L
                } ?: 0L
            }.getOrDefault(0L)
        }
        return 0L
    }

    @OptIn(UnstableApi::class)
    private fun publishSourceOnly(
        exoPlayer: ExoPlayer,
        simpleCache: SimpleCache,
    ) {
        val mediaItem = exoPlayer.currentMediaItem
        val uriString = mediaItem?.localConfiguration?.uri?.toString().orEmpty()
        val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
        val cacheKey = mediaItem?.localConfiguration?.customCacheKey
            ?: uriString.takeIf { it.isNotBlank() }
        val isCached = queryCacheStatusAsync(simpleCache, cacheKey, isOffline)
        val mediaId = mediaItem?.mediaId.orEmpty()
        val sourceLabel = resolvePipelineSourceLabel(mediaItem)
        AudioPipelineBus.update { current ->
            val next = current.copy(
                sourceLabel = sourceLabel,
                fromCache = isCached,
                fromOfflineDownload = isOffline,
                mediaId = mediaId.ifBlank { current.mediaId },
            )
            if (next == current) current else next
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
            runCatching {
                player.skipSilenceEnabled = if (videoModePlaybackActive) false else enabled
            }
        }
    }

    @OptIn(UnstableApi::class)
    internal fun setVideoModePlaybackInternal(active: Boolean) {
        videoModePlaybackActive = active
        val player = exoPlayerInstance ?: return
        postOnPlayer {
            runCatching {
                player.skipSilenceEnabled = if (active) false else PlaybackAudioBridge.isSkipSilenceEnabled()
                if (!active) {
                    player.clearVideoSurface()
                }
            }
        }
    }

    internal fun applyEqualizerInternal(enabled: Boolean, bandsDb: List<Float>) {
        dspProcessor?.updateEqualizer(enabled, bandsDb)
    }

    internal fun applyLoudnessNormalizationInternal(enabled: Boolean) {
        loudnessEnabledFlag = enabled
        val player = exoPlayerInstance
        if (player == null) {
            if (!enabled) releaseLoudnessEnhancer()
            return
        }
        postOnPlayer {
            if (!loudnessEnabledFlag) {
                releaseLoudnessEnhancer()
            } else {
                syncLoudnessEnhancerForSession(player.audioSessionId)
            }
        }
    }

    internal fun applyCrossfadeInternal(enabled: Boolean, seconds: Int) {
        crossfadeEnabledFlag = enabled
        crossfadeDurationMs = seconds.coerceIn(1, 12) * 1_000L
        postOnPlayer {
            if (!crossfadeEnabledFlag) {
                resetCrossfadeState()
            } else {
                maybeScheduleCrossfadeMonitor()
            }
        }
    }

    internal fun applySleepFadeMultiplierInternal(multiplier: Float) {
        dspProcessor?.setSleepFadeMultiplier(multiplier)
    }

    private fun syncLoudnessEnhancerForSession(sessionId: Int) {
        if (!loudnessEnabledFlag) {
            releaseLoudnessEnhancer()
            return
        }
        if (sessionId == C.AUDIO_SESSION_ID_UNSET || sessionId <= 0) {
            return
        }
        if (loudnessEnhancer != null && loudnessSessionId == sessionId) {
            runCatching {
                loudnessEnhancer?.setTargetGain(LOUDNESS_NORM_TARGET_MB)
                loudnessEnhancer?.enabled = true
            }
            return
        }
        releaseLoudnessEnhancer()
        try {
            val enhancer = LoudnessEnhancer(sessionId).apply {
                setTargetGain(LOUDNESS_NORM_TARGET_MB)
                enabled = true
            }
            loudnessEnhancer = enhancer
            loudnessSessionId = sessionId
        } catch (e: Exception) {
            Log.w(TAG, "LoudnessEnhancer unavailable for session $sessionId: ${e.message}")
            loudnessEnhancer = null
            loudnessSessionId = C.AUDIO_SESSION_ID_UNSET
        }
    }

    private fun releaseLoudnessEnhancer() {
        val existing = loudnessEnhancer
        loudnessEnhancer = null
        loudnessSessionId = C.AUDIO_SESSION_ID_UNSET
        if (existing != null) {
            try {
                existing.enabled = false
            } catch (_: Exception) {
                // Intentionally ignored during teardown; session may already be gone.
            }
            try {
                existing.release()
            } catch (_: Exception) {
                // Intentionally ignored during teardown; session may already be gone.
            }
        }
    }

    private fun shouldRunCrossfadeMonitor(): Boolean {
        val player = exoPlayerInstance ?: return false
        return crossfadeEnabledFlag &&
            (player.isPlaying || pendingCrossfadeFadeIn) &&
            player.playbackState == Player.STATE_READY
    }

    private fun maybeScheduleCrossfadeMonitor() {
        if (shouldRunCrossfadeMonitor()) {
            scheduleCrossfadeMonitor()
        }
    }

    private fun scheduleCrossfadeMonitor() {
        if (isServiceDestroyed || crossfadeMonitorScheduled) return
        val handler = playerHandler ?: return
        crossfadeMonitorScheduled = true
        handler.postDelayed(crossfadeMonitorRunnable, CROSSFADE_POLL_INTERVAL_MS)
    }

    private fun cancelCrossfadeMonitor() {
        if (crossfadeMonitorScheduled) {
            playerHandler?.removeCallbacks(crossfadeMonitorRunnable)
            crossfadeMonitorScheduled = false
        }
    }

    private fun maybeStartCrossfadeFadeIn() {
        if (!pendingCrossfadeFadeIn) return
        if (!crossfadeEnabledFlag) {
            pendingCrossfadeFadeIn = false
            dspProcessor?.setCrossfadeMultiplier(1f)
            return
        }
        if (crossfadeFadeInDurationMs <= 0L) {
            crossfadeFadeInStartMs = android.os.SystemClock.elapsedRealtime()
            crossfadeFadeInDurationMs = (crossfadeDurationMs / 2L).coerceIn(300L, 1_500L)
            dspProcessor?.setCrossfadeMultiplier(0f)
        }
    }

    private fun stepCrossfadeMonitor() {
        val player = exoPlayerInstance ?: return
        if (!crossfadeEnabledFlag) {
            resetCrossfadeState()
            return
        }

        if (pendingCrossfadeFadeIn && player.playbackState == Player.STATE_READY && player.isPlaying) {
            maybeStartCrossfadeFadeIn()
            val elapsed = (android.os.SystemClock.elapsedRealtime() - crossfadeFadeInStartMs).coerceAtLeast(0L)
            val progress = (elapsed.toFloat() / crossfadeFadeInDurationMs.coerceAtLeast(1L).toFloat())
                .coerceIn(0f, 1f)
            val gain = sin(progress * (Math.PI / 2.0)).toFloat().coerceIn(0f, 1f)
            dspProcessor?.setCrossfadeMultiplier(gain)
            if (progress >= 1f) {
                pendingCrossfadeFadeIn = false
                crossfadeFadeInDurationMs = 0L
                dspProcessor?.setCrossfadeMultiplier(1f)
            }
        }

        if (!player.isPlaying || player.playbackState != Player.STATE_READY) return
        if (player.repeatMode == Player.REPEAT_MODE_ONE) return

        val duration = player.duration
        val position = player.currentPosition
        if (duration == C.TIME_UNSET || duration < MIN_CROSSFADE_TRACK_MS || position < 0L) return

        val remainingMs = (duration - position).coerceAtLeast(0L)
        val windowMs = crossfadeDurationMs

        // If user sought backward out of the crossfade window, restore full gain and reset trigger.
        if (crossfadeFadeOutTriggeredToken == crossfadeActiveItemToken &&
            crossfadeAdvanceDispatchedToken != crossfadeActiveItemToken &&
            remainingMs > windowMs + 1_000L
        ) {
            crossfadeFadeOutTriggeredToken = -1
            crossfadeFadeOutStartRemainingMs = 0L
            dspProcessor?.setCrossfadeMultiplier(1f)
            return
        }

        if (remainingMs > windowMs) return
        if (!PlaybackQueueBridge.shouldAutoAdvanceOnEnd()) return

        if (crossfadeFadeOutTriggeredToken != crossfadeActiveItemToken) {
            crossfadeFadeOutTriggeredToken = crossfadeActiveItemToken
            crossfadeFadeOutStartRemainingMs = remainingMs.coerceAtLeast(300L)
        }

        val totalFadeOutMs = (crossfadeFadeOutStartRemainingMs - CROSSFADE_TAIL_MS).coerceAtLeast(250L)
        val elapsedInWindow = (crossfadeFadeOutStartRemainingMs - remainingMs).coerceAtLeast(0L)
        val progress = (elapsedInWindow.toFloat() / totalFadeOutMs.toFloat()).coerceIn(0f, 1f)
        val fadeOutGain = cos(progress * (Math.PI / 2.0)).toFloat().coerceIn(0f, 1f)
        dspProcessor?.setCrossfadeMultiplier(fadeOutGain)

        if ((remainingMs <= CROSSFADE_TAIL_MS || progress >= 1f) &&
            crossfadeAdvanceDispatchedToken != crossfadeActiveItemToken
        ) {
            crossfadeAdvanceDispatchedToken = crossfadeActiveItemToken
            pendingCrossfadeFadeIn = true
            crossfadeFadeInDurationMs = 0L
            endedTransitionGuard.onUnderlyingPlaybackStateChanged(
                rawState = Player.STATE_ENDED,
                hasPendingNextTransition = true,
            )
            PlaybackQueueBridge.onTrackEnded()
        }
    }

    private fun resetCrossfadeState() {
        cancelCrossfadeMonitor()
        crossfadeFadeOutTriggeredToken = -1
        crossfadeAdvanceDispatchedToken = -1
        crossfadeFadeOutStartRemainingMs = 0L
        pendingCrossfadeFadeIn = false
        crossfadeFadeInDurationMs = 0L
        dspProcessor?.setCrossfadeMultiplier(1f)
    }

    internal fun clearEndedTransitionSuppressionInternal() {
        endedTransitionGuard.clear()
        postOnPlayer {
            pendingCrossfadeFadeIn = false
            crossfadeFadeInDurationMs = 0L
            dspProcessor?.setCrossfadeMultiplier(1f)
        }
    }

    internal fun attachVideoTextureViewInternal(textureView: TextureView) {
        attachedTextureView = textureView
        postOnPlayer {
            exoPlayerInstance?.setVideoTextureView(textureView)
        }
    }

    internal fun clearVideoTextureViewInternal(textureView: TextureView) {
        if (attachedTextureView === textureView) {
            attachedTextureView = null
        }
        postOnPlayer {
            exoPlayerInstance?.clearVideoTextureView(textureView)
        }
    }

    @OptIn(UnstableApi::class)
    internal fun playVideoStreamInternal(
        videoUrl: String,
        audioUrl: String?,
        mediaItem: MediaItem,
        startPositionMs: Long,
        playWhenReady: Boolean,
    ): Boolean {
        val player = exoPlayerInstance ?: return false
        val factory = videoSourceFactory ?: return false
        videoModePlaybackActive = true
        postOnPlayer {
            player.skipSilenceEnabled = false
            val videoItem = mediaItem.buildUpon()
                .setUri(videoUrl)
                .build()
            val videoSource = factory.createMediaSource(videoItem)
            val combinedSource = if (!audioUrl.isNullOrBlank()) {
                val audioSource = factory.createMediaSource(MediaItem.fromUri(audioUrl))
                MergingMediaSource(true, videoSource, audioSource)
            } else {
                videoSource
            }
            attachedTextureView?.let { tv ->
                player.setVideoTextureView(tv)
            }
            player.setMediaSource(combinedSource, startPositionMs.coerceAtLeast(0L))
            player.prepare()
            player.playWhenReady = playWhenReady
        }
        return true
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

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? {
        return mediaSession
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = mediaSession?.player
        if (PlaybackAudioBridge.stopMusicOnClose) {
            runCatching {
                player?.pause()
                player?.stop()
            }
            stopSelf()
            return
        }
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        isServiceDestroyed = true
        endedTransitionGuard.clear()
        cancelCrossfadeMonitor()
        releaseLoudnessEnhancer()
        dspProcessor?.setCrossfadeMultiplier(1f)
        dspProcessor?.setSleepFadeMultiplier(1f)
        dspProcessor = null
        serviceScope.cancel()
        if (PlaybackAudioBridge.activeService === this) {
            PlaybackAudioBridge.activeService = null
        }
        playerHandler?.removeCallbacksAndMessages(null)
        playerHandler = null
        exoPlayerInstance = null
        mediaSession?.run {
            runCatching { player.release() }
            runCatching { release() }
        }
        mediaSession = null
        super.onDestroy()
    }

    @UnstableApi
    private class QueueAwarePlayer(
        delegate: ExoPlayer,
        private val endedTransitionGuard: EndedStateTransitionGuard,
    ) : ForwardingPlayer(delegate) {
        override fun getPlaybackState(): Int =
            endedTransitionGuard.effectivePlaybackState(super.getPlaybackState())

        override fun pause() {
            endedTransitionGuard.clear()
            super.pause()
        }

        override fun stop() {
            endedTransitionGuard.clear()
            super.stop()
        }

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
            val registeredHeaders = PlaybackAudioBridge.headersForStreamUrl(dataSpec.uri.toString())
            val customUa = registeredHeaders.entries
                .firstOrNull { it.key.equals("User-Agent", ignoreCase = true) }
                ?.value
            val effectiveUa = if (profile.isGooglevideo) profile.userAgent else (customUa ?: profile.userAgent)
            val factory = httpFactoriesByUa.getOrPut(effectiveUa) {
                DefaultHttpDataSource.Factory()
                    .setAllowCrossProtocolRedirects(true)
                    .setConnectTimeoutMs(8_000)
                    .setReadTimeoutMs(15_000)
                    .setUserAgent(effectiveUa)
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
            } else if (registeredHeaders.isNotEmpty()) {
                val mergedHeaders = dataSpec.httpRequestHeaders.toMutableMap()
                registeredHeaders.forEach { (key, value) ->
                    if (!key.equals("User-Agent", ignoreCase = true)) {
                        mergedHeaders[key] = value
                    }
                }
                dataSpec.buildUpon()
                    .setHttpRequestHeaders(mergedHeaders)
                    .build()
            } else {
                dataSpec
            }
            val openedLength = httpSource.open(resolvedSpec)
            val contentRangeTotal = parseHttpContentRangeTotal(httpSource.responseHeaders)
            val clenParam = dataSpec.uri.getQueryParameter("clen")?.toLongOrNull()?.takeIf { it > 0L }
            val isFullRequest = dataSpec.position == 0L &&
                dataSpec.length == C.LENGTH_UNSET.toLong() &&
                !resolvedSpec.uri.toString().contains("range=", ignoreCase = true)
            val totalBytes = when {
                contentRangeTotal > 0L -> contentRangeTotal
                clenParam != null -> clenParam
                isFullRequest -> {
                    val headerLen = parseHttpContentLength(httpSource.responseHeaders)
                    when {
                        headerLen > 0L -> headerLen
                        openedLength > 0L -> openedLength
                        else -> 0L
                    }
                }
                else -> 0L
            }
            if (totalBytes > 0L) {
                recordMeasuredContentBytes(dataSpec.uri.toString(), totalBytes)
                httpSource.uri?.toString()?.let { finalUri ->
                    if (finalUri != dataSpec.uri.toString()) {
                        recordMeasuredContentBytes(finalUri, totalBytes)
                    }
                }
                if (AudioPipelineBus.info.value.bitrateKbps <= 0) {
                    PlaybackAudioBridge.activeService?.let { service ->
                        service.postOnPlayer {
                            val player = service.exoPlayerInstance
                            if (player != null) {
                                service.refreshFormatTelemetryIfPossible(
                                    exoPlayer = player,
                                    simpleCache = getOrCreateCache(service),
                                )
                            }
                        }
                    }
                }
            }
            return openedLength
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
        private const val LOUDNESS_NORM_TARGET_MB = 300
        private const val MIN_CROSSFADE_TRACK_MS = 30_000L
        private const val CROSSFADE_POLL_INTERVAL_MS = 100L
        private const val CROSSFADE_TAIL_MS = 350L
        private const val MAX_EMPTY_RANGES = 5
        private const val MAX_TRANSIENT_CHUNK_RETRIES = 2
        private const val GOOGLEVIDEO_CHUNK_BYTES = 1024L * 1024L
        private const val NARROW_RANGE_BYTES = 512L * 1024L
        private const val EXO_CACHE_BYTES = 512L * 1024L * 1024L
        private const val MAX_CONTENT_BYTES_ENTRIES = 64

        private val contentBytesLock = Any()
        private val measuredContentBytesByUri = object : LinkedHashMap<String, Long>(64, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Long>?): Boolean =
                size > MAX_CONTENT_BYTES_ENTRIES
        }
        private val completedLoadBytesByUri = object : LinkedHashMap<String, Long>(64, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Long>?): Boolean =
                size > MAX_CONTENT_BYTES_ENTRIES
        }

        private fun recordMeasuredContentBytes(uri: String, bytes: Long) {
            if (uri.isBlank() || bytes <= 0L) return
            synchronized(contentBytesLock) {
                measuredContentBytesByUri[uri] = bytes
            }
        }

        internal fun resolvePipelineSourceLabel(mediaId: String, uriString: String): String {
            val isOffline = uriString.startsWith("file://") || uriString.startsWith("content://")
            return when {
                mediaId.startsWith("local:") -> "Local Music"
                mediaId.startsWith("smb:") -> "SMB"
                mediaId.startsWith("jellyfin:") -> "Jellyfin"
                mediaId.startsWith("subsonic:") -> "Subsonic"
                mediaId.startsWith("webdav:") -> "WebDAV"
                isOffline -> "Offline Download"
                uriString.contains("googlevideo.com", ignoreCase = true) -> "YouTube Music"
                uriString.isNotBlank() -> "Audio Stream"
                else -> "Idle"
            }
        }

        private fun lookupMeasuredContentBytes(uri: String): Long {
            if (uri.isBlank()) return 0L
            synchronized(contentBytesLock) {
                return measuredContentBytesByUri[uri] ?: 0L
            }
        }

        private fun recordCompletedLoadBytes(uri: String, bytes: Long) {
            if (uri.isBlank() || bytes <= 0L) return
            synchronized(contentBytesLock) {
                completedLoadBytesByUri[uri] = bytes
            }
        }

        private fun lookupCompletedLoadBytes(uri: String): Long {
            if (uri.isBlank()) return 0L
            synchronized(contentBytesLock) {
                return completedLoadBytesByUri[uri] ?: 0L
            }
        }
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
