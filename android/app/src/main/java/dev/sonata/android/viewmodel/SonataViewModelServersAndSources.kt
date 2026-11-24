/*
 * Self-hosted server management, LAN browser, health/speed diagnostics,
 * local MediaStore audio scanning, and Personal Only Mode helpers for SonataViewModel.
 */
package dev.sonata.android.viewmodel

import android.app.Application
import android.content.ContentUris
import android.provider.MediaStore
import androidx.lifecycle.viewModelScope
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.ServerBrowserState
import dev.sonata.android.ServerSourceSummary
import dev.sonata.android.SonataViewModel
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.data.LocalMusicLibrary
import dev.sonata.android.data.LocalMusicScanner
import dev.sonata.android.data.SmbClient
import dev.sonata.android.data.SmbProfileRegistry
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.isSelfHostedId
import dev.sonata.android.ui.components.PERSONAL_PLAYLIST_ID_PREFIX
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.screens.ServerDiagnosticState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidServerHealthReport
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile

internal fun SonataViewModel.openServersScreenImpl(initialServerId: String? = null) {
    _uiState.update { it.copy(showServersSheet = true) }
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val list = runSuspendCatching { engine.listServers() }.getOrDefault(emptyList())
        SmbProfileRegistry.replaceAll(list)
        _uiState.update { it.copy(servers = list) }
        val target = initialServerId?.let { id -> list.firstOrNull { it.id == id } }
        if (target != null) {
            openServerBrowser(target)
        }
    }
}

internal fun SonataViewModel.dismissServersScreenImpl() {
    serverBrowseJob?.cancel()
    serverTestJob?.cancel()
    serverSearchJob?.cancel()
    _serverBrowserState.value = ServerBrowserState()
    _uiState.update { it.copy(showServersSheet = false) }
}

internal fun SonataViewModel.onPersonalOnlyModeChangedImpl(enabled: Boolean) {
    if (enabled) {
        refreshJob?.cancel()
        radioRefreshJob?.cancel()
        homeMoodChipJob?.cancel()
        moodCategoryJob?.cancel()
        autoplayJob?.cancel()
        autoplayPrefetchJob?.cancel()
        listeningHistoryJob?.cancel()
        artistJob?.cancel()
        searchJob?.cancel()
        prefetchedAutoplaySeedKey = null
        prefetchedAutoplayTracks = emptyList()
        homeFetchInFlight = false
        radioFetchInFlight = false

        val currentState = _uiState.value
        val activeTrack = currentState.activeTrack
        val keepActiveTrack = activeTrack != null && isPersonalTrackId(activeTrack.id)
        val personalQueue = currentState.queue.filter { isPersonalTrackId(it.id) }
        canvasJob?.cancel()
        musicVideoProbeJob?.cancel()
        requestedCanvasTrackId = null
        _currentCanvas.value = null
        if (activeTrack != null && !keepActiveTrack) {
            playbackGeneration.next()
            playJob?.cancel()
            lyricsJob?.cancel()
            upgradeJob?.cancel()
            PlaybackAudioBridge.clearEndedTransitionSuppression()
            PlaybackAudioBridge.setVideoModePlayback(false)
            AudioPipelineBus.reset()
            viewModelScope.launch(Dispatchers.Main) {
                runCatching {
                    controller?.stop()
                    controller?.clearMediaItems()
                }
            }
        } else if (!keepActiveTrack) {
            playbackGeneration.next()
        }

        val personalHistory = _lastPlayedTracks.value.filter { isPersonalTrackId(it.id) }
        _lastPlayedTracks.value = personalHistory
        val localOnly = mergePlaylistsWithLocal(emptyList())
        _uiState.update { state ->
            val currentCollectionId = state.collectionBrowseId
            val keepCollection = currentCollectionId != null && (
                currentCollectionId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) ||
                    currentCollectionId.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) ||
                    isSelfHostedId(currentCollectionId)
                )
            val resolvedIndex = if (keepActiveTrack && activeTrack != null) {
                personalQueue.indexOfFirst { it.id == activeTrack.id }.coerceAtLeast(0)
            } else {
                -1
            }
            state.copy(
                loading = false,
                refreshing = false,
                shelves = emptyList(),
                exploreShelves = emptyList(),
                moodCategories = emptyList(),
                homeMoodChips = emptyList(),
                selectedHomeMoodParams = null,
                radioMixShelves = emptyList(),
                tracks = emptyList(),
                lastPlayedTracks = personalHistory,
                recentArtistCards = emptyList(),
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                serverSearchResult = null,
                collectionBrowseId = if (keepCollection) state.collectionBrowseId else null,
                collectionTitle = if (keepCollection) state.collectionTitle else null,
                collectionTracks = if (keepCollection) state.collectionTracks else emptyList(),
                playlists = localOnly,
                ownedPlaylistIds = emptySet(),
                activeTrack = if (keepActiveTrack) activeTrack else null,
                queue = if (keepActiveTrack) personalQueue else emptyList(),
                queueIndex = resolvedIndex,
                isPlaying = if (keepActiveTrack) state.isPlaying else false,
                isBuffering = if (keepActiveTrack) state.isBuffering else false,
                streamBadge = if (keepActiveTrack) state.streamBadge else "",
                lyrics = if (keepActiveTrack) state.lyrics else emptyList(),
                videoModeEnabled = if (keepActiveTrack) state.videoModeEnabled else false,
                videoFullscreen = false,
                musicVideoCandidate = null,
                status = sourcesStrings(preferences.state.value.appLanguage).personalOnlyModeBadge,
            )
        }
        scanLocalMusic()
    } else {
        lastHomeExploreFetchAtMs = 0L
        lastRadioFetchAtMs = 0L
        val restoredHistory = loadLocalLastPlayedHistory()
        _lastPlayedTracks.value = restoredHistory
        _uiState.update { state ->
            state.copy(lastPlayedTracks = restoredHistory)
        }
        loadHome(forceRefresh = true)
        refreshRadioRateLimited(forceManual = true)
    }
}

