package dev.sonata.android

import dev.sonata.android.ui.SteeredLyricClock
import dev.sonata.android.ui.components.SonataShareTarget
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidTrack

class ShareAndChangelogTest {

    @Test
    fun shareTargetsBuildCleanYouTubeMusicAndSearchFallbackUrls() {
        val ytmTrack = SonataShareTarget.Track(
            AndroidTrack(
                id = "dQw4w9WgXcQ",
                title = "Never Gonna Give You Up",
                artist = "Rick Astley",
                album = "Whenever You Need Somebody",
                durationSecs = 213u,
                thumbnailUrl = null,
                liked = false,
            ),
        )
        assertEquals("SONG", ytmTrack.eyebrowLabel)
        assertEquals("https://music.youtube.com/watch?v=dQw4w9WgXcQ", ytmTrack.shareUrl)
        assertEquals("Rick Astley · Whenever You Need Somebody", ytmTrack.shareSubtitle)

        val jioTrack = SonataShareTarget.Track(
            AndroidTrack(
                id = "jiosaavn:abc123",
                title = "Starboy",
                artist = "The Weeknd",
                album = "",
                durationSecs = 230u,
                thumbnailUrl = null,
                liked = true,
            ),
        )
        assertEquals("The Weeknd", jioTrack.shareSubtitle)
        assertEquals("https://music.youtube.com/search?q=Starboy%20The%20Weeknd", jioTrack.shareUrl)

        assertEquals(
            "https://music.youtube.com/playlist?list=PL_abc",
            buildShareUrlForCollection("VLPL_abc", "Mix"),
        )
        assertEquals(
            "https://music.youtube.com/playlist?list=OLAK5uy_xyz",
            buildShareUrlForCollection("OLAK5uy_xyz", "Album"),
        )
        assertEquals(
            "https://music.youtube.com/browse/MPREb_999",
            buildShareUrlForCollection("MPREb_999", "Album"),
        )
        assertEquals(
            "https://music.youtube.com/channel/UC_42",
            buildShareUrlForCollection("UC_42", "Artist"),
        )
        assertEquals(
            "https://music.youtube.com/search?q=Night%20Mix",
            buildShareUrlForCollection("local:pl_1", "Night Mix"),
        )

        val albumCollection = SonataShareTarget.Collection(
            browseId = "MPREb_999",
            collectionTitle = "After Hours",
            collectionSubtitle = "The Weeknd",
            coverUrl = null,
            isAlbum = true,
            trackCount = 14,
            year = "2020",
        )
        assertEquals("ALBUM", albumCollection.eyebrowLabel)
        assertEquals("14 TRACKS • 2020", albumCollection.badgeMeta)

        val singleTrackPlaylist = SonataShareTarget.Collection(
            browseId = "PL_1",
            collectionTitle = "One",
            collectionSubtitle = "",
            coverUrl = null,
            isAlbum = false,
            trackCount = 1,
            year = null,
        )
        assertEquals("PLAYLIST", singleTrackPlaylist.eyebrowLabel)
        assertEquals("1 TRACK", singleTrackPlaylist.badgeMeta)

        val ytmArtist = SonataShareTarget.Artist(
            artistId = "UC_weeknd",
            artistName = "The Weeknd",
            subscribers = "36M subscribers",
            avatarUrl = null,
        )
        assertEquals("ARTIST", ytmArtist.eyebrowLabel)
        assertEquals("https://music.youtube.com/channel/UC_weeknd", ytmArtist.shareUrl)

        val fallbackArtist = SonataShareTarget.Artist(
            artistId = "",
            artistName = "Daft Punk",
            subscribers = null,
            avatarUrl = null,
        )
        assertEquals("https://music.youtube.com/search?q=Daft%20Punk", fallbackArtist.shareUrl)
    }

    @Test
    fun formatDurationAndChangelogParserHandleMarkdownStructuredNotes() {
        assertEquals("0:00", formatDurationMs(-500L))
        assertEquals("0:05", formatDurationMs(5_000L))
        assertEquals("3:50", formatDurationMs(230_000L))

        assertEquals(
            "Sonata App — High-Fidelity Audio",
            cleanChangelogMarkdown("**[Sonata App](https://example.com)** — *High-Fidelity* `Audio`"),
        )

        val rawMarkdown = """
            ### Added
            - **Violet & Moonflower Themes**: Light & Dark modes.
            * Pinkish Liquid Glass pill — subtle rim highlight
            ---
            Rolling release summary paragraph.
        """.trimIndent()

        val parsed = parseChangelogEntries(rawMarkdown)
        assertEquals(4, parsed.size)
        assertEquals(ChangelogEntry.Heading("Added"), parsed[0])
        assertEquals(
            ChangelogEntry.Bullet(lead = "Violet & Moonflower Themes", body = "Light & Dark modes."),
            parsed[1],
        )
        assertEquals(
            ChangelogEntry.Bullet(lead = "Pinkish Liquid Glass pill", body = "subtle rim highlight"),
            parsed[2],
        )
        assertEquals(ChangelogEntry.Note("Rolling release summary paragraph."), parsed[3])
        assertTrue(parseChangelogEntries("   \n  ").isEmpty())
    }

    @Test
    fun steeredLyricClockAdvancesSmoothlyHoldsAndJumpsOnSeekDiscontinuity() {
        val clock = SteeredLyricClock(startMs = 0L)

        val initial = clock.frame(
            nowMs = 1_000.0,
            reportedMs = 1_000L,
            sampledAtMs = 1_000.0,
            discontinuity = 0L,
        )
        assertEquals(1_000L, initial)

        // Frame advance without new ExoPlayer poll interpolates monotonically.
        val advanced = clock.frame(
            nowMs = 1_016.0,
            reportedMs = 1_000L,
            sampledAtMs = 1_000.0,
            discontinuity = 0L,
        )
        assertEquals(1_016L, advanced)

        // Small jitter drift steers gradually rather than hard snapping.
        val steered = clock.frame(
            nowMs = 1_032.0,
            reportedMs = 1_120L,
            sampledAtMs = 1_032.0,
            discontinuity = 0L,
        )
        assertTrue("Steered clock ($steered) should advance smoothly beyond 1032 without hard snapping", steered in 1_033L..1_080L)

        // Hold freezes position during pause/buffering.
        clock.hold(positionMs = 5_000L, nowMs = 2_000.0)
        assertEquals(5_000.0, clock.displayedMs, 0.01)

        // Seek discontinuity jumps immediately to the new reported position.
        clock.restart()
        val afterSeek = clock.frame(
            nowMs = 2_100.0,
            reportedMs = 45_000L,
            sampledAtMs = 2_100.0,
            discontinuity = 1L,
        )
        assertEquals(45_000L, afterSeek)
    }
}
