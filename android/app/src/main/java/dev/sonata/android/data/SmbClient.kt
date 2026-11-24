/*
 * Native Android SMB2/SMB3 client and seekable Media3 ExoPlayer DataSource backed by smbj.
 *
 * Allows browsing SMB shares, probing reachability, benchmarking download throughput, searching
 * remote audio collections, and streaming Hi-Res/FLAC/MP3 files directly over TCP port 445
 * without requiring OS-level local mounts.
 */
package dev.sonata.android.data

import android.net.Uri
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import com.hierynomus.msdtyp.AccessMask
import com.hierynomus.msfscc.FileAttributes
import com.hierynomus.mssmb2.SMB2CreateDisposition
import com.hierynomus.mssmb2.SMB2ShareAccess
import com.hierynomus.smbj.SMBClient
import com.hierynomus.smbj.SmbConfig
import com.hierynomus.smbj.auth.AuthenticationContext
import com.hierynomus.smbj.connection.Connection
import com.hierynomus.smbj.session.Session
import com.hierynomus.smbj.share.DiskShare
import com.hierynomus.smbj.share.File as SmbFile
import dev.sonata.android.PlaybackService
import dev.sonata.android.ServerSourceSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.w3c.dom.Element
import org.w3c.dom.Node
import org.xml.sax.InputSource
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidLyricWord
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidServerHealthReport
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerLibrary
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSearchResult
import uniffi.sonata_android.AndroidServerSpeedTestReport
import uniffi.sonata_android.AndroidTrack
import java.io.ByteArrayOutputStream
import java.io.EOFException
import java.io.File
import java.io.IOException
import java.io.StringReader
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.EnumSet
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import javax.xml.XMLConstants
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

/**
 * Parsed SMB2/SMB3 connection target derived from an [AndroidServerProfile].
 */
data class SmbEndpoint(
    val host: String,
    val port: Int,
    val share: String,
    val baseSubpath: String,
    val username: String,
    val password: String,
    val domain: String,
)

/**
 * Result of listing a single SMB directory.
 */
data class SmbDirectoryListing(
    val folders: List<AndroidServerLibrary>,
    val tracks: List<AndroidTrack>,
)

/** Parsed metadata read from an artist folder (`artist.json`, `artist.nfo`, `info.json`). */
data class ParsedArtistMetadata(
    val name: String? = null,
    val bio: String? = null,
    val subtitle: String? = null,
)

/** Rich SMB artist profile combining artist folder metadata/avatar with scanned albums and tracks. */
data class SmbArtistProfile(
    val name: String,
    val avatarUrl: String?,
    val bio: String?,
    val subtitle: String?,
    val albums: List<AndroidHomeCard>,
    val tracks: List<AndroidTrack>,
    val relatedArtists: List<AndroidHomeCard>,
)

/**
 * Thread-safe in-memory registry mapping `serverId -> AndroidServerProfile` so
 * [SmbDataSource] inside `PlaybackService` can authenticate without exposing passwords in URIs.
 */
object SmbProfileRegistry {
    private val profiles = ConcurrentHashMap<String, AndroidServerProfile>()

    fun replaceAll(list: List<AndroidServerProfile>) {
        val nextIds = list.filter { it.kind == AndroidServerKind.SMB }.mapTo(HashSet()) { it.id }
        profiles.keys.retainAll(nextIds)
        list.filter { it.kind == AndroidServerKind.SMB }.forEach { srv ->
            profiles[srv.id] = srv
        }
    }

    fun put(profile: AndroidServerProfile) {
        if (profile.kind == AndroidServerKind.SMB && profile.id.isNotBlank()) {
            profiles[profile.id] = profile
        }
    }

    fun get(serverId: String): AndroidServerProfile? = profiles[serverId]
}

/**
 * Native SMB2/SMB3 client for Sonata Android.
 */
object SmbClient {
    private const val TAG = "SonataSmb"
    const val STREAM_SCHEME = "sonata-smb"
    const val DEFAULT_SMB_PORT = 445
    private const val MAX_SCAN_DEPTH = 6
    private const val MAX_SCAN_TRACKS = 1500
    private const val SPEED_TEST_MAX_BYTES = 1024 * 1024
    private const val MAX_ART_BYTES = 8 * 1024 * 1024
    private const val MAX_LYRICS_BYTES = 512 * 1024
    private const val FLAC_HEADER_READ_BYTES = 128 * 1024
    private const val FLAC_PICTURE_READ_BYTES = 768 * 1024
    private const val MAX_FLAC_TAG_READS_PER_FOLDER = 60

    @Volatile
    private var smbArtCacheDir: File? = null

    private val SUPPORTED_AUDIO_EXTS = setOf(
        "flac", "mp3", "m4a", "aac", "opus", "ogg", "oga", "wav", "alac", "aiff", "aif", "wma", "wv", "m4b",
    )

    private val SUPPORTED_IMAGE_EXTS = setOf("jpg", "jpeg", "png", "webp")

    private val GENERIC_CONTAINER_SEGMENTS = setOf(
        "music", "musix", "audio", "media", "library", "share", "songs", "tracks", "flac", "lossless", "mp3",
    )

    private val DISC_FOLDER_REGEX = Regex("^(?:cd|disc|disk)\\s*\\d+$", RegexOption.IGNORE_CASE)

    private val COVER_PRIORITY_BASENAMES = listOf("cover", "folder", "front", "album", "artwork", "thumb")
    private val ARTIST_IMAGE_BASENAMES = listOf("artist", "avatar", "photo", "portrait", "folder", "cover", "thumb")
    private val ARTIST_META_FILENAMES = listOf("artist.json", "artist.nfo", "info.json", "metadata.json")

    private val artistMetaCache = ConcurrentHashMap<String, MutableMap<String, CachedArtistFolderMeta>>()

    private data class CachedArtistFolderMeta(
        val relDir: String,
        val metadata: ParsedArtistMetadata,
        val avatarUrl: String?,
    )

    private val LRC_TAG_REGEX = Regex("""\[([^\]]+)]""")
    private val LRC_TIMESTAMP_REGEX = Regex("""^(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?$""")
    private val LRC_WORD_REGEX = Regex("""<(\d{1,3}:\d{1,2}(?:[.:]\d{1,3})?)>""")

    fun initCacheDir(baseCacheDir: File) {
        val dir = File(baseCacheDir, "smb_art")
        runCatching { dir.mkdirs() }
        smbArtCacheDir = dir
    }

    private val summaryCache = ConcurrentHashMap<String, CachedSummary>()

    private data class CachedSummary(
        val summary: ServerSourceSummary,
        val cachedAtMs: Long,
    )

    fun invalidateCache(serverId: String? = null) {
        if (serverId.isNullOrBlank()) {
            summaryCache.clear()
            artistMetaCache.clear()
        } else {
            summaryCache.remove(serverId)
            artistMetaCache.remove(serverId)
        }
    }

    fun clearArtworkCache() {
        smbArtCacheDir?.listFiles()?.forEach { file ->
            runCatching { file.delete() }
        }
    }

    /**
     * Parses `smb://host[:port]/share[/subpath]`, UNC `\\host\share[\subpath]`, or `host[:port]` + [profile.share].
     */
    fun parseEndpoint(profile: AndroidServerProfile): Result<SmbEndpoint> {
        val rawInput = profile.baseUrl.trim()
        val explicitShare = profile.share?.trim()?.trim('/')?.takeIf { it.isNotEmpty() }
        if (rawInput.isBlank()) {
            return Result.failure(IllegalArgumentException("SMB server host is required"))
        }

        var host = ""
        var port = DEFAULT_SMB_PORT
        var parsedShare: String? = null
        var subpathSegments: List<String> = emptyList()

        if (rawInput.startsWith("\\\\") || rawInput.startsWith("//")) {
            val parts = rawInput.trimStart('\\', '/').split('\\', '/').filter { it.isNotBlank() }
            val hostPort = parts.firstOrNull().orEmpty()
            val (h, p) = splitHostPort(hostPort)
            host = h
            port = p
            parsedShare = parts.getOrNull(1)
            subpathSegments = parts.drop(2)
        } else {
            val normalized = if (rawInput.contains("://")) rawInput else "smb://$rawInput"
            val uri = runCatching { URI(normalized) }.getOrElse { err ->
                return Result.failure(IllegalArgumentException("Invalid SMB address: ${err.message}"))
            }
            host = uri.host.orEmpty().trim()
            if (host.isBlank() && uri.authority != null) {
                val (h, p) = splitHostPort(uri.authority.substringAfterLast('@'))
                host = h
                port = p
            } else if (uri.port > 0) {
                port = uri.port
            }
            val pathParts = uri.rawPath.orEmpty()
                .split('/')
                .filter { it.isNotBlank() }
                .map { seg -> runCatching { URLDecoder.decode(seg, "UTF-8") }.getOrDefault(seg) }
            if (pathParts.isNotEmpty()) {
                parsedShare = pathParts.first()
                subpathSegments = pathParts.drop(1)
            }
        }

        if (host.isBlank()) {
            return Result.failure(IllegalArgumentException("SMB server hostname or IP is required"))
        }
        val share = (parsedShare?.takeIf { it.isNotBlank() } ?: explicitShare).orEmpty()
        if (share.isBlank()) {
            return Result.failure(IllegalArgumentException("SMB share name is required (e.g. smb://host/music)"))
        }

        return Result.success(
            SmbEndpoint(
                host = host,
                port = port,
                share = share,
                baseSubpath = subpathSegments.joinToString("/"),
                username = profile.username.orEmpty().trim(),
                password = profile.password.orEmpty(),
                domain = profile.domain.orEmpty().trim(),
            ),
        )
    }

    private fun splitHostPort(authority: String): Pair<String, Int> {
        val clean = authority.trim()
        if (clean.startsWith("[")) {
            val closeIdx = clean.indexOf(']')
            if (closeIdx > 1) {
                val ipv6 = clean.substring(1, closeIdx)
                val rem = clean.substring(closeIdx + 1)
                val p = rem.removePrefix(":").toIntOrNull()?.takeIf { it in 1..65535 } ?: DEFAULT_SMB_PORT
                return ipv6 to p
            }
        }
        val lastColon = clean.lastIndexOf(':')
        if (lastColon > 0 && clean.indexOf(':') == lastColon) {
            val p = clean.substring(lastColon + 1).toIntOrNull()
            if (p != null && p in 1..65535) {
                return clean.substring(0, lastColon) to p
            }
        }
        return clean to DEFAULT_SMB_PORT
    }

    /**
     * Assembles a canonical `smb://host[:port]/share[/subpath]` URL from separate UI form fields.
     */
    fun buildBaseUrl(host: String, portText: String, share: String, subpath: String = ""): String {
        val cleanHost = host.trim()
            .removePrefix("smb://")
            .removePrefix("SMB://")
            .trimStart('\\', '/')
            .substringBefore('/')
            .substringBefore('\\')
            .trim()
        val parsedPort = portText.trim().toIntOrNull()?.takeIf { it in 1..65535 && it != DEFAULT_SMB_PORT }
        val hostPart = if (parsedPort != null) "$cleanHost:$parsedPort" else cleanHost
        val cleanShare = share.trim().trim('/', '\\')
        val cleanSub = subpath.trim().trim('/', '\\').replace('\\', '/')
        return buildString {
            append("smb://")
            append(hostPart)
            if (cleanShare.isNotEmpty()) {
                append('/')
                append(cleanShare)
                if (cleanSub.isNotEmpty()) {
                    append('/')
                    append(cleanSub)
                }
            }
        }
    }

