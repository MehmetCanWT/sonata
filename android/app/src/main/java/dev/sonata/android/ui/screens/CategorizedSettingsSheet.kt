/*
 * Monochrome Inset-Grouped Settings Screen for Sonata Android.
 *
 * Implements an iOS-inspired pure monochrome inset-grouped settings hierarchy
 * consistent across all design themes (pure black/white canvas, neutral surface
 * cards, hairline inset dividers, and green iOS switches):
 * - Root screen with large "Settings" display title, live search across all
 *   categories, pinned Account & Audio Quality groups, and drill-down categories.
 * - Full working subpages for Downloads (quality tiers with MB/song estimates,
 *   Include JioSaavn, Internal/External storage selector, live usage card),
 *   Playback, Content Preferences, Appearance & Lyrics, Performance, Storage,
 *   Your Data, Miscellaneous, and Account & Updates.
 */
package dev.sonata.android.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.isGlassSupported
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidUpdateInfo
import java.util.Locale
import kotlin.math.roundToInt

// ---- Inset-grouped settings geometry constants ------------------------------

private val GroupShape = RoundedCornerShape(14.dp)
private val GROUP_INSET = 16.dp
private val ROW_INSET = 16.dp
private val ICON_SIZE = 22.dp
private val ICON_GAP = 14.dp
private val TEXT_INSET = ROW_INSET + ICON_SIZE + ICON_GAP
private val IosSwitchGreen = Color(0xFF34C759)

enum class SettingsCategory(
    val title: String,
    val icon: ImageVector,
) {
    DOWNLOADS("Downloads", SonataIcons.Download),
    PLAYBACK("Playback", SonataIcons.Play),
    CONTENT("Content Preferences", SonataIcons.TabExplore),
    APPEARANCE("Appearance", SonataIcons.Sparkles),
    PERFORMANCE("Performance", SonataIcons.GraphicEq),
    STORAGE("Storage", SonataIcons.Folder),
    YOUR_DATA("Your Data", SonataIcons.Download),
    MISC("Miscellaneous", SonataIcons.Settings),
    ACCOUNT("Account & Updates", SonataIcons.Person),
}

private enum class SettingsChoiceKind(val title: String) {
    STREAMING_QUALITY("Streaming Quality"),
    DOWNLOAD_QUALITY("Download Quality"),
    DOWNLOAD_STORAGE("Storage Location"),
    APP_LANGUAGE("Language"),
    DESIGN_THEME("Theme"),
    PLAYER_BACKDROP("Player Backdrop"),
    LYRICS_LANGUAGE("Translation Language"),
}

private fun SettingsCategory.localizedTitle(strings: SonataStrings): String = when (this) {
    SettingsCategory.DOWNLOADS -> strings.catDownloads
    SettingsCategory.PLAYBACK -> strings.catPlayback
    SettingsCategory.CONTENT -> strings.catContent
    SettingsCategory.APPEARANCE -> strings.catAppearance
    SettingsCategory.PERFORMANCE -> strings.catPerformance
    SettingsCategory.STORAGE -> strings.catStorage
    SettingsCategory.YOUR_DATA -> strings.catYourData
    SettingsCategory.MISC -> strings.catMisc
    SettingsCategory.ACCOUNT -> strings.catAccountUpdates
}

private fun appLanguageLabel(lang: AppLanguage, strings: SonataStrings): String = when (lang) {
    AppLanguage.SYSTEM -> strings.langSystem
    AppLanguage.ENGLISH -> strings.langEnglish
    AppLanguage.TURKISH -> strings.langTurkish
    AppLanguage.RUSSIAN -> strings.langRussian
}

private fun appLanguageDescription(lang: AppLanguage, strings: SonataStrings): String = when (lang) {
    AppLanguage.SYSTEM -> strings.langSystemDesc
    AppLanguage.ENGLISH -> strings.langEnglishDesc
    AppLanguage.TURKISH -> strings.langTurkishDesc
    AppLanguage.RUSSIAN -> strings.langRussianDesc
}

private sealed interface SettingsRoute {
    data object Root : SettingsRoute
    data class Category(val category: SettingsCategory) : SettingsRoute
    data class Choice(val kind: SettingsChoiceKind, val parent: SettingsRoute) : SettingsRoute
}

private val EqPresets = listOf("Flat", "Bass Boost", "Vocal", "Acoustic", "Electronic", "Late Night")
private val TranslationLangs = listOf(
    "en" to "English",
    "tr" to "Turkish",
    "es" to "Spanish",
    "ja" to "Japanese",
    "ko" to "Korean",
    "de" to "German",
)

/**
 * Full-screen monochrome grouped Settings experience for Sonata Android.
 */
