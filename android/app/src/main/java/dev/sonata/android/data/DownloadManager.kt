package dev.sonata.android.data

import android.content.ContentValues
import android.content.Context
import android.content.SharedPreferences
import android.net.ConnectivityManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.StatFs
import android.os.SystemClock
import android.provider.MediaStore
import androidx.compose.runtime.Immutable
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.sonata_android.AndroidMusicVideoSyncSegment
import java.io.BufferedOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.io.OutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

@Immutable
data class DownloadedTrackInput(
    val videoId: String,
    val title: String,
    val artist: String,
    val album: String? = null,
    val durationSecs: Long = 0L,
    val coverUrl: String? = null,
    val codec: String? = null,
)

@Immutable
data class OfflineVideoMedia(
    val videoUri: String,
    val audioUri: String?,
    val qualityLabel: String,
    val qualityHeight: Int,
    val syncSegments: List<AndroidMusicVideoSyncSegment> = emptyList(),
)

@Immutable
data class DownloadedTrackRecord(
    val videoId: String,
    val title: String,
    val artist: String,
    val album: String?,
    val durationSecs: Long,
    val coverUrl: String?,
    val localUri: String,
    val fileSizeBytes: Long,
    val downloadedAtMs: Long,
    val codec: String?,
    val collectionId: String? = null,
    val localVideoUri: String? = null,
    val localVideoAudioUri: String? = null,
    val videoQualityLabel: String? = null,
    val videoQualityHeight: Int = 0,
    val videoFileSizeBytes: Long = 0L,
    val videoSyncSegments: List<AndroidMusicVideoSyncSegment> = emptyList(),
) {
    val totalSizeBytes: Long
        get() = fileSizeBytes + videoFileSizeBytes

    val hasOfflineVideo: Boolean
        get() = !localVideoUri.isNullOrBlank()
}

@Immutable
data class ActiveDownload(
    val videoId: String,
    val title: String,
    val artist: String,
    val coverUrl: String?,
    val progress: Float,
    val status: String,
)

/**
 * Manages permanent offline track downloads for Sonata Android.
 *
 * Downloads progressive streams in bounded 1 MiB HTTP Range chunks with client-matched
 * googlevideo headers, writes atomically via `.part` files (private storage) or MediaStore
 * `IS_PENDING` rows (`Music/Sonata` on API 29+), and persists an index in SharedPreferences.
 */
class SonataDownloadManager(private val context: Context) {

    private val appContext = context.applicationContext
    private val prefs: SharedPreferences by lazy {
        appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }
    private val indexLock = Any()
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    @Volatile
    private var recordsLoaded = false
    private val cancelFlags = ConcurrentHashMap<String, AtomicBoolean>()

    private val _downloads = MutableStateFlow<List<DownloadedTrackRecord>>(emptyList())
    val downloads: StateFlow<List<DownloadedTrackRecord>> = _downloads.asStateFlow()

    private val _activeDownloads = MutableStateFlow<Map<String, ActiveDownload>>(emptyMap())
    val activeDownloads: StateFlow<Map<String, ActiveDownload>> = _activeDownloads.asStateFlow()

    private val _totalBytes = MutableStateFlow(0L)
    val totalBytes: StateFlow<Long> = _totalBytes.asStateFlow()

    init {
        ioScope.launch {
            ensureLoaded()
        }
    }

    private fun ensureLoaded() {
        if (recordsLoaded) return
        synchronized(indexLock) {
            ensureLoadedLocked()
        }
    }

    private fun ensureLoadedLocked() {
        if (recordsLoaded) return
        val loaded = loadPersistedRecords()
        _downloads.value = loaded
        _totalBytes.value = loaded.sumOf { it.totalSizeBytes }
        recordsLoaded = true
    }

    /** Indicates a download was blocked because Wi-Fi only mode is enabled on a metered network. */
    class WifiOnlyNetworkException : IOException("Wi-Fi only downloads enabled")

    fun isOnMeteredNetwork(): Boolean {
        val cm = appContext.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
            ?: return false
        return cm.isActiveNetworkMetered
    }

    fun isDownloaded(videoId: String): Boolean = localUriFor(videoId) != null

    fun localUriFor(videoId: String): String? {
        ensureLoaded()
        val record = _downloads.value.firstOrNull { it.videoId == videoId } ?: return null
        if (!uriExists(record.localUri)) {
            pruneMissingRecord(videoId)
            return null
        }
        return record.localUri
    }

    fun localVideoFor(videoId: String): OfflineVideoMedia? {
        ensureLoaded()
        val record = _downloads.value.firstOrNull { it.videoId == videoId } ?: return null
        val videoUri = record.localVideoUri?.takeIf { it.isNotBlank() } ?: return null
        val audioUri = record.localVideoAudioUri?.takeIf { it.isNotBlank() }
        val videoOk = uriExists(videoUri)
        val audioOk = audioUri == null || uriExists(audioUri)
        if (!videoOk || !audioOk) {
            pruneMissingVideo(videoId)
            return null
        }
        return OfflineVideoMedia(
            videoUri = videoUri,
            audioUri = audioUri,
            qualityLabel = record.videoQualityLabel?.takeIf { it.isNotBlank() }
                ?: if (record.videoQualityHeight > 0) "${record.videoQualityHeight}p" else "Video",
            qualityHeight = record.videoQualityHeight,
            syncSegments = record.videoSyncSegments,
        )
    }

    fun cancelDownload(videoId: String) {
        cancelFlags[videoId]?.set(true)
        _activeDownloads.update { it - videoId }
    }

    fun removeDownload(videoId: String) {
        cancelDownload(videoId)
        val removed = synchronized(indexLock) {
            ensureLoadedLocked()
            val current = _downloads.value
            val target = current.firstOrNull { it.videoId == videoId } ?: return
            val updated = current.filterNot { it.videoId == videoId }
            persistAndPublishLocked(updated)
            target
        }
        deleteBackedMedia(removed.localUri)
        removed.localVideoUri?.let(::deleteBackedMedia)
        removed.localVideoAudioUri?.let(::deleteBackedMedia)
    }

