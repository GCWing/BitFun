package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.serialization.DeserializationStrategy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.test.assertContentEquals

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
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler), "device-a")
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs#L2", "main.rs", "session-1", "ios-preview-1"))
        advanceUntilIdle()
        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val preview = assertIs<RemoteFilePreviewUiState.Text>(ready.preview)
        assertEquals("ios-preview-1", preview.identity.requestId)
        assertEquals("device-a", preview.identity.deviceKey)
        assertEquals("session-1", preview.identity.sessionId)
        assertEquals("src/main.rs", preview.identity.path)
        assertEquals("fn main() {}", preview.content)
        assertFalse(preview.truncated)
        val read = transport.commands.first { it.cmd == "read_file_chunk" }
        assertEquals("src/main.rs", read.path)
        assertEquals("session-1", read.sessionId)
        assertEquals(16, read.limit)
    }

    /**
     * The header line under the file name says what the file is and how big it
     * is, and the truncation banner says how much of it arrived. None of that
     * survives unless the read carries it out of the store, so this asserts the
     * three numbers rather than the rendering that consumes them.
     */
    @Test
    fun aTextPreviewCarriesTheTypeSizeAndHowMuchArrived() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs", "main.rs", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val preview = assertIs<RemoteFilePreviewUiState.Text>(ready.preview)
        assertEquals("text/plain", preview.mimeType)
        assertEquals(12, preview.sizeBytes)
        assertEquals(12, preview.loadedBytes)
    }

    /**
     * A refusal that arrives before `get_file_info` answers has no type or size
     * to show; the header falls back to the path rather than inventing one.
     */
    @Test
    fun aFailureBeforeTheFileInfoAnswersCarriesNoMetadata() = runTest {
        val transport = FakeWorkspaceTransport()
        transport.fileInfoError = "path is outside workspace"
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs", "main.rs", "session-1", "failed-preview"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val failed = assertIs<RemoteFilePreviewUiState.Failed>(ready.preview)
        assertEquals("failed-preview", failed.identity.requestId)
        assertEquals("", failed.mimeType)
        assertEquals(0, failed.sizeBytes)
    }

    /**
     * The desktop reports Markdown as `text/plain`, so the name is the only
     * thing that can ask for the rendered body instead of numbered source.
     */
    @Test
    fun aMarkdownFileAsksForTheRenderedBody() = runTest {
        val transport = FakeWorkspaceTransport()
        transport.fileName = "README.md"
        transport.chunkBase64 = "IyBUaXRsZQoKQm9keSB0ZXh0Lg=="
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://README.md", "README.md", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val preview = assertIs<RemoteFilePreviewUiState.Text>(ready.preview)
        assertTrue(preview.markdown)
        assertEquals("# Title\n\nBody text.", preview.content)
    }

    @Test
    fun aSourceFileIsShownAsSourceRatherThanRendered() = runTest {
        val transport = FakeWorkspaceTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs", "main.rs", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        assertFalse(assertIs<RemoteFilePreviewUiState.Text>(ready.preview).markdown)
    }

    /**
     * A desktop that calls an ELF binary `text/plain` gets the unsupported
     * body rather than a screen of replacement characters.
     */
    @Test
    fun bytesThatAreNotTextAreRefusedEvenWhenTheTypeSaysTheyAre() = runTest {
        val transport = FakeWorkspaceTransport()
        transport.chunkBase64 = "f0VMRgABAgM="
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://src/main.rs", "main.rs", "session-1", "unsupported-preview"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val unsupported = assertIs<RemoteFilePreviewUiState.Unsupported>(ready.preview)
        assertEquals("unsupported-preview", unsupported.identity.requestId)
        assertEquals("text/plain", unsupported.mimeType)
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

    @Test
    fun samePathRapidReopenRejectsFirstLateResponse() = runTest {
        val transport = DelayedPreviewTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler), "device-a")
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://same.txt", "same.txt", "session-1", "reused-request"))
        runCurrent()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://same.txt", "same.txt", "session-1", "reused-request"))
        runCurrent()
        transport.release(1)
        runCurrent()
        transport.release(0)
        runCurrent()
        assertEquals("reused-request", assertIs<RemoteFilePreviewUiState.Text>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).preview).identity.requestId)
    }

    @Test
    fun differentPathLateResponseCannotReplaceCurrentPreview() = runTest {
        val transport = DelayedPreviewTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler), "device-a")
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://old.txt", "old.txt", "session-1", "old"))
        runCurrent()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://new.txt", "new.txt", "session-1", "new"))
        runCurrent()
        transport.release(1)
        runCurrent()
        transport.release(0)
        runCurrent()
        val preview = assertIs<RemoteFilePreviewUiState.Text>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).preview)
        assertEquals("new", preview.identity.requestId)
        assertEquals("new.txt", preview.identity.path)
    }

    @Test
    fun dismissAndStopRejectLatePreviewResponses() = runTest {
        val transport = DelayedPreviewTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler), "device-a")
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://late.txt", "late.txt", "session-1", "dismissed"))
        runCurrent()
        store.dispatch(RemoteWorkspaceIntent.DismissPreview)
        transport.release(0)
        runCurrent()
        assertIs<RemoteFilePreviewUiState.None>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).preview)

        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://stop.txt", "stop.txt", "session-1", "stopped"))
        runCurrent()
        store.stop()
        transport.release(1)
        runCurrent()
        assertIs<RemoteFilePreviewUiState.Loading>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).preview)
    }

    @Test
    fun workspaceLoadInvalidatesLatePreviewResponse() = runTest {
        val transport = DelayedPreviewTransport()
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler), "device-a")
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()
        store.dispatch(RemoteWorkspaceIntent.OpenFile("computer://late.txt", "late.txt", "session-1", "before-load"))
        runCurrent()
        store.dispatch(RemoteWorkspaceIntent.Load)
        runCurrent()
        transport.release(0)
        advanceUntilIdle()
        assertIs<RemoteFilePreviewUiState.None>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).preview)
    }

    @Test
    fun downloadsAFileInChunksAndWaitsForThePlatformSaver() = runTest {
        val transport = FakeWorkspaceTransport(downloadChunks = true)
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteWorkspaceIntent.DownloadFile("computer://src/main.rs", "main.rs", "session-1"))
        advanceUntilIdle()

        val ready = assertIs<RemoteWorkspaceUiState.Ready>(store.state.value)
        val download = assertIs<RemoteFileDownloadUiState.AwaitingSave>(ready.download)
        assertContentEquals("fn main() {}".encodeToByteArray(), download.bytes)
        assertEquals(listOf(0, 6), transport.commands.filter { it.cmd == "read_file_chunk" }.map { it.offset })

        store.dispatch(RemoteWorkspaceIntent.DownloadSaved(download.target.path))
        assertIs<RemoteFileDownloadUiState.Saved>(assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).download)
    }

    @Test
    fun anOlderPeerWithoutChunkReadsFailsLoudly() = runTest {
        val transport = FakeWorkspaceTransport(readFileUnsupported = true)
        val store = RemoteWorkspaceStore.create(this, transport, StandardTestDispatcher(testScheduler))
        store.dispatch(RemoteWorkspaceIntent.Load)
        advanceUntilIdle()

        store.dispatch(RemoteWorkspaceIntent.DownloadFile("computer://src/main.rs", "main.rs", "session-1"))
        advanceUntilIdle()

        val download = assertIs<RemoteFileDownloadUiState.Failed>(
            assertIs<RemoteWorkspaceUiState.Ready>(store.state.value).download,
        )
        assertEquals(FilePreviewFailureKind.LOAD_FAILED, download.kind)
    }
}

