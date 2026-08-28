package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.RemoteSession
import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.transport.RelayFailure
import com.bitfun.mobile.core.transport.RelayTransportException
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
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
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertIs
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class RemoteSessionStoreTest {
    @Test
    fun additiveRevisionConstructorsKeepLegacySourceShape() {
        val succeeded = CreateSessionOperationState.Succeeded("request", "session", null)
        assertEquals(0, succeeded.commitRevision)
        val ready = RemoteSessionUiState.Ready(
            emptyList(), null, null, false, null, null, "", SessionAgentFilter.ALL,
            false, false, null, null, "",
        )
        assertEquals(0, ready.revision)
    }

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
        assertEquals("model-primary", ready.modelCatalog?.defaultModels?.primary)
        assertNull(ready.modelCatalogFailure)
    }

    @Test
    fun modelCatalogRemoteRejectedIsRetryableAndRefreshRecovers() = runTest {
        val transport = FakeSessionTransport()
        transport.modelCatalogFailure = RelayFailure.RemoteRejected("Unknown command")
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val failed = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertNull(failed.modelCatalog)
        // A rejection is not proof of an old peer: a modern desktop can refuse
        // the catalog command transiently, so it stays retryable.
        assertEquals(ModelCatalogFailure.LOAD_FAILED, failed.modelCatalogFailure)
        assertTrue(failed.sessions.isNotEmpty())

        transport.modelCatalogFailure = null
        transport.commands.clear()
        store.dispatch(RemoteSessionIntent.RefreshModelCatalog)
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("model-primary", ready.modelCatalog?.defaultModels?.primary)
        assertNull(ready.modelCatalogFailure)
        assertFalse(ready.busy)
        assertEquals(listOf("get_model_catalog"), transport.commands.map { it.cmd })
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun modelCatalogMalformedResponseIsTypedAsRetryableFailure() = runTest {
        val transport = FakeSessionTransport()
        transport.modelCatalogFailure = RelayFailure.MalformedResponse
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertNull(ready.modelCatalog)
        // Malformed is a local protocol fault, not a peer capability statement.
        assertEquals(ModelCatalogFailure.LOAD_FAILED, ready.modelCatalogFailure)
        assertTrue(ready.sessions.isNotEmpty())
    }

    @Test
    fun modelCatalogNetworkFailureIsTypedWithoutFailingTheSession() = runTest {
        val transport = FakeSessionTransport()
        transport.modelCatalogFailure = RelayFailure.Timeout
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertNull(ready.modelCatalog)
        assertEquals(ModelCatalogFailure.LOAD_FAILED, ready.modelCatalogFailure)
        assertTrue(ready.sessions.isNotEmpty())
    }

    @Test
    fun refreshModelCatalogRecoversFromATransientFailureAndUpdatesTheTimeline() = runTest {
        val transport = FakeSessionTransport()
        transport.modelCatalogFailure = RelayFailure.Timeout
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        assertEquals(
            ModelCatalogFailure.LOAD_FAILED,
            assertIs<RemoteSessionUiState.Ready>(store.state.value).modelCatalogFailure,
        )
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        store.dispatch(RemoteSessionIntent.UpdateDraft("keep-draft"))
        assertEquals("keep-draft", assertIs<RemoteSessionUiState.Ready>(store.state.value).draft)
        assertNull(assertIs<RemoteSessionUiState.Ready>(store.state.value).timeline?.modelCatalog?.defaultModels?.primary)

        transport.modelCatalogFailure = null
        transport.commands.clear()
        store.dispatch(RemoteSessionIntent.RefreshModelCatalog)
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("model-primary", ready.modelCatalog?.defaultModels?.primary)
        assertNull(ready.modelCatalogFailure)
        assertFalse(ready.busy)
        assertEquals("s-code", ready.selectedSessionId)
        assertEquals("model-primary", ready.timeline?.modelCatalog?.defaultModels?.primary)
        assertEquals("keep-draft", ready.draft)
        assertTrue(ready.sessions.isNotEmpty())
        // The refresh is the catalog command alone; it does not re-read the
        // session list or the transcript it must keep on screen.
        assertEquals(listOf("get_model_catalog"), transport.commands.map { it.cmd })
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun modelCatalogContractIsTheStaticSupportedFact() {
        assertEquals("get_model_catalog", ModelCatalogContract.commandName)
        assertEquals(ModelCatalogSupport.SUPPORTED, ModelCatalogContract.support)
    }

    @Test
    fun hidesDesktopOnlyAcpSessions() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("s-code", "s-cowork", "s-agentic"), ready.sessions.map { it.id })
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
    fun initialLoadFollowsServerPagesWithoutRepeatingRows() = runTest {
        val transport = FakeSessionTransport()
        transport.paged = true
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("page-0", "page-1"), ready.sessions.map { it.id })
        assertFalse(ready.hasMore)
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
        // The desktop routes send_message by agent type, so it must match the
        // session that create_session just opened, not fall back to "agentic".
        assertEquals("code", transport.commands.first { it.cmd == "send_message" }.agentType)
        assertEquals("s-new", assertIs<RemoteSessionUiState.Ready>(store.state.value).selectedSessionId)
        val outcome = assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        assertEquals("s-new", outcome.createdSessionId)
        assertEquals("/repo", outcome.confirmedSession?.workspacePath)
        assertTrue(outcome.requestId.isNotBlank())
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun createPreemptsBlockedRefreshAndPublishesRevisionLinkedCommit() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        val beforeRefreshReady = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        val beforeRefresh = beforeRefreshReady.revision

        transport.nonCancellableCommands += "list_sessions"
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()
        val lateRefresh = transport.lateCommandContinuations.remove("list_sessions")!!
        store.dispatch(
            RemoteSessionIntent.CreateSessionOperation(
                "create-preempts-refresh", "code", "", "", null, "/repo",
            ),
        )
        runCurrent()

        val succeeded = assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertTrue(succeeded.commitRevision > beforeRefresh)
        assertTrue(ready.revision >= succeeded.commitRevision)
        assertTrue(ready.sessions.any { it.id == succeeded.createdSessionId })
        assertEquals("s-new", ready.selectedSessionId)

        lateRefresh.resume(Unit)
        runCurrent()
        val afterLateRefresh = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(ready.revision, afterLateRefresh.revision)
        assertEquals(ready.sessions, afterLateRefresh.sessions)
        assertEquals("s-new", afterLateRefresh.selectedSessionId)
        store.stop()
    }

    @Test
    fun staleCommittedCreateCancellationDoesNotClearNewerBusyState() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        transport.commandGates["get_session_messages"] = CompletableDeferred()
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("committed-cancel", "code", "", "", null, "/repo"))
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)

        transport.commandGates["list_sessions"] = CompletableDeferred()
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()

        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).busy)
        transport.commandGates.remove("list_sessions")?.complete(Unit)
        runCurrent()
        store.stop()
    }

    @Test
    fun lateNonCancellableLoadMoreCannotOverwriteNewerRefresh() = runTest {
        val transport = FakeSessionTransport().apply {
            paged = true
            pagedLimit = 40
        }
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        transport.nonCancellableCommands += "list_sessions"
        store.dispatch(RemoteSessionIntent.LoadMore)
        runCurrent()
        val lateLoadMore = transport.lateCommandContinuations.remove("list_sessions")!!
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()
        val refreshed = assertIs<RemoteSessionUiState.Ready>(store.state.value)

        lateLoadMore.resume(Unit)
        runCurrent()
        val afterLatePage = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(refreshed.revision, afterLatePage.revision)
        assertEquals(refreshed.sessions, afterLatePage.sessions)
        store.stop()
    }

    @Test
    fun staleLoadMoreCannotReconcileAwayLocallyCreatedSession() = runTest {
        val transport = FakeSessionTransport().apply {
            paged = true
            pagedLimit = 40
        }
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.reconcileConfirmedCreatedSession(RemoteSession(
            "local-created", "Local", "code", "active", "", "", 0, "/repo", null,
        ))

        transport.listSessionsOverride = {
            """{"resp":"ok","has_more":false,"sessions":[{"id":"local-created","title":"Confirmed","agent_type":"code"}]}"""
        }
        transport.nonCancellableCommands += "list_sessions"
        store.dispatch(RemoteSessionIntent.LoadMore)
        runCurrent()
        val lateLoadMore = transport.lateCommandContinuations.remove("list_sessions")!!

        transport.listSessionsOverride = {
            """{"resp":"ok","has_more":false,"sessions":[{"id":"server-only","title":"Server","agent_type":"code"}]}"""
        }
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()
        lateLoadMore.resume(Unit)
        runCurrent()
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()

        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.any { it.id == "local-created" })
        store.stop()
    }

    @Test
    fun staleDeleteCannotResetNewlyOpenedTimeline() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        transport.nonCancellableCommands += "delete_session"
        store.dispatch(RemoteSessionIntent.DeleteSession("s-code"))
        runCurrent()
        val lateDelete = transport.lateCommandContinuations.remove("delete_session")!!
        store.dispatch(RemoteSessionIntent.Open("s-cowork"))
        runCurrent()
        lateDelete.resume(Unit)
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("s-cowork", ready.selectedSessionId)
        assertEquals("s-cowork", ready.timeline?.sessionId)
        store.dispatch(RemoteSessionIntent.SendMessage("s-cowork", "still active"))
        runCurrent()
        assertTrue(transport.commands.any { it.cmd == "send_message" && it.sessionId == "s-cowork" })
        store.stop()
    }

    @Test
    fun createTransportFailureIsTypedAndRetryable() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        transport.createFailure = RelayFailure.Timeout

        store.dispatch(
            RemoteSessionIntent.CreateSessionOperation(
                requestId = "request-timeout",
                agentType = "code",
                title = "",
                instruction = "",
                modelId = null,
            ),
        )
        advanceUntilIdle()

        val failed = assertIs<CreateSessionOperationState.Failed>(store.createOperation.value)
        assertEquals("request-timeout", failed.requestId)
        assertEquals(CreateSessionOperationFailure.TRANSPORT, failed.reason)
        assertTrue(failed.retryable)
        assertFalse(failed.unsupported)
    }

    @Test
    fun malformedCreateResponseIsUnsupported() = runTest {
        val transport = FakeSessionTransport()
        transport.createFailure = RelayFailure.MalformedResponse
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("malformed-create", "code", "", "", null))
        runCurrent()
        val failed = assertIs<CreateSessionOperationState.Failed>(store.createOperation.value)
        assertEquals(CreateSessionOperationFailure.UNSUPPORTED, failed.reason)
        assertTrue(failed.unsupported)
    }

    @Test
    fun rejectedCreateIsNotUnsupported() = runTest {
        val transport = FakeSessionTransport()
        transport.createFailure = RelayFailure.RemoteRejected("denied")
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("rejected-create", "code", "", "", null))
        runCurrent()
        val failed = assertIs<CreateSessionOperationState.Failed>(store.createOperation.value)
        assertEquals(CreateSessionOperationFailure.TRANSPORT, failed.reason)
        assertFalse(failed.unsupported)
    }

    @Test
    fun repeatedCreateRequestIdUsesLatestInternalGeneration() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        transport.createFailure = RelayFailure.RemoteRejected("first")
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("reused", "code", "", "", null))
        runCurrent()
        assertIs<CreateSessionOperationState.Failed>(store.createOperation.value)
        transport.createFailure = null
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("reused", "code", "", "", null))
        runCurrent()
        val succeeded = assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        assertEquals("reused", succeeded.requestId)
        assertEquals("s-new", succeeded.createdSessionId)
        store.stop()
    }

    @Test
    fun stopWhileModelInitializationIsGatedKeepsCommittedCreateSucceeded() = runTest {
        val transport = FakeSessionTransport()
        transport.commandGates["set_session_model"] = CompletableDeferred()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("model-gate", "code", "", "", "model-primary", "/repo"))
        runCurrent()

        val committed = assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        assertEquals("s-new", committed.createdSessionId)
        val committedReady = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(committed.commitRevision, committedReady.revision)
        assertTrue(committedReady.sessions.any { it.id == committed.createdSessionId })
        store.stop()
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
    }

    @Test
    fun stopWhileOpenInitializationIsGatedKeepsCommittedProjection() = runTest {
        val transport = FakeSessionTransport()
        transport.commandGates["get_session_messages"] = CompletableDeferred()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("open-gate", "code", "", "", null, "/repo"))
        runCurrent()

        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        assertEquals(listOf("s-new"), assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.map { it.id })
        store.stop()
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
    }

    @Test
    fun stopWhileInitialMessageIsGatedKeepsCommittedCreateSucceeded() = runTest {
        val transport = FakeSessionTransport()
        transport.commandGates["send_message"] = CompletableDeferred()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("send-gate", "code", "", "hello", null, "/repo"))
        runCurrent()

        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        store.stop()
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
    }

    @Test
    fun postCreateInitializationFailureDoesNotRollBackSucceededOutcome() = runTest {
        val transport = FakeSessionTransport().apply { sendMessageFailure = RelayFailure.Timeout }
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("send-fails", "code", "", "hello", null, "/repo"))
        runCurrent()

        val succeeded = assertIs<CreateSessionOperationState.Succeeded>(store.createOperation.value)
        assertEquals("s-new", succeeded.createdSessionId)
        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.any { it.id == "s-new" })
        store.stop()
    }

    @Test
    fun sessionStoreStopCancelsCreateOperation() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.CreateSessionOperation("stop-create", "code", "", "", null))
        store.stop()
        assertIs<CreateSessionOperationState.Cancelled>(store.createOperation.value)
    }

    @Test
    fun ordinaryOpenAndWorkspaceSelectionDoNotChangeCreateOperation() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport()
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")
        session.dispatch(RemoteSessionIntent.CreateSessionOperation("stable-create", "code", "", "", null))
        runCurrent()
        val succeeded = assertIs<CreateSessionOperationState.Succeeded>(session.createOperation.value)

        session.dispatch(RemoteSessionIntent.Open("s-code"))
        workspace.dispatch(RemoteWorkspaceIntent.Load)
        runCurrent()
        workspace.dispatch(RemoteWorkspaceIntent.SelectAssistant("/assistant"))
        runCurrent()

        assertEquals(succeeded, session.createOperation.value)
        session.stop()
    }

    @Test
    fun assistantCreateLoadsIdleSelectsAssistantAndCreates() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport()
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")

        session.createAssistantSession(workspace, "assistant-1", "/assistant", "", "", null)
        runCurrent()

        assertIs<CreateSessionOperationState.Succeeded>(session.createOperation.value)
        assertEquals(listOf("list_recent_workspaces", "list_assistants", "get_workspace_info", "set_assistant", "get_workspace_info"), workspaceTransport.commands.map { it.cmd })
        assertTrue(sessionTransport.commands.any { it.cmd == "create_session" && it.workspacePath == "/assistant" })
        session.stop()
    }

    @Test
    fun assistantCreateLoadFailureIsTypedWithoutSelecting() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport(loadFailure = true)
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")

        session.createAssistantSession(workspace, "assistant-load-fail", "/assistant", "", "", null)
        advanceUntilIdle()

        assertEquals(CreateSessionOperationFailure.WORKSPACE, assertIs<CreateSessionOperationState.Failed>(session.createOperation.value).reason)
        assertTrue(workspaceTransport.commands.none { it.cmd == "set_assistant" })
        assertTrue(sessionTransport.commands.none { it.cmd == "create_session" })
    }

    @Test
    fun unknownAssistantFailsImmediatelyWithoutSelection() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport()
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")
        workspace.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        workspaceTransport.commands.clear()

        session.createAssistantSession(workspace, "assistant-unknown", "/missing", "", "", null)
        runCurrent()

        assertEquals(CreateSessionOperationFailure.WORKSPACE, assertIs<CreateSessionOperationState.Failed>(session.createOperation.value).reason)
        assertTrue(workspaceTransport.commands.isEmpty())
        assertTrue(sessionTransport.commands.none { it.cmd == "create_session" })
    }

    @Test
    fun assistantCreateWaitsWhileSameSelectionIsBusy() = runTest {
        val gate = CompletableDeferred<Unit>()
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport(selectionGate = gate, initialSelectedPath = "/assistant")
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")
        workspace.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        workspace.dispatch(RemoteWorkspaceIntent.SelectAssistant("/assistant"))
        runCurrent()

        session.createAssistantSession(workspace, "assistant-busy", "/assistant", "", "", null)
        runCurrent()
        assertIs<CreateSessionOperationState.InFlight>(session.createOperation.value)
        assertTrue(assertIs<RemoteWorkspaceUiState.Ready>(workspace.state.value).busy)
        assertTrue(sessionTransport.commands.none { it.cmd == "create_session" })

        gate.complete(Unit)
        runCurrent()
        assertIs<CreateSessionOperationState.Succeeded>(session.createOperation.value)
        session.stop()
    }

    @Test
    fun assistantSelectionFailureNeverCreatesInOldWorkspace() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport(selectionFailure = true)
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")

        session.createAssistantSession(workspace, "assistant-select-fail", "/assistant", "", "", null)
        advanceUntilIdle()

        assertEquals(CreateSessionOperationFailure.WORKSPACE, assertIs<CreateSessionOperationState.Failed>(session.createOperation.value).reason)
        assertTrue(sessionTransport.commands.none { it.cmd == "create_session" })
    }

    @Test
    fun assistantDeviceMismatchSendsNoCommands() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport()
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-b")

        session.createAssistantSession(workspace, "assistant-mismatch", "/assistant", "", "", null)

        assertEquals(CreateSessionOperationFailure.DEVICE_MISMATCH, assertIs<CreateSessionOperationState.Failed>(session.createOperation.value).reason)
        assertTrue(workspaceTransport.commands.isEmpty())
        assertTrue(sessionTransport.commands.isEmpty())
    }

    @Test
    fun workspaceStoppedBeforeAssistantCoroutineRunsCancelsWithoutCommands() = runTest {
        val sessionTransport = FakeSessionTransport()
        val workspaceTransport = AssistantWorkspaceTransport()
        val session = RemoteSessionStore.create(this, sessionTransport, "device-a", null)
        val workspace = RemoteWorkspaceStore.create(this, workspaceTransport, StandardTestDispatcher(testScheduler), "device-a")

        session.createAssistantSession(workspace, "assistant-stop", "/assistant", "", "", null)
        workspace.stop()
        runCurrent()

        assertIs<CreateSessionOperationState.Cancelled>(session.createOperation.value)
        assertTrue(workspaceTransport.commands.isEmpty())
        assertTrue(sessionTransport.commands.none { it.cmd == "create_session" })
    }

    @Test
    fun loadOlderMessagesPrependsThePreviousTranscriptPage() = runTest {
        val transport = FakeSessionTransport()
        transport.messages = """[{"id":"m-new","role":"assistant","content":"new"}]"""
        transport.olderMessages = """[{"id":"m-old","role":"user","content":"old"}]"""
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).hasMoreMessages)

        store.dispatch(RemoteSessionIntent.LoadOlderMessages)
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(listOf("m-old", "m-new"), ready.timeline?.persistedMessages?.map { it.id })
        assertEquals(false, ready.hasMoreMessages)
        val request = transport.commands.last { it.cmd == "get_session_messages" }
        assertEquals("m-new", request.beforeMessageId)
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
    fun crossWorkspaceCreateDoesNotSwitchTheDesktopAndStaysProjected() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        transport.commands.clear()

        store.dispatch(
            RemoteSessionIntent.CreateSession(
                agentType = "code",
                title = "",
                instruction = "",
                modelId = null,
                workspacePath = "/other",
            ),
        )
        runCurrent()

        assertTrue(transport.commands.none { it.cmd == "get_workspace_info" })
        assertEquals("/other", transport.commands.first { it.cmd == "create_session" }.workspacePath)
        val created = assertIs<RemoteSessionUiState.Ready>(store.state.value)
            .sessions.first { it.id == "s-new" }
        assertEquals("/other", created.workspacePath)
        store.dispatch(RemoteSessionIntent.Refresh)
        runCurrent()
        assertTrue(assertIs<RemoteSessionUiState.Ready>(store.state.value).sessions.any { it.id == "s-new" })
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
    fun structuredQuestionAnswersUseIndexedTextAndChoiceValues() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()

        store.dispatch(
            RemoteSessionIntent.AnswerStructuredQuestion(
                "s-code",
                "tool-1",
                listOf(
                    QuestionAnswer(0, QuestionAnswerValue.Text("yes")),
                    QuestionAnswer(1, QuestionAnswerValue.Choice(listOf("a", "b"))),
                ),
            ),
        )
        advanceUntilIdle()

        val answer = transport.commands.first { it.cmd == "answer_question" }
        assertEquals("tool-1", answer.toolId)
        assertEquals("""{"0":"yes","1":["a","b"]}""", answer.answers.toString())
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
    fun unknownPermissionModeSurfacesAsUnknownNotAsk() = runTest {
        val transport = FakeSessionTransport()
        transport.permissionModeJson = """{"resp":"ok","mode":"future_mode"}"""
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(SessionPermissionMode.UNKNOWN, ready.permissionMode)
        assertNull(ready.permissionModeFailure)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun missingPermissionModeSurfacesAsUnknown() = runTest {
        val transport = FakeSessionTransport()
        transport.permissionModeJson = """{"resp":"ok"}"""
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals(SessionPermissionMode.UNKNOWN, ready.permissionMode)
        assertNull(ready.permissionModeFailure)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun editedToolApprovalIsGatedUnsupportedWithoutSending() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        store.dispatch(
            RemoteSessionIntent.ApproveTool("s-code", "tool-1", updatedInput = """{"x":1}"""),
        )
        runCurrent()

        assertTrue(transport.commands.none { it.cmd == "confirm_tool" })
        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertFalse(ready.busy)
        assertEquals(ToolApprovalEditSupport.UNSUPPORTED, ToolApprovalEditContract.support)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun plainToolApprovalStillSendsConfirmTool() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)

        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()
        store.dispatch(RemoteSessionIntent.ApproveTool("s-code", "tool-1"))
        runCurrent()

        val approval = transport.commands.last { it.cmd == "confirm_tool" }
        assertEquals("tool-1", approval.toolId)
        store.dispatch(RemoteSessionIntent.Stop)
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
    fun sendMessageCarriesTheSessionsAgentType() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        store.dispatch(RemoteSessionIntent.SendMessage("s-code", "hello"))
        runCurrent()

        val sent = transport.commands.last { it.cmd == "send_message" }
        assertEquals("s-code", sent.sessionId)
        assertEquals("hello", sent.content)
        // Matches the session opened above; without it the desktop defaults to
        // "agentic" and rejects a turn for a differently typed session.
        assertEquals("code", sent.agentType)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun sendMessageFallsBackToTheLocallyCreatedRecordWhenTheFilterHidesTheSession() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteSessionIntent.SetAgentFilter(SessionAgentFilter.CODE))
        advanceUntilIdle()

        // Cowork is not visible in the Code tab, so after a no-instruction
        // create the session stays selected but is filtered out of
        // Ready.sessions. send_message must still resolve its agent type from
        // the locally created record instead of omitting agent_type.
        store.dispatch(RemoteSessionIntent.CreateSession("cowork", "", "", null))
        runCurrent()
        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("s-new", ready.selectedSessionId)
        assertTrue(ready.sessions.none { it.id == "s-new" })

        store.dispatch(RemoteSessionIntent.SendMessage("s-new", "hello"))
        runCurrent()

        val sent = transport.commands.last { it.cmd == "send_message" }
        assertEquals("s-new", sent.sessionId)
        // The desktop normalizes cowork/Cowork case on its side; the store must
        // simply forward the created session's agent type instead of null.
        assertEquals("cowork", sent.agentType)
        store.dispatch(RemoteSessionIntent.Stop)
    }

    @Test
    fun sendFailureKeepsTheDraftTheComposerWasAboutToSend() = runTest {
        val transport = FakeSessionTransport()
        val store = RemoteSessionStore.create(this, transport)
        store.dispatch(RemoteSessionIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteSessionIntent.Open("s-code"))
        runCurrent()

        store.dispatch(RemoteSessionIntent.UpdateDraft("keep me"))
        transport.sendMessageFailure = RelayFailure.NetworkUnreachable
        store.dispatch(RemoteSessionIntent.SendMessage("s-code", "keep me"))
        runCurrent()

        val ready = assertIs<RemoteSessionUiState.Ready>(store.state.value)
        assertEquals("keep me", ready.draft)
        assertEquals(false, ready.busy)
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
        assertNull(ready.timeline?.activeTurn, ready.timeline.toString())
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

private class AssistantWorkspaceTransport(
    private val loadFailure: Boolean = false,
    private val selectionFailure: Boolean = false,
    val selectionGate: CompletableDeferred<Unit>? = null,
    initialSelectedPath: String = "/repo",
) : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    private var selectedPath: String = initialSelectedPath

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        commands += command
        if (loadFailure && command.cmd == "list_recent_workspaces") error("load failed")
        if (selectionFailure && command.cmd == "set_assistant") error("selection failed")
        val json = when (command.cmd) {
            "list_recent_workspaces" -> """{"resp":"ok","workspaces":[{"path":"/repo","name":"Repo"}]}"""
            "list_assistants" -> """{"resp":"ok","assistants":[{"path":"/assistant","name":"Assistant","assistant_id":"a1"}]}"""
            "set_assistant" -> {
                selectionGate?.await()
                selectedPath = command.path.orEmpty()
                """{"resp":"ok","success":true,"path":"$selectedPath"}"""
            }
            "get_workspace_info" -> """{"resp":"ok","has_workspace":true,"path":"$selectedPath","workspace_kind":"${if (selectedPath == "/assistant") "assistant" else "code"}"}"""
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}

private class FakeSessionTransport : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    var workspacePath: String = "/repo"

    /** When set, the permission commands fail while everything else works. */
    var permissionFailure: RelayFailure? = null

    var permissionModeJson: String? = null

    /** When set, `list_sessions` serves one row per offset so paging is observable. */
    var paged: Boolean = false
    var pagedLimit: Int = 1
    var listSessionsOverride: ((RemoteCommand) -> String)? = null

    /** When set, `list_sessions` is refused the way a desktop refuses it. */
    var rejection: String? = null

    /** When set, `list_sessions` fails below the desktop instead. */
    var failure: RelayFailure? = null

    /** When set, `get_model_catalog` fails with the selected transport result. */
    var modelCatalogFailure: RelayFailure? = null

    /** When set, the open conversation's health poll fails below the desktop. */
    var pollFailure: RelayFailure? = null

    /** When set, `send_message` fails below the desktop while the draft is kept. */
    var sendMessageFailure: RelayFailure? = null

    /** When set, `create_session` fails below the desktop. */
    var createFailure: RelayFailure? = null

    /** Optional command-stage gates used to exercise post-create cancellation races. */
    val commandGates = mutableMapOf<String, CompletableDeferred<Unit>>()

    /** Commands suspended by a primitive continuation, so cancellation cannot consume their late result. */
    val nonCancellableCommands = mutableSetOf<String>()
    val lateCommandContinuations = mutableMapOf<String, Continuation<Unit>>()

    /** Poll payloads served in order; the last one repeats, as a quiet desktop does. */
    var polls: List<String> = listOf(IDLE_POLL)
    private var pollIndex = 0

    /** What `get_session_messages` is holding right now, re-read on every call. */
    var messages: String = "[]"

    /** Optional previous page, served only for a cursor-bearing message request. */
    var olderMessages: String? = null

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        commands += command
        val preparedListSessions = if (command.cmd == "list_sessions") {
            listSessionsOverride?.invoke(command)
        } else {
            null
        }
        commandGates[command.cmd]?.await()
        if (nonCancellableCommands.remove(command.cmd)) {
            suspendCoroutine { continuation -> lateCommandContinuations[command.cmd] = continuation }
        }
        if (command.cmd == "list_sessions") {
            rejection?.let { throw RelayTransportException(RelayFailure.RemoteRejected(it)) }
            failure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "poll_session") {
            pollFailure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "send_message") {
            sendMessageFailure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "create_session") {
            createFailure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "get_permission_mode" || command.cmd == "set_permission_mode") {
            permissionFailure?.let { throw RelayTransportException(it) }
        }
        if (command.cmd == "get_model_catalog") {
            modelCatalogFailure?.let { throw RelayTransportException(it) }
        }
        val json = when (command.cmd) {
            "get_workspace_info" ->
                """{"resp":"ok","has_workspace":${workspacePath.isNotEmpty()},"path":"$workspacePath"}"""
            "get_model_catalog" -> """{
                "resp":"ok",
                "catalog":{
                  "version":7,
                  "models":[{
                    "id":"model-primary","name":"Primary","provider":"account",
                    "base_url":"","model_name":"primary","enabled":true
                  }],
                  "default_models":{"primary":"model-primary"}
                }
            }""".trimIndent()
            "list_sessions" -> preparedListSessions ?: if (paged) pagedSessions(command.offset ?: 0) else allSessions()
            "get_session_messages" -> if (command.beforeMessageId != null) {
                """{"resp":"ok","messages":${olderMessages ?: "[]"},"has_more":false}"""
            } else {
                """{"resp":"ok","messages":$messages,"has_more":${olderMessages != null}}"""
            }
            "get_permission_mode" -> permissionModeJson ?: """{"resp":"ok","mode":"ask"}"""
            "poll_session" -> polls[minOf(pollIndex++, polls.lastIndex)]
            "create_session" -> """{"resp":"ok","session_id":"s-new"}"""
            "set_session_model" -> """{"resp":"ok","model_id":"model-primary"}"""
            "send_message" -> """{"resp":"ok","turn_id":"t-1"}"""
            "delete_session", "update_session_title", "answer_question", "set_permission_mode", "confirm_tool" ->
                """{"resp":"ok"}"""
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }

    private fun allSessions(): String = """
        {"resp":"ok","has_more":false,"sessions":[
          {"id":"s-code","title":"Code","agent_type":"code"},
          {"id":"s-cowork","title":"Cowork","agent_type":"cowork"},
          {"id":"s-agentic","title":"Legacy","agent_type":"agentic"},
          {"id":"s-acp","title":"Desktop ACP","agent_type":"acp:codex"}
        ]}
    """.trimIndent()

    private fun pagedSessions(offset: Int): String =
        """{"resp":"ok","has_more":${offset < pagedLimit},"sessions":[{"id":"page-$offset","title":"Page $offset","agent_type":"code"}]}"""

    private companion object {
        const val IDLE_POLL = """{"resp":"ok","version":1,"changed":false,"session_state":"idle"}"""
    }
}
