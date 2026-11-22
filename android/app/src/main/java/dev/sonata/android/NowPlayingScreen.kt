/*
 * Full-screen Now Playing player (Portrait & Landscape), Queue, Changelog dialog, and share helpers.
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
import dev.sonata.android.ui.rememberSafeTopInset
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


internal data class NowPlayingExtras(
    val trackId: String = "",
    val settings: SonataSettingsState = SonataSettingsState(),
    val currentCanvas: CanvasMedia? = null,
    val isTrackDownloaded: Boolean = false,
    val activeTrackDownload: ActiveDownload? = null,
    val sleepTimerRemaining: Int? = null,
    val activeLyricsSource: String = "Synced",
    val trackLyricOffsetMs: Long = 0L,
    val onDownloadTrack: () -> Unit = {},
    val onOpenAudioControls: () -> Unit = {},
    val onOpenSleepTimer: () -> Unit = {},
    val onOpenAudioPipeline: () -> Unit = {},
    val onToggleLyricsTranslation: () -> Unit = {},
    val onToggleLyricsRomanization: () -> Unit = {},
    val onStepLyricOffsetMs: (Long) -> Unit = {},
)

internal val LocalDownloadedTrackIds = androidx.compose.runtime.compositionLocalOf<Set<String>> { emptySet() }

internal val LocalNowPlayingExtras = androidx.compose.runtime.compositionLocalOf { NowPlayingExtras() }

@Composable
internal fun NowPlayingScreen(
    track: AndroidTrack,
    state: SonataUiState,
    settings: SonataSettingsState,
    currentCanvas: CanvasMedia?,
    audioPipelineSummary: String?,
    isTrackDownloaded: Boolean,
    activeTrackDownload: ActiveDownload?,
    sleepTimerRemaining: Int?,
    activeLyricsSource: String,
    trackLyricOffsetMs: Long,
    clock: PlaybackClockState,
    onToggleLyrics: () -> Unit,
    onToggleStar: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onSeekTo: (Long) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onJumpToQueueIndex: (Int) -> Unit,
    onDownloadTrack: () -> Unit,
    onOpenAudioControls: () -> Unit,
    onOpenSleepTimer: () -> Unit,
    onOpenAudioPipeline: () -> Unit,
    onToggleLyricsTranslation: () -> Unit,
    onToggleLyricsRomanization: () -> Unit,
    onStepLyricOffsetMs: (Long) -> Unit,
    onResetLyricOffsetMs: () -> Unit,
    onShareInApp: () -> Unit,
    onStartRadio: () -> Unit,
    onPlayNext: () -> Unit,
    onAddToQueue: () -> Unit,
    onOpenAlbum: () -> Unit,
    onOpenArtist: () -> Unit,
    onOpenSettings: () -> Unit,
    onAddToPlaylist: () -> Unit = {},
    onDismiss: () -> Unit,
) {
    val extras = remember(
        track.id,
        settings,
        isTrackDownloaded,
        activeTrackDownload,
        sleepTimerRemaining,
        activeLyricsSource,
        trackLyricOffsetMs,
    ) {
        NowPlayingExtras(
            trackId = track.id,
            settings = settings,
            currentCanvas = currentCanvas,
            isTrackDownloaded = isTrackDownloaded,
            activeTrackDownload = activeTrackDownload,
            sleepTimerRemaining = sleepTimerRemaining,
            activeLyricsSource = activeLyricsSource,
            trackLyricOffsetMs = trackLyricOffsetMs,
            onDownloadTrack = onDownloadTrack,
            onOpenAudioControls = onOpenAudioControls,
            onOpenSleepTimer = onOpenSleepTimer,
            onOpenAudioPipeline = onOpenAudioPipeline,
            onToggleLyricsTranslation = onToggleLyricsTranslation,
            onToggleLyricsRomanization = onToggleLyricsRomanization,
            onStepLyricOffsetMs = onStepLyricOffsetMs,
        )
    }
    CompositionLocalProvider(LocalNowPlayingExtras provides extras) {
    var showQueue by remember { mutableStateOf(false) }
    var showPlayerOptions by remember { mutableStateOf(false) }
    var dragOffsetY by remember { mutableFloatStateOf(0f) }
    var scrubbing by remember { mutableStateOf(false) }

    if (showQueue && !showPlayerOptions) {
        BackHandler { showQueue = false }
    }

    if (showPlayerOptions) {
        BackHandler { showPlayerOptions = false }
    }

    val panelOpen = state.showLyrics || showQueue
    val isLiked = track.id in state.likedTrackIds
    val palette = rememberArtworkPalette(
        artworkUrl = track.thumbnailUrl,
        dark = true,
        artPx = CARD_ART_PX,
    )

    // Reads PlaybackClockState strictly inside withFrameNanos / snapshotFlow -> 0 recompositions here.
    val smoothClockMs = rememberSmoothLyricClock(
        clock = clock,
        isPlaying = state.isPlaying,
        isBuffering = state.isBuffering,
        fpsLimit = settings.fpsLimit,
        trackKey = track.id,
    )
    val density = LocalDensity.current
    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    val screenHeight = LocalConfiguration.current.screenHeightDp.dp
    val playerGutter = 30.dp
    val playerMaxWidth = 560.dp
    val headerHeight = 60.dp
    val artTitleGap = 20.dp
    val controlGapSpreadMax = 24.dp
    val topHandleStripHeight = 24.dp

    val isLandscapePlayer = screenWidth > screenHeight && screenWidth >= 560.dp
    val compactLandscape = isLandscapePlayer && screenHeight < 440.dp
    // All portrait viewports (phones and portrait tablets) use the full-bleed hero artwork style.
    val playerFillsWindow = !isLandscapePlayer
    val showPortraitHero = !panelOpen && !isLandscapePlayer
    val heroUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(PLAYER_ART_PX) }
    val heroPlaceholderKey = remember(track.thumbnailUrl) {
        artworkMemoryCacheKey(track.thumbnailUrl.artworkAt(CARD_ART_PX), 360, 360)
            ?: artworkMemoryCacheKey(track.thumbnailUrl.artworkAt(ROW_ART_PX), ROW_ART_PX, ROW_ART_PX)
    }
    val heroRequest = rememberArtworkRequest(heroUrl, 1024, 1024, placeholderKey = heroPlaceholderKey)
    val hasArtwork = heroRequest != null

    // Measured portrait column and artwork slot dimensions via onSizeChanged.
    var portraitColumnWidthPx by remember { mutableIntStateOf(0) }
    var portraitColumnHeightPx by remember { mutableIntStateOf(0) }
    var portraitSlotHeightPx by remember { mutableIntStateOf(0) }

    val statusBarTop = rememberSafeTopInset()
    val portraitColWidthDp = if (portraitColumnWidthPx > 0) {
        with(density) { portraitColumnWidthPx.toDp() }
    } else {
        (screenWidth - playerGutter * 2).coerceAtMost(playerMaxWidth)
    }
    val portraitColHeightDp = if (portraitColumnHeightPx > 0) {
        with(density) { portraitColumnHeightPx.toDp() }
    } else {
        (screenHeight - statusBarTop - 24.dp).coerceAtLeast(480.dp)
    }
    val portraitRoomy = (portraitColHeightDp - 309.dp).coerceAtLeast(240.dp)
    val portraitWantArt = minOf(
        portraitColWidthDp,
        (portraitRoomy - headerHeight - artTitleGap).coerceAtLeast(180.dp),
    )
    val portraitSlack = (portraitRoomy - portraitWantArt - headerHeight - artTitleGap).coerceAtLeast(0.dp)
    val controlSpread = if (!panelOpen && !isLandscapePlayer) {
        portraitSlack.coerceAtMost(controlGapSpreadMax * 2)
    } else {
        0.dp
    }
    val portraitTopGap = 12.dp + controlSpread / 2
    val portraitBottomGap = 16.dp + controlSpread / 2
    val portraitSlotHeightDp = if (portraitSlotHeightPx > 0) {
        with(density) { portraitSlotHeightPx.toDp() }
    } else {
        (portraitRoomy - controlSpread).coerceAtLeast(240.dp)
    }
    val heroHeight = (statusBarTop + topHandleStripHeight + 8.dp + portraitSlotHeightDp - headerHeight + 16.dp)
        .coerceIn(260.dp, screenHeight * 0.74f)

    val coverScale by animateFloatAsState(
        targetValue = when {
            scrubbing -> 0.94f
            state.isPlaying -> 1.0f
            else -> 0.88f
        },
        animationSpec = tween(
            durationMillis = 500,
            easing = CubicBezierEasing(0.215f, 0.61f, 0.355f, 1f),
        ),
        label = "nowPlayingCoverScale",
    )

    val nowPlayingBackdrop = rememberLayerBackdrop()

    Box(
        modifier = Modifier
            .fillMaxSize()
            .offset { IntOffset(0, dragOffsetY.roundToInt().coerceAtLeast(0)) }
            .pointerInput(onDismiss) {
                detectVerticalDragGestures(
                    onVerticalDrag = { change, amount ->
                        change.consume()
                        dragOffsetY = (dragOffsetY + amount).coerceAtLeast(0f)
                    },
                    onDragEnd = {
                        if (dragOffsetY > 220f) onDismiss()
                        dragOffsetY = 0f
                    },
                    onDragCancel = { dragOffsetY = 0f },
                )
            },
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(palette.background)
                .layerBackdrop(nowPlayingBackdrop),
        ) {

        // Smooth crossfade between selected Now Playing backdrop and FullArtworkBlurBackdrop (lyrics/queue mode)
        Crossfade(
            targetState = panelOpen to settings.playerBackdropMode,
            animationSpec = tween(320, easing = FastOutSlowInEasing),
            label = "nowPlayingBgCrossfade",
        ) { (blurred, backdropMode) ->
            if (blurred || backdropMode == PlayerBackdropMode.BLURRED_COVER) {
                Box(Modifier.fillMaxSize()) {
                    FullArtworkBlurBackdrop(
                        artworkUrl = track.thumbnailUrl.artworkAt(CARD_ART_PX),
                        modifier = Modifier.fillMaxSize(),
                    )
                    Box(
                        Modifier
                            .fillMaxSize()
                            .background(Color.Black.copy(alpha = if (blurred) 0.28f else 0.18f)),
                    )
                }
            } else if (backdropMode == PlayerBackdropMode.ARTWORK_MESH) {
                BakedArtworkMeshBackdrop(
                    artworkUrl = track.thumbnailUrl.artworkAt(CARD_ART_PX),
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                MeshGradientBackground(
                    palette = palette.toMeshPalette(),
                    paused = !state.isPlaying || !settings.animatedArtwork || settings.reduceAnimation,
                    trackKey = track.id,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }

        // Subtle dark readability gradient under lower controls
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(NowPlayingBottomScrim),
        )

        // Full-bleed edge-to-edge hero cover at y = 0 dissolving smoothly into mesh backdrop
        // (covers phones and portrait tablets).
        AnimatedVisibility(
            visible = showPortraitHero,
            enter = fadeIn(tween(260)),
            exit = fadeOut(tween(220)),
            modifier = Modifier.align(Alignment.TopCenter),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(heroHeight)
                    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                    .drawWithCache {
                        val fadeBrush = Brush.verticalGradient(
                            colorStops = HERO_FADE_STOPS,
                            startY = size.height * (1f - HERO_FADE_FRACTION),
                            endY = size.height,
                        )
                        onDrawWithContent {
                            drawContent()
                            drawRect(brush = fadeBrush, blendMode = BlendMode.DstIn)
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                if (heroRequest != null) {
                    AsyncImage(
                        model = heroRequest,
                        contentDescription = track.title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(palette.wash),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.45f),
                            modifier = Modifier.size(84.dp),
                        )
                    }
                }
                if (settings.animatedArtwork && currentCanvas != null) {
                    CanvasVideoSurface(
                        media = currentCanvas,
                        isVisible = showPortraitHero && state.isPlaying,
                        preferPortrait = true,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(
                    if (isLandscapePlayer) {
                        Modifier
                            .windowInsetsPadding(WindowInsets.safeDrawing)
                            .padding(bottom = if (compactLandscape) 8.dp else 20.dp)
                    } else {
                        Modifier
                            .padding(top = statusBarTop)
                            .navigationBarsPadding()
                            .padding(horizontal = playerGutter)
                            .onSizeChanged {
                                portraitColumnWidthPx = it.width
                                portraitColumnHeightPx = it.height
                            }
                    },
                ),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(topHandleStripHeight),
                contentAlignment = Alignment.TopCenter,
            ) {
                Box(
                    modifier = Modifier
                        .padding(top = 8.dp)
                        .width(38.dp)
                        .height(5.dp)
                        .clip(CircleShape)
                        .background(Color.White.copy(alpha = 0.45f))
                        .clickable(onClick = onDismiss),
                )
            }

            if (isLandscapePlayer) {
                NowPlayingLandscapeLayout(
                    track = track,
                    state = state,
                    clock = clock,
                    showQueue = showQueue,
                    isLiked = isLiked,
                    elevatedColor = palette.elevated,
                    heroRequest = heroRequest,
                    hasArtwork = hasArtwork,
                    coverScale = coverScale,
                    smoothClockMs = smoothClockMs,
                    screenWidth = screenWidth,
                    screenHeight = screenHeight,
                    compactLandscape = compactLandscape,
                    onScrubbingChanged = { scrubbing = it },
                    onSeekFraction = onSeekFraction,
                    onToggleLyrics = {
                        if (showQueue) showQueue = false
                        onToggleLyrics()
                    },
                    onToggleQueue = {
                        val openingQueue = !showQueue
                        showQueue = openingQueue
                        if (openingQueue && state.showLyrics) onToggleLyrics()
                    },
                    onClosePanels = {
                        if (showQueue) showQueue = false
                        if (state.showLyrics) onToggleLyrics()
                    },
                    onOpenLyrics = {
                        showQueue = false
                        if (!state.showLyrics) onToggleLyrics()
                    },
                    onToggleStar = onToggleStar,
                    onTogglePlayPause = onTogglePlayPause,
                    onNext = onNext,
                    onPrevious = onPrevious,
                    onToggleShuffle = onToggleShuffle,
                    onToggleRepeat = onToggleRepeat,
                    onSeekTo = onSeekTo,
                    onJumpToQueueIndex = onJumpToQueueIndex,
                    onOpenSettings = { showPlayerOptions = true },
                    modifier = Modifier.weight(1f),
                )
            } else {
                NowPlayingPortraitLayout(
                    track = track,
                    state = state,
                    clock = clock,
                    showQueue = showQueue,
                    panelOpen = panelOpen,
                    isLiked = isLiked,
                    elevatedColor = palette.elevated,
                    heroRequest = heroRequest,
                    hasArtwork = hasArtwork,
                    coverScale = coverScale,
                    smoothClockMs = smoothClockMs,
                    playerMaxWidth = playerMaxWidth,
                    headerHeight = headerHeight,
                    artTitleGap = artTitleGap,
                    playerFillsWindow = playerFillsWindow,
                    portraitWantArt = portraitWantArt,
                    portraitTopGap = portraitTopGap,
                    portraitBottomGap = portraitBottomGap,
                    onPortraitSlotHeightChanged = { portraitSlotHeightPx = it },
                    onScrubbingChanged = { scrubbing = it },
                    onSeekFraction = onSeekFraction,
                    onToggleLyrics = {
                        if (showQueue) showQueue = false
                        onToggleLyrics()
                    },
                    onToggleQueue = {
                        val openingQueue = !showQueue
                        showQueue = openingQueue
                        if (openingQueue && state.showLyrics) onToggleLyrics()
                    },
                    onClosePanels = {
                        if (showQueue) showQueue = false
                        if (state.showLyrics) onToggleLyrics()
                    },
                    onOpenLyrics = {
                        showQueue = false
                        if (!state.showLyrics) onToggleLyrics()
                    },
                    onToggleStar = onToggleStar,
                    onTogglePlayPause = onTogglePlayPause,
                    onNext = onNext,
                    onPrevious = onPrevious,
                    onToggleShuffle = onToggleShuffle,
                    onToggleRepeat = onToggleRepeat,
                    onSeekTo = onSeekTo,
                    onJumpToQueueIndex = onJumpToQueueIndex,
                    onOpenSettings = { showPlayerOptions = true },
                    modifier = Modifier.weight(1f),
                )
            }
        }
        }

        // Now Playing `⋯` Options floating Liquid Glass menu (sibling OUTSIDE nowPlayingBackdrop — zero RenderNode cycle)
        NowPlayingOptionsMenu(
            visible = showPlayerOptions,
            track = track,
            isLiked = isLiked,
            isDownloaded = isTrackDownloaded,
            activeDownload = activeTrackDownload,
            sleepTimerRemainingMinutes = sleepTimerRemaining,
            lyricOffsetMs = trackLyricOffsetMs,
            romanizationEnabled = settings.lyricsRomanizationEnabled,
            translationEnabled = settings.lyricsTranslationEnabled,
            pipelineSummary = audioPipelineSummary,
            backdrop = nowPlayingBackdrop,
            onDismiss = { showPlayerOptions = false },
            onToggleLike = onToggleStar,
            onAddToPlaylist = onAddToPlaylist,
            onShareInApp = onShareInApp,
            onOpenAudioControls = onOpenAudioControls,
            onOpenAudioPipeline = onOpenAudioPipeline,
            onDownloadTrack = onDownloadTrack,
            onOpenSleepTimer = onOpenSleepTimer,
            onStepLyricOffsetMs = onStepLyricOffsetMs,
            onResetLyricOffset = onResetLyricOffsetMs,
            onToggleRomanization = onToggleLyricsRomanization,
            onToggleTranslation = onToggleLyricsTranslation,
            onStartRadio = onStartRadio,
            onPlayNext = onPlayNext,
            onAddToQueue = onAddToQueue,
            onOpenAlbum = onOpenAlbum,
            onOpenArtist = onOpenArtist,
        )
    }
    }
}

@Composable
internal fun NowPlayingActionRowBlock(
    state: SonataUiState,
    showQueue: Boolean,
    compact: Boolean = false,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    val extras = LocalNowPlayingExtras.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ActionCutoutTile(
            active = state.showLyrics,
            onClick = onToggleLyrics,
        ) {
            Icon(
                imageVector = SonataIcons.LyricsQuote,
                contentDescription = "Lyrics",
                tint = Color.White,
                modifier = Modifier.size(26.dp),
            )
        }

        if (showQueue) {
            Row(
                modifier = Modifier
                    .clip(SquirclePillShape())
                    .background(Color.White.copy(alpha = 0.14f))
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(
                    onClick = onToggleShuffle,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.Shuffle,
                        contentDescription = "Shuffle",
                        tint = if (state.shuffleEnabled) SonataAccent else Color.White.copy(alpha = 0.72f),
                        modifier = Modifier.size(18.dp),
                    )
                }
                IconButton(
                    onClick = onToggleRepeat,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        imageVector = if (state.repeatMode == Player.REPEAT_MODE_ONE) {
                            SonataIcons.RepeatOne
                        } else {
                            SonataIcons.Repeat
                        },
                        contentDescription = "Repeat",
                        tint = if (state.repeatMode != Player.REPEAT_MODE_OFF) {
                            SonataAccent
                        } else {
                            Color.White.copy(alpha = 0.72f)
                        },
                        modifier = Modifier.size(18.dp),
                    )
                }
                Box(
                    modifier = Modifier.size(32.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.Infinity,
                        contentDescription = "Autoplay",
                        tint = Color.White.copy(alpha = 0.72f),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        } else {
            ActionCutoutTile(
                active = extras.settings.lyricsTranslationEnabled,
                onClick = extras.onToggleLyricsTranslation,
            ) {
                Icon(
                    imageVector = SonataIcons.Translate,
                    contentDescription = if (extras.settings.lyricsTranslationEnabled) {
                        "Disable Lyrics Translation"
                    } else {
                        "Translate Lyrics"
                    },
                    tint = if (extras.settings.lyricsTranslationEnabled) SonataAccent else Color.White,
                    modifier = Modifier.size(23.dp),
                )
            }
        }

        ActionCutoutTile(
            active = showQueue,
            onClick = onToggleQueue,
        ) {
            Icon(
                imageVector = SonataIcons.Queue,
                contentDescription = "Queue",
                tint = Color.White,
                modifier = Modifier.size(26.dp),
            )
        }
    }

    Spacer(Modifier.height(if (compact) 4.dp else 8.dp))
}

/**
 * Narrowest leaf composable that subscribes to [PlaybackClockState.positionMs] & [PlaybackClockState.durationMs].
 * Keeps all 5Hz playback ticking completely isolated from NowPlayingScreen, layouts, and tabs.
 */
