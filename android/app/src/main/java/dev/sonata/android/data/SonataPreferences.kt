package dev.sonata.android.data

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.Immutable
import dev.sonata.android.discord.DiscordActivityType
import dev.sonata.android.discord.DiscordApplication
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/**
 * Application UI language preference for Sonata Android.
 * Defaults to [SYSTEM], automatically following the Android device locale.
 */
enum class AppLanguage {
    SYSTEM,
    ENGLISH,
    TURKISH,
    RUSSIAN,
}

/**
 * Structural & aesthetic design theme family for Sonata Android (monochrome Basic theme).
 */
enum class DesignTheme {
    BASIC,
}

/**
 * Application color theme preference for Sonata Android.
 */
enum class ThemeMode {
    SYSTEM,
    LIGHT,
    DARK,
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
 * Preferred maximum video resolution when switching Now Playing into Video mode.
 */
enum class VideoQualityPreference(
    val label: String,
    val maxHeight: Int?,
) {
    AUTO(label = "Auto", maxHeight = 1080),
    P1080(label = "1080p", maxHeight = 1080),
    P720(label = "720p", maxHeight = 720),
    P480(label = "480p", maxHeight = 480),
    P360(label = "360p", maxHeight = 360),
}

/**
 * Preferred video resolution when downloading tracks with companion music videos offline.
 */
enum class VideoDownloadQuality(
    val maxHeight: Int,
    val approxMbPerVideo: String,
) {
    P1080(maxHeight = 1080, approxMbPerVideo = "~45 MB / video"),
    P720(maxHeight = 720, approxMbPerVideo = "~22 MB / video"),
    P480(maxHeight = 480, approxMbPerVideo = "~12 MB / video"),
    P360(maxHeight = 360, approxMbPerVideo = "~7 MB / video"),
    ;

    fun toVideoQualityPreference(): VideoQualityPreference = when (this) {
        P1080 -> VideoQualityPreference.P1080
        P720 -> VideoQualityPreference.P720
        P480 -> VideoQualityPreference.P480
        P360 -> VideoQualityPreference.P360
    }
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
 * Preferred offline download quality tier, ordered from highest to lowest quality
 * with average per-song storage estimates (for a typical ~3.5-minute track).
 */
enum class DownloadQualityTier(
    val title: String,
    val detail: String,
    val approxMbPerSong: String,
    val maxBitrateKbps: Int,
) {
    HIGHEST(
        title = "Highest (320 kbps / Extreme)",
        detail = "Best available AAC 320 kbps or Extreme Opus",
        approxMbPerSong = "~8.5 MB / song",
        maxBitrateKbps = 320,
    ),
    HIGH(
        title = "High (160–256 kbps)",
        detail = "High-bitrate Opus / AAC",
        approxMbPerSong = "~5.5 MB / song",
        maxBitrateKbps = 256,
    ),
    STANDARD(
        title = "Standard (128 kbps)",
        detail = "Balanced audio quality and storage use",
        approxMbPerSong = "~3.6 MB / song",
        maxBitrateKbps = 128,
    ),
    LOW(
        title = "Data Saver (64 kbps)",
        detail = "Smallest file size for large offline libraries",
        approxMbPerSong = "~1.8 MB / song",
        maxBitrateKbps = 64,
    ),
}

/**
 * Target storage location for offline downloaded tracks.
 */
enum class DownloadStorageLocation(
    val title: String,
    val detail: String,
) {
    INTERNAL(
        title = "Internal Storage",
        detail = "App-private storage (hidden from other apps)",
    ),
    EXTERNAL(
        title = "External Storage",
        detail = "Shared Music/Sonata folder visible to file managers",
    ),
}

/**
 * Immutable snapshot of all persisted user-facing preferences for Sonata Android.
 *
 * Contains no authentication tokens, cookies, or account secrets so it can be
 * safely serialized for settings backup and restore.
 */
@Immutable
data class SonataSettingsState(
    // Language & Appearance
    val appLanguage: AppLanguage = AppLanguage.SYSTEM,
    val designTheme: DesignTheme = DesignTheme.BASIC,
    val themeMode: ThemeMode = ThemeMode.SYSTEM,
    val liquidGlassEnabled: Boolean = true,
    val glassPerformance: Boolean = false,
    val reduceDynamicBlur: Boolean = false,
    val reduceAnimation: Boolean = false,
    val animatedArtwork: Boolean = true,
    val animatedArtworkCellular: Boolean = true,
    val artworkTintedPages: Boolean = true,
    val fullScreenCoverArt: Boolean = true,
    val fullScreenDetailCover: Boolean = false,
    val showTotalDuration: Boolean = false,
    val hideLyricsPreview: Boolean = false,
    val playerBackdropMode: PlayerBackdropMode = PlayerBackdropMode.ARTWORK_MESH,
    val fpsLimit: Int = 120,

    // Content Preferences
    val hideRecents: Boolean = false,
    val hideTopPicks: Boolean = false,
    val hideHomeMoods: Boolean = false,
    val hideHomeVideos: Boolean = false,
    val squareVideoThumbnails: Boolean = false,

    // Playback & Audio
    val audioQuality: AudioQualityTier = AudioQualityTier.HIGH,
    val preferHighQuality: Boolean = true,
    val gaplessEnabled: Boolean = true,
    val crossfadeEnabled: Boolean = false,
    val crossfadeSeconds: Int = 4,
    val loudnessNormEnabled: Boolean = false,
    val skipSilenceEnabled: Boolean = false,
    val sleepTimerFadeOut: Boolean = true,
    val playbackSpeed: Float = 1.0f,
    val eqEnabled: Boolean = false,
    val eqPreset: String = "Flat",
    val eqBandsDb: List<Float> = listOf(0f, 0f, 0f, 0f, 0f),
    val autoplayEnabled: Boolean = true,
    val shuffleEnabled: Boolean = false,
    val repeatMode: Int = 0,

    // Lyrics
    val lyricsBlurEnabled: Boolean = true,
    val lyricsTranslationEnabled: Boolean = false,
    val lyricsTranslationLang: String = "en",
    val lyricsRomanizationEnabled: Boolean = false,
    val lyricsOffsetMs: Long = 0L,
    val prioritizeWordSync: Boolean = true,

    // Downloads
    val downloadQuality: DownloadQualityTier = DownloadQualityTier.HIGH,
    val downloadIncludeJioSaavn: Boolean = true,
    val downloadStorageLocation: DownloadStorageLocation = DownloadStorageLocation.INTERNAL,
    val wifiOnlyDownloads: Boolean = true,
    val exportDownloads: Boolean = false,
    val autoDownloadLiked: Boolean = false,
    val downloadVideosIfAvailable: Boolean = false,
    val videoDownloadQuality: VideoDownloadQuality = VideoDownloadQuality.P720,

    // Miscellaneous
    val hideStatusBarInPlayer: Boolean = false,
    val hideSongStatus: Boolean = false,
    val stopMusicOnClose: Boolean = false,
    val showNerdStats: Boolean = false,

    // Modules & Sources
    val jioSaavnEnabled: Boolean = false,
    val personalOnlyMode: Boolean = false,
    val localMusicEnabled: Boolean = true,
    val excludeNonMusicAudio: Boolean = true,
    val localMusicMinDurationSecs: Int = 30,
    val localMusicFolders: Set<String> = emptySet(),
    val attributePersonalToYtm: Boolean = false,

    // Music Video Mode
    val suppressVideoAudioQualityWarning: Boolean = false,
    val videoQuality: VideoQualityPreference = VideoQualityPreference.AUTO,

    // Discord Rich Presence (non-secret configuration only; token lives in isolated private storage)
    val discordRpcEnabled: Boolean = false,
    val discordAppId: String = DiscordApplication.DEFAULT_APP_ID,
    val discordActivityType: DiscordActivityType = DiscordActivityType.LISTENING,
    val discordShowTimestamps: Boolean = true,
    val discordShowArtwork: Boolean = true,
    val discordShowButtons: Boolean = true,

    // Sonata Connect & Scrobbling (non-secret flags & device label only; tokens/keys stay isolated)
    val connectEnabled: Boolean = false,
    val connectDeviceName: String = SonataPreferences.DEFAULT_CONNECT_DEVICE_NAME,
    val lastFmEnabled: Boolean = false,
    val listenBrainzEnabled: Boolean = false,
) {
    fun resolvedDesignTheme(): DesignTheme = DesignTheme.BASIC

    fun isDarkActive(systemDark: Boolean): Boolean = when (themeMode) {
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
        ThemeMode.SYSTEM -> systemDark
    }

    fun usesExternalDownloadStorage(): Boolean =
        downloadStorageLocation == DownloadStorageLocation.EXTERNAL || exportDownloads

    fun isStreamingEnabled(): Boolean = SonataPreferences.isStreamingEnabled(personalOnlyMode)

    fun effectiveJioSaavnEnabled(): Boolean =
        SonataPreferences.effectiveJioSaavnEnabled(personalOnlyMode, jioSaavnEnabled)
}

/**
 * Persistent preference store for Sonata Android backed by [SharedPreferences].
 *
 * Exposes a reactive [state] flow for Compose and playback services, atomic state
 * updates, per-track lyric timing offsets, and secret-free JSON backup/restore.
 */
class SonataPreferences(context: Context) {

    private val appContext = context.applicationContext ?: context
    private val prefs: SharedPreferences by lazy {
        appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }
    private val discordAuthPrefs: SharedPreferences by lazy {
        appContext.getSharedPreferences(DISCORD_AUTH_PREFS_NAME, Context.MODE_PRIVATE)
    }

    private val lock = Any()
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    @Volatile
    private var stateLoaded = false
    private var trackOffsetsCache: MutableMap<String, Long>? = null

    private val _state = MutableStateFlow(SonataSettingsState())
    val state: StateFlow<SonataSettingsState> = _state.asStateFlow()

    private val _personalOnlyMode = MutableStateFlow(false)
    val personalOnlyMode: StateFlow<Boolean> = _personalOnlyMode.asStateFlow()

    private val _localMusicEnabled = MutableStateFlow(true)
    val localMusicEnabled: StateFlow<Boolean> = _localMusicEnabled.asStateFlow()

    private val _attributePersonalToYtm = MutableStateFlow(false)
    val attributePersonalToYtm: StateFlow<Boolean> = _attributePersonalToYtm.asStateFlow()

    private val _gaplessEnabled = MutableStateFlow(true)
    val gaplessEnabled: StateFlow<Boolean> = _gaplessEnabled.asStateFlow()

    private val _crossfadeSeconds = MutableStateFlow(0)
    val crossfadeSeconds: StateFlow<Int> = _crossfadeSeconds.asStateFlow()

    private val _connectEnabled = MutableStateFlow(false)
    val connectEnabled: StateFlow<Boolean> = _connectEnabled.asStateFlow()

    private val _connectDeviceName = MutableStateFlow(DEFAULT_CONNECT_DEVICE_NAME)
    val connectDeviceName: StateFlow<String> = _connectDeviceName.asStateFlow()

    private val _lastFmEnabled = MutableStateFlow(false)
    val lastFmEnabled: StateFlow<Boolean> = _lastFmEnabled.asStateFlow()

    private val _listenBrainzEnabled = MutableStateFlow(false)
    val listenBrainzEnabled: StateFlow<Boolean> = _listenBrainzEnabled.asStateFlow()

    private val _discordToken = MutableStateFlow<String?>(null)
    val discordToken: StateFlow<String?> = _discordToken.asStateFlow()

    init {
        ioScope.launch {
            ensureStateLoaded()
        }
    }

    private fun ensureStateLoaded() {
        if (stateLoaded) return
        synchronized(lock) {
            ensureStateLoadedLocked()
        }
    }

    /**
     * Ensures persisted preferences have been loaded from [SharedPreferences] and returns the
     * current snapshot. Call from a background thread (e.g. `Dispatchers.IO`).
     */
    fun ensureLoadedState(): SonataSettingsState {
        ensureStateLoaded()
        return _state.value
    }

    private fun ensureStateLoadedLocked() {
        if (stateLoaded) return
        publishStateFlowsLocked(readState())
        _discordToken.value = readDiscordToken()
        stateLoaded = true
    }

    private fun publishStateFlowsLocked(next: SonataSettingsState) {
        _state.value = next
        _personalOnlyMode.value = next.personalOnlyMode
        _localMusicEnabled.value = next.localMusicEnabled
        _attributePersonalToYtm.value = next.attributePersonalToYtm
        _gaplessEnabled.value = next.gaplessEnabled
        _crossfadeSeconds.value = if (next.crossfadeEnabled) {
            next.crossfadeSeconds.coerceIn(MIN_CROSSFADE_SEC, MAX_CROSSFADE_SEC)
        } else {
            0
        }
        _connectEnabled.value = next.connectEnabled
        _connectDeviceName.value = next.connectDeviceName
        _lastFmEnabled.value = next.lastFmEnabled
        _listenBrainzEnabled.value = next.listenBrainzEnabled
    }

    /**
     * Returns the privately stored Discord user token, or null if not signed in.
     * Never included in [exportBackupJson].
     */
    fun getDiscordToken(): String? {
        synchronized(lock) {
            ensureStateLoadedLocked()
            return _discordToken.value
        }
    }

    /**
     * Persists or clears the Discord user token in dedicated private [SharedPreferences].
     */
    fun setDiscordToken(token: String?) {
        val cleaned = DiscordApplication.sanitizeToken(token)
        synchronized(lock) {
            ensureStateLoadedLocked()
            val editor = discordAuthPrefs.edit()
            if (cleaned == null) {
                editor.remove(KEY_DISCORD_AUTH_SECRET)
            } else {
                editor.putString(KEY_DISCORD_AUTH_SECRET, cleaned)
            }
            editor.apply()
            _discordToken.value = cleaned
        }
    }

    fun clearDiscordToken() {
        setDiscordToken(null)
    }

    fun hasDiscordToken(): Boolean = !getDiscordToken().isNullOrBlank()

    private fun readDiscordToken(): String? =
        DiscordApplication.sanitizeToken(discordAuthPrefs.getString(KEY_DISCORD_AUTH_SECRET, null))

    /**
     * Applies [transform] to the current [SonataSettingsState], normalizes bounded values,
     * persists all fields atomically to [SharedPreferences], and publishes the updated state.
     */
    fun update(transform: (SonataSettingsState) -> SonataSettingsState) {
        synchronized(lock) {
            ensureStateLoadedLocked()
            val next = sanitize(transform(_state.value))
            if (next == _state.value) return
            writeState(next)
            publishStateFlowsLocked(next)
        }
    }

    /**
     * Toggles Personal Only Mode without mutating streaming module flags, cookies,
     * or audio/playback preferences so switching back restores the prior setup intact.
     */
    fun setPersonalOnlyMode(enabled: Boolean) {
        update { it.copy(personalOnlyMode = enabled) }
    }

    /**
     * Enables or disables local device audio library scanning.
     */
    fun setLocalMusicEnabled(enabled: Boolean) {
        update { it.copy(localMusicEnabled = enabled) }
    }

    /**
     * Enables or disables optional YouTube Music playback attribution for personal tracks.
     */
    fun setAttributePersonalToYtm(enabled: Boolean) {
        update { it.copy(attributePersonalToYtm = enabled) }
    }

    /**
     * Enables or disables gapless audio transitions across consecutive queued tracks.
     */
    fun setGaplessEnabled(enabled: Boolean) {
        update { it.copy(gaplessEnabled = enabled) }
    }

    /**
     * Sets effective crossfade duration in seconds (`0` disables crossfade; `1..12` enables it).
     */
    fun setCrossfadeSeconds(seconds: Int) {
        val clamped = seconds.coerceIn(0, MAX_CROSSFADE_SEC)
        update { current ->
            if (clamped == 0) {
                current.copy(crossfadeEnabled = false)
            } else {
                current.copy(crossfadeEnabled = true, crossfadeSeconds = clamped)
            }
        }
    }

    /**
     * Enables or disables the Sonata Connect LAN client.
     */
    fun setConnectEnabled(enabled: Boolean) {
        update { it.copy(connectEnabled = enabled) }
    }

    /**
     * Updates the user-visible device name advertised to Sonata Connect peers.
     */
    fun setConnectDeviceName(name: String) {
        val normalized = normalizeConnectDeviceName(name)
        update { it.copy(connectDeviceName = normalized) }
    }

    /**
     * Enables or disables Last.fm scrobbling.
     */
    fun setLastFmEnabled(enabled: Boolean) {
        update { it.copy(lastFmEnabled = enabled) }
    }

    /**
     * Enables or disables ListenBrainz scrobbling.
     */
    fun setListenBrainzEnabled(enabled: Boolean) {
        update { it.copy(listenBrainzEnabled = enabled) }
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
            ensureStateLoadedLocked()
            current = _state.value
            offsets = HashMap(getOrLoadTrackOffsetsLocked())
        }
        return formatBackupJson(current, offsets)
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
                    appLanguage = parseEnum(settingsObj.optString(KEY_APP_LANGUAGE), defaults.appLanguage),
                    designTheme = parseEnum(settingsObj.optString(KEY_DESIGN_THEME), defaults.designTheme),
                    themeMode = parseThemeMode(settingsObj.optString(KEY_THEME_MODE), defaults.themeMode),
                    liquidGlassEnabled = settingsObj.optBoolean(KEY_LIQUID_GLASS_ENABLED, defaults.liquidGlassEnabled),
                    glassPerformance = settingsObj.optBoolean(KEY_GLASS_PERFORMANCE, defaults.glassPerformance),
                    reduceDynamicBlur = settingsObj.optBoolean(KEY_REDUCE_DYNAMIC_BLUR, defaults.reduceDynamicBlur),
                    reduceAnimation = settingsObj.optBoolean(KEY_REDUCE_ANIMATION, defaults.reduceAnimation),
                    animatedArtwork = settingsObj.optBoolean(KEY_ANIMATED_ARTWORK, defaults.animatedArtwork),
                    animatedArtworkCellular = settingsObj.optBoolean(
                        KEY_ANIMATED_ARTWORK_CELLULAR,
                        defaults.animatedArtworkCellular,
                    ),
                    artworkTintedPages = settingsObj.optBoolean(KEY_ARTWORK_TINTED_PAGES, defaults.artworkTintedPages),
                    fullScreenCoverArt = settingsObj.optBoolean(KEY_FULL_SCREEN_COVER_ART, defaults.fullScreenCoverArt),
                    fullScreenDetailCover = settingsObj.optBoolean(
                        KEY_FULL_SCREEN_DETAIL_COVER,
                        defaults.fullScreenDetailCover,
                    ),
                    showTotalDuration = settingsObj.optBoolean(KEY_SHOW_TOTAL_DURATION, defaults.showTotalDuration),
                    hideLyricsPreview = settingsObj.optBoolean(KEY_HIDE_LYRICS_PREVIEW, defaults.hideLyricsPreview),
                    playerBackdropMode = parseEnum(
                        settingsObj.optString(KEY_PLAYER_BACKDROP_MODE),
                        defaults.playerBackdropMode,
                    ),
                    fpsLimit = settingsObj.optInt(KEY_FPS_LIMIT, defaults.fpsLimit),

                    hideRecents = settingsObj.optBoolean(KEY_HIDE_RECENTS, defaults.hideRecents),
                    hideTopPicks = settingsObj.optBoolean(KEY_HIDE_TOP_PICKS, defaults.hideTopPicks),
                    hideHomeMoods = settingsObj.optBoolean(KEY_HIDE_HOME_MOODS, defaults.hideHomeMoods),
                    hideHomeVideos = settingsObj.optBoolean(KEY_HIDE_HOME_VIDEOS, defaults.hideHomeVideos),
                    squareVideoThumbnails = settingsObj.optBoolean(
                        KEY_SQUARE_VIDEO_THUMBNAILS,
                        defaults.squareVideoThumbnails,
                    ),

                    audioQuality = parseEnum(settingsObj.optString(KEY_AUDIO_QUALITY), defaults.audioQuality),
                    preferHighQuality = settingsObj.optBoolean(KEY_PREFER_HIGH_QUALITY, defaults.preferHighQuality),
                    gaplessEnabled = settingsObj.optBoolean(KEY_GAPLESS_ENABLED, defaults.gaplessEnabled),
                    crossfadeEnabled = settingsObj.optBoolean(KEY_CROSSFADE_ENABLED, defaults.crossfadeEnabled),
                    crossfadeSeconds = settingsObj.optInt(KEY_CROSSFADE_SECONDS, defaults.crossfadeSeconds),
                    loudnessNormEnabled = settingsObj.optBoolean(KEY_LOUDNESS_NORM_ENABLED, defaults.loudnessNormEnabled),
                    skipSilenceEnabled = settingsObj.optBoolean(KEY_SKIP_SILENCE_ENABLED, defaults.skipSilenceEnabled),
                    sleepTimerFadeOut = settingsObj.optBoolean(KEY_SLEEP_TIMER_FADE_OUT, defaults.sleepTimerFadeOut),
                    playbackSpeed = settingsObj.optDouble(KEY_PLAYBACK_SPEED, defaults.playbackSpeed.toDouble()).toFloat(),
                    eqEnabled = settingsObj.optBoolean(KEY_EQ_ENABLED, defaults.eqEnabled),
                    eqPreset = settingsObj.optString(KEY_EQ_PRESET, defaults.eqPreset).ifBlank { defaults.eqPreset },
                    eqBandsDb = parsedBands,
                    autoplayEnabled = settingsObj.optBoolean(KEY_AUTOPLAY_ENABLED, defaults.autoplayEnabled),
                    shuffleEnabled = settingsObj.optBoolean(KEY_SHUFFLE_ENABLED, defaults.shuffleEnabled),
                    repeatMode = settingsObj.optInt(KEY_REPEAT_MODE, defaults.repeatMode),

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
                    downloadIncludeJioSaavn = settingsObj.optBoolean(
                        KEY_DOWNLOAD_INCLUDE_JIOSAAVN,
                        defaults.downloadIncludeJioSaavn,
                    ),
                    downloadStorageLocation = parseEnum(
                        settingsObj.optString(KEY_DOWNLOAD_STORAGE_LOCATION),
                        if (settingsObj.optBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads)) {
                            DownloadStorageLocation.EXTERNAL
                        } else {
                            defaults.downloadStorageLocation
                        },
                    ),
                    wifiOnlyDownloads = settingsObj.optBoolean(KEY_WIFI_ONLY_DOWNLOADS, defaults.wifiOnlyDownloads),
                    exportDownloads = settingsObj.optBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads),
                    autoDownloadLiked = settingsObj.optBoolean(KEY_AUTO_DOWNLOAD_LIKED, defaults.autoDownloadLiked),
                    downloadVideosIfAvailable = settingsObj.optBoolean(
                        KEY_DOWNLOAD_VIDEOS_IF_AVAILABLE,
                        defaults.downloadVideosIfAvailable,
                    ),
                    videoDownloadQuality = parseEnum(
                        settingsObj.optString(KEY_VIDEO_DOWNLOAD_QUALITY),
                        defaults.videoDownloadQuality,
                    ),

