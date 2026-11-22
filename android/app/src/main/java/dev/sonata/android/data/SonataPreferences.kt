package dev.sonata.android.data

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.Immutable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject

/**
 * Application color theme preference for Sonata Android.
 */
enum class ThemeMode {
    SYSTEM,
    DARK,
    LIGHT,
    AMOLED,
}

/**
 * Full-screen Now Playing background rendering mode.
 */
enum class PlayerBackdropMode {
    ARTWORK_MESH,
    AMBIENT_WAVES,
    BLURRED_COVER,
}

/**
 * Preferred streaming audio quality tier.
 */
enum class AudioQualityTier {
    LOSSLESS,
    HIGH,
    MEDIUM,
    LOW,
}

/**
 * Preferred offline download quality tier.
 */
enum class DownloadQualityTier {
    HIGH,
    STANDARD,
}

/**
 * Immutable snapshot of all persisted user-facing preferences for Sonata Android.
 *
 * Contains no authentication tokens, cookies, or account secrets so it can be
 * safely serialized for settings backup and restore.
 */
@Immutable
data class SonataSettingsState(
    // Appearance & Glass
    val themeMode: ThemeMode = ThemeMode.DARK,
    val liquidGlassEnabled: Boolean = true,
    val glassPerformance: Boolean = false,
    val reduceDynamicBlur: Boolean = false,
    val reduceAnimation: Boolean = false,
    val animatedArtwork: Boolean = true,
    val artworkTintedPages: Boolean = true,
    val playerBackdropMode: PlayerBackdropMode = PlayerBackdropMode.ARTWORK_MESH,
    val fpsLimit: Int = 120,

    // Playback & Audio
    val audioQuality: AudioQualityTier = AudioQualityTier.HIGH,
    val preferHighQuality: Boolean = true,
    val crossfadeEnabled: Boolean = false,
    val crossfadeSeconds: Int = 4,
    val loudnessNormEnabled: Boolean = false,
    val skipSilenceEnabled: Boolean = false,
    val playbackSpeed: Float = 1.0f,
    val eqEnabled: Boolean = false,
    val eqPreset: String = "Flat",
    val eqBandsDb: List<Float> = listOf(0f, 0f, 0f, 0f, 0f),
    val autoplayEnabled: Boolean = true,

    // Lyrics
    val lyricsBlurEnabled: Boolean = true,
    val lyricsTranslationEnabled: Boolean = false,
    val lyricsTranslationLang: String = "en",
    val lyricsRomanizationEnabled: Boolean = false,
    val lyricsOffsetMs: Long = 0L,
    val prioritizeWordSync: Boolean = true,

    // Downloads
    val downloadQuality: DownloadQualityTier = DownloadQualityTier.HIGH,
    val wifiOnlyDownloads: Boolean = true,
    val exportDownloads: Boolean = false,
    val autoDownloadLiked: Boolean = false,
)

/**
 * Persistent preference store for Sonata Android backed by [SharedPreferences].
 *
 * Exposes a reactive [state] flow for Compose and playback services, atomic state
 * updates, per-track lyric timing offsets, and secret-free JSON backup/restore.
 */
class SonataPreferences(context: Context) {

    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    private val lock = Any()
    private var trackOffsetsCache: MutableMap<String, Long>? = null

    private val _state = MutableStateFlow(readState())
    val state: StateFlow<SonataSettingsState> = _state.asStateFlow()

    /**
     * Applies [transform] to the current [SonataSettingsState], normalizes bounded values,
     * persists all fields atomically to [SharedPreferences], and publishes the updated state.
     */
    fun update(transform: (SonataSettingsState) -> SonataSettingsState) {
        synchronized(lock) {
            val next = sanitize(transform(_state.value))
            if (next == _state.value) return
            writeState(next)
            _state.value = next
        }
    }

    /**
     * Returns the saved per-track lyric timing offset in milliseconds for [videoId],
     * or `0L` if no custom offset has been set for that track.
     */
    fun getTrackLyricOffsetMs(videoId: String): Long {
        if (videoId.isBlank()) return 0L
        synchronized(lock) {
            return getOrLoadTrackOffsetsLocked()[videoId] ?: 0L
        }
    }

