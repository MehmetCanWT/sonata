/*
 * Clean-room Liquid Glass floating 3-dot track options menu for Sonata Android.
 *
 * Designed without a black dimming overlay so the underlying artwork, shelves,
 * or song rows refract cleanly through the menu card via [Modifier.liquidGlass].
 */
package dev.sonata.android.ui.components

import androidx.compose.animation.AnimatedVisibility
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
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import dev.sonata.android.ui.theme.sonataSpecularRim
import uniffi.sonata_android.AndroidTrack
import kotlin.math.roundToInt

internal val TrackMenuDivider: Color
    @Composable get() = LocalLunariaColors.current.cardStroke

/**
 * Compact Liquid Glass floating context menu for any track row (3-dot menu).
 * Renders with ZERO dark scrim so the scrollable page remains vibrant and refracts
 * naturally behind the menu surface.
 */
@Composable
fun TrackOptionsMenu(
    track: AndroidTrack?,
    isLiked: Boolean = false,
    isDisliked: Boolean = false,
    isDownloaded: Boolean = false,
    activeDownload: ActiveDownload? = null,
    backdrop: Backdrop? = null,
    anchorBounds: Rect? = null,
    alignment: Alignment = Alignment.CenterEnd,
    onDismiss: () -> Unit,
    onToggleLike: (AndroidTrack) -> Unit = {},
    onToggleDislike: (AndroidTrack) -> Unit = {},
    onShareInApp: (AndroidTrack) -> Unit = {},
    onAddToPlaylist: (AndroidTrack) -> Unit = {},
    onStartRadio: (AndroidTrack) -> Unit = {},
    onPlayNext: (AndroidTrack) -> Unit = {},
    onAddToQueue: (AndroidTrack) -> Unit = {},
    onDownloadTrack: (AndroidTrack) -> Unit = {},
    onRemoveFromCache: (AndroidTrack) -> Unit = {},
    onOpenAlbum: (AndroidTrack) -> Unit = {},
    onOpenArtist: (AndroidTrack) -> Unit = {},
    onSongInfo: (AndroidTrack) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    AnimatedVisibility(
        visible = track != null,
        enter = fadeIn() + scaleIn(
            initialScale = 0.92f,
            transformOrigin = if (anchorBounds != null) TransformOrigin(0.5f, 0.25f) else TransformOrigin(0.85f, 0.5f),
        ),
        exit = fadeOut() + scaleOut(
            targetScale = 0.92f,
            transformOrigin = if (anchorBounds != null) TransformOrigin(0.5f, 0.25f) else TransformOrigin(0.85f, 0.5f),
        ),
        modifier = modifier.fillMaxSize(),
    ) {
        val currentTrack = track ?: return@AnimatedVisibility
        val targetBackdrop = backdrop ?: LocalAppBackdrop.current
        val outsideInteraction = remember { MutableInteractionSource() }
        val density = LocalDensity.current
        var containerOrigin by remember { mutableStateOf(Offset.Zero) }
        var menuHeightPx by remember(currentTrack.id) {
            mutableIntStateOf(with(density) { 420.dp.roundToPx() })
        }

        BoxWithConstraints(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Transparent)
                .onGloballyPositioned { containerOrigin = it.positionInRoot() }
                .clickable(
                    interactionSource = outsideInteraction,
                    indication = null,
                    onClick = onDismiss,
                )
                .then(
                    if (anchorBounds == null) {
                        Modifier
                            .statusBarsPadding()
                            .navigationBarsPadding()
                            .padding(horizontal = 20.dp, vertical = 20.dp)
                    } else {
                        Modifier
                    },
                ),
            contentAlignment = if (anchorBounds == null) alignment else Alignment.TopStart,
        ) {
            val anchoredOffsetModifier = if (anchorBounds != null) {
                val maxWidthPx = constraints.maxWidth
                val maxHeightPx = constraints.maxHeight
                val menuWidthPx = with(density) { 256.dp.roundToPx() }
                val edgePadPx = with(density) { 14.dp.roundToPx() }
                val topInsetPx = WindowInsets.statusBars.getTop(density) + edgePadPx
                val bottomInsetPx = WindowInsets.navigationBars.getBottom(density) + with(density) { 16.dp.roundToPx() }

                val rel = anchorBounds.translate(-containerOrigin.x, -containerOrigin.y)
                val isIconAnchor = rel.width < with(density) { 96.dp.toPx() }
                val rawX = if (isIconAnchor) {
                    (rel.right - menuWidthPx).roundToInt()
                } else {
                    (rel.center.x - menuWidthPx * 0.5f).roundToInt()
                }
                val maxX = (maxWidthPx - menuWidthPx - edgePadPx).coerceAtLeast(edgePadPx)
                val xPx = rawX.coerceIn(edgePadPx, maxX)

                val rawY = rel.top.roundToInt()
                val maxY = (maxHeightPx - menuHeightPx - bottomInsetPx).coerceAtLeast(topInsetPx)
                val yPx = rawY.coerceIn(topInsetPx, maxY)
                Modifier.offset { IntOffset(xPx, yPx) }
            } else {
                Modifier
            }

            CompositionLocalProvider(LocalAppBackdrop provides targetBackdrop) {
                val themeTokens = LocalLunariaColors.current
                val menuShape = remember(themeTokens.isViolet) {
                    RoundedCornerShape(if (themeTokens.isViolet) 20.dp else 26.dp)
                }
                val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
                val baseModifier = anchoredOffsetModifier
                    .width(256.dp)
                    .onSizeChanged { menuHeightPx = it.height }
                    .shadow(
                        elevation = 24.dp,
                        shape = menuShape,
                        clip = false,
                        ambientColor = themeTokens.glassShadowColor,
                        spotColor = themeTokens.glassShadowColor,
                    )
                val cardModifier = if (glassOn) {
                    baseModifier
                        .liquidGlass(shape = menuShape, surfaceTint = themeTokens.glassTint)
                        .sonataSpecularRim(shape = menuShape, strokeWidth = 0.75.dp)
                } else {
                    baseModifier
                        .clip(menuShape)
                        .background(themeTokens.eclipseSurface.copy(alpha = 0.96f), menuShape)
                        .border(0.75.dp, themeTokens.cardStroke, menuShape)
                }

                val cardInteraction = remember { MutableInteractionSource() }
                Column(
                    modifier = cardModifier
                        .clickable(
                            interactionSource = cardInteraction,
                            indication = null,
                            onClick = {},
                        )
                        .verticalScroll(rememberScrollState())
                        .padding(vertical = 8.dp),
                ) {
                    // Top 3 quick actions: Like, Dislike, Share (in-app)
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
                                onToggleLike(currentTrack)
                                onDismiss()
                            },
                            modifier = Modifier.weight(1f),
                        )
                        TrackQuickTile(
                            icon = TrackDislikeIcon,
                            label = strings.dislike,
                            tint = if (isDisliked) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                            onClick = {
                                onToggleDislike(currentTrack)
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
                                onShareInApp(currentTrack)
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
                            onAddToPlaylist(currentTrack)
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
                            onStartRadio(currentTrack)
                        },
                    )
                    TrackMenuRow(
                        icon = TrackPlayNextIcon,
                        title = strings.playNext,
                        onClick = {
                            onDismiss()
                            onPlayNext(currentTrack)
                        },
                    )
                    TrackMenuRow(
                        icon = SonataIcons.Queue,
                        title = strings.addToQueue,
                        onClick = {
                            onDismiss()
                            onAddToQueue(currentTrack)
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
                            onDownloadTrack(currentTrack)
                            onDismiss()
                        },
                    )
                    TrackMenuRow(
                        icon = TrackTrashIcon,
                        title = strings.removeFromCache,
                        onClick = {
                            onRemoveFromCache(currentTrack)
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
                        subtitle = currentTrack.album?.takeIf { it.isNotBlank() } ?: currentTrack.title,
                        onClick = {
                            onDismiss()
                            onOpenAlbum(currentTrack)
                        },
                    )
                    if (currentTrack.artist.isNotBlank()) {
                        TrackMenuRow(
                            icon = SonataIcons.Person,
                            title = strings.goToArtist,
                            subtitle = currentTrack.artist,
                            onClick = {
                                onDismiss()
                                onOpenArtist(currentTrack)
                            },
                        )
                    }

                    HorizontalDivider(
                        color = TrackMenuDivider,
                        thickness = 0.5.dp,
                        modifier = Modifier.padding(vertical = 4.dp),
                    )

                    TrackMenuRow(
                        icon = TrackInfoIcon,
                        title = strings.songInfo,
                        onClick = {
                            onDismiss()
                            onSongInfo(currentTrack)
                        },
                    )
                }
            }
        }
    }
}

