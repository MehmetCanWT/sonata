/*
 * Sonata Connect, Scrobbling, Discord Rich Presence, Account & Updates,
 * and single-choice selection subpages for Sonata Settings.
 */
package dev.sonata.android.ui.screens.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.sonata.android.connect.ConnectUiState
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadStorageLocation
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.data.VideoDownloadQuality
import dev.sonata.android.discord.DiscordActivityType
import dev.sonata.android.discord.DiscordApplication
import dev.sonata.android.discord.DiscordRpcStatus
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.EcosystemStrings
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.theme.BasicPalette
import uniffi.sonata_android.AndroidAccountProfile
import uniffi.sonata_android.AndroidLastFmConfig
import uniffi.sonata_android.AndroidListenBrainzConfig
import uniffi.sonata_android.AndroidScrobbleConfig
import uniffi.sonata_android.AndroidUpdateInfo
import java.util.Locale

internal fun discordStatusLabel(status: DiscordRpcStatus, strings: SonataStrings): String = when (status) {
    DiscordRpcStatus.Disconnected -> strings.discordStatusDisconnected
    DiscordRpcStatus.Connecting -> strings.discordStatusConnecting
    is DiscordRpcStatus.Connected ->
        status.username?.takeIf { it.isNotBlank() }?.let { strings.discordStatusConnectedAs(it) }
            ?: strings.discordStatusConnected
    DiscordRpcStatus.Reconnecting -> strings.discordStatusReconnecting
    DiscordRpcStatus.AuthFailed -> strings.discordStatusAuthFailed
    is DiscordRpcStatus.Error -> strings.discordStatusError(status.message)
}

internal fun discordActivityTypeLabel(type: DiscordActivityType, strings: SonataStrings): String = when (type) {
    DiscordActivityType.LISTENING -> strings.discordActivityListening
    DiscordActivityType.PLAYING -> strings.discordActivityPlaying
    DiscordActivityType.WATCHING -> strings.discordActivityWatching
    DiscordActivityType.COMPETING -> strings.discordActivityCompeting
}

@Composable
internal fun ConnectCategoryContent(
    settings: SonataSettingsState,
    eco: EcosystemStrings,
    connectState: ConnectUiState,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    onUpdateSettings: ((SonataSettingsState) -> SonataSettingsState) -> Unit,
    onOpenConnect: () -> Unit,
) {
    val strings = LocalSonataStrings.current
    var nameDraft by remember(settings.connectDeviceName) { mutableStateOf(settings.connectDeviceName) }
    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = eco.connectTitle,
        footer = eco.connectSubtitle,
        secondaryText = secondaryText,
    ) {
        SettingsSwitchRow(
            title = eco.connectEnableTitle,
            subtitle = connectState.activePeer?.let { eco.connectConnectedTo(it.deviceName) }
                ?: if (settings.connectEnabled) eco.connectScanning else eco.connectNoDevices,
            checked = settings.connectEnabled,
            primaryText = primaryText,
            secondaryText = secondaryText,
            onCheckedChange = { enabled -> onUpdateSettings { it.copy(connectEnabled = enabled) } },
        )
        addRow(dividerStart = ROW_INSET) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = nameDraft,
                    onValueChange = { nameDraft = it },
                    label = { Text(eco.connectDeviceNameLabel) },
                    singleLine = true,
                    modifier = Modifier.weight(1f),
                )
                TextButton(
                    onClick = {
                        val cleaned = nameDraft.trim().ifEmpty { "Sonata Android" }
                        onUpdateSettings { it.copy(connectDeviceName = cleaned) }
                    },
                ) {
                    Text(strings.save, color = primaryText, fontWeight = FontWeight.SemiBold)
                }
            }
        }
        SettingsRow(
            icon = SonataIcons.Wifi,
            title = eco.connectDiscoverTitle,
            subtitle = eco.connectPairPinHint,
            value = connectState.discoveredDevices.size.toString(),
            primaryText = primaryText,
            secondaryText = secondaryText,
            onClick = onOpenConnect,
        )
    }
}

