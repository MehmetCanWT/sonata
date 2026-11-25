package dev.sonata.android.connect

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.Immutable
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
 * Secret storage abstraction for Sonata Connect paired device HMAC tokens.
 * Kept isolated from `SonataPreferences.exportBackupJson()` so tokens are never backed up.
 */
interface ConnectSecretStore {
    fun getSecret(deviceId: String): String?
    fun saveSecret(deviceId: String, secret: String)
    fun removeSecret(deviceId: String)
    fun pairedDeviceIds(): Set<String>
}

class InMemoryConnectSecretStore : ConnectSecretStore {
    private val secrets = LinkedHashMap<String, String>()

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
    }

    @Synchronized
    override fun pairedDeviceIds(): Set<String> = secrets.keys.toSet()
}

class SharedPrefsConnectSecretStore(context: Context) : ConnectSecretStore {
    private val prefs: SharedPreferences by lazy {
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    @Synchronized
    override fun getSecret(deviceId: String): String? =
        prefs.getString(keyFor(deviceId), null)?.trim()?.takeIf { it.isNotEmpty() }

    @Synchronized
    override fun saveSecret(deviceId: String, secret: String) {
        val cleanId = deviceId.trim()
        val cleanSecret = secret.trim()
        if (cleanId.isEmpty() || cleanSecret.isEmpty()) return
        prefs.edit().putString(keyFor(cleanId), cleanSecret).apply()
    }

    @Synchronized
    override fun removeSecret(deviceId: String) {
        prefs.edit().remove(keyFor(deviceId.trim())).apply()
    }

    @Synchronized
    override fun pairedDeviceIds(): Set<String> = prefs.all.keys
        .filter { it.startsWith(KEY_PREFIX) }
        .map { it.removePrefix(KEY_PREFIX) }
        .filter { it.isNotBlank() }
        .toSet()

    companion object {
        private const val PREFS_NAME = "sonata_connect_secrets"
        private const val KEY_PREFIX = "secret_"
        private fun keyFor(deviceId: String): String = "$KEY_PREFIX$deviceId"
    }
}

@Immutable
data class ConnectUiState(
    val enabled: Boolean = false,
    val deviceName: String = SonataConnectClient.DEFAULT_DEVICE_NAME,
    val scanning: Boolean = false,
    val discoveredDevices: List<ConnectBeacon> = emptyList(),
    val pairedDeviceIds: Set<String> = emptySet(),
    val activePeer: ConnectBeacon? = null,
    val remoteState: ConnectStateSnapshot? = null,
    val pairingInProgress: Boolean = false,
    val pairingError: String? = null,
    val statusMessage: String? = null,
    val transferInProgress: Boolean = false,
)

/**
 * Coordinates Sonata Connect LAN discovery, 6-digit PIN pairing, periodic remote state
 * polling, transport control commands, and `TransferQueue` handoffs.
 */
class ConnectController(
    private val client: SonataConnectClient,
    private val secretStore: ConnectSecretStore,
    private val scope: CoroutineScope,
    private val deviceNameProvider: () -> String,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val discoveryPort: Int = CONNECT_DISCOVERY_PORT,
) {
    private val _state = MutableStateFlow(
        ConnectUiState(
            deviceName = deviceNameProvider(),
        ),
    )
    val state: StateFlow<ConnectUiState> = _state.asStateFlow()

    private var discoveryObserverJob: Job? = null
    private var statePollJob: Job? = null

    init {
        scope.launch(ioDispatcher) {
            val paired = secretStore.pairedDeviceIds()
            _state.update { it.copy(pairedDeviceIds = paired) }
        }
        discoveryObserverJob = scope.launch {
            client.discoveredDevices.collect { devices ->
                _state.update { it.copy(discoveredDevices = devices) }
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
                _state.update { it.copy(pairedDeviceIds = paired) }
            }
            client.startDiscovery(scope, discoveryPort)
        } else {
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

    fun refreshDiscovery() {
        if (!_state.value.enabled) {
            setEnabled(true)
            return
        }
        client.stopDiscovery()
        client.startDiscovery(scope, discoveryPort)
        _state.update {
            it.copy(
                scanning = true,
            )
        }
        scope.launch(ioDispatcher) {
            val paired = secretStore.pairedDeviceIds()
            _state.update { it.copy(pairedDeviceIds = paired) }
        }
    }

    fun clearPairingError() {
        _state.update { it.copy(pairingError = null) }
    }

    suspend fun pairAndConnect(beacon: ConnectBeacon, pin: String): Boolean {
        val cleanPin = pin.trim()
        if (!SonataConnectCrypto.isValidPin(cleanPin)) {
            _state.update { it.copy(pairingError = "Invalid 6-digit PIN") }
            return false
        }
        _state.update { it.copy(pairingInProgress = true, pairingError = null) }
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
                            pairingError = ack.message ?: "Pairing rejected",
                        )
                    }
                    false
                } else {
                    secretStore.saveSecret(beacon.deviceId, secret)
                    val remote = client.fetchState(beacon.baseUrl(), secret).getOrNull()
                    _state.update {
                        it.copy(
                            pairingInProgress = false,
                            pairingError = null,
                            pairedDeviceIds = secretStore.pairedDeviceIds(),
                            activePeer = beacon,
                            remoteState = remote,
                            statusMessage = beacon.deviceName,
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
                        pairingError = err.message ?: "Pairing failed",
                    )
                }
                false
            },
        )
    }

    suspend fun connectToPaired(beacon: ConnectBeacon): Boolean {
        val secret = secretStore.getSecret(beacon.deviceId) ?: return false
        val stateResult = client.fetchState(beacon.baseUrl(), secret)
        return stateResult.fold(
            onSuccess = { snapshot ->
                _state.update {
                    it.copy(
                        activePeer = beacon,
                        remoteState = snapshot,
                        pairingError = null,
                        statusMessage = beacon.deviceName,
                    )
                }
                startPollingPeer(beacon, secret)
                true
            },
            onFailure = { err ->
                _state.update { it.copy(pairingError = err.message ?: "Connection failed") }
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
                statusMessage = null,
            )
        }
    }

    fun forgetDevice(deviceId: String) {
        if (_state.value.activePeer?.deviceId == deviceId) {
            disconnect()
        }
        secretStore.removeSecret(deviceId)
        _state.update { it.copy(pairedDeviceIds = secretStore.pairedDeviceIds()) }
    }

    suspend fun sendCommand(command: ConnectCommand): Result<Unit> {
        val peer = _state.value.activePeer
            ?: return Result.failure(IllegalStateException("No active Connect device"))
        val secret = secretStore.getSecret(peer.deviceId)
            ?: return Result.failure(IllegalStateException("Device is not paired"))
        val result = client.sendCommand(peer.baseUrl(), secret, command)
        if (result.isSuccess) {
            client.fetchState(peer.baseUrl(), secret).onSuccess { snapshot ->
                _state.update { it.copy(remoteState = snapshot) }
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
            return Result.failure(IllegalArgumentException("Queue is empty"))
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

    private fun startPollingPeer(beacon: ConnectBeacon, secret: String) {
        statePollJob?.cancel()
        statePollJob = scope.launch(ioDispatcher) {
            while (isActive && _state.value.activePeer?.deviceId == beacon.deviceId) {
                client.fetchState(beacon.baseUrl(), secret).onSuccess { snapshot ->
                    _state.update { current ->
                        if (current.activePeer?.deviceId == beacon.deviceId) {
                            current.copy(remoteState = snapshot)
                        } else {
                            current
                        }
                    }
                }
                delay(STATE_POLL_INTERVAL_MS)
            }
        }
    }

    fun close() {
        statePollJob?.cancel()
        statePollJob = null
        discoveryObserverJob?.cancel()
        discoveryObserverJob = null
        client.stopDiscovery()
    }

    companion object {
        private const val STATE_POLL_INTERVAL_MS = 2_000L
    }
}
