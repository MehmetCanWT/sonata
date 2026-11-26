package dev.sonata.android

import dev.sonata.android.data.SmbClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerProfile
import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets

class SmbClientTest {

    private fun smbProfile(
        baseUrl: String,
        share: String? = null,
        username: String? = "alice",
        password: String? = "secret",
        domain: String? = "WORKGROUP",
    ): AndroidServerProfile = AndroidServerProfile(
        id = "srv-smb-1",
        name = "Home NAS",
        kind = AndroidServerKind.SMB,
        baseUrl = baseUrl,
        enabled = true,
        username = username,
        password = password,
        apiKey = null,
        accessToken = null,
        userId = null,
        bearerToken = null,
        libraryIds = emptyList(),
        share = share,
        domain = domain,
        localMountPath = null,
    )

    @Test
    fun parseEndpointSupportsSmbUrlUncAndHostPlusShare() {
        val urlRes = SmbClient.parseEndpoint(smbProfile("smb://192.168.1.20:1445/Music/FLAC"))
        assertTrue(urlRes.isSuccess)
        val urlEp = urlRes.getOrThrow()
        assertEquals("192.168.1.20", urlEp.host)
        assertEquals(1445, urlEp.port)
        assertEquals("Music", urlEp.share)
        assertEquals("FLAC", urlEp.baseSubpath)
        assertEquals("alice", urlEp.username)

        val uncRes = SmbClient.parseEndpoint(smbProfile("""\\nas.local\Lossless\Albums"""))
        assertTrue(uncRes.isSuccess)
        val uncEp = uncRes.getOrThrow()
        assertEquals("nas.local", uncEp.host)
        assertEquals(SmbClient.DEFAULT_SMB_PORT, uncEp.port)
        assertEquals("Lossless", uncEp.share)
        assertEquals("Albums", uncEp.baseSubpath)

        val hostOnlyRes = SmbClient.parseEndpoint(smbProfile("10.0.0.5", share = "Audio"))
        assertTrue(hostOnlyRes.isSuccess)
        val hostEp = hostOnlyRes.getOrThrow()
        assertEquals("10.0.0.5", hostEp.host)
        assertEquals("Audio", hostEp.share)
        assertEquals("", hostEp.baseSubpath)
    }

    @Test
    fun parseEndpointRejectsMissingHostOrShare() {
        assertTrue(SmbClient.parseEndpoint(smbProfile("")).isFailure)
        assertTrue(SmbClient.parseEndpoint(smbProfile("192.168.1.20", share = null)).isFailure)
    }

    @Test
    fun buildBaseUrlFormatsCanonicalSmbUri() {
        assertEquals(
            "smb://192.168.1.20/Music/FLAC",
            SmbClient.buildBaseUrl(host = "192.168.1.20", portText = "445", share = "Music", subpath = "/FLAC/"),
        )
        assertEquals(
            "smb://nas.home:1445/Media",
            SmbClient.buildBaseUrl(host = "smb://nas.home/", portText = "1445", share = "/Media/", subpath = ""),
        )
    }

    @Test
    fun streamUriRoundTripsUtf8AndSpacesSafely() {
        val rawPath = "Daft Punk/Random Access Memories/01 Give Life Back to Music (Şarkı).flac"
        val uri = SmbClient.buildStreamUri(serverId = "srv-smb-1", relativePath = rawPath)
        assertTrue(uri.startsWith("sonata-smb://srv-smb-1/"))
        val parsed = SmbClient.parseStreamUri(uri)
        assertNotNull(parsed)
        assertEquals("srv-smb-1", parsed?.first)
        assertEquals(rawPath, parsed?.second)
        assertNull(SmbClient.parseStreamUri("https://example.com/test.flac"))
    }

    @Test
    fun parseTrackIdAndBadgeExtractServerAndCleanFormatOnly() {
        val trackId = "smb:srv-smb-1:Albums/track.flac"
        assertEquals("srv-smb-1" to "Albums/track.flac", SmbClient.parseTrackId(trackId))
        assertNull(SmbClient.parseTrackId("jellyfin:srv-1:item-2"))
        assertEquals("FLAC", SmbClient.initialBadgeForPath("Albums/track.flac"))
        assertEquals("MP3", SmbClient.initialBadgeForPath("track.mp3"))
        assertEquals("", SmbClient.initialBadgeForPath("track"))
    }

    @Test
    fun inferArtistAndAlbumUsesFolderHierarchyNeverServerName() {
        assertEquals("Eminem" to "MMLP", SmbClient.inferArtistAndAlbum("Music/Eminem/MMLP"))
        assertEquals("Eminem" to "The Slim Shady LP", SmbClient.inferArtistAndAlbum("share/musix/Music/Eminem/The Slim Shady LP"))
        assertEquals("Eminem" to "MMLP", SmbClient.inferArtistAndAlbum("Music/Eminem/MMLP/CD1"))
        assertEquals("Unknown Artist" to "Lo-Fi Beats", SmbClient.inferArtistAndAlbum("Music/Lo-Fi Beats"))
        assertEquals("Unknown Artist" to "Unknown Album", SmbClient.inferArtistAndAlbum("Music"))
        assertEquals("Unknown Artist" to "Unknown Album", SmbClient.inferArtistAndAlbum(""))
    }

