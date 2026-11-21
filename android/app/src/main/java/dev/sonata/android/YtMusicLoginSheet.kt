package dev.sonata.android

import android.annotation.SuppressLint
import android.view.View
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

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
    var capturedOnce by remember { mutableStateOf(false) }

    fun captureOnceIfReady(cookies: String) {
        if (capturedOnce || !hasAuthCookie(cookies)) return
        capturedOnce = true
        CookieManager.getInstance().flush()
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
                Icon(Icons.Rounded.Close, contentDescription = "Close", tint = Color(0xFFF3F6FC))
            }
            Column(modifier = Modifier.weight(1f).padding(horizontal = 6.dp)) {
                Text(
                    text = "Sign in to YouTube Music",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color(0xFFF3F6FC)
                )
                Text(
                    text = if (hasAuthCookies) {
                        "Session ready — tap Confirm to sync your account"
                    } else if (onMusicOrigin) {
                        "Pick your channel if prompted, then tap Confirm"
                    } else {
                        "Sign in with your Google / YouTube Music account"
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (hasAuthCookies) Color(0xFF56D364) else Color(0xFF8B94A7)
                )
            }
            Button(
                enabled = hasAuthCookies || onMusicOrigin,
                onClick = {
                    val manager = CookieManager.getInstance()
                    manager.flush()
                    val cookies = manager.getCookie(YTM_ORIGIN).orEmpty()
                    captureOnceIfReady(cookies)
                },
                shape = SquirclePillShape(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Color(0xFF79C0FF),
                    contentColor = Color(0xFF06101B)
                )
            ) {
                Icon(Icons.Rounded.Check, contentDescription = null)
                Text("Confirm", fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 4.dp))
            }
        }

        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                val cm = CookieManager.getInstance()
                cm.setAcceptCookie(true)
                if (freshSignIn) {
                    cm.removeAllCookies(null)
                    cm.flush()
                }

                WebView(context).apply {
                    setLayerType(View.LAYER_TYPE_HARDWARE, null)
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    cm.setAcceptThirdPartyCookies(this, true)

                    fun inspectOriginAndCookies(view: WebView?, url: String?) {
                        val isYtm = url?.startsWith(YTM_ORIGIN) == true
                        onMusicOrigin = isYtm
                        val raw = cm.getCookie(YTM_ORIGIN).orEmpty()
                        val authPresent = hasAuthCookie(raw)
                        hasAuthCookies = authPresent
                        if (isYtm && authPresent && !capturedOnce && view != null) {
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
                    }
                    loadUrl(if (freshSignIn) YTM_LOGIN_URL else "$YTM_ORIGIN/")
                }
            },
            onRelease = { webView ->
                webView.stopLoading()
                webView.destroy()
            }
        )
    }
}
