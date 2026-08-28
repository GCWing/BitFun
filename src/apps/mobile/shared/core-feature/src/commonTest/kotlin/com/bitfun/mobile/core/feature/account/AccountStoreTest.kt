package com.bitfun.mobile.core.feature.account

import com.bitfun.mobile.core.persistence.SecureStore
import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.transport.CloudAccountException
import com.bitfun.mobile.core.transport.CloudAccountFailure
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.DeserializationStrategy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class AccountStoreTest {
    @Test
    fun loginSelectsOnlineDesktopAndPersistsRestorableSession() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend()
        val store = AccountStore.create(this, backend, secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "top-secret-value"))
        advanceUntilIdle()

        val ready = assertIs<AccountUiState.Ready>(store.state.value)
        assertEquals("user-id", ready.userId)
        assertEquals("desktop-1", ready.selectedDeviceId)
        assertTrue(secure.read("cloud_account_session")?.isNotEmpty() == true)
        assertFalse(
            AccountIntent.Login("https://relay.test", "user", "top-secret-value")
                .toString()
                .contains("top-secret-value"),
        )

        val restored = AccountStore.create(this, backend, secure, "phone-1", "Android")
        restored.dispatch(AccountIntent.Restore)
        advanceUntilIdle()
        assertEquals("desktop-1", assertIs<AccountUiState.Ready>(restored.state.value).selectedDeviceId)
    }

    @Test
    fun onlyControllableDevicesReachTheList() = runTest {
        val store = AccountStore.create(this, FakeAccountBackend(), MemorySecureStore(), "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        val ready = assertIs<AccountUiState.Ready>(store.state.value)
        // This device and the user's other phone are gone; the offline desktop
        // stays, because "known but not running" is worth showing.
        assertEquals(listOf("desktop-1", "desktop-2"), ready.devices.map { it.id })
    }

    @Test
    fun deviceSelectionAndLogoutUpdateSecureState() = runTest {
        val secure = MemorySecureStore()
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        // Neither this device nor an offline one can become the control target.
        store.dispatch(AccountIntent.SelectDevice("phone-1"))
        store.dispatch(AccountIntent.SelectDevice("desktop-2"))
        assertEquals("desktop-1", assertIs<AccountUiState.Ready>(store.state.value).selectedDeviceId)
        store.dispatch(AccountIntent.Logout)

        assertIs<AccountUiState.SignedOut>(store.state.value)
        assertEquals(1, secure.deleteCount)
        assertNull(secure.read("cloud_account_session"))
    }

    @Test
    fun deviceSelectionWriteFailureRestoresBytesAndFailsClosed() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend().also { it.desktop2Online = true }
        val store = AccountStore.create(this, backend, secure, "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        assertEquals("desktop-1", assertIs<AccountUiState.Ready>(store.state.value).selectedDeviceId)
        val stored = secure.read("cloud_account_session")!!.toList()

        secure.failWrites = true
        secure.mutateBeforeWriteFailure = true
        store.dispatch(AccountIntent.SelectDevice("desktop-2"))

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.SECURE_STORAGE, failed.reason)
        assertEquals(AccountFailureStage.SECURE_STORAGE, failed.stage)
        assertEquals(stored, secure.read("cloud_account_session")?.toList())
        assertNull(store.createSessionStore(this))
        assertNull(store.createSessionStore(this, "desktop-1"))
        assertNull(store.createWorkspaceStore(this))
        assertNull(store.createWorkspaceStore(this, "desktop-1"))
        assertNull(store.cloudSettingsSource())
        assertTrue(backend.transportTargets.isEmpty())
        assertEquals(0, secure.deleteCount)
    }

    @Test
    fun refreshPicksUpADesktopThatCameOnlineAndSurvivesFailing() = runTest {
        val backend = FakeAccountBackend()
        val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        assertFalse(assertIs<AccountUiState.Ready>(store.state.value).devices.single { it.id == "desktop-2" }.online)

        backend.desktop2Online = true
        store.dispatch(AccountIntent.RefreshDevices)
        advanceUntilIdle()
        val refreshed = assertIs<AccountUiState.Ready>(store.state.value)
        assertTrue(refreshed.devices.single { it.id == "desktop-2" }.online)
        assertFalse(refreshed.refreshing)
        assertNull(refreshed.refreshFailure)

        // A failed refresh reports why and leaves the list it could not replace.
        backend.listFailure = CloudAccountFailure.NETWORK
        store.dispatch(AccountIntent.RefreshDevices)
        advanceUntilIdle()
        val failed = assertIs<AccountUiState.Ready>(store.state.value)
        assertEquals(AccountFailureReason.NETWORK, failed.refreshFailure)
        assertFalse(failed.refreshing)
        assertEquals(refreshed.devices, failed.devices)
        assertEquals("desktop-1", failed.selectedDeviceId)
    }

    @Test
    fun explicitDeviceStoresCoexistWithSelectedTargetStore() = runTest {
        val backend = FakeAccountBackend()
        val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        // The old single-target entry points still resolve the selected device.
        assertTrue(store.createSessionStore(this) != null)
        assertTrue(store.createWorkspaceStore(this) != null)

        // The new explicit-device entry points address a device without changing selection.
        assertTrue(store.createSessionStore(this, "desktop-2") != null)
        assertTrue(store.createWorkspaceStore(this, "desktop-2") != null)

        assertEquals(
            listOf("desktop-1", "desktop-1", "desktop-2", "desktop-2"),
            backend.transportTargets,
        )
    }

    @Test
    fun explicitDeviceStoresRequireAuthenticatedRegisteredControlTargets() = runTest {
        val backend = FakeAccountBackend()
        val signedOut = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")
        assertNull(signedOut.createSessionStore(this, "desktop-1"))
        assertNull(signedOut.createWorkspaceStore(this, "desktop-1"))
        assertTrue(backend.transportTargets.isEmpty())

        signedOut.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        assertNull(signedOut.createSessionStore(this, ""))
        assertNull(signedOut.createSessionStore(this, "phone-1"))
        assertNull(signedOut.createSessionStore(this, "phone-2"))
        assertNull(signedOut.createSessionStore(this, "unknown"))
        // Registered offline targets remain authorized for cache-backed directory rows.
        assertTrue(signedOut.createSessionStore(this, "desktop-2") != null)
        assertTrue(signedOut.createWorkspaceStore(this, "desktop-2") != null)
        assertEquals(listOf("desktop-2", "desktop-2"), backend.transportTargets)
    }

    @Test
    fun invalidRestoreKeepsOpaqueRecordAndCanBeRetried() = runTest {
        val secure = MemorySecureStore()
        val raw = "not-a-session-record".encodeToByteArray()
        secure.write("cloud_account_session", raw)
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Restore)
        advanceUntilIdle()
        assertEquals(AccountFailureReason.SECURE_STORAGE, assertIs<AccountUiState.Failed>(store.state.value).reason)
        assertTrue(assertIs<AccountUiState.Failed>(store.state.value).canRetry)
        assertEquals(0, secure.deleteCount)
        assertEquals(raw.toList(), secure.read("cloud_account_session")?.toList())

        store.dispatch(AccountIntent.Restore)
        advanceUntilIdle()
        assertEquals(AccountFailureReason.SECURE_STORAGE, assertIs<AccountUiState.Failed>(store.state.value).reason)
        assertEquals(0, secure.deleteCount)
        assertEquals(raw.toList(), secure.read("cloud_account_session")?.toList())
    }

    @Test
    fun secureReadFailureFailsClosedWithoutDeletingStoredBytes() = runTest {
        val secure = MemorySecureStore()
        val raw = "opaque-existing-session".encodeToByteArray()
        secure.write("cloud_account_session", raw)
        secure.failReads = true
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Restore)
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.SECURE_STORAGE, failed.reason)
        assertEquals(AccountFailureStage.RESTORE, failed.stage)
        assertNull(store.cloudSettingsSource())
        assertEquals(0, secure.deleteCount)
        secure.failReads = false
        assertEquals(raw.toList(), secure.read("cloud_account_session")?.toList())
    }

    @Test
    fun legacyRestoreKeepsOpaqueRecord() = runTest {
        val secure = MemorySecureStore()
        val raw = "{\"token\":\"legacy-token\"}".encodeToByteArray()
        secure.write("cloud_account_session", raw)
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Restore)
        advanceUntilIdle()

        assertEquals(AccountFailureReason.SECURE_STORAGE, assertIs<AccountUiState.Failed>(store.state.value).reason)
        assertEquals(0, secure.deleteCount)
        assertEquals(raw.toList(), secure.read("cloud_account_session")?.toList())
    }

    @Test
    fun validRestoreDoesNotDeletePersistedSession() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend()
        val first = AccountStore.create(this, backend, secure, "phone-1", "Android")
        first.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        val raw = secure.read("cloud_account_session")

        val restored = AccountStore.create(this, backend, secure, "phone-1", "Android")
        restored.dispatch(AccountIntent.Restore)
        advanceUntilIdle()

        assertIs<AccountUiState.Ready>(restored.state.value)
        assertEquals(0, secure.deleteCount)
        assertEquals(raw?.toList(), secure.read("cloud_account_session")?.toList())
    }

    @Test
    fun expiredRefreshClearsDevicesAndThePersistedSession() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend()
        val store = AccountStore.create(this, backend, secure, "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        assertTrue(assertIs<AccountUiState.Ready>(store.state.value).devices.isNotEmpty())

        backend.listFailure = CloudAccountFailure.AUTHENTICATION
        store.dispatch(AccountIntent.RefreshDevices)
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.AUTHENTICATION, failed.reason)
        assertNull(secure.read("cloud_account_session"))
        assertNull(store.createSessionStore(this))
    }

    @Test
    fun signInWithNothingOnlinePicksNoTarget() = runTest {
        val backend = FakeAccountBackend().also { it.desktop1Online = false }
        val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        // Not this device as a consolation prize: the live account has ten
        // registered devices and no desktop running, and that is what it says.
        assertNull(assertIs<AccountUiState.Ready>(store.state.value).selectedDeviceId)
    }

    @Test
    fun backendFailuresStayTyped() = runTest {
        val backend = FakeAccountBackend().also { it.failure = CloudAccountFailure.AUTHENTICATION }
        val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "wrong"))
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.AUTHENTICATION, failed.reason)
        assertEquals(AccountFailureStage.AUTHENTICATION, failed.stage)
    }

    @Test
    fun unknownAuthenticationFailureIsProtocolFailureNotSecureStorage() = runTest {
        val backend = FakeAccountBackend().also { it.loginThrowable = IllegalStateException("crypto failed") }
        val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.MALFORMED_RESPONSE, failed.reason)
        assertEquals(AccountFailureStage.AUTHENTICATION, failed.stage)
    }

    @Test
    fun secureStoreWriteFailureIsTheOnlyUntypedSecureStorageFailure() = runTest {
        val secure = MemorySecureStore(failWrites = true)
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.SECURE_STORAGE, failed.reason)
        assertEquals(AccountFailureStage.SECURE_STORAGE, failed.stage)
    }

    @Test
    fun deviceListFailurePreservesAuthenticatedAccountAndPersistedData() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend().also { it.listThrowable = IllegalStateException("transport failed") }
        val store = AccountStore.create(this, backend, secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.NETWORK, failed.reason)
        assertEquals(AccountFailureStage.DEVICE_LIST, failed.stage)
        assertTrue(store.cloudSettingsSource() != null)
        assertTrue(secure.read("cloud_account_session")?.isNotEmpty() == true)
        assertEquals(0, secure.deleteCount)
    }

    @Test
    fun deviceListFailuresKeepTheirTypedReasons() = runTest {
        CloudAccountFailure.entries.forEach { transportReason ->
            val backend = FakeAccountBackend().also { it.listFailure = transportReason }
            val store = AccountStore.create(this, backend, MemorySecureStore(), "phone-1", "Android")

            store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
            advanceUntilIdle()

            val failed = assertIs<AccountUiState.Failed>(store.state.value)
            assertEquals(transportReason.toExpectedReason(), failed.reason)
            assertEquals(AccountFailureStage.DEVICE_LIST, failed.stage)
        }
    }

    @Test
    fun deviceListRetryReusesSessionWithoutTouchingStoredBytes() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend().also { it.listFailure = CloudAccountFailure.TIMEOUT }
        val store = AccountStore.create(this, backend, secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureStage.DEVICE_LIST, failed.stage)
        val stored = secure.read("cloud_account_session")!!.toList()
        val writes = secure.writeCount
        val deletes = secure.deleteCount

        backend.listFailure = null
        store.dispatch(AccountIntent.Retry)
        advanceUntilIdle()

        val ready = assertIs<AccountUiState.Ready>(store.state.value)
        assertEquals("desktop-1", ready.selectedDeviceId)
        assertEquals(stored, secure.read("cloud_account_session")?.toList())
        assertEquals(writes, secure.writeCount)
        assertEquals(deletes, secure.deleteCount)
    }

    @Test
    fun writeFailureFailsClosedAndRestoresExistingBytes() = runTest {
        val secure = MemorySecureStore()
        val existing = "existing-session-bytes".encodeToByteArray()
        secure.write("cloud_account_session", existing)
        secure.failWrites = true
        secure.mutateBeforeWriteFailure = true
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")

        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.SECURE_STORAGE, failed.reason)
        assertNull(store.cloudSettingsSource())
        assertNull(store.createSessionStore(this))
        assertEquals(existing.toList(), secure.read("cloud_account_session")?.toList())
        assertEquals(0, secure.deleteCount)
    }

    @Test
    fun deleteFailureLogsOutInMemoryWithoutDestroyingStoredBytes() = runTest {
        val secure = MemorySecureStore()
        val store = AccountStore.create(this, FakeAccountBackend(), secure, "phone-1", "Android")
        store.dispatch(AccountIntent.Login("https://relay.test", "user", "password"))
        advanceUntilIdle()
        val stored = secure.read("cloud_account_session")!!.toList()
        secure.failDeletes = true

        store.dispatch(AccountIntent.Logout)

        val failed = assertIs<AccountUiState.Failed>(store.state.value)
        assertEquals(AccountFailureReason.SECURE_STORAGE, failed.reason)
        assertNull(store.cloudSettingsSource())
        assertNull(store.createSessionStore(this))
        assertEquals(stored, secure.read("cloud_account_session")?.toList())
    }

    @Test
    fun offlineRestoreKeepsEncryptedSessionForRetry() = runTest {
        val secure = MemorySecureStore()
        val backend = FakeAccountBackend()
        val first = AccountStore.create(this, backend, secure, "phone-1", "Android")
        first.dispatch(AccountIntent.Login("https://relay.test", "user", "top-secret-value"))
        advanceUntilIdle()
        backend.listFailure = CloudAccountFailure.NETWORK

        val restored = AccountStore.create(this, backend, secure, "phone-1", "Android")
        restored.dispatch(AccountIntent.Restore)
        advanceUntilIdle()

        assertEquals(AccountFailureReason.NETWORK, assertIs<AccountUiState.Failed>(restored.state.value).reason)
        assertTrue(secure.read("cloud_account_session")?.isNotEmpty() == true)
    }
}

