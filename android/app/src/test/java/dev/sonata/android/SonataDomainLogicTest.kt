package dev.sonata.android

import dev.sonata.android.data.VideoQualityPreference
import dev.sonata.android.playback.mapMusicToVideoMs
import dev.sonata.android.playback.mapVideoToMusicMs
import dev.sonata.android.ui.components.isEditableUserPlaylist
import dev.sonata.android.ui.components.isSaveableExternalCollection
import dev.sonata.android.ui.components.normalizeCollectionId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidMusicVideoSyncSegment
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidVideoQualityStream

class SonataDomainLogicTest {

    private fun track(
        id: String = "v1",
        title: String = "Starboy",
        artist: String = "The Weeknd",
        album: String = "",
        thumb: String? = null,
        albumId: String? = null,
    ) = AndroidTrack(
        id = id,
        title = title,
        artist = artist,
        album = album,
        durationSecs = 200u,
        thumbnailUrl = thumb,
        liked = false,
        albumId = albumId,
        isExplicit = false,
    )

    private fun card(
        id: String,
        title: String,
        subtitle: String = "",
        kind: AndroidHomeCardKind = AndroidHomeCardKind.ALBUM,
    ) = AndroidHomeCard(
        kind = kind,
        id = id,
        title = title,
        subtitle = subtitle,
        thumbnailUrl = null,
        track = null,
    )

    @Test
    fun primaryArtistExtractionPreservesCommaBandNamesAndStripsFeatures() {
        assertEquals("Tyler, The Creator", SonataViewModel.extractPrimaryArtistName("Tyler, The Creator (feat. Kali Uchis)"))
        assertEquals("Earth, Wind & Fire", SonataViewModel.extractPrimaryArtistName("Earth, Wind & Fire [Official Audio]"))
        assertEquals("The Weeknd", SonataViewModel.extractPrimaryArtistName("The Weeknd, Daft Punk feat. Gesaffelstein"))
        assertEquals("Daft Punk", SonataViewModel.extractPrimaryArtistName("Daft Punk & Pharrell Williams"))
        assertEquals("", SonataViewModel.extractPrimaryArtistName("   "))
    }

    @Test
    fun trackDeduplicationMatchesBothIdsAndCanonicalKeys() {
        val t1 = track(id = "id_1", title = "Blinding Lights (feat. Chromatics) [Official Audio]", artist = "The Weeknd feat. Chromatics")
        val t2 = track(id = "id_2", title = "Blinding Lights", artist = "The Weeknd")
        val t3 = track(id = "id_1", title = "Duplicate ID", artist = "Other Artist")
        val blankId = track(id = "", title = "Empty", artist = "Nobody")
        val distinct = track(id = "id_3", title = "Save Your Tears", artist = "The Weeknd")

        assertEquals("blinding lights|the weeknd", SonataViewModel.canonicalTrackKey(t1))
        assertEquals("blinding lights|the weeknd", SonataViewModel.canonicalTrackKey(t2))

        val deduped = SonataViewModel.deduplicateTracks(listOf(blankId, t1, t2, t3, distinct))
        assertEquals(listOf(t1, distinct), deduped)
    }

