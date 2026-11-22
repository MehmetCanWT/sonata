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
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
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
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.PathMeasure
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
import dev.sonata.android.lightweightLiquidGlass
import dev.sonata.android.ui.components.SonataMoodAnimatedArt
import dev.sonata.android.ui.player.CardCanvasSurface
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

/** Signature Lunaria Moon Lavender accent across Sonata themes. */
val SonataAccent = LunariaColors.PetalLavender

val SonataDarkColors = darkColorScheme(
    primary = Color.White,
    onPrimary = Color.Black,
    background = Color.Black,
    onBackground = Color.White,
    surface = Color(0xFF0D0D0F),
    onSurface = Color.White,
    surfaceVariant = Color(0xFF1C1C1E),
    onSurfaceVariant = Color(0xFF8E8E93),
    outline = Color(0xFF2C2C2E),
)

val SonataLightColors = lightColorScheme(
    primary = Color.Black,
    onPrimary = Color.White,
    background = Color.White,
    onBackground = Color.Black,
    surface = Color(0xFFF7F7F9),
    onSurface = Color.Black,
    surfaceVariant = Color(0xFFF2F2F7),
    onSurfaceVariant = Color(0xFF6E6E73),
    outline = Color(0xFFE5E5EA),
)

/**
 * SF Pro Display font family shifted one step lighter so Compose weights match
 * optical display rendering (W800 -> bold, W700 -> semibold, W600 -> medium).
 */
val SFProDisplay = FontFamily(
    Font(R.font.sf_pro_display_regular, FontWeight.W400),
    Font(R.font.sf_pro_display_regular, FontWeight.W500),
    Font(R.font.sf_pro_display_medium, FontWeight.W600),
    Font(R.font.sf_pro_display_semibold, FontWeight.W700),
    Font(R.font.sf_pro_display_bold, FontWeight.W800),
)

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
    content: @Composable () -> Unit,
) {
    val tokens = lunariaTokens(darkTheme = darkTheme, amoled = amoled)
    CompositionLocalProvider(LocalLunariaColors provides tokens) {
        MaterialTheme(
            colorScheme = lunariaColorScheme(darkTheme = darkTheme, amoled = amoled),
            typography = SonataTypography,
            content = content,
        )
    }
}