                    hideStatusBarInPlayer = settingsObj.optBoolean(
                        KEY_HIDE_STATUS_BAR_IN_PLAYER,
                        defaults.hideStatusBarInPlayer,
                    ),
                    hideSongStatus = settingsObj.optBoolean(KEY_HIDE_SONG_STATUS, defaults.hideSongStatus),
                    stopMusicOnClose = settingsObj.optBoolean(KEY_STOP_MUSIC_ON_CLOSE, defaults.stopMusicOnClose),
                    showNerdStats = settingsObj.optBoolean(KEY_SHOW_NERD_STATS, defaults.showNerdStats),
                    jioSaavnEnabled = settingsObj.optBoolean(KEY_JIOSAAVN_ENABLED, defaults.jioSaavnEnabled),
                    personalOnlyMode = settingsObj.optBoolean(KEY_PERSONAL_ONLY_MODE, defaults.personalOnlyMode),
                    localMusicEnabled = settingsObj.optBoolean(KEY_LOCAL_MUSIC_ENABLED, defaults.localMusicEnabled),
                    excludeNonMusicAudio = settingsObj.optBoolean(
                        KEY_EXCLUDE_NON_MUSIC_AUDIO,
                        defaults.excludeNonMusicAudio,
                    ),
                    localMusicMinDurationSecs = settingsObj.optInt(
                        KEY_LOCAL_MUSIC_MIN_DURATION_SECS,
                        defaults.localMusicMinDurationSecs,
                    ),
                    localMusicFolders = parseStringSet(settingsObj.optJSONArray(KEY_LOCAL_MUSIC_FOLDERS)),
                    attributePersonalToYtm = settingsObj.optBoolean(
                        KEY_ATTRIBUTE_PERSONAL_TO_YTM,
                        defaults.attributePersonalToYtm,
                    ),
                    suppressVideoAudioQualityWarning = settingsObj.optBoolean(
                        KEY_SUPPRESS_VIDEO_AUDIO_WARNING,
                        defaults.suppressVideoAudioQualityWarning,
                    ),
                    videoQuality = parseEnum(
                        settingsObj.optString(KEY_VIDEO_QUALITY),
                        defaults.videoQuality,
                    ),
                    discordRpcEnabled = settingsObj.optBoolean(
                        KEY_DISCORD_RPC_ENABLED,
                        defaults.discordRpcEnabled,
                    ),
                    discordAppId = DiscordApplication.DEFAULT_APP_ID,
                    discordActivityType = parseEnum(
                        settingsObj.optString(KEY_DISCORD_ACTIVITY_TYPE),
                        defaults.discordActivityType,
                    ),
                    discordShowTimestamps = settingsObj.optBoolean(
                        KEY_DISCORD_SHOW_TIMESTAMPS,
                        defaults.discordShowTimestamps,
                    ),
                    discordShowArtwork = settingsObj.optBoolean(
                        KEY_DISCORD_SHOW_ARTWORK,
                        defaults.discordShowArtwork,
                    ),
                    discordShowButtons = settingsObj.optBoolean(
                        KEY_DISCORD_SHOW_BUTTONS,
                        defaults.discordShowButtons,
                    ),
                    connectEnabled = settingsObj.optBoolean(KEY_CONNECT_ENABLED, defaults.connectEnabled),
                    connectDeviceName = settingsObj.optString(
                        KEY_CONNECT_DEVICE_NAME,
                        defaults.connectDeviceName,
                    ),
                    lastFmEnabled = settingsObj.optBoolean(KEY_LASTFM_ENABLED, defaults.lastFmEnabled),
                    listenBrainzEnabled = settingsObj.optBoolean(
                        KEY_LISTENBRAINZ_ENABLED,
                        defaults.listenBrainzEnabled,
                    ),
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
                publishStateFlowsLocked(restored)
                stateLoaded = true
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
                appLanguage = parseEnum(prefs.getString(KEY_APP_LANGUAGE, null), defaults.appLanguage),
                designTheme = parseEnum(prefs.getString(KEY_DESIGN_THEME, null), defaults.designTheme),
                themeMode = parseThemeMode(prefs.getString(KEY_THEME_MODE, null), defaults.themeMode),
                liquidGlassEnabled = prefs.getBoolean(KEY_LIQUID_GLASS_ENABLED, defaults.liquidGlassEnabled),
                glassPerformance = prefs.getBoolean(KEY_GLASS_PERFORMANCE, defaults.glassPerformance),
                reduceDynamicBlur = prefs.getBoolean(KEY_REDUCE_DYNAMIC_BLUR, defaults.reduceDynamicBlur),
                reduceAnimation = prefs.getBoolean(KEY_REDUCE_ANIMATION, defaults.reduceAnimation),
                animatedArtwork = prefs.getBoolean(KEY_ANIMATED_ARTWORK, defaults.animatedArtwork),
                animatedArtworkCellular = prefs.getBoolean(
                    KEY_ANIMATED_ARTWORK_CELLULAR,
                    defaults.animatedArtworkCellular,
                ),
                artworkTintedPages = prefs.getBoolean(KEY_ARTWORK_TINTED_PAGES, defaults.artworkTintedPages),
                fullScreenCoverArt = prefs.getBoolean(KEY_FULL_SCREEN_COVER_ART, defaults.fullScreenCoverArt),
                fullScreenDetailCover = prefs.getBoolean(
                    KEY_FULL_SCREEN_DETAIL_COVER,
                    defaults.fullScreenDetailCover,
                ),
                showTotalDuration = prefs.getBoolean(KEY_SHOW_TOTAL_DURATION, defaults.showTotalDuration),
                hideLyricsPreview = prefs.getBoolean(KEY_HIDE_LYRICS_PREVIEW, defaults.hideLyricsPreview),
                playerBackdropMode = parseEnum(
                    prefs.getString(KEY_PLAYER_BACKDROP_MODE, null),
                    defaults.playerBackdropMode,
                ),
                fpsLimit = prefs.getInt(KEY_FPS_LIMIT, defaults.fpsLimit),

