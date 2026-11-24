package dev.sonata.android

import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.discord.DISCORD_TOKEN_ALERT_PREFIX
import dev.sonata.android.discord.DiscordActivityType
import dev.sonata.android.discord.DiscordApplication
import dev.sonata.android.discord.DiscordPresenceConfig
import dev.sonata.android.discord.DiscordPresenceTrack
import dev.sonata.android.discord.DiscordRpcClient
import dev.sonata.android.discord.DiscordRpcStatus
import dev.sonata.android.discord.extractTokenFromJsAlert
import dev.sonata.android.discord.isDiscordAuthenticatedUrl
import dev.sonata.android.discord.sanitizeDiscordUserAgent
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.RussianSonataStrings
import dev.sonata.android.ui.i18n.TurkishSonataStrings
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DiscordRpcTest {

    @Test
    fun discordApplicationValidatesAndResolvesAppIdsAndAssets() {
        assertEquals("1520387624861438002", DiscordApplication.DEFAULT_APP_ID)
        assertTrue(DiscordApplication.isValidSnowflakeAppId("1520387624861438002"))
        assertTrue(DiscordApplication.isValidSnowflakeAppId("  123456789012345678  "))
        assertFalse(DiscordApplication.isValidSnowflakeAppId(""))
        assertFalse(DiscordApplication.isValidSnowflakeAppId("12345"))
        assertFalse(DiscordApplication.isValidSnowflakeAppId("123456789012345abc"))

        assertEquals(DiscordApplication.DEFAULT_APP_ID, DiscordApplication.resolveAppId(null))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, DiscordApplication.resolveAppId("invalid"))
        assertEquals("987654321098765432", DiscordApplication.resolveAppId(" 987654321098765432 "))

        assertEquals(
            "https://discord.com/api/v9/applications/987654321098765432/external-assets",
            DiscordApplication.externalAssetsUrl("987654321098765432"),
        )
        assertEquals(
            "mp:external/abc/https/lh3.googleusercontent.com/cover.jpg",
            DiscordApplication.normalizeExternalAssetImage("external/abc/https/lh3.googleusercontent.com/cover.jpg"),
        )
        assertEquals(
            "mp:external/abc/https/lh3.googleusercontent.com/cover.jpg",
            DiscordApplication.normalizeExternalAssetImage("mp:external/abc/https/lh3.googleusercontent.com/cover.jpg"),
        )
        assertEquals("", DiscordApplication.normalizeExternalAssetImage("   "))

        val redacted = DiscordApplication.redactSensitiveText(
            message = "Authorization: secret_tok_12345 failed for \"token\":\"secret_tok_12345\"",
            token = "secret_tok_12345",
        )
        assertFalse(redacted.contains("secret_tok_12345"))
        assertTrue(redacted.contains("[REDACTED]"))
    }

    @Test
    fun loginHelpersSanitizeUserAgentCheckUrlsAndExtractTokenSafely() {
        val rawUa =
            "Mozilla/5.0 (Linux; Android 15; Pixel 9; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/131.0.0.0 Mobile Safari/537.36"
        val cleanUa = sanitizeDiscordUserAgent(rawUa)
        assertFalse(cleanUa.contains("; wv"))
        assertFalse(cleanUa.contains("Version/"))
        assertTrue(cleanUa.contains("Chrome/131.0.0.0"))
        assertTrue(cleanUa.contains("Pixel 9)"))

        assertFalse(isDiscordAuthenticatedUrl(null))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/login"))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/login?redirect_to=%2Fchannels%2F%40me"))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/register"))
        assertFalse(isDiscordAuthenticatedUrl("https://evil.example.com/channels/@me"))
        assertTrue(isDiscordAuthenticatedUrl("https://discord.com/channels/@me"))
        assertTrue(isDiscordAuthenticatedUrl("https://discord.com/app"))
        assertTrue(isDiscordAuthenticatedUrl("https://ptb.discord.com/channels/@me"))

        assertNull(extractTokenFromJsAlert(null))
        assertNull(extractTokenFromJsAlert("Hello world"))
        assertNull(extractTokenFromJsAlert("${DISCORD_TOKEN_ALERT_PREFIX}null"))
        assertNull(extractTokenFromJsAlert("${DISCORD_TOKEN_ALERT_PREFIX}\"undefined\""))
        assertNull(extractTokenFromJsAlert("${DISCORD_TOKEN_ALERT_PREFIX}short"))
        assertNull(extractTokenFromJsAlert("${DISCORD_TOKEN_ALERT_PREFIX}has space inside token value"))
        assertEquals(
            "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456",
            extractTokenFromJsAlert("$DISCORD_TOKEN_ALERT_PREFIX\"mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456\""),
        )
    }

    @Test
    fun gatewayIdentifyHeartbeatAndResumePayloadsMatchProtocol() {
        val identify = DiscordRpcClient.buildIdentifyPayload("unit_test_token_value", "en-US")
        assertEquals(DiscordRpcClient.OP_IDENTIFY, identify.getInt("op"))
        val d = identify.getJSONObject("d")
        assertEquals("unit_test_token_value", d.getString("token"))
        val props = d.getJSONObject("properties")
        assertEquals("Android", props.getString("os"))
        assertEquals("Discord Android", props.getString("browser"))
        assertEquals("en-US", props.getString("system_locale"))

        val hbNull = DiscordRpcClient.buildHeartbeatPayload(null)
        assertEquals(DiscordRpcClient.OP_HEARTBEAT, hbNull.getInt("op"))
        assertTrue(hbNull.isNull("d"))

        val hbSeq = DiscordRpcClient.buildHeartbeatPayload(42)
        assertEquals(DiscordRpcClient.OP_HEARTBEAT, hbSeq.getInt("op"))
        assertEquals(42, hbSeq.getInt("d"))

        val resume = DiscordRpcClient.buildResumePayload("unit_test_token_value", "sess_99", 42)
        assertEquals(DiscordRpcClient.OP_RESUME, resume.getInt("op"))
        val rd = resume.getJSONObject("d")
        assertEquals("unit_test_token_value", rd.getString("token"))
        assertEquals("sess_99", rd.getString("session_id"))
        assertEquals(42, rd.getInt("seq"))
    }

    @Test
    fun timestampsScaleWithPlaybackSpeedAndPresenceClearsWhenPaused() {
        val nowMs = 1_700_000_000_000L

        // Normal speed (1.0x): 30s in, 180s total -> start = now - 30s, end = now + 150s
        val (s1, e1) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 1.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 30_000L, s1)
        assertEquals(nowMs + 150_000L, e1)

        // Double speed (2.0x): 30s in took 15s wall-clock, 150s left will take 75s wall-clock
        val (s2, e2) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 2.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 15_000L, s2)
        assertEquals(nowMs + 75_000L, e2)

        // Half speed (0.5x): 30s in took 60s wall-clock, 150s left will take 300s wall-clock
        val (sHalf, eHalf) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 0.5f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 60_000L, sHalf)
        assertEquals(nowMs + 300_000L, eHalf)

        // Unknown duration omits end timestamp
        val (sUnknown, eUnknown) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 10_000L,
            durationMs = 0L,
            playbackSpeed = 1.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 10_000L, sUnknown)
        assertNull(eUnknown)

        val config = DiscordPresenceConfig(
            enabled = true,
            appId = "1520387624861438002",
            activityType = DiscordActivityType.LISTENING,
            showTimestamps = true,
            showArtwork = true,
            showButtons = true,
        )
        val playingTrack = DiscordPresenceTrack(
            id = "dQw4w9WgXcQ",
            title = "Midnight City",
            artist = "M83",
            album = "Hurry Up, We're Dreaming",
            durationMs = 240_000L,
            artworkUrl = "https://lh3.googleusercontent.com/art=w544-h544",
            isPlaying = true,
            positionMs = 60_000L,
            playbackSpeed = 1.25f,
        )

        val playingPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack,
            config = config,
            largeImageAsset = "mp:external/hash123/https/lh3.googleusercontent.com/art",
            nowMs = nowMs,
        )
        assertEquals(DiscordRpcClient.OP_PRESENCE_UPDATE, playingPayload.getInt("op"))
        val activities = playingPayload.getJSONObject("d").getJSONArray("activities")
        assertEquals(1, activities.length())
        val act = activities.getJSONObject(0)
        assertEquals(2, act.getInt("type"))
        assertEquals("Sonata", act.getString("name"))
        assertEquals("1520387624861438002", act.getString("application_id"))
        assertEquals("Midnight City · 1.25x", act.getString("details"))
        assertEquals("M83", act.getString("state"))
        assertEquals(
            "mp:external/hash123/https/lh3.googleusercontent.com/art",
            act.getJSONObject("assets").getString("large_image"),
        )
        assertEquals("Listen on Sonata", act.getJSONArray("buttons").getString(0))
        assertEquals(
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
            act.getJSONObject("metadata").getJSONArray("button_urls").getString(0),
        )
        val ts = act.getJSONObject("timestamps")
        assertEquals(nowMs - 48_000L, ts.getLong("start"))
        assertEquals(nowMs + 144_000L, ts.getLong("end"))

        // Paused track emits empty activities array (clean clear/idle)
        val pausedPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack.copy(isPlaying = false),
            config = config,
            largeImageAsset = "mp:external/hash123/https/lh3.googleusercontent.com/art",
            nowMs = nowMs,
        )
        assertEquals(0, pausedPayload.getJSONObject("d").getJSONArray("activities").length())

        // Disabled RPC emits empty activities array
        val disabledPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack,
            config = config.copy(enabled = false),
            largeImageAsset = null,
            nowMs = nowMs,
        )
        assertEquals(0, disabledPayload.getJSONObject("d").getJSONArray("activities").length())

        // JioSaavn IDs fall back to the Sonata project URL instead of invalid YouTube watch links
        val jioPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack.copy(id = "jiosaavn:12345"),
            config = config,
            largeImageAsset = null,
            nowMs = nowMs,
        )
        val jioAct = jioPayload.getJSONObject("d").getJSONArray("activities").getJSONObject(0)
        assertEquals("Listen on Sonata", jioAct.getJSONArray("buttons").getString(0))
        assertEquals(
            DiscordApplication.PROJECT_URL,
            jioAct.getJSONObject("metadata").getJSONArray("button_urls").getString(0),
        )

        // Disabling buttons omits both buttons and metadata
        val noButtonsPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack,
            config = config.copy(showButtons = false),
            largeImageAsset = null,
            nowMs = nowMs,
        )
        val noButtonsAct = noButtonsPayload.getJSONObject("d").getJSONArray("activities").getJSONObject(0)
        assertFalse(noButtonsAct.has("buttons"))
        assertFalse(noButtonsAct.has("metadata"))
    }

    @Test
    fun preferencesAndBackupExcludeSecretsAndSanitizeDiscordConfig() {
        val defaults = SonataSettingsState()
        assertFalse(defaults.discordRpcEnabled)
        assertEquals(DiscordApplication.DEFAULT_APP_ID, defaults.discordAppId)
        assertEquals(DiscordActivityType.LISTENING, defaults.discordActivityType)
        assertTrue(defaults.discordShowTimestamps)
        assertTrue(defaults.discordShowArtwork)
        assertTrue(defaults.discordShowButtons)

        val dirty = defaults.copy(discordAppId = "not_a_snowflake")
        assertEquals(DiscordApplication.DEFAULT_APP_ID, SonataPreferences.sanitize(dirty).discordAppId)

        val custom = defaults.copy(
            discordRpcEnabled = true,
            discordAppId = "123456789012345678",
            discordActivityType = DiscordActivityType.PLAYING,
            discordShowTimestamps = false,
            discordShowArtwork = false,
            discordShowButtons = false,
        )
        assertEquals("123456789012345678", SonataPreferences.sanitize(custom).discordAppId)

        val backupJson = SonataPreferences.formatBackupJson(custom, emptyMap(), 1_700_000_000_000L)
        val lowered = backupJson.lowercase()
        assertFalse(lowered.contains("token"))
        assertFalse(lowered.contains("secret"))
        assertFalse(lowered.contains("authorization"))

        val parsedSettings = JSONObject(backupJson).getJSONObject("settings")
        assertTrue(parsedSettings.getBoolean("discord_rpc_enabled"))
        assertEquals("123456789012345678", parsedSettings.getString("discord_app_id"))
        assertEquals("PLAYING", parsedSettings.getString("discord_activity_type"))
        assertFalse(parsedSettings.getBoolean("discord_show_timestamps"))
        assertFalse(parsedSettings.getBoolean("discord_show_artwork"))
        assertFalse(parsedSettings.getBoolean("discord_show_buttons"))
    }

    @Test
    fun discordClientLifecycleAndLocalizedStringsAreComplete() {
        val client = DiscordRpcClient()
        assertEquals(DiscordRpcStatus.Disconnected, client.status.value)
        client.updateAuthAndConfig(null, DiscordPresenceConfig(enabled = true))
        assertEquals(DiscordRpcStatus.Disconnected, client.status.value)
        client.updatePresence(null)
        client.disconnect()
        assertEquals(DiscordRpcStatus.Disconnected, client.status.value)

        for (strings in listOf(EnglishSonataStrings, TurkishSonataStrings, RussianSonataStrings)) {
            assertTrue(strings.catDiscord.isNotBlank())
            assertTrue(strings.discordConsentTitle.isNotBlank())
            assertTrue(strings.discordConsentBody.isNotBlank())
            assertTrue(strings.discordSignInTitle.isNotBlank())
            assertTrue(strings.discordSignOutAction.isNotBlank())
            assertTrue(strings.discordEnableRpc.isNotBlank())
            assertTrue(strings.discordAppIdInvalid.isNotBlank())
            assertNotNull(strings.discordStatusConnectedAs("tester"))
        }
    }
}
