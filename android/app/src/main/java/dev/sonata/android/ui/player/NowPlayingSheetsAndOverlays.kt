/*
 * Now Playing lyrics panel, queue panel, modal/dialog overlays, and update changelog dialog.
 */
package dev.sonata.android.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil3.compose.AsyncImage
import dev.sonata.android.ChangelogEntry
import dev.sonata.android.LocalNowPlayingExtras
import dev.sonata.android.Pill50Shape
import dev.sonata.android.PlaybackClockState
import dev.sonata.android.R
import dev.sonata.android.Shape10
import dev.sonata.android.Shape7
import dev.sonata.android.SonataUiState
import dev.sonata.android.backdrop.backdrops.LayerBackdrop
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.parseChangelogEntries
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.lyrics.EnhancedLyricsPanel
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidLyricLine
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo
import kotlin.math.roundToInt

@Composable
internal fun NowPlayingLyricsPanel(
    lyrics: List<AndroidLyricLine>,
    lyricsLoading: Boolean,
    clockMs: State<Long>,
    clock: PlaybackClockState,
    onSeekTo: (Long) -> Unit,
) {
    val extras = LocalNowPlayingExtras.current
    EnhancedLyricsPanel(
        trackId = extras.trackId,
        lyrics = lyrics,
        lyricsLoading = lyricsLoading,
        clockMs = clockMs,
        clock = clock,
        lyricsBlurEnabled = extras.settings.lyricsBlurEnabled,
        translationEnabled = extras.settings.lyricsTranslationEnabled,
        translationLang = extras.settings.lyricsTranslationLang,
        romanizationEnabled = extras.settings.lyricsRomanizationEnabled,
        offsetMs = extras.settings.lyricsOffsetMs + extras.trackLyricOffsetMs,
        sourceLabel = extras.activeLyricsSource,
        onToggleTranslation = extras.onToggleLyricsTranslation,
        onToggleRomanization = extras.onToggleLyricsRomanization,
        onStepOffsetMs = extras.onStepLyricOffsetMs,
        onSeekTo = onSeekTo,
        modifier = Modifier.fillMaxSize(),
    )
}