/** Updates system status and navigation bar glyph appearance to match surface contrast. */
@Composable
fun SystemBarIcons(dark: Boolean) {
    val view = LocalView.current
    if (view.isInEditMode) return
    val window = findWindow(view) ?: return
    SideEffect {
        WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = dark
            isAppearanceLightNavigationBars = dark
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
// 3. Custom Vector Icons
// =============================================================================

/** Consistent thick-stroke and solid tab glyphs across Sonata Android. */
object SonataIcons {
    private const val STROKE = 2.2f
    private val stroke = SolidColor(Color.Black)

    val Home: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_home",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                fill = stroke,
                stroke = stroke,
                strokeLineWidth = 1.7f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(12f, 4.05f)
                lineTo(19.75f, 11.35f)
                lineTo(19.75f, 19.65f)
                lineTo(14.95f, 19.65f)
                lineTo(14.95f, 15.6f)
                lineTo(9.05f, 15.6f)
                lineTo(9.05f, 19.65f)
                lineTo(4.25f, 19.65f)
                lineTo(4.25f, 11.35f)
                close()
            }
        }.build()
    }

    val TabExplore: ImageVector by lazy {
        fontGlyph(
            "sonata_tab_explore",
            "M99.5 230.24Q58.5 230.24 58.5 188.24V93.24Q58.5 51.24 99.5 51.24H197.5Q238.5 51.24 238.5 93.24V188.24Q238.5 230.24 197.5 230.24Z" +
                "M314.5 230.24Q273.5 230.24 273.5 188.24V93.24Q273.5 51.24 314.5 51.24H412.5Q453.5 51.24 453.5 93.24V188.24Q453.5 230.24 412.5 230.24Z" +
                "M99.5 445.24Q58.5 445.24 58.5 403.24V307.24Q58.5 266.24 99.5 266.24H197.5Q238.5 266.24 238.5 307.24V403.24Q238.5 445.24 197.5 445.24Z" +
                "M314.5 445.24Q273.5 445.24 273.5 403.24V307.24Q273.5 266.24 314.5 266.24H412.5Q453.5 266.24 453.5 307.24V403.24Q453.5 445.24 412.5 445.24Z",
        )
    }

    val TabLibrary: ImageVector by lazy {
        fontGlyph(
            "sonata_tab_library",
            "M154 27.24Q156 0.24 185 0.24H322Q352 0.24 353 27.24Z" +
                "M116 87.24Q121 56.24 150 56.24H356Q385 56.24 390 87.24Z" +
                "M133 500.24Q66 500.24 66 434.24V187.24Q66 121.24 133 121.24H378Q446 121.24 446 187.24V434.24Q446 500.24 385 500.24Z",
        )
    }

    val TabSearch: ImageVector by lazy {
        fontGlyph(
            "sonata_tab_search",
            "M219 374.24Q151 374.24 102.5 325.74Q54 277.24 54 209.24Q54 141.24 102.5 92.74Q151 44.24 219 44.24Q287 44.24 335.5 92.74Q384 141.24 384 209.24Q384 264.24 350 309.24L451 410.24Q458 417.24 458 427.24Q458 438.24 451 445.24Q444 452.24 434 452.24Q423 452.24 416 445.24L315 343.24Q271 374.24 219 374.24Z" +
                "M128 300.24Q166 338.24 219 338.24Q272 338.24 310 300.24Q348 262.24 348 209.24Q348 156.24 310 118.24Q272 80.24 219 80.24Q166 80.24 128 118.24Q90 156.24 90 209.24Q90 262.24 128 300.24Z",
        )
    }

    private fun fontGlyph(name: String, pathData: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 512f,
            viewportHeight = 512f,
        ).addPath(pathData = addPathNodes(pathData), fill = stroke).build()

    val Queue: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_queue",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(stroke = stroke, strokeLineWidth = 2f, strokeLineCap = StrokeCap.Round) {
                for (y in listOf(6f, 12f, 18f)) {
                    moveTo(8f, y)
                    lineTo(21f, y)
                }
            }
            path(fill = stroke) {
                for (y in listOf(6f, 12f, 18f)) {
                    moveTo(4.35f, y)
                    arcToRelative(1.25f, 1.25f, 0f, true, true, -2.5f, 0f)
                    arcToRelative(1.25f, 1.25f, 0f, true, true, 2.5f, 0f)
                    close()
                }
            }
        }.build()
    }

    val LyricsQuote: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_lyrics_quote",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(6f, 3.5f)
                lineTo(18f, 3.5f)
                quadTo(21f, 3.5f, 21f, 6.5f)
                lineTo(21f, 15f)
                quadTo(21f, 18f, 18f, 18f)
                lineTo(10f, 18f)
                lineTo(6f, 21f)
                lineTo(6f, 18f)
                quadTo(3f, 18f, 3f, 15f)
                lineTo(3f, 6.5f)
                quadTo(3f, 3.5f, 6f, 3.5f)
                close()
            }
            path(fill = stroke) {
                for (x in listOf(7.6f, 12.8f)) {
                    moveTo(x, 8f)
                    lineTo(x + 3f, 8f)
                    lineTo(x + 3f, 11.1f)
                    quadTo(x + 3f, 13.2f, x + 0.5f, 13.7f)
                    lineTo(x + 0.5f, 12.3f)
                    quadTo(x + 1.7f, 12f, x + 1.7f, 11f)
                    lineTo(x, 11f)
                    close()
                }
            }
        }.build()
    }

    val Translate: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_translate",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = 1.9f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                // Source script glyph (left)
                moveTo(3.5f, 6.5f)
                lineTo(11.5f, 6.5f)
                moveTo(7.5f, 4f)
                lineTo(7.5f, 6.5f)
                moveTo(9.8f, 6.5f)
                quadTo(9.2f, 11.5f, 4.5f, 14.5f)
                moveTo(5.6f, 9.2f)
                quadTo(7.6f, 12f, 10.6f, 14f)
                // Target 'A' glyph (bottom-right)
                moveTo(12.2f, 19.5f)
                lineTo(16f, 10.2f)
                lineTo(19.8f, 19.5f)
                moveTo(13.4f, 16.8f)
                lineTo(18.6f, 16.8f)
            }
        }.build()
    }

    val Play: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_play",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                fill = stroke,
            ) {
                moveTo(6.8f, 4.8f)
                lineTo(19.2f, 12f)
                lineTo(6.8f, 19.2f)
                close()
            }
        }.build()
    }

    val Shuffle: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_shuffle",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(3.4f, 7.4f); lineTo(7f, 7.4f); lineTo(16.6f, 16.6f); lineTo(20.6f, 16.6f)
                moveTo(18.1f, 14.1f); lineTo(20.6f, 16.6f); lineTo(18.1f, 19.1f)
                moveTo(3.4f, 16.6f); lineTo(7f, 16.6f); lineTo(9.8f, 13.9f)
                moveTo(13.9f, 10.1f); lineTo(16.6f, 7.4f); lineTo(20.6f, 7.4f)
                moveTo(18.1f, 4.9f); lineTo(20.6f, 7.4f); lineTo(18.1f, 9.9f)
            }
        }.build()
    }

    val Repeat: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_repeat",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(8.6f, 7.6f)
                lineTo(15.4f, 7.6f)
                arcToRelative(4.4f, 4.4f, 0f, isMoreThanHalf = false, isPositiveArc = true, 0f, 8.8f)
                lineTo(8.6f, 16.4f)
                arcToRelative(4.4f, 4.4f, 0f, isMoreThanHalf = false, isPositiveArc = true, 0f, -8.8f)
                close()
                moveTo(13.5f, 5.7f); lineTo(15.4f, 7.6f); lineTo(13.5f, 9.5f)
                moveTo(10.5f, 14.5f); lineTo(8.6f, 16.4f); lineTo(10.5f, 18.3f)
            }
        }.build()
    }

    val RepeatOne: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_repeat_one",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(8.6f, 7.6f)
                lineTo(15.4f, 7.6f)
                arcToRelative(4.4f, 4.4f, 0f, isMoreThanHalf = false, isPositiveArc = true, 0f, 8.8f)
                lineTo(8.6f, 16.4f)
                arcToRelative(4.4f, 4.4f, 0f, isMoreThanHalf = false, isPositiveArc = true, 0f, -8.8f)
                close()
                moveTo(13.5f, 5.7f); lineTo(15.4f, 7.6f); lineTo(13.5f, 9.5f)
                moveTo(10.5f, 14.5f); lineTo(8.6f, 16.4f); lineTo(10.5f, 18.3f)
                // Centered "1" numeral inside the track loop.
                moveTo(11.3f, 10.7f); lineTo(12.2f, 10.0f); lineTo(12.2f, 14.0f)
            }
        }.build()
    }

    val Infinity: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_infinity",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(12f, 12f)
                curveTo(10.1f, 9.1f, 8.7f, 8f, 7.1f, 8f)
                arcToRelative(4f, 4f, 0f, isMoreThanHalf = false, isPositiveArc = false, 0f, 8f)
                curveTo(8.7f, 16f, 10.1f, 14.9f, 12f, 12f)
                curveTo(13.9f, 9.1f, 15.3f, 8f, 16.9f, 8f)
                arcToRelative(4f, 4f, 0f, isMoreThanHalf = false, isPositiveArc = true, 0f, 8f)
                curveTo(15.3f, 16f, 13.9f, 14.9f, 12f, 12f)
            }
        }.build()
    }

    val Star: ImageVector by lazy { star("sonata_star", filled = false) }
    val StarFilled: ImageVector by lazy { star("sonata_star_filled", filled = true) }

    private fun star(name: String, filled: Boolean): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                fill = if (filled) stroke else null,
            ) {
                moveTo(12f, 3.86f)
                lineTo(14.33f, 9.66f)
                lineTo(20.56f, 10.08f)
                lineTo(15.77f, 14.08f)
                lineTo(17.29f, 20.14f)
                lineTo(12f, 16.82f)
                lineTo(6.71f, 20.14f)
                lineTo(8.23f, 14.08f)
                lineTo(3.44f, 10.08f)
                lineTo(9.67f, 9.66f)
                close()
            }
        }.build()

    val Heart: ImageVector by lazy { heart("sonata_heart", filled = false) }
    val HeartFilled: ImageVector by lazy { heart("sonata_heart_filled", filled = true) }

    private fun heart(name: String, filled: Boolean): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                fill = if (filled) stroke else null,
            ) {
                moveTo(12f, 20f)
                curveTo(12f, 20f, 3.2f, 14.6f, 3.2f, 8.9f)
                arcToRelative(4.5f, 4.5f, 0f, isMoreThanHalf = false, isPositiveArc = true, 8.8f, -1.5f)
                arcToRelative(4.5f, 4.5f, 0f, isMoreThanHalf = false, isPositiveArc = true, 8.8f, 1.5f)
                curveTo(20.8f, 14.6f, 12f, 20f, 12f, 20f)
                close()
            }
        }.build()

    val MoreHoriz: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_more_horiz",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(fill = stroke) {
                for (cx in listOf(6.2f, 12f, 17.8f)) {
                    moveTo(cx + 1.5f, 12f)
                    arcToRelative(1.5f, 1.5f, 0f, true, true, -3f, 0f)
                    arcToRelative(1.5f, 1.5f, 0f, true, true, 3f, 0f)
                    close()
                }
            }
        }.build()
    }

    val Download: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_download",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(12f, 4.5f); lineTo(12f, 15.5f)
                moveTo(7.5f, 11f); lineTo(12f, 15.5f); lineTo(16.5f, 11f)
                moveTo(4.5f, 18f); lineTo(19.5f, 18f)
            }
        }.build()
    }

    val History: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_history",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(4.5f, 8.2f)
                arcToRelative(8.2f, 8.2f, 0f, isMoreThanHalf = true, isPositiveArc = true, -0.5f, 5.4f)
                moveTo(3.8f, 4.6f); lineTo(4.2f, 8.4f); lineTo(8.0f, 7.8f)
                moveTo(12f, 7.6f); lineTo(12f, 12f); lineTo(15.2f, 13.8f)
            }
        }.build()
    }

    val ChevronRight: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_chevron_right",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(9.5f, 6.2f)
                lineTo(15.3f, 12f)
                lineTo(9.5f, 17.8f)
            }
        }.build()
    }

    val ChevronLeft: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_chevron_left",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
            autoMirror = true,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(14.5f, 6.2f)
                lineTo(8.7f, 12f)
                lineTo(14.5f, 17.8f)
            }
        }.build()
    }

    val Folder: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_folder",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(3.5f, 7.2f)
                arcToRelative(2.2f, 2.2f, 0f, false, true, 2.2f, -2.2f)
                lineTo(9.5f, 5f)
                lineTo(11.6f, 7.3f)
                lineTo(18.3f, 7.3f)
                arcToRelative(2.2f, 2.2f, 0f, false, true, 2.2f, 2.2f)
                lineTo(20.5f, 16.8f)
                arcToRelative(2.2f, 2.2f, 0f, false, true, -2.2f, 2.2f)
                lineTo(5.7f, 19f)
                arcToRelative(2.2f, 2.2f, 0f, false, true, -2.2f, -2.2f)
                close()
            }
        }.build()
    }

    val Settings: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_settings",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = 1.9f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                // Inner hub
                moveTo(9f, 12f)
                arcToRelative(3f, 3f, 0f, true, true, 6f, 0f)
                arcToRelative(3f, 3f, 0f, true, true, -6f, 0f)
                // Gear outer contour
                moveTo(19.4f, 15f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 0.33f, 1.82f)
                lineTo(19.79f, 16.88f)
                arcToRelative(2f, 2f, 0f, true, true, -2.83f, 2.83f)
                lineTo(16.9f, 19.65f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1.82f, -0.33f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1f, 1.51f)
                lineTo(14.08f, 21f)
                arcToRelative(2f, 2f, 0f, true, true, -4f, 0f)
                lineTo(10.08f, 20.91f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1.08f, -1.51f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1.82f, 0.33f)
                lineTo(7.12f, 19.79f)
                arcToRelative(2f, 2f, 0f, true, true, -2.83f, -2.83f)
                lineTo(4.35f, 16.9f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 0.33f, -1.82f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1.51f, -1f)
                lineTo(3f, 14.08f)
                arcToRelative(2f, 2f, 0f, true, true, 0f, -4f)
                lineTo(3.09f, 10.08f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1.51f, -1.08f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -0.33f, -1.82f)
                lineTo(4.21f, 7.12f)
                arcToRelative(2f, 2f, 0f, true, true, 2.83f, -2.83f)
                lineTo(7.1f, 4.35f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1.82f, 0.33f)
                lineTo(9f, 4.68f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1f, -1.51f)
                lineTo(10f, 3f)
                arcToRelative(2f, 2f, 0f, true, true, 4f, 0f)
                lineTo(14f, 3.09f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1f, 1.51f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1.82f, -0.33f)
                lineTo(16.88f, 4.21f)
                arcToRelative(2f, 2f, 0f, true, true, 2.83f, 2.83f)
                lineTo(19.65f, 7.1f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -0.33f, 1.82f)
                lineTo(19.32f, 9f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, 1.51f, 1f)
                lineTo(21f, 10f)
                arcToRelative(2f, 2f, 0f, true, true, 0f, 4f)
                lineTo(20.91f, 14f)
                arcToRelative(1.65f, 1.65f, 0f, false, false, -1.51f, 1f)
                close()
            }
        }.build()
    }

    val Wifi: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_wifi",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(3.5f, 9.2f); quadTo(12f, 2.8f, 20.5f, 9.2f)
                moveTo(6.7f, 12.6f); quadTo(12f, 8.2f, 17.3f, 12.6f)
                moveTo(9.9f, 16.0f); quadTo(12f, 14.0f, 14.1f, 16.0f)
            }
            path(fill = stroke) {
                moveTo(13.2f, 19.1f)
                arcToRelative(1.2f, 1.2f, 0f, true, true, -2.4f, 0f)
                arcToRelative(1.2f, 1.2f, 0f, true, true, 2.4f, 0f)
                close()
            }
        }.build()
    }

    val Cellular: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_cellular",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(stroke = stroke, strokeLineWidth = 2.4f, strokeLineCap = StrokeCap.Round) {
                moveTo(5f, 18.5f); lineTo(5f, 15.8f)
                moveTo(9.6f, 18.5f); lineTo(9.6f, 12.8f)
                moveTo(14.3f, 18.5f); lineTo(14.3f, 9.2f)
                moveTo(19f, 18.5f); lineTo(19f, 5.5f)
            }
        }.build()
    }

    val Person: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_person",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(8.2f, 8.2f)
                arcToRelative(3.8f, 3.8f, 0f, true, true, 7.6f, 0f)
                arcToRelative(3.8f, 3.8f, 0f, true, true, -7.6f, 0f)
                moveTo(4.6f, 19.6f)
                curveTo(5.2f, 15.9f, 8.2f, 14.3f, 12f, 14.3f)
                curveTo(15.8f, 14.3f, 18.8f, 15.9f, 19.4f, 19.6f)
            }
        }.build()
    }

    val Sparkles: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_sparkles",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = 1.9f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                // Primary 4-point sparkle
                moveTo(10.5f, 3.5f)
                quadTo(11.5f, 10.5f, 18.5f, 11.5f)
                quadTo(11.5f, 12.5f, 10.5f, 19.5f)
                quadTo(9.5f, 12.5f, 2.5f, 11.5f)
                quadTo(9.5f, 10.5f, 10.5f, 3.5f)
                close()
                // Accent mini-sparkle
                moveTo(18.5f, 3.8f)
                quadTo(18.9f, 5.6f, 20.7f, 6.0f)
                quadTo(18.9f, 6.4f, 18.5f, 8.2f)
                quadTo(18.1f, 6.4f, 16.3f, 6.0f)
                quadTo(18.1f, 5.6f, 18.5f, 3.8f)
                close()
            }
        }.build()
    }

    val Check: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_check",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(5f, 12.8f); lineTo(9.6f, 17.4f); lineTo(19f, 6.9f)
            }
        }.build()
    }

    val Close: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_close",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(6f, 6f); lineTo(18f, 18f)
                moveTo(18f, 6f); lineTo(6f, 18f)
            }
        }.build()
    }

    val Search: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_search",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = STROKE,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(4.6f, 11f)
                arcToRelative(6.4f, 6.4f, 0f, isMoreThanHalf = true, isPositiveArc = true, 12.8f, 0f)
                arcToRelative(6.4f, 6.4f, 0f, isMoreThanHalf = true, isPositiveArc = true, -12.8f, 0f)
                moveTo(15.9f, 15.9f); lineTo(20.4f, 20.4f)
            }
        }.build()
    }

    val Instrumental: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_instrumental",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(
                "M19.952 1.651a.75.75 0 0 1 .298.599V16.303a3 3 0 0 1-2.176 2.884l-1.32.377a2.553 2.553 0 1 1-1.403-4.909" +
                    "l2.311-.66a1.5 1.5 0 0 0 1.088-1.442V6.994l-9 2.572v9.737a3 3 0 0 1-2.176 2.884l-1.32.377" +
                    "a2.553 2.553 0 1 1-1.402-4.909l2.31-.66a1.5 1.5 0 0 0 1.088-1.442V5.25a.75.75 0 0 1 .544-.721" +
                    "l10.5-3a.75.75 0 0 1 .658.122Z",
            ),
            pathFillType = PathFillType.EvenOdd,
            fill = stroke,
        ).build()
    }

    val MusicNote: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_music_note",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(
                "M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"
            ),
            fill = stroke,
        ).build()
    }

    val GraphicEq: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_graphic_eq",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(stroke = stroke, strokeLineWidth = 2.4f, strokeLineCap = StrokeCap.Round) {
                moveTo(4f, 10f); lineTo(4f, 14f)
                moveTo(8f, 6.5f); lineTo(8f, 17.5f)
                moveTo(12f, 3.5f); lineTo(12f, 20.5f)
                moveTo(16f, 6.5f); lineTo(16f, 17.5f)
                moveTo(20f, 10f); lineTo(20f, 14f)
            }
        }.build()
    }

    val Pause: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_pause",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(fill = stroke) {
                moveTo(6.5f, 5f); lineTo(10.2f, 5f); lineTo(10.2f, 19f); lineTo(6.5f, 19f); close()
                moveTo(13.8f, 5f); lineTo(17.5f, 5f); lineTo(17.5f, 19f); lineTo(13.8f, 19f); close()
            }
        }.build()
    }

    val MoreVert: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_more_vert",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(fill = stroke) {
                for (cy in listOf(6.2f, 12f, 17.8f)) {
                    moveTo(13.5f, cy)
                    arcToRelative(1.5f, 1.5f, 0f, true, true, -3f, 0f)
                    arcToRelative(1.5f, 1.5f, 0f, true, true, 3f, 0f)
                    close()
                }
            }
        }.build()
    }

    val ThumbUp: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_thumb_up",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(
                "M2 20h2c.55 0 1-.45 1-1v-9c0-.55-.45-1-1-1H2v11zm19.83-7.12c.11-.25.17-.52.17-.8V11c0-1.1-.9-2-2-2h-5.5" +
                    "l.92-4.65c.05-.22.02-.46-.08-.66-.23-.45-.52-.86-.88-1.22L14 2 7.59 8.41C7.21 8.79 7 9.3 7 9.83" +
                    "v7.84C7 18.95 8.05 20 9.34 20h8.11c.7 0 1.36-.37 1.72-.97l2.66-6.15z"
            ),
            fill = stroke,
        ).build()
    }
}

