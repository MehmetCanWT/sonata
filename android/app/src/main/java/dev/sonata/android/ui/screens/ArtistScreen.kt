/*
 * Clean-room Sonata full-screen Artist profile page.
 *
 * Presents a full-bleed artist hero fading smoothly into a dark neutral wash,
 * floating Liquid Glass top chrome, Shuffle / Play / Subscribe action bar,
 * Latest Release spotlight, Top Songs, Albums & Singles shelf, and Similar
 * Artists shelf — all kept strictly in-app with zero external redirects.
 */
package dev.sonata.android.ui.screens

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.backdrop.backdrops.LayerBackdrop
import dev.sonata.android.backdrop.backdrops.layerBackdrop
import dev.sonata.android.backdrop.backdrops.rememberLayerBackdrop
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.ShimmerBox
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.SonataShimmerHost
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.components.ScrollActivityTracker
import dev.sonata.android.ui.rememberArtworkPalette
import dev.sonata.android.ui.rememberArtworkRequest
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.sonata_android.AndroidTrack

@Immutable
data class ArtistReleaseItem(
    val id: String,
    val title: String,
    val subtitle: String,
    val year: String? = null,
    val kindLabel: String = "Album",
    val thumbnailUrl: String? = null,
    val isSaved: Boolean = false,
)

@Immutable
data class ArtistSimilarItem(
    val id: String,
    val name: String,
    val subtitle: String? = null,
    val thumbnailUrl: String? = null,
)

@Immutable
data class ArtistTrackItem(
    val track: AndroidTrack,
    val isExplicit: Boolean = false,
    val isDownloaded: Boolean = false,
)

@Immutable
data class ArtistScreenData(
    val id: String,
    val name: String,
    val subscriberText: String? = null,
    val monthlyListeners: String? = null,
    val aboutBio: String? = null,
    val avatarUrl: String? = null,
    val isSubscribed: Boolean = false,
    val latestRelease: ArtistReleaseItem? = null,
    val topSongs: List<ArtistTrackItem> = emptyList(),
    val albums: List<ArtistReleaseItem> = emptyList(),
    val singles: List<ArtistReleaseItem> = emptyList(),
    val similarArtists: List<ArtistSimilarItem> = emptyList(),
)

private val ArtistBaseWash = Color(0xFF1C1C1E)
private val ArtistDeepBackground = Color(0xFF0A0A0C)
private val ArtistCardNeutral = Color(0xFF242428)
private val ArtistButtonMuted = Color(0xFF2C2C30)

/**
 * Full-screen Artist profile page with full-bleed hero artwork, floating Liquid Glass
 * navigation chrome, Play/Shuffle/Subscribe actions, Latest Release, Top Songs,
 * Albums & Singles, and Similar Artists shelves.
 */