@Composable
internal fun NowPlayingQueuePanel(
    queue: List<AndroidTrack>,
    queueIndex: Int,
    isPlaying: Boolean,
    onSelectIndex: (Int) -> Unit,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        item(key = "queue_header", contentType = "queue_header") {
            Text(
                text = LocalSonataStrings.current.upNext,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontSize = 18.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = Color.White,
                modifier = Modifier.padding(vertical = 6.dp),
            )
        }
        itemsIndexed(
            items = queue,
            key = { idx, t -> "q_${t.id}_$idx" },
            contentType = { _, _ -> "queue_track" },
        ) { idx, item ->
            val isCurrent = idx == queueIndex
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(Shape10)
                    .background(if (isCurrent) Color.White.copy(alpha = 0.14f) else Color.Transparent)
                    .clickable { onSelectIndex(idx) }
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(Shape7)
                        .background(Color.White.copy(alpha = 0.1f)),
                    contentAlignment = Alignment.Center,
                ) {
                    val thumb = remember(item.thumbnailUrl) { item.thumbnailUrl.artworkAt(ROW_ART_PX) }
                    val thumbRequest = rememberArtworkRequest(thumb, ROW_ART_PX, ROW_ART_PX)
                    if (thumbRequest != null) {
                        AsyncImage(
                            model = thumbRequest,
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.6f),
                        )
                    }
                    if (isCurrent) {
                        Box(
                            modifier = Modifier
                                .fillMaxSize()
                                .background(Color.Black.copy(alpha = 0.45f)),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = if (isPlaying) SonataIcons.GraphicEq else SonataIcons.Pause,
                                contentDescription = null,
                                tint = sonataThemeAccent(),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = item.title,
                        style = MaterialTheme.typography.titleSmall.copy(
                            fontSize = 15.sp,
                            fontWeight = if (isCurrent) FontWeight.Bold else FontWeight.Medium,
                        ),
                        color = if (isCurrent) sonataThemeAccent() else Color.White,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = item.artist,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.White.copy(alpha = 0.65f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
internal fun NowPlayingOverlays(
    track: AndroidTrack,
    state: SonataUiState,
    settings: SonataSettingsState,
    clock: PlaybackClockState,
    isLiked: Boolean,
    isTrackDownloaded: Boolean,
    activeTrackDownload: ActiveDownload?,
    sleepTimerRemaining: Int?,
    trackLyricOffsetMs: Long,
    audioPipelineSummary: String?,
    connectDeviceName: String? = null,
    nowPlayingBackdrop: LayerBackdrop,
    showPlayerOptions: Boolean,
    showVideoWarningDialog: Boolean,
    showVideoQualityDialog: Boolean,
    onDismissOptions: () -> Unit,
    onOpenConnectDevices: () -> Unit = {},
    onToggleStar: () -> Unit,
    onAddToPlaylist: () -> Unit,
    onShareInApp: () -> Unit,
    onOpenAudioControls: () -> Unit,
    onOpenAudioPipeline: () -> Unit,
    onDownloadTrack: () -> Unit,
    onOpenSleepTimer: () -> Unit,
    onStepLyricOffsetMs: (Long) -> Unit,
    onResetLyricOffsetMs: () -> Unit,
    onToggleLyricsRomanization: () -> Unit,
    onToggleLyricsTranslation: () -> Unit,
    onStartRadio: () -> Unit,
    onPlayNext: () -> Unit,
    onAddToQueue: () -> Unit,
    onOpenAlbum: () -> Unit,
    onOpenArtist: () -> Unit,
    onTogglePlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onSeekTo: (Long) -> Unit,
    onSeekFraction: (Float) -> Unit,
    onOpenVideoQualityDialog: () -> Unit,
    onDismissVideoQualityDialog: () -> Unit,
    onExitVideoFullscreen: () -> Unit,
    onSuppressVideoWarning: (Boolean) -> Unit,
    onConfirmVideoMode: () -> Unit,
    onDismissVideoWarning: () -> Unit,
    onSelectVideoQuality: (VideoQualityPreference, Int?) -> Unit,
) {
    // Now Playing `⋯` Options floating Liquid Glass menu (sibling OUTSIDE nowPlayingBackdrop — zero RenderNode cycle)
    NowPlayingOptionsMenu(
        visible = showPlayerOptions,
        track = track,
        isLiked = isLiked,
        isDownloaded = isTrackDownloaded,
        activeDownload = activeTrackDownload,
        sleepTimerRemainingMinutes = sleepTimerRemaining,
        lyricOffsetMs = trackLyricOffsetMs,
        romanizationEnabled = settings.lyricsRomanizationEnabled,
        translationEnabled = settings.lyricsTranslationEnabled,
        pipelineSummary = audioPipelineSummary,
        connectDeviceName = connectDeviceName,
        appLanguage = settings.appLanguage,
        backdrop = nowPlayingBackdrop,
        onDismiss = onDismissOptions,
        onOpenConnectDevices = onOpenConnectDevices,
        onToggleLike = onToggleStar,
        onAddToPlaylist = onAddToPlaylist,
        onShareInApp = onShareInApp,
        onOpenAudioControls = onOpenAudioControls,
        onOpenAudioPipeline = onOpenAudioPipeline,
        onDownloadTrack = onDownloadTrack,
        onOpenSleepTimer = onOpenSleepTimer,
        onStepLyricOffsetMs = onStepLyricOffsetMs,
        onResetLyricOffset = onResetLyricOffsetMs,
        onToggleRomanization = onToggleLyricsRomanization,
        onToggleTranslation = onToggleLyricsTranslation,
        onStartRadio = onStartRadio,
        onPlayNext = onPlayNext,
        onAddToQueue = onAddToQueue,
        onOpenAlbum = onOpenAlbum,
        onOpenArtist = onOpenArtist,
    )

    if (state.videoModeEnabled && state.videoFullscreen) {
        FullscreenMusicVideoOverlay(
            track = track,
            isPlaying = state.isPlaying,
            isBuffering = state.isBuffering || state.videoLoading,
            hasNext = state.hasNext,
            hasPrevious = state.hasPrevious(clock.positionMs),
            clock = clock,
            qualityLabel = state.activeVideoQualityLabel,
            suspendAutoHide = showVideoQualityDialog,
            onTogglePlayPause = onTogglePlayPause,
            onNext = onNext,
            onPrevious = onPrevious,
            onSeekTo = onSeekTo,
            onSeekFraction = onSeekFraction,
            onOpenQualityPicker = onOpenVideoQualityDialog,
            onExitFullscreen = onExitVideoFullscreen,
        )
    }

    if (showVideoWarningDialog) {
        VideoAudioQualityWarningDialog(
            onConfirm = { dontShowAgain ->
                if (dontShowAgain) {
                    onSuppressVideoWarning(true)
                }
                onDismissVideoWarning()
                onConfirmVideoMode()
            },
            onDismiss = onDismissVideoWarning,
        )
    }

    if (showVideoQualityDialog) {
        VideoQualityPickerDialog(
            qualities = state.availableVideoQualities,
            selectedPreference = settings.videoQuality,
            activeHeight = state.activeVideoQualityHeight,
            onSelect = onSelectVideoQuality,
            onDismiss = onDismissVideoQualityDialog,
        )
    }
}

@Composable
internal fun NowPlayingUpdateDialog(
    info: AndroidUpdateInfo,
    downloading: Boolean,
    progress: Float?,
    detail: String,
    error: String?,
    hasDownloadedApk: Boolean,
    onDownloadOrInstall: () -> Unit,
    onCancelDownload: () -> Unit,
    onDismiss: () -> Unit,
) {
    val changelogEntries = remember(info.releaseNotes) {
        parseChangelogEntries(info.releaseNotes)
    }
    val strings = LocalSonataStrings.current
    val themeAccent = sonataThemeAccent()
    val dialogSurface = MaterialTheme.colorScheme.surface
    val dialogInk = MaterialTheme.colorScheme.onSurface
    val dialogMuted = MaterialTheme.colorScheme.onSurfaceVariant
    val dialogBorder = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)
    Dialog(
        onDismissRequest = { if (!downloading) onDismiss() },
        properties = DialogProperties(
            dismissOnBackPress = !downloading,
            dismissOnClickOutside = !downloading,
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(24.dp))
                .background(dialogSurface)
                .border(1.dp, dialogBorder, RoundedCornerShape(24.dp))
                .padding(22.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(themeAccent.copy(alpha = 0.18f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painter = painterResource(id = R.drawable.ic_sonata_moon),
                        contentDescription = null,
                        tint = themeAccent,
                        modifier = Modifier.size(24.dp),
                    )
                }
                Column(Modifier.weight(1f)) {
                    Text(
                        text = strings.updateAvailable,
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                        color = dialogInk,
                    )
                    Text(
                        text = "v${info.currentVersion}  →  v${info.latestVersion}",
                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
                        color = themeAccent,
                    )
                }
            }

            if (changelogEntries.isNotEmpty() && !downloading) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 260.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f))
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    changelogEntries.forEachIndexed { idx, entry ->
                        when (entry) {
                            is ChangelogEntry.Heading -> {
                                if (idx > 0) Spacer(Modifier.height(4.dp))
                                Text(
                                    text = entry.title.uppercase(),
                                    style = MaterialTheme.typography.labelSmall.copy(
                                        fontWeight = FontWeight.Bold,
                                        letterSpacing = 0.8.sp,
                                    ),
                                    color = themeAccent,
                                )
                            }
                            is ChangelogEntry.Bullet -> {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                                    verticalAlignment = Alignment.Top,
                                ) {
                                    Text(
                                        text = "•",
                                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.Bold),
                                        color = themeAccent,
                                    )
                                    val formatted = buildAnnotatedString {
                                        if (!entry.lead.isNullOrBlank()) {
                                            pushStyle(
                                                SpanStyle(
                                                    fontWeight = FontWeight.SemiBold,
                                                    color = dialogInk,
                                                ),
                                            )
                                            append(entry.lead)
                                            pop()
                                            if (entry.body.isNotBlank()) {
                                                append(" — ")
                                            }
                                        }
                                        pushStyle(
                                            SpanStyle(
                                                color = dialogMuted,
                                            ),
                                        )
                                        append(entry.body)
                                        pop()
                                    }
                                    Text(
                                        text = formatted,
                                        style = MaterialTheme.typography.bodySmall.copy(lineHeight = 18.sp),
                                        modifier = Modifier.weight(1f),
                                    )
                                }
                            }
                            is ChangelogEntry.Note -> {
                                Text(
                                    text = entry.text,
                                    style = MaterialTheme.typography.bodySmall.copy(lineHeight = 18.sp),
                                    color = dialogMuted,
                                )
                            }
                        }
                    }
                }
            }

            if (downloading) {
                val themeTokens = LocalLunariaColors.current
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (progress != null) {
                        LinearProgressIndicator(
                            progress = { progress.coerceIn(0f, 1f) },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = themeTokens.petalLavender,
                            trackColor = themeTokens.onSurfacePrimary.copy(alpha = 0.14f),
                        )
                    } else {
                        LinearProgressIndicator(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(Pill50Shape),
                            color = themeTokens.petalLavender,
                            trackColor = themeTokens.onSurfacePrimary.copy(alpha = 0.14f),
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = detail.ifBlank { strings.downloadingApk },
                            style = MaterialTheme.typography.labelMedium,
                            color = dialogMuted,
                        )
                        if (progress != null) {
                            Text(
                                text = "${(progress * 100f).roundToInt()}%",
                                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                                color = dialogInk,
                            )
                        }
                    }
                }
            } else if (detail.isNotBlank() && error == null) {
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = dialogMuted,
                )
            }

            if (!error.isNullOrBlank()) {
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFFFF6B6B),
                )
            }

            val themeTokens = LocalLunariaColors.current
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(46.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(themeTokens.eclipseCard)
                        .clickable {
                            if (downloading) onCancelDownload() else onDismiss()
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = if (downloading) strings.cancel else if (hasDownloadedApk) strings.close else strings.later,
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = dialogInk,
                    )
                }
                if (!downloading) {
                    val primaryBtnInk = if (themeTokens.petalLavender.luminance() > 0.55f) Color.Black else Color.White
                    Box(
                        modifier = Modifier
                            .weight(1.35f)
                            .height(46.dp)
                            .clip(RoundedCornerShape(14.dp))
                            .background(themeTokens.petalLavender)
                            .clickable(onClick = onDownloadOrInstall),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = if (hasDownloadedApk) strings.installNow else strings.downloadNow,
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                            color = primaryBtnInk,
                        )
                    }
                }
            }
        }
    }
}
