package dev.sonata.android.discord

import android.annotation.SuppressLint
import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import android.net.http.SslError
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.JsResult
import android.webkit.RenderProcessGoneDetail
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import dev.sonata.android.SquirclePillShape
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.net.URI
import java.util.Collections
import java.util.Locale
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

internal const val DISCORD_LOGIN_URL = "https://discord.com/login"
internal const val DISCORD_TOKEN_ALERT_PREFIX = "SONATA_DISCORD_TOKEN:"
internal const val DISCORD_TOKEN_POLL_ATTEMPTS = 60
internal const val DISCORD_TOKEN_POLL_INTERVAL_MS = 500L

internal val DISCORD_COOKIE_ORIGINS = listOf(
    "https://discord.com",
    "https://ptb.discord.com",
    "https://canary.discord.com",
    "https://discordapp.com",
)

internal sealed interface DiscordTokenValidationResult {
    data class Valid(val userId: String, val username: String?) : DiscordTokenValidationResult
    data object Unauthorized : DiscordTokenValidationResult
    data object TransientFailure : DiscordTokenValidationResult
}

internal fun extractCookieNames(cookieHeader: String?): List<String> {
    if (cookieHeader.isNullOrBlank()) return emptyList()
    return cookieHeader.split(';')
        .mapNotNull { part ->
            val name = part.substringBefore('=', "").trim()
            name.takeIf { it.isNotEmpty() }
        }
        .distinct()
}

private fun clearDiscordSessionStorage(cm: CookieManager) {
    DISCORD_COOKIE_ORIGINS.forEach { origin ->
        val host = runCatching { URI(origin).host }.getOrNull().orEmpty()
        val domainAttr = if (host.endsWith("discord.com")) {
            ".discord.com"
        } else if (host.endsWith("discordapp.com")) {
            ".discordapp.com"
        } else {
            host
        }
        extractCookieNames(cm.getCookie(origin)).forEach { name ->
            cm.setCookie(origin, "$name=; Max-Age=0; Path=/")
            if (domainAttr.isNotBlank()) {
                cm.setCookie(origin, "$name=; Max-Age=0; Path=/; Domain=$domainAttr")
            }
        }
        runCatching { WebStorage.getInstance().deleteOrigin(origin) }
    }
}

private val WEBVIEW_MARKER_REGEX = Regex(""";\s*wv\b""")
private val WEBVIEW_VERSION_REGEX = Regex("""\s*Version/\d+(?:\.\d+)*""")
private val MULTI_SPACE_REGEX = Regex("""\s{2,}""")

/**
 * Idempotent hook installed on Discord pages to observe the session token written to
 * `Storage.prototype.setItem('token', ...)` or sent via `Authorization` headers on
 * same-origin XHR/fetch requests after the user completes interactive login.
 */
private const val DISCORD_TOKEN_HOOK_JS =
    """
    (function() {
      try {
        if (window.__sonataDiscordHooked) return;
        window.__sonataDiscordHooked = true;
        window.__sonataDiscordToken = window.__sonataDiscordToken || null;
        var capture = function(raw) {
          if (typeof raw !== 'string') return;
          var cleaned = raw.trim();
          if (cleaned.length >= 2 && cleaned.charAt(0) === '"' && cleaned.charAt(cleaned.length - 1) === '"') {
            cleaned = cleaned.slice(1, -1).trim();
          }
          if (cleaned.length >= 20 && cleaned !== 'null' && cleaned !== 'undefined' &&
              !/^Bot\s/i.test(cleaned) && !/^Bearer\s/i.test(cleaned) && !/\s/.test(cleaned)) {
            window.__sonataDiscordToken = cleaned;
          }
        };
        if (window.Storage && Storage.prototype && Storage.prototype.setItem) {
          var origSetItem = Storage.prototype.setItem;
          Storage.prototype.setItem = function(k, v) {
            try {
              if (k === 'token' && typeof v === 'string') capture(v);
            } catch (e) {}
            return origSetItem.apply(this, arguments);
          };
        }
        if (window.XMLHttpRequest && XMLHttpRequest.prototype && XMLHttpRequest.prototype.setRequestHeader) {
          var origSetHeader = XMLHttpRequest.prototype.setRequestHeader;
          XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
            try {
              if (typeof name === 'string' && name.toLowerCase() === 'authorization' && typeof value === 'string') {
                capture(value);
              }
            } catch (e) {}
            return origSetHeader.apply(this, arguments);
          };
        }
        if (typeof window.fetch === 'function') {
          var origFetch = window.fetch;
          window.fetch = function(input, init) {
            try {
              var headers = init && init.headers;
              if (headers) {
                if (typeof headers.get === 'function') {
                  capture(headers.get('authorization') || headers.get('Authorization'));
                } else if (Array.isArray(headers)) {
                  for (var i = 0; i < headers.length; i++) {
                    if (headers[i] && String(headers[i][0]).toLowerCase() === 'authorization') {
                      capture(String(headers[i][1]));
                    }
                  }
                } else if (typeof headers === 'object') {
                  for (var key in headers) {
                    if (Object.prototype.hasOwnProperty.call(headers, key) && key.toLowerCase() === 'authorization') {
                      capture(String(headers[key]));
                    }
                  }
                }
              }
            } catch (e) {}
            return origFetch.apply(this, arguments);
          };
        }
      } catch (e) {}
    })();
    """

