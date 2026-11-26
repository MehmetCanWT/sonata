/*
 * Discord Rich Presence, Scrobbling, On-Device Listening Stats, Sonata Connect,
 * Android Auto playback routing, and in-app GitHub Releases APK updater for SonataViewModel.
 */
package dev.sonata.android.viewmodel

import android.app.Application
import android.util.Log
import androidx.lifecycle.viewModelScope
import dev.sonata.android.ApkInstallLaunchResult
import dev.sonata.android.ApkUpdater
import dev.sonata.android.SonataViewModel
import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.ConnectStateSnapshot
import dev.sonata.android.connect.ConnectTrackSnapshot
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.discord.DiscordPresenceConfig
import dev.sonata.android.discord.DiscordPresenceTrack
import dev.sonata.android.playback.SonataAutoBrowser
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidScrobbleConfig
import uniffi.sonata_android.AndroidStatsWindow
import uniffi.sonata_android.AndroidTrack
import java.io.File

internal fun SonataViewModel.observeDiscordPresenceImpl() {
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

internal fun SonataViewModel.saveDiscordTokenImpl(token: String) {
    val cleaned = token.trim()
    if (cleaned.isEmpty()) return
    preferences.setDiscordToken(cleaned)
    if (!preferences.state.value.discordRpcEnabled) {
        preferences.update { it.copy(discordRpcEnabled = true) }
    }
}

internal fun SonataViewModel.signOutDiscordImpl() {
    preferences.clearDiscordToken()
    discordRpcClient.updateAuthAndConfig(
        newToken = null,
        newConfig = DiscordPresenceConfig(enabled = false, appId = preferences.state.value.discordAppId),
    )
}

internal fun SonataViewModel.reconnectDiscordImpl() {
    discordRpcClient.reconnect()
}

internal fun SonataViewModel.playAutoMediaIdImpl(mediaId: String) {
    if (mediaId.isBlank()) return
    val resolved = autoDataProvider.resolvePlaybackTarget(mediaId)
    if (resolved != null) {
        playTrack(resolved.first, resolved.second)
        return
    }
    if (mediaId.startsWith(SonataAutoBrowser.PLAYLIST_PREFIX)) {
        val playlistId = mediaId.removePrefix(SonataAutoBrowser.PLAYLIST_PREFIX)
        viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            val tracks = fetchTracksForCollectionAction(engine, playlistId)
            if (tracks.isNotEmpty()) {
                autoPlaylistTracksCache[playlistId] = tracks
                withContext(Dispatchers.Main) {
                    playTrack(tracks.first(), tracks)
                }
            }
        }
        return
    }
    val active = _uiState.value.activeTrack
    if (active != null && active.id == mediaId) {
        playTrack(active, _uiState.value.queue)
    }
}

