package dev.sonata.android.playback

import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.ui.i18n.EcosystemStrings
import dev.sonata.android.ui.i18n.ecosystemStrings
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import java.util.Locale

/**
 * Android Auto and Media3 `MediaLibraryService` browse tree and search provider.
 *
 * Exposes a browseable root (`sonata:root`) with top-level categories (`Liked Music`,
 * `Playlists` / `Personal Playlists`, `Downloads`, `Local Music`, and `Recently Played`),
 * dynamically backed by [AutoDataProvider] callbacks populated by `SonataViewModel`.
 */
class SonataAutoBrowser(
    @Volatile var dataProvider: AutoDataProvider = EmptyAutoDataProvider,
    @Volatile var stringsProvider: () -> EcosystemStrings = { ecosystemStrings(AppLanguage.SYSTEM) },
) {
    /**
     * Dynamic library data source implemented by the UI/ViewModel layer without coupling
     * `PlaybackService` directly to ViewModel internals.
     */
    interface AutoDataProvider {
        fun getLikedTracks(): List<AndroidTrack> = emptyList()
        fun getPlaylists(): List<AndroidPlaylist> = emptyList()
        fun getPlaylistTracks(playlistId: String): List<AndroidTrack> = emptyList()
        fun getDownloadedTracks(): List<AndroidTrack> = emptyList()
        fun getLocalTracks(): List<AndroidTrack> = emptyList()
        fun getRecentlyPlayedTracks(): List<AndroidTrack> = emptyList()
        fun searchTracks(query: String): List<AndroidTrack> = emptyList()
        fun getTrackById(mediaId: String): AndroidTrack? = null
    }

    private object EmptyAutoDataProvider : AutoDataProvider

    fun getRootItem(): MediaItem {
        val strings = stringsProvider()
        return buildFolderItem(
            mediaId = ROOT_ID,
            title = strings.autoRootTitle,
            subtitle = null,
            mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
        )
    }

    /**
     * Returns browseable children for [parentId] sliced cleanly by [page] and [pageSize].
     * Returns `null` when [parentId] is unrecognized.
     */
    fun getChildren(
        parentId: String,
        page: Int = 0,
        pageSize: Int = DEFAULT_PAGE_SIZE,
    ): List<MediaItem>? {
        val items: List<MediaItem> = when {
            parentId == ROOT_ID -> buildRootChildren()
            parentId == LIKED_ID -> dataProvider.getLikedTracks().map(::trackToMediaItem)
            parentId == PLAYLISTS_ID -> dataProvider.getPlaylists().map(::playlistToMediaItem)
            parentId == DOWNLOADS_ID -> dataProvider.getDownloadedTracks().map(::trackToMediaItem)
            parentId == LOCAL_ID -> dataProvider.getLocalTracks().map(::trackToMediaItem)
            parentId == RECENT_ID -> dataProvider.getRecentlyPlayedTracks().map(::trackToMediaItem)
            parentId.startsWith(PLAYLIST_PREFIX) -> {
                val playlistId = parentId.removePrefix(PLAYLIST_PREFIX)
                if (playlistId.isBlank()) return null
                dataProvider.getPlaylistTracks(playlistId).map(::trackToMediaItem)
            }
            else -> return null
        }
        return paginate(items, page, pageSize)
    }

    /**
     * Resolves a single browseable node or playable track by [mediaId].
     */
    fun getItem(mediaId: String): MediaItem? {
        if (mediaId.isBlank()) return null
        val strings = stringsProvider()
        when (mediaId) {
            ROOT_ID -> return getRootItem()
            LIKED_ID -> return buildFolderItem(
                mediaId = LIKED_ID,
                title = strings.autoLikedMusic,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_PLAYLISTS,
            )
            PLAYLISTS_ID -> return buildFolderItem(
                mediaId = PLAYLISTS_ID,
                title = strings.autoPlaylists,
                subtitle = strings.autoPersonalPlaylists,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_PLAYLISTS,
            )
            DOWNLOADS_ID -> return buildFolderItem(
                mediaId = DOWNLOADS_ID,
                title = strings.autoDownloads,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
            )
            LOCAL_ID -> return buildFolderItem(
                mediaId = LOCAL_ID,
                title = strings.autoLocalMusic,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_ALBUMS,
            )
            RECENT_ID -> return buildFolderItem(
                mediaId = RECENT_ID,
                title = strings.autoRecentlyPlayed,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
            )
        }
        if (mediaId.startsWith(PLAYLIST_PREFIX)) {
            val playlistId = mediaId.removePrefix(PLAYLIST_PREFIX)
            val playlist = dataProvider.getPlaylists().firstOrNull { it.id == playlistId }
                ?: return null
            return playlistToMediaItem(playlist)
        }
        val track = findTrackById(mediaId) ?: return null
        return trackToMediaItem(track)
    }

    /**
     * Searches tracks via [AutoDataProvider.searchTracks] plus case-insensitive matching
     * across liked, downloaded, local, and recently played tracks.
     */
    fun search(
        query: String,
        page: Int = 0,
        pageSize: Int = DEFAULT_PAGE_SIZE,
    ): List<MediaItem> {
        val clean = query.trim()
        if (clean.isEmpty()) return emptyList()
        val lowered = clean.lowercase(Locale.ROOT)
        val combined = LinkedHashMap<String, AndroidTrack>()

        dataProvider.searchTracks(clean).forEach { track ->
            combined.putIfAbsent(track.id, track)
        }
        val localPools = sequenceOf(
            dataProvider.getLikedTracks(),
            dataProvider.getDownloadedTracks(),
            dataProvider.getLocalTracks(),
            dataProvider.getRecentlyPlayedTracks(),
        ).flatten()
        localPools.forEach { track ->
            if (trackMatchesQuery(track, lowered)) {
                combined.putIfAbsent(track.id, track)
            }
        }
        return paginate(combined.values.map(::trackToMediaItem), page, pageSize)
    }

    private fun buildRootChildren(): List<MediaItem> {
        val strings = stringsProvider()
        return listOf(
            buildFolderItem(
                mediaId = LIKED_ID,
                title = strings.autoLikedMusic,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_PLAYLISTS,
            ),
            buildFolderItem(
                mediaId = PLAYLISTS_ID,
                title = strings.autoPlaylists,
                subtitle = strings.autoPersonalPlaylists,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_PLAYLISTS,
            ),
            buildFolderItem(
                mediaId = DOWNLOADS_ID,
                title = strings.autoDownloads,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
            ),
            buildFolderItem(
                mediaId = LOCAL_ID,
                title = strings.autoLocalMusic,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_ALBUMS,
            ),
            buildFolderItem(
                mediaId = RECENT_ID,
                title = strings.autoRecentlyPlayed,
                subtitle = null,
                mediaType = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
            ),
        )
    }

    private fun findTrackById(mediaId: String): AndroidTrack? {
        dataProvider.getTrackById(mediaId)?.let { return it }
        return sequenceOf(
            dataProvider.getLikedTracks(),
            dataProvider.getDownloadedTracks(),
            dataProvider.getLocalTracks(),
            dataProvider.getRecentlyPlayedTracks(),
        ).flatten().firstOrNull { it.id == mediaId }
    }

    private fun playlistToMediaItem(playlist: AndroidPlaylist): MediaItem {
        val artwork = safeParseUri(playlist.thumbnailUrl)
        val metadata = MediaMetadata.Builder()
            .setTitle(playlist.name)
            .setSubtitle(
                if (playlist.id.startsWith("personal:")) {
                    stringsProvider().autoPersonalPlaylists
                } else {
                    null
                },
            )
            .setIsBrowsable(true)
            .setIsPlayable(false)
            .setMediaType(MediaMetadata.MEDIA_TYPE_PLAYLIST)
            .apply {
                if (artwork != null) setArtworkUri(artwork)
            }
            .build()
        return MediaItem.Builder()
            .setMediaId("$PLAYLIST_PREFIX${playlist.id}")
            .setMediaMetadata(metadata)
            .build()
    }

    private fun trackToMediaItem(track: AndroidTrack): MediaItem {
        val artwork = safeParseUri(track.thumbnailUrl)
        val metadata = MediaMetadata.Builder()
            .setTitle(track.title)
            .setArtist(track.artist)
            .setAlbumTitle(track.album)
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
            .apply {
                if (artwork != null) setArtworkUri(artwork)
            }
            .build()
        return MediaItem.Builder()
            .setMediaId(track.id)
            .setMediaMetadata(metadata)
            .build()
    }

    private fun buildFolderItem(
        mediaId: String,
        title: String,
        subtitle: String?,
        mediaType: Int,
    ): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(title)
            .setSubtitle(subtitle)
            .setIsBrowsable(true)
            .setIsPlayable(false)
            .setMediaType(mediaType)
            .build()
        return MediaItem.Builder()
            .setMediaId(mediaId)
            .setMediaMetadata(metadata)
            .build()
    }

    companion object {
        const val ROOT_ID = "sonata:root"
        const val LIKED_ID = "sonata:liked"
        const val PLAYLISTS_ID = "sonata:playlists"
        const val DOWNLOADS_ID = "sonata:downloads"
        const val LOCAL_ID = "sonata:local"
        const val RECENT_ID = "sonata:recent"
        const val PLAYLIST_PREFIX = "sonata:playlist:"
        const val DEFAULT_PAGE_SIZE = 50

        internal fun trackMatchesQuery(track: AndroidTrack, loweredQuery: String): Boolean {
            return track.title.lowercase(Locale.ROOT).contains(loweredQuery) ||
                track.artist.lowercase(Locale.ROOT).contains(loweredQuery) ||
                track.album.lowercase(Locale.ROOT).contains(loweredQuery)
        }

        internal fun <T> paginate(items: List<T>, page: Int, pageSize: Int): List<T> {
            if (items.isEmpty()) return emptyList()
            val safePage = page.coerceAtLeast(0)
            val safeSize = pageSize.coerceAtLeast(1)
            val from = safePage * safeSize
            if (from >= items.size) return emptyList()
            val to = minOf(from + safeSize, items.size)
            return items.subList(from, to)
        }

        private fun safeParseUri(raw: String?): Uri? {
            val trimmed = raw?.trim()?.takeIf { it.isNotEmpty() } ?: return null
            return runCatching { Uri.parse(trimmed) }.getOrNull()
        }
    }
}
