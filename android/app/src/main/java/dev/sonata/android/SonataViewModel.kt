package dev.sonata.android

import dev.sonata.android.viewmodel.*
import android.app.Application
import android.content.ComponentName
import android.content.ContentUris
import android.content.Context
import android.content.SharedPreferences
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.MediaStore
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
import dev.sonata.android.data.LocalAudioTrack
import dev.sonata.android.data.LocalMusicLibrary
import dev.sonata.android.data.LocalMusicScanner
import dev.sonata.android.data.SmbClient
import dev.sonata.android.data.SmbProfileRegistry
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.data.formatMeasuredAudioBadge
import dev.sonata.android.data.isLocalTrackId
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.data.mergeStreamBadgeWithMeasurement
import dev.sonata.android.data.resolveYtmFallbackCoverUrl
import dev.sonata.android.data.stripServerLabelsFromBadge
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
import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.ConnectController
import dev.sonata.android.connect.ConnectPlaybackRouter
import dev.sonata.android.connect.ConnectUiState
import dev.sonata.android.connect.SharedPrefsConnectSecretStore
import dev.sonata.android.connect.SonataConnectClient
import dev.sonata.android.ui.components.PERSONAL_PLAYLIST_ID_PREFIX
import dev.sonata.android.ui.components.isEditableUserPlaylist
import dev.sonata.android.ui.components.normalizeCollectionId
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.i18n.ecosystemStrings
import dev.sonata.android.ui.i18n.localizeMoodTitle
import dev.sonata.android.ui.i18n.resolveSonataStrings
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.screens.ServerDiagnosticState
import dev.sonata.android.playback.LiveAutoDataProvider
import dev.sonata.android.playback.PlaybackEligibility
import dev.sonata.android.playback.PlaybackGenerationCounter
import dev.sonata.android.playback.RateLimitPolicy
import dev.sonata.android.playback.RecoveryBudget
import dev.sonata.android.playback.ScrobbleAndStatsTracker
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
import uniffi.sonata_android.AndroidLastFmConfig
import uniffi.sonata_android.AndroidListenBrainzConfig
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidMoodCategory
import uniffi.sonata_android.AndroidMoodChip
import uniffi.sonata_android.AndroidMusicVideoCandidate
import uniffi.sonata_android.AndroidPersonalPlaylist
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidResolvedMusicVideo
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidScrobbleConfig
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerLibrary
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSearchResult
import uniffi.sonata_android.AndroidStatsSummary
import uniffi.sonata_android.AndroidStatsWindow
import uniffi.sonata_android.AndroidVideoQualityStream
import uniffi.sonata_android.AndroidSolvedCipher
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import uniffi.sonata_android.SonataAndroidEngine
import java.io.File
import java.util.Locale
import java.util.UUID
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
                settings.allowFileAccess = false
                settings.allowContentAccess = false
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
    val kindLabel: String = "",
    val isAlbum: Boolean = false,
    val pinnedAtMs: Long = 0L,
)

@Immutable
data class SavedExternalPlaylist(
    val id: String,
    val title: String,
    val subtitle: String = "",
    val coverUrl: String? = null,
    val kindLabel: String = "",
    val savedAtMs: Long = 0L,
)

internal fun isSelfHostedId(id: String?): Boolean {
    if (id.isNullOrBlank()) return false
    return id.startsWith("jellyfin:") ||
        id.startsWith("subsonic:") ||
        id.startsWith("smb:") ||
        id.startsWith("webdav:")
}

internal fun selfHostedKindTag(id: String?): String? = when {
    id == null -> null
    id.startsWith("jellyfin:") -> "JFN"
    id.startsWith("subsonic:") -> "SUB"
    id.startsWith("smb:") -> "SMB"
    id.startsWith("webdav:") -> "DAV"
    else -> null
}

@Immutable
data class ServerSourceSummary(
    val serverId: String,
    val albums: List<AndroidHomeCard> = emptyList(),
    val playlists: List<AndroidPlaylist> = emptyList(),
    val artists: List<AndroidHomeCard> = emptyList(),
    val tracks: List<AndroidTrack> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

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
    val status: String = "",
    val authInitialized: Boolean = false,
    val signedIn: Boolean = false,
    val accountProfile: AndroidAccountProfile? = null,
    val showLoginSheet: Boolean = false,
    val showSettingsSheet: Boolean = false,
    val showServersSheet: Boolean = false,
    val showStatsScreen: Boolean = false,
    val showConnectSheet: Boolean = false,
    val servers: List<AndroidServerProfile> = emptyList(),
    val serverDiagnostics: Map<String, ServerDiagnosticState> = emptyMap(),
    val serverSourceSnapshots: Map<String, ServerSourceSummary> = emptyMap(),
    val serverSearchResult: AndroidServerSearchResult? = null,
    val serverSearchLoading: Boolean = false,
    val personalOnlyMode: Boolean = false,
    val localMusicEnabled: Boolean = true,
    val attributePersonalToYtm: Boolean = false,
    val localMusicLibrary: LocalMusicLibrary = LocalMusicLibrary(),
    val localMusicLoading: Boolean = false,
    val personalPlaylists: List<AndroidPersonalPlaylist> = emptyList(),
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
    val listTitle: String = "",
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
    val isRemoteConnectActive: Boolean = false,
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
    val gaplessEnabled: Boolean = true,
    val crossfadeEnabled: Boolean = false,
    val crossfadeSeconds: Int = 4,
    val statsWindow: AndroidStatsWindow = AndroidStatsWindow.WEEK,
    val statsSummary: AndroidStatsSummary? = null,
    val statsLoading: Boolean = false,
    val scrobbleConfig: AndroidScrobbleConfig = AndroidScrobbleConfig(
        lastfm = AndroidLastFmConfig(
            enabled = false,
            apiKey = "",
            apiSecret = "",
            sessionKey = "",
            username = "",
        ),
        listenbrainz = AndroidListenBrainzConfig(
            enabled = false,
            token = "",
            username = "",
        ),
    ),
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
        get() = isRemoteConnectActive || (
            queue.isNotEmpty() && (
                repeatMode == Player.REPEAT_MODE_ALL ||
                    (shuffleEnabled && queue.size > 1) ||
                    (queueIndex in 0 until queue.lastIndex)
                )
            )

    val hasPreviousInQueue: Boolean
        get() = activeTrack != null && (
            repeatMode == Player.REPEAT_MODE_ALL ||
                queueIndex > 0
            )

    fun hasPrevious(positionMs: Long): Boolean =
        isRemoteConnectActive || (activeTrack != null && (positionMs > 3_000L || hasPreviousInQueue))
}

class SonataViewModel(application: Application) : AndroidViewModel(application) {
    internal data class CachedStream(
        val stream: AndroidResolvedStream,
        val cachedAtMs: Long,
    )

    internal data class CachedArtistPage(
        val page: AndroidArtistPage,
        val cachedAtMs: Long,
    )

    internal data class CachedVideoCandidate(
        val candidate: AndroidMusicVideoCandidate?,
        val cachedAtMs: Long,
    )

    internal data class CachedResolvedMusicVideo(
        val resolved: AndroidResolvedMusicVideo,
        val cachedAtMs: Long,
    )

    private val engineDeferred = CompletableDeferred<SonataEngineGateway>()
    private val cipherSolver = WebViewJsCipherSolver(application)

    internal val streamCache = ConcurrentHashMap<String, CachedStream>()
    internal val artistPageCache = ConcurrentHashMap<String, CachedArtistPage>()
    private val inFlightPrefetch = ConcurrentHashMap.newKeySet<String>()
    internal val trackCollectionContext = ConcurrentHashMap<String, String>()
    internal val jioToYtmIdCache = ConcurrentHashMap<String, String>()
    internal val resolvedArtistCardCache = ConcurrentHashMap<String, AndroidHomeCard>()
    internal val inFlightArtistLookups = ConcurrentHashMap.newKeySet<String>()
    internal val musicVideoCandidateCache = ConcurrentHashMap<String, CachedVideoCandidate>()
    internal val resolvedMusicVideoCache = ConcurrentHashMap<String, CachedResolvedMusicVideo>()
    internal val baseStreamBadges = ConcurrentHashMap<String, String>()
    internal val activeCacheKeysByTrackId = ConcurrentHashMap<String, String>()
    internal val localContentUrisByTrackId = ConcurrentHashMap<String, String>()
    internal val localTracksById = ConcurrentHashMap<String, LocalAudioTrack>()
    @Volatile internal var personalPlaylistsCache: List<AndroidPersonalPlaylist> = emptyList()
    @Volatile internal var likedTracksCache: List<AndroidTrack> = emptyList()
    internal val autoPlaylistTracksCache = ConcurrentHashMap<String, List<AndroidTrack>>()
    internal var localScanJob: Job? = null
    internal val serverHealthJobs = ConcurrentHashMap<String, Job>()
    internal val serverSpeedJobs = ConcurrentHashMap<String, Job>()
    internal val serverRescanJobs = ConcurrentHashMap<String, Job>()
    internal var musicVideoProbeJob: Job? = null
    internal var videoSwitchJob: Job? = null
    internal var listeningHistoryJob: Job? = null
    internal val historyLock = Any()
    internal var localHistoryCacheKey: String? = null
    internal var localHistoryCache: List<AndroidTrack>? = null

    @OptIn(ExperimentalCoroutinesApi::class)
    private val prefetchDispatcher = Dispatchers.IO.limitedParallelism(1)

    val playbackClock = PlaybackClockState()

    val preferences = SonataPreferences(application)
    val downloadManager = SonataDownloadManager(application)

    val settings: StateFlow<SonataSettingsState> = preferences.state
    val downloads: StateFlow<List<DownloadedTrackRecord>> = downloadManager.downloads
    val activeDownloads: StateFlow<Map<String, ActiveDownload>> = downloadManager.activeDownloads
    val downloadsTotalBytes: StateFlow<Long> = downloadManager.totalBytes

    internal val _currentCanvas = MutableStateFlow<CanvasMedia?>(null)
    val currentCanvas: StateFlow<CanvasMedia?> = _currentCanvas.asStateFlow()

    val audioPipelineInfo: StateFlow<AudioPipelineInfo> = AudioPipelineBus.info

    internal val _sleepTimerRemainingMinutes = MutableStateFlow<Int?>(null)
    val sleepTimerRemainingMinutes: StateFlow<Int?> = _sleepTimerRemainingMinutes.asStateFlow()

    private val _activeLyricsSource = MutableStateFlow("")
    val activeLyricsSource: StateFlow<String> = _activeLyricsSource.asStateFlow()

