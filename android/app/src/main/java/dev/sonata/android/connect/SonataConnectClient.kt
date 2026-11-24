package dev.sonata.android.connect

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Locale
import java.util.concurrent.TimeUnit
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

const val CONNECT_DISCOVERY_PORT = 45389

/**
 * UDP LAN discovery beacon broadcast by Sonata Connect hosts on port [CONNECT_DISCOVERY_PORT].
 */
data class ConnectBeacon(
    val deviceId: String,
    val deviceName: String,
    val platform: String,
    val httpPort: Int,
    val protocolVersion: Int = 1,
    val hostAddress: String = "",
    val lastSeenMs: Long = System.currentTimeMillis(),
) {
    fun baseUrl(): String {
        val host = hostAddress.trim()
        require(host.isNotEmpty()) { "Beacon hostAddress is empty" }
        val formattedHost = if (host.contains(':') && !host.startsWith("[")) "[$host]" else host
        return "http://$formattedHost:$httpPort"
    }

    fun toJson(): String = JSONObject().apply {
        put("device_id", deviceId)
        put("device_name", deviceName)
        put("platform", platform)
        put("http_port", httpPort)
        put("protocol_version", protocolVersion)
    }.toString()

    companion object {
        fun fromJson(
            rawJson: String,
            hostAddress: String = "",
            nowMs: Long = System.currentTimeMillis(),
        ): ConnectBeacon? = runCatching {
            val obj = JSONObject(rawJson)
            val deviceId = obj.optString("device_id").trim()
            val deviceName = obj.optString("device_name").trim()
            val httpPort = obj.optInt("http_port", -1)
            if (deviceId.isEmpty() || deviceName.isEmpty() || httpPort !in 1..65535) {
                return null
            }
            ConnectBeacon(
                deviceId = deviceId,
                deviceName = deviceName,
                platform = obj.optString("platform", "unknown").ifBlank { "unknown" },
                httpPort = httpPort,
                protocolVersion = obj.optInt("protocol_version", 1).coerceAtLeast(1),
                hostAddress = hostAddress.trim(),
                lastSeenMs = nowMs,
            )
        }.getOrNull()
    }
}

/**
 * Metadata returned by `GET /connect/info`.
 */
data class ConnectDeviceInfo(
    val deviceId: String,
    val deviceName: String,
    val platform: String,
    val version: String,
    val paired: Boolean = false,
    val protocolVersion: Int = 1,
) {
    fun toJson(): String = JSONObject().apply {
        put("device_id", deviceId)
        put("device_name", deviceName)
        put("platform", platform)
        put("version", version)
        put("paired", paired)
        put("protocol_version", protocolVersion)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): ConnectDeviceInfo? = runCatching {
            val obj = JSONObject(rawJson)
            val deviceId = obj.optString("device_id").trim()
            val deviceName = obj.optString("device_name").trim()
            if (deviceId.isEmpty() || deviceName.isEmpty()) return null
            ConnectDeviceInfo(
                deviceId = deviceId,
                deviceName = deviceName,
                platform = obj.optString("platform", "unknown").ifBlank { "unknown" },
                version = obj.optString("version", ""),
                paired = obj.optBoolean("paired", false),
                protocolVersion = obj.optInt("protocol_version", 1).coerceAtLeast(1),
            )
        }.getOrNull()
    }
}

/**
 * Pairing payload sent to `POST /connect/pair`.
 */
data class PairRequest(
    val clientDeviceId: String,
    val clientDeviceName: String,
    val pin: String,
) {
    fun toJson(): String = JSONObject().apply {
        put("client_device_id", clientDeviceId)
        put("client_device_name", clientDeviceName)
        put("pin", pin)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairRequest? = runCatching {
            val obj = JSONObject(rawJson)
            val id = obj.optString("client_device_id").trim()
            val name = obj.optString("client_device_name").trim()
            val pin = obj.optString("pin").trim()
            if (id.isEmpty() || name.isEmpty() || !SonataConnectCrypto.isValidPin(pin)) return null
            PairRequest(clientDeviceId = id, clientDeviceName = name, pin = pin)
        }.getOrNull()
    }
}

/**
 * Pairing acknowledgement returned by `POST /connect/pair`.
 */
