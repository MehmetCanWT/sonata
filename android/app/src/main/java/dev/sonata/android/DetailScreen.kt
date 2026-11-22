/*
 * Album, Playlist, and Mood detail screen and shared shelf/track row cards.
 */
package dev.sonata.android

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.CornerBasedShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import okhttp3.Dispatcher
import okhttp3.OkHttpClient
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.media3.common.Player
import coil3.ImageLoader
import coil3.SingletonImageLoader
import coil3.compose.AsyncImage
import coil3.disk.DiskCache
import coil3.disk.directory
import coil3.memory.MemoryCache
import coil3.request.allowHardware
import coil3.request.crossfade
import coil3.size.Precision
import dev.sonata.android.backdrop.backdrops.LayerBackdrop
import dev.sonata.android.backdrop.backdrops.layerBackdrop
import dev.sonata.android.backdrop.backdrops.rememberLayerBackdrop
import dev.sonata.android.floatingtabbar.FloatingTabBar
import dev.sonata.android.floatingtabbar.FloatingTabBarDefaults
import dev.sonata.android.floatingtabbar.FloatingTabBarInlineBehavior
import dev.sonata.android.floatingtabbar.FloatingTabBarScrollConnection
import dev.sonata.android.floatingtabbar.rememberFloatingTabBarScrollConnection
import dev.sonata.android.ui.ArtistCollageArt
import dev.sonata.android.ui.BAR_GUTTER
import dev.sonata.android.ui.BottomFadeScrim
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.DynamicMoonLogo
import dev.sonata.android.ui.FLOATING_BAR_MAX_WIDTH
import dev.sonata.android.ui.FullArtworkBlurBackdrop
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.HERO_FADE_FRACTION
import dev.sonata.android.ui.HERO_FADE_STOPS
import dev.sonata.android.ui.LYRIC_SYNC_LEAD_MS
import dev.sonata.android.ui.LosslessLabel
import dev.sonata.android.ui.LyricsSkeleton
import dev.sonata.android.ui.MarqueeText
import dev.sonata.android.ui.MeshGradientBackground
import dev.sonata.android.ui.MeshPalette
import dev.sonata.android.ui.MoodGenreSleeve
import dev.sonata.android.ui.MoodTile
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.PLAYER_ART_PX
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.ReleaseArt
import dev.sonata.android.ui.ReplayCreditCard
import dev.sonata.android.ui.SHELF_CARD_WIDTH
import dev.sonata.android.ui.SearchField
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.SonataShimmerHost
import dev.sonata.android.ui.SonataTheme
import dev.sonata.android.ui.theme.LunariaColors
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.drawThemeAtmosphericBackdrop
import dev.sonata.android.ui.SweptKaraokeLyricText
import dev.sonata.android.ui.SystemBarIcons
import dev.sonata.android.ui.ThinSlider
import dev.sonata.android.ui.TopFadeBlur
import dev.sonata.android.ui.TopPickMood
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.artworkMemoryCacheKey
import dev.sonata.android.ui.detailSkeleton
import dev.sonata.android.ui.feedSkeleton
import dev.sonata.android.ui.recentlyPlayedSkeleton
import dev.sonata.android.ui.rememberArtworkPalette
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberLayoutBoundsHolder
import dev.sonata.android.ui.rememberSmoothLyricClock
import dev.sonata.android.ui.toMeshPalette
import dev.sonata.android.ui.topPicksSkeleton
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.SonataThumbnailInterceptor
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.ui.components.AddToPlaylistSheet
import dev.sonata.android.ui.components.HeldContextMenu
import dev.sonata.android.ui.components.HeldContextTarget
import dev.sonata.android.ui.components.ScrollActivityTracker
import dev.sonata.android.ui.components.SonataArtistCircleCard
import dev.sonata.android.ui.components.SonataDiscoveryStationArt
import dev.sonata.android.ui.components.SonataEditorialCard
import dev.sonata.android.ui.components.SonataLastPlayedSquareCard
import dev.sonata.android.ui.components.SonataPersonalStationArt
import dev.sonata.android.ui.components.SonataSplitHeroHeader
import dev.sonata.android.ui.components.TrackOptionsMenu
import dev.sonata.android.ui.components.UserMenuSheet
import dev.sonata.android.ui.lyrics.EnhancedLyricsPanel
import dev.sonata.android.ui.player.AudioControlsSheet
import dev.sonata.android.ui.player.AudioPipelineModal
import dev.sonata.android.ui.player.BakedArtworkMeshBackdrop
import dev.sonata.android.ui.player.CanvasVideoSurface
import dev.sonata.android.ui.player.NowPlayingOptionsMenu
import dev.sonata.android.ui.player.SleepTimerSheet
import dev.sonata.android.ui.screens.ArtistReleaseItem
import dev.sonata.android.ui.screens.ArtistScreen
import dev.sonata.android.ui.screens.ArtistScreenData
import dev.sonata.android.ui.screens.ArtistSimilarItem
import dev.sonata.android.ui.screens.ArtistTrackItem
import dev.sonata.android.ui.screens.DownloadsManagerSheet
import dev.sonata.android.ui.screens.RadioMixesScreen
import dev.sonata.android.ui.screens.SonataSettingsScreen
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import dev.sonata.android.ui.theme.SonataTabIconSize
import dev.sonata.android.ui.trackColumnWidth
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import java.io.File
import kotlin.math.roundToInt


