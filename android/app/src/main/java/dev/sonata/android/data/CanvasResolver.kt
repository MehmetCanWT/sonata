package dev.sonata.android.data

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.text.Normalizer
import java.util.Base64
import java.util.Locale
import java.util.concurrent.TimeUnit

/**
 * Resolved animated album artwork (looping video clip) for a track.
 */
data class CanvasMedia(
    val url: String,
    val fallbackUrl: String? = null,
    val portraitUrl: String? = null,
    val isHls: Boolean = false,
    val source: String,
)

/**
 * Multi-provider animated album artwork resolver with bounded in-memory LRU caching
 * (including negative caching) and strict normalized title/artist validation.
 *
 * Resolution order:
 * 1. Apple Music catalog `editorialVideo` (square & 3:4 portrait HLS `.m3u8`).
 * 2. Tidal public search `album.videoCover` (`1280x1280.mp4`).
 * 3. Community canvas JSON manifest fallback (30-minute TTL).
 */
object CanvasResolver {

    private const val MAX_CACHE_ENTRIES = 64
    private const val CONNECT_TIMEOUT_MS = 5_000
    private const val READ_TIMEOUT_MS = 7_000
    private const val DESKTOP_USER_AGENT =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
            "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"

    private const val APPLE_BROWSE_URL = "https://music.apple.com/us/browse"
    private const val APPLE_AMP_BASE = "https://amp-api.music.apple.com/v1/catalog"
    private const val APPLE_TOKEN_RETRY_BACKOFF_MS = 30L * 60L * 1000L

    private const val TIDAL_SEARCH_URL = "https://api.tidal.com/v1/search"
    private const val TIDAL_EMBED_TOKEN = "vNVdglQOjFJJGG2U"

    private const val COMMUNITY_MANIFEST_URL = "https://vivimusicanvas.mkmdevilmi.workers.dev/canvas.json"
    private const val COMMUNITY_TTL_MS = 30L * 60L * 1000L

    private data class CacheEntry(
        val media: CanvasMedia?,
        val resolvedWithAlbum: Boolean,
    )

