/*
 * Dedicated Sources / Kaynaklar hub for Sonata Android Settings.
 *
 * Groups Personal Only Mode, streaming Modules (JioSaavn lossless/320k + YTM playback
 * attribution), Local Music scanning/permission controls, and Self-Hosted Servers
 * (Jellyfin, Subsonic, SMB, WebDAV) with live reachability health badges and
 * real throughput Speed Test (Mbps) controls.
 */
package dev.sonata.android.ui.screens

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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.SourcesStrings
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidServerHealthReport
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSpeedTestReport

/**
 * Per-server live health check and download throughput benchmark state.
 */
@Immutable
data class ServerDiagnosticState(
    val checkingHealth: Boolean = false,
    val healthReport: AndroidServerHealthReport? = null,
    val healthError: String? = null,
    val runningSpeedTest: Boolean = false,
    val speedTestReport: AndroidServerSpeedTestReport? = null,
    val speedTestError: String? = null,
)

private val SourcesGroupShape = RoundedCornerShape(14.dp)
private val SourcesSwitchGreen = Color(0xFF34C759)

/**
 * Standalone full-screen Sources / Kaynaklar view wrapping [SourcesHubContent].
 */
@Composable
fun SourcesScreen(
    settings: SonataSettingsState,
    signedIn: Boolean,
    localTrackCount: Int,
    localPermissionGranted: Boolean,
    servers: List<AndroidServerProfile>,
    serverDiagnostics: Map<String, ServerDiagnosticState>,
    onTogglePersonalOnlyMode: (Boolean) -> Unit,
    onToggleJioSaavn: (Boolean) -> Unit,
    onToggleAttributePersonalToYtm: (Boolean) -> Unit,
    onToggleLocalMusic: (Boolean) -> Unit,
    onRequestLocalPermission: () -> Unit,
    onRescanLocalMusic: () -> Unit,
    onProbeServerHealth: (String) -> Unit,
    onRunServerSpeedTest: (String) -> Unit,
    onOpenServers: (String?) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val statusTop = rememberSafeTopInset()
    val src = remember(settings.appLanguage) { sourcesStrings(settings.appLanguage) }
    val pageBg = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val cardBg = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val dividerColor = if (tokens.isDark) BasicPalette.DarkOutline else BasicPalette.LightOutline
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant

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
                        .clickable(onClick = onBack),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.ChevronLeft,
                        contentDescription = null,
                        tint = primaryText,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = src.sourcesTitle,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                        color = primaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = src.sourcesSubtitle,
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp)
                    .padding(top = 4.dp, bottom = 32.dp),
            ) {
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
                    onTogglePersonalOnlyMode = onTogglePersonalOnlyMode,
                    onToggleJioSaavn = onToggleJioSaavn,
                    onToggleAttributePersonalToYtm = onToggleAttributePersonalToYtm,
                    onToggleLocalMusic = onToggleLocalMusic,
                    onRequestLocalPermission = onRequestLocalPermission,
                    onRescanLocalMusic = onRescanLocalMusic,
                    onProbeServerHealth = onProbeServerHealth,
                    onRunServerSpeedTest = onRunServerSpeedTest,
                    onOpenServers = onOpenServers,
                )
            }
        }
    }
}

/**
 * Reusable inset-grouped Sources / Kaynaklar content embedded inside Settings or [SourcesScreen].
 */
