package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.domain.RemoteSession
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.persistence.ChatLocalStore
import com.openbitfun.mobile.core.persistence.DraftStore
import com.openbitfun.mobile.core.persistence.MobilePersistenceStores
import com.openbitfun.mobile.core.persistence.PersistedChatMessage
import com.openbitfun.mobile.core.persistence.PersistedChatSession
import com.openbitfun.mobile.core.persistence.PersistedRemoteCursor
import com.openbitfun.mobile.core.persistence.PersistedRemoteMessage
import com.openbitfun.mobile.core.persistence.PersistedRemoteSession
import com.openbitfun.mobile.core.persistence.RemoteSessionListStore
import com.openbitfun.mobile.core.persistence.RemoteTranscriptStore
import com.openbitfun.mobile.core.protocol.CommandStatus
import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.RemoteCommand
import com.openbitfun.mobile.core.transport.RelayFailure
import com.openbitfun.mobile.core.transport.RelayTransportException
import com.openbitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.DeserializationStrategy
import kotlin.coroutines.Continuation
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

@OptIn(ExperimentalCoroutinesApi::class)
class RemoteSessionPersistenceTest {
    @Test
    fun coldStartShowsCachedListThenServerList() = runTest {
        val stores = MemoryPersistence()
        stores.sessions.rows = listOf(PersistedRemoteSession(sessionId = "cached", title = "Cached"))
        val transport = PersistenceTransport()
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Load)
        assertEquals("cached", assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.single().id)
        advanceUntilIdle()
        assertEquals("server", assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.single().id)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun confirmedCreateReconcilePersistsByDeviceAndRebuildRestoresIt() = runTest {
        val stores = MemoryPersistence()
        stores.sessions.byDevice["device-b"] = listOf(PersistedRemoteSession(sessionId = "other", title = "Other"))
        val first = RemoteSessionStore.create(this, PersistenceTransport(), "device-a", stores.stores)
        val confirmed = RemoteSession(
            id = "created", title = "Created", agentType = "cowork", status = "active",
            updatedAt = "now", createdAt = "now", messageCount = 1,
            workspacePath = "/assistant", workspaceName = "Assistant",
        )

        assertEquals(true, first.reconcileConfirmedCreatedSession(confirmed))
        assertEquals(listOf("created"), stores.sessions.byDevice.getValue("device-a").map { it.sessionId })
        assertEquals(listOf("other"), stores.sessions.byDevice.getValue("device-b").map { it.sessionId })
        assertEquals("/assistant", stores.sessions.byDevice.getValue("device-a").single().workspacePath)

        assertEquals(true, stores.sessions.byDevice.getValue("device-a").single().pendingConfirmed)

        val laggingTransport = PersistenceTransport().apply { sessionsJson = "[]" }
        val rebuilt = RemoteSessionStore.create(this, laggingTransport, "device-a", stores.stores)
        rebuilt.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        val afterLaggingList = assertIs<RemoteSessionUiState.Ready>(rebuilt.state.value).sessions.single()
        assertEquals("created", afterLaggingList.id)
        assertEquals("/assistant", afterLaggingList.workspacePath)
        assertEquals(true, stores.sessions.byDevice.getValue("device-a").single().pendingConfirmed)

        laggingTransport.sessionsJson =
            """[{"id":"created","title":"Server calibrated","agent_type":"cowork","status":"idle","workspace_path":"/assistant","workspace_name":"Server assistant"}]"""
        rebuilt.dispatch(RemoteSessionIntent.Refresh)
        advanceUntilIdle()
        val calibrated = assertIs<RemoteSessionUiState.Ready>(rebuilt.state.value).sessions.single()
        assertEquals("Server calibrated", calibrated.title)
        assertEquals("Server assistant", calibrated.workspaceName)
        assertEquals(false, stores.sessions.byDevice.getValue("device-a").single().pendingConfirmed)
        first.stop()
        rebuilt.stop()
    }