internal fun SonataViewModel.scanLocalMusicImpl() {
    val ctx = getApplication<Application>()
    val prefsState = preferences.state.value
    if (!prefsState.localMusicEnabled) {
        localContentUrisByTrackId.clear()
        localTracksById.clear()
        _uiState.update {
            it.copy(
                localMusicLibrary = LocalMusicLibrary(
                    permissionGranted = LocalMusicScanner.hasAudioReadPermission(ctx),
                ),
                localMusicLoading = false,
            )
        }
        return
    }
    localScanJob?.cancel()
    _uiState.update { it.copy(localMusicLoading = true) }
    localScanJob = viewModelScope.launch(Dispatchers.IO) {
        val library = LocalMusicScanner.scanAsync(
            context = ctx,
            minDurationMs = if (prefsState.excludeNonMusicAudio) {
                prefsState.localMusicMinDurationSecs * 1000L
            } else {
                LocalMusicScanner.DEFAULT_MIN_DURATION_MS
            },
            excludeNonMusicAudio = prefsState.excludeNonMusicAudio,
            selectedFolders = prefsState.localMusicFolders,
        )
        localContentUrisByTrackId.clear()
        localTracksById.clear()
        library.tracks.forEach { localTrack ->
            localContentUrisByTrackId[localTrack.id] = localTrack.contentUri
            localTracksById[localTrack.id] = localTrack
        }
        _uiState.update {
            it.copy(
                localMusicLibrary = library,
                localMusicLoading = false,
            )
        }
    }
}

internal fun SonataViewModel.resolveLocalContentUriImpl(trackId: String): String? {
    if (!trackId.startsWith(LocalMusicScanner.LOCAL_ID_PREFIX) || trackId == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
        return null
    }
    localContentUrisByTrackId[trackId]?.let { return it }
    val mediaId = trackId.removePrefix(LocalMusicScanner.LOCAL_ID_PREFIX).toLongOrNull()
    if (mediaId != null && mediaId > 0L) {
        val reconstructed = ContentUris.withAppendedId(
            MediaStore.Audio.Media.EXTERNAL_CONTENT_URI,
            mediaId,
        ).toString()
        localContentUrisByTrackId[trackId] = reconstructed
        return reconstructed
    }
    return null
}