    fun clearAllDownloads() {
        cancelFlags.values.forEach { it.set(true) }
        cancelFlags.clear()
        _activeDownloads.value = emptyMap()
        val previous = synchronized(indexLock) {
            ensureLoadedLocked()
            val snapshot = _downloads.value
            persistAndPublishLocked(emptyList())
            snapshot
        }
        previous.forEach {
            deleteBackedMedia(it.localUri)
            it.localVideoUri?.let(::deleteBackedMedia)
            it.localVideoAudioUri?.let(::deleteBackedMedia)
        }
        privateVideoDownloadsDir().listFiles()?.forEach { file ->
            runCatching { file.delete() }
        }
        privateDownloadsDir().listFiles()?.forEach { file ->
            if (file.isFile) runCatching { file.delete() }
        }
    }

    suspend fun downloadTrack(
        track: DownloadedTrackInput,
        streamUrl: String,
        exportToMusic: Boolean,
        collectionId: String? = null,
        wifiOnly: Boolean = false,
        headers: Map<String, String> = emptyMap(),
    ): Result<DownloadedTrackRecord> = withContext(Dispatchers.IO) {
        ensureLoaded()
        val isSmbStream = streamUrl.startsWith("${SmbClient.STREAM_SCHEME}://", ignoreCase = true)
        val isLocalFile = !isSmbStream && !streamUrl.startsWith("http", ignoreCase = true)
        if (wifiOnly && !isLocalFile && isOnMeteredNetwork()) {
            return@withContext Result.failure(WifiOnlyNetworkException())
        }
        val resolvedCoverUrl = resolveCoverUrl(track.videoId, track.coverUrl)
        val effectiveTrack = if (resolvedCoverUrl == track.coverUrl) {
            track
        } else {
            track.copy(coverUrl = resolvedCoverUrl)
        }
        val cancelFlag = AtomicBoolean(false)
        cancelFlags[effectiveTrack.videoId] = cancelFlag

        var lastEmittedProgress = -1f
        var lastEmitAtMs = 0L
        var lastStatus = ""

        fun publishProgress(progress: Float, status: String) {
            val clamped = progress.coerceIn(0f, 1f)
            val now = SystemClock.elapsedRealtime()
            val isEdge = clamped <= 0f || clamped >= 1f || status != lastStatus
            val deltaOk = kotlin.math.abs(clamped - lastEmittedProgress) >= PROGRESS_MIN_DELTA ||
                (now - lastEmitAtMs) >= PROGRESS_MIN_INTERVAL_MS
            if (!isEdge && !deltaOk) return
            lastEmittedProgress = clamped
            lastEmitAtMs = now
            lastStatus = status
            _activeDownloads.update { current ->
                current + (
                    effectiveTrack.videoId to ActiveDownload(
                        videoId = effectiveTrack.videoId,
                        title = effectiveTrack.title,
                        artist = effectiveTrack.artist,
                        coverUrl = effectiveTrack.coverUrl,
                        progress = clamped,
                        status = status,
                    )
                )
            }
        }

        publishProgress(0f, "Starting")

        try {
            val parsedUri = Uri.parse(streamUrl)
            val mimeType = inferMimeType(parsedUri, effectiveTrack.codec)
            val extension = extensionForMime(mimeType)
            val totalLength = when {
                isLocalFile -> resolveContainedLocalFile(streamUrl).takeIf { it.isFile }?.length()
                isSmbStream -> null
                else -> parsedUri.getQueryParameter("clen")?.toLongOrNull()
                    ?: probeContentLength(streamUrl, parsedUri, headers)
            }

            val resultRecord = if (exportToMusic && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                downloadToMediaStore(
                    track = effectiveTrack,
                    streamUrl = streamUrl,
                    parsedUri = parsedUri,
                    mimeType = mimeType,
                    extension = extension,
                    knownTotal = totalLength,
                    collectionId = collectionId,
                    headers = headers,
                    cancelFlag = cancelFlag,
                    onProgress = ::publishProgress,
                )
            } else {
                downloadToPrivateStorage(
                    track = effectiveTrack,
                    streamUrl = streamUrl,
                    parsedUri = parsedUri,
                    extension = extension,
                    knownTotal = totalLength,
                    collectionId = collectionId,
                    headers = headers,
                    cancelFlag = cancelFlag,
                    onProgress = ::publishProgress,
                )
            }

            synchronized(indexLock) {
                val existing = _downloads.value.firstOrNull { it.videoId == effectiveTrack.videoId }
                if (existing != null && existing.localUri != resultRecord.localUri) {
                    deleteBackedMedia(existing.localUri)
                }
                val updated = listOf(resultRecord) + _downloads.value.filterNot { it.videoId == effectiveTrack.videoId }
                persistAndPublishLocked(updated)
            }

            Result.success(resultRecord)
        } catch (ce: CancellationException) {
            throw ce
        } catch (t: Throwable) {
            Result.failure(t)
        } finally {
            cancelFlags.remove(effectiveTrack.videoId)
            _activeDownloads.update { it - effectiveTrack.videoId }
        }
    }

