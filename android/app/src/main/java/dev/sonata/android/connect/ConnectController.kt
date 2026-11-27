package dev.sonata.android.connect

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.Immutable
import dev.sonata.android.data.AndroidKeyStoreSecretCipher
import dev.sonata.android.data.SecretCipher
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.sonata_android.AndroidTrack

/**
 * Secret storage abstraction for Sonata Connect trusted device HMAC tokens and display names.
 * Kept isolated from `SonataPreferences.exportBackupJson()` so tokens are never backed up.
 */
interface ConnectSecretStore {
    fun getSecret(deviceId: String): String?
    fun saveSecret(deviceId: String, secret: String)
    fun removeSecret(deviceId: String)
    fun pairedDeviceIds(): Set<String>

    fun getDeviceName(deviceId: String): String? = null
    fun saveDeviceName(deviceId: String, deviceName: String) = Unit
    fun trustedDevices(): Map<String, String> =
        pairedDeviceIds().associateWith { getDeviceName(it).orEmpty() }
}

class InMemoryConnectSecretStore : ConnectSecretStore {
    private val secrets = LinkedHashMap<String, String>()
    private val names = LinkedHashMap<String, String>()

    @Synchronized
    override fun getSecret(deviceId: String): String? = secrets[deviceId]

    @Synchronized
    override fun saveSecret(deviceId: String, secret: String) {
        if (deviceId.isNotBlank() && secret.isNotBlank()) {
            secrets[deviceId] = secret
        }
    }

    @Synchronized
    override fun removeSecret(deviceId: String) {
        secrets.remove(deviceId)
        names.remove(deviceId)
    }

    @Synchronized
    override fun pairedDeviceIds(): Set<String> = secrets.keys.toSet()

    @Synchronized
    override fun getDeviceName(deviceId: String): String? = names[deviceId]

    @Synchronized
    override fun saveDeviceName(deviceId: String, deviceName: String) {
        val cleanId = deviceId.trim()
        val cleanName = deviceName.trim()
        if (cleanId.isNotEmpty() && cleanName.isNotEmpty()) {
            names[cleanId] = cleanName
        }
    }

    @Synchronized
    override fun trustedDevices(): Map<String, String> =
        secrets.keys.associateWith { names[it].orEmpty() }
}

