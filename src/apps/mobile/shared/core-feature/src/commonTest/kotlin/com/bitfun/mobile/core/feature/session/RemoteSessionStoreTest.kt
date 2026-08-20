package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.transport.RelayFailure
import com.bitfun.mobile.core.transport.RelayTransportException
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.DeserializationStrategy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertIs
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class RemoteSessionStoreTest {
    @Test
    fun listsSessionsForTheWorkspaceTheDesktopHasOpen() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("s-code", "s-cowork", "s-agentic"), ready.sessions.map { it.id })
        val list = transport.commands.first { it.cmd == "list_sessions" }
        // The desktop rejects list_sessions outright without a workspace, which is
        // why the store resolves one first instead of sending a bare command.
        assertEquals("/repo", list.workspacePath)
        assertEquals(30, list.limit)
        assertEquals(0, list.offset)
        assertNull(list.query)
    }

    @Test
    fun reportsNoWorkspaceWithoutAskingForSessions() = runTest {
        val transport = FakeSessionTransport()
        transport.workspacePath = ""
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val failed = assertIs<RemoteSessionUiState.Failed>(store.state.value)
        assertEquals(RemoteSessionFailureReason.NO_WORKSPACE, failed.reason)
        assertTrue(transport.commands.none { it.cmd == "list_sessions" })
    }

    @Test
    fun searchSendsATrimmedQueryAndKeepsTheListOnScreen() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteSessionIntent.Search("  parser  "))
        runCurrent()
        assertIs<RemoteSessionUiState.Ready>(store.state.value)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("  parser  ", ready.query)
        assertEquals("parser", transport.commands.last { it.cmd == "list_sessions" }.query)
    }

    @Test
    fun agentFilterKeepsLegacyAgenticSessionsInTheCodeTab() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteSessionIntent.SetAgentFilter(SessionAgentFilter.CODE))
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("s-code", "s-agentic"), ready.sessions.map { it.id })
        // Narrowing happens on the client, so pages are pulled at the desktop's cap.
        assertEquals(100, transport.commands.last { it.cmd == "list_sessions" }.limit)
    }

    @Test
    fun loadMoreAppendsTheNextPageWithoutRepeatingRows() = runTest {
        val transport = FakeSessionTransport()
        transport.paged = true
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).hasMore)

        store.dispatch(RemoteSessionIntent.LoadMore)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("page-0", "page-1"), ready.sessions.map { it.id })
        assertEquals(1, transport.commands.last { it.cmd == "list_sessions" }.offset)
    }

    @Test
    fun createSessionNamesTheSessionAndOpensIt() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteSessionIntent.CreateSession("code", "", "review the parser", null))
        runCurrent()

        val create = transport.commands.first { it.cmd == "create_session" }
        assertEquals("Remote Code Session", create.sessionName)
        assertEquals("/repo", create.workspacePath)
        assertEquals("code", create.agentType)
        assertEquals("review the parser", transport.commands.first { it.cmd == "send_message" }.content)
        assertEquals("s-new", assertIs<RemoteSessionUiState.Ready>(store.state.value).selectedSessionId)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun createSessionUsesTheWorkspaceTheDesktopIsOnNow() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        // What the new-session screen does: pick a workspace, which goes out as
        // `set_workspace` on the workspace store, then create. The path cached
        // at load time is stale by the time the create lands.
        transport.workspacePath = "/other"
        store.dispatch(RemoteSessionIntent.CreateSession("code", "", "review the parser", null))
        runCurrent()

        assertEquals("/other", transport.commands.first { it.cmd == "create_session" }.workspacePath)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun deleteSessionDropsTheRowAndClosesTheOpenConversation() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        store.dispatch(RemoteSessionIntent.DeleteSession("s-code"))
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("s-cowork", "s-agentic"), ready.sessions.map { it.id })
        assertNull(ready.selectedSessionId)
        assertNull(ready.timeline)
        assertEquals("s-code", transport.commands.first { it.cmd == "delete_session" }.sessionId)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun renameSessionUpdatesTheRowInPlace() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteSessionIntent.RenameSession("s-code", "  Parser work  "))
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("Parser work", ready.sessions.first { it.id == "s-code" }.title)
        assertEquals("Parser work", transport.commands.first { it.cmd == "update_session_title" }.title)
    }

    @Test
    fun answerQuestionSendsBothSpellingsTheDesktopForwards() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteSessionIntent.AnswerQuestion("s-code", "tool-1", "yes"))
        advanceUntilIdle()

        val answer = transport.commands.first { it.cmd == "answer_question" }
        assertEquals("tool-1", answer.toolId)
        assertEquals("""{"answer":"yes","0":"yes"}""", answer.answers.toString())
    }

    @Test
    fun theDesktopsOwnRejectionReachesTheScreen() = runTest {
        val transport = FakeSessionTransport()
        transport.rejection = "Session is busy running a turn"
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val failed = assertIs<RemoteSessionUiState.Failed>(store.state.value)
        assertEquals(RemoteSessionFailureReason.REMOTE_REJECTED, failed.reason)
        // Written by the peer, so it is shown verbatim rather than translated.
        assertEquals("Session is busy running a turn", failed.remoteMessage)
    }

    @Test
    fun aTimeoutIsNotReportedAsAGenericTransportFailure() = runTest {
        val transport = FakeSessionTransport()
        transport.failure = RelayFailure.Timeout
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val failed = assertIs<RemoteSessionUiState.Failed>(store.state.value)
        assertEquals(RemoteSessionFailureReason.TIMEOUT, failed.reason)
        assertNull(failed.remoteMessage)
    }

    @Test
    fun aSessionStillOpensWhenItsPermissionModeCannotBeRead() = runTest {
        val transport = FakeSessionTransport()
        transport.permissionFailure = RelayFailure.Timeout
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        // The transcript loaded; only the settings section is missing, and it
        // says so rather than taking the session down with it.
        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("s-code", ready.selectedSessionId)
        assertNull(ready.permissionMode)
        assertEquals(PermissionModeFailure.LOAD, ready.permissionModeFailure)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun aDroppedPollKeepsTheTranscriptAndTheNextResponseRestoresTheConnection() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        assertEquals(ConnectionPhase.CONNECTED, store.connectionPhase.value)
        val beforeDrop = assertIs<RemoteSessionUiState.Ready>(store.state.value)

        transport.pollFailure = RelayFailure.NetworkUnreachable
        advanceTimeBy(10_000)
        runCurrent()

        val reconnecting = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(beforeDrop.selectedSessionId, reconnecting.selectedSessionId)
        assertEquals(beforeDrop.timeline?.sessionId, reconnecting.timeline?.sessionId)
        assertEquals(ConnectionPhase.RECONNECTING, store.connectionPhase.value)

        transport.pollFailure = null
        advanceTimeBy(10_000)
        runCurrent()

        assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(ConnectionPhase.CONNECTED, store.connectionPhase.value)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun refreshRetriesThePermissionModeAlone() = runTest {
        val transport = FakeSessionTransport()
        transport.permissionFailure = RelayFailure.Timeout
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        transport.permissionFailure = null
        transport.commands.clear()
        store.dispatch(RemoteSessionIntent.RefreshPermissionMode)
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(SessionPermissionMode.ASK, ready.permissionMode)
        assertNull(ready.permissionModeFailure)
        // The transcript is already on screen, so nothing re-fetches it.
        assertTrue(transport.commands.any { it.cmd == "get_permission_mode" })
        assertTrue(transport.commands.none { it.cmd == "get_session_messages" })
        store.dispatch(RemoteSessionIntent.Stop)
    }

    /**
     * The poll's last word on a turn is that it finished, and the store holds the
     * finished turn on screen until its text arrives as a stored message. The
     * poll never sends that message — from its side nothing changed after the
     * turn ended — so without the re-read the transcript reads correctly while
     * the composer keeps offering Stop for a turn that is over.
     */
    @Test
    fun theTranscriptIsReReadOnceTheTurnEnds() = runTest {
        val transport = FakeSessionTransport()
        transport.polls = listOf(
            """{"resp":"ok","version":1,"changed":true,"session_state":"running",
               "active_turn":{"turn_id":"t-1","status":"active","text":"All "}}""",
            """{"resp":"ok","version":2,"changed":true,"session_state":"idle",
               "active_turn":{"turn_id":"t-1","status":"completed","text":"All done"}}""",
            """{"resp":"ok","version":2,"changed":false,"session_state":"idle"}""",
        )
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        // The desktop stores the turn as a message only after it ends, which is
        // what the second read is for.
        transport.messages = """[{"id":"m-1","role":"assistant","content":"All done"}]"""
        // Past the active interval that carries the turn's end, and past the
        // settle interval that follows it, so the poll after the re-read is out.
        advanceTimeBy(1_000)
        runCurrent()
        store.dispatch(RemoteSessionIntent.Stop)

        assertTrue(transport.commands.count { it.cmd == "get_session_messages" } >= 2)
        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertNull(ready.timeline?.activeTurn)
        // The re-read replaced the transcript wholesale, so the next poll is
        // asked to describe everything rather than a delta from a spent version.
        assertEquals(0, transport.commands.last { it.cmd == "poll_session" }.sinceVersion)
    }

    @Test
    fun aRefusedPermissionChangeLeavesTheSessionStanding() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        transport.permissionFailure = RelayFailure.Timeout
        store.dispatch(RemoteSessionIntent.SetPermissionMode(SessionPermissionMode.FULL_ACCESS))
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        // The mode shown is still the desktop's, not the one we failed to set.
        assertEquals(SessionPermissionMode.ASK, ready.permissionMode)
        assertEquals(PermissionModeFailure.SAVE, ready.permissionModeFailure)
        assertEquals(false, ready.busy)
        store.dispatch(RemoteSessionIntent.Stop)
    }
}

