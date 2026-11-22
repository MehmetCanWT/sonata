/*
 * Sonata-distinct editorial card components and asymmetric Split-Hero detail header.
 */
package dev.sonata.android.ui.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalHapticFeedback
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
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.sonataGlassSurface
import uniffi.sonata_android.AndroidTrack

/**
 * Two-column frosted glass quick-resume strip for recent picks and jump-back-in items.
 */
@Composable
fun SonataQuickResumeGrid(
    tracks: List<AndroidTrack>,
    onTrackClick: (AndroidTrack) -> Unit,
    modifier: Modifier = Modifier,
) {
    val visible = remember(tracks) { tracks.take(6) }
    if (visible.isEmpty()) return
    val rows = remember(visible) { visible.chunked(2) }
    val tileShape = remember { RoundedCornerShape(14.dp) }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = PAGE_GUTTER),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        rows.forEach { pair ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                pair.forEach { track ->
                    val artUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
                    val request = rememberArtworkRequest(artUrl, ROW_ART_PX, ROW_ART_PX)
                    Row(
                        modifier = Modifier
                            .weight(1f)
                            .height(56.dp)
                            .sonataGlassSurface(shape = tileShape)
                            .clip(tileShape)
                            .clickable { onTrackClick(track) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(
                            modifier = Modifier
                                .size(56.dp)
                                .background(MaterialTheme.colorScheme.surfaceVariant),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (request != null) {
                                AsyncImage(
                                    model = request,
                                    contentDescription = track.title,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.MusicNote,
                                    contentDescription = null,
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .padding(horizontal = 10.dp),
                            verticalArrangement = Arrangement.Center,
                        ) {
                            Text(
                                text = track.title,
                                style = MaterialTheme.typography.labelMedium.copy(
                                    fontSize = 13.sp,
                                    fontWeight = FontWeight.SemiBold,
                                ),
                                color = MaterialTheme.colorScheme.onSurface,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            if (track.artist.isNotBlank()) {
                                Text(
                                    text = track.artist,
                                    style = MaterialTheme.typography.labelSmall.copy(
                                        fontSize = 11.sp,
                                        fontWeight = FontWeight.Normal,
                                    ),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
                if (pair.size == 1) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

/**
 * Clean 148.dp square cover + title card for Home's "Last Played" horizontal strip.
 * Supports long-press to open [TrackOptionsMenu] with haptic feedback.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SonataLastPlayedSquareCard(
    title: String,
    artworkUrl: String?,
    isCurrent: Boolean = false,
    onClick: () -> Unit,
    onLongClick: ((Rect?) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val cardShape = remember { RoundedCornerShape(12.dp) }
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 360, 360)
    val haptics = LocalHapticFeedback.current
    var cardBounds by remember { mutableStateOf<Rect?>(null) }

    Column(
        modifier = modifier
            .width(148.dp)
            .onGloballyPositioned { cardBounds = it.boundsInRoot() }
            .combinedClickable(
                onClick = onClick,
                onLongClick = if (onLongClick != null) {
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onLongClick(cardBounds)
                    }
                } else {
                    null
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .size(148.dp)
                .clip(cardShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(
                    width = if (isCurrent) 1.5.dp else 0.5.dp,
                    color = if (isCurrent) SonataAccent else Color.White.copy(alpha = 0.08f),
                    shape = cardShape,
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (request != null) {
                AsyncImage(
                    model = request,
                    contentDescription = title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(32.dp),
                )
            }
        }
        Spacer(Modifier.height(7.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 13.5.sp,
                lineHeight = 17.sp,
            ),
            color = if (isCurrent) SonataAccent else MaterialTheme.colorScheme.onBackground,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * Clean square card for albums, mixes, and playlists (proportional 148.dp size, zero play-button overlay).
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SonataEditorialCard(
    title: String,
    subtitle: String,
    artworkUrl: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    @Suppress("UNUSED_PARAMETER") badgeLabel: String? = null,
    @Suppress("UNUSED_PARAMETER") onPlayClick: (() -> Unit)? = null,
    onLongClick: ((Rect?) -> Unit)? = null,
    width: Dp = 148.dp,
) {
    val haptics = LocalHapticFeedback.current
    val cardShape = remember { RoundedCornerShape(12.dp) }
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 360, 360)
    var cardBounds by remember { mutableStateOf<Rect?>(null) }

    Column(
        modifier = modifier
            .width(width)
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
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(cardShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.5.dp, Color.White.copy(alpha = 0.08f), cardShape),
        ) {
            if (request != null) {
                AsyncImage(
                    model = request,
                    contentDescription = title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .size(34.dp)
                        .align(Alignment.Center),
                )
            }
        }

        Spacer(Modifier.height(7.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 13.5.sp,
                lineHeight = 17.sp,
            ),
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.fillMaxWidth(),
        )
        if (subtitle.isNotBlank()) {
            Spacer(Modifier.height(2.dp))
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * Circular artist portrait card with subtle radial halo ring and centered caption.
 */
@Composable
fun SonataArtistCircleCard(
    name: String,
    artworkUrl: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    diameter: Dp = 124.dp,
) {
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 320, 320)

    Column(
        modifier = modifier
            .width(diameter + 8.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(diameter)
                .clip(CircleShape)
                .background(
                    Brush.radialGradient(
                        0f to SonataAccent.copy(alpha = 0.16f),
                        1f to MaterialTheme.colorScheme.surfaceVariant,
                    ),
                )
                .border(
                    width = 1.5.dp,
                    brush = Brush.linearGradient(
                        0f to Color.White.copy(alpha = 0.28f),
                        1f to SonataAccent.copy(alpha = 0.22f),
                    ),
                    shape = CircleShape,
                )
                .padding(3.dp)
                .clip(CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (request != null) {
                AsyncImage(
                    model = request,
                    contentDescription = name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.Person,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(34.dp),
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            text = name,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 13.sp,
            ),
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
    }
}

/**
 * Asymmetric editorial Split-Hero header for Album / Playlist / Collection detail pages:
 * soft-shadowed cover on the left, typography + pill actions on the right, resting on
 * an artwork-tinted wash.
 */
@Composable
fun SonataSplitHeroHeader(
    title: String,
    subtitle: String?,
    kindLabel: String?,
    year: String?,
    trackCount: Int,
    artworkUrl: String?,
    accentColor: Color,
    washColor: Color,
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
    onDownloadAll: (() -> Unit)? = null,
    isDownloading: Boolean = false,
    allDownloaded: Boolean = false,
    onMoreOptions: (() -> Unit)? = null,
    onOpenArtist: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val coverShape = remember { RoundedCornerShape(18.dp) }
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 360, 360)
    val metaLine = remember(kindLabel, year, trackCount) {
        buildList {
            kindLabel?.takeIf { it.isNotBlank() }?.let { add(it) }
            year?.takeIf { it.isNotBlank() }?.let { add(it) }
            if (trackCount > 0) add("$trackCount tracks")
        }.joinToString(" · ")
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(
                Brush.verticalGradient(
                    0f to washColor.copy(alpha = 0.85f),
                    1f to Color.Transparent,
                ),
            )
            .padding(horizontal = PAGE_GUTTER, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(134.dp)
                    .shadow(18.dp, coverShape, ambientColor = Color.Black, spotColor = Color.Black)
                    .clip(coverShape)
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .border(0.5.dp, Color.White.copy(alpha = 0.12f), coverShape),
                contentAlignment = Alignment.Center,
            ) {
                if (request != null) {
                    AsyncImage(
                        model = request,
                        contentDescription = title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(40.dp),
                    )
                }
            }

            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (metaLine.isNotBlank()) {
                    Text(
                        text = metaLine.uppercase(),
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            letterSpacing = 0.8.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = accentColor,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    text = title,
                    style = MaterialTheme.typography.headlineMedium.copy(
                        fontSize = 22.sp,
                        fontWeight = FontWeight.Bold,
                        lineHeight = 26.sp,
                    ),
                    color = MaterialTheme.colorScheme.onBackground,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                if (!subtitle.isNullOrBlank()) {
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodyMedium.copy(fontSize = 13.sp),
                        color = if (onOpenArtist != null) accentColor else MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        modifier = if (onOpenArtist != null) {
                            Modifier
                                .clip(RoundedCornerShape(6.dp))
                                .clickable(onClick = onOpenArtist)
                        } else {
                            Modifier
                        },
                    )
                }
            }
        }

        // Action pill strip
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val pillShape = remember { RoundedCornerShape(percent = 50) }
            Row(
                modifier = Modifier
                    .weight(1f)
                    .height(44.dp)
                    .clip(pillShape)
                    .background(accentColor)
                    .clickable(onClick = onPlayAll),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = SonataIcons.Play,
                    contentDescription = null,
                    tint = Color.Black,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = "Play",
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = Color.Black,
                )
            }

            Row(
                modifier = Modifier
                    .weight(1f)
                    .height(44.dp)
                    .sonataGlassSurface(shape = pillShape)
                    .clip(pillShape)
                    .clickable(onClick = onShuffleAll),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = SonataIcons.Shuffle,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = "Shuffle",
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }

            if (onDownloadAll != null) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .sonataGlassSurface(shape = CircleShape)
                        .clip(CircleShape)
                        .clickable(enabled = !isDownloading, onClick = onDownloadAll),
                    contentAlignment = Alignment.Center,
                ) {
                    when {
                        isDownloading -> CircularProgressIndicator(
                            modifier = Modifier.size(18.dp),
                            strokeWidth = 2.dp,
                            color = accentColor,
                        )
                        allDownloaded -> Icon(
                            imageVector = SonataIcons.Check,
                            contentDescription = "All Downloaded",
                            tint = accentColor,
                            modifier = Modifier.size(18.dp),
                        )
                        else -> Icon(
                            imageVector = SonataIcons.Download,
                            contentDescription = "Download All",
                            tint = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }

            if (onMoreOptions != null) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .sonataGlassSurface(shape = CircleShape)
                        .clip(CircleShape)
                        .clickable(onClick = onMoreOptions),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.MoreHoriz,
                        contentDescription = "More options",
                        tint = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }
    }
}
