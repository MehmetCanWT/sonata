/*
 * Artwork color extraction, mesh gradient backdrop, and blurred cover backdrop.
 */
package dev.sonata.android.ui

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.location.LocationManager
import android.util.LruCache
import android.view.Window
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.EaseInCubic
import androidx.compose.animation.core.EaseInOut
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.PathOperation
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.ResolvedTextDirection
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.core.graphics.ColorUtils
import androidx.core.view.WindowCompat
import androidx.palette.graphics.Palette
import coil3.SingletonImageLoader
import coil3.compose.AsyncImage
import coil3.compose.LocalPlatformContext
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.request.crossfade
import coil3.size.Precision
import coil3.toBitmap
import dev.sonata.android.LocalReduceAnimation
import dev.sonata.android.PlaybackClockState
import dev.sonata.android.R
import dev.sonata.android.SquirclePillShape
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.lightweightLiquidGlass
import dev.sonata.android.ui.components.SonataMoodAnimatedArt
import dev.sonata.android.ui.player.CardCanvasSurface
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.LunariaColors
import dev.sonata.android.ui.theme.SonataAppFontFamily
import dev.sonata.android.ui.theme.lunariaColorScheme
import dev.sonata.android.ui.theme.lunariaTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidLyricWord
import java.util.Locale
import java.util.TimeZone
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.random.Random

// =============================================================================
// 5. Artwork Palette, Mesh Gradient Backdrop & Fade Curves
// =============================================================================

/** Smootherstep `6t^5 - 15t^4 + 10t^3` with zero slope and curvature at both ends. */
fun smootherStep(t: Float): Float {
    val x = t.coerceIn(0f, 1f)
    return x * x * x * (x * (x * 6f - 15f) + 10f)
}

const val FADE_CURVE_STOPS = 16
const val HERO_FADE_FRACTION = 0.42f

/** 16-stop smootherstep opacity mask used with [BlendMode.DstIn] at the bottom of hero artwork. */
val HERO_FADE_STOPS: Array<Pair<Float, Color>> = Array(FADE_CURVE_STOPS + 1) { i ->
    val t = i / FADE_CURVE_STOPS.toFloat()
    t to Color.Black.copy(alpha = 1f - smootherStep(t))
}

fun Modifier.fadingEdges(
    width: Dp,
    start: () -> Float = { 0f },
    end: () -> Float = { 0f },
): Modifier = graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithCache {
        val fade = width.toPx().coerceAtMost(size.width / 2f)
        val stops = Array(FADE_CURVE_STOPS + 1) { i ->
            val t = i / FADE_CURVE_STOPS.toFloat()
            t to Color.Black.copy(alpha = 1f - smootherStep(t))
        }
        val leftBrush = Brush.horizontalGradient(*stops, startX = 0f, endX = fade)
        val rightBrush = Brush.horizontalGradient(*stops, startX = size.width, endX = size.width - fade)
        val rtl = layoutDirection == LayoutDirection.Rtl
        onDrawWithContent {
            drawContent()
            val left = (if (rtl) end() else start()).coerceIn(0f, 1f)
            if (left > 0f) {
                drawRect(leftBrush, Offset.Zero, Size(fade, size.height), alpha = left, blendMode = BlendMode.DstOut)
            }
            val right = (if (rtl) start() else end()).coerceIn(0f, 1f)
            if (right > 0f) {
                drawRect(
                    rightBrush,
                    Offset(size.width - fade, 0f),
                    Size(fade, size.height),
                    alpha = right,
                    blendMode = BlendMode.DstOut,
                )
            }
        }
    }

@Immutable
data class ArtworkPalette(
    val background: Color,
    val wash: Color,
    val elevated: Color,
    val accent: Color,
    val onBackground: Color,
    val onBackgroundVariant: Color,
    val divider: Color,
    val meshColors: List<Color> = listOf(accent, wash, elevated, background),
)

private data class ArtworkSeed(
    val dominant: Color,
    val vibrant: Color,
    val edge: Color,
    val meshColors: List<Color>,
)

private val artworkSeedCache = LruCache<String, ArtworkSeed>(128)
private const val PALETTE_SOURCE_PX = 128
private const val PALETTE_FADE_MS = 260

private val DefaultFallbackColors = listOf(
    Color(0xFF3A1C71),
    Color(0xFFD76D77),
    Color(0xFF2B5876),
    Color(0xFFFFAF7B),
)

private val BasicFallbackColors = listOf(
    Color(0xFF1C1C1E),
    Color(0xFF2C2C2E),
    Color(0xFF0D0D0F),
    Color(0xFF141416),
)

