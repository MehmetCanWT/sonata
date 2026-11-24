package dev.sonata.android

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectUiState
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.ui.SonataTheme
import dev.sonata.android.ui.i18n.EnglishEcosystemStrings
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.EnglishSourcesStrings
import dev.sonata.android.ui.player.MusicVideoModePill
import dev.sonata.android.ui.screens.ConnectSheet
import dev.sonata.android.ui.screens.ServerDiagnosticState
import dev.sonata.android.ui.screens.SonataSettingsScreen
import dev.sonata.android.ui.screens.SourcesHubContent
import dev.sonata.android.ui.screens.StatsScreen
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.sonata_android.AndroidServerHealthReport
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSpeedTestReport
import uniffi.sonata_android.AndroidStatsSummary
import uniffi.sonata_android.AndroidStatsWindow

/**
 * Compose UI instrumented test suite covering Sources Hub toggles & server diagnostics,
 * Listening Stats time-window switching, Sonata Connect LAN device & PIN pairing dialog,
 * Categorized Settings navigation/search filtering, and Now Playing Music/Video mode pill.
 */
@RunWith(AndroidJUnit4::class)
class SonataComposeUiTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun sourcesHubContentTogglesPersonalModeJioSaavnAndDisplaysServerDiagnostics() {
        val sampleServer = AndroidServerProfile(
            id = "srv_jfn_1",
            name = "Home Jellyfin",
            kind = AndroidServerKind.JELLYFIN,
            baseUrl = "https://music.lan:8096",
            enabled = true,
            username = "listener",
            password = null,
            apiKey = null,
            accessToken = "tok",
            userId = "u1",
            bearerToken = null,
            libraryIds = emptyList(),
        )
        val diagnostics = mapOf(
            sampleServer.id to ServerDiagnosticState(
                healthReport = AndroidServerHealthReport(
                    alive = true,
                    latencyMs = 14uL,
                    message = "OK",
                ),
                speedTestReport = AndroidServerSpeedTestReport(
                    alive = true,
                    latencyMs = 14uL,
                    downloadMbps = 82.4,
                    bytesTested = 262_144uL,
                    sampleNote = "256 KB",
                ),
            ),
        )

        var latestSettings = SonataSettingsState(
            appLanguage = AppLanguage.ENGLISH,
            personalOnlyMode = false,
            jioSaavnEnabled = true,
        )