// =============================================================================
// 4. Shimmer Skeletons
// =============================================================================

private const val SHIMMER_PERIOD_MS = 1400
private val SkeletonBlockShape = RoundedCornerShape(6.dp)
private val SkeletonLineShape = RoundedCornerShape(4.dp)
private val TitleWidths = listOf(0.68f, 0.46f, 0.58f, 0.74f, 0.52f)
private val SubtitleWidths = listOf(0.34f, 0.44f, 0.27f, 0.38f, 0.31f)

/** Shared shimmer progress state so 30+ skeleton tiles run on a single InfiniteTransition. */
private val LocalShimmerProgress = compositionLocalOf<State<Float>?> { null }

/** Hosts a single shared shimmer transition for all descendant skeleton placeholders. */
@Composable
fun SonataShimmerHost(content: @Composable () -> Unit) {
    val sweep = rememberInfiniteTransition(label = "sonataShimmerHost").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(SHIMMER_PERIOD_MS, easing = LinearEasing)),
        label = "shimmerSweep",
    )
    CompositionLocalProvider(LocalShimmerProgress provides sweep) {
        content()
    }
}

@Composable
fun ShimmerBox(
    modifier: Modifier = Modifier,
    shape: Shape = SkeletonBlockShape,
) {
    val base = MaterialTheme.colorScheme.surfaceVariant
    val onVariant = MaterialTheme.colorScheme.onSurfaceVariant
    val highlight = remember(base, onVariant) {
        onVariant
            .copy(alpha = 0.16f)
            .compositeOver(base)
    }
    val sharedSweep = LocalShimmerProgress.current
    val sweep = sharedSweep ?: rememberInfiniteTransition(label = "skeleton").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(SHIMMER_PERIOD_MS, easing = LinearEasing)),
        label = "sweep",
    )
    Box(
        modifier
            .clip(shape)
            .drawWithCache {
                val band = (size.width * 0.5f).coerceAtLeast(1f)
                val bandBrush = Brush.horizontalGradient(
                    colors = listOf(base, highlight, base),
                    startX = 0f,
                    endX = band,
                )
                val travel = size.width + band * 2f
                onDrawBehind {
                    drawRect(base)
                    val startX = -band + sweep.value * travel
                    translate(left = startX) {
                        drawRect(brush = bandBrush, size = Size(band, size.height))
                    }
                }
            },
    )
}

@Composable
private fun SkeletonLine(fraction: Float, height: Dp, modifier: Modifier = Modifier) {
    ShimmerBox(
        modifier = modifier
            .fillMaxWidth(fraction)
            .height(height),
        shape = SkeletonLineShape,
    )
}

@Composable
fun SectionHeaderSkeleton(index: Int = 0) {
    Column(Modifier.padding(horizontal = PAGE_GUTTER, vertical = 10.dp)) {
        SkeletonLine(fraction = TitleWidths[index % TitleWidths.size] * 0.7f, height = 18.dp)
    }
}

@Composable
fun SongRowSkeleton(
    index: Int = 0,
    circular: Boolean = false,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ShimmerBox(Modifier.size(52.dp), if (circular) CircleShape else RoundedCornerShape(8.dp))
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            SkeletonLine(fraction = TitleWidths[index % TitleWidths.size], height = 14.dp)
            Spacer(Modifier.height(7.dp))
            SkeletonLine(fraction = SubtitleWidths[index % SubtitleWidths.size], height = 11.dp)
        }
    }
}

@Composable
private fun CompactSongRowSkeleton(index: Int) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ShimmerBox(Modifier.size(48.dp), RoundedCornerShape(7.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            SkeletonLine(fraction = TitleWidths[index % TitleWidths.size], height = 14.dp)
            Spacer(Modifier.height(6.dp))
            SkeletonLine(fraction = SubtitleWidths[index % SubtitleWidths.size], height = 11.dp)
        }
    }
}

fun LazyListScope.songListSkeleton(
    count: Int = 8,
    keyPrefix: String = "skeleton:song",
    circular: Boolean = false,
    alpha: Float = 1f,
) {
    items(count, key = { "$keyPrefix:$it" }) { index ->
        Box(Modifier.graphicsLayer { this.alpha = alpha }) {
            SongRowSkeleton(index = index, circular = circular)
        }
    }
}

fun LazyListScope.recentlyPlayedSkeleton(rows: Int = 4) {
    item(key = "skeleton:recently-played") {
        val screenWidth = LocalConfiguration.current.screenWidthDp.dp
        val columnWidth = trackColumnWidth(screenWidth)
        val columnCount = (((screenWidth - PAGE_GUTTER) / (columnWidth + 12.dp)).toInt() + 1).coerceIn(2, 5)
        Column(Modifier.padding(bottom = 26.dp)) {
            SectionHeaderSkeleton(index = 0)
            LazyRow(
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                userScrollEnabled = false,
            ) {
                items(columnCount) {
                    Column(Modifier.width(columnWidth)) {
                        repeat(rows.coerceAtLeast(1)) { index -> CompactSongRowSkeleton(index) }
                    }
                }
            }
        }
    }
}

fun LazyListScope.topPicksSkeleton() {
    item(key = "skeleton:top-picks") {
        val screenWidth = LocalConfiguration.current.screenWidthDp.dp
        val cardWidth = minOf(screenWidth * 0.60f, 280.dp)
        val cardCount = (((screenWidth - PAGE_GUTTER) / (cardWidth + 14.dp)).toInt() + 1).coerceIn(2, 6)
        Column(Modifier.padding(bottom = 26.dp)) {
            SectionHeaderSkeleton()
            LazyRow(
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                userScrollEnabled = false,
            ) {
                items(cardCount) {
                    ShimmerBox(
                        modifier = Modifier
                            .width(cardWidth)
                            .aspectRatio(0.75f),
                        shape = RoundedCornerShape(12.dp),
                    )
                }
            }
        }
    }
}

