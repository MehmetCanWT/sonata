/*
 * Self-hosted music servers management and browser screen (Jellyfin, Subsonic / OpenSubsonic, WebDAV).
 */
package dev.sonata.android.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil3.compose.AsyncImage
import dev.sonata.android.ServerBrowserState
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.ui.CARD_ART_PX
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.PillSegmentedControl
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SearchField
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.ServerStrings
import dev.sonata.android.ui.i18n.SourcesStrings
import dev.sonata.android.ui.i18n.sourcesStrings
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidTrack

/**
 * Full-screen self-hosted music server manager and media browser supporting Jellyfin,
 * Subsonic / OpenSubsonic, SMB, and WebDAV servers.
 */
@Composable
fun ServersScreen(
    servers: List<AndroidServerProfile>,
    browserState: ServerBrowserState,
    activeTrackId: String?,
    isPlaying: Boolean,
    onBack: () -> Unit,
    onOpenServer: (AndroidServerProfile) -> Unit,
    onCloseServerBrowser: () -> Unit,
    onSelectLibrary: (String?) -> Unit,
    onOpenArtist: (AndroidHomeCard) -> Unit,
    onCloseArtist: () -> Unit,
    onOpenAlbum: (AndroidHomeCard) -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onOpenWebDavFolder: (String?, String) -> Unit,
    onNavigateWebDavBack: () -> Boolean,
    onSearchInServer: (String) -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
    onSaveServer: (AndroidServerProfile, () -> Unit) -> Unit,
    onDeleteServer: (String) -> Unit,
    onSetServerEnabled: (String, Boolean) -> Unit,
    onTestConnection: (AndroidServerProfile) -> Unit,
    onClearTestResult: () -> Unit,
    modifier: Modifier = Modifier,
    appLanguage: AppLanguage = AppLanguage.SYSTEM,
    serverDiagnostics: Map<String, ServerDiagnosticState> = emptyMap(),
    onProbeServerHealth: (String) -> Unit = {},
    onRunServerSpeedTest: (String) -> Unit = {},
) {
    val strings = LocalSonataStrings.current
    val s = strings.servers
    val src = remember(appLanguage) { sourcesStrings(appLanguage) }
    val tokens = LocalLunariaColors.current
    val statusTop = rememberSafeTopInset()
    val accent = sonataThemeAccent()

    var editingTarget by remember { mutableStateOf<AndroidServerProfile?>(null) }
    var showAddDialog by remember { mutableStateOf(false) }
    var pendingDeleteServer by remember { mutableStateOf<AndroidServerProfile?>(null) }

    val pageBg = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val cardBg = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant
    val dividerColor = if (tokens.isDark) BasicPalette.DarkOutline else BasicPalette.LightOutline

    val activeServer = browserState.activeServer

    val handleBack: () -> Unit = {
        when {
            activeServer != null && activeServer.kind == AndroidServerKind.WEB_DAV && onNavigateWebDavBack() -> Unit
            activeServer != null && browserState.selectedArtistName != null -> onCloseArtist()
            activeServer != null -> onCloseServerBrowser()
            else -> onBack()
        }
    }

    BackHandler(onBack = handleBack)

    pendingDeleteServer?.let { target ->
        AlertDialog(
            onDismissRequest = { pendingDeleteServer = null },
            containerColor = cardBg,
            titleContentColor = primaryText,
            textContentColor = secondaryText,
            title = { Text(s.deleteServerConfirmTitle) },
            text = { Text(s.deleteServerConfirmBody(target.name)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        pendingDeleteServer = null
                        onDeleteServer(target.id)
                    },
                ) {
                    Text(strings.remove, color = BasicPalette.AccentRed, fontWeight = FontWeight.SemiBold)
                }
            },
            dismissButton = {
                TextButton(onClick = { pendingDeleteServer = null }) {
                    Text(strings.cancel, color = primaryText)
                }
            },
        )
    }

    if (showAddDialog || editingTarget != null) {
        ServerEditorDialog(
            initialProfile = editingTarget,
            src = src,
            testingConnection = browserState.testingConnection,
            testResultMessage = browserState.testResultMessage,
            testResultIsError = browserState.testResultIsError,
            saving = browserState.saving,
            saveError = browserState.error,
            cardBg = cardBg,
            dividerColor = dividerColor,
            primaryText = primaryText,
            secondaryText = secondaryText,
            accent = accent,
            onTestConnection = onTestConnection,
            onSave = { profile ->
                onSaveServer(profile) {
                    showAddDialog = false
                    editingTarget = null
                    onClearTestResult()
                }
            },
            onDismiss = {
                showAddDialog = false
                editingTarget = null
                onClearTestResult()
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
                .widthIn(max = 720.dp)
                .fillMaxSize(),
        ) {
            // Top bar
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
                        .clickable(onClick = handleBack),
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
                Column(Modifier.weight(1f)) {
                    Text(
                        text = activeServer?.name ?: s.selfHostedServers,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                        color = primaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (activeServer != null) {
                        Text(
                            text = serverKindLabel(activeServer.kind, s, src),
                            style = MaterialTheme.typography.labelSmall,
                            color = secondaryText,
                            maxLines = 1,
                        )
                    }
                }
                if (activeServer != null) {
                    Box(
                        modifier = Modifier
                            .size(40.dp)
                            .clip(CircleShape)
                            .background(cardBg)
                            .clickable {
                                onClearTestResult()
                                editingTarget = activeServer
                            },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Edit,
                            contentDescription = s.editServer,
                            tint = primaryText,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                } else {
                    Box(
                        modifier = Modifier
                            .size(40.dp)
                            .clip(CircleShape)
                            .background(accent)
                            .clickable {
                                onClearTestResult()
                                editingTarget = null
                                showAddDialog = true
                            },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Add,
                            contentDescription = s.addServer,
                            tint = Color.White,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
            }

            if (activeServer == null) {
                ServersListContent(
                    servers = servers,
                    serverDiagnostics = serverDiagnostics,
                    s = s,
                    src = src,
                    cardBg = cardBg,
                    dividerColor = dividerColor,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    accent = accent,
                    onAddServer = {
                        onClearTestResult()
                        editingTarget = null
                        showAddDialog = true
                    },
                    onOpenServer = onOpenServer,
                    onEditServer = { srv ->
                        onClearTestResult()
                        editingTarget = srv
                    },
                    onDeleteServer = { srv -> pendingDeleteServer = srv },
                    onToggleEnabled = onSetServerEnabled,
                    onProbeHealth = onProbeServerHealth,
                    onRunSpeedTest = onRunServerSpeedTest,
                )
            } else {
                ServerBrowserContent(
                    server = activeServer,
                    browserState = browserState,
                    activeTrackId = activeTrackId,
                    isPlaying = isPlaying,
                    cardBg = cardBg,
                    dividerColor = dividerColor,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    accent = accent,
                    onSelectLibrary = onSelectLibrary,
                    onOpenArtist = onOpenArtist,
                    onCloseArtist = onCloseArtist,
                    onOpenAlbum = onOpenAlbum,
                    onOpenPlaylist = onOpenPlaylist,
                    onOpenWebDavFolder = onOpenWebDavFolder,
                    onSearchInServer = onSearchInServer,
                    onPlayTrack = onPlayTrack,
                )
            }
        }
    }
}

@Composable
private fun ServersListContent(
    servers: List<AndroidServerProfile>,
    serverDiagnostics: Map<String, ServerDiagnosticState>,
    s: ServerStrings,
    src: SourcesStrings,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accent: Color,
    onAddServer: () -> Unit,
    onOpenServer: (AndroidServerProfile) -> Unit,
    onEditServer: (AndroidServerProfile) -> Unit,
    onDeleteServer: (AndroidServerProfile) -> Unit,
    onToggleEnabled: (String, Boolean) -> Unit,
    onProbeHealth: (String) -> Unit,
    onRunSpeedTest: (String) -> Unit,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (servers.isEmpty()) {
            item(key = "servers_empty") {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(18.dp))
                        .background(cardBg)
                        .border(0.75.dp, dividerColor, RoundedCornerShape(18.dp))
                        .padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.Folder,
                        contentDescription = null,
                        tint = accent,
                        modifier = Modifier.size(36.dp),
                    )
                    Text(
                        text = s.noServersTitle,
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                        color = primaryText,
                    )
                    Text(
                        text = s.noServersSubtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                    Spacer(Modifier.height(4.dp))
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(22.dp))
                            .background(accent)
                            .clickable(onClick = onAddServer)
                            .padding(horizontal = 20.dp, vertical = 10.dp),
                    ) {
                        Text(
                            text = s.addServer,
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = Color.White,
                        )
                    }
                }
            }
        } else {
            items(servers, key = { it.id }) { server ->
                ServerProfileCard(
                    server = server,
                    diagnostic = serverDiagnostics[server.id] ?: ServerDiagnosticState(),
                    s = s,
                    src = src,
                    cardBg = cardBg,
                    dividerColor = dividerColor,
                    primaryText = primaryText,
                    secondaryText = secondaryText,
                    accent = accent,
                    onClick = { onOpenServer(server) },
                    onEdit = { onEditServer(server) },
                    onDelete = { onDeleteServer(server) },
                    onToggleEnabled = { enabled -> onToggleEnabled(server.id, enabled) },
                    onProbeHealth = { onProbeHealth(server.id) },
                    onRunSpeedTest = { onRunSpeedTest(server.id) },
                )
            }
        }

        item(key = "servers_footer") {
            Text(
                text = s.footerServers,
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
                modifier = Modifier.padding(horizontal = 4.dp, vertical = 6.dp),
            )
        }
    }
}