@Composable
fun SonataSettingsScreen(
    settings: SonataSettingsState,
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    supports120Hz: Boolean,
    statusMessage: String,
    updateInfo: AndroidUpdateInfo?,
    checkingUpdates: Boolean,
    installedVersionName: String,
    downloadStorageStats: SonataDownloadManager.DownloadStorageStats = SonataDownloadManager.DownloadStorageStats(
        trackCount = 0,
        downloadedBytes = 0L,
        availableBytes = 0L,
        totalBytes = 0L,
        locationLabel = "Internal · Private App Storage",
    ),
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onClearMediaCache: () -> Unit,
    onClearImageCache: () -> Unit = {},
    onClearAllDownloads: () -> Unit = {},
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val statusTop = rememberSafeTopInset()
    var route by remember { mutableStateOf<SettingsRoute>(SettingsRoute.Root) }
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var confirmClearDownloads by remember { mutableStateOf(false) }

    val navigateBack: () -> Unit = {
        when (val current = route) {
            SettingsRoute.Root -> onBack()
            is SettingsRoute.Category -> route = SettingsRoute.Root
            is SettingsRoute.Choice -> route = current.parent
        }
    }

    BackHandler(enabled = route != SettingsRoute.Root) {
        navigateBack()
    }

    // Pure neutral monochrome palette regardless of active app theme
    val pageBg = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val cardBg = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val dividerColor = if (tokens.isDark) BasicPalette.DarkOutline else BasicPalette.LightOutline
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant
    val accentTint = primaryText

    if (confirmClearDownloads) {
        AlertDialog(
            onDismissRequest = { confirmClearDownloads = false },
            containerColor = cardBg,
            titleContentColor = primaryText,
            textContentColor = secondaryText,
            title = { Text("Remove all downloads?") },
            text = {
                Text(
                    "This deletes ${downloadStorageStats.trackCount} downloaded songs " +
                        "(${formatStorageBytes(downloadStorageStats.downloadedBytes)}) from offline storage.",
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmClearDownloads = false
                        onClearAllDownloads()
                    },
                ) {
                    Text("Remove", color = BasicPalette.AccentRed, fontWeight = FontWeight.SemiBold)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmClearDownloads = false }) {
                    Text("Cancel", color = primaryText)
                }
            },
        )
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(pageBg)
            .padding(top = statusTop)
            .navigationBarsPadding(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 680.dp)
                .fillMaxSize(),
        ) {
            // Compact top navigation bar with neutral back pill
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(40.dp)
                        .clip(CircleShape)
                        .background(cardBg)
                        .clickable(onClick = navigateBack),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.ChevronLeft,
                        contentDescription = "Back",
                        tint = primaryText,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Spacer(Modifier.width(12.dp))
                val topCaption = when (val current = route) {
                    SettingsRoute.Root -> ""
                    is SettingsRoute.Category -> "Settings"
                    is SettingsRoute.Choice -> when (val p = current.parent) {
                        SettingsRoute.Root -> "Settings"
                        is SettingsRoute.Category -> p.category.title
                        is SettingsRoute.Choice -> p.kind.title
                    }
                }
                if (topCaption.isNotEmpty()) {
                    Text(
                        text = topCaption,
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Medium),
                        color = secondaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.clickable(onClick = navigateBack),
                    )
                }
                Spacer(Modifier.weight(1f))
                if (statusMessage.isNotBlank() && route == SettingsRoute.Root) {
                    Text(
                        text = "v$installedVersionName",
                        style = MaterialTheme.typography.labelMedium,
                        color = secondaryText,
                    )
                }
            }

            AnimatedContent(
                targetState = route,
                transitionSpec = {
                    val forward = targetState != SettingsRoute.Root &&
                        (initialState == SettingsRoute.Root || targetState is SettingsRoute.Choice)
                    if (forward) {
                        (slideInHorizontally(tween(240)) { it / 5 } + fadeIn(tween(200))) togetherWith
                            (slideOutHorizontally(tween(200)) { -it / 8 } + fadeOut(tween(140)))
                    } else {
                        (slideInHorizontally(tween(240)) { -it / 5 } + fadeIn(tween(200))) togetherWith
                            (slideOutHorizontally(tween(200)) { it / 8 } + fadeOut(tween(140)))
                    }
                },
                label = "sonataSettingsRoute",
                modifier = Modifier.weight(1f),
            ) { activeRoute ->
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(bottom = 36.dp),
                ) {
                    when (activeRoute) {
                        is SettingsRoute.Choice -> {
                            SettingsChoicePage(
                                kind = activeRoute.kind,
                                settings = settings,
                                cardBg = cardBg,
                                dividerColor = dividerColor,
                                primaryText = primaryText,
                                secondaryText = secondaryText,
                                accentTint = accentTint,
                                onUpdateSettings = onUpdateSettings,
                                onSelectDone = navigateBack,
                            )
                        }

                        SettingsRoute.Root, is SettingsRoute.Category -> {
                            val strings = LocalSonataStrings.current
                            val activeCategory = (activeRoute as? SettingsRoute.Category)?.category
                            val search = remember(searchQuery, activeCategory) {
                                SettingsSearchMatcher(if (activeCategory == null) searchQuery else "")
                            }

                            val pageTitle = activeCategory?.localizedTitle(strings) ?: strings.settingsTitle
                            SettingsPageTitle(title = pageTitle, color = primaryText)

                            if (activeCategory == null) {
                                SettingsSearchField(
                                    query = searchQuery,
                                    onQueryChange = { searchQuery = it },
                                    cardBg = cardBg,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                )

                                // Pinned Account group
                                if (search.matches("Account", "Hesap", "YouTube Music", "Sign in", "Sign out", "Profile")) {
                                    SettingsGroup(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        header = strings.account,
                                        secondaryText = secondaryText,
                                    ) {
                                        SettingsRow(
                                            icon = SonataIcons.Person,
                                            title = if (signedIn) {
                                                accountProfile?.name?.takeIf { it.isNotBlank() } ?: "YouTube Music"
                                            } else {
                                                "YouTube Music"
                                            },
                                            subtitle = if (signedIn) {
                                                accountProfile?.handle?.takeIf { it.isNotBlank() } ?: strings.account
                                            } else {
                                                strings.signInBannerSubtitle
                                            },
                                            avatarUrl = if (signedIn) accountProfile?.avatarUrl else null,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onClick = { route = SettingsRoute.Category(SettingsCategory.ACCOUNT) },
                                        )
                                    }
                                    Spacer(Modifier.height(14.dp))
                                }

                                // Pinned Audio Quality & Downloads Quick Access group
                                if (search.matches(
                                        "Audio Quality",
                                        "Ses Kalitesi",
                                        "Streaming",
                                        "Download Quality",
                                        "JioSaavn",
                                        "320 kbps",
                                        "Lossless",
                                        "MB",
                                    )
                                ) {
                                    SettingsGroup(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        header = strings.sectionAudioQuality,
                                        secondaryText = secondaryText,
                                    ) {
                                        if (search.matches("Streaming", "Yayın", "Audio Quality", "Lossless", "High", "Medium")) {
                                            SettingsRow(
                                                icon = SonataIcons.GraphicEq,
                                                title = strings.streamingQuality,
                                                value = audioQualityShortLabel(settings.audioQuality),
                                                primaryText = primaryText,
                                                secondaryText = secondaryText,
                                                onClick = {
                                                    route = SettingsRoute.Choice(
                                                        SettingsChoiceKind.STREAMING_QUALITY,
                                                        SettingsRoute.Root,
                                                    )
                                                },
                                            )
                                        }
                                        if (search.matches("Download Quality", "İndirme", "Offline", "320 kbps", "MB")) {
                                            SettingsRow(
                                                icon = SonataIcons.Download,
                                                title = strings.downloadQuality,
                                                value = "${downloadQualityShortLabel(settings.downloadQuality)} · ${settings.downloadQuality.approxMbPerSong}",
                                                primaryText = primaryText,
                                                secondaryText = secondaryText,
                                                onClick = {
                                                    route = SettingsRoute.Choice(
                                                        SettingsChoiceKind.DOWNLOAD_QUALITY,
                                                        SettingsRoute.Root,
                                                    )
                                                },
                                            )
                                        }
                                        if (search.matches("JioSaavn", "Hi-Fi", "320 kbps", "Include")) {
                                            SettingsSwitchRow(
                                                title = strings.jioSaavnFallback,
                                                subtitle = strings.jioSaavnFallbackDesc,
                                                checked = settings.jioSaavnEnabled,
                                                primaryText = primaryText,
                                                secondaryText = secondaryText,
                                                onCheckedChange = { enabled ->
                                                    onUpdateSettings { it.copy(jioSaavnEnabled = enabled) }
                                                },
                                            )
                                        }
                                    }
                                    Spacer(Modifier.height(14.dp))
                                }

                                // Grouped Category List
                                if (!search.active) {
                                    SettingsGroup(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        secondaryText = secondaryText,
                                    ) {
                                        SettingsCategory.entries.forEach { cat ->
                                            val subtitle = when (cat) {
                                                SettingsCategory.DOWNLOADS ->
                                                    "${strings.tracksCount(downloadStorageStats.trackCount)} · ${formatStorageBytes(downloadStorageStats.downloadedBytes)}"
                                                SettingsCategory.PLAYBACK -> strings.playbackSummary
                                                SettingsCategory.CONTENT -> strings.contentSummary
                                                SettingsCategory.APPEARANCE ->
                                                    "${designThemeLabel(settings.resolvedDesignTheme())} · ${appLanguageLabel(settings.appLanguage, strings)}"
                                                SettingsCategory.PERFORMANCE -> strings.performanceSummary
                                                SettingsCategory.STORAGE -> strings.storageSummary
                                                SettingsCategory.YOUR_DATA -> strings.yourDataSummary
                                                SettingsCategory.MISC -> strings.miscSummary
                                                SettingsCategory.ACCOUNT ->
                                                    "v$installedVersionName${if (updateInfo?.updateAvailable == true) " · Update" else ""}"
                                            }
                                            SettingsRow(
                                                icon = cat.icon,
                                                title = cat.localizedTitle(strings),
                                                subtitle = subtitle,
                                                primaryText = primaryText,
                                                secondaryText = secondaryText,
                                                onClick = { route = SettingsRoute.Category(cat) },
                                            )
                                        }
                                    }
                                } else {
                                    // Live Search Results across all categories
                                    var matchCount = 0

                                    if (search.matches(
                                            "Downloads",
                                            "Download Quality",
                                            "JioSaavn",
                                            "Wi-Fi",
                                            "Storage",
                                            "Internal",
                                            "External",
                                            "Auto-download",
                                            "Liked",
                                            "MB",
                                        )
                                    ) {
                                        matchCount++
                                        DownloadsCategoryContent(
                                            settings = settings,
                                            downloadStorageStats = downloadStorageStats,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            accentTint = accentTint,
                                            onUpdateSettings = onUpdateSettings,
                                            onOpenChoice = { kind -> route = SettingsRoute.Choice(kind, SettingsRoute.Root) },
                                            onRequestClearAllDownloads = { confirmClearDownloads = true },
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches(
                                            "Playback",
                                            "Autoplay",
                                            "Loudness",
                                            "Normalization",
                                            "Crossfade",
                                            "Skip Silence",
                                            "Sleep Timer",
                                            "Fade Out",
                                            "Speed",
                                            "Equalizer",
                                            "EQ",
                                        )
                                    ) {
                                        matchCount++
                                        PlaybackCategoryContent(
                                            settings = settings,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            accentTint = accentTint,
                                            onUpdateSettings = onUpdateSettings,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches(
                                            "Content",
                                            "Recents",
                                            "Top Picks",
                                            "Mood",
                                            "Videos",
                                            "Square",
                                            "Thumbnails",
                                            "Hide",
                                            "Home",
                                        )
                                    ) {
                                        matchCount++
                                        ContentPreferencesCategoryContent(
                                            settings = settings,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onUpdateSettings = onUpdateSettings,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches(
                                            "Appearance",
                                            "Görünüm",
                                            "Language",
                                            "Dil",
                                            "Язык",
                                            "Turkish",
                                            "Türkçe",
                                            "Russian",
                                            "Русский",
                                            "English",
                                            "Theme",
                                            "Tema",
                                            "Violet",
                                            "Moonflower",
                                            "Basic",
                                            "Dark",
                                            "Light",
                                            "AMOLED",
                                            "Liquid Glass",
                                            "Dynamic Blur",
                                            "Blur",
                                            "Animation",
                                            "Full-Screen",
                                            "Cover Art",
                                            "Detail Cover",
                                            "Total Duration",
                                            "Backdrop",
                                            "Animated",
                                            "Cellular",
                                            "Lyrics",
                                            "Preview",
                                            "Word-by-Word",
                                            "Translate",
                                            "Romanize",
                                            "Offset",
                                        )
                                    ) {
                                        matchCount++
                                        AppearanceCategoryContent(
                                            settings = settings,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            accentTint = accentTint,
                                            onUpdateSettings = onUpdateSettings,
                                            onOpenChoice = { kind -> route = SettingsRoute.Choice(kind, SettingsRoute.Root) },
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches("Performance", "Refresh Rate", "120 Hz", "60 Hz", "FPS", "Glass")) {
                                        matchCount++
                                        PerformanceCategoryContent(
                                            settings = settings,
                                            supports120Hz = supports120Hz,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onUpdateSettings = onUpdateSettings,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches("Storage", "Cache", "Audio Cache", "Image Cache", "Clear")) {
                                        matchCount++
                                        StorageCategoryContent(
                                            downloadStorageStats = downloadStorageStats,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onOpenDownloads = { route = SettingsRoute.Category(SettingsCategory.DOWNLOADS) },
                                            onClearMediaCache = onClearMediaCache,
                                            onClearImageCache = onClearImageCache,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches("Backup", "Export", "Import", "JSON", "Data")) {
                                        matchCount++
                                        YourDataCategoryContent(
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onExportBackup = onExportBackup,
                                            onImportBackup = onImportBackup,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches(
                                            "Miscellaneous",
                                            "Status Bar",
                                            "Quality Badge",
                                            "Stop Music",
                                            "Close",
                                            "Nerd Stats",
                                            "Telemetry",
                                        )
                                    ) {
                                        matchCount++
                                        MiscellaneousCategoryContent(
                                            settings = settings,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onUpdateSettings = onUpdateSettings,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches("Account", "Update", "Version", "Sign in", "Sign out", "GitHub")) {
                                        matchCount++
                                        AccountCategoryContent(
                                            signedIn = signedIn,
                                            accountProfile = accountProfile,
                                            installedVersionName = installedVersionName,
                                            updateInfo = updateInfo,
                                            checkingUpdates = checkingUpdates,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onSignIn = onSignIn,
                                            onSignOut = onSignOut,
                                            onCheckForUpdates = onCheckForUpdates,
                                            onDownloadUpdate = onDownloadUpdate,
                                        )
                                    }

                                    if (matchCount == 0 &&
                                        !search.matches("Account", "Audio Quality", "Streaming", "Download Quality", "JioSaavn")
                                    ) {
                                        Box(
                                            modifier = Modifier
                                                .fillMaxWidth()
                                                .padding(vertical = 48.dp, horizontal = 24.dp),
                                            contentAlignment = Alignment.Center,
                                        ) {
                                            Text(
                                                text = "No settings match \"$searchQuery\"",
                                                style = MaterialTheme.typography.bodyMedium,
                                                color = secondaryText,
                                            )
                                        }
                                    }
                                }
                            } else {
                                when (activeCategory) {
                                    SettingsCategory.DOWNLOADS -> DownloadsCategoryContent(
                                        settings = settings,
                                        downloadStorageStats = downloadStorageStats,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        accentTint = accentTint,
                                        onUpdateSettings = onUpdateSettings,
                                        onOpenChoice = { kind ->
                                            route = SettingsRoute.Choice(kind, SettingsRoute.Category(activeCategory))
                                        },
                                        onRequestClearAllDownloads = { confirmClearDownloads = true },
                                    )

                                    SettingsCategory.PLAYBACK -> PlaybackCategoryContent(
                                        settings = settings,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        accentTint = accentTint,
                                        onUpdateSettings = onUpdateSettings,
                                    )

                                    SettingsCategory.CONTENT -> ContentPreferencesCategoryContent(
                                        settings = settings,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onUpdateSettings = onUpdateSettings,
                                    )

                                    SettingsCategory.APPEARANCE -> AppearanceCategoryContent(
                                        settings = settings,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        accentTint = accentTint,
                                        onUpdateSettings = onUpdateSettings,
                                        onOpenChoice = { kind ->
                                            route = SettingsRoute.Choice(kind, SettingsRoute.Category(activeCategory))
                                        },
                                    )

                                    SettingsCategory.PERFORMANCE -> PerformanceCategoryContent(
                                        settings = settings,
                                        supports120Hz = supports120Hz,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onUpdateSettings = onUpdateSettings,
                                    )

                                    SettingsCategory.STORAGE -> StorageCategoryContent(
                                        downloadStorageStats = downloadStorageStats,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onOpenDownloads = { route = SettingsRoute.Category(SettingsCategory.DOWNLOADS) },
                                        onClearMediaCache = onClearMediaCache,
                                        onClearImageCache = onClearImageCache,
                                    )

                                    SettingsCategory.YOUR_DATA -> YourDataCategoryContent(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onExportBackup = onExportBackup,
                                        onImportBackup = onImportBackup,
                                    )

                                    SettingsCategory.MISC -> MiscellaneousCategoryContent(
                                        settings = settings,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onUpdateSettings = onUpdateSettings,
                                    )

                                    SettingsCategory.ACCOUNT -> AccountCategoryContent(
                                        signedIn = signedIn,
                                        accountProfile = accountProfile,
                                        installedVersionName = installedVersionName,
                                        updateInfo = updateInfo,
                                        checkingUpdates = checkingUpdates,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onSignIn = onSignIn,
                                        onSignOut = onSignOut,
                                        onCheckForUpdates = onCheckForUpdates,
                                        onDownloadUpdate = onDownloadUpdate,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---- Category Subpages ------------------------------------------------------

@Composable
private fun DownloadsCategoryContent(
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
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Quality & Sources",
        footer = "Average song size assumes a ~3.5 minute track. Enabling Include JioSaavn lets offline downloads pick 320 kbps AAC when higher than YouTube Music.",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Download Quality", "Highest", "High", "Standard", "Data Saver", "MB")) {
            SettingsRow(
                icon = SonataIcons.Download,
                title = "Download Quality",
                subtitle = settings.downloadQuality.approxMbPerSong,
                value = downloadQualityShortLabel(settings.downloadQuality),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DOWNLOAD_QUALITY) },
            )
        }
        if (search.matches("Include JioSaavn", "JioSaavn", "320 kbps", "Highest")) {
            SettingsSwitchRow(
                title = "Include JioSaavn",
                subtitle = "Search JioSaavn 320 kbps AAC when resolving highest-quality downloads",
                checked = settings.downloadIncludeJioSaavn,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(downloadIncludeJioSaavn = enabled) }
                },
            )
        }
        if (search.matches("Wi-Fi", "Cellular", "Network")) {
            SettingsSwitchRow(
                title = "Download over Wi-Fi only",
                subtitle = "Avoid mobile data usage for offline tracks",
                checked = settings.wifiOnlyDownloads,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(wifiOnlyDownloads = enabled) }
                },
            )
        }
        if (search.matches("Auto-download", "Liked", "Favorites")) {
            SettingsSwitchRow(
                title = "Auto-download liked songs",
                subtitle = "Save tracks offline automatically when starred",
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

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Storage Location & Space",
        footer = "Existing downloads remain playable from their original folder. New downloads save to the selected storage location.",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Storage", "Location", "Internal", "External", "Folder")) {
            SettingsRow(
                icon = SonataIcons.Folder,
                title = "Download Location",
                subtitle = settings.downloadStorageLocation.detail,
                value = if (settings.downloadStorageLocation == DownloadStorageLocation.EXTERNAL) {
                    "External"
                } else {
                    "Internal"
                },
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DOWNLOAD_STORAGE) },
            )
        }

        // Live downloaded storage footprint & free space meter
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
                    text = "Downloaded Storage",
                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                    color = primaryText,
                )
                Text(
                    text = formatStorageBytes(downloadStorageStats.downloadedBytes),
                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                    color = primaryText,
                )
            }
            Text(
                text = "${downloadStorageStats.trackCount} ${if (downloadStorageStats.trackCount == 1) "song" else "songs"} · ${downloadStorageStats.locationLabel}",
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
                    text = "${formatStorageBytes(downloadStorageStats.availableBytes)} free of ${formatStorageBytes(downloadStorageStats.totalBytes)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = secondaryText,
                )
            }
        }
        }

        if (downloadStorageStats.trackCount > 0) {
            SettingsActionRow(
                title = "Remove All Downloads",
                subtitle = "Delete all ${downloadStorageStats.trackCount} offline songs",
                titleColor = BasicPalette.AccentRed,
                secondaryText = secondaryText,
                onClick = onRequestClearAllDownloads,
            )
        }
    }
}

