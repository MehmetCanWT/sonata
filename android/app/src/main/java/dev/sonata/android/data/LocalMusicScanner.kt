package dev.sonata.android.data

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.provider.MediaStore
import androidx.compose.runtime.Immutable
import androidx.core.content.ContextCompat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidTrack
import java.util.Locale
import kotlin.math.roundToInt

private val PERSONAL_TRACK_ID_PREFIXES = listOf(
    "local:",
    "jellyfin:",
    "subsonic:",
    "smb:",
    "webdav:",
)

/**
 * Returns `true` when [id] belongs to a personal local-device or self-hosted server track
 * (`local:`, `jellyfin:`, `subsonic:`, `smb:`, `webdav:`).
 */
fun isPersonalTrackId(id: String): Boolean {
    val normalized = id.trim().lowercase(Locale.ROOT)
    return PERSONAL_TRACK_ID_PREFIXES.any { normalized.startsWith(it) }
}

/**
 * Returns `true` when [id] identifies a local MediaStore track or album (`local:*`).
 */
fun isLocalTrackId(id: String?): Boolean {
    val normalized = id?.trim()?.lowercase(Locale.ROOT) ?: return false
    return normalized.startsWith("local:")
}

/**
 * Returns a fallback `i.ytimg.com` thumbnail URL only for non-personal YouTube Music track IDs
 * when [personalOnlyMode] is disabled.
 */
internal fun resolveYtmFallbackCoverUrl(
    trackId: String?,
    personalOnlyMode: Boolean = false,
): String? {
    if (personalOnlyMode) return null
    val raw = trackId?.trim().orEmpty()
    if (raw.isEmpty()) return null
    val lower = raw.lowercase(Locale.ROOT)
    if (isPersonalTrackId(raw) ||
        lower.startsWith("jiosaavn:") ||
        lower.startsWith("personal:")
    ) {
        return null
    }
    return "https://i.ytimg.com/vi/$raw/hqdefault.jpg"
}

/**
 * Resolves track bitrate in kbps using MediaStore's reported bitrate (in bits/sec) when positive,
 * falling back to `(fileSizeBytes * 8) / durationMs` when MediaStore omits bitrate (common for FLAC/WAV/VBR).
 */
fun resolveLocalBitrateKbps(
    mediaStoreBitrateBps: Int,
    fileSizeBytes: Long,
    durationMs: Long,
): Int {
    if (mediaStoreBitrateBps > 0) {
        val reportedKbps = (mediaStoreBitrateBps / 1000.0).roundToInt()
        if (reportedKbps > 0) return reportedKbps
    }
    return computeMeasuredBitrateKbps(contentBytes = fileSizeBytes, durationMs = durationMs)
}

/**
 * Metadata extracted from a single local audio track discovered via Android [MediaStore].
 */
@Immutable
data class LocalAudioTrack(
    val mediaId: Long,
    val id: String,
    val title: String,
    val artist: String,
    val album: String,
    val albumId: Long,
    val contentUri: String,
    val albumArtUri: String?,
    val durationMs: Long,
    val fileSizeBytes: Long,
    val bitrateKbps: Int,
    val folderName: String,
    val relativePath: String,
    val mimeType: String,
    val trackNumber: Int = 0,
    val dateModifiedSecs: Long = 0L,
) {
    /** Converts this local audio entry into a shared [AndroidTrack] for playback and queue use. */
    fun toAndroidTrack(): AndroidTrack = AndroidTrack(
        id = id,
        title = title,
        artist = artist,
        album = album,
        durationSecs = (durationMs / 1000L).takeIf { it > 0L }?.toULong(),
        thumbnailUrl = albumArtUri,
        liked = false,
        albumId = albumId.takeIf { it > 0L }?.let { "local:album:$it" },
        isExplicit = false,
    )
}

/**
 * Local device tracks grouped by MediaStore album.
 */
@Immutable
data class LocalAlbumGroup(
    val id: String,
    val albumId: Long,
    val title: String,
    val artist: String,
    val albumArtUri: String?,
    val tracks: List<LocalAudioTrack>,
) {
    val trackCount: Int get() = tracks.size
    val totalDurationMs: Long get() = tracks.sumOf { it.durationMs.coerceAtLeast(0L) }
}

