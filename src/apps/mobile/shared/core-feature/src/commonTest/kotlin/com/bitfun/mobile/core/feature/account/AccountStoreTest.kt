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
        assertNull(secure.read("cloud_account_session"))
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

private class MemorySecureStore : SecureStore {
    private val values = mutableMapOf<String, ByteArray>()
    override fun read(key: String): ByteArray? = values[key]?.copyOf()
    override fun write(key: String, value: ByteArray) {
        values[key] = value.copyOf()
    }
    override fun delete(key: String) {
        values.remove(key)
    }
}

private class FakeAccountBackend : AccountBackend {
    var failure: CloudAccountFailure? = null
    var listFailure: CloudAccountFailure? = null
    var desktop1Online: Boolean = true
    var desktop2Online: Boolean = false
    var settings: String? = null
    override suspend fun login(
        relayUrl: String,
        username: String,
        password: String,
        deviceId: String,
        deviceName: String,
    ): AccountSessionData {
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

    override suspend fun listDevices(session: AccountSessionData): List<AccountDeviceUi> {
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

    override fun transport(session: AccountSessionData, targetDeviceId: String): RemoteCommandTransport =
        object : RemoteCommandTransport {
            override suspend fun <T : CommandStatus> send(
                deserializer: DeserializationStrategy<T>,
                command: RemoteCommand,
                timeoutMs: Long,
            ): T = error("unused")
        }
}
