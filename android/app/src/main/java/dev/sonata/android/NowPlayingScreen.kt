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
import dev.sonata.android.ui.i18n.LocalSonataStrings
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
import androidx.compose.ui.graphics.luminance
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
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.sonataThemeAccent
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
import dev.sonata.android.playback.mapMusicToVideoMs
import dev.sonata.android.playback.mapVideoToMusicMs
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
import dev.sonata.android.ui.player.CurrentLyricStrip
import dev.sonata.android.ui.player.NowPlayingActionRowBlock
import dev.sonata.android.ui.player.NowPlayingLyricsPanel
import dev.sonata.android.ui.player.NowPlayingOverlays
import dev.sonata.android.ui.player.NowPlayingQueuePanel
import dev.sonata.android.ui.player.NowPlayingScrubberAndTransportBlock
import dev.sonata.android.ui.player.NowPlayingTopHandleAndModeBar
import dev.sonata.android.ui.player.NowPlayingUpdateDialog
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
    val onToggleAutoplay: () -> Unit = {},
    val onOpenVideoQualityPicker: () -> Unit = {},
    val onEnterVideoFullscreen: () -> Unit = {},
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
    onToggleAutoplay: () -> Unit = {},
    onShareInApp: () -> Unit,
    onStartRadio: () -> Unit,
    onPlayNext: () -> Unit,
    onAddToQueue: () -> Unit,
    onOpenAlbum: () -> Unit,
    onOpenArtist: () -> Unit,
    onOpenSettings: () -> Unit,
    onAddToPlaylist: () -> Unit = {},
    onEnableVideoMode: () -> Unit = {},
    onDisableVideoMode: () -> Unit = {},
    onSelectVideoQuality: (dev.sonata.android.data.VideoQualityPreference, Int?) -> Unit = { _, _ -> },
    onSetVideoFullscreen: (Boolean) -> Unit = {},
    onSuppressVideoWarning: (Boolean) -> Unit = {},
    onDismiss: () -> Unit,
) {
    var showVideoWarningDialog by remember { mutableStateOf(false) }
    var showVideoQualityDialog by remember { mutableStateOf(false) }
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
            onToggleAutoplay = onToggleAutoplay,
            onOpenVideoQualityPicker = { showVideoQualityDialog = true },
            onEnterVideoFullscreen = { onSetVideoFullscreen(true) },
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
    val effectiveLyricClockMs = remember(
        smoothClockMs,
        state.videoModeEnabled,
        state.musicVideoCandidate,
    ) {
        derivedStateOf {
            if (state.videoModeEnabled) {
                mapVideoToMusicMs(
                    videoMs = smoothClockMs.value,
                    segments = state.musicVideoCandidate?.syncSegments.orEmpty(),
                )
            } else {
                smoothClockMs.value
            }
        }
    }
    val lyricSeekTo: (Long) -> Unit = { musicPosMs ->
        val mappedPosMs = if (state.videoModeEnabled) {
            mapMusicToVideoMs(
                musicMs = musicPosMs,
                segments = state.musicVideoCandidate?.syncSegments.orEmpty(),
            )
        } else {
            musicPosMs
        }
        onSeekTo(mappedPosMs)
    }
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
    // Portrait viewports use full-bleed hero artwork unless user switches to framed cover art or video mode.
    val playerFillsWindow = !isLandscapePlayer && settings.fullScreenCoverArt && !state.videoModeEnabled
    val showPortraitHero = playerFillsWindow && !panelOpen

    val playerView = androidx.compose.ui.platform.LocalView.current
    DisposableEffect(settings.hideStatusBarInPlayer, state.videoModeEnabled, playerView) {
        val window = (playerView.context as? android.app.Activity)?.window
        val insetsController = window?.let { androidx.core.view.WindowCompat.getInsetsController(it, playerView) }
        if (settings.hideStatusBarInPlayer) {
            insetsController?.hide(androidx.core.view.WindowInsetsCompat.Type.statusBars())
        }
        if (state.videoModeEnabled) {
            window?.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
        onDispose {
            if (settings.hideStatusBarInPlayer) {
                insetsController?.show(androidx.core.view.WindowInsetsCompat.Type.statusBars())
            }
            if (state.videoModeEnabled) {
                window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            }
        }
    }
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
            .pointerInput(onDismiss, state.videoFullscreen) {
                if (state.videoFullscreen) return@pointerInput
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
                if (settings.animatedArtwork &&
                    currentCanvas != null &&
                    !state.videoModeEnabled &&
                    !state.videoLoading
                ) {
                    CanvasVideoSurface(
                        media = currentCanvas,
                        isVisible = showPortraitHero && state.isPlaying,
                        preferPortrait = true,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                // Top scrim keeps white status-bar icons legible over bright/white album covers.
                Box(
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .fillMaxWidth()
                        .height(statusBarTop + 36.dp)
                        .background(
                            dev.sonata.android.ui.smoothScrimBrush(
                                from = Color.Black.copy(alpha = 0.42f),
                                to = Color.Black.copy(alpha = 0f),
                            ),
                        ),
                )
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
            NowPlayingTopHandleAndModeBar(
                topHandleStripHeight = topHandleStripHeight,
                showModePill = state.musicVideoCandidate != null || state.videoModeEnabled,
                videoModeEnabled = state.videoModeEnabled,
                videoLoading = state.videoLoading,
                onDismiss = onDismiss,
                onSelectMusic = onDisableVideoMode,
                onSelectVideo = {
                    if (settings.suppressVideoAudioQualityWarning) {
                        onEnableVideoMode()
                    } else {
                        showVideoWarningDialog = true
                    }
                },
            )

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
                    smoothClockMs = effectiveLyricClockMs,
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
                    onSeekTo = lyricSeekTo,
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
                    smoothClockMs = effectiveLyricClockMs,
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
                    onSeekTo = lyricSeekTo,
                    onJumpToQueueIndex = onJumpToQueueIndex,
                    onOpenSettings = { showPlayerOptions = true },
                    modifier = Modifier.weight(1f),
                )
            }
        }
        }

        NowPlayingOverlays(
            track = track,
            state = state,
            settings = settings,
            clock = clock,
            isLiked = isLiked,
            isTrackDownloaded = isTrackDownloaded,
            activeTrackDownload = activeTrackDownload,
            sleepTimerRemaining = sleepTimerRemaining,
            trackLyricOffsetMs = trackLyricOffsetMs,
            audioPipelineSummary = audioPipelineSummary,
            nowPlayingBackdrop = nowPlayingBackdrop,
            showPlayerOptions = showPlayerOptions,
            showVideoWarningDialog = showVideoWarningDialog,
            showVideoQualityDialog = showVideoQualityDialog,
            onDismissOptions = { showPlayerOptions = false },
            onToggleStar = onToggleStar,
            onAddToPlaylist = onAddToPlaylist,
            onShareInApp = onShareInApp,
            onOpenAudioControls = onOpenAudioControls,
            onOpenAudioPipeline = onOpenAudioPipeline,
            onDownloadTrack = onDownloadTrack,
            onOpenSleepTimer = onOpenSleepTimer,
            onStepLyricOffsetMs = onStepLyricOffsetMs,
            onResetLyricOffsetMs = onResetLyricOffsetMs,
            onToggleLyricsRomanization = onToggleLyricsRomanization,
            onToggleLyricsTranslation = onToggleLyricsTranslation,
            onStartRadio = onStartRadio,
            onPlayNext = onPlayNext,
            onAddToQueue = onAddToQueue,
            onOpenAlbum = onOpenAlbum,
            onOpenArtist = onOpenArtist,
            onTogglePlayPause = onTogglePlayPause,
            onNext = onNext,
            onPrevious = onPrevious,
            onSeekTo = onSeekTo,
            onSeekFraction = onSeekFraction,
            onOpenVideoQualityDialog = { showVideoQualityDialog = true },
            onDismissVideoQualityDialog = { showVideoQualityDialog = false },
            onExitVideoFullscreen = { onSetVideoFullscreen(false) },
            onSuppressVideoWarning = onSuppressVideoWarning,
            onConfirmVideoMode = onEnableVideoMode,
            onDismissVideoWarning = { showVideoWarningDialog = false },
            onSelectVideoQuality = onSelectVideoQuality,
        )
    }
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
                val extras = LocalNowPlayingExtras.current
                if (state.videoModeEnabled && !state.videoFullscreen) {
                    dev.sonata.android.ui.player.InlineMusicVideoStage(
                        artworkUrl = state.musicVideoCandidate?.thumbnailUrl ?: track.thumbnailUrl,
                        qualityLabel = state.activeVideoQualityLabel,
                        isBuffering = state.isBuffering,
                        videoLoading = state.videoLoading,
                        onOpenQualityPicker = extras.onOpenVideoQualityPicker,
                        onEnterFullscreen = extras.onEnterVideoFullscreen,
                        modifier = Modifier
                            .widthIn(max = 480.dp)
                            .fillMaxWidth()
                            .aspectRatio(16f / 9f),
                    )
                } else {
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
                    if (extras.settings.animatedArtwork &&
                        extras.currentCanvas != null &&
                        !state.videoModeEnabled &&
                        !state.videoLoading
                    ) {
                        CanvasVideoSurface(
                            media = extras.currentCanvas,
                            isVisible = state.isPlaying,
                            preferPortrait = false,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
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
                                tint = if (isLiked) sonataThemeAccent() else Color.White,
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
                                tint = if (isLiked) sonataThemeAccent() else Color.White,
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
                                val portraitExtras = LocalNowPlayingExtras.current
                                if (state.videoModeEnabled && !state.videoFullscreen) {
                                    dev.sonata.android.ui.player.InlineMusicVideoStage(
                                        artworkUrl = state.musicVideoCandidate?.thumbnailUrl ?: track.thumbnailUrl,
                                        qualityLabel = state.activeVideoQualityLabel,
                                        isBuffering = state.isBuffering,
                                        videoLoading = state.videoLoading,
                                        onOpenQualityPicker = portraitExtras.onOpenVideoQualityPicker,
                                        onEnterFullscreen = portraitExtras.onEnterVideoFullscreen,
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .aspectRatio(16f / 9f),
                                    )
                                } else {
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
                                    if (portraitExtras.settings.animatedArtwork &&
                                        portraitExtras.currentCanvas != null &&
                                        !state.videoModeEnabled &&
                                        !state.videoLoading &&
                                        !panelOpen
                                    ) {
                                        CanvasVideoSurface(
                                            media = portraitExtras.currentCanvas,
                                            isVisible = state.isPlaying,
                                            preferPortrait = false,
                                            modifier = Modifier.fillMaxSize(),
                                        )
                                    }
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
                                tint = if (isLiked) sonataThemeAccent() else Color.White,
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
    NowPlayingUpdateDialog(
        info = info,
        downloading = downloading,
        progress = progress,
        detail = detail,
        error = error,
        hasDownloadedApk = hasDownloadedApk,
        onDownloadOrInstall = onDownloadOrInstall,
        onCancelDownload = onCancelDownload,
        onDismiss = onDismiss,
    )
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

