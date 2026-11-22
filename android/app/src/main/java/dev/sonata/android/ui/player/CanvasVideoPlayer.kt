package dev.sonata.android.ui.player

import android.content.Context
import android.graphics.Matrix
import android.graphics.SurfaceTexture
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.TextureView
import android.view.ViewGroup
import androidx.annotation.OptIn
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.TransferListener
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.ui.components.ScrollActivityTracker
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.Locale

private const val TAG = "SonataCanvas"

private val mainHandler = Handler(Looper.getMainLooper())

private const val CANVAS_PLAYER_UA =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"

/**
 * Disk-backed 150 MiB ExoPlayer cache dedicated to looping Canvas artwork clips
 * so repeated loops never re-download the same video segments.
 *
 * HLS `.m3u8` playlist manifests bypass the disk cache so Media3 always parses fresh
 * manifests cleanly, while `.ts` / `.m4s` / `.mp4` media segments are cached on disk.
 */
@OptIn(UnstableApi::class)
object CanvasCache {

    private const val MAX_CACHE_BYTES = 150L * 1024L * 1024L
    private const val CACHE_DIR_NAME = "canvas_video"

    @Volatile
    private var simpleCache: SimpleCache? = null

    fun isReady(): Boolean = simpleCache != null

    fun getOrInit(context: Context): SimpleCache {
        simpleCache?.let { return it }
        return synchronized(this) {
            simpleCache ?: SimpleCache(
                File(context.applicationContext.cacheDir, CACHE_DIR_NAME),
                LeastRecentlyUsedCacheEvictor(MAX_CACHE_BYTES),
                StandaloneDatabaseProvider(context.applicationContext),
            ).also { simpleCache = it }
        }
    }

    fun dataSourceFactory(context: Context): DataSource.Factory {
        val appContext = context.applicationContext
        val upstreamFactory = DefaultHttpDataSource.Factory()
            .setUserAgent(CANVAS_PLAYER_UA)
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(6_000)
            .setReadTimeoutMs(10_000)
        val cacheFactory = CacheDataSource.Factory()
            .setCache(getOrInit(appContext))
            .setUpstreamDataSourceFactory(upstreamFactory)
            .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)

        return DataSource.Factory {
            val cached = cacheFactory.createDataSource()
            val direct = DefaultDataSource.Factory(appContext, upstreamFactory).createDataSource()
            HlsAndFileBypassDataSource(cached = cached, direct = direct)
        }
    }
}

/**
 * Local file cache for progressive `.mp4` Canvas clips.
 *
 * Many community/CDN hosts (such as Cloudflare Workers) ignore HTTP `Range` requests and
 * return `200 OK` from byte 0, while MP4 clips encoded without `faststart` place their `moov`
 * atom at the end of the file (e.g. byte ~5.2 MB). Caching the complete `.mp4` locally once
 * allows Media3's `Mp4Extractor` to seek between `moov` and `mdat` instantaneously with zero
 * network stalls or Range errors.
 */
private object CanvasClipFileCache {

    private const val DIR_NAME = "canvas_clips"
    private const val MAX_CLIP_BYTES = 25L * 1024L * 1024L
    private const val MAX_DIR_BYTES = 150L * 1024L * 1024L

    private val downloadMutex = Mutex()