@Composable
internal fun NowPlayingProgressSection(
    clock: PlaybackClockState,
    isBuffering: Boolean,
    isUpgradingQuality: Boolean,
    streamBadge: String,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
) {
    var scrubbing by remember { mutableStateOf(false) }
    var scrubFraction by remember { mutableFloatStateOf(0f) }

    val durationMs = clock.durationMs
    val positionMs = clock.positionMs
    val playbackFraction = if (durationMs > 0L) {
        (positionMs.toFloat() / durationMs.toFloat()).coerceIn(0f, 1f)
    } else {
        0f
    }
    val displayedFraction = if (scrubbing) scrubFraction else playbackFraction
    val shownMs = (displayedFraction * durationMs).toLong()
    val remainingMs = (durationMs - shownMs).coerceAtLeast(0L)

    ThinSlider(
        value = displayedFraction,
        onValueChange = { frac ->
            if (!scrubbing) {
                scrubbing = true
                onScrubbingChanged(true)
            }
            scrubFraction = frac
        },
        onValueChangeFinished = {
            onSeekFraction(scrubFraction)
            scrubbing = false
            onScrubbingChanged(false)
        },
        loading = isBuffering,
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .offset(y = (-6).dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = formatDurationMs(shownMs),
                style = MaterialTheme.typography.labelMedium,
                color = Color.White.copy(alpha = 0.58f),
            )
            Text(
                text = "-${formatDurationMs(remainingMs)}",
                style = MaterialTheme.typography.labelMedium,
                color = Color.White.copy(alpha = 0.58f),
            )
        }
        val upgrading = isBuffering || isUpgradingQuality
        LosslessLabel(
            label = if (upgrading) {
                "Upgrading Quality"
            } else {
                streamBadge.ifBlank { "Lossless" }
            },
            pulsing = upgrading,
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 6.dp),
        )
    }
}