                hideRecents = prefs.getBoolean(KEY_HIDE_RECENTS, defaults.hideRecents),
                hideTopPicks = prefs.getBoolean(KEY_HIDE_TOP_PICKS, defaults.hideTopPicks),
                hideHomeMoods = prefs.getBoolean(KEY_HIDE_HOME_MOODS, defaults.hideHomeMoods),
                hideHomeVideos = prefs.getBoolean(KEY_HIDE_HOME_VIDEOS, defaults.hideHomeVideos),
                squareVideoThumbnails = prefs.getBoolean(
                    KEY_SQUARE_VIDEO_THUMBNAILS,
                    defaults.squareVideoThumbnails,
                ),

                audioQuality = parseEnum(prefs.getString(KEY_AUDIO_QUALITY, null), defaults.audioQuality),
                preferHighQuality = prefs.getBoolean(KEY_PREFER_HIGH_QUALITY, defaults.preferHighQuality),
                gaplessEnabled = prefs.getBoolean(KEY_GAPLESS_ENABLED, defaults.gaplessEnabled),
                crossfadeEnabled = prefs.getBoolean(KEY_CROSSFADE_ENABLED, defaults.crossfadeEnabled),
                crossfadeSeconds = prefs.getInt(KEY_CROSSFADE_SECONDS, defaults.crossfadeSeconds),
                loudnessNormEnabled = prefs.getBoolean(KEY_LOUDNESS_NORM_ENABLED, defaults.loudnessNormEnabled),
                skipSilenceEnabled = prefs.getBoolean(KEY_SKIP_SILENCE_ENABLED, defaults.skipSilenceEnabled),
                sleepTimerFadeOut = prefs.getBoolean(KEY_SLEEP_TIMER_FADE_OUT, defaults.sleepTimerFadeOut),
                playbackSpeed = prefs.getFloat(KEY_PLAYBACK_SPEED, defaults.playbackSpeed),
                eqEnabled = prefs.getBoolean(KEY_EQ_ENABLED, defaults.eqEnabled),
                eqPreset = prefs.getString(KEY_EQ_PRESET, defaults.eqPreset)
                    ?.takeIf { it.isNotBlank() }
                    ?: defaults.eqPreset,
                eqBandsDb = parsedBands,
                autoplayEnabled = prefs.getBoolean(KEY_AUTOPLAY_ENABLED, defaults.autoplayEnabled),
                shuffleEnabled = prefs.getBoolean(KEY_SHUFFLE_ENABLED, defaults.shuffleEnabled),
                repeatMode = prefs.getInt(KEY_REPEAT_MODE, defaults.repeatMode),

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
                downloadIncludeJioSaavn = prefs.getBoolean(
                    KEY_DOWNLOAD_INCLUDE_JIOSAAVN,
                    defaults.downloadIncludeJioSaavn,
                ),
                downloadStorageLocation = parseEnum(
                    prefs.getString(KEY_DOWNLOAD_STORAGE_LOCATION, null),
                    if (prefs.getBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads)) {
                        DownloadStorageLocation.EXTERNAL
                    } else {
                        defaults.downloadStorageLocation
                    },
                ),
                wifiOnlyDownloads = prefs.getBoolean(KEY_WIFI_ONLY_DOWNLOADS, defaults.wifiOnlyDownloads),
                exportDownloads = prefs.getBoolean(KEY_EXPORT_DOWNLOADS, defaults.exportDownloads),
                autoDownloadLiked = prefs.getBoolean(KEY_AUTO_DOWNLOAD_LIKED, defaults.autoDownloadLiked),
                downloadVideosIfAvailable = prefs.getBoolean(
                    KEY_DOWNLOAD_VIDEOS_IF_AVAILABLE,
                    defaults.downloadVideosIfAvailable,
                ),
                videoDownloadQuality = parseEnum(
                    prefs.getString(KEY_VIDEO_DOWNLOAD_QUALITY, null),
                    defaults.videoDownloadQuality,
                ),

                hideStatusBarInPlayer = prefs.getBoolean(
                    KEY_HIDE_STATUS_BAR_IN_PLAYER,
                    defaults.hideStatusBarInPlayer,
                ),
                hideSongStatus = prefs.getBoolean(KEY_HIDE_SONG_STATUS, defaults.hideSongStatus),
                stopMusicOnClose = prefs.getBoolean(KEY_STOP_MUSIC_ON_CLOSE, defaults.stopMusicOnClose),
                showNerdStats = prefs.getBoolean(KEY_SHOW_NERD_STATS, defaults.showNerdStats),
                jioSaavnEnabled = prefs.getBoolean(KEY_JIOSAAVN_ENABLED, defaults.jioSaavnEnabled),
                personalOnlyMode = prefs.getBoolean(KEY_PERSONAL_ONLY_MODE, defaults.personalOnlyMode),
                localMusicEnabled = prefs.getBoolean(KEY_LOCAL_MUSIC_ENABLED, defaults.localMusicEnabled),
                excludeNonMusicAudio = prefs.getBoolean(
                    KEY_EXCLUDE_NON_MUSIC_AUDIO,
                    defaults.excludeNonMusicAudio,
                ),
                localMusicMinDurationSecs = prefs.getInt(
                    KEY_LOCAL_MUSIC_MIN_DURATION_SECS,
                    defaults.localMusicMinDurationSecs,
                ),
                localMusicFolders = parseStringSet(prefs.getString(KEY_LOCAL_MUSIC_FOLDERS, null)),
                attributePersonalToYtm = prefs.getBoolean(
                    KEY_ATTRIBUTE_PERSONAL_TO_YTM,
                    defaults.attributePersonalToYtm,
                ),
                suppressVideoAudioQualityWarning = prefs.getBoolean(
                    KEY_SUPPRESS_VIDEO_AUDIO_WARNING,
                    defaults.suppressVideoAudioQualityWarning,
                ),
                videoQuality = parseEnum(prefs.getString(KEY_VIDEO_QUALITY, null), defaults.videoQuality),
                discordRpcEnabled = prefs.getBoolean(KEY_DISCORD_RPC_ENABLED, defaults.discordRpcEnabled),
                discordAppId = DiscordApplication.DEFAULT_APP_ID,
                discordActivityType = parseEnum(
                    prefs.getString(KEY_DISCORD_ACTIVITY_TYPE, null),
                    defaults.discordActivityType,
                ),
                discordShowTimestamps = prefs.getBoolean(
                    KEY_DISCORD_SHOW_TIMESTAMPS,
                    defaults.discordShowTimestamps,
                ),
                discordShowArtwork = prefs.getBoolean(KEY_DISCORD_SHOW_ARTWORK, defaults.discordShowArtwork),
                discordShowButtons = prefs.getBoolean(KEY_DISCORD_SHOW_BUTTONS, defaults.discordShowButtons),
                connectEnabled = prefs.getBoolean(KEY_CONNECT_ENABLED, defaults.connectEnabled),
                connectDeviceName = prefs.getString(KEY_CONNECT_DEVICE_NAME, null)
                    ?: defaults.connectDeviceName,
                lastFmEnabled = prefs.getBoolean(KEY_LASTFM_ENABLED, defaults.lastFmEnabled),
                listenBrainzEnabled = prefs.getBoolean(KEY_LISTENBRAINZ_ENABLED, defaults.listenBrainzEnabled),
            ),
        )
    }

    private fun writeState(
        state: SonataSettingsState,
        trackOffsetsOverride: Map<String, Long>? = null,
    ) {
        val editor = prefs.edit()
            .putString(KEY_APP_LANGUAGE, state.appLanguage.name)
            .putString(KEY_DESIGN_THEME, state.designTheme.name)
            .putString(KEY_THEME_MODE, state.themeMode.name)
            .putBoolean(KEY_LIQUID_GLASS_ENABLED, state.liquidGlassEnabled)
            .putBoolean(KEY_GLASS_PERFORMANCE, state.glassPerformance)
            .putBoolean(KEY_REDUCE_DYNAMIC_BLUR, state.reduceDynamicBlur)
            .putBoolean(KEY_REDUCE_ANIMATION, state.reduceAnimation)
            .putBoolean(KEY_ANIMATED_ARTWORK, state.animatedArtwork)
            .putBoolean(KEY_ANIMATED_ARTWORK_CELLULAR, state.animatedArtworkCellular)
            .putBoolean(KEY_ARTWORK_TINTED_PAGES, state.artworkTintedPages)
            .putBoolean(KEY_FULL_SCREEN_COVER_ART, state.fullScreenCoverArt)
            .putBoolean(KEY_FULL_SCREEN_DETAIL_COVER, state.fullScreenDetailCover)
            .putBoolean(KEY_SHOW_TOTAL_DURATION, state.showTotalDuration)
            .putBoolean(KEY_HIDE_LYRICS_PREVIEW, state.hideLyricsPreview)
            .putString(KEY_PLAYER_BACKDROP_MODE, state.playerBackdropMode.name)
            .putInt(KEY_FPS_LIMIT, state.fpsLimit)
            .putBoolean(KEY_HIDE_RECENTS, state.hideRecents)
            .putBoolean(KEY_HIDE_TOP_PICKS, state.hideTopPicks)
            .putBoolean(KEY_HIDE_HOME_MOODS, state.hideHomeMoods)
            .putBoolean(KEY_HIDE_HOME_VIDEOS, state.hideHomeVideos)
            .putBoolean(KEY_SQUARE_VIDEO_THUMBNAILS, state.squareVideoThumbnails)
            .putString(KEY_AUDIO_QUALITY, state.audioQuality.name)
            .putBoolean(KEY_PREFER_HIGH_QUALITY, state.preferHighQuality)
            .putBoolean(KEY_GAPLESS_ENABLED, state.gaplessEnabled)
            .putBoolean(KEY_CROSSFADE_ENABLED, state.crossfadeEnabled)
            .putInt(KEY_CROSSFADE_SECONDS, state.crossfadeSeconds)
            .putBoolean(KEY_LOUDNESS_NORM_ENABLED, state.loudnessNormEnabled)
            .putBoolean(KEY_SKIP_SILENCE_ENABLED, state.skipSilenceEnabled)
            .putBoolean(KEY_SLEEP_TIMER_FADE_OUT, state.sleepTimerFadeOut)
            .putFloat(KEY_PLAYBACK_SPEED, state.playbackSpeed)
            .putBoolean(KEY_EQ_ENABLED, state.eqEnabled)
            .putString(KEY_EQ_PRESET, state.eqPreset)
            .putString(KEY_EQ_BANDS_DB, encodeBands(state.eqBandsDb))
            .putBoolean(KEY_AUTOPLAY_ENABLED, state.autoplayEnabled)
            .putBoolean(KEY_SHUFFLE_ENABLED, state.shuffleEnabled)
            .putInt(KEY_REPEAT_MODE, state.repeatMode)
            .putBoolean(KEY_LYRICS_BLUR_ENABLED, state.lyricsBlurEnabled)
            .putBoolean(KEY_LYRICS_TRANSLATION_ENABLED, state.lyricsTranslationEnabled)
            .putString(KEY_LYRICS_TRANSLATION_LANG, state.lyricsTranslationLang)
            .putBoolean(KEY_LYRICS_ROMANIZATION_ENABLED, state.lyricsRomanizationEnabled)
            .putLong(KEY_LYRICS_OFFSET_MS, state.lyricsOffsetMs)
            .putBoolean(KEY_PRIORITIZE_WORD_SYNC, state.prioritizeWordSync)
            .putString(KEY_DOWNLOAD_QUALITY, state.downloadQuality.name)
            .putBoolean(KEY_DOWNLOAD_INCLUDE_JIOSAAVN, state.downloadIncludeJioSaavn)
            .putString(KEY_DOWNLOAD_STORAGE_LOCATION, state.downloadStorageLocation.name)
            .putBoolean(KEY_WIFI_ONLY_DOWNLOADS, state.wifiOnlyDownloads)
            .putBoolean(KEY_EXPORT_DOWNLOADS, state.exportDownloads)
            .putBoolean(KEY_AUTO_DOWNLOAD_LIKED, state.autoDownloadLiked)
            .putBoolean(KEY_DOWNLOAD_VIDEOS_IF_AVAILABLE, state.downloadVideosIfAvailable)
            .putString(KEY_VIDEO_DOWNLOAD_QUALITY, state.videoDownloadQuality.name)
            .putBoolean(KEY_HIDE_STATUS_BAR_IN_PLAYER, state.hideStatusBarInPlayer)
            .putBoolean(KEY_HIDE_SONG_STATUS, state.hideSongStatus)
            .putBoolean(KEY_STOP_MUSIC_ON_CLOSE, state.stopMusicOnClose)
            .putBoolean(KEY_SHOW_NERD_STATS, state.showNerdStats)
            .putBoolean(KEY_JIOSAAVN_ENABLED, state.jioSaavnEnabled)
            .putBoolean(KEY_PERSONAL_ONLY_MODE, state.personalOnlyMode)
            .putBoolean(KEY_LOCAL_MUSIC_ENABLED, state.localMusicEnabled)
            .putBoolean(KEY_EXCLUDE_NON_MUSIC_AUDIO, state.excludeNonMusicAudio)
            .putInt(KEY_LOCAL_MUSIC_MIN_DURATION_SECS, state.localMusicMinDurationSecs)
            .putString(KEY_LOCAL_MUSIC_FOLDERS, encodeStringSet(state.localMusicFolders))
            .putBoolean(KEY_ATTRIBUTE_PERSONAL_TO_YTM, state.attributePersonalToYtm)
            .putBoolean(KEY_SUPPRESS_VIDEO_AUDIO_WARNING, state.suppressVideoAudioQualityWarning)
            .putString(KEY_VIDEO_QUALITY, state.videoQuality.name)
            .putBoolean(KEY_DISCORD_RPC_ENABLED, state.discordRpcEnabled)
            .putString(KEY_DISCORD_APP_ID, DiscordApplication.DEFAULT_APP_ID)
            .putString(KEY_DISCORD_ACTIVITY_TYPE, state.discordActivityType.name)
            .putBoolean(KEY_DISCORD_SHOW_TIMESTAMPS, state.discordShowTimestamps)
            .putBoolean(KEY_DISCORD_SHOW_ARTWORK, state.discordShowArtwork)
            .putBoolean(KEY_DISCORD_SHOW_BUTTONS, state.discordShowButtons)
            .putBoolean(KEY_CONNECT_ENABLED, state.connectEnabled)
            .putString(KEY_CONNECT_DEVICE_NAME, state.connectDeviceName)
            .putBoolean(KEY_LASTFM_ENABLED, state.lastFmEnabled)
            .putBoolean(KEY_LISTENBRAINZ_ENABLED, state.listenBrainzEnabled)

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

    private inline fun <reified T : Enum<T>> parseEnum(raw: String?, fallback: T): T {
        if (raw.isNullOrBlank()) return fallback
        return runCatching { enumValueOf<T>(raw.trim().uppercase()) }.getOrDefault(fallback)
    }

    companion object {
        private const val PREFS_NAME = "sonata_preferences"
        private const val DISCORD_AUTH_PREFS_NAME = "sonata_discord_auth"
        private const val KEY_DISCORD_AUTH_SECRET = "discord_session_credential"

        /**
         * Returns `true` when online streaming sources (YouTube Music, JioSaavn) are active.
         */
        fun isStreamingEnabled(personalOnly: Boolean): Boolean = !personalOnly

        /**
         * Returns the effective JioSaavn state without mutating the underlying persisted flag.
         */
        fun effectiveJioSaavnEnabled(
            personalOnly: Boolean,
            jioSaavnEnabled: Boolean,
        ): Boolean = !personalOnly && jioSaavnEnabled

        internal fun encodeTrackOffsets(offsets: Map<String, Long>): String {
            val obj = JSONObject()
            offsets.forEach { (id, offset) ->
                if (id.isNotBlank() && offset != 0L) {
                    obj.put(id, offset.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS))
                }
            }
            return obj.toString()
        }

        internal fun sanitize(state: SonataSettingsState): SonataSettingsState {
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
                designTheme = DesignTheme.BASIC,
                fpsLimit = normalizedFps,
                crossfadeSeconds = state.crossfadeSeconds.coerceIn(MIN_CROSSFADE_SEC, MAX_CROSSFADE_SEC),
                playbackSpeed = normalizedSpeed,
                eqPreset = state.eqPreset.trim().ifEmpty { "Flat" },
                eqBandsDb = normalizedBands,
                repeatMode = state.repeatMode.coerceIn(0, 2),
                lyricsTranslationLang = state.lyricsTranslationLang.trim().lowercase().ifEmpty { "en" },
                lyricsOffsetMs = state.lyricsOffsetMs.coerceIn(MIN_LYRIC_OFFSET_MS, MAX_LYRIC_OFFSET_MS),
                localMusicMinDurationSecs = state.localMusicMinDurationSecs.coerceIn(0, MAX_LOCAL_MIN_DURATION_SEC),
                localMusicFolders = normalizeLocalMusicFolders(state.localMusicFolders),
                discordAppId = DiscordApplication.DEFAULT_APP_ID,
                connectDeviceName = normalizeConnectDeviceName(state.connectDeviceName),
            )
        }

        internal fun normalizeLocalMusicFolders(folders: Set<String>): Set<String> {
            val seen = LinkedHashMap<String, String>()
            folders.forEach { raw ->
                val clean = raw.trim().replace('\\', '/').replace(Regex("/+"), "/").trim('/')
                if (clean.isNotEmpty()) {
                    seen.putIfAbsent(clean.lowercase(), clean)
                }
            }
            return seen.values.sortedBy { it.lowercase() }.toSet()
        }

        internal fun encodeStringSet(values: Set<String>): String =
            JSONArray().apply { normalizeLocalMusicFolders(values).forEach { put(it) } }.toString()

        internal fun parseStringSet(array: JSONArray?): Set<String> {
            if (array == null) return emptySet()
            val out = LinkedHashSet<String>()
            for (i in 0 until array.length()) {
                val item = array.optString(i, "")
                if (item.isNotBlank()) out.add(item)
            }
            return normalizeLocalMusicFolders(out)
        }

        internal fun parseStringSet(raw: String?): Set<String> {
            if (raw.isNullOrBlank()) return emptySet()
            return runCatching { parseStringSet(JSONArray(raw)) }.getOrDefault(emptySet())
        }

        internal fun normalizeConnectDeviceName(raw: String?): String =
            raw?.trim()?.takeIf { it.isNotEmpty() }?.take(64) ?: DEFAULT_CONNECT_DEVICE_NAME

        internal fun encodeBands(bands: List<Float>): String =
            JSONArray().apply { bands.forEach { put(it.toDouble()) } }.toString()

        internal fun parseBands(raw: String?, fallback: List<Float>): List<Float> {
            if (raw.isNullOrBlank()) return fallback
            return runCatching {
                val array = JSONArray(raw)
                List(EQ_BAND_COUNT) { index ->
                    if (index < array.length()) array.optDouble(index, 0.0).toFloat() else 0f
                }
            }.getOrDefault(fallback)
        }

        internal fun parseThemeMode(raw: String?, fallback: ThemeMode): ThemeMode {
            val normalized = raw?.trim()?.uppercase() ?: return fallback
            return when (normalized) {
                "LIGHT", "BASIC_LIGHT", "VIOLET_LIGHT", "MOONFLOWER_LIGHT", "IMPERIAL_VIOLET" -> ThemeMode.LIGHT
                "DARK", "BASIC_DARK", "VIOLET_DARK", "MOONFLOWER_DARK", "ELECTRIC_LAVENDER", "AMOLED" -> ThemeMode.DARK
                "SYSTEM" -> ThemeMode.SYSTEM
                else -> runCatching { ThemeMode.valueOf(normalized) }.getOrDefault(fallback)
            }
        }

        private const val EQ_BAND_COUNT = 5
        private const val MIN_EQ_DB = -12f
        private const val MAX_EQ_DB = 12f
        private const val MIN_SPEED = 0.5f
        private const val MAX_SPEED = 2.0f
        private const val MIN_CROSSFADE_SEC = 1
        private const val MAX_CROSSFADE_SEC = 12
        private const val MIN_LYRIC_OFFSET_MS = -30_000L
        private const val MAX_LYRIC_OFFSET_MS = 30_000L
        private const val MAX_LOCAL_MIN_DURATION_SEC = 600
        internal const val DEFAULT_CONNECT_DEVICE_NAME = "Sonata Android"

        private const val BACKUP_APP_ID = "sonata"
        private const val BACKUP_SCHEMA_VERSION = 1
        private const val BACKUP_KEY_APP = "app"
        private const val BACKUP_KEY_SCHEMA = "schemaVersion"
        private const val BACKUP_KEY_EXPORTED_AT_MS = "exportedAtMs"
        private const val BACKUP_KEY_SETTINGS = "settings"
        private const val BACKUP_KEY_TRACK_OFFSETS = "trackLyricOffsets"

        private const val KEY_APP_LANGUAGE = "app_language"
        private const val KEY_DESIGN_THEME = "design_theme"
        private const val KEY_THEME_MODE = "theme_mode"
        private const val KEY_LIQUID_GLASS_ENABLED = "liquid_glass_enabled"
        private const val KEY_GLASS_PERFORMANCE = "glass_performance"
        private const val KEY_REDUCE_DYNAMIC_BLUR = "reduce_dynamic_blur"
        private const val KEY_REDUCE_ANIMATION = "reduce_animation"
        private const val KEY_ANIMATED_ARTWORK = "animated_artwork"
        private const val KEY_ANIMATED_ARTWORK_CELLULAR = "animated_artwork_cellular"
        private const val KEY_ARTWORK_TINTED_PAGES = "artwork_tinted_pages"
        private const val KEY_FULL_SCREEN_COVER_ART = "full_screen_cover_art"
        private const val KEY_FULL_SCREEN_DETAIL_COVER = "full_screen_detail_cover"
        private const val KEY_SHOW_TOTAL_DURATION = "show_total_duration"
        private const val KEY_HIDE_LYRICS_PREVIEW = "hide_lyrics_preview"
        private const val KEY_PLAYER_BACKDROP_MODE = "player_backdrop_mode"
        private const val KEY_FPS_LIMIT = "fps_limit"

        private const val KEY_HIDE_RECENTS = "hide_recents"
        private const val KEY_HIDE_TOP_PICKS = "hide_top_picks"
        private const val KEY_HIDE_HOME_MOODS = "hide_home_moods"
        private const val KEY_HIDE_HOME_VIDEOS = "hide_home_videos"
        private const val KEY_SQUARE_VIDEO_THUMBNAILS = "square_video_thumbnails"

        private const val KEY_AUDIO_QUALITY = "audio_quality"
        private const val KEY_PREFER_HIGH_QUALITY = "prefer_high_quality"
        private const val KEY_GAPLESS_ENABLED = "gapless_enabled"
        private const val KEY_CROSSFADE_ENABLED = "crossfade_enabled"
        private const val KEY_CROSSFADE_SECONDS = "crossfade_seconds"
        private const val KEY_LOUDNESS_NORM_ENABLED = "loudness_norm_enabled"
        private const val KEY_SKIP_SILENCE_ENABLED = "skip_silence_enabled"
        private const val KEY_SLEEP_TIMER_FADE_OUT = "sleep_timer_fade_out"
        private const val KEY_PLAYBACK_SPEED = "playback_speed"
        private const val KEY_EQ_ENABLED = "eq_enabled"
        private const val KEY_EQ_PRESET = "eq_preset"
        private const val KEY_EQ_BANDS_DB = "eq_bands_db"
        private const val KEY_AUTOPLAY_ENABLED = "autoplay_enabled"
        private const val KEY_SHUFFLE_ENABLED = "shuffle_enabled"
        private const val KEY_REPEAT_MODE = "repeat_mode"

        private const val KEY_LYRICS_BLUR_ENABLED = "lyrics_blur_enabled"
        private const val KEY_LYRICS_TRANSLATION_ENABLED = "lyrics_translation_enabled"
        private const val KEY_LYRICS_TRANSLATION_LANG = "lyrics_translation_lang"
        private const val KEY_LYRICS_ROMANIZATION_ENABLED = "lyrics_romanization_enabled"
        private const val KEY_LYRICS_OFFSET_MS = "lyrics_offset_ms"
        private const val KEY_PRIORITIZE_WORD_SYNC = "prioritize_word_sync"

        private const val KEY_DOWNLOAD_QUALITY = "download_quality"
        private const val KEY_DOWNLOAD_INCLUDE_JIOSAAVN = "download_include_jiosaavn"
        private const val KEY_DOWNLOAD_STORAGE_LOCATION = "download_storage_location"
        private const val KEY_WIFI_ONLY_DOWNLOADS = "wifi_only_downloads"
        private const val KEY_EXPORT_DOWNLOADS = "export_downloads"
        private const val KEY_AUTO_DOWNLOAD_LIKED = "auto_download_liked"
        private const val KEY_DOWNLOAD_VIDEOS_IF_AVAILABLE = "download_videos_if_available"
        private const val KEY_VIDEO_DOWNLOAD_QUALITY = "video_download_quality"

        private const val KEY_HIDE_STATUS_BAR_IN_PLAYER = "hide_status_bar_in_player"
        private const val KEY_HIDE_SONG_STATUS = "hide_song_status"
        private const val KEY_STOP_MUSIC_ON_CLOSE = "stop_music_on_close"
        private const val KEY_SHOW_NERD_STATS = "show_nerd_stats"
        private const val KEY_JIOSAAVN_ENABLED = "jiosaavn_enabled"
        private const val KEY_PERSONAL_ONLY_MODE = "personal_only_mode"
        private const val KEY_LOCAL_MUSIC_ENABLED = "local_music_enabled"
        private const val KEY_EXCLUDE_NON_MUSIC_AUDIO = "exclude_non_music_audio"
        private const val KEY_LOCAL_MUSIC_MIN_DURATION_SECS = "local_music_min_duration_secs"
        private const val KEY_LOCAL_MUSIC_FOLDERS = "local_music_folders"
        private const val KEY_ATTRIBUTE_PERSONAL_TO_YTM = "attribute_personal_to_ytm"
        private const val KEY_SUPPRESS_VIDEO_AUDIO_WARNING = "suppress_video_audio_warning"
        private const val KEY_VIDEO_QUALITY = "video_quality"

        private const val KEY_DISCORD_RPC_ENABLED = "discord_rpc_enabled"
        private const val KEY_DISCORD_APP_ID = "discord_app_id"
        private const val KEY_DISCORD_ACTIVITY_TYPE = "discord_activity_type"
        private const val KEY_DISCORD_SHOW_TIMESTAMPS = "discord_show_timestamps"
        private const val KEY_DISCORD_SHOW_ARTWORK = "discord_show_artwork"
        private const val KEY_DISCORD_SHOW_BUTTONS = "discord_show_buttons"

        private const val KEY_CONNECT_ENABLED = "connect_enabled"
        private const val KEY_CONNECT_DEVICE_NAME = "connect_device_name"
        private const val KEY_LASTFM_ENABLED = "lastfm_enabled"
        private const val KEY_LISTENBRAINZ_ENABLED = "listenbrainz_enabled"

        private const val KEY_TRACK_LYRIC_OFFSETS = "track_lyric_offsets"

        internal fun formatBackupJson(
            current: SonataSettingsState,
            offsets: Map<String, Long>,
            exportedAtMs: Long = System.currentTimeMillis(),
        ): String {
            val settingsObj = JSONObject().apply {
                put(KEY_APP_LANGUAGE, current.appLanguage.name)
                put(KEY_DESIGN_THEME, current.designTheme.name)
                put(KEY_THEME_MODE, current.themeMode.name)
                put(KEY_LIQUID_GLASS_ENABLED, current.liquidGlassEnabled)
                put(KEY_GLASS_PERFORMANCE, current.glassPerformance)
                put(KEY_REDUCE_DYNAMIC_BLUR, current.reduceDynamicBlur)
                put(KEY_REDUCE_ANIMATION, current.reduceAnimation)
                put(KEY_ANIMATED_ARTWORK, current.animatedArtwork)
                put(KEY_ANIMATED_ARTWORK_CELLULAR, current.animatedArtworkCellular)
                put(KEY_ARTWORK_TINTED_PAGES, current.artworkTintedPages)
                put(KEY_FULL_SCREEN_COVER_ART, current.fullScreenCoverArt)
                put(KEY_FULL_SCREEN_DETAIL_COVER, current.fullScreenDetailCover)
                put(KEY_SHOW_TOTAL_DURATION, current.showTotalDuration)
                put(KEY_HIDE_LYRICS_PREVIEW, current.hideLyricsPreview)
                put(KEY_PLAYER_BACKDROP_MODE, current.playerBackdropMode.name)
                put(KEY_FPS_LIMIT, current.fpsLimit)

                put(KEY_HIDE_RECENTS, current.hideRecents)
                put(KEY_HIDE_TOP_PICKS, current.hideTopPicks)
                put(KEY_HIDE_HOME_MOODS, current.hideHomeMoods)
                put(KEY_HIDE_HOME_VIDEOS, current.hideHomeVideos)
                put(KEY_SQUARE_VIDEO_THUMBNAILS, current.squareVideoThumbnails)

                put(KEY_AUDIO_QUALITY, current.audioQuality.name)
                put(KEY_PREFER_HIGH_QUALITY, current.preferHighQuality)
                put(KEY_GAPLESS_ENABLED, current.gaplessEnabled)
                put(KEY_CROSSFADE_ENABLED, current.crossfadeEnabled)
                put(KEY_CROSSFADE_SECONDS, current.crossfadeSeconds)
                put(KEY_LOUDNESS_NORM_ENABLED, current.loudnessNormEnabled)
                put(KEY_SKIP_SILENCE_ENABLED, current.skipSilenceEnabled)
                put(KEY_SLEEP_TIMER_FADE_OUT, current.sleepTimerFadeOut)
                put(KEY_PLAYBACK_SPEED, current.playbackSpeed.toDouble())
                put(KEY_EQ_ENABLED, current.eqEnabled)
                put(KEY_EQ_PRESET, current.eqPreset)
                put(KEY_EQ_BANDS_DB, JSONArray().apply { current.eqBandsDb.forEach { put(it.toDouble()) } })
                put(KEY_AUTOPLAY_ENABLED, current.autoplayEnabled)
                put(KEY_SHUFFLE_ENABLED, current.shuffleEnabled)
                put(KEY_REPEAT_MODE, current.repeatMode)

                put(KEY_LYRICS_BLUR_ENABLED, current.lyricsBlurEnabled)
                put(KEY_LYRICS_TRANSLATION_ENABLED, current.lyricsTranslationEnabled)
                put(KEY_LYRICS_TRANSLATION_LANG, current.lyricsTranslationLang)
                put(KEY_LYRICS_ROMANIZATION_ENABLED, current.lyricsRomanizationEnabled)
                put(KEY_LYRICS_OFFSET_MS, current.lyricsOffsetMs)
                put(KEY_PRIORITIZE_WORD_SYNC, current.prioritizeWordSync)

                put(KEY_DOWNLOAD_QUALITY, current.downloadQuality.name)
                put(KEY_DOWNLOAD_INCLUDE_JIOSAAVN, current.downloadIncludeJioSaavn)
                put(KEY_DOWNLOAD_STORAGE_LOCATION, current.downloadStorageLocation.name)
                put(KEY_WIFI_ONLY_DOWNLOADS, current.wifiOnlyDownloads)
                put(KEY_EXPORT_DOWNLOADS, current.exportDownloads)
                put(KEY_AUTO_DOWNLOAD_LIKED, current.autoDownloadLiked)
                put(KEY_DOWNLOAD_VIDEOS_IF_AVAILABLE, current.downloadVideosIfAvailable)
                put(KEY_VIDEO_DOWNLOAD_QUALITY, current.videoDownloadQuality.name)

                put(KEY_HIDE_STATUS_BAR_IN_PLAYER, current.hideStatusBarInPlayer)
                put(KEY_HIDE_SONG_STATUS, current.hideSongStatus)
                put(KEY_STOP_MUSIC_ON_CLOSE, current.stopMusicOnClose)
                put(KEY_SHOW_NERD_STATS, current.showNerdStats)
                put(KEY_JIOSAAVN_ENABLED, current.jioSaavnEnabled)
                put(KEY_PERSONAL_ONLY_MODE, current.personalOnlyMode)
                put(KEY_LOCAL_MUSIC_ENABLED, current.localMusicEnabled)
                put(KEY_EXCLUDE_NON_MUSIC_AUDIO, current.excludeNonMusicAudio)
                put(KEY_LOCAL_MUSIC_MIN_DURATION_SECS, current.localMusicMinDurationSecs)
                put(KEY_LOCAL_MUSIC_FOLDERS, JSONArray().apply { current.localMusicFolders.forEach { put(it) } })
                put(KEY_ATTRIBUTE_PERSONAL_TO_YTM, current.attributePersonalToYtm)
                put(KEY_SUPPRESS_VIDEO_AUDIO_WARNING, current.suppressVideoAudioQualityWarning)
                put(KEY_VIDEO_QUALITY, current.videoQuality.name)
                put(KEY_DISCORD_RPC_ENABLED, current.discordRpcEnabled)
                put(KEY_DISCORD_APP_ID, DiscordApplication.DEFAULT_APP_ID)
                put(KEY_DISCORD_ACTIVITY_TYPE, current.discordActivityType.name)
                put(KEY_DISCORD_SHOW_TIMESTAMPS, current.discordShowTimestamps)
                put(KEY_DISCORD_SHOW_ARTWORK, current.discordShowArtwork)
                put(KEY_DISCORD_SHOW_BUTTONS, current.discordShowButtons)
                put(KEY_CONNECT_ENABLED, current.connectEnabled)
                put(KEY_CONNECT_DEVICE_NAME, current.connectDeviceName)
                put(KEY_LASTFM_ENABLED, current.lastFmEnabled)
                put(KEY_LISTENBRAINZ_ENABLED, current.listenBrainzEnabled)
            }

            val offsetsObj = JSONObject().apply {
                offsets.forEach { (id, offset) -> put(id, offset) }
            }

            val root = JSONObject().apply {
                put(BACKUP_KEY_APP, BACKUP_APP_ID)
                put(BACKUP_KEY_SCHEMA, BACKUP_SCHEMA_VERSION)
                put(BACKUP_KEY_EXPORTED_AT_MS, exportedAtMs)
                put(BACKUP_KEY_SETTINGS, settingsObj)
                put(BACKUP_KEY_TRACK_OFFSETS, offsetsObj)
            }
            return root.toString(2)
        }
    }
}