    suspend fun getOrDownloadMp4(context: Context, url: String): File? = withContext(Dispatchers.IO) {
        val dir = File(context.applicationContext.cacheDir, DIR_NAME)
        if (!dir.exists() && !dir.mkdirs()) return@withContext null

        val key = sha256(url)
        val target = File(dir, "$key.mp4")
        if (target.isFile && target.length() > 0L) {
            target.setLastModified(System.currentTimeMillis())
            Log.d(TAG, "mp4 cache hit (${target.length()} bytes) for $url")
            return@withContext target
        }

        downloadMutex.withLock {
            if (target.isFile && target.length() > 0L) {
                target.setLastModified(System.currentTimeMillis())
                return@withLock target
            }
            val tmp = File(dir, "$key.mp4.part")
            var conn: HttpURLConnection? = null
            try {
                val started = System.currentTimeMillis()
                conn = (URL(url).openConnection() as HttpURLConnection).apply {
                    requestMethod = "GET"
                    connectTimeout = 6_000
                    readTimeout = 15_000
                    instanceFollowRedirects = true
                    setRequestProperty("User-Agent", CANVAS_PLAYER_UA)
                    setRequestProperty("Accept", "video/mp4,application/octet-stream,*/*")
                }
                val code = conn.responseCode
                if (code !in 200..299) {
                    Log.w(TAG, "mp4 download HTTP $code for $url")
                    return@withLock null
                }
                val declaredLen = conn.contentLengthLong
                if (declaredLen > MAX_CLIP_BYTES) {
                    Log.w(TAG, "mp4 exceeds max clip size ($declaredLen bytes) for $url")
                    return@withLock null
                }

                var total = 0L
                val buf = ByteArray(16 * 1024)
                conn.inputStream.use { input ->
                    FileOutputStream(tmp).use { output ->
                        while (true) {
                            val read = input.read(buf)
                            if (read < 0) break
                            total += read
                            if (total > MAX_CLIP_BYTES) {
                                tmp.delete()
                                return@withLock null
                            }
                            output.write(buf, 0, read)
                        }
                    }
                }
                if (total <= 0L) {
                    tmp.delete()
                    return@withLock null
                }
                if (!tmp.renameTo(target)) {
                    tmp.copyTo(target, overwrite = true)
                    tmp.delete()
                }
                trimLru(dir)
                Log.d(TAG, "mp4 cached ($total bytes in ${System.currentTimeMillis() - started}ms) for $url")
                target
            } catch (e: Exception) {
                tmp.delete()
                Log.w(TAG, "mp4 download failed for $url: ${e.message}")
                null
            } finally {
                conn?.disconnect()
            }
        }
    }

    private fun trimLru(dir: File) {
        val files = dir.listFiles { f -> f.isFile && f.name.endsWith(".mp4") }
            ?.sortedByDescending(File::lastModified)
            .orEmpty()
        var kept = 0L
        for (file in files) {
            kept += file.length()
            if (kept > MAX_DIR_BYTES) {
                file.delete()
            }
        }
    }

    private fun sha256(value: String): String = MessageDigest.getInstance("SHA-256")
        .digest(value.toByteArray(Charsets.UTF_8))
        .joinToString("") { "%02x".format(it) }
}

/**
 * Routes `file://` URIs and `.m3u8` HLS playlists directly through [direct] so local clips
 * and manifest parsing never touch `SimpleCache`, while routing HLS segments and remote
 * fallback streams through [cached].
 */
