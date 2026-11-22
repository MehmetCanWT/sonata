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
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
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
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
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
import dev.sonata.android.ui.DiscoveryStationArt
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
import dev.sonata.android.ui.PersonalStationArt
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
import dev.sonata.android.ui.rememberSmoothLyricClock
import dev.sonata.android.ui.toMeshPalette
import dev.sonata.android.ui.topPicksSkeleton
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.CanvasResolver
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

private val TopBarContentHeight = 52.dp
private val NavBarLift = 10.dp
private val InlineSpacing = 12.dp
private val InlineGutter = 24.dp

private val Shape7 = RoundedCornerShape(7.dp)
private val Shape8 = RoundedCornerShape(8.dp)
private val Shape10 = RoundedCornerShape(10.dp)
private val Shape11 = RoundedCornerShape(11.dp)
private val Shape12 = RoundedCornerShape(12.dp)
private val Shape14 = RoundedCornerShape(14.dp)
private val Shape20 = RoundedCornerShape(20.dp)
private val Shape22 = RoundedCornerShape(22.dp)
private val Pill50Shape = RoundedCornerShape(percent = 50)
private val CapsulePillShape = SquirclePillShape()
private val NowPlayingBottomScrim = Brush.verticalGradient(
    0.38f to Color.Transparent,
    1f to Color.Black.copy(alpha = 0.46f),
)

private data class NavTabSpec(
    val tab: AndroidTab,
    val label: String,
    val icon: ImageVector,
)

private val PrimaryTabs = listOf(
    NavTabSpec(AndroidTab.Home, "Home", SonataIcons.Home),
    NavTabSpec(AndroidTab.Explore, "Explore", SonataIcons.TabExplore),
    NavTabSpec(AndroidTab.Radio, "Radio", SonataRadioWavesIcon),
    NavTabSpec(AndroidTab.Library, "Library", SonataIcons.TabLibrary),
    NavTabSpec(AndroidTab.Search, "Search", SonataIcons.TabSearch),
)

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
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        setContent {
            SonataApp()
        }
    }
}

