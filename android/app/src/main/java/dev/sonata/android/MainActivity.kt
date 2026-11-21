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
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.ThumbUp
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.media3.common.Player
import coil3.compose.AsyncImage
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.hazeSource
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
import dev.sonata.android.ui.SonataTheme
import dev.sonata.android.ui.SweptKaraokeLyricText
import dev.sonata.android.ui.SystemBarIcons
import dev.sonata.android.ui.ThinSlider
import dev.sonata.android.ui.TopFadeBlur
import dev.sonata.android.ui.TopPickMood
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.detailSkeleton
import dev.sonata.android.ui.feedSkeleton
import dev.sonata.android.ui.recentlyPlayedSkeleton
import dev.sonata.android.ui.rememberArtworkPalette
import dev.sonata.android.ui.rememberSmoothLyricClock
import dev.sonata.android.ui.toMeshPalette
import dev.sonata.android.ui.topPicksSkeleton
import dev.sonata.android.ui.trackColumnWidth
import kotlinx.coroutines.delay
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import kotlin.math.roundToInt

private val TopBarContentHeight = 52.dp
private val NavBarLift = 10.dp
private val InlineSpacing = 12.dp
private val InlineGutter = 24.dp

private data class NavTabSpec(
    val tab: AndroidTab,
    val label: String,
    val icon: ImageVector,
)

private val PrimaryTabs = listOf(
    NavTabSpec(AndroidTab.Home, "Home", SonataIcons.Home),
    NavTabSpec(AndroidTab.Explore, "Explore", SonataIcons.TabExplore),
    NavTabSpec(AndroidTab.Library, "Library", SonataIcons.TabLibrary),
    NavTabSpec(AndroidTab.Search, "Search", SonataIcons.TabSearch),
)

class MainActivity : ComponentActivity() {
    private val notifLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        setContent {
            SonataTheme(darkTheme = true) {
                SystemBarIcons(dark = true)
                SonataApp()
            }
        }
    }
}

@Composable
private fun Modifier.adaptiveLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
    surfaceTint: Color? = null,
): Modifier = if (LocalLiquidGlassEnabled.current && isGlassSupported()) {
    this.liquidGlass(shape = shape, surfaceTint = surfaceTint)
} else {
    this.lightweightLiquidGlass(shape = shape, fallbackColor = fallbackColor)
}

@Composable
fun SonataApp(vm: SonataViewModel = viewModel()) {
    val state by vm.uiState.collectAsStateWithLifecycle()
    val context = LocalContext.current
    LaunchedEffect(state.fpsLimit, state.supports120Hz) {
        val activity = context as? ComponentActivity ?: return@LaunchedEffect
        val targetFps = if (state.fpsLimit >= 120 && state.supports120Hz) 120f else 60f
        runCatching {
            val window = activity.window
            val params = window.attributes
            params.preferredRefreshRate = targetFps
            @Suppress("DEPRECATION")
            val modes = window.windowManager?.defaultDisplay?.supportedModes
            val bestMode = modes?.minByOrNull { kotlin.math.abs(it.refreshRate - targetFps) }
            if (bestMode != null) {
                params.preferredDisplayModeId = bestMode.modeId
            }
            window.attributes = params
        }
    }
    val windowBackground = MaterialTheme.colorScheme.background
    val paintBackdrop: ContentDrawScope.() -> Unit = remember(windowBackground) {
        {
            drawRect(windowBackground)
            drawContent()
        }
    }
    val appBackdrop = rememberLayerBackdrop(onDraw = paintBackdrop)
    val glassEnabled = state.liquidGlassEnabled && isGlassSupported()

    CompositionLocalProvider(
        LocalLiquidGlassEnabled provides glassEnabled,
        LocalAppBackdrop provides appBackdrop,
    ) {
        SonataAppContent(
            state = state,
            appBackdrop = appBackdrop,
            vm = vm,
        )
    }
}