/**
 * Evaluates in-page token capture sources and returns the raw token string directly
 * through `WebView.evaluateJavascript` without depending on `window.alert`.
 */
private const val DISCORD_TOKEN_READ_JS =
    """
    (function() {
      try {
        if (window.__sonataDiscordToken) return window.__sonataDiscordToken;
      } catch (e) {}
      try {
        if (window.localStorage) {
          var direct = window.localStorage.getItem('token');
          if (direct) return direct;
        }
      } catch (e) {}
      try {
        if (document.body) {
          var iframe = document.createElement('iframe');
          iframe.style.display = 'none';
          document.body.appendChild(iframe);
          var raw = iframe.contentWindow && iframe.contentWindow.localStorage
            ? iframe.contentWindow.localStorage.getItem('token')
            : null;
          iframe.remove();
          if (raw) return raw;
        }
      } catch (e) {}
      return null;
    })();
    """

/**
 * Strips Android WebView markers (`; wv` and `Version/x.y`) from [userAgent] so
 * Discord's web sign-in flow allows modern 2FA and passkey prompts.
 */
internal fun sanitizeDiscordUserAgent(userAgent: String): String {
    if (userAgent.isBlank()) return ""
    return userAgent
        .replace(WEBVIEW_MARKER_REGEX, "")
        .replace(WEBVIEW_VERSION_REGEX, "")
        .replace("(; ", "(")
        .replace("; )", ")")
        .replace(MULTI_SPACE_REGEX, " ")
        .trim()
}

/**
 * Returns true only for HTTPS origins on `discord.com` or its subdomains.
 */
internal fun isAllowedDiscordOrigin(url: String?): Boolean {
    if (url.isNullOrBlank()) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val scheme = parsed.scheme?.lowercase(Locale.ROOT) ?: return false
    if (scheme != "https") return false
    val host = parsed.host?.lowercase(Locale.ROOT) ?: return false
    return host == "discord.com" || host.endsWith(".discord.com")
}

/**
 * Returns true for HTTPS Discord API requests where an `Authorization` header may carry
 * the authenticated user session token.
 */
internal fun isAllowedDiscordApiUrl(url: String?): Boolean {
    if (!isAllowedDiscordOrigin(url)) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val path = parsed.path?.lowercase(Locale.ROOT).orEmpty()
    return path.startsWith("/api/")
}

/**
 * Enforces an HTTPS host allowlist for Discord sign-in WebView top-level navigations.
 */
internal fun isAllowedDiscordNavigationUrl(url: String?): Boolean {
    if (url.isNullOrBlank()) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val scheme = parsed.scheme?.lowercase(Locale.ROOT) ?: return false
    if (scheme != "https") return false
    val host = parsed.host?.lowercase(Locale.ROOT) ?: return false
    return host == "discord.com" ||
        host.endsWith(".discord.com") ||
        host == "discordapp.com" ||
        host.endsWith(".discordapp.com") ||
        host == "hcaptcha.com" ||
        host.endsWith(".hcaptcha.com") ||
        host == "challenges.cloudflare.com"
}