@Composable
private fun PlaybackCategoryContent(
    settings: SonataSettingsState,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Queue & Transitions",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Autoplay", "Queue", "Similar")) {
            SettingsSwitchRow(
                title = "Autoplay Similar Tracks",
                subtitle = "Continue playing related tracks when queue ends",
                checked = settings.autoplayEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(autoplayEnabled = enabled) } },
            )
        }
        if (search.matches("Loudness", "Normalization", "Volume")) {
            SettingsSwitchRow(
                title = "Loudness Normalization",
                subtitle = "Keep perceived volume balanced across tracks",
                checked = settings.loudnessNormEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(loudnessNormEnabled = enabled) } },
            )
        }
        if (search.matches("Crossfade", "Blend", "Transitions")) {
            SettingsSwitchRow(
                title = "Crossfade",
                subtitle = if (settings.crossfadeEnabled) "${settings.crossfadeSeconds}s smooth overlap" else "Off",
                checked = settings.crossfadeEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(crossfadeEnabled = enabled) } },
            )
            if (settings.crossfadeEnabled) {
                addRow(dividerStart = ROW_INSET) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = ROW_INSET, vertical = 10.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text("Duration", style = MaterialTheme.typography.bodySmall, color = secondaryText)
                        Text(
                            "${settings.crossfadeSeconds}s",
                            style = MaterialTheme.typography.labelMedium,
                            color = primaryText,
                        )
                    }
                    Slider(
                        value = settings.crossfadeSeconds.toFloat(),
                        onValueChange = { value ->
                            onUpdateSettings { it.copy(crossfadeSeconds = value.roundToInt().coerceIn(1, 12)) }
                        },
                        valueRange = 1f..12f,
                        steps = 10,
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
        if (search.matches("Skip Silence", "Silence")) {
            SettingsSwitchRow(
                title = "Skip Silence",
                subtitle = "Shorten dead air between tracks",
                checked = settings.skipSilenceEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(skipSilenceEnabled = enabled) } },
            )
        }
        if (search.matches("Sleep Timer", "Fade Out", "Fade")) {
            SettingsSwitchRow(
                title = "Sleep Timer Fade Out",
                subtitle = "Gently lower volume over the final 10 seconds before pausing",
                checked = settings.sleepTimerFadeOut,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(sleepTimerFadeOut = enabled) } },
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    if (search.matches("Speed", "Tempo", "Equalizer", "EQ", "Audio")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = "Speed & Equalizer",
            secondaryText = secondaryText,
        ) {
            addRow(dividerStart = ROW_INSET) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 12.dp),
            ) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("Playback Speed", style = MaterialTheme.typography.bodyLarge, color = primaryText)
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
                title = "5-Band Equalizer",
                subtitle = if (settings.eqEnabled) "Preset: ${settings.eqPreset}" else "Disabled",
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
                                label = preset,
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
                                label = preset,
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
private fun ContentPreferencesCategoryContent(
    settings: SonataSettingsState,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Home Feed",
        footer = "Customize which shelves appear on the Home tab and how video cards are framed.",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Hide Recents", "Recents", "Recently Played", "Last Played")) {
            SettingsSwitchRow(
                title = "Hide Recents",
                subtitle = "Hide Last Played and Recently Played shelves on Home",
                checked = settings.hideRecents,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideRecents = enabled) } },
            )
        }
        if (search.matches("Hide Top Picks", "Top Picks", "Featured")) {
            SettingsSwitchRow(
                title = "Hide Top Picks",
                subtitle = "Hide the Top Picks for You shelf on Home",
                checked = settings.hideTopPicks,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideTopPicks = enabled) } },
            )
        }
        if (search.matches("Hide Find Your Mood", "Mood", "Moods")) {
            SettingsSwitchRow(
                title = "Hide Find Your Mood",
                subtitle = "Hide the mood carousel from Home",
                checked = settings.hideHomeMoods,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideHomeMoods = enabled) } },
            )
        }
        if (search.matches("Hide Home Videos", "Videos", "Music Videos")) {
            SettingsSwitchRow(
                title = "Hide Home Videos",
                subtitle = "Filter out video shelves from the Home feed",
                checked = settings.hideHomeVideos,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideHomeVideos = enabled) } },
            )
        }
        if (search.matches("Square Video Thumbnails", "Square", "Thumbnails", "Videos")) {
            SettingsSwitchRow(
                title = "Square Video Thumbnails",
                subtitle = "Crop 16:9 music video cards into clean 1:1 album sleeves",
                checked = settings.squareVideoThumbnails,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(squareVideoThumbnails = enabled) } },
            )
        }
    }
}

