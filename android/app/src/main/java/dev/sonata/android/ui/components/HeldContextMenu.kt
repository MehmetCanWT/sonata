/*
 * iOS-inspired Held Long-Press Context Menu for Sonata Android.
 *
 * Displays a lifted preview card (track row pill, numbered album track pill,
 * square album/playlist card, or tall Top Picks card) paired with an English-only
 * action menu over a softly blurred full-screen backdrop.
 */
package dev.sonata.android.ui.components

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.backdrop.drawBackdrop
import dev.sonata.android.backdrop.effects.blur
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import dev.sonata.android.ui.theme.sonataSpecularRim
import uniffi.sonata_android.AndroidTrack
import kotlin.math.roundToInt

@Immutable
sealed interface HeldContextTarget {
    val anchorBounds: Rect?

    data class TrackRow(
        val track: AndroidTrack,
        val trackNumber: Int? = null,
        val durationText: String? = null,
        val isExplicit: Boolean = false,
        override val anchorBounds: Rect? = null,
    ) : HeldContextTarget

    data class AlbumOrPlaylistCard(
        val browseId: String,
        val title: String,
        val subtitle: String,
        val coverUrl: String?,
        val isAlbum: Boolean,
        val eyebrow: String? = null,
        val isTopPickTall: Boolean = false,
        override val anchorBounds: Rect? = null,
    ) : HeldContextTarget
}

/**
 * Full-screen iOS-style held context menu rendered as a sibling overlay outside
 * [appBackdrop] / [artistBackdrop] to avoid RenderNode cycles.
 */
