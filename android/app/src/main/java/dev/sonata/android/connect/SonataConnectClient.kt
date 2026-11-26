package dev.sonata.android.connect

import dev.sonata.android.data.runSuspendCatching
import kotlinx.coroutines.CancellationException
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
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Locale
import java.util.concurrent.TimeUnit
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec
import kotlin.math.roundToInt
import kotlin.math.roundToLong

const val CONNECT_DISCOVERY_PORT = 45389
const val CONNECT_DEFAULT_HTTP_PORT = 45390

/**
 * Typed Sonata Connect error codes mapped to localized strings in `EcosystemStrings`.
 */
enum class ConnectError {
    INVALID_PIN,
    PAIRING_REJECTED,
    PAIRING_TIMEOUT,
    PAIRING_FAILED,
    CONNECTION_FAILED,
    TRUST_REVOKED,
    NO_ACTIVE_DEVICE,
    DEVICE_NOT_PAIRED,
    QUEUE_EMPTY,
    COMMAND_FAILED,
}

/**
 * Typed exception raised by [SonataConnectClient] and [ConnectController] carrying a
 * localizable [ConnectError] instead of raw English UI messages.
 */
class ConnectClientException(
    val error: ConnectError,
    val httpStatus: Int? = null,
    cause: Throwable? = null,
) : Exception(error.name, cause)

private fun JSONObject.optNonNullString(key: String): String? =
    if (isNull(key)) null else optString(key, "").trim().takeIf { it.isNotEmpty() }

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
        put("device_id", clientDeviceId)
        put("device_name", clientDeviceName)
        put("pin", pin)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairRequest? = runCatching {
            val obj = JSONObject(rawJson)
            val id = obj.optNonNullString("device_id")
                ?: obj.optNonNullString("client_device_id")
                ?: ""
            val name = obj.optNonNullString("device_name")
                ?: obj.optNonNullString("client_device_name")
                ?: ""
            val pin = obj.optNonNullString("pin") ?: ""
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
        if (!sharedSecret.isNullOrBlank()) put("token", sharedSecret)
        if (!message.isNullOrBlank()) put("message", message)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairAck? = runCatching {
            val obj = JSONObject(rawJson)
            val deviceId = obj.optNonNullString("device_id") ?: ""
            val deviceName = obj.optNonNullString("device_name") ?: ""
            if (deviceId.isEmpty() && deviceName.isEmpty() && !obj.has("accepted")) return null
            PairAck(
                accepted = obj.optBoolean("accepted", false),
                deviceId = deviceId,
                deviceName = deviceName,
                sharedSecret = obj.optNonNullString("token")
                    ?: obj.optNonNullString("shared_secret"),
                message = obj.optNonNullString("message"),
            )
        }.getOrNull()
    }
}

/**
 * Interactive approval pairing initiation payload sent to `POST /connect/pair/request`.
 */
data class PairApprovalRequest(
    val clientDeviceId: String,
    val clientDeviceName: String,
) {
    fun toJson(): String = JSONObject().apply {
        put("device_id", clientDeviceId)
        put("device_name", clientDeviceName)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairApprovalRequest? = runCatching {
            val obj = JSONObject(rawJson)
            val id = obj.optNonNullString("device_id")
                ?: obj.optNonNullString("client_device_id")
                ?: ""
            val name = obj.optNonNullString("device_name")
                ?: obj.optNonNullString("client_device_name")
                ?: ""
            if (id.isEmpty() || name.isEmpty()) return null
            PairApprovalRequest(clientDeviceId = id, clientDeviceName = name)
        }.getOrNull()
    }
}

/**
 * Response returned by `POST /connect/pair/request` (`202 Accepted`).
 */
data class PairApprovalInitAck(
    val requestId: String,
    val status: String,
    val deviceId: String,
    val deviceName: String,
) {
    fun toJson(): String = JSONObject().apply {
        put("request_id", requestId)
        put("status", status)
        put("device_id", deviceId)
        put("device_name", deviceName)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairApprovalInitAck? = runCatching {
            val obj = JSONObject(rawJson)
            val requestId = obj.optNonNullString("request_id") ?: ""
            if (requestId.isEmpty()) return null
            PairApprovalInitAck(
                requestId = requestId,
                status = obj.optNonNullString("status") ?: "pending",
                deviceId = obj.optNonNullString("device_id") ?: "",
                deviceName = obj.optNonNullString("device_name") ?: "",
            )
        }.getOrNull()
    }
}

