package com.bitfun.mobile.core.feature.directory

import com.bitfun.mobile.core.domain.RemoteSession
import com.bitfun.mobile.core.feature.session.RemoteSessionStore
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.transport.RelayFailure
import com.bitfun.mobile.core.transport.RelayTransportException
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.DeserializationStrategy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class DeviceDirectoryStoreTest {
    @Test
    fun devicesLoadIndependentlyAndOneFailureDoesNotClearOthers() = runTest {
        val transports = mutableMapOf(
            "a" to FakeDeviceTransport("a"),
            "b" to FakeDeviceTransport("b"),
        )
        transports.getValue("b").sessionFailure = RelayFailure.Timeout
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(transports))

        store.dispatch(
            DeviceDirectoryIntent.Sync(
                listOf(
                    DeviceDirectoryDevice("a", "Alpha", true),
                    DeviceDirectoryDevice("b", "Beta", true),
                ),
            ),
        )
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        store.dispatch(DeviceDirectoryIntent.Load("b"))
        advanceUntilIdle()

        val a = store.state.value.device("a")!!
        assertEquals(DeviceDirectoryStatus.READY, a.status)
        assertEquals(listOf("/repo-a"), a.workspaces.map { it.path })
        assertEquals(listOf("s-a"), a.sessions.map { it.id })

        val failedB = store.state.value.device("b")!!
        assertEquals(DeviceDirectoryStatus.FAILED, failedB.status)
        assertEquals(DeviceDirectoryFailure.TIMEOUT, failedB.error)

        // A retry recovers b without disturbing a's already-loaded content.
        transports.getValue("b").sessionFailure = null
        store.dispatch(DeviceDirectoryIntent.Retry("b"))
        advanceUntilIdle()

        val recoveredB = store.state.value.device("b")!!
        assertEquals(DeviceDirectoryStatus.READY, recoveredB.status)
        assertEquals(listOf("s-b"), recoveredB.sessions.map { it.id })
        val stillA = store.state.value.device("a")!!
        assertEquals(DeviceDirectoryStatus.READY, stillA.status)
        assertEquals(listOf("s-a"), stillA.sessions.map { it.id })
    }

    @Test
    fun duplicateLoadCollapsesToOneFetch() = runTest {
        val transport = FakeDeviceTransport("a")
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(mutableMapOf("a" to transport)))

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        advanceUntilIdle()

        val a = store.state.value.device("a")!!
        assertEquals(DeviceDirectoryStatus.READY, a.status)
        assertEquals(1, transport.commands.count { it.cmd == "list_recent_workspaces" })
        assertEquals(1, transport.commands.count { it.cmd == "list_sessions" })
    }

    @Test
    fun expandPreservesCachedDataWithoutRefetching() = runTest {
        val transport = FakeDeviceTransport("a")
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(mutableMapOf("a" to transport)))

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true))))
        store.dispatch(DeviceDirectoryIntent.Expand("a"))
        advanceUntilIdle()

        val first = store.state.value.device("a")!!
        assertTrue(first.expanded)
        assertEquals(DeviceDirectoryStatus.READY, first.status)
        assertEquals(listOf("s-a"), first.sessions.map { it.id })
        val listSessionsBefore = transport.commands.count { it.cmd == "list_sessions" }
        assertEquals(1, listSessionsBefore)

        store.dispatch(DeviceDirectoryIntent.Collapse("a"))
        assertFalse(store.state.value.device("a")!!.expanded)

        // Re-expanding a ready device keeps the cache instead of re-fetching.
        store.dispatch(DeviceDirectoryIntent.Expand("a"))
        advanceUntilIdle()

        val again = store.state.value.device("a")!!
        assertTrue(again.expanded)
        assertEquals(DeviceDirectoryStatus.READY, again.status)
        assertEquals(listOf("s-a"), again.sessions.map { it.id })
        assertEquals(listSessionsBefore, transport.commands.count { it.cmd == "list_sessions" })
    }

    @Test
    fun stopCancelsRunningLoadsButKeepsLoadedData() = runTest {
        val transports = mutableMapOf(
            "a" to FakeDeviceTransport("a"),
            "b" to FakeDeviceTransport("b"),
        )
        val bGate = CompletableDeferred<Unit>()
        transports.getValue("b").sessionGate = bGate
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(transports))

        store.dispatch(
            DeviceDirectoryIntent.Sync(
                listOf(
                    DeviceDirectoryDevice("a", true),
                    DeviceDirectoryDevice("b", true),
                ),
            ),
        )
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        advanceUntilIdle()
        assertEquals(DeviceDirectoryStatus.READY, store.state.value.device("a")!!.status)

        store.dispatch(DeviceDirectoryIntent.Load("b"))
        runCurrent()

        // b reached its session request and is blocked, so it is still loading.
        assertEquals(DeviceDirectoryStatus.LOADING, store.state.value.device("b")!!.status)
        assertTrue(transports.getValue("b").commands.any { it.cmd == "list_sessions" })

        store.dispatch(DeviceDirectoryIntent.Stop)
        advanceUntilIdle()

        // Loaded data survives; the in-flight load is cancelled, not turned into a failure.
        assertEquals(DeviceDirectoryStatus.READY, store.state.value.device("a")!!.status)
        assertEquals(listOf("s-a"), store.state.value.device("a")!!.sessions.map { it.id })
        assertEquals(DeviceDirectoryStatus.IDLE, store.state.value.device("b")!!.status)
        assertFalse(bGate.isCompleted)
    }

    @Test
    fun offlineTransitionCancelsLoadAndCachedDataCanReloadAfterReconnect() = runTest {
        val transport = FakeDeviceTransport("a")
        val factory = FakeDeviceStoreFactory(mutableMapOf("a" to transport))
        val store = DeviceDirectoryStore.create(this, factory)
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        advanceUntilIdle()
        assertEquals(DeviceDirectoryStatus.READY, store.state.value.device("a")!!.status)

        val gate = CompletableDeferred<Unit>()
        transport.sessionGate = gate
        store.dispatch(DeviceDirectoryIntent.Retry("a"))
        runCurrent()
        assertEquals(DeviceDirectoryStatus.LOADING, store.state.value.device("a")!!.status)

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", "Alpha", false))))
        assertEquals(DeviceDirectoryStatus.CACHED, store.state.value.device("a")!!.status)
        assertFalse(store.state.value.device("a")!!.online)
        assertEquals(2, transport.commands.count { it.cmd == "list_recent_workspaces" })
        transport.sessionGate = null

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", "Alpha", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        advanceUntilIdle()
        assertEquals(DeviceDirectoryStatus.READY, store.state.value.device("a")!!.status)
    }

    @Test
    fun stopThenImmediateReloadIgnoresCancelledJobFinally() = runTest {
        val transport = FakeDeviceTransport("a")
        val gate = CompletableDeferred<Unit>()
        transport.sessionGate = gate
        val factory = FakeDeviceStoreFactory(mutableMapOf("a" to transport))
        val store = DeviceDirectoryStore.create(this, factory)
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        runCurrent()
        store.dispatch(DeviceDirectoryIntent.Stop)
        transport.sessionGate = null
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        advanceUntilIdle()
        assertEquals(DeviceDirectoryStatus.READY, store.state.value.device("a")!!.status)
        assertEquals(2, transport.commands.count { it.cmd == "list_sessions" })
        assertFalse(gate.isCompleted)
    }

    @Test
    fun partialSlotCreationStopsTheSessionStore() = runTest {
        val transport = FakeDeviceTransport("a")
        val factory = FakeDeviceStoreFactory(mutableMapOf("a" to transport), failWorkspace = setOf("a"))
        val store = DeviceDirectoryStore.create(this, factory)
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        assertEquals(DeviceDirectoryFailure.NOT_SIGNED_IN, store.state.value.device("a")!!.error)
        assertTrue(transport.commands.none { it.cmd == "list_sessions" })
    }

    @Test
    fun confirmedCreateReconcilesOnlyOwningDeviceAndServerEventuallyCalibratesIt() = runTest {
        val transports = mutableMapOf(
            "a" to FakeDeviceTransport("a"),
            "b" to FakeDeviceTransport("b"),
        )
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(transports))
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", true), DeviceDirectoryDevice("b", true))))
        store.dispatch(DeviceDirectoryIntent.Load("a"))
        store.dispatch(DeviceDirectoryIntent.Load("b"))
        advanceUntilIdle()
        val key = store.reconcileKey("a")!!
        val confirmed = RemoteSession(
            id = "created", title = "Confirmed", agentType = "cowork", status = "active",
            updatedAt = "created-time", createdAt = "created-time", messageCount = 1,
            workspacePath = "/assistant-not-current", workspaceName = "Assistant",
        )

        assertTrue(store.reconcileCreatedSession(key, confirmed))
        assertTrue(store.reconcileCreatedSession(key, confirmed))
        assertEquals(listOf("created", "s-a"), store.state.value.device("a")!!.sessions.map { it.id })
        assertEquals("/assistant-not-current", store.state.value.device("a")!!.sessions.first().workspacePath)
        assertEquals(listOf("s-b"), store.state.value.device("b")!!.sessions.map { it.id })

        // The first server list is behind the confirmed create; the local row survives.
        store.dispatch(DeviceDirectoryIntent.Retry("a"))
        advanceUntilIdle()
        assertEquals(1, store.state.value.device("a")!!.sessions.count { it.id == "created" })

        // Once the source returns the id, its newer fields replace the projection without duplication.
        transports.getValue("a").sessionJson =
            """[{"id":"created","title":"Server title","agent_type":"cowork","status":"idle","workspace_path":"/assistant-not-current","workspace_name":"Server assistant"}]"""
        store.dispatch(DeviceDirectoryIntent.Retry("a"))
        advanceUntilIdle()
        val calibrated = store.state.value.device("a")!!.sessions.single { it.id == "created" }
        assertEquals("Server title", calibrated.title)
        assertEquals("Server assistant", calibrated.workspaceName)
        assertEquals(1, store.state.value.device("a")!!.sessions.count { it.id == "created" })
    }

    @Test
    fun staleReconcileCannotReviveRemovedStoppedOrReconnectedDevice() = runTest {
        val transport = FakeDeviceTransport("a")
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(mutableMapOf("a" to transport)))
        val device = DeviceDirectoryDevice("a", true)
        val confirmed = RemoteSession(
            id = "created", title = "Created", agentType = "code", status = "active",
            updatedAt = "", createdAt = "", messageCount = 0,
            workspacePath = "/repo-a", workspaceName = null,
        )

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(device)))
        val removedKey = store.reconcileKey("a")!!
        store.dispatch(DeviceDirectoryIntent.Sync(emptyList()))
        assertFalse(store.reconcileCreatedSession(removedKey, confirmed))

        store.dispatch(DeviceDirectoryIntent.Sync(listOf(device)))
        val stoppedKey = store.reconcileKey("a")!!
        store.dispatch(DeviceDirectoryIntent.Stop)
        assertFalse(store.reconcileCreatedSession(stoppedKey, confirmed))

        val reconnectKey = store.reconcileKey("a")!!
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("a", false))))
        store.dispatch(DeviceDirectoryIntent.Sync(listOf(device)))
        assertFalse(store.reconcileCreatedSession(reconnectKey, confirmed))
        assertTrue(store.state.value.device("a")!!.sessions.none { it.id == "created" })
    }

    @Test
    fun offlineDevicesWithoutCacheRemainIdleAndMissingStoresFailTyped() = runTest {
        val transports = mutableMapOf("a" to FakeDeviceTransport("a"))
        val store = DeviceDirectoryStore.create(this, FakeDeviceStoreFactory(transports, missing = setOf("b")))

        store.dispatch(
            DeviceDirectoryIntent.Sync(
                listOf(
                    DeviceDirectoryDevice("a", true),
                    DeviceDirectoryDevice("b", "Offline", false),
                ),
            ),
        )
        store.dispatch(DeviceDirectoryIntent.Load("b"))
        advanceUntilIdle()

        // Offline rows stay idle; they are never asked for content.
        val offline = store.state.value.device("b")!!
        assertEquals(DeviceDirectoryStatus.IDLE, offline.status)
        assertTrue(offline.workspaces.isEmpty())
        assertTrue(offline.sessions.isEmpty())
        assertEquals(0, transports.getValue("a").commands.count { it.cmd == "list_sessions" })

        // An online device whose store cannot be created fails as NOT_SIGNED_IN.
        val missingTransports = mutableMapOf<String, FakeDeviceTransport>()
        val missingStore = DeviceDirectoryStore.create(
            this,
            FakeDeviceStoreFactory(missingTransports, missing = setOf("x")),
        )
        missingStore.dispatch(DeviceDirectoryIntent.Sync(listOf(DeviceDirectoryDevice("x", true))))
        missingStore.dispatch(DeviceDirectoryIntent.Load("x"))
        advanceUntilIdle()
        val x = missingStore.state.value.device("x")!!
        assertEquals(DeviceDirectoryStatus.FAILED, x.status)
        assertEquals(DeviceDirectoryFailure.NOT_SIGNED_IN, x.error)
    }
}

