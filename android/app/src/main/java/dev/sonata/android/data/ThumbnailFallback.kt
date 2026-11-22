package dev.sonata.android.data

import coil3.intercept.Interceptor
import coil3.request.ErrorResult
import coil3.request.ImageResult
import coil3.request.SuccessResult
import java.util.concurrent.ConcurrentHashMap

/**
 * Coil 3 interceptor that normalizes artwork URLs and recovers from 404/broken
 * `i.ytimg.com` thumbnails (`maxresdefault` -> `sddefault` -> `hqdefault` -> `mqdefault`),
 * memoizing failed URLs so missing high-res covers never cause blank artwork.
 */
object SonataThumbnailInterceptor : Interceptor {

    private const val MAX_MEMO_ENTRIES = 512

    private val failedFallbacks = ConcurrentHashMap<String, String>()
    private val normalizedUrlCache = ConcurrentHashMap<String, String>()

    private val LH3_SIZE_REGEX = Regex("=w(\\d+)-h(\\d+)[^&\\s]*|=s(\\d+)[^&\\s]*")

    private val YT_TIERS = listOf(
        "maxresdefault.jpg",
        "sddefault.jpg",
        "hqdefault.jpg",
        "mqdefault.jpg",
    )

    fun normalizeCoverUrl(rawUrl: String?): String? {
        val trimmed = rawUrl?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        if (trimmed.startsWith("https://") && trimmed.contains("=w544-h544-l90-rj")) {
            return trimmed
        }
        normalizedUrlCache[trimmed]?.let { return it }

        val httpsUrl = if (trimmed.startsWith("http://") &&
            (trimmed.contains("googleusercontent.com") ||
                trimmed.contains("ytimg.com") ||
                trimmed.contains("ggpht.com"))
        ) {
            "https://" + trimmed.removePrefix("http://")
        } else {
            trimmed
        }

        val normalized = if (httpsUrl.contains("googleusercontent.com") || httpsUrl.contains("ggpht.com")) {
            val match = LH3_SIZE_REGEX.find(httpsUrl)
            if (match != null) {
                val width = match.groupValues[1].toIntOrNull()
                    ?: match.groupValues[3].toIntOrNull()
                    ?: 0
                if (width in 1 until 480) {
                    httpsUrl.replaceRange(match.range, "=w544-h544-l90-rj")
                } else {
                    httpsUrl
                }
            } else {
                "$httpsUrl=w544-h544-l90-rj"
            }
        } else {
            httpsUrl
        }

        rememberNormalizedUrl(trimmed, normalized)
        return normalized
    }

    private fun rememberNormalizedUrl(raw: String, normalized: String) {
        if (normalizedUrlCache.size >= MAX_MEMO_ENTRIES) {
            val iterator = normalizedUrlCache.keys.iterator()
            var removed = 0
            while (iterator.hasNext() && removed < 64) {
                iterator.next()
                iterator.remove()
                removed++
            }
        }
        normalizedUrlCache[raw] = normalized
    }

    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
        val originalData = chain.request.data as? String ?: return chain.proceed()
        val normalized = normalizeCoverUrl(originalData) ?: return chain.proceed()

        val memoized = failedFallbacks[normalized]
        val initialUrl = memoized ?: normalized
        val initialReq = if (initialUrl != originalData) {
            chain.request.newBuilder().data(initialUrl).build()
        } else {
            chain.request
        }

        val initialResult = chain.withRequest(initialReq).proceed()
        if (initialResult is SuccessResult || initialResult !is ErrorResult) {
            return initialResult
        }

        val candidates = ytFallbackCandidates(initialUrl)
        if (candidates.isEmpty()) {
            return initialResult
        }

        for (fallbackUrl in candidates) {
            val fallbackReq = chain.request.newBuilder().data(fallbackUrl).build()
            val nextResult = chain.withRequest(fallbackReq).proceed()
            if (nextResult is SuccessResult) {
                rememberFallback(normalized, fallbackUrl)
                return nextResult
            }
        }

        return initialResult
    }

    private fun ytFallbackCandidates(url: String): List<String> {
        if (!url.contains("ytimg.com") && !url.contains("img.youtube.com")) {
            return emptyList()
        }
        val basePath = url.substringBefore('?')
        val query = url.substringAfter('?', "")
        val currentIdx = YT_TIERS.indexOfFirst { basePath.endsWith("/$it", ignoreCase = true) }
        if (currentIdx < 0) {
            return emptyList()
        }
        val prefix = basePath.substringBeforeLast('/')
        return YT_TIERS.drop(currentIdx + 1).map { tier ->
            val candidate = "$prefix/$tier"
            if (query.isNotEmpty()) "$candidate?$query" else candidate
        }
    }

    private fun rememberFallback(failedUrl: String, workingUrl: String) {
        if (failedFallbacks.size >= MAX_MEMO_ENTRIES) {
            val iterator = failedFallbacks.keys.iterator()
            var removed = 0
            while (iterator.hasNext() && removed < 64) {
                iterator.next()
                iterator.remove()
                removed++
            }
        }
        failedFallbacks[failedUrl] = workingUrl
    }
}
