/*
 * Reusable monochrome inset-grouped settings primitives, search matcher,
 * route/choice enums, and localized label formatters.
 */
package dev.sonata.android.ui.screens.settings

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadStorageLocation
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.VideoDownloadQuality
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.EcosystemStrings
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.i18n.SourcesStrings
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.screens.SettingsCategory
import java.util.Locale

internal val GroupShape = RoundedCornerShape(14.dp)
internal val GROUP_INSET = 16.dp
internal val ROW_INSET = 16.dp
internal val ICON_SIZE = 22.dp
internal val ICON_GAP = 14.dp
internal val TEXT_INSET = ROW_INSET + ICON_SIZE + ICON_GAP
internal val IosSwitchGreen = Color(0xFF34C759)

internal enum class SettingsChoiceKind(val title: String) {
    STREAMING_QUALITY("Streaming Quality"),
    DOWNLOAD_QUALITY("Download Quality"),
    VIDEO_DOWNLOAD_QUALITY("Video Download Quality"),
    DOWNLOAD_STORAGE("Storage Location"),
    APP_LANGUAGE("Language"),
    DESIGN_THEME("Theme"),
    PLAYER_BACKDROP("Player Backdrop"),
    LYRICS_LANGUAGE("Translation Language"),
    DISCORD_ACTIVITY_TYPE("Activity Type"),
}

internal fun SettingsChoiceKind.localizedTitle(strings: SonataStrings): String = when (this) {
    SettingsChoiceKind.STREAMING_QUALITY -> strings.streamingQuality
    SettingsChoiceKind.DOWNLOAD_QUALITY -> strings.downloadQuality
    SettingsChoiceKind.VIDEO_DOWNLOAD_QUALITY -> strings.videoDownloadQuality
    SettingsChoiceKind.DOWNLOAD_STORAGE -> strings.downloadLocation
    SettingsChoiceKind.APP_LANGUAGE -> strings.appLanguage
    SettingsChoiceKind.DESIGN_THEME -> strings.designTheme
    SettingsChoiceKind.PLAYER_BACKDROP -> strings.playerBackdrop
    SettingsChoiceKind.LYRICS_LANGUAGE -> strings.translationLanguage
    SettingsChoiceKind.DISCORD_ACTIVITY_TYPE -> strings.discordActivityType
}

internal fun SettingsCategory.localizedTitle(
    strings: SonataStrings,
    sources: SourcesStrings,
    eco: EcosystemStrings,
): String = when (this) {
    SettingsCategory.SOURCES -> sources.sourcesTitle
    SettingsCategory.DOWNLOADS -> strings.catDownloads
    SettingsCategory.PLAYBACK -> strings.catPlayback
    SettingsCategory.CONNECT -> eco.connectTitle
    SettingsCategory.SCROBBLING -> eco.scrobblingTitle
    SettingsCategory.CONTENT -> strings.catContent
    SettingsCategory.APPEARANCE -> strings.catAppearance
    SettingsCategory.PERFORMANCE -> strings.catPerformance
    SettingsCategory.STORAGE -> strings.catStorage
    SettingsCategory.YOUR_DATA -> strings.catYourData
    SettingsCategory.MISC -> strings.catMisc
    SettingsCategory.DISCORD -> strings.catDiscord
    SettingsCategory.ACCOUNT -> strings.catAccountUpdates
}

internal fun appLanguageLabel(lang: AppLanguage, strings: SonataStrings): String = when (lang) {
    AppLanguage.SYSTEM -> strings.langSystem
    AppLanguage.ENGLISH -> strings.langEnglish
    AppLanguage.TURKISH -> strings.langTurkish
    AppLanguage.RUSSIAN -> strings.langRussian
}

