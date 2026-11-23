package dev.sonata.android

import dev.sonata.android.data.SonataEngineGateway
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
    var playlistsHandler: suspend () -> List<AndroidPlaylist> = { emptyList() }

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

    override suspend fun searchSongs(query: String): List<AndroidTrack> = try {
        searchHandler(query)
    } catch (ce: CancellationException) {
        cancelledSearchCount.incrementAndGet()
        throw ce
    }

    override suspend fun fetchCollectionTracks(browseId: String): List<AndroidTrack> = emptyList()

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

    override suspend fun fetchRadioQueue(videoId: String): List<AndroidTrack> = emptyList()

    override suspend fun setLikeStatus(videoId: String, liked: Boolean) = Unit

    override suspend fun reportPlayback(
        videoId: String,
        playlistId: String?,
        durationSecs: ULong?,
    ) = Unit

    override suspend fun addTracksToPlaylist(playlistId: String, videoIds: List<String>) = Unit

    override suspend fun createPlaylist(
        title: String,
        description: String,
        videoIds: List<String>,
    ): String = "PL_FAKE"

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

    override suspend fun clearLyricsCache() = Unit

    override suspend fun checkForUpdates(): AndroidUpdateInfo = checkForUpdatesForVersion("1.0.6")

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
}