    @Test
    fun releaseMergingDeduplicatesAcrossNewReleaseAndAlbumsSinglesShelves() {
        assertTrue(isNewReleaseShelfTitle("New Releases"))
        assertTrue(isNewReleaseShelfTitle("New Albums & Singles"))
        assertFalse(isNewReleaseShelfTitle("Quick picks"))

        val shelves = listOf(
            AndroidHomeShelf(
                title = "New Releases",
                tracks = emptyList(),
                cards = listOf(
                    card(id = "VLMPRE_1", title = "Hurry Up Tomorrow", subtitle = "Album • The Weeknd"),
                    card(id = "mood_1", title = "Chill", subtitle = "", kind = AndroidHomeCardKind.MOOD),
                ),
            ),
            AndroidHomeShelf(
                title = "New Albums & Singles",
                tracks = emptyList(),
                cards = listOf(
                    card(id = "MPRE_1", title = "Hurry Up Tomorrow", subtitle = "Album • The Weeknd"),
                    card(id = "MPRE_2", title = "GNX", subtitle = "Album • Kendrick Lamar"),
                ),
            ),
            AndroidHomeShelf(
                title = "Forgotten Favorites",
                tracks = listOf(track(id = "old_1", title = "Old Song", artist = "Someone")),
                cards = emptyList(),
            ),
            AndroidHomeShelf(
                title = "Quick picks",
                tracks = listOf(
                    track(id = "qp_1", title = "Starboy", artist = "The Weeknd"),
                    track(id = "qp_2", title = "Starboy [Remix]", artist = "The Weeknd, Daft Punk"),
                ),
                cards = listOf(
                    card(id = "MPRE_2", title = "GNX", subtitle = "Album • Kendrick Lamar"),
                    card(id = "MPRE_3", title = "Chromakopia", subtitle = "Album • Tyler, The Creator"),
                ),
            ),
        )

        val normalized = SonataViewModel.normalizeAndMergeHomeShelves(shelves)
        assertEquals(listOf("New Releases", "Quick picks"), normalized.map { it.title })
        assertEquals(listOf("Hurry Up Tomorrow", "GNX"), normalized[0].cards.map { it.title })
        assertEquals(listOf("qp_1"), normalized[1].tracks.map { it.id })
        assertEquals(listOf("Chromakopia"), normalized[1].cards.map { it.title })
    }

    @Test
    fun radioMixNormalizationDeduplicatesAcrossShelvesAndDropsExcludedOnes() {
        val mixes = listOf(
            AndroidHomeShelf(
                title = "Featured Mixes",
                tracks = emptyList(),
                cards = listOf(
                    card(id = "VL_MIX_1", title = "Supermix", subtitle = "The Weeknd", kind = AndroidHomeCardKind.PLAYLIST),
                ),
            ),
            AndroidHomeShelf(
                title = "Unutulan Favoriler",
                tracks = emptyList(),
                cards = listOf(card(id = "MIX_OLD", title = "Ignored", kind = AndroidHomeCardKind.PLAYLIST)),
            ),
            AndroidHomeShelf(
                title = "Your Mixes",
                tracks = emptyList(),
                cards = listOf(
                    card(id = "_MIX_1", title = "Supermix", subtitle = "The Weeknd", kind = AndroidHomeCardKind.PLAYLIST),
                    card(id = "VL_MIX_2", title = "Discovery Mix", subtitle = "Daft Punk", kind = AndroidHomeCardKind.PLAYLIST),
                ),
            ),
        )

        val clean = SonataViewModel.normalizeRadioMixShelves(mixes)
        assertEquals(2, clean.size)
        assertEquals(listOf("Supermix"), clean[0].cards.map { it.title })
        assertEquals(listOf("Discovery Mix"), clean[1].cards.map { it.title })
    }

    @Test
    fun subtitleParsingAndCollectionTrackBackfillPreserveExistingMetadata() {
        assertEquals("The Weeknd", SonataViewModel.parseArtistFromCollectionSubtitle("Album • The Weeknd • 2020"))
        assertNull(SonataViewModel.parseArtistFromCollectionSubtitle("Album • 2020"))

        val tracks = SonataViewModel.backfillCollectionTracks(
            rawTracks = listOf(
                track(id = "a1", title = "Track One", artist = "", album = "", thumb = null, albumId = null),
                track(id = "a2", title = "Track Two", artist = "Featured Artist", album = "Deluxe", thumb = "https://t2.jpg", albumId = "MPRE_other"),
            ),
            id = "VLOLAK5uy_123",
            title = "After Hours",
            subtitle = "Album • The Weeknd • 2020",
            coverUrl = "https://cover.jpg",
        )

        assertEquals("The Weeknd", tracks[0].artist)
        assertEquals("After Hours", tracks[0].album)
        assertEquals("https://cover.jpg", tracks[0].thumbnailUrl)
        assertEquals("OLAK5uy_123", tracks[0].albumId)

        assertEquals("Featured Artist", tracks[1].artist)
        assertEquals("Deluxe", tracks[1].album)
        assertEquals("https://t2.jpg", tracks[1].thumbnailUrl)
        assertEquals("MPRE_other", tracks[1].albumId)
    }