@Composable
fun rememberArtworkPalette(
    artworkUrl: String?,
    dark: Boolean = true,
    artPx: Int = CARD_ART_PX,
    animated: Boolean = true,
): ArtworkPalette {
    val context = LocalPlatformContext.current
    val scheme = MaterialTheme.colorScheme
    val tokens = LocalLunariaColors.current
    val reduceAnimation = LocalReduceAnimation.current
    var seed by remember(artworkUrl) { mutableStateOf(artworkUrl?.let(artworkSeedCache::get)) }
    val knownUpFront = remember(artworkUrl) { seed != null }

    LaunchedEffect(artworkUrl, artPx) {
        if (artworkUrl.isNullOrBlank() || seed != null) return@LaunchedEffect
        val request = ImageRequest.Builder(context)
            .data(artworkUrl.artworkAt(artPx))
            .size(PALETTE_SOURCE_PX)
            .precision(Precision.INEXACT)
            .allowHardware(false)
            .crossfade(false)
            .build()
        val result = SingletonImageLoader.get(context).execute(request)
        val bitmap = (result as? SuccessResult)?.image?.toBitmap() ?: return@LaunchedEffect
        val extracted = withContext(Dispatchers.Default) { extractArtworkSeed(bitmap) } ?: return@LaunchedEffect
        artworkSeedCache.put(artworkUrl, extracted)
        seed = extracted
    }

    val target = remember(seed, dark, scheme, tokens.isBasic) {
        seed?.toPalette(dark) ?: ArtworkPalette(
            background = scheme.background,
            wash = scheme.background,
            elevated = scheme.surfaceVariant,
            accent = if (tokens.isBasic) BasicPalette.AccentRed else SonataAccent,
            onBackground = scheme.onBackground,
            onBackgroundVariant = scheme.onSurfaceVariant,
            divider = scheme.outline,
            meshColors = if (tokens.isBasic) BasicFallbackColors else DefaultFallbackColors,
        )
    }

    if (!animated || reduceAnimation || knownUpFront) {
        return target
    }

    val spec: AnimationSpec<Color> = tween(PALETTE_FADE_MS)
    return ArtworkPalette(
        background = animateColorAsState(target.background, spec, label = "artBg").value,
        wash = animateColorAsState(target.wash, spec, label = "artWash").value,
        elevated = animateColorAsState(target.elevated, spec, label = "artElev").value,
        accent = animateColorAsState(target.accent, spec, label = "artAccent").value,
        onBackground = animateColorAsState(target.onBackground, spec, label = "artOnBg").value,
        onBackgroundVariant = animateColorAsState(target.onBackgroundVariant, spec, label = "artOnVar").value,
        divider = animateColorAsState(target.divider, spec, label = "artDiv").value,
        meshColors = target.meshColors,
    )
}

fun ArtworkPalette.toMeshPalette(): MeshPalette = MeshPalette(meshColors)

private fun extractArtworkSeed(bitmap: Bitmap): ArtworkSeed? {
    // Run a single median-cut quantization pass and filter extreme near-black/white swatches in Kotlin.
    val all = Palette.from(bitmap).maximumColorCount(16).clearFilters().generate().swatches
    if (all.isEmpty()) return null
    val hsl = FloatArray(3)
    val filtered = all.filter { swatch ->
        ColorUtils.colorToHSL(swatch.rgb, hsl)
        hsl[2] in 0.08f..0.92f
    }.ifEmpty { all }
    val dominant = all.maxBy { it.population }
    val vibrant = filtered.maxBy { swatch ->
        ColorUtils.colorToHSL(swatch.rgb, hsl)
        hsl[1] * sqrt(swatch.population.toFloat())
    }
    val edge = bitmap.sampleBottomBandColor()
    val mesh = filtered
        .sortedByDescending { it.population }
        .map { Color(it.rgb) }
        .distinctMeshColors()
    return ArtworkSeed(
        dominant = Color(dominant.rgb),
        vibrant = Color(vibrant.rgb),
        edge = edge,
        meshColors = mesh,
    )
}

private fun Bitmap.sampleBottomBandColor(): Color {
    val w = width.coerceAtLeast(1)
    val h = height.coerceAtLeast(1)
    val band = (h * 0.18f).toInt().coerceIn(1, h)
    val pixels = IntArray(w * band)
    getPixels(pixels, 0, w, 0, h - band, w, band)
    var red = 0L
    var green = 0L
    var blue = 0L
    for (px in pixels) {
        red += (px shr 16) and 0xFF
        green += (px shr 8) and 0xFF
        blue += px and 0xFF
    }
    val n = pixels.size.coerceAtLeast(1)
    return Color((red / n).toInt(), (green / n).toInt(), (blue / n).toInt())
}

