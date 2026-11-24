package dev.sonata.android

import dev.sonata.android.data.SonataEngineGateway
import kotlinx.coroutines.CancellationException
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidAlbumStat
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidArtistStat
import uniffi.sonata_android.AndroidExploreFeed
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidJellyfinAuth
import uniffi.sonata_android.AndroidJioSaavnStream
import uniffi.sonata_android.AndroidJsCipherSolver
import uniffi.sonata_android.AndroidLastFmConfig
import uniffi.sonata_android.AndroidListenBrainzConfig
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidMoodCategory
import uniffi.sonata_android.AndroidMoodChip
import uniffi.sonata_android.AndroidMotionMedia
import uniffi.sonata_android.AndroidMusicVideoCandidate
import uniffi.sonata_android.AndroidPersonalPlaylist
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidResolvedMusicVideo
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidScrobbleConfig
import uniffi.sonata_android.AndroidServerAlbum
import uniffi.sonata_android.AndroidServerHealthReport
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerLibrary
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSearchResult
import uniffi.sonata_android.AndroidServerSpeedTestReport
import uniffi.sonata_android.AndroidServerStream
import uniffi.sonata_android.AndroidSourceStat
import uniffi.sonata_android.AndroidStatsSummary
import uniffi.sonata_android.AndroidStatsWindow
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidTrackStat
import uniffi.sonata_android.AndroidUpdateInfo
import uniffi.sonata_android.AndroidWebDavListing
import java.util.concurrent.atomic.AtomicInteger

/**
 * Handwritten deterministic fake implementation of [SonataEngineGateway] for JVM coroutine tests.
 * Tracks cancellation counts so tests prove coroutine cancellation reaches the gateway boundary.
 */
class FakeSonataEngineGateway : SonataEngineGateway {
    var signedIn: Boolean = false
    var cipherSolver: AndroidJsCipherSolver? = null

    val cancelledSearchCount = AtomicInteger(0)
    val cancelledLyricsCount = AtomicInteger(0)
    val cancelledResolveStreamCount = AtomicInteger(0)
    val cancelledRecoveryCount = AtomicInteger(0)
    val cancelledUpgradeCount = AtomicInteger(0)
    val cancelledJioSaavnCount = AtomicInteger(0)
    val cancelledMotionCount = AtomicInteger(0)

    var searchHandler: suspend (String) -> List<AndroidTrack> = { emptyList() }
    var jioSaavnSearchHandler: suspend (String, UInt) -> List<AndroidTrack> = { _, _ -> emptyList() }
    var jioSaavnByIdHandler: suspend (String) -> AndroidJioSaavnStream? = { null }
    var jioSaavnUpgradeHandler: suspend (AndroidTrack, UInt) -> AndroidJioSaavnStream? = { _, _ -> null }
    var motionMediaHandler: suspend (AndroidTrack, String?, String?) -> AndroidMotionMedia? = { _, _, _ -> null }
    var lyricsHandler: suspend (AndroidTrack) -> List<AndroidLyricLine> = { emptyList() }
    var resolveStreamHandler: suspend (AndroidTrack) -> AndroidResolvedStream = { track ->
        AndroidResolvedStream(
            url = "https://stream.test/${track.id}",
            badge = "OPUS 137kbps · HQ",
            headers = emptyList(),
        )
    }
    var resolveRecoveryHandler: suspend (AndroidTrack) -> AndroidResolvedStream = { track ->
        AndroidResolvedStream(
            url = "https://stream.test/recovered/${track.id}",
            badge = "OPUS 256kbps · Extreme",
            headers = emptyList(),
        )
    }
    var resolveHifiUpgradeHandler: suspend (AndroidTrack, AndroidResolvedStream) -> AndroidResolvedStream? =
        { _, _ -> null }
    var resolveExplicitCounterpartHandler: suspend (AndroidTrack) -> AndroidTrack? = { null }
    var homeFeedHandler: suspend () -> List<AndroidHomeShelf> = { emptyList() }
    var exploreFeedHandler: suspend () -> AndroidExploreFeed = {
        AndroidExploreFeed(shelves = emptyList(), moods = emptyList())
    }
    var moodCategoryHandler: suspend (String, String) -> List<AndroidHomeShelf> = { _, _ -> emptyList() }
    var homeMoodChipsHandler: suspend () -> List<AndroidMoodChip> = { emptyList() }
    var homeMoodFeedHandler: suspend (String?) -> List<AndroidHomeShelf> = { emptyList() }
    var playlistsHandler: suspend () -> List<AndroidPlaylist> = { emptyList() }
    var lastLanguageCode: String? = null

