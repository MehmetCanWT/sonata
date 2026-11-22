/*
 * Shared shimmer transition host and skeleton loaders for Sonata Android.
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
    val tokens = LocalLunariaColors.current
    val baseFill = remember(tokens.onSurfacePrimary) { tokens.onSurfacePrimary.copy(alpha = 0.10f) }
    val bandColors = remember {
        listOf(
            Color.Transparent,
            tokens.onSurfacePrimary.copy(alpha = 0.18f),
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

