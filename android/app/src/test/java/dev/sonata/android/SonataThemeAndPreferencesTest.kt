package dev.sonata.android

import androidx.compose.ui.graphics.Color
import dev.sonata.android.data.AudioQualityTier
import dev.sonata.android.data.DesignTheme
import dev.sonata.android.data.DownloadQualityTier
import dev.sonata.android.data.PlayerBackdropMode
import dev.sonata.android.data.SonataPreferences
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.ui.theme.MoonflowerAmoledTokens
import dev.sonata.android.ui.theme.MoonflowerDarkTokens
import dev.sonata.android.ui.theme.MoonflowerDesignMetrics
import dev.sonata.android.ui.theme.MoonflowerLightTokens
import dev.sonata.android.ui.theme.VioletAmoledTokens
import dev.sonata.android.ui.theme.VioletDarkTokens
import dev.sonata.android.ui.theme.VioletDesignMetrics
import dev.sonata.android.ui.theme.VioletLightTokens
import dev.sonata.android.ui.theme.lunariaColorScheme
import dev.sonata.android.ui.theme.lunariaTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SonataThemeAndPreferencesTest {

    @Test
    fun defaultSettingsMatchVioletSystemTheme() {
        val defaults = SonataSettingsState()
        assertEquals(DesignTheme.VIOLET, defaults.designTheme)
        assertEquals(ThemeMode.SYSTEM, defaults.themeMode)
        assertEquals(DesignTheme.VIOLET, defaults.resolvedDesignTheme())
        assertFalse(defaults.isDarkActive(systemDark = false))
        assertTrue(defaults.isDarkActive(systemDark = true))
        assertTrue(defaults.liquidGlassEnabled)
        assertEquals(AudioQualityTier.HIGH, defaults.audioQuality)
        assertEquals(DownloadQualityTier.HIGH, defaults.downloadQuality)
        assertEquals(PlayerBackdropMode.ARTWORK_MESH, defaults.playerBackdropMode)
    }

    @Test
    fun explicitThemeModesOverrideSystemAppearanceAndDesignFamily() {
        val violetLight = SonataSettingsState(designTheme = DesignTheme.MOONFLOWER, themeMode = ThemeMode.VIOLET_LIGHT)
        assertEquals(DesignTheme.VIOLET, violetLight.resolvedDesignTheme())
        assertFalse(violetLight.isDarkActive(systemDark = true))

        val violetDark = SonataSettingsState(designTheme = DesignTheme.MOONFLOWER, themeMode = ThemeMode.VIOLET_DARK)
        assertEquals(DesignTheme.VIOLET, violetDark.resolvedDesignTheme())
        assertTrue(violetDark.isDarkActive(systemDark = false))

        val moonflowerLight = SonataSettingsState(designTheme = DesignTheme.VIOLET, themeMode = ThemeMode.MOONFLOWER_LIGHT)
        assertEquals(DesignTheme.MOONFLOWER, moonflowerLight.resolvedDesignTheme())
        assertFalse(moonflowerLight.isDarkActive(systemDark = true))

        val moonflowerDark = SonataSettingsState(designTheme = DesignTheme.VIOLET, themeMode = ThemeMode.MOONFLOWER_DARK)
        assertEquals(DesignTheme.MOONFLOWER, moonflowerDark.resolvedDesignTheme())
        assertTrue(moonflowerDark.isDarkActive(systemDark = false))

        val amoledMoonflower = SonataSettingsState(designTheme = DesignTheme.MOONFLOWER, themeMode = ThemeMode.AMOLED)
        assertEquals(DesignTheme.MOONFLOWER, amoledMoonflower.resolvedDesignTheme())
        assertTrue(amoledMoonflower.isDarkActive(systemDark = false))
    }

    @Test
    fun legacyThemeModeStringsMigrateCleanly() {
        assertEquals(ThemeMode.MOONFLOWER_DARK, SonataPreferences.parseThemeMode("DARK", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.MOONFLOWER_LIGHT, SonataPreferences.parseThemeMode(" light ", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.VIOLET_DARK, SonataPreferences.parseThemeMode("ELECTRIC_LAVENDER", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.VIOLET_LIGHT, SonataPreferences.parseThemeMode("IMPERIAL_VIOLET", ThemeMode.SYSTEM))
        assertEquals(ThemeMode.AMOLED, SonataPreferences.parseThemeMode("AMOLED", ThemeMode.SYSTEM))
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
        assertEquals(-5_000L, clean.lyricsOffsetMs)

        val slow = SonataPreferences.sanitize(
            SonataSettingsState(
                fpsLimit = 45,
                crossfadeSeconds = 0,
                playbackSpeed = Float.POSITIVE_INFINITY,
                lyricsTranslationLang = " ",
                lyricsOffsetMs = 15_000L,
            ),
        )
        assertEquals(60, slow.fpsLimit)
        assertEquals(1, slow.crossfadeSeconds)
        assertEquals(1.0f, slow.playbackSpeed, 0.001f)
        assertEquals("en", slow.lyricsTranslationLang)
        assertEquals(5_000L, slow.lyricsOffsetMs)
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
        assertTrue(encodedOffsets.contains("\"vid_high\":5000"))
        assertFalse(encodedOffsets.contains("vid_zero"))
    }

    @Test
    fun violetAndMoonflowerThemesRemainStructurallyDistinctWithPinkishGlassPills() {
        assertEquals(VioletLightTokens, lunariaTokens(darkTheme = false, amoled = false, designTheme = DesignTheme.VIOLET))
        assertEquals(VioletDarkTokens, lunariaTokens(darkTheme = true, amoled = false, designTheme = DesignTheme.VIOLET))
        assertEquals(VioletAmoledTokens, lunariaTokens(darkTheme = true, amoled = true, designTheme = DesignTheme.VIOLET))

        assertEquals(MoonflowerLightTokens, lunariaTokens(darkTheme = false, amoled = false, designTheme = DesignTheme.MOONFLOWER))
        assertEquals(MoonflowerDarkTokens, lunariaTokens(darkTheme = true, amoled = false, designTheme = DesignTheme.MOONFLOWER))
        assertEquals(MoonflowerAmoledTokens, lunariaTokens(darkTheme = true, amoled = true, designTheme = DesignTheme.MOONFLOWER))

        // Structural layout differences between Violet and Moonflower.
        assertNotEquals(VioletLightTokens.eclipseBase, MoonflowerLightTokens.eclipseBase)
        assertNotEquals(VioletLightTokens.petalLavender, MoonflowerLightTokens.petalLavender)
        assertNotEquals(VioletDesignMetrics.cardCornerRadius, MoonflowerDesignMetrics.cardCornerRadius)
        assertTrue(VioletLightTokens.heroFullBleed)
        assertFalse(MoonflowerLightTokens.heroFullBleed)
        assertFalse(VioletLightTokens.showTrackDividers)
        assertTrue(MoonflowerLightTokens.showTrackDividers)

        // AMOLED variants preserve true black base surfaces.
        assertEquals(Color.Black, VioletAmoledTokens.eclipseBase)
        assertEquals(Color.Black, MoonflowerAmoledTokens.eclipseBase)
        assertEquals(Color.Black, lunariaColorScheme(darkTheme = true, amoled = true, designTheme = DesignTheme.VIOLET).background)
        assertEquals(Color.Black, lunariaColorScheme(darkTheme = true, amoled = true, designTheme = DesignTheme.MOONFLOWER).background)

        // Pinkish translucent Liquid Glass active tab pill on both Violet and Moonflower light/dark.
        val glassPills = listOf(VioletLightTokens, VioletDarkTokens, MoonflowerLightTokens, MoonflowerDarkTokens)
        for (token in glassPills) {
            assertTrue("Pinkish pill indicator should have red channel dominant over green", token.pillIndicatorColor.red > token.pillIndicatorColor.green)
            assertTrue("Pinkish pill indicator should have red channel >= blue", token.pillIndicatorColor.red >= token.pillIndicatorColor.blue)
            assertTrue("Liquid Glass pill should remain translucent", token.pillIndicatorColor.alpha in 0.20f..0.45f)
        }
    }
}
