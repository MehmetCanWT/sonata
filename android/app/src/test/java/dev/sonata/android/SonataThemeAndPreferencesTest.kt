package dev.sonata.android

import androidx.compose.ui.graphics.Color
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.DownloadStorageLocation
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.ui.theme.BasicDarkTokens
import dev.sonata.android.ui.theme.BasicLightTokens
import dev.sonata.android.ui.theme.lunariaColorScheme
import dev.sonata.android.ui.theme.lunariaTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SonataThemeAndPreferencesTest {

    @Test
    fun defaultSettingsMatchBasicSystemTheme() {
        val defaults = SonataSettingsState()
        assertEquals(DesignTheme.BASIC, defaults.designTheme)
        assertEquals(ThemeMode.SYSTEM, defaults.themeMode)
        assertEquals(DesignTheme.BASIC, defaults.resolvedDesignTheme())
        assertFalse(defaults.isDarkActive(systemDark = false))
        assertTrue(defaults.isDarkActive(systemDark = true))
        assertTrue(defaults.liquidGlassEnabled)
        assertEquals(AudioQualityTier.HIGH, defaults.audioQuality)
        assertEquals(DownloadQualityTier.HIGH, defaults.downloadQuality)
        assertEquals(PlayerBackdropMode.ARTWORK_MESH, defaults.playerBackdropMode)
    }

    @Test
    fun explicitThemeModesControlLightAndDarkAppearanceInBasicTheme() {
        val light = SonataSettingsState(themeMode = ThemeMode.LIGHT)
        assertEquals(DesignTheme.BASIC, light.resolvedDesignTheme())
        assertFalse(light.isDarkActive(systemDark = true))

        val dark = SonataSettingsState(themeMode = ThemeMode.DARK)
        assertEquals(DesignTheme.BASIC, dark.resolvedDesignTheme())
        assertTrue(dark.isDarkActive(systemDark = false))

        val system = SonataSettingsState(themeMode = ThemeMode.SYSTEM)
        assertEquals(DesignTheme.BASIC, system.resolvedDesignTheme())
        assertTrue(system.isDarkActive(systemDark = true))
        assertFalse(system.isDarkActive(systemDark = false))
    }

    @Test
    fun legacyThemeModeStringsMigrateCleanlyToBasicLightOrDark() {
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("DARK", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.LIGHT, SonataPreferences.parseThemeMode(" light ", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("ELECTRIC_LAVENDER", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.LIGHT, SonataPreferences.parseThemeMode("IMPERIAL_VIOLET", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("VIOLET_DARK", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.LIGHT, SonataPreferences.parseThemeMode("VIOLET_LIGHT", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("MOONFLOWER_DARK", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.LIGHT, SonataPreferences.parseThemeMode("MOONFLOWER_LIGHT", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("BASIC_DARK", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.LIGHT, SonataPreferences.parseThemeMode("BASIC_LIGHT", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.DARK, SonataPreferences.parseThemeMode("AMOLED", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.SYSTEM, SonataPreferences.parseThemeMode("unknown_value", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.SYSTEM, SonataPreferences.parseThemeMode(null, ThemeMode.SYSTEM))
    }

    @Test
    fun sanitizeClampsOutOfRangeValuesAndNormalizesStrings() {
        val dirty = SonataSettingsState(
            fpsLimit = 144,
            crossfadeSeconds = 99,
            playbackSpeed = 5.0f,
            eqPreset = "   ",
            eqBandsDb = listOf(-40f, Float.NaN, 3.5f, 50f),
            lyricsTranslationLang = "  TR ",
            lyricsOffsetMs = -99_000L,
        )
        val clean = SonataPreferences.sanitize(dirty)
        assertEquals(120, clean.fpsLimit)
        assertEquals(12, clean.crossfadeSeconds)
        assertEquals(2.0f, clean.playbackSpeed, 0.001f)
        assertEquals("Flat", clean.eqPreset)
        assertEquals(listOf(-12f, 0f, 3.5f, 12f, 0f), clean.eqBandsDb)
        assertEquals("tr", clean.lyricsTranslationLang)
        assertEquals(-30_000L, clean.lyricsOffsetMs)

        val slow = SonataPreferences.sanitize(
            SonataSettingsState(
                fpsLimit = 45,
                crossfadeSeconds = 0,
                playbackSpeed = Float.POSITIVE_INFINITY,
                lyricsTranslationLang = " ",
                lyricsOffsetMs = 45_000L,
            ),
        )
        assertEquals(60, slow.fpsLimit)
        assertEquals(1, slow.crossfadeSeconds)
        assertEquals(1.0f, slow.playbackSpeed, 0.001f)
        assertEquals("en", slow.lyricsTranslationLang)
        assertEquals(30_000L, slow.lyricsOffsetMs)
    }

    @Test
    fun eqBandsAndTrackOffsetsJsonHelpersRoundtrip() {
        val bands = listOf(-3.5f, 0f, 2.5f, 4.0f, -1.0f)
        val encodedBands = SonataPreferences.encodeBands(bands)
        val decodedBands = SonataPreferences.parseBands(encodedBands, listOf(0f, 0f, 0f, 0f, 0f))
        assertEquals(5, decodedBands.size)
        for (i in bands.indices) {
            assertEquals(bands[i], decodedBands[i], 0.01f)
        }

        val fallback = listOf(1f, 1f, 1f, 1f, 1f)
        assertEquals(fallback, SonataPreferences.parseBands("not json", fallback))
        assertEquals(fallback, SonataPreferences.parseBands("", fallback))

        val encodedOffsets = SonataPreferences.encodeTrackOffsets(
            mapOf(
                "vid_ok" to 750L,
                "vid_zero" to 0L,
                "" to 500L,
                "vid_high" to 99_000L,
            ),
        )
        assertTrue(encodedOffsets.contains("\"vid_ok\":750"))
        assertTrue(encodedOffsets.contains("\"vid_high\":30000"))
        assertFalse(encodedOffsets.contains("vid_zero"))
    }

    @Test
    fun basicThemeLightAndDarkUseMonochromeSurfacesAndNeutralLiquidGlass() {
        assertEquals(BasicLightTokens, lunariaTokens(darkTheme = false, amoled = false, designTheme = DesignTheme.BASIC))
        assertEquals(BasicDarkTokens, lunariaTokens(darkTheme = true, amoled = false, designTheme = DesignTheme.BASIC))
        assertEquals(BasicDarkTokens, lunariaTokens(darkTheme = true, amoled = true, designTheme = DesignTheme.BASIC))
        assertEquals(Color.Black, BasicDarkTokens.eclipseBase)
        assertEquals(Color.White, BasicLightTokens.eclipseBase)
        assertEquals(Color.Black, lunariaColorScheme(darkTheme = true, amoled = false, designTheme = DesignTheme.BASIC).background)
        assertEquals(Color.White, lunariaColorScheme(darkTheme = false, amoled = false, designTheme = DesignTheme.BASIC).background)

        // Crystal-clear neutral Liquid Glass (zero pink/purple tint) in both Light & Dark.
        for (token in listOf(BasicLightTokens, BasicDarkTokens)) {
            assertEquals("Liquid Glass tint must be neutral (R == G)", token.glassTint.red, token.glassTint.green, 0.001f)
            assertEquals("Liquid Glass tint must be neutral (G == B)", token.glassTint.green, token.glassTint.blue, 0.001f)
            assertEquals("Glass pill indicator must be pure white", 1f, token.pillIndicatorColor.red, 0.001f)
            assertEquals("Glass pill indicator must be pure white", 1f, token.pillIndicatorColor.green, 0.001f)
            assertEquals("Glass pill indicator must be pure white", 1f, token.pillIndicatorColor.blue, 0.001f)
            assertTrue("Liquid Glass pill should remain translucent", token.pillIndicatorColor.alpha in 0.10f..0.55f)
        }
    }

    @Test
    fun backupJsonExcludesAllSessionCookiesAndCredentials() {
        val json = SonataPreferences.formatBackupJson(
            current = SonataSettingsState(jioSaavnEnabled = true),
            offsets = mapOf("vid_123" to 250L),
            exportedAtMs = 1_700_000_000_000L,
        )
        val lowered = json.lowercase()
        val forbiddenTokens = listOf(
            "cookie",
            "sapisid",
            "sp_dc",
            "authorization",
            "bearer",
            "ytm_session",
            "password",
            "secret",
            "token",
        )
        for (token in forbiddenTokens) {
            assertFalse("Backup JSON must never contain credential field `$token`", lowered.contains(token))
        }
        assertTrue(json.contains("\"jiosaavn_enabled\": true"))
        assertTrue(json.contains("\"vid_123\": 250"))
    }

    @Test
    fun basicThemeTokensAndDownloadSettingsBehaveAsExpected() {
        assertEquals(BasicDarkTokens, lunariaTokens(darkTheme = true, amoled = false, designTheme = DesignTheme.BASIC))
        assertEquals(BasicLightTokens, lunariaTokens(darkTheme = false, amoled = false, designTheme = DesignTheme.BASIC))
        assertTrue(BasicDarkTokens.isBasic)
        assertFalse(BasicDarkTokens.isViolet)
        assertFalse(BasicDarkTokens.isMoonflower)
        assertEquals(Color.Black, BasicDarkTokens.eclipseBase)
        assertEquals(Color.White, BasicLightTokens.eclipseBase)

        val basicDark = SonataSettingsState(themeMode = ThemeMode.DARK)
        assertEquals(DesignTheme.BASIC, basicDark.resolvedDesignTheme())
        assertTrue(basicDark.isDarkActive(systemDark = false))

        val basicLight = SonataSettingsState(themeMode = ThemeMode.LIGHT)
        assertEquals(DesignTheme.BASIC, basicLight.resolvedDesignTheme())
        assertFalse(basicLight.isDarkActive(systemDark = true))

        // Download quality tiers ordered strictly Highest -> Lowest with MB/song metadata.
        val tiers = DownloadQualityTier.entries
        assertEquals(DownloadQualityTier.HIGHEST, tiers.first())
        assertEquals(DownloadQualityTier.LOW, tiers.last())
        for (i in 0 until tiers.lastIndex) {
            assertTrue(tiers[i].maxBitrateKbps > tiers[i + 1].maxBitrateKbps)
            assertTrue(tiers[i].approxMbPerSong.contains("MB"))
        }
        assertTrue(tiers.last().approxMbPerSong.contains("MB"))

        // Storage location helper & backup serialization (including extended Appearance/Content/Misc fields).
        val externalState = SonataSettingsState(
            downloadQuality = DownloadQualityTier.HIGHEST,
            downloadIncludeJioSaavn = true,
            downloadStorageLocation = DownloadStorageLocation.EXTERNAL,
            hideRecents = true,
            hideTopPicks = true,
            hideHomeMoods = true,
            hideHomeVideos = true,
            squareVideoThumbnails = true,
            fullScreenCoverArt = false,
            fullScreenDetailCover = true,
            showTotalDuration = true,
            animatedArtworkCellular = false,
            hideLyricsPreview = true,
            sleepTimerFadeOut = false,
            hideStatusBarInPlayer = true,
            hideSongStatus = true,
            stopMusicOnClose = true,
            showNerdStats = true,
        )
        assertTrue(externalState.usesExternalDownloadStorage())
        val backup = SonataPreferences.formatBackupJson(externalState, emptyMap(), 1_700_000_000_000L)
        assertTrue(backup.contains("\"download_include_jiosaavn\": true"))
        assertTrue(backup.contains("\"download_storage_location\": \"EXTERNAL\""))
        assertTrue(backup.contains("\"hide_recents\": true"))
        assertTrue(backup.contains("\"hide_top_picks\": true"))
        assertTrue(backup.contains("\"square_video_thumbnails\": true"))
        assertTrue(backup.contains("\"full_screen_cover_art\": false"))
        assertTrue(backup.contains("\"full_screen_detail_cover\": true"))
        assertTrue(backup.contains("\"show_total_duration\": true"))
        assertTrue(backup.contains("\"hide_lyrics_preview\": true"))
        assertTrue(backup.contains("\"sleep_timer_fade_out\": false"))
        assertTrue(backup.contains("\"hide_status_bar_in_player\": true"))
        assertTrue(backup.contains("\"stop_music_on_close\": true"))
        assertTrue(backup.contains("\"show_nerd_stats\": true"))
        assertTrue(backup.contains("\"app_language\": \"SYSTEM\""))
    }

    @Test
    fun androidLanguageDefaultsToSystemAndResolvesTurkishAndEnglish() {
        assertEquals(dev.sonata.android.data.AppLanguage.SYSTEM, SonataSettingsState().appLanguage)

        val trSystem = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.SYSTEM,
            java.util.Locale("tr", "TR"),
        )
        assertEquals("Ana Sayfa", trSystem.tabHome)
        assertEquals("Ayarlar", trSystem.settingsTitle)

        val enSystem = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.SYSTEM,
            java.util.Locale.US,
        )
        assertEquals("Home", enSystem.tabHome)
        assertEquals("Settings", enSystem.settingsTitle)

        val forcedEn = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.ENGLISH,
            java.util.Locale("tr", "TR"),
        )
        assertEquals("Home", forcedEn.tabHome)

        val forcedTr = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.TURKISH,
            java.util.Locale.US,
        )
        assertEquals("Ana Sayfa", forcedTr.tabHome)

        val ruSystem = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.SYSTEM,
            java.util.Locale("ru", "RU"),
        )
        assertEquals("Главная", ruSystem.tabHome)
        assertEquals("Настройки", ruSystem.settingsTitle)

        val forcedRu = dev.sonata.android.ui.i18n.resolveSonataStrings(
            dev.sonata.android.data.AppLanguage.RUSSIAN,
            java.util.Locale.US,
        )
        assertEquals("Главная", forcedRu.tabHome)
    }

    @Test
    fun editableUserPlaylistGuardRejectsLikedSongsAndAlbums() {
        assertTrue(dev.sonata.android.ui.components.isEditableUserPlaylist("VLPL1234567890", signedIn = true))
        assertTrue(dev.sonata.android.ui.components.isEditableUserPlaylist("PL1234567890", signedIn = true))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist("VLPL1234567890", signedIn = false))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist("VLLM", signedIn = true))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist("LM", signedIn = true))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist("MPREb_album123", signedIn = true))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist("RDCLAK5uy_mix", signedIn = true))
        assertFalse(dev.sonata.android.ui.components.isEditableUserPlaylist(null, signedIn = true))
    }

    @Test
    fun shuffleAndRepeatPersistenceSanitizationAndLocalizedShelvesWork() {
        val defaults = SonataSettingsState()
        assertFalse(defaults.shuffleEnabled)
        assertEquals(0, defaults.repeatMode)

        assertEquals(0, SonataPreferences.sanitize(defaults.copy(repeatMode = -2)).repeatMode)
        assertEquals(2, SonataPreferences.sanitize(defaults.copy(repeatMode = 9)).repeatMode)

        val persisted = SonataSettingsState(shuffleEnabled = true, repeatMode = 2)
        val backup = SonataPreferences.formatBackupJson(persisted, emptyMap(), 1_700_000_000_000L)
        assertTrue(backup.contains("\"shuffle_enabled\": true"))
        assertTrue(backup.contains("\"repeat_mode\": 2"))

        val tr = dev.sonata.android.ui.i18n.TurkishSonataStrings
        val ru = dev.sonata.android.ui.i18n.RussianSonataStrings
        assertEquals(tr.quickPicks, dev.sonata.android.ui.i18n.localizeShelfTitle("Quick picks", tr))
        assertEquals(ru.newReleases, dev.sonata.android.ui.i18n.localizeShelfTitle("New Releases", ru))
        assertEquals("Custom Editorial Shelf", dev.sonata.android.ui.i18n.localizeShelfTitle("Custom Editorial Shelf", tr))
        assertEquals(tr.moodWorkout, dev.sonata.android.ui.i18n.localizeMoodTitle("Workout", tr))
        assertEquals(ru.moodFocus, dev.sonata.android.ui.i18n.localizeMoodTitle("Focus", ru))
        assertFalse(defaults.suppressVideoAudioQualityWarning)
        assertEquals(dev.sonata.android.data.VideoQualityPreference.AUTO, defaults.videoQuality)

        val withVideoPrefs = defaults.copy(
            suppressVideoAudioQualityWarning = true,
            videoQuality = dev.sonata.android.data.VideoQualityPreference.P720,
        )
        val videoBackup = SonataPreferences.formatBackupJson(withVideoPrefs, emptyMap(), 1_700_000_000_000L)
        assertTrue(videoBackup.contains("\"suppress_video_audio_warning\": true"))
        assertTrue(videoBackup.contains("\"video_quality\": \"P720\""))

        assertFalse(defaults.downloadVideosIfAvailable)
        assertEquals(
            dev.sonata.android.data.VideoDownloadQuality.P720,
            defaults.videoDownloadQuality,
        )
        assertEquals(
            dev.sonata.android.data.VideoQualityPreference.P1080,
            dev.sonata.android.data.VideoDownloadQuality.P1080.toVideoQualityPreference(),
        )
        assertEquals(
            dev.sonata.android.data.VideoQualityPreference.P360,
            dev.sonata.android.data.VideoDownloadQuality.P360.toVideoQualityPreference(),
        )
        val withVideoDownloadPrefs = defaults.copy(
            downloadVideosIfAvailable = true,
            videoDownloadQuality = dev.sonata.android.data.VideoDownloadQuality.P1080,
        )
        val dlBackup = SonataPreferences.formatBackupJson(withVideoDownloadPrefs, emptyMap(), 1_700_000_000_000L)
        assertTrue(dlBackup.contains("\"download_videos_if_available\": true"))
        assertTrue(dlBackup.contains("\"video_download_quality\": \"P1080\""))
    }

    @Test
    fun dismissedHistorySuppressionMatchesByIdAndCanonicalKey() {
        val removedTrack = uniffi.sonata_android.AndroidTrack(
            id = "vid_removed",
            title = "Midnight City (feat. Guest) [Remastered]",
            artist = "M83",
            album = "Hurry Up",
            durationSecs = 243uL,
            thumbnailUrl = null,
            liked = false,
            albumId = null,
            isExplicit = false,
        )
        val sameKeyDifferentId = removedTrack.copy(id = "vid_other_upload", title = "Midnight City")
        val keptTrack = removedTrack.copy(id = "vid_kept", title = "Wait", artist = "M83")

        val key = SonataViewModel.canonicalTrackKey(removedTrack)
        val pairs = setOf(removedTrack.id to key)
        val ids = setOf(removedTrack.id)
        val keys = setOf(key)

        assertTrue(dev.sonata.android.viewmodel.isDismissedHistoryTrack(removedTrack, pairs, ids, keys))
        assertTrue(dev.sonata.android.viewmodel.isDismissedHistoryTrack(sameKeyDifferentId, pairs, ids, keys))
        assertFalse(dev.sonata.android.viewmodel.isDismissedHistoryTrack(keptTrack, pairs, ids, keys))
    }
}