@Composable
internal fun ScrobblingCategoryContent(
    eco: EcosystemStrings,
    scrobbleConfig: AndroidScrobbleConfig,
    cardBg: Color,
    dividerColor: Color,
    primaryText: Color,
    secondaryText: Color,
    accentTint: Color,
    onSaveScrobbleConfig: (AndroidScrobbleConfig) -> Unit,
) {
    val strings = LocalSonataStrings.current
    var lfmEnabled by remember(scrobbleConfig.lastfm) { mutableStateOf(scrobbleConfig.lastfm.enabled) }
    var lfmUser by remember(scrobbleConfig.lastfm) { mutableStateOf(scrobbleConfig.lastfm.username) }
    var lfmApiKey by remember(scrobbleConfig.lastfm) { mutableStateOf(scrobbleConfig.lastfm.apiKey) }
    var lfmApiSecret by remember(scrobbleConfig.lastfm) { mutableStateOf(scrobbleConfig.lastfm.apiSecret) }
    var lfmSessionKey by remember(scrobbleConfig.lastfm) { mutableStateOf(scrobbleConfig.lastfm.sessionKey) }

    var lbEnabled by remember(scrobbleConfig.listenbrainz) { mutableStateOf(scrobbleConfig.listenbrainz.enabled) }
    var lbUser by remember(scrobbleConfig.listenbrainz) { mutableStateOf(scrobbleConfig.listenbrainz.username) }
    var lbToken by remember(scrobbleConfig.listenbrainz) { mutableStateOf(scrobbleConfig.listenbrainz.token) }

    val saveCurrent: () -> Unit = {
        onSaveScrobbleConfig(
            AndroidScrobbleConfig(
                lastfm = AndroidLastFmConfig(
                    enabled = lfmEnabled,
                    apiKey = lfmApiKey.trim(),
                    apiSecret = lfmApiSecret.trim(),
                    sessionKey = lfmSessionKey.trim(),
                    username = lfmUser.trim(),
                ),
                listenbrainz = AndroidListenBrainzConfig(
                    enabled = lbEnabled,
                    token = lbToken.trim(),
                    username = lbUser.trim(),
                ),
            ),
        )
    }

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = eco.lastFmTitle,
        footer = eco.scrobbleThresholdHint,
        secondaryText = secondaryText,
    ) {
        SettingsSwitchRow(
            title = eco.lastFmTitle,
            subtitle = if (lfmEnabled && lfmUser.isNotBlank()) {
                eco.scrobbleStatusConnected(lfmUser)
            } else {
                eco.lastFmDescription
            },
            checked = lfmEnabled,
            primaryText = primaryText,
            secondaryText = secondaryText,
            onCheckedChange = { checked ->
                lfmEnabled = checked
                onSaveScrobbleConfig(
                    scrobbleConfig.copy(
                        lastfm = AndroidLastFmConfig(
                            enabled = checked,
                            apiKey = lfmApiKey.trim(),
                            apiSecret = lfmApiSecret.trim(),
                            sessionKey = lfmSessionKey.trim(),
                            username = lfmUser.trim(),
                        ),
                    ),
                )
            },
        )
        addRow(dividerStart = ROW_INSET) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = lfmUser,
                    onValueChange = { lfmUser = it },
                    label = { Text("Username") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = lfmApiKey,
                    onValueChange = { lfmApiKey = it },
                    label = { Text("API Key") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = lfmApiSecret,
                    onValueChange = { lfmApiSecret = it },
                    label = { Text("API Secret") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = lfmSessionKey,
                    onValueChange = { lfmSessionKey = it },
                    label = { Text("Session Key") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = saveCurrent,
                    colors = ButtonDefaults.buttonColors(containerColor = accentTint),
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text(strings.save, color = cardBg)
                }
            }
        }
    }

    Spacer(Modifier.height(14.dp))

    SettingsGroup(
        cardBg = cardBg,
        dividerColor = dividerColor,
        header = eco.listenBrainzTitle,
        footer = eco.listenBrainzDescription,
        secondaryText = secondaryText,
    ) {
        SettingsSwitchRow(
            title = eco.listenBrainzTitle,
            subtitle = if (lbEnabled && lbToken.isNotBlank()) {
                eco.scrobbleStatusConnected(lbUser.ifBlank { eco.listenBrainzTitle })
            } else {
                eco.scrobbleStatusNotSignedIn
            },
            checked = lbEnabled,
            primaryText = primaryText,
            secondaryText = secondaryText,
            onCheckedChange = { checked ->
                lbEnabled = checked
                onSaveScrobbleConfig(
                    scrobbleConfig.copy(
                        listenbrainz = AndroidListenBrainzConfig(
                            enabled = checked,
                            token = lbToken.trim(),
                            username = lbUser.trim(),
                        ),
                    ),
                )
            },
        )
        addRow(dividerStart = ROW_INSET) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = ROW_INSET, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = lbUser,
                    onValueChange = { lbUser = it },
                    label = { Text("Username") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = lbToken,
                    onValueChange = { lbToken = it },
                    label = { Text(eco.listenBrainzTokenHint) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = saveCurrent,
                    colors = ButtonDefaults.buttonColors(containerColor = accentTint),
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text(strings.save, color = cardBg)
                }
            }
        }
    }
}

@Composable
internal fun DiscordCategoryContent(
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
internal fun AccountCategoryContent(
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

@Composable
internal fun SettingsChoicePage(
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
