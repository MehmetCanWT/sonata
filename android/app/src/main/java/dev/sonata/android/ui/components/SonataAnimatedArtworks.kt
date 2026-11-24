package dev.sonata.android.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.platform.AndroidUiDispatcher
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import dev.sonata.android.LocalReduceAnimation
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.ui.theme.LocalLunariaColors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.util.Locale
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

private const val TWO_PI = (2.0 * PI).toFloat()

/**
 * Zero-recomposition scroll state tracker updated by scrollable list containers.
 *
 * Read synchronously inside [SharedArtClock] and [dev.sonata.android.ui.player.CardCanvasSurface]
 * without triggering Compose recomposition or DisposableEffect churn on swipe start/stop.
 */
object ScrollActivityTracker {
    @Volatile
    var isScrolling: Boolean = false
        private set

    private val count = AtomicInteger(0)

    fun setScrolling(active: Boolean) {
        val next = if (active) {
            count.incrementAndGet()
        } else {
            count.updateAndGet { current -> (current - 1).coerceAtLeast(0) }
        }
        isScrolling = next > 0
    }
}

/**
 * Single shared ~60fps frame clock for all visible decorative station & mood artworks.
 *
 * Replaces per-card `rememberInfiniteTransition` instances with one Choreographer-synced
 * clock (`AndroidUiDispatcher.Main`) that holds steady with zero draw invalidations while
 * [ScrollActivityTracker.isScrolling] is active and runs only while foregrounded and
 * `LocalReduceAnimation` is false.
 */
private object SharedArtClock {
    private const val MIN_FRAME_STEP_NANOS = 16_000_000L // ~60fps cap on 120Hz displays

    private val scope = CoroutineScope(SupervisorJob() + AndroidUiDispatcher.Main)
    private val elapsedMs = mutableLongStateOf(0L)
    private var subscribers = 0
    private var job: Job? = null

    val state: State<Long> get() = elapsedMs

    fun acquire() {
        subscribers++
        if (subscribers == 1 && job == null) {
            job = scope.launch {
                var lastNanos = 0L
                while (isActive) {
                    withFrameNanos { frameNanos ->
                        if (lastNanos == 0L || ScrollActivityTracker.isScrolling) {
                            // Hold elapsedMs steady during active scroll: zero draw invalidations.
                            lastNanos = frameNanos
                        } else {
                            val delta = frameNanos - lastNanos
                            if (delta >= MIN_FRAME_STEP_NANOS) {
                                // Clamp delta so returning from pause/scroll resumes smoothly without jumping.
                                val clampedMs = (delta / 1_000_000L).coerceIn(0L, 100L)
                                if (clampedMs > 0L) {
                                    elapsedMs.longValue += clampedMs
                                }
                                lastNanos = frameNanos
                            }
                        }
                    }
                }
            }
        }
    }

    fun release() {
        subscribers = (subscribers - 1).coerceAtLeast(0)
        if (subscribers == 0) {
            job?.cancel()
            job = null
        }
    }
}

@Composable
private fun rememberSharedArtClockMs(): State<Long> {
    val lifecycleOwner = LocalLifecycleOwner.current
    var isResumed by remember {
        mutableStateOf(lifecycleOwner.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED))
    }

    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, _ ->
            isResumed = lifecycleOwner.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    val reduceAnimation = LocalReduceAnimation.current
    val shouldRun = isResumed && !reduceAnimation

    DisposableEffect(shouldRun) {
        if (shouldRun) SharedArtClock.acquire()
        onDispose {
            if (shouldRun) SharedArtClock.release()
        }
    }

    return SharedArtClock.state
}

/**
 * Always-animated personal station artwork featuring theme-adaptive flowing gossamer
 * aurora silk ribbons, a 3D translucent lunar glass sphere, and drifting light bokeh.
 */