/**
 * Returns true if a WebView navigation should be allowed for either main-frame or subframe
 * requests while blocking deep-link schemes (`discord://`, `intent://`) that would leave WebView.
 */
internal fun shouldAllowDiscordWebViewRequest(url: String?, isForMainFrame: Boolean): Boolean {
    if (url.isNullOrBlank()) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val scheme = parsed.scheme?.lowercase(Locale.ROOT) ?: return false
    if (!isForMainFrame) {
        return scheme == "https" || scheme == "about" || scheme == "blob" || scheme == "data"
    }
    return isAllowedDiscordNavigationUrl(url)
}

/**
 * Returns true once the WebView URL is on `https://discord.com` outside `/login` and `/register`.
 */
internal fun isDiscordAuthenticatedUrl(url: String?): Boolean {
    if (!isAllowedDiscordOrigin(url)) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val path = parsed.path?.lowercase(Locale.ROOT).orEmpty()
    if (path.isEmpty() || path == "/") return false
    if (path == "/login" || path.startsWith("/login/")) return false
    if (path == "/register" || path.startsWith("/register/")) return false
    return true
}

/**
 * Parses a Discord token delivered through `WebChromeClient.onJsAlert`, returning
 * null for unrelated alerts or invalid/empty payloads.
 */
internal fun extractTokenFromJsAlert(message: String?): String? {
    if (message.isNullOrBlank() || !message.startsWith(DISCORD_TOKEN_ALERT_PREFIX)) {
        return null
    }
    return DiscordApplication.sanitizeToken(message.removePrefix(DISCORD_TOKEN_ALERT_PREFIX))
}

/**
 * Parses the JSON-encoded string returned by `WebView.evaluateJavascript` (handling
 * double-escaped JSON strings like `"\"token\""` as well as plain `"token"`).
 */
internal fun extractTokenFromJsEvalResult(rawResult: String?): String? {
    if (rawResult.isNullOrBlank()) return null
    val trimmed = rawResult.trim()
    if (trimmed.equals("null", ignoreCase = true) || trimmed.equals("undefined", ignoreCase = true)) {
        return null
    }
    val decoded = runCatching {
        JSONObject("""{"v":$trimmed}""").optString("v", "")
    }.getOrDefault(trimmed)
    return DiscordApplication.sanitizeToken(decoded)
}

/**
 * Validates a candidate Discord user token against `GET /api/v9/users/@me` before persisting it.
 */
internal fun validateDiscordUserToken(
    httpClient: OkHttpClient,
    rawToken: String,
): DiscordTokenValidationResult {
    val token = DiscordApplication.sanitizeToken(rawToken) ?: return DiscordTokenValidationResult.Unauthorized
    val request = Request.Builder()
        .url(DiscordApplication.CURRENT_USER_URL)
        .header("Authorization", token)
        .header("User-Agent", DiscordApplication.GATEWAY_USER_AGENT)
        .header("Accept", "application/json")
        .get()
        .build()

    return try {
        httpClient.newCall(request).execute().use { response ->
            when {
                response.code == 401 || response.code == 403 -> DiscordTokenValidationResult.Unauthorized
                !response.isSuccessful -> DiscordTokenValidationResult.TransientFailure
                else -> {
                    val body = response.body?.string().orEmpty()
                    val obj = runCatching { JSONObject(body) }.getOrNull()
                        ?: return DiscordTokenValidationResult.TransientFailure
                    val userId = obj.optString("id", "").trim()
                    if (userId.isEmpty()) {
                        DiscordTokenValidationResult.Unauthorized
                    } else {
                        val username = obj.optString("global_name", "").takeIf { it.isNotBlank() }
                            ?: obj.optString("username", "").takeIf { it.isNotBlank() }
                        DiscordTokenValidationResult.Valid(userId = userId, username = username)
                    }
                }
            }
        }
    } catch (_: Throwable) {
        DiscordTokenValidationResult.TransientFailure
    }
}