@Composable
private fun ServerProfileCard(
    server: AndroidServerProfile,
    diagnostic: ServerDiagnosticState,
    s: ServerStrings,
    src: SourcesStrings,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accent: Color,
    onClick: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onToggleEnabled: (Boolean) -> Unit,
    onProbeHealth: () -> Unit,
    onRunSpeedTest: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    val endpointSummary = remember(server) {
        server.baseUrl.takeIf { it.isNotBlank() }
            ?: server.localMountPath?.takeIf { it.isNotBlank() }
            ?: server.share.orEmpty()
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(cardBg)
            .border(0.75.dp, dividerColor, RoundedCornerShape(16.dp))
            .clickable(onClick = onClick)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(accent.copy(alpha = 0.16f))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            ) {
                Text(
                    text = serverKindShortTag(server.kind),
                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                    color = accent,
                )
            }
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    text = server.name,
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = primaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = endpointSummary,
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Switch(
                checked = server.enabled,
                onCheckedChange = onToggleEnabled,
                colors = SwitchDefaults.colors(
                    checkedThumbColor = Color.White,
                    checkedTrackColor = Color(0xFF34C759),
                ),
            )
        }

        // Reachability badge + measured Speed Test output
        val health = diagnostic.healthReport
        val speed = diagnostic.speedTestReport
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when {
                diagnostic.checkingHealth -> {
                    Text(
                        text = src.statusChecking,
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                    )
                }
                health != null && health.alive -> {
                    val lat = health.latencyMs?.let { " · ${src.latencyMs(it.toLong())}" }.orEmpty()
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(Color(0xFF34C759).copy(alpha = 0.16f))
                            .padding(horizontal = 8.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = "${src.statusAlive}$lat",
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                            color = Color(0xFF34C759),
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
                    Text(
                        text = if (server.enabled) s.serverStatusConnected else s.serverStatusDisabled,
                        style = MaterialTheme.typography.labelSmall,
                        color = if (server.enabled) Color(0xFF34C759) else secondaryText,
                    )
                }
            }
            if (speed != null && speed.alive) {
                Text(
                    text = src.speedTestResult(speed.latencyMs.toLong(), speed.downloadMbps),
                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                    color = accent,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.weight(1f))
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(dividerColor.copy(alpha = 0.35f))
                    .clickable(enabled = !diagnostic.checkingHealth, onClick = onProbeHealth)
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            ) {
                Text(
                    text = if (diagnostic.checkingHealth) src.statusChecking else src.checkHealth,
                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Medium),
                    color = primaryText,
                )
            }
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(accent.copy(alpha = 0.16f))
                    .clickable(enabled = !diagnostic.runningSpeedTest, onClick = onRunSpeedTest)
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            ) {
                Text(
                    text = if (diagnostic.runningSpeedTest) src.runningSpeedTest else src.speedTest,
                    style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                    color = accent,
                )
            }
            Spacer(Modifier.weight(1f))
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(dividerColor.copy(alpha = 0.4f))
                    .clickable(onClick = onEdit)
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            ) {
                Text(
                    text = s.editServer,
                    style = MaterialTheme.typography.labelSmall,
                    color = primaryText,
                )
            }
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(BasicPalette.AccentRed.copy(alpha = 0.14f))
                    .clickable(onClick = onDelete)
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            ) {
                Text(
                    text = strings.remove,
                    style = MaterialTheme.typography.labelSmall,
                    color = BasicPalette.AccentRed,
                )
            }
        }
    }
}