@Composable
fun SonataPersonalStationArt(
    title: String,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val clockMs = rememberSharedArtClockMs()
    val seed = remember(title) { (title.hashCode() and 0x7FFF_FFFF) % 360 }
    val isViolet = tokens.isViolet
    val isDark = tokens.isDark

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val w = size.width
                val h = size.height
                val cx = w * 0.50f
                val cy = h * 0.44f
                val minDim = size.minDimension
                val haloR = minDim * 0.56f
                val sphereR = minDim * 0.18f
                val ribbonSpan = w * 1.16f
                val startX = cx - ribbonSpan * 0.5f

                val bgColors = when {
                    isViolet && !isDark -> listOf(Color(0xFF6B38B8), Color(0xFF9E63E8), Color(0xFF452182))
                    isViolet && isDark -> listOf(Color(0xFF1F0F42), Color(0xFF462285), Color(0xFF120826))
                    !isViolet && !isDark -> listOf(Color(0xFF7A4988), Color(0xFFB86FA3), Color(0xFF4D3872))
                    else -> listOf(Color(0xFF1E1334), Color(0xFF3E2252), Color(0xFF11152B))
                }
                val primaryGlow = when {
                    isViolet -> Color(0xFFE3B8FF)
                    else -> Color(0xFFFFC9E3)
                }
                val secondaryGlow = when {
                    isViolet -> Color(0xFF9E6BFF)
                    else -> Color(0xFFA7B0F2)
                }
                val tertiaryGlow = when {
                    isViolet -> Color(0xFFFF7BD5)
                    else -> Color(0xFFFFE0EC)
                }

                val bgBrush = Brush.linearGradient(
                    colors = bgColors,
                    start = Offset.Zero,
                    end = Offset(w, h),
                )
                val haloBrush = Brush.radialGradient(
                    colors = listOf(
                        tertiaryGlow.copy(alpha = 0.44f),
                        primaryGlow.copy(alpha = 0.28f),
                        secondaryGlow.copy(alpha = 0.10f),
                        secondaryGlow.copy(alpha = 0f),
                    ),
                    center = Offset.Zero,
                    radius = haloR,
                )
                val ribbonBrushA = Brush.horizontalGradient(
                    colors = listOf(
                        secondaryGlow.copy(alpha = 0f),
                        secondaryGlow.copy(alpha = 0.72f),
                        Color.White.copy(alpha = 0.90f),
                        primaryGlow.copy(alpha = 0.78f),
                        primaryGlow.copy(alpha = 0f),
                    ),
                    startX = startX,
                    endX = startX + ribbonSpan,
                )
                val ribbonBrushB = Brush.horizontalGradient(
                    colors = listOf(
                        tertiaryGlow.copy(alpha = 0f),
                        tertiaryGlow.copy(alpha = 0.65f),
                        primaryGlow.copy(alpha = 0.85f),
                        secondaryGlow.copy(alpha = 0.65f),
                        secondaryGlow.copy(alpha = 0f),
                    ),
                    startX = startX,
                    endX = startX + ribbonSpan,
                )
                val sphereBrush = Brush.radialGradient(
                    colors = listOf(
                        Color.White.copy(alpha = 0.94f),
                        tertiaryGlow.copy(alpha = 0.70f),
                        secondaryGlow.copy(alpha = 0.42f),
                        Color(0xFF1A0F33).copy(alpha = 0.55f),
                    ),
                    center = Offset(-sphereR * 0.34f, -sphereR * 0.34f),
                    radius = sphereR * 1.45f,
                )
                val auroraWideStroke = Stroke(width = minDim * 0.12f, cap = StrokeCap.Round)
                val auroraCoreStroke = Stroke(width = minDim * 0.036f, cap = StrokeCap.Round)
                val auroraRimStroke = Stroke(width = minDim * 0.011f, cap = StrokeCap.Round)
                val sphereRimStroke = Stroke(width = minDim * 0.012f)
                val pathA = Path()
                val pathB = Path()
                val pathC = Path()
                val basePhaseOffset = (seed / 360f) * TWO_PI

                onDrawBehind {
                    val phase = (clockMs.value % 18_000L) / 18_000f
                    val t = phase * TWO_PI + basePhaseOffset
                    clipRect {
                        drawRect(brush = bgBrush)

                        // Sweeping ambient aura behind ribbon folds (integer-periodic)
                        translate(
                            left = cx + cos(t) * minDim * 0.05f,
                            top = cy + sin(t) * minDim * 0.04f,
                        ) {
                            drawCircle(
                                brush = haloBrush,
                                radius = haloR,
                                center = Offset.Zero,
                            )
                        }

                        // Multi-layered 3D gossamer aurora silk ribbons (strictly integer time multiples)
                        pathA.reset()
                        pathB.reset()
                        pathC.reset()
                        val steps = 20
                        for (i in 0..steps) {
                            val u = i / steps.toFloat()
                            val x = startX + ribbonSpan * u
                            val yA = cy + sin(u * TWO_PI * 1.15f + t) * minDim * 0.16f - (u - 0.5f) * minDim * 0.14f
                            val yB = cy + cos(u * TWO_PI * 0.95f - t) * minDim * 0.14f + (u - 0.5f) * minDim * 0.10f
                            val yC = cy * 0.86f + sin(u * TWO_PI * 1.55f + t * 2f) * minDim * 0.11f
                            if (i == 0) {
                                pathA.moveTo(x, yA)
                                pathB.moveTo(x, yB)
                                pathC.moveTo(x, yC)
                            } else {
                                pathA.lineTo(x, yA)
                                pathB.lineTo(x, yB)
                                pathC.lineTo(x, yC)
                            }
                        }

                        drawPath(path = pathC, color = tertiaryGlow.copy(alpha = 0.22f), style = auroraWideStroke)
                        drawPath(path = pathA, color = primaryGlow.copy(alpha = 0.26f), style = auroraWideStroke)
                        drawPath(path = pathA, brush = ribbonBrushA, style = auroraCoreStroke)
                        drawPath(path = pathB, brush = ribbonBrushB, style = auroraCoreStroke)
                        drawPath(path = pathA, color = Color.White.copy(alpha = 0.68f), style = auroraRimStroke)
                        drawPath(path = pathC, color = Color.White.copy(alpha = 0.42f), style = auroraRimStroke)

                        // Floating light bokeh particles along the silk crests (integer-periodic)
                        for (p in 0 until 9) {
                            val angle = t + p * (TWO_PI / 9f)
                            val dist = minDim * (0.24f + (p % 3) * 0.08f)
                            val bx = cx + cos(angle) * dist
                            val by = cy + sin(angle) * dist * 0.68f
                            val twinkle = 0.45f + 0.55f * (0.5f + 0.5f * sin(t * 2f + p * 1.2f))
                            drawCircle(
                                color = if (p % 2 == 0) tertiaryGlow else Color.White,
                                alpha = twinkle * 0.72f,
                                radius = minDim * (0.011f + (p % 3) * 0.004f),
                                center = Offset(bx, by),
                            )
                        }

                        // Central 3D translucent glass core sphere (integer-periodic)
                        val sphereCx = cx + sin(t) * minDim * 0.025f
                        val sphereCy = cy + cos(t) * minDim * 0.025f
                        translate(left = sphereCx, top = sphereCy) {
                            drawCircle(
                                color = primaryGlow.copy(alpha = 0.34f),
                                radius = sphereR * 1.32f,
                                center = Offset.Zero,
                            )
                            drawCircle(
                                brush = sphereBrush,
                                radius = sphereR,
                                center = Offset.Zero,
                            )
                            drawCircle(
                                color = Color.White.copy(alpha = 0.78f),
                                radius = sphereR,
                                center = Offset.Zero,
                                style = sphereRimStroke,
                            )
                        }
                    }
                }
            },
    )
}

/**
 * Always-animated discovery station artwork featuring perspective light-trail horizons,
 * a luminous prism crest wave mesh, and glowing stardust bokeh matched to the active theme.
 */
