/*
 * Enhanced Synced Lyrics Panel for Sonata Android.
 *
 * Wraps Sonata's SteeredLyricClock and SweptKaraokeLyricText with:
 * - Per-line spring lift and held-note glow/contrast emphasis (bounded <= 1.0f scale)
 * - Depth-of-field blur on inactive verses (API 31+ RenderEffect, alpha fallback below)
 * - On-demand secondary translation & romanization lines via LyricsTranslator
 * - Clean minimal Translate control pill inside Lyrics view
 */
package dev.sonata.android.ui.lyrics

import android.os.Build
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.PlaybackClockState
import dev.sonata.android.data.LyricsTranslator
import dev.sonata.android.ui.LYRIC_SYNC_LEAD_MS
import dev.sonata.android.ui.LyricsSkeleton
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SweptKaraokeLyricText
import uniffi.sonata_android.AndroidLyricLine
import kotlin.math.abs
import kotlin.math.roundToInt

@Composable
fun EnhancedLyricsPanel(
    trackId: String,
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    lyricsBlurEnabled: Boolean,
    translationEnabled: Boolean,
    translationLang: String,
    romanizationEnabled: Boolean,
    offsetMs: Long,
    sourceLabel: String = "Synced",
    onToggleTranslation: () -> Unit,
    onToggleRomanization: () -> Unit,
    onStepOffsetMs: (Long) -> Unit,
    onSeekTo: (Long) -> Unit,
    modifier: Modifier = Modifier,
    prioritizeWordSync: Boolean = dev.sonata.android.LocalNowPlayingExtras.current.settings.prioritizeWordSync,
) {
    if (lyricsLoading) {
        LyricsSkeleton(modifier = modifier.fillMaxSize())
        return
    }
    if (lyrics.isEmpty()) {
        Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(
                text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.noLyricsAvailable,
                style = MaterialTheme.typography.titleMedium,
                color = Color.White.copy(alpha = 0.62f),
                textAlign = TextAlign.Center,
            )
        }
        return
    }

    val reduceBlur = LocalReduceDynamicBlur.current
    val effectiveBlur = lyricsBlurEnabled && !reduceBlur && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    val density = LocalDensity.current
    val listState = rememberLazyListState()
    var panelHeightPx by remember { mutableIntStateOf(0) }
    var lastUserTouchMs by remember { mutableLongStateOf(0L) }

    val cleanedLines = remember(lyrics) {
        lyrics.map { line ->
            line.text.trim()
                .removePrefix("v2:").removePrefix("V2:")
                .removePrefix("v1:").removePrefix("V1:")
                .removePrefix("bg:").removePrefix("BG:")
                .trim()
        }
    }
    val hasDuets = remember(lyrics) {
        lyrics.any {
            val trimmed = it.text.trim()
            it.isCounterVoice ||
                trimmed.startsWith("v2:", ignoreCase = true) ||
                trimmed.startsWith("bg:", ignoreCase = true)
        }
    }

    // On-demand translation & romanization via LyricsTranslator
    var translatedLines by remember(trackId, translationLang) { mutableStateOf<List<String>?>(null) }
    var romanizedLines by remember(trackId) { mutableStateOf<List<String>?>(null) }

    LaunchedEffect(trackId, translationEnabled, translationLang, cleanedLines) {
        if (translationEnabled && cleanedLines.isNotEmpty() && translatedLines == null) {
            translatedLines = LyricsTranslator.translateLines(trackId, cleanedLines, translationLang)
        }
    }
    LaunchedEffect(trackId, romanizationEnabled, cleanedLines) {
        if (romanizationEnabled && cleanedLines.isNotEmpty() && romanizedLines == null) {
            romanizedLines = LyricsTranslator.romanizeLines(trackId, cleanedLines)
        }
    }

    // Offset-adjusted clock state read strictly inside derivedStateOf / draw
    val adjustedClockMs = remember(clockMs, offsetMs) {
        derivedStateOf { (clockMs.value - offsetMs).coerceAtLeast(0L) }
    }

    val activeIndex by remember(lyrics, cleanedLines) {
        derivedStateOf {
            val pos = adjustedClockMs.value + LYRIC_SYNC_LEAD_MS
            var cand = lyrics.indexOfLast { (it.timeSecs * 1000.0).toLong() <= pos }.coerceAtLeast(0)
            // Keep an overlapping lead verse anchored while a nested (...) ad-lib starts mid-verse.
            while (cand > 0) {
                val candClean = cleanedLines[cand]
                val candIsBg = candClean.startsWith("(") && candClean.endsWith(")") && candClean.length > 2
                if (!candIsBg) break
                val prev = lyrics[cand - 1]
                val prevClean = cleanedLines[cand - 1]
                val prevIsBg = prevClean.startsWith("(") && prevClean.endsWith(")") && prevClean.length > 2
                if (prevIsBg) break
                val prevEndMs = prev.words
                    .maxOfOrNull { ((it.endSecs ?: (it.startSecs + 0.45)) * 1000.0).toLong() }
                    ?: ((prev.timeSecs * 1000.0).toLong() + 1_200L)
                if (pos < prevEndMs) {
                    cand -= 1
                } else {
                    break
                }
            }
            cand
        }
    }

    LaunchedEffect(listState.isScrollInProgress) {
        if (listState.isScrollInProgress) {
            lastUserTouchMs = System.currentTimeMillis()
        }
    }

    LaunchedEffect(activeIndex, lyrics.size, panelHeightPx) {
        val now = System.currentTimeMillis()
        if (activeIndex !in lyrics.indices ||
            listState.isScrollInProgress ||
            (now - lastUserTouchMs) <= 2_400L
        ) {
            return@LaunchedEffect
        }
        val info = listState.layoutInfo
        val span = (info.viewportEndOffset - info.viewportStartOffset).coerceAtLeast(1)
        val anchor = info.viewportStartOffset + (span * 0.28f).roundToInt()
        val visible = info.visibleItemsInfo.firstOrNull { it.index == activeIndex }
        if (visible != null) {
            val delta = (visible.offset + visible.size / 2 - anchor).toFloat()
            if (abs(delta) > 3f) {
                listState.animateScrollBy(delta, tween(340, easing = FastOutSlowInEasing))
            }
        } else {
            listState.scrollToItem(activeIndex, 0)
            withFrameNanos { }
            val updated = listState.layoutInfo
            val target = updated.visibleItemsInfo.firstOrNull { it.index == activeIndex }
            if (target != null) {
                val delta = (target.offset + target.size / 2 - anchor).toFloat()
                if (abs(delta) > 3f) {
                    listState.animateScrollBy(delta, tween(360, easing = FastOutSlowInEasing))
                }
            }
        }
    }

    val durationMs = clock.durationMs
    val bottomPadDp = with(density) {
        ((panelHeightPx * 0.56f).roundToInt().coerceAtLeast(1)).toDp()
    }.coerceAtLeast(180.dp)

    val isScrolling by remember(listState) {
        derivedStateOf { listState.isScrollInProgress }
    }
    val headlineStyle = MaterialTheme.typography.headlineMedium

    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val isWidePane = maxWidth >= 460.dp
        val horizontalPad = if (isWidePane) 28.dp else 20.dp
        val mainFontSp = if (isWidePane) 32.sp else 28.sp
        val mainLineHeightSp = if (isWidePane) 40.sp else 35.sp
        val bgFontSp = if (isWidePane) 23.sp else 20.sp
        val bgLineHeightSp = if (isWidePane) 29.sp else 25.sp

        Column(modifier = Modifier.fillMaxSize()) {
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .onSizeChanged { panelHeightPx = it.height }
                    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                    .drawWithCache {
                        val topFade = 16.dp.toPx().coerceAtMost(size.height * 0.12f)
                        val bottomFade = 44.dp.toPx().coerceAtMost(size.height * 0.22f)
                        val mask = Brush.verticalGradient(
                            0f to Color.Transparent,
                            (topFade / size.height.coerceAtLeast(1f)).coerceIn(0.01f, 0.15f) to Color.Black,
                            (1f - bottomFade / size.height.coerceAtLeast(1f)).coerceIn(0.75f, 0.99f) to Color.Black,
                            1f to Color.Transparent,
                        )
                        onDrawWithContent {
                            drawContent()
                            drawRect(brush = mask, blendMode = BlendMode.DstIn)
                        }
                    },
            ) {
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(
                        top = 16.dp,
                        bottom = bottomPadDp,
                        start = horizontalPad,
                        end = horizontalPad,
                    ),
                    verticalArrangement = Arrangement.spacedBy(if (isWidePane) 22.dp else 18.dp),
                ) {
                    itemsIndexed(
                        items = lyrics,
                        key = { idx, line -> "${idx}_${line.timeSecs}" },
                        contentType = { _, _ -> "lyric_line" },
                    ) { index, line ->
                        val dist = abs(index - activeIndex)
                        val rawTrimmed = line.text.trim()
                        val isCounter = line.isCounterVoice ||
                            rawTrimmed.startsWith("v2:", ignoreCase = true) ||
                            rawTrimmed.startsWith("bg:", ignoreCase = true)
                        val cleanText = cleanedLines[index]
                        val isBackground = cleanText.startsWith("(") && cleanText.endsWith(")") && cleanText.length > 2

                        val lineStartMs = (line.timeSecs * 1000.0).toLong()
                        val baseNextStartMs = ((index + 1) until lyrics.size)
                            .firstOrNull { nextIdx ->
                                if (isBackground) {
                                    true
                                } else {
                                    val nc = cleanedLines[nextIdx]
                                    !(nc.startsWith("(") && nc.endsWith(")") && nc.length > 2)
                                }
                            }
                            ?.let { (lyrics[it].timeSecs * 1000.0).toLong() }
                            ?: durationMs.takeIf { it > lineStartMs + 600L }
                            ?: (lineStartMs + 4_000L)
                        val wordMaxEndMs = remember(line.words) {
                            line.words
                                .maxOfOrNull { ((it.endSecs ?: (it.startSecs + 0.45)) * 1000.0).toLong() }
                        }
                        val nextStartMs = when {
                            isBackground -> {
                                val naturalEnd = lineStartMs + (cleanText.length * 85L).coerceIn(1_400L, 3_600L)
                                maxOf(wordMaxEndMs ?: 0L, naturalEnd)
                            }
                            wordMaxEndMs != null && wordMaxEndMs > lineStartMs ->
                                maxOf(wordMaxEndMs, lineStartMs + 500L)
                            else -> baseNextStartMs
                        }

                        val isActive by remember(index, lineStartMs, nextStartMs, isBackground, isCounter) {
                            derivedStateOf {
                                if (index == activeIndex) {
                                    true
                                } else {
                                    val pos = adjustedClockMs.value + LYRIC_SYNC_LEAD_MS
                                    val tailPadMs = if (isBackground) 480L else 280L
                                    val withinWindow = pos in lineStartMs..(nextStartMs + tailPadMs)
                                    if (!withinWindow) {
                                        false
                                    } else if (isBackground || isCounter || line.words.isNotEmpty()) {
                                        true
                                    } else {
                                        val currentActive = lyrics.getOrNull(activeIndex)
                                        val activeTrim = currentActive?.text?.trim().orEmpty()
                                        currentActive != null && (
                                            currentActive.isCounterVoice ||
                                                (activeTrim.startsWith("(") && activeTrim.endsWith(")"))
                                            )
                                    }
                                }
                            }
                        }

                        val interactionSource = remember { MutableInteractionSource() }
                        val isPressed by interactionSource.collectIsPressedAsState()

                        val liftTargetPx = with(density) { (if (isActive) (-2).dp else 0.dp).toPx() }
                        val liftPx by animateFloatAsState(
                            targetValue = liftTargetPx,
                            animationSpec = spring(
                                dampingRatio = Spring.DampingRatioMediumBouncy,
                                stiffness = Spring.StiffnessLow,
                            ),
                            label = "lyricLift",
                        )
                        // Never scale above 1.0f so full-width lines never clip horizontally.
                        val rowScale by animateFloatAsState(
                            targetValue = when {
                                isPressed -> 0.96f
                                isActive -> 1.0f
                                else -> 0.94f
                            },
                            animationSpec = tween(360, easing = FastOutSlowInEasing),
                            label = "lyricScale",
                        )
                        val rowAlpha by animateFloatAsState(
                            targetValue = when {
                                isActive -> 1.0f
                                dist == 1 -> 0.62f
                                dist == 2 -> 0.42f
                                else -> 0.26f
                            },
                            animationSpec = tween(280),
                            label = "lyricAlpha",
                        )
                        val blurDp = if (effectiveBlur && !isActive && !isScrolling) {
                            when (dist) {
                                1 -> 1.2.dp
                                2 -> 2.2.dp
                                else -> 3.4.dp
                            }
                        } else {
                            0.dp
                        }

                        val baseStyle = remember(
                            isBackground,
                            isCounter,
                            bgFontSp,
                            mainFontSp,
                            bgLineHeightSp,
                            mainLineHeightSp,
                            headlineStyle,
                        ) {
                            headlineStyle.copy(
                                fontSize = if (isBackground) bgFontSp else mainFontSp,
                                lineHeight = if (isBackground) bgLineHeightSp else mainLineHeightSp,
                                fontWeight = if (isBackground) FontWeight.SemiBold else FontWeight.Bold,
                                textAlign = if (isCounter) TextAlign.End else TextAlign.Start,
                                textDirection = TextDirection.Content,
                            )
                        }

                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(
                                    start = if (hasDuets && isCounter) 28.dp else 0.dp,
                                    end = if (hasDuets && !isCounter) 28.dp else 0.dp,
                                )
                                .graphicsLayer {
                                    translationY = liftPx
                                    scaleX = rowScale
                                    scaleY = rowScale
                                    alpha = rowAlpha
                                    transformOrigin = TransformOrigin(if (isCounter) 1f else 0f, 0.5f)
                                }
                                .then(if (blurDp >= 0.5.dp && !reduceBlur) Modifier.blur(blurDp) else Modifier)
                                .clip(RoundedCornerShape(12.dp))
                                .clickable(
                                    interactionSource = interactionSource,
                                    indication = null,
                                ) { onSeekTo((lineStartMs + offsetMs).coerceAtLeast(0L)) }
                                .padding(vertical = 4.dp),
                            horizontalAlignment = if (isCounter) Alignment.End else Alignment.Start,
                        ) {
                            if (prioritizeWordSync) {
                                SweptKaraokeLyricText(
                                    text = cleanText,
                                    lineStartMs = lineStartMs,
                                    lineEndMs = nextStartMs,
                                    words = line.words,
                                    clockMs = adjustedClockMs,
                                    style = baseStyle,
                                    active = isActive,
                                    featherDp = 28.dp,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                            } else {
                                val lineStyle = if (isActive) {
                                    baseStyle.copy(
                                        shadow = androidx.compose.ui.graphics.Shadow(
                                            color = Color.White.copy(alpha = 0.30f),
                                            offset = androidx.compose.ui.geometry.Offset.Zero,
                                            blurRadius = 16f,
                                        ),
                                    )
                                } else {
                                    baseStyle
                                }
                                Text(
                                    text = cleanText,
                                    style = lineStyle,
                                    color = if (isActive) Color.White else Color(0xFF98989F).copy(alpha = 0.34f),
                                    softWrap = true,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                            }

                            if (romanizationEnabled) {
                                val rom = romanizedLines?.getOrNull(index)
                                if (!rom.isNullOrBlank() && !rom.equals(cleanText, ignoreCase = true)) {
                                    Spacer(Modifier.height(4.dp))
                                    Text(
                                        text = rom,
                                        style = MaterialTheme.typography.bodyMedium.copy(
                                            fontSize = 15.sp,
                                            lineHeight = 20.sp,
                                            fontWeight = FontWeight.Medium,
                                            textDirection = TextDirection.Content,
                                        ),
                                        color = Color.White.copy(alpha = if (isActive) 0.80f else 0.54f),
                                        textAlign = if (isCounter) TextAlign.End else TextAlign.Start,
                                        softWrap = true,
                                        modifier = Modifier.fillMaxWidth(),
                                    )
                                }
                            }

                            if (translationEnabled) {
                                val trans = translatedLines?.getOrNull(index)
                                if (!trans.isNullOrBlank() && !trans.equals(cleanText, ignoreCase = true)) {
                                    Spacer(Modifier.height(3.dp))
                                    Text(
                                        text = trans,
                                        style = MaterialTheme.typography.bodyMedium.copy(
                                            fontSize = 15.sp,
                                            lineHeight = 20.sp,
                                            fontWeight = FontWeight.Normal,
                                            textDirection = TextDirection.Content,
                                        ),
                                        color = Color.White.copy(alpha = if (isActive) 0.88f else 0.58f),
                                        textAlign = if (isCounter) TextAlign.End else TextAlign.Start,
                                        softWrap = true,
                                        modifier = Modifier.fillMaxWidth(),
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
