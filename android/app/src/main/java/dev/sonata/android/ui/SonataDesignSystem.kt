/*
 * Sonata Android Design System: typography, dynamic artwork palette & mesh
 * gradients, custom vector icons, shimmer skeletons, station/mood/release
 * editorial artwork cards, and shared player/navigation chrome.
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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsIgnoringVisibility
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
import androidx.compose.ui.geometry.RoundRect
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
import androidx.compose.ui.graphics.drawscope.clipPath
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
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
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
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
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
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.resolveSonataStrings
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
// 1. Theme, Colors & Typography
// =============================================================================

/** Monochrome signature accent for Basic theme interactive highlights. */
val SonataAccent = BasicPalette.AccentRed

/** Theme-aware crimson accent for Basic interactive highlights. */
@Composable
fun sonataThemeAccent(): Color = BasicPalette.AccentRed

private fun Typography.withFamily(family: FontFamily) = Typography(
    displayLarge = displayLarge.copy(fontFamily = family),
    displayMedium = displayMedium.copy(fontFamily = family),
    displaySmall = displaySmall.copy(fontFamily = family),
    headlineLarge = headlineLarge.copy(fontFamily = family),
    headlineMedium = headlineMedium.copy(fontFamily = family),
    headlineSmall = headlineSmall.copy(fontFamily = family),
    titleLarge = titleLarge.copy(fontFamily = family),
    titleMedium = titleMedium.copy(fontFamily = family),
    titleSmall = titleSmall.copy(fontFamily = family),
    bodyLarge = bodyLarge.copy(fontFamily = family),
    bodyMedium = bodyMedium.copy(fontFamily = family),
    bodySmall = bodySmall.copy(fontFamily = family),
    labelLarge = labelLarge.copy(fontFamily = family),
    labelMedium = labelMedium.copy(fontFamily = family),
    labelSmall = labelSmall.copy(fontFamily = family),
)

val SonataTypography = Typography(
    displayLarge = TextStyle(fontWeight = FontWeight.W800, fontSize = 34.sp, letterSpacing = (-0.5).sp),
    headlineLarge = TextStyle(fontWeight = FontWeight.W800, fontSize = 30.sp, letterSpacing = (-0.4).sp),
    headlineMedium = TextStyle(fontWeight = FontWeight.W700, fontSize = 22.sp, letterSpacing = (-0.25).sp),
    titleLarge = TextStyle(fontWeight = FontWeight.W700, fontSize = 20.sp, letterSpacing = (-0.2).sp),
    titleMedium = TextStyle(fontWeight = FontWeight.W600, fontSize = 16.sp, letterSpacing = 0.05.sp),
    bodyLarge = TextStyle(fontWeight = FontWeight.W400, fontSize = 16.sp, letterSpacing = 0.2.sp),
    bodyMedium = TextStyle(fontWeight = FontWeight.W400, fontSize = 14.sp, letterSpacing = 0.3.sp),
    labelMedium = TextStyle(fontWeight = FontWeight.W600, fontSize = 12.sp, letterSpacing = 0.25.sp),
    labelSmall = TextStyle(fontWeight = FontWeight.W600, fontSize = 11.sp, letterSpacing = 0.3.sp),
).withFamily(SonataAppFontFamily)

@Composable
fun SonataTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    amoled: Boolean = false,
    designTheme: DesignTheme = DesignTheme.BASIC,
    appLanguage: AppLanguage = AppLanguage.SYSTEM,
    content: @Composable () -> Unit,
) {
    val tokens = lunariaTokens(
        darkTheme = darkTheme,
        amoled = amoled,
        designTheme = designTheme,
    )
    val configuration = androidx.compose.ui.platform.LocalConfiguration.current
    val systemLocale = configuration.locales.get(0) ?: java.util.Locale.getDefault()
    val strings = resolveSonataStrings(appLanguage = appLanguage, systemLocale = systemLocale)
    CompositionLocalProvider(
        LocalLunariaColors provides tokens,
        LocalSonataStrings provides strings,
    ) {
        MaterialTheme(
            colorScheme = lunariaColorScheme(
                darkTheme = darkTheme,
                amoled = amoled,
                designTheme = designTheme,
            ),
            typography = SonataTypography,
            content = content,
        )
    }
}

/** Returns a consistent safe top inset across edge-to-edge status bars, cutouts, and transient bar modes. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun rememberSafeTopInset(): Dp {
    val visibleStatus = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val ignoringVisStatus = WindowInsets.statusBarsIgnoringVisibility.asPaddingValues().calculateTopPadding()
    val cutoutTop = WindowInsets.displayCutout.asPaddingValues().calculateTopPadding()
    return maxOf(visibleStatus, ignoringVisStatus, cutoutTop, 28.dp)
}

/** Updates system bar icon contrast while keeping edge-to-edge status and navigation bars well-spaced. */
@Composable
fun SystemBarIcons(dark: Boolean) {
    val view = LocalView.current
    if (view.isInEditMode) return
    val window = findWindow(view) ?: return
    SideEffect {
        WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = dark
            isAppearanceLightNavigationBars = dark
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }
}

private fun findWindow(view: android.view.View): Window? {
    var ctx: Context? = view.context
    while (ctx is ContextWrapper) {
        if (ctx is Activity) return ctx.window
        ctx = ctx.baseContext
    }
    return null
}

// =============================================================================
// 2. Layout Constants & High-Res Artwork URL Upscaler
// =============================================================================

val PAGE_GUTTER = 16.dp
val BAR_GUTTER = PAGE_GUTTER
val SHELF_CARD_WIDTH = 150.dp
val FLOATING_BAR_MAX_WIDTH = 440.dp

private const val TRACK_COLUMN_FRACTION = 0.88f
private val TRACK_COLUMN_MAX_WIDTH = 400.dp

fun trackColumnWidth(available: Dp): Dp =
    minOf(available * TRACK_COLUMN_FRACTION, TRACK_COLUMN_MAX_WIDTH)

const val ROW_ART_PX = 160
const val CARD_ART_PX = 480
const val HEADER_ART_PX = 720
const val PLAYER_ART_PX = 1200

private val SIZE_HINT = Regex("""w\d+-h\d+""")
private val PARAM_SIZE_HINT = Regex("""=s\d+""")
private val GOOGLE_SIZE_SUFFIX = Regex("""=w\d+-h\d+[^/?]*$""")
private val YT_VIDEO_ID_REGEX = Regex("""i\.ytimg\.com/vi(?:_webp)?/([^/]+)/""")

private fun isGoogleImageHost(url: String): Boolean {
    val host = url.substringAfter("://", "")
        .ifEmpty { url.substringAfter("//", "") }
        .substringBefore('/')
        .substringBefore(':')
        .substringBefore('?')
        .lowercase(Locale.ROOT)
    return host == "ggpht.com" || host.endsWith(".ggpht.com") ||
        host == "googleusercontent.com" || host.endsWith(".googleusercontent.com")
}

/** Upscales Google/YouTube/Apple Music artwork URLs to a canonical square cache size. */
fun String?.artworkAt(px: Int): String? {
    if (this == null) return null
    if ("mzstatic.com/image/thumb/" in this) {
        return substringBeforeLast('/') + "/${px}x${px}bb.jpg"
    }
    if ("i.ytimg.com" in this) {
        val videoId = YT_VIDEO_ID_REGEX.find(this)?.groupValues?.getOrNull(1)
        if (videoId != null) {
            return if (px >= 720) {
                "https://i.ytimg.com/vi/$videoId/maxresdefault.jpg"
            } else {
                "https://i.ytimg.com/vi/$videoId/hqdefault.jpg"
            }
        }
    }
    val sized = replace(SIZE_HINT, "w$px-h$px")
    return if (isGoogleImageHost(this)) {
        sized.replace(PARAM_SIZE_HINT, "=s$px")
    } else {
        sized
    }
}

/** Requests a server-cropped portrait or square artwork variant when supported. */
fun String?.croppedAt(width: Int, height: Int = width): String? {
    if (this == null) return null
    if ("mzstatic.com/image/thumb/" in this) {
        val side = maxOf(width, height)
        return substringBeforeLast('/') + "/${side}x${side}cc.jpg"
    }
    return if (GOOGLE_SIZE_SUFFIX.containsMatchIn(this)) {
        replace(GOOGLE_SIZE_SUFFIX, "=w$width-h$height-p-l90-rj")
    } else {
        artworkAt(maxOf(width, height))
    }
}

