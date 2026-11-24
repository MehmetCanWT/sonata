/*
 * Now Playing header drag handle, Music/Video mode bar, progress scrubber,
 * transport controls, action cutouts, and live lyric preview strip.
 */
package dev.sonata.android.ui.player

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.Player
import dev.sonata.android.LocalNowPlayingExtras
import dev.sonata.android.PlaybackClockState
import dev.sonata.android.R
import dev.sonata.android.SonataUiState
import dev.sonata.android.SquirclePillShape
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.formatDurationMs
import dev.sonata.android.ui.LYRIC_SYNC_LEAD_MS
import dev.sonata.android.ui.LosslessLabel
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.SweptKaraokeLyricText
import dev.sonata.android.ui.ThinSlider
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.sonataThemeAccent
import kotlinx.coroutines.delay
import uniffi.sonata_android.AndroidLyricLine

@Composable
internal fun NowPlayingTopHandleAndModeBar(
    topHandleStripHeight: Dp,
    showModePill: Boolean,
    videoModeEnabled: Boolean,
    videoLoading: Boolean,
    onDismiss: () -> Unit,
    onSelectMusic: () -> Unit,
    onSelectVideo: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(topHandleStripHeight),
        contentAlignment = Alignment.TopCenter,
    ) {
        Box(
            modifier = Modifier
                .padding(top = 8.dp)
                .width(38.dp)
                .height(5.dp)
                .clip(CircleShape)
                .background(Color.White.copy(alpha = 0.45f))
                .clickable(onClick = onDismiss),
        )
    }

    if (showModePill) {
        Spacer(Modifier.height(4.dp))
        MusicVideoModePill(
            videoModeEnabled = videoModeEnabled,
            videoLoading = videoLoading,
            onSelectMusic = onSelectMusic,
            onSelectVideo = onSelectVideo,
        )
    }
}

@Composable
internal fun NowPlayingActionRowBlock(
    state: SonataUiState,
    showQueue: Boolean,
    compact: Boolean = false,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    val extras = LocalNowPlayingExtras.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ActionCutoutTile(
            active = state.showLyrics,
            onClick = onToggleLyrics,
        ) {
            Icon(
                imageVector = SonataIcons.LyricsQuote,
                contentDescription = LocalSonataStrings.current.lyrics,
                tint = Color.White,
                modifier = Modifier.size(26.dp),
            )
        }

        val themeAccent = sonataThemeAccent()
        if (showQueue) {
            Row(
                modifier = Modifier
                    .clip(SquirclePillShape())
                    .background(Color.White.copy(alpha = 0.14f))
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(
                    onClick = onToggleShuffle,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.Shuffle,
                        contentDescription = LocalSonataStrings.current.shuffle,
                        tint = if (state.shuffleEnabled) themeAccent else Color.White.copy(alpha = 0.72f),
                        modifier = Modifier.size(18.dp),
                    )
                }
                IconButton(
                    onClick = onToggleRepeat,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        imageVector = if (state.repeatMode == Player.REPEAT_MODE_ONE) {
                            SonataIcons.RepeatOne
                        } else {
                            SonataIcons.Repeat
                        },
                        contentDescription = when (state.repeatMode) {
                            Player.REPEAT_MODE_ALL -> LocalSonataStrings.current.repeatAll
                            Player.REPEAT_MODE_ONE -> LocalSonataStrings.current.repeatOne
                            else -> LocalSonataStrings.current.repeatOff
                        },
                        tint = if (state.repeatMode != Player.REPEAT_MODE_OFF) {
                            themeAccent
                        } else {
                            Color.White.copy(alpha = 0.72f)
                        },
                        modifier = Modifier.size(18.dp),
                    )
                }
                val autoplayEnabled = extras.settings.autoplayEnabled
                IconButton(
                    onClick = extras.onToggleAutoplay,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.Infinity,
                        contentDescription = LocalSonataStrings.current.autoplay,
                        tint = if (autoplayEnabled) themeAccent else Color.White.copy(alpha = 0.42f),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        } else {
            ActionCutoutTile(
                active = extras.settings.lyricsTranslationEnabled,
                onClick = extras.onToggleLyricsTranslation,
            ) {
                Icon(
                    imageVector = SonataIcons.Translate,
                    contentDescription = LocalSonataStrings.current.translateLyrics,
                    tint = if (extras.settings.lyricsTranslationEnabled) themeAccent else Color.White,
                    modifier = Modifier.size(23.dp),
                )
            }
        }

        ActionCutoutTile(
            active = showQueue,
            onClick = onToggleQueue,
        ) {
            Icon(
                imageVector = SonataIcons.Queue,
                contentDescription = LocalSonataStrings.current.queue,
                tint = Color.White,
                modifier = Modifier.size(26.dp),
            )
        }
    }

    Spacer(Modifier.height(if (compact) 4.dp else 8.dp))
}

