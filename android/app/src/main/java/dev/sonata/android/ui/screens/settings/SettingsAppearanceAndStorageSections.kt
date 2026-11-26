/*
 * Appearance, Performance, Storage, Your Data, and Miscellaneous sections for Sonata Settings.
 */
package dev.sonata.android.ui.screens.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.isGlassSupported
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.EcosystemStrings
import dev.sonata.android.ui.i18n.LocalSonataStrings
import uniffi.sonata_android.AndroidStatsSummary
import java.util.Locale
import kotlin.math.roundToInt

@Composable
internal fun AppearanceCategoryContent(
    settings: SonataSettingsState,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onOpenChoice: (SettingsChoiceKind) -> Unit,
) {
    val strings = LocalSonataStrings.current
    val activeDesign = settings.resolvedDesignTheme()

    if (search.matches("Language", "Dil", "Язык", "Turkish", "Türkçe", "Russian", "Русский", "English", "System", "Appearance", "Görünüm")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.sectionLanguage,
            secondaryText = secondaryText,
        ) {
            SettingsRow(
                icon = SonataIcons.Translate,
                title = strings.appLanguage,
                subtitle = strings.appLanguageDesc,
                value = appLanguageLabel(settings.appLanguage, strings),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.APP_LANGUAGE) },
            )
        }
        Spacer(Modifier.height(14.dp))
    }

    if (search.matches("Theme", "Tema", "Basic", "Dark", "Light", "Appearance", "Görünüm")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.sectionTheme,
            footer = strings.footerAppearanceTheme,
            secondaryText = secondaryText,
        ) {
            addRow(dividerStart = ROW_INSET) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = ROW_INSET, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(strings.colorAppearance, style = MaterialTheme.typography.bodyLarge, color = primaryText)
                    val modes = listOf(
                        strings.themeModeSystem,
                        strings.themeModeLight,
                        strings.themeModeDark,
                    )
                    val selectedIdx = when (settings.themeMode) {
                        ThemeMode.SYSTEM -> 0
                        ThemeMode.LIGHT -> 1
                        ThemeMode.DARK -> 2
                    }
                    PillSegmentedControl(
                        options = modes,
                        selectedIndex = selectedIdx,
                        onSelect = { idx ->
                            val nextMode = when (idx) {
                                0 -> ThemeMode.SYSTEM
                                1 -> ThemeMode.LIGHT
                                else -> ThemeMode.DARK
                            }
                            onUpdateSettings { it.copy(designTheme = DesignTheme.BASIC, themeMode = nextMode) }
                        },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        Spacer(Modifier.height(14.dp))
    }

    if (search.matches("Liquid Glass", "Glass", "Dynamic Blur", "Blur", "Animation")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.sectionGlassMotion,
            secondaryText = secondaryText,
        ) {
            if (search.matches("Liquid Glass", "Glass")) {
                val glassApiSupported = isGlassSupported()
                SettingsSwitchRow(
                    title = strings.liquidGlass,
                    subtitle = if (glassApiSupported) {
                        strings.liquidGlassDesc
                    } else {
                        strings.liquidGlassApiHint
                    },
                    checked = settings.liquidGlassEnabled && glassApiSupported,
                    enabled = glassApiSupported,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(liquidGlassEnabled = enabled) } },
                )
            }
            if (search.matches("Glass Performance", "Glass", "Performance")) {
                SettingsSwitchRow(
                    title = strings.glassPerformanceMode,
                    subtitle = strings.glassPerformanceDesc,
                    checked = settings.glassPerformance,
                    enabled = settings.liquidGlassEnabled && isGlassSupported(),
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(glassPerformance = enabled) } },
                )
            }
            if (search.matches("Reduce Dynamic Blur", "Dynamic Blur", "Blur")) {
                SettingsSwitchRow(
                    title = strings.reduceDynamicBlur,
                    subtitle = strings.reduceDynamicBlurDesc,
                    checked = settings.reduceDynamicBlur,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceDynamicBlur = enabled) } },
                )
            }
            if (search.matches("Reduce Animation", "Animation", "Motion")) {
                SettingsSwitchRow(
                    title = strings.reduceAnimation,
                    subtitle = strings.reduceAnimationDesc,
                    checked = settings.reduceAnimation,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceAnimation = enabled) } },
                )
            }
        }
        Spacer(Modifier.height(14.dp))
    }

    if (search.matches(
            "Full-Screen",
            "Cover Art",
            "Detail Cover",
            "Total Duration",
            "Backdrop",
            "Animated",
            "Cellular",
            "Artwork",
            "Player",
        )
    ) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.sectionPlayerCovers,
            secondaryText = secondaryText,
        ) {
            if (search.matches("Full-Screen Cover Art", "Full-Screen", "Cover Art")) {
                SettingsSwitchRow(
                    title = strings.fullScreenCoverArt,
                    subtitle = strings.fullScreenCoverArtDesc,
                    checked = settings.fullScreenCoverArt,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(fullScreenCoverArt = enabled) } },
                )
            }
            if (search.matches("Full-Screen Detail Cover", "Detail Cover", "Album")) {
                SettingsSwitchRow(
                    title = strings.fullScreenDetailCover,
                    subtitle = strings.fullScreenDetailCoverDesc,
                    checked = settings.fullScreenDetailCover,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(fullScreenDetailCover = enabled) } },
                )
            }
            if (search.matches("Show Total Duration", "Total Duration", "Time", "Remaining")) {
                SettingsSwitchRow(
                    title = strings.showTotalDuration,
                    subtitle = strings.showTotalDurationDesc,
                    checked = settings.showTotalDuration,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(showTotalDuration = enabled) } },
                )
            }
            if (search.matches("Player Backdrop", "Backdrop", "Mesh", "Blur")) {
                SettingsRow(
                    icon = SonataIcons.Sparkles,
                    title = strings.playerBackdrop,
                    value = backdropLabel(settings.playerBackdropMode, strings),
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onClick = { onOpenChoice(SettingsChoiceKind.PLAYER_BACKDROP) },
                )
            }
            if (search.matches("Animated Cover Art", "Animated", "Canvas", "Motion")) {
                SettingsSwitchRow(
                    title = strings.animatedArtwork,
                    subtitle = strings.animatedArtworkDesc,
                    checked = settings.animatedArtwork,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(animatedArtwork = enabled) } },
                )
            }
            if (search.matches("Play Animated Cover Over Cellular", "Cellular", "Animated")) {
                SettingsSwitchRow(
                    title = strings.animatedArtworkCellular,
                    subtitle = strings.animatedArtworkCellularDesc,
                    checked = settings.animatedArtworkCellular,
                    enabled = settings.animatedArtwork,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(animatedArtworkCellular = enabled) } },
                )
            }
        }
        Spacer(Modifier.height(14.dp))
    }

    if (search.matches(
            "Lyrics",
            "Preview",
            "Blur",
            "Word-by-Word",
            "Translate",
            "Language",
            "Romanize",
            "Offset",
        )
    ) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.lyrics,
            secondaryText = secondaryText,
        ) {
            if (search.matches("Hide Lyrics Preview", "Preview", "Lyrics")) {
                SettingsSwitchRow(
                    title = strings.hideLyricsPreview,
                    subtitle = strings.hideLyricsPreviewDesc,
                    checked = settings.hideLyricsPreview,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideLyricsPreview = enabled) } },
                )
            }
            if (search.matches("Blur Unfocused Lyrics", "Blur", "Lyrics")) {
                SettingsSwitchRow(
                    title = strings.blurUnfocusedLyrics,
                    subtitle = strings.blurUnfocusedLyricsDesc,
                    checked = settings.lyricsBlurEnabled,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(lyricsBlurEnabled = enabled) } },
                )
            }
            if (search.matches("Word-by-Word", "Karaoke", "Sync", "Lyrics")) {
                SettingsSwitchRow(
                    title = strings.wordByWordSync,
                    subtitle = strings.wordByWordSyncDesc,
                    checked = settings.prioritizeWordSync,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(prioritizeWordSync = enabled) } },
                )
            }
            if (search.matches("Translate", "Translation", "Language", "Lyrics")) {
                SettingsSwitchRow(
                    title = strings.translateLyrics,
                    subtitle = strings.translateLyricsDesc,
                    checked = settings.lyricsTranslationEnabled,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(lyricsTranslationEnabled = enabled) } },
                )
                if (settings.lyricsTranslationEnabled) {
                    val langName = strings.translationLangName(settings.lyricsTranslationLang)
                    SettingsRow(
                        icon = SonataIcons.Translate,
                        title = strings.translationLanguage,
                        value = langName,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        onClick = { onOpenChoice(SettingsChoiceKind.LYRICS_LANGUAGE) },
                    )
                }
            }
            if (search.matches("Romanize", "Romanization", "Lyrics")) {
                SettingsSwitchRow(
                    title = strings.romanizeLyrics,
                    subtitle = strings.romanizeLyricsDesc,
                    checked = settings.lyricsRomanizationEnabled,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(lyricsRomanizationEnabled = enabled) } },
                )
            }
            if (search.matches("Global Lyric Offset", "Offset", "Timing", "Lyrics")) {
                addRow(dividerStart = ROW_INSET) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = ROW_INSET, vertical = 12.dp),
                    ) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(strings.globalLyricOffset, style = MaterialTheme.typography.bodyLarge, color = primaryText)
                            Text(
                                "${if (settings.lyricsOffsetMs >= 0) "+" else ""}${settings.lyricsOffsetMs} ms",
                                style = MaterialTheme.typography.labelLarge,
                                color = primaryText,
                            )
                        }
                        Slider(
                            value = settings.lyricsOffsetMs.toFloat(),
                            onValueChange = { v ->
                                val snapped = ((v / 50f).roundToInt() * 50L).coerceIn(-5000L, 5000L)
                                onUpdateSettings { it.copy(lyricsOffsetMs = snapped) }
                            },
                            valueRange = -2500f..2500f,
                            colors = SliderDefaults.colors(
                                thumbColor = accentTint,
                                activeTrackColor = accentTint,
                                inactiveTrackColor = dividerColor,
                            ),
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun PerformanceCategoryContent(
    settings: SonataSettingsState,
    supports120Hz: Boolean,
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
        header = strings.hdrDisplayRendering,
        footer = strings.footerDisplayRendering,
        secondaryText = secondaryText,
    ) {
        if (supports120Hz && search.matches("Refresh Rate", "120 Hz", "60 Hz", "FPS", "Performance")) {
            SettingsSwitchRow(
                title = strings.refreshRateHigh,
                subtitle = strings.refreshRateDesc,
                checked = settings.fpsLimit >= 120,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(fpsLimit = if (enabled) 120 else 60) }
                },
            )
        }
        if (search.matches("Glass Performance", "Glass", "Performance")) {
            SettingsSwitchRow(
                title = strings.glassPerformanceMode,
                subtitle = strings.glassPerformanceDesc,
                checked = settings.glassPerformance,
                enabled = settings.liquidGlassEnabled && isGlassSupported(),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(glassPerformance = enabled) } },
            )
        }
        if (search.matches("Reduce Dynamic Blur", "Blur", "Performance")) {
            SettingsSwitchRow(
                title = strings.reduceDynamicBlur,
                subtitle = strings.reduceDynamicBlurDesc,
                checked = settings.reduceDynamicBlur,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceDynamicBlur = enabled) } },
            )
        }
        if (search.matches("Reduce Animation", "Animation", "Motion")) {
            SettingsSwitchRow(
                title = strings.reduceAnimation,
                subtitle = strings.reduceAnimationDesc,
                checked = settings.reduceAnimation,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceAnimation = enabled) } },
            )
        }
    }
}

