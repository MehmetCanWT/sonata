/*
 * GitHub Release APK downloader and in-app installer handoff for Sonata Android.
 *
 * Downloads the verified arm64 release APK into app-private cache/updates/, verifies
 * completeness, and automatically launches Android's PackageInstaller via FileProvider
 * (requesting unknown-sources permission when needed).
 */
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
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import kotlin.coroutines.coroutineContext

enum class ApkInstallLaunchResult {
    LAUNCHED,
    PERMISSION_REQUIRED,
    FILE_MISSING,
}

object ApkUpdater {

    suspend fun downloadApk(
        context: Context,
        urlString: String,
        versionLabel: String,
        onProgress: (fraction: Float?, downloadedBytes: Long, totalBytes: Long) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        val updatesDir = File(context.cacheDir, "updates").apply { mkdirs() }
        updatesDir.listFiles()?.forEach { old ->
            if (old.isFile) old.delete()
        }
        val safeVer = versionLabel.replace(Regex("[^A-Za-z0-9._-]"), "_")
        val fileName = "Sonata-v$safeVer-arm64.apk"
        val tempFile = File(updatesDir, "$fileName.part")
        val finalFile = File(updatesDir, fileName)
        tempFile.delete()
        finalFile.delete()

        var currentUrl = urlString
        var redirects = 0
        var connection: HttpURLConnection? = null
        try {
            while (true) {
                coroutineContext.ensureActive()
                val parsedUrl = URL(currentUrl)
                require(isAllowedUpdateUrl(parsedUrl)) {
                    "Refusing untrusted APK update URL (${parsedUrl.protocol}://${parsedUrl.host})"
                }
                val conn = (parsedUrl.openConnection() as HttpURLConnection).apply {
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
                    val nextUrl = URL(parsedUrl, location)
                    require(isAllowedUpdateUrl(nextUrl)) {
                        "Refusing redirect to untrusted APK host (${nextUrl.protocol}://${nextUrl.host})"
                    }
                    currentUrl = nextUrl.toString()
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
            if (totalBytes > 0L) {
                require(downloaded == totalBytes) {
                    "Downloaded APK size mismatch ($downloaded != $totalBytes bytes)"
                }
            }
            require(tempFile.length() > 100_000L) {
                "Downloaded APK looks incomplete (${tempFile.length()} bytes)"
            }
            val header = ByteArray(4)
            val headerRead = FileInputStream(tempFile).use { it.read(header) }
            require(
                headerRead == 4 &&
                    header[0] == 0x50.toByte() &&
                    header[1] == 0x4B.toByte() &&
                    header[2] == 0x03.toByte() &&
                    header[3] == 0x04.toByte()
            ) {
                "Downloaded file is not a valid APK archive"
            }
            if (!tempFile.renameTo(finalFile)) {
                tempFile.copyTo(finalFile, overwrite = true)
                tempFile.delete()
            }
            finalFile
        } catch (t: Throwable) {
            tempFile.delete()
            finalFile.delete()
            throw t
        } finally {
            connection?.disconnect()
        }
    }

    /**
     * Launches Android's system PackageInstaller for [apkFile], or opens the app's
     * "Install unknown apps" permission screen if not yet allowed.
     */
    fun launchInstall(context: Context, apkFile: File): ApkInstallLaunchResult {
        if (!apkFile.isFile || apkFile.length() <= 100_000L) {
            return ApkInstallLaunchResult.FILE_MISSING
        }
        if (!context.packageManager.canRequestPackageInstalls()) {
            val permIntent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                data = Uri.parse("package:${context.packageName}")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(permIntent)
            return ApkInstallLaunchResult.PERMISSION_REQUIRED
        }
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            apkFile,
        )
        val installIntent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(installIntent)
        return ApkInstallLaunchResult.LAUNCHED
    }

    fun formatBytes(bytes: Long): String {
        if (bytes <= 0L) return "0 MB"
        val mb = bytes / (1024.0 * 1024.0)
        return String.format(java.util.Locale.US, "%.1f MB", mb)
    }

    private fun isAllowedUpdateUrl(url: URL): Boolean {
        if (!url.protocol.equals("https", ignoreCase = true)) return false
        val host = url.host?.lowercase(Locale.ROOT).orEmpty()
        return host == "github.com" ||
            (host.endsWith(".githubusercontent.com") && host.length > ".githubusercontent.com".length)
    }
}
