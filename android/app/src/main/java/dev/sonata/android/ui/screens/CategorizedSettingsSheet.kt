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
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.isGlassSupported
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.SearchField
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.SonataSettingsTokens
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
        subtitle = "Quality, EQ, crossfade, speed, loudness",
        icon = SonataIcons.GraphicEq,
        accent = Color(0xFFE5E5EA),
        keywords = "playback audio quality lossless high medium low crossfade loudness normalization skip silence speed equalizer eq autoplay",
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
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    var activeCategory by remember { mutableStateOf<SettingsCategory?>(null) }
    var searchQuery by remember { mutableStateOf("") }

    BackHandler(enabled = activeCategory != null) {
        activeCategory = null
    }
    BackHandler(enabled = activeCategory == null && searchQuery.isNotEmpty()) {
        searchQuery = ""
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(SonataSettingsTokens.Background)
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
        // Top navigation bar with circular Back button & bold page title
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 10.dp, bottom = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(38.dp)
                    .clip(CircleShape)
                    .background(SonataSettingsTokens.Card)
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
                    tint = SonataSettingsTokens.TextPrimary,
                    modifier = Modifier.size(20.dp),
                )
            }
            Text(
                text = activeCategory?.title ?: "Settings",
                style = MaterialTheme.typography.headlineLarge.copy(
                    fontSize = 28.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = SonataSettingsTokens.TextPrimary,
                modifier = Modifier.weight(1f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        SettingsCategoryNavigator(
            activeCategory = activeCategory,
            onSelectCategory = { activeCategory = it },
            searchQuery = searchQuery,
            onSearchQueryChange = { searchQuery = it },
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
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var activeCategory by remember { mutableStateOf<SettingsCategory?>(null) }
    var searchQuery by remember { mutableStateOf("") }

    BackHandler(enabled = activeCategory != null) {
        activeCategory = null
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = SonataSettingsTokens.Background,
        contentColor = SonataSettingsTokens.TextPrimary,
        dragHandle = null,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(SonataSettingsTokens.Background)
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
                    text = activeCategory?.title ?: "Settings",
                    style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                    color = SonataSettingsTokens.TextPrimary,
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
                searchQuery = searchQuery,
                onSearchQueryChange = { searchQuery = it },
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
    searchQuery: String,
    onSearchQueryChange: (String) -> Unit,
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
                searchQuery = searchQuery,
                onSearchQueryChange = onSearchQueryChange,
                signedIn = signedIn,
                accountProfile = accountProfile,
                onSelectCategory = onSelectCategory,
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
    searchQuery: String,
    onSearchQueryChange: (String) -> Unit,
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    onSelectCategory: (SettingsCategory) -> Unit,
) {
    val filtered = remember(searchQuery) {
        if (searchQuery.isBlank()) {
            SettingsCategory.entries
        } else {
            SettingsCategory.entries.filter {
                it.title.contains(searchQuery, ignoreCase = true) ||
                    it.subtitle.contains(searchQuery, ignoreCase = true) ||
                    it.keywords.contains(searchQuery, ignoreCase = true)
            }
        }
    }

    LazyColumn(
        contentPadding = PaddingValues(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        item(key = "overview_search") {
            SearchField(
                query = searchQuery,
                onQueryChange = onSearchQueryChange,
                onSubmit = {},
                placeholder = "Search Settings",
            )
        }

        if (searchQuery.isBlank()) {
            item(key = "overview_profile_banner") {
                SettingsFlatCard(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onSelectCategory(SettingsCategory.ACCOUNT) },
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        val avatarRequest = rememberArtworkRequest(
                            url = if (signedIn) accountProfile?.avatarUrl else null,
                            widthPx = 128,
                            heightPx = 128,
                        )
                        Box(
                            modifier = Modifier
                                .size(44.dp)
                                .clip(CircleShape)
                                .background(Color.White.copy(alpha = 0.1f)),
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
                                Icon(SonataIcons.Person, null, tint = Color.White)
                            }
                        }
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = if (signedIn) accountProfile?.name ?: "YouTube Music" else "Not signed in",
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = SonataSettingsTokens.TextPrimary,
                            )
                            Text(
                                text = if (signedIn) {
                                    accountProfile?.email ?: accountProfile?.handle ?: "Synced account"
                                } else {
                                    "Tap to connect YouTube Music"
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = SonataSettingsTokens.TextSecondary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = SonataSettingsTokens.TextSecondary,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }

        item(key = "overview_categories") {
            SettingsFlatCard(modifier = Modifier.fillMaxWidth()) {
                filtered.forEachIndexed { idx, cat ->
                    if (idx > 0) SettingsDivider()
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelectCategory(cat) }
                            .padding(horizontal = 16.dp, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(34.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .background(Color(0xFF222226)),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = cat.icon,
                                contentDescription = null,
                                tint = Color.White.copy(alpha = 0.88f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = cat.title,
                                style = MaterialTheme.typography.titleSmall.copy(
                                    fontSize = 15.sp,
                                    fontWeight = FontWeight.SemiBold,
                                ),
                                color = Color.White,
                            )
                            Text(
                                text = cat.subtitle,
                                style = MaterialTheme.typography.bodySmall,
                                color = Color.White.copy(alpha = 0.58f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.4f),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
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
                        color = Color.White,
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
                    title = "Loudness Normalization",
                    subtitle = "Balance track volume across releases",
                    checked = settings.loudnessNormEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(loudnessNormEnabled = v) } },
                )
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

        item(key = "pb_eq") {
            SettingsSection("EQUALIZER") {
                SettingsToggleRow(
                    title = "Built-in Equalizer",
                    subtitle = "Active preset: ${settings.eqPreset}",
                    checked = settings.eqEnabled,
                    onCheckedChange = { v -> onUpdate { it.copy(eqEnabled = v) } },
                )
                if (settings.eqEnabled) {
                    SettingsDivider()
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            text = "Preset",
                            style = MaterialTheme.typography.labelMedium,
                            color = Color.White.copy(alpha = 0.7f),
                        )
                        val rows = EqPresets.chunked(3)
                        rows.forEach { rowPresets ->
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                rowPresets.forEach { preset ->
                                    val selected = settings.eqPreset.equals(preset, ignoreCase = true)
                                    Box(
                                        modifier = Modifier
                                            .weight(1f)
                                            .clip(RoundedCornerShape(12.dp))
                                            .background(if (selected) SonataAccent else Color.White.copy(alpha = 0.08f))
                                            .clickable { onUpdate { it.copy(eqPreset = preset) } }
                                            .padding(vertical = 8.dp),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        Text(
                                            text = preset,
                                            style = MaterialTheme.typography.labelSmall.copy(
                                                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                            ),
                                            color = Color.White,
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
            SettingsSection("THEME & BACKDROP") {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Theme Mode",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = Color.White,
                    )
                    PillSegmentedControl(
                        options = listOf("System", "Dark", "Light", "AMOLED"),
                        selectedIndex = ThemeMode.entries.indexOf(settings.themeMode),
                        onSelect = { idx -> onUpdate { it.copy(themeMode = ThemeMode.entries[idx]) } },
                    )
                }
                SettingsDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Now Playing Backdrop",
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = Color.White,
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
                        color = Color.White,
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
                        color = Color.White,
                    )
                    TranslationLangs.chunked(3).forEach { row ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            row.forEach { (code, label) ->
                                val selected = settings.lyricsTranslationLang.equals(code, ignoreCase = true)
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .clip(RoundedCornerShape(12.dp))
                                        .background(if (selected) SonataAccent else Color.White.copy(alpha = 0.08f))
                                        .clickable { onUpdate { it.copy(lyricsTranslationLang = code) } }
                                        .padding(vertical = 8.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = label,
                                        style = MaterialTheme.typography.labelSmall.copy(
                                            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                        ),
                                        color = Color.White,
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
                        color = Color.White,
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
            SettingsSection("YOUTUBE MUSIC SESSION") {
                SettingsActionRow(
                    title = if (signedIn) accountProfile?.name ?: "Signed In" else "Connect YouTube Music",
                    subtitle = if (signedIn) {
                        accountProfile?.email ?: accountProfile?.handle ?: "Tap to sign out"
                    } else {
                        "Sign in for personalized mixes and playlists"
                    },
                    accentTitle = SonataAccent,
                    onClick = { if (signedIn) onSignOut() else onSignIn() },
                )
            }
        }

        item(key = "ac_updates") {
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
                            color = Color.White,
                        )
                        Text(
                            text = statusLabel,
                            style = MaterialTheme.typography.bodySmall,
                            color = if (updateInfo?.updateAvailable == true) SonataAccent else Color.White.copy(alpha = 0.6f),
                        )
                    }
                    if (checkingUpdates) {
                        CircularProgressIndicator(
                            color = Color.White,
                            strokeWidth = 2.dp,
                            modifier = Modifier.size(18.dp),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.45f),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                if (updateInfo?.updateAvailable == true) {
                    SettingsDivider()
                    SettingsActionRow(
                        title = "Download v${updateInfo.latestVersion}",
                        subtitle = "Fetch and install latest release APK",
                        accentTitle = SonataAccent,
                        onClick = onDownloadUpdate,
                    )
                }
            }
        }
    }
}

@Composable
private fun SettingsFlatCard(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier = modifier
            .clip(shape)
            .background(SonataSettingsTokens.Card, shape),
        content = content,
    )
}

@Composable
private fun SettingsSection(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelSmall.copy(
                letterSpacing = 0.9.sp,
                fontWeight = FontWeight.Bold,
            ),
            color = SonataSettingsTokens.TextSecondary,
            modifier = Modifier.padding(horizontal = 6.dp),
        )
        SettingsFlatCard(modifier = Modifier.fillMaxWidth(), content = content)
    }
}

@Composable
private fun SettingsDivider() {
    HorizontalDivider(
        color = SonataSettingsTokens.Hairline,
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
                color = if (enabled) SonataSettingsTokens.TextPrimary else Color.White.copy(alpha = 0.4f),
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = if (enabled) SonataSettingsTokens.TextSecondary else Color.White.copy(alpha = 0.32f),
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
                checkedTrackColor = SonataSettingsTokens.ToggleActive,
                uncheckedThumbColor = Color.White,
                uncheckedTrackColor = SonataSettingsTokens.ToggleInactive,
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
                color = Color.White,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = valueLabel,
                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                color = SonataAccent,
            )
        }
        Slider(
            value = value.coerceIn(valueRange.start, valueRange.endInclusive),
            onValueChange = onValueChange,
            valueRange = valueRange,
            colors = SliderDefaults.colors(
                thumbColor = Color.White,
                activeTrackColor = SonataAccent,
                inactiveTrackColor = Color.White.copy(alpha = 0.16f),
            ),
        )
    }
}

@Composable
private fun SettingsActionRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    accentTitle: Color = Color.White,
) {
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
                color = Color.White.copy(alpha = 0.58f),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Icon(
            imageVector = SonataIcons.ChevronRight,
            contentDescription = null,
            tint = Color.White.copy(alpha = 0.45f),
            modifier = Modifier.size(18.dp),
        )
    }
}
