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
 * Always-animated personal station artwork featuring Lunaria moonlit orbital rings
 * and a flowing iridescent silver-violet silk light ribbon, strictly clipped & centered.
 */
@Composable
fun SonataPersonalStationArt(
    title: String,
    modifier: Modifier = Modifier,
) {
    val clockMs = rememberSharedArtClockMs()
    val seed = remember(title) { (title.hashCode() and 0x7FFF_FFFF) % 360 }

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val cx = size.width * 0.5f
                val cy = size.height * 0.5f
                val minDim = size.minDimension
                val haloR = minDim * 0.46f
                val coreR = minDim * 0.13f
                val ribbonSpan = minDim * 0.84f
                val startX = cx - ribbonSpan * 0.5f

                val bgBrush = Brush.linearGradient(
                    colors = listOf(
                        Color(0xFF0E0D1B),
                        Color(0xFF1B1635),
                        Color(0xFF101829),
                    ),
                    start = Offset.Zero,
                    end = Offset(size.width, size.height),
                )
                val haloBrush = Brush.radialGradient(
                    colors = listOf(
                        Color(0x5C9F8BFF),
                        Color(0x245D50A6),
                        Color.Transparent,
                    ),
                    center = Offset.Zero,
                    radius = haloR,
                )
                val ribbonBrush = Brush.horizontalGradient(
                    colors = listOf(
                        Color.Transparent,
                        Color(0x8C9F8BFF),
                        Color(0xD9F3EEFF),
                        Color(0x8CB3D9FF),
                        Color.Transparent,
                    ),
                    startX = startX,
                    endX = startX + ribbonSpan,
                )
                val coreBrush = Brush.radialGradient(
                    colors = listOf(
                        Color(0xEBFFFFFF),
                        Color(0x7AF3EEFF),
                        Color.Transparent,
                    ),
                    center = Offset(cx, cy),
                    radius = coreR,
                )
                val ringRadii = floatArrayOf(minDim * 0.18f, minDim * 0.265f, minDim * 0.35f)
                val ringColors = arrayOf(
                    Color(0x40D9CEFF),
                    Color(0x33D9CEFF),
                    Color(0x26D9CEFF),
                )
                val ringStroke = Stroke(width = minDim * 0.013f)
                val ribbonStroke = Stroke(width = minDim * 0.040f, cap = StrokeCap.Round)
                val nodeColor = Color(0xD9F3EEFF)
                val ribbonPath = Path()
                val baseTilt = seed.toFloat()

                onDrawBehind {
                    val phase = (clockMs.value % 18_000L) / 18_000f
                    val t = phase * TWO_PI
                    clipRect {
                        drawRect(brush = bgBrush)

                        // Centered soft lunar halo translated smoothly (zero Brush alloc)
                        translate(
                            left = cx + cos(t) * minDim * 0.04f,
                            top = cy + sin(t) * minDim * 0.04f,
                        ) {
                            drawCircle(
                                brush = haloBrush,
                                radius = haloR,
                                center = Offset.Zero,
                            )
                        }

                        // Centered orbiting Lunaria rings
                        for (ringIdx in 0..2) {
                            val radius = ringRadii[ringIdx]
                            val orbitAngle =
                                baseTilt + ringIdx * 42f + phase * 360f * (if (ringIdx % 2 == 0) 1f else -0.65f)
                            rotate(degrees = orbitAngle, pivot = Offset(cx, cy)) {
                                drawOval(
                                    color = ringColors[ringIdx],
                                    topLeft = Offset(cx - radius, cy - radius * 0.62f),
                                    size = Size(radius * 2f, radius * 1.24f),
                                    style = ringStroke,
                                )
                                drawCircle(
                                    color = nodeColor,
                                    radius = minDim * (0.020f - ringIdx * 0.003f),
                                    center = Offset(cx + radius, cy),
                                )
                            }
                        }

                        // Centered flowing iridescent silk ribbon
                        ribbonPath.reset()
                        val steps = 16
                        for (i in 0..steps) {
                            val u = i / steps.toFloat()
                            val x = startX + ribbonSpan * u
                            val wave = sin(u * TWO_PI * 1.4f + t) * minDim * 0.10f +
                                cos(u * TWO_PI * 0.7f - t * 0.5f) * minDim * 0.04f
                            val y = cy + wave
                            if (i == 0) ribbonPath.moveTo(x, y) else ribbonPath.lineTo(x, y)
                        }
                        drawPath(
                            path = ribbonPath,
                            brush = ribbonBrush,
                            style = ribbonStroke,
                        )

                        // Central moonstone core
                        drawCircle(
                            brush = coreBrush,
                            radius = coreR,
                            center = Offset(cx, cy),
                        )
                    }
                }
            },
    )
}