@Composable
private fun AppearanceCategoryContent(
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

    if (search.matches("Theme", "Tema", "Violet", "Moonflower", "Basic", "Dark", "Light", "AMOLED", "Appearance", "Görünüm")) {
        SettingsGroup(
            cardBg = cardBg,
            dividerColor = dividerColor,
            header = strings.sectionTheme,
            footer = "Basic applies the monochrome editorial layout on Home, Explore, Album, and Artist pages. Violet and Moonflower keep their full atmospheric layouts.",
            secondaryText = secondaryText,
        ) {
            SettingsRow(
                icon = SonataIcons.Sparkles,
                title = strings.designTheme,
                subtitle = designThemeDescription(activeDesign),
                value = designThemeLabel(activeDesign),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DESIGN_THEME) },
            )

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
                    strings.themeModeAmoled,
                )
                val selectedIdx = when (settings.themeMode) {
                    ThemeMode.SYSTEM -> 0
                    ThemeMode.VIOLET_LIGHT, ThemeMode.MOONFLOWER_LIGHT, ThemeMode.BASIC_LIGHT -> 1
                    ThemeMode.VIOLET_DARK, ThemeMode.MOONFLOWER_DARK, ThemeMode.BASIC_DARK -> 2
                    ThemeMode.AMOLED -> 3
                }
                PillSegmentedControl(
                    options = modes,
                    selectedIndex = selectedIdx,
                    onSelect = { idx ->
                        val nextMode = when (idx) {
                            0 -> ThemeMode.SYSTEM
                            1 -> when (activeDesign) {
                                DesignTheme.VIOLET -> ThemeMode.VIOLET_LIGHT
                                DesignTheme.MOONFLOWER -> ThemeMode.MOONFLOWER_LIGHT
                                DesignTheme.BASIC -> ThemeMode.BASIC_LIGHT
                            }
                            2 -> when (activeDesign) {
                                DesignTheme.VIOLET -> ThemeMode.VIOLET_DARK
                                DesignTheme.MOONFLOWER -> ThemeMode.MOONFLOWER_DARK
                                DesignTheme.BASIC -> ThemeMode.BASIC_DARK
                            }
                            else -> ThemeMode.AMOLED
                        }
                        onUpdateSettings { it.copy(designTheme = activeDesign, themeMode = nextMode) }
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
            header = "Glass & Motion",
            secondaryText = secondaryText,
        ) {
            if (search.matches("Liquid Glass", "Glass")) {
                val glassApiSupported = isGlassSupported()
                SettingsSwitchRow(
                    title = "Liquid Glass",
                    subtitle = if (glassApiSupported) {
                        "Crystal-clear refracted navigation bar and floating chrome"
                    } else {
                        "Requires Android 13+ (API 33)"
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
                    title = "Glass Performance Mode",
                    subtitle = "Lighter single-pass glass rendering for smoother scrolling",
                    checked = settings.glassPerformance,
                    enabled = settings.liquidGlassEnabled && isGlassSupported(),
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(glassPerformance = enabled) } },
                )
            }
            if (search.matches("Reduce Dynamic Blur", "Dynamic Blur", "Blur")) {
                SettingsSwitchRow(
                    title = "Reduce Dynamic Blur",
                    subtitle = "Use crisp static frosted surfaces instead of live lens sampling",
                    checked = settings.reduceDynamicBlur,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceDynamicBlur = enabled) } },
                )
            }
            if (search.matches("Reduce Animation", "Animation", "Motion")) {
                SettingsSwitchRow(
                    title = "Reduce Animation",
                    subtitle = "Minimize spring transitions and ambient wave motion",
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
            header = "Player & Covers",
            secondaryText = secondaryText,
        ) {
            if (search.matches("Full-Screen Cover Art", "Full-Screen", "Cover Art")) {
                SettingsSwitchRow(
                    title = "Full-Screen Cover Art",
                    subtitle = "Edge-to-edge dissolving hero cover in portrait Now Playing",
                    checked = settings.fullScreenCoverArt,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(fullScreenCoverArt = enabled) } },
                )
            }
            if (search.matches("Full-Screen Detail Cover", "Detail Cover", "Album")) {
                SettingsSwitchRow(
                    title = "Full-Screen Detail Cover",
                    subtitle = "Edge-to-edge album and playlist artwork in Basic detail pages",
                    checked = settings.fullScreenDetailCover,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(fullScreenDetailCover = enabled) } },
                )
            }
            if (search.matches("Show Total Duration", "Total Duration", "Time", "Remaining")) {
                SettingsSwitchRow(
                    title = "Show Total Duration",
                    subtitle = "Show total song length instead of remaining countdown",
                    checked = settings.showTotalDuration,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(showTotalDuration = enabled) } },
                )
            }
            if (search.matches("Player Backdrop", "Backdrop", "Mesh", "Blur")) {
                SettingsRow(
                    icon = SonataIcons.Sparkles,
                    title = "Player Backdrop",
                    value = backdropLabel(settings.playerBackdropMode),
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onClick = { onOpenChoice(SettingsChoiceKind.PLAYER_BACKDROP) },
                )
            }
            if (search.matches("Animated Cover Art", "Animated", "Canvas", "Motion")) {
                SettingsSwitchRow(
                    title = "Animated Cover Art",
                    subtitle = "Play looping motion covers when available",
                    checked = settings.animatedArtwork,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(animatedArtwork = enabled) } },
                )
            }
            if (search.matches("Play Animated Cover Over Cellular", "Cellular", "Animated")) {
                SettingsSwitchRow(
                    title = "Animated Covers Over Cellular",
                    subtitle = "Allow loading remote animated covers on metered connections",
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
            header = "Lyrics",
            secondaryText = secondaryText,
        ) {
            if (search.matches("Hide Lyrics Preview", "Preview", "Lyrics")) {
                SettingsSwitchRow(
                    title = "Hide Lyrics Preview",
                    subtitle = "Hide the inline synced lyric line above the scrubber",
                    checked = settings.hideLyricsPreview,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideLyricsPreview = enabled) } },
                )
            }
            if (search.matches("Blur Unfocused Lyrics", "Blur", "Lyrics")) {
                SettingsSwitchRow(
                    title = "Blur Unfocused Lyrics",
                    subtitle = "Softly defocus upcoming and past lyric lines",
                    checked = settings.lyricsBlurEnabled,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(lyricsBlurEnabled = enabled) } },
                )
            }
            if (search.matches("Word-by-Word", "Karaoke", "Sync", "Lyrics")) {
                SettingsSwitchRow(
                    title = "Word-by-Word Sync",
                    subtitle = "Prefer syllable-timed karaoke lyrics when available",
                    checked = settings.prioritizeWordSync,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(prioritizeWordSync = enabled) } },
                )
            }
            if (search.matches("Translate", "Translation", "Language", "Lyrics")) {
                SettingsSwitchRow(
                    title = "Translate Lyrics",
                    subtitle = "Show translated line beneath original lyrics",
                    checked = settings.lyricsTranslationEnabled,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onCheckedChange = { enabled -> onUpdateSettings { it.copy(lyricsTranslationEnabled = enabled) } },
                )
                if (settings.lyricsTranslationEnabled) {
                    val langName = TranslationLangs.firstOrNull { it.first == settings.lyricsTranslationLang }?.second
                        ?: settings.lyricsTranslationLang.uppercase(Locale.US)
                    SettingsRow(
                        icon = SonataIcons.Translate,
                        title = "Translation Language",
                        value = langName,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        onClick = { onOpenChoice(SettingsChoiceKind.LYRICS_LANGUAGE) },
                    )
                }
            }
            if (search.matches("Romanize", "Romanization", "Lyrics")) {
                SettingsSwitchRow(
                    title = "Romanize Lyrics",
                    subtitle = "Transliterate Japanese, Korean, and Cyrillic scripts",
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
                        Text("Global Lyric Offset", style = MaterialTheme.typography.bodyLarge, color = primaryText)
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
private fun PerformanceCategoryContent(
    settings: SonataSettingsState,
    supports120Hz: Boolean,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Display & Rendering",
        footer = "Lowering frame rate or enabling Glass Performance Mode reduces GPU usage and improves battery life.",
        secondaryText = secondaryText,
    ) {
        if (supports120Hz && search.matches("Refresh Rate", "120 Hz", "60 Hz", "FPS", "Performance")) {
            SettingsSwitchRow(
                title = "120 Hz High Refresh Rate",
                subtitle = "Smooth high-refresh scrolling and animations",
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
                title = "Glass Performance Mode",
                subtitle = "Lighter single-pass glass rendering for smoother frame pacing",
                checked = settings.glassPerformance,
                enabled = settings.liquidGlassEnabled && isGlassSupported(),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(glassPerformance = enabled) } },
            )
        }
        if (search.matches("Reduce Dynamic Blur", "Blur", "Performance")) {
            SettingsSwitchRow(
                title = "Reduce Dynamic Blur",
                subtitle = "Replace live backdrop refraction with crisp static surfaces",
                checked = settings.reduceDynamicBlur,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceDynamicBlur = enabled) } },
            )
        }
        if (search.matches("Reduce Animation", "Animation", "Motion")) {
            SettingsSwitchRow(
                title = "Reduce Animation",
                subtitle = "Minimize motion transitions across tabs and player",
                checked = settings.reduceAnimation,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(reduceAnimation = enabled) } },
            )
        }
    }
}