    override fun setJsCipherSolver(solver: AndroidJsCipherSolver) {
        cipherSolver = solver
    }

    override fun isSignedIn(): Boolean = signedIn

    override suspend fun warmCipherSolver(): Boolean = true

    override suspend fun getCachedAccountProfile(): AndroidAccountProfile? = null

    override suspend fun getCachedLibraryPlaylists(): List<AndroidPlaylist> = emptyList()

    override suspend fun setYtmCookieHeader(cookieHeader: String): AndroidAccountProfile? {
        signedIn = true
        return null
    }

    override suspend fun getAccountProfile(): AndroidAccountProfile? = null

    override suspend fun signOut() {
        signedIn = false
    }

    override suspend fun fetchLibraryPlaylists(): List<AndroidPlaylist> = playlistsHandler()

    override suspend fun fetchHomeFeed(): List<AndroidHomeShelf> = homeFeedHandler()

    override suspend fun setLanguage(languageCode: String) {
        lastLanguageCode = languageCode
    }

    override suspend fun fetchExploreFeed(): AndroidExploreFeed = exploreFeedHandler()

    override suspend fun fetchMoodCategory(
        browseId: String,
        params: String,
    ): List<AndroidHomeShelf> = moodCategoryHandler(browseId, params)

    override suspend fun fetchHomeMoodChips(): List<AndroidMoodChip> = homeMoodChipsHandler()

    override suspend fun fetchHomeMoodFeed(params: String?): List<AndroidHomeShelf> =
        homeMoodFeedHandler(params)

    override suspend fun searchSongs(query: String): List<AndroidTrack> = try {
        searchHandler(query)
    } catch (ce: CancellationException) {
        cancelledSearchCount.incrementAndGet()
        throw ce
    }

    var collectionTracksHandler: suspend (String) -> List<AndroidTrack> = { emptyList() }
    var savedPlaylistsLog = mutableListOf<Pair<String, Boolean>>()

    override suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack> =
        collectionTracksHandler(browseId)

    override suspend fun fetchCollectionTracksFresh(browseId: String): List<AndroidTrack> =
        collectionTracksHandler(browseId)

    override suspend fun fetchArtistPage(browseId: String): AndroidArtistPage {
        error("Stub fetchArtistPage not configured for $browseId")
    }

    override suspend fun resolveArtistBrowseId(name: String): String? = null

    override suspend fun fetchArtistByName(name: String): AndroidArtistPage {
        error("Stub fetchArtistByName not configured for $name")
    }

    override suspend fun resolveAlbumBrowseId(albumName: String, artistName: String): String? = null

    override suspend fun fetchRadioMixes(): List<AndroidHomeShelf> = emptyList()

    override suspend fun fetchListeningHistory(): List<AndroidTrack> = emptyList()

    override suspend fun resolveExplicitCounterpart(track: AndroidTrack): AndroidTrack? =
        resolveExplicitCounterpartHandler(track)

    var endlessRadioHandler: suspend (List<String>, String?, List<String>, List<String>) -> List<AndroidTrack> =
        { _, _, _, _ -> emptyList() }

    override suspend fun fetchRadioQueue(videoId: String): List<AndroidTrack> = emptyList()

    override suspend fun fetchEndlessRadioQueue(
        seedVideoIds: List<String>,
        playlistId: String?,
        excludeVideoIds: List<String>,
        preferredArtists: List<String>,
    ): List<AndroidTrack> = endlessRadioHandler(seedVideoIds, playlistId, excludeVideoIds, preferredArtists)

    override suspend fun setLikeStatus(videoId: String, liked: Boolean) = Unit