/**
 * Always-animated discovery station artwork featuring a luminous lunar prism
 * and drifting starlight constellation arcs over deep eclipse indigo, strictly clipped & centered.
 */
@Composable
fun SonataDiscoveryStationArt(
    modifier: Modifier = Modifier,
) {
    val clockMs = rememberSharedArtClockMs()

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val cx = size.width * 0.5f
                val cy = size.height * 0.5f
                val minDim = size.minDimension
                val prismR = minDim * 0.20f

                val bgBrush = Brush.radialGradient(
                    colors = listOf(
                        Color(0xFF1A2246),
                        Color(0xFF11142B),
                        Color(0xFF080A14),
                    ),
                    center = Offset(cx, size.height * 0.45f),
                    radius = size.maxDimension * 0.78f,
                )
                val prismBrush = Brush.linearGradient(
                    colors = listOf(
                        Color(0x6BD6E4FF),
                        Color(0x479C8CFF),
                        Color(0x5C56B4E9),
                    ),
                    start = Offset(cx - prismR, cy - prismR),
                    end = Offset(cx + prismR, cy + prismR),
                )
                val prismPath = Path().apply {
                    moveTo(cx, cy - prismR)
                    lineTo(cx + prismR * 0.78f, cy)
                    lineTo(cx, cy + prismR)
                    lineTo(cx - prismR * 0.78f, cy)
                    close()
                }
                val arcStroke = Stroke(width = minDim * 0.011f, cap = StrokeCap.Round)
                val prismStroke = Stroke(width = minDim * 0.013f)
                val prismBorderColor = Color(0x8CFFFFFF)
                val moonTint = Color(0xFFEBE4FF)
                val arcColors = arrayOf(
                    Color(0x42B8CEFF),
                    Color(0x3BB8CEFF),
                    Color(0x33B8CEFF),
                    Color(0x2BB8CEFF),
                )

                onDrawBehind {
                    val phase = (clockMs.value % 22_000L) / 22_000f
                    val t = phase * TWO_PI
                    clipRect {
                        drawRect(brush = bgBrush)

                        // Centered drifting constellation arcs
                        for (i in 0..3) {
                            val arcR = minDim * (0.20f + i * 0.072f)
                            val startDeg =
                                (i * 75f + phase * 360f * (if (i % 2 == 0) 0.5f else -0.35f)) % 360f
                            drawArc(
                                color = arcColors[i],
                                startAngle = startDeg,
                                sweepAngle = 140f,
                                useCenter = false,
                                topLeft = Offset(cx - arcR, cy - arcR),
                                size = Size(arcR * 2f, arcR * 2f),
                                style = arcStroke,
                            )
                        }

                        // Starlight constellation nodes
                        val nodeRadius = minDim * 0.015f
                        for (s in 0 until 7) {
                            val angle = s * (TWO_PI / 7f) + t * 0.25f
                            val dist = minDim * (0.19f + (s % 3) * 0.09f)
                            val twinkle = 0.45f + 0.55f * (0.5f + 0.5f * sin(t * 2f + s * 1.3f))
                            drawCircle(
                                color = moonTint,
                                alpha = twinkle * 0.85f,
                                radius = nodeRadius,
                                center = Offset(cx + cos(angle) * dist, cy + sin(angle) * dist),
                            )
                        }

                        // Rotating lunar diamond prism centered at (cx, cy)
                        rotate(degrees = sin(t) * 14f, pivot = Offset(cx, cy)) {
                            drawPath(path = prismPath, brush = prismBrush)
                            drawPath(path = prismPath, color = prismBorderColor, style = prismStroke)
                        }
                    }
                }
            },
    )
}