@Composable
fun SonataDiscoveryStationArt(
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val clockMs = rememberSharedArtClockMs()
    val isViolet = tokens.isViolet
    val isDark = tokens.isDark

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val w = size.width
                val h = size.height
                val cx = w * 0.50f
                val cy = h * 0.42f
                val minDim = size.minDimension

                val bgColors = when {
                    isViolet && !isDark -> listOf(Color(0xFF47258C), Color(0xFF7948C7), Color(0xFF281352))
                    isViolet && isDark -> listOf(Color(0xFF190C38), Color(0xFF2E175E), Color(0xFF0A051A))
                    !isViolet && !isDark -> listOf(Color(0xFF3E356B), Color(0xFF7E5A94), Color(0xFF251A42))
                    else -> listOf(Color(0xFF141838), Color(0xFF261E47), Color(0xFF090A18))
                }
                val trailPrimary = if (isViolet) Color(0xFF9F7BFF) else Color(0xFF7FA8FF)
                val trailAccent = if (isViolet) Color(0xFFFF6EC7) else Color(0xFFF5B8DC)
                val sunHighlight = if (isViolet) Color(0xFFF3E8FF) else Color(0xFFFFF4FA)

                val bgBrush = Brush.linearGradient(
                    colors = bgColors,
                    start = Offset(0f, 0f),
                    end = Offset(w, h),
                )
                val horizonR = minDim * 0.36f
                val horizonBrush = Brush.radialGradient(
                    colors = listOf(
                        sunHighlight.copy(alpha = 0.86f),
                        trailAccent.copy(alpha = 0.56f),
                        trailPrimary.copy(alpha = 0.22f),
                        trailPrimary.copy(alpha = 0f),
                    ),
                    center = Offset(cx, cy * 0.90f),
                    radius = horizonR,
                )
                val glowWideStroke = Stroke(width = minDim * 0.085f, cap = StrokeCap.Round)
                val glowMidStroke = Stroke(width = minDim * 0.036f, cap = StrokeCap.Round)
                val glowHotStroke = Stroke(width = minDim * 0.014f, cap = StrokeCap.Round)
                val trailPathA = Path()
                val trailPathB = Path()
                val trailPathC = Path()

                onDrawBehind {
                    val phase = (clockMs.value % 16_000L) / 16_000f
                    val t = phase * TWO_PI
                    clipRect {
                        drawRect(brush = bgBrush)

                        // Horizon bloom
                        drawCircle(
                            brush = horizonBrush,
                            radius = horizonR,
                            center = Offset(cx, cy * 0.90f),
                        )

                        // Sweeping 3D wave dot crest above horizon
                        val cols = 16
                        val rows = 5
                        for (r in 0 until rows) {
                            val rf = r / (rows - 1f)
                            val yBase = h * (0.14f + rf * 0.22f)
                            for (c in 0 until cols) {
                                val cf = c / (cols - 1f)
                                val dx = w * (0.06f + cf * 0.88f)
                                val dy = yBase - sin(cf * TWO_PI * 1.3f + t + rf * 2.1f) * h * 0.045f
                                val alpha = (0.28f + (1f - rf) * 0.52f).coerceIn(0f, 1f)
                                drawCircle(
                                    color = if (c % 2 == 0) sunHighlight else trailPrimary,
                                    alpha = alpha,
                                    radius = minDim * (0.008f + rf * 0.004f),
                                    center = Offset(dx, dy),
                                )
                            }
                        }

                        // Perspective highway light trails converging toward upper horizon
                        val drift = sin(t) * w * 0.06f
                        val lift = cos(t) * h * 0.035f
                        trailPathA.reset()
                        trailPathA.moveTo(w * 0.14f, h * 0.92f)
                        trailPathA.cubicTo(
                            w * 0.28f,
                            h * (0.58f + lift / h),
                            cx - drift,
                            cy * 1.05f,
                            w * 0.64f,
                            h * 0.24f,
                        )
                        trailPathB.reset()
                        trailPathB.moveTo(w * 0.34f, h * 0.96f)
                        trailPathB.cubicTo(
                            w * 0.44f,
                            h * 0.64f,
                            cx + drift * 0.8f,
                            cy * 1.12f,
                            w * 0.78f,
                            h * 0.28f,
                        )
                        trailPathC.reset()
                        trailPathC.moveTo(w * 0.86f, h * 0.92f)
                        trailPathC.cubicTo(
                            w * 0.68f,
                            h * (0.56f - lift / h),
                            cx + drift,
                            cy * 1.04f,
                            w * 0.42f,
                            h * 0.24f,
                        )

                        drawPath(trailPathA, color = trailPrimary.copy(alpha = 0.28f), style = glowWideStroke)
                        drawPath(trailPathA, color = trailPrimary.copy(alpha = 0.78f), style = glowMidStroke)
                        drawPath(trailPathA, color = sunHighlight.copy(alpha = 0.96f), style = glowHotStroke)

                        drawPath(trailPathC, color = trailAccent.copy(alpha = 0.30f), style = glowWideStroke)
                        drawPath(trailPathC, color = trailAccent.copy(alpha = 0.80f), style = glowMidStroke)
                        drawPath(trailPathC, color = sunHighlight.copy(alpha = 0.96f), style = glowHotStroke)

                        drawPath(trailPathB, color = trailAccent.copy(alpha = 0.22f), style = glowMidStroke)
                        drawPath(trailPathB, color = sunHighlight.copy(alpha = 0.72f), style = glowHotStroke)

                        // Luminous sun/moon diamond crest
                        val moonCoreR = minDim * 0.085f
                        drawCircle(
                            color = sunHighlight.copy(alpha = 0.92f),
                            radius = moonCoreR,
                            center = Offset(cx, cy * 0.90f),
                        )
                    }
                }
            },
    )
}

internal enum class MoodTheme {
    CHILL,
    ENERGY,
    FOCUS,
    ROMANCE,
    HEARTBREAK,
    SLEEP,
    PARTY,
    MELANCHOLY,
    COMMUTE,
    FEEL_GOOD,
    FOLK_ACOUSTIC,
    URBAN_PRISM,
    BOTANICAL_INDIE,
    HARMONIC_JAZZ,
    SYNTH_WAVE,
}

/**
 * Unique, centered, seamlessly looping artwork tailored deterministically to every mood in
 * Find Your Mood, strictly bounded inside its tile and harmonized with Violet / Moonflower.
 */
