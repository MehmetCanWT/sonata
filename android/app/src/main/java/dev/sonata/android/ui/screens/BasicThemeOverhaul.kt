/*
 * Monochrome Editorial Theme-Specific UI Overhaul ("Basic" Theme) for Sonata Android.
 *
 * All layouts in this file are rendered ONLY when DesignTheme.BASIC is active,
 * preserving Violet and Moonflower screens completely untouched.
 *
 * - Home display title & Find Your Mood duotone sleeves
 * - Explore screen (big "Explore" heading + responsive 2/3/4-col duotone mood grid)
 * - Release / Album / Playlist detail header (centered framed sleeve or full-bleed cover + Shuffle / Play / Download bar)
 * - Artist profile header (full-bleed fading photo + 52dp / 70dp / 52dp action circles)
 */
package dev.sonata.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind

// ---- Basic theme geometry constants -----------------------------------------

private val BasicMoodCardShape = RoundedCornerShape(12.dp)
private const val BasicMoodCardAspect = 1.72f
private const val BasicMoodStripeFraction = 0.09f
private val BasicMoodSpacing = 12.dp

private val BasicSleeveShape = RoundedCornerShape(12.dp)
private val BasicActionPillShape = RoundedCornerShape(12.dp)
private val BasicReleaseSideButton = 50.dp
private val BasicReleaseActionGap = 14.dp
private val BasicReleaseActionsMaxWidth = 420.dp

private val BasicArtistPlayButton = 70.dp
private val BasicArtistSideButton = 52.dp
private val BasicArtistActionGap = 24.dp

/**
 * Large Home display heading ("Home") rendered at top of Home feed in Basic theme.
 */
@Composable
fun BasicHomeTitle(
    title: String = dev.sonata.android.ui.i18n.LocalSonataStrings.current.homeHeading,
    modifier: Modifier = Modifier,
) {
    Text(
        text = title,
        style = MaterialTheme.typography.displayLarge.copy(
            fontSize = 34.sp,
            fontWeight = FontWeight.ExtraBold,
            letterSpacing = (-0.5).sp,
        ),
        color = MaterialTheme.colorScheme.onBackground,
        modifier = modifier.padding(
            start = PAGE_GUTTER,
            end = PAGE_GUTTER,
            top = 8.dp,
            bottom = 10.dp,
        ),
    )
}

/**
 * Duotone mood/genre sleeve card:
 * A solid vertical accent stripe on the left + a duotone `ColorMatrix`-mapped
 * cover sleeve on the right + soft top shade + bold white title anchored top-start.
 */