private enum class MoodTheme {
    CHILL,
    ENERGY,
    FOCUS,
    ROMANCE,
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
 * Unique, always-animated artwork tailored deterministically to every mood in Find Your Mood.
 * Strictly centered around `(size.width * 0.5f, size.height * 0.5f)`, clipped via
 * `clipToBounds()` + `clipRect {}`, and powered by the shared 30fps clock with zero per-frame allocations.
 */
@Composable
fun SonataMoodAnimatedArt(
    moodTitle: String,
    moodIndex: Int,
    modifier: Modifier = Modifier,
) {
    val theme = remember(moodTitle, moodIndex) { resolveMoodTheme(moodTitle, moodIndex) }
    val periodMs = when (theme) {
        MoodTheme.SLEEP, MoodTheme.CHILL -> 24_000L
        MoodTheme.PARTY, MoodTheme.ENERGY -> 12_000L
        else -> 16_000L
    }
    val clockMs = rememberSharedArtClockMs()

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .drawWithCache {
                val bgBrush = moodBackgroundBrush(theme, size)
                val cx = size.width * 0.5f
                val cy = size.height * 0.5f
                val minDim = size.minDimension
                val reusablePath = Path()

                // Pre-cached theme brushes & strokes (zero allocations inside onDrawBehind)
                val romanceHaloBrush = if (theme == MoodTheme.ROMANCE) {
                    Brush.radialGradient(
                        colors = listOf(Color(0x6BFF9CC4), Color.Transparent),
                        center = Offset(cx, cy),
                        radius = minDim * 0.40f,
                    )
                } else null
                val romanceRibbonSpan = minDim * 0.80f
                val romanceRibbonStartX = cx - romanceRibbonSpan * 0.5f
                val romanceRibbonBrush = if (theme == MoodTheme.ROMANCE) {
                    Brush.horizontalGradient(
                        colors = listOf(Color.Transparent, Color(0xB3FFC2DC), Color.Transparent),
                        startX = romanceRibbonStartX,
                        endX = romanceRibbonStartX + romanceRibbonSpan,
                    )
                } else null
                val sunriseBrush = if (theme == MoodTheme.FEEL_GOOD) {
                    Brush.radialGradient(
                        colors = listOf(Color(0xD9FFE29A), Color(0x59FF9852), Color.Transparent),
                        center = Offset(cx, cy),
                        radius = minDim * 0.36f,
                    )
                } else null
                val synthSunR = minDim * 0.18f
                val synthSunCy = cy - minDim * 0.10f
                val synthSunBrush = if (theme == MoodTheme.SYNTH_WAVE) {
                    Brush.verticalGradient(
                        colors = listOf(Color(0xCCFF7BD8), Color(0x4D8C66FF)),
                        startY = synthSunCy - synthSunR,
                        endY = synthSunCy + synthSunR,
                    )
                } else null
                if (theme == MoodTheme.SYNTH_WAVE) {
                    reusablePath.addOval(
                        Rect(
                            left = cx - synthSunR,
                            top = synthSunCy - synthSunR,
                            right = cx + synthSunR,
                            bottom = synthSunCy + synthSunR,
                        ),
                    )
                }

                val chillStroke = Stroke(width = minDim * 0.013f)
                val energyStroke = Stroke(width = minDim * 0.020f, cap = StrokeCap.Round)
                val focusStroke = Stroke(width = minDim * 0.010f)
                val romanceStroke = Stroke(width = minDim * 0.040f, cap = StrokeCap.Round)
                val sunriseStroke = Stroke(width = minDim * 0.012f)
                val folkStroke = Stroke(width = minDim * 0.014f, cap = StrokeCap.Round)
                val prismStroke = Stroke(width = minDim * 0.018f)
                val prismRingStroke = Stroke(width = minDim * 0.010f)
                val petalStroke = Stroke(width = minDim * 0.010f)
                val brassStroke = Stroke(width = minDim * 0.018f, cap = StrokeCap.Round)

                onDrawBehind {
                    val phase = (clockMs.value % periodMs) / periodMs.toFloat()
                    val t = phase * TWO_PI
                    clipRect {
                        drawRect(brush = bgBrush)
                        when (theme) {
                            MoodTheme.CHILL -> drawLunarRipples(t, chillStroke)
                            MoodTheme.ENERGY -> drawKineticPulseArcs(t, energyStroke)
                            MoodTheme.FOCUS -> drawGeometricOrbit(t, focusStroke)
                            MoodTheme.ROMANCE -> drawRoseMoonAurora(
                                t,
                                reusablePath,
                                romanceHaloBrush!!,
                                romanceRibbonBrush!!,
                                romanceRibbonStartX,
                                romanceRibbonSpan,
                                romanceStroke,
                            )
                            MoodTheme.SLEEP -> drawNebulaCrescent(t)
                            MoodTheme.PARTY -> drawChromaticBeams(t, reusablePath)
                            MoodTheme.MELANCHOLY -> drawSilverRain(t)
                            MoodTheme.COMMUTE -> drawHorizonMotion(t)
                            MoodTheme.FEEL_GOOD -> drawSunriseCrescent(t, sunriseBrush!!, sunriseStroke)
                            MoodTheme.FOLK_ACOUSTIC -> drawHarmonicStrings(t, reusablePath, folkStroke)
                            MoodTheme.URBAN_PRISM -> drawGeometricPrism(t, reusablePath, prismStroke, prismRingStroke)
                            MoodTheme.BOTANICAL_INDIE -> drawLunariaPetals(t, petalStroke)
                            MoodTheme.HARMONIC_JAZZ -> drawBrassWaves(t, reusablePath, brassStroke)
                            MoodTheme.SYNTH_WAVE -> drawSynthWaveGrid(t, reusablePath, synthSunBrush!!)
                        }
                    }
                }
            },
    )
}