@Composable
fun SonataMoodAnimatedArt(
    moodTitle: String,
    moodIndex: Int,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val designTheme = tokens.designTheme
    val isDark = tokens.isDark
    val theme = remember(moodTitle, moodIndex) { resolveMoodTheme(moodTitle, moodIndex) }
    val periodMs = when (theme) {
        MoodTheme.SLEEP, MoodTheme.CHILL -> 18_000L
        MoodTheme.PARTY, MoodTheme.ENERGY -> 12_000L
        else -> 16_000L
    }
    val clockMs = rememberSharedArtClockMs()

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val palette = moodPaletteFor(theme, designTheme, isDark)
                val bgBrush = Brush.linearGradient(
                    colors = palette.bg,
                    start = Offset.Zero,
                    end = Offset(size.width, size.height),
                )
                val cx = size.width * 0.5f
                val cy = size.height * 0.5f
                val minDim = size.minDimension
                val reusablePath = Path()
                val secondaryPath = Path()

                // Centered brushes & strokes (zero allocations inside onDrawBehind)
                val sphereBrush = Brush.radialGradient(
                    colors = listOf(
                        Color.White.copy(alpha = 0.85f),
                        palette.primary.copy(alpha = 0.55f),
                        palette.secondary.copy(alpha = 0.35f),
                        Color(0xFF0A0D1C).copy(alpha = 0.65f),
                    ),
                    center = Offset(-minDim * 0.05f, -minDim * 0.05f),
                    radius = minDim * 0.18f,
                )
                val sphereHaloBrush = Brush.radialGradient(
                    colors = listOf(palette.primary.copy(alpha = 0.45f), palette.primary.copy(alpha = 0f)),
                    center = Offset.Zero,
                    radius = minDim * 0.32f,
                )
                val ribbonSpan = size.width * 0.80f
                val ribbonStartX = cx - ribbonSpan * 0.5f
                val ribbonPrimaryBrush = Brush.horizontalGradient(
                    colors = listOf(
                        palette.primary.copy(alpha = 0f),
                        palette.primary.copy(alpha = 0.84f),
                        palette.highlight.copy(alpha = 0.94f),
                        palette.secondary.copy(alpha = 0.82f),
                        palette.secondary.copy(alpha = 0f),
                    ),
                    startX = ribbonStartX,
                    endX = ribbonStartX + ribbonSpan,
                )
                val ribbonSecondaryBrush = Brush.horizontalGradient(
                    colors = listOf(
                        palette.secondary.copy(alpha = 0f),
                        palette.secondary.copy(alpha = 0.68f),
                        Color.White.copy(alpha = 0.78f),
                        palette.primary.copy(alpha = 0.68f),
                        palette.primary.copy(alpha = 0f),
                    ),
                    startX = ribbonStartX,
                    endX = ribbonStartX + ribbonSpan,
                )
                val sunriseBrush = Brush.radialGradient(
                    colors = listOf(palette.highlight.copy(alpha = 0.82f), palette.primary.copy(alpha = 0.40f), palette.primary.copy(alpha = 0f)),
                    center = Offset(cx, cy),
                    radius = minDim * 0.36f,
                )

                val thinStroke = Stroke(width = minDim * 0.012f, cap = StrokeCap.Round)
                val mediumStroke = Stroke(width = minDim * 0.020f, cap = StrokeCap.Round)
                val glowWideStroke = Stroke(width = minDim * 0.070f, cap = StrokeCap.Round)
                val glowMidStroke = Stroke(width = minDim * 0.032f, cap = StrokeCap.Round)
                val glowHotStroke = Stroke(width = minDim * 0.013f, cap = StrokeCap.Round)
                val ribbonStroke = Stroke(width = minDim * 0.052f, cap = StrokeCap.Round)
                val ribbonRimStroke = Stroke(width = minDim * 0.012f, cap = StrokeCap.Round)
                val tunnelOuterStroke = Stroke(width = minDim * 0.036f)
                val tunnelCoreStroke = Stroke(width = minDim * 0.013f)

                onDrawBehind {
                    val phase = (clockMs.value % periodMs) / periodMs.toFloat()
                    val t = phase * TWO_PI
                    clipRect {
                        drawRect(brush = bgBrush)
                        when (theme) {
                            MoodTheme.COMMUTE -> drawHighwayLightTrails(
                                t = t,
                                primaryPath = reusablePath,
                                secondaryPath = secondaryPath,
                                palette = palette,
                                glowWide = glowWideStroke,
                                glowMid = glowMidStroke,
                                glowHot = glowHotStroke,
                            )
                            MoodTheme.CHILL -> drawLotusRippleWaves(
                                t = t,
                                palette = palette,
                                ringStroke = mediumStroke,
                                thinStroke = thinStroke,
                            )
                            MoodTheme.FOCUS -> drawOrbitalGlassSphere(
                                t = t,
                                palette = palette,
                                sphereBrush = sphereBrush,
                                haloBrush = sphereHaloBrush,
                                ringStroke = thinStroke,
                            )
                            MoodTheme.ENERGY -> drawDottedWaveMesh(
                                t = t,
                                palette = palette,
                            )
                            MoodTheme.PARTY -> drawRadialSpectrumEqualizer(
                                t = t,
                                palette = palette,
                                barStroke = mediumStroke,
                                glowStroke = glowWideStroke,
                            )
                            MoodTheme.ROMANCE -> drawSilkAuroraRibbons(
                                t = t,
                                pathA = reusablePath,
                                pathB = secondaryPath,
                                brushA = ribbonPrimaryBrush,
                                brushB = ribbonSecondaryBrush,
                                startX = ribbonStartX,
                                span = ribbonSpan,
                                bandStroke = ribbonStroke,
                                rimStroke = ribbonRimStroke,
                                haloBrush = sunriseBrush,
                            )
                            MoodTheme.FEEL_GOOD -> drawSolarPrismBloom(
                                t = t,
                                palette = palette,
                                haloBrush = sunriseBrush,
                                rayStroke = mediumStroke,
                            )
                            MoodTheme.HEARTBREAK -> drawFracturedCrystalPulse(
                                t = t,
                                pathA = reusablePath,
                                pathB = secondaryPath,
                                palette = palette,
                                pulseStroke = mediumStroke,
                                shardStroke = thinStroke,
                            )
                            MoodTheme.SYNTH_WAVE -> drawSynthwaveSunsetOrb(
                                t = t,
                                palette = palette,
                                haloBrush = sunriseBrush,
                                waveStroke = thinStroke,
                            )
                            MoodTheme.URBAN_PRISM -> drawPerspectiveTunnelArches(
                                t = t,
                                palette = palette,
                                outerStroke = tunnelOuterStroke,
                                coreStroke = tunnelCoreStroke,
                            )
                            MoodTheme.MELANCHOLY -> drawStardustCrescent(
                                t = t,
                                palette = palette,
                                path = reusablePath,
                                ribbonStroke = mediumStroke,
                            )
                            MoodTheme.SLEEP -> drawNocturnalStarlightHalo(
                                t = t,
                                palette = palette,
                                ringStroke = thinStroke,
                            )
                            MoodTheme.FOLK_ACOUSTIC -> drawHarmonicStrings(t, reusablePath, mediumStroke, palette.highlight)
                            MoodTheme.BOTANICAL_INDIE -> drawLunariaPetals(t, thinStroke, palette.primary, palette.highlight)
                            MoodTheme.HARMONIC_JAZZ -> drawBrassWaves(t, reusablePath, mediumStroke, palette.primary, palette.highlight)
                        }
                    }
                }
            },
    )
}

private data class MoodCoverPalette(
    val bg: List<Color>,
    val primary: Color,
    val secondary: Color,
    val highlight: Color,
)

