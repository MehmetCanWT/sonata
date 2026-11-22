package dev.sonata.android.data

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.sonata_android.AndroidTrack
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.text.Normalizer
import java.util.Base64
import java.util.Locale
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
import kotlin.math.abs

data class JioSaavnStream(
    val url: String,
    val bitrateKbps: Int,
    val title: String,
    val artist: String,
    val durationSecs: Int,
)

/**
 * Clean-room optional JioSaavn external source module.
 * Enabled only when the user turns on the JioSaavn module under Settings -> Modules.
 * Provides 320 kbps AAC streams when a strict title/artist/duration match beats the active stream.
 */
object JioSaavnSource {
    private const val TAG = "SonataJioSaavn"
    private const val API_BASE = "https://www.jiosaavn.com/api.php"
    private const val DES_KEY = "38346591"
    private const val MIN_USABLE_KBPS = 160
    private const val MAX_DURATION_DIFF_SECS = 6
    private const val CONNECT_TIMEOUT_MS = 4_000
    private const val READ_TIMEOUT_MS = 6_000

    private val bitrateRegex = Regex("_(48|96|160|320)\\.(mp4|aac|mp3)(?=[?#]|$)", RegexOption.IGNORE_CASE)
    private val parensFeatRegex = Regex("\\((feat|ft|featuring|with)\\.?[^)]*\\)", RegexOption.IGNORE_CASE)
    private val bracketNoiseRegex = Regex(
        "\\[(official|lyric|lyrics|audio|video|hd|4k|hq|mv)[^]]*]|\\((official|lyric|lyrics|audio|video|hd|4k|hq|mv)[^)]*\\)",
        RegexOption.IGNORE_CASE,
    )
    private val nonAlnumRegex = Regex("[^\\p{L}\\p{N}]+")

    suspend fun searchTracks(query: String, limit: Int = 8): List<AndroidTrack> = withContext(Dispatchers.IO) {
        val trimmed = query.trim()
        if (trimmed.isBlank()) return@withContext emptyList()
        val encoded = URLEncoder.encode(trimmed, "UTF-8")
        val body = httpGet(
            "$API_BASE?__call=search.getResults&_format=json&_marker=0&api_version=4&ctx=android&q=$encoded&p=1&n=$limit"
        ) ?: return@withContext emptyList()

        val root = runCatching { JSONObject(body) }.getOrNull() ?: return@withContext emptyList()
        val results = root.optJSONArray("results") ?: return@withContext emptyList()
        val tracks = ArrayList<AndroidTrack>(results.length())
        for (i in 0 until results.length()) {
            val item = results.optJSONObject(i) ?: continue
            val id = item.optString("id").trim()
            val title = decodeHtml(item.optString("title").ifBlank { item.optString("song") })
            if (id.isBlank() || title.isBlank()) continue
            val more = item.optJSONObject("more_info")
            val artist = extractArtists(item, more).ifBlank { "Unknown Artist" }
            val album = decodeHtml(more?.optString("album").orEmpty())
            val duration = more?.optString("duration")?.toULongOrNull()
            val image = upgradeArtwork(item.optString("image"))
            val explicit = item.optString("explicit_content") == "1"
            tracks.add(
                AndroidTrack(
                    id = "jiosaavn:$id",
                    title = title,
                    artist = artist,
                    album = album,
                    durationSecs = duration,
                    thumbnailUrl = image,
                    liked = false,
                    albumId = null,
                    isExplicit = explicit,
                )
            )
        }
        tracks
    }

    /**
     * Resolves a JioSaavn stream by track ID (for direct `jiosaavn:<id>` search hits).
     */
    suspend fun resolveById(rawId: String): JioSaavnStream? = withContext(Dispatchers.IO) {
        val songId = rawId.removePrefix("jiosaavn:").trim()
        if (songId.isBlank()) return@withContext null
        fetchSongDetailsStream(songId)
    }