internal fun SonataViewModel.checkForUpdatesImpl(autoPopup: Boolean = true, isManual: Boolean = true) {
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

internal fun SonataViewModel.openUpdateDialogImpl() {
    _uiState.update { it.copy(showUpdateDialog = true, updateError = null) }
}

internal fun SonataViewModel.dismissUpdateDialogImpl() {
    val latest = _uiState.value.updateInfo?.latestVersion
    if (!latest.isNullOrBlank()) {
        dismissedUpdateVersion = latest
    }
    _uiState.update { it.copy(showUpdateDialog = false) }
}

internal fun SonataViewModel.cancelUpdateDownloadImpl() {
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

internal fun SonataViewModel.downloadAndInstallUpdateImpl() {
    val state = _uiState.value
    if (state.updateDownloading) return
    val app = getApplication<Application>()

    val s = uiStrings()
    val existingPath = state.downloadedUpdateApkPath
    if (!existingPath.isNullOrBlank()) {
        val existingFile = File(existingPath)
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
            ApkInstallLaunchResult.SIGNATURE_MISMATCH -> {
                _uiState.update {
                    it.copy(
                        downloadedUpdateApkPath = null,
                        updateError = s.updateSignatureMismatch,
                    )
                }
                return
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
            val launchResult = withContext(Dispatchers.Main) {
                ApkUpdater.launchInstall(app, apkFile)
            }
            val curStrings = uiStrings()
            val detailMsg = when (launchResult) {
                ApkInstallLaunchResult.LAUNCHED -> curStrings.updateInstallingVersion(info.latestVersion)
                ApkInstallLaunchResult.PERMISSION_REQUIRED -> curStrings.updatePermissionRequired
                ApkInstallLaunchResult.FILE_MISSING -> curStrings.updateApkMissingRetry
                ApkInstallLaunchResult.SIGNATURE_MISMATCH -> curStrings.updateSignatureMismatch
            }
            val launchError = if (launchResult == ApkInstallLaunchResult.SIGNATURE_MISMATCH) detailMsg else null
            _uiState.update {
                it.copy(
                    updateDownloading = false,
                    updateDownloadProgress = if (launchError == null) 1f else null,
                    updateDownloadDetail = if (launchError == null) detailMsg else "",
                    updateError = launchError,
                    downloadedUpdateApkPath = if (launchError == null) apkFile.absolutePath else null,
                    status = detailMsg,
                )
            }
        } catch (cancelled: CancellationException) {
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

internal fun SonataViewModel.openStatsScreenImpl() {
    _uiState.update { it.copy(showStatsScreen = true) }
    refreshListeningStats()
}

internal fun SonataViewModel.dismissStatsScreenImpl() {
    _uiState.update { it.copy(showStatsScreen = false) }
}

internal fun SonataViewModel.selectStatsWindowImpl(window: AndroidStatsWindow) {
    _uiState.update { it.copy(statsWindow = window) }
    refreshListeningStats(window)
}

internal fun SonataViewModel.refreshListeningStatsImpl(window: AndroidStatsWindow = _uiState.value.statsWindow) {
    statsLoadJob?.cancel()
    statsLoadJob = viewModelScope.launch(Dispatchers.IO) {
        _uiState.update { it.copy(statsLoading = true) }
        val engine = requireEngine()
        runSuspendCatching { engine.queryListeningStats(window) }
            .onSuccess { summary ->
                _uiState.update {
                    if (it.statsWindow == window) {
                        it.copy(statsSummary = summary, statsLoading = false)
                    } else {
                        it.copy(statsLoading = false)
                    }
                }
            }
            .onFailure {
                _uiState.update { it.copy(statsLoading = false) }
            }
    }
}

internal fun SonataViewModel.saveScrobbleConfigImpl(config: AndroidScrobbleConfig) {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.saveScrobbleConfig(config) }
            .onSuccess { saved ->
                preferences.setLastFmEnabled(saved.lastfm.enabled)
                preferences.setListenBrainzEnabled(saved.listenbrainz.enabled)
                _uiState.update { it.copy(scrobbleConfig = saved) }
            }
            .onFailure { err ->
                Log.w("SonataScrobble", "Failed saving scrobble config: ${err.message}")
            }
    }
}

internal fun SonataViewModel.dispatchScrobbleNowPlayingImpl(track: AndroidTrack) {
    val cfg = _uiState.value.scrobbleConfig
    if (!cfg.lastfm.enabled && !cfg.listenbrainz.enabled) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.scrobbleNowPlaying(track) }
            .onFailure { err ->
                Log.w("SonataScrobble", "Now playing scrobble failed: ${err.message}")
            }
    }
}

internal fun SonataViewModel.dispatchScrobbleSubmitImpl(track: AndroidTrack, timestampUnix: ULong) {
    val cfg = _uiState.value.scrobbleConfig
    if (!cfg.lastfm.enabled && !cfg.listenbrainz.enabled) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.scrobbleSubmit(track, timestampUnix) }
            .onFailure { err ->
                Log.w("SonataScrobble", "Scrobble submit failed: ${err.message}")
            }
    }
}

