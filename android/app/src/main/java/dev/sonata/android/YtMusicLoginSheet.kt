package dev.sonata.android

import android.annotation.SuppressLint
import android.view.View
import android.webkit.CookieManager
import android.webkit.RenderProcessGoneDetail
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
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.i18n.RussianSonataStrings
import dev.sonata.android.ui.i18n.SonataStrings
import dev.sonata.android.ui.i18n.TurkishSonataStrings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicBoolean

private const val YTM_ORIGIN = "https://music.youtube.com"
private const val YTM_LOGIN_URL =
    "https://accounts.google.com/ServiceLogin" +
        "?ltmpl=music&service=youtube&prompt=select_account" +
        "&continue=https%3A%2F%2Fmusic.youtube.com%2F"

private const val LOGGED_IN_PROBE_JS =
    "(function(){try{return !!(window.ytcfg&&window.ytcfg.get&&window.ytcfg.get('LOGGED_IN'));}catch(e){return false;}})()"

private fun hasAuthCookie(raw: String): Boolean =
    raw.contains("SAPISID=") || raw.contains("__Secure-3PAPISID=")

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun YtMusicLoginScreen(
    onCaptured: (String) -> Unit,
    onDismiss: () -> Unit,
    freshSignIn: Boolean = true,
) {
    var onMusicOrigin by remember { mutableStateOf(false) }
    var hasAuthCookies by remember { mutableStateOf(false) }
    var visitedAuthFlow by remember { mutableStateOf(!freshSignIn) }
    var capturedOnce by remember { mutableStateOf(false) }
    var rendererGone by remember { mutableStateOf(false) }
    var webViewUnavailable by remember { mutableStateOf(false) }
    var webViewAttempt by remember { mutableIntStateOf(0) }
    // Screen-scoped guard ensures WebView retries do not wipe an in-progress sign-in session.
    val cookiesCleared = remember { AtomicBoolean(false) }
    val coroutineScope = rememberCoroutineScope()
    val strings = LocalSonataStrings.current

    fun flushCookiesAsync() {
        coroutineScope.launch(Dispatchers.IO) {
            runCatching { CookieManager.getInstance().flush() }
        }
    }

    fun captureOnceIfReady(cookies: String) {
        if (capturedOnce || !hasAuthCookie(cookies)) return
        capturedOnce = true
        flushCookiesAsync()
        onCaptured(cookies)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF0A0B10))
            .statusBarsPadding()
            .navigationBarsPadding()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            IconButton(onClick = onDismiss) {
                Icon(Icons.Rounded.Close, contentDescription = strings.close, tint = Color(0xFFF3F6FC))
            }
            Column(modifier = Modifier.weight(1f).padding(horizontal = 6.dp)) {
                Text(
                    text = strings.signInYtmTitle,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color(0xFFF3F6FC)
                )
                Text(
                    text = if (hasAuthCookies || onMusicOrigin) {
                        strings.signInYtmDesc
                    } else {
                        strings.signInYtmSettingsSubtitle
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (hasAuthCookies) Color(0xFF56D364) else Color(0xFF8B94A7)
                )
            }
            Button(
                enabled = hasAuthCookies || onMusicOrigin,
                onClick = {
                    val cookies = runCatching {
                        CookieManager.getInstance().getCookie(YTM_ORIGIN).orEmpty()
                    }.getOrDefault("")
                    flushCookiesAsync()
                    captureOnceIfReady(cookies)
                },
                shape = SquirclePillShape(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Color(0xFF79C0FF),
                    contentColor = Color(0xFF06101B)
                )
            ) {
                Icon(Icons.Rounded.Check, contentDescription = null)
                Text(strings.signIn, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 4.dp))
            }
        }

        if (rendererGone || webViewUnavailable) {
            WebViewFallbackBox(
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
                            val targetUrl = if (freshSignIn) YTM_LOGIN_URL else "$YTM_ORIGIN/"

                            webView.apply {
                                setLayerType(View.LAYER_TYPE_HARDWARE, null)
                                settings.javaScriptEnabled = true
                                settings.domStorageEnabled = true
                                cm.setAcceptThirdPartyCookies(this, true)

                                fun inspectOriginAndCookies(view: WebView?, url: String?) {
                                    if (url?.contains("accounts.google.com") == true ||
                                        url?.contains("accounts.youtube.com") == true ||
                                        url?.contains("/signin") == true ||
                                        url?.contains("ServiceLogin") == true
                                    ) {
                                        visitedAuthFlow = true
                                    }
                                    val isYtm = url?.startsWith(YTM_ORIGIN) == true
                                    onMusicOrigin = isYtm
                                    val raw = cm.getCookie(YTM_ORIGIN).orEmpty()
                                    val authPresent = hasAuthCookie(raw)
                                    hasAuthCookies = authPresent
                                    if (isYtm && authPresent && visitedAuthFlow && !capturedOnce && view != null) {
                                        view.evaluateJavascript(LOGGED_IN_PROBE_JS) { result ->
                                            if (result == "true" && !capturedOnce) {
                                                val latest = cm.getCookie(YTM_ORIGIN).orEmpty()
                                                captureOnceIfReady(latest)
                                            }
                                        }
                                    }
                                }

                                webViewClient = object : WebViewClient() {
                                    override fun shouldOverrideUrlLoading(
                                        view: WebView?,
                                        request: WebResourceRequest?
                                    ): Boolean = false

                                    override fun onPageCommitVisible(view: WebView?, url: String?) {
                                        inspectOriginAndCookies(view, url)
                                    }

                                    override fun onPageFinished(view: WebView?, url: String?) {
                                        inspectOriginAndCookies(view, url)
                                    }

                                    override fun onRenderProcessGone(
                                        view: WebView?,
                                        detail: RenderProcessGoneDetail?,
                                    ): Boolean {
                                        rendererGone = true
                                        return true
                                    }
                                }

                                if (freshSignIn && cookiesCleared.compareAndSet(false, true)) {
                                    clearCache(true)
                                    clearHistory()
                                    clearFormData()
                                    WebStorage.getInstance().deleteAllData()
                                    cm.removeAllCookies {
                                        flushCookiesAsync()
                                        post { loadUrl(targetUrl) }
                                    }
                                } else {
                                    loadUrl(targetUrl)
                                }
                            }
                        }.getOrElse {
                            runCatching { webView.destroy() }
                            webViewUnavailable = true
                            View(context)
                        }
                    },
                    onRelease = { view ->
                        (view as? WebView)?.run {
                            stopLoading()
                            destroy()
                        }
                    }
                )
            }
        }
    }
}

