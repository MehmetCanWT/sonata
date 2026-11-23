package dev.sonata.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.core.content.ContextCompat
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
import androidx.compose.runtime.rememberCoroutineScope
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
import dev.sonata.android.ui.components.SonataShareSheet
import dev.sonata.android.ui.components.SonataShareTarget
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
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


class MainActivity : ComponentActivity() {
    private val notifLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        SingletonImageLoader.setSafe { appCtx ->
            ImageLoader.Builder(appCtx)
                .components {
                    add(
                        OkHttpNetworkFetcherFactory(
                            callFactory = {
                                OkHttpClient.Builder()
                                    .dispatcher(
                                        Dispatcher().apply {
                                            maxRequests = 32
                                            maxRequestsPerHost = 12
                                        },
                                    )
                                    .build()
                            },
                        ),
                    )
                    add(SonataThumbnailInterceptor)
                }
                .memoryCache {
                    MemoryCache.Builder()
                        .maxSizePercent(appCtx, 0.25)
                        .strongReferencesEnabled(true)
                        .build()
                }
                .diskCache {
                    DiskCache.Builder()
                        .directory(File(appCtx.cacheDir, "image_cache"))
                        .maxSizeBytes(200L * 1024L * 1024L)
                        .build()
                }
                .crossfade(false)
                .precision(Precision.INEXACT)
                .allowHardware(true)
                .build()
        }
        enableEdgeToEdge()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        setContent {
            SonataApp()
        }
    }
}


@Composable
fun SonataApp(vm: SonataViewModel = viewModel()) {
    val state by vm.uiState.collectAsStateWithLifecycle()
    val settings by vm.settings.collectAsStateWithLifecycle()

    val context = LocalContext.current
    LaunchedEffect(settings.fpsLimit, state.supports120Hz) {
        val activity = context as? ComponentActivity ?: return@LaunchedEffect
        runCatching {
            val window = activity.window
            val params = window.attributes
            val currentDisplay = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                activity.display
            } else {
                @Suppress("DEPRECATION")
                window.windowManager?.defaultDisplay
            }
            val currentMode = currentDisplay?.mode
            val modes = currentDisplay?.supportedModes
            val sameResModes = if (currentMode != null && modes != null) {
                modes.filter {
                    it.physicalWidth == currentMode.physicalWidth &&
                        it.physicalHeight == currentMode.physicalHeight
                }.ifEmpty { modes.toList() }
            } else {
                modes?.toList().orEmpty()
            }
            val maxRefresh = sameResModes.maxOfOrNull { it.refreshRate }
                ?: if (state.supports120Hz) 120f else 60f
            val requestedCap = when {
                settings.fpsLimit <= 60 -> 60f
                settings.fpsLimit <= 90 -> 90f.coerceAtMost(maxRefresh)
                else -> maxRefresh
            }
            val bestMode = sameResModes
                .filter { it.refreshRate <= requestedCap + 1.5f }
                .maxByOrNull { it.refreshRate }
                ?: sameResModes.minByOrNull { kotlin.math.abs(it.refreshRate - requestedCap) }
            val resolvedHz = bestMode?.refreshRate ?: requestedCap
            params.preferredRefreshRate = resolvedHz
            if (bestMode != null) {
                params.preferredDisplayModeId = bestMode.modeId
            }
            window.attributes = params
        }
    }
    val systemDark = androidx.compose.foundation.isSystemInDarkTheme()
    val darkTheme = settings.isDarkActive(systemDark)
    val amoled = settings.themeMode == ThemeMode.AMOLED
    val resolvedDesign = settings.resolvedDesignTheme()
    SonataTheme(
        darkTheme = darkTheme,
        amoled = amoled,
        designTheme = resolvedDesign,
    ) {
        SystemBarIcons(dark = !darkTheme)
        val themeTokens = LocalLunariaColors.current
        val paintBackdrop: ContentDrawScope.() -> Unit = remember(
            themeTokens.pageBackdropBrush,
            themeTokens.designTheme,
            themeTokens.isDark,
            themeTokens.petalLavender,
            themeTokens.lunarMist,
        ) {
            {
                drawRect(brush = themeTokens.pageBackdropBrush)
                drawThemeAtmosphericBackdrop(
                    designTheme = themeTokens.designTheme,
                    isDark = themeTokens.isDark,
                    accent = themeTokens.petalLavender,
                    mist = themeTokens.lunarMist,
                )
                drawContent()
            }
        }
        val appBackdrop = rememberLayerBackdrop(onDraw = paintBackdrop)
        val glassEnabled = settings.liquidGlassEnabled && isGlassSupported()
        val downloadedRecords by vm.downloads.collectAsStateWithLifecycle()
        val downloadedTrackIds = remember(downloadedRecords) {
            downloadedRecords.mapTo(HashSet(downloadedRecords.size)) { it.videoId }
        }

        CompositionLocalProvider(
            LocalLiquidGlassEnabled provides glassEnabled,
            LocalGlassPerformance provides settings.glassPerformance,
            LocalReduceDynamicBlur provides settings.reduceDynamicBlur,
            LocalReduceAnimation provides settings.reduceAnimation,
            LocalAppBackdrop provides appBackdrop,
            LocalDownloadedTrackIds provides downloadedTrackIds,
        ) {
            SonataAppContent(
                state = state,
                settings = settings,
                appBackdrop = appBackdrop,
                vm = vm,
            )
        }
    }
}

