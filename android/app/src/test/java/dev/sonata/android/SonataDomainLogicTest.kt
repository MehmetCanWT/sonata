package dev.sonata.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidTrack

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
}