private fun List<Color>.distinctMeshColors(): List<Color> {
    val kept = mutableListOf<Color>()
    for (c in this) {
        if (kept.none { it.isCloseTo(c) }) kept += c
    }
    if (kept.isEmpty()) return DefaultFallbackColors
    if (kept.size >= 4) return kept.take(4)
    val out = kept.toMutableList()
    var step = 1
    while (out.size < 4) {
        out += kept[(out.size - kept.size) % kept.size].shiftedHsl(24f * step, 0.12f * step)
        step++
    }
    return out
}

private fun Color.isCloseTo(other: Color): Boolean {
    val a = FloatArray(3).also { ColorUtils.colorToHSL(toArgb(), it) }
    val b = FloatArray(3).also { ColorUtils.colorToHSL(other.toArgb(), it) }
    val hueGap = abs(a[0] - b[0]).let { min(it, 360f - it) }
    return hueGap < 15f && abs(a[2] - b[2]) < 0.12f
}

private fun Color.shiftedHsl(hue: Float, lightness: Float): Color {
    val hsl = FloatArray(3).also { ColorUtils.colorToHSL(toArgb(), it) }
    hsl[0] = (hsl[0] + hue) % 360f
    hsl[2] = (hsl[2] + lightness).coerceIn(0.2f, 0.7f)
    return Color(ColorUtils.HSLToColor(hsl))
}

private fun adaptedChromaticSaturation(source: Float, minimum: Float, maximum: Float): Float {
    val s = source.coerceIn(0f, 1f)
    return if (s < 0.12f) s else s.coerceIn(minimum, maximum)
}

private fun Color.withHsl(
    saturation: (Float) -> Float = { it },
    lightness: (Float) -> Float = { it },
): Color {
    val hsl = FloatArray(3).also { ColorUtils.colorToHSL(toArgb(), it) }
    hsl[1] = saturation(hsl[1]).coerceIn(0f, 1f)
    hsl[2] = lightness(hsl[2]).coerceIn(0f, 1f)
    return Color(ColorUtils.HSLToColor(hsl))
}

private fun ArtworkSeed.toPalette(dark: Boolean): ArtworkPalette = if (dark) {
    ArtworkPalette(
        background = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.20f, 0.62f) },
            lightness = { 0.13f },
        ),
        wash = edge.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.18f, 0.58f) },
            lightness = { it.coerceIn(0.14f, 0.24f) },
        ),
        elevated = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.20f, 0.62f) },
            lightness = { 0.22f },
        ),
        accent = vibrant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.55f, 1f) },
            lightness = { it.coerceIn(0.62f, 0.78f) },
        ),
        onBackground = Color.White,
        onBackgroundVariant = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.22f, 0.50f) },
            lightness = { 0.80f },
        ),
        divider = Color.White.copy(alpha = 0.12f),
        meshColors = meshColors,
    )
} else {
    ArtworkPalette(
        background = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.14f, 0.50f) },
            lightness = { 0.91f },
        ),
        wash = edge.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.12f, 0.46f) },
            lightness = { it.coerceIn(0.78f, 0.90f) },
        ),
        elevated = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.14f, 0.50f) },
            lightness = { 0.83f },
        ),
        accent = vibrant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.55f, 1f) },
            lightness = { it.coerceIn(0.30f, 0.44f) },
        ),
        onBackground = Color.Black,
        onBackgroundVariant = dominant.withHsl(
            saturation = { adaptedChromaticSaturation(it, 0.22f, 0.55f) },
            lightness = { 0.34f },
        ),
        divider = Color.Black.copy(alpha = 0.10f),
        meshColors = meshColors,
    )
}

@Immutable
data class MeshPalette(val colors: List<Color>)

private val MeshDriftRadians = (PI * 0.45).toFloat()
private val MeshAnchors = arrayOf(
    Offset(0.20f, 0.25f),
    Offset(0.80f, 0.20f),
    Offset(0.75f, 0.80f),
    Offset(0.25f, 0.75f),
)
private val MeshSpeeds = floatArrayOf(1f, -0.7f, 0.85f, -1.15f)
private val MeshSmoothAlphaSteps = FloatArray(FADE_CURVE_STOPS + 1) { i ->
    0.85f * (1f - smootherStep(i / FADE_CURVE_STOPS.toFloat()))
}
private val MeshReadabilityScrim = Brush.verticalGradient(
    colors = listOf(
        Color.Black.copy(alpha = 0.10f),
        Color.Black.copy(alpha = 0.38f),
    ),
)

