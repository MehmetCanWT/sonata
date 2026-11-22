/*
 * GitHub Release APK downloader for Sonata Android.
 *
 * Downloads the verified release APK into the user's public Downloads folder via
 * MediaStore.Downloads so the user can inspect and install it manually whenever ready,
 * requiring zero REQUEST_INSTALL_PACKAGES permission or FileProvider surface.
 */
package dev.sonata.android

import android.content.ContentValues
import android.content.Context
import android.os.Environment
import android.provider.MediaStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlin.coroutines.coroutineContext

object ApkUpdater {

    suspend fun downloadApk(
        context: Context,
        urlString: String,
        versionLabel: String,
        onProgress: (fraction: Float?, downloadedBytes: Long, totalBytes: Long) -> Unit,
    ): String = withContext(Dispatchers.IO) {
        val updatesDir = File(context.cacheDir, "updates").apply { mkdirs() }
        updatesDir.listFiles()?.forEach { old ->
            if (old.isFile) old.delete()
        }
        val safeVer = versionLabel.replace(Regex("[^A-Za-z0-9._-]"), "_")
        val fileName = "Sonata-v$safeVer-arm64.apk"
        val tempFile = File(updatesDir, "$fileName.part")
        tempFile.delete()

        var currentUrl = urlString
        var redirects = 0
        var connection: HttpURLConnection? = null
        try {
            while (true) {
                coroutineContext.ensureActive()
                val conn = (URL(currentUrl).openConnection() as HttpURLConnection).apply {
                    instanceFollowRedirects = false
                    connectTimeout = 15_000
                    readTimeout = 30_000
                    setRequestProperty("User-Agent", "Sonata-Android-Updater")
                    setRequestProperty("Accept", "application/octet-stream, */*")
                }
                val code = conn.responseCode
                if (code in 300..399) {
                    val location = conn.getHeaderField("Location")
                    conn.disconnect()
                    require(!location.isNullOrBlank() && redirects < 8) {
                        "Invalid redirect while downloading APK ($code)"
                    }
                    currentUrl = URL(URL(currentUrl), location).toString()
                    redirects++
                    continue
                }
                require(code in 200..299) {
                    "Download failed with HTTP $code"
                }
                connection = conn
                break
            }

            val conn = requireNotNull(connection)
            val totalBytes = conn.contentLengthLong.takeIf { it > 0L } ?: -1L
            var downloaded = 0L
            var lastEmitMs = 0L
            onProgress(if (totalBytes > 0L) 0f else null, 0L, totalBytes)

            conn.inputStream.use { input ->
                FileOutputStream(tempFile).use { output ->
                    val buffer = ByteArray(32 * 1024)
                    while (true) {
                        coroutineContext.ensureActive()
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        downloaded += read
                        val now = System.currentTimeMillis()
                        if (now - lastEmitMs >= 100L) {
                            lastEmitMs = now
                            val frac = if (totalBytes > 0L) {
                                (downloaded.toFloat() / totalBytes.toFloat()).coerceIn(0f, 1f)
                            } else {
                                null
                            }
                            onProgress(frac, downloaded, totalBytes)
                        }
                    }
                    output.fd.sync()
                }
            }
            onProgress(1f, downloaded, if (totalBytes > 0L) totalBytes else downloaded)
            require(tempFile.length() > 100_000L) {
                "Downloaded APK looks incomplete (${tempFile.length()} bytes)"
            }

            // Export cleanly to public Downloads so the user owns manual installation.
            val resolver = context.contentResolver
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, fileName)
                put(MediaStore.Downloads.MIME_TYPE, "application/vnd.android.package-archive")
                put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                ?: error("Could not create entry in Downloads")
            try {
                resolver.openOutputStream(uri)?.use { out ->
                    FileInputStream(tempFile).use { input ->
                        input.copyTo(out, bufferSize = 32 * 1024)
                    }
                } ?: error("Could not write APK to Downloads")
                values.clear()
                values.put(MediaStore.Downloads.IS_PENDING, 0)
                resolver.update(uri, values, null, null)
            } catch (t: Throwable) {
                resolver.delete(uri, null, null)
                throw t
            }
            "Downloads/$fileName"
        } finally {
            tempFile.delete()
            connection?.disconnect()
        }
    }

    fun formatBytes(bytes: Long): String {
        if (bytes <= 0L) return "0 MB"
        val mb = bytes / (1024.0 * 1024.0)
        return String.format(java.util.Locale.US, "%.1f MB", mb)
    }
}