    internal val _lastPlayedTracks = MutableStateFlow<List<AndroidTrack>>(emptyList())
    val lastPlayedTracks: StateFlow<List<AndroidTrack>> = _lastPlayedTracks.asStateFlow()

    internal val _uiState = MutableStateFlow(SonataUiState())
    val uiState: StateFlow<SonataUiState> = _uiState.asStateFlow()

    internal val _serverBrowserState = MutableStateFlow(ServerBrowserState())
    val serverBrowserState: StateFlow<ServerBrowserState> = _serverBrowserState.asStateFlow()

    internal var serverBrowseJob: Job? = null
    internal var serverTestJob: Job? = null
    internal var serverSearchJob: Job? = null

    internal val discordRpcClient = DiscordRpcClient()
    internal val discordSeekSignal = MutableStateFlow(0L)
    val discordStatus: StateFlow<DiscordRpcStatus> = discordRpcClient.status
    val discordConnected: StateFlow<Boolean> = preferences.discordToken
        .map { !it.isNullOrBlank() }
        .stateIn(viewModelScope, SharingStarted.Eagerly, !preferences.discordToken.value.isNullOrBlank())

    private val scrobbleAndStatsTracker = ScrobbleAndStatsTracker()
    internal var statsLoadJob: Job? = null
    private val connectDeviceId: String by lazy {
        prefs.getString("connect_device_id", null)?.takeIf { it.isNotBlank() }
            ?: UUID.randomUUID().toString().also { id ->
                prefs.edit().putString("connect_device_id", id).apply()
            }
    }
    internal data class SavedLocalPlaybackState(
        val activeTrack: AndroidTrack?,
        val queue: List<AndroidTrack>,
        val queueIndex: Int,
        val streamBadge: String,
        val positionMs: Long,
        val durationMs: Long,
    )

    internal val connectController: ConnectController by lazy {
        ConnectController(
            client = SonataConnectClient(localDeviceIdProvider = { connectDeviceId }),
            secretStore = SharedPrefsConnectSecretStore(application),
            scope = viewModelScope,
            deviceNameProvider = { preferences.state.value.connectDeviceName },
        )
    }
    internal val connectPlaybackRouter: ConnectPlaybackRouter by lazy {
        ConnectPlaybackRouter(connectController)
    }
    internal var savedLocalPlaybackForConnect: SavedLocalPlaybackState? = null
    internal var pendingRemoteTransferTrackId: String? = null
    internal var pendingRemoteTransferUntilMs: Long = 0L
    internal var pendingRemoteQueueIds: List<String>? = null
    internal var pendingRemoteQueueUntilMs: Long = 0L
    internal var lastMirroredConnectTrackId: String? = null

    val connectState: StateFlow<ConnectUiState> get() = connectController.state

    internal fun isRemoteConnectActive(): Boolean = connectPlaybackRouter.isRemoteActive

    internal fun pauseLocalPlaybackForConnect() {
        playJob?.cancel()
        upgradeJob?.cancel()
        recoveryJob?.cancel()
        autoplayJob?.cancel()
        autoplayPrefetchJob?.cancel()
        speculativePrefetchJob?.cancel()
        videoSwitchJob?.cancel()
        runCatching { controller?.pause() }
    }

    internal val playbackGeneration = PlaybackGenerationCounter()
    private val recoveryBudget = RecoveryBudget(MAX_RECOVERY_ATTEMPTS)
    @Volatile private var lastHandledEndedGeneration: Long = 0L

    internal var controller: MediaController? = null
    internal var playJob: Job? = null
    internal var upgradeJob: Job? = null
    internal var lyricsJob: Job? = null
    internal var canvasJob: Job? = null
    internal var sleepTimerJob: Job? = null
    internal var refreshJob: Job? = null
    internal var speculativePrefetchJob: Job? = null
    private var searchDebounceJob: Job? = null
    internal var searchJob: Job? = null
    private var recoveryJob: Job? = null
    internal var autoplayJob: Job? = null
    internal var autoplayPrefetchJob: Job? = null
    internal var artistJob: Job? = null
    internal var collectionJob: Job? = null
    private var lyricsFetchedForTrackId: String? = null
    private var activeQueuePlaylistId: String? = null
    private val playedTrackIdsInCurrentQueue = linkedSetOf<String>()
    @Volatile internal var prefetchedAutoplaySeedKey: String? = null
    @Volatile internal var prefetchedAutoplayTracks: List<AndroidTrack> = emptyList()
    @Volatile internal var sleepAtEndOfTrack: Boolean = false
    @Volatile private var cipherWarmupScheduled = false
    @Volatile internal var lastHomeExploreFetchAtMs: Long = 0L
    @Volatile internal var lastRadioFetchAtMs: Long = 0L
    @Volatile private var rateLimitBackoffUntilMs: Long = 0L
    @Volatile internal var homeFetchInFlight: Boolean = false
    private val homeFetchSequence = AtomicLong(0L)
    @Volatile internal var radioFetchInFlight: Boolean = false
    @Volatile private var lastAppliedLanguageCode: String? = null
    @Volatile internal var fullHomeShelves: List<AndroidHomeShelf> = emptyList()
    internal var radioRefreshJob: Job? = null
    internal var homeMoodChipJob: Job? = null
    internal var moodCategoryJob: Job? = null
    @Volatile internal var latestLikedCoverUrl: String? = null

