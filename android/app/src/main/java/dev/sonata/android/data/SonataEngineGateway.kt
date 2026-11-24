package dev.sonata.android.data

import kotlinx.coroutines.CancellationException
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidAlbumStat
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidArtistStat
import uniffi.sonata_android.AndroidExploreFeed
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
import uniffi.sonata_android.SonataAndroidEngine

/**
 * Coroutine-safe equivalent of [runCatching] that always rethrows [CancellationException] so
 * cancelling a coroutine propagates immediately to UniFFI futures and never writes stale state.
 */
internal inline fun <T> runSuspendCatching(block: () -> T): Result<T> {
    return try {
        Result.success(block())
    } catch (ce: CancellationException) {
        throw ce
    } catch (t: Throwable) {
        Result.failure(t)
    }
}

/**
 * Testable Android boundary over the native UniFFI [SonataAndroidEngine].
 *
 * Only tiny memory-only operations ([isSignedIn], [setJsCipherSolver]) remain synchronous;
 * every network, resolver, session, and disk/SQLite operation is a cancellable `suspend` call.
 */
interface SonataEngineGateway {
    fun setJsCipherSolver(solver: AndroidJsCipherSolver)
    fun isSignedIn(): Boolean

    suspend fun warmCipherSolver(): Boolean
    suspend fun getCachedAccountProfile(): AndroidAccountProfile?
    suspend fun getCachedLibraryPlaylists(): List<AndroidPlaylist>
    suspend fun setYtmCookieHeader(cookieHeader: String): AndroidAccountProfile?
    suspend fun getAccountProfile(): AndroidAccountProfile?
    suspend fun signOut()

    suspend fun fetchLibraryPlaylists(): List<AndroidPlaylist>
    suspend fun fetchHomeFeed(): List<AndroidHomeShelf>
    suspend fun setLanguage(languageCode: String)
    suspend fun fetchExploreFeed(): AndroidExploreFeed
    suspend fun fetchMoodCategory(browseId: String, params: String): List<AndroidHomeShelf>
    suspend fun fetchHomeMoodChips(): List<AndroidMoodChip>
    suspend fun fetchHomeMoodFeed(params: String?): List<AndroidHomeShelf>
    suspend fun searchSongs(query: String): List<AndroidTrack>
    suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack>
    suspend fun fetchCollectionTracksFresh(browseId: String): List<AndroidTrack>
    suspend fun fetchArtistPage(browseId: String): AndroidArtistPage
    suspend fun resolveArtistBrowseId(name: String): String?
    suspend fun fetchArtistByName(name: String): AndroidArtistPage
    suspend fun resolveAlbumBrowseId(albumName: String, artistName: String): String?
    suspend fun fetchRadioMixes(): List<AndroidHomeShelf>
    suspend fun fetchListeningHistory(): List<AndroidTrack>
    suspend fun resolveExplicitCounterpart(track: AndroidTrack): AndroidTrack?
    suspend fun fetchRadioQueue(videoId: String): List<AndroidTrack>
    suspend fun fetchEndlessRadioQueue(
        seedVideoIds: List<String>,
        playlistId: String?,
        excludeVideoIds: List<String>,
        preferredArtists: List<String>,
    ): List<AndroidTrack>

    suspend fun setLikeStatus(videoId: String, liked: Boolean)
    suspend fun reportPlayback(videoId: String, playlistId: String?, durationSecs: ULong?)
    suspend fun addTracksToPlaylist(playlistId: String, videoIds: List<String>)
    suspend fun createPlaylist(title: String, description: String, videoIds: List<String>): String
    suspend fun setPlaylistSaved(playlistId: String, saved: Boolean)
    suspend fun editPlaylistMetadata(
        playlistId: String,
        title: String?,
        description: String?,
        privacy: String?,
    )
    suspend fun uploadPlaylistCover(
        playlistId: String,
        imageBytes: ByteArray,
        mimeType: String,
    )