private fun Color.meshTuned(): Color {
    val hsl = FloatArray(3).also { ColorUtils.colorToHSL(toArgb(), it) }
    hsl[1] = (hsl[1] * 1.35f).coerceAtMost(1f)
    hsl[2] = hsl[2].coerceIn(0.28f, 0.58f)
    return Color(ColorUtils.HSLToColor(hsl))
}

private fun Color.meshDimmed(): Color {
    val hsl = FloatArray(3).also { ColorUtils.colorToHSL(toArgb(), it) }
    hsl[2] = 0.12f
    return Color(ColorUtils.HSLToColor(hsl))
}

@Composable
fun MeshGradientBackground(
    palette: MeshPalette,
    modifier: Modifier = Modifier,
    paused: Boolean = false,
    trackKey: Any? = null,
    blurRadius: Dp = 64.dp,
    continuous: Boolean = false,
    animated: Boolean = true,
    colorFadeMillis: Int = 1400,
) {
    val reduceAnimation = LocalReduceAnimation.current
    val tuned = remember(palette) {
        (palette.colors.ifEmpty { DefaultFallbackColors } + DefaultFallbackColors)
            .take(4)
            .map { it.meshTuned() }
    }
    val dimmedBase = remember(tuned) { tuned.first().meshDimmed() }

    val c0State: State<Color>
    val c1State: State<Color>
    val c2State: State<Color>
    val c3State: State<Color>
    val baseColorState: State<Color>
    if (reduceAnimation || !animated) {
        c0State = rememberUpdatedState(tuned[0])
        c1State = rememberUpdatedState(tuned[1])
        c2State = rememberUpdatedState(tuned[2])
        c3State = rememberUpdatedState(tuned[3])
        baseColorState = rememberUpdatedState(dimmedBase)
    } else {
        val colorSpec: AnimationSpec<Color> = tween(colorFadeMillis)
        c0State = animateColorAsState(tuned[0], colorSpec, label = "meshColor0")
        c1State = animateColorAsState(tuned[1], colorSpec, label = "meshColor1")
        c2State = animateColorAsState(tuned[2], colorSpec, label = "meshColor2")
        c3State = animateColorAsState(tuned[3], colorSpec, label = "meshColor3")
        baseColorState = animateColorAsState(dimmedBase, colorSpec, label = "meshBase")
    }

    val phase = remember { Animatable(0f) }
    LaunchedEffect(trackKey, reduceAnimation, continuous, animated, paused) {
        when {
            !animated || reduceAnimation -> phase.snapTo(0f)
            paused -> Unit
            continuous -> while (isActive) {
                phase.animateTo(
                    targetValue = phase.value + (2 * PI).toFloat(),
                    animationSpec = tween(32_000, easing = LinearEasing),
                )
            }
            else -> phase.animateTo(
                targetValue = phase.value + MeshDriftRadians,
                animationSpec = tween(8_000, easing = FastOutSlowInEasing),
            )
        }
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .graphicsLayer {
                scaleX = 1.3f
                scaleY = 1.3f
            }
            .drawWithCache {
                var cachedRadius = -1f
                val cachedColors = Array(4) { Color.Unspecified }
                val cachedBrushes = arrayOfNulls<Brush>(4)
                onDrawBehind {
                    drawRect(color = baseColorState.value)
                    val radius = (size.maxDimension * 0.62f).coerceAtLeast(1f)
                    val radiusChanged = cachedRadius != radius
                    if (radiusChanged) {
                        cachedRadius = radius
                    }
                    val c0 = c0State.value
                    val c1 = c1State.value
                    val c2 = c2State.value
                    val c3 = c3State.value
                    val drift = phase.value
                    for (index in 0 until 4) {
                        val col = when (index) {
                            0 -> c0
                            1 -> c1
                            2 -> c2
                            else -> c3
                        }
                        var brush = cachedBrushes[index]
                        if (brush == null || radiusChanged || cachedColors[index] != col) {
                            cachedColors[index] = col
                            val stops = Array(FADE_CURVE_STOPS + 1) { i ->
                                (i / FADE_CURVE_STOPS.toFloat()) to col.copy(alpha = MeshSmoothAlphaSteps[i])
                            }
                            brush = Brush.radialGradient(
                                colorStops = stops,
                                center = Offset.Zero,
                                radius = radius,
                            )
                            cachedBrushes[index] = brush
                        }
                        val anchor = MeshAnchors[index]
                        val speed = MeshSpeeds[index]
                        val cx = (anchor.x + 0.16f * cos(drift * speed + index * 1.7f)) * size.width
                        val cy = (anchor.y + 0.16f * sin(drift * speed * 0.9f + index * 2.3f)) * size.height
                        translate(left = cx, top = cy) {
                            drawCircle(
                                brush = brush,
                                radius = radius,
                                center = Offset.Zero,
                            )
                        }
                    }
                    drawRect(brush = MeshReadabilityScrim)
                }
            },
    )
}