internal fun SonataViewModel.resolveLocalInitialBadgeImpl(trackId: String): String {
    val local = localTracksById[trackId] ?: return "Local"
    val codec = when {
        local.mimeType.contains("flac", ignoreCase = true) -> "FLAC"
        local.mimeType.contains("alac", ignoreCase = true) -> "ALAC"
        local.mimeType.contains("opus", ignoreCase = true) -> "OPUS"
        local.mimeType.contains("mpeg", ignoreCase = true) -> "MP3"
        local.mimeType.contains("mp4", ignoreCase = true) || local.mimeType.contains("aac", ignoreCase = true) -> "AAC"
        local.mimeType.contains("wav", ignoreCase = true) -> "WAV"
        else -> "Local"
    }
    return if (local.bitrateKbps > 0) "$codec · ${local.bitrateKbps} kbps" else codec
}

internal fun SonataViewModel.probeServerHealthImpl(serverId: String) {
    if (serverId.isBlank()) return
    serverHealthJobs.remove(serverId)?.cancel()
    _uiState.update { state ->
        val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
        state.copy(
            serverDiagnostics = state.serverDiagnostics + (
                serverId to current.copy(checkingHealth = true, healthError = null)
                ),
        )
    }
    val job = viewModelScope.launch(Dispatchers.IO) {
        val smbServer = _uiState.value.servers.firstOrNull {
            it.id == serverId && it.kind == AndroidServerKind.SMB
        }
        val engine = if (smbServer == null) requireEngine() else null
        runSuspendCatching {
            if (smbServer != null) SmbClient.probeHealth(smbServer)
            else engine!!.probeServerHealth(serverId)
        }
            .onSuccess { report ->
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    state.copy(
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                checkingHealth = false,
                                healthReport = report,
                                healthError = if (report.alive) null else report.message,
                            )
                            ),
                    )
                }
            }
            .onFailure { err ->
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    state.copy(
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                checkingHealth = false,
                                healthError = err.message ?: "Health check failed",
                            )
                            ),
                    )
                }
            }
    }
    serverHealthJobs[serverId] = job
}

internal fun SonataViewModel.runServerSpeedTestImpl(serverId: String) {
    if (serverId.isBlank()) return
    serverSpeedJobs.remove(serverId)?.cancel()
    _uiState.update { state ->
        val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
        state.copy(
            serverDiagnostics = state.serverDiagnostics + (
                serverId to current.copy(runningSpeedTest = true, speedTestError = null)
                ),
        )
    }
    val job = viewModelScope.launch(Dispatchers.IO) {
        val smbServer = _uiState.value.servers.firstOrNull {
            it.id == serverId && it.kind == AndroidServerKind.SMB
        }
        val engine = if (smbServer == null) requireEngine() else null
        runSuspendCatching {
            if (smbServer != null) SmbClient.runSpeedTest(smbServer)
            else engine!!.runServerSpeedTest(serverId)
        }
            .onSuccess { report ->
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    val updatedHealth = if (report.alive) {
                        AndroidServerHealthReport(
                            alive = true,
                            latencyMs = report.latencyMs,
                            message = report.sampleNote,
                        )
                    } else {
                        current.healthReport
                    }
                    state.copy(
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                runningSpeedTest = false,
                                speedTestReport = report,
                                speedTestError = if (report.alive) null else report.sampleNote,
                                healthReport = updatedHealth,
                            )
                            ),
                    )
                }
            }
            .onFailure { err ->
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    state.copy(
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                runningSpeedTest = false,
                                speedTestError = err.message ?: "Speed test failed",
                            )
                            ),
                    )
                }
            }
    }
    serverSpeedJobs[serverId] = job
}

