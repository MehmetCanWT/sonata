/*
 * GitHub Release APK downloader and in-app installer handoff for Sonata Android.
 *
 * Downloads the verified arm64 release APK into app-private cache/updates/, verifies
 * URL origin, SHA-256 digest (when provided or published in SHA256SUMS.txt), and
 * signing certificate parity against the installed package before launching Android's
 * PackageInstaller via FileProvider.
 */
package dev.sonata.android

import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.content.pm.SigningInfo
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Locale
import kotlin.coroutines.coroutineContext

enum class ApkInstallLaunchResult {
    LAUNCHED,
    PERMISSION_REQUIRED,
    FILE_MISSING,
    SIGNATURE_MISMATCH,
}

object ApkUpdater {

    private const val RELEASE_PATH_PREFIX = "/MehmetCanWT/sonata/releases/download/"
    private const val MAX_SHA256SUMS_BYTES = 64 * 1024
    private val ALLOWED_CDN_HOSTS = setOf(
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
        "github-releases.githubusercontent.com",
    )
    private val SHA256_LINE_REGEX = Regex("""^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$""")

    suspend fun downloadApk(
        context: Context,
        urlString: String,
        versionLabel: String,
        expectedSha256: String? = null,
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

        val initialUrl = URL(urlString)
        require(isAllowedUpdateUrl(initialUrl)) {
            "Refusing untrusted APK update URL (${initialUrl.protocol}://${initialUrl.host})"
        }

        val resolvedExpectedSha256 = expectedSha256?.trim()?.takeIf { it.isNotEmpty() }
            ?: fetchExpectedSha256FromRelease(initialUrl)

        var connection: HttpURLConnection? = null
        try {
            connection = openValidatedConnection(urlString, acceptHeader = "application/octet-stream, */*")
            val conn = requireNotNull(connection)
            val totalBytes = conn.contentLengthLong.takeIf { it > 0L } ?: -1L
            var downloaded = 0L
            var lastEmitMs = 0L
            val sha256 = MessageDigest.getInstance("SHA-256")
            onProgress(if (totalBytes > 0L) 0f else null, 0L, totalBytes)

            conn.inputStream.use { input ->
                FileOutputStream(tempFile).use { output ->
                    val buffer = ByteArray(32 * 1024)
                    while (true) {
                        coroutineContext.ensureActive()
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        sha256.update(buffer, 0, read)
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
            if (resolvedExpectedSha256 != null) {
                val actualHex = bytesToHex(sha256.digest())
                require(verifySha256Match(actualHex, resolvedExpectedSha256)) {
                    "Downloaded APK SHA-256 checksum mismatch"
                }
            }
            require(verifyApkSignatureMatchesInstalled(context, tempFile)) {
                "Downloaded APK signing certificate does not match installed app"
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
        if (!verifyApkSignatureMatchesInstalled(context, apkFile)) {
            runCatching { apkFile.delete() }
            return ApkInstallLaunchResult.SIGNATURE_MISMATCH
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
        return String.format(Locale.US, "%.1f MB", mb)
    }

    internal fun isAllowedUpdateUrl(url: URL): Boolean {
        if (!url.protocol.equals("https", ignoreCase = true)) return false
        if (url.userInfo != null) return false
        if (url.port != -1 && url.port != 443) return false
        val host = url.host?.lowercase(Locale.ROOT).orEmpty()
        val path = url.path.orEmpty()
        if (path.isBlank() || path.contains("..") || path.contains('\\')) return false
        if (host == "github.com") {
            return path.startsWith(RELEASE_PATH_PREFIX, ignoreCase = true) &&
                path.length > RELEASE_PATH_PREFIX.length
        }
        return host in ALLOWED_CDN_HOSTS && path.startsWith("/")
    }

    internal fun deriveSha256SumsUrl(apkUrl: URL): Pair<URL, String>? {
        if (!apkUrl.protocol.equals("https", ignoreCase = true)) return null
        if (!apkUrl.host.equals("github.com", ignoreCase = true)) return null
        val path = apkUrl.path.orEmpty()
        if (!path.startsWith(RELEASE_PATH_PREFIX, ignoreCase = true)) return null
        val rest = path.substring(RELEASE_PATH_PREFIX.length)
        val slashIdx = rest.indexOf('/')
        if (slashIdx <= 0 || slashIdx >= rest.lastIndex) return null
        val tag = rest.substring(0, slashIdx)
        val assetName = rest.substring(slashIdx + 1)
        if (tag.isBlank() || assetName.isBlank() || assetName.contains('/')) return null
        val sumsUrl = URL("https://github.com${RELEASE_PATH_PREFIX}$tag/SHA256SUMS.txt")
        return sumsUrl to assetName
    }

    internal fun parseSha256Sums(content: String, assetFileName: String): String? {
        val target = assetFileName.trim()
        if (target.isEmpty()) return null
        for (rawLine in content.lineSequence()) {
            val line = rawLine.trim()
            if (line.isEmpty() || line.startsWith("#")) continue
            val match = SHA256_LINE_REGEX.matchEntire(line) ?: continue
            val hex = match.groupValues[1].lowercase(Locale.ROOT)
            val entryPath = match.groupValues[2].trim().removePrefix("./")
            val entryBase = entryPath.substringAfterLast('/')
            if (entryPath == target || entryBase == target) {
                return hex
            }
        }
        return null
    }

    internal fun verifySha256Match(actualHex: String, expectedHex: String): Boolean {
        val normActual = actualHex.trim().lowercase(Locale.ROOT)
        val normExpected = expectedHex.trim().lowercase(Locale.ROOT)
        if (normActual.length != 64 || normExpected.length != 64) return false
        if (!normActual.all { it in '0'..'9' || it in 'a'..'f' }) return false
        if (!normExpected.all { it in '0'..'9' || it in 'a'..'f' }) return false
        return MessageDigest.isEqual(
            normActual.toByteArray(StandardCharsets.US_ASCII),
            normExpected.toByteArray(StandardCharsets.US_ASCII),
        )
    }

    internal fun signerDigestsMatch(
        installedDigests: Set<String>,
        archiveDigests: Set<String>,
    ): Boolean = installedDigests.isNotEmpty() && installedDigests == archiveDigests

    internal fun verifyApkSignatureMatchesInstalled(context: Context, apkFile: File): Boolean {
        return runCatching {
            val pm = context.packageManager
            val installedInfo = getInstalledPackageInfoWithSigners(pm, context.packageName) ?: return false
            val archiveInfo = getArchivePackageInfoWithSigners(pm, apkFile.absolutePath) ?: return false
            if (archiveInfo.packageName != context.packageName) return false
            val installedDigests = extractSignerDigests(installedInfo.signingInfo)
            val archiveDigests = extractSignerDigests(archiveInfo.signingInfo)
            signerDigestsMatch(installedDigests, archiveDigests)
        }.getOrDefault(false)
    }

    private fun getInstalledPackageInfoWithSigners(pm: PackageManager, pkg: String): PackageInfo? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.getPackageInfo(
                pkg,
                PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()),
            )
        } else {
            @Suppress("DEPRECATION")
            pm.getPackageInfo(pkg, PackageManager.GET_SIGNING_CERTIFICATES)
        }
    }

    private fun getArchivePackageInfoWithSigners(pm: PackageManager, apkPath: String): PackageInfo? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.getPackageArchiveInfo(
                apkPath,
                PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()),
            )
        } else {
            @Suppress("DEPRECATION")
            pm.getPackageArchiveInfo(apkPath, PackageManager.GET_SIGNING_CERTIFICATES)
        }
    }

