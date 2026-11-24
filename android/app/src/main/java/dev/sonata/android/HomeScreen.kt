/*
 * Home tab feed, personalized stations carousel, Mood tiles, and editorial shelves.
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
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.ui.components.AddToPlaylistSheet
import dev.sonata.android.ui.components.PERSONAL_PLAYLIST_ID_PREFIX
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
import uniffi.sonata_android.AndroidServerKind
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
import uniffi.sonata_android.AndroidMoodCategory
import uniffi.sonata_android.AndroidMoodChip
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import java.io.File
import kotlin.math.roundToInt


internal fun isNewReleaseShelfTitle(title: String): Boolean {
    val t = title.trim().lowercase()
    return ("new" in t && ("release" in t || "album" in t || "single" in t)) ||
        t == "releases" ||
        t == "new releases" ||
        "yeni çıkan" in t ||
        ("yeni" in t && ("albüm" in t || "single" in t)) ||
        "новинк" in t ||
        ("нов" in t && ("релиз" in t || "альбом" in t || "сингл" in t))
}

internal fun AndroidHomeCard.dedupeContentKey(): String {
    val normTitle = title.trim().lowercase()
    val normSub = subtitle.trim().lowercase()
    return "$normTitle|$normSub"
}

internal fun mergeNewReleaseShelves(
    shelves: List<AndroidHomeShelf>,
    excludedIds: Set<String> = emptySet(),
    excludedContentKeys: Set<String> = emptySet(),
): List<AndroidHomeShelf> {
    val releaseShelves = shelves.filter { isNewReleaseShelfTitle(it.title) }
    val seenIds = HashSet<String>(excludedIds)
    val seenKeys = HashSet<String>(excludedContentKeys)

    fun filterUniqueCards(cards: List<AndroidHomeCard>): List<AndroidHomeCard> {
        val out = ArrayList<AndroidHomeCard>(cards.size)
        for (card in cards) {
            if (card.kind == AndroidHomeCardKind.MOOD) continue
            val normId = card.id.removePrefix("VL").trim()
            val contentKey = card.dedupeContentKey()
            val idFresh = normId.isBlank() || seenIds.add(normId)
            val keyFresh = contentKey == "|" || seenKeys.add(contentKey)
            if (idFresh && keyFresh) {
                out.add(card)
            }
        }
        return out
    }

    val mergedReleaseShelf = if (releaseShelves.isNotEmpty()) {
        val combinedCards = filterUniqueCards(releaseShelves.flatMap { it.cards })
        val combinedTracks = releaseShelves.flatMap { it.tracks }.distinctBy { it.id }
        if (combinedCards.isNotEmpty() || combinedTracks.isNotEmpty()) {
            AndroidHomeShelf(
                title = "New Releases",
                tracks = combinedTracks,
                cards = combinedCards,
            )
        } else {
            null
        }
    } else {
        null
    }

    val result = ArrayList<AndroidHomeShelf>(shelves.size)
    var insertedMerged = false
    for (shelf in shelves) {
        if (isNewReleaseShelfTitle(shelf.title)) {
            if (!insertedMerged && mergedReleaseShelf != null) {
                result.add(mergedReleaseShelf)
                insertedMerged = true
            }
        } else {
            val uniqueCards = filterUniqueCards(shelf.cards)
            if (uniqueCards.isNotEmpty() || shelf.tracks.isNotEmpty()) {
                result.add(shelf.copy(cards = uniqueCards))
            }
        }
    }
    return result
}

internal fun homeShelfSortOrder(title: String): Int {
    val t = title.lowercase()
    return when {
        ("new" in t && ("album" in t || "single" in t || "release" in t)) ||
            "yeni çıkan" in t ||
            "новинк" in t -> 0
        "album" in t || "single" in t || "release" in t ||
            "albüm" in t || "альбом" in t || "сингл" in t -> 1
        "trend" in t || "chart" in t ||
            "listeler" in t || "popüler" in t || "чарт" in t || "тренд" in t -> 2
        "favorite" in t || "replay" in t ||
            "favori" in t || "tekrar dinle" in t || "слушать снова" in t || "любим" in t -> 3
        else -> 4
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
internal fun HomeTabContent(
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
    onLastPlayedTrackLongClick: (AndroidTrack, Rect?) -> Unit = onTrackLongClick,
    onCardLongClick: (AndroidHomeCard, String?, Boolean, Rect?) -> Unit = { _, _, _, _ -> },
    onPlaylistLongClick: (AndroidPlaylist, Rect?) -> Unit = { _, _ -> },
    onPinnedLongClick: (PinnedCollectionEntry, Rect?) -> Unit = { _, _ -> },
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onOpenPinnedCollection: (PinnedCollectionEntry) -> Unit = {},
    onOpenMood: (String) -> Unit,
    onSelectHomeMoodChip: (String?) -> Unit = {},
    onOpenMoodCategory: (String, String, String) -> Unit = { title, _, _ -> onOpenMood(title) },
    onShowAllMoods: () -> Unit,
    onStartPersonalStation: () -> Unit = {},
    onStartDiscoveryStation: () -> Unit = {},
    onOpenServers: (String?) -> Unit = {},
) {
    val haptics = LocalHapticFeedback.current
    val visibleShelves = remember(state.shelves) {
        state.shelves.filterNot { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" in t || "unutulan" in t || "забыт" in t
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
    val fallbackHomeTracks = state.tracks.takeIf { state.shelves.isEmpty() }.orEmpty()
    val topPicks = remember(state.shelves, state.lastPlayedTracks, state.likedTrackIds, fallbackHomeTracks) {
        val artistAffinity = HashMap<String, Int>()
        state.lastPlayedTracks.take(25).forEachIndexed { idx, track ->
            val key = track.artist.trim().lowercase()
            if (key.isNotBlank()) {
                artistAffinity[key] = (artistAffinity[key] ?: 0) + (25 - idx).coerceAtLeast(2)
            }
        }
        state.shelves.flatMap { it.tracks }
            .filter { it.id in state.likedTrackIds }
            .forEach { track ->
                val key = track.artist.trim().lowercase()
                if (key.isNotBlank()) {
                    artistAffinity[key] = (artistAffinity[key] ?: 0) + 10
                }
            }
        fun cardAffinityScore(artistOrSubtitle: String): Int {
            val norm = artistOrSubtitle.trim().lowercase()
            if (norm.isBlank()) return 0
            return artistAffinity[norm]
                ?: artistAffinity.entries.firstOrNull { (k, _) -> norm.contains(k) || k.contains(norm) }?.value
                ?: 0
        }

        val allCardPairs = state.shelves.flatMap { shelf ->
            shelf.cards.map { it to shelf.tracks }
        }
        val albumOrReleaseCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.ALBUM && !card.thumbnailUrl.isNullOrBlank()
        }.sortedByDescending { (card, _) -> cardAffinityScore(card.subtitle) }
        val playlistCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.PLAYLIST && !card.thumbnailUrl.isNullOrBlank()
        }.sortedByDescending { (card, _) -> cardAffinityScore("${card.title} ${card.subtitle}") }
        val artistCards = allCardPairs.filter { (card, _) ->
            card.kind == AndroidHomeCardKind.ARTIST
        }.sortedByDescending { (card, _) -> cardAffinityScore(card.title) }
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
        }.sortedByDescending { (card, _) -> cardAffinityScore(card.subtitle) }
        val seedTracks = if (state.shelves.isNotEmpty()) {
            state.shelves.flatMap { it.tracks }
        } else {
            fallbackHomeTracks
        }
        val portraitUrls = (
            artistCards.mapNotNull { it.first.thumbnailUrl } +
                state.lastPlayedTracks.mapNotNull { it.thumbnailUrl } +
                seedTracks.mapNotNull { it.thumbnailUrl } +
                albumOrReleaseCards.mapNotNull { it.first.thumbnailUrl } +
                playlistCards.mapNotNull { it.first.thumbnailUrl }
            ).distinct()

        val entries = mutableListOf<TopPickEntry>()

        // Card 1: Single high-res track/album ReleaseArt (avoids 2x2 playlist collage grid on first slot)
        val firstRelease = albumOrReleaseCards.firstOrNull()
            ?: trackAsReleaseCards.firstOrNull { !it.first.thumbnailUrl.isNullOrBlank() }
            ?: playlistCards.firstOrNull()
        val firstPrimaryArtist = firstRelease?.first?.subtitle?.trim()?.lowercase().orEmpty()
        if (firstRelease != null) {
            entries += TopPickEntry.Release(card = firstRelease.first, shelfTracks = firstRelease.second)
        }

        // Card 2: ArtistCollageArt with 2-3 circular portraits peeking beside Card 1
        val artistCard = artistCards.firstOrNull()?.first
        val topArtists = (state.lastPlayedTracks.map { it.artist } + seedTracks.map { it.artist })
            .filter { it.isNotBlank() }
            .distinct()
            .take(2)
        val artistMixCard = artistCard ?: AndroidHomeCard(
            kind = AndroidHomeCardKind.ARTIST,
            id = "artist_mix",
            title = if (topArtists.isNotEmpty()) topArtists.joinToString(" & ") else "",
            subtitle = topArtists.firstOrNull().orEmpty(),
            thumbnailUrl = portraitUrls.firstOrNull(),
            track = null,
        )
        entries += TopPickEntry.ArtistMix(
            card = artistMixCard,
            portraits = (listOfNotNull(artistMixCard.thumbnailUrl) + portraitUrls).distinct().take(3),
        )

        // Card 3: Personal Station
        entries += TopPickEntry.PersonalStation

        // Card 4: Featured Playlist / Mix ReleaseArt (prefer distinct primary artist from Card 1)
        val featuredPlaylist = playlistCards.firstOrNull {
            it.first.id != firstRelease?.first?.id &&
                it.first.subtitle.trim().lowercase() != firstPrimaryArtist
        }
            ?: playlistCards.firstOrNull { it.first.id != firstRelease?.first?.id }
            ?: albumOrReleaseCards.drop(1).firstOrNull {
                it.first.subtitle.trim().lowercase() != firstPrimaryArtist
            }
            ?: albumOrReleaseCards.drop(1).firstOrNull()
            ?: trackAsReleaseCards.drop(1).firstOrNull()
        if (featuredPlaylist != null) {
            entries += TopPickEntry.Release(card = featuredPlaylist.first, shelfTracks = featuredPlaylist.second)
        }

        // Card 5: Discovery Station
        entries += TopPickEntry.DiscoveryStation
        entries
    }
    val remainingShelves = remember(visibleShelves, recentsShelf) {
        val rawRemaining = visibleShelves
            .filter { shelf ->
                shelf !== recentsShelf && shelf.cards.any { it.kind != AndroidHomeCardKind.MOOD }
            }
            .sortedBy { homeShelfSortOrder(it.title) }
        mergeNewReleaseShelves(rawRemaining)
            .filter { it.cards.isNotEmpty() }
            .distinctBy { it.title.trim().lowercase() }
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
    val lastPlayedTracks = remember(state.lastPlayedTracks, recentsTracks, fallbackHomeTracks, state.personalOnlyMode) {
        if (state.personalOnlyMode) {
            state.lastPlayedTracks
                .filter { isPersonalTrackId(it.id) }
                .distinctBy { it.id }
                .take(14)
        } else {
            state.lastPlayedTracks
                .ifEmpty { recentsTracks.ifEmpty { fallbackHomeTracks } }
                .distinctBy { it.id }
                .take(14)
        }
    }
    val showSkeleton = state.loading && state.shelves.isEmpty()
    val tokens = LocalLunariaColors.current
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val availableWidth = (screenWidthDp.dp - PAGE_GUTTER * 2).coerceAtLeast(240.dp)

    // Exact integer column count for Top Picks for You & Find Your Mood so cards never overflow the right edge.
    val topPickCols = when {
        screenWidthDp >= 960 -> 4
        screenWidthDp >= 600 -> 3
        else -> 2
    }
    val topPickSpacing = PAGE_GUTTER
    val topPickCardWidth = (availableWidth - topPickSpacing * (topPickCols - 1)) / topPickCols
    val topPickCardHeight = if (tokens.isMoonflower) {
        (topPickCardWidth * 1.28f).coerceIn(240.dp, 356.dp)
    } else {
        (topPickCardWidth * 1.32f).coerceIn(248.dp, 344.dp)
    }
    val topPicksRowState = rememberLazyListState()
    val topPicksSnap = rememberSnapFlingBehavior(lazyListState = topPicksRowState)

    val moodCols = when {
        screenWidthDp >= 960 -> 5
        screenWidthDp >= 600 -> 4
        else -> 2
    }
    val moodSpacing = PAGE_GUTTER
    val moodTileWidth = (availableWidth - moodSpacing * (moodCols - 1)) / moodCols
    val moodTileHeight = if (screenWidthDp >= 600) 144.dp else 134.dp
    val moodRowState = rememberLazyListState()
    val moodSnap = rememberSnapFlingBehavior(lazyListState = moodRowState)
    val srcStrings = remember(settings.appLanguage) {
        dev.sonata.android.ui.i18n.sourcesStrings(settings.appLanguage)
    }
    val personalServers = remember(state.servers, state.personalOnlyMode) {
        if (state.personalOnlyMode) state.servers.filter { it.enabled } else emptyList()
    }
    val personalLocalTracks = remember(state.localMusicLibrary.tracks, settings.localMusicEnabled, state.personalOnlyMode) {
        if (state.personalOnlyMode && settings.localMusicEnabled) {
            state.localMusicLibrary.tracks.map { it.toAndroidTrack() }
        } else {
            emptyList()
        }
    }
    val visiblePinned = remember(state.pinnedCollections, state.personalOnlyMode) {
        if (state.personalOnlyMode) {
            state.pinnedCollections.filter { entry ->
                entry.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) ||
                    entry.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) ||
                    isSelfHostedId(entry.id)
            }
        } else {
            state.pinnedCollections
        }
    }
    val visibleHomePlaylists = remember(state.playlists, state.personalOnlyMode) {
        if (state.personalOnlyMode) {
            state.playlists.filter { pl ->
                pl.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) ||
                    pl.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) ||
                    isSelfHostedId(pl.id)
            }
        } else {
            state.playlists
        }
    }

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
                        topPicksSkeleton()
                        recentlyPlayedSkeleton()
                        feedSkeleton()
                    }
                }
            } else {
                val isBasicHome = LocalLunariaColors.current.isBasic
                LaunchedEffect(isBasicHome) {
                    if (isBasicHome && listState.firstVisibleItemIndex <= 1) {
                        listState.scrollToItem(0)
                    }
                }
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = contentPadding,
                ) {
                    if (isBasicHome) {
                        item(key = "basic_home_title", contentType = "basic_home_title") {
                            dev.sonata.android.ui.screens.BasicHomeTitle(
                                title = dev.sonata.android.ui.i18n.LocalSonataStrings.current.homeHeading,
                            )
                        }
                    }
                    if (state.authInitialized && !state.signedIn && !state.personalOnlyMode) {
                        item(key = "home_sign_in_banner", contentType = "sign_in_banner") {
                            SignInBanner(
                                onSignIn = onSignIn,
                                modifier = Modifier.padding(top = 56.dp, bottom = 14.dp),
                            )
                        }
                    }
                    if (state.personalOnlyMode) {
                        if (personalServers.isNotEmpty()) {
                            item(key = "home_personal_servers_shelf", contentType = "personal_servers_shelf") {
                                Column(Modifier.padding(bottom = 24.dp)) {
                                    SectionHeader(title = srcStrings.selfHostedServersTitle)
                                    LazyRow(
                                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                                    ) {
                                        items(
                                            items = personalServers,
                                            key = { "home_srv_${it.id}" },
                                        ) { srv ->
                                            val kindLabel = when (srv.kind) {
                                                AndroidServerKind.JELLYFIN -> srcStrings.protocolJellyfin
                                                AndroidServerKind.SUBSONIC -> srcStrings.protocolSubsonic
                                                AndroidServerKind.SMB -> srcStrings.protocolSmb
                                                AndroidServerKind.WEB_DAV -> srcStrings.protocolWebDav
                                            }
                                            Column(
                                                modifier = Modifier
                                                    .width(180.dp)
                                                    .clip(RoundedCornerShape(16.dp))
                                                    .background(tokens.eclipseCard)
                                                    .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(16.dp))
                                                    .clickable { onOpenServers(srv.id) }
                                                    .padding(14.dp),
                                                verticalArrangement = Arrangement.spacedBy(6.dp),
                                            ) {
                                                Text(
                                                    text = kindLabel,
                                                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                                                    color = tokens.petalLavender,
                                                )
                                                Text(
                                                    text = srv.name,
                                                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                                                    color = tokens.onSurfacePrimary,
                                                    maxLines = 1,
                                                    overflow = TextOverflow.Ellipsis,
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if (personalLocalTracks.isNotEmpty()) {
                            item(key = "home_personal_local_shelf", contentType = "personal_local_shelf") {
                                Column(Modifier.padding(bottom = 24.dp)) {
                                    SectionHeader(title = srcStrings.localMusicTitle)
                                    LazyRow(
                                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                                    ) {
                                        items(
                                            items = personalLocalTracks.take(20),
                                            key = { "home_loc_${it.id}" },
                                        ) { track ->
                                            val activeId = state.activeTrack?.id
                                            SonataLastPlayedSquareCard(
                                                title = track.title,
                                                artworkUrl = track.thumbnailUrl,
                                                isCurrent = activeId != null && track.id == activeId,
                                                onClick = { onPlayTrack(track, personalLocalTracks) },
                                                onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                            )
                                        }
                                    }
                                }
                            }
                        } else if (personalServers.isEmpty()) {
                            item(key = "home_personal_empty_sources") {
                                Column(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = PAGE_GUTTER, vertical = 18.dp)
                                        .clip(RoundedCornerShape(16.dp))
                                        .background(tokens.eclipseCard)
                                        .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(16.dp))
                                        .clickable { onOpenServers(null) }
                                        .padding(18.dp),
                                    verticalArrangement = Arrangement.spacedBy(6.dp),
                                ) {
                                    Text(
                                        text = srcStrings.selfHostedServersTitle,
                                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                                        color = tokens.onSurfacePrimary,
                                    )
                                    Text(
                                        text = srcStrings.selfHostedServersDescription,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = tokens.onSurfaceSecondary,
                                    )
                                }
                            }
                        }
                    }

                    if (!state.personalOnlyMode && state.homeMoodChips.isNotEmpty()) {
                        item(key = "home_mood_chips", contentType = "home_mood_chips") {
                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                            LazyRow(
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 4.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(bottom = 14.dp),
                            ) {
                                items(
                                    items = state.homeMoodChips,
                                    key = { chip -> "home_chip_${chip.params}_${chip.label}" },
                                    contentType = { "home_mood_chip" },
                                ) { chip ->
                                    val selected = state.selectedHomeMoodParams == chip.params
                                    val chipBg = if (selected) {
                                        tokens.petalLavender
                                    } else {
                                        tokens.eclipseSurface.copy(alpha = 0.78f)
                                    }
                                    val chipInk = if (selected) {
                                        if (tokens.isBasic && !tokens.isDark) Color.White else if (tokens.isBasic) Color.Black else Color.White
                                    } else {
                                        tokens.onSurfacePrimary
                                    }
                                    Box(
                                        modifier = Modifier
                                            .clip(CapsulePillShape)
                                            .background(chipBg)
                                            .border(
                                                0.75.dp,
                                                if (selected) tokens.petalLavender else tokens.cardStroke,
                                                CapsulePillShape,
                                            )
                                            .clickable { onSelectHomeMoodChip(chip.params) }
                                            .padding(horizontal = 14.dp, vertical = 8.dp),
                                    ) {
                                        Text(
                                            text = dev.sonata.android.ui.i18n.localizeMoodTitle(chip.label, strings),
                                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                            color = chipInk,
                                            maxLines = 1,
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 1: Top Picks for You (hidden in Personal Only Mode)
                    if (!settings.hideTopPicks && !state.personalOnlyMode) {
                        item(key = "top_picks_shelf", contentType = "top_picks_shelf") {
                        Column(Modifier.padding(bottom = 26.dp)) {
                            SectionHeader(title = dev.sonata.android.ui.i18n.LocalSonataStrings.current.topPicksForYou)
                            LazyRow(
                                state = topPicksRowState,
                                flingBehavior = topPicksSnap,
                                modifier = Modifier.fillMaxWidth(),
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                horizontalArrangement = Arrangement.spacedBy(topPickSpacing),
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
                                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                                            val eyebrow = if (pick.card.kind == AndroidHomeCardKind.ALBUM) {
                                                strings.newReleaseEyebrow
                                            } else {
                                                strings.featuredPlaylistEyebrow
                                            }
                                            val cardBounds = rememberLayoutBoundsHolder()
                                            ReleaseArt(
                                                artworkUrl = artUrl,
                                                title = pick.card.title,
                                                subtitle = pick.card.subtitle,
                                                eyebrow = eyebrow,
                                                showBrandLogo = true,
                                                canvasMedia = cardCanvas,
                                                canvasActive = settings.animatedArtwork && !settings.reduceAnimation,
                                                cardWidth = topPickCardWidth,
                                                cardHeight = topPickCardHeight,
                                                modifier = Modifier
                                                    .onGloballyPositioned { cardBounds.coordinates = it }
                                                    .combinedClickable(
                                                        onClick = { onOpenCard(pick.card, pick.shelfTracks) },
                                                        onLongClick = {
                                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                                            onCardLongClick(pick.card, eyebrow, true, cardBounds.boundsInRoot())
                                                        },
                                                    ),
                                            )
                                        }

                                        is TopPickEntry.ArtistMix -> {
                                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                                            val sizedPortraits = remember(pick.portraits) {
                                                pick.portraits.mapNotNull { it.artworkAt(CARD_ART_PX) }
                                            }
                                            val cardBounds = rememberLayoutBoundsHolder()
                                            val mixTitle = pick.card.title.ifBlank { strings.artistMixDefaultTitle }
                                            val mixSubtitle = when {
                                                pick.card.id == "artist_mix" && pick.card.subtitle.isNotBlank() ->
                                                    strings.artistMixFeaturing(pick.card.subtitle)
                                                pick.card.subtitle.isNotBlank() -> pick.card.subtitle
                                                else -> strings.artistMixDefaultSubtitle
                                            }
                                            ArtistCollageArt(
                                                images = sizedPortraits,
                                                title = mixTitle,
                                                subtitle = mixSubtitle,
                                                cardWidth = topPickCardWidth,
                                                cardHeight = topPickCardHeight,
                                                modifier = Modifier
                                                    .onGloballyPositioned { cardBounds.coordinates = it }
                                                    .combinedClickable(
                                                        onClick = { onOpenCard(pick.card, emptyList()) },
                                                        onLongClick = {
                                                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                                            onCardLongClick(pick.card, "MIX", true, cardBounds.boundsInRoot())
                                                        },
                                                    ),
                                            )
                                        }

                                        TopPickEntry.PersonalStation -> {
                                            val stationW = topPickCardWidth
                                            val stationH = topPickCardHeight
                                            val stationCorner = if (tokens.isMoonflower) 26.dp else 14.dp
                                            val stationShape = RoundedCornerShape(stationCorner)
                                            val borderColor = if (tokens.isMoonflower) {
                                                Color.White.copy(alpha = if (tokens.isDark) 0.34f else 0.78f)
                                            } else {
                                                tokens.glassRimTopLeft.copy(alpha = 0.65f)
                                            }
                                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                                            val name = state.accountProfile?.name?.takeIf { it.isNotBlank() }
                                            val stationTitle = if (name != null) strings.userStationSuffix(name) else strings.myStation
                                            Box(
                                                modifier = Modifier
                                                    .width(stationW)
                                                    .height(stationH)
                                                    .then(
                                                        if (tokens.isViolet) {
                                                            Modifier.shadow(
                                                                elevation = 12.dp,
                                                                shape = stationShape,
                                                                clip = false,
                                                                ambientColor = tokens.glassShadowColor,
                                                                spotColor = tokens.glassShadowColor,
                                                            )
                                                        } else {
                                                            Modifier
                                                        },
                                                    )
                                                    .clip(stationShape)
                                                    .border(if (tokens.isMoonflower) 1.25.dp else 1.dp, borderColor, stationShape)
                                                    .clickable(onClick = onStartPersonalStation),
                                            ) {
                                                SonataPersonalStationArt(
                                                    title = stationTitle,
                                                    modifier = Modifier.fillMaxSize(),
                                                )
                                                Box(
                                                    modifier = Modifier
                                                        .fillMaxSize()
                                                        .background(
                                                            dev.sonata.android.ui.smoothScrimBrush(
                                                                from = tokens.heroCardScrimColor.copy(alpha = 0f),
                                                                to = tokens.heroCardScrimColor.copy(alpha = 0.85f),
                                                                startFraction = 0.48f,
                                                            ),
                                                        ),
                                                )
                                                Icon(
                                                    painter = painterResource(R.drawable.ic_sonata_moon),
                                                    contentDescription = null,
                                                    tint = Color.White.copy(alpha = 0.92f),
                                                    modifier = Modifier
                                                        .align(Alignment.TopEnd)
                                                        .padding(top = 14.dp, end = 14.dp)
                                                        .height(15.dp)
                                                        .aspectRatio(1f),
                                                )
                                                Column(
                                                    modifier = Modifier
                                                        .align(Alignment.BottomStart)
                                                        .padding(18.dp),
                                                    verticalArrangement = Arrangement.spacedBy(4.dp),
                                                ) {
                                                    if (tokens.isMoonflower) {
                                                        Box(
                                                            modifier = Modifier
                                                                .clip(RoundedCornerShape(percent = 50))
                                                                .background(Color.White.copy(alpha = 0.22f))
                                                                .padding(horizontal = 9.dp, vertical = 3.dp),
                                                        ) {
                                                            Text(
                                                                text = "PERSONALIZED STATION",
                                                                style = MaterialTheme.typography.labelSmall.copy(
                                                                    fontSize = 10.sp,
                                                                    fontWeight = FontWeight.Bold,
                                                                    letterSpacing = 0.8.sp,
                                                                ),
                                                                color = Color.White,
                                                            )
                                                        }
                                                    } else {
                                                        Text(
                                                            text = "PERSONALIZED STATION",
                                                            style = MaterialTheme.typography.labelSmall.copy(
                                                                fontWeight = FontWeight.Bold,
                                                                letterSpacing = 1.1.sp,
                                                            ),
                                                            color = Color.White.copy(alpha = 0.82f),
                                                        )
                                                    }
                                                    Text(
                                                        text = stationTitle,
                                                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                                                        color = Color.White,
                                                    )
                                                }
                                            }
                                        }

                                        TopPickEntry.DiscoveryStation -> {
                                            val stationW = topPickCardWidth
                                            val stationH = topPickCardHeight
                                            val stationCorner = if (tokens.isMoonflower) 26.dp else 14.dp
                                            val stationShape = RoundedCornerShape(stationCorner)
                                            val borderColor = if (tokens.isMoonflower) {
                                                Color.White.copy(alpha = if (tokens.isDark) 0.34f else 0.78f)
                                            } else {
                                                tokens.glassRimTopLeft.copy(alpha = 0.65f)
                                            }
                                            Box(
                                                modifier = Modifier
                                                    .width(stationW)
                                                    .height(stationH)
                                                    .then(
                                                        if (tokens.isViolet) {
                                                            Modifier.shadow(
                                                                elevation = 12.dp,
                                                                shape = stationShape,
                                                                clip = false,
                                                                ambientColor = tokens.glassShadowColor,
                                                                spotColor = tokens.glassShadowColor,
                                                            )
                                                        } else {
                                                            Modifier
                                                        },
                                                    )
                                                    .clip(stationShape)
                                                    .border(if (tokens.isMoonflower) 1.25.dp else 1.dp, borderColor, stationShape)
                                                    .clickable(onClick = onStartDiscoveryStation),
                                            ) {
                                                SonataDiscoveryStationArt(Modifier.fillMaxSize())
                                                Box(
                                                    modifier = Modifier
                                                        .fillMaxSize()
                                                        .background(
                                                            dev.sonata.android.ui.smoothScrimBrush(
                                                                from = tokens.heroCardScrimColor.copy(alpha = 0f),
                                                                to = tokens.heroCardScrimColor.copy(alpha = 0.85f),
                                                                startFraction = 0.48f,
                                                            ),
                                                        ),
                                                )
                                                Icon(
                                                    painter = painterResource(R.drawable.ic_sonata_moon),
                                                    contentDescription = null,
                                                    tint = Color.White.copy(alpha = 0.92f),
                                                    modifier = Modifier
                                                        .align(Alignment.TopEnd)
                                                        .padding(top = 14.dp, end = 14.dp)
                                                        .height(15.dp)
                                                        .aspectRatio(1f),
                                                )
                                                Column(
                                                    modifier = Modifier
                                                        .align(Alignment.BottomStart)
                                                        .padding(18.dp),
                                                    verticalArrangement = Arrangement.spacedBy(4.dp),
                                                ) {
                                                    if (tokens.isMoonflower) {
                                                        Box(
                                                            modifier = Modifier
                                                                .clip(RoundedCornerShape(percent = 50))
                                                                .background(Color.White.copy(alpha = 0.22f))
                                                                .padding(horizontal = 9.dp, vertical = 3.dp),
                                                        ) {
                                                            Text(
                                                                text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.curatedForYou,
                                                                style = MaterialTheme.typography.labelSmall.copy(
                                                                    fontSize = 10.sp,
                                                                    fontWeight = FontWeight.Bold,
                                                                    letterSpacing = 0.8.sp,
                                                                ),
                                                                color = Color.White,
                                                            )
                                                        }
                                                    } else {
                                                        Text(
                                                            text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.curatedForYou,
                                                            style = MaterialTheme.typography.labelSmall.copy(
                                                                fontWeight = FontWeight.Bold,
                                                                letterSpacing = 1.1.sp,
                                                            ),
                                                            color = Color.White.copy(alpha = 0.84f),
                                                        )
                                                    }
                                                    Text(
                                                        text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.discoveryStation,
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
                    }
                    }

                    // Shelf 2a: Last Played (156.dp square cover + title strip, ZERO glass)
                    if (!settings.hideRecents && lastPlayedTracks.isNotEmpty()) {
                        item(key = "last_played_square_strip", contentType = "last_played_strip") {
                            val activeId = state.activeTrack?.id
                            Column(Modifier.padding(bottom = 24.dp)) {
                                SectionHeader(title = dev.sonata.android.ui.i18n.LocalSonataStrings.current.lastPlayed)
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
                                            onLongClick = { bounds -> onLastPlayedTrackLongClick(track, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 2a-pin: Pinned playlists & collections directly beneath Last Played
                    if (visiblePinned.isNotEmpty()) {
                        item(key = "home_pinned_shelf", contentType = "home_pinned_shelf") {
                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                            val playlistCoverById = remember(state.playlists) {
                                state.playlists.associate { pl ->
                                    dev.sonata.android.ui.components.normalizeCollectionId(pl.id) to pl.thumbnailUrl
                                }
                            }
                            Column(Modifier.padding(bottom = 24.dp)) {
                                SectionHeader(title = strings.pinned)
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = visiblePinned,
                                        key = { entry ->
                                            "home_pin_${dev.sonata.android.ui.components.normalizeCollectionId(entry.id)}"
                                        },
                                        contentType = { "pinned_collection_card" },
                                    ) { entry ->
                                        val norm = dev.sonata.android.ui.components.normalizeCollectionId(entry.id)
                                        val liveCover = playlistCoverById[norm]?.takeIf { it.isNotBlank() }
                                            ?: entry.coverUrl
                                        SonataEditorialCard(
                                            title = entry.title,
                                            subtitle = entry.subtitle.ifBlank { entry.kindLabel },
                                            artworkUrl = liveCover,
                                            isPinned = true,
                                            onClick = { onOpenPinnedCollection(entry) },
                                            onPlayClick = { onOpenPinnedCollection(entry) },
                                            onLongClick = { bounds -> onPinnedLongClick(entry, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 2b: Smooth Classic "Recently Played" 4-rows-per-column pager (hidden in Personal Only Mode)
                    if (!state.personalOnlyMode && !settings.hideRecents && recentsShelf != null && recentsColumns.isNotEmpty()) {
                        item(key = "recents_shelf", contentType = "recents_shelf") {
                            val tokens = LocalLunariaColors.current
                            val activeId = state.activeTrack?.id
                            val colWidth = trackColumnWidth(LocalConfiguration.current.screenWidthDp.dp)
                            Column(Modifier.padding(bottom = 26.dp)) {
                                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                                SectionHeader(
                                    title = if (recentsShelf.title.isBlank()) {
                                        strings.recentlyPlayedFallback
                                    } else {
                                        dev.sonata.android.ui.i18n.localizeShelfTitle(recentsShelf.title, strings)
                                    },
                                )
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    items(
                                        items = recentsColumns,
                                        key = { col -> "recents_col_${col.joinToString("_") { it.id }}" },
                                        contentType = { "recents_col" },
                                    ) { column ->
                                        val colModifier = if (tokens.isViolet) {
                                            val containerShape = RoundedCornerShape(16.dp)
                                            Modifier
                                                .width(colWidth)
                                                .clip(containerShape)
                                                .background(
                                                    tokens.eclipseSurface.copy(alpha = if (tokens.isDark) 0.56f else 0.72f),
                                                    containerShape,
                                                )
                                                .border(0.75.dp, tokens.cardStroke, containerShape)
                                                .padding(horizontal = 10.dp, vertical = 6.dp)
                                        } else {
                                            Modifier.width(colWidth)
                                        }
                                        Column(colModifier) {
                                            column.forEachIndexed { rowIdx, track ->
                                                val current = activeId != null && track.id == activeId
                                                CompactTrackRow(
                                                    track = track,
                                                    isCurrent = current,
                                                    isPlaying = current && state.isPlaying,
                                                    onClick = { onPlayTrack(track, recentsTracks) },
                                                    onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                                                    onLongClick = { bounds -> onTrackLongClick(track, bounds) },
                                                )
                                                if (tokens.showTrackDividers && rowIdx < column.lastIndex) {
                                                    HorizontalDivider(
                                                        color = tokens.cardStroke.copy(alpha = 0.55f),
                                                        thickness = 0.5.dp,
                                                        modifier = Modifier.padding(start = 60.dp),
                                                    )
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Shelf 3: Find Your Mood (hidden in Personal Only Mode)
                    if (!settings.hideHomeMoods && !state.personalOnlyMode) {
                    item(key = "find_your_mood_shelf", contentType = "mood_shelf") {
                        val isBasicTheme = LocalLunariaColors.current.isBasic
                        val homeArtPool = remember(state.shelves, state.tracks) {
                            (
                                state.shelves.flatMap { s -> s.cards.mapNotNull { it.thumbnailUrl } } +
                                    state.tracks.mapNotNull { it.thumbnailUrl }
                                ).distinct()
                        }
                        Column(Modifier.padding(bottom = 26.dp)) {
                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                            SectionHeader(
                                title = strings.findYourMood,
                                onShowAll = onShowAllMoods,
                            )
                            LazyRow(
                                state = moodRowState,
                                flingBehavior = moodSnap,
                                modifier = Modifier.fillMaxWidth(),
                                contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                horizontalArrangement = Arrangement.spacedBy(moodSpacing),
                            ) {
                                if (state.moodCategories.isNotEmpty()) {
                                    itemsIndexed(
                                        items = state.moodCategories,
                                        key = { idx, cat -> "cat_${idx}_${cat.browseId}_${cat.params}_${cat.title}" },
                                        contentType = { _, _ -> if (isBasicTheme) "basic_mood_tile" else "mood_tile" },
                                    ) { idx, cat ->
                                        val localizedMood = dev.sonata.android.ui.i18n.localizeMoodTitle(cat.title, strings)
                                        val catAccent = cat.accentColor.takeIf { it != 0u }?.let { Color(it.toInt()) }
                                        if (isBasicTheme) {
                                            val art = homeArtPool.getOrNull(
                                                idx % homeArtPool.size.coerceAtLeast(1),
                                            )
                                            dev.sonata.android.ui.screens.BasicMoodSleeveCard(
                                                title = localizedMood,
                                                artworkUrl = art,
                                                accentColor = catAccent,
                                                onClick = { onOpenMoodCategory(cat.title, cat.browseId, cat.params) },
                                                modifier = Modifier
                                                    .width(196.dp)
                                                    .height(114.dp),
                                            )
                                        } else {
                                            MoodGenreSleeve(
                                                title = localizedMood,
                                                accentColor = catAccent,
                                                modifier = Modifier
                                                    .width(moodTileWidth)
                                                    .height(moodTileHeight)
                                                    .clickable { onOpenMoodCategory(cat.title, cat.browseId, cat.params) },
                                            )
                                        }
                                    }
                                } else {
                                    items(
                                        items = TopPickMood.entries,
                                        key = { it.name },
                                        contentType = { if (isBasicTheme) "basic_mood_tile" else "mood_tile" },
                                    ) { mood ->
                                        val localizedMood = dev.sonata.android.ui.i18n.localizeMoodTitle(mood.displayName, strings)
                                        if (isBasicTheme) {
                                            val art = homeArtPool.getOrNull(
                                                mood.ordinal % homeArtPool.size.coerceAtLeast(1),
                                            )
                                            dev.sonata.android.ui.screens.BasicMoodSleeveCard(
                                                title = localizedMood,
                                                artworkUrl = art,
                                                onClick = { onOpenMoodCategory(mood.displayName, "", "") },
                                                modifier = Modifier
                                                    .width(196.dp)
                                                    .height(114.dp),
                                            )
                                        } else {
                                            MoodTile(
                                                mood = mood,
                                                tileWidth = moodTileWidth,
                                                tileHeight = moodTileHeight,
                                                displayTitle = localizedMood,
                                                onClick = { _, label -> onOpenMoodCategory(label, "", "") },
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                    }

                    // Shelf 4: Personal Playlists
                    if (visibleHomePlaylists.isNotEmpty()) {
                        item(key = "personal_playlists_shelf", contentType = "personal_playlists_shelf") {
                            Column(Modifier.padding(bottom = 26.dp)) {
                                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                                SectionHeader(title = strings.yourPlaylists)
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = visibleHomePlaylists,
                                        key = { "home_pl_${it.id}" },
                                        contentType = { "playlist_card" },
                                    ) { playlist ->
                                        SonataEditorialCard(
                                            title = playlist.name,
                                            subtitle = strings.playlistLabel,
                                            artworkUrl = playlist.thumbnailUrl,
                                            badgeLabel = strings.playlistLabel,
                                            onClick = { onOpenPlaylist(playlist) },
                                            onPlayClick = { onOpenPlaylist(playlist) },
                                            onLongClick = { bounds -> onPlaylistLongClick(playlist, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }

                    // Remaining Shelves: clean 150.dp 1:1 square ShelfCards & 248.dp 16:9 VideoShelfCards (hidden in Personal Only Mode)
                    if (!state.personalOnlyMode) {
                    items(
                        items = remainingShelves,
                        key = { shelf -> "shelf_${shelf.title}" },
                        contentType = { "home_shelf" },
                    ) { shelf ->
                        val cards = remember(shelf.cards, settings.hideHomeVideos) {
                            shelf.cards
                                .filter { card ->
                                    card.kind != AndroidHomeCardKind.MOOD &&
                                        (!settings.hideHomeVideos || card.kind != AndroidHomeCardKind.VIDEO)
                                }
                                .distinctBy { "${it.kind}_${it.id}_${it.title}" }
                        }
                        if (cards.isNotEmpty()) {
                            val isVideoShelf = remember(cards) {
                                cards.all { it.kind == AndroidHomeCardKind.VIDEO }
                            }
                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                            Column(Modifier.padding(bottom = 26.dp)) {
                                SectionHeader(title = dev.sonata.android.ui.i18n.localizeShelfTitle(shelf.title, strings))
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = cards,
                                        key = { card -> "${card.kind}_${card.id}_${card.title}" },
                                        contentType = {
                                            if (isVideoShelf && !settings.squareVideoThumbnails) "video_card" else "home_card"
                                        },
                                    ) { card ->
                                        val activeId = state.activeTrack?.id
                                        val current = activeId != null && card.track?.id == activeId
                                        if (isVideoShelf && !settings.squareVideoThumbnails) {
                                            VideoShelfCard(
                                                card = card,
                                                isCurrent = current,
                                                isPlaying = current && state.isPlaying,
                                                onClick = { onOpenCard(card, shelf.tracks) },
                                                onLongClick = { bounds -> onCardLongClick(card, "VIDEO", false, bounds) },
                                            )
                                        } else if (isVideoShelf) {
                                            ShelfCard(
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
                                                    AndroidHomeCardKind.ALBUM -> strings.albumLabel
                                                    AndroidHomeCardKind.PLAYLIST -> strings.playlistLabel
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
}

@Composable
internal fun TrackScrollActivity(listState: LazyListState) {
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
internal fun SectionHeader(
    title: String,
    subtitle: String = "",
    onShowAll: (() -> Unit)? = null,
) {
    val tokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .padding(horizontal = PAGE_GUTTER, vertical = 10.dp)
            .fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (tokens.isViolet) {
            Box(
                modifier = Modifier
                    .width(4.dp)
                    .height(18.dp)
                    .clip(RoundedCornerShape(percent = 50))
                    .background(tokens.accentGradient),
            )
            Spacer(Modifier.width(9.dp))
        }
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
            if (tokens.isViolet) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(tokens.petalLavender.copy(alpha = 0.14f))
                        .clickable(onClick = onShowAll)
                        .padding(horizontal = 12.dp, vertical = 5.dp),
                ) {
                    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    Text(
                        text = strings.seeAll,
                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                        color = tokens.petalLavender,
                    )
                }
            } else {
                Text(
                    text = "${dev.sonata.android.ui.i18n.LocalSonataStrings.current.seeAll} ›",
                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                    color = tokens.petalLavender,
                    modifier = Modifier
                        .clickable(onClick = onShowAll)
                        .padding(start = 12.dp, top = 4.dp, bottom = 4.dp),
                )
            }
        }
    }
}

@Composable
internal fun SignInBanner(
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
        val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 18.dp)) {
            Text(
                text = strings.signInBannerTitle,
                style = MaterialTheme.typography.headlineSmall.copy(
                    fontWeight = FontWeight.ExtraBold,
                    fontSize = 24.sp,
                    lineHeight = 29.sp,
                ),
                color = Color.White,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                text = strings.signInBannerSubtitle,
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
                    text = strings.signIn,
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

internal data class TrackMenuRequest(
    val track: AndroidTrack,
    val anchorBounds: Rect? = null,
)

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

@Composable
internal fun ListeningHistoryScreen(
    tracks: List<AndroidTrack>,
    activeTrackId: String?,
    isPlaying: Boolean,
    contentPadding: PaddingValues,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onShuffleAll: (List<AndroidTrack>) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit,
    onTrackLongClick: (AndroidTrack, Int, Rect?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val listState = rememberLazyListState()
    TrackScrollActivity(listState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "history_header") {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                Text(
                    text = strings.tracksCount(tracks.size),
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.onSurfaceSecondary,
                )
                if (tracks.isNotEmpty()) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            modifier = Modifier
                                .clip(RoundedCornerShape(percent = 50))
                                .background(tokens.petalLavender)
                                .clickable { tracks.firstOrNull()?.let { onPlayTrack(it, tracks) } }
                                .padding(horizontal = 18.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Play,
                                contentDescription = null,
                                tint = Color.White,
                                modifier = Modifier.size(16.dp),
                            )
                            Text(
                                text = strings.playAll,
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = Color.White,
                            )
                        }
                        Row(
                            modifier = Modifier
                                .clip(RoundedCornerShape(percent = 50))
                                .background(tokens.eclipseSurface.copy(alpha = 0.82f))
                                .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(percent = 50))
                                .clickable { onShuffleAll(tracks) }
                                .padding(horizontal = 18.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Shuffle,
                                contentDescription = null,
                                tint = tokens.onSurfacePrimary,
                                modifier = Modifier.size(16.dp),
                            )
                            Text(
                                text = strings.shuffle,
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = tokens.onSurfacePrimary,
                            )
                        }
                    }
                }
            }
        }

        if (tracks.isEmpty()) {
            item(key = "history_empty") {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.History,
                        contentDescription = null,
                        tint = tokens.onSurfaceSecondary.copy(alpha = 0.65f),
                        modifier = Modifier.size(40.dp),
                    )
                    Text(
                        text = strings.playSongsToBuildHistory,
                        style = MaterialTheme.typography.bodyMedium,
                        color = tokens.onSurfaceSecondary,
                    )
                }
            }
        } else {
            itemsIndexed(
                items = tracks,
                key = { idx, tr -> "history_${idx}_${tr.id}" },
                contentType = { _, _ -> "history_track" },
            ) { idx, track ->
                val isCurrent = activeTrackId != null && track.id == activeTrackId
                Box(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                    CompactTrackRow(
                        track = track,
                        isCurrent = isCurrent,
                        isPlaying = isCurrent && isPlaying,
                        leadingIndex = idx + 1,
                        onClick = { onPlayTrack(track, tracks) },
                        onOptionsClick = { bounds -> onTrackOptions(track, bounds) },
                        onLongClick = { bounds -> onTrackLongClick(track, idx + 1, bounds) },
                    )
                }
            }
        }
    }
}

