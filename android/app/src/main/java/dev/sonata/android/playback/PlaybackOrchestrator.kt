package dev.sonata.android.playback

import androidx.media3.common.Player
import dev.sonata.android.data.SonataEngineGateway
import dev.sonata.android.data.runSuspendCatching
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidResolvedStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.SonataAndroidException
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

/**
 * Single monotonic playback generation counter shared across play, stream resolve,
 * explicit-track resolve, recovery, upgrade, lyrics, canvas, and prefetch flows.
 */
class PlaybackGenerationCounter(initial: Long = 0L) {
    private val counter = AtomicLong(initial)

    val current: Long
        get() = counter.get()

    fun next(): Long = counter.incrementAndGet()

    fun isCurrent(generation: Long): Boolean = generation > 0L && counter.get() == generation
}

/**
 * Recovery attempt budget keyed strictly by playback generation so same-track replays
 * always start with a fresh budget while stale generations cannot burn active attempts.
 */
class RecoveryBudget(
    private val maxAttempts: Int = DEFAULT_MAX_ATTEMPTS,
) {
    private val lock = Any()
    private var trackedGeneration: Long = 0L
    private var attempts: Int = 0

    fun consumeAttempt(generation: Long): Int? = synchronized(lock) {
        if (generation <= 0L) return null
        if (trackedGeneration != generation) {
            trackedGeneration = generation
            attempts = 0
        }
        if (attempts >= maxAttempts) {
            return null
        }
        attempts += 1
        attempts
    }

    fun attemptsFor(generation: Long): Int = synchronized(lock) {
        if (trackedGeneration == generation) attempts else 0
    }

    fun reset() = synchronized(lock) {
        trackedGeneration = 0L
        attempts = 0
    }

    companion object {
        const val DEFAULT_MAX_ATTEMPTS = 2
    }
}

/**
 * Typed rate-limit classification driven by [SonataAndroidException.RateLimited] instead of
 * brittle error-message substring matching.
 */
object RateLimitPolicy {
    fun backoffMsFor(err: Throwable, defaultBackoffMs: Long): Long? {
        val rateLimited = err as? SonataAndroidException.RateLimited ?: return null
        val retryAfterMs = rateLimited.retryAfterSecs?.toLong()?.times(1_000L)
        return if (retryAfterMs != null && retryAfterMs > 0L) {
            maxOf(retryAfterMs, defaultBackoffMs)
        } else {
            defaultBackoffMs
        }
    }
}

/**
 * Shared pure eligibility and queue navigation predicates used by both [dev.sonata.android.SonataViewModel]
 * and [dev.sonata.android.PlaybackService].
 */
object PlaybackEligibility {
    fun canAcceptGenerationResult(
        expectedGeneration: Long,
        currentGeneration: Long,
        expectedTrackId: String?,
        activeTrackId: String?,
    ): Boolean {
        if (expectedGeneration <= 0L || expectedGeneration != currentGeneration) return false
        if (expectedTrackId != null && activeTrackId != expectedTrackId) return false
        return true
    }

    fun hasNextInQueue(
        queueSize: Int,
        queueIndex: Int,
        repeatMode: Int,
        shuffleEnabled: Boolean,
    ): Boolean {
        if (queueSize <= 0) return false
        return when {
            shuffleEnabled && queueSize > 1 -> true
            queueIndex + 1 < queueSize -> true
            repeatMode == Player.REPEAT_MODE_ALL -> true
            else -> false
        }
    }

    fun resolveNextIndex(
        queueSize: Int,
        queueIndex: Int,
        repeatMode: Int,
        shuffleEnabled: Boolean,
        randomInt: (Int) -> Int = { bound -> Random.nextInt(bound) },
    ): Int? {
        if (queueSize <= 0) return null
        return when {
            shuffleEnabled && queueSize > 1 -> {
                var candidate = randomInt(queueSize).coerceIn(0, queueSize - 1)
                if (candidate == queueIndex) {
                    candidate = (candidate + 1) % queueSize
                }
                candidate
            }
            queueIndex + 1 < queueSize -> queueIndex + 1
            repeatMode == Player.REPEAT_MODE_ALL -> 0
            else -> null
        }
    }

