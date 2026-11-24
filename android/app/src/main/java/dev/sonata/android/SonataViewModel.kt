package dev.sonata.android

import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.SharedPreferences
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Display
import android.webkit.WebView
import androidx.annotation.OptIn
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.Snapshot
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadedTrackInput
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.discord.DiscordPresenceConfig
import dev.sonata.android.discord.DiscordPresenceTrack
import dev.sonata.android.discord.DiscordRpcClient
import dev.sonata.android.discord.DiscordRpcStatus
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.CoroutineScope
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import dev.sonata.android.data.NativeSonataEngineGateway
import dev.sonata.android.data.SonataEngineGateway
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.ui.components.isEditableUserPlaylist
import dev.sonata.android.ui.components.normalizeCollectionId
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.i18n.localizeMoodTitle
import dev.sonata.android.ui.i18n.resolveSonataStrings
import dev.sonata.android.playback.PlaybackEligibility
import dev.sonata.android.playback.PlaybackGenerationCounter
import dev.sonata.android.playback.RateLimitPolicy
import dev.sonata.android.playback.RecoveryBudget
import dev.sonata.android.playback.StreamSwapPlayer
import dev.sonata.android.playback.mapMusicToVideoMs
import dev.sonata.android.playback.mapVideoToMusicMs
import dev.sonata.android.playback.performAtomicStreamSwap
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidExploreFeed
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidJsCipherSolver
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidMoodCategory
import uniffi.sonata_android.AndroidMoodChip
import uniffi.sonata_android.AndroidMusicVideoCandidate
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidResolvedMusicVideo
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerLibrary
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSearchResult
import uniffi.sonata_android.AndroidVideoQualityStream
import uniffi.sonata_android.AndroidSolvedCipher
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import uniffi.sonata_android.SonataAndroidEngine
import java.io.File
import java.util.Locale
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

private class WebViewJsCipherSolver(context: Context) : AndroidJsCipherSolver {
    private val appContext = context.applicationContext
    private val mainHandler = Handler(Looper.getMainLooper())
    private val loadLock = Any()
    @Volatile private var webView: WebView? = null
    @Volatile private var loadedHash: String? = null

    private fun getOrCreateWebViewOnMain(): WebView {
        var wv = webView
        if (wv == null) {
            wv = WebView(appContext).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = false
                settings.blockNetworkImage = true
                settings.blockNetworkLoads = true
            }
            webView = wv
        }
        return wv
    }

    override fun ensureLoaded(playerHash: String, hookedScript: String): Boolean {
        if (loadedHash == playerHash) return true
        if (Looper.myLooper() == Looper.getMainLooper()) return false
        synchronized(loadLock) {
            if (loadedHash == playerHash) return true
            val future = CompletableFuture<Boolean>()
            mainHandler.post {
                try {
                    if (loadedHash == playerHash) {
                        future.complete(true)
                        return@post
                    }
                    val wv = getOrCreateWebViewOnMain()
                    wv.evaluateJavascript("$hookedScript;typeof globalThis._yt_solve === 'function'") { result ->
                        val ok = result?.trim()?.removeSurrounding("\"") == "true"
                        if (ok) {
                            loadedHash = playerHash
                        }
                        Log.i("SonataStream", "cipher_webview loaded hash=$playerHash ok=$ok")
                        future.complete(ok)
                    }
                } catch (t: Throwable) {
                    Log.w("SonataStream", "cipher_webview load failed: ${t.message}")
                    future.complete(false)
                }
            }
            return runCatching { future.get(8, TimeUnit.SECONDS) }.getOrDefault(false)
        }
    }

    override fun solve(playerHash: String, sig: String, n: String): AndroidSolvedCipher? {
        if (Looper.myLooper() == Looper.getMainLooper()) return null
        val future = CompletableFuture<AndroidSolvedCipher?>()
        mainHandler.post {
            try {
                val wv = webView
                if (wv == null || loadedHash != playerHash) {
                    future.complete(null)
                    return@post
                }
                val js = "globalThis._yt_solve(${JSONObject.quote(sig)}, ${JSONObject.quote(n)})"
                wv.evaluateJavascript(js) { rawResult ->
                    try {
                        val jsonStr = if (rawResult != null && rawResult.startsWith("\"")) {
                            JSONTokener(rawResult).nextValue() as? String
                        } else {
                            rawResult
                        }
                        if (jsonStr.isNullOrBlank() || jsonStr == "null") {
                            future.complete(null)
                        } else {
                            val obj = JSONObject(jsonStr)
                            val solvedSig = obj.optString("sig", "").takeIf { it.isNotEmpty() && !obj.isNull("sig") }
                            val solvedN = obj.optString("n", "").takeIf { it.isNotEmpty() && !obj.isNull("n") }
                            future.complete(AndroidSolvedCipher(sig = solvedSig, n = solvedN))
                        }
                    } catch (t: Throwable) {
                        Log.w("SonataStream", "cipher_webview parse failed: ${t.message}")
                        future.complete(null)
                    }
                }
            } catch (t: Throwable) {
                future.complete(null)
            }
        }
        return runCatching { future.get(3, TimeUnit.SECONDS) }.getOrNull()
    }

    fun destroy() {
        mainHandler.post {
            runCatching { webView?.destroy() }
            webView = null
            loadedHash = null
        }
    }
}

enum class AndroidTab { Home, Explore, Radio, Library, Search }

@Stable
class PlaybackClockState {
    var positionMs: Long by mutableLongStateOf(0L)
        internal set
    var sampledAtNanos: Long by mutableLongStateOf(0L)
        internal set
    var durationMs: Long by mutableLongStateOf(0L)
        internal set
    var seekDiscontinuity: Long by mutableLongStateOf(0L)
        internal set

    internal fun sync(
        positionMs: Long,
        durationMs: Long,
        sampledAtNanos: Long = System.nanoTime(),
    ) {
        val safePos = positionMs.coerceAtLeast(0L)
        val safeDur = durationMs.coerceAtLeast(0L)
        if (kotlin.math.abs(safePos - this.positionMs) < 15L &&
            (safeDur <= 0L || this.durationMs == safeDur) &&
            sampledAtNanos - this.sampledAtNanos < 150_000_000L
        ) {
            return
        }
        Snapshot.withMutableSnapshot {
            this.positionMs = safePos
            this.sampledAtNanos = sampledAtNanos
            if (safeDur > 0L && this.durationMs != safeDur) {
                this.durationMs = safeDur
            }
        }
    }

    internal fun onSeek(
        positionMs: Long,
        sampledAtNanos: Long = System.nanoTime(),
    ) {
        val safePos = positionMs.coerceAtLeast(0L)
        Snapshot.withMutableSnapshot {
            this.positionMs = safePos
            this.sampledAtNanos = sampledAtNanos
            this.seekDiscontinuity += 1L
        }
    }

    internal fun resetForTrack(
        durationMs: Long,
        sampledAtNanos: Long = System.nanoTime(),
    ) {
        val safeDur = durationMs.coerceAtLeast(0L)
        Snapshot.withMutableSnapshot {
            this.positionMs = 0L
            this.sampledAtNanos = sampledAtNanos
            this.durationMs = safeDur
            this.seekDiscontinuity += 1L
        }
    }
}

@Immutable
data class PinnedCollectionEntry(
    val id: String,
    val title: String,
    val subtitle: String = "",
    val coverUrl: String? = null,
    val kindLabel: String = "Playlist",
    val isAlbum: Boolean = false,
    val pinnedAtMs: Long = 0L,
)

@Immutable
data class SavedExternalPlaylist(
    val id: String,
    val title: String,
    val subtitle: String = "",
    val coverUrl: String? = null,
    val kindLabel: String = "Playlist",
    val savedAtMs: Long = 0L,
)

internal fun isSelfHostedId(id: String?): Boolean {
    if (id.isNullOrBlank()) return false
    return id.startsWith("jellyfin:") ||
        id.startsWith("subsonic:") ||
        id.startsWith("webdav:")
}

internal fun selfHostedKindTag(id: String?): String? = when {
    id == null -> null
    id.startsWith("jellyfin:") -> "JFN"
    id.startsWith("subsonic:") -> "SUB"
    id.startsWith("webdav:") -> "DAV"
    else -> null
}

@Immutable
data class ServerBrowserState(
    val activeServer: AndroidServerProfile? = null,
    val libraries: List<AndroidServerLibrary> = emptyList(),
    val selectedLibraryId: String? = null,
    val playlists: List<AndroidPlaylist> = emptyList(),
    val albums: List<AndroidHomeCard> = emptyList(),
    val artists: List<AndroidHomeCard> = emptyList(),
    val selectedArtistName: String? = null,
    val artistAlbums: List<AndroidHomeCard> = emptyList(),
    val webdavPathStack: List<Pair<String?, String>> = emptyList(),
    val webdavFolders: List<AndroidServerLibrary> = emptyList(),
    val webdavTracks: List<AndroidTrack> = emptyList(),
    val serverSearchQuery: String = "",
    val serverSearchResult: AndroidServerSearchResult? = null,
    val loading: Boolean = false,
    val testingConnection: Boolean = false,
    val testResultMessage: String? = null,
    val testResultIsError: Boolean = false,
    val saving: Boolean = false,
    val error: String? = null,
)

@Immutable
data class SonataUiState(
    val activeTab: AndroidTab = AndroidTab.Home,
    val query: String = "",
    val loading: Boolean = true,
    val refreshing: Boolean = false,
    val status: String = "Loading YouTube Music...",
    val authInitialized: Boolean = false,
    val signedIn: Boolean = false,
    val accountProfile: AndroidAccountProfile? = null,
    val showLoginSheet: Boolean = false,
    val showSettingsSheet: Boolean = false,
    val showServersSheet: Boolean = false,
    val servers: List<AndroidServerProfile> = emptyList(),
    val serverSearchResult: AndroidServerSearchResult? = null,
    val serverSearchLoading: Boolean = false,
    val shelves: List<AndroidHomeShelf> = emptyList(),
    val exploreShelves: List<AndroidHomeShelf> = emptyList(),
    val moodCategories: List<AndroidMoodCategory> = emptyList(),
    val homeMoodChips: List<AndroidMoodChip> = emptyList(),
    val selectedHomeMoodParams: String? = null,
    val moodCategoryTitle: String? = null,
    val moodCategoryShelves: List<AndroidHomeShelf> = emptyList(),
    val moodCategoryLoading: Boolean = false,
    val radioMixShelves: List<AndroidHomeShelf> = emptyList(),
    val lastPlayedTracks: List<AndroidTrack> = emptyList(),
    val recentArtistCards: List<AndroidHomeCard> = emptyList(),
    val playlists: List<AndroidPlaylist> = emptyList(),
    val pinnedCollections: List<PinnedCollectionEntry> = emptyList(),
    val savedExternalPlaylistIds: Set<String> = emptySet(),
    val ownedPlaylistIds: Set<String> = emptySet(),
    val tracks: List<AndroidTrack> = emptyList(),
    val listTitle: String = "Quick Picks",
    val collectionBrowseId: String? = null,
    val collectionTitle: String? = null,
    val collectionCoverUrl: String? = null,
    val collectionSubtitle: String? = null,
    val collectionKindLabel: String? = null,
    val collectionYear: String? = null,
    val collectionTracks: List<AndroidTrack> = emptyList(),
    val artistPage: AndroidArtistPage? = null,
    val artistLoading: Boolean = false,
    val queue: List<AndroidTrack> = emptyList(),
    val queueIndex: Int = -1,
    val activeTrack: AndroidTrack? = null,
    val streamBadge: String = "",
    val isPlaying: Boolean = false,
    val isBuffering: Boolean = false,
    val isUpgradingQuality: Boolean = false,
    val repeatMode: Int = Player.REPEAT_MODE_OFF,
    val shuffleEnabled: Boolean = false,
    val lyrics: List<AndroidLyricLine> = emptyList(),
    val lyricsLoading: Boolean = false,
    val lyricsUnavailable: Boolean = false,
    val showLyrics: Boolean = false,
    val musicVideoCandidate: AndroidMusicVideoCandidate? = null,
    val videoModeEnabled: Boolean = false,
    val videoFullscreen: Boolean = false,
    val videoLoading: Boolean = false,
    val availableVideoQualities: List<AndroidVideoQualityStream> = emptyList(),
    val activeVideoQualityLabel: String = "",
    val activeVideoQualityHeight: Int = 0,
    val preferHighQuality: Boolean = true,
    val animatedArtwork: Boolean = true,
    val crossfadeEnabled: Boolean = false,
    val liquidGlassEnabled: Boolean = true,
    val fpsLimit: Int = 60,
    val supports120Hz: Boolean = false,
    val likedTrackIds: Set<String> = emptySet(),
    val addToPlaylistTargets: List<AndroidTrack> = emptyList(),
    val addToPlaylistStatus: String? = null,
    val recentSearches: List<String> = emptyList(),
    val updateInfo: AndroidUpdateInfo? = null,
    val checkingUpdates: Boolean = false,
    val installedVersionName: String = "1.0.0",
    val showUpdateDialog: Boolean = false,
    val updateDownloading: Boolean = false,
    val updateDownloadProgress: Float? = null,
    val updateDownloadDetail: String = "",
    val updateError: String? = null,
    val downloadedUpdateApkPath: String? = null,
) {
    val hasNext: Boolean
        get() = queue.isNotEmpty() && (
            repeatMode == Player.REPEAT_MODE_ALL ||
                (shuffleEnabled && queue.size > 1) ||
                (queueIndex in 0 until queue.lastIndex)
            )

    val hasPreviousInQueue: Boolean
        get() = activeTrack != null && (
            repeatMode == Player.REPEAT_MODE_ALL ||
                queueIndex > 0
            )

    fun hasPrevious(positionMs: Long): Boolean =
        activeTrack != null && (positionMs > 3_000L || hasPreviousInQueue)
}

class SonataViewModel(application: Application) : AndroidViewModel(application) {
    private data class CachedStream(
        val stream: AndroidResolvedStream,
        val cachedAtMs: Long,
    )

    private data class CachedArtistPage(
        val page: AndroidArtistPage,
        val cachedAtMs: Long,
    )

    private data class CachedVideoCandidate(
        val candidate: AndroidMusicVideoCandidate?,
        val cachedAtMs: Long,
    )

    private data class CachedResolvedMusicVideo(
        val resolved: AndroidResolvedMusicVideo,
        val cachedAtMs: Long,
    )

    private val engineDeferred = CompletableDeferred<SonataEngineGateway>()
    private val cipherSolver = WebViewJsCipherSolver(application)

    private val streamCache = ConcurrentHashMap<String, CachedStream>()
    private val artistPageCache = ConcurrentHashMap<String, CachedArtistPage>()
    private val inFlightPrefetch = ConcurrentHashMap.newKeySet<String>()
    private val trackCollectionContext = ConcurrentHashMap<String, String>()
    private val jioToYtmIdCache = ConcurrentHashMap<String, String>()
    private val resolvedArtistCardCache = ConcurrentHashMap<String, AndroidHomeCard>()
    private val inFlightArtistLookups = ConcurrentHashMap.newKeySet<String>()
    private val musicVideoCandidateCache = ConcurrentHashMap<String, CachedVideoCandidate>()
    private val resolvedMusicVideoCache = ConcurrentHashMap<String, CachedResolvedMusicVideo>()
    private var musicVideoProbeJob: Job? = null
    private var videoSwitchJob: Job? = null
    private var listeningHistoryJob: Job? = null
    private val historyLock = Any()
    private var localHistoryCache: List<AndroidTrack>? = null

    @OptIn(ExperimentalCoroutinesApi::class)
    private val prefetchDispatcher = Dispatchers.IO.limitedParallelism(1)

    val playbackClock = PlaybackClockState()

    val preferences = SonataPreferences(application)
    val downloadManager = SonataDownloadManager(application)

    val settings: StateFlow<SonataSettingsState> = preferences.state
    val downloads: StateFlow<List<DownloadedTrackRecord>> = downloadManager.downloads
    val activeDownloads: StateFlow<Map<String, ActiveDownload>> = downloadManager.activeDownloads
    val downloadsTotalBytes: StateFlow<Long> = downloadManager.totalBytes

    private val _currentCanvas = MutableStateFlow<CanvasMedia?>(null)
    val currentCanvas: StateFlow<CanvasMedia?> = _currentCanvas.asStateFlow()

    val audioPipelineInfo: StateFlow<AudioPipelineInfo> = AudioPipelineBus.info

    private val _sleepTimerRemainingMinutes = MutableStateFlow<Int?>(null)
    val sleepTimerRemainingMinutes: StateFlow<Int?> = _sleepTimerRemainingMinutes.asStateFlow()

    private val _activeLyricsSource = MutableStateFlow("Synced")
    val activeLyricsSource: StateFlow<String> = _activeLyricsSource.asStateFlow()

    private val _lastPlayedTracks = MutableStateFlow<List<AndroidTrack>>(emptyList())
    val lastPlayedTracks: StateFlow<List<AndroidTrack>> = _lastPlayedTracks.asStateFlow()

    private val _uiState = MutableStateFlow(SonataUiState())
    val uiState: StateFlow<SonataUiState> = _uiState.asStateFlow()

    private val _serverBrowserState = MutableStateFlow(ServerBrowserState())
    val serverBrowserState: StateFlow<ServerBrowserState> = _serverBrowserState.asStateFlow()

    private var serverBrowseJob: Job? = null
    private var serverTestJob: Job? = null
    private var serverSearchJob: Job? = null

    private val discordRpcClient = DiscordRpcClient()
    private val discordSeekSignal = MutableStateFlow(0L)
    val discordStatus: StateFlow<DiscordRpcStatus> = discordRpcClient.status
    val discordConnected: StateFlow<Boolean> = preferences.discordToken
        .map { !it.isNullOrBlank() }
        .stateIn(viewModelScope, SharingStarted.Eagerly, preferences.hasDiscordToken())

    private val playbackGeneration = PlaybackGenerationCounter()
    private val recoveryBudget = RecoveryBudget(MAX_RECOVERY_ATTEMPTS)
    @Volatile private var lastHandledEndedGeneration: Long = 0L

    private var controller: MediaController? = null
    private var playJob: Job? = null
    private var upgradeJob: Job? = null
    private var lyricsJob: Job? = null
    private var canvasJob: Job? = null
    private var sleepTimerJob: Job? = null
    private var refreshJob: Job? = null
    private var speculativePrefetchJob: Job? = null
    private var searchDebounceJob: Job? = null
    private var searchJob: Job? = null
    private var recoveryJob: Job? = null
    private var autoplayJob: Job? = null
    private var autoplayPrefetchJob: Job? = null
    private var artistJob: Job? = null
    private var collectionJob: Job? = null
    private var lyricsFetchedForTrackId: String? = null
    private var activeQueuePlaylistId: String? = null
    private val playedTrackIdsInCurrentQueue = linkedSetOf<String>()
    @Volatile private var prefetchedAutoplaySeedKey: String? = null
    @Volatile private var prefetchedAutoplayTracks: List<AndroidTrack> = emptyList()
    @Volatile private var sleepAtEndOfTrack: Boolean = false
    @Volatile private var cipherWarmupScheduled = false
    @Volatile private var lastHomeExploreFetchAtMs: Long = 0L
    @Volatile private var lastRadioFetchAtMs: Long = 0L
    @Volatile private var rateLimitBackoffUntilMs: Long = 0L
    @Volatile private var homeFetchInFlight: Boolean = false
    private val homeFetchSequence = AtomicLong(0L)
    @Volatile private var radioFetchInFlight: Boolean = false
    @Volatile private var lastAppliedLanguageCode: String? = null
    @Volatile private var fullHomeShelves: List<AndroidHomeShelf> = emptyList()
    private var radioRefreshJob: Job? = null
    private var homeMoodChipJob: Job? = null
    private var moodCategoryJob: Job? = null
    @Volatile private var latestLikedCoverUrl: String? = null

