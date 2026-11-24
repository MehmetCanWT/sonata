package dev.sonata.android.discord

import android.annotation.SuppressLint
import android.view.View
import android.webkit.CookieManager
import android.webkit.JsResult
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
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
import dev.sonata.android.ui.i18n.RussianSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.i18n.TurkishSonataStrings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.net.URI
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean

internal const val DISCORD_LOGIN_URL = "https://discord.com/login"
internal const val DISCORD_TOKEN_ALERT_PREFIX = "SONATA_DISCORD_TOKEN:"
internal const val DISCORD_TOKEN_POLL_ATTEMPTS = 12
internal const val DISCORD_TOKEN_POLL_INTERVAL_MS = 500L

private val WEBVIEW_MARKER_REGEX = Regex(""";\s*wv\b""")
private val WEBVIEW_VERSION_REGEX = Regex("""\s*Version/\d+(?:\.\d+)*""")
private val MULTI_SPACE_REGEX = Regex("""\s{2,}""")

private const val IFRAME_TOKEN_PROBE_JS =
    """
    (function() {
      try {
        var iframe = document.createElement('iframe');
        iframe.style.display = 'none';
        document.body.appendChild(iframe);
        var raw = iframe.contentWindow && iframe.contentWindow.localStorage
          ? iframe.contentWindow.localStorage.getItem('token')
          : null;
        iframe.remove();
        if (raw) {
          alert('$DISCORD_TOKEN_ALERT_PREFIX' + raw);
        }
      } catch (e) {}
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
 * Returns true once the WebView URL is on `discord.com` outside `/login` and `/register`.
 */
internal fun isDiscordAuthenticatedUrl(url: String?): Boolean {
    if (url.isNullOrBlank()) return false
    val parsed = runCatching { URI(url) }.getOrNull() ?: return false
    val scheme = parsed.scheme?.lowercase(Locale.ROOT) ?: return false
    if (scheme != "https" && scheme != "http") return false
    val host = parsed.host?.lowercase(Locale.ROOT) ?: return false
    val isDiscordHost = host == "discord.com" || host.endsWith(".discord.com")
    if (!isDiscordHost) return false
    val path = parsed.path?.lowercase(Locale.ROOT).orEmpty()
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
    var candidate = message.removePrefix(DISCORD_TOKEN_ALERT_PREFIX).trim()
    if (candidate.length >= 2 && candidate.startsWith("\"") && candidate.endsWith("\"")) {
        candidate = candidate.substring(1, candidate.length - 1).trim()
    }
    if (candidate.isBlank() ||
        candidate.equals("null", ignoreCase = true) ||
        candidate.equals("undefined", ignoreCase = true) ||
        candidate.length < 20 ||
        candidate.any { it.isWhitespace() }
    ) {
        return null
    }
    return candidate
}

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
 * Strips `; wv` and `Version/...` from the User-Agent so 2FA/passkeys work cleanly,
 * waits until navigation leaves `/login` and `/register`, and reads iframe
 * `localStorage.token` via `WebChromeClient.onJsAlert` without ever logging the token.
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
    var rendererGone by remember { mutableStateOf(false) }
    var webViewUnavailable by remember { mutableStateOf(false) }
    var webViewAttempt by remember { mutableIntStateOf(0) }

    val capturedOnce = remember { AtomicBoolean(false) }
    val storageCleared = remember { AtomicBoolean(false) }
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

    fun startTokenProbeLoop(webView: WebView?) {
        if (webView == null || capturedOnce.get()) return
        tokenPollJob?.cancel()
        tokenPollJob = coroutineScope.launch(Dispatchers.Main) {
            repeat(DISCORD_TOKEN_POLL_ATTEMPTS) {
                if (!isActive || capturedOnce.get()) return@launch
                runCatching {
                    webView.evaluateJavascript(IFRAME_TOKEN_PROBE_JS, null)
                }
                delay(DISCORD_TOKEN_POLL_INTERVAL_MS)
            }
        }
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
                    text = if (onAuthenticatedRoute) {
                        strings.discordSignInCompleting
                    } else {
                        strings.discordSignInSubtitle
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (onAuthenticatedRoute) Color(0xFF56D364) else Color(0xFF8B94A7),
                )
            }
        }

        if (rendererGone || webViewUnavailable) {
            DiscordWebViewFallbackBox(
                strings = strings,
                isRendererGone = rendererGone,
                onRetry = {
                    rendererGone = false
                    webViewUnavailable = false
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

                        runCatching {
                            val cm = CookieManager.getInstance()
                            cm.setAcceptCookie(true)

                            webView.apply {
                                setLayerType(View.LAYER_TYPE_HARDWARE, null)
                                settings.javaScriptEnabled = true
                                settings.domStorageEnabled = true
                                settings.userAgentString = sanitizeDiscordUserAgent(settings.userAgentString)
                                cm.setAcceptThirdPartyCookies(this, true)

                                webChromeClient = object : WebChromeClient() {
                                    override fun onJsAlert(
                                        view: WebView?,
                                        url: String?,
                                        message: String?,
                                        result: JsResult?,
                                    ): Boolean {
                                        if (message?.startsWith(DISCORD_TOKEN_ALERT_PREFIX) == true) {
                                            result?.confirm()
                                            val token = extractTokenFromJsAlert(message)
                                            if (token != null && capturedOnce.compareAndSet(false, true)) {
                                                tokenPollJob?.cancel()
                                                tokenPollJob = null
                                                coroutineScope.launch(Dispatchers.IO) {
                                                    runCatching { cm.flush() }
                                                }
                                                onTokenCaptured(token)
                                            }
                                            return true
                                        }
                                        return super.onJsAlert(view, url, message, result)
                                    }
                                }

                                webViewClient = object : WebViewClient() {
                                    override fun shouldOverrideUrlLoading(
                                        view: WebView?,
                                        request: WebResourceRequest?,
                                    ): Boolean = false

                                    override fun onPageCommitVisible(view: WebView?, url: String?) {
                                        if (isDiscordAuthenticatedUrl(url)) {
                                            onAuthenticatedRoute = true
                                            startTokenProbeLoop(view)
                                        }
                                    }

                                    override fun onPageFinished(view: WebView?, url: String?) {
                                        if (isDiscordAuthenticatedUrl(url)) {
                                            onAuthenticatedRoute = true
                                            startTokenProbeLoop(view)
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
                                    WebStorage.getInstance().deleteAllData()
                                    cm.removeAllCookies {
                                        coroutineScope.launch(Dispatchers.IO) {
                                            runCatching { cm.flush() }
                                        }
                                        post { loadUrl(DISCORD_LOGIN_URL) }
                                    }
                                } else {
                                    loadUrl(DISCORD_LOGIN_URL)
                                }
                            }
                        }.getOrElse {
                            runCatching { webView.destroy() }
                            webViewUnavailable = true
                            View(context)
                        }
                    },
                    onRelease = { view ->
                        tokenPollJob?.cancel()
                        tokenPollJob = null
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
    onRetry: () -> Unit,
) {
    val title = when (strings) {
        TurkishSonataStrings -> if (isRendererGone) {
            "Discord oturum açma sayfası yeniden başlatılmalı"
        } else {
            "WebView kullanılamıyor"
        }
        RussianSonataStrings -> if (isRendererGone) {
            "Страница входа Discord была остановлена системой"
        } else {
            "WebView недоступен"
        }
        else -> if (isRendererGone) {
            "Discord sign-in page was stopped by the system"
        } else {
            "WebView is unavailable on this device"
        }
    }
    val subtitle = when (strings) {
        TurkishSonataStrings -> if (isRendererGone) {
            "Discord oturum açma işlemini sürdürmek için yeniden deneyin."
        } else {
            "Sistem WebView sağlayıcısı yüklenemedi. Tekrar deneyin veya Android System WebView'i güncelleyin."
        }
        RussianSonataStrings -> if (isRendererGone) {
            "Нажмите «Повторить», чтобы продолжить вход в Discord."
        } else {
            "Не удалось загрузить системный WebView. Повторите попытку или обновите Android System WebView."
        }
        else -> if (isRendererGone) {
            "Tap Retry to reload the Discord sign-in page."
        } else {
            "Could not initialize the system WebView provider. Retry or check Android System WebView."
        }
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