/**
 * Local device tracks grouped by storage folder.
 */
@Immutable
data class LocalFolderGroup(
    val folderName: String,
    val relativePath: String,
    val tracks: List<LocalAudioTrack>,
) {
    val trackCount: Int get() = tracks.size
}

/**
 * Complete snapshot of scanned local device audio organized into tracks, albums, and folders.
 */
@Immutable
data class LocalMusicLibrary(
    val tracks: List<LocalAudioTrack> = emptyList(),
    val albums: List<LocalAlbumGroup> = emptyList(),
    val folders: List<LocalFolderGroup> = emptyList(),
    val availableFolders: List<String> = emptyList(),
    val permissionGranted: Boolean = true,
)

/**
 * Stateless Android [MediaStore.Audio.Media] scanner and pure grouping helper for local audio files.
 */
object LocalMusicScanner {
    const val LOCAL_ID_PREFIX = "local:"
    const val ALBUM_ART_URI_PREFIX = "content://media/external/audio/albumart/"
    const val DEFAULT_MIN_DURATION_MS = 1_000L
    const val DEFAULT_FILTERED_MIN_DURATION_MS = 30_000L
    const val DEFAULT_FOLDER_NAME = "Music"

    private val EXCLUDED_SEGMENT_NAMES = setOf(
        "whatsapp",
        "whatsapp audio",
        "whatsapp voice notes",
        "telegram",
        "telegram audio",
        "recordings",
        "voice recorder",
        "sound_recorder",
        "soundrecorder",
        "call",
        "call recordings",
        "callrecordings",
        "record",
        "voice",
        "voicemail",
        "notifications",
        "ringtones",
        "alarms",
        "ui",
        "podcasts",
        "audiobooks",
        "messenger",
        "viber",
        "signal",
        "wechat",
        "snapchat",
        "instagram",
        "tiktok",
        "dcim",
        "movies",
        "screenrecords",
        "screenrecordings",
        "samsung",
    )

    private val EXCLUDED_TITLE_REGEXES = listOf(
        Regex("""^aud-\d{6,8}-wa\d+"""),
        Regex("""^ptt-\d{6,8}-wa\d+"""),
        Regex("""^voice[\s_-]?\d+"""),
        Regex("""^recording[\s_-]?\d+"""),
        Regex("""^call[\s_-]?\d+"""),
        Regex("""^rec[\s_-]?\d+"""),
    )

    /** Normalizes a relative folder path using forward slashes without leading/trailing slashes. */
    fun normalizeFolderPath(raw: String): String = raw.trim()
        .replace('\\', '/')
        .replace(Regex("/+"), "/")
        .trim('/')

    /**
     * Extracts a MediaStore-compatible relative folder path from a Storage Access Framework
     * tree document ID (e.g. `"primary:Music/FLAC"` -> `"Music/FLAC"`), or `null` for root/blank.
     */
    fun folderPathFromTreeDocumentId(documentId: String?): String? {
        val raw = documentId?.trim().orEmpty()
        if (raw.isEmpty()) return null
        val afterVolume = if (':' in raw) raw.substringAfter(':') else raw
        return normalizeFolderPath(afterVolume).takeIf { it.isNotEmpty() }
    }

