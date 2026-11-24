package dev.sonata.android.playback

import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import java.util.Locale

/**
 * Snapshot-backed [SonataAutoBrowser.AutoDataProvider] powering Android Auto browse trees
 * (`Liked Music`, `Playlists`, `Personal Playlists`, `Downloads`, `Local Music`, and
 * `Recently Played`) and car UI track/playlist playback resolution.
 */
class LiveAutoDataProvider(
    private val likedTracksProvider: () -> List<AndroidTrack>,
    private val playlistsProvider: () -> List<AndroidPlaylist>,
    private val playlistTracksProvider: (String) -> List<AndroidTrack>,
    private val downloadedTracksProvider: () -> List<AndroidTrack>,
    private val localTracksProvider: () -> List<AndroidTrack>,
    private val recentTracksProvider: () -> List<AndroidTrack>,
    private val fallbackTracksProvider: () -> List<AndroidTrack> = { emptyList() },
) : SonataAutoBrowser.AutoDataProvider {

    override fun getLikedTracks(): List<AndroidTrack> = likedTracksProvider()

    override fun getPlaylists(): List<AndroidPlaylist> = playlistsProvider()

    override fun getPlaylistTracks(playlistId: String): List<AndroidTrack> =
        playlistTracksProvider(playlistId)

    override fun getDownloadedTracks(): List<AndroidTrack> = downloadedTracksProvider()

    override fun getLocalTracks(): List<AndroidTrack> = localTracksProvider()

    override fun getRecentlyPlayedTracks(): List<AndroidTrack> = recentTracksProvider()

    override fun searchTracks(query: String): List<AndroidTrack> {
        val clean = query.trim().lowercase(Locale.ROOT)
        if (clean.isEmpty()) return emptyList()
        val combined = LinkedHashMap<String, AndroidTrack>()
        sequenceOf(
            getLikedTracks(),
            getDownloadedTracks(),
            getLocalTracks(),
            getRecentlyPlayedTracks(),
            fallbackTracksProvider(),
        ).flatten().forEach { track ->
            if (SonataAutoBrowser.trackMatchesQuery(track, clean)) {
                combined.putIfAbsent(track.id, track)
            }
        }
        getPlaylists().forEach { playlist ->
            getPlaylistTracks(playlist.id).forEach { track ->
                if (SonataAutoBrowser.trackMatchesQuery(track, clean)) {
                    combined.putIfAbsent(track.id, track)
                }
            }
        }
        return combined.values.toList()
    }

    override fun getTrackById(mediaId: String): AndroidTrack? {
        if (mediaId.isBlank()) return null
        val directPools = sequenceOf(
            getLikedTracks(),
            getDownloadedTracks(),
            getLocalTracks(),
            getRecentlyPlayedTracks(),
            fallbackTracksProvider(),
        ).flatten()
        directPools.firstOrNull { it.id == mediaId }?.let { return it }
        getPlaylists().forEach { playlist ->
            getPlaylistTracks(playlist.id).firstOrNull { it.id == mediaId }?.let { return it }
        }
        return null
    }

    /**
     * Resolves `(track, queue)` to play when the user selects [mediaId] in Android Auto.
     */
    fun resolvePlaybackTarget(mediaId: String): Pair<AndroidTrack, List<AndroidTrack>>? {
        if (mediaId.isBlank()) return null
        if (mediaId.startsWith(SonataAutoBrowser.PLAYLIST_PREFIX)) {
            val playlistId = mediaId.removePrefix(SonataAutoBrowser.PLAYLIST_PREFIX)
            val tracks = getPlaylistTracks(playlistId)
            val first = tracks.firstOrNull() ?: return null
            return first to tracks
        }
        val pools = listOf(
            getLikedTracks(),
            getDownloadedTracks(),
            getLocalTracks(),
            getRecentlyPlayedTracks(),
            fallbackTracksProvider(),
        )
        for (pool in pools) {
            val match = pool.firstOrNull { it.id == mediaId }
            if (match != null) {
                return match to pool
            }
        }
        for (playlist in getPlaylists()) {
            val playlistTracks = getPlaylistTracks(playlist.id)
            val match = playlistTracks.firstOrNull { it.id == mediaId }
            if (match != null) {
                return match to playlistTracks
            }
        }
        val fallback = getTrackById(mediaId) ?: return null
        return fallback to listOf(fallback)
    }
}