private fun resolveMoodTheme(moodTitle: String, moodIndex: Int): MoodTheme {
    val lower = moodTitle.lowercase(Locale.ROOT)
    return when {
        "chill" in lower || "relax" in lower || "ambient" in lower || "calm" in lower || "lounge" in lower ->
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
        "sad" in lower || "melanchol" in lower || "blue" in lower || "heartbreak" in lower || "rain" in lower ->
            MoodTheme.MELANCHOLY
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

private val MOOD_BG_COLORS = mapOf(
    MoodTheme.CHILL to listOf(Color(0xFF0A1D26), Color(0xFF10313E), Color(0xFF07131A)),
    MoodTheme.ENERGY to listOf(Color(0xFF10241A), Color(0xFF16432B), Color(0xFF0B1510)),
    MoodTheme.FOCUS to listOf(Color(0xFF0D1428), Color(0xFF17254D), Color(0xFF090D1A)),
    MoodTheme.ROMANCE to listOf(Color(0xFF281020), Color(0xFF471A36), Color(0xFF160812)),
    MoodTheme.SLEEP to listOf(Color(0xFF090A18), Color(0xFF141736), Color(0xFF05060E)),
    MoodTheme.PARTY to listOf(Color(0xFF220E36), Color(0xFF3E1559), Color(0xFF120822)),
    MoodTheme.MELANCHOLY to listOf(Color(0xFF111824), Color(0xFF1E2B3D), Color(0xFF0A0F17)),
    MoodTheme.COMMUTE to listOf(Color(0xFF1B1429), Color(0xFF34224B), Color(0xFF111828)),
    MoodTheme.FEEL_GOOD to listOf(Color(0xFF2C1B10), Color(0xFF533117), Color(0xFF1B100A)),
    MoodTheme.FOLK_ACOUSTIC to listOf(Color(0xFF221712), Color(0xFF41291D), Color(0xFF140D0A)),
    MoodTheme.URBAN_PRISM to listOf(Color(0xFF141418), Color(0xFF282319), Color(0xFF0B0B0E)),
    MoodTheme.BOTANICAL_INDIE to listOf(Color(0xFF13221E), Color(0xFF243B35), Color(0xFF0C1513)),
    MoodTheme.HARMONIC_JAZZ to listOf(Color(0xFF25121A), Color(0xFF421C29), Color(0xFF14090E)),
    MoodTheme.SYNTH_WAVE to listOf(Color(0xFF120D2A), Color(0xFF241752), Color(0xFF091B2C)),
)

private fun moodBackgroundBrush(theme: MoodTheme, size: Size): Brush = Brush.linearGradient(
    colors = MOOD_BG_COLORS.getValue(theme),
    start = Offset.Zero,
    end = Offset(size.width, size.height),
)

private val CHILL_RIPPLE_COLOR = Color(0xFF8BE2EC)
private val CHILL_CORE_COLOR = Color(0xB8D7F8FF)

private fun DrawScope.drawLunarRipples(t: Float, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val phaseNorm = t / TWO_PI
    for (i in 0..3) {
        val progress = (phaseNorm + i * 0.25f) % 1f
        val radius = minDim * (0.08f + progress * 0.34f)
        drawCircle(
            color = CHILL_RIPPLE_COLOR,
            alpha = (1f - progress) * 0.45f,
            radius = radius,
            center = Offset(cx, cy),
            style = stroke,
        )
    }
    drawCircle(
        color = CHILL_CORE_COLOR,
        radius = minDim * 0.055f,
        center = Offset(cx, cy),
    )
}

private val ENERGY_ARC_COLOR = Color(0xFF5CE698)

private fun DrawScope.drawKineticPulseArcs(t: Float, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val pulse = 0.5f + 0.5f * sin(t * 2f)
    for (i in 0..3) {
        val r = minDim * (0.14f + i * 0.085f + pulse * 0.02f)
        val startAngle = (i * 55f + (t / TWO_PI) * 360f) % 360f
        drawArc(
            color = ENERGY_ARC_COLOR,
            alpha = 0.55f - i * 0.10f,
            startAngle = startAngle,
            sweepAngle = 95f,
            useCenter = false,
            topLeft = Offset(cx - r, cy - r),
            size = Size(r * 2f, r * 2f),
            style = stroke,
        )
    }
}

private val FOCUS_RING_COLOR = Color(0x4D93B4FF)
private val FOCUS_NODE_COLOR = Color(0xE6DDE7FF)

private fun DrawScope.drawGeometricOrbit(t: Float, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val nodeRadius = minDim * 0.020f
    for (i in 1..3) {
        val r = minDim * (0.125f * i)
        drawCircle(
            color = FOCUS_RING_COLOR,
            radius = r,
            center = Offset(cx, cy),
            style = stroke,
        )
        val angle = t * (if (i % 2 == 0) 1f else -1f) + i
        drawCircle(
            color = FOCUS_NODE_COLOR,
            radius = nodeRadius,
            center = Offset(cx + cos(angle) * r, cy + sin(angle) * r),
        )
    }
}

private fun DrawScope.drawRoseMoonAurora(
    t: Float,
    path: Path,
    haloBrush: Brush,
    ribbonBrush: Brush,
    startX: Float,
    span: Float,
    stroke: Stroke,
) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    drawCircle(
        brush = haloBrush,
        radius = minDim * 0.40f,
        center = Offset(cx, cy),
    )
    path.reset()
    for (i in 0..14) {
        val u = i / 14f
        val x = startX + span * u
        val y = cy + sin(u * TWO_PI + t) * minDim * 0.12f
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    drawPath(path = path, brush = ribbonBrush, style = stroke)
}

private val SLEEP_MOON_COLOR = Color(0xE0E7E2FF)
private val SLEEP_CUTOUT_COLOR = Color(0xFF10122B)

private fun DrawScope.drawNebulaCrescent(t: Float) {
    val baseCx = size.width * 0.5f
    val baseCy = size.height * 0.5f
    val minDim = size.minDimension
    val cx = baseCx + cos(t) * minDim * 0.02f
    val cy = baseCy + sin(t) * minDim * 0.02f
    val moonR = minDim * 0.16f
    drawCircle(color = SLEEP_MOON_COLOR, radius = moonR, center = Offset(cx, cy))
    drawCircle(
        color = SLEEP_CUTOUT_COLOR,
        radius = moonR * 0.88f,
        center = Offset(cx + moonR * 0.36f, cy - moonR * 0.22f),
    )
    val starR = minDim * 0.012f
    for (i in 0 until 6) {
        val angle = i * (TWO_PI / 6f) + t * 0.35f
        val dist = minDim * (0.28f + (i % 2) * 0.10f)
        drawCircle(
            color = Color.White,
            alpha = 0.35f + 0.45f * (0.5f + 0.5f * sin(t * 2f + i)),
            radius = starR,
            center = Offset(baseCx + cos(angle) * dist, baseCy + sin(angle) * dist),
        )
    }
}

private val PARTY_BEAM_COLORS = arrayOf(Color(0x57FF6EC7), Color(0x579F7BFF), Color(0x57FFD166))
private val PARTY_CORE_COLOR = Color(0x73FFFFFF)

private fun DrawScope.drawChromaticBeams(t: Float, path: Path) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val beamLen = minDim * 0.43f
    val beamHalfWidth = minDim * 0.08f
    for (idx in PARTY_BEAM_COLORS.indices) {
        val angle = t + idx * (TWO_PI / 3f)
        val dx = cos(angle) * beamLen
        val dy = sin(angle) * beamLen
        val perpX = -sin(angle) * beamHalfWidth
        val perpY = cos(angle) * beamHalfWidth
        path.reset()
        path.moveTo(cx, cy)
        path.lineTo(cx + dx + perpX, cy + dy + perpY)
        path.lineTo(cx + dx - perpX, cy + dy - perpY)
        path.close()
        drawPath(path = path, color = PARTY_BEAM_COLORS[idx])
    }
    drawCircle(color = PARTY_CORE_COLOR, radius = minDim * 0.055f, center = Offset(cx, cy))
}

private val RAIN_COLOR = Color(0x7AB7CDE8)

private fun DrawScope.drawSilverRain(t: Float) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val p = t / TWO_PI
    val boxSpan = minDim * 0.72f
    val startX = cx - boxSpan * 0.5f
    val topY = cy - boxSpan * 0.45f
    val strokeW = minDim * 0.012f
    for (i in 0 until 7) {
        val frac = (p + i * 0.143f) % 1f
        val x = startX + ((i + 0.5f) / 7f) * boxSpan - frac * minDim * 0.08f
        val y = topY + frac * boxSpan * 0.85f
        drawLine(
            color = RAIN_COLOR,
            start = Offset(x, y),
            end = Offset(x - minDim * 0.04f, y + minDim * 0.11f),
            strokeWidth = strokeW,
            cap = StrokeCap.Round,
        )
    }
}

