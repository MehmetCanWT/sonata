package dev.sonata.android.data

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Abstraction for encrypting and decrypting persisted application secrets.
 */
interface SecretCipher {
    fun encrypt(plaintext: String): String
    fun decrypt(stored: String): String?
    fun isEncrypted(stored: String): Boolean = stored.startsWith(AndroidKeyStoreSecretCipher.ENCRYPTED_PREFIX)
}

/**
 * Hardware-backed Android Keystore (`AES/GCM/NoPadding`) secret cipher for tokens and pairing keys.
 *
 * Uses `AndroidKeyStore` when running on an Android device/emulator, and an ephemeral `SecureRandom`
 * 256-bit AES-GCM key under pure JVM unit tests (`Build.FINGERPRINT == null`).
 */
object AndroidKeyStoreSecretCipher : SecretCipher {
    const val ENCRYPTED_PREFIX = "enc:v1:"
    private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    private const val KEY_ALIAS = "sonata_app_secrets_v1"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val IV_LENGTH_BYTES = 12
    private const val GCM_TAG_BITS = 128

    private val keyLock = Any()
    private val secureRandom = SecureRandom()

    @Volatile
    private var jvmFallbackKey: SecretKey? = null

    private fun isAndroidRuntime(): Boolean =
        runCatching { Build.FINGERPRINT != null }.getOrDefault(false)

    private fun getOrCreateKey(): SecretKey {
        if (!isAndroidRuntime()) {
            jvmFallbackKey?.let { return it }
            return synchronized(keyLock) {
                jvmFallbackKey ?: run {
                    val raw = ByteArray(32)
                    secureRandom.nextBytes(raw)
                    SecretKeySpec(raw, "AES").also { jvmFallbackKey = it }
                }
            }
        }
        return synchronized(keyLock) {
            val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
            (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return@synchronized it }
            val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
            val spec = KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
            keyGenerator.init(spec)
            keyGenerator.generateKey()
        }
    }

    override fun encrypt(plaintext: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val iv = cipher.iv
        require(iv.size == IV_LENGTH_BYTES) { "Unexpected AES-GCM IV length: ${iv.size}" }
        val ciphertext = cipher.doFinal(plaintext.toByteArray(StandardCharsets.UTF_8))
        val combined = ByteArray(iv.size + ciphertext.size)
        System.arraycopy(iv, 0, combined, 0, iv.size)
        System.arraycopy(ciphertext, 0, combined, iv.size, ciphertext.size)
        return ENCRYPTED_PREFIX + Base64.getEncoder().encodeToString(combined)
    }

    override fun decrypt(stored: String): String? {
        if (stored.isEmpty()) return null
        if (!isEncrypted(stored)) {
            return stored
        }
        return runCatching {
            val encoded = stored.removePrefix(ENCRYPTED_PREFIX)
            val combined = Base64.getDecoder().decode(encoded)
            if (combined.size <= IV_LENGTH_BYTES) return null
            val iv = combined.copyOfRange(0, IV_LENGTH_BYTES)
            val ciphertext = combined.copyOfRange(IV_LENGTH_BYTES, combined.size)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
            String(cipher.doFinal(ciphertext), StandardCharsets.UTF_8)
        }.getOrNull()
    }
}