    @Test
    fun streamQualityHelpersParseBitrateCheckUpgradesAndStripRanges() {
        assertEquals(320, SonataViewModel.parseBitrateKbps("AAC 320kbps · JioSaavn"))
        assertEquals(256, SonataViewModel.parseBitrateKbps("256 kbps Extreme"))
        assertEquals(0, SonataViewModel.parseBitrateKbps("Unknown"))

        assertTrue(SonataViewModel.shouldUpgradeStreamBadge("AAC 128kbps · Standard", signedIn = true))
        assertTrue(SonataViewModel.shouldUpgradeStreamBadge("AAC 192kbps · HQ", signedIn = true))
        assertFalse(SonataViewModel.shouldUpgradeStreamBadge("Opus 251 · Extreme", signedIn = true))
        assertFalse(SonataViewModel.shouldUpgradeStreamBadge("FLAC · Lossless", signedIn = false))
        assertFalse(SonataViewModel.shouldUpgradeStreamBadge("Opus 160kbps · HQ", signedIn = false))

        assertEquals(
            "https://example.com/playback?itag=140&foo=1",
            SonataViewModel.stripRangeQueryParam("https://example.com/playback?itag=140&range=0-65535&foo=1"),
        )
        assertEquals(
            "https://example.com/playback?foo=1",
            SonataViewModel.stripRangeQueryParam("https://example.com/playback?range=0-65535&foo=1"),
        )
        assertEquals(
            "https://example.com/playback?itag=140",
            SonataViewModel.stripRangeQueryParam("https://example.com/playback?itag=140&range=0-65535"),
        )
        assertEquals(
            "https://example.com/playback?itag=140",
            SonataViewModel.stripRangeQueryParam("https://example.com/playback?itag=140"),
        )
    }

    @Test
    fun playlistOwnershipNormalizationAndSaveableDetectionWorkAccurately() {
        assertEquals("PL123", normalizeCollectionId("VLPL123"))
        assertEquals("PL123", normalizeCollectionId("  PL123  "))
        assertEquals("LM", normalizeCollectionId("VLLM"))
        assertEquals("RDCLAK_mix", normalizeCollectionId("VLRDCLAK_mix"))
        assertEquals("", normalizeCollectionId("   "))

        val ownedIds = setOf("PL_OWNED")
        val savedIds = setOf("PL_COMMUNITY")

        // Only owned playlists when signed in should allow editing.
        assertTrue(isEditableUserPlaylist("VLPL_OWNED", signedIn = true, ownedPlaylistIds = ownedIds, savedExternalPlaylistIds = savedIds))
        assertTrue(isEditableUserPlaylist("PL_OWNED", signedIn = true, ownedPlaylistIds = ownedIds, savedExternalPlaylistIds = savedIds))
        assertFalse(isEditableUserPlaylist("VLPL_OWNED", signedIn = false, ownedPlaylistIds = ownedIds, savedExternalPlaylistIds = savedIds))
        assertFalse(isEditableUserPlaylist("VLPL_COMMUNITY", signedIn = true, ownedPlaylistIds = ownedIds, savedExternalPlaylistIds = savedIds))
        assertFalse(isEditableUserPlaylist("VLPL_EXTERNAL_UNKNOWN", signedIn = true, ownedPlaylistIds = ownedIds, savedExternalPlaylistIds = savedIds))
        assertFalse(isEditableUserPlaylist("VLLM", signedIn = true, ownedPlaylistIds = setOf("VLLM"), savedExternalPlaylistIds = savedIds))
        assertFalse(isEditableUserPlaylist("RDCLAK5uy_123", signedIn = true, ownedPlaylistIds = setOf("RDCLAK5uy_123"), savedExternalPlaylistIds = savedIds))

        // External playlists and albums are saveable; owned/local/downloads/LM are not.
        assertTrue(isSaveableExternalCollection("VLPL_COMMUNITY", isOwned = false))
        assertTrue(isSaveableExternalCollection("MPREb_album123", isOwned = false))
        assertTrue(isSaveableExternalCollection("RDCLAK5uy_mix", isOwned = false))
        assertFalse(isSaveableExternalCollection("VLPL_OWNED", isOwned = true))
        assertFalse(isSaveableExternalCollection("__sonata_downloads__", isOwned = false))
        assertFalse(isSaveableExternalCollection("local_pl_123", isOwned = false))
        assertFalse(isSaveableExternalCollection("VLLM", isOwned = false))
    }