/**
 * Narrowest leaf composable that subscribes to [PlaybackClockState.positionMs] & [PlaybackClockState.durationMs].
 * Keeps all 5Hz playback ticking completely isolated from NowPlayingScreen, layouts, and tabs.
 */
@Composable
internal fun NowPlayingProgressSection(
    clock: PlaybackClockState,
    isBuffering: Boolean,
    isUpgradingQuality: Boolean,
    streamBadge: String,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
) {
    var scrubbing by remember { mutableStateOf(false) }
    var scrubFraction by remember { mutableFloatStateOf(0f) }

    val durationMs = clock.durationMs
    val positionMs = clock.positionMs
    val playbackFraction = if (durationMs > 0L) {
        (positionMs.toFloat() / durationMs.toFloat()).coerceIn(0f, 1f)
    } else {
        0f
    }
    val displayedFraction = if (scrubbing) scrubFraction else playbackFraction
    val shownMs = (displayedFraction * durationMs).toLong()
    val remainingMs = (durationMs - shownMs).coerceAtLeast(0L)

    ThinSlider(
        value = displayedFraction,
        onValueChange = { frac ->
            if (!scrubbing) {
                scrubbing = true
                onScrubbingChanged(true)
            }
            scrubFraction = frac
        },
        onValueChangeFinished = {
            onSeekFraction(scrubFraction)
            scrubbing = false
            onScrubbingChanged(false)
        },
        loading = isBuffering,
    )

    val extras = LocalNowPlayingExtras.current
    val showTotalDuration = extras.settings.showTotalDuration
    val hideSongStatus = extras.settings.hideSongStatus
    val showNerdStats = extras.settings.showNerdStats

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .offset(y = (-6).dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = formatDurationMs(shownMs),
                style = MaterialTheme.typography.labelMedium,
                color = Color.White.copy(alpha = 0.58f),
            )
            Text(
                text = if (showTotalDuration) formatDurationMs(durationMs) else "-${formatDurationMs(remainingMs)}",
                style = MaterialTheme.typography.labelMedium,
                color = Color.White.copy(alpha = 0.58f),
            )
        }
        val showStatusPulse = isBuffering || isUpgradingQuality
        if (showStatusPulse || !hideSongStatus) {
            LosslessLabel(
                label = if (isUpgradingQuality) {
                    "Upgrading Quality"
                } else {
                    streamBadge.ifBlank { "Lossless" }
                },
                pulsing = showStatusPulse,
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = 6.dp),
            )
        }
    }
    if (showNerdStats) {
        val pipelineInfo by AudioPipelineBus.info.collectAsStateWithLifecycle()
        val statsText = remember(pipelineInfo, streamBadge) {
            val parts = buildList {
                if (pipelineInfo.codec.isNotBlank() && pipelineInfo.codec != "—") add(pipelineInfo.codec)
                if (pipelineInfo.bitrateKbps > 0) add("${pipelineInfo.bitrateKbps} kbps")
                if (pipelineInfo.sampleRateHz > 0) add(pipelineInfo.sampleRateLabel)
                if (pipelineInfo.channelCount > 0) add(pipelineInfo.channelLabel)
                if (pipelineInfo.fromOfflineDownload) add("Offline")
                else if (pipelineInfo.fromCache) add("Cached")
            }
            if (parts.isNotEmpty()) parts.joinToString(" · ") else streamBadge.ifBlank { pipelineInfo.sourceLabel }
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 2.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = statsText,
                style = MaterialTheme.typography.labelSmall.copy(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 10.5.sp,
                ),
                color = Color.White.copy(alpha = 0.62f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .clip(RoundedCornerShape(percent = 50))
                    .background(Color.White.copy(alpha = 0.10f))
                    .clickable(onClick = extras.onOpenAudioPipeline)
                    .padding(horizontal = 10.dp, vertical = 3.dp),
            )
        }
    }
}

