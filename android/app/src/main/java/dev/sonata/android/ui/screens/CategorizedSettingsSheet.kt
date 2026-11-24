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
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
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
import dev.sonata.android.data.VideoDownloadQuality
import dev.sonata.android.discord.DiscordActivityType
import dev.sonata.android.discord.DiscordApplication
import dev.sonata.android.discord.DiscordConsentWarningDialog
import dev.sonata.android.discord.DiscordLoginScreen
import dev.sonata.android.discord.DiscordRpcStatus
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
    DISCORD("Discord Rich Presence", SonataIcons.MusicNote),
    ACCOUNT("Account & Updates", SonataIcons.Person),
}

private enum class SettingsChoiceKind(val title: String) {
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

private fun SettingsChoiceKind.localizedTitle(strings: SonataStrings): String = when (this) {
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

private fun SettingsCategory.localizedTitle(strings: SonataStrings): String = when (this) {
    SettingsCategory.DOWNLOADS -> strings.catDownloads
    SettingsCategory.PLAYBACK -> strings.catPlayback
    SettingsCategory.CONTENT -> strings.catContent
    SettingsCategory.APPEARANCE -> strings.catAppearance
    SettingsCategory.PERFORMANCE -> strings.catPerformance
    SettingsCategory.STORAGE -> strings.catStorage
    SettingsCategory.YOUR_DATA -> strings.catYourData
    SettingsCategory.MISC -> strings.catMisc
    SettingsCategory.DISCORD -> strings.catDiscord
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
    discordConnected: Boolean = false,
    discordStatus: DiscordRpcStatus = DiscordRpcStatus.Disconnected,
    onSaveDiscordToken: (String) -> Unit = {},
    onClearDiscordToken: () -> Unit = {},
    onReconnectDiscord: () -> Unit = {},
    serverCount: Int = 0,
    enabledServerCount: Int = 0,
    onOpenServers: () -> Unit = {},
) {
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current
    val statusTop = rememberSafeTopInset()
    var route by remember { mutableStateOf<SettingsRoute>(SettingsRoute.Root) }
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var confirmClearDownloads by remember { mutableStateOf(false) }
    var showDiscordConsentDialog by remember { mutableStateOf(false) }
    var showDiscordLoginSheet by remember { mutableStateOf(false) }

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
            title = { Text(strings.removeAllDownloadsConfirmTitle) },
            text = {
                Text(
                    strings.removeAllDownloadsConfirmBody(
                        downloadStorageStats.trackCount,
                        formatStorageBytes(downloadStorageStats.downloadedBytes),
                    ),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmClearDownloads = false
                        onClearAllDownloads()
                    },
                ) {
                    Text(strings.remove, color = BasicPalette.AccentRed, fontWeight = FontWeight.SemiBold)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmClearDownloads = false }) {
                    Text(strings.cancel, color = primaryText)
                }
            },
        )
    }

    if (showDiscordConsentDialog) {
        DiscordConsentWarningDialog(
            onConfirm = {
                showDiscordConsentDialog = false
                showDiscordLoginSheet = true
            },
            onDismiss = { showDiscordConsentDialog = false },
            containerColor = cardBg,
            titleColor = primaryText,
            bodyColor = secondaryText,
        )
    }

    if (showDiscordLoginSheet) {
        Dialog(
            onDismissRequest = { showDiscordLoginSheet = false },
            properties = DialogProperties(
                usePlatformDefaultWidth = false,
                decorFitsSystemWindows = false,
            ),
        ) {
            DiscordLoginScreen(
                onTokenCaptured = { token ->
                    showDiscordLoginSheet = false
                    onSaveDiscordToken(token)
                },
                onDismiss = { showDiscordLoginSheet = false },
                requireConsent = false,
            )
        }
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
                        contentDescription = strings.back,
                        tint = primaryText,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Spacer(Modifier.width(12.dp))
                val topCaption = when (val current = route) {
                    SettingsRoute.Root -> ""
                    is SettingsRoute.Category -> strings.settingsTitle
                    is SettingsRoute.Choice -> when (val p = current.parent) {
                        SettingsRoute.Root -> strings.settingsTitle
                        is SettingsRoute.Category -> p.category.localizedTitle(strings)
                        is SettingsRoute.Choice -> p.kind.localizedTitle(strings)
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
                                                value = audioQualityShortLabel(settings.audioQuality, strings),
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
                                        if (search.matches("Download Quality", "İndirme", "Загрузка", "Offline", "320 kbps", "MB")) {
                                            SettingsRow(
                                                icon = SonataIcons.Download,
                                                title = strings.downloadQuality,
                                                value = "${downloadQualityShortLabel(settings.downloadQuality, strings)} · ${settings.downloadQuality.approxMbPerSong}",
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

                                // Self-Hosted Music Servers quick access row
                                if (search.matches(
                                        "Server",
                                        "Servers",
                                        "Music Server",
                                        "Self-Hosted",
                                        "Jellyfin",
                                        "Subsonic",
                                        "OpenSubsonic",
                                        "Navidrome",
                                        "WebDAV",
                                        "Sunucu",
                                        "Сервер",
                                    )
                                ) {
                                    SettingsGroup(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        header = strings.servers.headerServers,
                                        secondaryText = secondaryText,
                                    ) {
                                        SettingsRow(
                                            icon = SonataIcons.Folder,
                                            title = strings.servers.manageServers,
                                            subtitle = if (serverCount > 0) {
                                                strings.servers.enabledCountSummary(enabledServerCount, serverCount)
                                            } else {
                                                strings.servers.manageServersSummary
                                            },
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onClick = onOpenServers,
                                        )
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
                                                SettingsCategory.DISCORD ->
                                                    if (discordConnected) {
                                                        discordStatusLabel(discordStatus, strings)
                                                    } else {
                                                        strings.discordSummary
                                                    }
                                                SettingsCategory.ACCOUNT ->
                                                    "v$installedVersionName${if (updateInfo?.updateAvailable == true) " · ${strings.updateAvailable}" else ""}"
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
                                            "Video",
                                            "Video Download",
                                            "1080p",
                                            "720p",
                                            "480p",
                                            "360p",
                                            "Video İndirme",
                                            "Видео",
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

                                    if (search.matches(
                                            "Discord",
                                            "Rich Presence",
                                            "RPC",
                                            "Presence",
                                            "Application ID",
                                            "Timestamps",
                                            "Activity",
                                            "Status",
                                        )
                                    ) {
                                        matchCount++
                                        DiscordCategoryContent(
                                            settings = settings,
                                            discordConnected = discordConnected,
                                            discordStatus = discordStatus,
                                            search = search,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onUpdateSettings = onUpdateSettings,
                                            onOpenChoice = { kind -> route = SettingsRoute.Choice(kind, SettingsRoute.Root) },
                                            onRequestSignIn = { showDiscordConsentDialog = true },
                                            onSignOut = onClearDiscordToken,
                                            onReconnect = onReconnectDiscord,
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
                                                text = strings.searchNoMatch(searchQuery),
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

                                    SettingsCategory.DISCORD -> DiscordCategoryContent(
                                        settings = settings,
                                        discordConnected = discordConnected,
                                        discordStatus = discordStatus,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onUpdateSettings = onUpdateSettings,
                                        onOpenChoice = { kind ->
                                            route = SettingsRoute.Choice(kind, SettingsRoute.Category(activeCategory))
                                        },
                                        onRequestSignIn = { showDiscordConsentDialog = true },
                                        onSignOut = onClearDiscordToken,
                                        onReconnect = onReconnectDiscord,
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
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrQueueTransitions,
        secondaryText = secondaryText,
    ) {
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
            SettingsSwitchRow(
                title = strings.crossfade,
                subtitle = if (settings.crossfadeEnabled) strings.crossfadeSecondsOverlap(settings.crossfadeSeconds) else strings.off,
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
                        Text(strings.durationLabel, style = MaterialTheme.typography.bodySmall, color = secondaryText)
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
private fun ContentPreferencesCategoryContent(
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
            footer = strings.footerAppearanceTheme,
            secondaryText = secondaryText,
        ) {
            SettingsRow(
                icon = SonataIcons.Sparkles,
                title = strings.designTheme,
                subtitle = designThemeDescription(activeDesign, strings),
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
                    val langName = TranslationLangs.firstOrNull { it.first == settings.lyricsTranslationLang }?.second
                        ?: settings.lyricsTranslationLang.uppercase(Locale.US)
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
private fun YourDataCategoryContent(
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
) {
    val strings = LocalSonataStrings.current
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
private fun MiscellaneousCategoryContent(
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

private fun discordStatusLabel(status: DiscordRpcStatus, strings: SonataStrings): String = when (status) {
    DiscordRpcStatus.Disconnected -> strings.discordStatusDisconnected
    DiscordRpcStatus.Connecting -> strings.discordStatusConnecting
    is DiscordRpcStatus.Connected ->
        status.username?.takeIf { it.isNotBlank() }?.let { strings.discordStatusConnectedAs(it) }
            ?: strings.discordStatusConnected
    DiscordRpcStatus.Reconnecting -> strings.discordStatusReconnecting
    DiscordRpcStatus.AuthFailed -> strings.discordStatusAuthFailed
    is DiscordRpcStatus.Error -> strings.discordStatusError(status.message)
}

private fun discordActivityTypeLabel(type: DiscordActivityType, strings: SonataStrings): String = when (type) {
    DiscordActivityType.LISTENING -> strings.discordActivityListening
    DiscordActivityType.PLAYING -> strings.discordActivityPlaying
    DiscordActivityType.WATCHING -> strings.discordActivityWatching
    DiscordActivityType.COMPETING -> strings.discordActivityCompeting
}

@Composable
private fun DiscordCategoryContent(
    settings: SonataSettingsState,
    discordConnected: Boolean,
    discordStatus: DiscordRpcStatus,
    search: SettingsSearchMatcher,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onOpenChoice: (SettingsChoiceKind) -> Unit,
    onRequestSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onReconnect: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    var appIdDraft by remember(settings.discordAppId) { mutableStateOf(settings.discordAppId) }
    var appIdValidationError by remember(settings.discordAppId) { mutableStateOf(false) }

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrDiscordAccount,
        secondaryText = secondaryText,
    ) {
        val statusText = discordStatusLabel(discordStatus, strings)
        val statusColor = when (discordStatus) {
            is DiscordRpcStatus.Connected -> IosSwitchGreen
            DiscordRpcStatus.AuthFailed, is DiscordRpcStatus.Error -> BasicPalette.AccentRed
            else -> secondaryText
        }
        addRow(dividerStart = ROW_INSET) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 52.dp)
                    .padding(horizontal = ROW_INSET, vertical = 11.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(statusColor),
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = strings.discordStatusLabel,
                        style = MaterialTheme.typography.bodyLarge,
                        color = primaryText,
                    )
                    Text(
                        text = statusText,
                        style = MaterialTheme.typography.bodySmall,
                        color = statusColor,
                    )
                }
            }
        }
        if (discordConnected) {
            if (settings.discordRpcEnabled) {
                SettingsActionRow(
                    title = strings.discordReconnect,
                    titleColor = primaryText,
                    secondaryText = secondaryText,
                    onClick = onReconnect,
                )
            }
            SettingsActionRow(
                title = strings.discordSignOutAction,
                subtitle = strings.discordSignOutDesc,
                titleColor = BasicPalette.AccentRed,
                secondaryText = secondaryText,
                onClick = onSignOut,
            )
        } else {
            SettingsActionRow(
                title = strings.discordSignInAction,
                subtitle = strings.discordSignInActionDesc,
                titleColor = primaryText,
                secondaryText = secondaryText,
                onClick = onRequestSignIn,
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrDiscordPresence,
        secondaryText = secondaryText,
    ) {
        if (search.matches("Discord", "Enable", "Rich Presence", "RPC")) {
            SettingsSwitchRow(
                title = strings.discordEnableRpc,
                subtitle = strings.discordEnableRpcDesc,
                checked = settings.discordRpcEnabled,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(discordRpcEnabled = enabled) }
                },
            )
        }
        if (search.matches("Discord", "Activity", "Listening", "Playing", "Watching")) {
            SettingsRow(
                icon = SonataIcons.MusicNote,
                title = strings.discordActivityType,
                value = discordActivityTypeLabel(settings.discordActivityType, strings),
                primaryText = primaryText,
                secondaryText = secondaryText,
                onClick = { onOpenChoice(SettingsChoiceKind.DISCORD_ACTIVITY_TYPE) },
            )
        }
        if (search.matches("Discord", "Timestamps", "Progress", "Speed")) {
            SettingsSwitchRow(
                title = strings.discordShowTimestamps,
                subtitle = strings.discordShowTimestampsDesc,
                checked = settings.discordShowTimestamps,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(discordShowTimestamps = enabled) }
                },
            )
        }
        if (search.matches("Discord", "Artwork", "Cover", "Image")) {
            SettingsSwitchRow(
                title = strings.discordShowArtwork,
                subtitle = strings.discordShowArtworkDesc,
                checked = settings.discordShowArtwork,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(discordShowArtwork = enabled) }
                },
            )
        }
        if (search.matches("Discord", "Button", "Link", "YouTube Music")) {
            SettingsSwitchRow(
                title = strings.discordShowButtons,
                subtitle = strings.discordShowButtonsDesc,
                checked = settings.discordShowButtons,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = { enabled ->
                    onUpdateSettings { it.copy(discordShowButtons = enabled) }
                },
            )
        }
    }

    Spacer(Modifier.height(14.dp))

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrDiscordAppId,
        footer = strings.footerDiscordAppId,
        secondaryText = secondaryText,
    ) {
        addRow(dividerStart = ROW_INSET) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(
                    text = strings.discordAppIdLabel,
                    style = MaterialTheme.typography.bodyLarge,
                    color = primaryText,
                )
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(dividerColor.copy(alpha = 0.45f))
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                ) {
                    if (appIdDraft.isEmpty()) {
                        Text(
                            text = strings.discordAppIdPlaceholder,
                            style = MaterialTheme.typography.bodyMedium,
                            color = secondaryText,
                        )
                    }
                    BasicTextField(
                        value = appIdDraft,
                        onValueChange = { next ->
                            appIdDraft = next.filter { it.isDigit() }
                            appIdValidationError = false
                        },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = primaryText),
                        cursorBrush = SolidColor(primaryText),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (appIdValidationError) {
                    Text(
                        text = strings.discordAppIdInvalid,
                        style = MaterialTheme.typography.labelSmall,
                        color = BasicPalette.AccentRed,
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(primaryText)
                            .clickable {
                                val trimmed = appIdDraft.trim()
                                if (DiscordApplication.isValidSnowflakeAppId(trimmed)) {
                                    appIdValidationError = false
                                    onUpdateSettings { it.copy(discordAppId = trimmed) }
                                } else {
                                    appIdValidationError = true
                                }
                            }
                            .padding(horizontal = 14.dp, vertical = 8.dp),
                    ) {
                        Text(
                            text = strings.discordAppIdApply,
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                            color = if (primaryText == Color.White) Color.Black else Color.White,
                        )
                    }
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(dividerColor.copy(alpha = 0.6f))
                            .clickable {
                                appIdDraft = DiscordApplication.DEFAULT_APP_ID
                                appIdValidationError = false
                                onUpdateSettings { it.copy(discordAppId = DiscordApplication.DEFAULT_APP_ID) }
                            }
                            .padding(horizontal = 14.dp, vertical = 8.dp),
                    ) {
                        Text(
                            text = strings.discordAppIdReset,
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Medium),
                            color = primaryText,
                        )
                    }
                }
            }
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
    val strings = LocalSonataStrings.current
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = strings.hdrYtmAccount,
        secondaryText = secondaryText,
    ) {
        SettingsRow(
            icon = SonataIcons.Person,
            title = if (signedIn) {
                accountProfile?.name?.takeIf { it.isNotBlank() } ?: strings.connectedStatus
            } else {
                strings.notSignedInStatus
            },
            subtitle = if (signedIn) {
                accountProfile?.handle?.takeIf { it.isNotBlank() } ?: strings.ytmSessionActive
            } else {
                strings.signInYtmSettingsSubtitle
            },
            avatarUrl = if (signedIn) accountProfile?.avatarUrl else null,
            primaryText = primaryText,
            secondaryText = secondaryText,
            showChevron = false,
            onClick = if (signedIn) null else onSignIn,
        )
        if (signedIn) {
            SettingsActionRow(
                title = strings.signOut,
                subtitle = strings.signOutDesc,
                titleColor = BasicPalette.AccentRed,
                secondaryText = secondaryText,
                onClick = onSignOut,
            )
        } else {
            SettingsActionRow(
                title = strings.signInYtmTitle,
                subtitle = strings.signInYtmDesc,
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
        header = strings.hdrAboutUpdates,
        footer = strings.footerAboutUpdates,
        secondaryText = secondaryText,
    ) {
        SettingsRow(
            icon = SonataIcons.Sparkles,
            title = strings.sonataVersion,
            value = installedVersionName,
            primaryText = primaryText,
            secondaryText = secondaryText,
            showChevron = false,
            onClick = null,
        )
        if (updateInfo?.updateAvailable == true) {
            SettingsActionRow(
                title = strings.downloadUpdateVersion(updateInfo.latestVersion),
                subtitle = strings.openReleaseApk,
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
                        text = if (checkingUpdates) strings.checkingForUpdates else strings.checkForUpdates,
                        style = MaterialTheme.typography.bodyLarge,
                        color = primaryText,
                    )
                    Text(
                        text = strings.compareGithubReleases,
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
    val choiceTitle = kind.localizedTitle(strings)
    SettingsPageTitle(title = choiceTitle, color = primaryText)

    when (kind) {
        SettingsChoiceKind.STREAMING_QUALITY -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                footer = strings.footerStreamingQuality,
                secondaryText = secondaryText,
            ) {
                AudioQualityTier.entries.forEach { tier ->
                    SettingsChoiceRow(
                        title = audioQualityLabel(tier, strings),
                        subtitle = audioQualityDetail(tier, strings),
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
                header = strings.hdrOrderedHighestToSmallest,
                footer = strings.footerDownloadQuality,
                secondaryText = secondaryText,
            ) {
                DownloadQualityTier.entries.forEach { tier ->
                    SettingsChoiceRow(
                        title = downloadQualityShortLabel(tier, strings),
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

        SettingsChoiceKind.VIDEO_DOWNLOAD_QUALITY -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                header = strings.hdrOrderedHighestToSmallest,
                footer = strings.footerVideoDownloads,
                secondaryText = secondaryText,
            ) {
                VideoDownloadQuality.entries.forEach { quality ->
                    SettingsChoiceRow(
                        title = videoDownloadQualityTitle(quality, strings),
                        subtitle = videoDownloadQualityDesc(quality, strings),
                        badge = videoDownloadQualitySize(quality, strings),
                        selected = settings.videoDownloadQuality == quality,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(videoDownloadQuality = quality) }
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
                footer = strings.footerStorageLocation,
                secondaryText = secondaryText,
            ) {
                DownloadStorageLocation.entries.forEach { loc ->
                    SettingsChoiceRow(
                        title = downloadStorageShortLabel(loc, strings),
                        subtitle = downloadStorageDetail(loc, strings),
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
                footer = strings.footerAppearanceTheme,
                secondaryText = secondaryText,
            ) {
                val activeDesign = settings.resolvedDesignTheme()
                DesignTheme.entries.forEach { design ->
                    SettingsChoiceRow(
                        title = designThemeLabel(design),
                        subtitle = designThemeDescription(design, strings),
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
                        title = backdropLabel(mode, strings),
                        subtitle = backdropDetail(mode, strings),
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

        SettingsChoiceKind.DISCORD_ACTIVITY_TYPE -> {
            SettingsGroup(
                cardBg = cardBg,
                dividerColor = dividerColor,
                secondaryText = secondaryText,
            ) {
                DiscordActivityType.entries.forEach { activityType ->
                    SettingsChoiceRow(
                        title = discordActivityTypeLabel(activityType, strings),
                        selected = settings.discordActivityType == activityType,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accentTint = accentTint,
                        onClick = {
                            onUpdateSettings { it.copy(discordActivityType = activityType) }
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
                    contentDescription = LocalSonataStrings.current.done,
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

private fun audioQualityShortLabel(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLossless
    AudioQualityTier.HIGH -> strings.qualityHigh
    AudioQualityTier.MEDIUM -> strings.qualityMedium
    AudioQualityTier.LOW -> strings.qualityLow
}

private fun audioQualityLabel(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLosslessDetail
    AudioQualityTier.HIGH -> strings.qualityHighDetail
    AudioQualityTier.MEDIUM -> strings.qualityMediumDetail
    AudioQualityTier.LOW -> strings.qualityLowDetail
}

private fun audioQualityDetail(tier: AudioQualityTier, strings: SonataStrings): String = when (tier) {
    AudioQualityTier.LOSSLESS -> strings.qualityLosslessDesc
    AudioQualityTier.HIGH -> strings.qualityHighDesc
    AudioQualityTier.MEDIUM -> strings.qualityMediumDesc
    AudioQualityTier.LOW -> strings.qualityLowDesc
}

private fun downloadQualityShortLabel(tier: DownloadQualityTier, strings: SonataStrings): String = when (tier) {
    DownloadQualityTier.HIGHEST -> strings.dlHighest
    DownloadQualityTier.HIGH -> strings.dlHigh
    DownloadQualityTier.STANDARD -> strings.dlStandard
    DownloadQualityTier.LOW -> strings.dlLow
}

private fun videoDownloadQualityTitle(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Title
    VideoDownloadQuality.P720 -> strings.videoDl720Title
    VideoDownloadQuality.P480 -> strings.videoDl480Title
    VideoDownloadQuality.P360 -> strings.videoDl360Title
}

private fun videoDownloadQualityDesc(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Desc
    VideoDownloadQuality.P720 -> strings.videoDl720Desc
    VideoDownloadQuality.P480 -> strings.videoDl480Desc
    VideoDownloadQuality.P360 -> strings.videoDl360Desc
}

private fun videoDownloadQualitySize(quality: VideoDownloadQuality, strings: SonataStrings): String = when (quality) {
    VideoDownloadQuality.P1080 -> strings.videoDl1080Size
    VideoDownloadQuality.P720 -> strings.videoDl720Size
    VideoDownloadQuality.P480 -> strings.videoDl480Size
    VideoDownloadQuality.P360 -> strings.videoDl360Size
}

private fun downloadStorageShortLabel(loc: DownloadStorageLocation, strings: SonataStrings): String = when (loc) {
    DownloadStorageLocation.INTERNAL -> strings.storageLocationInternal
    DownloadStorageLocation.EXTERNAL -> strings.storageLocationExternal
}

private fun downloadStorageDetail(loc: DownloadStorageLocation, strings: SonataStrings): String = when (loc) {
    DownloadStorageLocation.INTERNAL -> strings.storageInternalDesc
    DownloadStorageLocation.EXTERNAL -> strings.storageExternalDesc
}

private fun designThemeLabel(design: DesignTheme): String = when (design) {
    DesignTheme.VIOLET -> "Violet"
    DesignTheme.MOONFLOWER -> "Moonflower"
    DesignTheme.BASIC -> "Basic"
}

private fun designThemeDescription(design: DesignTheme, strings: SonataStrings): String = when (design) {
    DesignTheme.VIOLET -> strings.themeVioletDesc
    DesignTheme.MOONFLOWER -> strings.themeMoonflowerDesc
    DesignTheme.BASIC -> strings.themeBasicDesc
}

private fun backdropLabel(mode: PlayerBackdropMode, strings: SonataStrings): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> strings.backdropMesh
    PlayerBackdropMode.AMBIENT_WAVES -> strings.backdropWaves
    PlayerBackdropMode.BLURRED_COVER -> strings.backdropBlurred
}

private fun backdropDetail(mode: PlayerBackdropMode, strings: SonataStrings): String = when (mode) {
    PlayerBackdropMode.ARTWORK_MESH -> strings.backdropMeshDesc
    PlayerBackdropMode.AMBIENT_WAVES -> strings.backdropWavesDesc
    PlayerBackdropMode.BLURRED_COVER -> strings.backdropBlurredDesc
}

private fun eqPresetLabel(preset: String, strings: SonataStrings): String = when (preset) {
    "Flat" -> strings.eqPresetFlat
    "Bass Boost" -> strings.eqPresetBassBoost
    "Vocal" -> strings.eqPresetVocal
    "Acoustic" -> strings.eqPresetAcoustic
    "Electronic" -> strings.eqPresetElectronic
    "Late Night" -> strings.eqPresetLateNight
    "Custom" -> strings.eqPresetCustom
    else -> preset
}

private fun eqPresetCurve(preset: String): List<Float> = when (preset) {
    "Bass Boost" -> listOf(5.5f, 3.5f, 0.5f, 0.0f, 1.0f)
    "Vocal" -> listOf(-1.5f, 1.0f, 4.0f, 3.0f, 0.5f)
    "Acoustic" -> listOf(2.0f, 1.5f, 1.0f, 2.5f, 2.0f)
    "Electronic" -> listOf(4.5f, 2.0f, -1.0f, 2.5f, 4.0f)
    "Late Night" -> listOf(-2.0f, 1.0f, 2.0f, 1.0f, -2.5f)
    else -> listOf(0f, 0f, 0f, 0f, 0f)
}
