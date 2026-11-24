/*
 * Downloads, Playback, and Content Preferences sections for Sonata Settings.
 */
package dev.sonata.android.ui.screens.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.EcosystemStrings
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.theme.BasicPalette
import java.util.Locale
import kotlin.math.roundToInt

@Composable
internal fun DownloadsCategoryContent(
    settings: SonataSettingsState,
    downloadStorageStats: SonataDownloadManager.DownloadStorageStats,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onOpenChoice: (SettingsChoiceKind) -> Unit,
    onRequestClearAllDownloads: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrQualitySources,
        footer = strings.footerQualitySources,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Download Quality", "İndirme", "Загрузка", "Highest", "High", "Standard", "Data Saver", "MB")) {
            SettingsRow(
                icon = SonataIcons.Download,
                title = strings.downloadQuality,
                subtitle = settings.downloadQuality.approxMbPerSong,
                value = downloadQualityShortLabel(settings.downloadQuality, strings),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DOWNLOAD_QUALITY) },
            )
        }
        if (search.matches("Include JioSaavn", "JioSaavn", "320 kbps", "Highest")) {
            SettingsSwitchRow(
                title = strings.includeJioSaavnDownloads,
                subtitle = strings.includeJioSaavnDownloadsDesc,
                checked = settings.downloadIncludeJioSaavn,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(downloadIncludeJioSaavn = enabled) }
                },
            )
        }
        if (search.matches("Wi-Fi", "Cellular", "Network", "Hücresel")) {
            SettingsSwitchRow(
                title = strings.wifiOnlyDownloads,
                subtitle = strings.wifiOnlyDownloadsDesc,
                checked = settings.wifiOnlyDownloads,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(wifiOnlyDownloads = enabled) }
                },
            )
        }
        if (search.matches("Auto-download", "Liked", "Favorites", "Beğenilen")) {
            SettingsSwitchRow(
                title = strings.autoDownloadLiked,
                subtitle = strings.autoDownloadLikedDesc,
                checked = settings.autoDownloadLiked,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(autoDownloadLiked = enabled) }
                },
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    if (search.matches("Video", "Video Download", "1080p", "720p", "480p", "360p", "Video İndirme", "Видео")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.hdrVideoDownloads,
            footer = strings.footerVideoDownloads,
            secondaryText = secondaryText,
        ) {
            if (search.matches("Video", "Download videos", "Varsa videoları", "Скачивать видео")) {
                SettingsSwitchRow(
                    title = strings.downloadVideosIfAvailable,
                    subtitle = strings.downloadVideosIfAvailableDesc,
                    checked = settings.downloadVideosIfAvailable,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled ->
                        onUpdateSettings { it.copy(downloadVideosIfAvailable = enabled) }
                    },
                )
            }
            if (search.matches("Video", "Video Download", "1080p", "720p", "480p", "360p", "Video İndirme", "Качество загрузки видео")) {
                SettingsRow(
                    icon = SonataIcons.Tune,
                    title = strings.videoDownloadQuality,
                    subtitle = videoDownloadQualitySize(settings.videoDownloadQuality, strings),
                    value = "${settings.videoDownloadQuality.maxHeight}p",
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onClick = { onOpenChoice(SettingsChoiceKind.VIDEO_DOWNLOAD_QUALITY) },
                )
            }
        }

        Spacer(Modifier.height(14.dp))
    }

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrStorageLocation,
        footer = strings.footerStorageLocation,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Storage", "Depolama", "Хранилище", "Location", "Internal", "External", "Folder")) {
            SettingsRow(
                icon = SonataIcons.Folder,
                title = strings.downloadLocation,
                subtitle = downloadStorageDetail(settings.downloadStorageLocation, strings),
                value = downloadStorageShortLabel(settings.downloadStorageLocation, strings),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DOWNLOAD_STORAGE) },
            )
        }

        addRow(dividerStart = ROW_INSET) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = strings.downloadedStorage,
                        style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                        color = primaryText,
                    )
                    Text(
                        text = formatStorageBytes(downloadStorageStats.downloadedBytes),
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = primaryText,
                    )
                }
                val localizedStorageLabel = if (downloadStorageStats.locationLabel.contains("External", ignoreCase = true)) {
                    strings.storageExternalLabel
                } else {
                    strings.storageInternalLabel
                }
                Text(
                    text = "${strings.tracksCount(downloadStorageStats.trackCount)} · $localizedStorageLabel",
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                )
                val totalBytes = downloadStorageStats.totalBytes.coerceAtLeast(1L)
                val usedBytes = (downloadStorageStats.totalBytes - downloadStorageStats.availableBytes).coerceAtLeast(0L)
                val deviceUsedFrac = (usedBytes.toFloat() / totalBytes.toFloat()).coerceIn(0f, 1f)
                val downloadsFrac = if (downloadStorageStats.downloadedBytes > 0L) {
                    (downloadStorageStats.downloadedBytes.toFloat() / totalBytes.toFloat()).coerceIn(0.02f, 1f)
                } else {
                    0f
                }
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(8.dp)
                        .clip(CircleShape)
                        .background(dividerColor),
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth(deviceUsedFrac)
                            .height(8.dp)
                            .clip(CircleShape)
                            .background(secondaryText.copy(alpha = 0.35f)),
                    )
                    if (downloadsFrac > 0f) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth(downloadsFrac)
                                .height(8.dp)
                                .clip(CircleShape)
                                .background(accentTint),
                        )
                    }
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = "Sonata: ${formatStorageBytes(downloadStorageStats.downloadedBytes)}",
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                    )
                    Text(
                        text = strings.storageFreeOf(
                            formatStorageBytes(downloadStorageStats.availableBytes),
                            formatStorageBytes(downloadStorageStats.totalBytes),
                        ),
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                    )
                }
            }
        }

        if (downloadStorageStats.trackCount > 0) {
            SettingsActionRow(
                title = strings.removeAllDownloads,
                subtitle = strings.deleteAllOfflineSongs(downloadStorageStats.trackCount),
                titleColor = BasicPalette.AccentRed,
                secondaryText = secondaryText,
                onClick = onRequestClearAllDownloads,
            )
        }
    }
}