internal fun SonataViewModel.dispatchRecordListeningStatImpl(
    track: AndroidTrack,
    sourceTag: String,
    playedSecs: ULong,
    playedAtUnix: ULong?,
) {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching {
            engine.recordListeningStat(
                track = track,
                sourceTag = sourceTag,
                playedSecs = playedSecs,
                playedAtUnix = playedAtUnix,
            )
        }.onSuccess {
            runSuspendCatching { engine.queryListeningStats(_uiState.value.statsWindow) }
                .onSuccess { summary ->
                    _uiState.update { it.copy(statsSummary = summary) }
                }
        }.onFailure { err ->
            Log.w("SonataStats", "Recording listening stat failed: ${err.message}")
        }
    }
}

internal fun SonataViewModel.observeConnectRemoteStateImpl() {
    viewModelScope.launch {
        var wasRemoteConnected = false
        connectController.state.collect { connectState ->
            val peer = connectState.activePeer
            if (peer != null) {
                if (!wasRemoteConnected) {
                    wasRemoteConnected = true
                    val current = _uiState.value
                    savedLocalPlaybackForConnect = SonataViewModel.SavedLocalPlaybackState(
                        activeTrack = current.activeTrack,
                        queue = current.queue,
                        queueIndex = current.queueIndex,
                        streamBadge = current.streamBadge,
                        positionMs = playbackClock.positionMs.coerceAtLeast(0L),
                        durationMs = playbackClock.durationMs.coerceAtLeast(0L),
                    )
                    pauseLocalPlaybackForConnect()
                    _uiState.update {
                        it.copy(
                            isRemoteConnectActive = true,
                            isBuffering = false,
                            isUpgradingQuality = false,
                        )
                    }
                }
                connectState.remoteState?.let { remote ->
                    mirrorRemoteConnectSnapshot(peer, remote)
                }
            } else if (wasRemoteConnected) {
                wasRemoteConnected = false
                pendingRemoteTransferTrackId = null
                pendingRemoteTransferUntilMs = 0L
                pendingRemoteQueueIds = null
                pendingRemoteQueueUntilMs = 0L
                lastMirroredConnectTrackId = null
                val saved = savedLocalPlaybackForConnect
                savedLocalPlaybackForConnect = null
                if (saved != null) {
                    _uiState.update {
                        it.copy(
                            isRemoteConnectActive = false,
                            activeTrack = saved.activeTrack,
                            queue = saved.queue,
                            queueIndex = saved.queueIndex,
                            streamBadge = saved.streamBadge,
                            isPlaying = false,
                            isBuffering = false,
                            isUpgradingQuality = false,
                        )
                    }
                    val localPos = runCatching { controller?.currentPosition?.coerceAtLeast(0L) }.getOrNull()
                        ?: saved.positionMs
                    val localDur = runCatching { controller?.duration?.coerceAtLeast(0L) }.getOrNull()
                        ?.takeIf { it > 0L }
                        ?: saved.durationMs
                    playbackClock.sync(positionMs = localPos, durationMs = localDur)
                    saved.activeTrack?.let { fetchLyricsInBackground(it) }
                } else {
                    _uiState.update {
                        it.copy(
                            isRemoteConnectActive = false,
                            isPlaying = false,
                            isBuffering = false,
                            isUpgradingQuality = false,
                        )
                    }
                }
            }
        }
    }
}

