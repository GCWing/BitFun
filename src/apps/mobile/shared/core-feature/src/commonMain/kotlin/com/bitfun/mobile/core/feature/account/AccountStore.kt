package com.bitfun.mobile.core.feature.account

import com.bitfun.mobile.core.feature.CloudSettingsSource
import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.feature.pairing.asTransportLog
import com.bitfun.mobile.core.feature.session.RemoteSessionStore
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.bitfun.mobile.core.persistence.SecureStore
import com.bitfun.mobile.core.transport.AccountDeviceCommandTransport
import com.bitfun.mobile.core.transport.CloudAccountClient
import com.bitfun.mobile.core.transport.CloudAccountDevice
import com.bitfun.mobile.core.transport.CloudAccountException
import com.bitfun.mobile.core.transport.CloudAccountFailure
import com.bitfun.mobile.core.transport.CloudAccountSession
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import com.bitfun.mobile.core.transport.TransportLog
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlin.io.encoding.Base64

internal data class AccountSessionData(
    val relayUrl: String,
    val username: String,
    val token: String,
    val userId: String,
    val masterKey: ByteArray,
    val targetDeviceId: String?,
    val targetDeviceName: String?,
)

internal interface AccountBackend {
    suspend fun login(
        relayUrl: String,
        username: String,
        password: String,
        deviceId: String,
        deviceName: String,
    ): AccountSessionData

    /** [selfDeviceId] lets the transport drop this device's own row. */
    suspend fun listDevices(session: AccountSessionData, selfDeviceId: String): List<AccountDeviceUi>

    /** The account's settings document, or null when it has never synced one. */
    suspend fun fetchSettings(session: AccountSessionData): String?

    fun transport(session: AccountSessionData, targetDeviceId: String): RemoteCommandTransport
}