private val HORIZON_COLOR = Color(0xFFFFB27D)

private fun DrawScope.drawHorizonMotion(t: Float) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val p = t / TWO_PI
    val span = minDim * 0.76f
    val left = cx - span * 0.5f
    val streakLen = minDim * 0.22f
    val strokeW = minDim * 0.017f
    for (i in 0 until 5) {
        val y = cy + (i - 2) * minDim * 0.11f
        val offset = ((p + i * 0.21f) % 1f) * (span - streakLen)
        drawLine(
            color = HORIZON_COLOR,
            alpha = 0.55f - i * 0.06f,
            start = Offset(left + offset, y),
            end = Offset(left + offset + streakLen, y),
            strokeWidth = strokeW,
            cap = StrokeCap.Round,
        )
    }
}

private val SUNRISE_RING_COLOR = Color(0x66FFE8B5)

private fun DrawScope.drawSunriseCrescent(t: Float, sunBrush: Brush, ringStroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val pulse = 1f + 0.06f * sin(t)
    val radius = minDim * 0.36f * pulse
    drawCircle(brush = sunBrush, radius = radius, center = Offset(cx, cy))
    drawCircle(
        color = SUNRISE_RING_COLOR,
        radius = minDim * 0.24f,
        center = Offset(cx, cy),
        style = ringStroke,
    )
}

