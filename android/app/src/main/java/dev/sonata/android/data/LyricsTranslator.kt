package dev.sonata.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import org.json.JSONArray
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.util.Locale

/**
 * On-demand batched lyrics translation and romanization backed by Google's lightweight
 * translation endpoint (`translate.googleapis.com/translate_a/single`), with PUA line
 * delimiters and an in-memory LRU cache (24 tracks).
 */
object LyricsTranslator {

    private const val ENDPOINT = "https://translate.googleapis.com/translate_a/single"
    private const val MAX_BATCH_CHARS = 3_200
    private const val MAX_CONCURRENT_BATCHES = 2
    private const val TIMEOUT_MS = 10_000
    private const val CACHE_CAPACITY = 24
    private const val USER_AGENT = "Sonata/1.0"

    // Private-use delimiters preserve exact 1:1 line mapping across batched requests.
    // Some CJK responses normalize the closing U+E001 to U+E000, so accept either terminator.
    private val MARKER_SPLIT_REGEX = Regex("""\uE000[^\uE000\uE001]*[\uE000\uE001]""")

    private val cache = object : LinkedHashMap<String, List<String>>(CACHE_CAPACITY, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, List<String>>): Boolean =
            size > CACHE_CAPACITY
    }

    private data class Slot(
        val originalIndex: Int,
        val text: String,
    )

    private data class Batch(
        val slots: List<Slot>,
        val payload: String,
    )

    /**
     * Translates [lines] into [targetLang] while preserving blank lines and exact index mapping.
     * Returns `null` if the request fails or the response cannot be cleanly mapped back.
     */
    suspend fun translateLines(
        trackKey: String,
        lines: List<String>,
        targetLang: String,
    ): List<String>? {
        val target = targetLang.trim()
        if (target.isBlank() || lines.isEmpty()) return null

        val cacheKey = "t|$trackKey|${target.lowercase(Locale.ROOT)}|${lines.size}"
        synchronized(cache) {
            cache[cacheKey]?.let { cached ->
                if (cached.size == lines.size) return cached
            }
        }

        val slots = lines.mapIndexedNotNull { idx, text ->
            val trimmed = text.trim()
            if (trimmed.isEmpty()) null else Slot(idx, trimmed)
        }
        if (slots.isEmpty()) return lines

        val batches = buildBatches(slots)
        val translatedSlots = executeBatches(batches, targetLang = target, romanize = false) ?: return null
        if (translatedSlots.size != slots.size) return null

        val merged = mergeBack(lines, slots, translatedSlots)
        synchronized(cache) {
            cache[cacheKey] = merged
        }
        return merged
    }

    /**
     * Romanizes non-Latin [lines] into Latin script while preserving blank lines and exact index mapping.
     * Returns `null` on failure or if no romanization is available.
     */
    suspend fun romanizeLines(
        trackKey: String,
        lines: List<String>,
    ): List<String>? {
        if (lines.isEmpty()) return null

        val cacheKey = "r|$trackKey|${lines.size}"
        synchronized(cache) {
            cache[cacheKey]?.let { cached ->
                if (cached.size == lines.size) return cached
            }
        }

        val slots = lines.mapIndexedNotNull { idx, text ->
            val trimmed = text.trim()
            if (trimmed.isEmpty()) null else Slot(idx, trimmed)
        }
        if (slots.isEmpty()) return lines
        if (!containsNonLatinLetter(slots)) return null

        val batches = buildBatches(slots)
        val romanizedSlots = executeBatches(batches, targetLang = "en", romanize = true) ?: return null
        if (romanizedSlots.size != slots.size) return null

        val merged = mergeBack(lines, slots, romanizedSlots)
        synchronized(cache) {
            cache[cacheKey] = merged
        }
        return merged
    }

    private fun buildBatches(slots: List<Slot>): List<Batch> {
        val result = mutableListOf<Batch>()
        var current = mutableListOf<Slot>()
        var currentLen = 0

        fun flush() {
            if (current.isEmpty()) return
            val payload = buildString {
                current.forEachIndexed { idx, slot ->
                    if (idx > 0) {
                        append('\n').append(markerFor(idx)).append('\n')
                    }
                    append(slot.text)
                }
            }
            result.add(Batch(current.toList(), payload))
            current = mutableListOf()
            currentLen = 0
        }

        for (slot in slots) {
            val delta = slot.text.length + if (current.isEmpty()) 0 else 8
            if (current.isNotEmpty() && currentLen + delta > MAX_BATCH_CHARS) {
                flush()
            }
            current.add(slot)
            currentLen += delta
        }
        flush()
        return result
    }

    private fun markerFor(index: Int): String =
        "\uE000${index.toString().padStart(4, '0')}\uE001"

    private suspend fun executeBatches(
        batches: List<Batch>,
        targetLang: String,
        romanize: Boolean,
    ): List<String>? = coroutineScope {
        val all = mutableListOf<String>()
        for (chunk in batches.chunked(MAX_CONCURRENT_BATCHES)) {
            val answers = chunk.map { batch ->
                async(Dispatchers.IO) {
                    requestBatch(batch, targetLang = targetLang, romanize = romanize)
                }
            }.awaitAll()
            for (answer in answers) {
                if (answer == null) return@coroutineScope null
                all.addAll(answer)
            }
        }
        all
    }

    private fun requestBatch(
        batch: Batch,
        targetLang: String,
        romanize: Boolean,
    ): List<String>? {
        var conn: HttpURLConnection? = null
        return try {
            val dt = if (romanize) "rm" else "t"
            val form = "client=dict-chrome-ex&sl=auto&tl=${encode(targetLang)}&dt=$dt&q=${encode(batch.payload)}"
            val bodyBytes = form.toByteArray(Charsets.UTF_8)

            conn = (URL(ENDPOINT).openConnection() as HttpURLConnection).apply {
                requestMethod = "POST"
                doOutput = true
                connectTimeout = TIMEOUT_MS
                readTimeout = TIMEOUT_MS
                setRequestProperty("User-Agent", USER_AGENT)
                setRequestProperty("Accept", "application/json")
                setRequestProperty("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
                setFixedLengthStreamingMode(bodyBytes.size)
            }
            conn.outputStream.use { it.write(bodyBytes) }

            if (conn.responseCode !in 200..299) return null
            val responseText = conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            val root = JSONArray(responseText)
            val segments = root.optJSONArray(0) ?: return null

            val combined = buildString {
                val segmentFieldIndex = if (romanize) 3 else 0
                for (i in 0 until segments.length()) {
                    val seg = segments.optJSONArray(i) ?: continue
                    val piece = seg.optString(segmentFieldIndex, "")
                    if (piece.isNotEmpty() && piece != "null") {
                        append(piece)
                    }
                }
            }
            if (combined.isBlank()) return null

            val parts = combined.split(MARKER_SPLIT_REGEX).map(String::trim)
            if (parts.size != batch.slots.size || parts.any { it.isBlank() }) {
                null
            } else {
                parts
            }
        } catch (_: Exception) {
            null
        } finally {
            conn?.disconnect()
        }
    }

    private fun mergeBack(
        original: List<String>,
        slots: List<Slot>,
        transformed: List<String>,
    ): List<String> {
        val out = original.toMutableList()
        slots.forEachIndexed { i, slot ->
            out[slot.originalIndex] = transformed[i]
        }
        return out
    }

    private fun containsNonLatinLetter(slots: List<Slot>): Boolean = slots.any { slot ->
        slot.text.codePoints().anyMatch { cp ->
            Character.isLetter(cp) && Character.UnicodeScript.of(cp) != Character.UnicodeScript.LATIN
        }
    }

    private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8")
}