@Composable
internal fun NowPlayingScrubberAndTransportBlock(
    state: SonataUiState,
    clock: PlaybackClockState,
    showQueue: Boolean,
    includeActions: Boolean = true,
    compactTransport: Boolean = false,
    transportTopGap: Dp = 14.dp,
    transportBottomGap: Dp = 18.dp,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onPrevious: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    NowPlayingProgressSection(
        clock = clock,
        isBuffering = state.isBuffering,
        isUpgradingQuality = state.isUpgradingQuality,
        streamBadge = state.streamBadge,
        onScrubbingChanged = onScrubbingChanged,
        onSeekFraction = onSeekFraction,
    )

    if (transportTopGap > 0.dp) {
        Spacer(Modifier.height(transportTopGap))
    }

    val hasPrevious by remember(state.activeTrack, state.repeatMode, state.queueIndex, clock) {
        derivedStateOf { state.hasPrevious(clock.positionMs) }
    }
    val sideButtonSize = if (compactTransport) 44.dp else 64.dp
    val sideIconSize = if (compactTransport) 34.dp else 53.dp
    val playButtonSize = if (compactTransport) 52.dp else 88.dp
    val playIconSize = if (compactTransport) 36.dp else 54.dp

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceAround,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(
            onClick = onPrevious,
            enabled = hasPrevious,
            modifier = Modifier.size(sideButtonSize),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_previous),
                contentDescription = "Previous",
                tint = if (hasPrevious) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(sideIconSize)
                    .graphicsLayer { scaleY = 0.85f },
            )
        }

        if (state.isBuffering) {
            Box(
                modifier = Modifier.size(playButtonSize),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(
                    color = Color.White,
                    strokeWidth = if (compactTransport) 2.5.dp else 3.dp,
                    modifier = Modifier.size(if (compactTransport) 26.dp else 38.dp),
                )
            }
        } else {
            IconButton(
                onClick = onTogglePlayPause,
                modifier = Modifier.size(playButtonSize),
            ) {
                Icon(
                    painter = painterResource(
                        if (state.isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play
                    ),
                    contentDescription = if (state.isPlaying) "Pause" else "Play",
                    tint = Color.White,
                    modifier = Modifier.size(playIconSize),
                )
            }
        }

        IconButton(
            onClick = onNext,
            enabled = state.hasNext,
            modifier = Modifier.size(sideButtonSize),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_next),
                contentDescription = "Next",
                tint = if (state.hasNext) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(sideIconSize)
                    .graphicsLayer { scaleY = 0.85f },
            )
        }
    }

    if (includeActions) {
        if (transportBottomGap > 0.dp) {
            Spacer(Modifier.height(transportBottomGap))
        }
        NowPlayingActionRowBlock(
            state = state,
            showQueue = showQueue,
            onToggleLyrics = onToggleLyrics,
            onToggleQueue = onToggleQueue,
            onToggleShuffle = onToggleShuffle,
            onToggleRepeat = onToggleRepeat,
            onOpenSettings = onOpenSettings,
        )
    }
}