    suspend fun downloadCompanionVideo(
        track: DownloadedTrackInput,
        videoUrl: String,
        videoMimeHint: String?,
        audioUrl: String?,
        qualityLabel: String,
        qualityHeight: Int,
        syncSegments: List<AndroidMusicVideoSyncSegment> = emptyList(),
        wifiOnly: Boolean = false,
    ): Result<OfflineVideoMedia> = withContext(Dispatchers.IO) {
        ensureLoaded()
        if (wifiOnly && isOnMeteredNetwork()) {
            return@withContext Result.failure(WifiOnlyNetworkException())
        }
        val cancelFlag = AtomicBoolean(false)
        cancelFlags[track.videoId] = cancelFlag

        var lastEmittedProgress = -1f
        var lastEmitAtMs = 0L
        var lastStatus = ""

        fun publishProgress(progress: Float, status: String) {
            val clamped = progress.coerceIn(0f, 1f)
            val now = SystemClock.elapsedRealtime()
            val isEdge = clamped <= 0f || clamped >= 1f || status != lastStatus
            val deltaOk = kotlin.math.abs(clamped - lastEmittedProgress) >= PROGRESS_MIN_DELTA ||
                (now - lastEmitAtMs) >= PROGRESS_MIN_INTERVAL_MS
            if (!isEdge && !deltaOk) return
            lastEmittedProgress = clamped
            lastEmitAtMs = now
            lastStatus = status
            _activeDownloads.update { current ->
                current + (
                    track.videoId to ActiveDownload(
                        videoId = track.videoId,
                        title = track.title,
                        artist = track.artist,
                        coverUrl = resolveCoverUrl(track.videoId, track.coverUrl),
                        progress = clamped,
                        status = status,
                    )
                )
            }
        }

        val dir = privateVideoDownloadsDir()
        val safeId = sanitizeId(track.videoId)
        val parsedVideoUri = Uri.parse(videoUrl)
        val videoMime = inferVideoMimeType(parsedVideoUri, videoMimeHint)
        val videoExt = extensionForVideoMime(videoMime)
        val finalVideoFile = File(dir, "${safeId}_video.$videoExt")
        val partVideoFile = File(dir, "${safeId}_video.$videoExt.part")

        var finalVideoAudioFile: File? = null
        var partVideoAudioFile: File? = null

        publishProgress(0f, "Downloading video")

        try {
            if (partVideoFile.exists()) partVideoFile.delete()
            val videoTotal = parsedVideoUri.getQueryParameter("clen")?.toLongOrNull()
                ?: probeContentLength(videoUrl, parsedVideoUri)
            val videoWritten = BufferedOutputStream(FileOutputStream(partVideoFile), IO_BUFFER_BYTES).use { out ->
                streamIntoOutput(
                    streamUrl = videoUrl,
                    parsedUri = parsedVideoUri,
                    knownTotal = videoTotal,
                    output = out,
                    cancelFlag = cancelFlag,
                    onProgress = { p, _ ->
                        val scaled = if (!audioUrl.isNullOrBlank()) p * 0.85f else p
                        publishProgress(scaled, "Downloading video")
                    },
                )
            }
            if (videoWritten <= 0L) throw IOException("Downloaded video file is empty")
            if (finalVideoFile.exists()) finalVideoFile.delete()
            if (!partVideoFile.renameTo(finalVideoFile)) {
                partVideoFile.copyTo(finalVideoFile, overwrite = true)
                partVideoFile.delete()
            }

            var audioWritten = 0L
            var localAudioUriString: String? = null
            if (!audioUrl.isNullOrBlank()) {
                val parsedAudioUri = Uri.parse(audioUrl)
                val audioMime = inferMimeType(parsedAudioUri, null)
                val audioExt = extensionForMime(audioMime)
                val targetAudioFile = File(dir, "${safeId}_vaudio.$audioExt")
                val tempAudioFile = File(dir, "${safeId}_vaudio.$audioExt.part")
                finalVideoAudioFile = targetAudioFile
                partVideoAudioFile = tempAudioFile
                if (tempAudioFile.exists()) tempAudioFile.delete()
                val audioTotal = parsedAudioUri.getQueryParameter("clen")?.toLongOrNull()
                    ?: probeContentLength(audioUrl, parsedAudioUri)
                audioWritten = BufferedOutputStream(FileOutputStream(tempAudioFile), IO_BUFFER_BYTES).use { out ->
                    streamIntoOutput(
                        streamUrl = audioUrl,
                        parsedUri = parsedAudioUri,
                        knownTotal = audioTotal,
                        output = out,
                        cancelFlag = cancelFlag,
                        onProgress = { p, _ ->
                            publishProgress(0.85f + p * 0.15f, "Downloading video")
                        },
                    )
                }
                if (audioWritten <= 0L) throw IOException("Downloaded companion audio file is empty")
                if (targetAudioFile.exists()) targetAudioFile.delete()
                if (!tempAudioFile.renameTo(targetAudioFile)) {
                    tempAudioFile.copyTo(targetAudioFile, overwrite = true)
                    tempAudioFile.delete()
                }
                localAudioUriString = Uri.fromFile(targetAudioFile).toString()
            }

            val localVideoUriString = Uri.fromFile(finalVideoFile).toString()
            val totalVideoBytes = (finalVideoFile.length().takeIf { it > 0L } ?: videoWritten) +
                (finalVideoAudioFile?.length()?.takeIf { it > 0L } ?: audioWritten)

            var supersededVideoUri: String? = null
            var supersededAudioUri: String? = null
            synchronized(indexLock) {
                ensureLoadedLocked()
                val current = _downloads.value
                val existing = current.firstOrNull { it.videoId == track.videoId }
                    ?: throw IOException("Audio download record missing for ${track.videoId}")
                if (existing.localVideoUri != null && existing.localVideoUri != localVideoUriString) {
                    supersededVideoUri = existing.localVideoUri
                }
                if (existing.localVideoAudioUri != null && existing.localVideoAudioUri != localAudioUriString) {
                    supersededAudioUri = existing.localVideoAudioUri
                }
                val updatedRecord = existing.copy(
                    localVideoUri = localVideoUriString,
                    localVideoAudioUri = localAudioUriString,
                    videoQualityLabel = qualityLabel.ifBlank { "${qualityHeight}p" },
                    videoQualityHeight = qualityHeight,
                    videoFileSizeBytes = totalVideoBytes,
                    videoSyncSegments = syncSegments,
                )
                val updatedList = current.map { if (it.videoId == track.videoId) updatedRecord else it }
                persistAndPublishLocked(updatedList)
            }
            supersededVideoUri?.let(::deleteBackedMedia)
            supersededAudioUri?.let(::deleteBackedMedia)

            Result.success(
                OfflineVideoMedia(
                    videoUri = localVideoUriString,
                    audioUri = localAudioUriString,
                    qualityLabel = qualityLabel.ifBlank { "${qualityHeight}p" },
                    qualityHeight = qualityHeight,
                    syncSegments = syncSegments,
                ),
            )
        } catch (ce: CancellationException) {
            runCatching { partVideoFile.delete() }
            runCatching { finalVideoFile.delete() }
            runCatching { partVideoAudioFile?.delete() }
            runCatching { finalVideoAudioFile?.delete() }
            throw ce
        } catch (t: Throwable) {
            runCatching { partVideoFile.delete() }
            runCatching { finalVideoFile.delete() }
            runCatching { partVideoAudioFile?.delete() }
            runCatching { finalVideoAudioFile?.delete() }
            Result.failure(t)
        } finally {
            cancelFlags.remove(track.videoId)
            _activeDownloads.update { it - track.videoId }
        }
    }