internal fun appLanguageDescription(lang: AppLanguage, strings: SonataStrings): String = when (lang) {
    AppLanguage.SYSTEM -> strings.langSystemDesc
    AppLanguage.ENGLISH -> strings.langEnglishDesc
    AppLanguage.TURKISH -> strings.langTurkishDesc
    AppLanguage.RUSSIAN -> strings.langRussianDesc
}

internal sealed interface SettingsRoute {
    data object Root : SettingsRoute
    data class Category(val category: SettingsCategory) : SettingsRoute
    data class Choice(val kind: SettingsChoiceKind, val parent: SettingsRoute) : SettingsRoute
}

internal val EqPresets = listOf("Flat", "Bass Boost", "Vocal", "Acoustic", "Electronic", "Late Night")
internal val TranslationLangs = listOf(
    "en" to "English",
    "tr" to "Turkish",
    "es" to "Spanish",
    "ja" to "Japanese",
    "ko" to "Korean",
    "de" to "German",
)

@Composable
internal fun SettingsPageTitle(
    title: String,
    color: Color,
) {
    Text(
        text = title,
        style = MaterialTheme.typography.displayLarge.copy(
            fontSize = 32.sp,
            fontWeight = FontWeight.ExtraBold,
            letterSpacing = (-0.5).sp,
        ),
        color = color,
        modifier = Modifier.padding(start = GROUP_INSET + 4.dp, end = GROUP_INSET, top = 4.dp, bottom = 14.dp),
    )
}

@Composable
internal fun SettingsSearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    cardBg: Color,
    primaryText: Color,
    secondaryText: Color,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = GROUP_INSET)
            .padding(bottom = 16.dp)
            .height(42.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(cardBg)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = SonataIcons.Search,
            contentDescription = null,
            tint = secondaryText,
            modifier = Modifier.size(18.dp),
        )
        Spacer(Modifier.width(8.dp))
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (query.isEmpty()) {
                Text(
                    text = LocalSonataStrings.current.searchSettingsPlaceholder,
                    style = MaterialTheme.typography.bodyMedium,
                    color = secondaryText,
                )
            }
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium.copy(color = primaryText),
                cursorBrush = SolidColor(primaryText),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (query.isNotEmpty()) {
            Icon(
                imageVector = SonataIcons.Close,
                contentDescription = LocalSonataStrings.current.clear,
                tint = secondaryText,
                modifier = Modifier
                    .size(18.dp)
                    .clickable { onQueryChange("") },
            )
        }
    }
}

@Composable
internal fun SettingsGroup(
    cardBg: Color,
    dividerColor: Color,
    secondaryText: Color,
    header: String? = null,
    footer: String? = null,
    content: @Composable SettingsGroupScope.() -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = GROUP_INSET),
    ) {
        if (!header.isNullOrBlank()) {
            Text(
                text = header.uppercase(Locale.US),
                style = MaterialTheme.typography.labelSmall.copy(
                    fontWeight = FontWeight.SemiBold,
                    letterSpacing = 0.6.sp,
                ),
                color = secondaryText,
                modifier = Modifier.padding(start = 12.dp, bottom = 7.dp),
            )
        }
        val items = mutableListOf<Pair<Dp, @Composable () -> Unit>>()
        val scope = object : SettingsGroupScope {
            override fun addRow(dividerStart: Dp, row: @Composable () -> Unit) {
                items.add(dividerStart to row)
            }
        }
        scope.content()
        if (items.isNotEmpty()) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(GroupShape)
                    .background(cardBg),
            ) {
                items.forEachIndexed { idx, (dividerStart, row) ->
                    row()
                    if (idx < items.lastIndex) {
                        HorizontalDivider(
                            color = dividerColor,
                            thickness = 0.5.dp,
                            modifier = Modifier.padding(start = dividerStart),
                        )
                    }
                }
            }
        }
        if (!footer.isNullOrBlank()) {
            Text(
                text = footer,
                style = MaterialTheme.typography.bodySmall.copy(lineHeight = 17.sp),
                color = secondaryText,
                modifier = Modifier.padding(start = 12.dp, end = 12.dp, top = 7.dp),
            )
        }
    }
}

