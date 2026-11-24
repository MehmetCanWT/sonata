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
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.data.CanvasMedia
import dev.sonata.android.data.CanvasResolver
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.data.LocalAudioTrack
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.SonataThumbnailInterceptor
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.screens.ServerDiagnosticState
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile
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
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
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


enum class LibrarySourceTab {
    ALL,
    YOUTUBE,
    LOCAL,
    JELLYFIN,
    SUBSONIC,
    SMB,
    WEB_DAV,
}

internal fun buildVisibleLibrarySourceTabs(
    personalOnlyMode: Boolean,
    localMusicEnabled: Boolean,
    servers: List<AndroidServerProfile>,
): List<LibrarySourceTab> = buildList {
    add(LibrarySourceTab.ALL)
    if (!personalOnlyMode) {
        add(LibrarySourceTab.YOUTUBE)
    }
    if (localMusicEnabled) {
        add(LibrarySourceTab.LOCAL)
    }
    val activeKinds = servers.filter { it.enabled }.map { it.kind }.toSet()
    if (AndroidServerKind.JELLYFIN in activeKinds) add(LibrarySourceTab.JELLYFIN)
    if (AndroidServerKind.SUBSONIC in activeKinds) add(LibrarySourceTab.SUBSONIC)
    if (AndroidServerKind.SMB in activeKinds) add(LibrarySourceTab.SMB)
    if (AndroidServerKind.WEB_DAV in activeKinds) add(LibrarySourceTab.WEB_DAV)
}

internal fun filterTracksForSourceTab(
    tab: LibrarySourceTab,
    tracks: List<AndroidTrack>,
    personalOnlyMode: Boolean,
): List<AndroidTrack> = when (tab) {
    LibrarySourceTab.ALL -> if (personalOnlyMode) tracks.filter { isPersonalTrackId(it.id) } else tracks
    LibrarySourceTab.YOUTUBE -> tracks.filterNot { isPersonalTrackId(it.id) }
    LibrarySourceTab.LOCAL -> tracks.filter { it.id.startsWith("local:") }
    LibrarySourceTab.JELLYFIN -> tracks.filter { it.id.startsWith("jellyfin:") }
    LibrarySourceTab.SUBSONIC -> tracks.filter { it.id.startsWith("subsonic:") }
    LibrarySourceTab.SMB -> tracks.filter { it.id.startsWith("smb:") }
    LibrarySourceTab.WEB_DAV -> tracks.filter { it.id.startsWith("webdav:") }
}

internal fun filterPlaylistsForSourceTab(
    tab: LibrarySourceTab,
    playlists: List<AndroidPlaylist>,
    personalOnlyMode: Boolean,
): List<AndroidPlaylist> = when (tab) {
    LibrarySourceTab.ALL -> if (personalOnlyMode) {
        playlists.filter {
            it.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID ||
                it.id.startsWith("local:") ||
                it.id.startsWith("personal:") ||
                it.id.startsWith("jellyfin:") ||
                it.id.startsWith("subsonic:") ||
                it.id.startsWith("smb:") ||
                it.id.startsWith("webdav:")
        }
    } else {
        playlists
    }
    LibrarySourceTab.YOUTUBE -> playlists.filterNot {
        it.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID ||
            it.id.startsWith("local:") ||
            it.id.startsWith("personal:") ||
            it.id.startsWith("jellyfin:") ||
            it.id.startsWith("subsonic:") ||
            it.id.startsWith("smb:") ||
            it.id.startsWith("webdav:")
    }
    LibrarySourceTab.LOCAL -> playlists.filter {
        it.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID ||
            it.id.startsWith("local:") ||
            it.id.startsWith("personal:")
    }
    LibrarySourceTab.JELLYFIN -> playlists.filter { it.id.startsWith("jellyfin:") }
    LibrarySourceTab.SUBSONIC -> playlists.filter { it.id.startsWith("subsonic:") }
    LibrarySourceTab.SMB -> playlists.filter { it.id.startsWith("smb:") }
    LibrarySourceTab.WEB_DAV -> playlists.filter { it.id.startsWith("webdav:") }
}

private fun serverTagForKind(kind: AndroidServerKind): String = when (kind) {
    AndroidServerKind.JELLYFIN -> "JFN"
    AndroidServerKind.SUBSONIC -> "SUB"
    AndroidServerKind.SMB -> "SMB"
    AndroidServerKind.WEB_DAV -> "DAV"
}