private fun moodPaletteFor(
    theme: MoodTheme,
    designTheme: DesignTheme,
    isDark: Boolean,
): MoodCoverPalette {
    val violetBias = designTheme == DesignTheme.VIOLET
    val base = when (theme) {
        MoodTheme.COMMUTE -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF0A0D28), Color(0xFF21174E), Color(0xFF070A1A)),
                primary = Color(0xFF6B8BFF),
                secondary = Color(0xFFB57BFF),
                highlight = Color(0xFFE7EEFF),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF081026), Color(0xFF142A54), Color(0xFF050814)),
                primary = Color(0xFF4C8DFF),
                secondary = Color(0xFF9FB8FF),
                highlight = Color(0xFFFFFFFF),
            )
        }
        MoodTheme.CHILL -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF141132), Color(0xFF2B205E), Color(0xFF0A091B)),
                primary = Color(0xFF9F8BFF),
                secondary = Color(0xFF66C7F4),
                highlight = Color(0xFFF0EBFF),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF0B1C2E), Color(0xFF183654), Color(0xFF07101C)),
                primary = Color(0xFF7AD7F0),
                secondary = Color(0xFFAE9EFF),
                highlight = Color(0xFFEBFBFF),
            )
        }
        MoodTheme.FOCUS -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF120C29), Color(0xFF251A4D), Color(0xFF090617)),
                primary = Color(0xFF9D7EFF),
                secondary = Color(0xFF6D9BFF),
                highlight = Color(0xFFF3EEFF),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF0B1024), Color(0xFF182246), Color(0xFF060813)),
                primary = Color(0xFF7CA4FF),
                secondary = Color(0xFFCBB6FF),
                highlight = Color(0xFFFFFFFF),
            )
        }
        MoodTheme.ENERGY, MoodTheme.PARTY -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF280B36), Color(0xFF4D155E), Color(0xFF150620)),
                primary = Color(0xFFFF46B8),
                secondary = Color(0xFFB45CFF),
                highlight = Color(0xFFFFC7EC),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF2A0D25), Color(0xFF561844), Color(0xFF160714)),
                primary = Color(0xFFFF4DA6),
                secondary = Color(0xFFF285C8),
                highlight = Color(0xFFFFD6EC),
            )
        }
        MoodTheme.ROMANCE, MoodTheme.FEEL_GOOD -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF24113D), Color(0xFF4C226B), Color(0xFF180C2E)),
                primary = Color(0xFFFF59BD),
                secondary = Color(0xFFA988FF),
                highlight = Color(0xFFFFE5F6),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF281230), Color(0xFF562356), Color(0xFF1B102B)),
                primary = Color(0xFFFF72B6),
                secondary = Color(0xFFB5A0FF),
                highlight = Color(0xFFFFEDF7),
            )
        }
        MoodTheme.HEARTBREAK -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF2B0D22), Color(0xFF54173B), Color(0xFF180716)),
                primary = Color(0xFFFF4F87),
                secondary = Color(0xFFC77DFF),
                highlight = Color(0xFFFFE3EE),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF2E1124), Color(0xFF5A1D42), Color(0xFF190A16)),
                primary = Color(0xFFFF5C93),
                secondary = Color(0xFFD69CF2),
                highlight = Color(0xFFFFEBF2),
            )
        }
        MoodTheme.SYNTH_WAVE, MoodTheme.URBAN_PRISM -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF29132E), Color(0xFF52254B), Color(0xFF16091B)),
                primary = Color(0xFFFF9248),
                secondary = Color(0xFFC46CFF),
                highlight = Color(0xFFFFDEB8),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF26140D), Color(0xFF4D2812), Color(0xFF140A06)),
                primary = Color(0xFFFF9633),
                secondary = Color(0xFFFFC370),
                highlight = Color(0xFFFFECD1),
            )
        }
        MoodTheme.SLEEP, MoodTheme.MELANCHOLY -> if (violetBias) {
            MoodCoverPalette(
                bg = listOf(Color(0xFF14112E), Color(0xFF282052), Color(0xFF0A0819)),
                primary = Color(0xFFC2B2FF),
                secondary = Color(0xFF8CA6FF),
                highlight = Color(0xFFF6F2FF),
            )
        } else {
            MoodCoverPalette(
                bg = listOf(Color(0xFF101329), Color(0xFF22284E), Color(0xFF090B18)),
                primary = Color(0xFFB8C4FF),
                secondary = Color(0xFFE6C8F2),
                highlight = Color(0xFFFFFFFF),
            )
        }
        MoodTheme.FOLK_ACOUSTIC -> MoodCoverPalette(
            bg = listOf(Color(0xFF221521), Color(0xFF3F2337), Color(0xFF140B14)),
            primary = if (violetBias) Color(0xFFD99DFF) else Color(0xFFF2B694),
            secondary = Color(0xFFB888E8),
            highlight = Color(0xFFFFECE0),
        )
        MoodTheme.BOTANICAL_INDIE -> MoodCoverPalette(
            bg = listOf(Color(0xFF171832), Color(0xFF2B2D58), Color(0xFF0E0F20)),
            primary = if (violetBias) Color(0xFFC7A8FF) else Color(0xFFF2CBE0),
            secondary = Color(0xFF8CE2D0),
            highlight = Color(0xFFF2FAFF),
        )
        MoodTheme.HARMONIC_JAZZ -> MoodCoverPalette(
            bg = listOf(Color(0xFF251226), Color(0xFF461D3D), Color(0xFF140916)),
            primary = if (violetBias) Color(0xFFE08BFF) else Color(0xFFFFC08E),
            secondary = Color(0xFFEC7AB0),
            highlight = Color(0xFFFFECF4),
        )
    }
    return if (isDark) {
        base
    } else {
        // Light mode lifts the deep shadow floor slightly while keeping rich luminous highlights.
        base.copy(
            bg = base.bg.map { androidx.compose.ui.graphics.lerp(it, Color(0xFF3E2B68), 0.22f) },
        )
    }
}

private fun DrawScope.drawHighwayLightTrails(
    t: Float,
    primaryPath: Path,
    secondaryPath: Path,
    palette: MoodCoverPalette,
    glowWide: Stroke,
    glowMid: Stroke,
    glowHot: Stroke,
) {
    val w = size.width
    val h = size.height
    val drift = sin(t) * w * 0.045f
    val shiftY = cos(t) * h * 0.03f

    primaryPath.reset()
    primaryPath.moveTo(w * 0.80f, h * 0.14f)
    primaryPath.cubicTo(
        w * (0.28f + drift / w),
        h * 0.24f,
        w * (0.34f - drift / w),
        h * 0.56f,
        w * 0.78f,
        h * 0.86f,
    )

    secondaryPath.reset()
    secondaryPath.moveTo(w * 0.86f, h * 0.18f)
    secondaryPath.cubicTo(
        w * (0.38f + drift / w),
        h * (0.28f + shiftY / h),
        w * (0.44f - drift / w),
        h * 0.62f,
        w * 0.84f,
        h * 0.82f,
    )

    drawPath(primaryPath, color = palette.secondary.copy(alpha = 0.28f), style = glowWide)
    drawPath(primaryPath, color = palette.primary.copy(alpha = 0.72f), style = glowMid)
    drawPath(primaryPath, color = palette.highlight.copy(alpha = 0.95f), style = glowHot)

    drawPath(secondaryPath, color = palette.primary.copy(alpha = 0.22f), style = glowMid)
    drawPath(secondaryPath, color = palette.highlight.copy(alpha = 0.70f), style = glowHot)
}