internal interface SettingsGroupScope {
    fun addRow(dividerStart: Dp = ROW_INSET, row: @Composable () -> Unit)
}

@Composable
internal fun SettingsGroupScope.SettingsRow(
    icon: ImageVector,
    title: String,
    primaryText: Color,
    secondaryText: Color,
    subtitle: String? = null,
    value: String? = null,
    avatarUrl: String? = null,
    showChevron: Boolean = true,
    onClick: (() -> Unit)?,
) {
    addRow(dividerStart = TEXT_INSET) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 52.dp)
                .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
                .padding(horizontal = ROW_INSET, vertical = 11.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (!avatarUrl.isNullOrBlank()) {
                val req = rememberArtworkRequest(avatarUrl, 96, 96)
                Box(
                    modifier = Modifier
                        .size(28.dp)
                        .clip(CircleShape)
                        .background(secondaryText.copy(alpha = 0.2f)),
                    contentAlignment = Alignment.Center,
                ) {
                    if (req != null) {
                        AsyncImage(
                            model = req,
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))
            } else {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = primaryText,
                    modifier = Modifier.size(ICON_SIZE),
                )
                Spacer(Modifier.width(ICON_GAP))
            }
            Column(Modifier.weight(1f)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = primaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (!subtitle.isNullOrBlank()) {
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (!value.isNullOrBlank()) {
                Spacer(Modifier.width(8.dp))
                Text(
                    text = value,
                    style = MaterialTheme.typography.bodyMedium,
                    color = secondaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (showChevron && onClick != null) {
                Spacer(Modifier.width(6.dp))
                Icon(
                    imageVector = SonataIcons.ChevronRight,
                    contentDescription = null,
                    tint = secondaryText.copy(alpha = 0.7f),
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

@Composable
internal fun SettingsGroupScope.SettingsSwitchRow(
    title: String,
    subtitle: String? = null,
    checked: Boolean,
    enabled: Boolean = true,
    primaryText: Color,
    secondaryText: Color,
    onCheckedChange: (Boolean) -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    addRow(dividerStart = ROW_INSET) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 52.dp)
                .alpha(if (enabled) 1f else 0.45f)
                .clickable(enabled = enabled) {
                    haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onCheckedChange(!checked)
                }
                .padding(horizontal = ROW_INSET, vertical = 11.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = primaryText,
                )
                if (!subtitle.isNullOrBlank()) {
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
            }
            Spacer(Modifier.width(12.dp))
            MonochromeIosSwitch(
                checked = checked,
                enabled = enabled,
                onCheckedChange = { next ->
                    haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onCheckedChange(next)
                },
            )
        }
    }
}

@Composable
internal fun SettingsGroupScope.SettingsActionRow(
    title: String,
    subtitle: String? = null,
    titleColor: Color,
    secondaryText: Color,
    onClick: () -> Unit,
) {
    addRow(dividerStart = ROW_INSET) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 50.dp)
                .clickable(onClick = onClick)
                .padding(horizontal = ROW_INSET, vertical = 12.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                color = titleColor,
            )
            if (!subtitle.isNullOrBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                )
            }
        }
    }
}

@Composable
internal fun SettingsGroupScope.SettingsChoiceRow(
    title: String,
    subtitle: String? = null,
    badge: String? = null,
    selected: Boolean,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onClick: () -> Unit,
) {
    addRow(dividerStart = ROW_INSET) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 54.dp)
                .clickable(onClick = onClick)
                .padding(horizontal = ROW_INSET, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = title,
                        style = MaterialTheme.typography.bodyLarge.copy(
                            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                        ),
                        color = primaryText,
                    )
                    if (!badge.isNullOrBlank()) {
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(6.dp))
                                .background(primaryText.copy(alpha = 0.09f))
                                .padding(horizontal = 7.dp, vertical = 2.dp),
                        ) {
                            Text(
                                text = badge,
                                style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = secondaryText,
                            )
                        }
                    }
                }
                if (!subtitle.isNullOrBlank()) {
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
            }
            if (selected) {
                Spacer(Modifier.width(10.dp))
                Icon(
                    imageVector = SonataIcons.Check,
                    contentDescription = LocalSonataStrings.current.done,
                    tint = accentTint,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }
}