    private suspend fun downloadToPrivateStorage(
        track: DownloadedTrackInput,
        streamUrl: String,
        parsedUri: Uri,
        extension: String,
        knownTotal: Long?,
        collectionId: String?,
        headers: Map<String, String> = emptyMap(),
        cancelFlag: AtomicBoolean,
        onProgress: (Float, String) -> Unit,
    ): DownloadedTrackRecord {
        val dir = privateDownloadsDir()
        val safeId = sanitizeId(track.videoId)
        val finalFile = File(dir, "$safeId.$extension")
        val partFile = File(dir, "$safeId.$extension.part")
        if (partFile.exists()) partFile.delete()

        try {
            val written = BufferedOutputStream(FileOutputStream(partFile), IO_BUFFER_BYTES).use { out ->
                streamIntoOutput(
                    streamUrl = streamUrl,
                    parsedUri = parsedUri,
                    knownTotal = knownTotal,
                    output = out,
                    headers = headers,
                    cancelFlag = cancelFlag,
                    onProgress = onProgress,
                )
            }
            if (written <= 0L) throw IOException("Downloaded file is empty")
            if (finalFile.exists()) finalFile.delete()
            if (!partFile.renameTo(finalFile)) {
                partFile.copyTo(finalFile, overwrite = true)
                partFile.delete()
            }
            return DownloadedTrackRecord(
                videoId = track.videoId,
                title = track.title,
                artist = track.artist,
                album = track.album,
                durationSecs = track.durationSecs,
                coverUrl = track.coverUrl,
                localUri = Uri.fromFile(finalFile).toString(),
                fileSizeBytes = finalFile.length().takeIf { it > 0L } ?: written,
                downloadedAtMs = System.currentTimeMillis(),
                codec = track.codec ?: extension.uppercase(Locale.ROOT),
                collectionId = collectionId,
            )
        } catch (t: Throwable) {
            if (partFile.exists()) partFile.delete()
            throw t
        }
    }