class SharedPrefsConnectSecretStore(
    context: Context,
    private val secretCipher: SecretCipher = AndroidKeyStoreSecretCipher,
) : ConnectSecretStore {
    private val prefs: SharedPreferences by lazy {
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    @Synchronized
    override fun getSecret(deviceId: String): String? {
        val cleanId = deviceId.trim()
        if (cleanId.isEmpty()) return null
        val key = keyFor(cleanId)
        val stored = prefs.getString(key, null)?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val decrypted = secretCipher.decrypt(stored)?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        if (!secretCipher.isEncrypted(stored)) {
            prefs.edit().putString(key, secretCipher.encrypt(decrypted)).apply()
        }
        return decrypted
    }

    @Synchronized
    override fun saveSecret(deviceId: String, secret: String) {
        val cleanId = deviceId.trim()
        val cleanSecret = secret.trim()
        if (cleanId.isEmpty() || cleanSecret.isEmpty()) return
        val encrypted = secretCipher.encrypt(cleanSecret)
        prefs.edit().putString(keyFor(cleanId), encrypted).apply()
    }

    @Synchronized
    override fun removeSecret(deviceId: String) {
        val cleanId = deviceId.trim()
        prefs.edit()
            .remove(keyFor(cleanId))
            .remove(nameKeyFor(cleanId))
            .apply()
    }

    @Synchronized
    override fun pairedDeviceIds(): Set<String> = prefs.all.keys
        .filter { it.startsWith(KEY_PREFIX) }
        .map { it.removePrefix(KEY_PREFIX) }
        .filter { it.isNotBlank() }
        .toSet()

    @Synchronized
    override fun getDeviceName(deviceId: String): String? =
        prefs.getString(nameKeyFor(deviceId.trim()), null)?.trim()?.takeIf { it.isNotEmpty() }

    @Synchronized
    override fun saveDeviceName(deviceId: String, deviceName: String) {
        val cleanId = deviceId.trim()
        val cleanName = deviceName.trim()
        if (cleanId.isEmpty() || cleanName.isEmpty()) return
        prefs.edit().putString(nameKeyFor(cleanId), cleanName).apply()
    }

    @Synchronized
    override fun trustedDevices(): Map<String, String> =
        pairedDeviceIds().associateWith { getDeviceName(it).orEmpty() }

    companion object {
        private const val PREFS_NAME = "sonata_connect_secrets"
        private const val KEY_PREFIX = "secret_"
        private const val NAME_PREFIX = "name_"
        private fun keyFor(deviceId: String): String = "$KEY_PREFIX$deviceId"
        private fun nameKeyFor(deviceId: String): String = "$NAME_PREFIX$deviceId"
    }
}

@Immutable
data class PendingPairState(
    val beacon: ConnectBeacon,
    val requestId: String? = null,
)

@Immutable
data class ConnectUiState(
    val enabled: Boolean = false,
    val deviceName: String = SonataConnectClient.DEFAULT_DEVICE_NAME,
    val scanning: Boolean = false,
    val discoveredDevices: List<ConnectBeacon> = emptyList(),
    val pairedDeviceIds: Set<String> = emptySet(),
    val trustedDeviceNames: Map<String, String> = emptyMap(),
    val activePeer: ConnectBeacon? = null,
    val remoteState: ConnectStateSnapshot? = null,
    val pairingInProgress: Boolean = false,
    val pendingPair: PendingPairState? = null,
    val pairingError: ConnectError? = null,
    val transferInProgress: Boolean = false,
)

/**
 * Coordinates Sonata Connect passive LAN discovery, interactive approval pairing, legacy 6-digit
 * PIN fallback pairing, trusted-device recognition (strictly without auto-connecting), periodic
 * remote state polling, transport control commands, and `TransferQueue` handoffs.
 */
class ConnectController(
    private val client: SonataConnectClient,
    private val secretStore: ConnectSecretStore,
    private val scope: CoroutineScope,
    private val deviceNameProvider: () -> String,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val discoveryPort: Int = CONNECT_DISCOVERY_PORT,
    private val pairPollIntervalMs: Long = PAIR_POLL_INTERVAL_MS,
    private val pairTimeoutMs: Long = PAIR_TIMEOUT_MS,
    private val statePollIntervalMs: Long = STATE_POLL_INTERVAL_MS,
) {
    private val _state = MutableStateFlow(
        ConnectUiState(
            deviceName = deviceNameProvider(),
        ),
    )
    val state: StateFlow<ConnectUiState> = _state.asStateFlow()

    private var discoveryObserverJob: Job? = null
    private var statePollJob: Job? = null
    private var pairingJob: Job? = null

    init {
        scope.launch(ioDispatcher) {
            val paired = secretStore.pairedDeviceIds()
            val names = paired.associateWith { secretStore.getDeviceName(it).orEmpty() }
            _state.update {
                it.copy(
                    pairedDeviceIds = paired,
                    trustedDeviceNames = names,
                )
            }
        }
        discoveryObserverJob = scope.launch {
            client.discoveredDevices.collect { devices ->
                // Passive recognition only: NEVER auto-connect when a trusted host appears on LAN.
                val pairedIds = _state.value.pairedDeviceIds
                var namesUpdated = false
                for (beacon in devices) {
                    if (beacon.deviceId in pairedIds &&
                        secretStore.getDeviceName(beacon.deviceId) != beacon.deviceName
                    ) {
                        secretStore.saveDeviceName(beacon.deviceId, beacon.deviceName)
                        namesUpdated = true
                    }
                }
                val updatedNames = if (namesUpdated) {
                    pairedIds.associateWith { secretStore.getDeviceName(it).orEmpty() }
                } else {
                    null
                }
                _state.update { current ->
                    val refreshedActive = current.activePeer?.let { active ->
                        devices.firstOrNull { it.deviceId == active.deviceId } ?: active
                    }
                    current.copy(
                        discoveredDevices = devices,
                        activePeer = refreshedActive,
                        trustedDeviceNames = updatedNames ?: current.trustedDeviceNames,
                    )
                }
            }
        }
    }

    fun setEnabled(enabled: Boolean) {
        if (enabled) {
            _state.update {
                it.copy(
                    enabled = true,
                    scanning = true,
                    deviceName = deviceNameProvider(),
                    pairingError = null,
                )
            }
            scope.launch(ioDispatcher) {
                val paired = secretStore.pairedDeviceIds()
                val names = paired.associateWith { secretStore.getDeviceName(it).orEmpty() }
                _state.update {
                    it.copy(
                        pairedDeviceIds = paired,
                        trustedDeviceNames = names,
                    )
                }
            }
            client.startDiscovery(scope, discoveryPort)
        } else {
            cancelPairing()
            disconnect()
            client.stopDiscovery()
            _state.update {
                it.copy(
                    enabled = false,
                    scanning = false,
                    pairingError = null,
                )
            }
        }
    }

    fun updateDeviceName(name: String) {
        val clean = name.trim().ifEmpty { SonataConnectClient.DEFAULT_DEVICE_NAME }
        _state.update { it.copy(deviceName = clean) }
    }

    fun ensureDiscoveryRunning() {
        if (!_state.value.enabled) return
        client.startDiscovery(scope, discoveryPort)
        _state.update { it.copy(scanning = true) }
        scope.launch(ioDispatcher) {
            val paired = secretStore.pairedDeviceIds()
            val names = paired.associateWith { secretStore.getDeviceName(it).orEmpty() }
            _state.update {
                it.copy(
                    pairedDeviceIds = paired,
                    trustedDeviceNames = names,
                )
            }
        }
    }

    fun refreshDiscovery() {
        if (!_state.value.enabled) {
            setEnabled(true)
            return
        }
        client.stopDiscovery()
        client.startDiscovery(scope, discoveryPort)
        _state.update { it.copy(scanning = true) }
        scope.launch(ioDispatcher) {
            val paired = secretStore.pairedDeviceIds()
            val names = paired.associateWith { secretStore.getDeviceName(it).orEmpty() }
            _state.update {
                it.copy(
                    pairedDeviceIds = paired,
                    trustedDeviceNames = names,
                )
            }
        }
    }

    fun clearPairingError() {
        _state.update { it.copy(pairingError = null) }
    }

    fun startInteractivePairing(beacon: ConnectBeacon) {
        pairingJob?.cancel()
        pairingJob = scope.launch(ioDispatcher) {
            requestPairApproval(beacon)
        }
    }

    fun cancelPairing() {
        pairingJob?.cancel()
        pairingJob = null
        _state.update {
            it.copy(
                pairingInProgress = false,
                pendingPair = null,
                pairingError = null,
            )
        }
    }

    /**
     * Initiates interactive Desktop/TUI approval pairing (`POST /connect/pair/request`) and polls
     * `GET /connect/pair/status?request_id=<id>` until approved, rejected, cancelled, or timed out.
     *
     * On approval, persists the device into Trusted Devices ([ConnectSecretStore]) WITHOUT
     * auto-connecting or taking over playback until the user explicitly selects the device.
     */
    suspend fun requestPairApproval(beacon: ConnectBeacon): Boolean {
        _state.update {
            it.copy(
                pairingInProgress = true,
                pendingPair = PendingPairState(beacon = beacon),
                pairingError = null,
            )
        }
        try {
            val initResult = client.requestPairApproval(
                baseUrl = beacon.baseUrl(),
                clientDeviceName = deviceNameProvider(),
            )
            val initAck = initResult.getOrElse { err ->
                _state.update {
                    it.copy(
                        pairingInProgress = false,
                        pendingPair = null,
                        pairingError = err.toConnectError(ConnectError.PAIRING_FAILED),
                    )
                }
                return false
            }
            _state.update {
                it.copy(
                    pendingPair = PendingPairState(beacon = beacon, requestId = initAck.requestId),
                )
            }

            val maxAttempts = (pairTimeoutMs / pairPollIntervalMs.coerceAtLeast(1L))
                .coerceAtLeast(1L)
                .toInt()
            for (attempt in 0 until maxAttempts) {
                delay(pairPollIntervalMs)
                val pollResult = client.pollPairStatus(
                    baseUrl = beacon.baseUrl(),
                    requestId = initAck.requestId,
                )
                val statusAck = pollResult.getOrElse { err ->
                    _state.update {
                        it.copy(
                            pairingInProgress = false,
                            pendingPair = null,
                            pairingError = err.toConnectError(ConnectError.PAIRING_FAILED),
                        )
                    }
                    return false
                }
                when (statusAck.status) {
                    PairApprovalStatus.PENDING -> continue
                    PairApprovalStatus.APPROVED -> {
                        val secret = statusAck.sharedSecret?.trim().orEmpty()
                        if (!statusAck.accepted || secret.isEmpty()) {
                            _state.update {
                                it.copy(
                                    pairingInProgress = false,
                                    pendingPair = null,
                                    pairingError = ConnectError.PAIRING_FAILED,
                                )
                            }
                            return false
                        }
                        val hostName = statusAck.deviceName.ifBlank { beacon.deviceName }
                        secretStore.saveSecret(beacon.deviceId, secret)
                        secretStore.saveDeviceName(beacon.deviceId, hostName)
                        _state.update {
                            it.copy(
                                pairingInProgress = false,
                                pendingPair = null,
                                pairingError = null,
                                pairedDeviceIds = secretStore.pairedDeviceIds(),
                                trustedDeviceNames = secretStore.trustedDevices(),
                            )
                        }
                        return true
                    }
                    PairApprovalStatus.REJECTED -> {
                        _state.update {
                            it.copy(
                                pairingInProgress = false,
                                pendingPair = null,
                                pairingError = ConnectError.PAIRING_REJECTED,
                            )
                        }
                        return false
                    }
                }
            }

            _state.update {
                it.copy(
                    pairingInProgress = false,
                    pendingPair = null,
                    pairingError = ConnectError.PAIRING_TIMEOUT,
                )
            }
            return false
        } catch (ce: CancellationException) {
            _state.update {
                it.copy(
                    pairingInProgress = false,
                    pendingPair = null,
                )
            }
            throw ce
        }
    }

    /**
     * Legacy 6-digit PIN fallback pairing (`POST /connect/pair`), saving trust and connecting.
     */
    suspend fun pairAndConnect(beacon: ConnectBeacon, pin: String): Boolean {
        val cleanPin = pin.trim()
        if (!SonataConnectCrypto.isValidPin(cleanPin)) {
            _state.update { it.copy(pairingError = ConnectError.INVALID_PIN) }
            return false
        }
        _state.update { it.copy(pairingInProgress = true, pendingPair = null, pairingError = null) }
        val result = client.pair(
            baseUrl = beacon.baseUrl(),
            clientDeviceName = deviceNameProvider(),
            pin = cleanPin,
        )
        return result.fold(
            onSuccess = { ack ->
                val secret = ack.sharedSecret?.trim().orEmpty()
                if (!ack.accepted || secret.isEmpty()) {
                    _state.update {
                        it.copy(
                            pairingInProgress = false,
                            pairingError = ConnectError.PAIRING_REJECTED,
                        )
                    }
                    false
                } else {
                    val hostName = ack.deviceName.ifBlank { beacon.deviceName }
                    secretStore.saveSecret(beacon.deviceId, secret)
                    secretStore.saveDeviceName(beacon.deviceId, hostName)
                    val remote = client.fetchState(beacon.baseUrl(), secret).getOrNull()
                    _state.update {
                        it.copy(
                            pairingInProgress = false,
                            pairingError = null,
                            pairedDeviceIds = secretStore.pairedDeviceIds(),
                            trustedDeviceNames = secretStore.trustedDevices(),
                            activePeer = beacon,
                            remoteState = remote,
                        )
                    }
                    startPollingPeer(beacon, secret)
                    true
                }
            },
            onFailure = { err ->
                _state.update {
                    it.copy(
                        pairingInProgress = false,
                        pairingError = err.toConnectError(ConnectError.PAIRING_FAILED),
                    )
                }
                false
            },
        )
    }

    suspend fun connectToPaired(beacon: ConnectBeacon): Boolean {
        val secret = secretStore.getSecret(beacon.deviceId)
        if (secret == null) {
            _state.update { it.copy(pairingError = ConnectError.DEVICE_NOT_PAIRED) }
            return false
        }
        val trustResult = client.checkTrust(beacon.baseUrl(), secret)
        val trusted = trustResult.getOrElse { err ->
            _state.update {
                it.copy(pairingError = err.toConnectError(ConnectError.CONNECTION_FAILED))
            }
            return false
        }
        if (!trusted) {
            revokeLocalTrust(beacon.deviceId)
            return false
        }
        val stateResult = client.fetchState(beacon.baseUrl(), secret)
        return stateResult.fold(
            onSuccess = { snapshot ->
                secretStore.saveDeviceName(beacon.deviceId, beacon.deviceName)
                _state.update {
                    it.copy(
                        activePeer = beacon,
                        remoteState = snapshot,
                        pairingError = null,
                        trustedDeviceNames = secretStore.trustedDevices(),
                    )
                }
                startPollingPeer(beacon, secret)
                true
            },
            onFailure = { err ->
                val connectErr = err.toConnectError(ConnectError.CONNECTION_FAILED)
                if (connectErr == ConnectError.TRUST_REVOKED) {
                    revokeLocalTrust(beacon.deviceId)
                } else {
                    _state.update { it.copy(pairingError = connectErr) }
                }
                false
            },
        )
    }

    fun disconnect() {
        statePollJob?.cancel()
        statePollJob = null
        _state.update {
            it.copy(
                activePeer = null,
                remoteState = null,
            )
        }
    }

    fun forgetDevice(deviceId: String) {
        if (_state.value.activePeer?.deviceId == deviceId) {
            disconnect()
        }
        secretStore.removeSecret(deviceId)
        _state.update {
            it.copy(
                pairedDeviceIds = secretStore.pairedDeviceIds(),
                trustedDeviceNames = secretStore.trustedDevices(),
            )
        }
    }

    suspend fun sendCommand(command: ConnectCommand): Result<Unit> {
        val peer = _state.value.activePeer
            ?: return Result.failure(ConnectClientException(ConnectError.NO_ACTIVE_DEVICE))
        val secret = secretStore.getSecret(peer.deviceId)
            ?: return Result.failure(ConnectClientException(ConnectError.DEVICE_NOT_PAIRED))
        val result = client.sendCommand(peer.baseUrl(), secret, command)
        if (result.isFailure) {
            val err = result.exceptionOrNull().toConnectError(ConnectError.COMMAND_FAILED)
            if (err == ConnectError.TRUST_REVOKED) {
                revokeLocalTrust(peer.deviceId)
            }
            return Result.failure(ConnectClientException(err, cause = result.exceptionOrNull()))
        }
        client.fetchState(peer.baseUrl(), secret)
            .onSuccess { snapshot ->
                _state.update { current ->
                    if (current.activePeer?.deviceId == peer.deviceId) {
                        current.copy(remoteState = snapshot)
                    } else {
                        current
                    }
                }
            }
            .onFailure { err ->
                if (err.toConnectError(ConnectError.CONNECTION_FAILED) == ConnectError.TRUST_REVOKED) {
                    revokeLocalTrust(peer.deviceId)
                }
            }
        return result
    }

    suspend fun transferQueue(
        tracks: List<AndroidTrack>,
        startIndex: Int,
        positionMs: Long,
        playWhenReady: Boolean,
    ): Result<Unit> {
        if (tracks.isEmpty()) {
            return Result.failure(ConnectClientException(ConnectError.QUEUE_EMPTY))
        }
        _state.update { it.copy(transferInProgress = true) }
        val snapshots = tracks.map { track ->
            ConnectTrackSnapshot(
                id = track.id,
                title = track.title,
                artist = track.artist,
                album = track.album,
                durationSecs = track.durationSecs?.toLong() ?: 0L,
                thumbnailUrl = track.thumbnailUrl,
            )
        }
        val clampedIndex = startIndex.coerceIn(0, (snapshots.size - 1).coerceAtLeast(0))
        val cmd = ConnectCommand.TransferQueue(
            tracks = snapshots,
            startIndex = clampedIndex,
            positionMs = positionMs.coerceAtLeast(0L),
            playWhenReady = playWhenReady,
        )
        val result = sendCommand(cmd)
        _state.update { it.copy(transferInProgress = false) }
        return result
    }

    private fun revokeLocalTrust(deviceId: String) {
        if (_state.value.activePeer?.deviceId == deviceId) {
            statePollJob?.cancel()
            statePollJob = null
        }
        secretStore.removeSecret(deviceId)
        _state.update { current ->
            val clearActive = current.activePeer?.deviceId == deviceId
            current.copy(
                activePeer = if (clearActive) null else current.activePeer,
                remoteState = if (clearActive) null else current.remoteState,
                pairedDeviceIds = secretStore.pairedDeviceIds(),
                trustedDeviceNames = secretStore.trustedDevices(),
                pairingError = ConnectError.TRUST_REVOKED,
            )
        }
    }

    private fun startPollingPeer(beacon: ConnectBeacon, secret: String) {
        statePollJob?.cancel()
        statePollJob = scope.launch(ioDispatcher) {
            while (isActive && _state.value.activePeer?.deviceId == beacon.deviceId) {
                delay(statePollIntervalMs)
                if (_state.value.activePeer?.deviceId != beacon.deviceId) break
                val poll = client.fetchState(beacon.baseUrl(), secret)
                poll.onSuccess { snapshot ->
                    _state.update { current ->
                        if (current.activePeer?.deviceId == beacon.deviceId) {
                            current.copy(remoteState = snapshot)
                        } else {
                            current
                        }
                    }
                }.onFailure { err ->
                    if (err.toConnectError(ConnectError.CONNECTION_FAILED) == ConnectError.TRUST_REVOKED) {
                        revokeLocalTrust(beacon.deviceId)
                        return@launch
                    }
                }
            }
        }
    }

    fun close() {
        pairingJob?.cancel()
        pairingJob = null
        statePollJob?.cancel()
        statePollJob = null
        discoveryObserverJob?.cancel()
        discoveryObserverJob = null
        client.stopDiscovery()
    }

    companion object {
        private const val STATE_POLL_INTERVAL_MS = 1_500L
        private const val PAIR_POLL_INTERVAL_MS = 600L
        private const val PAIR_TIMEOUT_MS = 60_000L
    }
}

private fun Throwable?.toConnectError(fallback: ConnectError): ConnectError =
    (this as? ConnectClientException)?.error ?: fallback

/**
 * Pure playback routing coordinator between local Android ExoPlayer ("This Phone") and an active
 * Sonata Connect remote host. All transport and track/queue actions route to the active host until
 * the user disconnects or switches back to "This Phone".
 */
class ConnectPlaybackRouter(
    private val controller: ConnectController,
) {
    val isRemoteActive: Boolean
        get() = controller.state.value.activePeer != null

    val activeDeviceName: String?
        get() = controller.state.value.activePeer?.deviceName

    suspend fun connectToDevice(
        beacon: ConnectBeacon,
        onPauseLocal: () -> Unit,
    ): Boolean {
        val connected = controller.connectToPaired(beacon)
        if (connected) {
            onPauseLocal()
        }
        return connected
    }

    fun disconnectToLocal(onRestoreLocal: () -> Unit = {}) {
        controller.disconnect()
        onRestoreLocal()
    }

    suspend fun playTrack(
        track: AndroidTrack,
        queue: List<AndroidTrack>,
        onLocalPlay: (AndroidTrack, List<AndroidTrack>) -> Unit,
    ): Boolean {
        if (!isRemoteActive) {
            onLocalPlay(track, queue)
            return false
        }
        val effectiveQueue = if (queue.any { it.id == track.id }) queue else listOf(track) + queue
        val startIndex = effectiveQueue.indexOfFirst { it.id == track.id }.coerceAtLeast(0)
        controller.transferQueue(
            tracks = effectiveQueue,
            startIndex = startIndex,
            positionMs = 0L,
            playWhenReady = true,
        )
        return true
    }

    suspend fun togglePlayPause(
        fallbackIsPlaying: Boolean,
        onLocalToggle: () -> Unit,
    ): Boolean {
        if (!isRemoteActive) {
            onLocalToggle()
            return false
        }
        val playing = controller.state.value.remoteState?.isPlaying ?: fallbackIsPlaying
        controller.sendCommand(if (playing) ConnectCommand.Pause else ConnectCommand.Play)
        return true
    }

    suspend fun nextTrack(onLocalNext: () -> Unit): Boolean {
        if (!isRemoteActive) {
            onLocalNext()
            return false
        }
        controller.sendCommand(ConnectCommand.Next)
        return true
    }

    suspend fun prevTrack(onLocalPrev: () -> Unit): Boolean {
        if (!isRemoteActive) {
            onLocalPrev()
            return false
        }
        controller.sendCommand(ConnectCommand.Prev)
        return true
    }

    suspend fun seekTo(
        positionMs: Long,
        onLocalSeek: (Long) -> Unit,
    ): Boolean {
        val clamped = positionMs.coerceAtLeast(0L)
        if (!isRemoteActive) {
            onLocalSeek(clamped)
            return false
        }
        controller.sendCommand(ConnectCommand.SeekAbsolute(clamped))
        return true
    }

    suspend fun setVolume(volume: Float): Result<Unit> =
        controller.sendCommand(ConnectCommand.SetVolume(volume.coerceIn(0f, 1f)))

    suspend fun syncUpdatedQueue(
        tracks: List<AndroidTrack>,
        currentIndex: Int,
        positionMs: Long,
        playWhenReady: Boolean,
    ): Boolean {
        if (!isRemoteActive || tracks.isEmpty()) return false
        val clampedIndex = currentIndex.coerceIn(0, tracks.lastIndex)
        controller.transferQueue(
            tracks = tracks,
            startIndex = clampedIndex,
            positionMs = positionMs.coerceAtLeast(0L),
            playWhenReady = playWhenReady,
        )
        return true
    }
}