@Composable
private fun ServerBrowserContent(
    server: AndroidServerProfile,
    browserState: ServerBrowserState,
    activeTrackId: String?,
    isPlaying: Boolean,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accent: Color,
    onSelectLibrary: (String?) -> Unit,
    onOpenArtist: (AndroidHomeCard) -> Unit,
    onCloseArtist: () -> Unit,
    onOpenAlbum: (AndroidHomeCard) -> Unit,
    onOpenPlaylist: (AndroidPlaylist) -> Unit,
    onOpenWebDavFolder: (String?, String) -> Unit,
    onSearchInServer: (String) -> Unit,
    onPlayTrack: (AndroidTrack, List<AndroidTrack>) -> Unit,
) {
    val strings = LocalSonataStrings.current
    val s = strings.servers
    var selectedSegment by rememberSaveable(server.id) { mutableIntStateOf(0) }
    val segments = remember(s) { listOf(s.albumsTab, s.playlistsTab, s.artistsTab) }

    Column(modifier = Modifier.fillMaxSize()) {
        SearchField(
            query = browserState.serverSearchQuery,
            onQueryChange = onSearchInServer,
            onSubmit = { onSearchInServer(browserState.serverSearchQuery) },
            placeholder = s.searchServerPlaceholder,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = PAGE_GUTTER, vertical = 6.dp),
        )

        if (!browserState.error.isNullOrBlank()) {
            Text(
                text = s.serverSearchError(browserState.error),
                style = MaterialTheme.typography.bodySmall,
                color = BasicPalette.AccentRed,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 4.dp),
            )
        }

        if (browserState.serverSearchQuery.isNotBlank()) {
            val res = browserState.serverSearchResult
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (browserState.loading) {
                    item(key = "srv_search_loading") {
                        Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator(color = accent, strokeWidth = 2.5.dp)
                        }
                    }
                }
                val tracks = res?.tracks.orEmpty()
                val albums = res?.albums.orEmpty()
                val playlists = res?.playlists.orEmpty()
                if (!browserState.loading && tracks.isEmpty() && albums.isEmpty() && playlists.isEmpty()) {
                    item(key = "srv_search_empty") {
                        Text(
                            text = s.emptyServerSection,
                            style = MaterialTheme.typography.bodyMedium,
                            color = secondaryText,
                            modifier = Modifier.padding(vertical = 24.dp),
                        )
                    }
                }
                if (tracks.isNotEmpty()) {
                    item(key = "srv_search_tracks_hdr") {
                        Text(
                            text = s.serverTracksCount(tracks.size),
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                            color = primaryText,
                            modifier = Modifier.padding(vertical = 4.dp),
                        )
                    }
                    items(tracks, key = { "srv_trk_${it.id}" }) { track ->
                        ServerTrackRow(
                            track = track,
                            isCurrent = activeTrackId == track.id,
                            isPlaying = activeTrackId == track.id && isPlaying,
                            primaryText = primaryText,
                            secondaryText = secondaryText,
                            accent = accent,
                            onClick = { onPlayTrack(track, tracks) },
                        )
                    }
                }
                if (albums.isNotEmpty()) {
                    item(key = "srv_search_albums_hdr") {
                        Text(
                            text = s.albumsTab,
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                            color = primaryText,
                            modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                        )
                    }
                    items(albums, key = { "srv_alb_${it.id}" }) { album ->
                        ServerCardRow(
                            card = album,
                            cardBg = cardBg,
                            primaryText = primaryText,
                            secondaryText = secondaryText,
                            onClick = { onOpenAlbum(album) },
                        )
                    }
                }
                if (playlists.isNotEmpty()) {
                    item(key = "srv_search_pl_hdr") {
                        Text(
                            text = s.playlistsTab,
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                            color = primaryText,
                            modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                        )
                    }
                    items(playlists, key = { "srv_pl_${it.id}" }) { card ->
                        ServerCardRow(
                            card = card,
                            cardBg = cardBg,
                            primaryText = primaryText,
                            secondaryText = secondaryText,
                            onClick = {
                                onOpenPlaylist(
                                    AndroidPlaylist(
                                        id = card.id,
                                        name = card.title,
                                        thumbnailUrl = card.thumbnailUrl,
                                        isOwned = false,
                                    ),
                                )
                            },
                        )
                    }
                }
            }
            return
        }

        if (server.kind == AndroidServerKind.WEB_DAV) {
            val breadcrumb = browserState.webdavPathStack.joinToString(" / ") { it.second }
            if (breadcrumb.isNotBlank()) {
                Text(
                    text = breadcrumb,
                    style = MaterialTheme.typography.labelMedium,
                    color = secondaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 6.dp),
                )
            }
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (browserState.loading) {
                    item(key = "dav_loading") {
                        Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator(color = accent, strokeWidth = 2.5.dp)
                        }
                    }
                }
                if (!browserState.loading && browserState.webdavFolders.isEmpty() && browserState.webdavTracks.isEmpty()) {
                    item(key = "dav_empty") {
                        Text(
                            text = s.emptyFolder,
                            style = MaterialTheme.typography.bodyMedium,
                            color = secondaryText,
                            modifier = Modifier.padding(vertical = 24.dp),
                        )
                    }
                }
                items(browserState.webdavFolders, key = { "dav_dir_${it.id}" }) { folder ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(cardBg)
                            .clickable { onOpenWebDavFolder(folder.id, folder.name) }
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Folder,
                            contentDescription = null,
                            tint = accent,
                            modifier = Modifier.size(22.dp),
                        )
                        Spacer(Modifier.width(12.dp))
                        Text(
                            text = folder.name,
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Medium),
                            color = primaryText,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Icon(
                            imageVector = SonataIcons.ChevronRight,
                            contentDescription = null,
                            tint = secondaryText,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                items(browserState.webdavTracks, key = { "dav_trk_${it.id}" }) { track ->
                    ServerTrackRow(
                        track = track,
                        isCurrent = activeTrackId == track.id,
                        isPlaying = activeTrackId == track.id && isPlaying,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        accent = accent,
                        onClick = { onPlayTrack(track, browserState.webdavTracks) },
                    )
                }
            }
            return
        }

        // Jellyfin / Subsonic browse
        if (browserState.selectedArtistName != null) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onCloseArtist)
                    .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = SonataIcons.ChevronLeft,
                    contentDescription = strings.back,
                    tint = accent,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = browserState.selectedArtistName,
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                    color = primaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (browserState.loading) {
                    item(key = "artist_albums_loading") {
                        Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator(color = accent, strokeWidth = 2.5.dp)
                        }
                    }
                }
                items(browserState.artistAlbums, key = { "srv_art_alb_${it.id}" }) { album ->
                    ServerCardRow(
                        card = album,
                        cardBg = cardBg,
                        primaryText = primaryText,
                        secondaryText = secondaryText,
                        onClick = { onOpenAlbum(album) },
                    )
                }
            }
            return
        }

        if (browserState.libraries.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = PAGE_GUTTER, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                LibraryChip(
                    label = s.allLibraries,
                    selected = browserState.selectedLibraryId == null,
                    cardBg = cardBg,
                    primaryText = primaryText,
                    accent = accent,
                    onClick = { onSelectLibrary(null) },
                )
                browserState.libraries.forEach { lib ->
                    LibraryChip(
                        label = lib.name,
                        selected = browserState.selectedLibraryId == lib.id,
                        cardBg = cardBg,
                        primaryText = primaryText,
                        accent = accent,
                        onClick = { onSelectLibrary(lib.id) },
                    )
                }
            }
        }

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
            contentAlignment = Alignment.Center,
        ) {
            PillSegmentedControl(
                options = segments,
                selectedIndex = selectedSegment,
                onSelect = { selectedSegment = it },
            )
        }

        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(horizontal = PAGE_GUTTER, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (browserState.loading) {
                item(key = "srv_browse_loading") {
                    Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = accent, strokeWidth = 2.5.dp)
                    }
                }
            }
            when (selectedSegment) {
                0 -> {
                    val albumRows = browserState.albums.chunked(2)
                    if (!browserState.loading && albumRows.isEmpty()) {
                        item(key = "srv_albums_empty") {
                            Text(s.emptyServerSection, color = secondaryText, modifier = Modifier.padding(16.dp))
                        }
                    }
                    items(albumRows, key = { row -> "alb_row_${row.joinToString("_") { it.id }}" }) { pair ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            pair.forEach { card ->
                                ServerAlbumGridTile(
                                    card = card,
                                    cardBg = cardBg,
                                    dividerColor = dividerColor,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    onClick = { onOpenAlbum(card) },
                                    modifier = Modifier.weight(1f),
                                )
                            }
                            if (pair.size == 1) {
                                Spacer(Modifier.weight(1f))
                            }
                        }
                    }
                }

                1 -> {
                    if (!browserState.loading && browserState.playlists.isEmpty()) {
                        item(key = "srv_pl_empty") {
                            Text(s.emptyServerSection, color = secondaryText, modifier = Modifier.padding(16.dp))
                        }
                    }
                    items(browserState.playlists, key = { "srv_pl_${it.id}" }) { pl ->
                        ServerCardRow(
                            card = AndroidHomeCard(
                                kind = AndroidHomeCardKind.PLAYLIST,
                                id = pl.id,
                                title = pl.name,
                                subtitle = s.playlistsTab,
                                thumbnailUrl = pl.thumbnailUrl,
                                track = null,
                            ),
                            cardBg = cardBg,
                            primaryText = primaryText,
                            secondaryText = secondaryText,
                            onClick = { onOpenPlaylist(pl) },
                        )
                    }
                }

                else -> {
                    if (!browserState.loading && browserState.artists.isEmpty()) {
                        item(key = "srv_art_empty") {
                            Text(s.emptyServerSection, color = secondaryText, modifier = Modifier.padding(16.dp))
                        }
                    }
                    items(browserState.artists, key = { "srv_art_${it.id}" }) { artist ->
                        ServerCardRow(
                            card = artist,
                            cardBg = cardBg,
                            primaryText = primaryText,
                            secondaryText = secondaryText,
                            onClick = { onOpenArtist(artist) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun LibraryChip(
    label: String,
    selected: Boolean,
    cardBg: Color,
    primaryText: Color,
    accent: Color,
    onClick: () -> Unit,
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(18.dp))
            .background(if (selected) accent else cardBg)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 7.dp),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Medium),
            color = if (selected) Color.White else primaryText,
            maxLines = 1,
        )
    }
}

