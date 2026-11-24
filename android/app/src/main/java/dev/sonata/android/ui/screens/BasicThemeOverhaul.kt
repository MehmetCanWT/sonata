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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import dev.sonata.android.ui.components.SonataEditorialCard
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
import uniffi.sonata_android.AndroidMoodCategory

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
    accentColor: Color? = null,
    titleStyle: TextStyle = MaterialTheme.typography.titleMedium,
) {
    val tone = remember(title, accentColor) { buildBasicMoodTone(title, accentColor) }
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
                    dev.sonata.android.ui.smoothScrimBrush(
                        from = Color.Black.copy(alpha = 0.24f),
                        to = Color.Black.copy(alpha = 0f),
                        endFraction = 0.62f,
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
    moodCategories: List<AndroidMoodCategory> = emptyList(),
    onOpenMoodCategory: (String, String, String) -> Unit = { title, _, _ -> onOpenMood(title) },
    personalizedHeroCards: List<AndroidHomeCard> = emptyList(),
    onOpenCard: (AndroidHomeCard) -> Unit = {},
    onStartDiscoveryStation: () -> Unit = {},
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
            val effectiveMoods = remember(moodCategories, moodCards) {
                if (moodCategories.isNotEmpty()) {
                    moodCategories
                } else {
                    moodCards.map { card ->
                        AndroidMoodCategory(
                            title = card.title,
                            browseId = if (card.id.startsWith("FEmood")) card.id else "",
                            params = "",
                            accentColor = 0u,
                        )
                    }
                }
            }
            val rows = remember(effectiveMoods, columns) { effectiveMoods.chunked(columns) }

            LazyColumn(
                state = listState,
                contentPadding = contentPadding,
                modifier = Modifier.fillMaxSize(),
            ) {
                item(key = "basic_explore_title", contentType = "basic_title") {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(
                                start = PAGE_GUTTER,
                                end = PAGE_GUTTER,
                                top = 8.dp,
                                bottom = 14.dp,
                            ),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.exploreHeading,
                            style = MaterialTheme.typography.displayLarge.copy(
                                fontSize = 34.sp,
                                fontWeight = FontWeight.ExtraBold,
                                letterSpacing = (-0.5).sp,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                        )
                        val discoverLabel = dev.sonata.android.ui.i18n.LocalSonataStrings.current.discoveryStation
                        val pillFill = if (LocalLunariaColors.current.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(50))
                                .background(pillFill)
                                .clickable(onClick = onStartDiscoveryStation)
                                .padding(horizontal = 12.dp, vertical = 7.dp),
                        ) {
                            Text(
                                text = discoverLabel,
                                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                                color = MaterialTheme.colorScheme.onBackground,
                                maxLines = 1,
                            )
                        }
                    }
                }
                if (personalizedHeroCards.isNotEmpty()) {
                    item(key = "basic_explore_picks", contentType = "basic_explore_picks") {
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            modifier = Modifier.padding(bottom = 18.dp),
                        ) {
                            items(
                                items = personalizedHeroCards,
                                key = { c -> "basic_pick_${c.kind}_${c.id}" },
                            ) { card ->
                                SonataEditorialCard(
                                    title = card.title,
                                    subtitle = card.subtitle,
                                    artworkUrl = card.thumbnailUrl,
                                    onClick = { onOpenCard(card) },
                                    width = 148.dp,
                                )
                            }
                        }
                    }
                }
                items(
                    items = rows,
                    key = { row -> "basic_mood_row_${row.first().browseId}_${row.first().params}_${row.first().title}" },
                    contentType = { "basic_mood_row" },
                ) { row ->
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(BasicMoodSpacing),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = PAGE_GUTTER)
                            .padding(bottom = BasicMoodSpacing),
                    ) {
                        row.forEachIndexed { idx, cat ->
                            val art = artPool.getOrNull(
                                ((cat.title.hashCode() and Int.MAX_VALUE) + idx) % artPool.size.coerceAtLeast(1),
                            )
                            val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                            val catAccent = cat.accentColor.takeIf { it != 0u }?.let { Color(it.toInt()) }
                            BasicMoodSleeveCard(
                                title = dev.sonata.android.ui.i18n.localizeMoodTitle(cat.title, strings),
                                artworkUrl = art,
                                accentColor = catAccent,
                                onClick = { onOpenMoodCategory(cat.title, cat.browseId, cat.params) },
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
    isPinned: Boolean = false,
    onTogglePin: (() -> Unit)? = null,
    isSavedToLibrary: Boolean = false,
    onToggleSaveToLibrary: (() -> Unit)? = null,
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
                            dev.sonata.android.ui.smoothScrimBrush(
                                from = Color.Black.copy(alpha = 0.45f),
                                to = Color.Black.copy(alpha = 0f),
                            ),
                        ),
                )
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .fillMaxHeight(0.55f)
                        .align(Alignment.BottomCenter)
                        .background(
                            dev.sonata.android.ui.smoothScrimBrush(
                                from = bgColor.copy(alpha = 0f),
                                to = bgColor,
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
                if (trackCount > 0) dev.sonata.android.ui.i18n.LocalSonataStrings.current.tracksCount(trackCount) else null,
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

            // Release Action Row: [Shuffle] [Play Pill weight(1f)] [Download] [Edit/+] [Pin]
            val sideCount = 1 +
                (if (onDownloadAll != null) 1 else 0) +
                (if (onEditPlaylist != null || onToggleSaveToLibrary != null) 1 else 0) +
                (if (onTogglePin != null) 1 else 0)
            val buttonHeight = BasicReleaseSideButton
            val sideButtonSize = if (sideCount >= 4) 44.dp else BasicReleaseSideButton
            val rowGap = if (sideCount >= 4) 8.dp else BasicReleaseActionGap
            Row(
                modifier = Modifier
                    .widthIn(max = BasicReleaseActionsMaxWidth)
                    .fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(rowGap),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                Box(
                    modifier = Modifier
                        .size(width = sideButtonSize, height = buttonHeight)
                        .clip(BasicActionPillShape)
                        .background(secondaryFill)
                        .clickable(onClick = onShuffleAll),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.Shuffle,
                        contentDescription = strings.shuffle,
                        tint = primaryText,
                        modifier = Modifier.size(20.dp),
                    )
                }

                Row(
                    modifier = Modifier
                        .weight(1f)
                        .height(buttonHeight)
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
                        modifier = Modifier.size(19.dp),
                    )
                    Spacer(Modifier.width(7.dp))
                    Text(
                        text = strings.play,
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                        color = primaryContent,
                        maxLines = 1,
                    )
                }

                if (onDownloadAll != null) {
                    Box(
                        modifier = Modifier
                            .size(width = sideButtonSize, height = buttonHeight)
                            .clip(BasicActionPillShape)
                            .background(secondaryFill)
                            .clickable(enabled = !isDownloading, onClick = onDownloadAll),
                        contentAlignment = Alignment.Center,
                    ) {
                        when {
                            isDownloading -> CircularProgressIndicator(
                                color = primaryText,
                                strokeWidth = 2.dp,
                                modifier = Modifier.size(19.dp),
                            )
                            allDownloaded -> Icon(
                                imageVector = SonataIcons.Check,
                                contentDescription = strings.downloaded,
                                tint = Color(0xFF34C759),
                                modifier = Modifier.size(20.dp),
                            )
                            else -> Icon(
                                imageVector = SonataIcons.Download,
                                contentDescription = strings.downloadAll,
                                tint = primaryText,
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }

                if (onEditPlaylist != null) {
                    Box(
                        modifier = Modifier
                            .size(width = sideButtonSize, height = buttonHeight)
                            .clip(BasicActionPillShape)
                            .background(secondaryFill)
                            .clickable(onClick = onEditPlaylist),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Edit,
                            contentDescription = strings.editPlaylist,
                            tint = primaryText,
                            modifier = Modifier.size(19.dp),
                        )
                    }
                } else if (onToggleSaveToLibrary != null) {
                    Box(
                        modifier = Modifier
                            .size(width = sideButtonSize, height = buttonHeight)
                            .clip(BasicActionPillShape)
                            .background(if (isSavedToLibrary) primaryFill else secondaryFill)
                            .clickable(onClick = onToggleSaveToLibrary),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = if (isSavedToLibrary) SonataIcons.Check else SonataIcons.Add,
                            contentDescription = if (isSavedToLibrary) strings.removeFromLibrary else strings.addToLibrary,
                            tint = if (isSavedToLibrary) primaryContent else primaryText,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }

                if (onTogglePin != null) {
                    Box(
                        modifier = Modifier
                            .size(width = sideButtonSize, height = buttonHeight)
                            .clip(BasicActionPillShape)
                            .background(if (isPinned) primaryFill else secondaryFill)
                            .clickable(onClick = onTogglePin),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = if (isPinned) SonataIcons.PinFilled else SonataIcons.Pin,
                            contentDescription = if (isPinned) strings.unpinFromLibrary else strings.pinToLibrary,
                            tint = if (isPinned) primaryContent else primaryText,
                            modifier = Modifier.size(19.dp),
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
                        dev.sonata.android.ui.smoothScrimBrush(
                            from = Color.Black.copy(alpha = 0.45f),
                            to = Color.Black.copy(alpha = 0f),
                        ),
                    ),
            )
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .fillMaxHeight(0.55f)
                    .align(Alignment.BottomCenter)
                    .background(
                        dev.sonata.android.ui.smoothScrimBrush(
                            from = bgColor.copy(alpha = 0f),
                            to = bgColor,
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

private fun buildBasicMoodTone(title: String, accentColor: Color? = null): BasicMoodTone {
    val base = accentColor ?: when ((title.hashCode() and Int.MAX_VALUE) % 8) {
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