internal fun SonataViewModel.rescanServerSourceImpl(serverId: String) {
    val server = _uiState.value.servers.firstOrNull { it.id == serverId } ?: return
    serverRescanJobs.remove(serverId)?.cancel()

    SmbClient.invalidateCache(serverId)
    if (server.kind == AndroidServerKind.SMB) {
        SmbClient.clearArtworkCache()
    }
    artistPageCache.clear()
    streamCache.keys.filter { it.contains(":$serverId:") || it.startsWith("smb:$serverId:") }
        .forEach { streamCache.remove(it) }

    _uiState.update { state ->
        val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
        state.copy(
            serverDiagnostics = state.serverDiagnostics + (
                serverId to current.copy(
                    rescanning = true,
                    rescanError = null,
                )
                ),
            serverSourceSnapshots = state.serverSourceSnapshots + (
                serverId to ServerSourceSummary(serverId = serverId, loading = true)
                ),
        )
    }

    val srcStrings = sourcesStrings(preferences.state.value.appLanguage)
    val job = viewModelScope.launch(Dispatchers.IO) {
        runSuspendCatching {
            when (server.kind) {
                AndroidServerKind.SMB -> SmbClient.scanSummary(server, forceRefresh = true)
                AndroidServerKind.WEB_DAV -> {
                    val engine = requireEngine()
                    coroutineScope {
                        val rootDef = async { runSuspendCatching { engine.browseWebdavFolder(serverId, null) } }
                        val albumsDef = async { runSuspendCatching { engine.listServerAlbums(serverId, null, null) } }
                        val artistsDef = async { runSuspendCatching { engine.listServerArtists(serverId, null) } }
                        val rootRes = rootDef.await()
                        val albumsRes = albumsDef.await()
                        val artistsRes = artistsDef.await()
                        val listing = rootRes.getOrNull()
                        val folderPlaylists = listing?.folders.orEmpty().map { dir ->
                            AndroidPlaylist(
                                id = dir.id,
                                name = dir.name,
                                thumbnailUrl = null,
                                isOwned = false,
                            )
                        }
                        val err = rootRes.exceptionOrNull()
                        ServerSourceSummary(
                            serverId = serverId,
                            albums = albumsRes.getOrDefault(emptyList()),
                            playlists = folderPlaylists,
                            artists = artistsRes.getOrDefault(emptyList()),
                            tracks = listing?.tracks.orEmpty(),
                            loading = false,
                            error = if (rootRes.isFailure && albumsRes.isFailure) err?.message else null,
                        )
                    }
                }
                else -> {
                    val engine = requireEngine()
                    coroutineScope {
                        val albumsDef = async { runSuspendCatching { engine.listServerAlbums(serverId, null, null) } }
                        val playlistsDef = async { runSuspendCatching { engine.listServerPlaylists(serverId) } }
                        val artistsDef = async { runSuspendCatching { engine.listServerArtists(serverId, null) } }
                        val searchTracksDef = async { runSuspendCatching { engine.searchServer(serverId, "", 40u) } }
                        val albumsRes = albumsDef.await()
                        val playlistsRes = playlistsDef.await()
                        val artistsRes = artistsDef.await()
                        val tracksRes = searchTracksDef.await()
                        val err = albumsRes.exceptionOrNull() ?: playlistsRes.exceptionOrNull()
                        ServerSourceSummary(
                            serverId = serverId,
                            albums = albumsRes.getOrDefault(emptyList()),
                            playlists = playlistsRes.getOrDefault(emptyList()),
                            artists = artistsRes.getOrDefault(emptyList()),
                            tracks = tracksRes.getOrNull()?.tracks.orEmpty(),
                            loading = false,
                            error = if (albumsRes.isFailure && playlistsRes.isFailure) err?.message else null,
                        )
                    }
                }
            }
        }
            .onSuccess { summary ->
                val countSummary = srcStrings.rescanSummary(summary.tracks.size, summary.albums.size)
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    state.copy(
                        serverSourceSnapshots = state.serverSourceSnapshots + (serverId to summary),
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                rescanning = false,
                                rescanSummary = countSummary,
                                rescanError = summary.error,
                            )
                            ),
                    )
                }
                if (_serverBrowserState.value.activeServer?.id == serverId) {
                    openServerBrowserImpl(server)
                }
            }
            .onFailure { err ->
                _uiState.update { state ->
                    val current = state.serverDiagnostics[serverId] ?: ServerDiagnosticState()
                    state.copy(
                        serverSourceSnapshots = state.serverSourceSnapshots + (
                            serverId to ServerSourceSummary(
                                serverId = serverId,
                                loading = false,
                                error = err.message,
                            )
                            ),
                        serverDiagnostics = state.serverDiagnostics + (
                            serverId to current.copy(
                                rescanning = false,
                                rescanError = err.message ?: "Rescan failed",
                            )
                            ),
                    )
                }
            }
    }
    serverRescanJobs[serverId] = job
}