/** Deterministic Coil memory cache key for a sized artwork variant. */
fun artworkMemoryCacheKey(url: String?, widthPx: Int, heightPx: Int = widthPx): String? =
    url?.takeIf { it.isNotBlank() }?.let { "${it}_${widthPx}x${heightPx}" }

/**
 * Builds and remembers a hardware-accelerated, explicitly downsampled Coil 3 [ImageRequest]
 * with crossfade disabled so scrolling Lazy lists never allocate full-res bitmaps or fade-jank.
 */
@Composable
fun rememberArtworkRequest(
    url: String?,
    widthPx: Int,
    heightPx: Int = widthPx,
    placeholderKey: String? = null,
): ImageRequest? {
    val context = LocalPlatformContext.current
    return remember(context, url, widthPx, heightPx, placeholderKey) {
        val clean = url?.takeIf { it.isNotBlank() } ?: return@remember null
        ImageRequest.Builder(context)
            .data(clean)
            .size(widthPx, heightPx)
            .precision(Precision.INEXACT)
            .allowHardware(true)
            .crossfade(false)
            .memoryCacheKey(artworkMemoryCacheKey(clean, widthPx, heightPx))
            .apply {
                if (!placeholderKey.isNullOrBlank()) {
                    placeholderMemoryCacheKey(placeholderKey)
                }
            }
            .build()
    }
}

// =============================================================================
// 6. Moods, Releases, Explore Sleeves & Replay Cards
// =============================================================================

enum class TopPickMood(
    val displayName: String,
    val subtitle: String,
    val ytmParams: String,
    val shelfWords: List<String> = emptyList(),
) {
    ENERGY("Energy", "Upbeat tracks to get moving", "ggMPOg1uX2lRZUZiMnNrQnJW"),
    RELAX("Relax", "Unwind and slow down", "ggMPOg1uX1JOQWZFeDByc2Jm", listOf("unwind", "peaceful", "relaxing", "laidback")),
    FEEL_GOOD("Feel Good", "Bright, lifting melodies", "ggMPOg1uXzZQbDB5eThLRTQ3"),
    BLUE("Feeling Blue", "Quiet songs for heavy days", "ggMPOg1uX0JLQ0gySWZKZVY1", listOf("sad", "melanchol", "lonely")),
    FOCUS("Focus", "Steady flow without distraction", "ggMPOg1uX0NvNGNhWThMYWRh"),
    HEARTBREAK("Heartbreak", "Songs for letting go", "ggMPOg1uX0JLQ0gySWZKZVY1", listOf("heartbreak", "breakup", "break-up", "broken")),
    ROMANCE("Romance", "Warm songs for two", "ggMPOg1uX0FzQ2FhZWtUY211"),
}


/**
 * Builds a smoothstep-eased vertical scrim [Brush] whose transparent end carries
 * the target color's RGB channels rather than [Color.Transparent], preventing muddy
 * mid-fade banding and hard seam edges.
 */
fun smoothScrimBrush(
    from: Color,
    to: Color,
    startFraction: Float = 0f,
    endFraction: Float = 1f,
    startY: Float = 0f,
    endY: Float = Float.POSITIVE_INFINITY,
    steps: Int = 24,
): Brush =
    Brush.verticalGradient(
        colorStops =
            Array(steps + 1) { i ->
                val t = i / steps.toFloat()
                val position = startFraction + (endFraction - startFraction) * t
                position to lerp(from, to, t * t * (3f - 2f * t))
            },
        startY = startY,
        endY = endY,
    )

/**
 * Convenience [smoothScrimBrush] ramping [color] from zero alpha to full [color].
 */
fun artworkScrimBrush(
    color: Color,
    startFraction: Float = 0f,
    endFraction: Float = 1f,
    steps: Int = 24,
): Brush =
    smoothScrimBrush(
        from = color.copy(alpha = 0f),
        to = color,
        startFraction = startFraction,
        endFraction = endFraction,
        steps = steps,
    )

@Composable
fun MoodTile(
    mood: TopPickMood,
    onClick: (TopPickMood, String) -> Unit,
    modifier: Modifier = Modifier,
    tileWidth: Dp = 178.dp,
    tileHeight: Dp = 138.dp,
    displayTitle: String = mood.displayName,
) {
    val tokens = LocalLunariaColors.current
    val shape = RoundedCornerShape(18.dp)
    val borderColor = tokens.glassRimTopLeft.copy(alpha = 0.55f)
    Column(
        modifier = modifier
            .width(tileWidth)
            .clickable { onClick(mood, mood.displayName) },
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(tileHeight)
                .clip(shape)
                .border(1.dp, borderColor, shape),
        ) {
            SonataMoodAnimatedArt(
                moodTitle = mood.displayName,
                moodIndex = mood.ordinal,
                modifier = Modifier.fillMaxSize(),
            )
            Box(
                Modifier
                    .matchParentSize()
                    .background(
                        smoothScrimBrush(
                            from = Color.Black.copy(alpha = 0f),
                            to = Color.Black.copy(alpha = 0.42f),
                            startFraction = 0.45f,
                        ),
                    ),
            )
            Box(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 9.dp, end = 9.dp)
                    .size(24.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.16f))
                    .border(0.5.dp, Color.White.copy(alpha = 0.28f), CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_sonata_moon),
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.92f),
                    modifier = Modifier.size(13.dp),
                )
            }
            Text(
                text = displayTitle,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 16.5.sp,
                    fontWeight = FontWeight.Bold,
                    shadow = Shadow(Color.Black.copy(alpha = 0.50f), offset = Offset(0f, 1.5f), blurRadius = 8f),
                ),
                color = Color.White,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .align(Alignment.BottomStart)
                    .padding(start = 12.dp, end = 12.dp, bottom = 10.dp),
            )
        }
    }
}

private data class CollagePalette(val top: Color, val bottom: Color, val eyebrow: Color)

private val BasicCollagePalettes = listOf(
    CollagePalette(Color(0xFF1C1C1E), Color(0xFF2C2C2E), Color(0xFFE5E5EA)),
    CollagePalette(Color(0xFF141416), Color(0xFF262629), Color(0xFFE5E5EA)),
    CollagePalette(Color(0xFF18181B), Color(0xFF303036), Color(0xFFE5E5EA)),
    CollagePalette(Color(0xFF111113), Color(0xFF232327), Color(0xFFE5E5EA)),
)

