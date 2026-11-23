package dev.sonata.android

import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.runSuspendCatching
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.SonataAndroidException

class SonataEngineGatewayCancellationTest {

    private fun sampleTrack(id: String, title: String = "Track $id") = AndroidTrack(
        id = id,
        title = title,
        artist = "Artist",
        album = "Album",
        durationSecs = 200uL,
        thumbnailUrl = null,
        liked = false,
        albumId = null,
        isExplicit = false,
    )

    @Test
    fun runSuspendCatching_rethrowsCancellationAndCapturesTypedNativeErrors() = runTest {
        var cancellationRethrown = false
        try {
            runSuspendCatching { throw CancellationException("cancelled") }
        } catch (_: CancellationException) {
            cancellationRethrown = true
        }
        assertTrue("CancellationException must never be swallowed", cancellationRethrown)

        val rateLimited = runSuspendCatching<Unit> {
            throw SonataAndroidException.RateLimited(retryAfterSecs = 60uL, reason = "HTTP 429")
        }
        assertTrue(rateLimited.isFailure)
        val ex = rateLimited.exceptionOrNull() as SonataAndroidException.RateLimited
        assertEquals(60uL, ex.retryAfterSecs)
    }

    @Test
    fun cancellingSearchDropsInFlightGatewayCallAndPreventsStaleWrite() = runTest {
        val gateway = FakeSonataEngineGateway()
        val started = CompletableDeferred<Unit>()
        val gate = CompletableDeferred<List<AndroidTrack>>()
        gateway.searchHandler = {
            started.complete(Unit)
            gate.await()
        }

        val resultsState = MutableStateFlow<List<AndroidTrack>>(emptyList())
        val searchJob = launch {
            runSuspendCatching { gateway.searchSongs("first query") }
                .onSuccess { resultsState.value = it }
        }

        started.await()
        searchJob.cancelAndJoin()
        gate.complete(listOf(sampleTrack("stale_search")))

        assertEquals(1, gateway.cancelledSearchCount.get())
        assertTrue("Cancelled search must not write stale tracks", resultsState.value.isEmpty())
    }

    @Test
    fun cancellingLyricsDropsInFlightGatewayCallAndPreventsStaleWrite() = runTest {
        val gateway = FakeSonataEngineGateway()
        val started = CompletableDeferred<Unit>()
        val gate = CompletableDeferred<List<AndroidLyricLine>>()
        gateway.lyricsHandler = {
            started.complete(Unit)
            gate.await()
        }

        val lyricsState = MutableStateFlow<List<AndroidLyricLine>>(emptyList())
        val lyricsJob = launch {
            val fetched = runSuspendCatching { gateway.fetchLyrics(sampleTrack("t1")) }
                .getOrDefault(emptyList())
            lyricsState.value = fetched
        }

        started.await()
        lyricsJob.cancelAndJoin()
        gate.complete(
            listOf(
                AndroidLyricLine(
                    timeSecs = 1.0,
                    text = "Stale lyric line",
                    words = emptyList(),
                    isCounterVoice = false,
                )
            )
        )

        assertEquals(1, gateway.cancelledLyricsCount.get())
        assertTrue("Cancelled lyrics fetch must not write stale lines", lyricsState.value.isEmpty())
    }

    @Test
    fun cancellingCanvasPreventsStaleCanvasWrite() = runTest {
        val started = CompletableDeferred<Unit>()
        val gate = CompletableDeferred<CanvasMedia?>()
        val canvasState = MutableStateFlow<CanvasMedia?>(null)

        val canvasJob = launch {
            val resolved = runSuspendCatching {
                started.complete(Unit)
                gate.await()
            }.getOrNull()
            canvasState.value = resolved
        }

        started.await()
        canvasJob.cancelAndJoin()
        gate.complete(CanvasMedia(url = "https://cdn.test/stale.mp4", isHls = false, source = "Test"))

        assertNull("Cancelled canvas resolve must not write stale CanvasMedia", canvasState.value)
    }

    @Test
    fun cancellingStreamResolutionAndRecoveryPreventsStaleDelivery() = runTest {
        val gateway = FakeSonataEngineGateway()
        val resolveStarted = CompletableDeferred<Unit>()
        val resolveGate = CompletableDeferred<AndroidResolvedStream>()
        val recoveryStarted = CompletableDeferred<Unit>()
        val recoveryGate = CompletableDeferred<AndroidResolvedStream>()

        gateway.resolveStreamHandler = {
            resolveStarted.complete(Unit)
            resolveGate.await()
        }
        gateway.resolveRecoveryHandler = {
            recoveryStarted.complete(Unit)
            recoveryGate.await()
        }

        val deliveredStream = MutableStateFlow<AndroidResolvedStream?>(null)
        val track = sampleTrack("stream_1")

        val streamJob = launch {
            runSuspendCatching { gateway.resolveStream(track) }
                .onSuccess { deliveredStream.value = it }
        }
        resolveStarted.await()
        streamJob.cancelAndJoin()
        resolveGate.complete(
            AndroidResolvedStream(url = "https://stale.test/fast", badge = "AAC 128kbps", headers = emptyList())
        )

        val recoveryJob = launch {
            runSuspendCatching { gateway.resolveRecovery(track) }
                .onSuccess { deliveredStream.value = it }
        }
        recoveryStarted.await()
        recoveryJob.cancelAndJoin()
        recoveryGate.complete(
            AndroidResolvedStream(url = "https://stale.test/recovery", badge = "OPUS 256kbps", headers = emptyList())
        )

        assertEquals(1, gateway.cancelledResolveStreamCount.get())
        assertEquals(1, gateway.cancelledRecoveryCount.get())
        assertNull("Cancelled stream and recovery jobs must never deliver stale streams", deliveredStream.value)
    }
}