        composeRule.setContent {
            var settingsState by remember { mutableStateOf(latestSettings) }
            SonataTheme(darkTheme = true, appLanguage = AppLanguage.ENGLISH) {
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    SourcesHubContent(
                        settings = settingsState,
                        signedIn = true,
                        localTrackCount = 42,
                        localPermissionGranted = true,
                        servers = listOf(sampleServer),
                        serverDiagnostics = diagnostics,
                        cardBg = Color(0xFF1C1C1E),
                        dividerColor = Color(0xFF2C2C2E),
                        primaryText = Color.White,
                        secondaryText = Color(0xFFAEAEB2),
                        onTogglePersonalOnlyMode = { enabled ->
                            settingsState = settingsState.copy(personalOnlyMode = enabled)
                            latestSettings = settingsState
                        },
                        onToggleJioSaavn = { enabled ->
                            settingsState = settingsState.copy(jioSaavnEnabled = enabled)
                            latestSettings = settingsState
                        },
                        onToggleAttributePersonalToYtm = {},
                        onToggleLocalMusic = {},
                        onRequestLocalPermission = {},
                        onRescanLocalMusic = {},
                        onProbeServerHealth = {},
                        onRunServerSpeedTest = {},
                        onOpenServers = {},
                    )
                }
            }
        }

        // Verify Alive latency badge and Speed Test summary render cleanly.
        composeRule.onNodeWithText("Home Jellyfin").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("Alive · 14 ms").assertIsDisplayed()
        composeRule.onNodeWithText("14 ms · 82.4 Mbps — 256 KB").assertIsDisplayed()

        // Toggle JioSaavn module off while online streaming is active.
        composeRule.onNodeWithText(EnglishSourcesStrings.jioSaavnModuleTitle)
            .performScrollTo()
            .performClick()
        assertFalse(latestSettings.jioSaavnEnabled)

        // Toggle Personal Only Mode on and verify JioSaavn module shows paused notice.
        composeRule.onNodeWithText(EnglishSourcesStrings.personalOnlyModeTitle)
            .performScrollTo()
            .performClick()
        assertTrue(latestSettings.personalOnlyMode)
        composeRule.onNodeWithText(EnglishSourcesStrings.jioSaavnPausedByPersonalMode)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun statsScreenSwitchesTimeWindowsBetweenWeekMonthYearAndAllTime() {
        val selectedWindows = mutableListOf<AndroidStatsWindow>()
        val emptySummary = AndroidStatsSummary(
            totalPlays = 12uL,
            totalMinutes = 48uL,
            uniqueTracks = 9uL,
            uniqueArtists = 4uL,
            streakDays = 3uL,
            topArtists = emptyList(),
            topTracks = emptyList(),
            topAlbums = emptyList(),
            sourceBreakdown = emptyList(),
        )

        composeRule.setContent {
            var activeWindow by remember { mutableStateOf(AndroidStatsWindow.WEEK) }
            SonataTheme(darkTheme = true, appLanguage = AppLanguage.ENGLISH) {
                StatsScreen(
                    window = activeWindow,
                    summary = emptySummary,
                    loading = false,
                    appLanguage = AppLanguage.ENGLISH,
                    onSelectWindow = { w ->
                        activeWindow = w
                        selectedWindows += w
                    },
                    onRefresh = {},
                    onBack = {},
                )
            }
        }

        composeRule.onNodeWithText(EnglishEcosystemStrings.statsWindowMonth).performClick()
        composeRule.onNodeWithText(EnglishEcosystemStrings.statsWindowYear).performClick()
        composeRule.onNodeWithText(EnglishEcosystemStrings.statsWindowAllTime).performClick()
        composeRule.onNodeWithText(EnglishEcosystemStrings.statsWindowWeek).performClick()

        assertEquals(
            listOf(
                AndroidStatsWindow.MONTH,
                AndroidStatsWindow.YEAR,
                AndroidStatsWindow.ALL_TIME,
                AndroidStatsWindow.WEEK,
            ),
            selectedWindows,
        )
    }

    @Test
    fun connectSheetRendersLanDeviceAndSixDigitPinPairingDialog() {
        val beacon = ConnectBeacon(
            deviceId = "dev_desktop_1",
            deviceName = "Sonata Living Room",
            platform = "Linux",
            httpPort = 47890,
            hostAddress = "192.168.1.42",
        )
        var pairedResult: Pair<ConnectBeacon, String>? = null

        composeRule.setContent {
            SonataTheme(darkTheme = true, appLanguage = AppLanguage.ENGLISH) {
                ConnectSheet(
                    connectState = ConnectUiState(
                        enabled = true,
                        deviceName = "Sonata Android",
                        discoveredDevices = listOf(beacon),
                    ),
                    hasLocalQueue = true,
                    appLanguage = AppLanguage.ENGLISH,
                    onToggleEnabled = {},
                    onDeviceNameChange = {},
                    onRefreshDiscovery = {},
                    onPairDevice = { target, pin -> pairedResult = target to pin },
                    onConnectPaired = {},
                    onDisconnect = {},
                    onForgetDevice = {},
                    onSendCommand = {},
                    onTransferQueue = {},
                    onDismiss = {},
                )
            }
        }

        composeRule.onNodeWithText("Sonata Living Room").assertIsDisplayed()
        composeRule.onNodeWithText(EnglishEcosystemStrings.connectPairButton).performClick()

        // PIN AlertDialog opens with 6-digit input field and Pair confirmation button.
        composeRule.onNodeWithText(EnglishEcosystemStrings.connectPairPinTitle).assertIsDisplayed()
        composeRule.onNodeWithText("000000").performTextInput("123456")
        composeRule.onAllNodesWithText(EnglishEcosystemStrings.connectPairButton)
            .onLast()
            .performClick()

        assertEquals(beacon to "123456", pairedResult)
    }

    @Test
    fun categorizedSettingsSheetNavigatesCategoriesAndFiltersBySearchQuery() {
        composeRule.setContent {
            var settingsState by remember { mutableStateOf(SonataSettingsState(appLanguage = AppLanguage.ENGLISH)) }
            SonataTheme(darkTheme = true, appLanguage = AppLanguage.ENGLISH) {
                SonataSettingsScreen(
                    settings = settingsState,
                    signedIn = false,
                    accountProfile = null,
                    supports120Hz = true,
                    statusMessage = "",
                    updateInfo = null,
                    checkingUpdates = false,
                    installedVersionName = "1.0.7",
                    onUpdateSettings = { transform -> settingsState = transform(settingsState) },
                    onSignIn = {},
                    onSignOut = {},
                    onClearMediaCache = {},
                    onExportBackup = {},
                    onImportBackup = {},
                    onCheckForUpdates = {},
                    onDownloadUpdate = {},
                    onBack = {},
                )
            }
        }

        // Navigate into Playback category and verify Gapless & Crossfade appear, then navigate back.
        composeRule.onNodeWithText(EnglishSonataStrings.catPlayback)
            .performScrollTo()
            .performClick()
        composeRule.onNodeWithText(EnglishEcosystemStrings.gaplessPlaybackTitle)
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText(EnglishEcosystemStrings.crossfadeTitle)
            .performScrollTo()
            .assertIsDisplayed()

        composeRule.onNodeWithContentDescription(EnglishSonataStrings.back).performClick()

        // Filter root settings via live search query and verify matching section renders.
        composeRule.onNode(hasSetTextAction()).performTextInput("Crossfade")
        composeRule.onNodeWithText(EnglishEcosystemStrings.crossfadeTitle)
            .performScrollTo()
            .assertIsDisplayed()
        assertEquals(
            0,
            composeRule.onAllNodesWithText(EnglishSonataStrings.catDiscord)
                .fetchSemanticsNodes()
                .size,
        )
    }

    @Test
    fun musicVideoModePillTogglesBetweenMusicAndVideoModes() {
        val transitions = mutableListOf<String>()

        composeRule.setContent {
            var videoMode by remember { mutableStateOf(false) }
            SonataTheme(darkTheme = true, appLanguage = AppLanguage.ENGLISH) {
                MusicVideoModePill(
                    videoModeEnabled = videoMode,
                    videoLoading = false,
                    onSelectMusic = {
                        videoMode = false
                        transitions += "music"
                    },
                    onSelectVideo = {
                        videoMode = true
                        transitions += "video"
                    },
                )
            }
        }

        composeRule.onNodeWithText(EnglishSonataStrings.videoMode).performClick()
        composeRule.onNodeWithText(EnglishSonataStrings.musicMode).performClick()

        assertEquals(listOf("video", "music"), transitions)
    }
}
