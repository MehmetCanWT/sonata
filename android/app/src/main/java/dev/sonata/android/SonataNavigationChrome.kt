/*
 * Shared floating navigation bar, top chrome, and mini-player for Sonata Android.
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


internal val TopBarContentHeight = 52.dp
internal val NavBarLift = 10.dp
internal val InlineSpacing = 12.dp
internal val InlineGutter = 24.dp

internal val Shape7 = RoundedCornerShape(7.dp)
internal val Shape8 = RoundedCornerShape(8.dp)
internal val Shape10 = RoundedCornerShape(10.dp)
internal val Shape11 = RoundedCornerShape(11.dp)
internal val Shape12 = RoundedCornerShape(12.dp)
internal val Shape14 = RoundedCornerShape(14.dp)
internal val Shape20 = RoundedCornerShape(20.dp)
internal val Shape22 = RoundedCornerShape(22.dp)
internal val Pill50Shape = RoundedCornerShape(percent = 50)
internal val CapsulePillShape = SquirclePillShape()
internal val NowPlayingBottomScrim = Brush.verticalGradient(
    0.38f to Color.Transparent,
    1f to Color.Black.copy(alpha = 0.46f),
)

internal data class NavTabSpec(
    val tab: AndroidTab,
    val label: String,
    val icon: ImageVector,
)

internal val PrimaryTabs = listOf(
    NavTabSpec(AndroidTab.Home, "Home", SonataIcons.Home),
    NavTabSpec(AndroidTab.Explore, "Explore", SonataIcons.TabExplore),
    NavTabSpec(AndroidTab.Radio, "Radio", SonataRadioWavesIcon),
    NavTabSpec(AndroidTab.Library, "Library", SonataIcons.TabLibrary),
    NavTabSpec(AndroidTab.Search, "Search", SonataIcons.TabSearch),
)


@Composable
internal fun Modifier.adaptiveLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color = LocalLunariaColors.current.eclipseSurface.copy(alpha = 0.92f),
    surfaceTint: Color? = null,
    chromaticAberration: Boolean = true,
): Modifier = sonataThemeGlassButton(
    shape = shape,
    surfaceTint = surfaceTint,
    fallbackColor = fallbackColor,
    chromaticAberration = chromaticAberration,
)


@Composable
internal fun FloatingTopChrome(
    state: SonataUiState,
    showAllMoods: Boolean,
    showListeningHistory: Boolean = false,
    onBack: () -> Unit,
    onQueryChange: (String) -> Unit,
    onSearchSubmit: () -> Unit,
    onShareCollectionClick: (() -> Unit)? = null,
    onHistoryClick: () -> Unit,
    onAvatarClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val pillShape = remember { CircleShape }
    val showBack = state.collectionTitle != null || showAllMoods || showListeningHistory
    val showSearch = state.activeTab == AndroidTab.Search && !showBack
    val glassInk = glassContentColor()
    val backIconTint = glassInk
    val context = LocalContext.current

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
                    tint = backIconTint,
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
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.weight(1f),
                )
            } else if (showListeningHistory) {
                Text(
                    text = "Listening History",
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.weight(1f),
                )
            } else {
                Spacer(Modifier.weight(1f))
            }
        }

        if (state.collectionTitle != null && !showBack) {
            // Unreachable, kept for structural clarity.
        } else if (state.collectionTitle != null && showBack) {
            val capsuleShape = remember { SquirclePillShape() }
            val detailShareClick = {
                if (onShareCollectionClick != null) {
                    onShareCollectionClick()
                } else {
                    val id = state.collectionBrowseId.orEmpty()
                    val url = buildShareUrlForCollection(id, state.collectionTitle.orEmpty())
                    val title = state.collectionTitle.orEmpty()
                    val sendIntent = Intent(Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(Intent.EXTRA_SUBJECT, title)
                        putExtra(Intent.EXTRA_TEXT, url)
                    }
                    context.startActivity(Intent.createChooser(sendIntent, "Share"))
                }
            }
            Row(
                modifier = Modifier
                    .height(44.dp)
                    .adaptiveLiquidGlass(shape = capsuleShape, chromaticAberration = false)
                    .clip(capsuleShape)
                    .padding(horizontal = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(32.dp)
                        .clip(CircleShape)
                        .clickable(onClick = detailShareClick),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = DetailShareIcon,
                        contentDescription = "Share",
                        tint = glassInk,
                        modifier = Modifier.size(18.dp),
                    )
                }
                Box(
                    modifier = Modifier
                        .width(1.dp)
                        .height(18.dp)
                        .background(glassInk.copy(alpha = 0.22f)),
                )
                AvatarCircleContent(
                    profile = state.accountProfile,
                    signedIn = state.signedIn,
                    size = 32.dp,
                    onClick = onAvatarClick,
                )
            }
        } else {
            val capsuleShape = remember { SquirclePillShape() }
            Row(
                modifier = Modifier
                    .height(44.dp)
                    .adaptiveLiquidGlass(shape = capsuleShape, chromaticAberration = false)
                    .clip(capsuleShape)
                    .padding(horizontal = 5.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(34.dp)
                        .clip(CircleShape)
                        .clickable(onClick = onHistoryClick),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.History,
                        contentDescription = "Listening History",
                        tint = glassInk,
                        modifier = Modifier.size(19.dp),
                    )
                }
                Box(
                    modifier = Modifier
                        .width(1.dp)
                        .height(18.dp)
                        .background(glassInk.copy(alpha = 0.18f)),
                )
                AvatarCircleContent(
                    profile = state.accountProfile,
                    signedIn = state.signedIn,
                    size = 34.dp,
                    onClick = onAvatarClick,
                )
            }
        }
    }
}

@Composable
internal fun AvatarCircleContent(
    profile: AndroidAccountProfile?,
    signedIn: Boolean,
    size: Dp,
    onClick: () -> Unit,
) {
    val avatar = if (signedIn) profile?.avatarUrl else null
    val avatarRequest = rememberArtworkRequest(avatar, 128, 128)
    val glassInk = glassContentColor()
    Box(
        modifier = Modifier
            .size(size)
            .clip(CircleShape)
            .background(glassInk.copy(alpha = 0.08f))
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
                tint = glassInk,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

internal val DetailShareIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "DetailShare",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(8.6f, 13.5f); lineTo(15.4f, 17.5f)
            moveTo(15.4f, 6.5f); lineTo(8.6f, 10.5f)
        }
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            fill = SolidColor(Color.Black),
        ) {
            for ((cx, cy) in listOf(18f to 5f, 6f to 12f, 18f to 19f)) {
                moveTo(cx + 2.4f, cy)
                arcToRelative(2.4f, 2.4f, 0f, true, true, -4.8f, 0f)
                arcToRelative(2.4f, 2.4f, 0f, true, true, 4.8f, 0f)
                close()
            }
        }
    }.build()
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
internal fun SonataGlassNavBar(
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
    val themeTokens = LocalLunariaColors.current
    val glassSupported = LocalLiquidGlassEnabled.current && isGlassSupported()
    val contentColor = glassContentColor()
    val selectedColor = contentColor
    val unselectedColor = contentColor.copy(alpha = 0.62f)
    val fallbackSurface = themeTokens.eclipseSurface
    val selectedIndex = PrimaryTabs.indexOfFirst { it.tab == selectedTab }.coerceAtLeast(0)
    val tabClickCallbacks = remember(onTabSelected) {
        PrimaryTabs.map { spec -> { onTabSelected(spec.tab) } }
    }
    val navBarShape = themeTokens.navBarShape
    val miniPlayerShape = themeTokens.miniPlayerShape
    val glassSurface: @Composable () -> Modifier = remember(
        navBarShape,
        fallbackSurface,
        themeTokens.designTheme,
        themeTokens.isDark,
    ) {
        {
            Modifier.adaptiveLiquidGlass(
                shape = navBarShape,
                fallbackColor = fallbackSurface.copy(alpha = 0.94f),
            )
        }
    }
    val miniGlassSurface: @Composable () -> Modifier = remember(
        miniPlayerShape,
        fallbackSurface,
        themeTokens.designTheme,
        themeTokens.isDark,
    ) {
        {
            Modifier.adaptiveLiquidGlass(
                shape = miniPlayerShape,
                fallbackColor = fallbackSurface.copy(alpha = 0.94f),
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
                    modifier = accessoryModifier.then(miniGlassSurface()),
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
                    modifier = accessoryModifier.fillMaxWidth().then(miniGlassSurface()),
                )
            }
        },
        colors = FloatingTabBarDefaults.colors(
            backgroundColor = if (glassSupported) Color.Transparent else fallbackSurface,
            accessoryBackgroundColor = if (glassSupported) Color.Transparent else fallbackSurface,
            indicatorColor = glassIndicatorColor(),
        ),
        shapes = FloatingTabBarDefaults.shapes(
            tabBarShape = navBarShape,
            accessoryShape = miniPlayerShape,
        ),
        elevations = FloatingTabBarDefaults.elevations(
            inlineElevation = 0.dp,
            expandedElevation = 0.dp,
        ),
        sizes = FloatingTabBarDefaults.sizes(
            tabBarContentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
            tabExpandedContentPadding = PaddingValues(vertical = 6.dp),
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
internal fun GlassMiniPlayer(
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
    val themeTokens = LocalLunariaColors.current
    val isViolet = themeTokens.isViolet
    val pressSource = remember { MutableInteractionSource() }
    val isPressed by pressSource.collectIsPressedAsState()
    val pressScale by animateFloatAsState(
        targetValue = if (isPressed) 1.03f else 1f,
        animationSpec = spring(stiffness = 400f),
        label = "miniPressScale",
    )
    val barHeight = when {
        isInline -> 48.dp
        else -> themeTokens.miniPlayerHeight
    }
    val artSize = when {
        isInline -> 34.dp
        isViolet -> 44.dp
        else -> 38.dp
    }

    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .height(barHeight)
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
                    start = if (isViolet) 14.dp else 12.dp,
                    end = if (isInline) 6.dp else 10.dp,
                    top = 5.dp,
                    bottom = 5.dp,
                ),
        ) {
            val artShape = if (isViolet) RoundedCornerShape(11.dp) else RoundedCornerShape(10.dp)
            Box(
                modifier = Modifier
                    .size(artSize)
                    .clip(artShape)
                    .background(themeTokens.onSurfacePrimary.copy(alpha = 0.08f))
                    .border(0.5.dp, themeTokens.cardStroke, artShape),
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
            Spacer(Modifier.width(if (isInline) 8.dp else 12.dp))
            if (isViolet || isInline) {
                // Violet stacked two-line layout (`Violet.png`): Bold title above soft orchid artist.
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .clipToBounds(),
                ) {
                    MarqueeText(
                        text = track.title,
                        style = MaterialTheme.typography.titleSmall.copy(
                            fontSize = if (isInline) 13.5.sp else 15.5.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = contentColor,
                        enabled = isPlaying,
                    )
                    Text(
                        text = track.artist,
                        style = MaterialTheme.typography.bodySmall.copy(fontSize = if (isInline) 11.5.sp else 13.sp),
                        color = themeTokens.onSurfaceSecondary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            } else {
                // Moonflower inline capsule layout (`moonflower.png`): "Saturn Return — Luma Vale".
                val inlineLine = remember(track.title, track.artist, contentColor, themeTokens.onSurfaceSecondary) {
                    buildAnnotatedString {
                        withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = contentColor)) {
                            append(track.title)
                        }
                        if (track.artist.isNotBlank()) {
                            withStyle(SpanStyle(fontWeight = FontWeight.Normal, color = themeTokens.onSurfaceSecondary)) {
                                append(" — ")
                                append(track.artist)
                            }
                        }
                    }
                }
                Text(
                    text = inlineLine,
                    style = MaterialTheme.typography.titleSmall.copy(fontSize = 14.5.sp),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
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
                        imageVector = if (isPlaying) SonataIcons.Pause else SonataIcons.Play,
                        contentDescription = if (isPlaying) "Pause" else "Play",
                        tint = contentColor,
                        modifier = Modifier.size(if (isInline) 18.dp else 20.dp),
                    )
                }
            }
            if (!isInline) {
                IconButton(
                    onClick = onNext,
                    modifier = Modifier.size(38.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.FastForward,
                        contentDescription = "Next",
                        tint = contentColor,
                        modifier = Modifier.size(22.dp),
                    )
                }
            }
        }
    }
}