@Composable
internal fun NowPlayingScrubberAndTransportBlock(
    state: SonataUiState,
    clock: PlaybackClockState,
    showQueue: Boolean,
    includeActions: Boolean = true,
    compactTransport: Boolean = false,
    transportTopGap: Dp = 14.dp,
    transportBottomGap: Dp = 18.dp,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onPrevious: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    NowPlayingProgressSection(
        clock = clock,
        isBuffering = state.isBuffering,
        isUpgradingQuality = state.isUpgradingQuality,
        streamBadge = state.streamBadge,
        onScrubbingChanged = onScrubbingChanged,
        onSeekFraction = onSeekFraction,
    )

    if (transportTopGap > 0.dp) {
        Spacer(Modifier.height(transportTopGap))
    }

    val hasPrevious by remember(state.activeTrack, state.repeatMode, state.queueIndex, clock) {
        derivedStateOf { state.hasPrevious(clock.positionMs) }
    }
    val sideButtonSize = if (compactTransport) 44.dp else 64.dp
    val sideIconSize = if (compactTransport) 34.dp else 53.dp
    val playButtonSize = if (compactTransport) 52.dp else 88.dp
    val playIconSize = if (compactTransport) 36.dp else 54.dp

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceAround,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(
            onClick = onPrevious,
            enabled = hasPrevious,
            modifier = Modifier.size(sideButtonSize),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_previous),
                contentDescription = LocalSonataStrings.current.previous,
                tint = if (hasPrevious) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(sideIconSize)
                    .graphicsLayer { scaleY = 0.85f },
            )
        }

        if (state.isBuffering) {
            Box(
                modifier = Modifier.size(playButtonSize),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(
                    color = Color.White,
                    strokeWidth = if (compactTransport) 2.5.dp else 3.dp,
                    modifier = Modifier.size(if (compactTransport) 26.dp else 38.dp),
                )
            }
        } else {
            IconButton(
                onClick = onTogglePlayPause,
                modifier = Modifier.size(playButtonSize),
            ) {
                Icon(
                    painter = painterResource(
                        if (state.isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play
                    ),
                    contentDescription = if (state.isPlaying) LocalSonataStrings.current.pause else LocalSonataStrings.current.play,
                    tint = Color.White,
                    modifier = Modifier.size(playIconSize),
                )
            }
        }

        IconButton(
            onClick = onNext,
            enabled = state.hasNext,
            modifier = Modifier.size(sideButtonSize),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_next),
                contentDescription = LocalSonataStrings.current.next,
                tint = if (state.hasNext) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(sideIconSize)
                    .graphicsLayer { scaleY = 0.85f },
            )
        }
    }

    if (includeActions) {
        if (transportBottomGap > 0.dp) {
            Spacer(Modifier.height(transportBottomGap))
        }
        NowPlayingActionRowBlock(
            state = state,
            showQueue = showQueue,
            onToggleLyrics = onToggleLyrics,
            onToggleQueue = onToggleQueue,
            onToggleShuffle = onToggleShuffle,
            onToggleRepeat = onToggleRepeat,
            onOpenSettings = onOpenSettings,
        )
    }
}