internal fun SonataViewModel.loadServerSourceSummaryImpl(serverId: String, forceRefresh: Boolean = false) {
    if (serverId.isBlank()) return
    val existing = _uiState.value.serverSourceSnapshots[serverId]
    if (!forceRefresh && existing != null && (existing.loading || existing.albums.isNotEmpty() || existing.playlists.isNotEmpty() || existing.tracks.isNotEmpty())) {
        return
    }
    val server = _uiState.value.servers.firstOrNull { it.id == serverId } ?: return
    _uiState.update { state ->
        state.copy(
            serverSourceSnapshots = state.serverSourceSnapshots + (
                serverId to ServerSourceSummary(serverId = serverId, loading = true)
                ),
        )
    }
    viewModelScope.launch(Dispatchers.IO) {
        if (server.kind == AndroidServerKind.SMB) {
            runSuspendCatching { SmbClient.scanSummary(server, forceRefresh = forceRefresh) }
                .onSuccess { summary ->
                    _uiState.update { state ->
                        state.copy(serverSourceSnapshots = state.serverSourceSnapshots + (serverId to summary))
                    }
                }
                .onFailure { err ->
                    _uiState.update { state ->
                        state.copy(
                            serverSourceSnapshots = state.serverSourceSnapshots + (
                                serverId to ServerSourceSummary(
                                    serverId = serverId,
                                    loading = false,
                                    error = err.message,
                                )
                                ),
                        )
                    }
                }
            return@launch
        }
        val engine = requireEngine()
        if (server.kind == AndroidServerKind.WEB_DAV) {
            coroutineScope {
                val rootDef = async { runSuspendCatching { engine.browseWebdavFolder(serverId, null) } }
                val albumsDef = async { runSuspendCatching { engine.listServerAlbums(serverId, null, null) } }
                val artistsDef = async { runSuspendCatching { engine.listServerArtists(serverId, null) } }
                val rootRes = rootDef.await()
                val albumsRes = albumsDef.await()
                val artistsRes = artistsDef.await()
                val listing = rootRes.getOrNull()
                val folderPlaylists = listing?.folders.orEmpty().map { dir ->
                    AndroidPlaylist(
                        id = dir.id,
                        name = dir.name,
                        thumbnailUrl = null,
                        isOwned = false,
                    )
                }
                _uiState.update { state ->
                    state.copy(
                        serverSourceSnapshots = state.serverSourceSnapshots + (
                            serverId to ServerSourceSummary(
                                serverId = serverId,
                                albums = albumsRes.getOrDefault(emptyList()),
                                playlists = folderPlaylists,
                                artists = artistsRes.getOrDefault(emptyList()),
                                tracks = listing?.tracks.orEmpty(),
                                loading = false,
                                error = if (rootRes.isFailure && albumsRes.isFailure) rootRes.exceptionOrNull()?.message else null,
                            )
                            ),
                    )
                }
            }
            return@launch
        }
        coroutineScope {
            val albumsDef = async { runSuspendCatching { engine.listServerAlbums(serverId, null, null) } }
            val playlistsDef = async { runSuspendCatching { engine.listServerPlaylists(serverId) } }
            val artistsDef = async { runSuspendCatching { engine.listServerArtists(serverId, null) } }
            val searchTracksDef = async { runSuspendCatching { engine.searchServer(serverId, "", 40u) } }
            val albumsRes = albumsDef.await()
            val playlistsRes = playlistsDef.await()
            val artistsRes = artistsDef.await()
            val tracksRes = searchTracksDef.await()
            val err = albumsRes.exceptionOrNull() ?: playlistsRes.exceptionOrNull()
            _uiState.update { state ->
                state.copy(
                    serverSourceSnapshots = state.serverSourceSnapshots + (
                        serverId to ServerSourceSummary(
                            serverId = serverId,
                            albums = albumsRes.getOrDefault(emptyList()),
                            playlists = playlistsRes.getOrDefault(emptyList()),
                            artists = artistsRes.getOrDefault(emptyList()),
                            tracks = tracksRes.getOrNull()?.tracks.orEmpty(),
                            loading = false,
                            error = if (albumsRes.isFailure && playlistsRes.isFailure) err?.message else null,
                        )
                        ),
                )
            }
        }
    }
}