private val FOLK_STRING_COLOR = Color(0x94F0B98D)

private fun DrawScope.drawHarmonicStrings(t: Float, path: Path, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val halfSpan = minDim * 0.42f
    for (s in 0..3) {
        val yBase = cy + (s - 1.5f) * minDim * 0.11f
        val amp = minDim * 0.045f * sin(t * 2f + s * 1.1f)
        path.reset()
        path.moveTo(cx - halfSpan, yBase)
        path.quadraticTo(cx, yBase + amp, cx + halfSpan, yBase)
        drawPath(path = path, color = FOLK_STRING_COLOR, style = stroke)
    }
}

private val PRISM_GOLD = Color(0xA6F3CF7A)
private val PRISM_GOLD_FAINT = Color(0x38F3CF7A)

private fun DrawScope.drawGeometricPrism(t: Float, path: Path, stroke: Stroke, ringStroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val r = minDim * 0.27f
    path.reset()
    for (i in 0..2) {
        val a = t + i * (TWO_PI / 3f)
        val x = cx + cos(a) * r
        val y = cy + sin(a) * r
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    path.close()
    drawPath(path = path, color = PRISM_GOLD, style = stroke)
    drawCircle(color = PRISM_GOLD_FAINT, radius = minDim * 0.36f, center = Offset(cx, cy), style = ringStroke)
}

private val PETAL_FILL = Color(0x42DCEBE4)
private val PETAL_BORDER = Color(0x85EAF7F1)

private fun DrawScope.drawLunariaPetals(t: Float, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val r = size.minDimension * 0.17f
    val topLeft = Offset(cx - r * 0.45f, cy - r * 1.35f)
    val petalSize = Size(r * 0.9f, r * 1.35f)
    for (i in 0 until 5) {
        val deg = i * 72f + (t / TWO_PI) * 360f * 0.4f
        rotate(degrees = deg, pivot = Offset(cx, cy)) {
            drawOval(color = PETAL_FILL, topLeft = topLeft, size = petalSize)
            drawOval(color = PETAL_BORDER, topLeft = topLeft, size = petalSize, style = stroke)
        }
    }
}

private val JAZZ_COLORS = arrayOf(Color(0xA6FFD89E), Color(0x73E996B0))

private fun DrawScope.drawBrassWaves(t: Float, path: Path, stroke: Stroke) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val span = minDim * 0.82f
    val startX = cx - span * 0.5f
    for (k in 0..1) {
        path.reset()
        for (i in 0..16) {
            val u = i / 16f
            val x = startX + span * u
            val y = cy + sin(u * TWO_PI * (1.5f + k * 0.5f) + t * (k + 1)) * minDim * 0.11f
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(path = path, color = JAZZ_COLORS[k], style = stroke)
    }
}

private val SYNTH_GRID_COLOR = Color(0xFF56E4FF)

private fun DrawScope.drawSynthWaveGrid(t: Float, sunPath: Path, sunBrush: Brush) {
    val cx = size.width * 0.5f
    val cy = size.height * 0.5f
    val minDim = size.minDimension
    val horizonY = cy + minDim * 0.03f
    val gridSpan = minDim * 0.76f
    val left = cx - gridSpan * 0.5f
    val right = cx + gridSpan * 0.5f
    val bottom = cy + minDim * 0.40f
    val strokeW = minDim * 0.010f
    val p = t / TWO_PI
    for (i in 0..4) {
        val frac = (p + i * 0.2f) % 1f
        val y = horizonY + (frac * frac) * (bottom - horizonY)
        drawLine(
            color = SYNTH_GRID_COLOR,
            alpha = 0.45f * frac,
            start = Offset(left, y),
            end = Offset(right, y),
            strokeWidth = strokeW,
        )
    }
    drawPath(path = sunPath, brush = sunBrush)
}