    override suspend fun reportPlayback(
        videoId: String,
        playlistId: String?,
        durationSecs: ULong?,
    ) = Unit

    val ytmPlaylistAddsLog = mutableListOf<Pair<String, List<String>>>()
    val ytmPlaylistCreatesLog = mutableListOf<Pair<String, List<String>>>()
    val ytmAttributionLog = mutableListOf<AndroidTrack>()

    override suspend fun addTracksToPlaylist(playlistId: String, videoIds: List<String>) {
        ytmPlaylistAddsLog.add(playlistId to videoIds)
    }

    override suspend fun createPlaylist(
        title: String,
        description: String,
        videoIds: List<String>,
    ): String {
        ytmPlaylistCreatesLog.add(title to videoIds)
        return "PL_FAKE"
    }

    override suspend fun setPlaylistSaved(playlistId: String, saved: Boolean) {
        savedPlaylistsLog.add(playlistId to saved)
    }

    override suspend fun editPlaylistMetadata(
        playlistId: String,
        title: String?,
        description: String?,
        privacy: String?,
    ) = Unit

    override suspend fun uploadPlaylistCover(
        playlistId: String,
        imageBytes: ByteArray,
        mimeType: String,
    ) = Unit

    override suspend fun resolveStream(track: AndroidTrack): AndroidResolvedStream = try {
        resolveStreamHandler(track)
    } catch (ce: CancellationException) {
        cancelledResolveStreamCount.incrementAndGet()
        throw ce
    }

    override suspend fun resolveRecovery(track: AndroidTrack): AndroidResolvedStream = try {
        resolveRecoveryHandler(track)
    } catch (ce: CancellationException) {
        cancelledRecoveryCount.incrementAndGet()
        throw ce
    }

    override suspend fun resolveHifiUpgrade(
        track: AndroidTrack,
        current: AndroidResolvedStream,
    ): AndroidResolvedStream? = try {
        resolveHifiUpgradeHandler(track, current)
    } catch (ce: CancellationException) {
        cancelledUpgradeCount.incrementAndGet()
        throw ce
    }

    override suspend fun fetchLyrics(track: AndroidTrack): List<AndroidLyricLine> = try {
        lyricsHandler(track)
    } catch (ce: CancellationException) {
        cancelledLyricsCount.incrementAndGet()
        throw ce
    }

    override suspend fun fetchServerLyrics(track: AndroidTrack): List<AndroidLyricLine> = emptyList()

    override suspend fun clearLyricsCache() = Unit

    override suspend fun checkForUpdates(): AndroidUpdateInfo = checkForUpdatesForVersion("1.0.7")

    override suspend fun checkForUpdatesForVersion(currentVersion: String): AndroidUpdateInfo =
        AndroidUpdateInfo(
            currentVersion = currentVersion,
            latestVersion = currentVersion,
            updateAvailable = false,
            releaseUrl = "",
            apkDownloadUrl = null,
            releaseNotes = "",
        )

    override suspend fun searchJiosaavn(query: String, limit: UInt): List<AndroidTrack> = try {
        jioSaavnSearchHandler(query, limit)
    } catch (ce: CancellationException) {
        cancelledJioSaavnCount.incrementAndGet()
        throw ce
    }

    override suspend fun resolveJiosaavnById(rawId: String): AndroidJioSaavnStream? = try {
        jioSaavnByIdHandler(rawId)
    } catch (ce: CancellationException) {
        cancelledJioSaavnCount.incrementAndGet()
        throw ce
    }

    override suspend fun findJiosaavnUpgrade(
        track: AndroidTrack,
        currentBitrateKbps: UInt,
    ): AndroidJioSaavnStream? = try {
        jioSaavnUpgradeHandler(track, currentBitrateKbps)
    } catch (ce: CancellationException) {
        cancelledJioSaavnCount.incrementAndGet()
        throw ce
    }

    override suspend fun resolveMotionMedia(
        track: AndroidTrack,
        storefront: String?,
        countryCode: String?,
    ): AndroidMotionMedia? = try {
        motionMediaHandler(track, storefront, countryCode)
    } catch (ce: CancellationException) {
        cancelledMotionCount.incrementAndGet()
        throw ce
    }

