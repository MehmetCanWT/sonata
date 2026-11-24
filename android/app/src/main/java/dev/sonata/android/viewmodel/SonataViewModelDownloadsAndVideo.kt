/*
 * Offline track/video downloads, radio/station generators, sleep timer,
 * animated canvas discovery, and Music Video mode orchestration for SonataViewModel.
 */
package dev.sonata.android.viewmodel

import android.app.Application
import android.content.Context
import android.net.ConnectivityManager
import android.os.SystemClock
import android.util.Log
import androidx.lifecycle.viewModelScope
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.SonataViewModel
import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadedTrackInput
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataEngineGateway
import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.isSelfHostedId
import dev.sonata.android.playback.mapMusicToVideoMs
import dev.sonata.android.playback.mapVideoToMusicMs
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidMusicVideoCandidate
import uniffi.sonata_android.AndroidResolvedMusicVideo
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidVideoQualityStream

internal suspend fun SonataViewModel.resolveBestDownloadStreamImpl(
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
        if (SonataViewModel.shouldUpgradeStreamBadge(stream.badge, _uiState.value.signedIn)) {
            runSuspendCatching { engine.resolveHifiUpgrade(track, stream) }
                .getOrNull()
                ?.let { upgraded ->
                    stream = upgraded
                    putCachedStream(track.id, upgraded)
                }
        }
        if (currentSettings.downloadIncludeJioSaavn) {
            val curKbps = SonataViewModel.parseBitrateKbps(stream.badge).coerceAtLeast(0)
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

internal fun SonataViewModel.downloadTrackImpl(track: AndroidTrack, collectionId: String? = null) {
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
            val videoOk = downloadCompanionMusicVideoIfEnabledImpl(requireEngine(), track, wifiOnly)
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
        val stream = resolveBestDownloadStreamImpl(engine, track) ?: run {
            _uiState.update {
                it.copy(
                    status = s.downloadFailedStreamUnavailable,
                    addToPlaylistStatus = s.downloadFailedShort,
                )
            }
            return@launch
        }
        val cleanUrl = SonataViewModel.stripRangeQueryParam(stream.url)
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
                    streamUrl = SonataViewModel.stripRangeQueryParam(recovered.url),
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
                downloadCompanionMusicVideoIfEnabledImpl(engine, track, wifiOnly)
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

internal fun SonataViewModel.downloadCollectionImpl(tracks: List<AndroidTrack>, collectionId: String) {
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
                val stream = resolveBestDownloadStreamImpl(engine, track) ?: continue
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
                    streamUrl = SonataViewModel.stripRangeQueryParam(stream.url),
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
                            streamUrl = SonataViewModel.stripRangeQueryParam(recovered.url),
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
                val videoOk = downloadCompanionMusicVideoIfEnabledImpl(engine, track, wifiOnly)
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

internal suspend fun SonataViewModel.downloadCompanionMusicVideoIfEnabledImpl(
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
            ?.takeIf { SystemClock.elapsedRealtime() - it.cachedAtMs < SonataViewModel.ARTIST_CACHE_TTL_MS }
            ?.candidate
        val candidate = cachedCandidate
            ?: runSuspendCatching { engine.resolveMusicVideoCandidate(track) }
                .getOrNull()
                ?.also { fetched ->
                    musicVideoCandidateCache[track.id] = SonataViewModel.CachedVideoCandidate(
                        candidate = fetched,
                        cachedAtMs = SystemClock.elapsedRealtime(),
                    )
                }
            ?: return false

        val resolved = resolveOrFetchMusicVideoImpl(candidate.videoId) ?: return false
        if (resolved.qualities.isEmpty()) return false
        val targetPref = settings.value.videoDownloadQuality.toVideoQualityPreference()
        val selected = SonataViewModel.selectVideoQualityStream(
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

internal fun SonataViewModel.removeDownloadImpl(videoId: String) {
    viewModelScope.launch(Dispatchers.IO) {
        downloadManager.removeDownload(videoId)
    }
}

internal fun SonataViewModel.clearAllDownloadsImpl() {
    viewModelScope.launch(Dispatchers.IO) {
        downloadManager.clearAllDownloads()
        _uiState.update { it.copy(status = uiStrings().downloadsClearedStatus) }
    }
}

internal fun SonataViewModel.playDownloadedTrackImpl(record: DownloadedTrackRecord) {
    val queue = downloads.value.map { downloadedRecordToAndroidTrack(it) }
    val target = queue.firstOrNull { it.id == record.videoId } ?: downloadedRecordToAndroidTrack(record)
    playTrack(target, queue.ifEmpty { listOf(target) })
}

internal fun SonataViewModel.downloadedRecordToAndroidTrack(record: DownloadedTrackRecord): AndroidTrack = AndroidTrack(
    id = record.videoId,
    title = record.title,
    artist = record.artist,
    album = record.album.orEmpty(),
    durationSecs = record.durationSecs.takeIf { it > 0L }?.toULong(),
    thumbnailUrl = record.coverUrl,
    liked = record.videoId in _uiState.value.likedTrackIds,
)

internal fun SonataViewModel.collectPersonalRadioPoolImpl(): List<AndroidTrack> {
    val state = _uiState.value
    val local = if (preferences.state.value.localMusicEnabled) {
        state.localMusicLibrary.tracks.map { it.toAndroidTrack() }
    } else {
        emptyList()
    }
    val personalPlTracks = state.personalPlaylists.flatMap { it.tracks }
    val serverTracks = state.serverSourceSnapshots.values.flatMap { it.tracks }
    return (local + personalPlTracks + serverTracks).distinctBy { it.id }
}

internal fun SonataViewModel.startQuickRadioImpl(seedTrack: AndroidTrack?) {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val s = uiStrings()
        _uiState.update { it.copy(status = s.startingRadio) }
        if (preferences.state.value.personalOnlyMode) {
            val pool = collectPersonalRadioPoolImpl().shuffled()
            val queue = if (seedTrack != null) {
                (listOf(seedTrack) + pool.filterNot { it.id == seedTrack.id }).distinctBy { it.id }
            } else {
                pool
            }
            if (queue.isNotEmpty()) {
                playTrack(queue.first(), queue)
            } else {
                _uiState.update { it.copy(status = s.radioUnavailable) }
            }
            return@launch
        }
        val preferredArtists = buildPreferredArtists()

        if (seedTrack != null && !isPersonalTrackId(seedTrack.id)) {
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

internal fun SonataViewModel.startPersonalStationImpl() {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val s = uiStrings()
        _uiState.update { it.copy(status = s.startingPersonalMix) }
        val state = _uiState.value
        if (preferences.state.value.personalOnlyMode) {
            val pool = collectPersonalRadioPoolImpl().shuffled()
            if (pool.isNotEmpty()) {
                playTrack(pool.first(), pool)
            } else {
                _uiState.update { it.copy(status = s.radioUnavailable) }
            }
            return@launch
        }
        val preferredArtists = buildPreferredArtists()
        val seedCandidates = (_lastPlayedTracks.value +
            state.shelves.flatMap { it.tracks }.filter { it.id in state.likedTrackIds })
            .filterNot { isPersonalTrackId(it.id) }
            .distinctBy { it.id }
        val seeds = selectDistinctArtistIds(seedCandidates, maxCount = 3)
            .ifEmpty { listOfNotNull(state.activeTrack?.id?.takeUnless { isPersonalTrackId(it) }) }
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

internal fun SonataViewModel.startDiscoveryStationImpl() {
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val s = uiStrings()
        _uiState.update { it.copy(status = s.startingDiscoveryMix) }
        val state = _uiState.value
        if (preferences.state.value.personalOnlyMode) {
            val historyIds = _lastPlayedTracks.value.mapTo(HashSet()) { it.id }
            val pool = collectPersonalRadioPoolImpl()
            val fresh = pool.filterNot { it.id in historyIds }.ifEmpty { pool }.shuffled()
            if (fresh.isNotEmpty()) {
                playTrack(fresh.first(), fresh)
            } else {
                _uiState.update { it.copy(status = s.radioUnavailable) }
            }
            return@launch
        }
        val preferredArtists = buildPreferredArtists()
        val historyIds = _lastPlayedTracks.value.map { it.id }.toSet()
        val freshCandidates = state.shelves.flatMap { it.tracks }
            .filter { it.id.isNotBlank() && !isPersonalTrackId(it.id) && it.id !in historyIds && it.id !in state.likedTrackIds }
            .distinctBy { it.id }
        val seeds = selectDistinctArtistIds(freshCandidates, maxCount = 3)
            .ifEmpty { selectDistinctArtistIds(_lastPlayedTracks.value.filterNot { isPersonalTrackId(it.id) }, maxCount = 2) }
            .ifEmpty { listOfNotNull(state.activeTrack?.id?.takeUnless { isPersonalTrackId(it) }) }
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

internal fun SonataViewModel.setSleepTimerImpl(minutes: Int?) {
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

internal fun SonataViewModel.resolveCanvasForTrackImpl(
    track: AndroidTrack,
    generation: Long = playbackGeneration.current,
) {
    canvasJob?.cancel()
    val trackId = track.id
    requestedCanvasTrackId = trackId
    _currentCanvas.value = null
    if (preferences.state.value.personalOnlyMode) return
    if (!settings.value.animatedArtwork) return
    if (!settings.value.animatedArtworkCellular) {
        val cm = getApplication<Application>().getSystemService(Context.CONNECTIVITY_SERVICE)
            as? ConnectivityManager
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
            !preferences.state.value.personalOnlyMode &&
            settings.value.animatedArtwork &&
            requestedCanvasTrackId == trackId &&
            isGenerationActive(generation, trackId)
        ) {
            _currentCanvas.value = resolved
        }
    }
}

internal fun SonataViewModel.probeMusicVideoForTrackImpl(
    track: AndroidTrack,
    generation: Long = playbackGeneration.current,
) {
    musicVideoProbeJob?.cancel()
    val trackId = track.id.trim()
    if (preferences.state.value.personalOnlyMode ||
        trackId.isBlank() ||
        trackId.startsWith("local:") ||
        isSelfHostedId(trackId)
    ) {
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
        SystemClock.elapsedRealtime() - it.cachedAtMs < SonataViewModel.ARTIST_CACHE_TTL_MS
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
            musicVideoCandidateCache[trackId] = SonataViewModel.CachedVideoCandidate(
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

internal fun SonataViewModel.enableVideoModeImpl() {
    val state = _uiState.value
    val track = state.activeTrack ?: return
    val candidate = state.musicVideoCandidate ?: return
    val generation = playbackGeneration.current
    videoSwitchJob?.cancel()
    upgradeJob?.cancel()
    PlaybackAudioBridge.applyCrossfade(false, settings.value.crossfadeSeconds)
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
        val resolved = resolveOrFetchMusicVideoImpl(candidate.videoId)
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
        val selected = SonataViewModel.selectVideoQualityStream(
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

internal fun SonataViewModel.selectVideoQualityImpl(
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
        val resolved = resolveOrFetchMusicVideoImpl(candidate.videoId) ?: run {
            _uiState.update { it.copy(videoLoading = false) }
            return@launch
        }
        val selected = SonataViewModel.selectVideoQualityStream(
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

internal fun SonataViewModel.disableVideoModeImpl() {
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
        applyAudioSettings(settings.value)
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
            baseStreamBadges[track.id] = offlineBadge
            val offlineItem = buildOfflineMediaItem(track, offlineUri)
            withContext(Dispatchers.Main) {
                PlaybackAudioBridge.setVideoModePlayback(false)
                applyAudioSettings(settings.value)
                if (isGenerationActive(generation, track.id)) {
                    controller?.apply {
                        setMediaItem(offlineItem, mappedMusicPosMs)
                        prepare()
                        playWhenReady = shouldPlay
                    }
                }
            }
            val displayOfflineBadge = resolveDisplayBadgeForTrack(track.id, offlineBadge)
            _uiState.update {
                if (isGenerationActive(generation, track.id)) {
                    it.copy(
                        videoLoading = false,
                        streamBadge = displayOfflineBadge,
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
        baseStreamBadges[track.id] = stream.badge
        val mediaItem = buildMediaItem(track, stream)
        withContext(Dispatchers.Main) {
            PlaybackAudioBridge.setVideoModePlayback(false)
            applyAudioSettings(settings.value)
            if (isGenerationActive(generation, track.id)) {
                controller?.apply {
                    setMediaItem(mediaItem, mappedMusicPosMs)
                    prepare()
                    playWhenReady = shouldPlay
                }
            }
        }
        val displayAudioBadge = resolveDisplayBadgeForTrack(track.id, stream.badge)
        _uiState.update {
            if (isGenerationActive(generation, track.id)) {
                it.copy(
                    videoLoading = false,
                    streamBadge = displayAudioBadge,
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

internal suspend fun SonataViewModel.resolveOrFetchMusicVideoImpl(videoId: String): AndroidResolvedMusicVideo? {
    val now = SystemClock.elapsedRealtime()
    val cached = resolvedMusicVideoCache[videoId]?.takeIf { now - it.cachedAtMs < SonataViewModel.STREAM_CACHE_TTL_MS }
    if (cached != null) return cached.resolved
    val engine = requireEngine()
    val fetched = runSuspendCatching { engine.resolveMusicVideoStreams(videoId) }.getOrNull()
        ?: return null
    if (fetched.qualities.isNotEmpty()) {
        resolvedMusicVideoCache[videoId] = SonataViewModel.CachedResolvedMusicVideo(resolved = fetched, cachedAtMs = now)
    }
    return fetched
}
