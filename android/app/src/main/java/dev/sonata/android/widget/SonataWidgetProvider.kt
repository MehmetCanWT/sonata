/*
 * Reusable Home Screen AppWidget Infrastructure for Sonata Android.
 *
 * Provides persisted playback snapshot state (`SonataWidgetSnapshot`), Coil rounded artwork
 * decoding, transport PendingIntent helpers (`SonataWidgetControlReceiver`), and a pluggable
 * `SonataWidgetRenderer` registry so future widgets can render live playback state without
 * touching `SonataViewModel` or `PlaybackService`.
 */
package dev.sonata.android.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.Rect
import android.graphics.RectF
import android.net.Uri
import android.os.Handler
import android.os.Looper
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import dev.sonata.android.MainActivity
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.PlaybackQueueBridge
import dev.sonata.android.data.AppLanguage
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import java.io.File
import java.util.Locale
import java.util.concurrent.CopyOnWriteArrayList

object SonataWidgetActions {
    const val ACTION_PLAY_PAUSE = "dev.sonata.android.widget.ACTION_PLAY_PAUSE"
    const val ACTION_NEXT = "dev.sonata.android.widget.ACTION_NEXT"
    const val ACTION_PREV = "dev.sonata.android.widget.ACTION_PREV"
}

data class SonataWidgetSnapshot(
    val title: String,
    val rawArtist: String?,
    val artistLine: String,
    val artworkUrl: String?,
    val isPlaying: Boolean,
    val positionMs: Long,
    val durationMs: Long,
    val progressPercent: Int,
    val remainingMs: Long,
    val languageCode: String,
)

interface SonataWidgetRenderer {
    val providerClass: Class<out AppWidgetProvider>

    fun render(
        context: Context,
        manager: AppWidgetManager,
        widgetIds: IntArray,
        snapshot: SonataWidgetSnapshot,
        artworkBitmap: Bitmap?,
    )
}

/**
 * Non-exported receiver for home screen widget playback control PendingIntents.
 * Keeps transport control actions private to Sonata so third-party apps cannot spoof broadcasts.
 */
class SonataWidgetControlReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            SonataWidgetActions.ACTION_PLAY_PAUSE -> handlePlayPause(context)
            SonataWidgetActions.ACTION_NEXT -> handleNext(context)
            SonataWidgetActions.ACTION_PREV -> handlePrev(context)
        }
    }

    private fun handlePlayPause(context: Context) {
        val service = PlaybackAudioBridge.activeService
        if (service != null) {
            Handler(Looper.getMainLooper()).post {
                val toggled = service.togglePlayPauseFromWidget()
                if (!toggled) {
                    PlaybackQueueBridge.onPlayFromEmpty()
                }
            }
        } else {
            launchApp(context)
        }
    }

    private fun handleNext(context: Context) {
        if (PlaybackAudioBridge.activeService != null) {
            Handler(Looper.getMainLooper()).post {
                PlaybackQueueBridge.onSkipNext()
            }
        } else {
            launchApp(context)
        }
    }

    private fun handlePrev(context: Context) {
        if (PlaybackAudioBridge.activeService != null) {
            Handler(Looper.getMainLooper()).post {
                PlaybackQueueBridge.onSkipPrevious()
            }
        } else {
            launchApp(context)
        }
    }

    private fun launchApp(context: Context) {
        runCatching {
            val launch = Intent(context, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            }
            context.startActivity(launch)
        }
    }
}

object SonataWidgetUpdater {
    private const val PREFS_NAME = "sonata_widget_state"
    private const val SETTINGS_PREFS_NAME = "sonata_preferences"
    private const val KEY_APP_LANGUAGE = "app_language"
    private const val KEY_TITLE = "title"
    private const val KEY_ARTIST = "artist"
    private const val KEY_ARTWORK_URL = "artwork_url"
    private const val KEY_IS_PLAYING = "is_playing"
    private const val KEY_POS_MS = "pos_ms"
    private const val KEY_DUR_MS = "dur_ms"
    private const val ARTWORK_DECODE_PX = 256

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val artworkLock = Any()
    private val renderers = CopyOnWriteArrayList<SonataWidgetRenderer>()

    private var artworkJob: Job? = null

    @Volatile
    private var requestedArtworkUrl: String? = null

    @Volatile
    private var cachedArtworkUrl: String? = null

    @Volatile
    private var cachedArtworkBitmap: Bitmap? = null