private class FakeSessionTransport : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    var workspacePath: String = "/repo"

    /** When set, the permission commands fail while everything else works. */
    var permissionFailure: RelayFailure? = null

    /** When set, `list_sessions` serves one row per offset so paging is observable. */
    var paged: Boolean = false

    /** When set, `list_sessions` is refused the way a desktop refuses it. */
    var rejection: String? = null

    /** When set, `list_sessions` fails below the desktop instead. */
    var failure: RelayFailure? = null

    /** When set, the open conversation's health poll fails below the desktop. */
    var pollFailure: RelayFailure? = null

    /** Poll payloads served in order; the last one repeats, as a quiet desktop does. */
    var polls: List<String> = listOf(IDLE_POLL)
    private var pollIndex = 0

    /** What `get_session_messages` is holding right now, re-read on every call. */
    var messages: String = "[]"

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        commands += command
        if (command.cmd == "list_sessions") {
            rejection?.let { throw RelayTransportException(RelayFailure.RemoteRejected(it)) }
            failure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "poll_session") {
            pollFailure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "get_permission_mode" || command.cmd == "set_permission_mode") {
            permissionFailure?.let { throw RelayTransportException(it) }
        }
        val json = when (command.cmd) {
            "get_workspace_info" ->
                """{"resp":"ok","has_workspace":${workspacePath.isNotEmpty()},"path":"$workspacePath"}"""
            "list_sessions" -> if (paged) pagedSessions(command.offset ?: 0) else allSessions()
            "get_session_messages" -> """{"resp":"ok","messages":$messages,"has_more":false}"""
            "get_permission_mode" -> """{"resp":"ok","mode":"ask"}"""
            "poll_session" -> polls[minOf(pollIndex++, polls.lastIndex)]
            "create_session" -> """{"resp":"ok","session_id":"s-new"}"""
            "send_message" -> """{"resp":"ok","turn_id":"t-1"}"""
            "delete_session", "update_session_title", "answer_question", "set_permission_mode" ->
                """{"resp":"ok"}"""
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }

    private fun allSessions(): String = """
        {"resp":"ok","has_more":false,"sessions":[
          {"id":"s-code","title":"Code","agent_type":"code"},
          {"id":"s-cowork","title":"Cowork","agent_type":"cowork"},
          {"id":"s-agentic","title":"Legacy","agent_type":"agentic"}
        ]}
    """.trimIndent()

    private fun pagedSessions(offset: Int): String =
        """{"resp":"ok","has_more":${offset < 1},"sessions":[{"id":"page-$offset","title":"Page $offset","agent_type":"code"}]}"""

    private companion object {
        const val IDLE_POLL = """{"resp":"ok","version":1,"changed":false,"session_state":"idle"}"""
    }
}