@Composable
internal fun MonochromeIosSwitch(
    checked: Boolean,
    enabled: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    val trackColor by animateColorAsState(
        targetValue = if (checked) IosSwitchGreen else Color(0xFF787880).copy(alpha = 0.36f),
        animationSpec = tween(180),
        label = "iosSwitchTrack",
    )
    val thumbOffset by animateDpAsState(
        targetValue = if (checked) 22.dp else 2.dp,
        animationSpec = tween(180),
        label = "iosSwitchThumb",
    )
    Box(
        modifier = Modifier
            .width(50.dp)
            .height(30.dp)
            .clip(CircleShape)
            .background(trackColor)
            .clickable(enabled = enabled) { onCheckedChange(!checked) },
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                .offset(x = thumbOffset)
                .size(26.dp)
                .clip(CircleShape)
                .background(Color.White)
                .border(0.5.dp, Color.Black.copy(alpha = 0.08f), CircleShape),
        )
    }
}

@Composable
internal fun EqPresetChip(
    label: String,
    selected: Boolean,
    cardBg: Color,
    primaryText: Color,
    secondaryText: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .height(34.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) primaryText else cardBg)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            color = if (selected) {
                if (primaryText == Color.White) Color.Black else Color.White
            } else {
                secondaryText
            },
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

internal class SettingsSearchMatcher(rawQuery: String) {
    private val terms = rawQuery.trim().lowercase(Locale.US)
        .split(Regex("\\s+"))
        .filter { it.isNotEmpty() }

    val active: Boolean get() = terms.isNotEmpty()

    fun matches(vararg haystacks: String): Boolean {
        if (!active) return true
        val combined = haystacks.joinToString(" ").lowercase(Locale.US)
        return terms.all { it in combined }
    }
}

internal fun formatStorageBytes(bytes: Long): String {
    if (bytes <= 0L) return "0 MB"
    val kb = bytes / 1024.0
    val mb = kb / 1024.0
    val gb = mb / 1024.0
    return when {
        gb >= 1.0 -> String.format(Locale.US, "%.2f GB", gb)
        mb >= 1.0 -> String.format(Locale.US, "%.1f MB", mb)
        else -> String.format(Locale.US, "%.0f KB", kb.coerceAtLeast(1.0))
    }
}

internal fun audioQualityShortLabel(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLossless
    AudioQualityTier.HIGH -> strings.qualityHigh
    AudioQualityTier.MEDIUM -> strings.qualityMedium
    AudioQualityTier.LOW -> strings.qualityLow
}

internal fun audioQualityLabel(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLosslessDetail
    AudioQualityTier.HIGH -> strings.qualityHighDetail
    AudioQualityTier.MEDIUM -> strings.qualityMediumDetail
    AudioQualityTier.LOW -> strings.qualityLowDetail
}

internal fun audioQualityDetail(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLosslessDesc
    AudioQualityTier.HIGH -> strings.qualityHighDesc
    AudioQualityTier.MEDIUM -> strings.qualityMediumDesc
    AudioQualityTier.LOW -> strings.qualityLowDesc
}

internal fun downloadQualityShortLabel(tier: DownloadQualityTier, strings: SonataStrings): String = when (tier) {
    DownloadQualityTier.HIGHEST -> strings.dlHighest
    DownloadQualityTier.HIGH -> strings.dlHigh
    DownloadQualityTier.STANDARD -> strings.dlStandard
    DownloadQualityTier.LOW -> strings.dlLow
}