@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun CompactTrackRow(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    leadingIndex: Int? = null,
    onArtworkWash: Boolean = false,
    accentColor: Color = SonataAccent,
    onOptionsClick: ((Rect?) -> Unit)? = null,
    onLongClick: ((Rect?) -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    val rowBounds = rememberLayoutBoundsHolder()
    val optionsBounds = rememberLayoutBoundsHolder()
    val primaryInk = if (onArtworkWash) Color.White else MaterialTheme.colorScheme.onBackground
    val secondaryInk = if (onArtworkWash) Color.White.copy(alpha = 0.72f) else MaterialTheme.colorScheme.onSurfaceVariant
    val thumbBorder = if (onArtworkWash) Color.White.copy(alpha = 0.16f) else MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(Shape10)
            .onGloballyPositioned { rowBounds.coordinates = it }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(rowBounds.boundsInRoot())
                    }
                },
            )
            .padding(vertical = 6.dp, horizontal = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (leadingIndex != null) {
            Text(
                text = leadingIndex.toString(),
                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                color = if (isCurrent) accentColor else secondaryInk,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(26.dp),
            )
            Spacer(Modifier.width(6.dp))
        }
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(Shape10)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, thumbBorder, Shape10),
            contentAlignment = Alignment.Center,
        ) {
            val thumb = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
            val thumbRequest = rememberArtworkRequest(thumb, ROW_ART_PX, ROW_ART_PX)
            if (thumbRequest != null) {
                AsyncImage(
                    model = thumbRequest,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = secondaryInk,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        if (isCurrent) {
            Icon(
                imageVector = SonataIcons.GraphicEq,
                contentDescription = if (isPlaying) "Playing" else "Paused",
                tint = accentColor,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 15.5.sp,
                    fontWeight = if (isCurrent) FontWeight.SemiBold else FontWeight.Medium,
                ),
                color = if (isCurrent) accentColor else primaryInk,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                if (track.id in LocalDownloadedTrackIds.current) {
                    Icon(
                        imageVector = SonataIcons.Download,
                        contentDescription = "Downloaded",
                        tint = accentColor,
                        modifier = Modifier.size(13.dp),
                    )
                }
                Text(
                    text = track.artist,
                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp),
                    color = secondaryInk,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
        }
        Box(
            modifier = Modifier
                .size(32.dp)
                .clip(CircleShape)
                .onGloballyPositioned { optionsBounds.coordinates = it }
                .then(
                    if (onOptionsClick != null) Modifier.clickable { onOptionsClick(optionsBounds.boundsInRoot()) } else Modifier,
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = SonataIcons.MoreVert,
                contentDescription = "Track options",
                tint = secondaryInk,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
    modifier: Modifier = Modifier.width(SHELF_CARD_WIDTH),
) {
    val haptics = LocalHapticFeedback.current
    val cardBounds = rememberLayoutBoundsHolder()
    Column(
        modifier = modifier
            .onGloballyPositioned { cardBounds.coordinates = it }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds.boundsInRoot())
                    }
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(LocalLunariaColors.current.cardShape)
                .background(LocalLunariaColors.current.eclipseCard)
                .border(0.75.dp, LocalLunariaColors.current.cardStroke, LocalLunariaColors.current.cardShape),
            contentAlignment = Alignment.Center,
        ) {
            val thumb = remember(card.thumbnailUrl) { card.thumbnailUrl.artworkAt(CARD_ART_PX) }
            val thumbRequest = rememberArtworkRequest(thumb, 360, 360)
            if (thumbRequest != null) {
                AsyncImage(
                    model = thumbRequest,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = LocalLunariaColors.current.onSurfaceSecondary,
                    modifier = Modifier.size(34.dp),
                )
            }
            if (isCurrent && isPlaying) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color.Black.copy(alpha = 0.42f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.GraphicEq,
                        contentDescription = null,
                        tint = SonataAccent,
                        modifier = Modifier.size(28.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = card.title,
            style = MaterialTheme.typography.titleSmall.copy(
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (card.subtitle.isNotBlank()) {
            Text(
                text = card.subtitle,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun VideoShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
    modifier: Modifier = Modifier.width(248.dp),
) {
    val haptics = LocalHapticFeedback.current
    val cardBounds = rememberLayoutBoundsHolder()
    Column(
        modifier = modifier
            .onGloballyPositioned { cardBounds.coordinates = it }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds.boundsInRoot())
                    }
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(16f / 9f)
                .clip(LocalLunariaColors.current.cardShape)
                .background(LocalLunariaColors.current.eclipseCard)
                .border(0.75.dp, LocalLunariaColors.current.cardStroke, LocalLunariaColors.current.cardShape),
            contentAlignment = Alignment.Center,
        ) {
            val videoThumbRequest = rememberArtworkRequest(card.thumbnailUrl, 480, 270)
            if (videoThumbRequest != null) {
                AsyncImage(
                    model = videoThumbRequest,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            }
            if (isCurrent && isPlaying) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color.Black.copy(alpha = 0.45f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.GraphicEq,
                        contentDescription = null,
                        tint = SonataAccent,
                        modifier = Modifier.size(28.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = card.title,
            style = MaterialTheme.typography.titleSmall.copy(
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (card.subtitle.isNotBlank()) {
            Text(
                text = card.subtitle,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
internal fun DetailScreen(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    isDownloading: Boolean = false,
    allDownloaded: Boolean = false,
    onPlayTrack: (AndroidTrack) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Int, Rect?) -> Unit = { _, _, _ -> },
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
    onDownloadAll: (() -> Unit)? = null,
    onMoreOptions: (() -> Unit)? = null,
    onOpenArtist: ((String) -> Unit)? = null,
) {
    val themeTokens = LocalLunariaColors.current
    val rawPalette = rememberArtworkPalette(
        artworkUrl = state.collectionCoverUrl,
        dark = themeTokens.isDark,
        artPx = HEADER_ART_PX,
    )
    val palette = remember(rawPalette, themeTokens.designTheme, themeTokens.isDark) {
        if (!themeTokens.isDark) {
            rawPalette.copy(
                background = androidx.compose.ui.graphics.lerp(
                    rawPalette.background,
                    themeTokens.eclipseBase,
                    0.76f,
                ),
                wash = androidx.compose.ui.graphics.lerp(
                    rawPalette.wash,
                    themeTokens.eclipseSurface,
                    0.66f,
                ),
                elevated = androidx.compose.ui.graphics.lerp(
                    rawPalette.elevated,
                    themeTokens.eclipseCard,
                    0.72f,
                ),
                accent = themeTokens.petalLavender,
                onBackground = themeTokens.onSurfacePrimary,
                onBackgroundVariant = themeTokens.onSurfaceSecondary,
                divider = themeTokens.cardStroke,
            )
        } else {
            rawPalette.copy(
                background = androidx.compose.ui.graphics.lerp(
                    rawPalette.background,
                    themeTokens.eclipseBase,
                    0.60f,
                ),
                wash = androidx.compose.ui.graphics.lerp(
                    rawPalette.wash,
                    themeTokens.eclipseSurface,
                    0.46f,
                ),
                accent = themeTokens.petalLavender,
                onBackground = themeTokens.onSurfacePrimary,
                onBackgroundVariant = themeTokens.onSurfaceSecondary,
                divider = themeTokens.cardStroke,
            )
        }
    }
    // Lush full-page Apple Music color wash — stays tinted all the way down the tracklist.
    val midWash = remember(palette) {
        androidx.compose.ui.graphics.lerp(palette.wash, palette.background, 0.35f)
    }
    val bottomWash = remember(palette) {
        androidx.compose.ui.graphics.lerp(palette.background, palette.wash, 0.28f)
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    0.0f to palette.wash,
                    0.48f to midWash,
                    1.0f to bottomWash,
                ),
            ),
    ) {
        SonataShimmerHost {
            val detailTopInset = contentPadding.calculateTopPadding()
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(
                    top = 0.dp,
                    bottom = contentPadding.calculateBottomPadding(),
                ),
            ) {
                item(key = "detail_header", contentType = "detail_header") {
                    val subtitleArtist = state.collectionSubtitle?.takeIf {
                        it.isNotBlank() && state.collectionKindLabel != "Offline Library"
                    }
                    SonataSplitHeroHeader(
                        title = state.collectionTitle.orEmpty(),
                        subtitle = state.collectionSubtitle,
                        kindLabel = state.collectionKindLabel,
                        year = state.collectionYear,
                        trackCount = state.collectionTracks.size,
                        artworkUrl = state.collectionCoverUrl,
                        accentColor = palette.accent,
                        contentColor = palette.onBackground,
                        washColor = palette.wash,
                        topInset = detailTopInset,
                        onPlayAll = onPlayAll,
                        onShuffleAll = onShuffleAll,
                        onDownloadAll = onDownloadAll?.takeIf { state.collectionTracks.isNotEmpty() },
                        isDownloading = isDownloading,
                        allDownloaded = allDownloaded,
                        onMoreOptions = onMoreOptions,
                        onOpenArtist = subtitleArtist?.let { artist -> { onOpenArtist?.invoke(artist) } },
                    )
                }

                if (state.loading && state.collectionTracks.isEmpty()) {
                    detailSkeleton(trackRows = 8)
                } else {
                    itemsIndexed(
                        items = state.collectionTracks,
                        key = { idx, track -> "${track.id}_$idx" },
                        contentType = { _, _ -> "detail_track" },
                    ) { idx, track ->
                        val activeId = state.activeTrack?.id
                        val current = activeId != null && track.id == activeId
                        Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                            CompactTrackRow(
                                track = track,
                                isCurrent = current,
                                isPlaying = current && state.isPlaying,
                                leadingIndex = idx + 1,
                                onArtworkWash = themeTokens.isDark,
                                accentColor = palette.accent,
                                onClick = { onPlayTrack(track) },
                                onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                onLongClick = { bounds -> onTrackLongClick(track, idx + 1, bounds) },
                            )
                            if (themeTokens.isMoonflower) {
                                HorizontalDivider(
                                    color = palette.divider,
                                    modifier = Modifier.padding(start = 84.dp),
                                )
                            }
                        }
                    }

                    if (state.collectionTracks.isNotEmpty()) {
                        item(key = "detail_footer", contentType = "detail_footer") {
                            val totalMinutes = remember(state.collectionTracks) {
                                val totalSecs = state.collectionTracks.sumOf { it.durationSecs?.toLong() ?: 0L }
                                ((totalSecs + 30L) / 60L).coerceAtLeast(0L)
                            }
                            val summaryLine = remember(state.collectionTracks.size, totalMinutes) {
                                val countLabel = if (state.collectionTracks.size == 1) "1 song" else "${state.collectionTracks.size} songs"
                                if (totalMinutes > 0L) "$countLabel, $totalMinutes minutes" else countLabel
                            }
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = PAGE_GUTTER, vertical = 20.dp),
                                verticalArrangement = Arrangement.spacedBy(3.dp),
                            ) {
                                state.collectionYear?.takeIf { it.isNotBlank() }?.let { year ->
                                    Text(
                                        text = year,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = palette.onBackgroundVariant,
                                    )
                                }
                                Text(
                                    text = summaryLine,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = palette.onBackgroundVariant,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