    /** Builds a secret-free custom Media3 stream URI for an SMB track. */
    fun buildStreamUri(serverId: String, relativePath: String): String {
        val encodedServer = URLEncoder.encode(serverId, "UTF-8").replace("+", "%20")
        val encoded = URLEncoder.encode(relativePath.trimStart('/'), "UTF-8").replace("+", "%20")
        return "$STREAM_SCHEME://$encodedServer/$encoded"
    }

    /** Parses `sonata-smb://<serverId>/<encodedPath>` back into `(serverId, relativePath)`. */
    fun parseStreamUri(rawUri: String): Pair<String, String>? {
        val parsed = runCatching { URI(rawUri) }.getOrNull() ?: return null
        if (!parsed.scheme.equals(STREAM_SCHEME, ignoreCase = true)) return null
        val rawHost = parsed.rawAuthority?.substringBefore(':') ?: return null
        val serverId = runCatching { URLDecoder.decode(rawHost, "UTF-8") }.getOrDefault(rawHost)
            .takeIf { it.isNotBlank() } ?: return null
        val rawPath = parsed.rawPath?.removePrefix("/").orEmpty()
        if (rawPath.isBlank()) return null
        val decoded = runCatching { URLDecoder.decode(rawPath, "UTF-8") }.getOrDefault(rawPath)
        return serverId to decoded
    }

    /** Parses an Android [Uri] `sonata-smb://<serverId>/<encodedPath>` back into `(serverId, relativePath)`. */
    fun parseStreamUri(uri: Uri): Pair<String, String>? = parseStreamUri(uri.toString())

    /** Extracts `(serverId, relativePath)` from `smb:<serverId>:<relativePath>` track IDs. */
    fun parseTrackId(trackId: String): Pair<String, String>? {
        if (!trackId.startsWith("smb:")) return null
        val rest = trackId.removePrefix("smb:")
        val sep = rest.indexOf(':')
        if (sep <= 0 || sep >= rest.lastIndex) return null
        val serverId = rest.substring(0, sep)
        val relPath = rest.substring(sep + 1)
        return serverId to relPath
    }

    /** Infers a clean audio codec badge from an SMB filename without appending server protocol names. */
    fun initialBadgeForPath(path: String): String {
        val ext = path.substringAfterLast('.', "").lowercase(Locale.ROOT)
        return when (ext) {
            "flac" -> "FLAC"
            "alac" -> "ALAC"
            "wav", "aiff", "aif" -> "PCM"
            "opus" -> "OPUS"
            "ogg", "oga" -> "OGG"
            "mp3" -> "MP3"
            "m4a", "aac", "m4b" -> "AAC"
            "wv" -> "WV"
            else -> ""
        }
    }

    private val measuredTrackBadges = ConcurrentHashMap<String, String>()

    /** Returns a pre-measured quality badge for an SMB track if parsed during folder scan, else codec badge. */
    fun initialBadgeForTrack(serverId: String, relativePath: String): String =
        measuredTrackBadges["$serverId|$relativePath"] ?: initialBadgeForPath(relativePath)

    private fun newConfig(): SmbConfig = SmbConfig.builder()
        .withTimeout(10_000L, TimeUnit.MILLISECONDS)
        .withSoTimeout(15_000L, TimeUnit.MILLISECONDS)
        .build()

    private inline fun <T> withShare(profile: AndroidServerProfile, block: (SmbEndpoint, DiskShare) -> T): T {
        val endpoint = parseEndpoint(profile).getOrThrow()
        val client = SMBClient(newConfig())
        var connection: Connection? = null
        var session: Session? = null
        var share: DiskShare? = null
        try {
            connection = client.connect(endpoint.host, endpoint.port)
            val domain = endpoint.domain.ifBlank { null }
            val authCandidates = if (endpoint.username.isBlank() && endpoint.password.isBlank()) {
                listOf(AuthenticationContext.anonymous(), AuthenticationContext.guest())
            } else {
                listOf(AuthenticationContext(endpoint.username, endpoint.password.toCharArray(), domain))
            }
            var lastAuthErr: Throwable? = null
            for (ctx in authCandidates) {
                try {
                    session = connection.authenticate(ctx)
                    lastAuthErr = null
                    break
                } catch (err: Throwable) {
                    lastAuthErr = err
                }
            }
            val activeSession = session ?: throw (lastAuthErr ?: IOException("SMB authentication failed"))
            val connectedShare = activeSession.connectShare(endpoint.share)
            val diskShare = connectedShare as? DiskShare
                ?: throw IOException("SMB share '${endpoint.share}' is not a disk share")
            share = diskShare
            return block(endpoint, diskShare)
        } finally {
            runCatching { share?.close() }
            runCatching { session?.close() }
            runCatching { connection?.close() }
            runCatching { client.close() }
        }
    }