    var musicVideoCandidateHandler: suspend (AndroidTrack) -> AndroidMusicVideoCandidate? = { null }
    var musicVideoStreamsHandler: suspend (String) -> AndroidResolvedMusicVideo = { videoId ->
        AndroidResolvedMusicVideo(
            videoId = videoId,
            qualities = emptyList(),
            audioUrl = null,
            audioBadge = "AAC",
            headers = emptyList(),
        )
    }

    override suspend fun resolveMusicVideoCandidate(
        track: AndroidTrack,
    ): AndroidMusicVideoCandidate? = musicVideoCandidateHandler(track)

    override suspend fun resolveMusicVideoStreams(
        videoId: String,
    ): AndroidResolvedMusicVideo = musicVideoStreamsHandler(videoId)

    val fakeServers = mutableListOf<AndroidServerProfile>()
    var testServerConnectionHandler: suspend (AndroidServerProfile) -> String = { profile ->
        "${profile.kind.name} OK"
    }
    var authenticateJellyfinHandler: suspend (String, String, String, String) -> AndroidJellyfinAuth =
        { _, user, _, _ ->
            AndroidJellyfinAuth(
                accessToken = "fake-jf-token",
                userId = "fake-jf-uid",
                userName = user,
                serverId = "fake-jf-srv",
            )
        }
    var serverLibrariesHandler: suspend (String) -> List<AndroidServerLibrary> = { emptyList() }
    var serverArtistsHandler: suspend (String, String?) -> List<AndroidHomeCard> = { _, _ -> emptyList() }
    var serverAlbumsHandler: suspend (String, String?, String?) -> List<AndroidHomeCard> = { _, _, _ -> emptyList() }
    var serverAlbumHandler: suspend (String) -> AndroidServerAlbum = { albumId ->
        AndroidServerAlbum(
            id = albumId,
            title = "Server Album",
            subtitle = "Server Artist",
            year = null,
            coverUrl = null,
            tracks = emptyList(),
        )
    }
    var serverPlaylistsHandler: suspend (String) -> List<AndroidPlaylist> = { emptyList() }
    var serverPlaylistTracksHandler: suspend (String) -> List<AndroidTrack> = { emptyList() }
    var webdavBrowseHandler: suspend (String, String?) -> AndroidWebDavListing = { _, _ ->
        AndroidWebDavListing(folders = emptyList(), tracks = emptyList())
    }
    var serverSearchHandler: suspend (String, String, UInt) -> AndroidServerSearchResult = { _, _, _ ->
        AndroidServerSearchResult(
            tracks = emptyList(),
            albums = emptyList(),
            artists = emptyList(),
            playlists = emptyList(),
        )
    }
    var allServersSearchHandler: suspend (String, UInt) -> AndroidServerSearchResult = { _, _ ->
        AndroidServerSearchResult(
            tracks = emptyList(),
            albums = emptyList(),
            artists = emptyList(),
            playlists = emptyList(),
        )
    }
    var serverStreamHandler: suspend (AndroidTrack) -> AndroidServerStream = { track ->
        AndroidServerStream(
            url = "https://server.test/stream/${track.id}",
            headers = emptyList(),
            mimeType = "audio/flac",
            bitrateKbps = 900u,
            codec = "flac",
            audioBadge = "FLAC · 900kbps",
        )
    }

    override suspend fun listServers(): List<AndroidServerProfile> = fakeServers.toList()

    override suspend fun upsertServer(profile: AndroidServerProfile): List<AndroidServerProfile> {
        val assigned = if (profile.id.isBlank()) {
            val prefix = when (profile.kind) {
                AndroidServerKind.JELLYFIN -> "jf"
                AndroidServerKind.SUBSONIC -> "sub"
                AndroidServerKind.WEB_DAV -> "dav"
                AndroidServerKind.SMB -> "smb"
            }
            profile.copy(id = "$prefix-${fakeServers.size + 1}")
        } else {
            profile
        }
        val idx = fakeServers.indexOfFirst { it.id == assigned.id }
        if (idx >= 0) {
            fakeServers[idx] = assigned
        } else {
            fakeServers.add(assigned)
        }
        return fakeServers.toList()
    }