@Composable
private fun SonataAppContent(
    state: SonataUiState,
    settings: SonataSettingsState,
    appBackdrop: LayerBackdrop,
    vm: SonataViewModel,
) {
    val context = LocalContext.current
    var showNowPlaying by rememberSaveable { mutableStateOf(false) }
    var showAllMoods by remember { mutableStateOf(false) }
    var showListeningHistory by rememberSaveable { mutableStateOf(false) }
    var showUserMenu by remember { mutableStateOf(false) }
    var showDownloadsSheet by remember { mutableStateOf(false) }
    var showAudioPipeline by remember { mutableStateOf(false) }
    var showSleepTimer by remember { mutableStateOf(false) }
    var showAudioControls by remember { mutableStateOf(false) }
    var trackOptionsTarget by remember { mutableStateOf<TrackMenuRequest?>(null) }
    var heldContextTarget by remember { mutableStateOf<HeldContextTarget?>(null) }
    var shareTarget by remember { mutableStateOf<SonataShareTarget?>(null) }
    var collectionPickerMeta by remember { mutableStateOf<HeldContextTarget.AlbumOrPlaylistCard?>(null) }
    var pendingAddToPlaylistBrowseId by remember { mutableStateOf<String?>(null) }
    var visiblePlaylistStatus by remember { mutableStateOf<String?>(null) }
    val tabBackStack = remember { mutableStateListOf<AndroidTab>() }
    val navigateToTab: (AndroidTab) -> Unit = { target ->
        if (target != state.activeTab) {
            if (target == AndroidTab.Home) {
                tabBackStack.clear()
            } else if (state.activeTab != AndroidTab.Home) {
                tabBackStack.remove(target)
                tabBackStack.add(state.activeTab)
            }
        }
        vm.selectTab(target)
    }

    val ioScope = rememberCoroutineScope()
    val exportBackupLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        if (uri != null) {
            ioScope.launch(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openOutputStream(uri)?.use { out ->
                        out.write(vm.exportSettingsBackup().toByteArray(Charsets.UTF_8))
                    }
                }
            }
        }
    }
    val importBackupLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri != null) {
            ioScope.launch(Dispatchers.IO) {
                runCatching {
                    val json = context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                    if (!json.isNullOrBlank()) {
                        vm.importSettingsBackup(json)
                    }
                }
            }
        }
    }

    val homeListState = rememberLazyListState()
    val exploreListState = rememberLazyListState()
    val radioListState = rememberLazyListState()
    val libraryListState = rememberLazyListState()
    val searchListState = rememberLazyListState()
    val collectionListState = rememberLazyListState()

    val navBarScroll = rememberFloatingTabBarScrollConnection(
        initialIsInline = false,
        scrollThreshold = 42.dp,
        inlineBehavior = FloatingTabBarInlineBehavior.OnScrollDown,
    )

    val statusTopPadding = rememberSafeTopInset() + 8.dp
    val topBarHeight = statusTopPadding + TopBarContentHeight
    // Fixed bottom inset keyed only on activeTrack presence so FloatingTabBar collapse/expand
    // never triggers full LazyColumn relayout mid-fling.
    val navBottomPadding = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val bottomBarInset = navBottomPadding + if (state.activeTrack != null) 144.dp else 92.dp

    val scrolled by remember(
        state.activeTab,
        state.collectionTitle,
        showAllMoods,
        showListeningHistory,
        homeListState,
        exploreListState,
        radioListState,
        libraryListState,
        searchListState,
        collectionListState,
    ) {
        derivedStateOf {
            when {
                state.collectionTitle != null ->
                    collectionListState.firstVisibleItemIndex > 0 ||
                        collectionListState.firstVisibleItemScrollOffset > 28
                showAllMoods || showListeningHistory -> true
                state.activeTab == AndroidTab.Home ->
                    homeListState.firstVisibleItemIndex > 0 ||
                        homeListState.firstVisibleItemScrollOffset > 28
                state.activeTab == AndroidTab.Explore ->
                    exploreListState.firstVisibleItemIndex > 0 ||
                        exploreListState.firstVisibleItemScrollOffset > 28
                state.activeTab == AndroidTab.Radio ->
                    radioListState.firstVisibleItemIndex > 0 ||
                        radioListState.firstVisibleItemScrollOffset > 28
                state.activeTab == AndroidTab.Library ->
                    libraryListState.firstVisibleItemIndex > 0 ||
                        libraryListState.firstVisibleItemScrollOffset > 28
                else ->
                    searchListState.firstVisibleItemIndex > 0 ||
                        searchListState.firstVisibleItemScrollOffset > 28
            }
        }
    }

    val hasOverlayOpen = showNowPlaying ||
        showUserMenu ||
        showDownloadsSheet ||
        showAudioPipeline ||
        showSleepTimer ||
        showAudioControls ||
        trackOptionsTarget != null ||
        heldContextTarget != null ||
        state.addToPlaylistTargets.isNotEmpty() ||
        state.showSettingsSheet ||
        state.artistPage != null ||
        state.artistLoading

    LaunchedEffect(state.loading, state.collectionTracks, pendingAddToPlaylistBrowseId) {
        val pendingId = pendingAddToPlaylistBrowseId
        if (pendingId != null && !state.loading) {
            if (state.collectionTracks.isNotEmpty()) {
                vm.openAddToPlaylist(state.collectionTracks)
                pendingAddToPlaylistBrowseId = null
            } else if (state.collectionTitle != null) {
                pendingAddToPlaylistBrowseId = null
            }
        }
    }

    LaunchedEffect(state.addToPlaylistStatus) {
        val msg = state.addToPlaylistStatus
        if (!msg.isNullOrBlank()) {
            visiblePlaylistStatus = msg
            vm.clearAddToPlaylistStatus()
        }
    }

    LaunchedEffect(visiblePlaylistStatus) {
        if (!visiblePlaylistStatus.isNullOrBlank()) {
            delay(2000L)
            visiblePlaylistStatus = null
        }
    }

    if (trackOptionsTarget != null &&
        heldContextTarget == null &&
        state.addToPlaylistTargets.isEmpty() &&
        !showNowPlaying &&
        !state.showSettingsSheet
    ) {
        BackHandler { trackOptionsTarget = null }
    }

    if (showUserMenu &&
        heldContextTarget == null &&
        state.addToPlaylistTargets.isEmpty() &&
        !showNowPlaying &&
        !state.showSettingsSheet
    ) {
        BackHandler { showUserMenu = false }
    }

    val canStepBackFromSearch = state.activeTab == AndroidTab.Search && state.query.isNotEmpty()
    if (!hasOverlayOpen && (state.collectionTitle != null || showAllMoods || showListeningHistory || canStepBackFromSearch || state.activeTab != AndroidTab.Home)) {
        BackHandler {
            when {
                state.collectionTitle != null -> vm.closeCollection()
                showListeningHistory -> showListeningHistory = false
                showAllMoods -> showAllMoods = false
                canStepBackFromSearch -> vm.searchDebounced("")
                tabBackStack.isNotEmpty() -> {
                    val prevTab = tabBackStack.removeAt(tabBackStack.lastIndex)
                    vm.selectTab(prevTab)
                }
                state.activeTab != AndroidTab.Home -> vm.selectTab(AndroidTab.Home)
            }
        }
    }

    if (state.showLoginSheet) {
        BackHandler { vm.dismissLoginSheet() }
        YtMusicLoginScreen(
            onCaptured = vm::completeLogin,
            onDismiss = vm::dismissLoginSheet,
            freshSignIn = true,
        )
        return
    }

    if (showDownloadsSheet) {
        val downloads by vm.downloads.collectAsStateWithLifecycle()
        val activeDownloads by vm.activeDownloads.collectAsStateWithLifecycle()
        val downloadsTotalBytes by vm.downloadsTotalBytes.collectAsStateWithLifecycle()
        DownloadsManagerSheet(
            downloads = downloads,
            activeDownloads = activeDownloads,
            totalBytes = downloadsTotalBytes,
            wifiOnly = settings.wifiOnlyDownloads,
            exportToMusic = settings.exportDownloads,
            onToggleWifiOnly = { vm.updateSettings { it.copy(wifiOnlyDownloads = !it.wifiOnlyDownloads) } },
            onToggleExportToMusic = { vm.updateSettings { it.copy(exportDownloads = !it.exportDownloads) } },
            onPlayOfflineTrack = { rec ->
                vm.playDownloadedTrack(rec)
                showDownloadsSheet = false
            },
            onPlayAllOffline = {
                downloads.firstOrNull()?.let { first ->
                    vm.playDownloadedTrack(first)
                    showDownloadsSheet = false
                }
            },
            onCancelActiveDownload = vm::cancelDownload,
            onRemoveDownload = vm::removeDownload,
            onClearAllDownloads = vm::clearAllDownloads,
            onDismiss = { showDownloadsSheet = false },
        )
    }

    if (showAudioPipeline) {
        val audioPipelineInfo by vm.audioPipelineInfo.collectAsStateWithLifecycle()
        AudioPipelineModal(
            info = audioPipelineInfo,
            onDismiss = { showAudioPipeline = false },
        )
    }

    if (showSleepTimer) {
        val sleepTimerRemaining by vm.sleepTimerRemainingMinutes.collectAsStateWithLifecycle()
        SleepTimerSheet(
            remainingMinutes = sleepTimerRemaining,
            onSelectMinutes = { mins ->
                vm.setSleepTimer(mins)
                showSleepTimer = false
            },
            onDismiss = { showSleepTimer = false },
        )
    }

    if (showAudioControls) {
        AudioControlsSheet(
            speed = settings.playbackSpeed,
            skipSilence = settings.skipSilenceEnabled,
            onSpeedChange = { speed -> vm.updateSettings { it.copy(playbackSpeed = speed) } },
            onSkipSilenceToggle = { enabled -> vm.updateSettings { it.copy(skipSilenceEnabled = enabled) } },
            onDismiss = { showAudioControls = false },
        )
    }

    if (state.showUpdateDialog && state.updateInfo?.updateAvailable == true) {
        UpdateAvailableDialog(
            info = state.updateInfo!!,
            downloading = state.updateDownloading,
            progress = state.updateDownloadProgress,
            detail = state.updateDownloadDetail,
            error = state.updateError,
            hasDownloadedApk = !state.downloadedUpdateApkPath.isNullOrBlank(),
            onDownloadOrInstall = vm::downloadAndInstallUpdate,
            onCancelDownload = vm::cancelUpdateDownload,
            onDismiss = vm::dismissUpdateDialog,
        )
    }

    val artistWindowBg = MaterialTheme.colorScheme.background
    val artistBackdrop = rememberLayerBackdrop {
        drawRect(artistWindowBg)
        drawContent()
    }
    val activeOverlayBackdrop = if (state.artistPage != null || state.artistLoading) {
        artistBackdrop
    } else {
        appBackdrop
    }

    val openTrackHeldMenu: (AndroidTrack, Int?, Rect?) -> Unit = { track, trackNumber, bounds ->
        val resolvedTrack = if (track.thumbnailUrl.isNullOrBlank() && !state.collectionCoverUrl.isNullOrBlank()) {
            track.copy(thumbnailUrl = state.collectionCoverUrl)
        } else {
            track
        }
        heldContextTarget = HeldContextTarget.TrackRow(
            track = resolvedTrack,
            trackNumber = trackNumber,
            isExplicit = resolvedTrack.isExplicit,
            anchorBounds = bounds,
        )
    }
    val openCardHeldMenu: (AndroidHomeCard, String?, Boolean, Rect?) -> Unit = { card, eyebrow, isTopPickTall, bounds ->
        val cardTrack = card.track
        if (cardTrack != null && (card.kind == AndroidHomeCardKind.SONG || card.kind == AndroidHomeCardKind.VIDEO)) {
            heldContextTarget = HeldContextTarget.TrackRow(
                track = cardTrack,
                isExplicit = cardTrack.isExplicit,
                anchorBounds = bounds,
            )
        } else {
            heldContextTarget = HeldContextTarget.AlbumOrPlaylistCard(
                browseId = card.id,
                title = card.title,
                subtitle = card.subtitle,
                coverUrl = card.thumbnailUrl,
                isAlbum = card.kind == AndroidHomeCardKind.ALBUM,
                eyebrow = eyebrow,
                isTopPickTall = isTopPickTall,
                anchorBounds = bounds,
            )
        }
    }

    val themeTokens = LocalLunariaColors.current
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(themeTokens.pageBackdropBrush),
    ) {
        // Scrollable backdrop source sampled by sibling floating Liquid Glass overlays
        // (ArtistScreen renders as its own sibling outside appBackdrop so floating Liquid Glass never cycles).
        Box(
            modifier = Modifier
                .fillMaxSize()
                .layerBackdrop(appBackdrop),
        ) {
            val topPad = when {
                state.collectionTitle != null -> topBarHeight + 8.dp
                state.activeTab == AndroidTab.Search && !showAllMoods && !showListeningHistory -> topBarHeight + 10.dp
                else -> statusTopPadding + 8.dp
            }
            val contentPadding = PaddingValues(top = topPad, bottom = bottomBarInset)

            when {
                state.collectionTitle != null -> {
                    val detailDownloads by vm.downloads.collectAsStateWithLifecycle()
                    val detailActiveDownloads by vm.activeDownloads.collectAsStateWithLifecycle()
                    val detailTrackIds = remember(state.collectionTracks) {
                        state.collectionTracks.mapTo(HashSet()) { it.id }
                    }
                    val detailIsDownloading = remember(detailTrackIds, detailActiveDownloads) {
                        detailTrackIds.isNotEmpty() && detailActiveDownloads.keys.any { it in detailTrackIds }
                    }
                    val detailAllDownloaded = remember(detailTrackIds, detailDownloads) {
                        if (detailTrackIds.isEmpty()) false
                        else {
                            val downloadedIds = detailDownloads.mapTo(HashSet()) { it.videoId }
                            detailTrackIds.all { it in downloadedIds }
                        }
                    }
                    DetailScreen(
                        state = state,
                        listState = collectionListState,
                        contentPadding = contentPadding,
                        modifier = Modifier.nestedScroll(navBarScroll),
                        isDownloading = detailIsDownloading,
                        allDownloaded = detailAllDownloaded,
                        onPlayTrack = { track -> vm.playTrack(track, state.collectionTracks) },
                        onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                        onTrackLongClick = { track, trackNum, bounds -> openTrackHeldMenu(track, trackNum, bounds) },
                        onPlayAll = {
                            state.collectionTracks.firstOrNull()?.let { first ->
                                vm.playTrack(first, state.collectionTracks)
                            }
                        },
                        onShuffleAll = { vm.toggleShuffleAndPlay(state.collectionTracks) },
                        onDownloadAll = {
                            vm.downloadCollection(
                                tracks = state.collectionTracks,
                                collectionId = state.collectionBrowseId ?: state.collectionTitle ?: "collection",
                            )
                        },
                        onMoreOptions = {
                            heldContextTarget = HeldContextTarget.AlbumOrPlaylistCard(
                                browseId = state.collectionBrowseId.orEmpty(),
                                title = state.collectionTitle.orEmpty(),
                                subtitle = state.collectionSubtitle.orEmpty(),
                                coverUrl = state.collectionCoverUrl,
                                isAlbum = state.collectionKindLabel.equals("Album", ignoreCase = true),
                                eyebrow = state.collectionKindLabel,
                            )
                        },
                        onOpenArtist = { artistName ->
                            vm.openArtist("", artistName)
                        },
                    )
                }

                showAllMoods -> AllMoodsScreen(
                    contentPadding = PaddingValues(top = topBarHeight + 8.dp, bottom = bottomBarInset),
                    modifier = Modifier.nestedScroll(navBarScroll),
                    onOpenMood = { title ->
                        showAllMoods = false
                        vm.openMood(title)
                    },
                )

                showListeningHistory -> ListeningHistoryScreen(
                    tracks = state.lastPlayedTracks,
                    activeTrackId = state.activeTrack?.id,
                    isPlaying = state.isPlaying,
                    contentPadding = PaddingValues(top = topBarHeight + 8.dp, bottom = bottomBarInset),
                    modifier = Modifier.nestedScroll(navBarScroll),
                    onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                    onShuffleAll = { tracks -> vm.toggleShuffleAndPlay(tracks) },
                    onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                    onTrackLongClick = { track, idx, bounds -> openTrackHeldMenu(track, idx, bounds) },
                )

                else -> {
                    val tabStateHolder = rememberSaveableStateHolder()
                    tabStateHolder.SaveableStateProvider(key = state.activeTab) {
                        when (state.activeTab) {
                            AndroidTab.Home -> HomeTabContent(
                                state = state,
                                settings = settings,
                                listState = homeListState,
                                contentPadding = contentPadding,
                                modifier = Modifier.nestedScroll(navBarScroll),
                                onRefresh = vm::loadHome,
                                onSignIn = vm::openLoginSheet,
                                onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                                onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                                onTrackLongClick = { track, bounds -> openTrackHeldMenu(track, null, bounds) },
                                onCardLongClick = openCardHeldMenu,
                                onPlaylistLongClick = { playlist, bounds ->
                                    heldContextTarget = HeldContextTarget.AlbumOrPlaylistCard(
                                        browseId = playlist.id,
                                        title = playlist.name,
                                        subtitle = "Playlist",
                                        coverUrl = playlist.thumbnailUrl,
                                        isAlbum = false,
                                        eyebrow = "PLAYLIST",
                                        isTopPickTall = false,
                                        anchorBounds = bounds,
                                    )
                                },
                                onOpenCard = { card, shelfTracks -> vm.openCard(card, shelfTracks) },
                                onOpenPlaylist = vm::openPlaylist,
                                onOpenMood = vm::openMood,
                                onShowAllMoods = { showAllMoods = true },
                            )

                            AndroidTab.Explore -> ExploreTabContent(
                                state = state,
                                listState = exploreListState,
                                contentPadding = contentPadding,
                                modifier = Modifier.nestedScroll(navBarScroll),
                                onRefresh = vm::loadHome,
                                onOpenMood = vm::openMood,
                                onOpenCard = { card, shelfTracks -> vm.openCard(card, shelfTracks) },
                                onCardLongClick = { card, bounds -> openCardHeldMenu(card, null, false, bounds) },
                                onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                                onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                                onTrackLongClick = { track, idx, bounds -> openTrackHeldMenu(track, idx, bounds) },
                            )

                            AndroidTab.Radio -> {
                                TrackScrollActivity(radioListState)
                                RadioMixesScreen(
                                    homeShelves = state.shelves,
                                    radioMixShelves = state.radioMixShelves,
                                    activeTrack = state.activeTrack,
                                    contentPadding = contentPadding,
                                    onStartQuickRadio = { vm.startQuickRadio(state.activeTrack) },
                                    onCardClick = { card -> vm.openCard(card, emptyList()) },
                                    onCardLongClick = { card, bounds -> openCardHeldMenu(card, "RADIO MIX", false, bounds) },
                                    onMoodQueryClick = { title, query ->
                                        vm.openMood(title.ifBlank { query })
                                    },
                                    modifier = Modifier.nestedScroll(navBarScroll),
                                    listState = radioListState,
                                    lastPlayedTracks = state.lastPlayedTracks,
                                    refreshing = state.refreshing,
                                    onRefresh = vm::loadHome,
                                    onStartTrackRadio = { tr -> vm.startQuickRadio(tr) },
                                )
                            }

                            AndroidTab.Library -> {
                                val downloads by vm.downloads.collectAsStateWithLifecycle()
                                LibraryTabContent(
                                    state = state,
                                    downloadsCount = downloads.size,
                                    latestDownloadCoverUrl = downloads.firstOrNull()?.coverUrl,
                                    listState = libraryListState,
                                    contentPadding = contentPadding,
                                    modifier = Modifier.nestedScroll(navBarScroll),
                                    onSignIn = vm::openLoginSheet,
                                    onOpenPlaylist = vm::openPlaylist,
                                    onPlaylistLongClick = { playlist, bounds ->
                                        heldContextTarget = HeldContextTarget.AlbumOrPlaylistCard(
                                            browseId = playlist.id,
                                            title = playlist.name,
                                            subtitle = if (playlist.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID) "Offline Library" else "Playlist",
                                            coverUrl = playlist.thumbnailUrl,
                                            isAlbum = false,
                                            eyebrow = "PLAYLIST",
                                            isTopPickTall = false,
                                            anchorBounds = bounds,
                                        )
                                    },
                                    onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                                    onOpenDownloads = vm::openDownloadsPlaylist,
                                    onOpenDownloadsManager = { showDownloadsSheet = true },
                                    onOpenArtist = { id, name -> vm.openArtist(id, name) },
                                )
                            }

                            AndroidTab.Search -> SearchTabContent(
                                state = state,
                                listState = searchListState,
                                contentPadding = contentPadding,
                                modifier = Modifier.nestedScroll(navBarScroll),
                                onSearchQuery = { q -> vm.searchDebounced(q) },
                                onRemoveRecentSearch = vm::removeRecentSearch,
                                onClearRecentSearches = vm::clearRecentSearches,
                                onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                                onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                                onTrackLongClick = { track, bounds -> openTrackHeldMenu(track, null, bounds) },
                                onOpenPlaylist = vm::openPlaylist,
                            )
                        }
                    }
                }
            }
        }

        // Full-screen ArtistScreen rendered as a sibling outside appBackdrop (never nests layerBackdrops)
        if (state.artistPage != null || state.artistLoading) {
            val downloads by vm.downloads.collectAsStateWithLifecycle()
            val downloadedIds = remember(downloads) { downloads.map { it.videoId }.toSet() }
            val artistData = remember(state.artistPage, downloadedIds) {
                state.artistPage?.toArtistScreenData(downloadedIds)
            }
            BackHandler(
                enabled = !showNowPlaying &&
                    !state.showSettingsSheet &&
                    !showUserMenu &&
                    trackOptionsTarget == null &&
                    heldContextTarget == null &&
                    state.addToPlaylistTargets.isEmpty() &&
                    !showDownloadsSheet &&
                    !showAudioPipeline &&
                    !showSleepTimer &&
                    !showAudioControls &&
                    state.collectionTitle == null,
            ) {
                vm.closeArtist()
            }
            ArtistScreen(
                artist = artistData,
                loading = state.artistLoading,
                activeTrackId = state.activeTrack?.id,
                userAvatarUrl = if (state.signedIn) state.accountProfile?.avatarUrl else null,
                backdrop = artistBackdrop,
                contentBottomPadding = PaddingValues(bottom = bottomBarInset),
                onBack = vm::closeArtist,
                onShareArtist = { data ->
                    shareTarget = SonataShareTarget.Artist(
                        artistId = data.id,
                        artistName = data.name,
                        subscribers = data.monthlyListeners ?: data.subscriberText,
                        avatarUrl = data.avatarUrl,
                    )
                },
                onAvatarClick = { showUserMenu = true },
                onPlayTopSongs = { data ->
                    val tracks = data.topSongs.map { it.track }
                    tracks.firstOrNull()?.let { first -> vm.playTrack(first, tracks) }
                },
                onShuffleTopSongs = { data ->
                    val tracks = data.topSongs.map { it.track }
                    if (tracks.isNotEmpty()) vm.toggleShuffleAndPlay(tracks)
                },
                onToggleSubscribe = { data ->
                    android.widget.Toast.makeText(
                        context,
                        if (data.isSubscribed) "Removed ${data.name}" else "Subscribed to ${data.name}",
                        android.widget.Toast.LENGTH_SHORT,
                    ).show()
                },
                onOpenRelease = { release ->
                    vm.openAlbumOrPlaylist(
                        id = release.id,
                        title = release.title,
                        coverUrl = release.thumbnailUrl,
                        subtitle = release.subtitle,
                        kindLabel = release.kindLabel,
                        year = release.year,
                    )
                },
                onSaveRelease = { release ->
                    vm.openAlbumOrPlaylist(
                        id = release.id,
                        title = release.title,
                        coverUrl = release.thumbnailUrl,
                        subtitle = release.subtitle,
                        kindLabel = release.kindLabel,
                        year = release.year,
                    )
                },
                onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                onTrackOptions = { track, bounds -> trackOptionsTarget = TrackMenuRequest(track, bounds) },
                onTrackLongClick = { track, trackNum, bounds -> openTrackHeldMenu(track, trackNum, bounds) },
                onReleaseLongClick = { release, bounds ->
                    heldContextTarget = HeldContextTarget.AlbumOrPlaylistCard(
                        browseId = release.id,
                        title = release.title,
                        subtitle = release.subtitle,
                        coverUrl = release.thumbnailUrl,
                        isAlbum = release.kindLabel.equals("Album", ignoreCase = true) ||
                            release.kindLabel.equals("Single", ignoreCase = true),
                        eyebrow = release.kindLabel.uppercase(),
                        isTopPickTall = false,
                        anchorBounds = bounds,
                    )
                },
                onOpenAllTopSongs = { data ->
                    val tracks = data.topSongs.map { it.track }
                    tracks.firstOrNull()?.let { first -> vm.playTrack(first, tracks) }
                },
                onOpenSimilarArtist = { similar -> vm.openArtist(similar.id, similar.name) },
                modifier = Modifier
                    .fillMaxSize()
                    .nestedScroll(navBarScroll),
            )
        }

        // Top progressive fade scrim behind floating top-bar controls when scrolled
        val topFadeAlpha by animateFloatAsState(
            targetValue = if (scrolled && state.collectionTitle == null && state.artistPage == null && !state.artistLoading) 1f else 0f,
            animationSpec = tween(180),
            label = "topFadeAlpha",
        )
        if (topFadeAlpha > 0.01f) {
            TopFadeBlur(
                pageColor = MaterialTheme.colorScheme.background,
                topHeight = topBarHeight,
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .graphicsLayer { alpha = topFadeAlpha },
            )
        }

        // Floating sibling top chrome (hidden when ArtistScreen or full-screen Settings owns its top bar)
        if (state.artistPage == null && !state.artistLoading) {
            FloatingTopChrome(
                state = state,
                showAllMoods = showAllMoods,
                showListeningHistory = showListeningHistory,
                onBack = {
                    when {
                        state.collectionTitle != null -> vm.closeCollection()
                        showListeningHistory -> showListeningHistory = false
                        showAllMoods -> showAllMoods = false
                    }
                },
                onQueryChange = vm::searchDebounced,
                onSearchSubmit = { vm.search() },
                onShareCollectionClick = {
                    val browseId = state.collectionBrowseId.orEmpty()
                    val colTitle = state.collectionTitle.orEmpty()
                    if (colTitle.isNotBlank()) {
                        shareTarget = SonataShareTarget.Collection(
                            browseId = browseId,
                            collectionTitle = colTitle,
                            collectionSubtitle = state.collectionSubtitle.orEmpty(),
                            coverUrl = state.collectionCoverUrl
                                ?: state.collectionTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
                            isAlbum = state.collectionKindLabel.equals("Album", ignoreCase = true) ||
                                state.collectionKindLabel.equals("Single", ignoreCase = true) ||
                                browseId.startsWith("MPRE"),
                            trackCount = state.collectionTracks.size.takeIf { it > 0 },
                            year = state.collectionYear,
                        )
                    }
                },
                onHistoryClick = {
                    showAllMoods = false
                    vm.closeCollection()
                    vm.refreshListeningHistory()
                    showListeningHistory = true
                },
                onAvatarClick = { showUserMenu = true },
                modifier = Modifier.align(Alignment.TopCenter),
            )
        }

        // Bottom fade scrim behind the floating Liquid Glass navigation bar
        BottomFadeScrim(
            withMiniPlayer = state.activeTrack != null,
            pageColor = MaterialTheme.colorScheme.background,
            modifier = Modifier.align(Alignment.BottomCenter),
        )

        // Floating Liquid Glass Tab Bar + MiniPlayer + circular Search pill
        CompositionLocalProvider(LocalAppBackdrop provides activeOverlayBackdrop) {
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .widthIn(max = FLOATING_BAR_MAX_WIDTH)
                    .fillMaxWidth(),
            ) {
                SonataGlassNavBar(
                    selectedTab = state.activeTab,
                    onTabSelected = { tab ->
                        showAllMoods = false
                        showListeningHistory = false
                        vm.closeArtist()
                        navigateToTab(tab)
                    },
                    scrollConnection = navBarScroll,
                    activeTrack = state.activeTrack,
                    isPlaying = state.isPlaying,
                    isBuffering = state.isBuffering,
                    onPlayPause = vm::togglePlayPause,
                    onNext = vm::nextTrack,
                    onPrevious = vm::prevTrack,
                    onExpand = { showNowPlaying = true },
                )
            }
        }

        // Liquid Glass 3-dot Track Options Menu (sibling OUTSIDE appBackdrop & ArtistScreen — zero RenderNode cycles)
        if (trackOptionsTarget != null) {
            val menuDownloads by vm.downloads.collectAsStateWithLifecycle()
            val menuActiveDownloads by vm.activeDownloads.collectAsStateWithLifecycle()
            val activeTarget = trackOptionsTarget
            TrackOptionsMenu(
                track = activeTarget?.track,
                isLiked = activeTarget?.track?.id?.let { it in state.likedTrackIds } == true,
                isDownloaded = activeTarget?.track?.id?.let { id -> menuDownloads.any { it.videoId == id } } == true,
                activeDownload = activeTarget?.track?.id?.let { menuActiveDownloads[it] },
                backdrop = activeOverlayBackdrop,
                anchorBounds = activeTarget?.anchorBounds,
                onDismiss = { trackOptionsTarget = null },
                onToggleLike = { t -> vm.toggleStarTrack(t.id) },
                onToggleDislike = { t -> vm.toggleStarTrack(t.id) },
                onShareInApp = { t -> shareTarget = SonataShareTarget.Track(t) },
                onAddToPlaylist = { t -> vm.openAddToPlaylist(listOf(t)) },
                onStartRadio = { t -> vm.startQuickRadio(t) },
                onPlayNext = { t -> vm.playNext(t) },
                onAddToQueue = { t -> vm.addToQueue(t) },
                onDownloadTrack = { t -> vm.downloadTrack(t) },
                onRemoveFromCache = { t -> vm.removeTrackFromCache(t.id) },
                onOpenAlbum = { t ->
                    vm.openAlbum(
                        albumId = t.albumId.orEmpty(),
                        albumTitle = t.album.ifBlank { t.title },
                        artistName = t.artist,
                        coverUrl = t.thumbnailUrl,
                    )
                },
                onOpenArtist = { t ->
                    vm.openArtist("", t.artist)
                },
                onSongInfo = {
                    showAudioPipeline = true
                },
            )
        }

        // iOS-style Held Long-Press Context Menu (sibling OUTSIDE appBackdrop & ArtistScreen — zero RenderNode cycles)
        run {
            val heldDownloads by vm.downloads.collectAsStateWithLifecycle()
            val heldActiveDownloads by vm.activeDownloads.collectAsStateWithLifecycle()
            val targetTrack = (heldContextTarget as? HeldContextTarget.TrackRow)?.track
            HeldContextMenu(
                target = heldContextTarget,
                isLiked = targetTrack?.id?.let { it in state.likedTrackIds } == true,
                isDisliked = false,
                isDownloaded = targetTrack?.id?.let { id -> heldDownloads.any { it.videoId == id } } == true,
                activeDownload = targetTrack?.id?.let { heldActiveDownloads[it] },
                backdrop = activeOverlayBackdrop,
                onDismiss = { heldContextTarget = null },
                onToggleLike = { t -> vm.toggleStarTrack(t.id) },
                onToggleDislike = { t -> vm.toggleStarTrack(t.id) },
                onShareTrack = { t -> shareTarget = SonataShareTarget.Track(t) },
                onAddTrackToPlaylist = { t -> vm.openAddToPlaylist(listOf(t)) },
                onStartRadio = { t -> vm.startQuickRadio(t) },
                onPlayTrackNext = { t -> vm.playNext(t) },
                onAddTrackToQueue = { t -> vm.addToQueue(t) },
                onDownloadTrack = { t -> vm.downloadTrack(t) },
                onRemoveFromCache = { t -> vm.removeTrackFromCache(t.id) },
                onOpenAlbum = { t ->
                    vm.openAlbum(
                        albumId = t.albumId.orEmpty(),
                        albumTitle = t.album.ifBlank { t.title },
                        artistName = t.artist,
                        coverUrl = t.thumbnailUrl,
                    )
                },
                onOpenArtist = { t -> vm.openArtist("", t.artist) },
                onSongInfo = { showAudioPipeline = true },
                onPlayCollection = { col ->
                    if (col.browseId == state.collectionBrowseId && state.collectionTracks.isNotEmpty()) {
                        state.collectionTracks.firstOrNull()?.let { vm.playTrack(it, state.collectionTracks) }
                    } else if (col.browseId == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
                        vm.openDownloadsPlaylist()
                    } else {
                        vm.openAlbumOrPlaylist(
                            id = col.browseId,
                            title = col.title,
                            coverUrl = col.coverUrl,
                            subtitle = col.subtitle,
                            kindLabel = if (col.isAlbum) "Album" else "Playlist",
                        )
                    }
                },
                onShuffleCollection = { col ->
                    if (col.browseId == state.collectionBrowseId && state.collectionTracks.isNotEmpty()) {
                        vm.toggleShuffleAndPlay(state.collectionTracks)
                    } else {
                        vm.playCollectionShuffled(
                            browseId = col.browseId,
                            title = col.title,
                            subtitle = col.subtitle,
                            coverUrl = col.coverUrl,
                        )
                    }
                },
                onAddCollectionToPlaylist = { col ->
                    if (col.browseId == state.collectionBrowseId && state.collectionTracks.isNotEmpty()) {
                        vm.openAddToPlaylist(state.collectionTracks)
                    } else {
                        collectionPickerMeta = col
                        pendingAddToPlaylistBrowseId = col.browseId
                        vm.openAlbumOrPlaylist(
                            id = col.browseId,
                            title = col.title,
                            coverUrl = col.coverUrl,
                            subtitle = col.subtitle,
                            kindLabel = if (col.isAlbum) "Album" else "Playlist",
                        )
                    }
                },
                onPlayCollectionNext = { col -> vm.enqueueCollectionNext(col.browseId) },
                onAddCollectionToQueue = { col -> vm.enqueueCollectionLater(col.browseId) },
                onDownloadCollection = { col ->
                    if (col.browseId == state.collectionBrowseId && state.collectionTracks.isNotEmpty()) {
                        vm.downloadCollection(
                            tracks = state.collectionTracks,
                            collectionId = col.browseId.ifBlank { col.title },
                        )
                    } else {
                        vm.downloadCollectionById(col.browseId)
                    }
                },
                onOpenCollection = { col ->
                    if (col.browseId == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
                        vm.openDownloadsPlaylist()
                    } else {
                        vm.openAlbumOrPlaylist(
                            id = col.browseId,
                            title = col.title,
                            coverUrl = col.coverUrl,
                            subtitle = col.subtitle,
                            kindLabel = if (col.isAlbum) "Album" else "Playlist",
                        )
                    }
                },
                onShareCollection = { col ->
                    shareTarget = SonataShareTarget.Collection(
                        browseId = col.browseId,
                        collectionTitle = col.title,
                        collectionSubtitle = col.subtitle,
                        coverUrl = col.coverUrl,
                        isAlbum = col.isAlbum,
                    )
                },
            )
        }

        // Floating Liquid Glass Account Menu (sibling OUTSIDE appBackdrop & ArtistScreen — zero RenderNode cycles)
        val userMenuDownloads by vm.downloads.collectAsStateWithLifecycle()
        UserMenuSheet(
            visible = showUserMenu,
            signedIn = state.signedIn,
            accountProfile = state.accountProfile,
            settings = settings,
            downloadedCount = userMenuDownloads.size,
            backdrop = activeOverlayBackdrop,
            onDismiss = { showUserMenu = false },
            onSignIn = vm::openLoginSheet,
            onSignOut = vm::signOut,
            onOpenDownloads = { showDownloadsSheet = true },
            onOpenHistory = {
                showAllMoods = false
                vm.closeCollection()
                vm.refreshListeningHistory()
                showListeningHistory = true
            },
            onOpenAudioPipeline = { showAudioPipeline = true },
            onOpenSettings = vm::openSettingsSheet,
            onToggleLiquidGlass = { vm.updateSettings { it.copy(liquidGlassEnabled = !it.liquidGlassEnabled) } },
            onToggleAnimatedArt = { vm.updateSettings { it.copy(animatedArtwork = !it.animatedArtwork) } },
            onCycleThemeMode = {
                val modes = listOf(
                    ThemeMode.VIOLET_LIGHT,
                    ThemeMode.VIOLET_DARK,
                    ThemeMode.MOONFLOWER_LIGHT,
                    ThemeMode.MOONFLOWER_DARK,
                )
                val next = modes[(modes.indexOf(settings.themeMode).coerceAtLeast(-1) + 1) % modes.size]
                vm.updateSettings {
                    it.copy(
                        themeMode = next,
                        designTheme = when (next) {
                            ThemeMode.VIOLET_LIGHT, ThemeMode.VIOLET_DARK -> DesignTheme.VIOLET
                            else -> DesignTheme.MOONFLOWER
                        },
                    )
                }
            },
            onRefreshLibrary = vm::loadHome,
        )

        // Full-screen Flat Neutral Settings Page (zero Liquid Glass)
        AnimatedVisibility(
            visible = state.showSettingsSheet,
            enter = slideInVertically(tween(240)) { it / 8 } + fadeIn(tween(180)),
            exit = slideOutVertically(tween(200)) { it / 8 } + fadeOut(tween(150)),
            modifier = Modifier.fillMaxSize(),
        ) {
            BackHandler(enabled = state.showSettingsSheet) { vm.dismissSettingsSheet() }
            SonataSettingsScreen(
                settings = settings,
                signedIn = state.signedIn,
                accountProfile = state.accountProfile,
                supports120Hz = state.supports120Hz,
                statusMessage = state.status,
                updateInfo = state.updateInfo,
                checkingUpdates = state.checkingUpdates,
                installedVersionName = state.installedVersionName,
                onUpdateSettings = vm::updateSettings,
                onSignIn = vm::openLoginSheet,
                onSignOut = vm::signOut,
                onClearMediaCache = vm::clearMediaCache,
                onExportBackup = { exportBackupLauncher.launch("sonata-backup.json") },
                onImportBackup = { importBackupLauncher.launch(arrayOf("application/json", "*/*")) },
                onCheckForUpdates = { vm.checkForUpdates(autoPopup = true, isManual = true) },
                onDownloadUpdate = vm::downloadAndInstallUpdate,
                onBack = vm::dismissSettingsSheet,
            )
        }

        // Fullscreen Pull-Up NowPlayingScreen
        AnimatedVisibility(
            visible = showNowPlaying && state.activeTrack != null,
            enter = slideInVertically(
                animationSpec = spring(dampingRatio = 0.86f, stiffness = 360f),
                initialOffsetY = { it },
            ) + fadeIn(tween(200)),
            exit = slideOutVertically(
                animationSpec = spring(dampingRatio = 0.92f, stiffness = 400f),
                targetOffsetY = { it },
            ) + fadeOut(tween(180)),
            modifier = Modifier.fillMaxSize(),
        ) {
            state.activeTrack?.let { track ->
                BackHandler(enabled = showNowPlaying) { showNowPlaying = false }
                val downloads by vm.downloads.collectAsStateWithLifecycle()
                val activeDownloads by vm.activeDownloads.collectAsStateWithLifecycle()
                val currentCanvas by vm.currentCanvas.collectAsStateWithLifecycle()
                val audioPipelineInfo by vm.audioPipelineInfo.collectAsStateWithLifecycle()
                val sleepTimerRemaining by vm.sleepTimerRemainingMinutes.collectAsStateWithLifecycle()
                val activeLyricsSource by vm.activeLyricsSource.collectAsStateWithLifecycle()
                val isTrackDownloaded = remember(downloads, track.id) {
                    downloads.any { it.videoId == track.id }
                }
                val activeTrackDownload = activeDownloads[track.id]
                var trackLyricOffsetMs by remember(track.id, settings.lyricsOffsetMs) {
                    mutableLongStateOf(vm.getTrackLyricOffsetMs(track.id))
                }
                NowPlayingScreen(
                    track = track,
                    state = state,
                    settings = settings,
                    currentCanvas = currentCanvas,
                    audioPipelineSummary = audioPipelineInfo.codec.ifBlank { null },
                    isTrackDownloaded = isTrackDownloaded,
                    activeTrackDownload = activeTrackDownload,
                    sleepTimerRemaining = sleepTimerRemaining,
                    activeLyricsSource = activeLyricsSource,
                    trackLyricOffsetMs = trackLyricOffsetMs,
                    clock = vm.playbackClock,
                    onToggleLyrics = vm::toggleLyrics,
                    onToggleStar = { vm.toggleStarTrack(track.id) },
                    onTogglePlayPause = vm::togglePlayPause,
                    onNext = vm::nextTrack,
                    onPrevious = vm::prevTrack,
                    onToggleShuffle = vm::toggleShuffle,
                    onToggleRepeat = vm::toggleRepeatMode,
                    onSeekTo = vm::seekTo,
                    onSeekFraction = vm::seekToFraction,
                    onJumpToQueueIndex = vm::jumpToQueueIndex,
                    onDownloadTrack = { vm.downloadTrack(track) },
                    onOpenAudioControls = { showAudioControls = true },
                    onOpenSleepTimer = { showSleepTimer = true },
                    onOpenAudioPipeline = { showAudioPipeline = true },
                    onToggleLyricsTranslation = {
                        vm.updateSettings { it.copy(lyricsTranslationEnabled = !it.lyricsTranslationEnabled) }
                    },
                    onToggleLyricsRomanization = {
                        vm.updateSettings { it.copy(lyricsRomanizationEnabled = !it.lyricsRomanizationEnabled) }
                    },
                    onStepLyricOffsetMs = { delta ->
                        val next = (trackLyricOffsetMs + delta).coerceIn(-5000L, 5000L)
                        trackLyricOffsetMs = next
                        vm.setTrackLyricOffsetMs(track.id, next)
                    },
                    onResetLyricOffsetMs = {
                        trackLyricOffsetMs = 0L
                        vm.setTrackLyricOffsetMs(track.id, 0L)
                    },
                    onShareInApp = {
                        shareTarget = SonataShareTarget.Track(track)
                    },
                    onStartRadio = { vm.startQuickRadio(track) },
                    onPlayNext = { vm.playNext(track) },
                    onAddToQueue = { vm.addToQueue(track) },
                    onOpenAlbum = {
                        showNowPlaying = false
                        vm.openAlbum(
                            albumId = track.albumId.orEmpty(),
                            albumTitle = track.album.ifBlank { track.title },
                            artistName = track.artist,
                            coverUrl = track.thumbnailUrl,
                        )
                    },
                    onOpenArtist = {
                        showNowPlaying = false
                        vm.openArtist("", track.artist)
                    },
                    onOpenSettings = {
                        showNowPlaying = false
                        vm.openSettingsSheet()
                    },
                    onAddToPlaylist = {
                        vm.openAddToPlaylist(listOf(track))
                    },
                    onDismiss = { showNowPlaying = false },
                )
            }
        }

        // Visual Share Card modal (Song / Album / Playlist / Artist)
        SonataShareSheet(
            target = shareTarget,
            backdrop = activeOverlayBackdrop,
            onDismiss = { shareTarget = null },
            onStatusMessage = { msg -> visiblePlaylistStatus = msg },
        )

        // Full-screen Add to Playlist picker (rendered above NowPlaying & ArtistScreen)
        AddToPlaylistSheet(
            visible = state.addToPlaylistTargets.isNotEmpty(),
            targets = state.addToPlaylistTargets,
            playlists = state.playlists,
            overrideTitle = collectionPickerMeta?.title,
            overrideSubtitle = collectionPickerMeta?.subtitle,
            overrideCoverUrl = collectionPickerMeta?.coverUrl,
            onDismiss = {
                collectionPickerMeta = null
                vm.dismissAddToPlaylist()
            },
            onConfirmAdd = { playlistIds, tracks ->
                collectionPickerMeta = null
                vm.addTracksToPlaylists(tracks, playlistIds)
                vm.dismissAddToPlaylist()
            },
            onCreatePlaylist = { title, tracks ->
                collectionPickerMeta = null
                vm.createPlaylistWithTracks(title, tracks)
            },
        )

        // Brief floating feedback pill when tracks are added to a playlist
        AnimatedVisibility(
            visible = !visiblePlaylistStatus.isNullOrBlank(),
            enter = slideInVertically(tween(220)) { it / 2 } + fadeIn(tween(180)),
            exit = slideOutVertically(tween(180)) { it / 2 } + fadeOut(tween(150)),
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .navigationBarsPadding()
                .padding(bottom = 112.dp, start = 20.dp, end = 20.dp),
        ) {
            Row(
                modifier = Modifier
                    .clip(CapsulePillShape)
                    .background(themeTokens.eclipseSurface.copy(alpha = 0.96f))
                    .border(0.75.dp, themeTokens.cardStroke, CapsulePillShape)
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = SonataIcons.Check,
                    contentDescription = null,
                    tint = themeTokens.petalLavender,
                    modifier = Modifier.size(18.dp),
                )
                Text(
                    text = visiblePlaylistStatus.orEmpty(),
                    style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Medium),
                    color = themeTokens.onSurfacePrimary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}


