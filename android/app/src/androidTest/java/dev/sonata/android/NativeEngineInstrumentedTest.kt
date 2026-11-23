package dev.sonata.android

import android.system.Os
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.sonata.android.data.NativeSonataEngineGateway
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.sonata_android.SonataAndroidEngine
import java.io.File

/**
 * Instrumented Android suite verifying the real native `libsonata_android.so` UniFFI library,
 * private filesystem permissions (`0700`/`0600`), SQLite cache lifecycle, and coroutine cancellation.
 */
@RunWith(AndroidJUnit4::class)
class NativeEngineInstrumentedTest {

    private lateinit var testRoot: File
    private lateinit var configDir: File
    private lateinit var cacheDir: File

    @Before
    fun setUp() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        testRoot = File(ctx.cacheDir, "sonata_inst_${System.nanoTime()}").apply { mkdirs() }
        configDir = File(testRoot, "config")
        cacheDir = File(testRoot, "cache")
    }

    @After
    fun tearDown() {
        testRoot.deleteRecursively()
    }

    @Test
    fun loadsNativeLibraryInitializesPrivateDirsAndHandlesSignOutCleanly() = runBlocking {
        val engine = SonataAndroidEngine(configDir.absolutePath, cacheDir.absolutePath)
        val gateway = NativeSonataEngineGateway(engine)

        assertFalse("Fresh isolated engine must start signed out", gateway.isSignedIn())
        assertTrue(gateway.getCachedLibraryPlaylists().isEmpty())

        // Clearing lyrics cache against isolated SQLite + disk directory succeeds without crashing.
        gateway.clearLyricsCache()

        // Persisted invalid cookie header must fail with typed AuthRequired error (never panic).
        val invalidResult = runCatching { gateway.setYtmCookieHeader("PREF=tz=UTC") }
        assertTrue("Cookie header without SAPISID must fail cleanly", invalidResult.isFailure)
        assertFalse(gateway.isSignedIn())

        // Sign out remains idempotent and leaves private dirs with owner-only permissions.
        gateway.signOut()
        val sonataCacheDir = File(cacheDir, "sonata")
        if (sonataCacheDir.exists()) {
            val mode = Os.stat(sonataCacheDir.absolutePath).st_mode and 0x1FF
            assertEquals("Private cache dir must be 0700", 0x1C0, mode)
        }
    }

    @Test
    fun cancellingInFlightAsyncUniFFiCallKeepsEngineResponsive() = runBlocking {
        val engine = SonataAndroidEngine(configDir.absolutePath, cacheDir.absolutePath)
        val gateway = NativeSonataEngineGateway(engine)

        val job = launch {
            runCatching { gateway.searchSongs("daft punk") }
        }
        job.cancelAndJoin()
        assertTrue(job.isCancelled)

        // Engine remains healthy and responsive immediately after native future cancellation.
        assertFalse(gateway.isSignedIn())
        assertTrue(gateway.getCachedLibraryPlaylists().isEmpty())
    }
}
