package dev.sonata.android.data

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.sonata_android.AndroidMotionMedia
import uniffi.sonata_android.AndroidTrack
import java.util.Locale

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

internal fun AndroidMotionMedia.toCanvasMedia(): CanvasMedia = CanvasMedia(
    url = url,
    fallbackUrl = fallbackUrl,
    portraitUrl = portraitUrl,
    isHls = isHls,
    source = source,
)

/**
 * Android platform cache and lookup coordinator for animated album artwork.
 *
 * Remote discovery (Apple Music AMP/HTML, Tidal, Community Canvas) is owned exclusively by
 * `sonata-core::motion::resolve_motion_media` via [SonataEngineGateway]. This object keeps only
 * the bounded in-memory LRU cache (including negative-cache upgrade semantics) and locale metadata.
 */
object CanvasResolver {
    private const val MAX_CACHE_ENTRIES = 64

    private data class CacheEntry(
        val media: CanvasMedia?,
        val resolvedWithAlbum: Boolean,
    )

    private val resolveMutex = Mutex()
    private val cache = object : LinkedHashMap<String, CacheEntry>(MAX_CACHE_ENTRIES, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, CacheEntry>): Boolean =
            size > MAX_CACHE_ENTRIES
    }

    @Volatile
    private var attachedGateway: SonataEngineGateway? = null

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

    fun attachGateway(gateway: SonataEngineGateway) {
        attachedGateway = gateway
    }

    fun detachGateway(gateway: SonataEngineGateway? = null) {
        if (gateway == null || attachedGateway === gateway) {
            attachedGateway = null
        }
    }

    internal fun clearCacheForTest() {
        synchronized(cache) { cache.clear() }
    }

    /**
     * Returns an already-cached [CanvasMedia] for [videoId] without hitting the network.
     */
    fun cached(videoId: String): CanvasMedia? {
        if (videoId.isBlank()) return null
        return synchronized(cache) { cache[videoId]?.media }
    }

    /**
     * Resolves an animated artwork clip for the given track metadata via `sonata-core`.
     * Propagates [CancellationException] immediately; returns `null` when no verified clip is found.
     */
    suspend fun resolve(
        videoId: String,
        title: String,
        artist: String,
        album: String? = null,
        gateway: SonataEngineGateway? = null,
    ): CanvasMedia? {
        val trimmedTitle = title.trim()
        val trimmedArtist = artist.trim()
        val trimmedAlbum = album?.trim().orEmpty()
        if (trimmedTitle.isBlank() || trimmedArtist.isBlank()) return null

        val activeGateway = gateway ?: attachedGateway ?: return null
        val cacheKey = videoId.ifBlank {
            "${trimmedTitle.lowercase(Locale.ROOT)}|${trimmedArtist.lowercase(Locale.ROOT)}"
        }
        val hasAlbum = trimmedAlbum.isNotBlank()

        return resolveMutex.withLock {
            synchronized(cache) {
                val existing = cache[cacheKey]
                if (existing != null && (existing.media != null || existing.resolvedWithAlbum || !hasAlbum)) {
                    return@withLock existing.media
                }
            }

            val queryTrack = AndroidTrack(
                id = videoId,
                title = trimmedTitle,
                artist = trimmedArtist,
                album = trimmedAlbum,
                durationSecs = null,
                thumbnailUrl = null,
                liked = false,
                albumId = null,
                isExplicit = false,
            )

            val found = try {
                activeGateway
                    .resolveMotionMedia(
                        track = queryTrack,
                        storefront = storefront,
                        countryCode = tidalCountry,
                    )
                    ?.toCanvasMedia()
            } catch (ce: CancellationException) {
                throw ce
            } catch (err: Throwable) {
                runCatching {
                    Log.w("SonataCanvas", "Rust motion discovery error: ${err.message}")
                }
                null
            }

            synchronized(cache) {
                cache[cacheKey] = CacheEntry(media = found, resolvedWithAlbum = hasAlbum)
            }
            found
        }
    }
}
