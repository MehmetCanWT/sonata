/*
 * Expressive Search tab screen with recent discovery carousel and live segmented filtering.
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


@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
internal fun SearchTabContent(
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
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val recentCols = when {
        screenWidthDp >= 900 -> 6
        screenWidthDp >= 600 -> 4
        else -> 3
    }
    val sampleCols = when {
        screenWidthDp >= 900 -> 5
        screenWidthDp >= 600 -> 3
        else -> 2
    }
    val sampleRowsCount = 2
    val maxQuickChips = when {
        screenWidthDp >= 900 -> 10
        screenWidthDp >= 600 -> 7
        else -> 6
    }

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

    // Quick suggestion pills (recent searches + top artists from user's personalized feed)
    val quickChips = remember(state.recentSearches, state.lastPlayedTracks, state.shelves, maxQuickChips) {
        val fromArtists = (state.lastPlayedTracks.map { it.artist } +
            state.shelves.flatMap { it.tracks }.map { it.artist })
            .map { it.substringBefore(",").substringBefore("&").trim() }
            .filter { it.length in 2..22 && !it.equals("Unknown Artist", ignoreCase = true) }
        (state.recentSearches + fromArtists)
            .distinctBy { it.lowercase() }
            .take(maxQuickChips)
    }

    // Recent visual square cards (exact fit to screen width — zero right-edge overflow)
    val recentVisualTracks = remember(state.lastPlayedTracks, state.tracks, recentCols) {
        (state.lastPlayedTracks + state.tracks)
            .filter { !it.thumbnailUrl.isNullOrBlank() }
            .distinctBy { it.id }
            .take(recentCols)
    }

    // Vertical Sample discovery grid (exact fit to screen width & fills Portrait vertically)
    val sampleTracks = remember(state.shelves, state.tracks, state.lastPlayedTracks, sampleCols) {
        val recentHeadIds = state.lastPlayedTracks.take(4).mapTo(HashSet()) { it.id }
        val pool = state.shelves.flatMap { shelf ->
            shelf.tracks + shelf.cards.mapNotNull { it.track }
        } + state.tracks
        val preferred = pool
            .filter { !it.thumbnailUrl.isNullOrBlank() && it.id !in recentHeadIds }
            .distinctBy { it.id }
        (preferred.ifEmpty { pool.filter { !it.thumbnailUrl.isNullOrBlank() }.distinctBy { it.id } })
            .take(sampleCols * sampleRowsCount)
    }
    val sampleRows = remember(sampleTracks, sampleCols) { sampleTracks.chunked(sampleCols) }
    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        // Wrapped quick-search pill chips (fits cleanly within page margins — never clips mid-word)
        if (quickChips.isNotEmpty()) {
            item(key = "search_quick_chips", contentType = "search_chips") {
                FlowRow(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    quickChips.forEach { chipText ->
                        val isActiveQuery = state.query.equals(chipText, ignoreCase = true)
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(22.dp))
                                .background(
                                    if (isActiveQuery) SonataAccent.copy(alpha = 0.20f)
                                    else MaterialTheme.colorScheme.surfaceVariant,
                                )
                                .border(
                                    width = 0.5.dp,
                                    color = if (isActiveQuery) SonataAccent.copy(alpha = 0.6f)
                                    else MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                                    shape = RoundedCornerShape(22.dp),
                                )
                                .clickable { onSearchQuery(chipText) }
                                .padding(horizontal = 15.dp, vertical = 8.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = chipText,
                                style = MaterialTheme.typography.labelLarge.copy(
                                    fontSize = 13.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                                color = if (isActiveQuery) SonataAccent else MaterialTheme.colorScheme.onBackground,
                                maxLines = 1,
                            )
                        }
                    }
                }
            }
        }

        if (state.query.isBlank()) {
            // Material 3 Expressive Discovery Mode — fitted grid (zero right overflow, zero portrait dead space)
            if (recentVisualTracks.isNotEmpty()) {
                item(key = "search_recent_header", contentType = "section_header") {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(start = PAGE_GUTTER, end = PAGE_GUTTER, top = 10.dp, bottom = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        ExpressiveScallopedBadge(
                            badgeColor = Color(0xFFC5D2FF),
                            icon = SonataIcons.History,
                            iconTint = Color(0xFF141826),
                        )
                        Spacer(Modifier.width(12.dp))
                        Text(
                            text = "Recent searches",
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 21.sp,
                                fontWeight = FontWeight.Bold,
                                letterSpacing = (-0.3).sp,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.weight(1f),
                        )
                        if (state.recentSearches.isNotEmpty()) {
                            Text(
                                text = "Clear",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = SonataAccent,
                                modifier = Modifier
                                    .clip(RoundedCornerShape(12.dp))
                                    .clickable(onClick = onClearRecentSearches)
                                    .padding(horizontal = 8.dp, vertical = 4.dp),
                            )
                        }
                    }
                }
                item(key = "search_recent_cards", contentType = "search_recent_row") {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(start = PAGE_GUTTER, end = PAGE_GUTTER, bottom = 22.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        recentVisualTracks.forEach { track ->
                            SearchRecentSquareCard(
                                track = track,
                                isCurrent = state.activeTrack?.id == track.id,
                                isPlaying = state.activeTrack?.id == track.id && state.isPlaying,
                                onClick = { onPlayTrack(track, recentVisualTracks) },
                                onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                        repeat((recentCols - recentVisualTracks.size).coerceAtLeast(0)) {
                            Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }

            if (sampleRows.isNotEmpty()) {
                item(key = "search_samples_header", contentType = "section_header") {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(start = PAGE_GUTTER, end = PAGE_GUTTER, top = 2.dp, bottom = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        ExpressiveScallopedBadge(
                            badgeColor = Color(0xFFF6B7D1),
                            icon = SonataIcons.GraphicEq,
                            iconTint = Color(0xFF28121C),
                        )
                        Spacer(Modifier.width(12.dp))
                        Text(
                            text = "Samples",
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 21.sp,
                                fontWeight = FontWeight.Bold,
                                letterSpacing = (-0.3).sp,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                        )
                    }
                }
                itemsIndexed(
                    items = sampleRows,
                    key = { rowIdx, row -> "sample_row_${rowIdx}_${row.firstOrNull()?.id.orEmpty()}" },
                    contentType = { _, _ -> "sample_grid_row" },
                ) { _, rowTracks ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(start = PAGE_GUTTER, end = PAGE_GUTTER, bottom = 14.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        rowTracks.forEach { track ->
                            SearchSamplePortraitCard(
                                track = track,
                                isCurrent = state.activeTrack?.id == track.id,
                                isPlaying = state.activeTrack?.id == track.id && state.isPlaying,
                                onClick = { onPlayTrack(track, sampleTracks) },
                                onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                        repeat((sampleCols - rowTracks.size).coerceAtLeast(0)) {
                            Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        } else {
            // Active Query Results Mode
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
                        CircularProgressIndicator(color = LocalLunariaColors.current.petalLavender, strokeWidth = 2.5.dp)
                    }
                }
            }

            when (selectedSegment) {
                0 -> {
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
                                        tint = LocalLunariaColors.current.onSurfaceSecondary,
                                    )
                                }
                            }
                            Spacer(Modifier.width(12.dp))
                            Text(
                                text = playlist.name,
                                style = MaterialTheme.typography.titleMedium,
                                color = LocalLunariaColors.current.onSurfacePrimary,
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
}

@Composable
internal fun ExpressiveScallopedBadge(
    badgeColor: Color,
    icon: ImageVector,
    iconTint: Color,
    size: Dp = 36.dp,
) {
    Box(
        modifier = Modifier.size(size),
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
            val cx = this.size.width / 2f
            val cy = this.size.height / 2f
            val baseR = this.size.minDimension * 0.44f
            val amp = this.size.minDimension * 0.06f
            val lobes = 10
            val steps = 120
            val path = androidx.compose.ui.graphics.Path()
            for (i in 0..steps) {
                val theta = (i.toDouble() / steps) * 2.0 * kotlin.math.PI
                val r = baseR + amp * kotlin.math.cos(lobes * theta).toFloat()
                val x = cx + r * kotlin.math.cos(theta).toFloat()
                val y = cy + r * kotlin.math.sin(theta).toFloat()
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            path.close()
            drawPath(path = path, color = badgeColor)
        }
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = iconTint,
            modifier = Modifier.size(18.dp),
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun SearchRecentSquareCard(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: (Rect?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val artShape = tokens.cardShape
    val sizedUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(CARD_ART_PX) }
    val artReq = rememberArtworkRequest(sizedUrl, CARD_ART_PX, CARD_ART_PX)

    Column(
        modifier = modifier
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(cardBounds)
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(artShape)
                .background(tokens.eclipseCard)
                .border(0.75.dp, tokens.cardStroke, artShape),
            contentAlignment = Alignment.Center,
        ) {
            if (artReq != null) {
                AsyncImage(
                    model = artReq,
                    contentDescription = track.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = tokens.onSurfaceSecondary,
                    modifier = Modifier.size(34.dp),
                )
            }
            if (isCurrent) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color.Black.copy(alpha = 0.42f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = if (isPlaying) SonataIcons.GraphicEq else SonataIcons.Pause,
                        contentDescription = null,
                        tint = tokens.petalLavender,
                        modifier = Modifier.size(28.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = track.title,
            style = MaterialTheme.typography.titleSmall.copy(
                fontSize = 14.5.sp,
                fontWeight = FontWeight.SemiBold,
            ),
            color = if (isCurrent) tokens.petalLavender else tokens.onSurfacePrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = track.artist,
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
            color = tokens.onSurfaceSecondary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun SearchSamplePortraitCard(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    onClick: () -> Unit,
    onLongClick: (Rect?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val cardShape = remember { RoundedCornerShape(24.dp) }
    val sizedUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(HEADER_ART_PX) }
    val artReq = rememberArtworkRequest(sizedUrl, HEADER_ART_PX, HEADER_ART_PX)
    val scrim = remember {
        Brush.verticalGradient(
            0.0f to Color.Transparent,
            0.48f to Color.Transparent,
            0.78f to Color.Black.copy(alpha = 0.62f),
            1.0f to Color.Black.copy(alpha = 0.88f),
        )
    }

    Box(
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(0.68f)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .clip(cardShape)
            .background(LocalLunariaColors.current.eclipseCard)
            .border(0.75.dp, LocalLunariaColors.current.cardStroke, cardShape)
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(cardBounds)
                },
            ),
    ) {
        if (artReq != null) {
            AsyncImage(
                model = artReq,
                contentDescription = track.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(scrim),
        )
        Row(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = track.title,
                    style = MaterialTheme.typography.titleSmall.copy(
                        fontSize = 14.5.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = Color.White,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = track.artist,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                    color = Color.White.copy(alpha = 0.72f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.width(8.dp))
            Box(
                modifier = Modifier
                    .size(38.dp)
                    .clip(CircleShape)
                    .background(Color.White),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = if (isCurrent && isPlaying) SonataIcons.Pause else SonataIcons.Play,
                    contentDescription = "Play sample",
                    tint = Color(0xFF101012),
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