    private val prefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences("sonata_prefs", Context.MODE_PRIVATE)
    }

    private val historyPrefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences(LAST_PLAYED_PREFS_NAME, Context.MODE_PRIVATE)
    }

    private val localPlaylistsPrefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences(LOCAL_PLAYLISTS_PREFS_NAME, Context.MODE_PRIVATE)
    }

    private val localPlaylistsLock = Any()
    private val pinnedSavedLock = Any()
    private var cachedPinnedCollections: List<PinnedCollectionEntry> = emptyList()
    private var cachedSavedExternalPlaylists: List<SavedExternalPlaylist> = emptyList()

    private data class LocalPlaylistEntry(
        val id: String,
        val name: String,
        val tracks: List<AndroidTrack>,
    )

    private fun trackToJson(t: AndroidTrack): JSONObject = JSONObject().apply {
        put("id", t.id)
        put("title", t.title)
        put("artist", t.artist)
        put("album", t.album)
        put("durationSecs", t.durationSecs?.toLong() ?: 0L)
        put("thumbnailUrl", t.thumbnailUrl ?: "")
        put("liked", t.liked)
        put("albumId", t.albumId ?: "")
        put("isExplicit", t.isExplicit)
    }

    private fun jsonToTrack(obj: JSONObject?): AndroidTrack? {
        obj ?: return null
        val id = obj.optString("id").trim()
        val title = obj.optString("title").trim()
        if (id.isBlank() || title.isBlank()) return null
        return AndroidTrack(
            id = id,
            title = title,
            artist = obj.optString("artist"),
            album = obj.optString("album"),
            durationSecs = obj.optLong("durationSecs", 0L).takeIf { it > 0L }?.toULong(),
            thumbnailUrl = obj.optString("thumbnailUrl").takeIf { it.isNotBlank() },
            liked = obj.optBoolean("liked", false),
            albumId = obj.optString("albumId").takeIf { it.isNotBlank() },
            isExplicit = obj.optBoolean("isExplicit", false),
        )
    }

    private fun loadLocalPlaylistsLocked(): MutableList<LocalPlaylistEntry> {
        val raw = localPlaylistsPrefs.getString(LOCAL_PLAYLISTS_KEY, null) ?: return mutableListOf()
        return runCatching {
            val arr = JSONArray(raw)
            val out = mutableListOf<LocalPlaylistEntry>()
            for (i in 0 until arr.length()) {
                val obj = arr.optJSONObject(i) ?: continue
                val id = obj.optString("id").trim()
                val name = obj.optString("name").trim()
                if (id.isBlank() || name.isBlank()) continue
                val trackArr = obj.optJSONArray("tracks")
                val tracks = buildList {
                    if (trackArr != null) {
                        for (j in 0 until trackArr.length()) {
                            jsonToTrack(trackArr.optJSONObject(j))?.let { add(it) }
                        }
                    }
                }
                out.add(LocalPlaylistEntry(id = id, name = name, tracks = tracks))
            }
            out
        }.getOrDefault(mutableListOf())
    }

    private fun saveLocalPlaylistsLocked(entries: List<LocalPlaylistEntry>) {
        val arr = JSONArray()
        entries.forEach { entry ->
            arr.put(
                JSONObject().apply {
                    put("id", entry.id)
                    put("name", entry.name)
                    put(
                        "tracks",
                        JSONArray().apply {
                            entry.tracks.forEach { put(trackToJson(it)) }
                        },
                    )
                },
            )
        }
        localPlaylistsPrefs.edit().putString(LOCAL_PLAYLISTS_KEY, arr.toString()).apply()
    }

    private fun loadLocalPlaylistsAsAndroid(): List<AndroidPlaylist> = synchronized(localPlaylistsLock) {
        loadLocalPlaylistsLocked().map { entry ->
            AndroidPlaylist(
                id = entry.id,
                name = entry.name,
                thumbnailUrl = entry.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
            )
        }
    }

    private fun getLocalPlaylistTracks(playlistId: String): List<AndroidTrack> = synchronized(localPlaylistsLock) {
        loadLocalPlaylistsLocked()
            .firstOrNull { it.id == playlistId }
            ?.tracks
            .orEmpty()
    }

    private fun appendTracksToLocalPlaylist(playlistId: String, tracks: List<AndroidTrack>): Boolean =
        synchronized(localPlaylistsLock) {
            val entries = loadLocalPlaylistsLocked()
            val idx = entries.indexOfFirst { it.id == playlistId }
            if (idx < 0) return false
            val existing = entries[idx]
            val merged = (existing.tracks + tracks).distinctBy { it.id }
            entries[idx] = existing.copy(tracks = merged)
            saveLocalPlaylistsLocked(entries)
            true
        }

    private fun createLocalPlaylist(title: String, tracks: List<AndroidTrack>): AndroidPlaylist =
        synchronized(localPlaylistsLock) {
            val entries = loadLocalPlaylistsLocked()
            val id = "$LOCAL_PLAYLIST_PREFIX${System.currentTimeMillis()}"
            val uniqueTracks = tracks.distinctBy { it.id }
            val entry = LocalPlaylistEntry(id = id, name = title, tracks = uniqueTracks)
            entries.add(0, entry)
            saveLocalPlaylistsLocked(entries)
            AndroidPlaylist(
                id = entry.id,
                name = entry.name,
                thumbnailUrl = uniqueTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
            )
        }

    private fun isLikedMusicPlaylistId(id: String): Boolean =
        id.equals("VLLM", ignoreCase = true) || id.equals("LM", ignoreCase = true)

    private fun resolveTrackCoverUrl(track: AndroidTrack): String? =
        track.thumbnailUrl?.takeIf { it.isNotBlank() }
            ?: if (!track.id.startsWith("jiosaavn:") && !track.id.startsWith("local:") && track.id.isNotBlank()) {
                "https://i.ytimg.com/vi/${track.id}/hqdefault.jpg"
            } else {
                null
            }

    private fun applyLatestLikedCover(playlists: List<AndroidPlaylist>): List<AndroidPlaylist> {
        val likedCover = latestLikedCoverUrl?.takeIf { it.isNotBlank() } ?: return playlists
        return playlists.map { pl ->
            if (isLikedMusicPlaylistId(pl.id)) pl.copy(thumbnailUrl = likedCover) else pl
        }
    }

    private fun updateLatestLikedCover(coverUrl: String?, clearIfNull: Boolean = false) {
        val normalized = coverUrl?.takeIf { it.isNotBlank() }
        if (normalized != null) {
            latestLikedCoverUrl = normalized
            prefs.edit().putString("last_liked_cover_url", normalized).apply()
        } else if (clearIfNull) {
            latestLikedCoverUrl = null
            prefs.edit().remove("last_liked_cover_url").apply()
        } else {
            return
        }
        _uiState.update { state ->
            val updatedPlaylists = state.playlists.map { pl ->
                if (isLikedMusicPlaylistId(pl.id)) pl.copy(thumbnailUrl = normalized) else pl
            }
            val viewingLiked = state.collectionBrowseId?.let { isLikedMusicPlaylistId(it) } == true
            val updatedCollectionCover = if (viewingLiked) normalized else state.collectionCoverUrl
            if (updatedPlaylists == state.playlists && updatedCollectionCover == state.collectionCoverUrl) {
                state
            } else {
                state.copy(
                    playlists = updatedPlaylists,
                    collectionCoverUrl = updatedCollectionCover,
                )
            }
        }
        if (normalized != null) {
            syncCollectionMetadataToSavedAndPinned("VLLM", uiStrings().likedSongs, null, normalized)
        }
    }

    private fun computeOwnedPlaylistIds(
        remote: List<AndroidPlaylist>,
        savedExternalIds: Set<String>,
    ): Set<String> {
        val owned = LinkedHashSet<String>()
        remote.forEach { pl ->
            if (pl.isOwned) {
                val norm = normalizeCollectionId(pl.id)
                if (norm.startsWith("PL") && norm !in savedExternalIds) {
                    owned.add(norm)
                }
            }
        }
        return owned
    }

    private fun mergePlaylistsWithLocal(remote: List<AndroidPlaylist>): List<AndroidPlaylist> {
        val decoratedRemote = applyLatestLikedCover(remote)
        val local = loadLocalPlaylistsAsAndroid()
        val savedExternal = synchronized(pinnedSavedLock) { cachedSavedExternalPlaylists }
            .map { saved ->
                AndroidPlaylist(
                    id = saved.id,
                    name = saved.title,
                    thumbnailUrl = saved.coverUrl,
                    isOwned = false,
                )
            }
        val combined = decoratedRemote + savedExternal + local
        if (combined.isEmpty()) return emptyList()
        val seen = HashSet<String>(combined.size)
        return combined.filter { pl ->
            val key = normalizeCollectionId(pl.id).ifBlank { pl.id }
            seen.add(key)
        }
    }

    private fun loadPinnedCollectionsLocked(): List<PinnedCollectionEntry> {
        val raw = prefs.getString("pinned_collections_v1", null) ?: return emptyList()
        return runCatching {
            val arr = JSONArray(raw)
            buildList {
                for (i in 0 until arr.length()) {
                    val obj = arr.optJSONObject(i) ?: continue
                    val id = obj.optString("id").trim()
                    val title = obj.optString("title").trim()
                    if (id.isBlank() || title.isBlank()) continue
                    add(
                        PinnedCollectionEntry(
                            id = id,
                            title = title,
                            subtitle = obj.optString("subtitle"),
                            coverUrl = obj.optString("coverUrl").takeIf { it.isNotBlank() },
                            kindLabel = obj.optString("kindLabel", "Playlist").ifBlank { "Playlist" },
                            isAlbum = obj.optBoolean("isAlbum", false),
                            pinnedAtMs = obj.optLong("pinnedAtMs", 0L),
                        ),
                    )
                }
            }
        }.getOrDefault(emptyList())
    }

    private fun savePinnedCollectionsLocked(entries: List<PinnedCollectionEntry>) {
        val arr = JSONArray()
        entries.forEach { entry ->
            arr.put(
                JSONObject().apply {
                    put("id", entry.id)
                    put("title", entry.title)
                    put("subtitle", entry.subtitle)
                    put("coverUrl", entry.coverUrl ?: "")
                    put("kindLabel", entry.kindLabel)
                    put("isAlbum", entry.isAlbum)
                    put("pinnedAtMs", entry.pinnedAtMs)
                },
            )
        }
        prefs.edit().putString("pinned_collections_v1", arr.toString()).apply()
    }

    private fun loadSavedExternalPlaylistsLocked(): List<SavedExternalPlaylist> {
        val raw = prefs.getString("saved_external_playlists_v1", null) ?: return emptyList()
        return runCatching {
            val arr = JSONArray(raw)
            buildList {
                for (i in 0 until arr.length()) {
                    val obj = arr.optJSONObject(i) ?: continue
                    val id = obj.optString("id").trim()
                    val title = obj.optString("title").trim()
                    if (id.isBlank() || title.isBlank()) continue
                    add(
                        SavedExternalPlaylist(
                            id = id,
                            title = title,
                            subtitle = obj.optString("subtitle"),
                            coverUrl = obj.optString("coverUrl").takeIf { it.isNotBlank() },
                            kindLabel = obj.optString("kindLabel", "Playlist").ifBlank { "Playlist" },
                            savedAtMs = obj.optLong("savedAtMs", 0L),
                        ),
                    )
                }
            }
        }.getOrDefault(emptyList())
    }

    private fun saveExternalPlaylistsLocked(entries: List<SavedExternalPlaylist>) {
        val arr = JSONArray()
        entries.forEach { entry ->
            arr.put(
                JSONObject().apply {
                    put("id", entry.id)
                    put("title", entry.title)
                    put("subtitle", entry.subtitle)
                    put("coverUrl", entry.coverUrl ?: "")
                    put("kindLabel", entry.kindLabel)
                    put("savedAtMs", entry.savedAtMs)
                },
            )
        }
        prefs.edit().putString("saved_external_playlists_v1", arr.toString()).apply()
    }

    private fun initPinnedAndSavedFromPrefs() {
        val (pinned, saved) = synchronized(pinnedSavedLock) {
            cachedPinnedCollections = loadPinnedCollectionsLocked()
            cachedSavedExternalPlaylists = loadSavedExternalPlaylistsLocked()
            cachedPinnedCollections to cachedSavedExternalPlaylists
        }
        val savedIds = saved.mapTo(HashSet()) { normalizeCollectionId(it.id) }
        _uiState.update { state ->
            state.copy(
                pinnedCollections = pinned,
                savedExternalPlaylistIds = savedIds,
            )
        }
    }

    private fun syncCollectionMetadataToSavedAndPinned(
        id: String,
        title: String,
        subtitle: String?,
        coverUrl: String?,
    ) {
        val norm = normalizeCollectionId(id)
        if (norm.isBlank()) return
        val cleanTitle = title.trim()
        val cleanCover = coverUrl?.takeIf { it.isNotBlank() }
        val cleanSubtitle = subtitle?.trim()
        var pinnedUpdated: List<PinnedCollectionEntry>? = null
        var savedUpdated: List<SavedExternalPlaylist>? = null
        synchronized(pinnedSavedLock) {
            val nextPinned = cachedPinnedCollections.map { entry ->
                if (normalizeCollectionId(entry.id) == norm) {
                    entry.copy(
                        title = cleanTitle.ifBlank { entry.title },
                        subtitle = cleanSubtitle?.takeIf { it.isNotBlank() } ?: entry.subtitle,
                        coverUrl = cleanCover ?: entry.coverUrl,
                    )
                } else {
                    entry
                }
            }
            if (nextPinned != cachedPinnedCollections) {
                cachedPinnedCollections = nextPinned
                savePinnedCollectionsLocked(nextPinned)
                pinnedUpdated = nextPinned
            }

            val nextSaved = cachedSavedExternalPlaylists.map { entry ->
                if (normalizeCollectionId(entry.id) == norm) {
                    entry.copy(
                        title = cleanTitle.ifBlank { entry.title },
                        subtitle = cleanSubtitle?.takeIf { it.isNotBlank() } ?: entry.subtitle,
                        coverUrl = cleanCover ?: entry.coverUrl,
                    )
                } else {
                    entry
                }
            }
            if (nextSaved != cachedSavedExternalPlaylists) {
                cachedSavedExternalPlaylists = nextSaved
                saveExternalPlaylistsLocked(nextSaved)
                savedUpdated = nextSaved
            }
        }
        if (pinnedUpdated != null || savedUpdated != null) {
            _uiState.update { state ->
                val updatedPlaylists = if (savedUpdated != null) {
                    state.playlists.map { pl ->
                        if (normalizeCollectionId(pl.id) == norm) {
                            pl.copy(
                                name = cleanTitle.ifBlank { pl.name },
                                thumbnailUrl = cleanCover ?: pl.thumbnailUrl,
                            )
                        } else {
                            pl
                        }
                    }
                } else {
                    state.playlists
                }
                state.copy(
                    pinnedCollections = pinnedUpdated ?: state.pinnedCollections,
                    playlists = updatedPlaylists,
                )
            }
        }
    }

    private fun getCachedArtistPage(key: String): AndroidArtistPage? {
        val norm = key.trim().lowercase()
        if (norm.isBlank()) return null
        val entry = artistPageCache[norm] ?: return null
        if (SystemClock.elapsedRealtime() - entry.cachedAtMs >= ARTIST_CACHE_TTL_MS) {
            artistPageCache.remove(norm, entry)
            return null
        }
        return entry.page
    }

    private fun putCachedArtistPage(page: AndroidArtistPage, queryName: String = "") {
        val now = SystemClock.elapsedRealtime()
        val entry = CachedArtistPage(page = page, cachedAtMs = now)
        if (page.id.isNotBlank()) {
            artistPageCache[page.id.trim().lowercase()] = entry
        }
        if (page.name.isNotBlank()) {
            artistPageCache["name:${page.name.trim().lowercase()}"] = entry
        }
        if (queryName.isNotBlank()) {
            artistPageCache["name:${queryName.trim().lowercase()}"] = entry
        }
        if (artistPageCache.size > ARTIST_CACHE_MAX_ENTRIES * 2) {
            artistPageCache.entries
                .sortedBy { it.value.cachedAtMs }
                .take(artistPageCache.size - ARTIST_CACHE_MAX_ENTRIES * 2)
                .forEach { artistPageCache.remove(it.key, it.value) }
        }
        val primaryKey = page.name.trim().lowercase()
        if (primaryKey.isNotBlank() && page.id.startsWith("UC")) {
            val card = AndroidHomeCard(
                kind = AndroidHomeCardKind.ARTIST,
                id = page.id,
                title = page.name,
                subtitle = page.monthlyListeners ?: page.subscribers ?: uiStrings().artistDefault,
                thumbnailUrl = page.thumbnailUrl,
                track = null,
            )
            resolvedArtistCardCache[primaryKey] = card
            queryName.trim().lowercase().takeIf { it.isNotBlank() }?.let {
                resolvedArtistCardCache.putIfAbsent(it, card)
            }
        }
    }

    private fun seedArtistCardsFromShelves(shelves: List<AndroidHomeShelf>) {
        shelves.asSequence()
            .flatMap { it.cards.asSequence() }
            .filter { it.kind == AndroidHomeCardKind.ARTIST && it.title.isNotBlank() }
            .forEach { card ->
                val key = card.title.trim().lowercase()
                val existing = resolvedArtistCardCache[key]
                if (existing == null || (!existing.id.startsWith("UC") && card.id.startsWith("UC"))) {
                    resolvedArtistCardCache[key] = card
                }
            }
    }

    private fun buildRecentArtistCards(
        historyTracks: List<AndroidTrack>,
        shelves: List<AndroidHomeShelf>,
    ): List<AndroidHomeCard> {
        seedArtistCardsFromShelves(shelves)
        val merged = linkedMapOf<String, AndroidHomeCard>()
        for (track in historyTracks) {
            val primary = extractPrimaryArtistName(track.artist)
            val key = primary.lowercase()
            if (key.isBlank() || merged.containsKey(key)) continue
            val cached = resolvedArtistCardCache[key]
            merged[key] = if (cached != null) {
                if (cached.thumbnailUrl.isNullOrBlank() && !track.thumbnailUrl.isNullOrBlank()) {
                    cached.copy(thumbnailUrl = track.thumbnailUrl)
                } else {
                    cached
                }
            } else {
                AndroidHomeCard(
                    kind = AndroidHomeCardKind.ARTIST,
                    id = "",
                    title = primary,
                    subtitle = uiStrings().recentlyPlayedSubtitle,
                    thumbnailUrl = track.thumbnailUrl,
                    track = null,
                )
            }
        }
        shelves.asSequence()
            .flatMap { it.cards.asSequence() }
            .filter { it.kind == AndroidHomeCardKind.ARTIST && it.title.isNotBlank() }
            .forEach { card ->
                val key = card.title.trim().lowercase()
                if (!merged.containsKey(key)) {
                    merged[key] = card
                }
            }
        return merged.values.take(16)
    }

    private fun ensureArtistCardEnriched(rawArtist: String) {
        val primary = extractPrimaryArtistName(rawArtist)
        val key = primary.lowercase()
        if (key.isBlank()) return
        val existing = resolvedArtistCardCache[key]
        if (existing != null && existing.id.startsWith("UC") && !existing.thumbnailUrl.isNullOrBlank()) {
            return
        }
        if (!inFlightArtistLookups.add(key)) return
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val engine = requireEngine()
                val page = getCachedArtistPage("name:$key")
                    ?: runSuspendCatching { engine.fetchArtistByName(primary) }.getOrNull()
                        ?.also { putCachedArtistPage(it, primary) }
                if (page != null && page.id.startsWith("UC")) {
                    val refreshed = buildRecentArtistCards(_lastPlayedTracks.value, _uiState.value.shelves)
                    _uiState.update { it.copy(recentArtistCards = refreshed) }
                }
            } finally {
                inFlightArtistLookups.remove(key)
            }
        }
    }

    private fun loadLocalLastPlayedHistory(): List<AndroidTrack> = synchronized(historyLock) {
        localHistoryCache?.let { return it }
        val raw = historyPrefs.getString(LAST_PLAYED_KEY, null)
        val loaded = if (raw == null) {
            emptyList()
        } else {
            runCatching {
                val arr = JSONArray(raw)
                val parsed = buildList {
                    for (i in 0 until arr.length()) {
                        val obj = arr.optJSONObject(i) ?: continue
                        val id = obj.optString("id").trim()
                        val title = obj.optString("title").trim()
                        if (id.isBlank() || title.isBlank()) continue
                        add(
                            AndroidTrack(
                                id = id,
                                title = title,
                                artist = obj.optString("artist"),
                                album = obj.optString("album"),
                                durationSecs = obj.optLong("durationSecs", 0L).takeIf { it > 0L }?.toULong(),
                                thumbnailUrl = obj.optString("thumbnailUrl").takeIf { it.isNotBlank() },
                                liked = obj.optBoolean("liked", false),
                                albumId = obj.optString("albumId").takeIf { it.isNotBlank() },
                                isExplicit = obj.optBoolean("isExplicit", false),
                            )
                        )
                    }
                }
                deduplicateTracks(parsed).take(MAX_LAST_PLAYED_HISTORY)
            }.getOrDefault(emptyList())
        }
        localHistoryCache = loaded
        loaded
    }

    private fun saveLocalLastPlayedHistory(tracks: List<AndroidTrack>) = synchronized(historyLock) {
        val clean = deduplicateTracks(tracks).take(MAX_LAST_PLAYED_HISTORY)
        localHistoryCache = clean
        val arr = JSONArray()
        clean.forEach { t ->
            arr.put(
                JSONObject().apply {
                    put("id", t.id)
                    put("title", t.title)
                    put("artist", t.artist)
                    put("album", t.album)
                    put("durationSecs", t.durationSecs?.toLong() ?: 0L)
                    put("thumbnailUrl", t.thumbnailUrl ?: "")
                    put("liked", t.liked)
                    put("albumId", t.albumId ?: "")
                    put("isExplicit", t.isExplicit)
                }
            )
        }
        historyPrefs.edit().putString(LAST_PLAYED_KEY, arr.toString()).apply()
    }

    private fun recordLastPlayed(track: AndroidTrack) {
        if (track.id.isBlank() || track.title.isBlank()) return
        val trackKey = canonicalTrackKey(track)
        val merged = synchronized(historyLock) {
            val currentLocal = loadLocalLastPlayedHistory()
            val updatedLocal = deduplicateTracks(
                listOf(track) + currentLocal.filterNot {
                    it.id == track.id || canonicalTrackKey(it) == trackKey
                }
            ).take(MAX_LAST_PLAYED_HISTORY)
            saveLocalLastPlayedHistory(updatedLocal)
            deduplicateTracks(
                updatedLocal + _lastPlayedTracks.value.filterNot { existing ->
                    existing.id == track.id ||
                        canonicalTrackKey(existing) == trackKey ||
                        updatedLocal.any { it.id == existing.id || canonicalTrackKey(it) == canonicalTrackKey(existing) }
                }
            ).take(MAX_LAST_PLAYED_HISTORY)
        }
        _lastPlayedTracks.value = merged
        val updatedArtists = buildRecentArtistCards(merged, _uiState.value.shelves)
        _uiState.update { it.copy(lastPlayedTracks = merged, recentArtistCards = updatedArtists) }
        ensureArtistCardEnriched(track.artist)
    }

    private fun mergeYtmListeningHistory(ytmHistory: List<AndroidTrack>) {
        if (ytmHistory.isEmpty()) return
        val merged = synchronized(historyLock) {
            val local = loadLocalLastPlayedHistory()
            deduplicateTracks(local + ytmHistory).take(MAX_LAST_PLAYED_HISTORY)
        }
        _lastPlayedTracks.value = merged
        val updatedArtists = buildRecentArtistCards(merged, _uiState.value.shelves)
        _uiState.update { it.copy(lastPlayedTracks = merged, recentArtistCards = updatedArtists) }
    }

    fun refreshListeningHistory() {
        if (listeningHistoryJob?.isActive == true) return
        listeningHistoryJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) return@launch
            runSuspendCatching { engine.fetchListeningHistory() }
                .onSuccess { history ->
                    mergeYtmListeningHistory(history)
                }
                .onFailure { err ->
                    handleRateLimitIfNeeded(err)
                    Log.w("SonataLibrary", "Listening history refresh failed: ${err.message}")
                }
        }
    }

    private fun resolvePlaybackPlaylistId(
        track: AndroidTrack,
        queueList: List<AndroidTrack>?,
    ): String? {
        val state = _uiState.value
        val activeCollection = state.collectionBrowseId?.takeIf {
            !it.startsWith(LOCAL_PLAYLIST_PREFIX) && it != DOWNLOADS_PLAYLIST_ID
        }
        if (activeCollection != null && state.collectionTracks.any { it.id == track.id }) {
            return activeCollection
        }
        if (queueList != null) {
            trackCollectionContext.remove(track.id)
            return null
        }
        return trackCollectionContext[track.id]
    }

    private suspend fun resolveYtmVideoIdForTrack(
        engine: SonataEngineGateway,
        track: AndroidTrack,
    ): String? {
        val rawId = track.id.trim()
        if (rawId.isBlank() || rawId.startsWith("local:")) return null
        if (!rawId.startsWith("jiosaavn:")) return rawId
        jioToYtmIdCache[rawId]?.let { return it }

        val query = listOf(track.title.trim(), track.artist.trim())
            .filter { it.isNotBlank() }
            .joinToString(" ")
        if (query.isBlank()) return null

        val match = runSuspendCatching { engine.searchSongs(query) }
            .getOrNull()
            ?.firstOrNull { cand ->
                cand.id.isNotBlank() &&
                    !cand.id.startsWith("jiosaavn:") &&
                    !cand.id.startsWith("local:")
            }
            ?.id
            ?.takeIf { it.isNotBlank() }

        if (match != null) {
            if (jioToYtmIdCache.size > 256) jioToYtmIdCache.clear()
            jioToYtmIdCache[rawId] = match
        }
        return match
    }

    private suspend fun resolveYtmVideoIds(
        engine: SonataEngineGateway,
        tracks: List<AndroidTrack>,
    ): List<String> = tracks.mapNotNull { resolveYtmVideoIdForTrack(engine, it) }.distinct()

    private fun reportTrackPlaybackToYtm(track: AndroidTrack, playlistId: String?) {
        if (track.id.isBlank() || track.id.startsWith("local:") || isSelfHostedId(track.id)) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) return@launch
            val ytmVideoId = resolveYtmVideoIdForTrack(engine, track) ?: return@launch
            runSuspendCatching {
                engine.reportPlayback(
                    videoId = ytmVideoId,
                    playlistId = null,
                    durationSecs = track.durationSecs,
                )
            }.onFailure { err ->
                Log.w("SonataTelemetry", "YTM playback report failed for '$ytmVideoId': ${err.message}")
            }
            delay(3_500L)
            runSuspendCatching { engine.fetchListeningHistory() }
                .onSuccess { history -> mergeYtmListeningHistory(history) }
                .onFailure { handleRateLimitIfNeeded(it) }
        }
    }

    private fun uiStrings(): SonataStrings =
        resolveSonataStrings(preferences.state.value.appLanguage)

    private fun resolveEffectiveLanguageCode(appLanguage: AppLanguage): String = when (appLanguage) {
        AppLanguage.TURKISH -> "tr"
        AppLanguage.RUSSIAN -> "ru"
        AppLanguage.ENGLISH -> "en"
        AppLanguage.SYSTEM -> when (Locale.getDefault().language.lowercase(Locale.ROOT)) {
            "tr" -> "tr"
            "ru" -> "ru"
            else -> "en"
        }
    }

    private fun resolvedDownloadCoverUrl(track: AndroidTrack): String? =
        track.thumbnailUrl?.takeIf { it.isNotBlank() }
            ?: if (!track.id.startsWith("jiosaavn:") && !track.id.startsWith("local:") && !isSelfHostedId(track.id)) {
                "https://i.ytimg.com/vi/${track.id}/hqdefault.jpg"
            } else {
                null
            }

    private suspend fun requireEngine(): SonataEngineGateway = engineDeferred.await()

    private fun getCachedStream(trackId: String): AndroidResolvedStream? {
        val entry = streamCache[trackId] ?: return null
        if (SystemClock.elapsedRealtime() - entry.cachedAtMs >= STREAM_CACHE_TTL_MS) {
            streamCache.remove(trackId, entry)
            return null
        }
        return entry.stream
    }

    private fun putCachedStream(trackId: String, stream: AndroidResolvedStream) {
        val now = SystemClock.elapsedRealtime()
        streamCache[trackId] = CachedStream(stream = stream, cachedAtMs = now)
        if (streamCache.size > STREAM_CACHE_MAX_ENTRIES) {
            streamCache.entries
                .sortedBy { it.value.cachedAtMs }
                .take(streamCache.size - STREAM_CACHE_MAX_ENTRIES)
                .forEach { streamCache.remove(it.key, it.value) }
        }
    }

    init {
        val displayManager = application.getSystemService(DisplayManager::class.java)
        val defaultDisplay = displayManager?.getDisplay(Display.DEFAULT_DISPLAY)
        val supports120Hz = defaultDisplay?.supportedModes?.any { it.refreshRate >= 115f } ?: false
        val initialSettings = preferences.state.value
        val resolvedFps = if (initialSettings.fpsLimit >= 120 && supports120Hz) 120 else 60

        _uiState.value = SonataUiState(
            loading = true,
            refreshing = false,
            status = uiStrings().loadingYoutubeMusic,
            repeatMode = initialSettings.repeatMode,
            shuffleEnabled = initialSettings.shuffleEnabled,
            preferHighQuality = initialSettings.preferHighQuality,
            animatedArtwork = initialSettings.animatedArtwork,
            crossfadeEnabled = initialSettings.crossfadeEnabled,
            liquidGlassEnabled = initialSettings.liquidGlassEnabled,
            fpsLimit = resolvedFps,
            supports120Hz = supports120Hz,
        )

        applyAudioSettings(initialSettings)
        observePreferences(supports120Hz)
        observeDiscordPresence()
        registerQueueBridge()
        startPositionPoller()
        startColdLaunchBootstrap(application, supports120Hz)
    }

    private fun observeDiscordPresence() {
        viewModelScope.launch {
            val playbackFlow = _uiState
                .map { Triple(it.activeTrack, it.isPlaying, it.isBuffering) }
                .distinctUntilChanged()
            combine(
                preferences.discordToken,
                preferences.state,
                playbackFlow,
                discordSeekSignal,
            ) { token, prefsState, (track, isPlaying, isBuffering), _ ->
                val config = DiscordPresenceConfig(
                    enabled = prefsState.discordRpcEnabled,
                    appId = prefsState.discordAppId,
                    activityType = prefsState.discordActivityType,
                    showTimestamps = prefsState.discordShowTimestamps,
                    showArtwork = prefsState.discordShowArtwork,
                    showButtons = prefsState.discordShowButtons,
                )
                val presenceTrack = track?.let { active ->
                    val durationMs = playbackClock.durationMs.takeIf { it > 0L }
                        ?: ((active.durationSecs?.toLong() ?: 0L) * 1000L)
                    DiscordPresenceTrack(
                        id = active.id,
                        title = active.title,
                        artist = active.artist,
                        album = active.album,
                        durationMs = durationMs,
                        artworkUrl = active.thumbnailUrl,
                        isPlaying = isPlaying && !isBuffering,
                        positionMs = playbackClock.positionMs,
                        playbackSpeed = prefsState.playbackSpeed,
                    )
                }
                Triple(token, config, presenceTrack)
            }.collect { (token, config, presenceTrack) ->
                discordRpcClient.updateAuthAndConfig(token, config)
                discordRpcClient.updatePresence(presenceTrack)
            }
        }
    }

    fun saveDiscordToken(token: String) {
        val cleaned = token.trim()
        if (cleaned.isEmpty()) return
        preferences.setDiscordToken(cleaned)
        if (!preferences.state.value.discordRpcEnabled) {
            preferences.update { it.copy(discordRpcEnabled = true) }
        }
    }

    fun signOutDiscord() {
        preferences.clearDiscordToken()
        discordRpcClient.updateAuthAndConfig(
            newToken = null,
            newConfig = DiscordPresenceConfig(enabled = false, appId = preferences.state.value.discordAppId),
        )
    }

    fun reconnectDiscord() {
        discordRpcClient.reconnect()
    }

    private fun applyAudioSettings(state: SonataSettingsState) {
        PlaybackAudioBridge.applyPlaybackSpeed(state.playbackSpeed)
        PlaybackAudioBridge.applySkipSilence(state.skipSilenceEnabled)
        PlaybackAudioBridge.applyEqualizer(state.eqEnabled, state.eqBandsDb)
        PlaybackAudioBridge.applyLoudnessNormalization(state.loudnessNormEnabled)
        PlaybackAudioBridge.applyCrossfade(state.crossfadeEnabled, state.crossfadeSeconds)
        PlaybackAudioBridge.stopMusicOnClose = state.stopMusicOnClose
    }

    private fun observePreferences(supports120Hz: Boolean) {
        viewModelScope.launch {
            var prevPrefs: SonataSettingsState? = null
            preferences.state.collect { prefsState ->
                val resolvedFps = if (prefsState.fpsLimit >= 120 && supports120Hz) 120 else 60
                _uiState.update { ui ->
                    if (ui.preferHighQuality == prefsState.preferHighQuality &&
                        ui.animatedArtwork == prefsState.animatedArtwork &&
                        ui.crossfadeEnabled == prefsState.crossfadeEnabled &&
                        ui.liquidGlassEnabled == prefsState.liquidGlassEnabled &&
                        ui.repeatMode == prefsState.repeatMode &&
                        ui.shuffleEnabled == prefsState.shuffleEnabled &&
                        ui.fpsLimit == resolvedFps
                    ) {
                        ui
                    } else {
                        ui.copy(
                            preferHighQuality = prefsState.preferHighQuality,
                            animatedArtwork = prefsState.animatedArtwork,
                            crossfadeEnabled = prefsState.crossfadeEnabled,
                            liquidGlassEnabled = prefsState.liquidGlassEnabled,
                            repeatMode = prefsState.repeatMode,
                            shuffleEnabled = prefsState.shuffleEnabled,
                            fpsLimit = resolvedFps,
                        )
                    }
                }
                if (controller?.repeatMode != prefsState.repeatMode) {
                    runCatching { controller?.repeatMode = prefsState.repeatMode }
                }
                if (!prefsState.animatedArtwork) {
                    canvasJob?.cancel()
                    _currentCanvas.value = null
                } else if (_currentCanvas.value == null) {
                    _uiState.value.activeTrack?.let { resolveCanvasForTrack(it) }
                }
                val old = prevPrefs
                if (old == null ||
                    old.playbackSpeed != prefsState.playbackSpeed ||
                    old.skipSilenceEnabled != prefsState.skipSilenceEnabled ||
                    old.eqEnabled != prefsState.eqEnabled ||
                    old.eqBandsDb != prefsState.eqBandsDb ||
                    old.loudnessNormEnabled != prefsState.loudnessNormEnabled ||
                    old.crossfadeEnabled != prefsState.crossfadeEnabled ||
                    old.crossfadeSeconds != prefsState.crossfadeSeconds ||
                    old.stopMusicOnClose != prefsState.stopMusicOnClose
                ) {
                    applyAudioSettings(prefsState)
                }
                prevPrefs = prefsState

                val effectiveLang = resolveEffectiveLanguageCode(prefsState.appLanguage)
                val previousLang = lastAppliedLanguageCode
                if (previousLang != null && previousLang != effectiveLang) {
                    lastAppliedLanguageCode = effectiveLang
                    artistPageCache.clear()
                    resolvedArtistCardCache.clear()
                    streamCache.clear()
                    lastHomeExploreFetchAtMs = 0L
                    lastRadioFetchAtMs = 0L
                    viewModelScope.launch(Dispatchers.IO) {
                        val engine = requireEngine()
                        runSuspendCatching { engine.setLanguage(effectiveLang) }
                        loadHome(forceRefresh = true)
                    }
                }
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun startColdLaunchBootstrap(application: Application, supports120Hz: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            // 1. Belt-and-suspenders removal of any legacy Home feed cache + non-blocking prefs load.
            runCatching { File(application.cacheDir, "sonata/android_home.json").delete() }

            val prefsFps = preferences.state.value.fpsLimit
            val savedFps = if (prefsFps >= 120 && supports120Hz) 120 else 60
            val restoredTrack = loadSavedActiveTrack(emptyList())
            val savedBadge = prefs.getString("last_track_badge", null).orEmpty()
            latestLikedCoverUrl = prefs.getString("last_liked_cover_url", null)?.takeIf { it.isNotBlank() }
            val savedHistory = loadLocalLastPlayedHistory()
            val initialQueue = listOfNotNull(restoredTrack)
            val initialQueueIndex = if (restoredTrack != null) 0 else -1

            _lastPlayedTracks.value = savedHistory
            if (restoredTrack != null) {
                playbackClock.resetForTrack((restoredTrack.durationSecs?.toLong() ?: 0L) * 1000L)
                resolveCanvasForTrack(restoredTrack)
                probeMusicVideoForTrack(restoredTrack)
            }
            _uiState.update {
                it.copy(
                    lastPlayedTracks = savedHistory,
                    queue = if (it.queue.isEmpty()) initialQueue else it.queue,
                    queueIndex = if (it.queueIndex < 0) initialQueueIndex else it.queueIndex,
                    activeTrack = it.activeTrack ?: restoredTrack,
                    streamBadge = if (it.streamBadge.isEmpty()) savedBadge else it.streamBadge,
                    fpsLimit = savedFps,
                    supports120Hz = supports120Hz,
                )
            }

            // Pre-warm ExoPlayer's SimpleCache on IO so MediaSessionService.onCreate() does 0ms Main disk I/O.
            launch(Dispatchers.IO) {
                runCatching { PlaybackService.getOrCreateCache(application) }
                withContext(Dispatchers.Main) {
                    connectMediaController()
                }
            }

            // 2. Construct the Rust UniFFI engine off the Main thread.
            val engine: SonataEngineGateway = runCatching {
                NativeSonataEngineGateway(
                    SonataAndroidEngine(
                        configDir = application.filesDir.absolutePath,
                        cacheDir = application.cacheDir.absolutePath,
                    ).also { eng ->
                        runCatching { eng.setJsCipherSolver(cipherSolver) }
                    }
                )
            }.getOrElse { err ->
                Log.e("SonataEngine", "Failed to initialize SonataAndroidEngine", err)
                _uiState.update {
                    it.copy(
                        loading = false,
                        authInitialized = true,
                        status = uiStrings().engineInitErrorStatus(err.message.orEmpty()),
                    )
                }
                return@launch
            }
            val initialLangCode = resolveEffectiveLanguageCode(preferences.state.value.appLanguage)
            lastAppliedLanguageCode = initialLangCode
            runSuspendCatching { engine.setLanguage(initialLangCode) }
            CanvasResolver.attachGateway(engine)
            engineDeferred.complete(engine)

            // 3. Load lightweight cached auth/profile/playlists (never Home) for instant Library & Profile.
            val installedVer = runCatching {
                application.packageManager.getPackageInfo(application.packageName, 0).versionName.orEmpty()
            }.getOrDefault("").ifBlank { "1.0.0" }
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val cachedProfile = if (signedIn) runSuspendCatching { engine.getCachedAccountProfile() }.getOrNull() else null
            initPinnedAndSavedFromPrefs()
            val cachedPlaylists = if (signedIn) runSuspendCatching { engine.getCachedLibraryPlaylists() }.getOrDefault(emptyList()) else emptyList()
            val savedExternalIds = _uiState.value.savedExternalPlaylistIds
            val ownedIds = if (signedIn) computeOwnedPlaylistIds(cachedPlaylists, savedExternalIds) else emptySet()
            val mergedCachedPlaylists = mergePlaylistsWithLocal(cachedPlaylists)
            val initialServers = runSuspendCatching { engine.listServers() }.getOrDefault(emptyList())

            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = signedIn,
                    installedVersionName = installedVer,
                    accountProfile = cachedProfile ?: it.accountProfile,
                    ownedPlaylistIds = if (it.ownedPlaylistIds.isEmpty()) ownedIds else it.ownedPlaylistIds,
                    playlists = if (it.playlists.isEmpty()) mergedCachedPlaylists else it.playlists,
                    servers = initialServers,
                )
            }

            // 4. Immediately fetch fresh live Home shelves (never disk-cached).
            refreshInBackground(showBlockingSpinner = true)

            if (restoredTrack != null && lyricsFetchedForTrackId != restoredTrack.id) {
                fetchLyricsInBackground(restoredTrack)
            }

            // 5. Check for updates shortly after launch frames settle and show popup if newer APK exists.
            launch(Dispatchers.IO) {
                delay(2_200L)
                checkForUpdates(autoPopup = true, isManual = false)
            }
        }
    }

    private fun saveActiveTrackToPrefs(track: AndroidTrack, badge: String = "") {
        dev.sonata.android.widget.SonataWidgetUpdater.pushUpdate(
            context = getApplication(),
            title = track.title,
            artist = track.artist,
            artworkUrl = track.thumbnailUrl,
            isPlaying = _uiState.value.isPlaying,
            positionMs = playbackClock.positionMs.coerceAtLeast(0L),
            durationMs = (track.durationSecs?.toLong() ?: 0L) * 1000L,
        )
        prefs.edit()
            .putString("last_track_id", track.id)
            .putString("last_track_title", track.title)
            .putString("last_track_artist", track.artist)
            .putString("last_track_album", track.album)
            .putLong("last_track_duration_secs", track.durationSecs?.toLong() ?: 0L)
            .putString("last_track_thumb", track.thumbnailUrl)
            .putString("last_track_album_id", track.albumId)
            .putBoolean("last_track_explicit", track.isExplicit)
            .apply {
                if (badge.isNotBlank()) putString("last_track_badge", badge)
            }
            .apply()
    }

    private fun loadSavedActiveTrack(fallbackTracks: List<AndroidTrack>): AndroidTrack? {
        val id = prefs.getString("last_track_id", null)?.takeIf { it.isNotBlank() }
            ?: return fallbackTracks.firstOrNull()
        val title = prefs.getString("last_track_title", null).orEmpty()
        if (title.isBlank()) return fallbackTracks.firstOrNull()
        val artist = prefs.getString("last_track_artist", null).orEmpty()
        val album = prefs.getString("last_track_album", null).orEmpty()
        val dur = prefs.getLong("last_track_duration_secs", 0L).takeIf { it > 0L }?.toULong()
        val thumb = prefs.getString("last_track_thumb", null)
        val albumId = prefs.getString("last_track_album_id", null)?.takeIf { it.isNotBlank() }
        val isExplicit = prefs.getBoolean("last_track_explicit", false)
        return AndroidTrack(
            id = id,
            title = title,
            artist = artist,
            album = album,
            durationSecs = dur,
            thumbnailUrl = thumb,
            liked = false,
            albumId = albumId,
            isExplicit = isExplicit,
        )
    }

    private fun isGenerationActive(generation: Long, trackId: String? = null): Boolean {
        if (generation == 0L) {
            return trackId == null || _uiState.value.activeTrack?.id == trackId
        }
        return PlaybackEligibility.canAcceptGenerationResult(
            expectedGeneration = generation,
            currentGeneration = playbackGeneration.current,
            expectedTrackId = trackId,
            activeTrackId = _uiState.value.activeTrack?.id,
        )
    }

    @OptIn(UnstableApi::class)
    private fun controllerSwapAdapter(ctrl: MediaController?): StreamSwapPlayer? {
        val activeCtrl = ctrl ?: return null
        return object : StreamSwapPlayer {
            override val currentPositionMs: Long
                get() = activeCtrl.currentPosition.coerceAtLeast(0L)

            override val isPlaying: Boolean
                get() = activeCtrl.isPlaying

            override val playWhenReady: Boolean
                get() = activeCtrl.playWhenReady

            override fun replaceMediaItem(
                track: AndroidTrack,
                stream: AndroidResolvedStream,
                positionMs: Long,
            ) {
                activeCtrl.setMediaItem(buildMediaItem(track, stream), positionMs)
            }

            override fun prepare() = activeCtrl.prepare()

            override fun play() = activeCtrl.play()

            override fun pause() = activeCtrl.pause()
        }
    }

    private fun fetchLyricsInBackground(
        track: AndroidTrack,
        generation: Long = playbackGeneration.current,
    ) {
        lyricsJob?.cancel()
        lyricsFetchedForTrackId = track.id
        lyricsJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                if (isGenerationActive(generation, track.id)) {
                    it.copy(lyricsLoading = true, lyricsUnavailable = false)
                } else {
                    it
                }
            }
            val fetched = runSuspendCatching { engine.fetchLyrics(track) }.getOrDefault(emptyList())
            if (isActive && isGenerationActive(generation, track.id)) {
                _activeLyricsSource.value = when {
                    fetched.any { it.words.isNotEmpty() } -> "Word-Synced"
                    fetched.isNotEmpty() -> "Synced"
                    else -> "Unavailable"
                }
                _uiState.update {
                    it.copy(
                        lyrics = fetched,
                        lyricsLoading = false,
                        lyricsUnavailable = fetched.isEmpty(),
                    )
                }
            }
        }
    }

    private fun registerQueueBridge() {
        PlaybackQueueBridge.canSkipNext = { _uiState.value.hasNext }
        PlaybackQueueBridge.canSkipPrevious = { _uiState.value.hasPrevious(playbackClock.positionMs) }
        PlaybackQueueBridge.shouldAutoAdvanceOnEnd = {
            val state = _uiState.value
            val hasPlayableNext = if (state.shuffleEnabled && state.repeatMode == Player.REPEAT_MODE_OFF) {
                state.queue.indices.any { idx ->
                    idx != state.queueIndex && state.queue[idx].id !in playedTrackIdsInCurrentQueue
                }
            } else {
                state.hasNext
            }
            PlaybackEligibility.shouldAutoAdvanceOnEnd(
                hasNext = hasPlayableNext,
                repeatMode = state.repeatMode,
                autoplayEnabled = settings.value.autoplayEnabled,
                hasActiveTrack = state.activeTrack != null,
                sleepAtEndOfTrack = sleepAtEndOfTrack,
            )
        }
        PlaybackQueueBridge.onSkipNext = { viewModelScope.launch { nextTrack() } }
        PlaybackQueueBridge.onSkipPrevious = { viewModelScope.launch { prevTrack() } }
        PlaybackQueueBridge.onPlayFromEmpty = { viewModelScope.launch { togglePlayPause() } }
        PlaybackQueueBridge.onTrackEnded = {
            val gen = playbackGeneration.current
            viewModelScope.launch { onTrackEnded(gen) }
        }
    }

    private fun collectWarmCandidates(shelves: List<AndroidHomeShelf>): List<AndroidTrack> {
        val fromTracks = shelves.flatMap { it.tracks }
        val fromCards = shelves.flatMap { it.cards }.mapNotNull { it.track }
        return (fromTracks + fromCards).distinctBy { it.id }.take(2)
    }

    private fun scheduleDeferredCipherWarmupAndPrefetch(candidates: List<AndroidTrack>) {
        speculativePrefetchJob?.cancel()
        speculativePrefetchJob = viewModelScope.launch(prefetchDispatcher) {
            // Warm the WebView cipher solver promptly so signed-in TVHTML5 Extreme streams work on the first tap.
            val engine = requireEngine()
            if (!cipherWarmupScheduled) {
                cipherWarmupScheduled = true
                delay(300L)
                runSuspendCatching { engine.warmCipherSolver() }
            }
            delay(1_800L)
            resolvePrefetchList(engine, candidates.distinctBy { it.id }.take(2))
        }
    }

    private fun handleRateLimitIfNeeded(err: Throwable) {
        val backoffMs = RateLimitPolicy.backoffMsFor(err, RATE_LIMIT_BACKOFF_MS) ?: return
        rateLimitBackoffUntilMs = SystemClock.elapsedRealtime() + backoffMs
        Log.w(
            "SonataRateLimit",
            "Typed RateLimited received; backing off feed requests for ${backoffMs / 1000}s"
        )
    }

    fun refreshHomeAndExploreRateLimited(
        forceManual: Boolean = false,
        showBlockingSpinner: Boolean = false,
        forceBypassCooldown: Boolean = false,
    ) {
        val now = SystemClock.elapsedRealtime()
        if (!forceBypassCooldown && (now < rateLimitBackoffUntilMs || homeFetchInFlight)) return
        val cooldown = if (forceManual) MANUAL_REFRESH_COOLDOWN_MS else AUTO_REFRESH_COOLDOWN_MS
        if (!forceBypassCooldown &&
            !showBlockingSpinner &&
            _uiState.value.shelves.isNotEmpty() &&
            now - lastHomeExploreFetchAtMs < cooldown
        ) {
            return
        }
        val fetchSeq = homeFetchSequence.incrementAndGet()
        homeFetchInFlight = true
        refreshJob?.cancel()
        refreshJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val engine = requireEngine()
                lastHomeExploreFetchAtMs = SystemClock.elapsedRealtime()
                _uiState.update {
                    it.copy(
                        loading = showBlockingSpinner && it.shelves.isEmpty(),
                        refreshing = forceManual && !showBlockingSpinner,
                    )
                }
                val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
                if (signedIn) {
                    launch {
                        runSuspendCatching { engine.getAccountProfile() }
                            .onSuccess { profile ->
                                if (profile != null && profile != _uiState.value.accountProfile) {
                                    _uiState.update { it.copy(signedIn = true, accountProfile = profile) }
                                }
                            }
                            .onFailure { err ->
                                Log.w("SonataLibrary", "Profile fetch failed: ${err.message}")
                            }
                    }
                    launch {
                        var playlists = runSuspendCatching { engine.fetchLibraryPlaylists() }
                            .onFailure { err ->
                                handleRateLimitIfNeeded(err)
                                Log.w("SonataLibrary", "Library playlists fetch failed: ${err.message}")
                            }
                            .getOrNull()
                        if ((playlists == null || playlists.size <= 1) && SystemClock.elapsedRealtime() >= rateLimitBackoffUntilMs) {
                            delay(600L)
                            playlists = runSuspendCatching { engine.fetchLibraryPlaylists() }
                                .onFailure { err ->
                                    handleRateLimitIfNeeded(err)
                                    Log.w("SonataLibrary", "Library playlists retry failed: ${err.message}")
                                }
                                .getOrNull() ?: playlists
                        }
                        if (playlists != null) {
                            val savedIds = _uiState.value.savedExternalPlaylistIds
                            val ownedIds = computeOwnedPlaylistIds(playlists, savedIds)
                            val merged = mergePlaylistsWithLocal(playlists)
                            _uiState.update { state ->
                                state.copy(
                                    playlists = merged,
                                    ownedPlaylistIds = ownedIds,
                                )
                            }
                        }
                    }
                    launch {
                        runSuspendCatching { engine.fetchListeningHistory() }
                            .onSuccess { history ->
                                mergeYtmListeningHistory(history)
                            }
                            .onFailure { err ->
                                handleRateLimitIfNeeded(err)
                                Log.w("SonataLibrary", "Listening history fetch failed: ${err.message}")
                            }
                    }
                    launch {
                        runSuspendCatching { engine.fetchCollectionTracks("VLLM") }
                            .onSuccess { likedTracks ->
                                if (likedTracks.isNotEmpty()) {
                                    val likedIds = likedTracks.mapNotNull { t -> t.id.takeIf { it.isNotBlank() } }
                                    if (likedIds.isNotEmpty()) {
                                        _uiState.update { state ->
                                            val mergedLiked = state.likedTrackIds + likedIds
                                            if (mergedLiked == state.likedTrackIds) state else state.copy(likedTrackIds = mergedLiked)
                                        }
                                    }
                                    updateLatestLikedCover(
                                        likedTracks.firstNotNullOfOrNull { resolveTrackCoverUrl(it) },
                                    )
                                }
                            }
                            .onFailure { err ->
                                handleRateLimitIfNeeded(err)
                                Log.w("SonataLibrary", "Liked music cover fetch failed: ${err.message}")
                            }
                    }
                } else {
                    val localOnly = mergePlaylistsWithLocal(emptyList())
                    _uiState.update { it.copy(playlists = localOnly, ownedPlaylistIds = emptySet()) }
                }

                val (homeResult, exploreResult, moodChipsResult) = coroutineScope {
                    val homeDeferred = async { runSuspendCatching { engine.fetchHomeFeed() } }
                    val exploreDeferred = async { runSuspendCatching { engine.fetchExploreFeed() } }
                    val moodChipsDeferred = async { runSuspendCatching { engine.fetchHomeMoodChips() } }
                    Triple(homeDeferred.await(), exploreDeferred.await(), moodChipsDeferred.await())
                }

                exploreResult
                    .onSuccess { explore ->
                        val cleanedExplore = normalizeAndMergeHomeShelves(explore.shelves)
                        _uiState.update { state ->
                            state.copy(
                                exploreShelves = cleanedExplore,
                                moodCategories = explore.moods,
                            )
                        }
                    }
                    .onFailure { err ->
                        handleRateLimitIfNeeded(err)
                        Log.w("SonataExplore", "Explore feed fetch failed: ${err.message}")
                    }

                moodChipsResult
                    .onSuccess { chips ->
                        _uiState.update { state ->
                            state.copy(homeMoodChips = chips)
                        }
                    }
                    .onFailure { err ->
                        handleRateLimitIfNeeded(err)
                        Log.w("SonataExplore", "Home mood chips fetch failed: ${err.message}")
                    }

                homeResult
                    .onSuccess { rawShelves ->
                        lastHomeExploreFetchAtMs = SystemClock.elapsedRealtime()
                        val cleanedShelves = normalizeAndMergeHomeShelves(rawShelves)
                        fullHomeShelves = cleanedShelves
                        val existingShelves = _uiState.value.shelves
                        val shelves = if (cleanedShelves == existingShelves) existingShelves else cleanedShelves
                        val activeMoodFilter = if (forceBypassCooldown) null else _uiState.value.selectedHomeMoodParams
                        val displayShelves = if (activeMoodFilter == null) shelves else existingShelves
                        val firstTracks = shelves.firstOrNull { it.tracks.isNotEmpty() }?.tracks.orEmpty()
                        val currentActive = _uiState.value.activeTrack
                        val fallbackActive = currentActive ?: firstTracks.firstOrNull()
                        if (currentActive == null && fallbackActive != null) {
                            saveActiveTrackToPrefs(fallbackActive)
                            playbackClock.resetForTrack((fallbackActive.durationSecs?.toLong() ?: 0L) * 1000L)
                            fetchLyricsInBackground(fallbackActive)
                            probeMusicVideoForTrack(fallbackActive)
                        }
                        val updatedArtists = buildRecentArtistCards(_lastPlayedTracks.value, displayShelves)
                        _uiState.update {
                            it.copy(
                                loading = false,
                                refreshing = false,
                                authInitialized = true,
                                signedIn = signedIn,
                                shelves = displayShelves,
                                selectedHomeMoodParams = activeMoodFilter,
                                recentArtistCards = updatedArtists,
                                tracks = if (it.tracks.isEmpty()) firstTracks else it.tracks,
                                queue = if (it.queue.isEmpty()) firstTracks else it.queue,
                                queueIndex = if (it.queueIndex < 0 && fallbackActive != null) 0 else it.queueIndex,
                                activeTrack = it.activeTrack ?: fallbackActive,
                                status = uiStrings().personalizedShelvesCount(displayShelves.size),
                            )
                        }
                        val warmList = (listOfNotNull(_uiState.value.activeTrack) + collectWarmCandidates(displayShelves))
                            .distinctBy { it.id }
                            .take(2)
                        scheduleDeferredCipherWarmupAndPrefetch(warmList)
                    }
                    .onFailure { err ->
                        handleRateLimitIfNeeded(err)
                        _uiState.update {
                            it.copy(
                                loading = false,
                                refreshing = false,
                                authInitialized = true,
                                status = if (it.shelves.isEmpty()) {
                                    uiStrings().homeErrorStatus(err.message.orEmpty())
                                } else {
                                    it.status
                                },
                            )
                        }
                    }
            } finally {
                if (homeFetchSequence.get() == fetchSeq) {
                    homeFetchInFlight = false
                }
            }
        }
    }

    fun refreshRadioRateLimited(forceManual: Boolean = false) {
        val now = SystemClock.elapsedRealtime()
        if (now < rateLimitBackoffUntilMs || radioFetchInFlight) return
        val cooldown = if (forceManual) MANUAL_REFRESH_COOLDOWN_MS else AUTO_REFRESH_COOLDOWN_MS
        if (_uiState.value.radioMixShelves.isNotEmpty() && now - lastRadioFetchAtMs < cooldown) {
            return
        }
        radioFetchInFlight = true
        radioRefreshJob?.cancel()
        radioRefreshJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val engine = requireEngine()
                lastRadioFetchAtMs = SystemClock.elapsedRealtime()
                runSuspendCatching { engine.fetchRadioMixes() }
                    .onSuccess { rawMixes ->
                        lastRadioFetchAtMs = SystemClock.elapsedRealtime()
                        val mixes = normalizeRadioMixShelves(rawMixes)
                        val existingMixes = _uiState.value.radioMixShelves
                        if ((mixes.isNotEmpty() || existingMixes.isEmpty()) && mixes != existingMixes) {
                            _uiState.update { it.copy(radioMixShelves = mixes) }
                        }
                    }
                    .onFailure { err ->
                        handleRateLimitIfNeeded(err)
                        Log.w("SonataRadio", "Radio mixes fetch failed: ${err.message}")
                    }
            } finally {
                radioFetchInFlight = false
            }
        }
    }

    private fun refreshInBackground(showBlockingSpinner: Boolean = false) {
        refreshHomeAndExploreRateLimited(forceManual = !showBlockingSpinner, showBlockingSpinner = showBlockingSpinner)
        refreshRadioRateLimited(forceManual = !showBlockingSpinner)
    }

    private fun connectMediaController() {
        val ctx = getApplication<Application>()
        val token = SessionToken(ctx, ComponentName(ctx, PlaybackService::class.java))
        val future = MediaController.Builder(ctx, token).buildAsync()
        future.addListener({
            runCatching {
                val ctrl = future.get()
                controller = ctrl
                ctrl.repeatMode = _uiState.value.repeatMode
                val currentItem = ctrl.currentMediaItem
                if (currentItem != null && currentItem.mediaId.isNotBlank()) {
                    val meta = currentItem.mediaMetadata
                    val liveTrack = _uiState.value.activeTrack?.takeIf { it.id == currentItem.mediaId }
                        ?: AndroidTrack(
                            id = currentItem.mediaId,
                            title = meta.title?.toString().orEmpty().ifBlank { "Playing" },
                            artist = meta.artist?.toString().orEmpty(),
                            album = meta.albumTitle?.toString().orEmpty(),
                            durationSecs = (ctrl.duration.coerceAtLeast(0L) / 1000L).takeIf { it > 0L }?.toULong(),
                            thumbnailUrl = meta.artworkUri?.toString(),
                            liked = false,
                        )
                    playbackClock.sync(
                        positionMs = ctrl.currentPosition.coerceAtLeast(0L),
                        durationMs = ctrl.duration.coerceAtLeast(0L),
                    )
                    _uiState.update {
                        it.copy(
                            activeTrack = liveTrack,
                            isPlaying = ctrl.isPlaying,
                            isBuffering = ctrl.playbackState == Player.STATE_BUFFERING,
                        )
                    }
                    if (_uiState.value.lyrics.isEmpty()) {
                        fetchLyricsInBackground(liveTrack)
                    }
                    if (_currentCanvas.value == null) {
                        resolveCanvasForTrack(liveTrack)
                    }
                    if (_uiState.value.musicVideoCandidate == null) {
                        probeMusicVideoForTrack(liveTrack)
                    }
                }
                ctrl.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(isPlaying: Boolean) {
                        val state = _uiState.value
                        val activeId = state.activeTrack?.id
                        val matchesActive = activeId != null && ctrl.currentMediaItem?.mediaId == activeId
                        if (matchesActive) {
                            playbackClock.sync(
                                positionMs = ctrl.currentPosition.coerceAtLeast(0L),
                                durationMs = ctrl.duration.coerceAtLeast(0L),
                            )
                            val buffering = ctrl.playbackState == Player.STATE_BUFFERING
                            if (state.isPlaying != isPlaying || state.isBuffering != buffering) {
                                _uiState.update {
                                    it.copy(
                                        isPlaying = isPlaying,
                                        isBuffering = buffering,
                                    )
                                }
                            }
                        } else if (!isPlaying && state.isPlaying) {
                            _uiState.update { it.copy(isPlaying = false) }
                        }
                    }

                    override fun onPlaybackStateChanged(playbackState: Int) {
                        val state = _uiState.value
                        val activeTrack = state.activeTrack
                        val activeId = activeTrack?.id
                        val matchesActive = activeId != null && ctrl.currentMediaItem?.mediaId == activeId
                        if (matchesActive) {
                            val readyDurationMs = ctrl.duration.coerceAtLeast(0L)
                            val readyDurationSecs = (readyDurationMs / 1000L).takeIf { it > 0L }?.toULong()
                            val trackWithDuration = if (
                                playbackState == Player.STATE_READY &&
                                readyDurationSecs != null &&
                                (activeTrack.durationSecs == null || activeTrack.durationSecs == 0uL)
                            ) {
                                activeTrack.copy(durationSecs = readyDurationSecs)
                            } else {
                                activeTrack
                            }
                            playbackClock.sync(
                                positionMs = ctrl.currentPosition.coerceAtLeast(0L),
                                durationMs = readyDurationMs,
                            )
                            val nextPlaying = ctrl.isPlaying && playbackState == Player.STATE_READY
                            val nextBuffering = playbackState == Player.STATE_BUFFERING
                            if (
                                state.activeTrack != trackWithDuration ||
                                state.isPlaying != nextPlaying ||
                                state.isBuffering != nextBuffering
                            ) {
                                _uiState.update {
                                    it.copy(
                                        activeTrack = trackWithDuration,
                                        isPlaying = nextPlaying,
                                        isBuffering = nextBuffering,
                                    )
                                }
                            }
                            if (playbackState == Player.STATE_READY) {
                                if (trackWithDuration !== activeTrack) {
                                    val badgeSnapshot = _uiState.value.streamBadge
                                    viewModelScope.launch(Dispatchers.IO) {
                                        saveActiveTrackToPrefs(trackWithDuration, badgeSnapshot)
                                    }
                                }
                                if (lyricsFetchedForTrackId != activeId) {
                                    fetchLyricsInBackground(trackWithDuration)
                                }
                            }
                            if (playbackState == Player.STATE_ENDED &&
                                ctrl.repeatMode != Player.REPEAT_MODE_ONE
                            ) {
                                onTrackEnded(playbackGeneration.current)
                            }
                        }
                    }

                    override fun onRepeatModeChanged(repeatMode: Int) {
                        if (_uiState.value.repeatMode != repeatMode) {
                            _uiState.update { it.copy(repeatMode = repeatMode) }
                        }
                        if (preferences.state.value.repeatMode != repeatMode) {
                            preferences.update { it.copy(repeatMode = repeatMode) }
                        }
                    }

                    override fun onPositionDiscontinuity(
                        oldPosition: Player.PositionInfo,
                        newPosition: Player.PositionInfo,
                        reason: Int,
                    ) {
                        if (reason == Player.DISCONTINUITY_REASON_SEEK) {
                            val activeId = _uiState.value.activeTrack?.id
                            if (activeId != null && ctrl.currentMediaItem?.mediaId == activeId) {
                                playbackClock.onSeek(newPosition.positionMs.coerceAtLeast(0L))
                                discordSeekSignal.value += 1L
                            }
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        val active = _uiState.value.activeTrack ?: return
                        val generation = playbackGeneration.current
                        if (downloadManager.localUriFor(active.id) != null) {
                            PlaybackAudioBridge.clearEndedTransitionSuppression()
                            _uiState.update {
                                it.copy(
                                    isPlaying = false,
                                    isBuffering = false,
                                    status = uiStrings().offlinePlaybackError(error.message.orEmpty()),
                                )
                            }
                            return
                        }
                        val attempt = recoveryBudget.consumeAttempt(generation)
                        if (attempt == null) {
                            PlaybackAudioBridge.clearEndedTransitionSuppression()
                            _uiState.update {
                                it.copy(
                                    isPlaying = false,
                                    isBuffering = false,
                                    status = uiStrings().playbackStoppedStatus(error.message.orEmpty()),
                                )
                            }
                            return
                        }
                        val resumePositionMs = ctrl.currentPosition.coerceAtLeast(0L)
                        val resumePlayIntent = ctrl.isPlaying || ctrl.playWhenReady || _uiState.value.isPlaying
                        val oldBadge = _uiState.value.streamBadge
                        Log.w(
                            "SonataStream",
                            "Recovering track=${active.id} gen=$generation attempt=$attempt posMs=$resumePositionMs cause=${error.errorCodeName}"
                        )
                        recoveryJob?.cancel()
                        upgradeJob?.cancel()
                        recoveryJob = viewModelScope.launch(Dispatchers.IO) {
                            val engine = requireEngine()
                            streamCache.remove(active.id)
                            if (oldBadge.isNotBlank()) {
                                PlaybackService.evictCacheKey("${active.id}:$oldBadge")
                            }
                            _uiState.update {
                                if (isGenerationActive(generation, active.id)) {
                                    it.copy(isBuffering = true, status = uiStrings().recoveringStreamStatus)
                                } else it
                            }
                            runSuspendCatching { engine.resolveRecovery(active) }
                                .onSuccess { recovered ->
                                    if (!isActive || !isGenerationActive(generation, active.id)) return@onSuccess
                                    val swap = withContext(Dispatchers.Main) {
                                        runCatching {
                                            performAtomicStreamSwap(
                                                player = controllerSwapAdapter(ctrl),
                                                track = active,
                                                stream = recovered,
                                                overridePositionMs = resumePositionMs,
                                                overridePlayIntent = resumePlayIntent,
                                                isStillCurrent = { isActive && isGenerationActive(generation, active.id) },
                                                onCommitted = { _, _ ->
                                                    putCachedStream(active.id, recovered)
                                                    _uiState.update {
                                                        it.copy(
                                                            streamBadge = recovered.badge,
                                                            status = uiStrings().playingTrackStatus(active.title),
                                                        )
                                                    }
                                                },
                                            )
                                        }.onFailure { swapErr ->
                                            Log.w("SonataStream", "Recovery swap failed: ${swapErr.message}")
                                        }.getOrNull()
                                    }
                                    if (swap?.applied == true) {
                                        saveActiveTrackToPrefs(active, recovered.badge)
                                    }
                                }
                                .onFailure { recErr ->
                                    if (!isActive || !isGenerationActive(generation, active.id)) return@onFailure
                                    PlaybackAudioBridge.clearEndedTransitionSuppression()
                                    Log.w("SonataStream", "Stream recovery failed: ${recErr.message}")
                                    _uiState.update {
                                        it.copy(
                                            isPlaying = false,
                                            isBuffering = false,
                                            status = uiStrings().playbackErrorStatus(recErr.message.orEmpty()),
                                        )
                                    }
                                }
                        }
                    }
                })
            }
        }, ContextCompat.getMainExecutor(ctx))
    }

    private fun startPositionPoller() {
        viewModelScope.launch {
            _uiState
                .map { it.isPlaying && !it.isBuffering && it.activeTrack != null }
                .distinctUntilChanged()
                .collectLatest { activelyPlaying ->
                    if (!activelyPlaying) return@collectLatest
                    while (isActive) {
                        val ctrl = controller
                        val activeId = _uiState.value.activeTrack?.id
                        if (ctrl != null &&
                            activeId != null &&
                            ctrl.currentMediaItem?.mediaId == activeId &&
                            ctrl.playbackState == Player.STATE_READY &&
                            ctrl.isPlaying
                        ) {
                            playbackClock.sync(
                                positionMs = ctrl.currentPosition.coerceAtLeast(0L),
                                durationMs = ctrl.duration.coerceAtLeast(0L),
                            )
                        }
                        delay(200L)
                    }
                }
        }
    }

    fun selectTab(tab: AndroidTab) {
        _uiState.update {
            it.copy(
                activeTab = tab,
                collectionBrowseId = null,
                collectionTitle = null,
                collectionCoverUrl = null,
                collectionSubtitle = null,
                collectionKindLabel = null,
                collectionYear = null,
                collectionTracks = emptyList(),
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                showLyrics = false
            )
        }
        when (tab) {
            AndroidTab.Home, AndroidTab.Explore -> refreshHomeAndExploreRateLimited(forceManual = false)
            AndroidTab.Radio -> refreshRadioRateLimited(forceManual = false)
            else -> Unit
        }
    }

    fun closeCollection() {
        _uiState.update {
            it.copy(
                collectionBrowseId = null,
                collectionTitle = null,
                collectionCoverUrl = null,
                collectionSubtitle = null,
                collectionKindLabel = null,
                collectionYear = null,
                collectionTracks = emptyList(),
                artistPage = null,
                artistLoading = false,
            )
        }
    }

    fun closeArtist() {
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
            )
        }
    }

    fun openLoginSheet() {
        _uiState.update { it.copy(showLoginSheet = true, showSettingsSheet = false) }
    }

    fun dismissLoginSheet() {
        _uiState.update { it.copy(showLoginSheet = false) }
    }

    fun openSettingsSheet() {
        _uiState.update { it.copy(showSettingsSheet = true) }
    }

    fun dismissSettingsSheet() {
        _uiState.update { it.copy(showSettingsSheet = false) }
    }

    fun openServersScreen(initialServerId: String? = null) {
        _uiState.update { it.copy(showServersSheet = true) }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val list = runSuspendCatching { engine.listServers() }.getOrDefault(emptyList())
            _uiState.update { it.copy(servers = list) }
            val target = initialServerId?.let { id -> list.firstOrNull { it.id == id } }
            if (target != null) {
                openServerBrowser(target)
            }
        }
    }

    fun dismissServersScreen() {
        serverBrowseJob?.cancel()
        serverTestJob?.cancel()
        serverSearchJob?.cancel()
        _serverBrowserState.value = ServerBrowserState()
        _uiState.update { it.copy(showServersSheet = false) }
    }

    private fun clearSelfHostedCaches() {
        streamCache.keys.filter { isSelfHostedId(it) }.forEach { streamCache.remove(it) }
        PlaybackAudioBridge.clearStreamHeaders()
    }

    fun clearServerTestResult() {
        _serverBrowserState.update {
            it.copy(testingConnection = false, testResultMessage = null, testResultIsError = false)
        }
    }

    fun testServerConnection(profile: AndroidServerProfile) {
        serverTestJob?.cancel()
        val s = uiStrings().servers
        _serverBrowserState.update {
            it.copy(testingConnection = true, testResultMessage = s.testingConnection, testResultIsError = false)
        }
        serverTestJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.testServerConnection(profile) }
                .onSuccess { detail ->
                    _serverBrowserState.update {
                        it.copy(
                            testingConnection = false,
                            testResultMessage = s.connectionOk(detail),
                            testResultIsError = false,
                        )
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(
                            testingConnection = false,
                            testResultMessage = s.connectionFailed(err.message.orEmpty()),
                            testResultIsError = true,
                        )
                    }
                }
        }
    }

    fun saveServer(profile: AndroidServerProfile, onComplete: () -> Unit = {}) {
        val s = uiStrings().servers
        _serverBrowserState.update { it.copy(saving = true, error = null) }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.upsertServer(profile) }
                .onSuccess { updated ->
                    clearSelfHostedCaches()
                    val activeId = _serverBrowserState.value.activeServer?.id
                    val refreshedActive = activeId?.let { id -> updated.firstOrNull { it.id == id } }
                    _serverBrowserState.update {
                        it.copy(
                            saving = false,
                            activeServer = refreshedActive ?: it.activeServer,
                            error = null,
                        )
                    }
                    _uiState.update {
                        it.copy(
                            servers = updated,
                            status = s.serverSaved(profile.name.trim().ifBlank { profile.baseUrl }),
                        )
                    }
                    withContext(Dispatchers.Main) { onComplete() }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(
                            saving = false,
                            error = s.connectionFailed(err.message.orEmpty()),
                        )
                    }
                }
        }
    }

    fun deleteServer(serverId: String) {
        val s = uiStrings().servers
        val removedName = _uiState.value.servers.firstOrNull { it.id == serverId }?.name ?: serverId
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.deleteServer(serverId) }
                .onSuccess { updated ->
                    clearSelfHostedCaches()
                    if (_serverBrowserState.value.activeServer?.id == serverId) {
                        _serverBrowserState.value = ServerBrowserState()
                    }
                    _uiState.update {
                        it.copy(
                            servers = updated,
                            status = s.serverDeleted(removedName),
                        )
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update { it.copy(error = err.message) }
                }
        }
    }

    fun setServerEnabled(serverId: String, enabled: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.setServerEnabled(serverId, enabled) }
                .onSuccess { updated ->
                    clearSelfHostedCaches()
                    val refreshedActive = _serverBrowserState.value.activeServer?.let { cur ->
                        if (cur.id == serverId) updated.firstOrNull { it.id == serverId } else cur
                    }
                    _serverBrowserState.update { it.copy(activeServer = refreshedActive) }
                    _uiState.update { it.copy(servers = updated) }
                }
        }
    }

    fun openServerBrowser(server: AndroidServerProfile) {
        serverBrowseJob?.cancel()
        serverSearchJob?.cancel()
        _serverBrowserState.value = ServerBrowserState(
            activeServer = server,
            loading = true,
            webdavPathStack = if (server.kind == AndroidServerKind.WEB_DAV) {
                listOf(null to uiStrings().servers.rootFolder)
            } else {
                emptyList()
            },
        )
        serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            if (server.kind == AndroidServerKind.WEB_DAV) {
                runSuspendCatching { engine.browseWebdavFolder(server.id, null) }
                    .onSuccess { listing ->
                        _serverBrowserState.update {
                            it.copy(
                                loading = false,
                                webdavFolders = listing.folders,
                                webdavTracks = listing.tracks,
                                error = null,
                            )
                        }
                    }
                    .onFailure { err ->
                        _serverBrowserState.update {
                            it.copy(loading = false, error = err.message)
                        }
                    }
                return@launch
            }

            coroutineScope {
                val libsDef = async { runSuspendCatching { engine.listServerLibraries(server.id) } }
                val playlistsDef = async { runSuspendCatching { engine.listServerPlaylists(server.id) } }
                val albumsDef = async { runSuspendCatching { engine.listServerAlbums(server.id, null, null) } }
                val artistsDef = async { runSuspendCatching { engine.listServerArtists(server.id, null) } }

                val libsRes = libsDef.await()
                val playlistsRes = playlistsDef.await()
                val albumsRes = albumsDef.await()
                val artistsRes = artistsDef.await()
                val firstErr = libsRes.exceptionOrNull()
                    ?: playlistsRes.exceptionOrNull()
                    ?: albumsRes.exceptionOrNull()
                    ?: artistsRes.exceptionOrNull()

                _serverBrowserState.update {
                    it.copy(
                        loading = false,
                        libraries = libsRes.getOrDefault(emptyList()),
                        playlists = playlistsRes.getOrDefault(emptyList()),
                        albums = albumsRes.getOrDefault(emptyList()),
                        artists = artistsRes.getOrDefault(emptyList()),
                        error = if (libsRes.isFailure && playlistsRes.isFailure && albumsRes.isFailure) {
                            firstErr?.message
                        } else {
                            null
                        },
                    )
                }
            }
        }
    }

    fun closeServerBrowser() {
        serverBrowseJob?.cancel()
        serverSearchJob?.cancel()
        _serverBrowserState.value = ServerBrowserState()
    }

    fun selectServerLibrary(libraryId: String?) {
        val server = _serverBrowserState.value.activeServer ?: return
        if (server.kind == AndroidServerKind.WEB_DAV) {
            openWebDavFolder(libraryId, libraryId ?: uiStrings().servers.rootFolder)
            return
        }
        serverBrowseJob?.cancel()
        _serverBrowserState.update {
            it.copy(
                selectedLibraryId = libraryId,
                selectedArtistName = null,
                artistAlbums = emptyList(),
                loading = true,
                error = null,
            )
        }
        serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            coroutineScope {
                val albumsDef = async {
                    runSuspendCatching { engine.listServerAlbums(server.id, libraryId, null) }
                }
                val artistsDef = async {
                    runSuspendCatching { engine.listServerArtists(server.id, libraryId) }
                }
                val albumsRes = albumsDef.await()
                val artistsRes = artistsDef.await()
                _serverBrowserState.update {
                    it.copy(
                        loading = false,
                        albums = albumsRes.getOrDefault(emptyList()),
                        artists = artistsRes.getOrDefault(emptyList()),
                        error = albumsRes.exceptionOrNull()?.message,
                    )
                }
            }
        }
    }

    fun openServerArtist(artist: AndroidHomeCard) {
        val server = _serverBrowserState.value.activeServer ?: return
        serverBrowseJob?.cancel()
        _serverBrowserState.update {
            it.copy(
                selectedArtistName = artist.title,
                artistAlbums = emptyList(),
                loading = true,
                error = null,
            )
        }
        serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.listServerAlbums(server.id, null, artist.id) }
                .onSuccess { albums ->
                    _serverBrowserState.update {
                        it.copy(loading = false, artistAlbums = albums, error = null)
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(loading = false, error = err.message)
                    }
                }
        }
    }

    fun closeServerArtist() {
        _serverBrowserState.update {
            it.copy(selectedArtistName = null, artistAlbums = emptyList())
        }
    }

    fun openWebDavFolder(folderIdOrPath: String?, label: String) {
        val server = _serverBrowserState.value.activeServer ?: return
        serverBrowseJob?.cancel()
        val updatedStack = if (folderIdOrPath == null) {
            listOf(null to uiStrings().servers.rootFolder)
        } else {
            _serverBrowserState.value.webdavPathStack + (folderIdOrPath to label)
        }
        _serverBrowserState.update {
            it.copy(
                webdavPathStack = updatedStack,
                loading = true,
                error = null,
            )
        }
        serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.browseWebdavFolder(server.id, folderIdOrPath) }
                .onSuccess { listing ->
                    _serverBrowserState.update {
                        it.copy(
                            loading = false,
                            webdavFolders = listing.folders,
                            webdavTracks = listing.tracks,
                            error = null,
                        )
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(loading = false, error = err.message)
                    }
                }
        }
    }

    fun navigateWebDavBack(): Boolean {
        val stack = _serverBrowserState.value.webdavPathStack
        if (stack.size <= 1) return false
        val server = _serverBrowserState.value.activeServer ?: return false
        val trimmedStack = stack.dropLast(1)
        val targetPath = trimmedStack.lastOrNull()?.first
        serverBrowseJob?.cancel()
        _serverBrowserState.update {
            it.copy(
                webdavPathStack = trimmedStack,
                loading = true,
                error = null,
            )
        }
        serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.browseWebdavFolder(server.id, targetPath) }
                .onSuccess { listing ->
                    _serverBrowserState.update {
                        it.copy(
                            loading = false,
                            webdavFolders = listing.folders,
                            webdavTracks = listing.tracks,
                            error = null,
                        )
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(loading = false, error = err.message)
                    }
                }
        }
        return true
    }

    fun searchInsideServer(query: String) {
        val server = _serverBrowserState.value.activeServer ?: return
        val clean = query.trim()
        serverSearchJob?.cancel()
        if (clean.isEmpty()) {
            _serverBrowserState.update {
                it.copy(serverSearchQuery = "", serverSearchResult = null)
            }
            return
        }
        _serverBrowserState.update { it.copy(serverSearchQuery = query, loading = true, error = null) }
        serverSearchJob = viewModelScope.launch(Dispatchers.IO) {
            delay(250L)
            val engine = requireEngine()
            runSuspendCatching { engine.searchServer(server.id, clean, 40u) }
                .onSuccess { res ->
                    _serverBrowserState.update {
                        it.copy(loading = false, serverSearchResult = res, error = null)
                    }
                }
                .onFailure { err ->
                    _serverBrowserState.update {
                        it.copy(loading = false, error = err.message)
                    }
                }
        }
    }

    fun openServerAlbum(card: AndroidHomeCard) {
        openServerAlbumById(
            albumId = card.id,
            albumTitle = card.title,
            artistName = card.subtitle,
            coverUrl = card.thumbnailUrl,
        )
    }

    private fun openServerAlbumById(
        albumId: String,
        albumTitle: String,
        artistName: String = "",
        coverUrl: String? = null,
        year: String? = null,
    ) {
        val s = uiStrings()
        collectionJob?.cancel()
        collectionJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionBrowseId = albumId,
                    collectionTitle = albumTitle,
                    collectionCoverUrl = coverUrl,
                    collectionSubtitle = artistName.takeIf { sub -> sub.isNotBlank() },
                    collectionKindLabel = s.albumLabel,
                    collectionYear = year,
                    collectionTracks = emptyList(),
                    loading = true,
                    status = s.loadingNamedStatus(albumTitle),
                )
            }
            runSuspendCatching { engine.getServerAlbum(albumId) }
                .onSuccess { detail ->
                    val resolvedCover = detail.coverUrl ?: coverUrl
                    val resolvedTitle = detail.title.ifBlank { albumTitle }
                    val resolvedSubtitle = detail.subtitle.ifBlank { artistName }.takeIf { it.isNotBlank() }
                    _uiState.update { state ->
                        state.copy(
                            loading = false,
                            collectionBrowseId = detail.id,
                            collectionTitle = resolvedTitle,
                            collectionCoverUrl = resolvedCover,
                            collectionSubtitle = resolvedSubtitle,
                            collectionYear = detail.year ?: year,
                            collectionTracks = detail.tracks,
                            status = s.tracksInCollectionStatus(detail.tracks.size, resolvedTitle),
                        )
                    }
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = s.collectionErrorStatus(err.message.orEmpty()))
                    }
                }
        }
    }

    fun updateSettings(transform: (SonataSettingsState) -> SonataSettingsState) {
        preferences.update(transform)
        applyAudioSettings(preferences.state.value)
    }

    fun togglePreferHighQuality() {
        updateSettings { it.copy(preferHighQuality = !it.preferHighQuality) }
    }

    fun toggleAnimatedArtwork() {
        updateSettings { it.copy(animatedArtwork = !it.animatedArtwork) }
    }

    fun toggleCrossfade() {
        updateSettings { it.copy(crossfadeEnabled = !it.crossfadeEnabled) }
    }

    fun toggleLiquidGlass() {
        updateSettings { it.copy(liquidGlassEnabled = !it.liquidGlassEnabled) }
    }

    fun setFpsLimit(fps: Int) {
        val resolved = if (fps >= 120 && _uiState.value.supports120Hz) 120 else 60
        updateSettings { it.copy(fpsLimit = resolved) }
        viewModelScope.launch(Dispatchers.IO) {
            prefs.edit().putInt("fps_limit", resolved).apply()
        }
    }

    fun queryDownloadStorageStats(): SonataDownloadManager.DownloadStorageStats =
        downloadManager.queryStorageStats(settings.value.downloadStorageLocation)

    private suspend fun resolveBestDownloadStream(
        engine: SonataEngineGateway,
        track: AndroidTrack,
    ): AndroidResolvedStream? {
        val currentSettings = settings.value
        val quality = currentSettings.downloadQuality
        val wantsHighOrHighest =
            quality == DownloadQualityTier.HIGHEST || quality == DownloadQualityTier.HIGH

        var stream = if (track.id.startsWith("jiosaavn:")) {
            runSuspendCatching { engine.resolveJiosaavnById(track.id) }.getOrNull()?.let { js ->
                AndroidResolvedStream(
                    url = js.url,
                    badge = js.badge,
                    headers = emptyList(),
                )
            } ?: return null
        } else {
            getCachedStream(track.id) ?: runSuspendCatching { engine.resolveStream(track) }
                .onSuccess { putCachedStream(track.id, it) }
                .getOrNull() ?: return null
        }

        if (!track.id.startsWith("jiosaavn:") && !isSelfHostedId(track.id) && wantsHighOrHighest) {
            if (shouldUpgradeStreamBadge(stream.badge, _uiState.value.signedIn)) {
                runSuspendCatching { engine.resolveHifiUpgrade(track, stream) }
                    .getOrNull()
                    ?.let { upgraded ->
                        stream = upgraded
                        putCachedStream(track.id, upgraded)
                    }
            }
            if (currentSettings.downloadIncludeJioSaavn) {
                val curKbps = parseBitrateKbps(stream.badge).coerceAtLeast(0)
                if (quality == DownloadQualityTier.HIGHEST || curKbps < 256) {
                    runSuspendCatching {
                        engine.findJiosaavnUpgrade(track, curKbps.toUInt())
                    }.getOrNull()?.let { js ->
                        val jsKbps = js.bitrateKbps.toInt()
                        if (jsKbps > curKbps) {
                            stream = AndroidResolvedStream(
                                url = js.url,
                                badge = js.badge,
                                headers = emptyList(),
                            )
                        }
                    }
                }
            }
        }
        return stream
    }

    fun downloadTrack(track: AndroidTrack, collectionId: String? = null) {
        viewModelScope.launch(Dispatchers.IO) {
            val s = uiStrings()
            val wifiOnly = settings.value.wifiOnlyDownloads
            val wantsCompanionVideo = settings.value.downloadVideosIfAvailable &&
                !track.id.startsWith("local:") &&
                downloadManager.localVideoFor(track.id) == null
            if (downloadManager.isDownloaded(track.id)) {
                if (!wantsCompanionVideo) {
                    _uiState.update {
                        it.copy(
                            status = s.alreadyDownloadedNamed(track.title),
                            addToPlaylistStatus = s.alreadyDownloadedShort,
                        )
                    }
                    return@launch
                }
                if (wifiOnly && downloadManager.isOnMeteredNetwork()) {
                    val blocked = s.wifiOnlyDownloadBlocked
                    _uiState.update {
                        it.copy(
                            status = blocked,
                            addToPlaylistStatus = blocked,
                        )
                    }
                    return@launch
                }
                _uiState.update {
                    it.copy(
                        status = s.downloadingVideoStatus,
                        addToPlaylistStatus = s.downloadingVideoStatus,
                    )
                }
                val videoOk = downloadCompanionMusicVideoIfEnabled(requireEngine(), track, wifiOnly)
                val statusMsg = if (videoOk) s.downloadedNamed(track.title) else s.alreadyDownloadedNamed(track.title)
                val shortMsg = if (videoOk) statusMsg else s.alreadyDownloadedShort
                _uiState.update {
                    it.copy(
                        status = statusMsg,
                        addToPlaylistStatus = shortMsg,
                    )
                }
                return@launch
            }
            if (wifiOnly && downloadManager.isOnMeteredNetwork()) {
                val blocked = s.wifiOnlyDownloadBlocked
                _uiState.update {
                    it.copy(
                        status = blocked,
                        addToPlaylistStatus = blocked,
                    )
                }
                return@launch
            }
            val downloadingMsg = s.downloadingNamed(track.title)
            _uiState.update {
                it.copy(
                    status = downloadingMsg,
                    addToPlaylistStatus = downloadingMsg,
                )
            }
            val engine = requireEngine()
            val stream = resolveBestDownloadStream(engine, track) ?: run {
                _uiState.update {
                    it.copy(
                        status = s.downloadFailedStreamUnavailable,
                        addToPlaylistStatus = s.downloadFailedShort,
                    )
                }
                return@launch
            }
            val cleanUrl = stripRangeQueryParam(stream.url)
            val exportExternal = settings.value.usesExternalDownloadStorage()
            val input = DownloadedTrackInput(
                videoId = track.id,
                title = track.title,
                artist = track.artist,
                album = track.album.ifBlank { null },
                durationSecs = track.durationSecs?.toLong() ?: 0L,
                coverUrl = resolvedDownloadCoverUrl(track),
                codec = stream.badge.ifBlank { null },
            )
            var result = downloadManager.downloadTrack(
                track = input,
                streamUrl = cleanUrl,
                exportToMusic = exportExternal,
                collectionId = collectionId,
                wifiOnly = wifiOnly,
                headers = stream.headers.associate { it.name to it.value },
            )
            if (result.isFailure &&
                result.exceptionOrNull() !is SonataDownloadManager.WifiOnlyNetworkException &&
                !track.id.startsWith("jiosaavn:") &&
                !isSelfHostedId(track.id)
            ) {
                val recovered = runSuspendCatching { engine.resolveRecovery(track) }.getOrNull()
                if (recovered != null) {
                    putCachedStream(track.id, recovered)
                    result = downloadManager.downloadTrack(
                        track = input.copy(codec = recovered.badge.ifBlank { input.codec }),
                        streamUrl = stripRangeQueryParam(recovered.url),
                        exportToMusic = exportExternal,
                        collectionId = collectionId,
                        wifiOnly = wifiOnly,
                        headers = recovered.headers.associate { it.name to it.value },
                    )
                }
            }
            result.onSuccess {
                if (settings.value.downloadVideosIfAvailable && !track.id.startsWith("local:") && !isSelfHostedId(track.id)) {
                    _uiState.update {
                        it.copy(
                            status = s.downloadingVideoStatus,
                            addToPlaylistStatus = s.downloadingVideoStatus,
                        )
                    }
                    downloadCompanionMusicVideoIfEnabled(engine, track, wifiOnly)
                }
                val doneMsg = s.downloadedNamed(track.title)
                _uiState.update {
                    it.copy(
                        status = doneMsg,
                        addToPlaylistStatus = doneMsg,
                    )
                }
            }.onFailure { err ->
                Log.w("SonataDownload", "downloadTrack failed id=${track.id}: ${err.message}")
                val failStatus = if (err is SonataDownloadManager.WifiOnlyNetworkException) {
                    s.wifiOnlyDownloadBlocked
                } else {
                    s.downloadFailedNamed(err.message.orEmpty())
                }
                val failShort = if (err is SonataDownloadManager.WifiOnlyNetworkException) {
                    s.wifiOnlyDownloadBlocked
                } else {
                    s.downloadFailedShort
                }
                _uiState.update {
                    it.copy(
                        status = failStatus,
                        addToPlaylistStatus = failShort,
                    )
                }
            }
        }
    }

    fun downloadCollection(tracks: List<AndroidTrack>, collectionId: String) {
        if (tracks.isEmpty()) return
        viewModelScope.launch(Dispatchers.IO) {
            val s = uiStrings()
            val unique = tracks.distinctBy { it.id }
            val wantsVideos = settings.value.downloadVideosIfAvailable
            val pending = unique.filter { track ->
                !downloadManager.isDownloaded(track.id) ||
                    (wantsVideos && !track.id.startsWith("local:") && downloadManager.localVideoFor(track.id) == null)
            }
            if (pending.isEmpty()) {
                _uiState.update {
                    it.copy(
                        status = s.allTracksAlreadyDownloaded(unique.size),
                        addToPlaylistStatus = s.allTracksAlreadyDownloadedShort,
                    )
                }
                return@launch
            }
            val wifiOnly = settings.value.wifiOnlyDownloads
            if (wifiOnly && downloadManager.isOnMeteredNetwork()) {
                val blocked = s.wifiOnlyDownloadBlocked
                _uiState.update {
                    it.copy(
                        status = blocked,
                        addToPlaylistStatus = blocked,
                    )
                }
                return@launch
            }
            val batchMsg = s.downloadingTracksCount(pending.size)
            _uiState.update {
                it.copy(
                    status = batchMsg,
                    addToPlaylistStatus = batchMsg,
                )
            }
            val engine = requireEngine()
            val exportExternal = settings.value.usesExternalDownloadStorage()
            var savedCount = 0
            var blockedByWifi = false
            for (track in pending) {
                if (!isActive) break
                val alreadyHasAudio = downloadManager.isDownloaded(track.id)
                var audioReady = alreadyHasAudio
                if (!alreadyHasAudio) {
                    val stream = resolveBestDownloadStream(engine, track) ?: continue
                    val input = DownloadedTrackInput(
                        videoId = track.id,
                        title = track.title,
                        artist = track.artist,
                        album = track.album.ifBlank { null },
                        durationSecs = track.durationSecs?.toLong() ?: 0L,
                        coverUrl = resolvedDownloadCoverUrl(track),
                        codec = stream.badge.ifBlank { null },
                    )
                    var res = downloadManager.downloadTrack(
                        track = input,
                        streamUrl = stripRangeQueryParam(stream.url),
                        exportToMusic = exportExternal,
                        collectionId = collectionId,
                        wifiOnly = wifiOnly,
                        headers = stream.headers.associate { it.name to it.value },
                    )
                    if (res.exceptionOrNull() is SonataDownloadManager.WifiOnlyNetworkException) {
                        blockedByWifi = true
                        break
                    }
                    if (res.isFailure && !track.id.startsWith("jiosaavn:") && !isSelfHostedId(track.id)) {
                        val recovered = runSuspendCatching { engine.resolveRecovery(track) }.getOrNull()
                        if (recovered != null) {
                            putCachedStream(track.id, recovered)
                            res = downloadManager.downloadTrack(
                                track = input.copy(codec = recovered.badge.ifBlank { input.codec }),
                                streamUrl = stripRangeQueryParam(recovered.url),
                                exportToMusic = exportExternal,
                                collectionId = collectionId,
                                wifiOnly = wifiOnly,
                                headers = recovered.headers.associate { it.name to it.value },
                            )
                        }
                    }
                    if (res.exceptionOrNull() is SonataDownloadManager.WifiOnlyNetworkException) {
                        blockedByWifi = true
                        break
                    }
                    if (res.isSuccess) {
                        audioReady = true
                        savedCount++
                    } else {
                        Log.w("SonataDownload", "downloadCollection track failed id=${track.id}: ${res.exceptionOrNull()?.message}")
                    }
                }
                if (audioReady && wantsVideos && !track.id.startsWith("local:") && !isSelfHostedId(track.id) &&
                    downloadManager.localVideoFor(track.id) == null
                ) {
                    val videoOk = downloadCompanionMusicVideoIfEnabled(engine, track, wifiOnly)
                    if (alreadyHasAudio && videoOk) {
                        savedCount++
                    }
                }
            }
            val msg = when {
                blockedByWifi -> s.wifiOnlyDownloadBlocked
                savedCount > 0 -> s.downloadedCountOfTotal(savedCount, pending.size)
                else -> s.downloadFailedShort
            }
            _uiState.update { it.copy(status = msg, addToPlaylistStatus = msg) }
        }
    }

    private suspend fun downloadCompanionMusicVideoIfEnabled(
        engine: SonataEngineGateway,
        track: AndroidTrack,
        wifiOnly: Boolean,
    ): Boolean {
        if (!settings.value.downloadVideosIfAvailable || track.id.isBlank() || track.id.startsWith("local:")) {
            return false
        }
        if (downloadManager.localVideoFor(track.id) != null) return true
        return try {
            val cachedCandidate = musicVideoCandidateCache[track.id]
                ?.takeIf { SystemClock.elapsedRealtime() - it.cachedAtMs < ARTIST_CACHE_TTL_MS }
                ?.candidate
            val candidate = cachedCandidate
                ?: runSuspendCatching { engine.resolveMusicVideoCandidate(track) }
                    .getOrNull()
                    ?.also { fetched ->
                        musicVideoCandidateCache[track.id] = CachedVideoCandidate(
                            candidate = fetched,
                            cachedAtMs = SystemClock.elapsedRealtime(),
                        )
                    }
                ?: return false

            val resolved = resolveOrFetchMusicVideo(candidate.videoId) ?: return false
            if (resolved.qualities.isEmpty()) return false
            val targetPref = settings.value.videoDownloadQuality.toVideoQualityPreference()
            val selected = selectVideoQualityStream(
                qualities = resolved.qualities,
                preference = targetPref,
            ) ?: return false

            val input = DownloadedTrackInput(
                videoId = track.id,
                title = track.title,
                artist = track.artist,
                album = track.album.ifBlank { null },
                durationSecs = track.durationSecs?.toLong() ?: 0L,
                coverUrl = resolvedDownloadCoverUrl(track),
                codec = resolved.audioBadge.ifBlank { null },
            )
            val result = downloadManager.downloadCompanionVideo(
                track = input,
                videoUrl = selected.videoUrl,
                videoMimeHint = selected.mimeType.ifBlank { null },
                audioUrl = if (selected.isMuxed) null else resolved.audioUrl,
                qualityLabel = selected.qualityLabel.ifBlank { "${selected.height}p" },
                qualityHeight = selected.height.toInt(),
                syncSegments = candidate.syncSegments,
                wifiOnly = wifiOnly,
            )
            result.isSuccess
        } catch (ce: CancellationException) {
            throw ce
        } catch (t: Throwable) {
            Log.w("SonataDownload", "Companion video download skipped id=${track.id}: ${t.message}")
            false
        }
    }

    fun cancelDownload(videoId: String) {
        downloadManager.cancelDownload(videoId)
    }

    fun removeDownload(videoId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            downloadManager.removeDownload(videoId)
        }
    }

    fun clearAllDownloads() {
        viewModelScope.launch(Dispatchers.IO) {
            downloadManager.clearAllDownloads()
            _uiState.update { it.copy(status = uiStrings().downloadsClearedStatus) }
        }
    }

    fun playDownloadedTrack(record: DownloadedTrackRecord) {
        val queue = downloads.value.map { it.toAndroidTrack() }
        val target = queue.firstOrNull { it.id == record.videoId } ?: record.toAndroidTrack()
        playTrack(target, queue.ifEmpty { listOf(target) })
    }

    private fun DownloadedTrackRecord.toAndroidTrack(): AndroidTrack = AndroidTrack(
        id = videoId,
        title = title,
        artist = artist,
        album = album.orEmpty(),
        durationSecs = durationSecs.takeIf { it > 0L }?.toULong(),
        thumbnailUrl = coverUrl,
        liked = videoId in _uiState.value.likedTrackIds,
    )

    fun startQuickRadio(seedTrack: AndroidTrack? = uiState.value.activeTrack) {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val s = uiStrings()
            _uiState.update { it.copy(status = s.startingRadio) }
            val preferredArtists = buildPreferredArtists()

            if (seedTrack != null) {
                val radioTracks = runSuspendCatching {
                    engine.fetchEndlessRadioQueue(
                        seedVideoIds = listOf(seedTrack.id),
                        playlistId = null,
                        excludeVideoIds = listOf(seedTrack.id),
                        preferredArtists = preferredArtists,
                    )
                }.getOrDefault(emptyList()).ifEmpty {
                    runSuspendCatching { engine.fetchRadioQueue(seedTrack.id) }
                        .getOrDefault(emptyList())
                }
                val radioQueue = (listOf(seedTrack) + radioTracks).distinctBy { it.id }
                if (radioQueue.size > 1) {
                    playTrack(radioQueue.first(), radioQueue)
                    return@launch
                }
                val query = listOf(seedTrack.artist, seedTrack.title, "mix")
                    .filter { it.isNotBlank() }
                    .joinToString(" ")
                val results = runSuspendCatching { engine.searchSongs(query) }.getOrDefault(emptyList())
                val fallbackQueue = (listOf(seedTrack) + results).distinctBy { it.id }
                if (fallbackQueue.isNotEmpty()) {
                    playTrack(fallbackQueue.first(), fallbackQueue)
                    return@launch
                }
            }

            val mixCard = _uiState.value.shelves
                .firstOrNull { shelf ->
                    val title = shelf.title.lowercase()
                    title.contains("mix") || title.contains("radio") || title.contains("station")
                }
                ?.cards
                ?.firstOrNull { it.kind == AndroidHomeCardKind.PLAYLIST }

            if (mixCard != null) {
                val mixTracks = runSuspendCatching { engine.fetchCollectionTracks(mixCard.id) }
                    .getOrDefault(emptyList())
                if (mixTracks.isNotEmpty()) {
                    playTrack(mixTracks.first(), mixTracks)
                    return@launch
                }
            }

            val fallbackTracks = _uiState.value.shelves
                .flatMap { it.tracks }
                .ifEmpty { runSuspendCatching { engine.searchSongs("top hits radio mix") }.getOrDefault(emptyList()) }
                .distinctBy { it.id }

            if (fallbackTracks.isNotEmpty()) {
                playTrack(fallbackTracks.first(), fallbackTracks)
            } else {
                _uiState.update { it.copy(status = s.radioUnavailable) }
            }
        }
    }

    fun startPersonalStation() {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val s = uiStrings()
            _uiState.update { it.copy(status = s.startingPersonalMix) }
            val state = _uiState.value
            val preferredArtists = buildPreferredArtists()
            val seedCandidates = (_lastPlayedTracks.value +
                state.shelves.flatMap { it.tracks }.filter { it.id in state.likedTrackIds })
                .distinctBy { it.id }
            val seeds = selectDistinctArtistIds(seedCandidates, maxCount = 3)
                .ifEmpty { listOfNotNull(state.activeTrack?.id) }
            if (seeds.isNotEmpty()) {
                val stationTracks = runSuspendCatching {
                    engine.fetchEndlessRadioQueue(
                        seedVideoIds = seeds,
                        playlistId = null,
                        excludeVideoIds = emptyList(),
                        preferredArtists = preferredArtists,
                    )
                }.getOrDefault(emptyList()).ifEmpty {
                    runSuspendCatching { engine.fetchRadioQueue(seeds.first()) }
                        .getOrDefault(emptyList())
                }
                if (stationTracks.isNotEmpty()) {
                    playTrack(stationTracks.first(), stationTracks)
                    return@launch
                }
            }
            val query = preferredArtists.firstOrNull()?.let { "$it radio mix" } ?: "my supermix"
            val fallback = runSuspendCatching { engine.searchSongs(query) }.getOrDefault(emptyList())
            if (fallback.isNotEmpty()) {
                playTrack(fallback.first(), fallback)
            } else {
                startQuickRadio(state.activeTrack)
            }
        }
    }

    fun startDiscoveryStation() {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val s = uiStrings()
            _uiState.update { it.copy(status = s.startingDiscoveryMix) }
            val state = _uiState.value
            val preferredArtists = buildPreferredArtists()
            val historyIds = _lastPlayedTracks.value.map { it.id }.toSet()
            val freshCandidates = state.shelves.flatMap { it.tracks }
                .filter { it.id.isNotBlank() && it.id !in historyIds && it.id !in state.likedTrackIds }
                .distinctBy { it.id }
            val seeds = selectDistinctArtistIds(freshCandidates, maxCount = 3)
                .ifEmpty { selectDistinctArtistIds(_lastPlayedTracks.value, maxCount = 2) }
                .ifEmpty { listOfNotNull(state.activeTrack?.id) }
            val excludeIds = (historyIds + state.likedTrackIds).take(120)
            if (seeds.isNotEmpty()) {
                val discoveryTracks = runSuspendCatching {
                    engine.fetchEndlessRadioQueue(
                        seedVideoIds = seeds,
                        playlistId = null,
                        excludeVideoIds = excludeIds,
                        preferredArtists = preferredArtists,
                    )
                }.getOrDefault(emptyList()).ifEmpty {
                    runSuspendCatching { engine.fetchRadioQueue(seeds.first()) }
                        .getOrDefault(emptyList())
                }
                if (discoveryTracks.isNotEmpty()) {
                    playTrack(discoveryTracks.first(), discoveryTracks)
                    return@launch
                }
            }
            val fallback = runSuspendCatching { engine.searchSongs("new music discovery mix") }
                .getOrDefault(emptyList())
            if (fallback.isNotEmpty()) {
                playTrack(fallback.first(), fallback)
            } else {
                startQuickRadio(state.activeTrack)
            }
        }
    }

    fun setSleepTimer(minutes: Int?) {
        sleepTimerJob?.cancel()
        sleepTimerJob = null
        PlaybackAudioBridge.applySleepFadeMultiplier(1f)
        if (minutes == null) {
            sleepAtEndOfTrack = false
            _sleepTimerRemainingMinutes.value = null
            return
        }
        if (minutes <= 0) {
            sleepAtEndOfTrack = true
            _sleepTimerRemainingMinutes.value = 0
            return
        }
        sleepAtEndOfTrack = false
        _sleepTimerRemainingMinutes.value = minutes
        sleepTimerJob = viewModelScope.launch {
            try {
                var remaining = minutes
                while (remaining > 1 && isActive) {
                    delay(60_000L)
                    remaining -= 1
                    _sleepTimerRemainingMinutes.value = remaining
                }
                if (isActive) {
                    if (settings.value.sleepTimerFadeOut) {
                        delay(50_000L)
                        val steps = 20
                        for (step in 1..steps) {
                            if (!isActive) break
                            val vol = (1f - step.toFloat() / steps.toFloat()).coerceAtLeast(0.04f)
                            PlaybackAudioBridge.applySleepFadeMultiplier(vol)
                            delay(500L)
                        }
                    } else {
                        delay(60_000L)
                    }
                }
                if (isActive) {
                    withContext(Dispatchers.Main) {
                        controller?.pause()
                    }
                    _sleepTimerRemainingMinutes.value = null
                    _uiState.update { it.copy(status = uiStrings().sleepTimerPaused) }
                }
            } finally {
                withContext(NonCancellable) {
                    PlaybackAudioBridge.applySleepFadeMultiplier(1f)
                }
            }
        }
    }

    fun getTrackLyricOffsetMs(videoId: String): Long = preferences.getTrackLyricOffsetMs(videoId)

    fun setTrackLyricOffsetMs(videoId: String, offsetMs: Long) {
        preferences.setTrackLyricOffsetMs(videoId, offsetMs)
    }

    fun exportSettingsBackup(): String = preferences.exportBackupJson()

    fun importSettingsBackup(json: String): Boolean {
        val ok = preferences.importBackupJson(json)
        if (ok) {
            applyAudioSettings(preferences.state.value)
        }
        return ok
    }

    @Volatile
    private var requestedCanvasTrackId: String? = null

    private fun resolveCanvasForTrack(
        track: AndroidTrack,
        generation: Long = playbackGeneration.current,
    ) {
        canvasJob?.cancel()
        val trackId = track.id
        requestedCanvasTrackId = trackId
        _currentCanvas.value = null
        if (!settings.value.animatedArtwork) return
        if (!settings.value.animatedArtworkCellular) {
            val cm = getApplication<Application>().getSystemService(Context.CONNECTIVITY_SERVICE)
                as? android.net.ConnectivityManager
            if (cm?.isActiveNetworkMetered == true) return
        }
        canvasJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val resolved = runSuspendCatching {
                CanvasResolver.resolve(
                    videoId = trackId,
                    title = track.title,
                    artist = track.artist,
                    album = track.album.ifBlank { null },
                    gateway = engine,
                )
            }.getOrNull()
            if (isActive &&
                settings.value.animatedArtwork &&
                requestedCanvasTrackId == trackId &&
                isGenerationActive(generation, trackId)
            ) {
                _currentCanvas.value = resolved
            }
        }
    }

    fun toggleStarTrack(trackId: String) {
        val nowLiked = trackId !in _uiState.value.likedTrackIds
        _uiState.update { state ->
            val updated = if (nowLiked) {
                state.likedTrackIds + trackId
            } else {
                state.likedTrackIds - trackId
            }
            state.copy(likedTrackIds = updated)
        }
        val snapshotTrack = _uiState.value.let { state ->
            listOfNotNull(state.activeTrack).firstOrNull { it.id == trackId }
                ?: state.queue.firstOrNull { it.id == trackId }
                ?: state.collectionTracks.firstOrNull { it.id == trackId }
                ?: state.tracks.firstOrNull { it.id == trackId }
                ?: state.lastPlayedTracks.firstOrNull { it.id == trackId }
                ?: (state.shelves + state.exploreShelves + state.radioMixShelves + state.moodCategoryShelves)
                    .asSequence()
                    .flatMap { shelf -> shelf.tracks.asSequence() + shelf.cards.asSequence().mapNotNull { it.track } }
                    .firstOrNull { it.id == trackId }
        }
        if (nowLiked) {
            val likedCover = snapshotTrack?.let { resolveTrackCoverUrl(it) }
                ?: if (!trackId.startsWith("jiosaavn:") && !trackId.startsWith("local:") && trackId.isNotBlank()) {
                    "https://i.ytimg.com/vi/$trackId/hqdefault.jpg"
                } else {
                    null
                }
            updateLatestLikedCover(likedCover)
        }
        if (nowLiked &&
            settings.value.autoDownloadLiked &&
            snapshotTrack != null &&
            !snapshotTrack.id.startsWith("local:") &&
            !downloadManager.isDownloaded(snapshotTrack.id)
        ) {
            downloadTrack(snapshotTrack)
        }
        if (!trackId.startsWith("local:")) {
            viewModelScope.launch(Dispatchers.IO) {
                val engine = requireEngine()
                if (runCatching { engine.isSignedIn() }.getOrDefault(false)) {
                    val ytmId: String = (if (snapshotTrack != null) {
                        resolveYtmVideoIdForTrack(engine, snapshotTrack)
                    } else if (!trackId.startsWith("jiosaavn:")) {
                        trackId
                    } else {
                        null
                    }) ?: return@launch
                    runSuspendCatching { engine.setLikeStatus(ytmId, nowLiked) }
                        .onSuccess {
                            if (!nowLiked) {
                                runSuspendCatching { engine.fetchCollectionTracks("VLLM") }
                                    .onSuccess { likedTracks ->
                                        val nextCover = likedTracks.firstNotNullOfOrNull { resolveTrackCoverUrl(it) }
                                        updateLatestLikedCover(nextCover, clearIfNull = likedTracks.isEmpty())
                                    }
                            }
                        }
                        .onFailure { err ->
                            Log.w("SonataLibrary", "Like status sync failed for '$ytmId': ${err.message}")
                        }
                }
            }
        }
    }

    fun openAddToPlaylist(tracks: List<AndroidTrack>) {
        val unique = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
        if (unique.isEmpty()) return
        _uiState.update {
            it.copy(
                addToPlaylistTargets = unique,
                addToPlaylistStatus = null,
            )
        }
    }

    fun dismissAddToPlaylist() {
        _uiState.update {
            it.copy(
                addToPlaylistTargets = emptyList(),
                addToPlaylistStatus = null,
            )
        }
    }

    fun clearAddToPlaylistStatus() {
        _uiState.update {
            if (it.addToPlaylistStatus != null) it.copy(addToPlaylistStatus = null) else it
        }
    }

    fun addTracksToPlaylists(
        tracks: List<AndroidTrack>,
        playlistIds: List<String>,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) {
        val s = uiStrings()
        val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
        val cleanPlaylistIds = playlistIds.map { it.trim() }.filter { it.isNotBlank() }.distinct()
        if (cleanTracks.isEmpty() || cleanPlaylistIds.isEmpty()) {
            onComplete(false, s.selectAtLeastOnePlaylist)
            return
        }
        val videoIds = cleanTracks.map { it.id }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val resolvedVideoIds = if (signedIn) resolveYtmVideoIds(engine, cleanTracks) else videoIds
            _uiState.update { it.copy(addToPlaylistStatus = s.addingToPlaylistStatus) }

            var successCount = 0
            var firstError: String? = null

            val results = coroutineScope {
                cleanPlaylistIds.map { plId ->
                    async {
                        if (plId.startsWith(LOCAL_PLAYLIST_PREFIX) || !signedIn) {
                            val ok = if (plId.startsWith(LOCAL_PLAYLIST_PREFIX)) {
                                appendTracksToLocalPlaylist(plId, cleanTracks)
                            } else {
                                false
                            }
                            if (ok) Result.success(Unit) else Result.failure(IllegalStateException(s.signInToEditPlaylists))
                        } else if (resolvedVideoIds.isEmpty()) {
                            Result.failure(IllegalStateException(s.noYtmMatchFound))
                        } else {
                            runSuspendCatching { engine.addTracksToPlaylist(plId, resolvedVideoIds) }
                        }
                    }
                }.awaitAll()
            }

            results.forEach { res ->
                res.onSuccess {
                    successCount++
                }.onFailure { err ->
                    if (firstError == null) firstError = err.message ?: s.failedUpdatePlaylist
                }
            }

            val updatedRemote = if (signedIn && successCount > 0) {
                runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
            } else {
                null
            }
            val ok = successCount == cleanPlaylistIds.size
            val message = when {
                ok -> if (cleanTracks.size == 1) {
                    s.addedSingleTrackToPlaylists(cleanTracks.first().title, cleanPlaylistIds.size)
                } else {
                    s.addedTracksToPlaylistsCount(cleanTracks.size, cleanPlaylistIds.size)
                }
                successCount > 0 -> s.addedToSomePlaylists(successCount, cleanPlaylistIds.size)
                else -> firstError ?: s.couldNotAddToPlaylist
            }

            _uiState.update { state ->
                val nextPlaylists = if (updatedRemote != null) {
                    mergePlaylistsWithLocal(updatedRemote)
                } else {
                    mergePlaylistsWithLocal(state.playlists.filterNot { it.id.startsWith(LOCAL_PLAYLIST_PREFIX) })
                }
                state.copy(
                    playlists = nextPlaylists,
                    addToPlaylistTargets = if (ok) emptyList() else state.addToPlaylistTargets,
                    addToPlaylistStatus = message,
                    status = message,
                )
            }
            withContext(Dispatchers.Main) {
                onComplete(ok, message)
            }
        }
    }

    fun createPlaylistWithTracks(
        title: String,
        tracks: List<AndroidTrack>,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) {
        val s = uiStrings()
        val cleanTitle = title.trim()
        if (cleanTitle.isEmpty()) {
            onComplete(false, s.playlistNameEmpty)
            return
        }
        val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
        val videoIds = cleanTracks.map { it.id }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val resolvedVideoIds = if (signedIn) resolveYtmVideoIds(engine, cleanTracks) else videoIds
            _uiState.update { it.copy(addToPlaylistStatus = s.creatingPlaylistStatus(cleanTitle)) }

            val remoteResult = if (signedIn) {
                runSuspendCatching { engine.createPlaylist(cleanTitle, "", resolvedVideoIds) }
            } else {
                null
            }

            if (remoteResult != null && remoteResult.isSuccess) {
                val createdId = remoteResult.getOrNull().orEmpty()
                val updatedRemote = runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
                val createdThumb = cleanTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
                val message = s.createdPlaylistStatus(cleanTitle)
                _uiState.update { state ->
                    val base = updatedRemote ?: (listOf(AndroidPlaylist(createdId, cleanTitle, createdThumb, isOwned = true)) + state.playlists)
                    val ownedNorm = normalizeCollectionId(createdId)
                    val nextOwned = if (ownedNorm.startsWith("PL")) {
                        state.ownedPlaylistIds + ownedNorm
                    } else {
                        computeOwnedPlaylistIds(base, state.savedExternalPlaylistIds)
                    }
                    state.copy(
                        playlists = mergePlaylistsWithLocal(base),
                        ownedPlaylistIds = nextOwned,
                        addToPlaylistTargets = emptyList(),
                        addToPlaylistStatus = message,
                        status = message,
                    )
                }
                withContext(Dispatchers.Main) { onComplete(true, message) }
                return@launch
            }

            // Offline / signed-out fallback (or remote error fallback when signed out): save locally.
            if (!signedIn) {
                createLocalPlaylist(cleanTitle, cleanTracks)
                val message = s.createdOfflinePlaylistStatus(cleanTitle)
                _uiState.update { state ->
                    state.copy(
                        playlists = mergePlaylistsWithLocal(
                            state.playlists.filterNot { it.id.startsWith(LOCAL_PLAYLIST_PREFIX) }
                        ),
                        addToPlaylistTargets = emptyList(),
                        addToPlaylistStatus = message,
                        status = message,
                    )
                }
                withContext(Dispatchers.Main) { onComplete(true, message) }
            } else {
                val errMsg = remoteResult?.exceptionOrNull()?.message ?: s.failedCreatePlaylist
                _uiState.update {
                    it.copy(
                        addToPlaylistStatus = errMsg,
                        status = errMsg,
                    )
                }
                withContext(Dispatchers.Main) { onComplete(false, errMsg) }
            }
        }
    }

    /**
     * Renames a YouTube Music playlist, updates its optional description/privacy,
     * and/or uploads a custom cover image, syncing the changes to the user's account.
     */
    fun editPlaylist(
        playlistId: String,
        title: String?,
        description: String?,
        privacy: String?,
        coverBytes: ByteArray?,
        mimeType: String?,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) {
        val s = uiStrings()
        val cleanId = playlistId.trim()
        val cleanTitle = title?.trim()?.takeIf { it.isNotEmpty() }
        val cleanDesc = description?.trim()?.takeIf { it.isNotEmpty() }
        val cleanPrivacy = privacy?.trim()?.takeIf { it.isNotEmpty() }
        val hasMetadata = cleanTitle != null || cleanDesc != null || cleanPrivacy != null
        val hasCover = coverBytes != null && coverBytes.isNotEmpty()
        if (cleanId.isEmpty() || (!hasMetadata && !hasCover)) {
            onComplete(false, s.nothingToUpdate)
            return
        }

        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) {
                val msg = s.signInToEditPlaylists
                withContext(Dispatchers.Main) { onComplete(false, msg) }
                return@launch
            }

            if (hasMetadata) {
                val metaRes = runSuspendCatching {
                    engine.editPlaylistMetadata(cleanId, cleanTitle, cleanDesc, cleanPrivacy)
                }
                if (metaRes.isFailure) {
                    val err = metaRes.exceptionOrNull()?.message ?: s.failedUpdatePlaylist
                    _uiState.update { it.copy(status = err) }
                    withContext(Dispatchers.Main) { onComplete(false, err) }
                    return@launch
                }
            }

            if (hasCover) {
                val bytes = coverBytes!!
                val mime = mimeType?.takeIf { it.isNotBlank() } ?: "image/jpeg"
                val coverRes = runSuspendCatching {
                    engine.uploadPlaylistCover(cleanId, bytes, mime)
                }
                if (coverRes.isFailure) {
                    val err = coverRes.exceptionOrNull()?.message ?: s.failedUploadCover
                    _uiState.update { it.copy(status = err) }
                    withContext(Dispatchers.Main) { onComplete(false, err) }
                    return@launch
                }
            }

            val refreshedRemote = runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
            val normalizedId = cleanId.removePrefix("VL")
            val matchedRemote = refreshedRemote?.firstOrNull {
                it.id == cleanId || it.id.removePrefix("VL") == normalizedId
            }

            val updatedTitle = cleanTitle ?: matchedRemote?.name
            val statusMsg = if (hasCover && cleanTitle != null) {
                s.updatedPlaylistAndCover
            } else if (hasCover) {
                s.updatedPlaylistCover
            } else {
                s.updatedPlaylist
            }

            _uiState.update { state ->
                val isCurrentDetail = state.collectionBrowseId?.let {
                    it == cleanId || it.removePrefix("VL") == normalizedId
                } == true
                val basePlaylists = refreshedRemote ?: state.playlists.map { pl ->
                    if (pl.id == cleanId || pl.id.removePrefix("VL") == normalizedId) {
                        pl.copy(name = updatedTitle ?: pl.name)
                    } else {
                        pl
                    }
                }
                state.copy(
                    playlists = mergePlaylistsWithLocal(basePlaylists),
                    collectionTitle = if (isCurrentDetail && updatedTitle != null) updatedTitle else state.collectionTitle,
                    collectionCoverUrl = if (isCurrentDetail && matchedRemote?.thumbnailUrl != null) {
                        matchedRemote.thumbnailUrl
                    } else {
                        state.collectionCoverUrl
                    },
                    status = statusMsg,
                )
            }
            withContext(Dispatchers.Main) { onComplete(true, statusMsg) }
        }
    }

    @OptIn(UnstableApi::class)
    fun removeTrackFromCache(videoId: String) {
        if (videoId.isBlank()) return
        viewModelScope.launch(Dispatchers.IO) {
            val cached = streamCache.remove(videoId)
            PlaybackService.evictCacheKey(videoId)
            if (cached != null && cached.stream.badge.isNotBlank()) {
                PlaybackService.evictCacheKey("$videoId:${cached.stream.badge}")
            }
            val activeBadge = _uiState.value.takeIf { it.activeTrack?.id == videoId }?.streamBadge
            if (!activeBadge.isNullOrBlank()) {
                PlaybackService.evictCacheKey("$videoId:$activeBadge")
            }
            downloadManager.removeDownload(videoId)
            _uiState.update { it.copy(status = uiStrings().removedFromCacheStatus) }
        }
    }

    private suspend fun fetchTracksForCollectionAction(
        engine: SonataEngineGateway,
        browseId: String,
    ): List<AndroidTrack> {
        return when {
            browseId == DOWNLOADS_PLAYLIST_ID -> downloads.value.map { it.toAndroidTrack() }
            browseId.startsWith(LOCAL_PLAYLIST_PREFIX) -> getLocalPlaylistTracks(browseId)
            else -> runSuspendCatching { engine.fetchCollectionTracks(browseId) }.getOrDefault(emptyList())
        }
    }

    fun playCollectionShuffled(
        browseId: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
    ) {
        if (browseId.isBlank()) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val tracks = fetchTracksForCollectionAction(engine, browseId)
            if (tracks.isEmpty()) {
                _uiState.update { it.copy(status = uiStrings().noTracksInCollection(title)) }
                return@launch
            }
            val backfilled = backfillCollectionTracks(
                rawTracks = tracks,
                id = browseId,
                title = title.ifBlank { uiStrings().playlistLabel },
                subtitle = subtitle.takeIf { it.isNotBlank() },
                coverUrl = coverUrl,
            )
            toggleShuffleAndPlay(backfilled)
        }
    }

    fun enqueueCollectionNext(browseId: String) {
        if (browseId.isBlank()) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val tracks = fetchTracksForCollectionAction(engine, browseId)
            if (tracks.isEmpty()) return@launch
            val current = _uiState.value
            val existingQueue = current.queue
            if (existingQueue.isEmpty() || current.activeTrack == null) {
                playTrack(tracks.first(), tracks)
                return@launch
            }
            val insertIndex = (current.queueIndex + 1).coerceIn(0, existingQueue.size)
            val mutable = existingQueue.toMutableList()
            mutable.addAll(insertIndex, tracks)
            _uiState.update {
                it.copy(queue = mutable, status = uiStrings().addedToPlayNextStatus(tracks.size))
            }
        }
    }

    fun enqueueCollectionLater(browseId: String) {
        if (browseId.isBlank()) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val tracks = fetchTracksForCollectionAction(engine, browseId)
            if (tracks.isEmpty()) return@launch
            val current = _uiState.value
            val existingQueue = current.queue
            if (existingQueue.isEmpty() || current.activeTrack == null) {
                playTrack(tracks.first(), tracks)
                return@launch
            }
            _uiState.update {
                it.copy(
                    queue = existingQueue + tracks,
                    status = uiStrings().addedToQueueStatus(tracks.size),
                )
            }
        }
    }

    fun downloadCollectionById(browseId: String) {
        if (browseId.isBlank()) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val tracks = fetchTracksForCollectionAction(engine, browseId)
            if (tracks.isNotEmpty()) {
                downloadCollection(tracks, browseId)
            }
        }
    }

    fun playNext(track: AndroidTrack) {
        val current = _uiState.value
        val queue = current.queue
        if (queue.isEmpty() || current.activeTrack == null) {
            playTrack(track, listOf(track))
            return
        }
        val insertIndex = (current.queueIndex + 1).coerceIn(0, queue.size)
        val mutable = queue.toMutableList()
        mutable.add(insertIndex, track)
        _uiState.update { it.copy(queue = mutable, status = uiStrings().playingNextStatus(track.title)) }
    }

    fun addToQueue(track: AndroidTrack) {
        val current = _uiState.value
        val queue = current.queue
        if (queue.isEmpty() || current.activeTrack == null) {
            playTrack(track, listOf(track))
            return
        }
        _uiState.update {
            it.copy(queue = queue + track, status = uiStrings().addedTrackToQueueStatus(track.title))
        }
    }

    fun toggleShuffleAndPlay(tracks: List<AndroidTrack>) {
        if (tracks.isEmpty()) return
        preferences.update { it.copy(shuffleEnabled = true) }
        _uiState.update { it.copy(shuffleEnabled = true) }
        val start = tracks[Random.nextInt(tracks.size)]
        playTrack(start, tracks)
    }

    fun removeRecentSearch(q: String) {
        _uiState.update { state ->
            state.copy(recentSearches = state.recentSearches.filterNot { it == q })
        }
    }

    fun clearRecentSearches() {
        _uiState.update { it.copy(recentSearches = emptyList()) }
    }

    @OptIn(UnstableApi::class)
    fun clearMediaCache() {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            streamCache.clear()
            lyricsFetchedForTrackId = null
            runCatching {
                PlaybackService.clearMediaCache(getApplication())
            }
            runSuspendCatching {
                engine.clearLyricsCache()
            }
            _uiState.update { it.copy(status = uiStrings().mediaCacheClearedStatus) }
        }
    }

    fun clearImageCache() {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                val loader = coil3.SingletonImageLoader.get(getApplication())
                loader.memoryCache?.clear()
                loader.diskCache?.clear()
            }
            _uiState.update { it.copy(status = uiStrings().imageCacheClearedStatus) }
        }
    }

    fun completeLogin(cookieHeader: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val s = uiStrings()
            _uiState.update {
                it.copy(showLoginSheet = false, loading = true, status = s.signingInYtmStatus)
            }
            runSuspendCatching { engine.setYtmCookieHeader(cookieHeader) }
                .onSuccess { profile ->
                    streamCache.clear()
                    artistPageCache.clear()
                    jioToYtmIdCache.clear()
                    trackCollectionContext.clear()
                    resolvedArtistCardCache.clear()
                    lastHomeExploreFetchAtMs = 0L
                    lastRadioFetchAtMs = 0L
                    cipherWarmupScheduled = false
                    _uiState.update {
                        it.copy(
                            authInitialized = true,
                            signedIn = true,
                            accountProfile = profile,
                            status = profile?.let { p -> s.signedInAsStatus(p.name) } ?: s.signedInYtmStatus
                        )
                    }
                    refreshHomeAndExploreRateLimited(forceManual = true, showBlockingSpinner = _uiState.value.shelves.isEmpty())
                    refreshRadioRateLimited(forceManual = true)
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = s.signInFailedStatus(err.message.orEmpty()))
                    }
                }
        }
    }

    fun signOut() {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            engine.signOut()
            streamCache.clear()
            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = false,
                    accountProfile = null,
                    ownedPlaylistIds = emptySet(),
                    playlists = mergePlaylistsWithLocal(emptyList()),
                    status = uiStrings().signedOutStatus
                )
            }
            refreshInBackground(showBlockingSpinner = false)
        }
    }

    fun onQueryChange(newQuery: String) {
        _uiState.update { it.copy(query = newQuery) }
    }

    fun searchDebounced(newQuery: String) {
        _uiState.update { it.copy(query = newQuery) }
        searchDebounceJob?.cancel()
        val trimmed = newQuery.trim()
        if (trimmed.length < 2) return
        searchDebounceJob = viewModelScope.launch {
            delay(320L)
            search(trimmed)
        }
    }

    fun loadHome(forceRefresh: Boolean = false) {
        if (forceRefresh) {
            refreshJob?.cancel()
            lastHomeExploreFetchAtMs = 0L
            lastRadioFetchAtMs = 0L
            _uiState.update { it.copy(selectedHomeMoodParams = null) }
        }
        refreshHomeAndExploreRateLimited(
            forceManual = true,
            showBlockingSpinner = forceRefresh || _uiState.value.shelves.isEmpty(),
            forceBypassCooldown = forceRefresh,
        )
        refreshRadioRateLimited(forceManual = true)
    }

    fun search(overrideQuery: String? = null) {
        val q = (overrideQuery ?: _uiState.value.query).trim()
        if (q.isEmpty()) return
        speculativePrefetchJob?.cancel()
        searchJob?.cancel()
        searchJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val s = uiStrings()
            val includeJioSaavn = settings.value.jioSaavnEnabled
            val hasEnabledServers = _uiState.value.servers.any { it.enabled }
            _uiState.update { state ->
                val updatedHistory = (listOf(q) + state.recentSearches.filterNot { it.equals(q, ignoreCase = true) }).take(12)
                state.copy(
                    activeTab = AndroidTab.Search,
                    collectionBrowseId = null,
                    collectionTitle = null,
                    collectionCoverUrl = null,
                    collectionSubtitle = null,
                    collectionKindLabel = null,
                    collectionYear = null,
                    collectionTracks = emptyList(),
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    showLyrics = false,
                    query = q,
                    recentSearches = updatedHistory,
                    loading = true,
                    serverSearchLoading = hasEnabledServers,
                    serverSearchResult = if (hasEnabledServers) state.serverSearchResult else null,
                    status = s.searchingQueryStatus(q)
                )
            }
            val serverJob = if (hasEnabledServers) {
                launch {
                    val srvResult = withTimeoutOrNull(8_000L) {
                        runSuspendCatching { engine.searchAllServers(q, 20u) }.getOrNull()
                    }
                    if (!isActive) return@launch
                    _uiState.update { state ->
                        if (state.query == q) {
                            state.copy(serverSearchResult = srvResult, serverSearchLoading = false)
                        } else {
                            state
                        }
                    }
                }
            } else {
                null
            }
            val (ytResult, jioTracks) = coroutineScope {
                val ytDeferred = async { runSuspendCatching { engine.searchSongs(q) } }
                val jioDeferred = if (includeJioSaavn) {
                    async {
                        withTimeoutOrNull(4_000L) {
                            runSuspendCatching { engine.searchJiosaavn(q, 6u) }.getOrNull()
                        }.orEmpty()
                    }
                } else {
                    null
                }
                ytDeferred.await() to (jioDeferred?.await().orEmpty())
            }
            if (!isActive) {
                serverJob?.cancel()
                return@launch
            }
            ytResult
                .onSuccess { ytTracks ->
                    val merged = if (jioTracks.isEmpty()) {
                        ytTracks
                    } else {
                        val seenKeys = ytTracks.mapTo(HashSet()) {
                            "${it.title.trim().lowercase()}|${it.artist.trim().lowercase()}"
                        }
                        val extraJio = jioTracks.filter { js ->
                            seenKeys.add("${js.title.trim().lowercase()}|${js.artist.trim().lowercase()}")
                        }
                        ytTracks + extraJio
                    }
                    _uiState.update {
                        it.copy(
                            loading = false,
                            tracks = merged,
                            listTitle = s.searchResultsHeader(q),
                            status = if (includeJioSaavn && jioTracks.isNotEmpty()) {
                                s.songsFoundYtmJioCount(merged.size)
                            } else {
                                s.songsFoundCount(merged.size)
                            }
                        )
                    }
                    prefetchStreams(ytTracks.take(2), delayMs = 1_500L)
                }
                .onFailure { err ->
                    if (jioTracks.isNotEmpty()) {
                        _uiState.update {
                            it.copy(
                                loading = false,
                                tracks = jioTracks,
                                listTitle = s.searchResultsHeader(q),
                                status = s.songsFoundJioCount(jioTracks.size)
                            )
                        }
                    } else {
                        _uiState.update {
                            it.copy(loading = false, status = s.searchErrorStatus(err.message.orEmpty()))
                        }
                    }
                }
        }
    }

    fun isCollectionPinned(browseId: String?): Boolean {
        val norm = normalizeCollectionId(browseId)
        if (norm.isBlank()) return false
        return _uiState.value.pinnedCollections.any { normalizeCollectionId(it.id) == norm }
    }

    fun isPlaylistSavedToLibrary(browseId: String?): Boolean {
        val norm = normalizeCollectionId(browseId)
        if (norm.isBlank()) return false
        val state = _uiState.value
        if (norm in state.savedExternalPlaylistIds) return true
        if (norm in state.ownedPlaylistIds) return false
        return state.playlists.any { normalizeCollectionId(it.id) == norm }
    }

    fun isOwnedEditablePlaylist(browseId: String?): Boolean {
        val state = _uiState.value
        return isEditableUserPlaylist(
            browseId = browseId,
            signedIn = state.signedIn,
            ownedPlaylistIds = state.ownedPlaylistIds,
            savedExternalPlaylistIds = state.savedExternalPlaylistIds,
        )
    }

    fun togglePinCollection(
        id: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
        kindLabel: String = "",
        isAlbum: Boolean = false,
    ) {
        val norm = normalizeCollectionId(id)
        if (norm.isBlank()) return
        val s = uiStrings()
        val cleanTitle = title.trim().ifBlank { s.playlistLabel }
        viewModelScope.launch(Dispatchers.IO) {
            var nowPinned = false
            val updatedList = synchronized(pinnedSavedLock) {
                val existingIdx = cachedPinnedCollections.indexOfFirst { normalizeCollectionId(it.id) == norm }
                val next = if (existingIdx >= 0) {
                    nowPinned = false
                    cachedPinnedCollections.filterNot { normalizeCollectionId(it.id) == norm }
                } else {
                    nowPinned = true
                    val resolvedCover = coverUrl?.takeIf { it.isNotBlank() }
                        ?: _uiState.value.playlists.firstOrNull { normalizeCollectionId(it.id) == norm }?.thumbnailUrl
                    val entry = PinnedCollectionEntry(
                        id = id.trim(),
                        title = cleanTitle,
                        subtitle = subtitle.trim().ifBlank {
                            if (isAlbum) s.albumLabel else s.playlistLabel
                        },
                        coverUrl = resolvedCover,
                        kindLabel = kindLabel.ifBlank { if (isAlbum) s.albumLabel else s.playlistLabel },
                        isAlbum = isAlbum,
                        pinnedAtMs = System.currentTimeMillis(),
                    )
                    listOf(entry) + cachedPinnedCollections
                }
                cachedPinnedCollections = next
                savePinnedCollectionsLocked(next)
                next
            }
            val statusMsg = if (nowPinned) s.pinnedStatus(cleanTitle) else s.unpinnedStatus(cleanTitle)
            _uiState.update { state ->
                state.copy(
                    pinnedCollections = updatedList,
                    status = statusMsg,
                )
            }
        }
    }

    fun toggleSaveExternalPlaylist(
        id: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
        kindLabel: String = "",
    ) {
        val norm = normalizeCollectionId(id)
        if (norm.isBlank() || isOwnedEditablePlaylist(id)) return
        val s = uiStrings()
        val cleanTitle = title.trim().ifBlank { s.playlistLabel }
        viewModelScope.launch(Dispatchers.IO) {
            var nowSaved = false
            val updatedSaved = synchronized(pinnedSavedLock) {
                val existsInSaved = cachedSavedExternalPlaylists.any { normalizeCollectionId(it.id) == norm }
                val inPlaylists = _uiState.value.playlists.any { normalizeCollectionId(it.id) == norm }
                val shouldRemove = existsInSaved || inPlaylists
                val next = if (shouldRemove) {
                    nowSaved = false
                    cachedSavedExternalPlaylists.filterNot { normalizeCollectionId(it.id) == norm }
                } else {
                    nowSaved = true
                    val entry = SavedExternalPlaylist(
                        id = id.trim(),
                        title = cleanTitle,
                        subtitle = subtitle.trim().ifBlank { s.playlistLabel },
                        coverUrl = coverUrl?.takeIf { it.isNotBlank() },
                        kindLabel = kindLabel.ifBlank { s.playlistLabel },
                        savedAtMs = System.currentTimeMillis(),
                    )
                    listOf(entry) + cachedSavedExternalPlaylists
                }
                cachedSavedExternalPlaylists = next
                saveExternalPlaylistsLocked(next)
                next
            }
            val savedIdSet = updatedSaved.mapTo(HashSet()) { normalizeCollectionId(it.id) }
            val statusMsg = if (nowSaved) s.addedToLibraryStatus(cleanTitle) else s.removedFromLibraryStatus(cleanTitle)
            _uiState.update { state ->
                val remoteClean = state.playlists.filter { pl ->
                    val plNorm = normalizeCollectionId(pl.id)
                    !pl.id.startsWith(LOCAL_PLAYLIST_PREFIX) &&
                        plNorm !in state.savedExternalPlaylistIds &&
                        (nowSaved || plNorm != norm)
                }
                state.copy(
                    savedExternalPlaylistIds = savedIdSet,
                    playlists = mergePlaylistsWithLocal(remoteClean),
                    status = statusMsg,
                )
            }
            val engine = requireEngine()
            if (runCatching { engine.isSignedIn() }.getOrDefault(false)) {
                runSuspendCatching { engine.setPlaylistSaved(id, nowSaved) }
                    .onFailure { err ->
                        Log.w("SonataLibrary", "setPlaylistSaved($id, $nowSaved) remote sync failed: ${err.message}")
                    }
            }
        }
    }

    fun openPinnedCollection(entry: PinnedCollectionEntry) {
        if (entry.id == DOWNLOADS_PLAYLIST_ID) {
            openDownloadsPlaylist()
        } else {
            openAlbumOrPlaylist(
                id = entry.id,
                title = entry.title,
                coverUrl = entry.coverUrl,
                subtitle = entry.subtitle,
                kindLabel = entry.kindLabel,
            )
        }
    }

    fun openPlaylist(playlist: AndroidPlaylist) {
        val s = uiStrings()
        openCollectionById(
            id = playlist.id,
            title = playlist.name,
            coverUrl = playlist.thumbnailUrl,
            subtitle = if (playlist.isOwned) (_uiState.value.accountProfile?.name ?: s.playlistLabel) else s.playlistLabel,
            kindLabel = s.playlistLabel,
        )
    }

    fun openDownloadsPlaylist() {
        val s = uiStrings()
        val latestCover = downloads.value.maxByOrNull { it.downloadedAtMs }?.coverUrl?.takeIf { it.isNotBlank() }
            ?: downloads.value.firstOrNull { !it.coverUrl.isNullOrBlank() }?.coverUrl
        openCollectionById(
            id = DOWNLOADS_PLAYLIST_ID,
            title = s.downloads,
            coverUrl = latestCover,
            subtitle = s.offlineSongsCount(downloads.value.size),
            kindLabel = s.offlineLibrary,
        )
    }

    fun openAlbumOrPlaylist(
        id: String,
        title: String,
        coverUrl: String? = null,
        subtitle: String? = null,
        kindLabel: String? = null,
        year: String? = null,
    ) {
        val isAlbumKind = kindLabel.equals("Album", ignoreCase = true) ||
            kindLabel.equals(uiStrings().albumLabel, ignoreCase = true)
        if (isSelfHostedId(id)) {
            if (isAlbumKind) {
                openServerAlbumById(
                    albumId = id,
                    albumTitle = title,
                    artistName = subtitle.orEmpty(),
                    coverUrl = coverUrl,
                    year = year,
                )
            } else {
                openCollectionById(
                    id = id,
                    title = title,
                    coverUrl = coverUrl,
                    subtitle = subtitle,
                    kindLabel = kindLabel,
                    year = year,
                )
            }
            return
        }
        if (isAlbumKind &&
            !id.startsWith("MPRE") &&
            !id.startsWith("OLAK") &&
            !id.startsWith("VL") &&
            !id.startsWith("PL")
        ) {
            openAlbum(
                albumId = id,
                albumTitle = title,
                artistName = subtitle.orEmpty(),
                coverUrl = coverUrl,
                year = year,
            )
            return
        }
        openCollectionById(
            id = id,
            title = title,
            coverUrl = coverUrl,
            subtitle = subtitle,
            kindLabel = kindLabel,
            year = year,
        )
    }

    fun openAlbum(
        albumId: String,
        albumTitle: String,
        artistName: String = "",
        coverUrl: String? = null,
        year: String? = null,
    ) {
        val s = uiStrings()
        val cleanId = albumId.trim()
        val cleanTitle = albumTitle.trim()
        if (isSelfHostedId(cleanId)) {
            openServerAlbumById(
                albumId = cleanId,
                albumTitle = cleanTitle.ifBlank { s.albumLabel },
                artistName = artistName,
                coverUrl = coverUrl,
                year = year,
            )
            return
        }
        if (cleanId.startsWith("MPRE") ||
            cleanId.startsWith("OLAK") ||
            cleanId.startsWith("VL") ||
            cleanId.startsWith("PL")
        ) {
            openCollectionById(
                id = cleanId,
                title = cleanTitle.ifBlank { s.albumLabel },
                coverUrl = coverUrl,
                subtitle = artistName.takeIf { it.isNotBlank() },
                kindLabel = "Album",
                year = year,
            )
            return
        }
        if (cleanTitle.isBlank()) {
            if (artistName.isNotBlank()) openArtist("", artistName)
            return
        }
        speculativePrefetchJob?.cancel()
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionTitle = cleanTitle,
                    collectionCoverUrl = coverUrl,
                    collectionSubtitle = artistName.takeIf { sub -> sub.isNotBlank() },
                    collectionKindLabel = "Album",
                    collectionYear = year,
                    collectionTracks = emptyList(),
                    loading = true,
                    status = s.openingNamedStatus(cleanTitle),
                )
            }
            val resolvedId = runSuspendCatching {
                engine.resolveAlbumBrowseId(cleanTitle, artistName)
            }.getOrNull()
            if (!resolvedId.isNullOrBlank()) {
                openCollectionById(
                    id = resolvedId,
                    title = cleanTitle,
                    coverUrl = coverUrl,
                    subtitle = artistName.takeIf { sub -> sub.isNotBlank() },
                    kindLabel = "Album",
                    year = year,
                )
            } else {
                _uiState.update {
                    it.copy(
                        loading = false,
                        collectionTitle = null,
                        collectionTracks = emptyList(),
                    )
                }
                search(listOf(cleanTitle, artistName).filter { part -> part.isNotBlank() }.joinToString(" "))
            }
        }
    }

    fun openArtist(browseId: String, name: String = "") {
        val cleanBrowseId = browseId.trim()
        val cleanName = name.trim()
        if (cleanBrowseId.isBlank() && cleanName.isBlank()) return
        speculativePrefetchJob?.cancel()

        val cached = (if (cleanBrowseId.startsWith("UC")) getCachedArtistPage(cleanBrowseId) else null)
            ?: cleanName.takeIf { it.isNotBlank() }?.let { getCachedArtistPage("name:$it") }
        if (cached != null) {
            _uiState.update {
                it.copy(
                    collectionBrowseId = null,
                    collectionTitle = null,
                    collectionTracks = emptyList(),
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    artistLoading = false,
                    artistPage = cached,
                    status = cached.name,
                )
            }
            prefetchStreams(cached.popularTracks.take(2), delayMs = 1_000L)
            return
        }

        val s = uiStrings()
        artistJob?.cancel()
        artistJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    collectionBrowseId = null,
                    collectionTitle = null,
                    collectionTracks = emptyList(),
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    artistLoading = true,
                    artistPage = AndroidArtistPage(
                        id = cleanBrowseId,
                        name = cleanName.ifBlank { s.artistDefault },
                        subtitle = "",
                        description = null,
                        subscribers = null,
                        monthlyListeners = null,
                        thumbnailUrl = null,
                        shufflePlaylistId = null,
                        radioPlaylistId = null,
                        latestRelease = null,
                        popularTracks = emptyList(),
                        releases = emptyList(),
                        relatedArtists = emptyList(),
                    ),
                    status = s.loadingNamedStatus(cleanName.ifBlank { s.artistDefault }),
                )
            }
            val result = runSuspendCatching {
                if (cleanBrowseId.startsWith("UC")) {
                    engine.fetchArtistPage(cleanBrowseId)
                } else {
                    engine.fetchArtistByName(cleanName)
                }
            }
            result
                .onSuccess { page ->
                    putCachedArtistPage(page, cleanName)
                    _uiState.update {
                        it.copy(
                            artistLoading = false,
                            artistPage = page,
                            status = page.name,
                        )
                    }
                    prefetchStreams(page.popularTracks.take(2), delayMs = 1_200L)
                }
                .onFailure { err ->
                    if (!cleanBrowseId.startsWith("UC") && cleanName.isNotBlank()) {
                        _uiState.update {
                            it.copy(
                                artistLoading = false,
                                artistPage = null,
                            )
                        }
                        search(cleanName)
                    } else {
                        _uiState.update {
                            it.copy(
                                artistLoading = false,
                                status = s.artistErrorStatus(err.message.orEmpty()),
                            )
                        }
                    }
                }
        }
    }

    fun selectHomeMoodChip(params: String?) {
        val currentSelected = _uiState.value.selectedHomeMoodParams
        val nextParams = when {
            params.isNullOrBlank() -> null
            currentSelected == params -> null
            else -> params
        }
        homeMoodChipJob?.cancel()
        if (nextParams == null) {
            val cachedFull = fullHomeShelves
            if (cachedFull.isNotEmpty()) {
                val updatedArtists = buildRecentArtistCards(_lastPlayedTracks.value, cachedFull)
                _uiState.update {
                    it.copy(
                        selectedHomeMoodParams = null,
                        shelves = cachedFull,
                        recentArtistCards = updatedArtists,
                        refreshing = false,
                        status = uiStrings().personalizedShelvesCount(cachedFull.size),
                    )
                }
                return
            }
            _uiState.update { it.copy(selectedHomeMoodParams = null, refreshing = true) }
            homeMoodChipJob?.cancel()
            homeMoodChipJob = viewModelScope.launch(Dispatchers.IO) {
                val engine = requireEngine()
                runSuspendCatching { engine.fetchHomeMoodFeed(null) }
                    .onSuccess { rawShelves ->
                        if (!isActive || _uiState.value.selectedHomeMoodParams != null) return@onSuccess
                        val cleaned = normalizeAndMergeHomeShelves(rawShelves)
                        if (cleaned.isNotEmpty()) {
                            fullHomeShelves = cleaned
                        }
                        val updatedArtists = buildRecentArtistCards(_lastPlayedTracks.value, cleaned)
                        _uiState.update { state ->
                            state.copy(
                                shelves = cleaned.ifEmpty { state.shelves },
                                recentArtistCards = updatedArtists,
                                refreshing = false,
                                status = uiStrings().personalizedShelvesCount(cleaned.size),
                            )
                        }
                    }
                    .onFailure { err ->
                        handleRateLimitIfNeeded(err)
                        _uiState.update {
                            it.copy(
                                refreshing = false,
                                status = uiStrings().homeErrorStatus(err.message.orEmpty()),
                            )
                        }
                    }
            }
            return
        }

        _uiState.update { it.copy(selectedHomeMoodParams = nextParams, refreshing = true) }
        homeMoodChipJob?.cancel()
        homeMoodChipJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.fetchHomeMoodFeed(nextParams) }
                .onSuccess { rawShelves ->
                    if (!isActive || _uiState.value.selectedHomeMoodParams != nextParams) return@onSuccess
                    val cleaned = normalizeAndMergeHomeShelves(rawShelves)
                    val updatedArtists = buildRecentArtistCards(_lastPlayedTracks.value, cleaned)
                    _uiState.update { state ->
                        state.copy(
                            shelves = cleaned.ifEmpty { state.shelves },
                            recentArtistCards = updatedArtists,
                            refreshing = false,
                            status = uiStrings().personalizedShelvesCount(cleaned.size),
                        )
                    }
                }
                .onFailure { err ->
                    handleRateLimitIfNeeded(err)
                    _uiState.update { state ->
                        val fallback = if (fullHomeShelves.isNotEmpty()) fullHomeShelves else state.shelves
                        state.copy(
                            selectedHomeMoodParams = null,
                            shelves = fallback,
                            refreshing = false,
                            status = uiStrings().homeErrorStatus(err.message.orEmpty()),
                        )
                    }
                }
        }
    }

    fun openMoodCategory(title: String, browseId: String = "", params: String = "") {
        val cleanTitle = title.trim()
        if (cleanTitle.isBlank() && browseId.isBlank()) return
        speculativePrefetchJob?.cancel()
        moodCategoryJob?.cancel()
        val s = uiStrings()
        val displayTitle = localizeMoodTitle(cleanTitle, s).ifBlank { cleanTitle }
        _uiState.update {
            it.copy(
                collectionBrowseId = null,
                collectionTitle = null,
                collectionTracks = emptyList(),
                artistPage = null,
                artistLoading = false,
                showLyrics = false,
                moodCategoryTitle = displayTitle,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = true,
                status = s.loadingNamedStatus(displayTitle),
            )
        }
        moodCategoryJob?.cancel()
        moodCategoryJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            var resolvedBrowseId = browseId.trim()
            var resolvedParams = params.trim()

            if (resolvedBrowseId.isBlank() || resolvedParams.isBlank()) {
                var categories = _uiState.value.moodCategories
                if (categories.isEmpty()) {
                    runSuspendCatching { engine.fetchExploreFeed() }
                        .onSuccess { feed ->
                            categories = feed.moods
                            val cleanedExplore = normalizeAndMergeHomeShelves(feed.shelves)
                            _uiState.update { state ->
                                state.copy(
                                    exploreShelves = cleanedExplore.ifEmpty { state.exploreShelves },
                                    moodCategories = feed.moods.ifEmpty { state.moodCategories },
                                )
                            }
                        }
                        .onFailure { handleRateLimitIfNeeded(it) }
                }
                val match = categories.firstOrNull { cat ->
                    val catRaw = cat.title.trim()
                    val catLoc = localizeMoodTitle(catRaw, s)
                    val enLoc = localizeMoodTitle(catRaw, EnglishSonataStrings)
                    catRaw.equals(cleanTitle, ignoreCase = true) ||
                        catRaw.equals(displayTitle, ignoreCase = true) ||
                        catLoc.equals(cleanTitle, ignoreCase = true) ||
                        catLoc.equals(displayTitle, ignoreCase = true) ||
                        enLoc.equals(cleanTitle, ignoreCase = true) ||
                        (resolvedBrowseId.isNotBlank() && cat.browseId == resolvedBrowseId)
                }
                if (match != null) {
                    if (resolvedBrowseId.isBlank()) resolvedBrowseId = match.browseId
                    if (resolvedParams.isBlank()) resolvedParams = match.params
                }
            }

            if (resolvedBrowseId.isNotBlank()) {
                val rawShelves = runSuspendCatching {
                    engine.fetchMoodCategory(resolvedBrowseId, resolvedParams)
                }.onFailure { err ->
                    handleRateLimitIfNeeded(err)
                }.getOrDefault(emptyList())
                if (!isActive) return@launch
                val cleaned = normalizeAndMergeHomeShelves(rawShelves)
                if (cleaned.isNotEmpty()) {
                    _uiState.update {
                        it.copy(
                            moodCategoryTitle = displayTitle,
                            moodCategoryShelves = cleaned,
                            moodCategoryLoading = false,
                            status = displayTitle,
                        )
                    }
                    return@launch
                }
            }

            if (!isActive) return@launch
            _uiState.update {
                it.copy(
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                )
            }
            search("$cleanTitle music")
        }
    }

    fun openMood(title: String) {
        openMoodCategory(title)
    }

    fun closeMoodCategory() {
        moodCategoryJob?.cancel()
        _uiState.update {
            it.copy(
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
            )
        }
    }

    fun openCard(card: AndroidHomeCard, shelfTracks: List<AndroidTrack> = emptyList()) {
        when (card.kind) {
            AndroidHomeCardKind.SONG, AndroidHomeCardKind.VIDEO -> {
                card.track?.let { track ->
                    val queueSource = shelfTracks.ifEmpty { listOf(track) }
                    playTrack(track, queueSource)
                }
            }
            AndroidHomeCardKind.MOOD -> openMoodCategory(card.title, card.id)
            AndroidHomeCardKind.PLAYLIST, AndroidHomeCardKind.ALBUM -> {
                val kind = if (card.kind == AndroidHomeCardKind.ALBUM) "Album" else "Playlist"
                val year = YEAR_REGEX.find(card.subtitle)?.value
                openCollectionById(
                    id = card.id,
                    title = card.title,
                    coverUrl = card.thumbnailUrl,
                    subtitle = card.subtitle.ifBlank { null },
                    kindLabel = kind,
                    year = year,
                )
            }
            AndroidHomeCardKind.ARTIST -> openArtist(card.id, card.title)
        }
    }

    private fun openCollectionById(
        id: String,
        title: String,
        coverUrl: String? = null,
        subtitle: String? = null,
        kindLabel: String? = null,
        year: String? = null,
    ) {
        val s = uiStrings()
        speculativePrefetchJob?.cancel()
        if (id == DOWNLOADS_PLAYLIST_ID) {
            val latestDownloadCover = downloads.value.maxByOrNull { it.downloadedAtMs }?.coverUrl?.takeIf { it.isNotBlank() }
            val offlineTracks = downloads.value.map { it.toAndroidTrack() }
            val resolvedCover = coverUrl ?: latestDownloadCover ?: offlineTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            syncCollectionMetadataToSavedAndPinned(
                id = id,
                title = s.downloads,
                subtitle = s.offlineSongsCount(offlineTracks.size),
                coverUrl = resolvedCover,
            )
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionBrowseId = id,
                    collectionTitle = s.downloads,
                    collectionCoverUrl = resolvedCover,
                    collectionSubtitle = s.offlineSongsCount(offlineTracks.size),
                    collectionKindLabel = s.offlineLibrary,
                    collectionYear = null,
                    collectionTracks = offlineTracks,
                    loading = false,
                    status = s.downloadedSongsCount(offlineTracks.size),
                )
            }
            return
        }
        if (id.startsWith(LOCAL_PLAYLIST_PREFIX)) {
            val localTracks = getLocalPlaylistTracks(id)
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionBrowseId = id,
                    collectionTitle = title,
                    collectionCoverUrl = coverUrl ?: localTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
                    collectionSubtitle = s.offlinePlaylistSongsCount(localTracks.size),
                    collectionKindLabel = kindLabel ?: s.offlinePlaylist,
                    collectionYear = year,
                    collectionTracks = localTracks,
                    loading = false,
                    status = s.tracksInCollectionStatus(localTracks.size, title),
                )
            }
            return
        }
        collectionJob?.cancel()
        collectionJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionBrowseId = id,
                    collectionTitle = title,
                    collectionCoverUrl = coverUrl,
                    collectionSubtitle = subtitle,
                    collectionKindLabel = kindLabel,
                    collectionYear = year,
                    collectionTracks = emptyList(),
                    loading = true,
                    status = s.loadingNamedStatus(title)
                )
            }
            runSuspendCatching {
                if (isSelfHostedId(id)) {
                    engine.getServerPlaylistTracks(id)
                } else {
                    engine.fetchCollectionTracksFresh(id)
                }
            }
                .onSuccess { rawTracks ->
                    val firstLiveTrackCover = rawTracks.firstNotNullOfOrNull { resolveTrackCoverUrl(it) }
                    val fallbackCover = if (isLikedMusicPlaylistId(id)) {
                        firstLiveTrackCover ?: coverUrl
                    } else {
                        coverUrl ?: firstLiveTrackCover
                    }
                    val tracks = backfillCollectionTracks(
                        rawTracks = rawTracks,
                        id = id,
                        title = title,
                        subtitle = subtitle,
                        coverUrl = fallbackCover,
                    )
                    if (isLikedMusicPlaylistId(id)) {
                        val likedIds = tracks.mapNotNull { t -> t.id.takeIf { it.isNotBlank() } }
                        if (likedIds.isNotEmpty()) {
                            _uiState.update { state ->
                                val mergedLiked = state.likedTrackIds + likedIds
                                if (mergedLiked == state.likedTrackIds) state else state.copy(likedTrackIds = mergedLiked)
                            }
                        }
                        updateLatestLikedCover(fallbackCover, clearIfNull = tracks.isEmpty())
                    }
                    if (trackCollectionContext.size > 512) {
                        trackCollectionContext.clear()
                    }
                    tracks.forEach { t ->
                        if (t.id.isNotBlank()) trackCollectionContext[t.id] = id
                    }
                    val resolvedHeaderCover = if (isLikedMusicPlaylistId(id)) {
                        fallbackCover ?: _uiState.value.collectionCoverUrl
                    } else {
                        _uiState.value.collectionCoverUrl ?: fallbackCover
                    }
                    syncCollectionMetadataToSavedAndPinned(
                        id = id,
                        title = title,
                        subtitle = subtitle,
                        coverUrl = resolvedHeaderCover,
                    )
                    _uiState.update { state ->
                        state.copy(
                            loading = false,
                            collectionBrowseId = id,
                            collectionTitle = title,
                            collectionCoverUrl = resolvedHeaderCover,
                            collectionTracks = tracks,
                            status = s.tracksInCollectionStatus(tracks.size, title)
                        )
                    }
                    prefetchStreams(tracks.take(2), delayMs = 1_500L)
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = s.collectionErrorStatus(err.message.orEmpty()))
                    }
                }
        }
    }

    fun prefetchStreams(
        tracks: List<AndroidTrack>,
        delayMs: Long = 1_500L,
        cancelPrevious: Boolean = true,
        expectedGeneration: Long? = null,
    ) {
        val candidates = tracks.distinctBy { it.id }.take(2)
        if (candidates.isEmpty()) return
        if (cancelPrevious) {
            speculativePrefetchJob?.cancel()
        }
        val job = viewModelScope.launch(prefetchDispatcher) {
            if (delayMs > 0L) {
                delay(delayMs)
            }
            if (expectedGeneration != null && !playbackGeneration.isCurrent(expectedGeneration)) {
                return@launch
            }
            val engine = requireEngine()
            resolvePrefetchList(engine, candidates, expectedGeneration)
        }
        if (cancelPrevious) {
            speculativePrefetchJob = job
        }
    }

    private suspend fun resolvePrefetchList(
        engine: SonataEngineGateway,
        candidates: List<AndroidTrack>,
        expectedGeneration: Long? = null,
    ) {
        for (track in candidates) {
            kotlin.coroutines.coroutineContext.ensureActive()
            if (expectedGeneration != null && !playbackGeneration.isCurrent(expectedGeneration)) {
                return
            }
            if (getCachedStream(track.id) != null || !inFlightPrefetch.add(track.id)) continue
            try {
                runSuspendCatching { engine.resolveStream(track) }
                    .onSuccess { stream ->
                        putCachedStream(track.id, stream)
                        val itagMatch = ITAG_REGEX.find(stream.url)?.groupValues?.getOrNull(1) ?: "local"
                        Log.i(
                            "SonataStream",
                            "prefetched track=${track.id} title=\"${track.title}\" itag=$itagMatch badge=\"${stream.badge}\""
                        )
                        val generationStillValid = expectedGeneration == null || playbackGeneration.isCurrent(expectedGeneration)
                        if (generationStillValid && _uiState.value.activeTrack?.id == track.id && !_uiState.value.isPlaying) {
                            saveActiveTrackToPrefs(track, stream.badge)
                            _uiState.update { it.copy(streamBadge = stream.badge) }
                        }
                    }
            } finally {
                inFlightPrefetch.remove(track.id)
            }
        }
    }

    fun jumpToQueueIndex(index: Int) {
        val queue = _uiState.value.queue
        if (index in queue.indices) {
            playTrack(queue[index], queue)
        }
    }

    @OptIn(UnstableApi::class)
    fun playTrack(track: AndroidTrack, queueList: List<AndroidTrack>? = null) {
        playJob?.cancel()
        upgradeJob?.cancel()
        recoveryJob?.cancel()
        autoplayJob?.cancel()
        autoplayPrefetchJob?.cancel()
        speculativePrefetchJob?.cancel()
        lyricsJob?.cancel()
        canvasJob?.cancel()
        musicVideoProbeJob?.cancel()
        videoSwitchJob?.cancel()
        val generation = playbackGeneration.next()
        val wasVideoMode = _uiState.value.videoModeEnabled
        if (wasVideoMode) {
            PlaybackAudioBridge.setVideoModePlayback(false)
        }
        _uiState.update { state ->
            if (!state.videoModeEnabled &&
                !state.videoFullscreen &&
                state.musicVideoCandidate == null &&
                state.availableVideoQualities.isEmpty() &&
                !state.videoLoading
            ) {
                state
            } else {
                state.copy(
                    musicVideoCandidate = null,
                    videoModeEnabled = false,
                    videoFullscreen = false,
                    videoLoading = false,
                    availableVideoQualities = emptyList(),
                    activeVideoQualityLabel = "",
                    activeVideoQualityHeight = 0,
                )
            }
        }
        val previousQueue = _uiState.value.queue
        val isNewExplicitQueue = !queueList.isNullOrEmpty() && !queuesSharePrefix(queueList, previousQueue)
        if (isNewExplicitQueue) {
            playedTrackIdsInCurrentQueue.clear()
            prefetchedAutoplaySeedKey = null
            prefetchedAutoplayTracks = emptyList()
        }
        val updatedQueue = when {
            !queueList.isNullOrEmpty() -> {
                if (queueList.any { it.id == track.id }) queueList else listOf(track) + queueList
            }
            _uiState.value.queue.any { it.id == track.id } -> _uiState.value.queue
            else -> listOf(track)
        }
        val updatedQueueIds = updatedQueue.mapTo(HashSet(updatedQueue.size)) { it.id }
        playedTrackIdsInCurrentQueue.retainAll(updatedQueueIds)
        playedTrackIdsInCurrentQueue.add(track.id)
        val updatedIndex = updatedQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

        playbackClock.resetForTrack((track.durationSecs?.toLong() ?: 0L) * 1000L)
        resolveCanvasForTrack(track, generation)
        probeMusicVideoForTrack(track, generation)

        playJob = viewModelScope.launch(Dispatchers.IO) {
            recordLastPlayed(track)
            val playbackPlaylistId = resolvePlaybackPlaylistId(track, queueList)
            if (isNewExplicitQueue) {
                activeQueuePlaylistId = playbackPlaylistId
            }
            maybePrefetchEndlessAutoplay(updatedQueue, updatedIndex, generation)
            val offlineUri = downloadManager.localUriFor(track.id)
            if (offlineUri != null) {
                if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
                val offlineRecord = downloads.value.firstOrNull { it.videoId == track.id }
                val offlineBadge = offlineRecord?.codec?.takeIf { it.isNotBlank() }?.let { "Offline · $it" } ?: "Offline"
                saveActiveTrackToPrefs(track, offlineBadge)
                withContext(Dispatchers.Main) {
                    if (playbackGeneration.isCurrent(generation)) {
                        runCatching { controller?.pause() }
                    }
                }
                if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
                _uiState.update {
                    it.copy(
                        queue = updatedQueue,
                        queueIndex = updatedIndex,
                        activeTrack = track,
                        streamBadge = offlineBadge,
                        status = uiStrings().playingTrackOfflineStatus(track.title),
                        isPlaying = false,
                        isBuffering = true,
                        isUpgradingQuality = false,
                        lyrics = emptyList(),
                        lyricsLoading = true,
                        lyricsUnavailable = false,
                    )
                }
                lyricsFetchedForTrackId = null
                if ((track.durationSecs ?: 0uL) > 0uL) {
                    fetchLyricsInBackground(track, generation)
                }
                val offlineItem = buildOfflineMediaItem(track, offlineUri)
                withContext(Dispatchers.Main) {
                    if (!isActive || !isGenerationActive(generation, track.id)) return@withContext
                    controller?.apply {
                        setMediaItem(offlineItem)
                        prepare()
                        play()
                    }
                }
                reportTrackPlaybackToYtm(track, playbackPlaylistId)
                val engine = requireEngine()
                launchBackgroundQualityUpgrade(
                    generation = generation,
                    engine = engine,
                    effectiveTrack = track,
                    currentStream = AndroidResolvedStream(
                        url = offlineUri,
                        badge = offlineBadge,
                        headers = emptyList(),
                    ),
                )
                return@launch
            }

            val engine = requireEngine()
            if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
            val cachedStream = getCachedStream(track.id)
            saveActiveTrackToPrefs(track, cachedStream?.badge.orEmpty())
            withContext(Dispatchers.Main) {
                if (playbackGeneration.isCurrent(generation)) {
                    controller?.pause()
                }
            }
            if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
            _uiState.update {
                it.copy(
                    queue = updatedQueue,
                    queueIndex = updatedIndex,
                    activeTrack = track,
                    streamBadge = cachedStream?.badge ?: uiStrings().resolving,
                    status = if (cachedStream != null) {
                        uiStrings().playingTrackStatus(track.title)
                    } else {
                        uiStrings().resolvingTrackStatus(track.title)
                    },
                    isPlaying = false,
                    isBuffering = true,
                    isUpgradingQuality = false,
                    lyrics = emptyList(),
                    lyricsLoading = true,
                    lyricsUnavailable = false,
                )
            }

            lyricsFetchedForTrackId = null
            // Fetch lyrics immediately only when track duration is already known; otherwise wait for
            // ExoPlayer STATE_READY so provider duration matching rejects mismatched cuts/versions.
            if ((track.durationSecs ?: 0uL) > 0uL) {
                fetchLyricsInBackground(track, generation)
            }

            val (effectiveTrack, effectiveQueue, streamResult) = if (track.id.startsWith("jiosaavn:")) {
                val resolved = if (cachedStream != null) {
                    Result.success(cachedStream)
                } else {
                    runSuspendCatching {
                        val js = engine.resolveJiosaavnById(track.id)
                            ?: error("JioSaavn stream unavailable")
                        AndroidResolvedStream(
                            url = js.url,
                            badge = js.badge,
                            headers = emptyList(),
                        )
                    }.onSuccess { putCachedStream(track.id, it) }
                }
                Triple(track, updatedQueue, resolved)
            } else if (isSelfHostedId(track.id)) {
                val resolved = if (cachedStream != null) {
                    Result.success(cachedStream)
                } else {
                    runSuspendCatching { engine.resolveStream(track) }
                        .onSuccess { putCachedStream(track.id, it) }
                }
                Triple(track, updatedQueue, resolved)
            } else coroutineScope {
                val baseStreamDeferred = async {
                    if (cachedStream != null) {
                        Result.success(cachedStream)
                    } else {
                        runSuspendCatching { engine.resolveStream(track) }.onSuccess { resolved ->
                            putCachedStream(track.id, resolved)
                        }
                    }
                }
                val explicitCounterpartDeferred = if (!track.isExplicit && track.title.isNotBlank()) {
                    async {
                        withTimeoutOrNull(1_200L) {
                            runSuspendCatching { engine.resolveExplicitCounterpart(track) }.getOrNull()
                        }
                    }
                } else {
                    null
                }
                val explicitMatch = explicitCounterpartDeferred?.await()?.takeIf { it.id != track.id }
                if (explicitMatch != null) {
                    if (!isActive || !playbackGeneration.isCurrent(generation)) {
                        return@coroutineScope Triple(track, updatedQueue, Result.failure<AndroidResolvedStream>(IllegalStateException("Stale generation")))
                    }
                    Log.i(
                        "SonataStream",
                        "Preferring explicit track id=${explicitMatch.id} title='${explicitMatch.title}' over id=${track.id}"
                    )
                    val upgradedQueue = updatedQueue.toMutableList().apply {
                        if (updatedIndex in indices) set(updatedIndex, explicitMatch)
                    }
                    recordLastPlayed(explicitMatch)
                    resolveCanvasForTrack(explicitMatch, generation)
                    probeMusicVideoForTrack(explicitMatch, generation)
                    _uiState.update { state ->
                        if (isGenerationActive(generation, track.id)) {
                            state.copy(
                                activeTrack = explicitMatch,
                                queue = upgradedQueue,
                            )
                        } else {
                            state
                        }
                    }
                    val explicitCached = getCachedStream(explicitMatch.id)
                    val explicitStream = if (explicitCached != null) {
                        Result.success(explicitCached)
                    } else {
                        runSuspendCatching { engine.resolveStream(explicitMatch) }.onSuccess { resolved ->
                            putCachedStream(explicitMatch.id, resolved)
                        }
                    }
                    Triple(explicitMatch, upgradedQueue, explicitStream)
                } else {
                    Triple(track, updatedQueue, baseStreamDeferred.await())
                }
            }

            if (!isActive || !isGenerationActive(generation, effectiveTrack.id)) return@launch

            streamResult
                .onSuccess { stream ->
                    if (!isActive || !isGenerationActive(generation, effectiveTrack.id)) return@onSuccess
                    val itag = ITAG_REGEX.find(stream.url)?.groupValues?.getOrNull(1) ?: "unknown"
                    Log.i(
                        "SonataStream",
                        "Playing track='${effectiveTrack.title}' artist='${effectiveTrack.artist}' gen=$generation explicit=${effectiveTrack.isExplicit} itag=$itag badge='${stream.badge}'"
                    )
                    val swap = withContext(Dispatchers.Main) {
                        runCatching {
                            performAtomicStreamSwap(
                                player = controllerSwapAdapter(controller),
                                track = effectiveTrack,
                                stream = stream,
                                overridePositionMs = 0L,
                                overridePlayIntent = true,
                                isStillCurrent = { isActive && isGenerationActive(generation, effectiveTrack.id) },
                                onCommitted = { _, _ ->
                                    _uiState.update {
                                        it.copy(
                                            streamBadge = stream.badge,
                                            status = uiStrings().playingTrackStatus(effectiveTrack.title),
                                        )
                                    }
                                },
                            )
                        }.onFailure { err ->
                            Log.w("SonataStream", "Controller setMediaItem failed: ${err.message}")
                        }.getOrNull()
                    }
                    if (swap?.applied != true) {
                        PlaybackAudioBridge.clearEndedTransitionSuppression()
                        return@onSuccess
                    }
                    saveActiveTrackToPrefs(effectiveTrack, stream.badge)
                    reportTrackPlaybackToYtm(effectiveTrack, playbackPlaylistId)

                    // Delay upcoming queue prefetch so ExoPlayer gets full bandwidth for instant audio start.
                    val upcoming = PlaybackEligibility.selectUpcomingPrecacheCandidates(
                        queue = effectiveQueue,
                        currentIndex = updatedIndex,
                        lookaheadCount = 2,
                    )
                    if (upcoming.isNotEmpty()) {
                        prefetchStreams(
                            tracks = upcoming,
                            delayMs = 1_500L,
                            cancelPrevious = true,
                            expectedGeneration = generation,
                        )
                    }

                    launchBackgroundQualityUpgrade(
                        generation = generation,
                        engine = engine,
                        effectiveTrack = effectiveTrack,
                        currentStream = stream,
                    )
                }
                .onFailure { err ->
                    if (!isActive || !isGenerationActive(generation, effectiveTrack.id)) return@onFailure
                    PlaybackAudioBridge.clearEndedTransitionSuppression()
                    val s = uiStrings()
                    _uiState.update {
                        it.copy(
                            isBuffering = false,
                            isUpgradingQuality = false,
                            streamBadge = s.streamBadgeError,
                            status = s.playFailedStatus(err.message.orEmpty())
                        )
                    }
                }
        }
    }

    @OptIn(UnstableApi::class)
    private fun buildOfflineMediaItem(track: AndroidTrack, localUri: String): MediaItem {
        val uri = if (localUri.startsWith("content://") || localUri.startsWith("file://")) {
            Uri.parse(localUri)
        } else {
            Uri.fromFile(File(localUri))
        }
        return MediaItem.Builder()
            .setMediaId(track.id)
            .setUri(uri)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(track.title)
                    .setArtist(track.artist)
                    .setAlbumTitle(track.album)
                    .apply {
                        track.thumbnailUrl?.let { setArtworkUri(Uri.parse(it)) }
                    }
                    .build()
            )
            .build()
    }

    @OptIn(UnstableApi::class)
    private fun buildMediaItem(track: AndroidTrack, stream: AndroidResolvedStream): MediaItem {
        if (stream.headers.isNotEmpty()) {
            PlaybackAudioBridge.registerStreamHeaders(
                url = stream.url,
                headers = stream.headers.associate { it.name to it.value },
            )
        }
        val uri = if (stream.url.startsWith("http")) Uri.parse(stream.url) else Uri.fromFile(File(stream.url))
        return MediaItem.Builder()
            .setMediaId(track.id)
            .setCustomCacheKey("${track.id}:${stream.badge}")
            .setUri(uri)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(track.title)
                    .setArtist(track.artist)
                    .setAlbumTitle(track.album)
                    .apply {
                        track.thumbnailUrl?.let { setArtworkUri(Uri.parse(it)) }
                    }
                    .build()
            )
            .build()
    }

    private fun launchBackgroundQualityUpgrade(
        generation: Long,
        engine: SonataEngineGateway,
        effectiveTrack: AndroidTrack,
        currentStream: AndroidResolvedStream,
    ) {
        if (_uiState.value.videoModeEnabled) return
        if (isSelfHostedId(effectiveTrack.id)) return
        val qualityTier = settings.value.audioQuality
        if (!_uiState.value.preferHighQuality || qualityTier == AudioQualityTier.LOW) return

        val lowerBadge = currentStream.badge.lowercase()
        val initialIsLossless = lowerBadge.contains("flac") ||
            lowerBadge.contains("lossless") ||
            lowerBadge.contains("alac")
        if (initialIsLossless) return

        val isOfflinePlayback = lowerBadge.startsWith("offline")
        val rawKbps = parseBitrateKbps(currentStream.badge)
        val baselineKbps = if (rawKbps > 0) rawKbps else if (isOfflinePlayback) 160 else 0
        val ceilingKbps = when (qualityTier) {
            AudioQualityTier.LOW -> 96
            AudioQualityTier.MEDIUM -> 160
            AudioQualityTier.HIGH -> 320
            AudioQualityTier.LOSSLESS -> 2000
        }
        if (baselineKbps >= ceilingKbps) return

        val signedIn = _uiState.value.signedIn
        val allowYtmUpgrade = !effectiveTrack.id.startsWith("jiosaavn:") && when (qualityTier) {
            AudioQualityTier.LOW -> false
            AudioQualityTier.MEDIUM -> baselineKbps in 1..135
            AudioQualityTier.HIGH -> baselineKbps < 240 || (!isOfflinePlayback && shouldUpgradeStreamBadge(currentStream.badge, signedIn))
            AudioQualityTier.LOSSLESS -> baselineKbps < 256 || (!isOfflinePlayback && shouldUpgradeStreamBadge(currentStream.badge, signedIn))
        }
        val allowJioSaavnUpgrade = settings.value.jioSaavnEnabled &&
            !effectiveTrack.id.startsWith("jiosaavn:") &&
            (qualityTier == AudioQualityTier.HIGH || qualityTier == AudioQualityTier.LOSSLESS) &&
            baselineKbps < 320

        if (!allowYtmUpgrade && !allowJioSaavnUpgrade) return

        upgradeJob?.cancel()
        _uiState.update {
            if (isGenerationActive(generation, effectiveTrack.id)) it.copy(isUpgradingQuality = true) else it
        }
        upgradeJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                var upgraded: AndroidResolvedStream? = if (allowYtmUpgrade) {
                    val probeStream = if (isOfflinePlayback) {
                        currentStream.copy(url = "https://offline.local", badge = "AAC ${baselineKbps}kbps · Cached")
                    } else {
                        currentStream
                    }
                    runSuspendCatching { engine.resolveHifiUpgrade(effectiveTrack, probeStream) }
                        .getOrNull()
                        ?.takeIf { cand ->
                            val candKbps = parseBitrateKbps(cand.badge)
                            val candLossless = cand.badge.contains("FLAC", ignoreCase = true) ||
                                cand.badge.contains("Lossless", ignoreCase = true)
                            val fitsTier = qualityTier == AudioQualityTier.LOSSLESS || (!candLossless && (candKbps == 0 || candKbps <= ceilingKbps))
                            val improves = (candLossless && qualityTier == AudioQualityTier.LOSSLESS) || candKbps > baselineKbps
                            fitsTier && improves
                        }
                } else {
                    null
                }

                if (allowJioSaavnUpgrade && isActive && isGenerationActive(generation, effectiveTrack.id)) {
                    val bestSoFar = upgraded ?: currentStream
                    val bestIsLossless = bestSoFar.badge.contains("FLAC", ignoreCase = true) ||
                        bestSoFar.badge.contains("Lossless", ignoreCase = true)
                    val bestKbps = parseBitrateKbps(bestSoFar.badge).let { if (it > 0) it else baselineKbps }
                    if (!bestIsLossless && bestKbps < 320) {
                        val jioMatch = runSuspendCatching {
                            engine.findJiosaavnUpgrade(effectiveTrack, bestKbps.coerceAtLeast(0).toUInt())
                        }.getOrNull()
                        val jioKbps = jioMatch?.bitrateKbps?.toInt() ?: 0
                        if (jioMatch != null && jioKbps > bestKbps && jioKbps <= ceilingKbps) {
                            Log.i(
                                "SonataStream",
                                "JioSaavn quality upgrade for '${effectiveTrack.title}': ${bestKbps}kbps -> ${jioKbps}kbps"
                            )
                            upgraded = AndroidResolvedStream(
                                url = jioMatch.url,
                                badge = jioMatch.badge,
                                headers = emptyList(),
                            )
                        }
                    }
                }

                val targetUpgrade = upgraded
                if (targetUpgrade != null && isActive && isGenerationActive(generation, effectiveTrack.id)) {
                    val swap = withContext(Dispatchers.Main) {
                        runCatching {
                            performAtomicStreamSwap(
                                player = controllerSwapAdapter(controller),
                                track = effectiveTrack,
                                stream = targetUpgrade,
                                isStillCurrent = { isActive && isGenerationActive(generation, effectiveTrack.id) },
                                onCommitted = { _, _ ->
                                    putCachedStream(effectiveTrack.id, targetUpgrade)
                                    _uiState.update { it.copy(streamBadge = targetUpgrade.badge) }
                                },
                            )
                        }.onFailure { err ->
                            Log.w("SonataStream", "Hi-Fi swap failed: ${err.message}")
                        }.getOrNull()
                    }
                    if (swap?.applied == true) {
                        saveActiveTrackToPrefs(effectiveTrack, targetUpgrade.badge)
                    }
                }
            } finally {
                if (isGenerationActive(generation, effectiveTrack.id)) {
                    _uiState.update { it.copy(isUpgradingQuality = false) }
                }
            }
        }
    }

    private fun queuesSharePrefix(candidate: List<AndroidTrack>, existing: List<AndroidTrack>): Boolean {
        if (existing.isEmpty() || candidate.size < existing.size) return false
        for (i in existing.indices) {
            if (candidate[i].id != existing[i].id) return false
        }
        return true
    }

    private fun selectDistinctArtistIds(tracks: List<AndroidTrack>, maxCount: Int): List<String> {
        val out = ArrayList<String>(maxCount)
        val seenArtists = HashSet<String>()
        for (t in tracks) {
            if (t.id.isBlank() || t.id.startsWith("local:")) continue
            val normArtist = t.artist.trim().lowercase()
            if (normArtist.isBlank() || seenArtists.add(normArtist)) {
                out.add(t.id)
                if (out.size >= maxCount) break
            }
        }
        return out
    }

    private fun buildPreferredArtists(): List<String> {
        val scores = LinkedHashMap<String, Int>()
        val displayNames = HashMap<String, String>()
        _lastPlayedTracks.value.take(30).forEachIndexed { idx, track ->
            val name = track.artist.trim()
            if (name.isNotBlank()) {
                val key = name.lowercase()
                displayNames.putIfAbsent(key, name)
                scores[key] = (scores[key] ?: 0) + (30 - idx).coerceAtLeast(2)
            }
        }
        val state = _uiState.value
        (state.shelves.flatMap { it.tracks } + state.collectionTracks)
            .filter { it.id in state.likedTrackIds }
            .forEach { track ->
                val name = track.artist.trim()
                if (name.isNotBlank()) {
                    val key = name.lowercase()
                    displayNames.putIfAbsent(key, name)
                    scores[key] = (scores[key] ?: 0) + 12
                }
            }
        state.queue.forEach { track ->
            val name = track.artist.trim()
            if (name.isNotBlank()) {
                val key = name.lowercase()
                displayNames.putIfAbsent(key, name)
                scores[key] = (scores[key] ?: 0) + 4
            }
        }
        return scores.entries
            .sortedByDescending { it.value }
            .take(12)
            .mapNotNull { displayNames[it.key] }
    }

    private fun buildEndlessSeeds(queue: List<AndroidTrack>, currentIndex: Int): List<String> {
        val out = ArrayList<String>(3)
        val seenArtists = HashSet<String>()
        val start = currentIndex.coerceIn(0, (queue.size - 1).coerceAtLeast(0))
        for (i in start downTo 0) {
            val t = queue[i]
            if (t.id.isBlank() || t.id.startsWith("local:") || isSelfHostedId(t.id)) continue
            val key = t.artist.trim().lowercase()
            if (out.isEmpty() || key.isBlank() || seenArtists.add(key)) {
                if (!out.contains(t.id)) {
                    out.add(t.id)
                    if (key.isNotBlank()) seenArtists.add(key)
                }
                if (out.size >= 3) break
            }
        }
        if (out.isEmpty()) {
            _lastPlayedTracks.value.firstOrNull {
                it.id.isNotBlank() && !it.id.startsWith("local:") && !isSelfHostedId(it.id)
            }?.let { out.add(it.id) }
        }
        return out
    }

    private fun buildExcludeIds(queue: List<AndroidTrack>): List<String> {
        val excludes = LinkedHashSet<String>()
        queue.forEach { if (it.id.isNotBlank()) excludes.add(it.id) }
        excludes.addAll(playedTrackIdsInCurrentQueue)
        _lastPlayedTracks.value.take(40).forEach { if (it.id.isNotBlank()) excludes.add(it.id) }
        return excludes.take(120)
    }

    private suspend fun fetchEndlessContinuation(
        queue: List<AndroidTrack>,
        currentIndex: Int,
    ): List<AndroidTrack> {
        val seeds = buildEndlessSeeds(queue, currentIndex)
        if (seeds.isEmpty()) return emptyList()
        val existingIds = queue.mapTo(HashSet(queue.size)) { it.id }
        val seedKey = "${seeds.joinToString(",")}|${activeQueuePlaylistId.orEmpty()}"
        if (prefetchedAutoplaySeedKey == seedKey && prefetchedAutoplayTracks.isNotEmpty()) {
            val cached = prefetchedAutoplayTracks.filter { it.id !in existingIds }
            if (cached.isNotEmpty()) return cached
        }
        val engine = requireEngine()
        val excludes = buildExcludeIds(queue)
        val preferredArtists = buildPreferredArtists()
        val fetched = runSuspendCatching {
            engine.fetchEndlessRadioQueue(
                seedVideoIds = seeds,
                playlistId = activeQueuePlaylistId,
                excludeVideoIds = excludes,
                preferredArtists = preferredArtists,
            )
        }.getOrDefault(emptyList()).ifEmpty {
            runSuspendCatching { engine.fetchRadioQueue(seeds.first()) }
                .getOrDefault(emptyList())
        }.filter { it.id !in existingIds }
        if (fetched.isNotEmpty()) {
            prefetchedAutoplaySeedKey = seedKey
            prefetchedAutoplayTracks = fetched
        }
        return fetched
    }

    private fun maybePrefetchEndlessAutoplay(
        queue: List<AndroidTrack>,
        currentIndex: Int,
        generation: Long,
    ) {
        if (!settings.value.autoplayEnabled || _uiState.value.repeatMode != Player.REPEAT_MODE_OFF) return
        if (queue.isEmpty()) return
        val remainingUnplayed = if (_uiState.value.shuffleEnabled) {
            queue.count { it.id !in playedTrackIdsInCurrentQueue }
        } else {
            (queue.lastIndex - currentIndex).coerceAtLeast(0)
        }
        if (remainingUnplayed > 2) return
        val seeds = buildEndlessSeeds(queue, currentIndex)
        if (seeds.isEmpty()) return
        val seedKey = "${seeds.joinToString(",")}|${activeQueuePlaylistId.orEmpty()}"
        if (prefetchedAutoplaySeedKey == seedKey && prefetchedAutoplayTracks.isNotEmpty()) return
        autoplayPrefetchJob?.cancel()
        autoplayPrefetchJob = viewModelScope.launch(Dispatchers.IO) {
            delay(1_200L)
            if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
            fetchEndlessContinuation(queue, currentIndex)
        }
    }

    private fun triggerEndlessContinuation(
        state: SonataUiState,
        expectedGeneration: Long,
        stopWhenEmpty: Boolean,
    ) {
        val seed = state.activeTrack ?: return
        autoplayJob?.cancel()
        autoplayJob = viewModelScope.launch(Dispatchers.IO) {
            val radio = fetchEndlessContinuation(state.queue, state.queueIndex)
            if (!isActive || !isGenerationActive(expectedGeneration, seed.id)) return@launch
            if (radio.isNotEmpty()) {
                val extended = state.queue + radio
                prefetchedAutoplaySeedKey = null
                prefetchedAutoplayTracks = emptyList()
                playTrack(radio.first(), extended)
            } else if (stopWhenEmpty) {
                PlaybackAudioBridge.clearEndedTransitionSuppression()
                playbackClock.onSeek(0L)
                _uiState.update { it.copy(isPlaying = false) }
            }
        }
    }

    fun nextTrack() {
        val state = _uiState.value
        val nextIndex = if (state.shuffleEnabled && state.repeatMode == Player.REPEAT_MODE_OFF) {
            val unplayed = state.queue.indices.filter { idx ->
                idx != state.queueIndex && state.queue[idx].id !in playedTrackIdsInCurrentQueue
            }
            unplayed.takeIf { it.isNotEmpty() }?.let { pool -> pool[Random.nextInt(pool.size)] }
        } else {
            PlaybackEligibility.resolveNextIndex(
                queueSize = state.queue.size,
                queueIndex = state.queueIndex,
                repeatMode = state.repeatMode,
                shuffleEnabled = state.shuffleEnabled,
            )
        }
        if (nextIndex != null) {
            playTrack(state.queue[nextIndex], state.queue)
        } else if (settings.value.autoplayEnabled && state.activeTrack != null) {
            triggerEndlessContinuation(state, playbackGeneration.current, stopWhenEmpty = false)
        }
    }

    fun prevTrack() {
        val state = _uiState.value
        val currentPos = controller?.currentPosition ?: playbackClock.positionMs
        val prevIndex = PlaybackEligibility.resolvePreviousIndex(
            queueSize = state.queue.size,
            queueIndex = state.queueIndex,
            repeatMode = state.repeatMode,
            currentPositionMs = currentPos,
        )
        if (prevIndex == null) {
            seekTo(0L)
            return
        }
        playTrack(state.queue[prevIndex], state.queue)
    }

    private fun onTrackEnded(endedGeneration: Long = playbackGeneration.current) {
        if (endedGeneration > 0L) {
            if (!playbackGeneration.isCurrent(endedGeneration)) return
            if (lastHandledEndedGeneration == endedGeneration) return
            lastHandledEndedGeneration = endedGeneration
        }
        if (sleepAtEndOfTrack) {
            sleepAtEndOfTrack = false
            _sleepTimerRemainingMinutes.value = null
            PlaybackAudioBridge.clearEndedTransitionSuppression()
            controller?.pause()
            playbackClock.onSeek(0L)
            _uiState.update { it.copy(isPlaying = false, status = uiStrings().sleepTimerPaused) }
            return
        }
        val state = _uiState.value
        if (state.repeatMode == Player.REPEAT_MODE_ONE) {
            PlaybackAudioBridge.clearEndedTransitionSuppression()
            seekTo(0L)
            controller?.play()
            return
        }
        val hasPlayableNext = if (state.shuffleEnabled && state.repeatMode == Player.REPEAT_MODE_OFF) {
            state.queue.indices.any { idx ->
                idx != state.queueIndex && state.queue[idx].id !in playedTrackIdsInCurrentQueue
            }
        } else {
            state.hasNext
        }
        if (hasPlayableNext) {
            nextTrack()
        } else if (settings.value.autoplayEnabled && state.activeTrack != null) {
            triggerEndlessContinuation(state, endedGeneration, stopWhenEmpty = true)
        } else {
            PlaybackAudioBridge.clearEndedTransitionSuppression()
            playbackClock.onSeek(0L)
            _uiState.update { it.copy(isPlaying = false) }
        }
    }

    fun toggleRepeatMode() {
        val next = when (_uiState.value.repeatMode) {
            Player.REPEAT_MODE_OFF -> Player.REPEAT_MODE_ALL
            Player.REPEAT_MODE_ALL -> Player.REPEAT_MODE_ONE
            else -> Player.REPEAT_MODE_OFF
        }
        runCatching { controller?.repeatMode = next }
        _uiState.update { it.copy(repeatMode = next) }
        preferences.update { it.copy(repeatMode = next) }
    }

    fun toggleShuffle() {
        val next = !_uiState.value.shuffleEnabled
        _uiState.update { it.copy(shuffleEnabled = next) }
        preferences.update { it.copy(shuffleEnabled = next) }
    }

    fun toggleAutoplay() {
        val next = !preferences.state.value.autoplayEnabled
        preferences.update { it.copy(autoplayEnabled = next) }
        val s = uiStrings()
        _uiState.update { it.copy(status = if (next) s.autoplayOn else s.autoplayOff) }
    }

    fun togglePlayPause() {
        val ctrl = controller
        val active = _uiState.value.activeTrack
        val hasItem = runCatching { ctrl?.currentMediaItem != null }.getOrDefault(false)
        if (ctrl == null || !hasItem) {
            if (active != null) {
                playTrack(active, _uiState.value.queue)
            }
            return
        }
        runCatching {
            if (ctrl.isPlaying) {
                ctrl.pause()
            } else {
                if (ctrl.playbackState == Player.STATE_ENDED) {
                    ctrl.seekTo(0L)
                }
                ctrl.play()
            }
        }.onFailure { err ->
            Log.w("SonataStream", "togglePlayPause failed: ${err.message}")
        }
    }

    fun seekTo(positionMs: Long) {
        val clamped = positionMs.coerceAtLeast(0L)
        runCatching { controller?.seekTo(clamped) }
        playbackClock.onSeek(clamped)
        discordSeekSignal.value += 1L
    }

    private var updateDownloadJob: Job? = null
    private var dismissedUpdateVersion: String? = null

    fun checkForUpdates(autoPopup: Boolean = true, isManual: Boolean = true) {
        if (_uiState.value.checkingUpdates) return
        if (isManual) {
            dismissedUpdateVersion = null
        }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val installedVersion = runCatching {
                val app = getApplication<Application>()
                app.packageManager.getPackageInfo(app.packageName, 0).versionName.orEmpty()
            }.getOrDefault("").ifBlank { _uiState.value.installedVersionName }
            _uiState.update {
                it.copy(
                    checkingUpdates = true,
                    installedVersionName = installedVersion,
                    updateError = null,
                )
            }
            runSuspendCatching { engine.checkForUpdatesForVersion(installedVersion) }
                .onSuccess { info ->
                    val shouldShowPopup = autoPopup &&
                        info.updateAvailable &&
                        (isManual || dismissedUpdateVersion != info.latestVersion)
                    _uiState.update {
                        it.copy(
                            updateInfo = info,
                            checkingUpdates = false,
                            showUpdateDialog = it.showUpdateDialog || shouldShowPopup,
                        )
                    }
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(
                            checkingUpdates = false,
                            updateError = if (isManual) err.message ?: uiStrings().updateCheckFailed else it.updateError,
                        )
                    }
                }
        }
    }

    fun openUpdateDialog() {
        _uiState.update { it.copy(showUpdateDialog = true, updateError = null) }
    }

    fun dismissUpdateDialog() {
        val latest = _uiState.value.updateInfo?.latestVersion
        if (!latest.isNullOrBlank()) {
            dismissedUpdateVersion = latest
        }
        _uiState.update { it.copy(showUpdateDialog = false) }
    }

    fun cancelUpdateDownload() {
        updateDownloadJob?.cancel()
        updateDownloadJob = null
        _uiState.update {
            it.copy(
                updateDownloading = false,
                updateDownloadProgress = null,
                updateDownloadDetail = "",
                updateError = null,
            )
        }
    }

    fun downloadAndInstallUpdate() {
        val state = _uiState.value
        if (state.updateDownloading) return
        val app = getApplication<Application>()

        val s = uiStrings()
        // If already downloaded for this release, trigger installer directly.
        val existingPath = state.downloadedUpdateApkPath
        if (!existingPath.isNullOrBlank()) {
            val existingFile = java.io.File(existingPath)
            when (ApkUpdater.launchInstall(app, existingFile)) {
                ApkInstallLaunchResult.LAUNCHED -> {
                    _uiState.update {
                        it.copy(
                            updateDownloadDetail = s.updateOpeningInstaller,
                            updateError = null,
                        )
                    }
                    return
                }
                ApkInstallLaunchResult.PERMISSION_REQUIRED -> {
                    _uiState.update {
                        it.copy(
                            updateDownloadDetail = s.updatePermissionRequired,
                            updateError = null,
                        )
                    }
                    return
                }
                ApkInstallLaunchResult.FILE_MISSING -> {
                    _uiState.update { it.copy(downloadedUpdateApkPath = null) }
                }
            }
        }

        val info = state.updateInfo ?: return
        val apkUrl = info.apkDownloadUrl?.takeIf { it.isNotBlank() }
        if (apkUrl == null) {
            _uiState.update { it.copy(updateError = s.updateNoApkAsset) }
            return
        }

        _uiState.update {
            it.copy(
                showUpdateDialog = true,
                updateDownloading = true,
                updateDownloadProgress = null,
                updateDownloadDetail = s.updateStartingDownload,
                updateError = null,
            )
        }

        updateDownloadJob?.cancel()
        updateDownloadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val apkFile = ApkUpdater.downloadApk(
                    context = app,
                    urlString = apkUrl,
                    versionLabel = info.latestVersion,
                ) { fraction, downloaded, total ->
                    val detail = if (total > 0L) {
                        "${ApkUpdater.formatBytes(downloaded)} / ${ApkUpdater.formatBytes(total)}"
                    } else {
                        ApkUpdater.formatBytes(downloaded)
                    }
                    _uiState.update {
                        it.copy(
                            updateDownloadProgress = fraction,
                            updateDownloadDetail = detail,
                        )
                    }
                }
                val launchResult = kotlinx.coroutines.withContext(Dispatchers.Main) {
                    ApkUpdater.launchInstall(app, apkFile)
                }
                val curStrings = uiStrings()
                val detailMsg = when (launchResult) {
                    ApkInstallLaunchResult.LAUNCHED -> curStrings.updateInstallingVersion(info.latestVersion)
                    ApkInstallLaunchResult.PERMISSION_REQUIRED -> curStrings.updatePermissionRequired
                    ApkInstallLaunchResult.FILE_MISSING -> curStrings.updateApkMissingRetry
                }
                _uiState.update {
                    it.copy(
                        updateDownloading = false,
                        updateDownloadProgress = 1f,
                        updateDownloadDetail = detailMsg,
                        downloadedUpdateApkPath = apkFile.absolutePath,
                        status = detailMsg,
                    )
                }
            } catch (cancelled: kotlinx.coroutines.CancellationException) {
                throw cancelled
            } catch (err: Throwable) {
                Log.e("SonataUpdater", "APK download failed", err)
                _uiState.update {
                    it.copy(
                        updateDownloading = false,
                        updateDownloadProgress = null,
                        updateDownloadDetail = "",
                        updateError = err.message ?: uiStrings().downloadFailedShort,
                    )
                }
            }
        }
    }

    fun seekToFraction(fraction: Float) {
        val duration = playbackClock.durationMs
        if (duration > 0L) {
            seekTo((fraction.coerceIn(0f, 1f) * duration).toLong())
        }
    }

    fun toggleLyrics() {
        _uiState.update { it.copy(showLyrics = !it.showLyrics) }
    }

    fun setSuppressVideoAudioWarning(suppress: Boolean) {
        preferences.update { it.copy(suppressVideoAudioQualityWarning = suppress) }
    }

    fun setVideoFullscreen(fullscreen: Boolean) {
        _uiState.update {
            if (it.videoModeEnabled) it.copy(videoFullscreen = fullscreen) else it.copy(videoFullscreen = false)
        }
    }

    private fun probeMusicVideoForTrack(
        track: AndroidTrack,
        generation: Long = playbackGeneration.current,
    ) {
        musicVideoProbeJob?.cancel()
        val trackId = track.id.trim()
        if (trackId.isBlank() || trackId.startsWith("local:") || isSelfHostedId(trackId)) {
            _uiState.update { state ->
                if (isGenerationActive(generation, track.id)) {
                    state.copy(musicVideoCandidate = null)
                } else {
                    state
                }
            }
            return
        }
        val offlineVideo = downloadManager.localVideoFor(trackId)
        val offlineFallback = offlineVideo?.let {
            AndroidMusicVideoCandidate(
                videoId = trackId,
                title = track.title,
                thumbnailUrl = track.thumbnailUrl,
                durationSecs = track.durationSecs,
                syncSegments = it.syncSegments,
            )
        }
        if (offlineFallback != null) {
            _uiState.update { state ->
                if (isGenerationActive(generation, track.id)) {
                    state.copy(musicVideoCandidate = offlineFallback)
                } else {
                    state
                }
            }
        }
        val cached = musicVideoCandidateCache[trackId]?.takeIf {
            SystemClock.elapsedRealtime() - it.cachedAtMs < ARTIST_CACHE_TTL_MS
        }
        if (cached != null) {
            val resolvedCandidate = cached.candidate?.let { cand ->
                if (cand.syncSegments.isEmpty() && offlineVideo?.syncSegments?.isNotEmpty() == true) {
                    cand.copy(syncSegments = offlineVideo.syncSegments)
                } else {
                    cand
                }
            } ?: offlineFallback
            _uiState.update { state ->
                if (isGenerationActive(generation, track.id)) {
                    state.copy(musicVideoCandidate = resolvedCandidate)
                } else {
                    state
                }
            }
            return
        }
        musicVideoProbeJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val fetched = runSuspendCatching { engine.resolveMusicVideoCandidate(track) }.getOrNull()
            val candidate = fetched?.let { cand ->
                if (cand.syncSegments.isEmpty() && offlineVideo?.syncSegments?.isNotEmpty() == true) {
                    cand.copy(syncSegments = offlineVideo.syncSegments)
                } else {
                    cand
                }
            }
            if (candidate != null || offlineFallback == null) {
                musicVideoCandidateCache[trackId] = CachedVideoCandidate(
                    candidate = candidate,
                    cachedAtMs = SystemClock.elapsedRealtime(),
                )
            }
            val effectiveCandidate = candidate ?: offlineFallback
            if (isActive && isGenerationActive(generation, track.id)) {
                _uiState.update { state ->
                    if (isGenerationActive(generation, track.id)) {
                        state.copy(musicVideoCandidate = effectiveCandidate)
                    } else {
                        state
                    }
                }
            }
        }
    }

    fun enableVideoMode() {
        val state = _uiState.value
        val track = state.activeTrack ?: return
        val candidate = state.musicVideoCandidate ?: return
        val generation = playbackGeneration.current
        videoSwitchJob?.cancel()
        upgradeJob?.cancel()
        videoSwitchJob = viewModelScope.launch(Dispatchers.IO) {
            _uiState.update {
                if (isGenerationActive(generation, track.id)) {
                    it.copy(videoLoading = true, isUpgradingQuality = false, status = uiStrings().loadingVideo)
                } else {
                    it
                }
            }
            val offlineVideo = downloadManager.localVideoFor(track.id)
            val syncSegments = candidate.syncSegments.ifEmpty { offlineVideo?.syncSegments.orEmpty() }
            if (offlineVideo != null) {
                val activeMusicPosMs = withContext(Dispatchers.Main) {
                    controller?.currentPosition?.coerceAtLeast(0L) ?: playbackClock.positionMs
                }
                val startVideoPosMs = mapMusicToVideoMs(activeMusicPosMs, syncSegments)
                val shouldPlay = withContext(Dispatchers.Main) {
                    controller?.playWhenReady ?: _uiState.value.isPlaying
                }
                val offlineItem = buildOfflineMediaItem(track, offlineVideo.videoUri)
                val startedOffline = withContext(Dispatchers.Main) {
                    if (!isGenerationActive(generation, track.id)) {
                        false
                    } else {
                        PlaybackAudioBridge.playVideoStream(
                            videoUrl = offlineVideo.videoUri,
                            audioUrl = offlineVideo.audioUri,
                            mediaItem = offlineItem,
                            startPositionMs = startVideoPosMs,
                            playWhenReady = shouldPlay,
                        )
                    }
                }
                if (startedOffline) {
                    val offlineHeight = offlineVideo.qualityHeight.takeIf { it > 0 } ?: 720
                    val cachedQualities = resolvedMusicVideoCache[candidate.videoId]?.resolved?.qualities.orEmpty()
                    val qualities = if (cachedQualities.any { it.height.toInt() == offlineHeight }) {
                        cachedQualities
                    } else {
                        val syntheticOffline = AndroidVideoQualityStream(
                            qualityLabel = offlineVideo.qualityLabel,
                            height = offlineHeight.toUInt(),
                            fps = 30u,
                            videoUrl = offlineVideo.videoUri,
                            mimeType = "video/mp4",
                            bitrateBps = 0uL,
                            isMuxed = offlineVideo.audioUri == null,
                        )
                        (listOf(syntheticOffline) + cachedQualities).sortedByDescending { it.height.toInt() }
                    }
                    _uiState.update {
                        if (isGenerationActive(generation, track.id)) {
                            it.copy(
                                videoModeEnabled = true,
                                videoLoading = false,
                                availableVideoQualities = qualities,
                                activeVideoQualityLabel = offlineVideo.qualityLabel,
                                activeVideoQualityHeight = offlineHeight,
                                streamBadge = "Offline · ${offlineVideo.qualityLabel}",
                                status = "${uiStrings().videoMode} · Offline · ${offlineVideo.qualityLabel}",
                            )
                        } else {
                            it
                        }
                    }
                    return@launch
                }
            }
            val resolved = resolveOrFetchMusicVideo(candidate.videoId)
            if (!isActive || !isGenerationActive(generation, track.id)) return@launch
            if (resolved == null || resolved.qualities.isEmpty()) {
                _uiState.update {
                    if (isGenerationActive(generation, track.id)) {
                        it.copy(
                            videoLoading = false,
                            videoModeEnabled = false,
                            videoFullscreen = false,
                            status = uiStrings().videoUnavailableStatus,
                        )
                    } else {
                        it
                    }
                }
                return@launch
            }
            val selected = selectVideoQualityStream(
                qualities = resolved.qualities,
                preference = settings.value.videoQuality,
            )
            if (selected == null) {
                _uiState.update {
                    if (isGenerationActive(generation, track.id)) {
                        it.copy(
                            videoLoading = false,
                            videoModeEnabled = false,
                            videoFullscreen = false,
                            status = uiStrings().videoUnavailableStatus,
                        )
                    } else {
                        it
                    }
                }
                return@launch
            }
            val activeMusicPosMs = withContext(Dispatchers.Main) {
                controller?.currentPosition?.coerceAtLeast(0L) ?: playbackClock.positionMs
            }
            val startVideoPosMs = mapMusicToVideoMs(activeMusicPosMs, syncSegments)
            val shouldPlay = withContext(Dispatchers.Main) {
                controller?.playWhenReady ?: _uiState.value.isPlaying
            }
            val mediaItem = buildMediaItem(
                track = track,
                stream = AndroidResolvedStream(
                    url = selected.videoUrl,
                    badge = resolved.audioBadge,
                    headers = resolved.headers,
                ),
            )
            val audioForVideo = if (selected.isMuxed) null else resolved.audioUrl
            val badgeLabel = "${selected.qualityLabel} · ${resolved.audioBadge}".trim(' ', '·')
            val started = withContext(Dispatchers.Main) {
                if (!isGenerationActive(generation, track.id)) {
                    false
                } else {
                    PlaybackAudioBridge.playVideoStream(
                        videoUrl = selected.videoUrl,
                        audioUrl = audioForVideo,
                        mediaItem = mediaItem,
                        startPositionMs = startVideoPosMs,
                        playWhenReady = shouldPlay,
                    )
                }
            }
            if (!started) {
                _uiState.update {
                    if (isGenerationActive(generation, track.id)) {
                        it.copy(videoLoading = false, status = uiStrings().videoUnavailableStatus)
                    } else {
                        it
                    }
                }
                return@launch
            }
            _uiState.update {
                if (isGenerationActive(generation, track.id)) {
                    it.copy(
                        videoModeEnabled = true,
                        videoLoading = false,
                        availableVideoQualities = resolved.qualities,
                        activeVideoQualityLabel = selected.qualityLabel,
                        activeVideoQualityHeight = selected.height.toInt(),
                        streamBadge = badgeLabel,
                        status = "${uiStrings().videoMode} · ${selected.qualityLabel}",
                    )
                } else {
                    it
                }
            }
        }
    }

    fun selectVideoQuality(
        preference: VideoQualityPreference,
        explicitHeight: Int? = null,
    ) {
        preferences.update { it.copy(videoQuality = preference) }
        val state = _uiState.value
        if (!state.videoModeEnabled) return
        val track = state.activeTrack ?: return
        val candidate = state.musicVideoCandidate ?: return
        val generation = playbackGeneration.current
        videoSwitchJob?.cancel()
        videoSwitchJob = viewModelScope.launch(Dispatchers.IO) {
            _uiState.update {
                if (isGenerationActive(generation, track.id)) it.copy(videoLoading = true) else it
            }
            val offlineVideo = downloadManager.localVideoFor(track.id)
            val offlineMatches = offlineVideo != null &&
                (explicitHeight == null || (offlineVideo.qualityHeight > 0 && explicitHeight == offlineVideo.qualityHeight))
            if (offlineMatches && offlineVideo != null) {
                val activePosMs = withContext(Dispatchers.Main) {
                    controller?.currentPosition?.coerceAtLeast(0L) ?: playbackClock.positionMs
                }
                val shouldPlay = withContext(Dispatchers.Main) {
                    controller?.playWhenReady ?: _uiState.value.isPlaying
                }
                val offlineItem = buildOfflineMediaItem(track, offlineVideo.videoUri)
                val switchedOffline = withContext(Dispatchers.Main) {
                    if (isGenerationActive(generation, track.id)) {
                        PlaybackAudioBridge.playVideoStream(
                            videoUrl = offlineVideo.videoUri,
                            audioUrl = offlineVideo.audioUri,
                            mediaItem = offlineItem,
                            startPositionMs = activePosMs,
                            playWhenReady = shouldPlay,
                        )
                    } else {
                        false
                    }
                }
                if (switchedOffline && isActive && isGenerationActive(generation, track.id)) {
                    _uiState.update {
                        it.copy(
                            videoLoading = false,
                            activeVideoQualityLabel = offlineVideo.qualityLabel,
                            activeVideoQualityHeight = offlineVideo.qualityHeight,
                            streamBadge = "Offline · ${offlineVideo.qualityLabel}",
                            status = "${uiStrings().videoMode} · Offline · ${offlineVideo.qualityLabel}",
                        )
                    }
                    return@launch
                }
            }
            val resolved = resolveOrFetchMusicVideo(candidate.videoId) ?: run {
                _uiState.update { it.copy(videoLoading = false) }
                return@launch
            }
            val selected = selectVideoQualityStream(
                qualities = resolved.qualities,
                preference = preference,
                explicitHeight = explicitHeight,
            ) ?: run {
                _uiState.update { it.copy(videoLoading = false) }
                return@launch
            }
            val activePosMs = withContext(Dispatchers.Main) {
                controller?.currentPosition?.coerceAtLeast(0L) ?: playbackClock.positionMs
            }
            val shouldPlay = withContext(Dispatchers.Main) {
                controller?.playWhenReady ?: _uiState.value.isPlaying
            }
            val mediaItem = buildMediaItem(
                track = track,
                stream = AndroidResolvedStream(
                    url = selected.videoUrl,
                    badge = resolved.audioBadge,
                    headers = resolved.headers,
                ),
            )
            val audioForVideo = if (selected.isMuxed) null else resolved.audioUrl
            val badgeLabel = "${selected.qualityLabel} · ${resolved.audioBadge}".trim(' ', '·')
            withContext(Dispatchers.Main) {
                if (isGenerationActive(generation, track.id)) {
                    PlaybackAudioBridge.playVideoStream(
                        videoUrl = selected.videoUrl,
                        audioUrl = audioForVideo,
                        mediaItem = mediaItem,
                        startPositionMs = activePosMs,
                        playWhenReady = shouldPlay,
                    )
                }
            }
            if (isActive && isGenerationActive(generation, track.id)) {
                _uiState.update {
                    it.copy(
                        videoLoading = false,
                        availableVideoQualities = resolved.qualities,
                        activeVideoQualityLabel = selected.qualityLabel,
                        activeVideoQualityHeight = selected.height.toInt(),
                        streamBadge = badgeLabel,
                        status = "${uiStrings().videoMode} · ${selected.qualityLabel}",
                    )
                }
            }
        }
    }

    fun disableVideoMode() {
        val state = _uiState.value
        val track = state.activeTrack
        if (!state.videoModeEnabled && !state.videoFullscreen) return
        val generation = playbackGeneration.current
        val candidateSyncSegments = state.musicVideoCandidate?.syncSegments.orEmpty()
        videoSwitchJob?.cancel()
        _uiState.update {
            it.copy(
                videoModeEnabled = false,
                videoFullscreen = false,
                videoLoading = track != null,
            )
        }
        if (track == null) {
            PlaybackAudioBridge.setVideoModePlayback(false)
            return
        }
        if (settings.value.animatedArtwork && _currentCanvas.value == null) {
            resolveCanvasForTrack(track, generation)
        }
        videoSwitchJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val (activeVideoPosMs, shouldPlay) = withContext(Dispatchers.Main) {
                val pos = controller?.currentPosition?.coerceAtLeast(0L) ?: playbackClock.positionMs
                val play = controller?.playWhenReady ?: _uiState.value.isPlaying
                pos to play
            }
            val offlineVideo = downloadManager.localVideoFor(track.id)
            val syncSegments = candidateSyncSegments.ifEmpty { offlineVideo?.syncSegments.orEmpty() }
            val mappedMusicPosMs = mapVideoToMusicMs(activeVideoPosMs, syncSegments)
            val offlineUri = downloadManager.localUriFor(track.id)
            if (offlineUri != null) {
                val offlineRecord = downloads.value.firstOrNull { it.videoId == track.id }
                val offlineBadge = offlineRecord?.codec?.takeIf { it.isNotBlank() }?.let { "Offline · $it" } ?: "Offline"
                val offlineItem = buildOfflineMediaItem(track, offlineUri)
                withContext(Dispatchers.Main) {
                    PlaybackAudioBridge.setVideoModePlayback(false)
                    if (isGenerationActive(generation, track.id)) {
                        controller?.apply {
                            setMediaItem(offlineItem, mappedMusicPosMs)
                            prepare()
                            playWhenReady = shouldPlay
                        }
                    }
                }
                _uiState.update {
                    if (isGenerationActive(generation, track.id)) {
                        it.copy(
                            videoLoading = false,
                            streamBadge = offlineBadge,
                            status = uiStrings().playingTrackOfflineStatus(track.title),
                        )
                    } else {
                        it
                    }
                }
                return@launch
            }
            val stream = getCachedStream(track.id)
                ?: runSuspendCatching {
                    if (track.id.startsWith("jiosaavn:")) {
                        val js = engine.resolveJiosaavnById(track.id) ?: error("JioSaavn stream unavailable")
                        AndroidResolvedStream(url = js.url, badge = js.badge, headers = emptyList())
                    } else {
                        engine.resolveStream(track)
                    }
                }.onSuccess { putCachedStream(track.id, it) }.getOrNull()

            if (!isActive || !isGenerationActive(generation, track.id) || stream == null) {
                PlaybackAudioBridge.setVideoModePlayback(false)
                _uiState.update { it.copy(videoLoading = false) }
                return@launch
            }
            val mediaItem = buildMediaItem(track, stream)
            withContext(Dispatchers.Main) {
                PlaybackAudioBridge.setVideoModePlayback(false)
                if (isGenerationActive(generation, track.id)) {
                    controller?.apply {
                        setMediaItem(mediaItem, mappedMusicPosMs)
                        prepare()
                        playWhenReady = shouldPlay
                    }
                }
            }
            _uiState.update {
                if (isGenerationActive(generation, track.id)) {
                    it.copy(
                        videoLoading = false,
                        streamBadge = stream.badge,
                        status = uiStrings().playingTrackStatus(track.title),
                    )
                } else {
                    it
                }
            }
            launchBackgroundQualityUpgrade(
                generation = generation,
                engine = engine,
                effectiveTrack = track,
                currentStream = stream,
            )
        }
    }

    private suspend fun resolveOrFetchMusicVideo(videoId: String): AndroidResolvedMusicVideo? {
        val now = SystemClock.elapsedRealtime()
        val cached = resolvedMusicVideoCache[videoId]?.takeIf { now - it.cachedAtMs < STREAM_CACHE_TTL_MS }
        if (cached != null) return cached.resolved
        val engine = requireEngine()
        val fetched = runSuspendCatching { engine.resolveMusicVideoStreams(videoId) }.getOrNull()
            ?: return null
        if (fetched.qualities.isNotEmpty()) {
            resolvedMusicVideoCache[videoId] = CachedResolvedMusicVideo(resolved = fetched, cachedAtMs = now)
        }
        return fetched
    }

    override fun onCleared() {
        PlaybackQueueBridge.reset()
        PlaybackAudioBridge.clearEndedTransitionSuppression()
        CanvasResolver.detachGateway()
        listeningHistoryJob?.cancel()
        listeningHistoryJob = null
        speculativePrefetchJob?.cancel()
        speculativePrefetchJob = null
        refreshJob?.cancel()
        refreshJob = null
        radioRefreshJob?.cancel()
        radioRefreshJob = null
        playJob?.cancel()
        playJob = null
        upgradeJob?.cancel()
        upgradeJob = null
        recoveryJob?.cancel()
        recoveryJob = null
        autoplayJob?.cancel()
        autoplayJob = null
        autoplayPrefetchJob?.cancel()
        autoplayPrefetchJob = null
        searchDebounceJob?.cancel()
        searchDebounceJob = null
        searchJob?.cancel()
        searchJob = null
        lyricsJob?.cancel()
        lyricsJob = null
        canvasJob?.cancel()
        canvasJob = null
        musicVideoProbeJob?.cancel()
        musicVideoProbeJob = null
        videoSwitchJob?.cancel()
        videoSwitchJob = null
        sleepTimerJob?.cancel()
        sleepTimerJob = null
        PlaybackAudioBridge.applySleepFadeMultiplier(1f)
        homeMoodChipJob?.cancel()
        homeMoodChipJob = null
        moodCategoryJob?.cancel()
        moodCategoryJob = null
        artistJob?.cancel()
        artistJob = null
        collectionJob?.cancel()
        collectionJob = null
        updateDownloadJob?.cancel()
        updateDownloadJob = null
        discordRpcClient.disconnect()
        cipherSolver.destroy()
        runCatching { controller?.release() }
        controller = null
        super.onCleared()
    }

    companion object {
        const val DOWNLOADS_PLAYLIST_ID = "local:downloads"
        private const val LOCAL_PLAYLISTS_PREFS_NAME = "sonata_local_playlists"
        private const val LOCAL_PLAYLISTS_KEY = "playlists_json"
        private const val LOCAL_PLAYLIST_PREFIX = "local:pl_"
        private const val LAST_PLAYED_PREFS_NAME = "sonata_last_played_history"
        private const val LAST_PLAYED_KEY = "tracks_json"
        private const val MAX_LAST_PLAYED_HISTORY = 50
        private const val ARTIST_CACHE_TTL_MS = 15L * 60L * 1000L
        private const val ARTIST_CACHE_MAX_ENTRIES = 20
        private const val MAX_RECOVERY_ATTEMPTS = 2
        private const val STREAM_CACHE_TTL_MS = 4L * 60L * 60L * 1000L
        private const val STREAM_CACHE_MAX_ENTRIES = 64
        private const val AUTO_REFRESH_COOLDOWN_MS = 18_000L
        private const val MANUAL_REFRESH_COOLDOWN_MS = 4_000L
        private const val RATE_LIMIT_BACKOFF_MS = 45_000L
        private val YEAR_REGEX = Regex("\\b(19|20)\\d{2}\\b")
        private val ITAG_REGEX = Regex("[?&]itag=(\\d+)")
        private val KBPS_REGEX = Regex("(\\d+)\\s*kbps")
        private val PAREN_FEAT_REGEX = Regex("\\((?:feat|ft|featuring)\\b[^)]*\\)")
        private val BRACKET_TAG_REGEX = Regex("\\[[^\\]]*\\]")
        private val INLINE_FEAT_REGEX = Regex("\\s+(?:feat|ft|featuring)\\.?\\s+.*$")
        private val ARTIST_SPLIT_REGEX = Regex("\\s*(?:,|&|\\bfeat\\.?\\b|\\bft\\.?\\b|\\bfeaturing\\b|•)\\s*")
        private val NON_ALNUM_REGEX = Regex("[^\\p{L}\\p{N}]+")

        internal fun extractPrimaryArtistName(rawArtist: String): String {
            val clean = rawArtist
                .replace(PAREN_FEAT_REGEX, " ")
                .replace(BRACKET_TAG_REGEX, " ")
                .replace(INLINE_FEAT_REGEX, " ")
                .trim()
            if (clean.isBlank()) return ""
            val lower = clean.lowercase()
            if (lower.startsWith("tyler, the creator") || lower.startsWith("earth, wind")) {
                return clean
            }
            val first = clean.split(Regex("\\s*,\\s*|\\s+&\\s+"), limit = 2).firstOrNull()?.trim().orEmpty()
            return first.ifBlank { clean }
        }

        internal fun canonicalTrackKey(track: AndroidTrack): String {
            val cleanTitle = track.title
                .lowercase()
                .replace(PAREN_FEAT_REGEX, " ")
                .replace(BRACKET_TAG_REGEX, " ")
                .replace(INLINE_FEAT_REGEX, " ")
                .replace(NON_ALNUM_REGEX, " ")
                .trim()
            val cleanArtist = track.artist
                .lowercase()
                .replace(PAREN_FEAT_REGEX, " ")
                .replace(BRACKET_TAG_REGEX, " ")
                .split(ARTIST_SPLIT_REGEX, limit = 2)
                .firstOrNull()
                .orEmpty()
                .replace(NON_ALNUM_REGEX, " ")
                .trim()
            return if (cleanTitle.isEmpty()) track.id else "$cleanTitle|$cleanArtist"
        }

        internal fun deduplicateTracks(tracks: List<AndroidTrack>): List<AndroidTrack> {
            if (tracks.size <= 1) return tracks.filter { it.id.isNotBlank() }
            val seenIds = HashSet<String>(tracks.size)
            val seenKeys = HashSet<String>(tracks.size)
            val out = ArrayList<AndroidTrack>(tracks.size)
            for (t in tracks) {
                if (t.id.isBlank()) continue
                val key = canonicalTrackKey(t)
                if (seenIds.add(t.id) && seenKeys.add(key)) {
                    out.add(t)
                }
            }
            return out
        }

        internal fun isExcludedShelfTitle(title: String): Boolean {
            val lower = title.trim().lowercase()
            return "forgotten favorite" in lower || "unutulan favori" in lower
        }

        internal fun normalizeAndMergeHomeShelves(rawShelves: List<AndroidHomeShelf>): List<AndroidHomeShelf> {
            val visible = rawShelves
                .filterNot { isExcludedShelfTitle(it.title) }
                .map { shelf ->
                    if (shelf.tracks.size > 1) {
                        val deduped = deduplicateTracks(shelf.tracks)
                        if (deduped.size == shelf.tracks.size) shelf else shelf.copy(tracks = deduped)
                    } else {
                        shelf
                    }
                }
            return mergeNewReleaseShelves(visible)
        }

        internal fun normalizeRadioMixShelves(rawMixes: List<AndroidHomeShelf>): List<AndroidHomeShelf> {
            val seenIds = HashSet<String>()
            val seenKeys = HashSet<String>()
            return rawMixes
                .filterNot { isExcludedShelfTitle(it.title) }
                .mapNotNull { shelf ->
                    val uniqueCards = shelf.cards.filter { card ->
                        val cid = card.id.removePrefix("VL").trim()
                        val key = card.dedupeContentKey()
                        val idFresh = cid.isBlank() || seenIds.add(cid)
                        val keyFresh = key == "|" || seenKeys.add(key)
                        idFresh && keyFresh
                    }
                    if (uniqueCards.isEmpty()) null else shelf.copy(cards = uniqueCards)
                }
        }

        internal fun parseArtistFromCollectionSubtitle(subtitle: String?): String? {
            val raw = subtitle?.trim()?.takeIf { it.isNotEmpty() } ?: return null
            val parts = raw.split('•', '·')
                .map { it.trim() }
                .filter { part ->
                    part.isNotEmpty() &&
                        !part.equals("Album", ignoreCase = true) &&
                        !part.equals("Single", ignoreCase = true) &&
                        !part.equals("EP", ignoreCase = true) &&
                        !part.equals("Playlist", ignoreCase = true) &&
                        !YEAR_REGEX.matches(part)
                }
            return parts.firstOrNull()
        }

        internal fun backfillCollectionTracks(
            rawTracks: List<AndroidTrack>,
            id: String,
            title: String,
            subtitle: String?,
            coverUrl: String?,
        ): List<AndroidTrack> {
            val fallbackArtist = parseArtistFromCollectionSubtitle(subtitle)
            val fallbackCover = coverUrl ?: rawTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            val albumBrowseId = id.takeIf {
                it.startsWith("MPRE") || it.startsWith("OLAK") || it.startsWith("VLOLAK")
            }?.removePrefix("VL")
            return rawTracks.map { track ->
                val resolvedArtist = if (
                    (track.artist.isBlank() || track.artist.equals("Unknown Artist", ignoreCase = true)) &&
                    fallbackArtist != null
                ) {
                    fallbackArtist
                } else {
                    track.artist
                }
                val resolvedThumb = track.thumbnailUrl?.takeIf { it.isNotBlank() } ?: fallbackCover
                val resolvedAlbum = track.album.takeIf { it.isNotBlank() } ?: title
                val resolvedAlbumId = track.albumId ?: albumBrowseId
                if (
                    resolvedArtist == track.artist &&
                    resolvedThumb == track.thumbnailUrl &&
                    resolvedAlbum == track.album &&
                    resolvedAlbumId == track.albumId
                ) {
                    track
                } else {
                    track.copy(
                        artist = resolvedArtist,
                        thumbnailUrl = resolvedThumb,
                        album = resolvedAlbum,
                        albumId = resolvedAlbumId,
                    )
                }
            }
        }

        internal fun stripRangeQueryParam(url: String): String {
            if (!url.contains("range=", ignoreCase = true)) return url
            return url
                .replace(Regex("([?&])range=[^&]*&?"), "$1")
                .trimEnd('&', '?')
        }

        internal fun parseBitrateKbps(badge: String): Int =
            Regex("(\\d{2,4})\\s*kbps", RegexOption.IGNORE_CASE)
                .find(badge)
                ?.groupValues
                ?.getOrNull(1)
                ?.toIntOrNull()
                ?: 0

        internal fun selectVideoQualityStream(
            qualities: List<AndroidVideoQualityStream>,
            preference: VideoQualityPreference,
            explicitHeight: Int? = null,
        ): AndroidVideoQualityStream? {
            if (qualities.isEmpty()) return null
            val sorted = qualities.sortedWith(
                compareByDescending<AndroidVideoQualityStream> { it.height.toInt() }
                    .thenByDescending { it.fps.toInt() }
                    .thenByDescending { it.bitrateBps },
            )
            if (explicitHeight != null && explicitHeight > 0) {
                return sorted.firstOrNull { it.height.toInt() == explicitHeight }
                    ?: sorted.firstOrNull { it.height.toInt() <= explicitHeight }
                    ?: sorted.lastOrNull()
            }
            val cap = preference.maxHeight ?: return sorted.firstOrNull()
            return sorted.firstOrNull { it.height.toInt() <= cap } ?: sorted.lastOrNull()
        }

        internal fun shouldUpgradeStreamBadge(badge: String, signedIn: Boolean): Boolean {
            val lower = badge.lowercase()
            if (lower.contains("extreme") ||
                lower.contains("hi-res") ||
                lower.contains("hifi") ||
                lower.contains("flac") ||
                lower.contains("alac")
            ) {
                return false
            }
            val kbps = KBPS_REGEX.find(lower)?.groupValues?.getOrNull(1)?.toIntOrNull()
            if (signedIn && (kbps == null || kbps < 240)) {
                return true
            }
            if (lower.contains("opus") || lower.contains("hq")) {
                return false
            }
            if (kbps != null) return kbps < 160
            return lower.contains("standard")
        }
    }
}