    override suspend fun deleteServer(serverId: String): List<AndroidServerProfile> {
        fakeServers.removeAll { it.id == serverId }
        return fakeServers.toList()
    }

    override suspend fun setServerEnabled(
        serverId: String,
        enabled: Boolean,
    ): List<AndroidServerProfile> {
        val idx = fakeServers.indexOfFirst { it.id == serverId }
        if (idx >= 0) {
            fakeServers[idx] = fakeServers[idx].copy(enabled = enabled)
        }
        return fakeServers.toList()
    }

    override suspend fun testServerConnection(profile: AndroidServerProfile): String =
        testServerConnectionHandler(profile)

    override suspend fun authenticateJellyfin(
        baseUrl: String,
        username: String,
        password: String,
        deviceId: String,
    ): AndroidJellyfinAuth = authenticateJellyfinHandler(baseUrl, username, password, deviceId)

    override suspend fun listServerLibraries(serverId: String): List<AndroidServerLibrary> =
        serverLibrariesHandler(serverId)

    override suspend fun listServerArtists(
        serverId: String,
        libraryId: String?,
    ): List<AndroidHomeCard> = serverArtistsHandler(serverId, libraryId)

    override suspend fun listServerAlbums(
        serverId: String,
        libraryId: String?,
        artistId: String?,
    ): List<AndroidHomeCard> = serverAlbumsHandler(serverId, libraryId, artistId)

    override suspend fun getServerAlbum(albumId: String): AndroidServerAlbum =
        serverAlbumHandler(albumId)

    override suspend fun listServerPlaylists(serverId: String): List<AndroidPlaylist> =
        serverPlaylistsHandler(serverId)

    override suspend fun getServerPlaylistTracks(playlistId: String): List<AndroidTrack> =
        serverPlaylistTracksHandler(playlistId)

    override suspend fun browseWebdavFolder(
        serverId: String,
        folderPath: String?,
    ): AndroidWebDavListing = webdavBrowseHandler(serverId, folderPath)

    override suspend fun searchServer(
        serverId: String,
        query: String,
        limit: UInt,
    ): AndroidServerSearchResult = serverSearchHandler(serverId, query, limit)

    override suspend fun searchAllServers(
        query: String,
        limitPerServer: UInt,
    ): AndroidServerSearchResult = allServersSearchHandler(query, limitPerServer)

    override suspend fun resolveServerStream(track: AndroidTrack): AndroidServerStream =
        serverStreamHandler(track)

    var serverHealthHandler: suspend (String) -> AndroidServerHealthReport = { serverId ->
        val exists = fakeServers.any { it.id == serverId }
        AndroidServerHealthReport(
            alive = exists,
            latencyMs = if (exists) 12uL else null,
            message = if (exists) "OK" else "Not found",
        )
    }

    var serverSpeedTestHandler: suspend (String) -> AndroidServerSpeedTestReport = { serverId ->
        val exists = fakeServers.any { it.id == serverId }
        AndroidServerSpeedTestReport(
            alive = exists,
            latencyMs = if (exists) 14uL else 0uL,
            downloadMbps = if (exists) 184.5 else null,
            bytesTested = if (exists) 1_048_576uL else 0uL,
            sampleNote = if (exists) "Downloaded 1048576 bytes in 45ms" else "Not found",
        )
    }

    val fakePersonalPlaylists = mutableListOf<AndroidPersonalPlaylist>()
    private var nextPersonalPlaylistSeq = 1
    var attributePersonalTrackHandler: suspend (AndroidTrack) -> String? = { track ->
        if (signedIn) "ytm_${track.id}" else null
    }

    override suspend fun probeServerHealth(serverId: String): AndroidServerHealthReport =
        serverHealthHandler(serverId)

    override suspend fun runServerSpeedTest(serverId: String): AndroidServerSpeedTestReport =
        serverSpeedTestHandler(serverId)

    override suspend fun listPersonalPlaylists(): List<AndroidPersonalPlaylist> =
        fakePersonalPlaylists.toList()

