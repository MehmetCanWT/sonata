/*
 * Music Video mode UI for Now Playing:
 * - Top Music / Video segmented pill
 * - Audio-quality drop warning dialog with "Do not show again" checkbox
 * - Resolution selector dialog (Auto / 1080p / 720p / 480p / 360p)
 * - Inline 16:9 video stage & immersive landscape fullscreen overlay bound to PlaybackService ExoPlayer
 */
package dev.sonata.android.ui.player

import android.app.Activity
import android.content.pm.ActivityInfo
import android.graphics.Matrix
import android.graphics.SurfaceTexture
import android.view.TextureView
import android.view.ViewGroup
import android.view.WindowManager
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import dev.sonata.android.PlaybackAudioBridge
import dev.sonata.android.PlaybackClockState
import dev.sonata.android.R
import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.formatDurationMs
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.ThinSlider
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.LocalLunariaColors
import kotlinx.coroutines.delay
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidVideoQualityStream

@Composable
internal fun MusicVideoModePill(
    videoModeEnabled: Boolean,
    videoLoading: Boolean,
    onSelectMusic: () -> Unit,
    onSelectVideo: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val pillShape = RoundedCornerShape(percent = 50)
    val activeBg = if (tokens.isBasic) Color.White else sonataThemeAccent()
    val activeFg = if (tokens.isBasic) Color.Black else Color.Black

    Row(
        modifier = modifier
            .clip(pillShape)
            .background(Color.Black.copy(alpha = 0.42f))
            .border(0.75.dp, Color.White.copy(alpha = 0.20f), pillShape)
            .padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .clip(pillShape)
                .background(if (!videoModeEnabled) activeBg else Color.Black.copy(alpha = 0f))
                .clickable(enabled = videoModeEnabled, onClick = onSelectMusic)
                .padding(horizontal = 16.dp, vertical = 6.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = strings.musicMode,
                style = MaterialTheme.typography.labelMedium.copy(
                    fontWeight = if (!videoModeEnabled) FontWeight.Bold else FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = if (!videoModeEnabled) activeFg else Color.White.copy(alpha = 0.88f),
                maxLines = 1,
            )
        }
        Box(
            modifier = Modifier
                .clip(pillShape)
                .background(if (videoModeEnabled) activeBg else Color.Black.copy(alpha = 0f))
                .clickable(enabled = !videoModeEnabled && !videoLoading, onClick = onSelectVideo)
                .padding(horizontal = 16.dp, vertical = 6.dp),
            contentAlignment = Alignment.Center,
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (videoLoading && !videoModeEnabled) {
                    CircularProgressIndicator(
                        color = Color.White,
                        strokeWidth = 1.6.dp,
                        modifier = Modifier.size(12.dp),
                    )
                }
                Text(
                    text = strings.videoMode,
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontWeight = if (videoModeEnabled) FontWeight.Bold else FontWeight.Medium,
                        fontSize = 13.sp,
                    ),
                    color = if (videoModeEnabled) activeFg else Color.White.copy(alpha = 0.88f),
                    maxLines = 1,
                )
            }
        }
    }
}

@Composable
internal fun VideoAudioQualityWarningDialog(
    onConfirm: (dontShowAgain: Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val accent = if (tokens.isBasic) tokens.onSurfacePrimary else tokens.petalLavender
    var dontShowAgain by remember { mutableStateOf(false) }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        val cardShape = RoundedCornerShape(22.dp)
        Column(
            modifier = Modifier
                .padding(horizontal = 24.dp)
                .widthIn(max = 400.dp)
                .fillMaxWidth()
                .clip(cardShape)
                .background(tokens.eclipseSurface)
                .border(0.75.dp, tokens.cardStroke, cardShape)
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = strings.videoQualityWarningTitle,
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                color = tokens.onSurfacePrimary,
            )
            Text(
                text = strings.videoQualityWarningBody,
                style = MaterialTheme.typography.bodyMedium,
                color = tokens.onSurfaceSecondary,
            )
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .clickable { dontShowAgain = !dontShowAgain }
                    .padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Checkbox(
                    checked = dontShowAgain,
                    onCheckedChange = { dontShowAgain = it },
                    colors = CheckboxDefaults.colors(
                        checkedColor = accent,
                        checkmarkColor = if (tokens.isBasic && tokens.isDark) Color.Black else Color.White,
                        uncheckedColor = tokens.onSurfaceSecondary,
                    ),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = strings.doNotShowAgain,
                    style = MaterialTheme.typography.bodyMedium,
                    color = tokens.onSurfacePrimary,
                )
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .clickable(onClick = onDismiss)
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                ) {
                    Text(
                        text = strings.cancel,
                        style = MaterialTheme.typography.labelLarge,
                        color = tokens.onSurfaceSecondary,
                    )
                }
                Spacer(Modifier.width(8.dp))
                val ctaFill = if (tokens.isBasic) tokens.onSurfacePrimary else tokens.petalLavender
                val ctaText = if (tokens.isBasic && tokens.isDark) Color.Black else Color.White
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(ctaFill)
                        .clickable { onConfirm(dontShowAgain) }
                        .padding(horizontal = 18.dp, vertical = 10.dp),
                ) {
                    Text(
                        text = strings.switchToVideo,
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = ctaText,
                    )
                }
            }
        }
    }
}