@Composable
private fun WebViewFallbackBox(
    strings: SonataStrings,
    isRendererGone: Boolean,
    onRetry: () -> Unit,
) {
    val title = when (strings) {
        TurkishSonataStrings -> if (isRendererGone) {
            "Oturum açma sayfası yeniden başlatılmalı"
        } else {
            "WebView kullanılamıyor"
        }
        RussianSonataStrings -> if (isRendererGone) {
            "Страница входа была остановлена системой"
        } else {
            "WebView недоступен"
        }
        else -> if (isRendererGone) {
            "Sign-in page was stopped by the system"
        } else {
            "WebView is unavailable on this device"
        }
    }
    val subtitle = when (strings) {
        TurkishSonataStrings -> if (isRendererGone) {
            "Giriş işlemini güvenli biçimde sürdürmek için yeniden deneyin."
        } else {
            "Sistem WebView sağlayıcısı yüklenemedi. Tekrar deneyin veya Android Sistem WebView'i güncelleyin."
        }
        RussianSonataStrings -> if (isRendererGone) {
            "Нажмите «Повторить», чтобы продолжить вход."
        } else {
            "Не удалось загрузить системный WebView. Повторите попытку или обновите Android System WebView."
        }
        else -> if (isRendererGone) {
            "Tap Retry to reload the sign-in page without losing progress."
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
                    containerColor = Color(0xFF79C0FF),
                    contentColor = Color(0xFF06101B),
                ),
            ) {
                Text(strings.retry, fontWeight = FontWeight.Bold)
            }
        }
    }
}
