package dev.sonata.android

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
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
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import java.io.File
import java.io.InterruptedIOException
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap

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

class PlaybackService : MediaSessionService() {
    private var mediaSession: MediaSession? = null

    @OptIn(UnstableApi::class)
    override fun onCreate() {
        super.onCreate()
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

        val sessionPlayer = QueueAwarePlayer(exoPlayer)
        mediaSession = MediaSession.Builder(this, sessionPlayer).build()
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
        mediaSession?.run {
            player.release()
            release()
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
                    .setConnectTimeoutMs(6_000)
                    .setReadTimeoutMs(10_000)
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
            val length = minOf(chunkBytes, bytesRemaining)
            val spec = requireNotNull(baseSpec).buildUpon()
                .setPosition(position)
                .setLength(length)
                .build()
            try {
                upstream.open(spec)
            } catch (e: Exception) {
                if (e !is InterruptedIOException) {
                    val client = spec.uri.getQueryParameter("c") ?: "unknown"
                    Log.w(TAG, "Chunk $position-${position + length - 1} failed for $client: ${e.message}")
                }
                throw e
            }
            chunkRemaining = length
            chunkOpen = true
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
            return C.RESULT_END_OF_INPUT
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
        private const val MAX_EMPTY_RANGES = 3
        private const val GOOGLEVIDEO_CHUNK_BYTES = 1024L * 1024L
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