@OptIn(UnstableApi::class)
private class HlsAndFileBypassDataSource(
    private val cached: DataSource,
    private val direct: DataSource,
) : DataSource {

    private var active: DataSource? = null

    override fun addTransferListener(transferListener: TransferListener) {
        cached.addTransferListener(transferListener)
        direct.addTransferListener(transferListener)
    }

    override fun open(dataSpec: DataSpec): Long {
        val scheme = dataSpec.uri.scheme?.lowercase(Locale.ROOT).orEmpty()
        val path = dataSpec.uri.path?.lowercase(Locale.ROOT).orEmpty()
        val chosen = if (scheme == "file" || scheme == "content" || path.endsWith(".m3u8")) direct else cached
        active = chosen
        return chosen.open(dataSpec)
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int =
        active?.read(buffer, offset, length) ?: C.RESULT_END_OF_INPUT

    override fun getUri(): Uri? = active?.uri

    override fun getResponseHeaders(): Map<String, List<String>> =
        active?.responseHeaders ?: emptyMap()

    override fun close() {
        try {
            active?.close()
        } finally {
            active = null
        }
    }
}

/**
 * Bounds concurrent card-level hardware video decoders (max 2 active) so Home/Top Picks
 * scrolling remains butter-smooth without exhausting MediaCodec instances.
 */
private object CardDecoderPool {
    val semaphore = Semaphore(permits = 2)
}

/**
 * Silent secondary ExoPlayer surface for looping animated album artwork (Canvas).
 *
 * - Audio track is disabled and volume is muted so it never interferes with music playback or audio focus.
 * - Loops seamlessly via [Player.REPEAT_MODE_ONE] backed by [CanvasClipFileCache] (for `.mp4`) and [CanvasCache] (for HLS).
 * - Stays transparent until the first video frame renders (`onRenderedFirstFrame` or `onSurfaceTextureUpdated`),
 *   then fades in over 320ms.
 * - Automatically pauses when backgrounded or when [isVisible] is false.
 * - Retries [CanvasMedia.fallbackUrl] (and square [CanvasMedia.url] if [preferPortrait] fails)
 *   before gracefully yielding to static artwork.
 */
@OptIn(UnstableApi::class)
@Composable
fun CanvasVideoSurface(
    media: CanvasMedia,
    isVisible: Boolean,
    modifier: Modifier = Modifier,
    preferPortrait: Boolean = false,
    onFirstFrame: () -> Unit = {},
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val latestOnFirstFrame by rememberUpdatedState(onFirstFrame)

    val candidateUrls = remember(media, preferPortrait) {
        buildList {
            if (preferPortrait && !media.portraitUrl.isNullOrBlank()) {
                add(media.portraitUrl)
            }
            add(media.url)
            if (!media.fallbackUrl.isNullOrBlank()) {
                add(media.fallbackUrl)
            }
        }.distinct()
    }

    var candidateIndex by remember(candidateUrls) { mutableStateOf(0) }
    val activeUrl = candidateUrls.getOrElse(candidateIndex) { media.url }
    var firstFrameRendered by remember(media) { mutableStateOf(false) }
    var videoAspect by remember(media) { mutableFloatStateOf(0f) }
    var isResumed by remember { mutableStateOf(true) }
    val mountedTexture = remember { java.util.concurrent.atomic.AtomicReference<TextureView?>(null) }
    val transformMemo = remember { TransformMemo() }

    // Pre-warm SimpleCache SQLite index on IO thread so ExoPlayer creation never stalls main thread.
    var cacheReady by remember { mutableStateOf(CanvasCache.isReady()) }
    LaunchedEffect(cacheReady) {
        if (!cacheReady) {
            withContext(Dispatchers.IO) {
                CanvasCache.getOrInit(context)
            }
            cacheReady = true
        }
    }
    if (!cacheReady) return

    val exoPlayer = remember(context) {
        val mediaSourceFactory = DefaultMediaSourceFactory(CanvasCache.dataSourceFactory(context))
        ExoPlayer.Builder(context)
            .setMediaSourceFactory(mediaSourceFactory)
            .build()
            .apply {
                volume = 0f
                repeatMode = Player.REPEAT_MODE_ONE
                trackSelectionParameters = trackSelectionParameters
                    .buildUpon()
                    .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, true)
                    .build()
            }
    }

    fun markFirstFrameReady(via: String) {
        if (!firstFrameRendered) {
            firstFrameRendered = true
            Log.d(TAG, "first frame ready ($via) aspect=$videoAspect url=$activeUrl")
            latestOnFirstFrame()
        }
    }

    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> isResumed = true
                Lifecycle.Event.ON_PAUSE -> isResumed = false
                else -> Unit
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
        }
    }

    DisposableEffect(exoPlayer, candidateUrls) {
        val listener = object : Player.Listener {
            override fun onRenderedFirstFrame() {
                markFirstFrameReady("onRenderedFirstFrame")
            }

            override fun onVideoSizeChanged(videoSize: VideoSize) {
                val width = videoSize.width * videoSize.pixelWidthHeightRatio
                if (width.isFinite() && width > 0f && videoSize.height > 0) {
                    videoAspect = width / videoSize.height
                    mountedTexture.get()?.let { applyCenterCropTransform(it, videoAspect, transformMemo) }
                }
            }

            override fun onPlayerError(error: PlaybackException) {
                Log.w(
                    TAG,
                    "player error code=${error.errorCodeName} msg=${error.message} url=$activeUrl",
                    error,
                )
                firstFrameRendered = false
                if (candidateIndex + 1 < candidateUrls.size) {
                    candidateIndex += 1
                }
            }
        }
        exoPlayer.addListener(listener)
        onDispose {
            exoPlayer.removeListener(listener)
        }
    }

    DisposableEffect(exoPlayer) {
        onDispose {
            mountedTexture.getAndSet(null)?.let { tv ->
                exoPlayer.clearVideoTextureView(tv)
            }
            exoPlayer.pause()
            mainHandler.postDelayed(
                {
                    runCatching {
                        exoPlayer.stop()
                        exoPlayer.release()
                    }
                },
                150L,
            )
        }
    }

    LaunchedEffect(activeUrl) {
        firstFrameRendered = false
        videoAspect = 0f
        val path = activeUrl.substringBefore('?').lowercase(Locale.ROOT)
        val isHlsUrl = path.endsWith(".m3u8") || (media.isHls && activeUrl == media.url)

        val itemBuilder = MediaItem.Builder()
        if (isHlsUrl) {
            Log.d(TAG, "prepare HLS url=$activeUrl")
            itemBuilder.setUri(activeUrl).setMimeType(MimeTypes.APPLICATION_M3U8)
        } else {
            val localFile = CanvasClipFileCache.getOrDownloadMp4(context, activeUrl)
            if (localFile != null) {
                Log.d(TAG, "prepare local MP4 file=${localFile.name} source=$activeUrl")
                itemBuilder.setUri(Uri.fromFile(localFile)).setMimeType(MimeTypes.VIDEO_MP4)
            } else {
                Log.d(TAG, "prepare streaming MP4 fallback url=$activeUrl")
                itemBuilder.setUri(activeUrl)
                if (path.endsWith(".mp4")) itemBuilder.setMimeType(MimeTypes.VIDEO_MP4)
            }
        }

        mountedTexture.get()?.let { exoPlayer.setVideoTextureView(it) }
        exoPlayer.setMediaItem(itemBuilder.build())
        exoPlayer.playWhenReady = isVisible && isResumed
        exoPlayer.prepare()
    }

    LaunchedEffect(isVisible, isResumed, activeUrl) {
        val shouldPlay = isVisible && isResumed
        exoPlayer.playWhenReady = shouldPlay
        if (shouldPlay && exoPlayer.playbackState == Player.STATE_IDLE && exoPlayer.mediaItemCount > 0) {
            exoPlayer.prepare()
        }
    }

    val fadeAlpha by animateFloatAsState(
        targetValue = if (firstFrameRendered) 1f else 0f,
        animationSpec = tween(durationMillis = 320),
        label = "canvasSurfaceFade",
    )

    val latestAspect by rememberUpdatedState(videoAspect)
    val latestIsVisible by rememberUpdatedState(isVisible)

    AndroidView(
        factory = { viewContext ->
            TextureView(viewContext).apply {
                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                )
                isOpaque = false
                alpha = 0f
                mountedTexture.set(this)
                exoPlayer.setVideoTextureView(this)
                addOnLayoutChangeListener { v, _, _, _, _, _, _, _, _ ->
                    applyCenterCropTransform(v as TextureView, latestAspect, transformMemo)
                }
                val delegate = surfaceTextureListener
                surfaceTextureListener = object : TextureView.SurfaceTextureListener {
                    override fun onSurfaceTextureAvailable(
                        surface: SurfaceTexture,
                        width: Int,
                        height: Int,
                    ) {
                        delegate?.onSurfaceTextureAvailable(surface, width, height)
                        applyCenterCropTransform(this@apply, latestAspect, transformMemo)
                        if (exoPlayer.playbackState != Player.STATE_IDLE && latestIsVisible) {
                            exoPlayer.seekTo(exoPlayer.currentPosition)
                        }
                    }

                    override fun onSurfaceTextureSizeChanged(
                        surface: SurfaceTexture,
                        width: Int,
                        height: Int,
                    ) {
                        delegate?.onSurfaceTextureSizeChanged(surface, width, height)
                        applyCenterCropTransform(this@apply, latestAspect, transformMemo)
                    }

                    override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean =
                        delegate?.onSurfaceTextureDestroyed(surface) ?: true

                    override fun onSurfaceTextureUpdated(surface: SurfaceTexture) {
                        delegate?.onSurfaceTextureUpdated(surface)
                        if (!firstFrameRendered &&
                            (exoPlayer.playbackState == Player.STATE_READY || latestAspect > 0f)
                        ) {
                            markFirstFrameReady("onSurfaceTextureUpdated")
                        }
                    }
                }
            }
        },
        update = { textureView ->
            textureView.alpha = fadeAlpha
            applyCenterCropTransform(textureView, videoAspect, transformMemo)
        },
        modifier = modifier.clipToBounds(),
    )
}

