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
import androidx.compose.foundation.layout.aspectRatio
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
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
import dev.sonata.android.ui.MoodGenreSleeve
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.TopPickMood
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.components.SonataEditorialCard
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberLayoutBoundsHolder
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidTrack

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
        "station" in t ||
        "karışık" in t ||
        "radyo" in t ||
        "keşfet" in t ||
        "микс" in t ||
        "радио" in t ||
        "открыт" in t
}

private fun canonicalCardId(card: AndroidHomeCard): String =
    card.id.removePrefix("VL").trim().ifBlank { card.title.trim().lowercase() }

private fun primaryArtistLabel(artist: String): String =
    artist.split(",", "&", "•", "feat.", "ft.", "featuring", ignoreCase = true)
        .firstOrNull()
        ?.trim()
        .orEmpty()

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
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
    lastPlayedTracks: List<AndroidTrack> = emptyList(),
    refreshing: Boolean = false,
    onRefresh: () -> Unit = {},
    onStartTrackRadio: (AndroidTrack) -> Unit = { onStartQuickRadio() },
    onStartPersonalStation: () -> Unit = onStartQuickRadio,
    onStartDiscoveryStation: () -> Unit = onStartQuickRadio,
) {
    val themeTokens = LocalLunariaColors.current
    val sourceShelves = remember(radioMixShelves, homeShelves) {
        val raw = if (radioMixShelves.isNotEmpty()) {
            radioMixShelves
        } else {
            homeShelves.filter { shelf ->
                val t = shelf.title.lowercase()
                shelf.cards.isNotEmpty() && (
                    "mix" in t || "radio" in t || "station" in t || "mood" in t ||
                        "energy" in t || "focus" in t || "chill" in t ||
                        "karışık" in t || "radyo" in t || "istasyon" in t || "ruh hali" in t ||
                        "микс" in t || "радио" in t || "станци" in t || "настроен" in t ||
                        shelf.cards.any(::isMixLikeCard)
                    )
            }
        }
        raw.filterNot { shelf ->
            val t = shelf.title.lowercase()
            "forgotten" in t || "unutulan" in t || "забыт" in t
        }
    }

    val featuredMixes = remember(sourceShelves) {
        val seenIds = HashSet<String>()
        val seenTitles = HashSet<String>()
        val preferred = sourceShelves.flatMap { it.cards }
            .filter { (it.kind == AndroidHomeCardKind.PLAYLIST || it.kind == AndroidHomeCardKind.ALBUM) && isMixLikeCard(it) }
        val pool = if (preferred.isNotEmpty()) {
            preferred
        } else {
            sourceShelves.flatMap { it.cards }
                .filter { it.kind == AndroidHomeCardKind.PLAYLIST || it.kind == AndroidHomeCardKind.ALBUM }
        }
        pool.filter { card ->
            val cid = canonicalCardId(card)
            val normTitle = card.title.trim().lowercase()
            seenIds.add(cid) && (normTitle.isBlank() || seenTitles.add(normTitle))
        }.take(6)
    }

    val secondaryMixShelves = remember(sourceShelves, featuredMixes) {
        val usedIds = featuredMixes.mapTo(HashSet()) { canonicalCardId(it) }
        val usedTitles = featuredMixes.mapNotNullTo(HashSet()) {
            it.title.trim().lowercase().takeIf { t -> t.isNotBlank() }
        }
        sourceShelves.mapNotNull { shelf ->
            val uniqueCards = shelf.cards.filter { card ->
                val isCollection = card.kind == AndroidHomeCardKind.PLAYLIST || card.kind == AndroidHomeCardKind.ALBUM
                if (!isCollection) return@filter false
                val cid = canonicalCardId(card)
                val normTitle = card.title.trim().lowercase()
                val freshId = usedIds.add(cid)
                val freshTitle = normTitle.isBlank() || usedTitles.add(normTitle)
                freshId && freshTitle
            }
            if (uniqueCards.isEmpty()) null else shelf.copy(cards = uniqueCards)
        }.take(8)
    }

    val artistRadioSeeds = remember(activeTrack, lastPlayedTracks, homeShelves) {
        val artistScores = HashMap<String, Int>()
        lastPlayedTracks.take(40).forEachIndexed { idx, tr ->
            val key = primaryArtistLabel(tr.artist).lowercase()
            if (key.isNotBlank()) {
                artistScores[key] = (artistScores[key] ?: 0) + (40 - idx).coerceAtLeast(3)
            }
        }
        val shelfTracks = homeShelves.flatMap { shelf ->
            shelf.tracks + shelf.cards.mapNotNull { it.track }
        }
        val weightedHistory = lastPlayedTracks
            .filter { primaryArtistLabel(it.artist).isNotBlank() }
            .sortedByDescending { artistScores[primaryArtistLabel(it.artist).lowercase()] ?: 0 }
        val weightedShelf = shelfTracks.sortedByDescending {
            artistScores[primaryArtistLabel(it.artist).lowercase()] ?: 0
        }
        val seenArtists = HashSet<String>()
        val candidates = buildList {
            activeTrack?.let { add(it) }
            addAll(weightedHistory)
            addAll(weightedShelf)
        }
        candidates.filter { track ->
            val artist = primaryArtistLabel(track.artist)
            !track.thumbnailUrl.isNullOrBlank() &&
                artist.isNotBlank() &&
                seenArtists.add(artist.lowercase())
        }.take(12)
    }

    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val screenWidth = screenWidthDp.dp
    val moodColumns = if (screenWidthDp >= 780) 3 else 2
    val moodRows = remember(moodColumns) { TopPickMood.entries.chunked(moodColumns) }
    val featuredCardWidth = (screenWidth * 0.44f).coerceIn(180.dp, 224.dp)
    val featuredCardHeight = (featuredCardWidth * 1.18f).coerceIn(216.dp, 266.dp)
    val featuredListState = rememberLazyListState()
    val featuredSnapFling = rememberSnapFlingBehavior(lazyListState = featuredListState)

    PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = onRefresh,
        modifier = modifier.fillMaxSize(),
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(
                top = contentPadding.calculateTopPadding() + 8.dp,
                bottom = contentPadding.calculateBottomPadding() + 24.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            // Page heading + personalized quick station triggers
            item(key = "radio_header") {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                Column(
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                ) {
                    Text(
                        text = strings.radioHeading,
                        style = MaterialTheme.typography.headlineLarge.copy(
                            fontSize = 32.sp,
                            fontWeight = FontWeight.ExtraBold,
                            letterSpacing = (-0.5).sp,
                        ),
                        color = MaterialTheme.colorScheme.onBackground,
                    )
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        listOf(
                            strings.myStation to onStartPersonalStation,
                            strings.discoveryStation to onStartDiscoveryStation,
                            strings.radio to onStartQuickRadio,
                        ).forEach { (label, action) ->
                            Box(
                                modifier = Modifier
                                    .weight(1f)
                                    .clip(RoundedCornerShape(14.dp))
                                    .background(themeTokens.eclipseSurface.copy(alpha = 0.72f))
                                    .border(0.75.dp, themeTokens.cardStroke, RoundedCornerShape(14.dp))
                                    .clickable(onClick = action)
                                    .padding(horizontal = 10.dp, vertical = 10.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    text = label,
                                    style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                    color = MaterialTheme.colorScheme.onBackground,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }

            // "Featured Mixes" album-cover cards (never duplicates items in secondary shelves)
            if (featuredMixes.isNotEmpty()) {
                item(key = "your_mixes_section") {
                    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(
                            text = strings.featuredMixes,
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 20.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                        )
                        LazyRow(
                            state = featuredListState,
                            flingBehavior = featuredSnapFling,
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            itemsIndexed(
                                items = featuredMixes,
                                key = { idx, card -> "featured_${idx}_${card.id}" },
                            ) { _, card ->
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
                    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    Column(
                        modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(
                            text = strings.featuredStation,
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 20.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = themeTokens.onSurfacePrimary,
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

            // Deduplicated secondary mix shelves ("Your Mixes", "Mood Mixes", etc.)
            itemsIndexed(
                items = secondaryMixShelves,
                key = { idx, shelf -> "mix_shelf_${idx}_${shelf.title}" },
            ) { shelfIdx, shelf ->
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
                            text = dev.sonata.android.ui.i18n.localizeShelfTitle(
                                shelf.title,
                                dev.sonata.android.ui.i18n.LocalSonataStrings.current,
                            ),
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 20.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
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

                    LazyRow(
                        contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        itemsIndexed(
                            items = shelf.cards,
                            key = { cardIdx, card -> "${shelfIdx}_${cardIdx}_${card.id}" },
                        ) { _, card ->
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

            // Personalized Artist & Track Stations built from listener history & feed
            if (artistRadioSeeds.isNotEmpty()) {
                item(key = "artist_track_stations") {
                    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(
                            text = strings.artistAndTrackStations,
                            style = MaterialTheme.typography.titleLarge.copy(
                                fontSize = 20.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                        )
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            itemsIndexed(
                                items = artistRadioSeeds,
                                key = { idx, tr -> "station_${idx}_${tr.id}" },
                            ) { _, tr ->
                                val artistName = primaryArtistLabel(tr.artist)
                                SonataEditorialCard(
                                    title = strings.artistRadioTitle(artistName),
                                    subtitle = strings.basedOnTrack(tr.title),
                                    artworkUrl = tr.thumbnailUrl,
                                    onClick = { onStartTrackRadio(tr) },
                                    width = 148.dp,
                                )
                            }
                        }
                    }
                }
            }

            // Mood & Genre Radio Stations (animated theme-matched sleeves filling out the Radio page)
            item(key = "mood_radio_stations_header") {
                Text(
                    text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.moodAndGenreStations,
                    style = MaterialTheme.typography.titleLarge.copy(
                        fontSize = 20.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                )
            }

            itemsIndexed(
                items = moodRows,
                key = { rowIdx, row -> "radio_mood_row_${rowIdx}_${row.firstOrNull()?.name.orEmpty()}" },
            ) { _, rowItems ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = PAGE_GUTTER),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    rowItems.forEach { mood ->
                        val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                        MoodGenreSleeve(
                            title = dev.sonata.android.ui.i18n.localizeMoodTitle(mood.displayName, strings),
                            modifier = Modifier
                                .weight(1f)
                                .aspectRatio(1.9f)
                                .clip(RoundedCornerShape(18.dp))
                                .clickable { onMoodQueryClick(mood.displayName, "${mood.displayName} radio") },
                        )
                    }
                    repeat(moodColumns - rowItems.size) {
                        Spacer(Modifier.weight(1f))
                    }
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
    val themeTokens = LocalLunariaColors.current
    val haptics = LocalHapticFeedback.current
    val cardBounds = rememberLayoutBoundsHolder()
    val coverReq = rememberArtworkRequest(
        url = card.thumbnailUrl.artworkAt(HEADER_ART_PX),
        widthPx = HEADER_ART_PX,
        heightPx = HEADER_ART_PX,
    )
    val shape = themeTokens.cardShape

    Box(
        modifier = Modifier
            .width(width)
            .height(height)
            .clip(shape)
            .background(themeTokens.eclipseCard)
            .border(0.75.dp, themeTokens.cardStroke, shape)
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
                    tint = themeTokens.onSurfaceSecondary.copy(alpha = 0.55f),
                    modifier = Modifier.size(44.dp),
                )
            }
        }

        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    dev.sonata.android.ui.smoothScrimBrush(
                        from = themeTokens.heroCardScrimColor.copy(alpha = 0f),
                        to = themeTokens.heroCardScrimColor.copy(alpha = 0.94f),
                        startFraction = 0.12f,
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
    val themeTokens = LocalLunariaColors.current
    val coverReq = rememberArtworkRequest(
        url = track.thumbnailUrl.artworkAt(CARD_ART_PX),
        widthPx = CARD_ART_PX,
        heightPx = CARD_ART_PX,
    )
    val shape = themeTokens.cardShape

    Box(
        modifier = Modifier
            .width(width)
            .height(height)
            .clip(shape)
            .background(themeTokens.eclipseCard)
            .border(0.75.dp, themeTokens.cardStroke, shape)
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
                    dev.sonata.android.ui.smoothScrimBrush(
                        from = themeTokens.heroCardScrimColor.copy(alpha = 0f),
                        to = themeTokens.heroCardScrimColor.copy(alpha = 0.92f),
                        startFraction = 0.12f,
                    ),
                ),
        )
        val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = strings.artistRadioTitle(track.title),
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = strings.basedOnTrack(track.artist),
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                color = Color.White.copy(alpha = 0.76f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