    fun resolvePreviousIndex(
        queueSize: Int,
        queueIndex: Int,
        repeatMode: Int,
        currentPositionMs: Long,
        restartThresholdMs: Long = 3_000L,
    ): Int? {
        if (currentPositionMs > restartThresholdMs) return null
        if (queueSize <= 0) return null
        return when {
            queueIndex - 1 >= 0 -> queueIndex - 1
            repeatMode == Player.REPEAT_MODE_ALL -> queueSize - 1
            else -> null
        }
    }

    fun shouldAutoAdvanceOnEnd(
        hasNext: Boolean,
        repeatMode: Int,
        autoplayEnabled: Boolean,
        hasActiveTrack: Boolean,
        sleepAtEndOfTrack: Boolean,
    ): Boolean {
        if (sleepAtEndOfTrack) return false
        if (repeatMode == Player.REPEAT_MODE_ONE) return false
        return hasNext || (autoplayEnabled && hasActiveTrack)
    }

    fun selectUpcomingPrecacheCandidates(
        queue: List<AndroidTrack>,
        currentIndex: Int,
        lookaheadCount: Int = 2,
    ): List<AndroidTrack> {
        if (queue.isEmpty() || currentIndex !in queue.indices) return emptyList()
        return buildList {
            for (offset in 1..lookaheadCount) {
                val nextIdx = currentIndex + offset
                if (nextIdx in queue.indices) add(queue[nextIdx])
            }
        }.distinctBy { it.id }
    }
}

/**
 * Narrow ended-state transition guard for `PlaybackService`'s forwarding player.
 *
 * When a single-item ExoPlayer reaches `STATE_ENDED` while the ViewModel queue has a pending
 * next track (or autoplay), reporting `STATE_BUFFERING` until the next item loads prevents
 * Media3's `MediaSessionService` from demoting the foreground notification during background
 * or screen-locked track transitions.
 *
 * Suppression is cleared unconditionally on `STATE_READY`, `STATE_IDLE`, playback error,
 * pause (`playWhenReady == false`), stop, genuine end-of-queue, and service destruction.
 */
class EndedStateTransitionGuard {
    @Volatile
    private var suppressEnded: Boolean = false

    val isSuppressingEnded: Boolean
        get() = suppressEnded

    fun onUnderlyingPlaybackStateChanged(
        rawState: Int,
        hasPendingNextTransition: Boolean,
    ): Boolean {
        return when (rawState) {
            Player.STATE_ENDED -> {
                if (hasPendingNextTransition) {
                    suppressEnded = true
                    true
                } else {
                    suppressEnded = false
                    false
                }
            }
            Player.STATE_READY, Player.STATE_IDLE -> {
                suppressEnded = false
                false
            }
            else -> false
        }
    }

    fun effectivePlaybackState(rawState: Int): Int {
        return if (rawState == Player.STATE_ENDED && suppressEnded) {
            Player.STATE_BUFFERING
        } else {
            rawState
        }
    }

    fun onPlayWhenReadyChanged(playWhenReady: Boolean) {
        if (!playWhenReady) {
            suppressEnded = false
        }
    }

    fun clear() {
        suppressEnded = false
    }
}

/**
 * Minimal player abstraction allowing atomic stream swaps to follow one explicit contract
 * in both production `MediaController` playback and deterministic JVM tests.
 */
interface StreamSwapPlayer {
    val currentPositionMs: Long
    val isPlaying: Boolean
    val playWhenReady: Boolean

    fun replaceMediaItem(track: AndroidTrack, stream: AndroidResolvedStream, positionMs: Long)
    fun prepare()
    fun play()
    fun pause()
}

data class StreamSwapResult(
    val applied: Boolean,
    val positionMs: Long,
    val playIntent: Boolean,
)

/**
 * Named atomic stream swap contract:
 * 1. Verify the caller's generation and active track still match.
 * 2. Snapshot current position and play intent (unless overridden for recovery).
 * 3. Replace media item at that exact position and call `prepare()`.
 * 4. Restore play intent (`play()` if playing/intended, `pause()` if paused).
 * 5. Invoke [onCommitted] ONLY after the player swap succeeds so UI state never diverges.
 */