@Composable
fun BasicMoodSleeveCard(
    title: String,
    artworkUrl: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    titleStyle: TextStyle = MaterialTheme.typography.titleMedium,
) {
    val tone = remember(title) { buildBasicMoodTone(title) }
    val sizedUrl = remember(artworkUrl) { artworkUrl?.artworkAt(CARD_ART_PX) }
    val req = rememberArtworkRequest(sizedUrl, 360, 220)

    Box(
        modifier = modifier
            .clip(BasicMoodCardShape)
            .background(tone.stripe)
            .clickable(onClick = onClick),
    ) {
        Box(
            Modifier
                .align(Alignment.CenterEnd)
                .fillMaxHeight()
                .fillMaxWidth(1f - BasicMoodStripeFraction)
                .background(tone.placeholder),
        ) {
            if (req != null) {
                AsyncImage(
                    model = req,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    colorFilter = tone.duotone,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        Box(
            Modifier
                .matchParentSize()
                .background(
                    Brush.verticalGradient(
                        0f to Color.Black.copy(alpha = 0.24f),
                        0.62f to Color.Transparent,
                    ),
                ),
        )
        Text(
            text = title,
            style = titleStyle.copy(
                shadow = Shadow(
                    color = Color.Black.copy(alpha = 0.35f),
                    offset = Offset(0f, 1f),
                    blurRadius = 6f,
                ),
            ),
            fontWeight = FontWeight.Bold,
            color = Color.White,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .align(Alignment.TopStart)
                .padding(horizontal = 14.dp, vertical = 12.dp),
        )
    }
}

/**
 * Monochrome Basic Explore screen (large "Explore" heading + responsive 2/3/4-column
 * grid of duotone mood sleeve cards).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BasicExploreScreen(
    moodCards: List<AndroidHomeCard>,
    artPool: List<String>,
    listState: LazyListState,
    refreshing: Boolean,
    onRefresh: () -> Unit,
    onOpenMood: (String) -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = onRefresh,
        modifier = modifier.fillMaxSize(),
    ) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val columns = when {
                maxWidth >= 900.dp -> 4
                maxWidth >= 600.dp -> 3
                else -> 2
            }
            val rows = remember(moodCards, columns) { moodCards.chunked(columns) }

            LazyColumn(
                state = listState,
                contentPadding = contentPadding,
                modifier = Modifier.fillMaxSize(),
            ) {
                item(key = "basic_explore_title", contentType = "basic_title") {
                    Text(
                        text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.exploreHeading,
                        style = MaterialTheme.typography.displayLarge.copy(
                            fontSize = 34.sp,
                            fontWeight = FontWeight.ExtraBold,
                            letterSpacing = (-0.5).sp,
                        ),
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.padding(
                            start = PAGE_GUTTER,
                            end = PAGE_GUTTER,
                            top = 8.dp,
                            bottom = 14.dp,
                        ),
                    )
                }
                items(
                    items = rows,
                    key = { row -> "basic_mood_row_${row.first().id}_${row.first().title}" },
                    contentType = { "basic_mood_row" },
                ) { row ->
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(BasicMoodSpacing),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER)
                            .padding(bottom = BasicMoodSpacing),
                    ) {
                        row.forEachIndexed { idx, card ->
                            val art = card.thumbnailUrl?.takeIf { it.isNotBlank() }
                                ?: artPool.getOrNull(
                                    ((card.title.hashCode() and Int.MAX_VALUE) + idx) % artPool.size.coerceAtLeast(1),
                                )
                            BasicMoodSleeveCard(
                                title = card.title,
                                artworkUrl = art,
                                onClick = { onOpenMood(card.title) },
                                modifier = Modifier
                                    .weight(1f)
                                    .aspectRatio(BasicMoodCardAspect),
                            )
                        }
                        repeat(columns - row.size) {
                            Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        }
    }
}

/**
 * Release / Album / Playlist Detail Header for Basic theme:
 * Centered framed square sleeve (or optional full-bleed dissolving cover when
 * [fullScreenDetailCover] is enabled), centered bold title + clickable artist + metadata,
 * and centered 50dp Shuffle / Play Pill / 50dp Download row.
 */
@Composable
fun BasicDetailHeroHeader(
    title: String,
    subtitle: String?,
    kindLabel: String?,
    year: String?,
    trackCount: Int,
    artworkUrl: String?,
    topInset: Dp,
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
    onDownloadAll: (() -> Unit)?,
    isDownloading: Boolean,
    allDownloaded: Boolean,
    onEditPlaylist: (() -> Unit)? = null,
    onOpenArtist: (() -> Unit)?,
    fullScreenDetailCover: Boolean = false,
) {
    val tokens = LocalLunariaColors.current
    val config = LocalConfiguration.current
    val isLandscape = config.screenWidthDp > config.screenHeightDp
    val sleeveSize = if (isLandscape) {
        (config.screenHeightDp * 0.36f).coerceIn(170f, 230f).dp
    } else {
        (config.screenWidthDp * 0.64f).coerceIn(200f, 280f).dp
    }
    val heroArtUrl = remember(artworkUrl) { artworkUrl?.artworkAt(HEADER_ART_PX) }
    val heroReq = rememberArtworkRequest(heroArtUrl, 768, 768)

    val bgColor = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val primaryFill = if (tokens.isDark) Color.White else Color.Black
    val primaryContent = if (tokens.isDark) Color.Black else Color.White
    val secondaryFill = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(
                top = if (fullScreenDetailCover) 0.dp else topInset + 12.dp,
                bottom = 18.dp,
            ),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (fullScreenDetailCover) {
            val bleedHeight = if (isLandscape) {
                (config.screenHeightDp * 0.46f).coerceIn(220f, 320f).dp
            } else {
                (config.screenWidthDp * 0.86f).coerceIn(280f, 430f).dp
            }
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(bleedHeight + topInset),
            ) {
                if (heroReq != null) {
                    AsyncImage(
                        model = heroReq,
                        contentDescription = title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(secondaryFill),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = secondaryText,
                            modifier = Modifier.size(64.dp),
                        )
                    }
                }
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(topInset + 44.dp)
                        .align(Alignment.TopCenter)
                        .background(
                            Brush.verticalGradient(
                                listOf(Color.Black.copy(alpha = 0.45f), Color.Transparent),
                            ),
                        ),
                )
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .fillMaxHeight(0.55f)
                        .align(Alignment.BottomCenter)
                        .background(
                            Brush.verticalGradient(
                                0f to Color.Transparent,
                                0.68f to bgColor.copy(alpha = 0.84f),
                                1f to bgColor,
                            ),
                        ),
                )
            }
        } else {
            // Framed square sleeve with soft elevation shadow
            Box(
                modifier = Modifier
                    .size(sleeveSize)
                    .shadow(elevation = 18.dp, shape = BasicSleeveShape, clip = false)
                    .clip(BasicSleeveShape)
                    .background(secondaryFill)
                    .border(0.5.dp, tokens.cardStroke, BasicSleeveShape),
                contentAlignment = Alignment.Center,
            ) {
                if (heroReq != null) {
                    AsyncImage(
                        model = heroReq,
                        contentDescription = title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = secondaryText,
                        modifier = Modifier.size(56.dp),
                    )
                }
            }
            Spacer(Modifier.height(18.dp))
        }

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = PAGE_GUTTER),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.headlineMedium.copy(
                    fontSize = 23.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = (-0.25).sp,
                ),
                color = primaryText,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )

            if (!subtitle.isNullOrBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.titleMedium.copy(fontSize = 16.sp),
                    color = if (onOpenArtist != null) BasicPalette.AccentRed else secondaryText,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = if (onOpenArtist != null) Modifier.clickable(onClick = onOpenArtist) else Modifier,
                )
            }

            val metaParts = listOfNotNull(
                kindLabel?.takeIf { it.isNotBlank() },
                year?.takeIf { it.isNotBlank() },
                if (trackCount > 0) "$trackCount ${if (trackCount == 1) "song" else "songs"}" else null,
            )
            if (metaParts.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    text = metaParts.joinToString(" · "),
                    style = MaterialTheme.typography.labelMedium,
                    color = secondaryText,
                    textAlign = TextAlign.Center,
                )
            }

            Spacer(Modifier.height(18.dp))

            // Release Action Row: [Shuffle 50dp] [Play Pill weight(1f)] [Download 50dp]
            Row(
                modifier = Modifier
                    .widthIn(max = BasicReleaseActionsMaxWidth)
                    .fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(BasicReleaseActionGap),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(BasicReleaseSideButton)
                        .clip(BasicActionPillShape)
                        .background(secondaryFill)
                        .clickable(onClick = onShuffleAll),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.Shuffle,
                        contentDescription = "Shuffle",
                        tint = primaryText,
                        modifier = Modifier.size(21.dp),
                    )
                }

                Row(
                    modifier = Modifier
                        .weight(1f)
                        .height(BasicReleaseSideButton)
                        .clip(BasicActionPillShape)
                        .background(primaryFill)
                        .clickable(onClick = onPlayAll),
                    horizontalArrangement = Arrangement.Center,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        imageVector = SonataIcons.Play,
                        contentDescription = null,
                        tint = primaryContent,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = "Play",
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                        color = primaryContent,
                    )
                }

                if (onDownloadAll != null) {
                    Box(
                        modifier = Modifier
                            .size(BasicReleaseSideButton)
                            .clip(BasicActionPillShape)
                            .background(secondaryFill)
                            .clickable(enabled = !isDownloading, onClick = onDownloadAll),
                        contentAlignment = Alignment.Center,
                    ) {
                        when {
                            isDownloading -> CircularProgressIndicator(
                                color = primaryText,
                                strokeWidth = 2.dp,
                                modifier = Modifier.size(20.dp),
                            )
                            allDownloaded -> Icon(
                                imageVector = SonataIcons.Check,
                                contentDescription = "Downloaded",
                                tint = Color(0xFF34C759),
                                modifier = Modifier.size(21.dp),
                            )
                            else -> Icon(
                                imageVector = SonataIcons.Download,
                                contentDescription = "Download all",
                                tint = primaryText,
                                modifier = Modifier.size(21.dp),
                            )
                        }
                    }
                }

                if (onEditPlaylist != null) {
                    Box(
                        modifier = Modifier
                            .size(BasicReleaseSideButton)
                            .clip(BasicActionPillShape)
                            .background(secondaryFill)
                            .clickable(onClick = onEditPlaylist),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Edit,
                            contentDescription = "Edit playlist",
                            tint = primaryText,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
            }
        }
    }
}