internal fun SonataViewModel.clearSelfHostedCachesImpl() {
    streamCache.keys.filter { isSelfHostedId(it) }.forEach { streamCache.remove(it) }
    SmbClient.invalidateCache()
    PlaybackAudioBridge.clearStreamHeaders()
}

internal fun SonataViewModel.clearServerTestResultImpl() {
    _serverBrowserState.update {
        it.copy(testingConnection = false, testResultMessage = null, testResultIsError = false)
    }
}

internal fun SonataViewModel.testServerConnectionImpl(profile: AndroidServerProfile) {
    serverTestJob?.cancel()
    val s = uiStrings().servers
    _serverBrowserState.update {
        it.copy(testingConnection = true, testResultMessage = s.testingConnection, testResultIsError = false)
    }
    serverTestJob = viewModelScope.launch(Dispatchers.IO) {
        val engine = if (profile.kind == AndroidServerKind.SMB) null else requireEngine()
        runSuspendCatching {
            if (profile.kind == AndroidServerKind.SMB) {
                SmbClient.testConnection(profile)
            } else {
                engine!!.testServerConnection(profile)
            }
        }
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

internal fun SonataViewModel.saveServerImpl(profile: AndroidServerProfile, onComplete: () -> Unit = {}) {
    val s = uiStrings().servers
    _serverBrowserState.update { it.copy(saving = true, error = null) }
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.upsertServer(profile) }
            .onSuccess { updated ->
                SmbProfileRegistry.replaceAll(updated)
                clearSelfHostedCachesImpl()
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

internal fun SonataViewModel.deleteServerImpl(serverId: String) {
    val s = uiStrings().servers
    val removedName = _uiState.value.servers.firstOrNull { it.id == serverId }?.name ?: serverId
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.deleteServer(serverId) }
            .onSuccess { updated ->
                SmbProfileRegistry.replaceAll(updated)
                clearSelfHostedCachesImpl()
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

internal fun SonataViewModel.setServerEnabledImpl(serverId: String, enabled: Boolean) {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.setServerEnabled(serverId, enabled) }
            .onSuccess { updated ->
                SmbProfileRegistry.replaceAll(updated)
                clearSelfHostedCachesImpl()
                val refreshedActive = _serverBrowserState.value.activeServer?.let { cur ->
                    if (cur.id == serverId) updated.firstOrNull { it.id == serverId } else cur
                }
                _serverBrowserState.update { it.copy(activeServer = refreshedActive) }
                _uiState.update { it.copy(servers = updated) }
            }
    }
}

internal fun SonataViewModel.openServerBrowserImpl(server: AndroidServerProfile) {
    serverBrowseJob?.cancel()
    serverSearchJob?.cancel()
    SmbProfileRegistry.put(server)
    _serverBrowserState.value = ServerBrowserState(
        activeServer = server,
        loading = true,
        webdavPathStack = if (server.kind == AndroidServerKind.WEB_DAV || server.kind == AndroidServerKind.SMB) {
            listOf(null to uiStrings().servers.rootFolder)
        } else {
            emptyList()
        },
    )
    serverBrowseJob = viewModelScope.launch(Dispatchers.IO) {
        if (server.kind == AndroidServerKind.SMB) {
            runSuspendCatching { SmbClient.listFolder(server, null) }
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
                    return@launch
                }
            runSuspendCatching { SmbClient.scanSummary(server) }
                .onSuccess { summary ->
                    _serverBrowserState.update {
                        it.copy(
                            albums = summary.albums,
                            artists = summary.artists,
                            playlists = summary.playlists,
                        )
                    }
                }
            return@launch
        }
        val engine = requireEngine()
        if (server.kind == AndroidServerKind.WEB_DAV) {
            coroutineScope {
                val rootDef = async { runSuspendCatching { engine.browseWebdavFolder(server.id, null) } }
                val albumsDef = async { runSuspendCatching { engine.listServerAlbums(server.id, null, null) } }
                val artistsDef = async { runSuspendCatching { engine.listServerArtists(server.id, null) } }
                val rootRes = rootDef.await()
                val albumsRes = albumsDef.await()
                val artistsRes = artistsDef.await()
                val listing = rootRes.getOrNull()
                _serverBrowserState.update {
                    it.copy(
                        loading = false,
                        webdavFolders = listing?.folders.orEmpty(),
                        webdavTracks = listing?.tracks.orEmpty(),
                        albums = albumsRes.getOrDefault(emptyList()),
                        artists = artistsRes.getOrDefault(emptyList()),
                        error = if (rootRes.isFailure && albumsRes.isFailure) rootRes.exceptionOrNull()?.message else null,
                    )
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

internal fun SonataViewModel.closeServerBrowserImpl() {
    serverBrowseJob?.cancel()
    serverSearchJob?.cancel()
    _serverBrowserState.value = ServerBrowserState()
}

internal fun SonataViewModel.selectServerLibraryImpl(libraryId: String?) {
    val server = _serverBrowserState.value.activeServer ?: return
    if (server.kind == AndroidServerKind.WEB_DAV || server.kind == AndroidServerKind.SMB) {
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

internal fun SonataViewModel.openServerArtistImpl(artist: AndroidHomeCard) {
    val server = _serverBrowserState.value.activeServer
    if (server != null) {
        openArtistImpl(artist.id.ifBlank { "personal:artist:${artist.title}" }, artist.title)
        return
    }
}

internal fun SonataViewModel.closeServerArtistImpl() {
    _serverBrowserState.update {
        it.copy(selectedArtistName = null, artistAlbums = emptyList())
    }
}

internal fun SonataViewModel.openWebDavFolderImpl(folderIdOrPath: String?, label: String) {
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
        if (server.kind == AndroidServerKind.SMB) {
            runSuspendCatching { SmbClient.listFolder(server, folderIdOrPath) }
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

internal fun SonataViewModel.navigateWebDavBackImpl(): Boolean {
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
        if (server.kind == AndroidServerKind.SMB) {
            runSuspendCatching { SmbClient.listFolder(server, targetPath) }
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

internal fun SonataViewModel.searchInsideServerImpl(query: String) {
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
        if (server.kind == AndroidServerKind.SMB) {
            runSuspendCatching { SmbClient.searchTracks(server, clean) }
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
            return@launch
        }
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

internal fun SonataViewModel.openServerAlbumImpl(card: AndroidHomeCard) {
    openServerAlbumById(
        albumId = card.id,
        albumTitle = card.title,
        artistName = card.subtitle,
        coverUrl = card.thumbnailUrl,
    )
}

internal fun SonataViewModel.openServerAlbumByIdImpl(
    albumId: String,
    albumTitle: String,
    artistName: String = "",
    coverUrl: String? = null,
    year: String? = null,
) {
    val s = uiStrings()
    collectionJob?.cancel()
    collectionJob = viewModelScope.launch(Dispatchers.IO) {
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
        if (albumId.startsWith("smb:")) {
            val srvId = albumId.removePrefix("smb:").substringBefore(':')
            val srv = _uiState.value.servers.firstOrNull { it.id == srvId }
            if (srv != null) {
                runSuspendCatching { SmbClient.getAlbumTracks(srv, albumId) }
                    .onSuccess { tracks ->
                        _uiState.update { state ->
                            state.copy(
                                loading = false,
                                collectionTracks = tracks,
                                status = s.tracksInCollectionStatus(tracks.size, albumTitle),
                            )
                        }
                    }
                    .onFailure { err ->
                        _uiState.update {
                            it.copy(loading = false, status = s.collectionErrorStatus(err.message.orEmpty()))
                        }
                    }
                return@launch
            }
        }
        val engine = requireEngine()
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