internal inline fun performAtomicStreamSwap(
    player: StreamSwapPlayer?,
    track: AndroidTrack,
    stream: AndroidResolvedStream,
    overridePositionMs: Long? = null,
    overridePlayIntent: Boolean? = null,
    isStillCurrent: () -> Boolean,
    onCommitted: (positionMs: Long, playIntent: Boolean) -> Unit,
): StreamSwapResult {
    if (!isStillCurrent() || player == null) {
        return StreamSwapResult(applied = false, positionMs = 0L, playIntent = false)
    }
    val snapshotPositionMs = (overridePositionMs ?: player.currentPositionMs).coerceAtLeast(0L)
    val snapshotPlayIntent = overridePlayIntent ?: (player.isPlaying || player.playWhenReady)
    player.replaceMediaItem(track, stream, snapshotPositionMs)
    player.prepare()
    if (snapshotPlayIntent) {
        player.play()
    } else {
        player.pause()
    }
    onCommitted(snapshotPositionMs, snapshotPlayIntent)
    return StreamSwapResult(
        applied = true,
        positionMs = snapshotPositionMs,
        playIntent = snapshotPlayIntent,
    )
}

/**
 * Snapshot state emitted by [PlaybackOrchestrationController] for deterministic unit testing
 * of multi-track races, same-track replays, recovery budgets, Hi-Fi swaps, and cancellations.
 */
data class OrchestratedPlaybackState(
    val generation: Long = 0L,
    val queue: List<AndroidTrack> = emptyList(),
    val queueIndex: Int = -1,
    val activeTrack: AndroidTrack? = null,
    val activeStream: AndroidResolvedStream? = null,
    val streamBadge: String = "",
    val isPlaying: Boolean = false,
    val isBuffering: Boolean = false,
    val isUpgradingQuality: Boolean = false,
    val repeatMode: Int = Player.REPEAT_MODE_OFF,
    val shuffleEnabled: Boolean = false,
    val autoplayEnabled: Boolean = false,
    val lyrics: List<AndroidLyricLine> = emptyList(),
    val searchResults: List<AndroidTrack> = emptyList(),
    val status: String = "",
) {
    val hasNext: Boolean
        get() = PlaybackEligibility.hasNextInQueue(queue.size, queueIndex, repeatMode, shuffleEnabled)
}

/**
 * Focused, Android-framework-free playback orchestrator exercising the exact same generation,
 * job cancellation, recovery budget, atomic stream swap, queue navigation, and foreground
 * ended-state transition rules used by `SonataViewModel` and `PlaybackService`.
 */
