package dev.sonata.android

import dev.sonata.android.connect.ConnectCommand
import dev.sonata.android.connect.SonataConnectClient
import dev.sonata.android.data.SonataDownloadManager
import dev.sonata.android.discord.isAllowedDiscordNavigationUrl
import dev.sonata.android.discord.isAllowedDiscordOrigin
import dev.sonata.android.discord.isDiscordAuthenticatedUrl
import dev.sonata.android.playback.SonataDspAudioProcessor
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.URL
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class AndroidSecurityHardeningTest {

    @Test
    fun apkUpdaterUrlAllowlistRestrictsToSonataReleaseAndGitHubAssetCdns() {
        assertTrue(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://github.com/MehmetCanWT/sonata/releases/download/v1.0.9/Sonata-v1.0.9-arm64.apk"),
            ),
        )
        assertTrue(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://objects.githubusercontent.com/github-production-release-asset-2e65be/1234/asset.apk"),
            ),
        )
        assertTrue(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://release-assets.githubusercontent.com/github-production-release-asset/1234/asset.apk"),
            ),
        )
        assertTrue(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://github-releases.githubusercontent.com/1234/asset.apk"),
            ),
        )

        // Disallowed protocols, repositories, CDNs, ports, userinfo, and traversals
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("http://github.com/MehmetCanWT/sonata/releases/download/v1.0.9/Sonata-v1.0.9-arm64.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://github.com/attacker/sonata/releases/download/v1.0.9/Sonata-v1.0.9-arm64.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://raw.githubusercontent.com/attacker/repo/main/evil.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://evil.githubusercontent.com/evil.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://github.com/MehmetCanWT/sonata/releases/download/v1.0.9/../../../attacker/evil.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://user:pass@github.com/MehmetCanWT/sonata/releases/download/v1.0.9/Sonata.apk"),
            ),
        )
        assertFalse(
            ApkUpdater.isAllowedUpdateUrl(
                URL("https://github.com:8443/MehmetCanWT/sonata/releases/download/v1.0.9/Sonata.apk"),
            ),
        )
    }

    @Test
    fun apkUpdaterSha256SumsDerivationParsingAndMatching() {
        val apkUrl = URL("https://github.com/MehmetCanWT/sonata/releases/download/v1.0.9/Sonata-v1.0.9-arm64.apk")
        val derived = ApkUpdater.deriveSha256SumsUrl(apkUrl)
        assertNotNull(derived)
        assertEquals(
            "https://github.com/MehmetCanWT/sonata/releases/download/v1.0.9/SHA256SUMS.txt",
            derived!!.first.toString(),
        )
        assertEquals("Sonata-v1.0.9-arm64.apk", derived.second)

        val digestA = "a".repeat(64)
        val digestB = "b".repeat(64)
        val sumsText = """
            # Release checksums
            $digestA  Sonata-v1.0.9-arm64.apk
            $digestB *./dist/Sonata-v1.0.9-x86_64.AppImage
        """.trimIndent()

        assertEquals(digestA, ApkUpdater.parseSha256Sums(sumsText, "Sonata-v1.0.9-arm64.apk"))
        assertEquals(digestB, ApkUpdater.parseSha256Sums(sumsText, "Sonata-v1.0.9-x86_64.AppImage"))
        assertNull(ApkUpdater.parseSha256Sums(sumsText, "Missing.apk"))

        assertTrue(ApkUpdater.verifySha256Match(digestA, digestA.uppercase()))
        assertFalse(ApkUpdater.verifySha256Match(digestA, digestB))
        assertFalse(ApkUpdater.verifySha256Match("short", digestA))
        assertFalse(ApkUpdater.verifySha256Match("z".repeat(64), "z".repeat(64)))

        assertTrue(ApkUpdater.signerDigestsMatch(setOf(digestA), setOf(digestA)))
        assertFalse(ApkUpdater.signerDigestsMatch(emptySet(), emptySet()))
        assertFalse(ApkUpdater.signerDigestsMatch(setOf(digestA), setOf(digestB)))
    }

    @Test
    fun webViewAllowlistsEnforceHttpsAndTrustedOrigins() {
        // Discord origin & navigation
        assertTrue(isAllowedDiscordOrigin("https://discord.com/channels/@me"))
        assertTrue(isAllowedDiscordOrigin("https://ptb.discord.com/app"))
        assertFalse(isAllowedDiscordOrigin("http://discord.com/channels/@me"))
        assertFalse(isAllowedDiscordOrigin("https://evil-discord.com/channels/@me"))
        assertFalse(isAllowedDiscordOrigin("https://discord.com.evil.org/channels/@me"))
        assertFalse(isDiscordAuthenticatedUrl("http://discord.com/channels/@me"))

        assertTrue(isAllowedDiscordNavigationUrl("https://discord.com/login"))
        assertTrue(isAllowedDiscordNavigationUrl("https://canary.discord.com/app"))
        assertTrue(isAllowedDiscordNavigationUrl("https://discordapp.com/oauth2"))
        assertTrue(isAllowedDiscordNavigationUrl("https://newassets.hcaptcha.com/captcha/v1/"))
        assertFalse(isAllowedDiscordNavigationUrl("file:///data/user/0/dev.sonata.android/shared_prefs/a.xml"))
        assertFalse(isAllowedDiscordNavigationUrl("https://phishing.example/login"))

        // YouTube Music navigation
        assertTrue(isAllowedYtMusicNavigationUrl("https://music.youtube.com/"))
        assertTrue(isAllowedYtMusicNavigationUrl("https://accounts.google.com/ServiceLogin"))
        assertTrue(isAllowedYtMusicNavigationUrl("https://accounts.youtube.com/accounts/SetSID"))
        assertTrue(isAllowedYtMusicNavigationUrl("https://lh3.googleusercontent.com/avatar"))
        assertTrue(isAllowedYtMusicNavigationUrl("https://ssl.gstatic.com/accounts/ui/"))
        assertFalse(isAllowedYtMusicNavigationUrl("http://music.youtube.com/"))
        assertFalse(isAllowedYtMusicNavigationUrl("https://google.com.evil.example/"))
        assertFalse(isAllowedYtMusicNavigationUrl("file:///etc/passwd"))
    }

    @Test
    fun downloadManagerCanonicalContainmentBlocksEscapesAndSensitiveFiles() {
        val tempRoot = Files.createTempDirectory("sonata-dm-test").toFile()
        try {
            val cacheDir = File(tempRoot, "cache").apply { mkdirs() }
            val filesDir = File(tempRoot, "files").apply { mkdirs() }
            val siblingEvilDir = File(tempRoot, "files_evil").apply { mkdirs() }
            val allowedRoots = listOf(cacheDir, filesDir)

            val validCacheFile = File(cacheDir, "track.m4a").apply { writeText("audio") }
            val validDownloadFile = File(filesDir, "downloads/track.flac").apply {
                parentFile?.mkdirs()
                writeText("flac")
            }
            val siblingEvilFile = File(siblingEvilDir, "steal.m4a").apply { writeText("evil") }
            val traversalFile = File(cacheDir, "../files_evil/steal.m4a")
            val cookieFile = File(filesDir, "ytm_session.cookie").apply { writeText("secret") }
            val dbFile = File(filesDir, "sonata_cache.db").apply { writeText("db") }

            assertTrue(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(validCacheFile, allowedRoots))
            assertTrue(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(validDownloadFile, allowedRoots))
            assertFalse(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(siblingEvilFile, allowedRoots))
            assertFalse(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(traversalFile, allowedRoots))
            assertFalse(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(cookieFile, allowedRoots))
            assertFalse(SonataDownloadManager.isCanonicalFileWithinAllowedRoots(dbFile, allowedRoots))
        } finally {
            tempRoot.deleteRecursively()
        }
    }

    @Test
    fun sonataConnectClientRethrowsCancellationException() = runBlocking {
        val cancellingHttp = OkHttpClient.Builder()
            .addInterceptor { throw CancellationException("cancelled-in-test") }
            .build()
        val client = SonataConnectClient(localDeviceId = "test-dev", httpClient = cancellingHttp)

        suspend fun assertCancels(block: suspend () -> Any?) {
            var caughtCancellation = false
            try {
                block()
            } catch (_: CancellationException) {
                caughtCancellation = true
            }
            assertTrue("Expected CancellationException to propagate", caughtCancellation)
        }

        assertCancels { client.fetchInfo("http://127.0.0.1:1") }
        assertCancels { client.pair("http://127.0.0.1:1", "Phone", "123456") }
        assertCancels { client.fetchState("http://127.0.0.1:1", "secret") }
        assertCancels { client.sendCommand("http://127.0.0.1:1", "secret", ConnectCommand.Play) }
    }

    @Test
    fun dspAudioProcessorHandlesConcurrentEqualizerUpdatesWithoutRace() {
        val dsp = SonataDspAudioProcessor()
        val configured = dsp.configure(AudioProcessor.AudioFormat(48_000, 2, C.ENCODING_PCM_16BIT))
        assertEquals(48_000, configured.sampleRate)
        dsp.flush()

        val failure = AtomicReference<Throwable?>(null)
        val done = CountDownLatch(1)
        val writer = Thread {
            try {
                for (i in 0 until 500) {
                    val enabled = (i % 2) == 0
                    val boost = ((i % 7) - 3).toFloat() * 2f
                    dsp.updateEqualizer(enabled, listOf(boost, 0f, -boost, 1.5f, -1.5f))
                }
            } catch (t: Throwable) {
                failure.compareAndSet(null, t)
            } finally {
                done.countDown()
            }
        }
        writer.start()

        val pcmBytes = ByteArray(1024)
        val pcmBuf = ByteBuffer.wrap(pcmBytes).order(ByteOrder.nativeOrder())
        for (i in 0 until 512) {
            pcmBuf.putShort(i * 2, ((i % 64) * 200).toShort())
        }

        for (i in 0 until 500) {
            pcmBuf.position(0)
            pcmBuf.limit(pcmBytes.size)
            dsp.queueInput(pcmBuf)
            val out = dsp.output
            out.position(out.limit())
        }

        assertTrue(done.await(5, TimeUnit.SECONDS))
        assertNull("Unexpected concurrent DSP exception: ${failure.get()}", failure.get())
    }
}