@Composable
fun ArtistCollageArt(
    images: List<String>,
    title: String,
    subtitle: String = "",
    eyebrow: String = "FEATURING",
    modifier: Modifier = Modifier,
    showBrandLogo: Boolean = true,
    cardWidth: Dp? = null,
    cardHeight: Dp? = null,
) {
    val tokens = LocalLunariaColors.current
    val palette = BasicCollagePalettes[abs(title.hashCode()) % BasicCollagePalettes.size]
    val cardCorner = 14.dp
    val shape = remember(cardCorner) { RoundedCornerShape(cardCorner) }
    val w = cardWidth ?: 244.dp
    val h = cardHeight ?: 330.dp
    val borderColor = tokens.glassRimTopLeft.copy(alpha = 0.65f)
    Box(
        modifier
            .width(w)
            .height(h)
            .clip(shape)
            .background(Brush.verticalGradient(listOf(palette.top, palette.bottom)))
            .border(1.dp, borderColor, shape),
    ) {
        val slots = when (images.size) {
            0 -> emptyList()
            1 -> listOf(Triple(0.5f, 0.31f, 0.60f))
            2 -> listOf(Triple(0.38f, 0.25f, 0.54f), Triple(0.72f, 0.44f, 0.32f))
            else -> listOf(
                Triple(0.36f, 0.22f, 0.54f),
                Triple(0.75f, 0.40f, 0.30f),
                Triple(0.46f, 0.49f, 0.24f),
            )
        }
        slots.zip(images).forEach { (slot, url) ->
            val (cx, cy, d) = slot
            val diameter = w * d
            val targetPx = if (d > 0.5f) 320 else 200
            val sizedUrl = remember(url, targetPx) { url.croppedAt(targetPx) }
            val req = rememberArtworkRequest(sizedUrl, targetPx, targetPx)
            AsyncImage(
                model = req,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .offset(x = w * cx - diameter / 2, y = h * cy - diameter / 2)
                    .size(diameter)
                    .clip(CircleShape)
                    .border(1.dp, Color.White.copy(alpha = 0.28f), CircleShape)
                    .background(Color.Black.copy(alpha = 0.18f)),
            )
        }
        if (showBrandLogo) {
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 14.dp, end = 14.dp)
                    .height(15.dp)
                    .aspectRatio(1f),
            )
        }
        Box(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .height(h * 0.48f)
                .background(
                    smoothScrimBrush(
                        from = palette.bottom.copy(alpha = 0f),
                        to = palette.bottom.copy(alpha = 0.94f),
                    ),
                ),
        )
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(start = 18.dp, end = 18.dp, bottom = 18.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            if (tokens.isMoonflower) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(Color.White.copy(alpha = 0.22f))
                        .padding(horizontal = 9.dp, vertical = 3.dp),
                ) {
                    Text(
                        text = eyebrow,
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = 0.8.sp,
                        ),
                        color = Color.White,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            } else {
                Text(
                    text = eyebrow,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontSize = 11.5.sp,
                        lineHeight = 15.sp,
                        fontWeight = FontWeight.Bold,
                        letterSpacing = 0.8.sp,
                    ),
                    color = palette.eyebrow,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 17.5.sp,
                    lineHeight = 22.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp, lineHeight = 16.sp),
                    color = Color.White.copy(alpha = 0.86f),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private val ReleaseCardShape = RoundedCornerShape(12.dp)
private val ReleaseBottomScrim = smoothScrimBrush(
    from = Color.Black.copy(alpha = 0f),
    to = Color.Black.copy(alpha = 0.96f),
    startFraction = 0.42f,
)

@Composable
fun ReleaseArt(
    artworkUrl: String?,
    title: String,
    subtitle: String = "",
    eyebrow: String = dev.sonata.android.ui.i18n.LocalSonataStrings.current.newReleaseEyebrow,
    modifier: Modifier = Modifier,
    showBrandLogo: Boolean = false,
    canvasMedia: CanvasMedia? = null,
    canvasActive: Boolean = false,
    cardWidth: Dp? = null,
    cardHeight: Dp? = null,
) {
    val tokens = LocalLunariaColors.current
    val cardCorner = if (tokens.isMoonflower) 26.dp else 14.dp
    val cardShape = remember(cardCorner) { RoundedCornerShape(cardCorner) }
    val resolvedWidth = cardWidth ?: if (tokens.isMoonflower) 282.dp else 244.dp
    val resolvedHeight = cardHeight ?: if (tokens.isMoonflower) 356.dp else 330.dp
    val borderColor = if (tokens.isMoonflower) {
        Color.White.copy(alpha = if (tokens.isDark) 0.34f else 0.78f)
    } else {
        tokens.glassRimTopLeft.copy(alpha = 0.65f)
    }
    val scrimColor = tokens.heroCardScrimColor
    val scrimBrush = remember(scrimColor, tokens.isMoonflower) {
        if (tokens.isMoonflower) {
            smoothScrimBrush(
                from = scrimColor.copy(alpha = 0f),
                to = scrimColor.copy(alpha = 0.94f),
                startFraction = 0.42f,
            )
        } else {
            smoothScrimBrush(
                from = scrimColor.copy(alpha = 0f),
                to = scrimColor.copy(alpha = 0.96f),
                startFraction = 0.40f,
            )
        }
    }
    val coverUrl = remember(artworkUrl) { artworkUrl.croppedAt(CARD_ART_PX) }
    val coverRequest = rememberArtworkRequest(coverUrl, CARD_ART_PX, CARD_ART_PX)
    Box(
        modifier
            .width(resolvedWidth)
            .height(resolvedHeight)
            .then(
                if (tokens.isViolet) {
                    Modifier.shadow(
                        elevation = 12.dp,
                        shape = cardShape,
                        clip = false,
                        ambientColor = tokens.glassShadowColor,
                        spotColor = tokens.glassShadowColor,
                    )
                } else {
                    Modifier
                },
            )
            .clip(cardShape)
            .background(tokens.eclipseCard)
            .border(if (tokens.isMoonflower) 1.25.dp else 1.dp, borderColor, cardShape),
    ) {
        AsyncImage(
            model = coverRequest,
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier.fillMaxSize(),
        )
        if (canvasMedia != null) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f),
            ) {
                CardCanvasSurface(
                    media = canvasMedia,
                    isVisible = canvasActive,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        Box(
            Modifier
                .fillMaxSize()
                .background(scrimBrush),
        )
        if (showBrandLogo) {
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 14.dp, end = 14.dp)
                    .height(15.dp)
                    .aspectRatio(1f),
            )
        }
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(start = 18.dp, end = 18.dp, bottom = 18.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            if (tokens.isMoonflower) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(Color.White.copy(alpha = 0.22f))
                        .padding(horizontal = 9.dp, vertical = 3.dp),
                ) {
                    Text(
                        text = eyebrow,
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = 0.8.sp,
                        ),
                        color = Color.White,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            } else {
                Text(
                    text = eyebrow,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontSize = 11.5.sp,
                        lineHeight = 15.sp,
                        fontWeight = FontWeight.Bold,
                        letterSpacing = 0.8.sp,
                    ),
                    color = Color.White.copy(alpha = 0.72f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 17.5.sp,
                    lineHeight = 22.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp, lineHeight = 16.sp),
                    color = Color.White.copy(alpha = 0.86f),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}


@Composable
fun MoodGenreSleeve(
    title: String,
    thumbnailUrl: String? = null,
    stripeColor: Long? = null,
    accentColor: Color? = null,
    modifier: Modifier = Modifier,
    titleStyle: TextStyle = MaterialTheme.typography.titleMedium,
) {
    val tokens = LocalLunariaColors.current
    val shape = RoundedCornerShape(18.dp)
    val resolvedAccent = accentColor ?: stripeColor?.let { Color(it.toInt()) }
    val borderColor = when {
        resolvedAccent != null -> resolvedAccent.copy(alpha = 0.55f)
        tokens.isViolet -> tokens.petalLavender.copy(alpha = 0.28f)
        else -> tokens.glassRimTopLeft.copy(alpha = 0.45f)
    }
    Box(
        modifier
            .clip(shape)
            .border(1.dp, borderColor, shape),
    ) {
        SonataMoodAnimatedArt(
            moodTitle = title,
            moodIndex = (title.hashCode() and 0x7FFF_FFFF),
            modifier = Modifier.fillMaxSize(),
        )
        if (resolvedAccent != null) {
            Box(
                Modifier
                    .matchParentSize()
                    .background(
                        smoothScrimBrush(
                            from = resolvedAccent.copy(alpha = 0.28f),
                            to = resolvedAccent.copy(alpha = 0f),
                            endFraction = 0.55f,
                        ),
                    ),
            )
        }
        Box(
            Modifier
                .matchParentSize()
                .background(
                    smoothScrimBrush(
                        from = Color.Black.copy(alpha = 0f),
                        to = Color.Black.copy(alpha = 0.44f),
                        startFraction = 0.4f,
                    ),
                ),
        )
        Box(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = 9.dp, end = 9.dp)
                .size(24.dp)
                .clip(CircleShape)
                .background(Color.White.copy(alpha = 0.16f))
                .border(0.5.dp, Color.White.copy(alpha = 0.28f), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White.copy(alpha = 0.92f),
                modifier = Modifier.size(13.dp),
            )
        }
        Text(
            text = title,
            style = titleStyle.copy(
                fontSize = 16.5.sp,
                shadow = Shadow(Color.Black.copy(alpha = 0.50f), offset = Offset(0f, 1.5f), blurRadius = 8f),
            ),
            fontWeight = FontWeight.Bold,
            color = Color.White,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .align(Alignment.BottomStart)
                .padding(start = 13.dp, end = 13.dp, bottom = 10.dp),
        )
    }
}