@Composable
internal fun VideoQualityPickerDialog(
    qualities: List<AndroidVideoQualityStream>,
    selectedPreference: VideoQualityPreference,
    activeHeight: Int,
    onSelect: (VideoQualityPreference, Int?) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val accent = if (tokens.isBasic) tokens.onSurfacePrimary else tokens.petalLavender

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        val cardShape = RoundedCornerShape(22.dp)
        Column(
            modifier = Modifier
                .padding(horizontal = 28.dp)
                .widthIn(max = 360.dp)
                .fillMaxWidth()
                .clip(cardShape)
                .background(tokens.eclipseSurface)
                .border(0.75.dp, tokens.cardStroke, cardShape)
                .padding(vertical = 16.dp),
        ) {
            Text(
                text = strings.videoQuality,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                color = tokens.onSurfacePrimary,
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
            )
            Spacer(Modifier.height(6.dp))
            HorizontalDivider(color = tokens.cardStroke, thickness = 0.5.dp)

            val autoLabel = if (activeHeight > 0) {
                "${strings.videoQualityAuto} (${activeHeight}p)"
            } else {
                strings.videoQualityAuto
            }
            VideoQualityOptionRow(
                label = autoLabel,
                selected = selectedPreference == VideoQualityPreference.AUTO,
                accentColor = accent,
                textColor = tokens.onSurfacePrimary,
                onClick = {
                    onSelect(VideoQualityPreference.AUTO, null)
                    onDismiss()
                },
            )

            qualities.forEach { q ->
                val heightInt = q.height.toInt()
                val mappedPref = when {
                    heightInt >= 1080 -> VideoQualityPreference.P1080
                    heightInt >= 720 -> VideoQualityPreference.P720
                    heightInt >= 480 -> VideoQualityPreference.P480
                    else -> VideoQualityPreference.P360
                }
                val isSelected = selectedPreference != VideoQualityPreference.AUTO &&
                    activeHeight == heightInt
                VideoQualityOptionRow(
                    label = q.qualityLabel.ifBlank { "${heightInt}p" },
                    selected = isSelected,
                    accentColor = accent,
                    textColor = tokens.onSurfacePrimary,
                    onClick = {
                        onSelect(mappedPref, heightInt)
                        onDismiss()
                    },
                )
            }
        }
    }
}