@Composable
private fun StorageCategoryContent(
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
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Storage & Cache",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Downloaded", "Downloads", "Offline", "Storage")) {
            SettingsRow(
                icon = SonataIcons.Download,
                title = "Downloaded Songs",
                subtitle = "${downloadStorageStats.trackCount} songs · ${downloadStorageStats.locationLabel}",
                value = formatStorageBytes(downloadStorageStats.downloadedBytes),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = onOpenDownloads,
            )
        }
        if (search.matches("Audio Cache", "Media Cache", "Lyrics Cache", "Clear")) {
            SettingsActionRow(
                title = "Clear Audio & Lyrics Cache",
                subtitle = "Free cached audio segments and synced lyrics",
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onClearMediaCache,
            )
        }
        if (search.matches("Image Cache", "Artwork Cache", "Clear")) {
            SettingsActionRow(
                title = "Clear Image Cache",
                subtitle = "Clear cached album covers and thumbnails from memory and disk",
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onClearImageCache,
            )
        }
    }
}

@Composable
private fun YourDataCategoryContent(
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Backup & Restore",
        footer = "Settings backup files are plain JSON and strictly exclude session cookies and account credentials.",
        secondaryText = secondaryText,
    ) {
        SettingsActionRow(
            title = "Export Settings Backup",
            subtitle = "Save preferences and per-track lyric offsets as JSON",
            titleColor = primaryText,
            secondaryText = secondaryText,
            onClick = onExportBackup,
        )
        SettingsActionRow(
            title = "Import Settings Backup",
            subtitle = "Restore preferences from a previously exported JSON file",
            titleColor = primaryText,
            secondaryText = secondaryText,
            onClick = onImportBackup,
        )
    }
}