    /**
     * Searches JioSaavn for a matching recording and returns a stream ONLY when its
     * verified bitrate strictly exceeds [currentBitrateKbps] and passes strict metadata matching.
     */
    suspend fun findHigherQualityStream(
        track: AndroidTrack,
        currentBitrateKbps: Int,
    ): JioSaavnStream? = withContext(Dispatchers.IO) {
        if (currentBitrateKbps >= 320) return@withContext null
        if (track.id.startsWith("jiosaavn:")) {
            return@withContext resolveById(track.id)
        }

        val cleanTitle = cleanTitleForMatch(track.title)
        val primaryArtist = primaryArtistOf(track.artist)
        if (cleanTitle.isBlank()) return@withContext null

        val query = if (primaryArtist.isNotBlank()) "$cleanTitle $primaryArtist" else cleanTitle
        val encoded = URLEncoder.encode(query, "UTF-8")
        val body = httpGet(
            "$API_BASE?__call=search.getResults&_format=json&_marker=0&api_version=4&ctx=android&q=$encoded&p=1&n=10"
        ) ?: return@withContext null

        val root = runCatching { JSONObject(body) }.getOrNull() ?: return@withContext null
        val results = root.optJSONArray("results") ?: return@withContext null
        val expectedSecs = track.durationSecs?.toInt() ?: 0

        data class ScoredCandidate(val id: String, val score: Int, val explicit: Boolean)
        val candidates = ArrayList<ScoredCandidate>()

        for (i in 0 until results.length()) {
            val item = results.optJSONObject(i) ?: continue
            val id = item.optString("id").trim()
            val candTitle = decodeHtml(item.optString("title").ifBlank { item.optString("song") })
            if (id.isBlank() || candTitle.isBlank()) continue
            val more = item.optJSONObject("more_info")
            val candArtist = extractArtists(item, more)
            val candDur = more?.optString("duration")?.toIntOrNull() ?: 0
            val candExplicit = item.optString("explicit_content") == "1"

            val normWant = normalizeCore(cleanTitle)
            val normCand = normalizeCore(cleanTitleForMatch(candTitle))
            if (normWant.isBlank() || normCand != normWant) continue

            if (primaryArtist.isNotBlank() && !artistsOverlap(track.artist, candArtist)) continue
            if (expectedSecs > 0 && candDur > 0 && abs(expectedSecs - candDur) > MAX_DURATION_DIFF_SECS) continue

            var score = 100
            if (expectedSecs > 0 && candDur > 0) {
                score += (MAX_DURATION_DIFF_SECS - abs(expectedSecs - candDur)) * 5
            }
            if (track.isExplicit && candExplicit) score += 20
            if (more?.optString("320kbps").equals("true", ignoreCase = true)) score += 15
            candidates.add(ScoredCandidate(id = id, score = score, explicit = candExplicit))
        }

        val best = candidates.sortedWith(
            compareByDescending<ScoredCandidate> { it.score }.thenByDescending { it.explicit }
        ).firstOrNull() ?: return@withContext null

        val stream = fetchSongDetailsStream(best.id) ?: return@withContext null
        if (stream.bitrateKbps <= currentBitrateKbps || stream.bitrateKbps < MIN_USABLE_KBPS) {
            return@withContext null
        }
        Log.d(
            TAG,
            "Higher quality match: '${track.title}' (${currentBitrateKbps}kbps -> JioSaavn ${stream.bitrateKbps}kbps)"
        )
        stream
    }

    private fun fetchSongDetailsStream(songId: String): JioSaavnStream? {
        val encoded = URLEncoder.encode(songId, "UTF-8")
        val body = httpGet(
            "$API_BASE?__call=song.getDetails&_format=json&_marker=0&api_version=4&ctx=android&pids=$encoded"
        ) ?: return null
        val root = runCatching { JSONObject(body) }.getOrNull() ?: return null
        val songObj = root.optJSONArray("songs")?.optJSONObject(0)
            ?: root.optJSONObject(songId)
            ?: return null
        val more = songObj.optJSONObject("more_info") ?: return null
        val encrypted = more.optString("encrypted_media_url").trim()
        if (encrypted.isBlank()) return null

        val rawUrl = decryptUrl(encrypted) ?: return null
        val supports320 = more.optString("320kbps").equals("true", ignoreCase = true)
        val (finalUrl, kbps) = selectBestStreamUrl(rawUrl, supports320)
        if (kbps < MIN_USABLE_KBPS) return null

        return JioSaavnStream(
            url = finalUrl,
            bitrateKbps = kbps,
            title = decodeHtml(songObj.optString("title").ifBlank { songObj.optString("song") }),
            artist = extractArtists(songObj, more),
            durationSecs = more.optString("duration").toIntOrNull() ?: 0,
        )
    }