@Composable
fun HeldContextMenu(
    target: HeldContextTarget?,
    isLiked: Boolean = false,
    isDisliked: Boolean = false,
    isDownloaded: Boolean = false,
    activeDownload: ActiveDownload? = null,
    backdrop: Backdrop? = null,
    onDismiss: () -> Unit,
    // Track actions
    onToggleLike: (AndroidTrack) -> Unit = {},
    onToggleDislike: (AndroidTrack) -> Unit = {},
    onShareTrack: (AndroidTrack) -> Unit = {},
    onAddTrackToPlaylist: (AndroidTrack) -> Unit = {},
    onStartRadio: (AndroidTrack) -> Unit = {},
    onPlayTrackNext: (AndroidTrack) -> Unit = {},
    onAddTrackToQueue: (AndroidTrack) -> Unit = {},
    onDownloadTrack: (AndroidTrack) -> Unit = {},
    onRemoveFromCache: (AndroidTrack) -> Unit = {},
    onOpenAlbum: (AndroidTrack) -> Unit = {},
    onOpenArtist: (AndroidTrack) -> Unit = {},
    onSongInfo: (AndroidTrack) -> Unit = {},
    // Album / Playlist actions
    onPlayCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onShuffleCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onAddCollectionToPlaylist: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onPlayCollectionNext: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onAddCollectionToQueue: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onDownloadCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onOpenCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    onShareCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit = {},
    isCollectionPinned: Boolean = false,
    isCollectionSavedToLibrary: Boolean = false,
    canSaveCollectionToLibrary: Boolean = false,
    onTogglePinCollection: ((HeldContextTarget.AlbumOrPlaylistCard) -> Unit)? = null,
    onToggleSaveCollectionToLibrary: ((HeldContextTarget.AlbumOrPlaylistCard) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    if (target != null) {
        BackHandler(onBack = onDismiss)
    }

    AnimatedVisibility(
        visible = target != null,
        enter = fadeIn() + scaleIn(
            initialScale = 0.94f,
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioNoBouncy,
                stiffness = Spring.StiffnessMediumLow,
            ),
            transformOrigin = TransformOrigin(0.5f, 0.5f),
        ),
        exit = fadeOut() + scaleOut(
            targetScale = 0.94f,
            transformOrigin = TransformOrigin(0.5f, 0.5f),
        ),
        modifier = modifier.fillMaxSize(),
    ) {
        val activeTarget = target ?: return@AnimatedVisibility
        val themeTokens = LocalLunariaColors.current
        val targetBackdrop = backdrop ?: LocalAppBackdrop.current
        val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
        val density = LocalDensity.current
        val outsideInteraction = remember { MutableInteractionSource() }
        val overlayDim = when {
            themeTokens.isDark -> Color(0x9C06060A)
            themeTokens.isBasic -> Color(0x52000000)
            else -> Color(0x521B142B)
        }
        val overlayFallback = themeTokens.eclipseBase.copy(alpha = 0.90f)

        val blurOverlayModifier = if (glassOn) {
            val blurPx = with(density) { 26.dp.toPx() } * 0.20f
            Modifier
                .fillMaxSize()
                .drawBackdrop(
                    backdrop = targetBackdrop,
                    shape = { RectangleShape },
                    effects = { blur(blurPx) },
                    highlight = null,
                    shadow = null,
                    onDrawSurface = { drawRect(overlayDim) },
                    backdropScale = 0.20f,
                )
        } else {
            Modifier
                .fillMaxSize()
                .background(overlayFallback)
        }

        BoxWithConstraints(
            modifier = blurOverlayModifier
                .clickable(
                    interactionSource = outsideInteraction,
                    indication = null,
                    onClick = onDismiss,
                )
                .statusBarsPadding()
                .navigationBarsPadding(),
        ) {
            val maxWidthPx = constraints.maxWidth.toFloat().coerceAtLeast(1f)
            val maxHeightPx = constraints.maxHeight.toFloat().coerceAtLeast(1f)
            val anchor = activeTarget.anchorBounds
            val menuAbovePreview = anchor != null && anchor.center.y > maxHeightPx * 0.52f
            val alignEnd = activeTarget is HeldContextTarget.AlbumOrPlaylistCard &&
                anchor != null &&
                anchor.center.x > maxWidthPx * 0.58f
            val columnAlignment = if (alignEnd) Alignment.End else Alignment.Start

            Column(
                modifier = Modifier
                    .align(Alignment.Center)
                    .fillMaxWidth()
                    .widthIn(max = 556.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 28.dp, vertical = 38.dp),
                horizontalAlignment = columnAlignment,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (menuAbovePreview) {
                    HeldActionMenuCard(
                        target = activeTarget,
                        isLiked = isLiked,
                        isDisliked = isDisliked,
                        isDownloaded = isDownloaded,
                        activeDownload = activeDownload,
                        onDismiss = onDismiss,
                        onToggleLike = onToggleLike,
                        onToggleDislike = onToggleDislike,
                        onShareTrack = onShareTrack,
                        onAddTrackToPlaylist = onAddTrackToPlaylist,
                        onStartRadio = onStartRadio,
                        onPlayTrackNext = onPlayTrackNext,
                        onAddTrackToQueue = onAddTrackToQueue,
                        onDownloadTrack = onDownloadTrack,
                        onRemoveFromCache = onRemoveFromCache,
                        onOpenAlbum = onOpenAlbum,
                        onOpenArtist = onOpenArtist,
                        onSongInfo = onSongInfo,
                        onPlayCollection = onPlayCollection,
                        onShuffleCollection = onShuffleCollection,
                        onAddCollectionToPlaylist = onAddCollectionToPlaylist,
                        onPlayCollectionNext = onPlayCollectionNext,
                        onAddCollectionToQueue = onAddCollectionToQueue,
                        onDownloadCollection = onDownloadCollection,
                        onOpenCollection = onOpenCollection,
                        onShareCollection = onShareCollection,
                        isCollectionPinned = isCollectionPinned,
                        isCollectionSavedToLibrary = isCollectionSavedToLibrary,
                        canSaveCollectionToLibrary = canSaveCollectionToLibrary,
                        onTogglePinCollection = onTogglePinCollection,
                        onToggleSaveCollectionToLibrary = onToggleSaveCollectionToLibrary,
                    )
                    HeldLiftedPreviewCard(target = activeTarget)
                } else {
                    HeldLiftedPreviewCard(target = activeTarget)
                    HeldActionMenuCard(
                        target = activeTarget,
                        isLiked = isLiked,
                        isDisliked = isDisliked,
                        isDownloaded = isDownloaded,
                        activeDownload = activeDownload,
                        onDismiss = onDismiss,
                        onToggleLike = onToggleLike,
                        onToggleDislike = onToggleDislike,
                        onShareTrack = onShareTrack,
                        onAddTrackToPlaylist = onAddTrackToPlaylist,
                        onStartRadio = onStartRadio,
                        onPlayTrackNext = onPlayTrackNext,
                        onAddTrackToQueue = onAddTrackToQueue,
                        onDownloadTrack = onDownloadTrack,
                        onRemoveFromCache = onRemoveFromCache,
                        onOpenAlbum = onOpenAlbum,
                        onOpenArtist = onOpenArtist,
                        onSongInfo = onSongInfo,
                        onPlayCollection = onPlayCollection,
                        onShuffleCollection = onShuffleCollection,
                        onAddCollectionToPlaylist = onAddCollectionToPlaylist,
                        onPlayCollectionNext = onPlayCollectionNext,
                        onAddCollectionToQueue = onAddCollectionToQueue,
                        onDownloadCollection = onDownloadCollection,
                        onOpenCollection = onOpenCollection,
                        onShareCollection = onShareCollection,
                        isCollectionPinned = isCollectionPinned,
                        isCollectionSavedToLibrary = isCollectionSavedToLibrary,
                        canSaveCollectionToLibrary = canSaveCollectionToLibrary,
                        onTogglePinCollection = onTogglePinCollection,
                        onToggleSaveCollectionToLibrary = onToggleSaveCollectionToLibrary,
                    )
                }
            }
        }
    }
}