    internal val prefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences("sonata_prefs", Context.MODE_PRIVATE)
    }

    internal val historyPrefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences(LAST_PLAYED_PREFS_NAME, Context.MODE_PRIVATE)
    }

    internal val localPlaylistsPrefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences(LOCAL_PLAYLISTS_PREFS_NAME, Context.MODE_PRIVATE)
    }

    internal val localPlaylistsLock = Any()
    internal val pinnedSavedLock = Any()
    internal var cachedPinnedCollections: List<PinnedCollectionEntry> = emptyList()
    internal var cachedSavedExternalPlaylists: List<SavedExternalPlaylist> = emptyList()

    internal data class LocalPlaylistEntry(
        val id: String,
        val name: String,
        val tracks: List<AndroidTrack>,
    )

    internal fun getLocalPlaylistTracks(playlistId: String): List<AndroidTrack> = getLocalPlaylistTracksImpl(playlistId)

    internal fun resolveTrackCoverUrl(track: AndroidTrack): String? = resolveTrackCoverUrlImpl(track)

    internal fun applyLatestLikedCover(playlists: List<AndroidPlaylist>): List<AndroidPlaylist> = applyLatestLikedCoverImpl(playlists)

    internal fun updateLatestLikedCover(coverUrl: String?, clearIfNull: Boolean = false) = updateLatestLikedCoverImpl(coverUrl, clearIfNull)

    internal fun computeOwnedPlaylistIds(
        remote: List<AndroidPlaylist>,
        savedExternalIds: Set<String>,
    ): Set<String> = computeOwnedPlaylistIdsImpl(remote, savedExternalIds)

    internal fun mergePlaylistsWithLocal(
        remote: List<AndroidPlaylist>,
        personalList: List<AndroidPersonalPlaylist> = personalPlaylistsCache,
    ): List<AndroidPlaylist> = mergePlaylistsWithLocalImpl(remote, personalList)

    internal fun initPinnedAndSavedFromPrefs() = initPinnedAndSavedFromPrefsImpl()

    internal fun buildRecentArtistCards(
        historyTracks: List<AndroidTrack>,
        shelves: List<AndroidHomeShelf>,
    ): List<AndroidHomeCard> = buildRecentArtistCardsImpl(historyTracks, shelves)

    internal fun loadLocalLastPlayedHistory(signedInOverride: Boolean? = null): List<AndroidTrack> =
        loadLocalLastPlayedHistoryImpl(signedInOverride)

    internal fun recordLastPlayed(track: AndroidTrack) = recordLastPlayedImpl(track)

    fun removeFromLastPlayed(track: AndroidTrack) = removeFromLastPlayedImpl(track)

    internal fun mergeYtmListeningHistory(ytmHistory: List<AndroidTrack>) = mergeYtmListeningHistoryImpl(ytmHistory)

    fun refreshListeningHistory() = refreshListeningHistoryImpl()

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

    internal suspend fun resolveYtmVideoIdForTrack(
        engine: SonataEngineGateway,
        track: AndroidTrack,
    ): String? {
        if (preferences.state.value.personalOnlyMode) return null
        val rawId = track.id.trim()
        if (rawId.isBlank() ||
            isLocalTrackId(rawId) ||
            isSelfHostedId(rawId) ||
            isPersonalTrackId(rawId) ||
            rawId.startsWith("personal:", ignoreCase = true)
        ) {
            return null
        }
        if (!rawId.startsWith("jiosaavn:", ignoreCase = true)) return rawId
        jioToYtmIdCache[rawId]?.let { return it }

        val query = listOf(track.title.trim(), track.artist.trim())
            .filter { it.isNotBlank() }
            .joinToString(" ")
        if (query.isBlank()) return null

        val match = runSuspendCatching { engine.searchSongs(query) }
            .getOrNull()
            ?.firstOrNull { cand ->
                cand.id.isNotBlank() &&
                    !cand.id.startsWith("jiosaavn:", ignoreCase = true) &&
                    !cand.id.startsWith("personal:", ignoreCase = true) &&
                    !isPersonalTrackId(cand.id)
            }
            ?.id
            ?.takeIf { it.isNotBlank() }

        if (match != null) {
            if (jioToYtmIdCache.size > 256) jioToYtmIdCache.clear()
            jioToYtmIdCache[rawId] = match
        }
        return match
    }

    internal suspend fun resolveYtmVideoIds(
        engine: SonataEngineGateway,
        tracks: List<AndroidTrack>,
    ): List<String> = tracks.mapNotNull { resolveYtmVideoIdForTrack(engine, it) }.distinct()

    private fun reportTrackPlaybackToYtm(track: AndroidTrack, playlistId: String?) {
        val currentSettings = settings.value
        if (currentSettings.personalOnlyMode || track.id.isBlank()) return
        if (isPersonalTrackId(track.id)) {
            viewModelScope.launch(Dispatchers.IO) {
                val engine = requireEngine()
                val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
                if (!shouldAttributePersonalPlay(
                        personalOnlyMode = currentSettings.personalOnlyMode,
                        attributePersonalToYtm = currentSettings.attributePersonalToYtm,
                        signedIn = signedIn,
                        trackId = track.id,
                    )
                ) {
                    return@launch
                }
                runSuspendCatching { engine.attributePersonalTrackToYtm(track) }
                    .onFailure { err ->
                        Log.w("SonataTelemetry", "Personal track YTM attribution failed: ${err.message}")
                    }
            }
            return
        }
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

    internal fun uiStrings(): SonataStrings =
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

    internal fun resolvedDownloadCoverUrl(track: AndroidTrack): String? =
        track.thumbnailUrl?.takeIf { it.isNotBlank() }
            ?: resolveYtmFallbackCoverUrl(
                trackId = track.id,
                personalOnlyMode = preferences.state.value.personalOnlyMode,
            )

    internal suspend fun requireEngine(): SonataEngineGateway = engineDeferred.await()

    internal fun getCachedStream(trackId: String): AndroidResolvedStream? {
        val entry = streamCache[trackId] ?: return null
        if (SystemClock.elapsedRealtime() - entry.cachedAtMs >= STREAM_CACHE_TTL_MS) {
            streamCache.remove(trackId, entry)
            return null
        }
        return entry.stream
    }

    internal fun putCachedStream(trackId: String, stream: AndroidResolvedStream) {
        val now = SystemClock.elapsedRealtime()
        streamCache[trackId] = CachedStream(stream = stream, cachedAtMs = now)
        if (streamCache.size > STREAM_CACHE_MAX_ENTRIES) {
            streamCache.entries
                .sortedBy { it.value.cachedAtMs }
                .take(streamCache.size - STREAM_CACHE_MAX_ENTRIES)
                .forEach { streamCache.remove(it.key, it.value) }
        }
    }

    // Must be declared before `init`, which calls `registerQueueBridge()`.
    internal val autoDataProvider: LiveAutoDataProvider by lazy {
        LiveAutoDataProvider(
            likedTracksProvider = {
                likedTracksCache.ifEmpty {
                    val state = _uiState.value
                    (state.tracks + state.lastPlayedTracks)
                        .filter { it.id in state.likedTrackIds }
                        .distinctBy { it.id }
                }
            },
            playlistsProvider = { _uiState.value.playlists },
            playlistTracksProvider = { playlistId ->
                when {
                    playlistId == DOWNLOADS_PLAYLIST_ID -> downloads.value.map { it.toAndroidTrack() }
                    playlistId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) ->
                        personalPlaylistsCache.firstOrNull { it.id == playlistId }?.tracks.orEmpty()
                    playlistId.startsWith(LOCAL_PLAYLIST_PREFIX) -> getLocalPlaylistTracks(playlistId)
                    else -> autoPlaylistTracksCache[playlistId].orEmpty()
                }
            },
            downloadedTracksProvider = { downloads.value.map { it.toAndroidTrack() } },
            localTracksProvider = { _uiState.value.localMusicLibrary.tracks.map { it.toAndroidTrack() } },
            recentTracksProvider = { _lastPlayedTracks.value },
            fallbackTracksProvider = { _uiState.value.tracks + _uiState.value.collectionTracks },
        )
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
            listTitle = uiStrings().quickPicks,
            repeatMode = initialSettings.repeatMode,
            shuffleEnabled = initialSettings.shuffleEnabled,
            preferHighQuality = initialSettings.preferHighQuality,
            animatedArtwork = initialSettings.animatedArtwork,
            gaplessEnabled = initialSettings.gaplessEnabled,
            crossfadeEnabled = initialSettings.crossfadeEnabled,
            crossfadeSeconds = initialSettings.crossfadeSeconds,
            liquidGlassEnabled = initialSettings.liquidGlassEnabled,
            fpsLimit = resolvedFps,
            supports120Hz = supports120Hz,
        )

        applyAudioSettings(initialSettings)
        SmbClient.initCacheDir(application.cacheDir)
        viewModelScope.launch(Dispatchers.IO) {
            val loadedSettings = preferences.ensureLoadedState()
            preferences.hasDiscordToken()
            connectDeviceId
            connectController.setEnabled(loadedSettings.connectEnabled)
        }
        observePreferences(supports120Hz)
        observeAudioPipelineTelemetry()
        observeDiscordPresence()
        observeConnectRemoteStateImpl()
        registerQueueBridge()
        startPositionPoller()
        startColdLaunchBootstrap(application, supports120Hz)
    }

    internal fun resolveDisplayBadgeForTrack(
        trackId: String,
        initialBadge: String,
    ): String {
        val cleanInitial = stripServerLabelsFromBadge(initialBadge)
        val pipeline = AudioPipelineBus.info.value
        if (pipeline.mediaId != trackId) return cleanInitial
        val measured = pipeline.measuredBadge.ifBlank {
            formatMeasuredAudioBadge(
                codec = pipeline.codec,
                bitrateKbps = pipeline.bitrateKbps,
                sampleRateHz = pipeline.sampleRateHz,
                bitDepthBits = pipeline.bitDepthBits,
            )
        }
        if (measured.isBlank()) return cleanInitial
        return mergeStreamBadgeWithMeasurement(
            initialBadge = cleanInitial,
            measuredBadge = measured,
            sourceLabel = pipeline.sourceLabel,
            measuredBitrateKbps = pipeline.bitrateKbps,
        )
    }

    private fun observeAudioPipelineTelemetry() {
        viewModelScope.launch {
            AudioPipelineBus.info.collect { pipeline ->
                val currentState = _uiState.value
                val activeTrack = currentState.activeTrack ?: return@collect
                if (currentState.videoModeEnabled) return@collect
                if (pipeline.mediaId.isNotBlank() && pipeline.mediaId != activeTrack.id) return@collect
                val measured = pipeline.measuredBadge.ifBlank {
                    formatMeasuredAudioBadge(
                        codec = pipeline.codec,
                        bitrateKbps = pipeline.bitrateKbps,
                        sampleRateHz = pipeline.sampleRateHz,
                        bitDepthBits = pipeline.bitDepthBits,
                    )
                }
                if (measured.isBlank()) return@collect
                val baseBadge = baseStreamBadges[activeTrack.id] ?: currentState.streamBadge
                val merged = mergeStreamBadgeWithMeasurement(
                    initialBadge = baseBadge,
                    measuredBadge = measured,
                    sourceLabel = pipeline.sourceLabel,
                    measuredBitrateKbps = pipeline.bitrateKbps,
                )
                if (merged.isNotBlank() && merged != currentState.streamBadge) {
                    var updated = false
                    _uiState.update { state ->
                        if (state.activeTrack?.id == activeTrack.id && !state.videoModeEnabled && state.streamBadge != merged) {
                            updated = true
                            state.copy(streamBadge = merged)
                        } else {
                            state
                        }
                    }
                    if (updated) {
                        withContext(Dispatchers.IO) {
                            saveActiveTrackToPrefs(activeTrack, merged)
                        }
                    }
                }
            }
        }
    }

    internal fun observeDiscordPresence() = observeDiscordPresenceImpl()

    fun saveDiscordToken(token: String) = saveDiscordTokenImpl(token)

    fun signOutDiscord() = signOutDiscordImpl()

    fun reconnectDiscord() = reconnectDiscordImpl()

    internal fun applyAudioSettings(state: SonataSettingsState) {
        PlaybackAudioBridge.applyPlaybackSpeed(state.playbackSpeed)
        PlaybackAudioBridge.applySkipSilence(state.skipSilenceEnabled)
        PlaybackAudioBridge.applyEqualizer(state.eqEnabled, state.eqBandsDb)
        PlaybackAudioBridge.applyLoudnessNormalization(state.loudnessNormEnabled)
        PlaybackAudioBridge.applyGapless(state.gaplessEnabled)
        val effectiveCrossfade = state.crossfadeEnabled && !_uiState.value.videoModeEnabled
        PlaybackAudioBridge.applyCrossfade(effectiveCrossfade, state.crossfadeSeconds)
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
                        ui.gaplessEnabled == prefsState.gaplessEnabled &&
                        ui.crossfadeEnabled == prefsState.crossfadeEnabled &&
                        ui.crossfadeSeconds == prefsState.crossfadeSeconds &&
                        ui.liquidGlassEnabled == prefsState.liquidGlassEnabled &&
                        ui.repeatMode == prefsState.repeatMode &&
                        ui.shuffleEnabled == prefsState.shuffleEnabled &&
                        ui.fpsLimit == resolvedFps &&
                        ui.personalOnlyMode == prefsState.personalOnlyMode &&
                        ui.localMusicEnabled == prefsState.localMusicEnabled &&
                        ui.attributePersonalToYtm == prefsState.attributePersonalToYtm
                    ) {
                        ui
                    } else {
                        ui.copy(
                            preferHighQuality = prefsState.preferHighQuality,
                            animatedArtwork = prefsState.animatedArtwork,
                            gaplessEnabled = prefsState.gaplessEnabled,
                            crossfadeEnabled = prefsState.crossfadeEnabled,
                            crossfadeSeconds = prefsState.crossfadeSeconds,
                            liquidGlassEnabled = prefsState.liquidGlassEnabled,
                            repeatMode = prefsState.repeatMode,
                            shuffleEnabled = prefsState.shuffleEnabled,
                            fpsLimit = resolvedFps,
                            personalOnlyMode = prefsState.personalOnlyMode,
                            localMusicEnabled = prefsState.localMusicEnabled,
                            attributePersonalToYtm = prefsState.attributePersonalToYtm,
                        )
                    }
                }
                val previous = prevPrefs
                if (previous != null && previous.connectEnabled != prefsState.connectEnabled) {
                    connectController.setEnabled(prefsState.connectEnabled)
                }
                if (previous != null && previous.connectDeviceName != prefsState.connectDeviceName) {
                    connectController.updateDeviceName(prefsState.connectDeviceName)
                }
                if (previous != null && (
                        previous.localMusicEnabled != prefsState.localMusicEnabled ||
                            previous.excludeNonMusicAudio != prefsState.excludeNonMusicAudio ||
                            previous.localMusicMinDurationSecs != prefsState.localMusicMinDurationSecs ||
                            previous.localMusicFolders != prefsState.localMusicFolders
                        )
                ) {
                    if (prefsState.localMusicEnabled) {
                        scanLocalMusic()
                    } else {
                        localScanJob?.cancel()
                        localScanJob = null
                        localContentUrisByTrackId.clear()
                        localTracksById.clear()
                        _uiState.update {
                            it.copy(
                                localMusicLibrary = LocalMusicLibrary(
                                    permissionGranted = LocalMusicScanner.hasAudioReadPermission(getApplication()),
                                ),
                                localMusicLoading = false,
                            )
                        }
                    }
                }
                if (previous != null && previous.personalOnlyMode != prefsState.personalOnlyMode) {
                    onPersonalOnlyModeChanged(prefsState.personalOnlyMode)
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
                    old.gaplessEnabled != prefsState.gaplessEnabled ||
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
            val initialPersonalOnly = preferences.state.value.personalOnlyMode
            val restoredTrack = loadSavedActiveTrack(emptyList())?.takeIf {
                !initialPersonalOnly || isPersonalTrackId(it.id)
            }
            val savedBadge = if (restoredTrack != null) prefs.getString("last_track_badge", null).orEmpty() else ""
            latestLikedCoverUrl = prefs.getString("last_liked_cover_url", null)?.takeIf { it.isNotBlank() }
            val initialQueue = listOfNotNull(restoredTrack)
            val initialQueueIndex = if (restoredTrack != null) 0 else -1

            if (restoredTrack != null) {
                playbackClock.resetForTrack((restoredTrack.durationSecs?.toLong() ?: 0L) * 1000L)
                if (!initialPersonalOnly) {
                    resolveCanvasForTrack(restoredTrack)
                    probeMusicVideoForTrack(restoredTrack)
                }
            }
            _uiState.update {
                it.copy(
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
            val cachedPlaylists = if (signedIn && !preferences.state.value.personalOnlyMode) {
                runSuspendCatching { engine.getCachedLibraryPlaylists() }.getOrDefault(emptyList())
            } else {
                emptyList()
            }
            val personalList = runSuspendCatching { engine.listPersonalPlaylists() }.getOrDefault(emptyList())
            personalPlaylistsCache = personalList
            val savedExternalIds = _uiState.value.savedExternalPlaylistIds
            val ownedIds = if (signedIn) computeOwnedPlaylistIds(cachedPlaylists, savedExternalIds) else emptySet()
            val mergedCachedPlaylists = mergePlaylistsWithLocal(cachedPlaylists, personalList)
            val initialServers = runSuspendCatching { engine.listServers() }.getOrDefault(emptyList())
            SmbProfileRegistry.replaceAll(initialServers)
            val initialScrobbleCfg = runSuspendCatching { engine.getScrobbleConfig() }.getOrNull()
            val initialStatsSummary = runSuspendCatching {
                engine.queryListeningStats(_uiState.value.statsWindow)
            }.getOrNull()
            val savedHistory = loadLocalLastPlayedHistory(signedInOverride = signedIn).let { hist ->
                if (preferences.state.value.personalOnlyMode) hist.filter { isPersonalTrackId(it.id) } else hist
            }
            _lastPlayedTracks.value = savedHistory

            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = signedIn,
                    installedVersionName = installedVer,
                    accountProfile = cachedProfile ?: it.accountProfile,
                    ownedPlaylistIds = if (it.ownedPlaylistIds.isEmpty()) ownedIds else it.ownedPlaylistIds,
                    personalPlaylists = personalList,
                    playlists = if (it.playlists.isEmpty()) mergedCachedPlaylists else it.playlists,
                    lastPlayedTracks = savedHistory,
                    servers = initialServers,
                    scrobbleConfig = initialScrobbleCfg ?: it.scrobbleConfig,
                    statsSummary = initialStatsSummary ?: it.statsSummary,
                    personalOnlyMode = preferences.state.value.personalOnlyMode,
                    localMusicEnabled = preferences.state.value.localMusicEnabled,
                    attributePersonalToYtm = preferences.state.value.attributePersonalToYtm,
                )
            }

            if (preferences.state.value.localMusicEnabled) {
                scanLocalMusic()
            }

            // 4. Immediately fetch fresh live Home shelves (unless Personal Only Mode is enabled).
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

    internal fun isGenerationActive(generation: Long, trackId: String? = null): Boolean {
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

    internal fun fetchLyricsInBackground(
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
            val fetched = if (track.id.startsWith("smb:")) {
                val parsed = SmbClient.parseTrackId(track.id)
                val smbProfile = parsed?.let { (srvId, _) ->
                    SmbProfileRegistry.get(srvId)
                        ?: _uiState.value.servers.firstOrNull { it.id == srvId }
                }
                val serverLyrics = if (parsed != null && smbProfile != null) {
                    runSuspendCatching { SmbClient.fetchLyrics(smbProfile, parsed.second) }
                        .getOrDefault(emptyList())
                } else {
                    emptyList()
                }
                when {
                    serverLyrics.isNotEmpty() -> serverLyrics
                    preferences.state.value.personalOnlyMode -> emptyList()
                    else -> runSuspendCatching { engine.fetchLyrics(track) }.getOrDefault(emptyList())
                }
            } else if (track.id.startsWith("webdav:") || track.id.startsWith("jellyfin:")) {
                if (preferences.state.value.personalOnlyMode) {
                    runSuspendCatching { engine.fetchServerLyrics(track) }.getOrDefault(emptyList())
                } else {
                    runSuspendCatching { engine.fetchLyrics(track) }.getOrDefault(emptyList())
                }
            } else if (preferences.state.value.personalOnlyMode && isPersonalTrackId(track.id)) {
                emptyList()
            } else {
                runSuspendCatching { engine.fetchLyrics(track) }.getOrDefault(emptyList())
            }
            if (isActive && isGenerationActive(generation, track.id)) {
                val isSynced = fetched.isNotEmpty() &&
                    fetched.none { it.timeSecs < 0.0 } &&
                    fetched.any { it.timeSecs > 0.0 || it.words.any { w -> w.startSecs > 0.0 } } &&
                    (fetched.size < 2 || fetched.map { (it.timeSecs * 100.0).toLong() }.distinct().size >= 2)
                val s = uiStrings()
                _activeLyricsSource.value = when {
                    isSynced && fetched.any { it.words.isNotEmpty() } -> s.wordSynced
                    isSynced -> s.syncedLabel
                    fetched.isNotEmpty() -> s.lyrics
                    else -> s.unavailableLabel
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

    fun playAutoMediaId(mediaId: String) = playAutoMediaIdImpl(mediaId)

    private fun registerQueueBridge() {
        PlaybackAudioBridge.registerAutoDataProvider(autoDataProvider)
        PlaybackAudioBridge.registerAutoStringsProvider { ecosystemStrings(preferences.state.value.appLanguage) }
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
        PlaybackQueueBridge.onPlayMediaId = { mediaId -> playAutoMediaId(mediaId) }
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

    internal fun handleRateLimitIfNeeded(err: Throwable) {
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
        if (preferences.state.value.personalOnlyMode) {
            homeFetchInFlight = false
            val localOnly = mergePlaylistsWithLocal(emptyList())
            _uiState.update {
                it.copy(
                    loading = false,
                    refreshing = false,
                    authInitialized = true,
                    shelves = emptyList(),
                    exploreShelves = emptyList(),
                    moodCategories = emptyList(),
                    homeMoodChips = emptyList(),
                    radioMixShelves = emptyList(),
                    playlists = localOnly,
                    ownedPlaylistIds = emptySet(),
                    status = sourcesStrings(preferences.state.value.appLanguage).personalOnlyModeBadge,
                )
            }
            scanLocalMusic()
            return
        }
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
                                    likedTracksCache = likedTracks
                                    autoPlaylistTracksCache["VLLM"] = likedTracks
                                    autoPlaylistTracksCache["LM"] = likedTracks
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
        if (preferences.state.value.personalOnlyMode) {
            radioFetchInFlight = false
            return
        }
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
                    if (preferences.state.value.personalOnlyMode && !isPersonalTrackId(currentItem.mediaId)) {
                        runCatching {
                            ctrl.stop()
                            ctrl.clearMediaItems()
                        }
                    } else {
                        val meta = currentItem.mediaMetadata
                        val liveTrack = _uiState.value.activeTrack?.takeIf { it.id == currentItem.mediaId }
                            ?: AndroidTrack(
                                id = currentItem.mediaId,
                                title = meta.title?.toString().orEmpty().ifBlank { uiStrings().trackLabel },
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
                        if (_currentCanvas.value == null && !preferences.state.value.personalOnlyMode) {
                            resolveCanvasForTrack(liveTrack)
                        }
                        if (_uiState.value.musicVideoCandidate == null && !preferences.state.value.personalOnlyMode) {
                            probeMusicVideoForTrack(liveTrack)
                        }
                    }
                }
                ctrl.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(isPlaying: Boolean) {
                        if (isRemoteConnectActive()) return
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
                        if (isRemoteConnectActive()) return
                        val state = _uiState.value
                        val activeTrack = state.activeTrack
                        val activeId = activeTrack?.id
                        val matchesActive = activeId != null && ctrl.currentMediaItem?.mediaId == activeId
                        if (matchesActive) {
                            val readyDurationMs = ctrl.duration.coerceAtLeast(0L)
                            val readyDurationSecs = (readyDurationMs / 1000L).takeIf { it > 0L }?.toULong()
                            val prevDurationSecs = activeTrack.durationSecs
                            val durationShifted = readyDurationSecs != null && (
                                prevDurationSecs == null ||
                                    prevDurationSecs == 0uL ||
                                    kotlin.math.abs(readyDurationSecs.toLong() - prevDurationSecs.toLong()) > 3L
                                )
                            val trackWithDuration = if (
                                playbackState == Player.STATE_READY && durationShifted
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
                                if (lyricsFetchedForTrackId != activeId || durationShifted) {
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
                        if (isRemoteConnectActive()) return
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
                        val rawCacheKey = activeCacheKeysByTrackId.remove(active.id)
                        Log.w(
                            "SonataStream",
                            "Recovering track=${active.id} gen=$generation attempt=$attempt posMs=$resumePositionMs cause=${error.errorCodeName}"
                        )
                        recoveryJob?.cancel()
                        upgradeJob?.cancel()
                        recoveryJob = viewModelScope.launch(Dispatchers.IO) {
                            val engine = requireEngine()
                            streamCache.remove(active.id)
                            if (rawCacheKey != null) {
                                PlaybackService.evictCacheKey(rawCacheKey)
                            }
                            if (oldBadge.isNotBlank()) {
                                PlaybackService.evictCacheKey("${active.id}:$oldBadge")
                            }
                            _uiState.update {
                                if (isGenerationActive(generation, active.id)) {
                                    it.copy(isBuffering = true, status = uiStrings().recoveringStreamStatus)
                                } else it
                            }
                            val recoveryResult = if (active.id.startsWith("smb:")) {
                                val parsedSmb = SmbClient.parseTrackId(active.id)
                                if (parsedSmb == null) {
                                    Result.failure(IllegalArgumentException("Invalid SMB track ID"))
                                } else {
                                    _uiState.value.servers.firstOrNull { it.id == parsedSmb.first }
                                        ?.let { SmbProfileRegistry.put(it) }
                                    Result.success(
                                        AndroidResolvedStream(
                                            url = SmbClient.buildStreamUri(parsedSmb.first, parsedSmb.second),
                                            badge = SmbClient.initialBadgeForTrack(parsedSmb.first, parsedSmb.second),
                                            headers = emptyList(),
                                        ),
                                    )
                                }
                            } else {
                                runSuspendCatching { engine.resolveRecovery(active) }
                            }
                            recoveryResult
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
                                                    baseStreamBadges[active.id] = recovered.badge
                                                    val displayBadge = resolveDisplayBadgeForTrack(active.id, recovered.badge)
                                                    _uiState.update {
                                                        it.copy(
                                                            streamBadge = displayBadge,
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
                                        val finalBadge = _uiState.value.takeIf { it.activeTrack?.id == active.id }?.streamBadge
                                            ?: recovered.badge
                                        saveActiveTrackToPrefs(active, finalBadge)
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
                    var lastTickMs = SystemClock.elapsedRealtime()
                    while (isActive) {
                        val ctrl = controller
                        val activeTrack = _uiState.value.activeTrack
                        val activeId = activeTrack?.id
                        val nowMs = SystemClock.elapsedRealtime()
                        val deltaMs = (nowMs - lastTickMs).coerceIn(0L, 1_000L)
                        lastTickMs = nowMs
                        if (isRemoteConnectActive()) {
                            if (connectController.state.value.remoteState?.isPlaying == true) {
                                val dur = playbackClock.durationMs
                                val nextPos = (playbackClock.positionMs + deltaMs).let { pos ->
                                    if (dur > 0L) pos.coerceAtMost(dur) else pos
                                }
                                playbackClock.sync(positionMs = nextPos, durationMs = dur)
                            }
                            delay(200L)
                            continue
                        }
                        if (ctrl != null &&
                            activeTrack != null &&
                            activeId != null &&
                            ctrl.currentMediaItem?.mediaId == activeId &&
                            ctrl.playbackState == Player.STATE_READY &&
                            ctrl.isPlaying
                        ) {
                            playbackClock.sync(
                                positionMs = ctrl.currentPosition.coerceAtLeast(0L),
                                durationMs = ctrl.duration.coerceAtLeast(0L),
                            )
                            val gen = playbackGeneration.current
                            scrobbleAndStatsTracker.consumeNowPlaying(gen)?.let { startedTrack ->
                                dispatchScrobbleNowPlaying(startedTrack)
                            }
                            val outcome = scrobbleAndStatsTracker.advance(gen, deltaMs)
                            val scrobbleTs = outcome.scrobbleTriggerTimestampUnix
                            val playedSecs = outcome.statsPlayedSecs
                            val triggeredTrack = outcome.track
                            if (triggeredTrack != null && scrobbleTs != null) {
                                dispatchScrobbleSubmit(triggeredTrack, scrobbleTs.toULong())
                            }
                            if (triggeredTrack != null && playedSecs != null) {
                                dispatchRecordListeningStat(
                                    track = triggeredTrack,
                                    sourceTag = outcome.sourceTag,
                                    playedSecs = playedSecs.toULong(),
                                    playedAtUnix = scrobbleTs?.toULong(),
                                )
                            }
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

    fun openServersScreen(initialServerId: String? = null) = openServersScreenImpl(initialServerId)

    fun dismissServersScreen() = dismissServersScreenImpl()

    internal fun onPersonalOnlyModeChanged(enabled: Boolean) = onPersonalOnlyModeChangedImpl(enabled)

    fun scanLocalMusic() = scanLocalMusicImpl()

    internal fun resolveLocalContentUri(trackId: String): String? = resolveLocalContentUriImpl(trackId)

    internal fun resolveLocalInitialBadge(trackId: String): String = resolveLocalInitialBadgeImpl(trackId)

    fun probeServerHealth(serverId: String) = probeServerHealthImpl(serverId)

    fun runServerSpeedTest(serverId: String) = runServerSpeedTestImpl(serverId)

    fun rescanServerSource(serverId: String) = rescanServerSourceImpl(serverId)

    fun loadServerSourceSummary(serverId: String, forceRefresh: Boolean = false) =
        loadServerSourceSummaryImpl(serverId, forceRefresh)

    fun clearServerTestResult() = clearServerTestResultImpl()

    fun testServerConnection(profile: AndroidServerProfile) = testServerConnectionImpl(profile)

    fun saveServer(profile: AndroidServerProfile, onComplete: () -> Unit = {}) =
        saveServerImpl(profile, onComplete)

    fun deleteServer(serverId: String) = deleteServerImpl(serverId)

    fun setServerEnabled(serverId: String, enabled: Boolean) = setServerEnabledImpl(serverId, enabled)

    fun openServerBrowser(server: AndroidServerProfile) = openServerBrowserImpl(server)

    fun closeServerBrowser() = closeServerBrowserImpl()

    fun selectServerLibrary(libraryId: String?) = selectServerLibraryImpl(libraryId)

    fun openServerArtist(artist: AndroidHomeCard) = openServerArtistImpl(artist)

    fun closeServerArtist() = closeServerArtistImpl()

    fun openWebDavFolder(folderIdOrPath: String?, label: String) = openWebDavFolderImpl(folderIdOrPath, label)

    fun navigateWebDavBack(): Boolean = navigateWebDavBackImpl()

    fun searchInsideServer(query: String) = searchInsideServerImpl(query)

    fun openServerAlbum(card: AndroidHomeCard) = openServerAlbumImpl(card)

    internal fun openServerAlbumById(
        albumId: String,
        albumTitle: String,
        artistName: String = "",
        coverUrl: String? = null,
        year: String? = null,
    ) = openServerAlbumByIdImpl(albumId, albumTitle, artistName, coverUrl, year)

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

    fun downloadTrack(track: AndroidTrack, collectionId: String? = null) =
        downloadTrackImpl(track, collectionId)

    fun downloadCollection(tracks: List<AndroidTrack>, collectionId: String) =
        downloadCollectionImpl(tracks, collectionId)

    fun cancelDownload(videoId: String) {
        downloadManager.cancelDownload(videoId)
    }

    fun removeDownload(videoId: String) = removeDownloadImpl(videoId)

    fun clearAllDownloads() = clearAllDownloadsImpl()

    fun playDownloadedTrack(record: DownloadedTrackRecord) = playDownloadedTrackImpl(record)

    internal fun DownloadedTrackRecord.toAndroidTrack(): AndroidTrack = downloadedRecordToAndroidTrack(this)

    fun startQuickRadio(seedTrack: AndroidTrack? = uiState.value.activeTrack) = startQuickRadioImpl(seedTrack)

    fun startPersonalStation() = startPersonalStationImpl()

    fun startDiscoveryStation() = startDiscoveryStationImpl()

    fun setSleepTimer(minutes: Int?) = setSleepTimerImpl(minutes)

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
    internal var requestedCanvasTrackId: String? = null

    internal fun resolveCanvasForTrack(
        track: AndroidTrack,
        generation: Long = playbackGeneration.current,
    ) = resolveCanvasForTrackImpl(track, generation)

    fun toggleStarTrack(trackId: String) = toggleStarTrackImpl(trackId)

    fun openAddToPlaylist(tracks: List<AndroidTrack>) = openAddToPlaylistImpl(tracks)

    fun dismissAddToPlaylist() = dismissAddToPlaylistImpl()

    fun clearAddToPlaylistStatus() = clearAddToPlaylistStatusImpl()

    fun addTracksToPlaylists(
        tracks: List<AndroidTrack>,
        playlistIds: List<String>,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) = addTracksToPlaylistsImpl(tracks, playlistIds, onComplete)

    fun createPlaylistWithTracks(
        title: String,
        tracks: List<AndroidTrack>,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) = createPlaylistWithTracksImpl(title, tracks, onComplete)

    fun createPersonalPlaylist(name: String) {
        createPlaylistWithTracksImpl(
            title = name,
            tracks = emptyList(),
            forcePersonal = true,
        )
    }

    fun deletePersonalPlaylist(playlistId: String) = deletePersonalPlaylistImpl(playlistId)

    fun removeTrackFromPersonalPlaylist(playlistId: String, trackId: String) =
        removeTrackFromPersonalPlaylistImpl(playlistId, trackId)

    fun editPlaylist(
        playlistId: String,
        title: String?,
        description: String?,
        privacy: String?,
        coverBytes: ByteArray?,
        mimeType: String?,
        onComplete: (Boolean, String) -> Unit = { _, _ -> },
    ) = editPlaylistImpl(playlistId, title, description, privacy, coverBytes, mimeType, onComplete)

    fun removeTrackFromCache(videoId: String) = removeTrackFromCacheImpl(videoId)

    internal suspend fun fetchTracksForCollectionAction(
        engine: SonataEngineGateway,
        browseId: String,
    ): List<AndroidTrack> = fetchTracksForCollectionActionImpl(engine, browseId)

    fun playCollectionShuffled(
        browseId: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
    ) = playCollectionShuffledImpl(browseId, title, subtitle, coverUrl)

    fun enqueueCollectionNext(browseId: String) = enqueueCollectionNextImpl(browseId)

    fun enqueueCollectionLater(browseId: String) = enqueueCollectionLaterImpl(browseId)

    fun downloadCollectionById(browseId: String) = downloadCollectionByIdImpl(browseId)

    fun playNext(track: AndroidTrack) = playNextImpl(track)

    fun addToQueue(track: AndroidTrack) = addToQueueImpl(track)

    fun toggleShuffleAndPlay(tracks: List<AndroidTrack>) = toggleShuffleAndPlayImpl(tracks)

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
                    clearGuestLastPlayedHistoryImpl()
                    _lastPlayedTracks.value = emptyList()
                    val guestActive = _uiState.value.activeTrack
                    val dropGuestTrack = guestActive != null && !isPersonalTrackId(guestActive.id)
                    if (dropGuestTrack) {
                        playbackGeneration.next()
                        playJob?.cancel()
                        lyricsJob?.cancel()
                        canvasJob?.cancel()
                        musicVideoProbeJob?.cancel()
                        upgradeJob?.cancel()
                        PlaybackAudioBridge.clearEndedTransitionSuppression()
                        AudioPipelineBus.reset()
                        viewModelScope.launch(Dispatchers.Main) {
                            runCatching {
                                controller?.stop()
                                controller?.clearMediaItems()
                            }
                        }
                    }
                    refreshJob?.cancel()
                    homeFetchInFlight = false
                    fullHomeShelves = emptyList()
                    _uiState.update {
                        it.copy(
                            authInitialized = true,
                            signedIn = true,
                            accountProfile = profile,
                            shelves = emptyList(),
                            exploreShelves = emptyList(),
                            radioMixShelves = emptyList(),
                            lastPlayedTracks = emptyList(),
                            recentArtistCards = emptyList(),
                            activeTrack = if (dropGuestTrack) null else it.activeTrack,
                            queue = if (dropGuestTrack) emptyList() else it.queue,
                            queueIndex = if (dropGuestTrack) -1 else it.queueIndex,
                            isPlaying = if (dropGuestTrack) false else it.isPlaying,
                            isBuffering = if (dropGuestTrack) false else it.isBuffering,
                            streamBadge = if (dropGuestTrack) "" else it.streamBadge,
                            status = profile?.let { p -> s.signedInAsStatus(p.name) } ?: s.signedInYtmStatus
                        )
                    }
                    refreshHomeAndExploreRateLimited(
                        forceManual = true,
                        showBlockingSpinner = true,
                        forceBypassCooldown = true,
                    )
                    refreshRadioRateLimited(forceManual = true)
                    refreshListeningHistory()
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
            clearAccountLastPlayedHistoryImpl()
            _lastPlayedTracks.value = emptyList()
            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = false,
                    accountProfile = null,
                    ownedPlaylistIds = emptySet(),
                    playlists = mergePlaylistsWithLocal(emptyList()),
                    lastPlayedTracks = emptyList(),
                    recentArtistCards = emptyList(),
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
        if (trimmed.length < 2) {
            searchJob?.cancel()
            _uiState.update { it.copy(loading = false, serverSearchLoading = false) }
            return
        }
        searchDebounceJob = viewModelScope.launch {
            delay(320L)
            search(overrideQuery = null)
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
            val currentSettings = settings.value
            val personalOnly = currentSettings.personalOnlyMode
            val includeJioSaavn = currentSettings.effectiveJioSaavnEnabled()
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
                    query = if (overrideQuery != null) q else state.query,
                    recentSearches = updatedHistory,
                    loading = !personalOnly,
                    serverSearchLoading = hasEnabledServers,
                    serverSearchResult = if (hasEnabledServers) state.serverSearchResult else null,
                    status = s.searchingQueryStatus(q)
                )
            }
            val localMatches = if (currentSettings.localMusicEnabled) {
                _uiState.value.localMusicLibrary.tracks
                    .filter {
                        it.title.contains(q, ignoreCase = true) ||
                            it.artist.contains(q, ignoreCase = true) ||
                            it.album.contains(q, ignoreCase = true)
                    }
                    .map { it.toAndroidTrack() }
            } else {
                emptyList()
            }
            val downloadMatches = if (personalOnly) {
                emptyList()
            } else {
                downloads.value
                    .filter {
                        it.title.contains(q, ignoreCase = true) ||
                            it.artist.contains(q, ignoreCase = true) ||
                            it.album.orEmpty().contains(q, ignoreCase = true)
                    }
                    .map { it.toAndroidTrack() }
            }
            val offlinePool = (localMatches + downloadMatches).distinctBy { it.id }

            if (personalOnly) {
                val srvResult = if (hasEnabledServers) {
                    withTimeoutOrNull(8_000L) {
                        val rustSrv = runSuspendCatching { engine.searchAllServers(q, 20u) }.getOrNull()
                        val smbServers = _uiState.value.servers.filter { it.enabled && it.kind == AndroidServerKind.SMB }
                        if (smbServers.isEmpty()) {
                            rustSrv
                        } else {
                            val smbResults = smbServers.mapNotNull { srv ->
                                runSuspendCatching { SmbClient.searchTracks(srv, q) }.getOrNull()
                            }
                            uniffi.sonata_android.AndroidServerSearchResult(
                                tracks = (rustSrv?.tracks.orEmpty() + smbResults.flatMap { it.tracks }).distinctBy { it.id },
                                albums = (rustSrv?.albums.orEmpty() + smbResults.flatMap { it.albums }).distinctBy { it.id },
                                artists = (rustSrv?.artists.orEmpty() + smbResults.flatMap { it.artists }).distinctBy { it.id },
                                playlists = (rustSrv?.playlists.orEmpty() + smbResults.flatMap { it.playlists }).distinctBy { it.id },
                            )
                        }
                    }
                } else {
                    null
                }
                if (!isActive) return@launch
                val combinedPersonal = (localMatches + srvResult?.tracks.orEmpty()).distinctBy { it.id }
                _uiState.update { state ->
                    if (state.query == q) {
                        state.copy(
                            loading = false,
                            serverSearchLoading = false,
                            serverSearchResult = srvResult,
                            tracks = combinedPersonal,
                            listTitle = s.searchResultsHeader(q),
                            status = s.songsFoundCount(combinedPersonal.size),
                        )
                    } else {
                        state
                    }
                }
                return@launch
            }

            val serverJob = if (hasEnabledServers) {
                launch {
                    val srvResult = withTimeoutOrNull(8_000L) {
                        val rustSrv = runSuspendCatching { engine.searchAllServers(q, 20u) }.getOrNull()
                        val smbServers = _uiState.value.servers.filter { it.enabled && it.kind == AndroidServerKind.SMB }
                        if (smbServers.isEmpty()) {
                            rustSrv
                        } else {
                            val smbResults = smbServers.mapNotNull { srv ->
                                runSuspendCatching { SmbClient.searchTracks(srv, q) }.getOrNull()
                            }
                            uniffi.sonata_android.AndroidServerSearchResult(
                                tracks = (rustSrv?.tracks.orEmpty() + smbResults.flatMap { it.tracks }).distinctBy { it.id },
                                albums = (rustSrv?.albums.orEmpty() + smbResults.flatMap { it.albums }).distinctBy { it.id },
                                artists = (rustSrv?.artists.orEmpty() + smbResults.flatMap { it.artists }).distinctBy { it.id },
                                playlists = (rustSrv?.playlists.orEmpty() + smbResults.flatMap { it.playlists }).distinctBy { it.id },
                            )
                        }
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
                        (ytTracks + offlinePool).distinctBy { it.id }
                    } else {
                        val seenKeys = ytTracks.mapTo(HashSet()) {
                            "${it.title.trim().lowercase()}|${it.artist.trim().lowercase()}"
                        }
                        val extraJio = jioTracks.filter { js ->
                            seenKeys.add("${js.title.trim().lowercase()}|${js.artist.trim().lowercase()}")
                        }
                        (ytTracks + extraJio + offlinePool).distinctBy { it.id }
                    }
                    _uiState.update { state ->
                        if (state.query.trim() != q) return@update state
                        state.copy(
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
                    if (_uiState.value.query.trim() == q) {
                        prefetchStreams(ytTracks.take(2), delayMs = 1_500L)
                    }
                }
                .onFailure { err ->
                    val fallback = (jioTracks + offlinePool).distinctBy { it.id }
                    if (fallback.isNotEmpty()) {
                        _uiState.update { state ->
                            if (state.query.trim() != q) return@update state
                            state.copy(
                                loading = false,
                                tracks = fallback,
                                listTitle = s.searchResultsHeader(q),
                                status = s.songsFoundJioCount(fallback.size)
                            )
                        }
                    } else {
                        _uiState.update { state ->
                            if (state.query.trim() != q) return@update state
                            state.copy(loading = false, status = s.searchErrorStatus(err.message.orEmpty()))
                        }
                    }
                }
        }
    }

    fun isCollectionPinned(browseId: String?): Boolean = isCollectionPinnedImpl(browseId)

    fun isPlaylistSavedToLibrary(browseId: String?): Boolean = isPlaylistSavedToLibraryImpl(browseId)

    fun isOwnedEditablePlaylist(browseId: String?): Boolean = isOwnedEditablePlaylistImpl(browseId)

    fun togglePinCollection(
        id: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
        kindLabel: String = "",
        isAlbum: Boolean = false,
    ) = togglePinCollectionImpl(id, title, subtitle, coverUrl, kindLabel, isAlbum)

    fun toggleSaveExternalPlaylist(
        id: String,
        title: String,
        subtitle: String = "",
        coverUrl: String? = null,
        kindLabel: String = "",
    ) = toggleSaveExternalPlaylistImpl(id, title, subtitle, coverUrl, kindLabel)

    fun openPinnedCollection(entry: PinnedCollectionEntry) = openPinnedCollectionImpl(entry)

    fun openPlaylist(playlist: AndroidPlaylist) = openPlaylistImpl(playlist)

    fun openDownloadsPlaylist() = openDownloadsPlaylistImpl()

    fun openAlbumOrPlaylist(
        id: String,
        title: String,
        coverUrl: String? = null,
        subtitle: String? = null,
        kindLabel: String? = null,
        year: String? = null,
    ) = openAlbumOrPlaylistImpl(id, title, coverUrl, subtitle, kindLabel, year)

    fun openAlbum(
        albumId: String,
        albumTitle: String,
        artistName: String = "",
        coverUrl: String? = null,
        year: String? = null,
    ) = openAlbumImpl(albumId, albumTitle, artistName, coverUrl, year)

    fun openArtist(browseId: String, name: String = "") = openArtistImpl(browseId, name)

    fun selectHomeMoodChip(params: String?) = selectHomeMoodChipImpl(params)

    fun openMoodCategory(title: String, browseId: String = "", params: String = "") =
        openMoodCategoryImpl(title, browseId, params)

    fun openMood(title: String) {
        openMoodCategory(title)
    }

    fun closeMoodCategory() = closeMoodCategoryImpl()

    fun openCard(card: AndroidHomeCard, shelfTracks: List<AndroidTrack> = emptyList()) =
        openCardImpl(card, shelfTracks)

    internal fun openCollectionById(
        id: String,
        title: String,
        coverUrl: String? = null,
        subtitle: String? = null,
        kindLabel: String? = null,
        year: String? = null,
    ) = openCollectionByIdImpl(id, title, coverUrl, subtitle, kindLabel, year)

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
            if (isPersonalTrackId(track.id) || getCachedStream(track.id) != null || !inFlightPrefetch.add(track.id)) continue
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
                            baseStreamBadges[track.id] = stream.badge
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
        val personalOnly = preferences.state.value.personalOnlyMode
        if (personalOnly && !isPersonalTrackId(track.id)) {
            return
        }
        val scopedQueueList = if (personalOnly) {
            queueList?.filter { isPersonalTrackId(it.id) }
        } else {
            queueList
        }
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
        scrobbleAndStatsTracker.onTrackStarted(
            generation = generation,
            track = track,
            sourceTag = ScrobbleAndStatsTracker.inferSourceTag(track.id),
            startedAtUnixSecs = System.currentTimeMillis() / 1000L,
        )
        val wasVideoMode = _uiState.value.videoModeEnabled
        if (wasVideoMode) {
            PlaybackAudioBridge.setVideoModePlayback(false)
            PlaybackAudioBridge.applyCrossfade(
                settings.value.crossfadeEnabled,
                settings.value.crossfadeSeconds,
            )
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
        val previousQueue = _uiState.value.queue.let { prev ->
            if (personalOnly) prev.filter { isPersonalTrackId(it.id) } else prev
        }
        val isNewExplicitQueue = !scopedQueueList.isNullOrEmpty() && !queuesSharePrefix(scopedQueueList, previousQueue)
        if (isNewExplicitQueue) {
            playedTrackIdsInCurrentQueue.clear()
            prefetchedAutoplaySeedKey = null
            prefetchedAutoplayTracks = emptyList()
        }
        val updatedQueue = when {
            !scopedQueueList.isNullOrEmpty() -> {
                if (scopedQueueList.any { it.id == track.id }) scopedQueueList else listOf(track) + scopedQueueList
            }
            previousQueue.any { it.id == track.id } -> previousQueue
            else -> listOf(track)
        }
        val updatedQueueIds = updatedQueue.mapTo(HashSet(updatedQueue.size)) { it.id }
        playedTrackIdsInCurrentQueue.retainAll(updatedQueueIds)
        playedTrackIdsInCurrentQueue.add(track.id)
        val updatedIndex = updatedQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

        if (isRemoteConnectActive()) {
            pauseLocalPlaybackForConnect()
            pendingRemoteTransferTrackId = track.id
            pendingRemoteTransferUntilMs = System.currentTimeMillis() + 3_500L
            lastMirroredConnectTrackId = track.id
            val peerName = connectController.state.value.activePeer?.deviceName.orEmpty()
            _uiState.update {
                it.copy(
                    queue = updatedQueue,
                    queueIndex = updatedIndex,
                    activeTrack = track,
                    isRemoteConnectActive = true,
                    isPlaying = true,
                    isBuffering = true,
                    isUpgradingQuality = false,
                    streamBadge = peerName,
                    lyrics = emptyList(),
                    lyricsLoading = true,
                    lyricsUnavailable = false,
                )
            }
            playbackClock.resetForTrack((track.durationSecs?.toLong() ?: 0L) * 1000L)
            fetchLyricsInBackground(track, generation)
            viewModelScope.launch(Dispatchers.IO) {
                recordLastPlayed(track)
                connectController.transferQueue(
                    tracks = updatedQueue,
                    startIndex = updatedIndex,
                    positionMs = 0L,
                    playWhenReady = true,
                )
            }
            return
        }

        // Switch activeTrack and clear lyrics BEFORE resetting playbackClock so the old track's
        // position poller/listener cannot overwrite the new track's clock with end-of-song positionMs.
        _uiState.update {
            it.copy(
                queue = updatedQueue,
                queueIndex = updatedIndex,
                activeTrack = track,
                isPlaying = false,
                isBuffering = true,
                isUpgradingQuality = false,
                lyrics = emptyList(),
                lyricsLoading = true,
                lyricsUnavailable = false,
            )
        }
        playbackClock.resetForTrack((track.durationSecs?.toLong() ?: 0L) * 1000L)
        if (!personalOnly) {
            resolveCanvasForTrack(track, generation)
            probeMusicVideoForTrack(track, generation)
        }

        playJob = viewModelScope.launch(Dispatchers.IO) {
            recordLastPlayed(track)
            val playbackPlaylistId = resolvePlaybackPlaylistId(track, scopedQueueList)
            if (isNewExplicitQueue) {
                activeQueuePlaylistId = playbackPlaylistId
            }
            if (!personalOnly) {
                maybePrefetchEndlessAutoplay(updatedQueue, updatedIndex, generation)
            }
            val localMediaUri = resolveLocalContentUri(track.id)
            val offlineUri = downloadManager.localUriFor(track.id) ?: localMediaUri
            if (offlineUri != null) {
                if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
                val offlineRecord = if (localMediaUri == null) {
                    downloads.value.firstOrNull { it.videoId == track.id }
                } else {
                    null
                }
                val offlineBadge = if (localMediaUri != null) {
                    resolveLocalInitialBadge(track.id)
                } else {
                    val offlineLabel = uiStrings().offlineLabel
                    offlineRecord?.codec?.takeIf { it.isNotBlank() }?.let { "$offlineLabel · $it" } ?: offlineLabel
                }
                baseStreamBadges[track.id] = offlineBadge
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
                if (localMediaUri == null) {
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
                }
                return@launch
            }

            val engine = requireEngine()
            if (!isActive || !playbackGeneration.isCurrent(generation)) return@launch
            val cachedStream = getCachedStream(track.id)
            if (cachedStream != null) {
                baseStreamBadges[track.id] = cachedStream.badge
            } else {
                baseStreamBadges.remove(track.id)
            }
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
            if ((track.durationSecs ?: 0uL) > 0uL ||
                track.id.startsWith("smb:") ||
                track.id.startsWith("local:")
            ) {
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
            } else if (track.id.startsWith("smb:")) {
                val parsed = SmbClient.parseTrackId(track.id)
                val resolved = if (parsed == null) {
                    Result.failure(IllegalArgumentException("Invalid SMB track ID"))
                } else {
                    val (srvId, relPath) = parsed
                    _uiState.value.servers.firstOrNull { it.id == srvId }?.let { SmbProfileRegistry.put(it) }
                    Result.success(
                        AndroidResolvedStream(
                            url = SmbClient.buildStreamUri(srvId, relPath),
                            badge = SmbClient.initialBadgeForTrack(srvId, relPath),
                            headers = emptyList(),
                        ),
                    )
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
                                    val cleanBadge = stripServerLabelsFromBadge(stream.badge)
                                    baseStreamBadges[effectiveTrack.id] = cleanBadge
                                    val displayBadge = resolveDisplayBadgeForTrack(effectiveTrack.id, cleanBadge)
                                    _uiState.update {
                                        it.copy(
                                            streamBadge = displayBadge,
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
                    val committedBadge = _uiState.value.takeIf { it.activeTrack?.id == effectiveTrack.id }?.streamBadge
                        ?: stream.badge
                    saveActiveTrackToPrefs(effectiveTrack, committedBadge)
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

                    if (!isPersonalTrackId(effectiveTrack.id)) {
                        launchBackgroundQualityUpgrade(
                            generation = generation,
                            engine = engine,
                            effectiveTrack = effectiveTrack,
                            currentStream = stream,
                        )
                    }
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
    internal fun buildOfflineMediaItem(track: AndroidTrack, localUri: String): MediaItem {
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
    internal fun buildMediaItem(track: AndroidTrack, stream: AndroidResolvedStream): MediaItem {
        if (stream.headers.isNotEmpty()) {
            PlaybackAudioBridge.registerStreamHeaders(
                url = stream.url,
                headers = stream.headers.associate { it.name to it.value },
            )
        }
        val isSmbStream = stream.url.startsWith("${SmbClient.STREAM_SCHEME}://", ignoreCase = true)
        val uri = if (stream.url.contains("://")) Uri.parse(stream.url) else Uri.fromFile(File(stream.url))
        val cacheKey = "${track.id}:${stream.badge}"
        if (!isSmbStream) {
            activeCacheKeysByTrackId[track.id] = cacheKey
        }
        return MediaItem.Builder()
            .setMediaId(track.id)
            .apply {
                if (!isSmbStream) setCustomCacheKey(cacheKey)
            }
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

    internal fun launchBackgroundQualityUpgrade(
        generation: Long,
        engine: SonataEngineGateway,
        effectiveTrack: AndroidTrack,
        currentStream: AndroidResolvedStream,
    ) {
        if (_uiState.value.videoModeEnabled) return
        if (settings.value.personalOnlyMode || isPersonalTrackId(effectiveTrack.id)) return
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
        val allowJioSaavnUpgrade = settings.value.effectiveJioSaavnEnabled() &&
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
                                    baseStreamBadges[effectiveTrack.id] = targetUpgrade.badge
                                    val displayBadge = resolveDisplayBadgeForTrack(effectiveTrack.id, targetUpgrade.badge)
                                    _uiState.update { it.copy(streamBadge = displayBadge) }
                                },
                            )
                        }.onFailure { err ->
                            Log.w("SonataStream", "Hi-Fi swap failed: ${err.message}")
                        }.getOrNull()
                    }
                    if (swap?.applied == true) {
                        val finalBadge = _uiState.value.takeIf { it.activeTrack?.id == effectiveTrack.id }?.streamBadge
                            ?: targetUpgrade.badge
                        saveActiveTrackToPrefs(effectiveTrack, finalBadge)
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

    internal fun selectDistinctArtistIds(tracks: List<AndroidTrack>, maxCount: Int): List<String> {
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

    internal fun buildPreferredArtists(): List<String> {
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
        val out = ArrayList<String>(2)
        fun isYtmSeed(t: AndroidTrack): Boolean =
            t.id.isNotBlank() && !t.id.startsWith("local:") && !t.id.startsWith("jiosaavn:") && !isSelfHostedId(t.id)
        // Anchor endless radio to the user's original session seed (first YTM track in queue)
        // plus the currently playing track rather than the lowest-scored tail of previous batches.
        queue.firstOrNull(::isYtmSeed)?.let { out.add(it.id) }
        val currentTrack = queue.getOrNull(currentIndex.coerceIn(0, (queue.size - 1).coerceAtLeast(0)))
        if (currentTrack != null && isYtmSeed(currentTrack) && !out.contains(currentTrack.id)) {
            out.add(currentTrack.id)
        }
        if (out.size < 2) {
            queue.firstOrNull { isYtmSeed(it) && !out.contains(it.id) }?.let { out.add(it.id) }
        }
        if (out.isEmpty()) {
            _lastPlayedTracks.value.firstOrNull(::isYtmSeed)?.let { out.add(it.id) }
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
        if (preferences.state.value.personalOnlyMode) return emptyList()
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
        if (preferences.state.value.personalOnlyMode || !settings.value.autoplayEnabled || _uiState.value.repeatMode != Player.REPEAT_MODE_OFF) return
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
                activeQueuePlaylistId = null
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
        if (isRemoteConnectActive()) {
            viewModelScope.launch(Dispatchers.IO) {
                connectPlaybackRouter.nextTrack {}
            }
            return
        }
        val personalOnly = preferences.state.value.personalOnlyMode
        val state = _uiState.value
        val activeQueue = if (personalOnly) state.queue.filter { isPersonalTrackId(it.id) } else state.queue
        val activeIdx = if (personalOnly) {
            activeQueue.indexOfFirst { it.id == state.activeTrack?.id }
        } else {
            state.queueIndex
        }
        val nextIndex = if (state.shuffleEnabled && state.repeatMode == Player.REPEAT_MODE_OFF) {
            val unplayed = activeQueue.indices.filter { idx ->
                idx != activeIdx && activeQueue[idx].id !in playedTrackIdsInCurrentQueue
            }
            unplayed.takeIf { it.isNotEmpty() }?.let { pool -> pool[Random.nextInt(pool.size)] }
        } else {
            PlaybackEligibility.resolveNextIndex(
                queueSize = activeQueue.size,
                queueIndex = activeIdx,
                repeatMode = state.repeatMode,
                shuffleEnabled = state.shuffleEnabled,
            )
        }
        if (nextIndex != null && nextIndex in activeQueue.indices) {
            playTrack(activeQueue[nextIndex], activeQueue)
        } else if (!personalOnly && settings.value.autoplayEnabled && state.activeTrack != null) {
            triggerEndlessContinuation(state, playbackGeneration.current, stopWhenEmpty = false)
        }
    }

    fun prevTrack() {
        if (isRemoteConnectActive()) {
            viewModelScope.launch(Dispatchers.IO) {
                connectPlaybackRouter.prevTrack {}
            }
            return
        }
        val personalOnly = preferences.state.value.personalOnlyMode
        val state = _uiState.value
        val activeQueue = if (personalOnly) state.queue.filter { isPersonalTrackId(it.id) } else state.queue
        val activeIdx = if (personalOnly) {
            activeQueue.indexOfFirst { it.id == state.activeTrack?.id }
        } else {
            state.queueIndex
        }
        val currentPos = controller?.currentPosition ?: playbackClock.positionMs
        val prevIndex = PlaybackEligibility.resolvePreviousIndex(
            queueSize = activeQueue.size,
            queueIndex = activeIdx,
            repeatMode = state.repeatMode,
            currentPositionMs = currentPos,
        )
        if (prevIndex == null || prevIndex !in activeQueue.indices) {
            seekTo(0L)
            return
        }
        playTrack(activeQueue[prevIndex], activeQueue)
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
        val personalOnly = preferences.state.value.personalOnlyMode
        val state = _uiState.value
        if (state.repeatMode == Player.REPEAT_MODE_ONE) {
            if (personalOnly && state.activeTrack != null && !isPersonalTrackId(state.activeTrack.id)) {
                PlaybackAudioBridge.clearEndedTransitionSuppression()
                playbackClock.onSeek(0L)
                _uiState.update { it.copy(isPlaying = false) }
                return
            }
            PlaybackAudioBridge.clearEndedTransitionSuppression()
            seekTo(0L)
            controller?.play()
            return
        }
        val activeQueue = if (personalOnly) state.queue.filter { isPersonalTrackId(it.id) } else state.queue
        val activeIdx = if (personalOnly) {
            activeQueue.indexOfFirst { it.id == state.activeTrack?.id }
        } else {
            state.queueIndex
        }
        val hasPlayableNext = if (state.shuffleEnabled && state.repeatMode == Player.REPEAT_MODE_OFF) {
            activeQueue.indices.any { idx ->
                idx != activeIdx && activeQueue[idx].id !in playedTrackIdsInCurrentQueue
            }
        } else {
            PlaybackEligibility.resolveNextIndex(
                queueSize = activeQueue.size,
                queueIndex = activeIdx,
                repeatMode = state.repeatMode,
                shuffleEnabled = state.shuffleEnabled,
            ) != null
        }
        if (hasPlayableNext) {
            nextTrack()
        } else if (!personalOnly && settings.value.autoplayEnabled && state.activeTrack != null) {
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
        if (isRemoteConnectActive()) {
            val currentPlaying = connectController.state.value.remoteState?.isPlaying ?: _uiState.value.isPlaying
            _uiState.update { it.copy(isPlaying = !currentPlaying) }
            viewModelScope.launch(Dispatchers.IO) {
                connectPlaybackRouter.togglePlayPause(currentPlaying) {}
            }
            return
        }
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
        if (isRemoteConnectActive()) {
            playbackClock.onSeek(clamped)
            viewModelScope.launch(Dispatchers.IO) {
                connectPlaybackRouter.seekTo(clamped) {}
            }
            return
        }
        runCatching { controller?.seekTo(clamped) }
        playbackClock.onSeek(clamped)
        discordSeekSignal.value += 1L
    }

    internal var updateDownloadJob: Job? = null
    internal var dismissedUpdateVersion: String? = null

    fun checkForUpdates(autoPopup: Boolean = true, isManual: Boolean = true) =
        checkForUpdatesImpl(autoPopup, isManual)

    fun openUpdateDialog() = openUpdateDialogImpl()

    fun dismissUpdateDialog() = dismissUpdateDialogImpl()

    fun cancelUpdateDownload() = cancelUpdateDownloadImpl()

    fun downloadAndInstallUpdate() = downloadAndInstallUpdateImpl()

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

    internal fun probeMusicVideoForTrack(
        track: AndroidTrack,
        generation: Long = playbackGeneration.current,
    ) = probeMusicVideoForTrackImpl(track, generation)

    fun enableVideoMode() = enableVideoModeImpl()

    fun selectVideoQuality(
        preference: VideoQualityPreference,
        explicitHeight: Int? = null,
    ) = selectVideoQualityImpl(preference, explicitHeight)

    fun disableVideoMode() = disableVideoModeImpl()

    fun openStatsScreen() = openStatsScreenImpl()

    fun dismissStatsScreen() = dismissStatsScreenImpl()

    fun selectStatsWindow(window: AndroidStatsWindow) = selectStatsWindowImpl(window)

    fun refreshListeningStats(window: AndroidStatsWindow = _uiState.value.statsWindow) =
        refreshListeningStatsImpl(window)

    fun saveScrobbleConfig(config: AndroidScrobbleConfig) = saveScrobbleConfigImpl(config)

    internal fun dispatchScrobbleNowPlaying(track: AndroidTrack) = dispatchScrobbleNowPlayingImpl(track)

    internal fun dispatchScrobbleSubmit(track: AndroidTrack, timestampUnix: ULong) =
        dispatchScrobbleSubmitImpl(track, timestampUnix)

    internal fun dispatchRecordListeningStat(
        track: AndroidTrack,
        sourceTag: String,
        playedSecs: ULong,
        playedAtUnix: ULong?,
    ) = dispatchRecordListeningStatImpl(track, sourceTag, playedSecs, playedAtUnix)

    fun openConnectSheet() = openConnectSheetImpl()

    fun dismissConnectSheet() = dismissConnectSheetImpl()

    fun setConnectEnabled(enabled: Boolean) = setConnectEnabledImpl(enabled)

    fun setConnectDeviceName(name: String) = setConnectDeviceNameImpl(name)

    fun refreshConnectDiscovery() = refreshConnectDiscoveryImpl()

    fun requestPairConnectDevice(beacon: ConnectBeacon) = requestPairConnectDeviceImpl(beacon)

    fun cancelConnectPairing() = cancelConnectPairingImpl()

    fun clearConnectError() = clearConnectErrorImpl()

    fun pairConnectDevice(beacon: ConnectBeacon, pin: String) = pairConnectDeviceImpl(beacon, pin)

    fun connectPairedDevice(beacon: ConnectBeacon) = connectPairedDeviceImpl(beacon)

    fun disconnectConnectDevice() = disconnectConnectDeviceImpl()

    fun forgetConnectDevice(deviceId: String) = forgetConnectDeviceImpl(deviceId)

    fun sendConnectCommand(command: ConnectCommand) = sendConnectCommandImpl(command)

    fun setConnectRemoteVolume(volume: Float) = setConnectRemoteVolumeImpl(volume)

    fun transferQueueToConnectDevice() = transferQueueToConnectDeviceImpl()

    override fun onCleared() {
        PlaybackQueueBridge.reset()
        PlaybackAudioBridge.clearEndedTransitionSuppression()
        CanvasResolver.detachGateway()
        statsLoadJob?.cancel()
        statsLoadJob = null
        connectController.close()
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
        localScanJob?.cancel()
        localScanJob = null
        serverHealthJobs.values.forEach { it.cancel() }
        serverHealthJobs.clear()
        serverSpeedJobs.values.forEach { it.cancel() }
        serverSpeedJobs.clear()
        serverRescanJobs.values.forEach { it.cancel() }
        serverRescanJobs.clear()
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
        internal const val LOCAL_PLAYLISTS_KEY = "playlists_json"
        internal const val LOCAL_PLAYLIST_PREFIX = "local:pl_"
        private const val LAST_PLAYED_PREFS_NAME = "sonata_last_played_history"
        internal const val LAST_PLAYED_KEY = "tracks_json"
        internal const val GUEST_LAST_PLAYED_KEY = "last_played_guest_v1"
        internal const val ACCOUNT_LAST_PLAYED_KEY = "last_played_account_v1"
        internal const val DISMISSED_LAST_PLAYED_KEY = "dismissed_last_played_v1"
        internal const val MAX_LAST_PLAYED_HISTORY = 50
        internal const val MAX_DISMISSED_LAST_PLAYED = 250
        internal const val ARTIST_CACHE_TTL_MS = 15L * 60L * 1000L
        internal const val ARTIST_CACHE_MAX_ENTRIES = 20
        private const val MAX_RECOVERY_ATTEMPTS = 2
        internal const val STREAM_CACHE_TTL_MS = 4L * 60L * 60L * 1000L
        private const val STREAM_CACHE_MAX_ENTRIES = 64
        private const val AUTO_REFRESH_COOLDOWN_MS = 18_000L
        private const val MANUAL_REFRESH_COOLDOWN_MS = 4_000L
        private const val RATE_LIMIT_BACKOFF_MS = 45_000L
        internal val YEAR_REGEX = Regex("\\b(19|20)\\d{2}\\b")
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

        internal fun shouldAttributePersonalPlay(
            personalOnlyMode: Boolean,
            attributePersonalToYtm: Boolean,
            signedIn: Boolean,
            trackId: String,
        ): Boolean =
            !personalOnlyMode &&
                attributePersonalToYtm &&
                signedIn &&
                trackId.isNotBlank() &&
                isPersonalTrackId(trackId)

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
