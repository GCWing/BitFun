package com.bitfun.mobile.core.feature.account

import com.bitfun.mobile.core.feature.CloudSettingsSource
import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.feature.pairing.asTransportLog
import com.bitfun.mobile.core.feature.session.RemoteSessionStore
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.bitfun.mobile.core.persistence.MobilePersistenceStores
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
    private val persistence: MobilePersistenceStores? = null,
) {
    private val _state = MutableStateFlow<AccountUiState>(AccountUiState.Idle)
    public val state: StateFlow<AccountUiState> = _state.asStateFlow()
    private var session: AccountSessionData? = null
    private var work: Job? = null
    /** Latest account membership snapshot, used to authorize explicit device stores. */
    private var controllableDevices: List<AccountDeviceUi> = emptyList()

    public fun dispatch(intent: AccountIntent) {
        when (intent) {
            AccountIntent.Restore -> restore()
            is AccountIntent.Login -> login(intent)
            is AccountIntent.SelectDevice -> selectDevice(intent.deviceId)
            AccountIntent.RefreshDevices -> refreshDevices()
            AccountIntent.Retry -> retryFailedStage()
            AccountIntent.Logout -> logout()
            AccountIntent.Stop -> stop()
        }
    }

    public fun createSessionStore(scope: CoroutineScope): RemoteSessionStore? {
        val current = session ?: return null
        val target = current.targetDeviceId?.takeIf(String::isNotBlank) ?: return null
        return RemoteSessionStore.create(
            scope,
            backend.transport(current, target),
            deviceKey = target,
            persistence = persistence,
        )
    }

    public fun createWorkspaceStore(scope: CoroutineScope): RemoteWorkspaceStore? {
        val current = session ?: return null
        val target = current.targetDeviceId?.takeIf(String::isNotBlank) ?: return null
        return RemoteWorkspaceStore.create(
            scope,
            backend.transport(current, target),
            kotlinx.coroutines.Dispatchers.Default,
            target,
        )
    }

    /**
     * A session store addressed to one specific device, independent of the
     * currently selected control target. This is how a multi-device directory
     * loads several devices at once while the old single-target methods keep
     * their existing meaning.
     */
    public fun createSessionStore(scope: CoroutineScope, deviceId: String): RemoteSessionStore? {
        val current = session ?: return null
        val target = authorizedDeviceId(deviceId) ?: return null
        return RemoteSessionStore.create(
            scope,
            backend.transport(current, target),
            deviceKey = target,
            persistence = persistence,
        )
    }

    /** The explicit-device twin of [createWorkspaceStore]. */
    public fun createWorkspaceStore(scope: CoroutineScope, deviceId: String): RemoteWorkspaceStore? {
        val current = session ?: return null
        val target = authorizedDeviceId(deviceId) ?: return null
        return RemoteWorkspaceStore.create(
            scope,
            backend.transport(current, target),
            kotlinx.coroutines.Dispatchers.Default,
            target,
        )
    }

    /**
     * The directory may retain an offline account row, but an explicit store is
     * still only granted to a device in the latest authenticated membership
     * snapshot. Offline is allowed here so cached directory data can be shown;
     * the directory's online guard prevents commands from being sent.
     */
    private fun authorizedDeviceId(deviceId: String): String? {
        val target = deviceId.trim().takeIf(String::isNotBlank) ?: return null
        return controllableDevices.firstOrNull { it.id == target }?.id
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
                // A record we cannot decode may belong to a newer or older app.
                // Keep the opaque value in secure storage so a retry or upgraded
                // client can still read it; only clear this store's projection.
                session = null
                controllableDevices = emptyList()
                _state.value = AccountUiState.Failed(
                    AccountFailureReason.SECURE_STORAGE,
                    true,
                    AccountFailureStage.RESTORE,
                )
                return@launch
            }
            session = restored
            try {
                publishReady(restored, backend.listDevices(restored, deviceId))
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: CloudAccountException) {
                if (error.failure == CloudAccountFailure.AUTHENTICATION) {
                    expireSession(error.failure.toUiReason(), AccountFailureStage.DEVICE_LIST)
                } else {
                    _state.value = AccountUiState.Failed(
                        error.failure.toUiReason(),
                        true,
                        AccountFailureStage.DEVICE_LIST,
                    )
                }
            } catch (_: Throwable) {
                _state.value = AccountUiState.Failed(
                    AccountFailureReason.NETWORK,
                    true,
                    AccountFailureStage.DEVICE_LIST,
                )
            }
        }
    }

    private fun login(intent: AccountIntent.Login) {
        work?.cancel()
        _state.value = AccountUiState.SigningIn
        work = scope.launch {
            val loggedIn = try {
                backend.login(
                    intent.relayUrl,
                    intent.username,
                    intent.password,
                    deviceId,
                    deviceName,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: CloudAccountException) {
                failLogin(error.failure.toUiReason(), AccountFailureStage.AUTHENTICATION)
                return@launch
            } catch (_: Throwable) {
                // Live transport failures are normalized by CloudAccountClient.
                // An untyped failure here is therefore a crypto/protocol failure,
                // never evidence that secure storage was involved.
                failLogin(AccountFailureReason.MALFORMED_RESPONSE, AccountFailureStage.AUTHENTICATION)
                return@launch
            }

            controllableDevices = emptyList()
            if (!persistLogin(loggedIn)) return@launch
            session = loggedIn

            val devices = loadDevices(loggedIn) ?: return@launch

            // Never this device, even as a fallback: driving the phone from
            // the phone is what `canSelectAccountDevice` forbids, and a
            // target nothing can be asked of is worse than none at all.
            val preferred = AccountDevicePolicy.preferredTarget(devices, deviceId)
            val selected = loggedIn.copy(
                targetDeviceId = preferred?.id,
                targetDeviceName = preferred?.name,
            )
            if (!persistLogin(selected)) return@launch
            session = selected
            publishReady(selected, devices)
        }
    }

    private suspend fun loadDevices(current: AccountSessionData): List<AccountDeviceUi>? = try {
        backend.listDevices(current, deviceId)
    } catch (cancelled: CancellationException) {
        throw cancelled
    } catch (error: CloudAccountException) {
        _state.value = AccountUiState.Failed(
            error.failure.toUiReason(),
            true,
            AccountFailureStage.DEVICE_LIST,
        )
        null
    } catch (_: Throwable) {
        _state.value = AccountUiState.Failed(
            AccountFailureReason.NETWORK,
            true,
            AccountFailureStage.DEVICE_LIST,
        )
        null
    }

    private fun retryFailedStage() {
        val failed = _state.value as? AccountUiState.Failed ?: return
        val current = session ?: return
        if (!failed.canRetry || failed.stage != AccountFailureStage.DEVICE_LIST) return
        work?.cancel()
        _state.value = AccountUiState.SigningIn
        work = scope.launch {
            val devices = loadDevices(current) ?: return@launch
            val preferred = current.targetDeviceId
                ?.let { selectedId -> devices.firstOrNull { it.id == selectedId } }
                ?.takeIf { AccountDevicePolicy.canSelect(it, deviceId) }
                ?: AccountDevicePolicy.preferredTarget(devices, deviceId)
            val selected = current.copy(
                targetDeviceId = preferred?.id,
                targetDeviceName = preferred?.name,
            )
            // This retry only refreshes the volatile device-list projection. The
            // authenticated bytes saved before the failed list request stay exact.
            session = selected
            publishReady(selected, devices)
        }
    }

    private fun persistLogin(value: AccountSessionData): Boolean {
        val previous = try {
            secureStore.read(SESSION_KEY)
        } catch (_: Throwable) {
            failLogin(AccountFailureReason.SECURE_STORAGE, AccountFailureStage.SECURE_STORAGE)
            return false
        }
        return try {
            secureStore.write(SESSION_KEY, encodeRecord(value).encodeToByteArray())
            true
        } catch (_: Throwable) {
            // Platform secure stores are expected to update atomically. Restore a
            // fake or adapter that mutated before reporting failure as an extra
            // compatibility guard, without ever deleting pre-existing bytes.
            try {
                if (previous != null) secureStore.write(SESSION_KEY, previous)
                else secureStore.delete(SESSION_KEY)
            } catch (_: Throwable) {
                // The observable session still fails closed below. The original
                // write contract must preserve its previous value on failure.
            }
            failLogin(AccountFailureReason.SECURE_STORAGE, AccountFailureStage.SECURE_STORAGE)
            false
        }
    }

    private fun failLogin(reason: AccountFailureReason, stage: AccountFailureStage) {
        session = null
        controllableDevices = emptyList()
        _state.value = AccountUiState.Failed(reason, true, stage)
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
        if (!persistLogin(updated)) return
        session = updated
        _state.value = ready.copy(selectedDeviceId = selected.id, selectedDeviceName = selected.name)
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
                if (error.failure == CloudAccountFailure.AUTHENTICATION) {
                    expireSession(error.failure.toUiReason(), AccountFailureStage.DEVICE_LIST)
                } else {
                    _state.value = ready.copy(refreshing = false, refreshFailure = error.failure.toUiReason())
                }
            } catch (_: Throwable) {
                _state.value = ready.copy(refreshing = false, refreshFailure = AccountFailureReason.NETWORK)
            }
        }
    }

    private fun logout() {
        work?.cancel()
        work = null
        // Logout is immediately observable even when Keychain cannot remove the
        // durable record. A stale persisted record must never keep capabilities
        // active in this process.
        session = null
        controllableDevices = emptyList()
        _state.value = AccountUiState.SignedOut
        try {
            secureStore.delete(SESSION_KEY)
        } catch (_: Throwable) {
            _state.value = AccountUiState.Failed(
                AccountFailureReason.SECURE_STORAGE,
                true,
                AccountFailureStage.SECURE_STORAGE,
            )
        }
    }

    /** Clears every observable and persisted fact owned by an expired token. */
    private fun expireSession(reason: AccountFailureReason, stage: AccountFailureStage) {
        session = null
        controllableDevices = emptyList()
        _state.value = AccountUiState.Failed(reason, false, stage)
        try {
            secureStore.delete(SESSION_KEY)
        } catch (_: Throwable) {
            // The in-memory projection is already safe. A storage failure must
            // not put stale account devices back on screen.
        }
    }

    /**
     * The one place the relay's list becomes the list a screen renders, so the
     * filter cannot be forgotten by a caller — or applied twice with two
     * different answers on two platforms.
     */
    private fun publishReady(current: AccountSessionData, devices: List<AccountDeviceUi>) {
        controllableDevices = AccountDevicePolicy.controlTargets(devices, deviceId)
        _state.value = AccountUiState.Ready(
            userId = current.userId,
            username = current.username,
            devices = controllableDevices,
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
            persistence: MobilePersistenceStores? = null,
        ): AccountStore = AccountStore(scope, backend, secureStore, deviceId, deviceName, persistence)

        internal fun backend(log: CoreLog, legacyMobileDeviceNames: Set<String>): AccountBackend {
            val transportLog = log.asTransportLog()
            return CloudBackend(
                CloudAccountClient.create(transportLog, legacyMobileDeviceNames),
                transportLog,
            )
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