data class PairAck(
    val accepted: Boolean,
    val deviceId: String,
    val deviceName: String,
    val sharedSecret: String? = null,
    val message: String? = null,
) {
    fun toJson(): String = JSONObject().apply {
        put("accepted", accepted)
        put("device_id", deviceId)
        put("device_name", deviceName)
        if (!sharedSecret.isNullOrBlank()) put("shared_secret", sharedSecret)
        if (!message.isNullOrBlank()) put("message", message)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairAck? = runCatching {
            val obj = JSONObject(rawJson)
            val deviceId = obj.optString("device_id").trim()
            val deviceName = obj.optString("device_name").trim()
            if (deviceId.isEmpty() && deviceName.isEmpty() && !obj.has("accepted")) return null
            PairAck(
                accepted = obj.optBoolean("accepted", false),
                deviceId = deviceId,
                deviceName = deviceName,
                sharedSecret = obj.optString("shared_secret", "").takeIf { it.isNotBlank() },
                message = obj.optString("message", "").takeIf { it.isNotBlank() },
            )
        }.getOrNull()
    }
}

/**
 * Lightweight track representation carried in Sonata Connect state snapshots and `TransferQueue`.
 */
data class ConnectTrackSnapshot(
    val id: String,
    val title: String,
    val artist: String,
    val album: String = "",
    val durationSecs: Long = 0L,
    val thumbnailUrl: String? = null,
) {
    fun toJsonObject(): JSONObject = JSONObject().apply {
        put("id", id)
        put("title", title)
        put("artist", artist)
        put("album", album)
        put("duration_secs", durationSecs.coerceAtLeast(0L))
        if (!thumbnailUrl.isNullOrBlank()) put("thumbnail_url", thumbnailUrl)
    }

    companion object {
        fun fromJsonObject(obj: JSONObject): ConnectTrackSnapshot? {
            val id = obj.optString("id").trim()
            val title = obj.optString("title").trim()
            if (id.isEmpty() || title.isEmpty()) return null
            return ConnectTrackSnapshot(
                id = id,
                title = title,
                artist = obj.optString("artist", ""),
                album = obj.optString("album", ""),
                durationSecs = obj.optLong("duration_secs", 0L).coerceAtLeast(0L),
                thumbnailUrl = obj.optString("thumbnail_url", "").takeIf { it.isNotBlank() },
            )
        }
    }
}

/**
 * Playback state snapshot returned by `GET /connect/state`.
 */
data class ConnectStateSnapshot(
    val deviceId: String,
    val deviceName: String,
    val isPlaying: Boolean,
    val positionMs: Long,
    val durationMs: Long,
    val volume: Float,
    val activeTrack: ConnectTrackSnapshot?,
    val queue: List<ConnectTrackSnapshot> = emptyList(),
    val queueIndex: Int = 0,
    val timestampMs: Long = System.currentTimeMillis(),
) {
    fun toJson(): String = JSONObject().apply {
        put("device_id", deviceId)
        put("device_name", deviceName)
        put("is_playing", isPlaying)
        put("position_ms", positionMs.coerceAtLeast(0L))
        put("duration_ms", durationMs.coerceAtLeast(0L))
        put("volume", volume.coerceIn(0f, 1f).toDouble())
        if (activeTrack != null) {
            put("active_track", activeTrack.toJsonObject())
        }
        put("queue", JSONArray().apply { queue.forEach { put(it.toJsonObject()) } })
        put("queue_index", queueIndex.coerceAtLeast(0))
        put("timestamp_ms", timestampMs)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): ConnectStateSnapshot? = runCatching {
            val obj = JSONObject(rawJson)
            val deviceId = obj.optString("device_id").trim()
            val deviceName = obj.optString("device_name").trim()
            if (deviceId.isEmpty() && deviceName.isEmpty()) return null
            val activeTrack = obj.optJSONObject("active_track")?.let {
                ConnectTrackSnapshot.fromJsonObject(it)
            }
            val queue = buildList {
                val arr = obj.optJSONArray("queue")
                if (arr != null) {
                    for (i in 0 until arr.length()) {
                        arr.optJSONObject(i)?.let { ConnectTrackSnapshot.fromJsonObject(it) }?.let(::add)
                    }
                }
            }
            ConnectStateSnapshot(
                deviceId = deviceId,
                deviceName = deviceName,
                isPlaying = obj.optBoolean("is_playing", false),
                positionMs = obj.optLong("position_ms", 0L).coerceAtLeast(0L),
                durationMs = obj.optLong("duration_ms", 0L).coerceAtLeast(0L),
                volume = obj.optDouble("volume", 1.0).toFloat().coerceIn(0f, 1f),
                activeTrack = activeTrack,
                queue = queue,
                queueIndex = obj.optInt("queue_index", 0).coerceAtLeast(0),
                timestampMs = obj.optLong("timestamp_ms", System.currentTimeMillis()),
            )
        }.getOrNull()
    }
}