@Composable
internal fun TrackQuickTile(
    icon: ImageVector,
    label: String,
    tint: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val themeTokens = LocalLunariaColors.current
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(14.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp, horizontal = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = tint,
            modifier = Modifier.size(20.dp),
        )
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall.copy(
                fontSize = 11.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = themeTokens.onSurfacePrimary.copy(alpha = 0.90f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
internal fun TrackMenuRow(
    icon: ImageVector,
    title: String,
    subtitle: String? = null,
    tint: Color = LocalLunariaColors.current.onSurfacePrimary,
    onClick: () -> Unit,
) {
    val themeTokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(19.dp),
        )
        Spacer(Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Medium,
                ),
                color = themeTokens.onSurfacePrimary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (!subtitle.isNullOrBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.labelSmall.copy(fontSize = 11.5.sp),
                    color = themeTokens.onSurfaceSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

internal val TrackPlaylistAddIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackPlaylistAdd",
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
            moveTo(4f, 6f)
            lineTo(16f, 6f)
            moveTo(4f, 11f)
            lineTo(16f, 11f)
            moveTo(4f, 16f)
            lineTo(11f, 16f)
            moveTo(18f, 13f)
            lineTo(18f, 20f)
            moveTo(14.5f, 16.5f)
            lineTo(21.5f, 16.5f)
        }
    }.build()
}

internal val TrackPlayNextIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackPlayNext",
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
            moveTo(4f, 7f)
            lineTo(19f, 7f)
            moveTo(4f, 12f)
            lineTo(14f, 12f)
            moveTo(4f, 17f)
            lineTo(14f, 17f)
            moveTo(18f, 12f)
            lineTo(21f, 14.5f)
            lineTo(18f, 17f)
        }
    }.build()
}

