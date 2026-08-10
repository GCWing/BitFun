package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.serialization.DeserializationStrategy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs

@OptIn(ExperimentalCoroutinesApi::class)
class RemoteWorkspaceStoreTest {
    @Test
    fun loadsWorkspaceAssistantAndCurrentSelection() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))

        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        assertEquals(listOf("/repo"), ready.workspaces.map { it.path })
        assertEquals(listOf("/assistant"), ready.assistants.map { it.path })
        assertEquals("/repo", ready.selected?.path)
        assertEquals(listOf("list_recent_workspaces", "list_assistants", "get_workspace_info"), transport.commands.map { it.cmd })
    }

    @Test
    fun switchesWorkspaceAndRefreshesSelection() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteWorkspaceIntent.SelectWorkspace(" /next "))
        advanceUntilIdle()

        assertEquals("/next", transport.commands.first { it.cmd == "set_workspace" }.path)
        assertFalse(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).busy)
    }

    @Test
    fun loadsBoundedTextPreviewThroughCommandTransport() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs#L2", "main.rs", "session-1"))
        advanceUntilIdle()
        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val preview = assertIs<RemoteFilePreviewUiState.Text>(ready.preview)
        assertEquals("fn main() {}", preview.content)
        assertFalse(preview.truncated)
        val read = transport.commands.first { it.cmd == "read_file_chunk" }
        assertEquals("src/main.rs", read.path)
        assertEquals("session-1", read.sessionId)
        assertEquals(16, read.limit)
    }

    @Test
    fun anExternalLinkIsNotSomethingRetryingWillOpen() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteWorkspaceIntent.OpenFile("https://example.com/a.rs", "a.rs", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val failed = assertIs<RemoteFilePreviewUiState.Failed>(ready.preview)
        assertEquals(FilePreviewFailureKind.UNAVAILABLE, failed.kind)
        assertFalse(transport.commands.any { it.cmd == "read_file_chunk" })
    }

    /**
     * The desktop's own sentence is classified in the core; only the cause
     * reaches the app, and an out-of-workspace path is not worth a Retry button.
     */
    @Test
    fun theDesktopsRefusalArrivesAsACauseRatherThanItsWording() = runTest {
        val transport = FakeWorkspaceTransport()
        transport.fileInfoError = "path is outside workspace"
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://../secret.env", "secret.env", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val failed = assertIs<RemoteFilePreviewUiState.Failed>(ready.preview)
        assertEquals(FilePreviewFailureKind.ACCESS_DENIED, failed.kind)
        assertFalse(failed.retryable)
    }
}

private class FakeWorkspaceTransport : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    var fileInfoError: String? = null

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        commands += command
        val json = when (command.cmd) {
            "list_recent_workspaces" ->
                """{"resp":"ok","workspaces":[{"path":"/repo","name":"Repo","last_opened":"2026-08-09"}]}"""
            "list_assistants" ->
                """{"resp":"ok","assistants":[{"path":"/assistant","name":"Assistant","assistant_id":"a1"}]}"""
            "get_workspace_info" ->
                """{"resp":"ok","has_workspace":true,"path":"/repo","project_name":"Repo","git_branch":"main"}"""
            "set_workspace" -> """{"resp":"ok","success":true,"path":"${command.path}"}"""
            "set_assistant" -> """{"resp":"ok","success":true,"path":"${command.path}"}"""
            "get_file_info" -> {
                fileInfoError?.let { error(it) }
                """{"resp":"ok","name":"main.rs","size":16,"mime_type":"text/plain"}"""
            }
            "read_file_chunk" ->
                """{"resp":"ok","name":"main.rs","chunk_base64":"Zm4gbWFpbigpIHt9","offset":0,"chunk_size":12,"total_size":12,"mime_type":"text/plain"}"""
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}