private class DelayedPreviewTransport : RemoteCommandTransport {
    private val gates = mutableListOf<CompletableDeferred<Unit>>()
    private var readIndex: Int = 0

    fun release(index: Int) {
        gates[index].complete(Unit)
    }

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        val json = when (command.cmd) {
            "list_recent_workspaces" -> """{"resp":"ok","workspaces":[{"path":"/repo","name":"Repo"}]}"""
            "list_assistants" -> """{"resp":"ok","assistants":[]}"""
            "get_workspace_info" -> """{"resp":"ok","has_workspace":true,"path":"/repo"}"""
            "get_file_info" -> """{"resp":"ok","name":"${command.path}","size":4,"mime_type":"text/plain"}"""
            "read_file_chunk" -> {
                val index = readIndex++
                val gate = CompletableDeferred<Unit>().also(gates::add)
                withContext(NonCancellable) { gate.await() }
                """{"resp":"ok","name":"${command.path}","chunk_base64":"dGV4dA==","offset":0,"chunk_size":4,"total_size":4,"mime_type":"text/plain"}"""
            }
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}

private class FakeWorkspaceTransport(
    private val downloadChunks: Boolean = false,
    private val readFileUnsupported: Boolean = false,
) : RemoteCommandTransport {
    val commands = mutableListOf<RemoteCommand>()
    var fileInfoError: String? = null

    /** What the desktop calls the file, and the bytes it hands back for it. */
    var fileName: String = "main.rs"
    var chunkBase64: String = "Zm4gbWFpbigpIHt9"

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
                """{"resp":"ok","name":"$fileName","size":${if (downloadChunks) 12 else 16},"mime_type":"text/plain"}"""
            }
            "read_file_chunk" -> if (readFileUnsupported) {
                error("unsupported command read_file_chunk")
            } else if (downloadChunks && command.offset == 0) {
                """{"resp":"ok","name":"main.rs","chunk_base64":"Zm4gbWFp","offset":0,"chunk_size":6,"total_size":12,"mime_type":"text/plain"}"""
            } else if (downloadChunks) {
                """{"resp":"ok","name":"main.rs","chunk_base64":"bigpIHt9","offset":6,"chunk_size":6,"total_size":12,"mime_type":"text/plain"}"""
            } else {
                """{"resp":"ok","name":"$fileName","chunk_base64":"$chunkBase64","offset":0,"chunk_size":12,"total_size":12,"mime_type":"text/plain"}"""
            }
            else -> error("Unexpected command ${command.cmd}")
        }
        return RelayJson.decodeFromString(deserializer, json)
    }
}