private class FakeDeviceStoreFactory(
    private val transports: MutableMap<String, FakeDeviceTransport>,
    private val missing: Set<String> = emptySet(),
    private val failWorkspace: Set<String> = emptySet(),
) : DeviceStoreFactory {
    override fun createSessionStore(scope: CoroutineScope, deviceId: String): RemoteSessionStore? {
        if (deviceId in missing) return null
        return RemoteSessionStore.create(scope, transports.getValue(deviceId))
    }

    override fun createWorkspaceStore(scope: CoroutineScope, deviceId: String): RemoteWorkspaceStore? {
        if (deviceId in missing || deviceId in failWorkspace) return null
        return RemoteWorkspaceStore.create(scope, transports.getValue(deviceId))
    }
}

private class FakeDeviceTransport(private val deviceId: String) : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    var workspacePath: String = "/repo-$deviceId"
    var sessionFailure: RelayFailure? = null
    var workspaceFailure: RelayFailure? = null
    var sessionGate: CompletableDeferred<Unit>? = null
    var sessionJson: String =
        """[{"id":"s-$deviceId","title":"Session $deviceId","agent_type":"code"}]"""

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        commands += command
        val json = when (command.cmd) {
            "list_recent_workspaces" -> {
                workspaceFailure?.let { throw RelayTransportException(it) }
                """{"resp":"ok","workspaces":[{"path":"/repo-$deviceId","name":"Repo $deviceId","last_opened":"2026-08-09","workspace_kind":"local"}]}"""
            }
            "list_assistants" -> """{"resp":"ok","assistants":[]}"""
            "get_workspace_info" ->
                """{"resp":"ok","has_workspace":true,"path":"$workspacePath","project_name":"Repo","git_branch":"main"}"""
            "list_sessions" -> {
                sessionFailure?.let { throw RelayTransportException(it) }
                sessionGate?.await()
                """{"resp":"ok","has_more":false,"sessions":$sessionJson}"""
            }
            "get_model_catalog" -> """{"resp":"ok"}"""
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}