    suspend fun testConnection(profile: AndroidServerProfile): String = withContext(Dispatchers.IO) {
        val t0 = System.nanoTime()
        withShare(profile) { endpoint, diskShare ->
            val rootWinPath = toSmbWinPath(endpoint.baseSubpath, "")
            diskShare.list(rootWinPath)
            val elapsedMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - t0).coerceAtLeast(1L)
            "SMB2/3 OK (${endpoint.host}:${endpoint.port}/${endpoint.share}, ${elapsedMs}ms)"
        }
    }

    suspend fun probeHealth(profile: AndroidServerProfile): AndroidServerHealthReport = withContext(Dispatchers.IO) {
        val t0 = System.nanoTime()
        runCatching {
            withShare(profile) { endpoint, diskShare ->
                val rootWinPath = toSmbWinPath(endpoint.baseSubpath, "")
                diskShare.list(rootWinPath)
                val elapsedMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - t0).coerceAtLeast(1L)
                AndroidServerHealthReport(
                    alive = true,
                    latencyMs = elapsedMs.toULong(),
                    message = "SMB2/3 (${endpoint.host}:${endpoint.port}/${endpoint.share})",
                )
            }
        }.getOrElse { err ->
            Log.w(TAG, "SMB health check failed for ${profile.id}: ${err.message}")
            AndroidServerHealthReport(
                alive = false,
                latencyMs = null,
                message = err.message ?: "SMB connection failed",
            )
        }
    }

    suspend fun runSpeedTest(profile: AndroidServerProfile): AndroidServerSpeedTestReport = withContext(Dispatchers.IO) {
        val t0 = System.nanoTime()
        runCatching {
            withShare(profile) { endpoint, diskShare ->
                val connectMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - t0).coerceAtLeast(1L)
                val sampleRel = findFirstAudioFile(diskShare, endpoint.baseSubpath, "", 0)
                if (sampleRel == null) {
                    return@withShare AndroidServerSpeedTestReport(
                        alive = true,
                        latencyMs = connectMs.toULong(),
                        downloadMbps = null,
                        bytesTested = 0uL,
                        sampleNote = "Connected (${connectMs}ms), no audio sample found",
                    )
                }
                val winPath = toSmbWinPath(endpoint.baseSubpath, sampleRel)
                val smbFile = diskShare.openFile(
                    winPath,
                    EnumSet.of(AccessMask.GENERIC_READ),
                    null,
                    SMB2ShareAccess.ALL,
                    SMB2CreateDisposition.FILE_OPEN,
                    null,
                )
                try {
                    val buf = ByteArray(65_536)
                    var offset = 0L
                    var totalRead = 0L
                    val readStart = System.nanoTime()
                    while (totalRead < SPEED_TEST_MAX_BYTES) {
                        val toRead = min(buf.size, SPEED_TEST_MAX_BYTES - totalRead.toInt())
                        val n = smbFile.read(buf, offset, 0, toRead)
                        if (n <= 0) break
                        offset += n
                        totalRead += n
                    }
                    val readNs = (System.nanoTime() - readStart).coerceAtLeast(1L)
                    val mbps = if (totalRead > 0L) {
                        (totalRead * 8.0) / (readNs / 1_000_000_000.0) / 1_000_000.0
                    } else {
                        null
                    }
                    AndroidServerSpeedTestReport(
                        alive = true,
                        latencyMs = connectMs.toULong(),
                        downloadMbps = mbps,
                        bytesTested = totalRead.toULong(),
                        sampleNote = "${totalRead / 1024} KB sampled from ${sampleRel.substringAfterLast('/')}",
                    )
                } finally {
                    runCatching { smbFile.close() }
                }
            }
        }.getOrElse { err ->
            Log.w(TAG, "SMB speed test failed for ${profile.id}: ${err.message}")
            AndroidServerSpeedTestReport(
                alive = false,
                latencyMs = 0uL,
                downloadMbps = null,
                bytesTested = 0uL,
                sampleNote = err.message ?: "SMB speed test failed",
            )
        }
    }

    suspend fun listFolder(profile: AndroidServerProfile, relativePath: String?): SmbDirectoryListing =
        withContext(Dispatchers.IO) {
            withShare(profile) { endpoint, diskShare ->
                listFolderInternal(profile, endpoint, diskShare, relativePath.orEmpty())
            }
        }

    suspend fun scanSummary(
        profile: AndroidServerProfile,
        forceRefresh: Boolean = false,
    ): ServerSourceSummary = withContext(Dispatchers.IO) {
        val now = System.currentTimeMillis()
        if (!forceRefresh) {
            summaryCache[profile.id]?.let { cached ->
                if (now - cached.cachedAtMs < 5 * 60_000L) {
                    return@withContext cached.summary
                }
            }
        }
        val tracks = ArrayList<AndroidTrack>()
        val albumsMap = LinkedHashMap<String, MutableList<AndroidTrack>>()
        val artistsMap = LinkedHashMap<String, MutableList<AndroidTrack>>()
        val discoveredArtistMeta = LinkedHashMap<String, CachedArtistFolderMeta>()

        withShare(profile) { endpoint, diskShare ->
            fun walk(relDir: String, depth: Int) {
                if (depth > MAX_SCAN_DEPTH || tracks.size >= MAX_SCAN_TRACKS) return
                val listing = listFolderInternal(profile, endpoint, diskShare, relDir)
                if (listing.folders.isNotEmpty() && relDir.isNotBlank()) {
                    val folderMeta = readArtistFolderMetadata(profile, endpoint, diskShare, relDir)
                    if (folderMeta != null) {
                        val folderKey = relDir.substringAfterLast('/').trim().lowercase(Locale.ROOT)
                        if (folderKey.isNotEmpty()) {
                            discoveredArtistMeta.putIfAbsent(folderKey, folderMeta)
                        }
                        folderMeta.metadata.name?.trim()?.lowercase(Locale.ROOT)?.takeIf { it.isNotEmpty() }?.let {
                            discoveredArtistMeta.putIfAbsent(it, folderMeta)
                        }
                    }
                }
                if (listing.tracks.isNotEmpty()) {
                    tracks.addAll(listing.tracks.take(MAX_SCAN_TRACKS - tracks.size))
                    val albumKey = relDir.ifBlank { listing.tracks.first().album }
                    albumsMap.getOrPut(albumKey) { ArrayList() }.addAll(listing.tracks)
                    listing.tracks.forEach { t ->
                        val artistName = t.artist.ifBlank { "Unknown Artist" }
                        artistsMap.getOrPut(artistName) { ArrayList() }.add(t)
                    }
                }
                listing.folders.forEach { dir ->
                    if (tracks.size < MAX_SCAN_TRACKS) {
                        walk(dir.id, depth + 1)
                    }
                }
            }
            walk("", 0)
        }
        artistMetaCache[profile.id] = discoveredArtistMeta

        val albums = albumsMap.entries.map { (key, list) ->
            val firstTrack = list.firstOrNull()
            val title = firstTrack?.album
                ?.takeIf { it.isNotBlank() && it != "Unknown Album" }
                ?: key.substringAfterLast('/').ifBlank { "Unknown Album" }
            val subtitle = firstTrack?.artist?.takeIf { it.isNotBlank() && it != "Unknown Artist" }
                ?: "${list.size} tracks"
            val coverThumb = list.firstNotNullOfOrNull { it.thumbnailUrl }
            AndroidHomeCard(
                kind = AndroidHomeCardKind.ALBUM,
                id = "smb:${profile.id}:dir:$key",
                title = title,
                subtitle = subtitle,
                thumbnailUrl = coverThumb,
                track = firstTrack,
            )
        }
        val artists = artistsMap.entries.map { (name, list) ->
            val meta = discoveredArtistMeta[name.trim().lowercase(Locale.ROOT)]
            AndroidHomeCard(
                kind = AndroidHomeCardKind.ARTIST,
                id = "smb:${profile.id}:artist:$name",
                title = meta?.metadata?.name ?: name,
                subtitle = meta?.metadata?.subtitle ?: "${list.size} tracks",
                thumbnailUrl = meta?.avatarUrl ?: list.firstNotNullOfOrNull { it.thumbnailUrl },
                track = list.firstOrNull(),
            )
        }
        val summary = ServerSourceSummary(
            serverId = profile.id,
            albums = albums,
            playlists = emptyList(),
            artists = artists,
            tracks = tracks,
            loading = false,
            error = null,
        )
        summaryCache[profile.id] = CachedSummary(summary, System.currentTimeMillis())
        summary
    }

    /**
     * Parses artist metadata from JSON (`artist.json`, `info.json`, `metadata.json`) or XML (`artist.nfo`).
     */
    fun parseArtistMetadataText(raw: String, isXml: Boolean = false): ParsedArtistMetadata {
        val cleaned = raw.trim().removePrefix("\uFEFF")
        if (cleaned.isEmpty()) return ParsedArtistMetadata()

        if (!isXml && cleaned.startsWith("{")) {
            return runCatching {
                val obj = JSONObject(cleaned)
                val name = obj.optString("name").trim().ifBlank { null }
                val bio = obj.optString("bio")
                    .ifBlank { obj.optString("biography") }
                    .ifBlank { obj.optString("description") }
                    .trim()
                    .ifBlank { null }
                val genre = obj.optString("genre").trim().ifBlank { null }
                val origin = obj.optString("origin")
                    .ifBlank { obj.optString("country") }
                    .ifBlank { obj.optString("formed") }
                    .trim()
                    .ifBlank { null }
                val subtitle = obj.optString("subtitle").trim().ifBlank { null }
                    ?: listOfNotNull(genre, origin).joinToString(" · ").ifBlank { null }
                ParsedArtistMetadata(name = name, bio = bio, subtitle = subtitle)
            }.getOrDefault(ParsedArtistMetadata())
        }

        fun xmlTag(tag: String): String? {
            val regex = Regex("""<$tag\b[^>]*>(.*?)</$tag>""", setOf(RegexOption.IGNORE_CASE, RegexOption.DOT_MATCHES_ALL))
            return regex.find(cleaned)?.groupValues?.getOrNull(1)
                ?.replace("<![CDATA[", "")
                ?.replace("]]>", "")
                ?.trim()
                ?.ifBlank { null }
        }

        val name = xmlTag("name")
        val bio = xmlTag("biography") ?: xmlTag("plot") ?: xmlTag("description")
        val genre = xmlTag("genre")
        val origin = xmlTag("origin") ?: xmlTag("formed") ?: xmlTag("country")
        val subtitle = listOfNotNull(genre, origin).joinToString(" · ").ifBlank { null }
        return ParsedArtistMetadata(name = name, bio = bio, subtitle = subtitle)
    }

    private fun readArtistFolderMetadata(
        profile: AndroidServerProfile,
        endpoint: SmbEndpoint,
        diskShare: DiskShare,
        relDir: String,
    ): CachedArtistFolderMeta? {
        val cleanRel = relDir.trim('/', '\\').replace('\\', '/')
        val winDir = toSmbWinPath(endpoint.baseSubpath, cleanRel)
        val entries = runCatching { diskShare.list(winDir) }.getOrNull() ?: return null

        val imageFiles = ArrayList<String>()
        var metaFileName: String? = null

        for (entry in entries) {
            val name = entry.fileName ?: continue
            if (name.startsWith(".")) continue
            val isDir = (entry.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
            if (isDir) continue
            val ext = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
            if (ext in SUPPORTED_IMAGE_EXTS) {
                imageFiles += name
            } else if (metaFileName == null && ARTIST_META_FILENAMES.any { it.equals(name, ignoreCase = true) }) {
                metaFileName = name
            }
        }

        if (imageFiles.isEmpty() && metaFileName == null) return null

        val chosenImage = ARTIST_IMAGE_BASENAMES.firstNotNullOfOrNull { base ->
            imageFiles.firstOrNull { it.substringBeforeLast('.').equals(base, ignoreCase = true) }
        } ?: imageFiles.firstOrNull()

        val avatarUri = chosenImage?.let { imgName ->
            val ext = imgName.substringAfterLast('.', "jpg").lowercase(Locale.ROOT)
            val cacheKey = "${profile.id}|$winDir|artist|$imgName"
            cachedArtworkUriIfExists(cacheKey, ext) ?: runCatching {
                val imgRel = if (cleanRel.isBlank()) imgName else "$cleanRel/$imgName"
                val imgBytes = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, imgRel), MAX_ART_BYTES)
                cacheArtworkBytes(cacheKey, ext, imgBytes)
            }.getOrNull()
        }

        val parsedMeta = metaFileName?.let { mName ->
            runCatching {
                val metaRel = if (cleanRel.isBlank()) mName else "$cleanRel/$mName"
                val raw = String(readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, metaRel), 64 * 1024), StandardCharsets.UTF_8)
                parseArtistMetadataText(raw, isXml = mName.endsWith(".nfo", ignoreCase = true))
            }.getOrNull()
        } ?: ParsedArtistMetadata()

        return CachedArtistFolderMeta(
            relDir = cleanRel,
            metadata = parsedMeta,
            avatarUrl = avatarUri,
        )
    }

    /**
     * Builds a rich [SmbArtistProfile] (avatar, bio, subtitle, albums, tracks, related artists) for [artistName].
     */
    suspend fun fetchArtistProfile(profile: AndroidServerProfile, artistName: String): SmbArtistProfile? =
        withContext(Dispatchers.IO) {
            val targetName = artistName.trim()
            if (targetName.isEmpty()) return@withContext null
            val summary = scanSummary(profile)
            val matchedTracks = summary.tracks.filter { it.artist.equals(targetName, ignoreCase = true) }
            val matchedAlbums = summary.albums.filter {
                it.subtitle.equals(targetName, ignoreCase = true) ||
                    matchedTracks.any { tr -> tr.albumId != null && tr.albumId == it.id }
            }
            val metaKey = targetName.lowercase(Locale.ROOT)
            var cachedFolderMeta = artistMetaCache[profile.id]?.get(metaKey)
            if (cachedFolderMeta == null && matchedTracks.isNotEmpty()) {
                val firstTrackAlbumDir = matchedTracks.firstNotNullOfOrNull { it.albumId }
                    ?.removePrefix("smb:${profile.id}:dir:")
                    ?.trim('/')
                val parentArtistDir = firstTrackAlbumDir?.substringBeforeLast('/', "")?.takeIf { '/' in firstTrackAlbumDir }
                if (!parentArtistDir.isNullOrBlank()) {
                    cachedFolderMeta = runCatching {
                        withShare(profile) { endpoint, diskShare ->
                            readArtistFolderMetadata(profile, endpoint, diskShare, parentArtistDir)
                        }
                    }.getOrNull()?.also { fetched ->
                        artistMetaCache.getOrPut(profile.id) { LinkedHashMap() }[metaKey] = fetched
                    }
                }
            }

            if (matchedTracks.isEmpty() && matchedAlbums.isEmpty() && cachedFolderMeta == null) {
                return@withContext null
            }

            val resolvedName = cachedFolderMeta?.metadata?.name
                ?: matchedTracks.firstOrNull()?.artist?.takeIf { it.isNotBlank() }
                ?: targetName
            val avatar = cachedFolderMeta?.avatarUrl
                ?: matchedAlbums.firstNotNullOfOrNull { it.thumbnailUrl }
                ?: matchedTracks.firstNotNullOfOrNull { it.thumbnailUrl }
            val related = summary.artists.filterNot { it.title.equals(resolvedName, ignoreCase = true) }

            SmbArtistProfile(
                name = resolvedName,
                avatarUrl = avatar,
                bio = cachedFolderMeta?.metadata?.bio,
                subtitle = cachedFolderMeta?.metadata?.subtitle,
                albums = matchedAlbums,
                tracks = matchedTracks,
                relatedArtists = related,
            )
        }

    suspend fun getAlbumTracks(profile: AndroidServerProfile, cardId: String): List<AndroidTrack> =
        withContext(Dispatchers.IO) {
            val prefix = "smb:${profile.id}:dir:"
            if (cardId.startsWith(prefix)) {
                val relDir = cardId.removePrefix(prefix).takeUnless { it == profile.name }.orEmpty()
                return@withContext listFolder(profile, relDir).tracks
            }
            val artistPrefix = "smb:${profile.id}:artist:"
            if (cardId.startsWith(artistPrefix)) {
                val targetArtist = cardId.removePrefix(artistPrefix)
                val summary = scanSummary(profile)
                return@withContext summary.tracks.filter {
                    it.artist.equals(targetArtist, ignoreCase = true)
                }
            }
            emptyList()
        }

    suspend fun searchTracks(profile: AndroidServerProfile, query: String): AndroidServerSearchResult =
        withContext(Dispatchers.IO) {
            val q = query.trim()
            val summary = scanSummary(profile)
            if (q.isBlank()) {
                return@withContext AndroidServerSearchResult(
                    tracks = summary.tracks.take(40),
                    albums = summary.albums.take(20),
                    artists = summary.artists.take(20),
                    playlists = emptyList(),
                )
            }
            val matchedTracks = summary.tracks.filter {
                it.title.contains(q, ignoreCase = true) ||
                    it.artist.contains(q, ignoreCase = true) ||
                    it.album.contains(q, ignoreCase = true) ||
                    it.id.contains(q, ignoreCase = true)
            }.take(40)
            val matchedAlbums = summary.albums.filter {
                it.title.contains(q, ignoreCase = true) ||
                    it.subtitle.contains(q, ignoreCase = true)
            }.take(20)
            val matchedArtists = summary.artists.filter {
                it.title.contains(q, ignoreCase = true)
            }.take(20)
            AndroidServerSearchResult(
                tracks = matchedTracks,
                albums = matchedAlbums,
                artists = matchedArtists,
                playlists = emptyList(),
            )
        }

    private fun findFirstAudioFile(
        diskShare: DiskShare,
        baseSubpath: String,
        relDir: String,
        depth: Int,
    ): String? {
        if (depth > 4) return null
        val winPath = toSmbWinPath(baseSubpath, relDir)
        val entries = runCatching { diskShare.list(winPath) }.getOrNull() ?: return null
        val subdirs = ArrayList<String>()
        for (entry in entries) {
            val name = entry.fileName ?: continue
            if (name == "." || name == ".." || name.startsWith(".")) continue
            val isDir = (entry.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
            val childRel = if (relDir.isBlank()) name else "$relDir/$name"
            if (isDir) {
                subdirs += childRel
            } else if (isSupportedAudioFile(name)) {
                return childRel
            }
        }
        for (dir in subdirs) {
            val found = findFirstAudioFile(diskShare, baseSubpath, dir, depth + 1)
            if (found != null) return found
        }
        return null
    }

    private data class FolderSidecarMetadata(
        val artist: String? = null,
        val album: String? = null,
    )

    data class FlacHeaderMetadata(
        val title: String? = null,
        val artist: String? = null,
        val albumArtist: String? = null,
        val album: String? = null,
        val durationSecs: Long? = null,
        val exactDurationSecs: Double? = null,
        val sampleRateHz: Int = 0,
        val channels: Int = 0,
        val bitsPerSample: Int = 0,
        val lyrics: String? = null,
        val pictureMime: String? = null,
        val pictureBytes: ByteArray? = null,
        val hasPictureBlock: Boolean = false,
    )

    private fun readLeUInt32(bytes: ByteArray, pos: Int): Long =
        (bytes[pos].toLong() and 0xFFL) or
            ((bytes[pos + 1].toLong() and 0xFFL) shl 8) or
            ((bytes[pos + 2].toLong() and 0xFFL) shl 16) or
            ((bytes[pos + 3].toLong() and 0xFFL) shl 24)

    private fun readBeUInt32(bytes: ByteArray, pos: Int): Long =
        ((bytes[pos].toLong() and 0xFFL) shl 24) or
            ((bytes[pos + 1].toLong() and 0xFFL) shl 16) or
            ((bytes[pos + 2].toLong() and 0xFFL) shl 8) or
            (bytes[pos + 3].toLong() and 0xFFL)

    /**
     * Parses FLAC `STREAMINFO`, `VORBIS_COMMENT`, and `PICTURE` metadata blocks from an initial byte slice.
     */
    fun parseFlacHeader(bytes: ByteArray): FlacHeaderMetadata? {
        if (bytes.size < 8) return null
        if (bytes[0] != 0x66.toByte() ||
            bytes[1] != 0x4C.toByte() ||
            bytes[2] != 0x61.toByte() ||
            bytes[3] != 0x43.toByte()
        ) {
            return null
        }

        var title: String? = null
        var artist: String? = null
        var albumArtist: String? = null
        var album: String? = null
        var durationSecs: Long? = null
        var exactDurationSecs: Double? = null
        var sampleRateHz = 0
        var channels = 0
        var bitsPerSample = 0
        var lyrics: String? = null
        var pictureMime: String? = null
        var pictureBytes: ByteArray? = null
        var hasPictureBlock = false

        var offset = 4
        while (offset + 4 <= bytes.size) {
            val header0 = bytes[offset].toInt() and 0xFF
            val isLast = (header0 and 0x80) != 0
            val blockType = header0 and 0x7F
            val blockLen = ((bytes[offset + 1].toInt() and 0xFF) shl 16) or
                ((bytes[offset + 2].toInt() and 0xFF) shl 8) or
                (bytes[offset + 3].toInt() and 0xFF)
            val blockStart = offset + 4
            if (blockType == 6) {
                hasPictureBlock = true
            }
            if (blockLen < 0 || blockStart + blockLen > bytes.size) {
                break
            }
            when (blockType) {
                0 -> if (blockLen >= 18) {
                    val b10 = bytes[blockStart + 10].toInt() and 0xFF
                    val b11 = bytes[blockStart + 11].toInt() and 0xFF
                    val b12 = bytes[blockStart + 12].toInt() and 0xFF
                    val b13 = bytes[blockStart + 13].toInt() and 0xFF
                    val b14 = bytes[blockStart + 14].toInt() and 0xFF
                    val b15 = bytes[blockStart + 15].toInt() and 0xFF
                    val b16 = bytes[blockStart + 16].toInt() and 0xFF
                    val b17 = bytes[blockStart + 17].toInt() and 0xFF
                    sampleRateHz = (b10 shl 12) or (b11 shl 4) or (b12 ushr 4)
                    channels = ((b12 and 0x0E) ushr 1) + 1
                    bitsPerSample = (((b12 and 0x01) shl 4) or (b13 ushr 4)) + 1
                    val totalSamples = ((b13.toLong() and 0x0FL) shl 32) or
                        (b14.toLong() shl 24) or
                        (b15.toLong() shl 16) or
                        (b16.toLong() shl 8) or
                        b17.toLong()
                    if (sampleRateHz > 0 && totalSamples > 0L) {
                        exactDurationSecs = totalSamples.toDouble() / sampleRateHz.toDouble()
                        durationSecs = totalSamples / sampleRateHz.toLong()
                    }
                }
                4 -> if (blockLen >= 8) {
                    var pos = blockStart
                    val end = blockStart + blockLen
                    val vendorLen = readLeUInt32(bytes, pos)
                    pos += 4
                    if (vendorLen >= 0L && pos + vendorLen <= end) {
                        pos += vendorLen.toInt()
                        if (pos + 4 <= end) {
                            val commentCount = readLeUInt32(bytes, pos).coerceAtMost(512L).toInt()
                            pos += 4
                            for (i in 0 until commentCount) {
                                if (pos + 4 > end) break
                                val commentLen = readLeUInt32(bytes, pos)
                                pos += 4
                                if (commentLen < 0L || pos + commentLen > end) break
                                val entryStr = String(bytes, pos, commentLen.toInt(), StandardCharsets.UTF_8)
                                pos += commentLen.toInt()
                                val eq = entryStr.indexOf('=')
                                if (eq <= 0) continue
                                val key = entryStr.substring(0, eq).trim().uppercase(Locale.ROOT)
                                val value = entryStr.substring(eq + 1).trim()
                                if (value.isEmpty()) continue
                                when (key) {
                                    "TITLE" -> if (title == null) title = value
                                    "ARTIST" -> if (artist == null) artist = value
                                    "ALBUMARTIST", "ALBUM_ARTIST", "ALBUM ARTIST" -> if (albumArtist == null) albumArtist = value
                                    "ALBUM" -> if (album == null) album = value
                                    "LYRICS", "UNSYNCEDLYRICS" -> if (lyrics == null) lyrics = value
                                }
                            }
                        }
                    }
                }
                6 -> if (blockLen >= 32 && pictureBytes == null) {
                    var pos = blockStart
                    val end = blockStart + blockLen
                    pos += 4 // pictureType
                    val mimeLen = readBeUInt32(bytes, pos)
                    pos += 4
                    if (mimeLen >= 0L && pos + mimeLen <= end) {
                        pictureMime = String(bytes, pos, mimeLen.toInt(), StandardCharsets.US_ASCII)
                        pos += mimeLen.toInt()
                        if (pos + 4 <= end) {
                            val descLen = readBeUInt32(bytes, pos)
                            pos += 4
                            if (descLen >= 0L && pos + descLen + 20 <= end) {
                                pos += descLen.toInt() + 16
                                val picLen = readBeUInt32(bytes, pos)
                                pos += 4
                                if (picLen in 1L..(end - pos).toLong()) {
                                    pictureBytes = bytes.copyOfRange(pos, pos + picLen.toInt())
                                }
                            }
                        }
                    }
                }
            }
            offset = blockStart + blockLen
            if (isLast) break
        }
        return FlacHeaderMetadata(
            title = title,
            artist = artist,
            albumArtist = albumArtist,
            album = album,
            durationSecs = durationSecs,
            exactDurationSecs = exactDurationSecs,
            sampleRateHz = sampleRateHz,
            channels = channels,
            bitsPerSample = bitsPerSample,
            lyrics = lyrics,
            pictureMime = pictureMime,
            pictureBytes = pictureBytes,
            hasPictureBlock = hasPictureBlock,
        )
    }

    private data class TtmlSpanChunk(
        val startSecs: Double,
        val endSecs: Double?,
        var text: String,
        val separatedFromPrev: Boolean,
    )

    private fun parseTtmlTimestampSecs(raw: String?): Double? {
        val trimmed = raw?.trim().orEmpty()
        if (trimmed.isEmpty()) return null
        if (trimmed.endsWith("ms")) {
            val millis = trimmed.removeSuffix("ms").trim().toDoubleOrNull() ?: return null
            return if (millis.isFinite() && millis >= 0.0) millis / 1000.0 else null
        }
        val clean = trimmed.removeSuffix("s").trim()
        val parts = clean.split(':')
        if (parts.isEmpty() || parts.size > 3) return null
        var seconds = 0.0
        for (part in parts) {
            val v = part.trim().toDoubleOrNull() ?: return null
            if (!v.isFinite() || v < 0.0) return null
            seconds = seconds * 60.0 + v
        }
        return if (seconds.isFinite()) seconds else null
    }

    private fun ttmlAttr(elem: Element, ns: String, prefixName: String, localName: String): String =
        elem.getAttributeNS(ns, localName).ifBlank {
            elem.getAttribute(prefixName).ifBlank { elem.getAttribute(localName) }
        }

    /**
     * Parses Apple-style TTML XML lyrics (including word-level karaoke spans, `x-bg` background
     * vocals, and counter-voice agents) into sorted [AndroidLyricLine] entries.
     */
    fun parseTtmlLyrics(xml: String): List<AndroidLyricLine> {
        val cleaned = xml.trim().removePrefix("\uFEFF").trim()
        if (cleaned.isEmpty() || !cleaned.startsWith("<")) return emptyList()

        val doc = runCatching {
            val factory = DocumentBuilderFactory.newInstance().apply {
                isNamespaceAware = true
                isExpandEntityReferences = false
                runCatching { setFeature("http://apache.org/xml/features/disallow-doctype-decl", true) }
                runCatching { setFeature("http://xml.org/sax/features/external-general-entities", false) }
                runCatching { setFeature("http://xml.org/sax/features/external-parameter-entities", false) }
                runCatching { setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true) }
            }
            factory.newDocumentBuilder().parse(InputSource(StringReader(cleaned)))
        }.getOrNull() ?: return emptyList()

        val ttmNs = "http://www.w3.org/ns/ttml#metadata"
        val xmlNs = "http://www.w3.org/XML/1998/namespace"
        val skipRoles = setOf("x-translation", "x-roman")

        val personAgents = HashMap<String, Boolean>()
        val agentNodes = doc.getElementsByTagNameNS("*", "agent")
        for (i in 0 until agentNodes.length) {
            val el = agentNodes.item(i) as? Element ?: continue
            val id = ttmlAttr(el, xmlNs, "xml:id", "id")
            if (id.isNotBlank()) {
                personAgents[id] = el.getAttribute("type") == "person"
            }
        }
        if (personAgents.isEmpty()) {
            val fallbackAgents = doc.getElementsByTagName("ttm:agent")
            for (i in 0 until fallbackAgents.length) {
                val el = fallbackAgents.item(i) as? Element ?: continue
                val id = ttmlAttr(el, xmlNs, "xml:id", "id")
                if (id.isNotBlank()) {
                    personAgents[id] = el.getAttribute("type") == "person"
                }
            }
        }

        val pNodes = ArrayList<Element>()
        val rawParagraphs = doc.getElementsByTagNameNS("*", "p")
        for (i in 0 until rawParagraphs.length) {
            (rawParagraphs.item(i) as? Element)?.let { pNodes += it }
        }
        if (pNodes.isEmpty()) {
            val fallbackParagraphs = doc.getElementsByTagName("p")
            for (i in 0 until fallbackParagraphs.length) {
                (fallbackParagraphs.item(i) as? Element)?.let { pNodes += it }
            }
        }
        if (pNodes.isEmpty()) return emptyList()

        val leadAgent = pNodes
            .map { ttmlAttr(it, ttmNs, "ttm:agent", "agent") }
            .firstOrNull { it.isNotBlank() && personAgents[it] == true }

        fun elemRole(el: Element): String = ttmlAttr(el, ttmNs, "ttm:role", "role")

        fun hasSkippedAncestor(node: Node): Boolean {
            var cur = node.parentNode
            while (cur != null) {
                if (cur is Element && elemRole(cur) in skipRoles) return true
                cur = cur.parentNode
            }
            return false
        }

        fun collectPlainText(node: Node, excludeBg: Boolean): String {
            val sb = StringBuilder()
            fun walk(cur: Node) {
                if (cur is Element) {
                    val r = elemRole(cur)
                    if ((excludeBg && r == "x-bg") || r in skipRoles) return
                }
                if (cur.nodeType == Node.TEXT_NODE || cur.nodeType == Node.CDATA_SECTION_NODE) {
                    sb.append(cur.nodeValue.orEmpty())
                }
                var ch = cur.firstChild
                while (ch != null) {
                    walk(ch)
                    ch = ch.nextSibling
                }
            }
            walk(node)
            return sb.toString().trim().replace(Regex("\\s+"), " ")
        }

        fun hasTimedChild(el: Element): Boolean {
            var ch = el.firstChild
            while (ch != null) {
                if (ch is Element) {
                    val r = elemRole(ch)
                    if (r != "x-bg" && r !in skipRoles) {
                        if (ch.hasAttribute("begin") && ch.getAttribute("begin").isNotBlank()) return true
                        if (hasTimedChild(ch)) return true
                    }
                }
                ch = ch.nextSibling
            }
            return false
        }

        fun mergeChunks(chunks: List<TtmlSpanChunk>): List<AndroidLyricWord> {
            val words = ArrayList<AndroidLyricWord>(chunks.size)
            for (chunk in chunks) {
                if (!chunk.separatedFromPrev && words.isNotEmpty()) {
                    val prev = words.removeAt(words.lastIndex)
                    val prevEnd = prev.endSecs
                    val chunkEnd = chunk.endSecs
                    val mergedEnd = when {
                        chunkEnd != null && prevEnd != null -> max(prevEnd, chunkEnd)
                        chunkEnd != null -> chunkEnd
                        else -> prevEnd
                    }
                    words += prev.copy(text = prev.text + chunk.text, endSecs = mergedEnd)
                } else {
                    words += AndroidLyricWord(
                        startSecs = chunk.startSecs,
                        endSecs = chunk.endSecs,
                        text = chunk.text,
                    )
                }
            }
            return words
        }

        fun collectSpans(
            parent: Node,
            chunks: MutableList<TtmlSpanChunk>,
            bgLines: MutableList<AndroidLyricLine>,
            pendingSpace: BooleanArray,
            isCounter: Boolean,
        ) {
            var ch = parent.firstChild
            while (ch != null) {
                when (ch.nodeType) {
                    Node.TEXT_NODE, Node.CDATA_SECTION_NODE -> {
                        val raw = ch.nodeValue.orEmpty()
                        if (raw.any { it.isWhitespace() }) {
                            pendingSpace[0] = true
                        }
                        val trimmed = raw.trim()
                        if (trimmed.isNotEmpty() && chunks.isNotEmpty()) {
                            val last = chunks.last()
                            if (pendingSpace[0] && last.text.lastOrNull()?.isWhitespace() != true) {
                                last.text += " "
                            }
                            last.text += trimmed
                            pendingSpace[0] = raw.lastOrNull()?.isWhitespace() == true
                        }
                    }
                    Node.ELEMENT_NODE -> {
                        val el = ch as Element
                        val role = elemRole(el)
                        when {
                            role == "x-bg" -> {
                                val bgChunks = ArrayList<TtmlSpanChunk>()
                                val ignored = ArrayList<AndroidLyricLine>()
                                val bgSpace = booleanArrayOf(true)
                                collectSpans(el, bgChunks, ignored, bgSpace, isCounter)
                                val bgWords = mergeChunks(bgChunks).toMutableList()
                                var bgText = if (bgWords.isEmpty()) {
                                    collectPlainText(el, excludeBg = false)
                                } else {
                                    bgWords.joinToString(" ") { it.text }
                                }.trim()
                                if (bgText.isNotEmpty()) {
                                    if (!(bgText.startsWith("(") && bgText.endsWith(")"))) {
                                        bgText = "($bgText)"
                                        if (bgWords.isNotEmpty()) {
                                            if (!bgWords[0].text.startsWith("(")) {
                                                bgWords[0] = bgWords[0].copy(text = "(${bgWords[0].text}")
                                            }
                                            val lastIdx = bgWords.lastIndex
                                            if (!bgWords[lastIdx].text.endsWith(")")) {
                                                bgWords[lastIdx] = bgWords[lastIdx].copy(text = "${bgWords[lastIdx].text})")
                                            }
                                        }
                                    }
                                    val bgStart = parseTtmlTimestampSecs(el.getAttribute("begin"))
                                        ?: bgWords.firstOrNull()?.startSecs
                                    if (bgStart != null) {
                                        bgLines += AndroidLyricLine(
                                            timeSecs = bgStart,
                                            text = bgText,
                                            words = bgWords,
                                            isCounterVoice = isCounter,
                                        )
                                    }
                                }
                            }
                            role in skipRoles -> Unit
                            el.getAttribute("begin").isNotBlank() && !hasTimedChild(el) -> {
                                val start = parseTtmlTimestampSecs(el.getAttribute("begin"))
                                if (start != null) {
                                    val end = parseTtmlTimestampSecs(el.getAttribute("end"))?.let { max(it, start) }
                                    val rawSpanText = buildString {
                                        fun textWalk(n: Node) {
                                            if (n is Element && elemRole(n) in skipRoles) return
                                            if ((n.nodeType == Node.TEXT_NODE || n.nodeType == Node.CDATA_SECTION_NODE) &&
                                                !hasSkippedAncestor(n)
                                            ) {
                                                append(n.nodeValue.orEmpty())
                                            }
                                            var inner = n.firstChild
                                            while (inner != null) {
                                                textWalk(inner)
                                                inner = inner.nextSibling
                                            }
                                        }
                                        textWalk(el)
                                    }
                                    val startsWs = rawSpanText.firstOrNull()?.isWhitespace() == true
                                    val endsWs = rawSpanText.lastOrNull()?.isWhitespace() == true
                                    val normalized = rawSpanText.trim().replace(Regex("\\s+"), " ")
                                    if (normalized.isNotEmpty()) {
                                        chunks += TtmlSpanChunk(
                                            startSecs = start,
                                            endSecs = end,
                                            text = normalized,
                                            separatedFromPrev = pendingSpace[0] || startsWs || chunks.isEmpty(),
                                        )
                                        pendingSpace[0] = endsWs
                                    } else if (endsWs || startsWs) {
                                        pendingSpace[0] = true
                                    }
                                }
                            }
                            else -> collectSpans(el, chunks, bgLines, pendingSpace, isCounter)
                        }
                    }
                }
                ch = ch.nextSibling
            }
        }

        val out = ArrayList<AndroidLyricLine>()
        for (p in pNodes) {
            val agent = ttmlAttr(p, ttmNs, "ttm:agent", "agent")
            val isCounter = agent.isNotBlank() && personAgents[agent] == true && agent != leadAgent
            val chunks = ArrayList<TtmlSpanChunk>()
            val bgLines = ArrayList<AndroidLyricLine>()
            collectSpans(p, chunks, bgLines, booleanArrayOf(true), isCounter)
            val words = mergeChunks(chunks)
            val text = if (words.isEmpty()) {
                collectPlainText(p, excludeBg = true)
            } else {
                words.joinToString(" ") { it.text }
            }
            var leadStart: Double? = null
            var leadEnd: Double? = null
            val pBegin = parseTtmlTimestampSecs(p.getAttribute("begin"))
            if (text.isNotEmpty() && (pBegin != null || words.isNotEmpty())) {
                val timeSecs = pBegin ?: words.firstOrNull()?.startSecs ?: 0.0
                leadStart = timeSecs
                val lastWord = words.lastOrNull()
                leadEnd = lastWord?.let { it.endSecs ?: (it.startSecs + 0.4) }
                    ?: parseTtmlTimestampSecs(p.getAttribute("end"))
                out += AndroidLyricLine(
                    timeSecs = timeSecs,
                    text = text,
                    words = words,
                    isCounterVoice = isCounter,
                )
            }
            if (leadStart != null && leadEnd != null) {
                for (bgIdx in bgLines.indices) {
                    val bg = bgLines[bgIdx]
                    if (bg.timeSecs < leadEnd && bg.timeSecs < leadStart + 0.05) {
                        bgLines[bgIdx] = bg.copy(timeSecs = min(leadStart + 0.05, leadEnd))
                    }
                }
            }
            out += bgLines
        }

        val hasTiming = out.any { it.timeSecs > 0.0 || it.words.isNotEmpty() }
        if (out.isEmpty() || !hasTiming) return emptyList()
        return splitInlineBackgroundVocals(out)
    }

    private fun splitWordSyncedLine(line: AndroidLyricLine, out: MutableList<AndroidLyricLine>): Boolean {
        if (line.words.size < 2) return false
        var curWords = line.words.toList()
        var curTime = line.timeSecs
        var splitAny = false

        if (curWords[0].text.trimStart().startsWith("(")) {
            var depth = 0
            var closeIdx = -1
            for (j in curWords.indices) {
                for (ch in curWords[j].text) {
                    if (ch == '(') depth += 1 else if (ch == ')') depth -= 1
                }
                if (depth == 0) {
                    if (j + 1 < curWords.size && curWords[j].text.trimEnd().endsWith(")")) {
                        closeIdx = j
                    }
                    break
                }
                if (depth < 0) break
            }
            if (closeIdx >= 0) {
                val bgWords = curWords.subList(0, closeIdx + 1).toList()
                val leadWords = curWords.subList(closeIdx + 1, curWords.size).toList()
                val bgText = bgWords.joinToString(" ") { it.text.trim() }
                val nextStart = max(leadWords[0].startSecs, curTime + 0.05)
                out += AndroidLyricLine(
                    timeSecs = curTime,
                    text = bgText,
                    words = bgWords,
                    isCounterVoice = line.isCounterVoice,
                )
                curWords = leadWords
                curTime = nextStart
                splitAny = true
            }
        }

        if (curWords.size >= 2 && curWords.last().text.trimEnd().endsWith(")")) {
            var depth = 0
            var openIdx = -1
            for (j in curWords.size - 1 downTo 0) {
                val text = curWords[j].text
                for (k in text.length - 1 downTo 0) {
                    val ch = text[k]
                    if (ch == ')') depth += 1 else if (ch == '(') depth -= 1
                }
                if (depth == 0) {
                    if (j > 0 && text.trimStart().startsWith("(")) {
                        openIdx = j
                    }
                    break
                }
                if (depth < 0) break
            }
            if (openIdx > 0) {
                val leadWords = curWords.subList(0, openIdx).toList()
                val bgWords = curWords.subList(openIdx, curWords.size).toList()
                val leadText = leadWords.joinToString(" ") { it.text.trim() }
                val bgText = bgWords.joinToString(" ") { it.text.trim() }
                val bgTime = max(bgWords[0].startSecs, curTime + 0.05)
                out += AndroidLyricLine(
                    timeSecs = curTime,
                    text = leadText,
                    words = leadWords,
                    isCounterVoice = line.isCounterVoice,
                )
                out += AndroidLyricLine(
                    timeSecs = bgTime,
                    text = bgText,
                    words = bgWords,
                    isCounterVoice = line.isCounterVoice,
                )
                return true
            }
        }

        if (splitAny) {
            out += AndroidLyricLine(
                timeSecs = curTime,
                text = curWords.joinToString(" ") { it.text.trim() },
                words = curWords,
                isCounterVoice = line.isCounterVoice,
            )
            return true
        }
        return false
    }

    /**
     * Splits leading and trailing parenthesized background vocals (e.g. `Lead text (ad-lib)` or
     * `(Ooh) Lead text`) from both word-synced and line-synced [AndroidLyricLine] entries.
     */
    fun splitInlineBackgroundVocals(lines: List<AndroidLyricLine>): List<AndroidLyricLine> {
        if (lines.isEmpty()) return lines
        val sorted = lines.sortedBy { it.timeSecs }
        val out = ArrayList<AndroidLyricLine>(sorted.size + 8)
        for (idx in sorted.indices) {
            val line = sorted[idx]
            if (line.words.isNotEmpty()) {
                if (!splitWordSyncedLine(line, out)) {
                    out += line
                }
                continue
            }
            var remaining = line.text.trim()
            var curTime = line.timeSecs
            val nextT = sorted.getOrNull(idx + 1)?.timeSecs
            val totalSpan = nextT?.takeIf { it > curTime + 0.25 }?.let { it - curTime }

            if (remaining.startsWith("(")) {
                var depth = 0
                var closeIdx = -1
                for (pos in remaining.indices) {
                    when (remaining[pos]) {
                        '(' -> depth += 1
                        ')' -> {
                            depth -= 1
                            if (depth == 0) {
                                closeIdx = pos
                                break
                            }
                        }
                    }
                }
                if (closeIdx > 0 && closeIdx + 1 < remaining.length && remaining[closeIdx + 1].isWhitespace()) {
                    val bgInner = remaining.substring(1, closeIdx).trim()
                    val rest = remaining.substring(closeIdx + 1).trim()
                    if (bgInner.isNotEmpty() && rest.isNotEmpty()) {
                        val leadOffset = if (totalSpan != null) {
                            val ratio = (bgInner.length.toDouble() / (bgInner.length + rest.length).coerceAtLeast(1))
                                .coerceIn(0.25, 0.50)
                            totalSpan * ratio
                        } else {
                            0.9
                        }
                        out += AndroidLyricLine(
                            timeSecs = curTime,
                            text = "($bgInner)",
                            words = emptyList(),
                            isCounterVoice = line.isCounterVoice,
                        )
                        curTime += leadOffset
                        remaining = rest
                    }
                }
            }

            if (remaining.endsWith(")")) {
                var depth = 0
                var openIdx = -1
                for (pos in remaining.length - 1 downTo 0) {
                    when (remaining[pos]) {
                        ')' -> depth += 1
                        '(' -> {
                            depth -= 1
                            if (depth == 0) {
                                openIdx = pos
                                break
                            }
                        }
                    }
                }
                if (openIdx > 0 && remaining[openIdx - 1].isWhitespace()) {
                    val lead = remaining.substring(0, openIdx).trim()
                    val inner = remaining.substring(openIdx + 1, remaining.length - 1).trim()
                    if (lead.isNotEmpty() && inner.isNotEmpty()) {
                        val remSpan = nextT?.takeIf { it > curTime + 0.25 }?.let { it - curTime }
                        val bgTime = if (remSpan != null) {
                            val ratio = (lead.length.toDouble() / (lead.length + inner.length).coerceAtLeast(1))
                                .coerceIn(0.45, 0.72)
                            curTime + remSpan * ratio
                        } else {
                            curTime + 1.2
                        }
                        out += AndroidLyricLine(
                            timeSecs = curTime,
                            text = lead,
                            words = emptyList(),
                            isCounterVoice = line.isCounterVoice,
                        )
                        out += AndroidLyricLine(
                            timeSecs = bgTime,
                            text = "($inner)",
                            words = emptyList(),
                            isCounterVoice = line.isCounterVoice,
                        )
                        continue
                    }
                }
            }

            out += AndroidLyricLine(
                timeSecs = curTime,
                text = remaining,
                words = emptyList(),
                isCounterVoice = line.isCounterVoice,
            )
        }
        return out.sortedBy { it.timeSecs }
    }

    /**
     * Parses TTML XML, LRC synced lyrics (including multi-timestamp lines, `[offset:ms]`, and
     * enhanced `<mm:ss.xx>` karaoke word timestamps), or plain unsynced lyrics text into sorted
     * [AndroidLyricLine] entries.
     */
    fun parseLyricsText(raw: String): List<AndroidLyricLine> {
        val cleaned = raw.trim().removePrefix("\uFEFF")
        if (cleaned.isEmpty()) return emptyList()
        if (cleaned.startsWith("<") && cleaned.contains("<tt")) {
            val ttmlLines = parseTtmlLyrics(cleaned)
            if (ttmlLines.isNotEmpty()) return ttmlLines
        }

        var offsetMs = 0.0
        val synced = ArrayList<AndroidLyricLine>()
        val plain = ArrayList<AndroidLyricLine>()

        fun parseTimestampSecs(ts: String): Double? {
            val match = LRC_TIMESTAMP_REGEX.matchEntire(ts.trim()) ?: return null
            val mins = match.groupValues[1].toLongOrNull() ?: return null
            val secs = match.groupValues[2].toLongOrNull() ?: return null
            val fracStr = match.groupValues.getOrNull(3).orEmpty()
            val fracSecs = if (fracStr.isEmpty()) {
                0.0
            } else {
                fracStr.toDouble() / 10.0.pow(fracStr.length.toDouble())
            }
            return mins * 60.0 + secs + fracSecs
        }

        for (rawLine in cleaned.lineSequence()) {
            val line = rawLine.trim()
            if (line.isEmpty()) continue
            val tags = LRC_TAG_REGEX.findAll(line).toList()
            if (tags.isNotEmpty()) {
                var isMetaLine = false
                val lineTimes = ArrayList<Double>()
                for (tag in tags) {
                    val inner = tag.groupValues[1].trim()
                    if (inner.startsWith("offset:", ignoreCase = true)) {
                        offsetMs = inner.substringAfter(':').trim().toDoubleOrNull() ?: offsetMs
                        isMetaLine = true
                    } else {
                        val parsedTime = parseTimestampSecs(inner)
                        if (parsedTime != null) {
                            lineTimes += parsedTime
                        } else if (inner.contains(':') &&
                            inner.substringBefore(':').lowercase(Locale.ROOT) in setOf("ar", "ti", "al", "au", "by", "length", "re", "tool", "ve")
                        ) {
                            isMetaLine = true
                        }
                    }
                }
                if (lineTimes.isNotEmpty()) {
                    var body = LRC_TAG_REGEX.replace(line, "").trim()
                    val wordMatches = LRC_WORD_REGEX.findAll(body).toList()
                    val words = if (wordMatches.isNotEmpty()) {
                        val parsedWords = ArrayList<AndroidLyricWord>(wordMatches.size)
                        for (idx in wordMatches.indices) {
                            val wm = wordMatches[idx]
                            val start = ((parseTimestampSecs(wm.groupValues[1]) ?: 0.0) - offsetMs / 1000.0)
                                .coerceAtLeast(0.0)
                            val end = wordMatches.getOrNull(idx + 1)?.let { nextWm ->
                                ((parseTimestampSecs(nextWm.groupValues[1]) ?: start) - offsetMs / 1000.0)
                                    .coerceAtLeast(start)
                            }
                            val textEnd = wordMatches.getOrNull(idx + 1)?.range?.first ?: body.length
                            val token = body.substring(wm.range.last + 1, textEnd)
                            if (token.isNotBlank()) {
                                parsedWords += AndroidLyricWord(
                                    startSecs = start,
                                    endSecs = end,
                                    text = token,
                                )
                            }
                        }
                        body = LRC_WORD_REGEX.replace(body, "").replace(Regex("\\s{2,}"), " ").trim()
                        parsedWords
                    } else {
                        emptyList()
                    }
                    if (body.isNotEmpty()) {
                        for (t in lineTimes) {
                            val adjusted = max(0.0, t - offsetMs / 1000.0)
                            synced += AndroidLyricLine(
                                timeSecs = adjusted,
                                text = body,
                                words = words,
                                isCounterVoice = false,
                            )
                        }
                    }
                    continue
                }
                if (isMetaLine && LRC_TAG_REGEX.replace(line, "").trim().isEmpty()) {
                    continue
                }
            }
            plain += AndroidLyricLine(
                timeSecs = 0.0,
                text = line,
                words = emptyList(),
                isCounterVoice = false,
            )
        }

        return if (synced.isNotEmpty()) {
            splitInlineBackgroundVocals(synced)
        } else {
            plain
        }
    }

    /**
     * Fetches synced `.ttml` / `.lrc` / `.txt` sidecar lyrics from the track's SMB folder or
     * embedded FLAC Vorbis tags.
     */
    suspend fun fetchLyrics(profile: AndroidServerProfile, relativePath: String): List<AndroidLyricLine> =
        withContext(Dispatchers.IO) {
            val cleanRel = relativePath.trim('/', '\\').replace('\\', '/')
            if (cleanRel.isEmpty()) return@withContext emptyList()
            runCatching {
                withShare(profile) { endpoint, diskShare ->
                    val folderRel = cleanRel.substringBeforeLast('/', "").takeIf { '/' in cleanRel }.orEmpty()
                    val fileName = cleanRel.substringAfterLast('/')
                    val fileStem = fileName.substringBeforeLast('.')
                    val cleanStem = cleanTrackTitle(fileName)
                    val winDir = toSmbWinPath(endpoint.baseSubpath, folderRel)
                    val entries = diskShare.list(winDir)

                    val lyricFiles = entries.mapNotNull { entry ->
                        val name = entry.fileName ?: return@mapNotNull null
                        val isDir = (entry.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
                        if (isDir || name.startsWith(".")) return@mapNotNull null
                        val ext = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
                        if (ext == "ttml" || ext == "lrc" || ext == "txt") name else null
                    }
                    val audioCount = entries.count { entry ->
                        val name = entry.fileName ?: return@count false
                        val isDir = (entry.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
                        !isDir && isSupportedAudioFile(name)
                    }

                    val chosenLyricFile = lyricFiles.firstOrNull {
                        it.substringBeforeLast('.').equals(fileStem, ignoreCase = true) &&
                            it.endsWith(".ttml", ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.substringBeforeLast('.').equals(cleanStem, ignoreCase = true) &&
                            it.endsWith(".ttml", ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.substringBeforeLast('.').equals(fileStem, ignoreCase = true) &&
                            it.endsWith(".lrc", ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.substringBeforeLast('.').equals(cleanStem, ignoreCase = true) &&
                            it.endsWith(".lrc", ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.substringBeforeLast('.').equals(fileStem, ignoreCase = true) ||
                            it.substringBeforeLast('.').equals(cleanStem, ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.equals("lyrics.ttml", ignoreCase = true)
                    } ?: lyricFiles.firstOrNull {
                        it.equals("lyrics.lrc", ignoreCase = true)
                    } ?: lyricFiles.singleOrNull { it.endsWith(".ttml", ignoreCase = true) && audioCount == 1 }
                        ?: lyricFiles.singleOrNull { it.endsWith(".lrc", ignoreCase = true) && audioCount == 1 }

                    if (chosenLyricFile != null) {
                        val lyricRel = if (folderRel.isBlank()) chosenLyricFile else "$folderRel/$chosenLyricFile"
                        val rawBytes = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, lyricRel), MAX_LYRICS_BYTES)
                        val parsed = parseLyricsText(String(rawBytes, StandardCharsets.UTF_8))
                        if (parsed.isNotEmpty()) return@withShare parsed
                    }

                    if (fileName.endsWith(".flac", ignoreCase = true)) {
                        val prefix = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, cleanRel), FLAC_HEADER_READ_BYTES)
                        val flacMeta = parseFlacHeader(prefix)
                        val embeddedLyrics = flacMeta?.lyrics?.let { parseLyricsText(it) }.orEmpty()
                        if (embeddedLyrics.isNotEmpty()) return@withShare embeddedLyrics
                    }

                    emptyList()
                }
            }.getOrDefault(emptyList())
        }

    private fun readFileBytes(
        diskShare: DiskShare,
        winPath: String,
        maxBytes: Int,
    ): ByteArray {
        val smbFile = diskShare.openFile(
            winPath,
            EnumSet.of(AccessMask.GENERIC_READ),
            null,
            SMB2ShareAccess.ALL,
            SMB2CreateDisposition.FILE_OPEN,
            null,
        )
        try {
            val out = ByteArrayOutputStream(min(maxBytes, 16_384))
            val buf = ByteArray(32_768)
            var offset = 0L
            while (offset < maxBytes) {
                val toRead = min(buf.size, maxBytes - offset.toInt())
                val n = smbFile.read(buf, offset, 0, toRead)
                if (n <= 0) break
                out.write(buf, 0, n)
                offset += n
            }
            return out.toByteArray()
        } finally {
            runCatching { smbFile.close() }
        }
    }

    private fun cacheArtworkBytes(cacheKey: String, ext: String, bytes: ByteArray): String? {
        if (bytes.isEmpty()) return null
        val dir = smbArtCacheDir ?: return null
        runCatching { if (!dir.exists()) dir.mkdirs() }
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(cacheKey.toByteArray(StandardCharsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
        val safeExt = ext.lowercase(Locale.ROOT).takeIf { it in SUPPORTED_IMAGE_EXTS } ?: "jpg"
        val outFile = File(dir, "$digest.$safeExt")
        if (!outFile.isFile || outFile.length() == 0L) {
            runCatching {
                val tmp = File(dir, "$digest.$safeExt.tmp")
                tmp.writeBytes(bytes)
                if (!tmp.renameTo(outFile)) {
                    outFile.writeBytes(bytes)
                    tmp.delete()
                }
            }.getOrElse { return null }
        }
        return outFile.toURI().toString()
    }

    private fun cachedArtworkUriIfExists(cacheKey: String, ext: String): String? {
        val dir = smbArtCacheDir ?: return null
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(cacheKey.toByteArray(StandardCharsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
        val safeExt = ext.lowercase(Locale.ROOT).takeIf { it in SUPPORTED_IMAGE_EXTS } ?: "jpg"
        val outFile = File(dir, "$digest.$safeExt")
        return if (outFile.isFile && outFile.length() > 0L) outFile.toURI().toString() else null
    }

    private fun listFolderInternal(
        profile: AndroidServerProfile,
        endpoint: SmbEndpoint,
        diskShare: DiskShare,
        relDir: String,
    ): SmbDirectoryListing {
        val cleanRel = relDir.trim('/', '\\').replace('\\', '/')
        val winPath = toSmbWinPath(endpoint.baseSubpath, cleanRel)
        val entries = diskShare.list(winPath)
        val folders = ArrayList<AndroidServerLibrary>()
        val tracks = ArrayList<AndroidTrack>()

        val sorted = entries
            .filter {
                val name = it.fileName
                !name.isNullOrBlank() && name != "." && name != ".." && !name.startsWith(".")
            }
            .sortedBy { it.fileName.lowercase(Locale.ROOT) }

        val dirNames = ArrayList<String>()
        val audioFileNames = ArrayList<String>()
        val audioFileSizes = HashMap<String, Long>()
        val imageFileNames = ArrayList<String>()
        var jsonMetaName: String? = null

        for (entry in sorted) {
            val name = entry.fileName
            val isDir = (entry.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
            if (isDir) {
                dirNames += name
            } else {
                val ext = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
                when {
                    isSupportedAudioFile(name) -> {
                        audioFileNames += name
                        if (entry.endOfFile > 0L) {
                            audioFileSizes[name] = entry.endOfFile
                        }
                    }
                    ext in SUPPORTED_IMAGE_EXTS -> imageFileNames += name
                    name.equals("metadata.json", ignoreCase = true) || name.equals("album.json", ignoreCase = true) -> {
                        if (jsonMetaName == null) jsonMetaName = name
                    }
                }
            }
        }

        for (name in dirNames) {
            val childRel = if (cleanRel.isBlank()) name else "$cleanRel/$name"
            folders += AndroidServerLibrary(id = childRel, name = name, kind = "folder")
        }

        val folderMeta = jsonMetaName?.let { metaName ->
            runCatching {
                val metaRel = if (cleanRel.isBlank()) metaName else "$cleanRel/$metaName"
                val raw = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, metaRel), 64 * 1024)
                val obj = JSONObject(String(raw, StandardCharsets.UTF_8))
                FolderSidecarMetadata(
                    artist = obj.optString("albumArtist").ifBlank { obj.optString("artist") }.ifBlank { null },
                    album = obj.optString("album").ifBlank { null },
                )
            }.getOrNull()
        }

        val folderCoverName = COVER_PRIORITY_BASENAMES.firstNotNullOfOrNull { targetBase ->
            imageFileNames.firstOrNull { it.substringBeforeLast('.').equals(targetBase, ignoreCase = true) }
        } ?: imageFileNames.firstOrNull()

        val imageCacheByName = HashMap<String, String>()
        fun resolveImageUri(imgName: String): String? {
            imageCacheByName[imgName]?.let { return it }
            val ext = imgName.substringAfterLast('.', "jpg").lowercase(Locale.ROOT)
            val cacheKey = "${profile.id}|$winPath|$imgName"
            val uri = cachedArtworkUriIfExists(cacheKey, ext)
                ?: runCatching {
                    val imgRel = if (cleanRel.isBlank()) imgName else "$cleanRel/$imgName"
                    val imgBytes = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, imgRel), MAX_ART_BYTES)
                    cacheArtworkBytes(cacheKey, ext, imgBytes)
                }.getOrNull()
            if (uri != null) {
                imageCacheByName[imgName] = uri
            }
            return uri
        }

        val defaultFolderCoverUri = folderCoverName?.let { resolveImageUri(it) }
        val (inferredArtist, inferredAlbum) = inferArtistAndAlbum(cleanRel)
        var flacReads = 0

        for (name in audioFileNames) {
            val childRel = if (cleanRel.isBlank()) name else "$cleanRel/$name"
            val stem = name.substringBeforeLast('.')
            val perTrackImage = imageFileNames.firstOrNull {
                it.substringBeforeLast('.').equals(stem, ignoreCase = true)
            }
            var thumbnailUri = perTrackImage?.let { resolveImageUri(it) } ?: defaultFolderCoverUri

            var flacMeta: FlacHeaderMetadata? = null
            if (name.endsWith(".flac", ignoreCase = true) && flacReads < MAX_FLAC_TAG_READS_PER_FOLDER) {
                flacReads += 1
                flacMeta = runCatching {
                    val headerBytes = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, childRel), FLAC_HEADER_READ_BYTES)
                    val initial = parseFlacHeader(headerBytes)
                    if (thumbnailUri == null && initial?.hasPictureBlock == true && initial.pictureBytes == null) {
                        val bigger = readFileBytes(diskShare, toSmbWinPath(endpoint.baseSubpath, childRel), FLAC_PICTURE_READ_BYTES)
                        parseFlacHeader(bigger) ?: initial
                    } else {
                        initial
                    }
                }.getOrNull()
            }

            if (thumbnailUri == null && flacMeta?.pictureBytes != null) {
                val picExt = when (flacMeta.pictureMime?.lowercase(Locale.ROOT)) {
                    "image/png" -> "png"
                    "image/webp" -> "webp"
                    else -> "jpg"
                }
                thumbnailUri = cacheArtworkBytes("${profile.id}|$childRel|embedded", picExt, flacMeta.pictureBytes)
            }

            val resolvedTitle = flacMeta?.title?.takeIf { it.isNotBlank() } ?: cleanTrackTitle(name)
            val resolvedArtist = flacMeta?.artist?.takeIf { it.isNotBlank() }
                ?: flacMeta?.albumArtist?.takeIf { it.isNotBlank() }
                ?: folderMeta?.artist?.takeIf { it.isNotBlank() }
                ?: inferredArtist
            val resolvedAlbum = flacMeta?.album?.takeIf { it.isNotBlank() }
                ?: folderMeta?.album?.takeIf { it.isNotBlank() }
                ?: inferredAlbum

            val fileSizeBytes = audioFileSizes[name] ?: 0L
            if (fileSizeBytes > 0L) {
                val durSecs = flacMeta?.exactDurationSecs
                    ?: flacMeta?.durationSecs?.takeIf { it > 0L }?.toDouble()
                    ?: 0.0
                val kbps = if (durSecs > 0.0) {
                    computeMeasuredBitrateKbps(fileSizeBytes, (durSecs * 1000.0).toLong())
                } else {
                    0
                }
                val codec = initialBadgeForPath(name)
                if (codec.isNotEmpty() && (kbps > 0 || (flacMeta?.sampleRateHz ?: 0) > 0)) {
                    val preBadge = formatMeasuredAudioBadge(
                        codec = codec,
                        bitrateKbps = kbps,
                        sampleRateHz = flacMeta?.sampleRateHz ?: 0,
                        bitDepthBits = flacMeta?.bitsPerSample ?: 0,
                    )
                    if (preBadge.isNotBlank()) {
                        measuredTrackBadges["${profile.id}|$childRel"] = preBadge
                    }
                }
            }

            tracks += AndroidTrack(
                id = "smb:${profile.id}:$childRel",
                title = resolvedTitle,
                artist = resolvedArtist,
                album = resolvedAlbum,
                durationSecs = flacMeta?.durationSecs?.takeIf { it > 0L }?.toULong(),
                thumbnailUrl = thumbnailUri,
                liked = false,
                albumId = if (cleanRel.isNotBlank()) "smb:${profile.id}:dir:$cleanRel" else null,
                isExplicit = false,
            )
        }
        return SmbDirectoryListing(folders = folders, tracks = tracks)
    }

    private fun isSupportedAudioFile(filename: String): Boolean {
        val ext = filename.substringAfterLast('.', "").lowercase(Locale.ROOT)
        return ext in SUPPORTED_AUDIO_EXTS
    }

    private fun cleanTrackTitle(filename: String): String {
        val noExt = filename.substringBeforeLast('.')
        return noExt.replace(Regex("^\\d{1,3}\\s*[-.]\\s*"), "").trim().ifBlank { noExt }
    }

    /**
     * Infers `(Artist, Album)` from an SMB folder path while skipping generic share/root folders
     * (e.g. `Music/Eminem/MMLP` -> `("Eminem", "MMLP")`, never falling back to a server name).
     */
    internal fun inferArtistAndAlbum(relDir: String): Pair<String, String> {
        val parts = relDir.split('/', '\\')
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .dropWhile { it.lowercase(Locale.ROOT) in GENERIC_CONTAINER_SEGMENTS }
            .toMutableList()
        if (parts.size >= 3 && DISC_FOLDER_REGEX.matches(parts.last())) {
            parts.removeAt(parts.lastIndex)
        }
        return when {
            parts.size >= 2 -> parts[parts.size - 2] to parts.last()
            parts.size == 1 -> "Unknown Artist" to parts.first()
            else -> "Unknown Artist" to "Unknown Album"
        }
    }

    internal fun toSmbWinPath(baseSubpath: String, relPath: String): String {
        val parts = (baseSubpath.split('/', '\\') + relPath.split('/', '\\'))
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        return parts.joinToString("\\")
    }

    /**
     * Opens an SMB file stream handle for Media3 [SmbDataSource].
     */
    internal fun openReadSession(profile: AndroidServerProfile, relativePath: String): OpenSmbFileHandle {
        val endpoint = parseEndpoint(profile).getOrThrow()
        val client = SMBClient(newConfig())
        var connection: Connection? = null
        var session: Session? = null
        var diskShare: DiskShare? = null
        var smbFile: SmbFile? = null
        try {
            connection = client.connect(endpoint.host, endpoint.port)
            val domain = endpoint.domain.ifBlank { null }
            val authCandidates = if (endpoint.username.isBlank() && endpoint.password.isBlank()) {
                listOf(AuthenticationContext.anonymous(), AuthenticationContext.guest())
            } else {
                listOf(AuthenticationContext(endpoint.username, endpoint.password.toCharArray(), domain))
            }
            var lastAuthErr: Throwable? = null
            for (ctx in authCandidates) {
                try {
                    session = connection.authenticate(ctx)
                    lastAuthErr = null
                    break
                } catch (err: Throwable) {
                    lastAuthErr = err
                }
            }
            val activeSession = session ?: throw (lastAuthErr ?: IOException("SMB authentication failed"))
            diskShare = activeSession.connectShare(endpoint.share) as? DiskShare
                ?: throw IOException("SMB share '${endpoint.share}' is not a disk share")
            val winPath = toSmbWinPath(endpoint.baseSubpath, relativePath)
            smbFile = diskShare.openFile(
                winPath,
                EnumSet.of(AccessMask.GENERIC_READ),
                null,
                SMB2ShareAccess.ALL,
                SMB2CreateDisposition.FILE_OPEN,
                null,
            )
            val totalBytes = smbFile.fileInformation.standardInformation.endOfFile
            return OpenSmbFileHandle(
                client = client,
                connection = connection,
                session = session,
                share = diskShare,
                file = smbFile,
                length = totalBytes,
            )
        } catch (err: Throwable) {
            runCatching { smbFile?.close() }
            runCatching { diskShare?.close() }
            runCatching { session?.close() }
            runCatching { connection?.close() }
            runCatching { client.close() }
            throw err
        }
    }

    internal data class OpenSmbFileHandle(
        val client: SMBClient,
        val connection: Connection,
        val session: Session,
        val share: DiskShare,
        val file: SmbFile,
        val length: Long,
    ) : AutoCloseable {
        override fun close() {
            runCatching { file.close() }
            runCatching { share.close() }
            runCatching { session.close() }
            runCatching { connection.close() }
            runCatching { client.close() }
        }
    }
}

/**
 * Seekable Media3 [DataSource] streaming audio directly from SMB2/SMB3 shares via `sonata-smb://`.
 */
@OptIn(UnstableApi::class)
class SmbDataSource : BaseDataSource(/* isNetwork = */ true) {
    private var currentUri: Uri? = null
    private var handle: SmbClient.OpenSmbFileHandle? = null
    private var totalLength: Long = C.LENGTH_UNSET.toLong()
    private var position: Long = 0L
    private var bytesRemaining: Long = C.LENGTH_UNSET.toLong()
    private var opened: Boolean = false

    override fun open(dataSpec: DataSpec): Long {
        val uri = dataSpec.uri
        currentUri = uri
        val (serverId, relPath) = SmbClient.parseStreamUri(uri)
            ?: throw IOException("Invalid SMB stream URI: $uri")
        val profile = SmbProfileRegistry.get(serverId)
            ?: throw IOException("SMB server '$serverId' is not configured")

        transferInitializing(dataSpec)
        val openHandle = try {
            SmbClient.openReadSession(profile, relPath)
        } catch (err: IOException) {
            throw err
        } catch (err: Throwable) {
            throw IOException("Failed to open SMB file: ${err.message}", err)
        }

        handle = openHandle
        val totalLen = openHandle.length
        totalLength = totalLen
        if (totalLen > 0L) {
            PlaybackService.recordMeasuredContentBytes(uri.toString(), totalLen)
        }
        if (dataSpec.position > totalLen) {
            close()
            throw EOFException("Requested offset ${dataSpec.position} beyond EOF $totalLen")
        }
        position = dataSpec.position
        bytesRemaining = if (dataSpec.length != C.LENGTH_UNSET.toLong()) {
            dataSpec.length
        } else {
            (totalLen - position).coerceAtLeast(0L)
        }
        opened = true
        transferStarted(dataSpec)
        return bytesRemaining
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (length == 0) return 0
        if (bytesRemaining == 0L) return C.RESULT_END_OF_INPUT
        val activeFile = handle?.file ?: return C.RESULT_END_OF_INPUT

        val toRead = if (bytesRemaining == C.LENGTH_UNSET.toLong()) {
            length
        } else {
            min(length.toLong(), bytesRemaining).toInt()
        }
        if (toRead <= 0) return C.RESULT_END_OF_INPUT

        val readBytes = try {
            activeFile.read(buffer, position, offset, toRead)
        } catch (err: Throwable) {
            throw IOException("SMB read error at $position: ${err.message}", err)
        }
        if (readBytes <= 0) {
            return C.RESULT_END_OF_INPUT
        }
        position += readBytes
        if (bytesRemaining != C.LENGTH_UNSET.toLong()) {
            bytesRemaining = (bytesRemaining - readBytes).coerceAtLeast(0L)
        }
        bytesTransferred(readBytes)
        return readBytes
    }

    override fun getUri(): Uri? = currentUri

    override fun getResponseHeaders(): Map<String, List<String>> =
        if (totalLength > 0L) mapOf("Content-Length" to listOf(totalLength.toString())) else emptyMap()

    override fun close() {
        currentUri = null
        totalLength = C.LENGTH_UNSET.toLong()
        try {
            handle?.close()
        } finally {
            handle = null
            if (opened) {
                opened = false
                transferEnded()
            }
        }
    }
}

/**
 * [DataSource.Factory] creating [SmbDataSource] instances for `sonata-smb://` URIs.
 */
@OptIn(UnstableApi::class)
class SmbDataSourceFactory : DataSource.Factory {
    override fun createDataSource(): DataSource = SmbDataSource()
}

/**
 * Dispatcher [DataSource.Factory] routing `sonata-smb://` URIs to [smbFactory] and all other URIs
 * (HTTP/HTTPS/file/content) to [fallbackFactory] so EXO/Media3 caches HTTP streams without interfering with SMB.
 */
@OptIn(UnstableApi::class)
class SmbAwareDataSourceFactory(
    private val smbFactory: DataSource.Factory,
    private val fallbackFactory: DataSource.Factory,
) : DataSource.Factory {
    override fun createDataSource(): DataSource = object : DataSource {
        private var activeDelegate: DataSource? = null

        override fun addTransferListener(transferListener: androidx.media3.datasource.TransferListener) {
            smbFactory.also { /* listeners attached per open below if needed */ }
            listeners.add(transferListener)
        }

        private val listeners = mutableListOf<androidx.media3.datasource.TransferListener>()

        override fun open(dataSpec: DataSpec): Long {
            val isSmb = dataSpec.uri.scheme.equals(SmbClient.STREAM_SCHEME, ignoreCase = true)
            val chosen = if (isSmb) smbFactory.createDataSource() else fallbackFactory.createDataSource()
            listeners.forEach { chosen.addTransferListener(it) }
            activeDelegate = chosen
            return chosen.open(dataSpec)
        }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int =
            activeDelegate?.read(buffer, offset, length) ?: C.RESULT_END_OF_INPUT

        override fun getUri(): Uri? = activeDelegate?.uri

        override fun getResponseHeaders(): Map<String, List<String>> =
            activeDelegate?.responseHeaders ?: emptyMap()

        override fun close() {
            try {
                activeDelegate?.close()
            } finally {
                activeDelegate = null
            }
        }
    }
}