@Composable
private fun Modifier.adaptiveLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
    surfaceTint: Color? = null,
    chromaticAberration: Boolean = true,
): Modifier = if (LocalLiquidGlassEnabled.current && isGlassSupported()) {
    this.liquidGlass(
        shape = shape,
        surfaceTint = surfaceTint,
        chromaticAberration = chromaticAberration,
    )
} else {
    this.lightweightLiquidGlass(shape = shape, fallbackColor = fallbackColor)
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
    val darkTheme = when (settings.themeMode) {
        ThemeMode.LIGHT -> false
        ThemeMode.DARK, ThemeMode.AMOLED -> true
        ThemeMode.SYSTEM -> androidx.compose.foundation.isSystemInDarkTheme()
    }
    val amoled = settings.themeMode == ThemeMode.AMOLED
    SonataTheme(darkTheme = darkTheme, amoled = amoled) {
        SystemBarIcons(dark = !darkTheme)
        val windowBackground = MaterialTheme.colorScheme.background
        val paintBackdrop: ContentDrawScope.() -> Unit = remember(windowBackground) {
            {
                drawRect(windowBackground)
                drawContent()
            }
        }
        val appBackdrop = rememberLayerBackdrop(onDraw = paintBackdrop)
        val glassEnabled = settings.liquidGlassEnabled && isGlassSupported()

        CompositionLocalProvider(
            LocalLiquidGlassEnabled provides glassEnabled,
            LocalGlassPerformance provides settings.glassPerformance,
            LocalReduceDynamicBlur provides settings.reduceDynamicBlur,
            LocalReduceAnimation provides settings.reduceAnimation,
            LocalAppBackdrop provides appBackdrop,
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
    var showUserMenu by remember { mutableStateOf(false) }
    var showDownloadsSheet by remember { mutableStateOf(false) }
    var showAudioPipeline by remember { mutableStateOf(false) }
    var showSleepTimer by remember { mutableStateOf(false) }
    var showAudioControls by remember { mutableStateOf(false) }
    var trackOptionsTarget by remember { mutableStateOf<TrackMenuRequest?>(null) }
    var heldContextTarget by remember { mutableStateOf<HeldContextTarget?>(null) }
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

    val exportBackupLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        if (uri != null) {
            runCatching {
                context.contentResolver.openOutputStream(uri)?.use { out ->
                    out.write(vm.exportSettingsBackup().toByteArray(Charsets.UTF_8))
                }
            }
        }
    }
    val importBackupLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri != null) {
            runCatching {
                val json = context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                if (!json.isNullOrBlank()) {
                    vm.importSettingsBackup(json)
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

    val statusTopPadding = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val topBarHeight = statusTopPadding + TopBarContentHeight
    // Fixed bottom inset keyed only on activeTrack presence so FloatingTabBar collapse/expand
    // never triggers full LazyColumn relayout mid-fling.
    val navBottomPadding = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val bottomBarInset = navBottomPadding + if (state.activeTrack != null) 144.dp else 92.dp

    val scrolled by remember(
        state.activeTab,
        state.collectionTitle,
        showAllMoods,
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
                showAllMoods -> true
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
            delay(2500L)
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
    if (!hasOverlayOpen && (state.collectionTitle != null || showAllMoods || canStepBackFromSearch || state.activeTab != AndroidTab.Home)) {
        BackHandler {
            when {
                state.collectionTitle != null -> vm.closeCollection()
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
            freshSignIn = !state.signedIn,
        )
        return
    }

    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner, vm) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                vm.onAppResumed()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
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
            eqEnabled = settings.eqEnabled,
            eqPreset = settings.eqPreset,
            eqBandsDb = settings.eqBandsDb,
            loudnessNorm = settings.loudnessNormEnabled,
            skipSilence = settings.skipSilenceEnabled,
            onSpeedChange = { speed -> vm.updateSettings { it.copy(playbackSpeed = speed) } },
            onEqToggle = { enabled -> vm.updateSettings { it.copy(eqEnabled = enabled) } },
            onEqPreset = { preset, bands -> vm.updateSettings { it.copy(eqPreset = preset, eqBandsDb = bands) } },
            onEqBandChange = { index, gainDb ->
                vm.updateSettings { current ->
                    val updated = current.eqBandsDb.toMutableList()
                    if (index in updated.indices) updated[index] = gainDb
                    current.copy(eqPreset = "Custom", eqBandsDb = updated)
                }
            },
            onLoudnessToggle = { enabled -> vm.updateSettings { it.copy(loudnessNormEnabled = enabled) } },
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
        heldContextTarget = HeldContextTarget.TrackRow(
            track = track,
            trackNumber = trackNumber,
            isExplicit = track.isExplicit,
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

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black),
    ) {
        // Scrollable backdrop source sampled by sibling floating Liquid Glass overlays
        // (ArtistScreen renders as its own sibling outside appBackdrop so floating Liquid Glass never cycles).
        Box(
            modifier = Modifier
                .fillMaxSize()
                .layerBackdrop(appBackdrop),
        ) {
            val topPad = if (state.activeTab == AndroidTab.Search && state.collectionTitle == null && !showAllMoods) {
                topBarHeight + 10.dp
            } else if (state.collectionTitle != null) {
                topBarHeight + 8.dp
            } else {
                statusTopPadding + 14.dp
            }
            val contentPadding = PaddingValues(top = topPad, bottom = bottomBarInset)

            when {
                state.collectionTitle != null -> DetailScreen(
                    state = state,
                    listState = collectionListState,
                    contentPadding = contentPadding,
                    modifier = Modifier.nestedScroll(navBarScroll),
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
                            collectionId = state.collectionTitle ?: "collection",
                        )
                    },
                    onShare = {
                        shareInAppText(
                            context = context,
                            title = state.collectionTitle.orEmpty(),
                            subtitle = state.collectionSubtitle.orEmpty(),
                        )
                    },
                    onOpenArtist = { artistName ->
                        vm.openArtist("", artistName)
                    },
                )

                showAllMoods -> AllMoodsScreen(
                    contentPadding = PaddingValues(top = topBarHeight + 8.dp, bottom = bottomBarInset),
                    modifier = Modifier.nestedScroll(navBarScroll),
                    onOpenMood = { title ->
                        showAllMoods = false
                        vm.openMood(title)
                    },
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
                                onOpenMood = vm::openMood,
                                onOpenCard = { card, shelfTracks -> vm.openCard(card, shelfTracks) },
                                onCardLongClick = { card, bounds -> openCardHeldMenu(card, null, false, bounds) },
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
                    shareInAppText(context, data.name, data.subscriberText ?: "Artist on Sonata")
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
                pageColor = Color.Black,
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
                onBack = {
                    when {
                        state.collectionTitle != null -> vm.closeCollection()
                        showAllMoods -> showAllMoods = false
                    }
                },
                onQueryChange = vm::searchDebounced,
                onSearchSubmit = { vm.search() },
                onHistoryClick = { navigateToTab(AndroidTab.Search) },
                onAvatarClick = { showUserMenu = true },
                modifier = Modifier.align(Alignment.TopCenter),
            )
        }

        // Bottom fade scrim behind the floating Liquid Glass navigation bar
        BottomFadeScrim(
            withMiniPlayer = state.activeTrack != null,
            pageColor = Color.Black,
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
                onShareInApp = { t -> shareInAppText(context, t.title, t.artist) },
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
                onShareTrack = { t -> shareInAppText(context, t.title, t.artist) },
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
                onShuffleCollection = { col ->
                    vm.playCollectionShuffled(
                        browseId = col.browseId,
                        title = col.title,
                        subtitle = col.subtitle,
                        coverUrl = col.coverUrl,
                    )
                },
                onAddCollectionToPlaylist = { col ->
                    collectionPickerMeta = col
                    pendingAddToPlaylistBrowseId = col.browseId
                    vm.openAlbumOrPlaylist(
                        id = col.browseId,
                        title = col.title,
                        coverUrl = col.coverUrl,
                        subtitle = col.subtitle,
                        kindLabel = if (col.isAlbum) "Album" else "Playlist",
                    )
                },
                onPlayCollectionNext = { col -> vm.enqueueCollectionNext(col.browseId) },
                onAddCollectionToQueue = { col -> vm.enqueueCollectionLater(col.browseId) },
                onDownloadCollection = { col -> vm.downloadCollectionById(col.browseId) },
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
                    shareInAppText(
                        context = context,
                        title = col.title,
                        subtitle = col.subtitle.ifBlank {
                            if (col.isAlbum) "Album on Sonata" else "Playlist on Sonata"
                        },
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
            onOpenHistory = { navigateToTab(AndroidTab.Search) },
            onOpenAudioPipeline = { showAudioPipeline = true },
            onOpenSettings = vm::openSettingsSheet,
            onToggleLiquidGlass = { vm.updateSettings { it.copy(liquidGlassEnabled = !it.liquidGlassEnabled) } },
            onToggleAnimatedArt = { vm.updateSettings { it.copy(animatedArtwork = !it.animatedArtwork) } },
            onCycleThemeMode = {
                val modes = ThemeMode.entries
                val next = modes[(modes.indexOf(settings.themeMode) + 1) % modes.size]
                vm.updateSettings { it.copy(themeMode = next) }
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
                        shareInAppText(context, track.title, track.artist)
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
                    .background(Color(0xEE1C1C22))
                    .border(0.5.dp, Color.White.copy(alpha = 0.16f), CapsulePillShape)
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = SonataIcons.Check,
                    contentDescription = null,
                    tint = SonataAccent,
                    modifier = Modifier.size(18.dp),
                )
                Text(
                    text = visiblePlaylistStatus.orEmpty(),
                    style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Medium),
                    color = Color.White,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun FloatingTopChrome(
    state: SonataUiState,
    showAllMoods: Boolean,
    onBack: () -> Unit,
    onQueryChange: (String) -> Unit,
    onSearchSubmit: () -> Unit,
    onHistoryClick: () -> Unit,
    onAvatarClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val pillShape = remember { CircleShape }
    val showBack = state.collectionTitle != null || showAllMoods
    val showSearch = state.activeTab == AndroidTab.Search && !showBack

    Row(
        modifier = modifier
            .fillMaxWidth()
            .statusBarsPadding()
            .height(TopBarContentHeight)
            .padding(horizontal = PAGE_GUTTER),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (showBack) {
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .adaptiveLiquidGlass(shape = pillShape, chromaticAberration = false)
                    .clip(pillShape)
                    .clickable(onClick = onBack),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataIcons.ChevronLeft,
                    contentDescription = "Back",
                    tint = Color.White,
                    modifier = Modifier.size(22.dp),
                )
            }
        }

        if (showSearch) {
            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(46.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                SearchField(
                    query = state.query,
                    onQueryChange = onQueryChange,
                    onSubmit = onSearchSubmit,
                    placeholder = "Artists, Songs, Lyrics, and More",
                )
            }
        } else {
            if (showAllMoods) {
                Text(
                    text = "Find Your Mood",
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = Color.White,
                    modifier = Modifier.weight(1f),
                )
            } else {
                Spacer(Modifier.weight(1f))
            }
        }

        if (state.activeTab == AndroidTab.Library && !showBack) {
            val capsuleShape = remember { SquirclePillShape() }
            Row(
                modifier = Modifier
                    .height(44.dp)
                    .adaptiveLiquidGlass(shape = capsuleShape, chromaticAberration = false)
                    .clip(capsuleShape)
                    .padding(horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(36.dp)
                        .clip(CircleShape)
                        .clickable(onClick = onHistoryClick),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.History,
                        contentDescription = "History",
                        tint = Color.White,
                        modifier = Modifier.size(20.dp),
                    )
                }
                AvatarCircleContent(
                    profile = state.accountProfile,
                    signedIn = state.signedIn,
                    size = 34.dp,
                    onClick = onAvatarClick,
                )
            }
        } else {
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .adaptiveLiquidGlass(shape = pillShape, chromaticAberration = false)
                    .clip(pillShape)
                    .clickable(onClick = onAvatarClick),
                contentAlignment = Alignment.Center,
            ) {
                AvatarCircleContent(
                    profile = state.accountProfile,
                    signedIn = state.signedIn,
                    size = 38.dp,
                    onClick = onAvatarClick,
                )
            }
        }
    }
}

@Composable
private fun AvatarCircleContent(
    profile: AndroidAccountProfile?,
    signedIn: Boolean,
    size: Dp,
    onClick: () -> Unit,
) {
    val avatar = if (signedIn) profile?.avatarUrl else null
    val avatarRequest = rememberArtworkRequest(avatar, 128, 128)
    Box(
        modifier = Modifier
            .size(size)
            .clip(CircleShape)
            .background(Color.White.copy(alpha = 0.08f))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        if (avatarRequest != null) {
            AsyncImage(
                model = avatarRequest,
                contentDescription = "Account and Settings",
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize().clip(CircleShape),
            )
        } else {
            Icon(
                imageVector = SonataIcons.Person,
                contentDescription = "Account and Settings",
                tint = Color.White,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun SonataGlassNavBar(
    selectedTab: AndroidTab,
    onTabSelected: (AndroidTab) -> Unit,
    scrollConnection: FloatingTabBarScrollConnection,
    activeTrack: AndroidTrack?,
    isPlaying: Boolean,
    isBuffering: Boolean,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onExpand: () -> Unit,
) {
    val glassSupported = LocalLiquidGlassEnabled.current && isGlassSupported()
    val contentColor = glassContentColor()
    val selectedColor = contentColor
    val unselectedColor = contentColor.copy(alpha = 0.65f)
    val selectedIndex = PrimaryTabs.indexOfFirst { it.tab == selectedTab }.coerceAtLeast(0)
    val tabClickCallbacks = remember(onTabSelected) {
        PrimaryTabs.map { spec -> { onTabSelected(spec.tab) } }
    }
    val glassSurface: @Composable () -> Modifier = remember {
        {
            Modifier.adaptiveLiquidGlass(
                shape = Pill50Shape,
                fallbackColor = Color(0xFF141418).copy(alpha = 0.94f),
            )
        }
    }

    FloatingTabBar(
        selectedTabKey = selectedIndex,
        scrollConnection = scrollConnection,
        modifier = Modifier
            .navigationBarsPadding()
            .padding(horizontal = BAR_GUTTER)
            .padding(bottom = NavBarLift)
            .fillMaxWidth(),
        mergeStandaloneWhenExpanded = true,
        tabBarContentModifier = glassSurface,
        inlineAccessory = activeTrack?.let { track ->
            { accessoryModifier, _ ->
                GlassMiniPlayer(
                    track = track,
                    isInline = true,
                    isPlaying = isPlaying,
                    isBuffering = isBuffering,
                    contentColor = contentColor,
                    onPlayPause = onPlayPause,
                    onNext = onNext,
                    onPrevious = onPrevious,
                    onExpand = onExpand,
                    modifier = accessoryModifier.then(glassSurface()),
                )
            }
        },
        expandedAccessory = activeTrack?.let { track ->
            { accessoryModifier, _ ->
                GlassMiniPlayer(
                    track = track,
                    isInline = false,
                    isPlaying = isPlaying,
                    isBuffering = isBuffering,
                    contentColor = contentColor,
                    onPlayPause = onPlayPause,
                    onNext = onNext,
                    onPrevious = onPrevious,
                    onExpand = onExpand,
                    modifier = accessoryModifier.fillMaxWidth().then(glassSurface()),
                )
            }
        },
        colors = FloatingTabBarDefaults.colors(
            backgroundColor = if (glassSupported) Color.Transparent else Color(0xFF141418),
            accessoryBackgroundColor = if (glassSupported) Color.Transparent else Color(0xFF141418),
            indicatorColor = glassIndicatorColor(),
        ),
        elevations = FloatingTabBarDefaults.elevations(
            inlineElevation = 0.dp,
            expandedElevation = 0.dp,
        ),
        sizes = FloatingTabBarDefaults.sizes(
            tabBarContentPadding = PaddingValues(4.dp),
            tabExpandedContentPadding = PaddingValues(vertical = 8.dp),
            inlineComponentSpacing = InlineSpacing,
            inlineBarPadding = PaddingValues(
                start = InlineGutter - BAR_GUTTER,
                end = InlineGutter - BAR_GUTTER,
                bottom = 4.dp,
            ),
        ),
        contentKey = remember(selectedIndex, contentColor) {
            listOf(selectedIndex, contentColor)
        },
    ) {
        PrimaryTabs.forEachIndexed { index, spec ->
            val isSelected = index == selectedIndex
            val tint = if (isSelected) selectedColor else unselectedColor
            val onClick = tabClickCallbacks[index]
            if (index == PrimaryTabs.lastIndex) {
                standaloneTab(
                    key = index,
                    icon = {
                        Icon(
                            imageVector = spec.icon,
                            contentDescription = spec.label,
                            tint = tint,
                            modifier = Modifier.size(SonataTabIconSize),
                        )
                    },
                    onClick = onClick,
                    title = {
                        Text(
                            text = spec.label,
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 10.sp,
                                fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
                            ),
                            color = tint,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    },
                )
            } else {
                tab(
                    key = index,
                    icon = {
                        Icon(
                            imageVector = spec.icon,
                            contentDescription = spec.label,
                            tint = tint,
                            modifier = Modifier.size(SonataTabIconSize),
                        )
                    },
                    title = {
                        Text(
                            text = spec.label,
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 10.sp,
                                fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
                            ),
                            color = tint,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    },
                    onClick = onClick,
                )
            }
        }
    }
}

@Composable
private fun GlassMiniPlayer(
    track: AndroidTrack,
    isInline: Boolean,
    isPlaying: Boolean,
    isBuffering: Boolean,
    contentColor: Color,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onExpand: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val pressSource = remember { MutableInteractionSource() }
    val isPressed by pressSource.collectIsPressedAsState()
    val pressScale by animateFloatAsState(
        targetValue = if (isPressed) 1.03f else 1f,
        animationSpec = spring(stiffness = 400f),
        label = "miniPressScale",
    )

    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .height(if (isInline) 48.dp else 56.dp)
            .pointerInput(onNext, onPrevious) {
                var totalX = 0f
                detectHorizontalDragGestures(
                    onDragStart = { totalX = 0f },
                    onHorizontalDrag = { change, amount ->
                        change.consume()
                        totalX += amount
                    },
                    onDragEnd = {
                        if (totalX < -72f) onNext()
                        else if (totalX > 72f) onPrevious()
                    },
                )
            }
            .pointerInput(onExpand) {
                var totalY = 0f
                detectVerticalDragGestures(
                    onDragStart = { totalY = 0f },
                    onVerticalDrag = { change, amount ->
                        change.consume()
                        totalY += amount
                    },
                    onDragEnd = {
                        if (totalY < -54f) onExpand()
                    },
                )
            }
            .graphicsLayer {
                scaleX = pressScale
                scaleY = pressScale
            }
            .then(modifier)
            .clickable(
                interactionSource = pressSource,
                indication = null,
                onClick = onExpand,
            ),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    start = 12.dp,
                    end = if (isInline) 6.dp else 10.dp,
                    top = 5.dp,
                    bottom = 5.dp,
                ),
        ) {
            Box(
                modifier = Modifier
                    .size(if (isInline) 34.dp else 40.dp)
                    .clip(Shape8)
                    .background(Color.White.copy(alpha = 0.08f))
                    .border(0.5.dp, Color.White.copy(alpha = 0.14f), Shape8),
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
                        tint = contentColor.copy(alpha = 0.7f),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
            Spacer(Modifier.width(if (isInline) 8.dp else 10.dp))
            Column(
                modifier = Modifier
                    .weight(1f)
                    .clipToBounds(),
            ) {
                MarqueeText(
                    text = track.title,
                    style = MaterialTheme.typography.titleSmall.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = contentColor,
                    enabled = isPlaying,
                )
                Text(
                    text = track.artist,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                    color = contentColor.copy(alpha = 0.66f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (isBuffering) {
                Box(
                    modifier = Modifier.size(if (isInline) 34.dp else 40.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(
                        color = contentColor,
                        strokeWidth = 2.dp,
                        modifier = Modifier.size(18.dp),
                    )
                }
            } else {
                IconButton(
                    onClick = onPlayPause,
                    modifier = Modifier.size(if (isInline) 34.dp else 40.dp),
                ) {
                    Icon(
                        painter = painterResource(
                            if (isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play
                        ),
                        contentDescription = if (isPlaying) "Pause" else "Play",
                        tint = contentColor,
                        modifier = Modifier.size(if (isInline) 18.dp else 20.dp),
                    )
                }
            }
            if (!isInline) {
                IconButton(
                    onClick = onNext,
                    modifier = Modifier.size(36.dp),
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ic_player_next),
                        contentDescription = "Next",
                        tint = contentColor,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
    }
}

private sealed interface TopPickEntry {
    val stableKey: String
    val contentType: String

    data class Release(val card: AndroidHomeCard, val shelfTracks: List<AndroidTrack>) : TopPickEntry {
        override val stableKey: String get() = "release_${card.kind}_${card.id}_${card.title}"
        override val contentType: String get() = "top_pick_release"
    }

    data class ArtistMix(val card: AndroidHomeCard, val portraits: List<String>) : TopPickEntry {
        override val stableKey: String get() = "artist_mix_${card.id}_${card.title}"
        override val contentType: String get() = "top_pick_artist_mix"
    }

    data object PersonalStation : TopPickEntry {
        override val stableKey: String get() = "personal_station"
        override val contentType: String get() = "top_pick_personal"
    }

    data object DiscoveryStation : TopPickEntry {
        override val stableKey: String get() = "discovery_station"
        override val contentType: String get() = "top_pick_discovery"
    }
}

private fun homeShelfSortOrder(title: String): Int {
    val t = title.lowercase()
    return when {
        "new" in t && ("album" in t || "single" in t || "release" in t) -> 0
        "album" in t || "single" in t || "release" in t -> 1
        "trend" in t || "chart" in t -> 2
        "favorite" in t || "replay" in t -> 3
        else -> 4
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
private fun HomeTabContent(
    state: SonataUiState,
    settings: SonataSettingsState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onRefresh: () -> Unit,
    onSignIn: () -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onCardLongClick: (AndroidHomeCard, String?, Boolean, Rect?) -> Unit = { _, _, _, _ -> },
    onPlaylistLongClick: (AndroidPlaylist, Rect?) -> Unit = { _, _ -> },
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onOpenMood: (String) -> Unit,
    onShowAllMoods: () -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    val visibleShelves = remember(state.shelves) {
        state.shelves.filterNot { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" in t || "unutulan" in t
        }
    }
    val recentsShelf = remember(visibleShelves) {
        visibleShelves.firstOrNull { shelf ->
            shelf.tracks.isNotEmpty() || (
                shelf.cards.isNotEmpty() && shelf.cards.all {
                    it.kind == AndroidHomeCardKind.SONG || it.kind == AndroidHomeCardKind.VIDEO
                }
                )
        }
    }
    val remainingShelves = remember(visibleShelves, recentsShelf) {
        visibleShelves
            .filter { shelf ->
                shelf !== recentsShelf && shelf.cards.any { it.kind != AndroidHomeCardKind.MOOD }
            }
            .sortedBy { homeShelfSortOrder(it.title) }
            .distinctBy { it.title }
    }
    val fallbackHomeTracks = state.tracks.takeIf { state.shelves.isEmpty() }.orEmpty()
    val topPicks = remember(state.shelves, fallbackHomeTracks) {
        val allCardPairs = state.shelves.flatMap { shelf ->
            shelf.cards.map { it to shelf.tracks }
        }
        val albumOrReleaseCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.ALBUM && !card.thumbnailUrl.isNullOrBlank()
        }
        val playlistCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.PLAYLIST && !card.thumbnailUrl.isNullOrBlank()
        }
        val artistCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.ARTIST
        }
        val trackAsReleaseCards = state.shelves.flatMap { shelf ->
            shelf.tracks.map { track ->
                AndroidHomeCard(
                    kind = AndroidHomeCardKind.SONG,
                    id = track.id,
                    title = track.title,
                    subtitle = track.artist,
                    thumbnailUrl = track.thumbnailUrl,
                    track = track,
                ) to shelf.tracks
            }
        }
        val seedTracks = if (state.shelves.isNotEmpty()) {
            state.shelves.flatMap { it.tracks }
        } else {
            fallbackHomeTracks
        }
        val portraitUrls = (
            artistCards.mapNotNull { it.first.thumbnailUrl } +
                seedTracks.mapNotNull { it.thumbnailUrl } +
                albumOrReleaseCards.mapNotNull { it.first.thumbnailUrl } +
                playlistCards.mapNotNull { it.first.thumbnailUrl }
            ).distinct()

        val entries = mutableListOf<TopPickEntry>()

        // Card 1: Single high-res track/album ReleaseArt (avoids 2x2 playlist collage grid on first slot)
        val firstRelease = albumOrReleaseCards.firstOrNull()
            ?: trackAsReleaseCards.firstOrNull { !it.first.thumbnailUrl.isNullOrBlank() }
            ?: playlistCards.firstOrNull()
        if (firstRelease != null) {
            entries += TopPickEntry.Release(card = firstRelease.first, shelfTracks = firstRelease.second)
        }

        // Card 2: Vivid purple ArtistCollageArt with 2-3 circular portraits peeking beside Card 1
        val artistCard = artistCards.firstOrNull()?.first
        val topArtists = seedTracks.map { it.artist }.filter { it.isNotBlank() }.distinct().take(2)
        val artistMixCard = artistCard ?: AndroidHomeCard(
            kind = AndroidHomeCardKind.ARTIST,
            id = "artist_mix",
            title = if (topArtists.isNotEmpty()) topArtists.joinToString(" & ") else "Favorites Mix",
            subtitle = topArtists.firstOrNull()?.let { "Featuring $it" } ?: "Personalized artist mix",
            thumbnailUrl = portraitUrls.firstOrNull(),
            track = null,
        )
        entries += TopPickEntry.ArtistMix(
            card = artistMixCard,
            portraits = (listOfNotNull(artistMixCard.thumbnailUrl) + portraitUrls).distinct().take(3),
        )

        // Card 3: Personal Station
        entries += TopPickEntry.PersonalStation

        // Card 4: Featured Playlist / Mix ReleaseArt
        val featuredPlaylist = playlistCards.firstOrNull { it.first.id != firstRelease?.first?.id }
            ?: albumOrReleaseCards.drop(1).firstOrNull()
            ?: trackAsReleaseCards.drop(1).firstOrNull()
        if (featuredPlaylist != null) {
            entries += TopPickEntry.Release(card = featuredPlaylist.first, shelfTracks = featuredPlaylist.second)
        }

        // Card 5: Discovery Station
        entries += TopPickEntry.DiscoveryStation
        entries
    }

    val recentsTracks = remember(recentsShelf) {
        if (recentsShelf == null) {
            emptyList()
        } else if (recentsShelf.tracks.isNotEmpty()) {
            recentsShelf.tracks
        } else {
            recentsShelf.cards.mapNotNull { it.track }
        }
    }
    val recentsColumns = remember(recentsTracks) { recentsTracks.chunked(4) }
    val lastPlayedTracks = remember(state.lastPlayedTracks, recentsTracks, fallbackHomeTracks) {
        state.lastPlayedTracks
            .ifEmpty { recentsTracks.ifEmpty { fallbackHomeTracks } }
            .distinctBy { it.id }
            .take(14)
    }
    val showSkeleton = state.loading && state.shelves.isEmpty()
    TrackScrollActivity(listState)

    PullToRefreshBox(
        isRefreshing = state.refreshing,
        onRefresh = onRefresh,
        modifier = modifier.fillMaxSize(),
    ) {
        Crossfade(
            targetState = showSkeleton,
            animationSpec = tween(durationMillis = 260, easing = FastOutSlowInEasing),
            label = "homeSkeletonCrossfade",
            modifier = Modifier.fillMaxSize(),
        ) { loadingSkeleton ->
            if (loadingSkeleton) {
                SonataShimmerHost {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = contentPadding,
                        userScrollEnabled = false,
                    ) {
                        item(key = "home_logo_skeleton", contentType = "home_logo") {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = PAGE_GUTTER, vertical = 6.dp)
                                    .padding(bottom = 12.dp),
                            ) {
                                DynamicMoonLogo(modifier = Modifier.size(34.dp))
                            }
                        }
                        topPicksSkeleton()
                        recentlyPlayedSkeleton()
                        feedSkeleton()
                    }
                }
            } else {
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = contentPadding,
                ) {
                    // Top-left DynamicMoonLogo (live astronomical phase, geographic tilt, Blood Moon tap cycle)
                    item(key = "home_logo", contentType = "home_logo") {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = PAGE_GUTTER, vertical = 6.dp)
                                .padding(bottom = 12.dp),
                        ) {
                            DynamicMoonLogo(modifier = Modifier.size(34.dp))
                        }
                    }

                    if (state.authInitialized && !state.signedIn) {
                        item(key = "home_sign_in_banner", contentType = "sign_in_banner") {
                            SignInBanner(
                                onSignIn = onSignIn,
                                modifier = Modifier.padding(bottom = 14.dp),
                            )
                        }
                    }

                    // Shelf 1: Top Picks for You (252.dp x 336.dp 3:4 portrait cards + live Canvas & Sonata Stations)
                    item(key = "top_picks_shelf", contentType = "top_picks_shelf") {
                        Column(Modifier.padding(bottom = 26.dp)) {
                            SectionHeader(title = "Top Picks for You")
                            LazyRow(
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                horizontalArrangement = Arrangement.spacedBy(14.dp),
                            ) {
                                items(
                                    items = topPicks,
                                    key = { it.stableKey },
                                    contentType = { it.contentType },
                                ) { pick ->
                                    when (pick) {
                                        is TopPickEntry.Release -> {
                                            val artUrl = remember(pick.card.thumbnailUrl) {
                                                pick.card.thumbnailUrl.artworkAt(CARD_ART_PX)
                                            }
                                            var cardCanvas by remember(pick.card.id, settings.animatedArtwork) {
                                                mutableStateOf(
                                                    pick.card.track?.id?.let { CanvasResolver.cached(it) }
                                                        ?: CanvasResolver.cached(pick.card.id),
                                                )
                                            }
                                            LaunchedEffect(pick.card.id, settings.animatedArtwork) {
                                                if (settings.animatedArtwork && !settings.reduceAnimation && cardCanvas == null) {
                                                    val targetTrack = pick.card.track ?: pick.shelfTracks.firstOrNull()
                                                    val resolved = CanvasResolver.resolve(
                                                        videoId = targetTrack?.id ?: pick.card.id,
                                                        title = targetTrack?.title ?: pick.card.title,
                                                        artist = targetTrack?.artist ?: pick.card.subtitle,
                                                        album = targetTrack?.album,
                                                    )
                                                    if (resolved != null) cardCanvas = resolved
                                                }
                                            }
                                            val eyebrow = if (pick.card.kind == AndroidHomeCardKind.ALBUM) {
                                                "NEW RELEASE"
                                            } else {
                                                "FEATURED PLAYLIST"
                                            }
                                            var cardBounds by remember { mutableStateOf<Rect?>(null) }
                                            ReleaseArt(
                                                artworkUrl = artUrl,
                                                title = pick.card.title,
                                                subtitle = pick.card.subtitle,
                                                eyebrow = eyebrow,
                                                canvasMedia = cardCanvas,
                                                canvasActive = settings.animatedArtwork && !settings.reduceAnimation,
                                                modifier = Modifier
                                                    .onGloballyPositioned { cardBounds = it.boundsInRoot() }
                                                    .combinedClickable(
                                                        onClick = { onOpenCard(pick.card, pick.shelfTracks) },
                                                        onLongClick = {
                                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                                            onCardLongClick(pick.card, eyebrow, true, cardBounds)
                                                        },
                                                    ),
                                            )
                                        }

                                        is TopPickEntry.ArtistMix -> {
                                            val sizedPortraits = remember(pick.portraits) {
                                                pick.portraits.mapNotNull { it.artworkAt(CARD_ART_PX) }
                                            }
                                            var cardBounds by remember { mutableStateOf<Rect?>(null) }
                                            ArtistCollageArt(
                                                images = sizedPortraits,
                                                title = pick.card.title,
                                                subtitle = pick.card.subtitle.ifBlank { "Featuring artists you love" },
                                                modifier = Modifier
                                                    .onGloballyPositioned { cardBounds = it.boundsInRoot() }
                                                    .combinedClickable(
                                                        onClick = { onOpenCard(pick.card, emptyList()) },
                                                        onLongClick = {
                                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                                            onCardLongClick(pick.card, "MIX", true, cardBounds)
                                                        },
                                                    ),
                                            )
                                        }

                                        TopPickEntry.PersonalStation -> {
                                            val name = state.accountProfile?.name?.takeIf { it.isNotBlank() }
                                            val stationTitle = if (name != null) "$name's Station" else "My Station"
                                            Box(
                                                modifier = Modifier
                                                    .width(252.dp)
                                                    .height(336.dp)
                                                    .clip(Shape12)
                                                    .clickable {
                                                        val tracks = recentsShelf?.tracks?.ifEmpty { state.tracks } ?: state.tracks
                                                        tracks.firstOrNull()?.let { first -> onPlayTrack(first, tracks) }
                                                    },
                                            ) {
                                                SonataPersonalStationArt(
                                                    title = stationTitle,
                                                    modifier = Modifier.fillMaxSize(),
                                                )
                                                Column(
                                                    modifier = Modifier
                                                        .align(Alignment.BottomStart)
                                                        .padding(18.dp),
                                                ) {
                                                    Text(
                                                        text = "PERSONALIZED STATION",
                                                        style = MaterialTheme.typography.labelSmall.copy(
                                                            fontWeight = FontWeight.Bold,
                                                            letterSpacing = 1.2.sp,
                                                        ),
                                                        color = Color.White.copy(alpha = 0.8f),
                                                    )
                                                    Spacer(Modifier.height(4.dp))
                                                    Text(
                                                        text = stationTitle,
                                                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                                                        color = Color.White,
                                                    )
                                                }
                                            }
                                        }

                                        TopPickEntry.DiscoveryStation -> Box(
                                            modifier = Modifier
                                                .width(252.dp)
                                                .height(336.dp)
                                                .clip(Shape12)
                                                .clickable {
                                                    val tracks = state.tracks.reversed()
                                                    tracks.firstOrNull()?.let { first -> onPlayTrack(first, tracks) }
                                                },
                                        ) {
                                            SonataDiscoveryStationArt(Modifier.fillMaxSize())
                                            Column(
                                                modifier = Modifier
                                                    .align(Alignment.BottomStart)
                                                    .padding(18.dp),
                                            ) {
                                                Text(
                                                    text = "CURATED FOR YOU",
                                                    style = MaterialTheme.typography.labelSmall.copy(
                                                        fontWeight = FontWeight.Bold,
                                                        letterSpacing = 1.2.sp,
                                                    ),
                                                    color = Color.White.copy(alpha = 0.82f),
                                                )
                                                Spacer(Modifier.height(4.dp))
                                                Text(
                                                    text = "Discovery Station",
                                                    style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                                                    color = Color.White,
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 2a: Last Played (156.dp square cover + title strip, ZERO glass)
                    if (lastPlayedTracks.isNotEmpty()) {
                        item(key = "last_played_square_strip", contentType = "last_played_strip") {
                            val activeId = state.activeTrack?.id
                            Column(Modifier.padding(bottom = 24.dp)) {
                                SectionHeader(title = "Last Played")
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    items(
                                        items = lastPlayedTracks,
                                        key = { t -> "lp_${t.id}" },
                                        contentType = { "last_played" },
                                    ) { track ->
                                        SonataLastPlayedSquareCard(
                                            title = track.title,
                                            artworkUrl = track.thumbnailUrl,
                                            isCurrent = activeId != null && track.id == activeId,
                                            onClick = { onPlayTrack(track, lastPlayedTracks) },
                                            onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 2b: Smooth Classic "Son Dinlenenler" 4-rows-per-column pager
                    if (recentsShelf != null && recentsColumns.isNotEmpty()) {
                        item(key = "recents_shelf", contentType = "recents_shelf") {
                            val activeId = state.activeTrack?.id
                            val colWidth = trackColumnWidth(LocalConfiguration.current.screenWidthDp.dp)
                            Column(Modifier.padding(bottom = 26.dp)) {
                                SectionHeader(title = recentsShelf.title.ifBlank { "Recently Played" })
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    items(
                                        items = recentsColumns,
                                        key = { col -> "recents_col_${col.joinToString("_") { it.id }}" },
                                        contentType = { "recents_col" },
                                    ) { column ->
                                        Column(Modifier.width(colWidth)) {
                                            column.forEach { track ->
                                                val current = activeId != null && track.id == activeId
                                                CompactTrackRow(
                                                    track = track,
                                                    isCurrent = current,
                                                    isPlaying = current && state.isPlaying,
                                                    onClick = { onPlayTrack(track, recentsTracks) },
                                                    onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                                    onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 3: Find Your Mood (always-animated SonataMoodAnimatedArt)
                    item(key = "find_your_mood_shelf", contentType = "mood_shelf") {
                        Column(Modifier.padding(bottom = 26.dp)) {
                            SectionHeader(
                                title = "Find Your Mood",
                                onShowAll = onShowAllMoods,
                            )
                            LazyRow(
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                horizontalArrangement = Arrangement.spacedBy(14.dp),
                            ) {
                                items(
                                    items = TopPickMood.entries,
                                    key = { it.name },
                                    contentType = { "mood_tile" },
                                ) { mood ->
                                    MoodTile(
                                        mood = mood,
                                        onClick = { _, label -> onOpenMood(label) },
                                    )
                                }
                            }
                        }
                    }

                    // Shelf 4: Personal Playlists
                    if (state.playlists.isNotEmpty()) {
                        item(key = "personal_playlists_shelf", contentType = "personal_playlists_shelf") {
                            Column(Modifier.padding(bottom = 26.dp)) {
                                SectionHeader(title = "Your Playlists")
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = state.playlists,
                                        key = { "home_pl_${it.id}" },
                                        contentType = { "playlist_card" },
                                    ) { playlist ->
                                        SonataEditorialCard(
                                            title = playlist.name,
                                            subtitle = "Playlist",
                                            artworkUrl = playlist.thumbnailUrl,
                                            badgeLabel = "Playlist",
                                            onClick = { onOpenPlaylist(playlist) },
                                            onPlayClick = { onOpenPlaylist(playlist) },
                                            onLongClick = { bounds -> onPlaylistLongClick(playlist, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Remaining Shelves: clean 150.dp 1:1 square ShelfCards & 248.dp 16:9 VideoShelfCards
                    items(
                        items = remainingShelves,
                        key = { shelf -> "shelf_${shelf.title}" },
                        contentType = { "home_shelf" },
                    ) { shelf ->
                        val cards = remember(shelf.cards) {
                            shelf.cards
                                .filter { it.kind != AndroidHomeCardKind.MOOD }
                                .distinctBy { "${it.kind}_${it.id}_${it.title}" }
                        }
                        if (cards.isNotEmpty()) {
                            val isVideoShelf = remember(cards) {
                                cards.all { it.kind == AndroidHomeCardKind.VIDEO }
                            }
                            Column(Modifier.padding(bottom = 26.dp)) {
                                SectionHeader(title = shelf.title)
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = cards,
                                        key = { card -> "${card.kind}_${card.id}_${card.title}" },
                                        contentType = { if (isVideoShelf) "video_card" else "home_card" },
                                    ) { card ->
                                        val activeId = state.activeTrack?.id
                                        val current = activeId != null && card.track?.id == activeId
                                        if (isVideoShelf) {
                                            VideoShelfCard(
                                                card = card,
                                                isCurrent = current,
                                                isPlaying = current && state.isPlaying,
                                                onClick = { onOpenCard(card, shelf.tracks) },
                                                onLongClick = { bounds -> onCardLongClick(card, "VIDEO", false, bounds) },
                                            )
                                        } else if (card.kind == AndroidHomeCardKind.ARTIST) {
                                            SonataArtistCircleCard(
                                                name = card.title,
                                                artworkUrl = card.thumbnailUrl,
                                                onClick = { onOpenCard(card, shelf.tracks) },
                                            )
                                        } else {
                                            SonataEditorialCard(
                                                title = card.title,
                                                subtitle = card.subtitle,
                                                artworkUrl = card.thumbnailUrl,
                                                badgeLabel = when (card.kind) {
                                                    AndroidHomeCardKind.ALBUM -> "Album"
                                                    AndroidHomeCardKind.PLAYLIST -> "Playlist"
                                                    else -> null
                                                },
                                                onClick = { onOpenCard(card, shelf.tracks) },
                                                onPlayClick = { onOpenCard(card, shelf.tracks) },
                                                onLongClick = { bounds ->
                                                    onCardLongClick(
                                                        card,
                                                        if (card.kind == AndroidHomeCardKind.ALBUM) "ALBUM" else "PLAYLIST",
                                                        false,
                                                        bounds,
                                                    )
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TrackScrollActivity(listState: LazyListState) {
    LaunchedEffect(listState) {
        var active = false
        try {
            snapshotFlow { listState.isScrollInProgress }
                .distinctUntilChanged()
                .collect { scrolling ->
                    if (scrolling != active) {
                        active = scrolling
                        ScrollActivityTracker.setScrolling(scrolling)
                    }
                }
        } finally {
            if (active) {
                ScrollActivityTracker.setScrolling(false)
            }
        }
    }
}

@Composable
private fun SectionHeader(
    title: String,
    subtitle: String = "",
    onShowAll: (() -> Unit)? = null,
) {
    Row(
        modifier = Modifier
            .padding(horizontal = PAGE_GUTTER, vertical = 10.dp)
            .fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 20.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (onShowAll != null) {
            Text(
                text = "Show all",
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                color = SonataAccent,
                modifier = Modifier
                    .clickable(onClick = onShowAll)
                    .padding(start = 12.dp, top = 4.dp, bottom = 4.dp),
            )
        }
    }
}

@Composable
private fun SignInBanner(
    onSignIn: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val palette = remember {
        MeshPalette(
            listOf(
                Color(0xFF9E1B3A),
                Color(0xFF42216B),
                Color(0xFF1D2E6B),
                Color(0xFFD63858),
            )
        )
    }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = PAGE_GUTTER, vertical = 6.dp)
            .clip(Shape22)
            .border(0.75.dp, Color.White.copy(alpha = 0.14f), Shape22)
            .clickable(onClick = onSignIn),
    ) {
        MeshGradientBackground(
            palette = palette,
            modifier = Modifier.matchParentSize(),
            blurRadius = 36.dp,
            continuous = false,
            animated = false,
        )
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 18.dp)) {
            Text(
                text = "Sign in to YouTube Music",
                style = MaterialTheme.typography.headlineSmall.copy(
                    fontWeight = FontWeight.ExtraBold,
                    fontSize = 24.sp,
                    lineHeight = 29.sp,
                ),
                color = Color.White,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                text = "Sync your library, personalized mixes, and Quick Picks",
                style = MaterialTheme.typography.bodyMedium,
                color = Color.White.copy(alpha = 0.82f),
            )
            Spacer(Modifier.height(16.dp))
            Row(
                modifier = Modifier
                    .clip(CircleShape)
                    .background(Color.White)
                    .padding(horizontal = 18.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = SonataIcons.Person,
                    contentDescription = null,
                    tint = Color.Black,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    text = "Sign in with Google",
                    style = MaterialTheme.typography.titleSmall.copy(
                        fontWeight = FontWeight.Bold,
                        fontSize = 15.sp,
                    ),
                    color = Color.Black,
                )
            }
        }
    }
}

private data class TrackMenuRequest(
    val track: AndroidTrack,
    val anchorBounds: Rect? = null,
)

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun CompactTrackRow(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    leadingIndex: Int? = null,
    onOptionsClick: ((Rect?) -> Unit)? = null,
    onLongClick: ((Rect?) -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    var rowBounds by remember { mutableStateOf<Rect?>(null) }
    var optionsBounds by remember { mutableStateOf<Rect?>(null) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(Shape10)
            .onGloballyPositioned { rowBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(rowBounds)
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
                color = if (isCurrent) SonataAccent else MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(26.dp),
            )
            Spacer(Modifier.width(6.dp))
        }
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(Shape8)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), Shape8),
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
                    tint = Color.White.copy(alpha = 0.6f),
                    modifier = Modifier.size(20.dp),
                )
            }
            if (isCurrent) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color.Black.copy(alpha = 0.48f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = if (isPlaying) SonataIcons.GraphicEq else SonataIcons.Pause,
                        contentDescription = null,
                        tint = SonataAccent,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 15.5.sp,
                    fontWeight = if (isCurrent) FontWeight.SemiBold else FontWeight.Medium,
                ),
                color = if (isCurrent) SonataAccent else Color.White,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = track.artist,
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Box(
            modifier = Modifier
                .size(32.dp)
                .clip(CircleShape)
                .onGloballyPositioned { optionsBounds = it.boundsInRoot() }
                .then(
                    if (onOptionsClick != null) Modifier.clickable { onOptionsClick(optionsBounds) } else Modifier,
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = SonataIcons.MoreVert,
                contentDescription = "Track options",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
    modifier: Modifier = Modifier.width(SHELF_CARD_WIDTH),
) {
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    Column(
        modifier = modifier
            .clip(Shape12)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds)
                    }
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(Shape12)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), Shape12),
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
                    tint = Color.White.copy(alpha = 0.45f),
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
            color = Color.White,
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
private fun VideoShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
    modifier: Modifier = Modifier.width(248.dp),
) {
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    Column(
        modifier = modifier
            .clip(Shape12)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds)
                    }
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(16f / 9f)
                .clip(Shape12)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), Shape12),
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
            color = Color.White,
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
private fun DetailScreen(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onPlayTrack: (AndroidTrack) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Int, Rect?) -> Unit = { _, _, _ -> },
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
    onDownloadAll: (() -> Unit)? = null,
    onShare: (() -> Unit)? = null,
    onOpenArtist: ((String) -> Unit)? = null,
) {
    val palette = rememberArtworkPalette(
        artworkUrl = state.collectionCoverUrl,
        dark = true,
        artPx = HEADER_ART_PX,
    )

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    0f to palette.wash,
                    0.44f to palette.background,
                    1f to Color.Black,
                )
            ),
    ) {
        SonataShimmerHost {
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = contentPadding,
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
                        accentColor = SonataAccent,
                        washColor = palette.wash,
                        onPlayAll = onPlayAll,
                        onShuffleAll = onShuffleAll,
                        onDownloadAll = onDownloadAll?.takeIf { state.collectionTracks.isNotEmpty() },
                        onShare = onShare,
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
                                onClick = { onPlayTrack(track) },
                                onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                onLongClick = { bounds -> onTrackLongClick(track, idx + 1, bounds) },
                            )
                            HorizontalDivider(
                                color = Color.White.copy(alpha = 0.08f),
                                modifier = Modifier.padding(start = 84.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun AllMoodsScreen(
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onOpenMood: (String) -> Unit,
) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(minSize = 160.dp),
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = PAGE_GUTTER),
        contentPadding = contentPadding,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        items(
            items = TopPickMood.entries,
            key = { it.name },
            contentType = { "mood_grid_tile" },
        ) { mood ->
            MoodTile(
                mood = mood,
                onClick = { _, label -> onOpenMood(label) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun ExploreTabContent(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onOpenMood: (String) -> Unit,
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onCardLongClick: (AndroidHomeCard, Rect?) -> Unit = { _, _ -> },
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val moodColumns = when {
        screenWidthDp >= 780 -> 3
        else -> 2
    }
    val artPool = remember(state.shelves, state.tracks, state.playlists) {
        (
            state.shelves.flatMap { shelf ->
                shelf.cards.mapNotNull { it.thumbnailUrl } + shelf.tracks.mapNotNull { it.thumbnailUrl }
            } +
                state.tracks.mapNotNull { it.thumbnailUrl } +
                state.playlists.mapNotNull { it.thumbnailUrl }
            ).distinct()
    }
    val moodSleeves = remember(state.shelves) {
        val fromFeed = state.shelves.flatMap { it.cards }.filter { it.kind == AndroidHomeCardKind.MOOD }
        val fallback = TopPickMood.entries.map { mood ->
            AndroidHomeCard(
                kind = AndroidHomeCardKind.MOOD,
                id = mood.name,
                title = mood.displayName,
                subtitle = "",
                thumbnailUrl = null,
                track = null,
            )
        }
        (fromFeed + fallback).distinctBy { it.title }
    }
    val orderedExploreShelves = remember(state.shelves) {
        state.shelves.filter { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" !in t && "unutulan" !in t &&
                shelf.cards.any {
                    it.kind == AndroidHomeCardKind.ALBUM ||
                        it.kind == AndroidHomeCardKind.PLAYLIST ||
                        it.kind == AndroidHomeCardKind.ARTIST ||
                        it.kind == AndroidHomeCardKind.VIDEO
                }
        }.distinctBy { it.title }
    }
    val topReleaseAndChartShelves = remember(orderedExploreShelves) {
        orderedExploreShelves.filter { shelf ->
            val t = shelf.title.lowercase()
            "new" in t || "release" in t || "chart" in t || "trend" in t || "album" in t || "single" in t
        }.sortedBy { homeShelfSortOrder(it.title) }
    }
    val lowerExploreShelves = remember(orderedExploreShelves, topReleaseAndChartShelves) {
        orderedExploreShelves.filter { it !in topReleaseAndChartShelves }
    }

    val moodRows = remember(moodSleeves, moodColumns) { moodSleeves.chunked(moodColumns) }
    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "explore_heading", contentType = "tab_heading") {
            Text(
                text = "Explore",
                style = MaterialTheme.typography.displayLarge.copy(
                    fontSize = 34.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
            )
        }

        // 1. New Releases & Charts / Trending shelves first
        items(
            items = topReleaseAndChartShelves,
            key = { shelf -> "explore_top_${shelf.title}" },
            contentType = { "explore_shelf" },
        ) { shelf ->
            val cards = remember(shelf.cards) {
                shelf.cards
                    .filter { it.kind != AndroidHomeCardKind.MOOD }
                    .distinctBy { "${it.kind}_${it.id}_${it.title}" }
            }
            if (cards.isNotEmpty()) {
                Column(Modifier.padding(top = 8.dp, bottom = 14.dp)) {
                    SectionHeader(title = shelf.title)
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        items(
                            items = cards,
                            key = { card -> "top_${card.kind}_${card.id}_${card.title}" },
                            contentType = { "shelf_card" },
                        ) { card ->
                            val activeId = state.activeTrack?.id
                            val current = activeId != null && card.track?.id == activeId
                            ShelfCard(
                                card = card,
                                isCurrent = current,
                                isPlaying = current && state.isPlaying,
                                onClick = { onOpenCard(card, shelf.tracks) },
                                onLongClick = if (card.kind == AndroidHomeCardKind.ARTIST) {
                                    null
                                } else {
                                    { bounds -> onCardLongClick(card, bounds) }
                                },
                            )
                        }
                    }
                }
            }
        }

        // 2. Moods & Genres section (always-animated SonataMoodAnimatedArt)
        item(key = "explore_moods_header", contentType = "section_header") {
            SectionHeader(title = "Moods & Genres")
        }

        itemsIndexed(
            items = moodRows,
            key = { rowIdx, row -> "mood_row_${rowIdx}_${row.firstOrNull()?.title.orEmpty()}" },
            contentType = { _, _ -> "explore_mood_row" },
        ) { rowIdx, rowItems ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                rowItems.forEachIndexed { colIdx, mood ->
                    val globalIdx = rowIdx * moodColumns + colIdx
                    val resolvedThumb = mood.thumbnailUrl?.takeIf { it.isNotBlank() }
                        ?: if (artPool.isNotEmpty()) artPool[globalIdx % artPool.size] else null
                    MoodGenreSleeve(
                        title = mood.title,
                        thumbnailUrl = resolvedThumb,
                        modifier = Modifier
                            .weight(1f)
                            .aspectRatio(1.9f)
                            .clip(Shape12)
                            .clickable { onOpenMood(mood.title) },
                    )
                }
                repeat(moodColumns - rowItems.size) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }

        // 3. Featured Videos & Remaining Explore shelves
        items(
            items = lowerExploreShelves,
            key = { shelf -> "explore_rest_${shelf.title}" },
            contentType = { "explore_shelf" },
        ) { shelf ->
            val cards = remember(shelf.cards) {
                shelf.cards
                    .filter { it.kind != AndroidHomeCardKind.MOOD }
                    .distinctBy { "${it.kind}_${it.id}_${it.title}" }
            }
            if (cards.isNotEmpty()) {
                val isVideoShelf = remember(cards) { cards.all { it.kind == AndroidHomeCardKind.VIDEO } }
                Column(Modifier.padding(top = 18.dp, bottom = 10.dp)) {
                    SectionHeader(title = shelf.title)
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        items(
                            items = cards,
                            key = { card -> "${card.kind}_${card.id}_${card.title}" },
                            contentType = { if (isVideoShelf) "video_card" else "shelf_card" },
                        ) { card ->
                            val activeId = state.activeTrack?.id
                            val current = activeId != null && card.track?.id == activeId
                            if (isVideoShelf) {
                                VideoShelfCard(
                                    card = card,
                                    isCurrent = current,
                                    isPlaying = current && state.isPlaying,
                                    onClick = { onOpenCard(card, shelf.tracks) },
                                    onLongClick = { bounds -> onCardLongClick(card, bounds) },
                                )
                            } else {
                                ShelfCard(
                                    card = card,
                                    isCurrent = current,
                                    isPlaying = current && state.isPlaying,
                                    onClick = { onOpenCard(card, shelf.tracks) },
                                    onLongClick = if (card.kind == AndroidHomeCardKind.ARTIST) {
                                        null
                                    } else {
                                        { bounds -> onCardLongClick(card, bounds) }
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LibraryTabContent(
    state: SonataUiState,
    downloadsCount: Int,
    latestDownloadCoverUrl: String?,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onSignIn: () -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onPlaylistLongClick: (AndroidPlaylist, Rect?) -> Unit = { _, _ -> },
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onOpenDownloads: () -> Unit,
    onOpenDownloadsManager: () -> Unit,
    onOpenArtist: (String, String) -> Unit,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val playlistColumns = ((screenWidthDp - 32) / 160).coerceIn(2, 6)
    val replayArt = remember(state.tracks, state.playlists) {
        state.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            ?: state.playlists.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
    }
    val holderName = state.accountProfile?.name?.takeIf { it.isNotBlank() } ?: "SONATA LISTENER"
    val downloadsPlaylist = remember(latestDownloadCoverUrl) {
        AndroidPlaylist(
            id = SonataViewModel.DOWNLOADS_PLAYLIST_ID,
            name = "Downloads",
            thumbnailUrl = latestDownloadCoverUrl,
        )
    }
    val combinedPlaylists = remember(downloadsPlaylist, state.playlists) {
        listOf(downloadsPlaylist) + state.playlists.filterNot {
            it.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID
        }
    }
    val playlistRows = remember(combinedPlaylists, playlistColumns) {
        combinedPlaylists.chunked(playlistColumns)
    }
    val libraryArtists = remember(state.shelves) {
        state.shelves.flatMap { it.cards }
            .filter { it.kind == AndroidHomeCardKind.ARTIST }
            .distinctBy { it.id }
            .take(12)
    }

    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "library_heading", contentType = "tab_heading") {
            Text(
                text = "Library",
                style = MaterialTheme.typography.displayLarge.copy(
                    fontSize = 34.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
            )
        }

        // Horizontal ReplayCreditCard carousel
        item(key = "library_replay_carousel", contentType = "replay_carousel") {
            LazyRow(
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                item(key = "replay_card_tracks", contentType = "replay_card") {
                    ReplayCreditCard(
                        label = "Tracks Ready",
                        value = "${state.tracks.size} Tracks",
                        detail = state.activeTrack?.title ?: "Tap to resume listening",
                        artworkUrl = replayArt.artworkAt(CARD_ART_PX),
                        holder = holderName,
                        onClick = {
                            state.tracks.firstOrNull()?.let { onPlayTrack(it, state.tracks) }
                        },
                        modifier = Modifier.width(300.dp),
                    )
                }
                item(key = "replay_card_playlists", contentType = "replay_card") {
                    ReplayCreditCard(
                        label = "Offline Library",
                        value = "$downloadsCount Downloaded",
                        detail = if (downloadsCount > 0) "Ready without network" else "Save songs for offline play",
                        artworkUrl = latestDownloadCoverUrl?.artworkAt(CARD_ART_PX)
                            ?: replayArt.artworkAt(CARD_ART_PX),
                        holder = holderName,
                        onClick = onOpenDownloads,
                        modifier = Modifier.width(300.dp),
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
        }

        // Downloads Playlist & Download Manager rows (never navigate to Search!)
        item(key = "library_device_rows", contentType = "device_rows") {
            Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                LibraryNavRow(
                    icon = SonataIcons.Download,
                    label = "Downloads",
                    subtitle = if (downloadsCount > 0) "$downloadsCount offline tracks" else "Offline cached streams",
                    onClick = onOpenDownloads,
                )
                HorizontalDivider(
                    color = MaterialTheme.colorScheme.outline.copy(alpha = 0.6f),
                    modifier = Modifier.padding(start = 40.dp),
                )
                LibraryNavRow(
                    icon = SonataIcons.Folder,
                    label = "Manage Downloads",
                    subtitle = "Storage & download queue",
                    onClick = onOpenDownloadsManager,
                )
            }
            Spacer(Modifier.height(14.dp))
        }

        if (libraryArtists.isNotEmpty()) {
            item(key = "library_artists_shelf", contentType = "library_artists") {
                Column(Modifier.padding(bottom = 18.dp)) {
                    SectionHeader(title = "Artists")
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        items(
                            items = libraryArtists,
                            key = { "lib_artist_${it.id}" },
                            contentType = { "artist_circle" },
                        ) { artist ->
                            SonataArtistCircleCard(
                                name = artist.title,
                                artworkUrl = artist.thumbnailUrl,
                                onClick = { onOpenArtist(artist.id, artist.title) },
                            )
                        }
                    }
                }
            }
        }

        if (state.authInitialized && !state.signedIn) {
            item(key = "library_signin_banner", contentType = "sign_in_banner") {
                SignInBanner(onSignIn = onSignIn)
            }
        }

        item(key = "library_playlists_header", contentType = "section_header") {
            SectionHeader(title = "Playlists")
        }
        items(
            items = playlistRows,
            key = { row -> "pl_row_${row.joinToString("_") { it.id }}" },
            contentType = { "playlist_grid_row" },
        ) { rowPlaylists ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                rowPlaylists.forEach { playlist ->
                    PlaylistGridCard(
                        playlist = playlist,
                        onClick = {
                            if (playlist.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
                                onOpenDownloads()
                            } else {
                                onOpenPlaylist(playlist)
                            }
                        },
                        onLongClick = { bounds -> onPlaylistLongClick(playlist, bounds) },
                        modifier = Modifier.weight(1f),
                    )
                }
                repeat(playlistColumns - rowPlaylists.size) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun LibraryNavRow(
    icon: ImageVector,
    label: String,
    @Suppress("UNUSED_PARAMETER") subtitle: String,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = Color.White,
            modifier = Modifier.size(22.dp),
        )
        Spacer(Modifier.width(16.dp))
        Text(
            text = label,
            style = MaterialTheme.typography.titleMedium.copy(fontSize = 17.sp, fontWeight = FontWeight.Medium),
            color = Color.White,
            modifier = Modifier.weight(1f),
        )
        Icon(
            imageVector = SonataIcons.ChevronRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(18.dp),
        )
    }
}

private val LikedPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF7B52FF), Color(0xFFB79CED)))
private val DownloadsPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF1D1A2B), Color(0xFF8F96D9)))

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun PlaylistGridCard(
    playlist: AndroidPlaylist,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onLongClick: ((Rect?) -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val isDownloadsPlaylist = playlist.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID
    val isLikedPlaylist = !isDownloadsPlaylist && (playlist.id == "LM" || playlist.name.contains("Liked", ignoreCase = true))
    val cover = remember(playlist.thumbnailUrl) { playlist.thumbnailUrl.artworkAt(CARD_ART_PX) }
    val coverRequest = rememberArtworkRequest(cover, 360, 360)
    Column(
        modifier = modifier
            .clip(Shape12)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds)
                    }
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(Shape12)
                .then(
                    when {
                        isDownloadsPlaylist && coverRequest == null -> Modifier.background(DownloadsPlaylistBrush)
                        isLikedPlaylist || coverRequest == null -> Modifier.background(LikedPlaylistBrush)
                        else -> Modifier.background(MaterialTheme.colorScheme.surfaceVariant)
                    }
                )
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), Shape12),
            contentAlignment = Alignment.Center,
        ) {
            when {
                isDownloadsPlaylist -> {
                    if (coverRequest != null) {
                        AsyncImage(
                            model = coverRequest,
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                        Box(
                            modifier = Modifier
                                .align(Alignment.BottomEnd)
                                .padding(8.dp)
                                .size(30.dp)
                                .clip(CircleShape)
                                .background(Color.Black.copy(alpha = 0.72f)),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = SonataIcons.Download,
                                contentDescription = null,
                                tint = SonataAccent,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    } else {
                        Icon(
                            imageVector = SonataIcons.Download,
                            contentDescription = null,
                            tint = Color.White,
                            modifier = Modifier.size(54.dp),
                        )
                    }
                }
                isLikedPlaylist || coverRequest == null -> {
                    Icon(
                        imageVector = SonataIcons.ThumbUp,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(64.dp),
                    )
                }
                else -> {
                    AsyncImage(
                        model = coverRequest,
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = playlist.name,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
            color = Color.White,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun SearchTabContent(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onSearchQuery: (String) -> Unit,
    onRemoveRecentSearch: (String) -> Unit,
    onClearRecentSearches: () -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
) {
    var selectedSegment by remember { mutableIntStateOf(0) }
    val segments = remember { listOf("YouTube", "Library", "Playlists") }
    val filteredLibraryTracks = remember(state.tracks, state.query, selectedSegment) {
        if (state.query.isBlank()) state.tracks
        else state.tracks.filter {
            it.title.contains(state.query, ignoreCase = true) ||
                it.artist.contains(state.query, ignoreCase = true)
        }
    }
    val filteredPlaylists = remember(state.playlists, state.query, selectedSegment) {
        if (state.query.isBlank()) state.playlists
        else state.playlists.filter { it.name.contains(state.query, ignoreCase = true) }
    }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "search_segmented_control") {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                contentAlignment = Alignment.Center,
            ) {
                PillSegmentedControl(
                    options = segments,
                    selectedIndex = selectedSegment,
                    onSelect = { selectedSegment = it },
                )
            }
        }

        if (state.loading && selectedSegment == 0) {
            item(key = "search_loading") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 32.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(color = Color.White, strokeWidth = 2.5.dp)
                }
            }
        }

        when (selectedSegment) {
            0 -> {
                if (state.recentSearches.isNotEmpty() && state.query.isBlank()) {
                    item(key = "recent_searches_header") {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = "Recent Searches",
                                style = MaterialTheme.typography.titleMedium.copy(
                                    fontSize = 19.sp,
                                    fontWeight = FontWeight.Bold,
                                ),
                                color = Color.White,
                                modifier = Modifier.weight(1f),
                            )
                            Text(
                                text = "Clear",
                                style = MaterialTheme.typography.titleSmall,
                                color = SonataAccent,
                                modifier = Modifier.clickable(onClick = onClearRecentSearches),
                            )
                        }
                    }
                    items(
                        items = state.recentSearches,
                        key = { "recent_$it" },
                        contentType = { "recent_search" },
                    ) { q ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSearchQuery(q) }
                                .padding(horizontal = PAGE_GUTTER, vertical = 11.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                imageVector = SonataIcons.History,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(Modifier.width(12.dp))
                            Text(
                                text = q,
                                style = MaterialTheme.typography.bodyLarge,
                                color = Color.White,
                                modifier = Modifier.weight(1f),
                            )
                            IconButton(
                                onClick = { onRemoveRecentSearch(q) },
                                modifier = Modifier.size(28.dp),
                            ) {
                                Icon(
                                    imageVector = SonataIcons.Close,
                                    contentDescription = "Remove",
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.size(16.dp),
                                )
                            }
                        }
                    }
                }

                if (state.tracks.isNotEmpty()) {
                    item(key = "search_results_header", contentType = "section_header") {
                        SectionHeader(title = state.listTitle)
                    }
                    items(
                        items = state.tracks,
                        key = { "yt_${it.id}" },
                        contentType = { "search_track" },
                    ) { track ->
                        val activeId = state.activeTrack?.id
                        val current = activeId != null && track.id == activeId
                        Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                            CompactTrackRow(
                                track = track,
                                isCurrent = current,
                                isPlaying = current && state.isPlaying,
                                onClick = { onPlayTrack(track, state.tracks) },
                                onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                            )
                        }
                    }
                }
            }

            1 -> {
                items(
                    items = filteredLibraryTracks,
                    key = { "lib_search_${it.id}" },
                    contentType = { "search_track" },
                ) { track ->
                    val activeId = state.activeTrack?.id
                    val current = activeId != null && track.id == activeId
                    Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                        CompactTrackRow(
                            track = track,
                            isCurrent = current,
                            isPlaying = current && state.isPlaying,
                            onClick = { onPlayTrack(track, filteredLibraryTracks) },
                            onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                            onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                        )
                    }
                }
            }

            else -> {
                items(
                    items = filteredPlaylists,
                    key = { "pl_search_${it.id}" },
                    contentType = { "search_playlist" },
                ) { playlist ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onOpenPlaylist(playlist) }
                            .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(
                            modifier = Modifier
                                .size(50.dp)
                                .clip(Shape8)
                                .background(MaterialTheme.colorScheme.surfaceVariant),
                            contentAlignment = Alignment.Center,
                        ) {
                            val thumb = remember(playlist.thumbnailUrl) { playlist.thumbnailUrl.artworkAt(ROW_ART_PX) }
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
                                    imageVector = SonataIcons.TabLibrary,
                                    contentDescription = null,
                                    tint = Color.White.copy(alpha = 0.6f),
                                )
                            }
                        }
                        Spacer(Modifier.width(12.dp))
                        Text(
                            text = playlist.name,
                            style = MaterialTheme.typography.titleMedium,
                            color = Color.White,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsSheet(
    state: SonataUiState,
    onDismiss: () -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onTogglePreferHighQuality: () -> Unit,
    onToggleCrossfade: () -> Unit,
    onToggleLiquidGlass: () -> Unit,
    onToggleAnimatedArtwork: () -> Unit,
    onSetFpsLimit: (Int) -> Unit,
    onClearMediaCache: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
) {
    val context = LocalContext.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var filter by remember { mutableStateOf("") }
    fun matches(text: String) = filter.isBlank() || text.contains(filter, ignoreCase = true)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = Color(0xFF0B0B0D),
        contentColor = Color.White,
        dragHandle = null,
    ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(horizontal = PAGE_GUTTER),
            contentPadding = PaddingValues(top = 18.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item(key = "settings_header") {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Settings",
                        style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                        color = Color.White,
                        modifier = Modifier.weight(1f),
                    )
                    CircleGlyph(
                        icon = SonataIcons.Close,
                        contentDescription = "Close Settings",
                        onClick = onDismiss,
                    )
                }
            }

            item(key = "settings_search") {
                SearchField(
                    query = filter,
                    onQueryChange = { filter = it },
                    onSubmit = {},
                    placeholder = "Search Settings",
                )
            }

            if (matches("Account YouTube Music Sign In Sign Out")) {
                item(key = "settings_account_card") {
                    SettingsGroupCard(title = "Account") {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(46.dp)
                                    .clip(CircleShape)
                                    .background(Color.White.copy(alpha = 0.1f)),
                                contentAlignment = Alignment.Center,
                            ) {
                                val avatar = if (state.signedIn) state.accountProfile?.avatarUrl else null
                                val avatarRequest = rememberArtworkRequest(avatar, 128, 128)
                                if (avatarRequest != null) {
                                    AsyncImage(
                                        model = avatarRequest,
                                        contentDescription = null,
                                        contentScale = ContentScale.Crop,
                                        modifier = Modifier.fillMaxSize(),
                                    )
                                } else {
                                    Icon(
                                        imageVector = SonataIcons.Person,
                                        contentDescription = null,
                                        tint = Color.White,
                                    )
                                }
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(
                                    text = if (state.signedIn) {
                                        state.accountProfile?.name ?: "YouTube Music Account"
                                    } else {
                                        "Not signed in"
                                    },
                                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                                    color = Color.White,
                                )
                                Text(
                                    text = if (state.signedIn) {
                                        state.accountProfile?.email
                                            ?: state.accountProfile?.handle
                                            ?: "Connected to YouTube Music"
                                    } else {
                                        "Connect your account for personalized mixes"
                                    },
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        HorizontalDivider(color = Color.White.copy(alpha = 0.08f))
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (state.signedIn) onSignOut() else onSignIn()
                                }
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = if (state.signedIn) "Sign Out" else "Connect YouTube Music",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = SonataAccent,
                                modifier = Modifier.weight(1f),
                            )
                            Icon(
                                imageVector = SonataIcons.ChevronRight,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
            }

            if (matches("Playback Audio Lossless Hi-Fi Stream Crossfade")) {
                item(key = "settings_playback_card") {
                    SettingsGroupCard(title = "Playback & Audio") {
                        SettingsSwitchRow(
                            title = "Lossless / Hi-Fi Stream",
                            subtitle = "Automatically upgrade low-bitrate streams",
                            checked = state.preferHighQuality,
                            onToggle = onTogglePreferHighQuality,
                        )
                        HorizontalDivider(
                            color = Color.White.copy(alpha = 0.08f),
                            modifier = Modifier.padding(start = 16.dp),
                        )
                        SettingsSwitchRow(
                            title = "Crossfade",
                            subtitle = "Smooth transitions between queue tracks",
                            checked = state.crossfadeEnabled,
                            onToggle = onToggleCrossfade,
                        )
                    }
                }
            }

            if (matches("Appearance Liquid Glass Navigation Animated Covers Frame Rate 60 FPS 120 FPS")) {
                item(key = "settings_appearance_card") {
                    SettingsGroupCard(title = "Appearance") {
                        SettingsSwitchRow(
                            title = "Liquid Glass Navigation",
                            subtitle = if (isGlassSupported()) {
                                "Real-time refraction over scrolling pages"
                            } else {
                                "Requires Android 12 or newer"
                            },
                            checked = state.liquidGlassEnabled,
                            enabled = isGlassSupported(),
                            onToggle = onToggleLiquidGlass,
                        )
                        HorizontalDivider(
                            color = Color.White.copy(alpha = 0.08f),
                            modifier = Modifier.padding(start = 16.dp),
                        )
                        SettingsSwitchRow(
                            title = "Animated Covers",
                            subtitle = "Ambient mesh motion in Now Playing",
                            checked = state.animatedArtwork,
                            onToggle = onToggleAnimatedArtwork,
                        )
                        HorizontalDivider(
                            color = Color.White.copy(alpha = 0.08f),
                            modifier = Modifier.padding(start = 16.dp),
                        )
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Column(Modifier.weight(1f)) {
                                    Text(
                                        text = "Frame Rate",
                                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                                        color = Color.White,
                                    )
                                    Text(
                                        text = if (state.supports120Hz) {
                                            "Default 60 FPS · 120 FPS supported on this display"
                                        } else {
                                            "Locked to 60 FPS on this display"
                                        },
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                            Spacer(Modifier.height(10.dp))
                            PillSegmentedControl(
                                options = if (state.supports120Hz) {
                                    listOf("60 FPS", "120 FPS")
                                } else {
                                    listOf("60 FPS")
                                },
                                selectedIndex = if (state.fpsLimit >= 120 && state.supports120Hz) 1 else 0,
                                onSelect = { idx: Int ->
                                    onSetFpsLimit(if (idx == 1 && state.supports120Hz) 120 else 60)
                                },
                            )
                        }
                    }
                }
            }

            if (matches("Storage Clear Media Cache")) {
                item(key = "settings_storage_card") {
                    SettingsGroupCard(title = "Storage") {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable(onClick = onClearMediaCache)
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    text = "Clear Media Cache",
                                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                                    color = Color.White,
                                )
                                Text(
                                    text = state.status.ifBlank { "Frees cached ExoPlayer audio segments" },
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            Icon(
                                imageVector = SonataIcons.ChevronRight,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
            }

            if (matches("Updates Version Check Download APK Release")) {
                item(key = "settings_updates_card") {
                    val currentVer = state.updateInfo?.currentVersion ?: state.installedVersionName
                    val statusLabel = when {
                        state.updateDownloading -> state.updateDownloadDetail.ifBlank { "Downloading update..." }
                        state.checkingUpdates -> "Checking for updates..."
                        state.updateInfo?.updateAvailable == true ->
                            "Update v${state.updateInfo.latestVersion} available"
                        state.updateInfo != null -> "Up to date"
                        else -> "Tap Check for Updates"
                    }
                    val releaseTarget = state.updateInfo?.apkDownloadUrl
                        ?: state.updateInfo?.releaseUrl
                        ?: "https://github.com/MehmetCanWT/sonata/releases"

                    SettingsGroupCard(title = "Updates") {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    text = "Current Version: v$currentVer",
                                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                                    color = Color.White,
                                )
                                Text(
                                    text = statusLabel,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = if (state.updateInfo?.updateAvailable == true) {
                                        SonataAccent
                                    } else {
                                        MaterialTheme.colorScheme.onSurfaceVariant
                                    },
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        HorizontalDivider(
                            color = Color.White.copy(alpha = 0.08f),
                            modifier = Modifier.padding(start = 16.dp),
                        )
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable(enabled = !state.checkingUpdates, onClick = onCheckForUpdates)
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = "Check for Updates",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                                color = SonataAccent,
                                modifier = Modifier.weight(1f),
                            )
                            if (state.checkingUpdates) {
                                CircularProgressIndicator(
                                    color = Color.White,
                                    strokeWidth = 2.dp,
                                    modifier = Modifier.size(16.dp),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.ChevronRight,
                                    contentDescription = null,
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                        }
                        HorizontalDivider(
                            color = Color.White.copy(alpha = 0.08f),
                            modifier = Modifier.padding(start = 16.dp),
                        )
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (state.updateInfo?.updateAvailable == true &&
                                        !state.updateInfo.apkDownloadUrl.isNullOrBlank()
                                    ) {
                                        onDownloadUpdate()
                                    } else {
                                        runCatching {
                                            context.startActivity(
                                                Intent(Intent.ACTION_VIEW, Uri.parse(releaseTarget))
                                            )
                                        }
                                    }
                                }
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = if (state.updateInfo?.updateAvailable == true) {
                                    "Download & Install v${state.updateInfo.latestVersion}"
                                } else {
                                    "Open GitHub Releases"
                                },
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                                color = if (state.updateInfo?.updateAvailable == true) SonataAccent else Color.White,
                                modifier = Modifier.weight(1f),
                            )
                            Icon(
                                imageVector = SonataIcons.ChevronRight,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SettingsGroupCard(
    title: String,
    content: @Composable () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            text = title.uppercase(),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.8.sp,
            ),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 6.dp),
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(20.dp))
                .background(Color(0xFF1C1C1E)),
        ) {
            content()
        }
    }
}

@Composable
private fun SettingsSwitchRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    enabled: Boolean = true,
    onToggle: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled, onClick = onToggle)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                color = if (enabled) Color.White else Color.White.copy(alpha = 0.45f),
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = checked,
            onCheckedChange = { onToggle() },
            enabled = enabled,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = SonataAccent,
                uncheckedThumbColor = Color.White,
                uncheckedTrackColor = Color(0xFF3A3A3C),
            ),
        )
    }
}

private data class NowPlayingExtras(
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

private val LocalNowPlayingExtras = androidx.compose.runtime.compositionLocalOf { NowPlayingExtras() }

@Composable
private fun NowPlayingScreen(
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

    val statusBarTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
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
                            .statusBarsPadding()
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
private fun NowPlayingActionRowBlock(
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
        }

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
private fun NowPlayingProgressSection(
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
private fun NowPlayingScrubberAndTransportBlock(
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
private fun NowPlayingLandscapeLayout(
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
private fun NowPlayingPortraitLayout(
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
private fun ActionCutoutTile(
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
private fun CurrentLyricStrip(
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
private fun NowPlayingLyricsPanel(
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

@Suppress("unused")
@Composable
private fun LegacyNowPlayingLyricsPanel(
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    onSeekTo: (Long) -> Unit,
) {
    if (lyricsLoading) {
        LyricsSkeleton(modifier = Modifier.fillMaxSize())
        return
    }
    if (lyrics.isEmpty()) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(
                text = "No lyrics available",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White.copy(alpha = 0.62f),
                textAlign = TextAlign.Center,
            )
        }
        return
    }

    val durationMs = clock.durationMs
    val isTablet = LocalConfiguration.current.screenWidthDp >= 600
    val hasDuetVoices = remember(lyrics) {
        lyrics.any {
            it.isCounterVoice ||
                it.text.trimStart().startsWith("v2:", ignoreCase = true) ||
                it.text.trimStart().startsWith("bg:", ignoreCase = true)
        }
    }
    val listState = rememberLazyListState()
    val density = LocalDensity.current
    var panelHeightPx by remember { mutableIntStateOf(0) }
    val activeIndex by remember(lyrics) {
        derivedStateOf {
            val pos = clockMs.value + LYRIC_SYNC_LEAD_MS
            lyrics.indexOfLast { (it.timeSecs * 1000.0).toLong() <= pos }.coerceAtLeast(0)
        }
    }
    var lastUserTouchMs by remember { mutableLongStateOf(0L) }

    LaunchedEffect(listState.isScrollInProgress) {
        if (listState.isScrollInProgress) {
            lastUserTouchMs = System.currentTimeMillis()
        }
    }

    LaunchedEffect(activeIndex, lyrics.size, panelHeightPx) {
        val now = System.currentTimeMillis()
        if (activeIndex !in lyrics.indices ||
            listState.isScrollInProgress ||
            (now - lastUserTouchMs) <= 2_500L
        ) {
            return@LaunchedEffect
        }
        val layoutInfo = listState.layoutInfo
        val viewportSpan = (layoutInfo.viewportEndOffset - layoutInfo.viewportStartOffset).coerceAtLeast(1)
        val anchorOffset = layoutInfo.viewportStartOffset + (viewportSpan * 0.24f).roundToInt()
        val visibleTarget = layoutInfo.visibleItemsInfo.firstOrNull { it.index == activeIndex }
        if (visibleTarget != null) {
            val itemAnchor = visibleTarget.offset + (visibleTarget.size / 2)
            val delta = (itemAnchor - anchorOffset).toFloat()
            if (kotlin.math.abs(delta) > 3f) {
                listState.animateScrollBy(
                    value = delta,
                    animationSpec = tween(durationMillis = 320, easing = FastOutSlowInEasing),
                )
            }
        } else {
            listState.scrollToItem(activeIndex, scrollOffset = 0)
            withFrameNanos { }
            val updatedInfo = listState.layoutInfo
            val updatedSpan = (updatedInfo.viewportEndOffset - updatedInfo.viewportStartOffset).coerceAtLeast(1)
            val updatedAnchor = updatedInfo.viewportStartOffset + (updatedSpan * 0.24f).roundToInt()
            val targetAfterJump = updatedInfo.visibleItemsInfo.firstOrNull { it.index == activeIndex }
            if (targetAfterJump != null) {
                val itemAnchor = targetAfterJump.offset + (targetAfterJump.size / 2)
                val delta = (itemAnchor - updatedAnchor).toFloat()
                if (kotlin.math.abs(delta) > 3f) {
                    listState.animateScrollBy(
                        value = delta,
                        animationSpec = tween(durationMillis = 360, easing = FastOutSlowInEasing),
                    )
                }
            }
        }
    }

    val bottomPadDp = with(density) { ((panelHeightPx * 0.62f).roundToInt().coerceAtLeast(1)).toDp() }.coerceAtLeast(180.dp)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .onSizeChanged { panelHeightPx = it.height }
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .drawWithCache {
                val topFadePx = 14.dp.toPx().coerceAtMost(size.height * 0.12f)
                val bottomFadePx = 42.dp.toPx().coerceAtMost(size.height * 0.22f)
                val maskBrush = Brush.verticalGradient(
                    0f to Color.Transparent,
                    (topFadePx / size.height.coerceAtLeast(1f)).coerceIn(0.01f, 0.15f) to Color.Black,
                    (1f - (bottomFadePx / size.height.coerceAtLeast(1f))).coerceIn(0.75f, 0.99f) to Color.Black,
                    1f to Color.Transparent,
                )
                onDrawWithContent {
                    drawContent()
                    drawRect(brush = maskBrush, blendMode = BlendMode.DstIn)
                }
            },
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = 14.dp, bottom = bottomPadDp),
            verticalArrangement = Arrangement.spacedBy(if (isTablet) 24.dp else 20.dp),
        ) {
            itemsIndexed(
                items = lyrics,
                key = { idx, line -> "${idx}_${line.timeSecs}" },
                contentType = { _, _ -> "lyric_line" },
            ) { index, line ->
                val isActive = index == activeIndex
                val rawTrimmed = line.text.trim()
                val isCounter = line.isCounterVoice ||
                    rawTrimmed.startsWith("v2:", ignoreCase = true) ||
                    rawTrimmed.startsWith("bg:", ignoreCase = true)
                val cleanText = remember(rawTrimmed) {
                    rawTrimmed
                        .removePrefix("v2:").removePrefix("V2:")
                        .removePrefix("v1:").removePrefix("V1:")
                        .removePrefix("bg:").removePrefix("BG:")
                        .trim()
                }
                val isBackgroundVocal = cleanText.startsWith("(") && cleanText.endsWith(")") && cleanText.length > 2

                val scale by animateFloatAsState(
                    targetValue = if (isActive) 1f else 0.96f,
                    animationSpec = tween(durationMillis = 340, easing = FastOutSlowInEasing),
                    label = "lyricRowScale",
                )
                val lineStartMs = (line.timeSecs * 1000.0).toLong().coerceAtLeast(0L)
                val nextLineStartMs = lyrics.getOrNull(index + 1)
                    ?.let { (it.timeSecs * 1000.0).toLong() }
                val lineEndMs = nextLineStartMs
                    ?.coerceAtLeast(lineStartMs + 400L)
                    ?: maxOf(durationMs, lineStartMs + 4_000L)

                val baseActiveSize = if (isTablet) 29f else 25.5f
                val baseInactiveSize = if (isTablet) 25f else 22f
                val sizeScale = if (isBackgroundVocal) 0.84f else 1f
                val fontSizeSp = (if (isActive) baseActiveSize else baseInactiveSize) * sizeScale
                val lineHeightSp = fontSizeSp * 1.28f

                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(
                            start = if (isCounter) 44.dp else 4.dp,
                            end = if (!isCounter && hasDuetVoices) 44.dp else 8.dp,
                        ),
                    horizontalAlignment = if (isCounter) Alignment.End else Alignment.Start,
                ) {
                    SweptKaraokeLyricText(
                        text = cleanText,
                        lineStartMs = lineStartMs,
                        lineEndMs = lineEndMs,
                        words = line.words,
                        clockMs = clockMs,
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontSize = fontSizeSp.sp,
                            fontWeight = if (isActive) FontWeight.ExtraBold else FontWeight.Bold,
                            lineHeight = lineHeightSp.sp,
                            letterSpacing = (-0.35).sp,
                            textAlign = if (isCounter) TextAlign.End else TextAlign.Start,
                        ),
                        color = Color.White,
                        dimAlpha = if (isBackgroundVocal) 0.28f else 0.34f,
                        uncommittedColor = Color(0xFF98989F).copy(
                            alpha = if (isBackgroundVocal) 0.28f else 0.34f
                        ),
                        active = isActive,
                        modifier = Modifier
                            .fillMaxWidth()
                            .graphicsLayer {
                                scaleX = scale
                                scaleY = scale
                                transformOrigin = androidx.compose.ui.graphics.TransformOrigin(
                                    pivotFractionX = if (isCounter) 1f else 0f,
                                    pivotFractionY = 0.5f,
                                )
                            }
                            .clip(Shape12)
                            .clickable { onSeekTo(lineStartMs) }
                            .padding(vertical = 4.dp),
                    )

                    // 3-dot instrumental break indicator (Draw-phase Canvas -> 0 row recompositions/sec)
                    if (isActive && nextLineStartMs != null && (nextLineStartMs - lineStartMs) >= 5_500L) {
                        InstrumentalBreakDots(
                            lineStartMs = lineStartMs,
                            nextLineStartMs = nextLineStartMs,
                            cleanTextLength = cleanText.length,
                            clockMs = clockMs,
                        )
                    }
                }
            }
        }
    }
}

/**
 * Leaf composable rendering the 3-dot instrumental interlude progress indicator.
 * Visibility is gated via `derivedStateOf` and dot lighting reads `clockMs.value` strictly
 * in the Canvas Draw phase so the parent lyric row never recomposes at 60/120fps.
 */
@Composable
private fun InstrumentalBreakDots(
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
private fun NowPlayingQueuePanel(
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

@Composable
private fun UpdateAvailableDialog(
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
    val notesPreview = remember(info.releaseNotes) {
        info.releaseNotes
            .lineSequence()
            .map { it.trim() }
            .filter { it.isNotEmpty() }
            .take(8)
            .joinToString("\n")
    }
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
                .background(Color(0xFF14151C))
                .border(1.dp, Color.White.copy(alpha = 0.12f), RoundedCornerShape(24.dp))
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
                        tint = Color.White,
                        modifier = Modifier.size(24.dp),
                    )
                }
                Column(Modifier.weight(1f)) {
                    Text(
                        text = "Update Available",
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                        color = Color.White,
                    )
                    Text(
                        text = "v${info.currentVersion}  →  v${info.latestVersion}",
                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
                        color = SonataAccent,
                    )
                }
            }

            if (notesPreview.isNotBlank() && !downloading) {
                Text(
                    text = notesPreview,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.White.copy(alpha = 0.74f),
                    maxLines = 8,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            if (downloading) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (progress != null) {
                        LinearProgressIndicator(
                            progress = { progress.coerceIn(0f, 1f) },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = SonataAccent,
                            trackColor = Color.White.copy(alpha = 0.14f),
                        )
                    } else {
                        LinearProgressIndicator(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = SonataAccent,
                            trackColor = Color.White.copy(alpha = 0.14f),
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
                            color = Color.White.copy(alpha = 0.78f),
                        )
                        if (progress != null) {
                            Text(
                                text = "${(progress * 100f).roundToInt()}%",
                                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                                color = Color.White,
                            )
                        }
                    }
                }
            } else if (detail.isNotBlank() && error == null) {
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.White.copy(alpha = 0.80f),
                )
            }

            if (!error.isNullOrBlank()) {
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFFFF6B6B),
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(46.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(Color.White.copy(alpha = 0.10f))
                        .clickable {
                            if (downloading) onCancelDownload() else onDismiss()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = if (downloading) "Cancel" else "Later",
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = Color.White.copy(alpha = 0.86f),
                    )
                }
                if (!downloading) {
                    Box(
                        modifier = Modifier
                            .weight(1.35f)
                            .height(46.dp)
                            .clip(RoundedCornerShape(14.dp))
                            .background(SonataAccent)
                            .clickable(onClick = onDownloadOrInstall),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = if (hasDownloadedApk) "Install Update" else "Download Now",
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                            color = Color.White,
                        )
                    }
                }
            }
        }
    }
}

private fun formatDurationMs(ms: Long): String {
    val totalSec = (ms / 1000L).coerceAtLeast(0L)
    val mins = totalSec / 60L
    val secs = totalSec % 60L
    return "$mins:${secs.toString().padStart(2, '0')}"
}

private fun shareInAppText(context: android.content.Context, title: String, subtitle: String) {
    val text = if (subtitle.isNotBlank()) "$title — $subtitle" else title
    if (text.isBlank()) return
    val sendIntent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, text)
    }
    runCatching {
        context.startActivity(Intent.createChooser(sendIntent, "Share via"))
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