@Composable
internal fun NowPlayingLandscapeLayout(
    track: AndroidTrack,
    state: SonataUiState,
    clock: PlaybackClockState,
    showQueue: Boolean,
    isLiked: Boolean,
    elevatedColor: Color,
    heroRequest: coil3.request.ImageRequest?,
    hasArtwork: Boolean,
    coverScale: Float,
    smoothClockMs: State<Long>,
    screenWidth: Dp,
    screenHeight: Dp,
    compactLandscape: Boolean,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onClosePanels: () -> Unit,
    onOpenLyrics: () -> Unit,
    onToggleStar: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onSeekTo: (Long) -> Unit,
    onJumpToQueueIndex: (Int) -> Unit,
    onOpenSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val density = LocalDensity.current
    val activePane = when {
        showQueue -> 2
        state.showLyrics -> 1
        else -> 0
    }
    val colPadding = if (compactLandscape) 20.dp else 30.dp
    var sleeveSlotWidthPx by remember { mutableIntStateOf(0) }
    var sleeveSlotHeightPx by remember { mutableIntStateOf(0) }

    Row(
        modifier = modifier
            .widthIn(max = 1100.dp)
            .fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
                .padding(horizontal = colPadding),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .onSizeChanged {
                        sleeveSlotWidthPx = it.width
                        sleeveSlotHeightPx = it.height
                    },
                contentAlignment = Alignment.Center,
            ) {
                val measuredSizeDp = with(density) {
                    minOf(sleeveSlotWidthPx, sleeveSlotHeightPx).toDp()
                }
                val coverSize = if (measuredSizeDp > 100.dp) {
                    measuredSizeDp.coerceIn(180.dp, 420.dp)
                } else {
                    minOf(screenWidth * 0.40f, screenHeight * 0.66f).coerceIn(200.dp, 360.dp)
                }
                Box(
                    modifier = Modifier
                        .size(coverSize)
                        .graphicsLayer {
                            scaleX = coverScale
                            scaleY = coverScale
                            if (hasArtwork) {
                                shadowElevation = 18.dp.toPx()
                                ambientShadowColor = Color.Black.copy(alpha = 0.65f)
                                spotShadowColor = Color.Black.copy(alpha = 0.75f)
                            }
                            shape = Shape11
                            clip = true
                        }
                        .background(elevatedColor)
                        .border(0.5.dp, Color.White.copy(alpha = 0.08f), Shape11)
                        .clickable {
                            if (showQueue) onClosePanels() else onToggleLyrics()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    if (heroRequest != null) {
                        AsyncImage(
                            model = heroRequest,
                            contentDescription = track.title,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.45f),
                            modifier = Modifier.size(84.dp),
                        )
                    }
                    val extras = LocalNowPlayingExtras.current
                    if (extras.settings.animatedArtwork && extras.currentCanvas != null) {
                        CanvasVideoSurface(
                            media = extras.currentCanvas,
                            isVisible = state.isPlaying,
                            preferPortrait = false,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
            }

            Column(
                modifier = Modifier
                    .widthIn(max = 360.dp)
                    .fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                if (activePane != 0) {
                    Spacer(Modifier.height(8.dp))
                    NowPlayingScrubberAndTransportBlock(
                        state = state,
                        clock = clock,
                        showQueue = showQueue,
                        includeActions = false,
                        compactTransport = true,
                        transportTopGap = 4.dp,
                        onScrubbingChanged = onScrubbingChanged,
                        onSeekFraction = onSeekFraction,
                        onPrevious = onPrevious,
                        onTogglePlayPause = onTogglePlayPause,
                        onNext = onNext,
                        onToggleLyrics = onToggleLyrics,
                        onToggleQueue = onToggleQueue,
                        onToggleShuffle = onToggleShuffle,
                        onToggleRepeat = onToggleRepeat,
                        onOpenSettings = onOpenSettings,
                    )
                    Spacer(Modifier.height(6.dp))
                } else {
                    Spacer(Modifier.height(if (compactLandscape) 8.dp else 14.dp))
                }
                NowPlayingActionRowBlock(
                    state = state,
                    showQueue = showQueue,
                    compact = activePane != 0 || compactLandscape,
                    onToggleLyrics = onToggleLyrics,
                    onToggleQueue = onToggleQueue,
                    onToggleShuffle = onToggleShuffle,
                    onToggleRepeat = onToggleRepeat,
                    onOpenSettings = onOpenSettings,
                )
            }
        }

        AnimatedContent(
            targetState = activePane,
            transitionSpec = {
                fadeIn(tween(220, delayMillis = 90)) togetherWith fadeOut(tween(140))
            },
            label = "landscapeTabletPane",
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
                .padding(horizontal = colPadding),
        ) { pane ->
            when (pane) {
                1 -> NowPlayingLyricsPanel(
                    lyrics = state.lyrics,
                    lyricsLoading = state.lyricsLoading,
                    clockMs = smoothClockMs,
                    clock = clock,
                    onSeekTo = onSeekTo,
                )

                2 -> NowPlayingQueuePanel(
                    queue = state.queue,
                    queueIndex = state.queueIndex,
                    isPlaying = state.isPlaying,
                    onSelectIndex = onJumpToQueueIndex,
                )

                else -> Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier
                            .widthIn(max = 460.dp)
                            .fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                MarqueeText(
                                    text = track.title,
                                    style = MaterialTheme.typography.headlineSmall.copy(
                                        fontSize = 24.sp,
                                        fontWeight = FontWeight.Bold,
                                    ),
                                    color = Color.White,
                                    enabled = state.isPlaying,
                                )
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    text = track.artist,
                                    style = MaterialTheme.typography.titleMedium.copy(
                                        fontSize = 17.sp,
                                        fontWeight = FontWeight.Normal,
                                    ),
                                    color = Color.White.copy(alpha = 0.62f),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            Spacer(Modifier.width(12.dp))
                            CircleGlyph(
                                icon = if (isLiked) SonataIcons.StarFilled else SonataIcons.Star,
                                contentDescription = "Favorite",
                                active = isLiked,
                                tint = if (isLiked) SonataAccent else Color.White,
                                onClick = onToggleStar,
                            )
                            Spacer(Modifier.width(10.dp))
                            CircleGlyph(
                                icon = SonataIcons.MoreHoriz,
                                contentDescription = "Options",
                                onClick = onOpenSettings,
                            )
                        }

                        Spacer(Modifier.height(if (compactLandscape) 2.dp else 10.dp))

                        CurrentLyricStrip(
                            trackId = track.id,
                            lyrics = state.lyrics,
                            lyricsLoading = state.lyricsLoading,
                            lyricsUnavailable = state.lyricsUnavailable,
                            clockMs = smoothClockMs,
                            clock = clock,
                            onClick = onOpenLyrics,
                        )

                        NowPlayingScrubberAndTransportBlock(
                            state = state,
                            clock = clock,
                            showQueue = showQueue,
                            includeActions = false,
                            transportTopGap = if (compactLandscape) 0.dp else 8.dp,
                            onScrubbingChanged = onScrubbingChanged,
                            onSeekFraction = onSeekFraction,
                            onPrevious = onPrevious,
                            onTogglePlayPause = onTogglePlayPause,
                            onNext = onNext,
                            onToggleLyrics = onToggleLyrics,
                            onToggleQueue = onToggleQueue,
                            onToggleShuffle = onToggleShuffle,
                            onToggleRepeat = onToggleRepeat,
                            onOpenSettings = onOpenSettings,
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun NowPlayingPortraitLayout(
    track: AndroidTrack,
    state: SonataUiState,
    clock: PlaybackClockState,
    showQueue: Boolean,
    panelOpen: Boolean,
    isLiked: Boolean,
    elevatedColor: Color,
    heroRequest: coil3.request.ImageRequest?,
    hasArtwork: Boolean,
    coverScale: Float,
    smoothClockMs: State<Long>,
    playerMaxWidth: Dp,
    headerHeight: Dp,
    artTitleGap: Dp,
    playerFillsWindow: Boolean,
    portraitWantArt: Dp,
    portraitTopGap: Dp,
    portraitBottomGap: Dp,
    onPortraitSlotHeightChanged: (Int) -> Unit,
    onScrubbingChanged: (Boolean) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onClosePanels: () -> Unit,
    onOpenLyrics: () -> Unit,
    onToggleStar: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onSeekTo: (Long) -> Unit,
    onJumpToQueueIndex: (Int) -> Unit,
    onOpenSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .widthIn(max = playerMaxWidth)
            .fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(top = 8.dp, bottom = 14.dp)
                .onSizeChanged { onPortraitSlotHeightChanged(it.height) },
        ) {
            AnimatedContent(
                targetState = panelOpen,
                transitionSpec = {
                    fadeIn(tween(240, easing = FastOutSlowInEasing)) togetherWith
                        fadeOut(tween(180, easing = FastOutSlowInEasing))
                },
                label = "heroToPanelTransition",
                modifier = Modifier.fillMaxSize(),
            ) { isPanelMode ->
                if (isPanelMode) {
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(top = 6.dp),
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(headerHeight),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            val thumbUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
                            val thumbRequest = rememberArtworkRequest(thumbUrl, ROW_ART_PX, ROW_ART_PX)
                            Box(
                                modifier = Modifier
                                    .size(54.dp)
                                    .clip(Shape8)
                                    .background(Color.White.copy(alpha = 0.1f))
                                    .border(0.5.dp, Color.White.copy(alpha = 0.18f), Shape8)
                                    .clickable(onClick = onClosePanels),
                                contentAlignment = Alignment.Center,
                            ) {
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
                                        tint = Color.White.copy(alpha = 0.65f),
                                    )
                                }
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                MarqueeText(
                                    text = track.title,
                                    style = MaterialTheme.typography.titleMedium.copy(
                                        fontSize = 18.sp,
                                        fontWeight = FontWeight.Bold,
                                    ),
                                    color = Color.White,
                                    enabled = state.isPlaying,
                                )
                                Text(
                                    text = track.artist,
                                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.5.sp),
                                    color = Color.White.copy(alpha = 0.72f),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            Spacer(Modifier.width(10.dp))
                            CircleGlyph(
                                icon = if (isLiked) SonataIcons.StarFilled else SonataIcons.Star,
                                contentDescription = "Favorite",
                                active = isLiked,
                                tint = if (isLiked) SonataAccent else Color.White,
                                onClick = onToggleStar,
                            )
                            Spacer(Modifier.width(8.dp))
                            CircleGlyph(
                                icon = SonataIcons.MoreHoriz,
                                contentDescription = "Options",
                                onClick = onOpenSettings,
                            )
                        }

                        Spacer(Modifier.height(10.dp))
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxWidth(),
                        ) {
                            if (showQueue) {
                                NowPlayingQueuePanel(
                                    queue = state.queue,
                                    queueIndex = state.queueIndex,
                                    isPlaying = state.isPlaying,
                                    onSelectIndex = onJumpToQueueIndex,
                                )
                            } else {
                                NowPlayingLyricsPanel(
                                    lyrics = state.lyrics,
                                    lyricsLoading = state.lyricsLoading,
                                    clockMs = smoothClockMs,
                                    clock = clock,
                                    onSeekTo = onSeekTo,
                                )
                            }
                        }
                    }
                } else {
                    Column(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.Bottom,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        if (!playerFillsWindow) {
                            Box(
                                modifier = Modifier
                                    .weight(1f)
                                    .fillMaxWidth(),
                                contentAlignment = Alignment.Center,
                            ) {
                                val portraitArtSize = portraitWantArt.coerceIn(260.dp, 460.dp)
                                Box(
                                    modifier = Modifier
                                        .size(portraitArtSize)
                                        .graphicsLayer {
                                            scaleX = coverScale
                                            scaleY = coverScale
                                            if (hasArtwork) {
                                                shadowElevation = 18.dp.toPx()
                                                ambientShadowColor = Color.Black.copy(alpha = 0.65f)
                                                spotShadowColor = Color.Black.copy(alpha = 0.75f)
                                            }
                                            shape = Shape12
                                            clip = true
                                        }
                                        .background(elevatedColor)
                                        .border(0.5.dp, Color.White.copy(alpha = 0.08f), Shape12),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    if (heroRequest != null) {
                                        AsyncImage(
                                            model = heroRequest,
                                            contentDescription = track.title,
                                            contentScale = ContentScale.Crop,
                                            modifier = Modifier.fillMaxSize(),
                                        )
                                    } else {
                                        Icon(
                                            imageVector = SonataIcons.MusicNote,
                                            contentDescription = null,
                                            tint = Color.White.copy(alpha = 0.45f),
                                            modifier = Modifier.size(84.dp),
                                        )
                                    }
                                }
                            }
                            Spacer(Modifier.height(artTitleGap))
                        }
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(headerHeight),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                MarqueeText(
                                    text = track.title,
                                    style = MaterialTheme.typography.headlineSmall.copy(
                                        fontSize = 21.sp,
                                        fontWeight = FontWeight.Bold,
                                    ),
                                    color = Color.White,
                                    enabled = state.isPlaying,
                                )
                                Spacer(Modifier.height(2.dp))
                                Text(
                                    text = track.artist,
                                    style = MaterialTheme.typography.titleMedium.copy(
                                        fontSize = 16.sp,
                                        fontWeight = FontWeight.Normal,
                                    ),
                                    color = Color.White.copy(alpha = 0.72f),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            Spacer(Modifier.width(12.dp))
                            CircleGlyph(
                                icon = if (isLiked) SonataIcons.StarFilled else SonataIcons.Star,
                                contentDescription = "Favorite",
                                active = isLiked,
                                tint = if (isLiked) SonataAccent else Color.White,
                                onClick = onToggleStar,
                            )
                            Spacer(Modifier.width(10.dp))
                            CircleGlyph(
                                icon = SonataIcons.MoreHoriz,
                                contentDescription = "Options",
                                onClick = onOpenSettings,
                            )
                        }
                    }
                }
            }
        }

        if (!panelOpen) {
            CurrentLyricStrip(
                trackId = track.id,
                lyrics = state.lyrics,
                lyricsLoading = state.lyricsLoading,
                lyricsUnavailable = state.lyricsUnavailable,
                clockMs = smoothClockMs,
                clock = clock,
                onClick = onOpenLyrics,
            )
        } else {
            Spacer(Modifier.height(8.dp))
        }

        NowPlayingScrubberAndTransportBlock(
            state = state,
            clock = clock,
            showQueue = showQueue,
            includeActions = true,
            transportTopGap = if (panelOpen) 12.dp else portraitTopGap,
            transportBottomGap = if (panelOpen) 14.dp else portraitBottomGap,
            onScrubbingChanged = onScrubbingChanged,
            onSeekFraction = onSeekFraction,
            onPrevious = onPrevious,
            onTogglePlayPause = onTogglePlayPause,
            onNext = onNext,
            onToggleLyrics = onToggleLyrics,
            onToggleQueue = onToggleQueue,
            onToggleShuffle = onToggleShuffle,
            onToggleRepeat = onToggleRepeat,
            onOpenSettings = onOpenSettings,
        )
    }
}