private val ReplayPolishedInk = Brush.verticalGradient(
    listOf(Color(0xFFFFFFFF), Color(0xFFF3F4F8), Color(0xFFC9CCD6)),
)
private val ReplayEmbossShadow = Shadow(
    color = Color(0x99000000),
    offset = Offset(0f, 2.5f),
    blurRadius = 4f,
)

@Composable
fun ReplayCreditCard(
    label: String,
    value: String,
    detail: String? = null,
    artworkUrl: String? = null,
    holder: String = dev.sonata.android.ui.i18n.LocalSonataStrings.current.sonataListenerPlaceholder,
    memberSince: String? = "09/26",
    cardIndex: Int = 0,
    onClick: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
    val cardCorner = 16.dp
    val cardShape = remember(cardCorner) { RoundedCornerShape(cardCorner) }
    val scrimTint = Color(0xFF000000)
    val borderColor = Color.White.copy(alpha = if (tokens.isDark) 0.16f else 0.24f)
    Box(
        modifier = modifier
            .aspectRatio(1.586f)
            .shadow(16.dp, cardShape, clip = false)
            .clip(cardShape)
            .border(1.dp, borderColor, cardShape)
            .clickable(onClick = onClick),
    ) {
        val basicGradient = if (cardIndex % 2 == 0) {
            Brush.linearGradient(listOf(Color(0xFF242428), Color(0xFF0D0D0F)))
        } else {
            Brush.linearGradient(listOf(Color(0xFF1C1C20), Color(0xFF08080A)))
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(basicGradient),
        )
        Box(
            Modifier
                .fillMaxSize()
                .background(
                    Brush.radialGradient(
                        colors = listOf(Color.White.copy(alpha = 0.14f), Color.White.copy(alpha = 0f)),
                        radius = 650f,
                    ),
                )
                .background(
                    Brush.verticalGradient(
                        0.0f to scrimTint.copy(alpha = 0.12f),
                        0.55f to scrimTint.copy(alpha = 0.22f),
                        1.0f to scrimTint.copy(alpha = 0.48f),
                    ),
                ),
        )
        Column(
            Modifier
                .fillMaxSize()
                .padding(horizontal = 18.dp, vertical = 16.dp),
        ) {
            Row(verticalAlignment = Alignment.Top) {
                Text(
                    text = strings.yourListeningExperienceEyebrow,
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.W700,
                    letterSpacing = 1.4.sp,
                    lineHeight = 13.sp,
                    color = Color.White.copy(alpha = 0.72f),
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(10.dp))
                Icon(
                    painter = painterResource(R.drawable.ic_sonata_moon),
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier.size(width = 34.dp, height = 22.dp),
                )
            }
            Spacer(Modifier.weight(1f))
            Text(
                text = value,
                style = MaterialTheme.typography.headlineMedium.copy(
                    fontSize = 27.sp,
                    lineHeight = 30.sp,
                    fontWeight = FontWeight.W800,
                    brush = ReplayPolishedInk,
                    shadow = ReplayEmbossShadow,
                ),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = label.uppercase(Locale.ROOT),
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.W700,
                letterSpacing = 1.8.sp,
                color = Color.White.copy(alpha = 0.65f),
            )
            Spacer(Modifier.weight(0.85f))
            Row(verticalAlignment = Alignment.Bottom) {
                Column(Modifier.weight(1f)) {
                    EmbossedCardLine(
                        text = holder.ifBlank { strings.sonataListenerPlaceholder }.uppercase(Locale.getDefault()),
                        size = 13.sp,
                    )
                    if (!detail.isNullOrBlank()) {
                        Text(
                            text = detail,
                            style = MaterialTheme.typography.labelSmall,
                            color = Color.White.copy(alpha = 0.6f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (!memberSince.isNullOrBlank()) {
                    Spacer(Modifier.width(10.dp))
                    Column(horizontalAlignment = Alignment.End) {
                        Text(
                            text = strings.memberSinceEyebrow,
                            style = MaterialTheme.typography.labelSmall,
                            fontSize = 7.sp,
                            lineHeight = 8.sp,
                            letterSpacing = 0.8.sp,
                            color = Color.White.copy(alpha = 0.55f),
                            textAlign = TextAlign.End,
                        )
                        EmbossedCardLine(text = memberSince, size = 12.sp)
                    }
                }
            }
        }
    }
}

@Composable
private fun EmbossedCardLine(text: String, size: TextUnit) {
    Text(
        text = text,
        style = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.W600,
            fontSize = size,
            letterSpacing = 1.6.sp,
            brush = ReplayPolishedInk,
            shadow = ReplayEmbossShadow,
        ),
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

// =============================================================================
// 7. Shared Player & Navigation Chrome
// =============================================================================

@Composable
fun ThinSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    onValueChangeFinished: (() -> Unit)? = null,
    loading: Boolean = false,
    idleHeight: Dp = 7.dp,
    activeHeight: Dp = 12.dp,
    activeColor: Color = Color.White.copy(alpha = 0.92f),
    inactiveColor: Color = Color.White.copy(alpha = 0.26f),
) {
    var dragging by remember { mutableStateOf(false) }
    val latestOnValueChange = rememberUpdatedState(onValueChange)
    val latestOnValueChangeFinished = rememberUpdatedState(onValueChangeFinished)
    val height by animateDpAsState(
        targetValue = if (dragging) activeHeight else idleHeight,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessMediumLow,
        ),
        label = "thinSliderHeight",
    )
    val sweepState: State<Float>? = if (loading) {
        rememberInfiniteTransition(label = "thinSliderSheen").animateFloat(
            initialValue = 0f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(1100, easing = LinearEasing)),
            label = "sheen",
        )
    } else {
        null
    }

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(activeHeight + 22.dp)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    if (size.width <= 0) return@awaitEachGesture
                    dragging = true
                    try {
                        latestOnValueChange.value((down.position.x / size.width).coerceIn(0f, 1f))
                        while (true) {
                            val event = awaitPointerEvent()
                            val pointer = event.changes.firstOrNull { it.id == down.id } ?: break
                            if (!pointer.pressed) {
                                pointer.consume()
                                break
                            }
                            if (pointer.positionChanged()) {
                                latestOnValueChange.value((pointer.position.x / size.width).coerceIn(0f, 1f))
                                pointer.consume()
                            }
                        }
                    } finally {
                        if (dragging) {
                            dragging = false
                            latestOnValueChangeFinished.value?.invoke()
                        }
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        val sheenColors = remember(activeColor) {
            listOf(
                activeColor.copy(alpha = 0f),
                activeColor.copy(alpha = 0.65f),
                activeColor.copy(alpha = 0f),
            )
        }
        Spacer(
            Modifier
                .fillMaxWidth()
                .height(height)
                .drawWithCache {
                    val radius = CornerRadius(size.height / 2f)
                    val band = (size.width * 0.34f).coerceAtLeast(1f)
                    val sheenBrush = Brush.horizontalGradient(
                        colors = sheenColors,
                        startX = 0f,
                        endX = band,
                    )
                    val pillPath = Path().apply {
                        addRoundRect(RoundRect(0f, 0f, size.width, size.height, radius))
                    }
                    onDrawBehind {
                        drawRoundRect(color = inactiveColor, cornerRadius = radius)
                        if (loading) {
                            val sweep = sweepState?.value ?: 0f
                            val startX = -band + sweep * (size.width + band * 2f)
                            clipPath(pillPath) {
                                translate(left = startX) {
                                    drawRect(
                                        brush = sheenBrush,
                                        size = Size(band, size.height),
                                    )
                                }
                            }
                        } else {
                            val fraction = value.coerceIn(0f, 1f)
                            val filled = (size.width * fraction).coerceAtLeast(size.height).coerceAtMost(size.width)
                            if (fraction > 0.001f) {
                                drawRoundRect(
                                    color = activeColor,
                                    size = Size(filled, size.height),
                                    cornerRadius = radius,
                                )
                            }
                        }
                    }
                },
        )
    }
}

@Composable
fun ShimmerText(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = MaterialTheme.typography.labelMedium.copy(
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
    ),
    baseAlpha: Float = 0.50f,
) {
    val reduceAnimation = LocalReduceAnimation.current
    val baseColor = remember(baseAlpha) { Color.White.copy(alpha = baseAlpha) }
    val glowStyle = remember(style) {
        style.copy(
            shadow = Shadow(
                color = Color.White.copy(alpha = 0.85f),
                offset = Offset.Zero,
                blurRadius = 12f,
            ),
        )
    }
    if (reduceAnimation) {
        Text(
            text = text,
            style = style.copy(shadow = null),
            color = baseColor,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = modifier.padding(horizontal = 4.dp, vertical = 2.dp),
        )
        return
    }

    // Left-to-right pure white glowing spotlight gliding smoothly over steady muted gray text
    val phase = rememberInfiniteTransition(label = "lossless-shimmer").animateFloat(
        initialValue = -0.35f,
        targetValue = 1.35f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 2_800, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "lossless-shimmer-phase",
    )
    val spotlightStops = remember {
        arrayOf(
            0.0f to Color.White.copy(alpha = 0f),
            0.25f to Color.White.copy(alpha = 0.25f),
            0.50f to Color.White,
            0.75f to Color.White.copy(alpha = 0.25f),
            1.0f to Color.White.copy(alpha = 0f),
        )
    }

    Box(modifier = modifier) {
        // Layer 1: Steady muted gray base text (never changes color or brightness)
        Text(
            text = text,
            style = style.copy(shadow = null),
            color = baseColor,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp),
        )

        // Layer 2: Soft glowing white text clipped by a smooth Gaussian-like left-to-right mask
        Text(
            text = text,
            style = glowStyle,
            color = Color.White,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .graphicsLayer(compositingStrategy = CompositingStrategy.Offscreen)
                .drawWithContent {
                    drawContent()
                    val bandWidth = (size.width * 0.42f).coerceAtLeast(24f)
                    val centerX = phase.value * size.width
                    val bandLeft = centerX - bandWidth * 0.5f
                    val bandRight = centerX + bandWidth * 0.5f
                    drawRect(
                        brush = Brush.horizontalGradient(
                            colorStops = spotlightStops,
                            startX = bandLeft,
                            endX = bandRight,
                        ),
                        blendMode = BlendMode.DstIn,
                    )
                }
                .padding(horizontal = 4.dp, vertical = 2.dp),
        )
    }
}

private val LosslessPillShape = SquirclePillShape()

@Composable
fun LosslessLabel(
    label: String,
    @Suppress("UNUSED_PARAMETER") pulsing: Boolean = false,
    onClick: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .lightweightLiquidGlass(LosslessPillShape, fallbackColor = Color.White.copy(alpha = 0.14f))
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(start = 10.dp, end = 6.dp, top = 2.dp, bottom = 2.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_lossless),
            contentDescription = null,
            tint = Color.White.copy(alpha = 0.56f),
            modifier = Modifier.height(12.dp),
        )
        Spacer(Modifier.width(1.dp))
        ShimmerText(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(
                fontWeight = FontWeight.Medium,
            ),
            baseAlpha = 0.50f,
        )
    }
}

