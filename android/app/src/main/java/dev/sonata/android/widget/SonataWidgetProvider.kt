/*
 * Sonata Retro Pixel-Art Turntable Home Screen AppWidget.
 *
 * Renders the pastel blush-pink retro window frame with pixel-art vinyl turntable,
 * live track info, rounded album cover, working Previous / Play-Pause / Next controls,
 * and a star progress bar synced directly with PlaybackService.
 */
package dev.sonata.android.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
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
import android.widget.RemoteViews
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import dev.sonata.android.MainActivity
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.PlaybackQueueBridge
import dev.sonata.android.R
import dev.sonata.android.data.AppLanguage
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import java.io.File
import java.util.Locale

class SonataWidgetProvider : AppWidgetProvider() {

    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        SonataWidgetUpdater.renderAll(context, appWidgetManager, appWidgetIds)
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        when (intent.action) {
            ACTION_PLAY_PAUSE -> handlePlayPause(context)
            ACTION_NEXT -> handleNext(context)
            ACTION_PREV -> handlePrev(context)
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

    companion object {
        const val ACTION_PLAY_PAUSE = "dev.sonata.android.widget.ACTION_PLAY_PAUSE"
        const val ACTION_NEXT = "dev.sonata.android.widget.ACTION_NEXT"
        const val ACTION_PREV = "dev.sonata.android.widget.ACTION_PREV"
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

    private var artworkJob: Job? = null

    @Volatile
    private var requestedArtworkUrl: String? = null

    @Volatile
    private var cachedArtworkUrl: String? = null

    @Volatile
    private var cachedArtworkBitmap: Bitmap? = null

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

            val manager = AppWidgetManager.getInstance(appContext)
            val ids = manager.getAppWidgetIds(ComponentName(appContext, SonataWidgetProvider::class.java))
            if (ids.isEmpty()) return@launch

            renderAll(appContext, manager, ids)

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
                            val latestIds = manager.getAppWidgetIds(
                                ComponentName(appContext, SonataWidgetProvider::class.java),
                            )
                            if (latestIds.isNotEmpty()) {
                                renderAll(appContext, manager, latestIds)
                            }
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

            val manager = AppWidgetManager.getInstance(appContext)
            val ids = manager.getAppWidgetIds(ComponentName(appContext, SonataWidgetProvider::class.java))
            if (ids.isEmpty()) return@launch
            renderAll(appContext, manager, ids)
        }
    }

    internal fun renderAll(
        context: Context,
        manager: AppWidgetManager,
        widgetIds: IntArray,
    ) {
        if (widgetIds.isEmpty()) return
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

        val views = RemoteViews(context.packageName, R.layout.widget_sonata_player).apply {
            setTextViewText(R.id.widget_track_title, title)
            setTextViewText(R.id.widget_track_artist, artistLine)
            setTextViewText(R.id.widget_time_elapsed, formatClock(posMs))
            setTextViewText(R.id.widget_time_remaining, formatClock(remainingMs))
            setProgressBar(R.id.widget_progress, 100, progressPct, false)
            setImageViewResource(
                R.id.widget_btn_play_pause,
                if (isPlaying) R.drawable.widget_icon_pause else R.drawable.widget_icon_play,
            )

            val cachedBmp = if (artworkUrl != null && artworkUrl == cachedArtworkUrl) cachedArtworkBitmap else null
            if (cachedBmp != null) {
                setImageViewBitmap(R.id.widget_album_art, cachedBmp)
            } else {
                setImageViewResource(R.id.widget_album_art, R.drawable.ic_launcher_foreground_img)
            }

            // Open Sonata when tapping the widget frame
            val openIntent = Intent(context, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            }
            val openPending = PendingIntent.getActivity(
                context,
                0,
                openIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            setOnClickPendingIntent(R.id.widget_root, openPending)

            // Transport controls
            setOnClickPendingIntent(
                R.id.widget_btn_prev,
                broadcastPending(context, SonataWidgetProvider.ACTION_PREV, 101),
            )
            setOnClickPendingIntent(
                R.id.widget_btn_play_pause,
                broadcastPending(context, SonataWidgetProvider.ACTION_PLAY_PAUSE, 102),
            )
            setOnClickPendingIntent(
                R.id.widget_btn_next,
                broadcastPending(context, SonataWidgetProvider.ACTION_NEXT, 103),
            )
        }

        for (id in widgetIds) {
            manager.updateAppWidget(id, views)
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

    private fun broadcastPending(context: Context, action: String, reqCode: Int): PendingIntent {
        val intent = Intent(context, SonataWidgetProvider::class.java).apply {
            this.action = action
        }
        return PendingIntent.getBroadcast(
            context,
            reqCode,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private fun formatClock(ms: Long): String {
        val totalSec = (ms / 1000L).coerceAtLeast(0L)
        val mins = totalSec / 60L
        val secs = totalSec % 60L
        return "$mins:${secs.toString().padStart(2, '0')}"
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