    @Test
    fun parseFlacHeaderExtractsStreamInfoAndVorbisComments() {
        fun leInt(v: Int): ByteArray = byteArrayOf(
            (v and 0xFF).toByte(),
            ((v ushr 8) and 0xFF).toByte(),
            ((v ushr 16) and 0xFF).toByte(),
            ((v ushr 24) and 0xFF).toByte(),
        )
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf('f'.code.toByte(), 'L'.code.toByte(), 'a'.code.toByte(), 'C'.code.toByte()))
        // Block 0 (STREAMINFO, 34 bytes, non-last): 44100 Hz, 2ch, 16-bit, 8820000 samples (= 200s)
        out.write(byteArrayOf(0x00, 0x00, 0x00, 34))
        val streamInfo = ByteArray(34)
        // 44100 = 0xAC44 -> b10=0x0A, b11=0xC4, high nibble b12=0x4; channels=2->(1<<1)=0x2; bps=16->15=0x0F (0x0 in b12 bit0, 0xF0 in b13 high nibble)
        streamInfo[10] = 0x0A.toByte()
        streamInfo[11] = 0xC4.toByte()
        streamInfo[12] = 0x42.toByte()
        streamInfo[13] = 0xF0.toByte()
        val totalSamples = 8_820_000L
        streamInfo[14] = ((totalSamples ushr 24) and 0xFF).toByte()
        streamInfo[15] = ((totalSamples ushr 16) and 0xFF).toByte()
        streamInfo[16] = ((totalSamples ushr 8) and 0xFF).toByte()
        streamInfo[17] = (totalSamples and 0xFF).toByte()
        out.write(streamInfo)

        // Block 4 (VORBIS_COMMENT, last)
        val comments = listOf(
            "TITLE=Guilty Conscience",
            "ARTIST=Eminem",
            "ALBUM=The Slim Shady LP",
            "LYRICS=[00:02.00] Meet Eddie",
        )
        val vorbisPayload = ByteArrayOutputStream()
        val vendor = "Lavf".toByteArray(StandardCharsets.UTF_8)
        vorbisPayload.write(leInt(vendor.size))
        vorbisPayload.write(vendor)
        vorbisPayload.write(leInt(comments.size))
        for (c in comments) {
            val b = c.toByteArray(StandardCharsets.UTF_8)
            vorbisPayload.write(leInt(b.size))
            vorbisPayload.write(b)
        }
        val vcBytes = vorbisPayload.toByteArray()
        out.write(
            byteArrayOf(
                0x84.toByte(),
                ((vcBytes.size ushr 16) and 0xFF).toByte(),
                ((vcBytes.size ushr 8) and 0xFF).toByte(),
                (vcBytes.size and 0xFF).toByte(),
            ),
        )
        out.write(vcBytes)