    @Test
    fun validCreateIdIsDurableBeforeGatedModelInitializationAndSurvivesStop() = runTest {
        val stores = MemoryPersistence()
        val transport = PersistenceTransport()
        transport.commandGates["set_session_model"] = CompletableDeferred()
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(
            RemoteSessionIntent.CreateSessionOperation(
                "durable-create", "cowork", "Created", "", "model-primary", "/assistant",
            ),
        )
        runCurrent()

        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        val persisted = stores.sessions.byDevice.getValue("device-a").single()
        assertEquals("created", persisted.sessionId)
        assertEquals("/assistant", persisted.workspacePath)
        assertEquals(true, persisted.pendingConfirmed)
        store.stop()
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)

        val rebuiltTransport = PersistenceTransport().apply { sessionsJson = "[]" }
        val rebuilt = RemoteSessionStore.create(this, rebuiltTransport, "device-a", stores.stores)
        rebuilt.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        assertEquals("created", assertIs<RemoteSessionUiState.Ready>(rebuilt.state.value).sessions.single().id)
        rebuilt.stop()
    }

    @Test
    fun draftIsSavedRestoredAndClearedAfterSend() = runTest {
        val stores = MemoryPersistence()
        val transport = PersistenceTransport()
        val first = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        first.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        first.dispatch(RemoteSessionIntent.UpdateDraft("keep me"))
        assertEquals("keep me", stores.drafts.values["remote-composer:device-a:server"])
        val second = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        second.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        assertEquals("keep me", assertIs<RemoteSessionUiState.Ready>(second.state.value).draft)
        second.dispatch(RemoteSessionIntent.SendMessage("server", "hello")); runCurrent()
        assertEquals(null, stores.drafts.values["remote-composer:device-a:server"])
        first.dispatch(RemoteSessionIntent.Stop)
        second.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun activeTurnIsPersistedOnEndAndRestoredOnReopen() = runTest {
        val stores = MemoryPersistence()
        val transport = PersistenceTransport()
        transport.messagesJson = """[{"id":"m-1","role":"assistant","content":"All done"}]"""
        transport.polls = listOf(
            """{"resp":"ok","version":1,"changed":true,"session_state":"running","active_turn":{"turn_id":"t-1","status":"active","text":"All "}}""",
            """{"resp":"ok","version":2,"changed":true,"session_state":"idle","active_turn":{"turn_id":"t-1","status":"completed","text":"All done"}}""",
            """{"resp":"ok","version":2,"changed":false,"session_state":"idle"}""",
        )
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        advanceTimeBy(350); runCurrent()
        assertEquals("m-1", stores.transcripts.rows["device-a::server"]!!.single().messageId)
        assertEquals("0", stores.transcripts.cursors["device-a::server"]!!.pollVersion)
        store.dispatch(RemoteSessionIntent.Stop)

        val transport2 = PersistenceTransport()
        transport2.messagesJson = transport.messagesJson
        val store2 = RemoteSessionStore.create(this, transport2, "device-a", stores.stores)
        store2.dispatch(RemoteSessionIntent.Open("server"))
        assertEquals("m-1", assertIs<RemoteSessionUiState.Ready>(store2.state.value).timeline?.persistedMessages?.single()?.id)
        runCurrent(); store2.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun disconnectDuringPollRetainsPersistedTranscriptAndRecovers() = runTest {
        val stores = MemoryPersistence()
        stores.transcripts.rows["device-a::server"] = listOf(
            PersistedRemoteMessage(
                messageId = "m-1", sessionId = "server", role = "assistant", text = "cached",
                payloadJson = """{"render_version":null}""",
            ),
        )
        val transport = PersistenceTransport()
        transport.messagesJson = """[{"id":"m-1","role":"assistant","content":"cached"}]"""
        transport.pollFailure = RelayFailure.NetworkUnreachable
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        assertEquals(ConnectionPhase.RECONNECTING, store.connectionPhase.value)
        assertEquals(1, stores.transcripts.rows["device-a::server"]?.size)
        transport.pollFailure = null
        advanceTimeBy(10_000); runCurrent()
        assertEquals(ConnectionPhase.CONNECTED, store.connectionPhase.value)
        assertEquals("m-1", assertIs<RemoteSessionUiState.Ready>(store.state.value).timeline?.persistedMessages?.single()?.id)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun partialRereadDoesNotTruncateOlderCachedMessages() = runTest {
        val stores = MemoryPersistence()
        stores.transcripts.rows["device-a::server"] = (0 until 120).map { i ->
            PersistedRemoteMessage(
                messageId = "m-$i", sessionId = "server", role = "assistant",
                text = "msg $i", payloadJson = "{}",
            )
        }
        val transport = PersistenceTransport()
        transport.messagesJson = (20 until 120).joinToString(prefix = "[", postfix = "]") { i ->
            "{\"id\":\"m-$i\",\"role\":\"assistant\",\"content\":\"msg $i\"}"
        }
        transport.hasMore = true
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        assertEquals(120, stores.transcripts.rows["device-a::server"]?.size)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun staleLoadMoreDoesNotWriteSessionPersistence() = runTest {
        val stores = MemoryPersistence()
        val transport = PersistenceTransport().apply { hasMore = true }
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        val savesBeforeLatePage = stores.sessions.saveCount

        transport.nonCancellableCommands += "list_sessions"
        store.dispatch(RemoteSessionIntent.LoadMore)
        runCurrent()
        val lateLoadMore = transport.lateCommandContinuations.remove("list_sessions")!!
        store.dispatch(RemoteSessionIntent.Open("server"))
        runCurrent()
        lateLoadMore.resume(Unit)
        runCurrent()

        assertEquals(savesBeforeLatePage, stores.sessions.saveCount)
        store.stop()
    }

    @Test
    fun corruptedPayloadIsRetainedAsDegradedMessage() = runTest {
        val stores = MemoryPersistence()
        stores.transcripts.rows["device-a::server"] = listOf(PersistedRemoteMessage(messageId = "bad", sessionId = "server", role = "assistant", text = "retained", payloadJson = "not-json"))
        val transport = PersistenceTransport()
        transport.messagesJson = "[{\"id\":\"bad\",\"role\":\"assistant\",\"content\":\"retained\"}]"
        val store = RemoteSessionStore.create(this, transport, "device-a", stores.stores)
        store.dispatch(RemoteSessionIntent.Open("server")); runCurrent()
        assertEquals("bad", assertIs<RemoteSessionUiState.Ready>(store.state.value).timeline?.persistedMessages?.single()?.id)
        assertEquals(1, stores.transcripts.rows["device-a::server"]?.size)
        store.dispatch(RemoteSessionIntent.Stop)
    }
}

private class MemoryPersistence {
    val drafts = MemoryDrafts()
    val sessions = MemorySessions()
    val transcripts = MemoryTranscripts()
    val stores = MobilePersistenceStores(drafts, NoOpChats(), sessions, transcripts)
}

private class MemoryDrafts : DraftStore {
    val values = mutableMapOf<String, String>()
    override fun load(draftId: String): String? = values[draftId]
    override fun save(draftId: String, text: String) { values[draftId] = text }
    override fun delete(draftId: String) { values.remove(draftId) }
}

private class NoOpChats : ChatLocalStore {
    override fun listSessions(agentType: String): List<PersistedChatSession> = emptyList()
    override fun loadSession(sessionId: String): PersistedChatSession? = null
    override fun loadMessages(sessionId: String): List<PersistedChatMessage> = emptyList()
    override fun saveSession(session: PersistedChatSession) = Unit
    override fun saveMessage(message: PersistedChatMessage) = Unit
    override fun pinSession(agentType: String, sessionId: String, pinned: Boolean) = Unit
    override fun setSessionStatus(sessionId: String, status: String) = Unit
    override fun deleteSession(sessionId: String) = Unit
}

private class MemorySessions : RemoteSessionListStore {
    var rows = emptyList<PersistedRemoteSession>()
    val byDevice = mutableMapOf<String, List<PersistedRemoteSession>>()
    var more = false
    var saveCount = 0
    override fun load(deviceKey: String): List<PersistedRemoteSession> = byDevice[deviceKey] ?: rows
    override fun save(deviceKey: String, sessions: List<PersistedRemoteSession>, hasMore: Boolean) {
        saveCount += 1
        rows = sessions
        byDevice[deviceKey] = sessions
        more = hasMore
    }
    override fun hasMore(deviceKey: String): Boolean = more
}

private class MemoryTranscripts : RemoteTranscriptStore {
    val rows = mutableMapOf<String, List<PersistedRemoteMessage>>()
    val cursors = mutableMapOf<String, PersistedRemoteCursor>()
    override fun load(deviceKey: String, sessionId: String) = rows["$deviceKey::$sessionId"].orEmpty()
    override fun append(deviceKey: String, sessionId: String, startSeq: Int, messages: List<PersistedRemoteMessage>) = Unit
    override fun replace(deviceKey: String, sessionId: String, messages: List<PersistedRemoteMessage>) { rows["$deviceKey::$sessionId"] = messages }
    override fun loadCursor(deviceKey: String, sessionId: String) = cursors["$deviceKey::$sessionId"]
    override fun saveCursor(deviceKey: String, sessionId: String, cursor: PersistedRemoteCursor) { cursors["$deviceKey::$sessionId"] = cursor }
}

private class PersistenceTransport : RemoteCommandTransport {
    val commandGates = mutableMapOf<String, CompletableDeferred<Unit>>()
    val nonCancellableCommands = mutableSetOf<String>()
    val lateCommandContinuations = mutableMapOf<String, Continuation<Unit>>()
    var sessionsJson: String = """[{"id":"server","title":"Server","agent_type":"code"}]"""
    var messagesJson: String = "[]"
    var hasMore: Boolean = false
    var polls: List<String> = listOf("""{"resp":"ok","version":1,"changed":false,"session_state":"idle"}""")
    private var pollIndex = 0
    var pollFailure: RelayFailure? = null
    val sinceVersions = mutableListOf<Int>()
    override suspend fun <T : CommandStatus> send(deserializer: DeserializationStrategy<T>, command: RemoteCommand, timeoutMs: Long): T {
        commandGates[command.cmd]?.await()
        if (nonCancellableCommands.remove(command.cmd)) {
            suspendCoroutine { continuation -> lateCommandContinuations[command.cmd] = continuation }
        }
        if (command.cmd == "poll_session") {
            sinceVersions += command.sinceVersion ?: 0
            pollFailure?.let { throw RelayTransportException(it) }
        }
        val json = when (command.cmd) {
            "get_workspace_info" -> "{\"resp\":\"ok\",\"path\":\"/repo\"}"
            "create_session" -> "{\"resp\":\"ok\",\"session_id\":\"created\",\"title\":\"Created\"}"
            "set_session_model" -> "{\"resp\":\"ok\",\"model_id\":\"model-primary\"}"
            "list_sessions" -> "{\"resp\":\"ok\",\"sessions\":$sessionsJson,\"has_more\":$hasMore}"
            "get_session_messages" -> "{\"resp\":\"ok\",\"messages\":$messagesJson,\"has_more\":$hasMore}"
            "get_permission_mode" -> "{\"resp\":\"ok\",\"mode\":\"ask\"}"
            "get_model_catalog" -> "{\"resp\":\"ok\",\"catalog\":{\"version\":0,\"models\":[],\"default_models\":{}}}"
            "poll_session" -> polls[minOf(pollIndex++, polls.lastIndex)]
            "send_message" -> "{\"resp\":\"ok\",\"turn_id\":\"t\"}"
            else -> "{\"resp\":\"ok\"}"
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}