/**
 * Interactive pairing approval state returned by `GET /connect/pair/status`.
 */
enum class PairApprovalStatus {
    PENDING,
    APPROVED,
    REJECTED,
}

/**
 * Polling response returned by `GET /connect/pair/status?request_id=<id>`.
 */
data class PairStatusAck(
    val status: PairApprovalStatus,
    val accepted: Boolean,
    val deviceId: String,
    val deviceName: String,
    val sharedSecret: String? = null,
    val message: String? = null,
) {
    fun toJson(): String = JSONObject().apply {
        put(
            "status",
            when (status) {
                PairApprovalStatus.PENDING -> "pending"
                PairApprovalStatus.APPROVED -> "approved"
                PairApprovalStatus.REJECTED -> "rejected"
            },
        )
        put("accepted", accepted)
        put("device_id", deviceId)
        put("device_name", deviceName)
        if (!sharedSecret.isNullOrBlank()) put("token", sharedSecret)
        if (!message.isNullOrBlank()) put("message", message)
    }.toString()

    companion object {
        fun fromJson(rawJson: String): PairStatusAck? = runCatching {
            val obj = JSONObject(rawJson)
            val rawStatus = obj.optNonNullString("status")?.lowercase(Locale.ROOT) ?: return null
            val status = when (rawStatus) {
                "pending" -> PairApprovalStatus.PENDING
                "approved" -> PairApprovalStatus.APPROVED
                else -> PairApprovalStatus.REJECTED
            }
            val accepted = obj.optBoolean("accepted", status == PairApprovalStatus.APPROVED)
            PairStatusAck(
                status = status,
                accepted = accepted,
                deviceId = obj.optNonNullString("device_id") ?: "",
                deviceName = obj.optNonNullString("device_name") ?: "",
                sharedSecret = obj.optNonNullString("token")
                    ?: obj.optNonNullString("shared_secret"),
                message = obj.optNonNullString("message"),
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
        put("source", "YouTubeMusic")
        put("audio_badge", JSONObject.NULL)
    }

    companion object {
        fun fromJsonObject(obj: JSONObject): ConnectTrackSnapshot? {
            val id = obj.optNonNullString("id") ?: ""
            val title = obj.optNonNullString("title") ?: ""
            if (id.isEmpty() || title.isEmpty()) return null
            return ConnectTrackSnapshot(
                id = id,
                title = title,
                artist = obj.optNonNullString("artist") ?: "",
                album = obj.optNonNullString("album") ?: "",
                durationSecs = obj.optLong("duration_secs", 0L).coerceAtLeast(0L),
                thumbnailUrl = obj.optNonNullString("thumbnail_url"),
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
            val deviceId = obj.optNonNullString("device_id") ?: ""
            val deviceName = obj.optNonNullString("device_name") ?: ""
            if (deviceId.isEmpty() && deviceName.isEmpty()) return null

            val isRustSchema =
                obj.has("playing") || obj.has("position_secs") || obj.has("updated_at_unix")
            val isPlaying = if (obj.has("playing")) {
                obj.optBoolean("playing", false) && !obj.optBoolean("paused", false)
            } else {
                obj.optBoolean("is_playing", false)
            }
            val positionMs = if (obj.has("position_ms")) {
                obj.optLong("position_ms", 0L).coerceAtLeast(0L)
            } else {
                (obj.optDouble("position_secs", 0.0) * 1000.0).roundToLong().coerceAtLeast(0L)
            }
            val durationMs = if (obj.has("duration_ms")) {
                obj.optLong("duration_ms", 0L).coerceAtLeast(0L)
            } else {
                (obj.optDouble("duration_secs", 0.0) * 1000.0).roundToLong().coerceAtLeast(0L)
            }
            val rawVolume = obj.optDouble("volume", if (isRustSchema) 100.0 else 1.0)
            val volume = if (isRustSchema || rawVolume > 1.0) {
                (rawVolume / 100.0).toFloat().coerceIn(0f, 1f)
            } else {
                rawVolume.toFloat().coerceIn(0f, 1f)
            }

            val flatTrackId = obj.optNonNullString("track_id")
            val flatTitle = obj.optNonNullString("title")
            val activeTrack = obj.optJSONObject("active_track")?.let {
                ConnectTrackSnapshot.fromJsonObject(it)
            } ?: if (flatTrackId != null || flatTitle != null) {
                ConnectTrackSnapshot(
                    id = flatTrackId ?: flatTitle.orEmpty(),
                    title = flatTitle ?: flatTrackId.orEmpty(),
                    artist = obj.optNonNullString("artist") ?: "",
                    album = obj.optNonNullString("album") ?: "",
                    durationSecs = (durationMs / 1000L).coerceAtLeast(0L),
                    thumbnailUrl = obj.optNonNullString("thumbnail_url"),
                )
            } else {
                null
            }

            val queue = buildList {
                val arr = obj.optJSONArray("queue")
                if (arr != null) {
                    for (i in 0 until arr.length()) {
                        arr.optJSONObject(i)?.let { ConnectTrackSnapshot.fromJsonObject(it) }?.let(::add)
                    }
                }
            }
            val queueIndex = when {
                obj.has("current_index") && !obj.isNull("current_index") ->
                    obj.optInt("current_index", 0).coerceAtLeast(0)
                obj.has("queue_index") && !obj.isNull("queue_index") ->
                    obj.optInt("queue_index", 0).coerceAtLeast(0)
                else -> 0
            }
            val timestampMs = when {
                obj.has("timestamp_ms") && !obj.isNull("timestamp_ms") ->
                    obj.optLong("timestamp_ms", System.currentTimeMillis())
                obj.has("updated_at_unix") && !obj.isNull("updated_at_unix") ->
                    obj.optLong("updated_at_unix", 0L).coerceAtLeast(0L) * 1000L
                else -> System.currentTimeMillis()
            }

            ConnectStateSnapshot(
                deviceId = deviceId,
                deviceName = deviceName,
                isPlaying = isPlaying,
                positionMs = positionMs,
                durationMs = durationMs,
                volume = volume,
                activeTrack = activeTrack,
                queue = queue,
                queueIndex = queueIndex,
                timestampMs = timestampMs,
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
            is Play -> put("command", "play")
            is Pause -> put("command", "pause")
            is Next -> put("command", "next")
            is Prev -> put("command", "prev")
            is SeekAbsolute -> {
                put("command", "seek_absolute")
                put("position_secs", positionMs.coerceAtLeast(0L).toDouble() / 1000.0)
            }
            is SetVolume -> {
                put("command", "set_volume")
                put("volume", (volume.coerceIn(0f, 1f) * 100f).roundToInt().coerceIn(0, 100))
            }
            is TransferQueue -> {
                put("command", "transfer_queue")
                put("tracks", JSONArray().apply { tracks.forEach { put(it.toJsonObject()) } })
                put("start_index", startIndex.coerceAtLeast(0))
                put("position_secs", positionMs.coerceAtLeast(0L).toDouble() / 1000.0)
                put("play_when_ready", playWhenReady)
            }
        }
    }.toString()

    companion object {
        fun fromJson(rawJson: String): ConnectCommand? = runCatching {
            val obj = JSONObject(rawJson)
            val tag = obj.optNonNullString("command")
                ?: obj.optNonNullString("type")
                ?: ""
            when (tag.lowercase(Locale.ROOT)) {
                "play" -> Play
                "pause" -> Pause
                "next" -> Next
                "prev" -> Prev
                "seek_absolute", "seekabsolute" -> {
                    val posMs = if (obj.has("position_ms")) {
                        obj.optLong("position_ms", 0L).coerceAtLeast(0L)
                    } else {
                        (obj.optDouble("position_secs", 0.0) * 1000.0).roundToLong().coerceAtLeast(0L)
                    }
                    SeekAbsolute(posMs)
                }
                "set_volume", "setvolume" -> {
                    val raw = obj.optDouble("volume", 100.0)
                    val normalized = if (tag == "set_volume" || raw > 1.0) {
                        (raw / 100.0).toFloat().coerceIn(0f, 1f)
                    } else {
                        raw.toFloat().coerceIn(0f, 1f)
                    }
                    SetVolume(normalized)
                }
                "transfer_queue", "transferqueue" -> {
                    val tracks = buildList {
                        val arr = obj.optJSONArray("tracks")
                        if (arr != null) {
                            for (i in 0 until arr.length()) {
                                arr.optJSONObject(i)?.let { ConnectTrackSnapshot.fromJsonObject(it) }?.let(::add)
                            }
                        }
                    }
                    val posMs = if (obj.has("position_ms")) {
                        obj.optLong("position_ms", 0L).coerceAtLeast(0L)
                    } else {
                        (obj.optDouble("position_secs", 0.0) * 1000.0).roundToLong().coerceAtLeast(0L)
                    }
                    TransferQueue(
                        tracks = tracks,
                        startIndex = obj.optInt("start_index", 0).coerceAtLeast(0),
                        positionMs = posMs,
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
    private val localDeviceIdProvider: () -> String,
    private val httpClient: OkHttpClient = defaultHttpClient(),
) {
    constructor(
        localDeviceId: String,
        httpClient: OkHttpClient = defaultHttpClient(),
    ) : this(
        localDeviceIdProvider = { localDeviceId },
        httpClient = httpClient,
    )
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
            } catch (ce: CancellationException) {
                throw ce
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
        if (beacon.deviceId == localDeviceIdProvider()) return null
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
        runSuspendCatching {
            val url = "${normalizeBaseUrl(baseUrl)}/connect/info"
            val request = Request.Builder().url(url).get().build()
            httpClient.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    throw ConnectClientException(ConnectError.CONNECTION_FAILED, response.code)
                }
                ConnectDeviceInfo.fromJson(body)
                    ?: throw ConnectClientException(ConnectError.CONNECTION_FAILED, response.code)
            }
        }
    }

    suspend fun requestPairApproval(
        baseUrl: String,
        clientDeviceName: String,
    ): Result<PairApprovalInitAck> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            val payload = PairApprovalRequest(
                clientDeviceId = localDeviceIdProvider(),
                clientDeviceName = clientDeviceName.trim().ifEmpty { DEFAULT_DEVICE_NAME },
            ).toJson()
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/pair/request")
                .post(payload.toRequestBody(JSON_MEDIA_TYPE))
                .build()
            httpClient.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    val err = if (response.code == 401 || response.code == 403) {
                        ConnectError.PAIRING_REJECTED
                    } else {
                        ConnectError.PAIRING_FAILED
                    }
                    throw ConnectClientException(err, response.code)
                }
                PairApprovalInitAck.fromJson(body)
                    ?: throw ConnectClientException(ConnectError.PAIRING_FAILED, response.code)
            }
        }
    }

    suspend fun pollPairStatus(
        baseUrl: String,
        requestId: String,
    ): Result<PairStatusAck> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            val encodedId = URLEncoder.encode(requestId.trim(), StandardCharsets.UTF_8.name())
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/pair/status?request_id=$encodedId")
                .get()
                .build()
            httpClient.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (response.code == 404 || response.code == 410) {
                    return@use PairStatusAck(
                        status = PairApprovalStatus.REJECTED,
                        accepted = false,
                        deviceId = "",
                        deviceName = "",
                    )
                }
                if (!response.isSuccessful) {
                    val err = if (response.code == 401 || response.code == 403) {
                        ConnectError.PAIRING_REJECTED
                    } else {
                        ConnectError.PAIRING_FAILED
                    }
                    throw ConnectClientException(err, response.code)
                }
                PairStatusAck.fromJson(body)
                    ?: throw ConnectClientException(ConnectError.PAIRING_FAILED, response.code)
            }
        }
    }

    suspend fun checkTrust(
        baseUrl: String,
        sharedSecret: String,
    ): Result<Boolean> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            if (sharedSecret.isBlank()) {
                throw ConnectClientException(ConnectError.DEVICE_NOT_PAIRED)
            }
            val timestamp = System.currentTimeMillis().toString()
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/trust")
                .header(HEADER_DEVICE_ID, localDeviceIdProvider())
                .header(HEADER_TIMESTAMP, timestamp)
                .header(HEADER_SIGNATURE, SonataConnectCrypto.signPayload(sharedSecret, timestamp))
                .get()
                .build()
            httpClient.newCall(request).execute().use { response ->
                when (response.code) {
                    401 -> false
                    404 -> true
                    in 200..299 -> {
                        val body = response.body?.string().orEmpty()
                        runCatching { JSONObject(body).optBoolean("trusted", true) }.getOrDefault(true)
                    }
                    else -> throw ConnectClientException(ConnectError.CONNECTION_FAILED, response.code)
                }
            }
        }
    }

    suspend fun pair(
        baseUrl: String,
        clientDeviceName: String,
        pin: String,
    ): Result<PairAck> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            val cleanPin = pin.trim()
            if (!SonataConnectCrypto.isValidPin(cleanPin)) {
                throw ConnectClientException(ConnectError.INVALID_PIN)
            }
            val payload = PairRequest(
                clientDeviceId = localDeviceIdProvider(),
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
                    val err = if (response.code == 401 || response.code == 403) {
                        ConnectError.PAIRING_REJECTED
                    } else {
                        ConnectError.PAIRING_FAILED
                    }
                    throw ConnectClientException(err, response.code)
                }
                parsed ?: throw ConnectClientException(ConnectError.PAIRING_FAILED, response.code)
            }
        }
    }

    suspend fun fetchState(
        baseUrl: String,
        sharedSecret: String? = null,
    ): Result<ConnectStateSnapshot> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            val url = "${normalizeBaseUrl(baseUrl)}/connect/state"
            val builder = Request.Builder()
                .url(url)
                .header(HEADER_DEVICE_ID, localDeviceIdProvider())
                .get()
            if (!sharedSecret.isNullOrBlank()) {
                val timestamp = System.currentTimeMillis().toString()
                builder.header(HEADER_TIMESTAMP, timestamp)
                builder.header(HEADER_SIGNATURE, SonataConnectCrypto.signPayload(sharedSecret, timestamp))
            }
            httpClient.newCall(builder.build()).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (response.code == 401) {
                    throw ConnectClientException(ConnectError.TRUST_REVOKED, 401)
                }
                if (!response.isSuccessful) {
                    throw ConnectClientException(ConnectError.CONNECTION_FAILED, response.code)
                }
                ConnectStateSnapshot.fromJson(body)
                    ?: throw ConnectClientException(ConnectError.CONNECTION_FAILED, response.code)
            }
        }
    }

    suspend fun sendCommand(
        baseUrl: String,
        sharedSecret: String,
        command: ConnectCommand,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        runSuspendCatching {
            if (sharedSecret.isBlank()) {
                throw ConnectClientException(ConnectError.DEVICE_NOT_PAIRED)
            }
            val bodyJson = command.toJson()
            val signature = SonataConnectCrypto.signPayload(sharedSecret, bodyJson)
            val timestamp = System.currentTimeMillis().toString()
            val request = Request.Builder()
                .url("${normalizeBaseUrl(baseUrl)}/connect/command")
                .header(HEADER_DEVICE_ID, localDeviceIdProvider())
                .header(HEADER_TIMESTAMP, timestamp)
                .header(HEADER_SIGNATURE, signature)
                .post(bodyJson.toRequestBody(JSON_MEDIA_TYPE))
                .build()
            httpClient.newCall(request).execute().use { response ->
                if (response.code == 401) {
                    throw ConnectClientException(ConnectError.TRUST_REVOKED, 401)
                }
                if (!response.isSuccessful) {
                    throw ConnectClientException(ConnectError.COMMAND_FAILED, response.code)
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
