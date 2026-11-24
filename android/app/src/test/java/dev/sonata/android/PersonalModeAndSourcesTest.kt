package dev.sonata.android

import android.Manifest
import android.content.Context
import android.content.ContextWrapper
import android.content.SharedPreferences
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.LocalAudioTrack
import dev.sonata.android.data.LocalMusicScanner
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.data.resolveLocalBitrateKbps
import dev.sonata.android.ui.i18n.EnglishSourcesStrings
import dev.sonata.android.ui.i18n.RussianSourcesStrings
import dev.sonata.android.ui.i18n.TurkishSourcesStrings
import dev.sonata.android.ui.i18n.sourcesStrings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Locale

class PersonalModeAndSourcesTest {

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

        override fun registerOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) = Unit
        override fun unregisterOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) = Unit
    }

    private class FakePreferencesContext : ContextWrapper(null) {
        private val stores = HashMap<String, InMemorySharedPreferences>()

        override fun getApplicationContext(): Context = this

        override fun getSharedPreferences(name: String?, mode: Int): SharedPreferences =
            stores.getOrPut(name.orEmpty()) { InMemorySharedPreferences() }
    }

    @Test
    fun `personal only mode preserves jioSaavnEnabled and audio settings across ON and OFF toggles`() {
        val context = FakePreferencesContext()
        val prefs = SonataPreferences(context)

        // Default values
        assertFalse(prefs.personalOnlyMode.value)
        assertTrue(prefs.localMusicEnabled.value)
        assertFalse(prefs.attributePersonalToYtm.value)
        assertTrue(SonataPreferences.isStreamingEnabled(personalOnly = false))
        assertFalse(SonataPreferences.isStreamingEnabled(personalOnly = true))
        assertFalse(SonataPreferences.effectiveJioSaavnEnabled(personalOnly = true, jioSaavnEnabled = true))
        assertTrue(SonataPreferences.effectiveJioSaavnEnabled(personalOnly = false, jioSaavnEnabled = true))

        // Configure streaming & audio preferences before enabling Personal Only Mode
        prefs.setDiscordToken("super-secret-discord-token")
        prefs.update {
            it.copy(
                jioSaavnEnabled = true,
                audioQuality = AudioQualityTier.LOSSLESS,
                crossfadeEnabled = true,
                crossfadeSeconds = 7,
                playbackSpeed = 1.25f,
                eqEnabled = true,
                eqPreset = "Studio",
            )
        }
        prefs.setAttributePersonalToYtm(true)

        // Enable Personal Only Mode
        prefs.setPersonalOnlyMode(true)
        assertTrue(prefs.personalOnlyMode.value)
        assertTrue(prefs.state.value.personalOnlyMode)
        assertTrue("jioSaavnEnabled must remain preserved", prefs.state.value.jioSaavnEnabled)
        assertFalse("effective JioSaavn must pause while personalOnlyMode=true", prefs.state.value.effectiveJioSaavnEnabled())
        assertFalse(prefs.state.value.isStreamingEnabled())
        assertEquals(AudioQualityTier.LOSSLESS, prefs.state.value.audioQuality)
        assertTrue(prefs.state.value.crossfadeEnabled)
        assertEquals(7, prefs.state.value.crossfadeSeconds)
        assertEquals(1.25f, prefs.state.value.playbackSpeed, 0.001f)
        assertTrue(prefs.state.value.eqEnabled)
        assertEquals("Studio", prefs.state.value.eqPreset)
        assertTrue(prefs.attributePersonalToYtm.value)

        // Disable Personal Only Mode -> prior streaming & audio settings remain intact
        prefs.setPersonalOnlyMode(false)
        assertFalse(prefs.personalOnlyMode.value)
        assertTrue(prefs.state.value.jioSaavnEnabled)
        assertTrue(prefs.state.value.effectiveJioSaavnEnabled())
        assertTrue(prefs.state.value.isStreamingEnabled())
        assertEquals(AudioQualityTier.LOSSLESS, prefs.state.value.audioQuality)
        assertEquals("Studio", prefs.state.value.eqPreset)
        assertEquals("super-secret-discord-token", prefs.getDiscordToken())

        // Verify backup JSON includes new source preferences while excluding secrets
        prefs.setPersonalOnlyMode(true)
        prefs.setLocalMusicEnabled(false)
        val backup = prefs.exportBackupJson()
        val lowered = backup.lowercase(Locale.ROOT)
        assertTrue(backup.contains("\"personal_only_mode\": true"))
        assertTrue(backup.contains("\"local_music_enabled\": false"))
        assertTrue(backup.contains("\"attribute_personal_to_ytm\": true"))
        assertFalse(lowered.contains("super-secret-discord-token"))
        assertFalse(lowered.contains("cookie"))
        assertFalse(lowered.contains("token"))
        assertFalse(lowered.contains("secret"))

        // Verify importBackupJson restores source preferences cleanly
        val restoredPrefs = SonataPreferences(FakePreferencesContext())
        assertTrue(restoredPrefs.importBackupJson(backup))
        assertTrue(restoredPrefs.personalOnlyMode.value)
        assertFalse(restoredPrefs.localMusicEnabled.value)
        assertTrue(restoredPrefs.attributePersonalToYtm.value)
        assertTrue(restoredPrefs.state.value.jioSaavnEnabled)
    }

    @Test
    fun `isPersonalTrackId classifies local and self-hosted prefixes accurately`() {
        assertTrue(isPersonalTrackId("local:42"))
        assertTrue(isPersonalTrackId("LOCAL:1001"))
        assertTrue(isPersonalTrackId("jellyfin:srv-1:item-99"))
        assertTrue(isPersonalTrackId("subsonic:nav-1:trk-5"))
        assertTrue(isPersonalTrackId("smb:nas:/Music/track.flac"))
        assertTrue(isPersonalTrackId("webdav:cloud:/Audio/song.opus"))
        assertTrue(isPersonalTrackId("  smb:share/song.mp3  "))

        assertFalse(isPersonalTrackId("dQw4w9WgXcQ"))
        assertFalse(isPersonalTrackId("jiosaavn:xyz123"))
        assertFalse(isPersonalTrackId(""))
        assertFalse(isPersonalTrackId("   "))
        assertFalse(isPersonalTrackId("https://music.youtube.com/watch?v=dQw4w9WgXcQ"))
    }

    @Test
    fun `sourcesStrings are complete and localized across English, Turkish, and Russian`() {
        val en = sourcesStrings(AppLanguage.ENGLISH)
        val tr = sourcesStrings(AppLanguage.TURKISH)
        val ru = sourcesStrings(AppLanguage.RUSSIAN)

        assertEquals(EnglishSourcesStrings, en)
        assertEquals(TurkishSourcesStrings, tr)
        assertEquals(RussianSourcesStrings, ru)
        assertEquals(tr, sourcesStrings(AppLanguage.SYSTEM, Locale("tr", "TR")))
        assertEquals(ru, sourcesStrings(AppLanguage.SYSTEM, Locale("ru", "RU")))
        assertEquals(en, sourcesStrings(AppLanguage.SYSTEM, Locale.US))

        assertEquals("Sources", en.sourcesTitle)
        assertEquals("Kaynaklar", tr.sourcesTitle)
        assertEquals("Modules", en.modulesTitle)
        assertEquals("Modüller", tr.modulesTitle)
        assertEquals("Personal Only Mode", en.personalOnlyModeTitle)
        assertEquals("Yalnızca Kişisel Mod", tr.personalOnlyModeTitle)
        assertEquals("Speed Test", en.speedTest)
        assertEquals("Hız Testi", tr.speedTest)

        listOf(en, tr, ru).forEach { s ->
            assertTrue(s.sourcesTitle.isNotBlank())
            assertTrue(s.sourcesSubtitle.isNotBlank())
            assertTrue(s.modulesTitle.isNotBlank())
            assertTrue(s.modulesSubtitle.isNotBlank())
            assertTrue(s.jioSaavnModuleTitle.isNotBlank())
            assertTrue(s.jioSaavnModuleDescription.isNotBlank())
            assertTrue(s.jioSaavnPausedByPersonalMode.isNotBlank())
            assertTrue(s.personalOnlyModeTitle.isNotBlank())
            assertTrue(s.personalOnlyModeDescription.isNotBlank())
            assertTrue(s.personalOnlyModeBadge.isNotBlank())
            assertTrue(s.localMusicTitle.isNotBlank())
            assertTrue(s.localMusicDescription.isNotBlank())
            assertTrue(s.localMusicPermissionTitle.isNotBlank())
            assertTrue(s.localMusicPermissionSubtitle.isNotBlank())
            assertTrue(s.grantPermission.isNotBlank())
            assertTrue(s.rescanLocalMusic.isNotBlank())
            assertTrue(s.localTracksTab.isNotBlank())
            assertTrue(s.localAlbumsTab.isNotBlank())
            assertTrue(s.localFoldersTab.isNotBlank())
            assertTrue(s.localTracksCount(5).contains("5"))
            assertTrue(s.emptyLocalMusic.isNotBlank())
            assertTrue(s.selfHostedServersTitle.isNotBlank())
            assertTrue(s.selfHostedServersDescription.isNotBlank())
            assertEquals("Jellyfin", s.protocolJellyfin)
            assertEquals("Subsonic", s.protocolSubsonic)
            assertEquals("SMB", s.protocolSmb)
            assertEquals("WebDAV", s.protocolWebDav)
            assertTrue(s.smbShareLabel.isNotBlank())
            assertTrue(s.smbDomainLabel.isNotBlank())
            assertTrue(s.statusAlive.isNotBlank())
            assertTrue(s.statusOnline.isNotBlank())
            assertTrue(s.statusOffline.isNotBlank())
            assertTrue(s.statusChecking.isNotBlank())
            assertTrue(s.checkHealth.isNotBlank())
            assertTrue(s.speedTest.isNotBlank())
            assertTrue(s.runningSpeedTest.isNotBlank())
            assertTrue(s.latencyMs(42).contains("42"))
            assertTrue(s.throughputMbps(125.4).contains("125.4"))
            assertTrue(s.speedTestResult(18, 92.5).contains("18"))
            assertTrue(s.speedTestResult(18, null).contains("18"))
            assertTrue(s.personalPlaylistsTitle.isNotBlank())
            assertTrue(s.addPersonalPlaylist.isNotBlank())
            assertTrue(s.removePersonalPlaylist.isNotBlank())
            assertTrue(s.addToPersonalPlaylist.isNotBlank())
            assertTrue(s.removeFromPersonalPlaylist.isNotBlank())
            assertTrue(s.personalPlaylistCreated("Mix").contains("Mix"))
            assertTrue(s.personalPlaylistRemoved("Mix").contains("Mix"))
            assertTrue(s.tabAll.isNotBlank())
            assertEquals("YouTube", s.tabYouTube)
            assertTrue(s.tabLocal.isNotBlank())
            assertEquals("Jellyfin", s.tabJellyfin)
            assertEquals("Subsonic", s.tabSubsonic)
            assertEquals("SMB", s.tabSmb)
            assertEquals("WebDAV", s.tabWebDav)
            assertTrue(s.attributePersonalToYtmTitle.isNotBlank())
            assertTrue(s.attributePersonalToYtmDescription.isNotBlank())
        }

        assertFalse(en.sourcesTitle == tr.sourcesTitle)
        assertFalse(en.sourcesTitle == ru.sourcesTitle)
        assertFalse(tr.sourcesTitle == ru.sourcesTitle)
        assertFalse(en.tabLocal == tr.tabLocal)
        assertFalse(en.tabLocal == ru.tabLocal)
    }

    @Test
    fun `local bitrate helper math and scanner grouping behave accurately`() {
        // Prefers positive MediaStore bitrate (bps -> kbps)
        assertEquals(320, resolveLocalBitrateKbps(mediaStoreBitrateBps = 320_000, fileSizeBytes = 10_000_000L, durationMs = 200_000L))
        // Falls back to exact (size * 8) / durationMs when MediaStore omits bitrate (0 or -1)
        assertEquals(880, resolveLocalBitrateKbps(mediaStoreBitrateBps = 0, fileSizeBytes = 44_000_000L, durationMs = 400_000L))
        assertEquals(1411, resolveLocalBitrateKbps(mediaStoreBitrateBps = -1, fileSizeBytes = 35_280_000L, durationMs = 200_000L))
        // Zero/invalid size or duration returns 0 safely
        assertEquals(0, resolveLocalBitrateKbps(mediaStoreBitrateBps = 0, fileSizeBytes = 0L, durationMs = 200_000L))
        assertEquals(0, resolveLocalBitrateKbps(mediaStoreBitrateBps = 0, fileSizeBytes = 10_000_000L, durationMs = 0L))

        // Permission selector by Android SDK level
        assertEquals(Manifest.permission.READ_MEDIA_AUDIO, LocalMusicScanner.requiredReadAudioPermission(33))
        assertEquals(Manifest.permission.READ_MEDIA_AUDIO, LocalMusicScanner.requiredReadAudioPermission(35))
        assertEquals(Manifest.permission.READ_EXTERNAL_STORAGE, LocalMusicScanner.requiredReadAudioPermission(32))
        assertEquals(Manifest.permission.READ_EXTERNAL_STORAGE, LocalMusicScanner.requiredReadAudioPermission(31))

        // URI, folder, and grouping helpers
        assertEquals("local:77", LocalMusicScanner.buildLocalTrackId(77L))
        assertEquals("content://media/external/audio/albumart/99", LocalMusicScanner.buildLocalAlbumArtUri(99L))
        assertNull(LocalMusicScanner.buildLocalAlbumArtUri(0L))
        assertEquals("Lossless", LocalMusicScanner.extractFolderName(null, "Music/Lossless/"))
        assertEquals("Favorites", LocalMusicScanner.extractFolderName("Favorites", "Music/Lossless/"))

        val t1 = LocalAudioTrack(
            mediaId = 10L,
            id = LocalMusicScanner.buildLocalTrackId(10L),
            title = "Second Song",
            artist = "Artist A",
            album = "Album One",
            albumId = 500L,
            contentUri = "content://media/external/audio/media/10",
            albumArtUri = LocalMusicScanner.buildLocalAlbumArtUri(500L),
            durationMs = 180_000L,
            fileSizeBytes = 19_800_000L,
            bitrateKbps = resolveLocalBitrateKbps(0, 19_800_000L, 180_000L),
            folderName = "Lossless",
            relativePath = "Music/Lossless/",
            mimeType = "audio/flac",
            trackNumber = 2,
        )
        val t2 = LocalAudioTrack(
            mediaId = 11L,
            id = LocalMusicScanner.buildLocalTrackId(11L),
            title = "First Song",
            artist = "Artist A",
            album = "Album One",
            albumId = 500L,
            contentUri = "content://media/external/audio/media/11",
            albumArtUri = LocalMusicScanner.buildLocalAlbumArtUri(500L),
            durationMs = 120_000L,
            fileSizeBytes = 13_200_000L,
            bitrateKbps = resolveLocalBitrateKbps(0, 13_200_000L, 120_000L),
            folderName = "Lossless",
            relativePath = "Music/Lossless/",
            mimeType = "audio/flac",
            trackNumber = 1,
        )

        val lib = LocalMusicScanner.groupLocalMusicLibrary(listOf(t1, t2))
        assertEquals(2, lib.tracks.size)
        assertEquals(1, lib.albums.size)
        assertEquals(1, lib.folders.size)
        assertEquals(listOf(11L, 10L), lib.albums.first().tracks.map { it.mediaId })
        assertEquals(300_000L, lib.albums.first().totalDurationMs)
        assertEquals("Lossless", lib.folders.first().folderName)

        val androidTrack = t2.toAndroidTrack()
        assertEquals("local:11", androidTrack.id)
        assertEquals(120uL, androidTrack.durationSecs)
        assertNotNull(androidTrack.thumbnailUrl)
        assertEquals("local:album:500", androidTrack.albumId)

        // Missing permission / fake context scan returns empty cleanly without throwing
        val emptyResult = LocalMusicScanner.scan(FakePreferencesContext())
        assertFalse(emptyResult.permissionGranted)
        assertTrue(emptyResult.tracks.isEmpty())
    }
}