private fun defaultValidationHttpClient(): OkHttpClient = OkHttpClient.Builder()
    .connectTimeout(12, TimeUnit.SECONDS)
    .readTimeout(12, TimeUnit.SECONDS)
    .writeTimeout(12, TimeUnit.SECONDS)
    .build()

/**
 * Explicit user consent dialog explaining how Discord user-token Rich Presence works
 * before opening the in-app Discord login WebView.
 */
@Composable
fun DiscordConsentWarningDialog(
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    containerColor: Color = Color(0xFF161820),
    titleColor: Color = Color(0xFFF3F6FC),
    bodyColor: Color = Color(0xFFB4BCCB),
    accentColor: Color = Color(0xFF5865F2),
) {
    val strings = LocalSonataStrings.current
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = containerColor,
        titleContentColor = titleColor,
        textContentColor = bodyColor,
        title = {
            Text(
                text = strings.discordConsentTitle,
                fontWeight = FontWeight.Bold,
            )
        },
        text = {
            Text(
                text = strings.discordConsentBody,
                style = MaterialTheme.typography.bodyMedium,
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    text = strings.discordConsentContinue,
                    color = accentColor,
                    fontWeight = FontWeight.SemiBold,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(
                    text = strings.cancel,
                    color = titleColor,
                )
            }
        },
    )
}