private fun SonataViewModel.mirrorRemoteConnectSnapshot(
    peer: ConnectBeacon,
    remote: ConnectStateSnapshot,
) {
    val nowMs = System.currentTimeMillis()
    val pendingId = pendingRemoteTransferTrackId
    val remoteTrackSnap = remote.activeTrack
    if (pendingId != null && nowMs < pendingRemoteTransferUntilMs) {
        if (remoteTrackSnap?.id == pendingId) {
            pendingRemoteTransferTrackId = null
            pendingRemoteTransferUntilMs = 0L
        } else {
            // Wait briefly for Desktop to apply the newly transferred track without snapping back.
            return
        }
    } else if (pendingId != null) {
        pendingRemoteTransferTrackId = null
        pendingRemoteTransferUntilMs = 0L
    }

    val currentUi = _uiState.value
    val likedIds = currentUi.likedTrackIds
    val pendingQueueIds = pendingRemoteQueueIds
    val keepLocalPendingQueue = if (pendingQueueIds != null && nowMs < pendingRemoteQueueUntilMs) {
        if (remote.queue.map { it.id } == pendingQueueIds) {
            pendingRemoteQueueIds = null
            pendingRemoteQueueUntilMs = 0L
            false
        } else {
            true
        }
    } else {
        if (pendingQueueIds != null) {
            pendingRemoteQueueIds = null
            pendingRemoteQueueUntilMs = 0L
        }
        false
    }
    if (remoteTrackSnap != null) {
        val existingMatch = currentUi.queue.firstOrNull { it.id == remoteTrackSnap.id }
            ?: currentUi.activeTrack?.takeIf { it.id == remoteTrackSnap.id }
        val remoteTrack = remoteTrackSnap.toAndroidTrack(likedIds, existingMatch)
        val mappedQueue = if (keepLocalPendingQueue && currentUi.queue.isNotEmpty()) {
            currentUi.queue
        } else if (remote.queue.isNotEmpty()) {
            remote.queue.map { snap ->
                val match = currentUi.queue.firstOrNull { it.id == snap.id }
                snap.toAndroidTrack(likedIds, match)
            }
        } else if (currentUi.queue.any { it.id == remoteTrack.id }) {
            currentUi.queue
        } else {
            listOf(remoteTrack)
        }
        val resolvedIndex = if (keepLocalPendingQueue && currentUi.queue.isNotEmpty()) {
            currentUi.queueIndex.coerceIn(0, (mappedQueue.size - 1).coerceAtLeast(0))
        } else if (remote.queue.isNotEmpty()) {
            remote.queueIndex.coerceIn(0, (mappedQueue.size - 1).coerceAtLeast(0))
        } else {
            mappedQueue.indexOfFirst { it.id == remoteTrack.id }.coerceAtLeast(0)
        }
        val trackChanged = lastMirroredConnectTrackId != remoteTrack.id
        lastMirroredConnectTrackId = remoteTrack.id
        val durationMs = remote.durationMs.takeIf { it > 0L }
            ?: ((remoteTrack.durationSecs?.toLong() ?: 0L) * 1000L)
        if (trackChanged) {
            playbackClock.resetForTrack(durationMs)
            playbackClock.onSeek(remote.positionMs.coerceAtLeast(0L))
            fetchLyricsInBackground(remoteTrack)
        } else {
            playbackClock.sync(
                positionMs = remote.positionMs.coerceAtLeast(0L),
                durationMs = durationMs,
            )
        }
        _uiState.update {
            it.copy(
                isRemoteConnectActive = true,
                activeTrack = remoteTrack,
                queue = mappedQueue,
                queueIndex = resolvedIndex,
                isPlaying = remote.isPlaying,
                isBuffering = false,
                isUpgradingQuality = false,
                streamBadge = peer.deviceName,
            )
        }
    } else {
        _uiState.update {
            it.copy(
                isRemoteConnectActive = true,
                isPlaying = false,
                isBuffering = false,
                isUpgradingQuality = false,
                streamBadge = peer.deviceName,
            )
        }
    }
}

private fun ConnectTrackSnapshot.toAndroidTrack(
    likedIds: Set<String>,
    existing: AndroidTrack?,
): AndroidTrack = AndroidTrack(
    id = id,
    title = title.ifBlank { existing?.title.orEmpty() },
    artist = artist.ifBlank { existing?.artist.orEmpty() },
    album = album.ifBlank { existing?.album.orEmpty() },
    durationSecs = durationSecs.takeIf { it > 0L }?.toULong() ?: existing?.durationSecs,
    thumbnailUrl = thumbnailUrl?.takeIf { it.isNotBlank() } ?: existing?.thumbnailUrl,
    liked = id in likedIds || existing?.liked == true,
    albumId = existing?.albumId,
    isExplicit = existing?.isExplicit ?: false,
)

internal fun SonataViewModel.openConnectSheetImpl() {
    _uiState.update { it.copy(showConnectSheet = true) }
    if (preferences.state.value.connectEnabled) {
        connectController.ensureDiscoveryRunning()
    }
}