@Composable
fun SourcesHubContent(
    settings: SonataSettingsState,
    signedIn: Boolean,
    localTrackCount: Int,
    localPermissionGranted: Boolean,
    servers: List<AndroidServerProfile>,
    serverDiagnostics: Map<String, ServerDiagnosticState>,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onTogglePersonalOnlyMode: (Boolean) -> Unit,
    onToggleJioSaavn: (Boolean) -> Unit,
    onToggleAttributePersonalToYtm: (Boolean) -> Unit,
    onToggleLocalMusic: (Boolean) -> Unit,
    onRequestLocalPermission: () -> Unit,
    onRescanLocalMusic: () -> Unit,
    onProbeServerHealth: (String) -> Unit,
    onRunServerSpeedTest: (String) -> Unit,
    onOpenServers: (String?) -> Unit,
    searchQuery: String = "",
) {
    val src = remember(settings.appLanguage) { sourcesStrings(settings.appLanguage) }
    val terms = remember(searchQuery) {
        searchQuery.trim().lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }
    }
    fun matches(vararg haystacks: String): Boolean {
        if (terms.isEmpty()) return true
        val combined = haystacks.joinToString(" ").lowercase()
        return terms.all { it in combined }
    }

    // 1. Personal Only Mode
    if (matches("Personal", "Kişisel", "Личное", "Only", "Offline", "YouTube", "JioSaavn", "Spotify")) {
        SourcesSectionGroup(
            header = src.personalOnlyModeTitle,
            footer = src.personalOnlyModeDescription,
            cardBg = cardBg,
            dividerColor = dividerColor,
            secondaryText = secondaryText,
        ) {
            SourcesSwitchRow(
                title = src.personalOnlyModeTitle,
                subtitle = if (settings.personalOnlyMode) src.personalOnlyModeBadge else src.sourcesSubtitle,
                checked = settings.personalOnlyMode,
                enabled = true,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = onTogglePersonalOnlyMode,
            )
        }
        Spacer(Modifier.height(14.dp))
    }

    // 2. Modules (JioSaavn 320k/lossless + Report personal plays to YouTube Music)
    if (matches("Modules", "Modüller", "Модули", "JioSaavn", "320", "Lossless", "Report", "History", "YouTube", "Attribution")) {
        SourcesSectionGroup(
            header = src.modulesTitle,
            footer = src.modulesSubtitle,
            cardBg = cardBg,
            dividerColor = dividerColor,
            secondaryText = secondaryText,
        ) {
            val effectiveJio = settings.effectiveJioSaavnEnabled()
            SourcesSwitchRow(
                title = src.jioSaavnModuleTitle,
                subtitle = if (settings.personalOnlyMode) {
                    src.jioSaavnPausedByPersonalMode
                } else {
                    src.jioSaavnModuleDescription
                },
                badge = "320k",
                checked = effectiveJio,
                enabled = !settings.personalOnlyMode,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = onToggleJioSaavn,
            )
            HorizontalDivider(
                thickness = 0.5.dp,
                color = dividerColor,
                modifier = Modifier.padding(start = 16.dp),
            )
            SourcesSwitchRow(
                title = src.attributePersonalToYtmTitle,
                subtitle = src.attributePersonalToYtmDescription,
                checked = settings.attributePersonalToYtm,
                enabled = !settings.personalOnlyMode && signedIn,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = onToggleAttributePersonalToYtm,
            )
        }
        Spacer(Modifier.height(14.dp))
    }

    // 3. Local Music
    if (matches("Local", "Yerel", "Локальная", "Device", "Permission", "Audio", "FLAC", "Scan")) {
        SourcesSectionGroup(
            header = src.localMusicTitle,
            footer = src.localMusicDescription,
            cardBg = cardBg,
            dividerColor = dividerColor,
            secondaryText = secondaryText,
        ) {
            SourcesSwitchRow(
                title = src.localMusicTitle,
                subtitle = if (settings.localMusicEnabled && localPermissionGranted) {
                    src.localTracksCount(localTrackCount)
                } else {
                    src.localMusicDescription
                },
                checked = settings.localMusicEnabled,
                enabled = true,
                primaryText = primaryText,
                secondaryText = secondaryText,
                onCheckedChange = onToggleLocalMusic,
            )
            if (settings.localMusicEnabled) {
                HorizontalDivider(
                    thickness = 0.5.dp,
                    color = dividerColor,
                    modifier = Modifier.padding(start = 16.dp),
                )
                if (!localPermissionGranted) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = src.localMusicPermissionTitle,
                                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                                color = primaryText,
                            )
                            Text(
                                text = src.localMusicPermissionSubtitle,
                                style = MaterialTheme.typography.bodySmall,
                                color = secondaryText,
                            )
                        }
                        Spacer(Modifier.width(12.dp))
                        SourcesPillButton(
                            label = src.grantPermission,
                            containerColor = primaryText,
                            contentColor = if (primaryText == Color.White) Color.Black else Color.White,
                            onClick = onRequestLocalPermission,
                        )
                    }
                } else {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(onClick = onRescanLocalMusic)
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Folder,
                            contentDescription = null,
                            tint = primaryText,
                            modifier = Modifier.size(20.dp),
                        )
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = src.rescanLocalMusic,
                                style = MaterialTheme.typography.bodyLarge,
                                color = primaryText,
                            )
                            Text(
                                text = if (localTrackCount > 0) {
                                    src.localTracksCount(localTrackCount)
                                } else {
                                    src.emptyLocalMusic
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = secondaryText,
                            )
                        }
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
        Spacer(Modifier.height(14.dp))
    }

    // 4. Self-Hosted Servers (Jellyfin, Subsonic, SMB, WebDAV) with Alive + Latency + Speed Test
    if (matches("Server", "Servers", "Sunucu", "Сервер", "Jellyfin", "Subsonic", "SMB", "WebDAV", "Speed", "Hız", "Alive", "Health")) {
        SourcesSectionGroup(
            header = src.selfHostedServersTitle,
            footer = src.selfHostedServersDescription,
            cardBg = cardBg,
            dividerColor = dividerColor,
            secondaryText = secondaryText,
        ) {
            servers.forEachIndexed { idx, srv ->
                if (idx > 0) {
                    HorizontalDivider(
                        thickness = 0.5.dp,
                        color = dividerColor,
                        modifier = Modifier.padding(start = 16.dp),
                    )
                }
                SourceServerDiagnosticRow(
                    server = srv,
                    diagnostic = serverDiagnostics[srv.id] ?: ServerDiagnosticState(),
                    src = src,
                    dividerColor = dividerColor,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    onOpenServer = { onOpenServers(srv.id) },
                    onCheckHealth = { onProbeServerHealth(srv.id) },
                    onRunSpeedTest = { onRunServerSpeedTest(srv.id) },
                )
            }
            if (servers.isNotEmpty()) {
                HorizontalDivider(
                    thickness = 0.5.dp,
                    color = dividerColor,
                    modifier = Modifier.padding(start = 16.dp),
                )
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpenServers(null) }
                    .padding(horizontal = 16.dp, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = SonataIcons.Add,
                    contentDescription = null,
                    tint = primaryText,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = src.selfHostedServersTitle,
                        style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                        color = primaryText,
                    )
                    Text(
                        text = "${src.protocolJellyfin} · ${src.protocolSubsonic} · ${src.protocolSmb} · ${src.protocolWebDav}",
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
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
private fun SourceServerDiagnosticRow(
    server: AndroidServerProfile,
    diagnostic: ServerDiagnosticState,
    src: SourcesStrings,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onOpenServer: () -> Unit,
    onCheckHealth: () -> Unit,
    onRunSpeedTest: () -> Unit,
) {
    val kindTag = when (server.kind) {
        AndroidServerKind.JELLYFIN -> "JFN"
        AndroidServerKind.SUBSONIC -> "SUB"
        AndroidServerKind.WEB_DAV -> "DAV"
        AndroidServerKind.SMB -> "SMB"
    }
    val endpointLabel = server.baseUrl.takeIf { it.isNotBlank() }
        ?: server.localMountPath?.takeIf { it.isNotBlank() }
        ?: server.share.orEmpty()

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpenServer)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .background(primaryText.copy(alpha = 0.09f))
                    .padding(horizontal = 7.dp, vertical = 3.dp),
            ) {
                Text(
                    text = kindTag,
                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                    color = primaryText,
                )
            }
            Column(Modifier.weight(1f)) {
                Text(
                    text = server.name,
                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                    color = primaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (endpointLabel.isNotBlank()) {
                    Text(
                        text = endpointLabel,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            // Alive / Reachable + latency badge
            val health = diagnostic.healthReport
            when {
                diagnostic.checkingHealth -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(12.dp),
                            strokeWidth = 1.5.dp,
                            color = secondaryText,
                        )
                        Text(
                            text = src.statusChecking,
                            style = MaterialTheme.typography.labelSmall,
                            color = secondaryText,
                        )
                    }
                }
                health != null && health.alive -> {
                    val latText = health.latencyMs?.let { " · ${src.latencyMs(it.toLong())}" }.orEmpty()
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(SourcesSwitchGreen.copy(alpha = 0.16f))
                            .padding(horizontal = 8.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = "${src.statusAlive}$latText",
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                            color = SourcesSwitchGreen,
                        )
                    }
                }
                health != null && !health.alive || diagnostic.healthError != null -> {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(BasicPalette.AccentRed.copy(alpha = 0.14f))
                            .padding(horizontal = 8.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = src.statusOffline,
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                            color = BasicPalette.AccentRed,
                        )
                    }
                }
                else -> {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(dividerColor.copy(alpha = 0.45f))
                            .padding(horizontal = 8.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = if (server.enabled) src.statusOnline else src.statusOffline,
                            style = MaterialTheme.typography.labelSmall,
                            color = secondaryText,
                        )
                    }
                }
            }
        }

        // Measured Speed Test summary if present
        val speedReport = diagnostic.speedTestReport
        val speedErr = diagnostic.speedTestError
        if (speedReport != null) {
            val summary = if (speedReport.alive) {
                src.speedTestResult(speedReport.latencyMs.toLong(), speedReport.downloadMbps)
            } else {
                src.statusOffline
            }
            Text(
                text = "$summary — ${speedReport.sampleNote}",
                style = MaterialTheme.typography.bodySmall,
                color = if (speedReport.alive && speedReport.downloadMbps != null) SourcesSwitchGreen else secondaryText,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        } else if (!speedErr.isNullOrBlank()) {
            Text(
                text = speedErr,
                style = MaterialTheme.typography.bodySmall,
                color = BasicPalette.AccentRed,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        } else if (!diagnostic.healthError.isNullOrBlank()) {
            Text(
                text = diagnostic.healthError,
                style = MaterialTheme.typography.bodySmall,
                color = BasicPalette.AccentRed,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SourcesPillButton(
                label = if (diagnostic.checkingHealth) src.statusChecking else src.checkHealth,
                containerColor = dividerColor.copy(alpha = 0.45f),
                contentColor = primaryText,
                enabled = !diagnostic.checkingHealth,
                onClick = onCheckHealth,
            )
            SourcesPillButton(
                label = if (diagnostic.runningSpeedTest) src.runningSpeedTest else src.speedTest,
                containerColor = primaryText.copy(alpha = 0.12f),
                contentColor = primaryText,
                enabled = !diagnostic.runningSpeedTest,
                onClick = onRunSpeedTest,
            )
        }
    }
}

@Composable
private fun SourcesPillButton(
    label: String,
    containerColor: Color,
    contentColor: Color,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .alpha(if (enabled) 1f else 0.55f)
            .background(containerColor)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 11.dp, vertical = 6.dp),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            color = contentColor,
            maxLines = 1,
        )
    }
}