private fun DrawScope.drawOrbitalGlassSphere(
    t: Float,
    palette: MoodCoverPalette,
    sphereBrush: Brush,
    haloBrush: Brush,
    ringStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension

    translate(cx, cy) {
        drawCircle(brush = haloBrush, radius = minDim * 0.32f, center = Offset.Zero)
    }

    val radii = floatArrayOf(minDim * 0.24f, minDim * 0.31f, minDim * 0.38f)
    rotate(degrees = -18f, pivot = Offset(cx, cy)) {
        for (i in radii.indices) {
            val rx = radii[i]
            val ry = rx * 0.44f
            drawOval(
                color = palette.primary.copy(alpha = 0.58f - i * 0.14f),
                topLeft = Offset(cx - rx, cy - ry),
                size = Size(rx * 2f, ry * 2f),
                style = ringStroke,
            )
            // Integer-periodic orbit around centered ellipse
            val angle = t * (if (i % 2 == 0) 1f else -1f) + i * 2.1f
            val sx = cx + cos(angle) * rx
            val sy = cy + sin(angle) * ry
            drawCircle(
                color = palette.primary.copy(alpha = 0.35f),
                radius = minDim * 0.042f,
                center = Offset(sx, sy),
            )
            drawCircle(
                color = palette.highlight,
                radius = minDim * 0.019f,
                center = Offset(sx, sy),
            )
        }
    }

    val sphereR = minDim * 0.13f
    translate(cx, cy) {
        drawCircle(brush = sphereBrush, radius = sphereR, center = Offset.Zero)
        drawCircle(
            color = palette.highlight.copy(alpha = 0.72f),
            radius = sphereR,
            center = Offset.Zero,
            style = ringStroke,
        )
    }
}

private fun DrawScope.drawDottedWaveMesh(
    t: Float,
    palette: MoodCoverPalette,
) {
    val w = size.width
    val h = size.height
    val cols = 16
    val rows = 8
    val minDim = size.minDimension
    val dotBaseR = minDim * 0.012f

    for (r in 0 until rows) {
        val rowFrac = r / (rows - 1f)
        val yBase = h * (0.28f + rowFrac * 0.44f)
        val depthScale = 0.65f + rowFrac * 0.45f
        for (c in 0 until cols) {
            val colFrac = c / (cols - 1f)
            val x = w * (0.12f + colFrac * 0.76f)
            val wave1 = sin(colFrac * TWO_PI * 1.4f + t + rowFrac * 2.2f)
            val wave2 = cos(colFrac * TWO_PI * 0.7f - t + rowFrac * 3.1f)
            val crest = (wave1 * 0.65f + wave2 * 0.35f)
            val y = yBase - crest * h * 0.10f
            val crestBoost = ((crest + 1f) * 0.5f).coerceIn(0f, 1f)
            val color = if (crestBoost > 0.68f) palette.highlight else if (crestBoost > 0.38f) palette.primary else palette.secondary
            drawCircle(
                color = color,
                alpha = 0.35f + crestBoost * 0.62f,
                radius = dotBaseR * depthScale * (0.75f + crestBoost * 0.50f),
                center = Offset(x, y),
            )
        }
    }
}

private fun DrawScope.drawSilkAuroraRibbons(
    t: Float,
    pathA: Path,
    pathB: Path,
    brushA: Brush,
    brushB: Brush,
    startX: Float,
    span: Float,
    bandStroke: Stroke,
    rimStroke: Stroke,
    haloBrush: Brush,
) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    drawCircle(brush = haloBrush, radius = minDim * 0.34f, center = Offset(cx, cy))

    pathA.reset()
    pathB.reset()
    val steps = 18
    for (i in 0..steps) {
        val u = i / steps.toFloat()
        val x = startX + span * u
        val envelope = sin(u * PI.toFloat())
        val yA = cy + sin(u * TWO_PI * 1.15f + t) * minDim * 0.15f * envelope
        val yB = cy + cos(u * TWO_PI * 1.05f - t) * minDim * 0.14f * envelope
        if (i == 0) {
            pathA.moveTo(x, yA)
            pathB.moveTo(x, yB)
        } else {
            pathA.lineTo(x, yA)
            pathB.lineTo(x, yB)
        }
    }

    drawPath(pathA, brush = brushA, style = bandStroke)
    drawPath(pathB, brush = brushB, style = bandStroke)
    drawPath(pathA, color = Color.White.copy(alpha = 0.55f), style = rimStroke)
    drawPath(pathB, color = Color.White.copy(alpha = 0.42f), style = rimStroke)
}

private fun DrawScope.drawPerspectiveTunnelArches(
    t: Float,
    palette: MoodCoverPalette,
    outerStroke: Stroke,
    coreStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val w = size.width
    val h = size.height
    val phase = t / TWO_PI

    // Centered vanishing rails kept safely inside the card padding
    for (offsetFrac in listOf(-0.22f, -0.09f, 0.09f, 0.22f)) {
        drawLine(
            color = palette.primary.copy(alpha = 0.36f),
            start = Offset(cx, cy + h * 0.03f),
            end = Offset(cx + w * offsetFrac * 1.4f, h * 0.84f),
            strokeWidth = 2.0f,
        )
    }

    // Centered perspective glowing portals with smooth sin(p * PI) envelope so fade in/out is 100% seamless
    val archCount = 5
    for (i in 0 until archCount) {
        val p = (phase + i / archCount.toFloat()) % 1f
        val scale = 0.14f + p * p * 0.62f
        val archW = w * 0.80f * scale
        val archH = h * 0.74f * scale
        val left = cx - archW * 0.5f
        val top = cy - archH * 0.5f
        val corner = CornerRadius(archW * 0.22f, archW * 0.22f)
        val envAlpha = sin(p * PI.toFloat()).coerceIn(0f, 1f)
        drawRoundRect(
            color = palette.primary.copy(alpha = envAlpha * 0.44f),
            topLeft = Offset(left, top),
            size = Size(archW, archH),
            cornerRadius = corner,
            style = outerStroke,
        )
        drawRoundRect(
            color = palette.highlight.copy(alpha = envAlpha * 0.88f),
            topLeft = Offset(left, top),
            size = Size(archW, archH),
            cornerRadius = corner,
            style = coreStroke,
        )
    }
}