    /**
     * Returns `true` when an audio entry appears to be a voice note, call recording, notification/ringtone,
     * app cache audio, or short sound clip rather than music.
     */
    fun isExcludedNonMusicAudio(
        title: String,
        relativePath: String,
        folderName: String,
        durationMs: Long,
        minDurationMs: Long = DEFAULT_FILTERED_MIN_DURATION_MS,
    ): Boolean {
        if (minDurationMs > 0L && durationMs in 1L until minDurationMs) {
            return true
        }
        val normalizedPath = normalizeFolderPath(relativePath).lowercase(Locale.ROOT)
        if (normalizedPath == "android/media" ||
            normalizedPath.startsWith("android/media/") ||
            normalizedPath == "android/data" ||
            normalizedPath.startsWith("android/data/")
        ) {
            return true
        }
        val pathSegments = buildList {
            if (normalizedPath.isNotEmpty()) {
                addAll(normalizedPath.split('/').filter { it.isNotBlank() })
            }
            val normFolder = normalizeFolderPath(folderName).lowercase(Locale.ROOT)
            if (normFolder.isNotEmpty()) {
                addAll(normFolder.split('/').filter { it.isNotBlank() })
            }
        }
        if (pathSegments.any { it in EXCLUDED_SEGMENT_NAMES }) {
            return true
        }
        val normalizedTitle = title.trim().lowercase(Locale.ROOT)
        if (normalizedTitle == "over the horizon") {
            return true
        }
        return EXCLUDED_TITLE_REGEXES.any { it.containsMatchIn(normalizedTitle) }
    }

    /**
     * Returns `true` when [selectedFolders] is empty (all music folders included) or when
     * the track's folder matches any selected folder or subfolder prefix (case-insensitive).
     */
    fun matchesSelectedFolders(
        relativePath: String,
        folderName: String,
        selectedFolders: Set<String>,
    ): Boolean {
        val normalizedTargets = selectedFolders
            .map { normalizeFolderPath(it).lowercase(Locale.ROOT) }
            .filter { it.isNotEmpty() }
        if (normalizedTargets.isEmpty()) return true
        val candidate = normalizeFolderPath(relativePath).lowercase(Locale.ROOT)
            .ifEmpty { normalizeFolderPath(folderName).lowercase(Locale.ROOT) }
        if (candidate.isEmpty()) return false
        return normalizedTargets.any { target ->
            candidate == target || candidate.startsWith("$target/")
        }
    }

    /**
     * Returns the runtime storage/audio permission required for the given Android [sdkInt].
     */
    fun requiredReadAudioPermission(sdkInt: Int = Build.VERSION.SDK_INT): String =
        if (sdkInt >= Build.VERSION_CODES.TIRAMISU) {
            Manifest.permission.READ_MEDIA_AUDIO
        } else {
            Manifest.permission.READ_EXTERNAL_STORAGE
        }

    /**
     * Returns `true` when the app currently holds permission to query local audio via [MediaStore].
     */
    fun hasAudioReadPermission(context: Context): Boolean {
        val permission = requiredReadAudioPermission()
        return runCatching {
            ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
        }.getOrDefault(false)
    }

    /** Formats a stable Sonata track identifier (`local:<mediaId>`) for a MediaStore audio row. */
    fun buildLocalTrackId(mediaId: Long): String = "$LOCAL_ID_PREFIX$mediaId"

    /** Formats the standard MediaStore album art content URI for [albumId], or `null` if invalid. */
    fun buildLocalAlbumArtUri(albumId: Long): String? =
        if (albumId > 0L) "$ALBUM_ART_URI_PREFIX$albumId" else null

    /**
     * Extracts a human-readable folder name from MediaStore's `BUCKET_DISPLAY_NAME` or `RELATIVE_PATH`.
     */
    fun extractFolderName(bucketDisplayName: String?, relativePath: String?): String {
        val bucket = bucketDisplayName?.trim().orEmpty()
        if (bucket.isNotEmpty()) return bucket
        val path = relativePath?.trim()?.trimEnd('/', '\\').orEmpty()
        if (path.isNotEmpty()) {
            val lastSegment = path.substringAfterLast('/').substringAfterLast('\\').trim()
            if (lastSegment.isNotEmpty()) return lastSegment
        }
        return DEFAULT_FOLDER_NAME
    }