internal fun videoDownloadQualityTitle(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Title
    VideoDownloadQuality.P720 -> strings.videoDl720Title
    VideoDownloadQuality.P480 -> strings.videoDl480Title
    VideoDownloadQuality.P360 -> strings.videoDl360Title
}

internal fun videoDownloadQualityDesc(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Desc
    VideoDownloadQuality.P720 -> strings.videoDl720Desc
    VideoDownloadQuality.P480 -> strings.videoDl480Desc
    VideoDownloadQuality.P360 -> strings.videoDl360Desc
}

internal fun videoDownloadQualitySize(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Size
    VideoDownloadQuality.P720 -> strings.videoDl720Size
    VideoDownloadQuality.P480 -> strings.videoDl480Size
    VideoDownloadQuality.P360 -> strings.videoDl360Size
}

internal fun downloadStorageShortLabel(loc: DownloadStorageLocation, strings: SonataStrings): String = when (loc) {
    DownloadStorageLocation.INTERNAL -> strings.storageLocationInternal
    DownloadStorageLocation.EXTERNAL -> strings.storageLocationExternal
}

internal fun downloadStorageDetail(loc: DownloadStorageLocation, strings: SonataStrings): String = when (loc) {
    DownloadStorageLocation.INTERNAL -> strings.storageInternalDesc
    DownloadStorageLocation.EXTERNAL -> strings.storageExternalDesc
}

internal fun designThemeLabel(design: DesignTheme): String = when (design) {
    DesignTheme.VIOLET -> "Violet"
    DesignTheme.MOONFLOWER -> "Moonflower"
    DesignTheme.BASIC -> "Basic"
}

internal fun designThemeDescription(design: DesignTheme, strings: SonataStrings): String = when (design) {
    DesignTheme.VIOLET -> strings.themeVioletDesc
    DesignTheme.MOONFLOWER -> strings.themeMoonflowerDesc
    DesignTheme.BASIC -> strings.themeBasicDesc
}

internal fun backdropLabel(mode: PlayerBackdropMode, strings: SonataStrings): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> strings.backdropMesh
    PlayerBackdropMode.AMBIENT_WAVES -> strings.backdropWaves
    PlayerBackdropMode.BLURRED_COVER -> strings.backdropBlurred
}

internal fun backdropDetail(mode: PlayerBackdropMode, strings: SonataStrings): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> strings.backdropMeshDesc
    PlayerBackdropMode.AMBIENT_WAVES -> strings.backdropWavesDesc
    PlayerBackdropMode.BLURRED_COVER -> strings.backdropBlurredDesc
}

internal fun eqPresetLabel(preset: String, strings: SonataStrings): String = when (preset) {
    "Flat" -> strings.eqPresetFlat
    "Bass Boost" -> strings.eqPresetBassBoost
    "Vocal" -> strings.eqPresetVocal
    "Acoustic" -> strings.eqPresetAcoustic
    "Electronic" -> strings.eqPresetElectronic
    "Late Night" -> strings.eqPresetLateNight
    "Custom" -> strings.eqPresetCustom
    else -> preset
}

internal fun eqPresetCurve(preset: String): List<Float> = when (preset) {
    "Bass Boost" -> listOf(5.5f, 3.5f, 0.5f, 0.0f, 1.0f)
    "Vocal" -> listOf(-1.5f, 1.0f, 4.0f, 3.0f, 0.5f)
    "Acoustic" -> listOf(2.0f, 1.5f, 1.0f, 2.5f, 2.0f)
    "Electronic" -> listOf(4.5f, 2.0f, -1.0f, 2.5f, 4.0f)
    "Late Night" -> listOf(-2.0f, 1.0f, 2.0f, 1.0f, -2.5f)
    else -> listOf(0f, 0f, 0f, 0f, 0f)
}
