package dev.sonata.android

import dev.sonata.android.connect.ConnectBeacon
import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.ConnectController
import dev.sonata.android.connect.ConnectDeviceInfo
import dev.sonata.android.connect.ConnectStateSnapshot
import dev.sonata.android.connect.ConnectTrackSnapshot
import dev.sonata.android.connect.InMemoryConnectSecretStore
import dev.sonata.android.connect.PairAck
import dev.sonata.android.connect.PairRequest
import dev.sonata.android.connect.SonataConnectClient
import dev.sonata.android.connect.SonataConnectCrypto
import dev.sonata.android.playback.LiveAutoDataProvider
import dev.sonata.android.playback.ScrobbleAndStatsTracker
import dev.sonata.android.playback.SonataAutoBrowser
import dev.sonata.android.ui.i18n.EnglishEcosystemStrings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.runTest
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidStatsWindow
import uniffi.sonata_android.AndroidTrack
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.ServerSocket
import java.nio.charset.StandardCharsets
import java.util.Locale
import kotlin.concurrent.thread

class Faz4UiIntegrationTest {

    private fun track(
        id: String,
        title: String,
        artist: String = "Artist",
        album: String = "Album",
        durationSecs: ULong? = 210uL,
    ) = AndroidTrack(
        id = id,
        title = title,
        artist = artist,
        album = album,
        durationSecs = durationSecs,
        thumbnailUrl = "https://example.com/$id.jpg",
        liked = id.startsWith("liked"),
        albumId = "alb_$id",
        isExplicit = false,
    )

    @Test
    fun `stats window queries filter Week, Month, Year, and AllTime plays and source breakdown`() = runTest {
        val gateway = FakeSonataEngineGateway()
        val now = 1_700_500_000uL
        val day = 86_400uL
        gateway.fakeStatsNowUnix = now

        gateway.recordListeningStat(
            track = track("yt_1", "Song Week", "Artist A", "Album 1", 240uL),
            sourceTag = "",
            playedSecs = 240uL,
            playedAtUnix = now - 120uL,
        )
        gateway.recordListeningStat(
            track = track("local:2", "Song Month", "Artist B", "Album 2", 180uL),
            sourceTag = "",
            playedSecs = 180uL,
            playedAtUnix = now - 15uL * day,
        )
        gateway.recordListeningStat(
            track = track("jellyfin:srv:3", "Song Year", "Artist A", "Album 3", 300uL),
            sourceTag = "",
            playedSecs = 300uL,
            playedAtUnix = now - 120uL * day,
        )
        gateway.recordListeningStat(
            track = track("subsonic:srv:4", "Song Old", "Artist C", "Album 4", 120uL),
            sourceTag = "",
            playedSecs = 120uL,
            playedAtUnix = now - 500uL * day,
        )

        val week = gateway.queryListeningStats(AndroidStatsWindow.WEEK)
        assertEquals(1uL, week.totalPlays)
        assertEquals(4uL, week.totalMinutes)
        assertEquals("YTM", week.sourceBreakdown.single().sourceTag)

        val month = gateway.queryListeningStats(AndroidStatsWindow.MONTH)
        assertEquals(2uL, month.totalPlays)
        assertEquals(7uL, month.totalMinutes)
        assertEquals(2uL, month.uniqueArtists)
        assertTrue(month.sourceBreakdown.any { it.sourceTag == "LOC" && it.plays == 1uL })

        val year = gateway.queryListeningStats(AndroidStatsWindow.YEAR)
        assertEquals(3uL, year.totalPlays)
        assertEquals(12uL, year.totalMinutes)
        assertTrue(year.sourceBreakdown.any { it.sourceTag == "JFN" && it.plays == 1uL })

        val all = gateway.queryListeningStats(AndroidStatsWindow.ALL_TIME)
        assertEquals(4uL, all.totalPlays)
        assertEquals(14uL, all.totalMinutes)
        assertEquals(4uL, all.uniqueTracks)
        assertEquals(3uL, all.uniqueArtists)
    }

