/*
 * Clean-room Sonata Now Playing options popover menu.
 *
 * Replaces cluttered bottom-bar chips on the full-screen player with a crisp
 * floating popover containing quick top actions (Like, Add to Playlist, Share)
 * and grouped playback, download, audio pipeline, timer, lyrics timing/translation,
 * and navigation actions.
 */
package dev.sonata.android.ui.player

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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.ecosystemStrings
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.SonataRadioWavesIcon
import dev.sonata.android.ui.theme.sonataSpecularRim
import uniffi.sonata_android.AndroidTrack
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Floating Liquid Glass options menu for [NowPlayingScreen], triggered from the player `⋯` button.
 * Renders with NO black background dimming overlay so the underlying album cover / animated
 * Canvas refracts through the menu via [backdrop].
 */
@Composable
fun NowPlayingOptionsMenu(
    visible: Boolean,
    track: AndroidTrack,
    isLiked: Boolean,
    isDownloaded: Boolean,
    activeDownload: ActiveDownload? = null,
    sleepTimerRemainingMinutes: Int? = null,
    lyricOffsetMs: Long = 0L,
    romanizationEnabled: Boolean = false,
    translationEnabled: Boolean = false,
    pipelineSummary: String? = null,
    connectDeviceName: String? = null,
    appLanguage: AppLanguage = AppLanguage.SYSTEM,
    backdrop: Backdrop? = null,
    onDismiss: () -> Unit,
    onOpenConnectDevices: () -> Unit = {},
    onToggleLike: () -> Unit,
    onAddToPlaylist: () -> Unit,
    onShareInApp: () -> Unit,
    onOpenAudioControls: () -> Unit,
    onOpenAudioPipeline: () -> Unit,
    onDownloadTrack: () -> Unit,
    onOpenSleepTimer: () -> Unit,
    onStepLyricOffsetMs: (Long) -> Unit,
    onResetLyricOffset: () -> Unit = {},
    onToggleRomanization: () -> Unit,
    onToggleTranslation: () -> Unit,
    onStartRadio: () -> Unit,
    onPlayNext: () -> Unit,
    onAddToQueue: () -> Unit,
    onOpenAlbum: () -> Unit,
    onOpenArtist: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AnimatedVisibility(
        visible = visible,
        enter = fadeIn() + scaleIn(
            initialScale = 0.90f,
            transformOrigin = TransformOrigin(0.92f, 0.14f),
        ),
        exit = fadeOut() + scaleOut(
            targetScale = 0.90f,
            transformOrigin = TransformOrigin(0.92f, 0.14f),
        ),
        modifier = modifier.fillMaxSize(),
    ) {
        val targetBackdrop = backdrop ?: LocalAppBackdrop.current
        val outsideInteraction = remember { MutableInteractionSource() }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Transparent)
                .clickable(
                    interactionSource = outsideInteraction,
                    indication = null,
                    onClick = onDismiss,
                )
                .statusBarsPadding()
                .navigationBarsPadding()
                .padding(start = 16.dp, end = 18.dp, top = 56.dp, bottom = 20.dp),
            contentAlignment = Alignment.TopEnd,
        ) {
            CompositionLocalProvider(LocalAppBackdrop provides targetBackdrop) {
            val themeTokens = LocalLunariaColors.current
            val menuShape = remember(themeTokens.isViolet) {
                RoundedCornerShape(if (themeTokens.isViolet) 20.dp else 26.dp)
            }
            val dividerColor = themeTokens.cardStroke
            val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
            val cardModifier = if (glassOn) {
                Modifier
                    .widthIn(min = 276.dp, max = 312.dp)
                    .shadow(
                        elevation = 24.dp,
                        shape = menuShape,
                        clip = false,
                        ambientColor = themeTokens.glassShadowColor,
                        spotColor = themeTokens.glassShadowColor,
                    )
                    .liquidGlass(shape = menuShape, surfaceTint = themeTokens.glassTint)
                    .sonataSpecularRim(shape = menuShape, strokeWidth = 0.75.dp)
            } else {
                Modifier
                    .widthIn(min = 276.dp, max = 312.dp)
                    .shadow(
                        elevation = 24.dp,
                        shape = menuShape,
                        clip = false,
                        ambientColor = themeTokens.glassShadowColor,
                        spotColor = themeTokens.glassShadowColor,
                    )
                    .clip(menuShape)
                    .background(themeTokens.eclipseSurface.copy(alpha = 0.96f), menuShape)
                    .border(0.75.dp, themeTokens.cardStroke, menuShape)
            }

            // Consume clicks inside the card so tapping non-interactive gaps doesn't dismiss
            val cardInteraction = remember { MutableInteractionSource() }
            Column(
                modifier = cardModifier
                    .clickable(
                        interactionSource = cardInteraction,
                        indication = null,
                        onClick = {},
                    )
                    .verticalScroll(rememberScrollState())
                    .padding(12.dp),
            ) {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                // Top 3-button quick strip: Like, Add to Playlist, Share (strictly in-app)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    QuickTopTile(
                        icon = if (isLiked) SonataIcons.HeartFilled else SonataIcons.Heart,
                        label = if (isLiked) strings.saved else strings.like,
                        tint = if (isLiked) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                        onClick = {
                            onToggleLike()
                            onDismiss()
                        },
                        modifier = Modifier.weight(1f),
                    )
                    QuickTopTile(
                        icon = PopoverPlaylistAddIcon,
                        label = strings.playlistLabel,
                        tint = themeTokens.onSurfacePrimary,
                        onClick = {
                            onDismiss()
                            onAddToPlaylist()
                        },
                        modifier = Modifier.weight(1f),
                    )
                    QuickTopTile(
                        icon = PopoverShareIcon,
                        label = strings.share,
                        tint = themeTokens.onSurfacePrimary,
                        onClick = {
                            onDismiss()
                            onShareInApp()
                        },
                        modifier = Modifier.weight(1f),
                    )
                }

                Spacer(Modifier.height(10.dp))
                HorizontalDivider(color = dividerColor, thickness = 0.5.dp)

                val eco = remember(appLanguage) { ecosystemStrings(appLanguage) }
                // Connect Devices
                PopoverActionRow(
                    icon = SonataIcons.Wifi,
                    title = eco.connectDevicesTitle,
                    subtitle = connectDeviceName?.let { eco.connectPlayingOn(it) } ?: eco.connectThisPhone,
                    tint = if (connectDeviceName != null) Color(0xFF34C759) else themeTokens.onSurfacePrimary,
                    onClick = {
                        onDismiss()
                        onOpenConnectDevices()
                    },
                )

                // Audio Controls & Audio Pipeline
                PopoverActionRow(
                    icon = SonataIcons.GraphicEq,
                    title = strings.audioControls,
                    subtitle = strings.audioPipelineSubtitle,
                    onClick = {
                        onDismiss()
                        onOpenAudioControls()
                    },
                )
                PopoverActionRow(
                    icon = PopoverInfoIcon,
                    title = strings.audioPipeline,
                    subtitle = pipelineSummary,
                    onClick = {
                        onDismiss()
                        onOpenAudioPipeline()
                    },
                )

                // Download Track
                val downloadSubtitle = when {
                    activeDownload != null -> "${(activeDownload.progress * 100f).roundToInt()}%"
                    isDownloaded -> strings.downloaded
                    else -> strings.download
                }
                PopoverActionRow(
                    icon = if (isDownloaded) SonataIcons.Check else SonataIcons.Download,
                    title = if (isDownloaded) strings.downloaded else strings.download,
                    subtitle = downloadSubtitle,
                    tint = if (isDownloaded) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                    onClick = {
                        onDownloadTrack()
                        onDismiss()
                    },
                )

                // Sleep Timer
                PopoverActionRow(
                    icon = SonataIcons.History,
                    title = strings.sleepTimer,
                    subtitle = sleepTimerRemainingMinutes?.let { strings.sleepMinutesRemaining(it) } ?: strings.sleepOff,
                    tint = if (sleepTimerRemainingMinutes != null) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
                    onClick = {
                        onDismiss()
                        onOpenSleepTimer()
                    },
                )

                HorizontalDivider(
                    color = dividerColor,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                // Lyrics Timing Offset (-5.0s .. +5.0s)
                LyricsOffsetStepperRow(
                    offsetMs = lyricOffsetMs,
                    onStep = onStepLyricOffsetMs,
                    onReset = onResetLyricOffset,
                )

                // Romanize Lyrics toggle
                PopoverSwitchRow(
                    icon = SonataIcons.LyricsQuote,
                    title = strings.romanizeLyrics,
                    checked = romanizationEnabled,
                    onCheckedChange = { onToggleRomanization() },
                )

                // Translate Lyrics toggle
                PopoverSwitchRow(
                    icon = SonataIcons.Sparkles,
                    title = strings.translateLyrics,
                    checked = translationEnabled,
                    onCheckedChange = { onToggleTranslation() },
                )

                HorizontalDivider(
                    color = dividerColor,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                // Queue & Radio actions
                PopoverActionRow(
                    icon = SonataRadioWavesIcon,
                    title = strings.startRadio,
                    onClick = {
                        onDismiss()
                        onStartRadio()
                    },
                )
                PopoverActionRow(
                    icon = SonataIcons.Play,
                    title = strings.playNext,
                    onClick = {
                        onDismiss()
                        onPlayNext()
                    },
                )
                PopoverActionRow(
                    icon = SonataIcons.Queue,
                    title = strings.addToQueue,
                    onClick = {
                        onDismiss()
                        onAddToQueue()
                    },
                )

                HorizontalDivider(
                    color = dividerColor,
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp),
                )

                // In-app Navigation
                PopoverActionRow(
                    icon = SonataIcons.MusicNote,
                    title = strings.openAlbum,
                    subtitle = track.album?.takeIf { it.isNotBlank() } ?: track.title,
                    onClick = {
                        onDismiss()
                        onOpenAlbum()
                    },
                )
                if (track.artist.isNotBlank()) {
                    PopoverActionRow(
                        icon = SonataIcons.Person,
                        title = strings.goToArtist,
                        subtitle = track.artist,
                        onClick = {
                            onDismiss()
                            onOpenArtist()
                        },
                    )
                }
            }
            }
        }
    }
}