@Composable
internal fun ActionCutoutTile(
    active: Boolean,
    onClick: () -> Unit,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = Modifier
            .size(35.dp)
            .clip(RoundedCornerShape(7.dp))
            .background(if (active) Color.White.copy(alpha = 0.24f) else Color.Transparent)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
internal fun CurrentLyricStrip(
    trackId: String,
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    lyricsUnavailable: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    onClick: () -> Unit,
) {
    val durationMs = clock.durationMs
    var showUnavailable by remember(trackId) { mutableStateOf(false) }

    LaunchedEffect(trackId, lyricsUnavailable, lyricsLoading) {
        if (!lyricsLoading && lyricsUnavailable && lyrics.isEmpty()) {
            showUnavailable = true
            delay(3_000L)
            showUnavailable = false
        } else {
            showUnavailable = false
        }
    }

    val activeIndex by remember(lyrics) {
        derivedStateOf {
            val pos = clockMs.value + LYRIC_SYNC_LEAD_MS
            lyrics.indexOfLast { (it.timeSecs * 1000.0).toLong() <= pos }.coerceAtLeast(0)
        }
    }
    val stripAlpha by animateFloatAsState(
        targetValue = if (lyricsLoading || lyrics.isNotEmpty() || showUnavailable) 1f else 0f,
        animationSpec = tween(280),
        label = "lyricStripAlpha",
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(26.dp)
            .offset(y = 6.dp)
            .clipToBounds()
            .graphicsLayer { alpha = stripAlpha }
            .then(
                if (lyrics.isNotEmpty()) Modifier.clickable(onClick = onClick) else Modifier
            ),
        contentAlignment = Alignment.CenterStart,
    ) {
        when {
            lyricsLoading -> Text(
                text = "Loading lyrics...",
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.5.sp),
                color = Color.White.copy(alpha = 0.55f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )

            lyrics.isNotEmpty() -> {
                AnimatedContent(
                    targetState = activeIndex,
                    transitionSpec = {
                        fadeIn(tween(140, delayMillis = 80)) togetherWith fadeOut(tween(100))
                    },
                    label = "liveLyricStripLine",
                    modifier = Modifier
                        .fillMaxWidth()
                        .clipToBounds(),
                ) { idx ->
                    val activeLine = lyrics.getOrNull(idx)
                    if (activeLine != null) {
                        val cleanStripText = activeLine.text.trim()
                            .removePrefix("v2:").removePrefix("V2:")
                            .removePrefix("v1:").removePrefix("V1:")
                            .removePrefix("bg:").removePrefix("BG:")
                            .trim()
                        val lineStartMs = (activeLine.timeSecs * 1000.0).toLong().coerceAtLeast(0L)
                        val lineEndMs = lyrics.getOrNull(idx + 1)
                            ?.let { (it.timeSecs * 1000.0).toLong() }
                            ?.coerceAtLeast(lineStartMs + 400L)
                            ?: maxOf(durationMs, lineStartMs + 4_000L)
                        SweptKaraokeLyricText(
                            text = cleanStripText,
                            lineStartMs = lineStartMs,
                            lineEndMs = lineEndMs,
                            words = activeLine.words,
                            clockMs = clockMs,
                            style = MaterialTheme.typography.titleSmall.copy(
                                fontSize = 15.5.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                            dimAlpha = 0.42f,
                            uncommittedColor = Color(0xFF98989F).copy(alpha = 0.42f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }

            showUnavailable -> Text(
                text = "Lyrics not available",
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.5.sp),
                color = Color.White.copy(alpha = 0.55f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
internal fun NowPlayingLyricsPanel(
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    onSeekTo: (Long) -> Unit,
) {
    val extras = LocalNowPlayingExtras.current
    EnhancedLyricsPanel(
        trackId = extras.trackId,
        lyrics = lyrics,
        lyricsLoading = lyricsLoading,
        clockMs = clockMs,
        clock = clock,
        lyricsBlurEnabled = extras.settings.lyricsBlurEnabled,
        translationEnabled = extras.settings.lyricsTranslationEnabled,
        translationLang = extras.settings.lyricsTranslationLang,
        romanizationEnabled = extras.settings.lyricsRomanizationEnabled,
        offsetMs = extras.trackLyricOffsetMs,
        sourceLabel = extras.activeLyricsSource,
        onToggleTranslation = extras.onToggleLyricsTranslation,
        onToggleRomanization = extras.onToggleLyricsRomanization,
        onStepOffsetMs = extras.onStepLyricOffsetMs,
        onSeekTo = onSeekTo,
        modifier = Modifier.fillMaxSize(),
    )
}



/**
 * Leaf composable rendering the 3-dot instrumental interlude progress indicator.
 * Visibility is gated via `derivedStateOf` and dot lighting reads `clockMs.value` strictly
 * in the Canvas Draw phase so the parent lyric row never recomposes at 60/120fps.
 */
@Composable
internal fun InstrumentalBreakDots(
    lineStartMs: Long,
    nextLineStartMs: Long,
    cleanTextLength: Int,
    clockMs: State<Long>,
) {
    val breakStartMs = remember(lineStartMs, nextLineStartMs, cleanTextLength) {
        lineStartMs + minOf(
            (cleanTextLength.coerceAtLeast(3) * 65L).coerceIn(1_100L, 3_600L),
            ((nextLineStartMs - lineStartMs) * 55L / 100L),
        )
    }
    val visible by remember(breakStartMs, nextLineStartMs, clockMs) {
        derivedStateOf {
            val now = clockMs.value
            now >= breakStartMs && now < nextLineStartMs
        }
    }
    if (!visible) return

    Canvas(
        modifier = Modifier
            .padding(top = 12.dp, bottom = 4.dp)
            .width(46.dp)
            .height(10.dp),
    ) {
        val nowMs = clockMs.value
        val breakProgress = ((nowMs - breakStartMs).toFloat() /
            (nextLineStartMs - breakStartMs).coerceAtLeast(1L).toFloat()).coerceIn(0f, 1f)
        val centerY = size.height / 2f
        val spacing = 16.dp.toPx()
        val startX = 5.dp.toPx()
        for (dotIdx in 0 until 3) {
            val dotThreshold = (dotIdx + 1) / 3.2f
            val lit = breakProgress >= dotThreshold
            drawCircle(
                color = Color.White.copy(alpha = if (lit) 0.92f else 0.28f),
                radius = (if (lit) 4.5.dp else 3.5.dp).toPx(),
                center = Offset(startX + dotIdx * spacing, centerY),
            )
        }
    }
}

@Composable
internal fun NowPlayingQueuePanel(
    queue: List<AndroidTrack>,
    queueIndex: Int,
    isPlaying: Boolean,
    onSelectIndex: (Int) -> Unit,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        item(key = "queue_header", contentType = "queue_header") {
            Text(
                text = "Up Next",
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 18.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                modifier = Modifier.padding(vertical = 6.dp),
            )
        }
        itemsIndexed(
            items = queue,
            key = { idx, t -> "q_${t.id}_$idx" },
            contentType = { _, _ -> "queue_track" },
        ) { idx, item ->
            val isCurrent = idx == queueIndex
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(Shape10)
                    .background(if (isCurrent) Color.White.copy(alpha = 0.14f) else Color.Transparent)
                    .clickable { onSelectIndex(idx) }
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(Shape7)
                        .background(Color.White.copy(alpha = 0.1f)),
                    contentAlignment = Alignment.Center,
                ) {
                    val thumb = remember(item.thumbnailUrl) { item.thumbnailUrl.artworkAt(ROW_ART_PX) }
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
                            tint = Color.White.copy(alpha = 0.6f),
                        )
                    }
                    if (isCurrent) {
                        Box(
                            modifier = Modifier
                                .fillMaxSize()
                                .background(Color.Black.copy(alpha = 0.45f)),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = if (isPlaying) SonataIcons.GraphicEq else SonataIcons.Pause,
                                contentDescription = null,
                                tint = SonataAccent,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = item.title,
                        style = MaterialTheme.typography.titleSmall.copy(
                            fontSize = 15.sp,
                            fontWeight = if (isCurrent) FontWeight.Bold else FontWeight.Medium,
                        ),
                        color = if (isCurrent) SonataAccent else Color.White,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = item.artist,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.White.copy(alpha = 0.65f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

internal sealed interface ChangelogEntry {
    data class Heading(val title: String) : ChangelogEntry
    data class Bullet(val lead: String?, val body: String) : ChangelogEntry
    data class Note(val text: String) : ChangelogEntry
}

internal val MarkdownLinkRegex = Regex("""\[([^\]]+)]\([^)]+\)""")
internal val MarkdownInlineNoiseRegex = Regex("""(\*\*\*|\*\*|__|~~|`)""")
internal val MarkdownItalicRegex = Regex("""(?<!\w)\*([^*]+)\*(?!\w)""")
internal val MarkdownRuleRegex = Regex("""^[-*_]{3,}$""")

internal fun cleanChangelogMarkdown(raw: String): String = raw
    .replace(MarkdownLinkRegex, "$1")
    .replace(MarkdownInlineNoiseRegex, "")
    .replace(MarkdownItalicRegex, "$1")
    .replace(Regex("""\s+"""), " ")
    .trim()

internal fun parseChangelogEntries(raw: String): List<ChangelogEntry> {
    if (raw.isBlank()) return emptyList()
    val result = mutableListOf<ChangelogEntry>()
    for (rawLine in raw.lineSequence()) {
        val trimmed = rawLine.trim()
        if (trimmed.isEmpty() || MarkdownRuleRegex.matches(trimmed)) continue
        when {
            trimmed.startsWith("#") -> {
                val heading = cleanChangelogMarkdown(trimmed.trimStart('#').trim())
                if (heading.isNotBlank()) result += ChangelogEntry.Heading(heading)
            }
            trimmed.startsWith("- ") || trimmed.startsWith("* ") || trimmed.startsWith("• ") -> {
                val cleaned = cleanChangelogMarkdown(trimmed.drop(2))
                if (cleaned.isBlank()) continue
                val splitIdx = cleaned.indexOf(": ").takeIf { it in 2..42 }
                    ?: cleaned.indexOf(" — ").takeIf { it in 2..42 }
                    ?: cleaned.indexOf(" - ").takeIf { it in 2..42 }
                if (splitIdx != null) {
                    val lead = cleaned.substring(0, splitIdx).trim()
                    val sepLen = if (cleaned.startsWith(": ", splitIdx)) 2 else 3
                    val body = cleaned.substring(splitIdx + sepLen).trim()
                    result += ChangelogEntry.Bullet(lead = lead, body = body)
                } else {
                    result += ChangelogEntry.Bullet(lead = null, body = cleaned)
                }
            }
            else -> {
                val cleaned = cleanChangelogMarkdown(trimmed)
                if (cleaned.isNotBlank()) result += ChangelogEntry.Note(cleaned)
            }
        }
    }
    return result
}

@Composable
internal fun UpdateAvailableDialog(
    info: AndroidUpdateInfo,
    downloading: Boolean,
    progress: Float?,
    detail: String,
    error: String?,
    hasDownloadedApk: Boolean,
    onDownloadOrInstall: () -> Unit,
    onCancelDownload: () -> Unit,
    onDismiss: () -> Unit,
) {
    val changelogEntries = remember(info.releaseNotes) {
        parseChangelogEntries(info.releaseNotes)
    }
    val dialogSurface = MaterialTheme.colorScheme.surface
    val dialogInk = MaterialTheme.colorScheme.onSurface
    val dialogMuted = MaterialTheme.colorScheme.onSurfaceVariant
    val dialogBorder = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)
    Dialog(
        onDismissRequest = { if (!downloading) onDismiss() },
        properties = DialogProperties(
            dismissOnBackPress = !downloading,
            dismissOnClickOutside = !downloading,
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(24.dp))
                .background(dialogSurface)
                .border(1.dp, dialogBorder, RoundedCornerShape(24.dp))
                .padding(22.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(SonataAccent.copy(alpha = 0.18f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painter = painterResource(id = R.drawable.ic_sonata_moon),
                        contentDescription = null,
                        tint = SonataAccent,
                        modifier = Modifier.size(24.dp),
                    )
                }
                Column(Modifier.weight(1f)) {
                    Text(
                        text = "Update Available",
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                        color = dialogInk,
                    )
                    Text(
                        text = "v${info.currentVersion}  →  v${info.latestVersion}",
                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
                        color = SonataAccent,
                    )
                }
            }

            if (changelogEntries.isNotEmpty() && !downloading) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 260.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f))
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    changelogEntries.forEachIndexed { idx, entry ->
                        when (entry) {
                            is ChangelogEntry.Heading -> {
                                if (idx > 0) Spacer(Modifier.height(4.dp))
                                Text(
                                    text = entry.title.uppercase(),
                                    style = MaterialTheme.typography.labelSmall.copy(
                                        fontWeight = FontWeight.Bold,
                                        letterSpacing = 0.8.sp,
                                    ),
                                    color = SonataAccent,
                                )
                            }
                            is ChangelogEntry.Bullet -> {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                                    verticalAlignment = Alignment.Top,
                                ) {
                                    Text(
                                        text = "•",
                                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.Bold),
                                        color = SonataAccent,
                                    )
                                    val formatted = androidx.compose.ui.text.buildAnnotatedString {
                                        if (!entry.lead.isNullOrBlank()) {
                                            pushStyle(
                                                androidx.compose.ui.text.SpanStyle(
                                                    fontWeight = FontWeight.SemiBold,
                                                    color = dialogInk,
                                                ),
                                            )
                                            append(entry.lead)
                                            pop()
                                            if (entry.body.isNotBlank()) {
                                                append(" — ")
                                            }
                                        }
                                        pushStyle(
                                            androidx.compose.ui.text.SpanStyle(
                                                color = dialogMuted,
                                            ),
                                        )
                                        append(entry.body)
                                        pop()
                                    }
                                    Text(
                                        text = formatted,
                                        style = MaterialTheme.typography.bodySmall.copy(lineHeight = 18.sp),
                                        modifier = Modifier.weight(1f),
                                    )
                                }
                            }
                            is ChangelogEntry.Note -> {
                                Text(
                                    text = entry.text,
                                    style = MaterialTheme.typography.bodySmall.copy(lineHeight = 18.sp),
                                    color = dialogMuted,
                                )
                            }
                        }
                    }
                }
            }

            if (downloading) {
                val themeTokens = LocalLunariaColors.current
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (progress != null) {
                        LinearProgressIndicator(
                            progress = { progress.coerceIn(0f, 1f) },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = themeTokens.petalLavender,
                            trackColor = themeTokens.onSurfacePrimary.copy(alpha = 0.14f),
                        )
                    } else {
                        LinearProgressIndicator(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = themeTokens.petalLavender,
                            trackColor = themeTokens.onSurfacePrimary.copy(alpha = 0.14f),
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = detail.ifBlank { "Downloading APK..." },
                            style = MaterialTheme.typography.labelMedium,
                            color = dialogMuted,
                        )
                        if (progress != null) {
                            Text(
                                text = "${(progress * 100f).roundToInt()}%",
                                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                                color = dialogInk,
                            )
                        }
                    }
                }
            } else if (detail.isNotBlank() && error == null) {
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = dialogMuted,
                )
            }

            if (!error.isNullOrBlank()) {
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFFFF6B6B),
                )
            }

            val themeTokens = LocalLunariaColors.current
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(46.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(themeTokens.eclipseCard)
                        .clickable {
                            if (downloading) onCancelDownload() else onDismiss()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = if (downloading) "Cancel" else if (hasDownloadedApk) "Close" else "Later",
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = dialogInk,
                    )
                }
                if (!downloading && !hasDownloadedApk) {
                    Box(
                        modifier = Modifier
                            .weight(1.35f)
                            .height(46.dp)
                            .clip(RoundedCornerShape(14.dp))
                            .background(themeTokens.petalLavender)
                            .clickable(onClick = onDownloadOrInstall),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = "Download APK",
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                            color = Color.White,
                        )
                    }
                }
            }
        }
    }
}