/**
 * Signed control commands sent to `POST /connect/command`.
 */
sealed class ConnectCommand {
    data object Play : ConnectCommand()
    data object Pause : ConnectCommand()
    data object Next : ConnectCommand()
    data object Prev : ConnectCommand()
    data class SeekAbsolute(val positionMs: Long) : ConnectCommand()
    data class SetVolume(val volume: Float) : ConnectCommand()
    data class TransferQueue(
        val tracks: List<ConnectTrackSnapshot>,
        val startIndex: Int = 0,
        val positionMs: Long = 0L,
        val playWhenReady: Boolean = true,
    ) : ConnectCommand()

    fun toJson(): String = JSONObject().apply {
        when (this@ConnectCommand) {
            is Play -> put("command", "Play")
            is Pause -> put("command", "Pause")
            is Next -> put("command", "Next")
            is Prev -> put("command", "Prev")
            is SeekAbsolute -> {
                put("command", "SeekAbsolute")
                put("position_ms", positionMs.coerceAtLeast(0L))
            }
            is SetVolume -> {
                put("command", "SetVolume")
                put("volume", volume.coerceIn(0f, 1f).toDouble())
            }
            is TransferQueue -> {
                put("command", "TransferQueue")
                put("tracks", JSONArray().apply { tracks.forEach { put(it.toJsonObject()) } })
                put("start_index", startIndex.coerceAtLeast(0))
                put("position_ms", positionMs.coerceAtLeast(0L))
                put("play_when_ready", playWhenReady)
            }
        }
    }.toString()

    companion object {
        fun fromJson(rawJson: String): ConnectCommand? = runCatching {
            val obj = JSONObject(rawJson)
            val tag = obj.optString("command").ifBlank { obj.optString("type") }.trim()
            when (tag) {
                "Play" -> Play
                "Pause" -> Pause
                "Next" -> Next
                "Prev" -> Prev
                "SeekAbsolute" -> SeekAbsolute(obj.optLong("position_ms", 0L).coerceAtLeast(0L))
                "SetVolume" -> SetVolume(obj.optDouble("volume", 1.0).toFloat().coerceIn(0f, 1f))
                "TransferQueue" -> {
                    val tracks = buildList {
                        val arr = obj.optJSONArray("tracks")
                        if (arr != null) {
                            for (i in 0 until arr.length()) {
                                arr.optJSONObject(i)?.let { ConnectTrackSnapshot.fromJsonObject(it) }?.let(::add)
                            }
                        }
                    }
                    TransferQueue(
                        tracks = tracks,
                        startIndex = obj.optInt("start_index", 0).coerceAtLeast(0),
                        positionMs = obj.optLong("position_ms", 0L).coerceAtLeast(0L),
                        playWhenReady = obj.optBoolean("play_when_ready", true),
                    )
                }
                else -> null
            }
        }.getOrNull()
    }
}

/**
 * Cryptographic helpers for Sonata Connect 6-digit PIN validation and HMAC-SHA1
 * request signing / constant-time verification.
 */
object SonataConnectCrypto {
    private const val HMAC_ALGORITHM = "HmacSHA1"

    fun isValidPin(pin: String): Boolean = pin.length == 6 && pin.all { it.isDigit() }

    fun signPayload(secret: String, payload: String): String =
        signPayload(secret, payload.toByteArray(StandardCharsets.UTF_8))

    fun signPayload(secret: String, payload: ByteArray): String {
        require(secret.isNotEmpty()) { "HMAC secret must not be empty" }
        val mac = Mac.getInstance(HMAC_ALGORITHM)
        mac.init(SecretKeySpec(secret.toByteArray(StandardCharsets.UTF_8), HMAC_ALGORITHM))
        val digest = mac.doFinal(payload)
        return digest.joinToString("") { byte -> "%02x".format(Locale.US, byte) }
    }

