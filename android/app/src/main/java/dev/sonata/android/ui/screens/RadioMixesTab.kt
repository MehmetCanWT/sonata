/*
 * Dedicated Radio & Mixes tab screen for Sonata Android.
 *
 * Uses proportional album-cover cards for personalized YouTube Music mixes
 * ("Your Mixes") and horizontal mix shelves sourced strictly from real YTM
 * mix shelves (never user playlists or synthetic gradient boxes), with zero
 * play-button overlays on artwork cards.
 */
package dev.sonata.android.ui.screens

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.components.SonataEditorialCard
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidTrack

private val MixCardFallbackBg = Color(0xFF1C1C20)

private fun isMixLikeCard(card: AndroidHomeCard): Boolean {
    val id = card.id
    val t = "${card.title} ${card.subtitle}".lowercase()
    return id.startsWith("RD") ||
        id.startsWith("VLRD") ||
        "mix" in t ||
        "radio" in t ||
        "supermix" in t ||
        "discover" in t ||
        "replay" in t ||
        "station" in t
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun RadioMixesScreen(
    homeShelves: List<AndroidHomeShelf>,
    activeTrack: AndroidTrack?,
    contentPadding: PaddingValues,
    onStartQuickRadio: () -> Unit,
    onCardClick: (AndroidHomeCard) -> Unit,
    onCardLongClick: (AndroidHomeCard, Rect?) -> Unit = { _, _ -> },
    onMoodQueryClick: (String, String) -> Unit,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState(),
    radioMixShelves: List<AndroidHomeShelf> = emptyList(),
) {
    val sourceShelves = remember(radioMixShelves, homeShelves) {
        val raw = if (radioMixShelves.isNotEmpty()) {
            radioMixShelves
        } else {
            homeShelves.filter { shelf ->
                val t = shelf.title.lowercase()
                shelf.cards.isNotEmpty() && (
                    "mix" in t || "radio" in t || "station" in t || "mood" in t ||
                        "energy" in t || "focus" in t || "chill" in t ||
                        shelf.cards.any(::isMixLikeCard)
                    )
            }
        }
        raw.filterNot { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" in t || "unutulan" in t
        }
    }

    val featuredMixes = remember(sourceShelves) {
        val mixCards = sourceShelves.flatMap { it.cards }
            .filter { (it.kind == AndroidHomeCardKind.PLAYLIST || it.kind == AndroidHomeCardKind.ALBUM) && isMixLikeCard(it) }
            .distinctBy { it.id }
        if (mixCards.isNotEmpty()) {
            mixCards.take(10)
        } else {
            sourceShelves.flatMap { it.cards }
                .filter { it.kind == AndroidHomeCardKind.PLAYLIST || it.kind == AndroidHomeCardKind.ALBUM }
                .distinctBy { it.id }
                .take(10)
        }
    }

    val secondaryMixShelves = remember(sourceShelves) {
        sourceShelves.filterNot { shelf ->
            val t = shelf.title.trim().lowercase()
            t == "your mixes" || t == "featured mixes"
        }.take(8)
    }

    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    val featuredCardWidth = (screenWidth * 0.44f).coerceIn(180.dp, 224.dp)
    val featuredCardHeight = (featuredCardWidth * 1.18f).coerceIn(216.dp, 266.dp)
    val featuredListState = rememberLazyListState()
    val featuredSnapFling = rememberSnapFlingBehavior(lazyListState = featuredListState)

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            top = contentPadding.calculateTopPadding() + 8.dp,
            bottom = contentPadding.calculateBottomPadding() + 24.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        // Page heading
        item(key = "radio_header") {
            Text(
                text = "Radio",
                style = MaterialTheme.typography.headlineLarge.copy(
                    fontSize = 32.sp,
                    fontWeight = FontWeight.ExtraBold,
                    letterSpacing = (-0.5).sp,
                ),
                color = Color.White,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER),
            )
        }

        // "Featured Mixes" album-cover cards (never duplicates secondary "Your Mixes" shelf header)
        if (featuredMixes.isNotEmpty()) {
            item(key = "your_mixes_section") {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        text = "Featured Mixes",
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontSize = 20.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = Color.White,
                        modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                    )
                    LazyRow(
                        state = featuredListState,
                        flingBehavior = featuredSnapFling,
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        items(
                            items = featuredMixes,
                            key = { "featured_${it.id}" },
                        ) { card ->
                            FeaturedMixCoverCard(
                                card = card,
                                width = featuredCardWidth,
                                height = featuredCardHeight,
                                onClick = { onCardClick(card) },
                                onLongClick = { bounds -> onCardLongClick(card, bounds) },
                            )
                        }
                    }
                }
            }
        } else if (activeTrack != null) {
            item(key = "active_track_station") {
                Column(
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = "Your Mixes",
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontSize = 20.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = Color.White,
                    )
                    FeaturedTrackRadioCoverCard(
                        track = activeTrack,
                        width = featuredCardWidth,
                        height = featuredCardHeight,
                        onClick = onStartQuickRadio,
                    )
                }
            }
        }

        // Personalized per-mood / mix horizontal shelves (proportional 148.dp square covers, NO play button)
        items(
            items = secondaryMixShelves,
            key = { "mix_shelf_${it.title}" },
        ) { shelf ->
            val shelfCards = remember(shelf.cards) {
                shelf.cards
                    .filter { it.kind == AndroidHomeCardKind.PLAYLIST || it.kind == AndroidHomeCardKind.ALBUM }
                    .distinctBy { it.id }
            }
            if (shelfCards.isNotEmpty()) {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onMoodQueryClick(shelf.title, shelf.title) }
                            .padding(horizontal = PAGE_GUTTER),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            text = shelf.title,
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 20.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.55f),
                            modifier = Modifier.size(18.dp),
                        )
                    }

                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        items(
                            items = shelfCards,
                            key = { "${shelf.title}_${it.id}" },
                        ) { card ->
                            SonataEditorialCard(
                                title = card.title,
                                subtitle = card.subtitle,
                                artworkUrl = card.thumbnailUrl,
                                onClick = { onCardClick(card) },
                                onLongClick = { bounds -> onCardLongClick(card, bounds) },
                                width = 148.dp,
                            )
                        }
                    }
                }
            }
        }

        if (featuredMixes.isEmpty() && secondaryMixShelves.isEmpty() && activeTrack == null) {
            item(key = "radio_empty_state") {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER, vertical = 36.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        imageVector = SonataRadioWavesIcon,
                        contentDescription = null,
                        tint = Color.White.copy(alpha = 0.48f),
                        modifier = Modifier.size(40.dp),
                    )
                    Text(
                        text = "Your personalized mixes will appear here once loaded",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.White.copy(alpha = 0.65f),
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun FeaturedMixCoverCard(
    card: AndroidHomeCard,
    width: androidx.compose.ui.unit.Dp,
    height: androidx.compose.ui.unit.Dp,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
) {
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }
    val coverReq = rememberArtworkRequest(
        url = card.thumbnailUrl.artworkAt(HEADER_ART_PX),
        widthPx = HEADER_ART_PX,
        heightPx = HEADER_ART_PX,
    )
    val shape = RoundedCornerShape(16.dp)

    Box(
        modifier = Modifier
            .width(width)
            .height(height)
            .clip(shape)
            .background(MixCardFallbackBg)
            .border(0.5.dp, Color.White.copy(alpha = 0.10f), shape)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { callback ->
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        callback(cardBounds)
                    }
                },
            ),
    ) {
        if (coverReq != null) {
            AsyncImage(
                model = coverReq,
                contentDescription = card.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataRadioWavesIcon,
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.35f),
                    modifier = Modifier.size(44.dp),
                )
            }
        }

        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        0.0f to Color.Transparent,
                        0.48f to Color.Black.copy(alpha = 0.16f),
                        0.74f to Color.Black.copy(alpha = 0.60f),
                        1.0f to Color.Black.copy(alpha = 0.90f),
                    ),
                ),
        )

        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = card.title,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (card.subtitle.isNotBlank()) {
                Text(
                    text = card.subtitle,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                    color = Color.White.copy(alpha = 0.76f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun FeaturedTrackRadioCoverCard(
    track: AndroidTrack,
    width: androidx.compose.ui.unit.Dp,
    height: androidx.compose.ui.unit.Dp,
    onClick: () -> Unit,
) {
    val coverReq = rememberArtworkRequest(
        url = track.thumbnailUrl.artworkAt(CARD_ART_PX),
        widthPx = CARD_ART_PX,
        heightPx = CARD_ART_PX,
    )
    val shape = RoundedCornerShape(16.dp)

    Box(
        modifier = Modifier
            .width(width)
            .height(height)
            .clip(shape)
            .background(MixCardFallbackBg)
            .border(0.5.dp, Color.White.copy(alpha = 0.10f), shape)
            .clickable(onClick = onClick),
    ) {
        if (coverReq != null) {
            AsyncImage(
                model = coverReq,
                contentDescription = track.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        0.0f to Color.Transparent,
                        0.50f to Color.Black.copy(alpha = 0.25f),
                        1.0f to Color.Black.copy(alpha = 0.90f),
                    ),
                ),
        )
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = "${track.title} Radio",
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = "Based on ${track.artist}",
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                color = Color.White.copy(alpha = 0.76f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