private class MemorySecureStore(
    var failWrites: Boolean = false,
    var failReads: Boolean = false,
    var failDeletes: Boolean = false,
    var mutateBeforeWriteFailure: Boolean = false,
) : SecureStore {
    private val values = mutableMapOf<String, ByteArray>()
    var writeCount: Int = 0
        private set
    var deleteCount: Int = 0
        private set

    override fun read(key: String): ByteArray? {
        if (failReads) error("secure store read failed")
        return values[key]?.copyOf()
    }

    override fun write(key: String, value: ByteArray) {
        writeCount += 1
        if (failWrites) {
            if (mutateBeforeWriteFailure) {
                values[key] = value.copyOf()
                // Let AccountStore prove that it restores the previous bytes.
                failWrites = false
            }
            error("secure store write failed")
        }
        values[key] = value.copyOf()
    }

    override fun delete(key: String) {
        deleteCount += 1
        if (failDeletes) error("secure store delete failed")
        values.remove(key)
    }
}

private fun CloudAccountFailure.toExpectedReason(): AccountFailureReason = when (this) {
    CloudAccountFailure.INVALID_CREDENTIALS -> AccountFailureReason.INVALID_CREDENTIALS
    CloudAccountFailure.AUTHENTICATION -> AccountFailureReason.AUTHENTICATION
    CloudAccountFailure.RATE_LIMITED -> AccountFailureReason.RATE_LIMITED
    CloudAccountFailure.RELAY_UNAVAILABLE -> AccountFailureReason.RELAY_UNAVAILABLE
    CloudAccountFailure.NETWORK -> AccountFailureReason.NETWORK
    CloudAccountFailure.TIMEOUT -> AccountFailureReason.TIMEOUT
    CloudAccountFailure.MALFORMED_RESPONSE -> AccountFailureReason.MALFORMED_RESPONSE
}