@Composable
fun ShelfSkeleton(
    index: Int = 0,
    cardWidth: Dp = SHELF_CARD_WIDTH,
    cardCorner: Dp = 12.dp,
) {
    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    val cardCount = (((screenWidth - PAGE_GUTTER) / (cardWidth + 14.dp)).toInt() + 1).coerceIn(3, 8)
    Column(Modifier.padding(bottom = 26.dp)) {
        SectionHeaderSkeleton(index = index)
        LazyRow(
            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            userScrollEnabled = false,
        ) {
            items(cardCount) { card ->
                Column(Modifier.width(cardWidth)) {
                    ShimmerBox(
                        modifier = Modifier
                            .width(cardWidth)
                            .aspectRatio(1f),
                        shape = RoundedCornerShape(cardCorner),
                    )
                    Spacer(Modifier.height(12.dp))
                    SkeletonLine(fraction = TitleWidths[card % TitleWidths.size], height = 13.dp)
                    Spacer(Modifier.height(6.dp))
                    SkeletonLine(fraction = SubtitleWidths[card % SubtitleWidths.size], height = 11.dp)
                }
            }
        }
    }
}

fun LazyListScope.feedSkeleton(sections: Int = 2) {
    items(sections, key = { "skeleton:shelf:$it" }) { index ->
        ShelfSkeleton(index = index)
    }
}

@Composable
fun ReplayCardRowSkeleton(modifier: Modifier = Modifier) {
    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    val cardCount = (((screenWidth - PAGE_GUTTER) / 312.dp).toInt() + 1).coerceIn(2, 5)
    LazyRow(
        modifier = modifier.padding(vertical = 6.dp),
        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        userScrollEnabled = false,
    ) {
        items(cardCount) {
            ShimmerBox(
                modifier = Modifier
                    .width(300.dp)
                    .aspectRatio(1.586f),
                shape = RoundedCornerShape(20.dp),
            )
        }
    }
}

fun LazyListScope.detailSkeleton(trackRows: Int = 8, isArtist: Boolean = false) {
    if (isArtist) {
        item(key = "skeleton:detail:actions") {
            Column(Modifier.graphicsLayer { alpha = 0.4f }) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER + 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally),
                ) {
                    ShimmerBox(Modifier.size(50.dp), CircleShape)
                    ShimmerBox(Modifier.width(130.dp).height(50.dp), CircleShape)
                }
                Spacer(Modifier.height(22.dp))
            }
        }
        item(key = "skeleton:detail:top") {
            val screenWidth = LocalConfiguration.current.screenWidthDp.dp
            val columnWidth = trackColumnWidth(screenWidth)
            Column(Modifier.graphicsLayer { alpha = 0.4f }) {
                SectionHeaderSkeleton()
                LazyRow(
                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    userScrollEnabled = false,
                ) {
                    item {
                        Column(Modifier.width(columnWidth)) {
                            repeat(4) { index -> CompactSongRowSkeleton(index) }
                        }
                    }
                }
            }
        }
        item(key = "skeleton:detail:sections") {
            Box(Modifier.graphicsLayer { alpha = 0.4f }) {
                ShelfSkeleton(index = 2, cardCorner = 10.dp)
            }
        }
        return
    }
    item(key = "skeleton:detail:actions") {
        Column(Modifier.graphicsLayer { alpha = 0.4f }) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER + 8.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally),
            ) {
                ShimmerBox(Modifier.size(50.dp), CircleShape)
                ShimmerBox(Modifier.width(130.dp).height(50.dp), CircleShape)
                ShimmerBox(Modifier.size(50.dp), CircleShape)
            }
            Spacer(Modifier.height(20.dp))
        }
    }
    songListSkeleton(count = trackRows, keyPrefix = "skeleton:detail:song", alpha = 0.4f)
}

private val LYRICS_SKELETON_BLOCKS = listOf(
    floatArrayOf(0.97f, 0.54f),
    floatArrayOf(0.92f, 0.99f, 0.41f),
    floatArrayOf(0.68f),
    floatArrayOf(0.95f, 0.73f),
    floatArrayOf(0.89f, 0.96f, 0.37f),
)

@Composable
fun LyricsSkeleton(modifier: Modifier = Modifier) {
    val sweep = rememberInfiniteTransition(label = "lyricsSkeleton").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(SHIMMER_PERIOD_MS, easing = LinearEasing)),
        label = "sweep",
    )
    var columnWidthPx by remember { mutableIntStateOf(0) }
    val baseFill = remember { Color.White.copy(alpha = 0.10f) }
    val bandColors = remember {
        listOf(
            Color.Transparent,
            Color.White.copy(alpha = 0.18f),
            Color.Transparent,
        )
    }
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = 40.dp)
            .onSizeChanged { columnWidthPx = it.width },
        verticalArrangement = Arrangement.spacedBy(35.dp),
    ) {
        LYRICS_SKELETON_BLOCKS.forEach { rows ->
            Column(verticalArrangement = Arrangement.spacedBy(15.dp)) {
                rows.forEach { fraction ->
                    Box(
                        Modifier
                            .fillMaxWidth(fraction)
                            .height(26.dp)
                            .clip(SkeletonLineShape)
                            .drawWithCache {
                                val full = columnWidthPx.toFloat().coerceAtLeast(size.width).coerceAtLeast(1f)
                                val band = (full * 0.45f).coerceAtLeast(1f)
                                val bandBrush = Brush.horizontalGradient(
                                    colors = bandColors,
                                    startX = 0f,
                                    endX = band,
                                )
                                val travel = full + band * 2f
                                onDrawBehind {
                                    drawRect(baseFill)
                                    val startX = -band + sweep.value * travel
                                    translate(left = startX) {
                                        drawRect(brush = bandBrush, size = Size(band, size.height))
                                    }
                                }
                            },
                    )
                }
            }
        }
    }
}

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