@Composable
private fun MiscellaneousCategoryContent(
    settings: SonataSettingsState,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "Player & System",
        secondaryText = secondaryText,
    ) {
        if (search.matches("Hide Status Bar", "Status Bar", "Player")) {
            SettingsSwitchRow(
                title = "Hide Status Bar in Player",
                subtitle = "Use immersive full-height layout while Now Playing is open",
                checked = settings.hideStatusBarInPlayer,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideStatusBarInPlayer = enabled) } },
            )
        }
        if (search.matches("Hide Quality Badge", "Quality Badge", "Song Status")) {
            SettingsSwitchRow(
                title = "Hide Quality Badge",
                subtitle = "Hide the audio format pill below the playback scrubber",
                checked = settings.hideSongStatus,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(hideSongStatus = enabled) } },
            )
        }
        if (search.matches("Stop Music on Close", "Stop Music", "Close", "Swipe")) {
            SettingsSwitchRow(
                title = "Stop Music on Close",
                subtitle = "Stop playback and close media notification when swiping the app away",
                checked = settings.stopMusicOnClose,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(stopMusicOnClose = enabled) } },
            )
        }
        if (search.matches("Show Nerd Stats", "Nerd Stats", "Codec", "Bitrate", "Telemetry")) {
            SettingsSwitchRow(
                title = "Show Nerd Stats",
                subtitle = "Display live codec, bitrate, sample rate, and cache source in Now Playing",
                checked = settings.showNerdStats,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled -> onUpdateSettings { it.copy(showNerdStats = enabled) } },
            )
        }
    }
}