    @Test
    fun videoQualitySelectionHonorsAutoCapExplicitHeightAndFallback() {
        val qualities = listOf(
            AndroidVideoQualityStream("360p", 360u, 30u, "https://v/360", "video/mp4", 500_000u, true),
            AndroidVideoQualityStream("480p", 480u, 30u, "https://v/480", "video/mp4", 900_000u, false),
            AndroidVideoQualityStream("720p60", 720u, 60u, "https://v/720", "video/mp4", 1_800_000u, false),
            AndroidVideoQualityStream("1080p", 1080u, 30u, "https://v/1080", "video/mp4", 2_500_000u, false),
        )

        assertNull(SonataViewModel.selectVideoQualityStream(emptyList(), VideoQualityPreference.AUTO))
        assertEquals(
            1080u,
            SonataViewModel.selectVideoQualityStream(qualities, VideoQualityPreference.AUTO)?.height,
        )
        assertEquals(
            720u,
            SonataViewModel.selectVideoQualityStream(qualities, VideoQualityPreference.P720)?.height,
        )
        assertEquals(
            480u,
            SonataViewModel.selectVideoQualityStream(qualities, VideoQualityPreference.P480)?.height,
        )
        assertEquals(
            720u,
            SonataViewModel.selectVideoQualityStream(qualities, VideoQualityPreference.AUTO, explicitHeight = 720)?.height,
        )
        // Fallback when only higher resolutions exist
        val hdOnly = qualities.takeLast(2)
        assertEquals(
            720u,
            SonataViewModel.selectVideoQualityStream(hdOnly, VideoQualityPreference.P360)?.height,
        )
    }

    @Test
    fun musicVideoSyncMappingHandlesIntrosInterludesAndRoundTrips() {
        assertEquals(0L, mapMusicToVideoMs(-50L, emptyList()))
        assertEquals(42_000L, mapMusicToVideoMs(42_000L, emptyList()))
        assertEquals(42_000L, mapVideoToMusicMs(42_000L, emptyList()))

        // Intro silence 0..18.5s in video, verse/chorus 1 (92s), skit 110.5..125s, rest of song (110s)
        val segments = listOf(
            AndroidMusicVideoSyncSegment(musicStartMs = 0uL, videoStartMs = 18_500uL, durationMs = 92_000uL),
            AndroidMusicVideoSyncSegment(musicStartMs = 92_000uL, videoStartMs = 125_000uL, durationMs = 110_000uL),
        )

        // Switching to video at start of music lands at start of song inside video (18.5s)
        assertEquals(18_500L, mapMusicToVideoMs(0L, segments))
        // Mid-first-segment seeks cleanly
        assertEquals(68_500L, mapMusicToVideoMs(50_000L, segments))
        // Second segment skips mid-video pause
        assertEquals(133_000L, mapMusicToVideoMs(100_000L, segments))

        // During silent intro in video, lyrics wait cleanly at 0ms
        assertEquals(0L, mapVideoToMusicMs(10_000L, segments))
        // Inside video first segment, maps back to music timeline
        assertEquals(50_000L, mapVideoToMusicMs(68_500L, segments))
        // Inside mid-video interlude (115s in video), holds cleanly at upcoming music boundary (92s)
        assertEquals(92_000L, mapVideoToMusicMs(115_000L, segments))
        // Past final music segment (outro credits), clamps at end of music (202s)
        assertEquals(202_000L, mapVideoToMusicMs(250_000L, segments))
        // Round-trip inside any music segment preserves timestamp
        assertEquals(125_000L, mapVideoToMusicMs(mapMusicToVideoMs(125_000L, segments), segments))
    }
}
