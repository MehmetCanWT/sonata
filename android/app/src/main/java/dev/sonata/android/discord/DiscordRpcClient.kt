package dev.sonata.android.discord

import android.os.Build
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale
import java.util.concurrent.TimeUnit
import kotlin.math.abs
import kotlin.random.Random

/**
 * Snapshot of the currently playing track synced to Discord Rich Presence.
 */
data class DiscordPresenceTrack(
    val id: String,
    val title: String,
    val artist: String,
    val album: String = "",
    val durationMs: Long = 0L,
    val artworkUrl: String? = null,
    val isPlaying: Boolean = false,
    val positionMs: Long = 0L,
    val playbackSpeed: Float = 1.0f,
)

/**
 * User-configurable options governing Discord Rich Presence formatting and behavior.
 */
data class DiscordPresenceConfig(
    val enabled: Boolean = false,
    val appId: String = DiscordApplication.DEFAULT_APP_ID,
    val activityType: DiscordActivityType = DiscordActivityType.LISTENING,
    val showTimestamps: Boolean = true,
    val showArtwork: Boolean = true,
    val showButtons: Boolean = true,
)

/**
 * Live connection state of [DiscordRpcClient] exposed to Settings UI without leaking secrets.
 */
sealed interface DiscordRpcStatus {
    data object Disconnected : DiscordRpcStatus
    data object Connecting : DiscordRpcStatus
    data class Connected(val username: String? = null) : DiscordRpcStatus
    data object Reconnecting : DiscordRpcStatus
    data object AuthFailed : DiscordRpcStatus
    data class Error(val message: String) : DiscordRpcStatus
}

/**
 * Discord User Gateway v10 Rich Presence client for Sonata built on OkHttp WebSockets.
 *
 * Connects to `wss://gateway.discord.gg/?v=10&encoding=json`, authenticates via `IDENTIFY`
 * (op 2) with Android client super-properties, maintains `HEARTBEAT` (op 1) with jitter,
 * supports `RESUME` (op 6) and exponential backoff reconnects on drops, proxies external
 * track artwork through `/api/v9/applications/{id}/external-assets` with per-URL caching
 * and failure cooldowns, and sends `SET_PRESENCE` (op 3) updates for active playback.
 */
