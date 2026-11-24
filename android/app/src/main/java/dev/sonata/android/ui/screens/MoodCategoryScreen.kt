/*
 * Curated Mood & Genre category browse screen displaying YTM mood/genre shelves.
 */
package dev.sonata.android.ui.screens

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.unit.dp
import dev.sonata.android.CompactTrackRow
import dev.sonata.android.SectionHeader
import dev.sonata.android.ShelfCard
import dev.sonata.android.TrackScrollActivity
import dev.sonata.android.VideoShelfCard
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.SonataShimmerHost
import dev.sonata.android.ui.components.SonataArtistCircleCard
import dev.sonata.android.ui.components.SonataEditorialCard
import dev.sonata.android.ui.feedSkeleton
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.localizeMoodTitle
import dev.sonata.android.ui.i18n.localizeShelfTitle
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidTrack

@Composable
internal fun MoodCategoryScreen(
    title: String,
    shelves: List<AndroidHomeShelf>,
    loading: Boolean,
    activeTrackId: String?,
    isPlaying: Boolean,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState(),
    onOpenCard: (AndroidHomeCard, List<AndroidTrack>) -> Unit,
    onCardLongClick: (AndroidHomeCard, String?, Boolean, Rect?) -> Unit = { _, _, _, _ -> },
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onTrackOptions: (AndroidTrack, Rect?) -> Unit = { _, _ -> },
    onTrackLongClick: (AndroidTrack, Int?, Rect?) -> Unit = { _, _, _ -> },
) {
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val showSkeleton = loading && shelves.isEmpty()

    TrackScrollActivity(listState)

    Crossfade(
        targetState = showSkeleton,
        animationSpec = tween(durationMillis = 240, easing = FastOutSlowInEasing),
        label = "moodCategorySkeletonCrossfade",
        modifier = modifier.fillMaxSize(),
    ) { isLoadingSkeleton ->
        if (isLoadingSkeleton) {
            SonataShimmerHost {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = contentPadding,
                    userScrollEnabled = false,
                ) {
                    feedSkeleton(sections = 3)
                }
            }
        } else if (shelves.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(contentPadding),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER),
                ) {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = tokens.onSurfaceSecondary.copy(alpha = 0.65f),
                        modifier = Modifier.size(40.dp),
                    )
                    Text(
                        text = strings.emptyCollection,
                        style = MaterialTheme.typography.bodyMedium,
                        color = tokens.onSurfaceSecondary,
                    )
                }
            }
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = contentPadding,
            ) {
                itemsIndexed(
                    items = shelves,
                    key = { idx, shelf -> "mood_shelf_${idx}_${shelf.title}" },
                    contentType = { _, _ -> "mood_shelf" },
                ) { _, shelf ->
                    val cards = remember(shelf.cards) {
                        shelf.cards.distinctBy { "${it.kind}_${it.id}_${it.title}" }
                    }
                    val tracks = remember(shelf.tracks) {
                        shelf.tracks.distinctBy { it.id }
                    }
                    if (cards.isNotEmpty() || tracks.isNotEmpty()) {
                        Column(Modifier.padding(bottom = 24.dp)) {
                            val headerTitle = if (shelf.title.isNotBlank()) {
                                localizeShelfTitle(shelf.title, strings)
                            } else {
                                localizeMoodTitle(title, strings)
                            }
                            SectionHeader(title = headerTitle)

                            if (cards.isNotEmpty()) {
                                val isVideoShelf = remember(cards) {
                                    cards.all { it.kind == AndroidHomeCardKind.VIDEO }
                                }
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = PAGE_GUTTER),
                                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                                ) {
                                    items(
                                        items = cards,
                                        key = { card -> "mc_${card.kind}_${card.id}_${card.title}" },
                                        contentType = { card ->
                                            when {
                                                isVideoShelf -> "mood_video_card"
                                                card.kind == AndroidHomeCardKind.ARTIST -> "mood_artist_card"
                                                else -> "mood_editorial_card"
                                            }
                                        },
                                    ) { card ->
                                        val current = activeTrackId != null && card.track?.id == activeTrackId
                                        when {
                                            isVideoShelf -> VideoShelfCard(
                                                card = card,
                                                isCurrent = current,
                                                isPlaying = current && isPlaying,
                                                onClick = { onOpenCard(card, tracks) },
                                                onLongClick = { bounds -> onCardLongClick(card, "VIDEO", false, bounds) },
                                            )
                                            card.kind == AndroidHomeCardKind.ARTIST -> SonataArtistCircleCard(
                                                name = card.title,
                                                artworkUrl = card.thumbnailUrl,
                                                onClick = { onOpenCard(card, tracks) },
                                            )
                                            card.kind == AndroidHomeCardKind.SONG -> ShelfCard(
                                                card = card,
                                                isCurrent = current,
                                                isPlaying = current && isPlaying,
                                                onClick = { onOpenCard(card, tracks) },
                                                onLongClick = { bounds -> onCardLongClick(card, null, false, bounds) },
                                            )
                                            else -> SonataEditorialCard(
                                                title = card.title,
                                                subtitle = card.subtitle,
                                                artworkUrl = card.thumbnailUrl,
                                                badgeLabel = when (card.kind) {
                                                    AndroidHomeCardKind.ALBUM -> strings.albumLabel
                                                    AndroidHomeCardKind.PLAYLIST -> strings.playlistLabel
                                                    else -> null
                                                },
                                                onClick = { onOpenCard(card, tracks) },
                                                onPlayClick = { onOpenCard(card, tracks) },
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

                            if (tracks.isNotEmpty()) {
                                if (cards.isNotEmpty()) {
                                    Spacer(Modifier.height(10.dp))
                                }
                                Column(modifier = Modifier.padding(horizontal = PAGE_GUTTER)) {
                                    tracks.forEachIndexed { idx, track ->
                                        val current = activeTrackId != null && track.id == activeTrackId
                                        CompactTrackRow(
                                            track = track,
                                            isCurrent = current,
                                            isPlaying = current && isPlaying,
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
                }
            }
        }
    }
}