internal fun SonataViewModel.dismissConnectSheetImpl() {
    _uiState.update { it.copy(showConnectSheet = false) }
}

internal fun SonataViewModel.setConnectEnabledImpl(enabled: Boolean) {
    preferences.setConnectEnabled(enabled)
    connectController.setEnabled(enabled)
}

internal fun SonataViewModel.setConnectDeviceNameImpl(name: String) {
    preferences.setConnectDeviceName(name)
    connectController.updateDeviceName(preferences.state.value.connectDeviceName)
}

internal fun SonataViewModel.refreshConnectDiscoveryImpl() {
    if (!preferences.state.value.connectEnabled) {
        preferences.setConnectEnabled(true)
    }
    connectController.refreshDiscovery()
}

internal fun SonataViewModel.requestPairConnectDeviceImpl(beacon: ConnectBeacon) {
    connectController.startInteractivePairing(beacon)
}

internal fun SonataViewModel.cancelConnectPairingImpl() {
    connectController.cancelPairing()
}

internal fun SonataViewModel.clearConnectErrorImpl() {
    connectController.clearPairingError()
}

internal fun SonataViewModel.pairConnectDeviceImpl(beacon: ConnectBeacon, pin: String) {
    viewModelScope.launch(Dispatchers.IO) {
        if (connectController.pairAndConnect(beacon, pin)) {
            withContext(Dispatchers.Main) {
                pauseLocalPlaybackForConnect()
            }
        }
    }
}

internal fun SonataViewModel.connectPairedDeviceImpl(beacon: ConnectBeacon) {
    viewModelScope.launch(Dispatchers.IO) {
        connectPlaybackRouter.connectToDevice(beacon) {
            viewModelScope.launch(Dispatchers.Main) {
                pauseLocalPlaybackForConnect()
            }
        }
    }
}

internal fun SonataViewModel.disconnectConnectDeviceImpl() {
    connectPlaybackRouter.disconnectToLocal()
}

internal fun SonataViewModel.forgetConnectDeviceImpl(deviceId: String) {
    connectController.forgetDevice(deviceId)
}

internal fun SonataViewModel.sendConnectCommandImpl(command: ConnectCommand) {
    viewModelScope.launch(Dispatchers.IO) {
        connectController.sendCommand(command)
    }
}

internal fun SonataViewModel.setConnectRemoteVolumeImpl(volume: Float) {
    viewModelScope.launch(Dispatchers.IO) {
        connectPlaybackRouter.setVolume(volume)
    }
}

internal fun SonataViewModel.syncRemoteQueueIfConnectedImpl(
    updatedQueue: List<AndroidTrack>,
    queueIndex: Int,
    playWhenReady: Boolean,
) {
    if (!isRemoteConnectActive() || updatedQueue.isEmpty()) return
    val clampedIndex = queueIndex.coerceIn(0, updatedQueue.lastIndex)
    val posMs = playbackClock.positionMs.coerceAtLeast(0L)
    pendingRemoteQueueIds = updatedQueue.map { it.id }
    pendingRemoteQueueUntilMs = System.currentTimeMillis() + 3_500L
    viewModelScope.launch(Dispatchers.IO) {
        connectPlaybackRouter.syncUpdatedQueue(
            tracks = updatedQueue,
            currentIndex = clampedIndex,
            positionMs = posMs,
            playWhenReady = playWhenReady,
        )
    }
}

internal fun SonataViewModel.transferQueueToConnectDeviceImpl() {
    val state = _uiState.value
    val queue = state.queue
    if (queue.isEmpty()) return
    val startIndex = state.queueIndex.coerceAtLeast(0)
    val posMs = playbackClock.positionMs.coerceAtLeast(0L)
    val isPlaying = state.isPlaying
    viewModelScope.launch(Dispatchers.IO) {
        connectController.transferQueue(
            tracks = queue,
            startIndex = startIndex,
            positionMs = posMs,
            playWhenReady = isPlaying,
        )
    }
}
