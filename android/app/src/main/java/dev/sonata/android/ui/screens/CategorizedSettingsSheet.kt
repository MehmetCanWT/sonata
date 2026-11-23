/*
 * Categorized Settings modal sheet for Sonata Android.
 *
 * Replaces the flat settings list with a searchable overview and 5 drill-down
 * category pages wired to SonataSettingsState.
 */
package dev.sonata.android.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.isGlassSupported
import dev.sonata.android.sonataThemeGlassButton
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidUpdateInfo
import kotlin.math.roundToInt

enum class SettingsCategory(
    val title: String,
    val subtitle: String,
    val icon: ImageVector,
    val accent: Color,
    val keywords: String,
) {
    PLAYBACK(
        title = "Playback & Audio",
        subtitle = "Quality, crossfade, speed, skip silence",
        icon = SonataIcons.GraphicEq,
        accent = Color(0xFFE5E5EA),
        keywords = "playback audio quality lossless high medium low crossfade skip silence speed autoplay",
    ),
    APPEARANCE(
        title = "Appearance & Liquid Glass",
        subtitle = "Theme, glass refraction, backdrop, FPS",
        icon = SonataIcons.Sparkles,
        accent = Color(0xFFE5E5EA),
        keywords = "appearance theme dark light amoled system liquid glass blur animation canvas artwork tint backdrop mesh waves fps 60 120",
    ),
    LYRICS(
        title = "Synced Lyrics",
        subtitle = "Blur, translation, romanization, timing",
        icon = SonataIcons.LyricsQuote,
        accent = Color(0xFFE5E5EA),
        keywords = "lyrics blur translation language romanization offset sync word syllable",
    ),
    DOWNLOADS(
        title = "Downloads & Storage",
        subtitle = "Offline quality, Wi-Fi, cache, backup",
        icon = SonataIcons.Download,
        accent = Color(0xFFE5E5EA),
        keywords = "downloads offline storage wifi export music liked cache clear backup json import",
    ),
    MODULES(
        title = "Modules",
        subtitle = "External audio sources (JioSaavn)",
        icon = SonataIcons.Sparkles,
        accent = Color(0xFFE5E5EA),
        keywords = "modules sources jiosaavn external audio 320kbps aac higher quality stream search",
    ),
    ACCOUNT(
        title = "Account & Updates",
        subtitle = "YouTube Music session & app updates",
        icon = SonataIcons.Person,
        accent = Color(0xFFE5E5EA),
        keywords = "account youtube music sign in out updates version apk check release",
    ),
}

private val EqPresets = listOf("Flat", "Bass Boost", "Vocal", "Acoustic", "Electronic", "Late Night")
private val TranslationLangs = listOf("en" to "English", "tr" to "Turkish", "es" to "Spanish", "ja" to "Japanese", "ko" to "Korean", "de" to "German")