class DiscordRpcClient(
    private val httpClient: OkHttpClient = defaultOkHttpClient(),
    private val nowProviderMs: () -> Long = { System.currentTimeMillis() },
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val stateLock = Any()

    private val _status = MutableStateFlow<DiscordRpcStatus>(DiscordRpcStatus.Disconnected)
    val status: StateFlow<DiscordRpcStatus> = _status.asStateFlow()

    private var token: String? = null
    private var config: DiscordPresenceConfig = DiscordPresenceConfig()
    private var currentTrack: DiscordPresenceTrack? = null

    private var webSocket: WebSocket? = null
    private var heartbeatJob: Job? = null
    private var reconnectJob: Job? = null
    private var sessionId: String? = null
    private var resumeGatewayUrl: String? = null
    private var lastSequence: Int? = null
    private var connectedUsername: String? = null
    private var heartbeatAcked: Boolean = true
    private var reconnectAttempts: Int = 0
    private var authRejected: Boolean = false
    private var isReady: Boolean = false
    private var disposed: Boolean = false

    private val assetCache = object : LinkedHashMap<AssetCacheKey, String>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<AssetCacheKey, String>?): Boolean =
            size > MAX_CACHED_ASSETS
    }
    private val assetCooldownUntilMs = HashMap<AssetCacheKey, Long>()
    private val inFlightAssetKeys = HashSet<AssetCacheKey>()

    private data class AssetCacheKey(
        val appId: String,
        val artworkUrl: String,
    )

    /**
     * Updates authentication credentials and presence configuration, connecting or
     * disconnecting the Gateway socket as needed.
     */
    fun updateAuthAndConfig(newToken: String?, newConfig: DiscordPresenceConfig) {
        val normalizedToken = newToken?.trim()?.takeIf { it.isNotEmpty() }
        val resolvedConfig = newConfig.copy(appId = DiscordApplication.resolveAppId(newConfig.appId))

        var shouldConnect = false
        var shouldDisconnect = false
        var shouldClearPresenceFirst = false
        var shouldPushPresence = false

        synchronized(stateLock) {
            if (disposed) return
            val tokenChanged = token != normalizedToken
            val configChanged = config != resolvedConfig
            if (!tokenChanged && !configChanged) return

            token = normalizedToken
            config = resolvedConfig

            if (tokenChanged) {
                sessionId = null
                resumeGatewayUrl = null
                lastSequence = null
                connectedUsername = null
                authRejected = false
                reconnectAttempts = 0
            }

            if (normalizedToken == null || !resolvedConfig.enabled) {
                shouldClearPresenceFirst = isReady && webSocket != null
                shouldDisconnect = true
            } else if (tokenChanged || (webSocket == null && reconnectJob == null && !authRejected)) {
                shouldConnect = true
            } else if (isReady && webSocket != null && configChanged) {
                shouldPushPresence = true
            }
        }

        if (shouldDisconnect) {
            if (shouldClearPresenceFirst) {
                sendClearPresence()
            }
            closeConnectionLocked(resetSession = normalizedToken == null, nextStatus = DiscordRpcStatus.Disconnected)
        } else if (shouldConnect) {
            reconnectLocked(forceFresh = true)
        } else if (shouldPushPresence) {
            pushCurrentPresence()
        }
    }

    /**
     * Publishes an updated [track] state (or clears presence when null or paused).
     */
    fun updatePresence(track: DiscordPresenceTrack?) {
        val shouldSend: Boolean
        synchronized(stateLock) {
            if (disposed) return
            currentTrack = track
            shouldSend = isReady && webSocket != null && config.enabled && !token.isNullOrBlank()
        }
        if (shouldSend) {
            pushCurrentPresence()
        }
    }

    /**
     * Forces a fresh Gateway reconnect when enabled and authenticated.
     */
    fun reconnect() {
        synchronized(stateLock) {
            if (disposed) return
            authRejected = false
            reconnectAttempts = 0
            if (token.isNullOrBlank() || !config.enabled) {
                _status.value = DiscordRpcStatus.Disconnected
                return
            }
        }
        reconnectLocked(forceFresh = false)
    }

    /**
     * Gracefully clears Rich Presence, closes any active WebSocket, and cancels internal jobs.
     */
    fun disconnect() {
        val shouldClear: Boolean
        synchronized(stateLock) {
            if (disposed) return
            disposed = true
            shouldClear = isReady && webSocket != null
        }
        if (shouldClear) {
            sendClearPresence()
        }
        closeConnectionLocked(resetSession = true, nextStatus = DiscordRpcStatus.Disconnected)
        scope.cancel()
    }

    private fun reconnectLocked(forceFresh: Boolean) {
        closeConnectionLocked(
            resetSession = forceFresh,
            nextStatus = if (forceFresh) DiscordRpcStatus.Connecting else DiscordRpcStatus.Reconnecting,
        )
        openSocketIfEligible()
    }

    private fun closeConnectionLocked(
        resetSession: Boolean,
        nextStatus: DiscordRpcStatus,
    ) {
        val activeSocket: WebSocket?
        synchronized(stateLock) {
            heartbeatJob?.cancel()
            heartbeatJob = null
            reconnectJob?.cancel()
            reconnectJob = null
            activeSocket = webSocket
            webSocket = null
            isReady = false
            heartbeatAcked = true
            if (resetSession) {
                sessionId = null
                resumeGatewayUrl = null
                lastSequence = null
                connectedUsername = null
            }
            if (!authRejected || nextStatus == DiscordRpcStatus.AuthFailed || nextStatus == DiscordRpcStatus.Disconnected) {
                _status.value = nextStatus
            }
        }
        runCatching { activeSocket?.close(CLOSE_NORMAL, "Closing session") }
    }

    private fun openSocketIfEligible() {
        val gatewayUrl: String
        val activeToken: String
        synchronized(stateLock) {
            if (disposed || authRejected || !config.enabled) return
            activeToken = token ?: return
            if (webSocket != null) return
            gatewayUrl = resumeGatewayUrl
                ?.takeIf { !sessionId.isNullOrBlank() && it.startsWith("wss://") }
                ?.let { base ->
                    if (base.contains("v=")) base else "${base.trimEnd('/')}/?v=10&encoding=json"
                }
                ?: DiscordApplication.GATEWAY_URL
            _status.value = if (sessionId != null) DiscordRpcStatus.Reconnecting else DiscordRpcStatus.Connecting
        }

        val request = Request.Builder()
            .url(gatewayUrl)
            .build()

        val socket = httpClient.newWebSocket(request, GatewayListener(activeToken))
        synchronized(stateLock) {
            if (disposed || token != activeToken || !config.enabled) {
                runCatching { socket.close(CLOSE_NORMAL, "Stale socket") }
                return
            }
            webSocket = socket
        }
    }

    private fun scheduleReconnect(delayMs: Long? = null) {
        synchronized(stateLock) {
            if (disposed || authRejected || !config.enabled || token.isNullOrBlank()) return
            if (reconnectJob?.isActive == true) return
            val attempt = reconnectAttempts.coerceAtMost(MAX_BACKOFF_SHIFT)
            reconnectAttempts += 1
            val waitMs = delayMs ?: (BASE_RECONNECT_DELAY_MS shl attempt).coerceAtMost(MAX_RECONNECT_DELAY_MS)
            _status.value = DiscordRpcStatus.Reconnecting
            reconnectJob = scope.launch {
                delay(waitMs)
                synchronized(stateLock) {
                    reconnectJob = null
                }
                openSocketIfEligible()
            }
        }
    }

    private fun startHeartbeatLoop(socket: WebSocket, intervalMs: Long) {
        val safeInterval = intervalMs.coerceAtLeast(MIN_HEARTBEAT_INTERVAL_MS)
        synchronized(stateLock) {
            heartbeatJob?.cancel()
            heartbeatAcked = true
            heartbeatJob = scope.launch {
                val jitterMs = (safeInterval * (0.1 + 0.8 * Random.nextDouble())).toLong()
                delay(jitterMs)
                while (isActive) {
                    val seq: Int?
                    val canContinue: Boolean
                    synchronized(stateLock) {
                        canContinue = !disposed && webSocket === socket && heartbeatAcked
                        if (canContinue) {
                            heartbeatAcked = false
                        }
                        seq = lastSequence
                    }
                    if (!canContinue) {
                        Log.w(TAG, "Discord gateway heartbeat ACK missed; reconnecting")
                        runCatching { socket.close(CLOSE_ZOMBIE, "Heartbeat timeout") }
                        return@launch
                    }
                    socket.send(buildHeartbeatPayload(seq).toString())
                    delay(safeInterval)
                }
            }
        }
    }

    private fun sendClearPresence() {
        val socket: WebSocket
        val activeConfig: DiscordPresenceConfig
        synchronized(stateLock) {
            socket = webSocket ?: return
            activeConfig = config
        }
        val payload = buildPresencePayload(
            track = null,
            config = activeConfig,
            largeImageAsset = null,
            nowMs = nowProviderMs(),
        )
        runCatching { socket.send(payload.toString()) }
    }

    private fun pushCurrentPresence() {
        val socket: WebSocket
        val activeToken: String
        val activeConfig: DiscordPresenceConfig
        val trackSnapshot: DiscordPresenceTrack?
        val cachedImage: String?
        val shouldFetchAsset: Boolean

        synchronized(stateLock) {
            if (disposed || !isReady) return
            socket = webSocket ?: return
            activeToken = token ?: return
            activeConfig = config
            trackSnapshot = currentTrack
            val artworkUrl = trackSnapshot
                ?.takeIf { it.isPlaying && activeConfig.enabled && activeConfig.showArtwork }
                ?.artworkUrl
                ?.trim()
                ?.takeIf { it.startsWith("http://") || it.startsWith("https://") }

            if (artworkUrl != null) {
                val key = AssetCacheKey(activeConfig.appId, artworkUrl)
                cachedImage = assetCache[key]
                val now = nowProviderMs()
                val cooledDown = (assetCooldownUntilMs[key] ?: 0L) > now
                shouldFetchAsset = cachedImage == null && !cooledDown && inFlightAssetKeys.add(key)
            } else {
                cachedImage = null
                shouldFetchAsset = false
            }
        }

        val payload = buildPresencePayload(
            track = trackSnapshot,
            config = activeConfig,
            largeImageAsset = cachedImage,
            nowMs = nowProviderMs(),
        )
        runCatching { socket.send(payload.toString()) }

        val targetUrl = trackSnapshot?.artworkUrl?.trim()
        if (shouldFetchAsset && !targetUrl.isNullOrBlank()) {
            val key = AssetCacheKey(activeConfig.appId, targetUrl)
            scope.launch {
                try {
                    val resolved = fetchExternalAsset(
                        token = activeToken,
                        appId = activeConfig.appId,
                        artworkUrl = targetUrl,
                    )
                    val shouldRefreshPresence: Boolean
                    synchronized(stateLock) {
                        inFlightAssetKeys.remove(key)
                        if (resolved != null) {
                            assetCache[key] = resolved
                            assetCooldownUntilMs.remove(key)
                        } else {
                            assetCooldownUntilMs[key] = nowProviderMs() + ASSET_FAILURE_COOLDOWN_MS
                        }
                        shouldRefreshPresence = resolved != null &&
                            !disposed &&
                            isReady &&
                            config.enabled &&
                            currentTrack?.id == trackSnapshot.id &&
                            currentTrack?.isPlaying == true &&
                            currentTrack?.artworkUrl?.trim() == targetUrl
                    }
                    if (shouldRefreshPresence) {
                        pushCurrentPresence()
                    }
                } catch (t: Throwable) {
                    synchronized(stateLock) {
                        inFlightAssetKeys.remove(key)
                        assetCooldownUntilMs[key] = nowProviderMs() + ASSET_FAILURE_COOLDOWN_MS
                    }
                    Log.w(
                        TAG,
                        "External asset lookup failed: ${DiscordApplication.redactSensitiveText(t.message.orEmpty(), activeToken)}",
                    )
                }
            }
        }
    }

    private fun fetchExternalAsset(
        token: String,
        appId: String,
        artworkUrl: String,
    ): String? {
        val bodyJson = JSONObject().apply {
            put("urls", JSONArray().put(artworkUrl))
        }.toString()
        val request = Request.Builder()
            .url(DiscordApplication.externalAssetsUrl(appId))
            .header("Authorization", token)
            .header("Content-Type", "application/json")
            .post(bodyJson.toRequestBody(JSON_MEDIA_TYPE))
            .build()

        httpClient.newCall(request).execute().use { response ->
            if (response.code == 429) {
                val retryAfterSec = response.header("Retry-After")?.toDoubleOrNull()?.coerceAtLeast(1.0)
                if (retryAfterSec != null) {
                    synchronized(stateLock) {
                        assetCooldownUntilMs[AssetCacheKey(appId, artworkUrl)] =
                            nowProviderMs() + (retryAfterSec * 1000.0).toLong()
                    }
                }
                return null
            }
            if (!response.isSuccessful) {
                Log.w(TAG, "Discord external-assets HTTP ${response.code}")
                return null
            }
            val rawBody = response.body?.string().orEmpty()
            if (rawBody.isBlank()) return null
            val array = runCatching { JSONArray(rawBody) }.getOrNull() ?: return null
            if (array.length() == 0) return null
            val first = array.optJSONObject(0) ?: return null
            val externalPath = first.optString("external_asset_path", "").trim()
            if (externalPath.isEmpty()) return null
            return DiscordApplication.normalizeExternalAssetImage(externalPath)
        }
    }

    private inner class GatewayListener(
        private val activeToken: String,
    ) : WebSocketListener() {

        override fun onMessage(webSocket: WebSocket, text: String) {
            val payload = runCatching { JSONObject(text) }.getOrNull() ?: return
            val op = payload.optInt("op", -1)
            val seq = if (payload.isNull("s")) null else payload.optInt("s", -1).takeIf { it >= 0 }
            if (seq != null) {
                synchronized(stateLock) {
                    if (this@DiscordRpcClient.webSocket === webSocket) {
                        lastSequence = seq
                    }
                }
            }

            when (op) {
                OP_HELLO -> {
                    val intervalMs = payload.optJSONObject("d")
                        ?.optLong("heartbeat_interval", DEFAULT_HEARTBEAT_INTERVAL_MS)
                        ?: DEFAULT_HEARTBEAT_INTERVAL_MS
                    startHeartbeatLoop(webSocket, intervalMs)

                    val resumeInfo: Pair<String, Int>?
                    synchronized(stateLock) {
                        val sid = sessionId
                        val lastSeq = lastSequence
                        resumeInfo = if (!sid.isNullOrBlank() && lastSeq != null) sid to lastSeq else null
                    }
                    val authFrame = if (resumeInfo != null) {
                        buildResumePayload(activeToken, resumeInfo.first, resumeInfo.second)
                    } else {
                        buildIdentifyPayload(activeToken, Locale.getDefault().toLanguageTag())
                    }
                    webSocket.send(authFrame.toString())
                }

                OP_HEARTBEAT -> {
                    val currentSeq = synchronized(stateLock) { lastSequence }
                    webSocket.send(buildHeartbeatPayload(currentSeq).toString())
                }

                OP_HEARTBEAT_ACK -> {
                    synchronized(stateLock) {
                        heartbeatAcked = true
                    }
                }

                OP_DISPATCH -> {
                    val eventName = payload.optString("t", "")
                    val data = payload.optJSONObject("d")
                    when (eventName) {
                        "READY" -> {
                            val userObj = data?.optJSONObject("user")
                            val displayName = userObj?.optString("global_name", "")?.takeIf { it.isNotBlank() }
                                ?: userObj?.optString("username", "")?.takeIf { it.isNotBlank() }
                            val sid = data?.optString("session_id", "")?.takeIf { it.isNotBlank() }
                            val resumeUrl = data?.optString("resume_gateway_url", "")?.takeIf { it.isNotBlank() }
                            synchronized(stateLock) {
                                if (this@DiscordRpcClient.webSocket !== webSocket) return
                                sessionId = sid
                                resumeGatewayUrl = resumeUrl
                                connectedUsername = displayName
                                isReady = true
                                reconnectAttempts = 0
                                _status.value = DiscordRpcStatus.Connected(displayName)
                            }
                            pushCurrentPresence()
                        }

                        "RESUMED" -> {
                            val displayName: String?
                            synchronized(stateLock) {
                                if (this@DiscordRpcClient.webSocket !== webSocket) return
                                isReady = true
                                reconnectAttempts = 0
                                displayName = connectedUsername
                                _status.value = DiscordRpcStatus.Connected(displayName)
                            }
                            pushCurrentPresence()
                        }
                    }
                }

                OP_RECONNECT -> {
                    Log.i(TAG, "Discord gateway requested reconnect")
                    runCatching { webSocket.close(CLOSE_RECONNECT, "Gateway reconnect") }
                }

                OP_INVALID_SESSION -> {
                    val resumable = payload.optBoolean("d", false)
                    synchronized(stateLock) {
                        if (!resumable) {
                            sessionId = null
                            lastSequence = null
                        }
                        isReady = false
                    }
                    runCatching { webSocket.close(CLOSE_RECONNECT, "Invalid session") }
                }
            }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            runCatching { webSocket.close(code, reason) }
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            handleSocketEnded(webSocket, code, reason)
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            val safeMsg = DiscordApplication.redactSensitiveText(t.message ?: "Connection failed", activeToken)
            Log.w(TAG, "Discord gateway socket error: $safeMsg")
            handleSocketEnded(webSocket, code = -1, reason = safeMsg)
        }

        private fun handleSocketEnded(socket: WebSocket, code: Int, reason: String) {
            val shouldReconnect: Boolean
            synchronized(stateLock) {
                if (this@DiscordRpcClient.webSocket !== socket) return
                this@DiscordRpcClient.webSocket = null
                heartbeatJob?.cancel()
                heartbeatJob = null
                isReady = false

                if (code in FATAL_AUTH_CLOSE_CODES) {
                    authRejected = true
                    sessionId = null
                    lastSequence = null
                    connectedUsername = null
                    _status.value = DiscordRpcStatus.AuthFailed
                    return
                }
                if (code == CLOSE_NORMAL || disposed || !config.enabled || token != activeToken) {
                    if (!authRejected) {
                        _status.value = DiscordRpcStatus.Disconnected
                    }
                    return
                }
                if (code == 4007 || code == 4009) {
                    sessionId = null
                    lastSequence = null
                }
                if (reason.isNotBlank() && code < 0) {
                    _status.value = DiscordRpcStatus.Error(reason)
                }
                shouldReconnect = true
            }
            if (shouldReconnect) {
                scheduleReconnect()
            }
        }
    }

    companion object {
        private const val TAG = "SonataDiscordRpc"

        internal const val OP_DISPATCH = 0
        internal const val OP_HEARTBEAT = 1
        internal const val OP_IDENTIFY = 2
        internal const val OP_PRESENCE_UPDATE = 3
        internal const val OP_RESUME = 6
        internal const val OP_RECONNECT = 7
        internal const val OP_INVALID_SESSION = 9
        internal const val OP_HELLO = 10
        internal const val OP_HEARTBEAT_ACK = 11

        private const val CLOSE_NORMAL = 1000
        private const val CLOSE_RECONNECT = 4000
        private const val CLOSE_ZOMBIE = 4001
        private val FATAL_AUTH_CLOSE_CODES = setOf(4004, 4010, 4011, 4013, 4014)

        private const val DEFAULT_HEARTBEAT_INTERVAL_MS = 41_250L
        private const val MIN_HEARTBEAT_INTERVAL_MS = 5_000L
        private const val BASE_RECONNECT_DELAY_MS = 1_500L
        private const val MAX_RECONNECT_DELAY_MS = 30_000L
        private const val MAX_BACKOFF_SHIFT = 5
        private const val MAX_CACHED_ASSETS = 200
        private const val ASSET_FAILURE_COOLDOWN_MS = 60_000L

        private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
        private val YOUTUBE_ID_REGEX = Regex("""^[A-Za-z0-9_-]{6,32}$""")

        fun defaultOkHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .writeTimeout(15, TimeUnit.SECONDS)
            .pingInterval(0, TimeUnit.MILLISECONDS)
            .build()

        internal fun buildHeartbeatPayload(sequence: Int?): JSONObject = JSONObject().apply {
            put("op", OP_HEARTBEAT)
            if (sequence != null) {
                put("d", sequence)
            } else {
                put("d", JSONObject.NULL)
            }
        }

        internal fun buildIdentifyPayload(
            token: String,
            locale: String = "en-US",
        ): JSONObject {
            val deviceName = runCatching { Build.MODEL }.getOrNull()?.takeIf { it.isNotBlank() } ?: "Android"
            val osRelease = runCatching { Build.VERSION.RELEASE }.getOrNull()?.takeIf { it.isNotBlank() } ?: "14"
            return JSONObject().apply {
                put("op", OP_IDENTIFY)
                put(
                    "d",
                    JSONObject().apply {
                        put("token", token)
                        put("capabilities", 16381)
                        put(
                            "properties",
                            JSONObject().apply {
                                put("os", "Android")
                                put("browser", "Discord Android")
                                put("device", deviceName)
                                put("system_locale", locale.ifBlank { "en-US" })
                                put("client_version", "240.0")
                                put("os_version", osRelease)
                                put("release_channel", "googleRelease")
                                put("client_build_number", 240000)
                            },
                        )
                        put("compress", false)
                        put(
                            "presence",
                            JSONObject().apply {
                                put("status", "online")
                                put("since", 0)
                                put("activities", JSONArray())
                                put("afk", false)
                            },
                        )
                    },
                )
            }
        }

        internal fun buildResumePayload(
            token: String,
            sessionId: String,
            sequence: Int,
        ): JSONObject = JSONObject().apply {
            put("op", OP_RESUME)
            put(
                "d",
                JSONObject().apply {
                    put("token", token)
                    put("session_id", sessionId)
                    put("seq", sequence)
                },
            )
        }

        internal fun computeAdjustedTimestamps(
            positionMs: Long,
            durationMs: Long,
            playbackSpeed: Float,
            nowMs: Long,
        ): Pair<Long, Long?> {
            val speed = if (playbackSpeed.isFinite()) playbackSpeed.coerceIn(0.5f, 2.0f) else 1.0f
            val safePos = positionMs.coerceAtLeast(0L)
            val elapsedRealMs = (safePos / speed).toLong()
            val startMs = nowMs - elapsedRealMs
            val endMs = if (durationMs > 0L) {
                val safeEndPos = safePos.coerceAtMost(durationMs)
                val remainingRealMs = ((durationMs - safeEndPos).coerceAtLeast(0L) / speed).toLong()
                nowMs + remainingRealMs
            } else {
                null
            }
            return startMs to endMs
        }

        internal fun formatTrackDetails(title: String, playbackSpeed: Float): String {
            val cleanTitle = title.trim().ifEmpty { "Unknown Track" }
            val speed = if (playbackSpeed.isFinite()) playbackSpeed.coerceIn(0.5f, 2.0f) else 1.0f
            val withBadge = if (abs(speed - 1.0f) >= 0.05f) {
                val formatted = String.format(Locale.US, "%.2fx", speed)
                    .replace(Regex("""0+x$"""), "x")
                    .replace(".x", "x")
                "$cleanTitle · $formatted"
            } else {
                cleanTitle
            }
            return clampDiscordField(withBadge)
        }

        internal fun clampDiscordField(raw: String, maxLength: Int = 128): String {
            val trimmed = raw.trim()
            if (trimmed.length <= maxLength) return trimmed
            if (maxLength <= 1) return trimmed.take(maxLength)
            return trimmed.take(maxLength - 1) + "…"
        }

        internal fun buildListenUrl(trackId: String): String? {
            val trimmed = trackId.trim()
            if (!YOUTUBE_ID_REGEX.matches(trimmed)) return null
            return "https://music.youtube.com/watch?v=$trimmed"
        }

        internal fun buildPresencePayload(
            track: DiscordPresenceTrack?,
            config: DiscordPresenceConfig,
            largeImageAsset: String?,
            nowMs: Long = System.currentTimeMillis(),
        ): JSONObject {
            val activities = JSONArray()
            if (config.enabled && track != null && track.isPlaying && track.title.isNotBlank()) {
                val activity = JSONObject().apply {
                    put("name", DiscordApplication.DEFAULT_APP_NAME)
                    put("type", config.activityType.gatewayType)
                    put("application_id", DiscordApplication.resolveAppId(config.appId))
                    put("details", formatTrackDetails(track.title, track.playbackSpeed))
                    if (track.artist.isNotBlank()) {
                        put("state", clampDiscordField(track.artist))
                    }
                    if (config.showTimestamps) {
                        val (startMs, endMs) = computeAdjustedTimestamps(
                            positionMs = track.positionMs,
                            durationMs = track.durationMs,
                            playbackSpeed = track.playbackSpeed,
                            nowMs = nowMs,
                        )
                        put(
                            "timestamps",
                            JSONObject().apply {
                                put("start", startMs)
                                if (endMs != null) {
                                    put("end", endMs)
                                }
                            },
                        )
                    }
                    if (config.showArtwork && !largeImageAsset.isNullOrBlank()) {
                        val largeText = track.album.takeIf { it.isNotBlank() } ?: track.title
                        put(
                            "assets",
                            JSONObject().apply {
                                put("large_image", largeImageAsset)
                                put("large_text", clampDiscordField(largeText))
                            },
                        )
                    }
                    if (config.showButtons) {
                        val buttonUrl = buildListenUrl(track.id) ?: DiscordApplication.PROJECT_URL
                        put("buttons", JSONArray().put(DiscordApplication.DEFAULT_BUTTON_LABEL))
                        put(
                            "metadata",
                            JSONObject().apply {
                                put("button_urls", JSONArray().put(buttonUrl))
                            },
                        )
                    }
                }
                activities.put(activity)
            }

            return JSONObject().apply {
                put("op", OP_PRESENCE_UPDATE)
                put(
                    "d",
                    JSONObject().apply {
                        put("since", if (activities.length() > 0) nowMs else JSONObject.NULL)
                        put("activities", activities)
                        put("status", "online")
                        put("afk", false)
                    },
                )
            }
        }
    }
}