@Composable
internal fun StorageCategoryContent(
    downloadStorageStats: SonataDownloadManager.DownloadStorageStats,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onOpenDownloads: () -> Unit,
    onClearMediaCache: () -> Unit,
    onClearImageCache: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrStorageCache,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Downloaded", "Downloads", "Offline", "Storage")) {
            val localizedStorageLabel = if (downloadStorageStats.locationLabel.contains("External", ignoreCase = true)) {
                strings.storageExternalLabel
            } else {
                strings.storageInternalLabel
            }
            SettingsRow(
                icon = SonataIcons.Download,
                title = strings.downloadedSongs,
                subtitle = "${strings.tracksCount(downloadStorageStats.trackCount)} · $localizedStorageLabel",
                value = formatStorageBytes(downloadStorageStats.downloadedBytes),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = onOpenDownloads,
            )
        }
        if (search.matches("Audio Cache", "Media Cache", "Lyrics Cache", "Clear")) {
            SettingsActionRow(
                title = strings.clearAudioCache,
                subtitle = strings.clearAudioCacheDesc,
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onClearMediaCache,
            )
        }
        if (search.matches("Image Cache", "Artwork Cache", "Clear")) {
            SettingsActionRow(
                title = strings.clearImageCache,
                subtitle = strings.clearImageCacheDesc,
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onClearImageCache,
            )
        }
    }
}

