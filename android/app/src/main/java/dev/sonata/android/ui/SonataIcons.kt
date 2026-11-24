/*
 * Custom vector icon catalog for Sonata Android.
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

    val Add: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_add",
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
                moveTo(12f, 5f); lineTo(12f, 19f)
                moveTo(5f, 12f); lineTo(19f, 12f)
            }
        }.build()
    }

    val Pin: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_pin",
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
                moveTo(9f, 4.5f); lineTo(15f, 4.5f)
                moveTo(10f, 4.5f); lineTo(9.2f, 10.2f); lineTo(6.5f, 13.4f); lineTo(17.5f, 13.4f); lineTo(14.8f, 10.2f); lineTo(14f, 4.5f)
                moveTo(12f, 13.4f); lineTo(12f, 20.5f)
            }
        }.build()
    }

    val PinFilled: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_pin_filled",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                fill = stroke,
                stroke = stroke,
                strokeLineWidth = 1.6f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                moveTo(9f, 4.5f); lineTo(15f, 4.5f); lineTo(14.6f, 10.2f); lineTo(17.5f, 13.4f)
                lineTo(6.5f, 13.4f); lineTo(9.4f, 10.2f); close()
            }
            path(
                stroke = stroke,
                strokeLineWidth = 2.0f,
                strokeLineCap = StrokeCap.Round,
            ) {
                moveTo(12f, 13.4f); lineTo(12f, 20.5f)
            }
        }.build()
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

    val FastForward: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_fast_forward",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(
                stroke = stroke,
                strokeLineWidth = 1.6f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                fill = stroke,
            ) {
                moveTo(4.2f, 6.4f); lineTo(11.6f, 12f); lineTo(4.2f, 17.6f); close()
                moveTo(12.2f, 6.4f); lineTo(19.6f, 12f); lineTo(12.2f, 17.6f); close()
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

    val Edit: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_edit",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(
                "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25z" +
                    "M20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
            ),
            fill = stroke,
        ).build()
    }

    val Image: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_image",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(
                "M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2z" +
                    "M8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z"
            ),
            fill = stroke,
        ).build()
    }

    val Fullscreen: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_fullscreen",
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
                moveTo(4f, 9f); lineTo(4f, 4f); lineTo(9f, 4f)
                moveTo(15f, 4f); lineTo(20f, 4f); lineTo(20f, 9f)
                moveTo(20f, 15f); lineTo(20f, 20f); lineTo(15f, 20f)
                moveTo(9f, 20f); lineTo(4f, 20f); lineTo(4f, 15f)
            }
        }.build()
    }

    val FullscreenExit: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_fullscreen_exit",
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
                moveTo(4f, 9f); lineTo(9f, 9f); lineTo(9f, 4f)
                moveTo(20f, 9f); lineTo(15f, 9f); lineTo(15f, 4f)
                moveTo(15f, 20f); lineTo(15f, 15f); lineTo(20f, 15f)
                moveTo(9f, 20f); lineTo(9f, 15f); lineTo(4f, 15f)
            }
        }.build()
    }

    val Tune: ImageVector by lazy {
        ImageVector.Builder(
            name = "sonata_tune",
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
                moveTo(4f, 7f); lineTo(14f, 7f); moveTo(18f, 7f); lineTo(20f, 7f)
                moveTo(16f, 5f); lineTo(16f, 9f)
                moveTo(4f, 12f); lineTo(7f, 12f); moveTo(11f, 12f); lineTo(20f, 12f)
                moveTo(9f, 10f); lineTo(9f, 14f)
                moveTo(4f, 17f); lineTo(13f, 17f); moveTo(17f, 17f); lineTo(20f, 17f)
                moveTo(15f, 15f); lineTo(15f, 19f)
            }
        }.build()
    }
}