private class FakeAccountBackend : AccountBackend {
    var failure: CloudAccountFailure? = null
    var loginThrowable: Throwable? = null
    var listFailure: CloudAccountFailure? = null
    var listThrowable: Throwable? = null
    var desktop1Online: Boolean = true
    var desktop2Online: Boolean = false
    var settings: String? = null
    val transportTargets = mutableListOf<String>()
    override suspend fun login(
        relayUrl: String,
        username: String,
        password: String,
        deviceId: String,
        deviceName: String,
    ): AccountSessionData {
        loginThrowable?.let { throw it }
        failure?.let { throw CloudAccountException(it) }
        return AccountSessionData(
            relayUrl,
            username,
            "token",
            "user-id",
            ByteArray(32) { it.toByte() },
            null,
            null,
        )
    }

    override suspend fun listDevices(session: AccountSessionData, selfDeviceId: String): List<AccountDeviceUi> {
        listThrowable?.let { throw it }
        listFailure?.let { throw CloudAccountException(it) }
        // The shape the live account returns: this device, one of the user's
        // other phones, and the desktops that are the only real targets.
        return listOf(
            AccountDeviceUi("phone-1", "Android", true, null),
            AccountDeviceUi("phone-2", "HarmonyOS Phone", true, 1),
            AccountDeviceUi("desktop-1", "Desktop", desktop1Online, 1),
            AccountDeviceUi("desktop-2", "DESKTOP-KM3L4UI", desktop2Online, 1),
        )
    }

    override suspend fun fetchSettings(session: AccountSessionData): String? = settings

    override fun transport(session: AccountSessionData, targetDeviceId: String): RemoteCommandTransport {
        transportTargets += targetDeviceId
        return object : RemoteCommandTransport {
            override suspend fun <T : CommandStatus> send(
                deserializer: DeserializationStrategy<T>,
                command: RemoteCommand,
                timeoutMs: Long,
            ): T = error("unused")
        }
    }
}