    override suspend fun createPersonalPlaylist(name: String): AndroidPersonalPlaylist {
        val clean = name.trim()
        require(clean.isNotEmpty()) { "Personal playlist name cannot be empty" }
        val now = (System.currentTimeMillis() / 1000L).toULong()
        val created = AndroidPersonalPlaylist(
            id = "personal:fake_${nextPersonalPlaylistSeq++}",
            name = clean,
            tracks = emptyList(),
            createdAtEpochSecs = now,
            updatedAtEpochSecs = now,
        )
        fakePersonalPlaylists.add(created)
        return created
    }

    override suspend fun renamePersonalPlaylist(
        playlistId: String,
        newName: String,
    ): AndroidPersonalPlaylist {
        val clean = newName.trim()
        require(clean.isNotEmpty()) { "Personal playlist name cannot be empty" }
        val idx = fakePersonalPlaylists.indexOfFirst { it.id == playlistId }
        require(idx >= 0) { "Personal playlist $playlistId not found" }
        val updated = fakePersonalPlaylists[idx].copy(
            name = clean,
            updatedAtEpochSecs = (System.currentTimeMillis() / 1000L).toULong(),
        )
        fakePersonalPlaylists[idx] = updated
        return updated
    }

    override suspend fun deletePersonalPlaylist(playlistId: String): Boolean =
        fakePersonalPlaylists.removeAll { it.id == playlistId }

    override suspend fun addTrackToPersonalPlaylist(
        playlistId: String,
        track: AndroidTrack,
    ): AndroidPersonalPlaylist {
        val idx = fakePersonalPlaylists.indexOfFirst { it.id == playlistId }
        require(idx >= 0) { "Personal playlist $playlistId not found" }
        val current = fakePersonalPlaylists[idx]
        val nextTracks = if (current.tracks.any { it.id == track.id }) {
            current.tracks
        } else {
            current.tracks + track
        }
        val updated = current.copy(
            tracks = nextTracks,
            updatedAtEpochSecs = (System.currentTimeMillis() / 1000L).toULong(),
        )
        fakePersonalPlaylists[idx] = updated
        return updated
    }

    override suspend fun removeTrackFromPersonalPlaylist(
        playlistId: String,
        trackId: String,
    ): AndroidPersonalPlaylist {
        val idx = fakePersonalPlaylists.indexOfFirst { it.id == playlistId }
        require(idx >= 0) { "Personal playlist $playlistId not found" }
        val current = fakePersonalPlaylists[idx]
        val updated = current.copy(
            tracks = current.tracks.filterNot { it.id == trackId },
            updatedAtEpochSecs = (System.currentTimeMillis() / 1000L).toULong(),
        )
        fakePersonalPlaylists[idx] = updated
        return updated
    }

    override suspend fun attributePersonalTrackToYtm(track: AndroidTrack): String? {
        ytmAttributionLog.add(track)
        return attributePersonalTrackHandler(track)
    }

    var fakeScrobbleConfig = AndroidScrobbleConfig(
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
    )
    val nowPlayingScrobblesLog = mutableListOf<AndroidTrack>()
    val submittedScrobblesLog = mutableListOf<Pair<AndroidTrack, ULong?>>()

    data class FakeStatEvent(
        val track: AndroidTrack,
        val sourceTag: String,
        val playedSecs: ULong,
        val playedAtUnix: ULong,
    )

    val recordedStatsLog = mutableListOf<FakeStatEvent>()
    var fakeStatsNowUnix: ULong = 1_700_500_000uL

    override suspend fun getScrobbleConfig(): AndroidScrobbleConfig = fakeScrobbleConfig

    override suspend fun saveScrobbleConfig(config: AndroidScrobbleConfig): AndroidScrobbleConfig {
        fakeScrobbleConfig = config
        return config
    }

    override suspend fun scrobbleNowPlaying(track: AndroidTrack) {
        nowPlayingScrobblesLog.add(track)
    }

    override suspend fun scrobbleSubmit(track: AndroidTrack, timestampUnix: ULong?) {
        submittedScrobblesLog.add(track to timestampUnix)
    }