    @Test
    fun `scrobble threshold tracker enforces 40s and 50 percent rule without duplicate triggers`() {
        val tracker = ScrobbleAndStatsTracker()

        // Long track (240s): 40s triggers before 50% (120s)
        val longTrack = track("yt_long", "Long Song", durationSecs = 240uL)
        tracker.onTrackStarted(generation = 1L, track = longTrack, sourceTag = "YTM", startedAtUnixSecs = 1_700_000_000L)
        assertEquals(longTrack, tracker.consumeNowPlaying(1L))
        assertNull("Now playing must only fire once", tracker.consumeNowPlaying(1L))

        val beforeThreshold = tracker.advance(1L, 39_000L)
        assertNull(beforeThreshold.scrobbleTriggerTimestampUnix)
        assertNull(beforeThreshold.statsPlayedSecs)
        assertFalse(tracker.isThresholdTriggered(1L))

        val atThreshold = tracker.advance(1L, 1_500L)
        assertEquals(1_700_000_000L, atThreshold.scrobbleTriggerTimestampUnix)
        assertEquals(240L, atThreshold.statsPlayedSecs)
        assertEquals("YTM", atThreshold.sourceTag)
        assertTrue(tracker.isThresholdTriggered(1L))

        val afterThreshold = tracker.advance(1L, 60_000L)
        assertNull("Must not double-scrobble", afterThreshold.scrobbleTriggerTimestampUnix)
        assertNull("Must not double-record stats", afterThreshold.statsPlayedSecs)

        // Short track (50s): 50% (25s) triggers before 40s
        val shortTrack = track("local:short", "Short Track", durationSecs = 50uL)
        tracker.onTrackStarted(generation = 2L, track = shortTrack, sourceTag = "LOC", startedAtUnixSecs = 1_700_001_000L)
        assertNull(tracker.advance(2L, 24_000L).scrobbleTriggerTimestampUnix)
        val shortHit = tracker.advance(2L, 1_000L)
        assertEquals(1_700_001_000L, shortHit.scrobbleTriggerTimestampUnix)
        assertEquals(50L, shortHit.statsPlayedSecs)
        assertEquals("LOC", shortHit.sourceTag)

        // Unknown duration requires full 40s
        val unknownDur = track("yt_unknown", "Unknown", durationSecs = null)
        tracker.onTrackStarted(generation = 3L, track = unknownDur, sourceTag = "YTM", startedAtUnixSecs = 1_700_002_000L)
        assertNull(tracker.advance(3L, 39_000L).scrobbleTriggerTimestampUnix)
        val unknownHit = tracker.advance(3L, 1_000L)
        assertEquals(1_700_002_000L, unknownHit.scrobbleTriggerTimestampUnix)
        assertEquals(40L, unknownHit.statsPlayedSecs)
    }

    @Test
    fun `auto browser provider exposes live categories and dispatches PlaybackQueueBridge onPlayMediaId`() {
        PlaybackQueueBridge.reset()
        val liked = listOf(track("liked_1", "Liked Track"))
        val personal = listOf(AndroidPlaylist("personal:p1", "Lossless Vault", null, isOwned = true))
        val ytmPlaylists = listOf(AndroidPlaylist("PL_road", "Roadtrip", null, isOwned = false))
        val personalTracks = listOf(track("local:10", "FLAC Track"))
        val ytmTracks = listOf(track("yt_road_1", "Road Song"))
        val downloads = listOf(track("dl_1", "Offline Track"))
        val local = listOf(track("local:10", "FLAC Track"))
        val recent = listOf(track("rec_1", "Recent Track"))

        val provider = LiveAutoDataProvider(
            likedTracksProvider = { liked },
            playlistsProvider = { ytmPlaylists + personal },
            playlistTracksProvider = { id ->
                when (id) {
                    "personal:p1" -> personalTracks
                    "PL_road" -> ytmTracks
                    else -> emptyList()
                }
            },
            downloadedTracksProvider = { downloads },
            localTracksProvider = { local },
            recentTracksProvider = { recent },
        )

        val browser = SonataAutoBrowser(
            dataProvider = provider,
            stringsProvider = { EnglishEcosystemStrings },
        )

        assertEquals(listOf("liked_1"), browser.getChildren(SonataAutoBrowser.LIKED_ID)?.map { it.mediaId })
        assertEquals(
            listOf("sonata:playlist:PL_road", "sonata:playlist:personal:p1"),
            browser.getChildren(SonataAutoBrowser.PLAYLISTS_ID)?.map { it.mediaId },
        )
        assertEquals(listOf("local:10"), browser.getChildren("sonata:playlist:personal:p1")?.map { it.mediaId })
        assertEquals(listOf("dl_1"), browser.getChildren(SonataAutoBrowser.DOWNLOADS_ID)?.map { it.mediaId })
        assertEquals(listOf("local:10"), browser.getChildren(SonataAutoBrowser.LOCAL_ID)?.map { it.mediaId })
        assertEquals(listOf("rec_1"), browser.getChildren(SonataAutoBrowser.RECENT_ID)?.map { it.mediaId })

        var playedPair: Pair<AndroidTrack, List<AndroidTrack>>? = null
        PlaybackQueueBridge.onPlayMediaId = { mediaId ->
            playedPair = provider.resolvePlaybackTarget(mediaId)
        }

        PlaybackQueueBridge.onPlayMediaId("dl_1")
        assertNotNull(playedPair)
        assertEquals("dl_1", playedPair!!.first.id)
        assertEquals(listOf("dl_1"), playedPair!!.second.map { it.id })

        PlaybackQueueBridge.onPlayMediaId("sonata:playlist:PL_road")
        assertNotNull(playedPair)
        assertEquals("yt_road_1", playedPair!!.first.id)
        assertEquals(listOf("yt_road_1"), playedPair!!.second.map { it.id })

        PlaybackQueueBridge.reset()
    }