@Composable
internal fun YourDataCategoryContent(
    eco: EcosystemStrings,
    statsSummary: AndroidStatsSummary?,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onOpenStats: () -> Unit,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = eco.listeningStatsTitle,
        footer = eco.statsPrivacyNote,
        secondaryText = secondaryText,
    ) {
        SettingsRow(
            icon = SonataIcons.Sparkles,
            title = eco.listeningStatsTitle,
            subtitle = eco.listeningStatsSubtitle,
            value = statsSummary?.let { "${it.totalPlays} · ${it.totalMinutes}m" } ?: "",
            primaryText = primaryText,
            secondaryText = secondaryText,
            onClick = onOpenStats,
        )
    }

    Spacer(Modifier.height(14.dp))

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrBackupRestore,
        footer = strings.footerBackupRestore,
        secondaryText = secondaryText,
    ) {
        SettingsActionRow(
            title = strings.exportBackup,
            subtitle = strings.exportBackupDesc,
            titleColor = primaryText,
            secondaryText = secondaryText,
            onClick = onExportBackup,
        )
        SettingsActionRow(
            title = strings.importBackup,
            subtitle = strings.importBackupDesc,
            titleColor = primaryText,
            secondaryText = secondaryText,
            onClick = onImportBackup,
        )
    }
}