    suspend fun resolveStream(track: AndroidTrack): AndroidResolvedStream
    suspend fun resolveRecovery(track: AndroidTrack): AndroidResolvedStream
    suspend fun resolveHifiUpgrade(
        track: AndroidTrack,
        current: AndroidResolvedStream,
    ): AndroidResolvedStream?

    suspend fun fetchLyrics(track: AndroidTrack): List<AndroidLyricLine>
    suspend fun clearLyricsCache()
    suspend fun checkForUpdates(): AndroidUpdateInfo
    suspend fun checkForUpdatesForVersion(currentVersion: String): AndroidUpdateInfo

    suspend fun searchJiosaavn(query: String, limit: UInt): List<AndroidTrack>
    suspend fun resolveJiosaavnById(rawId: String): AndroidJioSaavnStream?
    suspend fun findJiosaavnUpgrade(
        track: AndroidTrack,
        currentBitrateKbps: UInt,
    ): AndroidJioSaavnStream?
    suspend fun resolveMotionMedia(
        track: AndroidTrack,
        storefront: String?,
        countryCode: String?,
    ): AndroidMotionMedia?
    suspend fun resolveMusicVideoCandidate(track: AndroidTrack): AndroidMusicVideoCandidate?
    suspend fun resolveMusicVideoStreams(videoId: String): AndroidResolvedMusicVideo

    suspend fun listServers(): List<AndroidServerProfile>
    suspend fun upsertServer(profile: AndroidServerProfile): List<AndroidServerProfile>
    suspend fun deleteServer(serverId: String): List<AndroidServerProfile>
    suspend fun setServerEnabled(serverId: String, enabled: Boolean): List<AndroidServerProfile>
    suspend fun testServerConnection(profile: AndroidServerProfile): String
    suspend fun authenticateJellyfin(
        baseUrl: String,
        username: String,
        password: String,
        deviceId: String,
    ): AndroidJellyfinAuth
    suspend fun listServerLibraries(serverId: String): List<AndroidServerLibrary>
    suspend fun listServerArtists(serverId: String, libraryId: String?): List<uniffi.sonata_android.AndroidHomeCard>
    suspend fun listServerAlbums(
        serverId: String,
        libraryId: String?,
        artistId: String?,
    ): List<uniffi.sonata_android.AndroidHomeCard>
    suspend fun getServerAlbum(albumId: String): AndroidServerAlbum
    suspend fun listServerPlaylists(serverId: String): List<AndroidPlaylist>
    suspend fun getServerPlaylistTracks(playlistId: String): List<AndroidTrack>
    suspend fun browseWebdavFolder(serverId: String, folderPath: String?): AndroidWebDavListing
    suspend fun searchServer(serverId: String, query: String, limit: UInt): AndroidServerSearchResult
    suspend fun searchAllServers(query: String, limitPerServer: UInt): AndroidServerSearchResult
    suspend fun resolveServerStream(track: AndroidTrack): AndroidServerStream
    suspend fun probeServerHealth(serverId: String): AndroidServerHealthReport
    suspend fun runServerSpeedTest(serverId: String): AndroidServerSpeedTestReport

    suspend fun listPersonalPlaylists(): List<AndroidPersonalPlaylist>
    suspend fun createPersonalPlaylist(name: String): AndroidPersonalPlaylist
    suspend fun renamePersonalPlaylist(playlistId: String, newName: String): AndroidPersonalPlaylist
    suspend fun deletePersonalPlaylist(playlistId: String): Boolean
    suspend fun addTrackToPersonalPlaylist(playlistId: String, track: AndroidTrack): AndroidPersonalPlaylist
    suspend fun removeTrackFromPersonalPlaylist(playlistId: String, trackId: String): AndroidPersonalPlaylist

    suspend fun attributePersonalTrackToYtm(track: AndroidTrack): String?

    suspend fun getScrobbleConfig(): AndroidScrobbleConfig
    suspend fun saveScrobbleConfig(config: AndroidScrobbleConfig): AndroidScrobbleConfig
    suspend fun scrobbleNowPlaying(track: AndroidTrack)
    suspend fun scrobbleSubmit(track: AndroidTrack, timestampUnix: ULong?)

