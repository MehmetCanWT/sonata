package dev.sonata.android

import androidx.media3.common.Player
import dev.sonata.android.playback.EndedStateTransitionGuard
import dev.sonata.android.playback.PlaybackOrchestrationController
import dev.sonata.android.playback.RateLimitPolicy
import dev.sonata.android.playback.StreamSwapPlayer
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.SonataAndroidException

@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackOrchestrationTest {

    private class FakeSwapPlayer : StreamSwapPlayer {
        data class SwapCall(
            val trackId: String,
            val url: String,
            val badge: String,
            val positionMs: Long,
        )

        override var currentPositionMs: Long = 0L
        override var isPlaying: Boolean = false
        override var playWhenReady: Boolean = false

        val swapHistory = mutableListOf<SwapCall>()
        var prepareCalls: Int = 0
        var playCalls: Int = 0
        var pauseCalls: Int = 0

        override fun replaceMediaItem(
            track: AndroidTrack,
            stream: AndroidResolvedStream,
            positionMs: Long,
        ) {
            currentPositionMs = positionMs
            swapHistory += SwapCall(track.id, stream.url, stream.badge, positionMs)
        }

        override fun prepare() {
            prepareCalls += 1
        }

        override fun play() {
            playCalls += 1
            isPlaying = true
            playWhenReady = true
        }

        override fun pause() {
            pauseCalls += 1
            isPlaying = false
            playWhenReady = false
        }
    }

    private fun track(id: String, title: String = "Track $id") = AndroidTrack(
        id = id,
        title = title,
        artist = "Artist",
        album = "Album",
        durationSecs = 180uL,
        thumbnailUrl = null,
        liked = false,
        albumId = null,
        isExplicit = true,
    )

    private fun stream(url: String, badge: String) = AndroidResolvedStream(
        url = url,
        badge = badge,
        headers = emptyList(),
    )

    @Test
    fun playA_requestBBeforeAResolves_AFinishesLast_neverOverwritesB() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")
        val trackB = track("B")

        val gateA = CompletableDeferred<AndroidResolvedStream>()
        val gateB = CompletableDeferred<AndroidResolvedStream>()
        gateway.resolveStreamHandler = { t ->
            when (t.id) {
                "A" -> gateA.await()
                "B" -> gateB.await()
                else -> stream("https://stream.test/${t.id}", "OPUS 137kbps · HQ")
            }
        }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA, trackB), attemptHifiUpgrade = false)
        testScheduler.runCurrent()

        orchestrator.playTrack(trackB, listOf(trackA, trackB), attemptHifiUpgrade = false)
        testScheduler.runCurrent()

        gateB.complete(stream("https://stream.test/B_fast", "OPUS 160kbps · HQ"))
        advanceUntilIdle()

        // Complete stale A after B is already playing.
        gateA.complete(stream("https://stream.test/A_stale", "AAC 128kbps · Standard"))
        advanceUntilIdle()

        assertEquals("B", orchestrator.state.value.activeTrack?.id)
        assertEquals("https://stream.test/B_fast", orchestrator.state.value.activeStream?.url)
        assertEquals(listOf("B"), player.swapHistory.map { it.trackId })
    }

    @Test
    fun replayA_whileOldAUpgradeRunning_oldUpgradeIsRejected() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        val oldUpgradeStarted = CompletableDeferred<Unit>()
        val oldUpgradeGate = CompletableDeferred<AndroidResolvedStream?>()
        var upgradeCalls = 0

        gateway.resolveStreamHandler = {
            stream("https://stream.test/A_base", "AAC 128kbps · Standard")
        }
        gateway.resolveHifiUpgradeHandler = { _, _ ->
            upgradeCalls += 1
            if (upgradeCalls == 1) {
                oldUpgradeStarted.complete(Unit)
                oldUpgradeGate.await()
            } else {
                null
            }
        }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        val gen1 = orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = true)
        oldUpgradeStarted.await()

        // Replay the exact same track A while gen1's upgrade is still in flight.
        val gen2 = orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = false)
        advanceUntilIdle()

        assertTrue(gen2 > gen1)

        // Complete gen1's upgrade late — must NOT overwrite gen2's stream or badge.
        oldUpgradeGate.complete(stream("https://stream.test/A_old_upgrade", "OPUS 256kbps · Extreme"))
        advanceUntilIdle()

        assertEquals(gen2, orchestrator.state.value.generation)
        assertEquals("https://stream.test/A_base", orchestrator.state.value.activeStream?.url)
        assertEquals("AAC 128kbps · Standard", orchestrator.state.value.streamBadge)
        assertEquals(1, gateway.cancelledUpgradeCount.get())
    }

    @Test
    fun cancelSearchAndLyricsDuringNativeWork_dropsWorkAndPreventsStaleWrites() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        val searchStarted = CompletableDeferred<Unit>()
        val searchGate = CompletableDeferred<List<AndroidTrack>>()
        val lyricsStarted = CompletableDeferred<Unit>()
        val lyricsGate = CompletableDeferred<List<AndroidLyricLine>>()

        gateway.searchHandler = {
            searchStarted.complete(Unit)
            searchGate.await()
        }
        gateway.lyricsHandler = {
            lyricsStarted.complete(Unit)
            lyricsGate.await()
        }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = false)
        advanceUntilIdle()

        orchestrator.search("slow query")
        orchestrator.fetchLyrics(trackA)
        searchStarted.await()
        lyricsStarted.await()

        orchestrator.cancelSearch()
        orchestrator.cancelLyrics()
        advanceUntilIdle()

        searchGate.complete(listOf(track("stale_result")))
        lyricsGate.complete(
            listOf(AndroidLyricLine(1.0, "Stale lyric", emptyList(), isCounterVoice = false))
        )
        advanceUntilIdle()

        assertEquals(1, gateway.cancelledSearchCount.get())
        assertEquals(1, gateway.cancelledLyricsCount.get())
        assertTrue(orchestrator.state.value.searchResults.isEmpty())
        assertTrue(orchestrator.state.value.lyrics.isEmpty())
    }

    @Test
    fun recoverySuccess_restoresExactSavedPositionAndPlayIntent() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        gateway.resolveStreamHandler = { stream("https://stream.test/A_v1", "OPUS 137kbps · HQ") }
        gateway.resolveRecoveryHandler = { stream("https://stream.test/A_recovered", "OPUS 256kbps · Extreme") }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = false)
        advanceUntilIdle()

        // Advance playback to 42_750 ms while playing, then simulate a player error.
        player.currentPositionMs = 42_750L
        player.isPlaying = true
        player.playWhenReady = true

        assertTrue(orchestrator.onPlaybackError("HTTP 403 expired"))
        advanceUntilIdle()

        val lastSwap = player.swapHistory.last()
        assertEquals("https://stream.test/A_recovered", lastSwap.url)
        assertEquals(42_750L, lastSwap.positionMs)
        assertTrue(player.isPlaying)
        assertEquals("OPUS 256kbps · Extreme", orchestrator.state.value.streamBadge)
    }

    @Test
    fun recoveryExhaustion_stopsAfterMaxAttempts_andReplayGetsFreshBudget() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        gateway.resolveStreamHandler = { stream("https://stream.test/A_base", "OPUS 137kbps · HQ") }
        gateway.resolveRecoveryHandler = { throw SonataAndroidException.Offline("network lost") }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        val gen1 = orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = false)
        advanceUntilIdle()

        assertTrue(orchestrator.onPlaybackError("err1"))
        advanceUntilIdle()
        assertEquals(1, orchestrator.recoveryBudget.attemptsFor(gen1))

        assertTrue(orchestrator.onPlaybackError("err2"))
        advanceUntilIdle()
        assertEquals(2, orchestrator.recoveryBudget.attemptsFor(gen1))

        // 3rd attempt in the same generation must be rejected (exhausted).
        assertFalse(orchestrator.onPlaybackError("err3"))
        assertTrue(orchestrator.state.value.status.startsWith("Playback stopped:"))

        // Replaying the same track A creates a new generation with a fresh recovery budget!
        gateway.resolveRecoveryHandler = { stream("https://stream.test/A_fresh", "OPUS 256kbps · Extreme") }
        val gen2 = orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = false)
        advanceUntilIdle()

        assertEquals(0, orchestrator.recoveryBudget.attemptsFor(gen2))
        assertTrue(orchestrator.onPlaybackError("err_after_replay"))
        advanceUntilIdle()
        assertEquals(1, orchestrator.recoveryBudget.attemptsFor(gen2))
        assertEquals("https://stream.test/A_fresh", orchestrator.state.value.activeStream?.url)
    }

    @Test
    fun hifiUpgrade_preservesPausedState() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        val upgradeGate = CompletableDeferred<AndroidResolvedStream?>()
        gateway.resolveStreamHandler = { stream("https://stream.test/A_base", "AAC 128kbps · Standard") }
        gateway.resolveHifiUpgradeHandler = { _, _ -> upgradeGate.await() }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = true)
        advanceUntilIdle()

        // User pauses playback at 18_500 ms before the Hi-Fi upgrade finishes.
        player.currentPositionMs = 18_500L
        player.pause()

        upgradeGate.complete(stream("https://stream.test/A_hifi", "OPUS 256kbps · Extreme"))
        advanceUntilIdle()

        val lastSwap = player.swapHistory.last()
        assertEquals("https://stream.test/A_hifi", lastSwap.url)
        assertEquals(18_500L, lastSwap.positionMs)
        assertFalse("Hi-Fi upgrade must NOT resume playback when user paused", player.isPlaying)
        assertFalse(orchestrator.state.value.isPlaying)
        assertEquals("OPUS 256kbps · Extreme", orchestrator.state.value.streamBadge)
    }

    @Test
    fun hifiUpgrade_preservesPlayingStateAndPosition() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        val upgradeGate = CompletableDeferred<AndroidResolvedStream?>()
        gateway.resolveStreamHandler = { stream("https://stream.test/A_base", "AAC 128kbps · Standard") }
        gateway.resolveHifiUpgradeHandler = { _, _ -> upgradeGate.await() }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = true)
        advanceUntilIdle()

        player.currentPositionMs = 31_200L
        player.isPlaying = true
        player.playWhenReady = true

        upgradeGate.complete(stream("https://stream.test/A_hifi", "OPUS 256kbps · Extreme"))
        advanceUntilIdle()

        val lastSwap = player.swapHistory.last()
        assertEquals("https://stream.test/A_hifi", lastSwap.url)
        assertEquals(31_200L, lastSwap.positionMs)
        assertTrue(player.isPlaying)
        assertTrue(orchestrator.state.value.isPlaying)
        assertEquals("OPUS 256kbps · Extreme", orchestrator.state.value.streamBadge)
    }

    @Test
    fun failedUpgrade_leavesBaseStreamRunningUntouched() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val trackA = track("A")

        gateway.resolveStreamHandler = { stream("https://stream.test/A_base", "OPUS 137kbps · HQ") }
        gateway.resolveHifiUpgradeHandler = { _, _ -> throw SonataAndroidException.Timeout("timed out") }

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(trackA, listOf(trackA), attemptHifiUpgrade = true)
        advanceUntilIdle()

        assertEquals(1, player.swapHistory.size)
        assertEquals("https://stream.test/A_base", orchestrator.state.value.activeStream?.url)
        assertEquals("OPUS 137kbps · HQ", orchestrator.state.value.streamBadge)
        assertTrue(player.isPlaying)
        assertFalse(orchestrator.state.value.isUpgradingQuality)
    }

    @Test
    fun endOfTrackWithNextItem_keepsForegroundAliveAcrossFourTrackBoundaries() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val guard = EndedStateTransitionGuard()
        val queue = listOf(track("T1"), track("T2"), track("T3"), track("T4"), track("T5"))

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            endedGuard = guard,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )

        orchestrator.playTrack(queue[0], queue, attemptHifiUpgrade = false)
        advanceUntilIdle()
        assertEquals("T1", orchestrator.state.value.activeTrack?.id)

        // Cross 4 consecutive track boundaries: T1->T2->T3->T4->T5.
        for (expectedIdx in 1..4) {
            val reportedState = orchestrator.onUnderlyingPlaybackEnded()
            assertEquals(
                "Pending next track must keep foreground service in STATE_BUFFERING, not STATE_ENDED",
                Player.STATE_BUFFERING,
                reportedState,
            )
            assertTrue(guard.isSuppressingEnded)
            advanceUntilIdle()
            assertEquals(queue[expectedIdx].id, orchestrator.state.value.activeTrack?.id)
            assertEquals(expectedIdx, orchestrator.state.value.queueIndex)
            assertFalse("Suppression clears once next track loads", guard.isSuppressingEnded)
            assertTrue(player.isPlaying)
        }
    }

    @Test
    fun genuineEndOfQueue_reachesStateEndedAndReleasesForegroundState() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val guard = EndedStateTransitionGuard()
        val onlyTrack = track("FINAL")

        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            endedGuard = guard,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
        )
        orchestrator.setQueueOptions(
            repeatMode = Player.REPEAT_MODE_OFF,
            shuffleEnabled = false,
            autoplayEnabled = false,
        )

        orchestrator.playTrack(onlyTrack, listOf(onlyTrack), attemptHifiUpgrade = false)
        advanceUntilIdle()

        val reportedState = orchestrator.onUnderlyingPlaybackEnded()
        assertEquals(Player.STATE_ENDED, reportedState)
        assertFalse(guard.isSuppressingEnded)
        assertFalse(orchestrator.state.value.isPlaying)

        // Also verify pause / stop / clear unconditionally drop suppression.
        guard.onUnderlyingPlaybackStateChanged(Player.STATE_ENDED, hasPendingNextTransition = true)
        assertTrue(guard.isSuppressingEnded)
        guard.onPlayWhenReadyChanged(false)
        assertFalse(guard.isSuppressingEnded)
    }

    @Test
    fun shuffleNextPrevious_keepQueueIndexCorrect() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val gateway = FakeSonataEngineGateway()
        val player = FakeSwapPlayer()
        val queue = listOf(track("Q0"), track("Q1"), track("Q2"), track("Q3"))

        // Deterministic RNG: always proposes index 2 first, then index 2 again (which must wrap to 3 when already at 2).
        val randomSeq = ArrayDeque(listOf(2, 2))
        val orchestrator = PlaybackOrchestrationController(
            scope = this,
            gateway = gateway,
            player = player,
            ioDispatcher = dispatcher,
            mainDispatcher = dispatcher,
            randomInt = { bound -> randomSeq.removeFirstOrNull()?.rem(bound) ?: 0 },
        )

        orchestrator.playTrack(queue[0], queue, attemptHifiUpgrade = false)
        advanceUntilIdle()
        assertEquals(0, orchestrator.state.value.queueIndex)

        // Sequential next: 0 -> 1.
        assertTrue(orchestrator.nextTrack())
        advanceUntilIdle()
        assertEquals(1, orchestrator.state.value.queueIndex)
        assertEquals("Q1", orchestrator.state.value.activeTrack?.id)

        // Previous at position 0 ms: 1 -> 0.
        player.currentPositionMs = 500L
        assertTrue(orchestrator.prevTrack())
        advanceUntilIdle()
        assertEquals(0, orchestrator.state.value.queueIndex)

        // Previous past 3s restarts current track (does not change index).
        player.currentPositionMs = 5_000L
        assertFalse(orchestrator.prevTrack())
        assertEquals(0, orchestrator.state.value.queueIndex)

        // Shuffle enabled: 0 -> 2, then 2 -> wraps to 3 (never repeats current index when queue.size > 1).
        orchestrator.setQueueOptions(shuffleEnabled = true)
        assertTrue(orchestrator.nextTrack())
        advanceUntilIdle()
        assertEquals(2, orchestrator.state.value.queueIndex)

        assertTrue(orchestrator.nextTrack())
        advanceUntilIdle()
        assertEquals(3, orchestrator.state.value.queueIndex)
        assertEquals("Q3", orchestrator.state.value.activeTrack?.id)
    }

    @Test
    fun teardown_resetsStaticQueueBridgeWithNoRetainedCallbacks() {
        PlaybackQueueBridge.reset()
        assertFalse(PlaybackQueueBridge.hasActiveCallbacks())

        val sentinel = Any()
        PlaybackQueueBridge.canSkipNext = { sentinel.hashCode() != 0 }
        PlaybackQueueBridge.canSkipPrevious = { sentinel.hashCode() != 0 }
        PlaybackQueueBridge.shouldAutoAdvanceOnEnd = { sentinel.hashCode() != 0 }
        PlaybackQueueBridge.onSkipNext = { sentinel.toString() }
        PlaybackQueueBridge.onSkipPrevious = { sentinel.toString() }
        PlaybackQueueBridge.onPlayFromEmpty = { sentinel.toString() }
        PlaybackQueueBridge.onTrackEnded = { sentinel.toString() }
        assertTrue(PlaybackQueueBridge.hasActiveCallbacks())

        PlaybackQueueBridge.reset()
        assertFalse("Teardown must release every static PlaybackQueueBridge closure", PlaybackQueueBridge.hasActiveCallbacks())
        assertFalse(PlaybackQueueBridge.canSkipNext())
        assertFalse(PlaybackQueueBridge.canSkipPrevious())
        assertFalse(PlaybackQueueBridge.shouldAutoAdvanceOnEnd())
    }

    @Test
    fun rateLimitPolicy_dependsOnTypedFfiErrorNotMessageSubstrings() {
        val typedWithRetry = SonataAndroidException.RateLimited(
            retryAfterSecs = 90uL,
            reason = "slow down",
        )
        assertEquals(90_000L, RateLimitPolicy.backoffMsFor(typedWithRetry, defaultBackoffMs = 60_000L))

        val typedDefault = SonataAndroidException.RateLimited(
            retryAfterSecs = null,
            reason = "slow down",
        )
        assertEquals(60_000L, RateLimitPolicy.backoffMsFor(typedDefault, defaultBackoffMs = 60_000L))

        // Generic exception containing "429" substring must NOT trigger rate-limit backoff.
        val generic429 = RuntimeException("HTTP 429 too many requests")
        assertNull(RateLimitPolicy.backoffMsFor(generic429, defaultBackoffMs = 60_000L))
    }

    @Test
    fun gateway_endlessRadioQueue_receivesSeedPlaylistExcludeAndPreferredArtists() = runTest {
        val gateway = FakeSonataEngineGateway()
        var capturedSeeds: List<String> = emptyList()
        var capturedPlaylistId: String? = null
        var capturedExcludes: List<String> = emptyList()
        var capturedPreferredArtists: List<String> = emptyList()

        gateway.endlessRadioHandler = { seeds, playlistId, excludes, artists ->
            capturedSeeds = seeds
            capturedPlaylistId = playlistId
            capturedExcludes = excludes
            capturedPreferredArtists = artists
            listOf(track("R1"), track("R2"))
        }

        val result = gateway.fetchEndlessRadioQueue(
            seedVideoIds = listOf("S1", "S2"),
            playlistId = "PL123",
            excludeVideoIds = listOf("E1"),
            preferredArtists = listOf("Artist A", "Artist B"),
        )

        assertEquals(listOf("R1", "R2"), result.map { it.id })
        assertEquals(listOf("S1", "S2"), capturedSeeds)
        assertEquals("PL123", capturedPlaylistId)
        assertEquals(listOf("E1"), capturedExcludes)
        assertEquals(listOf("Artist A", "Artist B"), capturedPreferredArtists)
    }
}