internal fun formatDurationMs(ms: Long): String {
    val totalSec = (ms / 1000L).coerceAtLeast(0L)
    val mins = totalSec / 60L
    val secs = totalSec % 60L
    return "$mins:${secs.toString().padStart(2, '0')}"
}

internal fun shareYouTubeMusicUrl(context: android.content.Context, url: String, title: String) {
    val cleanUrl = url.trim()
    if (cleanUrl.isBlank()) return
    val sendIntent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, cleanUrl)
        if (title.isNotBlank()) {
            putExtra(Intent.EXTRA_SUBJECT, title)
            putExtra(Intent.EXTRA_TITLE, title)
        }
    }
    runCatching {
        context.startActivity(Intent.createChooser(sendIntent, title.ifBlank { "Share Link" }))
    }
}

internal fun encodeUrlQuery(value: String): String =
    java.net.URLEncoder.encode(value, "UTF-8").replace("+", "%20")

internal fun shareTrackLink(context: android.content.Context, track: AndroidTrack) {
    val url = if (track.id.isNotBlank() && !track.id.startsWith("jiosaavn:") && !track.id.startsWith("local:")) {
        "https://music.youtube.com/watch?v=${track.id}"
    } else {
        val query = listOf(track.title, track.artist).filter { it.isNotBlank() }.joinToString(" ")
        "https://music.youtube.com/search?q=${encodeUrlQuery(query)}"
    }
    val label = listOf(track.title, track.artist).filter { it.isNotBlank() }.joinToString(" — ")
    shareYouTubeMusicUrl(context, url, label)
}