    fun verifySignature(secret: String, payload: String, signatureHex: String): Boolean =
        verifySignature(secret, payload.toByteArray(StandardCharsets.UTF_8), signatureHex)

    fun verifySignature(secret: String, payload: ByteArray, signatureHex: String): Boolean {
        if (secret.isEmpty() || signatureHex.isBlank()) return false
        val expected = signPayload(secret, payload)
        val normalized = signatureHex.trim().lowercase(Locale.ROOT)
        return MessageDigest.isEqual(
            expected.toByteArray(StandardCharsets.US_ASCII),
            normalized.toByteArray(StandardCharsets.US_ASCII),
        )
    }
}

/**
 * Sonata Connect Android LAN client:
 * - Listens for UDP discovery beacons on port [CONNECT_DISCOVERY_PORT] (`45389`).
 * - Speaks HTTP/JSON endpoints (`/connect/info`, `/connect/pair`, `/connect/state`,
 *   `/connect/command`) via OkHttp with HMAC-SHA1 request signing.
 */
class SonataConnectClient(
    private val localDeviceId: String,
    private val httpClient: OkHttpClient = defaultHttpClient(),
) {
    private val beaconsLock = Any()
    private val beaconsById = LinkedHashMap<String, ConnectBeacon>()
    private val _discoveredDevices = MutableStateFlow<List<ConnectBeacon>>(emptyList())
    val discoveredDevices: StateFlow<List<ConnectBeacon>> = _discoveredDevices.asStateFlow()

    @Volatile
    private var discoveryJob: Job? = null

    fun startDiscovery(scope: CoroutineScope, port: Int = CONNECT_DISCOVERY_PORT) {
        if (discoveryJob?.isActive == true) return
        discoveryJob = scope.launch(Dispatchers.IO) {
            var socket: DatagramSocket? = null
            try {
                socket = DatagramSocket(null).apply {
                    reuseAddress = true
                    broadcast = true
                    soTimeout = SOCKET_TIMEOUT_MS
                    bind(InetSocketAddress(port))
                }
                val buffer = ByteArray(MAX_BEACON_BYTES)
                while (isActive) {
                    val packet = DatagramPacket(buffer, buffer.size)
                    try {
                        socket.receive(packet)
                        val payload = String(packet.data, packet.offset, packet.length, StandardCharsets.UTF_8)
                        val senderHost = packet.address?.hostAddress.orEmpty()
                        ingestBeaconPacket(payload, senderHost, System.currentTimeMillis())
                    } catch (_: SocketTimeoutException) {
                        pruneStaleBeacons(System.currentTimeMillis())
                    }
                }
            } catch (_: Exception) {
                // Socket closed or unavailable on restricted network interface.
            } finally {
                runCatching { socket?.close() }
            }
        }
    }

    fun stopDiscovery() {
        discoveryJob?.cancel()
        discoveryJob = null
    }

    internal fun ingestBeaconPacket(
        rawJson: String,
        senderHost: String,
        nowMs: Long = System.currentTimeMillis(),
    ): ConnectBeacon? {
        val beacon = ConnectBeacon.fromJson(rawJson, senderHost, nowMs) ?: return null
        if (beacon.deviceId == localDeviceId) return null
        synchronized(beaconsLock) {
            beaconsById[beacon.deviceId] = beacon
            pruneStaleBeaconsLocked(nowMs)
            _discoveredDevices.value = beaconsById.values.sortedBy { it.deviceName.lowercase(Locale.ROOT) }
        }
        return beacon
    }

    internal fun pruneStaleBeacons(nowMs: Long = System.currentTimeMillis()) {
        synchronized(beaconsLock) {
            if (pruneStaleBeaconsLocked(nowMs)) {
                _discoveredDevices.value = beaconsById.values.sortedBy { it.deviceName.lowercase(Locale.ROOT) }
            }
        }
    }

    private fun pruneStaleBeaconsLocked(nowMs: Long): Boolean {
        val iterator = beaconsById.entries.iterator()
        var changed = false
        while (iterator.hasNext()) {
            val entry = iterator.next()
            if (nowMs - entry.value.lastSeenMs > BEACON_TTL_MS) {
                iterator.remove()
                changed = true
            }
        }
        return changed
    }

    suspend fun fetchInfo(baseUrl: String): Result<ConnectDeviceInfo> = withContext(Dispatchers.IO) {
        runCatching {
            val url = "${normalizeBaseUrl(baseUrl)}/connect/info"
            val request = Request.Builder().url(url).get().build()
            httpClient.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    error("Connect info failed with HTTP ${response.code}")
                }
                ConnectDeviceInfo.fromJson(body) ?: error("Invalid /connect/info payload")
            }
        }
    }

    suspend fun pair(
        baseUrl: String,
        clientDeviceName: String,
        pin: String,
    ): Result<PairAck> = withContext(Dispatchers.IO) {
        runCatching {
            val cleanPin = pin.trim()
            require(SonataConnectCrypto.isValidPin(cleanPin)) { "Pairing PIN must be 6 digits" }
            val payload = PairRequest(
                clientDeviceId = localDeviceId,
                clientDeviceName = clientDeviceName.trim().ifEmpty { DEFAULT_DEVICE_NAME },
                pin = cleanPin,
            ).toJson()
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/pair")
                .post(payload.toRequestBody(JSON_MEDIA_TYPE))
                .build()
            httpClient.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                val parsed = PairAck.fromJson(body)
                if (!response.isSuccessful) {
                    error(parsed?.message ?: "Pairing failed with HTTP ${response.code}")
                }
                parsed ?: error("Invalid /connect/pair response")
            }
        }
    }

    suspend fun fetchState(
        baseUrl: String,
        sharedSecret: String? = null,
    ): Result<ConnectStateSnapshot> = withContext(Dispatchers.IO) {
        runCatching {
            val url = "${normalizeBaseUrl(baseUrl)}/connect/state"
            val builder = Request.Builder()
                .url(url)
                .header(HEADER_DEVICE_ID, localDeviceId)
                .get()
            if (!sharedSecret.isNullOrBlank()) {
                val timestamp = System.currentTimeMillis().toString()
                builder.header(HEADER_TIMESTAMP, timestamp)
                builder.header(HEADER_SIGNATURE, SonataConnectCrypto.signPayload(sharedSecret, timestamp))
            }
            httpClient.newCall(builder.build()).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    error("Connect state failed with HTTP ${response.code}")
                }
                ConnectStateSnapshot.fromJson(body) ?: error("Invalid /connect/state payload")
            }
        }
    }

    suspend fun sendCommand(
        baseUrl: String,
        sharedSecret: String,
        command: ConnectCommand,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            require(sharedSecret.isNotBlank()) { "Shared secret is required for Connect commands" }
            val bodyJson = command.toJson()
            val signature = SonataConnectCrypto.signPayload(sharedSecret, bodyJson)
            val timestamp = System.currentTimeMillis().toString()
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/command")
                .header(HEADER_DEVICE_ID, localDeviceId)
                .header(HEADER_TIMESTAMP, timestamp)
                .header(HEADER_SIGNATURE, signature)
                .post(bodyJson.toRequestBody(JSON_MEDIA_TYPE))
                .build()
            httpClient.newCall(request).execute().use { response ->
                if (!response.isSuccessful) {
                    error("Connect command rejected with HTTP ${response.code}")
                }
            }
        }
    }

    companion object {
        const val HEADER_SIGNATURE = "X-Sonata-Signature"
        const val HEADER_DEVICE_ID = "X-Sonata-Device-Id"
        const val HEADER_TIMESTAMP = "X-Sonata-Timestamp"
        const val DEFAULT_DEVICE_NAME = "Sonata Android"

        private const val SOCKET_TIMEOUT_MS = 1_500
        private const val MAX_BEACON_BYTES = 4_096
        private const val BEACON_TTL_MS = 15_000L
        private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()

        private fun normalizeBaseUrl(raw: String): String = raw.trim().trimEnd('/')

        private fun defaultHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(4, TimeUnit.SECONDS)
            .readTimeout(6, TimeUnit.SECONDS)
            .writeTimeout(6, TimeUnit.SECONDS)
            .build()
    }
}
