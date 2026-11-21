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
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.json.JSONTokener
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidJsCipherSolver
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidSolvedCipher
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import uniffi.sonata_android.SonataAndroidEngine
import java.io.File
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
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

enum class AndroidTab { Home, Explore, Library, Search }

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
    val shelves: List<AndroidHomeShelf> = emptyList(),
    val playlists: List<AndroidPlaylist> = emptyList(),
    val tracks: List<AndroidTrack> = emptyList(),
    val listTitle: String = "Quick Picks",
    val collectionTitle: String? = null,
    val collectionCoverUrl: String? = null,
    val collectionSubtitle: String? = null,
    val collectionKindLabel: String? = null,
    val collectionYear: String? = null,
    val collectionTracks: List<AndroidTrack> = emptyList(),
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
    val preferHighQuality: Boolean = true,
    val animatedArtwork: Boolean = true,
    val crossfadeEnabled: Boolean = false,
    val liquidGlassEnabled: Boolean = true,
    val fpsLimit: Int = 60,
    val supports120Hz: Boolean = false,
    val likedTrackIds: Set<String> = emptySet(),
    val recentSearches: List<String> = emptyList(),
    val updateInfo: AndroidUpdateInfo? = null,
    val checkingUpdates: Boolean = false,
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

    private val engineDeferred = CompletableDeferred<SonataAndroidEngine>()
    private val cipherSolver = WebViewJsCipherSolver(application)

    private val streamCache = ConcurrentHashMap<String, CachedStream>()
    private val inFlightPrefetch = ConcurrentHashMap.newKeySet<String>()

    @OptIn(ExperimentalCoroutinesApi::class)
    private val prefetchDispatcher = Dispatchers.IO.limitedParallelism(1)

    val playbackClock = PlaybackClockState()

    private val _uiState = MutableStateFlow(SonataUiState())
    val uiState: StateFlow<SonataUiState> = _uiState.asStateFlow()

    private var controller: MediaController? = null
    private var playJob: Job? = null
    private var lyricsJob: Job? = null
    private var refreshJob: Job? = null
    private var speculativePrefetchJob: Job? = null
    private var lyricsFetchedForTrackId: String? = null
    @Volatile private var cipherWarmupScheduled = false

    private val prefs: SharedPreferences by lazy {
        getApplication<Application>().getSharedPreferences("sonata_prefs", Context.MODE_PRIVATE)
    }

    private suspend fun requireEngine(): SonataAndroidEngine = engineDeferred.await()

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

        _uiState.value = SonataUiState(
            loading = true,
            refreshing = false,
            status = "Loading YouTube Music...",
            supports120Hz = supports120Hz,
        )

        registerQueueBridge()
        startPositionPoller()
        startColdLaunchBootstrap(application, supports120Hz)
    }

    @OptIn(UnstableApi::class)
    private fun startColdLaunchBootstrap(application: Application, supports120Hz: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            // 1. Belt-and-suspenders removal of any legacy Home feed cache + non-blocking prefs load.
            runCatching { File(application.cacheDir, "sonata/android_home.json").delete() }

            val savedFps = prefs.getInt("fps_limit", 60).let { if (it == 120 && supports120Hz) 120 else 60 }
            val restoredTrack = loadSavedActiveTrack(emptyList())
            val savedBadge = prefs.getString("last_track_badge", null).orEmpty()
            val initialQueue = listOfNotNull(restoredTrack)
            val initialQueueIndex = if (restoredTrack != null) 0 else -1

            if (restoredTrack != null) {
                playbackClock.resetForTrack((restoredTrack.durationSecs?.toLong() ?: 0L) * 1000L)
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
            val engine = runCatching {
                SonataAndroidEngine(
                    configDir = application.filesDir.absolutePath,
                    cacheDir = application.cacheDir.absolutePath,
                ).also { eng ->
                    runCatching { eng.setJsCipherSolver(cipherSolver) }
                }
            }.getOrElse { err ->
                Log.e("SonataEngine", "Failed to initialize SonataAndroidEngine", err)
                _uiState.update {
                    it.copy(
                        loading = false,
                        authInitialized = true,
                        status = "Engine init error: ${err.message}",
                    )
                }
                return@launch
            }
            engineDeferred.complete(engine)

            // 3. Load lightweight cached auth/profile/playlists (never Home) for instant Library & Profile.
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val cachedProfile = if (signedIn) runCatching { engine.getCachedAccountProfile() }.getOrNull() else null
            val cachedPlaylists = if (signedIn) runCatching { engine.getCachedLibraryPlaylists() }.getOrDefault(emptyList()) else emptyList()

            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = signedIn,
                    accountProfile = cachedProfile ?: it.accountProfile,
                    playlists = if (it.playlists.isEmpty()) cachedPlaylists else it.playlists,
                )
            }

            // 4. Immediately fetch fresh live Home shelves (never disk-cached).
            refreshInBackground(showBlockingSpinner = true)

            if (restoredTrack != null && lyricsFetchedForTrackId != restoredTrack.id) {
                fetchLyricsInBackground(restoredTrack)
            }

            // 5. Defer update check well past critical cold launch & first scroll frames.
            launch(Dispatchers.IO) {
                delay(8_000L)
                checkForUpdates()
            }
        }
    }

    private fun saveActiveTrackToPrefs(track: AndroidTrack, badge: String = "") {
        prefs.edit()
            .putString("last_track_id", track.id)
            .putString("last_track_title", track.title)
            .putString("last_track_artist", track.artist)
            .putString("last_track_album", track.album)
            .putLong("last_track_duration_secs", track.durationSecs?.toLong() ?: 0L)
            .putString("last_track_thumb", track.thumbnailUrl)
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
        return AndroidTrack(
            id = id,
            title = title,
            artist = artist,
            album = album,
            durationSecs = dur,
            thumbnailUrl = thumb,
            liked = false,
        )
    }

    private fun fetchLyricsInBackground(track: AndroidTrack) {
        lyricsJob?.cancel()
        lyricsFetchedForTrackId = track.id
        lyricsJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                if (it.activeTrack?.id == track.id) it.copy(lyricsLoading = true, lyricsUnavailable = false) else it
            }
            val fetched = try {
                runCatching { engine.fetchLyrics(track) }.getOrDefault(emptyList())
            } catch (_: Throwable) {
                emptyList()
            }
            if (_uiState.value.activeTrack?.id == track.id) {
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
        PlaybackQueueBridge.onSkipNext = { viewModelScope.launch { nextTrack() } }
        PlaybackQueueBridge.onSkipPrevious = { viewModelScope.launch { prevTrack() } }
        PlaybackQueueBridge.onPlayFromEmpty = { viewModelScope.launch { togglePlayPause() } }
    }

    private fun collectWarmCandidates(shelves: List<AndroidHomeShelf>): List<AndroidTrack> {
        val fromTracks = shelves.flatMap { it.tracks }
        val fromCards = shelves.flatMap { it.cards }.mapNotNull { it.track }
        return (fromTracks + fromCards).distinctBy { it.id }.take(2)
    }

    private fun scheduleDeferredCipherWarmupAndPrefetch(candidates: List<AndroidTrack>) {
        speculativePrefetchJob?.cancel()
        speculativePrefetchJob = viewModelScope.launch(prefetchDispatcher) {
            // Wait until first Home paint & initial user interaction/scroll settle before spinning up WebView JS.
            delay(2_500L)
            val engine = requireEngine()
            if (!cipherWarmupScheduled) {
                cipherWarmupScheduled = true
                runCatching { engine.warmCipherSolver() }
            }
            resolvePrefetchList(engine, candidates.distinctBy { it.id }.take(2))
        }
    }

    private fun refreshInBackground(showBlockingSpinner: Boolean = false) {
        refreshJob?.cancel()
        refreshJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    loading = showBlockingSpinner && it.shelves.isEmpty(),
                    refreshing = !showBlockingSpinner,
                )
            }
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            if (signedIn) {
                launch {
                    runCatching { engine.getAccountProfile() }
                        .onSuccess { profile ->
                            if (profile != null && profile != _uiState.value.accountProfile) {
                                _uiState.update { it.copy(signedIn = true, accountProfile = profile) }
                            }
                        }
                }
                launch {
                    runCatching { engine.fetchLibraryPlaylists() }
                        .onSuccess { playlists ->
                            if (playlists != _uiState.value.playlists) {
                                _uiState.update { it.copy(playlists = playlists) }
                            }
                        }
                }
            }
            runCatching { engine.fetchHomeFeed() }
                .onSuccess { shelves ->
                    val firstTracks = shelves.firstOrNull { it.tracks.isNotEmpty() }?.tracks.orEmpty()
                    val currentActive = _uiState.value.activeTrack
                    val fallbackActive = currentActive ?: firstTracks.firstOrNull()
                    if (currentActive == null && fallbackActive != null) {
                        saveActiveTrackToPrefs(fallbackActive)
                        playbackClock.resetForTrack((fallbackActive.durationSecs?.toLong() ?: 0L) * 1000L)
                        fetchLyricsInBackground(fallbackActive)
                    }
                    _uiState.update {
                        it.copy(
                            loading = false,
                            refreshing = false,
                            authInitialized = true,
                            signedIn = signedIn,
                            shelves = shelves,
                            tracks = if (it.tracks.isEmpty()) firstTracks else it.tracks,
                            queue = if (it.queue.isEmpty()) firstTracks else it.queue,
                            queueIndex = if (it.queueIndex < 0 && fallbackActive != null) 0 else it.queueIndex,
                            activeTrack = it.activeTrack ?: fallbackActive,
                            status = "${shelves.size} personalized shelves"
                        )
                    }
                    val warmList = (listOfNotNull(_uiState.value.activeTrack) + collectWarmCandidates(shelves))
                        .distinctBy { it.id }
                        .take(2)
                    scheduleDeferredCipherWarmupAndPrefetch(warmList)
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(
                            loading = false,
                            refreshing = false,
                            authInitialized = true,
                            status = if (it.shelves.isEmpty()) "Home error: ${err.message}" else it.status
                        )
                    }
                }
        }
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
                                onTrackEnded()
                            }
                        }
                    }

                    override fun onRepeatModeChanged(repeatMode: Int) {
                        if (_uiState.value.repeatMode != repeatMode) {
                            _uiState.update { it.copy(repeatMode = repeatMode) }
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
                collectionTitle = null,
                collectionCoverUrl = null,
                collectionSubtitle = null,
                collectionKindLabel = null,
                collectionYear = null,
                collectionTracks = emptyList(),
                showLyrics = false
            )
        }
    }

    fun closeCollection() {
        _uiState.update {
            it.copy(
                collectionTitle = null,
                collectionCoverUrl = null,
                collectionSubtitle = null,
                collectionKindLabel = null,
                collectionYear = null,
                collectionTracks = emptyList()
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

    fun togglePreferHighQuality() {
        _uiState.update { it.copy(preferHighQuality = !it.preferHighQuality) }
    }

    fun toggleAnimatedArtwork() {
        _uiState.update { it.copy(animatedArtwork = !it.animatedArtwork) }
    }

    fun toggleCrossfade() {
        _uiState.update { it.copy(crossfadeEnabled = !it.crossfadeEnabled) }
    }

    fun toggleLiquidGlass() {
        _uiState.update { it.copy(liquidGlassEnabled = !it.liquidGlassEnabled) }
    }

    fun setFpsLimit(fps: Int) {
        val resolved = if (fps >= 120 && _uiState.value.supports120Hz) 120 else 60
        _uiState.update { it.copy(fpsLimit = resolved) }
        viewModelScope.launch(Dispatchers.IO) {
            prefs.edit().putInt("fps_limit", resolved).apply()
        }
    }

    fun toggleStarTrack(trackId: String) {
        _uiState.update { state ->
            val updated = if (trackId in state.likedTrackIds) {
                state.likedTrackIds - trackId
            } else {
                state.likedTrackIds + trackId
            }
            state.copy(likedTrackIds = updated)
        }
    }

    fun toggleShuffleAndPlay(tracks: List<AndroidTrack>) {
        if (tracks.isEmpty()) return
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
            runCatching {
                engine.clearLyricsCache()
            }
            _uiState.update { it.copy(status = "Media & lyrics cache cleared") }
        }
    }

    fun completeLogin(cookieHeader: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(showLoginSheet = false, loading = true, status = "Signing in to YouTube Music...")
            }
            runCatching { engine.setYtmCookieHeader(cookieHeader) }
                .onSuccess { profile ->
                    streamCache.clear()
                    cipherWarmupScheduled = false
                    _uiState.update {
                        it.copy(
                            authInitialized = true,
                            signedIn = true,
                            accountProfile = profile,
                            status = profile?.let { p -> "Signed in as ${p.name}" } ?: "Signed in to YouTube Music"
                        )
                    }
                    refreshInBackground(showBlockingSpinner = _uiState.value.shelves.isEmpty())
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = "Sign-in failed: ${err.message}")
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
                    playlists = emptyList(),
                    status = "Signed out"
                )
            }
            refreshInBackground(showBlockingSpinner = false)
        }
    }

    fun onQueryChange(newQuery: String) {
        _uiState.update { it.copy(query = newQuery) }
    }

    fun loadHome() {
        refreshInBackground(showBlockingSpinner = _uiState.value.shelves.isEmpty())
    }

    fun search(overrideQuery: String? = null) {
        val q = (overrideQuery ?: _uiState.value.query).trim()
        if (q.isEmpty()) return
        speculativePrefetchJob?.cancel()
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update { state ->
                val updatedHistory = (listOf(q) + state.recentSearches.filterNot { it.equals(q, ignoreCase = true) }).take(12)
                state.copy(
                    activeTab = AndroidTab.Search,
                    collectionTitle = null,
                    collectionCoverUrl = null,
                    collectionSubtitle = null,
                    collectionKindLabel = null,
                    collectionYear = null,
                    collectionTracks = emptyList(),
                    showLyrics = false,
                    query = q,
                    recentSearches = updatedHistory,
                    loading = true,
                    status = "Searching \"$q\"..."
                )
            }
            runCatching { engine.searchSongs(q) }
                .onSuccess { results ->
                    _uiState.update {
                        it.copy(
                            loading = false,
                            tracks = results,
                            listTitle = "Search: $q",
                            status = "${results.size} songs found"
                        )
                    }
                    prefetchStreams(results.take(2), delayMs = 1_500L)
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = "Search error: ${err.message}")
                    }
                }
        }
    }

    fun openPlaylist(playlist: AndroidPlaylist) {
        openCollectionById(
            id = playlist.id,
            title = playlist.name,
            coverUrl = playlist.thumbnailUrl,
            subtitle = _uiState.value.accountProfile?.name ?: "Playlist",
            kindLabel = "Playlist",
        )
    }

    fun openMood(title: String) {
        search("$title music")
    }

    fun openCard(card: AndroidHomeCard, shelfTracks: List<AndroidTrack> = emptyList()) {
        when (card.kind) {
            AndroidHomeCardKind.SONG, AndroidHomeCardKind.VIDEO -> {
                card.track?.let { track ->
                    val queueSource = shelfTracks.ifEmpty { listOf(track) }
                    playTrack(track, queueSource)
                }
            }
            AndroidHomeCardKind.MOOD -> openMood(card.title)
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
            AndroidHomeCardKind.ARTIST -> search(card.title)
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
        speculativePrefetchJob?.cancel()
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    collectionTitle = title,
                    collectionCoverUrl = coverUrl,
                    collectionSubtitle = subtitle,
                    collectionKindLabel = kindLabel,
                    collectionYear = year,
                    collectionTracks = emptyList(),
                    loading = true,
                    status = "Loading $title..."
                )
            }
            runCatching { engine.fetchCollectionTracks(id) }
                .onSuccess { tracks ->
                    _uiState.update { state ->
                        val fallbackCover = state.collectionCoverUrl ?: tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
                        state.copy(
                            loading = false,
                            collectionTitle = title,
                            collectionCoverUrl = fallbackCover,
                            collectionTracks = tracks,
                            status = "${tracks.size} tracks in $title"
                        )
                    }
                    prefetchStreams(tracks.take(2), delayMs = 1_500L)
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(loading = false, status = "Collection error: ${err.message}")
                    }
                }
        }
    }

    fun prefetchStreams(
        tracks: List<AndroidTrack>,
        delayMs: Long = 1_500L,
        cancelPrevious: Boolean = true,
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
            val engine = requireEngine()
            resolvePrefetchList(engine, candidates)
        }
        if (cancelPrevious) {
            speculativePrefetchJob = job
        }
    }

    private suspend fun resolvePrefetchList(
        engine: SonataAndroidEngine,
        candidates: List<AndroidTrack>,
    ) {
        for (track in candidates) {
            kotlin.coroutines.coroutineContext.ensureActive()
            if (getCachedStream(track.id) != null || !inFlightPrefetch.add(track.id)) continue
            try {
                runCatching { engine.resolveStream(track) }
                    .onSuccess { stream ->
                        putCachedStream(track.id, stream)
                        val itagMatch = ITAG_REGEX.find(stream.url)?.groupValues?.getOrNull(1) ?: "local"
                        Log.i(
                            "SonataStream",
                            "prefetched track=${track.id} title=\"${track.title}\" itag=$itagMatch badge=\"${stream.badge}\""
                        )
                        if (_uiState.value.activeTrack?.id == track.id && !_uiState.value.isPlaying) {
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
        speculativePrefetchJob?.cancel()
        val updatedQueue = when {
            !queueList.isNullOrEmpty() -> {
                if (queueList.any { it.id == track.id }) queueList else listOf(track) + queueList
            }
            _uiState.value.queue.any { it.id == track.id } -> _uiState.value.queue
            else -> listOf(track)
        }
        val updatedIndex = updatedQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

        playbackClock.resetForTrack((track.durationSecs?.toLong() ?: 0L) * 1000L)

        playJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val cachedStream = getCachedStream(track.id)
            saveActiveTrackToPrefs(track, cachedStream?.badge.orEmpty())
            withContext(Dispatchers.Main) {
                controller?.pause()
            }
            _uiState.update {
                it.copy(
                    queue = updatedQueue,
                    queueIndex = updatedIndex,
                    activeTrack = track,
                    streamBadge = cachedStream?.badge ?: "Resolving...",
                    status = if (cachedStream != null) "Playing ${track.title}" else "Resolving ${track.title}...",
                    isPlaying = false,
                    isBuffering = true,
                    isUpgradingQuality = false,
                    lyrics = emptyList(),
                    lyricsLoading = true,
                    lyricsUnavailable = false,
                )
            }

            lyricsJob?.cancel()
            lyricsFetchedForTrackId = null
            // Fetch lyrics immediately only when track duration is already known; otherwise wait for
            // ExoPlayer STATE_READY so provider duration matching rejects mismatched cuts/versions.
            if ((track.durationSecs ?: 0uL) > 0uL) {
                fetchLyricsInBackground(track)
            }

            val streamResult = if (cachedStream != null) {
                Result.success(cachedStream)
            } else {
                runCatching { engine.resolveStream(track) }.onSuccess { resolved ->
                    putCachedStream(track.id, resolved)
                }
            }

            streamResult
                .onSuccess { stream ->
                    if (!isActive || _uiState.value.activeTrack?.id != track.id) return@onSuccess
                    val itag = ITAG_REGEX.find(stream.url)?.groupValues?.getOrNull(1) ?: "unknown"
                    Log.i(
                        "SonataStream",
                        "Playing track='${track.title}' artist='${track.artist}' itag=$itag badge='${stream.badge}'"
                    )
                    saveActiveTrackToPrefs(track, stream.badge)
                    _uiState.update {
                        it.copy(
                            streamBadge = stream.badge,
                            status = "Playing ${track.title}"
                        )
                    }
                    val item = buildMediaItem(track, stream)
                    withContext(Dispatchers.Main) {
                        if (!isActive || _uiState.value.activeTrack?.id != track.id) return@withContext
                        controller?.apply {
                            setMediaItem(item)
                            prepare()
                            play()
                        }
                    }

                    // Delay upcoming queue prefetch so ExoPlayer gets full bandwidth for instant audio start.
                    val upcoming = buildList {
                        for (offset in 1..2) {
                            val nextIdx = updatedIndex + offset
                            if (nextIdx in updatedQueue.indices) add(updatedQueue[nextIdx])
                        }
                    }
                    if (upcoming.isNotEmpty()) {
                        prefetchStreams(upcoming, delayMs = 1_500L, cancelPrevious = true)
                    }

                    // Only upgrade mid-song if the fast-start stream is low quality (<160kbps / Standard).
                    // Already-playing OPUS / >=160kbps streams are never interrupted with setMediaItem.
                    if (_uiState.value.preferHighQuality && isLowQualityBadge(stream.badge)) {
                        _uiState.update {
                            if (it.activeTrack?.id == track.id) it.copy(isUpgradingQuality = true) else it
                        }
                        launch(Dispatchers.IO) {
                            try {
                                val upgraded = runCatching {
                                    engine.resolveHifiUpgrade(track, stream)
                                }.getOrNull()
                                if (upgraded != null && isActive && _uiState.value.activeTrack?.id == track.id) {
                                    putCachedStream(track.id, upgraded)
                                    _uiState.update { it.copy(streamBadge = upgraded.badge) }
                                    val hiFiItem = buildMediaItem(track, upgraded)
                                    withContext(Dispatchers.Main) {
                                        if (!isActive || _uiState.value.activeTrack?.id != track.id) return@withContext
                                        controller?.let { ctrl ->
                                            val pos = ctrl.currentPosition.coerceAtLeast(0L)
                                            val wasPlaying = ctrl.isPlaying
                                            ctrl.setMediaItem(hiFiItem, pos)
                                            ctrl.prepare()
                                            if (wasPlaying) ctrl.play()
                                        }
                                    }
                                }
                            } finally {
                                if (_uiState.value.activeTrack?.id == track.id) {
                                    _uiState.update { it.copy(isUpgradingQuality = false) }
                                }
                            }
                        }
                    }
                }
                .onFailure { err ->
                    _uiState.update {
                        it.copy(
                            isBuffering = false,
                            isUpgradingQuality = false,
                            streamBadge = "Error",
                            status = "Play failed: ${err.message}"
                        )
                    }
                }
        }
    }

    @OptIn(UnstableApi::class)
    private fun buildMediaItem(track: AndroidTrack, stream: AndroidResolvedStream): MediaItem {
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

    private fun isLowQualityBadge(badge: String): Boolean {
        val lower = badge.lowercase()
        if (lower.contains("opus") ||
            lower.contains("hq") ||
            lower.contains("extreme") ||
            lower.contains("hi-res") ||
            lower.contains("hifi") ||
            lower.contains("flac")
        ) {
            return false
        }
        val kbps = KBPS_REGEX.find(lower)?.groupValues?.getOrNull(1)?.toIntOrNull()
        if (kbps != null) return kbps < 160
        return lower.contains("standard")
    }

    fun nextTrack() {
        val state = _uiState.value
        val queue = state.queue
        if (queue.isEmpty()) return
        val nextIndex = when {
            state.shuffleEnabled && queue.size > 1 -> {
                var candidate = Random.nextInt(queue.size)
                if (candidate == state.queueIndex) candidate = (candidate + 1) % queue.size
                candidate
            }
            state.queueIndex + 1 < queue.size -> state.queueIndex + 1
            state.repeatMode == Player.REPEAT_MODE_ALL -> 0
            else -> return
        }
        playTrack(queue[nextIndex], queue)
    }

    fun prevTrack() {
        val state = _uiState.value
        val currentPos = controller?.currentPosition ?: playbackClock.positionMs
        if (currentPos > 3_000L) {
            seekTo(0L)
            return
        }
        val queue = state.queue
        if (queue.isEmpty()) return
        val prevIndex = when {
            state.queueIndex - 1 >= 0 -> state.queueIndex - 1
            state.repeatMode == Player.REPEAT_MODE_ALL -> queue.lastIndex
            else -> {
                seekTo(0L)
                return
            }
        }
        playTrack(queue[prevIndex], queue)
    }

    private fun onTrackEnded() {
        val state = _uiState.value
        if (state.repeatMode == Player.REPEAT_MODE_ONE) {
            seekTo(0L)
            controller?.play()
            return
        }
        if (state.hasNext) {
            nextTrack()
        } else {
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
        controller?.repeatMode = next
        _uiState.update { it.copy(repeatMode = next) }
    }

    fun toggleShuffle() {
        _uiState.update { it.copy(shuffleEnabled = !it.shuffleEnabled) }
    }

    fun togglePlayPause() {
        val ctrl = controller
        val active = _uiState.value.activeTrack
        if (ctrl == null || ctrl.currentMediaItem == null) {
            if (active != null) {
                playTrack(active, _uiState.value.queue)
            }
            return
        }
        if (ctrl.isPlaying) {
            ctrl.pause()
        } else {
            if (ctrl.playbackState == Player.STATE_ENDED) {
                ctrl.seekTo(0L)
            }
            ctrl.play()
        }
    }

    fun seekTo(positionMs: Long) {
        val clamped = positionMs.coerceAtLeast(0L)
        controller?.seekTo(clamped)
        playbackClock.onSeek(clamped)
    }

    fun checkForUpdates() {
        if (_uiState.value.checkingUpdates) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val installedVersion = runCatching {
                val app = getApplication<Application>()
                app.packageManager.getPackageInfo(app.packageName, 0).versionName.orEmpty()
            }.getOrDefault("")
            _uiState.update { it.copy(checkingUpdates = true) }
            runCatching { engine.checkForUpdatesForVersion(installedVersion) }
                .onSuccess { info ->
                    _uiState.update { it.copy(updateInfo = info, checkingUpdates = false) }
                }
                .onFailure {
                    _uiState.update { it.copy(checkingUpdates = false) }
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

    override fun onCleared() {
        PlaybackQueueBridge.canSkipNext = { false }
        PlaybackQueueBridge.canSkipPrevious = { false }
        PlaybackQueueBridge.onSkipNext = {}
        PlaybackQueueBridge.onSkipPrevious = {}
        PlaybackQueueBridge.onPlayFromEmpty = {}
        speculativePrefetchJob?.cancel()
        refreshJob?.cancel()
        playJob?.cancel()
        lyricsJob?.cancel()
        cipherSolver.destroy()
        controller?.release()
        controller = null
        super.onCleared()
    }

    companion object {
        private const val STREAM_CACHE_TTL_MS = 4L * 60L * 60L * 1000L
        private const val STREAM_CACHE_MAX_ENTRIES = 64
        private val YEAR_REGEX = Regex("\\b(19|20)\\d{2}\\b")
        private val ITAG_REGEX = Regex("[?&]itag=(\\d+)")
        private val KBPS_REGEX = Regex("(\\d+)\\s*kbps")
    }
}