    /** Groups scanned [tracks] into sorted [LocalAlbumGroup] entries. */
    fun groupLocalAlbums(tracks: List<LocalAudioTrack>): List<LocalAlbumGroup> {
        if (tracks.isEmpty()) return emptyList()
        return tracks
            .groupBy { track ->
                if (track.albumId > 0L) {
                    "id:${track.albumId}"
                } else {
                    "name:${track.album.trim().lowercase(Locale.ROOT)}"
                }
            }
            .values
            .map { group ->
                val sortedTracks = group.sortedWith(
                    compareBy<LocalAudioTrack>(
                        { if (it.trackNumber > 0) it.trackNumber else Int.MAX_VALUE },
                        { it.title.lowercase(Locale.ROOT) },
                    ),
                )
                val head = sortedTracks.first()
                val albumTitle = head.album.trim().ifEmpty { "Unknown Album" }
                val artists = sortedTracks.map { it.artist.trim() }.filter { it.isNotEmpty() }.distinct()
                val albumArtist = when {
                    artists.isEmpty() -> "Unknown Artist"
                    artists.size == 1 -> artists.first()
                    else -> "Various Artists"
                }
                val albumArt = sortedTracks.firstNotNullOfOrNull { it.albumArtUri }
                LocalAlbumGroup(
                    id = if (head.albumId > 0L) "local:album:${head.albumId}" else "local:album:${albumTitle.lowercase(Locale.ROOT)}",
                    albumId = head.albumId,
                    title = albumTitle,
                    artist = albumArtist,
                    albumArtUri = albumArt,
                    tracks = sortedTracks,
                )
            }
            .sortedBy { it.title.lowercase(Locale.ROOT) }
    }

    /** Groups scanned [tracks] into sorted [LocalFolderGroup] entries. */
    fun groupLocalFolders(tracks: List<LocalAudioTrack>): List<LocalFolderGroup> {
        if (tracks.isEmpty()) return emptyList()
        return tracks
            .groupBy { track ->
                val rel = track.relativePath.trim().trimEnd('/').lowercase(Locale.ROOT)
                rel.ifEmpty { track.folderName.trim().lowercase(Locale.ROOT) }
            }
            .values
            .map { group ->
                val sortedTracks = group.sortedBy { it.title.lowercase(Locale.ROOT) }
                val head = sortedTracks.first()
                LocalFolderGroup(
                    folderName = head.folderName.ifBlank { DEFAULT_FOLDER_NAME },
                    relativePath = head.relativePath.trim(),
                    tracks = sortedTracks,
                )
            }
            .sortedWith(
                compareBy<LocalFolderGroup>(
                    { it.folderName.lowercase(Locale.ROOT) },
                    { it.relativePath.lowercase(Locale.ROOT) },
                ),
            )
    }

    /** Builds a [LocalMusicLibrary] containing tracks, grouped albums, and grouped folders. */
    fun groupLocalMusicLibrary(
        tracks: List<LocalAudioTrack>,
        permissionGranted: Boolean = true,
        availableFolders: List<String> = emptyList(),
    ): LocalMusicLibrary {
        val groupedFolders = groupLocalFolders(tracks)
        val discoveredFolders = if (availableFolders.isNotEmpty()) {
            availableFolders
        } else {
            groupedFolders.map { folder ->
                normalizeFolderPath(folder.relativePath).ifEmpty { normalizeFolderPath(folder.folderName) }
            }.filter { it.isNotEmpty() }.distinctBy { it.lowercase(Locale.ROOT) }.sortedBy { it.lowercase(Locale.ROOT) }
        }
        return LocalMusicLibrary(
            tracks = tracks,
            albums = groupLocalAlbums(tracks),
            folders = groupedFolders,
            availableFolders = discoveredFolders,
            permissionGranted = permissionGranted,
        )
    }

