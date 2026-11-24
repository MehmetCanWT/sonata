/*
 * Library tab screen with Replay summary cards, offline navigation rows, artists, and playlists.
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
import dev.sonata.android.ui.components.normalizeCollectionId
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
internal fun LibraryTabContent(
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
    onOpenArtist: (String, String) -> Unit,
) {
    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val playlistColumns = ((screenWidthDp - 32) / 160).coerceIn(2, 6)
    val replayArt = remember(state.tracks, state.playlists) {
        state.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            ?: state.playlists.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
    }
    val holderName = state.accountProfile?.name?.takeIf { it.isNotBlank() } ?: "SONATA LISTENER"
    val downloadsPlaylist = remember(latestDownloadCoverUrl, strings) {
        AndroidPlaylist(
            id = SonataViewModel.DOWNLOADS_PLAYLIST_ID,
            name = strings.downloads,
            thumbnailUrl = latestDownloadCoverUrl,
        )
    }
    val pinnedIdSet = remember(state.pinnedCollections) {
        state.pinnedCollections.mapTo(HashSet()) { normalizeCollectionId(it.id) }
    }
    val combinedPlaylists = remember(downloadsPlaylist, state.playlists, state.pinnedCollections, pinnedIdSet) {
        val baseById = LinkedHashMap<String, AndroidPlaylist>().apply {
            put(normalizeCollectionId(downloadsPlaylist.id), downloadsPlaylist)
            state.playlists.forEach { pl ->
                val norm = normalizeCollectionId(pl.id)
                if (norm.isNotBlank() && norm != normalizeCollectionId(SonataViewModel.DOWNLOADS_PLAYLIST_ID)) {
                    putIfAbsent(norm, pl)
                }
            }
        }
        val ordered = ArrayList<AndroidPlaylist>(state.pinnedCollections.size + baseById.size)
        val addedKeys = HashSet<String>()
        state.pinnedCollections.forEach { pinned ->
            val norm = normalizeCollectionId(pinned.id)
            if (norm.isNotBlank() && addedKeys.add(norm)) {
                val base = baseById[norm]
                val resolved = if (base != null) {
                    if (base.thumbnailUrl.isNullOrBlank() && !pinned.coverUrl.isNullOrBlank()) {
                        base.copy(thumbnailUrl = pinned.coverUrl)
                    } else {
                        base
                    }
                } else {
                    AndroidPlaylist(
                        id = pinned.id,
                        name = pinned.title,
                        thumbnailUrl = pinned.coverUrl,
                        isOwned = false,
                    )
                }
                ordered.add(resolved)
            }
        }
        baseById.forEach { (norm, pl) ->
            if (norm !in pinnedIdSet && addedKeys.add(norm)) {
                ordered.add(pl)
            }
        }
        ordered
    }
    val playlistRows = remember(combinedPlaylists, playlistColumns) {
        combinedPlaylists.chunked(playlistColumns)
    }
    val libraryArtists = remember(state.recentArtistCards, state.lastPlayedTracks, state.shelves, strings) {
        val merged = linkedMapOf<String, AndroidHomeCard>()
        state.recentArtistCards.forEach { card ->
            val key = card.title.trim().lowercase()
            if (key.isNotBlank()) merged.putIfAbsent(key, card)
        }
        state.lastPlayedTracks.forEach { track ->
            val cleanArtist = track.artist
                .split(Regex("\\s*,\\s*|\\s+&\\s+|(?i)\\s+(?:feat\\.?|ft\\.?|featuring|x|×)\\s+"), limit = 2)
                .firstOrNull()
                ?.trim()
                .orEmpty()
            val key = cleanArtist.lowercase()
            if (key.isNotBlank() && !merged.containsKey(key)) {
                merged[key] = AndroidHomeCard(
                    kind = AndroidHomeCardKind.ARTIST,
                    id = "",
                    title = cleanArtist,
                    subtitle = strings.recentlyPlayedFallback,
                    thumbnailUrl = track.thumbnailUrl,
                    track = null,
                )
            }
        }
        state.shelves.asSequence()
            .flatMap { it.cards.asSequence() }
            .filter { it.kind == AndroidHomeCardKind.ARTIST }
            .forEach { card ->
                val key = card.title.trim().lowercase()
                if (key.isNotBlank()) merged.putIfAbsent(key, card)
            }
        merged.values.take(16)
    }

    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "library_heading", contentType = "tab_heading") {
            Text(
                text = strings.libraryHeading,
                style = MaterialTheme.typography.displayLarge.copy(
                    fontSize = 34.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = MaterialTheme.colorScheme.onBackground,
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
                        label = strings.quickPicks,
                        value = strings.tracksCount(state.tracks.size),
                        detail = state.activeTrack?.title ?: strings.playSongsToBuildHistory,
                        artworkUrl = replayArt.artworkAt(CARD_ART_PX),
                        holder = holderName,
                        cardIndex = 0,
                        onClick = {
                            state.tracks.firstOrNull()?.let { onPlayTrack(it, state.tracks) }
                        },
                        modifier = Modifier.width(300.dp),
                    )
                }
                item(key = "replay_card_playlists", contentType = "replay_card") {
                    ReplayCreditCard(
                        label = strings.offlineLabel,
                        value = strings.savedCount(downloadsCount),
                        detail = if (downloadsCount > 0) strings.downloaded else strings.noDownloadsHint,
                        artworkUrl = latestDownloadCoverUrl?.artworkAt(CARD_ART_PX)
                            ?: replayArt.artworkAt(CARD_ART_PX),
                        holder = holderName,
                        cardIndex = 1,
                        onClick = onOpenDownloads,
                        modifier = Modifier.width(300.dp),
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
        }

        // Downloads Playlist row (never navigates to Search!)
        item(key = "library_device_rows", contentType = "device_rows") {
            Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                LibraryNavRow(
                    icon = SonataIcons.Download,
                    label = strings.downloads,
                    subtitle = if (downloadsCount > 0) strings.tracksCount(downloadsCount) else strings.offlineLabel,
                    onClick = onOpenDownloads,
                )
            }
            Spacer(Modifier.height(14.dp))
        }

        if (libraryArtists.isNotEmpty()) {
            item(key = "library_artists_shelf", contentType = "library_artists") {
                Column(Modifier.padding(bottom = 18.dp)) {
                    SectionHeader(title = strings.artists)
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        itemsIndexed(
                            items = libraryArtists,
                            key = { idx, card ->
                                "lib_artist_${card.id.ifBlank { card.title.trim().lowercase() }}_$idx"
                            },
                            contentType = { _, _ -> "artist_circle" },
                        ) { _, artist ->
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
            SectionHeader(title = strings.playlists)
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
                        isPinned = normalizeCollectionId(playlist.id) in pinnedIdSet,
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
internal fun LibraryNavRow(
    icon: ImageVector,
    label: String,
    @Suppress("UNUSED_PARAMETER") subtitle: String,
    onClick: () -> Unit,
) {
    val tokens = LocalLunariaColors.current
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
            tint = tokens.petalLavender,
            modifier = Modifier.size(22.dp),
        )
        Spacer(Modifier.width(16.dp))
        Text(
            text = label,
            style = MaterialTheme.typography.titleMedium.copy(fontSize = 17.sp, fontWeight = FontWeight.Medium),
            color = tokens.onSurfacePrimary,
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

internal val LikedPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF7B52FF), Color(0xFFB79CED)))
internal val DownloadsPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF1D1A2B), Color(0xFF8F96D9)))
internal val BasicLikedPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF2C2C2E), Color(0xFF1C1C1E)))
internal val BasicDownloadsPlaylistBrush = Brush.linearGradient(listOf(Color(0xFF2C2C2E), Color(0xFF161618)))

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun PlaylistGridCard(
    playlist: AndroidPlaylist,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    isPinned: Boolean = false,
    onLongClick: ((Rect?) -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    val tokens = LocalLunariaColors.current
    val cardBounds = rememberLayoutBoundsHolder()
    val isDownloadsPlaylist = playlist.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID
    val isLikedPlaylist = !isDownloadsPlaylist && (
        playlist.id.equals("LM", ignoreCase = true) ||
            playlist.id.equals("VLLM", ignoreCase = true) ||
            playlist.name.contains("Liked", ignoreCase = true)
        )
    val cover = remember(playlist.thumbnailUrl) { playlist.thumbnailUrl.artworkAt(CARD_ART_PX) }
    val coverRequest = rememberArtworkRequest(cover, 360, 360)
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
                .clip(tokens.cardShape)
                .then(
                    when {
                        coverRequest != null -> Modifier.background(tokens.eclipseCard)
                        isDownloadsPlaylist -> Modifier.background(
                            if (tokens.isBasic) BasicDownloadsPlaylistBrush else DownloadsPlaylistBrush,
                        )
                        else -> Modifier.background(
                            if (tokens.isBasic) BasicLikedPlaylistBrush else LikedPlaylistBrush,
                        )
                    }
                )
                .border(0.75.dp, tokens.cardStroke, tokens.cardShape),
            contentAlignment = Alignment.Center,
        ) {
            when {
                coverRequest != null -> {
                    AsyncImage(
                        model = coverRequest,
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                isDownloadsPlaylist -> {
                    Icon(
                        imageVector = SonataIcons.Download,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(54.dp),
                    )
                }
                else -> {
                    Icon(
                        imageVector = SonataIcons.ThumbUp,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(64.dp),
                    )
                }
            }
            if (isPinned) {
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(8.dp)
                        .size(26.dp)
                        .clip(CircleShape)
                        .background(tokens.eclipseSurface.copy(alpha = 0.88f))
                        .border(0.75.dp, tokens.cardStroke, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.PinFilled,
                        contentDescription = null,
                        tint = if (tokens.isBasic) tokens.onSurfacePrimary else tokens.petalLavender,
                        modifier = Modifier.size(14.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = playlist.name,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
            color = LocalLunariaColors.current.onSurfacePrimary,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