@Composable
fun PillSegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    if (options.isEmpty()) return
    val haptics = LocalHapticFeedback.current
    val density = LocalDensity.current
    val style = MaterialTheme.typography.labelMedium
    val measurer = rememberTextMeasurer()
    val tokens = LocalLunariaColors.current
    val pillColor = if (tokens.isDark) tokens.eclipseCard else tokens.settingsCard
    val labelColor = tokens.onSurfacePrimary
    val reduceAnimation = LocalReduceAnimation.current
    val selected = selectedIndex.takeIf { it in options.indices }
    val trackShape = tokens.tabPillShape
    val containerColor = if (tokens.isDark) tokens.eclipseSurface else tokens.eclipseCard
    val fallbackViewportPx = with(density) {
        (LocalConfiguration.current.screenWidthDp.dp - PAGE_GUTTER * 2).toPx().coerceAtLeast(1f)
    }
    var measuredWidthPx by remember { mutableIntStateOf(0) }
    val viewport = if (measuredWidthPx > 0) measuredWidthPx.toFloat() else fallbackViewportPx

    Box(
        modifier = modifier
            .fillMaxWidth()
            .onSizeChanged { measuredWidthPx = it.width }
            .clip(trackShape)
            .background(containerColor),
    ) {
        val inset = with(density) { 4.dp.toPx() }
        val labelPadding = with(density) { 18.dp.toPx() }
        val room = (viewport - inset * 2f).coerceAtLeast(1f)
        val natural = remember(options, style, measurer, density) {
            options.map { label ->
                measurer.measure(label, style, maxLines = 1, softWrap = false).size.width + labelPadding * 2f
            }
        }
        val needed = natural.sum()
        val scrolls = needed > room
        val widths = when {
            scrolls -> natural
            natural.max() * options.size <= room -> List(options.size) { room / options.size }
            else -> natural.map { it + (room - needed) / options.size }
        }
        val starts = widths.runningFold(inset) { start, width -> start + width }
        val contentWidth = if (scrolls) needed + inset * 2f else viewport

        val motion: AnimationSpec<Float> = if (reduceAnimation) snap() else spring(0.8f, Spring.StiffnessMediumLow)
        val pillStart by animateFloatAsState(selected?.let(starts::get) ?: 0f, motion, label = "segStart")
        val pillWidth by animateFloatAsState(selected?.let(widths::get) ?: 0f, motion, label = "segWidth")

        val scroll = rememberScrollState()
        val placed = remember { booleanArrayOf(false) }
        LaunchedEffect(selected, natural, viewport) {
            if (!scrolls || selected == null) return@LaunchedEffect
            val max = snapshotFlow { scroll.maxValue }.first { it > 0 }
            val centred = (starts[selected] + widths[selected] / 2f - viewport / 2f).roundToInt().coerceIn(0, max)
            if (placed[0] && !reduceAnimation) scroll.animateScrollTo(centred) else scroll.scrollTo(centred)
            placed[0] = true
        }

        Box(
            Modifier
                .fillMaxWidth()
                .then(
                    if (scrolls) {
                        val fadePx = with(density) { 28.dp.toPx() }
                        Modifier
                            .fadingEdges(
                                28.dp,
                                start = { scroll.value / fadePx },
                                end = { (scroll.maxValue - scroll.value) / fadePx },
                            )
                            .horizontalScroll(scroll)
                    } else {
                        Modifier
                    },
                ),
        ) {
            Box(
                Modifier
                    .width(with(density) { contentWidth.toDp() })
                    .padding(vertical = 4.dp),
            ) {
                if (selected != null) {
                    Box(
                        Modifier
                            .matchParentSize()
                            .layout { measurable, constraints ->
                                val width = pillWidth.roundToInt().coerceAtLeast(0)
                                val placeable = measurable.measure(Constraints.fixed(width, constraints.maxHeight))
                                layout(constraints.maxWidth, constraints.maxHeight) {
                                    placeable.place(pillStart.roundToInt(), 0)
                                }
                            }
                            .clip(trackShape)
                            .background(pillColor),
                    )
                }
                options.forEachIndexed { index, label ->
                    Box(
                        modifier = Modifier
                            .layout { measurable, constraints ->
                                val placeable = measurable.measure(constraints)
                                layout(placeable.width, placeable.height) {
                                    placeable.place(IntOffset(starts[index].roundToInt(), 0))
                                }
                            }
                            .width(with(density) { widths[index].toDp() })
                            .clickable(
                                enabled = enabled,
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                            ) {
                                if (index != selected) {
                                    haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                    onSelect(index)
                                }
                            }
                            .padding(vertical = 9.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = label,
                            style = style,
                            color = labelColor,
                            maxLines = 1,
                            softWrap = false,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun BottomFadeScrim(
    modifier: Modifier = Modifier,
    withMiniPlayer: Boolean = false,
    pageColor: Color = MaterialTheme.colorScheme.background,
) {
    val isDark = LocalLunariaColors.current.isDark
    val inset = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val height by animateDpAsState(
        targetValue = inset + if (withMiniPlayer) 118.dp else 76.dp,
        animationSpec = tween(220),
        label = "bottomScrimHeight",
    )
    val maxAlpha = if (isDark) 0.42f else 0.22f
    val brush = remember(pageColor, maxAlpha) {
        Brush.verticalGradient(
            colorStops = Array(12) { i ->
                val t = i / 11f
                t to pageColor.copy(alpha = EaseInCubic.transform(t) * maxAlpha)
            },
        )
    }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(height)
            .background(brush),
    )
}

/**
 * Smooth eased vertical top scrim behind floating top controls.
 * Avoids a second full-screen Haze offscreen capture on every scroll frame.
 */
@Composable
fun TopFadeBlur(
    pageColor: Color = MaterialTheme.colorScheme.background,
    topHeight: Dp = rememberSafeTopInset() + 56.dp,
    modifier: Modifier = Modifier,
) {
    val brush = remember(pageColor) {
        Brush.verticalGradient(
            colorStops = Array(12) { i ->
                val t = i / 11f
                val alpha = (1f - EaseInOut.transform(t)) * 0.88f
                t to pageColor.copy(alpha = alpha)
            },
        )
    }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(topHeight + 20.dp)
            .background(brush),
    )
}

val LocalSearchFieldOnSurface = staticCompositionLocalOf { false }

@Composable
fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    onSubmit: () -> Unit,
    focusRequester: FocusRequester = remember { FocusRequester() },
    placeholder: String = dev.sonata.android.ui.i18n.LocalSonataStrings.current.searchPlaceholder,
    modifier: Modifier = Modifier,
) {
    val searchStrings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
    val focusManager = LocalFocusManager.current
    val keyboardController = LocalSoftwareKeyboardController.current
    val submit = {
        onSubmit()
        focusManager.clearFocus()
    }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(
                if (LocalSearchFieldOnSurface.current) {
                    Modifier.fillMaxHeight()
                } else {
                    Modifier
                        .height(46.dp)
                        .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(11.dp))
                },
            )
            .padding(start = 8.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = SonataIcons.Search,
            contentDescription = searchStrings.tabSearch,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier
                .size(32.dp)
                .clip(CircleShape)
                .clickable(enabled = query.isNotBlank(), onClick = submit)
                .padding(6.dp),
        )
        Spacer(Modifier.width(4.dp))
        Box(Modifier.weight(1f)) {
            if (query.isEmpty()) {
                Text(
                    text = placeholder,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyLarge.copy(
                    color = MaterialTheme.colorScheme.onBackground,
                ),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { submit() }),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
            )
        }
        if (query.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .size(28.dp)
                    .clip(CircleShape)
                    .clickable {
                        onQueryChange("")
                        focusRequester.requestFocus()
                        keyboardController?.show()
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataIcons.Close,
                    contentDescription = searchStrings.clearSearch,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

private val LINE_FADE = 36.dp
private val MARQUEE_GAP = 42.dp
private const val MARQUEE_DP_PER_SEC = 28f
private const val MARQUEE_REST_MS = 1_600L

@Composable
fun MarqueeText(
    text: String,
    style: TextStyle,
    color: Color,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    startDelayMillis: Long = 0L,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val density = LocalDensity.current
    val textMeasurer = rememberTextMeasurer()
    val reduceAnimation = LocalReduceAnimation.current
    Row(modifier = modifier.clipToBounds(), verticalAlignment = Alignment.CenterVertically) {
        if (leading != null) {
            leading()
            Spacer(Modifier.width(6.dp))
        }
        var maxWidthPx by remember { mutableIntStateOf(0) }
        val layout = remember(text, style) {
            textMeasurer.measure(text = text, style = style, maxLines = 1, softWrap = false)
        }
        val overflowing = maxWidthPx > 0 && layout.size.width > maxWidthPx
        val crawls = enabled && !reduceAnimation && overflowing
        val travelPx = if (crawls) {
            layout.size.width + with(density) { MARQUEE_GAP.roundToPx() }
        } else {
            0
        }

        val offsetX = remember { Animatable(0f) }
        LaunchedEffect(text, travelPx, startDelayMillis) {
            offsetX.snapTo(0f)
            if (travelPx <= 0) return@LaunchedEffect
            val pxPerMs = with(density) { MARQUEE_DP_PER_SEC.dp.toPx() } / 1000f
            val scrollMs = (travelPx / pxPerMs).roundToInt().coerceAtLeast(400)
            delay(startDelayMillis)
            while (isActive) {
                offsetX.animateTo(-travelPx.toFloat(), tween(scrollMs, easing = LinearEasing))
                offsetX.snapTo(0f)
                delay(MARQUEE_REST_MS)
            }
        }

        val fadePx = with(density) { LINE_FADE.toPx() }
        Box(
            Modifier
                .weight(1f, fill = false)
                .clipToBounds()
                .onSizeChanged { maxWidthPx = it.width },
        ) {
            Row(
                modifier = Modifier
                    .then(
                        if (overflowing) {
                            Modifier.fadingEdges(
                                LINE_FADE,
                                start = {
                                    val moved = -offsetX.value
                                    minOf(moved, travelPx - moved) / fadePx
                                },
                                end = { 1f },
                            )
                        } else {
                            Modifier
                        },
                    )
                    .wrapContentWidth(align = Alignment.Start, unbounded = true)
                    .graphicsLayer { translationX = offsetX.value },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(text = text, style = style, color = color, maxLines = 1, softWrap = false, overflow = TextOverflow.Clip)
                if (crawls) {
                    Spacer(Modifier.width(MARQUEE_GAP))
                    Text(text = text, style = style, color = color, maxLines = 1, softWrap = false, overflow = TextOverflow.Clip)
                }
            }
        }
        if (trailing != null) {
            Spacer(Modifier.width(6.dp))
            trailing()
        }
    }
}

@Composable
fun CircleGlyph(
    icon: ImageVector,
    contentDescription: String,
    onClick: () -> Unit,
    active: Boolean = false,
    modifier: Modifier = Modifier,
    tint: Color = LocalLunariaColors.current.onSurfacePrimary,
) {
    val tokens = LocalLunariaColors.current
    val haptics = LocalHapticFeedback.current
    val discAlpha by animateFloatAsState(
        targetValue = if (active) 0.26f else 0.12f,
        label = "glyphDisc",
    )
    Box(
        modifier = modifier
            .size(34.dp)
            .clip(CircleShape)
            .background(tokens.onSurfacePrimary.copy(alpha = discAlpha))
            .clickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
            ) {
                haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onClick()
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = contentDescription,
            tint = tint,
            modifier = Modifier.size(18.dp),
        )
    }
}


/**
 * Steered monotonic lyric clock: advances smoothly on frame timestamps and gently steers
 * toward dated ExoPlayer readings rather than snapping or stuttering on polling jitter.
 * Explicit seeks / track changes (`discontinuity`) jump immediately.
 */
internal class SteeredLyricClock(startMs: Long) {
    var displayedMs: Double = startMs.coerceAtLeast(0L).toDouble()
        private set

    private var lastNowMs = Double.NaN
    private var anchorAtMs = Double.NaN
    private var anchorMs = startMs.coerceAtLeast(0L).toDouble()
    private var lastReportedMs = Long.MIN_VALUE
    private var lastSampledAtMs = Double.NaN
    private var heldAtMs = Double.NEGATIVE_INFINITY
    private var lastDiscontinuity: Long? = null

    fun hold(positionMs: Long, nowMs: Double) {
        val clamped = positionMs.coerceAtLeast(0L).toDouble()
        lastNowMs = Double.NaN
        displayedMs = clamped
        anchorMs = clamped
        anchorAtMs = nowMs
        heldAtMs = nowMs
    }

    fun restart() {
        lastNowMs = Double.NaN
    }

    fun frame(
        nowMs: Double,
        reportedMs: Long,
        sampledAtMs: Double,
        discontinuity: Long,
        playbackSpeed: Float = 1f,
    ): Long {
        val safeReported = reportedMs.coerceAtLeast(0L)
        val safeSpeed = if (playbackSpeed.isFinite() && playbackSpeed > 0.1f) playbackSpeed.toDouble() else 1.0
        val firstOfRun = lastNowMs.isNaN()
        val jumped = lastDiscontinuity != null && discontinuity != lastDiscontinuity
        lastDiscontinuity = discontinuity

        val isNewReading = safeReported != lastReportedMs ||
            (!sampledAtMs.isNaN() && sampledAtMs != lastSampledAtMs)
        lastReportedMs = safeReported
        lastSampledAtMs = sampledAtMs

        if (firstOfRun || jumped) {
            lastNowMs = nowMs
            val takenAt = when {
                sampledAtMs.isNaN() || sampledAtMs <= heldAtMs -> nowMs
                else -> minOf(sampledAtMs, nowMs).coerceAtLeast(nowMs - 500.0)
            }
            anchorAtMs = takenAt
            anchorMs = safeReported.toDouble()
            displayedMs = (anchorMs + (nowMs - anchorAtMs) * safeSpeed).coerceAtLeast(0.0)
            return displayedMs.toLong()
        }

        val dt = (nowMs - lastNowMs).coerceIn(0.0, 100.0) * safeSpeed
        lastNowMs = nowMs

        if (isNewReading) {
            val takenAt = if (sampledAtMs.isNaN()) {
                nowMs
            } else {
                minOf(sampledAtMs, nowMs).coerceAtLeast(nowMs - 500.0)
            }
            if (takenAt >= anchorAtMs) {
                anchorAtMs = takenAt
                anchorMs = safeReported.toDouble()
            }
        }

        val target = anchorMs + (nowMs - anchorAtMs).coerceIn(0.0, 500.0) * safeSpeed
        val error = target - (displayedMs + dt)
        displayedMs = when {
            abs(error) > 400.0 -> target
            else -> {
                val closing = error * (dt / (dt + 300.0))
                val correction = closing.coerceIn(-0.45 * dt, 0.30 * dt)
                (displayedMs + dt + correction).coerceAtLeast(0.0)
            }
        }
        return displayedMs.toLong()
    }
}


/**
 * 120fps-capable monotonic playback clock driven by `withFrameNanos` while playing,
 * reading [PlaybackClockState] strictly inside frame callbacks / snapshotFlow so host
 * composables NEVER recompose on playback ticks.
 */
@Composable
fun rememberSmoothLyricClock(
    clock: PlaybackClockState,
    isPlaying: Boolean,
    isBuffering: Boolean,
    fpsLimit: Int = 60,
    trackKey: String = "",
    playbackSpeed: Float = 1f,
): State<Long> {
    val initialPos = remember(trackKey, clock) {
        Snapshot.withoutReadObservation { clock.positionMs.coerceAtLeast(0L) }
    }
    val smoothClock = remember(trackKey) { mutableLongStateOf(initialPos) }
    val engine = remember(trackKey) { SteeredLyricClock(initialPos) }
    val minFrameStepNanos = if (fpsLimit >= 120) 7_000_000L else 15_500_000L

    LaunchedEffect(clock, trackKey, isPlaying, isBuffering, minFrameStepNanos, playbackSpeed) {
        if (!isPlaying || isBuffering) {
            snapshotFlow { clock.positionMs to clock.seekDiscontinuity }
                .collectLatest { (reported, _) ->
                    val clamped = reported.coerceAtLeast(0L)
                    engine.hold(clamped, System.nanoTime() / 1_000_000.0)
                    smoothClock.longValue = clamped
                }
            return@LaunchedEffect
        }
        engine.restart()
        var lastFrameNanos = 0L
        while (isActive) {
            withFrameNanos { frameNanos ->
                if (frameNanos - lastFrameNanos >= minFrameStepNanos) {
                    lastFrameNanos = frameNanos
                    val nowMs = System.nanoTime() / 1_000_000.0
                    val sampledNanos = clock.sampledAtNanos
                    val sampledAtMs = if (sampledNanos > 0L) sampledNanos / 1_000_000.0 else Double.NaN
                    smoothClock.longValue = engine.frame(
                        nowMs = nowMs,
                        reportedMs = clock.positionMs,
                        sampledAtMs = sampledAtMs,
                        discontinuity = clock.seekDiscontinuity,
                        playbackSpeed = playbackSpeed,
                    )
                }
            }
        }
    }

    return smoothClock
}

/**
 * Legacy primitive overload kept for direct callers.
 */
@Composable
fun rememberSmoothLyricClock(
    reportedMs: Long,
    sampledAtNanos: Long,
    isPlaying: Boolean,
    isBuffering: Boolean,
    fpsLimit: Int = 60,
    trackKey: String = "",
    discontinuity: Long = 0L,
): State<Long> {
    val clock = remember(trackKey) { mutableLongStateOf(reportedMs.coerceAtLeast(0L)) }
    val engine = remember(trackKey) { SteeredLyricClock(reportedMs.coerceAtLeast(0L)) }
    val latestReported by rememberUpdatedState(reportedMs.coerceAtLeast(0L))
    val latestSampledNanos by rememberUpdatedState(sampledAtNanos)
    val latestDiscontinuity by rememberUpdatedState(discontinuity)
    val minFrameStepNanos = if (fpsLimit >= 120) 7_000_000L else 15_500_000L

    LaunchedEffect(trackKey, isPlaying, isBuffering, minFrameStepNanos) {
        if (!isPlaying || isBuffering) {
            val nowMs = System.nanoTime() / 1_000_000.0
            engine.hold(latestReported, nowMs)
            clock.longValue = latestReported
            return@LaunchedEffect
        }
        engine.restart()
        var lastFrameNanos = 0L
        while (isActive) {
            withFrameNanos { frameNanos ->
                if (frameNanos - lastFrameNanos >= minFrameStepNanos) {
                    lastFrameNanos = frameNanos
                    val nowMs = System.nanoTime() / 1_000_000.0
                    val sampledNanos = latestSampledNanos
                    val sampledAtMs = if (sampledNanos > 0L) sampledNanos / 1_000_000.0 else Double.NaN
                    clock.longValue = engine.frame(
                        nowMs = nowMs,
                        reportedMs = latestReported,
                        sampledAtMs = sampledAtMs,
                        discontinuity = latestDiscontinuity,
                    )
                }
            }
        }
    }

    LaunchedEffect(trackKey, reportedMs, isPlaying, isBuffering, discontinuity) {
        if (!isPlaying || isBuffering) {
            val clamped = reportedMs.coerceAtLeast(0L)
            engine.hold(clamped, System.nanoTime() / 1_000_000.0)
            clock.longValue = clamped
        }
    }

    return clock
}

private val KaraokeWordRegex = Regex("""\S+""")
private val KaraokeSweepStops = arrayOf(
    0f to Color.Black,
    1f to Color.Black.copy(alpha = 0f),
)

private data class KaraokeWordSpan(
    val startChar: Int,
    val endChar: Int,
    val startMs: Long,
    val endMs: Long,
)

private fun buildKaraokeSpans(
    text: String,
    lineStartMs: Long,
    lineEndMs: Long,
    words: List<AndroidLyricWord>,
): List<KaraokeWordSpan> {
    if (text.isEmpty()) return emptyList()
    val safeLineEnd = maxOf(lineEndMs, lineStartMs + 350L)

    if (words.isNotEmpty()) {
        val spans = ArrayList<KaraokeWordSpan>(words.size)
        var cursor = 0
        words.forEachIndexed { idx, word ->
            val token = word.text.trim()
            val found = if (token.isNotEmpty()) text.indexOf(token, startIndex = cursor) else -1
            val startChar = if (found >= 0) found else cursor.coerceAtMost(text.length)
            val endChar = if (found >= 0) {
                (found + token.length).coerceAtMost(text.length)
            } else {
                (startChar + word.text.length).coerceAtMost(text.length)
            }
            if (endChar > startChar) {
                cursor = endChar
            }
            val startMs = (word.startSecs * 1000.0).roundToInt().toLong().coerceAtLeast(0L)
            val fallbackNext = words.getOrNull(idx + 1)?.startSecs?.let { (it * 1000.0).roundToInt().toLong() }
            val rawEnd = word.endSecs?.let { (it * 1000.0).roundToInt().toLong() } ?: fallbackNext ?: safeLineEnd
            val endMs = maxOf(rawEnd, startMs + 60L)
            spans.add(KaraokeWordSpan(startChar, endChar, startMs, endMs))
        }
        if (spans.isNotEmpty()) return spans
    }

    // Synthesize smooth proportional spans with punctuation cadence weights when word timestamps are absent.
    val matches = KaraokeWordRegex.findAll(text).toList()
    if (matches.isEmpty()) {
        return listOf(KaraokeWordSpan(0, text.length, lineStartMs, safeLineEnd))
    }
    fun wordCadenceWeight(word: String): Float {
        val trimmed = word.trim()
        val base = trimmed.length.coerceAtLeast(1).toFloat()
        return when (trimmed.lastOrNull()) {
            ',', ';', ':' -> base + 0.55f
            '.', '!', '?' -> base + 0.85f
            else -> base
        }
    }
    val totalWeight = matches.sumOf { wordCadenceWeight(it.value).toDouble() }.toFloat().coerceAtLeast(1f)
    val duration = (safeLineEnd - lineStartMs).coerceIn(250L, 6_200L)
    var accWeight = 0f
    return matches.map { m ->
        val w = wordCadenceWeight(m.value)
        val sMs = lineStartMs + ((accWeight / totalWeight) * duration).roundToInt().toLong()
        accWeight += w
        val eMs = lineStartMs + ((accWeight / totalWeight) * duration).roundToInt().toLong()
        KaraokeWordSpan(m.range.first, m.range.last + 1, sMs, maxOf(eMs, sMs + 50L))
    }
}

/** Small perceptual lead aligning swept letter illumination smoothly with vocal onset. */
const val LYRIC_SYNC_LEAD_MS = 35L

/**
 * Per-frame word/character karaoke lyric text rendered via `onTextLayout` +
 * `CompositingStrategy.Offscreen` + feathered `BlendMode.DstIn` sweep without recomposition.
 */
@Composable
fun SweptKaraokeLyricText(
    text: String,
    lineStartMs: Long,
    lineEndMs: Long,
    words: List<AndroidLyricWord>,
    clockMs: State<Long>,
    style: TextStyle,
    color: Color = Color.White,
    dimAlpha: Float = 0.34f,
    uncommittedColor: Color = Color(0xFF98989F).copy(alpha = dimAlpha),
    active: Boolean = true,
    featherDp: Dp = 26.dp,
    modifier: Modifier = Modifier,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val spans = remember(text, lineStartMs, lineEndMs, words) {
        buildKaraokeSpans(text, lineStartMs, lineEndMs, words)
    }
    var textLayout by remember(text, style, maxLines, overflow) { mutableStateOf<TextLayoutResult?>(null) }

    Box(modifier) {
        if (!active) {
            Text(
                text = text,
                style = style,
                color = uncommittedColor,
                softWrap = true,
                maxLines = maxLines,
                overflow = overflow,
                modifier = Modifier.fillMaxWidth(),
            )
            return@Box
        }
        Text(
            text = text,
            style = style,
            color = uncommittedColor,
            softWrap = true,
            maxLines = maxLines,
            overflow = overflow,
            onTextLayout = { textLayout = it },
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            text = text,
            style = style.copy(
                shadow = androidx.compose.ui.graphics.Shadow(
                    color = color.copy(alpha = 0.46f),
                    offset = Offset.Zero,
                    blurRadius = 22f,
                ),
            ),
            color = color,
            softWrap = true,
            maxLines = maxLines,
            overflow = overflow,
            modifier = Modifier
                .fillMaxWidth()
                .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                .drawWithContent {
                    drawContent()
                    val layout = textLayout
                    if (layout == null || spans.isEmpty()) {
                        drawRect(Color.Black, blendMode = BlendMode.DstOut)
                        return@drawWithContent
                    }
                    val nowMs = clockMs.value + LYRIC_SYNC_LEAD_MS
                    val featherPx = featherDp.toPx().coerceAtLeast(4f)
                    val len = text.length

                    for (lineIdx in 0 until layout.lineCount) {
                        val lineTop = layout.getLineTop(lineIdx)
                        val lineBottom = layout.getLineBottom(lineIdx)
                        val clipTop = if (lineIdx == 0) 0f else lineTop
                        val clipBottom = if (lineIdx == layout.lineCount - 1) size.height else lineBottom
                        val clipH = (clipBottom - clipTop).coerceAtLeast(1f)
                        val lineStartChar = layout.getLineStart(lineIdx)
                        val lineEndChar = layout.getLineEnd(lineIdx, visibleEnd = true)
                        val isRtl = layout.getParagraphDirection(lineIdx) == ResolvedTextDirection.Rtl
                        val lineEdgeStart = if (isRtl) layout.getLineRight(lineIdx) else layout.getLineLeft(lineIdx)
                        val lineEdgeEnd = if (isRtl) layout.getLineLeft(lineIdx) else layout.getLineRight(lineIdx)

                        var minStart = Long.MAX_VALUE
                        var maxEnd = Long.MIN_VALUE
                        var sweepX = lineEdgeStart
                        var anyStarted = false

                        for (span in spans) {
                            if (span.endChar <= lineStartChar || span.startChar >= lineEndChar) continue
                            if (span.startMs < minStart) minStart = span.startMs
                            if (span.endMs > maxEnd) maxEnd = span.endMs

                            val x0 = if (span.startChar <= lineStartChar) {
                                lineEdgeStart
                            } else {
                                layout.getHorizontalPosition(
                                    span.startChar.coerceIn(0, len),
                                    usePrimaryDirection = true,
                                )
                            }
                            val x1 = if (span.endChar >= lineEndChar) {
                                lineEdgeEnd
                            } else {
                                layout.getHorizontalPosition(
                                    span.endChar.coerceIn(0, len),
                                    usePrimaryDirection = true,
                                )
                            }

                            when {
                                nowMs >= span.endMs -> {
                                    anyStarted = true
                                    sweepX = if (isRtl) minOf(sweepX, x1) else maxOf(sweepX, x1)
                                }
                                nowMs >= span.startMs -> {
                                    anyStarted = true
                                    val dur = (span.endMs - span.startMs).coerceAtLeast(1L).toFloat()
                                    val t = ((nowMs - span.startMs).toFloat() / dur).coerceIn(0f, 1f)
                                    val currentX = x0 + (x1 - x0) * t
                                    sweepX = if (isRtl) minOf(sweepX, currentX) else maxOf(sweepX, currentX)
                                }
                            }
                        }

                        clipRect(0f, clipTop, size.width, clipBottom) {
                            when {
                                minStart == Long.MAX_VALUE || !anyStarted || nowMs <= minStart -> {
                                    drawRect(
                                        color = Color.Black,
                                        topLeft = Offset(0f, clipTop),
                                        size = Size(size.width, clipH),
                                        blendMode = BlendMode.DstOut,
                                    )
                                }
                                nowMs >= maxEnd -> Unit
                                else -> {
                                    val brush = if (!isRtl) {
                                        val gradStart = (sweepX - featherPx * 0.35f).coerceAtLeast(0f)
                                        val gradEnd = (sweepX + featherPx * 0.65f).coerceAtLeast(gradStart + 1f)
                                        Brush.horizontalGradient(
                                            colorStops = KaraokeSweepStops,
                                            startX = gradStart,
                                            endX = gradEnd,
                                        )
                                    } else {
                                        val gradStart = (sweepX + featherPx * 0.35f).coerceAtMost(size.width)
                                        val gradEnd = (sweepX - featherPx * 0.65f).coerceAtMost(gradStart - 1f)
                                        Brush.horizontalGradient(
                                            colorStops = KaraokeSweepStops,
                                            startX = gradStart,
                                            endX = gradEnd,
                                        )
                                    }
                                    drawRect(
                                        brush = brush,
                                        topLeft = Offset(0f, clipTop),
                                        size = Size(size.width, clipH),
                                        blendMode = BlendMode.DstIn,
                                    )
                                }
                            }
                        }
                    }
                },
        )
    }
}

class LayoutBoundsHolder {
    var coordinates: LayoutCoordinates? = null
    fun boundsInRoot(): Rect? = coordinates?.takeIf { it.isAttached }?.boundsInRoot()
}

@Composable
fun rememberLayoutBoundsHolder(): LayoutBoundsHolder = remember { LayoutBoundsHolder() }