    /**
     * Queries [MediaStore.Audio.Media.EXTERNAL_CONTENT_URI] on the calling thread and returns
     * a grouped [LocalMusicLibrary]. Returns an empty library cleanly if permission is denied
     * or if the query throws.
     */
    fun scan(
        context: Context,
        minDurationMs: Long = DEFAULT_MIN_DURATION_MS,
        excludeNonMusicAudio: Boolean = true,
        selectedFolders: Set<String> = emptySet(),
    ): LocalMusicLibrary {
        if (!hasAudioReadPermission(context)) {
            return LocalMusicLibrary(permissionGranted = false)
        }
        val resolver = runCatching { context.contentResolver }.getOrNull()
            ?: return LocalMusicLibrary(permissionGranted = false)

        val effectiveMinDurationMs = if (excludeNonMusicAudio) {
            minDurationMs.coerceAtLeast(DEFAULT_FILTERED_MIN_DURATION_MS)
        } else {
            minDurationMs.coerceAtLeast(DEFAULT_MIN_DURATION_MS)
        }

        val projection = arrayOf(
            MediaStore.Audio.Media._ID,
            MediaStore.Audio.Media.TITLE,
            MediaStore.Audio.Media.ARTIST,
            MediaStore.Audio.Media.ALBUM,
            MediaStore.Audio.Media.ALBUM_ID,
            MediaStore.Audio.Media.DURATION,
            MediaStore.Audio.Media.SIZE,
            MediaStore.Audio.Media.BITRATE,
            MediaStore.Audio.Media.RELATIVE_PATH,
            MediaStore.Audio.Media.BUCKET_DISPLAY_NAME,
            MediaStore.Audio.Media.MIME_TYPE,
            MediaStore.Audio.Media.TRACK,
            MediaStore.Audio.Media.DATE_MODIFIED,
        )
        val selection = "${MediaStore.Audio.Media.IS_MUSIC} != 0 AND ${MediaStore.Audio.Media.DURATION} >= ?"
        val selectionArgs = arrayOf(DEFAULT_MIN_DURATION_MS.toString())
        val sortOrder = "${MediaStore.Audio.Media.TITLE} COLLATE NOCASE ASC"

        val scanned = ArrayList<LocalAudioTrack>()
        val discoveredFoldersMap = LinkedHashMap<String, String>()
        val queryResult = runCatching {
            resolver.query(
                MediaStore.Audio.Media.EXTERNAL_CONTENT_URI,
                projection,
                selection,
                selectionArgs,
                sortOrder,
            )?.use { cursor ->
                val idCol = cursor.getColumnIndex(MediaStore.Audio.Media._ID)
                if (idCol < 0) return@use
                val titleCol = cursor.getColumnIndex(MediaStore.Audio.Media.TITLE)
                val artistCol = cursor.getColumnIndex(MediaStore.Audio.Media.ARTIST)
                val albumCol = cursor.getColumnIndex(MediaStore.Audio.Media.ALBUM)
                val albumIdCol = cursor.getColumnIndex(MediaStore.Audio.Media.ALBUM_ID)
                val durationCol = cursor.getColumnIndex(MediaStore.Audio.Media.DURATION)
                val sizeCol = cursor.getColumnIndex(MediaStore.Audio.Media.SIZE)
                val bitrateCol = cursor.getColumnIndex(MediaStore.Audio.Media.BITRATE)
                val relPathCol = cursor.getColumnIndex(MediaStore.Audio.Media.RELATIVE_PATH)
                val bucketCol = cursor.getColumnIndex(MediaStore.Audio.Media.BUCKET_DISPLAY_NAME)
                val mimeCol = cursor.getColumnIndex(MediaStore.Audio.Media.MIME_TYPE)
                val trackCol = cursor.getColumnIndex(MediaStore.Audio.Media.TRACK)
                val dateCol = cursor.getColumnIndex(MediaStore.Audio.Media.DATE_MODIFIED)

                while (cursor.moveToNext()) {
                    val mediaId = cursor.getLong(idCol)
                    if (mediaId <= 0L) continue
                    val rawTitle = if (titleCol >= 0) cursor.getString(titleCol).orEmpty().trim() else ""
                    val title = rawTitle.ifEmpty { "Track $mediaId" }
                    val rawArtist = if (artistCol >= 0) cursor.getString(artistCol).orEmpty().trim() else ""
                    val artist = if (rawArtist.isEmpty() || rawArtist == "<unknown>") "Unknown Artist" else rawArtist
                    val rawAlbum = if (albumCol >= 0) cursor.getString(albumCol).orEmpty().trim() else ""
                    val album = if (rawAlbum.isEmpty() || rawAlbum == "<unknown>") "Unknown Album" else rawAlbum
                    val albumId = if (albumIdCol >= 0) cursor.getLong(albumIdCol) else 0L
                    val durationMs = if (durationCol >= 0) cursor.getLong(durationCol).coerceAtLeast(0L) else 0L
                    val fileSizeBytes = if (sizeCol >= 0) cursor.getLong(sizeCol).coerceAtLeast(0L) else 0L
                    val reportedBitrateBps = if (bitrateCol >= 0) cursor.getInt(bitrateCol) else 0
                    val relativePath = if (relPathCol >= 0) cursor.getString(relPathCol).orEmpty().trim() else ""
                    val bucketDisplayName = if (bucketCol >= 0) cursor.getString(bucketCol) else null
                    val folderName = extractFolderName(bucketDisplayName, relativePath)
                    val mimeType = if (mimeCol >= 0) cursor.getString(mimeCol).orEmpty().trim() else ""
                    val trackNum = if (trackCol >= 0) (cursor.getInt(trackCol) % 1000).coerceAtLeast(0) else 0
                    val dateModifiedSecs = if (dateCol >= 0) cursor.getLong(dateCol).coerceAtLeast(0L) else 0L

                    if (excludeNonMusicAudio &&
                        isExcludedNonMusicAudio(
                            title = title,
                            relativePath = relativePath,
                            folderName = folderName,
                            durationMs = durationMs,
                            minDurationMs = effectiveMinDurationMs,
                        )
                    ) {
                        continue
                    }

                    val normalizedFolder = normalizeFolderPath(relativePath)
                        .ifEmpty { normalizeFolderPath(folderName) }
                    if (normalizedFolder.isNotEmpty()) {
                        discoveredFoldersMap.putIfAbsent(normalizedFolder.lowercase(Locale.ROOT), normalizedFolder)
                    }

                    if (!matchesSelectedFolders(
                            relativePath = relativePath,
                            folderName = folderName,
                            selectedFolders = selectedFolders,
                        )
                    ) {
                        continue
                    }

                    val contentUri = ContentUris.withAppendedId(
                        MediaStore.Audio.Media.EXTERNAL_CONTENT_URI,
                        mediaId,
                    ).toString()

                    scanned.add(
                        LocalAudioTrack(
                            mediaId = mediaId,
                            id = buildLocalTrackId(mediaId),
                            title = title,
                            artist = artist,
                            album = album,
                            albumId = albumId,
                            contentUri = contentUri,
                            albumArtUri = buildLocalAlbumArtUri(albumId),
                            durationMs = durationMs,
                            fileSizeBytes = fileSizeBytes,
                            bitrateKbps = resolveLocalBitrateKbps(
                                mediaStoreBitrateBps = reportedBitrateBps,
                                fileSizeBytes = fileSizeBytes,
                                durationMs = durationMs,
                            ),
                            folderName = folderName,
                            relativePath = relativePath,
                            mimeType = mimeType,
                            trackNumber = trackNum,
                            dateModifiedSecs = dateModifiedSecs,
                        ),
                    )
                }
            }
        }

        if (queryResult.isFailure) {
            val isSecurity = queryResult.exceptionOrNull() is SecurityException
            return LocalMusicLibrary(permissionGranted = !isSecurity)
        }
        val availableFolders = discoveredFoldersMap.values.sortedBy { it.lowercase(Locale.ROOT) }
        return groupLocalMusicLibrary(
            tracks = scanned,
            permissionGranted = true,
            availableFolders = availableFolders,
        )
    }

    /** Offloads [scan] onto [Dispatchers.IO]. */
    suspend fun scanAsync(
        context: Context,
        minDurationMs: Long = DEFAULT_MIN_DURATION_MS,
        excludeNonMusicAudio: Boolean = true,
        selectedFolders: Set<String> = emptySet(),
    ): LocalMusicLibrary = withContext(Dispatchers.IO) {
        scan(
            context = context,
            minDurationMs = minDurationMs,
            excludeNonMusicAudio = excludeNonMusicAudio,
            selectedFolders = selectedFolders,
        )
    }
}
