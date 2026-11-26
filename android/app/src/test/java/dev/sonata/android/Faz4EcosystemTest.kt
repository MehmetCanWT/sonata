package dev.sonata.android

import android.content.Context
import android.content.ContextWrapper
import android.content.SharedPreferences
import dev.sonata.android.connect.CONNECT_DISCOVERY_PORT
import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectClientException
import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.ConnectDeviceInfo
import dev.sonata.android.connect.ConnectError
import dev.sonata.android.connect.ConnectStateSnapshot
import dev.sonata.android.connect.ConnectTrackSnapshot
import dev.sonata.android.connect.PairAck
import dev.sonata.android.connect.PairApprovalInitAck
import dev.sonata.android.connect.PairApprovalRequest
import dev.sonata.android.connect.PairApprovalStatus
import dev.sonata.android.connect.PairRequest
import dev.sonata.android.connect.PairStatusAck
import dev.sonata.android.connect.SonataConnectClient
import dev.sonata.android.connect.SonataConnectCrypto
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioPipelineBus
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.playback.SonataAutoBrowser
import dev.sonata.android.ui.i18n.EnglishEcosystemStrings
import dev.sonata.android.ui.i18n.RussianEcosystemStrings
import dev.sonata.android.ui.i18n.TurkishEcosystemStrings
import dev.sonata.android.ui.i18n.connectErrorMessage
import dev.sonata.android.ui.i18n.ecosystemStrings
import kotlinx.coroutines.test.runTest
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.ServerSocket
import java.nio.charset.StandardCharsets
import java.util.Locale
import kotlin.concurrent.thread

class Faz4EcosystemTest {

    private class InMemorySharedPreferences : SharedPreferences {
        private val data = LinkedHashMap<String, Any?>()

        override fun getAll(): MutableMap<String, *> = HashMap(data)
        override fun getString(key: String?, defValue: String?): String? =
            (data[key] as? String) ?: defValue

        @Suppress("UNCHECKED_CAST")
        override fun getStringSet(key: String?, defValues: MutableSet<String>?): MutableSet<String>? =
            (data[key] as? Set<String>)?.toMutableSet() ?: defValues

        override fun getInt(key: String?, defValue: Int): Int =
            (data[key] as? Int) ?: defValue

        override fun getLong(key: String?, defValue: Long): Long =
            (data[key] as? Long) ?: defValue

        override fun getFloat(key: String?, defValue: Float): Float =
            (data[key] as? Float) ?: defValue

        override fun getBoolean(key: String?, defValue: Boolean): Boolean =
            (data[key] as? Boolean) ?: defValue

        override fun contains(key: String?): Boolean = data.containsKey(key)

        override fun edit(): SharedPreferences.Editor = object : SharedPreferences.Editor {
            private val pending = LinkedHashMap<String, Any?>()
            private val removals = HashSet<String>()
            private var clearAll = false

            override fun putString(key: String?, value: String?): SharedPreferences.Editor = apply {
                if (key != null) {
                    if (value == null) removals.add(key) else pending[key] = value
                }
            }

            override fun putStringSet(key: String?, values: MutableSet<String>?): SharedPreferences.Editor = apply {
                if (key != null) {
                    if (values == null) removals.add(key) else pending[key] = values.toSet()
                }
            }

            override fun putInt(key: String?, value: Int): SharedPreferences.Editor = apply {
                if (key != null) pending[key] = value
            }

            override fun putLong(key: String?, value: Long): SharedPreferences.Editor = apply {
                if (key != null) pending[key] = value
            }

            override fun putFloat(key: String?, value: Float): SharedPreferences.Editor = apply {
                if (key != null) pending[key] = value
            }

            override fun putBoolean(key: String?, value: Boolean): SharedPreferences.Editor = apply {
                if (key != null) pending[key] = value
            }

            override fun remove(key: String?): SharedPreferences.Editor = apply {
                if (key != null) removals.add(key)
            }

            override fun clear(): SharedPreferences.Editor = apply {
                clearAll = true
            }

            override fun commit(): Boolean {
                apply()
                return true
            }

            override fun apply() {
                if (clearAll) data.clear()
                removals.forEach { data.remove(it) }
                data.putAll(pending)
            }
        }

        override fun registerOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?,
        ) = Unit

