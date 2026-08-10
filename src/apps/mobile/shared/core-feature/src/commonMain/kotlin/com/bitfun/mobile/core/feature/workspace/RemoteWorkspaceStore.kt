package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.domain.FilePreviewFailure
import com.bitfun.mobile.core.domain.FilePreviewFailureReason
import com.bitfun.mobile.core.domain.FilePreviewPolicy
import com.bitfun.mobile.core.domain.FilePreviewTarget
import com.bitfun.mobile.core.domain.FilePreviewTargetContext
import com.bitfun.mobile.core.domain.FileReferenceKind
import com.bitfun.mobile.core.domain.FileTargetResolver
import com.bitfun.mobile.core.domain.RecentWorkspace
import com.bitfun.mobile.core.domain.SelectedWorkspace
import com.bitfun.mobile.core.domain.WorkspaceAssistant
import com.bitfun.mobile.core.protocol.AssistantListResponse
import com.bitfun.mobile.core.protocol.FileInfoResponse
import com.bitfun.mobile.core.protocol.ReadFileChunkResponse
import com.bitfun.mobile.core.protocol.RecentWorkspaceListResponse
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.protocol.SetAssistantResponse
import com.bitfun.mobile.core.protocol.SetWorkspaceResponse
import com.bitfun.mobile.core.protocol.WorkspaceInfoResponse
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import com.bitfun.mobile.core.transport.send
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Job
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlin.io.encoding.Base64