private fun serverKindForTab(tab: LibrarySourceTab): AndroidServerKind? = when (tab) {
    LibrarySourceTab.JELLYFIN -> AndroidServerKind.JELLYFIN
    LibrarySourceTab.SUBSONIC -> AndroidServerKind.SUBSONIC
    LibrarySourceTab.SMB -> AndroidServerKind.SMB
    LibrarySourceTab.WEB_DAV -> AndroidServerKind.WEB_DAV
    else -> null
}

@Composable
internal fun LibraryTabContent(
    state: SonataUiState,
    downloadsCount: Int,
    latestDownloadCoverUrl: String?,
    listState: LazyListState,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    appLanguage: AppLanguage = AppLanguage.SYSTEM,
    localMusicEnabled: Boolean = true,
    onSignIn: () -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onPlaylistLongClick: (AndroidPlaylist, Rect?) -> Unit = { _, _ -> },
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onOpenDownloads: () -> Unit,
    onOpenServers: (String?) -> Unit = {},
    onOpenArtist: (String, String) -> Unit,
    onRequestLocalPermission: () -> Unit = {},
    onRescanLocalMusic: () -> Unit = {},
    onCreatePersonalPlaylist: (String) -> Unit = {},
    onLoadServerSource: (String) -> Unit = {},
    onOpenAlbumOrPlaylist: (id: String, title: String, coverUrl: String?, subtitle: String?, kindLabel: String?) -> Unit = { _, _, _, _, _ -> },
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
) {
    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val accent = sonataThemeAccent()
    val srcStrings = remember(appLanguage) { sourcesStrings(appLanguage) }
    val visibleTabs = remember(state.personalOnlyMode, localMusicEnabled, state.servers) {
        buildVisibleLibrarySourceTabs(
            personalOnlyMode = state.personalOnlyMode,
            localMusicEnabled = localMusicEnabled,
            servers = state.servers,
        )
    }
    var selectedTab by remember(visibleTabs) { mutableStateOf(LibrarySourceTab.ALL) }
    LaunchedEffect(visibleTabs) {
        if (selectedTab !in visibleTabs) {
            selectedTab = LibrarySourceTab.ALL
        }
    }
    var localSubTabIndex by remember { mutableIntStateOf(0) }
    var showCreatePersonalDialog by remember { mutableStateOf(false) }
    var newPersonalPlaylistName by remember { mutableStateOf("") }
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val playlistColumns = ((screenWidthDp - 32) / 160).coerceIn(2, 6)
    val visibleTracksForTab = remember(selectedTab, state.tracks, state.personalOnlyMode) {
        filterTracksForSourceTab(selectedTab, state.tracks, state.personalOnlyMode)
    }
    val replayArt = remember(visibleTracksForTab, state.playlists) {
        visibleTracksForTab.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
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
    val combinedPlaylists = remember(
        selectedTab,
        downloadsPlaylist,
        state.playlists,
        state.pinnedCollections,
        pinnedIdSet,
        state.personalOnlyMode,
    ) {
        val baseById = LinkedHashMap<String, AndroidPlaylist>().apply {
            if (!state.personalOnlyMode || selectedTab != LibrarySourceTab.YOUTUBE) {
                put(normalizeCollectionId(downloadsPlaylist.id), downloadsPlaylist)
            }
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
        filterPlaylistsForSourceTab(selectedTab, ordered, state.personalOnlyMode)
    }
    val playlistRows = remember(combinedPlaylists, playlistColumns) {
        combinedPlaylists.chunked(playlistColumns)
    }
    val libraryArtists = remember(state.recentArtistCards, state.lastPlayedTracks, state.shelves, strings, state.personalOnlyMode) {
        val merged = linkedMapOf<String, AndroidHomeCard>()
        if (!state.personalOnlyMode) {
            state.recentArtistCards.forEach { card ->
                val key = card.title.trim().lowercase()
                if (key.isNotBlank()) merged.putIfAbsent(key, card)
            }
        }
        state.lastPlayedTracks
            .filter { !state.personalOnlyMode || isPersonalTrackId(it.id) }
            .forEach { track ->
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
        if (!state.personalOnlyMode) {
            state.shelves.asSequence()
                .flatMap { it.cards.asSequence() }
                .filter { it.kind == AndroidHomeCardKind.ARTIST }
                .forEach { card ->
                    val key = card.title.trim().lowercase()
                    if (key.isNotBlank()) merged.putIfAbsent(key, card)
                }
        }
        merged.values.take(16)
    }

    val activeServerKind = serverKindForTab(selectedTab)
    val activeTabServers = remember(state.servers, activeServerKind) {
        if (activeServerKind == null) {
            emptyList()
        } else {
            state.servers.filter { it.enabled && it.kind == activeServerKind }
        }
    }
    LaunchedEffect(selectedTab, activeTabServers) {
        activeTabServers.forEach { srv ->
            onLoadServerSource(srv.id)
        }
    }

    if (showCreatePersonalDialog) {
        val tokens = LocalLunariaColors.current
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { showCreatePersonalDialog = false },
            containerColor = tokens.eclipseSurface,
            title = {
                Text(
                    text = srcStrings.addPersonalPlaylist,
                    color = tokens.onSurfacePrimary,
                )
            },
            text = {
                androidx.compose.material3.OutlinedTextField(
                    value = newPersonalPlaylistName,
                    onValueChange = { newPersonalPlaylistName = it },
                    placeholder = { Text(strings.playlistName) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                androidx.compose.material3.TextButton(
                    onClick = {
                        val trimmed = newPersonalPlaylistName.trim()
                        if (trimmed.isNotEmpty()) {
                            onCreatePersonalPlaylist(trimmed)
                            newPersonalPlaylistName = ""
                            showCreatePersonalDialog = false
                        }
                    },
                ) {
                    Text(strings.create, color = sonataThemeAccent())
                }
            },
            dismissButton = {
                androidx.compose.material3.TextButton(onClick = { showCreatePersonalDialog = false }) {
                    Text(strings.cancel, color = tokens.onSurfaceSecondary)
                }
            },
        )
    }

    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "library_heading", contentType = "tab_heading") {
            val tokens = LocalLunariaColors.current
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(
                    text = strings.libraryHeading,
                    style = MaterialTheme.typography.displayLarge.copy(
                        fontSize = 34.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.weight(1f),
                )
                if (state.personalOnlyMode) {
                    Box(
                        modifier = Modifier
                            .clip(CapsulePillShape)
                            .background(tokens.petalLavender.copy(alpha = 0.16f))
                            .border(0.75.dp, tokens.petalLavender.copy(alpha = 0.45f), CapsulePillShape)
                            .padding(horizontal = 10.dp, vertical = 5.dp),
                    ) {
                        Text(
                            text = srcStrings.personalOnlyModeBadge,
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                            color = tokens.petalLavender,
                        )
                    }
                }
            }
        }

        // Dynamic Library Source Tabs
        item(key = "library_source_tabs", contentType = "source_tabs") {
            val tokens = LocalLunariaColors.current
            val accent = sonataThemeAccent()
            LazyRow(
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
            ) {
                items(visibleTabs, key = { "src_tab_${it.name}" }) { tab ->
                    val label = when (tab) {
                        LibrarySourceTab.ALL -> srcStrings.tabAll
                        LibrarySourceTab.YOUTUBE -> srcStrings.tabYouTube
                        LibrarySourceTab.LOCAL -> srcStrings.tabLocal
                        LibrarySourceTab.JELLYFIN -> srcStrings.tabJellyfin
                        LibrarySourceTab.SUBSONIC -> srcStrings.tabSubsonic
                        LibrarySourceTab.SMB -> srcStrings.tabSmb
                        LibrarySourceTab.WEB_DAV -> srcStrings.tabWebDav
                    }
                    val selected = selectedTab == tab
                    Box(
                        modifier = Modifier
                            .clip(CapsulePillShape)
                            .background(if (selected) accent else tokens.eclipseSurface.copy(alpha = 0.78f))
                            .border(
                                0.75.dp,
                                if (selected) accent else tokens.cardStroke,
                                CapsulePillShape,
                            )
                            .clickable { selectedTab = tab }
                            .padding(horizontal = 14.dp, vertical = 7.dp),
                    ) {
                        Text(
                            text = label,
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = if (selected) {
                                if (tokens.isBasic && tokens.isDark) Color.Black else Color.White
                            } else {
                                tokens.onSurfacePrimary
                            },
                        )
                    }
                }
            }
        }

        if (selectedTab == LibrarySourceTab.ALL || selectedTab == LibrarySourceTab.YOUTUBE) {
            // Horizontal ReplayCreditCard carousel
            item(key = "library_replay_carousel", contentType = "replay_carousel") {
                LazyRow(
                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    item(key = "replay_card_tracks", contentType = "replay_card") {
                        ReplayCreditCard(
                            label = if (state.personalOnlyMode && selectedTab == LibrarySourceTab.ALL) {
                                srcStrings.tabLocal
                            } else {
                                strings.quickPicks
                            },
                            value = strings.tracksCount(visibleTracksForTab.size),
                            detail = state.activeTrack?.title ?: strings.playSongsToBuildHistory,
                            artworkUrl = replayArt.artworkAt(CARD_ART_PX),
                            holder = holderName,
                            cardIndex = 0,
                            onClick = {
                                visibleTracksForTab.firstOrNull()?.let { onPlayTrack(it, visibleTracksForTab) }
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

            // Downloads & Self-Hosted Servers rows
            if (selectedTab == LibrarySourceTab.ALL) {
                item(key = "library_device_rows", contentType = "device_rows") {
                    val enabledServers = remember(state.servers) { state.servers.filter { it.enabled } }
                    Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                        LibraryNavRow(
                            icon = SonataIcons.Download,
                            label = strings.downloads,
                            subtitle = if (downloadsCount > 0) strings.tracksCount(downloadsCount) else strings.offlineLabel,
                            onClick = onOpenDownloads,
                        )
                        LibraryNavRow(
                            icon = SonataIcons.Folder,
                            label = strings.servers.selfHostedServers,
                            subtitle = if (state.servers.isNotEmpty()) {
                                strings.servers.enabledCountSummary(enabledServers.size, state.servers.size)
                            } else {
                                strings.servers.manageServersSummary
                            },
                            onClick = { onOpenServers(null) },
                        )
                    }
                    if (enabledServers.isNotEmpty()) {
                        val tokens = LocalLunariaColors.current
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 4.dp),
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            items(enabledServers, key = { "lib_srv_${it.id}" }) { srv ->
                                val tag = serverTagForKind(srv.kind)
                                Row(
                                    modifier = Modifier
                                        .clip(RoundedCornerShape(14.dp))
                                        .background(tokens.eclipseCard)
                                        .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(14.dp))
                                        .clickable { onOpenServers(srv.id) }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    Text(
                                        text = tag,
                                        style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                                        color = tokens.petalLavender,
                                    )
                                    Text(
                                        text = srv.name,
                                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Medium),
                                        color = tokens.onSurfacePrimary,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        }
                    }
                    Spacer(Modifier.height(14.dp))
                }
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

            if (!state.personalOnlyMode && state.authInitialized && !state.signedIn) {
                item(key = "library_signin_banner", contentType = "sign_in_banner") {
                    SignInBanner(onSignIn = onSignIn)
                }
            }

            item(key = "library_playlists_header", contentType = "section_header") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(end = PAGE_GUTTER),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(modifier = Modifier.weight(1f)) {
                        SectionHeader(title = strings.playlists)
                    }
                    if (selectedTab == LibrarySourceTab.ALL) {
                        androidx.compose.material3.TextButton(
                            onClick = { showCreatePersonalDialog = true },
                        ) {
                            Text(
                                text = "+ ${srcStrings.addPersonalPlaylist}",
                                color = sonataThemeAccent(),
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                            )
                        }
                    }
                }
            }
            items(
                items = playlistRows,
                key = { row -> "pl_row_${selectedTab.name}_${row.joinToString("_") { it.id }}" },
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
        } else if (selectedTab == LibrarySourceTab.LOCAL) {
            val localLib = state.localMusicLibrary
            item(key = "local_header_controls") {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (!localLib.permissionGranted) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(16.dp))
                                .background(tokens.eclipseCard)
                                .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(16.dp))
                                .padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Text(
                                text = srcStrings.localMusicPermissionTitle,
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                                color = tokens.onSurfacePrimary,
                            )
                            Text(
                                text = srcStrings.localMusicPermissionSubtitle,
                                style = MaterialTheme.typography.bodySmall,
                                color = tokens.onSurfaceSecondary,
                            )
                            Button(
                                onClick = onRequestLocalPermission,
                                colors = ButtonDefaults.buttonColors(containerColor = sonataThemeAccent()),
                            ) {
                                Text(srcStrings.grantPermission)
                            }
                        }
                    } else {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(
                                text = srcStrings.localTracksCount(localLib.tracks.size),
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                                color = tokens.onSurfacePrimary,
                            )
                            androidx.compose.material3.TextButton(onClick = onRescanLocalMusic) {
                                Text(srcStrings.rescanLocalMusic, color = sonataThemeAccent())
                            }
                        }
                        PillSegmentedControl(
                            options = listOf(
                                srcStrings.localTracksTab,
                                srcStrings.localAlbumsTab,
                                srcStrings.localFoldersTab,
                            ),
                            selectedIndex = localSubTabIndex,
                            onSelect = { localSubTabIndex = it },
                        )
                    }
                }
            }

            if (localLib.permissionGranted) {
                if (state.localMusicLoading) {
                    item(key = "local_loading") {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 24.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            CircularProgressIndicator(color = sonataThemeAccent(), strokeWidth = 2.5.dp)
                        }
                    }
                } else if (localLib.tracks.isEmpty()) {
                    item(key = "local_empty") {
                        Text(
                            text = srcStrings.emptyLocalMusic,
                            style = MaterialTheme.typography.bodyMedium,
                            color = tokens.onSurfaceSecondary,
                            modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 20.dp),
                        )
                    }
                } else {
                    when (localSubTabIndex) {
                        0 -> {
                            val localAndroidTracks = localLib.tracks.map { it.toAndroidTrack() }
                            items(
                                items = localAndroidTracks,
                                key = { "loc_trk_${it.id}" },
                            ) { track ->
                                val activeId = state.activeTrack?.id
                                val isCurrent = activeId != null && track.id == activeId
                                Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                                    CompactTrackRow(
                                        track = track,
                                        isCurrent = isCurrent,
                                        isPlaying = isCurrent && state.isPlaying,
                                        onClick = { onPlayTrack(track, localAndroidTracks) },
                                        onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                        onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                    )
                                }
                            }
                        }
                        1 -> {
                            items(
                                items = localLib.albums,
                                key = { "loc_alb_${it.title}_${it.artist}" },
                            ) { album ->
                                val albumQueue = album.tracks.map { it.toAndroidTrack() }
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clickable {
                                            albumQueue.firstOrNull()?.let { first ->
                                                onPlayTrack(first, albumQueue)
                                            }
                                        }
                                        .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(48.dp)
                                            .clip(Shape8)
                                            .background(tokens.eclipseCard),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        val req = rememberArtworkRequest(album.albumArtUri, ROW_ART_PX, ROW_ART_PX)
                                        if (req != null) {
                                            AsyncImage(
                                                model = req,
                                                contentDescription = null,
                                                contentScale = ContentScale.Crop,
                                                modifier = Modifier.fillMaxSize(),
                                            )
                                        } else {
                                            Icon(
                                                imageVector = SonataIcons.MusicNote,
                                                contentDescription = null,
                                                tint = tokens.onSurfaceSecondary,
                                            )
                                        }
                                    }
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            text = album.title,
                                            style = MaterialTheme.typography.titleMedium,
                                            color = tokens.onSurfacePrimary,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        Text(
                                            text = "${album.artist} · ${srcStrings.localTracksCount(album.tracks.size)}",
                                            style = MaterialTheme.typography.bodySmall,
                                            color = tokens.onSurfaceSecondary,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                                }
                            }
                        }
                        else -> {
                            items(
                                items = localLib.folders,
                                key = { "loc_fld_${it.relativePath}" },
                            ) { folder ->
                                val folderQueue = folder.tracks.map { it.toAndroidTrack() }
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clickable {
                                            folderQueue.firstOrNull()?.let { first ->
                                                onPlayTrack(first, folderQueue)
                                            }
                                        }
                                        .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Icon(
                                        imageVector = SonataIcons.Folder,
                                        contentDescription = null,
                                        tint = tokens.petalLavender,
                                        modifier = Modifier.size(22.dp),
                                    )
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            text = folder.folderName,
                                            style = MaterialTheme.typography.titleMedium,
                                            color = tokens.onSurfacePrimary,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        Text(
                                            text = "${folder.relativePath} · ${srcStrings.localTracksCount(folder.tracks.size)}",
                                            style = MaterialTheme.typography.bodySmall,
                                            color = tokens.onSurfaceSecondary,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }

            item(key = "local_personal_playlists_hdr") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(end = PAGE_GUTTER, top = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(modifier = Modifier.weight(1f)) {
                        SectionHeader(title = srcStrings.personalPlaylistsTitle)
                    }
                    androidx.compose.material3.TextButton(
                        onClick = { showCreatePersonalDialog = true },
                    ) {
                        Text("+ ${srcStrings.addPersonalPlaylist}", color = sonataThemeAccent())
                    }
                }
            }
            items(
                items = playlistRows,
                key = { row -> "loc_pl_row_${row.joinToString("_") { it.id }}" },
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
        } else {
            // Specific Self-Hosted Server protocol tab (Jellyfin / Subsonic / SMB / WebDAV)
            items(activeTabServers, key = { "tab_srv_hdr_${it.id}" }) { srv ->
                val diag: ServerDiagnosticState? = state.serverDiagnostics[srv.id]
                val snap = state.serverSourceSnapshots[srv.id]
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 8.dp)
                        .clip(RoundedCornerShape(16.dp))
                        .background(tokens.eclipseCard)
                        .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(16.dp))
                        .clickable { onOpenServers(srv.id) }
                        .padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = serverTagForKind(srv.kind),
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                            color = tokens.petalLavender,
                        )
                        Text(
                            text = srv.name,
                            style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                            color = tokens.onSurfacePrimary,
                            modifier = Modifier.weight(1f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        val report = diag?.healthReport
                        if (report != null) {
                            val badgeText = if (report.alive) {
                                val ms = report.latencyMs?.toLong()
                                if (ms != null) "${srcStrings.statusAlive} · ${srcStrings.latencyMs(ms)}" else srcStrings.statusAlive
                            } else {
                                srcStrings.statusOffline
                            }
                            Text(
                                text = badgeText,
                                style = MaterialTheme.typography.labelSmall,
                                color = if (report.alive) Color(0xFF34C759) else Color(0xFFFF453A),
                            )
                        }
                    }
                    val subtitle = srv.share?.takeIf { it.isNotBlank() }?.let { "${srv.baseUrl}/$it" }
                        ?: srv.localMountPath?.takeIf { it.isNotBlank() }
                        ?: srv.baseUrl
                    if (subtitle.isNotBlank()) {
                        Text(
                            text = subtitle,
                            style = MaterialTheme.typography.bodySmall,
                            color = tokens.onSurfaceSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (snap?.loading == true) {
                        CircularProgressIndicator(
                            color = accent,
                            strokeWidth = 2.dp,
                            modifier = Modifier.size(18.dp),
                        )
                    } else if (snap != null) {
                        val tracks = snap.tracks
                        val albums = snap.albums
                        val playlists = snap.playlists
                        Text(
                            text = "${strings.tracksCount(tracks.size)} · ${albums.size} ${strings.servers.albumsTab} · ${playlists.size} ${strings.playlists}",
                            style = MaterialTheme.typography.labelMedium,
                            color = tokens.onSurfaceSecondary,
                        )
                        playlists.take(6).forEach { pl ->
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(RoundedCornerShape(10.dp))
                                    .clickable { onOpenPlaylist(pl) }
                                    .padding(vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(
                                    text = pl.name,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = tokens.onSurfacePrimary,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f),
                                )
                                Icon(
                                    imageVector = SonataIcons.ChevronRight,
                                    contentDescription = null,
                                    tint = tokens.onSurfaceSecondary,
                                    modifier = Modifier.size(16.dp),
                                )
                            }
                        }
                        albums.take(6).forEach { album ->
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(RoundedCornerShape(10.dp))
                                    .clickable {
                                        onOpenAlbumOrPlaylist(
                                            album.id,
                                            album.title,
                                            album.thumbnailUrl,
                                            album.subtitle,
                                            "Album",
                                        )
                                    }
                                    .padding(vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(
                                    text = "${album.title}${album.subtitle.takeIf { it.isNotBlank() }?.let { " · $it" }.orEmpty()}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = tokens.onSurfacePrimary,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f),
                                )
                                Icon(
                                    imageVector = SonataIcons.ChevronRight,
                                    contentDescription = null,
                                    tint = tokens.onSurfaceSecondary,
                                    modifier = Modifier.size(16.dp),
                                )
                            }
                        }
                        tracks.take(10).forEach { track ->
                            val isCurrent = state.activeTrack?.id == track.id
                            CompactTrackRow(
                                track = track,
                                isCurrent = isCurrent,
                                isPlaying = isCurrent && state.isPlaying,
                                onClick = { onPlayTrack(track, tracks) },
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