    override suspend fun recordListeningStat(
        track: AndroidTrack,
        sourceTag: String,
        playedSecs: ULong,
        playedAtUnix: ULong?,
    ) {
        val resolvedTag = sourceTag.trim().ifEmpty {
            when {
                track.id.startsWith("local:") -> "LOC"
                track.id.startsWith("jellyfin:") -> "JFN"
                track.id.startsWith("subsonic:") -> "SUB"
                track.id.startsWith("smb:") -> "SMB"
                track.id.startsWith("webdav:") -> "DAV"
                track.id.startsWith("jiosaavn:") -> "JIO"
                else -> "YTM"
            }
        }
        recordedStatsLog.add(
            FakeStatEvent(
                track = track,
                sourceTag = resolvedTag,
                playedSecs = playedSecs,
                playedAtUnix = playedAtUnix ?: fakeStatsNowUnix,
            ),
        )
    }

    override suspend fun queryListeningStats(window: AndroidStatsWindow): AndroidStatsSummary {
        val now = fakeStatsNowUnix
        val cutoff = when (window) {
            AndroidStatsWindow.WEEK -> if (now > 7uL * 86_400uL) now - 7uL * 86_400uL else 0uL
            AndroidStatsWindow.MONTH -> if (now > 30uL * 86_400uL) now - 30uL * 86_400uL else 0uL
            AndroidStatsWindow.YEAR -> if (now > 365uL * 86_400uL) now - 365uL * 86_400uL else 0uL
            AndroidStatsWindow.ALL_TIME -> 0uL
        }
        val filtered = recordedStatsLog.filter { it.playedAtUnix >= cutoff && it.playedAtUnix <= now }
        val totalPlayedSecs = filtered.sumOf { it.playedSecs }
        val topArtists = filtered
            .filter { it.track.artist.isNotBlank() }
            .groupBy { it.track.artist }
            .map { (artist, events) ->
                AndroidArtistStat(
                    artist = artist,
                    plays = events.size.toULong(),
                    minutes = events.sumOf { it.playedSecs } / 60uL,
                )
            }
            .sortedByDescending { it.plays }
        val topTracks = filtered
            .filter { it.track.title.isNotBlank() }
            .groupBy { Triple(it.track.id, it.track.title, it.track.artist) }
            .map { (_, events) ->
                val sample = events.first()
                AndroidTrackStat(
                    trackId = sample.track.id,
                    title = sample.track.title,
                    artist = sample.track.artist,
                    album = sample.track.album,
                    sourceTag = sample.sourceTag,
                    plays = events.size.toULong(),
                    minutes = events.sumOf { it.playedSecs } / 60uL,
                )
            }
            .sortedByDescending { it.plays }
        val topAlbums = filtered
            .filter { it.track.album.isNotBlank() }
            .groupBy { it.track.album to it.track.artist }
            .map { (key, events) ->
                AndroidAlbumStat(
                    album = key.first,
                    artist = key.second,
                    plays = events.size.toULong(),
                    minutes = events.sumOf { it.playedSecs } / 60uL,
                )
            }
            .sortedByDescending { it.plays }
        val sourceBreakdown = filtered
            .groupBy { it.sourceTag }
            .map { (tag, events) ->
                AndroidSourceStat(
                    sourceTag = tag,
                    plays = events.size.toULong(),
                    minutes = events.sumOf { it.playedSecs } / 60uL,
                )
            }
            .sortedByDescending { it.plays }
        return AndroidStatsSummary(
            totalPlays = filtered.size.toULong(),
            totalMinutes = totalPlayedSecs / 60uL,
            uniqueTracks = filtered.mapNotNull { it.track.id.takeIf(String::isNotBlank) }.toSet().size.toULong(),
            uniqueArtists = filtered.mapNotNull { it.track.artist.takeIf(String::isNotBlank) }.toSet().size.toULong(),
            streakDays = if (filtered.isEmpty()) 0uL else 1uL,
            topArtists = topArtists,
            topTracks = topTracks,
            topAlbums = topAlbums,
            sourceBreakdown = sourceBreakdown,
        )
    }
}