    private val resolveMutex = Mutex()
    private val cache = object : LinkedHashMap<String, CacheEntry>(MAX_CACHE_ENTRIES, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, CacheEntry>): Boolean =
            size > MAX_CACHE_ENTRIES
    }

    // Apple anonymous web-player JWT state
    @Volatile
    private var appleJwt: String? = null
    @Volatile
    private var appleJwtExpiryMs: Long = 0L
    @Volatile
    private var appleJwtRetryAfterMs: Long = 0L
    private val rejectedAppleJwts = HashSet<String>()

    // Community manifest cache
    private data class CommunityEntry(
        val song: String,
        val artist: String,
        val album: String,
        val url: String,
    )

    @Volatile
    private var communityEntries: List<CommunityEntry> = emptyList()
    @Volatile
    private var communityFetchedAtMs: Long = 0L

    private val storefront: String by lazy {
        Locale.getDefault().country
            .takeIf { it.length == 2 }
            ?.lowercase(Locale.ROOT)
            ?: "us"
    }

    private val tidalCountry: String by lazy {
        Locale.getDefault().country
            .takeIf { it.length == 2 }
            ?.uppercase(Locale.ROOT)
            ?: "US"
    }

    /**
     * Returns an already-cached [CanvasMedia] for [videoId] without hitting the network.
     */
    fun cached(videoId: String): CanvasMedia? {
        if (videoId.isBlank()) return null
        return synchronized(cache) { cache[videoId]?.media }
    }

    /**
     * Resolves an animated artwork clip for the given track metadata.
     * Never throws; returns `null` when no verified clip is found.
     */
    suspend fun resolve(
        videoId: String,
        title: String,
        artist: String,
        album: String? = null,
    ): CanvasMedia? {
        val cleanTitle = cleanTitlePackaging(title)
        val cleanArtist = cleanArtistPackaging(artist)
        val cleanAlbum = album?.let(::cleanTitlePackaging)?.takeIf { it.isNotBlank() }
        if (cleanTitle.isBlank() || cleanArtist.isBlank()) return null

        val cacheKey = videoId.ifBlank { "${normalizeText(cleanTitle)}|${normalizeText(cleanArtist)}" }
        val hasAlbum = !cleanAlbum.isNullOrBlank()

        return resolveMutex.withLock {
            synchronized(cache) {
                val existing = cache[cacheKey]
                if (existing != null && (existing.media != null || existing.resolvedWithAlbum || !hasAlbum)) {
                    Log.d(
                        "SonataCanvas",
                        "resolve cache hit id=$videoId title='$cleanTitle' source=${existing.media?.source ?: "none"} url=${existing.media?.url}",
                    )
                    return@withLock existing.media
                }
            }

            Log.d(
                "SonataCanvas",
                "resolve start id=$videoId title='$cleanTitle' artist='$cleanArtist' album=${cleanAlbum ?: "<none>"}",
            )
            val found = withContext(Dispatchers.IO) {
                runCatching { searchAppleMusic(cleanTitle, cleanArtist, cleanAlbum) }
                    .onFailure { Log.w("SonataCanvas", "AppleMusic search error: ${it.message}") }
                    .getOrNull()
                    ?: runCatching { searchTidal(cleanTitle, cleanArtist) }
                        .onFailure { Log.w("SonataCanvas", "Tidal search error: ${it.message}") }
                        .getOrNull()
                    ?: runCatching { searchCommunity(cleanTitle, cleanArtist, cleanAlbum) }
                        .onFailure { Log.w("SonataCanvas", "Community search error: ${it.message}") }
                        .getOrNull()
            }

            Log.d(
                "SonataCanvas",
                "resolve result id=$videoId title='$cleanTitle' hit=${found != null} source=${found?.source ?: "none"} url=${found?.url}",
            )
            synchronized(cache) {
                cache[cacheKey] = CacheEntry(media = found, resolvedWithAlbum = hasAlbum)
            }
            found
        }
    }

    // -------------------------------------------------------------------------
    // Provider 1: Apple Music Editorial Video (HLS)
    // -------------------------------------------------------------------------

    private fun searchAppleMusic(title: String, artist: String, album: String?): CanvasMedia? {
        val jwt = obtainAppleJwt() ?: return null
        val primaryArtist = splitArtists(artist).firstOrNull().orEmpty()
        val term = buildString {
            if (primaryArtist.isNotBlank() && !normalizeText(title).contains(primaryArtist)) {
                append(primaryArtist).append(' ')
            }
            append(title)
            if (!album.isNullOrBlank() && !title.contains(album, ignoreCase = true)) {
                append(' ').append(album)
            }
        }

        val queryUrl = "$APPLE_AMP_BASE/$storefront/search" +
            "?term=${encode(term)}&types=songs&limit=10&extend=editorialVideo&include=albums"
        val body = appleGet(queryUrl, jwt) ?: return null
        val root = JSONObject(body)
        val songs = root.optJSONObject("results")
            ?.optJSONObject("songs")
            ?.optJSONArray("data")

        if (songs != null) {
            val ranked = buildList {
                for (i in 0 until songs.length()) {
                    val song = songs.optJSONObject(i) ?: continue
                    val score = scoreAppleCandidate(song, title, artist, album) ?: continue
                    if (score >= 12) add(score to song)
                }
            }.sortedByDescending { it.first }

            val triedAlbumIds = HashSet<String>()
            for ((_, song) in ranked) {
                val attrs = song.optJSONObject("attributes") ?: continue

                // 1a. Inline song editorialVideo
                attrs.optJSONObject("editorialVideo")?.let { video ->
                    buildAppleMedia(video)?.let { return it }
                }

                // 1b. Expanded album relationship editorialVideo
                val relAlbum = song.optJSONObject("relationships")
                    ?.optJSONObject("albums")
                    ?.optJSONArray("data")
                    ?.optJSONObject(0)
                relAlbum?.optJSONObject("attributes")?.optJSONObject("editorialVideo")?.let { video ->
                    buildAppleMedia(video)?.let { return it }
                }

                // 1c. Explicit album catalog fetch (deduplicated across hits)
                val albumId = extractAppleAlbumId(song) ?: continue
                if (!triedAlbumIds.add(albumId)) continue
                val albumUrl = "$APPLE_AMP_BASE/$storefront/albums/$albumId?extend=editorialVideo"
                val albumBody = appleGet(albumUrl, jwt) ?: continue
                val albumObj = JSONObject(albumBody)
                    .optJSONArray("data")
                    ?.optJSONObject(0)
                    ?.optJSONObject("attributes")
                    ?: continue

                val albumName = albumObj.optString("name", "")
                if (isCompilationLabel(albumName)) continue
                val video = albumObj.optJSONObject("editorialVideo") ?: continue
                buildAppleMedia(video)?.let { return it }
            }
        }

        // 1d. Fallback album search when song hits lacked editorialVideo
        if (!album.isNullOrBlank()) {
            searchAppleAlbum(album, artist, jwt)?.let { return it }
        }
        return null
    }

    private fun searchAppleAlbum(album: String, artist: String, jwt: String): CanvasMedia? {
        val term = if (album.contains(artist, ignoreCase = true)) album else "$artist $album"
        val url = "$APPLE_AMP_BASE/$storefront/search" +
            "?term=${encode(term)}&types=albums&limit=8&extend=editorialVideo"
        val body = appleGet(url, jwt) ?: return null
        val albums = JSONObject(body)
            .optJSONObject("results")
            ?.optJSONObject("albums")
            ?.optJSONArray("data")
            ?: return null

        val wantAlbum = normalizeText(album)
        for (i in 0 until albums.length()) {
            val entry = albums.optJSONObject(i) ?: continue
            val attrs = entry.optJSONObject("attributes") ?: continue
            val hitAlbum = attrs.optString("name", "")
            val hitArtist = attrs.optString("artistName", "")
            if (isCompilationLabel(hitAlbum)) continue
            if (!artistsMatch(artist, hitArtist)) continue
            if (normalizeText(cleanTitlePackaging(hitAlbum)) != wantAlbum) continue
            val video = attrs.optJSONObject("editorialVideo") ?: continue
            buildAppleMedia(video)?.let { return it }
        }
        return null
    }

    private fun scoreAppleCandidate(
        song: JSONObject,
        wantedTitle: String,
        wantedArtist: String,
        wantedAlbum: String?,
    ): Int? {
        val attrs = song.optJSONObject("attributes") ?: return null
        val rawHitTitle = attrs.optString("name", "")
        val hitArtist = attrs.optString("artistName", "")
        val rawHitAlbum = attrs.optString("albumName", "")

        if (isCompilationLabel(rawHitTitle) || isCompilationLabel(rawHitAlbum)) return null
        if (!artistsMatch(wantedArtist, hitArtist)) return null

        val normWantTitle = normalizeText(wantedTitle)
        val normHitTitle = normalizeText(cleanTitlePackaging(rawHitTitle))
        if (normWantTitle.isEmpty() || normHitTitle.isEmpty()) return null

        var score = 10
        score += when {
            normHitTitle == normWantTitle -> 15
            normHitTitle.contains(normWantTitle) || normWantTitle.contains(normHitTitle) -> 7
            else -> return null
        }

        if (!wantedAlbum.isNullOrBlank() && rawHitAlbum.isNotBlank()) {
            val normWantAlbum = normalizeText(wantedAlbum)
            val normHitAlbum = normalizeText(cleanTitlePackaging(rawHitAlbum))
            score += when {
                normHitAlbum == normWantAlbum -> 20
                normHitAlbum.contains(normWantAlbum) || normWantAlbum.contains(normHitAlbum) -> 10
                else -> 0
            }
        }

        for (word in EDITION_WORDS) {
            val inWanted = wantedTitle.contains(word, ignoreCase = true)
            val inHit = rawHitTitle.contains(word, ignoreCase = true)
            if (inWanted && inHit) score += 5 else if (inHit && !inWanted) score -= 4
        }

        return score
    }

    private fun extractAppleAlbumId(song: JSONObject): String? {
        val relId = song.optJSONObject("relationships")
            ?.optJSONObject("albums")
            ?.optJSONArray("data")
            ?.optJSONObject(0)
            ?.optString("id", "")
            ?.takeIf { it.isNotBlank() && !it.startsWith("pl.") }
        if (relId != null) return relId

        val webUrl = song.optJSONObject("attributes")?.optString("url", "").orEmpty()
        return webUrl.substringAfter("/album/", "")
            .substringBefore('?')
            .substringAfterLast('/')
            .takeIf { it.isNotBlank() && it.all(Char::isDigit) }
    }

    private fun buildAppleMedia(video: JSONObject): CanvasMedia? {
        fun urlOf(key: String): String? {
            val obj = video.optJSONObject(key) ?: return null
            return listOf("video", "videoUrl", "hlsUrl", "url")
                .firstNotNullOfOrNull { field -> obj.optString(field, "").takeIf { it.startsWith("http") } }
        }
        val square = urlOf("motionDetailSquare") ?: urlOf("motionSquareVideo1x1")
        val tall = urlOf("motionDetailTall") ?: urlOf("motionTallVideo3x4")
        val raw = urlOf("motionDetailRaw")
        val primary = square ?: raw ?: tall ?: return null
        val fallback = listOfNotNull(square, raw, tall).firstOrNull { it != primary }
        return CanvasMedia(
            url = primary,
            fallbackUrl = fallback,
            portraitUrl = tall,
            isHls = primary.contains(".m3u8", ignoreCase = true),
            source = "Apple Music",
        )
    }

    @Synchronized
    private fun obtainAppleJwt(): String? {
        val now = System.currentTimeMillis()
        appleJwt?.let { cached ->
            if (now < appleJwtExpiryMs - TimeUnit.MINUTES.toMillis(1)) return cached
        }
        if (now < appleJwtRetryAfterMs) return null

        val html = httpGet(APPLE_BROWSE_URL, mapOf("User-Agent" to DESKTOP_USER_AGENT))
        val scriptPaths = html?.let {
            APPLE_SCRIPT_REGEX.findAll(it).map(MatchResult::value).distinct().toList()
        }.orEmpty()

        for (path in scriptPaths) {
            val js = httpGet("https://music.apple.com$path", mapOf("User-Agent" to DESKTOP_USER_AGENT))
                ?: continue
            val candidates = JWT_REGEX.findAll(js)
                .map(MatchResult::value)
                .distinct()
                .filter { it !in rejectedAppleJwts }
                .mapNotNull { token -> parseJwtExpiryMs(token)?.let { token to it } }
                .filter { (_, exp) -> exp > now }
                .toList()

            if (candidates.isEmpty()) continue
            val (chosen, exp) = candidates.firstOrNull { isAppleWebPlayerJwt(it.first) } ?: candidates.first()
            appleJwt = chosen
            appleJwtExpiryMs = exp
            return chosen
        }

        appleJwtRetryAfterMs = now + APPLE_TOKEN_RETRY_BACKOFF_MS
        return null
    }

    private fun appleGet(url: String, jwt: String): String? {
        val headers = mapOf(
            "Authorization" to "Bearer $jwt",
            "Origin" to "https://music.apple.com",
            "Referer" to "https://music.apple.com/",
            "User-Agent" to DESKTOP_USER_AGENT,
            "Accept" to "application/json",
        )
        val (code, body) = httpGetWithStatus(url, headers)
        if (code == 401) {
            synchronized(this) {
                rejectedAppleJwts.add(jwt)
                if (appleJwt == jwt) {
                    appleJwt = null
                    appleJwtExpiryMs = 0L
                }
            }
            return null
        }
        return body
    }

    private fun isAppleWebPlayerJwt(jwt: String): Boolean = runCatching {
        val parts = jwt.split('.')
        if (parts.size < 2) return false
        val header = String(Base64.getUrlDecoder().decode(parts[0]), Charsets.UTF_8)
        val payload = String(Base64.getUrlDecoder().decode(parts[1]), Charsets.UTF_8)
        header.contains("WebPlayKid") || payload.contains("AMPWebPlay")
    }.getOrDefault(false)

    private fun parseJwtExpiryMs(jwt: String): Long? = runCatching {
        val parts = jwt.split('.')
        if (parts.size < 2) return null
        val payload = String(Base64.getUrlDecoder().decode(parts[1]), Charsets.UTF_8)
        val exp = JSONObject(payload).optLong("exp", 0L)
        if (exp > 0L) exp * 1000L else null
    }.getOrNull()

    // -------------------------------------------------------------------------
    // Provider 2: Tidal Video Cover (1280x1280 MP4)
    // -------------------------------------------------------------------------

    private fun searchTidal(title: String, artist: String): CanvasMedia? {
        val primaryArtist = splitArtists(artist).firstOrNull().orEmpty()
        val query = if (primaryArtist.isBlank()) title else "$primaryArtist $title"
        val url = "$TIDAL_SEARCH_URL?query=${encode(query)}&limit=10&types=TRACKS&countryCode=$tidalCountry"
        val body = httpGet(
            url,
            mapOf(
                "X-Tidal-Token" to TIDAL_EMBED_TOKEN,
                "User-Agent" to DESKTOP_USER_AGENT,
                "Accept" to "application/json",
            ),
        ) ?: return null

        val items = JSONObject(body)
            .optJSONObject("tracks")
            ?.optJSONArray("items")
            ?: return null

        val wantTitle = normalizeText(title)

        for (i in 0 until items.length()) {
            val track = items.optJSONObject(i) ?: continue
            val rawTrackTitle = track.optString("title", "")
            if (normalizeText(cleanTitlePackaging(rawTrackTitle)) != wantTitle) continue

            val artistsArr = track.optJSONArray("artists")
            val credited = buildList {
                if (artistsArr != null) {
                    for (j in 0 until artistsArr.length()) {
                        artistsArr.optJSONObject(j)?.optString("name", "")
                            ?.takeIf { it.isNotBlank() }
                            ?.let(::add)
                    }
                }
            }.joinToString(", ")
            if (!artistsMatch(artist, credited)) continue

            val albumObj = track.optJSONObject("album") ?: continue
            val videoCover = albumObj.optString("videoCover", "")
            val videoUrl = tidalVideoUrl(videoCover) ?: continue
            return CanvasMedia(
                url = videoUrl,
                fallbackUrl = null,
                portraitUrl = null,
                isHls = false,
                source = "Tidal",
            )
        }
        return null
    }

    private fun tidalVideoUrl(uuid: String): String? {
        val parts = uuid.trim().split('-')
        if (parts.size != 5 || parts.any { it.isBlank() }) return null
        return "https://resources.tidal.com/videos/${parts.joinToString("/")}/1280x1280.mp4"
    }

    // -------------------------------------------------------------------------
    // Provider 3: Community Canvas JSON Fallback
    // -------------------------------------------------------------------------

    private fun searchCommunity(title: String, artist: String, album: String?): CanvasMedia? {
        val entries = loadCommunityManifest()
        if (entries.isEmpty()) return null

        val wantTitle = normalizeText(title)
        val wantAlbum = album?.let(::normalizeText).orEmpty()

        val matches = entries.filter { entry ->
            val gotTitle = normalizeText(cleanTitlePackaging(entry.song))
            val titleOk = gotTitle.isNotEmpty() &&
                (gotTitle == wantTitle || wantTitle.contains(gotTitle) || gotTitle.contains(wantTitle))
            titleOk && artistsMatch(artist, entry.artist)
        }
        if (matches.isEmpty()) return null

        // Prefer an album-matching entry when album metadata is known, without rejecting
        // valid tracks whose YouTube album tag is a compilation or deluxe edition.
        val hit = matches.firstOrNull { entry ->
            val gotAlbum = normalizeText(cleanTitlePackaging(entry.album))
            wantAlbum.isNotEmpty() && gotAlbum.isNotEmpty() && gotAlbum == wantAlbum
        } ?: matches.first()

        return CanvasMedia(
            url = hit.url,
            fallbackUrl = null,
            portraitUrl = null,
            isHls = hit.url.contains(".m3u8", ignoreCase = true),
            source = "Community",
        )
    }

    @Synchronized
    private fun loadCommunityManifest(): List<CommunityEntry> {
        val now = System.currentTimeMillis()
        if (communityEntries.isNotEmpty() && now - communityFetchedAtMs < COMMUNITY_TTL_MS) {
            return communityEntries
        }
        val body = httpGet(COMMUNITY_MANIFEST_URL, mapOf("User-Agent" to DESKTOP_USER_AGENT))
        communityFetchedAtMs = now
        if (body == null) return communityEntries

        val parsed = runCatching {
            val items = JSONObject(body).optJSONArray("items") ?: return@runCatching emptyList()
            buildList {
                for (i in 0 until items.length()) {
                    val obj = items.optJSONObject(i) ?: continue
                    val song = obj.optString("song", "")
                    val artist = obj.optString("artist", "")
                    val url = obj.optString("url", "")
                    if (song.isNotBlank() && artist.isNotBlank() && url.startsWith("http")) {
                        add(
                            CommunityEntry(
                                song = song,
                                artist = artist,
                                album = obj.optString("album", ""),
                                url = url,
                            ),
                        )
                    }
                }
            }
        }.getOrDefault(emptyList())

        if (parsed.isNotEmpty()) {
            communityEntries = parsed
        }
        return communityEntries
    }

    // -------------------------------------------------------------------------
    // HTTP & Metadata Normalization Helpers
    // -------------------------------------------------------------------------

    private fun httpGet(url: String, headers: Map<String, String>): String? =
        httpGetWithStatus(url, headers).second

    private fun httpGetWithStatus(url: String, headers: Map<String, String>): Pair<Int, String?> {
        var conn: HttpURLConnection? = null
        return try {
            conn = (URL(url).openConnection() as HttpURLConnection).apply {
                requestMethod = "GET"
                connectTimeout = CONNECT_TIMEOUT_MS
                readTimeout = READ_TIMEOUT_MS
                instanceFollowRedirects = true
                headers.forEach { (k, v) -> setRequestProperty(k, v) }
            }
            val code = conn.responseCode
            val body = if (code in 200..299) {
                conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            } else {
                null
            }
            code to body
        } catch (_: Exception) {
            -1 to null
        } finally {
            conn?.disconnect()
        }
    }

    private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8")

    private fun cleanTitlePackaging(raw: String): String = raw
        .replace(TITLE_PACKAGING_REGEX, " ")
        .replace(TRAILING_FEAT_REGEX, " ")
        .substringBefore(" | ")
        .replace(MULTI_SPACE_REGEX, " ")
        .trim()
        .trimEnd('-', '–', '—', ',')
        .trim()
        .ifBlank { raw.trim() }

    private fun cleanArtistPackaging(raw: String): String = raw
        .removeSuffix(" - Topic")
        .trim()
        .ifBlank { raw.trim() }

    private fun normalizeText(input: String): String = Normalizer
        .normalize(input, Normalizer.Form.NFD)
        .replace(COMBINING_MARKS_REGEX, "")
        .lowercase(Locale.ROOT)
        .replace('ı', 'i')
        .replace(NON_ALNUM_REGEX, " ")
        .replace(MULTI_SPACE_REGEX, " ")
        .trim()

    private fun splitArtists(raw: String): List<String> = raw
        .split(ARTIST_SPLIT_REGEX)
        .map(::normalizeText)
        .filter { it.isNotEmpty() }

    private fun artistsMatch(wantedRaw: String, candidateRaw: String): Boolean {
        val wanted = splitArtists(wantedRaw)
        val credited = splitArtists(candidateRaw)
        if (wanted.isEmpty() || credited.isEmpty()) return false
        return wanted.all { w -> credited.any { c -> c == w } } ||
            credited.all { c -> wanted.any { w -> w == c } }
    }

    private fun isCompilationLabel(name: String): Boolean {
        val lower = name.lowercase(Locale.ROOT)
        return COMPILATION_KEYWORDS.any { lower.contains(it) }
    }

    private val APPLE_SCRIPT_REGEX = Regex("""/assets/index(?:-legacy)?[~-][A-Za-z0-9_-]+\.js""")
    private val JWT_REGEX = Regex("""ey[A-Za-z0-9_-]+\.ey[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+""")
    private val COMBINING_MARKS_REGEX = Regex("""\p{Mn}+""")
    private val NON_ALNUM_REGEX = Regex("""[^\p{L}\p{N}\s]""")
    private val MULTI_SPACE_REGEX = Regex("""\s+""")
    private val ARTIST_SPLIT_REGEX = Regex(
        """(?:\s*,\s*|\s*&\s*|\s*•\s*|\s*/\s*|\s+×\s+|\s+x\s+|\bfeat\.?\b|\bft\.?\b|\bfeaturing\b|\bwith\b)""",
        RegexOption.IGNORE_CASE,
    )
    private val TITLE_PACKAGING_REGEX = Regex(
        """\s*[(\[]\s*(?:from|official|lyrical|video|audio|visualizer|visualiser|hd|hq|4k|full\s*song|feat\.?|ft\.?|featuring|with)[^)\]]*[)\]]|""" +
            """\b(?:official\s+(?:video|audio|music\s+video)|lyrical|full\s+song|4k\s+video)\b""",
        RegexOption.IGNORE_CASE,
    )
    private val TRAILING_FEAT_REGEX = Regex(
        """\s+(?:feat|ft|featuring)\.?\s+.*$""",
        RegexOption.IGNORE_CASE,
    )
    private val EDITION_WORDS = listOf(
        "deluxe", "expanded", "remastered", "remix", "version", "edit", "mix", "live", "acoustic",
    )
    private val COMPILATION_KEYWORDS = listOf(
        "playlist",
        "set list",
        "essentials",
        "dj mix",
        "apple music",
        "today's hits",
    )
}