private fun DrawScope.drawStardustCrescent(
    t: Float,
    palette: MoodCoverPalette,
    path: Path,
    ribbonStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension

    // Centered stardust ribbon safely inset from both horizontal edges
    path.reset()
    val span = size.width * 0.76f
    val startX = cx - span * 0.5f
    for (i in 0..16) {
        val u = i / 16f
        val x = startX + span * u
        val env = sin(u * PI.toFloat())
        val y = cy + sin(u * TWO_PI + t) * minDim * 0.13f * env + (0.5f - u) * minDim * 0.10f
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    drawPath(path, color = palette.secondary.copy(alpha = 0.24f), style = Stroke(width = minDim * 0.07f, cap = StrokeCap.Round))
    drawPath(path, color = palette.primary.copy(alpha = 0.45f), style = ribbonStroke)

    val starR = minDim * 0.010f
    for (s in 0 until 10) {
        val frac = s / 9f
        val sx = cx + (frac - 0.5f) * span * 0.92f
        val sy = cy + sin(frac * TWO_PI + t + s) * minDim * 0.20f
        val twinkle = 0.35f + 0.65f * (0.5f + 0.5f * sin(t * 2f + s * 1.4f))
        drawCircle(
            color = palette.highlight,
            alpha = twinkle * 0.82f,
            radius = starR * (0.6f + (s % 3) * 0.30f),
            center = Offset(sx, sy),
        )
    }

    // Centered luminous crescent moon
    val moonCx = cx + cos(t) * minDim * 0.016f
    val moonCy = cy + sin(t) * minDim * 0.016f
    val moonR = minDim * 0.19f
    drawCircle(
        color = palette.primary.copy(alpha = 0.28f),
        radius = moonR * 1.32f,
        center = Offset(moonCx, moonCy),
    )
    drawCircle(
        color = palette.highlight.copy(alpha = 0.94f),
        radius = moonR,
        center = Offset(moonCx, moonCy),
    )
    drawCircle(
        color = palette.bg[1],
        radius = moonR * 0.86f,
        center = Offset(moonCx + moonR * 0.38f, moonCy - moonR * 0.20f),
    )
}

private fun DrawScope.drawLotusRippleWaves(
    t: Float,
    palette: MoodCoverPalette,
    ringStroke: Stroke,
    thinStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    val phase = t / TWO_PI

    for (i in 0..3) {
        val p = (phase + i * 0.25f) % 1f
        val r = minDim * (0.08f + p * 0.28f)
        val alpha = sin(p * PI.toFloat()).coerceIn(0f, 1f)
        drawCircle(
            color = if (i % 2 == 0) palette.primary else palette.secondary,
            alpha = alpha * 0.72f,
            radius = r,
            center = Offset(cx, cy),
            style = if (i % 2 == 0) ringStroke else thinStroke,
        )
    }
    for (orb in 0 until 4) {
        val angle = t + orb * (TWO_PI / 4f)
        val r = minDim * 0.24f
        drawCircle(
            color = palette.highlight,
            alpha = 0.85f,
            radius = minDim * 0.016f,
            center = Offset(cx + cos(angle) * r, cy + sin(angle) * r),
        )
    }
}

private fun DrawScope.drawSolarPrismBloom(
    t: Float,
    palette: MoodCoverPalette,
    haloBrush: Brush,
    rayStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    drawCircle(brush = haloBrush, radius = minDim * 0.34f, center = Offset(cx, cy))

    val degOffset = (t / TWO_PI) * 360f
    val innerR = minDim * 0.14f
    val outerR = minDim * 0.32f + sin(t * 2f) * minDim * 0.03f
    rotate(degrees = degOffset, pivot = Offset(cx, cy)) {
        for (i in 0 until 8) {
            val angle = i * (TWO_PI / 8f)
            val cosA = cos(angle)
            val sinA = sin(angle)
            drawLine(
                color = if (i % 2 == 0) palette.highlight else palette.primary,
                start = Offset(cx + cosA * innerR, cy + sinA * innerR),
                end = Offset(cx + cosA * outerR, cy + sinA * outerR),
                strokeWidth = rayStroke.width,
                cap = StrokeCap.Round,
                alpha = 0.84f,
            )
        }
    }
    drawCircle(
        color = palette.highlight.copy(alpha = 0.92f),
        radius = minDim * 0.095f,
        center = Offset(cx, cy),
    )
}

private fun DrawScope.drawFracturedCrystalPulse(
    t: Float,
    pathA: Path,
    pathB: Path,
    palette: MoodCoverPalette,
    pulseStroke: Stroke,
    shardStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension

    // Centered counter-phase heartbeat harmonic wave bounded cleanly inside [0.16*w..0.84*w]
    val span = size.width * 0.68f
    val startX = cx - span * 0.5f
    pathA.reset()
    pathB.reset()
    for (i in 0..20) {
        val u = i / 20f
        val x = startX + span * u
        val env = sin(u * PI.toFloat())
        val wave = sin(u * TWO_PI * 2f - t) * minDim * 0.14f * env
        val yA = cy + wave
        val yB = cy - wave * 0.72f
        if (i == 0) {
            pathA.moveTo(x, yA)
            pathB.moveTo(x, yB)
        } else {
            pathA.lineTo(x, yA)
            pathB.lineTo(x, yB)
        }
    }
    drawPath(pathA, color = palette.primary.copy(alpha = 0.85f), style = pulseStroke)
    drawPath(pathB, color = palette.secondary.copy(alpha = 0.68f), style = shardStroke)

    // Two orbiting crystalline prism halves breathing smoothly around center
    val gap = minDim * 0.045f + (0.5f + 0.5f * sin(t)) * minDim * 0.035f
    val crystalH = minDim * 0.18f
    val crystalW = minDim * 0.11f
    for (dir in listOf(-1f, 1f)) {
        val ox = cx + dir * gap
        pathA.reset()
        pathA.moveTo(ox, cy - crystalH)
        pathA.lineTo(ox + dir * crystalW, cy - crystalH * 0.18f)
        pathA.lineTo(ox + dir * crystalW * 0.5f, cy + crystalH)
        pathA.close()
        drawPath(pathA, color = palette.highlight.copy(alpha = 0.28f))
        drawPath(pathA, color = palette.highlight.copy(alpha = 0.85f), style = shardStroke)
    }
}

private fun DrawScope.drawRadialSpectrumEqualizer(
    t: Float,
    palette: MoodCoverPalette,
    barStroke: Stroke,
    glowStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    val barCount = 18
    val baseR = minDim * 0.16f
    val deg = (t / TWO_PI) * 360f
    drawCircle(color = palette.primary.copy(alpha = 0.16f), radius = minDim * 0.34f, center = Offset(cx, cy), style = glowStroke)
    rotate(degrees = deg, pivot = Offset(cx, cy)) {
        for (i in 0 until barCount) {
            val a = i * (TWO_PI / barCount)
            val amp = 0.5f + 0.5f * sin(t * 2f + i * 2f * (TWO_PI / barCount) * 3f)
            val outerR = baseR + minDim * (0.06f + amp * 0.13f)
            val cosA = cos(a)
            val sinA = sin(a)
            drawLine(
                color = if (i % 2 == 0) palette.highlight else palette.primary,
                start = Offset(cx + cosA * baseR, cy + sinA * baseR),
                end = Offset(cx + cosA * outerR, cy + sinA * outerR),
                strokeWidth = barStroke.width,
                cap = StrokeCap.Round,
                alpha = 0.88f,
            )
        }
    }
}

private fun DrawScope.drawNocturnalStarlightHalo(
    t: Float,
    palette: MoodCoverPalette,
    ringStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    val breath = 0.88f + 0.12f * sin(t)

    for (rIdx in 0..2) {
        val rad = minDim * (0.14f + rIdx * 0.09f) * breath
        drawCircle(
            color = palette.secondary.copy(alpha = 0.36f - rIdx * 0.08f),
            radius = rad,
            center = Offset(cx, cy),
            style = ringStroke,
        )
    }
    for (s in 0 until 8) {
        val angle = -t + s * (TWO_PI / 8f)
        val dist = minDim * (0.16f + (s % 2) * 0.12f)
        val twinkle = 0.45f + 0.55f * (0.5f + 0.5f * cos(t * 2f + s))
        drawCircle(
            color = palette.highlight,
            alpha = twinkle * 0.90f,
            radius = minDim * 0.015f,
            center = Offset(cx + cos(angle) * dist, cy + sin(angle) * dist),
        )
    }
    drawCircle(
        color = palette.highlight.copy(alpha = 0.90f),
        radius = minDim * 0.055f,
        center = Offset(cx, cy),
    )
}

private fun DrawScope.drawSynthwaveSunsetOrb(
    t: Float,
    palette: MoodCoverPalette,
    haloBrush: Brush,
    waveStroke: Stroke,
) {
    val cx = size.width * 0.50f
    val cy = size.height * 0.50f
    val minDim = size.minDimension
    drawCircle(brush = haloBrush, radius = minDim * 0.34f, center = Offset(cx, cy))

    val orbR = minDim * 0.21f
    val pulseR = orbR * (0.94f + 0.06f * sin(t))
    drawCircle(
        brush = Brush.verticalGradient(
            colors = listOf(palette.highlight.copy(alpha = 0.94f), palette.primary.copy(alpha = 0.45f)),
            startY = cy - pulseR,
            endY = cy + pulseR,
        ),
        radius = pulseR,
        center = Offset(cx, cy),
    )
    // Smooth horizontal scanline stripes moving continuously with integer phase modulo step
    val step = pulseR * 0.32f
    val phaseOffset = (t / TWO_PI) * step
    for (k in -1..2) {
        val lineY = cy + k * step + phaseOffset - step * 0.5f
        if (lineY in (cy - pulseR * 0.2f)..(cy + pulseR * 0.85f)) {
            drawLine(
                color = palette.bg[1],
                start = Offset(cx - pulseR * 0.96f, lineY),
                end = Offset(cx + pulseR * 0.96f, lineY),
                strokeWidth = waveStroke.width * 2.2f,
                cap = StrokeCap.Round,
            )
        }
    }
}

internal fun resolveMoodTheme(moodTitle: String, moodIndex: Int): MoodTheme {
    val lower = moodTitle.lowercase(Locale.ROOT)
    return when {
        "heartbreak" in lower || "breakup" in lower || "break-up" in lower || "broken" in lower ->
            MoodTheme.HEARTBREAK
        "blue" in lower || "sad" in lower || "melanchol" in lower || "lonely" in lower ->
            MoodTheme.MELANCHOLY
        "chill" in lower || "relax" in lower || "unwind" in lower || "ambient" in lower || "calm" in lower || "lounge" in lower ->
            MoodTheme.CHILL
        "energ" in lower || "workout" in lower || "gym" in lower || "pump" in lower || "sport" in lower || "power" in lower ->
            MoodTheme.ENERGY
        "focus" in lower || "study" in lower || "work" in lower || "deep" in lower || "concentrat" in lower ->
            MoodTheme.FOCUS
        "romanc" in lower || "love" in lower || "date" in lower || "soul" in lower ->
            MoodTheme.ROMANCE
        "sleep" in lower || "night" in lower || "dream" in lower || "bed" in lower ->
            MoodTheme.SLEEP
        "party" in lower || "dance" in lower || "club" in lower || "friday" in lower ->
            MoodTheme.PARTY
        "commut" in lower || "driv" in lower || "road" in lower || "travel" in lower ->
            MoodTheme.COMMUTE
        "feel good" in lower || "happy" in lower || "sun" in lower || "morning" in lower || "upbeat" in lower ->
            MoodTheme.FEEL_GOOD
        "folk" in lower || "acoustic" in lower || "unplugged" in lower ->
            MoodTheme.FOLK_ACOUSTIC
        "hip" in lower || "rap" in lower || "trap" in lower || "r&b" in lower ->
            MoodTheme.URBAN_PRISM
        "indie" in lower || "alt" in lower ->
            MoodTheme.BOTANICAL_INDIE
        "jazz" in lower || "classical" in lower ->
            MoodTheme.HARMONIC_JAZZ
        "synth" in lower || "electro" in lower || "gaming" in lower ->
            MoodTheme.SYNTH_WAVE
        else -> {
            val all = MoodTheme.entries
            val hash = (moodTitle.hashCode() xor (moodIndex * 31)) and 0x7FFF_FFFF
            all[hash % all.size]
        }
    }
}

private fun DrawScope.drawHarmonicStrings(t: Float, path: Path, stroke: Stroke, color: Color) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val halfSpan = minDim * 0.34f
    for (s in 0..3) {
        val yBase = cy + (s - 1.5f) * minDim * 0.10f
        val amp = minDim * 0.045f * sin(t * 2f + s * 1.1f)
        path.reset()
        path.moveTo(cx - halfSpan, yBase)
        path.quadraticTo(cx, yBase + amp, cx + halfSpan, yBase)
        drawPath(path = path, color = color.copy(alpha = 0.65f), style = stroke)
    }
}