/**
 * Artist Profile Hero for Basic theme:
 * Full-bleed edge-to-edge artist portrait dissolving cleanly into pure black/white,
 * centered bold artist name, and 52dp / 70dp / 52dp circular action row.
 */
@Composable
fun BasicArtistHeroHeader(
    artist: ArtistScreenData,
    heroUrl: String?,
    statusTop: Dp,
    onPlay: () -> Unit,
    onShuffle: () -> Unit,
    onToggleSubscribe: () -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val config = LocalConfiguration.current
    val isLandscape = config.screenWidthDp > config.screenHeightDp
    val photoHeight = if (isLandscape) {
        (config.screenHeightDp * 0.46f).coerceIn(220f, 320f).dp
    } else {
        (config.screenWidthDp * 0.88f).coerceIn(280f, 440f).dp
    }
    val bgColor = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val primaryFill = if (tokens.isDark) Color.White else Color.Black
    val primaryContent = if (tokens.isDark) Color.Black else Color.White
    val secondaryFill = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant
    val heroReq = rememberArtworkRequest(heroUrl?.artworkAt(HEADER_ART_PX), 900, 900)

    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(photoHeight + statusTop),
        ) {
            if (heroReq != null) {
                AsyncImage(
                    model = heroReq,
                    contentDescription = artist.name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(secondaryFill),
                )
            }
            // Top subtle status-bar scrim + bottom dissolve into pure Basic canvas
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(statusTop + 44.dp)
                    .align(Alignment.TopCenter)
                    .background(
                        Brush.verticalGradient(
                            listOf(Color.Black.copy(alpha = 0.45f), Color.Transparent),
                        ),
                    ),
            )
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .fillMaxHeight(0.55f)
                    .align(Alignment.BottomCenter)
                    .background(
                        Brush.verticalGradient(
                            0f to Color.Transparent,
                            0.65f to bgColor.copy(alpha = 0.82f),
                            1f to bgColor,
                        ),
                    ),
            )
            Column(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = artist.name,
                    style = MaterialTheme.typography.displayLarge.copy(
                        fontSize = 34.sp,
                        fontWeight = FontWeight.ExtraBold,
                        letterSpacing = (-0.5).sp,
                    ),
                    color = primaryText,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val caption = artist.monthlyListeners ?: artist.subscriberText
                if (!caption.isNullOrBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = caption,
                        style = MaterialTheme.typography.bodyMedium,
                        color = secondaryText,
                    )
                }
            }
        }

        Spacer(Modifier.height(14.dp))

        // Artist Action Row: [Shuffle 52dp] [Play 70dp] [Subscribe/Star 52dp]
        Row(
            horizontalArrangement = Arrangement.spacedBy(BasicArtistActionGap),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(BasicArtistSideButton)
                    .clip(CircleShape)
                    .background(secondaryFill)
                    .clickable(onClick = onShuffle),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataIcons.Shuffle,
                    contentDescription = "Shuffle",
                    tint = primaryText,
                    modifier = Modifier.size(22.dp),
                )
            }

            Box(
                modifier = Modifier
                    .size(BasicArtistPlayButton)
                    .clip(CircleShape)
                    .background(primaryFill)
                    .clickable(onClick = onPlay),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataIcons.Play,
                    contentDescription = "Play",
                    tint = primaryContent,
                    modifier = Modifier.size(34.dp),
                )
            }

            Box(
                modifier = Modifier
                    .size(BasicArtistSideButton)
                    .clip(CircleShape)
                    .background(if (artist.isSubscribed) BasicPalette.AccentRed else secondaryFill)
                    .clickable(onClick = onToggleSubscribe),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = if (artist.isSubscribed) SonataIcons.StarFilled else SonataIcons.Star,
                    contentDescription = if (artist.isSubscribed) "Following" else "Follow",
                    tint = if (artist.isSubscribed) Color.White else primaryText,
                    modifier = Modifier.size(22.dp),
                )
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}