    /**
     * Saves a per-track lyric timing offset in milliseconds for [videoId].
     * Passing `0L` clears the track-specific override.
     */
    fun setTrackLyricOffsetMs(videoId: String, offsetMs: Long) {
        if (videoId.isBlank()) return
        val clamped = offsetMs.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS)
        synchronized(lock) {
            val offsets = getOrLoadTrackOffsetsLocked()
            val prev = offsets[videoId] ?: 0L
            if (prev == clamped) return
            if (clamped == 0L) {
                offsets.remove(videoId)
            } else {
                offsets[videoId] = clamped
            }
            writeTrackOffsets(offsets)
        }
    }

    /**
     * Serializes all non-secret settings and per-track lyric offsets into a JSON string.
     */
    fun exportBackupJson(): String {
        val current: SonataSettingsState
        val offsets: Map<String, Long>
        synchronized(lock) {
            current = _state.value
            offsets = HashMap(getOrLoadTrackOffsetsLocked())
        }

        val settingsObj = JSONObject().apply {
            put(KEY_THEME_MODE, current.themeMode.name)
            put(KEY_LIQUID_GLASS_ENABLED, current.liquidGlassEnabled)
            put(KEY_GLASS_PERFORMANCE, current.glassPerformance)
            put(KEY_REDUCE_DYNAMIC_BLUR, current.reduceDynamicBlur)
            put(KEY_REDUCE_ANIMATION, current.reduceAnimation)
            put(KEY_ANIMATED_ARTWORK, current.animatedArtwork)
            put(KEY_ARTWORK_TINTED_PAGES, current.artworkTintedPages)
            put(KEY_PLAYER_BACKDROP_MODE, current.playerBackdropMode.name)
            put(KEY_FPS_LIMIT, current.fpsLimit)

            put(KEY_AUDIO_QUALITY, current.audioQuality.name)
            put(KEY_PREFER_HIGH_QUALITY, current.preferHighQuality)
            put(KEY_CROSSFADE_ENABLED, current.crossfadeEnabled)
            put(KEY_CROSSFADE_SECONDS, current.crossfadeSeconds)
            put(KEY_LOUDNESS_NORM_ENABLED, current.loudnessNormEnabled)
            put(KEY_SKIP_SILENCE_ENABLED, current.skipSilenceEnabled)
            put(KEY_PLAYBACK_SPEED, current.playbackSpeed.toDouble())
            put(KEY_EQ_ENABLED, current.eqEnabled)
            put(KEY_EQ_PRESET, current.eqPreset)
            put(KEY_EQ_BANDS_DB, JSONArray().apply { current.eqBandsDb.forEach { put(it.toDouble()) } })
            put(KEY_AUTOPLAY_ENABLED, current.autoplayEnabled)

            put(KEY_LYRICS_BLUR_ENABLED, current.lyricsBlurEnabled)
            put(KEY_LYRICS_TRANSLATION_ENABLED, current.lyricsTranslationEnabled)
            put(KEY_LYRICS_TRANSLATION_LANG, current.lyricsTranslationLang)
            put(KEY_LYRICS_ROMANIZATION_ENABLED, current.lyricsRomanizationEnabled)
            put(KEY_LYRICS_OFFSET_MS, current.lyricsOffsetMs)
            put(KEY_PRIORITIZE_WORD_SYNC, current.prioritizeWordSync)

            put(KEY_DOWNLOAD_QUALITY, current.downloadQuality.name)
            put(KEY_WIFI_ONLY_DOWNLOADS, current.wifiOnlyDownloads)
            put(KEY_EXPORT_DOWNLOADS, current.exportDownloads)
            put(KEY_AUTO_DOWNLOAD_LIKED, current.autoDownloadLiked)
        }

        val offsetsObj = JSONObject().apply {
            offsets.forEach { (id, offset) -> put(id, offset) }
        }

        val root = JSONObject().apply {
            put(BACKUP_KEY_APP, BACKUP_APP_ID)
            put(BACKUP_KEY_SCHEMA, BACKUP_SCHEMA_VERSION)
            put(BACKUP_KEY_EXPORTED_AT_MS, System.currentTimeMillis())
            put(BACKUP_KEY_SETTINGS, settingsObj)
            put(BACKUP_KEY_TRACK_OFFSETS, offsetsObj)
        }
        return root.toString(2)
    }

    /**
     * Restores settings and per-track lyric offsets from a JSON string created by
     * [exportBackupJson]. Returns `true` if the payload was valid and applied.
     */
    fun importBackupJson(json: String): Boolean {
        return runCatching {
            val root = JSONObject(json)
            if (root.optString(BACKUP_KEY_APP) != BACKUP_APP_ID) return false
            val schema = root.optInt(BACKUP_KEY_SCHEMA, -1)
            if (schema !in 1..BACKUP_SCHEMA_VERSION) return false

            val settingsObj = root.optJSONObject(BACKUP_KEY_SETTINGS) ?: return false
            val defaults = SonataSettingsState()

            val parsedBands = settingsObj.optJSONArray(KEY_EQ_BANDS_DB)?.let { array ->
                List(array.length()) { index -> array.optDouble(index, 0.0).toFloat() }
            } ?: defaults.eqBandsDb

            val restored = sanitize(
                SonataSettingsState(
                    themeMode = parseEnum(settingsObj.optString(KEY_THEME_MODE), defaults.themeMode),
                    liquidGlassEnabled = settingsObj.optBoolean(KEY_LIQUID_GLASS_ENABLED, defaults.liquidGlassEnabled),
                    glassPerformance = settingsObj.optBoolean(KEY_GLASS_PERFORMANCE, defaults.glassPerformance),
                    reduceDynamicBlur = settingsObj.optBoolean(KEY_REDUCE_DYNAMIC_BLUR, defaults.reduceDynamicBlur),
                    reduceAnimation = settingsObj.optBoolean(KEY_REDUCE_ANIMATION, defaults.reduceAnimation),
                    animatedArtwork = settingsObj.optBoolean(KEY_ANIMATED_ARTWORK, defaults.animatedArtwork),
                    artworkTintedPages = settingsObj.optBoolean(KEY_ARTWORK_TINTED_PAGES, defaults.artworkTintedPages),
                    playerBackdropMode = parseEnum(
                        settingsObj.optString(KEY_PLAYER_BACKDROP_MODE),
                        defaults.playerBackdropMode,
                    ),
                    fpsLimit = settingsObj.optInt(KEY_FPS_LIMIT, defaults.fpsLimit),

                    audioQuality = parseEnum(settingsObj.optString(KEY_AUDIO_QUALITY), defaults.audioQuality),
                    preferHighQuality = settingsObj.optBoolean(KEY_PREFER_HIGH_QUALITY, defaults.preferHighQuality),
                    crossfadeEnabled = settingsObj.optBoolean(KEY_CROSSFADE_ENABLED, defaults.crossfadeEnabled),
                    crossfadeSeconds = settingsObj.optInt(KEY_CROSSFADE_SECONDS, defaults.crossfadeSeconds),
                    loudnessNormEnabled = settingsObj.optBoolean(KEY_LOUDNESS_NORM_ENABLED, defaults.loudnessNormEnabled),
                    skipSilenceEnabled = settingsObj.optBoolean(KEY_SKIP_SILENCE_ENABLED, defaults.skipSilenceEnabled),
                    playbackSpeed = settingsObj.optDouble(KEY_PLAYBACK_SPEED, defaults.playbackSpeed.toDouble()).toFloat(),
                    eqEnabled = settingsObj.optBoolean(KEY_EQ_ENABLED, defaults.eqEnabled),
                    eqPreset = settingsObj.optString(KEY_EQ_PRESET, defaults.eqPreset).ifBlank { defaults.eqPreset },
                    eqBandsDb = parsedBands,
                    autoplayEnabled = settingsObj.optBoolean(KEY_AUTOPLAY_ENABLED, defaults.autoplayEnabled),

                    lyricsBlurEnabled = settingsObj.optBoolean(KEY_LYRICS_BLUR_ENABLED, defaults.lyricsBlurEnabled),
                    lyricsTranslationEnabled = settingsObj.optBoolean(
                        KEY_LYRICS_TRANSLATION_ENABLED,
                        defaults.lyricsTranslationEnabled,
                    ),
                    lyricsTranslationLang = settingsObj.optString(
                        KEY_LYRICS_TRANSLATION_LANG,
                        defaults.lyricsTranslationLang,
                    ).ifBlank { defaults.lyricsTranslationLang },
                    lyricsRomanizationEnabled = settingsObj.optBoolean(
                        KEY_LYRICS_ROMANIZATION_ENABLED,
                        defaults.lyricsRomanizationEnabled,
                    ),
                    lyricsOffsetMs = settingsObj.optLong(KEY_LYRICS_OFFSET_MS, defaults.lyricsOffsetMs),
                    prioritizeWordSync = settingsObj.optBoolean(KEY_PRIORITIZE_WORD_SYNC, defaults.prioritizeWordSync),

                    downloadQuality = parseEnum(settingsObj.optString(KEY_DOWNLOAD_QUALITY), defaults.downloadQuality),
                    wifiOnlyDownloads = settingsObj.optBoolean(KEY_WIFI_ONLY_DOWNLOADS, defaults.wifiOnlyDownloads),
                    exportDownloads = settingsObj.optBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads),
                    autoDownloadLiked = settingsObj.optBoolean(KEY_AUTO_DOWNLOAD_LIKED, defaults.autoDownloadLiked),
                ),
            )

            val restoredOffsets = mutableMapOf<String, Long>()
            root.optJSONObject(BACKUP_KEY_TRACK_OFFSETS)?.let { offsetsObj ->
                val keys = offsetsObj.keys()
                while (keys.hasNext()) {
                    val trackId = keys.next()
                    if (trackId.isNotBlank()) {
                        val value = offsetsObj.optLong(trackId, 0L)
                            .coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS)
                        if (value != 0L) {
                            restoredOffsets[trackId] = value
                        }
                    }
                }
            }

            synchronized(lock) {
                trackOffsetsCache = LinkedHashMap(restoredOffsets)
                writeState(restored, restoredOffsets)
                _state.value = restored
            }
            true
        }.getOrDefault(false)
    }

    private fun readState(): SonataSettingsState {
        val defaults = SonataSettingsState()
        val rawBands = prefs.getString(KEY_EQ_BANDS_DB, null)
        val parsedBands = parseBands(rawBands, defaults.eqBandsDb)

        return sanitize(
            SonataSettingsState(
                themeMode = parseEnum(prefs.getString(KEY_THEME_MODE, null), defaults.themeMode),
                liquidGlassEnabled = prefs.getBoolean(KEY_LIQUID_GLASS_ENABLED, defaults.liquidGlassEnabled),
                glassPerformance = prefs.getBoolean(KEY_GLASS_PERFORMANCE, defaults.glassPerformance),
                reduceDynamicBlur = prefs.getBoolean(KEY_REDUCE_DYNAMIC_BLUR, defaults.reduceDynamicBlur),
                reduceAnimation = prefs.getBoolean(KEY_REDUCE_ANIMATION, defaults.reduceAnimation),
                animatedArtwork = prefs.getBoolean(KEY_ANIMATED_ARTWORK, defaults.animatedArtwork),
                artworkTintedPages = prefs.getBoolean(KEY_ARTWORK_TINTED_PAGES, defaults.artworkTintedPages),
                playerBackdropMode = parseEnum(
                    prefs.getString(KEY_PLAYER_BACKDROP_MODE, null),
                    defaults.playerBackdropMode,
                ),
                fpsLimit = prefs.getInt(KEY_FPS_LIMIT, defaults.fpsLimit),

                audioQuality = parseEnum(prefs.getString(KEY_AUDIO_QUALITY, null), defaults.audioQuality),
                preferHighQuality = prefs.getBoolean(KEY_PREFER_HIGH_QUALITY, defaults.preferHighQuality),
                crossfadeEnabled = prefs.getBoolean(KEY_CROSSFADE_ENABLED, defaults.crossfadeEnabled),
                crossfadeSeconds = prefs.getInt(KEY_CROSSFADE_SECONDS, defaults.crossfadeSeconds),
                loudnessNormEnabled = prefs.getBoolean(KEY_LOUDNESS_NORM_ENABLED, defaults.loudnessNormEnabled),
                skipSilenceEnabled = prefs.getBoolean(KEY_SKIP_SILENCE_ENABLED, defaults.skipSilenceEnabled),
                playbackSpeed = prefs.getFloat(KEY_PLAYBACK_SPEED, defaults.playbackSpeed),
                eqEnabled = prefs.getBoolean(KEY_EQ_ENABLED, defaults.eqEnabled),
                eqPreset = prefs.getString(KEY_EQ_PRESET, defaults.eqPreset)
                    ?.takeIf { it.isNotBlank() }
                    ?: defaults.eqPreset,
                eqBandsDb = parsedBands,
                autoplayEnabled = prefs.getBoolean(KEY_AUTOPLAY_ENABLED, defaults.autoplayEnabled),

                lyricsBlurEnabled = prefs.getBoolean(KEY_LYRICS_BLUR_ENABLED, defaults.lyricsBlurEnabled),
                lyricsTranslationEnabled = prefs.getBoolean(
                    KEY_LYRICS_TRANSLATION_ENABLED,
                    defaults.lyricsTranslationEnabled,
                ),
                lyricsTranslationLang = prefs.getString(KEY_LYRICS_TRANSLATION_LANG, defaults.lyricsTranslationLang)
                    ?.takeIf { it.isNotBlank() }
                    ?: defaults.lyricsTranslationLang,
                lyricsRomanizationEnabled = prefs.getBoolean(
                    KEY_LYRICS_ROMANIZATION_ENABLED,
                    defaults.lyricsRomanizationEnabled,
                ),
                lyricsOffsetMs = prefs.getLong(KEY_LYRICS_OFFSET_MS, defaults.lyricsOffsetMs),
                prioritizeWordSync = prefs.getBoolean(KEY_PRIORITIZE_WORD_SYNC, defaults.prioritizeWordSync),

                downloadQuality = parseEnum(prefs.getString(KEY_DOWNLOAD_QUALITY, null), defaults.downloadQuality),
                wifiOnlyDownloads = prefs.getBoolean(KEY_WIFI_ONLY_DOWNLOADS, defaults.wifiOnlyDownloads),
                exportDownloads = prefs.getBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads),
                autoDownloadLiked = prefs.getBoolean(KEY_AUTO_DOWNLOAD_LIKED, defaults.autoDownloadLiked),
            ),
        )
    }

    private fun writeState(
        state: SonataSettingsState,
        trackOffsetsOverride: Map<String, Long>? = null,
    ) {
        val editor = prefs.edit()
            .putString(KEY_THEME_MODE, state.themeMode.name)
            .putBoolean(KEY_LIQUID_GLASS_ENABLED, state.liquidGlassEnabled)
            .putBoolean(KEY_GLASS_PERFORMANCE, state.glassPerformance)
            .putBoolean(KEY_REDUCE_DYNAMIC_BLUR, state.reduceDynamicBlur)
            .putBoolean(KEY_REDUCE_ANIMATION, state.reduceAnimation)
            .putBoolean(KEY_ANIMATED_ARTWORK, state.animatedArtwork)
            .putBoolean(KEY_ARTWORK_TINTED_PAGES, state.artworkTintedPages)
            .putString(KEY_PLAYER_BACKDROP_MODE, state.playerBackdropMode.name)
            .putInt(KEY_FPS_LIMIT, state.fpsLimit)
            .putString(KEY_AUDIO_QUALITY, state.audioQuality.name)
            .putBoolean(KEY_PREFER_HIGH_QUALITY, state.preferHighQuality)
            .putBoolean(KEY_CROSSFADE_ENABLED, state.crossfadeEnabled)
            .putInt(KEY_CROSSFADE_SECONDS, state.crossfadeSeconds)
            .putBoolean(KEY_LOUDNESS_NORM_ENABLED, state.loudnessNormEnabled)
            .putBoolean(KEY_SKIP_SILENCE_ENABLED, state.skipSilenceEnabled)
            .putFloat(KEY_PLAYBACK_SPEED, state.playbackSpeed)
            .putBoolean(KEY_EQ_ENABLED, state.eqEnabled)
            .putString(KEY_EQ_PRESET, state.eqPreset)
            .putString(KEY_EQ_BANDS_DB, encodeBands(state.eqBandsDb))
            .putBoolean(KEY_AUTOPLAY_ENABLED, state.autoplayEnabled)
            .putBoolean(KEY_LYRICS_BLUR_ENABLED, state.lyricsBlurEnabled)
            .putBoolean(KEY_LYRICS_TRANSLATION_ENABLED, state.lyricsTranslationEnabled)
            .putString(KEY_LYRICS_TRANSLATION_LANG, state.lyricsTranslationLang)
            .putBoolean(KEY_LYRICS_ROMANIZATION_ENABLED, state.lyricsRomanizationEnabled)
            .putLong(KEY_LYRICS_OFFSET_MS, state.lyricsOffsetMs)
            .putBoolean(KEY_PRIORITIZE_WORD_SYNC, state.prioritizeWordSync)
            .putString(KEY_DOWNLOAD_QUALITY, state.downloadQuality.name)
            .putBoolean(KEY_WIFI_ONLY_DOWNLOADS, state.wifiOnlyDownloads)
            .putBoolean(KEY_EXPORT_DOWNLOADS, state.exportDownloads)
            .putBoolean(KEY_AUTO_DOWNLOAD_LIKED, state.autoDownloadLiked)

        if (trackOffsetsOverride != null) {
            editor.putString(KEY_TRACK_LYRIC_OFFSETS, encodeTrackOffsets(trackOffsetsOverride))
        }

        editor.apply()
    }

    private fun getOrLoadTrackOffsetsLocked(): MutableMap<String, Long> {
        trackOffsetsCache?.let { return it }
        return LinkedHashMap(readTrackOffsets()).also { trackOffsetsCache = it }
    }

    private fun readTrackOffsets(): Map<String, Long> {
        val raw = prefs.getString(KEY_TRACK_LYRIC_OFFSETS, null) ?: return emptyMap()
        return runCatching {
            val obj = JSONObject(raw)
            val result = LinkedHashMap<String, Long>()
            val keys = obj.keys()
            while (keys.hasNext()) {
                val id = keys.next()
                val value = obj.optLong(id, 0L)
                if (id.isNotBlank() && value != 0L) {
                    result[id] = value.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS)
                }
            }
            result
        }.getOrDefault(emptyMap())
    }

    private fun writeTrackOffsets(offsets: Map<String, Long>) {
        prefs.edit()
            .putString(KEY_TRACK_LYRIC_OFFSETS, encodeTrackOffsets(offsets))
            .apply()
    }

    private fun encodeTrackOffsets(offsets: Map<String, Long>): String {
        val obj = JSONObject()
        offsets.forEach { (id, offset) ->
            if (id.isNotBlank() && offset != 0L) {
                obj.put(id, offset.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS))
            }
        }
        return obj.toString()
    }

    private fun sanitize(state: SonataSettingsState): SonataSettingsState {
        val normalizedBands = List(EQ_BAND_COUNT) { index ->
            val raw = state.eqBandsDb.getOrElse(index) { 0f }
            if (raw.isFinite()) raw.coerceIn(MIN_EQ_DB, MAX_EQ_DB) else 0f
        }
        val normalizedSpeed = if (state.playbackSpeed.isFinite()) {
            state.playbackSpeed.coerceIn(MIN_SPEED, MAX_SPEED)
        } else {
            1.0f
        }
        val normalizedFps = when {
            state.fpsLimit <= 60 -> 60
            state.fpsLimit <= 90 -> 90
            else -> 120
        }
        return state.copy(
            fpsLimit = normalizedFps,
            crossfadeSeconds = state.crossfadeSeconds.coerceIn(MIN_CROSSFADE_SEC, MAX_CROSSFADE_SEC),
            playbackSpeed = normalizedSpeed,
            eqPreset = state.eqPreset.trim().ifEmpty { "Flat" },
            eqBandsDb = normalizedBands,
            lyricsTranslationLang = state.lyricsTranslationLang.trim().lowercase().ifEmpty { "en" },
            lyricsOffsetMs = state.lyricsOffsetMs.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS),
        )
    }

    private fun encodeBands(bands: List<Float>): String =
        JSONArray().apply { bands.forEach { put(it.toDouble()) } }.toString()

    private fun parseBands(raw: String?, fallback: List<Float>): List<Float> {
        if (raw.isNullOrBlank()) return fallback
        return runCatching {
            val array = JSONArray(raw)
            List(EQ_BAND_COUNT) { index ->
                if (index < array.length()) array.optDouble(index, 0.0).toFloat() else 0f
            }
        }.getOrDefault(fallback)
    }

    private inline fun <reified T : Enum<T>> parseEnum(raw: String?, fallback: T): T {
        if (raw.isNullOrBlank()) return fallback
        return runCatching { enumValueOf<T>(raw.trim().uppercase()) }.getOrDefault(fallback)
    }

    companion object {
        private const val PREFS_NAME = "sonata_preferences"

        private const val EQ_BAND_COUNT = 5
        private const val MIN_EQ_DB = -12f
        private const val MAX_EQ_DB = 12f
        private const val MIN_SPEED = 0.5f
        private const val MAX_SPEED = 2.0f
        private const val MIN_CROSSFADE_SEC = 1
        private const val MAX_CROSSFADE_SEC = 12
        private const val MIN_LYRIC_OFFSET_MS = -5_000L
        private const val MAX_LYRIC_OFFSET_MS = 5_000L

        private const val BACKUP_APP_ID = "sonata"
        private const val BACKUP_SCHEMA_VERSION = 1
        private const val BACKUP_KEY_APP = "app"
        private const val BACKUP_KEY_SCHEMA = "schemaVersion"
        private const val BACKUP_KEY_EXPORTED_AT_MS = "exportedAtMs"
        private const val BACKUP_KEY_SETTINGS = "settings"
        private const val BACKUP_KEY_TRACK_OFFSETS = "trackLyricOffsets"

        private const val KEY_THEME_MODE = "theme_mode"
        private const val KEY_LIQUID_GLASS_ENABLED = "liquid_glass_enabled"
        private const val KEY_GLASS_PERFORMANCE = "glass_performance"
        private const val KEY_REDUCE_DYNAMIC_BLUR = "reduce_dynamic_blur"
        private const val KEY_REDUCE_ANIMATION = "reduce_animation"
        private const val KEY_ANIMATED_ARTWORK = "animated_artwork"
        private const val KEY_ARTWORK_TINTED_PAGES = "artwork_tinted_pages"
        private const val KEY_PLAYER_BACKDROP_MODE = "player_backdrop_mode"
        private const val KEY_FPS_LIMIT = "fps_limit"

        private const val KEY_AUDIO_QUALITY = "audio_quality"
        private const val KEY_PREFER_HIGH_QUALITY = "prefer_high_quality"
        private const val KEY_CROSSFADE_ENABLED = "crossfade_enabled"
        private const val KEY_CROSSFADE_SECONDS = "crossfade_seconds"
        private const val KEY_LOUDNESS_NORM_ENABLED = "loudness_norm_enabled"
        private const val KEY_SKIP_SILENCE_ENABLED = "skip_silence_enabled"
        private const val KEY_PLAYBACK_SPEED = "playback_speed"
        private const val KEY_EQ_ENABLED = "eq_enabled"
        private const val KEY_EQ_PRESET = "eq_preset"
        private const val KEY_EQ_BANDS_DB = "eq_bands_db"
        private const val KEY_AUTOPLAY_ENABLED = "autoplay_enabled"

        private const val KEY_LYRICS_BLUR_ENABLED = "lyrics_blur_enabled"
        private const val KEY_LYRICS_TRANSLATION_ENABLED = "lyrics_translation_enabled"
        private const val KEY_LYRICS_TRANSLATION_LANG = "lyrics_translation_lang"
        private const val KEY_LYRICS_ROMANIZATION_ENABLED = "lyrics_romanization_enabled"
        private const val KEY_LYRICS_OFFSET_MS = "lyrics_offset_ms"
        private const val KEY_PRIORITIZE_WORD_SYNC = "prioritize_word_sync"

        private const val KEY_DOWNLOAD_QUALITY = "download_quality"
        private const val KEY_WIFI_ONLY_DOWNLOADS = "wifi_only_downloads"
        private const val KEY_EXPORT_DOWNLOADS = "export_downloads"
        private const val KEY_AUTO_DOWNLOAD_LIKED = "auto_download_liked"

        private const val KEY_TRACK_LYRIC_OFFSETS = "track_lyric_offsets"
    }
}