internal fun buildShareUrlForCollection(browseId: String, title: String): String {
    val cleanId = browseId.trim()
    return when {
        cleanId.startsWith("VL") -> "https://music.youtube.com/playlist?list=${cleanId.removePrefix("VL")}"
        cleanId.startsWith("PL") || cleanId.startsWith("OLAK") || cleanId.startsWith("RD") ->
            "https://music.youtube.com/playlist?list=$cleanId"
        cleanId.startsWith("MPRE") -> "https://music.youtube.com/browse/$cleanId"
        cleanId.startsWith("UC") -> "https://music.youtube.com/channel/$cleanId"
        else -> "https://music.youtube.com/search?q=${encodeUrlQuery(title)}"
    }
}

internal fun shareCollectionLink(context: android.content.Context, browseId: String, title: String) {
    val url = buildShareUrlForCollection(browseId, title)
    shareYouTubeMusicUrl(context, url, title)
}

internal fun shareArtistLink(context: android.content.Context, artistId: String, artistName: String) {
    val cleanId = artistId.trim()
    val url = if (cleanId.startsWith("UC")) {
        "https://music.youtube.com/channel/$cleanId"
    } else {
        "https://music.youtube.com/search?q=${encodeUrlQuery(artistName)}"
    }
    shareYouTubeMusicUrl(context, url, artistName)
}