/**
 * Lightweight card-scoped animated Canvas player for Top Picks & editorial shelves.
 *
 * Acquires a slot from the max-2 [CardDecoderPool], settles for 450ms while
 * [ScrollActivityTracker.isScrolling] is false before mounting an ExoPlayer decoder,
 * and releases when offscreen or when [reduceAnimation] is enabled — without recomposing
 * on scroll start/stop.
 */
@Composable
fun CardCanvasSurface(
    media: CanvasMedia,
    isVisible: Boolean,
    reduceAnimation: Boolean = false,
    modifier: Modifier = Modifier,
    onFirstFrame: () -> Unit = {},
) {
    var decoderAcquired by remember(media) { mutableStateOf(false) }

    LaunchedEffect(media, isVisible, reduceAnimation) {
        if (!isVisible || reduceAnimation) {
            decoderAcquired = false
            return@LaunchedEffect
        }
        var settledMs = 0L
        while (settledMs < 450L) {
            delay(50L)
            settledMs = if (ScrollActivityTracker.isScrolling) 0L else settledMs + 50L
        }
        CardDecoderPool.semaphore.acquire()
        decoderAcquired = true
        try {
            kotlinx.coroutines.awaitCancellation()
        } finally {
            decoderAcquired = false
            CardDecoderPool.semaphore.release()
        }
    }

    if (decoderAcquired && isVisible && !reduceAnimation) {
        CanvasVideoSurface(
            media = media,
            isVisible = true,
            preferPortrait = false,
            modifier = modifier.clipToBounds(),
            onFirstFrame = onFirstFrame,
        )
    }
}