    private suspend fun downloadToMediaStore(
        track: DownloadedTrackInput,
        streamUrl: String,
        parsedUri: Uri,
        mimeType: String,
        extension: String,
        knownTotal: Long?,
        collectionId: String?,
        headers: Map<String, String> = emptyMap(),
        cancelFlag: AtomicBoolean,
        onProgress: (Float, String) -> Unit,
    ): DownloadedTrackRecord {
        val resolver = appContext.contentResolver
        val displayName = buildDisplayName(track.artist, track.title, track.videoId, extension)
        val values = ContentValues().apply {
            put(MediaStore.Audio.Media.DISPLAY_NAME, displayName)
            put(MediaStore.Audio.Media.MIME_TYPE, mimeType)
            put(MediaStore.Audio.Media.TITLE, track.title)
            put(MediaStore.Audio.Media.ARTIST, track.artist)
            if (!track.album.isNullOrBlank()) {
                put(MediaStore.Audio.Media.ALBUM, track.album)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                put(MediaStore.Audio.Media.RELATIVE_PATH, "${Environment.DIRECTORY_MUSIC}/$EXPORT_FOLDER")
                put(MediaStore.Audio.Media.IS_PENDING, 1)
            }
        }

        val itemUri = resolver.insert(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, values)
            ?: throw IOException("Could not create MediaStore entry")

        try {
            val written = resolver.openOutputStream(itemUri, "w")?.let { rawOut ->
                BufferedOutputStream(rawOut, IO_BUFFER_BYTES).use { out ->
                    streamIntoOutput(
                        streamUrl = streamUrl,
                        parsedUri = parsedUri,
                        knownTotal = knownTotal,
                        output = out,
                        headers = headers,
                        cancelFlag = cancelFlag,
                        onProgress = onProgress,
                    )
                }
            } ?: throw IOException("Could not open MediaStore output stream")

            if (written <= 0L) throw IOException("Downloaded audio is empty")

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val finalizeValues = ContentValues().apply {
                    put(MediaStore.Audio.Media.IS_PENDING, 0)
                }
                resolver.update(itemUri, finalizeValues, null, null)
            }

            return DownloadedTrackRecord(
                videoId = track.videoId,
                title = track.title,
                artist = track.artist,
                album = track.album,
                durationSecs = track.durationSecs,
                coverUrl = track.coverUrl,
                localUri = itemUri.toString(),
                fileSizeBytes = written,
                downloadedAtMs = System.currentTimeMillis(),
                codec = track.codec ?: extension.uppercase(Locale.ROOT),
                collectionId = collectionId,
            )
        } catch (t: Throwable) {
            runCatching { resolver.delete(itemUri, null, null) }
            throw t
        }
    }

    private suspend fun streamIntoOutput(
        streamUrl: String,
        parsedUri: Uri,
        knownTotal: Long?,
        output: OutputStream,
        headers: Map<String, String> = emptyMap(),
        cancelFlag: AtomicBoolean,
        onProgress: (Float, String) -> Unit,
    ): Long {
        val isSmbStream = streamUrl.startsWith("${SmbClient.STREAM_SCHEME}://", ignoreCase = true)
        val isLocalFile = !isSmbStream && !streamUrl.startsWith("http", ignoreCase = true)
        val buffer = ByteArray(IO_BUFFER_BYTES)
        if (isSmbStream) {
            val (serverId, relPath) = SmbClient.parseStreamUri(parsedUri)
                ?: throw IOException("Invalid SMB stream URI: $streamUrl")
            val profile = SmbProfileRegistry.get(serverId)
                ?: throw IOException("SMB server '$serverId' is not configured")
            var written = 0L
            SmbClient.openReadSession(profile, relPath).use { handle ->
                val total = handle.length.takeIf { it > 0L }
                while (total == null || written < total) {
                    checkCancelled(cancelFlag)
                    val read = handle.file.read(buffer, written, 0, buffer.size)
                    if (read <= 0) break
                    output.write(buffer, 0, read)
                    written += read
                    val fraction = if (total != null) written.toFloat() / total.toFloat() else 0f
                    onProgress(fraction, "Downloading")
                }
                if (total != null && written < total) {
                    throw IOException("Incomplete SMB read: $written of $total bytes")
                }
            }
            output.flush()
            return written
        }
        if (isLocalFile) {
            val sourceFile = resolveContainedLocalFile(streamUrl)
            if (!sourceFile.isFile) throw IOException("Cached audio source missing")
            val total = sourceFile.length().coerceAtLeast(1L)
            var written = 0L
            sourceFile.inputStream().use { input ->
                while (true) {
                    checkCancelled(cancelFlag)
                    val read = input.read(buffer)
                    if (read <= 0) break
                    output.write(buffer, 0, read)
                    written += read
                    onProgress(written.toFloat() / total.toFloat(), "Saving")
                }
            }
            output.flush()
            return written
        }

        val isGooglevideo = parsedUri.host?.lowercase(Locale.ROOT)?.endsWith("googlevideo.com") == true

        if (isGooglevideo && knownTotal != null && knownTotal > 0L) {
            val client = parsedUri.getQueryParameter("c")?.uppercase(Locale.ROOT).orEmpty()
            val chunkSize = if (client == "ANDROID_VR" || client.startsWith("TVHTML5_SIMPLY")) {
                NARROW_CHUNK_BYTES
            } else {
                CHUNK_BYTES
            }
            return streamChunkedHttpRanges(
                streamUrl = streamUrl,
                parsedUri = parsedUri,
                knownTotal = knownTotal,
                chunkSize = chunkSize,
                output = output,
                headers = headers,
                buffer = buffer,
                cancelFlag = cancelFlag,
                onProgress = onProgress,
            )
        }

        // Direct single-request stream for non-googlevideo or unknown-length URLs.
        val conn = openConnection(streamUrl, parsedUri, rangeHeader = null, extraHeaders = headers)
        return try {
            val code = conn.responseCode
            if (code !in 200..299) {
                throw IOException("Download failed with HTTP $code")
            }
            val total = conn.contentLengthLong.takeIf { it > 0L } ?: knownTotal
            var written = 0L
            conn.inputStream.use { input ->
                while (true) {
                    checkCancelled(cancelFlag)
                    val read = input.read(buffer)
                    if (read <= 0) break
                    output.write(buffer, 0, read)
                    written += read
                    val fraction = if (total != null && total > 0L) {
                        written.toFloat() / total.toFloat()
                    } else {
                        0f
                    }
                    onProgress(fraction, "Downloading")
                }
            }
            output.flush()
            written
        } finally {
            conn.disconnect()
        }
    }

    internal suspend fun streamChunkedHttpRanges(
        streamUrl: String,
        parsedUri: Uri?,
        knownTotal: Long,
        chunkSize: Long,
        output: OutputStream,
        headers: Map<String, String>,
        buffer: ByteArray = ByteArray(IO_BUFFER_BYTES),
        cancelFlag: AtomicBoolean,
        onProgress: (Float, String) -> Unit,
    ): Long {
        var position = 0L
        while (position < knownTotal) {
            checkCancelled(cancelFlag)
            val endInclusive = minOf(knownTotal - 1L, position + chunkSize - 1L)
            var attempt = 0
            while (true) {
                checkCancelled(cancelFlag)
                val conn = openConnection(
                    streamUrl = streamUrl,
                    parsedUri = parsedUri,
                    rangeHeader = "bytes=$position-$endInclusive",
                    extraHeaders = headers,
                )
                try {
                    val code = conn.responseCode
                    if (code !in 200..299) {
                        throw IOException("HTTP $code while fetching range $position-$endInclusive")
                    }
                    var bytesInChunk = 0L
                    conn.inputStream.use { input ->
                        while (true) {
                            checkCancelled(cancelFlag)
                            val read = input.read(buffer)
                            if (read <= 0) break
                            output.write(buffer, 0, read)
                            bytesInChunk += read
                            position += read
                            onProgress(position.toFloat() / knownTotal.toFloat(), "Downloading")
                        }
                    }
                    if (bytesInChunk <= 0L) {
                        throw IOException("Unexpected 0-byte EOF while fetching range $position-$endInclusive")
                    }
                    break
                } catch (ce: CancellationException) {
                    throw ce
                } catch (e: IOException) {
                    if (attempt >= 2) throw e
                    attempt++
                    kotlinx.coroutines.delay(attempt * 250L)
                } finally {
                    conn.disconnect()
                }
            }
        }
        output.flush()
        return position
    }

    private suspend fun checkCancelled(cancelFlag: AtomicBoolean) {
        currentCoroutineContext().ensureActive()
        if (cancelFlag.get()) {
            throw CancellationException("Download cancelled")
        }
    }

    private fun probeContentLength(
        streamUrl: String,
        parsedUri: Uri,
        extraHeaders: Map<String, String> = emptyMap(),
    ): Long? {
        val conn = openConnection(streamUrl, parsedUri, rangeHeader = "bytes=0-0", extraHeaders = extraHeaders)
        return try {
            val code = conn.responseCode
            if (code == 206) {
                val contentRange = conn.getHeaderField("Content-Range")
                contentRange?.substringAfterLast('/')?.toLongOrNull()
            } else if (code in 200..299) {
                conn.contentLengthLong.takeIf { it > 0L }
            } else {
                null
            }
        } catch (_: Throwable) {
            null
        } finally {
            conn.disconnect()
        }
    }

    private fun openConnection(
        streamUrl: String,
        parsedUri: Uri?,
        rangeHeader: String?,
        extraHeaders: Map<String, String> = emptyMap(),
    ): HttpURLConnection {
        val conn = (URL(streamUrl).openConnection() as HttpURLConnection).apply {
            connectTimeout = CONNECT_TIMEOUT_MS
            readTimeout = READ_TIMEOUT_MS
            instanceFollowRedirects = true
            requestMethod = "GET"
        }
        val headers = parsedUri?.let(::headersForUri) ?: mapOf("User-Agent" to DEFAULT_MOBILE_UA)
        headers.forEach { (key, value) -> conn.setRequestProperty(key, value) }
        extraHeaders.forEach { (key, value) ->
            if (key.isNotBlank() && value.isNotBlank()) {
                conn.setRequestProperty(key, value)
            }
        }
        if (rangeHeader != null) {
            conn.setRequestProperty("Range", rangeHeader)
        }
        return conn
    }

    private fun headersForUri(uri: Uri): Map<String, String> {
        val host = uri.host?.lowercase(Locale.ROOT).orEmpty()
        if (!host.endsWith("googlevideo.com")) {
            return mapOf("User-Agent" to DEFAULT_MOBILE_UA)
        }
        val client = uri.getQueryParameter("c")?.uppercase(Locale.ROOT).orEmpty()
        val cver = uri.getQueryParameter("cver")
        return when {
            client == "VISIONOS" -> mapOf("User-Agent" to VISIONOS_UA)
            client.startsWith("IOS") -> mapOf(
                "User-Agent" to "com.google.ios.youtube/${cver ?: "20.10.4"} (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)"
            )
            client == "ANDROID_VR" -> mapOf(
                "User-Agent" to "com.google.android.apps.youtube.vr.oculus/${cver ?: "1.65.10"} (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip"
            )
            client == "ANDROID_MUSIC" -> mapOf(
                "User-Agent" to "com.google.android.apps.youtube.music/${cver ?: "8.39.42"} (Linux; U; Android 15; en_US; Pixel 9 Pro; Build/AP4A.250205.002) gzip"
            )
            client.startsWith("ANDROID") -> mapOf(
                "User-Agent" to "com.google.android.youtube/${cver ?: "21.26.364"} (Linux; U; Android 15; en_US; Pixel 9 Pro; Build/AP4A.250205.002; Cronet/132.0.6834.79) gzip"
            )
            client == "WEB_REMIX" -> mapOf(
                "User-Agent" to WEB_UA,
                "Origin" to "https://music.youtube.com",
                "Referer" to "https://music.youtube.com/",
            )
            client.startsWith("TVHTML5") -> mapOf(
                "User-Agent" to TVHTML5_UA,
                "Origin" to "https://www.youtube.com",
                "Referer" to "https://www.youtube.com/",
            )
            client.startsWith("WEB") || client == "MWEB" -> mapOf(
                "User-Agent" to WEB_UA,
                "Origin" to "https://www.youtube.com",
                "Referer" to "https://www.youtube.com/",
            )
            else -> mapOf(
                "User-Agent" to "com.google.ios.youtube/${cver ?: "20.10.4"} (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X)"
            )
        }
    }

    private fun uriExists(uriString: String): Boolean {
        val uri = Uri.parse(uriString)
        return when (uri.scheme) {
            "file" -> uri.path?.let { File(it).isFile } == true
            "content" -> runCatching {
                appContext.contentResolver.openAssetFileDescriptor(uri, "r")?.use { true } ?: false
            }.getOrDefault(false)
            else -> File(uriString).isFile
        }
    }

    private fun deleteBackedMedia(uriString: String) {
        val uri = Uri.parse(uriString)
        runCatching {
            when (uri.scheme) {
                "file" -> uri.path?.let { File(it).delete() }
                "content" -> appContext.contentResolver.delete(uri, null, null)
                else -> File(uriString).delete()
            }
        }
    }

    private fun pruneMissingRecord(videoId: String) {
        synchronized(indexLock) {
            val target = _downloads.value.firstOrNull { it.videoId == videoId }
            val updated = _downloads.value.filterNot { it.videoId == videoId }
            if (updated.size != _downloads.value.size) {
                persistAndPublishLocked(updated)
                target?.localVideoUri?.let(::deleteBackedMedia)
                target?.localVideoAudioUri?.let(::deleteBackedMedia)
            }
        }
    }

    private fun pruneMissingVideo(videoId: String) {
        var clearedVideo: String? = null
        var clearedAudio: String? = null
        synchronized(indexLock) {
            val current = _downloads.value
            val existing = current.firstOrNull { it.videoId == videoId } ?: return
            clearedVideo = existing.localVideoUri
            clearedAudio = existing.localVideoAudioUri
            val updated = current.map { rec ->
                if (rec.videoId == videoId) {
                    rec.copy(
                        localVideoUri = null,
                        localVideoAudioUri = null,
                        videoQualityLabel = null,
                        videoQualityHeight = 0,
                        videoFileSizeBytes = 0L,
                        videoSyncSegments = emptyList(),
                    )
                } else {
                    rec
                }
            }
            persistAndPublishLocked(updated)
        }
        clearedVideo?.let(::deleteBackedMedia)
        clearedAudio?.let(::deleteBackedMedia)
    }

    private fun privateDownloadsDir(): File =
        File(appContext.filesDir, "downloads").apply { mkdirs() }

    private fun privateVideoDownloadsDir(): File =
        File(privateDownloadsDir(), "videos").apply { mkdirs() }

    private fun allowedLocalSourceRoots(): List<File> = listOfNotNull(
        appContext.cacheDir,
        appContext.filesDir,
        privateDownloadsDir(),
        appContext.externalCacheDir,
        appContext.getExternalFilesDir(Environment.DIRECTORY_MUSIC),
    )

    private fun resolveContainedLocalFile(streamUrl: String): File {
        val rawPath = if (streamUrl.startsWith("file://", ignoreCase = true)) {
            runCatching { java.net.URI(streamUrl).path }.getOrNull()
                ?: streamUrl.removePrefix("file://")
        } else {
            streamUrl
        }
        val candidate = File(rawPath)
        if (!isCanonicalFileWithinAllowedRoots(candidate, allowedLocalSourceRoots())) {
            throw SecurityException("Refusing local file outside Sonata's allowed directories")
        }
        return candidate.canonicalFile
    }

    private fun persistAndPublishLocked(records: List<DownloadedTrackRecord>) {
        val array = JSONArray()
        records.forEach { rec ->
            array.put(
                JSONObject().apply {
                    put("videoId", rec.videoId)
                    put("title", rec.title)
                    put("artist", rec.artist)
                    if (rec.album != null) put("album", rec.album)
                    put("durationSecs", rec.durationSecs)
                    if (rec.coverUrl != null) put("coverUrl", rec.coverUrl)
                    put("localUri", rec.localUri)
                    put("fileSizeBytes", rec.fileSizeBytes)
                    put("downloadedAtMs", rec.downloadedAtMs)
                    if (rec.codec != null) put("codec", rec.codec)
                    if (rec.collectionId != null) put("collectionId", rec.collectionId)
                    if (rec.localVideoUri != null) put("localVideoUri", rec.localVideoUri)
                    if (rec.localVideoAudioUri != null) put("localVideoAudioUri", rec.localVideoAudioUri)
                    if (rec.videoQualityLabel != null) put("videoQualityLabel", rec.videoQualityLabel)
                    if (rec.videoQualityHeight > 0) put("videoQualityHeight", rec.videoQualityHeight)
                    if (rec.videoFileSizeBytes > 0L) put("videoFileSizeBytes", rec.videoFileSizeBytes)
                    if (rec.videoSyncSegments.isNotEmpty()) {
                        put(
                            "videoSyncSegments",
                            JSONArray().apply {
                                rec.videoSyncSegments.forEach { seg ->
                                    put(
                                        JSONObject().apply {
                                            put("m", seg.musicStartMs.toLong())
                                            put("v", seg.videoStartMs.toLong())
                                            put("d", seg.durationMs.toLong())
                                        }
                                    )
                                }
                            }
                        )
                    }
                }
            )
        }
        prefs.edit().putString(KEY_RECORDS_JSON, array.toString()).apply()
        _downloads.value = records
        _totalBytes.value = records.sumOf { it.totalSizeBytes }
    }

    private fun loadPersistedRecords(): List<DownloadedTrackRecord> {
        val raw = prefs.getString(KEY_RECORDS_JSON, null) ?: return emptyList()
        return runCatching {
            val array = JSONArray(raw)
            buildList(array.length()) {
                for (i in 0 until array.length()) {
                    val obj = array.optJSONObject(i) ?: continue
                    val videoId = obj.optString("videoId").takeIf { it.isNotBlank() } ?: continue
                    val localUri = obj.optString("localUri").takeIf { it.isNotBlank() } ?: continue
                    val parsedSyncSegments = obj.optJSONArray("videoSyncSegments")?.let { segArr ->
                        buildList(segArr.length()) {
                            for (j in 0 until segArr.length()) {
                                val sObj = segArr.optJSONObject(j) ?: continue
                                val dur = sObj.optLong("d", 0L)
                                if (dur <= 0L) continue
                                add(
                                    AndroidMusicVideoSyncSegment(
                                        musicStartMs = sObj.optLong("m", 0L).coerceAtLeast(0L).toULong(),
                                        videoStartMs = sObj.optLong("v", 0L).coerceAtLeast(0L).toULong(),
                                        durationMs = dur.toULong(),
                                    )
                                )
                            }
                        }
                    }.orEmpty()
                    add(
                        DownloadedTrackRecord(
                            videoId = videoId,
                            title = obj.optString("title", "Unknown Track"),
                            artist = obj.optString("artist", "Unknown Artist"),
                            album = obj.optString("album").takeIf { it.isNotBlank() },
                            durationSecs = obj.optLong("durationSecs", 0L),
                            coverUrl = resolveCoverUrl(
                                videoId,
                                obj.optString("coverUrl").takeIf { it.isNotBlank() },
                            ),
                            localUri = localUri,
                            fileSizeBytes = obj.optLong("fileSizeBytes", 0L),
                            downloadedAtMs = obj.optLong("downloadedAtMs", 0L),
                            codec = obj.optString("codec").takeIf { it.isNotBlank() },
                            collectionId = obj.optString("collectionId").takeIf { it.isNotBlank() },
                            localVideoUri = obj.optString("localVideoUri").takeIf { it.isNotBlank() },
                            localVideoAudioUri = obj.optString("localVideoAudioUri").takeIf { it.isNotBlank() },
                            videoQualityLabel = obj.optString("videoQualityLabel").takeIf { it.isNotBlank() },
                            videoQualityHeight = obj.optInt("videoQualityHeight", 0),
                            videoFileSizeBytes = obj.optLong("videoFileSizeBytes", 0L),
                            videoSyncSegments = parsedSyncSegments,
                        )
                    )
                }
            }
        }.getOrDefault(emptyList())
    }

    private fun inferMimeType(uri: Uri, codecHint: String?): String {
        val mimeParam = uri.getQueryParameter("mime")?.lowercase(Locale.ROOT).orEmpty()
        if (mimeParam.startsWith("audio/")) return mimeParam
        val hint = codecHint?.lowercase(Locale.ROOT).orEmpty()
        return when {
            "flac" in hint -> "audio/flac"
            "opus" in hint || "webm" in hint -> "audio/webm"
            "mp3" in hint -> "audio/mpeg"
            else -> "audio/mp4"
        }
    }

    private fun extensionForMime(mimeType: String): String = when {
        mimeType.contains("flac") -> "flac"
        mimeType.contains("webm") || mimeType.contains("opus") -> "webm"
        mimeType.contains("mpeg") || mimeType.contains("mp3") -> "mp3"
        mimeType.contains("ogg") -> "ogg"
        else -> "m4a"
    }

    private fun inferVideoMimeType(uri: Uri, mimeHint: String?): String {
        val hint = mimeHint?.lowercase(Locale.ROOT).orEmpty()
        if (hint.startsWith("video/")) return hint
        val mimeParam = uri.getQueryParameter("mime")?.lowercase(Locale.ROOT).orEmpty()
        if (mimeParam.startsWith("video/")) return mimeParam
        return if ("webm" in hint || "vp9" in hint) "video/webm" else "video/mp4"
    }

    private fun extensionForVideoMime(mimeType: String): String =
        if (mimeType.contains("webm")) "webm" else "mp4"

    /** Snapshot of offline download usage and free space on the selected storage target. */
    @Immutable
    data class DownloadStorageStats(
        val trackCount: Int,
        val downloadedBytes: Long,
        val availableBytes: Long,
        val totalBytes: Long,
        val locationLabel: String,
    )

    fun queryStorageStats(location: DownloadStorageLocation): DownloadStorageStats {
        val records = _downloads.value
        val used = _totalBytes.value
        val targetDir = when (location) {
            DownloadStorageLocation.INTERNAL -> appContext.filesDir
            DownloadStorageLocation.EXTERNAL -> appContext.getExternalFilesDir(Environment.DIRECTORY_MUSIC)
                ?: Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC)
                ?: appContext.filesDir
        }
        val (avail, total) = runCatching {
            val stat = StatFs(targetDir.absolutePath)
            stat.availableBytes to stat.totalBytes
        }.getOrDefault(0L to 0L)
        val label = when (location) {
            DownloadStorageLocation.INTERNAL -> "Internal · Private App Storage"
            DownloadStorageLocation.EXTERNAL -> "External · Music/Sonata"
        }
        return DownloadStorageStats(
            trackCount = records.size,
            downloadedBytes = used,
            availableBytes = avail,
            totalBytes = total,
            locationLabel = label,
        )
    }

    internal fun resolveCoverUrl(videoId: String, coverUrl: String?): String? =
        coverUrl?.takeIf { it.isNotBlank() }
            ?: resolveYtmFallbackCoverUrl(videoId)

    private fun sanitizeId(id: String): String =
        id.replace(Regex("[^A-Za-z0-9._-]"), "_").ifBlank { "track" }

    private fun buildDisplayName(artist: String, title: String, videoId: String, ext: String): String {
        val cleanArtist = artist.replace(Regex("[\\\\/:*?\"<>|]"), "").trim()
        val cleanTitle = title.replace(Regex("[\\\\/:*?\"<>|]"), "").trim()
        val stem = when {
            cleanArtist.isNotEmpty() && cleanTitle.isNotEmpty() -> "$cleanArtist - $cleanTitle"
            cleanTitle.isNotEmpty() -> cleanTitle
            else -> sanitizeId(videoId)
        }.take(96).trimEnd('.', ' ')
        return "$stem.$ext"
    }

    companion object {
        internal fun isCanonicalFileWithinAllowedRoots(
            candidate: File,
            allowedRoots: List<File>,
        ): Boolean {
            val canonicalCandidate = runCatching { candidate.canonicalFile }.getOrNull() ?: return false
            val candidateName = canonicalCandidate.name.lowercase(Locale.ROOT)
            if (candidateName.endsWith(".cookie") || candidateName.endsWith(".key") || candidateName.contains(".db")) {
                return false
            }
            val candidatePath = canonicalCandidate.toPath()
            return allowedRoots.any { root ->
                val canonicalRoot = runCatching { root.canonicalFile.toPath() }.getOrNull()
                    ?: return@any false
                candidatePath.startsWith(canonicalRoot)
            }
        }

        private const val PREFS_NAME = "sonata_downloads"
        private const val KEY_RECORDS_JSON = "downloaded_records_v1"
        private const val EXPORT_FOLDER = "Sonata"
        private const val CHUNK_BYTES = 1024L * 1024L
        private const val NARROW_CHUNK_BYTES = 512L * 1024L
        private const val PROGRESS_MIN_DELTA = 0.02f
        private const val PROGRESS_MIN_INTERVAL_MS = 120L
        private const val IO_BUFFER_BYTES = 64 * 1024
        private const val CONNECT_TIMEOUT_MS = 8_000
        private const val READ_TIMEOUT_MS = 15_000

        private const val VISIONOS_UA =
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 " +
                "(KHTML, like Gecko) Version/26.0 Safari/605.1.15"
        private const val WEB_UA =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
                "(KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
        private const val TVHTML5_UA =
            "Mozilla/5.0 (SMART-TV; Linux; Tizen 2.4.0) AppleWebKit/538.1 " +
                "(KHTML, like Gecko) Version/2.4.0 TV Safari/538.1"
        private const val DEFAULT_MOBILE_UA =
            "Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36 Chrome/131.0.0.0 Mobile Safari/537.36"
    }
}
