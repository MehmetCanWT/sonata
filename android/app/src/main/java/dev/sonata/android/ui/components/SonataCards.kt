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
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.backdrop.backdrops.layerBackdrop
import dev.sonata.android.backdrop.backdrops.rememberLayerBackdrop
import dev.sonata.android.sonataThemeGlassButton
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberLayoutBoundsHolder
import dev.sonata.android.ui.theme.LocalLunariaColors
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
    val themeTokens = LocalLunariaColors.current
    val cardSize = if (themeTokens.isMoonflower) 116.dp else 142.dp
    val cornerRadius = if (themeTokens.isMoonflower) 18.dp else 14.dp
    val cardShape = remember(cornerRadius) { RoundedCornerShape(cornerRadius) }
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 360, 360)
    val haptics = LocalHapticFeedback.current
    val cardBounds = rememberLayoutBoundsHolder()

    Column(
        modifier = modifier
            .width(cardSize)
            .onGloballyPositioned { cardBounds.coordinates = it }
            .combinedClickable(
                onClick = onClick,
                onLongClick = if (onLongClick != null) {
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onLongClick(cardBounds.boundsInRoot())
                    }
                } else {
                    null
                },
            ),
    ) {
        Box(
            modifier = Modifier
                .size(cardSize)
                .then(
                    if (themeTokens.isViolet) {
                        Modifier.shadow(
                            elevation = 8.dp,
                            shape = cardShape,
                            clip = false,
                            ambientColor = themeTokens.glassShadowColor,
                            spotColor = themeTokens.glassShadowColor,
                        )
                    } else {
                        Modifier
                    },
                )
                .clip(cardShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(
                    width = if (isCurrent) 1.5.dp else 0.75.dp,
                    color = if (isCurrent) themeTokens.petalLavender else themeTokens.cardStroke,
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
            color = if (isCurrent) themeTokens.petalLavender else MaterialTheme.colorScheme.onBackground,
            maxLines = if (themeTokens.isMoonflower) 1 else 2,
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
    val themeTokens = LocalLunariaColors.current
    val haptics = LocalHapticFeedback.current
    val cardShape = themeTokens.cardShape
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(CARD_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 360, 360)
    val cardBounds = rememberLayoutBoundsHolder()

    Column(
        modifier = modifier
            .width(width)
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
                .then(
                    if (themeTokens.isViolet) {
                        Modifier.shadow(
                            elevation = 8.dp,
                            shape = cardShape,
                            clip = false,
                            ambientColor = themeTokens.glassShadowColor,
                            spotColor = themeTokens.glassShadowColor,
                        )
                    } else {
                        Modifier
                    },
                )
                .clip(cardShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(0.75.dp, themeTokens.cardStroke, cardShape),
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
    val themeTokens = LocalLunariaColors.current
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
                        0f to themeTokens.petalLavender.copy(alpha = 0.16f),
                        1f to MaterialTheme.colorScheme.surfaceVariant,
                    ),
                )
                .border(
                    width = 1.5.dp,
                    brush = Brush.linearGradient(
                        0f to themeTokens.glassRimTopLeft.copy(alpha = 0.50f),
                        1f to themeTokens.petalLavender.copy(alpha = 0.24f),
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
    contentColor: Color = Color.White,
    topInset: Dp = 64.dp,
    onPlayAll: () -> Unit,
    onShuffleAll: () -> Unit,
    onDownloadAll: (() -> Unit)? = null,
    isDownloading: Boolean = false,
    allDownloaded: Boolean = false,
    onEditPlaylist: (() -> Unit)? = null,
    onMoreOptions: (() -> Unit)? = null,
    onOpenArtist: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val themeTokens = LocalLunariaColors.current
    val sizedUrl = remember(artworkUrl) { artworkUrl.artworkAt(HEADER_ART_PX) }
    val request = rememberArtworkRequest(sizedUrl, 800, 800)
    val metaLine = remember(kindLabel, year, trackCount) {
        buildList {
            kindLabel?.takeIf { it.isNotBlank() }?.let { add(it) }
            year?.takeIf { it.isNotBlank() }?.let { add(it) }
            if (trackCount > 0) add("$trackCount tracks")
        }.joinToString(" • ")
    }
    val isOfflineLibrary = kindLabel.equals("Offline Library", ignoreCase = true)
    val pillShape = remember { RoundedCornerShape(percent = 50) }
    val heroBackdrop = rememberLayerBackdrop()
    val buttonContentTint = themeTokens.onSurfacePrimary
    val playPillTint = if (themeTokens.isDark) {
        themeTokens.glassTint.copy(alpha = 0.62f)
    } else {
        Color.White.copy(alpha = 0.78f)
    }
    val playFallbackBg = if (themeTokens.isDark) themeTokens.eclipseCard else Color.White
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val coverSize = if (screenWidthDp >= 600) 236.dp else 216.dp
    val artShape = RoundedCornerShape(if (themeTokens.isMoonflower) 24.dp else 18.dp)

    Box(
        modifier = modifier.fillMaxWidth(),
    ) {
        if (request != null) {
            // Soft blurred ambient artwork aura spanning the full header and fading smoothly to 0% alpha.
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .layerBackdrop(heroBackdrop),
            ) {
                AsyncImage(
                    model = request,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(if (themeTokens.isDark) 0.34f else 0.24f)
                        .graphicsLayer(compositingStrategy = CompositingStrategy.Offscreen)
                        .drawWithCache {
                            val auraMask = Brush.verticalGradient(
                                0.0f to Color.Black,
                                0.30f to Color.Black.copy(alpha = 0.86f),
                                0.58f to Color.Black.copy(alpha = 0.45f),
                                0.78f to Color.Black.copy(alpha = 0.14f),
                                0.92f to Color.Transparent,
                                1.0f to Color.Transparent,
                            )
                            onDrawWithContent {
                                drawContent()
                                drawRect(brush = auraMask, blendMode = BlendMode.DstIn)
                            }
                        }
                        .blur(52.dp, BlurredEdgeTreatment.Unbounded),
                )
            }
        } else {
            // Soft ambient halo backdrop for Offline Library / Downloads and collections without cover art.
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .layerBackdrop(heroBackdrop)
                    .background(
                        Brush.radialGradient(
                            colors = listOf(
                                accentColor.copy(alpha = 0.24f),
                                washColor.copy(alpha = 0.30f),
                                Color.Transparent,
                            ),
                        ),
                    ),
            )
        }

        // Centered non-overlapping Column: cover card strictly below the top bar, above metadata + action pills.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    top = topInset + 10.dp,
                    start = PAGE_GUTTER,
                    end = PAGE_GUTTER,
                    bottom = 22.dp,
                ),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(5.dp),
        ) {
            val placeholderBrush = Brush.linearGradient(
                listOf(
                    themeTokens.eclipseCard,
                    accentColor.copy(alpha = if (themeTokens.isDark) 0.42f else 0.34f),
                ),
            )
            Box(
                modifier = Modifier
                    .size(coverSize)
                    .shadow(
                        elevation = 22.dp,
                        shape = artShape,
                        clip = false,
                        ambientColor = themeTokens.glassShadowColor,
                        spotColor = themeTokens.glassShadowColor,
                    )
                    .clip(artShape)
                    .then(
                        if (request != null) {
                            Modifier.background(washColor)
                        } else {
                            Modifier.background(placeholderBrush)
                        },
                    )
                    .border(0.9.dp, themeTokens.glassRimTopLeft, artShape),
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
                        imageVector = if (isOfflineLibrary) SonataIcons.Download else SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = themeTokens.onSurfacePrimary.copy(alpha = 0.78f),
                        modifier = Modifier.size(56.dp),
                    )
                }
            }
            Spacer(Modifier.height(14.dp))

            Text(
                text = title,
                style = MaterialTheme.typography.headlineMedium.copy(
                    fontSize = 26.sp,
                    fontWeight = FontWeight.Bold,
                    lineHeight = 31.sp,
                ),
                color = contentColor,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )

            if (!subtitle.isNullOrBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.titleMedium.copy(
                        fontSize = 15.5.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = contentColor.copy(alpha = 0.88f),
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = if (onOpenArtist != null) {
                        Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .clickable(onClick = onOpenArtist)
                            .padding(horizontal = 6.dp, vertical = 2.dp)
                    } else {
                        Modifier
                    },
                )
            }

            if (metaLine.isNotBlank()) {
                Text(
                    text = metaLine.uppercase(),
                    style = MaterialTheme.typography.labelSmall.copy(
                        fontSize = 11.5.sp,
                        letterSpacing = 0.8.sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = accentColor.copy(alpha = 0.90f),
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            Spacer(Modifier.height(10.dp))

            // Centered Liquid Glass action row: [Shuffle] [▶ Play] [Download] [⋯]
            CompositionLocalProvider(LocalAppBackdrop provides heroBackdrop) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        modifier = Modifier
                            .size(48.dp)
                            .sonataThemeGlassButton(
                                shape = CircleShape,
                                fallbackColor = themeTokens.eclipseSurface,
                            )
                            .clickable(onClick = onShuffleAll),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Shuffle,
                            contentDescription = "Shuffle",
                            tint = buttonContentTint,
                            modifier = Modifier.size(20.dp),
                        )
                    }

                    Row(
                        modifier = Modifier
                            .height(48.dp)
                            .widthIn(min = 148.dp)
                            .sonataThemeGlassButton(
                                shape = pillShape,
                                surfaceTint = playPillTint,
                                fallbackColor = playFallbackBg,
                                shadowElevation = 12.dp,
                            )
                            .clickable(onClick = onPlayAll)
                            .padding(horizontal = 26.dp),
                        horizontalArrangement = Arrangement.Center,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Play,
                            contentDescription = null,
                            tint = buttonContentTint,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(7.dp))
                        Text(
                            text = "Play",
                            style = MaterialTheme.typography.labelMedium.copy(
                                fontSize = 15.5.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = buttonContentTint,
                        )
                    }

                    if (onDownloadAll != null) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .sonataThemeGlassButton(
                                    shape = CircleShape,
                                    fallbackColor = themeTokens.eclipseSurface,
                                )
                                .clickable(enabled = !isDownloading, onClick = onDownloadAll),
                            contentAlignment = Alignment.Center,
                        ) {
                            when {
                                isDownloading -> CircularProgressIndicator(
                                    modifier = Modifier.size(18.dp),
                                    strokeWidth = 2.dp,
                                    color = buttonContentTint,
                                )
                                allDownloaded -> Icon(
                                    imageVector = SonataIcons.Check,
                                    contentDescription = "All Downloaded",
                                    tint = accentColor,
                                    modifier = Modifier.size(20.dp),
                                )
                                else -> Icon(
                                    imageVector = SonataIcons.Download,
                                    contentDescription = "Download All",
                                    tint = buttonContentTint,
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }
                    }

                    if (onEditPlaylist != null) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .sonataThemeGlassButton(
                                    shape = CircleShape,
                                    fallbackColor = themeTokens.eclipseSurface,
                                )
                                .clickable(onClick = onEditPlaylist),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = SonataIcons.Edit,
                                contentDescription = "Edit playlist",
                                tint = buttonContentTint,
                                modifier = Modifier.size(19.dp),
                            )
                        }
                    }

                    if (onMoreOptions != null) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .sonataThemeGlassButton(
                                    shape = CircleShape,
                                    fallbackColor = themeTokens.eclipseSurface,
                                )
                                .clickable(onClick = onMoreOptions),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = SonataIcons.MoreHoriz,
                                contentDescription = "More options",
                                tint = buttonContentTint,
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}
