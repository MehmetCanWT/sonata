/*
 * Astronomical Moon phase logo for Sonata Android.
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

private fun resolvePassiveObserver(@Suppress("UNUSED_PARAMETER") context: Context): LunarObserver {
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
