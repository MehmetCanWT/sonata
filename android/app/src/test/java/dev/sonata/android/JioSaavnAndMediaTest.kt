package dev.sonata.android

import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.artworkMemoryCacheKey
import dev.sonata.android.ui.components.MoodTheme
import dev.sonata.android.ui.components.resolveMoodTheme
import dev.sonata.android.ui.croppedAt
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidJioSaavnStream
import uniffi.sonata_android.AndroidMotionMedia
import uniffi.sonata_android.AndroidTrack
import java.util.concurrent.atomic.AtomicInteger

class JioSaavnAndMediaTest {

    @After
    fun tearDown() {
        CanvasResolver.detachGateway()
        CanvasResolver.clearCacheForTest()
    }

    @Test
    fun canvasResolverDelegatesDiscoveryToRustGatewayWithNegativeCacheUpgradeSemantics() = runTest {
        val gateway = FakeSonataEngineGateway()
        val calls = AtomicInteger(0)
        gateway.motionMediaHandler = { track, storefront, country ->
            val attempt = calls.incrementAndGet()
            assertEquals(2, storefront?.length)
            assertEquals(2, country?.length)
            if (track.album.isBlank()) {
                null
            } else {
                AndroidMotionMedia(
                    url = "https://mvod.itunes.apple.com/square_$attempt.m3u8",
                    fallbackUrl = "https://mvod.itunes.apple.com/tall_$attempt.m3u8",
                    portraitUrl = "https://mvod.itunes.apple.com/tall_$attempt.m3u8",
                    isHls = true,
                    source = "Apple Music",
                )
            }
        }

        // 1. Lookup without album misses and negative-caches with resolvedWithAlbum = false.
        assertNull(
            CanvasResolver.resolve(
                videoId = "vid_1",
                title = "Blinding Lights",
                artist = "The Weeknd",
                album = null,
                gateway = gateway,
            )
        )
        assertEquals(1, calls.get())

        // 2. Repeated lookup without album hits negative cache (no duplicate network call).
        assertNull(
            CanvasResolver.resolve(
                videoId = "vid_1",
                title = "Blinding Lights",
                artist = "The Weeknd",
                album = null,
                gateway = gateway,
            )
        )
        assertEquals(1, calls.get())

        // 3. Providing album metadata retries once and caches the discovered MotionMedia.
        val resolved = CanvasResolver.resolve(
            videoId = "vid_1",
            title = "Blinding Lights",
            artist = "The Weeknd",
            album = "After Hours",
            gateway = gateway,
        )
        assertEquals(2, calls.get())
        assertEquals("Apple Music", resolved?.source)
        assertTrue(resolved?.isHls == true)
        assertEquals("https://mvod.itunes.apple.com/tall_2.m3u8", resolved?.portraitUrl)
        assertEquals(resolved, CanvasResolver.cached("vid_1"))

        // 4. Subsequent lookup hits positive cache without calling gateway again.
        val cachedHit = CanvasResolver.resolve(
            videoId = "vid_1",
            title = "Blinding Lights",
            artist = "The Weeknd",
            album = "After Hours",
            gateway = gateway,
        )
        assertEquals(2, calls.get())
        assertEquals(resolved, cachedHit)
    }

    @Test
    fun canvasResolverPropagatesCancellationAndGatewayExposesJioSaavnOperations() = runTest {
        val gateway = FakeSonataEngineGateway()
        val gate = CompletableDeferred<AndroidMotionMedia?>()
        gateway.motionMediaHandler = { _, _, _ -> gate.await() }

        val job = launch {
            CanvasResolver.resolve(
                videoId = "vid_cancel",
                title = "Starboy",
                artist = "The Weeknd",
                album = "Starboy",
                gateway = gateway,
            )
        }
        testScheduler.runCurrent()
        job.cancel()
        testScheduler.advanceUntilIdle()
        assertEquals(1, gateway.cancelledMotionCount.get())
        assertNull(CanvasResolver.cached("vid_cancel"))

        val sampleTrack = AndroidTrack(
            id = "jiosaavn:abc123",
            title = "Starboy",
            artist = "The Weeknd",
            album = "Starboy",
            durationSecs = 230uL,
            thumbnailUrl = "https://c.saavncdn.com/000/cover_500x500.jpg",
            liked = false,
            albumId = null,
            isExplicit = true,
        )
        gateway.jioSaavnSearchHandler = { _, _ -> listOf(sampleTrack) }
        gateway.jioSaavnByIdHandler = {
            AndroidJioSaavnStream(
                url = "https://aac.saavncdn.com/123/abc_320.mp4",
                bitrateKbps = 320u,
                badge = "AAC 320kbps · JioSaavn",
                title = "Starboy",
                artist = "The Weeknd",
                durationSecs = 230uL,
            )
        }
        assertEquals(listOf(sampleTrack), gateway.searchJiosaavn("starboy", 6u))
        assertEquals(320u, gateway.resolveJiosaavnById("jiosaavn:abc123")?.bitrateKbps)
    }