public class RemoteWorkspaceStore internal constructor(
    private val scope: CoroutineScope,
    private val transport: RemoteCommandTransport,
    private val backgroundDispatcher: CoroutineDispatcher,
) {
    private val _state = MutableStateFlow<RemoteWorkspaceUiState>(RemoteWorkspaceUiState.Idle)
    public val state: StateFlow<RemoteWorkspaceUiState> = _state.asStateFlow()
    private var work: Job? = null
    private var targetEpoch: Int = 0

    public fun dispatch(intent: RemoteWorkspaceIntent) {
        when (intent) {
            RemoteWorkspaceIntent.Load -> load()
            is RemoteWorkspaceIntent.SelectWorkspace -> selectWorkspace(intent.path)
            is RemoteWorkspaceIntent.SelectAssistant -> selectAssistant(intent.path)
            is RemoteWorkspaceIntent.OpenFile -> resolveAndOpenFile(intent)
            RemoteWorkspaceIntent.DismissPreview -> updateReady { it.copy(preview = RemoteFilePreviewUiState.None) }
            RemoteWorkspaceIntent.Stop -> stop()
        }
    }

    public fun stop() {
        work?.cancel()
        work = null
    }

    private fun load() {
        work?.cancel()
        _state.value = RemoteWorkspaceUiState.Loading
        work = scope.launch {
            try {
                val recent = transport.send<RecentWorkspaceListResponse>(RemoteCommand(cmd = "list_recent_workspaces"))
                val assistants = transport.send<AssistantListResponse>(RemoteCommand(cmd = "list_assistants"))
                val info = transport.send<WorkspaceInfoResponse>(RemoteCommand(cmd = "get_workspace_info"))
                _state.value = RemoteWorkspaceUiState.Ready(
                    workspaces = recent.workspaces.map { item ->
                        RecentWorkspace(
                            path = item.path.orEmpty(),
                            name = item.name?.takeIf(String::isNotBlank) ?: basename(item.path.orEmpty()),
                            lastOpened = item.lastOpened,
                            kind = item.workspaceKind.orEmpty(),
                        )
                    }.filter { it.path.isNotEmpty() },
                    assistants = assistants.assistants.map { item ->
                        WorkspaceAssistant(item.path, item.name, item.assistantId)
                    },
                    selected = info.asSelectedWorkspace(),
                    preview = RemoteFilePreviewUiState.None,
                    busy = false,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                _state.value = RemoteWorkspaceUiState.Failed(true)
            }
        }
    }

    private fun selectWorkspace(path: String) {
        val normalized = path.trim()
        if (normalized.isEmpty()) return
        runSelection(RemoteCommand(cmd = "set_workspace", path = normalized), false)
    }

    private fun selectAssistant(path: String) {
        val normalized = path.trim()
        if (normalized.isEmpty()) return
        runSelection(RemoteCommand(cmd = "set_assistant", path = normalized), true)
    }

    private fun runSelection(command: RemoteCommand, assistant: Boolean) {
        val current = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        work?.cancel()
        _state.value = current.copy(busy = true)
        work = scope.launch {
            try {
                if (assistant) {
                    transport.send<SetAssistantResponse>(command)
                } else {
                    transport.send<SetWorkspaceResponse>(command)
                }
                val info = transport.send<WorkspaceInfoResponse>(RemoteCommand(cmd = "get_workspace_info"))
                updateReady { it.copy(selected = info.asSelectedWorkspace(), busy = false) }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                _state.value = RemoteWorkspaceUiState.Failed(true)
            }
        }
    }

    private fun openFile(target: FilePreviewTarget) {
        val current = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        work?.cancel()
        _state.value = current.copy(preview = RemoteFilePreviewUiState.Loading(target))
        work = scope.launch {
            try {
                val info = transport.send<FileInfoResponse>(
                    RemoteCommand(cmd = "get_file_info", path = target.remotePath, sessionId = target.sessionId.ifEmpty { null }),
                )
                val size = info.size ?: 0
                val mime = info.mimeType ?: "application/octet-stream"
                when {
                    isText(mime, target.remotePath) -> loadText(target, info.name ?: basename(target.remotePath), size)
                    mime.startsWith("image/") && FilePreviewPolicy.canPreviewImage(size) ->
                        loadImage(target, info.name ?: basename(target.remotePath), mime, size)
                    mime.startsWith("image/") -> failPreview(target, "file too large")
                    else -> updateReady { it.copy(preview = RemoteFilePreviewUiState.Unsupported(target, mime)) }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                failPreview(target, error.message.orEmpty())
            }
        }
    }

    private fun resolveAndOpenFile(intent: RemoteWorkspaceIntent.OpenFile) {
        val ready = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        targetEpoch += 1
        val resolution = FileTargetResolver.resolve(
            reference = intent.reference,
            label = intent.label,
            context = FilePreviewTargetContext(
                sessionId = intent.sessionId,
                workspacePath = ready.selected?.path.orEmpty(),
                controlTargetEpoch = targetEpoch,
            ),
        )
        val resolvedTarget = resolution.target
        if (resolution.kind != FileReferenceKind.REMOTE_WORKSPACE_FILE || resolvedTarget == null) {
            val placeholder = FilePreviewTarget(
                intent.reference,
                intent.reference,
                intent.label,
                intent.sessionId,
                ready.selected?.path.orEmpty(),
                targetEpoch,
                0,
                0,
            )
            updateReady {
                it.copy(
                    preview = RemoteFilePreviewUiState.Failed(
                        placeholder,
                        FilePreviewFailureKind.UNAVAILABLE,
                        true,
                    ),
                )
            }
            return
        }
        openFile(resolvedTarget)
    }

    private suspend fun loadText(target: FilePreviewTarget, name: String, size: Long) {
        val limit = FilePreviewPolicy.textReadLimit(size).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        val response = readChunk(target, limit)
        val bytes = withContext(backgroundDispatcher) { decode(response.chunkBase64.orEmpty()) }
        val content = withContext(backgroundDispatcher) { bytes.decodeToString() }
        updateReady {
            it.copy(
                preview = RemoteFilePreviewUiState.Text(
                    target = target,
                    name = response.name ?: name,
                    content = content,
                    truncated = (response.totalSize ?: size) > bytes.size,
                ),
            )
        }
    }

    private suspend fun loadImage(target: FilePreviewTarget, name: String, mime: String, size: Long) {
        val response = readChunk(target, size.coerceAtLeast(1).coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
        val bytes = withContext(backgroundDispatcher) { decode(response.chunkBase64.orEmpty()) }
        updateReady {
            it.copy(
                preview = RemoteFilePreviewUiState.Image(
                    target = target,
                    name = response.name ?: name,
                    mimeType = response.mimeType ?: mime,
                    bytes = bytes,
                ),
            )
        }
    }

    private suspend fun readChunk(target: FilePreviewTarget, limit: Int): ReadFileChunkResponse =
        transport.send(
            RemoteCommand(
                cmd = "read_file_chunk",
                path = target.remotePath,
                sessionId = target.sessionId.ifEmpty { null },
                offset = 0,
                limit = limit,
            ),
        )

    private fun failPreview(target: FilePreviewTarget, message: String) {
        val failure = if (message.isBlank()) {
            FilePreviewFailure(FilePreviewFailureReason.LOAD_FAILED, true)
        } else {
            FilePreviewPolicy.failure(message)
        }
        updateReady {
            it.copy(
                preview = RemoteFilePreviewUiState.Failed(target, failure.toKind(), failure.retryable),
            )
        }
    }

    private fun updateReady(transform: (RemoteWorkspaceUiState.Ready) -> RemoteWorkspaceUiState.Ready) {
        val current = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        _state.value = transform(current)
    }

    private fun WorkspaceInfoResponse.asSelectedWorkspace(): SelectedWorkspace? {
        val path = resolvedPath.orEmpty()
        if (hasWorkspace != true && path.isEmpty()) return null
        return SelectedWorkspace(
            path = path,
            name = resolvedName?.takeIf(String::isNotBlank) ?: basename(path),
            gitBranch = gitBranch.orEmpty(),
            kind = workspaceKind.orEmpty(),
            assistantId = assistantId,
        )
    }

    private fun isText(mime: String, path: String): Boolean =
        mime.startsWith("text/") || mime in setOf("application/json", "application/xml", "application/javascript") ||
            path.substringAfterLast('.', "").lowercase() in TEXT_EXTENSIONS

    private fun decode(value: String): ByteArray = Base64.Default.decode(value)

    private fun basename(path: String): String = path.replace('\\', '/').substringAfterLast('/').ifEmpty { "file" }

    public companion object {
        internal fun create(scope: CoroutineScope, transport: RemoteCommandTransport): RemoteWorkspaceStore =
            RemoteWorkspaceStore(scope, transport, Dispatchers.Default)

        internal fun create(
            scope: CoroutineScope,
            transport: RemoteCommandTransport,
            backgroundDispatcher: CoroutineDispatcher,
        ): RemoteWorkspaceStore = RemoteWorkspaceStore(scope, transport, backgroundDispatcher)

        private val TEXT_EXTENSIONS = setOf(
            "md", "txt", "kt", "kts", "java", "swift", "rs", "ts", "tsx", "js", "jsx", "json", "xml",
            "yaml", "yml", "toml", "gradle", "properties", "sh", "py", "c", "cc", "cpp", "h", "hpp",
        )
    }
}