private val FullArtworkBlurCache = LruCache<String, ImageBitmap>(8)
private val FullArtworkBackdropFallback = Color(0xFF121212)
private val FullArtworkScrim = Brush.verticalGradient(
    0f to Color.Black.copy(alpha = 0.34f),
    0.55f to Color.Black.copy(alpha = 0.48f),
    1f to Color.Black.copy(alpha = 0.64f),
)

@Composable
fun FullArtworkBlurBackdrop(
    artworkUrl: String?,
    modifier: Modifier = Modifier,
) {
    val context = LocalPlatformContext.current
    var blurred by remember(artworkUrl) { mutableStateOf(artworkUrl?.let(FullArtworkBlurCache::get)) }

    LaunchedEffect(artworkUrl) {
        artworkUrl?.let(FullArtworkBlurCache::get)?.let {
            blurred = it
            return@LaunchedEffect
        }
        if (artworkUrl.isNullOrBlank()) return@LaunchedEffect
        val request = ImageRequest.Builder(context)
            .data(artworkUrl.artworkAt(ROW_ART_PX))
            .size(64)
            .precision(Precision.INEXACT)
            .allowHardware(false)
            .crossfade(false)
            .build()
        val result = SingletonImageLoader.get(context).execute(request)
        val bitmap = (result as? SuccessResult)?.image?.toBitmap() ?: return@LaunchedEffect
        val ready = withContext(Dispatchers.Default) { blurBitmapBox(bitmap, passes = 3).asImageBitmap() }
        FullArtworkBlurCache.put(artworkUrl, ready)
        blurred = ready
    }

    FullArtworkBlurBackdrop(image = blurred, modifier = modifier)
}

@Composable
fun FullArtworkBlurBackdrop(
    image: ImageBitmap?,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(FullArtworkBackdropFallback),
    ) {
        Crossfade(
            targetState = image,
            animationSpec = tween(320, easing = FastOutSlowInEasing),
            label = "fullArtworkBlurBackdrop",
        ) { bitmap ->
            if (bitmap != null) {
                val painter = remember(bitmap) { BitmapPainter(bitmap) }
                Image(
                    painter = painter,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        Canvas(Modifier.fillMaxSize()) { drawRect(brush = FullArtworkScrim) }
    }
}

private fun blurBitmapBox(bitmap: Bitmap, passes: Int): Bitmap {
    val w = bitmap.width
    val h = bitmap.height
    if (w < 2 || h < 2) return bitmap
    val source = IntArray(w * h)
    bitmap.getPixels(source, 0, w, 0, 0, w, h)
    val target = IntArray(source.size)
    val radius = (minOf(w, h) / 12).coerceAtLeast(2)
    repeat(passes) {
        boxBlurArray(source, target, w, h, radius, horizontal = true)
        boxBlurArray(target, source, w, h, radius, horizontal = false)
    }
    return Bitmap.createBitmap(source, w, h, Bitmap.Config.ARGB_8888)
}

private fun boxBlurArray(
    source: IntArray,
    target: IntArray,
    width: Int,
    height: Int,
    radius: Int,
    horizontal: Boolean,
) {
    val major = if (horizontal) width else height
    val minor = if (horizontal) height else width
    val window = radius * 2 + 1
    repeat(minor) { fixed ->
        var red = 0
        var green = 0
        var blue = 0
        fun pixel(at: Int): Int {
            val pos = at.coerceIn(0, major - 1)
            return if (horizontal) source[fixed * width + pos] else source[pos * width + fixed]
        }
        for (offset in -radius..radius) {
            val c = pixel(offset)
            red += (c shr 16) and 0xFF
            green += (c shr 8) and 0xFF
            blue += c and 0xFF
        }
        repeat(major) { moving ->
            val idx = if (horizontal) fixed * width + moving else moving * width + fixed
            target[idx] = (0xFF shl 24) or
                (((red + window / 2) / window) shl 16) or
                (((green + window / 2) / window) shl 8) or
                ((blue + window / 2) / window)
            val leaving = pixel(moving - radius)
            val entering = pixel(moving + radius + 1)
            red += ((entering shr 16) and 0xFF) - ((leaving shr 16) and 0xFF)
            green += ((entering shr 8) and 0xFF) - ((leaving shr 8) and 0xFF)
            blue += (entering and 0xFF) - (leaving and 0xFF)
        }
    }
}