// ---- Duotone Color Matrix ---------------------------------------------------

private class BasicMoodTone(
    val stripe: Color,
    val placeholder: Color,
    val duotone: ColorFilter,
)

private fun buildBasicMoodTone(title: String): BasicMoodTone {
    val base = when ((title.hashCode() and Int.MAX_VALUE) % 8) {
        0 -> Color(0xFFCC6A55)
        1 -> Color(0xFFC07A92)
        2 -> Color(0xFF9C8AC0)
        3 -> Color(0xFF8090C8)
        4 -> Color(0xFFD0A060)
        5 -> Color(0xFF6A88B0)
        6 -> Color(0xFF68A088)
        else -> Color(0xFFB5786A)
    }
    val stripe = lerp(base, Color.Black, 0.08f)
    val shadow = lerp(base, Color.Black, 0.68f)
    val highlight = lerp(base, Color.White, 0.32f)
    return BasicMoodTone(
        stripe = stripe,
        placeholder = lerp(shadow, highlight, 0.45f),
        duotone = basicDuotoneFilter(shadow, highlight),
    )
}

private fun basicDuotoneFilter(shadow: Color, highlight: Color): ColorFilter {
    fun channel(from: Float, to: Float): FloatArray {
        val span = to - from
        return floatArrayOf(span * 0.299f, span * 0.587f, span * 0.114f, 0f, from * 255f)
    }
    return ColorFilter.colorMatrix(
        ColorMatrix(
            channel(shadow.red, highlight.red) +
                channel(shadow.green, highlight.green) +
                channel(shadow.blue, highlight.blue) +
                floatArrayOf(0f, 0f, 0f, 1f, 0f),
        ),
    )
}