    suspend fun recordListeningStat(
        track: AndroidTrack,
        sourceTag: String,
        playedSecs: ULong,
        playedAtUnix: ULong?,
    )
    suspend fun queryListeningStats(window: AndroidStatsWindow): AndroidStatsSummary
}

/** Production [SonataEngineGateway] backed directly by UniFFI async [SonataAndroidEngine]. */
class NativeSonataEngineGateway(
    private val engine: SonataAndroidEngine,
) : SonataEngineGateway {
    override fun setJsCipherSolver(solver: AndroidJsCipherSolver) = engine.setJsCipherSolver(solver)

    override fun isSignedIn(): Boolean = engine.isSignedIn()

    override suspend fun warmCipherSolver(): Boolean = engine.warmCipherSolver()

    override suspend fun getCachedAccountProfile(): AndroidAccountProfile? =
        engine.getCachedAccountProfile()

    override suspend fun getCachedLibraryPlaylists(): List<AndroidPlaylist> =
        engine.getCachedLibraryPlaylists()

    override suspend fun setYtmCookieHeader(cookieHeader: String): AndroidAccountProfile? =
        engine.setYtmCookieHeader(cookieHeader)

    override suspend fun getAccountProfile(): AndroidAccountProfile? = engine.getAccountProfile()

    override suspend fun signOut() = engine.signOut()

    override suspend fun fetchLibraryPlaylists(): List<AndroidPlaylist> =
        engine.fetchLibraryPlaylists()

    override suspend fun fetchHomeFeed(): List<AndroidHomeShelf> = engine.fetchHomeFeed()

    override suspend fun setLanguage(languageCode: String) = engine.setLanguage(languageCode)

    override suspend fun fetchExploreFeed(): AndroidExploreFeed = engine.fetchExploreFeed()

    override suspend fun fetchMoodCategory(
        browseId: String,
        params: String,
    ): List<AndroidHomeShelf> = engine.fetchMoodCategory(browseId, params)

    override suspend fun fetchHomeMoodChips(): List<AndroidMoodChip> = engine.fetchHomeMoodChips()

    override suspend fun fetchHomeMoodFeed(params: String?): List<AndroidHomeShelf> =
        engine.fetchHomeMoodFeed(params)

    override suspend fun searchSongs(query: String): List<AndroidTrack> = engine.searchSongs(query)

    override suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack> =
        engine.fetchCollectionTracks(browseId)

    override suspend fun fetchCollectionTracksFresh(browseId: String): List<AndroidTrack> =
        engine.fetchCollectionTracksFresh(browseId)

    override suspend fun fetchArtistPage(browseId: String): AndroidArtistPage =
        engine.fetchArtistPage(browseId)

    override suspend fun resolveArtistBrowseId(name: String): String? =
        engine.resolveArtistBrowseId(name)

    override suspend fun fetchArtistByName(name: String): AndroidArtistPage =
        engine.fetchArtistByName(name)

    override suspend fun resolveAlbumBrowseId(albumName: String, artistName: String): String? =
        engine.resolveAlbumBrowseId(albumName, artistName)

    override suspend fun fetchRadioMixes(): List<AndroidHomeShelf> = engine.fetchRadioMixes()

    override suspend fun fetchListeningHistory(): List<AndroidTrack> =
        engine.fetchListeningHistory()

    override suspend fun resolveExplicitCounterpart(track: AndroidTrack): AndroidTrack? =
        engine.resolveExplicitCounterpart(track)

    override suspend fun fetchRadioQueue(videoId: String): List<AndroidTrack> =
        engine.fetchRadioQueue(videoId)

    override suspend fun fetchEndlessRadioQueue(
        seedVideoIds: List<String>,
        playlistId: String?,
        excludeVideoIds: List<String>,
        preferredArtists: List<String>,
    ): List<AndroidTrack> = engine.fetchEndlessRadioQueue(
        seedVideoIds = seedVideoIds,
        playlistId = playlistId,
        excludeVideoIds = excludeVideoIds,
        preferredArtists = preferredArtists,
    )

    override suspend fun setLikeStatus(videoId: String, liked: Boolean) =
        engine.setLikeStatus(videoId, liked)

    override suspend fun reportPlayback(
        videoId: String,
        playlistId: String?,
        durationSecs: ULong?,
    ) = engine.reportPlayback(videoId, playlistId, durationSecs)

    override suspend fun addTracksToPlaylist(playlistId: String, videoIds: List<String>) =
        engine.addTracksToPlaylist(playlistId, videoIds)

    override suspend fun createPlaylist(
        title: String,
        description: String,
        videoIds: List<String>,
    ): String = engine.createPlaylist(title, description, videoIds)

    override suspend fun setPlaylistSaved(playlistId: String, saved: Boolean) =
        engine.setPlaylistSaved(playlistId, saved)

    override suspend fun editPlaylistMetadata(
        playlistId: String,
        title: String?,
        description: String?,
        privacy: String?,
    ) = engine.editPlaylistMetadata(playlistId, title, description, privacy)

    override suspend fun uploadPlaylistCover(
        playlistId: String,
        imageBytes: ByteArray,
        mimeType: String,
    ) = engine.uploadPlaylistCover(playlistId, imageBytes, mimeType)

    override suspend fun resolveStream(track: AndroidTrack): AndroidResolvedStream =
        engine.resolveStream(track)

    override suspend fun resolveRecovery(track: AndroidTrack): AndroidResolvedStream =
        engine.resolveRecovery(track)

    override suspend fun resolveHifiUpgrade(
        track: AndroidTrack,
        current: AndroidResolvedStream,
    ): AndroidResolvedStream? = engine.resolveHifiUpgrade(track, current)

    override suspend fun fetchLyrics(track: AndroidTrack): List<AndroidLyricLine> =
        engine.fetchLyrics(track)

    override suspend fun clearLyricsCache() = engine.clearLyricsCache()

    override suspend fun checkForUpdates(): AndroidUpdateInfo = engine.checkForUpdates()

    override suspend fun checkForUpdatesForVersion(currentVersion: String): AndroidUpdateInfo =
        engine.checkForUpdatesForVersion(currentVersion)

    override suspend fun searchJiosaavn(query: String, limit: UInt): List<AndroidTrack> =
        engine.searchJiosaavn(query, limit)

    override suspend fun resolveJiosaavnById(rawId: String): AndroidJioSaavnStream? =
        engine.resolveJiosaavnById(rawId)

    override suspend fun findJiosaavnUpgrade(
        track: AndroidTrack,
        currentBitrateKbps: UInt,
    ): AndroidJioSaavnStream? = engine.findJiosaavnUpgrade(track, currentBitrateKbps)

    override suspend fun resolveMotionMedia(
        track: AndroidTrack,
        storefront: String?,
        countryCode: String?,
    ): AndroidMotionMedia? = engine.resolveMotionMedia(track, storefront, countryCode)

    override suspend fun resolveMusicVideoCandidate(
        track: AndroidTrack,
    ): AndroidMusicVideoCandidate? = engine.resolveMusicVideoCandidate(track)

    override suspend fun resolveMusicVideoStreams(
        videoId: String,
    ): AndroidResolvedMusicVideo = engine.resolveMusicVideoStreams(videoId)

    override suspend fun listServers(): List<AndroidServerProfile> = engine.listServers()

    override suspend fun upsertServer(profile: AndroidServerProfile): List<AndroidServerProfile> =
        engine.upsertServer(profile)

    override suspend fun deleteServer(serverId: String): List<AndroidServerProfile> =
        engine.deleteServer(serverId)

    override suspend fun setServerEnabled(
        serverId: String,
        enabled: Boolean,
    ): List<AndroidServerProfile> = engine.setServerEnabled(serverId, enabled)

    override suspend fun testServerConnection(profile: AndroidServerProfile): String =
        engine.testServerConnection(profile)

    override suspend fun authenticateJellyfin(
        baseUrl: String,
        username: String,
        password: String,
        deviceId: String,
    ): AndroidJellyfinAuth = engine.authenticateJellyfin(baseUrl, username, password, deviceId)

    override suspend fun listServerLibraries(serverId: String): List<AndroidServerLibrary> =
        engine.listServerLibraries(serverId)

    override suspend fun listServerArtists(
        serverId: String,
        libraryId: String?,
    ): List<uniffi.sonata_android.AndroidHomeCard> = engine.listServerArtists(serverId, libraryId)

    override suspend fun listServerAlbums(
        serverId: String,
        libraryId: String?,
        artistId: String?,
    ): List<uniffi.sonata_android.AndroidHomeCard> =
        engine.listServerAlbums(serverId, libraryId, artistId)

    override suspend fun getServerAlbum(albumId: String): AndroidServerAlbum =
        engine.getServerAlbum(albumId)

    override suspend fun listServerPlaylists(serverId: String): List<AndroidPlaylist> =
        engine.listServerPlaylists(serverId)

    override suspend fun getServerPlaylistTracks(playlistId: String): List<AndroidTrack> =
        engine.getServerPlaylistTracks(playlistId)

    override suspend fun browseWebdavFolder(
        serverId: String,
        folderPath: String?,
    ): AndroidWebDavListing = engine.browseWebdavFolder(serverId, folderPath)

    override suspend fun searchServer(
        serverId: String,
        query: String,
        limit: UInt,
    ): AndroidServerSearchResult = engine.searchServer(serverId, query, limit)

    override suspend fun searchAllServers(
        query: String,
        limitPerServer: UInt,
    ): AndroidServerSearchResult = engine.searchAllServers(query, limitPerServer)

    override suspend fun resolveServerStream(track: AndroidTrack): AndroidServerStream =
        engine.resolveServerStream(track)

    override suspend fun probeServerHealth(serverId: String): AndroidServerHealthReport =
        engine.probeServerHealth(serverId)

    override suspend fun runServerSpeedTest(serverId: String): AndroidServerSpeedTestReport =
        engine.runServerSpeedTest(serverId)

    override suspend fun listPersonalPlaylists(): List<AndroidPersonalPlaylist> =
        engine.listPersonalPlaylists()

    override suspend fun createPersonalPlaylist(name: String): AndroidPersonalPlaylist =
        engine.createPersonalPlaylist(name)

    override suspend fun renamePersonalPlaylist(
        playlistId: String,
        newName: String,
    ): AndroidPersonalPlaylist = engine.renamePersonalPlaylist(playlistId, newName)

    override suspend fun deletePersonalPlaylist(playlistId: String): Boolean =
        engine.deletePersonalPlaylist(playlistId)

    override suspend fun addTrackToPersonalPlaylist(
        playlistId: String,
        track: AndroidTrack,
    ): AndroidPersonalPlaylist = engine.addTrackToPersonalPlaylist(playlistId, track)

    override suspend fun removeTrackFromPersonalPlaylist(
        playlistId: String,
        trackId: String,
    ): AndroidPersonalPlaylist = engine.removeTrackFromPersonalPlaylist(playlistId, trackId)

    override suspend fun attributePersonalTrackToYtm(track: AndroidTrack): String? =
        engine.attributePersonalTrackToYtm(track)

    override suspend fun getScrobbleConfig(): AndroidScrobbleConfig =
        engine.getScrobbleConfig()

    override suspend fun saveScrobbleConfig(config: AndroidScrobbleConfig): AndroidScrobbleConfig =
        engine.saveScrobbleConfig(config)

    override suspend fun scrobbleNowPlaying(track: AndroidTrack) =
        engine.scrobbleNowPlaying(track)

    override suspend fun scrobbleSubmit(track: AndroidTrack, timestampUnix: ULong?) =
        engine.scrobbleSubmit(track, timestampUnix)

    override suspend fun recordListeningStat(
        track: AndroidTrack,
        sourceTag: String,
        playedSecs: ULong,
        playedAtUnix: ULong?,
    ) = engine.recordListeningStat(track, sourceTag, playedSecs, playedAtUnix)

    override suspend fun queryListeningStats(window: AndroidStatsWindow): AndroidStatsSummary =
        engine.queryListeningStats(window)
}