@Composable
fun rememberArtworkPalette(
    artworkUrl: String?,
    dark: Boolean = true,
    artPx: Int = CARD_ART_PX,
    animated: Boolean = true,
): ArtworkPalette {
    val context = LocalPlatformContext.current
    val scheme = MaterialTheme.colorScheme
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

    val target = remember(seed, dark, scheme) {
        seed?.toPalette(dark) ?: ArtworkPalette(
            background = scheme.background,
            wash = scheme.background,
            elevated = scheme.surfaceVariant,
            accent = SonataAccent,
            onBackground = scheme.onBackground,
            onBackgroundVariant = scheme.onSurfaceVariant,
            divider = scheme.outline,
            meshColors = DefaultFallbackColors,
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

    val c0: Color
    val c1: Color
    val c2: Color
    val c3: Color
    val baseColor: Color
    if (reduceAnimation || !animated) {
        c0 = tuned[0]
        c1 = tuned[1]
        c2 = tuned[2]
        c3 = tuned[3]
        baseColor = dimmedBase
    } else {
        val colorSpec: AnimationSpec<Color> = tween(colorFadeMillis)
        c0 = animateColorAsState(tuned[0], colorSpec, label = "meshColor0").value
        c1 = animateColorAsState(tuned[1], colorSpec, label = "meshColor1").value
        c2 = animateColorAsState(tuned[2], colorSpec, label = "meshColor2").value
        c3 = animateColorAsState(tuned[3], colorSpec, label = "meshColor3").value
        baseColor = animateColorAsState(dimmedBase, colorSpec, label = "meshBase").value
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
            .background(baseColor)
            .drawWithCache {
                val radius = (size.maxDimension * 0.62f).coerceAtLeast(1f)
                val colors = arrayOf(c0, c1, c2, c3)
                val brushes = Array(4) { idx ->
                    val col = colors[idx]
                    val stops = Array(FADE_CURVE_STOPS + 1) { i ->
                        (i / FADE_CURVE_STOPS.toFloat()) to col.copy(alpha = MeshSmoothAlphaSteps[i])
                    }
                    Brush.radialGradient(
                        colorStops = stops,
                        center = Offset.Zero,
                        radius = radius,
                    )
                }
                onDrawBehind {
                    val drift = phase.value
                    for (index in 0 until 4) {
                        val anchor = MeshAnchors[index]
                        val speed = MeshSpeeds[index]
                        val cx = (anchor.x + 0.16f * cos(drift * speed + index * 1.7f)) * size.width
                        val cy = (anchor.y + 0.16f * sin(drift * speed * 0.9f + index * 2.3f)) * size.height
                        translate(left = cx, top = cy) {
                            drawCircle(
                                brush = brushes[index],
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

// =============================================================================
// 6. Stations, Moods, Releases, Explore Sleeves & Replay Cards
// =============================================================================

private val LocalCardMotionEnabled = staticCompositionLocalOf { true }

@Composable
private fun loopPhase(periodMs: Int): State<Float> {
    val phase = remember { mutableFloatStateOf(0f) }
    val running = LocalCardMotionEnabled.current && !LocalReduceAnimation.current
    LaunchedEffect(running, periodMs) {
        if (!running) return@LaunchedEffect
        var last = Long.MIN_VALUE
        while (isActive) {
            withFrameNanos { now ->
                if (last != Long.MIN_VALUE) {
                    phase.floatValue = (phase.floatValue + (now - last) / (periodMs * 1_000_000f)) % 1f
                }
                last = now
            }
            delay(33L)
        }
    }
    return phase
}

private class SizeCache<T : Any>(private val build: (Size) -> T) {
    private var size = Size.Unspecified
    private var value: T? = null
    fun get(current: Size): T {
        val cached = value
        if (cached != null && current == size) return cached
        size = current
        return build(current).also { value = it }
    }
}

private val PersonalLayers = listOf(
    Triple(1.00f, Color(0xFFFE5E83), Color(0xFFFA6196)),
    Triple(0.66f, Color(0xFFFE5065), Color(0xFFFC657C)),
    Triple(0.43f, Color(0xFFFB594C), Color(0xFFFC776D)),
    Triple(0.21f, Color(0xFFFB6729), Color(0xFFF98A48)),
)

private class PersonalBrushes(val background: Brush, val layers: List<Brush>)

@Composable
fun PersonalStationArt(
    modifier: Modifier = Modifier,
    still: Boolean = false,
) {
    val phaseState = if (still) null else loopPhase(9_000)
    val path = remember { Path() }
    val brushes = remember {
        SizeCache { size ->
            PersonalBrushes(
                background = Brush.horizontalGradient(
                    0f to Color(0xFFF7334F),
                    0.38f to Color(0xFFFA6094),
                    0.75f to Color(0xFFF02A4A),
                    1f to Color(0xFFEE213D),
                ),
                layers = PersonalLayers.map { (rest, deep, light) ->
                    Brush.horizontalGradient(listOf(deep, light), startX = 0f, endX = size.width * rest)
                },
            )
        }
    }
    Box(modifier) {
        Canvas(Modifier.fillMaxSize()) {
            val phase = phaseState?.value ?: 0f
            val w = size.width
            val h = size.height
            val cached = brushes.get(size)
            drawRect(cached.background)
            PersonalLayers.forEachIndexed { index, (rest, _, _) ->
                val tip = if (index == 0) rest else rest + 0.07f * sin(2f * PI.toFloat() * (phase + index * 0.19f))
                path.reset()
                path.moveTo(0f, 0f)
                path.lineTo(w * tip, h / 2f)
                path.lineTo(0f, h)
                path.close()
                drawPath(path, cached.layers[index])
            }
        }
        Icon(
            painter = painterResource(R.drawable.ic_sonata_moon),
            contentDescription = null,
            tint = Color.White,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = 12.dp, end = 12.dp)
                .height(15.dp)
                .aspectRatio(1f),
        )
    }
}

private val DiscoveryTopColors = listOf(
    Color(0xFF3A2496), Color(0xFF2F4FB0), Color(0xFF3B30A0),
    Color(0xFF2E7FB8), Color(0xFF27988A), Color(0xFF3C8DA8),
)
private val DiscoveryBottomColors = listOf(
    Color(0xFF3A2496), Color(0xFF333EA4), Color(0xFF2F62B6),
    Color(0xFF288B99), Color(0xFF3477BA), Color(0xFF42A296),
)
private val DiscoveryEdges = listOf(0f, 0.24f, 0.44f, 0.62f, 0.78f, 0.91f, 1.02f)
private val DiscoveryInnerArrow = Color(0xFF3C1981).copy(alpha = 0.35f)
private class DiscoveryBrushes(val background: Brush, val top: List<Brush>, val bottom: List<Brush>, val arrow: Brush)

@Composable
fun DiscoveryStationArt(
    modifier: Modifier = Modifier,
    still: Boolean = false,
) {
    val phaseState = if (still) null else loopPhase(9_000)
    val path = remember { Path() }
    val brushes = remember {
        SizeCache { size ->
            val apex = Offset(size.width, size.height / 2f)
            fun shards(edgeY: Float, colors: List<Color>) = (0 until DiscoveryEdges.lastIndex).map { i ->
                val mid = size.width * (DiscoveryEdges[i] + DiscoveryEdges[i + 1]) / 2f
                val base = colors[i % colors.size]
                Brush.linearGradient(listOf(lerp(base, Color.White, 0.10f), base), start = apex, end = Offset(mid, edgeY))
            }
            DiscoveryBrushes(
                background = Brush.linearGradient(
                    listOf(Color(0xFF2A869F), Color(0xFF3477BA)),
                    Offset.Zero,
                    Offset(size.width, size.height),
                ),
                top = shards(0f, DiscoveryTopColors),
                bottom = shards(size.height, DiscoveryBottomColors),
                arrow = Brush.horizontalGradient(
                    0f to Color(0xFF502798),
                    0.55f to Color(0xFF421C88),
                    1f to Color(0xFF4A2292),
                ),
            )
        }
    }
    Box(modifier) {
        Canvas(Modifier.fillMaxSize()) {
            val phase = phaseState?.value ?: 0f
            val w = size.width
            val h = size.height
            val cached = brushes.get(size)
            drawRect(cached.background)
            fun fan(edgeY: Float, shards: List<Brush>, offset: Float) {
                var previous = DiscoveryEdges[0]
                for (i in 1..DiscoveryEdges.lastIndex) {
                    val rest = DiscoveryEdges[i]
                    val edge = if (i == DiscoveryEdges.lastIndex) rest else {
                        rest + 0.06f * sin(2f * PI.toFloat() * (phase + offset + i * 0.11f))
                    }
                    path.reset()
                    path.moveTo(w, h / 2f)
                    path.lineTo(w * previous, edgeY)
                    path.lineTo(w * edge, edgeY)
                    path.close()
                    drawPath(path, shards[i - 1])
                    previous = edge
                }
            }
            fan(0f, cached.top, 0f)
            fan(h, cached.bottom, 0.5f)

            path.reset()
            path.moveTo(0f, 0f)
            path.lineTo(w, h / 2f)
            path.lineTo(0f, h)
            path.close()
            drawPath(path, cached.arrow)
            val inner = 0.08f + 0.04f * sin(2f * PI.toFloat() * (phase + 0.25f))
            path.reset()
            path.moveTo(0f, h * inner)
            path.lineTo(w * (0.9f - inner), h / 2f)
            path.lineTo(0f, h * (1f - inner))
            path.close()
            drawPath(path, DiscoveryInnerArrow)
        }
        Icon(
            painter = painterResource(R.drawable.ic_sonata_moon),
            contentDescription = null,
            tint = Color.White,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = 12.dp, end = 12.dp)
                .height(15.dp)
                .aspectRatio(1f),
        )
    }
}

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

private data class MoodVisualStyle(
    val background: Color,
    val stroke: Color,
    val periodMs: Int,
)

private fun TopPickMood.visualStyle(): MoodVisualStyle = when (this) {
    TopPickMood.ENERGY -> MoodVisualStyle(Color(0xFF1EBD38), Color(0xFFE2F6A2), 3_000)
    TopPickMood.RELAX -> MoodVisualStyle(Color(0xFF31AFF3), Color(0xFFB4E9FF), 5_600)
    TopPickMood.FEEL_GOOD -> MoodVisualStyle(Color(0xFFF2A900), Color(0xFFFFE8A0), 4_000)
    TopPickMood.BLUE -> MoodVisualStyle(Color(0xFF253AB2), Color(0xFF6CC4FF), 3_700)
    TopPickMood.FOCUS -> MoodVisualStyle(Color(0xFFF28400), Color(0xFFFFD6A4), 3_600)
    TopPickMood.HEARTBREAK -> MoodVisualStyle(Color(0xFFEA3520), Color(0xFFFFA690), 2_700)
    TopPickMood.ROMANCE -> MoodVisualStyle(Color(0xFFF34CA0), Color(0xFFFFB8DC), 1_700)
}

private class MoodFrame(val left: Float, val top: Float, val side: Float) {
    fun x(f: Float) = left + side * f
    fun y(f: Float) = top + side * f
    val center: Offset get() = Offset(x(0.5f), y(0.5f))
}

private class MoodEmblemCache {
    private var builtFor = Size.Unspecified
    private val outline = Path()
    val segment = Path()
    val measure = PathMeasure()
    var length = 0f
        private set

    fun outline(size: Size, measured: Boolean = false, build: (Path) -> Unit): Path {
        if (size != builtFor) {
            outline.reset()
            build(outline)
            if (measured) {
                measure.setPath(outline, false)
                length = measure.length
            }
            builtFor = size
        }
        return outline
    }
}

@Composable
fun MoodArt(
    mood: TopPickMood,
    modifier: Modifier = Modifier,
    square: Boolean = false,
    still: Boolean = false,
) {
    val style = mood.visualStyle()
    val loop = if (still) null else loopPhase(style.periodMs)
    val cache = remember(mood) { MoodEmblemCache() }
    Canvas(modifier.graphicsLayer { if (!still) compositingStrategy = CompositingStrategy.Offscreen }) {
        drawRect(style.background)
        val w = size.width
        val h = size.height
        val f = if (square) {
            val side = w * 0.70f
            MoodFrame(left = (w - side) / 2f, top = (h - side) / 2f, side = side)
        } else {
            val side = minOf(w * 0.84f, h * 0.62f)
            MoodFrame(left = (w - side) / 2f, top = h * 0.37f - side / 2f, side = side)
        }
        val stroke = Stroke(
            width = (if (square) 2.dp else 2.5.dp).toPx(),
            cap = StrokeCap.Round,
            join = StrokeJoin.Round,
        )
        drawMoodEmblem(mood, f, loop?.value ?: 0f, style.stroke, stroke, cache)
    }
}

private fun DrawScope.drawMoodEmblem(
    mood: TopPickMood,
    f: MoodFrame,
    phase: Float,
    color: Color,
    stroke: Stroke,
    cache: MoodEmblemCache,
) {
    val tau = 2f * PI.toFloat()
    val side = f.side
    when (mood) {
        TopPickMood.ENERGY -> {
            val path = cache.outline(size) { out ->
                val pts = listOf(
                    0.486f to 0.123f, 0.886f to 0.123f, 0.532f to 0.509f, 0.877f to 0.509f,
                    0.114f to 0.895f, 0.409f to 0.614f, 0.105f to 0.614f,
                )
                out.reset()
                pts.forEachIndexed { idx, (px, py) ->
                    if (idx == 0) out.moveTo(f.x(px), f.y(py)) else out.lineTo(f.x(px), f.y(py))
                }
                out.close()
            }
            val t = (phase - 0.28f) / 0.26f
            val flash = if (t in 0f..1f) sin(PI.toFloat() * t) else 0f
            if (flash > 0f) {
                translate(left = side * 0.07f * flash) { drawPath(path, color.copy(alpha = 0.55f * flash), style = stroke) }
                translate(left = -side * 0.05f * flash) { drawPath(path, color.copy(alpha = 0.35f * flash), style = stroke) }
            }
            drawPath(path, color, style = stroke)
        }
        TopPickMood.RELAX -> {
            val stones = listOf(
                Triple(0.17f, 0.44f, 0.18f),
                Triple(0.38f, 0.68f, 0.28f),
                Triple(0.66f, 0.84f, 0.36f),
            )
            stones.forEachIndexed { index, (cy, width, height) ->
                val sway = sin(tau * (phase + index * 0.22f))
                val center = Offset(f.x(0.5f + 0.015f * sway), f.y(cy))
                rotate(degrees = 4f * sway, pivot = center) {
                    drawOval(
                        color = color,
                        topLeft = Offset(center.x - side * width / 2f, center.y - side * height / 2f),
                        size = Size(side * width, side * height),
                        style = stroke,
                    )
                }
            }
        }
        TopPickMood.BLUE -> {
            cache.outline(size, measured = true) { out ->
                val turns = 5.5f
                val steps = 320
                out.reset()
                for (k in 0..steps) {
                    val t = k.toFloat() / steps
                    val angle = PI.toFloat() + t * turns * tau
                    val rx = 0.38f * (1f - 0.72f * t)
                    val ry = 0.075f * (1f - 0.55f * t)
                    val px = f.x(0.5f + rx * cos(angle))
                    val py = f.y(0.10f + 0.78f * t + ry * sin(angle))
                    if (k == 0) out.moveTo(px, py) else out.lineTo(px, py)
                }
            }
            val length = cache.length
            fun ease(t: Float) = 0.5f - 0.5f * cos(PI.toFloat() * t.coerceIn(0f, 1f))
            val from: Float
            val to: Float
            when {
                phase < 0.35f -> { from = 0f; to = length }
                phase < 0.8f -> { from = length * ease((phase - 0.35f) / 0.45f); to = length }
                else -> { from = 0f; to = length * ease((phase - 0.8f) / 0.2f) }
            }
            cache.segment.reset()
            if (to > from) cache.measure.getSegment(from, to, cache.segment, true)
            drawPath(cache.segment, color, style = stroke)
        }
        TopPickMood.FEEL_GOOD -> {
            val path = cache.outline(size) { out ->
                out.reset()
                out.moveTo(f.x(0.5f), f.y(0.06f))
                out.quadraticTo(f.x(0.56f), f.y(0.44f), f.x(0.94f), f.y(0.5f))
                out.quadraticTo(f.x(0.56f), f.y(0.56f), f.x(0.5f), f.y(0.94f))
                out.quadraticTo(f.x(0.44f), f.y(0.56f), f.x(0.06f), f.y(0.5f))
                out.quadraticTo(f.x(0.44f), f.y(0.44f), f.x(0.5f), f.y(0.06f))
                out.close()
            }
            val pulse = 0.9f + 0.1f * (0.5f + 0.5f * cos(tau * phase))
            withTransform({
                rotate(8f * sin(tau * phase), f.center)
                scale(pulse, pulse, f.center)
            }) { drawPath(path, color, style = stroke) }
        }
        TopPickMood.FOCUS -> {
            for (i in 0 until 4) {
                val share = (i + 1 - phase) / 4f
                val alpha = when {
                    share > 0.85f -> (1f - share) / 0.15f
                    share < 0.15f -> share / 0.15f
                    else -> 1f
                }.coerceIn(0f, 1f)
                drawCircle(color.copy(alpha = alpha), radius = side * 0.5f * share, center = f.center, style = stroke)
            }
        }
        TopPickMood.HEARTBREAK -> {
            val path = cache.outline(size) { buildHeartPath(it, f) }
            val dash = side * 0.075f
            val gap = side * 0.055f
            drawPath(
                path,
                color,
                style = Stroke(
                    width = stroke.width,
                    cap = StrokeCap.Round,
                    join = StrokeJoin.Round,
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(dash, gap), -phase * (dash + gap) * 2f),
                ),
            )
        }
        TopPickMood.ROMANCE -> {
            val path = cache.outline(size) { buildHeartPath(it, f) }
            fun bump(at: Float): Float {
                val d = (phase - at) / 0.06f
                return exp(-d * d)
            }
            val beat = 1f + 0.07f * (bump(0.15f) + 0.7f * bump(0.38f))
            scale(beat, beat, f.center) { drawPath(path, color, style = stroke) }
        }
    }
}

private fun buildHeartPath(path: Path, f: MoodFrame) {
    val r = 0.24f
    path.reset()
    path.moveTo(f.x(0.5f), f.y(0.92f))
    path.lineTo(f.x(0.28f - r * 0.707f), f.y(0.30f + r * 0.707f))
    path.arcTo(Rect(f.x(0.28f - r), f.y(0.30f - r), f.x(0.28f + r), f.y(0.30f + r)), 135f, 195f, false)
    path.arcTo(Rect(f.x(0.72f - r), f.y(0.30f - r), f.x(0.72f + r), f.y(0.30f + r)), 210f, 195f, false)
    path.close()
}

@Composable
fun MoodTile(
    mood: TopPickMood,
    onClick: (TopPickMood, String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(12.dp)
    val borderColor = if (isSystemInDarkTheme()) Color.White.copy(alpha = 0.15f) else Color.Black.copy(alpha = 0.15f)
    Column(
        modifier
            .width(SHELF_CARD_WIDTH)
            .clickable { onClick(mood, mood.displayName) },
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(shape)
                .border(1.dp, borderColor, shape),
        ) {
            SonataMoodAnimatedArt(
                moodTitle = mood.displayName,
                moodIndex = mood.ordinal,
                modifier = Modifier.fillMaxSize(),
            )
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 8.dp, end = 8.dp)
                    .height(11.dp)
                    .aspectRatio(1f),
            )
        }
        Spacer(Modifier.height(10.dp))
        Text(
            text = mood.displayName,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

private data class CollagePalette(val top: Color, val bottom: Color, val eyebrow: Color)

private val CollagePalettes = listOf(
    CollagePalette(Color(0xFF8F3F00), Color(0xFFFF6A0F), Color(0xFFFFC48A)),
    CollagePalette(Color(0xFF2D8A20), Color(0xFF4FD22C), Color(0xFFCBFFA3)),
    CollagePalette(Color(0xFF173E9E), Color(0xFF2F7BFF), Color(0xFFC0D8FF)),
    CollagePalette(Color(0xFF4A1E99), Color(0xFF8A4BFF), Color(0xFFDCC8FF)),
    CollagePalette(Color(0xFF0B6E6E), Color(0xFF14B8A6), Color(0xFFB5FFF3)),
    CollagePalette(Color(0xFF8E1426), Color(0xFFF0364F), Color(0xFFFFC4CC)),
)

@Composable
fun ArtistCollageArt(
    images: List<String>,
    title: String,
    subtitle: String = "",
    eyebrow: String = "FEATURING",
    modifier: Modifier = Modifier,
    showBrandLogo: Boolean = true,
) {
    val palette = CollagePalettes[abs(title.hashCode()) % CollagePalettes.size]
    val shape = RoundedCornerShape(12.dp)
    val w = 252.dp
    val h = 336.dp
    Box(
        modifier
            .width(w)
            .height(h)
            .clip(shape)
            .background(Brush.verticalGradient(listOf(palette.top, palette.bottom))),
    ) {
        val slots = when (images.size) {
            0 -> emptyList()
            1 -> listOf(Triple(0.5f, 0.34f, 0.66f))
            2 -> listOf(Triple(0.40f, 0.28f, 0.60f), Triple(0.72f, 0.52f, 0.36f))
            else -> listOf(
                Triple(0.37f, 0.257f, 0.594f),
                Triple(0.76f, 0.48f, 0.333f),
                Triple(0.457f, 0.608f, 0.29f),
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
                    .padding(top = 12.dp, end = 12.dp)
                    .height(15.dp)
                    .aspectRatio(1f),
            )
        }
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
        ) {
            Text(
                text = eyebrow,
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp, lineHeight = 16.sp),
                color = palette.eyebrow,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 17.sp,
                    lineHeight = 21.sp,
                    fontWeight = FontWeight.SemiBold,
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
private val ReleaseBottomScrim = Brush.verticalGradient(
    0.42f to Color.Transparent,
    0.66f to Color.Black.copy(alpha = 0.56f),
    0.84f to Color.Black.copy(alpha = 0.86f),
    1.0f to Color.Black.copy(alpha = 0.96f),
)

@Composable
fun ReleaseArt(
    artworkUrl: String?,
    title: String,
    subtitle: String = "",
    eyebrow: String = "NEW RELEASE",
    modifier: Modifier = Modifier,
    showBrandLogo: Boolean = false,
    canvasMedia: CanvasMedia? = null,
    canvasActive: Boolean = false,
) {
    val coverUrl = remember(artworkUrl) { artworkUrl.croppedAt(CARD_ART_PX) }
    val coverRequest = rememberArtworkRequest(coverUrl, CARD_ART_PX, CARD_ART_PX)
    Box(
        modifier
            .width(252.dp)
            .height(336.dp)
            .clip(ReleaseCardShape)
            .background(Color.Black),
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
                .background(ReleaseBottomScrim),
        )
        if (showBrandLogo) {
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 12.dp, end = 12.dp)
                    .height(15.dp)
                    .aspectRatio(1f),
            )
        }
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
        ) {
            Text(
                text = eyebrow,
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp, lineHeight = 16.sp),
                color = Color.White.copy(alpha = 0.62f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 17.sp,
                    lineHeight = 21.sp,
                    fontWeight = FontWeight.SemiBold,
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

private const val MOOD_STRIPE_FRACTION = 0.18f

private class MoodSleeveTone(val stripe: Color, val placeholder: Color, val duotone: ColorFilter)

@Composable
fun MoodGenreSleeve(
    title: String,
    thumbnailUrl: String? = null,
    stripeColor: Long? = null,
    modifier: Modifier = Modifier,
    titleStyle: TextStyle = MaterialTheme.typography.titleMedium,
) {
    val tone = remember(stripeColor, title) {
        val base = stripeColor?.let { Color(it.toInt()) } ?: fallbackSleeveColor(title)
        val stripe = lerp(base, Color.Black, 0.08f)
        val shadow = lerp(base, Color.Black, 0.68f)
        val highlight = lerp(base, Color.White, 0.32f)
        MoodSleeveTone(stripe, lerp(shadow, highlight, 0.45f), duotoneFilter(shadow, highlight))
    }
    Box(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .background(tone.stripe),
    ) {
        SonataMoodAnimatedArt(
            moodTitle = title,
            moodIndex = (title.hashCode() and 0x7FFF_FFFF),
            modifier = Modifier.fillMaxSize(),
        )
        Box(
            Modifier
                .matchParentSize()
                .background(Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.28f), 0.72f to Color.Transparent)),
        )
        Text(
            text = title,
            style = titleStyle.copy(
                shadow = Shadow(Color.Black.copy(alpha = 0.35f), offset = Offset(0f, 1f), blurRadius = 6f),
            ),
            fontWeight = FontWeight.Bold,
            color = Color.White,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .align(Alignment.TopStart)
                .padding(horizontal = 14.dp, vertical = 12.dp),
        )
    }
}

private fun duotoneFilter(shadow: Color, highlight: Color): ColorFilter {
    fun channel(from: Float, to: Float): FloatArray {
        val span = to - from
        return floatArrayOf(span * 0.299f, span * 0.587f, span * 0.114f, 0f, from * 255f)
    }
    return ColorFilter.colorMatrix(
        ColorMatrix(
            channel(shadow.red, highlight.red) +
                channel(shadow.green, highlight.green) +
                channel(shadow.blue, highlight.blue) +
                floatArrayOf(0f, 0f, 0f, 1f, 0f),
        ),
    )
}

private fun fallbackSleeveColor(title: String): Color = when ((title.hashCode() and Int.MAX_VALUE) % 8) {
    0 -> Color(0xFFCC6A55)
    1 -> Color(0xFFC07A92)
    2 -> Color(0xFF9C8AC0)
    3 -> Color(0xFF8090C8)
    4 -> Color(0xFFD0A060)
    5 -> Color(0xFF6A88B0)
    6 -> Color(0xFF5FA896)
    else -> Color(0xFFB87060)
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
    holder: String = "SONATA LISTENER",
    memberSince: String? = "09/26",
    onClick: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val cardShape = remember { RoundedCornerShape(20.dp) }
    val artPalette = rememberArtworkPalette(artworkUrl, dark = true, animated = false)
    Box(
        modifier = modifier
            .aspectRatio(1.586f)
            .shadow(16.dp, cardShape, clip = false)
            .clip(cardShape)
            .clickable(onClick = onClick),
    ) {
        MeshGradientBackground(
            palette = artPalette.toMeshPalette(),
            trackKey = artworkUrl ?: label,
            blurRadius = 34.dp,
            animated = false,
        )
        Box(
            Modifier
                .fillMaxSize()
                .background(
                    Brush.radialGradient(
                        colors = listOf(Color.White.copy(alpha = 0.14f), Color.Transparent),
                        radius = 650f,
                    ),
                )
                .background(
                    Brush.verticalGradient(
                        0.0f to Color.Black.copy(alpha = 0.18f),
                        0.55f to Color.Black.copy(alpha = 0.30f),
                        1.0f to Color.Black.copy(alpha = 0.55f),
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
                    text = "YOUR LISTENING EXPERIENCE",
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
                        text = holder.ifBlank { "SONATA LISTENER" }.uppercase(Locale.getDefault()),
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
                            text = "MEMBER\nSINCE",
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
    val height by animateDpAsState(
        targetValue = if (dragging) activeHeight else idleHeight,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessMediumLow,
        ),
        label = "thinSliderHeight",
    )
    val sweep = if (loading) {
        rememberInfiniteTransition(label = "thinSliderSheen").animateFloat(
            initialValue = 0f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(1100, easing = LinearEasing)),
            label = "sheen",
        ).value
    } else {
        0f
    }

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(activeHeight + 22.dp)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    dragging = true
                    onValueChange((down.position.x / size.width).coerceIn(0f, 1f))
                    while (true) {
                        val event = awaitPointerEvent()
                        val pointer = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!pointer.pressed) {
                            pointer.consume()
                            break
                        }
                        if (pointer.positionChanged()) {
                            onValueChange((pointer.position.x / size.width).coerceIn(0f, 1f))
                            pointer.consume()
                        }
                    }
                    dragging = false
                    onValueChangeFinished?.invoke()
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(height),
        ) {
            val radius = CornerRadius(size.height / 2f)
            drawRoundRect(color = inactiveColor, cornerRadius = radius)
            if (loading) {
                val band = size.width * 0.34f
                val startX = -band + sweep * (size.width + band * 2f)
                drawRoundRect(
                    brush = Brush.horizontalGradient(
                        colors = listOf(Color.Transparent, activeColor.copy(alpha = 0.65f), Color.Transparent),
                        startX = startX,
                        endX = startX + band,
                    ),
                    cornerRadius = radius,
                )
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
            0.0f to Color.Transparent,
            0.25f to Color.White.copy(alpha = 0.25f),
            0.50f to Color.White,
            0.75f to Color.White.copy(alpha = 0.25f),
            1.0f to Color.Transparent,
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
    val light = MaterialTheme.colorScheme.surface.luminanceCompat() > 0.5f
    val trackColor = Color(0xFF767680).copy(alpha = if (light) 0.12f else 0.24f)
    val pillColor = if (light) Color.White else Color(0xFF3A3A3C)
    val labelColor = if (light) Color.Black else Color.White
    val reduceAnimation = LocalReduceAnimation.current
    val selected = selectedIndex.takeIf { it in options.indices }
    val trackShape = remember { SquirclePillShape() }
    val containerColor = if (light) trackColor else Color(0xFF1C1C1E)
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

private fun Color.luminanceCompat(): Float = ColorUtils.calculateLuminance(toArgb()).toFloat()

@Composable
fun BottomFadeScrim(
    modifier: Modifier = Modifier,
    withMiniPlayer: Boolean = false,
    pageColor: Color = MaterialTheme.colorScheme.background,
) {
    val inset = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val height by animateDpAsState(
        targetValue = inset + if (withMiniPlayer) 254.dp else 180.dp,
        animationSpec = tween(220),
        label = "bottomScrimHeight",
    )
    val brush = remember(pageColor) {
        Brush.verticalGradient(
            colorStops = Array(16) { i ->
                val t = i / 15f
                t to pageColor.copy(alpha = EaseInCubic.transform(t))
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
    topHeight: Dp = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + 52.dp,
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
    placeholder: String = "Artists, Songs, Lyrics, and More",
    modifier: Modifier = Modifier,
) {
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
            contentDescription = "Search",
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
                    contentDescription = "Clear search",
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
    tint: Color = Color.White,
) {
    val haptics = LocalHapticFeedback.current
    val discAlpha by animateFloatAsState(
        targetValue = if (active) 0.34f else 0.18f,
        label = "glyphDisc",
    )
    Box(
        modifier = modifier
            .size(34.dp)
            .clip(CircleShape)
            .background(Color.White.copy(alpha = discAlpha))
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


// =============================================================================
// 8. Dynamic Astronomical Moon Logo & 120fps Swept Karaoke Lyrics
// =============================================================================

/** Interactive visual states supported by [DynamicMoonLogo]. */
enum class MoonAppearanceMode {
    Live,
    BloodMoon,
    WaxingCrescent,
    FullMoon,
    ;

    fun next(): MoonAppearanceMode = when (this) {
        Live -> BloodMoon
        BloodMoon -> WaxingCrescent
        WaxingCrescent -> FullMoon
        FullMoon -> Live
    }
}

private const val SYNODIC_MONTH_MS = 29.530588853 * 86_400_000.0
private const val NEW_MOON_EPOCH_MS = 947_182_440_000L

private data class LunarObserver(
    val latitudeDeg: Float,
    val longitudeDeg: Float,
)

private fun resolvePassiveObserver(context: Context): LunarObserver {
    val hasCoarse = ContextCompat.checkSelfPermission(
        context,
        Manifest.permission.ACCESS_COARSE_LOCATION,
    ) == PackageManager.PERMISSION_GRANTED
    val hasFine = ContextCompat.checkSelfPermission(
        context,
        Manifest.permission.ACCESS_FINE_LOCATION,
    ) == PackageManager.PERMISSION_GRANTED

    if (hasCoarse || hasFine) {
        val loc = runCatching {
            val lm = context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager
            lm?.getLastKnownLocation(LocationManager.NETWORK_PROVIDER)
                ?: lm?.getLastKnownLocation(LocationManager.PASSIVE_PROVIDER)
        }.getOrNull()
        if (loc != null) {
            return LunarObserver(
                latitudeDeg = loc.latitude.toFloat().coerceIn(-89f, 89f),
                longitudeDeg = loc.longitude.toFloat().coerceIn(-180f, 180f),
            )
        }
    }

    val tz = TimeZone.getDefault()
    val tzLon = ((tz.rawOffset / 3_600_000.0) * 15.0).toFloat().coerceIn(-180f, 180f)
    val idLower = tz.id.lowercase(Locale.ROOT)
    val fallbackLat = if (
        "australia" in idLower || "argentina" in idLower || "sao_paulo" in idLower ||
        "santiago" in idLower || "johannesburg" in idLower || "auckland" in idLower
    ) {
        -33f
    } else {
        40f
    }
    return LunarObserver(latitudeDeg = fallbackLat, longitudeDeg = tzLon)
}

private fun liveLunarPhase(nowMs: Long = System.currentTimeMillis()): Float {
    val raw = ((nowMs - NEW_MOON_EPOCH_MS) / SYNODIC_MONTH_MS) % 1.0
    return (if (raw < 0.0) raw + 1.0 else raw).toFloat()
}

private fun parallacticTiltDegrees(observer: LunarObserver, nowMs: Long): Float {
    val utcHours = ((nowMs / 3_600_000.0) % 24.0).toFloat()
    val localHourAngleRad = ((utcHours + observer.longitudeDeg / 15f - 12f) * (PI / 12.0)).toFloat()
    val latRad = Math.toRadians(observer.latitudeDeg.toDouble()).toFloat()
    val baseOrientation = if (observer.latitudeDeg < 0f) 156f else -24f
    val hourSwing = 28f * sin(localHourAngleRad) * cos(latRad)
    return baseOrientation + hourSwing
}

/**
 * Astronomical Moon emblem computing live lunar phase, geographic parallactic tilt,
 * subtle earthshine craters, and an interactive Blood Moon total lunar eclipse mode.
 */
@Composable
fun DynamicMoonLogo(
    modifier: Modifier = Modifier,
    initialMode: MoonAppearanceMode = MoonAppearanceMode.Live,
    interactive: Boolean = true,
    onModeChange: ((MoonAppearanceMode) -> Unit)? = null,
) {
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    var mode by remember(initialMode) { mutableStateOf(initialMode) }
    val nowMs = remember { System.currentTimeMillis() }
    val observer = remember(context) { resolvePassiveObserver(context) }

    val targetPhase = when (mode) {
        MoonAppearanceMode.Live -> liveLunarPhase(nowMs)
        MoonAppearanceMode.BloodMoon -> 0.5f
        MoonAppearanceMode.WaxingCrescent -> 0.17f
        MoonAppearanceMode.FullMoon -> 0.5f
    }
    val tiltDeg = remember(observer, nowMs) { parallacticTiltDegrees(observer, nowMs) }
    val animatedPhase by animateFloatAsState(
        targetValue = targetPhase,
        animationSpec = tween(durationMillis = 480, easing = FastOutSlowInEasing),
        label = "moonPhase",
    )
    val bloodBlend by animateFloatAsState(
        targetValue = if (mode == MoonAppearanceMode.BloodMoon) 1f else 0f,
        animationSpec = tween(durationMillis = 520, easing = FastOutSlowInEasing),
        label = "bloodMoonBlend",
    )
    val litPath = remember { Path() }
    val clipCircle = remember { Path() }

    Box(
        modifier = modifier
            .aspectRatio(1f)
            .clip(CircleShape)
            .then(
                if (interactive) {
                    Modifier.clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                    ) {
                        val next = mode.next()
                        mode = next
                        haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onModeChange?.invoke(next)
                    }
                } else {
                    Modifier
                },
            ),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val radius = size.minDimension * 0.44f
            val center = Offset(size.width / 2f, size.height / 2f)

            // Penumbral / atmospheric halo.
            val haloColor = lerp(
                Color(0xFFD6E0FF).copy(alpha = 0.22f),
                Color(0xFFFF5A36).copy(alpha = 0.48f),
                bloodBlend,
            )
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(haloColor, Color.Transparent),
                    center = center,
                    radius = size.minDimension * 0.5f,
                ),
                radius = size.minDimension * 0.5f,
                center = center,
            )

            rotate(degrees = tiltDeg, pivot = center) {
                // Dark side (earthshine) disc.
                val earthshine = lerp(Color(0xFF1E2233), Color(0xFF3B060A), bloodBlend)
                drawCircle(color = earthshine, radius = radius, center = center)

                // Illuminated limb geometry parameterized by phase 0..1.
                val phaseAngle = animatedPhase * 2f * PI.toFloat()
                val cosPhase = cos(phaseAngle)
                val waxing = animatedPhase <= 0.5f
                val rx = abs(cosPhase) * radius

                litPath.reset()
                clipCircle.reset()
                clipCircle.addOval(Rect(center, radius))

                // Half-disc on the lit side + terminator ellipse signed by cos(phase).
                val sweepStart = if (waxing) -90f else 90f
                litPath.arcTo(
                    rect = Rect(center.x - radius, center.y - radius, center.x + radius, center.y + radius),
                    startAngleDegrees = sweepStart,
                    sweepAngleDegrees = 180f,
                    forceMoveTo = true,
                )
                val steps = 36
                for (i in 0..steps) {
                    val t = i / steps.toFloat()
                    val y = if (waxing) center.y + radius - 2f * radius * t else center.y - radius + 2f * radius * t
                    val ny = ((y - center.y) / radius).coerceIn(-1f, 1f)
                    val xOffset = sqrt((1f - ny * ny).coerceAtLeast(0f)) * rx
                    val x = if (waxing) {
                        if (cosPhase >= 0f) center.x + xOffset else center.x - xOffset
                    } else {
                        if (cosPhase >= 0f) center.x - xOffset else center.x + xOffset
                    }
                    litPath.lineTo(x, y)
                }
                litPath.close()

                val litBrush = if (bloodBlend > 0.01f) {
                    Brush.radialGradient(
                        0.0f to lerp(Color.White, Color(0xFFFF5A36), bloodBlend),
                        0.55f to lerp(Color(0xFFF3F6FF), Color(0xFFB81D24), bloodBlend),
                        1.0f to lerp(Color(0xFFD2DAEF), Color(0xFF3B060A), bloodBlend),
                        center = Offset(center.x - radius * 0.18f, center.y - radius * 0.18f),
                        radius = radius * 1.25f,
                    )
                } else {
                    Brush.radialGradient(
                        0.0f to Color.White,
                        0.75f to Color(0xFFF3F6FF),
                        1.0f to Color(0xFFD6DFF5),
                        center = Offset(center.x - radius * 0.2f, center.y - radius * 0.2f),
                        radius = radius * 1.2f,
                    )
                }
                drawPath(path = litPath, brush = litBrush)

                // Subtle maria / crater shading masked to the lunar disc.
                val craterTint = lerp(
                    Color(0xFF8C97B8).copy(alpha = 0.20f),
                    Color(0xFF280305).copy(alpha = 0.34f),
                    bloodBlend,
                )
                drawCircle(
                    color = craterTint,
                    radius = radius * 0.22f,
                    center = Offset(center.x - radius * 0.26f, center.y - radius * 0.18f),
                )
                drawCircle(
                    color = craterTint,
                    radius = radius * 0.15f,
                    center = Offset(center.x + radius * 0.18f, center.y + radius * 0.22f),
                )
                drawCircle(
                    color = craterTint.copy(alpha = craterTint.alpha * 0.7f),
                    radius = radius * 0.11f,
                    center = Offset(center.x - radius * 0.05f, center.y + radius * 0.34f),
                )
            }
        }
    }
}

/**
 * Steered monotonic lyric clock: advances smoothly on frame timestamps and gently steers
 * toward dated ExoPlayer readings rather than snapping or stuttering on polling jitter.
 * Explicit seeks / track changes (`discontinuity`) jump immediately.
 */
private class SteeredLyricClock(startMs: Long) {
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

    fun frame(nowMs: Double, reportedMs: Long, sampledAtMs: Double, discontinuity: Long): Long {
        val safeReported = reportedMs.coerceAtLeast(0L)
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
            displayedMs = (anchorMs + (nowMs - anchorAtMs)).coerceAtLeast(0.0)
            return displayedMs.toLong()
        }

        val dt = (nowMs - lastNowMs).coerceIn(0.0, 100.0)
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

        val target = anchorMs + (nowMs - anchorAtMs).coerceIn(0.0, 500.0)
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
): State<Long> {
    val initialPos = remember(trackKey, clock) {
        Snapshot.withoutReadObservation { clock.positionMs.coerceAtLeast(0L) }
    }
    val smoothClock = remember(trackKey) { mutableLongStateOf(initialPos) }
    val engine = remember(trackKey) { SteeredLyricClock(initialPos) }
    val minFrameStepNanos = if (fpsLimit >= 120) 7_000_000L else 15_500_000L

    LaunchedEffect(clock, trackKey, isPlaying, isBuffering, minFrameStepNanos) {
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
    1f to Color.Transparent,
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

    // Synthesize smooth proportional spans across the exact source line duration when word timestamps are absent.
    val matches = KaraokeWordRegex.findAll(text).toList()
    if (matches.isEmpty()) {
        return listOf(KaraokeWordSpan(0, text.length, lineStartMs, safeLineEnd))
    }
    val totalWeight = matches.sumOf { it.value.length.coerceAtLeast(1) }.toFloat()
    val duration = (safeLineEnd - lineStartMs).coerceAtLeast(250L)
    var accWeight = 0f
    return matches.map { m ->
        val w = m.value.length.coerceAtLeast(1).toFloat()
        val sMs = lineStartMs + ((accWeight / totalWeight) * duration).roundToInt().toLong()
        accWeight += w
        val eMs = lineStartMs + ((accWeight / totalWeight) * duration).roundToInt().toLong()
        KaraokeWordSpan(m.range.first, m.range.last + 1, sMs, maxOf(eMs, sMs + 50L))
    }
}

/** Exact source lyric timing offset (0 ms keeps playback strictly aligned to provider timestamps). */
const val LYRIC_SYNC_LEAD_MS = 0L

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
                    color = color.copy(alpha = 0.30f),
                    offset = Offset.Zero,
                    blurRadius = 16f,
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
