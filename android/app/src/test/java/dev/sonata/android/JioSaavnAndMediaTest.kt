package dev.sonata.android

import dev.sonata.android.data.JioSaavnSource
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.artworkMemoryCacheKey
import dev.sonata.android.ui.components.MoodTheme
import dev.sonata.android.ui.components.resolveMoodTheme
import dev.sonata.android.ui.croppedAt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec

class JioSaavnAndMediaTest {

    private fun encryptJioSaavnUrlForTest(plainUrl: String): String {
        val keySpec = SecretKeySpec("38346591".toByteArray(Charsets.UTF_8), "DES")
        val cipher = Cipher.getInstance("DES/ECB/PKCS5Padding")
        cipher.init(Cipher.ENCRYPT_MODE, keySpec)
        return Base64.getEncoder().encodeToString(cipher.doFinal(plainUrl.toByteArray(Charsets.UTF_8)))
    }

    @Test
    fun decryptsJioSaavnUrlToHttpsAndUpgradesBitrateTo320() {
        val sampleHttp = "http://aac.saavncdn.com/123/abc_96.mp4"
        val encrypted = encryptJioSaavnUrlForTest(sampleHttp)
        val decrypted = JioSaavnSource.decryptUrl(encrypted)
        assertEquals("https://aac.saavncdn.com/123/abc_96.mp4", decrypted)

        val (upgradedUrl, upgradedKbps) = JioSaavnSource.selectBestStreamUrl(decrypted!!, supports320 = true)
        assertEquals("https://aac.saavncdn.com/123/abc_320.mp4", upgradedUrl)
        assertEquals(320, upgradedKbps)

        val (keptUrl, keptKbps) = JioSaavnSource.selectBestStreamUrl(decrypted, supports320 = false)
        assertEquals(decrypted, keptUrl)
        assertEquals(96, keptKbps)
    }

    @Test
    fun normalizesTitlesHtmlAndArtistMatchesAccurately() {
        assertEquals(
            "Daft Punk & The Weeknd's \"Starboy\"",
            JioSaavnSource.decodeHtml("Daft Punk &amp; The Weeknd&#039;s &quot;Starboy&quot;"),
        )
        assertEquals(
            "Midnight Dreams",
            JioSaavnSource.cleanTitleForMatch("Midnight Dreams (feat. Guest) [Official Video]"),
        )
        assertEquals("istanbul night", JioSaavnSource.normalizeCore("İstanbul Night!"))
        assertEquals("The Weeknd", JioSaavnSource.primaryArtistOf("The Weeknd, Daft Punk"))
        assertTrue(JioSaavnSource.artistsOverlap("The Weeknd, Daft Punk", "The Weeknd"))
        assertFalse(JioSaavnSource.artistsOverlap("Daft Punk", "Drake"))
    }

    @Test
    fun artworkUrlsScaleDeterministicallyAcrossHosts() {
        assertEquals(
            "https://c.saavncdn.com/000/cover_500x500.jpg",
            JioSaavnSource.upgradeArtwork("http://c.saavncdn.com/000/cover_150x150.jpg"),
        )
        assertNull(JioSaavnSource.upgradeArtwork("   "))

        assertEquals(
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/1200x1200bb.jpg",
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/100x100bb.jpg".artworkAt(1200),
        )
        assertEquals(
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/720x720cc.jpg",
            "https://is1-ssl.mzstatic.com/image/thumb/Music/v4/100x100bb.jpg".croppedAt(480, 720),
        )
        assertEquals(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg",
            "https://i.ytimg.com/vi_webp/dQw4w9WgXcQ/sddefault.webp".artworkAt(720),
        )
        assertEquals(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg".artworkAt(480),
        )
        assertEquals(
            "https://lh3.googleusercontent.com/abc=s720",
            "https://lh3.googleusercontent.com/abc=s120".artworkAt(720),
        )
        assertEquals(
            "https://lh3.googleusercontent.com/abc=w480-h640-p-l90-rj",
            "https://lh3.googleusercontent.com/abc=w120-h120-l90-rj".croppedAt(480, 640),
        )
        assertEquals(
            "https://example.com/art.jpg_480x480",
            artworkMemoryCacheKey("https://example.com/art.jpg", 480),
        )
        assertNull(artworkMemoryCacheKey(" ", 480))
        assertNull((null as String?).artworkAt(480))
    }

    @Test
    fun moodAnimationsAreDistinctAcrossNamedMoodsEspeciallyBlueVsHeartbreak() {
        val feelingBlue = resolveMoodTheme("Feeling Blue", 0)
        val heartbreak = resolveMoodTheme("Heartbreak", 1)
        assertEquals(MoodTheme.MELANCHOLY, feelingBlue)
        assertEquals(MoodTheme.HEARTBREAK, heartbreak)
        assertNotEquals(feelingBlue, heartbreak)

        assertEquals(MoodTheme.ENERGY, resolveMoodTheme("Energy", 0))
        assertEquals(MoodTheme.ENERGY, resolveMoodTheme("Workout", 1))
        assertEquals(MoodTheme.CHILL, resolveMoodTheme("Relax", 2))
        assertEquals(MoodTheme.FOCUS, resolveMoodTheme("Focus", 3))
        assertEquals(MoodTheme.ROMANCE, resolveMoodTheme("Romance", 4))
        assertEquals(MoodTheme.SLEEP, resolveMoodTheme("Sleep", 5))
        assertEquals(MoodTheme.PARTY, resolveMoodTheme("Party", 6))
        assertEquals(MoodTheme.COMMUTE, resolveMoodTheme("Commute", 7))
        assertEquals(MoodTheme.FEEL_GOOD, resolveMoodTheme("Feel Good", 8))
        assertEquals(MoodTheme.FOLK_ACOUSTIC, resolveMoodTheme("Folk & Acoustic", 9))
        assertEquals(MoodTheme.URBAN_PRISM, resolveMoodTheme("Hip-Hop & R&B", 10))
        assertEquals(MoodTheme.BOTANICAL_INDIE, resolveMoodTheme("Indie & Alt", 11))
        assertEquals(MoodTheme.HARMONIC_JAZZ, resolveMoodTheme("Jazz Classics", 12))
        assertEquals(MoodTheme.SYNTH_WAVE, resolveMoodTheme("Electronic Gaming", 13))

        // Unknown mood titles deterministically scatter by title + index.
        val firstFallback = resolveMoodTheme("Celestial Wander", 0)
        val secondFallback = resolveMoodTheme("Celestial Wander", 1)
        assertTrue(firstFallback in MoodTheme.entries)
        assertNotEquals(firstFallback, secondFallback)
    }
}