private fun AndroidArtistPage.toArtistScreenData(downloadedIds: Set<String>): ArtistScreenData {
    val latest = latestRelease?.let { card ->
        ArtistReleaseItem(
            id = card.id,
            title = card.title,
            subtitle = card.subtitle,
            kindLabel = if (card.kind == AndroidHomeCardKind.ALBUM) "Album" else "Single",
            thumbnailUrl = card.thumbnailUrl,
        )
    }
    val songs = popularTracks.map { t ->
        ArtistTrackItem(
            track = t,
            isExplicit = t.isExplicit,
            isDownloaded = t.id in downloadedIds,
        )
    }
    val albumItems = releases
        .filter { card ->
            card.kind == AndroidHomeCardKind.ALBUM &&
                !card.subtitle.contains("Single", ignoreCase = true) &&
                !card.subtitle.contains("EP", ignoreCase = true)
        }
        .map { card ->
            ArtistReleaseItem(
                id = card.id,
                title = card.title,
                subtitle = card.subtitle,
                kindLabel = "Album",
                thumbnailUrl = card.thumbnailUrl,
            )
        }
    val singleItems = releases
        .filter { card ->
            card.kind != AndroidHomeCardKind.ALBUM ||
                card.subtitle.contains("Single", ignoreCase = true) ||
                card.subtitle.contains("EP", ignoreCase = true)
        }
        .map { card ->
            ArtistReleaseItem(
                id = card.id,
                title = card.title,
                subtitle = card.subtitle,
                kindLabel = if (card.subtitle.contains("EP", ignoreCase = true)) "EP" else "Single",
                thumbnailUrl = card.thumbnailUrl,
            )
        }
    val similar = relatedArtists.map { card ->
        ArtistSimilarItem(
            id = card.id,
            name = card.title,
            subtitle = card.subtitle.takeIf { it.isNotBlank() },
            thumbnailUrl = card.thumbnailUrl,
        )
    }
    return ArtistScreenData(
        id = id,
        name = name,
        subscriberText = subscribers ?: subtitle.takeIf { it.isNotBlank() },
        monthlyListeners = monthlyListeners?.takeIf { it.isNotBlank() },
        aboutBio = description?.takeIf { it.isNotBlank() },
        avatarUrl = thumbnailUrl,
        latestRelease = latest,
        topSongs = songs,
        albums = albumItems,
        singles = singleItems,
        similarArtists = similar,
    )
}