private class TransformMemo(
    var width: Int = -1,
    var height: Int = -1,
    var aspect: Float = -1f,
)

private fun applyCenterCropTransform(
    textureView: TextureView,
    videoAspect: Float,
    memo: TransformMemo,
) {
    val viewW = textureView.width
    val viewH = textureView.height
    val normalizedAspect = if (viewW > 0 && viewH > 0 && videoAspect.isFinite() && videoAspect > 0f) {
        videoAspect
    } else {
        0f
    }
    if (memo.width == viewW && memo.height == viewH && memo.aspect == normalizedAspect) {
        return
    }
    memo.width = viewW
    memo.height = viewH
    memo.aspect = normalizedAspect
    if (normalizedAspect == 0f) {
        textureView.setTransform(Matrix())
        return
    }
    val viewAspect = viewW.toFloat() / viewH.toFloat()
    val scaleX: Float
    val scaleY: Float
    if (normalizedAspect > viewAspect) {
        scaleX = normalizedAspect / viewAspect
        scaleY = 1f
    } else {
        scaleX = 1f
        scaleY = viewAspect / normalizedAspect
    }
    val matrix = Matrix().apply {
        setScale(scaleX, scaleY, viewW / 2f, viewH / 2f)
    }
    textureView.setTransform(matrix)
}