@Composable
internal fun PlaybackCategoryContent(
    settings: SonataSettingsState,
    eco: EcosystemStrings,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrQueueTransitions,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Gapless", "Aralıksız", "Непрерывное", "Queue", "Transitions")) {
            SettingsSwitchRow(
                title = eco.gaplessPlaybackTitle,
                subtitle = eco.gaplessPlaybackDescription,
                checked = settings.gaplessEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(gaplessEnabled = enabled) } },
            )
        }
        if (search.matches("Autoplay", "Sonsuz", "Бесконечный", "Queue", "Similar")) {
            SettingsSwitchRow(
                title = strings.autoplay,
                subtitle = strings.autoplayDesc,
                checked = settings.autoplayEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(autoplayEnabled = enabled) } },
            )
        }
        if (search.matches("Loudness", "Normalization", "Volume", "Ses", "Eşitleme", "Громкость", "Выравнивание")) {
            SettingsSwitchRow(
                title = strings.loudnessNormalization,
                subtitle = strings.loudnessNormalizationDesc,
                checked = settings.loudnessNormEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(loudnessNormEnabled = enabled) } },
            )
        }
        if (search.matches("Crossfade", "Blend", "Transitions", "Geçiş", "Кроссфейд")) {
            val effectiveSeconds = if (settings.crossfadeEnabled) settings.crossfadeSeconds.coerceIn(1, 12) else 0
            SettingsSwitchRow(
                title = eco.crossfadeTitle,
                subtitle = if (effectiveSeconds > 0) eco.crossfadeSecondsValue(effectiveSeconds) else eco.crossfadeOff,
                checked = settings.crossfadeEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(crossfadeEnabled = enabled) } },
            )
            addRow(dividerStart = ROW_INSET) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = ROW_INSET, vertical = 10.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(eco.crossfadeDescription, style = MaterialTheme.typography.bodySmall, color = secondaryText, modifier = Modifier.weight(1f))
                        Spacer(Modifier.width(8.dp))
                        Text(
                            eco.crossfadeSecondsValue(effectiveSeconds),
                            style = MaterialTheme.typography.labelMedium,
                            color = primaryText,
                        )
                    }
                    Slider(
                        value = effectiveSeconds.toFloat(),
                        onValueChange = { value ->
                            val sec = value.roundToInt().coerceIn(0, 12)
                            onUpdateSettings { current ->
                                if (sec == 0) {
                                    current.copy(crossfadeEnabled = false)
                                } else {
                                    current.copy(crossfadeEnabled = true, crossfadeSeconds = sec)
                                }
                            }
                        },
                        valueRange = 0f..12f,
                        steps = 11,
                        colors = SliderDefaults.colors(
                            thumbColor = accentTint,
                            activeTrackColor = accentTint,
                            inactiveTrackColor = dividerColor,
                        ),
                    )
                    Text(
                        text = eco.crossfadeVideoBypassHint,
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                    )
                }
            }
        }
        if (search.matches("Skip Silence", "Silence", "Sessizliği", "Тишина")) {
            SettingsSwitchRow(
                title = strings.skipSilence,
                subtitle = strings.skipSilenceDesc,
                checked = settings.skipSilenceEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(skipSilenceEnabled = enabled) } },
            )
        }
        if (search.matches("Sleep Timer", "Fade Out", "Fade", "Uyku", "Таймер")) {
            SettingsSwitchRow(
                title = strings.sleepTimerFadeOut,
                subtitle = strings.sleepTimerFadeOutDesc,
                checked = settings.sleepTimerFadeOut,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(sleepTimerFadeOut = enabled) } },
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    if (search.matches("Speed", "Tempo", "Hız", "Скорость", "Equalizer", "EQ", "Audio")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.hdrSpeedAndTiming,
            secondaryText = secondaryText,
        ) {
            addRow(dividerStart = ROW_INSET) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = ROW_INSET, vertical = 12.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(strings.playbackSpeed, style = MaterialTheme.typography.bodyLarge, color = primaryText)
                        Text(
                            String.format(Locale.US, "%.2fx", settings.playbackSpeed),
                            style = MaterialTheme.typography.labelLarge,
                            color = primaryText,
                        )
                    }
                    Slider(
                        value = settings.playbackSpeed,
                        onValueChange = { v ->
                            val snapped = ((v * 20f).roundToInt() / 20f).coerceIn(0.5f, 2.0f)
                            onUpdateSettings { it.copy(playbackSpeed = snapped) }
                        },
                        valueRange = 0.5f..2.0f,
                        colors = SliderDefaults.colors(
                            thumbColor = accentTint,
                            activeTrackColor = accentTint,
                            inactiveTrackColor = dividerColor,
                        ),
                    )
                }
            }

            SettingsSwitchRow(
                title = strings.equalizerTitle,
                subtitle = if (settings.eqEnabled) {
                    eqPresetLabel(settings.eqPreset, strings)
                } else {
                    strings.off
                },
                checked = settings.eqEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(eqEnabled = enabled) } },
            )

            if (settings.eqEnabled) {
                addRow(dividerStart = ROW_INSET) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = ROW_INSET, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            EqPresets.take(3).forEach { preset ->
                                EqPresetChip(
                                    label = eqPresetLabel(preset, strings),
                                    selected = settings.eqPreset == preset,
                                    cardBg = dividerColor,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    onClick = {
                                        onUpdateSettings {
                                            it.copy(eqPreset = preset, eqBandsDb = eqPresetCurve(preset))
                                        }
                                    },
                                    modifier = Modifier.weight(1f),
                                )
                            }
                        }
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            EqPresets.drop(3).forEach { preset ->
                                EqPresetChip(
                                    label = eqPresetLabel(preset, strings),
                                    selected = settings.eqPreset == preset,
                                    cardBg = dividerColor,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    onClick = {
                                        onUpdateSettings {
                                            it.copy(eqPreset = preset, eqBandsDb = eqPresetCurve(preset))
                                        }
                                    },
                                    modifier = Modifier.weight(1f),
                                )
                            }
                        }
                        val bandLabels = listOf("60 Hz", "230 Hz", "910 Hz", "3.6 kHz", "14 kHz")
                        bandLabels.forEachIndexed { idx, freq ->
                            val gain = settings.eqBandsDb.getOrElse(idx) { 0f }
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text(
                                    text = freq,
                                    style = MaterialTheme.typography.labelSmall,
                                    color = secondaryText,
                                    modifier = Modifier.width(50.dp),
                                )
                                Slider(
                                    value = gain,
                                    onValueChange = { next ->
                                        val updated = settings.eqBandsDb.toMutableList().apply {
                                            while (size < 5) add(0f)
                                            this[idx] = (next * 2f).roundToInt() / 2f
                                        }
                                        onUpdateSettings { it.copy(eqPreset = "Custom", eqBandsDb = updated) }
                                    },
                                    valueRange = -12f..12f,
                                    modifier = Modifier.weight(1f),
                                    colors = SliderDefaults.colors(
                                        thumbColor = accentTint,
                                        activeTrackColor = accentTint,
                                        inactiveTrackColor = dividerColor,
                                    ),
                                )
                                Text(
                                    text = String.format(Locale.US, "%+.1f", gain),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = primaryText,
                                    modifier = Modifier.width(42.dp),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun ContentPreferencesCategoryContent(
    settings: SonataSettingsState,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrHomeFeed,
        footer = strings.footerHomeFeed,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Hide Recents", "Recents", "Recently Played", "Last Played", "Son")) {
            SettingsSwitchRow(
                title = strings.hideRecents,
                subtitle = strings.hideRecentsDesc,
                checked = settings.hideRecents,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideRecents = enabled) } },
            )
        }
        if (search.matches("Hide Top Picks", "Top Picks", "Featured", "Önerilen", "Рекомендации")) {
            SettingsSwitchRow(
                title = strings.hideTopPicks,
                subtitle = strings.hideTopPicksDesc,
                checked = settings.hideTopPicks,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideTopPicks = enabled) } },
            )
        }
        if (search.matches("Hide Find Your Mood", "Mood", "Moods", "Ruh Hali", "Настроение")) {
            SettingsSwitchRow(
                title = strings.hideHomeMoods,
                subtitle = strings.hideHomeMoodsDesc,
                checked = settings.hideHomeMoods,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideHomeMoods = enabled) } },
            )
        }
        if (search.matches("Hide Home Videos", "Videos", "Music Videos", "Video", "Видео")) {
            SettingsSwitchRow(
                title = strings.hideHomeVideos,
                subtitle = strings.hideHomeVideosDesc,
                checked = settings.hideHomeVideos,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideHomeVideos = enabled) } },
            )
        }
        if (search.matches("Square Video Thumbnails", "Square", "Thumbnails", "Videos", "Kare", "Квадратные")) {
            SettingsSwitchRow(
                title = strings.squareVideoThumbnails,
                subtitle = strings.squareVideoThumbnailsDesc,
                checked = settings.squareVideoThumbnails,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(squareVideoThumbnails = enabled) } },
            )
        }
    }
}