/**
 * Full-screen flat Settings page for Sonata Android.
 *
 * Matches the flat `#000000` background and `#161618` neutral gray grouped
 * card aesthetic with zero Liquid Glass refraction, a top-left back circle,
 * bold Settings heading, search bar, and drill-down categories.
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
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onClearMediaCache: () -> Unit,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val statusTop = rememberSafeTopInset()
    var activeCategory by remember { mutableStateOf<SettingsCategory?>(null) }

    BackHandler(enabled = activeCategory != null) {
        activeCategory = null
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(tokens.settingsBg)
            .padding(top = statusTop)
            .navigationBarsPadding(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxSize()
                .padding(horizontal = PAGE_GUTTER),
        ) {
        // Minimalist Settings & Support top header matching reference layout
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 14.dp, bottom = 18.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(38.dp)
                    .sonataThemeGlassButton(
                        shape = CircleShape,
                        fallbackColor = tokens.settingsCard,
                        shadowElevation = 2.dp,
                    )
                    .clickable {
                        if (activeCategory != null) {
                            activeCategory = null
                        } else {
                            onBack()
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = SonataIcons.ChevronLeft,
                    contentDescription = if (activeCategory != null) "Back to Settings" else "Back",
                    tint = tokens.onSurfacePrimary,
                    modifier = Modifier.size(19.dp),
                )
            }
            Text(
                text = activeCategory?.title ?: "Settings & Support",
                style = MaterialTheme.typography.headlineMedium.copy(
                    fontSize = 23.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = (-0.3).sp,
                ),
                color = tokens.onSurfacePrimary,
                modifier = Modifier.weight(1f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        SettingsCategoryNavigator(
            activeCategory = activeCategory,
            onSelectCategory = { activeCategory = it },
            settings = settings,
            signedIn = signedIn,
            accountProfile = accountProfile,
            supports120Hz = supports120Hz,
            statusMessage = statusMessage,
            updateInfo = updateInfo,
            checkingUpdates = checkingUpdates,
            installedVersionName = installedVersionName,
            onUpdateSettings = onUpdateSettings,
            onSignIn = onSignIn,
            onSignOut = onSignOut,
            onClearMediaCache = onClearMediaCache,
            onExportBackup = onExportBackup,
            onImportBackup = onImportBackup,
            onCheckForUpdates = onCheckForUpdates,
            onDownloadUpdate = onDownloadUpdate,
        )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CategorizedSettingsSheet(
    settings: SonataSettingsState,
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    supports120Hz: Boolean,
    statusMessage: String,
    updateInfo: AndroidUpdateInfo?,
    checkingUpdates: Boolean,
    installedVersionName: String,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onClearMediaCache: () -> Unit,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
    onDismiss: () -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var activeCategory by remember { mutableStateOf<SettingsCategory?>(null) }

    BackHandler(enabled = activeCategory != null) {
        activeCategory = null
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = tokens.settingsBg,
        contentColor = tokens.onSurfacePrimary,
        dragHandle = null,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(tokens.settingsBg)
                .navigationBarsPadding()
                .padding(horizontal = PAGE_GUTTER),
        ) {
            // Top bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 18.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                if (activeCategory != null) {
                    CircleGlyph(
                        icon = SonataIcons.ChevronLeft,
                        contentDescription = "Back to Settings",
                        onClick = { activeCategory = null },
                    )
                }
                Text(
                    text = activeCategory?.title ?: "Settings & Support",
                    style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                    color = tokens.onSurfacePrimary,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                CircleGlyph(
                    icon = SonataIcons.Close,
                    contentDescription = "Close Settings",
                    onClick = onDismiss,
                )
            }

            SettingsCategoryNavigator(
                activeCategory = activeCategory,
                onSelectCategory = { activeCategory = it },
                settings = settings,
                signedIn = signedIn,
                accountProfile = accountProfile,
                supports120Hz = supports120Hz,
                statusMessage = statusMessage,
                updateInfo = updateInfo,
                checkingUpdates = checkingUpdates,
                installedVersionName = installedVersionName,
                onUpdateSettings = onUpdateSettings,
                onSignIn = onSignIn,
                onSignOut = onSignOut,
                onClearMediaCache = onClearMediaCache,
                onExportBackup = onExportBackup,
                onImportBackup = onImportBackup,
                onCheckForUpdates = onCheckForUpdates,
                onDownloadUpdate = onDownloadUpdate,
            )
        }
    }
}

@Composable
private fun SettingsCategoryNavigator(
    activeCategory: SettingsCategory?,
    onSelectCategory: (SettingsCategory) -> Unit,
    settings: SonataSettingsState,
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    supports120Hz: Boolean,
    statusMessage: String,
    updateInfo: AndroidUpdateInfo?,
    checkingUpdates: Boolean,
    installedVersionName: String,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onClearMediaCache: () -> Unit,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
) {
    AnimatedContent(
        targetState = activeCategory,
        transitionSpec = {
            if (targetState != null) {
                (slideInHorizontally(tween(220)) { it / 3 } + fadeIn(tween(180))) togetherWith
                    (slideOutHorizontally(tween(200)) { -it / 4 } + fadeOut(tween(150)))
            } else {
                (slideInHorizontally(tween(220)) { -it / 3 } + fadeIn(tween(180))) togetherWith
                    (slideOutHorizontally(tween(200)) { it / 4 } + fadeOut(tween(150)))
            }
        },
        label = "settingsCategoryNav",
    ) { category ->
        if (category == null) {
            SettingsOverviewContent(
                signedIn = signedIn,
                accountProfile = accountProfile,
                updateInfo = updateInfo,
                installedVersionName = installedVersionName,
                onSelectCategory = onSelectCategory,
                onSignIn = onSignIn,
                onSignOut = onSignOut,
                onClearMediaCache = onClearMediaCache,
                onCheckForUpdates = onCheckForUpdates,
            )
        } else {
            when (category) {
                SettingsCategory.PLAYBACK -> PlaybackSettingsPage(
                    settings = settings,
                    onUpdate = onUpdateSettings,
                )
                SettingsCategory.APPEARANCE -> AppearanceSettingsPage(
                    settings = settings,
                    supports120Hz = supports120Hz,
                    onUpdate = onUpdateSettings,
                )
                SettingsCategory.LYRICS -> LyricsSettingsPage(
                    settings = settings,
                    onUpdate = onUpdateSettings,
                )
                SettingsCategory.DOWNLOADS -> DownloadsStorageSettingsPage(
                    settings = settings,
                    statusMessage = statusMessage,
                    onUpdate = onUpdateSettings,
                    onClearMediaCache = onClearMediaCache,
                    onExportBackup = onExportBackup,
                    onImportBackup = onImportBackup,
                )
                SettingsCategory.MODULES -> ModulesSettingsPage(
                    settings = settings,
                    onUpdate = onUpdateSettings,
                )
                SettingsCategory.ACCOUNT -> AccountUpdatesSettingsPage(
                    signedIn = signedIn,
                    accountProfile = accountProfile,
                    updateInfo = updateInfo,
                    checkingUpdates = checkingUpdates,
                    installedVersionName = installedVersionName,
                    onSignIn = onSignIn,
                    onSignOut = onSignOut,
                    onCheckForUpdates = onCheckForUpdates,
                    onDownloadUpdate = onDownloadUpdate,
                )
            }
        }
    }
}

@Composable
private fun SettingsOverviewContent(
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    updateInfo: AndroidUpdateInfo?,
    installedVersionName: String,
    onSelectCategory: (SettingsCategory) -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onClearMediaCache: () -> Unit,
    onCheckForUpdates: () -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val currentVer = updateInfo?.currentVersion ?: installedVersionName
    val updateBadge = if (updateInfo?.updateAvailable == true) "Update v${updateInfo.latestVersion}" else "v$currentVer"

    LazyColumn(
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        item(key = "hub_main") {
            SettingsHubSection("Settings Hub") {
                SettingsHubRow(
                    icon = SonataIcons.Person,
                    title = "Account & Session",
                    onClick = { onSelectCategory(SettingsCategory.ACCOUNT) },
                )
                SettingsHubRow(
                    icon = SonataIcons.Sparkles,
                    title = "Appearance & Themes",
                    onClick = { onSelectCategory(SettingsCategory.APPEARANCE) },
                )
                SettingsHubRow(
                    icon = SonataIcons.GraphicEq,
                    title = "Playback & Audio",
                    onClick = { onSelectCategory(SettingsCategory.PLAYBACK) },
                )
                SettingsHubRow(
                    icon = SonataIcons.LyricsQuote,
                    title = "Synced Lyrics",
                    onClick = { onSelectCategory(SettingsCategory.LYRICS) },
                )
                SettingsHubRow(
                    icon = SonataIcons.MusicNote,
                    title = "Audio Modules",
                    onClick = { onSelectCategory(SettingsCategory.MODULES) },
                )
            }
        }

        item(key = "hub_account_mgmt") {
            SettingsHubSection(title = "Account Management", badgeText = "New") {
                val avatarRequest = rememberArtworkRequest(
                    url = if (signedIn) accountProfile?.avatarUrl else null,
                    widthPx = 96,
                    heightPx = 96,
                )
                SettingsHubRow(
                    icon = SonataIcons.Person,
                    title = if (signedIn) "Account Switching" else "Connect Account",
                    onClick = onSignIn,
                    trailingContent = {
                        Box(
                            modifier = Modifier
                                .size(24.dp)
                                .clip(CircleShape)
                                .background(tokens.onSurfacePrimary.copy(alpha = 0.09f)),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (avatarRequest != null) {
                                AsyncImage(
                                    model = avatarRequest,
                                    contentDescription = null,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.Person,
                                    contentDescription = null,
                                    tint = tokens.onSurfaceSecondary,
                                    modifier = Modifier.size(14.dp),
                                )
                            }
                        }
                    },
                )
                if (signedIn) {
                    SettingsHubRow(
                        icon = SonataIcons.Close,
                        title = "Sign Out",
                        onClick = onSignOut,
                    )
                }
            }
        }

        item(key = "hub_storage") {
            SettingsHubSection("Storage & Backup") {
                SettingsHubRow(
                    icon = SonataIcons.Folder,
                    title = "Downloads & Backup",
                    onClick = { onSelectCategory(SettingsCategory.DOWNLOADS) },
                )
                SettingsHubRow(
                    icon = SonataIcons.Close,
                    title = "Clear Media Cache",
                    onClick = onClearMediaCache,
                )
            }
        }

        item(key = "hub_app") {
            SettingsHubSection("App") {
                SettingsHubRow(
                    icon = SonataIcons.Download,
                    title = "App Updates",
                    onClick = {
                        onCheckForUpdates()
                        onSelectCategory(SettingsCategory.ACCOUNT)
                    },
                    trailingContent = {
                        Text(
                            text = updateBadge,
                            style = MaterialTheme.typography.labelMedium,
                            color = if (updateInfo?.updateAvailable == true) {
                                tokens.petalLavender
                            } else {
                                tokens.onSurfaceSecondary.copy(alpha = 0.78f)
                            },
                        )
                    },
                )
            }
        }
    }
}

@Composable
private fun SettingsHubSection(
    title: String,
    badgeText: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val tokens = LocalLunariaColors.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(
            modifier = Modifier.padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.labelMedium.copy(
                    fontSize = 12.5.sp,
                    fontWeight = FontWeight.Medium,
                ),
                color = tokens.onSurfaceSecondary.copy(alpha = 0.86f),
            )
            if (!badgeText.isNullOrBlank()) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(Color(0xFFDF3C30))
                        .padding(horizontal = 7.dp, vertical = 1.5.dp),
                ) {
                    Text(
                        text = badgeText,
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = Color.White,
                    )
                }
            }
        }
        val cardShape = RoundedCornerShape(16.dp)
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(cardShape)
                .background(tokens.settingsCard, cardShape),
            content = content,
        )
    }
}

@Composable
private fun SettingsHubRow(
    icon: ImageVector,
    title: String,
    onClick: () -> Unit,
    trailingContent: (@Composable () -> Unit)? = null,
) {
    val tokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 15.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = tokens.onSurfacePrimary.copy(alpha = 0.78f),
            modifier = Modifier.size(19.dp),
        )
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall.copy(
                fontSize = 15.5.sp,
                fontWeight = FontWeight.Medium,
            ),
            color = tokens.onSurfacePrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (trailingContent != null) {
            trailingContent()
        }
        Icon(
            imageVector = SonataIcons.ChevronRight,
            contentDescription = null,
            tint = tokens.onSurfaceSecondary.copy(alpha = 0.65f),
            modifier = Modifier.size(16.dp),
        )
    }
}

@Composable
private fun PlaybackSettingsPage(
    settings: SonataSettingsState,
    onUpdate: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "pb_quality") {
            SettingsSection("STREAMING QUALITY") {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Audio Quality Ceiling",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = LocalLunariaColors.current.onSurfacePrimary,
                    )
                    PillSegmentedControl(
                        options = AudioQualityTier.entries.map { it.name.lowercase().replaceFirstChar(Char::uppercase) },
                        selectedIndex = AudioQualityTier.entries.indexOf(settings.audioQuality),
                        onSelect = { idx ->
                            val next = AudioQualityTier.entries[idx]
                            onUpdate {
                                it.copy(
                                    audioQuality = next,
                                    preferHighQuality = next == AudioQualityTier.LOSSLESS || next == AudioQualityTier.HIGH,
                                )
                            }
                        },
                    )
                }
                SettingsDivider()
                SettingsToggleRow(
                    title = "Autoplay Similar Tracks",
                    subtitle = "Keep playing radio tracks when queue ends",
                    checked = settings.autoplayEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(autoplayEnabled = v) } },
                )
            }
        }

        item(key = "pb_transitions") {
            SettingsSection("TRANSITIONS & PROCESSING") {
                SettingsToggleRow(
                    title = "Crossfade",
                    subtitle = "Blend smoothly between consecutive tracks",
                    checked = settings.crossfadeEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(crossfadeEnabled = v) } },
                )
                if (settings.crossfadeEnabled) {
                    SettingsDivider()
                    SettingsSliderRow(
                        title = "Crossfade Duration",
                        valueLabel = "${settings.crossfadeSeconds}s",
                        value = settings.crossfadeSeconds.toFloat(),
                        valueRange = 1f..12f,
                        onValueChange = { v -> onUpdate { it.copy(crossfadeSeconds = v.roundToInt()) } },
                    )
                }
                SettingsDivider()
                SettingsToggleRow(
                    title = "Skip Silence",
                    subtitle = "Trim extended dead air at track edges",
                    checked = settings.skipSilenceEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(skipSilenceEnabled = v) } },
                )
                SettingsDivider()
                SettingsSliderRow(
                    title = "Playback Speed",
                    valueLabel = String.format("%.2fx", settings.playbackSpeed),
                    value = settings.playbackSpeed,
                    valueRange = 0.5f..2.0f,
                    onValueChange = { v ->
                        val stepped = (v * 20f).roundToInt() / 20f
                        onUpdate { it.copy(playbackSpeed = stepped.coerceIn(0.5f, 2.0f)) }
                    },
                )
            }
        }
    }
}

@Composable
private fun AppearanceSettingsPage(
    settings: SonataSettingsState,
    supports120Hz: Boolean,
    onUpdate: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "ap_theme") {
            val tokens = LocalLunariaColors.current
            val activeDesign = settings.resolvedDesignTheme()
            val colorModeOptions = listOf("Light", "Dark", "System", "AMOLED")
            val selectedColorIdx = when (settings.themeMode) {
                ThemeMode.VIOLET_LIGHT, ThemeMode.MOONFLOWER_LIGHT -> 0
                ThemeMode.VIOLET_DARK, ThemeMode.MOONFLOWER_DARK -> 1
                ThemeMode.SYSTEM -> 2
                ThemeMode.AMOLED -> 3
            }

            SettingsSection("THEME & DESIGN") {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        text = "Design Theme",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = tokens.onSurfacePrimary,
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        ThemeDesignCard(
                            title = "Violet",
                            subtitle = "Orchid glass, layered shadows & stacked deck",
                            previewTop = Color(0xFFE5D7F6),
                            previewBottom = Color(0xFF7B52AB),
                            selected = activeDesign == DesignTheme.VIOLET,
                            onClick = {
                                onUpdate { s ->
                                    val nextMode = when (s.themeMode) {
                                        ThemeMode.MOONFLOWER_LIGHT -> ThemeMode.VIOLET_LIGHT
                                        ThemeMode.MOONFLOWER_DARK -> ThemeMode.VIOLET_DARK
                                        ThemeMode.SYSTEM, ThemeMode.AMOLED -> s.themeMode
                                        else -> s.themeMode
                                    }
                                    s.copy(designTheme = DesignTheme.VIOLET, themeMode = nextMode)
                                }
                            },
                            modifier = Modifier.weight(1f),
                        )
                        ThemeDesignCard(
                            title = "Moonflower",
                            subtitle = "Gossamer silk veils, blush pill & inline bar",
                            previewTop = Color(0xFFEDE4F7),
                            previewBottom = Color(0xFFF2CBE0),
                            selected = activeDesign == DesignTheme.MOONFLOWER,
                            onClick = {
                                onUpdate { s ->
                                    val nextMode = when (s.themeMode) {
                                        ThemeMode.VIOLET_LIGHT -> ThemeMode.MOONFLOWER_LIGHT
                                        ThemeMode.VIOLET_DARK -> ThemeMode.MOONFLOWER_DARK
                                        ThemeMode.SYSTEM, ThemeMode.AMOLED -> s.themeMode
                                        else -> s.themeMode
                                    }
                                    s.copy(designTheme = DesignTheme.MOONFLOWER, themeMode = nextMode)
                                }
                            },
                            modifier = Modifier.weight(1f),
                        )
                    }
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = "Color Mode",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = tokens.onSurfacePrimary,
                    )
                    PillSegmentedControl(
                        options = colorModeOptions,
                        selectedIndex = selectedColorIdx,
                        onSelect = { idx ->
                            onUpdate { s ->
                                val design = s.resolvedDesignTheme()
                                val nextMode = when (idx) {
                                    0 -> if (design == DesignTheme.VIOLET) ThemeMode.VIOLET_LIGHT else ThemeMode.MOONFLOWER_LIGHT
                                    1 -> if (design == DesignTheme.VIOLET) ThemeMode.VIOLET_DARK else ThemeMode.MOONFLOWER_DARK
                                    2 -> ThemeMode.SYSTEM
                                    else -> ThemeMode.AMOLED
                                }
                                s.copy(designTheme = design, themeMode = nextMode)
                            }
                        },
                    )
                }
                SettingsDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Now Playing Backdrop",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = tokens.onSurfacePrimary,
                    )
                    PillSegmentedControl(
                        options = listOf("Artwork Mesh", "Waves", "Blurred"),
                        selectedIndex = PlayerBackdropMode.entries.indexOf(settings.playerBackdropMode),
                        onSelect = { idx ->
                            onUpdate { it.copy(playerBackdropMode = PlayerBackdropMode.entries[idx]) }
                        },
                    )
                }
                SettingsDivider()
                SettingsToggleRow(
                    title = "Artwork-Tinted Pages",
                    subtitle = "Tint album & playlist headers from cover palette",
                    checked = settings.artworkTintedPages,
                    onCheckedChange = { v -> onUpdate { it.copy(artworkTintedPages = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Animated Album Art (Canvas)",
                    subtitle = "Play looping motion covers when available",
                    checked = settings.animatedArtwork,
                    onCheckedChange = { v -> onUpdate { it.copy(animatedArtwork = v) } },
                )
            }
        }

        item(key = "ap_glass") {
            SettingsSection("LIQUID GLASS & PERFORMANCE") {
                SettingsToggleRow(
                    title = "Liquid Glass Navigation",
                    subtitle = if (isGlassSupported()) "Real-time backdrop refraction" else "Requires Android 12+",
                    checked = settings.liquidGlassEnabled,
                    enabled = isGlassSupported(),
                    onCheckedChange = { v -> onUpdate { it.copy(liquidGlassEnabled = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Glass Performance Mode",
                    subtitle = "Use lighter frosted blur without lens shader",
                    checked = settings.glassPerformance,
                    enabled = settings.liquidGlassEnabled && isGlassSupported(),
                    onCheckedChange = { v -> onUpdate { it.copy(glassPerformance = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Reduce Dynamic Blur",
                    subtitle = "Replace live blur layers with solid glass surfaces",
                    checked = settings.reduceDynamicBlur,
                    onCheckedChange = { v -> onUpdate { it.copy(reduceDynamicBlur = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Reduce Animation",
                    subtitle = "Snap transitions and freeze ambient drift",
                    checked = settings.reduceAnimation,
                    onCheckedChange = { v -> onUpdate { it.copy(reduceAnimation = v) } },
                )
                SettingsDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Display Frame Rate",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = LocalLunariaColors.current.onSurfacePrimary,
                    )
                    val context = androidx.compose.ui.platform.LocalContext.current
                    val fpsTiers = remember(context, supports120Hz) {
                        val display = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                            context.display
                        } else {
                            @Suppress("DEPRECATION")
                            (context.getSystemService(android.content.Context.WINDOW_SERVICE) as? android.view.WindowManager)?.defaultDisplay
                        }
                        val currentMode = display?.mode
                        val modes = display?.supportedModes
                        val sameResModes = if (currentMode != null && modes != null) {
                            modes.filter {
                                it.physicalWidth == currentMode.physicalWidth &&
                                    it.physicalHeight == currentMode.physicalHeight
                            }.ifEmpty { modes.toList() }
                        } else {
                            modes?.toList().orEmpty()
                        }
                        val maxHz = sameResModes.maxOfOrNull { it.refreshRate } ?: if (supports120Hz) 120f else 60f
                        val has90Hz = maxHz >= 85f && (sameResModes.any { it.refreshRate in 85f..95f } || maxHz < 115f)
                        val has120Hz = supports120Hz || maxHz >= 115f
                        buildList {
                            add(60)
                            if (has90Hz) add(90)
                            if (has120Hz) add(120)
                        }
                    }
                    val selectedFpsIdx = remember(settings.fpsLimit, fpsTiers) {
                        fpsTiers.indexOfLast { it <= settings.fpsLimit }.coerceAtLeast(0)
                    }
                    PillSegmentedControl(
                        options = remember(fpsTiers) { fpsTiers.map { "$it FPS" } },
                        selectedIndex = selectedFpsIdx,
                        onSelect = { idx ->
                            val chosen = fpsTiers.getOrElse(idx) { 60 }
                            onUpdate { it.copy(fpsLimit = chosen) }
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun LyricsSettingsPage(
    settings: SonataSettingsState,
    onUpdate: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "ly_display") {
            SettingsSection("LYRICS RENDERING") {
                SettingsToggleRow(
                    title = "Depth-of-Field Blur",
                    subtitle = "Softly blur inactive lyric verses",
                    checked = settings.lyricsBlurEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(lyricsBlurEnabled = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Prioritize Word-Synced Lyrics",
                    subtitle = "Prefer syllable/word timing over line-only",
                    checked = settings.prioritizeWordSync,
                    onCheckedChange = { v -> onUpdate { it.copy(prioritizeWordSync = v) } },
                )
                SettingsDivider()
                SettingsSliderRow(
                    title = "Default Timing Offset",
                    valueLabel = String.format("%+.1fs", settings.lyricsOffsetMs / 1000f),
                    value = settings.lyricsOffsetMs.toFloat(),
                    valueRange = -5000f..5000f,
                    onValueChange = { v ->
                        val snapped = (v / 100f).roundToInt() * 100L
                        onUpdate { it.copy(lyricsOffsetMs = snapped.coerceIn(-5000L, 5000L)) }
                    },
                )
            }
        }

        item(key = "ly_translation") {
            SettingsSection("TRANSLATION & ROMANIZATION") {
                SettingsToggleRow(
                    title = "Show Lyrics Translation",
                    subtitle = "Display translated lines under each verse",
                    checked = settings.lyricsTranslationEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(lyricsTranslationEnabled = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Show Romanization",
                    subtitle = "Transliterate non-Latin scripts on demand",
                    checked = settings.lyricsRomanizationEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(lyricsRomanizationEnabled = v) } },
                )
                SettingsDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = "Translation Target Language",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = LocalLunariaColors.current.onSurfacePrimary,
                    )
                    TranslationLangs.chunked(3).forEach { row ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            row.forEach { (code, label) ->
                                val tokens = LocalLunariaColors.current
                                val selected = settings.lyricsTranslationLang.equals(code, ignoreCase = true)
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .clip(RoundedCornerShape(12.dp))
                                        .background(if (selected) tokens.petalLavender else tokens.onSurfacePrimary.copy(alpha = 0.08f))
                                        .clickable { onUpdate { it.copy(lyricsTranslationLang = code) } }
                                        .padding(vertical = 8.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = label,
                                        style = MaterialTheme.typography.labelSmall.copy(
                                            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                        ),
                                        color = if (selected) Color.White else tokens.onSurfacePrimary,
                                        maxLines = 1,
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

@Composable
private fun DownloadsStorageSettingsPage(
    settings: SonataSettingsState,
    statusMessage: String,
    onUpdate: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onClearMediaCache: () -> Unit,
    onExportBackup: () -> Unit,
    onImportBackup: () -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "dl_prefs") {
            SettingsSection("OFFLINE DOWNLOADS") {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Download Quality",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = LocalLunariaColors.current.onSurfacePrimary,
                    )
                    PillSegmentedControl(
                        options = listOf("High", "Standard"),
                        selectedIndex = DownloadQualityTier.entries.indexOf(settings.downloadQuality),
                        onSelect = { idx ->
                            onUpdate { it.copy(downloadQuality = DownloadQualityTier.entries[idx]) }
                        },
                    )
                }
                SettingsDivider()
                SettingsToggleRow(
                    title = "Wi-Fi Only Downloads",
                    subtitle = "Pause downloads on metered cellular networks",
                    checked = settings.wifiOnlyDownloads,
                    onCheckedChange = { v -> onUpdate { it.copy(wifiOnlyDownloads = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Export to Music/Sonata",
                    subtitle = "Save downloaded tracks in shared device storage",
                    checked = settings.exportDownloads,
                    onCheckedChange = { v -> onUpdate { it.copy(exportDownloads = v) } },
                )
                SettingsDivider()
                SettingsToggleRow(
                    title = "Auto-Download Liked Songs",
                    subtitle = "Automatically save tracks when you star/like them",
                    checked = settings.autoDownloadLiked,
                    onCheckedChange = { v -> onUpdate { it.copy(autoDownloadLiked = v) } },
                )
            }
        }

        item(key = "dl_cache_backup") {
            SettingsSection("CACHE & BACKUP") {
                SettingsActionRow(
                    title = "Clear Media Cache",
                    subtitle = statusMessage.ifBlank { "Frees cached streaming audio segments" },
                    onClick = onClearMediaCache,
                )
                SettingsDivider()
                SettingsActionRow(
                    title = "Export Settings Backup (JSON)",
                    subtitle = "Save preferences to a portable file (no secrets)",
                    onClick = onExportBackup,
                )
                SettingsDivider()
                SettingsActionRow(
                    title = "Import Settings Backup (JSON)",
                    subtitle = "Restore preferences from a previously exported file",
                    onClick = onImportBackup,
                )
            }
        }
    }
}

@Composable
private fun ModulesSettingsPage(
    settings: SonataSettingsState,
    onUpdate: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "modules_jiosaavn") {
            SettingsSection("EXTERNAL AUDIO MODULES") {
                SettingsToggleRow(
                    title = "JioSaavn",
                    subtitle = "Also search JioSaavn and upgrade playback when a higher-bitrate (up to 320 kbps AAC) stream is found",
                    checked = settings.jioSaavnEnabled,
                    onCheckedChange = { enabled -> onUpdate { it.copy(jioSaavnEnabled = enabled) } },
                )
                SettingsDivider()
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(
                        text = "Matching Notice",
                        style = MaterialTheme.typography.labelMedium.copy(
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.3.sp,
                        ),
                        color = Color(0xFFFFB74D),
                    )
                    Text(
                        text = "JioSaavn may occasionally match a different version, cover, or album on tracks with similar titles. Streams are only substituted when JioSaavn provides a strictly higher bitrate than YouTube Music.",
                        style = MaterialTheme.typography.bodySmall.copy(lineHeight = 18.sp),
                        color = LocalLunariaColors.current.onSurfaceSecondary,
                    )
                }
            }
        }
    }
}

@Composable
private fun AccountUpdatesSettingsPage(
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    updateInfo: AndroidUpdateInfo?,
    checkingUpdates: Boolean,
    installedVersionName: String,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
) {
    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(key = "ac_session") {
            val tokens = LocalLunariaColors.current
            SettingsSection("YOUTUBE MUSIC SESSION") {
                SettingsActionRow(
                    title = if (signedIn) accountProfile?.name ?: "Signed In" else "Connect YouTube Music",
                    subtitle = if (signedIn) {
                        accountProfile?.email ?: accountProfile?.handle ?: "Tap to sign out"
                    } else {
                        "Sign in for personalized mixes and playlists"
                    },
                    accentTitle = tokens.petalLavender,
                    onClick = { if (signedIn) onSignOut() else onSignIn() },
                )
            }
        }

        item(key = "ac_updates") {
            val tokens = LocalLunariaColors.current
            val currentVer = updateInfo?.currentVersion ?: installedVersionName
            val statusLabel = when {
                checkingUpdates -> "Checking GitHub Releases..."
                updateInfo?.updateAvailable == true -> "Update v${updateInfo.latestVersion} available"
                updateInfo != null -> "Up to date"
                else -> "Tap to check for new Android builds"
            }
            SettingsSection("UPDATES") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(enabled = !checkingUpdates, onClick = onCheckForUpdates)
                        .padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            text = "Sonata Android v$currentVer",
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                            color = tokens.onSurfacePrimary,
                        )
                        Text(
                            text = statusLabel,
                            style = MaterialTheme.typography.bodySmall,
                            color = if (updateInfo?.updateAvailable == true) tokens.petalLavender else tokens.onSurfaceSecondary,
                        )
                    }
                    if (checkingUpdates) {
                        CircularProgressIndicator(
                            color = tokens.petalLavender,
                            strokeWidth = 2.dp,
                            modifier = Modifier.size(18.dp),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = tokens.onSurfaceSecondary.copy(alpha = 0.7f),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                if (updateInfo?.updateAvailable == true) {
                    SettingsDivider()
                    SettingsActionRow(
                        title = "Download v${updateInfo.latestVersion}",
                        subtitle = "Download latest APK to Downloads",
                        accentTitle = tokens.petalLavender,
                        onClick = onDownloadUpdate,
                    )
                }
            }
        }
    }
}

@Composable
private fun ThemeDesignCard(
    title: String,
    subtitle: String,
    previewTop: Color,
    previewBottom: Color,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val shape = RoundedCornerShape(16.dp)
    val borderColor = if (selected) tokens.petalLavender else tokens.settingsDivider
    Column(
        modifier = modifier
            .clip(shape)
            .background(
                if (selected) tokens.petalLavender.copy(alpha = 0.14f) else tokens.onSurfacePrimary.copy(alpha = 0.04f),
            )
            .border(if (selected) 1.5.dp else 0.75.dp, borderColor, shape)
            .clickable(onClick = onClick)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(24.dp)
                    .clip(CircleShape)
                    .background(
                        androidx.compose.ui.graphics.Brush.linearGradient(listOf(previewTop, previewBottom)),
                    )
                    .border(0.75.dp, Color.White.copy(alpha = 0.65f), CircleShape),
            )
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                color = tokens.onSurfacePrimary,
            )
        }
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 11.5.sp, lineHeight = 15.sp),
            color = tokens.onSurfaceSecondary,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun SettingsFlatCard(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier = modifier
            .clip(shape)
            .background(tokens.settingsCard, shape)
            .border(0.5.dp, tokens.settingsDivider.copy(alpha = 0.75f), shape),
        content = content,
    )
}

@Composable
private fun SettingsSection(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    val tokens = LocalLunariaColors.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelSmall.copy(
                letterSpacing = 0.8.sp,
                fontWeight = FontWeight.SemiBold,
            ),
            color = tokens.onSurfaceSecondary,
            modifier = Modifier.padding(horizontal = 6.dp),
        )
        SettingsFlatCard(modifier = Modifier.fillMaxWidth(), content = content)
    }
}

@Composable
private fun SettingsDivider() {
    val tokens = LocalLunariaColors.current
    HorizontalDivider(
        color = tokens.settingsDivider,
        modifier = Modifier.padding(start = 16.dp),
    )
}

@Composable
private fun SettingsToggleRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    enabled: Boolean = true,
    onCheckedChange: (Boolean) -> Unit,
) {
    val tokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled) { onCheckedChange(!checked) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                color = if (enabled) tokens.onSurfacePrimary else tokens.onSurfacePrimary.copy(alpha = 0.4f),
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = if (enabled) tokens.onSurfaceSecondary else tokens.onSurfaceSecondary.copy(alpha = 0.5f),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            enabled = enabled,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = tokens.petalLavender,
                uncheckedThumbColor = Color.White,
                uncheckedTrackColor = tokens.onSurfaceSecondary.copy(alpha = 0.28f),
                uncheckedBorderColor = Color.Transparent,
            ),
        )
    }
}

@Composable
private fun SettingsSliderRow(
    title: String,
    valueLabel: String,
    value: Float,
    valueRange: ClosedFloatingPointRange<Float>,
    onValueChange: (Float) -> Unit,
) {
    val tokens = LocalLunariaColors.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                color = tokens.onSurfacePrimary,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = valueLabel,
                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                color = tokens.petalLavender,
            )
        }
        Slider(
            value = value.coerceIn(valueRange.start, valueRange.endInclusive),
            onValueChange = onValueChange,
            valueRange = valueRange,
            colors = SliderDefaults.colors(
                thumbColor = Color.White,
                activeTrackColor = tokens.petalLavender,
                inactiveTrackColor = tokens.onSurfaceSecondary.copy(alpha = 0.22f),
            ),
        )
    }
}

@Composable
private fun SettingsActionRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    accentTitle: Color = LocalLunariaColors.current.onSurfacePrimary,
) {
    val tokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                color = accentTitle,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = tokens.onSurfaceSecondary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Icon(
            imageVector = SonataIcons.ChevronRight,
            contentDescription = null,
            tint = tokens.onSurfaceSecondary.copy(alpha = 0.7f),
            modifier = Modifier.size(18.dp),
        )
    }
}