@Composable
private fun ServerAlbumGridTile(
    card: AndroidHomeCard,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cover = remember(card.thumbnailUrl) { card.thumbnailUrl.artworkAt(CARD_ART_PX) }
    val req = rememberArtworkRequest(cover, 360, 360)
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .clip(RoundedCornerShape(12.dp))
                .background(cardBg)
                .border(0.75.dp, dividerColor, RoundedCornerShape(12.dp)),
            contentAlignment = Alignment.Center,
        ) {
            if (req != null) {
                AsyncImage(
                    model = req,
                    contentDescription = card.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = secondaryText,
                    modifier = Modifier.size(36.dp),
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            text = card.title,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
            color = primaryText,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (card.subtitle.isNotBlank()) {
            Text(
                text = card.subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun ServerCardRow(
    card: AndroidHomeCard,
    cardBg: Color,
    primaryText: Color,
    secondaryText: Color,
    onClick: () -> Unit,
) {
    val thumb = remember(card.thumbnailUrl) { card.thumbnailUrl.artworkAt(ROW_ART_PX) }
    val req = rememberArtworkRequest(thumb, ROW_ART_PX, ROW_ART_PX)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(cardBg)
            .clickable(onClick = onClick)
            .padding(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(secondaryText.copy(alpha = 0.14f)),
            contentAlignment = Alignment.Center,
        ) {
            if (req != null) {
                AsyncImage(
                    model = req,
                    contentDescription = card.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = if (card.kind == AndroidHomeCardKind.ARTIST) SonataIcons.Person else SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = secondaryText,
                    modifier = Modifier.size(22.dp),
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                text = card.title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                color = primaryText,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (card.subtitle.isNotBlank()) {
                Text(
                    text = card.subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Icon(
            imageVector = SonataIcons.ChevronRight,
            contentDescription = null,
            tint = secondaryText,
            modifier = Modifier.size(18.dp),
        )
    }
}

@Composable
private fun ServerTrackRow(
    track: AndroidTrack,
    isCurrent: Boolean,
    isPlaying: Boolean,
    primaryText: Color,
    secondaryText: Color,
    accent: Color,
    onClick: () -> Unit,
) {
    val thumb = remember(track.thumbnailUrl) { track.thumbnailUrl.artworkAt(ROW_ART_PX) }
    val req = rememberArtworkRequest(thumb, ROW_ART_PX, ROW_ART_PX)
    val tag = remember(track.id) {
        when {
            track.id.startsWith("jellyfin:") -> "JFN"
            track.id.startsWith("subsonic:") -> "SUB"
            track.id.startsWith("smb:") -> "SMB"
            track.id.startsWith("webdav:") -> "DAV"
            else -> null
        }
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(46.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(secondaryText.copy(alpha = 0.14f)),
            contentAlignment = Alignment.Center,
        ) {
            if (req != null) {
                AsyncImage(
                    model = req,
                    contentDescription = track.title,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    imageVector = SonataIcons.MusicNote,
                    contentDescription = null,
                    tint = secondaryText,
                    modifier = Modifier.size(20.dp),
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
                        tint = accent,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                text = track.title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                color = if (isCurrent) accent else primaryText,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = listOf(track.artist, track.album).filter { it.isNotBlank() }.joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (tag != null) {
            Spacer(Modifier.width(8.dp))
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .background(accent.copy(alpha = 0.14f))
                    .padding(horizontal = 6.dp, vertical = 2.dp),
            ) {
                Text(
                    text = tag,
                    style = MaterialTheme.typography.labelSmall.copy(fontSize = 10.sp, fontWeight = FontWeight.Bold),
                    color = accent,
                )
            }
        }
    }
}

@Composable
private fun ServerEditorDialog(
    initialProfile: AndroidServerProfile?,
    src: SourcesStrings,
    testingConnection: Boolean,
    testResultMessage: String?,
    testResultIsError: Boolean,
    saving: Boolean,
    saveError: String?,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accent: Color,
    onTestConnection: (AndroidServerProfile) -> Unit,
    onSave: (AndroidServerProfile) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    val s = strings.servers
    val isEditing = initialProfile != null

    var name by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.name.orEmpty()) }
    var kind by rememberSaveable(initialProfile?.id) {
        mutableStateOf(initialProfile?.kind ?: AndroidServerKind.JELLYFIN)
    }
    var baseUrl by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.baseUrl.orEmpty()) }
    var username by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.username.orEmpty()) }
    var password by rememberSaveable(initialProfile?.id) { mutableStateOf("") }
    var apiKey by rememberSaveable(initialProfile?.id) { mutableStateOf("") }
    var accessToken by rememberSaveable(initialProfile?.id) { mutableStateOf("") }
    var userId by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.userId.orEmpty()) }
    var bearerToken by rememberSaveable(initialProfile?.id) { mutableStateOf("") }
    var smbShare by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.share.orEmpty()) }
    var smbDomain by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.domain.orEmpty()) }
    var smbMountPath by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.localMountPath.orEmpty()) }
    var enabled by rememberSaveable(initialProfile?.id) { mutableStateOf(initialProfile?.enabled ?: true) }
    var validationError by remember { mutableStateOf<String?>(null) }

    fun buildCandidate(): AndroidServerProfile? {
        val cleanName = name.trim()
        val cleanUrl = baseUrl.trim().trimEnd('/')
        val cleanMount = smbMountPath.trim().takeIf { it.isNotEmpty() }
        if (cleanName.isBlank()) {
            validationError = s.validationNameRequired
            return null
        }
        if (kind == AndroidServerKind.SMB) {
            if (cleanUrl.isBlank() && cleanMount == null) {
                validationError = s.validationUrlRequired
                return null
            }
        } else if (!cleanUrl.startsWith("http://") && !cleanUrl.startsWith("https://")) {
            validationError = s.validationUrlRequired
            return null
        }
        validationError = null
        return AndroidServerProfile(
            id = initialProfile?.id.orEmpty(),
            name = cleanName,
            kind = kind,
            baseUrl = cleanUrl,
            enabled = enabled,
            username = username.trim().takeIf { it.isNotEmpty() },
            password = password.takeIf { it.isNotEmpty() } ?: initialProfile?.password,
            apiKey = apiKey.trim().takeIf { it.isNotEmpty() } ?: initialProfile?.apiKey,
            accessToken = accessToken.trim().takeIf { it.isNotEmpty() } ?: initialProfile?.accessToken,
            userId = userId.trim().takeIf { it.isNotEmpty() },
            bearerToken = bearerToken.trim().takeIf { it.isNotEmpty() } ?: initialProfile?.bearerToken,
            libraryIds = initialProfile?.libraryIds.orEmpty(),
            share = if (kind == AndroidServerKind.SMB) smbShare.trim().takeIf { it.isNotEmpty() } else null,
            domain = if (kind == AndroidServerKind.SMB) smbDomain.trim().takeIf { it.isNotEmpty() } else null,
            localMountPath = if (kind == AndroidServerKind.SMB) cleanMount else null,
        )
    }

    val fieldColors = OutlinedTextFieldDefaults.colors(
        focusedBorderColor = accent,
        unfocusedBorderColor = dividerColor,
        focusedTextColor = primaryText,
        unfocusedTextColor = primaryText,
        focusedLabelColor = accent,
        unfocusedLabelColor = secondaryText,
        cursorColor = accent,
    )

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .widthIn(max = 540.dp)
                .clip(RoundedCornerShape(20.dp))
                .background(cardBg)
                .border(0.75.dp, dividerColor, RoundedCornerShape(20.dp))
                .padding(20.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = if (isEditing) s.editServer else s.addServer,
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                color = primaryText,
            )

            // Protocol chips
            Text(
                text = s.serverKindLabel,
                style = MaterialTheme.typography.labelMedium,
                color = secondaryText,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                AndroidServerKind.entries.forEach { k ->
                    val selected = kind == k
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .clip(RoundedCornerShape(10.dp))
                            .background(if (selected) accent else dividerColor.copy(alpha = 0.35f))
                            .clickable(enabled = !isEditing) { kind = k }
                            .padding(vertical = 8.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = serverKindShortTag(k),
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                            color = if (selected) Color.White else primaryText,
                        )
                    }
                }
            }
            Text(
                text = serverKindLabel(kind, s, src),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )

            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(s.serverNameLabel) },
                placeholder = { Text(s.serverNamePlaceholder) },
                singleLine = true,
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = baseUrl,
                onValueChange = { baseUrl = it },
                label = { Text(s.serverBaseUrlLabel) },
                placeholder = {
                    Text(
                        if (kind == AndroidServerKind.SMB) "smb://192.168.1.20/Music"
                        else s.serverBaseUrlPlaceholder,
                    )
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = username,
                onValueChange = { username = it },
                label = { Text(s.serverUsernameLabel) },
                singleLine = true,
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = password,
                onValueChange = { password = it },
                label = { Text(s.serverPasswordLabel) },
                placeholder = {
                    if (isEditing && !initialProfile?.password.isNullOrEmpty()) {
                        Text(s.serverPasswordKeepHint)
                    }
                },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                colors = fieldColors,
                modifier = Modifier.fillMaxWidth(),
            )

            if (kind == AndroidServerKind.JELLYFIN) {
                OutlinedTextField(
                    value = apiKey,
                    onValueChange = { apiKey = it },
                    label = { Text(s.serverApiKeyLabel) },
                    placeholder = {
                        if (isEditing && !initialProfile?.apiKey.isNullOrEmpty()) {
                            Text(s.serverPasswordKeepHint)
                        }
                    },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = accessToken,
                    onValueChange = { accessToken = it },
                    label = { Text(s.serverAccessTokenLabel) },
                    placeholder = {
                        if (isEditing && !initialProfile?.accessToken.isNullOrEmpty()) {
                            Text(s.serverPasswordKeepHint)
                        }
                    },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = userId,
                    onValueChange = { userId = it },
                    label = { Text(s.serverUserIdLabel) },
                    singleLine = true,
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
            } else if (kind == AndroidServerKind.WEB_DAV) {
                OutlinedTextField(
                    value = bearerToken,
                    onValueChange = { bearerToken = it },
                    label = { Text(s.serverBearerTokenLabel) },
                    placeholder = {
                        if (isEditing && !initialProfile?.bearerToken.isNullOrEmpty()) {
                            Text(s.serverPasswordKeepHint)
                        }
                    },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
            } else if (kind == AndroidServerKind.SMB) {
                OutlinedTextField(
                    value = smbShare,
                    onValueChange = { smbShare = it },
                    label = { Text(src.smbShareLabel) },
                    placeholder = { Text("Music") },
                    singleLine = true,
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = smbDomain,
                    onValueChange = { smbDomain = it },
                    label = { Text(src.smbDomainLabel) },
                    placeholder = { Text("WORKGROUP") },
                    singleLine = true,
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = smbMountPath,
                    onValueChange = { smbMountPath = it },
                    label = { Text("Local Mount Path (optional)") },
                    placeholder = { Text("/mnt/music") },
                    singleLine = true,
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = s.serverEnabledLabel,
                    style = MaterialTheme.typography.bodyLarge,
                    color = primaryText,
                    modifier = Modifier.weight(1f),
                )
                Switch(
                    checked = enabled,
                    onCheckedChange = { enabled = it },
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = Color.White,
                        checkedTrackColor = Color(0xFF34C759),
                    ),
                )
            }

            val errText = validationError ?: saveError
            if (!errText.isNullOrBlank()) {
                Text(
                    text = errText,
                    style = MaterialTheme.typography.bodySmall,
                    color = BasicPalette.AccentRed,
                )
            }

            if (!testResultMessage.isNullOrBlank()) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(
                            if (testResultIsError) BasicPalette.AccentRed.copy(alpha = 0.14f)
                            else Color(0xFF34C759).copy(alpha = 0.14f),
                        )
                        .padding(10.dp),
                ) {
                    Text(
                        text = testResultMessage,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (testResultIsError) BasicPalette.AccentRed else Color(0xFF34C759),
                    )
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(
                    enabled = !testingConnection && !saving,
                    onClick = {
                        buildCandidate()?.let(onTestConnection)
                    },
                ) {
                    Text(
                        text = if (testingConnection) s.testingConnection else s.testConnection,
                        color = accent,
                    )
                }
                Spacer(Modifier.weight(1f))
                TextButton(onClick = onDismiss) {
                    Text(strings.cancel, color = secondaryText)
                }
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent)
                        .clickable(enabled = !saving) {
                            buildCandidate()?.let(onSave)
                        }
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                ) {
                    Text(
                        text = if (saving) s.savingServer else s.saveServer,
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = Color.White,
                    )
                }
            }
        }
    }
}

private fun serverKindShortTag(kind: AndroidServerKind): String = when (kind) {
    AndroidServerKind.JELLYFIN -> "JFN"
    AndroidServerKind.SUBSONIC -> "SUB"
    AndroidServerKind.WEB_DAV -> "DAV"
    AndroidServerKind.SMB -> "SMB"
}

private fun serverKindLabel(kind: AndroidServerKind, s: ServerStrings, src: SourcesStrings): String = when (kind) {
    AndroidServerKind.JELLYFIN -> s.kindJellyfin
    AndroidServerKind.SUBSONIC -> s.kindSubsonic
    AndroidServerKind.WEB_DAV -> s.kindWebDav
    AndroidServerKind.SMB -> src.protocolSmb
}