@Composable
internal fun ActionCutoutTile(
    active: Boolean,
    onClick: () -> Unit,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = Modifier
            .size(35.dp)
            .clip(RoundedCornerShape(7.dp))
            .background(if (active) Color.White.copy(alpha = 0.24f) else Color.Transparent)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
internal fun CurrentLyricStrip(
    trackId: String,
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    lyricsUnavailable: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    onClick: () -> Unit,
) {
    if (LocalNowPlayingExtras.current.settings.hideLyricsPreview) {
        Spacer(Modifier.height(10.dp))
        return
    }
    val durationMs = clock.durationMs
    var showUnavailable by remember(trackId) { mutableStateOf(false) }

    LaunchedEffect(trackId, lyricsUnavailable, lyricsLoading) {
        if (!lyricsLoading && lyricsUnavailable && lyrics.isEmpty()) {
            showUnavailable = true
            delay(3_000L)
            showUnavailable = false
        } else {
            showUnavailable = false
        }
    }

    val activeIndex by remember(lyrics) {
        derivedStateOf {
            val pos = clockMs.value + LYRIC_SYNC_LEAD_MS
            lyrics.indexOfLast { (it.timeSecs * 1000.0).toLong() <= pos }.coerceAtLeast(0)
        }
    }
    val stripAlpha by animateFloatAsState(
        targetValue = if (lyricsLoading || lyrics.isNotEmpty() || showUnavailable) 1f else 0f,
        animationSpec = tween(280),
        label = "lyricStripAlpha",
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(26.dp)
            .offset(y = 6.dp)
            .clipToBounds()
            .graphicsLayer { alpha = stripAlpha }
            .then(
                if (lyrics.isNotEmpty()) Modifier.clickable(onClick = onClick) else Modifier
            ),
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            lyricsLoading -> Text(
                text = LocalSonataStrings.current.loadingLyrics,
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.5.sp),
                color = Color.White.copy(alpha = 0.55f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )

            lyrics.isNotEmpty() -> {
                AnimatedContent(
                    targetState = activeIndex,
                    transitionSpec = {
                        fadeIn(tween(140, delayMillis = 80)) togetherWith fadeOut(tween(100))
                    },
                    label = "liveLyricStripLine",
                    modifier = Modifier
                        .fillMaxWidth()
                        .clipToBounds(),
                ) { idx ->
                    val activeLine = lyrics.getOrNull(idx)
                    if (activeLine != null) {
                        val cleanStripText = activeLine.text.trim()
                            .removePrefix("v2:").removePrefix("V2:")
                            .removePrefix("v1:").removePrefix("V1:")
                            .removePrefix("bg:").removePrefix("BG:")
                            .trim()
                        val lineStartMs = (activeLine.timeSecs * 1000.0).toLong().coerceAtLeast(0L)
                        val lineEndMs = lyrics.getOrNull(idx + 1)
                            ?.let { (it.timeSecs * 1000.0).toLong() }
                            ?.coerceAtLeast(lineStartMs + 400L)
                            ?: maxOf(durationMs, lineStartMs + 4_000L)
                        SweptKaraokeLyricText(
                            text = cleanStripText,
                            lineStartMs = lineStartMs,
                            lineEndMs = lineEndMs,
                            words = activeLine.words,
                            clockMs = clockMs,
                            style = MaterialTheme.typography.titleSmall.copy(
                                fontSize = 15.5.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                            dimAlpha = 0.42f,
                            uncommittedColor = Color(0xFF98989F).copy(alpha = 0.42f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }

            showUnavailable -> Text(
                text = LocalSonataStrings.current.noLyricsAvailable,
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.5.sp),
                color = Color.White.copy(alpha = 0.55f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * Leaf composable rendering the 3-dot instrumental interlude progress indicator.
 * Visibility is gated via `derivedStateOf` and dot lighting reads `clockMs.value` strictly
 * in the Canvas Draw phase so the parent lyric row never recomposes at 60/120fps.
 */
@Composable
internal fun InstrumentalBreakDots(
    lineStartMs: Long,
    nextLineStartMs: Long,
    cleanTextLength: Int,
    clockMs: State<Long>,
) {
    val breakStartMs = remember(lineStartMs, nextLineStartMs, cleanTextLength) {
        lineStartMs + minOf(
            (cleanTextLength.coerceAtLeast(3) * 65L).coerceIn(1_100L, 3_600L),
            ((nextLineStartMs - lineStartMs) * 55L / 100L),
        )
    }
    val visible by remember(breakStartMs, nextLineStartMs, clockMs) {
        derivedStateOf {
            val now = clockMs.value
            now >= breakStartMs && now < nextLineStartMs
        }
    }
    if (!visible) return

    Canvas(
        modifier = Modifier
            .padding(top = 12.dp, bottom = 4.dp)
            .width(46.dp)
            .height(10.dp),
    ) {
        val nowMs = clockMs.value
        val breakProgress = ((nowMs - breakStartMs).toFloat() /
            (nextLineStartMs - breakStartMs).coerceAtLeast(1L).toFloat()).coerceIn(0f, 1f)
        val centerY = size.height / 2f
        val spacing = 16.dp.toPx()
        val startX = 5.dp.toPx()
        for (dotIdx in 0 until 3) {
            val dotThreshold = (dotIdx + 1) / 3.2f
            val lit = breakProgress >= dotThreshold
            drawCircle(
                color = Color.White.copy(alpha = if (lit) 0.92f else 0.28f),
                radius = (if (lit) 4.5.dp else 3.5.dp).toPx(),
                center = Offset(startX + dotIdx * spacing, centerY),
            )
        }
    }
}