public class AccountStore internal constructor(
    private val scope: CoroutineScope,
    private val backend: AccountBackend,
    private val secureStore: SecureStore,
    private val deviceId: String,
    private val deviceName: String,
) {
    private val _state = MutableStateFlow<AccountUiState>(AccountUiState.Idle)
    public val state: StateFlow<AccountUiState> = _state.asStateFlow()
    private var session: AccountSessionData? = null
    private var work: Job? = null

    public fun dispatch(intent: AccountIntent) {
        when (intent) {
            AccountIntent.Restore -> restore()
            is AccountIntent.Login -> login(intent)
            is AccountIntent.SelectDevice -> selectDevice(intent.deviceId)
            AccountIntent.RefreshDevices -> refreshDevices()
            AccountIntent.Logout -> logout()
            AccountIntent.Stop -> stop()
        }
    }

    public fun createSessionStore(scope: CoroutineScope): RemoteSessionStore? {
        val current = session ?: return null
        val target = current.targetDeviceId?.takeIf(String::isNotBlank) ?: return null
        return RemoteSessionStore.create(scope, backend.transport(current, target))
    }

    public fun createWorkspaceStore(scope: CoroutineScope): RemoteWorkspaceStore? {
        val current = session ?: return null
        val target = current.targetDeviceId?.takeIf(String::isNotBlank) ?: return null
        return RemoteWorkspaceStore.create(scope, backend.transport(current, target))
    }

    /**
     * A handle another feature can use to read the account's settings document.
     *
     * Bound to the session that was current when it was asked for, so a handle
     * taken before a logout reads that session and not the next one — the caller
     * asks again after every sign-in change, and gets null while signed out.
     */
    public fun cloudSettingsSource(): CloudSettingsSource? {
        val current = session ?: return null
        return CloudSettingsSource { backend.fetchSettings(current) }
    }

    public fun stop() {
        work?.cancel()
        work = null
    }

    private fun restore() {
        work?.cancel()
        _state.value = AccountUiState.Restoring
        work = scope.launch {
            val restored = try {
                val stored = secureStore.read(SESSION_KEY)?.decodeToString()
                if (stored.isNullOrEmpty()) {
                    _state.value = AccountUiState.SignedOut
                    return@launch
                }
                decodeRecord(stored)
            } catch (_: Throwable) {
                secureStore.delete(SESSION_KEY)
                session = null
                _state.value = AccountUiState.Failed(AccountFailureReason.SECURE_STORAGE, true)
                return@launch
            }
            session = restored
            try {
                publishReady(restored, backend.listDevices(restored, deviceId))
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: CloudAccountException) {
                _state.value = AccountUiState.Failed(error.failure.toUiReason(), true)
            } catch (_: Throwable) {
                _state.value = AccountUiState.Failed(AccountFailureReason.NETWORK, true)
            }
        }
    }

    private fun login(intent: AccountIntent.Login) {
        work?.cancel()
        _state.value = AccountUiState.SigningIn
        work = scope.launch {
            try {
                val loggedIn = backend.login(
                    intent.relayUrl,
                    intent.username,
                    intent.password,
                    deviceId,
                    deviceName,
                )
                val devices = backend.listDevices(loggedIn, deviceId)
                // Never this device, even as a fallback: driving the phone from
                // the phone is what `canSelectAccountDevice` forbids, and a
                // target nothing can be asked of is worse than none at all.
                val preferred = AccountDevicePolicy.preferredTarget(devices, deviceId)
                val selected = loggedIn.copy(
                    targetDeviceId = preferred?.id,
                    targetDeviceName = preferred?.name,
                )
                secureStore.write(SESSION_KEY, encodeRecord(selected).encodeToByteArray())
                session = selected
                publishReady(selected, devices)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: CloudAccountException) {
                session = null
                _state.value = AccountUiState.Failed(error.failure.toUiReason(), true)
            } catch (_: Throwable) {
                session = null
                _state.value = AccountUiState.Failed(AccountFailureReason.SECURE_STORAGE, true)
            }
        }
    }

    private fun selectDevice(targetId: String) {
        val current = session ?: return
        val ready = _state.value as? AccountUiState.Ready ?: return
        // `ready.devices` is already filtered, so the id alone would nearly do —
        // but presence is not, and an offline row is a row the user can see.
        val selected = ready.devices.firstOrNull { it.id == targetId }
            ?.takeIf { AccountDevicePolicy.canSelect(it, deviceId) }
            ?: return
        val updated = current.copy(targetDeviceId = selected.id, targetDeviceName = selected.name)
        try {
            secureStore.write(SESSION_KEY, encodeRecord(updated).encodeToByteArray())
            session = updated
            _state.value = ready.copy(selectedDeviceId = selected.id, selectedDeviceName = selected.name)
        } catch (_: Throwable) {
            _state.value = AccountUiState.Failed(AccountFailureReason.SECURE_STORAGE, true)
        }
    }

    /**
     * Re-ask the relay who is online.
     *
     * The old list stays on screen while this runs, and survives a failure: the
     * user asked whether anything changed, and "I could not find out" has to
     * leave them no worse off than not asking.
     */
    private fun refreshDevices() {
        val current = session ?: return
        val ready = _state.value as? AccountUiState.Ready ?: return
        if (ready.refreshing) return
        work?.cancel()
        _state.value = ready.copy(refreshing = true, refreshFailure = null)
        work = scope.launch {
            try {
                publishReady(current, backend.listDevices(current, deviceId))
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: CloudAccountException) {
                _state.value = ready.copy(refreshing = false, refreshFailure = error.failure.toUiReason())
            } catch (_: Throwable) {
                _state.value = ready.copy(refreshing = false, refreshFailure = AccountFailureReason.NETWORK)
            }
        }
    }

    private fun logout() {
        work?.cancel()
        work = null
        try {
            secureStore.delete(SESSION_KEY)
            session = null
            _state.value = AccountUiState.SignedOut
        } catch (_: Throwable) {
            _state.value = AccountUiState.Failed(AccountFailureReason.SECURE_STORAGE, true)
        }
    }

    /**
     * The one place the relay's list becomes the list a screen renders, so the
     * filter cannot be forgotten by a caller — or applied twice with two
     * different answers on two platforms.
     */
    private fun publishReady(current: AccountSessionData, devices: List<AccountDeviceUi>) {
        _state.value = AccountUiState.Ready(
            userId = current.userId,
            username = current.username,
            devices = AccountDevicePolicy.controlTargets(devices, deviceId),
            selectedDeviceId = current.targetDeviceId,
            selectedDeviceName = current.targetDeviceName,
        )
    }

    public companion object {
        internal fun create(
            scope: CoroutineScope,
            backend: AccountBackend,
            secureStore: SecureStore,
            deviceId: String,
            deviceName: String,
        ): AccountStore = AccountStore(scope, backend, secureStore, deviceId, deviceName)

        internal fun backend(log: CoreLog): AccountBackend {
            val transportLog = log.asTransportLog()
            return CloudBackend(CloudAccountClient.create(transportLog), transportLog)
        }

        private const val SESSION_KEY = "cloud_account_session"
        private val JSON = Json { ignoreUnknownKeys = true }

        private fun encodeRecord(session: AccountSessionData): String = JSON.encodeToString(
            AccountSessionRecord(
                relayUrl = session.relayUrl,
                username = session.username,
                token = session.token,
                userId = session.userId,
                masterKey = Base64.Default.encode(session.masterKey),
                targetDeviceId = session.targetDeviceId,
                targetDeviceName = session.targetDeviceName,
            ),
        )

        private fun decodeRecord(value: String): AccountSessionData {
            val record = JSON.decodeFromString<AccountSessionRecord>(value)
            return AccountSessionData(
                record.relayUrl,
                record.username,
                record.token,
                record.userId,
                Base64.Default.decode(record.masterKey),
                record.targetDeviceId,
                record.targetDeviceName,
            )
        }
    }
}