@Composable
private fun SonataAppContent(
    state: SonataUiState,
    appBackdrop: LayerBackdrop,
    vm: SonataViewModel,
) {
    val hazeState = remember { HazeState() }
    val density = LocalDensity.current

    var showNowPlaying by rememberSaveable { mutableStateOf(false) }
    var showAllMoods by remember { mutableStateOf(false) }

    val homeListState = rememberLazyListState()
    val exploreListState = rememberLazyListState()
    val libraryListState = rememberLazyListState()
    val searchListState = rememberLazyListState()
    val collectionListState = rememberLazyListState()

    val navBarScroll = rememberFloatingTabBarScrollConnection(
        initialIsInline = false,
        scrollThreshold = 42.dp,
        inlineBehavior = FloatingTabBarInlineBehavior.OnScrollDown,
    )

    var bottomBarsHeightPx by remember { mutableIntStateOf(0) }
    val statusTopPadding = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val topBarHeight = statusTopPadding + TopBarContentHeight
    val bottomBarInset = with(density) { bottomBarsHeightPx.toDp() }.coerceAtLeast(
        WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() + 92.dp
    ) + 12.dp

    val scrolled by remember(
        state.activeTab,
        state.collectionTitle,
        showAllMoods,
        homeListState,
        exploreListState,
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
                state.activeTab == AndroidTab.Library ->
                    libraryListState.firstVisibleItemIndex > 0 ||
                        libraryListState.firstVisibleItemScrollOffset > 28
                else ->
                    searchListState.firstVisibleItemIndex > 0 ||
                        searchListState.firstVisibleItemScrollOffset > 28
            }
        }
    }

    if (showAllMoods && !showNowPlaying && state.collectionTitle == null) {
        BackHandler { showAllMoods = false }
    }
    if (state.collectionTitle != null && !showNowPlaying) {
        BackHandler { vm.closeCollection() }
    }

    if (state.showLoginSheet) {
        YtMusicLoginScreen(
            onCaptured = vm::completeLogin,
            onDismiss = vm::dismissLoginSheet,
            freshSignIn = !state.signedIn,
        )
        return
    }

    if (state.showSettingsSheet) {
        SettingsSheet(
            state = state,
            onDismiss = vm::dismissSettingsSheet,
            onSignIn = vm::openLoginSheet,
            onSignOut = vm::signOut,
            onTogglePreferHighQuality = vm::togglePreferHighQuality,
            onToggleCrossfade = vm::toggleCrossfade,
            onToggleLiquidGlass = vm::toggleLiquidGlass,
            onToggleAnimatedArtwork = vm::toggleAnimatedArtwork,
            onSetFpsLimit = vm::setFpsLimit,
            onClearMediaCache = vm::clearMediaCache,
            onCheckForUpdates = vm::checkForUpdates,
        )
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black),
    ) {
        // Scrollable backdrop source sampled by sibling floating Liquid Glass overlays
        Box(
            modifier = Modifier
                .fillMaxSize()
                .layerBackdrop(appBackdrop)
                .hazeSource(hazeState),
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
                    onPlayAll = {
                        state.collectionTracks.firstOrNull()?.let { first ->
                            vm.playTrack(first, state.collectionTracks)
                        }
                    },
                    onShuffleAll = { vm.toggleShuffleAndPlay(state.collectionTracks) },
                )

                showAllMoods -> AllMoodsScreen(
                    contentPadding = PaddingValues(top = topBarHeight + 8.dp, bottom = bottomBarInset),
                    modifier = Modifier.nestedScroll(navBarScroll),
                    onOpenMood = { title ->
                        showAllMoods = false
                        vm.openMood(title)
                    },
                )

                else -> when (state.activeTab) {
                    AndroidTab.Home -> HomeTabContent(
                        state = state,
                        listState = homeListState,
                        contentPadding = contentPadding,
                        modifier = Modifier.nestedScroll(navBarScroll),
                        onRefresh = vm::loadHome,
                        onSignIn = vm::openLoginSheet,
                        onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                        onOpenCard = { card, shelfTracks -> vm.openCard(card, shelfTracks) },
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
                    )

                    AndroidTab.Library -> LibraryTabContent(
                        state = state,
                        listState = libraryListState,
                        contentPadding = contentPadding,
                        modifier = Modifier.nestedScroll(navBarScroll),
                        onSignIn = vm::openLoginSheet,
                        onOpenPlaylist = vm::openPlaylist,
                        onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                        onOpenSearch = { vm.selectTab(AndroidTab.Search) },
                    )

                    AndroidTab.Search -> SearchTabContent(
                        state = state,
                        listState = searchListState,
                        contentPadding = contentPadding,
                        modifier = Modifier.nestedScroll(navBarScroll),
                        onSearchQuery = { q -> vm.search(q) },
                        onRemoveRecentSearch = vm::removeRecentSearch,
                        onClearRecentSearches = vm::clearRecentSearches,
                        onPlayTrack = { track, queue -> vm.playTrack(track, queue) },
                        onOpenPlaylist = vm::openPlaylist,
                    )
                }
            }
        }

        // Top progressive fade blur behind floating top-bar controls when scrolled
        val topFadeAlpha by animateFloatAsState(
            targetValue = if (scrolled && state.collectionTitle == null) 1f else 0f,
            animationSpec = tween(180),
            label = "topFadeAlpha",
        )
        if (topFadeAlpha > 0.01f) {
            TopFadeBlur(
                hazeState = hazeState,
                pageColor = Color.Black,
                topHeight = topBarHeight,
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .graphicsLayer { alpha = topFadeAlpha },
            )
        }

        // Floating sibling top chrome (outside layerBackdrop so adaptiveLiquidGlass never recurses)
        FloatingTopChrome(
            state = state,
            showAllMoods = showAllMoods,
            onBack = {
                when {
                    state.collectionTitle != null -> vm.closeCollection()
                    showAllMoods -> showAllMoods = false
                }
            },
            onQueryChange = vm::onQueryChange,
            onSearchSubmit = { vm.search() },
            onHistoryClick = { vm.selectTab(AndroidTab.Search) },
            onAvatarClick = vm::openSettingsSheet,
            modifier = Modifier.align(Alignment.TopCenter),
        )

        // Bottom fade scrim behind the floating Liquid Glass navigation bar
        BottomFadeScrim(
            withMiniPlayer = state.activeTrack != null,
            pageColor = Color.Black,
            modifier = Modifier.align(Alignment.BottomCenter),
        )

        // Floating Liquid Glass Tab Bar + MiniPlayer + circular Search pill
        Box(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .widthIn(max = FLOATING_BAR_MAX_WIDTH)
                .fillMaxWidth()
                .onSizeChanged { bottomBarsHeightPx = it.height },
        ) {
            SonataGlassNavBar(
                selectedTab = state.activeTab,
                onTabSelected = { tab ->
                    showAllMoods = false
                    vm.selectTab(tab)
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
                BackHandler { showNowPlaying = false }
                NowPlayingScreen(
                    track = track,
                    state = state,
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
                    onOpenSettings = {
                        showNowPlaying = false
                        vm.openSettingsSheet()
                    },
                    onDismiss = { showNowPlaying = false },
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
                    .adaptiveLiquidGlass(shape = pillShape)
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
                    .adaptiveLiquidGlass(shape = capsuleShape)
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
                    .adaptiveLiquidGlass(shape = pillShape)
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
    Box(
        modifier = Modifier
            .size(size)
            .clip(CircleShape)
            .background(Color.White.copy(alpha = 0.08f))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        if (!avatar.isNullOrBlank()) {
            AsyncImage(
                model = avatar,
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
    val pillShape = remember { RoundedCornerShape(percent = 50) }
    val glassSupported = LocalLiquidGlassEnabled.current && isGlassSupported()
    val contentColor = glassContentColor()
    val selectedColor = contentColor
    val unselectedColor = contentColor.copy(alpha = 0.65f)
    val selectedIndex = PrimaryTabs.indexOfFirst { it.tab == selectedTab }.coerceAtLeast(0)
    val glassSurface: @Composable () -> Modifier = {
        Modifier.adaptiveLiquidGlass(
            shape = pillShape,
            fallbackColor = Color(0xFF141418).copy(alpha = 0.94f),
        )
    }

    FloatingTabBar(
        selectedTabKey = selectedIndex,
        scrollConnection = scrollConnection,
        modifier = Modifier
            .navigationBarsPadding()
            .padding(horizontal = BAR_GUTTER)
            .padding(bottom = NavBarLift)
            .fillMaxWidth(),
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
            val onClick = { onTabSelected(spec.tab) }
            if (index == PrimaryTabs.lastIndex) {
                standaloneTab(
                    key = index,
                    icon = {
                        Icon(
                            imageVector = spec.icon,
                            contentDescription = spec.label,
                            tint = tint,
                            modifier = Modifier.size(22.dp),
                        )
                    },
                    onClick = onClick,
                )
            } else {
                tab(
                    key = index,
                    icon = {
                        Icon(
                            imageVector = spec.icon,
                            contentDescription = spec.label,
                            tint = tint,
                            modifier = Modifier.size(22.dp),
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
                    .clip(RoundedCornerShape(8.dp))
                    .background(Color.White.copy(alpha = 0.08f))
                    .border(0.5.dp, Color.White.copy(alpha = 0.14f), RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center,
            ) {
                val thumb = track.thumbnailUrl.artworkAt(ROW_ART_PX)
                if (!thumb.isNullOrBlank()) {
                    AsyncImage(
                        model = thumb,
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        imageVector = Icons.Rounded.MusicNote,
                        contentDescription = null,
                        tint = contentColor.copy(alpha = 0.7f),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(modifier = Modifier.weight(1f)) {
                MarqueeText(
                    text = track.title,
                    style = MaterialTheme.typography.titleSmall.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = contentColor,
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
    data class Release(val card: AndroidHomeCard, val shelfTracks: List<AndroidTrack>) : TopPickEntry
    data class ArtistMix(val card: AndroidHomeCard, val portraits: List<String>) : TopPickEntry
    data object PersonalStation : TopPickEntry
    data object DiscoveryStation : TopPickEntry
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HomeTabContent(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onRefresh: () -> Unit,
    onSignIn: () -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onOpenMood: (String) -> Unit,
    onShowAllMoods: () -> Unit,
) {
    val recentsShelf = remember(state.shelves) {
        state.shelves.firstOrNull { shelf ->
            shelf.tracks.isNotEmpty() || (
                shelf.cards.isNotEmpty() && shelf.cards.all {
                    it.kind == AndroidHomeCardKind.SONG || it.kind == AndroidHomeCardKind.VIDEO
                }
                )
        }
    }
    val remainingShelves = remember(state.shelves, recentsShelf) {
        state.shelves.filter { shelf ->
            shelf !== recentsShelf && shelf.cards.any { it.kind != AndroidHomeCardKind.MOOD }
        }
    }
    val topPicks = remember(state.shelves, state.tracks) {
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
        val portraitUrls = (
            artistCards.mapNotNull { it.first.thumbnailUrl } +
                state.tracks.mapNotNull { it.thumbnailUrl } +
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
        val topArtists = state.tracks.map { it.artist }.filter { it.isNotBlank() }.distinct().take(2)
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

    PullToRefreshBox(
        isRefreshing = state.refreshing,
        onRefresh = onRefresh,
        modifier = modifier.fillMaxSize(),
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = contentPadding,
        ) {
            // Top-left DynamicMoonLogo (live astronomical phase, geographic tilt, Blood Moon tap cycle)
            item(key = "home_logo") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 6.dp)
                        .padding(bottom = 12.dp),
                ) {
                    DynamicMoonLogo(modifier = Modifier.size(34.dp))
                }
            }

            if (!state.signedIn) {
                item(key = "home_sign_in_banner") {
                    SignInBanner(
                        onSignIn = onSignIn,
                        modifier = Modifier.padding(bottom = 14.dp),
                    )
                }
            }

            if (state.loading && state.shelves.isEmpty()) {
                topPicksSkeleton()
                recentlyPlayedSkeleton()
                feedSkeleton()
            } else {
                // Shelf 1: Top Picks for You (252.dp x 336.dp 3:4 portrait cards)
                item(key = "top_picks_shelf") {
                    Column(Modifier.padding(bottom = 26.dp)) {
                        SectionHeader(title = "Top Picks for You")
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(14.dp),
                        ) {
                            items(topPicks, key = { it.hashCode() }) { pick ->
                                when (pick) {
                                    is TopPickEntry.Release -> ReleaseArt(
                                        artworkUrl = pick.card.thumbnailUrl.artworkAt(CARD_ART_PX),
                                        title = pick.card.title,
                                        subtitle = pick.card.subtitle,
                                        eyebrow = if (pick.card.kind == AndroidHomeCardKind.ALBUM) {
                                            "NEW RELEASE"
                                        } else {
                                            "FEATURED PLAYLIST"
                                        },
                                        modifier = Modifier.clickable {
                                            onOpenCard(pick.card, pick.shelfTracks)
                                        },
                                    )

                                    is TopPickEntry.ArtistMix -> ArtistCollageArt(
                                        images = pick.portraits.mapNotNull { it.artworkAt(CARD_ART_PX) },
                                        title = pick.card.title,
                                        subtitle = pick.card.subtitle.ifBlank { "Featuring artists you love" },
                                        modifier = Modifier.clickable {
                                            onOpenCard(pick.card, emptyList())
                                        },
                                    )

                                    TopPickEntry.PersonalStation -> {
                                        val name = state.accountProfile?.name?.takeIf { it.isNotBlank() }
                                        Box(
                                            modifier = Modifier
                                                .width(252.dp)
                                                .height(336.dp)
                                                .clip(RoundedCornerShape(12.dp))
                                                .clickable {
                                                    val tracks = recentsShelf?.tracks?.ifEmpty { state.tracks } ?: state.tracks
                                                    tracks.firstOrNull()?.let { first -> onPlayTrack(first, tracks) }
                                                },
                                        ) {
                                            PersonalStationArt(Modifier.fillMaxSize())
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
                                                    text = if (name != null) "$name's Station" else "My Station",
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
                                            .clip(RoundedCornerShape(12.dp))
                                            .clickable {
                                                val tracks = state.tracks.reversed()
                                                tracks.firstOrNull()?.let { first -> onPlayTrack(first, tracks) }
                                            },
                                    ) {
                                        DiscoveryStationArt(Modifier.fillMaxSize())
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

                // Shelf 2: Recents / Quick Picks (4-rows-per-column pager)
                if (recentsShelf != null) {
                    val recentsTracks = if (recentsShelf.tracks.isNotEmpty()) {
                        recentsShelf.tracks
                    } else {
                        recentsShelf.cards.mapNotNull { it.track }
                    }
                    if (recentsTracks.isNotEmpty()) {
                        item(key = "recents_shelf") {
                            val activeId = state.activeTrack?.id
                            val colWidth = trackColumnWidth(LocalConfiguration.current.screenWidthDp.dp)
                            Column(Modifier.padding(bottom = 26.dp)) {
                                SectionHeader(title = recentsShelf.title.ifBlank { "Recently Played" })
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    items(recentsTracks.chunked(4)) { column ->
                                        Column(Modifier.width(colWidth)) {
                                            column.forEach { track ->
                                                val current = activeId != null && track.id == activeId
                                                CompactTrackRow(
                                                    track = track,
                                                    isCurrent = current,
                                                    isPlaying = current && state.isPlaying,
                                                    onClick = { onPlayTrack(track, recentsTracks) },
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Shelf 3: Find Your Mood (150.dp 1:1 square MoodTiles + Show all)
                item(key = "find_your_mood_shelf") {
                    Column(Modifier.padding(bottom = 26.dp)) {
                        SectionHeader(
                            title = "Find Your Mood",
                            onShowAll = onShowAllMoods,
                        )
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(14.dp),
                        ) {
                            items(TopPickMood.entries, key = { it.name }) { mood ->
                                MoodTile(
                                    mood = mood,
                                    onClick = { _, label -> onOpenMood(label) },
                                )
                            }
                        }
                    }
                }

                // Remaining Shelves: clean 150.dp 1:1 square ShelfCards & 248.dp 16:9 VideoShelfCards
                itemsIndexed(
                    items = remainingShelves,
                    key = { idx, shelf -> "shelf_${shelf.title}_$idx" },
                ) { _, shelf ->
                    val cards = shelf.cards.filter { it.kind != AndroidHomeCardKind.MOOD }
                    if (cards.isNotEmpty()) {
                        val isVideoShelf = cards.all { it.kind == AndroidHomeCardKind.VIDEO }
                        Column(Modifier.padding(bottom = 26.dp)) {
                            SectionHeader(title = shelf.title)
                            LazyRow(
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                horizontalArrangement = Arrangement.spacedBy(14.dp),
                            ) {
                                items(cards, key = { "${it.kind}_${it.id}_${it.title}" }) { card ->
                                    val activeId = state.activeTrack?.id
                                    val current = activeId != null && card.track?.id == activeId
                                    if (isVideoShelf) {
                                        VideoShelfCard(
                                            card = card,
                                            isCurrent = current,
                                            isPlaying = current && state.isPlaying,
                                            onClick = { onOpenCard(card, shelf.tracks) },
                                        )
                                    } else {
                                        ShelfCard(
                                            card = card,
                                            isCurrent = current,
                                            isPlaying = current && state.isPlaying,
                                            onClick = { onOpenCard(card, shelf.tracks) },
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
    val shape = RoundedCornerShape(22.dp)
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = PAGE_GUTTER, vertical = 6.dp)
            .clip(shape)
            .border(0.75.dp, Color.White.copy(alpha = 0.14f), shape)
            .clickable(onClick = onSignIn),
    ) {
        MeshGradientBackground(
            palette = palette,
            modifier = Modifier.matchParentSize(),
            blurRadius = 36.dp,
            continuous = true,
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

@Composable
private fun CompactTrackRow(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    leadingIndex: Int? = null,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
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
                .clip(RoundedCornerShape(8.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), RoundedCornerShape(8.dp)),
            contentAlignment = Alignment.Center,
        ) {
            val thumb = track.thumbnailUrl.artworkAt(ROW_ART_PX)
            if (!thumb.isNullOrBlank()) {
                AsyncImage(
                    model = thumb,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = Icons.Rounded.MusicNote,
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
                        imageVector = if (isPlaying) Icons.Rounded.GraphicEq else Icons.Rounded.Pause,
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
        Icon(
            imageVector = Icons.Rounded.MoreVert,
            contentDescription = "Track options",
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier
                .padding(start = 6.dp)
                .size(18.dp),
        )
    }
}

@Composable
private fun ShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier.width(SHELF_CARD_WIDTH),
) {
    val shape = RoundedCornerShape(12.dp)
    Column(
        modifier = modifier
            .clip(shape)
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(shape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), shape),
            contentAlignment = Alignment.Center,
        ) {
            val thumb = card.thumbnailUrl.artworkAt(CARD_ART_PX)
            if (!thumb.isNullOrBlank()) {
                AsyncImage(
                    model = thumb,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = Icons.Rounded.MusicNote,
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
                        imageVector = Icons.Rounded.GraphicEq,
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
private fun VideoShelfCard(
    card: AndroidHomeCard,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier.width(248.dp),
) {
    val shape = RoundedCornerShape(12.dp)
    Column(
        modifier = modifier
            .clip(shape)
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(16f / 9f)
                .clip(shape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), shape),
            contentAlignment = Alignment.Center,
        ) {
            if (!card.thumbnailUrl.isNullOrBlank()) {
                AsyncImage(
                    model = card.thumbnailUrl,
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
                        imageVector = Icons.Rounded.GraphicEq,
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
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
) {
    val palette = rememberArtworkPalette(
        artworkUrl = state.collectionCoverUrl,
        dark = true,
        artPx = HEADER_ART_PX,
    )
    var savedToLibrary by remember(state.collectionTitle) { mutableStateOf(false) }

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
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = contentPadding,
        ) {
            item(key = "detail_header") {
                val screenWidth = LocalConfiguration.current.screenWidthDp.dp
                val isWideTablet = screenWidth >= 680.dp
                val artShape = RoundedCornerShape(14.dp)
                val cover = state.collectionCoverUrl.artworkAt(HEADER_ART_PX)
                val meta = listOfNotNull(
                    state.collectionKindLabel,
                    state.collectionYear,
                    state.collectionTracks.takeIf { it.isNotEmpty() }?.let { "${it.size} songs" },
                ).joinToString(" · ")

                @Composable
                fun HeaderActionRow() {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Button(
                            onClick = onPlayAll,
                            enabled = state.collectionTracks.isNotEmpty(),
                            shape = SquirclePillShape(),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = SonataAccent,
                                contentColor = Color.White,
                            ),
                            contentPadding = PaddingValues(horizontal = 26.dp, vertical = 12.dp),
                        ) {
                            Icon(
                                painter = painterResource(R.drawable.ic_player_play),
                                contentDescription = null,
                                modifier = Modifier.size(16.dp),
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = "Play",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                            )
                        }
                        Box(
                            modifier = Modifier
                                .size(42.dp)
                                .lightweightLiquidGlass(CircleShape, fallbackColor = Color.White.copy(alpha = 0.14f))
                                .clip(CircleShape)
                                .clickable(enabled = state.collectionTracks.isNotEmpty(), onClick = onShuffleAll),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = SonataIcons.Shuffle,
                                contentDescription = "Shuffle",
                                tint = Color.White,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        Box(
                            modifier = Modifier
                                .size(42.dp)
                                .lightweightLiquidGlass(CircleShape, fallbackColor = Color.White.copy(alpha = 0.14f))
                                .clip(CircleShape)
                                .clickable { savedToLibrary = !savedToLibrary },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = if (savedToLibrary) SonataIcons.Check else SonataIcons.TabLibrary,
                                contentDescription = "Save to Library",
                                tint = if (savedToLibrary) SonataAccent else Color.White,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }

                if (isWideTablet) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(26.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(196.dp)
                                .shadow(18.dp, artShape, clip = false)
                                .clip(artShape)
                                .background(palette.elevated)
                                .border(0.5.dp, Color.White.copy(alpha = 0.16f), artShape),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (!cover.isNullOrBlank()) {
                                AsyncImage(
                                    model = cover,
                                    contentDescription = state.collectionTitle,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.TabLibrary,
                                    contentDescription = null,
                                    tint = Color.White.copy(alpha = 0.55f),
                                    modifier = Modifier.size(60.dp),
                                )
                            }
                        }
                        Column(
                            modifier = Modifier.weight(1f),
                            verticalArrangement = Arrangement.Center,
                        ) {
                            Text(
                                text = state.collectionTitle.orEmpty(),
                                style = MaterialTheme.typography.headlineMedium.copy(
                                    fontSize = 28.sp,
                                    fontWeight = FontWeight.Bold,
                                ),
                                color = Color.White,
                            )
                            state.collectionSubtitle?.takeIf { it.isNotBlank() }?.let { subtitle ->
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    text = subtitle,
                                    style = MaterialTheme.typography.titleMedium.copy(
                                        fontSize = 18.sp,
                                        fontWeight = FontWeight.Medium,
                                    ),
                                    color = SonataAccent,
                                )
                            }
                            if (meta.isNotBlank()) {
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    text = meta,
                                    style = MaterialTheme.typography.labelLarge,
                                    color = Color.White.copy(alpha = 0.62f),
                                )
                            }
                            Spacer(Modifier.height(18.dp))
                            HeaderActionRow()
                        }
                    }
                } else {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        val artWidth = ((screenWidth - PAGE_GUTTER * 2) * 0.76f).coerceAtMost(310.dp)
                        Box(
                            modifier = Modifier
                                .size(artWidth)
                                .shadow(18.dp, artShape, clip = false)
                                .clip(artShape)
                                .background(palette.elevated)
                                .border(0.5.dp, Color.White.copy(alpha = 0.16f), artShape),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (!cover.isNullOrBlank()) {
                                AsyncImage(
                                    model = cover,
                                    contentDescription = state.collectionTitle,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.TabLibrary,
                                    contentDescription = null,
                                    tint = Color.White.copy(alpha = 0.55f),
                                    modifier = Modifier.size(64.dp),
                                )
                            }
                        }

                        Spacer(Modifier.height(18.dp))
                        Text(
                            text = state.collectionTitle.orEmpty(),
                            style = MaterialTheme.typography.headlineSmall.copy(
                                fontSize = 23.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                            textAlign = TextAlign.Center,
                        )
                        state.collectionSubtitle?.takeIf { it.isNotBlank() }?.let { subtitle ->
                            Spacer(Modifier.height(4.dp))
                            Text(
                                text = subtitle,
                                style = MaterialTheme.typography.titleMedium.copy(
                                    fontSize = 17.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                                color = SonataAccent,
                                textAlign = TextAlign.Center,
                            )
                        }
                        if (meta.isNotBlank()) {
                            Spacer(Modifier.height(4.dp))
                            Text(
                                text = meta,
                                style = MaterialTheme.typography.labelMedium,
                                color = Color.White.copy(alpha = 0.62f),
                            )
                        }

                        Spacer(Modifier.height(16.dp))
                        HeaderActionRow()
                        Spacer(Modifier.height(14.dp))
                    }
                }
            }

            if (state.loading && state.collectionTracks.isEmpty()) {
                detailSkeleton(trackRows = 8)
            } else {
                itemsIndexed(
                    items = state.collectionTracks,
                    key = { idx, track -> "${track.id}_$idx" },
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
        items(TopPickMood.entries, key = { it.name }) { mood ->
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
    val releaseShelves = remember(state.shelves) {
        state.shelves.filter { shelf ->
            shelf.cards.any {
                it.kind == AndroidHomeCardKind.ALBUM ||
                    it.kind == AndroidHomeCardKind.PLAYLIST ||
                    it.kind == AndroidHomeCardKind.ARTIST
            }
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "explore_heading") {
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

        itemsIndexed(moodSleeves.chunked(moodColumns)) { rowIdx, rowItems ->
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
                    val shape = RoundedCornerShape(12.dp)
                    MoodGenreSleeve(
                        title = mood.title,
                        thumbnailUrl = resolvedThumb.artworkAt(CARD_ART_PX),
                        modifier = Modifier
                            .weight(1f)
                            .aspectRatio(1.9f)
                            .clip(shape)
                            .clickable { onOpenMood(mood.title) },
                    )
                }
                repeat(moodColumns - rowItems.size) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }

        itemsIndexed(
            items = releaseShelves,
            key = { idx, shelf -> "explore_${shelf.title}_$idx" },
        ) { _, shelf ->
            val cards = shelf.cards.filter { it.kind != AndroidHomeCardKind.MOOD }
            if (cards.isNotEmpty()) {
                Column(Modifier.padding(top = 18.dp, bottom = 10.dp)) {
                    SectionHeader(title = shelf.title)
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        items(cards, key = { "${it.kind}_${it.id}_${it.title}" }) { card ->
                            val activeId = state.activeTrack?.id
                            val current = activeId != null && card.track?.id == activeId
                            ShelfCard(
                                card = card,
                                isCurrent = current,
                                isPlaying = current && state.isPlaying,
                                onClick = { onOpenCard(card, shelf.tracks) },
                            )
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
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onSignIn: () -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onOpenSearch: () -> Unit,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val playlistColumns = when {
        screenWidthDp >= 760 -> 4
        screenWidthDp >= 560 -> 3
        else -> 2
    }
    val replayArt = remember(state.tracks, state.playlists) {
        state.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            ?: state.playlists.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
    }
    val holderName = state.accountProfile?.name?.takeIf { it.isNotBlank() } ?: "SONATA LISTENER"

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "library_heading") {
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
        item(key = "library_replay_carousel") {
            LazyRow(
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                item(key = "replay_card_tracks") {
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
                item(key = "replay_card_playlists") {
                    ReplayCreditCard(
                        label = "Library Playlists",
                        value = "${state.playlists.size} Mixes",
                        detail = state.playlists.firstOrNull()?.name ?: "Sign in to sync",
                        artworkUrl = state.playlists.getOrNull(1)?.thumbnailUrl?.artworkAt(CARD_ART_PX)
                            ?: replayArt.artworkAt(CARD_ART_PX),
                        holder = holderName,
                        onClick = {
                            state.playlists.firstOrNull()?.let(onOpenPlaylist)
                        },
                        modifier = Modifier.width(300.dp),
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
        }

        // Downloads & Local Music navigation rows
        item(key = "library_device_rows") {
            Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                LibraryNavRow(
                    icon = SonataIcons.Download,
                    label = "Downloads",
                    subtitle = "Offline cached streams",
                    onClick = onOpenSearch,
                )
                HorizontalDivider(
                    color = MaterialTheme.colorScheme.outline.copy(alpha = 0.6f),
                    modifier = Modifier.padding(start = 40.dp),
                )
                LibraryNavRow(
                    icon = SonataIcons.Folder,
                    label = "Local Music",
                    subtitle = "Device audio library",
                    onClick = onOpenSearch,
                )
            }
            Spacer(Modifier.height(14.dp))
        }

        if (!state.signedIn) {
            item(key = "library_signin_banner") {
                SignInBanner(onSignIn = onSignIn)
            }
        } else {
            item(key = "library_playlists_header") {
                SectionHeader(title = "Playlists")
            }
            if (state.playlists.isEmpty()) {
                item(key = "library_empty_playlists") {
                    Text(
                        text = "Your YouTube Music playlists will appear here once synced.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 16.dp),
                    )
                }
            } else {
                items(state.playlists.chunked(playlistColumns)) { rowPlaylists ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        rowPlaylists.forEach { playlist ->
                            PlaylistGridCard(
                                playlist = playlist,
                                onClick = { onOpenPlaylist(playlist) },
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

@Composable
private fun PlaylistGridCard(
    playlist: AndroidPlaylist,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(12.dp)
    val isLikedPlaylist = playlist.id == "LM" || playlist.name.contains("Liked", ignoreCase = true)
    val cover = playlist.thumbnailUrl.artworkAt(CARD_ART_PX)
    Column(
        modifier = modifier
            .clip(shape)
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(shape)
                .background(
                    if (isLikedPlaylist || cover.isNullOrBlank()) {
                        Brush.linearGradient(listOf(Color(0xFF7B52FF), Color(0xFFF35CA8)))
                    } else {
                        Brush.linearGradient(
                            listOf(
                                MaterialTheme.colorScheme.surfaceVariant,
                                MaterialTheme.colorScheme.surfaceVariant,
                            )
                        )
                    }
                )
                .border(0.5.dp, Color.White.copy(alpha = 0.14f), shape),
            contentAlignment = Alignment.Center,
        ) {
            if (isLikedPlaylist || cover.isNullOrBlank()) {
                Icon(
                    imageVector = Icons.Rounded.ThumbUp,
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier.size(64.dp),
                )
            } else {
                AsyncImage(
                    model = cover,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
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
                    items(state.recentSearches, key = { "recent_$it" }) { q ->
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
                    item(key = "search_results_header") {
                        SectionHeader(title = state.listTitle)
                    }
                    items(state.tracks, key = { "yt_${it.id}" }) { track ->
                        val activeId = state.activeTrack?.id
                        val current = activeId != null && track.id == activeId
                        Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                            CompactTrackRow(
                                track = track,
                                isCurrent = current,
                                isPlaying = current && state.isPlaying,
                                onClick = { onPlayTrack(track, state.tracks) },
                            )
                        }
                    }
                }
            }

            1 -> {
                items(filteredLibraryTracks, key = { "lib_search_${it.id}" }) { track ->
                    val activeId = state.activeTrack?.id
                    val current = activeId != null && track.id == activeId
                    Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                        CompactTrackRow(
                            track = track,
                            isCurrent = current,
                            isPlaying = current && state.isPlaying,
                            onClick = { onPlayTrack(track, filteredLibraryTracks) },
                        )
                    }
                }
            }

            else -> {
                items(filteredPlaylists, key = { "pl_search_${it.id}" }) { playlist ->
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
                                .clip(RoundedCornerShape(8.dp))
                                .background(MaterialTheme.colorScheme.surfaceVariant),
                            contentAlignment = Alignment.Center,
                        ) {
                            val thumb = playlist.thumbnailUrl.artworkAt(ROW_ART_PX)
                            if (!thumb.isNullOrBlank()) {
                                AsyncImage(
                                    model = thumb,
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
                                if (!avatar.isNullOrBlank()) {
                                    AsyncImage(
                                        model = avatar,
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
                                color = if (state.signedIn) SonataAccent else Color(0xFF64D2FF),
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
                    val currentVer = state.updateInfo?.currentVersion ?: "0.1.0"
                    val statusLabel = when {
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
                                color = Color(0xFF64D2FF),
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
                                    runCatching {
                                        context.startActivity(
                                            Intent(Intent.ACTION_VIEW, Uri.parse(releaseTarget))
                                        )
                                    }
                                }
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = "Download Latest APK / Release",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
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

@Composable
private fun NowPlayingScreen(
    track: AndroidTrack,
    state: SonataUiState,
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
    onOpenSettings: () -> Unit,
    onDismiss: () -> Unit,
) {
    var showQueue by remember { mutableStateOf(false) }
    var dragOffsetY by remember { mutableFloatStateOf(0f) }

    val panelOpen = state.showLyrics || showQueue
    val isLiked = track.id in state.likedTrackIds
    val palette = rememberArtworkPalette(
        artworkUrl = track.thumbnailUrl,
        dark = true,
        artPx = CARD_ART_PX,
    )

    var scrubbing by remember { mutableStateOf(false) }
    var scrubFraction by remember { mutableFloatStateOf(0f) }
    val playbackFraction = if (state.durationMs > 0L) {
        (state.positionMs.toFloat() / state.durationMs.toFloat()).coerceIn(0f, 1f)
    } else {
        0f
    }
    val displayedFraction = if (scrubbing) scrubFraction else playbackFraction
    val shownMs = (displayedFraction * state.durationMs).toLong()
    val remainingMs = (state.durationMs - shownMs).coerceAtLeast(0L)

    val smoothClockMs = rememberSmoothLyricClock(
        reportedMs = state.positionMs,
        sampledAtNanos = state.positionSampledAtNanos,
        isPlaying = state.isPlaying,
        isBuffering = state.isBuffering,
        fpsLimit = state.fpsLimit,
        trackKey = track.id,
        discontinuity = state.seekDiscontinuity,
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
    val heroUrl = track.thumbnailUrl.artworkAt(PLAYER_ART_PX)
    val hasArtwork = !heroUrl.isNullOrBlank()

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
            }
            .background(palette.background),
    ) {

        // Smooth crossfade between MeshGradientBackground (artwork mode) and FullArtworkBlurBackdrop (lyrics/queue mode)
        Crossfade(
            targetState = panelOpen,
            animationSpec = tween(320, easing = FastOutSlowInEasing),
            label = "nowPlayingBgCrossfade",
        ) { blurred ->
            if (blurred) {
                Box(Modifier.fillMaxSize()) {
                    FullArtworkBlurBackdrop(
                        artworkUrl = track.thumbnailUrl.artworkAt(CARD_ART_PX),
                        modifier = Modifier.fillMaxSize(),
                    )
                    Box(
                        Modifier
                            .fillMaxSize()
                            .background(Color.Black.copy(alpha = 0.28f)),
                    )
                }
            } else {
                MeshGradientBackground(
                    palette = palette.toMeshPalette(),
                    paused = !state.isPlaying || !state.animatedArtwork,
                    trackKey = track.id,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }

        // Subtle dark readability gradient under lower controls
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        0.38f to Color.Transparent,
                        1f to Color.Black.copy(alpha = 0.46f),
                    )
                ),
        )

        // Full-bleed edge-to-edge hero cover at y = 0 dissolving smoothly into mesh backdrop
        // (covers phones and portrait tablets up to 620dp width).
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
                    .drawWithContent {
                        drawContent()
                        drawRect(
                            brush = Brush.verticalGradient(
                                colorStops = HERO_FADE_STOPS,
                                startY = size.height * (1f - HERO_FADE_FRACTION),
                                endY = size.height,
                            ),
                            blendMode = BlendMode.DstIn,
                        )
                    },
                contentAlignment = Alignment.Center,
            ) {
                if (hasArtwork) {
                    AsyncImage(
                        model = heroUrl,
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
                            imageVector = Icons.Rounded.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.45f),
                            modifier = Modifier.size(84.dp),
                        )
                    }
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
                    showQueue = showQueue,
                    isLiked = isLiked,
                    elevatedColor = palette.elevated,
                    heroUrl = heroUrl,
                    hasArtwork = hasArtwork,
                    coverScale = coverScale,
                    displayedFraction = displayedFraction,
                    shownMs = shownMs,
                    remainingMs = remainingMs,
                    smoothClockMs = smoothClockMs,
                    screenWidth = screenWidth,
                    screenHeight = screenHeight,
                    compactLandscape = compactLandscape,
                    onScrubStartOrChange = { frac ->
                        scrubbing = true
                        scrubFraction = frac
                    },
                    onScrubFinished = {
                        onSeekFraction(scrubFraction)
                        scrubbing = false
                    },
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
                    onOpenSettings = onOpenSettings,
                    modifier = Modifier.weight(1f),
                )
            } else {
                NowPlayingPortraitLayout(
                    track = track,
                    state = state,
                    showQueue = showQueue,
                    panelOpen = panelOpen,
                    isLiked = isLiked,
                    elevatedColor = palette.elevated,
                    heroUrl = heroUrl,
                    hasArtwork = hasArtwork,
                    coverScale = coverScale,
                    displayedFraction = displayedFraction,
                    shownMs = shownMs,
                    remainingMs = remainingMs,
                    smoothClockMs = smoothClockMs,
                    playerMaxWidth = playerMaxWidth,
                    headerHeight = headerHeight,
                    artTitleGap = artTitleGap,
                    playerFillsWindow = playerFillsWindow,
                    portraitWantArt = portraitWantArt,
                    portraitTopGap = portraitTopGap,
                    portraitBottomGap = portraitBottomGap,
                    onPortraitSlotHeightChanged = { portraitSlotHeightPx = it },
                    onScrubStartOrChange = { frac ->
                        scrubbing = true
                        scrubFraction = frac
                    },
                    onScrubFinished = {
                        onSeekFraction(scrubFraction)
                        scrubbing = false
                    },
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
                    onOpenSettings = onOpenSettings,
                    modifier = Modifier.weight(1f),
                )
            }
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
            Box(
                modifier = Modifier
                    .size(35.dp)
                    .clip(RoundedCornerShape(7.dp))
                    .clickable(onClick = onOpenSettings),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_output_speaker),
                    contentDescription = "Audio Output",
                    tint = Color.White.copy(alpha = 0.78f),
                    modifier = Modifier.size(24.dp),
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

    Spacer(Modifier.height(if (compact) 6.dp else 10.dp))
    Text(
        text = Build.MODEL,
        style = MaterialTheme.typography.labelMedium,
        color = Color.White.copy(alpha = 0.56f),
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
    Spacer(Modifier.height(if (compact) 2.dp else 6.dp))
}

@Composable
private fun NowPlayingScrubberAndTransportBlock(
    state: SonataUiState,
    showQueue: Boolean,
    displayedFraction: Float,
    shownMs: Long,
    remainingMs: Long,
    includeActions: Boolean = true,
    transportTopGap: Dp = 14.dp,
    transportBottomGap: Dp = 18.dp,
    onScrubStartOrChange: (Float) -> Unit,
    onScrubFinished: () -> Unit,
    onPrevious: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onToggleLyrics: () -> Unit,
    onToggleQueue: () -> Unit,
    onToggleShuffle: () -> Unit,
    onToggleRepeat: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    ThinSlider(
        value = displayedFraction,
        onValueChange = onScrubStartOrChange,
        onValueChangeFinished = onScrubFinished,
        loading = state.isBuffering,
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
        val upgrading = state.isBuffering || state.isUpgradingQuality
        LosslessLabel(
            label = if (upgrading) {
                "Upgrading Quality"
            } else {
                state.streamBadge.ifBlank { "Lossless" }
            },
            pulsing = upgrading,
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 6.dp),
        )
    }

    if (transportTopGap > 0.dp) {
        Spacer(Modifier.height(transportTopGap))
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceAround,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(
            onClick = onPrevious,
            enabled = state.hasPrevious,
            modifier = Modifier.size(64.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_previous),
                contentDescription = "Previous",
                tint = if (state.hasPrevious) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(53.dp)
                    .graphicsLayer { scaleY = 0.85f },
            )
        }

        if (state.isBuffering) {
            Box(
                modifier = Modifier.size(88.dp),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(
                    color = Color.White,
                    strokeWidth = 3.dp,
                    modifier = Modifier.size(38.dp),
                )
            }
        } else {
            IconButton(
                onClick = onTogglePlayPause,
                modifier = Modifier.size(88.dp),
            ) {
                Icon(
                    painter = painterResource(
                        if (state.isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play
                    ),
                    contentDescription = if (state.isPlaying) "Pause" else "Play",
                    tint = Color.White,
                    modifier = Modifier.size(54.dp),
                )
            }
        }

        IconButton(
            onClick = onNext,
            enabled = state.hasNext,
            modifier = Modifier.size(64.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_player_next),
                contentDescription = "Next",
                tint = if (state.hasNext) Color.White else Color.White.copy(alpha = 0.35f),
                modifier = Modifier
                    .size(53.dp)
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
    showQueue: Boolean,
    isLiked: Boolean,
    elevatedColor: Color,
    heroUrl: String?,
    hasArtwork: Boolean,
    coverScale: Float,
    displayedFraction: Float,
    shownMs: Long,
    remainingMs: Long,
    smoothClockMs: State<Long>,
    screenWidth: Dp,
    screenHeight: Dp,
    compactLandscape: Boolean,
    onScrubStartOrChange: (Float) -> Unit,
    onScrubFinished: () -> Unit,
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
                val coverShape = RoundedCornerShape(11.dp)
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
                            shape = coverShape
                            clip = true
                        }
                        .background(elevatedColor)
                        .border(0.5.dp, Color.White.copy(alpha = 0.08f), coverShape)
                        .clickable {
                            if (showQueue) onClosePanels() else onToggleLyrics()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    if (hasArtwork) {
                        AsyncImage(
                            model = heroUrl,
                            contentDescription = track.title,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Rounded.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.45f),
                            modifier = Modifier.size(84.dp),
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
                    ThinSlider(
                        value = displayedFraction,
                        onValueChange = onScrubStartOrChange,
                        onValueChangeFinished = onScrubFinished,
                        loading = state.isBuffering,
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
                        val upgrading = state.isBuffering || state.isUpgradingQuality
                        LosslessLabel(
                            label = if (upgrading) {
                                "Upgrading Quality"
                            } else {
                                state.streamBadge.ifBlank { "Lossless" }
                            },
                            pulsing = upgrading,
                            modifier = Modifier
                                .align(Alignment.TopCenter)
                                .padding(top = 6.dp),
                        )
                    }
                    Spacer(Modifier.height(4.dp))
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceAround,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        IconButton(
                            onClick = onPrevious,
                            enabled = state.hasPrevious,
                            modifier = Modifier.size(44.dp),
                        ) {
                            Icon(
                                painter = painterResource(R.drawable.ic_player_previous),
                                contentDescription = "Previous",
                                tint = if (state.hasPrevious) Color.White else Color.White.copy(alpha = 0.35f),
                                modifier = Modifier
                                    .size(34.dp)
                                    .graphicsLayer { scaleY = 0.85f },
                            )
                        }
                        if (state.isBuffering) {
                            Box(
                                modifier = Modifier.size(52.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                CircularProgressIndicator(
                                    color = Color.White,
                                    strokeWidth = 2.5.dp,
                                    modifier = Modifier.size(26.dp),
                                )
                            }
                        } else {
                            IconButton(
                                onClick = onTogglePlayPause,
                                modifier = Modifier.size(52.dp),
                            ) {
                                Icon(
                                    painter = painterResource(
                                        if (state.isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play
                                    ),
                                    contentDescription = if (state.isPlaying) "Pause" else "Play",
                                    tint = Color.White,
                                    modifier = Modifier.size(36.dp),
                                )
                            }
                        }
                        IconButton(
                            onClick = onNext,
                            enabled = state.hasNext,
                            modifier = Modifier.size(44.dp),
                        ) {
                            Icon(
                                painter = painterResource(R.drawable.ic_player_next),
                                contentDescription = "Next",
                                tint = if (state.hasNext) Color.White else Color.White.copy(alpha = 0.35f),
                                modifier = Modifier
                                    .size(34.dp)
                                    .graphicsLayer { scaleY = 0.85f },
                            )
                        }
                    }
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
                    durationMs = state.durationMs,
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
                            durationMs = state.durationMs,
                            onClick = onOpenLyrics,
                        )

                        NowPlayingScrubberAndTransportBlock(
                            state = state,
                            showQueue = showQueue,
                            displayedFraction = displayedFraction,
                            shownMs = shownMs,
                            remainingMs = remainingMs,
                            includeActions = false,
                            transportTopGap = if (compactLandscape) 0.dp else 8.dp,
                            onScrubStartOrChange = onScrubStartOrChange,
                            onScrubFinished = onScrubFinished,
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
    showQueue: Boolean,
    panelOpen: Boolean,
    isLiked: Boolean,
    elevatedColor: Color,
    heroUrl: String?,
    hasArtwork: Boolean,
    coverScale: Float,
    displayedFraction: Float,
    shownMs: Long,
    remainingMs: Long,
    smoothClockMs: State<Long>,
    playerMaxWidth: Dp,
    headerHeight: Dp,
    artTitleGap: Dp,
    playerFillsWindow: Boolean,
    portraitWantArt: Dp,
    portraitTopGap: Dp,
    portraitBottomGap: Dp,
    onPortraitSlotHeightChanged: (Int) -> Unit,
    onScrubStartOrChange: (Float) -> Unit,
    onScrubFinished: () -> Unit,
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
                            val thumbUrl = track.thumbnailUrl.artworkAt(CARD_ART_PX)
                            Box(
                                modifier = Modifier
                                    .size(54.dp)
                                    .clip(RoundedCornerShape(8.dp))
                                    .background(Color.White.copy(alpha = 0.1f))
                                    .border(0.5.dp, Color.White.copy(alpha = 0.18f), RoundedCornerShape(8.dp))
                                    .clickable(onClick = onClosePanels),
                                contentAlignment = Alignment.Center,
                            ) {
                                if (!thumbUrl.isNullOrBlank()) {
                                    AsyncImage(
                                        model = thumbUrl,
                                        contentDescription = null,
                                        contentScale = ContentScale.Crop,
                                        modifier = Modifier.fillMaxSize(),
                                    )
                                } else {
                                    Icon(
                                        imageVector = Icons.Rounded.MusicNote,
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
                                    durationMs = state.durationMs,
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
                                val artShape = RoundedCornerShape(12.dp)
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
                                            shape = artShape
                                            clip = true
                                        }
                                        .background(elevatedColor)
                                        .border(0.5.dp, Color.White.copy(alpha = 0.08f), artShape),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    if (hasArtwork) {
                                        AsyncImage(
                                            model = heroUrl,
                                            contentDescription = track.title,
                                            contentScale = ContentScale.Crop,
                                            modifier = Modifier.fillMaxSize(),
                                        )
                                    } else {
                                        Icon(
                                            imageVector = Icons.Rounded.MusicNote,
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
                durationMs = state.durationMs,
                onClick = onOpenLyrics,
            )
        } else {
            Spacer(Modifier.height(8.dp))
        }

        NowPlayingScrubberAndTransportBlock(
            state = state,
            showQueue = showQueue,
            displayedFraction = displayedFraction,
            shownMs = shownMs,
            remainingMs = remainingMs,
            includeActions = true,
            transportTopGap = if (panelOpen) 12.dp else portraitTopGap,
            transportBottomGap = if (panelOpen) 14.dp else portraitBottomGap,
            onScrubStartOrChange = onScrubStartOrChange,
            onScrubFinished = onScrubFinished,
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
    durationMs: Long,
    onClick: () -> Unit,
) {
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
                        (fadeIn(tween(240)) + slideInVertically(tween(240)) { it / 2 }) togetherWith
                            (fadeOut(tween(180)) + slideOutVertically(tween(180)) { -it / 2 })
                    },
                    label = "liveLyricStripLine",
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
    durationMs: Long,
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
            .drawWithContent {
                drawContent()
                val topFadePx = 14.dp.toPx().coerceAtMost(size.height * 0.12f)
                val bottomFadePx = 42.dp.toPx().coerceAtMost(size.height * 0.22f)
                drawRect(
                    brush = Brush.verticalGradient(
                        0f to Color.Transparent,
                        (topFadePx / size.height.coerceAtLeast(1f)).coerceIn(0.01f, 0.15f) to Color.Black,
                        (1f - (bottomFadePx / size.height.coerceAtLeast(1f))).coerceIn(0.75f, 0.99f) to Color.Black,
                        1f to Color.Transparent,
                    ),
                    blendMode = BlendMode.DstIn,
                )
            },
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = 14.dp, bottom = bottomPadDp),
            verticalArrangement = Arrangement.spacedBy(if (isTablet) 24.dp else 20.dp),
        ) {
            itemsIndexed(lyrics) { index, line ->
                val isActive = index == activeIndex
                val rawTrimmed = line.text.trim()
                val isCounter = line.isCounterVoice ||
                    rawTrimmed.startsWith("v2:", ignoreCase = true) ||
                    rawTrimmed.startsWith("bg:", ignoreCase = true)
                val cleanText = rawTrimmed
                    .removePrefix("v2:").removePrefix("V2:")
                    .removePrefix("v1:").removePrefix("V1:")
                    .removePrefix("bg:").removePrefix("BG:")
                    .trim()
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
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { onSeekTo(lineStartMs) }
                            .padding(vertical = 4.dp),
                    )

                    // 3-dot instrumental break indicator when a >= 5.5s gap follows this verse
                    if (nextLineStartMs != null && (nextLineStartMs - lineStartMs) >= 5_500L) {
                        val breakStartMs = lineStartMs + minOf(
                            (cleanText.length.coerceAtLeast(3) * 65L).coerceIn(1_100L, 3_600L),
                            ((nextLineStartMs - lineStartMs) * 55L / 100L),
                        )
                        val nowMs = clockMs.value
                        if (isActive && nowMs >= breakStartMs && nowMs < nextLineStartMs) {
                            val breakProgress = ((nowMs - breakStartMs).toFloat() /
                                (nextLineStartMs - breakStartMs).coerceAtLeast(1L).toFloat()).coerceIn(0f, 1f)
                            Row(
                                modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                repeat(3) { dotIdx ->
                                    val dotThreshold = (dotIdx + 1) / 3.2f
                                    val lit = breakProgress >= dotThreshold
                                    Box(
                                        modifier = Modifier
                                            .size(if (lit) 9.dp else 7.dp)
                                            .clip(CircleShape)
                                            .background(
                                                Color.White.copy(alpha = if (lit) 0.92f else 0.28f)
                                            ),
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
        item(key = "queue_header") {
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
        itemsIndexed(queue, key = { idx, t -> "q_${t.id}_$idx" }) { idx, item ->
            val isCurrent = idx == queueIndex
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .background(if (isCurrent) Color.White.copy(alpha = 0.14f) else Color.Transparent)
                    .clickable { onSelectIndex(idx) }
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(RoundedCornerShape(7.dp))
                        .background(Color.White.copy(alpha = 0.1f)),
                    contentAlignment = Alignment.Center,
                ) {
                    val thumb = item.thumbnailUrl.artworkAt(ROW_ART_PX)
                    if (!thumb.isNullOrBlank()) {
                        AsyncImage(
                            model = thumb,
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Rounded.MusicNote,
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
                                imageVector = if (isPlaying) Icons.Rounded.GraphicEq else Icons.Rounded.Pause,
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

private fun formatDurationMs(ms: Long): String {
    val totalSec = (ms / 1000L).coerceAtLeast(0L)
    val mins = totalSec / 60L
    val secs = totalSec % 60L
    return "$mins:${secs.toString().padStart(2, '0')}"
}
