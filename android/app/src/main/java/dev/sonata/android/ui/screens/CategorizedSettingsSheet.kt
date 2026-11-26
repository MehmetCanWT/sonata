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
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.sonata.android.connect.ConnectUiState
import dev.sonata.android.data.LocalAudioTrack
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.discord.DiscordConsentWarningDialog
import dev.sonata.android.discord.DiscordLoginScreen
import dev.sonata.android.discord.DiscordRpcStatus
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.ecosystemStrings
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.screens.settings.AccountCategoryContent
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.ui.screens.settings.AppearanceCategoryContent
import dev.sonata.android.ui.screens.settings.ConnectCategoryContent
import dev.sonata.android.ui.screens.settings.ContentPreferencesCategoryContent
import dev.sonata.android.ui.screens.settings.DiscordCategoryContent
import dev.sonata.android.ui.screens.settings.DownloadsCategoryContent
import dev.sonata.android.ui.screens.settings.MiscellaneousCategoryContent
import dev.sonata.android.ui.screens.settings.PerformanceCategoryContent
import dev.sonata.android.ui.screens.settings.PlaybackCategoryContent
import dev.sonata.android.ui.screens.settings.ScrobblingCategoryContent
import dev.sonata.android.ui.screens.settings.SettingsChoiceKind
import dev.sonata.android.ui.screens.settings.SettingsChoicePage
import dev.sonata.android.ui.screens.settings.SettingsGroup
import dev.sonata.android.ui.screens.settings.SettingsPageTitle
import dev.sonata.android.ui.screens.settings.SettingsRoute
import dev.sonata.android.ui.screens.settings.SettingsRow
import dev.sonata.android.ui.screens.settings.SettingsSearchField
import dev.sonata.android.ui.screens.settings.SettingsSearchMatcher
import dev.sonata.android.ui.screens.settings.StorageCategoryContent
import dev.sonata.android.ui.screens.settings.YourDataCategoryContent
import dev.sonata.android.ui.screens.settings.appLanguageLabel
import dev.sonata.android.ui.screens.settings.audioQualityShortLabel
import dev.sonata.android.ui.screens.settings.designThemeLabel
import dev.sonata.android.ui.screens.settings.discordStatusLabel
import dev.sonata.android.ui.screens.settings.downloadQualityShortLabel
import dev.sonata.android.ui.screens.settings.formatStorageBytes
import dev.sonata.android.ui.screens.settings.localizedTitle
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidLastFmConfig
import uniffi.sonata_android.AndroidListenBrainzConfig
import uniffi.sonata_android.AndroidScrobbleConfig
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidStatsSummary
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidUpdateInfo

enum class SettingsCategory(
    val title: String,
    val icon: ImageVector,
) {
    SOURCES("Sources", SonataIcons.Folder),
    DOWNLOADS("Downloads", SonataIcons.Download),
    PLAYBACK("Playback", SonataIcons.Play),
    CONNECT("Sonata Connect", SonataIcons.Wifi),
    SCROBBLING("Scrobbling", SonataIcons.MusicNote),
    CONTENT("Content Preferences", SonataIcons.TabExplore),
    APPEARANCE("Appearance", SonataIcons.Sparkles),
    PERFORMANCE("Performance", SonataIcons.GraphicEq),
    STORAGE("Storage", SonataIcons.Folder),
    YOUR_DATA("Your Data", SonataIcons.Download),
    MISC("Miscellaneous", SonataIcons.Settings),
    DISCORD("Discord Rich Presence", SonataIcons.MusicNote),
    ACCOUNT("Account & Updates", SonataIcons.Person),
}

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
    localTrackCount: Int = 0,
    localPermissionGranted: Boolean = true,
    servers: List<AndroidServerProfile> = emptyList(),
    serverDiagnostics: Map<String, ServerDiagnosticState> = emptyMap(),
    onRequestLocalPermission: () -> Unit = {},
    onRescanLocalMusic: () -> Unit = {},
    onProbeServerHealth: (String) -> Unit = {},
    onRunServerSpeedTest: (String) -> Unit = {},
    onRescanServer: (String) -> Unit = {},
    onOpenServer: (String?) -> Unit = { onOpenServers() },
    scrobbleConfig: AndroidScrobbleConfig = AndroidScrobbleConfig(
        lastfm = AndroidLastFmConfig(false, "", "", "", ""),
        listenbrainz = AndroidListenBrainzConfig(false, "", ""),
    ),
    onSaveScrobbleConfig: (AndroidScrobbleConfig) -> Unit = {},
    connectState: ConnectUiState = ConnectUiState(),
    onOpenConnect: () -> Unit = {},
    statsSummary: AndroidStatsSummary? = null,
    onOpenStats: () -> Unit = {},
    localTracks: List<LocalAudioTrack> = emptyList(),
    onPlayLocalTrack: (AndroidTrack, List<AndroidTrack>) -> Unit = { _, _ -> },
    localAvailableFolders: List<String> = emptyList(),
    onToggleExcludeNonMusic: (Boolean) -> Unit = {},
    onToggleLocalMusicFolder: (String) -> Unit = {},
    onAddLocalMusicFolder: () -> Unit = {},
    onClearLocalMusicFolders: () -> Unit = {},
) {
    val strings = LocalSonataStrings.current
    val srcStrings = remember(settings.appLanguage) { sourcesStrings(settings.appLanguage) }
    val ecoStrings = remember(settings.appLanguage) { ecosystemStrings(settings.appLanguage) }
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
                        is SettingsRoute.Category -> p.category.localizedTitle(strings, srcStrings, ecoStrings)
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

                            val pageTitle = activeCategory?.localizedTitle(strings, srcStrings, ecoStrings) ?: strings.settingsTitle
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
                                    }
                                    Spacer(Modifier.height(14.dp))
                                }

                                // Grouped Category List (single Sources / Kaynaklar entry)
                                if (!search.active) {
                                    val sourcesValue = when {
                                        settings.personalOnlyMode -> srcStrings.personalOnlyModeBadge
                                        servers.isNotEmpty() -> strings.servers.enabledCountSummary(enabledServerCount, servers.size)
                                        else -> srcStrings.localTracksCount(localTrackCount)
                                    }
                                    SettingsGroup(
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        secondaryText = secondaryText,
                                    ) {
                                        SettingsCategory.entries.forEach { cat ->
                                            val subtitle = when (cat) {
                                                SettingsCategory.SOURCES ->
                                                    if (settings.personalOnlyMode) {
                                                        srcStrings.personalOnlyModeBadge
                                                    } else {
                                                        srcStrings.sourcesSubtitle
                                                    }
                                                SettingsCategory.DOWNLOADS ->
                                                    "${strings.tracksCount(downloadStorageStats.trackCount)} · ${formatStorageBytes(downloadStorageStats.downloadedBytes)}"
                                                SettingsCategory.PLAYBACK -> strings.playbackSummary
                                                SettingsCategory.CONNECT ->
                                                    connectState.activePeer?.let { ecoStrings.connectConnectedTo(it.deviceName) }
                                                        ?: ecoStrings.connectSubtitle
                                                SettingsCategory.SCROBBLING -> {
                                                    val activeCount = listOf(
                                                        scrobbleConfig.lastfm.enabled,
                                                        scrobbleConfig.listenbrainz.enabled,
                                                    ).count { it }
                                                    if (activeCount > 0) ecoStrings.scrobbleThresholdHint else ecoStrings.scrobblingSubtitle
                                                }
                                                SettingsCategory.CONTENT -> strings.contentSummary
                                                SettingsCategory.APPEARANCE -> {
                                                    val modeLabel = when (settings.themeMode) {
                                                        ThemeMode.SYSTEM -> strings.themeModeSystem
                                                        ThemeMode.LIGHT -> strings.themeModeLight
                                                        ThemeMode.DARK -> strings.themeModeDark
                                                    }
                                                    "$modeLabel · ${appLanguageLabel(settings.appLanguage, strings)}"
                                                }
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
                                                title = cat.localizedTitle(strings, srcStrings, ecoStrings),
                                                subtitle = subtitle,
                                                value = if (cat == SettingsCategory.SOURCES) sourcesValue else null,
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
                                            "Sources",
                                            "Kaynaklar",
                                            "Источники",
                                            "Personal",
                                            "Kişisel",
                                            "Личное",
                                            "Modules",
                                            "Modüller",
                                            "JioSaavn",
                                            "Local",
                                            "Yerel",
                                            "Server",
                                            "Sunucu",
                                            "Сервер",
                                            "Jellyfin",
                                            "Subsonic",
                                            "SMB",
                                            "WebDAV",
                                            "Speed Test",
                                            "Hız Testi",
                                            "Report",
                                            "History",
                                        )
                                    ) {
                                        matchCount++
                                        SourcesHubContent(
                                            settings = settings,
                                            signedIn = signedIn,
                                            localTrackCount = localTrackCount,
                                            localPermissionGranted = localPermissionGranted,
                                            servers = servers,
                                            serverDiagnostics = serverDiagnostics,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onTogglePersonalOnlyMode = { enabled ->
                                                onUpdateSettings { it.copy(personalOnlyMode = enabled) }
                                            },
                                            onToggleJioSaavn = { enabled ->
                                                onUpdateSettings { it.copy(jioSaavnEnabled = enabled) }
                                            },
                                            onToggleAttributePersonalToYtm = { enabled ->
                                                onUpdateSettings { it.copy(attributePersonalToYtm = enabled) }
                                            },
                                            onToggleLocalMusic = { enabled ->
                                                onUpdateSettings { it.copy(localMusicEnabled = enabled) }
                                            },
                                            onRequestLocalPermission = onRequestLocalPermission,
                                            onRescanLocalMusic = onRescanLocalMusic,
                                            onProbeServerHealth = onProbeServerHealth,
                                            onRunServerSpeedTest = onRunServerSpeedTest,
                                            onRescanServer = onRescanServer,
                                            onOpenServers = onOpenServer,
                                            searchQuery = searchQuery,
                                            localTracks = localTracks,
                                            onPlayLocalTrack = onPlayLocalTrack,
                                            localAvailableFolders = localAvailableFolders,
                                            onToggleExcludeNonMusic = onToggleExcludeNonMusic,
                                            onToggleLocalMusicFolder = onToggleLocalMusicFolder,
                                            onAddLocalMusicFolder = onAddLocalMusicFolder,
                                            onClearLocalMusicFolders = onClearLocalMusicFolders,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

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
                                            "Gapless",
                                            "Aralıksız",
                                            "Непрерывное",
                                            "Autoplay",
                                            "Loudness",
                                            "Normalization",
                                            "Crossfade",
                                            "Geçiş",
                                            "Кроссфейд",
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
                                            eco = ecoStrings,
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
                                            "Connect",
                                            "Sonata Connect",
                                            "LAN",
                                            "PIN",
                                            "Transfer",
                                            "Aktar",
                                            "Передать",
                                            "Pair",
                                            "Eşleştir",
                                        )
                                    ) {
                                        matchCount++
                                        ConnectCategoryContent(
                                            settings = settings,
                                            eco = ecoStrings,
                                            connectState = connectState,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onUpdateSettings = onUpdateSettings,
                                            onOpenConnect = onOpenConnect,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }

                                    if (search.matches(
                                            "Scrobble",
                                            "Scrobbling",
                                            "Skroplama",
                                            "Скробблинг",
                                            "Last.fm",
                                            "ListenBrainz",
                                        )
                                    ) {
                                        matchCount++
                                        ScrobblingCategoryContent(
                                            eco = ecoStrings,
                                            scrobbleConfig = scrobbleConfig,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            accentTint = accentTint,
                                            onSaveScrobbleConfig = onSaveScrobbleConfig,
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
                                            "Basic",
                                            "Dark",
                                            "Light",
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

                                    if (search.matches("Backup", "Export", "Import", "JSON", "Data", "Stats", "Listening", "İstatistik", "Статистика")) {
                                        matchCount++
                                        YourDataCategoryContent(
                                            eco = ecoStrings,
                                            statsSummary = statsSummary,
                                            cardBg = cardBg,
                                            dividerColor = dividerColor,
                                            primaryText = primaryText,
                                            secondaryText = secondaryText,
                                            onOpenStats = onOpenStats,
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
                                    SettingsCategory.SOURCES -> SourcesHubContent(
                                        settings = settings,
                                        signedIn = signedIn,
                                        localTrackCount = localTrackCount,
                                        localPermissionGranted = localPermissionGranted,
                                        servers = servers,
                                        serverDiagnostics = serverDiagnostics,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onTogglePersonalOnlyMode = { enabled ->
                                            onUpdateSettings { it.copy(personalOnlyMode = enabled) }
                                        },
                                        onToggleJioSaavn = { enabled ->
                                            onUpdateSettings { it.copy(jioSaavnEnabled = enabled) }
                                        },
                                        onToggleAttributePersonalToYtm = { enabled ->
                                            onUpdateSettings { it.copy(attributePersonalToYtm = enabled) }
                                        },
                                        onToggleLocalMusic = { enabled ->
                                            onUpdateSettings { it.copy(localMusicEnabled = enabled) }
                                        },
                                        onRequestLocalPermission = onRequestLocalPermission,
                                        onRescanLocalMusic = onRescanLocalMusic,
                                        onProbeServerHealth = onProbeServerHealth,
                                        onRunServerSpeedTest = onRunServerSpeedTest,
                                        onRescanServer = onRescanServer,
                                        onOpenServers = onOpenServer,
                                        localTracks = localTracks,
                                        onPlayLocalTrack = onPlayLocalTrack,
                                        localAvailableFolders = localAvailableFolders,
                                        onToggleExcludeNonMusic = onToggleExcludeNonMusic,
                                        onToggleLocalMusicFolder = onToggleLocalMusicFolder,
                                        onAddLocalMusicFolder = onAddLocalMusicFolder,
                                        onClearLocalMusicFolders = onClearLocalMusicFolders,
                                    )

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
                                        eco = ecoStrings,
                                        search = search,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        accentTint = accentTint,
                                        onUpdateSettings = onUpdateSettings,
                                    )

                                    SettingsCategory.CONNECT -> ConnectCategoryContent(
                                        settings = settings,
                                        eco = ecoStrings,
                                        connectState = connectState,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onUpdateSettings = onUpdateSettings,
                                        onOpenConnect = onOpenConnect,
                                    )

                                    SettingsCategory.SCROBBLING -> ScrobblingCategoryContent(
                                        eco = ecoStrings,
                                        scrobbleConfig = scrobbleConfig,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        accentTint = accentTint,
                                        onSaveScrobbleConfig = onSaveScrobbleConfig,
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
                                        eco = ecoStrings,
                                        statsSummary = statsSummary,
                                        cardBg = cardBg,
                                        dividerColor = dividerColor,
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                        onOpenStats = onOpenStats,
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