/**
 * In-app WebView sign-in sheet for Discord Rich Presence.
 *
 * Strips `; wv` and `Version/...` from the User-Agent so modern 2FA/passkeys work cleanly,
 * tracks React SPA route transitions via `doUpdateVisitedHistory`, observes authenticated
 * Discord API `Authorization` headers and localStorage/iframe tokens without logging secrets,
 * and validates the captured session against `/api/v9/users/@me` before finishing sign-in.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun DiscordLoginScreen(
    onTokenCaptured: (String) -> Unit,
    onDismiss: () -> Unit,
    requireConsent: Boolean = false,
) {
    var consentAccepted by remember(requireConsent) { mutableStateOf(!requireConsent) }
    var onAuthenticatedRoute by remember { mutableStateOf(false) }
    var verifyingSession by remember { mutableStateOf(false) }
    var verificationFailed by remember { mutableStateOf(false) }
    var rendererGone by remember { mutableStateOf(false) }
    var webViewUnavailable by remember { mutableStateOf(false) }
    var loadErrorDetail by remember { mutableStateOf<String?>(null) }
    var loadProgress by remember { mutableIntStateOf(0) }
    var webViewAttempt by remember { mutableIntStateOf(0) }
    var activeWebView by remember { mutableStateOf<WebView?>(null) }

    val capturedOnce = remember { AtomicBoolean(false) }
    val storageCleared = remember { AtomicBoolean(false) }
    val rejectedTokens = remember { Collections.synchronizedSet(HashSet<String>()) }
    val inFlightTokens = remember { Collections.synchronizedSet(HashSet<String>()) }
    val validationHttpClient = remember { defaultValidationHttpClient() }
    val coroutineScope = rememberCoroutineScope()
    var tokenPollJob by remember { mutableStateOf<Job?>(null) }
    val strings = LocalSonataStrings.current

    DisposableEffect(Unit) {
        onDispose {
            tokenPollJob?.cancel()
            tokenPollJob = null
        }
    }

    if (!consentAccepted) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color(0xFF0A0B10).copy(alpha = 0.72f)),
        ) {
            DiscordConsentWarningDialog(
                onConfirm = { consentAccepted = true },
                onDismiss = onDismiss,
            )
        }
        return
    }

    fun submitCandidateToken(rawCandidate: String?) {
        val token = DiscordApplication.sanitizeToken(rawCandidate) ?: return
        if (capturedOnce.get() || rejectedTokens.contains(token) || !inFlightTokens.add(token)) {
            return
        }
        coroutineScope.launch(Dispatchers.Main) {
            verifyingSession = true
            verificationFailed = false
            val result = withContext(Dispatchers.IO) {
                validateDiscordUserToken(validationHttpClient, token)
            }
            inFlightTokens.remove(token)
            when (result) {
                is DiscordTokenValidationResult.Valid -> {
                    if (capturedOnce.compareAndSet(false, true)) {
                        tokenPollJob?.cancel()
                        tokenPollJob = null
                        withContext(Dispatchers.IO) {
                            runCatching { CookieManager.getInstance().flush() }
                        }
                        onTokenCaptured(token)
                    }
                }
                DiscordTokenValidationResult.Unauthorized -> {
                    rejectedTokens.add(token)
                    if (!capturedOnce.get()) {
                        verifyingSession = false
                    }
                }
                DiscordTokenValidationResult.TransientFailure -> {
                    if (!capturedOnce.get()) {
                        verifyingSession = false
                        verificationFailed = true
                    }
                }
            }
        }
    }

    fun probeWebViewForToken(webView: WebView?) {
        if (webView == null || capturedOnce.get()) return
        runCatching {
            webView.evaluateJavascript(DISCORD_TOKEN_READ_JS) { rawResult ->
                val candidate = extractTokenFromJsEvalResult(rawResult)
                if (candidate != null) {
                    submitCandidateToken(candidate)
                }
            }
        }
    }

    fun startTokenProbeLoop(webView: WebView?) {
        if (webView == null || capturedOnce.get()) return
        tokenPollJob?.cancel()
        tokenPollJob = coroutineScope.launch(Dispatchers.Main) {
            repeat(DISCORD_TOKEN_POLL_ATTEMPTS) {
                if (!isActive || capturedOnce.get()) return@launch
                probeWebViewForToken(webView)
                delay(DISCORD_TOKEN_POLL_INTERVAL_MS)
            }
            if (isActive && !capturedOnce.get() && onAuthenticatedRoute && !verifyingSession) {
                verificationFailed = true
            }
        }
    }

    val subtitleText = when {
        verifyingSession -> strings.discordSignInVerifying
        verificationFailed -> strings.discordSignInVerifyFailed
        onAuthenticatedRoute -> strings.discordSignInCompleting
        else -> strings.discordSignInSubtitle
    }
    val subtitleColor = when {
        verificationFailed -> Color(0xFFFF6B6B)
        verifyingSession || onAuthenticatedRoute -> Color(0xFF56D364)
        else -> Color(0xFF8B94A7)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF0A0B10))
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            IconButton(onClick = onDismiss) {
                Icon(
                    imageVector = Icons.Rounded.Close,
                    contentDescription = strings.close,
                    tint = Color(0xFFF3F6FC),
                )
            }
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 6.dp),
            ) {
                Text(
                    text = strings.discordSignInTitle,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color(0xFFF3F6FC),
                )
                Text(
                    text = subtitleText,
                    style = MaterialTheme.typography.labelSmall,
                    color = subtitleColor,
                )
            }
            if (verificationFailed && !capturedOnce.get()) {
                TextButton(
                    onClick = {
                        verificationFailed = false
                        rejectedTokens.clear()
                        startTokenProbeLoop(activeWebView)
                    },
                ) {
                    Text(
                        text = strings.retry,
                        color = Color(0xFF5865F2),
                        fontWeight = FontWeight.SemiBold,
                    )
                }
            }
        }

        if (loadErrorDetail == null && !rendererGone && !webViewUnavailable && loadProgress < 100) {
            if (loadProgress in 1..99) {
                LinearProgressIndicator(
                    progress = { loadProgress / 100f },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(2.dp),
                    color = Color(0xFF5865F2),
                    trackColor = Color(0xFF161820),
                )
            } else {
                LinearProgressIndicator(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(2.dp),
                    color = Color(0xFF5865F2),
                    trackColor = Color(0xFF161820),
                )
            }
        }

        if (rendererGone || webViewUnavailable || loadErrorDetail != null) {
            DiscordWebViewFallbackBox(
                strings = strings,
                isRendererGone = rendererGone,
                loadErrorDetail = loadErrorDetail,
                onRetry = {
                    rendererGone = false
                    webViewUnavailable = false
                    loadErrorDetail = null
                    loadProgress = 0
                    verificationFailed = false
                    webViewAttempt += 1
                },
            )
        } else {
            key(webViewAttempt) {
                AndroidView(
                    modifier = Modifier.fillMaxSize(),
                    factory = { context ->
                        val webView = runCatching { WebView(context) }.getOrElse {
                            webViewUnavailable = true
                            return@AndroidView View(context)
                        }
                        activeWebView = webView

                        runCatching {
                            val cm = CookieManager.getInstance()
                            cm.setAcceptCookie(true)

                            webView.apply {
                                layoutParams = ViewGroup.LayoutParams(
                                    ViewGroup.LayoutParams.MATCH_PARENT,
                                    ViewGroup.LayoutParams.MATCH_PARENT,
                                )
                                setBackgroundColor(AndroidColor.parseColor("#0A0B10"))
                                settings.javaScriptEnabled = true
                                settings.domStorageEnabled = true
                                settings.useWideViewPort = true
                                settings.loadWithOverviewMode = true
                                settings.javaScriptCanOpenWindowsAutomatically = false
                                settings.allowFileAccess = false
                                settings.allowContentAccess = false
                                settings.userAgentString = sanitizeDiscordUserAgent(settings.userAgentString)
                                cm.setAcceptThirdPartyCookies(this, true)

                                webChromeClient = object : WebChromeClient() {
                                    override fun onProgressChanged(view: WebView?, newProgress: Int) {
                                        loadProgress = newProgress.coerceIn(0, 100)
                                    }

                                    override fun onJsAlert(
                                        view: WebView?,
                                        url: String?,
                                        message: String?,
                                        result: JsResult?,
                                    ): Boolean {
                                        if (message?.startsWith(DISCORD_TOKEN_ALERT_PREFIX) == true) {
                                            result?.confirm()
                                            if (!isAllowedDiscordOrigin(url)) {
                                                return true
                                            }
                                            val token = extractTokenFromJsAlert(message)
                                            if (token != null) {
                                                submitCandidateToken(token)
                                            }
                                            return true
                                        }
                                        return super.onJsAlert(view, url, message, result)
                                    }
                                }

                                webViewClient = object : WebViewClient() {
                                    override fun onPageStarted(
                                        view: WebView?,
                                        url: String?,
                                        favicon: Bitmap?,
                                    ) {
                                        super.onPageStarted(view, url, favicon)
                                        loadErrorDetail = null
                                    }

                                    override fun shouldOverrideUrlLoading(
                                        view: WebView?,
                                        request: WebResourceRequest?,
                                    ): Boolean = !shouldAllowDiscordWebViewRequest(
                                        url = request?.url?.toString(),
                                        isForMainFrame = request?.isForMainFrame ?: true,
                                    )

                                    override fun shouldInterceptRequest(
                                        view: WebView?,
                                        request: WebResourceRequest?,
                                    ): WebResourceResponse? {
                                        val reqUrl = request?.url?.toString()
                                        if (!capturedOnce.get() && isAllowedDiscordApiUrl(reqUrl)) {
                                            val authHeader = request?.requestHeaders?.entries
                                                ?.firstOrNull { it.key.equals("authorization", ignoreCase = true) }
                                                ?.value
                                            if (!authHeader.isNullOrBlank()) {
                                                submitCandidateToken(authHeader)
                                            }
                                        }
                                        return null
                                    }

                                    override fun doUpdateVisitedHistory(
                                        view: WebView?,
                                        url: String?,
                                        isReload: Boolean,
                                    ) {
                                        super.doUpdateVisitedHistory(view, url, isReload)
                                        if (isAllowedDiscordOrigin(url)) {
                                            runCatching { view?.evaluateJavascript(DISCORD_TOKEN_HOOK_JS, null) }
                                        }
                                        if (isDiscordAuthenticatedUrl(url)) {
                                            onAuthenticatedRoute = true
                                            startTokenProbeLoop(view)
                                        }
                                    }

                                    override fun onPageCommitVisible(view: WebView?, url: String?) {
                                        if (isAllowedDiscordOrigin(url)) {
                                            runCatching { view?.evaluateJavascript(DISCORD_TOKEN_HOOK_JS, null) }
                                        }
                                        if (isDiscordAuthenticatedUrl(url)) {
                                            onAuthenticatedRoute = true
                                            startTokenProbeLoop(view)
                                        }
                                    }

                                    override fun onPageFinished(view: WebView?, url: String?) {
                                        loadProgress = 100
                                        if (isAllowedDiscordOrigin(url)) {
                                            runCatching { view?.evaluateJavascript(DISCORD_TOKEN_HOOK_JS, null) }
                                        }
                                        if (isDiscordAuthenticatedUrl(url)) {
                                            onAuthenticatedRoute = true
                                            startTokenProbeLoop(view)
                                        }
                                    }

                                    override fun onReceivedError(
                                        view: WebView?,
                                        request: WebResourceRequest?,
                                        error: WebResourceError?,
                                    ) {
                                        super.onReceivedError(view, request, error)
                                        if (request?.isForMainFrame == true && loadErrorDetail == null) {
                                            val desc = error?.description?.toString()?.trim().orEmpty()
                                            val code = error?.errorCode ?: -1
                                            Log.w("SonataDiscord", "WebView main-frame error code=$code desc=$desc")
                                            loadErrorDetail = desc.ifEmpty { "ERR_WEBVIEW_$code" }
                                        }
                                    }

                                    override fun onReceivedSslError(
                                        view: WebView?,
                                        handler: SslErrorHandler?,
                                        error: SslError?,
                                    ) {
                                        handler?.cancel()
                                        val primary = error?.primaryError ?: -1
                                        Log.w("SonataDiscord", "WebView SSL error code=$primary")
                                        if (loadErrorDetail == null) {
                                            loadErrorDetail = "SSL_ERROR_$primary"
                                        }
                                    }

                                    override fun onRenderProcessGone(
                                        view: WebView?,
                                        detail: RenderProcessGoneDetail?,
                                    ): Boolean {
                                        rendererGone = true
                                        return true
                                    }
                                }

                                if (storageCleared.compareAndSet(false, true)) {
                                    clearCache(true)
                                    clearHistory()
                                    clearFormData()
                                    clearDiscordSessionStorage(cm)
                                    coroutineScope.launch(Dispatchers.IO) {
                                        runCatching { cm.flush() }
                                        withContext(Dispatchers.Main) {
                                            if (activeWebView === this@apply) {
                                                loadUrl(DISCORD_LOGIN_URL)
                                            }
                                        }
                                    }
                                } else {
                                    loadUrl(DISCORD_LOGIN_URL)
                                }
                            }
                        }.getOrElse {
                            runCatching { webView.destroy() }
                            activeWebView = null
                            webViewUnavailable = true
                            View(context)
                        }
                    },
                    onRelease = { view ->
                        tokenPollJob?.cancel()
                        tokenPollJob = null
                        if (activeWebView === view) {
                            activeWebView = null
                        }
                        (view as? WebView)?.run {
                            stopLoading()
                            destroy()
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun DiscordWebViewFallbackBox(
    strings: SonataStrings,
    isRendererGone: Boolean,
    loadErrorDetail: String? = null,
    onRetry: () -> Unit,
) {
    val title = when {
        isRendererGone -> strings.discordWebViewRendererGoneTitle
        loadErrorDetail != null -> strings.discordWebViewLoadErrorTitle
        else -> strings.discordWebViewUnavailableTitle
    }
    val subtitle = when {
        isRendererGone -> strings.discordWebViewRendererGoneBody
        loadErrorDetail != null -> strings.discordWebViewLoadErrorBody
        else -> strings.discordWebViewUnavailableBody
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = Color(0xFFF3F6FC),
                textAlign = TextAlign.Center,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = Color(0xFF8B94A7),
                textAlign = TextAlign.Center,
            )
            if (!loadErrorDetail.isNullOrBlank()) {
                Text(
                    text = loadErrorDetail,
                    style = MaterialTheme.typography.labelSmall,
                    color = Color(0xFFFF8E8E),
                    textAlign = TextAlign.Center,
                )
            }
            Button(
                onClick = onRetry,
                shape = SquirclePillShape(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Color(0xFF5865F2),
                    contentColor = Color.White,
                ),
            ) {
                Text(strings.retry, fontWeight = FontWeight.Bold)
            }
        }
    }
}