internal val TrackShareIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackShare",
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

internal val TrackDislikeIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackDislike",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.85f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(12f, 20f)
            curveTo(12f, 20f, 4f, 14.8f, 4f, 9.2f)
            curveTo(4f, 6.4f, 6.2f, 4.5f, 8.7f, 4.5f)
            curveTo(10.3f, 4.5f, 11.4f, 5.4f, 12f, 6.5f)
            curveTo(12.6f, 5.4f, 13.7f, 4.5f, 15.3f, 4.5f)
            curveTo(17.8f, 4.5f, 20f, 6.4f, 20f, 9.2f)
            curveTo(20f, 14.8f, 12f, 20f, 12f, 20f)
            close()
            moveTo(12.8f, 8.5f)
            lineTo(10.8f, 12.2f)
            lineTo(13.2f, 13.6f)
            lineTo(11.3f, 17f)
        }
    }.build()
}

internal val TrackInfoIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackInfo",
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
            arcTo(9f, 9f, 0f, true, true, 11.99f, 3f)
            close()
            moveTo(12f, 11f)
            lineTo(12f, 16.5f)
        }
        path(
            fill = SolidColor(Color.Black),
            pathFillType = PathFillType.NonZero,
        ) {
            moveTo(12f, 7.1f)
            arcTo(1.1f, 1.1f, 0f, true, true, 11.99f, 7.1f)
            close()
        }
    }.build()
}

internal val TrackTrashIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "TrackTrash",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.85f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(4f, 6.5f)
            lineTo(20f, 6.5f)
            moveTo(9.5f, 6.5f)
            lineTo(9.5f, 4.3f)
            lineTo(14.5f, 4.3f)
            lineTo(14.5f, 6.5f)
            moveTo(6.5f, 6.5f)
            lineTo(7.4f, 19.2f)
            arcTo(1.5f, 1.5f, 0f, false, false, 8.9f, 20.6f)
            lineTo(15.1f, 20.6f)
            arcTo(1.5f, 1.5f, 0f, false, false, 16.6f, 19.2f)
            lineTo(17.5f, 6.5f)
            moveTo(10.3f, 10.2f)
            lineTo(10.3f, 16.8f)
            moveTo(13.7f, 10.2f)
            lineTo(13.7f, 16.8f)
        }
    }.build()
}
