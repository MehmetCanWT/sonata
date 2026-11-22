/*
 * Explore tab screen, editorial hero carousel, Viral Hits chart, and All Moods grid.
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


@Composable
internal fun AllMoodsScreen(
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

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
internal fun ExploreTabContent(
    state: SonataUiState,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onRefresh: () -> Unit = {},
    onOpenMood: (String) -> Unit,
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onCardLongClick: (AndroidHomeCard, Rect?) -> Unit = { _, _ -> },
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit = { _, _ -> },
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Int, Rect?) -> Unit = { _, _, _ -> },
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val moodColumns = if (screenWidthDp >= 780) 3 else 2
    val heroCardWidth = when {
        screenWidthDp >= 900 -> 380.dp
        screenWidthDp >= 600 -> 348.dp
        else -> (screenWidthDp - 48).coerceIn(286, 336).dp
    }
    val songColumnWidth = when {
        screenWidthDp >= 900 -> ((screenWidthDp - 56) / 3f).coerceIn(270f, 340f).dp
        screenWidthDp >= 600 -> ((screenWidthDp - 48) / 2f).coerceIn(270f, 360f).dp
        else -> (screenWidthDp - 44).coerceIn(280, 360).dp
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
        val raw = state.shelves.filter { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" !in t && "unutulan" !in t &&
                shelf.cards.any {
                    it.kind == AndroidHomeCardKind.ALBUM ||
                        it.kind == AndroidHomeCardKind.PLAYLIST ||
                        it.kind == AndroidHomeCardKind.ARTIST ||
                        it.kind == AndroidHomeCardKind.VIDEO
                }
        }
        mergeNewReleaseShelves(raw)
            .filter { it.cards.isNotEmpty() }
            .distinctBy { it.title.trim().lowercase() }
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

    // Editorial featured hero cards (Playlists, New Releases & Charts with rich artwork, deduplicated)
    val editorialHeroes = remember(topReleaseAndChartShelves, orderedExploreShelves) {
        val primary = (topReleaseAndChartShelves + orderedExploreShelves)
            .flatMap { shelf ->
                shelf.cards.map { card -> card to shelf }
            }
            .filter { (card, _) ->
                (card.kind == AndroidHomeCardKind.PLAYLIST || card.kind == AndroidHomeCardKind.ALBUM) &&
                    !card.thumbnailUrl.isNullOrBlank()
            }
            .distinctBy { (card, _) -> card.id.removePrefix("VL").ifBlank { card.dedupeContentKey() } }
            .distinctBy { (card, _) -> card.dedupeContentKey() }
            .take(8)
        primary
    }

    // Trending / Viral Hits 4-row horizontal paging song columns
    val viralHitsTracks = remember(state.shelves, state.tracks) {
        val trendShelves = state.shelves.filter { shelf ->
            val t = shelf.title.lowercase()
            ("trend" in t || "chart" in t || "hit" in t || "quick" in t || "new" in t || "pop" in t) &&
                "forgotten" !in t && "unutulan" !in t
        }
        val fromTrend = trendShelves.flatMap { shelf ->
            shelf.tracks + shelf.cards.mapNotNull { it.track }
        }
        val fromAll = state.shelves.flatMap { shelf ->
            shelf.tracks + shelf.cards.mapNotNull { it.track }
        }
        (fromTrend + fromAll + state.tracks)
            .distinctBy { it.id }
            .take(24)
    }
    val viralColumns = remember(viralHitsTracks) { viralHitsTracks.chunked(4) }

    val moodRows = remember(moodSleeves, moodColumns) { moodSleeves.chunked(moodColumns) }
    val heroRowState = rememberLazyListState()
    val heroSnap = rememberSnapFlingBehavior(lazyListState = heroRowState)
    val viralRowState = rememberLazyListState()
    val viralSnap = rememberSnapFlingBehavior(lazyListState = viralRowState)
    TrackScrollActivity(listState)

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
        item(key = "explore_heading", contentType = "tab_heading") {
            Text(
                text = "Explore",
                style = MaterialTheme.typography.displayLarge.copy(
                    fontSize = 34.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
            )
        }

        // 1. Editorial Hero Carousel (Eyebrow + Title + Subtitle above wide rounded cover)
        if (editorialHeroes.isNotEmpty()) {
            item(key = "explore_editorial_hero", contentType = "explore_hero") {
                LazyRow(
                    state = heroRowState,
                    flingBehavior = heroSnap,
                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                    modifier = Modifier.padding(top = 4.dp, bottom = 20.dp),
                ) {
                    items(
                        items = editorialHeroes,
                        key = { (card, _) -> "hero_${card.kind}_${card.id}" },
                        contentType = { "editorial_hero_card" },
                    ) { (card, shelf) ->
                        val eyebrow = when {
                            card.kind == AndroidHomeCardKind.ALBUM -> "NEW RELEASE"
                            "chart" in shelf.title.lowercase() || "trend" in shelf.title.lowercase() -> "FEATURED CHART"
                            else -> "UPDATED PLAYLIST"
                        }
                        val caption = card.subtitle.ifBlank { shelf.title }
                        ExploreEditorialHeroCard(
                            eyebrow = eyebrow,
                            title = card.title,
                            subtitle = caption,
                            artworkUrl = card.thumbnailUrl,
                            cardWidth = heroCardWidth,
                            onClick = { onOpenCard(card, shelf.tracks) },
                            onLongClick = { bounds -> onCardLongClick(card, bounds) },
                        )
                    }
                }
            }
        }

        // 2. Trending & Viral Hits — 4-row horizontal paging track columns
        if (viralColumns.isNotEmpty()) {
            item(key = "explore_viral_hits", contentType = "explore_viral_hits") {
                Column(modifier = Modifier.padding(bottom = 22.dp)) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Star,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.size(20.dp),
                        )
                        Spacer(Modifier.width(7.dp))
                        Text(
                            text = "Trending & Viral Hits",
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 21.sp,
                                fontWeight = FontWeight.Bold,
                                letterSpacing = (-0.3).sp,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        Spacer(Modifier.width(4.dp))
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(20.dp),
                        )
                    }

                    LazyRow(
                        state = viralRowState,
                        flingBehavior = viralSnap,
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        itemsIndexed(
                            items = viralColumns,
                            key = { colIdx, col -> "viral_col_${colIdx}_${col.firstOrNull()?.id.orEmpty()}" },
                            contentType = { _, _ -> "viral_column" },
                        ) { colIdx, columnTracks ->
                            Column(modifier = Modifier.width(songColumnWidth)) {
                                columnTracks.forEachIndexed { rowIdx, track ->
                                    val globalIdx = colIdx * 4 + rowIdx + 1
                                    val isCurrent = state.activeTrack?.id == track.id
                                    val isLiked = track.id in state.likedTrackIds
                                    ExploreViralHitRow(
                                        track = track,
                                        isCurrent = isCurrent,
                                        isPlaying = isCurrent && state.isPlaying,
                                        isLiked = isLiked,
                                        onClick = { onPlayTrack(track, viralHitsTracks) },
                                        onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                        onLongClick = { bounds -> onTrackLongClick(track, globalIdx, bounds) },
                                    )
                                    if (rowIdx < columnTracks.lastIndex) {
                                        HorizontalDivider(
                                            color = LocalLunariaColors.current.cardStroke,
                                            thickness = 0.5.dp,
                                            modifier = Modifier.padding(start = 62.dp),
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. New Releases & Charts / Trending shelves
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
                Column(Modifier.padding(top = 4.dp, bottom = 14.dp)) {
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
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ExploreEditorialHeroCard(
    eyebrow: String,
    title: String,
    subtitle: String,
    artworkUrl: String?,
    cardWidth: Dp,
    onClick: () -> Unit,
    onLongClick: (Rect?) -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val heroShape = tokens.cardShape
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(HEADER_ART_PX) }
    val artRequest = rememberArtworkRequest(sizedUrl, HEADER_ART_PX, HEADER_ART_PX)
    val captionScrim = remember {
        Brush.verticalGradient(
            0.0f to Color.Transparent,
            0.52f to Color.Transparent,
            0.82f to Color.Black.copy(alpha = 0.56f),
            1.0f to Color.Black.copy(alpha = 0.80f),
        )
    }

    Column(
        modifier = Modifier
            .width(cardWidth)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(cardBounds)
                },
            ),
    ) {
        Text(
            text = eyebrow.uppercase(),
            style = MaterialTheme.typography.labelSmall.copy(
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.6.sp,
            ),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(2.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.titleLarge.copy(
                fontSize = 20.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = (-0.3).sp,
            ),
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodyLarge.copy(
                fontSize = 16.sp,
                fontWeight = FontWeight.Normal,
            ),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(9.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1.42f)
                .clip(heroShape)
                .background(tokens.eclipseCard)
                .border(0.75.dp, tokens.cardStroke, heroShape),
        ) {
            if (artRequest != null) {
                AsyncImage(
                    model = artRequest,
                    contentDescription = title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            }
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(captionScrim),
            )
            Text(
                text = subtitle.ifBlank { title },
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontSize = 13.5.sp,
                    fontWeight = FontWeight.Medium,
                    lineHeight = 18.sp,
                ),
                color = Color.White.copy(alpha = 0.92f),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .align(Alignment.BottomStart)
                    .padding(horizontal = 14.dp, vertical = 12.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ExploreViralHitRow(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    isLiked: Boolean,
    onClick: () -> Unit,
    onOptionsClick: (Rect?) -> Unit,
    onLongClick: (Rect?) -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    var rowBounds by remember { mutableStateOf<Rect?>(null) }
    var optionsBounds by remember { mutableStateOf<Rect?>(null) }
    val thumbShape = remember { RoundedCornerShape(8.dp) }
    val thumb = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
    val thumbReq = rememberArtworkRequest(thumb, ROW_ART_PX, ROW_ART_PX)
    val isDownloaded = track.id in LocalDownloadedTrackIds.current

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(Shape10)
            .onGloballyPositioned { rowBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(rowBounds)
                },
            )
            .padding(vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (isLiked) {
            Icon(
                imageVector = SonataIcons.StarFilled,
                contentDescription = "Liked",
                tint = SonataAccent,
                modifier = Modifier
                    .padding(end = 5.dp)
                    .size(10.dp),
            )
        }
        val tokens = LocalLunariaColors.current
        Box(
            modifier = Modifier
                .size(50.dp)
                .clip(thumbShape)
                .background(tokens.eclipseCard)
                .border(0.75.dp, tokens.cardStroke, thumbShape),
            contentAlignment = Alignment.Center,
        ) {
            if (thumbReq != null) {
                AsyncImage(
                    model = thumbReq,
                    contentDescription = track.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = tokens.onSurfaceSecondary,
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
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                Text(
                    text = track.title,
                    style = MaterialTheme.typography.titleMedium.copy(
                        fontSize = 15.sp,
                        fontWeight = if (isCurrent) FontWeight.SemiBold else FontWeight.Medium,
                    ),
                    color = if (isCurrent) SonataAccent else MaterialTheme.colorScheme.onBackground,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (track.isExplicit) {
                    Box(
                        modifier = Modifier
                            .size(14.dp)
                            .clip(RoundedCornerShape(3.dp))
                            .background(MaterialTheme.colorScheme.onBackground.copy(alpha = 0.14f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = "E",
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 9.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                        )
                    }
                }
            }
            Spacer(Modifier.height(1.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                if (isDownloaded) {
                    Icon(
                        imageVector = SonataIcons.Download,
                        contentDescription = "Downloaded",
                        tint = SonataAccent,
                        modifier = Modifier.size(12.dp),
                    )
                }
                Text(
                    text = track.artist,
                    style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp),
                    color = tokens.onSurfaceSecondary,
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
                .onGloballyPositioned { optionsBounds = it.boundsInRoot() }
                .clickable { onOptionsClick(optionsBounds) },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = SonataIcons.MoreHoriz,
                contentDescription = "Track options",
                tint = tokens.onSurfaceSecondary,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