private fun DrawScope.drawLunariaPetals(t: Float, stroke: Stroke, fill: Color, border: Color) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val r = size.minDimension * 0.14f
    val topLeft = Offset(cx - r * 0.45f, cy - r * 1.35f)
    val petalSize = Size(r * 0.9f, r * 1.35f)
    for (i in 0 until 5) {
        // Strictly 1 full rotation per period for zero seam
        val deg = i * 72f + (t / TWO_PI) * 360f
        rotate(degrees = deg, pivot = Offset(cx, cy)) {
            drawOval(color = fill.copy(alpha = 0.28f), topLeft = topLeft, size = petalSize)
            drawOval(color = border.copy(alpha = 0.60f), topLeft = topLeft, size = petalSize, style = stroke)
        }
    }
}

private fun DrawScope.drawBrassWaves(t: Float, path: Path, stroke: Stroke, c1: Color, c2: Color) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val span = minDim * 0.68f
    val startX = cx - span * 0.5f
    val colors = arrayOf(c1.copy(alpha = 0.72f), c2.copy(alpha = 0.55f))
    for (k in 0..1) {
        path.reset()
        for (i in 0..16) {
            val u = i / 16f
            val x = startX + span * u
            val env = sin(u * PI.toFloat())
            val y = cy + sin(u * TWO_PI * (1.5f + k * 0.5f) + t * (k + 1)) * minDim * 0.11f * env
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(path = path, color = colors[k], style = stroke)
    }
}