@Composable
private fun QuickTopTile(
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
            .background(themeTokens.eclipseCard.copy(alpha = 0.65f))
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp, horizontal = 6.dp),
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
private fun PopoverActionRow(
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
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 9.dp),
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

@Composable
private fun PopoverSwitchRow(
    icon: ImageVector,
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    val themeTokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = if (checked) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
            modifier = Modifier.size(19.dp),
        )
        Spacer(Modifier.width(12.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = themeTokens.onSurfacePrimary,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = themeTokens.petalLavender,
                uncheckedThumbColor = if (themeTokens.isDark) Color.White else themeTokens.eclipseSurface,
                uncheckedTrackColor = themeTokens.onSurfaceSecondary.copy(alpha = 0.28f),
                uncheckedBorderColor = Color.Transparent,
            ),
        )
    }
}

@Composable
private fun LyricsOffsetStepperRow(
    offsetMs: Long,
    onStep: (Long) -> Unit,
    onReset: () -> Unit,
) {
    val themeTokens = LocalLunariaColors.current
    val formatted = remember(offsetMs) {
        val secs = offsetMs / 1000f
        String.format(Locale.US, "%+.2fs", secs)
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = SonataIcons.LyricsQuote,
            contentDescription = null,
            tint = if (offsetMs != 0L) themeTokens.petalLavender else themeTokens.onSurfacePrimary,
            modifier = Modifier.size(19.dp),
        )
        Spacer(Modifier.width(12.dp))
        Text(
            text = dev.sonata.android.ui.i18n.LocalSonataStrings.current.lyricsTiming,
            style = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = themeTokens.onSurfacePrimary,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(26.dp)
                    .clip(CircleShape)
                    .background(themeTokens.eclipseCard.copy(alpha = 0.72f))
                    .clickable(enabled = offsetMs > -5000L) { onStep(-250L) },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "−",
                    color = if (offsetMs > -5000L) themeTokens.onSurfacePrimary else themeTokens.onSurfaceSecondary.copy(alpha = 0.40f),
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
            Text(
                text = formatted,
                style = MaterialTheme.typography.labelMedium.copy(
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                ),
                color = if (offsetMs != 0L) themeTokens.petalLavender else themeTokens.onSurfaceSecondary,
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(onClick = onReset)
                    .padding(horizontal = 4.dp, vertical = 2.dp),
            )
            Box(
                modifier = Modifier
                    .size(26.dp)
                    .clip(CircleShape)
                    .background(themeTokens.eclipseCard.copy(alpha = 0.72f))
                    .clickable(enabled = offsetMs < 5000L) { onStep(250L) },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "+",
                    color = if (offsetMs < 5000L) themeTokens.onSurfacePrimary else themeTokens.onSurfaceSecondary.copy(alpha = 0.40f),
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
    }
}

private val PopoverPlaylistAddIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "PopoverPlaylistAdd",
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

private val PopoverShareIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "PopoverShare",
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

private val PopoverInfoIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "PopoverInfo",
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
