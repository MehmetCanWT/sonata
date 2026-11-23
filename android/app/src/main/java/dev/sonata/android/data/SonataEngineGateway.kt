package dev.sonata.android.data

import kotlinx.coroutines.CancellationException
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidJioSaavnStream
import uniffi.sonata_android.AndroidJsCipherSolver
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidMotionMedia
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
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
    suspend fun searchSongs(query: String): List<AndroidTrack>
    suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack>
    suspend fun fetchArtistPage(browseId: String): AndroidArtistPage
    suspend fun resolveArtistBrowseId(name: String): String?
    suspend fun fetchArtistByName(name: String): AndroidArtistPage
    suspend fun resolveAlbumBrowseId(albumName: String, artistName: String): String?
    suspend fun fetchRadioMixes(): List<AndroidHomeShelf>
    suspend fun fetchListeningHistory(): List<AndroidTrack>
    suspend fun resolveExplicitCounterpart(track: AndroidTrack): AndroidTrack?
    suspend fun fetchRadioQueue(videoId: String): List<AndroidTrack>

    suspend fun setLikeStatus(videoId: String, liked: Boolean)
    suspend fun reportPlayback(videoId: String, playlistId: String?, durationSecs: ULong?)
    suspend fun addTracksToPlaylist(playlistId: String, videoIds: List<String>)
    suspend fun createPlaylist(title: String, description: String, videoIds: List<String>): String
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

    override suspend fun searchSongs(query: String): List<AndroidTrack> = engine.searchSongs(query)

    override suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack> =
        engine.fetchCollectionTracks(browseId)

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
}
