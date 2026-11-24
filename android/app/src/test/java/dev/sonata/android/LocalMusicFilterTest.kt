package dev.sonata.android

import dev.sonata.android.data.LocalMusicScanner
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalMusicFilterTest {

    @Test
    fun excludesWhatsAppVoiceNotesCallRecordingsSystemTonesAndShortClips() {
        assertTrue(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "AUD-20261014-WA0005",
                relativePath = "Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Audio/",
                folderName = "WhatsApp Audio",
                durationMs = 29_000L,
            ),
        )
        assertTrue(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "PTT-20260401-WA0002",
                relativePath = "Music/",
                folderName = "Music",
                durationMs = 45_000L,
            ),
        )
        assertTrue(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "Over the Horizon",
                relativePath = "Music/Samsung/",
                folderName = "Samsung",
                durationMs = 190_000L,
            ),
        )
        assertTrue(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "Note",
                relativePath = "Recordings/Call/",
                folderName = "Call",
                durationMs = 120_000L,
            ),
        )
        assertTrue(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "Short Effect",
                relativePath = "Music/",
                folderName = "Music",
                durationMs = 12_000L,
                minDurationMs = 30_000L,
            ),
        )
    }

    @Test
    fun preservesNormalMusicTracks() {
        assertFalse(
            LocalMusicScanner.isExcludedNonMusicAudio(
                title = "Instant Crush",
                relativePath = "Music/FLAC/Daft Punk/",
                folderName = "Daft Punk",
                durationMs = 337_000L,
            ),
        )
    }

    @Test
    fun selectedFoldersMatchExactAndSubfolderPrefixes() {
        assertTrue(LocalMusicScanner.matchesSelectedFolders("Music/Pop", "Pop", emptySet()))
        assertTrue(LocalMusicScanner.matchesSelectedFolders("Music/FLAC/2025/", "2025", setOf("Music/FLAC")))
        assertFalse(LocalMusicScanner.matchesSelectedFolders("Download/", "Download", setOf("Music/FLAC")))
    }

    @Test
    fun folderPathFromTreeDocumentIdExtractsRelativePath() {
        assertEquals("Music/FLAC", LocalMusicScanner.folderPathFromTreeDocumentId("primary:Music/FLAC"))
        assertEquals("Audio", LocalMusicScanner.folderPathFromTreeDocumentId("1234-5678:Audio/"))
        assertNull(LocalMusicScanner.folderPathFromTreeDocumentId("primary:"))
        assertNull(LocalMusicScanner.folderPathFromTreeDocumentId(null))
    }

    @Test
    fun backupSerializationPreservesLocalMusicFilterAndFolders() {
        val state = SonataPreferences.sanitize(
            SonataSettingsState(
                excludeNonMusicAudio = true,
                localMusicMinDurationSecs = 45,
                localMusicFolders = setOf("/Music/FLAC/", "music/flac", "Audio\\HiRes"),
            ),
        )
        assertEquals(setOf("Audio/HiRes", "Music/FLAC"), state.localMusicFolders)
        val json = SonataPreferences.formatBackupJson(state, emptyMap(), exportedAtMs = 1234L)
        assertTrue(json.contains("\"exclude_non_music_audio\": true"))
        assertTrue(json.contains("\"local_music_min_duration_secs\": 45"))
        assertTrue(json.contains("Music/FLAC"))
        assertTrue(json.contains("Audio/HiRes"))
    }
}