        override fun unregisterOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?,
        ) = Unit
    }

    private class FakePreferencesContext : ContextWrapper(null) {
        private val stores = HashMap<String, InMemorySharedPreferences>()

        override fun getApplicationContext(): Context = this

        override fun getSharedPreferences(name: String?, mode: Int): SharedPreferences =
            stores.getOrPut(name.orEmpty()) { InMemorySharedPreferences() }
    }

    private fun sampleTrack(
        id: String,
        title: String,
        artist: String = "Sonata Artist",
        album: String = "Sonata Album",
    ) = AndroidTrack(
        id = id,
        title = title,
        artist = artist,
        album = album,
        durationSecs = 210uL,
        thumbnailUrl = "https://example.com/art/$id.jpg",
        liked = id.startsWith("liked"),
        albumId = "alb_$id",
        isExplicit = false,
    )

    @Test
    fun `crossfade clamping and equal-power curve behave accurately and bypass video mode`() {
        assertEquals(0, clampCrossfadeSeconds(-5))
        assertEquals(0, clampCrossfadeSeconds(0))
        assertEquals(6, clampCrossfadeSeconds(6))
        assertEquals(12, clampCrossfadeSeconds(12))
        assertEquals(12, clampCrossfadeSeconds(42))

        // Equal-power fade-out: 1.0 at start -> ~0.7071 at midpoint -> 0.0 at end
        assertEquals(1.0f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, elapsedMs = 0L, durationMs = 4_000L), 0.0001f)
        assertEquals(0.7071f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, elapsedMs = 2_000L, durationMs = 4_000L), 0.005f)
        assertEquals(0.0f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, elapsedMs = 4_000L, durationMs = 4_000L), 0.0001f)
        assertEquals(0.0f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, elapsedMs = 9_000L, durationMs = 4_000L), 0.0001f)

        // Equal-power fade-in: 0.0 at start -> ~0.7071 at midpoint -> 1.0 at end
        assertEquals(0.0f, computeCrossfadeGain(CrossfadePhase.FADE_IN, elapsedMs = 0L, durationMs = 4_000L), 0.0001f)
        assertEquals(0.7071f, computeCrossfadeGain(CrossfadePhase.FADE_IN, elapsedMs = 2_000L, durationMs = 4_000L), 0.005f)
        assertEquals(1.0f, computeCrossfadeGain(CrossfadePhase.FADE_IN, elapsedMs = 4_000L, durationMs = 4_000L), 0.0001f)

        // Constant power sum (fadeOut^2 + fadeIn^2 == 1.0)
        val outMid = computeCrossfadeGain(CrossfadePhase.FADE_OUT, 1_500L, 4_000L)
        val inMid = computeCrossfadeGain(CrossfadePhase.FADE_IN, 1_500L, 4_000L)
        assertEquals(1.0f, (outMid * outMid) + (inMid * inMid), 0.001f)

        // Disabled when duration <= 0 or during Music Video mode
        assertEquals(1.0f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, 2_000L, 0L), 0.0001f)
        assertEquals(1.0f, computeCrossfadeGain(CrossfadePhase.FADE_IN, 0L, 4_000L, videoModeActive = true), 0.0001f)
        assertEquals(1.0f, computeCrossfadeGain(CrossfadePhase.FADE_OUT, 3_500L, 4_000L, videoModeActive = true), 0.0001f)

        // Bridge clamping & AudioPipelineBus updates
        PlaybackAudioBridge.applyCrossfade(8)
        assertEquals(8, AudioPipelineBus.info.value.crossfadeSeconds)
        PlaybackAudioBridge.applyCrossfade(0)
        assertEquals(0, AudioPipelineBus.info.value.crossfadeSeconds)
        PlaybackAudioBridge.applyGapless(false)
        assertFalse(PlaybackAudioBridge.isGaplessEnabled())
        PlaybackAudioBridge.applyGapless(true)
        assertTrue(PlaybackAudioBridge.isGaplessEnabled())
    }

    @Test
    fun `preferences expose ecosystem flows, persist safely, and keep backup secret-free`() {
        val prefs = SonataPreferences(FakePreferencesContext())

        // Required defaults
        assertTrue(prefs.gaplessEnabled.value)
        assertEquals(0, prefs.crossfadeSeconds.value)
        assertFalse(prefs.connectEnabled.value)
        assertEquals("Sonata Android", prefs.connectDeviceName.value)
        assertFalse(prefs.lastFmEnabled.value)
        assertFalse(prefs.listenBrainzEnabled.value)

        prefs.setGaplessEnabled(false)
        prefs.setCrossfadeSeconds(99)
        assertEquals(12, prefs.crossfadeSeconds.value)
        prefs.setCrossfadeSeconds(0)
        assertEquals(0, prefs.crossfadeSeconds.value)
        prefs.setCrossfadeSeconds(5)
        assertEquals(5, prefs.crossfadeSeconds.value)

        prefs.setConnectEnabled(true)
        prefs.setConnectDeviceName("   ")
        assertEquals("Sonata Android", prefs.connectDeviceName.value)
        prefs.setConnectDeviceName("Pixel 9 Pro")
        prefs.setLastFmEnabled(true)
        prefs.setListenBrainzEnabled(true)
        prefs.setDiscordToken("secret-should-never-appear")

        assertFalse(prefs.gaplessEnabled.value)
        assertTrue(prefs.connectEnabled.value)
        assertEquals("Pixel 9 Pro", prefs.connectDeviceName.value)
        assertTrue(prefs.lastFmEnabled.value)
        assertTrue(prefs.listenBrainzEnabled.value)

        val backup = prefs.exportBackupJson()
        val lowered = backup.lowercase(Locale.ROOT)
        assertTrue(backup.contains("\"gapless_enabled\": false"))
        assertTrue(backup.contains("\"crossfade_enabled\": true"))
        assertTrue(backup.contains("\"crossfade_seconds\": 5"))
        assertTrue(backup.contains("\"connect_enabled\": true"))
        assertTrue(backup.contains("\"connect_device_name\": \"Pixel 9 Pro\""))
        assertTrue(backup.contains("\"lastfm_enabled\": true"))
        assertTrue(backup.contains("\"listenbrainz_enabled\": true"))
        assertFalse(lowered.contains("secret-should-never-appear"))
        assertFalse(lowered.contains("token"))
        assertFalse(lowered.contains("secret"))
        assertFalse(lowered.contains("cookie"))

        val restored = SonataPreferences(FakePreferencesContext())
        assertTrue(restored.importBackupJson(backup))
        assertFalse(restored.gaplessEnabled.value)
        assertEquals(5, restored.crossfadeSeconds.value)
        assertTrue(restored.connectEnabled.value)
        assertEquals("Pixel 9 Pro", restored.connectDeviceName.value)
        assertTrue(restored.lastFmEnabled.value)
        assertTrue(restored.listenBrainzEnabled.value)
        assertNull(restored.getDiscordToken())
        assertEquals(true, SonataSettingsState().gaplessEnabled)
    }

    @Test
    fun `auto browser exposes browseable root categories, playlist drilldown, getItem, and search`() {
        val liked = listOf(sampleTrack("liked_1", "Midnight Echo", "Luna"))
        val playlists = listOf(
            AndroidPlaylist("personal:p1", "Lossless Mix", null, isOwned = true),
            AndroidPlaylist("PL_ytm_2", "Roadtrip", null, isOwned = false),
        )
        val playlistTracks = mapOf(
            "personal:p1" to listOf(sampleTrack("local:7", "Flac Sunrise", "Ambient Crew")),
            "PL_ytm_2" to listOf(sampleTrack("yt_99", "Highway Beat", "SynthWave")),
        )
        val downloads = listOf(sampleTrack("dl_1", "Offline Rain", "Cloud"))
        val local = listOf(sampleTrack("local:7", "Flac Sunrise", "Ambient Crew"))
        val recent = listOf(sampleTrack("rec_1", "Late Night Jazz", "Miles"))

        val browser = SonataAutoBrowser(
            dataProvider = object : SonataAutoBrowser.AutoDataProvider {
                override fun getLikedTracks() = liked
                override fun getPlaylists() = playlists
                override fun getPlaylistTracks(playlistId: String) = playlistTracks[playlistId].orEmpty()
                override fun getDownloadedTracks() = downloads
                override fun getLocalTracks() = local
                override fun getRecentlyPlayedTracks() = recent
                override fun searchTracks(query: String) =
                    if (query.contains("Highway", ignoreCase = true)) playlistTracks["PL_ytm_2"].orEmpty() else emptyList()
            },
            stringsProvider = { EnglishEcosystemStrings },
        )

        val root = browser.getRootItem()
        assertEquals(SonataAutoBrowser.ROOT_ID, root.mediaId)

        val rootChildren = browser.getChildren(SonataAutoBrowser.ROOT_ID).orEmpty()
        assertEquals(
            listOf(
                SonataAutoBrowser.LIKED_ID,
                SonataAutoBrowser.PLAYLISTS_ID,
                SonataAutoBrowser.DOWNLOADS_ID,
                SonataAutoBrowser.LOCAL_ID,
                SonataAutoBrowser.RECENT_ID,
            ),
            rootChildren.map { it.mediaId },
        )

        assertEquals(listOf("liked_1"), browser.getChildren(SonataAutoBrowser.LIKED_ID)?.map { it.mediaId })
        assertEquals(
            listOf("sonata:playlist:personal:p1", "sonata:playlist:PL_ytm_2"),
            browser.getChildren(SonataAutoBrowser.PLAYLISTS_ID)?.map { it.mediaId },
        )
        assertEquals(listOf("local:7"), browser.getChildren("sonata:playlist:personal:p1")?.map { it.mediaId })
        assertEquals(listOf("dl_1"), browser.getChildren(SonataAutoBrowser.DOWNLOADS_ID)?.map { it.mediaId })
        assertEquals(listOf("local:7"), browser.getChildren(SonataAutoBrowser.LOCAL_ID)?.map { it.mediaId })
        assertEquals(listOf("rec_1"), browser.getChildren(SonataAutoBrowser.RECENT_ID)?.map { it.mediaId })
        assertNull(browser.getChildren("sonata:unknown"))

        // getItem resolves nodes, playlists, and tracks
        assertEquals(SonataAutoBrowser.LOCAL_ID, browser.getItem(SonataAutoBrowser.LOCAL_ID)?.mediaId)
        assertEquals("sonata:playlist:personal:p1", browser.getItem("sonata:playlist:personal:p1")?.mediaId)
        assertEquals("liked_1", browser.getItem("liked_1")?.mediaId)
        assertNull(browser.getItem("missing_id"))

        // Search combines provider search and local pools without duplicates
        assertEquals(listOf("local:7"), browser.search("Flac").map { it.mediaId })
        assertEquals(listOf("yt_99"), browser.search("Highway").map { it.mediaId })
        assertTrue(browser.search("   ").isEmpty())

        // PlaybackQueueBridge onPlayMediaId resets cleanly
        PlaybackQueueBridge.reset()
        var triggeredId: String? = null
        PlaybackQueueBridge.onPlayMediaId = { id -> triggeredId = id }
        assertTrue(PlaybackQueueBridge.hasActiveCallbacks())
        PlaybackQueueBridge.onPlayMediaId("liked_1")
        assertEquals("liked_1", triggeredId)
        PlaybackQueueBridge.reset()
        assertFalse(PlaybackQueueBridge.hasActiveCallbacks())
    }

    @Test
    fun `sonata connect signs HMAC-SHA1, roundtrips JSON payloads, and talks to HTTP endpoints`() = runTest {
        assertEquals(45389, CONNECT_DISCOVERY_PORT)
        assertTrue(SonataConnectCrypto.isValidPin("042918"))
        assertFalse(SonataConnectCrypto.isValidPin("12345"))
        assertFalse(SonataConnectCrypto.isValidPin("12a456"))

        val secret = "paired-shared-secret-key"
        val body = """{"command":"Play"}"""
        val sig = SonataConnectCrypto.signPayload(secret, body)
        assertEquals(40, sig.length)
        assertTrue(SonataConnectCrypto.verifySignature(secret, body, sig))
        assertTrue(SonataConnectCrypto.verifySignature(secret, body, sig.uppercase(Locale.ROOT)))
        assertFalse(SonataConnectCrypto.verifySignature(secret, """{"command":"Pause"}""", sig))
        assertFalse(SonataConnectCrypto.verifySignature("wrong-secret", body, sig))

        // Beacon JSON & discovery ingestion/pruning
        val beacon = ConnectBeacon(
            deviceId = "desktop-1",
            deviceName = "Sonata Desktop",
            platform = "linux",
            httpPort = 45390,
            protocolVersion = 1,
            hostAddress = "192.168.1.42",
        )
        val parsedBeacon = ConnectBeacon.fromJson(beacon.toJson(), hostAddress = "192.168.1.42")
        assertNotNull(parsedBeacon)
        assertEquals("http://192.168.1.42:45390", parsedBeacon!!.baseUrl())
        assertEquals("desktop-1", parsedBeacon.deviceId)

        // All 7 ConnectCommand variants roundtrip cleanly
        val track = ConnectTrackSnapshot("yt_1", "Song One", "Artist A", "Album X", 195L, null)
        val commands = listOf(
            ConnectCommand.Play,
            ConnectCommand.Pause,
            ConnectCommand.Next,
            ConnectCommand.Prev,
            ConnectCommand.SeekAbsolute(42_500L),
            ConnectCommand.SetVolume(0.75f),
            ConnectCommand.TransferQueue(listOf(track), startIndex = 0, positionMs = 12_000L, playWhenReady = true),
        )
        for (cmd in commands) {
            val decoded = ConnectCommand.fromJson(cmd.toJson())
            assertEquals(cmd, decoded)
        }

        // State snapshot roundtrip
        val snapshot = ConnectStateSnapshot(
            deviceId = "desktop-1",
            deviceName = "Sonata Desktop",
            isPlaying = true,
            positionMs = 15_000L,
            durationMs = 195_000L,
            volume = 0.8f,
            activeTrack = track,
            queue = listOf(track),
            queueIndex = 0,
            timestampMs = 1_700_000_000_000L,
        )
        val decodedSnapshot = ConnectStateSnapshot.fromJson(snapshot.toJson())
        assertEquals(snapshot, decodedSnapshot)

        // End-to-end loopback HTTP server testing /connect/info, /connect/pair, /connect/state, /connect/command
        var receivedCommandSig: String? = null
        var receivedCommandBody: String? = null
        val server = ServerSocket(0)
        val port = server.localPort
        val serverThread = thread(isDaemon = true) {
            repeat(4) {
                val socket = server.accept()
                socket.use { sock ->
                    val reader = BufferedReader(InputStreamReader(sock.getInputStream(), StandardCharsets.UTF_8))
                    val requestLine = reader.readLine().orEmpty()
                    val headers = HashMap<String, String>()
                    var contentLength = 0
                    while (true) {
                        val line = reader.readLine() ?: break
                        if (line.isEmpty()) break
                        val idx = line.indexOf(':')
                        if (idx > 0) {
                            val name = line.substring(0, idx).trim().lowercase(Locale.ROOT)
                            val value = line.substring(idx + 1).trim()
                            headers[name] = value
                            if (name == "content-length") {
                                contentLength = value.toIntOrNull() ?: 0
                            }
                        }
                    }
                    val reqBody = if (contentLength > 0) {
                        val chars = CharArray(contentLength)
                        var readTotal = 0
                        while (readTotal < contentLength) {
                            val n = reader.read(chars, readTotal, contentLength - readTotal)
                            if (n < 0) break
                            readTotal += n
                        }
                        String(chars, 0, readTotal)
                    } else {
                        ""
                    }

                    val respBody = when {
                        requestLine.startsWith("GET /connect/info") ->
                            ConnectDeviceInfo("desktop-1", "Sonata Desktop", "linux", "1.0.7", paired = false).toJson()
                        requestLine.startsWith("POST /connect/pair") -> {
                            val pairReq = PairRequest.fromJson(reqBody)
                            assertNotNull(pairReq)
                            assertEquals("654321", pairReq!!.pin)
                            PairAck(true, "desktop-1", "Sonata Desktop", sharedSecret = secret).toJson()
                        }
                        requestLine.startsWith("GET /connect/state") -> snapshot.toJson()
                        requestLine.startsWith("POST /connect/command") -> {
                            receivedCommandSig = headers[SonataConnectClient.HEADER_SIGNATURE.lowercase(Locale.ROOT)]
                            receivedCommandBody = reqBody
                            JSONObject().put("ok", true).toString()
                        }
                        else -> "{}"
                    }
                    val bytes = respBody.toByteArray(StandardCharsets.UTF_8)
                    val httpResp = "HTTP/1.1 200 OK\r\n" +
                        "Content-Type: application/json\r\n" +
                        "Content-Length: ${bytes.size}\r\n" +
                        "Connection: close\r\n\r\n"
                    val out = sock.getOutputStream()
                    out.write(httpResp.toByteArray(StandardCharsets.US_ASCII))
                    out.write(bytes)
                    out.flush()
                }
            }
        }

        try {
            val client = SonataConnectClient(localDeviceId = "android-test-1")
            // Ingest self vs peer beacon
            assertNull(client.ingestBeaconPacket(beacon.copy(deviceId = "android-test-1").toJson(), "127.0.0.1", 1_000L))
            assertNotNull(client.ingestBeaconPacket(beacon.toJson(), "127.0.0.1", 1_000L))
            assertEquals(1, client.discoveredDevices.value.size)
            client.pruneStaleBeacons(nowMs = 20_000L)
            assertTrue(client.discoveredDevices.value.isEmpty())

            val baseUrl = "http://127.0.0.1:$port"
            val info = client.fetchInfo(baseUrl).getOrThrow()
            assertEquals("Sonata Desktop", info.deviceName)

            val pairAck = client.pair(baseUrl, "Pixel Test", "654321").getOrThrow()
            assertTrue(pairAck.accepted)
            assertEquals(secret, pairAck.sharedSecret)

            val fetchedState = client.fetchState(baseUrl, secret).getOrThrow()
            assertEquals(snapshot.activeTrack?.id, fetchedState.activeTrack?.id)

            client.sendCommand(baseUrl, secret, ConnectCommand.SeekAbsolute(98_000L)).getOrThrow()
            assertNotNull(receivedCommandSig)
            assertNotNull(receivedCommandBody)
            assertTrue(SonataConnectCrypto.verifySignature(secret, receivedCommandBody!!, receivedCommandSig!!))
            assertEquals(ConnectCommand.SeekAbsolute(98_000L), ConnectCommand.fromJson(receivedCommandBody!!))

            // Verify canonical Rust wire format payloads emitted and parsed by Android client
            val pairReqJson = JSONObject(
                PairRequest("android-test-1", "Pixel Test", "654321").toJson(),
            )
            assertEquals("android-test-1", pairReqJson.getString("device_id"))
            assertEquals("Pixel Test", pairReqJson.getString("device_name"))
            assertEquals("654321", pairReqJson.getString("pin"))
            assertFalse(pairReqJson.has("client_device_id"))

            val rustPairAck = PairAck.fromJson(
                """{"accepted":true,"device_id":"sonata-linux","device_name":"Sonata Linux","token":"deadbeef","message":null}""",
            )
            assertNotNull(rustPairAck)
            assertTrue(rustPairAck!!.accepted)
            assertEquals("deadbeef", rustPairAck.sharedSecret)
            assertNull(rustPairAck.message)

            val rustState = ConnectStateSnapshot.fromJson(
                """
                {
                  "device_id": "sonata-linux",
                  "device_name": "Sonata Linux",
                  "playing": true,
                  "paused": false,
                  "track_id": "test-track-1",
                  "title": "Test Track",
                  "artist": "Test Artist",
                  "album": "Test Album",
                  "thumbnail_url": null,
                  "position_secs": 42.5,
                  "duration_secs": 180.0,
                  "volume": 70,
                  "shuffle": false,
                  "repeat_mode": "Off",
                  "queue": [],
                  "current_index": 2,
                  "updated_at_unix": 1700000123
                }
                """.trimIndent(),
            )
            assertNotNull(rustState)
            assertTrue(rustState!!.isPlaying)
            assertEquals(42_500L, rustState.positionMs)
            assertEquals(180_000L, rustState.durationMs)
            assertEquals(0.70f, rustState.volume, 0.001f)
            assertEquals("test-track-1", rustState.activeTrack?.id)
            assertEquals("Test Track", rustState.activeTrack?.title)
            assertEquals("Test Artist", rustState.activeTrack?.artist)
            assertNull(rustState.activeTrack?.thumbnailUrl)
            assertEquals(2, rustState.queueIndex)
            assertEquals(1_700_000_123_000L, rustState.timestampMs)

            val playJson = JSONObject(ConnectCommand.Play.toJson())
            assertEquals("play", playJson.getString("command"))
            val seekJson = JSONObject(ConnectCommand.SeekAbsolute(12_500L).toJson())
            assertEquals("seek_absolute", seekJson.getString("command"))
            assertEquals(12.5, seekJson.getDouble("position_secs"), 0.001)
            val volJson = JSONObject(ConnectCommand.SetVolume(0.75f).toJson())
            assertEquals("set_volume", volJson.getString("command"))
            assertEquals(75, volJson.getInt("volume"))
            val transferJson = JSONObject(
                ConnectCommand.TransferQueue(listOf(track), startIndex = 0, positionMs = 9_500L).toJson(),
            )
            assertEquals("transfer_queue", transferJson.getString("command"))
            assertEquals(9.5, transferJson.getDouble("position_secs"), 0.001)
            assertEquals(
                "YouTubeMusic",
                transferJson.getJSONArray("tracks").getJSONObject(0).getString("source"),
            )
        } finally {
            runCatching { server.close() }
            serverThread.join(2_000L)
        }
    }

    @Test
    fun `ecosystem strings are complete and localized across English, Turkish, and Russian`() {
        val en = ecosystemStrings(AppLanguage.ENGLISH)
        val tr = ecosystemStrings(AppLanguage.TURKISH)
        val ru = ecosystemStrings(AppLanguage.RUSSIAN)

        assertEquals(EnglishEcosystemStrings, en)
        assertEquals(TurkishEcosystemStrings, tr)
        assertEquals(RussianEcosystemStrings, ru)
        assertEquals(tr, ecosystemStrings(AppLanguage.SYSTEM, Locale("tr", "TR")))
        assertEquals(ru, ecosystemStrings(AppLanguage.SYSTEM, Locale("ru", "RU")))
        assertEquals(en, ecosystemStrings(AppLanguage.SYSTEM, Locale.US))

        assertEquals("Liked Music", en.autoLikedMusic)
        assertEquals("Beğenilen Müzikler", tr.autoLikedMusic)
        assertEquals("Понравившиеся", ru.autoLikedMusic)

        assertEquals("Playing on Sonata Linux", en.connectPlayingOn("Sonata Linux"))
        assertEquals("Sonata Linux üzerinde çalınıyor", tr.connectPlayingOn("Sonata Linux"))
        assertEquals("Воспроизводится на Sonata Linux", ru.connectPlayingOn("Sonata Linux"))

        listOf(en, tr, ru).forEach { s ->
            assertTrue(s.gaplessPlaybackTitle.isNotBlank())
            assertTrue(s.gaplessPlaybackDescription.isNotBlank())
            assertTrue(s.crossfadeTitle.isNotBlank())
            assertTrue(s.crossfadeDescription.isNotBlank())
            assertTrue(s.crossfadeOff.isNotBlank())
            assertEquals(s.crossfadeOff, s.crossfadeSecondsValue(0))
            assertTrue(s.crossfadeSecondsValue(6).contains("6"))
            assertTrue(s.crossfadeVideoBypassHint.isNotBlank())
            assertEquals("Sonata Connect", s.connectTitle)
            assertTrue(s.connectSubtitle.isNotBlank())
            assertTrue(s.connectEnableTitle.isNotBlank())
            assertTrue(s.connectDeviceNameLabel.isNotBlank())
            assertTrue(s.connectDiscoverTitle.isNotBlank())
            assertTrue(s.connectScanning.isNotBlank())
            assertTrue(s.connectNoDevices.isNotBlank())
            assertTrue(s.connectPairButton.isNotBlank())
            assertTrue(s.connectPairPinTitle.isNotBlank())
            assertTrue(s.connectPairPinHint.isNotBlank())
            assertTrue(s.connectPairedBadge.isNotBlank())
            assertTrue(s.connectConnectedTo("Desktop").contains("Desktop"))
            assertTrue(s.connectTransferPlayback.isNotBlank())
            assertTrue(s.connectDisconnect.isNotBlank())
            assertTrue(s.connectPairFailed.isNotBlank())
            assertTrue(s.connectDevicesTitle.isNotBlank())
            assertTrue(s.connectThisPhone.isNotBlank())
            assertTrue(s.connectThisPhoneSubtitle.isNotBlank())
            assertTrue(s.connectActiveBadge.isNotBlank())
            assertTrue(s.connectTrustedBadge.isNotBlank())
            assertTrue(s.connectTrustedDevicesTitle.isNotBlank())
            assertTrue(s.connectPlayingOn("Sonata Linux").contains("Sonata Linux"))
            assertTrue(s.connectWaitingApprovalTitle.isNotBlank())
            assertTrue(s.connectWaitingApprovalBody("Sonata Linux").contains("Sonata Linux"))
            assertTrue(s.connectUsePinFallback.isNotBlank())
            assertTrue(s.connectRemoteVolume.isNotBlank())
            assertTrue(s.connectOfflineLabel.isNotBlank())
            assertTrue(s.connectSelectDeviceHint.isNotBlank())
            ConnectError.entries.forEach { err ->
                assertTrue("Missing localized error for $err", s.connectErrorMessage(err).isNotBlank())
            }
            assertTrue(s.scrobblingTitle.isNotBlank())
            assertTrue(s.scrobblingSubtitle.isNotBlank())
            assertEquals("Last.fm", s.lastFmTitle)
            assertTrue(s.lastFmDescription.isNotBlank())
            assertEquals("ListenBrainz", s.listenBrainzTitle)
            assertTrue(s.listenBrainzDescription.isNotBlank())
            assertTrue(s.listenBrainzTokenHint.isNotBlank())
            assertTrue(s.scrobbleThresholdHint.isNotBlank())
            assertTrue(s.scrobbleStatusConnected("alice").contains("alice"))
            assertTrue(s.scrobbleStatusNotSignedIn.isNotBlank())
            assertTrue(s.listeningStatsTitle.isNotBlank())
            assertTrue(s.listeningStatsSubtitle.isNotBlank())
            assertTrue(s.statsWindowWeek.isNotBlank())
            assertTrue(s.statsWindowMonth.isNotBlank())
            assertTrue(s.statsWindowYear.isNotBlank())
            assertTrue(s.statsWindowAllTime.isNotBlank())
            assertTrue(s.statsTotalPlays.isNotBlank())
            assertTrue(s.statsTotalMinutes.isNotBlank())
            assertTrue(s.statsUniqueTracks.isNotBlank())
            assertTrue(s.statsUniqueArtists.isNotBlank())
            assertTrue(s.statsStreakDays(5).contains("5"))
            assertTrue(s.statsTopArtists.isNotBlank())
            assertTrue(s.statsTopTracks.isNotBlank())
            assertTrue(s.statsTopAlbums.isNotBlank())
            assertTrue(s.statsSourceBreakdown.isNotBlank())
            assertTrue(s.statsPrivacyNote.isNotBlank())
            assertEquals("Sonata", s.autoRootTitle)
            assertTrue(s.autoLikedMusic.isNotBlank())
            assertTrue(s.autoPlaylists.isNotBlank())
            assertTrue(s.autoPersonalPlaylists.isNotBlank())
            assertTrue(s.autoDownloads.isNotBlank())
            assertTrue(s.autoLocalMusic.isNotBlank())
            assertTrue(s.autoRecentlyPlayed.isNotBlank())
            assertTrue(s.autoEmptyNode.isNotBlank())
        }

        assertNotEquals(en.gaplessPlaybackTitle, tr.gaplessPlaybackTitle)
        assertNotEquals(en.gaplessPlaybackTitle, ru.gaplessPlaybackTitle)
        assertNotEquals(en.listeningStatsTitle, tr.listeningStatsTitle)
        assertNotEquals(en.listeningStatsTitle, ru.listeningStatsTitle)
        assertNotEquals(en.connectThisPhone, tr.connectThisPhone)
        assertNotEquals(en.connectThisPhone, ru.connectThisPhone)
        assertNotEquals(en.connectErrorPairingRejected, tr.connectErrorPairingRejected)
        assertNotEquals(en.connectErrorPairingRejected, ru.connectErrorPairingRejected)
    }

    @Test
    fun `sonata connect supports interactive approval request, status polling, and trust verification`() = runTest {
        val secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        var trustChecks = 0
        val server = ServerSocket(0)
        val port = server.localPort
        val serverThread = thread(isDaemon = true) {
            repeat(5) {
                val socket = runCatching { server.accept() }.getOrNull() ?: return@thread
                socket.use { sock ->
                    val reader = BufferedReader(InputStreamReader(sock.getInputStream(), StandardCharsets.UTF_8))
                    val requestLine = reader.readLine().orEmpty()
                    val headers = HashMap<String, String>()
                    var contentLength = 0
                    while (true) {
                        val line = reader.readLine() ?: break
                        if (line.isEmpty()) break
                        val idx = line.indexOf(':')
                        if (idx > 0) {
                            val name = line.substring(0, idx).trim().lowercase(Locale.ROOT)
                            val value = line.substring(idx + 1).trim()
                            headers[name] = value
                            if (name == "content-length") contentLength = value.toIntOrNull() ?: 0
                        }
                    }
                    val reqBody = if (contentLength > 0) {
                        val chars = CharArray(contentLength)
                        var readTotal = 0
                        while (readTotal < contentLength) {
                            val n = reader.read(chars, readTotal, contentLength - readTotal)
                            if (n < 0) break
                            readTotal += n
                        }
                        String(chars, 0, readTotal)
                    } else {
                        ""
                    }

                    val (statusLine, respBody) = when {
                        requestLine.startsWith("POST /connect/pair/request") -> {
                            val parsed = PairApprovalRequest.fromJson(reqBody)
                            assertNotNull(parsed)
                            assertEquals("android-test-2", parsed!!.clientDeviceId)
                            assertEquals("Test Phone", parsed.clientDeviceName)
                            "HTTP/1.1 202 Accepted" to PairApprovalInitAck(
                                requestId = "req-42",
                                status = "pending",
                                deviceId = "linux-host-1",
                                deviceName = "Sonata Linux",
                            ).toJson()
                        }
                        requestLine.startsWith("GET /connect/pair/status?request_id=req-42") -> {
                            "HTTP/1.1 200 OK" to PairStatusAck(
                                status = PairApprovalStatus.APPROVED,
                                accepted = true,
                                deviceId = "linux-host-1",
                                deviceName = "Sonata Linux",
                                sharedSecret = secret,
                            ).toJson()
                        }
                        requestLine.startsWith("GET /connect/trust") -> {
                            trustChecks++
                            val ts = headers[SonataConnectClient.HEADER_TIMESTAMP.lowercase(Locale.ROOT)].orEmpty()
                            val sig = headers[SonataConnectClient.HEADER_SIGNATURE.lowercase(Locale.ROOT)].orEmpty()
                            val valid = SonataConnectCrypto.verifySignature(secret, ts, sig)
                            if (trustChecks == 1 && valid) {
                                "HTTP/1.1 200 OK" to """{"trusted":true}"""
                            } else {
                                "HTTP/1.1 401 Unauthorized" to """{"trusted":false}"""
                            }
                        }
                        requestLine.startsWith("POST /connect/command") -> {
                            "HTTP/1.1 401 Unauthorized" to """{"error":"untrusted"}"""
                        }
                        else -> "HTTP/1.1 404 Not Found" to "{}"
                    }

                    val bytes = respBody.toByteArray(StandardCharsets.UTF_8)
                    val httpResp = "$statusLine\r\n" +
                        "Content-Type: application/json\r\n" +
                        "Content-Length: ${bytes.size}\r\n" +
                        "Connection: close\r\n\r\n"
                    val out = sock.getOutputStream()
                    out.write(httpResp.toByteArray(StandardCharsets.US_ASCII))
                    out.write(bytes)
                    out.flush()
                }
            }
        }

        try {
            val client = SonataConnectClient(localDeviceId = "android-test-2")
            val baseUrl = "http://127.0.0.1:$port"

            val init = client.requestPairApproval(baseUrl, "Test Phone").getOrThrow()
            assertEquals("req-42", init.requestId)
            assertEquals("pending", init.status)

            val status = client.pollPairStatus(baseUrl, init.requestId).getOrThrow()
            assertEquals(PairApprovalStatus.APPROVED, status.status)
            assertTrue(status.accepted)
            assertEquals(secret, status.sharedSecret)

            assertTrue(client.checkTrust(baseUrl, secret).getOrThrow())
            assertFalse(client.checkTrust(baseUrl, secret).getOrThrow())

            val cmdErr = client.sendCommand(baseUrl, secret, ConnectCommand.Play).exceptionOrNull()
            assertTrue(cmdErr is ConnectClientException)
            assertEquals(ConnectError.TRUST_REVOKED, (cmdErr as ConnectClientException).error)
        } finally {
            runCatching { server.close() }
            serverThread.join(2_000L)
        }
    }
}