@Composable
private fun HeldLiftedPreviewCard(target: HeldContextTarget) {
    val themeTokens = LocalLunariaColors.current
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val previewShape = remember(themeTokens.isViolet) {
        RoundedCornerShape(if (themeTokens.isViolet) 16.dp else 22.dp)
    }
    val cardInteraction = remember { MutableInteractionSource() }
    val blockClickModifier = Modifier.clickable(
        interactionSource = cardInteraction,
        indication = null,
        onClick = {},
    )
    val surfaceModifier = if (glassOn) {
        Modifier
            .shadow(
                elevation = 20.dp,
                shape = previewShape,
                clip = false,
                ambientColor = themeTokens.glassShadowColor,
                spotColor = themeTokens.glassShadowColor,
            )
            .liquidGlass(shape = previewShape, surfaceTint = themeTokens.glassTint)
            .sonataSpecularRim(shape = previewShape, strokeWidth = 0.75.dp)
    } else {
        Modifier
            .shadow(
                elevation = 20.dp,
                shape = previewShape,
                clip = false,
                ambientColor = themeTokens.glassShadowColor,
                spotColor = themeTokens.glassShadowColor,
            )
            .clip(previewShape)
            .background(themeTokens.eclipseSurface.copy(alpha = 0.96f), previewShape)
            .border(0.75.dp, themeTokens.cardStroke, previewShape)
    }

    when (target) {
        is HeldContextTarget.TrackRow -> {
            val track = target.track
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .then(surfaceModifier)
                    .then(blockClickModifier)
                    .padding(horizontal = 14.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val thumbUrl = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
                val thumbRequest = rememberArtworkRequest(thumbUrl, ROW_ART_PX, ROW_ART_PX)
                Box(
                    modifier = Modifier
                        .size(46.dp)
                        .clip(RoundedCornerShape(9.dp))
                        .background(themeTokens.eclipseCard)
                        .border(0.5.dp, themeTokens.cardStroke, RoundedCornerShape(9.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    if (thumbRequest != null) {
                        AsyncImage(
                            model = thumbRequest,
                            contentDescription = track.title,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = themeTokens.onSurfaceSecondary,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))

                Column(modifier = Modifier.weight(1f)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        if (target.isExplicit) {
                            Box(
                                modifier = Modifier
                                    .size(14.dp)
                                    .clip(RoundedCornerShape(3.dp))
                                    .background(themeTokens.onSurfaceSecondary.copy(alpha = 0.24f)),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    text = "E",
                                    style = MaterialTheme.typography.labelSmall.copy(
                                        fontSize = 9.sp,
                                        fontWeight = FontWeight.Bold,
                                    ),
                                    color = themeTokens.onSurfacePrimary,
                                )
                            }
                        }
                        Text(
                            text = track.title,
                            style = MaterialTheme.typography.titleMedium.copy(
                                fontSize = 15.5.sp,
                                fontWeight = FontWeight.SemiBold,
                            ),
                            color = themeTokens.onSurfacePrimary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (track.artist.isNotBlank()) {
                        Spacer(Modifier.height(1.dp))
                        Text(
                            text = track.artist,
                            style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
                            color = themeTokens.onSurfaceSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }

                if (!target.durationText.isNullOrBlank()) {
                    Spacer(Modifier.width(10.dp))
                    Text(
                        text = target.durationText,
                        style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                        color = themeTokens.onSurfaceSecondary,
                    )
                }
            }
        }

        is HeldContextTarget.AlbumOrPlaylistCard -> {
            val coverUrl = remember(target.coverUrl) { target.coverUrl.artworkAt(CARD_ART_PX) }
            val coverReq = rememberArtworkRequest(coverUrl, 360, 360)
            if (target.isTopPickTall) {
                Box(
                    modifier = Modifier
                        .width(216.dp)
                        .height(288.dp)
                        .then(surfaceModifier)
                        .then(blockClickModifier),
                ) {
                    if (coverReq != null) {
                        AsyncImage(
                            model = coverReq,
                            contentDescription = target.title,
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
                                    to = themeTokens.heroCardScrimColor.copy(alpha = 0.94f),
                                    startFraction = 0.42f,
                                ),
                            ),
                    )
                    Column(
                        modifier = Modifier
                            .align(Alignment.BottomStart)
                            .fillMaxWidth()
                            .padding(14.dp),
                    ) {
                        target.eyebrow?.takeIf { it.isNotBlank() }?.let { eye ->
                            Text(
                                text = eye,
                                style = MaterialTheme.typography.labelSmall.copy(
                                    fontSize = 11.5.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                                color = Color.White.copy(alpha = 0.74f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Text(
                            text = target.title,
                            style = MaterialTheme.typography.titleMedium.copy(
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            color = Color.White,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (target.subtitle.isNotBlank()) {
                            Text(
                                text = target.subtitle,
                                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                                color = Color.White.copy(alpha = 0.78f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .width(166.dp)
                        .then(surfaceModifier)
                        .then(blockClickModifier)
                        .padding(10.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .aspectRatio(1f)
                            .clip(RoundedCornerShape(12.dp))
                            .background(themeTokens.eclipseCard),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (coverReq != null) {
                            AsyncImage(
                                model = coverReq,
                                contentDescription = target.title,
                                contentScale = ContentScale.Crop,
                                modifier = Modifier.fillMaxSize(),
                            )
                        } else {
                            Icon(
                                imageVector = SonataIcons.MusicNote,
                                contentDescription = null,
                                tint = themeTokens.onSurfaceSecondary,
                                modifier = Modifier.size(34.dp),
                            )
                        }
                    }
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = target.title,
                        style = MaterialTheme.typography.titleSmall.copy(
                            fontSize = 13.5.sp,
                            fontWeight = FontWeight.SemiBold,
                        ),
                        color = themeTokens.onSurfacePrimary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (target.subtitle.isNotBlank()) {
                        Text(
                            text = target.subtitle,
                            style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                            color = themeTokens.onSurfaceSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun HeldActionMenuCard(
    target: HeldContextTarget,
    isLiked: Boolean,
    isDisliked: Boolean,
    isDownloaded: Boolean,
    activeDownload: ActiveDownload?,
    onDismiss: () -> Unit,
    onToggleLike: (AndroidTrack) -> Unit,
    onToggleDislike: (AndroidTrack) -> Unit,
    onShareTrack: (AndroidTrack) -> Unit,
    onAddTrackToPlaylist: (AndroidTrack) -> Unit,
    onStartRadio: (AndroidTrack) -> Unit,
    onPlayTrackNext: (AndroidTrack) -> Unit,
    onAddTrackToQueue: (AndroidTrack) -> Unit,
    onDownloadTrack: (AndroidTrack) -> Unit,
    onRemoveFromCache: (AndroidTrack) -> Unit,
    onOpenAlbum: (AndroidTrack) -> Unit,
    onOpenArtist: (AndroidTrack) -> Unit,
    onSongInfo: (AndroidTrack) -> Unit,
    onPlayCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onShuffleCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onAddCollectionToPlaylist: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onPlayCollectionNext: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onAddCollectionToQueue: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onDownloadCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onOpenCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    onShareCollection: (HeldContextTarget.AlbumOrPlaylistCard) -> Unit,
    isCollectionPinned: Boolean = false,
    isCollectionSavedToLibrary: Boolean = false,
    canSaveCollectionToLibrary: Boolean = false,
    onTogglePinCollection: ((HeldContextTarget.AlbumOrPlaylistCard) -> Unit)? = null,
    onToggleSaveCollectionToLibrary: ((HeldContextTarget.AlbumOrPlaylistCard) -> Unit)? = null,
) {
    val themeTokens = LocalLunariaColors.current
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val menuShape = remember(themeTokens.isViolet) {
        RoundedCornerShape(if (themeTokens.isViolet) 20.dp else 26.dp)
    }
    val cardInteraction = remember { MutableInteractionSource() }
    val baseModifier = Modifier
        .width(256.dp)
        .shadow(
            elevation = 24.dp,
            shape = menuShape,
            clip = false,
            ambientColor = themeTokens.glassShadowColor,
            spotColor = themeTokens.glassShadowColor,
        )
    val menuSurfaceModifier = if (glassOn) {
        baseModifier
            .liquidGlass(shape = menuShape, surfaceTint = themeTokens.glassTint)
            .sonataSpecularRim(shape = menuShape, strokeWidth = 0.75.dp)
    } else {
        baseModifier
            .clip(menuShape)
            .background(themeTokens.eclipseSurface.copy(alpha = 0.96f), menuShape)
            .border(0.75.dp, themeTokens.cardStroke, menuShape)
    }

    Column(
        modifier = menuSurfaceModifier
            .clickable(
                interactionSource = cardInteraction,
                indication = null,
                onClick = {},
            )
            .padding(vertical = 8.dp),
    ) {
        when (target) {
            is HeldContextTarget.TrackRow -> {
                val track = target.track
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    TrackQuickTile(
                        icon = if (isLiked) SonataIcons.StarFilled else SonataIcons.Star,
                        label = if (isLiked) strings.saved else strings.like,
                        tint = if (isLiked) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                        onClick = {
                            onToggleLike(track)
                            onDismiss()
                        },
                        modifier = Modifier.weight(1f),
                    )
                    TrackQuickTile(
                        icon = TrackDislikeIcon,
                        label = strings.dislike,
                        tint = if (isDisliked) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                        onClick = {
                            onToggleDislike(track)
                            onDismiss()
                        },
                        modifier = Modifier.weight(1f),
                    )
                    TrackQuickTile(
                        icon = TrackShareIcon,
                        label = strings.share,
                        tint = themeTokens.onSurfacePrimary,
                        onClick = {
                            onDismiss()
                            onShareTrack(track)
                        },
                        modifier = Modifier.weight(1f),
                    )
                }

                HorizontalDivider(
                    color = TrackMenuDivider,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                TrackMenuRow(
                    icon = TrackPlaylistAddIcon,
                    title = strings.addToPlaylist,
                    onClick = {
                        onDismiss()
                        onAddTrackToPlaylist(track)
                    },
                )

                HorizontalDivider(
                    color = TrackMenuDivider,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                TrackMenuRow(
                    icon = SonataRadioWavesIcon,
                    title = strings.startRadio,
                    onClick = {
                        onDismiss()
                        onStartRadio(track)
                    },
                )
                TrackMenuRow(
                    icon = TrackPlayNextIcon,
                    title = strings.playNext,
                    onClick = {
                        onDismiss()
                        onPlayTrackNext(track)
                    },
                )
                TrackMenuRow(
                    icon = SonataIcons.Queue,
                    title = strings.addToQueue,
                    onClick = {
                        onDismiss()
                        onAddTrackToQueue(track)
                    },
                )
                val downloadTitle = when {
                    activeDownload != null -> "${(activeDownload.progress * 100f).roundToInt()}%"
                    isDownloaded -> strings.downloaded
                    else -> strings.download
                }
                TrackMenuRow(
                    icon = if (isDownloaded) SonataIcons.Check else SonataIcons.Download,
                    title = downloadTitle,
                    tint = if (isDownloaded) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                    onClick = {
                        onDownloadTrack(track)
                        onDismiss()
                    },
                )
                TrackMenuRow(
                    icon = TrackTrashIcon,
                    title = strings.removeFromCache,
                    onClick = {
                        onRemoveFromCache(track)
                        onDismiss()
                    },
                )

                HorizontalDivider(
                    color = TrackMenuDivider,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                TrackMenuRow(
                    icon = SonataIcons.MusicNote,
                    title = strings.openAlbum,
                    subtitle = track.album.takeIf { it.isNotBlank() } ?: track.title,
                    onClick = {
                        onDismiss()
                        onOpenAlbum(track)
                    },
                )
                if (track.artist.isNotBlank()) {
                    TrackMenuRow(
                        icon = SonataIcons.Person,
                        title = strings.goToArtist,
                        subtitle = track.artist,
                        onClick = {
                            onDismiss()
                            onOpenArtist(track)
                        },
                    )
                }
                TrackMenuRow(
                    icon = TrackInfoIcon,
                    title = strings.songInfo,
                    onClick = {
                        onDismiss()
                        onSongInfo(track)
                    },
                )
            }

            is HeldContextTarget.AlbumOrPlaylistCard -> {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                TrackMenuRow(
                    icon = SonataIcons.Play,
                    title = strings.playCollection,
                    onClick = {
                        onDismiss()
                        onPlayCollection(target)
                    },
                )
                TrackMenuRow(
                    icon = SonataIcons.Shuffle,
                    title = strings.shufflePlay,
                    onClick = {
                        onDismiss()
                        onShuffleCollection(target)
                    },
                )
                TrackMenuRow(
                    icon = TrackPlaylistAddIcon,
                    title = strings.addToPlaylist,
                    onClick = {
                        onDismiss()
                        onAddCollectionToPlaylist(target)
                    },
                )
                TrackMenuRow(
                    icon = TrackPlayNextIcon,
                    title = strings.playNext,
                    onClick = {
                        onDismiss()
                        onPlayCollectionNext(target)
                    },
                )
                TrackMenuRow(
                    icon = SonataIcons.Queue,
                    title = strings.addToQueue,
                    onClick = {
                        onDismiss()
                        onAddCollectionToQueue(target)
                    },
                )
                TrackMenuRow(
                    icon = SonataIcons.Download,
                    title = strings.downloadAll,
                    onClick = {
                        onDismiss()
                        onDownloadCollection(target)
                    },
                )
                TrackMenuRow(
                    icon = SonataIcons.ChevronRight,
                    title = if (target.isAlbum) strings.openAlbum else strings.openPlaylist,
                    onClick = {
                        onDismiss()
                        onOpenCollection(target)
                    },
                )
                if (onTogglePinCollection != null) {
                    TrackMenuRow(
                        icon = if (isCollectionPinned) SonataIcons.PinFilled else SonataIcons.Pin,
                        title = if (isCollectionPinned) strings.unpinFromLibrary else strings.pinToLibrary,
                        tint = if (isCollectionPinned && !themeTokens.isBasic) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                        onClick = {
                            onDismiss()
                            onTogglePinCollection(target)
                        },
                    )
                }
                if (canSaveCollectionToLibrary && onToggleSaveCollectionToLibrary != null) {
                    TrackMenuRow(
                        icon = if (isCollectionSavedToLibrary) SonataIcons.Check else SonataIcons.Add,
                        title = if (isCollectionSavedToLibrary) strings.removeFromLibrary else strings.addToLibrary,
                        tint = if (isCollectionSavedToLibrary && !themeTokens.isBasic) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                        onClick = {
                            onDismiss()
                            onToggleSaveCollectionToLibrary(target)
                        },
                    )
                }
                TrackMenuRow(
                    icon = TrackShareIcon,
                    title = strings.share,
                    onClick = {
                        onDismiss()
                        onShareCollection(target)
                    },
                )
            }
        }
    }
}
