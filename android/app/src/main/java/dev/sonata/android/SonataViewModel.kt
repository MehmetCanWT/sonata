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
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadedTrackInput
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.data.JioSaavnSource
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
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
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.CoroutineScope
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidArtistPage
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
    val radioMixShelves: List<AndroidHomeShelf> = emptyList(),
    val lastPlayedTracks: List<AndroidTrack> = emptyList(),
    val recentArtistCards: List<AndroidHomeCard> = emptyList(),
    val playlists: List<AndroidPlaylist> = emptyList(),
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

    private val engineDeferred = CompletableDeferred<SonataAndroidEngine>()
    private val cipherSolver = WebViewJsCipherSolver(application)

    private val streamCache = ConcurrentHashMap<String, CachedStream>()
    private val artistPageCache = ConcurrentHashMap<String, CachedArtistPage>()
    private val inFlightPrefetch = ConcurrentHashMap.newKeySet<String>()
    private val trackCollectionContext = ConcurrentHashMap<String, String>()
    private val jioToYtmIdCache = ConcurrentHashMap<String, String>()
    private val resolvedArtistCardCache = ConcurrentHashMap<String, AndroidHomeCard>()
    private val inFlightArtistLookups = ConcurrentHashMap.newKeySet<String>()
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

    private var controller: MediaController? = null
    private var playJob: Job? = null
    private var lyricsJob: Job? = null
    private var canvasJob: Job? = null
    private var sleepTimerJob: Job? = null
    private var refreshJob: Job? = null
    private var speculativePrefetchJob: Job? = null
    private var searchDebounceJob: Job? = null
    private var recoveryJob: Job? = null
    private var autoplayJob: Job? = null
    private var lyricsFetchedForTrackId: String? = null
    @Volatile private var recoveryTrackId: String? = null
    @Volatile private var recoveryAttemptsForTrack: Int = 0
    @Volatile private var sleepAtEndOfTrack: Boolean = false
    @Volatile private var cipherWarmupScheduled = false
    @Volatile private var lastHomeExploreFetchAtMs: Long = 0L
    @Volatile private var lastRadioFetchAtMs: Long = 0L
    @Volatile private var rateLimitBackoffUntilMs: Long = 0L
    @Volatile private var homeFetchInFlight: Boolean = false
    @Volatile private var radioFetchInFlight: Boolean = false
    private var radioRefreshJob: Job? = null

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

    private fun mergePlaylistsWithLocal(remote: List<AndroidPlaylist>): List<AndroidPlaylist> {
        val local = loadLocalPlaylistsAsAndroid()
        if (local.isEmpty()) return remote
        return (remote + local).distinctBy { it.id }
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
                subtitle = page.monthlyListeners ?: page.subscribers ?: "Artist",
                thumbnailUrl = page.thumbnailUrl,
                track = null,
            )
            resolvedArtistCardCache[primaryKey] = card
            queryName.trim().lowercase().takeIf { it.isNotBlank() }?.let {
                resolvedArtistCardCache.putIfAbsent(it, card)
            }
        }
    }

    private fun extractPrimaryArtistName(rawArtist: String): String {
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
                    subtitle = "Recently played",
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
                    ?: runCatching { engine.fetchArtistByName(primary) }.getOrNull()
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

    private fun canonicalTrackKey(track: AndroidTrack): String {
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

    private fun deduplicateTracks(tracks: List<AndroidTrack>): List<AndroidTrack> {
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
            runCatching { engine.fetchListeningHistory() }
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
        engine: SonataAndroidEngine,
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

        val match = runCatching { engine.searchSongs(query) }
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
        engine: SonataAndroidEngine,
        tracks: List<AndroidTrack>,
    ): List<String> = tracks.mapNotNull { resolveYtmVideoIdForTrack(engine, it) }.distinct()

    private fun reportTrackPlaybackToYtm(track: AndroidTrack, playlistId: String?) {
        if (track.id.isBlank() || track.id.startsWith("local:")) return
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) return@launch
            val ytmVideoId = resolveYtmVideoIdForTrack(engine, track) ?: return@launch
            runCatching {
                engine.reportPlayback(
                    videoId = ytmVideoId,
                    playlistId = playlistId,
                    durationSecs = track.durationSecs,
                )
            }.onFailure { err ->
                Log.w("SonataTelemetry", "YTM playback report failed for '$ytmVideoId': ${err.message}")
            }
            delay(3_500L)
            runCatching { engine.fetchListeningHistory() }
                .onSuccess { history -> mergeYtmListeningHistory(history) }
                .onFailure { handleRateLimitIfNeeded(it) }
        }
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
        val initialSettings = preferences.state.value
        val resolvedFps = if (initialSettings.fpsLimit >= 120 && supports120Hz) 120 else 60

        _uiState.value = SonataUiState(
            loading = true,
            refreshing = false,
            status = "Loading YouTube Music...",
            preferHighQuality = initialSettings.preferHighQuality,
            animatedArtwork = initialSettings.animatedArtwork,
            crossfadeEnabled = initialSettings.crossfadeEnabled,
            liquidGlassEnabled = initialSettings.liquidGlassEnabled,
            fpsLimit = resolvedFps,
            supports120Hz = supports120Hz,
        )

        applyAudioSettings(initialSettings)
        observePreferences(supports120Hz)
        registerQueueBridge()
        startPositionPoller()
        startColdLaunchBootstrap(application, supports120Hz)
    }

    private fun applyAudioSettings(state: SonataSettingsState) {
        PlaybackAudioBridge.applyPlaybackSpeed(state.playbackSpeed)
        PlaybackAudioBridge.applySkipSilence(state.skipSilenceEnabled)
        PlaybackAudioBridge.applyLoudnessNormalization(state.loudnessNormEnabled)
        PlaybackAudioBridge.applyEqualizer(state.eqEnabled, state.eqBandsDb)
    }

    private fun observePreferences(supports120Hz: Boolean) {
        viewModelScope.launch {
            preferences.state.collect { prefsState ->
                val resolvedFps = if (prefsState.fpsLimit >= 120 && supports120Hz) 120 else 60
                _uiState.update { ui ->
                    if (ui.preferHighQuality == prefsState.preferHighQuality &&
                        ui.animatedArtwork == prefsState.animatedArtwork &&
                        ui.crossfadeEnabled == prefsState.crossfadeEnabled &&
                        ui.liquidGlassEnabled == prefsState.liquidGlassEnabled &&
                        ui.fpsLimit == resolvedFps
                    ) {
                        ui
                    } else {
                        ui.copy(
                            preferHighQuality = prefsState.preferHighQuality,
                            animatedArtwork = prefsState.animatedArtwork,
                            crossfadeEnabled = prefsState.crossfadeEnabled,
                            liquidGlassEnabled = prefsState.liquidGlassEnabled,
                            fpsLimit = resolvedFps,
                        )
                    }
                }
                if (!prefsState.animatedArtwork) {
                    canvasJob?.cancel()
                    _currentCanvas.value = null
                } else if (_currentCanvas.value == null) {
                    _uiState.value.activeTrack?.let { resolveCanvasForTrack(it) }
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
            val savedHistory = loadLocalLastPlayedHistory()
            val initialQueue = listOfNotNull(restoredTrack)
            val initialQueueIndex = if (restoredTrack != null) 0 else -1

            _lastPlayedTracks.value = savedHistory
            if (restoredTrack != null) {
                playbackClock.resetForTrack((restoredTrack.durationSecs?.toLong() ?: 0L) * 1000L)
                resolveCanvasForTrack(restoredTrack)
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
            val installedVer = runCatching {
                application.packageManager.getPackageInfo(application.packageName, 0).versionName.orEmpty()
            }.getOrDefault("").ifBlank { "1.0.0" }
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val cachedProfile = if (signedIn) runCatching { engine.getCachedAccountProfile() }.getOrNull() else null
            val cachedPlaylists = if (signedIn) runCatching { engine.getCachedLibraryPlaylists() }.getOrDefault(emptyList()) else emptyList()
            val mergedCachedPlaylists = mergePlaylistsWithLocal(cachedPlaylists)

            _uiState.update {
                it.copy(
                    authInitialized = true,
                    signedIn = signedIn,
                    installedVersionName = installedVer,
                    accountProfile = cachedProfile ?: it.accountProfile,
                    playlists = if (it.playlists.isEmpty()) mergedCachedPlaylists else it.playlists,
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
            // Warm the WebView cipher solver promptly so signed-in TVHTML5 Extreme streams work on the first tap.
            val engine = requireEngine()
            if (!cipherWarmupScheduled) {
                cipherWarmupScheduled = true
                delay(300L)
                runCatching { engine.warmCipherSolver() }
            }
            delay(1_800L)
            resolvePrefetchList(engine, candidates.distinctBy { it.id }.take(2))
        }
    }

    private fun normalizeAndMergeHomeShelves(rawShelves: List<AndroidHomeShelf>): List<AndroidHomeShelf> {
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

    private fun normalizeRadioMixShelves(rawMixes: List<AndroidHomeShelf>): List<AndroidHomeShelf> {
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

    private fun handleRateLimitIfNeeded(err: Throwable) {
        val msg = err.message.orEmpty()
        if ("429" in msg || "too many requests" in msg.lowercase()) {
            rateLimitBackoffUntilMs = SystemClock.elapsedRealtime() + RATE_LIMIT_BACKOFF_MS
            Log.w("SonataRateLimit", "HTTP 429 received; backing off feed requests for ${RATE_LIMIT_BACKOFF_MS / 1000}s")
        }
    }

    fun refreshHomeAndExploreRateLimited(forceManual: Boolean = false, showBlockingSpinner: Boolean = false) {
        val now = SystemClock.elapsedRealtime()
        if (now < rateLimitBackoffUntilMs || homeFetchInFlight) return
        val cooldown = if (forceManual) MANUAL_REFRESH_COOLDOWN_MS else AUTO_REFRESH_COOLDOWN_MS
        if (!showBlockingSpinner && _uiState.value.shelves.isNotEmpty() && now - lastHomeExploreFetchAtMs < cooldown) {
            return
        }
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
                        runCatching { engine.getAccountProfile() }
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
                        var playlists = runCatching { engine.fetchLibraryPlaylists() }
                            .onFailure { err ->
                                handleRateLimitIfNeeded(err)
                                Log.w("SonataLibrary", "Library playlists fetch failed: ${err.message}")
                            }
                            .getOrNull()
                        if ((playlists == null || playlists.size <= 1) && SystemClock.elapsedRealtime() >= rateLimitBackoffUntilMs) {
                            delay(600L)
                            playlists = runCatching { engine.fetchLibraryPlaylists() }
                                .onFailure { err ->
                                    handleRateLimitIfNeeded(err)
                                    Log.w("SonataLibrary", "Library playlists retry failed: ${err.message}")
                                }
                                .getOrNull() ?: playlists
                        }
                        if (playlists != null) {
                            val merged = mergePlaylistsWithLocal(playlists)
                            if (merged != _uiState.value.playlists) {
                                _uiState.update { it.copy(playlists = merged) }
                            }
                        }
                    }
                    launch {
                        runCatching { engine.fetchListeningHistory() }
                            .onSuccess { history ->
                                mergeYtmListeningHistory(history)
                            }
                            .onFailure { err ->
                                handleRateLimitIfNeeded(err)
                                Log.w("SonataLibrary", "Listening history fetch failed: ${err.message}")
                            }
                    }
                } else {
                    val localOnly = loadLocalPlaylistsAsAndroid()
                    if (localOnly != _uiState.value.playlists) {
                        _uiState.update { it.copy(playlists = localOnly) }
                    }
                }

                runCatching { engine.fetchHomeFeed() }
                    .onSuccess { rawShelves ->
                        lastHomeExploreFetchAtMs = SystemClock.elapsedRealtime()
                        val cleanedShelves = normalizeAndMergeHomeShelves(rawShelves)
                        val existingShelves = _uiState.value.shelves
                        val shelves = if (cleanedShelves == existingShelves) existingShelves else cleanedShelves
                        val firstTracks = shelves.firstOrNull { it.tracks.isNotEmpty() }?.tracks.orEmpty()
                        val currentActive = _uiState.value.activeTrack
                        val fallbackActive = currentActive ?: firstTracks.firstOrNull()
                        if (currentActive == null && fallbackActive != null) {
                            saveActiveTrackToPrefs(fallbackActive)
                            playbackClock.resetForTrack((fallbackActive.durationSecs?.toLong() ?: 0L) * 1000L)
                            fetchLyricsInBackground(fallbackActive)
                        }
                        val updatedArtists = buildRecentArtistCards(_lastPlayedTracks.value, shelves)
                        _uiState.update {
                            it.copy(
                                loading = false,
                                refreshing = false,
                                authInitialized = true,
                                signedIn = signedIn,
                                shelves = shelves,
                                recentArtistCards = updatedArtists,
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
                        handleRateLimitIfNeeded(err)
                        _uiState.update {
                            it.copy(
                                loading = false,
                                refreshing = false,
                                authInitialized = true,
                                status = if (it.shelves.isEmpty()) "Home error: ${err.message}" else it.status
                            )
                        }
                    }
            } finally {
                homeFetchInFlight = false
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
                runCatching { engine.fetchRadioMixes() }
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

                    override fun onPlayerError(error: PlaybackException) {
                        val active = _uiState.value.activeTrack ?: return
                        if (downloadManager.localUriFor(active.id) != null) {
                            _uiState.update {
                                it.copy(
                                    isPlaying = false,
                                    isBuffering = false,
                                    status = "Offline playback error: ${error.message}",
                                )
                            }
                            return
                        }
                        if (recoveryTrackId != active.id) {
                            recoveryTrackId = active.id
                            recoveryAttemptsForTrack = 0
                        }
                        if (recoveryAttemptsForTrack >= MAX_RECOVERY_ATTEMPTS) {
                            _uiState.update {
                                it.copy(
                                    isPlaying = false,
                                    isBuffering = false,
                                    status = "Playback stopped: ${error.message}",
                                )
                            }
                            return
                        }
                        recoveryAttemptsForTrack += 1
                        val resumePositionMs = ctrl.currentPosition.coerceAtLeast(0L)
                        val oldBadge = _uiState.value.streamBadge
                        Log.w(
                            "SonataStream",
                            "Recovering track=${active.id} attempt=$recoveryAttemptsForTrack posMs=$resumePositionMs cause=${error.errorCodeName}"
                        )
                        recoveryJob?.cancel()
                        recoveryJob = viewModelScope.launch(Dispatchers.IO) {
                            val engine = requireEngine()
                            streamCache.remove(active.id)
                            if (oldBadge.isNotBlank()) {
                                PlaybackService.evictCacheKey("${active.id}:$oldBadge")
                            }
                            _uiState.update {
                                if (it.activeTrack?.id == active.id) {
                                    it.copy(isBuffering = true, status = "Recovering stream...")
                                } else it
                            }
                            runCatching { engine.resolveRecovery(active) }
                                .onSuccess { recovered ->
                                    if (!isActive || _uiState.value.activeTrack?.id != active.id) return@onSuccess
                                    putCachedStream(active.id, recovered)
                                    saveActiveTrackToPrefs(active, recovered.badge)
                                    _uiState.update {
                                        it.copy(
                                            streamBadge = recovered.badge,
                                            status = "Playing ${active.title}",
                                        )
                                    }
                                    val recoveredItem = buildMediaItem(active, recovered)
                                    withContext(Dispatchers.Main) {
                                        if (!isActive || _uiState.value.activeTrack?.id != active.id) return@withContext
                                        ctrl.setMediaItem(recoveredItem, resumePositionMs)
                                        ctrl.prepare()
                                        ctrl.play()
                                    }
                                }
                                .onFailure { recErr ->
                                    Log.w("SonataStream", "Stream recovery failed: ${recErr.message}")
                                    _uiState.update {
                                        it.copy(
                                            isPlaying = false,
                                            isBuffering = false,
                                            status = "Playback error: ${recErr.message}",
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

    fun downloadTrack(track: AndroidTrack, collectionId: String? = null) {
        viewModelScope.launch(Dispatchers.IO) {
            if (downloadManager.isDownloaded(track.id)) {
                _uiState.update {
                    it.copy(
                        status = "Already downloaded: ${track.title}",
                        addToPlaylistStatus = "Already downloaded",
                    )
                }
                return@launch
            }
            _uiState.update {
                it.copy(
                    status = "Downloading ${track.title}...",
                    addToPlaylistStatus = "Downloading ${track.title}...",
                )
            }
            val engine = requireEngine()
            var stream = if (track.id.startsWith("jiosaavn:")) {
                JioSaavnSource.resolveById(track.id)?.let { js ->
                    AndroidResolvedStream(
                        url = js.url,
                        badge = "AAC ${js.bitrateKbps}kbps · JioSaavn",
                        headers = emptyList(),
                    )
                } ?: run {
                    _uiState.update {
                        it.copy(
                            status = "Download failed: JioSaavn unavailable",
                            addToPlaylistStatus = "Download failed",
                        )
                    }
                    return@launch
                }
            } else {
                getCachedStream(track.id) ?: runCatching { engine.resolveStream(track) }
                    .onSuccess { putCachedStream(track.id, it) }
                    .getOrElse { err ->
                        _uiState.update {
                            it.copy(
                                status = "Download failed: ${err.message}",
                                addToPlaylistStatus = "Download failed",
                            )
                        }
                        return@launch
                    }
            }
            if (!track.id.startsWith("jiosaavn:") &&
                settings.value.downloadQuality == DownloadQualityTier.HIGH &&
                shouldUpgradeStreamBadge(stream.badge, _uiState.value.signedIn)
            ) {
                runCatching { engine.resolveHifiUpgrade(track, stream) }
                    .getOrNull()
                    ?.let { upgraded ->
                        stream = upgraded
                        putCachedStream(track.id, upgraded)
                    }
            }
            val cleanUrl = stripRangeQueryParam(stream.url)
            val input = DownloadedTrackInput(
                videoId = track.id,
                title = track.title,
                artist = track.artist,
                album = track.album.ifBlank { null },
                durationSecs = track.durationSecs?.toLong() ?: 0L,
                coverUrl = track.thumbnailUrl,
                codec = stream.badge.ifBlank { null },
            )
            var result = downloadManager.downloadTrack(
                track = input,
                streamUrl = cleanUrl,
                exportToMusic = settings.value.exportDownloads,
                collectionId = collectionId,
            )
            if (result.isFailure && !track.id.startsWith("jiosaavn:")) {
                val recovered = runCatching { engine.resolveRecovery(track) }.getOrNull()
                if (recovered != null) {
                    putCachedStream(track.id, recovered)
                    result = downloadManager.downloadTrack(
                        track = input.copy(codec = recovered.badge.ifBlank { input.codec }),
                        streamUrl = stripRangeQueryParam(recovered.url),
                        exportToMusic = settings.value.exportDownloads,
                        collectionId = collectionId,
                    )
                }
            }
            result.onSuccess {
                _uiState.update {
                    it.copy(
                        status = "Downloaded ${track.title}",
                        addToPlaylistStatus = "Downloaded ${track.title}",
                    )
                }
            }.onFailure { err ->
                Log.w("SonataDownload", "downloadTrack failed id=${track.id}: ${err.message}")
                _uiState.update {
                    it.copy(
                        status = "Download failed: ${err.message}",
                        addToPlaylistStatus = "Download failed",
                    )
                }
            }
        }
    }

    fun downloadCollection(tracks: List<AndroidTrack>, collectionId: String) {
        if (tracks.isEmpty()) return
        viewModelScope.launch(Dispatchers.IO) {
            val unique = tracks.distinctBy { it.id }
            val pending = unique.filterNot { downloadManager.isDownloaded(it.id) }
            if (pending.isEmpty()) {
                _uiState.update {
                    it.copy(
                        status = "All ${unique.size} tracks already downloaded",
                        addToPlaylistStatus = "All tracks already downloaded",
                    )
                }
                return@launch
            }
            _uiState.update {
                it.copy(
                    status = "Downloading ${pending.size} tracks...",
                    addToPlaylistStatus = "Downloading ${pending.size} tracks...",
                )
            }
            val engine = requireEngine()
            var savedCount = 0
            for (track in pending) {
                if (!isActive) break
                var stream = getCachedStream(track.id) ?: runCatching { engine.resolveStream(track) }
                    .onSuccess { putCachedStream(track.id, it) }
                    .getOrNull() ?: continue
                if (settings.value.downloadQuality == DownloadQualityTier.HIGH &&
                    shouldUpgradeStreamBadge(stream.badge, _uiState.value.signedIn)
                ) {
                    runCatching { engine.resolveHifiUpgrade(track, stream) }
                        .getOrNull()
                        ?.let { upgraded ->
                            stream = upgraded
                            putCachedStream(track.id, upgraded)
                        }
                }
                val input = DownloadedTrackInput(
                    videoId = track.id,
                    title = track.title,
                    artist = track.artist,
                    album = track.album.ifBlank { null },
                    durationSecs = track.durationSecs?.toLong() ?: 0L,
                    coverUrl = track.thumbnailUrl,
                    codec = stream.badge.ifBlank { null },
                )
                var res = downloadManager.downloadTrack(
                    track = input,
                    streamUrl = stripRangeQueryParam(stream.url),
                    exportToMusic = settings.value.exportDownloads,
                    collectionId = collectionId,
                )
                if (res.isFailure) {
                    val recovered = runCatching { engine.resolveRecovery(track) }.getOrNull()
                    if (recovered != null) {
                        putCachedStream(track.id, recovered)
                        res = downloadManager.downloadTrack(
                            track = input.copy(codec = recovered.badge.ifBlank { input.codec }),
                            streamUrl = stripRangeQueryParam(recovered.url),
                            exportToMusic = settings.value.exportDownloads,
                            collectionId = collectionId,
                        )
                    }
                }
                if (res.isSuccess) {
                    savedCount++
                } else {
                    Log.w("SonataDownload", "downloadCollection track failed id=${track.id}: ${res.exceptionOrNull()?.message}")
                }
            }
            val msg = if (savedCount > 0) {
                "Downloaded $savedCount of ${pending.size} tracks"
            } else {
                "Download failed"
            }
            _uiState.update { it.copy(status = msg, addToPlaylistStatus = msg) }
        }
    }

    private fun stripRangeQueryParam(url: String): String {
        if (!url.contains("range=", ignoreCase = true)) return url
        return url
            .replace(Regex("([?&])range=[^&]*&?"), "$1")
            .trimEnd('&', '?')
    }

    private fun parseBitrateKbps(badge: String): Int =
        Regex("(\\d{2,4})\\s*kbps", RegexOption.IGNORE_CASE)
            .find(badge)
            ?.groupValues
            ?.getOrNull(1)
            ?.toIntOrNull()
            ?: 0

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
            _uiState.update { it.copy(status = "Downloads cleared") }
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
            _uiState.update { it.copy(status = "Starting radio...") }

            if (seedTrack != null) {
                val radioTracks = runCatching { engine.fetchRadioQueue(seedTrack.id) }
                    .getOrDefault(emptyList())
                val radioQueue = (listOf(seedTrack) + radioTracks).distinctBy { it.id }
                if (radioQueue.size > 1) {
                    playTrack(radioQueue.first(), radioQueue)
                    return@launch
                }
                val query = listOf(seedTrack.artist, seedTrack.title, "mix")
                    .filter { it.isNotBlank() }
                    .joinToString(" ")
                val results = runCatching { engine.searchSongs(query) }.getOrDefault(emptyList())
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
                val mixTracks = runCatching { engine.fetchCollectionTracks(mixCard.id) }
                    .getOrDefault(emptyList())
                if (mixTracks.isNotEmpty()) {
                    playTrack(mixTracks.first(), mixTracks)
                    return@launch
                }
            }

            val fallbackTracks = _uiState.value.shelves
                .flatMap { it.tracks }
                .ifEmpty { runCatching { engine.searchSongs("top hits radio mix") }.getOrDefault(emptyList()) }
                .distinctBy { it.id }

            if (fallbackTracks.isNotEmpty()) {
                playTrack(fallbackTracks.first(), fallbackTracks)
            } else {
                _uiState.update { it.copy(status = "Could not start radio right now") }
            }
        }
    }

    fun setSleepTimer(minutes: Int?) {
        sleepTimerJob?.cancel()
        sleepTimerJob = null
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
            var remaining = minutes
            while (remaining > 0 && isActive) {
                delay(60_000L)
                remaining -= 1
                _sleepTimerRemainingMinutes.value = if (remaining > 0) remaining else null
            }
            if (isActive) {
                withContext(Dispatchers.Main) {
                    controller?.pause()
                }
                _sleepTimerRemainingMinutes.value = null
                _uiState.update { it.copy(status = "Sleep timer paused playback") }
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

    private fun resolveCanvasForTrack(track: AndroidTrack) {
        canvasJob?.cancel()
        val trackId = track.id
        requestedCanvasTrackId = trackId
        _currentCanvas.value = null
        if (!settings.value.animatedArtwork) return
        canvasJob = viewModelScope.launch(Dispatchers.IO) {
            val resolved = runCatching {
                CanvasResolver.resolve(
                    videoId = trackId,
                    title = track.title,
                    artist = track.artist,
                    album = track.album.ifBlank { null },
                )
            }.getOrNull()
            if (isActive && settings.value.animatedArtwork && requestedCanvasTrackId == trackId) {
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
        if (!trackId.startsWith("local:")) {
            val snapshotTrack = _uiState.value.let { state ->
                listOfNotNull(state.activeTrack).firstOrNull { it.id == trackId }
                    ?: state.queue.firstOrNull { it.id == trackId }
                    ?: state.collectionTracks.firstOrNull { it.id == trackId }
                    ?: state.tracks.firstOrNull { it.id == trackId }
                    ?: state.lastPlayedTracks.firstOrNull { it.id == trackId }
            }
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
                    runCatching { engine.setLikeStatus(ytmId, nowLiked) }
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
        val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
        val cleanPlaylistIds = playlistIds.map { it.trim() }.filter { it.isNotBlank() }.distinct()
        if (cleanTracks.isEmpty() || cleanPlaylistIds.isEmpty()) {
            onComplete(false, "Select at least one playlist")
            return
        }
        val videoIds = cleanTracks.map { it.id }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val resolvedVideoIds = if (signedIn) resolveYtmVideoIds(engine, cleanTracks) else videoIds
            _uiState.update { it.copy(addToPlaylistStatus = "Adding to playlist...") }

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
                            if (ok) Result.success(Unit) else Result.failure(IllegalStateException("Sign in to edit YouTube Music playlists"))
                        } else if (resolvedVideoIds.isEmpty()) {
                            Result.failure(IllegalStateException("No YouTube Music track match found"))
                        } else {
                            runCatching { engine.addTracksToPlaylist(plId, resolvedVideoIds) }
                        }
                    }
                }.awaitAll()
            }

            results.forEach { res ->
                res.onSuccess {
                    successCount++
                }.onFailure { err ->
                    if (firstError == null) firstError = err.message ?: "Failed to update playlist"
                }
            }

            val updatedRemote = if (signedIn && successCount > 0) {
                runCatching { engine.fetchLibraryPlaylists() }.getOrNull()
            } else {
                null
            }
            val ok = successCount == cleanPlaylistIds.size
            val message = when {
                ok -> if (cleanTracks.size == 1) {
                    "Added ${cleanTracks.first().title} to ${cleanPlaylistIds.size} playlist(s)"
                } else {
                    "Added ${cleanTracks.size} tracks to ${cleanPlaylistIds.size} playlist(s)"
                }
                successCount > 0 -> "Added to $successCount of ${cleanPlaylistIds.size} playlists"
                else -> firstError ?: "Could not add to playlist"
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
        val cleanTitle = title.trim()
        if (cleanTitle.isEmpty()) {
            onComplete(false, "Playlist name cannot be empty")
            return
        }
        val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
        val videoIds = cleanTracks.map { it.id }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val signedIn = runCatching { engine.isSignedIn() }.getOrDefault(false)
            val resolvedVideoIds = if (signedIn) resolveYtmVideoIds(engine, cleanTracks) else videoIds
            _uiState.update { it.copy(addToPlaylistStatus = "Creating $cleanTitle...") }

            val remoteResult = if (signedIn) {
                runCatching { engine.createPlaylist(cleanTitle, "", resolvedVideoIds) }
            } else {
                null
            }

            if (remoteResult != null && remoteResult.isSuccess) {
                val createdId = remoteResult.getOrNull().orEmpty()
                val updatedRemote = runCatching { engine.fetchLibraryPlaylists() }.getOrNull()
                val createdThumb = cleanTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
                val message = "Created \"$cleanTitle\""
                _uiState.update { state ->
                    val base = updatedRemote ?: (listOf(AndroidPlaylist(createdId, cleanTitle, createdThumb)) + state.playlists)
                    state.copy(
                        playlists = mergePlaylistsWithLocal(base),
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
                val message = "Created offline playlist \"$cleanTitle\""
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
                val errMsg = remoteResult?.exceptionOrNull()?.message ?: "Failed to create playlist"
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
            _uiState.update { it.copy(status = "Removed from cache") }
        }
    }

    private suspend fun fetchTracksForCollectionAction(
        engine: SonataAndroidEngine,
        browseId: String,
    ): List<AndroidTrack> {
        return when {
            browseId == DOWNLOADS_PLAYLIST_ID -> downloads.value.map { it.toAndroidTrack() }
            browseId.startsWith(LOCAL_PLAYLIST_PREFIX) -> getLocalPlaylistTracks(browseId)
            else -> runCatching { engine.fetchCollectionTracks(browseId) }.getOrDefault(emptyList())
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
                _uiState.update { it.copy(status = "No tracks found in $title") }
                return@launch
            }
            val backfilled = backfillCollectionTracks(
                rawTracks = tracks,
                id = browseId,
                title = title.ifBlank { "Playlist" },
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
                it.copy(queue = mutable, status = "Added ${tracks.size} tracks to Play Next")
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
                    status = "Added ${tracks.size} tracks to Queue",
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
        _uiState.update { it.copy(queue = mutable, status = "Playing next: ${track.title}") }
    }

    fun addToQueue(track: AndroidTrack) {
        val current = _uiState.value
        val queue = current.queue
        if (queue.isEmpty() || current.activeTrack == null) {
            playTrack(track, listOf(track))
            return
        }
        _uiState.update {
            it.copy(queue = queue + track, status = "Added ${track.title} to queue")
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
                            status = profile?.let { p -> "Signed in as ${p.name}" } ?: "Signed in to YouTube Music"
                        )
                    }
                    refreshHomeAndExploreRateLimited(forceManual = true, showBlockingSpinner = _uiState.value.shelves.isEmpty())
                    refreshRadioRateLimited(forceManual = true)
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
                    playlists = loadLocalPlaylistsAsAndroid(),
                    status = "Signed out"
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

    fun loadHome() {
        refreshHomeAndExploreRateLimited(
            forceManual = true,
            showBlockingSpinner = _uiState.value.shelves.isEmpty(),
        )
        refreshRadioRateLimited(forceManual = true)
    }

    fun search(overrideQuery: String? = null) {
        val q = (overrideQuery ?: _uiState.value.query).trim()
        if (q.isEmpty()) return
        speculativePrefetchJob?.cancel()
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val includeJioSaavn = settings.value.jioSaavnEnabled
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
                    showLyrics = false,
                    query = q,
                    recentSearches = updatedHistory,
                    loading = true,
                    status = "Searching \"$q\"..."
                )
            }
            val (ytResult, jioTracks) = coroutineScope {
                val ytDeferred = async { runCatching { engine.searchSongs(q) } }
                val jioDeferred = if (includeJioSaavn) {
                    async {
                        withTimeoutOrNull(4_000L) {
                            JioSaavnSource.searchTracks(q, limit = 6)
                        }.orEmpty()
                    }
                } else {
                    null
                }
                ytDeferred.await() to (jioDeferred?.await().orEmpty())
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
                            listTitle = "Search: $q",
                            status = if (includeJioSaavn && jioTracks.isNotEmpty()) {
                                "${merged.size} songs found (YTM + JioSaavn)"
                            } else {
                                "${merged.size} songs found"
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
                                listTitle = "Search: $q",
                                status = "${jioTracks.size} songs found (JioSaavn)"
                            )
                        }
                    } else {
                        _uiState.update {
                            it.copy(loading = false, status = "Search error: ${err.message}")
                        }
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

    fun openDownloadsPlaylist() {
        openCollectionById(
            id = DOWNLOADS_PLAYLIST_ID,
            title = "Downloads",
            coverUrl = downloads.value.firstOrNull()?.coverUrl,
            subtitle = "${downloads.value.size} offline songs",
            kindLabel = "Offline Library",
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
        if (kindLabel.equals("Album", ignoreCase = true) &&
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
        val cleanId = albumId.trim()
        val cleanTitle = albumTitle.trim()
        if (cleanId.startsWith("MPRE") ||
            cleanId.startsWith("OLAK") ||
            cleanId.startsWith("VL") ||
            cleanId.startsWith("PL")
        ) {
            openCollectionById(
                id = cleanId,
                title = cleanTitle.ifBlank { "Album" },
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
                    collectionTitle = cleanTitle,
                    collectionCoverUrl = coverUrl,
                    collectionSubtitle = artistName.takeIf { s -> s.isNotBlank() },
                    collectionKindLabel = "Album",
                    collectionYear = year,
                    collectionTracks = emptyList(),
                    loading = true,
                    status = "Opening $cleanTitle...",
                )
            }
            val resolvedId = runCatching {
                engine.resolveAlbumBrowseId(cleanTitle, artistName)
            }.getOrNull()
            if (!resolvedId.isNullOrBlank()) {
                openCollectionById(
                    id = resolvedId,
                    title = cleanTitle,
                    coverUrl = coverUrl,
                    subtitle = artistName.takeIf { s -> s.isNotBlank() },
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
                search(listOf(cleanTitle, artistName).filter { s -> s.isNotBlank() }.joinToString(" "))
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
                    artistLoading = false,
                    artistPage = cached,
                    status = cached.name,
                )
            }
            prefetchStreams(cached.popularTracks.take(2), delayMs = 1_000L)
            return
        }

        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    collectionBrowseId = null,
                    collectionTitle = null,
                    collectionTracks = emptyList(),
                    artistLoading = true,
                    artistPage = AndroidArtistPage(
                        id = cleanBrowseId,
                        name = cleanName.ifBlank { "Artist" },
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
                    status = "Loading ${cleanName.ifBlank { "artist" }}...",
                )
            }
            val result = runCatching {
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
                                status = "Artist error: ${err.message}",
                            )
                        }
                    }
                }
        }
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
        speculativePrefetchJob?.cancel()
        if (id == DOWNLOADS_PLAYLIST_ID) {
            val offlineTracks = downloads.value.map { it.toAndroidTrack() }
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    collectionBrowseId = id,
                    collectionTitle = "Downloads",
                    collectionCoverUrl = coverUrl ?: offlineTracks.firstOrNull()?.thumbnailUrl,
                    collectionSubtitle = "${offlineTracks.size} offline songs",
                    collectionKindLabel = "Offline Library",
                    collectionYear = null,
                    collectionTracks = offlineTracks,
                    loading = false,
                    status = "${offlineTracks.size} downloaded songs",
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
                    collectionBrowseId = id,
                    collectionTitle = title,
                    collectionCoverUrl = coverUrl ?: localTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
                    collectionSubtitle = "${localTracks.size} offline playlist songs",
                    collectionKindLabel = kindLabel ?: "Offline Playlist",
                    collectionYear = year,
                    collectionTracks = localTracks,
                    loading = false,
                    status = "${localTracks.size} tracks in $title",
                )
            }
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    collectionBrowseId = id,
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
                .onSuccess { rawTracks ->
                    val fallbackCover = coverUrl ?: rawTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
                    val tracks = backfillCollectionTracks(
                        rawTracks = rawTracks,
                        id = id,
                        title = title,
                        subtitle = subtitle,
                        coverUrl = fallbackCover,
                    )
                    if (trackCollectionContext.size > 512) {
                        trackCollectionContext.clear()
                    }
                    tracks.forEach { t ->
                        if (t.id.isNotBlank()) trackCollectionContext[t.id] = id
                    }
                    _uiState.update { state ->
                        val resolvedHeaderCover = state.collectionCoverUrl ?: fallbackCover
                        state.copy(
                            loading = false,
                            collectionBrowseId = id,
                            collectionTitle = title,
                            collectionCoverUrl = resolvedHeaderCover,
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
        recoveryJob?.cancel()
        autoplayJob?.cancel()
        speculativePrefetchJob?.cancel()
        recoveryTrackId = track.id
        recoveryAttemptsForTrack = 0
        val updatedQueue = when {
            !queueList.isNullOrEmpty() -> {
                if (queueList.any { it.id == track.id }) queueList else listOf(track) + queueList
            }
            _uiState.value.queue.any { it.id == track.id } -> _uiState.value.queue
            else -> listOf(track)
        }
        val updatedIndex = updatedQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

        playbackClock.resetForTrack((track.durationSecs?.toLong() ?: 0L) * 1000L)
        resolveCanvasForTrack(track)

        playJob = viewModelScope.launch(Dispatchers.IO) {
            recordLastPlayed(track)
            val playbackPlaylistId = resolvePlaybackPlaylistId(track, queueList)
            val offlineUri = downloadManager.localUriFor(track.id)
            if (offlineUri != null) {
                val offlineRecord = downloads.value.firstOrNull { it.videoId == track.id }
                val offlineBadge = offlineRecord?.codec?.takeIf { it.isNotBlank() }?.let { "Offline · $it" } ?: "Offline"
                saveActiveTrackToPrefs(track, offlineBadge)
                withContext(Dispatchers.Main) {
                    runCatching { controller?.pause() }
                }
                _uiState.update {
                    it.copy(
                        queue = updatedQueue,
                        queueIndex = updatedIndex,
                        activeTrack = track,
                        streamBadge = offlineBadge,
                        status = "Playing ${track.title} (Offline)",
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
                if ((track.durationSecs ?: 0uL) > 0uL) {
                    fetchLyricsInBackground(track)
                }
                val offlineItem = buildOfflineMediaItem(track, offlineUri)
                withContext(Dispatchers.Main) {
                    if (!isActive || _uiState.value.activeTrack?.id != track.id) return@withContext
                    controller?.apply {
                        setMediaItem(offlineItem)
                        prepare()
                        play()
                    }
                }
                reportTrackPlaybackToYtm(track, playbackPlaylistId)
                val engine = requireEngine()
                launchBackgroundQualityUpgrade(
                    scope = this,
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

            val (effectiveTrack, effectiveQueue, streamResult) = if (track.id.startsWith("jiosaavn:")) {
                val resolved = if (cachedStream != null) {
                    Result.success(cachedStream)
                } else {
                    runCatching {
                        val js = JioSaavnSource.resolveById(track.id)
                            ?: error("JioSaavn stream unavailable")
                        AndroidResolvedStream(
                            url = js.url,
                            badge = "AAC ${js.bitrateKbps}kbps · JioSaavn",
                            headers = emptyList(),
                        )
                    }.onSuccess { putCachedStream(track.id, it) }
                }
                Triple(track, updatedQueue, resolved)
            } else coroutineScope {
                val baseStreamDeferred = async {
                    if (cachedStream != null) {
                        Result.success(cachedStream)
                    } else {
                        runCatching { engine.resolveStream(track) }.onSuccess { resolved ->
                            putCachedStream(track.id, resolved)
                        }
                    }
                }
                val explicitCounterpartDeferred = if (!track.isExplicit && track.title.isNotBlank()) {
                    async {
                        withTimeoutOrNull(1_200L) {
                            runCatching { engine.resolveExplicitCounterpart(track) }.getOrNull()
                        }
                    }
                } else {
                    null
                }
                val explicitMatch = explicitCounterpartDeferred?.await()?.takeIf { it.id != track.id }
                if (explicitMatch != null) {
                    Log.i(
                        "SonataStream",
                        "Preferring explicit track id=${explicitMatch.id} title='${explicitMatch.title}' over id=${track.id}"
                    )
                    val upgradedQueue = updatedQueue.toMutableList().apply {
                        if (updatedIndex in indices) set(updatedIndex, explicitMatch)
                    }
                    recoveryTrackId = explicitMatch.id
                    recordLastPlayed(explicitMatch)
                    resolveCanvasForTrack(explicitMatch)
                    _uiState.update { state ->
                        if (state.activeTrack?.id == track.id) {
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
                        runCatching { engine.resolveStream(explicitMatch) }.onSuccess { resolved ->
                            putCachedStream(explicitMatch.id, resolved)
                        }
                    }
                    Triple(explicitMatch, upgradedQueue, explicitStream)
                } else {
                    Triple(track, updatedQueue, baseStreamDeferred.await())
                }
            }

            streamResult
                .onSuccess { stream ->
                    if (!isActive || _uiState.value.activeTrack?.id != effectiveTrack.id) return@onSuccess
                    val itag = ITAG_REGEX.find(stream.url)?.groupValues?.getOrNull(1) ?: "unknown"
                    Log.i(
                        "SonataStream",
                        "Playing track='${effectiveTrack.title}' artist='${effectiveTrack.artist}' explicit=${effectiveTrack.isExplicit} itag=$itag badge='${stream.badge}'"
                    )
                    saveActiveTrackToPrefs(effectiveTrack, stream.badge)
                    _uiState.update {
                        it.copy(
                            streamBadge = stream.badge,
                            status = "Playing ${effectiveTrack.title}"
                        )
                    }
                    val item = buildMediaItem(effectiveTrack, stream)
                    withContext(Dispatchers.Main) {
                        if (!isActive || _uiState.value.activeTrack?.id != effectiveTrack.id) return@withContext
                        runCatching {
                            controller?.apply {
                                setMediaItem(item)
                                prepare()
                                play()
                            }
                        }.onFailure { err ->
                            Log.w("SonataStream", "Controller setMediaItem failed: ${err.message}")
                        }
                    }
                    reportTrackPlaybackToYtm(effectiveTrack, playbackPlaylistId)

                    // Delay upcoming queue prefetch so ExoPlayer gets full bandwidth for instant audio start.
                    val upcoming = buildList {
                        for (offset in 1..2) {
                            val nextIdx = updatedIndex + offset
                            if (nextIdx in effectiveQueue.indices) add(effectiveQueue[nextIdx])
                        }
                    }
                    if (upcoming.isNotEmpty()) {
                        prefetchStreams(upcoming, delayMs = 1_500L, cancelPrevious = true)
                    }

                    launchBackgroundQualityUpgrade(
                        scope = this,
                        engine = engine,
                        effectiveTrack = effectiveTrack,
                        currentStream = stream,
                    )
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
        scope: CoroutineScope,
        engine: SonataAndroidEngine,
        effectiveTrack: AndroidTrack,
        currentStream: AndroidResolvedStream,
    ) {
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

        _uiState.update {
            if (it.activeTrack?.id == effectiveTrack.id) it.copy(isUpgradingQuality = true) else it
        }
        scope.launch(Dispatchers.IO) {
            try {
                var upgraded: AndroidResolvedStream? = if (allowYtmUpgrade) {
                    val probeStream = if (isOfflinePlayback) {
                        currentStream.copy(url = "https://offline.local", badge = "AAC ${baselineKbps}kbps · Cached")
                    } else {
                        currentStream
                    }
                    runCatching { engine.resolveHifiUpgrade(effectiveTrack, probeStream) }
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

                if (allowJioSaavnUpgrade && isActive && _uiState.value.activeTrack?.id == effectiveTrack.id) {
                    val bestSoFar = upgraded ?: currentStream
                    val bestIsLossless = bestSoFar.badge.contains("FLAC", ignoreCase = true) ||
                        bestSoFar.badge.contains("Lossless", ignoreCase = true)
                    val bestKbps = parseBitrateKbps(bestSoFar.badge).let { if (it > 0) it else baselineKbps }
                    if (!bestIsLossless && bestKbps < 320) {
                        val jioMatch = runCatching {
                            JioSaavnSource.findHigherQualityStream(effectiveTrack, bestKbps)
                        }.getOrNull()
                        if (jioMatch != null && jioMatch.bitrateKbps > bestKbps && jioMatch.bitrateKbps <= ceilingKbps) {
                            Log.i(
                                "SonataStream",
                                "JioSaavn quality upgrade for '${effectiveTrack.title}': ${bestKbps}kbps -> ${jioMatch.bitrateKbps}kbps"
                            )
                            upgraded = AndroidResolvedStream(
                                url = jioMatch.url,
                                badge = "AAC ${jioMatch.bitrateKbps}kbps · JioSaavn",
                                headers = emptyList(),
                            )
                        }
                    }
                }

                if (upgraded != null && isActive && _uiState.value.activeTrack?.id == effectiveTrack.id) {
                    putCachedStream(effectiveTrack.id, upgraded)
                    _uiState.update { it.copy(streamBadge = upgraded.badge) }
                    val hiFiItem = buildMediaItem(effectiveTrack, upgraded)
                    withContext(Dispatchers.Main) {
                        if (!isActive || _uiState.value.activeTrack?.id != effectiveTrack.id) return@withContext
                        runCatching {
                            controller?.let { ctrl ->
                                val pos = ctrl.currentPosition.coerceAtLeast(0L)
                                val wasPlaying = ctrl.isPlaying
                                ctrl.setMediaItem(hiFiItem, pos)
                                ctrl.prepare()
                                if (wasPlaying) ctrl.play()
                            }
                        }.onFailure { err ->
                            Log.w("SonataStream", "Hi-Fi swap failed: ${err.message}")
                        }
                    }
                }
            } finally {
                if (_uiState.value.activeTrack?.id == effectiveTrack.id) {
                    _uiState.update { it.copy(isUpgradingQuality = false) }
                }
            }
        }
    }

    private fun shouldUpgradeStreamBadge(badge: String, signedIn: Boolean): Boolean {
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
        if (sleepAtEndOfTrack) {
            sleepAtEndOfTrack = false
            _sleepTimerRemainingMinutes.value = null
            controller?.pause()
            playbackClock.onSeek(0L)
            _uiState.update { it.copy(isPlaying = false, status = "Sleep timer paused playback") }
            return
        }
        val state = _uiState.value
        if (state.repeatMode == Player.REPEAT_MODE_ONE) {
            seekTo(0L)
            controller?.play()
            return
        }
        if (state.hasNext) {
            nextTrack()
        } else if (settings.value.autoplayEnabled && state.activeTrack != null) {
            val seed = state.activeTrack
            autoplayJob?.cancel()
            autoplayJob = viewModelScope.launch(Dispatchers.IO) {
                val engine = requireEngine()
                val existingIds = state.queue.map { it.id }.toSet()
                val radio = runCatching { engine.fetchRadioQueue(seed.id) }
                    .getOrDefault(emptyList())
                    .filter { it.id !in existingIds }
                if (radio.isNotEmpty() && isActive) {
                    val extended = state.queue + radio
                    playTrack(radio.first(), extended)
                } else {
                    playbackClock.onSeek(0L)
                    _uiState.update { it.copy(isPlaying = false) }
                }
            }
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
        runCatching { controller?.repeatMode = next }
        _uiState.update { it.copy(repeatMode = next) }
    }

    fun toggleShuffle() {
        _uiState.update { it.copy(shuffleEnabled = !it.shuffleEnabled) }
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
            runCatching { engine.checkForUpdatesForVersion(installedVersion) }
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
                            updateError = if (isManual) err.message ?: "Update check failed" else it.updateError,
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
        val info = state.updateInfo ?: return
        val apkUrl = info.apkDownloadUrl?.takeIf { it.isNotBlank() }
        if (apkUrl == null) {
            _uiState.update { it.copy(updateError = "No APK asset found in this release.") }
            return
        }

        _uiState.update {
            it.copy(
                showUpdateDialog = true,
                updateDownloading = true,
                updateDownloadProgress = null,
                updateDownloadDetail = "Starting download...",
                updateError = null,
            )
        }

        updateDownloadJob?.cancel()
        updateDownloadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val app = getApplication<Application>()
                val savedPath = ApkUpdater.downloadApk(
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
                _uiState.update {
                    it.copy(
                        updateDownloading = false,
                        updateDownloadProgress = 1f,
                        updateDownloadDetail = "Saved to $savedPath — open from your Downloads app to install.",
                        downloadedUpdateApkPath = savedPath,
                        status = "Update saved to $savedPath",
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
                        updateError = err.message ?: "Download failed",
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

    override fun onCleared() {
        PlaybackQueueBridge.canSkipNext = { false }
        PlaybackQueueBridge.canSkipPrevious = { false }
        PlaybackQueueBridge.onSkipNext = {}
        PlaybackQueueBridge.onSkipPrevious = {}
        PlaybackQueueBridge.onPlayFromEmpty = {}
        speculativePrefetchJob?.cancel()
        refreshJob?.cancel()
        radioRefreshJob?.cancel()
        playJob?.cancel()
        recoveryJob?.cancel()
        autoplayJob?.cancel()
        searchDebounceJob?.cancel()
        lyricsJob?.cancel()
        canvasJob?.cancel()
        sleepTimerJob?.cancel()
        cipherSolver.destroy()
        runCatching { controller?.release() }
        controller = null
        super.onCleared()
    }

    private fun backfillCollectionTracks(
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

        private fun isExcludedShelfTitle(title: String): Boolean {
            val lower = title.trim().lowercase()
            return "forgotten favorite" in lower || "unutulan favori" in lower
        }

        private fun parseArtistFromCollectionSubtitle(subtitle: String?): String? {
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
    }
}