    private fun extractSignerDigests(signingInfo: SigningInfo?): Set<String> {
        if (signingInfo == null) return emptySet()
        val signers = signingInfo.apkContentsSigners ?: return emptySet()
        val digest = MessageDigest.getInstance("SHA-256")
        return signers.mapTo(HashSet()) { sig ->
            digest.reset()
            bytesToHex(digest.digest(sig.toByteArray()))
        }
    }

    private suspend fun fetchExpectedSha256FromRelease(apkUrl: URL): String? {
        val (sumsUrl, assetName) = deriveSha256SumsUrl(apkUrl) ?: return null
        var conn: HttpURLConnection? = null
        return try {
            conn = openValidatedConnection(sumsUrl.toString(), acceptHeader = "text/plain, */*", allowMissing = true)
                ?: return null
            val out = ByteArrayOutputStream()
            conn.inputStream.use { input ->
                val buf = ByteArray(4096)
                var total = 0
                while (true) {
                    coroutineContext.ensureActive()
                    val read = input.read(buf)
                    if (read < 0) break
                    total += read
                    if (total > MAX_SHA256SUMS_BYTES) return null
                    out.write(buf, 0, read)
                }
            }
            val text = out.toString(StandardCharsets.UTF_8.name())
            parseSha256Sums(text, assetName)
        } catch (ce: kotlinx.coroutines.CancellationException) {
            throw ce
        } catch (_: Throwable) {
            null
        } finally {
            conn?.disconnect()
        }
    }

    private suspend fun openValidatedConnection(
        initialUrl: String,
        acceptHeader: String,
        allowMissing: Boolean = false,
    ): HttpURLConnection? {
        var currentUrl = initialUrl
        var redirects = 0
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
                setRequestProperty("Accept", acceptHeader)
            }
            val code = conn.responseCode
            if (code in 300..399) {
                val location = conn.getHeaderField("Location")
                conn.disconnect()
                require(!location.isNullOrBlank() && redirects < 8) {
                    "Invalid redirect while downloading update ($code)"
                }
                val nextUrl = URL(parsedUrl, location)
                require(isAllowedUpdateUrl(nextUrl)) {
                    "Refusing redirect to untrusted APK host (${nextUrl.protocol}://${nextUrl.host})"
                }
                currentUrl = nextUrl.toString()
                redirects++
                continue
            }
            if (allowMissing && code == HttpURLConnection.HTTP_NOT_FOUND) {
                conn.disconnect()
                return null
            }
            if (code !in 200..299) {
                conn.disconnect()
                throw IllegalArgumentException("Download failed with HTTP $code")
            }
            return conn
        }
    }

    private fun bytesToHex(bytes: ByteArray): String =
        bytes.joinToString("") { "%02x".format(Locale.US, it) }
}