@Composable
private fun AccountCategoryContent(
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    installedVersionName: String,
    updateInfo: AndroidUpdateInfo?,
    checkingUpdates: Boolean,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
) {
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "YouTube Music Account",
        secondaryText = secondaryText,
    ) {
        SettingsRow(
            icon = SonataIcons.Person,
            title = if (signedIn) {
                accountProfile?.name?.takeIf { it.isNotBlank() } ?: "Connected"
            } else {
                "Not Signed In"
            },
            subtitle = if (signedIn) {
                accountProfile?.handle?.takeIf { it.isNotBlank() } ?: "YouTube Music session active"
            } else {
                "Sign in for personalized Home, playlists, and likes"
            },
            avatarUrl = if (signedIn) accountProfile?.avatarUrl else null,
            primaryText = primaryText,
            secondaryText = secondaryText,
            showChevron = false,
            onClick = if (signedIn) null else onSignIn,
        )
        if (signedIn) {
            SettingsActionRow(
                title = "Sign Out",
                subtitle = "Clear local YouTube Music session cookies",
                titleColor = BasicPalette.AccentRed,
                secondaryText = secondaryText,
                onClick = onSignOut,
            )
        } else {
            SettingsActionRow(
                title = "Sign In to YouTube Music",
                subtitle = "Open secure in-app browser sign-in",
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onSignIn,
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = "About & Updates",
        footer = "Sonata Android is 100% open-source with zero trackers or proprietary analytics SDKs.",
        secondaryText = secondaryText,
    ) {
        SettingsRow(
            icon = SonataIcons.Sparkles,
            title = "Sonata Version",
            value = installedVersionName,
            primaryText = primaryText,
            secondaryText = secondaryText,
            showChevron = false,
            onClick = null,
        )
        if (updateInfo?.updateAvailable == true) {
            SettingsActionRow(
                title = "Download Update (${updateInfo.latestVersion})",
                subtitle = "Open release APK download",
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onDownloadUpdate,
            )
        } else {
            addRow(dividerStart = ROW_INSET) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = !checkingUpdates, onClick = onCheckForUpdates)
                    .padding(horizontal = ROW_INSET, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = if (checkingUpdates) "Checking for Updates..." else "Check for Updates",
                        style = MaterialTheme.typography.bodyLarge,
                        color = primaryText,
                    )
                    Text(
                        text = "Compare installed build with GitHub Releases",
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
                if (checkingUpdates) {
                    CircularProgressIndicator(
                        color = primaryText,
                        strokeWidth = 2.dp,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
            }
        }
    }
}

// ---- Choice Subpages --------------------------------------------------------

@Composable
private fun SettingsChoicePage(
    kind: SettingsChoiceKind,
    settings: SonataSettingsState,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onSelectDone: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    val choiceTitle = when (kind) {
        SettingsChoiceKind.STREAMING_QUALITY -> strings.streamingQuality
        SettingsChoiceKind.DOWNLOAD_QUALITY -> strings.downloadQuality
        SettingsChoiceKind.APP_LANGUAGE -> strings.appLanguage
        SettingsChoiceKind.DESIGN_THEME -> strings.designTheme
        else -> kind.title
    }
    SettingsPageTitle(title = choiceTitle, color = primaryText)

    when (kind) {
        SettingsChoiceKind.STREAMING_QUALITY -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                footer = "Higher streaming tiers prioritize Opus 251 / AAC 320 kbps when available.",
                secondaryText = secondaryText,
            ) {
                AudioQualityTier.entries.forEach { tier ->
                    SettingsChoiceRow(
                        title = audioQualityLabel(tier),
                        subtitle = audioQualityDetail(tier),
                        selected = settings.audioQuality == tier,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings {
                                it.copy(
                                    audioQuality = tier,
                                    preferHighQuality = tier == AudioQualityTier.LOSSLESS || tier == AudioQualityTier.HIGH,
                                )
                            }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.DOWNLOAD_QUALITY -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                header = "Ordered from Highest to Smallest",
                footer = "Average per-song size assumes a standard ~3.5 minute track. Actual file size varies by song duration and codec.",
                secondaryText = secondaryText,
            ) {
                DownloadQualityTier.entries.forEach { tier ->
                    SettingsChoiceRow(
                        title = tier.title,
                        subtitle = tier.detail,
                        badge = tier.approxMbPerSong,
                        selected = settings.downloadQuality == tier,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(downloadQuality = tier) }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.DOWNLOAD_STORAGE -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                footer = "Internal storage keeps downloads inside Sonata's private sandbox. External storage writes tracks into Music/Sonata so file managers and other players can access them.",
                secondaryText = secondaryText,
            ) {
                DownloadStorageLocation.entries.forEach { loc ->
                    SettingsChoiceRow(
                        title = loc.title,
                        subtitle = loc.detail,
                        selected = settings.downloadStorageLocation == loc,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings {
                                it.copy(
                                    downloadStorageLocation = loc,
                                    exportDownloads = loc == DownloadStorageLocation.EXTERNAL,
                                )
                            }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.APP_LANGUAGE -> {
            val strings = LocalSonataStrings.current
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                footer = strings.appLanguageDesc,
                secondaryText = secondaryText,
            ) {
                AppLanguage.entries.forEach { lang ->
                    SettingsChoiceRow(
                        title = appLanguageLabel(lang, strings),
                        subtitle = appLanguageDescription(lang, strings),
                        selected = settings.appLanguage == lang,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(appLanguage = lang) }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.DESIGN_THEME -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                footer = "Basic activates the monochrome editorial UI overhaul on Home, Explore, Album, and Artist pages. Violet and Moonflower keep their full distinct visual styles.",
                secondaryText = secondaryText,
            ) {
                val activeDesign = settings.resolvedDesignTheme()
                DesignTheme.entries.forEach { design ->
                    SettingsChoiceRow(
                        title = designThemeLabel(design),
                        subtitle = designThemeDescription(design),
                        selected = activeDesign == design,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            val nextMode = when (settings.themeMode) {
                                ThemeMode.SYSTEM -> ThemeMode.SYSTEM
                                ThemeMode.AMOLED -> ThemeMode.AMOLED
                                ThemeMode.VIOLET_LIGHT, ThemeMode.MOONFLOWER_LIGHT, ThemeMode.BASIC_LIGHT -> when (design) {
                                    DesignTheme.VIOLET -> ThemeMode.VIOLET_LIGHT
                                    DesignTheme.MOONFLOWER -> ThemeMode.MOONFLOWER_LIGHT
                                    DesignTheme.BASIC -> ThemeMode.BASIC_LIGHT
                                }
                                ThemeMode.VIOLET_DARK, ThemeMode.MOONFLOWER_DARK, ThemeMode.BASIC_DARK -> when (design) {
                                    DesignTheme.VIOLET -> ThemeMode.VIOLET_DARK
                                    DesignTheme.MOONFLOWER -> ThemeMode.MOONFLOWER_DARK
                                    DesignTheme.BASIC -> ThemeMode.BASIC_DARK
                                }
                            }
                            onUpdateSettings { it.copy(designTheme = design, themeMode = nextMode) }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.PLAYER_BACKDROP -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                secondaryText = secondaryText,
            ) {
                PlayerBackdropMode.entries.forEach { mode ->
                    SettingsChoiceRow(
                        title = backdropLabel(mode),
                        subtitle = backdropDetail(mode),
                        selected = settings.playerBackdropMode == mode,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(playerBackdropMode = mode) }
                            onSelectDone()
                        },
                    )
                }
            }
        }

        SettingsChoiceKind.LYRICS_LANGUAGE -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                secondaryText = secondaryText,
            ) {
                TranslationLangs.forEach { (code, name) ->
                    SettingsChoiceRow(
                        title = name,
                        subtitle = code.uppercase(Locale.US),
                        selected = settings.lyricsTranslationLang == code,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(lyricsTranslationLang = code) }
                            onSelectDone()
                        },
                    )
                }
            }
        }
    }
}

// ---- Reusable Monochrome Settings Primitives --------------------------------

@Composable
private fun SettingsPageTitle(
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
private fun SettingsSearchField(
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
                contentDescription = "Clear search",
                tint = secondaryText,
                modifier = Modifier
                    .size(18.dp)
                    .clickable { onQueryChange("") },
            )
        }
    }
}

@Composable
private fun SettingsGroup(
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

private interface SettingsGroupScope {
    fun addRow(dividerStart: Dp = ROW_INSET, row: @Composable () -> Unit)
}

@Composable
private fun SettingsGroupScope.SettingsRow(
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
private fun SettingsGroupScope.SettingsSwitchRow(
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
private fun SettingsGroupScope.SettingsActionRow(
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
private fun SettingsGroupScope.SettingsChoiceRow(
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
                    contentDescription = "Selected",
                    tint = accentTint,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }
}

@Composable
private fun MonochromeIosSwitch(
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
private fun EqPresetChip(
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

// ---- Search & Format Helpers ------------------------------------------------

private class SettingsSearchMatcher(rawQuery: String) {
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

private fun formatStorageBytes(bytes: Long): String {
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

private fun audioQualityShortLabel(tier: AudioQualityTier): String = when (tier) {
    AudioQualityTier.LOSSLESS -> "Lossless"
    AudioQualityTier.HIGH -> "High"
    AudioQualityTier.MEDIUM -> "Medium"
    AudioQualityTier.LOW -> "Low"
}

private fun audioQualityLabel(tier: AudioQualityTier): String = when (tier) {
    AudioQualityTier.LOSSLESS -> "Lossless / Maximum"
    AudioQualityTier.HIGH -> "High (256–320 kbps)"
    AudioQualityTier.MEDIUM -> "Medium (128 kbps)"
    AudioQualityTier.LOW -> "Data Saver (64 kbps)"
}

private fun audioQualityDetail(tier: AudioQualityTier): String = when (tier) {
    AudioQualityTier.LOSSLESS -> "Highest bitrate stream available"
    AudioQualityTier.HIGH -> "Best balance of fidelity and fast startup"
    AudioQualityTier.MEDIUM -> "Standard streaming bitrate"
    AudioQualityTier.LOW -> "Minimal mobile data usage"
}

private fun downloadQualityShortLabel(tier: DownloadQualityTier): String = when (tier) {
    DownloadQualityTier.HIGHEST -> "Highest"
    DownloadQualityTier.HIGH -> "High"
    DownloadQualityTier.STANDARD -> "Standard"
    DownloadQualityTier.LOW -> "Data Saver"
}

private fun designThemeLabel(design: DesignTheme): String = when (design) {
    DesignTheme.VIOLET -> "Violet"
    DesignTheme.MOONFLOWER -> "Moonflower"
    DesignTheme.BASIC -> "Basic"
}

private fun designThemeDescription(design: DesignTheme): String = when (design) {
    DesignTheme.VIOLET -> "Sculpted deep amethyst glass"
    DesignTheme.MOONFLOWER -> "Botanical silk & floral aura"
    DesignTheme.BASIC -> "Monochrome editorial layout overhaul"
}

private fun colorModeLabel(mode: ThemeMode): String = when (mode) {
    ThemeMode.SYSTEM -> "System"
    ThemeMode.AMOLED -> "AMOLED"
    ThemeMode.VIOLET_LIGHT, ThemeMode.MOONFLOWER_LIGHT, ThemeMode.BASIC_LIGHT -> "Light"
    ThemeMode.VIOLET_DARK, ThemeMode.MOONFLOWER_DARK, ThemeMode.BASIC_DARK -> "Dark"
}

private fun backdropLabel(mode: PlayerBackdropMode): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> "Artwork Mesh"
    PlayerBackdropMode.AMBIENT_WAVES -> "Ambient Waves"
    PlayerBackdropMode.BLURRED_COVER -> "Blurred Cover"
}

private fun backdropDetail(mode: PlayerBackdropMode): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> "Rich color mesh baked from album artwork"
    PlayerBackdropMode.AMBIENT_WAVES -> "Smooth animated ambient palette gradient"
    PlayerBackdropMode.BLURRED_COVER -> "Full-screen softly blurred cover art"
}

private fun eqPresetCurve(preset: String): List<Float> = when (preset) {
    "Bass Boost" -> listOf(5.5f, 3.5f, 0.5f, 0.0f, 1.0f)
    "Vocal" -> listOf(-1.5f, 1.0f, 4.0f, 3.0f, 0.5f)
    "Acoustic" -> listOf(2.0f, 1.5f, 1.0f, 2.5f, 2.0f)
    "Electronic" -> listOf(4.5f, 2.0f, -1.0f, 2.5f, 4.0f)
    "Late Night" -> listOf(-2.0f, 1.0f, 2.0f, 1.0f, -2.5f)
    else -> listOf(0f, 0f, 0f, 0f, 0f)
}