@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun ArtistScreen(
    artist: ArtistScreenData?,
    loading: Boolean = false,
    activeTrackId: String? = null,
    userAvatarUrl: String? = null,
    contentBottomPadding: PaddingValues = PaddingValues(bottom = 120.dp),
    onBack: () -> Unit,
    onShareArtist: (ArtistScreenData) -> Unit = {},
    onAvatarClick: () -> Unit = {},
    onPlayTopSongs: (ArtistScreenData) -> Unit = {},
    onShuffleTopSongs: (ArtistScreenData) -> Unit = {},
    onToggleSubscribe: (ArtistScreenData) -> Unit = {},
    onOpenRelease: (ArtistReleaseItem) -> Unit = {},
    onSaveRelease: (ArtistReleaseItem) -> Unit = {},
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit = { _, _ -> },
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Int, Rect?) -> Unit = { _, _, _ -> },
    onReleaseLongClick: (ArtistReleaseItem, Rect?) -> Unit = { _, _ -> },
    onOpenAllTopSongs: (ArtistScreenData) -> Unit = {},
    onOpenSimilarArtist: (ArtistSimilarItem) -> Unit = {},
    backdrop: LayerBackdrop? = null,
    modifier: Modifier = Modifier,
) {
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val resolvedHeroUrl = remember(artist) {
        artist?.avatarUrl?.takeIf { it.isNotBlank() }
            ?: artist?.latestRelease?.thumbnailUrl?.takeIf { it.isNotBlank() }
            ?: artist?.albums?.firstOrNull()?.thumbnailUrl?.takeIf { it.isNotBlank() }
            ?: artist?.topSongs?.firstOrNull()?.track?.thumbnailUrl
    }
    val palette = rememberArtworkPalette(
        artworkUrl = resolvedHeroUrl,
        dark = true,
        artPx = HEADER_ART_PX,
    )
    val moodyWash = palette.wash
    val artistBgBrush = remember(moodyWash, palette.background) {
        Brush.verticalGradient(
            0f to moodyWash,
            0.48f to palette.background,
            1f to ArtistDeepBackground,
        )
    }
    val paintArtistBackdrop: ContentDrawScope.() -> Unit = remember(artistBgBrush) {
        {
            drawRect(brush = artistBgBrush)
            drawContent()
        }
    }
    val artistBackdrop = backdrop ?: rememberLayerBackdrop(onDraw = paintArtistBackdrop)
    val showSkeleton = loading && (
        artist == null || (
            artist.topSongs.isEmpty() &&
                artist.albums.isEmpty() &&
                artist.singles.isEmpty() &&
                artist.latestRelease == null
            )
        )

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(artistBgBrush),
    ) {
        if (showSkeleton) {
            ArtistLoadingSkeleton(
                artistName = artist?.name,
                heroUrl = resolvedHeroUrl,
                statusTop = statusTop,
                moodyWash = moodyWash,
                bottomPadding = contentBottomPadding.calculateBottomPadding() + 24.dp,
                modifier = Modifier
                    .fillMaxSize()
                    .layerBackdrop(artistBackdrop),
            )
        } else if (artist != null) {
            val allTracks = remember(artist.topSongs) { artist.topSongs.map { it.track } }
            val songColumns = remember(artist.topSongs) { artist.topSongs.chunked(4) }
            val artistListState = rememberLazyListState()
            val songsListState = rememberLazyListState()
            val snapFling = rememberSnapFlingBehavior(lazyListState = songsListState)
            LaunchedEffect(artistListState) {
                var active = false
                try {
                    snapshotFlow { artistListState.isScrollInProgress }
                        .distinctUntilChanged()
                        .collect { scrolling ->
                            if (scrolling != active) {
                                active = scrolling
                                ScrollActivityTracker.setScrolling(scrolling)
                            }
                        }
                } finally {
                    if (active) ScrollActivityTracker.setScrolling(false)
                }
            }
            val screenWidth = LocalConfiguration.current.screenWidthDp.dp
            val columnWidth = when {
                screenWidth >= 900.dp -> ((screenWidth - PAGE_GUTTER * 2 - 24.dp) / 3f).coerceIn(270.dp, 340.dp)
                screenWidth >= 600.dp -> ((screenWidth - PAGE_GUTTER * 2 - 12.dp) / 2f).coerceIn(265.dp, 360.dp)
                else -> (screenWidth - 44.dp).coerceIn(272.dp, 360.dp)
            }

            LazyColumn(
                state = artistListState,
                modifier = Modifier
                    .fillMaxSize()
                    .layerBackdrop(artistBackdrop),
                contentPadding = PaddingValues(
                    bottom = contentBottomPadding.calculateBottomPadding() + 28.dp,
                ),
            ) {
                // Full-bleed artist hero fading smoothly into artwork-derived moody wash
                item(key = "hero_${artist.id}", contentType = "artist_hero") {
                    ArtistHeroHeader(
                        artist = artist,
                        heroUrl = resolvedHeroUrl,
                        moodyWash = moodyWash,
                        statusTop = statusTop,
                        onPlay = { onPlayTopSongs(artist) },
                        onShuffle = { onShuffleTopSongs(artist) },
                        onToggleSubscribe = { onToggleSubscribe(artist) },
                    )
                }

                // Sculpted Tinted Latest Release spotlight card (centered & width-capped on tablets)
                artist.latestRelease?.let { release ->
                    item(key = "latest_${release.id}", contentType = "artist_latest_release") {
                        Spacer(Modifier.height(14.dp))
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = PAGE_GUTTER),
                            contentAlignment = Alignment.Center,
                        ) {
                            LatestReleaseCard(
                                release = release,
                                washColor = moodyWash,
                                onClick = { onOpenRelease(release) },
                                onLongClick = { bounds -> onReleaseLongClick(release, bounds) },
                                onSaveClick = { onSaveRelease(release) },
                                modifier = Modifier
                                    .widthIn(max = 560.dp)
                                    .fillMaxWidth(),
                            )
                        }
                    }
                }

                // Popular tracks horizontal 4-row paging columns with circular Play button
                if (artist.topSongs.isNotEmpty()) {
                    item(key = "top_songs_${artist.id}", contentType = "artist_top_songs") {
                        Spacer(Modifier.height(22.dp))
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = PAGE_GUTTER, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Row(
                                modifier = Modifier
                                    .weight(1f)
                                    .clickable(enabled = artist.topSongs.size > 4) {
                                        onOpenAllTopSongs(artist)
                                    },
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(
                                    text = "Popular tracks",
                                    style = MaterialTheme.typography.titleLarge.copy(
                                        fontSize = 22.sp,
                                        fontWeight = FontWeight.Bold,
                                        letterSpacing = (-0.3).sp,
                                    ),
                                    color = Color.White,
                                )
                                if (artist.topSongs.size > 4) {
                                    Spacer(Modifier.width(4.dp))
                                    Icon(
                                        imageVector = SonataIcons.ChevronRight,
                                        contentDescription = "See all",
                                        tint = Color.White.copy(alpha = 0.55f),
                                        modifier = Modifier.size(20.dp),
                                    )
                                }
                            }
                            Box(
                                modifier = Modifier
                                    .size(40.dp)
                                    .clip(CircleShape)
                                    .background(SonataAccent.copy(alpha = 0.24f))
                                    .border(0.5.dp, SonataAccent.copy(alpha = 0.45f), CircleShape)
                                    .clickable { onPlayTopSongs(artist) },
                                contentAlignment = Alignment.Center,
                            ) {
                                Icon(
                                    imageVector = SonataIcons.Play,
                                    contentDescription = "Play popular tracks",
                                    tint = Color.White,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                        }
                        LazyRow(
                            state = songsListState,
                            flingBehavior = snapFling,
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            itemsIndexed(
                                items = songColumns,
                                key = { _, col -> col.firstOrNull()?.track?.id ?: col.hashCode() },
                                contentType = { _, _ -> "artist_song_column" },
                            ) { colIdx, columnTracks ->
                                Column(
                                    modifier = Modifier.width(columnWidth),
                                    verticalArrangement = Arrangement.spacedBy(2.dp),
                                ) {
                                    columnTracks.forEachIndexed { rowIdx, item ->
                                        val trackNumber = colIdx * 4 + rowIdx + 1
                                        ArtistSongRow(
                                            item = item,
                                            trackNumber = trackNumber,
                                            isActive = item.track.id == activeTrackId,
                                            onClick = { onPlayTrack(item.track, allTracks) },
                                            onLongClick = { bounds -> onTrackLongClick(item.track, trackNumber, bounds) },
                                            onOptionsClick = { bounds -> onTrackOptions(item.track, bounds) },
                                        )
                                    }
                                }
                            }
                        }
                    }
                }

                // Albums shelf
                if (artist.albums.isNotEmpty()) {
                    item(key = "albums_${artist.id}", contentType = "artist_albums") {
                        Spacer(Modifier.height(24.dp))
                        SectionHeaderRow(title = "Albums")
                        ReleaseHorizontalShelf(
                            releases = artist.albums,
                            onReleaseClick = onOpenRelease,
                            onReleaseLongClick = onReleaseLongClick,
                        )
                    }
                }

                // Singles & EPs shelf
                if (artist.singles.isNotEmpty()) {
                    item(key = "singles_${artist.id}", contentType = "artist_singles") {
                        Spacer(Modifier.height(24.dp))
                        SectionHeaderRow(title = "Singles & EPs")
                        ReleaseHorizontalShelf(
                            releases = artist.singles,
                            onReleaseClick = onOpenRelease,
                            onReleaseLongClick = onReleaseLongClick,
                        )
                    }
                }

                // Similar Artists shelf
                if (artist.similarArtists.isNotEmpty()) {
                    item(key = "similar_${artist.id}", contentType = "artist_similar") {
                        Spacer(Modifier.height(24.dp))
                        SectionHeaderRow(title = "Similar Artists")
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(14.dp),
                        ) {
                            items(
                                items = artist.similarArtists,
                                key = { it.id },
                                contentType = { "similar_artist" },
                            ) { similar ->
                                SimilarArtistCard(
                                    artist = similar,
                                    onClick = { onOpenSimilarArtist(similar) },
                                )
                            }
                        }
                    }
                }

                // About Artist section with expandable More/Less toggle & stats pills
                val hasAboutSection = !artist.aboutBio.isNullOrBlank() ||
                    !artist.subscriberText.isNullOrBlank() ||
                    !artist.monthlyListeners.isNullOrBlank()
                if (hasAboutSection) {
                    item(key = "about_${artist.id}", contentType = "artist_about") {
                        Spacer(Modifier.height(26.dp))
                        AboutArtistSection(
                            artistName = artist.name,
                            bio = artist.aboutBio,
                            subscriberText = artist.subscriberText,
                            monthlyListeners = artist.monthlyListeners,
                        )
                    }
                }
            }
        }

        // Top floating Liquid Glass bar (sibling outside artistBackdrop — never creates a RenderNode cycle)
        CompositionLocalProvider(LocalAppBackdrop provides artistBackdrop) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = statusTop + 8.dp, start = PAGE_GUTTER, end = PAGE_GUTTER),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                FloatingTopCircleButton(
                    onClick = onBack,
                    contentDescription = "Back",
                ) {
                    Icon(
                        imageVector = SonataIcons.ChevronLeft,
                        contentDescription = "Back",
                        tint = Color.White,
                        modifier = Modifier.size(22.dp),
                    )
                }

                if (artist != null) {
                    FloatingShareAndAvatarPill(
                        userAvatarUrl = userAvatarUrl,
                        onShareClick = { onShareArtist(artist) },
                        onAvatarClick = onAvatarClick,
                    )
                }
            }
        }
    }
}