@Composable
private fun VideoQualityOptionRow(
    label: String,
    selected: Boolean,
    accentColor: Color,
    textColor: Color,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge.copy(
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
            ),
            color = if (selected) accentColor else textColor,
        )
        if (selected) {
            Icon(
                imageVector = SonataIcons.Check,
                contentDescription = null,
                tint = accentColor,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

@Composable
internal fun MusicVideoTextureSurface(
    modifier: Modifier = Modifier,
) {
    val videoAspect by PlaybackAudioBridge.videoAspectRatio.collectAsStateWithLifecycle()
    val textureRef = remember { mutableStateOf<TextureView?>(null) }

    DisposableEffect(Unit) {
        onDispose {
            textureRef.value?.let { PlaybackAudioBridge.clearVideoTextureView(it) }
            textureRef.value = null
        }
    }

    AndroidView(
        factory = { viewContext ->
            TextureView(viewContext).apply {
                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                )
                isOpaque = true
                textureRef.value = this
                PlaybackAudioBridge.attachVideoTextureView(this)
                addOnLayoutChangeListener { v, _, _, _, _, _, _, _, _ ->
                    applyFitCenterTransform(v as TextureView, PlaybackAudioBridge.videoAspectRatio.value)
                }
                surfaceTextureListener = object : TextureView.SurfaceTextureListener {
                    override fun onSurfaceTextureAvailable(surface: SurfaceTexture, width: Int, height: Int) {
                        PlaybackAudioBridge.attachVideoTextureView(this@apply)
                        applyFitCenterTransform(this@apply, PlaybackAudioBridge.videoAspectRatio.value)
                    }

                    override fun onSurfaceTextureSizeChanged(surface: SurfaceTexture, width: Int, height: Int) {
                        applyFitCenterTransform(this@apply, PlaybackAudioBridge.videoAspectRatio.value)
                    }

                    override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean = true

                    override fun onSurfaceTextureUpdated(surface: SurfaceTexture) = Unit
                }
            }
        },
        update = { tv ->
            applyFitCenterTransform(tv, videoAspect)
        },
        modifier = modifier.clipToBounds(),
    )
}

@Composable
internal fun InlineMusicVideoStage(
    artworkUrl: String?,
    qualityLabel: String,
    isBuffering: Boolean,
    videoLoading: Boolean,
    onOpenQualityPicker: () -> Unit,
    onEnterFullscreen: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalSonataStrings.current
    val thumbReq = rememberArtworkRequest(artworkUrl.artworkAt(CARD_ART_PX), 480, 480)
    var showOverlayControls by remember { mutableStateOf(true) }

    LaunchedEffect(showOverlayControls) {
        if (showOverlayControls) {
            delay(3_200L)
            showOverlayControls = false
        }
    }

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(Color.Black)
            .border(0.75.dp, Color.White.copy(alpha = 0.16f), RoundedCornerShape(16.dp)),
        contentAlignment = Alignment.Center,
    ) {
        if (thumbReq != null) {
            AsyncImage(
                model = thumbReq,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.36f)),
            )
        }

        MusicVideoTextureSurface(modifier = Modifier.fillMaxSize())

        // Dedicated Compose tap surface above TextureView so surface child never intercepts clicks
        Box(
            modifier = Modifier
                .fillMaxSize()
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                ) {
                    showOverlayControls = !showOverlayControls
                },
        )

        if (isBuffering || videoLoading) {
            CircularProgressIndicator(
                color = Color.White,
                strokeWidth = 2.5.dp,
                modifier = Modifier.size(34.dp),
            )
        }

        AnimatedVisibility(
            visible = showOverlayControls,
            enter = fadeIn(),
            exit = fadeOut(),
            modifier = Modifier.fillMaxSize(),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(10.dp),
            ) {
                Row(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .clip(RoundedCornerShape(percent = 50))
                        .background(Color.Black.copy(alpha = 0.62f))
                        .border(0.75.dp, Color.White.copy(alpha = 0.22f), RoundedCornerShape(percent = 50))
                        .clickable(onClick = onOpenQualityPicker)
                        .padding(horizontal = 10.dp, vertical = 5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.Tune,
                        contentDescription = strings.videoQuality,
                        tint = Color.White,
                        modifier = Modifier.size(14.dp),
                    )
                    Text(
                        text = qualityLabel.ifBlank { strings.videoQualityAuto },
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 11.5.sp,
                        ),
                        color = Color.White,
                    )
                }

                Box(
                    modifier = Modifier
                        .align(Alignment.BottomEnd)
                        .size(36.dp)
                        .clip(CircleShape)
                        .background(Color.Black.copy(alpha = 0.62f))
                        .border(0.75.dp, Color.White.copy(alpha = 0.22f), CircleShape)
                        .clickable(onClick = onEnterFullscreen),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.Fullscreen,
                        contentDescription = strings.fullscreen,
                        tint = Color.White,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }
    }
}

