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
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.Rect
import android.graphics.RectF
import android.os.Handler
import android.os.Looper
import android.widget.RemoteViews
import dev.sonata.android.MainActivity
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.PlaybackQueueBridge
import dev.sonata.android.R
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.net.HttpURLConnection
import java.net.URL

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
    private const val KEY_TITLE = "title"
    private const val KEY_ARTIST = "artist"
    private const val KEY_ARTWORK_URL = "artwork_url"
    private const val KEY_IS_PLAYING = "is_playing"
    private const val KEY_POS_MS = "pos_ms"
    private const val KEY_DUR_MS = "dur_ms"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

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
        scope.launch {
            appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).edit()
                .putString(KEY_TITLE, title)
                .putString(KEY_ARTIST, artist)
                .putString(KEY_ARTWORK_URL, artworkUrl.orEmpty())
                .putBoolean(KEY_IS_PLAYING, isPlaying)
                .putLong(KEY_POS_MS, positionMs.coerceAtLeast(0L))
                .putLong(KEY_DUR_MS, durationMs.coerceAtLeast(0L))
                .apply()

            val manager = AppWidgetManager.getInstance(appContext)
            val ids = manager.getAppWidgetIds(ComponentName(appContext, SonataWidgetProvider::class.java))
            if (ids.isEmpty()) return@launch

            renderAll(appContext, manager, ids)

            val cleanUrl = artworkUrl?.takeIf { it.startsWith("http") }
            if (cleanUrl != null && cleanUrl != cachedArtworkUrl) {
                val bmp = fetchRoundedArtwork(cleanUrl, sizePx = 144)
                if (bmp != null) {
                    cachedArtworkUrl = cleanUrl
                    cachedArtworkBitmap = bmp
                    renderAll(appContext, manager, ids)
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
        val artist = prefs.getString(KEY_ARTIST, null)?.takeIf { it.isNotBlank() } ?: "Tap to play"
        val artworkUrl = prefs.getString(KEY_ARTWORK_URL, null)?.takeIf { it.isNotBlank() }
        val isPlaying = prefs.getBoolean(KEY_IS_PLAYING, false)
        val posMs = prefs.getLong(KEY_POS_MS, 0L).coerceAtLeast(0L)
        val durMs = prefs.getLong(KEY_DUR_MS, 0L).coerceAtLeast(0L)

        val progressPct = if (durMs > 0L) ((posMs * 100L) / durMs).toInt().coerceIn(0, 100) else 0
        val remainingMs = if (durMs > posMs) durMs - posMs else 0L

        val views = RemoteViews(context.packageName, R.layout.widget_sonata_player).apply {
            setTextViewText(R.id.widget_track_title, title)
            setTextViewText(R.id.widget_track_artist, "by $artist")
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

    private fun fetchRoundedArtwork(urlString: String, sizePx: Int): Bitmap? {
        var conn: HttpURLConnection? = null
        return try {
            conn = (URL(urlString).openConnection() as HttpURLConnection).apply {
                connectTimeout = 5_000
                readTimeout = 5_000
                instanceFollowRedirects = true
            }
            if (conn.responseCode !in 200..299) return null
            val raw = conn.inputStream.use { BitmapFactory.decodeStream(it) } ?: return null
            val scaled = Bitmap.createScaledBitmap(raw, sizePx, sizePx, true)
            if (scaled !== raw) raw.recycle()
            val out = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(out)
            val paint = Paint(Paint.ANTI_ALIAS_FLAG)
            val rect = Rect(0, 0, sizePx, sizePx)
            val rectF = RectF(rect)
            val radius = sizePx * 0.12f
            canvas.drawRoundRect(rectF, radius, radius, paint)
            paint.xfermode = PorterDuffXfermode(PorterDuff.Mode.SRC_IN)
            canvas.drawBitmap(scaled, rect, rect, paint)
            scaled.recycle()
            out
        } catch (_: Exception) {
            null
        } finally {
            conn?.disconnect()
        }
    }
}
