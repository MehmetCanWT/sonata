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
import dev.sonata.android.data.LocalMusicLibrary
import dev.sonata.android.data.LocalMusicScanner
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.isSelfHostedId
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
        prefetchedAutoplaySeedKey = null
        prefetchedAutoplayTracks = emptyList()
        homeFetchInFlight = false
        radioFetchInFlight = false
        val localOnly = mergePlaylistsWithLocal(emptyList())
        _uiState.update { state ->
            state.copy(
                loading = false,
                refreshing = false,
                shelves = emptyList(),
                exploreShelves = emptyList(),
                moodCategories = emptyList(),
                homeMoodChips = emptyList(),
                selectedHomeMoodParams = null,
                radioMixShelves = emptyList(),
                playlists = localOnly,
                ownedPlaylistIds = emptySet(),
                status = sourcesStrings(preferences.state.value.appLanguage).personalOnlyModeBadge,
            )
        }
        scanLocalMusic()
    } else {
        lastHomeExploreFetchAtMs = 0L
        lastRadioFetchAtMs = 0L
        loadHome(forceRefresh = true)
    }
}

internal fun SonataViewModel.scanLocalMusicImpl() {
    val ctx = getApplication<Application>()
    if (!preferences.state.value.localMusicEnabled) {
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
        val library = LocalMusicScanner.scanAsync(ctx)
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
        val engine = requireEngine()
        runSuspendCatching { engine.probeServerHealth(serverId) }
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
        val engine = requireEngine()
        runSuspendCatching { engine.runServerSpeedTest(serverId) }
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
        val engine = requireEngine()
        if (server.kind == AndroidServerKind.WEB_DAV) {
            runSuspendCatching { engine.browseWebdavFolder(serverId, null) }
                .onSuccess { listing ->
                    val folderPlaylists = listing.folders.map { dir ->
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
                                    playlists = folderPlaylists,
                                    tracks = listing.tracks,
                                    loading = false,
                                )
                                ),
                        )
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

internal fun SonataViewModel.saveServerImpl(profile: AndroidServerProfile, onComplete: () -> Unit = {}) {
    val s = uiStrings().servers
    _serverBrowserState.update { it.copy(saving = true, error = null) }
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.upsertServer(profile) }
            .onSuccess { updated ->
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

internal fun SonataViewModel.closeServerBrowserImpl() {
    serverBrowseJob?.cancel()
    serverSearchJob?.cancel()
    _serverBrowserState.value = ServerBrowserState()
}

internal fun SonataViewModel.selectServerLibraryImpl(libraryId: String?) {
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

internal fun SonataViewModel.openServerArtistImpl(artist: AndroidHomeCard) {
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