        val meta = SmbClient.parseFlacHeader(out.toByteArray())
        assertNotNull(meta)
        assertEquals("Guilty Conscience", meta?.title)
        assertEquals("Eminem", meta?.artist)
        assertEquals("The Slim Shady LP", meta?.album)
        assertEquals(200L, meta?.durationSecs)
        assertEquals(44_100, meta?.sampleRateHz)
        assertEquals(2, meta?.channels)
        assertEquals(16, meta?.bitsPerSample)
        assertEquals("[00:02.00] Meet Eddie", meta?.lyrics)
    }

    @Test
    fun parseLyricsTextParsesSyncedOffsetWordTagsAndPlainFallback() {
        val syncedRaw = """
            [ar:Eminem]
            [offset:500]
            [00:02.50][01:00.00] Meet <00:03.00>Eddie <00:03.50>now
        """.trimIndent()
        val parsed = SmbClient.parseLyricsText(syncedRaw)
        assertEquals(2, parsed.size)
        assertEquals(2.0, parsed[0].timeSecs, 0.01)
        assertEquals("Meet Eddie now", parsed[0].text)
        assertEquals(2, parsed[0].words.size)
        assertEquals(59.5, parsed[1].timeSecs, 0.01)

        val plainParsed = SmbClient.parseLyricsText("Line one\nLine two")
        assertEquals(2, plainParsed.size)
        assertEquals("Line one", plainParsed[0].text)
    }

    @Test
    fun parseLyricsTextSplitsInlineBackgroundVocalsAndParsesTtml() {
        val inlineLrc = """
            [00:04.50] Fed up with life (damn, I'm goin' insane)
            [00:08.50] Next line word(s)
        """.trimIndent()
        val split = SmbClient.parseLyricsText(inlineLrc)
        assertEquals(3, split.size)
        assertEquals("Fed up with life", split[0].text)
        assertEquals("(damn, I'm goin' insane)", split[1].text)
        assertTrue(split[1].timeSecs > 4.5 && split[1].timeSecs < 8.5)
        assertEquals("Next line word(s)", split[2].text)

        val ttml = """
            <tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata">
              <head><metadata>
                <ttm:agent type="person" xml:id="v1"/>
                <ttm:agent type="person" xml:id="v2"/>
              </metadata></head>
              <body><div>
                <p begin="2.10" end="4.60" ttm:agent="v1">
                  <span begin="2.10" end="2.60">Meet</span> <span begin="2.60" end="4.60">Eddie</span>
                  <span ttm:role="x-bg"><span begin="3.80" end="4.50">(who?)</span></span>
                </p>
                <p begin="5.00" end="6.50" ttm:agent="v2">
                  <span begin="5.00" end="6.50">Alright</span>
                </p>
              </div></body>
            </tt>
        """.trimIndent()
        val ttmlParsed = SmbClient.parseLyricsText(ttml)
        assertEquals(3, ttmlParsed.size)
        assertEquals("Meet Eddie", ttmlParsed[0].text)
        assertEquals(2, ttmlParsed[0].words.size)
        assertFalse(ttmlParsed[0].isCounterVoice)
        assertEquals("(who?)", ttmlParsed[1].text)
        assertEquals(3.8, ttmlParsed[1].timeSecs, 0.01)
        assertEquals("Alright", ttmlParsed[2].text)
        assertTrue(ttmlParsed[2].isCounterVoice)

        val wordSyncedInline = """
            [00:22.00] <00:22.00>Think <00:22.50>twice <00:23.20>(who <00:23.60>are <00:23.90>you?)
        """.trimIndent()
        val splitWord = SmbClient.parseLyricsText(wordSyncedInline)
        assertEquals(2, splitWord.size)
        assertEquals("Think twice", splitWord[0].text)
        assertEquals(2, splitWord[0].words.size)
        assertEquals("(who are you?)", splitWord[1].text)
        assertEquals(3, splitWord[1].words.size)
        assertEquals(23.2, splitWord[1].timeSecs, 0.01)
    }

    @Test
    fun parseArtistMetadataTextSupportsJsonAndXmlNfo() {
        val jsonRaw = """
            {
              "name": "Eminem",
              "genre": "Hip-Hop / Rap",
              "origin": "Detroit, Michigan",
              "bio": "Marshall Bruce Mathers III is an American rapper."
            }
        """.trimIndent()
        val jsonMeta = SmbClient.parseArtistMetadataText(jsonRaw, isXml = false)
        assertEquals("Eminem", jsonMeta.name)
        assertEquals("Hip-Hop / Rap · Detroit, Michigan", jsonMeta.subtitle)
        assertEquals("Marshall Bruce Mathers III is an American rapper.", jsonMeta.bio)

        val nfoRaw = """
            <artist>
              <name>Daft Punk</name>
              <genre>Electronic</genre>
              <formed>Paris, France</formed>
              <biography><![CDATA[French electronic music duo formed in 1993.]]></biography>
            </artist>
        """.trimIndent()
        val nfoMeta = SmbClient.parseArtistMetadataText(nfoRaw, isXml = true)
        assertEquals("Daft Punk", nfoMeta.name)
        assertEquals("Electronic · Paris, France", nfoMeta.subtitle)
        assertEquals("French electronic music duo formed in 1993.", nfoMeta.bio)
    }

    @Test
    fun toSmbWinPathNormalizesBackslashesAndTrimsEdges() {
        assertEquals("Music\\Rock\\01 Intro.flac", SmbClient.toSmbWinPath("", "/Music//Rock/01 Intro.flac/"))
        assertEquals("FLAC\\Albums", SmbClient.toSmbWinPath("FLAC/", "/Albums/"))
        assertEquals("", SmbClient.toSmbWinPath("", "///"))
    }

    @Test
    fun toSmbWinPathRejectsTraversalAndDriveColonSegments() {
        val maliciousInputs = listOf(
            "" to "../etc/passwd",
            "Music" to "..\\secret.flac",
            "Music" to "./track.flac",
            "." to "track.flac",
            ".." to "track.flac",
            "" to "C:/Windows/win.ini",
            "Music" to "track.flac:hiddenStream",
            "Music" to "bad\u0000name.flac",
        )
        for ((base, rel) in maliciousInputs) {
            val threw = try {
                SmbClient.toSmbWinPath(base, rel)
                false
            } catch (_: IllegalArgumentException) {
                true
            }
            assertTrue("Expected IllegalArgumentException for base='$base', rel='$rel'", threw)
        }
    }
}