@Composable
private fun ArtistHeroHeader(
    artist: ArtistScreenData,
    heroUrl: String?,
    moodyWash: Color,
    statusTop: androidx.compose.ui.unit.Dp,
    onPlay: () -> Unit,
    onShuffle: () -> Unit,
    onToggleSubscribe: () -> Unit,
) {
    val heroHeight = 360.dp + statusTop
    val heroReq = rememberArtworkRequest(
        url = heroUrl.artworkAt(HEADER_ART_PX),
        widthPx = HEADER_ART_PX,
        heightPx = HEADER_ART_PX,
    )
    // Local hero-only backdrop so Shuffle & Star buttons can sample the hero artwork
    // as a sibling overlay without forming a RenderNode cycle inside artistBackdrop.
    val heroBackdrop = rememberLayerBackdrop()
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val heroTopVignetteBrush = remember {
        Brush.verticalGradient(
            0.0f to Color.Black.copy(alpha = 0.36f),
            0.26f to Color.Transparent,
            0.62f to Color.Transparent,
            1.0f to Color.Black.copy(alpha = 0.24f),
        )
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(heroHeight),
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .layerBackdrop(heroBackdrop),
        ) {
            if (heroReq != null) {
                // Smooth alpha dissolve matching Portrait Now Playing so the artist portrait
                // melts seamlessly into the artwork-derived page background with zero seam.
                AsyncImage(
                    model = heroReq,
                    contentDescription = artist.name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxSize()
                        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                        .drawWithCache {
                            val fadeBrush = Brush.verticalGradient(
                                colorStops = arrayOf(
                                    0.00f to Color.Black,
                                    0.32f to Color.Black.copy(alpha = 0.86f),
                                    0.64f to Color.Black.copy(alpha = 0.44f),
                                    0.86f to Color.Black.copy(alpha = 0.14f),
                                    1.00f to Color.Transparent,
                                ),
                                startY = size.height * 0.40f,
                                endY = size.height,
                            )
                            onDrawWithContent {
                                drawContent()
                                drawRect(brush = fadeBrush, blendMode = BlendMode.DstIn)
                            }
                        },
                )
            } else {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(
                            Brush.verticalGradient(
                                0f to moodyWash.copy(alpha = 0.65f),
                                1f to Color.Transparent,
                            ),
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.Person,
                        contentDescription = null,
                        tint = Color.White.copy(alpha = 0.25f),
                        modifier = Modifier.size(96.dp),
                    )
                }
            }

            // Subtle top/bottom readability vignette for chrome and artist title
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(heroTopVignetteBrush),
            )
        }

        val releaseCount = artist.albums.size + artist.singles.size
        val summaryLine = remember(artist.topSongs.size, releaseCount) {
            buildList {
                if (artist.topSongs.isNotEmpty()) add("${artist.topSongs.size}+ tracks")
                if (releaseCount > 0) add("$releaseCount releases")
            }.joinToString(" • ")
        }

        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .padding(horizontal = PAGE_GUTTER, vertical = 14.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = artist.name,
                style = MaterialTheme.typography.headlineLarge.copy(
                    fontSize = 36.sp,
                    fontWeight = FontWeight.ExtraBold,
                    letterSpacing = (-0.8).sp,
                ),
                color = Color.White,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            artist.subscriberText?.takeIf { it.isNotBlank() }?.let { subs ->
                Spacer(Modifier.height(4.dp))
                Text(
                    text = subs,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Medium,
                    ),
                    color = Color.White.copy(alpha = 0.78f),
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (summaryLine.isNotBlank()) {
                Spacer(Modifier.height(2.dp))
                Text(
                    text = summaryLine,
                    style = MaterialTheme.typography.bodySmall.copy(
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Normal,
                    ),
                    color = Color.White.copy(alpha = 0.62f),
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            Spacer(Modifier.height(16.dp))

            CompositionLocalProvider(LocalAppBackdrop provides heroBackdrop) {
                Row(
                    modifier = Modifier
                        .widthIn(max = 360.dp)
                        .fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterHorizontally),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Circular Liquid Glass Shuffle button
                    val shuffleGlassMod = if (glassOn) {
                        Modifier
                            .size(52.dp)
                            .liquidGlass(
                                shape = CircleShape,
                                surfaceTint = Color.Black.copy(alpha = 0.24f),
                                chromaticAberration = false,
                            )
                            .border(0.5.dp, Color.White.copy(alpha = 0.20f), CircleShape)
                    } else {
                        Modifier
                            .size(52.dp)
                            .clip(CircleShape)
                            .background(ArtistButtonMuted.copy(alpha = 0.90f))
                            .border(0.5.dp, Color.White.copy(alpha = 0.14f), CircleShape)
                    }
                    Box(
                        modifier = shuffleGlassMod.clickable(onClick = onShuffle),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Shuffle,
                            contentDescription = "Shuffle",
                            tint = Color.White,
                            modifier = Modifier.size(22.dp),
                        )
                    }

                    // Large primary White Pill PLAY button
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(52.dp)
                            .clip(CircleShape)
                            .background(Color.White)
                            .clickable(onClick = onPlay),
                        contentAlignment = Alignment.Center,
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Play,
                                contentDescription = null,
                                tint = Color(0xFF101012),
                                modifier = Modifier.size(18.dp),
                            )
                            Text(
                                text = "PLAY",
                                style = MaterialTheme.typography.titleSmall.copy(
                                    fontSize = 15.sp,
                                    fontWeight = FontWeight.ExtraBold,
                                    letterSpacing = 0.8.sp,
                                ),
                                color = Color(0xFF101012),
                            )
                        }
                    }

                    // Circular Liquid Glass Subscribe / Favorite button
                    val subscribeGlassMod = if (glassOn) {
                        Modifier
                            .size(52.dp)
                            .liquidGlass(
                                shape = CircleShape,
                                surfaceTint = if (artist.isSubscribed) {
                                    SonataAccent.copy(alpha = 0.24f)
                                } else {
                                    Color.Black.copy(alpha = 0.24f)
                                },
                                chromaticAberration = false,
                            )
                            .border(
                                width = 0.5.dp,
                                color = if (artist.isSubscribed) SonataAccent.copy(alpha = 0.65f)
                                else Color.White.copy(alpha = 0.20f),
                                shape = CircleShape,
                            )
                    } else {
                        Modifier
                            .size(52.dp)
                            .clip(CircleShape)
                            .background(
                                if (artist.isSubscribed) SonataAccent.copy(alpha = 0.22f)
                                else ArtistButtonMuted.copy(alpha = 0.90f),
                            )
                            .border(
                                width = 0.5.dp,
                                color = if (artist.isSubscribed) SonataAccent.copy(alpha = 0.65f)
                                else Color.White.copy(alpha = 0.14f),
                                shape = CircleShape,
                            )
                    }
                    Box(
                        modifier = subscribeGlassMod.clickable(onClick = onToggleSubscribe),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = if (artist.isSubscribed) SonataIcons.StarFilled else SonataIcons.Star,
                            contentDescription = if (artist.isSubscribed) "Subscribed" else "Subscribe",
                            tint = if (artist.isSubscribed) SonataAccent else Color.White,
                            modifier = Modifier.size(22.dp),
                        )
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun LatestReleaseCard(
    release: ArtistReleaseItem,
    washColor: Color = ArtistCardNeutral,
    onClick: () -> Unit,
    onLongClick: (Rect?) -> Unit = {},
    onSaveClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val coverReq = rememberArtworkRequest(
        url = release.thumbnailUrl.artworkAt(CARD_ART_PX),
        widthPx = CARD_ART_PX,
        heightPx = CARD_ART_PX,
    )
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val cardShape = remember { RoundedCornerShape(26.dp) }
    val cardBg = remember(washColor) {
        Brush.horizontalGradient(
            0f to washColor.copy(alpha = 0.58f),
            1f to Color(0xFF1F1C26).copy(alpha = 0.92f),
        )
    }

    Row(
        modifier = modifier
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .clip(cardShape)
            .background(cardBg)
            .border(0.5.dp, Color.White.copy(alpha = 0.12f), cardShape)
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(cardBounds)
                },
            )
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Tactile layered & subtly tilted album sleeve stack
        Box(
            modifier = Modifier
                .width(106.dp)
                .height(92.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            if (coverReq != null) {
                Box(
                    modifier = Modifier
                        .offset(x = 18.dp, y = 2.dp)
                        .size(76.dp)
                        .graphicsLayer {
                            rotationZ = 7.5f
                            alpha = 0.44f
                        }
                        .clip(RoundedCornerShape(14.dp))
                        .background(Color(0xFF161618)),
                ) {
                    AsyncImage(
                        model = coverReq,
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
            Box(
                modifier = Modifier
                    .size(86.dp)
                    .graphicsLayer { rotationZ = -3.5f }
                    .clip(RoundedCornerShape(16.dp))
                    .background(Color(0xFF161618))
                    .border(0.5.dp, Color.White.copy(alpha = 0.18f), RoundedCornerShape(16.dp)),
                contentAlignment = Alignment.Center,
            ) {
                if (coverReq != null) {
                    AsyncImage(
                        model = coverReq,
                        contentDescription = release.title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = Color.White.copy(alpha = 0.45f),
                        modifier = Modifier.size(28.dp),
                    )
                }
            }
        }

        Spacer(Modifier.width(14.dp))

        Column(modifier = Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(SonataAccent),
                )
                Text(
                    text = "Latest release",
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontSize = 13.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = SonataAccent,
                    maxLines = 1,
                )
            }
            Spacer(Modifier.height(4.dp))
            Text(
                text = release.title,
                style = MaterialTheme.typography.titleLarge.copy(
                    fontSize = 19.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = (-0.2).sp,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val meta = listOfNotNull(
                release.kindLabel.takeIf { it.isNotBlank() },
                release.year?.takeIf { it.isNotBlank() } ?: release.subtitle.takeIf { it.isNotBlank() },
            ).joinToString(" • ")
            if (meta.isNotBlank()) {
                Spacer(Modifier.height(3.dp))
                Text(
                    text = meta,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
                    color = Color.White.copy(alpha = 0.68f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }

        Spacer(Modifier.width(8.dp))

        Box(
            modifier = Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(Color.White.copy(alpha = 0.14f))
                .clickable(onClick = onSaveClick),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = if (release.isSaved) SonataIcons.Check else ArtistPlusIcon,
                contentDescription = if (release.isSaved) "Saved" else "Add to library",
                tint = Color.White,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ArtistSongRow(
    item: ArtistTrackItem,
    isActive: Boolean,
    onClick: () -> Unit,
    trackNumber: Int? = null,
    onLongClick: (Rect?) -> Unit = {},
    onOptionsClick: (Rect?) -> Unit,
) {
    val track = item.track
    val haptics = LocalHapticFeedback.current
    var rowBounds by remember { mutableStateOf<Rect?>(null) }
    var optionsBounds by remember { mutableStateOf<Rect?>(null) }
    val artReq = rememberArtworkRequest(
        url = track.thumbnailUrl.artworkAt(ROW_ART_PX),
        widthPx = ROW_ART_PX,
        heightPx = ROW_ART_PX,
    )

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .onGloballyPositioned { rowBounds = it.boundsInRoot() }
            .clip(RoundedCornerShape(10.dp))
            .background(if (isActive) Color.White.copy(alpha = 0.08f) else Color.Transparent)
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick(rowBounds)
                },
            )
            .padding(vertical = 6.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(50.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(Color(0xFF28282C)),
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
                    tint = Color.White.copy(alpha = 0.45f),
                    modifier = Modifier.size(20.dp),
                )
            }
        }

        if (trackNumber != null) {
            Spacer(Modifier.width(10.dp))
            Text(
                text = trackNumber.toString(),
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontSize = 13.5.sp,
                    fontWeight = FontWeight.SemiBold,
                ),
                color = if (isActive) SonataAccent else Color.White.copy(alpha = 0.52f),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                modifier = Modifier.width(20.dp),
            )
            Spacer(Modifier.width(10.dp))
        } else {
            Spacer(Modifier.width(12.dp))
        }

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.title,
                style = MaterialTheme.typography.bodyLarge.copy(
                    fontSize = 14.5.sp,
                    fontWeight = FontWeight.SemiBold,
                ),
                color = if (isActive) SonataAccent else Color.White,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(2.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                if (item.isExplicit) {
                    Box(
                        modifier = Modifier
                            .size(14.dp)
                            .clip(RoundedCornerShape(3.dp))
                            .background(Color.White.copy(alpha = 0.24f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = "E",
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 9.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                        )
                    }
                }
                Text(
                    text = track.artist,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                    color = Color.White.copy(alpha = 0.62f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (item.isDownloaded) {
                    Icon(
                        imageVector = SonataIcons.Download,
                        contentDescription = "Downloaded",
                        tint = SonataAccent,
                        modifier = Modifier.size(13.dp),
                    )
                }
            }
        }

        IconButton(
            onClick = { onOptionsClick(optionsBounds) },
            modifier = Modifier
                .size(36.dp)
                .onGloballyPositioned { optionsBounds = it.boundsInRoot() },
        ) {
            Icon(
                imageVector = SonataIcons.MoreVert,
                contentDescription = "Track options",
                tint = Color.White.copy(alpha = 0.65f),
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ReleaseHorizontalShelf(
    releases: List<ArtistReleaseItem>,
    onReleaseClick: (ArtistReleaseItem) -> Unit,
    onReleaseLongClick: (ArtistReleaseItem, Rect?) -> Unit = { _, _ -> },
) {
    val haptics = LocalHapticFeedback.current
    LazyRow(
        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(
            items = releases,
            key = { it.id },
            contentType = { "artist_release" },
        ) { release ->
            val coverReq = rememberArtworkRequest(
                url = release.thumbnailUrl.artworkAt(CARD_ART_PX),
                widthPx = 360,
                heightPx = 360,
            )
            var cardBounds by remember { mutableStateOf<Rect?>(null) }
            Column(
                modifier = Modifier
                    .width(146.dp)
                    .onGloballyPositioned { cardBounds = it.boundsInRoot() }
                    .combinedClickable(
                        onClick = { onReleaseClick(release) },
                        onLongClick = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            onReleaseLongClick(release, cardBounds)
                        },
                    ),
            ) {
                Box(
                    modifier = Modifier
                        .size(146.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .background(ArtistCardNeutral)
                        .border(0.5.dp, Color.White.copy(alpha = 0.08f), RoundedCornerShape(12.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    if (coverReq != null) {
                        AsyncImage(
                            model = coverReq,
                            contentDescription = release.title,
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
                }
                Spacer(Modifier.height(7.dp))
                Text(
                    text = release.title,
                    style = MaterialTheme.typography.titleSmall.copy(
                        fontSize = 13.5.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = Color.White,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                val meta = listOfNotNull(
                    release.year?.takeIf { it.isNotBlank() },
                    release.subtitle.takeIf { it.isNotBlank() },
                ).joinToString(" • ")
                if (meta.isNotBlank()) {
                    Text(
                        text = meta,
                        style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                        color = Color.White.copy(alpha = 0.60f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun SimilarArtistCard(
    artist: ArtistSimilarItem,
    onClick: () -> Unit,
) {
    val avatarReq = rememberArtworkRequest(
        url = artist.thumbnailUrl.artworkAt(CARD_ART_PX),
        widthPx = 280,
        heightPx = 280,
    )
    Column(
        modifier = Modifier
            .width(118.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(108.dp)
                .clip(CircleShape)
                .background(ArtistCardNeutral)
                .border(0.5.dp, Color.White.copy(alpha = 0.10f), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (avatarReq != null) {
                AsyncImage(
                    model = avatarReq,
                    contentDescription = artist.name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.Person,
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.48f),
                    modifier = Modifier.size(34.dp),
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = artist.name,
            style = MaterialTheme.typography.titleSmall.copy(
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
            ),
            color = Color.White,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
        artist.subtitle?.takeIf { it.isNotBlank() }?.let { sub ->
            Text(
                text = sub,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 11.5.sp),
                color = Color.White.copy(alpha = 0.56f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun SectionHeaderRow(
    title: String,
    showChevron: Boolean = false,
    onHeaderClick: (() -> Unit)? = null,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (onHeaderClick != null && showChevron) {
                    Modifier.clickable(onClick = onHeaderClick)
                } else {
                    Modifier
                },
            )
            .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleLarge.copy(
                fontSize = 20.sp,
                fontWeight = FontWeight.Bold,
            ),
            color = Color.White,
        )
        if (showChevron) {
            Icon(
                imageVector = SonataIcons.ChevronRight,
                contentDescription = "See all",
                tint = Color.White.copy(alpha = 0.65f),
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@Composable
private fun FloatingTopCircleButton(
    onClick: () -> Unit,
    contentDescription: String,
    content: @Composable () -> Unit,
) {
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val baseModifier = if (glassOn) {
        Modifier
            .size(40.dp)
            .liquidGlass(
                shape = CircleShape,
                surfaceTint = Color.Black.copy(alpha = 0.34f),
                chromaticAberration = false,
            )
    } else {
        Modifier
            .size(40.dp)
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.52f), CircleShape)
            .border(0.5.dp, Color.White.copy(alpha = 0.18f), CircleShape)
    }
    Box(
        modifier = baseModifier.clickable(onClick = onClick, onClickLabel = contentDescription),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
private fun FloatingShareAndAvatarPill(
    userAvatarUrl: String?,
    onShareClick: () -> Unit,
    onAvatarClick: () -> Unit,
) {
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val avatarReq = rememberArtworkRequest(
        url = userAvatarUrl.artworkAt(ROW_ART_PX),
        widthPx = ROW_ART_PX,
        heightPx = ROW_ART_PX,
    )
    val containerModifier = if (glassOn) {
        Modifier
            .height(40.dp)
            .liquidGlass(
                shape = CircleShape,
                surfaceTint = Color.Black.copy(alpha = 0.34f),
                chromaticAberration = false,
            )
    } else {
        Modifier
            .height(40.dp)
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.52f), CircleShape)
            .border(0.5.dp, Color.White.copy(alpha = 0.18f), CircleShape)
    }

    Row(
        modifier = containerModifier.padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(
            modifier = Modifier
                .size(30.dp)
                .clip(CircleShape)
                .clickable(onClick = onShareClick),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = ArtistShareIcon,
                contentDescription = "Share artist",
                tint = Color.White,
                modifier = Modifier.size(18.dp),
            )
        }

        Box(
            modifier = Modifier
                .size(28.dp)
                .clip(CircleShape)
                .background(Color.White.copy(alpha = 0.14f))
                .clickable(onClick = onAvatarClick),
            contentAlignment = Alignment.Center,
        ) {
            if (avatarReq != null) {
                AsyncImage(
                    model = avatarReq,
                    contentDescription = "Account",
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.Person,
                    contentDescription = "Account",
                    tint = Color.White,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
    }
}

private val ArtistPlusIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "ArtistPlus",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 2.2f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(12f, 5f)
            lineTo(12f, 19f)
            moveTo(5f, 12f)
            lineTo(19f, 12f)
        }
    }.build()
}

private val ArtistShareIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "ArtistShare",
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
            moveTo(12f, 3f)
            lineTo(12f, 14f)
            moveTo(7.5f, 7.5f)
            lineTo(12f, 3f)
            lineTo(16.5f, 7.5f)
            moveTo(5f, 13f)
            lineTo(5f, 19f)
            arcTo(2f, 2f, 0f, false, false, 7f, 21f)
            lineTo(17f, 21f)
            arcTo(2f, 2f, 0f, false, false, 19f, 19f)
            lineTo(19f, 13f)
        }
    }.build()
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AboutArtistSection(
    artistName: String,
    bio: String?,
    subscriberText: String?,
    monthlyListeners: String?,
) {
    var expanded by remember(artistName, bio) { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = PAGE_GUTTER)
            .animateContentSize(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            text = "About $artistName",
            style = MaterialTheme.typography.titleLarge.copy(
                fontSize = 20.sp,
                fontWeight = FontWeight.Bold,
            ),
            color = Color.White,
        )

        if (!bio.isNullOrBlank()) {
            Text(
                text = bio.trim(),
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontSize = 14.sp,
                    lineHeight = 20.sp,
                ),
                color = Color.White.copy(alpha = 0.80f),
                maxLines = if (expanded) Int.MAX_VALUE else 3,
                overflow = TextOverflow.Ellipsis,
            )
            if (bio.length > 140) {
                Text(
                    text = if (expanded) "Show less" else "More",
                    style = MaterialTheme.typography.labelLarge.copy(
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = SonataAccent,
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable { expanded = !expanded }
                        .padding(vertical = 2.dp),
                )
            }
        }

        val pills = listOfNotNull(
            subscriberText?.takeIf { it.isNotBlank() }?.let { SonataIcons.Person to it },
            monthlyListeners?.takeIf { it.isNotBlank() }?.let { SonataIcons.GraphicEq to it },
        )
        if (pills.isNotEmpty()) {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                pills.forEach { (icon, label) ->
                    Row(
                        modifier = Modifier
                            .clip(CircleShape)
                            .background(Color.White.copy(alpha = 0.10f))
                            .border(0.5.dp, Color.White.copy(alpha = 0.14f), CircleShape)
                            .padding(horizontal = 12.dp, vertical = 7.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Icon(
                            imageVector = icon,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.78f),
                            modifier = Modifier.size(14.dp),
                        )
                        Text(
                            text = label,
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                            color = Color.White.copy(alpha = 0.90f),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ArtistLoadingSkeleton(
    artistName: String?,
    heroUrl: String?,
    statusTop: androidx.compose.ui.unit.Dp,
    moodyWash: Color,
    bottomPadding: androidx.compose.ui.unit.Dp,
    modifier: Modifier = Modifier,
) {
    val heroHeight = 360.dp + statusTop
    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    val columnCount = when {
        screenWidth >= 900.dp -> 3
        screenWidth >= 600.dp -> 2
        else -> 1
    }
    val heroReq = rememberArtworkRequest(
        url = heroUrl.artworkAt(HEADER_ART_PX),
        widthPx = HEADER_ART_PX,
        heightPx = HEADER_ART_PX,
    )

    SonataShimmerHost {
        LazyColumn(
            modifier = modifier,
            contentPadding = PaddingValues(bottom = bottomPadding),
            userScrollEnabled = false,
        ) {
            item(key = "artist_skel_hero") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(heroHeight),
                ) {
                    if (heroReq != null) {
                        AsyncImage(
                            model = heroReq,
                            contentDescription = artistName,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        ShimmerBox(modifier = Modifier.fillMaxSize(), shape = RoundedCornerShape(0.dp))
                    }
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(
                                Brush.verticalGradient(
                                    0.0f to Color.Black.copy(alpha = 0.34f),
                                    0.35f to Color.Transparent,
                                    1.0f to moodyWash,
                                ),
                            ),
                    )
                    Column(
                        modifier = Modifier
                            .align(Alignment.BottomStart)
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER, vertical = 14.dp),
                    ) {
                        if (!artistName.isNullOrBlank()) {
                            Text(
                                text = artistName,
                                style = MaterialTheme.typography.headlineLarge.copy(
                                    fontSize = 34.sp,
                                    fontWeight = FontWeight.ExtraBold,
                                ),
                                color = Color.White,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        } else {
                            ShimmerBox(Modifier.width(210.dp).height(34.dp), RoundedCornerShape(8.dp))
                        }
                        Spacer(Modifier.height(18.dp))
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            ShimmerBox(Modifier.size(48.dp), CircleShape)
                            ShimmerBox(Modifier.weight(1f).height(48.dp), CircleShape)
                            ShimmerBox(Modifier.size(48.dp), CircleShape)
                        }
                    }
                }
            }

            item(key = "artist_skel_latest") {
                Spacer(Modifier.height(14.dp))
                Column(
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    ShimmerBox(Modifier.width(130.dp).height(20.dp), RoundedCornerShape(6.dp))
                    ShimmerBox(Modifier.fillMaxWidth().height(88.dp), RoundedCornerShape(16.dp))
                }
            }

            item(key = "artist_skel_songs") {
                Spacer(Modifier.height(22.dp))
                Column(
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    ShimmerBox(Modifier.width(110.dp).height(20.dp), RoundedCornerShape(6.dp))
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        repeat(columnCount) {
                            Column(
                                modifier = Modifier.weight(1f),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                repeat(4) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                                    ) {
                                        ShimmerBox(Modifier.size(50.dp), RoundedCornerShape(8.dp))
                                        Column(
                                            modifier = Modifier.weight(1f),
                                            verticalArrangement = Arrangement.spacedBy(5.dp),
                                        ) {
                                            ShimmerBox(Modifier.fillMaxWidth(0.75f).height(14.dp), RoundedCornerShape(4.dp))
                                            ShimmerBox(Modifier.fillMaxWidth(0.45f).height(11.dp), RoundedCornerShape(4.dp))
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
