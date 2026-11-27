package dev.sonata.android

import android.content.Context
import android.content.ContextWrapper
import android.content.SharedPreferences
import android.os.Process
import dev.sonata.android.connect.SharedPrefsConnectSecretStore
import dev.sonata.android.connect.SonataConnectCrypto
import dev.sonata.android.data.AndroidKeyStoreSecretCipher
import dev.sonata.android.data.SonataPreferences
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SecurityHardeningTest {

    private class MemoryPrefs : SharedPreferences {
        val backing = LinkedHashMap<String, Any?>()

        override fun getAll(): MutableMap<String, *> = HashMap(backing)
        override fun getString(key: String?, defValue: String?): String? =
            (backing[key] as? String) ?: defValue

        @Suppress("UNCHECKED_CAST")
        override fun getStringSet(key: String?, defValues: MutableSet<String>?): MutableSet<String>? =
            (backing[key] as? Set<String>)?.toMutableSet() ?: defValues

        override fun getInt(key: String?, defValue: Int): Int = (backing[key] as? Int) ?: defValue
        override fun getLong(key: String?, defValue: Long): Long = (backing[key] as? Long) ?: defValue
        override fun getFloat(key: String?, defValue: Float): Float = (backing[key] as? Float) ?: defValue
        override fun getBoolean(key: String?, defValue: Boolean): Boolean =
            (backing[key] as? Boolean) ?: defValue

        override fun contains(key: String?): Boolean = backing.containsKey(key)

        override fun edit(): SharedPreferences.Editor = object : SharedPreferences.Editor {
            private val pending = LinkedHashMap<String, Any?>()
            private val removals = HashSet<String>()
            private var clearAll = false

            override fun putString(key: String?, value: String?): SharedPreferences.Editor = apply {
                if (key != null) {
                    removals.remove(key)
                    pending[key] = value
                }
            }

            override fun putStringSet(key: String?, values: MutableSet<String>?): SharedPreferences.Editor = apply {
                if (key != null) {
                    removals.remove(key)
                    pending[key] = values?.toSet()
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
                if (key != null) {
                    pending.remove(key)
                    removals.add(key)
                }
            }

            override fun clear(): SharedPreferences.Editor = apply {
                pending.clear()
                removals.clear()
                clearAll = true
            }

            override fun commit(): Boolean {
                apply()
                return true
            }

            override fun apply() {
                if (clearAll) backing.clear()
                removals.forEach { backing.remove(it) }
                pending.forEach { (k, v) -> if (v == null) backing.remove(k) else backing[k] = v }
            }
        }

        override fun registerOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?,
        ) {}

        override fun unregisterOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?,
        ) {}
    }

    private class FakePrefsContext : ContextWrapper(null) {
        val stores = LinkedHashMap<String, MemoryPrefs>()
        override fun getApplicationContext(): Context = this
        override fun getSharedPreferences(name: String?, mode: Int): SharedPreferences =
            stores.getOrPut(name ?: "default") { MemoryPrefs() }
    }

    @Test
    fun secretCipherEncryptsWithAesGcmAndRejectsCorruptCiphertext() {
        val encryptedA = AndroidKeyStoreSecretCipher.encrypt("mock-token-value-1")
        val encryptedB = AndroidKeyStoreSecretCipher.encrypt("mock-token-value-1")
        assertTrue(encryptedA.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX))
        assertNotEquals(encryptedA, encryptedB)
        assertFalse(encryptedA.contains("mock-token-value-1"))
        assertEquals("mock-token-value-1", AndroidKeyStoreSecretCipher.decrypt(encryptedA))
        assertEquals("mock-token-value-1", AndroidKeyStoreSecretCipher.decrypt(encryptedB))
        assertNull(AndroidKeyStoreSecretCipher.decrypt("${AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX}not-valid"))
    }

    @Test
    fun connectSecretStoreAndDiscordPrefsEncryptAndMigrateLegacyPlaintext() {
        val ctx = FakePrefsContext()
        val store = SharedPrefsConnectSecretStore(ctx)
        store.saveSecret("desktop-1", "test-connect-token-1")

        val rawStored = ctx.stores["sonata_connect_secrets"]!!.backing["secret_desktop-1"] as String
        assertTrue(rawStored.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX))
        assertFalse(rawStored.contains("test-connect-token-1"))
        assertEquals("test-connect-token-1", store.getSecret("desktop-1"))

        // Legacy plaintext migration on read
        ctx.stores["sonata_connect_secrets"]!!.backing["secret_legacy"] = "legacy-plain-secret"
        assertEquals("legacy-plain-secret", store.getSecret("legacy"))
        val migratedRaw = ctx.stores["sonata_connect_secrets"]!!.backing["secret_legacy"] as String
        assertTrue(migratedRaw.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX))

        val prefs = SonataPreferences(ctx)
        prefs.setDiscordToken("test-discord-token-1")
        val rawDiscord = ctx.stores["sonata_discord_auth"]!!.backing["discord_session_credential"] as String
        assertTrue(rawDiscord.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX))
        assertFalse(rawDiscord.contains("test-discord-token-1"))
        assertEquals("test-discord-token-1", prefs.getDiscordToken())

        // Legacy plaintext Discord token migration on read
        ctx.stores["sonata_discord_auth"]!!.backing["discord_session_credential"] = "legacy-discord-token-1"
        val migratedPrefs = SonataPreferences(ctx)
        assertEquals("legacy-discord-token-1", migratedPrefs.getDiscordToken())
        val migratedDiscordRaw = ctx.stores["sonata_discord_auth"]!!.backing["discord_session_credential"] as String
        assertTrue(migratedDiscordRaw.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX))
    }

    @Test
    fun canonicalConnectRequestSigningBindsMethodPathTimestampAndNonce() {
        val nonce = SonataConnectCrypto.generateNonce()
        assertEquals(32, nonce.length)
        val sig = SonataConnectCrypto.signRequest(
            secret = "test-secret-1",
            method = "POST",
            path = "/connect/command",
            timestamp = "1700000000",
            nonce = nonce,
            body = """{"command":"pause"}""",
        )
        assertTrue(
            SonataConnectCrypto.verifyRequestSignature(
                secret = "test-secret-1",
                method = "post",
                path = "/connect/command",
                timestamp = "1700000000",
                nonce = nonce,
                body = """{"command":"pause"}""",
                signatureHex = sig,
            ),
        )
        assertFalse(
            SonataConnectCrypto.verifyRequestSignature(
                secret = "test-secret-1",
                method = "GET",
                path = "/connect/command",
                timestamp = "1700000000",
                nonce = nonce,
                body = """{"command":"pause"}""",
                signatureHex = sig,
            ),
        )
        assertFalse(
            SonataConnectCrypto.verifyRequestSignature(
                secret = "test-secret-1",
                method = "POST",
                path = "/connect/state",
                timestamp = "1700000000",
                nonce = nonce,
                body = """{"command":"pause"}""",
                signatureHex = sig,
            ),
        )
    }

    @Test
    fun playbackServiceRejectsUntrustedExternalControllers() {
        assertTrue(
            isTrustedMediaController(
                controllerPackage = "dev.sonata.android",
                controllerUid = 10123,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = false,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = false,
            ),
        )
        assertTrue(
            isTrustedMediaController(
                controllerPackage = "com.android.systemui",
                controllerUid = Process.SYSTEM_UID,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = false,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = false,
            ),
        )
        assertTrue(
            isTrustedMediaController(
                controllerPackage = "com.google.android.projection.gearhead",
                controllerUid = 10200,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = true,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = false,
            ),
        )
        assertFalse(
            isTrustedMediaController(
                controllerPackage = "com.malicious.app",
                controllerUid = 10999,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = false,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = false,
            ),
        )
        assertFalse(
            isTrustedMediaController(
                controllerPackage = "dev.sonata.android",
                controllerUid = 10999,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = false,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = false,
            ),
        )
        assertFalse(
            isTrustedMediaController(
                controllerPackage = " ",
                controllerUid = 10123,
                ownPackage = "dev.sonata.android",
                ownUid = 10123,
                isNotificationController = false,
                isAutomotiveController = false,
                isAutoCompanionController = false,
                isSystemTrustedForMediaControl = true,
            ),
        )
    }
}