@Composable
private fun SourcesSectionGroup(
    header: String,
    footer: String? = null,
    cardBg: Color,
    dividerColor: Color,
    secondaryText: Color,
    content: @Composable () -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = header.uppercase(),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.6.sp,
            ),
            color = secondaryText,
            modifier = Modifier.padding(start = 12.dp, end = 12.dp, bottom = 7.dp),
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(SourcesGroupShape)
                .background(cardBg)
                .border(0.5.dp, dividerColor, SourcesGroupShape),
        ) {
            content()
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

@Composable
private fun SourcesSwitchRow(
    title: String,
    subtitle: String? = null,
    badge: String? = null,
    checked: Boolean,
    enabled: Boolean,
    primaryText: Color,
    secondaryText: Color,
    onCheckedChange: (Boolean) -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    val trackColor by animateColorAsState(
        targetValue = if (checked) SourcesSwitchGreen else Color(0xFF787880).copy(alpha = 0.36f),
        animationSpec = tween(180),
        label = "sourcesSwitchTrack",
    )
    val thumbOffset by animateDpAsState(
        targetValue = if (checked) 22.dp else 2.dp,
        animationSpec = tween(180),
        label = "sourcesSwitchThumb",
    )
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .alpha(if (enabled) 1f else 0.45f)
            .clickable(enabled = enabled) {
                haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onCheckedChange(!checked)
            }
            .padding(horizontal = 16.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyLarge,
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
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        Box(
            modifier = Modifier
                .width(50.dp)
                .height(30.dp)
                .clip(CircleShape)
                .background(trackColor),
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
}