private class CloudBackend(
    private val client: CloudAccountClient,
    private val log: TransportLog,
) : AccountBackend {
    override suspend fun login(
        relayUrl: String,
        username: String,
        password: String,
        deviceId: String,
        deviceName: String,
    ): AccountSessionData {
        val session = client.login(relayUrl, username, password, deviceId, deviceName)
        return AccountSessionData(
            relayUrl = relayUrl.trim().ifEmpty { com.bitfun.mobile.core.transport.DEFAULT_CLOUD_RELAY_URL },
            username = username.trim(),
            token = session.token,
            userId = session.userId,
            masterKey = session.masterKey,
            targetDeviceId = null,
            targetDeviceName = null,
        )
    }

    override suspend fun listDevices(session: AccountSessionData, selfDeviceId: String): List<AccountDeviceUi> =
        client.listDevices(session.relayUrl, session.toTransportSession(), selfDeviceId).map { it.toUi() }

    override suspend fun fetchSettings(session: AccountSessionData): String? =
        client.fetchSettings(session.relayUrl, session.toTransportSession())?.plaintext

    override fun transport(session: AccountSessionData, targetDeviceId: String): RemoteCommandTransport =
        AccountDeviceCommandTransport(client, session.relayUrl, session.toTransportSession(), targetDeviceId, log)

    private fun AccountSessionData.toTransportSession(): CloudAccountSession =
        CloudAccountSession(token, userId, masterKey)

    private fun CloudAccountDevice.toUi(): AccountDeviceUi = AccountDeviceUi(deviceId, deviceName, online, lastSeenAt)
}

@Serializable
private data class AccountSessionRecord(
    val relayUrl: String,
    val username: String,
    val token: String,
    val userId: String,
    val masterKey: String,
    val targetDeviceId: String?,
    val targetDeviceName: String?,
)

private fun CloudAccountFailure.toUiReason(): AccountFailureReason = when (this) {
    CloudAccountFailure.INVALID_CREDENTIALS -> AccountFailureReason.INVALID_CREDENTIALS
    CloudAccountFailure.AUTHENTICATION -> AccountFailureReason.AUTHENTICATION
    CloudAccountFailure.RATE_LIMITED -> AccountFailureReason.RATE_LIMITED
    CloudAccountFailure.RELAY_UNAVAILABLE -> AccountFailureReason.RELAY_UNAVAILABLE
    CloudAccountFailure.NETWORK -> AccountFailureReason.NETWORK
    CloudAccountFailure.TIMEOUT -> AccountFailureReason.TIMEOUT
    CloudAccountFailure.MALFORMED_RESPONSE -> AccountFailureReason.MALFORMED_RESPONSE
}