class PlaybackOrchestrationController(
    private val scope: CoroutineScope,
    private val gateway: SonataEngineGateway,
    private val player: StreamSwapPlayer,
    private val endedGuard: EndedStateTransitionGuard = EndedStateTransitionGuard(),
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.Default,
    private val mainDispatcher: CoroutineDispatcher = Dispatchers.Default,
    private val randomInt: (Int) -> Int = { bound -> Random.nextInt(bound) },
) {
    val generationCounter = PlaybackGenerationCounter()
    val recoveryBudget = RecoveryBudget()

    private val _state = MutableStateFlow(OrchestratedPlaybackState())
    val state: StateFlow<OrchestratedPlaybackState> = _state.asStateFlow()

    private var playJob: Job? = null
    private var upgradeJob: Job? = null
    private var recoveryJob: Job? = null
    private var lyricsJob: Job? = null
    private var searchJob: Job? = null
    private var autoplayJob: Job? = null

    fun setQueueOptions(
        repeatMode: Int = _state.value.repeatMode,
        shuffleEnabled: Boolean = _state.value.shuffleEnabled,
        autoplayEnabled: Boolean = _state.value.autoplayEnabled,
    ) {
        _state.update {
            it.copy(
                repeatMode = repeatMode,
                shuffleEnabled = shuffleEnabled,
                autoplayEnabled = autoplayEnabled,
            )
        }
    }

    fun playTrack(
        track: AndroidTrack,
        queueList: List<AndroidTrack>? = null,
        attemptHifiUpgrade: Boolean = true,
    ): Long {
        playJob?.cancel()
        upgradeJob?.cancel()
        recoveryJob?.cancel()
        autoplayJob?.cancel()
        lyricsJob?.cancel()

        val generation = generationCounter.next()
        val updatedQueue = when {
            !queueList.isNullOrEmpty() ->
                if (queueList.any { it.id == track.id }) queueList else listOf(track) + queueList
            _state.value.queue.any { it.id == track.id } -> _state.value.queue
            else -> listOf(track)
        }
        val updatedIndex = updatedQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

        _state.update {
            it.copy(
                generation = generation,
                queue = updatedQueue,
                queueIndex = updatedIndex,
                activeTrack = track,
                streamBadge = "Resolving...",
                isBuffering = true,
                isUpgradingQuality = false,
                lyrics = emptyList(),
                status = "Resolving ${track.title}...",
            )
        }

        playJob = scope.launch(ioDispatcher) {
            val (effectiveTrack, effectiveQueue, streamResult) = coroutineScope {
                val baseDeferred = async { runSuspendCatching { gateway.resolveStream(track) } }
                val explicitDeferred = if (!track.isExplicit && track.title.isNotBlank()) {
                    async {
                        withTimeoutOrNull(1_200L) {
                            runSuspendCatching { gateway.resolveExplicitCounterpart(track) }.getOrNull()
                        }
                    }
                } else {
                    null
                }
                val explicitMatch = explicitDeferred?.await()?.takeIf { it.id != track.id }
                if (explicitMatch != null) {
                    val upgradedQueue = updatedQueue.toMutableList().apply {
                        if (updatedIndex in indices) set(updatedIndex, explicitMatch)
                    }
                    if (isActive && generationCounter.isCurrent(generation)) {
                        _state.update {
                            if (it.generation == generation) {
                                it.copy(activeTrack = explicitMatch, queue = upgradedQueue)
                            } else {
                                it
                            }
                        }
                    }
                    val explicitRes = runSuspendCatching { gateway.resolveStream(explicitMatch) }
                    Triple(explicitMatch, upgradedQueue, explicitRes)
                } else {
                    Triple(track, updatedQueue, baseDeferred.await())
                }
            }

            if (!isActive || !generationCounter.isCurrent(generation)) return@launch

            streamResult
                .onSuccess { resolved ->
                    val swap = withContext(mainDispatcher) {
                        performAtomicStreamSwap(
                            player = player,
                            track = effectiveTrack,
                            stream = resolved,
                            overridePositionMs = 0L,
                            overridePlayIntent = true,
                            isStillCurrent = {
                                isActive && PlaybackEligibility.canAcceptGenerationResult(
                                    expectedGeneration = generation,
                                    currentGeneration = generationCounter.current,
                                    expectedTrackId = effectiveTrack.id,
                                    activeTrackId = _state.value.activeTrack?.id,
                                )
                            },
                            onCommitted = { _, playIntent ->
                                endedGuard.clear()
                                _state.update {
                                    it.copy(
                                        queue = effectiveQueue,
                                        activeTrack = effectiveTrack,
                                        activeStream = resolved,
                                        streamBadge = resolved.badge,
                                        isPlaying = playIntent,
                                        isBuffering = false,
                                        status = "Playing ${effectiveTrack.title}",
                                    )
                                }
                            },
                        )
                    }
                    if (swap.applied && attemptHifiUpgrade) {
                        launchQualityUpgrade(generation, effectiveTrack, resolved)
                    }
                }
                .onFailure { err ->
                    if (!isActive || !generationCounter.isCurrent(generation)) return@onFailure
                    endedGuard.clear()
                    _state.update {
                        it.copy(
                            isPlaying = false,
                            isBuffering = false,
                            isUpgradingQuality = false,
                            streamBadge = "Error",
                            status = "Play failed: ${err.message}",
                        )
                    }
                }
        }

        return generation
    }

    fun launchQualityUpgrade(
        generation: Long,
        track: AndroidTrack,
        baseStream: AndroidResolvedStream,
    ) {
        if (dev.sonata.android.data.isPersonalTrackId(track.id)) return
        upgradeJob?.cancel()
        _state.update {
            if (it.generation == generation && it.activeTrack?.id == track.id) {
                it.copy(isUpgradingQuality = true)
            } else {
                it
            }
        }
        upgradeJob = scope.launch(ioDispatcher) {
            try {
                val upgraded = runSuspendCatching {
                    gateway.resolveHifiUpgrade(track, baseStream)
                }.getOrNull() ?: return@launch

                if (!isActive || !PlaybackEligibility.canAcceptGenerationResult(
                        expectedGeneration = generation,
                        currentGeneration = generationCounter.current,
                        expectedTrackId = track.id,
                        activeTrackId = _state.value.activeTrack?.id,
                    )
                ) {
                    return@launch
                }

                withContext(mainDispatcher) {
                    performAtomicStreamSwap(
                        player = player,
                        track = track,
                        stream = upgraded,
                        isStillCurrent = {
                            isActive && PlaybackEligibility.canAcceptGenerationResult(
                                expectedGeneration = generation,
                                currentGeneration = generationCounter.current,
                                expectedTrackId = track.id,
                                activeTrackId = _state.value.activeTrack?.id,
                            )
                        },
                        onCommitted = { _, playIntent ->
                            _state.update {
                                it.copy(
                                    activeStream = upgraded,
                                    streamBadge = upgraded.badge,
                                    isPlaying = playIntent,
                                )
                            }
                        },
                    )
                }
            } finally {
                if (generationCounter.isCurrent(generation) && _state.value.activeTrack?.id == track.id) {
                    _state.update { it.copy(isUpgradingQuality = false) }
                }
            }
        }
    }

    fun onPlaybackError(reason: String = "Playback error"): Boolean {
        val snapshot = _state.value
        val active = snapshot.activeTrack ?: return false
        val generation = snapshot.generation
        val attempt = recoveryBudget.consumeAttempt(generation)
        if (attempt == null) {
            endedGuard.clear()
            _state.update {
                if (it.generation == generation) {
                    it.copy(
                        isPlaying = false,
                        isBuffering = false,
                        status = "Playback stopped: $reason",
                    )
                } else {
                    it
                }
            }
            return false
        }

        val resumePositionMs = player.currentPositionMs.coerceAtLeast(0L)
        val resumePlayIntent = player.isPlaying || player.playWhenReady || snapshot.isPlaying
        recoveryJob?.cancel()
        upgradeJob?.cancel()
        recoveryJob = scope.launch(ioDispatcher) {
            _state.update {
                if (it.generation == generation && it.activeTrack?.id == active.id) {
                    it.copy(isBuffering = true, status = "Recovering stream...")
                } else {
                    it
                }
            }
            runSuspendCatching { gateway.resolveRecovery(active) }
                .onSuccess { recovered ->
                    withContext(mainDispatcher) {
                        performAtomicStreamSwap(
                            player = player,
                            track = active,
                            stream = recovered,
                            overridePositionMs = resumePositionMs,
                            overridePlayIntent = resumePlayIntent,
                            isStillCurrent = {
                                isActive && PlaybackEligibility.canAcceptGenerationResult(
                                    expectedGeneration = generation,
                                    currentGeneration = generationCounter.current,
                                    expectedTrackId = active.id,
                                    activeTrackId = _state.value.activeTrack?.id,
                                )
                            },
                            onCommitted = { _, playIntent ->
                                endedGuard.clear()
                                _state.update {
                                    it.copy(
                                        activeStream = recovered,
                                        streamBadge = recovered.badge,
                                        isPlaying = playIntent,
                                        isBuffering = false,
                                        status = "Playing ${active.title}",
                                    )
                                }
                            },
                        )
                    }
                }
                .onFailure { err ->
                    if (!isActive || !generationCounter.isCurrent(generation)) return@onFailure
                    endedGuard.clear()
                    _state.update {
                        it.copy(
                            isPlaying = false,
                            isBuffering = false,
                            status = "Playback error: ${err.message}",
                        )
                    }
                }
        }
        return true
    }

    fun fetchLyrics(track: AndroidTrack, generation: Long = generationCounter.current) {
        lyricsJob?.cancel()
        lyricsJob = scope.launch(ioDispatcher) {
            val fetched = runSuspendCatching { gateway.fetchLyrics(track) }.getOrDefault(emptyList())
            if (isActive && PlaybackEligibility.canAcceptGenerationResult(
                    expectedGeneration = generation,
                    currentGeneration = generationCounter.current,
                    expectedTrackId = track.id,
                    activeTrackId = _state.value.activeTrack?.id,
                )
            ) {
                _state.update { it.copy(lyrics = fetched) }
            }
        }
    }

    fun search(query: String) {
        searchJob?.cancel()
        searchJob = scope.launch(ioDispatcher) {
            runSuspendCatching { gateway.searchSongs(query) }
                .onSuccess { results ->
                    if (isActive) {
                        _state.update { it.copy(searchResults = results) }
                    }
                }
        }
    }

    fun cancelSearch() {
        searchJob?.cancel()
        searchJob = null
    }

    fun cancelLyrics() {
        lyricsJob?.cancel()
        lyricsJob = null
    }

    fun nextTrack(): Boolean {
        val snapshot = _state.value
        val nextIdx = PlaybackEligibility.resolveNextIndex(
            queueSize = snapshot.queue.size,
            queueIndex = snapshot.queueIndex,
            repeatMode = snapshot.repeatMode,
            shuffleEnabled = snapshot.shuffleEnabled,
            randomInt = randomInt,
        ) ?: return false
        playTrack(snapshot.queue[nextIdx], snapshot.queue)
        return true
    }

    fun prevTrack(): Boolean {
        val snapshot = _state.value
        val prevIdx = PlaybackEligibility.resolvePreviousIndex(
            queueSize = snapshot.queue.size,
            queueIndex = snapshot.queueIndex,
            repeatMode = snapshot.repeatMode,
            currentPositionMs = player.currentPositionMs,
        )
        if (prevIdx == null) {
            return false
        }
        playTrack(snapshot.queue[prevIdx], snapshot.queue)
        return true
    }

    fun onUnderlyingPlaybackEnded(): Int {
        val snapshot = _state.value
        val shouldTransition = PlaybackEligibility.shouldAutoAdvanceOnEnd(
            hasNext = snapshot.hasNext,
            repeatMode = snapshot.repeatMode,
            autoplayEnabled = snapshot.autoplayEnabled,
            hasActiveTrack = snapshot.activeTrack != null,
            sleepAtEndOfTrack = false,
        )
        endedGuard.onUnderlyingPlaybackStateChanged(Player.STATE_ENDED, shouldTransition)
        val effective = endedGuard.effectivePlaybackState(Player.STATE_ENDED)
        if (shouldTransition) {
            if (!nextTrack() && snapshot.autoplayEnabled && snapshot.activeTrack != null) {
                val seed = snapshot.activeTrack
                val gen = snapshot.generation
                autoplayJob?.cancel()
                autoplayJob = scope.launch(ioDispatcher) {
                    val radio = runSuspendCatching { gateway.fetchRadioQueue(seed.id) }
                        .getOrDefault(emptyList())
                        .filter { cand -> snapshot.queue.none { it.id == cand.id } }
                    if (isActive && generationCounter.isCurrent(gen) && radio.isNotEmpty()) {
                        playTrack(radio.first(), snapshot.queue + radio)
                    } else if (isActive && generationCounter.isCurrent(gen)) {
                        endedGuard.clear()
                        _state.update { it.copy(isPlaying = false) }
                    }
                }
            }
        } else {
            _state.update { it.copy(isPlaying = false) }
        }
        return effective
    }

    fun teardown() {
        playJob?.cancel()
        playJob = null
        upgradeJob?.cancel()
        upgradeJob = null
        recoveryJob?.cancel()
        recoveryJob = null
        lyricsJob?.cancel()
        lyricsJob = null
        searchJob?.cancel()
        searchJob = null
        autoplayJob?.cancel()
        autoplayJob = null
        endedGuard.clear()
    }
}