    @Test
    fun artworkUrlsScaleDeterministicallyAcrossHosts() {
        assertEquals(
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/1200x1200bb.jpg",
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/100x100bb.jpg".artworkAt(1200),
        )
        assertEquals(
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/720x720cc.jpg",
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/100x100bb.jpg".croppedAt(480, 720),
        )
        assertEquals(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg",
            "https://i.ytimg.com/vi_webp/dQw4w9WgXcQ/sddefault.webp".artworkAt(720),
        )
        assertEquals(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg".artworkAt(480),
        )
        assertEquals(
            "https://lh3.googleusercontent.com/abc=s720",
            "https://lh3.googleusercontent.com/abc=s120".artworkAt(720),
        )
        assertEquals(
            "https://lh3.googleusercontent.com/abc=w480-h640-p-l90-rj",
            "https://lh3.googleusercontent.com/abc=w120-h120-l90-rj".croppedAt(480, 640),
        )
        assertEquals(
            "https://example.com/art.jpg_480x480",
            artworkMemoryCacheKey("https://example.com/art.jpg", 480),
        )
        assertNull(artworkMemoryCacheKey(" ", 480))
        assertNull((null as String?).artworkAt(480))
    }

    @Test
    fun moodAnimationsAreDistinctAcrossNamedMoodsEspeciallyBlueVsHeartbreak() {
        val feelingBlue = resolveMoodTheme("Feeling Blue", 0)
        val heartbreak = resolveMoodTheme("Heartbreak", 1)
        assertEquals(MoodTheme.MELANCHOLY, feelingBlue)
        assertEquals(MoodTheme.HEARTBREAK, heartbreak)
        assertNotEquals(feelingBlue, heartbreak)

        assertEquals(MoodTheme.ENERGY, resolveMoodTheme("Energy", 0))
        assertEquals(MoodTheme.ENERGY, resolveMoodTheme("Workout", 1))
        assertEquals(MoodTheme.CHILL, resolveMoodTheme("Relax", 2))
        assertEquals(MoodTheme.FOCUS, resolveMoodTheme("Focus", 3))
        assertEquals(MoodTheme.ROMANCE, resolveMoodTheme("Romance", 4))
        assertEquals(MoodTheme.SLEEP, resolveMoodTheme("Sleep", 5))
        assertEquals(MoodTheme.PARTY, resolveMoodTheme("Party", 6))
        assertEquals(MoodTheme.COMMUTE, resolveMoodTheme("Commute", 7))
        assertEquals(MoodTheme.FEEL_GOOD, resolveMoodTheme("Feel Good", 8))
        assertEquals(MoodTheme.FOLK_ACOUSTIC, resolveMoodTheme("Folk & Acoustic", 9))
        assertEquals(MoodTheme.URBAN_PRISM, resolveMoodTheme("Hip-Hop & R&B", 10))
        assertEquals(MoodTheme.BOTANICAL_INDIE, resolveMoodTheme("Indie & Alt", 11))
        assertEquals(MoodTheme.HARMONIC_JAZZ, resolveMoodTheme("Jazz Classics", 12))
        assertEquals(MoodTheme.SYNTH_WAVE, resolveMoodTheme("Electronic Gaming", 13))

        // Unknown mood titles deterministically scatter by title + index.
        val firstFallback = resolveMoodTheme("Celestial Wander", 0)
        val secondFallback = resolveMoodTheme("Celestial Wander", 1)
        assertTrue(firstFallback in MoodTheme.entries)
        assertNotEquals(firstFallback, secondFallback)
    }
}
