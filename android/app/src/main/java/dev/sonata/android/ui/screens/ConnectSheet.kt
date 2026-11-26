/*
 * Sonata Connect Devices, Interactive Approval Pairing, Legacy PIN Fallback,
 * Remote Playback Control & Queue Handoff Sheet.
 */
package dev.sonata.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.ConnectUiState
import dev.sonata.android.connect.SonataConnectCrypto
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.connectErrorMessage
import dev.sonata.android.ui.i18n.ecosystemStrings
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.SonataGlassCard
import kotlin.math.roundToLong

private val ConnectActiveGreen = Color(0xFF34C759)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConnectSheet(
    connectState: ConnectUiState,
    hasLocalQueue: Boolean,
    appLanguage: AppLanguage,
    onToggleEnabled: (Boolean) -> Unit,
    onDeviceNameChange: (String) -> Unit,
    onRefreshDiscovery: () -> Unit,
    onRequestPairApproval: (ConnectBeacon) -> Unit,
    onCancelPairing: () -> Unit,
    onPairDevice: (ConnectBeacon, String) -> Unit,
    onConnectPaired: (ConnectBeacon) -> Unit,
    onDisconnect: () -> Unit,
    onForgetDevice: (String) -> Unit,
    onSendCommand: (ConnectCommand) -> Unit,
    onTransferQueue: () -> Unit,
    onDismiss: () -> Unit,
) {
    val tokens = LocalLunariaColors.current
    val accent = sonataThemeAccent()
    val strings = LocalSonataStrings.current
    val eco = remember(appLanguage) { ecosystemStrings(appLanguage) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var pinPairingTarget by remember { mutableStateOf<ConnectBeacon?>(null) }
    var pinInput by remember { mutableStateOf("") }
    var deviceNameDraft by remember(connectState.deviceName) { mutableStateOf(connectState.deviceName) }

    LaunchedEffect(connectState.activePeer?.deviceId, connectState.pairedDeviceIds) {
        val targetId = pinPairingTarget?.deviceId
        if (targetId != null &&
            (connectState.activePeer?.deviceId == targetId || targetId in connectState.pairedDeviceIds)
        ) {
            pinPairingTarget = null
            pinInput = ""
        }
    }

    // Interactive approval polling dialog (default pairing flow)
    connectState.pendingPair?.let { pending ->
        AlertDialog(
            onDismissRequest = onCancelPairing,
            containerColor = tokens.eclipseSurface,
            titleContentColor = tokens.onSurfacePrimary,
            textContentColor = tokens.onSurfaceSecondary,
            title = { Text(eco.connectWaitingApprovalTitle) },
            text = {
                Column(
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    CircularProgressIndicator(
                        color = accent,
                        strokeWidth = 2.5.dp,
                        modifier = Modifier.size(30.dp),
                    )
                    Text(
                        text = eco.connectWaitingApprovalBody(pending.beacon.deviceName),
                        style = MaterialTheme.typography.bodyMedium,
                        color = tokens.onSurfacePrimary,
                    )
                    TextButton(
                        onClick = {
                            val target = pending.beacon
                            onCancelPairing()
                            pinInput = ""
                            pinPairingTarget = target
                        },
                    ) {
                        Text(
                            text = eco.connectUsePinFallback,
                            style = MaterialTheme.typography.labelMedium,
                            color = accent,
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = onCancelPairing) {
                    Text(strings.cancel, color = tokens.onSurfaceSecondary)
                }
            },
        )
    }

    // Optional legacy 6-digit PIN fallback dialog
    val pinTarget = pinPairingTarget
    if (pinTarget != null && connectState.pendingPair == null) {
        AlertDialog(
            onDismissRequest = {
                if (!connectState.pairingInProgress) {
                    pinPairingTarget = null
                    pinInput = ""
                }
            },
            containerColor = tokens.eclipseSurface,
            titleContentColor = tokens.onSurfacePrimary,
            textContentColor = tokens.onSurfaceSecondary,
            title = { Text(eco.connectPairPinTitle) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "${pinTarget.deviceName} · ${eco.connectPairPinHint}",
                        style = MaterialTheme.typography.bodySmall,
                        color = tokens.onSurfaceSecondary,
                    )
                    OutlinedTextField(
                        value = pinInput,
                        onValueChange = { raw ->
                            pinInput = raw.filter { it.isDigit() }.take(6)
                        },
                        enabled = !connectState.pairingInProgress,
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        placeholder = { Text("000000") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    connectState.pairingError?.let { err ->
                        Text(
                            text = eco.connectErrorMessage(err),
                            style = MaterialTheme.typography.labelSmall,
                            color = BasicPalette.AccentRed,
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(
                    enabled = SonataConnectCrypto.isValidPin(pinInput) && !connectState.pairingInProgress,
                    onClick = { onPairDevice(pinTarget, pinInput) },
                ) {
                    Text(eco.connectPairButton, color = accent, fontWeight = FontWeight.SemiBold)
                }
            },
            dismissButton = {
                TextButton(
                    enabled = !connectState.pairingInProgress,
                    onClick = {
                        pinPairingTarget = null
                        pinInput = ""
                    },
                ) {
                    Text(strings.cancel, color = tokens.onSurfaceSecondary)
                }
            },
        )
    }

    val discoveredIds = remember(connectState.discoveredDevices) {
        connectState.discoveredDevices.mapTo(HashSet()) { it.deviceId }
    }
    val offlineTrustedDevices = remember(
        connectState.pairedDeviceIds,
        connectState.trustedDeviceNames,
        discoveredIds,
    ) {
        connectState.pairedDeviceIds
            .filter { it !in discoveredIds }
            .map { id -> id to (connectState.trustedDeviceNames[id]?.takeIf { it.isNotBlank() } ?: id) }
            .sortedBy { it.second.lowercase() }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = tokens.eclipseSurface,
        contentColor = tokens.onSurfacePrimary,
        dragHandle = null,
    ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(horizontal = PAGE_GUTTER),
            contentPadding = PaddingValues(top = 18.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item(key = "connect_header") {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            text = eco.connectDevicesTitle,
                            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
                            color = tokens.onSurfacePrimary,
                        )
                        Text(
                            text = eco.connectSubtitle,
                            style = MaterialTheme.typography.bodySmall,
                            color = tokens.onSurfaceSecondary,
                        )
                    }
                    CircleGlyph(
                        icon = SonataIcons.Close,
                        contentDescription = strings.close,
                        onClick = onDismiss,
                    )
                }
            }

            // "This Phone" local playback selector
            item(key = "connect_this_phone") {
                val isLocalActive = connectState.activePeer == null
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(14.dp))
                        .background(
                            if (isLocalActive) ConnectActiveGreen.copy(alpha = 0.12f) else tokens.eclipseCard,
                        )
                        .border(
                            width = if (isLocalActive) 1.dp else 0.75.dp,
                            color = if (isLocalActive) ConnectActiveGreen.copy(alpha = 0.55f) else tokens.cardStroke,
                            shape = RoundedCornerShape(14.dp),
                        )
                        .clickable(enabled = !isLocalActive, onClick = onDisconnect)
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = if (isLocalActive) ConnectActiveGreen else tokens.onSurfaceSecondary,
                        modifier = Modifier.size(20.dp),
                    )
                    Column(Modifier.weight(1f)) {
                        Text(
                            text = eco.connectThisPhone,
                            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = tokens.onSurfacePrimary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = eco.connectThisPhoneSubtitle,
                            style = MaterialTheme.typography.labelSmall,
                            color = tokens.onSurfaceSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (isLocalActive) {
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(ConnectActiveGreen.copy(alpha = 0.18f))
                                .padding(horizontal = 10.dp, vertical = 4.dp),
                        ) {
                            Text(
                                text = eco.connectActiveBadge,
                                style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = ConnectActiveGreen,
                            )
                        }
                    }
                }
            }

            if (pinTarget == null && connectState.pendingPair == null && connectState.pairingError != null) {
                item(key = "connect_pairing_error") {
                    SonataGlassCard(
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(tokens.cardCornerRadius),
                    ) {
                        Text(
                            text = eco.connectErrorMessage(connectState.pairingError),
                            style = MaterialTheme.typography.bodySmall,
                            color = BasicPalette.AccentRed,
                            modifier = Modifier.padding(14.dp),
                        )
                    }
                }
            }

            val activePeer = connectState.activePeer
            if (activePeer != null) {
                item(key = "connect_active_peer") {
                    SonataGlassCard(
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(tokens.cardCornerRadius),
                    ) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Column(Modifier.weight(1f)) {
                                    Text(
                                        text = eco.connectPlayingOn(activePeer.deviceName),
                                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                                        color = tokens.onSurfacePrimary,
                                    )
                                    Text(
                                        text = "${activePeer.platform} · ${activePeer.hostAddress}:${activePeer.httpPort}",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = tokens.onSurfaceSecondary,
                                    )
                                }
                                Box(
                                    modifier = Modifier
                                        .clip(CircleShape)
                                        .background(ConnectActiveGreen.copy(alpha = 0.18f))
                                        .padding(horizontal = 10.dp, vertical = 4.dp),
                                ) {
                                    Text(
                                        text = eco.connectActiveBadge,
                                        style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                                        color = ConnectActiveGreen,
                                    )
                                }
                            }

                            val remote = connectState.remoteState
                            if (remote != null) {
                                val track = remote.activeTrack
                                Text(
                                    text = track?.let { "${it.title} — ${it.artist}" } ?: strings.noTrackPlaying,
                                    style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                                    color = tokens.onSurfacePrimary,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )

                                val durationMs = remote.durationMs.coerceAtLeast(1L)
                                var seekFraction by remember(remote.positionMs, durationMs) {
                                    mutableFloatStateOf(
                                        (remote.positionMs.toFloat() / durationMs.toFloat()).coerceIn(0f, 1f),
                                    )
                                }
                                Slider(
                                    value = seekFraction,
                                    onValueChange = { seekFraction = it },
                                    onValueChangeFinished = {
                                        onSendCommand(
                                            ConnectCommand.SeekAbsolute((seekFraction * durationMs).roundToLong()),
                                        )
                                    },
                                    colors = SliderDefaults.colors(
                                        thumbColor = accent,
                                        activeTrackColor = accent,
                                    ),
                                )

                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceEvenly,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    CircleGlyph(
                                        icon = SonataIcons.ChevronLeft,
                                        contentDescription = strings.previous,
                                        onClick = { onSendCommand(ConnectCommand.Prev) },
                                    )
                                    CircleGlyph(
                                        icon = if (remote.isPlaying) SonataIcons.Pause else SonataIcons.Play,
                                        contentDescription = if (remote.isPlaying) strings.pause else strings.play,
                                        onClick = {
                                            onSendCommand(
                                                if (remote.isPlaying) ConnectCommand.Pause else ConnectCommand.Play,
                                            )
                                        },
                                    )
                                    CircleGlyph(
                                        icon = SonataIcons.FastForward,
                                        contentDescription = strings.next,
                                        onClick = { onSendCommand(ConnectCommand.Next) },
                                    )
                                }

                                var volumeDraft by remember(remote.volume) {
                                    mutableFloatStateOf(remote.volume.coerceIn(0f, 1f))
                                }
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    Text(
                                        text = eco.connectRemoteVolume,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = tokens.onSurfaceSecondary,
                                    )
                                    Row(
                                        modifier = Modifier.fillMaxWidth(),
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                                    ) {
                                        Icon(
                                            imageVector = SonataIcons.GraphicEq,
                                            contentDescription = eco.connectRemoteVolume,
                                            tint = tokens.onSurfaceSecondary,
                                            modifier = Modifier.size(18.dp),
                                        )
                                        Slider(
                                            value = volumeDraft,
                                            onValueChange = { volumeDraft = it },
                                            onValueChangeFinished = {
                                                onSendCommand(ConnectCommand.SetVolume(volumeDraft))
                                            },
                                            modifier = Modifier.weight(1f),
                                            colors = SliderDefaults.colors(
                                                thumbColor = accent,
                                                activeTrackColor = accent,
                                            ),
                                        )
                                    }
                                }
                            }

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                if (hasLocalQueue) {
                                    Button(
                                        onClick = onTransferQueue,
                                        enabled = !connectState.transferInProgress,
                                        colors = ButtonDefaults.buttonColors(containerColor = accent),
                                        modifier = Modifier.weight(1f),
                                    ) {
                                        Text(
                                            text = eco.connectTransferPlayback,
                                            color = if (tokens.isBasic && tokens.isDark) Color.Black else Color.White,
                                        )
                                    }
                                }
                                Button(
                                    onClick = onDisconnect,
                                    colors = ButtonDefaults.buttonColors(containerColor = tokens.eclipseCard),
                                    modifier = Modifier.weight(1f),
                                ) {
                                    Text(eco.connectDisconnect, color = tokens.onSurfacePrimary)
                                }
                            }
                        }
                    }
                }
            }

            item(key = "connect_devices_header") {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = eco.connectDiscoverTitle,
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                        color = tokens.onSurfacePrimary,
                    )
                    if (connectState.enabled) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier
                                .clip(CircleShape)
                                .clickable(onClick = onRefreshDiscovery)
                                .padding(horizontal = 8.dp, vertical = 4.dp),
                        ) {
                            if (connectState.scanning) {
                                CircularProgressIndicator(
                                    modifier = Modifier.size(14.dp),
                                    strokeWidth = 2.dp,
                                    color = accent,
                                )
                            }
                            Text(
                                text = strings.refresh,
                                style = MaterialTheme.typography.labelMedium,
                                color = accent,
                            )
                        }
                    }
                }
            }

            if (connectState.discoveredDevices.isEmpty()) {
                item(key = "connect_empty_devices") {
                    SonataGlassCard(
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(tokens.cardCornerRadius),
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Wifi,
                                contentDescription = null,
                                tint = tokens.onSurfaceSecondary,
                                modifier = Modifier.size(20.dp),
                            )
                            Text(
                                text = if (connectState.enabled && connectState.scanning) {
                                    eco.connectScanning
                                } else {
                                    eco.connectNoDevices
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = tokens.onSurfaceSecondary,
                            )
                        }
                    }
                }
            } else {
                items(connectState.discoveredDevices, key = { it.deviceId }) { beacon ->
                    val isPaired = beacon.deviceId in connectState.pairedDeviceIds
                    val isActive = connectState.activePeer?.deviceId == beacon.deviceId
                    val actionClick = {
                        when {
                            isActive -> onDisconnect()
                            isPaired -> onConnectPaired(beacon)
                            else -> onRequestPairApproval(beacon)
                        }
                    }
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(
                                if (isActive) ConnectActiveGreen.copy(alpha = 0.12f) else tokens.eclipseCard,
                            )
                            .border(
                                width = if (isActive) 1.dp else 0.75.dp,
                                color = if (isActive) ConnectActiveGreen.copy(alpha = 0.55f) else tokens.cardStroke,
                                shape = RoundedCornerShape(14.dp),
                            )
                            .clickable(onClick = actionClick)
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Icon(
                            imageVector = SonataIcons.Wifi,
                            contentDescription = null,
                            tint = if (isActive) ConnectActiveGreen else accent,
                            modifier = Modifier.size(20.dp),
                        )
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = beacon.deviceName,
                                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = tokens.onSurfacePrimary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = if (isPaired && !isActive) {
                                    "${beacon.platform} · ${eco.connectSelectDeviceHint}"
                                } else {
                                    "${beacon.platform} · ${beacon.hostAddress}:${beacon.httpPort}"
                                },
                                style = MaterialTheme.typography.labelSmall,
                                color = tokens.onSurfaceSecondary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        if (isPaired) {
                            TextButton(onClick = { onForgetDevice(beacon.deviceId) }) {
                                Text(strings.remove, color = tokens.onSurfaceSecondary)
                            }
                        }
                        val badgeTint = when {
                            isActive -> ConnectActiveGreen
                            isPaired -> ConnectActiveGreen
                            else -> accent
                        }
                        val badgeText = when {
                            isActive -> eco.connectActiveBadge
                            isPaired -> eco.connectTrustedBadge
                            else -> eco.connectPairButton
                        }
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(badgeTint.copy(alpha = 0.16f))
                                .clickable(onClick = actionClick)
                                .padding(horizontal = 12.dp, vertical = 6.dp),
                        ) {
                            Text(
                                text = badgeText,
                                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                                color = badgeTint,
                            )
                        }
                    }
                }
            }

            if (offlineTrustedDevices.isNotEmpty()) {
                item(key = "connect_trusted_offline_header") {
                    Text(
                        text = eco.connectTrustedDevicesTitle,
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        color = tokens.onSurfaceSecondary,
                    )
                }
                items(offlineTrustedDevices, key = { "trusted_${it.first}" }) { (deviceId, deviceName) ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(tokens.eclipseCard.copy(alpha = 0.72f))
                            .border(0.75.dp, tokens.cardStroke, RoundedCornerShape(14.dp))
                            .padding(horizontal = 14.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Icon(
                            imageVector = SonataIcons.Wifi,
                            contentDescription = null,
                            tint = tokens.onSurfaceSecondary.copy(alpha = 0.6f),
                            modifier = Modifier.size(18.dp),
                        )
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = deviceName,
                                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                                color = tokens.onSurfacePrimary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = "${eco.connectTrustedBadge} · ${eco.connectOfflineLabel}",
                                style = MaterialTheme.typography.labelSmall,
                                color = tokens.onSurfaceSecondary,
                            )
                        }
                        TextButton(onClick = { onForgetDevice(deviceId) }) {
                            Text(strings.remove, color = tokens.onSurfaceSecondary)
                        }
                    }
                }
            }

            item(key = "connect_controls_card") {
                Spacer(Modifier.height(2.dp))
                SonataGlassCard(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(tokens.cardCornerRadius),
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(
                                text = eco.connectEnableTitle,
                                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = tokens.onSurfacePrimary,
                            )
                            Switch(
                                checked = connectState.enabled,
                                onCheckedChange = onToggleEnabled,
                                colors = SwitchDefaults.colors(checkedTrackColor = ConnectActiveGreen),
                            )
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            OutlinedTextField(
                                value = deviceNameDraft,
                                onValueChange = { deviceNameDraft = it },
                                label = { Text(eco.connectDeviceNameLabel) },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                            )
                            TextButton(
                                onClick = { onDeviceNameChange(deviceNameDraft) },
                            ) {
                                Text(strings.save, color = accent, fontWeight = FontWeight.SemiBold)
                            }
                        }
                    }
                }
            }
        }
    }
}