    @Test
    fun `connect controller pairs via 6-digit PIN, dispatches remote controls, and transfers queue`() = runTest {
        val secret = "connect-test-secret-0123456789"
        val receivedCommands = mutableListOf<ConnectCommand>()
        val server = ServerSocket(0)
        val port = server.localPort

        val serverThread = thread(isDaemon = true) {
            // Pair + fetchState + Pause + fetchState + TransferQueue + fetchState
            repeat(6) {
                val sock = runCatching { server.accept() }.getOrNull() ?: return@thread
                sock.use { s ->
                    val reader = BufferedReader(InputStreamReader(s.getInputStream(), StandardCharsets.UTF_8))
                    val reqLine = reader.readLine().orEmpty()
                    val headers = HashMap<String, String>()
                    var contentLen = 0
                    while (true) {
                        val line = reader.readLine() ?: break
                        if (line.isEmpty()) break
                        val idx = line.indexOf(':')
                        if (idx > 0) {
                            val name = line.substring(0, idx).trim().lowercase(Locale.ROOT)
                            val value = line.substring(idx + 1).trim()
                            headers[name] = value
                            if (name == "content-length") contentLen = value.toIntOrNull() ?: 0
                        }
                    }
                    val body = if (contentLen > 0) {
                        val buf = CharArray(contentLen)
                        var read = 0
                        while (read < contentLen) {
                            val n = reader.read(buf, read, contentLen - read)
                            if (n < 0) break
                            read += n
                        }
                        String(buf, 0, read)
                    } else {
                        ""
                    }

                    val resp = when {
                        reqLine.startsWith("POST /connect/pair") -> {
                            val req = PairRequest.fromJson(body)
                            assertEquals("482910", req?.pin)
                            PairAck(
                                accepted = true,
                                deviceId = "desktop-host",
                                deviceName = "Sonata Desktop",
                                sharedSecret = secret,
                            ).toJson()
                        }
                        reqLine.startsWith("GET /connect/state") -> {
                            ConnectStateSnapshot(
                                deviceId = "desktop-host",
                                deviceName = "Sonata Desktop",
                                isPlaying = true,
                                positionMs = 10_000L,
                                durationMs = 200_000L,
                                volume = 0.8f,
                                activeTrack = ConnectTrackSnapshot("yt_1", "Song 1", "Artist 1", "Album 1", 200L),
                            ).toJson()
                        }
                        reqLine.startsWith("POST /connect/command") -> {
                            val sig = headers[SonataConnectClient.HEADER_SIGNATURE.lowercase(Locale.ROOT)].orEmpty()
                            assertTrue(SonataConnectCrypto.verifySignature(secret, body, sig))
                            val cmd = ConnectCommand.fromJson(body)
                            assertNotNull(cmd)
                            receivedCommands.add(cmd!!)
                            JSONObject().put("ok", true).toString()
                        }
                        else -> "{}"
                    }

                    val bytes = resp.toByteArray(StandardCharsets.UTF_8)
                    val out = s.getOutputStream()
                    out.write(
                        (
                            "HTTP/1.1 200 OK\r\n" +
                                "Content-Type: application/json\r\n" +
                                "Content-Length: ${bytes.size}\r\n" +
                                "Connection: close\r\n\r\n"
                            ).toByteArray(StandardCharsets.US_ASCII),
                    )
                    out.write(bytes)
                    out.flush()
                }
            }
        }

        val controllerScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        try {
            val client = SonataConnectClient(localDeviceId = "android-peer-1")
            val controller = ConnectController(
                client = client,
                secretStore = InMemoryConnectSecretStore(),
                scope = controllerScope,
                deviceNameProvider = { "Pixel 9" },
            )

            val beacon = ConnectBeacon(
                deviceId = "desktop-host",
                deviceName = "Sonata Desktop",
                platform = "linux",
                httpPort = port,
                protocolVersion = 1,
                hostAddress = "127.0.0.1",
            )
            client.ingestBeaconPacket(beacon.toJson(), "127.0.0.1")

            assertTrue(controller.pairAndConnect(beacon, "482910"))
            assertEquals("desktop-host", controller.state.value.activePeer?.deviceId)
            assertTrue("desktop-host" in controller.state.value.pairedDeviceIds)

            assertTrue(controller.sendCommand(ConnectCommand.Pause).isSuccess)

            val queue = listOf(
                track("yt_a", "Alpha", "Artist A", "Album A", 180uL),
                track("local:b", "Beta", "Artist B", "Album B", 240uL),
            )
            val transferResult = controller.transferQueue(
                tracks = queue,
                startIndex = 1,
                positionMs = 45_000L,
                playWhenReady = true,
            )
            assertTrue(transferResult.isSuccess)

            assertEquals(2, receivedCommands.size)
            assertEquals(ConnectCommand.Pause, receivedCommands[0])
            val handoff = receivedCommands[1] as ConnectCommand.TransferQueue
            assertEquals(listOf("yt_a", "local:b"), handoff.tracks.map { it.id })
            assertEquals(1, handoff.startIndex)
            assertEquals(45_000L, handoff.positionMs)
            assertTrue(handoff.playWhenReady)

            controller.close()
        } finally {
            controllerScope.cancel()
            runCatching { server.close() }
            serverThread.join(2_000L)
        }
    }
}