    internal fun decryptUrl(encryptedBase64: String): String? = runCatching {
        val keySpec = SecretKeySpec(DES_KEY.toByteArray(Charsets.UTF_8), "DES")
        val cipher = Cipher.getInstance("DES/ECB/PKCS5Padding")
        cipher.init(Cipher.DECRYPT_MODE, keySpec)
        val decoded = Base64.getDecoder().decode(encryptedBase64)
        val plain = String(cipher.doFinal(decoded), Charsets.UTF_8).trim()
        plain.replace("http://", "https://")
    }.getOrNull()

    internal fun selectBestStreamUrl(url: String, supports320: Boolean): Pair<String, Int> {
        val match = bitrateRegex.find(url)
        val offeredKbps = match?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 160
        if (supports320 && match != null) {
            val ext = match.groupValues[2]
            val upgraded = bitrateRegex.replace(url, "_320.$ext")
            return upgraded to 320
        }
        return url to offeredKbps
    }

    private fun httpGet(urlStr: String): String? = runCatching {
        val conn = (URL(urlStr).openConnection() as HttpURLConnection).apply {
            requestMethod = "GET"
            connectTimeout = CONNECT_TIMEOUT_MS
            readTimeout = READ_TIMEOUT_MS
            setRequestProperty(
                "User-Agent",
                "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36",
            )
            setRequestProperty("Accept", "application/json")
            setRequestProperty("Accept-Language", "en-IN,en;q=0.9")
            setRequestProperty("Cookie", "explicit_content=1")
            setRequestProperty("X-Forwarded-For", "49.36.0.1")
            setRequestProperty("X-Real-IP", "49.36.0.1")
        }
        try {
            if (conn.responseCode !in 200..299) return@runCatching null
            conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
        } finally {
            conn.disconnect()
        }
    }.getOrNull()

    private fun extractArtists(item: JSONObject, more: JSONObject?): String {
        val artistMap = more?.optJSONObject("artistMap")
        val primary = artistMap?.optJSONArray("primary_artists")
        if (primary != null && primary.length() > 0) {
            val names = (0 until primary.length())
                .mapNotNull { primary.optJSONObject(it)?.optString("name")?.trim()?.takeIf(String::isNotBlank) }
                .distinct()
            if (names.isNotEmpty()) return decodeHtml(names.joinToString(", "))
        }
        val singers = more?.optString("singers")?.trim().orEmpty()
        if (singers.isNotBlank()) return decodeHtml(singers)
        return decodeHtml(item.optString("subtitle").substringBefore(" - ").trim())
    }

    internal fun upgradeArtwork(raw: String): String? {
        val trimmed = raw.trim()
        if (trimmed.isBlank()) return null
        return trimmed
            .replace("http://", "https://")
            .replace("150x150", "500x500")
            .replace("50x50", "500x500")
    }

    internal fun decodeHtml(raw: String): String = raw
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()

    internal fun cleanTitleForMatch(title: String): String {
        var s = parensFeatRegex.replace(title, " ")
        s = bracketNoiseRegex.replace(s, " ")
        return s.trim()
    }

    internal fun normalizeCore(text: String): String {
        val nfd = Normalizer.normalize(text.lowercase(Locale.ROOT), Normalizer.Form.NFD)
        val noMarks = nfd.replace(Regex("\\p{Mn}+"), "").replace('ı', 'i')
        return nonAlnumRegex.replace(noMarks, " ").trim()
    }

    internal fun primaryArtistOf(artist: String): String =
        artist.split(",", "&", "feat.", "ft.", "featuring", ignoreCase = true)
            .firstOrNull()
            ?.trim()
            .orEmpty()

    internal fun artistsOverlap(a: String, b: String): Boolean {
        val setA = a.split(",", "&", "feat.", "ft.", "featuring", ignoreCase = true)
            .map { normalizeCore(it) }
            .filter { it.length >= 2 }
            .toSet()
        val setB = b.split(",", "&", "feat.", "ft.", "featuring", ignoreCase = true)
            .map { normalizeCore(it) }
            .filter { it.length >= 2 }
            .toSet()
        if (setA.isEmpty() || setB.isEmpty()) return false
        return setA.any { x -> setB.any { y -> x == y || x.contains(y) || y.contains(x) } }
    }
}
