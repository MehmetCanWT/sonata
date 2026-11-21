package dev.sonata.android

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlin.coroutines.coroutineContext

/**
 * Downloads GitHub Release APKs into private cache storage and hands them off
 * to Android's native package installer sheet via FileProvider.
 */
object ApkUpdater {

    sealed interface InstallOutcome {
        data object LaunchedInstaller : InstallOutcome
        data object NeedsUnknownSourcesPermission : InstallOutcome
    }

    suspend fun downloadApk(
        context: Context,
        urlString: String,
        versionLabel: String,
        onProgress: (fraction: Float?, downloadedBytes: Long, totalBytes: Long) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        val updatesDir = File(context.cacheDir, "updates").apply { mkdirs() }
        // Clean previous APKs so cache never accumulates old installers.
        updatesDir.listFiles()?.forEach { old ->
            if (old.isFile && old.name.endsWith(".apk")) old.delete()
        }
        val safeVer = versionLabel.replace(Regex("[^A-Za-z0-9._-]"), "_")
        val tempFile = File(updatesDir, "sonata-v$safeVer.apk.part")
        val finalFile = File(updatesDir, "sonata-v$safeVer.apk")
        tempFile.delete()
        finalFile.delete()

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
            if (!tempFile.renameTo(finalFile)) {
                tempFile.copyTo(finalFile, overwrite = true)
                tempFile.delete()
            }
            finalFile
        } catch (t: Throwable) {
            tempFile.delete()
            throw t
        } finally {
            connection?.disconnect()
        }
    }

    fun launchInstaller(context: Context, apkFile: File): InstallOutcome {
        if (!context.packageManager.canRequestPackageInstalls()) {
            val settingsIntent = Intent(
                Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                Uri.parse("package:${context.packageName}"),
            ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(settingsIntent)
            return InstallOutcome.NeedsUnknownSourcesPermission
        }
        val apkUri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            apkFile,
        )
        val installIntent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(apkUri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(installIntent)
        return InstallOutcome.LaunchedInstaller
    }

    fun formatBytes(bytes: Long): String {
        if (bytes <= 0L) return "0 MB"
        val mb = bytes / (1024.0 * 1024.0)
        return String.format(java.util.Locale.US, "%.1f MB", mb)
    }
}