@Composable
internal fun FullscreenMusicVideoOverlay(
    track: AndroidTrack,
    isPlaying: Boolean,
    isBuffering: Boolean,
    hasNext: Boolean,
    hasPrevious: Boolean,
    clock: PlaybackClockState,
    qualityLabel: String,
    suspendAutoHide: Boolean = false,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onSeekTo: (Long) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onOpenQualityPicker: () -> Unit,
    onExitFullscreen: () -> Unit,
) {
    BackHandler(onBack = onExitFullscreen)

    val view = LocalView.current
    DisposableEffect(view) {
        val activity = view.context as? Activity
        val window = activity?.window
        val prevOrientation = activity?.requestedOrientation ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        window?.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        controller?.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller?.hide(WindowInsetsCompat.Type.systemBars())

        onDispose {
            controller?.show(WindowInsetsCompat.Type.systemBars())
            window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            activity?.requestedOrientation = prevOrientation
        }
    }

    val strings = LocalSonataStrings.current
    var controlsVisible by remember { mutableStateOf(true) }
    var scrubbing by remember { mutableStateOf(false) }
    var scrubFraction by remember { mutableFloatStateOf(0f) }
    var interactionTick by remember { mutableIntStateOf(0) }

    val durationMs = clock.durationMs
    val positionMs = clock.positionMs
    val latestPositionMs by rememberUpdatedState(positionMs)
    val latestDurationMs by rememberUpdatedState(durationMs)
    val latestOnSeekTo by rememberUpdatedState(onSeekTo)

    LaunchedEffect(suspendAutoHide) {
        if (!suspendAutoHide) {
            val activity = view.context as? Activity
            val window = activity?.window
            window?.let { WindowCompat.getInsetsController(it, view) }?.apply {
                systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
            interactionTick++
        }
    }

    LaunchedEffect(controlsVisible, isPlaying, scrubbing, suspendAutoHide, interactionTick) {
        if (controlsVisible && isPlaying && !scrubbing && !suspendAutoHide) {
            delay(3_500L)
            controlsVisible = false
        }
    }

    val liveFraction = if (durationMs > 0L) {
        (positionMs.toFloat() / durationMs.toFloat()).coerceIn(0f, 1f)
    } else {
        0f
    }
    val displayedFraction = if (scrubbing) scrubFraction else liveFraction
    val shownMs = (displayedFraction * durationMs).toLong()
    val remainingMs = (durationMs - shownMs).coerceAtLeast(0L)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black),
    ) {
        MusicVideoTextureSurface(modifier = Modifier.fillMaxSize())

        // Dedicated gesture layer keyed with Unit so playback position ticks never cancel taps
        Box(
            modifier = Modifier
                .fillMaxSize()
                .pointerInput(Unit) {
                    detectTapGestures(
                        onTap = {
                            controlsVisible = !controlsVisible
                            interactionTick++
                        },
                        onDoubleTap = { offset ->
                            val delta = if (offset.x < size.width / 2f) -10_000L else 10_000L
                            val target = (latestPositionMs + delta).coerceIn(
                                0L,
                                latestDurationMs.coerceAtLeast(0L),
                            )
                            latestOnSeekTo(target)
                            interactionTick++
                        },
                    )
                },
        )

        if (isBuffering) {
            CircularProgressIndicator(
                color = Color.White,
                strokeWidth = 3.dp,
                modifier = Modifier
                    .align(Alignment.Center)
                    .size(42.dp),
            )
        }

        AnimatedVisibility(
            visible = controlsVisible,
            enter = fadeIn(),
            exit = fadeOut(),
            modifier = Modifier.fillMaxSize(),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(
                        Brush.verticalGradient(
                            0.0f to Color.Black.copy(alpha = 0.65f),
                            0.35f to Color.Black.copy(alpha = 0.16f),
                            0.65f to Color.Black.copy(alpha = 0.16f),
                            1.0f to Color.Black.copy(alpha = 0.72f),
                        ),
                    )
                    .windowInsetsPadding(WindowInsets.safeDrawing)
                    .padding(horizontal = 20.dp, vertical = 12.dp),
            ) {
                // Top bar: Exit, Title + Artist, Quality selector
                Row(
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(onClick = onExitFullscreen) {
                        Icon(
                            imageVector = SonataIcons.FullscreenExit,
                            contentDescription = strings.exitFullscreen,
                            tint = Color.White,
                            modifier = Modifier.size(22.dp),
                        )
                    }
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = track.title,
                            style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                            color = Color.White,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = track.artist,
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.White.copy(alpha = 0.78f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Row(
                        modifier = Modifier
                            .clip(RoundedCornerShape(percent = 50))
                            .background(Color.White.copy(alpha = 0.18f))
                            .border(0.75.dp, Color.White.copy(alpha = 0.25f), RoundedCornerShape(percent = 50))
                            .clickable {
                                interactionTick++
                                onOpenQualityPicker()
                            }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Icon(
                            imageVector = SonataIcons.Tune,
                            contentDescription = strings.videoQuality,
                            tint = Color.White,
                            modifier = Modifier.size(15.dp),
                        )
                        Text(
                            text = qualityLabel.ifBlank { strings.videoQualityAuto },
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                            color = Color.White,
                        )
                    }
                }

                // Center transport controls
                Row(
                    modifier = Modifier.align(Alignment.Center),
                    horizontalArrangement = Arrangement.spacedBy(36.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(
                        onClick = {
                            interactionTick++
                            onPrevious()
                        },
                        enabled = hasPrevious,
                        modifier = Modifier.size(52.dp),
                    ) {
                        Icon(
                            painter = painterResource(R.drawable.ic_player_previous),
                            contentDescription = strings.previous,
                            tint = if (hasPrevious) Color.White else Color.White.copy(alpha = 0.35f),
                            modifier = Modifier.size(40.dp),
                        )
                    }
                    IconButton(
                        onClick = {
                            interactionTick++
                            onTogglePlayPause()
                        },
                        modifier = Modifier.size(68.dp),
                    ) {
                        Icon(
                            painter = painterResource(
                                if (isPlaying) R.drawable.ic_player_pause else R.drawable.ic_player_play,
                            ),
                            contentDescription = if (isPlaying) strings.pause else strings.play,
                            tint = Color.White,
                            modifier = Modifier.size(50.dp),
                        )
                    }
                    IconButton(
                        onClick = {
                            interactionTick++
                            onNext()
                        },
                        enabled = hasNext,
                        modifier = Modifier.size(52.dp),
                    ) {
                        Icon(
                            painter = painterResource(R.drawable.ic_player_next),
                            contentDescription = strings.next,
                            tint = if (hasNext) Color.White else Color.White.copy(alpha = 0.35f),
                            modifier = Modifier.size(40.dp),
                        )
                    }
                }

                // Bottom scrubber + timestamps + exit button
                Column(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth(),
                ) {
                    ThinSlider(
                        value = displayedFraction,
                        onValueChange = { frac ->
                            if (!scrubbing) scrubbing = true
                            scrubFraction = frac
                        },
                        onValueChangeFinished = {
                            onSeekFraction(scrubFraction)
                            scrubbing = false
                            interactionTick++
                        },
                        loading = isBuffering,
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = formatDurationMs(shownMs),
                            style = MaterialTheme.typography.labelMedium,
                            color = Color.White.copy(alpha = 0.85f),
                        )
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                text = "-${formatDurationMs(remainingMs)}",
                                style = MaterialTheme.typography.labelMedium,
                                color = Color.White.copy(alpha = 0.85f),
                            )
                            Spacer(Modifier.width(12.dp))
                            IconButton(
                                onClick = onExitFullscreen,
                                modifier = Modifier.size(32.dp),
                            ) {
                                Icon(
                                    imageVector = SonataIcons.FullscreenExit,
                                    contentDescription = strings.exitFullscreen,
                                    tint = Color.White,
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

private fun applyFitCenterTransform(
    textureView: TextureView,
    videoAspect: Float,
) {
    val viewW = textureView.width
    val viewH = textureView.height
    if (viewW <= 0 || viewH <= 0 || !videoAspect.isFinite() || videoAspect <= 0f) {
        textureView.setTransform(Matrix())
        return
    }
    val viewAspect = viewW.toFloat() / viewH.toFloat()
    val scaleX: Float
    val scaleY: Float
    if (videoAspect > viewAspect) {
        scaleX = 1f
        scaleY = viewAspect / videoAspect
    } else {
        scaleX = videoAspect / viewAspect
        scaleY = 1f
    }
    val matrix = Matrix().apply {
        setScale(scaleX, scaleY, viewW / 2f, viewH / 2f)
    }
    textureView.setTransform(matrix)
}
