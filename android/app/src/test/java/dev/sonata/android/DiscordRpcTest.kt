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
import dev.sonata.android.discord.DiscordTokenValidationResult
import dev.sonata.android.discord.extractCookieNames
import dev.sonata.android.discord.extractTokenFromJsAlert
import dev.sonata.android.discord.extractTokenFromJsEvalResult
import dev.sonata.android.discord.isAllowedDiscordApiUrl
import dev.sonata.android.discord.isDiscordAuthenticatedUrl
import dev.sonata.android.discord.sanitizeDiscordUserAgent
import dev.sonata.android.discord.shouldAllowDiscordWebViewRequest
import dev.sonata.android.discord.validateDiscordUserToken
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.RussianSonataStrings
import dev.sonata.android.ui.i18n.TurkishSonataStrings
import dev.sonata.android.ui.screens.settings.discordStatusLabel
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DiscordRpcTest {

    @Test
    fun discordApplicationLocksImmutableAppIdAndNormalizesAssetsAndTokens() {
        assertEquals("1520387624861438002", DiscordApplication.DEFAULT_APP_ID)
        assertTrue(DiscordApplication.isValidSnowflakeAppId("1520387624861438002"))
        assertTrue(DiscordApplication.isValidSnowflakeAppId("  123456789012345678  "))
        assertFalse(DiscordApplication.isValidSnowflakeAppId(""))
        assertFalse(DiscordApplication.isValidSnowflakeAppId("12345"))
        assertFalse(DiscordApplication.isValidSnowflakeAppId("123456789012345abc"))

        // resolveAppId is immutable and always returns DEFAULT_APP_ID even for valid custom snowflakes
        assertEquals(DiscordApplication.DEFAULT_APP_ID, DiscordApplication.resolveAppId(null))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, DiscordApplication.resolveAppId("invalid"))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, DiscordApplication.resolveAppId(" 987654321098765432 "))

        assertEquals(
            "https://discord.com/api/v9/applications/1520387624861438002/external-assets",
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
        assertEquals(
            "mp:attachments/123/456/cover.png",
            DiscordApplication.normalizeExternalAssetImage("https://media.discordapp.net/attachments/123/456/cover.png"),
        )
        assertEquals("", DiscordApplication.normalizeExternalAssetImage("   "))

        assertNull(DiscordApplication.sanitizeToken(null))
        assertNull(DiscordApplication.sanitizeToken("   "))
        assertNull(DiscordApplication.sanitizeToken("null"))
        assertNull(DiscordApplication.sanitizeToken("\"undefined\""))
        assertNull(DiscordApplication.sanitizeToken("short"))
        assertNull(DiscordApplication.sanitizeToken("Bot AbCdEfGhIjKlMnOpQrStUvWxYz123456"))
        assertNull(DiscordApplication.sanitizeToken("Bearer AbCdEfGhIjKlMnOpQrStUvWxYz123456"))
        assertNull(DiscordApplication.sanitizeToken("has space inside token value"))
        assertEquals(
            "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456",
            DiscordApplication.sanitizeToken("  \"'mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456'\"  "),
        )

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
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/"))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/login"))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/login?redirect_to=%2Fchannels%2F%40me"))
        assertFalse(isDiscordAuthenticatedUrl("https://discord.com/register"))
        assertFalse(isDiscordAuthenticatedUrl("https://evil.example.com/channels/@me"))
        assertTrue(isDiscordAuthenticatedUrl("https://discord.com/channels/@me"))
        assertTrue(isDiscordAuthenticatedUrl("https://discord.com/app"))
        assertTrue(isDiscordAuthenticatedUrl("https://ptb.discord.com/channels/@me"))

        assertTrue(isAllowedDiscordApiUrl("https://discord.com/api/v9/users/@me"))
        assertTrue(isAllowedDiscordApiUrl("https://ptb.discord.com/api/v9/science"))
        assertFalse(isAllowedDiscordApiUrl("https://evil.example.com/api/v9/users/@me"))
        assertFalse(isAllowedDiscordApiUrl("https://discord.com/channels/@me"))

        assertTrue(shouldAllowDiscordWebViewRequest("https://discord.com/login", isForMainFrame = true))
        assertFalse(shouldAllowDiscordWebViewRequest("discord://-/channels/@me", isForMainFrame = true))
        assertFalse(shouldAllowDiscordWebViewRequest("intent://discord.com", isForMainFrame = false))
        assertFalse(shouldAllowDiscordWebViewRequest("https://evil.example.com/", isForMainFrame = true))
        assertTrue(shouldAllowDiscordWebViewRequest("https://newassets.hcaptcha.com/captcha/v1/frame", isForMainFrame = false))

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

        assertNull(extractTokenFromJsEvalResult(null))
        assertNull(extractTokenFromJsEvalResult("null"))
        assertNull(extractTokenFromJsEvalResult("\"null\""))
        assertEquals(
            "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456",
            extractTokenFromJsEvalResult("\"\\\"mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456\\\"\""),
        )
        assertEquals(
            "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456",
            extractTokenFromJsEvalResult("\"mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456\""),
        )
    }

    @Test
    fun validateDiscordUserTokenHandlesSuccessAuthFailureAndTransientErrors() {
        var statusCode = 200
        var responseBody = """{"id":"123456789012345678","username":"test_user","global_name":"Test User"}"""
        var seenAuthHeader: String? = null
        var seenUserAgent: String? = null
        val client = OkHttpClient.Builder()
            .addInterceptor { chain ->
                seenAuthHeader = chain.request().header("Authorization")
                seenUserAgent = chain.request().header("User-Agent")
                Response.Builder()
                    .request(chain.request())
                    .protocol(Protocol.HTTP_1_1)
                    .code(statusCode)
                    .message("OK")
                    .body(responseBody.toResponseBody("application/json".toMediaType()))
                    .build()
            }
            .build()

        val validResult = validateDiscordUserToken(client, "\"mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456\"")
        assertEquals(
            DiscordTokenValidationResult.Valid(userId = "123456789012345678", username = "Test User"),
            validResult,
        )
        assertEquals("mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456", seenAuthHeader)
        assertEquals(DiscordApplication.GATEWAY_USER_AGENT, seenUserAgent)

        statusCode = 401
        responseBody = """{"message":"401: Unauthorized"}"""
        assertEquals(
            DiscordTokenValidationResult.Unauthorized,
            validateDiscordUserToken(client, "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456"),
        )

        statusCode = 503
        responseBody = """{}"""
        assertEquals(
            DiscordTokenValidationResult.TransientFailure,
            validateDiscordUserToken(client, "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456"),
        )

        assertEquals(
            DiscordTokenValidationResult.Unauthorized,
            validateDiscordUserToken(client, "short"),
        )
    }

    @Test
    fun gatewayIdentifyHeartbeatAndResumePayloadsMatchProtocol() {
        val identify = DiscordRpcClient.buildIdentifyPayload("unit_test_token_value", "en-US")
        assertEquals(DiscordRpcClient.OP_IDENTIFY, identify.getInt("op"))
        val d = identify.getJSONObject("d")
        assertEquals("unit_test_token_value", d.getString("token"))
        assertEquals(65, d.getInt("capabilities"))
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

        val (s1, e1) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 1.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 30_000L, s1)
        assertEquals(nowMs + 150_000L, e1)

        val (s2, e2) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 2.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 15_000L, s2)
        assertEquals(nowMs + 75_000L, e2)

        val (sHalf, eHalf) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 30_000L,
            durationMs = 180_000L,
            playbackSpeed = 0.5f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 60_000L, sHalf)
        assertEquals(nowMs + 300_000L, eHalf)

        val (sUnknown, eUnknown) = DiscordRpcClient.computeAdjustedTimestamps(
            positionMs = 10_000L,
            durationMs = 0L,
            playbackSpeed = 1.0f,
            nowMs = nowMs,
        )
        assertEquals(nowMs - 10_000L, sUnknown)
        assertNull(eUnknown)

        // Even if caller supplies a custom appId in config, payload MUST lock to DEFAULT_APP_ID
        val config = DiscordPresenceConfig(
            enabled = true,
            appId = "999999999999999999",
            activityType = DiscordActivityType.LISTENING,
            showTimestamps = true,
            showArtwork = true,
            showButtons = true,
        )
        val playingTrack = DiscordPresenceTrack(
            id = "test_track_1",
            title = "Test Track One",
            artist = "Test Artist",
            album = "Test Album",
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
        val dObj = playingPayload.getJSONObject("d")
        assertEquals(0, dObj.getInt("since"))
        assertEquals("online", dObj.getString("status"))
        val activities = dObj.getJSONArray("activities")
        assertEquals(1, activities.length())
        val act = activities.getJSONObject(0)
        assertEquals(2, act.getInt("type"))
        assertEquals("Sonata", act.getString("name"))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, act.getString("application_id"))
        assertEquals("Test Track One · 1.25x", act.getString("details"))
        assertEquals("Test Artist", act.getString("state"))
        assertEquals(
            "mp:external/hash123/https/lh3.googleusercontent.com/art",
            act.getJSONObject("assets").getString("large_image"),
        )
        assertEquals("Listen on Sonata", act.getJSONArray("buttons").getString(0))
        assertEquals(
            DiscordApplication.PROJECT_URL,
            act.getJSONObject("metadata").getJSONArray("button_urls").getString(0),
        )
        val ts = act.getJSONObject("timestamps")
        assertEquals(nowMs - 48_000L, ts.getLong("start"))
        assertEquals(nowMs + 144_000L, ts.getLong("end"))

        // Single-character track title/artist/album are padded to >= 2 chars so Discord accepts presence
        val shortTrackPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack.copy(title = "X", artist = "Y", album = "Z", playbackSpeed = 1.0f),
            config = config,
            largeImageAsset = "mp:external/hash123/art",
            nowMs = nowMs,
        )
        val shortAct = shortTrackPayload.getJSONObject("d").getJSONArray("activities").getJSONObject(0)
        assertTrue(shortAct.getString("details").length >= 2)
        assertTrue(shortAct.getString("state").length >= 2)
        assertTrue(shortAct.getJSONObject("assets").getString("large_text").length >= 2)

        // When external-assets lookup fails (largeImageAsset == null), presence still builds cleanly without assets
        val noAssetPayload = DiscordRpcClient.buildPresencePayload(
            track = playingTrack,
            config = config,
            largeImageAsset = null,
            nowMs = nowMs,
        )
        val noAssetAct = noAssetPayload.getJSONObject("d").getJSONArray("activities").getJSONObject(0)
        assertFalse(noAssetAct.has("assets"))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, noAssetAct.getString("application_id"))

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
    fun preferencesAndBackupExcludeSecretsAndLockDiscordAppId() {
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
        // Custom Discord Application ID overrides are always rejected and locked to DEFAULT_APP_ID
        assertEquals(DiscordApplication.DEFAULT_APP_ID, SonataPreferences.sanitize(custom).discordAppId)

        val backupJson = SonataPreferences.formatBackupJson(custom, emptyMap(), 1_700_000_000_000L)
        val lowered = backupJson.lowercase()
        assertFalse(lowered.contains("token"))
        assertFalse(lowered.contains("secret"))
        assertFalse(lowered.contains("authorization"))

        val parsedSettings = JSONObject(backupJson).getJSONObject("settings")
        assertTrue(parsedSettings.getBoolean("discord_rpc_enabled"))
        assertEquals(DiscordApplication.DEFAULT_APP_ID, parsedSettings.getString("discord_app_id"))
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
            assertTrue(strings.discordSummary.isNotBlank())
            assertTrue(strings.discordConsentTitle.isNotBlank())
            assertTrue(strings.discordConsentBody.isNotBlank())
            assertTrue(strings.discordSignInTitle.isNotBlank())
            assertTrue(strings.discordSignInVerifying.isNotBlank())
            assertTrue(strings.discordSignInVerifyFailed.isNotBlank())
            assertTrue(strings.discordSignOutAction.isNotBlank())
            assertTrue(strings.discordEnableRpc.isNotBlank())
            assertTrue(strings.discordStatusConnectionFailed.isNotBlank())
            assertTrue(strings.discordWebViewRendererGoneTitle.isNotBlank())
            assertTrue(strings.discordWebViewRendererGoneBody.isNotBlank())
            assertTrue(strings.discordWebViewUnavailableTitle.isNotBlank())
            assertTrue(strings.discordWebViewUnavailableBody.isNotBlank())
            assertNotNull(strings.discordStatusConnectedAs("tester"))

            // Blank error message uses localized fallback instead of hardcoded English text
            assertEquals(
                strings.discordStatusConnectionFailed,
                discordStatusLabel(DiscordRpcStatus.Error(""), strings),
            )
            assertEquals(
                strings.discordStatusError("HTTP 502"),
                discordStatusLabel(DiscordRpcStatus.Error("HTTP 502"), strings),
            )
        }
    }

    @Test
    fun extractCookieNamesParsesOnlyCookieNamesSafely() {
        assertEquals(emptyList<String>(), extractCookieNames(null))
        assertEquals(emptyList<String>(), extractCookieNames("   "))
        assertEquals(
            listOf("__dcfduid", "__sdcfduid", "locale"),
            extractCookieNames("__dcfduid=abc123; __sdcfduid=xyz=99; locale=en-US; __dcfduid=dup"),
        )
    }

    @Test
    fun externalAssetLookupHandlesSuccessNon200FallbackAnd429RetryAfter() {
        var nowMs = 1_000_000L
        var responseCode = 200
        var responseBody = """[{"external_asset_path":"external/hash/https/example.com/cover.jpg"}]"""
        var retryAfterHeader: String? = null
        var seenUrl: String? = null
        var seenUserAgent: String? = null

        val client = OkHttpClient.Builder()
            .addInterceptor { chain ->
                seenUrl = chain.request().url.toString()
                seenUserAgent = chain.request().header("User-Agent")
                val builder = Response.Builder()
                    .request(chain.request())
                    .protocol(Protocol.HTTP_1_1)
                    .code(responseCode)
                    .message("Mock")
                    .body(responseBody.toResponseBody("application/json".toMediaType()))
                retryAfterHeader?.let { builder.header("Retry-After", it) }
                builder.build()
            }
            .build()

        val rpc = DiscordRpcClient(
            httpClient = client,
            nowProviderMs = { nowMs },
        )

        // Success normalizes external_asset_path and always hits immutable DEFAULT_APP_ID endpoint
        val resolved = rpc.fetchAndRecordExternalAssetForTest("tok", "999999999999999999", "https://example.com/ok.jpg")
        assertEquals("mp:external/hash/https/example.com/cover.jpg", resolved)
        assertEquals(DiscordApplication.externalAssetsUrl(), seenUrl)
        assertEquals(DiscordApplication.GATEWAY_USER_AGENT, seenUserAgent)

        // Non-200 failure returns null cleanly and records default cooldown (no crash)
        responseCode = 403
        responseBody = """{"message":"Forbidden"}"""
        val artFail = "https://example.com/fail.jpg"
        assertNull(rpc.fetchAndRecordExternalAssetForTest("tok", DiscordApplication.DEFAULT_APP_ID, artFail))
        assertEquals(nowMs + 60_000L, rpc.assetCooldownUntilMsForTest(DiscordApplication.DEFAULT_APP_ID, artFail))

        // 429 preserves explicit Retry-After (long and short)
        responseCode = 429
        responseBody = "{}"
        retryAfterHeader = "180"
        val artLong = "https://example.com/cover-long.jpg"
        assertNull(rpc.fetchAndRecordExternalAssetForTest("tok", DiscordApplication.DEFAULT_APP_ID, artLong))
        assertEquals(nowMs + 180_000L, rpc.assetCooldownUntilMsForTest(DiscordApplication.DEFAULT_APP_ID, artLong))

        retryAfterHeader = "15"
        val artShort = "https://example.com/cover-short.jpg"
        assertNull(rpc.fetchAndRecordExternalAssetForTest("tok", DiscordApplication.DEFAULT_APP_ID, artShort))
        assertEquals(nowMs + 15_000L, rpc.assetCooldownUntilMsForTest(DiscordApplication.DEFAULT_APP_ID, artShort))

        // Direct Discord asset URLs resolve without proxy calls
        assertEquals(
            "mp:external/direct/art.png",
            DiscordRpcClient.resolveDirectDiscordAsset("mp:external/direct/art.png"),
        )
        assertEquals(
            "mp:attachments/1/2/art.png",
            DiscordRpcClient.resolveDirectDiscordAsset("https://media.discordapp.net/attachments/1/2/art.png"),
        )
        assertNull(DiscordRpcClient.resolveDirectDiscordAsset("https://lh3.googleusercontent.com/art"))

        rpc.disconnect()
    }

    @Test
    fun `shouldAllowDiscordWebViewRequest permits safe subframe schemes and localizes load error strings`() {
        assertTrue(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("https://discord.com/login", true))
        assertFalse(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("about:blank", true))
        assertTrue(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("about:blank", false))
        assertTrue(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("blob:https://discord.com/123", false))
        assertTrue(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("data:text/html,ok", false))
        assertFalse(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("discord://discord.com", false))
        assertFalse(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("intent://login", false))
        assertFalse(dev.sonata.android.discord.shouldAllowDiscordWebViewRequest("file:///etc/passwd", false))

        listOf(
            dev.sonata.android.ui.i18n.EnglishSonataStrings,
            dev.sonata.android.ui.i18n.TurkishSonataStrings,
            dev.sonata.android.ui.i18n.RussianSonataStrings,
        ).forEach { s ->
            assertTrue(s.discordWebViewLoadErrorTitle.isNotBlank())
            assertTrue(s.discordWebViewLoadErrorBody.isNotBlank())
        }
    }
}