@Composable
internal fun MiscellaneousCategoryContent(
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
        header = strings.hdrPlayerSystem,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Hide Status Bar", "Status Bar", "Player")) {
            SettingsSwitchRow(
                title = strings.hideStatusBarPlayer,
                subtitle = strings.hideStatusBarPlayerDesc,
                checked = settings.hideStatusBarInPlayer,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideStatusBarInPlayer = enabled) } },
            )
        }
        if (search.matches("Hide Quality Badge", "Quality Badge", "Song Status")) {
            SettingsSwitchRow(
                title = strings.hideQualityBadge,
                subtitle = strings.hideQualityBadgeDesc,
                checked = settings.hideSongStatus,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideSongStatus = enabled) } },
            )
        }
        if (search.matches("Stop Music on Close", "Stop Music", "Close", "Swipe")) {
            SettingsSwitchRow(
                title = strings.stopMusicOnClose,
                subtitle = strings.stopMusicOnCloseDesc,
                checked = settings.stopMusicOnClose,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(stopMusicOnClose = enabled) } },
            )
        }
        if (search.matches("Show Nerd Stats", "Nerd Stats", "Codec", "Bitrate", "Telemetry")) {
            SettingsSwitchRow(
                title = strings.showNerdStats,
                subtitle = strings.showNerdStatsDesc,
                checked = settings.showNerdStats,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(showNerdStats = enabled) } },
            )
        }
    }
}