    fun registerRenderer(renderer: SonataWidgetRenderer) {
        if (renderers.none { it.providerClass == renderer.providerClass }) {
            renderers.add(renderer)
        }
    }

    fun unregisterRenderer(providerClass: Class<out AppWidgetProvider>) {
        renderers.removeAll { it.providerClass == providerClass }
    }

    fun pushUpdate(
        context: Context,
        title: String,
        artist: String,
        artworkUrl: String?,
        isPlaying: Boolean,
        positionMs: Long,
        durationMs: Long,
    ) {
        val appContext = context.applicationContext
        val cleanUrl = normalizeArtworkUri(artworkUrl)

        synchronized(artworkLock) {
            requestedArtworkUrl = cleanUrl
            if (cleanUrl == null || cleanUrl != cachedArtworkUrl) {
                artworkJob?.cancel()
                artworkJob = null
            }
        }

        scope.launch {
            appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).edit()
                .putString(KEY_TITLE, title)
                .putString(KEY_ARTIST, artist)
                .putString(KEY_ARTWORK_URL, cleanUrl.orEmpty())
                .putBoolean(KEY_IS_PLAYING, isPlaying)
                .putLong(KEY_POS_MS, positionMs.coerceAtLeast(0L))
                .putLong(KEY_DUR_MS, durationMs.coerceAtLeast(0L))
                .apply()

            if (renderers.isEmpty()) return@launch
            val manager = AppWidgetManager.getInstance(appContext)
            if (!hasActiveWidgets(appContext, manager)) return@launch

            renderRegistered(appContext, manager)

            if (cleanUrl != null && cleanUrl != cachedArtworkUrl) {
                synchronized(artworkLock) {
                    if (requestedArtworkUrl != cleanUrl) return@launch
                    artworkJob?.cancel()
                    artworkJob = scope.launch {
                        val bmp = fetchRoundedArtwork(appContext, cleanUrl, sizePx = ARTWORK_DECODE_PX)
                        ensureActive()
                        if (bmp != null && requestedArtworkUrl == cleanUrl) {
                            cachedArtworkUrl = cleanUrl
                            cachedArtworkBitmap = bmp
                            renderRegistered(appContext, manager)
                        }
                    }
                }
            }
        }
    }

    fun pushPlaybackState(
        context: Context,
        isPlaying: Boolean,
        positionMs: Long,
        durationMs: Long,
    ) {
        val appContext = context.applicationContext
        scope.launch {
            val prefs = appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit()
                .putBoolean(KEY_IS_PLAYING, isPlaying)
                .putLong(KEY_POS_MS, positionMs.coerceAtLeast(0L))
                .putLong(KEY_DUR_MS, durationMs.coerceAtLeast(0L))
                .apply()

            if (renderers.isEmpty()) return@launch
            val manager = AppWidgetManager.getInstance(appContext)
            if (!hasActiveWidgets(appContext, manager)) return@launch
            renderRegistered(appContext, manager)
        }
    }

    fun readSnapshot(context: Context): SonataWidgetSnapshot {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val title = prefs.getString(KEY_TITLE, null)?.takeIf { it.isNotBlank() } ?: "Sonata"
        val rawArtist = prefs.getString(KEY_ARTIST, null)?.takeIf { it.isNotBlank() }
        val lang = resolveWidgetLanguage(context)
        val artistLine = if (rawArtist != null) {
            formatByArtist(lang, rawArtist)
        } else {
            localizedTapToPlay(lang)
        }
        val artworkUrl = prefs.getString(KEY_ARTWORK_URL, null)?.takeIf { it.isNotBlank() }
        val isPlaying = prefs.getBoolean(KEY_IS_PLAYING, false)
        val posMs = prefs.getLong(KEY_POS_MS, 0L).coerceAtLeast(0L)
        val durMs = prefs.getLong(KEY_DUR_MS, 0L).coerceAtLeast(0L)
        val progressPct = if (durMs > 0L) ((posMs * 100L) / durMs).toInt().coerceIn(0, 100) else 0
        val remainingMs = if (durMs > posMs) durMs - posMs else 0L
        return SonataWidgetSnapshot(
            title = title,
            rawArtist = rawArtist,
            artistLine = artistLine,
            artworkUrl = artworkUrl,
            isPlaying = isPlaying,
            positionMs = posMs,
            durationMs = durMs,
            progressPercent = progressPct,
            remainingMs = remainingMs,
            languageCode = lang,
        )
    }

    fun openAppPending(context: Context, reqCode: Int = 0): PendingIntent {
        val openIntent = Intent(context, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        }
        return PendingIntent.getActivity(
            context,
            reqCode,
            openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    fun broadcastPending(context: Context, action: String, reqCode: Int): PendingIntent {
        val intent = Intent(context, SonataWidgetControlReceiver::class.java).apply {
            this.action = action
        }
        return PendingIntent.getBroadcast(
            context,
            reqCode,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    fun formatClock(ms: Long): String {
        val totalSec = (ms / 1000L).coerceAtLeast(0L)
        val mins = totalSec / 60L
        val secs = totalSec % 60L
        return "$mins:${secs.toString().padStart(2, '0')}"
    }

    private fun hasActiveWidgets(context: Context, manager: AppWidgetManager): Boolean =
        renderers.any { renderer ->
            manager.getAppWidgetIds(ComponentName(context, renderer.providerClass)).isNotEmpty()
        }

    internal fun renderRegistered(context: Context, manager: AppWidgetManager) {
        if (renderers.isEmpty()) return
        val snapshot = readSnapshot(context)
        val artworkBitmap = if (snapshot.artworkUrl != null && snapshot.artworkUrl == cachedArtworkUrl) {
            cachedArtworkBitmap
        } else {
            null
        }
        for (renderer in renderers) {
            val ids = manager.getAppWidgetIds(ComponentName(context, renderer.providerClass))
            if (ids.isNotEmpty()) {
                renderer.render(context, manager, ids, snapshot, artworkBitmap)
            }
        }
    }

    private fun resolveWidgetLanguage(context: Context): String {
        val prefs = context.getSharedPreferences(SETTINGS_PREFS_NAME, Context.MODE_PRIVATE)
        return when (prefs.getString(KEY_APP_LANGUAGE, null)?.trim()?.uppercase(Locale.ROOT)) {
            AppLanguage.ENGLISH.name -> "en"
            AppLanguage.TURKISH.name -> "tr"
            AppLanguage.RUSSIAN.name -> "ru"
            else -> when (Locale.getDefault().language.lowercase(Locale.ROOT)) {
                "tr" -> "tr"
                "ru" -> "ru"
                else -> "en"
            }
        }
    }

    private fun localizedTapToPlay(lang: String): String = when (lang) {
        "tr" -> "Oynatmak için dokun"
        "ru" -> "Нажмите для воспроизведения"
        else -> "Tap to play"
    }

    private fun formatByArtist(lang: String, artist: String): String = when (lang) {
        "tr" -> artist
        "ru" -> "от $artist"
        else -> "by $artist"
    }

    private fun normalizeArtworkUri(raw: String?): String? {
        val trimmed = raw?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        return when {
            trimmed.startsWith("http://", ignoreCase = true) ||
                trimmed.startsWith("https://", ignoreCase = true) ||
                trimmed.startsWith("file://", ignoreCase = true) ||
                trimmed.startsWith("content://", ignoreCase = true) -> trimmed
            trimmed.startsWith("/") -> Uri.fromFile(File(trimmed)).toString()
            else -> null
        }
    }

    private suspend fun fetchRoundedArtwork(
        appContext: Context,
        uriString: String,
        sizePx: Int,
    ): Bitmap? {
        return try {
            val requestData: Any = when {
                uriString.startsWith("file://", ignoreCase = true) ||
                    uriString.startsWith("content://", ignoreCase = true) -> Uri.parse(uriString)
                else -> uriString
            }
            val request = ImageRequest.Builder(appContext)
                .data(requestData)
                .size(sizePx, sizePx)
                .allowHardware(false)
                .build()
            val result = SingletonImageLoader.get(appContext).execute(request)
            val decoded = (result as? SuccessResult)?.image?.toBitmap() ?: return null
            val out = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(out)
            val paint = Paint(Paint.ANTI_ALIAS_FLAG)
            val srcRect = Rect(0, 0, decoded.width, decoded.height)
            val dstRect = Rect(0, 0, sizePx, sizePx)
            val rectF = RectF(dstRect)
            val radius = sizePx * 0.12f
            canvas.drawRoundRect(rectF, radius, radius, paint)
            paint.xfermode = PorterDuffXfermode(PorterDuff.Mode.SRC_IN)
            canvas.drawBitmap(decoded, srcRect, dstRect, paint)
            out
        } catch (_: Exception) {
            null
        }
    }
}
